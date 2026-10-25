def slugify(s):
    try:
        s = s.lower().strip()
    except:
        return ''
    out = ''
    for c in s:
        if c.isalnum(): out += c
        elif out and out[-1] != '-': out += '-'
    return out.strip('-')
