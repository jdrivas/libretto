"""Split the decoded score into pieces, name them from the score's contents, describe them.

A piece is a run of systems whose bars are numbered from 1: a number, one part of a number
whose bars restart (the aria of a "Recitativo istrumentato ed Aria"), a secco recitative,
or the sinfonia. Pieces start at a system that

  - carries a number heading ("N°. 19 Sestetto"), or
  - has no printed bar number (bar 1 of something) and either a heading or a tempo mark.

A "Recitativo" heading above a system whose bar numbers continue is a recitative passage
inside a number (No. 23, bar 175): recorded as a section of that piece, like the scenes
that begin inside a piece.

Pieces are named from the contents (contents.py) by the printed page they start on.
"""
import re
from collections import Counter

from . import vector

_NUMBER = re.compile(r"N[°º]\.\s*(\d+a?)")
_SCENE = re.compile(r"\bScena\s+(ultima|[IVXL]+)\b")
_ACT = re.compile(r"\bATTO\s+(PRIMO|SECONDO|TERZO|QUARTO)\b")
_GERMAN = re.compile(r"Szene|AKT\b|\bAkt\b|ERSTER|ZWEITER|DRITTER|VIERTER")
ACTS = {"PRIMO": 1, "SECONDO": 2, "TERZO": 3, "QUARTO": 4}
HEADING_MIN_SIZE = 11.5
MARK_SNAP_PT = 15  # a mark centred this close before a barline belongs to the bar it starts


def page_headings(spans):
    """Italian headings on a page as (y, text, x): upright Palatino at heading size."""
    return [(s["y"], s["t"].strip(), (s["x0"] + s["x1"]) / 2) for s in spans
            if s["font"] == "Palatino-Roman" and s["size"] >= HEADING_MIN_SIZE
            and s["t"].strip() and not _GERMAN.search(s["t"])]


def page_cues(spans):
    """Capitalised names in the music (upright Palatino): speaker cues and staff names."""
    return [(s["y"], s["t"].strip()) for s in spans
            if s["font"].startswith("Palatino") and 7.5 <= s["size"] <= 9.5
            and s["t"].strip().isupper() and len(re.sub(r"[^A-Z]", "", s["t"])) >= 3]


def split(pages):
    """pages: [(pdf_page, systems, headings)] in order. Returns raw pieces:
    {"systems": [...], "heading": str, "number": str|None, "kind": ..., "act", "scene",
     "appendix", "sections": [...]}."""
    pieces, cur = [], None
    act, scene, appendix = 0, None, False
    for pdf_page, systems, heads in pages:
        prev_bottom = 0.0
        for sy in systems:
            top, bottom = sy["staves"][0]["top"], sy["staves"][-1]["bottom"]
            mine = [(t, x) for y, t, x in heads if prev_bottom < y < top]
            text = " ".join(t for t, _ in mine)
            prev_bottom = bottom
            appendix = appendix or "APPENDIX" in text
            if (a := _ACT.search(text)):
                act = ACTS[a.group(1)]
            new_scene = _SCENE.search(text)
            num = _NUMBER.search(text)
            recit = "Recitativo" in text and not num
            fresh = sy["number"] is None and len(sy["barx"]) > 0 and len(sy["staves"]) >= 2
            if cur is None:
                kind = "sinfonia"
            elif num:
                kind = "number"
            elif recit and fresh:
                kind = "accompanied" if "istrumentato" in text else "secco"
            elif fresh and (text or sy["tempo"]):
                kind = "part"
            else:
                kind = None
            if new_scene:
                scene = new_scene.group(1)
            if kind:
                cur = {"systems": [], "heading": text, "number": num.group(1) if num else None,
                       "kind": kind, "act": act, "scene": scene, "appendix": appendix,
                       "sections": []}
                pieces.append(cur)
            elif new_scene or recit:
                # A "Recitativo" mark is printed over its bar, possibly mid-system; a scene
                # heading belongs to the system.
                mark_x = next((x for t, x in mine if "Recitativo" in t), 0) if recit else 0
                bar = (sy["number"] + sum(1 for bx in sy["barx"] if bx < mark_x + MARK_SNAP_PT)
                       if sy["number"] is not None else None)
                cur["sections"].append({"pdf_page": pdf_page, "system": sy["system"], "bar": bar,
                                        "mark": "Recitativo" if recit else f"Scena {scene}"})
            cur["systems"].append(sy)
    return pieces


