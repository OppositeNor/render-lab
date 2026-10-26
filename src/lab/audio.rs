//! The viewer's narration track: an `ffplay` child process started at the
//! timeline's position and stopped when the viewer pauses, seeks, or quits.
//! The lab has no audio stack of its own; FFplay ships with the video
//! build's Conda environment (or `FFPLAY` / PATH).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub struct AudioPlayer {
    ffplay: PathBuf,
    track: PathBuf,
    child: Option<Child>,
}

impl AudioPlayer {
    /// A player for `track`, or None when the file is missing.
    pub fn new(track: PathBuf, manifest_dir: &Path) -> Option<AudioPlayer> {
        if !track.is_file() {
            return None;
        }
        let ffplay = std::env::var_os("FFPLAY").map(PathBuf::from).unwrap_or_else(|| {
            let local = manifest_dir.join("video/.conda-env/bin/ffplay");
            if local.is_file() { local } else { PathBuf::from("ffplay") }
        });
        Some(AudioPlayer { ffplay, track, child: None })
    }

    pub fn track(&self) -> &Path {
        &self.track
    }

    /// Play from `seconds` (restarting if already playing). Returns false if
    /// FFplay could not be started.
    pub fn play_from(&mut self, seconds: f64) -> bool {
        self.stop();
        let child = Command::new(&self.ffplay)
            .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "-ss"])
            .arg(format!("{:.3}", seconds.max(0.0)))
            .arg(&self.track)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match child {
            Ok(c) => {
                self.child = Some(c);
                true
            }
            Err(_) => false,
        }
    }

    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
