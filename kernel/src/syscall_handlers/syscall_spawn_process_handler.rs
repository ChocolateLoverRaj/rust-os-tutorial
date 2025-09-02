use core::{num::NonZero, ops::Deref, ptr::NonNull};

use alloc::{boxed::Box, format, sync::Arc};
use common::{
    LOWER_HALF_END, MapModule, PageSize, SpawnProcessMemoryFlags, SpawnProcessMemoryMapping,
    SpawnProcessRelativePriority, Syscall, SyscallSpawnProcess, SyscallSpawnProcessError,
    SyscallSpawnProcessInput,
};
use ez_paging::{ConfigurableFlags, Frame, MapPageError, Page};
use itertools::Itertools;
use nodit::{InclusiveInterval, Interval, NoditMap};
use x2apic::lapic::IpiAllShorthand;
use x86_64::{
    VirtAddr, registers::model_specific::PatMemoryType, structures::paging::FrameAllocator,
};
use zerocopy::TryFromBytes;

use crate::{
    CAPABILITIES,
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    limine_requests::MODULE_REQUEST,
    memory::MEMORY,
    run_tasks::run_threads,
    task::{
        Process, ProcessId, ProcessMemory, StartData, THREAD_PRIORITIES, THREADS, Thread, ThreadId,
        ThreadReadyState, ThreadReadyStateInSyscall, ThreadState, UserVirtMem,
    },
    translate_addr::TranslateToPhys,
    try_access_user_mem::try_access_user_mem,
};

use super::GenericSyscallHandler;

#[derive(Debug)]
enum CheckPointerError {
    AboveLowerHalf,
}

/// Checks that the pointer is not null and the pointer lies entirely in the lower half.
/// If you are just checking `T` and not `[T]` then use `1` as the slice len.
fn check_pointer<T>(
    ptr: NonZero<usize>,
    slice_len: usize,
) -> Result<NonNull<T>, CheckPointerError> {
    let data_len = size_of::<T>()
        .checked_mul(slice_len)
        .ok_or(CheckPointerError::AboveLowerHalf)?;
    let data_end_ptr = ptr
        .checked_add(data_len)
        .ok_or(CheckPointerError::AboveLowerHalf)?;
    if data_end_ptr.get() as u64 > LOWER_HALF_END {
        return Err(CheckPointerError::AboveLowerHalf);
    }
    Ok(NonNull::new(ptr.get() as *mut T).unwrap())
}

pub struct SyscallSpawnProcessHandler;
impl GenericSyscallHandler for SyscallSpawnProcessHandler {
    type S = SyscallSpawnProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let input_ptr =
                check_pointer::<[u8; size_of::<SyscallSpawnProcessInput>()]>(*helper.input(), 1)
                    .map_err(|_| SyscallSpawnProcessError::InvalidInputPtr)?;
            let input = try_access_user_mem(|| Box::new(unsafe { input_ptr.read() }))
                .map_err(|_e| SyscallSpawnProcessError::InvalidInputPtr)?;
            let input = SyscallSpawnProcessInput::try_ref_from_bytes(input.deref())
                .map_err(|_| SyscallSpawnProcessError::InvalidInput)?;

            let memory_mappings_ptr =
                check_pointer::<[u8; size_of::<SpawnProcessMemoryMapping>()]>(
                    input.send_memory.ptr,
                    input.send_memory.len,
                )
                .map_err(|_| SyscallSpawnProcessError::InvalidMemoryMappingPtr)?;
            let send_capabilities_ptr =
                check_pointer::<u64>(input.send_capabilities.ptr, input.send_capabilities.len)
                    .map_err(|_| SyscallSpawnProcessError::InvalidCapabilityPtr)?;
            let map_modules_ptr = check_pointer::<[u8; size_of::<MapModule>()]>(
                input.map_modules.ptr,
                input.map_modules.len,
            )
            .map_err(|_| SyscallSpawnProcessError::InvalidMapModulesPtr)?;

            let local = get_local();
            let mut running_thread_lock = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread_lock.unwrap();
            let mut threads = THREADS.write();
            let running_thread = threads.get(&running_thread_id).unwrap();

            // FIXME: Clean up if errors are found
            let new_process_id = ProcessId::new_unique();
            let mut capabilities = CAPABILITIES.write();
            for i in 0..input.send_capabilities.len {
                let capability_ptr = unsafe { send_capabilities_ptr.add(i) };
                let capability = try_access_user_mem(|| Box::new(unsafe { capability_ptr.read() }))
                    .map_err(|_e| SyscallSpawnProcessError::InvalidCapabilityPtr)?;
                let capability_id =
                    NonZero::new(*capability).ok_or(SyscallSpawnProcessError::CapabilityIdZero)?;
                let capability = capabilities
                    .get_mut(&capability_id)
                    .ok_or(SyscallSpawnProcessError::CapabilityNotFound)?;
                if capability.process_id != running_thread.process.id.into() {
                    Err(SyscallSpawnProcessError::CapabilityNotFound)?
                }
                capability.process_id = new_process_id.into();
            }

