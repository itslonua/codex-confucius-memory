"""Lookups use the archive key formatter, independently of ingestion."""
from archive import archive_key


def lookup_record(title: str, records: dict[str, str]) -> str | None:
    return records.get(archive_key(title))
