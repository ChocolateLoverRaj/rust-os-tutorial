use std::env;

fn main() {
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    // On qemu this is 0x10000, but on a real Raspberry Pi Zero this is 0x8000
    // See https://forum.osdev.org/viewtopic.php?p=260378&hilit=raspi+0x8000#p260378,
    // https://forum.osdev.org/viewtopic.php?t=29998
    let is_qemu = cfg!(feature = "qemu");
    let is_aarch64 = env::var("CARGO_CFG_TARGET_ARCH").unwrap() == "aarch64";
    let loader_addr = if is_aarch64 {
        0x80000
    } else if is_qemu {
        0x10000
    } else {
        0x8000
    };
    let stack_align = if is_aarch64 { 16 } else { 8 };
    println!("cargo:rustc-link-arg=--defsym=LOADER_ADDR={loader_addr:#X}");
    println!("cargo:rustc-link-arg=--defsym=STACK_ALIGN={stack_align:#X}");
}
