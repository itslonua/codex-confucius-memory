"""Tiny synthetic catalog package used by the memory effect benchmark."""

import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII slug with underscores between words."""
    words = re.findall(r"[a-z0-9]+", label.lower())
    return "_".join(words)
