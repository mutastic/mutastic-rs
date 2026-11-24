//! Sets a `cfg` of its own, as many crates' build scripts do for what the compiler supports.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(from_build)");
    println!("cargo::rustc-cfg=from_build");
}
