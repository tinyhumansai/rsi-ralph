"""Behavior gate for the file-upgrade loop (standard library only).

The loop never edits this file. It runs before any score: a rewrite of
``slug.py`` that changes what ``slugify`` returns exits non-zero and is
rejected however tidy it looks.
"""

import os
import sys

CASES = {
    "Hello, World!": "hello-world",
    "  spaced   out  ": "spaced-out",
    "Already-slugged": "already-slugged",
    "": "",
    "Ünïcödé 2026": "ünïcödé-2026",
}

if __name__ == "__main__":
    sys.path.insert(0, os.environ["RSI_CANDIDATE"])
    import slug

    for text, expected in CASES.items():
        got = slug.slugify(text)
        if got != expected:
            print(f"slugify({text!r}) returned {got!r}, expected {expected!r}", file=sys.stderr)
            sys.exit(1)
