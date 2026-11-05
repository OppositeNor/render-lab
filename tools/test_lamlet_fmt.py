"""Tests of the Lamlet formatter (lamlet_fmt.py): `python3 -m unittest tools/test_lamlet_fmt.py`."""
from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import lamlet_fmt  # noqa: E402
from lamlet_fmt import format_document, tokens  # noqa: E402


class Layout(unittest.TestCase):
    def test_short_code_stays_on_one_line(self) -> None:
        source = "$[# {f x} {+   x\n    1}]\n"
        self.assertEqual(format_document(source), "$[# {f x} {+ x 1}]\n")

    def test_a_long_list_puts_one_item_per_line_and_hangs(self) -> None:
        fields = " ".join(f'{{Field "field{i}" Float}}' for i in range(6))
        out = format_document(f"$[# Thing [% {{Struct {{' {fields}}}}}]]\n")
        self.assertEqual(out.splitlines()[0], "$[# Thing [% {Struct {'")
        self.assertEqual(out.splitlines()[1], '    {Field "field0" Float}')
        self.assertTrue(out.rstrip().endswith('{Field "field5" Float}}}]]'))

    def test_a_list_of_plain_values_fills_its_lines(self) -> None:
        values = " ".join(f"{i}.0" for i in range(40))
        out = format_document(f"${{f {{' {values}}}}}\n")
        self.assertLessEqual(max(len(line) for line in out.splitlines()), lamlet_fmt.WIDTH)
        self.assertLess(len(out.splitlines()), 6)

    def test_a_record_puts_one_field_per_line(self) -> None:
        keys = " ".join(f"key{i} {{some value{i}}}" for i in range(8))
        out = format_document(f"$[# r {{. {keys}}}]\n")
        self.assertIn("\n    key3 {some value3}\n", out)

    def test_an_ascription_keeps_its_type_last(self) -> None:
        body = " ".join(f"{{set{i} 0.0}}" for i in range(12))
        out = format_document(f"$[# h {{: {{^ {{t}} {{' {body}}}}} {{Action Args}}}}]\n")
        self.assertTrue(out.rstrip().endswith("}} {Action Args}}]"), out)

    def test_lines_respect_the_width(self) -> None:
        cue = "${cue \"key\" {' " + " ".join(f"{{value{i} {i}.0}}" for i in range(20)) + "}}"
        out = format_document(cue + " The sentence.\n")
        self.assertLessEqual(max(len(line) for line in out.splitlines()), lamlet_fmt.WIDTH)

    def test_a_long_cue_line_puts_the_sentence_on_its_own_line(self) -> None:
        source = "${cue \"key\" {' {callout \"one\"}}} ${cue \"key\" {' {callout \"two\"}}} " + "word " * 12 + "end.\n"
        lines = format_document(source).splitlines()
        self.assertEqual(lines[0], "${cue \"key\" {' {callout \"one\"}}}")
        self.assertEqual(lines[1], "${cue \"key\" {' {callout \"two\"}}}")
        self.assertTrue(lines[2].startswith("word "))

    def test_a_cue_against_a_word_stays_there(self) -> None:
        source = "${cue \"key\" {' {callout \"one\"}}}word " + "word " * 20 + "end.\n"
        self.assertEqual(format_document(source).splitlines()[0][:40], source[:40])

    def test_a_long_import_line_puts_one_import_per_line(self) -> None:
        names = ["std", "content", "present", "render_lab", "a_long_package_name", "a_long_package_story"]
        out = format_document("".join(f"$[Import* {n}]" for n in names) + "\n")
        self.assertEqual(out.splitlines(), [f"$[Import* {n}]" for n in names])

    def test_strings_templates_and_escaped_dollars_are_kept(self) -> None:
        source = ("Prose with $$[Import* x] in it.\n"
                  '$[# s {f "a } ] \\" b" f"{x ${spoken {n}} y}" ' + " ".join(f"arg{i}" for i in range(20)) + "}]\n")
        out = format_document(source)
        self.assertEqual(tokens(out), tokens(source))
        self.assertIn('"a } ] \\" b"', out)
        self.assertIn('f"{x ${spoken {n}} y}"', out)
        self.assertTrue(out.startswith("Prose with $$[Import* x] in it.\n"))

    def test_glued_tokens_stay_glued(self) -> None:
        out = format_document("${r.method " + " ".join(f"{{arg{i}}}" for i in range(20)) + "}\n")
        self.assertTrue(out.startswith("${r.method"))

    def test_comments_are_left_alone(self) -> None:
        with self.assertRaises(lamlet_fmt.Unsupported):
            format_document("$[# x ;; why\n 1]\n")


class TrackedFiles(unittest.TestCase):
    def test_every_tracked_file_is_formatted(self) -> None:
        """Formatting is stable, keeps every token, and the tracked files are already formatted."""
        for path in lamlet_fmt.tracked():
            with self.subTest(path=path.name):
                source = path.read_text(encoding="utf-8")
                formatted = format_document(source)
                self.assertEqual(tokens(formatted), tokens(source))
                self.assertEqual(formatted, source, "run python3 tools/lamlet_fmt.py")


if __name__ == "__main__":
    unittest.main()
