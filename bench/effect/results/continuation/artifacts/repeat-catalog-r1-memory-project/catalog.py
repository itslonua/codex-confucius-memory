"""Tiny synthetic catalog package used by the memory effect benchmark."""

import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII key with underscores between words."""
    words = re.sub(r"[^a-z0-9\s]", "", label.lower()).split()
    return "_".join(words)
