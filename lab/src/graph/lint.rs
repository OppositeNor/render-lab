//! The lint: a shading's declared reads and writes against what its WGSL does.

use super::*;

/// Whether `name` is one of `shading`'s volumes (a chain pass lists the
/// volumes it samples in its reads; they are bound at group 3, not group 2).
pub fn is_volume(shading: &SceneShading, name: &str) -> bool {
    shading.volumes.iter().any(|v| v.name == name)
}

/// Whether a chain pass binds `name` at group 3: a volume, or a resource of
/// the program's scene group (a shading without volumes).
pub fn is_group3(shading: &SceneShading, name: &str) -> bool {
    is_volume(shading, name) || (shading.volumes.is_empty() && shading.program.is_some_and(|p| p.scene_uses.iter().any(|u| u.resource == name)))
}

/// The bindings an entry point uses: group, binding, and how.
type Uses = Vec<(u32, u32, GlobalUse)>;

/// Checks `shading`'s declarations against its WGSL: every resource an
/// entry point reads is in its pass's `reads`, every one it writes in its
/// `writes`, and every one listed is used so. A buffer is bound
/// `read_write` whichever list names it; the lists say which it does.
/// Returns what is wrong, one message each.
pub fn lint(shading: &SceneShading) -> Result<Vec<String>> {
    let (module, info) = crate::shaders::analyse(shading.module)?;
    let mut problems = Vec::new();
    let uses = |entry: &str| -> Option<Uses> {
        let index = module.entry_points.iter().position(|e| e.name == entry)?;
        let function = info.get_entry_point(index);
        Some(
            module
                .global_variables
                .iter()
                .filter_map(|(handle, var)| {
                    let binding = var.binding.as_ref()?;
                    let used = function[handle];
                    (!used.is_empty()).then_some((binding.group, binding.binding, used))
                })
                .collect(),
        )
    };
    // Group 2 of the chain: what each binding is.
    let mut chain: Vec<(u32, &str, bool)> = Vec::new();
    for t in shading.targets {
        chain.push((chain_binding(shading, t.name, false), t.name, false));
        chain.push((chain_binding(shading, t.name, true), t.name, true));
    }
    chain.push((chain_binding(shading, "depth", false), "depth", false));
    chain.push((chain_binding(shading, "hdr", true), "hdr", true));
    let buffers: Vec<&str> = shading.buffers.iter().map(|b| b.name).collect();
    for b in &buffers {
        chain.push((chain_binding(shading, b, false), b, false));
    }
    if let Some(d) = shading.depth_views {
        chain.push((chain_binding(shading, d.name, false), d.name, false));
    }
    // Group 3 of a chain pass: the volumes, or (a shading without volumes) the program's scene group.
    let volume = |binding: u32| {
        if shading.volumes.is_empty() {
            return shading.program.and_then(|p| p.scene_uses.iter().find(|u| u.binding == binding)).map(|u| u.resource);
        }
        shading.volumes.get(binding.wrapping_sub(1) as usize).map(|v| v.name)
    };
    // A chain pass, the scene pass (`writes` None: it may only read), or the pre-pass (`subset`: only what it uses is checked).
    let mut check_chain = |entry: &str, reads: &[&str], writes: Option<&[&str]>, subset: bool| {
        let Some(used) = uses(entry) else {
            problems.push(format!("{entry}: no such entry point in {}", shading.module));
            return;
        };
        let mut seen: Vec<(&str, bool)> = Vec::new();
        for &(group, binding, flags) in &used {
            match group {
                2 => {
                    let Some(&(_, name, written)) = chain.iter().find(|c| c.0 == binding) else {
                        problems.push(format!("{entry}: binds group 2 binding {binding}, which is none of the chain's resources"));
                        continue;
                    };
                    if buffers.contains(&name) {
                        if flags.intersects(GlobalUse::READ | GlobalUse::QUERY) {
                            seen.push((name, false));
                            if !reads.contains(&name) && flags.contains(GlobalUse::READ) {
                                problems.push(format!("{entry}: reads buffer `{name}` but does not list it in its reads"));
                            }
                        }
                        if flags.contains(GlobalUse::WRITE) {
                            seen.push((name, true));
                            match writes {
                                Some(w) if !w.contains(&name) => problems.push(format!("{entry}: writes buffer `{name}` but does not list it in its writes")),
                                None => problems.push(format!("{entry}: writes buffer `{name}`, which a scene pass cannot list (it only reads)")),
                                _ => {}
                            }
                        }
                    } else {
                        seen.push((name, written));
                        let listed = if written { writes.is_some_and(|w| w.contains(&name)) } else { reads.contains(&name) };
                        if !listed {
                            problems.push(format!("{entry}: {} `{name}` but does not list it in its {}", if written { "writes" } else { "reads" }, if written { "writes" } else { "reads" }));
                        }
                    }
                }
                3 if binding > 0 => {
                    let Some(name) = volume(binding) else { continue };
                    seen.push((name, false));
                    if !reads.contains(&name) {
                        problems.push(format!("{entry}: samples volume `{name}` but does not list it in its reads"));
                    }
                }
                _ => {}
            }
        }
        if subset {
            return;
        }
        for name in reads {
            if !seen.contains(&(name, false)) {
                problems.push(format!("{entry}: lists `{name}` in its reads but does not read it"));
            }
        }
        for name in writes.unwrap_or(&[]) {
            if !seen.contains(&(name, true)) {
                problems.push(format!("{entry}: lists `{name}` in its writes but does not write it"));
            }
        }
    };
    for pass in shading.passes.iter() {
        check_chain(pass.entry, pass.reads, Some(pass.writes), false);
    }
    if shading.chained() {
        check_chain(shading.fragment, shading.scene_reads, None, false);
        // The pre-pass runs before the passes that fill the scene's reads: it reads none of them,
        // but may read the program's scene group, which it binds too.
        if let Some(entry) = shading.prepass {
            let scene_uses: Vec<&str> = shading.program.map_or(Vec::new(), |p| program_reads(p.scene_uses));
            check_chain(entry, &scene_uses, None, true);
        }
    }
    drop(check_chain);
    for pass in shading.passes.iter() {
        if pass.writes.iter().any(|w| is_volume(shading, w)) {
            problems.push(format!("{}: a chain pass cannot write a volume (an update pass does)", pass.entry));
        }
    }
    // The update passes: group 2 their parameters, the update's buffers, the volumes written; group 3 the volumes read.
    if let Some(update) = shading.update {
        let nb = update.buffers.len() as u32;
        for pass in update.passes {
            let Some(used) = uses(pass.entry) else {
                problems.push(format!("{}: no such entry point in {}", pass.entry, shading.module));
                continue;
            };
            let mut seen: Vec<(&str, bool)> = Vec::new();
            for &(group, binding, flags) in &used {
                match (group, binding) {
                    (2, 0) => {}
                    (2, b) if b <= nb => {
                        let name = update.buffers[(b - 1) as usize].name;
                        if flags.contains(GlobalUse::READ) {
                            seen.push((name, false));
                            if !pass.reads.contains(&name) {
                                problems.push(format!("{}: reads buffer `{name}` but does not list it in its reads", pass.entry));
                            }
                        }
                        if flags.contains(GlobalUse::QUERY) && pass.reads.contains(&name) {
                            seen.push((name, false));
                        }
                        if flags.contains(GlobalUse::WRITE) {
                            seen.push((name, true));
                            if !pass.writes.contains(&name) {
                                problems.push(format!("{}: writes buffer `{name}` but does not list it in its writes", pass.entry));
                            }
                        }
                    }
                    (2, b) => {
                        if let Some(v) = shading.volumes.get((b - 1 - nb) as usize) {
                            seen.push((v.name, true));
                            if !pass.writes.contains(&v.name) {
                                problems.push(format!("{}: writes volume `{}` but does not list it in its writes", pass.entry, v.name));
                            }
                        }
                    }
                    (3, b) if b > 0 => {
                        if let Some(name) = volume(b) {
                            seen.push((name, false));
                            if !pass.reads.contains(&name) {
                                problems.push(format!("{}: samples volume `{name}` but does not list it in its reads", pass.entry));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for name in pass.reads {
                if !seen.contains(&(name, false)) {
                    problems.push(format!("{}: lists `{name}` in its reads but does not read it", pass.entry));
                }
            }
            for name in pass.writes {
                if !seen.contains(&(name, true)) {
                    problems.push(format!("{}: lists `{name}` in its writes but does not write it", pass.entry));
                }
            }
        }
    }
    if let Some(program) = shading.program {
        // The scene group is bound by the scene pass, the pre-pass, and (a shading without volumes) the chain's compute passes.
        let mut chain_entries: Vec<&str> = shading.prepass.into_iter().collect();
        if shading.volumes.is_empty() {
            chain_entries.extend(shading.passes.iter().map(|p| p.entry));
        }
        lint_program(shading, program, &uses, &mut problems, shading.module, shading.fragment, &chain_entries);
    }
    Ok(problems)
}

/// A program's passes against their WGSL: every group-3 binding an entry
/// point uses is one of its `uses`, used as declared (a write where it
/// writes, atomics where `Atomic`), and every use is used.
pub(super) fn lint_program(shading: &SceneShading, program: &crate::investigation::ShadingProgram, uses: &dyn Fn(&str) -> Option<Uses>, problems: &mut Vec<String>, module: &str, fragment: &str, chain_entries: &[&str]) {
    use crate::investigation::{Access, PassKind};
    let own = |name: &str| program.images.iter().any(|i| i.name == name) || program.buffers.iter().any(|b| b.name == name) || matches!(name, "linearSampler" | "nearestSampler" | "trilinearSampler");
    let frame = |name: &str| crate::program::external(shading, name).is_some();
    let mut check = |label: &str, entries: &[&str], declared: &[crate::investigation::ProgramUse], read_only: bool| {
        let mut used: Vec<(u32, GlobalUse)> = Vec::new();
        for entry in entries {
            let Some(u) = uses(entry) else {
                problems.push(format!("{label}: no entry point `{entry}` in {module}"));
                return;
            };
            for (group, binding, flags) in u {
                if group != 3 {
                    continue;
                }
                match used.iter_mut().find(|x| x.0 == binding) {
                    Some(x) => x.1 |= flags,
                    None => used.push((binding, flags)),
                }
            }
        }
        for d in declared {
            if read_only && frame(d.resource) {
                problems.push(format!("{label}: the scene pass reads the frame's `{}` itself, not through the program", d.resource));
            } else if frame(d.resource) && d.access != Access::Load {
                problems.push(format!("{label}: reads the frame's `{}` with {:?}; the frame's textures are read with Load", d.resource, d.access));
            } else if !own(d.resource) && !frame(d.resource) {
                problems.push(format!("{label}: uses `{}`, which is none of the program's images, buffers, or samplers, nor a texture of the frame", d.resource));
            }
            if read_only && d.access.writes() {
                problems.push(format!("{label}: the scene pass may only read `{}`", d.resource));
            }
        }
        for (binding, flags) in &used {
            let Some(d) = declared.iter().find(|d| d.binding == *binding) else {
                problems.push(format!("{label}: binds group 3 binding {binding}, which none of its uses declares"));
                continue;
            };
            if flags.contains(GlobalUse::WRITE) && !d.access.writes() {
                problems.push(format!("{label}: writes `{}` (binding {binding}), declared {:?}", d.resource, d.access));
            }
            // naga marks texture atomics; a buffer's atomics are reads and writes.
            let image = program.images.iter().any(|i| i.name == d.resource);
            if image && flags.contains(GlobalUse::ATOMIC) != (d.access == Access::Atomic) && !matches!(d.access, Access::Sampler | Access::Uniform) {
                problems.push(format!("{label}: `{}` (binding {binding}) is {}used with atomics, declared {:?}", d.resource, if flags.contains(GlobalUse::ATOMIC) { "" } else { "not " }, d.access));
            }
            if d.access.writes() && !flags.intersects(GlobalUse::WRITE | GlobalUse::ATOMIC) {
                problems.push(format!("{label}: declares `{}` {:?} but only reads it", d.resource, d.access));
            }
        }
        for d in declared {
            if !used.iter().any(|(b, _)| *b == d.binding) {
                problems.push(format!("{label}: declares `{}` at binding {} but does not use it", d.resource, d.binding));
            }
        }
    };
    for pass in program.passes {
        match pass.kind {
            PassKind::Compute { entry, .. } => check(pass.name, &[entry], pass.uses, false),
            PassKind::Raster { vertex, fragment } => check(pass.name, &[vertex, fragment], pass.uses, false),
        }
    }
    // The scene group: read by the scene pass and the chain's compute passes (`chain`).
    if !program.scene_uses.is_empty() {
        let mut entries = vec![fragment];
        entries.extend(chain_entries.iter().copied());
        check(fragment, &entries, program.scene_uses, true);
    }

}
