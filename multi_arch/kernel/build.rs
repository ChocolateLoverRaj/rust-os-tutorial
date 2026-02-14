fn main() {
    println!("cargo:rustc-link-arg=-Tlinkers/riscv_qemu.ld");
    #[cfg(feature = "defmt")]
    {
        println!("cargo:rustc-link-arg=-Tdefmt.x");
        println!("cargo::rustc-env=DEFMT_LOG=trace");
    }
}
