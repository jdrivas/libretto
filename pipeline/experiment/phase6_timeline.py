"""Phase 6: diagnostic timeline of vocal entries/exits vs. the score alignments.

Inputs:
  work/demucs/htdemucs/sextet/vocals.wav, no_vocals.wav  (Demucs htdemucs, 2 stems)
  MIDI score (aligned to the recording)          -> vocal notes + bars, "MIDI-based"
  Typeset OMR v1 (aligned, no MIDI)              -> vocal notes + bars, "scan-only"

Analysis of the vocal stem:
  - loudness (RMS, dB) of vocals and accompaniment
  - sung pitch (pYIN) on the vocal stem
  - detected singing spans (voiced + loud enough), and their entries

Writes out/timeline/data.js (consumed by out/timeline/index.html) and prints
how well each alignment's predicted vocal entries match the detected ones.
"""
import csv
import json
from pathlib import Path

import librosa
import music21
import numpy as np
import pretty_midi
import scipy.ndimage as ndi

from phase2_compare import SEC_PER_QUARTER, load_omr
from phase4_align import MIDI, OUT, align, load_audio, score_dataframe

HERE = Path(__file__).parent
STEMS = HERE / "work" / "demucs" / "htdemucs" / "sextet"
OMR_V1 = HERE / "work" / "omr_typeset"
CAST = ["SUSANNA", "MARCELLINA", "DON CURZIO", "IL CONTE", "DON BARTOLO", "FIGARO"]
OMR_VOCAL_STAVES = {7: "SUSANNA", 8: "MARCELLINA", 9: "DON CURZIO",
                    10: "IL CONTE", 11: "DON BARTOLO", 12: "FIGARO"}
SR, HOP = 16000, 320            # 20 ms analysis frames
PHRASE_GAP_S = 0.45             # silence (in audio seconds) that separates phrases
TEXTURE_GAP_S = 0.75            # all voices silent at least this long -> a "texture entry"
SING_DB = -24                   # vocal-stem level (dB re loud reference) counted as singing


# ---------- score side ----------

def midi_vocal_notes(pm, to_audio):
    parts = json.loads((OUT / "sextet.parts.json").read_text())
    notes = {c: [] for c in CAST}
    for inst in pm.instruments:
        char = parts.get(inst.name)
        if char:
            for n in inst.notes:
                notes[char].append((to_audio(n.start), to_audio(n.end), n.pitch))
    missing = [c for c in CAST if not notes[c]]
    assert not missing, f"no MIDI notes for {missing}; instrument names: {[i.name for i in pm.instruments]}"
    return notes


def omr_vocal_notes(to_audio):
    score = music21.converter.parse(str(next(OMR_V1.glob("*.mxl")))).stripTies()
    assert len(score.parts) == 15, f"expected 15 staves, got {len(score.parts)}"
    notes = {c: [] for c in CAST}
    for pi, char in OMR_VOCAL_STAVES.items():
        for n in score.parts[pi].recurse().notes:
            off = float(n.getOffsetInHierarchy(score)) * SEC_PER_QUARTER
            end = off + float(n.duration.quarterLength) * SEC_PER_QUARTER
            for p in n.pitches:
                notes[char].append((to_audio(off), to_audio(end), p.midi))
    return notes


def phrases(note_list, gap=PHRASE_GAP_S):
    """Merge a character's notes into sung phrases (start, end)."""
    out = []
    for s, e, _ in sorted(note_list):
        if out and s - out[-1][1] < gap:
            out[-1][1] = max(out[-1][1], e)
        else:
            out.append([s, e])
    return out


def any_voice_spans(notes):
    return phrases([n for c in CAST for n in notes[c]], TEXTURE_GAP_S)


# ---------- audio side ----------

def db(x):
    return 20 * np.log10(np.maximum(x, 1e-6))


def analyse_stems():
    voc, _ = librosa.load(str(STEMS / "vocals.wav"), sr=SR, mono=True)
    acc, _ = librosa.load(str(STEMS / "no_vocals.wav"), sr=SR, mono=True)
    rms_v = librosa.feature.rms(y=voc, frame_length=1024, hop_length=HOP)[0]
    rms_a = librosa.feature.rms(y=acc, frame_length=1024, hop_length=HOP)[0]
    ref = np.percentile(np.concatenate([rms_v, rms_a]), 99.5)
    lv, la = db(rms_v / ref), db(rms_a / ref)
    print("  pitch tracking the vocal stem (pYIN)...")
    # fill_na=None keeps pYIN's best pitch guess even where it isn't confident:
    # with several voices at once its confidence is low almost everywhere.
    f0, voiced, vprob = librosa.pyin(voc, fmin=65, fmax=1050, sr=SR, frame_length=1024,
                                     hop_length=HOP, fill_na=None)
    n = min(len(lv), len(f0))
    lv, la, f0, vprob = lv[:n], la[:n], f0[:n], vprob[:n]
    f0[lv < -30] = np.nan  # no pitch where the vocal stem is near-silent
    t = np.arange(n) * HOP / SR
    # Singing = vocal stem loud enough (score-sung frames median -16 dB, silent -32 dB);
    # then merge short gaps, drop blips.
    active = ndi.median_filter(lv, 9) > SING_DB
    active = ndi.binary_closing(active, np.ones(13))  # bridge gaps < ~0.25 s
    active = ndi.binary_opening(active, np.ones(10))  # drop blips < ~0.2 s
    lab, k = ndi.label(active)
    spans = [(t[sl[0].start], t[sl[0].stop - 1]) for sl in ndi.find_objects(lab)]
    return t, lv, la, f0, vprob, spans


