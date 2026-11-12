"""`text` (draft): a piece's decoded sung text, laid out for writing its text file.

The text file itself (verse list, phrase split, literal gloss) is written in a session
(decision 3); this prints what the score says, in order, as the starting point:
  - speeches: runs of one character's lines, in score order (what route C needs);
  - per character: each line once, in order, repeats removed (what route B's verse
    list starts from).
Syllables are rejoined into words ("Ri - co - no - sci" -> "Riconosci").
"""
import json
import re


def words(text):
    """Rejoin hyphenated syllables; drop extender lines and empty marks."""
    t = re.sub(r"\s*-\s*", "", text)
    t = re.sub(r"_+", "", t)
    return re.sub(r"\s+", " ", t).strip()


def draft(opera, pid):
    sc = json.loads((opera.work / "score" / f"{pid}.json").read_text())
    lines = sorted(sc["lines"], key=lambda l: (l["score_start"], l["voice"]))
    speeches = []
    for ln in lines:
        text = words(ln["text"])
        if not text:
            continue
        bar = f"{ln['bar_start']:.1f}"
        if speeches and speeches[-1]["character"] == ln["character"]:
            speeches[-1]["text"] += " " + text
        else:
            speeches.append({"character": ln["character"], "bar": bar, "text": text})
    out = [f"# {pid}: decoded text ({len(speeches)} speeches)", "", "## Speeches in score order", ""]
    out += [f"- bar {s['bar']:>6}  {s['character']}: {s['text']}" for s in speeches]
    out += ["", "## Per character, repeats removed", ""]
    for char in [n for n, _ in opera.cast.members]:
        mine = [words(l["text"]) for l in lines if l["character"] == char]
        seen, uniq = set(), []
        for t in mine:
            key = re.sub(r"[^a-zà-ù]", "", t.lower())
            if t and key not in seen:
                seen.add(key)
                uniq.append(t)
        if uniq:
            out.append(f"### {char}")
            out += [f"- {t}" for t in uniq]
            out.append("")
    return "\n".join(out)
