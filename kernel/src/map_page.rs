use core::ptr::NonNull;

use common::AllocPageSize;
use x86_64::{
    PhysAddr, VirtAddr,
    instructions::tlb::flush,
    structures::paging::{
        FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB,
        page_table::{FrameError, PageTableEntry},
    },
};

use crate::translate_addr::TranslateAddr;

fn get_page_table_mut<'a>(
    page_table_entry: &'a mut PageTableEntry,
) -> Result<&'a mut PageTable, FrameError> {
    page_table_entry
        .frame(false)?
        .start_address()
        .to_virt()
        .as_mut_ptr::<PageTable>();
    let mut page_table_ptr =
        NonNull::new(page_table_entry.addr().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    unsafe { Ok(page_table_ptr.as_mut()) }
}

fn set_addr(
    page_table_entry: &mut PageTableEntry,
    addr: PhysAddr,
    flags: PageTableFlags,
) -> Result<(), MapPageError> {
    if page_table_entry.is_unused() {
        page_table_entry.set_addr(addr, flags);
        Ok(())
    } else {
        Err(MapPageError::AlreadyMapped)
    }
}

#[derive(Debug)]
pub enum MapPageError {
    AllocateFrame,
    Frame(FrameError),
    AlreadyMapped,
}

fn get_or_create_page_table<'a>(
    page_table_entry: &'a mut PageTableEntry,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<&'a mut PageTable, MapPageError> {
    Ok({
        if page_table_entry.is_unused() {
            let frame = frame_allocator
                .allocate_frame()
                .ok_or(MapPageError::AllocateFrame)?;
            let mut new_page_table =
                NonNull::new(frame.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
            unsafe { new_page_table.write_bytes(0, 1) };
            page_table_entry.set_frame(
                frame,
                PageTableFlags::PRESENT
                    | PageTableFlags::WRITABLE
                    | PageTableFlags::USER_ACCESSIBLE,
            );
            unsafe { new_page_table.as_mut() }
        } else {
            get_page_table_mut(page_table_entry).map_err(MapPageError::Frame)?
        }
    })
}

/// Maps a page to a phys frame.
/// To avoid bugs, it is expected that the page is currently unmapped. It will error if the entry is not completely 0.
///
/// PRESENT and HUGE_PAGE flags are automatically added as needed.
///
/// # Safety
/// Don't mess up page tables, don't give user mode access to things it shouldn't access, don't accidentally create multiple &mut T to the same data.
pub unsafe fn map_page(
    p4_table: PhysFrame<Size4KiB>,
    size: AllocPageSize,
    page: VirtAddr,
    frame: PhysAddr,
    flags: PageTableFlags,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapPageError> {
    let mut p4_ptr =
        NonNull::new(p4_table.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    let p4 = unsafe { p4_ptr.as_mut() };
    let p3 = get_or_create_page_table(&mut p4[page.p4_index()], frame_allocator)?;
    if let AllocPageSize::_1GiB = size {
        set_addr(
            &mut p3[page.p3_index()],
            frame,
            flags | PageTableFlags::PRESENT | PageTableFlags::HUGE_PAGE,
        )?;
        return Ok(());
    }
    let p2 = get_or_create_page_table(&mut p3[page.p3_index()], frame_allocator)?;
    if let AllocPageSize::_2MiB = size {
        set_addr(
            &mut p2[page.p2_index()],
            frame,
            flags | PageTableFlags::PRESENT | PageTableFlags::HUGE_PAGE,
        )?;
        return Ok(());
    }
    let p1 = get_or_create_page_table(&mut p2[page.p2_index()], frame_allocator)?;
    set_addr(
        &mut p1[page.p1_index()],
        frame,
        flags | PageTableFlags::PRESENT,
    )?;
    Ok(())
}

#[derive(Debug)]
pub enum UnmapPageError {
    Frame(FrameError),
    NotMapped,
    /// You tried to unmap an entire page table
    IsPageTable,
}

fn unmap_entry(entry: &mut PageTableEntry, is_p1: bool) -> Result<PageTableEntry, UnmapPageError> {
    if !entry.flags().contains(PageTableFlags::PRESENT) {
        return Err(UnmapPageError::NotMapped);
    }
    if !entry.flags().contains(PageTableFlags::HUGE_PAGE) && !is_p1 {
        return Err(UnmapPageError::IsPageTable);
    }
    let original_entry = entry.clone();
    entry.set_unused();
    Ok(original_entry)
}

/// Also does `invlpg` after successfully un-mapping.
/// Returns the entry that was removed.
pub unsafe fn unmap_page(
    p4_table: PhysFrame<Size4KiB>,
    page_size: AllocPageSize,
    page: VirtAddr,
) -> Result<PageTableEntry, UnmapPageError> {
    let mut p4_ptr =
        NonNull::new(p4_table.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    let p4 = unsafe { p4_ptr.as_mut() };
    let p3 = get_page_table_mut(&mut p4[page.p4_index()]).map_err(UnmapPageError::Frame)?;
    let entry = if let AllocPageSize::_1GiB = page_size {
        unmap_entry(&mut p3[page.p3_index()], false)?
    } else {
        let p2 = get_page_table_mut(&mut p3[page.p3_index()]).map_err(UnmapPageError::Frame)?;
        if let AllocPageSize::_2MiB = page_size {
            unmap_entry(&mut p2[page.p2_index()], false)?
        } else {
            let p1 = get_page_table_mut(&mut p2[page.p2_index()]).map_err(UnmapPageError::Frame)?;
            unmap_entry(&mut p1[page.p1_index()], true)?
        }
    };
    flush(page);
    Ok(entry)
}
