#![no_std]
#![no_main]

mod logger;
mod paging;

use core::{
    arch::naked_asm,
    fmt::Debug,
    mem::{MaybeUninit, transmute},
    ptr::NonNull,
};

use arbitrary_int::{u20, u22, u34};
use bitbybit::bitfield;
use loader::{Arch, BootInfo, MapPageError, MappingFlags, heapless};
use log::info;
use riscv::asm::wfi;
use sbi::legacy::shutdown;

use crate::{logger::early_log, paging::RiscvPaging};

// These variables are defined in the linker script
unsafe extern "C" {
    static __bss_start: usize;
    static __bss_end: usize;
    static __stack_top: usize;
}

/// OpenSBI passes the HART ID in the `a0` register and a pointer to the device tree in the `a1`
/// register. Since we don't modify those registers, we can just jump to `kernel_main` and those
/// two inputs will be passed to it.
#[unsafe(link_section = ".text._header")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn _start() {
    naked_asm!(
        "
            j {start}
        ",
        start = sym start
    )
}

#[cfg(target_pointer_width = "32")]
#[unsafe(naked)]
extern "C" fn start() {
    naked_asm!(
        "
        lla t0, _start

        // Do relocations
        lla t1, __rel_start
        lla t2, __rel_end
        .reloc_loop:
            beq t1, t2, .reloc_loop_done

            // Load the relocation type
            // It should be R_RISCV_RELATIVE
            // The lower 8 bytes store it
            lbu t3, 4(t1)
            li t4, 3
            bne t3, t4, .unknown_reloc

            // Load the default offset
            lw t4, 8(t1)
            // Add the load offset
            add t4, t4, t0

            // Get a pointer to the location in memory we need to modify
            lw t5, (t1)
            // Adjust the pointer itself for the offset
            add t5, t5, t0

            // Write to it
            sw t4, (t5)

            // Continue to the next relocation
            add t1, t1, 12
            j .reloc_loop

        .unknown_reloc:
            j .unknown_reloc

        .reloc_loop_done:

        // Set the stack pointer
        lla sp, __stack_top

        // Zero the BSS
        lla t1, __bss_start
        lla t2, __bss_end
        .zero_bss_loop:
            beq t1, t2, .zero_bss_loop_done
            sw zero, (t1)
            add t1, t1, 4
            j .zero_bss_loop

        .zero_bss_loop_done:

        j {kernel_main}
        ",
        kernel_main = sym kernel_main
    )
}

#[cfg(target_pointer_width = "64")]
#[unsafe(naked)]
extern "C" fn start() {
    naked_asm!(
        "
        lla t0, _start

        // Do relocations
        lla t1, __rel_start
        lla t2, __rel_end
        .reloc_loop:
            beq t1, t2, .reloc_loop_done

            // Load the relocation type
            // It should be R_RISCV_RELATIVE
            // The lower 32 bytes store it
            lwu t3, 8(t1)
            li t4, 3
            bne t3, t4, .unknown_reloc

            // Load the default offset
            ld t4, 16(t1)
            // Add the load offset
            add t4, t4, t0

            // Get a pointer to the location in memory we need to modify
            ld t5, (t1)
            // Adjust the pointer itself for the offset
            add t5, t5, t0

            // Write to it
            sd t4, (t5)

            // Continue to the next relocation
            add t1, t1, 24
            j .reloc_loop

        .unknown_reloc:
            j .unknown_reloc

        .reloc_loop_done:

        // Set the stack pointer
        lla sp, __stack_top

        // Zero the BSS
        lla t1, __bss_start
        lla t2, __bss_end
        .zero_bss_loop:
            beq t1, t2, .zero_bss_loop_done
            sd zero, (t1)
            add t1, t1, 8
            j .zero_bss_loop

        .zero_bss_loop_done:

        j {kernel_main}
        ",
        kernel_main = sym kernel_main
    )
}

#[derive(Debug)]
pub struct Sv32PageTable {
    entries: [Sv32PageTableEntry; 0x400],
}

impl Default for Sv32PageTable {
    fn default() -> Self {
        Self {
            entries: [Default::default(); _],
        }
    }
}

#[bitfield(u32, default = 0, debug)]
pub struct Sv32PageTableEntry {
    #[bits(10..=31, rw)]
    physical_page_number: u22,
    #[bit(7, rw)]
    d: bool,
    #[bit(6, rw)]
    a: bool,
    #[bit(5, rw)]
    g: bool,
    #[bit(4, rw)]
    u: bool,
    #[bit(3, rw)]
    x: bool,
    #[bit(2, rw)]
    w: bool,
    #[bit(1, rw)]
    r: bool,
    #[bit(0, rw)]
    v: bool,
}

const PAGE_SIZE: usize = 0x1000;

pub struct RiscvArch;
impl Arch for RiscvArch {
    type Paging = RiscvPaging;
    type PhysAddr = u64;

