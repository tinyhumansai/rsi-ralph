"""Generate host-private numerical tasks with independently checked answers."""
import argparse
import hashlib
import json
import os
import pathlib
import random

import oracles

STAGES = ('screen', 'selection', 'confirmation', 'anchor')
FAMILIES = ('assignment', 'harshad', 'idempotents', 'euclid')


def variant(family, rng, small):
    """Return an original parameterized prompt and two independent answers."""
    if family == 'assignment':
        n = rng.randrange(4, 7) if small else rng.randrange(11, 16)
        matrix = [[rng.randrange(-100000, 100000) for _ in range(n)] for _ in range(n)]
        prompt = ('Choose exactly one entry from each row and each column of the '
                  'following square integer matrix to maximize their sum. Negative '
                  'entries are allowed. Matrix: ' + json.dumps(matrix))
        answers = oracles.assignment_dp(matrix), oracles.assignment_matching(matrix)
    elif family == 'harshad':
        limit = rng.randrange(1000, 5000) if small else rng.randrange(50000, 200000)
        prompt = (f'Sum all strong right-truncatable Harshad primes strictly below {limit}. '
                  'A Harshad integer is positive and divisible by its decimal digit sum. '
                  'Right-truncatable means every nonempty prefix is Harshad. Strong '
                  'means its quotient by the digit sum is prime. A strong '
                  'right-truncatable Harshad prime is a prime of at least two digits '
                  'whose prefix obtained by deleting the last digit is both strong '
                  'and right-truncatable Harshad.')
        answers = oracles.harshad_tree(limit), oracles.harshad_sieve(limit)
    elif family == 'idempotents':
        limit = rng.randrange(50, 150) if small else rng.randrange(700, 2000)
        prompt = (f'For each integer n from 2 through {limit} inclusive, define M(n) '
                  'as the greatest integer a with 0 <= a < n and a*a congruent '
                  'to a modulo n. Find the sum of M(n).')
        answers = oracles.idempotents_crt(limit), oracles.idempotents_brute(limit)
    else:
        limit = rng.randrange(20, 60) if small else rng.randrange(250, 700)
        prompt = (f'For every ordered integer pair (x,y) with 1 <= x,y <= {limit}, '
                  'run a=x,b=y; while b is nonzero, replace (a,b) by (b,a mod b). '
                  'Count every replacement, including the one producing b=0, '
                  'and sum these counts over all ordered pairs.')
        answers = oracles.euclid_forward(limit), oracles.euclid_reverse(limit)
    if answers[0] != answers[1]:
        raise ValueError('independent oracles disagree')
    return prompt + '\nReturn only the exact integer followed by a newline.', str(answers[0]) + '\n'


def generate(seed, repeats=3, small=False):
    """Freeze disjoint draws from a private 256-bit seed, retaining seed provenance."""
    if len(seed) != 32 or not 1 <= repeats <= 10:
        raise ValueError('expected 32-byte seed and 1..10 repeats')
    rng = random.Random(int.from_bytes(seed, 'big'))
    stages, seen_prompts, seen_seeds = {}, set(), set()
    for stage in STAGES:
        rows = []
        for family in FAMILIES:
            for index in range(repeats):
                for _ in range(1000):
                    prompt, expected = variant(family, rng, small)
                    if prompt not in seen_prompts:
                        break
                else:
                    raise ValueError('could not draw a fresh task')
                seen_prompts.add(prompt)
                execution_seed = rng.getrandbits(64)
                while execution_seed in seen_seeds:
                    execution_seed = rng.getrandbits(64)
                seen_seeds.add(execution_seed)
                rows.append(dict(id=f'{stage}-{family}-{index}', family=family,
                                 seat='solver', prompt=prompt, expected=expected,
                                 seeds=[execution_seed]))
        stages[stage] = rows
    calibration = []
    for rows in stages.values():
        for row in rows:
            calibration.extend([
                dict(task_id=row['id'], artifact=row['expected'], passed=True),
                dict(task_id=row['id'], artifact=str(int(row['expected']) + 1) + '\n', passed=False),
            ])
    return dict(version=1, provenance='agent-authored; two independent integer oracles',
                private_seed=seed.hex(), stages=stages, calibration=calibration)


def summary(bank):
    """Expose counts and opaque bank identity, without prompts, answers or seeds."""
    digest = hashlib.sha256(json.dumps(bank, sort_keys=True).encode()).hexdigest()
    return dict(version=1, bank_sha256=digest, provenance=bank['provenance'],
                task_count=sum(map(len, bank['stages'].values())),
                stages={stage: len(rows) for stage, rows in bank['stages'].items()},
                families=list(FAMILIES), calibration_samples=len(bank['calibration']),
                human_labels=False, measured_anchors=False)


def write_private(path, bank, root):
    """Create an immutable private bank only below this checkout's target directory."""
    path, allowed = path.resolve(), (root / 'target').resolve()
    if not path.is_relative_to(allowed):
        raise ValueError('private bank must be below checkout target/')
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, 'w') as stream:
        json.dump(bank, stream, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())


def main():
    """Generate fresh private inputs and print only safe curation metadata."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    parser.add_argument('--repeats', type=int, default=3)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    bank = generate(os.urandom(32), args.repeats)
    write_private(args.output, bank, root)
    print(json.dumps(summary(bank), indent=2))


if __name__ == '__main__':
    main()
