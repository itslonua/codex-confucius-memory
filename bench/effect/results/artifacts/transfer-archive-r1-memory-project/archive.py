"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    # Treat punctuation and non-ASCII characters as word boundaries so the
    # resulting key contains only lowercase ASCII letters, digits, and `_`.
    return re.sub(r"[^a-z0-9]+", "_", title.lower()).strip("_")
