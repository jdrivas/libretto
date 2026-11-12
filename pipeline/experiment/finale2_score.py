"""Act II finale, recording-independent part: who sings on each staff, and timed sung lines
in score time.

The Ricordi vocal score changes its vocal staves from system to system (the Count alone,
then Count and Countess, ... seven singers at the end), and Audiveris's "parts" don't follow
characters: one part holds different singers in different systems. So characters are
decided per (part, system), the "unit":
  - units of one system come in staff order (top to bottom), which follows the cast order
    of the score (CAST below), so a system is assigned by a DP over cast order;
  - each unit scores against each character by how well its OCR'd words match that
    character's text (local letter alignment), its OCR'd staff label (first letter) and
    its clef (only bass clef is trusted: it means a man).

Output: out/finale2.score.json  {"units": [...], "lines": [...]} where lines are phase-7
style (score_start/score_end in score seconds, sylls with score times), ready for the
phase-8 text clean-up. Score seconds: quarter notes at nominal 120 bpm (SEC_PER_QUARTER),
from the OCR run of Audiveris (omr_ocr2).
"""
import json
import re
import sys
from pathlib import Path

import music21
import numpy as np

import phase8_text as P8
from finale2_text import TEXT
from phase2_compare import SEC_PER_QUARTER
from phase4_align import OUT
from phase5_lines import DYNAMIC, lines_for

WORK = Path.home() / "Development" / "libretto-work" / "finale2"
OMR = WORK / "omr_ocr2"
# Staff order in this score (top to bottom), seen on the printed pages.
CAST = ["SUSANNA", "LA CONTESSA", "MARCELLINA", "BASILIO", "IL CONTE", "ANTONIO", "DON BARTOLO", "FIGARO"]
MEN = {"BASILIO", "IL CONTE", "ANTONIO", "DON BARTOLO", "FIGARO"}
BASS = {"IL CONTE", "ANTONIO", "DON BARTOLO", "FIGARO"}
LABEL_FIRST = {"s": {"SUSANNA"}, "m": {"MARCELLINA"}, "a": {"ANTONIO"}, "f": {"FIGARO"},
               "b": {"BASILIO", "DON BARTOLO"}, "c": {"LA CONTESSA", "IL CONTE"}}
# Who is on stage, by page of the finale PDF (0 = printed p. 134), from the scene starts in
# the page map (Scena IX p. 151, X p. 160, XI p. 178).
ON_STAGE = [(0, {"SUSANNA", "LA CONTESSA", "IL CONTE"}),
            (17, {"SUSANNA", "LA CONTESSA", "IL CONTE", "FIGARO"}),
            (26, {"SUSANNA", "LA CONTESSA", "IL CONTE", "ANTONIO", "FIGARO"}),
            (44, {"SUSANNA", "LA CONTESSA", "MARCELLINA", "BASILIO", "IL CONTE", "DON BARTOLO", "FIGARO"})]
W_TEXT, W_LABEL, W_CLEF, W_OFFSTAGE = 10.0, 2.0, 3.0, 5.0
MIN_TEXT_LETTERS = 6
PHRASE_MIN_LETTERS, PHRASE_MARGIN = 8, 0.3


def local_score(q, ref):
    """Smith-Waterman score of letter string q inside ref, per letter of q (0..MATCH)."""
    if not q:
        return 0.0
    r = np.frombuffer(ref.encode(), np.uint8)
    H = np.zeros(len(r) + 1)
    best = 0.0
    for ch in q.encode():
        diag = H[:-1] + np.where(r == ch, 2.0, -1.0)
        new = np.zeros_like(H)
        new[1:] = np.maximum(diag, H[1:] - 1.0)  # q letter skipped
        new = np.maximum(new, 0)
        # ref letter skipped (gap in q): running max with penalty 1 per letter
        idx = np.arange(len(new))
        new = np.maximum(new, np.maximum.accumulate(new + idx) - idx - 1.0)
        new = np.maximum(new, 0)
        H = new
        best = max(best, H.max())
    return best / (2.0 * len(q))


