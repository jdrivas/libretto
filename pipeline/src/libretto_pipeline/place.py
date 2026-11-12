"""The `place` stage (P5): where each piece lies in a recording.

Per act, the act's tracks are joined into one timeline, and every piece of the act
(numbers and secco recitatives) is joined in score order into one score. One DTW aligns
that score to the act's audio from start to end (CENS chroma, PLACE_RATE frames per
second, tempo between a third and three times nominal), so every boundary comes from one
consistent path: a piece can't be matched out of order, and repeated music (Nos. 8 and 9)
lands where it belongs. Per piece:
  - start, end: where its score frames fall on the path;
  - fit: mean cosine similarity of score and audio chroma along its part of the path
    (secco recitative is sparse, so expect it lower than for numbers);
  - tempo ratio: its audio duration over its nominal score duration;
  - titles: whether the tracks it covers name its number ("No. 12"), an independent check.
Secco recitatives also get a region for route C: from the end of the number before to the
start of the number after (shared by consecutive recitatives), so a slightly misplaced
boundary can't cut a speech off.

Output: <work>/<opera>/placement/<recording>.json
"""
import json
import logging
import re

import librosa
import numpy as np
import pandas as pd
from synctoolbox.dtw.utils import shift_chroma_vectors
from synctoolbox.feature.chroma import quantized_chroma_to_CENS

from . import align

log = logging.getLogger(__name__)

PLACE_RATE = 2           # placement feature rate (Hz)
CENS_WIN = 41            # smoothing window (frames at 50 Hz)
SHIFTS = (0, 1, 11)      # chroma shifts tried for the act: none, a semitone up or down
MIN_FIT = 0.65           # numbers below this are flagged for review
TEMPO_RANGE = (0.4, 2.8) # numbers outside this audio/nominal ratio are flagged (cut? wrong?)


