//! The homepage's narration download: an investigation with no narrated video whose package says where its narration
//! package is published (`narration-url`) gets a Download button where Watch was (src/install.rs; the address is
//! confirmed first, src/download.rs). Once installed, the button restarts the lab, which reads its packages at start.

use super::*;
use crate::install::{Install, Status};

/// What the selection page's first button is for the chosen investigation.
pub(super) enum WatchButton {
    /// Watch its narrated video (enabled when it has one).
    Watch,
    /// Download its narration package from this address (its manifest's `narration-url`).
    Download(&'static str),
    /// A download under way: its label, the button disabled.
    Busy(String),
    /// Installed: restart to watch.
    Restart,
}

impl Home {
    /// The first button for `chosen`, `playable` when it has a narrated video to watch.
    pub(super) fn watch_button(
        &self,
        chosen: &'static dyn Investigation,
        playable: bool,
    ) -> WatchButton {
        let Some(package) = chosen.package().filter(|_| !playable) else {
            return WatchButton::Watch;
        };
        if let Some(install) = self.install.as_ref() {
            // One download at a time: another investigation's leaves this one's button disabled.
            if install.package != package {
                return WatchButton::Busy(tr("home-download_narration").into());
            }
            return match install.status() {
                Status::Installed => WatchButton::Restart,
                Status::Downloading(bytes) => WatchButton::Busy(tr_with(
                    "home-downloading",
                    args!(megabytes = format!("{:.0}", bytes as f64 / 1e6)),
                )),
                Status::Unpacking => WatchButton::Busy(tr("home-unpacking").into()),
                _ => WatchButton::Busy(tr("home-download_narration").into()),
            };
        }
        // A checkout builds its own narration packages: nothing is downloaded beside them.
        let url = resources::packages()
            .iter()
            .find(|p| p.name == package)
            .and_then(|p| p.narration_url.as_deref());
        match url {
            Some(url) if resources::sources_dir().is_none() => WatchButton::Download(url),
            _ => WatchButton::Watch,
        }
    }

    /// Asks to download `package`'s narration package from `url`: the confirmation shows next.
    pub(super) fn start_install(&mut self, package: &str, url: &str) {
        match Install::new(package, url, &resources::packages_dir()) {
            Ok(install) => self.install = Some(install),
            Err(e) => self.popups.push(
                Popup::new(tr("home-download_failed"), format!("{e:#}"))
                    .button(tr("popup-close"), popup::Action::Close),
            ),
        }
    }

    /// Each frame: the installation moved on, its confirmation drawn while it waits, and a failure shown.
    pub(super) fn draw_install(&mut self, ctx: &egui::Context, canvas: Vec2) {
        let Some(install) = self.install.as_mut() else {
            return;
        };
        install.poll(&resources::packages_dir());
        install.download.draw_confirmation(ctx, canvas);
        match install.status() {
            Status::Cancelled => self.install = None,
            Status::Failed(e) => {
                self.install = None;
                self.popups.push(
                    Popup::new(tr("home-download_failed"), e)
                        .button(tr("popup-close"), popup::Action::Close),
                );
            }
            _ => {}
        }
    }
}

/// Starts the lab again as it was started (its arguments), for the packages installed since it started.
pub(super) fn restart() {
    let started = std::env::current_exe().and_then(|exe| {
        std::process::Command::new(exe)
            .args(std::env::args_os().skip(1))
            .spawn()
    });
    if let Err(e) = started {
        eprintln!("render-lab: could not start again: {e}");
    }
}
