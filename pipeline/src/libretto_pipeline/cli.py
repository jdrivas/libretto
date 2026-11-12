"""libretto-pipeline: build time-synced sung-text files for an opera, stage by stage."""
import argparse
import json
import logging
import sys

from . import logs
from .config import load_opera
from .version import VERSION_STRING

log = logging.getLogger("libretto_pipeline")


def cmd_decode(args):
    from . import decode
    decode.run(load_opera(args.opera))


def cmd_status(args):
    opera = load_opera(args.opera)
    manifest = opera.work / "decode" / "manifest.json"
    print(f"{opera.title} ({opera.id}), work directory {opera.work}")
    if not manifest.exists():
        print("  decode: not run")
        return
    m = json.loads(manifest.read_text())
    c = m["checks"]
    print(f"  decode: {c['pieces']} pieces, {c['with_problems']} with problems, "
          f"{len(c['contents_not_found'])} contents entries not found")


def parser():
    p = argparse.ArgumentParser(prog="libretto-pipeline", description=__doc__)
    p.add_argument("--version", action="version", version=VERSION_STRING)
    p.add_argument("--log-level", choices=list(logs.LEVELS), default="info",
                   help="log verbosity (default: info)")
    p.add_argument("--log-utc", action="store_true", help="log timestamps in UTC instead of local time")
    sub = p.add_subparsers(dest="command", required=True, metavar="command")
    d = sub.add_parser("decode", help="decode the score PDF into pieces and a manifest")
    d.add_argument("opera", help="opera id, e.g. figaro (operas/<id>/opera.toml)")
    d.set_defaults(func=cmd_decode)
    s = sub.add_parser("status", help="show what has been built for an opera")
    s.add_argument("opera")
    s.set_defaults(func=cmd_status)
    return p


def main(argv=None):
    args = parser().parse_args(argv)
    logs.setup(args.log_level, args.log_utc)
    log.info(VERSION_STRING)
    try:
        args.func(args)
    except KeyboardInterrupt:
        log.error("interrupted")
        return 130
    return 0


if __name__ == "__main__":
    sys.exit(main())
