//! The resources the built packages declare (src/packages.rs: every
//! directory of `packages/` beside the binary, `packages_dir`), loaded at run
//! time. From a source checkout (the lab's root has `packages/`), the lab
//! first brings the built packages up to date with their sources
//! (src/cook.rs, incrementally); a binary without sources reads what is
//! beside it. A resource is named by its full key
//! (`resources::get("render_lab.font/dejavu-sans")`).
//!
//! Nothing is read until it is asked for. Data a scene decodes (the sky, a
//! mesh) is read with `load`, which keeps no copy; the lab's loading step
//! (src/loading.rs) `prepare`s a scene's resources before it plays. Data
//! the lab keeps for the whole run (fonts) is read with `bytes`.

use crate::investigation::TimelineInfo;
use crate::packages::{self, Package};
use crate::cook;
use crate::pipeline::{self, Declaration, Form, ShaderDeclaration};
use anyhow::{anyhow, Context, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Resource {
    decl: Declaration,
    bytes: OnceLock<Vec<u8>>,
}

struct Registry {
    /// Each after the packages it imports.
    packages: Vec<Package>,
    resources: Vec<Resource>,
    shaders: Vec<ShaderDeclaration>,
    /// Each package's demos, as the viewer plays them.
    timelines: BTreeMap<String, Vec<TimelineInfo>>,
}

static ROOT: OnceLock<PathBuf> = OnceLock::new();
static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// Sets the lab's root (a source checkout: the repository's directory)
/// before any resource is loaded. Without it the root is `RENDER_LAB_ROOT`,
/// or the repository this lab was built from.
pub fn set_root(root: PathBuf) {
    assert!(ROOT.set(root).is_ok(), "the lab's root is set once, before any resource is loaded");
}

/// The lab's root: the source checkout whose packages are built, and the
/// paths messages name are relative to it.
pub fn root() -> &'static Path {
    ROOT.get_or_init(|| std::env::var_os("RENDER_LAB_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(env!("LAB_ROOT"))))
}

/// Where the built packages are: `RENDER_LAB_PACKAGES`, or `packages/` beside the binary.
pub fn packages_dir() -> PathBuf {
    std::env::var_os("RENDER_LAB_PACKAGES").map(PathBuf::from).unwrap_or_else(|| packages::bin_dir().join("packages"))
}

/// The source checkout's packages, if the root is one.
pub fn sources_dir() -> Option<PathBuf> {
    let dir = root().join("packages");
    let has_package = std::fs::read_dir(&dir).ok()?.flatten().any(|e| e.path().join("manifest.toml").is_file());
    has_package.then_some(dir)
}

/// Builds the source checkout's packages into `packages_dir` (nothing
/// without sources): what the lab does before it reads them.
pub fn build_packages() -> Result<Option<cook::Report>> {
    let Some(sources) = sources_dir() else { return Ok(None) };
    cook::cook(root(), &sources, &packages_dir()).map(Some).map_err(|e| anyhow!(e))
}

fn mount() -> Result<Registry> {
    let root = root().to_path_buf();
    if let Some(report) = build_packages()? {
        if !report.produced.is_empty() || !report.libraries.is_empty() {
            eprintln!("packages: built {} resource(s) and {} library(ies) into {}", report.produced.len(), report.libraries.len(), packages_dir().display());
        }
    }
    let dir = packages_dir();
    if !dir.is_dir() {
        return Err(anyhow!("packages: no {} (a source checkout's packages are built there; RENDER_LAB_ROOT or --root names one)", dir.display()));
    }
    let packages = packages::scan(&root, &dir).map_err(|e| anyhow!(e))?;
    let mut resources: Vec<Resource> = Vec::new();
    let mut shaders: Vec<ShaderDeclaration> = Vec::new();
    let mut overrides: Vec<Declaration> = Vec::new();
    for (i, package) in packages.iter().enumerate() {
        let resolve = |reference: &str| packages::resolve(package, &packages[..i], reference);
        for collection in &package.collections {
            let read = pipeline::read_collection(&root, &package.dir, collection, &package.name, Form::Built, &resolve).map_err(|e| anyhow!(e))?;
            for decl in read.resources {
                if let Some(other) = resources.iter().find(|r| r.decl.id == decl.id) {
                    return Err(anyhow!("resources: `{}` is declared by {} and {}", decl.id, other.decl.manifest, decl.manifest));
                }
                if decl.overrides.is_some() {
                    overrides.push(decl.clone());
                }
                resources.push(Resource { decl, bytes: OnceLock::new() });
            }
            for shader in read.shaders {
                if let Some(other) = shaders.iter().find(|s| s.id == shader.id) {
                    return Err(anyhow!("resources: shader `{}` is declared by {} and {}", shader.id, other.manifest, shader.manifest));
                }
                shaders.push(shader);
            }
        }
    }
    // A package replaces, for everyone, what an imported package lets it.
    let mut replaced: Vec<(String, String)> = Vec::new();
    for decl in overrides {
        let target = decl.overrides.clone().unwrap();
        if let Some((_, by)) = replaced.iter().find(|(t, _)| *t == target) {
            return Err(anyhow!("resources: `{target}` is overridden by {by} and {}", decl.manifest));
        }
        let Some(slot) = resources.iter_mut().find(|r| r.decl.id == target) else {
            return Err(anyhow!("resources: `{}` ({}) overrides `{target}`, which is not a resource any package declares", decl.id, decl.manifest));
        };
        if !slot.decl.overridable {
            return Err(anyhow!("resources: `{}` ({}) overrides `{target}`, which is not `overridable` ({})", decl.id, decl.manifest, slot.decl.manifest));
        }
        replaced.push((target.clone(), decl.manifest.clone()));
        *slot = Resource { decl: Declaration { id: target, overridable: false, overrides: None, ..decl }, bytes: OnceLock::new() };
    }
    resources.sort_by(|a, b| a.decl.id.cmp(&b.decl.id));
    let declared = |key: &str| shaders.iter().any(|s| s.id == key) || resources.binary_search_by(|r| r.decl.id.as_str().cmp(key)).is_ok();
    for shader in &shaders {
        for part in &shader.parts {
            if !declared(part) {
                return Err(anyhow!("resources: shader `{}` ({}): no resource or shader `{part}`", shader.id, shader.manifest));
            }
        }
    }
    let mut timelines = BTreeMap::new();
    for package in &packages {
        for key in &package.exports {
            if !declared(&format!("{}.{key}", package.name)) {
                return Err(anyhow!("packages: {}: exports `{key}`, which it does not declare", package.manifest));
            }
        }
        let mut list = Vec::new();
        for demo in &package.demos {
            let source = |key: &str| -> Result<String> {
                let i = resources.binary_search_by(|r| r.decl.id.as_str().cmp(key)).map_err(|_| anyhow!("packages: {}: demo `{}`: no resource `{key}`", package.manifest, demo.key))?;
                Ok(resources[i].decl.file.to_string_lossy().into_owned())
            };
            list.push(TimelineInfo { id: demo.key.clone(), title: demo.title.clone(), edition: demo.edition.clone(), path: source(&demo.timeline)?, narration: source(&demo.audio)? });
        }
        timelines.insert(package.name.clone(), list);
    }
    Ok(Registry { packages, resources, shaders, timelines })
}

fn registry() -> &'static Registry {
    REGISTRY.get_or_init(|| mount().unwrap_or_else(|e| panic!("{e:#}")))
}

/// Every package, each after the packages it imports.
pub fn packages() -> &'static [Package] {
    &registry().packages
}

