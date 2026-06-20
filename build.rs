use std::env;
use std::path::PathBuf;

fn main() {
    let target = env::var("TARGET").unwrap_or_default();
    let _profile = env::var("PROFILE").unwrap_or_default();

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    println!("cargo:rustc-env=TARGET={}", target);

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    println!("cargo:rustc-link-search={}", out_dir.display());

    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    println!("cargo:rustc-env=BUILD_VERSION={}", version);

    println!("cargo:rustc-env=BUILD_TIME={}", env!("CARGO_PKG_VERSION"));
}
