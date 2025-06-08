use alloc::slice;
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{Page, PageSize, PhysFrame, Size1GiB, Size2MiB, Size4KiB},
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

pub trait ZeroFrame {
    /// # Safety
    /// Frame must be offset mapped. Do not have another ref to the frame while zeroing it.
    unsafe fn zero(self);
}

impl ZeroFrame for PhysFrame<Size4KiB> {
    unsafe fn zero(self) {
        let ptr = self.start_address().to_virt().as_mut_ptr::<[u8; 0x1000]>();
        // Safety: frame is offset mapped
        unsafe {
            ptr.write_bytes(0, 1);
        };
    }
}

impl ZeroFrame for PhysFrame<Size2MiB> {
    unsafe fn zero(self) {
        let ptr = self
            .start_address()
            .to_virt()
            .as_mut_ptr::<[u8; 0x200000]>();
        // Safety: frame is offset mapped
        unsafe {
            ptr.write_bytes(0, 1);
        };
    }
}

impl ZeroFrame for PhysFrame<Size1GiB> {
    unsafe fn zero(self) {
        let ptr = self
            .start_address()
            .to_virt()
            .as_mut_ptr::<[u8; 0x40000000]>();
        // Safety: frame is offset mapped
        unsafe {
            ptr.write_bytes(0, 1);
        };
    }
}
