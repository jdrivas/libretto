"""Stage runners: per-piece outputs under <work>/<opera>/, one file per piece and stage.

  score/<piece>.json              score stage (all pieces)
  clean/<piece>.json              clean stage (pieces with a text file in operas/<opera>/text/)
  placement/<recording>.json      place stage
  timed/<recording>/<piece>.json  time stage
"""
import json
import logging

from . import clean, score_time
from .config import OPERAS

log = logging.getLogger(__name__)


def manifest(opera):
    return json.loads((opera.work / "decode" / "manifest.json").read_text())


def main_pieces(opera):
    return [p for p in manifest(opera)["pieces"] if not p["appendix"]]


def text_of(opera, pid):
    path = OPERAS / opera.id / "text" / f"{pid}.json"
    return json.loads(path.read_text()) if path.exists() else None


def run_score(opera, ids=None):
    ids = ids or [p["id"] for p in main_pieces(opera)]
    res = score_time.run(opera, ids)
    unattributed = {k: v["unattributed"] for k, v in res.items() if v["unattributed"]}
    log.info(f"score: {len(res)} pieces" + (f"; syllables with no speaker: {unattributed}" if unattributed else ""))
    return res


def _write_timed(opera, rec_id, res):
    dst = opera.work / "timed" / rec_id
    dst.mkdir(parents=True, exist_ok=True)
    (dst / f"{res['piece']}.json").write_text(json.dumps(res, ensure_ascii=False, indent=1))
    c = res["checks"]
    if c["route"] == "B":
        log.info(f"time {rec_id} {res['piece']} (B): singing agreement {c['singing_agreement']:.0%}; entries "
                 f"{c['entries_matched']}/{c['entries_predicted']} matched, median |offset| {c['median_abs_offset_s']} s, "
                 f"within 1 s {c['within_1s']}; phrases mostly silent {c['phrases_mostly_silent']}/{c['phrases']}")
    else:
        log.info(f"time {rec_id} {res['piece']} (C): {c['words']} words, mostly silent {c['words_mostly_silent']}; "
                 f"men's speeches low {c['men_speeches_low']}")


def run_time(opera, rec_id, ids=None):
    """Time pieces in a recording from its placement: numbers by route B over the tracks
    the placement found (with the placed span as the excerpt); seccos by route C over the
    region between the numbers, consecutive seccos together."""
    from . import timing
    plc = json.loads((opera.work / "placement" / f"{rec_id}.json").read_text())
    rec = json.loads((opera.work / "recordings" / rec_id / "recording.json").read_text())
    order = [t["id"] for t in rec["tracks"]]
    acts = {a["act"]: a for a in plc["acts"]}
    want = set(ids) if ids else None
    have_clean = {p.stem for p in (opera.work / "clean").glob("*.json")}
    entries = plc["pieces"]
    i = 0
    while i < len(entries):
        e = entries[i]
        act = acts[e["act"]]
        if e["kind"] != "secco":
            if (want is None or e["id"] in want) and e["id"] in have_clean:
                k0 = act["tracks"].index(e["tracks"][0])
                o0 = act["offsets"][k0]
                res = timing.time_number(opera, rec_id, e["id"], e["tracks"],
                                         excerpt=(e["start"] - o0, e["end"] - o0))
                _write_timed(opera, rec_id, res)
            i += 1
            continue
        group = [e]
        while i + len(group) < len(entries) and entries[i + len(group)]["kind"] == "secco" \
                and entries[i + len(group)]["region"] == e["region"]:
            group.append(entries[i + len(group)])
        i += len(group)
        texts = [(g["id"], (text_of(opera, g["id"]) or {}).get("speeches")) for g in group]
        if want is not None and not any(g["id"] in want for g in group):
            continue
        if not all(sp for _, sp in texts):
            log.warning(f"time {rec_id}: skipping {', '.join(g['id'] for g in group)}: no speeches text for "
                        f"{', '.join(pid for pid, sp in texts if not sp)}")
            continue
        k0 = act["tracks"].index(e["tracks"][0])
        o0 = act["offsets"][k0]
        region = (e["region"][0] - o0, e["region"][1] - o0)
        for res in timing.time_recitatives(opera, rec_id, texts, e["tracks"], region).values():
            _write_timed(opera, rec_id, res)


def run_clean(opera, ids=None):
    order = [n for n, _ in opera.cast.members]
    ids = ids or [p["id"] for p in main_pieces(opera) if text_of(opera, p["id"])]
    dst = opera.work / "clean"
    dst.mkdir(parents=True, exist_ok=True)
    for pid in ids:
        text = text_of(opera, pid)
        if text is None:
            log.warning(f"clean {pid}: no text file (operas/{opera.id}/text/{pid}.json)")
            continue
        sc = json.loads((opera.work / "score" / f"{pid}.json").read_text())
        lines, report = clean.clean_piece(sc["lines"], text["characters"], order)
        (dst / f"{pid}.json").write_text(json.dumps({"piece": pid, "lines": lines, "report": report},
                                                    ensure_ascii=False, indent=1))
        missing = sum(len(r.get("verses_not_found", [])) for r in report.values())
        worst = min((r["letter_match"] for r in report.values() if "letter_match" in r), default=None)
        log.info(f"clean {pid}: {len(lines)} lines; lowest letter match {worst}; verses not found {missing}")
