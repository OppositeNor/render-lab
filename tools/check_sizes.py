"""Checks the size limits on the Rust and Python sources: at most 1400 lines a file, 100 lines a Python function.

Run from the repository's root: `python3 tools/check_sizes.py [PATH ...]`. Without paths it checks the `.rs` and `.py`
files `git` would publish (`git ls-files --cached --others --exclude-standard`, without `vendor/`); with paths, those
files, and the `.rs` and `.py` files under those directories. It prints one line per problem (exit status 1 when there
is one).

A file counts all its lines. A Python function counts the lines of its body that hold code, as clippy's
`too_many_lines` counts a Rust function (which enforces the same 100 for Rust, `Cargo.toml`'s `[workspace.lints]`):
blank lines, comment lines, and its docstring do not count. A function nested in another counts in both.
"""

import ast
import io
import subprocess
import sys
import tokenize
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_FILE_LINES = 1400
MAX_FUNCTION_LINES = 100
SUFFIXES = (".rs", ".py")


def published() -> list[Path]:
    """The Rust and Python files `git` would publish, without `vendor/`."""
    out = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    return [
        ROOT / name
        for name in out.splitlines()
        if name.endswith(SUFFIXES) and not name.startswith("vendor/") and (ROOT / name).is_file()
    ]


def named(paths: list[str]) -> list[Path]:
    """The files `paths` names: each file, and the Rust and Python files under each directory."""
    files: list[Path] = []
    for name in paths:
        path = Path(name)
        if path.is_dir():
            files += sorted(p for p in path.rglob("*") if p.suffix in SUFFIXES and p.is_file())
        else:
            files.append(path)
    return files


def code_lines(source: str) -> set[int]:
    """The lines of `source` that hold a token other than a comment (a string's every line counts)."""
    lines: set[int] = set()
    skip = {tokenize.COMMENT, tokenize.NL, tokenize.NEWLINE, tokenize.INDENT, tokenize.DEDENT, tokenize.ENDMARKER}
    for token in tokenize.generate_tokens(io.StringIO(source).readline):
        if token.type not in skip:
            lines.update(range(token.start[0], token.end[0] + 1))
    return lines


def long_functions(source: str) -> list[tuple[str, int, int]]:
    """Each Python function of `source` over the limit: its name, first line, and code lines."""
    tree = ast.parse(source)
    code = code_lines(source)
    found = []
    for node in ast.walk(tree):
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            continue
        body = node.body
        first = body[0]
        docstring = (
            isinstance(first, ast.Expr) and isinstance(first.value, ast.Constant) and isinstance(first.value.value, str)
        )
        doc = set(range(first.lineno, (first.end_lineno or first.lineno) + 1)) if docstring else set()
        start = body[0].lineno
        end = node.end_lineno or start
        count = len([n for n in range(start, end + 1) if n in code and n not in doc])
        if count > MAX_FUNCTION_LINES:
            found.append((node.name, node.lineno, count))
    return found


def show(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def main(argv: list[str]) -> int:
    files = named(argv[1:]) if len(argv) > 1 else published()
    problems = []
    for path in files:
        source = path.read_text(encoding="utf-8")
        lines = source.count("\n") + (0 if source.endswith("\n") or not source else 1)
        if lines > MAX_FILE_LINES:
            problems.append(f"{show(path)}: {lines} lines (at most {MAX_FILE_LINES} a file)")
        if path.suffix == ".py":
            for name, line, count in long_functions(source):
                problems.append(
                    f"{show(path)}:{line}: function `{name}` has {count} lines (at most {MAX_FUNCTION_LINES})"
                )
    for problem in problems:
        print(problem)
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
