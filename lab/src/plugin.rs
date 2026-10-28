//! The negotiation between the loader and a package's library, the way the
//! Vulkan loader negotiates with a layer. A package whose manifest names a
//! `library` and its `negotiate` symbol (src/packages.rs) is a cdylib that
//! exports that function:
//!
//! ```ignore
//! lab::negotiate!(o3de_lighting_negotiate, ["investigations" => investigations as InvestigationsFn]);
//! ```
//!
//! The loader opens the library, fills a `Negotiation` (its interface
//! version, its lab's build, and a map to fill) and calls the function. The
//! function checks them and registers the package's method handles in the map
//! by name, through the loader's `register`; the loader then asks the map for
//! a handle by name (`Handles`).
//!
//! The structure is C, and `negotiate!` expands in the library: everything
//! the function does before it registers a handle is compiled into the
//! library, against the lab it was built with (its `ABI`, a constant), and
//! touches only the structure. So a library built against another lab can
//! still read it and refuse, before it calls any of this lab's code. The
//! handles are Rust functions, valid only when the library links the very
//! lab the binary does, which that check ensures.

use crate::investigation::Investigation;
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr};

/// The highest interface version this loader speaks.
pub const INTERFACE_VERSION: u32 = 1;

/// The lab's build: its version, the compiler, and a hash of its sources
/// (build.rs). A library compiled against another build is refused.
pub const ABI: &str = concat!(env!("CARGO_PKG_VERSION"), "/", env!("LAB_RUSTC"), "/", env!("LAB_SOURCE_HASH"));

/// What a negotiation function returns.
pub const NEGOTIATED: i32 = 0;
pub const ERROR_STRUCT: i32 = 1;
pub const ERROR_VERSION: i32 = 2;
pub const ERROR_ABI: i32 = 3;
pub const ERROR_HANDLE: i32 = 4;

/// The loader's side of the negotiation.
#[repr(C)]
pub struct Negotiation {
    /// `size_of::<Negotiation>()` as the loader knows it.
    pub struct_size: usize,
    /// In: the loader's highest version. Out: the version the library speaks.
    pub interface_version: u32,
    /// The loader's lab build (`ABI`), UTF-8.
    pub abi: *const u8,
    pub abi_len: usize,
    /// The loader's handle map, passed back to `register`.
    pub map: *mut c_void,
    /// Registers one handle by name (a NUL-terminated string); returns
    /// `NEGOTIATED`, or `ERROR_HANDLE` for a name registered twice.
    pub register: unsafe extern "C" fn(map: *mut c_void, name: *const c_char, handle: *const c_void) -> i32,
}

/// A negotiation function, as the manifest's `negotiate` names it.
pub type NegotiateFn = unsafe extern "C" fn(negotiation: *mut Negotiation) -> i32;

/// The handle `investigations`: the package's investigations, as the homepage lists them.
pub type InvestigationsFn = fn() -> &'static [&'static dyn Investigation];

/// Defines a package's negotiation function, `$symbol` (its manifest's
/// `negotiate`), which hands the loader `handles`: each a name and a function
/// of that handle's type (`"investigations" => f as InvestigationsFn`).
#[macro_export]
macro_rules! negotiate {
    ($symbol:ident, [$($name:literal => $handle:expr),* $(,)?]) => {
        /// The package's negotiation function (the lab's src/plugin.rs).
        ///
        /// # Safety
        /// `negotiation` is the loader's, valid for the call.
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $symbol(negotiation: *mut $crate::plugin::Negotiation) -> i32 {
            use $crate::plugin::{ABI, ERROR_ABI, ERROR_STRUCT, ERROR_VERSION, INTERFACE_VERSION, NEGOTIATED};
            // Compiled here, against this library's lab: the structure only.
            // SAFETY: the caller's; the size is checked before any other field is read.
            let Some(n) = (unsafe { negotiation.as_mut() }) else { return ERROR_STRUCT };
            if n.struct_size != ::core::mem::size_of::<$crate::plugin::Negotiation>() {
                return ERROR_STRUCT;
            }
            if n.interface_version < INTERFACE_VERSION {
                return ERROR_VERSION;
            }
            n.interface_version = INTERFACE_VERSION;
            // SAFETY: the loader passes its build, UTF-8 of `abi_len` bytes.
            if unsafe { ::core::slice::from_raw_parts(n.abi, n.abi_len) } != ABI.as_bytes() {
                return ERROR_ABI;
            }
            $(
                // SAFETY: the loader's function, with its own map, and a NUL-terminated name.
                let code = unsafe { (n.register)(n.map, concat!($name, "\0").as_ptr().cast(), ($handle) as *const ::core::ffi::c_void) };
                if code != NEGOTIATED {
                    return code;
                }
            )*
            NEGOTIATED
        }
    };
}

