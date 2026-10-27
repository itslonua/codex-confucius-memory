"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    """Return lowercase ASCII words joined by underscores."""
    return "_".join(re.findall(r"[a-z0-9]+", title.lower()))
