fn main() {
    // Use the linker script.
    println!("cargo:rustc-link-arg=-Tsrc/script.ld");
    println!("cargo:rerun-if-changed=src/script.ld");
    // Don't do any magic linker stuff.
    //println!("cargo:rustc-link-arg=--omagic");
}
