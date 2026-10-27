"""Archive lookup uses the underscore key format in this earlier snapshot."""


def archive_key(title: str) -> str:
    return "_".join(title.strip().lower().split())
