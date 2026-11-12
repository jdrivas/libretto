"""Act II finale: clean text (score time), then time it for a recording and write its tracks.

  finale2_build.py clean [--ba]          out/finale2.score.json -> out/finale2.clean.json
  finale2_build.py rec <label> [--ba]    align, re-time, trim, check, write the interchange tracks

--ba: use the Bärenreiter vector score (ba_finale2_score.py -> out/finale2ba.*) instead of
the Ricordi scan read by Audiveris (finale2_score.py -> out/finale2.*).

The finale spans several tracks (Solti 3, Currentzis 7). Each recording's tracks are joined
into one timeline (wav and Demucs stems, in ~/Development/libretto-work), the whole score is
aligned to it, and the lines are split back into tracks by start time.
"""
import copy
import json
import sys
from pathlib import Path

import librosa
import numpy as np

import phase4_align as P4
import phase6_timeline as P6
import phase8_text as P8
from finale2_score import CAST, load_score, notes_df
from finale2_text import TEXT
from phase2_compare import SEC_PER_QUARTER
from phase4_align import FS, OUT, align

P8.TEXT = TEXT
BA = "--ba" in sys.argv
PREFIX = "finale2ba" if BA else "finale2"
WORK = Path.home() / "Development" / "libretto-work"
LIB = OUT / "librettos"
TITLE = "Le nozze di Figaro, K. 492: Atto Secondo: "
RECORDINGS = {
    "solti": {"interchange": "georg-solti-1981.interchange.json", "stems": "solti-finale2",
              # the Solti file's own tracks for these three files (same lengths)
              "tracks": ["d2t4", "d2t5", "d2t6"]},
    "currentzis": {"interchange": "currentzis-2014.interchange.json", "stems": "currentzis-finale2",
                   "tracks": [(36, "Esci omai garzon malnato (No. 16, Finale: Il Conte, La Contessa)"),
                              (37, "Signore, cos'è quel stupore? (Susanna, Il Conte, La Contessa)"),
                              (38, "Susanna, son morta: il fiato mi manca (La Contessa, Susanna, Il Conte)"),
                              (39, "Signori, di fuori son gia i suonatori (Figaro, Il Conte, La Contessa, Susanna)"),
                              (40, "Conoscete, signor Figaro (Il Conte, Figaro, La Contessa, Susanna)"),
                              (41, "Ah signor... signor... Cosa è stato ... Vostre dunque saran... "
                                   "(Antonio, Il Conte, La Contessa, Susanna, Figaro)"),
                              (42, "Voi signor, che giusto siete ... Son confusa, son stordita (Marcellina, "
                                   "Basilio, Bartolo, Il Conte, La Contessa, Figaro, Susanna)")]},
}


SPLIT_GAP_S = 2.0    # score seconds (a beat is 0.5)
PHRASE_MAX_S = 4.0   # two bars of 4/4


def split_at_gaps(ln):
    """A cleaned line can hold phrases far apart in the score (words the OCR lost in
    between, or a dropped line's time absorbed), so it would stay lit for 20 s and overlap
    everyone else's entries. Split it where consecutive phrases are SPLIT_GAP_S apart, and
    let each part span only its own phrases.

    A phrase the OCR lost and phase 8 filled in from the reference spans the whole gap
    between its neighbours (up to 30 s here), so phrases are first capped at
    PHRASE_MAX_S, starting where they start (right after the words before them)."""
    for g in ln["gloss"]:
        g["score_end"] = min(g["score_end"], g["score_start"] + PHRASE_MAX_S)
        g["end"] = min(g["end"], g["start"] + PHRASE_MAX_S)
    groups = [[ln["gloss"][0]]]
    for g in ln["gloss"][1:]:
        if g["score_start"] - groups[-1][-1]["score_end"] >= SPLIT_GAP_S:
            groups.append([g])
        else:
            groups[-1].append(g)
    out = []
    for gl in groups:
        part = copy.deepcopy(ln)
        part["gloss"] = gl
        part["text"] = " ".join(g["it"] for g in gl)
        part["translation"] = " ".join(g["en"] for g in gl)
        part["verses"] = {v: c for v, c in ln["verses"].items() if any(g["v"] == v for g in gl)}
        for a, b in (("start", "end"), ("score_start", "score_end")):
            part[a], part[b] = gl[0][a], max(g[b] for g in gl)
        out.append(part)
    return out


