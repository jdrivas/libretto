"""Act II finale from the Bärenreiter vector PDF: sung lines and notes in score time.

Input:  ~/Development/libretto-work/finale2-ba/ba_score.json  (ba_vector.py)
Output: out/finale2ba.score.json  {"lines": [...], "notes": [...], "sections": [...]}
        lines as phase 7/8 expect (sylls with score times); notes for the alignment.

Score time now has a tempo: each bar lasts what its metre and printed tempo mark suggest
(TEMPO_BPM below, one generic rule, not tuned per recording), so a Molto andante bar is
no longer as short as an Allegro one. The alignment still absorbs the performers' tempo.
"""
import json
import re
from pathlib import Path

import numpy as np

from phase4_align import OUT

BA = Path.home() / "Development" / "libretto-work" / "finale2-ba" / "ba_score.json"
LABELS = [("sus", "SUSANNA"), ("la c", "LA CONTESSA"), ("contessa", "LA CONTESSA"), ("il c", "IL CONTE"),
          ("conte", "IL CONTE"), ("fig", "FIGARO"), ("ant", "ANTONIO"), ("marc", "MARCELLINA"),
          ("bas", "BASILIO"), ("bart", "DON BARTOLO")]
CAST = ["SUSANNA", "LA CONTESSA", "MARCELLINA", "BASILIO", "IL CONTE", "ANTONIO", "DON BARTOLO", "FIGARO"]
# Beats per minute by tempo word, longest match first. The beat is the time signature's
# note value, except in compound metres (6/8, 9/8, 12/8) and fast 3/8, where it is the
# dotted quarter (one or two beats in a bar).
TEMPO_BPM = [("prestissimo", 184), ("presto", 168), ("più allegro", 150), ("allegro assai", 150),
             ("allegro molto", 150), ("allegretto", 108), ("allegro", 132), ("andantino", 84),
             ("molto andante", 84), ("andante", 76), ("larghetto", 60), ("adagio", 56), ("largo", 50)]
LINE_GAP_S = 0.6       # a pause of this much (score s) between syllables ends a line
LINE_MAX_SYLLS = 20


def char_of(label):
    l = label.lower().strip()
    if l.startswith("bas"):
        return "BASILIO"
    for k, c in LABELS:
        if l.startswith(k):
            return c
    return None


def bar_seconds(timesig, tempo):
    num, den = timesig
    word = (tempo or "allegro").lower()
    bpm = next((b for k, b in TEMPO_BPM if k in word), 120)
    quarters = num * 4 / den
    compound = den == 8 and (num in (6, 9, 12) or (num == 3 and bpm >= 100))
    beats = quarters / 1.5 if compound else num
    return beats * 60.0 / bpm


def main():
    sc = json.loads(BA.read_text())
    bars = sc["bars"]
    start, length = {}, {}
    t = 0.0
    for b in bars:
        start[b["bar"]], length[b["bar"]] = t, bar_seconds(b["timesig"], b["tempo"])
        t += length[b["bar"]]
    to_s = lambda bt: start[int(bt)] + (bt - int(bt)) * length[int(bt)] if int(bt) in start else t
    sections, prev = [], None
    for b in bars:
        key = (tuple(b["timesig"]), b["tempo"])
        if key != prev:
            sections.append({"bar": b["bar"], "score_s": round(start[b["bar"]], 3), "timesig": b["timesig"],
                             "tempo": b["tempo"], "bar_s": round(length[b["bar"]], 3)})
            prev = key
    print(f"{len(bars)} bars, {t / 60:.1f} min of score time at the printed tempos")
    for s in sections:
        print(f"   bar {s['bar']:4}  {s['tempo']:<14} {s['timesig'][0]}/{s['timesig'][1]}  {s['bar_s']:.2f} s/bar")

    notes = [{"start": round(to_s(n["t0"]), 3), "duration": round(max(to_s(n["t1"]) - to_s(n["t0"]), 0.06), 3),
              "pitch": n["pitch"], "velocity": 80, "instrument": n["label"] or "piano"} for n in sc["notes"]]
    lines = []
    unknown = set()
    for char in CAST:
        mine = sorted((s for s in sc["syllables"] if char_of(s["label"]) == char), key=lambda s: s["t0"])
        cur = []

        def flush():
            if cur:
                sy = [{"text": s["text"], "start": round(to_s(s["t0"]), 3), "end": round(to_s(s["t1"]), 3),
                       "score_start": round(to_s(s["t0"]), 3), "score_end": round(to_s(s["t1"]), 3)} for s in cur]
                lines.append({"start": sy[0]["start"], "end": sy[-1]["end"], "score_start": sy[0]["start"],
                              "score_end": sy[-1]["end"], "character": char, "voice": CAST.index(char),
                              "text": " ".join(s["text"] for s in cur), "sylls": sy})
            cur.clear()
        for s in mine:
            if cur and (to_s(s["t0"]) - to_s(cur[-1]["t1"]) >= LINE_GAP_S
                        or re.search(r"[.!?;:]$", cur[-1]["text"]) or len(cur) >= LINE_MAX_SYLLS):
                flush()
            cur.append(s)
        flush()
    for s in sc["syllables"]:
        if char_of(s["label"]) is None:
            unknown.add(s["label"])
    lines.sort(key=lambda l: l["start"])
    counts = {c: sum(l["character"] == c for l in lines) for c in CAST}
    print(f"{len(lines)} lines {counts}; staff labels not recognised: {sorted(unknown)}")
    (OUT / "finale2ba.score.json").write_text(json.dumps({"lines": lines, "notes": notes, "sections": sections},
                                                         ensure_ascii=False))


if __name__ == "__main__":
    main()
