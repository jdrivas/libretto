"""Log lines of the form `YYYY-MM-DD HH:MM:SS.mmm <level>   message`, local time or UTC."""
import logging
import sys
from datetime import datetime, timezone

TRACE = 5
logging.addLevelName(TRACE, "TRACE")

# Command-line level names, in order, and what they map to.
LEVELS = {"trace": TRACE, "debug": logging.DEBUG, "info": logging.INFO,
          "warn": logging.WARNING, "error": logging.ERROR, "off": logging.CRITICAL + 10}
_NAMES = {TRACE: "trace", logging.DEBUG: "debug", logging.INFO: "info",
          logging.WARNING: "warn", logging.ERROR: "error", logging.CRITICAL: "error"}
_TAG_WIDTH = len("<debug>   ")  # longest level token, then the message column


class LineFormatter(logging.Formatter):
    def __init__(self, utc=False):
        super().__init__()
        self.utc = utc

    def format(self, record):
        when = datetime.fromtimestamp(record.created, timezone.utc if self.utc else None)
        stamp = when.strftime("%Y-%m-%d %H:%M:%S.") + f"{int(record.msecs):03d}"
        tag = f"<{_NAMES.get(record.levelno, 'info')}>".ljust(_TAG_WIDTH)
        msg = record.getMessage()
        if record.exc_info:
            msg += "\n" + self.formatException(record.exc_info)
        return f"{stamp} {tag}{msg}"


def setup(level="info", utc=False):
    handler = logging.StreamHandler(sys.stderr)
    handler.setFormatter(LineFormatter(utc))
    root = logging.getLogger()
    root.handlers[:] = [handler]
    root.setLevel(LEVELS[level])
    # Third-party libraries stay at warn unless asked for trace.
    for noisy in ("matplotlib", "numba", "PIL"):
        logging.getLogger(noisy).setLevel(logging.WARNING if level != "trace" else TRACE)
