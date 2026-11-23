//! Downloads from the internet: the lab's only way to fetch from the network. A download starts as a request that
//! the user must confirm in a window naming what it is for, the host it comes from, and its whole address, so that a
//! package cannot make the lab fetch from an address nobody saw. Only `https` addresses are taken, with a plain host
//! (ASCII letters, digits, dots, and dashes: no user name, no look-alike letters). The transfer is `curl`'s (on
//! every platform the lab runs on; `RENDER_LAB_CURL` names another), kept to HTTPS through redirects and to the
//! request's size, written beside its target and moved there once complete.

use crate::i18n::tr;
use crate::palette;
use crate::text::bold;
use anyhow::{Result, bail};
use egui::{Align2, CornerRadius, FontFamily, FontId, Pos2, Rect, RichText, Stroke, Vec2, vec2};
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// What to download, why, and where to.
pub struct Request {
    pub url: String,
    /// What it is for, as the confirmation says it.
    pub purpose: String,
    /// The file it is written to once complete.
    pub file: PathBuf,
    /// The most it may be.
    pub max_bytes: u64,
}

/// Where a download is.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Waiting for the user to confirm it.
    Confirming,
    Running,
    Done,
    Cancelled,
    Failed(String),
}

pub struct Download {
    pub request: Request,
    host: String,
    state: State,
    child: Option<Child>,
}

/// The host of an `https` address, or why the address is not taken.
pub fn host(url: &str) -> Result<String> {
    let Some(rest) = url.strip_prefix("https://") else {
        bail!("only https addresses are downloaded");
    };
    if url
        .chars()
        .any(|c| c.is_control() || c.is_whitespace() || !c.is_ascii())
    {
        bail!("the address has spaces, control characters, or letters outside ASCII");
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        bail!("the address names a user before its host");
    }
    let (host, port) = authority.split_once(':').unwrap_or((authority, ""));
    let plain = |b: u8| b.is_ascii_alphanumeric() || b == b'.' || b == b'-';
    if host.is_empty() || host.starts_with(['.', '-']) || !host.bytes().all(plain) {
        bail!("`{host}` is not a host name");
    }
    if !port.bytes().all(|b| b.is_ascii_digit()) {
        bail!("`{port}` is not a port");
    }
    Ok(host.to_ascii_lowercase())
}

/// curl's arguments: fail on an HTTP error, follow redirects on HTTPS only, stop past `max_bytes`, write `partial`.
fn curl_args(url: &str, partial: &std::path::Path, max_bytes: u64) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--fail",
        "--location",
        "--proto",
        "=https",
        "--proto-redir",
        "=https",
        "--silent",
        "--show-error",
        "--max-filesize",
    ]
    .iter()
    .map(OsString::from)
    .collect();
    args.push(max_bytes.to_string().into());
    args.push("--output".into());
    args.push(partial.into());
    args.push("--url".into());
    args.push(url.into());
    args
}

