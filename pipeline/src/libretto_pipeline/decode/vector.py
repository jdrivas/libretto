"""Read a born-digital (vector) vocal score directly from its PDF: no image OMR.

Engraved PDFs (here Bärenreiter BA 4565, set in Finale with the Maestro music font) draw
staff lines and barlines as vector lines and every note, rest, clef and accidental as a
music-font character at an exact position; lyrics, staff labels, bar numbers and tempo
marks are real text. So the score can be decoded instead of recognised:

  staves      groups of 5 equally spaced horizontal lines
  systems     staves joined by the vertical line at the left edge
  barlines    vertical lines spanning exactly one staff (or a whole grand staff)
  pitch       notehead height on its staff + clef + key signature + accidental before it
  time        bar number + the note's horizontal position between the bar's lines
              (approximate rhythm: engraved spacing grows with duration)
  lyrics      Italian = Palatino-Roman 9.2 under the staff (German is italic: ignored)
  characters  the staff label at the left of every system ("Sus.", "La C.", "Il C.", ...)
  tempo       bold tempo marks ("Allegro", "Molto andante") and time signatures

Output (per page range): a list of systems with staves, bars, notes and syllables.

Copied from libretto-scratch/sextet-experiment/ba_vector.py (2026-11-11), with the
double-barline fix; `decode_systems` decodes one piece from its systems.
"""
import re
from collections import defaultdict

import numpy as np
import pymupdf

NOTEHEADS = {"œ": "black", "˙": "half", "w": "whole"}
ACCIDENTALS = {"b": -1, "#": 1, "n": 0}
RESTS = {"Œ", "‰", "Ó", "∑", "Å", "®"}
CLEFS = {"&": "treble", "?": "bass", "V": "treble8"}
TIMESIG = set("c0123456789C")
FERMATA = {"U", "u"}
# Bottom staff line, as a diatonic step number (C4 = 28: octave*7 + step) per clef.
BOTTOM_LINE = {"treble": 4 * 7 + 2, "bass": 2 * 7 + 4, "treble8": 3 * 7 + 2}  # E4, G2, E3
STEP_SEMI = [0, 2, 4, 5, 7, 9, 11]
DOUBLE_BAR_PT = 6.0  # barlines closer than this are one double barline
# Italian lyrics (upright Palatino), matched per staff rather than as one size range,
# because stage directions in the same font are 8.6 pt on a full-size page:
#   - full-size staves (3.96 pt line spacing): 9.2 pt, sometimes 9.0;
#   - pages scaled down to fit a large ensemble: 2.33 x the spacing (8.4 pt at 3.6);
#   - the smaller choir staves: 2.49 x the spacing (7.9 pt at 3.17).
LYRIC_FULL_PT, LYRIC_FULL_TOLERANCE = 9.2, 0.25
LYRIC_PER_SP = (2.33, 2.49)
LYRIC_TOLERANCE_PT = 0.15


def is_lyric_size(size, sp):
    return (abs(size - LYRIC_FULL_PT) < LYRIC_FULL_TOLERANCE
            or any(abs(size - r * sp) < LYRIC_TOLERANCE_PT for r in LYRIC_PER_SP))
INSET_SP_RATIO = 0.75  # a system whose staves are all this much smaller is an inset
_STAGE_DIRECTION = re.compile(r"\([a-zà-ù’' ]+\)")
CHOIR_PARTS = {"Soprano", "Alto", "Tenore", "Basso", "Soprani", "Tenori", "Bassi", "I", "II"}
# Full-size noteheads are 15.9 pt on a staff with 3.96 pt line spacing; grace notes are
# about two-thirds of that. 3.6 x spacing (14.3 pt on solo staves) keeps the old 15 pt cut.
EVENT_SIZE_PER_SP = 3.6
SHARPS_ORDER, FLATS_ORDER = [3, 0, 4, 1, 5, 2, 6], [6, 2, 5, 1, 4, 0, 3]  # F C G D A E B / B E A D G C F


