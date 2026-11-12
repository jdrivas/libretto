"""Phase 7: snap the score-only alignment to vocal anchors heard in the recording.

Score-only (no MIDI) pipeline:
  1. Align the typeset scan (OMR v1 notes) to the recording -> base map, score s -> audio s.
  2. From the score, list phrase starts the audio can confirm:
       - texture entries: someone starts after all voices were silent >= 0.75 s
       - handovers: one voice stops and another starts at once, with a big register jump
  3. Find each in the separated vocal stem near where the base map predicts it:
       - texture entry -> nearest start of detected singing (vocal-stem loudness)
       - handover      -> pitch-jump in the vocal stem matching the score's jump
  4. Drop anchors that disagree with their neighbours; correct the base map by
     interpolating the anchor offsets (fading out beyond the first/last anchor).
  5. Re-time the OCR'd sung-text lines (OMR v2) through the corrected map and
     write the roon-rd test interchange file + timeline overlay (out/timeline/snapped.js).

Reports a leave-one-out error for the anchors (each predicted from the others)
and, for reference only, bar-time agreement with the MIDI-based alignment.
"""
import csv
import json
from pathlib import Path

import music21
import numpy as np

from phase2_compare import SEC_PER_QUARTER, load_omr
from phase4_align import OUT, align, load_audio
from phase5_lines import INTERCHANGE, assign_characters, lines_for, syllables, vocal_parts
from phase6_timeline import CAST, OMR_V1, analyse_stems, any_voice_spans, entry_offsets, frame_agreement, \
    omr_vocal_notes, phrases

HERE = Path(__file__).parent
OMR_V2 = HERE / "work" / "omr_typeset_v2"
STEM_CACHE = HERE / "work" / "stems_analysis.npz"
SEARCH_S = 1.5          # look for a detected event this far either side of the prediction
HANDOVER_MIN_JUMP = 7   # semitones; smaller jumps aren't reliably visible in the pitch track
OUTLIER_S = 1.0         # anchor offset may differ from its neighbours' median by at most this
FADE_S = 8.0            # beyond the first/last anchor, the correction fades to 0 over this (score s)
# Leave-one-out on the Sextet (2026-11-06): scan-only median 0.25 s / 100% within 1 s at the anchors,
# snapped 0.44 s / 92%. The base alignment is already as good as the anchor detection, so snapping
# adds noise. Lines are timed with the base (scan-only) map unless this is switched on.
USE_SNAPPED_FOR_LINES = False


def stems():
    if STEM_CACHE.exists():
        z = np.load(STEM_CACHE)
        return z["t"], z["lv"], z["la"], z["f0"], z["vprob"], [tuple(s) for s in z["spans"]]
    t, lv, la, f0, vprob, spans = analyse_stems()
    np.savez(STEM_CACHE, t=t, lv=lv, la=la, f0=f0, vprob=vprob, spans=np.array(spans))
    return t, lv, la, f0, vprob, spans


def candidates(notes):
    """Phrase starts (score s) that the audio should be able to confirm."""
    out = []
    for c in CAST:
        for s, _ in phrases(notes[c]):
            others = [n for c2 in CAST if c2 != c for n in notes[c2]]
            sounding = [n for n in others if n[0] < s - 0.05 and n[1] > s + 0.05]
            recent = [n for n in others if s - 0.75 < n[1] <= s + 0.05 and n[0] < s - 0.05]
            first = min((n for n in notes[c] if abs(n[0] - s) < 1e-6), key=lambda n: n[2])
            if sounding:
                continue
            if not recent:
                out.append({"score_t": s, "kind": "entry", "char": c})
            else:
                prev = max(recent, key=lambda n: n[1])
                jump = first[2] - prev[2]
                if abs(jump) >= HANDOVER_MIN_JUMP:
                    out.append({"score_t": s, "kind": "handover", "char": c, "jump": jump})
    return sorted(out, key=lambda a: a["score_t"])


def find_handover(t, f0_midi, p, jump):
    """Frame time near p where the vocal pitch jumps like the score says it should."""
    sel = np.where((t >= p - SEARCH_S) & (t <= p + SEARCH_S))[0]
    w = 12  # 0.24 s either side
    best = None
    for i in sel:
        before = f0_midi[max(0, i - w):i]
        after = f0_midi[i:i + w]
        before, after = before[~np.isnan(before)], after[~np.isnan(after)]
        if len(before) < w // 2 or len(after) < w // 2:
            continue
        d = np.median(after) - np.median(before)
        if np.sign(d) != np.sign(jump) or abs(d) < 0.6 * abs(jump):
            continue
        score = abs(d - jump) + 2.0 * abs(t[i] - p)  # semitones off + distance from prediction
        if best is None or score < best[0]:
            best = (score, t[i])
    return None if best is None else best[1]


