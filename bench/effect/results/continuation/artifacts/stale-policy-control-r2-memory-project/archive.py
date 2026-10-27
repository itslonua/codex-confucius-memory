"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    words = re.sub(r"[^\w]+", " ", title.strip().lower()).split()
    return CURRENT_SEPARATOR.join(words)
