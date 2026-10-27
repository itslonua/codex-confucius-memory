"""Tiny synthetic catalog package used by the memory effect benchmark."""


def slugify(label: str) -> str:
    """Return a lowercase ASCII key with punctuation removed and words separated."""
    words = [
        "".join(char for char in word.lower() if char.isascii() and char.isalnum())
        for word in label.split()
    ]
    return "_".join(word for word in words if word)
