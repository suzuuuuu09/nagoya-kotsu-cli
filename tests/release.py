import tempfile
from pathlib import Path
import unittest

from scripts.bundle_licenses import collect


class LicenseBundleTest(unittest.TestCase):
    def test_nested_licenses_and_notices_are_preserved_and_missing_text_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = {"name": "fixture", "version": "1.0.0", "license": "MIT",
                       "manifest_path": str(root / "Cargo.toml"), "license_file": None}
            with self.assertRaises(ValueError):
                collect(package)
            (root / "LICENSE").write_text("See third_party/LICENSE-MIT\n", encoding="utf-8")
            (root / "NOTICE").write_text("Copyright fixture contributors\n", encoding="utf-8")
            (root / "third_party").mkdir()
            (root / "third_party/LICENSE-MIT").write_text("Full license text\n", encoding="utf-8")
            files = collect(package)["files"]
            self.assertEqual(files["NOTICE"], "Copyright fixture contributors\n")
            self.assertEqual(files[str(Path("third_party/LICENSE-MIT"))], "Full license text\n")
            self.assertEqual(len(files), 3)