impl Download {
    /// A request, waiting for its confirmation; an error when its address is not taken.
    pub fn new(request: Request) -> Result<Download> {
        let host = host(&request.url)?;
        Ok(Download {
            request,
            host,
            state: State::Confirming,
            child: None,
        })
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    /// The file being written until the download completes.
    fn partial(&self) -> PathBuf {
        let mut name = self.request.file.as_os_str().to_owned();
        name.push(".partial");
        PathBuf::from(name)
    }

    /// The bytes received so far.
    pub fn received(&self) -> u64 {
        std::fs::metadata(self.partial()).map_or(0, |m| m.len())
    }

    /// Confirmed: curl starts (the confirmation's Download; a snapshot's test of the whole download).
    pub fn start(&mut self) {
        let curl = std::env::var_os("RENDER_LAB_CURL").unwrap_or_else(|| "curl".into());
        let _ = std::fs::remove_file(self.partial());
        if let Some(parent) = self.request.file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut command = Command::new(&curl);
        command
            .args(curl_args(
                &self.request.url,
                &self.partial(),
                self.request.max_bytes,
            ))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            // No console window for curl beside the lab's (CREATE_NO_WINDOW).
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        match command.spawn() {
            Ok(child) => {
                self.child = Some(child);
                self.state = State::Running;
            }
            Err(e) => {
                self.state = State::Failed(format!("{} {e}", tr("download-no_curl")));
            }
        }
    }

    /// Stops it: curl is ended and what it wrote removed.
    pub fn cancel(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(self.partial());
        if matches!(self.state, State::Confirming | State::Running) {
            self.state = State::Cancelled;
        }
    }

    /// Checks on curl: once it ends, the download is done (its file moved into place) or failed (with curl's
    /// message).
    pub fn poll(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        let status = match child.try_wait() {
            Ok(None) => return,
            Ok(Some(status)) => status,
            Err(e) => {
                self.state = State::Failed(e.to_string());
                self.child = None;
                return;
            }
        };
        let mut message = String::new();
        if let Some(mut err) = child.stderr.take() {
            let _ = err.read_to_string(&mut message);
        }
        self.child = None;
        self.state = if !status.success() {
            let _ = std::fs::remove_file(self.partial());
            State::Failed(format!("curl: {} ({status})", message.trim()))
        } else {
            match std::fs::rename(self.partial(), &self.request.file) {
                Ok(()) => State::Done,
                Err(e) => State::Failed(e.to_string()),
            }
        };
    }

    /// While it waits for its confirmation: the window over everything that asks for it, with what it is for, its
    /// host, and its whole address. Download starts it; Cancel ends it.
    pub fn draw_confirmation(&mut self, ctx: &egui::Context, canvas: Vec2) {
        if self.state != State::Confirming {
            return;
        }
        let mut picked = None;
        egui::Area::new(egui::Id::unique("download-confirmation"))
            .order(egui::Order::Foreground)
            .fixed_pos(Pos2::ZERO)
            .fade_in(false)
            .constrain(false)
            .show(ctx, |area| {
                // A dimmed page that takes the pointer, so only the window can be used.
                let full = Rect::from_min_size(Pos2::ZERO, canvas);
                area.allocate_rect(full, egui::Sense::click_and_drag());
                area.painter().rect_filled(
                    full,
                    CornerRadius::ZERO,
                    palette::background().gamma_multiply(0.85),
                );
                let width = 1100.0f32.min(canvas.x - 120.0);
                egui::Area::new(egui::Id::unique("download-confirmation-card"))
                    .order(egui::Order::Tooltip)
                    .pivot(Align2::CENTER_CENTER)
                    .fixed_pos(Pos2::new(canvas.x / 2.0, canvas.y / 2.0))
                    .fade_in(false)
                    .constrain(false)
                    .show(area.ctx(), |ui| picked = self.card(ui, width));
            });
        match picked {
            Some(true) => self.start(),
            Some(false) => self.cancel(),
            None => {}
        }
    }

    /// The confirmation's card: Some(true) when Download is clicked, Some(false) for Cancel.
    fn card(&self, ui: &mut egui::Ui, width: f32) -> Option<bool> {
        let frame = egui::Frame::new()
            .fill(palette::panel())
            .stroke(Stroke::new(2.0, palette::WARNING))
            .corner_radius(CornerRadius::same(18))
            .inner_margin(egui::Margin::symmetric(40, 32));
        frame
            .show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 10.0;
                let text = |s: String, size: f32, color| {
                    RichText::new(s)
                        .font(FontId::new(size, FontFamily::Proportional))
                        .color(color)
                };
                ui.label(
                    RichText::new(tr("download-title"))
                        .font(FontId::new(38.0, bold()))
                        .color(palette::WARNING),
                );
                ui.add(
                    egui::Label::new(text(self.request.purpose.clone(), 28.0, palette::text()))
                        .wrap(),
                );
                ui.add_space(8.0);
                ui.label(text(
                    tr("download-from").to_string(),
                    24.0,
                    palette::text_dim(),
                ));
                ui.label(
                    RichText::new(&self.host)
                        .font(FontId::new(40.0, bold()))
                        .color(palette::text()),
                );
                ui.label(text(
                    tr("download-address").to_string(),
                    24.0,
                    palette::text_dim(),
                ));
                ui.add(
                    egui::Label::new(
                        RichText::new(&self.request.url)
                            .font(FontId::new(24.0, FontFamily::Monospace))
                            .color(palette::text()),
                    )
                    .wrap(),
                );
                ui.add_space(8.0);
                ui.add(
                    egui::Label::new(text(
                        tr("download-check").to_string(),
                        24.0,
                        palette::text_dim(),
                    ))
                    .wrap(),
                );
                ui.add_space(16.0);
                let mut picked = None;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 24.0;
                    let button = |label: String| {
                        egui::Button::new(RichText::new(label).font(FontId::new(28.0, bold())))
                            .min_size(vec2(260.0, 64.0))
                    };
                    if ui.add(button(tr("download-cancel").to_string())).clicked() {
                        picked = Some(false);
                    }
                    if ui
                        .add(
                            button(tr("download-confirm").to_string())
                                .stroke(Stroke::new(2.0, palette::WARNING)),
                        )
                        .clicked()
                    {
                        picked = Some(true);
                    }
                });
                picked
            })
            .inner
    }
}

impl Drop for Download {
    /// A download still running when its owner goes is cancelled.
    fn drop(&mut self) {
        if self.child.is_some() {
            self.cancel();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_https_hosts_are_taken() {
        assert_eq!(host("https://github.com/a/b.zip").unwrap(), "github.com");
        assert_eq!(
            host("https://Example.org:8443/x?y#z").unwrap(),
            "example.org"
        );
        for url in [
            "http://github.com/a",
            "file:///etc/passwd",
            "https://github.com@evil.example/a",
            "https://gіthub.com/a",
            "https://github.com/a b",
            "https://-x.com/",
            "https:///a",
            "https://h.com:80x/",
            " https://github.com/",
        ] {
            assert!(host(url).is_err(), "{url}");
        }
    }

    #[test]
    fn curl_stays_on_https_and_within_the_size() {
        let args = curl_args(
            "https://h.com/f.zip",
            std::path::Path::new("/tmp/f.partial"),
            99,
        );
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        let after = |flag: &str| args[args.iter().position(|a| a == flag).unwrap() + 1].clone();
        assert_eq!(
            (after("--proto"), after("--proto-redir")),
            ("=https".into(), "=https".into())
        );
        assert_eq!(
            (after("--max-filesize"), after("--url")),
            ("99".into(), "https://h.com/f.zip".into())
        );
    }

    #[test]
    fn nothing_starts_before_the_confirmation() {
        let request = Request {
            url: "https://h.com/f.zip".into(),
            purpose: String::new(),
            file: std::env::temp_dir().join("render-lab-download-test.zip"),
            max_bytes: 1,
        };
        let mut download = Download::new(request).unwrap();
        download.poll();
        assert_eq!(download.state(), &State::Confirming);
        download.cancel();
        assert_eq!(download.state(), &State::Cancelled);
    }
}
