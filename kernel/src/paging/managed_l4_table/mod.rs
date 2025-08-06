use core::ptr::NonNull;

use page_table_with_level::{PageTableLevel, PageTableWithLevelMut};
use x86_64::structures::paging::{PageTable, PhysFrame};

use crate::translate_addr::TranslateToVirt;

mod map_page;
mod page_table_with_level;
mod unmap_page;
mod update_flags;

#[derive(Debug)]
pub struct ManagedL4PageTable {
    frame: PhysFrame,
}

impl ManagedL4PageTable {
    /// This method also zeroes the frame.
    ///
    /// # Safety
    /// You must "own" the frame (nothing else can reference it). The page table must be valid.
    pub unsafe fn new(frame: PhysFrame) -> Self {
        {
            let ptr = frame.start_address().to_virt().as_mut_ptr::<PageTable>();
            unsafe { ptr.write_bytes(0, 1) };
        }
        Self { frame }
    }

    fn table_mut(&mut self) -> PageTableWithLevelMut {
        let mut ptr = NonNull::new(
            self.frame
                .start_address()
                .to_virt()
                .as_mut_ptr::<PageTable>(),
        )
        .unwrap();
        PageTableWithLevelMut {
            page_table: unsafe { ptr.as_mut() },
            level: PageTableLevel::L4,
        }
    }
}
