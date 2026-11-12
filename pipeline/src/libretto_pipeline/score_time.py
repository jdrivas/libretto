"""The `score` stage: a piece's decode -> its sung lines and notes in score time.

From the experiment's ba_finale2_score.py, generalised to any piece and cast:
  - each bar lasts what its metre and printed tempo word suggest (TEMPO_BPM; one generic
    rule, not tuned per recording; the alignment absorbs the performers' tempo);
  - each syllable belongs to the character its staff is labelled with or, on a shared
    staff (secco recitative), to the last speaker cue printed before it;
  - a character's syllables are grouped into lines at a pause, phrase-final punctuation,
    or LINE_MAX_SYLLS.
Output: {"lines", "notes", "sections", "unattributed"}; lines carry score_start/score_end
(and start/end, equal, as in the experiment) and their syllables with score times.
"""
import json
import re

TEMPO_BPM = [("prestissimo", 184), ("presto", 168), ("più allegro", 150), ("allegro assai", 150),
             ("allegro molto", 150), ("allegretto", 108), ("allegro", 132), ("andantino", 84),
             ("molto andante", 84), ("andante", 76), ("larghetto", 60), ("adagio", 56), ("largo", 50)]
LINE_GAP_S = 0.6       # a pause of this much (score s) between syllables ends a line
LINE_MAX_SYLLS = 20


def bar_seconds(timesig, tempo):
    num, den = timesig
    word = (tempo or "allegro").lower()
    bpm = next((b for k, b in TEMPO_BPM if k in word), 120)
    quarters = num * 4 / den
    compound = den == 8 and (num in (6, 9, 12) or (num == 3 and bpm >= 100))
    beats = quarters / 1.5 if compound else num
    return beats * 60.0 / bpm


def speakers(dec, cast):
    """Character of each syllable (same order as dec['syllables']), or None."""
    cues = sorted(((c["staff"], c["t0"], cast.find(c["name"])) for c in dec.get("cues", [])),
                  key=lambda c: (c[0], c[1]))
    out = []
    for s in dec["syllables"]:
        found = cast.find(s["label"])
        if len(found) == 1:
            out.append(next(iter(found)))
            continue
        # A shared staff: the last cue on this staff at or before the syllable.
        prior = [names for staff, t, names in cues if staff == s["staff"] and t <= s["t0"] + 0.02 and len(names) == 1]
        if prior:
            out.append(next(iter(prior[-1])))
        elif len(found) > 1:  # labelled with several names and no cue yet: the first named
            out.append(min(found, key=cast.order))
        else:
            out.append(None)
    # Unlabelled staves: the score names a staff on a piece's first system only (solo
    # pieces, choir staves). Carry each staff's character forward in time; failing that,
    # the piece's only named character, or the chorus.
    named = {n for s in dec["syllables"] for n in cast.find(s["label"])}
    named |= {n for c in dec.get("cues", []) for n in cast.find(c["name"])}
    last = {}
    for i in sorted(range(len(out)), key=lambda i: dec["syllables"][i]["t0"]):
        st = dec["syllables"][i]["staff"]
        if out[i] is None:
            out[i] = last.get(st)
        if out[i] is None:
            out[i] = (next(iter(named)) if len(named) == 1 else "CORO" if "CORO" in named else None)
        if out[i] is not None:
            last[st] = out[i]
    return out


def score_time(dec, cast):
    bars = dec["bars"]
    start, length, t = {}, {}, 0.0
    for b in bars:
        start[b["bar"]], length[b["bar"]] = t, bar_seconds(b["timesig"], b["tempo"])
        t += length[b["bar"]]
    end_t = t
    to_s = lambda bt: start[int(bt)] + (bt - int(bt)) * length[int(bt)] if int(bt) in start else end_t
    sections, prev = [], None
    for b in bars:
        key = (tuple(b["timesig"]), b["tempo"])
        if key != prev:
            sections.append({"bar": b["bar"], "score_s": round(start[b["bar"]], 3), "timesig": b["timesig"],
                             "tempo": b["tempo"], "bar_s": round(length[b["bar"]], 3)})
            prev = key
    notes = [{"start": round(to_s(n["t0"]), 3), "duration": round(max(to_s(n["t1"]) - to_s(n["t0"]), 0.06), 3),
              "pitch": n["pitch"], "velocity": 80, "instrument": n["label"] or "piano"} for n in dec["notes"]]
    who = speakers(dec, cast)
    order = [n for n, _ in cast.members]
    lines, unattributed = [], 0
    for char in order:
        mine = sorted((s for s, w in zip(dec["syllables"], who) if w == char), key=lambda s: s["t0"])
        cur = []

        def flush():
            if cur:
                sy = [{"text": s["text"], "start": round(to_s(s["t0"]), 3), "end": round(to_s(s["t1"]), 3),
                       "score_start": round(to_s(s["t0"]), 3), "score_end": round(to_s(s["t1"]), 3),
                       "bar": round(s["t0"], 3)} for s in cur]
                lines.append({"start": sy[0]["start"], "end": sy[-1]["end"], "score_start": sy[0]["start"],
                              "score_end": sy[-1]["end"], "bar_start": sy[0]["bar"], "bar_end": round(cur[-1]["t1"], 3),
                              "character": char, "voice": order.index(char),
                              "text": " ".join(s["text"] for s in cur), "sylls": sy})
            cur.clear()
        for s in mine:
            if cur and (to_s(s["t0"]) - to_s(cur[-1]["t1"]) >= LINE_GAP_S
                        or re.search(r"[.!?;:]$", cur[-1]["text"]) or len(cur) >= LINE_MAX_SYLLS):
                flush()
            cur.append(s)
        flush()
    unattributed = sum(w is None for w in who)
    lines.sort(key=lambda l: l["start"])
    return {"lines": lines, "notes": notes, "sections": sections, "score_end_s": round(end_t, 3),
            "bars": [[b["bar"], round(start[b["bar"]], 3), round(length[b["bar"]], 3)] for b in bars],
            "unattributed": unattributed}


def bar_of(score, s):
    """Score seconds -> bar + fraction, as printed in the edition (e.g. 83.5 = halfway
    through bar 83)."""
    bars = score["bars"]
    if not bars:
        return None
    k = max(0, min(len(bars) - 1, sum(1 for b in bars if b[1] <= s) - 1))
    bar, start, length = bars[k]
    return round(bar + min(max((s - start) / length, 0.0), 0.999), 2)


def run(opera, piece_ids):
    from .decode import vector  # noqa: F401  (decode outputs must exist)
    src, dst = opera.work / "decode" / "pieces", opera.work / "score"
    dst.mkdir(parents=True, exist_ok=True)
    out = {}
    for pid in piece_ids:
        dec = json.loads((src / f"{pid}.score.json").read_text())
        sc = score_time(dec, opera.cast)
        (dst / f"{pid}.json").write_text(json.dumps(sc, ensure_ascii=False))
        out[pid] = sc
    return out
