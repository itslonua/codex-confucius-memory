"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    return "_".join(title.strip().lower().split())
