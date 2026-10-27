"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    words = re.findall(r"[a-z0-9]+", title.lower())
    return CURRENT_SEPARATOR.join(words)
