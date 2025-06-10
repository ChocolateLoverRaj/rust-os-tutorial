use x86_64::{
    instructions::tables::load_tss,
    registers::{
        model_specific::Star,
        segmentation::{CS, SS, Segment},
    },
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::{boxed_stack::BoxedStack, cpu_local_data::get_local};

pub struct TssStacks {
    first_exception: BoxedStack,
    double_fault: BoxedStack,
    privilege_switch: BoxedStack,
}

pub struct Gdt {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
}

pub const FIRST_EXCEPTION_STACK_INDEX: u16 = 0;
pub const DOUBLE_FAULT_STACK_INDEX: u16 = 1;

/// # Safety
/// This function must be called exactly once
pub unsafe fn init() {
    let local = get_local();
    let tss_stacks = local.tss_stacks.call_once(|| TssStacks {
        first_exception: BoxedStack::new_uninit(64 * 0x400),
        double_fault: BoxedStack::new_uninit(64 * 0x400),
        privilege_switch: BoxedStack::new_uninit(64 * 0x400),
    });
    let tss = local.tss.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[FIRST_EXCEPTION_STACK_INDEX as usize] =
            tss_stacks.first_exception.top();
        tss.interrupt_stack_table[DOUBLE_FAULT_STACK_INDEX as usize] =
            tss_stacks.double_fault.top();
        tss.privilege_stack_table[0] = tss_stacks.privilege_switch.top();
        tss
    });
    let gdt = local.gdt.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        // Changing the order of these could mess things up!
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        Gdt {
            gdt,
            kernel_code_selector,
            kernel_data_selector,
            tss_selector,
            user_code_selector,
            user_data_selector,
        }
    });
    gdt.gdt.load();
    unsafe { CS::set_reg(gdt.kernel_code_selector) };
    unsafe { SS::set_reg(gdt.kernel_data_selector) };
    unsafe { load_tss(gdt.tss_selector) };
    // Writing to this register is necessary for the syscall and sysretq instructions
    Star::write(
        gdt.user_code_selector,
        gdt.user_data_selector,
        gdt.kernel_code_selector,
        gdt.kernel_data_selector,
    )
    .unwrap();
}
