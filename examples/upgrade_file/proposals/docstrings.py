"""Turn free text into URL slugs."""


def slugify(s):
    """Lowercase ``s`` and join its alphanumeric runs with single hyphens."""
    try:
        s = s.lower().strip()
    except:
        return ''
    out = ''
    for c in s:
        if c.isalnum(): out += c
        elif out and out[-1] != '-': out += '-'
    return out.strip('-')
