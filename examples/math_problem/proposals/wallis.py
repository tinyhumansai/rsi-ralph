"""Estimate pi with the Wallis product.

pi/2 = product over k of (4k^2) / (4k^2 - 1).
"""


def estimate():
    product = 1.0
    for k in range(1, 50):
        product *= (4 * k * k) / (4 * k * k - 1)
    return 2 * product
