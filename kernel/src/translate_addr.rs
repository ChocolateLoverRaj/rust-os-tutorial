use alloc::slice;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PageSize, PhysFrame},
};

use crate::hhdm_offset::HhdmOffset;

pub trait TranslateAddr {
    fn to_virt(self) -> VirtAddr;
}

impl TranslateAddr for PhysAddr {
    fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.as_u64() + u64::from(HhdmOffset::get_from_response()))
    }
}

pub trait TranslateFrame<S: PageSize> {
    fn to_page(self) -> Page<S>;
}

impl<S: PageSize> TranslateFrame<S> for PhysFrame<S> {
    fn to_page(self) -> Page<S> {
        Page::from_start_address(self.start_address().to_virt()).unwrap()
    }
}

pub trait GetFrameSlice {
    /// # Safety
    /// Follow Rust's rule of not having two mutable pointers to the physical memory at the same time.
    /// Don't have an immutable and mutable pointer at the same time either.
    unsafe fn get_slice<'a>(self) -> &'a [u8];

    /// # Safety
    /// Follow Rust's rule of not having two mutable pointers to the physical memory at the same time.
    /// Don't have an immutable and mutable pointer at the same time either.
    unsafe fn get_slice_mut<'a>(self) -> &'a mut [u8];
}

impl<S: PageSize> GetFrameSlice for PhysFrame<S> {
    unsafe fn get_slice<'a>(self) -> &'a [u8] {
        let ptr = self.start_address().to_virt().as_mut_ptr();
        let len = self.size() as usize;
        unsafe { slice::from_raw_parts(ptr, len) }
    }

    unsafe fn get_slice_mut<'a>(self) -> &'a mut [u8] {
        let ptr = self.start_address().to_virt().as_mut_ptr();
        let len = self.size() as usize;
        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }
}
