//! The text keys a Lamlet file names, found by the pinned Lamlet parser rather than by matching text: each
//! region's syntax, before any macro expands, and the holes of its templates. A key must be written where it is
//! used, as a literal String, so that the check sees every key a file can show.

use lamlet::source::{RegionKind, Segment, Span, scan_document};
use lamlet::syntax::{Syntax, Tok, Token, lex, read};
use std::path::Path;

/// The calls whose one argument is a text key: a script's `textKey` and a storyboard's cues.
const KEY_CALLS: [&str; 4] = ["textKey", "imageTitle", "imageMarkText", "labelText"];

/// The fields `{attr "<field>" key}` writes a text key to.
const KEY_ATTRS: [&str; 3] = ["imageTitle", "imageMarkText", "labelText"];

/// The record field that holds a Lamlet data file's title (a scene's `title-text`, lab/src/script/data.rs).
const KEY_FIELD: &str = "titleText";

/// The text keys `text` (the file `path`) names, each with its file and line. Panics, naming the place, on a key
/// that is not a literal String, and on a file the parser refuses.
pub fn keys(path: &Path, text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let file = File { path, text };
    let segments =
        scan_document(0, text).unwrap_or_else(|e| panic!("{}: {}", path.display(), e.message));
    for segment in segments {
        // Code is in brace and bracket regions; prose, comments, and lookups name no key.
        if let Segment::Region {
            kind: RegionKind::Brace | RegionKind::Bracket,
            inner,
            ..
        } = segment
        {
            file.region(inner.start as usize, inner.end as usize, &mut found);
        }
    }
    found
}

/// A Lamlet file being read: its path and its text.
struct File<'a> {
    path: &'a Path,
    text: &'a str,
}

impl File<'_> {
    /// `path:line` of the byte `offset`.
    fn place(&self, offset: usize) -> String {
        let line = self.text[..offset.min(self.text.len())]
            .matches('\n')
            .count()
            + 1;
        format!("{}:{line}", self.path.display())
    }

    /// The keys of the code between the bytes `start` and `end`.
    fn region(&self, start: usize, end: usize, found: &mut Vec<(String, String)>) {
        let at = || self.place(start);
        let tokens = lex(0, &self.text[start..end], start)
            .unwrap_or_else(|e| panic!("{}: {}", at(), e.message));
        let trees = read(&tokens, Span::new(0, start, end))
            .unwrap_or_else(|e| panic!("{}: {}", at(), e.message));
        for tree in &trees {
            self.walk(tree, found);
        }
    }

    /// The keys of a syntax tree and everything in it.
    fn walk(&self, tree: &Syntax, found: &mut Vec<(String, String)>) {
        match tree {
            Syntax::Brace(items, _) => {
                self.call(items, found);
                self.fields(items, found);
                for item in items {
                    self.walk(item, found);
                }
            }
            Syntax::Bracket(items, _, _) => {
                // A definition's head (`[# {textKey key} key]`) is its parameters, not a call.
                let body = match items.first() {
                    Some(head) if ident(head) == Some("#") => items.get(2..).unwrap_or_default(),
                    _ => items.as_slice(),
                };
                for item in body {
                    self.walk(item, found);
                }
            }
            // A template's holes are regions of their own, in the template's text.
            Syntax::Atom(Token {
                tok: Tok::Template(lit),
                ..
            }) => {
                let base = lit.offset;
                let holes = scan_document(0, &lit.raw)
                    .unwrap_or_else(|e| panic!("{}: {}", self.place(base), e.message));
                for hole in holes {
                    if let Segment::Region { inner, .. } = hole {
                        self.region(
                            base + inner.start as usize,
                            base + inner.end as usize,
                            found,
                        );
                    }
                }
            }
            Syntax::Atom(_) => {}
        }
    }

    /// A call that takes a text key: `{textKey "key"}`, a cue's `{imageTitle "key"}`, or `{attr "imageTitle"
    /// "key"}`. Its key must be a literal String (an empty one, a cue clearing its text, names none).
    fn call(&self, items: &[Syntax], found: &mut Vec<(String, String)>) {
        let key = match items {
            [head, key] if ident(head).is_some_and(|h| KEY_CALLS.contains(&h)) => key,
            [head, field, key]
                if ident(head) == Some("attr")
                    && string(field).is_some_and(|f| KEY_ATTRS.contains(&f)) =>
            {
                key
            }
            _ => return,
        };
        self.key(key, &items[0], found);
    }

    /// A record literal's `titleText` (`{. titleText "key" …}`). Besides a literal String, it may be the same
    /// field read back (`titleText {.titleText r}`): a helper passing on a key its caller wrote.
    fn fields(&self, items: &[Syntax], found: &mut Vec<(String, String)>) {
        if items.first().and_then(ident) != Some(".") {
            return;
        }
        for pair in items[1..].chunks(2) {
            let [name, value] = pair else { continue };
            if ident(name) != Some(KEY_FIELD) || reads_field(value, KEY_FIELD) {
                continue;
            }
            self.key(value, name, found);
        }
    }

    /// `key`, which `by` gives a text key: a literal String, or a build error at its place.
    fn key(&self, key: &Syntax, by: &Syntax, found: &mut Vec<(String, String)>) {
        let at = self.place(key.span().start as usize);
        let Some(key) = string(key) else {
            panic!(
                "{at}: the text key `{}` takes must be a literal String, written where it is used, for the \
                 text check to find it",
                self.source(by)
            );
        };
        if !key.is_empty() {
            found.push((key.to_string(), at));
        }
    }

    /// The source text of a tree.
    fn source(&self, tree: &Syntax) -> &str {
        let span = tree.span();
        &self.text[span.start as usize..span.end as usize]
    }
}

