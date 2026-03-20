#![no_std]
#![no_main]
#![feature(stdarch_arm_hints)]
#![cfg_attr(target_arch = "arm", feature(stdarch_arm_neon_intrinsics))]

// mod logger;

use core::{arch::naked_asm, panic::PanicInfo};

unsafe extern "C" {
    static __interrupt_handler_stack_top: usize;
}

#[panic_handler]
pub fn panic_handler(panic_info: &PanicInfo) -> ! {
    loop {}
}

#[unsafe(link_section = ".text._header")]
// Prevent this function from being removed
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _start() {
    naked_asm!(
        "
        // Get the actual start address (which is also the load offset)
        adr x1, _start
        b {start}
        ",
        start = sym start
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn start() {
    naked_asm!(
        "
        // Do relocations
        // Get the actual start and end addresses
        ldr x2, =__rel_start
        add x2, x2, x1
        ldr x3, =__rel_end
        add x3, x3, x1
        // Loop through all relocations
        .reloc_loop:
            // Exit if we're done
            cmp x2, x3
            beq .reloc_loop_done

            // Load the relocation type to make sure it is R_AARCH64_RELATIV
            ldr x4, [x2, #8]
            // The type is the lower 32 bits
            cmp w4, #0x403
            bne .unknown_reloc

            // Read `r_addend`
            ldr x4, [x2, #16]
            // Add the offset
            add x4, x4, x1

            // Read the location to be patched
            ldr x5, [x2]
            // Add the offset
            add x5, x5, x1

            // Patch the location
            str x4, [x5]

            // Go to the next relocation
            add x2, x2, #24
            b .reloc_loop

        .unknown_reloc:
            b .unknown_reloc

        .reloc_loop_done:

        // Set stack pointer
        ldr x2, =__stack_top
        add x2, x2, x1
        mov sp, x2

        // Clear bss
        ldr x2, =__bss_start
        add x2, x2, x1
        ldr x3, =__bss_end
        add x3, x3, x1

        .zero_bss_loop:
            // Exit the loop if we're done
            cmp x2, x3
            beq .zero_bss_loop_done

            // Write *x2 = 0_u64; x2 += 8;
            str     xzr, [x2], #8

            b .zero_bss_loop

        .zero_bss_loop_done:
            bl      {kernel_main}
        ",
        kernel_main = sym kernel_main
    )
}

unsafe extern "C" fn kernel_main(fdt_addr: usize) -> ! {
    #[cfg(feature = "semihosting")]
    semihosting::println!("Hello from kernel (written in Rust) on aarch64");

    // FIXME: This is just for testing and is hard-coded for a Raspberry Pi 3B. Use the device tree!
    // Base address for the Auxiliary peripherals (BCM2837 Physical Address)
    const AUX_BASE: usize = 0x3F215000;
    // const AUX_ENABLES: *mut u32 = (AUX_BASE + 0x04) as *mut u32;
    const AUX_MU_IO: *mut u32 = (AUX_BASE + 0x40) as *mut u32;
    const AUX_MU_LSR: *mut u32 = (AUX_BASE + 0x54) as *mut u32;

    print_uart("Hello from Rust Bare Metal!\r\n");

    fn print_uart(s: &str) {
        for c in s.chars() {
            // Wait until Transmitter is empty (Bit 5 of LSR)
            unsafe {
                while (core::ptr::read_volatile(AUX_MU_LSR) & 0x20) == 0 {}
                core::ptr::write_volatile(AUX_MU_IO, c as u32);
            }
        }
    }

    loop {}
}
