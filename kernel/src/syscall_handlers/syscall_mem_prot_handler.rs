use core::sync::atomic::AtomicUsize;

use common::{MemProt, Syscall, SyscallMemProt, SyscallMemProtError};
use itertools::Itertools;
use nodit::{InclusiveInterval, Interval};
use x2apic::lapic::IpiAllShorthand;
use x86_64::{VirtAddr, registers::model_specific::PatMemoryType};

use crate::{
    ConfigurableFlags, GetTableError, MapPageError2, Page, SetFlagsError, UnmapPageError2,
    UpdateFlagsError2,
    cpu_local_data::get_local,
    cpus_count,
    interrupt_vector::InterruptVector,
    memory::{MEMORY, MemoryType},
    run_tasks::run_threads,
    task::{FlushingTlbState, THREADS, ThreadReadyStateInSyscall, ThreadState, UserVirtMem},
};

use super::GenericSyscallHandler;

pub struct SyscallMemProtHandler;
impl GenericSyscallHandler for SyscallMemProtHandler {
    type S = SyscallMemProt;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        #[derive(Debug)]
        enum Action {
            Return,
            RunTasks,
        }
        let result = (|| {
            let threads = THREADS.read();
            let local = get_local();
            let mut running_thread = local.running_thread.lock();
            let thread = threads.get(&running_thread.unwrap()).unwrap();
            let current_process = &thread.process;
            // We still have to obtain a write lock to safely map pages
            let mut process_memory = current_process.memory.write();
            // Make sure the mem is plain
            let page_size = helper.input().page_size;
            let interval = Interval::from({
                let start = helper
                    .input()
                    .start_page_index
                    .get()
                    .checked_mul(page_size.byte_len())
                    .ok_or(SyscallMemProtError::InvalidInterval)?
                    as u64;
                start
                    ..start
                        .checked_add(
                            helper
                                .input()
                                .pages_len
                                .get()
                                .checked_mul(page_size.byte_len())
                                .ok_or(SyscallMemProtError::InvalidInterval)?
                                as u64,
                        )
                        .ok_or(SyscallMemProtError::InvalidInterval)?
            });
            log::debug!("Interval: {interval:X?}");
            let (overlapping_interval, mem) = process_memory
                .mapped_virtual_memory
                .overlapping(interval)
                .exactly_one()
                .map_err(|_| SyscallMemProtError::NotPlain)?;
            if !overlapping_interval.contains_interval(&interval) {
                return Err(SyscallMemProtError::NotPlain);
            }
            if !matches!(mem, UserVirtMem::Plain) {
                return Err(SyscallMemProtError::NotPlain);
            }
            let prot = MemProt::from_bits_retain(helper.input().new_prot);

            // Actually change mappings
            let mut needs_flush = false;
            for i in 0..helper.input().pages_len.get() {
                let page = Page::new(VirtAddr::new(interval.start()), page_size)
                    .unwrap()
                    .offset(i as u64)
                    .unwrap();
                needs_flush |= if prot.contains(MemProt::READABLE) {
                    let flags = ConfigurableFlags {
                        writable: prot.contains(MemProt::WRITABLE),
                        executable: prot.contains(MemProt::EXECUTABLE),
                        pat_memory_type: PatMemoryType::WriteBack,
                    };
                    let result = unsafe { process_memory.l4.update_flags(page, flags) };
                    if let Err(e) = result {
                        if let UpdateFlagsError2::SetFlags(SetFlagsError::NotPresent) = e {
                            let mut phys_mem = MEMORY.get().unwrap().physical_memory.lock();
                            let frame = if let Some(frame) = phys_mem.allocate_frame_with_type(
                                page_size,
                                MemoryType::UsedByUserMode(current_process.id),
                            ) {
                                frame
                            } else {
                                // TODO: Maybe cleanup
                                return Err(SyscallMemProtError::OutOfPhysMem);
                            };
                            let mut frame_allocator =
                                phys_mem.get_user_mode_program_frame_allocator(current_process.id);
                            let result = unsafe {
                                process_memory
                                    .l4
                                    .map_page(page, frame, flags, &mut frame_allocator)
                            };
                            if let Err(MapPageError2::FrameAllocationFailed) = &result {
                                // TODO: Cleanup
                                return Err(SyscallMemProtError::OutOfPhysMem);
                            }
                            false
                        } else {
                            panic!("Unexpected error: {e:#?}. Page: {page:?}");
                        }
                    } else {
                        true
                    }
                } else {
                    let result = unsafe { process_memory.l4.unmap_page(page) };
                    if let Err(e) = result {
                        if matches!(e, UnmapPageError2::GetTable(GetTableError::NotMapped)) {
                            false
                        } else {
                            panic!("Unexpected error: {e:#?}");
                        }
                    } else {
                        true
                    }
                };
            }
            // FIXME: invlpg only flushes the cached mapping on the current CPU. Other CPUs could still have the old mapping cache.
            // Have an AtomicUsize called "flush count"
            // Call invlpg on this cpu
            // Flush count is now 1
            // Send IPI to all other CPUs
            // Mark this task as "waiting for tlb flush"
            // Other CPUs flush the TLB for this process, depending on if PCID is enabled. They increase flush count.
            // If the flush count == total CPUs, they send a "run tasks" IPI
            // When checking if this task is ready, the flush count atomic var is referenced. If it == total CPUs, the state is changed to running.
            Ok(if needs_flush && cpus_count() > 1 {
                log::warn!("Need to flush");
                *thread.state.write() = ThreadState::FlushingTlb(FlushingTlbState {
                    flushed_count: AtomicUsize::new(1),
                    state: ThreadReadyStateInSyscall {
                        saved_regs: helper.saved_regs().clone(),
                        output: Self::S::encode_output(&Ok(())),
                    },
                });
                *running_thread = None;
                let mut local_apic = local.local_apic.get().unwrap().lock();
                unsafe {
                    local_apic.send_ipi_all(
                        InterruptVector::FlushTlb.into(),
                        IpiAllShorthand::AllExcludingSelf,
                    )
                };
                Action::RunTasks
            } else {
                Action::Return
            })
        })();
        match result {
            Ok(action) => match action {
                Action::Return => helper.syscall_return(&Ok(())),
                Action::RunTasks => run_threads(),
            },
            Err(e) => helper.syscall_return(&Err(e)),
        }
    }
}
