//! The C ABI between packages: functions one package's library exports and
//! another's imports, by name, through the handles the loader gives a
//! library of the packages its manifest imports (src/plugin.rs).
//!
//! Rust has no stable ABI, so only C crosses: `#[repr(C)]` data (integers,
//! `f32`, fixed arrays, and structs of them declared with `c_struct!`),
//! `RlStr`, `RlSlice` and `RlSliceMut`, and raw pointers. The caller owns
//! every buffer and the callee fills it; nothing allocated on one side is freed
//! on the other.
//!
//! An exporter writes its functions with `export!`:
//!
//! ```ignore
//! lab::export! {
//!     /// The cosine integral over a quad.
//!     pub fn godot_voxelgi_ltc_diffuse_v1(quad: *const LtcQuad, out: *mut f32) -> Result<(), String> {
//!         // ...
//!     }
//! }
//! ```
//!
//! Each is a symbol `<package>_<name>_v<major>` (a breaking change makes a
//! new major; adding a function adds a symbol), taking, last, the caller's
//! message buffer (`*mut RlBuffer`, may be null) and returning an `RlStatus`.
//! Its body runs inside `catch_unwind`, so no panic crosses. Beside it
//! `export!` writes `<symbol>_signature`, an `RlSignature`: a fingerprint of
//! the function's type (its parameters' and result's names, sizes, and
//! alignments, `CType`).
//!
//! An importer declares what it uses as statics of `Import` and names them in
//! its `negotiate!`, which resolves each from the handles before it answers:
//! a missing symbol or another signature makes the negotiation fail, so the
//! loader refuses the package at load, never at a first call.

use std::ffi::c_void;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicPtr, Ordering};

/// What every exported function returns.
pub type RlStatus = i32;
/// It did what it was asked.
pub const RL_OK: RlStatus = 0;
/// It refused, with a message in the caller's buffer.
pub const RL_ERROR: RlStatus = 1;
/// It panicked; the panic's message is in the caller's buffer.
pub const RL_PANIC: RlStatus = 2;

/// A UTF-8 string another side owns: a pointer and a length in bytes.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct RlStr {
    pub ptr: *const u8,
    pub len: usize,
}

// An `RlStr` only reads what it points to.
unsafe impl Send for RlStr {}
unsafe impl Sync for RlStr {}

impl RlStr {
    pub const fn new(text: &'static str) -> RlStr {
        RlStr {
            ptr: text.as_ptr(),
            len: text.len(),
        }
    }

    /// A string valid while `text` is: the caller keeps it alive across the call.
    pub fn borrowed(text: &str) -> RlStr {
        RlStr {
            ptr: text.as_ptr(),
            len: text.len(),
        }
    }

    /// The text, or `None` for invalid UTF-8.
    ///
    /// # Safety
    /// `ptr` points to `len` bytes, alive for `'a`.
    pub unsafe fn as_str<'a>(&self) -> Option<&'a str> {
        if self.len == 0 {
            return Some("");
        }
        // SAFETY: the caller's.
        std::str::from_utf8(unsafe { std::slice::from_raw_parts(self.ptr, self.len) }).ok()
    }
}

/// A slice another side owns, read-only.
#[repr(C)]
pub struct RlSlice<T> {
    pub ptr: *const T,
    pub len: usize,
}

impl<T> Clone for RlSlice<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for RlSlice<T> {}

impl<T> RlSlice<T> {
    pub fn new(items: &[T]) -> RlSlice<T> {
        RlSlice {
            ptr: items.as_ptr(),
            len: items.len(),
        }
    }

    /// # Safety
    /// `ptr` points to `len` items, alive for `'a`.
    pub unsafe fn as_slice<'a>(&self) -> &'a [T] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: the caller's.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

/// A slice another side owns, for the callee to fill.
#[repr(C)]
pub struct RlSliceMut<T> {
    pub ptr: *mut T,
    pub len: usize,
}

impl<T> RlSliceMut<T> {
    pub fn new(items: &mut [T]) -> RlSliceMut<T> {
        RlSliceMut {
            ptr: items.as_mut_ptr(),
            len: items.len(),
        }
    }