def correction(anchors, exclude=None):
    """Offset function (score s -> audio-s correction) from anchors (score_t, offset)."""
    a = [x for i, x in enumerate(anchors) if i != exclude]
    st = np.array([x[0] for x in a])
    d = np.array([x[1] for x in a])

    def f(s):
        s = np.asarray(s, float)
        out = np.interp(s, st, d)
        out = np.where(s < st[0], d[0] * np.clip(1 - (st[0] - s) / FADE_S, 0, 1), out)
        out = np.where(s > st[-1], d[-1] * np.clip(1 - (s - st[-1]) / FADE_S, 0, 1), out)
        return out
    return f


def snapped_map(base, corr, s_max):
    grid = np.arange(0, s_max + 2, 0.05)
    a = np.array([base(s) for s in grid]) + corr(grid)
    a = np.maximum.accumulate(a)  # keep it monotonic
    return lambda s: float(np.interp(s, grid, a))


def v2_to_v1(v2_bars, v2_end, v1_bars, v1_end):
    """Map OMR v2 score seconds to v1 score seconds bar by bar (same 140 bars, some lengths differ)."""
    v2e, v1e = v2_bars + [v2_end], v1_bars + [v1_end]

    def f(s):
        i = max(0, min(len(v2_bars) - 1, np.searchsorted(v2_bars, s, side="right") - 1))
        frac = (s - v2e[i]) / (v2e[i + 1] - v2e[i])
        return v1e[i] + frac * (v1e[i + 1] - v1e[i])
    return f


