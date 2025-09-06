use num_enum::IntoPrimitive;
use x86_64::{
    instructions::tables::load_tss,
    registers::segmentation::{CS, SS, Segment},
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
};

use crate::{
    EXCEPTION_HANDLER_STACK_SIZE, GuardedStack, StackId, StackType, cpu_local_data::get_local,
};

pub struct Gdt {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

#[derive(Debug, IntoPrimitive)]
#[repr(u8)]
pub enum IstStackIndexes {
    Exception,
}

pub fn init() {
    let local = get_local();
    let tss = local.tss.call_once(|| {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[u8::from(IstStackIndexes::Exception) as usize] =
            GuardedStack::new(
                EXCEPTION_HANDLER_STACK_SIZE,
                StackId {
                    _type: StackType::ExceptionHandler,
                    cpu_id: local.kernel_assigned_id,
                },
            )
            .top();
        tss
    });
    let gdt = local.gdt.call_once(|| {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code_selector = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data_selector = gdt.append(Descriptor::kernel_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(tss));
        Gdt {
            gdt,
            kernel_code_selector,
            kernel_data_selector,
            tss_selector,
        }
    });
    gdt.gdt.load();
    unsafe { CS::set_reg(gdt.kernel_code_selector) };
    unsafe { SS::set_reg(gdt.kernel_data_selector) };
    unsafe { load_tss(gdt.tss_selector) };
}
