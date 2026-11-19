//! Every investigation the lab has, grouped by engine on the homepage: those
//! the packages' libraries hand the lab when they negotiate (src/packages.rs,
//! src/plugin.rs). The libraries are opened the first time the lab needs the
//! list, in the packages' order; a library that cannot be used is reported
//! (and so are the libraries that import it), and its package's content
//! stays mounted.

use crate::investigation::{Engine, Investigation};
use crate::plugin::Handles;
use crate::{packages, resources};
use std::sync::OnceLock;

/// What opening one package's library gave.
pub struct Library {
    pub package: &'static str,
    pub loaded: Result<Handles, String>,
}

static TITLE: OnceLock<&'static str> = OnceLock::new();

/// Names the investigation whose scene the homepage shows (`engine/feature`).
pub fn set_title(path: &'static str) {
    let _ = TITLE.set(path);
}

/// Every package's library, opened once, in the packages' order (each after
/// its imports', which it is handed; src/packages.rs `load_all`).
pub fn libraries() -> &'static [Library] {
    static LIBRARIES: OnceLock<Vec<Library>> = OnceLock::new();
    LIBRARIES.get_or_init(|| {
        let libraries: Vec<Library> = packages::load_all(resources::packages(), packages::load)
            .into_iter()
            .map(|(m, loaded)| Library {
                package: &m.name,
                loaded,
            })
            .collect();
        for library in &libraries {
            if let Err(e) = &library.loaded {
                eprintln!("{e}");
            }
        }
        libraries
    })
}

/// Why a loaded package's investigations cannot run here (its manifest's `requires`), if they cannot.
fn unmet(package: &str) -> Option<&'static str> {
    resources::packages()
        .iter()
        .find(|m| m.name == package)
        .and_then(|m| m.unmet())
}

/// Every investigation the packages' libraries hand the lab, with why it cannot run here, if it cannot: by
/// package name (the libraries open imports first, which would list an importer after the package it imports).
fn handed() -> &'static [(&'static dyn Investigation, Option<&'static str>)] {
    static HANDED: OnceLock<Vec<(&'static dyn Investigation, Option<&'static str>)>> =
        OnceLock::new();
    HANDED.get_or_init(|| {
        let mut libraries: Vec<&Library> = libraries().iter().collect();
        libraries.sort_by_key(|l| l.package);
        libraries
            .into_iter()
            .filter_map(|l| Some((l.package, l.loaded.as_ref().ok()?.investigations()?)))
            .flat_map(|(package, f)| f().iter().map(move |i| (*i, unmet(package))))
            .collect()
    })
}

/// The investigations that cannot run here, each with the reason (what its package requires).
pub fn unavailable() -> Vec<(&'static dyn Investigation, &'static str)> {
    handed()
        .iter()
        .filter_map(|(i, why)| Some((*i, (*why)?)))
        .collect()
}

/// The investigations that can run here.
pub fn all() -> &'static [&'static dyn Investigation] {
    static ALL: OnceLock<Vec<&'static dyn Investigation>> = OnceLock::new();
    ALL.get_or_init(|| {
        let all: Vec<&'static dyn Investigation> = handed()
            .iter()
            .filter(|(_, why)| why.is_none())
            .map(|(i, _)| *i)
            .collect();
        // Their choices' WGSL constants, before any shader composes them.
        crate::shader_data::write_choices(&all);
        all
    })
}

/// The viewer features the packages' libraries add (src/feature/), in the packages' order.
pub fn features() -> Vec<(&'static str, &'static dyn crate::feature::Feature)> {
    libraries()
        .iter()
        .filter_map(|l| Some((l.package, l.loaded.as_ref().ok()?.features()?)))
        .flat_map(|(package, f)| f().iter().map(move |f| (package, *f)))
        .collect()
}

