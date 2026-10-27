"""A small visible smoke test; task-specific acceptance checks are external."""
from archive import archive_key

assert archive_key("Hello World") == "hello_world"
