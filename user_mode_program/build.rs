fn main() {
    // Tell cargo to specify in the output ELF what the entry function is
    let entry_function = "entry_point";
    println!("cargo:rustc-link-arg=-e{entry_function}");
}
