//! The resources the packages declare (each a `resources.toml` beside its
//! layer's code: src/lab/resources.toml, an investigation's
//! `Investigation::package`, an extension's `packages`), loaded at run time.
//! The resource pipeline (src/lab/pipeline) processes a resource the first
//! time it is asked for and caches the result in build/resources under the
//! lab's root; `cook` processes them all. A resource is named by its id
//! (`resources::get("font/dejavu-sans")`).
//!
//! Nothing is read until it is asked for. Data a scene decodes (the sky, a
//! mesh) is read with `load`, which keeps no copy; the lab's loading step
//! (src/lab/loading.rs) `prepare`s a scene's resources before it plays. Data
//! the lab keeps for the whole run (fonts) is read with `bytes`.

use crate::lab::pipeline::{self, Declaration, ShaderDeclaration};
use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Resource {
    decl: Declaration,
    bytes: OnceLock<Vec<u8>>,
}

struct Registry {
    root: PathBuf,
    /// The manifests mounted, in order, from the root.
    packages: Vec<String>,
    resources: Vec<Resource>,
    shaders: Vec<ShaderDeclaration>,
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Sets the lab's root (the directory of Cargo.toml) before any resource is
/// loaded. Without it the root is `RENDER_LAB_ROOT`, or the crate's directory.
pub fn set_root(root: PathBuf) {
    assert!(ROOT.set(root).is_ok(), "the lab's root is set once, before any resource is loaded");
}

/// The lab's root: package manifests, the cache, and the paths content names
/// are relative to it.
pub fn root() -> &'static Path {
    ROOT.get_or_init(|| std::env::var_os("RENDER_LAB_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR"))))
}

/// Where processed resources are cached.
pub fn cache_dir() -> PathBuf {
    root().join("build/resources")
}

fn mount() -> Result<Registry> {
    let root = root().to_path_buf();
    let packages: Vec<String> = crate::investigations::packages();
    let mut resources: Vec<Resource> = Vec::new();
    let mut shaders: Vec<ShaderDeclaration> = Vec::new();
    for package in &packages {
        let read = pipeline::read_package(&root, &root.join(package)).map_err(|e| anyhow!(e))?;
        for decl in read.resources {
            match resources.iter().position(|r| r.decl.id == decl.id) {
                // The overlay: a later package replaces what an earlier one lets it.
                Some(i) if decl.overrides && resources[i].decl.overridable => resources[i] = Resource { decl, bytes: OnceLock::new() },
                Some(i) => return Err(anyhow!("resources: `{}` is declared by {} and {} (only an `overridable` resource may be replaced, by one with `override`)", decl.id, resources[i].decl.manifest, decl.manifest)),
                None if decl.overrides => return Err(anyhow!("resources: `{}` ({}) overrides a resource no earlier package declares", decl.id, decl.manifest)),
                None => resources.push(Resource { decl, bytes: OnceLock::new() }),
            }
        }
        for shader in read.shaders {
            if let Some(other) = shaders.iter().find(|s| s.id == shader.id) {
                return Err(anyhow!("resources: shader `{}` is declared by {} and {}", shader.id, other.manifest, shader.manifest));
            }
            shaders.push(shader);
        }
    }
    resources.sort_by(|a, b| a.decl.id.cmp(&b.decl.id));
    for shader in &shaders {
        for part in &shader.parts {
            if !shaders.iter().any(|s| &s.id == part) && resources.binary_search_by(|r| r.decl.id.as_str().cmp(part)).is_err() {
                return Err(anyhow!("resources: shader `{}` ({}): no resource or shader `{part}`", shader.id, shader.manifest));
            }
        }
    }
    Ok(Registry { root, packages, resources, shaders })
}

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| mount().unwrap_or_else(|e| panic!("{e:#}")))
}

/// Every resource of the mounted packages, by id.
pub fn all() -> &'static [Resource] {
    &registry().resources
}

/// The resource `id`, if a package declares it.
pub fn find(id: &str) -> Option<&'static Resource> {
    let resources = &registry().resources;
    resources.binary_search_by(|r| r.decl.id.as_str().cmp(id)).ok().map(|i| &resources[i])
}

