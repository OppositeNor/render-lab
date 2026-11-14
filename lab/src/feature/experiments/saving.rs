//! Saving and loading the experiment: a file of the user's replaced only when asked, unsaved changes
//! discarded only when asked, and whether the session holds changes no experiment file has.

use super::Experiments;
use crate::experiment::{Compare, Preset, Source};
use crate::feature::Host;
use crate::i18n::{args, tr_with};
use crate::shot::ShotState;
use anyhow::Result;

/// A question the tools ask before they act.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Pending {
    /// Saving under this name would replace the user's file of that name.
    Replace(String),
    /// Loading the listed experiment would discard unsaved changes.
    Load(usize),
}

/// The experiment as last saved or loaded: its name, and the live state and reference it held.
pub(super) struct Saved {
    pub(super) name: String,
    live: ShotState,
    reference: Option<ShotState>,
}

impl Experiments {
    /// Save the experiment under `name` among the user's; a file of that name is replaced only with `replace`,
    /// else the tools ask first.
    pub(super) fn save(&mut self, name: String, replace: bool, host: &mut dyn Host) {
        if !replace && crate::experiment::user_file(&name).is_ok_and(|p| p.exists()) {
            self.pending = Some(Pending::Replace(name));
            return;
        }
        self.pending = None;
        let defaults = host.default_state(host.state().demo);
        let s = &mut self.session;
        let experiment = crate::experiment::Experiment::capture(
            &name,
            host.state(),
            &defaults,
            s.reference.as_ref(),
        );
        s.message = Some(match crate::experiment::save_user(&experiment) {
            Ok(path) => {
                // A shipped experiment of the same name stays: the user's is listed beside it.
                let path = path.display().to_string();
                let shipped = s
                    .presets
                    .iter()
                    .any(|p| p.name == name && matches!(p.source, Source::Shipped(_)));
                let message = if shipped {
                    tr_with(
                        "experiments-saved_beside_shipped",
                        args!(path = path, name = name.clone()),
                    )
                } else {
                    tr_with("experiments-saved", args!(path = path))
                };
                self.saved = Some(Saved {
                    name,
                    live: host.state().clone(),
                    reference: s.reference.clone(),
                });
                message
            }
            Err(e) => format!("{e:#}"),
        });
        s.presets = crate::experiment::presets();
    }

    /// Load the listed experiment `i`; unsaved changes are discarded only with `discard`, else the tools ask
    /// first.
    pub(super) fn load_preset(&mut self, i: usize, discard: bool, host: &mut dyn Host) {
        if !discard && self.unsaved {
            self.pending = Some(Pending::Load(i));
            return;
        }
        self.pending = None;
        let Some(preset) = self.session.presets.get(i).cloned() else {
            return;
        };
        let message = match self.load(&preset, host) {
            Ok(()) => {
                self.saved = Some(Saved {
                    name: preset.name.clone(),
                    live: host.state().clone(),
                    reference: self.session.reference.clone(),
                });
                tr_with("experiments-loaded", args!(name = preset.name.clone()))
            }
            Err(e) => format!("{e:#}"),
        };
        self.session.message = Some(message);
    }

    /// Open an experiment file: its demonstration, camera, and base values,
    /// and its reference (with a comparison) when it has one.
    fn load(&mut self, preset: &Preset, host: &mut dyn Host) -> Result<()> {
        let experiment = preset.load()?;
        experiment.check()?;
        let demo = experiment.start_demo()?;
        let mut state = host.default_state(demo);
        experiment.apply(&mut state, &experiment.base)?;
        self.session.reference = match experiment.reference_values() {
            Some(values) => {
                let mut reference = state.clone();
                experiment.apply(&mut reference, &values)?;
                Some(reference)
            }
            None => None,
        };
        if self.session.reference.is_some() && self.session.compare == Compare::Off {
            self.session.compare = Compare::Wipe;
        }
        self.session.measurement = None;
        host.set_state(state);
        Ok(())
    }

    /// Once a frame, outside a playing timeline: whether the session holds changes no file has: its
    /// demonstration, values, or reference differ from the experiment last saved or loaded, else (once the tab
    /// was used) its values from the defaults or a reference is set. The camera is not counted.
    pub(super) fn track_unsaved(&mut self, host: &mut dyn Host) {
        if !host.interactive() {
            return;
        }
        let demo = host.state().demo;
        if self.defaults.as_ref().is_none_or(|d| d.demo != demo) {
            self.defaults = Some(host.default_state(demo));
        }
        let live = host.state();
        let reference = self.session.reference.as_ref().map(|r| &r.values);
        self.unsaved = match (&self.saved, &self.defaults) {
            (Some(saved), _) => {
                saved.live.demo != live.demo
                    || saved.live.values != live.values
                    || saved.reference.as_ref().map(|r| &r.values) != reference
            }
            (None, Some(defaults)) => {
                self.used && (live.values != defaults.values || reference.is_some())
            }
            (None, None) => false,
        };
    }
}
