"""Synthetic archive key module, related to catalog labels but independently used."""
import re


def archive_key(title: str) -> str:
    """Return a lowercase ASCII key with words separated by underscores."""
    return re.sub(r"[^a-z0-9]+", "_", title.lower()).strip("_")