/// Every shader the packages declare, in the order they are mounted.
pub fn shaders() -> &'static [ShaderDeclaration] {
    &registry().shaders
}

/// The shader `id`, if a package declares it.
pub fn shader(id: &str) -> Option<&'static ShaderDeclaration> {
    registry().shaders.iter().find(|s| s.id == id)
}

/// The resource `id`; a resource no package declares is a bug in the code
/// that names it.
pub fn get(id: &str) -> &'static Resource {
    find(id).unwrap_or_else(|| panic!("no resource `{id}` in the packages {}", registry().packages.join(", ")))
}

/// What `cook` did with one resource.
pub struct Cooked {
    pub resource: &'static Resource,
    pub file: PathBuf,
    pub bytes: usize,
    pub cached: bool,
}

/// Processes every resource that is not cached yet, and removes cached files
/// no resource names any more.
pub fn cook() -> Result<Vec<Cooked>> {
    let registry = registry();
    let cache = cache_dir();
    std::fs::create_dir_all(&cache).with_context(|| format!("create {}", cache.display()))?;
    let mut cooked = Vec::new();
    for resource in &registry.resources {
        let (bytes, file, cached) = resource.decl.cook(&registry.root, &cache).map_err(|e| anyhow!(e))?;
        cooked.push(Cooked { resource, file, bytes: bytes.len(), cached });
    }
    for entry in std::fs::read_dir(&cache).with_context(|| format!("read {}", cache.display()))?.flatten() {
        if !cooked.iter().any(|c| c.file == entry.path()) {
            std::fs::remove_file(entry.path()).with_context(|| format!("remove {}", entry.path().display()))?;
        }
    }
    Ok(cooked)
}

impl Resource {
    pub fn id(&self) -> &str {
        &self.decl.id
    }

    /// The processor that made `bytes` (src/lab/pipeline's `KINDS`).
    pub fn kind(&self) -> &'static str {
        self.decl.kind.name
    }

    /// The manifest that declares it, from the lab's root.
    pub fn manifest(&self) -> &str {
        &self.decl.manifest
    }

    /// The file it is made from, from the lab's root.
    pub fn source(&self) -> &str {
        &self.decl.source
    }

    pub fn license(&self) -> &str {
        &self.decl.license
    }

    fn read(&self) -> Result<Vec<u8>> {
        let registry = registry();
        self.decl.cook(&registry.root, &cache_dir()).map(|(bytes, _, _)| bytes).map_err(|e| anyhow!(e))
    }

    /// Processes it into the cache if it is not there yet, without keeping it.
    pub fn prepare(&self) -> Result<()> {
        let registry = registry();
        self.decl.prepare(&registry.root, &cache_dir()).map_err(|e| anyhow!(e))
    }

    /// The processed bytes, for whoever decodes them; the lab keeps no copy.
    pub fn load(&'static self) -> Loaded {
        Loaded { resource: self, bytes: self.read().unwrap_or_else(|e| panic!("{e:#}")) }
    }

    /// The processed bytes, read on first use and kept for the whole run.
    pub fn bytes(&'static self) -> &'static [u8] {
        self.bytes.get_or_init(|| self.read().unwrap_or_else(|e| panic!("{e:#}")))
    }

    pub fn text(&'static self) -> &'static str {
        std::str::from_utf8(self.bytes()).unwrap_or_else(|_| panic!("resource {} is not UTF-8", self.id()))
    }
}

/// A resource's processed bytes, read by `Resource::load`.
pub struct Loaded {
    pub resource: &'static Resource,
    pub bytes: Vec<u8>,
}

impl Loaded {
    pub fn id(&self) -> &str {
        self.resource.id()
    }

    /// The little-endian `u32` at word `i`: a processor's header.
    pub fn word(&self, i: usize) -> u32 {
        u32::from_le_bytes(self.bytes[4 * i..4 * i + 4].try_into().unwrap())
    }

    /// `count` values of `T` from byte `offset`, copied (the bytes need not be
    /// aligned).
    pub fn array<T: bytemuck::Pod>(&self, offset: usize, count: usize) -> Vec<T> {
        bytemuck::pod_collect_to_vec(&self.bytes[offset..offset + count * size_of::<T>()])
    }
}
