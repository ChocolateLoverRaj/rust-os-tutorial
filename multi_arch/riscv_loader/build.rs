use std::env;

fn main() {
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    let pointer_width = env::var("CARGO_CFG_TARGET_POINTER_WIDTH")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    println!("cargo:rustc-link-arg=--defsym=POINTER_WIDTH={pointer_width:#X}");
}
