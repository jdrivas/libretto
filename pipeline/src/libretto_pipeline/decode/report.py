"""The manifest as a Markdown table for review."""

KIND = {"sinfonia": "sinfonia", "number": "number", "accompanied": "acc. recit",
        "aria": "aria", "part": "part", "secco": "secco"}
ABBR = {"IL CONTE": "Conte", "LA CONTESSA": "Contessa", "SUSANNA": "Sus", "FIGARO": "Fig",
        "CHERUBINO": "Cher", "MARCELLINA": "Marc", "BARTOLO": "Bart", "BASILIO": "Bas",
        "DON CURZIO": "Curz", "BARBARINA": "Barb", "ANTONIO": "Ant", "DUE DONNE": "2 donne",
        "CORO": "Coro"}


def manifest_markdown(m):
    c = m["checks"]
    out = [f"# {m['title']}: decoded score manifest", "",
           f"Edition: {m['edition']}. {c['pieces']} pieces; {c['with_problems']} with problems; "
           f"{c['contents_entries']} contents entries, {len(c['contents_not_found'])} not found.", ""]
    if c["contents_not_found"] or c["pieces_not_in_contents"] or c["duplicate_ids"]:
        out += ["## Checks", ""]
        for e in c["contents_not_found"]:
            out.append(f"- Contents entry not found: p. {e['page']} {e['kind']}: {e['title'][:70]}")
        for i in c["pieces_not_in_contents"]:
            out.append(f"- Piece not in the contents: {i}")
        for i in c["duplicate_ids"]:
            out.append(f"- Duplicate id: {i}")
        out.append("")
    out += ["## Pieces", "",
            "| id | act/sc. | kind | title | pages | bars | metre, tempo | characters | first words | problems |",
            "|---|---|---|---|---|---|---|---|---|---|"]
    for p in m["pieces"]:
        title = p["title"].split("(")[0].strip()[:48]
        bars = f"{p['bars'][0]}–{p['bars'][1]}" if p["bars"] else "–"
        metre = f"{p['metre'][0]}/{p['metre'][1]}" if p["metre"] else ""
        tempo = f"{metre} {p['tempo'] or ''}".strip()
        chars = ", ".join(ABBR.get(c, c) for c in p["characters"])
        act = f"{'A' if p['appendix'] else ''}{p['act']}/{p['scene'] or ''}"
        out.append(f"| {p['id']} | {act} | {KIND.get(p['kind'], p['kind'])} | {title} | "
                   f"{p['pages'][0]}–{p['pages'][1]} | {bars} | {tempo} | {chars} | "
                   f"{p['first_words'][:40]} | {len(p['problems']) or ''} |")
    probs = [p for p in m["pieces"] if p["problems"]]
    if probs:
        out += ["", "## Problems by piece", ""]
        for p in probs:
            out.append(f"### {p['id']}")
            out += [f"- {x}" for x in p["problems"][:12]]
            if len(p["problems"]) > 12:
                out.append(f"- … and {len(p['problems']) - 12} more")
            out.append("")
    secs = [p for p in m["pieces"] if p["sections"]]
    if secs:
        out += ["", "## Sections inside pieces", ""]
        for p in secs:
            marks = "; ".join(
                f"contents entry “{s['contents'][:40]}” (p. {s['page']})" if s["mark"] == "contents"
                else f"{s['mark']} at bar {s['bar']} (p. {s['page']})" for s in p["sections"])
            out.append(f"- {p['id']}: {marks}")
    return "\n".join(out) + "\n"
