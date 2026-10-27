"""Synthetic archive key module, related to catalog labels but independently used."""
import re


def archive_key(title: str) -> str:
    words = re.findall(r"[a-z0-9]+", title.lower(), flags=re.ASCII)
    return "_".join(words)
