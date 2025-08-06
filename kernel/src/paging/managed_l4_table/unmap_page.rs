use common::PageSize;
use x86_64::instructions::tlb::flush;

use crate::{Frame, ManagedL4PageTable, Page};

use super::page_table_with_level::{GetTableError, UnmapFrameError};

#[derive(Debug)]
pub enum UnmapPageError2 {
    GetTable(GetTableError),
    UnmapFrame(UnmapFrameError),
}

impl ManagedL4PageTable {
    /// Also does `invlpg` after successfully un-mapping.
    /// Returns the entry that was removed.
    ///
    /// # Safety
    /// Don't unmap the wrong thing. It can cause page faults.
    pub unsafe fn unmap_page(&mut self, page: Page) -> Result<Frame, UnmapPageError2> {
        let mut l4 = self.table_mut();
        let mut l3 = l4
            .entry_mut(page.start_addr().p4_index())
            .get_page_table_mut()
            .map_err(UnmapPageError2::GetTable)?;
        let mut l3_entry = l3.entry_mut(page.start_addr().p3_index());

        let mut entry = if let PageSize::_1GiB = page.size() {
            l3_entry
        } else {
            let mut l2 = l3_entry
                .get_page_table_mut()
                .map_err(UnmapPageError2::GetTable)?;
            let mut l2_entry = l2.entry_mut(page.start_addr().p2_index());
            if let PageSize::_2MiB = page.size() {
                l2_entry
            } else {
                let mut l1 = l2_entry
                    .get_page_table_mut()
                    .map_err(UnmapPageError2::GetTable)?;
                l1.entry_mut(page.start_addr().p1_index())
            }
        };
        let frame = entry.unmap_frame().map_err(UnmapPageError2::UnmapFrame)?;
        flush(page.start_addr());
        Ok(frame)
    }
}
