fn main() {
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    // On qemu this is 0x10000, but on a real Raspberry Pi Zero this is 0x8000
    // See https://forum.osdev.org/viewtopic.php?p=260378&hilit=raspi+0x8000#p260378,
    // https://forum.osdev.org/viewtopic.php?t=29998
    println!("cargo:rustc-link-arg=--defsym=LOADER_ADDR=0x10000");
}
