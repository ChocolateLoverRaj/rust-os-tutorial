fn main() {
    println!("cargo:rustc-link-arg=-Tlinkers/riscv_qemu.ld");
}
