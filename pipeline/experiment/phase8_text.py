"""Phase 8: clean the OCR'd sung text and attach a literal, phrase-aligned gloss.

For each character, the OCR'd lines (in time order) are concatenated into one letter
stream and aligned against that character's reference text (sextet_text.py) with a
letter-level DP that may jump back to any word start, because singers repeat phrases.
The OCR therefore still decides what is sung when and how often; the reference only
supplies clean spelling and the gloss.

Each word is assigned to the line in which it starts (merging fragments like "re" /
"ster or"); reference words the OCR skipped between two matched words are filled in;
lines left without words are dropped (the previous line of that character absorbs
their time).

Input:  out/sextet.lines.snapped.json  (from phase7_anchors.py)
Output: out/sextet.lines.clean.json and the roon-rd test interchange file, whose
        segments gain `gloss` (aligned phrases), `translation` and `verses`.
"""
import copy
import json
import re
import unicodedata

import numpy as np

import phase7_anchors as P7
from phase4_align import OUT
from phase5_lines import INTERCHANGE
from phase6_timeline import silences, trim_to_silences
from sextet_text import TEXT

MATCH, MISMATCH, OCR_JUNK, REF_SKIP, JUMP = 2, -1, -1, -1, 5
FILL_MAX_WORDS = 4   # fill in at most this many skipped reference words
MIN_WORD_MATCH = 0.6  # a word counts only if this share of its letters match exactly
# Performance directions printed in the vocal staves, which OCR reads as lyrics.
DIRECTIONS = re.compile(r"sotto|sorta|voce", re.I)
DIAG, INS, DEL, JMP = 1, 2, 3, 4


def letters(s):
    s = unicodedata.normalize("NFKC", s)
    s = "".join(ch for ch in unicodedata.normalize("NFD", s.lower()) if unicodedata.category(ch) != "Mn")
    return re.sub(r"[^a-z]", "", s)


def reference(char):
    """Words of the character's text with (verse, chunk) ids, and the letter string."""
    words = []
    for vi, verse in enumerate(TEXT[char]):
        for ci, (it, en) in enumerate(verse):
            for tok in it.split():
                if letters(tok):
                    words.append({"v": vi, "c": ci, "tok": tok})
    # Repeats restart at a phrase (chunk) start, so only those are jump targets / free starts.
    ref, letter_word, starts, ends = [], [], [], []
    for wi, w in enumerate(words):
        if wi == 0 or (words[wi - 1]["v"], words[wi - 1]["c"]) != (w["v"], w["c"]):
            starts.append(len(ref))
        for ch in letters(w["tok"]):
            ref.append(ch)
            letter_word.append(wi)
        ends.append(len(ref))
    return words, ref, np.array(letter_word), np.array(starts), np.array(ends)


def align(ocr, ref, starts, ends):
    """Letter DP with jumps (word end -> any word start). Returns per-OCR-letter ref index
    (or -1) and a per-OCR-letter flag 'a jump happened just before this letter'."""
    n, m = len(ocr), len(ref)
    ref_arr = np.array(ref)
    NEG = -1e9
    H = np.full(m + 1, NEG)
    H[starts] = 0.0
    op = np.zeros((n + 1, m + 1), np.uint8)
    src = np.full((n + 1, m + 1), -1, np.int32)
    idx = np.arange(m + 1)
    for i in range(1, n + 1):
        A = np.full(m + 1, NEG)
        o = np.zeros(m + 1, np.uint8)
        diag = H[:-1] + np.where(ref_arr == ocr[i - 1], MATCH, MISMATCH)
        A[1:], o[1:] = diag, DIAG
        ins = H + OCR_JUNK
        better = ins > A
        A[better], o[better] = ins[better], INS
        e = ends[np.argmax(A[ends])]
        jv = A[e] - JUMP
        tgt = starts[A[starts] < jv]
        A[tgt], o[tgt] = jv, JMP
        src[i, tgt] = e
        C = np.maximum.accumulate(A + idx * (-REF_SKIP)) - idx * (-REF_SKIP)
        better = C > A + 1e-9
        A[better], o[better] = C[better], DEL
        op[i] = o
        H = A
    # traceback from the best end position
    i, j = n, int(np.argmax(H))
    mapping = np.full(n, -1)
    jumped = np.zeros(n, bool)
    while i > 0:
        k = op[i, j]
        if k == DIAG:
            mapping[i - 1] = j - 1  # match or substitution (an OCR misread)
            i, j = i - 1, j - 1
        elif k == INS:
            i -= 1
        elif k == DEL:
            j -= 1
        elif k == JMP:
            if i < n:
                jumped[i] = True
            j = src[i, j]
        else:
            break
    return mapping, jumped


