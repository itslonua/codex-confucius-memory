"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    """Return the normalized archive key for a title."""
    words = title.strip().split()
    return "_".join(
        "".join(char.lower() for char in word if char.isascii() and char.isalnum())
        for word in words
        if any(char.isascii() and char.isalnum() for char in word)
    )
