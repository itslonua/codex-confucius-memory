"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    """Return a lowercase ASCII key with punctuation removed."""
    cleaned = re.sub(r"[^a-z0-9\s]", "", title.lower())
    return "_".join(cleaned.split())