TIME_KEYS = (("start", "end"), ("score_start", "score_end"))


def time_phrases(gloss, line):
    """Give each phrase start/end (audio) and score_start/score_end: the span of its
    matched words. A phrase made only of filled-in words runs from the previous phrase's
    end to the next one's start (the line's bounds at either edge). Times stay inside the
    line and in order."""
    for a, b in TIME_KEYS:
        for g in gloss:
            if g["_t"]:
                g[a] = min(t[a] for t in g["_t"])
                g[b] = max(t[b] for t in g["_t"])
        for i, g in enumerate(gloss):
            if a not in g:
                g[a] = next((h[b] for h in reversed(gloss[:i]) if b in h), line[a])
                g[b] = next((h[a] for h in gloss[i + 1:] if a in h and h[a] >= g[a]), line[b])
        lo = line[a]
        for g in gloss:
            g[a] = round(min(max(g[a], lo), line[b]), 3)
            g[b] = round(min(max(g[b], g[a]), line[b]), 3)
            lo = g[a]


def clean_character(char, lines):
    words, ref, letter_word, starts, ends = reference(char)
    stream, line_of, time_of = [], [], []
    for li, ln in enumerate(lines):
        # The line's letters, each with the OCR syllable it came from (for times);
        # performance directions are cut out of the joined letters.
        chars = [(ch, sy) for sy in ln["sylls"] for ch in letters(sy["text"])]
        joined = "".join(ch for ch, _ in chars)
        cut = np.zeros(len(chars), bool)
        for m in DIRECTIONS.finditer(joined):
            cut[m.start():m.end()] = True
        for (ch, sy), c in zip(chars, cut):
            if not c:
                stream.append(ch)
                line_of.append(li)
                time_of.append(sy)
    mapping, jumped = align(stream, ref, starts, ends)
    matched = sum(1 for k, r in enumerate(mapping) if r >= 0 and ref[r] == stream[k])
    word_len = np.bincount(letter_word, minlength=len(words))

    # 1. Word occurrences: runs of OCR letters mapped to the same reference word.
    occs, cur, after_jump = [], None, False
    for k, r in enumerate(mapping):
        if jumped[k]:
            cur, after_jump = None, True
        if r < 0:
            continue
        w = int(letter_word[r])
        if cur is None or w != cur["w"] or r <= cur["r"]:
            cur = {"w": w, "line": line_of[k], "eq": 0, "r": r, "after_jump": after_jump, "t": {}}
            occs.append(cur)
            after_jump = False
        cur["r"] = r
        cur["eq"] += ref[r] == stream[k]
        ow, tt = time_of[k], cur["t"]
        for key, pick in (("start", min), ("score_start", min), ("end", max), ("score_end", max)):
            tt[key] = pick(tt[key], ow[key]) if key in tt else ow[key]
    # 2. Keep only words most of whose letters really match (drops OCR junk matched to text).
    occs = [o for o in occs if o["eq"] >= max(1, MIN_WORD_MATCH * word_len[o["w"]])]
    # One-letter words ("è", "a", "E") match stray letters easily: keep one only when
    # the next kept word is the one that follows it in the text.
    occs = [o for i, o in enumerate(occs) if word_len[o["w"]] > 1
            or (i + 1 < len(occs) and occs[i + 1]["w"] == o["w"] + 1)]
    chunk = lambda w: (words[w]["v"], words[w]["c"])
    # 3. Assign words to lines, filling in words the OCR dropped:
    #    - between two kept words (no repeat in between): a filled word goes with the
    #      neighbour that shares its phrase (the previous word's line by default);
    #    - at the start of a repeat that picks a phrase up mid-way: the phrase's
    #      opening words ("un effetto" -> "È un effetto").
    #    Each assigned word is (reference word, its times or None if filled in).
    assigned = [[] for _ in lines]
    prev = None
    for o in occs:
        w = o["w"]
        if prev and not o["after_jump"] and 1 <= w - prev["w"] - 1 <= FILL_MAX_WORDS:
            for f in range(prev["w"] + 1, w):
                same_as_next = chunk(f) == chunk(w) and chunk(f) != chunk(prev["w"])
                assigned[o["line"] if same_as_next else prev["line"]].append((f, None))
        elif prev is None or o["after_jump"] or w < prev["w"]:
            first = w
            while first > 0 and chunk(first - 1) == chunk(w) and w - first < FILL_MAX_WORDS:
                first -= 1
            assigned[o["line"]].extend((f, None) for f in range(first, w))
        assigned[o["line"]].append((w, o["t"]))
        prev = o

    out = []
    for ln, ws in zip(lines, assigned):
        if not ws:
            if out:
                out[-1]["end"] = max(out[-1]["end"], ln["end"])
                if "score_end" in ln:
                    out[-1]["score_end"] = max(out[-1]["score_end"], ln["score_end"])
            continue
        gloss, verses = [], {}
        for w, tt in ws:
            wd = words[w]
            vid = f"{char}:{wd['v']}"
            if gloss and gloss[-1]["v"] == vid and gloss[-1]["c"] == wd["c"] and gloss[-1]["_last"] == w - 1:
                gloss[-1]["it"] += " " + wd["tok"]
                gloss[-1]["_last"] = w
            else:
                gloss.append({"v": vid, "c": wd["c"], "it": wd["tok"],
                              "en": TEXT[char][wd["v"]][wd["c"]][1], "_last": w, "_t": []})
            if tt:
                gloss[-1]["_t"].append(tt)
            verses[vid] = [list(p) for p in TEXT[char][wd["v"]]]
        time_phrases(gloss, ln)
        for g in gloss:
            del g["_last"], g["_t"]
        line = {k: v for k, v in ln.items() if k != "sylls"}
        out.append({**line, "ocr": ln["text"], "text": " ".join(words[w]["tok"] for w, _ in ws),
                    "translation": " ".join(g["en"] for g in gloss), "gloss": gloss, "verses": verses})
    return out, matched / max(1, len(stream))


