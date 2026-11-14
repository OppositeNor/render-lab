//! The Experiments tab's snapshot pages: the tab, a comparison with a reference, the session carried across
//! demonstrations, and saving over a file.

use super::{Experiments, Pending};
use crate::experiment::Compare;
use crate::feature::{Host, Snapshot};
use crate::investigation;
use crate::ui::ViewerAction;
use crate::values::{self, Entry, Reflect};
use anyhow::Result;

impl Experiments {
    /// One step of `page`. The demonstration at its defaults, rendered over eighteen frames; `viewer-compare`
    /// first sets it as the reference, moves one engine setting (else any ranged value) to its other end,
    /// compares them with a wipe, and measures them before the fifteenth; `viewer-compare-stale` then puts the
    /// value back before the sixteenth, so the measurement no longer shows the live state.
    /// `viewer-experiments-session` changes demonstrations (`session_step`).
    pub(super) fn page_step(
        &mut self,
        page: &str,
        step: usize,
        host: &mut dyn Host,
    ) -> Result<Snapshot> {
        if page == "viewer-experiments-session" {
            return Ok(self.session_step(step, host));
        }
        if page == "viewer-experiments-overwrite" {
            return self.overwrite_step(step, host);
        }
        let stale_page = page == "viewer-compare-stale";
        let compare = page == "viewer-compare" || stale_page;
        if step == 0 {
            let defaults = host.default_state(host.state().demo);
            *host.state_mut() = defaults;
            if compare {
                // The comparison page: the defaults as the reference, then one ranged value moved to its far end.
                self.set_reference(host);
                if let Some((key, now, to)) = move_far(host) {
                    self.report
                        .push(format!("reference compared with {key} {now} → {to}"));
                }
                self.session.compare = Compare::Wipe;
                // A setting baked into the scene data: its build waited for, so the snapshot shows it.
                let state = host.state().clone();
                if let Some(renderer) = host.renderer() {
                    renderer.prepare_scene_data(&state);
                }
            }
        }
        if step == 18 {
            if let Some(m) = self.session.measurement {
                self.report.push(format!(
                    "PSNR {:.2} dB, SSIM {:.4}, FLIP {:.4}",
                    m.psnr, m.ssim, m.flip
                ));
            }
            if compare {
                let state = host.state().clone();
                let status = match (self.session.measurement.is_some(), self.stale(&state)) {
                    (false, _) => "no measurement",
                    (true, false) => "measurement current",
                    (true, true) => "measurement stale",
                };
                self.report.push(status.into());
            }
            return Ok(Snapshot::Done(self.report.join("; ")));
        }
        if compare && step == 15 {
            self.measure(host);
        }
        if stale_page
            && step == 16
            && let Some(e) = ranged_entry()
        {
            // The moved value goes back to its default: the live state is no longer the one measured.
            let default = host.default_state(host.state().demo).p(e.key);
            host.set_value(e.key, default);
            self.report.push(format!("{} back to {default}", e.key));
        }
        Ok(Snapshot::Settle)
    }

    /// `viewer-experiments-session`, from the carry off as Explore opens the viewer: a value moved and another
    /// demonstration selected (step 0); what that kept of the value, then the carry on, the value moved again,
    /// and another demonstration selected (step 1); what that kept (step 2); settled until step 8.
    fn session_step(&mut self, step: usize, host: &mut dyn Host) -> Snapshot {
        match step {
            0 | 1 => {
                // Each switch starts from a moved value; the first with the carry off, the second with it on.
                if step == 0 {
                    let defaults = host.default_state(host.state().demo);
                    *host.state_mut() = defaults;
                    self.session.carry = Some(false);
                } else {
                    self.report_carried(host);
                    self.session.carry = Some(true);
                }
                let demo = host.state().demo;
                let other = investigation::current()
                    .demos()
                    .iter()
                    .copied()
                    .find(|d| *d != demo);
                let (Some((key, now, to)), Some(other)) = (move_far(host), other) else {
                    return Snapshot::Done("needs a ranged value and two demonstrations".into());
                };
                self.report.push(format!(
                    "{key} {now} → {to}, {} → {}",
                    demo.key(),
                    other.key()
                ));
                host.act(ViewerAction::SelectDemo(other));
                Snapshot::Settle
            }
            2 => {
                self.report_carried(host);
                Snapshot::Settle
            }
            8.. => Snapshot::Done(self.report.join("; ")),
            _ => Snapshot::Settle,
        }
    }

