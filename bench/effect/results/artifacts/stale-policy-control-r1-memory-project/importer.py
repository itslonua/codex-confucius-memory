"""Feed ingestion delegates its durable key to catalog.slugify."""
from catalog import slugify


def import_record(title: str, records: dict[str, str]) -> str:
    key = slugify(title)
    records[key] = title
    return key
