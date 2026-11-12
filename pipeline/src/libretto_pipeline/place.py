"""The `place` stage (P5): where each piece lies in a recording.

Per act, the act's tracks are joined into one timeline. Each number (and part of a number)
is found in turn by subsequence DTW of its score chroma against the act's audio chroma
(CENS, PLACE_RATE frames per second), searching only after the previous number's end, so
repeated music (Nos. 8 and 9) lands in order. Each placement is scored by:
  - fit: mean cosine similarity of score and audio chroma along the path (1 = perfect);
  - margin: how much better that is than the best match elsewhere in the act;
  - titles: whether the tracks it covers name its number ("No. 12"), an independent check.
Secco recitatives are not placed by notes: their region is the gap between the numbers
around them (route C aligns their text inside it).

Output: <work>/<opera>/placement/<recording>.json
"""
import json
import logging
import re

import librosa
import numpy as np
import pandas as pd
from synctoolbox.dtw.utils import compute_optimal_chroma_shift, shift_chroma_vectors
from synctoolbox.feature.chroma import quantized_chroma_to_CENS

from . import align

log = logging.getLogger(__name__)

PLACE_RATE = 2           # placement feature rate (Hz)
CENS_WIN = 41            # smoothing window (frames at 50 Hz)
SEARCH_BACK_S = 10.0     # a piece may start this much before the previous one ended
MIN_FIT, MIN_MARGIN = 0.70, 0.02   # below either: flagged for review


def _cens(chroma):
    f, _ = quantized_chroma_to_CENS(chroma, CENS_WIN, align.RATE // PLACE_RATE, align.RATE)
    return f


def _subseq(score_f, audio_f, shift):
    """Best subsequence match of score in audio: (start frame, end frame, mean cost)."""
    s = shift_chroma_vectors(score_f, shift)
    cost = 1.0 - s.T @ audio_f
    D, wp = librosa.sequence.dtw(C=cost, subseq=True, step_sizes_sigma=np.array([[1, 1], [2, 1], [1, 2]]),
                                 weights_mul=np.array([1.0, 2.0, 2.0]))
    wp = wp[::-1]
    return int(wp[0, 1]), int(wp[-1, 1]), float(np.mean(cost[wp[:, 0], wp[:, 1]])), D[-1]


def _title_numbers(title):
    """Numbers a track title names: "No. 7, Trio ... No. 8, Chorus" -> {"7", "8"}."""
    return set(re.findall(r"\bNo\.?\s*(\d+a?)\b", title))


def place(opera, rec_id):
    rec = json.loads((opera.work / "recordings" / rec_id / "recording.json").read_text())
    manifest = json.loads((opera.work / "decode" / "manifest.json").read_text())
    pieces = [p for p in manifest["pieces"] if not p["appendix"]]
    out = {"recording": rec_id, "rate_hz": PLACE_RATE, "acts": [], "pieces": []}
    for act in sorted({t["act"] for t in rec["tracks"]}):
        tracks = [t for t in rec["tracks"] if t["act"] == act]
        mix = [opera.work / "recordings" / rec_id / "mix" / f"{t['id']}.flac" for t in tracks]
        x, tuning, offsets = align.load_mix(mix)
        a_chroma, _ = align.features_audio(x, tuning)
        a_f = _cens(a_chroma)
        dur = len(x) / align.FS
        out["acts"].append({"act": act, "tracks": [t["id"] for t in tracks],
                            "offsets": [round(o, 3) for o in offsets], "duration": round(dur, 2),
                            "tuning_cents": float(tuning)})
        act_pieces = [p for p in pieces if p["act"] == act]
        cursor, placed = 0.0, []
        for p in act_pieces:
            if p["kind"] == "secco":
                placed.append({"id": p["id"], "kind": "secco", "act": act})
                continue
            sc = json.loads((opera.work / "score" / f"{p['id']}.json").read_text())
            df = pd.DataFrame(sc["notes"]).sort_values("start").reset_index(drop=True)
            s_chroma, _ = align.features_score(df)
            s_f = _cens(s_chroma)
            lo = max(0, int((cursor - SEARCH_BACK_S) * PLACE_RATE))
            sub = a_f[:, lo:]
            shift = compute_optimal_chroma_shift(sub, s_f)
            f0, f1, c, last_row = _subseq(s_f, sub, shift)
            # The best match ending clearly outside this one: how much worse is it?
            n = s_f.shape[1]
            other = np.array(last_row, float).copy() / max(n, 1)
            other[max(0, f0 - 1):f1 + n // 2 + 1] = np.inf
            best_elsewhere = float(np.min(other)) if np.isfinite(other).any() else np.inf
            start, end = (lo + f0) / PLACE_RATE, (lo + f1 + 1) / PLACE_RATE
            entry = {"id": p["id"], "kind": p["kind"], "act": act, "start": round(start, 2), "end": round(end, 2),
                     "fit": round(1 - c, 3), "margin": round(best_elsewhere - float(last_row[f1]) / max(n, 1), 3),
                     "nominal_s": sc["score_end_s"], "tempo_ratio": round((end - start) / max(sc["score_end_s"], 1e-6), 2),
                     "chroma_shift": int(shift)}
            k0 = int(np.searchsorted(offsets, start, side="right") - 1)
            k1 = int(np.searchsorted(offsets, max(start, end - 0.5), side="right") - 1)
            entry["tracks"] = [t["id"] for t in tracks[k0:k1 + 1]]
            entry["track_start"] = [tracks[k0]["id"], round(start - offsets[k0], 2)]
            entry["track_end"] = [tracks[k1]["id"], round(end - offsets[k1], 2)]
            num = re.match(r"no0*(\d+a?)", p["id"])
            named = set().union(*(_title_numbers(t["title"]) for t in tracks[k0:k1 + 1]))
            entry["titles"] = (None if not named else (num.group(1) in named) if num else None)
            entry["flags"] = [f for f, bad in (("low fit", entry["fit"] < MIN_FIT),
                                               ("low margin", entry["margin"] < MIN_MARGIN),
                                               ("titles disagree", entry["titles"] is False),
                                               ("tempo ratio", not 0.5 <= entry["tempo_ratio"] <= 2.0)) if bad]
            placed.append(entry)
            cursor = end
        # Secco regions: from the end of what comes before to the start of what comes after.
        for i, e in enumerate(placed):
            if e["kind"] != "secco":
                continue
            before = next((q["end"] for q in reversed(placed[:i]) if "end" in q and q["kind"] != "secco"), 0.0)
            after = next((q["start"] for q in placed[i + 1:] if "start" in q and q["kind"] != "secco"), dur)
            e["region"] = [round(before, 2), round(after, 2)]
            e["region_s"] = round(after - before, 2)
            k0 = int(np.searchsorted(offsets, before, side="right") - 1)
            k1 = int(np.searchsorted(offsets, max(before, after - 0.5), side="right") - 1)
            e["tracks"] = [t["id"] for t in tracks[k0:k1 + 1]]
        out["pieces"] += placed
        flagged = [e["id"] for e in placed if e.get("flags")]
        log.info(f"{rec_id} act {act}: {sum(e['kind'] != 'secco' for e in placed)} numbers placed in "
                 f"{dur / 60:.1f} min" + (f"; flagged: {', '.join(flagged)}" if flagged else ""))
    dst = opera.work / "placement"
    dst.mkdir(parents=True, exist_ok=True)
    (dst / f"{rec_id}.json").write_text(json.dumps(out, ensure_ascii=False, indent=1))
    return out
