"""Phase 5: sung text from the OMR score -> timed lines -> interchange file for roon-rd.

Score-only pipeline (no MIDI, no libretto):
  1. Load the Audiveris export (with OCR'd lyrics), align its notes to the recording.
  2. Identify which character each vocal part is (cast order + OCR labels + clefs).
  3. Build words from syllables, split into display lines at rests/punctuation.
  4. Time each line by its first syllable's score position -> recording time.
  5. Write out/sextet.lines.json and a copy of the Solti interchange file whose
     Sextet track segments are these lines (out/librettos/).

Usage: phase5_lines.py [omr_dir]   (default work/omr_typeset_v2)
"""
import json
import re
import sys
from pathlib import Path

import music21
import numpy as np

from phase2_compare import SEC_PER_QUARTER, load_omr
from phase4_align import OUT, align, load_audio

HERE = Path(__file__).parent
INTERCHANGE = HERE.parent / "working" / "librettos" / "georg-solti-1981.interchange.json"

# Cast in score (staff) order, with the clef each normally uses.
CAST = [("SUSANNA", "T"), ("MARCELLINA", "T"), ("DON CURZIO", "T8"),
        ("IL CONTE", "B"), ("DON BARTOLO", "B"), ("FIGARO", "B")]
# OCR'd staff labels, normalised (lowercase, letters/digits only) -> cast index.
LABELS = {"s": 0, "susanna": 0, "m": 1, "marcellina": 1, "dc": 2, "doncurzio": 2, "doncumin": 2,
          "c": 3, "ilconte": 3, "11come": 3, "conte": 3, "db": 4, "donbartolo": 4,
          "f": 5, "figaro": 5}
TRANSPOSE = [(re.compile(r"\bcor", re.I), -7)]  # horns in F
DYNAMIC = re.compile(r"^(p+|f+|fp|sfp?|sf|mf|mp|cresc|dim|decresc|rf|fz|tr|a2|z|az|.*rc.*sc.*)$", re.I)
REST_BREAK_QL = 1.0   # a gap of at least a beat ends a line
MAX_WORDS = 9


def clef_class(part):
    clefs = {type(c).__name__ for c in part.recurse().getElementsByClass("Clef")}
    if "Treble8vbClef" in clefs:
        return "T8"
    if "BassClef" in clefs:
        return "B"
    return "T"


def vocal_parts(score):
    """Parts that are mostly texted notes, in score order."""
    out = []
    for i, p in enumerate(score.parts):
        notes = list(p.recurse().notes)
        texted = [n for n in notes if n.lyric and not DYNAMIC.match(re.sub(r"\W", "", n.lyric))]
        if notes and len(texted) / len(notes) >= 0.5:
            out.append(i)
    return out


def span(part, score):
    offs = [float(n.getOffsetInHierarchy(score)) for n in part.recurse().notes]
    return min(offs), max(offs)


def assign_characters(score, parts):
    """Split vocal parts into layout blocks, then DP-align each block to the cast order.

    Parts belong to the same block (one system layout) when they are close in
    score order and their time spans overlap.
    """
    spans = {i: span(score.parts[i], score) for i in parts}
    blocks, cur = [], [parts[0]]
    for i in parts[1:]:
        lo, hi = min(spans[j][0] for j in cur), max(spans[j][1] for j in cur)
        overlaps = spans[i][0] <= hi and spans[i][1] >= lo
        if i - cur[-1] <= 3 and overlaps and len(cur) < len(CAST):
            cur.append(i)
        else:
            blocks.append(cur)
            cur = [i]
    blocks.append(cur)

    who = {}
    for block in blocks:
        k, n = len(block), len(CAST)

        def score_of(pi, ci):
            p = score.parts[pi]
            label = re.sub(r"[^a-z0-9]", "", (p.partName or "").lower())
            s = 0
            if label in LABELS:
                s += 10 if LABELS[label] == ci else -10
            s += 2 if clef_class(p) == CAST[ci][1] else -5
            return s

        # best[j][c]: best total assigning first j parts with part j-1 -> cast c
        NEG = -1e9
        best = [[NEG] * n for _ in range(k)]
        back = [[None] * n for _ in range(k)]
        for c in range(n):
            best[0][c] = score_of(block[0], c)
        for j in range(1, k):
            for c in range(n):
                prev = max(range(c), key=lambda q: best[j - 1][q], default=None) if c else None
                if prev is not None and best[j - 1][prev] > NEG:
                    best[j][c] = best[j - 1][prev] + score_of(block[j], c)
                    back[j][c] = prev
        c = max(range(n), key=lambda q: best[k - 1][q])
        for j in range(k - 1, -1, -1):
            who[block[j]] = CAST[c][0]
            c = back[j][c]
    return who


