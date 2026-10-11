"""Fixed checker for the graph-colouring example (standard library only).

The loop never edits this file. For each seed it builds a random graph with a
hidden 6-colouring: 150 vertices are split into six secret classes and every
pair from different classes is joined with probability 0.35. Vertex labels and
edge order are shuffled, and the seed is removed from the environment before
the solver is imported, so the classes cannot be recovered from the input.

It calls ``coloring.color(n, edges)`` from the editable ``coloring.py`` and
checks the answer: one non-negative integer per vertex, with no edge joining
two vertices of the same colour. A valid colouring prints
``METRIC colors=<distinct colours used>`` (lower is better; 6 is optimal for
these graphs). An invalid colouring prints no metric, so the seed fails.
"""

import os
import random
import sys

VERTICES = 150
CLASSES = 6
DENSITY = 0.35


def planted_graph(seed):
    """Edges of a graph with a hidden CLASSES-colouring; the classes are discarded."""
    rng = random.Random(seed)
    secret = [rng.randrange(CLASSES) for _ in range(VERTICES)]
    labels = list(range(VERTICES))
    rng.shuffle(labels)
    edges = set()
    for i in range(VERTICES):
        for j in range(i + 1, VERTICES):
            if secret[i] != secret[j] and rng.random() < DENSITY:
                a, b = labels[i], labels[j]
                edges.add((min(a, b), max(a, b)))
    edges = sorted(edges)
    rng.shuffle(edges)
    return edges


def invalid(colors, edges):
    """Why a colouring is invalid, or None when it is a proper colouring."""
    if not isinstance(colors, (list, tuple)) or len(colors) != VERTICES:
        return f"expected a list of {VERTICES} colours"
    if any(not isinstance(c, int) or isinstance(c, bool) or c < 0 for c in colors):
        return "colours must be non-negative integers"
    for a, b in edges:
        if colors[a] == colors[b]:
            return f"edge ({a}, {b}) joins two vertices of colour {colors[a]}"
    return None


def main():
    seed = int(os.environ.pop("TINYRSI_SEED", "1"))
    edges = planted_graph(seed)
    del seed
    import coloring  # imported only after the seed is gone

    colors = coloring.color(VERTICES, [tuple(e) for e in edges])
    reason = invalid(colors, edges)
    if reason is not None:
        print(f"invalid colouring: {reason}", file=sys.stderr)
        sys.exit(1)
    print(f"METRIC colors={len(set(colors))}")


if __name__ == "__main__":
    main()
