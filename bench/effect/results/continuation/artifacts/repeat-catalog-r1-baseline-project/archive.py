"""Synthetic archive key module, related to catalog labels but independently used."""


def archive_key(title: str) -> str:
    words = "".join(
        character.lower()
        if character.isascii() and character.isalnum()
        else " " if character.isspace()
        else ""
        for character in title
    ).split()
    return "_".join(words)
