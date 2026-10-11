"""Estimate pi with the Leibniz series.

pi/4 = sum over k of (-1)^k / (2k + 1). The error after n terms is about
1/n, so each tenfold increase in terms buys one more correct digit.
"""


def estimate():
    total = 0.0
    for k in range(200_000):
        total += (-1) ** k / (2 * k + 1)
    return 4 * total
