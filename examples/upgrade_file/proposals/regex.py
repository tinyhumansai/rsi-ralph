"""Turn free text into URL slugs."""

import re


def slugify(text: str) -> str:
    """Lowercase ``text`` and replace non-word runs with hyphens."""
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")
