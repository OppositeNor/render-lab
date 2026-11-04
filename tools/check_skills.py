"""Checks the agent skills in .agents/skills (and AGENTS.md) for what can rot without anyone noticing.

Run from the repository's root: `python3 tools/check_skills.py`. It reads files only, and prints one line per problem
(exit status 1 when there is one):

- each skill's front matter: `name` (the directory's name, lower-case words joined by `-`, at most 64 characters) and
  `description` (at most 1024 characters), and no two skills with one name;
- `.claude/skills`: one link per skill, to `../../.agents/skills/<name>`, and nothing else;
- references: every `references/<file>.md` a skill names exists (in that skill, or in the skill named in the same
  paragraph), and every file in a skill's `references/` is named by its SKILL.md;
- repository paths in backticks (`lab/…`, `packages/…`, `tools/…`, …) exist, unless they are a pattern (`<name>`, `*`,
  `…`) or a generated output;
- commands (`render-lab <command>`, `cargo run --release -- … <command>`) name a subcommand of lab/src/cli.rs;
- each SKILL.md has a section or table that says how to check its work.
"""

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILLS = ROOT / '.agents' / 'skills'
LINKS = ROOT / '.claude' / 'skills'

# Path prefixes of the repository that a skill may cite in backticks.
PATH_PREFIXES = ('lab/', 'packages/', 'tools/', 'todo/', '.agents/', 'LICENSES/')
ROOT_FILES = ('AGENTS.md', 'README.md', 'THIRD_PARTY.md', 'Cargo.toml', 'Cargo.lock', '.gitignore')
# Generated or local outputs: cited, but not in a checkout until a tool writes them.
GENERATED = ('/build/', 'target/', '.cache/', 'vendor/')
PATTERN_MARKS = ('<', '>', '*', '…', '{', '}', '[', ']', '|')


def front_matter(text: str) -> dict[str, str] | None:
    """The `key: value` lines between the leading `---` lines, or None without them."""
    lines = text.split('\n')
    if not lines or lines[0].strip() != '---':
        return None
    fields: dict[str, str] = {}
    for line in lines[1:]:
        if line.strip() == '---':
            return fields
        match = re.match(r'^([a-z][a-z0-9_-]*):\s*(.*)$', line)
        if match:
            fields[match.group(1)] = match.group(2).strip()
    return None


def subcommands() -> set[str]:
    """The CLI's subcommands, kebab-case, from the `Command` enums of lab/src/cli.rs."""
    source = (ROOT / 'lab' / 'src' / 'cli.rs').read_text(encoding='utf-8')
    names: set[str] = set()
    for body in re.findall(r'#\[derive\(Subcommand\)\]\s*enum \w+ \{(.*?)\n\}', source, re.S):
        for variant in re.findall(r'^    ([A-Z][A-Za-z]+)\b', body, re.M):
            names.add(re.sub(r'(?<!^)(?=[A-Z])', '-', variant).lower())
    return names


def paragraphs(text: str) -> list[str]:
    return re.split(r'\n\s*\n', text)


def check_paths(source: Path, text: str, problems: list[str]) -> None:
    """Backticked repository paths that do not exist."""
    for span in re.findall(r'`([^`\n]+)`', text):
        candidate = span.split()[0].rstrip('.,;:)') if span.split() else ''
        if not (candidate.startswith(PATH_PREFIXES) or candidate in ROOT_FILES):
            continue
        if any(mark in candidate for mark in PATTERN_MARKS) or any(g in candidate for g in GENERATED):
            continue
        if not (ROOT / candidate).exists():
            problems.append(f'{source.relative_to(ROOT)}: `{candidate}` does not exist')


def check_commands(source: Path, text: str, known: set[str], problems: list[str]) -> None:
    """`render-lab X` and `cargo run --release -- [global options] X` with an X the CLI does not have."""
    globals_with_value = {'--investigation', '--root', '--listing-style'}
    for line in text.split('\n'):
        for match in re.finditer(r'(?:render-lab|cargo run --release --)((?:\s+\S+)+)', line):
            words = match.group(1).split()
            i = 0
            while i < len(words) and words[i].startswith('-'):
                i += 2 if words[i] in globals_with_value else 1
            if i >= len(words):
                continue
            word = words[i].strip('`')
            if not re.fullmatch(r'[a-z][a-z-]*', word) or word in ('check', 'core'):
                continue
            if word not in known:
                problems.append(f'{source.relative_to(ROOT)}: `{word}` is not a render-lab command ({line.strip()[:80]})')


