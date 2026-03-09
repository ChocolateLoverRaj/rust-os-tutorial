#![no_std]

mod align;
mod arch;
mod logger;
pub mod paging;
mod phys_mem_allocator;
pub use heapless;

use core::{mem::MaybeUninit, panic::PanicInfo, ptr::NonNull, slice};

pub use arch::{Arch, MapPageError, MappingFlags};
use elf::{ElfBytes, abi::PT_LOAD, endian::NativeEndian};
use fdt_raw::Fdt;
pub use logger::EarlyLogger;

use log::{error, info};

use crate::phys_mem_allocator::{AllocRequest, PhysMemAllocator};

pub struct BootInfo {
    pub cpu_id: usize,
    pub fdt_addr: usize,
}

#[panic_handler]
fn panic_handler(panic_info: &PanicInfo) -> ! {
    error!("{panic_info}");
    loop {}
}

pub fn start<A: Arch>(boot_info: BootInfo) -> ! {
    // let arch = ARCH.call_once(|| arch);
    logger::init(A::early_log);

    info!("Tutorial OS Stage 0");

    let fdt = {
        let ptr = boot_info.fdt_addr as *mut _;
        unsafe { Fdt::from_ptr(ptr) }
    }
    .unwrap();

    // let cpu_node = fdt.find_by_path("/cpus/cpu@0").unwrap();
    // let mmu_type = cpu_node.find_property("mmu-type").unwrap();
    // let extensions = cpu_node.find_property("riscv,isa-extensions").unwrap();
    // for extension in extensions.as_str_iter() {
    //     info!("RISC-V extension: {extension:?}");
    // }

    for node in fdt.all_nodes() {
        let node_path = node.path();
        info!("node: {node_path:?}");
    }

    for memory in fdt.memory() {
        for reg in memory.reg().unwrap() {
            info!("memory at: {reg:#X?}");
        }
    }

    for memory_reservation in fdt.memory_reservations() {
        info!("memory reservation: {memory_reservation:#X?}");
    }

    for node in fdt.reserved_memory() {
        for reg in node.reg().unwrap() {
            info!("reserved mem: {reg:#X?}");
        }
    }

    let chosen = fdt.chosen().unwrap();
    for property in chosen.properties() {
        let name = property.name();
        info!("/chosen property: {name:?}");
    }
    let initrd_start = chosen.find_property("linux,initrd-start").unwrap();
    let initrd_start: usize = if let Some(initrd_start) = initrd_start.as_u32() {
        initrd_start.try_into().unwrap()
    } else {
        initrd_start.as_u64().unwrap().try_into().unwrap()
    };
    let initrd_end = chosen.find_property("linux,initrd-end").unwrap();
    let initrd_end: usize = if let Some(initrd_end) = initrd_end.as_u32() {
        initrd_end.try_into().unwrap()
    } else {
        initrd_end.as_u64().unwrap().try_into().unwrap()
    };
    let initrd_len = initrd_end - initrd_start;
    info!("initrd-start: {initrd_start:#X}");
    info!("initrd len: {initrd_len:#X}");

    let mut allocator = PhysMemAllocator::<A::PhysAddr, _>::new(fdt);
    // Create a page table
    let alloc_request = AllocRequest {
        size: size_of::<A::Page>().try_into().unwrap(),
        align: size_of::<A::Page>().try_into().unwrap(),
    };
    let page_table_pointer: usize = (allocator.allocate(alloc_request).unwrap())
        .try_into()
        .unwrap();
    let mut page_table = NonNull::new(page_table_pointer as *mut MaybeUninit<A::Page>).unwrap();
    let page_table = A::new_page(unsafe { page_table.as_mut() });

    let elf = ElfBytes::<NativeEndian>::minimal_parse({
        let ptr = initrd_start as *const _;
        let len = initrd_len;
        unsafe { slice::from_raw_parts(ptr, len) }
    })
    .unwrap();
    let entry_address = elf.ehdr.e_entry;
    info!("entry address: {entry_address:#X}");
    let mut extra_pages = heapless::Vec::<_, 1>::new();
    for segment in elf.segments().unwrap() {
        if segment.p_type == PT_LOAD {
            info!("segment: {segment:#X?}");
            let start_phys_page_number =
                elf.segment_data(&segment).unwrap().as_ptr().addr() / size_of::<A::Page>();
            let n_pages = usize::try_from(segment.p_filesz).unwrap() / size_of::<A::Page>();
            for i in 0..n_pages {
                let phys_page_number = A::PhysPageNumber::try_from(
                    A::PhysAddr::try_from(start_phys_page_number + i).unwrap(),
                )
                .unwrap();
                while !extra_pages.is_full() {
                    let mut ptr = NonNull::new(
                        allocator
                            .allocate(alloc_request)
                            .unwrap()
                            .try_into()
                            .unwrap() as *mut MaybeUninit<A::Page>,
                    )
                    .unwrap();
                    extra_pages
                        .push(A::new_page(unsafe { ptr.as_mut() }))
                        .ok()
                        .unwrap();
                }
                // Safety: MMU is disabled
                unsafe {
                    A::map_page(
                        page_table,
                        (segment.p_vaddr as usize / size_of::<A::Page>())
                            .try_into()
                            .unwrap(),
                        phys_page_number,
                        MappingFlags::ReadWriteExecute,
                        extra_pages.as_mut_view(),
                    )
                }
                .unwrap();
            }
        }
    }

    info!("Mapped all segments. Top page table: {:#X?}", unsafe {
        A::debug_page_tables(page_table)
    });

    if A::can_shutdown() {
        A::shutdown()
    } else {
        A::low_power_loop()
    }
}
