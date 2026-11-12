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


def cmd_add_recording(args):
    from . import recording
    opera = load_opera(args.opera)
    for rid in args.recording or [r["id"] for r in opera.recordings]:
        recording.inventory(opera, opera.recording(rid))


def cmd_separate(args):
    from . import recording
    opera = load_opera(args.opera)
    for rid in args.recording or [r["id"] for r in opera.recordings]:
        opera.recording(rid)
        recording.separate(opera, rid, args.device)


def cmd_score(args):
    from . import stages
    stages.run_score(load_opera(args.opera), args.piece or None)


def cmd_clean(args):
    from . import stages
    stages.run_clean(load_opera(args.opera), args.piece or None)


def cmd_text(args):
    from . import textdraft
    opera = load_opera(args.opera)
    for pid in args.piece:
        print(textdraft.draft(opera, pid))


def cmd_place(args):
    from . import place
    opera = load_opera(args.opera)
    for rid in args.recording or [r["id"] for r in opera.recordings]:
        opera.recording(rid)
        place.place(opera, rid, args.act)


def cmd_time(args):
    from . import stages
    opera = load_opera(args.opera)
    opera.recording(args.recording)
    stages.run_time(opera, args.recording, args.piece or None)


def cmd_publish(args):
    from . import publish
    opera = load_opera(args.opera)
    for rid in args.recording or [r["id"] for r in opera.recordings]:
        opera.recording(rid)
        publish.publish(opera, rid)


def cmd_status(args):
    from . import recording
    opera = load_opera(args.opera)
    manifest = opera.work / "decode" / "manifest.json"
    print(f"{opera.title} ({opera.id}), work directory {opera.work}")
    if manifest.exists():
        c = json.loads(manifest.read_text())["checks"]
        print(f"  decode: {c['pieces']} pieces, {c['with_problems']} with problems, "
              f"{len(c['contents_not_found'])} contents entries not found")
    else:
        print("  decode: not run")
    for r in opera.recordings:
        print(f"  {r['id']}: {recording.status(opera, r['id'])}")


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
    a = sub.add_parser("add-recording", help="inventory a recording's tracks (all configured if none named)")
    a.add_argument("opera")
    a.add_argument("recording", nargs="*", help="recording id(s) from opera.toml")
    a.set_defaults(func=cmd_add_recording)
    sp = sub.add_parser("separate", help="mix and Demucs vocal stems for every track (resumable, slow)")
    sp.add_argument("opera")
    sp.add_argument("recording", nargs="*")
    sp.add_argument("--device", default="cpu", help="torch device for Demucs (default cpu; mps fails)")
    sp.set_defaults(func=cmd_separate)
    for name, func, hlp in (("score", cmd_score, "sung lines and notes in score time, per piece"),
                            ("clean", cmd_clean, "clean lines against the text files, attach the gloss")):
        c = sub.add_parser(name, help=hlp)
        c.add_argument("opera")
        c.add_argument("piece", nargs="*", help="piece ids (default: all main pieces)")
        c.set_defaults(func=func)
    tx = sub.add_parser("text", help="print a piece's decoded text by speaker, as a draft for its text file")
    tx.add_argument("opera")
    tx.add_argument("piece", nargs="+")
    tx.set_defaults(func=cmd_text)
    pl = sub.add_parser("place", help="find each piece in a recording (all configured if none named)")
    pl.add_argument("opera")
    pl.add_argument("recording", nargs="*")
    pl.add_argument("--act", type=int, action="append", help="only this act (repeatable); others are kept")
    pl.set_defaults(func=cmd_place)
    tm = sub.add_parser("time", help="time placed pieces in one recording (routes B and C)")
    tm.add_argument("opera")
    tm.add_argument("recording")
    tm.add_argument("piece", nargs="*", help="piece ids (default: every placed piece with a clean/text file)")
    tm.set_defaults(func=cmd_time)
    pb = sub.add_parser("publish", help="write each recording's interchange file from its timed pieces")
    pb.add_argument("opera")
    pb.add_argument("recording", nargs="*")
    pb.set_defaults(func=cmd_publish)
    s = sub.add_parser("status", help="show what has been built for an opera")
    s.add_argument("opera")
    s.set_defaults(func=cmd_status)
    return p


def main(argv=None):
    import os
    from .config import workspace
    args = parser().parse_args(argv)
    os.environ.setdefault("MPLCONFIGDIR", str(os.path.expanduser(workspace()["work_root"]) + "/.cache/matplotlib"))
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
