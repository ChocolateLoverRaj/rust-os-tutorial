use core::{fmt::Debug, num::NonZero, ops::Deref};

use alloc::{boxed::Box, collections::btree_set::BTreeSet, sync::Arc};
use common::{
    LOWER_HALF_END, SpawnProcessMemoryFlags, SpawnProcessMemoryMapping,
    SpawnProcessRelativePriority, Syscall, SyscallSpawnProcess, SyscallSpawnProcessError,
    SyscallSpawnProcessInput,
};
use nodit::{Interval, NoditMap};
use x2apic::lapic::IpiAllShorthand;
use x86_64::{
    VirtAddr,
    structures::paging::{
        Mapper, Page, PageSize, PageTableFlags, Size1GiB, Size2MiB, Size4KiB,
        mapper::{TranslateError, UnmapError},
    },
};
use zerocopy::TryFromBytes;

use crate::{
    CAPABILITIES,
    cpu_local_data::get_local,
    get_page_table::get_page_table,
    interrupt_vector::InterruptVector,
    memory::{MEMORY, MemoryType, PhysicalMemory},
    run_tasks::run_threads,
    task::{
        Process, ProcessId, ProcessMappedVirtMem, ProcessMemory, StartData, THREAD_PRIORITIES,
        THREADS, Thread, ThreadId, ThreadReadyState, ThreadReadyStateInSyscall, ThreadState,
        UserVirtMem,
    },
    try_access_user_mem::try_access_user_mem,
};

use super::GenericSyscallHandler;

pub struct SyscallSpawnProcessHandler;
impl GenericSyscallHandler for SyscallSpawnProcessHandler {
    type S = SyscallSpawnProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let input_ptr = *helper.input();
            if input_ptr == 0
                || input_ptr + u64::try_from(size_of::<SyscallSpawnProcessInput>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }
            let input_ptr = input_ptr as *const [u8; size_of::<SyscallSpawnProcessInput>()];
            let input = try_access_user_mem(|| Box::new(unsafe { input_ptr.read() }))
                .map_err(|_e| SyscallSpawnProcessError::InvalidInputPtr)?;
            let input = SyscallSpawnProcessInput::try_ref_from_bytes(input.deref())
                .map_err(|_| SyscallSpawnProcessError::InvalidInput)?;
            if input.memory_mappings.pointer() == 0
                || input.memory_mappings.pointer()
                    + input.memory_mappings.len()
                        * u64::try_from(size_of::<SpawnProcessMemoryMapping>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }
            if input.send_capabilities.pointer() == 0
                || input.send_capabilities.pointer()
                    + input.send_capabilities.len()
                        * u64::try_from(size_of::<NonZero<u64>>()).unwrap()
                    > LOWER_HALF_END
            {
                Err(SyscallSpawnProcessError::InvalidInputPtr)?
            }

            let local = get_local();
            let mut running_thread_lock = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread_lock.unwrap();
            let mut threads = THREADS.write();
            let running_thread = threads.get(&running_thread_id).unwrap();

