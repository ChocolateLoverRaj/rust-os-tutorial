fn main() {
    println!("cargo:rustc-link-arg=-Tlinkers/riscv_qemu.ld");
    println!("cargo:rustc-link-arg=-Tdefmt.x");
    println!("cargo::rustc-env=DEFMT_LOG=trace");
}