def chars(page):
    """Every character with font, size and origin."""
    out = []
    for b in page.get_text("rawdict")["blocks"]:
        for l in b.get("lines", []):
            for s in l["spans"]:
                for ch in s["chars"]:
                    out.append({"c": ch["c"], "font": s["font"], "size": round(s["size"], 1),
                                "x": ch["origin"][0], "y": ch["origin"][1], "bbox": ch["bbox"]})
    return out


def spans(page):
    out = []
    for b in page.get_text("dict")["blocks"]:
        for l in b.get("lines", []):
            for s in l["spans"]:
                if s["text"].strip():
                    out.append({"t": s["text"], "font": s["font"], "size": round(s["size"], 1),
                                "x0": s["bbox"][0], "x1": s["bbox"][2], "y": s["origin"][1]})
    return out


def lines(page):
    """Horizontal (long) and vertical line segments from the vector drawings."""
    hs, vs = [], []
    for d in page.get_drawings():
        for it in d["items"]:
            if it[0] == "l":
                a, b = it[1], it[2]
                x0, x1, y0, y1 = min(a.x, b.x), max(a.x, b.x), min(a.y, b.y), max(a.y, b.y)
            elif it[0] == "re":
                r = it[1]
                x0, x1, y0, y1 = r.x0, r.x1, r.y0, r.y1
            else:
                continue
            if y1 - y0 < 1.0 and x1 - x0 > 60:
                hs.append(((y0 + y1) / 2, x0, x1))
            elif x1 - x0 < 1.6 and y1 - y0 > 8:
                vs.append(((x0 + x1) / 2, y0, y1))
    return hs, vs


def staves(hs):
    """Groups of 5 horizontal lines, equally spaced, overlapping in x."""
    hs = sorted(set((round(y, 1), round(x0), round(x1)) for y, x0, x1 in hs))
    out, used = [], set()
    for i, (y, x0, x1) in enumerate(hs):
        if i in used:
            continue
        grp = [(y, x0, x1)]
        for j in range(i + 1, len(hs)):
            yj, a, b = hs[j]
            if j in used or abs(a - x0) > 3 or abs(b - x1) > 3:
                continue
            if yj - grp[-1][0] > 6:
                break
            if 2.5 < yj - grp[-1][0] < 6:
                grp.append(hs[j])
                used.add(j)
            if len(grp) == 5:
                break
        if len(grp) == 5:
            ys = [g[0] for g in grp]
            sp = float(np.mean(np.diff(ys)))
            if np.ptp(np.diff(ys)) < 0.6:
                out.append({"top": ys[0], "bottom": ys[-1], "sp": sp, "x0": x0, "x1": x1})
    return sorted(out, key=lambda s: s["top"])


def read_page(page, pno):
    cs, sp_, (hs, vs) = chars(page), spans(page), lines(page)
    sts = staves(hs)
    # Systems: staves joined by a left-edge vertical line.
    # (the first system of a piece is indented, so match each line against staves' own x0)
    left = [v for v in vs if any(abs(v[0] - s["x0"]) < 3 for s in sts)]
    systems = []
    # Longest line first: a chorus system has a short line joining the choir staves inside
    # the long one joining choir and piano, and the long one defines the system.
    for v in sorted(left, key=lambda v: (-(v[2] - v[1]), v[1])):
        members = [s for s in sts if s["top"] >= v[1] - 2 and s["bottom"] <= v[2] + 2]
        if members and not any(m in sy for sy in systems for m in members):
            systems.append(members)
    for s in sts:  # a one-staff system has no joining line
        if not any(s in sy for sy in systems):
            systems.append([s])
    systems.sort(key=lambda sy: sy[0]["top"])
    # An inset (an ossia, an alternative passage) is printed on smaller staves than the
    # page's systems and is not part of the bar count: leave it out.
    page_sp = max((s["sp"] for s in sts), default=0)
    systems = [sy for sy in systems if max(s["sp"] for s in sy) >= INSET_SP_RATIO * page_sp]
    out = []
    for si, sy in enumerate(systems):
        out.append(read_system(sy, cs, sp_, vs, pno, si))
    return out


