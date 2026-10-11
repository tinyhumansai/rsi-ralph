"""Fixed checker for the math example (standard library only).

The loop never edits this file. It calls ``pi.estimate()`` from the editable
``pi.py``, compares the result with the true value of pi, and prints how many
decimal digits are correct as ``METRIC digits=<value>`` (higher is better,
capped at 15, the precision of a double). The estimate must come back within
the time limit the loop enforces, so brute force has a cost.
"""

import math
import os

import pi

TRUE_PI = math.pi


def correct_digits(estimate):
    """Decimal digits of agreement with pi: -log10 of the absolute error."""
    error = abs(float(estimate) - TRUE_PI)
    if error == 0.0:
        return 15.0
    return max(0.0, min(15.0, -math.log10(error)))


if __name__ == "__main__":
    # The seed is unused: the estimate is deterministic, so every seed agrees.
    os.environ.get("TINYRSI_SEED")
    print(f"METRIC digits={correct_digits(pi.estimate()):.6f}")
