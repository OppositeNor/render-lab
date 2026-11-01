//! Python pseudo-code of an engine's shading logic, shown in the struct panel's
//! place (PanelMode::Code) and listed in the script and the source guide.
//!
//! An investigation embeds its listings and parses them with `Listing::parse`. Each listing starts with header
//! comments, `# title: English | 中文`, `# source: <source-index key>`
//! (the engine function it follows), `# demo: <demo key>`, and optionally
//! `# dispatch: English | 中文` (how its first function runs, e.g. "for each
//! probe, in parallel", which the block style's first hat shows in place of
//! the `def`). A line may end in tags,
//! `# @name ...`; a shot's `highlight` names the tags whose lines are lit. A
//! line may also carry a mark of what it does, `# @@read`, `@@write`, or
//! `@@math` (or `@@control`, `@@statement`), which colours its block. Tags
//! and marks are not shown.
//!
//! The panel draws a listing in one of two styles (`Style`): as text, or as
//! Scratch-like blocks (`blocks`), statements stacked in C-shaped loops and
//! branches under a hat.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// How the code panel draws a listing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Style {
    /// Python text, indented.
    Text,
    /// Scratch-like blocks around the same text: nesting is drawn, not
    /// indented, so lines get more of the panel's width.
    #[default]
    Blocks,
}

impl Style {
    pub const ALL: [Style; 2] = [Style::Text, Style::Blocks];

    /// Its name in the settings file and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Style::Text => "text",
            Style::Blocks => "blocks",
        }
    }

    pub fn from_key(key: &str) -> Option<Style> {
        Style::ALL.into_iter().find(|s| s.key() == key)
    }
}

// The style in use: blocks (the default) until the settings set it, so
// recordings never depend on them; a command line's `--listing-style` fixes
// it for the run.
static STYLE: AtomicU8 = AtomicU8::new(Style::Blocks as u8);
static FORCED: AtomicBool = AtomicBool::new(false);

/// The style the code panel draws listings in.
pub fn style() -> Style {
    if STYLE.load(Ordering::Relaxed) == Style::Text as u8 { Style::Text } else { Style::Blocks }
}

/// Use `style` from the next frame on, unless the command line fixed one.
pub fn set_style(style: Style) {
    if !FORCED.load(Ordering::Relaxed) {
        STYLE.store(style as u8, Ordering::Relaxed);
    }
}

/// Use `style` for the rest of the run, whatever the settings say.
pub fn force_style(style: Style) {
    STYLE.store(style as u8, Ordering::Relaxed);
    FORCED.store(true, Ordering::Relaxed);
}

/// Run `f` with `style` in use, then put back the style before: a
/// self-test's check that a listing fits in each style.
pub fn with_style<T>(style: Style, f: impl FnOnce() -> T) -> T {
    let before = STYLE.swap(style as u8, Ordering::Relaxed);
    let out = f();
    STYLE.store(before, Ordering::Relaxed);
    out
}

/// What a line does, which colours its block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Loops, branches, and leaving (`for`, `if`, `return`).
    Control,
    /// Reads memory: a texture, a buffer.
    Read,
    /// Writes memory.
    Write,
    /// Arithmetic.
    Math,
    /// Anything else.
    Statement,
}

impl Kind {
    pub fn from_mark(mark: &str) -> Option<Kind> {
        Some(match mark {
            "control" => Kind::Control,
            "read" => Kind::Read,
            "write" => Kind::Write,
            "math" => Kind::Math,
            "statement" => Kind::Statement,
            _ => return None,
        })
    }
}

const CONTROL_WORDS: &[&str] = &["for", "while", "if", "elif", "else", "return", "break", "continue", "raise", "pass"];

pub struct Line {
    pub text: String,
    pub tags: Vec<String>,
    /// Its `# @@kind` mark.
    pub mark: Option<Kind>,
}

impl Line {
    /// Its first word.
    fn word(&self) -> &str {
        let t = self.text.trim_start();
        let end = t.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(t.len());
        &t[..end]
    }