def main():
    lines = json.loads((OUT / "sextet.lines.snapped.json").read_text())
    cleaned = []
    for char in TEXT:
        mine = sorted((l for l in lines if l["character"] == char), key=lambda l: l["start"])
        out, q = clean_character(char, mine)
        print(f"\n== {char}: {len(mine)} OCR lines -> {len(out)} lines; {q:.0%} of OCR letters match the reference")
        for l in out:
            print(f"   {l['start']:6.1f}  {l['text']:<42.42} | {l['ocr'][:40]}")
        cleaned += out
    cleaned.sort(key=lambda l: l["start"])
    (OUT / "sextet.lines.clean.json").write_text(json.dumps(cleaned, indent=1, ensure_ascii=False))
    # The Solti file gets this recording's silences applied; the clean file stays
    # recording-independent (phase9 does the same for other recordings).
    solti = copy.deepcopy(cleaned)
    st = P7.stems()
    trimmed = trim_to_silences(solti, silences(st[0], st[1]))
    solti.sort(key=lambda l: l["start"])
    print(f"\nKept lines out of clear silences in the Solti vocal stem: "
          f"{sum(k == 'end' for _, k, _, _ in trimmed)} ends cut, {sum(k == 'start' for _, k, _, _ in trimmed)} starts moved")
    for s, k, old, new in trimmed:
        print(f"   {k:5} {old:6.1f} -> {new:6.1f}  {s['character']:11} {s['text'][:40]}")
    ic = json.loads(INTERCHANGE.read_text())
    track = next(t for t in ic["tracks"] if "Sestetto" in t["title"] or "Riconosci" in t["title"])
    track["segments"] = solti
    (OUT / "librettos" / INTERCHANGE.name).write_text(json.dumps(ic, indent=1, ensure_ascii=False))
    print(f"\n{len(cleaned)} lines -> {OUT / 'librettos' / INTERCHANGE.name}")


if __name__ == "__main__":
    main()
