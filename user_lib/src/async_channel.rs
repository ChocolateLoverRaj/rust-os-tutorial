use core::task::Poll;

use common::log;

use crate::{ExecutorContext, syscall_create_channel, syscall_tx_send};

pub struct Sender {
    channel_id: u64,
}

impl Sender {
    pub fn send(&mut self) {
        syscall_tx_send(self.channel_id);
    }

    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    /// # Safety
    /// The channel with the id must exist, the calling process must own the Sender for it, and do not create multiple instances of [`Sender`] from the channel id.
    pub unsafe fn from_channel_id(channel_id: u64) -> Self {
        Self { channel_id }
    }
}

pub struct Receiver {
    channel_id: u64,
}

impl Receiver {
    pub fn receive<'a>(&'a mut self, executor_context: &'a ExecutorContext) -> ReceiveFuture<'a> {
        ReceiveFuture {
            receiver: self,
            executor_context,
        }
    }

    pub fn channel_id(&self) -> u64 {
        self.channel_id
    }

    /// # Safety
    /// The channel with the id must exist, the calling process must own the Receiver for it, and do not create multiple instances of [`Sender`] from the channel id.
    pub unsafe fn from_channel_id(channel_id: u64) -> Self {
        Self { channel_id }
    }
}

pub struct ReceiveFuture<'a> {
    receiver: &'a mut Receiver,
    executor_context: &'a ExecutorContext,
}
impl Future for ReceiveFuture<'_> {
    type Output = ();

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        if self.executor_context.take(self.receiver.channel_id) {
            Poll::Ready(())
        } else {
            self.executor_context
                .register_waker(self.receiver.channel_id, cx.waker());
            Poll::Pending
        }
    }
}

pub fn create() -> (Sender, Receiver) {
    let channel_id = syscall_create_channel();
    (Sender { channel_id }, Receiver { channel_id })
}