/// The package `name`'s demos, as the viewer plays them.
pub fn timelines(name: &str) -> &'static [TimelineInfo] {
    registry().timelines.get(name).map(Vec::as_slice).unwrap_or_else(|| panic!("no package `{name}` in {}", packages_dir().display()))
}

/// Every resource of the packages, by full key.
pub fn all() -> &'static [Resource] {
    &registry().resources
}

/// The resource `id` (a full key), if a package declares it.
pub fn find(id: &str) -> Option<&'static Resource> {
    let resources = &registry().resources;
    resources.binary_search_by(|r| r.decl.id.as_str().cmp(id)).ok().map(|i| &resources[i])
}

/// Every shader the packages declare, in their order.
pub fn shaders() -> &'static [ShaderDeclaration] {
    &registry().shaders
}

/// The shader `id` (a full key), if a package declares it.
pub fn shader(id: &str) -> Option<&'static ShaderDeclaration> {
    registry().shaders.iter().find(|s| s.id == id)
}

/// The resource `id` (a full key); a resource no package declares is a bug in
/// the code that names it.
pub fn get(id: &str) -> &'static Resource {
    find(id).unwrap_or_else(|| panic!("no resource `{id}` in the packages {}", registry().packages.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", ")))
}

impl Resource {
    pub fn id(&self) -> &str {
        &self.decl.id
    }

    /// The processor that made `bytes` (src/pipeline's `KINDS`).
    pub fn kind(&self) -> &'static str {
        self.decl.kind.name
    }

    /// The built collection that declares it.
    pub fn manifest(&self) -> &str {
        &self.decl.manifest
    }

    /// The file it was made from, from the lab's root (for messages).
    pub fn source(&self) -> &str {
        &self.decl.source
    }

    /// Its file in its built package, for what reads a file itself (FFplay,
    /// the viewer's timeline and the images beside it).
    pub fn path(&self) -> PathBuf {
        self.decl.file.clone()
    }

    /// A tool's output; it may not be built yet.
    pub fn built(&self) -> bool {
        self.decl.built
    }

    pub fn license(&self) -> &str {
        &self.decl.license
    }

    fn read(&self) -> Result<Vec<u8>> {
        std::fs::read(&self.decl.file).with_context(|| format!("resources: {}: read {}", self.decl.id, self.decl.file.display()))
    }

    /// Checks that it is there to read (a tool's output may not be built yet).
    pub fn prepare(&self) -> Result<()> {
        if self.decl.file.exists() { Ok(()) } else { Err(anyhow!("resources: {}: {} is not built", self.decl.id, self.decl.file.display())) }
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
