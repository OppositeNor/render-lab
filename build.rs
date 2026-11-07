//! Lets the binary find the libraries it links at run time: the lab
//! (`liblab.so`, `liblab.dylib` on macOS, `lab.dll` on Windows, beside it in
//! the target directory) and the standard library the lab and the packages'
//! libraries share (in the toolchain; on Windows, copied beside it, with the
//! MinGW runtime for the GNU target); and puts the
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
    match env::var("CARGO_CFG_TARGET_OS").unwrap().as_str() {
        // dyld knows `@executable_path`, not `$ORIGIN`, and takes one path per rpath.
        "macos" => {
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,@executable_path");
            println!("cargo:rustc-link-arg-bins=-Wl,-rpath,{}", std_dir.display());
        }
        // Windows has no rpath: it finds a DLL beside the executable.
        "windows" => {
            install_std_dll(&std_dir);
            if env::var("CARGO_CFG_TARGET_ENV").unwrap() == "gnu" {
                install_mingw_runtime();
            }
        }
        _ => println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN:{}", std_dir.display()),
    }
    install_themes();
}

/// The MinGW runtime the GNU target's DLLs import (the standard library's
/// too): copied beside the binary from the linker's library directories, or
/// beside them in `bin/` (Arch, MSYS2).
fn install_mingw_runtime() {
    let Some(bin_dir) = bin_dir() else { return };
    let linker = env::var("RUSTC_LINKER").unwrap_or_else(|_| "x86_64-w64-mingw32-gcc".into());
    let dirs: Vec<PathBuf> = Command::new(&linker)
        .arg("-print-search-dirs")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|text| text.lines().find_map(|l| l.strip_prefix("libraries: =").map(str::to_owned)))
        .map(|list| env::split_paths(&list).flat_map(|d| [d.join("../bin"), d]).collect())
        .unwrap_or_default();
    for name in ["libgcc_s_seh-1.dll", "libwinpthread-1.dll"] {
        match dirs.iter().map(|d| d.join(name)).find(|p| p.is_file()) {
            Some(path) => {
                let to = bin_dir.join(name);
                std::fs::copy(&path, &to).unwrap_or_else(|e| panic!("copy {} to {}: {e}", path.display(), to.display()));
            }
            None => println!("cargo:warning={name} not found in {linker}'s library directories: put it beside the binary or on PATH"),
        }
    }
}

/// The binary's directory (`target/<profile>/`, three folders above `OUT_DIR`).
fn bin_dir() -> Option<PathBuf> {
    PathBuf::from(env::var("OUT_DIR").unwrap()).ancestors().nth(3).map(PathBuf::from)
}

/// Copies the standard library's DLL (`std-<hash>.dll`) from the toolchain
/// to the binary's directory.
fn install_std_dll(std_dir: &std::path::Path) {
    let Some(bin_dir) = bin_dir() else { return };
    for entry in std::fs::read_dir(std_dir).unwrap_or_else(|e| panic!("read {}: {e}", std_dir.display())) {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy();
        if name.starts_with("std-") && name.ends_with(".dll") {
            let to = bin_dir.join(path.file_name().unwrap());
            std::fs::copy(&path, &to).unwrap_or_else(|e| panic!("copy {} to {}: {e}", path.display(), to.display()));
        }
    }
}

/// Copies `themes/*.toml` to `themes/` in the binary's directory.
fn install_themes() {
    let source = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("themes");
    println!("cargo:rerun-if-changed={}", source.display());
    let Some(bin_dir) = bin_dir() else { return };
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
