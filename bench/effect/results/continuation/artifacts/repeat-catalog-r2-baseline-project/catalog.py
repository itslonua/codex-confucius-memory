"""Tiny synthetic catalog package used by the memory effect benchmark."""
import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII slug with underscores between words."""
    return re.sub(r"[^a-z0-9]+", "_", label.lower()).strip("_")
