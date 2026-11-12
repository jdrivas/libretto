"""Clean the decoded sung text against the reference text and attach the literal gloss.

From the experiment's phase8_text.py (letter DP with jump-back, word assignment, phrase
timing) and finale2_build.py (phrase cap and splitting at gaps), unchanged in method; the
reference text is a parameter: {character: [verse: [(italian phrase, literal english)]]}.
"""
import copy
import re
import unicodedata

import numpy as np

MATCH, MISMATCH, OCR_JUNK, REF_SKIP, JUMP = 2, -1, -1, -1, 5
FILL_MAX_WORDS = 4    # fill in at most this many skipped reference words
MIN_WORD_MATCH = 0.6  # a word counts only if this share of its letters match exactly
DIRECTIONS = re.compile(r"sotto|sorta|voce", re.I)  # performance directions in the lyrics
DIAG, INS, DEL, JMP = 1, 2, 3, 4
SPLIT_GAP_S = 2.0     # score seconds between phrases that split a line
PHRASE_MAX_S = 4.0    # cap on a phrase's score span


def letters(s):
    s = unicodedata.normalize("NFKC", s)
    s = "".join(ch for ch in unicodedata.normalize("NFD", s.lower()) if unicodedata.category(ch) != "Mn")
    return re.sub(r"[^a-z]", "", s)


def reference(verses):
    """Words of a character's text with (verse, chunk) ids, and the letter string."""
    words = []
    for vi, verse in enumerate(verses):
        for ci, (it, en) in enumerate(verse):
            for tok in it.split():
                if letters(tok):
                    words.append({"v": vi, "c": ci, "tok": tok})
    ref, letter_word, starts, ends = [], [], [], []
    for wi, w in enumerate(words):
        if wi == 0 or (words[wi - 1]["v"], words[wi - 1]["c"]) != (w["v"], w["c"]):
            starts.append(len(ref))
        for ch in letters(w["tok"]):
            ref.append(ch)
            letter_word.append(wi)
        ends.append(len(ref))
    return words, ref, np.array(letter_word), np.array(starts), np.array(ends)


def align_letters(ocr, ref, starts, ends):
    """Letter DP with jumps (word end -> any phrase start). Per decoded letter: the
    reference index (or -1), and whether a jump happened just before it."""
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
    i, j = n, int(np.argmax(H))
    mapping = np.full(n, -1)
    jumped = np.zeros(n, bool)
    while i > 0:
        k = op[i, j]
        if k == DIAG:
            mapping[i - 1] = j - 1
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
    for a, b in TIME_KEYS:
        if a not in line:
            continue
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


def clean_character(char, lines, verses):
    """lines: one character's decoded lines in order (each with 'sylls' carrying times).
    Returns (cleaned lines, share of decoded letters matching the reference)."""
    words, ref, letter_word, starts, ends = reference(verses)
    stream, line_of, time_of = [], [], []
    for li, ln in enumerate(lines):
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
    if not stream or not ref:
        return [], 0.0
    mapping, jumped = align_letters(stream, ref, starts, ends)
    matched = sum(1 for k, r in enumerate(mapping) if r >= 0 and ref[r] == stream[k])
    word_len = np.bincount(letter_word, minlength=len(words))

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
            if key in ow:
                tt[key] = pick(tt[key], ow[key]) if key in tt else ow[key]
    occs = [o for o in occs if o["eq"] >= max(1, MIN_WORD_MATCH * word_len[o["w"]])]
    occs = [o for i, o in enumerate(occs) if word_len[o["w"]] > 1
            or (i + 1 < len(occs) and occs[i + 1]["w"] == o["w"] + 1)]
    chunk = lambda w: (words[w]["v"], words[w]["c"])
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
                for a, b in TIME_KEYS:
                    if b in ln:
                        out[-1][b] = max(out[-1][b], ln[b])
            continue
        gloss, used = [], {}
        for w, tt in ws:
            wd = words[w]
            vid = f"{char}:{wd['v']}"
            if gloss and gloss[-1]["v"] == vid and gloss[-1]["c"] == wd["c"] and gloss[-1]["_last"] == w - 1:
                gloss[-1]["it"] += " " + wd["tok"]
                gloss[-1]["_last"] = w
            else:
                gloss.append({"v": vid, "c": wd["c"], "it": wd["tok"],
                              "en": verses[wd["v"]][wd["c"]][1], "_last": w, "_t": []})
            if tt:
                gloss[-1]["_t"].append(tt)
            used[vid] = [list(p) for p in verses[wd["v"]]]
        time_phrases(gloss, ln)
        for g in gloss:
            del g["_last"], g["_t"]
        line = {k: v for k, v in ln.items() if k != "sylls"}
        out.append({**line, "ocr": ln["text"], "text": " ".join(words[w]["tok"] for w, _ in ws),
                    "translation": " ".join(g["en"] for g in gloss), "gloss": gloss, "verses": used})
    return out, matched / max(1, len(stream))


def split_at_gaps(ln):
    """Cap each phrase at PHRASE_MAX_S (score s), then split a line where consecutive
    phrases are SPLIT_GAP_S apart, so a line does not stay lit across a long gap."""
    for g in ln["gloss"]:
        for a, b in TIME_KEYS:
            if a in g:
                g[b] = min(g[b], g[a] + PHRASE_MAX_S)
    groups = [[ln["gloss"][0]]]
    for g in ln["gloss"][1:]:
        if g["score_start"] - groups[-1][-1]["score_end"] >= SPLIT_GAP_S:
            groups.append([g])
        else:
            groups[-1].append(g)
    out = []
    for gl in groups:
        part = copy.deepcopy(ln)
        part["gloss"] = gl
        part["text"] = " ".join(g["it"] for g in gl)
        part["translation"] = " ".join(g["en"] for g in gl)
        part["verses"] = {v: c for v, c in ln["verses"].items() if any(g["v"] == v for g in gl)}
        for a, b in TIME_KEYS:
            if a in gl[0]:
                part[a], part[b] = gl[0][a], max(g[b] for g in gl)
        out.append(part)
    return out


def clean_piece(lines, text, cast_order):
    """All characters of a piece. Returns (cleaned lines in score order, report)."""
    out, report = [], {}
    for char in cast_order:
        mine = sorted((l for l in lines if l["character"] == char), key=lambda l: l["score_start"])
        if not mine:
            continue
        if char not in text:
            report[char] = {"lines": len(mine), "error": "no reference text"}
            continue
        cleaned, q = clean_character(char, mine, text[char])
        seen = {(g["v"], g["c"]) for l in cleaned for g in l["gloss"]}
        missing = [" ".join(it for it, _ in verse) for vi, verse in enumerate(text[char])
                   if not any((f"{char}:{vi}", ci) in seen for ci in range(len(verse)))]
        report[char] = {"lines": len(mine), "cleaned": len(cleaned), "letter_match": round(q, 3),
                        "verses": len(text[char]), "verses_not_found": missing}
        out += cleaned
    out = [part for ln in out for part in split_at_gaps(ln)]
    out.sort(key=lambda l: l["score_start"])
    return out, report
