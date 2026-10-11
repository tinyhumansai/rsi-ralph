"""Fixed curve-fitting harness for the script-loop example (standard library only).

The loop never edits this file. It generates a noisy damped sinusoid from the
seed in TINYRSI_SEED, splits it into training and held-out points, calls
``model.fit`` on the training points, and prints the held-out RMSE as
``METRIC rmse=<value>``, after ``DIAG`` lines with the training RMSE and the
mean residual over ten slices of t, which the loop shows the supervisor. Only ``model.py`` is editable, so a proposal can
improve the model but cannot change the data or the evaluation.
"""

import math
import os
import random

import model


def damped_sinusoid(t):
    """The true signal the model should recover."""
    return 1.5 * math.exp(-0.25 * t) * math.sin(2.2 * t + 0.4) + 0.3


def dataset(seed, points=120, noise=0.05):
    """Deterministic noisy samples on [0, 10], split 80/20 into train and test."""
    rng = random.Random(seed)
    samples = []
    for i in range(points):
        t = 10.0 * i / (points - 1)
        samples.append((t, damped_sinusoid(t) + rng.gauss(0.0, noise)))
    rng.shuffle(samples)
    cut = int(0.8 * points)
    return samples[:cut], samples[cut:]


def main():
    seed = int(os.environ.get("TINYRSI_SEED", "1"))
    train, test = dataset(seed)
    predict = model.fit([t for t, _ in train], [y for _, y in train])
    error = sum((predict(t) - y) ** 2 for t, y in test) / len(test)
    train_error = sum((predict(t) - y) ** 2 for t, y in train) / len(train)
    print(f"DIAG train rmse {math.sqrt(train_error):.4f} (held-out {math.sqrt(error):.4f})")
    # Mean residual (target minus prediction) over ten equal slices of t on
    # all points: a sign pattern here is structure the model is missing.
    slices = [[] for _ in range(10)]
    for t, y in train + test:
        slices[min(9, int(t))].append(y - predict(t))
    profile = " ".join(f"{sum(r) / len(r):+.2f}" for r in slices)
    print(f"DIAG mean residual by t in [0,1),[1,2),...,[9,10]: {profile}")
    print(f"METRIC rmse={math.sqrt(error):.6f}")


if __name__ == "__main__":
    main()
