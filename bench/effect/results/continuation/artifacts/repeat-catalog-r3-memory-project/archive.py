"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    """Return a lowercase ASCII key with punctuation removed and words separated."""
    words = [
        "".join(char for char in word.lower() if char.isascii() and char.isalnum())
        for word in title.split()
    ]
    return "_".join(word for word in words if word)