    /// What it does: its mark, else control for a line that starts with a
    /// control word, else a statement.
    pub fn kind(&self) -> Kind {
        self.mark.unwrap_or(if CONTROL_WORDS.contains(&self.word()) { Kind::Control } else { Kind::Statement })
    }

    fn indent(&self) -> usize {
        self.text.chars().take_while(|c| *c == ' ').count()
    }
}

pub struct Listing {
    pub name: &'static str,
    title: String,
    title_zh: Option<String>,
    dispatch: Option<(String, Option<String>)>,
    pub source: String,
    pub demo: Option<String>,
    pub lines: Vec<Line>,
}

/// `English | 中文` as its two parts.
fn bilingual(value: &str) -> (String, Option<String>) {
    let (en, zh) = value.split_once(" | ").map_or((value, None), |(a, b)| (a, Some(b)));
    (en.trim().to_string(), zh.map(|z| z.trim().to_string()))
}

impl Listing {
    pub fn title(&self) -> &str {
        match (&self.title_zh, crate::i18n::zh()) {
            (Some(zh), true) => zh,
            _ => &self.title,
        }
    }

    /// How the GPU runs it (`# dispatch:`), in the language shown.
    pub fn dispatch(&self) -> Option<&str> {
        self.dispatch.as_ref().map(|(en, zh)| match (zh, crate::i18n::zh()) {
            (Some(zh), true) => zh.as_str(),
            _ => en.as_str(),
        })
    }

    pub fn lit(&self, line: &Line, highlight: &[String]) -> bool {
        line.tags.iter().any(|t| highlight.contains(t))
    }

    pub fn parse(name: &'static str, text: &str) -> Listing {
        let mut listing = Listing { name, title: name.to_string(), title_zh: None, dispatch: None, source: String::new(), demo: None, lines: Vec::new() };
        let mut header = true;
        for raw in text.lines() {
            if header {
                if let Some((key, value)) = raw.strip_prefix("# ").and_then(|r| r.split_once(": ")) {
                    match key {
                        "title" => {
                            (listing.title, listing.title_zh) = bilingual(value);
                            continue;
                        }
                        "source" => {
                            listing.source = value.trim().to_string();
                            continue;
                        }
                        "demo" => {
                            listing.demo = Some(value.trim().to_string());
                            continue;
                        }
                        "dispatch" => {
                            listing.dispatch = Some(bilingual(value));
                            continue;
                        }
                        _ => {}
                    }
                }
                header = false;
            }
            // Trailers of tags and marks, `# @a @b  # @@read`, from the end.
            let (mut code, mut tags, mut mark) = (raw.trim_end(), Vec::new(), None);
            while let Some(at) = code.rfind("# @") {
                for word in code[at + 2..].split_whitespace() {
                    if let Some(m) = word.strip_prefix("@@") {
                        match Kind::from_mark(m) {
                            Some(kind) => mark = Some(kind),
                            None => eprintln!("listing {name}: unknown mark `@@{m}` (read, write, math, control, statement)"),
                        }
                    } else if let Some(tag) = word.strip_prefix('@') {
                        tags.insert(0, tag.to_string());
                    }
                }
                code = code[..at].trim_end();
            }
            listing.lines.push(Line { text: code.to_string(), tags, mark });
        }
        while listing.lines.last().is_some_and(|l| l.text.is_empty()) {
            listing.lines.pop();
        }
        listing
    }
}

/// A block of the block style: lines of the listing, by index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// A function's `def`: the hat its body stacks under.
    Hat { lines: Range<usize> },
    /// A statement and the lines it continues on.
    Stmt { lines: Range<usize> },
    /// A loop or a branch: each arm's head (`if`, `elif`, `else`) and its body.
    C { arms: Vec<Arm> },
    /// A statement nothing follows: `return`, `break`, `continue`.
    Cap { lines: Range<usize> },
    /// A line of only a comment.
    Note { line: usize },
    /// A blank line.
    Gap,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arm {
    pub head: Range<usize>,
    pub body: Vec<Block>,
}

