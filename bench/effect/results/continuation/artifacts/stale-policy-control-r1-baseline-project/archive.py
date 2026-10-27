"""Synthetic archive key module, related to catalog labels but independently used."""

import unicodedata

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    parts = []
    current = []

    for char in title.strip().lower():
        if char.isspace() or unicodedata.category(char).startswith("P"):
            if current:
                parts.append("".join(current))
                current = []
        else:
            current.append(char)

    if current:
        parts.append("".join(current))

    return CURRENT_SEPARATOR.join(parts)
