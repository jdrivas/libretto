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
CHORUS = "CORO"


def bar_seconds(timesig, tempo):
    num, den = timesig
    word = (tempo or "allegro").lower()
    bpm = next((b for k, b in TEMPO_BPM if k in word), 120)
    quarters = num * 4 / den
    compound = den == 8 and (num in (6, 9, 12) or (num == 3 and bpm >= 100))
    beats = quarters / 1.5 if compound else num
    return beats * 60.0 / bpm


def speakers(dec, cast):
    """Character of each syllable (same order as dec['syllables']), or None.

    In score order, per syllable:
      1. its staff's label names one character;
      2. a smaller, unlabelled choir staff: the chorus;
      3. the staff's current character: the last cue on it (a shared staff, as in secco
         recitative) or the character it last carried. Staff positions only carry over
         while the system layout (its number of staves) stays the same;
      4. several names on the label and no cue yet: the first named;
      5. an unlabelled staff after the layout changed (No. 23: four solo staves, then two
         unnamed ones): the character with this staff's clef who sang most recently and
         is not already on another staff of this system;
      6. the piece's only named character, or the chorus."""
    named = {n for s in dec["syllables"] for n in cast.find(s["label"])}
    named |= {n for c in dec.get("cues", []) for n in cast.find(c["name"])}
    events = [(c["t0"], 0, k, c) for k, c in enumerate(dec.get("cues", []))]
    events += [(s["t0"], 1, k, s) for k, s in enumerate(dec["syllables"])]
    out = [None] * len(dec["syllables"])
    state, layout, cur_sys, on_sys = {}, None, None, {}
    recent, clef_of = {}, {}
    for t, kind, k, e in sorted(events, key=lambda e: (e[0] - 0.02 * (e[1] == 0), e[1], e[2])):
        sys_ = tuple(e.get("sys") or ()) or None
        if sys_ != cur_sys:
            cur_sys, on_sys = sys_, {}
            if e.get("n_staves") is not None and e["n_staves"] != layout:
                if layout is not None:
                    state = {}
                layout = e["n_staves"]
        st = e["staff"]
        if kind == 0:
            names = cast.find(e["name"])
            if len(names) == 1:
                state[st] = next(iter(names))
                recent[state[st]] = t
            continue
        found = cast.find(e["label"])
        if len(found) == 1:
            c = next(iter(found))
        elif e.get("choir") and not found:
            c = CHORUS
        elif state.get(st):
            c = state[st]
        elif len(found) > 1:
            c = min(found, key=cast.order)
        else:
            taken = {v for s2, v in on_sys.items() if s2 != st}
            cands = [n for n in recent if n != CHORUS and n not in taken
                     and (clef_of.get(n) is None or e.get("clef") is None or clef_of[n] == e["clef"])]
            c = (max(cands, key=lambda n: recent[n]) if cands
                 else next(iter(named)) if len(named) == 1 else CHORUS if CHORUS in named else None)
        out[k] = c
        if c is not None:
            state[st], on_sys[st], recent[c] = c, c, t
            if e.get("clef") and c != CHORUS:
                clef_of.setdefault(c, e["clef"])
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
    who0 = speakers(dec, cast)
    who = who0
    # The chorus: Figaro's choir parts sing the same words, so its text is read from one
    # staff, the top choir staff that sings in each bar (the viewer shows it once).
    top = {}
    for s, w in zip(dec["syllables"], who):
        if w == CHORUS:
            b = int(s["t0"])
            top[b] = min(top.get(b, s["staff"]), s["staff"])
    who = [None if w == CHORUS and s["staff"] != top[int(s["t0"])] else w
           for s, w in zip(dec["syllables"], who)]
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
    unattributed = sum(w is None for w in who0)
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
