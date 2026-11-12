"""Phase 4: align the Solti recording to the reference MIDI and derive barline times.

Steps
  1. Coarse subsequence DTW (CENS chroma, 5 Hz) to find where the Sextet sits in
     the track. The track continues into the recitative "Eccovi, o caro amico",
     which the score does not cover.
  2. Fine alignment of that audio excerpt with synctoolbox MrMsDTW
     (quantized chroma + DLNCO onset features, 50 Hz).
  3. Map each score barline/beat through the warping path to audio time.
  4. Write out/sextet.bars.aligned.csv, a tempo-curve plot, and a click-track
     mix (out/sextet_clicks.flac) for listening checks.
"""
import csv
import json
import os
from pathlib import Path

os.environ.setdefault("MPLCONFIGDIR", str(Path(os.environ.get("TMPDIR", "/tmp")) / "mpl"))

import librosa
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import pandas as pd
import pretty_midi
import soundfile as sf
from synctoolbox.dtw.mrmsdtw import sync_via_mrmsdtw
from synctoolbox.dtw.utils import (compute_optimal_chroma_shift, make_path_strictly_monotonic,
                                   shift_chroma_vectors)
from synctoolbox.feature.chroma import pitch_to_chroma, quantize_chroma, quantized_chroma_to_CENS
from synctoolbox.feature.csv_tools import df_to_pitch_features, df_to_pitch_onset_features
from synctoolbox.feature.dlnco import pitch_onset_features_to_DLNCO
from synctoolbox.feature.pitch import audio_to_pitch_features
from synctoolbox.feature.pitch_onset import audio_to_pitch_onset_features
from synctoolbox.feature.utils import estimate_tuning

HERE = Path(__file__).parent
OUT = HERE / "out"
MIDI = HERE / "source" / "WIMA.f0ac-ricon.mid"
AUDIO = next((HERE.parent.parent / "roon-rd" / "Audio").glob("*Sextet*.flac"))
FS = 22050
RATE = 50  # feature rate, Hz
BEATS_PER_BAR = 4  # whole Sextet is 4/4 (Phase 1)


def score_dataframe(pm):
    rows = []
    for inst in pm.instruments:
        for n in inst.notes:
            rows.append({"start": n.start, "duration": n.end - n.start, "pitch": n.pitch,
                         "velocity": n.velocity, "instrument": inst.name})
    return pd.DataFrame(rows).sort_values("start").reset_index(drop=True)


def features_audio(x, tuning):
    f_pitch = audio_to_pitch_features(f_audio=x, Fs=FS, tuning_offset=tuning, feature_rate=RATE)
    chroma = quantize_chroma(pitch_to_chroma(f_pitch))
    onsets = audio_to_pitch_onset_features(f_audio=x, Fs=FS, tuning_offset=tuning)
    dlnco = pitch_onset_features_to_DLNCO(f_peaks=onsets, feature_rate=RATE,
                                          feature_sequence_length=chroma.shape[1])
    return chroma, dlnco


def features_score(df):
    f_pitch = df_to_pitch_features(df, feature_rate=RATE)
    chroma = quantize_chroma(pitch_to_chroma(f_pitch))
    onsets = df_to_pitch_onset_features(df)
    dlnco = pitch_onset_features_to_DLNCO(f_peaks=onsets, feature_rate=RATE,
                                          feature_sequence_length=chroma.shape[1])
    return chroma, dlnco


def cens(chroma_q, win, down):
    f, rate = quantized_chroma_to_CENS(chroma_q, win, down, RATE)
    return f, rate


def locate_excerpt(audio_chroma, score_chroma):
    """Subsequence DTW on 5 Hz CENS: where does the score sit inside the track?"""
    a, rate = cens(audio_chroma, 41, 10)
    s, _ = cens(score_chroma, 41, 10)
    shift = compute_optimal_chroma_shift(a, s)
    s = shift_chroma_vectors(s, shift)
    cost = 1.0 - s.T @ a  # (score frames, audio frames); CENS is L2-normalised
    # Slope-constrained steps (tempo ratio 0.5x-2x) so a sparse score can't
    # collapse onto a short stretch of audio.
    _, wp = librosa.sequence.dtw(C=cost, subseq=True,
                                 step_sizes_sigma=np.array([[1, 1], [2, 1], [1, 2]]),
                                 weights_mul=np.array([1.0, 2.0, 2.0]))
    wp = wp[::-1]
    return wp[0, 1] / rate, wp[-1, 1] / rate, shift


def load_audio():
    x, _ = librosa.load(str(AUDIO), sr=FS, mono=True)
    return x, estimate_tuning(x, FS)