def staff_of(y, sts, below=False):
    """Index of the staff a y belongs to (on/near it), or for lyrics the staff above."""
    for i, s in enumerate(sts):
        if below:
            nxt = sts[i + 1]["top"] if i + 1 < len(sts) else s["bottom"] + 60
            if s["bottom"] + 2 < y < nxt - 2:
                return i
        elif s["top"] - 4 * s["sp"] <= y <= s["bottom"] + 4 * s["sp"]:
            return i
    return None


def read_system(sts, cs, sp_, vs, pno, si):
    top, bottom = sts[0]["top"], sts[-1]["bottom"]
    x0, x1 = sts[0]["x0"], sts[0]["x1"]
    music = [c for c in cs if c["font"].startswith("Maestro") and top - 30 <= c["y"] <= bottom + 30]
    # Per staff: clef, key signature, notes, rests, accidentals.
    staves_out = []
    for i, s in enumerate(sts):
        near = lambda c: s["top"] - 4 * s["sp"] <= c["y"] <= s["bottom"] + 4 * s["sp"]
        mine = [c for c in music if near(c) and staff_of(c["y"], sts) == i]
        clefs = sorted((c for c in mine if c["c"] in CLEFS), key=lambda c: c["x"])
        clef = CLEFS[clefs[0]["c"]] if clefs else None
        # Key signature: accidentals right after the first clef, before any notehead/time sig.
        first_event = min([c["x"] for c in mine if c["c"] in NOTEHEADS or c["c"] in RESTS or c["c"] in TIMESIG]
                          + [x1])
        clef_x = clefs[0]["x"] if clefs else x0
        keysig = [c for c in mine if c["c"] in ("b", "#") and clef_x < c["x"] < min(first_event, clef_x + 60)]
        staves_out.append({"i": i, "top": s["top"], "bottom": s["bottom"], "sp": s["sp"], "clef": clef,
                           "key": len(keysig) * (1 if keysig and keysig[0]["c"] == "#" else -1),
                           "chars": mine, "keysig_x": max([c["x"] for c in keysig], default=clef_x)})
    # Barlines: vertical lines from a staff's top line to a staff's bottom line (its own,
    # or a lower one for a grand staff), at an x shared by most staves of the system
    # (stems can span a staff too, but never line up across staves).
    bottoms = [st["bottom"] for st in staves_out]
    hits = defaultdict(set)
    for x, a, b in vs:
        for st in staves_out:
            if abs(a - st["top"]) < 1.2 and any(abs(b - bt) < 1.2 for bt in bottoms) and x > st["keysig_x"] + 5:
                for st2 in staves_out:
                    if st2["top"] >= st["top"] - 1 and st2["bottom"] <= b + 1.2:
                        hits[round(x)].add(st2["i"])
    xs = sorted(hits)
    clusters = []
    for x in xs:
        if clusters and x - clusters[-1][-1] <= 2:
            clusters[-1].append(x)
        else:
            clusters.append([x])
    need = len(staves_out)  # every staff has a barline at every bar in this edition
    barx = [float(np.mean(c)) for c in clusters if len(set().union(*(hits[x] for x in c))) >= need]
    barx = merge_double_barlines(barx)
    # Printed bar number at the start of the system (italic, above the first staff).
    num = [s for s in sp_ if s["font"] == "Palatino-Italic" and abs(s["size"] - 9.0) < 0.2
           and re.fullmatch(r"\d+", s["t"].strip()) and s["x0"] < x0 + 15 and top - 25 < s["y"] < bottom]
    tempo = [(s["x0"], s["t"].strip()) for s in sp_ if s["font"] == "Palatino-Bold" and s["size"] < 11
             and top - 30 < s["y"] < top + 5 and re.search(r"[a-z]", s["t"])]
    # Staff labels in the left margin (abbreviated, or the full name on a piece's first system).
    for st in staves_out:
        lab = [s for s in sp_ if s["font"].startswith("Palatino") and s["x1"] < st["chars"][0]["x"] + 1
               and s["x1"] < x0 + 2 and st["top"] - 3 <= s["y"] <= st["bottom"] + 4] if st["chars"] else []
        st["label"] = " ".join(s["t"].strip() for s in lab)
    # Time signatures: "c" or stacked digits, per staff; take them from the first staff that has one.
    timesig = []
    for st in staves_out:
        mid = (st["top"] + st["bottom"]) / 2
        digits = sorted((c for c in st["chars"] if c["c"] in TIMESIG), key=lambda c: c["x"])
        cols = []
        for c in digits:
            if cols and abs(c["x"] - cols[-1][0]["x"]) < 3:
                cols[-1].append(c)
            else:
                cols.append([c])
        for col in cols:
            if len(col) == 1 and col[0]["c"] == "c":
                timesig.append((col[0]["x"], (4, 4)))
            elif len(col) == 1 and col[0]["c"] == "C":
                timesig.append((col[0]["x"], (2, 2)))
            elif len(col) == 2:
                up, lo = sorted(col, key=lambda c: c["y"])
                if up["c"].isdigit() and lo["c"].isdigit() and up["y"] < mid < lo["y"]:
                    timesig.append((up["x"], (int(up["c"]), int(lo["c"]))))
        if timesig:
            break
    # Lyrics: Italian syllables (upright Palatino at lyric size) under each staff; not the
    # all-capital character cues printed above a staff in the same font.
    for i, st in enumerate(staves_out):
        nxt = staves_out[i + 1]["top"] if i + 1 < len(staves_out) else st["bottom"] + 40
        ly = [s for s in sp_ if s["font"] == "Palatino-Roman" and is_lyric_size(s["size"], st["sp"])
              # Not a stage direction at lyric size ("(partono)"; a sung aside opens a
              # parenthesis without closing it) or a choir part name.
              and not _STAGE_DIRECTION.fullmatch(s["t"].strip())
              and s["t"].strip() not in CHOIR_PARTS
              and st["bottom"] + 2 < s["y"] < min(nxt - 2, st["bottom"] + 40) and s["x0"] > x0 - 1
              and not (s["t"].strip().isupper() and len(re.sub(r"\W", "", s["t"])) >= 3)]
        if ly:  # first lyric line only (a second upright line would be a second verse)
            y0 = min(s["y"] for s in ly)
            ly = [s for s in ly if s["y"] < y0 + 4]
        st["lyrics"] = sorted(ly, key=lambda s: s["x0"])
    return {"page": pno, "system": si, "x0": x0, "x1": x1, "staves": staves_out, "barx": barx,
            "number": int(num[0]["t"]) if num else None, "tempo": tempo, "timesig": timesig}


