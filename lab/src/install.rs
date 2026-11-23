//! Installing a package's narration package from the internet: the homepage offers it when an investigation has no
//! narrated video and its package names where one is published (`narration-url`, src/packages.rs). The archive is
//! downloaded through src/download.rs (so the user confirms its address first), unpacked into a folder of its own
//! (src/archive.rs), checked to be that package's narration package and nothing more
//! (`packages::check_narration`: no code, no shaders, no overrides), and only then moved into the packages folder.
//! The lab reads its packages when it starts, so the narration plays after a restart.

use crate::archive::{self, Limits};
use crate::download::{self, Download, Request};
use crate::i18n::{args, tr_with};
use crate::packages;
use anyhow::{Context, Result, anyhow, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

/// The most a narration package may be: its archive, and its files once unpacked.
const MAX_BYTES: u64 = 2 << 30;
const LIMITS: Limits = Limits {
    entries: 1000,
    bytes: MAX_BYTES,
};

/// Where an installation is, for the homepage to show.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Confirming,
    /// Bytes received so far.
    Downloading(u64),
    Unpacking,
    /// In the packages folder: it plays after a restart.
    Installed,
    Cancelled,
    Failed(String),
}

pub struct Install {
    /// The package whose narration it is.
    pub package: String,
    pub download: Download,
    unpacking: Option<JoinHandle<Result<()>>>,
    done: Option<Result<(), String>>,
}

/// The narration package of `package`.
pub fn narration_name(package: &str) -> String {
    format!("{package}_narration")
}

impl Install {
    /// The installation of `package`'s narration package from `url` (its manifest's `narration-url`), into
    /// `packages_dir`; it waits for the user to confirm the download.
    pub fn new(package: &str, url: &str, packages_dir: &Path) -> Result<Install> {
        let name = narration_name(package);
        let url = url.replace("{version}", env!("CARGO_PKG_VERSION"));
        let request = Request {
            purpose: tr_with("download-narration_purpose", args!(package = name.as_str())),
            file: packages_dir.join(".downloads").join(format!("{name}.zip")),
            url,
            max_bytes: MAX_BYTES,
        };
        Ok(Install {
            package: package.to_string(),
            download: Download::new(request)?,
            unpacking: None,
            done: None,
        })
    }

    /// Moves it on: a finished download is unpacked on a thread of its own, and a finished unpacking ends it.
    pub fn poll(&mut self, packages_dir: &Path) {
        self.download.poll();
        if self.download.state() == &download::State::Done
            && self.unpacking.is_none()
            && self.done.is_none()
        {
            let (zip, dir, package) = (
                self.download.request.file.clone(),
                packages_dir.to_path_buf(),
                self.package.clone(),
            );
            self.unpacking = Some(std::thread::spawn(move || install(&zip, &dir, &package)));
        }
        if self.unpacking.as_ref().is_some_and(JoinHandle::is_finished) {
            let joined = self.unpacking.take().expect("an unpacking").join();
            self.done = Some(match joined {
                Ok(result) => result.map_err(|e| format!("{e:#}")),
                Err(_) => Err("the unpacking stopped".into()),
            });
        }
    }

    pub fn status(&self) -> Status {
        if let Some(done) = &self.done {
            return done
                .clone()
                .map_or_else(Status::Failed, |()| Status::Installed);
        }
        match self.download.state() {
            download::State::Confirming => Status::Confirming,
            download::State::Running => Status::Downloading(self.download.received()),
            download::State::Done => Status::Unpacking,
            download::State::Cancelled => Status::Cancelled,
            download::State::Failed(e) => Status::Failed(e.clone()),
        }
    }
}

/// Unpacks the downloaded `zip` beside the packages, checks it is `package`'s narration package, and moves it into
/// `packages_dir` (replacing one installed the same way before; a package the checkout builds is left alone).
pub fn install(zip: &Path, packages_dir: &Path, package: &str) -> Result<()> {
    let name = narration_name(package);
    let staging = zip.with_extension("unpacked");
    let _ = fs::remove_dir_all(&staging);
    let result = (|| -> Result<()> {
        archive::unpack(zip, &staging, LIMITS)?;
        // Exactly one folder, the package.
        let top: Vec<PathBuf> = fs::read_dir(&staging)?
            .map(|e| e.map(|e| e.path()))
            .collect::<Result<_, _>>()?;
        if top.len() != 1
            || top[0].file_name().and_then(|n| n.to_str()) != Some(name.as_str())
            || !top[0].is_dir()
        {
            bail!("the archive does not hold the one folder `{name}`");
        }
        packages::check_narration(&top[0], package).map_err(|e| anyhow!(e))?;
        let target = packages_dir.join(&name);
        if target.exists() {
            if fs::read_to_string(target.join("manifest.toml"))
                .is_ok_and(|m| m.contains("\ncooked-from"))
            {
                bail!(
                    "{} is built from a source checkout; it is not replaced",
                    target.display()
                );
            }
            fs::remove_dir_all(&target).with_context(|| format!("remove {}", target.display()))?;
        }
        fs::rename(&top[0], &target).with_context(|| format!("move it to {}", target.display()))
    })();
    let _ = fs::remove_dir_all(&staging);
    if result.is_ok() {
        let _ = fs::remove_file(zip);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narration_url_takes_the_version() {
        let dir = std::env::temp_dir();
        let install = Install::new(
            "demo",
            "https://h.com/v{version}/demo_narration-v{version}.zip",
            &dir,
        )
        .unwrap();
        let version = env!("CARGO_PKG_VERSION");
        assert_eq!(
            install.download.request.url,
            format!("https://h.com/v{version}/demo_narration-v{version}.zip")
        );
        assert_eq!(install.status(), Status::Confirming);
        assert!(Install::new("demo", "http://h.com/x.zip", &dir).is_err());
    }
}
