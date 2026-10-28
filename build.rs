//! Lets the binary find the libraries it links at run time: the lab
//! (`liblab.so`, beside it in the target directory) and the standard library
//! the lab and the packages' libraries share (in the toolchain).

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
}
