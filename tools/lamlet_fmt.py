"""Lays out the code of the lab's Lamlet files, so a long expression reads one part per line.

Run from the repository's root: `python3 tools/lamlet_fmt.py` rewrites every tracked `.lam` file of `packages/`
(or the files given), and `--check` only lists the files it would change (exit status 1 when there is one).

Inside each code region (`$[...]`, `${...}`), an expression that fits in the width stays on one line. A longer one
breaks Lisp-style, its parts 4 spaces in from the indent of the line it starts on, and its closing brackets at the end of its last line:

- a list (`{' ...}`) puts one item per line, or fills its lines when every item is a plain value; a record
  (`{. ...}`) one field and its value per line;
- a call keeps its function and leading plain arguments on its first line (`{cue "key"`), and a form its header
  (`[# {f x}`, `{^ {click}`, `{!if cond`); when everything before the last argument fits, the last one hangs from the
  first line (`{opening demoRoom {'`);
- `{: expr Type}` keeps its type after the expression.

Two rules change prose whitespace, which the lab's storyboard reader reads as a space: a line of cues and the
sentence after them (`${cue …} ${cue …} The sentence.`) that is too long puts each on a line of its own, and a line
of imports that is too long puts one import per line. Every token, and every word of the prose, stays as it was;
the tool checks that before it writes a file. A file with a comment (`;;`, `;{…}`, `$;{…}`) in its code is left as
it is.
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WIDTH = 100
STEP = 4
CLOSE = {"{": "}", "[": "]"}
# Forms whose first argument stays on their first line.
HEADED = ("^", "#", "!if", "!case", "!let", "%", "match")


class Unsupported(Exception):
    """Code the formatter leaves alone."""


@dataclass
class Atom:
    text: str
    glued: bool = False  # written against the token before it


@dataclass
class Node:
    open: str
    close: str
    children: list[Atom | Node] = field(default_factory=list)
    glued: bool = False


Item = Atom | Node


def _string_end(s: str, i: int) -> int:
    """The index after the String that starts at `s[i] == '"'`."""
    i += 1
    while s[i] != '"':
        # A backslash escapes the character after it, so skip both.
        i += 2 if s[i] == "\\" else 1
    return i + 1


def _template_end(s: str, i: int) -> int:
    """The index after the template that starts at `s[i:i+3] == 'f"{'`: its text runs to the first `}"` outside a
    hole."""
    i += 3
    while not s.startswith('}"', i):
        if s.startswith("$$", i):
            # `$$` is a literal dollar; a hole (`${`, `$[`) is parsed whole, so its `}` cannot end the template.
            i += 2
        elif s.startswith("${", i) or s.startswith("$[", i):
            _, i = parse(s, i + 1)
        else:
            i += 1
    return i + 2


def parse(s: str, i: int) -> tuple[Node, int]:
    """The bracketed expression at `s[i]`, and the index after it."""
    node = Node(s[i], CLOSE[s[i]])
    i += 1
    space = True
    while True:
        c = s[i]
        if c in " \t\n":
            # Whitespace only records that the next token is not glued to the one before it.
            space = True
            i += 1
            continue
        if c == ";":
            raise Unsupported("a comment")
        glued = not space and bool(node.children)
        space = False
        if c == node.close:
            return node, i + 1
        if c in "}]":
            raise Unsupported("unbalanced brackets")
        if c in CLOSE:
            # A bracket opens a child expression, parsed to its own close.
            child, i = parse(s, i)
            child.glued = glued
            node.children.append(child)
            continue
        if c == '"':
            # Anything else is an atom; a string runs to its closing quote.
            j = _string_end(s, i)
        elif s.startswith('f"{', i):
            j = _template_end(s, i)
        else:
            j = i
            while s[j] not in ' \t\n{}[]";' and not s.startswith('f"{', j):
                j += 1
        node.children.append(Atom(s[i:j], glued))
        i = j


def flat(x: Item) -> str:
    """`x` on one line."""
    if isinstance(x, Atom):
        return x.text
    parts = [("" if k == 0 or c.glued else " ") + flat(c) for k, c in enumerate(x.children)]
    return x.open + "".join(parts) + x.close


def _groups(node: Node) -> list[list[Item]]:
    """The children, with each run of glued ones kept together."""
    out: list[list[Item]] = []
    for c in node.children:
        if c.glued and out:
            out[-1].append(c)
        else:
            out.append([c])
    return out


def _gflat(g: list[Item]) -> str:
    return "".join(flat(c) for c in g)


def _plain(g: list[Item]) -> bool:
    return all(isinstance(c, Atom) for c in g)


def _end_column(text: str, col: int) -> int:
    nl = text.rfind("\n")
    return col + len(text) if nl < 0 else len(text) - nl - 1


def _layout_group(g: list[Item], col: int, ind: int, tail: int) -> str:
    return layout(g[0], col, ind, tail) if len(g) == 1 else _gflat(g)


def layout(x: Item, col: int, ind: int, tail: int = 0) -> str:
    """`x` written from column `col`, its broken parts at column `ind`, with `tail` characters still to follow it on
    its last line."""
    text = flat(x)
    if isinstance(x, Atom) or col + len(text) + tail <= WIDTH:
        # An expression that fits on its line stays flat.
        return text
    groups = _groups(x)
    if len(groups) < 2:
        return text
    head, rest = _gflat(groups[0]), groups[1:]
    first = x.open + head
    nl = "\n" + " " * ind
    if head == "'" and all(_plain(g) for g in rest):
        # A list of plain values fills its lines, wrapping at the width.
        out, column = first, col + len(first)
        for g in rest:
            t = _gflat(g)
            if column + 1 + len(t) + 1 > WIDTH:
                out, column = out + nl + t, ind + len(t)
            else:
                out, column = out + " " + t, column + 1 + len(t)
        return out + x.close
    if head == "." and len(rest) % 2 == 0:
        # A record puts each field and its value on a line of its own.
        out = first
        for k in range(0, len(rest), 2):
            key = _gflat(rest[k])
            last = tail + 1 if k == len(rest) - 2 else 0
            out += nl + key + " " + _layout_group(rest[k + 1], ind + len(key) + 1, ind + STEP, last)
        return out + x.close
    if head == ":" and len(rest) == 2 and len(_gflat(rest[1])) <= WIDTH // 2:
        # A type annotation keeps its (short) type after the expression.
        end = " " + _gflat(rest[1]) + x.close
        return x.open + ": " + _layout_group(rest[0], col + 3, ind, tail + len(end)) + end
    if head in ("'", "."):
        # A list or record keeps no argument on its first line; a form keeps one, a call its plain ones.
        header = 0
    elif head in HEADED:
        header = 1
    else:
        header = 0
        while header < len(rest) - 1 and _plain(rest[header]):
            header += 1
    # The last argument hangs from the first line when every argument before it is in the header.
    if head != "'" and header >= len(rest) - 1 and len(rest[-1]) == 1 and isinstance(rest[-1][0], Node):
        before = first + "".join(" " + _gflat(g) for g in rest[:-1])
        if col + len(before) + 8 <= WIDTH:
            hung = layout(rest[-1][0], col + len(before) + 1, ind, tail + 1)
            if "\n" in hung:
                return before + " " + hung + x.close
    # Otherwise the header arguments follow the head, and every other argument takes a line of its own.
    out, column = first, col + len(first)
    for g in rest[:header]:
        t = _layout_group(g, column + 1, ind + STEP, 0)
        out, column = out + " " + t, _end_column(t, column + 1)
    for m, g in enumerate(rest[header:]):
        last = tail + 1 if header + m == len(rest) - 1 else 0
        out += nl + _layout_group(g, ind, ind + STEP, last)
    return out + x.close


def _regions(s: str) -> list[tuple[int, int, Node | None]]:
    """Each code region of a document: its start (the `$`), its end, and its expression; `None` for prose kept
    verbatim (`$$`)."""
    out: list[tuple[int, int, Node | None]] = []
    i = 0
    while i < len(s):
        if s.startswith("$$", i):
            # `$$` is a dollar in the prose, not a region.
            i += 2
        elif s.startswith("$;", i):
            raise Unsupported("a disabled region")
        elif s.startswith("${", i) or s.startswith("$[", i):
            node, j = parse(s, i + 1)
            out.append((i, j, node))
            i = j
        else:
            i += 1
    return out


def _split_cue_lines(s: str) -> str:
    """A line `${...} ${...} prose` wider than WIDTH: each region, and the prose, on a line of its own."""
    out: list[str] = []
    i = 0
    while i < len(s):
        end = s.find("\n", i)
        end = len(s) if end < 0 else end
        if not s.startswith("${", i):
            # A line that does not start with a cue is kept as it is.
            out.append(s[i:end + 1])
            i = end + 1
            continue
        pieces: list[str] = []
        j = i
        while s.startswith("${", j):
            # Collect the line's leading cues, each followed by one space.
            node, k = parse(s, j + 1)
            pieces.append(s[j:k])
            j = k + 1 if s.startswith(" ", k) else k
            if j == k:
                break
        end = s.find("\n", j)
        end = len(s) if end < 0 else end
        prose = s[j:end]
        # Split only a wide line whose cues are single-spaced, and never one whose prose starts a region.
        separated = all(s[i:j].count(p + " ") for p in pieces[:-1]) and (not prose or s[j - 1] == " ")
        wide = len(" ".join(flat(parse(p, 1)[0]) for p in pieces)) + 1 + len(prose) > WIDTH
        if separated and wide and len(pieces) + bool(prose) > 1 and not prose.startswith("$"):
            out.append("\n".join(pieces + ([prose] if prose else [])) + s[end:end + 1])
        else:
            out.append(s[i:end + 1])
        i = end + 1
    return "".join(out)


def _split_imports(m: re.Match[str]) -> str:
    line = m.group(0)
    return line if len(line) <= WIDTH else line.replace("]$[", "]\n$[")


def format_document(s: str) -> str:
    """The document `s`, its code laid out."""
    s = re.sub(r"^(\$\[Import[^\]\n]*\])+$", _split_imports, _split_cue_lines(s), flags=re.M)
    out: list[str] = []
    at = 0
    for start, end, node in _regions(s):
        # Each region is laid out from the column it starts at, its broken parts one step in from its line's indent.
        out.append(s[at:start])
        done = "".join(out)
        line = done[done.rfind("\n") + 1:]
        indent = len(line) - len(line.lstrip(" ")) + STEP
        out.append("$" + layout(node, len(line) + 1, indent) if node else s[start:end])
        at = end
    out.append(s[at:])
    return "".join(out)


def tokens(s: str) -> list[str]:
    """The document's prose words and code regions (each on one line): what formatting must not change."""
    out: list[str] = []
    at = 0
    for start, end, node in _regions(s):
        out += s[at:start].split()
        out.append("$" + flat(node) if node else s[start:end])
        at = end
    return out + s[at:].split()


def tracked() -> list[Path]:
    """The tracked `.lam` files of `packages/` (the built vocabularies in a package's `build/` are not tracked)."""
    names = subprocess.run(["git", "ls-files", "-z", "--", "packages/*.lam"], cwd=ROOT, check=True,
                           capture_output=True, text=True).stdout.split("\0")
    return [ROOT / n for n in names if n]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("files", nargs="*", type=Path, help="the files (default: every tracked .lam in packages/)")
    parser.add_argument("--check", action="store_true", help="list the files it would change, and change nothing")
    args = parser.parse_args(argv)
    changed = 0
    for path in args.files or tracked():
        source = path.read_text(encoding="utf-8")
        try:
            formatted = format_document(source)
        except Unsupported as e:
            print(f"{path}: left as it is ({e})", file=sys.stderr)
            continue
        if tokens(formatted) != tokens(source):
            # Formatting may only move whitespace: refuse a result whose tokens differ.
            print(f"{path}: formatting would change a token; left as it is", file=sys.stderr)
            return 2
        if formatted != source:
            changed += 1
            print(path.relative_to(ROOT) if path.is_relative_to(ROOT) else path)
            if not args.check:
                path.write_text(formatted, encoding="utf-8")
    return 1 if args.check and changed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
