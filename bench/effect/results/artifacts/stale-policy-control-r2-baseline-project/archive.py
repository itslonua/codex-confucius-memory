"""Synthetic archive key module, related to catalog labels but independently used."""

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    parts = []
    current = []
    for character in title.strip().lower():
        if character.isalnum():
            current.append(character)
        elif current:
            parts.append("".join(current))
            current = []
    if current:
        parts.append("".join(current))
    return CURRENT_SEPARATOR.join(parts)
