"""Colour a graph with DSATUR, then shrink the palette with TabuCol.

DSATUR gives a valid k-colouring. TabuCol (Hertz and de Werra, 1987) then
tries k - 1 colours: start from the k-colouring with the top colour folded
into the others, and repeatedly make the best single recolouring of a
conflicting vertex, forbidding the reverse move for a few iterations so the
search can leave local minima. When the conflicts reach zero, the palette is
one smaller and the search repeats; when an iteration budget runs out, the
last valid colouring is returned.
"""

import random


def dsatur(n, neighbours):
    colors = [-1] * n
    seen = [set() for _ in range(n)]
    for _ in range(n):
        v = max(
            (u for u in range(n) if colors[u] < 0),
            key=lambda u: (len(seen[u]), len(neighbours[u])),
        )
        c = 0
        while c in seen[v]:
            c += 1
        colors[v] = c
        for u in neighbours[v]:
            seen[u].add(c)
    return colors


def tabucol(n, neighbours, start, k, rng, budget):
    """A proper k-colouring reached from ``start``, or None within ``budget`` moves."""
    c = [x if x < k else rng.randrange(k) for x in start]
    clash = [[0] * k for _ in range(n)]
    for v in range(n):
        for u in neighbours[v]:
            clash[v][c[u]] += 1
    conflicts = sum(clash[v][c[v]] for v in range(n)) // 2
    tabu = {}
    for step in range(budget):
        if conflicts == 0:
            return c
        best, delta = None, None
        for v in range(n):
            here = clash[v][c[v]]
            if here == 0:
                continue
            for x in range(k):
                if x == c[v]:
                    continue
                d = clash[v][x] - here
                allowed = tabu.get((v, x), -1) < step or conflicts + d == 0
                if allowed and (delta is None or d < delta):
                    best, delta = (v, x), d
        if best is None:
            continue
        v, x = best
        old, c[v] = c[v], x
        conflicts += delta
        for u in neighbours[v]:
            clash[u][old] -= 1
            clash[u][x] += 1
        tabu[(v, old)] = step + int(0.6 * conflicts) + rng.randrange(10)
    return None


def color(n, edges):
    neighbours = [[] for _ in range(n)]
    for a, b in edges:
        neighbours[a].append(b)
        neighbours[b].append(a)
    rng = random.Random(0)
    best = dsatur(n, neighbours)
    while True:
        k = max(best)  # try one colour fewer than the current palette
        if k == 0:
            return best
        smaller = tabucol(n, neighbours, best, k, rng, budget=20000)
        if smaller is None:
            return best
        best = smaller