/// The name an atom is, if it is a plain one.
fn ident(tree: &Syntax) -> Option<&str> {
    match tree {
        Syntax::Atom(Token {
            tok: Tok::Ident(parts),
            ..
        }) if parts.len() == 1 => Some(lamlet::syntax::base_name(&parts[0])),
        _ => None,
    }
}

/// The value of a literal String.
fn string(tree: &Syntax) -> Option<&str> {
    match tree {
        Syntax::Atom(Token {
            tok: Tok::Str(lit), ..
        }) => lit.value(),
        _ => None,
    }
}

/// Whether `tree` reads the field `name` of a record: `{.name r}`.
fn reads_field(tree: &Syntax, name: &str) -> bool {
    matches!(tree, Syntax::Brace(items, _) if matches!(
        items.first(),
        Some(Syntax::Atom(Token { tok: Tok::Label(lamlet::syntax::LabelKind::Get, l), .. })) if l == name
    ) && items.len() == 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(text: &str) -> Vec<String> {
        keys(Path::new("t.lam"), text)
            .into_iter()
            .map(|(k, at)| format!("{k}@{at}"))
            .collect()
    }

    #[test]
    fn keys_are_found_wherever_the_code_writes_them() {
        let text = "Prose names {textKey \"not-a-key\"}.\n\
            ${cue \"key\" {' {imageTitle \"views-a\"} {imageMarkText \"\"}}}\n\
            $[# {_shape at} {labelText at\n    {textKey \"demos-b\"}\n    textColor}]\n\
            $[# {textKey key} key]\n\
            ${attr \"labelText\" \"views-c\"}\n\
            ${f\"{a ${textKey \"views-d\"} b}\"}\n\
            $[# {_room r} {. titleText {.titleText r} floor 1.0}]\n\
            $[# room {_room {. titleText \"scene-e\" floor 2.0}}]\n";
        assert_eq!(
            found(text),
            vec![
                "views-a@t.lam:2",
                "demos-b@t.lam:4",
                "views-c@t.lam:7",
                "views-d@t.lam:8",
                "scene-e@t.lam:10",
            ]
        );
    }

    #[test]
    #[should_panic(expected = "t.lam:1: the text key `imageTitle` takes must be a literal String")]
    fn a_computed_key_is_refused() {
        found("$[# {_titled name} {imageTitle {++ \"views-\" name}}]\n");
    }

    #[test]
    #[should_panic(expected = "the text key `titleText` takes must be a literal String")]
    fn a_computed_title_is_refused() {
        found("$[# room {. titleText {++ \"scene-\" \"x\"}}]\n");
    }
}
