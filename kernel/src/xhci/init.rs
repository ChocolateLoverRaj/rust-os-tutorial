use core::{num::NonZero, ptr::NonNull};

use volatile::VolatilePtr;
use x86_64::{PhysAddr, VirtAddr, registers::model_specific::PatMemoryType};

use crate::{
    ConfigurableFlags, Frame, max_page_size,
    memory::MEMORY,
    pci::{BarWithSize, PciFunction},
    xhci::regs::{OperationalRegs, OperationalRegsVolatileFieldAccess},
};

use super::{CapabilityRegs, CapabilityRegsVolatileFieldAccess};

pub fn init(mut function: PciFunction) {
    let bar = function
        .read_bar_with_size(0)
        .expect("Header type is known")
        .expect("Expected BAR 0");
    let bar = match bar {
        BarWithSize::Memory(bar) => bar,
        BarWithSize::Io(_) => panic!("Expected Memory BAR for xHCI"),
    };
    let memory = MEMORY.get().unwrap();
    let mut virt_mem = memory.virtual_memory.lock();
    let mut phys_mem = memory.physical_memory.lock();
    let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
    let page_size = max_page_size();
    let addr_and_size = bar.addr_and_size.addr_and_size_u64();
    let pages_len = (addr_and_size.addr + addr_and_size.size).div_ceil(page_size.byte_len_u64())
        - addr_and_size.addr / page_size.byte_len_u64();
    let mut pages = virt_mem
        .allocate_contiguous_pages_2(page_size, NonZero::new(pages_len).unwrap())
        .unwrap();
    let first_frame = Frame::new(
        PhysAddr::new(addr_and_size.addr).align_down(page_size.byte_len_u64()),
        page_size,
    )
    .unwrap();
    for i in 0..pages_len {
        let page = pages.start_page().offset(i).unwrap();
        let frame = first_frame.offset(i).unwrap();
        let flags = ConfigurableFlags {
            writable: true,
            executable: false,
            pat_memory_type: if bar.prefetchable {
                PatMemoryType::WriteThrough
            } else {
                PatMemoryType::StrongUncacheable
            },
        };
        unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
    }
    let mmio = pages.start_addr() + (addr_and_size.addr % page_size.byte_len_u64());
    parse_capability_registers(mmio);
}

fn parse_capability_registers(mmio: VirtAddr) {
    let ptr = NonNull::new(mmio.as_mut_ptr::<CapabilityRegs>()).unwrap();
    let cap_regs = unsafe { VolatilePtr::new(ptr) };
    let cap_regs_len = cap_regs.cap_length().read();

    let ops_regs_ptr =
        NonNull::new((mmio + cap_regs_len as u64).as_mut_ptr::<OperationalRegs>()).unwrap();
    let op_regs = unsafe { VolatilePtr::new(ops_regs_ptr) };

    // let max_device_slots = cap_regs.hcs_params_1().read().max_slots();
    // let max_interrupters = cap_regs.hcs_params_1().read().max_interrupters();
    // let max_ports = cap_regs.hcs_params_1().read().max_ports();

    // let isochronous_scheduling_threshold = cap_regs
    //     .hcs_params_2()
    //     .read()
    //     .isochronous_scheduling_threshold();
    // let erst_max = cap_regs.hcs_params_2().read().erst_max();
    // let max_scratchpad_buffers = cap_regs.hcs_params_2().read().max_scratchpad_buffers();

    log::debug!("xHCI Capability Registers: {:#X?}", cap_regs.read());
    log::debug!("xHCI Operational Registers: {:#X?}", op_regs.read());

    // Reset the host controller
    log::debug!("Stopping host controller");
    op_regs.usb_cmd().update(|mut cmd| {
        cmd.set_run(false);
        cmd
    });
    // TODO: Timeout after 200ms
    while !op_regs.usb_sts().read().hch() {}

    log::debug!("Resetting host controller");
    op_regs.usb_cmd().update(|mut cmd| {
        cmd.set_host_controller_reset(true);
        cmd
    });
    // TODO: Timeout after 1000ms
    while op_regs.usb_cmd().read().host_controller_reset() || op_regs.usb_sts().read().cnr() {}

    // TODO: On real hardware, wait for 50ms - https://youtu.be/9rI_fYvng6Q?list=PLATP7rOKo3E82tBnMp90B4zejpWeAKlxn&t=359
    if op_regs.usb_cmd().read().0 != 0 {
        panic!()
    }
    if op_regs.dn_ctrl().read().0 != 0 {
        panic!()
    }
    if op_regs.crcr().read().0 != 0 {
        panic!()
    }
    if op_regs.dcbaap().read().0 != 0 {
        panic!()
    }
    if op_regs.config().read().0 != 0 {
        panic!()
    }

    log::debug!(
        "xHCI Operational Registers after resetting: {:#X?}",
        op_regs.read()
    );
}
