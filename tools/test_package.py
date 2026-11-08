#!/usr/bin/env python3
"""The .hnmod is reproducible, source-only and carries a valid native manifest."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]


class PackageTests(unittest.TestCase):
    def test_reproducible_source_bundle(self):
        def build():
            result = subprocess.check_output([sys.executable, 'tools/package.py'], cwd=ROOT, text=True)
            report = json.loads(result)
            data = Path(report['file']).read_bytes()
            self.assertEqual(report['sha256'], hashlib.sha256(data).hexdigest())
            return report, data

        first, data = build()
        self.assertEqual((first, data), build())
        with zipfile.ZipFile(first['file']) as archive:
            names = set(archive.namelist())
            self.assertIn('horizon-native.json', names)
            self.assertIn('mixins/ultimine/horizon-mixin.toml', names)
            self.assertIn('mixins/ultimine/api/Cargo.toml', names)
            self.assertIn('mixins/ultimine/api/src/lib.rs', names)
            self.assertIn('mixins/ultimine/api/src/engine/shape.rs', names)
            self.assertTrue(all(n == 'horizon-native.json' or n.startswith('mixins/ultimine/') for n in names))
            meta = json.loads(archive.read('horizon-native.json'))
            self.assertEqual(meta['schema'], 1)
            self.assertEqual(meta['id'], 'ultimine')
            self.assertEqual(len(meta['horizon']), 40)
            self.assertEqual(len(meta['mixins']), 40)
            for info in archive.infolist():
                self.assertEqual(info.date_time, (2026, 1, 1, 0, 0, 0))
                if info.filename != 'horizon-native.json':
                    self.assertEqual(archive.read(info.filename), (ROOT / info.filename).read_bytes())


if __name__ == '__main__':
    unittest.main()
