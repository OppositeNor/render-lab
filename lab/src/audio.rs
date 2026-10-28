//! Sound through an `ffplay` child process: the viewer's narration track,
//! started at the timeline's position and stopped when the viewer pauses,
//! seeks, or quits, and the homepage's music, looping while it is shown.
//! Each plays at a gain (the user's `Volume`, src/volume.rs); FFplay takes
//! it when it starts, so a player restarts to take a new one.
//! The lab has no audio stack of its own; FFplay ships with the video
//! build's Conda environment (or `FFPLAY` / PATH).

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

pub struct AudioPlayer {
    ffplay: PathBuf,
    track: PathBuf,
    /// Loop the track for ever, fading in over these seconds.
    looping: Option<f32>,
    /// The factor the sound is scaled by (0 to 1); at 0 nothing is started.
    gain: f32,
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
        Some(AudioPlayer { ffplay, track, looping: None, gain: 1.0, child: None })
    }

    /// Loop the track for ever, fading in over `fade` seconds.
    pub fn looping(mut self, fade: f32) -> AudioPlayer {
        self.looping = Some(fade);
        self
    }

    /// The gain of the next `play_from`; playing on takes it only when restarted.
    pub fn set_gain(&mut self, gain: f32) {
        self.gain = gain.clamp(0.0, 1.0);
    }

    pub fn track(&self) -> &Path {
        &self.track
    }

    /// Play from `seconds` (restarting if already playing). Returns false if
    /// FFplay could not be started, or nothing would be heard (a gain of 0).
    pub fn play_from(&mut self, seconds: f64) -> bool {
        self.stop();
        if self.gain <= 0.0 {
            return false;
        }
        let mut filter = format!("volume={:.2}", self.gain);
        let mut options = Vec::new();
        if let Some(fade) = self.looping {
            options.extend(["-loop".to_string(), "0".to_string()]);
            filter.push_str(&format!(",afade=t=in:d={fade:.1}"));
        }
        options.extend(["-af".to_string(), filter]);
        let child = Command::new(&self.ffplay)
            .args(["-nodisp", "-autoexit", "-loglevel", "quiet", "-ss"])
            .arg(format!("{:.3}", seconds.max(0.0)))
            .args(&options)
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