/// A method handle a library registered.
#[derive(Clone, Copy)]
struct Handle(*const c_void);

// Handles are function pointers into libraries that are never unloaded.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

/// The handles one library registered, by name.
#[derive(Default)]
pub struct Handles(BTreeMap<String, Handle>);

impl Handles {
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    /// The `investigations` handle, if the library registered it.
    pub fn investigations(&self) -> Option<InvestigationsFn> {
        // SAFETY: `negotiate!` registers this name with a function of this type.
        self.0.get("investigations").map(|h| unsafe { std::mem::transmute::<*const c_void, InvestigationsFn>(h.0) })
    }
}

unsafe extern "C" fn register(map: *mut c_void, name: *const c_char, handle: *const c_void) -> i32 {
    // SAFETY: `map` is the `Handles` `negotiate` passed, and `name` a C string.
    let (map, name) = unsafe { (&mut *(map as *mut Handles), CStr::from_ptr(name)) };
    let Ok(name) = name.to_str() else { return ERROR_HANDLE };
    if map.0.contains_key(name) {
        return ERROR_HANDLE;
    }
    map.0.insert(name.to_string(), Handle(handle));
    NEGOTIATED
}

/// The loader's side: calls a library's negotiation function and returns the
/// handles it registered, or why it refused.
///
/// # Safety
/// `negotiate` is a library's function of the type `NegotiateFn`.
pub unsafe fn negotiate(negotiate: NegotiateFn) -> Result<Handles, String> {
    let mut handles = Handles::default();
    let mut negotiation = Negotiation {
        struct_size: size_of::<Negotiation>(),
        interface_version: INTERFACE_VERSION,
        abi: ABI.as_ptr(),
        abi_len: ABI.len(),
        map: &mut handles as *mut Handles as *mut c_void,
        register,
    };
    // SAFETY: the caller's.
    match unsafe { negotiate(&mut negotiation) } {
        NEGOTIATED if negotiation.interface_version == 0 || negotiation.interface_version > INTERFACE_VERSION => Err(format!("it answered interface version {}, and this lab speaks 1 to {INTERFACE_VERSION}", negotiation.interface_version)),
        NEGOTIATED => Ok(handles),
        ERROR_STRUCT => Err("it does not know this lab's negotiation structure; rebuild it".into()),
        ERROR_VERSION => Err(format!("it speaks no interface version up to {INTERFACE_VERSION}; rebuild it")),
        ERROR_ABI => Err(format!("it was built against another build of the lab (this one is {ABI}); rebuild it")),
        ERROR_HANDLE => Err("it registered a handle twice".into()),
        code => Err(format!("it refused (code {code})")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> &'static [&'static dyn Investigation] {
        &[]
    }

    crate::negotiate!(good, ["investigations" => none as InvestigationsFn]);
    crate::negotiate!(twice, ["investigations" => none as InvestigationsFn, "investigations" => none as InvestigationsFn]);

    /// A library of another build: it checks against a fingerprint of its own.
    unsafe extern "C" fn other_build(n: *mut Negotiation) -> i32 {
        let n = unsafe { &mut *n };
        if unsafe { std::slice::from_raw_parts(n.abi, n.abi_len) } != b"0.1.0/another build" { ERROR_ABI } else { NEGOTIATED }
    }

    #[test]
    fn negotiates() {
        let handles = unsafe { negotiate(good) }.unwrap();
        assert_eq!(handles.names().collect::<Vec<_>>(), ["investigations"]);
        assert!(handles.investigations().unwrap()().is_empty());
        assert!(unsafe { negotiate(twice) }.err().unwrap().contains("twice"));
        assert!(unsafe { negotiate(other_build) }.err().unwrap().contains("another build"));
    }
}
