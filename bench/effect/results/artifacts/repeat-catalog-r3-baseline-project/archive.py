"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    """Return a lowercase ASCII key with words separated by underscores."""
    words = []
    current = []
    for character in title.lower():
        if character in "abcdefghijklmnopqrstuvwxyz0123456789":
            current.append(character)
        elif character.isspace():
            if current:
                words.append("".join(current))
                current = []
        # Punctuation and non-ASCII characters are removed.
    if current:
        words.append("".join(current))
    return "_".join(words)