/// The code of a line without its comment.
fn code_part(text: &str) -> &str {
    let mut quote = None;
    for (i, c) in text.char_indices() {
        match (quote, c) {
            (None, '#') => return &text[..i],
            (None, '\'' | '"') => quote = Some(c),
            (Some(q), _) if c == q => quote = None,
            _ => {}
        }
    }
    text
}

/// One logical line: its lines, its indentation, and its code joined.
struct Logical {
    lines: Range<usize>,
    indent: usize,
    code: String,
    blank: bool,
}

fn logical_lines(listing: &Listing) -> Vec<Logical> {
    let mut out: Vec<Logical> = Vec::new();
    let mut open = 0i32;
    let mut continued = false;
    for (i, line) in listing.lines.iter().enumerate() {
        let blank = line.text.trim().is_empty();
        let code = code_part(&line.text).trim_end();
        // A line inside brackets, after a backslash, or indented off the
        // four-space grid continues the one before.
        let joins = !blank && (open > 0 || continued || line.indent() % 4 != 0) && out.last().is_some_and(|l| !l.blank);
        if joins {
            let last = out.last_mut().unwrap();
            last.lines.end = i + 1;
            last.code.push(' ');
            last.code.push_str(code.trim());
        } else {
            out.push(Logical { lines: i..i + 1, indent: line.indent(), code: code.trim().to_string(), blank });
            open = 0;
        }
        for c in code.chars() {
            match c {
                '(' | '[' | '{' => open += 1,
                ')' | ']' | '}' => open -= 1,
                _ => {}
            }
        }
        continued = code.ends_with('\\');
    }
    out
}

/// A listing as blocks: its statements, loops, and branches, nested by indentation.
pub fn blocks(listing: &Listing) -> Vec<Block> {
    let logical = logical_lines(listing);
    let mut i = 0;
    let mut out = Vec::new();
    // Whatever is left (a line indented less than the first) starts a new run.
    while i < logical.len() {
        let before = i;
        out.extend(build(listing, &logical, &mut i, 0));
        if i == before {
            i += 1;
        }
    }
    out
}

