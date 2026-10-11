"""Fixed checker for the file-upgrade example (standard library only).

The loop never edits this file. It first runs behavior tests against the
editable ``slug.py``: if any fails, it exits non-zero, so a rewrite that
changes behavior is rejected however tidy it looks. Then it counts violations
of the written standard and prints ``METRIC violations=<n>`` (lower is
better):

1. the module has a docstring;
2. every function has a docstring;
3. every parameter and every return value has a type annotation;
4. no bare ``except:``;
5. no line is longer than 79 characters.
"""

import ast
import sys

import slug

CASES = {
    "Hello, World!": "hello-world",
    "  spaced   out  ": "spaced-out",
    "Already-slugged": "already-slugged",
    "": "",
    "Ünïcödé 2026": "ünïcödé-2026",
}


def behaves():
    """True when slugify keeps its documented behavior."""
    return all(slug.slugify(text) == expected for text, expected in CASES.items())


def violations(source):
    """Number of breaches of the standard in ``source``."""
    tree = ast.parse(source)
    count = 0 if ast.get_docstring(tree) else 1
    for node in ast.walk(tree):
        if isinstance(node, ast.FunctionDef):
            count += 0 if ast.get_docstring(node) else 1
            args = node.args.posonlyargs + node.args.args + node.args.kwonlyargs
            count += sum(1 for arg in args if arg.annotation is None)
            count += 0 if node.returns is not None else 1
        if isinstance(node, ast.ExceptHandler) and node.type is None:
            count += 1
    count += sum(1 for line in source.splitlines() if len(line) > 79)
    return count


if __name__ == "__main__":
    if not behaves():
        print("behavior changed", file=sys.stderr)
        sys.exit(1)
    with open(slug.__file__, encoding="utf-8") as handle:
        print(f"METRIC violations={violations(handle.read())}")
