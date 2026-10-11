"""Colour a graph with DSATUR (Brelaz, 1979).

Repeatedly colour the uncoloured vertex whose neighbours already use the most
distinct colours (its saturation), breaking ties by degree, with the smallest
free colour. Constrained vertices go first, so fewer colours are needed than
with first fit in label order.
"""


def color(n, edges):
    neighbours = [set() for _ in range(n)]
    for a, b in edges:
        neighbours[a].add(b)
        neighbours[b].add(a)
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