def name(pieces, toc, page_of):
    """Give each piece an id and title from the contents entry that starts on its page.
    page_of(pdf_page) -> printed page. Returns the contents entries no piece or section
    accounts for."""
    by_page = {}
    for e in toc:
        by_page.setdefault(e["page"], []).append(e)
    used = set()
    last_no, recits = "00", Counter()
    parent = None
    for pc in pieces:
        first = pc["systems"][0]
        page = page_of(first["page"])
        cands = [e for e in by_page.get(page, []) if id(e) not in used
                 and e["kind"] != "scene" and e["appendix"] == pc["appendix"]]
        entry = None
        if cands:
            want = {"sinfonia": "sinfonia", "number": "number", "secco": "recitative",
                    "accompanied": "recitative", "part": "part"}[pc["kind"]]
            entry = next((e for e in cands if e["kind"] == want), None)
            if entry is None and pc["kind"] == "part":  # an appendix piece without heading
                entry = next((e for e in cands if e["kind"] == "number"), None)
            if entry is None:
                entry = cands[0]
            used.add(id(entry))
        pc["contents"] = entry
        no = (entry or {}).get("number") or pc["number"]
        prefix = "app-" if pc["appendix"] else ""
        if pc["kind"] == "sinfonia":
            pc["id"], pc["title"] = "sinfonia", "Sinfonia"
        elif no and pc["kind"] in ("number", "part"):
            base = f"no{int(re.match(r'\d+', no).group()):02d}{no.lstrip('0123456789')}"
            if not pc["appendix"]:
                last_no, recits = base[2:], Counter()
            two_part = "ed Aria" in (entry or {}).get("title", pc["heading"])
            pc["id"] = prefix + base + ("-recit" if two_part else "")
            pc["kind"] = "accompanied" if two_part else pc["kind"]
            parent = pc
        elif pc["kind"] == "part" and parent is not None:
            pc["id"] = prefix + parent["id"].removesuffix("-recit") + "-aria"
            pc["kind"] = "aria"
        else:
            recits[last_no] += 1
            pc["id"] = f"{prefix}rec{last_no}{'abcdefgh'[recits[last_no] - 1]}"
        pc["title"] = (entry or {}).get("title") or pc["heading"] or pc["id"]
        if entry and (pc["appendix"] or not pc["scene"]):
            # Appendix pieces say where they belong ("Atto quarto, in Scena X").
            pc["act"], pc["scene"] = entry["act"], entry["scene"] or pc["scene"]
    # Contents entries that begin inside a piece (scenes, the recitative inside No. 23).
    for pc in pieces:
        for sec in pc["sections"]:
            page = page_of(sec["pdf_page"])
            e = next((e for e in by_page.get(page, []) if id(e) not in used
                      and e["kind"] in ("scene", "recitative")), None)
            if e:
                used.add(id(e))
                sec["contents"] = e["title"]
    # Contents sub-entries with no heading in the score (a new passage within a scene, like
    # "Amanti costanti" in No. 23): kept as marks on the piece whose pages hold them.
    for e in toc:
        if id(e) in used or e["kind"] != "scene":
            continue
        for pc in pieces:
            first, last = page_of(pc["systems"][0]["page"]), page_of(pc["systems"][-1]["page"])
            if first <= e["page"] <= last and pc["appendix"] == e["appendix"]:
                pc["sections"].append({"pdf_page": None, "page": e["page"], "system": None,
                                       "bar": None, "mark": "contents", "contents": e["title"]})
                used.add(id(e))
                break
    return [e for e in toc if id(e) not in used]


def describe(pc, cast, cues_by_page, page_of):
    """Decode a piece and summarise it for the manifest. Returns (summary, decode)."""
    dec = vector.decode_systems(pc["systems"])
    systems = pc["systems"]
    names, unknown = set(), Counter()
    for sy in systems:
        top, bottom = sy["staves"][0]["top"], sy["staves"][-1]["bottom"]
        for y, t in cues_by_page.get(sy["page"], []):
            if top - 30 <= y <= bottom + 6:
                names |= cast.find(t)
    for st in (st for sy in systems for st in sy["staves"]):
        label = st["label"]
        # Not a staff label: a stray number, or a footnote running into the margin.
        if label and not label.isdigit() and len(label) <= 30:
            found = cast.find(label)
            names |= found
            if not found:
                unknown[label] += 1
    sung_by = Counter()
    for s in dec["syllables"]:
        for n in cast.find(s["label"]) or {"?"}:
            sung_by[n] += 1
    first_words = _first_words(dec["syllables"])
    bars = dec["bars"]
    tempos = [b["tempo"] for b in bars]
    summary = {
        "id": pc["id"], "kind": pc["kind"], "title": pc["title"], "act": pc["act"],
        "scene": pc["scene"], "appendix": pc["appendix"],
        "pages": [page_of(systems[0]["page"]), page_of(systems[-1]["page"])],
        "pdf_pages": [systems[0]["page"], systems[-1]["page"]],
        "start_system": systems[0]["system"],
        "bars": [bars[0]["bar"], bars[-1]["bar"]] if bars else None,
        "bar_count": len(bars),
        "pickup": dec["pickup"],
        "metre": bars[0]["timesig"] if bars else None,
        "tempo": next((t for t in tempos if t), None),
        "tempo_changes": len({t for t in tempos if t}),
        "systems": len(systems),
        "characters": sorted(names, key=cast.order),
        "syllables_by_staff": dict(sung_by),
        "notes": len(dec["notes"]),
        "syllables": len(dec["syllables"]),
        "first_words": first_words,
        "sections": [{k: v for k, v in s.items() if k != "pdf_page"}
                     | ({"page": page_of(s["pdf_page"])} if s["pdf_page"] else {})
                     for s in pc["sections"]],
        "problems": [re.sub(r"^p(\d+) sys (\d+)", lambda m: f"p. {page_of(int(m[1]))}, system {int(m[2]) + 1}", x)
                     for x in dec["problems"]]
                    + [f"staff label not recognised: {l!r} ({n} staves)" for l, n in unknown.items()],
    }
    return summary, dec


def _first_words(sylls, n=8):
    if not sylls:
        return ""
    first_staff = min(sylls, key=lambda s: s["t0"])["staff"]
    mine = sorted((s for s in sylls if s["staff"] == first_staff), key=lambda s: (s["t0"], s["x"]))
    text = " ".join(s["text"] for s in mine[:4 * n])
    text = re.sub(r"\s*-\s*", "", text)
    return " ".join(text.split()[:n])
