"""Score function for the pi loop (standard library only).

The loop never edits this file. It imports ``pi.estimate()`` from the candidate
folder in ``$RSI_CANDIDATE``, compares the result with the true value of pi and
prints how many decimal digits agree as ``METRIC digits=<value>`` (higher is
better, capped at 15, the precision of a double). The step timeout bounds the
run, so brute force has a cost. The estimate is deterministic, so every seed
agrees.
"""

import math
import os
import sys


def correct_digits(estimate):
    """Decimal digits of agreement with pi: -log10 of the absolute error."""
    error = abs(float(estimate) - math.pi)
    if error == 0.0:
        return 15.0
    return max(0.0, min(15.0, -math.log10(error)))


if __name__ == "__main__":
    sys.path.insert(0, os.environ["RSI_CANDIDATE"])
    import pi

    print(f"METRIC digits={correct_digits(pi.estimate()):.6f}")
