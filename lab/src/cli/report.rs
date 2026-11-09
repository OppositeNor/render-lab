//! The commands that check and list: what an investigation and its packages hold, printed as text or JSON.

use super::*;

/// Runs a command that checks or lists for the investigation `chosen`.
pub(super) fn report(command: Command, chosen: &'static dyn Investigation) -> Result<()> {
    match command {
        Command::I18n { check } => {
            let report = crate::i18n::report();
            println!("{}", serde_json::to_string_pretty(&report)?);
            if check && report.iter().any(|r| r["problems"].as_u64() != Some(0)) {
                bail!("a text table has a missing key or a character without a glyph");
            }
            Ok(())
        }
        Command::Text {
            package,
            locale,
            sources,
        } => text(&package, &locale, sources),
        Command::Formulas { check } => {
            let report = crate::formula::check_files(&check)?;
            println!("{}", serde_json::to_string_pretty(&report)?);
            if report.iter().any(|r| r["error"] != serde_json::Value::Null) {
                bail!("a formula failed its check");
            }
            Ok(())
        }
        Command::Knowledge { known, levels } => knowledge(chosen, &known, levels),
        Command::Series => series(),
        Command::Graph { dot, json } => graph(chosen, dot.as_deref(), json),
        Command::Values { json, label, scope } => values(json, label, scope),
        Command::Story(StoryCommand::Keys) => {
            let keys =
                crate::script::keys::keys(investigation::current()).map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&keys)?);
            Ok(())
        }
        Command::Story(StoryCommand::Headings { translation }) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&crate::script::keys::heading_keys(translation))?
            );
            Ok(())
        }
        Command::Story(StoryCommand::Schema) => {
            let schema = crate::script::keys::schema(investigation::current())
                .map_err(anyhow::Error::msg)?;
            println!("{}", serde_json::to_string_pretty(&schema)?);
            Ok(())
        }
        Command::Story(StoryCommand::Check { file, timeline }) => story_check(&file, timeline),
        Command::Choices => choices(),
        Command::Actions => actions(),
        Command::Images => images(),
        Command::Packages { json } => packages(json),
        Command::Resources { json } => resources(json),
        Command::Cook => cook(),
        Command::Layout { json } => layout(json),
        _ => unreachable!("`run` runs the other commands"),
    }
}

