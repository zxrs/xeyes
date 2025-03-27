fn main() {
    #[cfg(debug_assertions)]
    println!("cargo:rustc-link-search=target/debug");

    #[cfg(not(debug_assertions))]
    println!("cargo:rustc-link-search=target/release");
}