def _cens(chroma):
    f, _ = quantized_chroma_to_CENS(chroma, CENS_WIN, align.RATE // PLACE_RATE, align.RATE)
    return f


def _title_numbers(title):
    """Numbers a track title names: "No. 7, Trio ... No. 8, Chorus" -> {"7", "8"}."""
    return set(re.findall(r"\bNo\.?\s*(\d+a?)\b", title))


def _act_score(opera, act_pieces):
    """All pieces' notes joined in order. Returns (notes DataFrame, [(piece, start s, end s)])."""
    frames, spans, t = [], [], 0.0
    for p in act_pieces:
        sc = json.loads((opera.work / "score" / f"{p['id']}.json").read_text())
        df = pd.DataFrame(sc["notes"])
        if len(df):
            df["start"] = df["start"] + t
            frames.append(df)
        spans.append((p, t, t + sc["score_end_s"]))
        t += sc["score_end_s"]
    return pd.concat(frames).sort_values("start").reset_index(drop=True), spans


def _track_span(tracks, offsets, start, end):
    k0 = int(np.searchsorted(offsets, start, side="right") - 1)
    k1 = int(np.searchsorted(offsets, max(start, end - 0.5), side="right") - 1)
    return k0, k1


def place(opera, rec_id, acts=None):
    rec = json.loads((opera.work / "recordings" / rec_id / "recording.json").read_text())
    manifest = json.loads((opera.work / "decode" / "manifest.json").read_text())
    pieces = [p for p in manifest["pieces"] if not p["appendix"]]
    dst = opera.work / "placement" / f"{rec_id}.json"
    out = {"recording": rec_id, "rate_hz": PLACE_RATE, "acts": [], "pieces": []}
    if acts is not None and dst.exists():  # re-placing some acts: keep the others
        old = json.loads(dst.read_text())
        out["acts"] = [a for a in old["acts"] if a["act"] not in acts]
        out["pieces"] = [p for p in old["pieces"] if p["act"] not in acts]
    for act in sorted({t["act"] for t in rec["tracks"]}):
        if acts is not None and act not in acts:
            continue
        tracks = [t for t in rec["tracks"] if t["act"] == act]
        act_pieces = [p for p in pieces if p["act"] == act]
        mix = [opera.work / "recordings" / rec_id / "mix" / f"{t['id']}.flac" for t in tracks]
        x, tuning, offsets = align.load_mix(mix)
        a_f = _cens(align.features_audio(x, tuning)[0])
        dur = len(x) / align.FS
        df, spans = _act_score(opera, act_pieces)
        s_f = _cens(align.features_score(df)[0])
        best = None
        for shift in SHIFTS:
            cost = 1.0 - shift_chroma_vectors(s_f, shift).T @ a_f
            D, wp = librosa.sequence.dtw(C=cost, step_sizes_sigma=np.array([[1, 1], [2, 1], [1, 2], [3, 1], [1, 3]]),
                                         weights_mul=np.array([1.0, 2.0, 2.0, 3.0, 3.0]))
            wp = wp[::-1]
            i = np.clip(wp[:, 0], 0, cost.shape[0] - 1)
            j = np.clip(wp[:, 1], 0, cost.shape[1] - 1)
            c = cost[i, j]
            if best is None or c.mean() < best[0]:
                best = (float(c.mean()), shift, i, j, c)
        mean_cost, shift, pi, pj, pc = best
        out["acts"].append({"act": act, "tracks": [t["id"] for t in tracks],
                            "offsets": [round(o, 3) for o in offsets], "duration": round(dur, 2),
                            "tuning_cents": float(tuning), "chroma_shift": int(shift),
                            "fit": round(1 - mean_cost, 3)})
        placed = []
        for p, s0, s1 in spans:
            f0, f1 = int(s0 * PLACE_RATE), max(int(s0 * PLACE_RATE), int(np.ceil(s1 * PLACE_RATE)) - 1)
            on = (pi >= f0) & (pi <= f1)
            start, end = pj[on].min() / PLACE_RATE, (pj[on].max() + 1) / PLACE_RATE
            k0, k1 = _track_span(tracks, offsets, start, end)
            e = {"id": p["id"], "kind": p["kind"], "act": act, "start": round(float(start), 2),
                 "end": round(float(end), 2), "fit": round(1 - float(pc[on].mean()), 3),
                 "nominal_s": round(s1 - s0, 1), "tempo_ratio": round(float(end - start) / max(s1 - s0, 1e-6), 2),
                 "tracks": [t["id"] for t in tracks[k0:k1 + 1]],
                 "track_start": [tracks[k0]["id"], round(float(start - offsets[k0]), 2)],
                 "track_end": [tracks[k1]["id"], round(float(end - offsets[k1]), 2)]}
            num = re.match(r"no0*(\d+a?)", p["id"])
            named = set().union(*(_title_numbers(t["title"]) for t in tracks[k0:k1 + 1]))
            e["titles"] = (num.group(1) in named) if (num and named) else None
            if p["kind"] != "secco":
                e["flags"] = [f for f, bad in (("low fit", e["fit"] < MIN_FIT),
                                               ("titles disagree", e["titles"] is False),
                                               ("tempo ratio", not TEMPO_RANGE[0] <= e["tempo_ratio"] <= TEMPO_RANGE[1]))
                              if bad]
            placed.append(e)
        # Route C regions: between the numbers around each run of recitatives.
        for i, e in enumerate(placed):
            if e["kind"] != "secco":
                continue
            before = next((q["end"] for q in reversed(placed[:i]) if q["kind"] != "secco"), 0.0)
            after = next((q["start"] for q in placed[i + 1:] if q["kind"] != "secco"), dur)
            e["region"] = [round(before, 2), round(after, 2)]
            k0, k1 = _track_span(tracks, offsets, before, after)
            e["region_tracks"] = [t["id"] for t in tracks[k0:k1 + 1]]
        out["pieces"] += placed
        flagged = [e["id"] for e in placed if e.get("flags")]
        log.info(f"{rec_id} act {act}: {len(placed)} pieces placed in {dur / 60:.1f} min (fit {1 - mean_cost:.2f})"
                 + (f"; flagged: {', '.join(flagged)}" if flagged else ""))
    out["acts"].sort(key=lambda a: a["act"])
    act_order = {a: i for i, a in enumerate(sorted({p["act"] for p in out["pieces"]}))}
    out["pieces"].sort(key=lambda p: act_order[p["act"]])  # stable: score order within an act
    dst.parent.mkdir(parents=True, exist_ok=True)
    dst.write_text(json.dumps(out, ensure_ascii=False, indent=1))
    return out
