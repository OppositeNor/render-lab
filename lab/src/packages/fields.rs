//! Fields of study: a package declares one with a `[[provides]]` of `type = "field"`, and a package with
//! investigations joins one with `field = "<package>.<key>"`. The homepage's first page after the title shows a
//! folder per field (src/home/fields_page.rs); a package that joins none goes to the Other folder.

use super::{Package, resolve, string, valid_demo_key};

/// A field of study a package declares: the homepage's folder for the investigations of the packages that join it.
pub struct Field {
    /// Its key, unique among its package's fields; other packages name it `<package>.<key>`, as an export.
    pub key: String,
    /// Its name and, if given, a line about it: keys of its package's text tables.
    pub title: String,
    pub summary: Option<String>,
    /// Its icon, a PNG resource (a white mask, tinted where it is drawn): by full key once the packages are scanned.
    pub icon: String,
    /// Its place on the homepage, smaller first; ties keep the packages' order.
    pub order: i64,
}

/// The fields of a `[[provides]]` of `type = "field"`.
pub(super) const FIELD_KEYS: &[&str] =
    &["type", "key", "title-text", "summary-text", "icon", "order"];

/// A `[[provides]]` field: its key (unique among `fields`), its text keys, its icon as written, and its order.
pub(super) fn read_field(
    entry: &toml_edit::Table,
    fields: &[Field],
    fail: &dyn Fn(String) -> String,
) -> Result<Field, String> {
    let required = |name: &str| {
        string(entry, name, fail)?.ok_or_else(|| fail(format!("a field needs `{name}`, a string")))
    };
    let key = required("key")?;
    if !valid_demo_key(&key) || fields.iter().any(|f| f.key == key) {
        return Err(fail(format!(
            "field `{key}`: a key is letters, digits, `-` and `_`, unique in its package"
        )));
    }
    let order = match entry.get("order") {
        None => 0,
        Some(item) => item
            .as_integer()
            .ok_or_else(|| fail(format!("field `{key}`: `order` is an integer")))?,
    };
    Ok(Field {
        title: required("title-text")?,
        summary: string(entry, "summary-text", fail)?,
        icon: required("icon")?,
        key,
        order,
    })
}

/// `package`'s field icons and the field it joins, resolved to full keys against the packages taken before it
/// (`ordered`, its imports among them); the joined field must be one its package declares.
pub(super) fn resolve_fields(package: &mut Package, ordered: &[Package]) -> Result<(), String> {
    let fail = |e: String| format!("packages: {}: {e}", package.manifest);
    let mut icons = Vec::new();
    for field in &package.fields {
        let icon = resolve(package, ordered, &field.icon)
            .map_err(|e| fail(format!("field `{}`: icon {e}", field.key)))?;
        icons.push(icon);
    }
    let joined = match &package.field {
        Some(reference) => {
            let full =
                resolve(package, ordered, reference).map_err(|e| fail(format!("`field`: {e}")))?;
            // The package the reference names, the package itself or an import, must declare that field.
            let (name, key) = full.split_once('.').unwrap_or_default();
            let declares = |p: &Package| p.fields.iter().any(|f| f.key == key);
            let declared = if name == package.name {
                declares(package)
            } else {
                ordered.iter().any(|p| p.name == name && declares(p))
            };
            if !declared {
                return Err(fail(format!(
                    "`field = \"{reference}\"`: the package `{name}` declares no field `{key}`"
                )));
            }
            Some(full)
        }
        None => None,
    };
    for (field, icon) in package.fields.iter_mut().zip(icons) {
        field.icon = icon;
    }
    package.field = joined;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::scan;
    use super::super::tests::{directory, manifest};

    /// `a` declares the field `f`, which it exports; `b` joins it by full key.
    fn field(key: &str) -> String {
        format!(
            "[[provides]]\ntype = \"field\"\nkey = \"{key}\"\ntitle-text = \"field-f\"\nicon = \"icon/f\"\norder = 2\n"
        )
    }

    #[test]
    fn a_package_joins_a_field_it_imports() {
        let a = manifest("a", "", "\"f\"") + &field("f");
        let b = "field = \"a.f\"\n".to_string() + &manifest("b", "\"a\"", "");
        let dir = directory(
            "fields",
            &[("a", &a), ("b", &b), ("c", &manifest("c", "", ""))],
        );
        let packages = scan(&dir, &dir).unwrap();
        let find = |name: &str| packages.iter().find(|p| p.name == name).unwrap();
        let f = &find("a").fields[0];
        assert_eq!(
            (f.key.as_str(), f.icon.as_str(), f.order),
            ("f", "a.icon/f", 2)
        );
        assert_eq!(find("b").field.as_deref(), Some("a.f"));
        // A package that joins none goes to the homepage's Other folder.
        assert_eq!(find("c").field, None);
    }

    #[test]
    fn a_duplicate_field_is_refused() {
        let a = manifest("a", "", "") + &field("f") + &field("f");
        let dir = directory("fields-duplicate", &[("a", &a)]);
        assert!(
            scan(&dir, &dir)
                .err()
                .unwrap()
                .contains("unique in its package")
        );
    }

    #[test]
    fn a_bad_field_reference_is_refused() {
        let a = manifest("a", "", "\"f\"") + &field("f");
        // Not imported, not exported, and not declared.
        for (test, b, expected) in [
            (
                "unimported",
                "field = \"a.f\"\n".to_string() + &manifest("b", "", ""),
                "does not list",
            ),
            (
                "unexported",
                "field = \"a.g\"\n".to_string() + &manifest("b", "\"a\"", ""),
                "does not export",
            ),
            (
                "undeclared",
                "field = \"g\"\n".to_string() + &manifest("b", "", ""),
                "declares no field",
            ),
        ] {
            let dir = directory(&format!("fields-{test}"), &[("a", &a), ("b", &b)]);
            let error = scan(&dir, &dir).err().unwrap();
            assert!(error.contains(expected), "{test}: {error}");
        }
    }
}
