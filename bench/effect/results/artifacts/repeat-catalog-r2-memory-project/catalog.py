"""Tiny synthetic catalog package used by the memory effect benchmark."""

import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII key with punctuation removed."""
    cleaned = re.sub(r"[^a-z0-9\s]", "", label.lower())
    return "_".join(cleaned.split())
