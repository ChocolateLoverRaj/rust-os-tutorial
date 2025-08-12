use core::num::NonZero;

use common::PageSize;
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};
use xhci_driver::{
    AllocRequest, AllocResponse, MultiAllocRequest, MultiAllocResponse, ScratchpadPages,
    SetUpDcbaaInput, XhciPage,
};

use crate::{
    ConfigurableFlags, Frame, max_page_size,
    memory::{MEMORY, MemoryType},
    pci::{BarWithSize, PciFunction},
    translate_addr::TranslateToVirt,
};

pub fn init(mut function: PciFunction) {
    let bar0 = {
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
        let pages_len = (addr_and_size.addr + addr_and_size.size)
            .div_ceil(page_size.byte_len_u64())
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
        NonZero::new(
            (pages.start_addr().as_u64() + (addr_and_size.addr % page_size.byte_len_u64()))
                as usize,
        )
        .unwrap()
    };
    let mut xhci_driver = unsafe { xhci_driver::Driver::new(bar0) };
    log::debug!(
        "xHCI cap regs: {:#X?}",
        xhci_driver.debug_capability_registers()
    );
    log::debug!(
        "xHCI op regs: {:#X?}",
        xhci_driver.debug_operational_registers()
    );
    xhci_driver.reset_host_controller();
    log::debug!(
        "xHCI op regs: {:#X?}",
        xhci_driver.debug_operational_registers()
    );
    let res = xhci_driver
        .configure_operational_registers_req()
        .map(allocate_multi);
    // Safety: pages are properly allocated
    unsafe { xhci_driver.configure_operational_registers(res) };
    log::debug!("xHCI driver: {xhci_driver:X?}");
    log::debug!(
        "xHCI op regs: {:#X?}",
        xhci_driver.debug_operational_registers()
    );
}

pub fn allocate_xhci_page() -> XhciPage {
    let memory = MEMORY.get().unwrap();
    let mut virt_mem = memory.virtual_memory.lock();
    let mut phys_mem = memory.physical_memory.lock();
    let frame = phys_mem
        .allocate_frame_with_type(PageSize::_4KiB, MemoryType::UsedByXhci)
        .unwrap();
    let mut pages = virt_mem
        .allocate_contiguous_pages_2(PageSize::_4KiB, NonZero::new(1).expect("1 != 0"))
        .unwrap();
    let page = pages.start_page();
    let flags = ConfigurableFlags {
        writable: true,
        executable: false,
        pat_memory_type: PatMemoryType::StrongUncacheable,
    };
    let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
    unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
    XhciPage {
        phys_addr: frame.start_addr().as_u64(),
        virt_addr: NonZero::new(page.start_addr().as_u64() as usize).expect("ptr is not null"),
    }
}

pub fn allocate(request: &AllocRequest) -> AllocResponse {
    let memory = MEMORY.get().unwrap();
    let phys_addr = memory
        .physical_memory
        .lock()
        .allocate(
            request.size,
            request.align,
            request.boundary,
            MemoryType::UsedByXhci,
        )
        .unwrap();
    AllocResponse {
        phys_addr: phys_addr.as_u64(),
        virt_addr: NonZero::new(phys_addr.to_virt().as_u64() as usize).expect("ptr is not null"),
    }
}

pub fn allocate_multi(request: Option<MultiAllocRequest>) -> MultiAllocResponse {
    request.map_or_else(Default::default, |request| {
        (0..request.count.get())
            .map(|_| allocate(&request.request))
            .collect()
    })
}