def pitch_of(c, st, clefs, acc_in_bar):
    """MIDI pitch of a notehead: staff position + clef (latest one to the left) + key
    signature + an accidental just before it (kept for the rest of the bar)."""
    clef = st["clef"]
    for cl in clefs:
        if cl["x"] < c["x"]:
            clef = CLEFS[cl["c"]]
    step = int(round((st["bottom"] - c["y"]) / (st["sp"] / 2)))
    dia = BOTTOM_LINE[clef or "treble"] + step
    letter, octave = dia % 7, dia // 7
    alt = 0
    k = st["key"]
    if k > 0 and letter in SHARPS_ORDER[:k]:
        alt = 1
    if k < 0 and letter in FLATS_ORDER[:-k]:
        alt = -1
    if dia in acc_in_bar:
        alt = acc_in_bar[dia]
    midi = 12 * (octave + 1) + STEP_SEMI[letter] + alt
    if clef == "treble8":
        midi -= 0  # BOTTOM_LINE already an octave down
    return midi, dia


def merge_double_barlines(barx):
    """A double barline (closing, sectional, or repeat) is two strokes about 3 pt apart:
    one barline, not a bar between them. Keep the left-hand stroke, where the bar ends;
    the next bar's fraction 0 is set by its first event anyway (decode's pads). No real
    bar is narrower than DOUBLE_BAR_PT."""
    merged = []
    for x in barx:
        if not merged or x - merged[-1] >= DOUBLE_BAR_PT:
            merged.append(x)
    return merged


