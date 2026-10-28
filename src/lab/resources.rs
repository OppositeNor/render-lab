//! The resources declared in `resources.toml` manifests (src/lab/resources.toml
//! and any layer's own), as build.rs's resource pipeline (pipeline/mod.rs)
//! processed them, embedded in the program. Each is a static named after its id
//! (`font/dejavu-sans` is `FONT_DEJAVU_SANS`); `ALL` lists them.

pub struct Resource {
    pub id: &'static str,
    /// The processor that made `bytes` (pipeline/mod.rs's `KINDS`).
    pub kind: &'static str,
    /// The manifest that declares it and the file it was made from, from the
    /// crate root.
    pub manifest: &'static str,
    pub source: &'static str,
    pub license: &'static str,
    pub bytes: &'static [u8],
}

include!(concat!(env!("OUT_DIR"), "/resources.rs"));

impl Resource {
    /// The little-endian `u32` at word `i`: a processor's header.
    pub fn word(&self, i: usize) -> u32 {
        u32::from_le_bytes(self.bytes[4 * i..4 * i + 4].try_into().unwrap())
    }

    /// `count` values of `T` from byte `offset`, copied (the embedded bytes
    /// are not aligned).
    pub fn array<T: bytemuck::Pod>(&self, offset: usize, count: usize) -> Vec<T> {
        bytemuck::pod_collect_to_vec(&self.bytes[offset..offset + count * size_of::<T>()])
    }

    pub fn text(&self) -> &'static str {
        std::str::from_utf8(self.bytes).unwrap_or_else(|_| panic!("resource {} is not UTF-8", self.id))
    }
}
