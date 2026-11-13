//! A test library for the C ABI between packages (the lab's
//! src/packages/library_tests.rs): it imports the functions of
//! lab/abi-test/exporter. Each negotiation function imports another set:
//! all that exist with their signatures (`abi_importer_negotiate`), one the
//! exporter does not have (`…_missing`), and one of another signature
//! (`…_other`). `cargo build --release` builds it.

use lab::investigation::Investigation;
use lab::plugin::InvestigationsFn;
use lab::plugin::abi::{self, Import, RL_OK, RlBuffer, RlStatus};
use std::sync::atomic::{AtomicI32, Ordering};

lab::c_struct! {
    /// The exporter's `Pair`, copied.
    pub struct Pair {
        pub a: f32,
        pub b: f32,
    }
}

type SumFn = unsafe extern "C" fn(*const Pair, *mut f32, *mut RlBuffer) -> RlStatus;
type FailFn = unsafe extern "C" fn(i32, *mut RlBuffer) -> RlStatus;
/// The sum with a pointer to a float where the exporter has a `Pair`.
type OtherSumFn = unsafe extern "C" fn(*const f32, *mut f32, *mut RlBuffer) -> RlStatus;

static SUM: Import<SumFn> = Import::new("abi_exporter", "sum_v1");
static FAIL: Import<FailFn> = Import::new("abi_exporter", "fail_v1");
static MISSING: Import<FailFn> = Import::new("abi_exporter", "missing_v1");
static OTHER_SUM: Import<OtherSumFn> = Import::new("abi_exporter", "sum_v1");

/// The sum `investigations` computed through the import: -1 before it ran.
static LISTED: AtomicI32 = AtomicI32::new(-1);

/// The package's investigations: none, after it adds 1 and 2 through the import.
fn investigations() -> &'static [&'static dyn Investigation] {
    let mut sum = 0.0;
    let pair = Pair { a: 1.0, b: 2.0 };
    // SAFETY: the exporter's function, resolved by the negotiation.
    let status = unsafe { SUM.get()(&pair, &mut sum, std::ptr::null_mut()) };
    LISTED.store(
        if status == RL_OK {
            sum as i32
        } else {
            -2
        },
        Ordering::SeqCst,
    );
    &[]
}

lab::export! {
    /// Writes what `investigations` computed through the import (-1: it did not run).
    pub fn abi_importer_listed_v1(out: *mut i32) -> Result<(), String> {
        // SAFETY: the caller's integer.
        unsafe { *out = LISTED.load(Ordering::SeqCst) };
        Ok(())
    }

    /// Calls the exporter's `fail_v1` with `code`: its message.
    pub fn abi_importer_fail_v1(code: i32) -> Result<(), String> {
        // SAFETY: the exporter's function, resolved by the negotiation.
        abi::call(|message| unsafe { FAIL.get()(code, message) })
    }
}

lab::negotiate!(abi_importer_negotiate, ["investigations" => investigations as InvestigationsFn], imports [SUM, FAIL]);
lab::negotiate!(abi_importer_negotiate_missing, ["investigations" => investigations as InvestigationsFn], imports [SUM, MISSING]);
lab::negotiate!(abi_importer_negotiate_other, ["investigations" => investigations as InvestigationsFn], imports [OTHER_SUM]);
