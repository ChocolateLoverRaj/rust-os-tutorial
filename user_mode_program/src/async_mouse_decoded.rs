use core::task::Poll;

use common::SyscallSubscribeToMouseError;
use futures::{Stream, StreamExt};
use ps2_mouse::{MousePacket, MousePacketParser};

use crate::{async_mouse::AsyncMouse, executor_context::ExecutorContext};

pub struct AsyncMouseDecoded<'a> {
    async_mouse: AsyncMouse<'a>,
    packet_parser: MousePacketParser,
}

impl<'a> AsyncMouseDecoded<'a> {
    pub fn new(
        executor_context: &'a ExecutorContext,
    ) -> Result<Self, SyscallSubscribeToMouseError> {
        Ok(Self {
            async_mouse: AsyncMouse::new(executor_context)?,
            packet_parser: Default::default(),
        })
    }
}

impl Stream for AsyncMouseDecoded<'_> {
    type Item = MousePacket;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        let s = self.get_mut();
        loop {
            match s.async_mouse.poll_next_unpin(cx) {
                Poll::Pending => break Poll::Pending,
                Poll::Ready(None) => break Poll::Ready(None),
                Poll::Ready(Some(data)) => {
                    if let Some(packet) = s.packet_parser.add_byte(data) {
                        break Poll::Ready(Some(packet));
                    }
                }
            }
        }
    }
}
