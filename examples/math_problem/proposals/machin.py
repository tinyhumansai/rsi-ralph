"""Estimate pi with Machin's formula.

pi/4 = 4 arctan(1/5) - arctan(1/239). Each arctan(1/x) is the alternating
series sum of (-1)^k / ((2k + 1) x^(2k + 1)), whose terms shrink by a factor
of x^2 per step, so twenty terms already exceed double precision. Every
step is exact arithmetic on that identity; no constant is copied in.
"""


def arctan_inverse(x, terms=20):
    """arctan(1/x) by its alternating power series."""
    total = 0.0
    power = 1.0 / x
    for k in range(terms):
        total += (-1) ** k * power / (2 * k + 1)
        power /= x * x
    return total


def estimate():
    return 4 * (4 * arctan_inverse(5) - arctan_inverse(239))
