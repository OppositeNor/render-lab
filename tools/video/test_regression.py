"""The regression gate must reject incomplete captures, changed narration and changed timeline semantics."""
from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare  # noqa: E402
import narration  # noqa: E402
import regression  # noqa: E402
from config import Config, Environment  # noqa: E402


class Regression(unittest.TestCase):
    def snapshot(self, root: Path, *, frame: int = 3, audio: str = 'one', spoken: str = 'hello') -> Path:
        outputs = root / 'outputs'
        edition = outputs / 'godot_ssr/en'
        edition.mkdir(parents=True)
        for name in compare.PREPARED:
            (edition / name).write_text('{}' if name.endswith('.json') else 'prepared')
        regression.write_json(edition / 'timeline.json', {'keys': [{'frame': frame}]})
        regression.write_json(edition / 'document.json', {'text': spoken})
        (outputs / 'godot_ssr/scripts.txt').write_text('action march\n')
        (outputs / 'godot_ssr/audio.txt').write_text('clip.wav\nclip.json\n')
        regression.write_json(outputs / 'godot_ssr/audio-sha256.json', {'clip.wav': audio, 'clip.json': 'timings'})
        regression.write_json(outputs / 'inputs.json', {'storyboard.lam': 'input hash'})
        regression.write_json(root / 'manifest.json', {
            'renderLabLamletRegression': regression.VERSION,
            'cases': [{'investigation': 'godot/ssr', 'package': 'godot_ssr', 'languages': ['en']}],
            'host': {'binary': str(root)},
            'files': {p.relative_to(outputs).as_posix(): compare.digest(p) for p in outputs.rglob('*') if p.is_file()},
        })
        return root

    def test_compares_semantics_and_same_name_audio(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            old = self.snapshot(root / 'old')
            same = self.snapshot(root / 'same')
            self.assertEqual(regression.differences(old, same), [])  # Host provenance may change.
            timeline = self.snapshot(root / 'timeline', frame=4)
            self.assertTrue(any('.keys[0].frame: 3 != 4' in d for d in regression.differences(old, timeline)))
            audio = self.snapshot(root / 'audio', audio='changed bytes under the same clip.wav name')
            self.assertTrue(any('audio-sha256.json' in d for d in regression.differences(old, audio)))
            document = self.snapshot(root / 'document', spoken='changed narration')
            self.assertTrue(any('document.json' in d for d in regression.differences(old, document)))

    def test_missing_or_corrupt_artifacts_fail(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            with self.assertRaisesRegex(RuntimeError, 'incomplete capture'):
                regression.differences(root / 'missing-old', root / 'missing-new')
            old = self.snapshot(root / 'old')
            for mode in ('missing', 'modified', 'unlisted'):
                new = self.snapshot(root / mode)
                artifact = new / 'outputs/godot_ssr/en/timeline.json'
                if mode == 'missing':
                    artifact.unlink()
                elif mode == 'modified':
                    artifact.write_text('{"keys": []}')
                else:
                    (new / 'outputs/stale.txt').write_text('stale')
                with self.subTest(mode=mode), self.assertRaisesRegex(RuntimeError, 'missing or changed'):
                    regression.differences(old, new)

    def test_manifest_cannot_silently_omit_an_output(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            snapshot = self.snapshot(Path(tmp) / 'capture')
            path = snapshot / 'manifest.json'
            manifest = json.loads(path.read_text())
            del manifest['files']['godot_ssr/en/captions.vtt']
            regression.write_json(path, manifest)
            with self.assertRaisesRegex(RuntimeError, 'missing required artifacts'):
                regression.checked_manifest(snapshot)

    def test_required_prepared_files_and_existing_destination(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cfg = Config('godot/ssr', 'en', {}, root / 'package/video')
            with self.assertRaisesRegex(RuntimeError, 'missing prepared outputs'):
                compare.save_edition(cfg, root / 'snapshot', required=True)
            cfg.build().mkdir(parents=True)
            cfg.timeline_dir().mkdir(parents=True)
            cfg.timeline().write_text('{}')
            for name in compare.PREPARED:
                (cfg.build() / name).write_text('prepared')
            dest = compare.save_edition(cfg, root / 'snapshot', required=True)
            self.assertEqual(len(list(dest.iterdir())), len(compare.PREPARED) + 1)
            binary = root / 'render-lab'
            binary.touch()
            with patch.object(regression, 'configurations', return_value=[[cfg]]), \
                    patch.object(regression, 'input_hashes', return_value={}):
                with self.assertRaises(FileExistsError):
                    regression.capture(root / 'snapshot', binary, ['godot/ssr'])

    def test_cache_only_fails_before_creating_a_voice(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            cfg = Config('test', 'en', {}, Path(tmp) / 'package/video')
            sentence = {'at': 'story.lam:3:1'}
            with patch.object(narration, 'spoken', return_value='A sentence.'), \
                    patch.object(narration, 'create_voice') as voice:
                with self.assertRaisesRegex(RuntimeError, 'narration cache miss'):
                    narration.synthesize(cfg, Environment(), 'cpu', [sentence], cached_only=True)
                voice.assert_not_called()
                digest = hashlib.sha256(('A sentence.' + str(narration.VOICE_SPEED) + narration.VOICE_ID).encode()).hexdigest()[:24]
                # An existing WAV without word timing metadata is also a miss.
                (cfg.audio_cache / f'{digest}.wav').touch()
                with self.assertRaisesRegex(RuntimeError, 'narration cache miss'):
                    narration.synthesize(cfg, Environment(), 'cpu', [sentence], cached_only=True)
                voice.assert_not_called()

    def test_preparation_outputs_are_isolated_but_audio_cache_is_shared(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            cfg = Config('test', 'zh-CN', {}, root / 'package/video')
            original_cache = cfg.audio_cache
            cfg.prepared_root = root / 'capture/prepared/package'
            self.assertEqual(cfg.timeline('en'), cfg.prepared_root / 'timelines/en/timeline.json')
            self.assertEqual(cfg.build(), cfg.prepared_root / 'video/zh-CN')
            self.assertEqual(cfg.audio_cache, original_cache)


if __name__ == '__main__':
    unittest.main()
