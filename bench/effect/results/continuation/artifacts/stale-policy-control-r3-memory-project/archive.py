"""Synthetic archive key module, related to catalog labels but independently used."""

import re

from policy import CURRENT_SEPARATOR


def archive_key(title: str) -> str:
    return re.sub(r"[\W_]+", CURRENT_SEPARATOR, title.strip().lower()).strip(
        CURRENT_SEPARATOR
    )
