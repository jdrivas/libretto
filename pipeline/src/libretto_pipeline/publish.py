"""The `publish` stage: every timed piece of a recording -> one interchange file.

The experiment's "1.0-score" shape (what the roon-rd viewer reads), plus (decision 5):
  - top level: "edition" (the score the bars refer to);
  - per segment: "piece" (e.g. "no19", "rec19a") and, where known, "bar_start"/"bar_end"
    (bar + fraction, as printed in that edition).
Version "1.1-score". Lines are split into tracks by start time; times are track-relative.
Disc numbers count the recording's discs from 1 in order (Roon numbers a box set's
discs per album: Solti's Figaro is discs 9-11 in the files, 1-3 in Roon).

Output: <work>/<opera>/interchange/<recording>.interchange.json
"""
import copy
import json
import logging

import numpy as np

log = logging.getLogger(__name__)
VERSION = "1.1-score"


def publish(opera, rec_id):
    rec = json.loads((opera.work / "recordings" / rec_id / "recording.json").read_text())
    discs = sorted({t["disc"] or 1 for t in rec["tracks"]})
    disc_no = {d: i + 1 for i, d in enumerate(discs)}
    tracks, by_id = [], {}
    for t in rec["tracks"]:
        d = disc_no[t["disc"] or 1]
        tr = {"track_id": f"d{d}t{t['track']}", "title": t["title"], "disc_number": d,
              "track_number": t["track"], "duration_seconds": round(t["duration_s"], 3),
              "act": str(t["act"]), "segments": []}
        tracks.append(tr)
        by_id[t["id"]] = (tr, t["duration_s"])
    n_pieces = 0
    for path in sorted((opera.work / "timed" / rec_id).glob("*.json")):
        timed = json.loads(path.read_text())
        offs = timed["offsets"]
        n_pieces += 1
        for s in timed["lines"]:
            k = int(np.clip(np.searchsorted(offs, s["start"], side="right") - 1, 0, len(offs) - 1))
            tr, dur = by_id[timed["tracks"][k]]
            o, end = offs[k], offs[k] + dur
            s2 = copy.deepcopy(s)
            for key in ("start", "end"):
                s2[key] = round(min(s2[key], end) - o, 2)
            for g in s2.get("gloss", []):
                g["start"], g["end"] = round(min(g["start"], end) - o, 2), round(min(g["end"], end) - o, 2)
            for drop in ("ocr", "sylls"):
                s2.pop(drop, None)
            s2["piece"] = timed["piece"]
            tr["segments"].append(s2)
    for tr in tracks:
        tr["segments"].sort(key=lambda s: (s["start"], s.get("voice", 0)))
    ic = {"version": VERSION,
          "opera": {"title": f"{opera.composer} {opera.title}", "composer": opera.composer,
                    "language": opera.language, "translation_language": "en", "year": rec.get("year")},
          "edition": opera.edition,
          "recording": {k: rec[k] for k in ("album_title", "album_title_aliases", "artist", "conductor",
                                            "orchestra", "year", "label") if k in rec},
          "tracks": tracks}
    dst = opera.work / "interchange"
    dst.mkdir(parents=True, exist_ok=True)
    out = dst / f"{rec_id}.interchange.json"
    out.write_text(json.dumps(ic, ensure_ascii=False, indent=1))
    log.info(f"publish {rec_id}: {n_pieces} pieces, {sum(len(t['segments']) for t in tracks)} segments "
             f"in {sum(bool(t['segments']) for t in tracks)} of {len(tracks)} tracks -> {out}")
    return out
