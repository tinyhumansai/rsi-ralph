"""Editable model for the curve-fitting harness.

Predict the largest training target everywhere: robust to noise, but it ignores
the signal entirely.
"""


def fit(ts, ys):
    peak = max(ys)
    return lambda t: peak
