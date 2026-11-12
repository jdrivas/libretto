"""Score-to-audio alignment (route B): chroma + onset features, subsequence DTW, MrMsDTW.

From the experiment's phase4_align.py (features, locate, align) and finale2_build.py
(the two-pass tempo rescaling), unchanged in method; paths and globals removed.
"""
import logging

import librosa
import numpy as np
from synctoolbox.dtw.mrmsdtw import sync_via_mrmsdtw
from synctoolbox.dtw.utils import (compute_optimal_chroma_shift, make_path_strictly_monotonic,
                                   shift_chroma_vectors)
from synctoolbox.feature.chroma import pitch_to_chroma, quantize_chroma, quantized_chroma_to_CENS
from synctoolbox.feature.csv_tools import df_to_pitch_features, df_to_pitch_onset_features
from synctoolbox.feature.dlnco import pitch_onset_features_to_DLNCO
from synctoolbox.feature.pitch import audio_to_pitch_features
from synctoolbox.feature.pitch_onset import audio_to_pitch_onset_features
from synctoolbox.feature.utils import estimate_tuning

log = logging.getLogger(__name__)

FS = 22050   # audio sample rate for alignment (the mix FLACs are written at this rate)
RATE = 50    # feature rate, Hz


def load_mix(paths):
    """Mono mix of consecutive tracks joined into one timeline, its tuning, and each
    track's start offset (s)."""
    parts, offsets, t = [], [], 0.0
    for p in paths:
        x, _ = librosa.load(str(p), sr=FS, mono=True)
        parts.append(x)
        offsets.append(t)
        t += len(x) / FS
    x = np.concatenate(parts)
    return x, estimate_tuning(x, FS), offsets


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


def locate_excerpt(audio_chroma, score_chroma):
    """Subsequence DTW on 5 Hz CENS: where does the score sit inside the audio?
    Returns (start s, end s, chroma shift, mean path cost)."""
    a, rate = quantized_chroma_to_CENS(audio_chroma, 41, 10, RATE)
    s, _ = quantized_chroma_to_CENS(score_chroma, 41, 10, RATE)
    shift = compute_optimal_chroma_shift(a, s)
    s = shift_chroma_vectors(s, shift)
    cost = 1.0 - s.T @ a  # (score frames, audio frames); CENS is L2-normalised
    # Slope-constrained steps (tempo ratio 0.5x-2x) so a sparse score can't collapse onto
    # a short stretch of audio.
    D, wp = librosa.sequence.dtw(C=cost, subseq=True,
                                 step_sizes_sigma=np.array([[1, 1], [2, 1], [1, 2]]),
                                 weights_mul=np.array([1.0, 2.0, 2.0]))
    wp = wp[::-1]
    path_cost = float(np.mean(cost[np.clip(wp[:, 0], 0, cost.shape[0] - 1), np.clip(wp[:, 1], 0, cost.shape[1] - 1)]))
    return wp[0, 1] / rate, wp[-1, 1] / rate, shift, path_cost


def align(x, tuning, df, label="score", excerpt=None):
    """Align a score note DataFrame to audio x. Returns a score-s -> audio-s function.
    excerpt=(t0, t1) fixes where the score starts and ends in x (the coarse search still
    decides the chroma shift)."""
    dur = len(x) / FS
    a_chroma, _ = features_audio(x, tuning)
    s_chroma, s_dlnco = features_score(df)
    t0, t1, shift, _ = locate_excerpt(a_chroma, s_chroma)
    if excerpt is not None:
        t0, t1 = excerpt
    log.debug(f"[{label}] {'given' if excerpt else 'coarse match'}: {t0:.1f}-{t1:.1f} s (chroma shift {shift})")
    m0, m1 = max(0.0, t0 - 1.0), min(dur, t1 + 1.0)
    xe = x[int(m0 * FS):int(m1 * FS)]
    a_chroma, a_dlnco = features_audio(xe, tuning)
    wp = sync_via_mrmsdtw(f_chroma1=a_chroma, f_onset1=a_dlnco,
                          f_chroma2=shift_chroma_vectors(s_chroma, shift),
                          f_onset2=shift_chroma_vectors(s_dlnco, shift),
                          input_feature_rate=RATE, step_weights=np.array([1.5, 1.5, 2.0]),
                          threshold_rec=10 ** 6, verbose=False)
    wp = make_path_strictly_monotonic(wp)
    audio_t, score_t = wp[0] / RATE + m0, wp[1] / RATE
    return lambda t: float(np.interp(t, score_t, audio_t))


def two_pass(x, tuning, df, bounds_s, excerpt, label):
    """Align, take each section's tempo from the first pass, rescale the score's sections
    to it, and align again (the score's nominal tempos can be far off, e.g. a slow 3/8
    section against a 4/4 Allegro). bounds_s: section starts in score s, plus the end.
    Returns (score-s -> audio-s, the section tempo factors)."""
    first = align(x, tuning, df, f"{label} pass 1", excerpt=excerpt)
    sb = np.array(bounds_s, float)
    ab = np.array([first(s) for s in sb])
    factor = np.clip(np.diff(ab) / np.maximum(np.diff(sb), 1e-6), 0.5, 4.0)
    norm_b = np.concatenate([[0.0], np.cumsum(np.diff(sb) * factor)])
    to_norm = lambda s: np.interp(s, sb, norm_b)
    k_of = lambda s: np.clip(np.searchsorted(sb, s, side="right") - 1, 0, len(factor) - 1)
    df2 = df.drop(columns=["end"], errors="ignore").copy()  # synctoolbox adds "end" in place
    df2["duration"] = np.maximum(df["duration"] * factor[k_of(df["start"].to_numpy())], 0.1)
    df2["start"] = to_norm(df["start"].to_numpy())
    second = align(x, tuning, df2, f"{label} pass 2", excerpt=excerpt)
    return (lambda s: second(float(to_norm(s)))), factor
