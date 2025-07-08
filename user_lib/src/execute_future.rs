use core::{
    future::Future,
    task::{Context, Poll, Waker},
};

use futures::pin_mut;

use crate::{executor_context::ExecutorContext, syscall_wait_until_event};

/// Execute a single future
pub fn execute_future<T>(executor_context: &ExecutorContext, future: impl Future<Output = T>) -> T {
    pin_mut!(future);
    // We don't care about getting woken up because we will call poll after receiving any event
    let waker = Waker::noop();
    let mut context = Context::from_waker(waker);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => break value,
            Poll::Pending => {}
        }
        let mut events_buffer = executor_context.event_not_happened();
        // FIXME: If there is a keyboard event that we didn't read yet (because we're waiting for something else), then this will instantly return, resulting in a busy loop forever
        syscall_wait_until_event(&mut events_buffer)
            .iter()
            .for_each(|event_id| {
                executor_context.wake(*event_id);
            });
    }
}