def system_starts(score):
    piano = max(score.parts, key=lambda p: len(p.recurse().notes))
    starts, pages = {1}, [1]
    for m in piano.getElementsByClass("Measure"):
        if m.getElementsByClass("PageLayout"):
            pages.append(m.number)
        if m.getElementsByClass("PageLayout") or any(getattr(l, "isNew", False)
                                                    for l in m.getElementsByClass("SystemLayout")):
            starts.add(m.number)
    starts = sorted(starts)
    page_of_sys = [int(np.searchsorted(pages, m, side="right") - 1) for m in starts]
    return starts, page_of_sys


METERS = np.array([1.5, 2.0, 3.0, 4.0])  # bar lengths (quarters) of the finale's metres
GRID_HALF_WINDOW = 8                      # bars either side for the running median


def bar_grid(score):
    """One shared bar grid for all staves, and each note's time on it.

    Audiveris reads this scan's rhythms badly, so each part's bars have their own lengths
    and the parts drift apart (by 17 beats within 10 bars). Here every bar gets one length,
    the median of the parts' readings, smoothed over neighbouring bars (the metre rarely
    changes) and snapped to a metre; a part's notes are spread over the bar in proportion
    to where that part's reading puts them. Sets n.t0/n.t1 (quarters on the grid) on every
    note and rest; returns {bar number: (start, length)}."""
    reads = {}
    for p in score.parts:
        for m in p.getElementsByClass("Measure"):
            d = float(m.duration.quarterLength)
            if m.recurse().notes and d > 0:
                reads.setdefault(m.number, []).append(d)
    nums = sorted({m.number for p in score.parts for m in p.getElementsByClass("Measure")})
    med = np.array([np.median(reads[n]) if n in reads else np.nan for n in nums])
    grid, start = {}, 0.0
    for i, n in enumerate(nums):
        win = med[max(0, i - GRID_HALF_WINDOW):i + GRID_HALF_WINDOW + 1]
        win = win[np.isfinite(win)]
        L = float(METERS[np.abs(METERS - np.median(win)).argmin()]) if len(win) else 4.0
        grid[n] = (start, L)
        start += L
    for p in score.parts:
        for m in p.getElementsByClass("Measure"):
            s0, L = grid[m.number]
            d = float(m.duration.quarterLength)
            k = L / d if d > 0 else 1.0
            for n in m.recurse().notesAndRests:
                o = float(n.getOffsetInHierarchy(m))
                n.t0 = s0 + min(o * k, L)
                n.t1 = n.t0 + float(n.duration.quarterLength) * k
    return grid


def units_of(score, starts):
    sys_of = lambda num: int(np.searchsorted(starts, num, side="right") - 1)
    units = {}
    for pi, p in enumerate(score.parts):
        for m in p.getElementsByClass("Measure"):
            for n in m.recurse().notesAndRests:
                u = units.setdefault((sys_of(m.number), pi), {"sys": sys_of(m.number), "part": pi,
                                                               "label": p.partName or "", "items": []})
                u["items"].append(n)
    out = []
    for u in units.values():
        notes = [n for n in u["items"] if not n.isRest]
        if not notes:
            continue
        texted = [n for n in notes if any((ly.text or "").strip() and not DYNAMIC.match(re.sub(r"\W", "", ly.text))
                                          for ly in n.lyrics)]
        u["n_notes"], u["n_texted"] = len(notes), len(texted)
        u["clef"] = {type(c).__name__ for c in score.parts[u["part"]].recurse().getElementsByClass("Clef")
                     if c.getContextByClass("Measure") is not None
                     and sys_of(c.getContextByClass("Measure").number) == u["sys"]}
        u["text"] = " ".join(ly.text.strip() for n in texted for ly in n.lyrics if (ly.text or "").strip())
        out.append(u)
    return out


