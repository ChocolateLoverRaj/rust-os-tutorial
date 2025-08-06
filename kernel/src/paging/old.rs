use core::ptr::NonNull;

use common::PageSize;
use x86_64::{
    PhysAddr, VirtAddr,
    instructions::tlb::flush,
    structures::paging::{
        FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB,
        page_table::{FrameError, PageTableEntry},
    },
};

use crate::translate_addr::TranslateToVirt;

fn get_page_table_mut(page_table_entry: &mut PageTableEntry) -> Result<&mut PageTable, FrameError> {
    if !page_table_entry
        .flags()
        .contains(PageTableFlags::USER_ACCESSIBLE | PageTableFlags::WRITABLE)
    {
        panic!()
    }
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
    FrameAllocationFailed,
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
                .ok_or(MapPageError::FrameAllocationFailed)?;
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
    size: PageSize,
    page: VirtAddr,
    frame: PhysAddr,
    flags: PageTableFlags,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapPageError> {
    let mut p4_ptr =
        NonNull::new(p4_table.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    let p4 = unsafe { p4_ptr.as_mut() };
    let p3 = get_or_create_page_table(&mut p4[page.p4_index()], frame_allocator)?;
    if let PageSize::_1GiB = size {
        set_addr(
            &mut p3[page.p3_index()],
            frame,
            flags | PageTableFlags::PRESENT | PageTableFlags::HUGE_PAGE,
        )?;
        return Ok(());
    }
    let p2 = get_or_create_page_table(&mut p3[page.p3_index()], frame_allocator)?;
    if let PageSize::_2MiB = size {
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
///
/// # Safety
/// Don't unmap the wrong thing. It can cause page faults.
pub unsafe fn unmap_page(
    p4_table: PhysFrame<Size4KiB>,
    page_size: PageSize,
    page: VirtAddr,
) -> Result<PageTableEntry, UnmapPageError> {
    let mut p4_ptr =
        NonNull::new(p4_table.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    let p4 = unsafe { p4_ptr.as_mut() };
    let p3 = get_page_table_mut(&mut p4[page.p4_index()]).map_err(UnmapPageError::Frame)?;
    let entry = if let PageSize::_1GiB = page_size {
        unmap_entry(&mut p3[page.p3_index()], false)?
    } else {
        let p2 = get_page_table_mut(&mut p3[page.p3_index()]).map_err(UnmapPageError::Frame)?;
        if let PageSize::_2MiB = page_size {
            unmap_entry(&mut p2[page.p2_index()], false)?
        } else {
            let p1 = get_page_table_mut(&mut p2[page.p2_index()]).map_err(UnmapPageError::Frame)?;
            unmap_entry(&mut p1[page.p1_index()], true)?
        }
    };
    flush(page);
    Ok(entry)
}

#[derive(Debug)]
pub enum UpdateFlagsError {
    Frame(FrameError),
    /// You tried to set flags for an entire page table
    IsPageTable,
    NotMapped,
}

fn set_entry_flags(
    entry: &mut PageTableEntry,
    flags: PageTableFlags,
    is_p1: bool,
) -> Result<(), UpdateFlagsError> {
    if !entry.flags().contains(PageTableFlags::PRESENT) {
        return Err(UpdateFlagsError::NotMapped);
    }
    if !entry.flags().contains(PageTableFlags::HUGE_PAGE) && !is_p1 {
        return Err(UpdateFlagsError::IsPageTable);
    }
    entry.set_flags({
        let mut flags = PageTableFlags::PRESENT | flags;
        if !is_p1 {
            flags |= PageTableFlags::HUGE_PAGE;
        }
        flags
    });
    Ok(())
}

/// Updates flags to change RWX. Fails if the mapping is not present.
/// PRESENT and HUGE_PAGE flags are automatically added as needed.
/// Also does `invlpg` after successfully changing flags.
///
/// # Safety
/// Don't mess up page tables, don't give user mode access to things it shouldn't access, don't accidentally create multiple &mut T to the same data.
pub unsafe fn update_flags(
    p4_table: PhysFrame<Size4KiB>,
    page_size: PageSize,
    page: VirtAddr,
    flags: PageTableFlags,
) -> Result<(), UpdateFlagsError> {
    let mut p4_ptr =
        NonNull::new(p4_table.start_address().to_virt().as_mut_ptr::<PageTable>()).unwrap();
    let p4 = unsafe { p4_ptr.as_mut() };
    let p3 = get_page_table_mut(&mut p4[page.p4_index()]).map_err(UpdateFlagsError::Frame)?;
    if let PageSize::_1GiB = page_size {
        set_entry_flags(&mut p3[page.p3_index()], flags, false)?;
    } else {
        let p2 = get_page_table_mut(&mut p3[page.p3_index()]).map_err(UpdateFlagsError::Frame)?;
        if let PageSize::_2MiB = page_size {
            set_entry_flags(&mut p2[page.p2_index()], flags, false)?
        } else {
            let p1 =
                get_page_table_mut(&mut p2[page.p2_index()]).map_err(UpdateFlagsError::Frame)?;
            set_entry_flags(&mut p1[page.p1_index()], flags, true)?
        }
    };
    flush(page);
    Ok(())
}
