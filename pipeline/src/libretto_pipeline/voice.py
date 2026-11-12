"""The vocal stem: where singing is heard, clear silences, and the checks against it.

From the experiment's phase6_timeline.py, unchanged in method (thresholds and filters as
defined in the report, §10.1); paths and globals removed. Stems are read from the FLACs
written by `separate`, joined over consecutive tracks.
"""
import librosa
import numpy as np
import scipy.ndimage as ndi

SR, HOP = 16000, 320            # 20 ms analysis frames
PHRASE_GAP_S = 0.45             # silence (in audio seconds) that separates phrases
TEXTURE_GAP_S = 0.75            # all voices silent at least this long -> a "texture entry"
SING_DB = -24                   # vocal-stem level (dB re loud reference) counted as singing
SILENT_DB = -30                 # clearly silent vocal stem


def db(x):
    return 20 * np.log10(np.maximum(x, 1e-6))


def _load(paths):
    return np.concatenate([librosa.load(str(p), sr=SR, mono=True)[0] for p in paths])


def analyse(vocal_paths, accomp_paths, pitch=True):
    """Levels (dB re the 99.5th percentile of both stems), pYIN pitch, and singing spans.
    Returns (t, vocal dB, accompaniment dB, f0, voiced probability, spans)."""
    voc, acc = _load(vocal_paths), _load(accomp_paths)
    rms_v = librosa.feature.rms(y=voc, frame_length=1024, hop_length=HOP)[0]
    rms_a = librosa.feature.rms(y=acc, frame_length=1024, hop_length=HOP)[0]
    ref = np.percentile(np.concatenate([rms_v, rms_a]), 99.5)
    lv, la = db(rms_v / ref), db(rms_a / ref)
    n = len(lv)
    if pitch:
        # fill_na=None keeps pYIN's best guess where it isn't confident (ensembles).
        f0, _, vprob = librosa.pyin(voc, fmin=65, fmax=1050, sr=SR, frame_length=1024,
                                    hop_length=HOP, fill_na=None)
        n = min(n, len(f0))
        f0, vprob = f0[:n], vprob[:n]
        f0[lv[:n] < -30] = np.nan
    else:
        f0 = vprob = np.full(n, np.nan)
    lv, la = lv[:n], la[:n]
    t = np.arange(n) * HOP / SR
    active = ndi.median_filter(lv, 9) > SING_DB
    active = ndi.binary_closing(active, np.ones(13))  # bridge gaps < ~0.25 s
    active = ndi.binary_opening(active, np.ones(10))  # drop blips < ~0.2 s
    lab, _ = ndi.label(active)
    spans = [(t[sl[0].start], t[sl[0].stop - 1]) for sl in ndi.find_objects(lab)]
    return t, lv, la, f0, vprob, spans


def silences(t, lv, min_gap=1.0):
    """Stretches where the vocal stem is clearly silent for at least min_gap seconds."""
    quiet = ndi.median_filter(lv, 9) < SILENT_DB
    quiet = ndi.binary_closing(quiet, np.ones(13))
    lab, _ = ndi.label(quiet)
    out = []
    for (sl,) in ndi.find_objects(lab):
        a, b = t[sl.start], t[sl.stop - 1]
        if b - a >= min_gap:
            out.append((float(a), float(b)))
    return out


def trim_to_silences(segs, gaps, min_len=0.3, lead_in=0.6):
    """Keep lines out of clear silences (see the experiment's phase6_timeline.py): a line
    ending inside a silence ends where it starts; a line starting inside (or just before)
    one and running past it starts where it ends. Changes segs in place."""
    changed = []
    for s in segs:
        for g0, g1 in gaps:
            if g0 - lead_in <= s["start"] < g1 < s["end"] - min_len:
                new_start = round(g1, 2)
                changed.append((s, "start", s["start"], new_start))
                s["start"] = new_start
                for g in s.get("gloss", []):
                    if "start" in g:
                        g["start"] = max(g["start"], new_start)
                        g["end"] = max(g["end"], g["start"])
            elif s["start"] < g0 < s["end"] and g1 >= s["end"] - 0.3:
                new_end = round(max(g0, s["start"] + min_len), 2)
                changed.append((s, "end", s["end"], new_end))
                s["end"] = new_end
                for g in s.get("gloss", []):
                    if "end" in g:
                        g["start"] = min(g["start"], new_end)
                        g["end"] = min(g["end"], new_end)
    return changed


def phrases(note_list, gap=PHRASE_GAP_S):
    out = []
    for s, e, *_ in sorted(note_list):
        if out and s - out[-1][1] < gap:
            out[-1][1] = max(out[-1][1], e)
        else:
            out.append([s, e])
    return out


def any_voice_spans(notes):
    """notes: {character: [(start, end, ...)]} -> spans where any voice sings."""
    return phrases([n for ns in notes.values() for n in ns], TEXTURE_GAP_S)


def entry_offsets(pred_spans, det_spans, window=3.0):
    """For each predicted entry, offset (predicted - detected) to the nearest detected
    entry, or None beyond the window."""
    if not det_spans:
        return [(s, None) for s, _ in pred_spans]
    det = np.array([s for s, _ in det_spans])
    out = []
    for s, _ in pred_spans:
        d = s - det[np.abs(det - s).argmin()]
        out.append((s, d if abs(d) <= window else None))
    return out


def frame_agreement(t, pred_spans, det_spans, t0, t1):
    def mask(spans):
        m = np.zeros(len(t), bool)
        for s, e in spans:
            m[(t >= s) & (t <= e)] = True
        return m
    sel = (t >= t0) & (t <= t1)
    return float((mask(pred_spans)[sel] == mask(det_spans)[sel]).mean()) if sel.any() else float("nan")


def checks(t, det, lv, segs, notes, t0, t1):
    """The report's checks for one timed piece. segs: timed lines (with gloss);
    notes: {character: [(start, end)]} predicted sung phrases. Matched / predicted is
    always reported with the entry figures."""
    spans = any_voice_spans(notes)
    offs = [d for _, d in entry_offsets(spans, det) if d is not None]
    heard = np.zeros(len(t), bool)
    for s0, s1 in det:
        heard[(t >= s0) & (t <= s1)] = True
    ins = [heard[(t >= g["start"]) & (t <= g["end"])].mean() for s in segs for g in s.get("gloss", [])
           if g["end"] > g["start"] and ((t >= g["start"]) & (t <= g["end"])).any()]
    return {
        "singing_agreement": round(frame_agreement(t, spans, det, t0, t1), 3),
        "entries_predicted": len(spans),
        "entries_matched": len(offs),
        "median_abs_offset_s": round(float(np.median(np.abs(offs))), 3) if offs else None,
        "within_1s": round(float(np.mean(np.abs(offs) <= 1.0)), 3) if offs else None,
        "phrases": len(ins),
        "phrases_mostly_silent": int(sum(i < 0.5 for i in ins)),
    }
