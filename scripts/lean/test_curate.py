"""Private draw identity, reproducibility, permissions and output redaction tests."""
import contextlib
import io
import json
import pathlib
import tempfile
import unittest
from unittest import mock

import curate


class SliceTests(unittest.TestCase):
    """Retain immutable host inputs without exposing them in curation output."""

    def test_fresh_stage_ids_seeds_and_goals_reproduce_from_private_seed(self):
        bank = curate.generate(b'a' * 32)
        self.assertEqual(bank, curate.generate(b'a' * 32))
        self.assertNotEqual(bank, curate.generate(b'b' * 32))
        rows = [row for stage in bank['stages'].values() for row in stage]
        self.assertEqual(len({row['id'] for row in rows}), 12)
        self.assertEqual(len({row['seed'] for row in rows}), 12)
        self.assertEqual(len({row['theorem'] for row in rows}), 12)
        self.assertTrue(all(row['theorem'].endswith(':=') for row in rows))
        with self.assertRaises(ValueError):
            curate.generate(b'')

    def test_cli_keeps_inputs_private_and_refuses_existing_or_public_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            path = root / 'target' / 'private' / 'slice.json'
            output = io.StringIO()
            with mock.patch('curate.__file__', str(root / 'scripts' / 'lean' / 'curate.py')), \
                    mock.patch('curate.os.urandom', return_value=b'a' * 32), \
                    mock.patch('sys.argv', ['curate', '--output', str(path)]), \
                    contextlib.redirect_stdout(output):
                curate.main()
            bank = json.loads(path.read_text())
            self.assertEqual(bank, curate.generate(b'a' * 32))
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            self.assertNotIn('theorem', output.getvalue())
            self.assertNotIn('private_seed', output.getvalue())
            with self.assertRaises(FileExistsError):
                curate.write_private(path, bank, root)
            with self.assertRaises(ValueError):
                curate.write_private(root / 'public.json', bank, root)


if __name__ == '__main__':
    unittest.main()
