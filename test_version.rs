fn main() {
    #[cfg(feature = "lzma-rs")]
    println!("lzma-rs is enabled");
    
    // Проверим через cargo metadata
    println!("Run: cargo metadata --format-version=1 | grep lzma");
}
