"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    normalized = title.strip().lower()
    return re.sub(r"[^\w]+", CURRENT_SEPARATOR, normalized).strip(CURRENT_SEPARATOR)