def syllables(score, parts, who):
    """Sung syllables with score offsets. A note without a lyric that directly follows a
    sung note continues the previous syllable (a melisma, or a lyric the OMR lost), so
    it extends that syllable's end instead of looking like a rest."""
    out = []
    for pi in parts:
        last = None  # this part's latest syllable, while no rest has intervened
        for n in score.parts[pi].recurse().notesAndRests:
            off = float(n.getOffsetInHierarchy(score))
            end = off + float(n.duration.quarterLength)
            if n.isRest:
                last = None
                continue
            texts = [(ly, (ly.text or "").strip()) for ly in n.lyrics]
            texts = [(ly, t) for ly, t in texts if t and not DYNAMIC.match(re.sub(r"\W", "", t))]
            if not texts:
                if last is not None and abs(off - last["end"]) < 1e-6:
                    last["end"] = end
                continue
            for ly, t in texts:
                last = {"char": who[pi], "off": off, "end": end, "text": t, "syl": ly.syllabic or "single"}
                out.append(last)
    return out


def lines_for(sylls):
    """Join syllables into words, then words into lines (per character).

    Each line also keeps its words with their score offsets ("words": [{text, off, end,
    sylls: [{text, off, end}]}]), so later steps can time phrases within a line. OMR
    sometimes glues several sung words into one "word", so the syllables matter."""
    lines, words, start, last_end = [], [], None, None
    cur_word, word_start, cur_sylls = "", None, []

    def flush():
        nonlocal words, start
        if words:
            lines.append({"start_off": start, "end_off": last_end, "text": " ".join(w["text"] for w in words),
                          "words": words})
        words, start = [], None

    def end_word():
        nonlocal cur_word, word_start, cur_sylls
        words.append({"text": cur_word, "off": word_start, "end": last_end, "sylls": cur_sylls})
        cur_word, word_start, cur_sylls = "", None, []

    for s in sorted(sylls, key=lambda x: x["off"]):
        if last_end is not None and s["off"] - last_end >= REST_BREAK_QL and not cur_word:
            flush()
        if start is None:
            start = s["off"]
        if word_start is None:
            word_start = s["off"]
        cur_word += s["text"]
        cur_sylls.append({"text": s["text"], "off": s["off"], "end": s["end"]})
        last_end = s["end"]  # end of the line so far = end of its latest syllable
        if s["syl"] in ("single", "end"):
            end_word()
            if re.search(r"[.!?;]$", words[-1]["text"]) or len(words) >= MAX_WORDS:
                flush()
    if cur_word:
        end_word()
    flush()
    return lines


def main():
    omr_dir = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE / "work" / "omr_typeset_v2"
    mxl = next(omr_dir.glob("*.mxl"))
    score = music21.converter.parse(str(mxl)).stripTies()
    parts = vocal_parts(score)
    who = assign_characters(score, parts)
    print("vocal parts -> characters:")
    for pi in parts:
        print(f"  part {pi:2} {score.parts[pi].partName!r:16} -> {who[pi]}")

    df, _, summary, _ = load_omr(omr_dir, TRANSPOSE)
    x, tuning = load_audio()
    to_audio = align(x, tuning, df, "typeset-ocr")

    sylls = syllables(score, parts, who)
    timed = []
    for char in [c for c, _ in CAST]:
        for ln in lines_for([s for s in sylls if s["char"] == char]):
            timed.append({"start": round(to_audio(ln["start_off"] * SEC_PER_QUARTER), 2),
                          "character": char, "text": ln["text"], "translation": ""})
    timed.sort(key=lambda s: s["start"])
    (OUT / "sextet.lines.json").write_text(json.dumps(timed, indent=1, ensure_ascii=False))
    print(f"\n{len(timed)} lines from {len(sylls)} syllables. First 25:")
    for s in timed[:25]:
        m, sec = divmod(s["start"], 60)
        print(f"  {int(m)}:{sec:05.2f}  {s['character']:<11} {s['text']}")

    ic = json.loads(INTERCHANGE.read_text())
    track = next(t for t in ic["tracks"] if "Sestetto" in t["title"] or "Riconosci" in t["title"])
    track["segments"] = timed
    dest = OUT / "librettos"
    dest.mkdir(exist_ok=True)
    (dest / INTERCHANGE.name).write_text(json.dumps(ic, indent=1, ensure_ascii=False))
    print(f"\nReplaced segments of track {track['track_id']} ({track['title'][:50]}...)")
    print(f"Wrote {dest / INTERCHANGE.name}")


if __name__ == "__main__":
    main()
