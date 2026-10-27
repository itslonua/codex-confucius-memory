"""Tiny synthetic catalog package used by the memory effect benchmark."""


def slugify(label: str) -> str:
    """Return a lowercase slug. Historical investigation found punctuation remains."""
    return "-".join(label.strip().lower().split())
