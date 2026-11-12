"""The version line: `libretto-pipeline <version> <YYYY-MM-DD HH:MM:SS.mmm> (<hash>[-dirty])`."""
APP_NAME = "libretto-pipeline"

try:
    from ._version import BUILD_TIME, GIT_DIRTY, GIT_HASH, VERSION
except ImportError:  # running from a source tree that was never built
    VERSION, BUILD_TIME, GIT_HASH, GIT_DIRTY = "0.0.0", "unknown", "unknown", True

VERSION_STRING = f"{APP_NAME} {VERSION} {BUILD_TIME} ({GIT_HASH}{'-dirty' if GIT_DIRTY else ''})"
