"""P8 side test (decision 2): a secco recitative timed by route B (score alignment) as
well as route C (forced alignment), compared. Run from pipeline/ after the build:
.venv/bin/python experiment/side_test_p8.py [piece]"""
import json
import re
import sys
import unicodedata
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).parents[1] / "src"))
from libretto_pipeline import timing, voice  # noqa: E402
from libretto_pipeline.config import load_opera  # noqa: E402


def letters(s):
    t = unicodedata.normalize("NFD", s.lower())
    return re.sub(r"[^a-z]", "", "".join(c for c in t if unicodedata.category(c) != "Mn"))


def main():
    pid = sys.argv[1] if len(sys.argv) > 1 else "rec19a"
    op = load_opera("figaro")
    for rec in ("georg-solti-1981", "currentzis-2014"):
        plc = json.loads((op.work / "placement" / f"{rec}.json").read_text())
        e = next(p for p in plc["pieces"] if p["id"] == pid)
        act = next(a for a in plc["acts"] if a["act"] == e["act"])
        k0 = act["tracks"].index(e["tracks"][0])
        o0 = act["offsets"][k0]
        b = timing.time_number(op, rec, pid, e["tracks"], excerpt=(e["start"] - o0, e["end"] - o0))
        c = json.loads((op.work / "timed" / rec / f"{pid}.json").read_text())
        # Route C times are on its region's tracks; bring both to the act timeline.
        oc = act["offsets"][act["tracks"].index(c["tracks"][0])]
        ob = o0
        c_start = {}
        for s in c["lines"]:
            for g in s["gloss"]:
                c_start.setdefault((s["character"], letters(g["it"])), g["start"] + oc)
        d = []
        for s in b["lines"]:
            for g in s["gloss"]:
                k = (s["character"], letters(g["it"]))
                if k in c_start:
                    d.append(g["start"] + ob - c_start[k])
        d = np.array(d)
        cb = b["checks"]
        print(f"{rec} {pid}: route B agreement {cb['singing_agreement']:.0%}, entries "
              f"{cb['entries_matched']}/{cb['entries_predicted']}, median {cb['median_abs_offset_s']} s, "
              f"phrases in silence {cb['phrases_mostly_silent']}/{cb['phrases']}")
        print(f"   route B vs route C phrase starts: {len(d)} phrases, |diff| median {np.median(np.abs(d)):.2f} s, "
              f"over 1 s {int((np.abs(d) > 1).sum())}, B later on average by {np.mean(d):+.2f} s")


if __name__ == "__main__":
    main()
