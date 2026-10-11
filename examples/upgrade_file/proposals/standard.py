"""Turn free text into URL slugs.

A slug is the lowercase text with every run of characters that are not
letters or digits collapsed into a single hyphen, and no hyphen at either
end. Letters outside ASCII are kept.
"""


def slugify(text: str) -> str:
    """Return the slug of ``text``; an empty or blank string gives ``""``."""
    words: list[str] = []
    current: list[str] = []
    for char in text.lower():
        if char.isalnum():
            current.append(char)
        elif current:
            words.append("".join(current))
            current = []
    if current:
        words.append("".join(current))
    return "-".join(words)