def _event(c, st):
    """A full-size notehead or rest on this staff. Grace notes and cue notes are smaller;
    the size scales with the staff (choir staves are smaller than solo staves)."""
    return (c["c"] in NOTEHEADS or c["c"] in RESTS) and c["size"] >= EVENT_SIZE_PER_SP * st["sp"]


def decode(sy, first_bar):
    """Events of one system on its bars: notes (pitch, onset/end as bar + fraction),
    syllables attached to their notes. Bars are numbered from first_bar."""
    barx = sy["barx"]
    if not barx:
        return [], [], []
    evs_all = [c for st in sy["staves"] for c in st["chars"]
               if _event(c, st) and c["x"] < barx[-1]]
    start = min([c["x"] for c in evs_all], default=barx[0]) - 1.0
    edges = [start] + barx
    # Each bar's first event sits a little right of its barline: that is fraction 0.
    pads = []
    for k in range(len(barx)):
        inbar = [c["x"] for c in evs_all if edges[k] <= c["x"] < edges[k + 1]]
        pads.append((min(inbar) - edges[k]) if inbar else 0.0)

    def when(x):
        k = int(np.clip(np.searchsorted(edges, x, side="right") - 1, 0, len(barx) - 1))
        a = edges[k] + pads[k]
        return first_bar + k + float(np.clip((x - a) / max(edges[k + 1] - a, 1.0), 0, 0.999))

    bars = [{"bar": first_bar + k, "page": sy["page"], "system": sy["system"]} for k in range(len(barx))]
    notes, sylls = [], []
    for st in sy["staves"]:
        clefs = sorted((c for c in st["chars"] if c["c"] in CLEFS and c["x"] > st["keysig_x"]), key=lambda c: c["x"])
        evs = sorted((c for c in st["chars"] if _event(c, st) and c["x"] < barx[-1]), key=lambda c: c["x"])
        accs = [c for c in st["chars"] if c["c"] in ACCIDENTALS and c["x"] > st["keysig_x"] + 3]
        onsets = sorted(set(round(c["x"], 0) for c in evs))
        acc_bar, cur_bar = {}, None
        staff_notes = []
        for c in evs:
            t0 = when(c["x"])
            if int(t0) != cur_bar:
                acc_bar, cur_bar = {}, int(t0)
            later = [o for o in onsets if o > round(c["x"], 0)]
            t1 = when(later[0]) if later else first_bar + len(barx)
            t1 = min(t1, int(t0) + 1) if later and when(later[0]) >= int(t0) + 1 else t1
            if c["c"] in RESTS:
                continue
            # an accidental just left of the notehead, at its height
            for a in accs:
                if 0 < c["x"] - a["x"] < 14 and abs(a["y"] - c["y"]) < st["sp"] * 0.6:
                    step = int(round((st["bottom"] - c["y"]) / (st["sp"] / 2)))
                    clef = st["clef"]
                    for cl in clefs:
                        if cl["x"] < c["x"]:
                            clef = CLEFS[cl["c"]]
                    acc_bar[BOTTOM_LINE[clef or "treble"] + step] = ACCIDENTALS[a["c"]]
            midi, _ = pitch_of(c, st, clefs, acc_bar)
            n = {"staff": st["i"], "label": st["label"], "x": c["x"], "t0": t0, "t1": max(t1, t0 + 0.02),
                 "pitch": midi, "kind": NOTEHEADS[c["c"]]}
            notes.append(n)
            staff_notes.append(n)
        # Syllables: each lyric span goes with the notehead nearest its centre (the
        # syllable is centred under its note); a span may hold an elision ("sci o").
        heads = sorted({round(n["x"], 0): n for n in staff_notes}.values(), key=lambda n: n["x"])
        for s in st["lyrics"]:
            if not heads:
                break
            cx = (s["x0"] + s["x1"]) / 2
            n = min(heads, key=lambda n: abs(n["x"] + 3.5 - cx))
            sylls.append({"staff": st["i"], "label": st["label"], "text": s["t"].strip(),
                          "t0": n["t0"], "t1": n["t1"], "x": n["x"]})
    return bars, notes, sylls


