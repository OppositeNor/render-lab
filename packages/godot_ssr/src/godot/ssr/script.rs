//! What the investigation's scripts (script/*.lam, lab/src/script/runtime.rs) read from its code: the thread
//! group a demonstration draws, so a click on its inset picks the thread under it as P.
//!
//! | Native | Returns |
//! |---|---|
//! | `laneGroup` | P's 8 x 8 group in this frame: its first pixel `{x, y}` and the SSR size `{w, h}` |

use lab::script::lamlet::embed::{Error, NativeRegistry};
use lab::script::runtime::frame_data;

lab::lamlet_record! {
    /// P's group of threads: its first SSR pixel, and the SSR buffer's size it is a pixel of.
    pub struct LaneGroup {
        pub x: i64 = "x",
        pub y: i64 = "y",
        pub w: i64 = "w",
        pub h: i64 = "h",
    }
}

/// Registers the natives (`Investigation::script_natives`).
pub fn register(natives: &mut NativeRegistry) -> Result<(), Error> {
    natives
        .function("godot_ssr/lane-group", |_, ()| {
            let g = frame_data::<LaneGroup>()?;
            Ok(LaneGroup { x: g.x, y: g.y, w: g.w, h: g.h })
        })
        .doc("Fn([], {h: Int, w: Int, x: Int, y: Int}). P's group of eight by eight threads in the frame clicked: its first SSR pixel and the SSR size.")
        .register()?;
    Ok(())
}
