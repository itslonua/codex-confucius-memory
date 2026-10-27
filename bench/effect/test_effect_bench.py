import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from continuation_bench import Task, parse_task, score_trial, inject_context, rebind_frozen_store

ROOT = Path(__file__).resolve().parent
FIXTURE = ROOT / "fixtures/repo"
ORACLES = ROOT / "oracle"


class EffectBenchTests(unittest.TestCase):
    def test_parses_well_formed_task(self):
        task = parse_task({
            "id": "repeat-investigation", "kind": "repeat",
            "prompt": "Investigate the parser.", "query": "decoder parser",
            "acceptance": {"oracle": "check.py", "artifacts": ["parser.py"]},
        })
        self.assertIsInstance(task, Task)
        self.assertEqual(task.id, "repeat-investigation")

    def test_rejects_unsafe_acceptance_path(self):
        value = {
            "id": "x", "kind": "repeat", "prompt": "x", "query": "x",
            "acceptance": {"oracle": "check.py", "artifacts": ["../outside.py"]},
        }
        with self.assertRaises(ValueError):
            parse_task(value)

    def test_automatic_restore_context_is_preserved_verbatim(self):
        injected = inject_context("task prompt", "historical note: separator is hyphen")
        self.assertIn("task prompt", injected)
        self.assertTrue(injected.endswith("historical note: separator is hyphen"))
        with self.assertRaises(ValueError):
            inject_context("task prompt", None)

    def test_rebind_frozen_store_changes_only_project_key_and_preserves_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            old_cwd, new_cwd = root / "old-project", root / "new-project"
            old_cwd.mkdir()
            new_cwd.mkdir()
            source = root / "frozen-store"
            old_key = hashlib.sha256(str(old_cwd.resolve()).encode()).hexdigest()
            source_project = source / old_key
            (source_project / "notes").mkdir(parents=True)
            source_files = {
                "runtime.json": b'{"path":"historical"}\n',
                "notes/prior.md": b"frozen note bytes\n",
            }
            for relative, content in source_files.items():
                (source_project / relative).write_bytes(content)
            source_lock = source / f"{old_key}.lock"
            source_lock.write_bytes(b"lock bytes")

            destination = root / "rebound-store"
            rebound = rebind_frozen_store(source, destination, old_cwd, new_cwd)
            new_key = hashlib.sha256(str(new_cwd.resolve()).encode()).hexdigest()
            self.assertEqual(rebound, destination / new_key)
            self.assertFalse((destination / old_key).exists())
            self.assertEqual({path.relative_to(rebound).as_posix(): path.read_bytes()
                              for path in rebound.rglob("*") if path.is_file()}, source_files)
            self.assertEqual((destination / f"{new_key}.lock").read_bytes(), b"lock bytes")
            self.assertEqual((source / old_key / "runtime.json").read_bytes(), source_files["runtime.json"])
            with self.assertRaises(FileExistsError):
                rebind_frozen_store(source, destination, old_cwd, new_cwd)
            with self.assertRaises(ValueError):
                rebind_frozen_store(source, root / "unsafe", Path("relative-old"), new_cwd)

    def test_acceptance_requires_zero_exit_and_expected_artifacts(self):
        task = Task("x", "repeat", "x", "query", "check.py", ("fix.py", "tests/test_fix.py"))
        self.assertTrue(score_trial(task, 0, ["fix.py", "tests/test_fix.py"]))
        self.assertFalse(score_trial(task, 1, ["fix.py", "tests/test_fix.py"]))
        self.assertFalse(score_trial(task, 0, ["fix.py"]))

    def test_each_frozen_oracle_fails_then_passes_with_only_permitted_files(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary) / "base"
            shutil.copytree(FIXTURE, base)
            env = {**os.environ, "PYTHONDONTWRITEBYTECODE": "1"}

            def check(project, name):
                return subprocess.run([sys.executable, str(ORACLES / name), str(project)],
                                      cwd=Path(temporary), env=env, capture_output=True).returncode

            def corrected_copy(name):
                project = Path(temporary) / name
                shutil.copytree(base, project)
                return project

            archive_fix = (
                "import re\n\ndef archive_key(title):\n"
                "    cleaned = re.sub(r'[^a-z0-9\\s]', '', title.lower())\n"
                "    return re.sub(r'\\s+', '_', cleaned.strip())\n"
            )
            catalog_fix = (
                "import re\n\ndef slugify(label):\n"
                "    cleaned = re.sub(r'[^a-z0-9\\s]', '', label.lower())\n"
                "    return re.sub(r'\\s+', '_', cleaned.strip())\n"
            )
            for oracle in ("catalog.py", "archive.py", "policy.py"):
                self.assertNotEqual(check(base, oracle), 0)

            repeat = corrected_copy("repeat")
            (repeat / "catalog.py").write_text(catalog_fix, encoding="utf-8")
            (repeat / "archive.py").write_text(archive_fix, encoding="utf-8")
            (repeat / "tests/test_catalog.py").write_text(
                "from catalog import slugify\n\ndef test_normalizes_title():\n"
                "    assert slugify('Sprocket, Widget!') == 'sprocket_widget'\n", encoding="utf-8")
            self.assertEqual(check(repeat, "catalog.py"), 0)
            self.assertEqual((repeat / "policy.py").read_text(), (base / "policy.py").read_text())

            transfer = corrected_copy("transfer")
            (transfer / "archive.py").write_text(archive_fix, encoding="utf-8")
            (transfer / "tests/test_archive.py").write_text(
                "from archive import archive_key\n\ndef test_normalizes_title():\n"
                "    assert archive_key('The, Archive!') == 'the_archive'\n", encoding="utf-8")
            self.assertEqual(check(transfer, "archive.py"), 0)
            self.assertEqual((transfer / "catalog.py").read_text(), (base / "catalog.py").read_text())

            stale = corrected_copy("stale")
            (stale / "archive.py").write_text(archive_fix, encoding="utf-8")
            (stale / "tests/test_policy.py").write_text(
                "from archive import archive_key\nfrom policy import CURRENT_SEPARATOR\n\n"
                "def test_follows_current_policy():\n"
                "    assert CURRENT_SEPARATOR == '_'\n"
                "    assert archive_key('A, B!') == 'a_b'\n", encoding="utf-8")
            self.assertEqual(check(stale, "policy.py"), 0)
            self.assertEqual((stale / "catalog.py").read_text(), (base / "catalog.py").read_text())
            self.assertFalse(any(Path(temporary).rglob("__pycache__")))


if __name__ == "__main__":
    unittest.main()