def main():
    x, tuning = load_audio()
    df, v1_bars, _, _ = load_omr(OMR_V1, {3: -7})
    base = align(x, tuning, df, "scan")
    score_notes = omr_vocal_notes(lambda s: s)  # vocal notes in score seconds
    s_max = max(e for c in CAST for _, e, _ in score_notes[c])

    print("  vocal stem analysis (cached after first run)...")
    t, lv, la, f0, vprob, det = stems()
    f0_midi = 69 + 12 * np.log2(f0 / 440.0)
    det_starts = np.array([s for s, _ in det])

    # --- find anchors ---
    cands = candidates(score_notes)
    anchors, report = [], []
    for c in cands:
        p = base(c["score_t"])
        if c["kind"] == "entry":
            j = np.abs(det_starts - p).argmin()
            hit = det_starts[j] if abs(det_starts[j] - p) <= SEARCH_S else None
        else:
            hit = find_handover(t, f0_midi, p, c["jump"])
        report.append({**c, "pred": p, "hit": hit})
        if hit is not None:
            anchors.append([c["score_t"], hit - p, c])
    # outlier rejection: compare each offset with the median of its neighbours
    keep = []
    for i, (s, d, c) in enumerate(anchors):
        nb = [anchors[j][1] for j in range(max(0, i - 2), min(len(anchors), i + 3)) if j != i]
        if nb and abs(d - np.median(nb)) > OUTLIER_S:
            c["rejected"] = True
            continue
        keep.append((s, d))
    # monotonic: audio times of kept anchors must increase
    mono = []
    for s, d in keep:
        if not mono or base(s) + d > base(mono[-1][0]) + mono[-1][1]:
            mono.append((s, d))
    anchors = mono
    n_entry = sum(1 for r in report if r["kind"] == "entry")
    print(f"\n{len(cands)} confirmable phrase starts in the score ({n_entry} entries after silence, "
          f"{len(cands) - n_entry} register handovers); {sum(r['hit'] is not None for r in report)} found in "
          f"the vocal stem; {len(anchors)} kept as anchors")
    for r in report:
        tag = "—" if r["hit"] is None else ("rejected" if r.get("rejected") else f"{r['hit'] - r['pred']:+.2f}s")
        print(f"   {r['pred']:6.1f}s  {r['kind']:8} {r['char']:<11} {tag}")

    # --- leave-one-out: predict each anchor from the others ---
    loo_base, loo_snap = [], []
    for i, (s, d) in enumerate(anchors):
        corr = correction(anchors, exclude=i)
        loo_base.append(abs(d))
        loo_snap.append(abs(d - float(corr(s))))
    lb, ls = np.array(loo_base), np.array(loo_snap)
    print(f"\nLeave-one-out at the anchors (each anchor predicted without using it):")
    print(f"   scan-only alignment : median {np.median(lb):.2f}s, within 0.5s {(lb <= .5).mean():.0%}, "
          f"within 1s {(lb <= 1).mean():.0%}, max {lb.max():.2f}s")
    print(f"   snapped             : median {np.median(ls):.2f}s, within 0.5s {(ls <= .5).mean():.0%}, "
          f"within 1s {(ls <= 1).mean():.0%}, max {ls.max():.2f}s")

    corr = correction(anchors)
    snap = snapped_map(base, corr, s_max)

    # --- reference only: agreement with MIDI-based bar times ---
    ref = np.array([float(r["audio_s"]) for r in csv.DictReader(open(OUT / "sextet.bars.aligned.csv"))])
    for name, m in (("scan-only", base), ("snapped", snap)):
        e = np.array([m(b) for b in v1_bars]) - ref
        blocks = " ".join(f"{np.abs(e[a:a + 20]).mean():.2f}" for a in range(0, 140, 20))
        print(f"   vs MIDI-based bars, {name:9}: median {np.median(np.abs(e)):.2f}s, within 1s "
              f"{(np.abs(e) < 1).mean():.0%}; mean |diff| per 20 bars: {blocks}")

    # --- re-time the OCR'd lines (v2) through v1 bars -> snapped map ---
    score2 = music21.converter.parse(str(next(OMR_V2.glob("*.mxl")))).stripTies()
    v2_bars = [float(m.offset) * SEC_PER_QUARTER for m in score2.parts[0].getElementsByClass("Measure")]
    to_v1 = v2_to_v1(v2_bars, float(score2.highestTime) * SEC_PER_QUARTER,
                     v1_bars, s_max + 0.5)
    parts = vocal_parts(score2)
    who = assign_characters(score2, parts)
    sylls = syllables(score2, parts, who)
    line_map = snap if USE_SNAPPED_FOR_LINES else base
    print(f"\nTiming lines with the {'snapped' if USE_SNAPPED_FOR_LINES else 'scan-only (OMR v1)'} map")
    timed = []
    for char in CAST:
        for ln in lines_for([s for s in sylls if s["char"] == char]):
            # end = when the line's last note stops; voice = score (staff) order, for the
            # viewer's ensemble panel (rows high-to-low like the score).
            # score_start/score_end: position in the score (OMR v1 score seconds, nominal
            # 120 bpm), recording-independent; phase9 re-times other recordings from them.
            s0 = to_v1(ln["start_off"] * SEC_PER_QUARTER)
            s1 = to_v1(ln["end_off"] * SEC_PER_QUARTER)
            # sylls: the line's OCR'd syllables with the same times, so phase8 can time phrases.
            sylls_out = []
            for w in ln["words"]:
                for sy in w["sylls"]:
                    w0, w1 = to_v1(sy["off"] * SEC_PER_QUARTER), to_v1(sy["end"] * SEC_PER_QUARTER)
                    sylls_out.append({"text": sy["text"], "start": round(line_map(w0), 2),
                                      "end": round(line_map(w1), 2),
                                      "score_start": round(float(w0), 3), "score_end": round(float(w1), 3)})
            timed.append({"start": round(line_map(s0), 2), "end": round(line_map(s1), 2),
                          "score_start": round(float(s0), 3), "score_end": round(float(s1), 3),
                          "character": char, "voice": CAST.index(char),
                          "text": ln["text"], "translation": "", "sylls": sylls_out})
    timed.sort(key=lambda s: s["start"])
    (OUT / "sextet.lines.snapped.json").write_text(json.dumps(timed, indent=1, ensure_ascii=False))
    ic = json.loads(INTERCHANGE.read_text())
    track = next(tr for tr in ic["tracks"] if "Sestetto" in tr["title"] or "Riconosci" in tr["title"])
    track["segments"] = timed
    (OUT / "librettos" / INTERCHANGE.name).write_text(json.dumps(ic, indent=1, ensure_ascii=False))
    print(f"\n{len(timed)} lines re-timed -> {OUT / 'librettos' / INTERCHANGE.name}")

    # --- overlay for the timeline page ---
    notes = {c: [[round(snap(s), 3), round(snap(e), 3), p] for s, e, p in score_notes[c]] for c in CAST}
    spans = any_voice_spans(notes)
    t0, t1 = ref[0], ref[-1] + 2.0
    det_in = [s for s in det if s[1] >= t0 - 1 and s[0] <= t1]
    offs = [d for _, d in entry_offsets(spans, det_in) if d is not None]
    summary = {"entries": len(spans), "matched": len(offs),
               "median_abs": round(float(np.median(np.abs(offs))), 2),
               "within_0_5": round(float(np.mean(np.abs(offs) <= 0.5)), 2),
               "within_1": round(float(np.mean(np.abs(offs) <= 1.0)), 2),
               "frame_agreement": round(frame_agreement(t, spans, det_in, t0, t1), 3)}
    overlay = {"notes": notes, "bars": [round(snap(b), 3) for b in v1_bars], "summary": summary,
               "anchors": [[round(snap(s), 3), r_kind] for s, r_kind in
                           ((s, next(r["kind"] for r in report if r["score_t"] == s)) for s, _ in anchors)]}
    (OUT / "timeline" / "snapped.js").write_text("window.SNAPPED = " + json.dumps(overlay, separators=(",", ":")) + ";\n")
    print(f"Wrote {OUT / 'timeline' / 'snapped.js'}  (entries check is partly circular: anchors were fitted to them)")


if __name__ == "__main__":
    main()