def align(x, tuning, df, label="score", excerpt=None):
    """Align a score note DataFrame to audio x. Returns a score-seconds -> audio-seconds function.

    excerpt=(t0, t1) overrides where the score is taken to start and end in the track
    (the coarse search still decides the chroma shift)."""
    dur = len(x) / FS
    # 1. Locate the score within the track.
    a_chroma, _ = features_audio(x, tuning)
    s_chroma, s_dlnco = features_score(df)
    t0, t1, shift = locate_excerpt(a_chroma, s_chroma)
    if excerpt is not None:
        t0, t1 = excerpt
    print(f"  [{label}] {'given' if excerpt else 'coarse match'}: {t0:.1f}s - {t1:.1f}s in track (chroma shift {shift})")

    # 2. Fine alignment on the excerpt (small margin either side).
    m0 = max(0.0, t0 - 1.0)
    m1 = min(dur, t1 + 1.0)
    xe = x[int(m0 * FS):int(m1 * FS)]
    a_chroma, a_dlnco = features_audio(xe, tuning)
    wp = sync_via_mrmsdtw(f_chroma1=a_chroma, f_onset1=a_dlnco,
                          f_chroma2=shift_chroma_vectors(s_chroma, shift),
                          f_onset2=shift_chroma_vectors(s_dlnco, shift),
                          input_feature_rate=RATE, step_weights=np.array([1.5, 1.5, 2.0]),
                          threshold_rec=10 ** 6, verbose=False)
    wp = make_path_strictly_monotonic(wp)
    audio_t = wp[0] / RATE + m0
    score_t = wp[1] / RATE
    return lambda t: float(np.interp(t, score_t, audio_t))


def main():
    OUT.mkdir(exist_ok=True)
    print(f"Audio: {AUDIO.name[:70]}...")
    x, tuning = load_audio()
    print(f"  duration {len(x) / FS:.1f}s, tuning offset {tuning} cents")

    pm = pretty_midi.PrettyMIDI(str(MIDI))
    df = score_dataframe(pm)
    bars = json.loads((OUT / "sextet.bars.json").read_text())
    to_audio = align(x, tuning, df, "MIDI")

    # 3. Barlines and beats.
    score_end = pm.get_end_time()
    rows = []
    for k, b in enumerate(bars):
        s0 = b["start_s"]
        s1 = bars[k + 1]["start_s"] if k + 1 < len(bars) else score_end
        a0, a1 = to_audio(s0), to_audio(s1)
        perf_bpm = BEATS_PER_BAR * 60.0 / (a1 - a0) if a1 > a0 else float("nan")
        rows.append({"measure": b["measure"], "score_s": round(s0, 3), "audio_s": round(a0, 3),
                     "score_bpm": b["tempo_bpm"], "performed_bpm": round(perf_bpm, 1),
                     "singing": " ".join(b["singing"])})
    with open(OUT / "sextet.bars.aligned.csv", "w", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]))
        w.writeheader()
        w.writerows(rows)
    end_audio = to_audio(score_end)
    print(f"  bar 1 at {rows[0]['audio_s']:.2f}s, bar 140 at {rows[-1]['audio_s']:.2f}s, "
          f"Sextet ends at {end_audio:.2f}s")

    bpm = np.array([r["performed_bpm"] for r in rows])
    print(f"  performed tempo: median {np.nanmedian(bpm):.1f} bpm, "
          f"range {np.nanmin(bpm):.1f}-{np.nanmax(bpm):.1f}")
    odd = [r for r in rows if abs(r["performed_bpm"] / np.nanmedian(bpm) - 1) > 0.35]
    print(f"  bars with tempo >35% off median: {[r['measure'] for r in odd]}")

    # Tempo-curve plot.
    fig, ax = plt.subplots(figsize=(12, 4))
    ax.plot([r["measure"] for r in rows], bpm, label="Solti (aligned)")
    ax.step([r["measure"] for r in rows], [r["score_bpm"] for r in rows], where="post",
            label="MIDI notated", alpha=0.6)
    ax.set_xlabel("measure")
    ax.set_ylabel("bpm (quarter)")
    ax.set_title("Sextet: performed tempo per bar")
    ax.legend()
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(OUT / "sextet.tempo.png", dpi=110)

    # 4. Click-track mix at full quality: high click on downbeats, low on other beats.
    full, sr = sf.read(str(AUDIO), always_2d=True)
    clicks = np.zeros(len(full))

    def add_click(t, freq, amp):
        n = int(0.03 * sr)
        tt = np.arange(n) / sr
        tone = amp * np.sin(2 * np.pi * freq * tt) * np.exp(-tt * 120)
        i = int(t * sr)
        if 0 <= i < len(clicks) - n:
            clicks[i:i + n] += tone

    for k, b in enumerate(bars):
        s0 = b["start_s"]
        s1 = bars[k + 1]["start_s"] if k + 1 < len(bars) else score_end
        for beat in range(BEATS_PER_BAR):
            t = to_audio(s0 + (s1 - s0) * beat / BEATS_PER_BAR)
            add_click(t, 1760 if beat == 0 else 880, 0.5 if beat == 0 else 0.25)
    mix = 0.7 * full + clicks[:, None]
    mix /= max(1.0, np.abs(mix).max())
    sf.write(str(OUT / "sextet_clicks.flac"), mix, sr)
    print(f"Wrote {OUT / 'sextet.bars.aligned.csv'}, sextet.tempo.png, sextet_clicks.flac")


if __name__ == "__main__":
    main()