    /// # Safety
    /// `ptr` points to `len` items, alive for `'a` and not aliased.
    pub unsafe fn as_slice<'a>(&mut self) -> &'a mut [T] {
        if self.len == 0 {
            return &mut [];
        }
        // SAFETY: the caller's.
        unsafe { std::slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

/// A caller's buffer for a message: `capacity` bytes at `ptr`, of which the
/// callee writes `len` (UTF-8, cut at a character).
#[repr(C)]
pub struct RlBuffer {
    pub ptr: *mut u8,
    pub capacity: usize,
    pub len: usize,
}

impl RlBuffer {
    pub fn new(bytes: &mut [u8]) -> RlBuffer {
        RlBuffer {
            ptr: bytes.as_mut_ptr(),
            capacity: bytes.len(),
            len: 0,
        }
    }

    /// Writes as much of `message` as fits, cut at a character.
    pub fn write(&mut self, message: &str) {
        let mut len = message.len().min(self.capacity);
        while !message.is_char_boundary(len) {
            len -= 1;
        }
        // SAFETY: `ptr` holds `capacity` bytes (`new`, or the caller's promise).
        unsafe { std::ptr::copy_nonoverlapping(message.as_ptr(), self.ptr, len) };
        self.len = len;
    }

    /// What the callee wrote.
    pub fn text(&self) -> String {
        if self.len == 0 {
            return String::new();
        }
        // SAFETY: the callee wrote `len` bytes at `ptr`.
        let bytes = unsafe { std::slice::from_raw_parts(self.ptr, self.len) };
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Runs an exported function's body: its error or panic becomes a status,
/// and its message goes into the caller's buffer (if it gave one).
///
/// # Safety
/// `message` is null or a caller's buffer (`RlBuffer::new`).
pub unsafe fn guarded(
    message: *mut RlBuffer,
    body: impl FnOnce() -> Result<(), String>,
) -> RlStatus {
    let (status, text) = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => return RL_OK,
        Ok(Err(text)) => (RL_ERROR, text),
        Err(panic) => {
            // A panic's payload is the message `panic!` formatted, or the literal it was given.
            let text = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "a panic".into());
            (RL_PANIC, text)
        }
    };
    // SAFETY: the caller's.
    if let Some(buffer) = unsafe { message.as_mut() } {
        buffer.write(&text);
    }
    status
}

/// Calls an imported function with a message buffer of its own: its error
/// (or panic) as a `String`.
pub fn call(f: impl FnOnce(*mut RlBuffer) -> RlStatus) -> Result<(), String> {
    let mut bytes = [0u8; 512];
    let mut buffer = RlBuffer::new(&mut bytes);
    match f(&mut buffer) {
        RL_OK => Ok(()),
        RL_PANIC => Err(format!("it panicked: {}", buffer.text())),
        _ => Err(buffer.text()),
    }
}

/// The layout of a type that crosses: its size, alignment, and a
/// fingerprint of its name and its parts' layouts.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RlLayout {
    pub size: usize,
    pub align: usize,
    pub hash: u64,
}

/// FNV-1a over `bytes`, from `hash`.
pub const fn fnv(mut hash: u64, bytes: &[u8]) -> u64 {
    let mut i = 0;
    while i < bytes.len() {
        hash ^= bytes[i] as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
        i += 1;
    }
    hash
}

const FNV_START: u64 = 0xcbf2_9ce4_8422_2325;

/// `T`'s layout: its size and alignment, its `name`, and its `parts`' fingerprints, in order.
pub const fn layout<T>(name: &str, parts: &[u64]) -> RlLayout {
    let (size, align) = (size_of::<T>(), align_of::<T>());
    let mut hash = fnv(FNV_START, name.as_bytes());
    let mut i = 0;
    while i < parts.len() {
        hash = fnv(hash, &parts[i].to_le_bytes());
        i += 1;
    }
    hash = fnv(hash, &(size as u64).to_le_bytes());
    hash = fnv(hash, &(align as u64).to_le_bytes());
    RlLayout { size, align, hash }
}

/// A type that may cross: its C layout, the same on both sides.
///
/// # Safety
/// The type is `#[repr(C)]` (or a C scalar or pointer), and `LAYOUT` names its parts.
pub unsafe trait CType {
    const LAYOUT: RlLayout;
}

macro_rules! scalars {
    ($($t:ty),*) => {
        $(unsafe impl CType for $t {
            const LAYOUT: RlLayout = layout::<$t>(stringify!($t), &[]);
        })*
    };
}
scalars!(
    u8,
    i8,
    u16,
    i16,
    u32,
    i32,
    u64,
    i64,
    usize,
    isize,
    f32,
    f64,
    bool,
    (),
    c_void
);

unsafe impl<T: CType, const N: usize> CType for [T; N] {
    const LAYOUT: RlLayout = layout::<[T; N]>("array", &[T::LAYOUT.hash, N as u64]);
}
unsafe impl<T: CType> CType for *const T {
    const LAYOUT: RlLayout = layout::<*const T>("*const", &[T::LAYOUT.hash]);
}
unsafe impl<T: CType> CType for *mut T {
    const LAYOUT: RlLayout = layout::<*mut T>("*mut", &[T::LAYOUT.hash]);
}
unsafe impl<T: CType> CType for RlSlice<T> {
    const LAYOUT: RlLayout = layout::<RlSlice<T>>("RlSlice", &[T::LAYOUT.hash]);
}
unsafe impl<T: CType> CType for RlSliceMut<T> {
    const LAYOUT: RlLayout = layout::<RlSliceMut<T>>("RlSliceMut", &[T::LAYOUT.hash]);
}
unsafe impl CType for RlStr {
    const LAYOUT: RlLayout = layout::<RlStr>("RlStr", &[]);
}
unsafe impl CType for RlBuffer {
    const LAYOUT: RlLayout = layout::<RlBuffer>("RlBuffer", &[]);
}

/// Declares a `#[repr(C)]` struct that may cross (`CType`): its layout is
/// its name's, and its fields' names and layouts, in order.
#[macro_export]
macro_rules! c_struct {
    ($(#[$meta:meta])* $vis:vis struct $name:ident {
        $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),* $(,)?
    }) => {
        $(#[$meta])*
        #[repr(C)]
        $vis struct $name {
            $($(#[$field_meta])* $field_vis $field: $ty),*
        }

        unsafe impl $crate::plugin::abi::CType for $name {
            const LAYOUT: $crate::plugin::abi::RlLayout = $crate::plugin::abi::layout::<$name>(
                concat!(stringify!($name) $(, " ", stringify!($field))*),
                &[$(<$ty as $crate::plugin::abi::CType>::LAYOUT.hash),*],
            );
        }
    };
}

/// A function type that may cross: a fingerprint of its parameters' and result's layouts.
///
/// # Safety
/// The type is a C function pointer.
pub unsafe trait CFunction: Copy + Send + Sync + 'static {
    const SIGNATURE: u64;
}

/// The fingerprint of a function of `params` returning `result`.
pub const fn signature(result: RlLayout, params: &[RlLayout]) -> u64 {
    let mut hash = fnv(FNV_START, b"fn");
    let mut i = 0;
    while i < params.len() {
        hash = fnv(hash, &params[i].hash.to_le_bytes());
        i += 1;
    }
    fnv(hash, &result.hash.to_le_bytes())
}

macro_rules! functions {
    ($($a:ident),*) => {
        unsafe impl<R: CType + 'static, $($a: CType + 'static),*> CFunction for unsafe extern "C" fn($($a),*) -> R {
            const SIGNATURE: u64 = signature(R::LAYOUT, &[$($a::LAYOUT),*]);
        }
        unsafe impl<R: CType + 'static, $($a: CType + 'static),*> CFunction for extern "C" fn($($a),*) -> R {
            const SIGNATURE: u64 = signature(R::LAYOUT, &[$($a::LAYOUT),*]);
        }
    };
}
functions!();
functions!(A);
functions!(A, B);
functions!(A, B, C);
functions!(A, B, C, D);
functions!(A, B, C, D, E);
functions!(A, B, C, D, E, F);
functions!(A, B, C, D, E, F, G);
functions!(A, B, C, D, E, F, G, H);

/// The symbol `export!` writes beside each function: its type's fingerprint,
/// and the type as written, for messages.
#[repr(C)]
pub struct RlSignature {
    pub hash: u64,
    pub text: RlStr,
}

impl RlSignature {
    pub const fn of<F: CFunction>(text: &'static str) -> RlSignature {
        RlSignature {
            hash: F::SIGNATURE,
            text: RlStr::new(text),
        }
    }
}

/// Exports C functions from a package's library (the module's docs): each
/// `pub fn <package>_<name>_v<major>(params) -> Result<(), String> { body }`
/// becomes `unsafe extern "C" fn(params, message: *mut RlBuffer) -> RlStatus`,
/// its body guarded (`guarded`), with its signature beside it.
#[macro_export]
macro_rules! export {
    ($($(#[$meta:meta])* $vis:vis fn $name:ident ($($arg:ident : $ty:ty),* $(,)?) -> Result<(), String> $body:block)*) => {
        $(
            $(#[$meta])*
            ///
            /// # Safety
            /// Its pointers are valid as its interface describes, and `message` is null or a caller's buffer.
            #[unsafe(no_mangle)]
            $vis unsafe extern "C" fn $name(
                $($arg: $ty,)*
                message: *mut $crate::plugin::abi::RlBuffer,
            ) -> $crate::plugin::abi::RlStatus {
                // The body is built outside the `unsafe` block, so its own unsafe operations need their own.
                let body = move || -> Result<(), String> { $body };
                // SAFETY: the caller's buffer, or null.
                unsafe { $crate::plugin::abi::guarded(message, body) }
            }

            const _: () = {
                #[unsafe(export_name = concat!(stringify!($name), "_signature"))]
                static SIGNATURE: $crate::plugin::abi::RlSignature = $crate::plugin::abi::RlSignature::of::<
                    unsafe extern "C" fn($($ty,)* *mut $crate::plugin::abi::RlBuffer) -> $crate::plugin::abi::RlStatus,
                >(stringify!(fn($($ty,)* *mut RlBuffer) -> RlStatus));
            };
        )*
    };
}

/// Another package's library as the loader hands it to an importer: its
/// name, and the loader's lookup of a symbol in it (null if it has none).
#[repr(C)]
pub struct RlPackage {
    pub name: RlStr,
    pub library: *const c_void,
    pub symbol: unsafe extern "C" fn(library: *const c_void, name: RlStr) -> *const c_void,
}

impl RlPackage {
    pub fn name(&self) -> &str {
        // SAFETY: the loader's, alive for the negotiation.
        unsafe { self.name.as_str() }.unwrap_or("")
    }

    /// The address of `symbol` in the library, or null.
    pub fn symbol(&self, symbol: &str) -> *const c_void {
        // SAFETY: the loader's function and library, alive for the negotiation.
        unsafe { (self.symbol)(self.library, RlStr::borrowed(symbol)) }
    }
}

/// One function a package imports from another: `package`'s symbol
/// `<package>_<name>`, of the type `F`, resolved by `negotiate!`.
pub struct Import<F> {
    package: &'static str,
    name: &'static str,
    slot: AtomicPtr<c_void>,
    function: PhantomData<F>,
}

impl<F: CFunction> Import<F> {
    pub const fn new(package: &'static str, name: &'static str) -> Import<F> {
        assert!(size_of::<F>() == size_of::<*const c_void>());
        Import {
            package,
            name,
            slot: AtomicPtr::new(std::ptr::null_mut()),
            function: PhantomData,
        }
    }

    /// The function; `negotiate!` resolved it before the lab could call into the library.
    pub fn get(&self) -> F {
        let address = self.slot.load(Ordering::Acquire);
        assert!(
            !address.is_null(),
            "{} was not resolved (name it in the package's negotiate!)",
            self.symbol()
        );
        // SAFETY: `resolve` stored a function whose signature is `F`'s.
        unsafe { std::mem::transmute_copy::<*mut c_void, F>(&address) }
    }
}

/// What `negotiate!` does with each of a package's imports.
pub trait Resolve: Sync {
    /// The symbol it imports.
    fn symbol(&self) -> String;
    /// Finds it in the handles, checks its signature, and stores it.
    fn resolve(&self, packages: &[RlPackage]) -> Result<(), String>;
}

impl<F: CFunction> Resolve for Import<F> {
    fn symbol(&self) -> String {
        format!("{}_{}", self.package, self.name)
    }

    fn resolve(&self, packages: &[RlPackage]) -> Result<(), String> {
        let symbol = self.symbol();
        let package = packages.iter().find(|p| p.name() == self.package).ok_or_else(|| {
            format!(
                "it imports `{symbol}` from `{}`, which its manifest does not import, or which has no library",
                self.package
            )
        })?;
        let function = package.symbol(&symbol);
        if function.is_null() {
            return Err(format!("`{}` exports no `{symbol}`", self.package));
        }
        let signature = package.symbol(&format!("{symbol}_signature")) as *const RlSignature;
        // SAFETY: `export!` writes this symbol as an `RlSignature`.
        let Some(signature) = (unsafe { signature.as_ref() }) else {
            return Err(format!(
                "`{}`'s `{symbol}` has no signature (export it with lab::export!)",
                self.package
            ));
        };
        if signature.hash != F::SIGNATURE {
            // SAFETY: a static string of the exporter's.
            let exported = unsafe { signature.text.as_str() }.unwrap_or("?");
            return Err(format!(
                "`{}`'s `{symbol}` has another signature: it exports `{exported}`, this imports `{}`",
                self.package,
                std::any::type_name::<F>()
            ));
        }
        self.slot.store(function.cast_mut(), Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::c_struct! {
        struct Pair {
            a: [f32; 3],
            b: u32,
        }
    }

    crate::c_struct! {
        struct Swapped {
            b: u32,
            a: [f32; 3],
        }
    }

    #[test]
    fn layouts_tell_types_apart() {
        assert_eq!(Pair::LAYOUT.size, 16);
        assert_ne!(Pair::LAYOUT.hash, Swapped::LAYOUT.hash);
        assert_ne!(<[f32; 3]>::LAYOUT.hash, <[f32; 4]>::LAYOUT.hash);
        assert_ne!(<*const Pair>::LAYOUT.hash, <*mut Pair>::LAYOUT.hash);
        type One = unsafe extern "C" fn(*const Pair, *mut f32) -> RlStatus;
        type Other = unsafe extern "C" fn(*const Swapped, *mut f32) -> RlStatus;
        assert_ne!(One::SIGNATURE, Other::SIGNATURE);
    }

    #[test]
    fn guarded_turns_errors_and_panics_into_statuses() {
        assert_eq!(unsafe { guarded(std::ptr::null_mut(), || Ok(())) }, RL_OK);
        assert_eq!(
            call(|m| unsafe { guarded(m, || Err("no light".into())) }).unwrap_err(),
            "no light"
        );
        let panicked = call(|m| unsafe { guarded(m, || panic!("at {}", 3)) }).unwrap_err();
        assert_eq!(panicked, "it panicked: at 3");
        // A message longer than the buffer is cut at a character.
        let mut bytes = [0u8; 4];
        let mut buffer = RlBuffer::new(&mut bytes);
        buffer.write("ab光");
        assert_eq!(buffer.text(), "ab");
    }
}
