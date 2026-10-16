fn main() {
    println!("cargo:rustc-cdylib-link-arg=-Wl,--kill-at");
}
