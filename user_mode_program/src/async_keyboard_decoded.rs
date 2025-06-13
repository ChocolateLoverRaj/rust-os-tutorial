use core::task::Poll;

use futures::{Stream, StreamExt};
use pc_keyboard::{HandleControl, KeyEvent, Keyboard, KeyboardLayout, ScancodeSet};

use crate::{async_keyboard::AsyncKeyboard, executor_context::ExecutorContext};

pub struct AsyncKeyboardDecoded<'a, L: KeyboardLayout, S: ScancodeSet> {
    async_keyboard: AsyncKeyboard<'a>,
    keyboard: Keyboard<L, S>,
}

impl<'a, L: KeyboardLayout, S: ScancodeSet> AsyncKeyboardDecoded<'a, L, S> {
    pub fn new(
        executor_context: &'a ExecutorContext,
        scancode_set: S,
        layout: L,
        handle_ctrl: HandleControl,
    ) -> Self {
        Self {
            async_keyboard: AsyncKeyboard::new(executor_context),
            keyboard: Keyboard::new(scancode_set, layout, handle_ctrl),
        }
    }
}

impl<L: KeyboardLayout + Unpin, S: ScancodeSet + Unpin> Stream for AsyncKeyboardDecoded<'_, L, S> {
    type Item = Result<KeyEvent, pc_keyboard::Error>;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        let s = self.get_mut();
        loop {
            match s.async_keyboard.poll_next_unpin(cx) {
                Poll::Pending => break Poll::Pending,
                Poll::Ready(data) => {
                    if let Some(data) = data {
                        match s.keyboard.add_byte(data) {
                            Ok(Some(key_event)) => {
                                break Poll::Ready(Some(Ok(key_event)));
                            }
                            Ok(None) => {}
                            Err(error) => break Poll::Ready(Some(Err(error))),
                        }
                    } else {
                        break Poll::Ready(None);
                    }
                }
            }
        }
    }
}
