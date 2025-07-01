use x86_64::structures::paging::{
    FrameAllocator, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};

use crate::translate_addr::{TranslateAddr, ZeroFrame};

pub unsafe fn map_page(
    l4_table: PhysFrame<Size4KiB>,
    page: Page<Size4KiB>,
    frame: PhysFrame<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    flags: PageTableFlags,
) {
    let l4_frame = unsafe {
        l4_table
            .start_address()
            .to_virt()
            .as_mut_ptr::<PageTable>()
            .as_mut()
            .unwrap()
    };
    let l4_entry = &mut l4_frame[page.p4_index()];
    if l4_entry.is_unused() {
        let l3_frame = frame_allocator.allocate_frame().unwrap();
        unsafe { l3_frame.zero() };
        l4_entry.set_frame(
            l3_frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
    }
    let l3_table = unsafe { l4_entry.addr().to_virt().as_mut_ptr::<PageTable>().as_mut() }.unwrap();
    let l3_entry = &mut l3_table[page.p3_index()];
    if l3_entry.is_unused() {
        let l2_frame = frame_allocator.allocate_frame().unwrap();
        unsafe { l2_frame.zero() };
        l3_entry.set_frame(
            l2_frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
    }
    let l2_table = unsafe { l3_entry.addr().to_virt().as_mut_ptr::<PageTable>().as_mut() }.unwrap();
    let l2_entry = &mut l2_table[page.p2_index()];
    if l2_entry.is_unused() {
        let l1_frame = frame_allocator.allocate_frame().unwrap();
        unsafe { l1_frame.zero() };
        l3_entry.set_frame(
            l1_frame,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE,
        );
    }
    let l1_table = unsafe { l2_entry.addr().to_virt().as_mut_ptr::<PageTable>().as_mut() }.unwrap();
    let l1_entry = &mut l1_table[page.p1_index()];
    l1_entry.set_frame(frame, flags);
}
