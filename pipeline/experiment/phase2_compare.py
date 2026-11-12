"""Phase 2: is an OMR-derived score good enough to align with?

For each Audiveris MusicXML export (Ricordi scan, typeset PDF):
  - summarise what OMR produced (measures, notes, parts, lyrics, odd measures)
  - align it to the Solti recording with the same pipeline as Phase 4
  - compare its barline times with the MIDI-reference barline times

Usage: phase2_compare.py <label> <omr_output_dir> [<label> <dir> ...]
"""
import csv
import sys
from pathlib import Path

import music21
import numpy as np
import pandas as pd

from phase4_align import OUT, align, load_audio

SEC_PER_QUARTER = 0.5  # nominal 120 bpm; DTW absorbs tempo differences

# Sounding-pitch corrections Audiveris can't know (it names every part "Voice"/"Piano").
# Keyed by run label -> {part index (staff order): semitones}.
# Typeset full score staff order: Fl, Ob, Fag, Cor in F, Vl I, Vl II, Vla, S, M, DC, C, DB, F, Vc, Cb.
TRANSPOSE = {"typeset": {3: -7}}  # horns in F sound a perfect fifth below written


def load_omr(folder: Path, transpose=None):
    transpose = transpose or {}
    files = sorted(folder.rglob("*.mxl")) + sorted(folder.rglob("*.musicxml")) + sorted(folder.rglob("*.xml"))
    if not files:
        raise SystemExit(f"no MusicXML in {folder}")
    rows, bar_starts, odd, lyrics, parts = [], [], 0, [], set()
    base = 0.0  # running offset (quarters) across movements/files
    bar_base = 0
    for f in files:
        score = music21.converter.parse(str(f))
        score = score.stripTies()  # a tied note is one sounding note, not two onsets
        parts.add(len(score.parts))
        ref = score.parts[0]
        measures = list(ref.getElementsByClass(music21.stream.Measure))
        for m in measures:
            bar_starts.append(base + float(m.offset))
            if abs(float(m.duration.quarterLength) - 4.0) > 1e-3:
                odd += 1
        for pi, p in enumerate(score.parts):
            if isinstance(transpose, dict):
                semis = transpose.get(pi, 0)
            else:  # list of (regex on part name, semitones)
                semis = next((s for rx, s in transpose if rx.search(p.partName or "")), 0)
            for mi, m in enumerate(p.getElementsByClass(music21.stream.Measure)):
                for n in m.recurse().notes:
                    off = base + float(n.getOffsetInHierarchy(score))
                    dur = max(float(n.duration.quarterLength), 0.125)
                    if n.lyric:
                        lyrics.append((bar_base + mi + 1, pi, n.lyric))
                    for pitch in n.pitches:
                        rows.append({"start": off * SEC_PER_QUARTER, "duration": dur * SEC_PER_QUARTER,
                                     "pitch": pitch.midi + semis, "velocity": 80,
                                     "instrument": p.partName or "part"})
        base += float(score.highestTime)
        bar_base += len(measures)
    df = pd.DataFrame(rows).sort_values("start").reset_index(drop=True)
    summary = {"files": len(files), "measures": len(bar_starts), "notes": len(df),
               "parts_per_file": sorted(parts), "non_4/4_measures": odd,
               "notes_with_lyrics": len(lyrics)}
    return df, [b * SEC_PER_QUARTER for b in bar_starts], summary, lyrics


def main():
    args = sys.argv[1:]
    ref = list(csv.DictReader(open(OUT / "sextet.bars.aligned.csv")))
    ref_t = np.array([float(r["audio_s"]) for r in ref])
    x, tuning = load_audio()

    report = []
    for label, folder in zip(args[::2], args[1::2]):
        df, bars, summary, lyrics = load_omr(Path(folder), TRANSPOSE.get(label.split("_")[0]))
        print(f"\n== {label}: {summary}")
        if lyrics:
            with open(OUT / f"phase2_{label}.lyrics.csv", "w", newline="") as fh:
                w = csv.writer(fh)
                w.writerow(["omr_bar", "part_index", "syllable"])
                w.writerows(lyrics)
            print("   first lyrics:", " ".join(s for _, _, s in lyrics[:25]))
        to_audio = align(x, tuning, df, label)
        t = np.array([to_audio(b) for b in bars])
        if len(t) == len(ref_t):
            d = t - ref_t
            print(f"    same bar count -> direct bar-by-bar: median |err| {np.median(np.abs(d)):.2f}s, "
                  f"max {np.abs(d).max():.2f}s, within 1s {(np.abs(d) < 1).mean():.0%}, "
                  f"within 2s {(np.abs(d) < 2).mean():.0%}")
        # nearest reference barline for each OMR barline
        idx = np.abs(t[:, None] - ref_t[None, :]).argmin(axis=1)
        err = np.abs(t - ref_t[idx])
        # reference barlines with no OMR barline within 0.5 s (missed bars)
        missed = int((np.abs(ref_t[:, None] - t[None, :]).min(axis=1) > 0.5).sum())
        stats = {
            "label": label, **summary,
            "median_err_s": round(float(np.median(err)), 3),
            "p90_err_s": round(float(np.percentile(err, 90)), 3),
            "max_err_s": round(float(err.max()), 3),
            "within_0.25s": f"{(err < 0.25).mean():.0%}",
            "within_0.5s": f"{(err < 0.5).mean():.0%}",
            "within_1s": f"{(err < 1.0).mean():.0%}",
            "ref_bars_missed": missed,
        }
        print("   ", {k: v for k, v in stats.items() if k not in summary and k != "label"})
        report.append(stats)
        with open(OUT / f"phase2_{label}.bars.csv", "w", newline="") as fh:
            w = csv.writer(fh)
            w.writerow(["omr_bar", "audio_s", "nearest_ref_measure", "err_s"])
            for k, (tt, i, e) in enumerate(zip(t, idx, err)):
                w.writerow([k + 1, round(tt, 3), ref[i]["measure"], round(e, 3)])

    with open(OUT / "phase2_summary.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(report[0]))
        w.writeheader()
        w.writerows(report)
    print(f"\nWrote {OUT / 'phase2_summary.csv'}")


if __name__ == "__main__":
    main()
