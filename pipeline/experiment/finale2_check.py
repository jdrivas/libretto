"""Check a long, multi-character number for misplaced stretches, independent of the aligner.

For every line sung by one character while nobody else is singing (by the timing being
checked), the vocal stem's pitch should fit the singer: men clearly below the women. Shown
per minute, this points at stretches where the alignment has slid (a man's line placed where
a woman sings, or the other way round).

Usage: finale2_check.py <label>    (reads out/finale2.lines.<label>.json and the joined stems)
"""
import json
import sys
from pathlib import Path

import numpy as np

import phase6_timeline as P6
from finale2_build import RECORDINGS, WORK
from finale2_score import MEN
from phase4_align import OUT

BIN_S = 60


def main(label):
    lines = json.loads((OUT / f"finale2.lines.{label}.json").read_text())
    P6.STEMS = WORK / "stems" / "htdemucs" / RECORDINGS[label]["stems"]
    t, lv, la, f0, vprob, det = P6.analyse_stems()
    rows = []
    for s in lines:
        if s["end"] - s["start"] < 0.8:
            continue
        others = [o for o in lines if o is not s and o["character"] != s["character"]
                  and o["start"] < s["end"] and o["end"] > s["start"]]
        if others:
            continue
        m = (t >= s["start"]) & (t <= s["end"]) & np.isfinite(f0)
        if m.sum() < 10:
            continue
        rows.append((s["start"], s["character"], float(np.nanmedian(f0[m])), s["text"]))
    women = np.median([hz for _, c, hz, _ in rows if c not in MEN])
    men = np.median([hz for _, c, hz, _ in rows if c in MEN])
    cut = np.sqrt(women * men)  # halfway between, on a log scale
    ok = [(st, (c in MEN) == (hz < cut), c, hz, tx) for st, c, hz, tx in rows]
    print(f"{label}: {len(ok)} solo lines; men's median {men:.0f} Hz, women's {women:.0f} Hz; "
          f"register fits for {np.mean([o[1] for o in ok]):.0%}")
    end = max(s["end"] for s in lines)
    for b in range(0, int(end) + 1, BIN_S):
        sel = [o for o in ok if b <= o[0] < b + BIN_S]
        if sel:
            bad = [o for o in sel if not o[1]]
            bar = "#" * len(sel)
            print(f"  {b // 60:2d}:00  {len(sel) - len(bad):3d}/{len(sel):<3d} fit  "
                  + "; ".join(f"{o[0]:.0f}s {o[2].split()[-1][:6]} {o[3]:.0f}Hz '{o[4][:18]}'" for o in bad[:3]))


if __name__ == "__main__":
    main(sys.argv[1])