def notes_df(score):
    """All notes (voices and piano) on the shared bar grid, for score-to-audio alignment;
    times in score seconds. Call after bar_grid()."""
    import pandas as pd
    rows = []
    for p in score.parts:
        for n in p.recurse().notes:
            for pitch in n.pitches:
                rows.append({"start": n.t0 * SEC_PER_QUARTER,
                             "duration": max(n.t1 - n.t0, 0.125) * SEC_PER_QUARTER,
                             "pitch": pitch.midi, "velocity": 80, "instrument": p.partName or "part"})
    return pd.DataFrame(rows).sort_values("start").reset_index(drop=True)


def load_score():
    score = music21.converter.parse(str(next(OMR.glob("*.mxl")))).stripTies()
    grid = bar_grid(score)
    return score, grid


def on_stage(page):
    return [chars for p0, chars in ON_STAGE if p0 <= page][-1]


def assign(units, page_of_sys):
    refs = {c: "".join(P8.letters(it) for verse in TEXT[c] for it, _ in verse) for c in CAST}
    by_sys = {}
    for u in units:
        by_sys.setdefault(u["sys"], []).append(u)
    for sysno, us in sorted(by_sys.items()):
        vocal = [u for u in sorted(us, key=lambda u: u["part"])
                 if u["n_texted"] >= 1 and u["n_texted"] / u["n_notes"] >= 0.25]
        for u in us:
            u["char"] = None
        if not vocal:
            continue
        sc = np.zeros((len(vocal), len(CAST)))
        for i, u in enumerate(vocal):
            q = P8.letters(u["text"])
            if len(q) >= MIN_TEXT_LETTERS:
                u["text_scores"] = {c: round(local_score(q, refs[c]), 3) for c in CAST}
                sc[i] += [W_TEXT * u["text_scores"][c] for c in CAST]
            lab = re.sub(r"[^a-z]", "", u["label"].lower())
            if lab and lab not in ("voice", "piano") and lab[0] in LABEL_FIRST:
                sc[i] += [W_LABEL if c in LABEL_FIRST[lab[0]] else -W_LABEL for c in CAST]
            if u["clef"] == {"BassClef"}:
                sc[i] += [W_CLEF if c in BASS else -W_CLEF for c in CAST]
            # Not on stage per the page map: a penalty, not a ban, as page boundaries from
            # the page map and from Audiveris can differ by a system or two.
            sc[i] += [0 if c in on_stage(page_of_sys[sysno]) else -W_OFFSTAGE for c in CAST]
        # DP: staves top to bottom take strictly increasing cast positions.
        k, n = sc.shape
        best = np.full((k, n), -1e9)
        back = np.zeros((k, n), int)
        best[0] = sc[0]
        for j in range(1, k):
            run = np.maximum.accumulate(best[j - 1])
            arg = np.array([int(np.argmax(best[j - 1][:c + 1])) for c in range(n)])
            best[j, 1:] = run[:-1] + sc[j, 1:]
            back[j, 1:] = arg[:-1]
        if k > n:
            raise SystemExit(f"system {sysno}: {k} vocal staves, more than the cast")
        c = int(np.argmax(best[k - 1]))
        for j in range(k - 1, -1, -1):
            vocal[j]["char"] = CAST[c]
            c = back[j, c]


