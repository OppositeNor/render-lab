//! Lets the binary find the libraries it links at run time: the lab
//! (`liblab.so`, beside it in the target directory) and the standard library
//! the lab and the packages' libraries share (in the toolchain); and puts the
//! themes it offers (`themes/*.toml`) in `themes/` beside it, where the lab
//! reads them at run time (lab/src/theme.rs).

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let sysroot = Command::new(&rustc).args(["--print", "sysroot"]).output().expect("run rustc --print sysroot");
    let sysroot = PathBuf::from(String::from_utf8(sysroot.stdout).expect("sysroot is UTF-8").trim());
    let target = env::var("TARGET").unwrap();
    let std_dir = sysroot.join("lib/rustlib").join(&target).join("lib");
    println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN:{}", std_dir.display());
    install_themes();
}

/// Copies `themes/*.toml` to `themes/` in the binary's directory
/// (`target/<profile>/`, three folders above `OUT_DIR`).
fn install_themes() {
    let source = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("themes");
    println!("cargo:rerun-if-changed={}", source.display());
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let Some(bin_dir) = out.ancestors().nth(3) else { return };
    let target = bin_dir.join("themes");
    std::fs::create_dir_all(&target).unwrap_or_else(|e| panic!("create {}: {e}", target.display()));
    for entry in std::fs::read_dir(&source).unwrap_or_else(|e| panic!("read {}: {e}", source.display())) {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "toml") {
            let to = target.join(path.file_name().unwrap());
            std::fs::copy(&path, &to).unwrap_or_else(|e| panic!("copy {} to {}: {e}", path.display(), to.display()));
        }
    }
}
