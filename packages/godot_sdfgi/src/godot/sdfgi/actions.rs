//! The investigation's actions (lab/src/actions.rs): work its story starts
//! with `{@ run=…}` and stops with `{@ stop=…}`, setting the shot's values
//! as the seconds pass.

use super::Sdfgi;
use lab::actions::ActionFrame;

/// The jump flood's last step (`floodStep`: the full-size pass).
const LAST_FLOOD_STEP: u32 = 8;

#[lab::actions]
impl Sdfgi {
    /// Steps the jump flood (`floodStep`) from `from` (the half-size seeds by
    /// default) to its full-size pass, one step every `seconds`.
    #[action]
    fn jump_flood(frame: &mut ActionFrame, seconds: f32, from: Option<u32>) -> Result<(), String> {
        if seconds <= 0.0 {
            return Err(format!("a step takes {seconds} s; it takes longer than 0"));
        }
        let step = (from.unwrap_or(0) + (frame.elapsed / seconds) as u32).min(LAST_FLOOD_STEP);
        frame.state.set("floodStep", step as f32);
        Ok(())
    }
}
