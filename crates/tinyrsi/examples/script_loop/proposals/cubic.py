"""Editable model for the curve-fitting harness.

A cubic polynomial fitted exactly by least squares (normal equations solved
with Gaussian elimination) instead of an undertrained straight line.
"""


def solve(matrix, vector):
    n = len(vector)
    rows = [row[:] + [value] for row, value in zip(matrix, vector)]
    for col in range(n):
        pivot = max(range(col, n), key=lambda r: abs(rows[r][col]))
        rows[col], rows[pivot] = rows[pivot], rows[col]
        for r in range(n):
            if r != col:
                factor = rows[r][col] / rows[col][col]
                rows[r] = [x - factor * p for x, p in zip(rows[r], rows[col])]
    return [rows[i][n] / rows[i][i] for i in range(n)]


def fit(ts, ys, degree=3):
    scale = max(ts)
    features = [[(t / scale) ** k for k in range(degree + 1)] for t in ts]
    gram = [[sum(f[i] * f[j] for f in features) for j in range(degree + 1)]
            for i in range(degree + 1)]
    moment = [sum(f[i] * y for f, y in zip(features, ys)) for i in range(degree + 1)]
    coef = solve(gram, moment)
    return lambda t: sum(c * (t / scale) ** k for k, c in enumerate(coef))
