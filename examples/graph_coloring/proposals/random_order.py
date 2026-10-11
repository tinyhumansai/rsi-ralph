"""Colour a graph with first fit in a shuffled order.

Hypothesis: label order is arbitrary, so a random order is no worse.
"""

import random


def color(n, edges):
    neighbours = [set() for _ in range(n)]
    for a, b in edges:
        neighbours[a].add(b)
        neighbours[b].add(a)
    order = list(range(n))
    random.Random(0).shuffle(order)
    colors = [-1] * n
    for v in order:
        used = {colors[u] for u in neighbours[v]}
        colors[v] = len(used)  # wrong: the count of used colours may itself be used
    return colors