    /// `viewer-experiments-overwrite`: an experiment saved under the page's own name, a value moved, and saved
    /// again under it, which asks to replace the file and leaves it as it was (step 0); Home, which asks to
    /// discard the unsaved change (step 2); both questions shown, then Replace, which writes the file, and the
    /// file removed (step 8). A file of that name the page did not write is left alone.
    fn overwrite_step(&mut self, step: usize, host: &mut dyn Host) -> Result<Snapshot> {
        const NAME: &str = "render-lab-snapshot-overwrite";
        let path = crate::experiment::user_file(NAME)?;
        let read = || std::fs::read_to_string(&path).unwrap_or_default();
        match step {
            0 => {
                if path.exists() {
                    return Ok(Snapshot::Done(format!(
                        "{} exists: left alone, nothing checked",
                        path.display()
                    )));
                }
                let defaults = host.default_state(host.state().demo);
                *host.state_mut() = defaults;
                self.save(NAME.into(), false, host);
                let first = read();
                move_far(host);
                self.save(NAME.into(), false, host);
                let asked = matches!(&self.pending, Some(Pending::Replace(n)) if n == NAME);
                self.report.push(format!(
                    "saved; saved again: {}, file {}",
                    if asked {
                        "asked to replace"
                    } else {
                        "not asked"
                    },
                    if read() == first {
                        "unchanged"
                    } else {
                        "changed"
                    }
                ));
                self.report_page = first;
            }
            2 => {
                host.act(ViewerAction::Home);
                self.report.push(format!(
                    "Home: {}",
                    if self.unsaved {
                        "asked to discard"
                    } else {
                        "nothing unsaved"
                    }
                ));
            }
            8 => {
                // The questions are on the last frame drawn; Replace now writes the file, which then goes.
                self.save(NAME.into(), true, host);
                let replaced = read() != self.report_page;
                std::fs::remove_file(&path).ok();
                self.report.push(format!(
                    "Replace: file {}; removed",
                    if replaced {
                        "written"
                    } else {
                        "unchanged"
                    }
                ));
                return Ok(Snapshot::Done(self.report.join("; ")));
            }
            _ => {}
        }
        Ok(Snapshot::Settle)
    }

    /// For the session page's report: whether the carry is on, and whether the moved value kept its value
    /// through the switch or is at the new demonstration's default.
    fn report_carried(&mut self, host: &mut dyn Host) {
        let Some(e) = ranged_entry() else {
            return;
        };
        let now = host.state().p(e.key);
        let default = host.default_state(host.state().demo).p(e.key);
        let carry = if self.carries() {
            "on"
        } else {
            "off"
        };
        let kept = if now == default {
            "its default"
        } else {
            "carried"
        };
        self.report
            .push(format!("carry {carry}: {} = {now} ({kept})", e.key));
    }
}

/// The value the pages move: an engine setting with a range, else any ranged value of the dock (no camera
/// value, no choice, no vector).
fn ranged_entry() -> Option<&'static Entry> {
    let entries = values::list();
    let camera = crate::camera::OrbitCamera::schema();
    let ranged = |e: &&'static Entry| {
        e.range.is_some()
            && e.components.is_empty()
            && e.choices.is_none()
            && !camera.iter().any(|c| std::ptr::eq(c, *e))
    };
    entries
        .iter()
        .copied()
        .find(|e| e.engine.is_some() && ranged(e))
        .or_else(|| {
            entries
                .iter()
                .copied()
                .find(|e| ranged(e) && !e.labels.is_empty())
        })
}

/// The pages' value moved to whichever end of its range is farther from where it is: its key, from, and to.
fn move_far(host: &mut dyn Host) -> Option<(&'static str, f32, f32)> {
    let e = ranged_entry()?;
    let (lo, hi) = e.range.expect("ranged");
    let now = host.state().p(e.key);
    let to = if (now - lo).abs() > (now - hi).abs() {
        lo
    } else {
        hi
    };
    host.set_value(e.key, to);
    Some((e.key, now, to))
}
