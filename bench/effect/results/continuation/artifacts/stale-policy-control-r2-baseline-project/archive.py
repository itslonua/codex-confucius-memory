"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    key = re.sub(r"[^\w]+", CURRENT_SEPARATOR, title.strip().lower())
    separator = re.escape(CURRENT_SEPARATOR)
    return re.sub(f"{separator}+", CURRENT_SEPARATOR, key).strip(CURRENT_SEPARATOR)
