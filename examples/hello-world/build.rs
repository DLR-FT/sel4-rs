use std::env;

fn main() {
    println!("cargo:rustc-link-arg=-Tarmv7a.ld");
    println!("cargo:rustc-link-arg=-Tsel4-overlay.ld");
}