/// Blocks from `logical[*i]` on, while lines are indented at least `min`.
fn build(listing: &Listing, logical: &[Logical], i: &mut usize, min: usize) -> Vec<Block> {
    let mut out = Vec::new();
    while *i < logical.len() {
        let l = &logical[*i];
        if l.blank {
            // A blank line belongs to whatever follows it.
            let next = logical[*i..].iter().find(|l| !l.blank);
            if next.is_some_and(|n| n.indent < min) {
                break;
            }
            out.push(Block::Gap);
            *i += 1;
            continue;
        }
        if l.indent < min {
            break;
        }
        let word = listing.lines[l.lines.start].word();
        if l.code.is_empty() {
            out.push(Block::Note { line: l.lines.start });
            *i += 1;
        } else if word == "def" || word == "class" {
            out.push(Block::Hat { lines: l.lines.clone() });
            *i += 1;
        } else if l.code.ends_with(':') && matches!(word, "for" | "while" | "if" | "with" | "try") {
            let indent = l.indent;
            let mut arms = Vec::new();
            let mut head = l.lines.clone();
            loop {
                *i += 1;
                let body = build(listing, logical, i, indent + 1);
                arms.push(Arm { head, body });
                // Its next arm: `elif`, `else`, `except`, at its indentation.
                match logical.get(*i) {
                    Some(n) if !n.blank && n.indent == indent && n.code.ends_with(':') && matches!(listing.lines[n.lines.start].word(), "elif" | "else" | "except" | "finally") => head = n.lines.clone(),
                    _ => break,
                }
            }
            out.push(Block::C { arms });
        } else if matches!(word, "return" | "break" | "continue" | "raise") {
            out.push(Block::Cap { lines: l.lines.clone() });
            *i += 1;
        } else {
            out.push(Block::Stmt { lines: l.lines.clone() });
            *i += 1;
        }
    }
    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token {
    Plain,
    Keyword,
    Name,
    Number,
    Text,
    Comment,
}

/// Python tokens of one line, for colouring.
pub fn tokens(line: &str) -> Vec<(Token, &str)> {
    const KEYWORDS: &[&str] = &["def", "return", "for", "in", "if", "elif", "else", "and", "or", "not", "while", "break", "continue", "None", "True", "False", "lambda", "import", "from", "with", "as", "pass", "is", "class"];
    let mut out = Vec::new();
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut after_def = false;
    while i < bytes.len() {
        let start = i;
        let c = line[i..].chars().next().unwrap_or(' ');
        let kind = if c == '#' {
            i = bytes.len();
            Token::Comment
        } else if c == '\'' || c == '"' {
            // Strings in these listings are ASCII.
            i += 1;
            while i < bytes.len() && bytes[i] != c as u8 {
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            Token::Text
        } else if c.is_ascii_digit() {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'.' || bytes[i] == b'_') {
                i += 1;
            }
            Token::Number
        } else if c.is_ascii_alphabetic() || c == '_' {
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &line[start..i];
            if KEYWORDS.contains(&word) {
                after_def = word == "def" || word == "class";
                out.push((Token::Keyword, word));
                continue;
            }
            if after_def {
                after_def = false;
                Token::Name
            } else {
                Token::Plain
            }
        } else {
            i += c.len_utf8();
            Token::Plain
        };
        out.push((kind, &line[start..i]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = "# title: Update | 更新\n# source: a\n# demo: b\n# dispatch: for each cascade | 每个级联\ndef update(cascades):\n    for c in cascades:  # @all\n        x = load(c)  # 64^3  # @read @@read\n        if x > 0 and \\\n           x < 1:  # @test\n            store(c, x)  # @@write\n        elif x == 0:\n            continue\n        else:\n            # nothing\n            y = f(x,\n                  2)\n\n    return c\n";

    #[test]
    fn parses_tags_marks_and_dispatch() {
        let listing = Listing::parse("t", LISTING);
        assert_eq!(listing.dispatch.as_ref().map(|d| d.0.as_str()), Some("for each cascade"));
        assert_eq!(listing.lines[0].text, "def update(cascades):");
        let load = &listing.lines[2];
        assert_eq!(load.text, "        x = load(c)  # 64^3");
        assert_eq!(load.tags, vec!["read".to_string()]);
        assert_eq!(load.mark, Some(Kind::Read));
        assert_eq!(listing.lines[5].mark, Some(Kind::Write));
        assert!(listing.lines[5].tags.is_empty());
        assert_eq!(listing.lines[1].kind(), Kind::Control);
        assert_eq!(listing.lines[0].kind(), Kind::Statement);
    }

    #[test]
    fn nests_blocks() {
        let listing = Listing::parse("t", LISTING);
        let blocks = blocks(&listing);
        let Block::C { arms } = &blocks[1] else { panic!("{blocks:?}") };
        assert_eq!(blocks[0], Block::Hat { lines: 0..1 });
        assert_eq!(arms.len(), 1);
        assert_eq!(arms[0].head, 1..2);
        assert_eq!(arms[0].body[0], Block::Stmt { lines: 2..3 });
        let Block::C { arms: branch } = &arms[0].body[1] else { panic!("{:?}", arms[0].body) };
        // The `if` continues on its second line; then `elif` and `else`.
        assert_eq!(branch.iter().map(|a| a.head.clone()).collect::<Vec<_>>(), vec![3..5, 6..7, 8..9]);
        assert_eq!(branch[1].body, vec![Block::Cap { lines: 7..8 }]);
        assert_eq!(branch[2].body, vec![Block::Note { line: 9 }, Block::Stmt { lines: 10..12 }]);
        // The blank line, then the `return` back at the function's level.
        assert_eq!(&blocks[2..], &[Block::Gap, Block::Cap { lines: 13..14 }]);
    }
}
