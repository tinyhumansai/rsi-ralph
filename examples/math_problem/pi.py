"""Estimate pi."""


def estimate():
    # Leibniz series: pi/4 = 1 - 1/3 + 1/5 - 1/7 + ...
    total = 0.0
    for k in range(1000):
        total += (-1) ** k / (2 * k + 1)
    return 4 * total
