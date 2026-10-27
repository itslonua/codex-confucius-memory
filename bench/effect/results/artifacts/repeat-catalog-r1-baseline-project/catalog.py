"""Tiny synthetic catalog package used by the memory effect benchmark."""

import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII slug with underscores between words."""
    words = (re.sub(r"[^A-Za-z0-9]", "", part) for part in label.lower().split())
    return "_".join(word.lower() for word in words if word)
