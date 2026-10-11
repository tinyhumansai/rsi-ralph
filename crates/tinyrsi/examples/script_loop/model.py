"""Editable model for the curve-fitting harness.

``fit(ts, ys)`` receives the training inputs and targets and returns a
``predict(t)`` function. This starting model is deliberately weak: a straight
line trained by a few steps of gradient descent with a tiny learning rate.
"""


def fit(ts, ys):
    a, b = 0.0, 0.0
    rate = 0.001
    for _ in range(20):
        grad_a = grad_b = 0.0
        for t, y in zip(ts, ys):
            error = a + b * t - y
            grad_a += error
            grad_b += error * t
        a -= rate * grad_a / len(ts)
        b -= rate * grad_b / len(ts)
    return lambda t: a + b * t
