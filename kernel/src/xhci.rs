use core::num::NonZero;

use common::PageSize;
use ez_pci::{
    ApicMsiMessageAddress, ApicMsiMessageData, BarWithSize, MsiXTableEntryVolatileFieldAccess,
    PciFunction,
};
use spin::Once;
use x86_64::{PhysAddr, registers::model_specific::PatMemoryType};
use xhci_driver::{AllocRequest, AllocResponse, XhciMemAllocator, XhciMmio};

use crate::{
    ConfigurableFlags, Frame,
    interrupt_vector::InterruptVector,
    max_page_size,
    memory::{MEMORY, MemoryType},
};

pub static XHCI_DRIVER: Once<spin::Mutex<xhci_driver::Driver>> = Once::new();

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
    if let Some(mut msi_x) = function.msi_x().expect("header type is known") {
        log::debug!("xHCI supports MSI-X. Configuring and enabling MSI-X for xHCI.");
        let mut table = unsafe { msi_x.table(bar0) };
        let entry = table.entry_mut(0);
        entry.message_address().write({
            let mut address = ApicMsiMessageAddress::default();
            address.set_destination_id(0);
            address.0 as u64
        });
        entry.message_data().write({
            let mut data = ApicMsiMessageData(0);
            data.set_vector(InterruptVector::Pci.into());
            data.0 as u32
        });
        entry.vector_control().update(|mut vector_control| {
            vector_control.set_mask(false);
            vector_control
        });
        {
            let mut message_control = msi_x.message_control();
            message_control.set_enable(true);
            msi_x.set_message_control(message_control);
        }
    } else {
        log::info!(
            "xHCI does not support MSI-X. Legacy interrupt info: {:#?}.",
            function.interrupt_info()
        );
    }
    // let mut xhci_driver = unsafe { xhci_driver::Driver::new(bar0) };
    // xhci_driver.reset_host_controller();
    // let res = xhci_driver
    //     .init_req()
    //     .into_iter()
    //     .map(allocate_multi)
    //     .collect::<Box<_>>();
    // // Safety: pages are properly allocated
    // unsafe { xhci_driver.init(&res) };
    // xhci_driver.start_device();
    let xhci_mmio = unsafe { XhciMmio::new(bar0) };
    let mut allocator = Allocator;
    let driver = xhci_driver::Driver::new(xhci_mmio, &mut allocator);
    XHCI_DRIVER.call_once(|| spin::Mutex::new(driver));
    log::info!("Started driver");
}

pub struct Allocator;

unsafe impl XhciMemAllocator for Allocator {
    fn alloc(&mut self, request: AllocRequest) -> AllocResponse {
        let memory = MEMORY.get().unwrap();
        let mut phys_mem = memory.physical_memory.lock();
        let mut virt_mem = memory.virtual_memory.lock();
        let phys_addr = phys_mem
            .allocate(
                request.size,
                request.align,
                request.boundary,
                MemoryType::UsedByXhci,
            )
            .unwrap();
        let page_size = PageSize::_4KiB;
        let n_pages = (phys_addr.as_u64() + request.size.get()).div_ceil(page_size.byte_len_u64())
            - phys_addr.as_u64() / page_size.byte_len_u64();
        let mut pages = virt_mem
            .allocate_contiguous_pages_2(page_size, NonZero::new(n_pages).unwrap())
            .unwrap();
        let first_frame =
            Frame::new(phys_addr.align_down(page_size.byte_len_u64()), page_size).unwrap();
        let mut frame_allocator = phys_mem.get_kernel_frame_allocator();
        for i in 0..n_pages {
            let page = pages.start_page().offset(i).unwrap();
            let frame = first_frame.offset(i).unwrap();
            let flags = ConfigurableFlags {
                writable: true,
                executable: false,
                pat_memory_type: PatMemoryType::StrongUncacheable,
            };
            unsafe { pages.map_to(page, frame, flags, &mut frame_allocator) }.unwrap();
        }
        AllocResponse {
            phys_addr: phys_addr.as_u64(),
            virt_addr: NonZero::new(
                (pages.start_addr().as_u64() + (phys_addr.as_u64() % page_size.byte_len_u64()))
                    as usize,
            )
            .expect("ptr is not null"),
        }
    }
}
