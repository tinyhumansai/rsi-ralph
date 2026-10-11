"""Colour a graph so that no edge joins two vertices of the same colour."""


def color(n, edges):
    # First fit in label order: each vertex takes the smallest colour unused
    # by its already-coloured neighbours.
    neighbours = [set() for _ in range(n)]
    for a, b in edges:
        neighbours[a].add(b)
        neighbours[b].add(a)
    colors = [-1] * n
    for v in range(n):
        used = {colors[u] for u in neighbours[v]}
        c = 0
        while c in used:
            c += 1
        colors[v] = c
    return colors
