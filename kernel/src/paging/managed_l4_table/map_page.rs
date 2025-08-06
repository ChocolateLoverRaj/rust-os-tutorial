use common::PageSize;
use x86_64::structures::paging::{FrameAllocator, PageTableFlags, Size4KiB};

use crate::{Frame, Page};

use super::{
    ManagedL4PageTable, PageTableWithLevelMut,
    page_table_with_level::{
        GetTableError, PageTableEntryWithLevelMut, SetFrameError, SetTableError,
    },
};

#[derive(Debug)]
pub enum MapPageError2 {
    FrameAllocationFailed,
    SetTable(SetTableError),
    GetTable(GetTableError),
    SetFrame(SetFrameError),
}

fn get_or_create_page_table<'a>(
    mut page_table_entry: PageTableEntryWithLevelMut<'a>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<PageTableWithLevelMut<'a>, MapPageError2> {
    Ok({
        if page_table_entry.is_empty() {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapPageError2::FrameAllocationFailed)?;
            page_table_entry
                .set_page_table(frame)
                .map_err(MapPageError2::SetTable)?
        } else {
            page_table_entry
                .get_page_table_mut()
                .map_err(MapPageError2::GetTable)?
        }
    })
}

impl ManagedL4PageTable {
    /// Maps a page to a phys frame.
    /// To avoid bugs, it is expected that the page is currently unmapped. It will error if the entry is not completely 0.
    ///
    /// PRESENT and HUGE_PAGE flags are automatically added as needed.
    ///
    /// # Safety
    /// Don't mess up page tables, don't give user mode access to things it shouldn't access, don't accidentally create multiple &mut T to the same data.
    pub unsafe fn map_page(
        &mut self,
        page: Page,
        frame: Frame,
        flags: PageTableFlags,
        frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    ) -> Result<(), MapPageError2> {
        let mut l4 = self.table_mut();
        let mut l3 =
            get_or_create_page_table(l4.entry_mut(page.start_addr().p4_index()), frame_allocator)?;
        if let PageSize::_1GiB = page.size() {
            l3.entry_mut(page.start_addr().p3_index())
                .set_frame(frame, flags)
                .map_err(MapPageError2::SetFrame)?;
            return Ok(());
        }
        let mut l2 =
            get_or_create_page_table(l3.entry_mut(page.start_addr().p3_index()), frame_allocator)?;
        if let PageSize::_2MiB = page.size() {
            l2.entry_mut(page.start_addr().p2_index())
                .set_frame(frame, flags)
                .map_err(MapPageError2::SetFrame)?;
            return Ok(());
        }
        let mut l1 =
            get_or_create_page_table(l2.entry_mut(page.start_addr().p2_index()), frame_allocator)?;
        l1.entry_mut(page.start_addr().p1_index())
            .set_frame(frame, flags)
            .map_err(MapPageError2::SetFrame)?;
        Ok(())
    }
}