    fn early_log(arguments: core::fmt::Arguments<'_>) {
        early_log(arguments);
    }

    fn can_shutdown() -> bool {
        true
    }

    fn shutdown() -> ! {
        shutdown()
    }

    fn low_power_loop() -> ! {
        loop {
            wfi();
        }
    }

    type Page = [u8; PAGE_SIZE];

    type PhysPageNumber = u32;

    type VirtPageNumber = u32;

    const MAX_NEW_PAGES_NEEDED: usize = 1;

    fn new_page(bytes: &mut core::mem::MaybeUninit<Self::Page>) -> &mut Self::Page {
        bytes.write([0; _])
    }

    unsafe fn map_page(
        page_table: &mut Self::Page,
        virt_page: Self::VirtPageNumber,
        phys_page: Self::PhysPageNumber,
        flags: loader::MappingFlags,
        new_page_tables: &mut heapless::VecView<&mut Self::Page>,
    ) -> Result<(), MapPageError> {
        log::info!(
            "mapping page. page table = {page_table:p} virt_page = {virt_page:#X} phys_page = {phys_page:#X} flags = {flags:?}"
        );
        let page_table = unsafe { transmute::<_, &mut Sv32PageTable>(page_table) };
        let vpn_1 = virt_page >> 10;
        let entry = &mut page_table.entries[vpn_1 as usize];
        if !entry.v() {
            if let Some(new_page_table) = new_page_tables.pop() {
                entry.set_physical_page_number(
                    (new_page_table.as_ptr().addr() / size_of::<Self::Page>())
                        .try_into()
                        .unwrap(),
                );
                entry.set_v(true);
                info!(
                    "created entry pointing new new page table at {new_page_table:p} vpn_1 = {vpn_1:#X}"
                );
            } else {
                return Err(MapPageError {
                    n_page_tables_needed: 1,
                });
            }
        }
        let mut page_table_ptr = NonNull::new(
            (usize::try_from(entry.physical_page_number()).unwrap() * size_of::<Self::Page>())
                as *mut Sv32PageTable,
        )
        .unwrap();
        info!("page table 2 ptr: {page_table_ptr:p}");
        let page_table = unsafe { page_table_ptr.as_mut() };
        let vpn_2 = virt_page & 0x3FF;
        let entry = &mut page_table.entries[vpn_2 as usize];
        if !entry.v() {
            let (x, w, r) = match flags {
                MappingFlags::Read => (false, false, true),
                MappingFlags::ReadWrite => (false, true, true),
                MappingFlags::ReadExecute => (true, false, true),
                MappingFlags::ReadWriteExecute => (true, true, true),
            };
            entry.set_x(x);
            entry.set_w(w);
            entry.set_r(r);
            entry.set_physical_page_number(u22::new(phys_page));
            entry.set_v(true);
            info!("created mapping. vpn_2 = {vpn_2:#X}");
        } else {
            panic!("page already mapped");
        }
        Ok(())
    }

    unsafe fn debug_page_tables(page_table: &mut Self::Page) -> impl core::fmt::Debug {
        // let page_table = unsafe { transmute::<_, *mut Sv32PageTable>(page_table) };
        DebugPageTables {
            ptr: NonNull::from_mut(page_table).cast(),
        }
    }
}

enum PageTableLevel {
    Level1,
    Level2,
}

struct DebugPageTables {
    ptr: NonNull<Sv32PageTable>,
    // level: PageTableLevel,
}
impl Debug for DebugPageTables {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut f = f.debug_map();
        let page_table = unsafe { self.ptr.as_ref() };
        for (i, entry) in page_table.entries.iter().enumerate() {
            if entry.v() {
                f.key(&i);
                enum Rwx {
                    PointerToPageTable,
                    Mapping(MappingFlags),
                }
                let rwx = match (entry.r(), entry.w(), entry.x()) {
                    (false, false, false) => Some(Rwx::PointerToPageTable),
                    (true, true, true) => Some(Rwx::Mapping(MappingFlags::ReadWriteExecute)),
                    _ => None,
                };
                match rwx {
                    Some(Rwx::PointerToPageTable) => {
                        f.value(&DebugPageTables {
                            ptr: NonNull::new(
                                (usize::try_from(entry.physical_page_number()).unwrap() * PAGE_SIZE)
                                    as *mut _,
                            )
                            .unwrap(),
                        });
                    }
                    Some(Rwx::Mapping(flags)) => {
                        f.value(&(entry.physical_page_number(), flags));
                    }
                    None => {
                        f.value(&"unknown");
                    }
                };
            }
        }
        f.finish()
    }
}

extern "C" fn kernel_main(hart_id: usize, fdt_addr: usize) -> ! {
    let _ = hart_id;
    loader::start::<RiscvArch>(BootInfo {
        cpu_id: hart_id,
        fdt_addr,
    })
}
