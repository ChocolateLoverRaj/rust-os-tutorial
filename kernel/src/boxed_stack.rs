use core::{any::type_name, cell::SyncUnsafeCell, fmt::Debug, mem::MaybeUninit};

use alloc::boxed::Box;
use x86_64::VirtAddr;

#[repr(C, align(16))]
struct StackChunk([u8; 16]);

pub struct BoxedStack {
    b: Box<[MaybeUninit<SyncUnsafeCell<StackChunk>>]>,
}

impl BoxedStack {
    pub fn new_uninit(stack_size_bytes: usize) -> Self {
        assert!(stack_size_bytes.is_multiple_of(size_of::<StackChunk>()));
        Self {
            b: Box::new_uninit_slice(stack_size_bytes / size_of::<StackChunk>()),
        }
    }

    pub fn top(&self) -> VirtAddr {
        VirtAddr::from_ptr(self.b.as_ptr_range().end)
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.b.len() * size_of::<StackChunk>()
    }
}

impl Debug for BoxedStack {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct(type_name::<Self>())
            .field("len", &format_args!("0x{:X}", self.len()))
            .field("top", &self.top())
            .finish()
    }
}
