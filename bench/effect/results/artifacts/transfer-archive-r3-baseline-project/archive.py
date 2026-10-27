"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    cleaned = "".join(
        char.lower()
        for char in title
        if char.isspace() or (char.isascii() and char.isalnum())
    )
    return "_".join(cleaned.split())
