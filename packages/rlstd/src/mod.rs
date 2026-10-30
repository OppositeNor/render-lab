//! The standard package's code: what investigations would otherwise each
//! write for themselves, used as it is. It is built into the lab, which
//! links it as `lab::utility` (lab/src/lib.rs), so it reaches the lab's types
//! and every package's library reaches it through the lab.

/// Arrows and labels a story declares, and the points and sets an investigation publishes for them.
pub mod annotations;
/// The Play animation of a demonstration, sampled.
pub mod demo;
/// Pictures in the viewport's corners.
pub mod inset;
/// A selftest's report, its single frames, and its test inputs.
pub mod selftest;
/// A package's source index, and its cited lines checked against the checkout.
pub mod sources;
/// A package's on-screen text (`text_table!`).
pub mod text;
