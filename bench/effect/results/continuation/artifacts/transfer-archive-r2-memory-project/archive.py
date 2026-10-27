"""Synthetic archive key module, related to catalog labels but independently used."""

import re


def archive_key(title: str) -> str:
    ascii_title = title.encode("ascii", "ignore").decode("ascii").lower()
    without_punctuation = re.sub(r"[^a-z0-9\s]", "", ascii_title)
    return "_".join(without_punctuation.split())