def clean():
    d = json.loads((OUT / f"{PREFIX}.score.json").read_text())
    cleaned, missing, total = [], [], 0
    for char in CAST:
        mine = sorted((l for l in d["lines"] if l["character"] == char), key=lambda l: l["start"])
        out, q = P8.clean_character(char, mine)
        seen = {(g["v"], g["c"]) for l in out for g in l["gloss"]}
        for vi, verse in enumerate(TEXT[char]):
            total += 1
            if not any((f"{char}:{vi}", ci) in seen for ci in range(len(verse))):
                missing.append((char, " ".join(it for it, _ in verse)))
        print(f"  {char:12} {len(mine):4} score lines -> {len(out):4}; {q:.0%} of OCR letters match the text")
        cleaned += out
    before = len(cleaned)
    cleaned = [part for ln in cleaned for part in split_at_gaps(ln)]
    cleaned.sort(key=lambda l: l["start"])
    print(f"{before} lines, {len(cleaned)} after splitting at gaps of {SPLIT_GAP_S} score s or more; "
          f"speeches never found in the score text: {len(missing)}/{total}")
    for c, t in missing:
        print(f"     {c:12} {t[:70]}")
    (OUT / f"{PREFIX}.clean.json").write_text(json.dumps(cleaned, indent=1, ensure_ascii=False))


