use alloc::slice;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PageSize, PhysFrame},
};

use crate::hhdm_offset::HhdmOffset;

pub trait TranslateToVirt {
    fn to_virt(self) -> VirtAddr;
}

impl TranslateToVirt for PhysAddr {
    fn to_virt(self) -> VirtAddr {
        VirtAddr::new(self.as_u64() + u64::from(HhdmOffset::get_from_response()))
    }
}

pub trait TranslateToPhys {
    /// Remember that not all virt addresses are offset mapped. Make sure your virt address is offset mapped.
    fn to_phys_offset_mapped(self) -> PhysAddr;
}

impl TranslateToPhys for VirtAddr {
    fn to_phys_offset_mapped(self) -> PhysAddr {
        PhysAddr::new(self.as_u64() - u64::from(HhdmOffset::get_from_response()))
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

pub trait TranslateFrame2 {
    fn to_page(self) -> ez_paging::Page;
}

impl TranslateFrame2 for ez_paging::Frame {
    fn to_page(self) -> ez_paging::Page {
        ez_paging::Page::new(self.start_addr().to_virt(), self.size()).unwrap()
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

pub trait ZeroFrame {
    /// # Safety
    /// Frame must be offset mapped. Do not have another ref to the frame while zeroing it.
    unsafe fn zero(self);
}

impl<S: PageSize> ZeroFrame for PhysFrame<S> {
    unsafe fn zero(self) {
        let ptr = self.start_address().to_virt().as_mut_ptr::<u8>();
        let len = S::SIZE as usize;
        // Safety: frame is offset mapped
        unsafe {
            ptr.write_bytes(0, len);
        };
    }
}

impl ZeroFrame for ez_paging::Frame {
    unsafe fn zero(self) {
        let ptr = self.start_addr().to_virt().as_mut_ptr::<u8>();
        let len = self.size().byte_len();
        // Safety: frame is offset mapped
        unsafe {
            ptr.write_bytes(0, len);
        };
    }
}
