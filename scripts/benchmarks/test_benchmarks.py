"""Deterministic oracle, privacy and frozen-bank contract tests."""
import itertools
import json
import pathlib
import random
import tempfile
import unittest
from unittest import mock
import contextlib
import io

import curate
import oracles
import catalog


class OracleTests(unittest.TestCase):
    """Exercise independently implemented ground-truth algorithms."""

    def test_assignment_including_negative_entries_and_ties(self):
        for matrix in ([[7]], [[-2, -1], [-4, -3]], [[1, 1], [1, 1]]):
            brute = max(sum(row[j] for row, j in zip(matrix, p))
                        for p in itertools.permutations(range(len(matrix))))
            self.assertEqual(oracles.assignment_dp(matrix), brute)
            self.assertEqual(oracles.assignment_matching(matrix), brute)
        rng = random.Random(41)
        for n in range(2, 8):
            matrix = [[rng.randrange(-100, 100) for _ in range(n)] for _ in range(n)]
            self.assertEqual(oracles.assignment_dp(matrix),
                             oracles.assignment_matching(matrix))

    def test_harshad_prime_definition_and_strict_limit(self):
        self.assertEqual(oracles.harshad_tree(10000), 90619)
        for limit in (1, 10, 100, 181, 182, 1000, 10000, 18317):
            self.assertEqual(oracles.harshad_tree(limit), oracles.harshad_sieve(limit))
        self.assertEqual(oracles.harshad_tree(181), 0)
        self.assertEqual(oracles.harshad_tree(182), 181)

    def test_idempotents_include_one_and_prime_powers(self):
        self.assertEqual(oracles.idempotents_crt(6), 8)
        for limit in (2, 6, 8, 20, 101, 237):
            self.assertEqual(oracles.idempotents_crt(limit),
                             oracles.idempotents_brute(limit))

    def test_euclid_steps_count_terminal_division_and_order(self):
        self.assertEqual(oracles.euclid_forward(1), 1)
        self.assertEqual(oracles.euclid_forward(10), 221)
        for limit in (1, 2, 5, 10, 31, 76):
            self.assertEqual(oracles.euclid_forward(limit), oracles.euclid_reverse(limit))


class BankTests(unittest.TestCase):
    """Pin private generation and sanitized evidence behavior."""

    def test_distinct_stages_reproducible_private_draws_and_no_answers_in_summary(self):
        bank = curate.generate(bytes(range(32)), repeats=1, small=True)
        self.assertEqual(bank, curate.generate(bytes(range(32)), repeats=1, small=True))
        self.assertNotEqual(bank, curate.generate(bytes(reversed(range(32))), 1, True))
        self.assertEqual(set(bank['stages']), {'screen', 'selection', 'confirmation', 'anchor'})
        rows = [row for stage in bank['stages'].values() for row in stage]
        self.assertEqual(len({row['id'] for row in rows}), 16)
        self.assertEqual(len({row['seeds'][0] for row in rows}), 16)
        self.assertEqual(len({row['prompt'] for row in rows}), 16)
        self.assertTrue(all(row['expected'].endswith('\n') for row in rows))
        expected = {row['id']: row['expected'] for row in rows}
        self.assertEqual(len(bank['calibration']), 32)
        for sample in bank['calibration']:
            self.assertEqual(sample['passed'], sample['artifact'] == expected[sample['task_id']])
        public = curate.summary(bank)
        self.assertEqual(public['task_count'], 16)
        self.assertNotIn('prompt', json.dumps(public))
        self.assertNotIn('expected', json.dumps(public))
        self.assertNotIn('private_seed', json.dumps(public))

    def test_refuses_public_or_existing_destination_and_preserves_private_permissions(self):
        bank = curate.generate(b'z' * 32, 1, True)
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            with self.assertRaises(ValueError):
                curate.write_private(root / 'public.json', bank, root)
            path = root / 'target' / 'bank.json'
            curate.write_private(path, bank, root)
            self.assertEqual(path.stat().st_mode & 0o777, 0o600)
            with self.assertRaises(FileExistsError):
                curate.write_private(path, bank, root)
            self.assertEqual(json.loads(path.read_text()), bank)

    def test_rejects_invalid_seed_or_trial_count(self):
        for seed, repeats in ((b'', 1), (b'a' * 32, 0), (b'a' * 32, 100)):
            with self.assertRaises(ValueError):
                curate.generate(seed, repeats, True)

    def test_source_catalog_requires_problem_metadata_solution_and_answer(self):
        page = '<title>#433 Steps in Euclid’s Algorithm - Project Euler</title>Difficulty: Level 36 [92%]'
        source = b'Solution to Project Euler problem 433'
        answers = 'Problem 433: 326624372659664\n'
        row = catalog.parse_problem(433, page, source, answers, 'java')
        self.assertEqual(row['difficulty_percent'], 92)
        self.assertFalse(row['reference_executed'])
        self.assertIn(catalog.REVISION, row['solution_url'])
        for bad_page, bad_source, bad_answers in (
                ('', source, answers), (page.replace('#433', '#434'), source, answers),
                (page, b'wrong problem', answers),
                (page, source, 'Problem 434: 1\n')):
            with self.assertRaises(ValueError):
                catalog.parse_problem(433, bad_page, bad_source, bad_answers, 'java')

    def test_catalog_cli_fetches_all_pinned_controls_without_executing_sources(self):
        answers = ''.join(f'Problem {n}: {n}\n' for n in catalog.PROBLEMS)

        def fetch(url):
            if url.endswith('Answers.txt'):
                return answers.encode()
            if 'projecteuler.net' in url:
                number = url.rsplit('=', 1)[1]
                return f'<title>#{number} Example - Project Euler</title>Difficulty: Level 36 [92%]'.encode()
            number = int(url.rsplit('/p', 1)[1].split('.')[0])
            return f'Solution to Project Euler problem {number}'.encode()

        output = io.StringIO()
        with mock.patch('catalog.fetch', side_effect=fetch), contextlib.redirect_stdout(output):
            catalog.main()
        self.assertEqual(len(json.loads(output.getvalue())['problems']), 9)
        with mock.patch('catalog.urllib.request.urlopen', return_value=io.BytesIO(b'public')):
            self.assertEqual(catalog.fetch('https://example.com'), b'public')
        with mock.patch('catalog.urllib.request.urlopen', return_value=io.BytesIO(b'x' * 1000001)):
            with self.assertRaises(ValueError):
                catalog.fetch('https://example.com')

    def test_generator_cli_writes_private_inputs_and_prints_only_metadata(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            path = root / 'target' / 'private' / 'bank.json'
            output = io.StringIO()
            with mock.patch('curate.__file__', str(root / 'scripts' / 'benchmarks' / 'curate.py')), \
                    mock.patch('curate.os.urandom', return_value=b'm' * 32), \
                    mock.patch('sys.argv', ['curate', '--output', str(path), '--repeats', '1']), \
                    contextlib.redirect_stdout(output):
                curate.main()
            self.assertEqual(json.loads(output.getvalue())['task_count'], 16)
            self.assertEqual(len(json.loads(path.read_text())['calibration']), 32)
            self.assertNotIn('artifact', output.getvalue())


if __name__ == '__main__':
    unittest.main()