/// The investigation whose backdrop the homepage shows behind every page:
/// the one the binary names, or the first.
pub fn title() -> &'static dyn Investigation {
    TITLE
        .get()
        .and_then(|path| find(path))
        .or_else(|| all().first().copied())
        .expect("no package provides an investigation")
}

/// A field of study as the homepage lists it: one a package declares (src/packages/fields.rs), or Other, where the
/// investigations of packages that join none go.
#[derive(Clone, Copy)]
pub enum Field {
    Declared(&'static packages::Package, &'static packages::Field),
    Other,
}

/// The icon of the Other folder.
const OTHER_ICON: &str = "render_lab.icon/field-other";

impl Field {
    /// Its full key (`render_lab.computer-graphics`), or `other`.
    pub fn id(self) -> String {
        match self {
            Field::Declared(p, f) => format!("{}.{}", p.name, f.key),
            Field::Other => "other".into(),
        }
    }

    /// Its name in the current language.
    pub fn title(self) -> &'static str {
        match self {
            Field::Declared(p, f) => crate::i18n::package_text(&p.name, &f.title),
            Field::Other => crate::i18n::tr("home-field_other"),
        }
    }

    /// The line about it, if its package gives one.
    pub fn summary(self) -> Option<&'static str> {
        match self {
            Field::Declared(p, f) => f
                .summary
                .as_ref()
                .map(|s| crate::i18n::package_text(&p.name, s)),
            Field::Other => None,
        }
    }

    /// Its icon's resource key.
    pub fn icon(self) -> &'static str {
        match self {
            Field::Declared(_, f) => &f.icon,
            Field::Other => OTHER_ICON,
        }
    }

    /// Whether `self` and `other` are the same field.
    pub fn is(self, other: Field) -> bool {
        self.id() == other.id()
    }
}

/// The field `investigation` is in: the one its package joins (`field`), else Other.
pub fn field_of(investigation: &dyn Investigation) -> Field {
    let packages = resources::packages();
    let joined = investigation
        .package()
        .and_then(|name| packages.iter().find(|p| p.name == name))
        .and_then(|p| p.field.as_deref())
        .and_then(|full| full.split_once('.'));
    joined
        .and_then(|(name, key)| {
            let package = packages.iter().find(|p| p.name == name)?;
            let field = package.fields.iter().find(|f| f.key == key)?;
            Some(Field::Declared(package, field))
        })
        .unwrap_or(Field::Other)
}

/// The fields the homepage shows: those declared, by `order` and then the packages' order, that have an
/// investigation that can run here, then Other if an investigation is in it.
pub fn fields() -> Vec<Field> {
    let mut declared: Vec<(i64, usize, Field)> = Vec::new();
    for (n, package) in resources::packages().iter().enumerate() {
        for field in &package.fields {
            declared.push((field.order, n, Field::Declared(package, field)));
        }
    }
    // A stable sort by order keeps the packages' order among equals.
    declared.sort_by_key(|(order, n, _)| (*order, *n));
    let used: Vec<Field> = all().iter().map(|i| field_of(*i)).collect();
    let mut fields: Vec<Field> = declared
        .into_iter()
        .map(|(_, _, f)| f)
        .filter(|f| used.iter().any(|u| u.is(*f)))
        .collect();
    if used.iter().any(|u| matches!(u, Field::Other)) {
        fields.push(Field::Other);
    }
    fields
}

/// The engines of `field`'s investigations, in the order their first investigation is listed.
pub fn engines(field: Field) -> Vec<&'static Engine> {
    let mut engines: Vec<&'static Engine> = Vec::new();
    for i in all().iter().filter(|i| field_of(**i).is(field)) {
        if !engines.iter().any(|e| e.id == i.engine().id) {
            engines.push(i.engine());
        }
    }
    engines
}

/// An investigation by its `engine/feature` path.
pub fn find(path: &str) -> Option<&'static dyn Investigation> {
    all().iter().copied().find(|i| i.path() == path)
}
