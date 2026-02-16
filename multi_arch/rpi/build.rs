fn main() {
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    println!("cargo:target-feature=+v6k")
}
