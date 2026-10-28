//! Python pseudo-code of an engine's shading logic, shown in the struct panel's
//! place (PanelMode::Code) and listed in the script and the source guide.
//!
//! An investigation embeds its listings and parses them with `Listing::parse`. Each listing starts with header
//! comments, `# title: English | 中文`, `# source: <source-index key>`
//! (the engine function it follows), and `# demo: <demo key>`. A line may
//! end in tags, `# @name ...`; a shot's `highlight` names the tags whose
//! lines are lit. Tags are not shown.

pub struct Line {
    pub text: String,
    pub tags: Vec<String>,
}

pub struct Listing {
    pub name: &'static str,
    title: String,
    title_zh: Option<String>,
    pub source: String,
    pub demo: Option<String>,
    pub lines: Vec<Line>,
}

impl Listing {
    pub fn title(&self) -> &str {
        match (&self.title_zh, crate::i18n::zh()) {
            (Some(zh), true) => zh,
            _ => &self.title,
        }
    }

    pub fn lit(&self, line: &Line, highlight: &[String]) -> bool {
        line.tags.iter().any(|t| highlight.contains(t))
    }

    pub fn parse(name: &'static str, text: &str) -> Listing {
        let mut listing = Listing { name, title: name.to_string(), title_zh: None, source: String::new(), demo: None, lines: Vec::new() };
        let mut header = true;
        for raw in text.lines() {
            if header {
                if let Some((key, value)) = raw.strip_prefix("# ").and_then(|r| r.split_once(": ")) {
                    match key {
                        "title" => {
                            let (en, zh) = value.split_once(" | ").map_or((value, None), |(a, b)| (a, Some(b)));
                            listing.title = en.trim().to_string();
                            listing.title_zh = zh.map(|z| z.trim().to_string());
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
                        _ => {}
                    }
                }
                header = false;
            }
            let (code, tags) = match raw.rfind("# @") {
                Some(at) => (raw[..at].trim_end(), raw[at + 2..].split_whitespace().filter_map(|t| t.strip_prefix('@')).map(str::to_string).collect()),
                None => (raw.trim_end(), Vec::new()),
            };
            listing.lines.push(Line { text: code.to_string(), tags });
        }
        while listing.lines.last().is_some_and(|l| l.text.is_empty()) {
            listing.lines.pop();
        }
        listing
    }
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
