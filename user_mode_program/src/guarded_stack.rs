use common::{SpawnThreadRelativePriority, SyscallAllocStackError, SyscallAllocStackOutput};

use crate::syscalls::{syscall_alloc_stack, syscall_spawn_thread};

/// An unmapped guard page with RW pages for the stack.
/// Dropping does not unmap the stack.
pub struct GuardedStack {
    top: u64,
}

impl GuardedStack {
    pub fn new(len: usize) -> Result<Self, SyscallAllocStackError> {
        Ok(Self {
            top: {
                let SyscallAllocStackOutput { usable_stack } = syscall_alloc_stack(len)?;
                usable_stack.pointer() + usable_stack.len()
            },
        })
    }

    pub fn spawn_thread(self, f: extern "sysv64" fn() -> !, priority: SpawnThreadRelativePriority) {
        // Safety: the stack has guard pages so it won't cause undefined behavior
        unsafe { syscall_spawn_thread(f, self.top as *const (), priority) };
    }
}
