"""Synthetic archive key module, related to catalog labels but independently used."""

import unicodedata

import policy


def archive_key(title: str) -> str:
    normalized = "".join(
        character
        for character in title.strip().lower()
        if not unicodedata.category(character).startswith("P")
    )
    return policy.CURRENT_SEPARATOR.join(normalized.split())