def main():
    score = music21.converter.parse(str(next(OMR.glob("*.mxl")))).stripTies()
    grid = bar_grid(score)
    print(f"bar grid: {len(grid)} bars, {sum(L for _, L in grid.values()):.0f} quarters "
          f"(Audiveris's own reading: {float(score.highestTime):.0f})")
    starts, page_of_sys = system_starts(score)
    units = units_of(score, starts)
    assign(units, page_of_sys)
    print(f"{len(starts)} systems, {len(units)} (part, system) units, "
          f"{sum(u['char'] is not None for u in units)} vocal")
    verbose = "-v" in sys.argv
    if verbose:
        for u in sorted(units, key=lambda u: (u["sys"], u["part"])):
            if u["char"]:
                ts = u.get("text_scores", {})
                top = sorted(ts, key=ts.get, reverse=True)[:2]
                print(f"  sys {u['sys']:3} part {u['part']:3} {u['label'][:8]!r:10} {','.join(sorted(u['clef']))[:12]:12} "
                      f"-> {u['char']:<12} text best {' '.join(f'{c[:6]}={ts[c]:.2f}' for c in top):28} {u['text'][:50]}")

    # Ricordi also puts alternating dialogue on one staff, with the singer's name over each
    # entry, so a unit can hold several characters. Each phrase (notes between rests) keeps
    # the unit's character unless its words clearly match another character better.
    refs = {c: "".join(P8.letters(it) for verse in TEXT[c] for it, _ in verse) for c in CAST}
    moved = []
    for u in units:
        if not u["char"]:
            continue
        u["phrases"], cur = [], []
        for n in u["items"]:
            if n.isRest:
                if cur:
                    u["phrases"].append(cur)
                cur = []
            else:
                cur.append(n)
        if cur:
            u["phrases"].append(cur)
        u["phrase_char"] = []
        for ph in u["phrases"]:
            q = P8.letters(" ".join(ly.text or "" for n in ph for ly in n.lyrics))
            char = u["char"]
            if len(q) >= PHRASE_MIN_LETTERS:
                s = {c: local_score(q, refs[c]) for c in CAST}
                best = max(s, key=s.get)
                if best != char and s[best] >= s.get(char, 0) + PHRASE_MARGIN:
                    moved.append((u["sys"], u["part"], char, best, round(s.get(char, 0), 2), round(s[best], 2), q))
                    char = best
            u["phrase_char"].append(char)
    print(f"{len(moved)} phrases moved to another character than their staff's:")
    for m in moved:
        print(f"   sys {m[0]:3} part {m[1]:3} {m[2]:<12} -> {m[3]:<12} ({m[4]:.2f} -> {m[5]:.2f})  {m[6][:40]}")

    # Syllables per character (phase-5 rules: a note without a lyric right after a sung
    # note extends it; a rest ends it).
    sylls = []
    for u in units:
        if not u["char"]:
            continue
        last = None
        for ph, ch in zip(u["phrases"], u["phrase_char"]):
          last = None
          for n in ph:
            off, end = n.t0, n.t1
            texts = [(ly, (ly.text or "").strip()) for ly in n.lyrics]
            texts = [(ly, t) for ly, t in texts if t and not DYNAMIC.match(re.sub(r"\W", "", t))]
            if not texts:
                if last is not None and abs(off - last["end"]) < 1e-6:
                    last["end"] = end
                continue
            for ly, t in texts:
                last = {"char": ch, "off": off, "end": end, "text": t, "syl": ly.syllabic or "single"}
                sylls.append(last)
    lines = []
    for char in CAST:
        for ln in lines_for([s for s in sylls if s["char"] == char]):
            q = SEC_PER_QUARTER
            sy = [{"text": s["text"], "start": round(s["off"] * q, 3), "end": round(s["end"] * q, 3),
                   "score_start": round(s["off"] * q, 3), "score_end": round(s["end"] * q, 3)}
                  for w in ln["words"] for s in w["sylls"]]
            lines.append({"start": round(ln["start_off"] * q, 3), "end": round(ln["end_off"] * q, 3),
                          "score_start": round(ln["start_off"] * q, 3), "score_end": round(ln["end_off"] * q, 3),
                          "character": char, "voice": CAST.index(char), "text": ln["text"], "sylls": sy})
    lines.sort(key=lambda l: l["start"])
    counts = {c: sum(l["character"] == c for l in lines) for c in CAST}
    print(f"{len(sylls)} syllables -> {len(lines)} lines: {counts}")
    keep = [{k: u[k] for k in ("sys", "part", "label", "char", "text", "n_notes", "n_texted")} for u in units]
    (OUT / "finale2.score.json").write_text(json.dumps({"units": keep, "lines": lines}, indent=1, ensure_ascii=False))
    print(f"-> {OUT / 'finale2.score.json'}")


if __name__ == "__main__":
    main()