def rec(label):
    cfg = RECORDINGS[label]
    stems = WORK / "stems" / "htdemucs" / cfg["stems"]
    info = json.loads((stems / "tracks.json").read_text())
    x, _ = librosa.load(str(WORK / "wav" / f"{cfg['stems']}.wav"), sr=FS, mono=True)
    tuning = P4.estimate_tuning(x, FS)
    print(f"{label}: {len(x) / FS:.1f} s in {len(info['tracks'])} tracks, tuning {tuning:+.0f} cents")
    if BA:
        import pandas as pd
        sc = json.loads((OUT / f"{PREFIX}.score.json").read_text())
        df = pd.DataFrame(sc["notes"]).sort_values("start").reset_index(drop=True)
        end_s = float((df["start"] + df["duration"]).max())
        bounds_s = [s["score_s"] for s in sc["sections"]] + [end_s]
    else:
        score, grid = load_score()
        df = notes_df(score)
        bars_ = sorted(grid.values())
        bounds_s = list(np.array([bars_[0][0]] + [b[0] for a, b in zip(bars_, bars_[1:]) if a[1] != b[1]]
                                 + [bars_[-1][0] + bars_[-1][1]]) * SEC_PER_QUARTER)
    # The score covers the whole joined timeline: from the first to the last loud moment.
    rms = librosa.feature.rms(y=x, hop_length=FS // 10)[0]
    loud = np.where(20 * np.log10(rms / rms.max() + 1e-9) > -30)[0]
    excerpt = (loud[0] / 10.0, loud[-1] / 10.0)
    first = align(x, tuning, df, f"{label} pass 1", excerpt=excerpt)
    # Pass 2: the score has no tempo (every bar at a nominal 120 bpm), so a slow 3/8
    # section is ~3x too short against a 4/4 Allegro and the alignment squeezes it. Take
    # each section's tempo (a run of bars in one metre) from pass 1, rescale the score's
    # sections to it, and align again.
    sb = np.array(bounds_s)
    ab = np.array([first(s) for s in sb])
    # Bounded, so a section pass 1 squeezed to nothing still gets some room.
    factor = np.clip(np.diff(ab) / np.diff(sb), 0.5, 4.0)
    print("  section tempo factors (audio s per nominal score s): "
          + " ".join(f"{f:.2f}" for f in factor))
    norm_b = np.concatenate([[0.0], np.cumsum(np.diff(sb) * factor)])  # sections at their pass-1 tempo
    to_norm = lambda s: np.interp(s, sb, norm_b)
    k_of = lambda s: np.clip(np.searchsorted(sb, s, side="right") - 1, 0, len(factor) - 1)
    df2 = df.drop(columns=["end"], errors="ignore").copy()  # synctoolbox adds "end" in place
    df2["duration"] = np.maximum(df["duration"] * factor[k_of(df["start"].to_numpy())], 0.1)
    df2["start"] = to_norm(df["start"].to_numpy())
    second = align(x, tuning, df2, f"{label} pass 2", excerpt=excerpt)
    to_audio = lambda s: second(float(to_norm(s)))

    lines = json.loads((OUT / f"{PREFIX}.clean.json").read_text())
    timed = []
    for ln in lines:
        seg = dict(ln)
        seg["start"], seg["end"] = round(to_audio(ln["score_start"]), 2), round(to_audio(ln["score_end"]), 2)
        seg["gloss"] = [{**g, "start": round(to_audio(g["score_start"]), 2), "end": round(to_audio(g["score_end"]), 2)}
                        for g in ln["gloss"]]
        timed.append(seg)
    P6.STEMS = stems
    t, lv, la, f0, vprob, det = P6.analyse_stems()
    trimmed = P6.trim_to_silences(timed, P6.silences(t, lv))
    timed.sort(key=lambda s: s["start"])
    print(f"  kept out of clear silences: {sum(k == 'end' for _, k, _, _ in trimmed)} ends cut, "
          f"{sum(k == 'start' for _, k, _, _ in trimmed)} starts moved")

    # Checks against the vocal stem.
    notes = {c: [] for c in CAST}
    for ln in lines:
        for g in ln["gloss"]:
            notes[ln["character"]].append((to_audio(g["score_start"]), to_audio(g["score_end"]), 0))
    P6.CAST = CAST
    spans = P6.any_voice_spans(notes)
    offs = [d for _, d in P6.entry_offsets(spans, det) if d is not None]
    agree = P6.frame_agreement(t, spans, det, loud[0] / 10.0, loud[-1] / 10.0)
    heard = np.zeros(len(t), bool)
    for s0, s1 in det:
        heard[(t >= s0) & (t <= s1)] = True
    ins = [heard[(t >= g["start"]) & (t <= g["end"])].mean() for s in timed for g in s["gloss"]
           if g["end"] > g["start"]]
    print(f"  checks: singing/silence agreement {agree:.0%}; entries after silence {len(offs)}/{len(spans)} "
          f"matched, median |offset| {np.median(np.abs(offs)):.2f} s, within 1 s {np.mean(np.abs(offs) <= 1):.0%}; "
          f"phrases mostly in silence {sum(i < .5 for i in ins)}/{len(ins)}")
    # Bar times per track boundary, for a quick look at where the music splits.
    for k, (name, off) in enumerate(zip(info["tracks"], info["offsets"])):
        first = next((s for s in timed if s["start"] >= off), None)
        print(f"  track {name}: starts {off:7.1f} s; first line {first['start'] - off:6.1f} s in: "
              f"{first['character']} {first['text'][:40]}" if first else f"  track {name}: no lines")

    # Split into tracks (by start time), make times track-relative.
    offs_ = info["offsets"] + [info["total"]]
    per_track = [[] for _ in info["tracks"]]
    for s in timed:
        k = int(np.searchsorted(info["offsets"], s["start"], side="right") - 1)
        o, end = offs_[k], offs_[k + 1]
        s2 = copy.deepcopy(s)
        for key in ("start", "end"):
            s2[key] = round(min(s2[key], end) - o, 2)
        for g in s2["gloss"]:
            g["start"], g["end"] = round(min(g["start"], end) - o, 2), round(min(g["end"], end) - o, 2)
        s2.pop("ocr", None)
        per_track[k].append(s2)
    (OUT / f"{PREFIX}.lines.{label}.json").write_text(json.dumps(timed, indent=1, ensure_ascii=False))

    path = LIB / cfg["interchange"]
    ic = json.loads(path.read_text())
    if label == "solti":
        for tid, segs in zip(cfg["tracks"], per_track):
            tr = next(tr for tr in ic["tracks"] if tr["track_id"] == tid)
            tr["segments"] = segs
            print(f"  {tid}: {len(segs)} lines  ({tr['title'][:60]})")
    else:
        for (num, title), segs, k in zip(cfg["tracks"], per_track, range(len(per_track))):
            tid = f"d1t{num}"
            ic["tracks"] = [tr for tr in ic["tracks"] if tr["track_id"] != tid] + [{
                "track_id": tid, "title": TITLE + title, "disc_number": 1, "track_number": num,
                "duration_seconds": round(offs_[k + 1] - offs_[k], 1), "act": "2", "segments": segs}]
            print(f"  {tid}: {len(segs)} lines  ({title[:60]})")
        ic["tracks"].sort(key=lambda tr: (tr.get("disc_number", 1), tr.get("track_number", 0)))
    path.write_text(json.dumps(ic, indent=1, ensure_ascii=False))
    print(f"  -> {path}")


if __name__ == "__main__":
    if sys.argv[1] == "clean":
        clean()
    else:
        rec(sys.argv[2])  # sys.argv may also hold --ba
