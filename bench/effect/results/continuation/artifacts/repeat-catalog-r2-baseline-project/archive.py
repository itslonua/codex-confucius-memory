"""Synthetic archive key module, related to catalog labels but independently used."""
import re


def archive_key(title: str) -> str:
    return re.sub(r"[^a-z0-9]+", "_", title.lower()).strip("_")
