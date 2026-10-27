"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    normalized = re.sub(r"[^\w]+", CURRENT_SEPARATOR, title.strip().lower(), flags=re.UNICODE)
    normalized = re.sub(r"_+", CURRENT_SEPARATOR, normalized)
    return normalized.strip(CURRENT_SEPARATOR)
