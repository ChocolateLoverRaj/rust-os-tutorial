use core::{mem::MaybeUninit, task::Poll};

use common::SyscallSubscribeToMouseError;
use futures::Stream;

use crate::{
    executor_context::ExecutorContext,
    syscalls::{syscall_read_mouse, syscall_subscribe_to_mouse},
};

pub struct AsyncMouse<'a> {
    executor_context: &'a ExecutorContext,
    event_id: u64,
}

impl<'a> AsyncMouse<'a> {
    pub fn new(
        executor_context: &'a ExecutorContext,
    ) -> Result<Self, SyscallSubscribeToMouseError> {
        Ok(Self {
            executor_context,
            event_id: syscall_subscribe_to_mouse()?,
        })
    }
}

impl Stream for AsyncMouse<'_> {
    type Item = u8;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        self.executor_context
            .register_waker(self.event_id, cx.waker());
        let mut buffer = [MaybeUninit::uninit(); 1];
        let item = syscall_read_mouse(&mut buffer);
        if let Some(item) = item.first() {
            Poll::Ready(Some(*item))
        } else {
            Poll::Pending
        }
    }
}
