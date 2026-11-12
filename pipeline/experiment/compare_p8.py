"""P8: the from-scratch rebuild against the experiment's interchange files.

For each recording and passage: the new build's checks, and how far its line starts are
from the experiment's for the same lines (same track, same character, same opening
letters). Run from pipeline/: .venv/bin/python experiment/compare_p8.py
"""
import json
import re
import sys
import unicodedata
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))
from libretto_pipeline.config import load_opera  # noqa: E402

EXP = Path(__file__).parents[3] / "libretto-scratch" / "sextet-experiment" / "out" / "librettos"
PIECES = ["no12", "rec12a", "no16", "no19", "rec19a", "rec19b"]


def key(s):
    t = unicodedata.normalize("NFD", s["text"].lower())
    t = re.sub(r"[^a-z]", "", "".join(c for c in t if unicodedata.category(c) != "Mn"))
    return (s["character"].replace("DON BARTOLO", "BARTOLO"), t[:16])


def main():
    op = load_opera("figaro")
    for rec in ("georg-solti-1981", "currentzis-2014"):
        new = json.loads((op.work / "interchange" / f"{rec}.interchange.json").read_text())
        old = json.loads((EXP / f"{rec}.interchange.json").read_text())
        old_tracks = {(t.get("disc_number", 1), t.get("track_number")): t for t in old["tracks"]}
        print(f"\n== {rec}")
        for pid in PIECES:
            tf = op.work / "timed" / rec / f"{pid}.json"
            if not tf.exists():
                print(f"  {pid:7} not timed")
                continue
            c = json.loads(tf.read_text())["checks"]
            if c["route"] == "B":
                chk = (f"B: agreement {c['singing_agreement']:.0%}, entries {c['entries_matched']}/{c['entries_predicted']}, "
                       f"median {c['median_abs_offset_s']} s, within 1 s {c['within_1s']:.0%}, "
                       f"phrases in silence {c['phrases_mostly_silent']}/{c['phrases']}")
            else:
                chk = f"C: words in silence {c['words_mostly_silent']}/{c['words']}, men low {c['men_speeches_low']}"
            diffs, n_new = [], 0
            for t in new["tracks"]:
                segs = [s for s in t["segments"] if s.get("piece") == pid]
                n_new += len(segs)
                ot = old_tracks.get((t["disc_number"], t["track_number"]))
                if not ot:
                    continue
                pool = {}
                for s in ot["segments"]:
                    if s.get("character"):
                        pool.setdefault(key(s), []).append(s["start"])
                for s in segs:
                    cands = pool.get(key(s))
                    if cands:
                        diffs.append(min(cands, key=lambda x: abs(x - s["start"])) - s["start"])
            d = np.abs(diffs)
            vs = (f"vs experiment: {len(d)}/{n_new} lines found, |start diff| median {np.median(d):.2f} s, "
                  f"over 1 s {int((d > 1).sum())}" if len(d) else f"vs experiment: no matching lines ({n_new} new)")
            print(f"  {pid:7} {chk}\n          {vs}")


if __name__ == "__main__":
    main()
