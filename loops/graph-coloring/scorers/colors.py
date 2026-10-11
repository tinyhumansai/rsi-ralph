"""Score function for the graph-colouring loop (standard library only).

The loop never edits this file. For each seed it builds a random graph with a
hidden 6-colouring: 150 vertices are split into six secret classes and every
pair from different classes is joined with probability 0.35. Vertex labels and
edge order are shuffled, and the seed is removed from the environment before
the solver is imported, so the classes cannot be recovered from the input.

It imports ``coloring.color(n, edges)`` from the candidate folder in
``$RSI_CANDIDATE`` (the seed comes from ``$RSI_SEED``) and
checks the answer: one non-negative integer per vertex, with no edge joining
two vertices of the same colour. A valid colouring prints
``METRIC colors=<distinct colours used>`` (lower is better; 6 is optimal for
these graphs). An invalid colouring prints no metric, so the seed fails.

It also prints ``DIAG`` lines the loop shows the supervisor: the graph's size,
the solver's run time, the colour class sizes, and for an invalid colouring
how many edges conflict. ``DIAG`` lines the solver prints itself pass through.
"""

import os
import random
import sys
import time

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
    clashes = [(a, b) for a, b in edges if colors[a] == colors[b]]
    if clashes:
        a, b = clashes[0]
        return (
            f"{len(clashes)} of {len(edges)} edges join same-coloured vertices, "
            f"first ({a}, {b}) with colour {colors[a]}"
        )
    return None


def main():
    seed = int(os.environ.pop("RSI_SEED", "1"))
    edges = planted_graph(seed)
    del seed
    sys.path.insert(0, os.environ["RSI_CANDIDATE"])
    import coloring  # imported only after the seed is gone

    degrees = [0] * VERTICES
    for a, b in edges:
        degrees[a] += 1
        degrees[b] += 1
    print(f"DIAG graph: {VERTICES} vertices, {len(edges)} edges, max degree {max(degrees)}")
    started = time.monotonic()
    colors = coloring.color(VERTICES, [tuple(e) for e in edges])
    print(f"DIAG color() took {time.monotonic() - started:.2f}s")
    reason = invalid(colors, edges)
    if reason is not None:
        print(f"DIAG invalid colouring: {reason}")
        print(f"invalid colouring: {reason}", file=sys.stderr)
        sys.exit(1)
    sizes = sorted((colors.count(c) for c in set(colors)), reverse=True)
    print(f"DIAG {len(sizes)} colour classes, sizes {sizes}")
    print(f"METRIC colors={len(set(colors))}")


if __name__ == "__main__":
    main()