            // FIXME: Clean up if errors are found
            let new_process_id = ProcessId::new_unique();
            let mut capabilities = CAPABILITIES.write();
            for i in 0..input.send_capabilities.len() as usize {
                let capability = try_access_user_mem(|| {
                    let capability_ptr = (input.send_capabilities.pointer() as usize
                        + i * size_of::<u64>())
                        as *const u64;
                    Box::new(unsafe { capability_ptr.read() })
                })
                .map_err(|_e| SyscallSpawnProcessError::InvalidCapabilityPtr)?;
                let capability_id = NonZero::new(*capability)
                    .ok_or(SyscallSpawnProcessError::InvalidCapabilityId)?;
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
            let new_cr3 = physical_memory
                .allocate_frame_with_type::<Size4KiB>(MemoryType::UsedByUserMode(BTreeSet::from([
                    new_process_id,
                ])))
                .ok_or(SyscallSpawnProcessError::OutOfPhysMem)?;
            let mut mapped_virtual_memory = NoditMap::default();
            let mut current_mapper = unsafe { get_page_table(running_thread.process.cr3, false) };
            let mut new_mapper = unsafe { get_page_table(new_cr3, true) };
            for i in 256..512 {
                new_mapper.level_4_table_mut()[i].clone_from(&current_mapper.level_4_table()[i]);
            }

            for i in 0..input.memory_mappings.len() as usize {
                let memory_mapping_ptr = (input.memory_mappings.pointer() as usize
                    + i * size_of::<SpawnProcessMemoryMapping>())
                    as *const [u8; size_of::<SpawnProcessMemoryMapping>()];
                let memory_mapping =
                    try_access_user_mem(|| Box::new(unsafe { memory_mapping_ptr.read() }))
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMappingPtr)?;
                let memory_mapping =
                    SpawnProcessMemoryMapping::try_ref_from_bytes(memory_mapping.deref())
                        .map_err(|_e| SyscallSpawnProcessError::InvalidMemoryMapping)?;
                let interval = Interval::from(
                    memory_mapping.current_process_start
                        ..memory_mapping
                            .current_process_start
                            .checked_add(memory_mapping.len)
                            .ok_or(SyscallSpawnProcessError::InvalidMemoryMappingSrc)?,
                );
                #[allow(clippy::too_many_arguments)]
                fn map_region_with_page_size<S: PageSize + Debug>(
                    memory_mapping: &SpawnProcessMemoryMapping,
                    mapped_virtual_memory: &mut ProcessMappedVirtMem,
                    current_mapper: &mut impl Mapper<S>,
                    new_mapper: &mut impl Mapper<S>,
                    physical_memory: &mut PhysicalMemory,
                    new_process_id: ProcessId,
                    process_memory: &mut ProcessMemory,
                    running_thread: &Thread,
                    interval: Interval<u64>,
                ) -> Result<(), ()> {
                    if !(process_memory
                        .mapped_virtual_memory
                        .contains_interval(interval)
                        && memory_mapping.current_process_start.is_multiple_of(S::SIZE)
                        && memory_mapping.len.is_multiple_of(S::SIZE))
                        && memory_mapping.new_process_start.is_multiple_of(S::SIZE)
                        && memory_mapping
                            .new_process_start
                            .checked_add(memory_mapping.len)
                            .ok_or(())?
                            <= 0x800000000000
                    {
                        Err(())?
                    }
                    let memory_mapping_flags =
                        SpawnProcessMemoryFlags::from_bits_retain(memory_mapping.flags);
                    if memory_mapping_flags.contains(SpawnProcessMemoryFlags::SHARE) {
                        let interval = Interval::from(
                            memory_mapping.new_process_start
                                ..memory_mapping.new_process_start + memory_mapping.len,
                        );
                        mapped_virtual_memory
                            .insert_merge_touching_if_values_equal(interval, UserVirtMem::Plain)
                            .map_err(|_| ())?;
                        let start_page_current = Page::<S>::from_start_address(VirtAddr::new(
                            memory_mapping.current_process_start,
                        ))
                        .unwrap();
                        let start_page_new = Page::<S>::from_start_address(VirtAddr::new(
                            memory_mapping.new_process_start,
                        ))
                        .unwrap();
                        let page_count = memory_mapping.len / S::SIZE;
                        for i in 0..page_count {
                            let page = start_page_current + i;
                            let frame =
                                current_mapper.translate_page(page).map_err(|e| match e {
                                    TranslateError::ParentEntryHugePage => (),
                                    e => panic!(
                                        "Unexpected translate error: {e:?}. Page size: {:?}",
                                        S::DEBUG_STR
                                    ),
                                })?;
                            physical_memory.share_memory(frame, new_process_id);
                            // physical_memory.
                            let page = start_page_new + i;
                            let flags = PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | memory_mapping_flags.into();
                            let mut frame_allocator = physical_memory
                                .get_user_mode_program_frame_allocator(new_process_id);
                            // FIXME: Gracefully handle out of memory
                            unsafe { new_mapper.map_to(page, frame, flags, &mut frame_allocator) }
                                .unwrap()
                                .flush();
                        }
                    } else {
                        let _ = process_memory.mapped_virtual_memory.cut(interval);
                        let interval = Interval::from(
                            memory_mapping.new_process_start
                                ..memory_mapping.new_process_start + memory_mapping.len,
                        );
                        mapped_virtual_memory
                            .insert_merge_touching_if_values_equal(interval, UserVirtMem::Plain)
                            .map_err(|_| ())?;
                        let start_page_current = Page::<S>::from_start_address(VirtAddr::new(
                            memory_mapping.current_process_start,
                        ))
                        .unwrap();
                        let start_page_new = Page::<S>::from_start_address(VirtAddr::new(
                            memory_mapping.new_process_start,
                        ))
                        .unwrap();
                        let page_count = memory_mapping.len / S::SIZE;
                        for i in 0..page_count {
                            let page = start_page_current + i;
                            let (frame, _flags, flush) =
                                current_mapper.unmap(page).map_err(|e| match e {
                                    UnmapError::ParentEntryHugePage => (),
                                    e => panic!("Unexpected unmap error: {e:?}"),
                                })?;
                            flush.flush();
                            physical_memory.share_memory(frame, new_process_id);
                            physical_memory.unshare_memory(frame, running_thread.process.id);
                            let page = start_page_new + i;
                            let flags = PageTableFlags::PRESENT
                                | PageTableFlags::USER_ACCESSIBLE
                                | memory_mapping_flags.into();
                            let mut frame_allocator = physical_memory
                                .get_user_mode_program_frame_allocator(new_process_id);
                            // FIXME: Gracefully handle out of memory
                            unsafe { new_mapper.map_to(page, frame, flags, &mut frame_allocator) }
                                .unwrap()
                                .flush();
                        }
                    }
                    Ok(())
                }
                let memory_mapping_flags =
                    SpawnProcessMemoryFlags::from_bits_retain(memory_mapping.flags);
                if memory_mapping_flags.contains(SpawnProcessMemoryFlags::_1GiB_PAGE) {
                    map_region_with_page_size::<Size1GiB>(
                        &memory_mapping,
                        &mut mapped_virtual_memory,
                        &mut current_mapper,
                        &mut new_mapper,
                        &mut physical_memory,
                        new_process_id,
                        &mut process_memory,
                        running_thread,
                        interval,
                    )
                    .unwrap();
                } else if memory_mapping_flags.contains(SpawnProcessMemoryFlags::_2MiB_PAGE) {
                    map_region_with_page_size::<Size2MiB>(
                        &memory_mapping,
                        &mut mapped_virtual_memory,
                        &mut current_mapper,
                        &mut new_mapper,
                        &mut physical_memory,
                        new_process_id,
                        &mut process_memory,
                        running_thread,
                        interval,
                    )
                    .unwrap();
                } else {
                    map_region_with_page_size::<Size4KiB>(
                        &memory_mapping,
                        &mut mapped_virtual_memory,
                        &mut current_mapper,
                        &mut new_mapper,
                        &mut physical_memory,
                        new_process_id,
                        &mut process_memory,
                        running_thread,
                        interval,
                    )
                    .unwrap();
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
                        cr3: new_cr3,
                        id: new_process_id,
                        memory: spin::RwLock::new(ProcessMemory {
                            mapped_virtual_memory,
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
