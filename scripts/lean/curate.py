"""Create a host-private finite Lean slice; kernel builds independently check truth."""
import argparse
import json
import os
import pathlib
import random


def generate(seed):
    """Draw fresh Euclidean division, idempotent and modular-inverse proof goals."""
    if len(seed) != 32:
        raise ValueError('expected 32-byte private seed')
    rng = random.Random(int.from_bytes(seed, 'big'))
    stages, used_seeds = {}, set()
    for stage in ('screen', 'selection', 'confirmation', 'anchor'):
        a, b = rng.randrange(10000, 1000000), rng.randrange(20, 200)
        q, r = divmod(a, b)
        p, other = rng.sample([7, 11, 13, 17, 19, 23, 29, 31], 2)
        n = p * other
        residue = next(x for x in range(n - 1, 0, -1) if (x * x - x) % n == 0)
        modulus = rng.choice([101, 103, 107, 109, 113, 127, 131, 137])
        unit = rng.randrange(2, modulus)
        inverse = pow(unit, -1, modulus)
        statements = [
            ('euclid', f'({q} * {b} + {r} : Nat) = {a} ∧ {r} < {b}'),
            ('idempotent', f'({residue} * {residue} % {n} : Nat) = {residue}'),
            ('inverse', f'({unit} * {inverse} % {modulus} : Nat) = 1'),
        ]
        rows = []
        for family, statement in statements:
            execution_seed = rng.getrandbits(64)
            while execution_seed in used_seeds:
                execution_seed = rng.getrandbits(64)
            used_seeds.add(execution_seed)
            rows.append(dict(id=f'{stage}-lean-{family}', seed=execution_seed,
                             theorem=f'import Std\ntheorem rsi_goal : {statement} :='))
        stages[stage] = rows
    return dict(version=1, private_seed=seed.hex(), toolchain='leanprover/lean4:v4.34.1', stages=stages)


def write_private(path, bank, root):
    """Refuse public output and overwrite; retain private replay seed with mode 0600."""
    path = path.resolve()
    if not path.is_relative_to((root / 'target').resolve()):
        raise ValueError('private Lean slice must be below checkout target/')
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, 'w') as stream:
        json.dump(bank, stream, indent=2)
        stream.write('\n')
        stream.flush()
        os.fsync(stream.fileno())


def main():
    """Generate private inputs; print no prompt, theorem, answer or seed."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=pathlib.Path, required=True)
    args = parser.parse_args()
    root = pathlib.Path(__file__).resolve().parents[2]
    bank = generate(os.urandom(32))
    write_private(args.output, bank, root)
    print(json.dumps(dict(version=1, provenance='agent-authored finite mathematics',
                          task_count=12, stages=4, toolchain=bank['toolchain'])))


if __name__ == '__main__':
    main()
