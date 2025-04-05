fn main() {
    println!(
        "cargo:rustc-env=TARGET_PLATFORM={}",
        std::env::var("TARGET").expect("TARGET env var not set")
    );
}
