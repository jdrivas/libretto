"""The score's own table of contents, as a check on how the decode splits the score.

Bärenreiter prints each contents page as an Italian block followed by the same entries in
German. Entries end with their printed page number; a page number lower than the previous
one marks the start of the German block, which is skipped.
"""
import re

_PAGE_AT_END = re.compile(r"^(.*?)[\s.]*?(\d{1,3})\s*$")
_ROMAN = re.compile(r"^[IVXLC]+$")
_ACT = re.compile(r"\bAtto\s+(primo|secondo|terzo|quarto)\b", re.I)
_SCENE = re.compile(r"\bScena\s+(ultima|[IVXL]+)\b")
_NUMBER = re.compile(r"\bNo\.\s*(\d+a?)\b")
ACTS = {"primo": 1, "secondo": 2, "terzo": 3, "quarto": 4}


def _entries(text):
    """(entry text, page) pairs from one contents page, Italian block only."""
    out, buf, last_page = [], [], 0
    for raw in text.splitlines():
        line = raw.strip()
        if not line or _ROMAN.match(line) or line.startswith(("Issued for", "DP-")) \
                or line.isupper():
            continue
        m = _PAGE_AT_END.match(line)
        if m and (m.group(1).endswith(".") or m.group(1) == "" or "  " in raw or " . " in line):
            page = int(m.group(2))
            if page < last_page:
                break  # German block
            last_page = page
            entry = " ".join(buf + [m.group(1)]).replace(" .", "").strip(" .")
            out.append((re.sub(r"\s+", " ", entry), page))
            buf = []
        else:
            buf.append(line)
    return out


def read_contents(pdf, pdf_pages):
    """Contents entries: {act, scene, kind, number, title, page} in score order.
    kind is 'sinfonia', 'number', 'recitative' (secco), 'part' (a section of a number,
    e.g. its aria) or 'scene' (a scene that begins inside a piece)."""
    act, scene, appendix, out = 0, None, False, []
    for p in pdf_pages:
        for text, page in _entries(pdf[p - 1].get_text()):
            appendix = appendix or "Appendix" in text
            # A heading line (act, scene) is folded into the entry that follows it.
            a = _ACT.search(text)
            if a:
                act = ACTS[a.group(1).lower()]
            s = _SCENE.search(text)
            if s:
                scene = s.group(1)
            body = _SCENE.sub("", _ACT.sub("", text)).strip(" –-")
            n = _NUMBER.search(body)
            if body.startswith("Sinfonia"):
                kind = "sinfonia"
            elif n:
                kind = "number"
            elif body.startswith("Recitativo istrumentato") or body.startswith("Aria"):
                kind = "part"
            elif body.startswith("Recitativo"):
                kind = "recitative"
            else:
                kind = "scene"
            if appendix:
                body = body[body.find("No."):] if "No." in body else body
            out.append({"act": act, "scene": scene, "kind": kind, "appendix": appendix,
                        "number": n.group(1) if n else None, "title": body, "page": page})
    return out
