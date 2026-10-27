"""Tiny synthetic catalog package used by the memory effect benchmark."""


def slugify(label: str) -> str:
    """Return a lowercase, underscore-separated key without punctuation."""
    words = label.strip().split()
    return "_".join(
        "".join(char.lower() for char in word if char.isascii() and char.isalnum())
        for word in words
        if any(char.isascii() and char.isalnum() for char in word)
    )