/// `values`: the keys the dock, timelines, and stories set, as text or JSON.
fn values(json: bool, label: Option<String>, scope: Option<String>) -> Result<()> {
    use crate::values::Value;
    let entries: Vec<&crate::values::Entry> = crate::values::list()
        .into_iter()
        .filter(|e| label.as_deref().is_none_or(|l| e.has_label(l)))
        .filter(|e| {
            scope
                .as_deref()
                .is_none_or(|s| e.key.starts_with(&format!("{s}.")))
        })
        .collect();
    let numbers = |v: Option<Value>| {
        v.map(|v| {
            v.components()
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
    };
    if json {
        let list: Vec<serde_json::Value> = entries
            .iter()
            .map(|e| {
                serde_json::json!({"key": e.key, "type": e.ty.name(), "default": e.default.map(|v| v.components()), "range": e.range.map(|r| [r.0, r.1]), "componentRanges": e.component_ranges.iter().map(|r| [r.0, r.1]).collect::<Vec<_>>(),
                    "step": e.step, "unit": format!("{:?}", e.unit), "wrap": e.wrap, "labels": e.labels, "text": e.text.map(|_| e.label_text()), "doc": e.doc, "engine": e.engine, "help": e.help.map(|_| e.help_text()), "components": e.components, "choices": e.choices.map(|c| c.options.iter().map(|o| o.name).collect::<Vec<_>>())})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        for e in entries {
            let channels = if e.components.is_empty() {
                String::new()
            } else {
                format!(" ({})", e.components.join(", "))
            };
            let default = numbers(e.default)
                .map(|d| format!(" = {d}"))
                .unwrap_or_default();
            let ranges: Vec<String> = (0..e.ty.components())
                .filter_map(|i| e.range_of(i))
                .map(|r| format!("{}..{}", r.0, r.1))
                .collect();
            let range = if ranges.is_empty() {
                String::new()
            } else if e.component_ranges.is_empty() {
                format!(" in {}", ranges[0])
            } else {
                format!(" in {}", ranges.join(", "))
            };
            let labels = if e.labels.is_empty() {
                String::new()
            } else {
                format!(" [{}]", e.labels.join(", "))
            };
            let choice = e
                .choices
                .map(|c| format!(" choice of {} ({})", c.name, c.names()))
                .unwrap_or_default();
            let engine = e.engine.map(|p| format!(" engine {p}")).unwrap_or_default();
            println!(
                "{}{channels}: {}{default}{range} {:?}{labels}{choice}{engine}",
                e.key,
                e.ty.name(),
                e.unit
            );
            if !e.doc.is_empty() {
                println!("    {}", e.doc);
            }
        }
    }
    Ok(())
}

/// `choices`: the choices (enums), their options, and where their WGSL constants go.
fn choices() -> Result<()> {
    for choices in investigation::current().choices() {
        let options: Vec<String> = (0..choices.options.len())
            .map(|i| {
                format!(
                    "{i} {} ({})",
                    choices.options[i].name,
                    choices.option_text(i)
                )
            })
            .collect();
        println!("{}: {}", choices.name, options.join(", "));
        if let Some(prefix) = choices.wgsl {
            println!(
                "    WGSL {prefix}_*, written to {}'s {}",
                choices.package,
                choices.wgsl_file()
            );
        }
    }
    Ok(())
}

/// `actions`: the click handlers and actions, the code's and the scripts'.
fn actions() -> Result<()> {
    let actions = investigation::current().actions();
    for click in actions.clicks {
        println!("click {}", click.key);
        if !click.doc.is_empty() {
            println!("    {}", click.doc);
        }
    }
    for click in crate::script::runtime::click_handlers() {
        println!("click {click} (a script's)");
    }
    for action in crate::script::runtime::actions() {
        let args: Vec<String> = action.args.iter().map(|a| a.describe()).collect();
        println!("{}({}) (a script's)", action.name, args.join(", "));
    }
    for spec in actions.specs {
        let args: Vec<String> = spec
            .args
            .iter()
            .map(|a| {
                format!(
                    "{}: {}{}",
                    a.key,
                    a.kind.describe(),
                    if a.required {
                        ""
                    } else {
                        " (optional)"
                    }
                )
            })
            .collect();
        println!("{}({})", spec.key, args.join(", "));
        if !spec.doc.is_empty() {
            println!("    {}", spec.doc);
        }
    }
    Ok(())
}

/// `images`: the images, annotations, and GPU image kinds the investigation publishes.
fn images() -> Result<()> {
    for image in investigation::current().images() {
        println!("{}", image.key);
        if !image.doc.is_empty() {
            println!("    {}", image.doc);
        }
    }
    for a in investigation::current().annotations() {
        let kind = match a.kind {
            crate::utility::annotations::AnnotationKind::Point => "point",
            crate::utility::annotations::AnnotationKind::ArrowSet => "arrow set",
        };
        println!("{kind} {}", a.key);
        if !a.doc.is_empty() {
            println!("    {}", a.doc);
        }
    }
    let package = investigation::current().package().unwrap_or("render_lab");
    for kind in crate::views::kinds(package)? {
        let params: Vec<String> = kind
            .params
            .iter()
            .map(|p| {
                format!(
                    "{} = {}",
                    p.key,
                    p.default
                        .iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })
            .collect();
        println!(
            "kind {} ({}x{}{}, {} · {}): {}",
            kind.key,
            kind.size[0],
            kind.size[1],
            if kind.viewport_aspect {
                ", the viewport's shape"
            } else {
                ""
            },
            kind.module,
            kind.entry,
            params.join(", ")
        );
        if !kind.doc.is_empty() {
            println!("    {}", kind.doc);
        }
    }
    Ok(())
}

/// `packages`: each package's metadata, imports, exports, library, demos, and series.
fn packages(json: bool) -> Result<()> {
    let packages = crate::resources::packages();
    // Its library: the file, and its handles or why it is not loaded.
    let library = |m: &crate::packages::Package| -> Option<(String, Result<Vec<String>, String>)> {
        let (name, _) = m.library.as_ref()?;
        let loaded = investigations::libraries()
            .iter()
            .find(|l| l.package == m.name)?;
        Some((
            crate::packages::library_path(m, name).display().to_string(),
            loaded
                .loaded
                .as_ref()
                .map(|h| h.names().map(String::from).collect())
                .map_err(Clone::clone),
        ))
    };
    if json {
        let list: Vec<serde_json::Value> = packages
            .iter()
            .map(|m| {
                let demos: Vec<serde_json::Value> = m.demos.iter().map(|d| serde_json::json!({"key": d.key, "title": d.title, "timeline": d.timeline, "audio": d.audio})).collect();
                let library = library(m).map(|(file, loaded)| match loaded {
                    Ok(handles) => serde_json::json!({"file": file, "handles": handles}),
                    Err(error) => serde_json::json!({"file": file, "error": error}),
                });
                serde_json::json!({"name": m.name, "author": m.author, "contact": m.contact, "license": m.license, "manifest": m.manifest, "import": m.imports, "export": m.exports, "library": library, "demos": demos})
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        for m in packages {
            let contact = m
                .contact
                .as_deref()
                .map(|c| format!(" <{c}>"))
                .unwrap_or_default();
            println!(
                "{}  by {}{contact}, {}  ({})",
                m.name, m.author, m.license, m.manifest
            );
            println!(
                "    import: {}",
                if m.imports.is_empty() {
                    "-".to_string()
                } else {
                    m.imports.join(", ")
                }
            );
            println!(
                "    export: {}",
                if m.exports.is_empty() {
                    "-".to_string()
                } else {
                    m.exports.join(", ")
                }
            );
            match library(m) {
                Some((file, Ok(handles))) => {
                    println!("    library: {file}, handles: {}", handles.join(", "))
                }
                Some((file, Err(_))) => println!("    library: {file}, not loaded (above)"),
                None => {}
            }
            for d in &m.demos {
                println!(
                    "    demo {}: {}, {} + {}",
                    d.key, d.title, d.timeline, d.audio
                );
            }
            for s in &m.series {
                println!(
                    "    series {}: {}, parts {}",
                    s.key,
                    s.title_in("en"),
                    s.parts.join(" -> ")
                );
            }
        }
    }
    Ok(())
}

/// `resources`: the built packages' resources and their sizes.
fn resources(json: bool) -> Result<()> {
    let all = crate::resources::all();
    let size = |r: &crate::resources::Resource| std::fs::metadata(r.path()).ok().map(|m| m.len());
    if json {
        let list: Vec<serde_json::Value> = all
            .iter()
            .map(|r| serde_json::json!({"id": r.id(), "kind": r.kind(), "file": r.path(), "bytes": size(r), "built": r.built(), "license": r.license(), "manifest": r.manifest(), "source": r.source()}))
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        for r in all {
            let size = size(r)
                .map(|b| format!("{b:>10} B"))
                .unwrap_or_else(|| format!("{:>12}", "not built"));
            println!(
                "{:<44} {:<9} {size}  {:<36} {}",
                r.id(),
                r.kind(),
                r.license(),
                r.source()
            );
        }
    }
    Ok(())
}

/// `cook`: builds the checkout's packages beside the binary.
fn cook() -> Result<()> {
    let Some(report) = crate::resources::build_packages()? else {
        bail!(
            "{} has no source packages (RENDER_LAB_ROOT or --root names a checkout)",
            crate::resources::root().display()
        )
    };
    for id in &report.produced {
        println!("built   {id}");
    }
    for package in &report.libraries {
        println!("library {package}");
    }
    for path in &report.removed {
        println!("removed {}", path.display());
    }
    println!(
        "{} built, {} up to date, in {}",
        report.produced.len(),
        report.kept,
        crate::resources::packages_dir().display()
    );
    Ok(())
}

/// `layout`: the WGSL records' layouts checked against the Rust ones.
fn layout(json: bool) -> Result<()> {
    let layouts = layout::parse_layouts(&shaders::layout_json()?)?;
    let checks = layout::check_all(&layouts);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"wgsl": layouts, "checks": checks}))?
        );
    } else {
        for c in &checks {
            println!(
                "{:<16} {:>4} B  padding {:>3} B  {}",
                c.record,
                c.wgsl_size,
                c.padding_bytes,
                if c.ok {
                    "Rust = WGSL"
                } else {
                    "MISMATCH"
                }
            );
            for f in &c.fields {
                println!(
                    "    {:>4}  {:<22} {:<12} {:>3} B{}",
                    f.wgsl_offset,
                    f.name,
                    f.wgsl_type,
                    f.wgsl_size,
                    if f.ok {
                        ""
                    } else {
                        "  <-- mismatch"
                    }
                );
            }
        }
    }
    Ok(())
}

/// `text`: a package's text table in `locale` (its source texts when `sources`).
fn text(package: &str, locale: &str, sources: bool) -> Result<()> {
    crate::i18n::set(
        crate::i18n::Locale::from_code(locale)
            .with_context(|| format!("unknown locale `{locale}`"))?,
    );
    let table: serde_json::Map<String, serde_json::Value> = if sources {
        crate::i18n::source_texts(package)
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.into()))
            .collect()
    } else {
        crate::i18n::package_keys(package)
            .into_iter()
            .map(|k| (k.to_string(), crate::i18n::package_text(package, k).into()))
            .collect()
    };
    println!("{}", serde_json::to_string_pretty(&table)?);
    Ok(())
}

/// `story check`: a storyboard's cues checked, the report printed; exits 1 when it fails.
fn story_check(file: &Path, timeline: bool) -> Result<()> {
    let text = if file == std::path::Path::new("-") {
        std::io::read_to_string(std::io::stdin())?
    } else {
        std::fs::read_to_string(file)
            .with_context(|| format!("read story check input {}", file.display()))?
    };
    let report = crate::story::check(&text, timeline);
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.ok {
        std::process::exit(1);
    }
    Ok(())
}
