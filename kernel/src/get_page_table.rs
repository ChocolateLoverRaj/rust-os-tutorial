use core::mem::MaybeUninit;

use x86_64::structures::paging::{OffsetPageTable, PageTable, PhysFrame};

use crate::{hhdm_offset::HhdmOffset, translate_addr::TranslateAddr};

/// # Safety
/// The phys frame must be offset mapped and be a valid page table.
/// If `is_new` is true, the page will be set to a new empty page.
/// If it is false, then it will assume that the page table is valid.
pub unsafe fn get_page_table<'a>(l4_frame: PhysFrame, is_new: bool) -> OffsetPageTable<'a> {
    let page_table_ptr = l4_frame
        .start_address()
        .to_virt()
        .as_mut_ptr::<MaybeUninit<PageTable>>();
    // Safety: the frame is offset mapped
    let maybe_uninit_page_table = unsafe { page_table_ptr.as_mut() }.unwrap();
    let page_table = if is_new {
        // We initialize the page table to be blank
        maybe_uninit_page_table.write(Default::default())
    } else {
        // Safety: we are assuming that it's valid because is_new is false
        unsafe { maybe_uninit_page_table.assume_init_mut() }
    };
    // Safety: the HHDM and page table is valid
    unsafe { OffsetPageTable::new(page_table, HhdmOffset::get_from_response().into()) }
}
