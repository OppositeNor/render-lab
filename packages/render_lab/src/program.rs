//! A GPU program's commands, as an investigation's plan writes them
//! (`ProgramCommand`, lab/src/investigation.rs).

use crate::investigation::ProgramCommand;

/// A dispatch's or a draw's parameters: `words`, then zeros.
pub fn params(words: &[u32]) -> [u32; 16] {
    let mut p = [0u32; 16];
    p[..words.len()].copy_from_slice(words);
    p
}

/// A dispatch of `groups` workgroups of `pass` (its `instance`), with `words` as its parameters.
pub fn dispatch(
    pass: &'static str,
    instance: u32,
    groups: [u32; 3],
    words: &[u32],
) -> ProgramCommand {
    ProgramCommand::Dispatch {
        pass,
        instance,
        groups,
        params: params(words),
    }
}

/// The workgroups that cover an image of `size` with square groups of `side` threads a side.
pub fn groups(size: [u32; 2], side: u32) -> [u32; 3] {
    [size[0].div_ceil(side), size[1].div_ceil(side), 1]
}