            let mut process_memory = running_thread.process.memory.write();
            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let mut new_process_l4 = {
                let frame = physical_memory
                    .get_user_mode_program_frame_allocator(new_process_id)
                    .allocate_frame()
                    .ok_or(SyscallSpawnProcessError::OutOfPhysMem)?;
                let mut virt_mem = memory.virtual_memory.lock();
                unsafe { virt_mem.new_user_page_table(frame) }
            };
            let mut new_virt_mem = NoditMap::default();
            for i in 0..input.map_modules.len {
                let map_module_ptr = unsafe { map_modules_ptr.add(i) };
                let map_module = try_access_user_mem(|| Box::new(unsafe { map_module_ptr.read() }))
                    .map_err(|_e| SyscallSpawnProcessError::InvalidMapModulesPtr)?;
                let map_module = MapModule::try_ref_from_bytes(map_module.deref())
                    .map_err(|_| SyscallSpawnProcessError::InvalidMapModule)?;
                let file = MODULE_REQUEST
                    .get_response()
                    .unwrap()
                    .modules()
                    .iter()
                    .find(|file| {
                        file.path().to_str()
                            == Ok(&format!("/extra_module_{}", map_module.module_id))
                    })
                    .ok_or(SyscallSpawnProcessError::ModuleNotFound)?;
                let file_frames = file.size().div_ceil(PageSize::_4KiB.byte_len_u64());
                if (map_module.start_page_offset as u64)
                    .checked_add(map_module.pages_len.get() as u64)
                    .ok_or(SyscallSpawnProcessError::InvalidModuleRange)?
                    > file_frames
                {
                    return Err(SyscallSpawnProcessError::OutOfModuleRange);
                }
                if !map_module
                    .new_process_start
                    .is_multiple_of(PageSize::_4KiB.byte_len())
                {
                    return Err(SyscallSpawnProcessError::ModuleUnalignedDest);
                }
                new_virt_mem
                    .insert_merge_touching_if_values_equal(
                        {
                            let start = map_module.new_process_start as u64;
                            let end = start
                                .checked_add(
                                    map_module.pages_len.get() as u64
                                        * PageSize::_4KiB.byte_len_u64(),
                                )
                                .ok_or(SyscallSpawnProcessError::InvalidModuleDest)?;
                            if end > LOWER_HALF_END {
                                return Err(SyscallSpawnProcessError::InvalidSendMemDestInterval);
                            }
                            start..end
                        }
                        .into(),
                        UserVirtMem::LimineModule,
                    )
                    .map_err(|_e| SyscallSpawnProcessError::DestMemOverlap)?;
                let start_page = Page::new(
                    VirtAddr::new(map_module.new_process_start as u64),
                    PageSize::_4KiB,
                )
                .unwrap();
                let start_frame = Frame::new(
                    VirtAddr::from_ptr(file.addr()).to_phys_offset_mapped(),
                    PageSize::_4KiB,
                )
                .unwrap()
                .offset(map_module.start_page_offset as u64)
                .unwrap();
                let mut frame_allocator = physical_memory
                    .get_user_mode_program_frame_allocator(running_thread.process.id);
                for i in 0..map_module.pages_len.get() {
                    let page = start_page.offset(i as u64).unwrap();
                    let frame = start_frame.offset(i as u64).unwrap();
                    let flags = ConfigurableFlags {
                        writable: false,
                        executable: map_module.executable,
                        pat_memory_type: PatMemoryType::WriteBack,
                    };
                    let result = unsafe {
                        new_process_l4.map_page(page, frame, flags, &mut frame_allocator)
                    };
                    if let Err(e) = &result {
                        match e {
                            MapPageError::FrameAllocationFailed => {
                                return Err(SyscallSpawnProcessError::OutOfPhysMem);
                            }
                            _ => result.unwrap(),
                        }
                    }
                }
                let last_frame_mapped = map_module.start_page_offset + map_module.pages_len.get()
                    == file_frames as usize;
                if last_frame_mapped {
                    let rem = file.size() as usize & PageSize::_4KiB.byte_len();
                    if rem != 0 {
                        // Zero unused bytes of last frame
                        // If this module gets mapped multiple times, we don't need to zero the bytes the 2nd+ times
                        // But to keep things simple we can just zero every time
                        let bytes_to_copy = PageSize::_4KiB.byte_len() - rem;
                        let copy_start = file.size() as usize - bytes_to_copy;
                        unsafe {
                            file.addr()
                                .add(copy_start)
                                .write_bytes(Default::default(), bytes_to_copy);
                        }
                    }
                }
            }

