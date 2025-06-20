use core::ops::{Deref, DerefMut};

use x2apic::lapic::IpiAllShorthand;
use x86_64::instructions::port::Port;

use crate::{
    cpu_local_data::get_local,
    interrupt_vector::InterruptVector,
    interrupted_context::InterruptedContext,
    run_tasks::run_threads,
    task::{EVENT_STREAMS, EventStreamSource, THREADS, ThreadReadyState, ThreadState},
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
        for (event_id, event_stream) in EVENT_STREAMS.read().deref() {
            if event_stream.source == ps2_source {
                event_stream.queue.force_push(data);
                for thread in threads.values() {
                    if thread.process.id == event_stream.process {
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
