//! A test library for the C ABI between packages (the lab's
//! src/packages/library_tests.rs): it exports C functions with `lab::export!`.
//! `cargo build --release` builds it.

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;

lab::c_struct! {
    /// Two numbers.
    pub struct Pair {
        pub a: f32,
        pub b: f32,
    }
}

lab::export! {
    /// Writes the sum of `pair` to `out`.
    pub fn abi_exporter_sum_v1(pair: *const Pair, out: *mut f32) -> Result<(), String> {
        // SAFETY: the caller's pair, or null.
        let pair = unsafe { pair.as_ref() }.ok_or("no pair")?;
        // SAFETY: the caller's float.
        unsafe { *out = pair.a + pair.b };
        Ok(())
    }

    /// Refuses `code`, or panics for 0.
    pub fn abi_exporter_fail_v1(code: i32) -> Result<(), String> {
        if code == 0 {
            panic!("asked to panic");
        }
        Err(format!("refused {code}"))
    }
}

fn investigations() -> &'static [&'static dyn Investigation] {
    &[]
}

lab::negotiate!(abi_exporter_negotiate, ["investigations" => investigations as InvestigationsFn]);