            for i in 0..input.send_memory.len {
                let memory_mapping_ptr = unsafe { memory_mappings_ptr.add(i) };
                let memory_mapping =
                    try_access_user_mem(|| Box::new(unsafe { memory_mapping_ptr.read() }))
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMappingPtr)?;
                let memory_mapping =
                    SpawnProcessMemoryMapping::try_ref_from_bytes(memory_mapping.deref())
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMapping)?;
                let memory_mapping_flags =
                    SpawnProcessMemoryFlags::from_bits_retain(memory_mapping.flags);
                let page_size = memory_mapping_flags.page_size();
                // Check that it's valid
                {
                    let interval = {
                        let start = u64::try_from(memory_mapping.current_process_start).unwrap();
                        Interval::from(
                            start
                                ..start
                                    .checked_add(
                                        u64::try_from(memory_mapping.pages_len).unwrap()
                                            * page_size.byte_len_u64(),
                                    )
                                    .ok_or(SyscallSpawnProcessError::InvalidSendMemSrcInterval)?,
                        )
                    };
                    let (overlapping_interval, mem) = process_memory
                        .mapped_virtual_memory
                        .overlapping(interval)
                        .exactly_one()
                        .map_err(|_| SyscallSpawnProcessError::SendMemSrcMix)?;
                    if !overlapping_interval.contains_interval(&interval) {
                        return Err(SyscallSpawnProcessError::SendMemSrcPartial);
                    }
                    if let UserVirtMem::Plain = mem {
                    } else {
                        return Err(SyscallSpawnProcessError::SendMemNotPlain);
                    }
                    let _ = process_memory.mapped_virtual_memory.cut(interval);
                }
                new_virt_mem
                    .insert_merge_touching_if_values_equal(
                        {
                            let start = u64::try_from(memory_mapping.new_process_start).unwrap();
                            Interval::from(
                                start
                                    ..start
                                        .checked_add(
                                            u64::try_from(memory_mapping.pages_len).unwrap()
                                                * page_size.byte_len_u64(),
                                        )
                                        .ok_or(
                                            SyscallSpawnProcessError::InvalidSendMemDestInterval,
                                        )?,
                            )
                        },
                        UserVirtMem::Plain,
                    )
                    .map_err(|_e| SyscallSpawnProcessError::DestMemOverlap)?;
                let start_page_current = Page::new(
                    VirtAddr::new(memory_mapping.current_process_start as u64),
                    page_size,
                )
                .unwrap();
                let start_page_new = Page::new(
                    VirtAddr::new(memory_mapping.new_process_start as u64),
                    page_size,
                )
                .unwrap();
                for i in 0..memory_mapping.pages_len {
                    let page = start_page_current.offset(i as u64).unwrap();
                    let frame = unsafe { process_memory.l4.unmap_page(page) }.unwrap();
                    physical_memory.change_owner(frame, new_process_id);
                    let page = start_page_new.offset(i as u64).unwrap();
                    let flags = ConfigurableFlags {
                        writable: memory_mapping_flags.contains(SpawnProcessMemoryFlags::WRITABLE),
                        executable: memory_mapping_flags
                            .contains(SpawnProcessMemoryFlags::EXECUTABLE),
                        pat_memory_type: PatMemoryType::WriteBack,
                    };
                    let mut frame_allocator =
                        physical_memory.get_user_mode_program_frame_allocator(new_process_id);
                    match unsafe {
                        new_process_l4.map_page(page, frame, flags, &mut frame_allocator)
                    } {
                        Ok(_) => {}
                        Err(e) => match e {
                            MapPageError::FrameAllocationFailed => {
                                return Err(SyscallSpawnProcessError::OutOfPhysMem);
                            }
                            e => unreachable!("{e:?}"),
                        },
                    }
                }
            }
            let new_thread_id = ThreadId::new_unique();
            drop(process_memory);
            *running_thread_lock = None;
            *running_thread.state.write() =
                ThreadState::Ready(ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                    saved_regs: helper.saved_regs().clone(),
                    output: <Self::S as Syscall>::encode_output(&Ok(new_process_id.into())),
                }));
            threads.insert(
                new_thread_id,
                Thread {
                    process: Arc::new(Process {
                        id: new_process_id,
                        memory: spin::RwLock::new(ProcessMemory {
                            mapped_virtual_memory: new_virt_mem,
                            l4: new_process_l4,
                        }),
                        mutexes: Default::default(),
                    }),
                    state: spin::RwLock::new(ThreadState::Ready(ThreadReadyState::ReadyToStart(
                        StartData {
                            rip: input.rip,
                            rsp: input.rsp,
                        },
                    ))),
                },
            );
            let mut thread_priorities = THREAD_PRIORITIES.write();
            let running_thread_position = thread_priorities
                .iter()
                .position(|thread_id| *thread_id == running_thread_id)
                .unwrap();
            let new_thread_position = match input.priority {
                SpawnProcessRelativePriority::Lower => running_thread_position + 1,
                SpawnProcessRelativePriority::Higher => running_thread_position,
            };
            thread_priorities.insert(new_thread_position, new_thread_id);
            let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
            unsafe {
                local_apic.send_ipi_all(
                    InterruptVector::CheckTasks.into(),
                    IpiAllShorthand::AllExcludingSelf,
                );
            }
            Ok(())
        })();
        match result {
            Err(e) => helper.syscall_return(&Err(e)),
            Ok(_) => run_threads(),
        }
    }
}