SILENT_DB = -30  # clearly silent vocal stem (sung frames median -16 dB, silent -32 dB)


def silences(t, lv, min_gap=1.0):
    """Stretches where the vocal stem is clearly silent for at least min_gap seconds.
    Stricter than 'not singing' (SING_DB): quiet sotto voce singing sits in between."""
    quiet = ndi.median_filter(lv, 9) < SILENT_DB
    quiet = ndi.binary_closing(quiet, np.ones(13))  # a breath or blip doesn't end a silence
    lab, _ = ndi.label(quiet)
    out = []
    for (sl,) in ndi.find_objects(lab):
        a, b = t[sl.start], t[sl.stop - 1]
        if b - a >= min_gap:
            out.append((float(a), float(b)))
    return out


def trim_to_silences(segs, gaps, min_len=0.3, lead_in=0.6):
    """Keep lines out of clear silences in this recording.

    The score has no length for fermatas and pauses, so alignment stretches or shifts the
    notes around them: lines stay lit through a silence, or start before it is over. For
    each silence (from silences()):
      - a line that starts before it and would end inside it (or within 0.3 s after)
        ends where the silence starts;
      - a line that would start inside it, or less than lead_in seconds before it, and
        runs on past it starts where it ends (a line doesn't open with a moment of
        singing and then a long silence: that start belongs after the pause).
    Phrases are clipped to match. Nothing else moves. Changes segs in place; returns
    [(line, "end"|"start", old, new)]."""
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


# ---------- comparison ----------

def entry_offsets(pred_spans, det_spans, window=3.0):
    """For each predicted texture entry, offset (pred - detected) to the nearest detected entry."""
    det = np.array([s for s, _ in det_spans])
    out = []
    for s, _ in pred_spans:
        j = np.abs(det - s).argmin()
        d = s - det[j]
        out.append((s, d if abs(d) <= window else None))
    return out


def frame_agreement(t, pred_spans, det_spans, t0, t1):
    def mask(spans):
        m = np.zeros(len(t), bool)
        for s, e in spans:
            m[(t >= s) & (t <= e)] = True
        return m
    sel = (t >= t0) & (t <= t1)
    return float((mask(pred_spans)[sel] == mask(det_spans)[sel]).mean())


def main():
    x, tuning = load_audio()
    pm = pretty_midi.PrettyMIDI(str(MIDI))
    midi_map = align(x, tuning, score_dataframe(pm), "midi")
    df, omr_bars, _, _ = load_omr(OMR_V1, {3: -7})
    omr_map = align(x, tuning, df, "scan")

    ref = list(csv.DictReader(open(OUT / "sextet.bars.aligned.csv")))
    bars = {"midi": [round(float(r["audio_s"]), 3) for r in ref],
            "scan": [round(omr_map(b), 3) for b in omr_bars]}
    notes = {"midi": midi_vocal_notes(pm, midi_map), "scan": omr_vocal_notes(omr_map)}

    print("  analysing vocal stem...")
    t, lv, la, f0, vprob, det = analyse_stems()

    t0, t1 = bars["midi"][0], bars["midi"][-1] + 2.0
    det_in = [s for s in det if s[1] >= t0 - 1 and s[0] <= t1]
    print(f"\nDetected {len(det_in)} singing spans in the Sextet ({t0:.1f}-{t1:.1f}s)")
    summary = {}
    for k in ("midi", "scan"):
        spans = any_voice_spans(notes[k])
        offs = entry_offsets(spans, det_in)
        found = [d for _, d in offs if d is not None]
        agree = frame_agreement(t, spans, det_in, t0, t1)
        summary[k] = {"entries": len(offs), "matched": len(found),
                      "median_abs": round(float(np.median(np.abs(found))), 2) if found else None,
                      "within_0_5": round(float(np.mean(np.abs(found) <= 0.5)), 2) if found else None,
                      "within_1": round(float(np.mean(np.abs(found) <= 1.0)), 2) if found else None,
                      "frame_agreement": round(agree, 3)}
        print(f"  {k:4}: {len(offs)} predicted texture entries, {len(found)} with a detected entry within 3 s; "
              f"median |offset| {summary[k]['median_abs']} s; within 0.5 s {summary[k]['within_0_5']:.0%}, "
              f"within 1 s {summary[k]['within_1']:.0%}; singing/silent frame agreement {agree:.0%}")
        for s, d in offs:
            print(f"        entry at {s:6.1f}s  ->  " + (f"{d:+.2f}s" if d is not None else "no detected entry"))

    keep = ~np.isnan(f0)
    data = {
        "duration": round(float(t[-1]), 2),
        "frame": HOP / SR,
        "vocal_db": [round(float(v), 1) for v in lv[::2]],     # 25 Hz
        "accomp_db": [round(float(v), 1) for v in la[::2]],
        "level_rate": SR / HOP / 2,
        "f0": [[round(float(tt), 2), round(float(librosa.hz_to_midi(ff)), 2), round(float(p), 2)]
               for tt, ff, p in zip(t[keep], f0[keep], vprob[keep])],
        "detected": [[round(float(s), 2), round(float(e), 2)] for s, e in det],
        "cast": CAST,
        "bars": bars,
        "notes": {k: {c: [[round(s, 3), round(e, 3), p] for s, e, p in v[c]] for c in CAST}
                  for k, v in notes.items()},
        "summary": summary,
    }
    dest = OUT / "timeline"
    dest.mkdir(exist_ok=True)
    (dest / "data.js").write_text("window.TIMELINE = " + json.dumps(data, separators=(",", ":")) + ";\n")
    print(f"\nWrote {dest / 'data.js'}")


if __name__ == "__main__":
    main()
