use core::sync::atomic::Ordering;

use common::{SyscallSubscribeToMouse, SyscallSubscribeToMouseError};
use crossbeam_queue::ArrayQueue;

use crate::{
    init_ps2_mouse,
    task::{EVENT_ID, EventStream, EventStreamSource, TASK},
};

use super::GenericSyscallHandler;

fn try_init() -> Result<(), ()> {
    let mut ps2 = unsafe { ps2::Controller::new() };
    ps2.enable_mouse().map_err(|_| ())?;
    ps2.mouse().set_defaults().map_err(|_| ())?;
    ps2.mouse().enable_data_reporting().map_err(|_| ())?;
    Ok(())
}

pub struct SyscallSubscribeToMouseHandler;
impl GenericSyscallHandler for SyscallSubscribeToMouseHandler {
    type S = SyscallSubscribeToMouse;
    fn handle_decoded_syscall(helper: super::SyscallHelper<Self::S>) -> ! {
        let output = {
            if init_ps2_mouse::mouse_exists() {
                let mut task = TASK.try_lock().unwrap();
                let task = task.as_mut().unwrap();
                let event_stream_id = EVENT_ID.fetch_add(1, Ordering::Relaxed);
                task.event_streams.insert(
                    event_stream_id,
                    EventStream {
                        source: EventStreamSource::Ps2Mouse,
                        queue: ArrayQueue::new(64),
                        pending_event: false,
                    },
                );
                Ok(event_stream_id)
            } else {
                Err(SyscallSubscribeToMouseError::NoPs2Mouse)
            }
        };
        helper.syscall_return(&output)
    }
}
