"""Phase 9: time the prepared sung text for another recording, with no manual work.

Recording-independent inputs (built once per work, from the Solti run):
  out/sextet.lines.clean.json   lines with text, gloss, verses and score_start/score_end
  work/omr_typeset              the scanned score's notes (OMR v1)

Per recording, automatically:
  1. Align the score's notes to the recording (same alignment as for Solti).
  2. Re-time every line: start/end = alignment(score_start/score_end).
  3. Objective check, independent of Solti: separate the vocals (Demucs) and compare
     where the re-timed score says someone sings with where singing is heard.
  4. Write an interchange file for the album so roon-rd matches the track.

Usage: phase9_recording.py <audio.flac> <label> --album "..." --artist "..." --title "..."
           [--conductor ...] [--orchestra ...] [--year N] [--label ...] [--track N] [--disc N]
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path

import librosa
import numpy as np

import phase4_align as P4
import phase6_timeline as P6
from phase2_compare import load_omr
from phase4_align import FS, OUT, align
from phase6_timeline import CAST, OMR_V1, any_voice_spans, entry_offsets, frame_agreement, trim_to_silences

HERE = Path(__file__).parent
SOLTI_IC = OUT / "librettos" / "georg-solti-1981.interchange.json"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("audio")
    ap.add_argument("label")
    ap.add_argument("--album", required=True)
    ap.add_argument("--artist", required=True)
    ap.add_argument("--title", required=True)
    ap.add_argument("--conductor")
    ap.add_argument("--orchestra")
    ap.add_argument("--year", type=int)
    ap.add_argument("--record-label")
    ap.add_argument("--track", type=int)
    ap.add_argument("--disc", type=int)
    a = ap.parse_args()
    audio = Path(a.audio)

    x, _ = librosa.load(str(audio), sr=FS, mono=True)
    tuning = P4.estimate_tuning(x, FS)
    print(f"{audio.name}: {len(x) / FS:.1f} s, tuning offset {tuning:+.0f} cents vs A=440")
    df, v1_bars, _, _ = load_omr(OMR_V1, {3: -7})

    # Separated vocals: used to choose between alignments and to check the result.
    stems = HERE / "work" / f"demucs_{a.label}"
    wav = HERE / "work" / f"{a.label}.wav"
    vocals = stems / "htdemucs" / wav.stem / "vocals.wav"
    if not vocals.exists():
        subprocess.run(["ffmpeg", "-loglevel", "error", "-y", "-i", str(audio), "-ac", "2", "-ar", "44100",
                        str(wav)], check=True)
        subprocess.run([sys.executable, "-m", "demucs", "--two-stems=vocals", "-n", "htdemucs",
                        "-o", str(stems), str(wav)], check=True)
    P6.STEMS = vocals.parent
    t, lv, la, f0, vprob, det = P6.analyse_stems()

    # 1. Align the score. The coarse search can misjudge where the number ends (e.g. a
    #    much broader finale), so also try "to where the music ends" and "the whole
    #    track". Two checks decide (tested on Solti and Currentzis, 2026-11-07):
    #    - harmonic fit (score chroma vs the audio it is mapped to) vetoes placements
    #      that spread the score over other music, e.g. a recitative after the number;
    #    - among the rest, singing agreement with the vocal stem decides, and an
    #      alternative must beat the coarse placement clearly.
    rms = librosa.feature.rms(y=x, hop_length=FS // 10)[0]
    loud = np.where(20 * np.log10(rms / rms.max() + 1e-9) > -30)[0]
    m0, m1 = loud[0] / 10.0, loud[-1] / 10.0
    a_chroma = P4.features_audio(x, tuning)[0]
    s_chroma = P4.features_score(df)[0]
    c0, c1, _ = P4.locate_excerpt(a_chroma, s_chroma)
    A, rate = P4.cens(a_chroma, 41, 10)
    S, _ = P4.cens(s_chroma, 41, 10)
    s_times = np.arange(S.shape[1]) / rate

    def harmonic_fit(f):
        aj = np.clip(np.round(np.array([f(s) for s in s_times]) * rate).astype(int), 0, A.shape[1] - 1)
        return float(np.sum(S * A[:, aj], axis=0).mean())

    det_all = [s for s in det if s[1] >= m0 and s[0] <= m1]
    results, tried = [], []
    for name, (e0, e1) in {"coarse search": (c0, c1), "to music end": (c0, m1), "whole track": (m0, m1)}.items():
        if any(abs(e0 - p0) < 1 and abs(e1 - p1) < 1 for p0, p1 in tried):
            continue  # same placement as one already tried
        tried.append((e0, e1))
        f = align(x, tuning, df, f"{a.label}: {name}", excerpt=(e0, e1))
        fit = harmonic_fit(f)
        sing = frame_agreement(t, any_voice_spans(P6.omr_vocal_notes(f)), det_all, m0, m1)
        print(f"    -> harmonic fit {fit:.3f}, singing agreement {sing:.1%}")
        results.append((name, f, fit, sing))
    base = results[0]
    ok = [r for r in results[1:] if r[2] >= base[2] - 0.002 and r[3] > base[3] + 0.01]
    chosen, to_audio, _, _ = max(ok, key=lambda r: r[3]) if ok else base
    print(f"  chose: {chosen}")
    bar_t = [to_audio(b) for b in v1_bars]
    print(f"  bar 1 at {bar_t[0]:.1f} s, bar {len(bar_t)} at {bar_t[-1]:.1f} s "
          f"(Sextet takes {bar_t[-1] - bar_t[0]:.0f} s; Solti 282 s)")

    # 2. Re-time the prepared lines.
    lines = json.loads((OUT / "sextet.lines.clean.json").read_text())
    timed = []
    for ln in lines:
        seg = dict(ln)
        seg["start"] = round(to_audio(ln["score_start"]), 2)
        seg["end"] = round(to_audio(ln["score_end"]), 2)
        seg["gloss"] = [{**g, "start": round(to_audio(g["score_start"]), 2), "end": round(to_audio(g["score_end"]), 2)}
                        for g in ln.get("gloss", [])]
        timed.append(seg)
    trimmed = trim_to_silences(timed, P6.silences(t, lv))
    timed.sort(key=lambda s: s["start"])
    print(f"  kept lines out of clear silences in the vocal stem: {sum(k == 'end' for _, k, _, _ in trimmed)} ends "
          f"cut, {sum(k == 'start' for _, k, _, _ in trimmed)} starts moved")
    for s, k, old, new in trimmed:
        print(f"     {k:5} {old:6.1f} -> {new:6.1f}  {s['character']:11} {s['text'][:40]}")
    (OUT / f"sextet.lines.{a.label}.json").write_text(json.dumps(timed, indent=1, ensure_ascii=False))

    # 3. Check against the singing heard in this recording (vocal stem loudness).
    sc_notes = P6.omr_vocal_notes(to_audio)
    t0, t1 = bar_t[0], bar_t[-1] + 2.0
    det_in = [s for s in det if s[1] >= t0 - 1 and s[0] <= t1]
    spans = any_voice_spans(sc_notes)
    offs = [d for _, d in entry_offsets(spans, det_in) if d is not None]
    agree = frame_agreement(t, spans, det_in, t0, t1)
    print(f"  check vs vocal stem: singing/silence agreement {agree:.0%} (Solti scan-only 84%); "
          f"entries after silence: {len(offs)}/{len(spans)} matched, median |offset| "
          f"{np.median(np.abs(offs)):.2f} s, within 1 s {np.mean(np.abs(offs) <= 1):.0%}")

    # 4. Interchange file for this album (one track).
    solti = json.loads(SOLTI_IC.read_text())
    rec = {"album_title": a.album, "artist": a.artist}
    for k, v in (("conductor", a.conductor), ("orchestra", a.orchestra), ("year", a.year), ("label", a.record_label)):
        if v:
            rec[k] = v
    ic = {"version": solti["version"], "opera": solti["opera"], "recording": rec,
          "tracks": [{"track_id": f"d{a.disc or 1}t{a.track or 1}", "title": a.title,
                      "disc_number": a.disc or 1, "track_number": a.track or 1,
                      "duration_seconds": round(len(x) / FS, 1), "act": "3", "segments": timed}]}
    dest = OUT / "librettos" / f"{a.label}.interchange.json"
    dest.write_text(json.dumps(ic, indent=1, ensure_ascii=False))
    print(f"  {len(timed)} lines, first at {timed[0]['start']:.1f} s, last at {timed[-1]['start']:.1f} s -> {dest}")


if __name__ == "__main__":
    main()