def read_score(pdf_path, first=1, last=None):
    pdf = pymupdf.open(pdf_path)
    last = last or len(pdf)
    systems = [sy for p in range(first - 1, last) for sy in read_page(pdf[p], p + 1)]
    return decode_systems(systems)


def decode_systems(systems):
    """Decode a run of systems as one piece whose bars are numbered from 1. A printed bar
    number that disagrees with the count is reported, and the count follows the print.
    A piece that starts mid-number (an appendix excerpt) starts at its printed number.

    An upbeat: the edition does not count an incomplete opening bar, so when the first
    printed number is one less than the count, the opening bar is renumbered 0."""
    bars, notes, sylls, problems, ts_at, tempo_at = [], [], [], [], [], []
    nxt = (systems[0]["number"] if systems and systems[0]["number"] else 1)
    pickup, numbered = False, bool(systems and systems[0]["number"])
    for sy in systems:
        if sy["number"] is not None and sy["number"] != nxt:
            if not numbered and sy["number"] == nxt - 1:
                pickup = True
                for b in bars:
                    b["bar"] -= 1
                for e in notes + sylls:
                    e["t0"] -= 1
                    e["t1"] -= 1
                ts_at = [(b - 1, v) for b, v in ts_at]
                tempo_at = [(b - 1, v) for b, v in tempo_at]
            elif sy["number"] < nxt - 4:
                problems.append(f"p{sy['page']} sys {sy['system']}: printed bar {sy['number']}, counted {nxt}: "
                                "bar numbers restart inside the previous system, so a new piece starts "
                                "there (not split: pieces are split at system starts only)")
            else:
                problems.append(f"p{sy['page']} sys {sy['system']}: printed bar {sy['number']}, counted {nxt}")
            nxt = sy["number"]
        numbered = numbered or sy["number"] is not None
        b, n, s = decode(sy, nxt)
        for tx, ts in sy["timesig"]:
            ts_at.append((nxt + sum(1 for x in sy["barx"] if x < tx), ts))
        for tx, word in sy["tempo"]:
            tempo_at.append((nxt + sum(1 for x in sy["barx"] if x < tx), word))
        bars += b
        notes += n
        sylls += s
        nxt += len(sy["barx"])
    # Metre and tempo per bar, carried forward.
    ts_at.sort()
    tempo_at.sort()
    ts, tempo, ti, wi = (4, 4), None, 0, 0
    for b in bars:
        while ti < len(ts_at) and ts_at[ti][0] <= b["bar"]:
            ts = ts_at[ti][1]
            ti += 1
        while wi < len(tempo_at) and tempo_at[wi][0] <= b["bar"]:
            tempo = tempo_at[wi][1]
            wi += 1
        b["timesig"], b["tempo"] = list(ts), tempo
    return {"systems": len(systems), "pickup": pickup, "bars": bars, "notes": notes,
            "syllables": sylls, "problems": problems}
