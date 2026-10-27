"""Tiny synthetic catalog package used by the memory effect benchmark."""

import re


def slugify(label: str) -> str:
    """Return a lowercase ASCII slug with words separated by underscores."""
    return "_".join(re.findall(r"[a-z0-9]+", label.lower()))
