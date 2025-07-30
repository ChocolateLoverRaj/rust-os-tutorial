use core::{fmt::Debug, num::NonZero};

use alloc::{collections::btree_set::BTreeSet, sync::Arc};
use common::{
    SpawnProcessMemoryFlags, SpawnProcessMemoryMapping, SpawnProcessRelativePriority, Syscall,
    SyscallSpawnProcess, SyscallSpawnProcessInput,
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
};

use super::GenericSyscallHandler;

pub struct SyscallSpawnProcessHandler;
impl GenericSyscallHandler for SyscallSpawnProcessHandler {
    type S = SyscallSpawnProcess;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let result = (|| {
            let input_ptr = *helper.input();
            let local = get_local();
            let mut running_thread_lock = local.running_thread.try_lock().unwrap();
            let running_thread_id = running_thread_lock.unwrap();
            let mut threads = THREADS.write();
            let running_thread = threads.get(&running_thread_id).unwrap();
            let mut process_memory = running_thread.process.memory.write();

            let interval = Interval::from(
                input_ptr
                    ..input_ptr
                        .checked_add(size_of::<SyscallSpawnProcessInput>() as u64)
                        .ok_or(())?,
            );
            if !(process_memory
                .mapped_virtual_memory
                .contains_interval(interval)
                && process_memory
                    .mapped_virtual_memory
                    .overlapping(interval)
                    .all(|(_, mem)| mem.permissions().read))
            {
                Err(())?
            }
            let input = {
                let data = input_ptr as *const u8;
                let len = size_of::<SyscallSpawnProcessInput>();
                unsafe { core::slice::from_raw_parts(data, len) }
            };
            let input = SyscallSpawnProcessInput::try_ref_from_bytes(input).map_err(|_| ())?;

            let interval = Interval::from(
                input.memory_mappings.pointer()
                    ..input
                        .memory_mappings
                        .pointer()
                        .checked_add(input.memory_mappings.len())
                        .ok_or(())?,
            );
            if !(process_memory
                .mapped_virtual_memory
                .contains_interval(interval)
                && process_memory
                    .mapped_virtual_memory
                    .overlapping(interval)
                    .all(|(_, mem)| mem.permissions().read))
            {
                Err(())?
            }
            let memory_mappings = unsafe {
                input
                    .memory_mappings
                    .try_to_slice::<SpawnProcessMemoryMapping>()
            }
            .ok_or(())?;

            let interval = Interval::from(
                input.send_capabilities.pointer()
                    ..input.send_capabilities.pointer() + input.send_capabilities.len(),
            );
            if !(process_memory
                .mapped_virtual_memory
                .contains_interval(interval)
                && process_memory
                    .mapped_virtual_memory
                    .overlapping(interval)
                    .all(|(_, mem)| mem.permissions().read))
            {
                Err(())?
            }
            let new_process_id = ProcessId::new_unique();
            let send_capabilities =
                unsafe { input.send_capabilities.try_to_slice::<u64>().ok_or(()) }?;
            let mut capabilities = CAPABILITIES.write();
            for capability in send_capabilities {
                let capability_id = NonZero::new(*capability).ok_or(())?;
                let capability = capabilities.get_mut(&capability_id).ok_or(())?;
                if capability.process_id != running_thread.process.id.into() {
                    Err(())?
                }
                capability.process_id = new_process_id.into();
            }

            let memory = MEMORY.get().unwrap();
            let mut physical_memory = memory.physical_memory.lock();
            let new_cr3 = physical_memory
                .allocate_frame_with_type::<Size4KiB>(MemoryType::UsedByUserMode(BTreeSet::from([
                    new_process_id,
                ])))
                .unwrap(); // TODO: Don't panic here
            let mut mapped_virtual_memory = NoditMap::default();
            let mut current_mapper = unsafe { get_page_table(running_thread.process.cr3, false) };
            let mut new_mapper = unsafe { get_page_table(new_cr3, true) };
            for i in 256..512 {
                new_mapper.level_4_table_mut()[i].clone_from(&current_mapper.level_4_table()[i]);
            }
            for memory_mapping in memory_mappings {
                // Don't let another thread modify this data while we're using it
                let memory_mapping = memory_mapping.clone();
                let interval = Interval::from(
                    memory_mapping.current_process_start
                        ..memory_mapping
                            .current_process_start
                            .checked_add(memory_mapping.len)
                            .ok_or(())?,
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
                        && process_memory
                            .mapped_virtual_memory
                            .overlapping(interval)
                            .all(|(_, mem)| mem.permissions().read)
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
                            .insert_merge_touching_if_values_equal(
                                interval,
                                UserVirtMem::Plain(memory_mapping_flags.into()),
                            )
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
                            .insert_merge_touching_if_values_equal(
                                interval,
                                UserVirtMem::Plain(memory_mapping_flags.into()),
                            )
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
                    )?;
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
                    )?;
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
                    )?;
                }
            }
            let new_thread_id = ThreadId::new_unique();
            drop(process_memory);
            *running_thread_lock = None;
            *running_thread.state.write() =
                ThreadState::Ready(ThreadReadyState::InSyscall(ThreadReadyStateInSyscall {
                    saved_regs: helper.saved_regs().clone(),
                    output: <Self::S as Syscall>::encode_output(&new_process_id.into()),
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
            Ok::<_, ()>(())
        })();
        match result {
            Err(_) => todo!(),
            Ok(_) => run_threads(),
        }
    }
}
