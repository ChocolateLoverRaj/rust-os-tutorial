use alloc::collections::btree_map::BTreeMap;
use x86_64::{
    VirtAddr,
    structures::paging::{Page, PageSize, PageTableFlags, Size4KiB},
};

use crate::{
    call_with_rsp::call_with_rsp,
    local_apic_id::LocalApicId,
    memory::{KernelMemoryUsageType, MEMORY, MemoryType},
};

/// A stack with a guard page at the bottom.
/// Dropping this does not unmap the stack.
pub struct GuardedStack {
    top: VirtAddr,
}

impl GuardedStack {
    /// Locks physical and virtual memory to allocate the stack
    pub fn new(size: u64, stack_type: StackType) -> GuardedStack {
        let memory = MEMORY.get().unwrap();
        let mut physical_memory = memory.physical_memory.lock();
        let mut virtual_memory = memory.virtual_memory.lock();
        let n_mapped_pages = size.div_ceil(Size4KiB::SIZE);
        let n_virtual_pages = n_mapped_pages + 1;
        let mut allocated_pages = virtual_memory
            .allocate_contiguous_pages::<Size4KiB>(n_virtual_pages)
            .unwrap();
        // We purposely don't map the bottom page
        // so that it causes a page fault instead of silently overwriting data used for other purposes
        let guard_page = *allocated_pages.range().start();
        STACK_GUARD_PAGES.lock().insert(guard_page, stack_type);
        let start_page = guard_page + 1;
        for i in 0..n_mapped_pages {
            let page = start_page + i;
            let frame = physical_memory
                .allocate_frame_with_type(MemoryType::UsedByKernel(KernelMemoryUsageType::Stack))
                .unwrap();
            let flags =
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE;
            let mut frame_allocator = physical_memory.get_kernel_frame_allocator();
            unsafe { allocated_pages.map_to(page, frame, flags, &mut frame_allocator) };
        }
        GuardedStack {
            top: (start_page + n_mapped_pages).start_address(),
        }
    }

    pub fn top(&self) -> VirtAddr {
        self.top
    }

    pub fn switch(self, f: extern "sysv64" fn() -> !) -> ! {
        let new_rsp = self.top.as_u64();
        // Safety: The worst that can happen is a stack overflow, since we mapped a guard page
        unsafe { call_with_rsp(new_rsp, f) }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum StackType {
    FirstException(LocalApicId),
    DoubleFault(LocalApicId),
    Normal(LocalApicId),
}

pub static STACK_GUARD_PAGES: spin::Mutex<BTreeMap<Page, StackType>> =
    spin::Mutex::new(BTreeMap::new());
