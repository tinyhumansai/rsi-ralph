"""Editable model for the curve-fitting harness.

Fit the right model family: a damped sinusoid plus offset,
``y = exp(-k t) * (a sin(w t) + b cos(w t)) + c``. For fixed decay ``k`` and
frequency ``w`` the model is linear in ``a, b, c``, so a coarse-to-fine grid
search over ``(k, w)`` with exact least squares for the rest finds the fit.
"""

import math


def solve(matrix, vector):
    n = len(vector)
    rows = [row[:] + [value] for row, value in zip(matrix, vector)]
    for col in range(n):
        pivot = max(range(col, n), key=lambda r: abs(rows[r][col]))
        rows[col], rows[pivot] = rows[pivot], rows[col]
        if abs(rows[col][col]) < 1e-12:
            return None
        for r in range(n):
            if r != col:
                factor = rows[r][col] / rows[col][col]
                rows[r] = [x - factor * p for x, p in zip(rows[r], rows[col])]
    return [rows[i][n] / rows[i][i] for i in range(n)]


def basis(t, k, w):
    envelope = math.exp(-k * t)
    return (envelope * math.sin(w * t), envelope * math.cos(w * t), 1.0)


def least_squares(ts, ys, k, w):
    features = [basis(t, k, w) for t in ts]
    gram = [[sum(f[i] * f[j] for f in features) for j in range(3)] for i in range(3)]
    moment = [sum(f[i] * y for f, y in zip(features, ys)) for i in range(3)]
    coef = solve(gram, moment)
    if coef is None:
        return None, math.inf
    sse = sum((sum(c * x for c, x in zip(coef, f)) - y) ** 2 for f, y in zip(features, ys))
    return coef, sse


def fit(ts, ys):
    best = (math.inf, 0.0, 1.0, None)
    grid = [(k / 20, w / 10) for k in range(0, 21) for w in range(5, 41)]
    for step in (None, 0.02):
        if step is not None:
            _, k0, w0, _ = best
            grid = [(max(0.0, k0 + dk * step / 2), w0 + dw * step)
                    for dk in range(-5, 6) for dw in range(-5, 6)]
        for k, w in grid:
            coef, sse = least_squares(ts, ys, k, w)
            if sse < best[0]:
                best = (sse, k, w, coef)
    _, k, w, coef = best
    return lambda t: sum(c * x for c, x in zip(coef, basis(t, k, w)))
