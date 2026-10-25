"""Score function for the file-upgrade loop (standard library only).

The loop never edits this file. It counts breaches of the written standard in
the candidate's ``slug.py`` and prints ``METRIC violations=<n>`` (lower is
better):

1. the module has a docstring;
2. every function has a docstring;
3. every parameter and every return value has a type annotation;
4. no bare ``except:``;
5. no line is longer than 79 characters.
"""

import ast
import os


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
    path = os.path.join(os.environ["RSI_CANDIDATE"], "slug.py")
    with open(path, encoding="utf-8") as handle:
        print(f"METRIC violations={violations(handle.read())}")
