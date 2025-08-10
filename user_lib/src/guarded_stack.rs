use core::num::NonZero;

use common::{
    MemProt, PageSize, SpawnThreadRelativePriority, SyscallAllocError, SyscallMemProt,
    SyscallMemProtError, SyscallMemProtInput, log,
};

use crate::{syscall, syscall_alloc, syscalls::syscall_spawn_thread};

/// An unmapped guard page with RW pages for the stack.
/// Dropping does not unmap the stack.
pub struct GuardedStack {
    top: u64,
}

#[derive(Debug)]
pub enum GuardedStackError {
    Alloc(SyscallAllocError),
    MemProt(SyscallMemProtError),
}

impl GuardedStack {
    pub fn new(len: usize) -> Result<Self, GuardedStackError> {
        Ok(Self {
            top: {
                let page_size = PageSize::_4KiB;
                let stack_pages_len = NonZero::new(len.div_ceil(page_size.byte_len())).unwrap();
                let output = syscall_alloc(
                    page_size,
                    stack_pages_len.checked_add(1).unwrap(),
                    false,
                    MemProt::empty(),
                )
                .map_err(GuardedStackError::Alloc)?;
                let input = SyscallMemProtInput {
                    page_size,
                    start_page_index: (output.addr().get() / page_size.byte_len() + 1)
                        .try_into()
                        .unwrap(),
                    pages_len: stack_pages_len,
                    new_prot: (MemProt::READABLE | MemProt::WRITABLE).bits(),
                };
                log::debug!("Alloc outpput: {output:p}. Input: {input:#?}");
                unsafe { syscall::<SyscallMemProt>(&input) }.map_err(GuardedStackError::MemProt)?;
                (output.addr().get() + len) as u64
            },
        })
    }

    pub fn spawn_thread(self, f: extern "sysv64" fn() -> !, priority: SpawnThreadRelativePriority) {
        // Safety: the stack has guard pages so it won't cause undefined behavior
        unsafe { syscall_spawn_thread(f, self.top as *const (), priority) };
    }
}
