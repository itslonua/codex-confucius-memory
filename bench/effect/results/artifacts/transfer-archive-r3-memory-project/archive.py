"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    words = re.sub(r"[^a-z0-9]+", " ", title.lower()).split()
    return "_".join(words)
