use core::alloc::Layout;

use alloc::alloc::{Allocator, Global};
use x86_64::{
    VirtAddr,
    instructions::tables::load_tss,
    registers::segmentation::{CS, DS, ES, SS, Segment},
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::cpu_local_data::get_local;

pub struct Gdt {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

pub const FIRST_EXCEPTION_STACK_INDEX: u16 = 0;
pub const DOUBLE_FAULT_STACK_INDEX: u16 = 1;

/// # Safety
/// This function must be called exactly once
pub unsafe fn init() {
    let local = get_local();
    let tss = {
        local
            .tss
            .try_init_once(|| {
                let mut tss = TaskStateSegment::new();
                tss.interrupt_stack_table[FIRST_EXCEPTION_STACK_INDEX as usize] =
                    VirtAddr::from_ptr(unsafe {
                        Global
                            .allocate(Layout::from_size_align(4 * 0x400, 16).unwrap())
                            .unwrap()
                            .as_uninit_slice_mut()
                            .as_mut_ptr_range()
                            .end
                    });
                tss.interrupt_stack_table[DOUBLE_FAULT_STACK_INDEX as usize] =
                    VirtAddr::from_ptr(unsafe {
                        Global
                            .allocate(Layout::from_size_align(4 * 0x400, 16).unwrap())
                            .unwrap()
                            .as_uninit_slice_mut()
                            .as_mut_ptr_range()
                            .end
                    });
                tss
            })
            .unwrap();
        local.tss.try_get().unwrap()
    };
    let gdt = {
        local
            .gdt
            .try_init_once(|| {
                let mut gdt = GlobalDescriptorTable::new();
                let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
                let tss_selector = gdt.append(Descriptor::tss_segment(tss));
                Gdt {
                    gdt,
                    kernel_code_selector,
                    tss_selector,
                }
            })
            .unwrap();
        local.gdt.try_get().unwrap()
    };
    gdt.gdt.load();
    unsafe { CS::set_reg(gdt.kernel_code_selector) };
    unsafe { SS::set_reg(SegmentSelector::NULL) };
    unsafe { DS::set_reg(SegmentSelector::NULL) };
    unsafe { ES::set_reg(SegmentSelector::NULL) };
    unsafe { load_tss(gdt.tss_selector) };
}
