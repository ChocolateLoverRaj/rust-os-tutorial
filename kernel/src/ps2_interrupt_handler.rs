use core::{
    ops::{Deref, DerefMut},
    slice,
    sync::atomic::AtomicU8,
};

use common::QueueWriter;
use x2apic::lapic::IpiAllShorthand;
use x86_64::{instructions::port::Port, registers::control::Cr3};

use crate::{
    cpu_local_data::get_local,
    event_stream_mem::EventStreamMem,
    interrupt_vector::InterruptVector,
    interrupted_context::InterruptedContext,
    memory::MEMORY,
    run_tasks::run_threads,
    task::{EventStreamSource, PS2_EVENT_STREAMS, THREADS, ThreadReadyState, ThreadState},
    virt::LOWER_HALF_END,
};

/// # Safety
/// Must be called from an actual PS/2 interrupt handler
pub unsafe fn ps2_interrupt_handler(
    interrupted_context: &mut InterruptedContext,
    ps2_source: EventStreamSource,
) -> ! {
    {
        let mut port = Port::<u8>::new(0x60);
        let data = unsafe { port.read() };
        let local = get_local();

        let mut local_apic = local.local_apic.get().unwrap().try_lock().unwrap();
        unsafe { local_apic.end_of_interrupt() };

        let threads = THREADS.read();
        for (event_id, event_stream) in PS2_EVENT_STREAMS.read().deref() {
            if event_stream.source == ps2_source {
                {
                    unsafe {
                        let frame = event_stream.process.cr3;
                        let flags = MEMORY.get().unwrap().new_kernel_cr3_flags;
                        Cr3::write(frame, flags)
                    };
                }
                let mem_ptr = event_stream.ptr as *const EventStreamMem;
                let mem = unsafe { mem_ptr.as_ref() }.unwrap();
                let slots_len = mem.slots_len;
                let slots_ptr = (mem_ptr.addr() + size_of::<EventStreamMem>()) as *const AtomicU8;
                if !(slots_ptr.addr() + slots_len <= LOWER_HALF_END as usize) {
                    todo!()
                }
                let slots = unsafe { slice::from_raw_parts(slots_ptr, slots_len) };
                let mut writer = QueueWriter::new(&mem.write_count, &mem.read_count, &slots);
                let _ = writer.push(data);
                for thread in threads.values() {
                    if thread.process.id == event_stream.process.id {
                        let mut state = thread.state.write();
                        if let ThreadState::WaitingForEvents(state) = state.deref_mut()
                            && let Some(happened) = state.events.get_mut(event_id)
                        {
                            *happened = true;
                        }
                    }
                }
            }
        }

        // log::info!("Threads: {threads:#?}");
        unsafe {
            local_apic.send_ipi_all(
                InterruptVector::CheckTasks.into(),
                IpiAllShorthand::AllExcludingSelf,
            )
        };

        if let Some(running_thread_id) = local.running_thread.try_lock().unwrap().take() {
            *threads.get(&running_thread_id).unwrap().state.write() =
                ThreadState::Ready(ThreadReadyState::Interrupted(interrupted_context.clone()));
        }
        // log::info!("Running threads: {:#?}", threads.len());
    };
    run_threads()
}
