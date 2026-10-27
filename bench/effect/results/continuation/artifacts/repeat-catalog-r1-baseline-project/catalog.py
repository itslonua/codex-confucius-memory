"""Tiny synthetic catalog package used by the memory effect benchmark."""


def slugify(label: str) -> str:
    """Return a lowercase ASCII slug with underscores between words."""
    words = "".join(
        character.lower()
        if character.isascii() and character.isalnum()
        else " " if character.isspace()
        else ""
        for character in label
    ).split()
    return "_".join(words)
