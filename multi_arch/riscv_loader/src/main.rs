#![no_std]
#![no_main]

mod logger;
mod paging;

use core::{arch::naked_asm, mem::MaybeUninit};

use arbitrary_int::{u20, u22, u34};
use bitbybit::bitfield;
use loader::{Arch, BootInfo};
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

#[bitfield(u32, default = 0)]
pub struct Sv32PageTableEntry {
    #[bits(10..=31, rw)]
    physical_page_number: u22,
    #[bit(7)]
    d: bool,
    #[bit(6)]
    a: bool,
    #[bit(5)]
    g: bool,
    #[bit(4)]
    u: bool,
    #[bit(3)]
    x: bool,
    #[bit(2)]
    w: bool,
    #[bit(1)]
    r: bool,
    #[bit(0)]
    v: bool,
}

pub struct RiscvArch;
impl Arch for RiscvArch {
    type Paging = RiscvPaging;
    type PhysAddr = u34;

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

    type Page = [u8; 0x1000];

    type PhysPageNumber = u22;

    type VirtPageNumber = u20;

    const MAX_NEW_PAGES_NEEDED: usize = 1;

    fn new_page(bytes: &mut core::mem::MaybeUninit<Self::Page>) -> &mut Self::Page {
        bytes.write([0; _])
    }

    unsafe fn map_page(
        page_table: &mut Self::Page,
        virt_page: Self::VirtPageNumber,
        phys_page: Self::PhysPageNumber,
        flags: loader::MappingFlags,
        new_page_tables: &mut [&mut Self::Page],
    ) -> loader::MapPageResult {
        todo!("map page. virt_page = {virt_page:#X} phys_page = {phys_page:#X} flags = {flags:?}");
    }
}

extern "C" fn kernel_main(hart_id: usize, fdt_addr: usize) -> ! {
    let _ = hart_id;
    loader::start::<RiscvArch>(BootInfo {
        cpu_id: hart_id,
        fdt_addr,
    })
}
