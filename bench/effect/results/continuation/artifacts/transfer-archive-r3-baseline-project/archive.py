"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    words = []
    current = []
    for char in title.lower():
        if char.isascii() and char.isalnum():
            current.append(char)
        elif current:
            words.append("".join(current))
            current = []
    if current:
        words.append("".join(current))
    return "_".join(words)
