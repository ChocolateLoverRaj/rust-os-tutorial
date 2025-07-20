use core::{
    mem::MaybeUninit,
    slice::{self},
    sync::atomic::{AtomicU8, AtomicUsize},
    task::Poll,
};

use alloc::boxed::Box;
use common::{
    ENV_PS2_KEYBOARD_CAPABILITY, QueueReader, SyscallSubscribeToKeyboard,
    SyscallSubscribeToKeyboardInput,
};
use futures::Stream;

use crate::{EnvEntries, ExecutorContext, syscall};

#[derive(Debug)]
#[repr(C)]
struct M {
    slots_len: usize,
    write_count: AtomicUsize,
    read_count: AtomicUsize,
    slots: [AtomicU8; 0],
}

impl M {
    pub fn size(slots_len: usize) -> usize {
        size_of::<Self>() + size_of::<AtomicU8>() * slots_len
    }
}

pub struct AsyncKeyboard<'a> {
    executor_context: &'a ExecutorContext,
    event_id: u64,
    b: Box<[MaybeUninit<usize>]>,
}

impl<'a> AsyncKeyboard<'a> {
    /// Slots_len will be rounded up to `size_of::<usize>()`
    pub fn new(env: &EnvEntries, executor_context: &'a ExecutorContext, slots_len: usize) -> Self {
        // We use usize for alignment
        let len = M::size(slots_len).div_ceil(size_of::<usize>());
        let mut b = Box::<[usize]>::new_uninit_slice(len);
        let mem = b.as_mut_ptr().cast::<MaybeUninit<M>>();
        // Safety: The pointer is convertible to M
        let mem = unsafe { mem.as_mut() }.unwrap();
        let mem = mem.write(M {
            slots_len,
            read_count: AtomicUsize::new(0),
            write_count: AtomicUsize::new(0),
            slots: [],
        });
        let input = SyscallSubscribeToKeyboardInput {
            capability: *env.get(&ENV_PS2_KEYBOARD_CAPABILITY).unwrap(),
            queue_ptr: mem as *mut _ as u64,
        };
        // Safety: The pointer points to valid memory
        let event_id = unsafe { syscall::<SyscallSubscribeToKeyboard>(&input) }.unwrap();
        Self {
            executor_context,
            event_id,
            b,
        }
    }
}

impl Drop for AsyncKeyboard<'_> {
    fn drop(&mut self) {
        self.executor_context.take(self.event_id);
        // TODO: Tell kernel that we're unsubscribing to the keyboard
        todo!()
    }
}

impl Stream for AsyncKeyboard<'_> {
    type Item = u8;

    fn poll_next(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        let s = self.get_mut();
        s.executor_context.take(s.event_id);
        let mem_ptr = s.b.as_mut_ptr().cast::<M>();
        let mem = unsafe { mem_ptr.as_mut() }.unwrap();
        let shared_slots_ptr = (s.b.as_ptr().addr() + size_of::<M>()) as *const AtomicU8;
        let shared_slots = unsafe { slice::from_raw_parts(shared_slots_ptr, mem.slots_len) };
        let mut reader = QueueReader::new(&mem.write_count, &mem.read_count, shared_slots);
        if let Some(data) = reader.pop() {
            Poll::Ready(Some(data))
        } else {
            s.executor_context.register_waker(s.event_id, cx.waker());
            Poll::Pending
        }
    }
}
