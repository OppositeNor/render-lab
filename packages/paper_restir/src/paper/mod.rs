//! Investigations of published papers: each implements a paper's method as the
//! paper describes it, in the lab's own code, and cites the paper by section,
//! page, and equation (its source index's `paper`). The PDF is wherever the user
//! keeps it (asked, or the package's variable).

pub mod restir;

use lab::investigation::Engine;

pub static PAPERS: Engine = Engine {
    id: "paper",
    name: "Papers",
    summary: "Methods from published papers, implemented as each paper describes them. Every claim cites a section, a page, and an equation or algorithm line.",
};