def main() -> int:
    problems: list[str] = []
    known = subcommands()
    skills = sorted(p for p in SKILLS.iterdir() if p.is_dir())
    names: dict[str, Path] = {}
    for skill in skills:
        main_file = skill / 'SKILL.md'
        if not main_file.is_file():
            problems.append(f'{skill.relative_to(ROOT)}: no SKILL.md')
            continue
        text = main_file.read_text(encoding='utf-8')
        fields = front_matter(text)
        rel = main_file.relative_to(ROOT)
        if fields is None:
            problems.append(f'{rel}: no front matter')
            continue
        name = fields.get('name', '')
        description = fields.get('description', '')
        if name != skill.name:
            problems.append(f'{rel}: name `{name}` is not its directory, `{skill.name}`')
        if not re.fullmatch(r'[a-z0-9]+(-[a-z0-9]+)*', name) or len(name) > 64:
            problems.append(f'{rel}: name `{name}` is not lower-case words joined by `-` (at most 64)')
        if name in names:
            problems.append(f'{rel}: name `{name}` is also {names[name].relative_to(ROOT)}')
        names[name] = main_file
        if not description:
            problems.append(f'{rel}: no description')
        elif len(description) > 1024:
            problems.append(f'{rel}: description is {len(description)} characters (at most 1024)')
        if not re.search(r'^#+ .*\b(Check|Checks|check|Validat|Verif)', text, re.M) and 'Done when' not in text:
            problems.append(f'{rel}: no section says how to check the work')

        # References: named ones exist, existing ones are named.
        refs_dir = skill / 'references'
        existing = {p.name for p in refs_dir.glob('*.md')} if refs_dir.is_dir() else set()
        named_here: set[str] = set()
        for path in [main_file, *sorted(refs_dir.glob('*.md'))] if refs_dir.is_dir() else [main_file]:
            body = path.read_text(encoding='utf-8')
            for paragraph in paragraphs(body):
                others = set(re.findall(r'`([a-z0-9-]+)`', paragraph)) & {s.name for s in skills}
                for ref in re.findall(r'references/([A-Za-z0-9_-]+\.md)', paragraph):
                    if path == main_file:
                        named_here.add(ref)
                    if (refs_dir / ref).is_file():
                        continue
                    if any((SKILLS / other / 'references' / ref).is_file() for other in others):
                        continue
                    problems.append(f'{path.relative_to(ROOT)}: `references/{ref}` does not exist here or in a skill the paragraph names')
            check_paths(path, body, problems)
            check_commands(path, body, known, problems)
        for orphan in sorted(existing - named_here):
            problems.append(f'{rel}: references/{orphan} is not named by its SKILL.md')

    # References from outside the skills (AGENTS.md, other skills' paragraphs) were checked above; AGENTS.md's paths too.
    agents = ROOT / 'AGENTS.md'
    agents_text = agents.read_text(encoding='utf-8')
    check_paths(agents, agents_text, problems)
    check_commands(agents, agents_text, known, problems)
    for name in names:
        if f'`{name}`' not in agents_text:
            problems.append(f'AGENTS.md: does not name the skill `{name}`')

    # .claude/skills: one link per skill, to its directory.
    if LINKS.is_dir():
        for link in sorted(LINKS.iterdir()):
            if not link.is_symlink():
                problems.append(f'{link.relative_to(ROOT)}: not a link')
            elif link.readlink() != Path('../../.agents/skills') / link.name:
                problems.append(f'{link.relative_to(ROOT)}: links to {link.readlink()}, not ../../.agents/skills/{link.name}')
            elif link.name not in names:
                problems.append(f'{link.relative_to(ROOT)}: no skill `{link.name}`')
        for name in names:
            if not (LINKS / name).is_symlink():
                problems.append(f'.claude/skills: no link for `{name}`')
    else:
        problems.append('.claude/skills: missing')

    for problem in problems:
        print(problem)
    print(f'{len(names)} skills, {len(problems)} problem(s)')
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
