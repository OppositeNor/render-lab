//! The C ABI between packages through the real loader: the test libraries
//! lab/abi-test/exporter and importer (which `cargo build --release` builds
//! beside the binary) opened by `load_all` as two packages' libraries.

use super::*;
use crate::plugin::abi::{RL_OK, RlBuffer, RlStatus};

/// A packages directory with the exporter and an importer that negotiates
/// with `negotiate`, each package's `lib/` linking to the example's library.
fn directory(test: &str, negotiate: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("render-lab-abi-{test}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (name, import, symbol) in [
        ("abi_exporter", "", "abi_exporter_negotiate"),
        ("abi_importer", "\"abi_exporter\"", negotiate),
    ] {
        let lib = dir.join(name).join("lib");
        fs::create_dir_all(&lib).unwrap();
        let file = libloading::library_filename(name);
        let built = bin_dir().join(&file);
        assert!(
            built.is_file(),
            "{} is not built (`cargo build --release` builds lab/abi-test)",
            built.display()
        );
        std::os::unix::fs::symlink(&built, lib.join(&file)).unwrap();
        let manifest = format!(
            "name = \"{name}\"\nauthor = \"a\"\nlicense = \"MIT\"\nimport = [{import}]\nexport = []\nlibrary = \"{name}\"\nnegotiate = \"{symbol}\"\n"
        );
        fs::write(dir.join(name).join("manifest.toml"), manifest).unwrap();
    }
    dir
}

/// The importer's result of loading both packages.
fn load_importer(test: &str, negotiate: &str) -> Result<Handles, String> {
    let dir = directory(test, negotiate);
    let packages = scan(&dir, &dir).unwrap();
    let mut loaded = load_all(&packages, load);
    assert_eq!(loaded[0].0.name, "abi_exporter");
    assert!(loaded[0].1.is_ok(), "{:?}", loaded[0].1.as_ref().err());
    loaded.pop().unwrap().1
}

#[test]
fn imports_resolve_before_any_handle_runs() {
    let handles = load_importer("good", "abi_importer_negotiate").unwrap();
    let resolved: Vec<String> = handles
        .resolved
        .iter()
        .map(|(p, s)| format!("{p}: {s}"))
        .collect();
    assert!(resolved.contains(&"abi_exporter: abi_exporter_sum_v1".to_string()));
    assert!(resolved.contains(&"abi_exporter: abi_exporter_fail_v1".to_string()));
    let library = handles.library.unwrap();
    type ListedFn = unsafe extern "C" fn(*mut i32, *mut RlBuffer) -> RlStatus;
    type FailFn = unsafe extern "C" fn(i32, *mut RlBuffer) -> RlStatus;
    // SAFETY: the importer's exports, of these types (lab/abi-test/importer).
    let (listed, fail) = unsafe {
        (
            *library.get::<ListedFn>(b"abi_importer_listed_v1").unwrap(),
            *library.get::<FailFn>(b"abi_importer_fail_v1").unwrap(),
        )
    };
    let mut sum = 0;
    // `investigations` has not run yet; once it has, it added through the resolved import.
    assert_eq!(unsafe { listed(&mut sum, std::ptr::null_mut()) }, RL_OK);
    assert_eq!(sum, -1);
    assert!(handles.investigations().unwrap()().is_empty());
    assert_eq!(unsafe { listed(&mut sum, std::ptr::null_mut()) }, RL_OK);
    assert_eq!(sum, 3);
    // An error and a panic in the exporter come back as messages, through the importer too.
    let error = crate::plugin::abi::call(|m| unsafe { fail(7, m) }).unwrap_err();
    assert_eq!(error, "refused 7");
    let panic = crate::plugin::abi::call(|m| unsafe { fail(0, m) }).unwrap_err();
    assert!(panic.contains("it panicked: asked to panic"), "{panic}");
}

#[test]
fn a_missing_symbol_refuses_the_importer() {
    let error = load_importer("missing", "abi_importer_negotiate_missing")
        .err()
        .unwrap();
    assert!(error.contains("could not resolve an import"), "{error}");
    assert!(
        error.contains("exports no `abi_exporter_missing_v1`"),
        "{error}"
    );
}

#[test]
fn another_signature_refuses_the_importer() {
    let error = load_importer("other", "abi_importer_negotiate_other")
        .err()
        .unwrap();
    assert!(
        error.contains("`abi_exporter_sum_v1` has another signature"),
        "{error}"
    );
}
