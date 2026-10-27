"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    """Return the same lowercase ASCII key format used by the catalog."""
    words = (re.sub(r"[^A-Za-z0-9]", "", part) for part in title.lower().split())
    return "_".join(word.lower() for word in words if word)
