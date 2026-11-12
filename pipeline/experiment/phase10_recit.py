"""Phase 10: time recitativo secco by aligning the known text to the vocal stem (no score).

Recitativo secco is close to speech (clear diction, speaking rhythm, one voice at a time),
so a speech forced aligner can place each word: torchaudio's MMS_FA (multilingual
wav2vec2 CTC model, covers Italian), run on the Demucs vocal stem. A wildcard token
between speeches absorbs orchestra, pauses and anything else not in the text.

Input:  the recording, its vocal stem, the region after the Sextet, recit_text.py.
Output: lines like the Sextet's (start/end, character, voice, text, translation, gloss
        with phrase times, verses), appended to the recording's interchange track.

Checks (independent of the aligner):
  - words should fall where singing is heard (vocal-stem loudness);
  - each speech's pitch register should fit its singer (Bartolo/Figaro low, the women
    and Cherubino high), which tests the speaker turns.

Usage: phase10_recit.py <label> <audio> <vocal stem dir> <region start s> [--into interchange.json]
"""
import argparse
import json
import os
import re
import unicodedata
from pathlib import Path

HERE = Path(__file__).parent
# The aligner model (~1.2 GB) goes to the standard torch cache (~/.cache/torch), not
# this Dropbox folder.

import librosa
import numpy as np
import torch
import torchaudio

import phase6_timeline as P6
from phase4_align import OUT
from act2_text import BRAVO, VOI_CHE_SAPETE
from recit_text import ANDIAM, ECCOVI, ENSEMBLE, VOICE

SR = 16000
CHUNK_S, OVERLAP_S = 30.0, 2.0
MALE = {"DON BARTOLO", "FIGARO", "IL CONTE", "DON CURZIO"}
LINE_END = re.compile(r"[.;:?!]$|\.\.\.$")


def norm(word, vocab):
    w = unicodedata.normalize("NFD", word.lower().replace("’", "'"))
    w = "".join(ch for ch in w if unicodedata.category(ch) != "Mn")
    return "".join(ch for ch in w if ch in vocab)


def emissions(model, wav):
    """CTC emissions for long audio, computed in overlapping chunks and stitched."""
    n, step, ov = wav.shape[1], int((CHUNK_S - OVERLAP_S) * SR), int(OVERLAP_S * SR)
    parts, ratio = [], None
    with torch.inference_mode():
        for a in range(0, n, step):
            seg = wav[:, a:a + int(CHUNK_S * SR)]
            em, _ = model(seg)
            ratio = ratio or seg.shape[1] / em.shape[1]
            drop = 0 if a == 0 else int(round(ov / 2 / ratio))
            keep = em.shape[1] if a + seg.shape[1] >= n else em.shape[1] - int(round(ov / 2 / ratio))
            parts.append(em[0, drop:keep])
            if a + seg.shape[1] >= n:
                break
    return torch.cat(parts)[None], ratio / SR  # seconds per frame


# Speech ids number all texts in this order: append new texts at the end so ids stay stable.
TEXTS = {"eccovi": ECCOVI, "andiam": ANDIAM, "voi": VOI_CHE_SAPETE, "bravo": BRAVO}


def speeches(names):
    """Flatten the chosen texts: [(characters, chunks, speech id)]. Speech ids number all
    texts in order, so they stay the same whichever texts a track holds."""
    out, k = [], 0
    for name, text in TEXTS.items():
        for sp in text:
            if name in names:
                out.append(([sp[0]], sp[1], k) if sp[0] != ENSEMBLE else (sp[1], sp[2], k))
            k += 1
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("label")
    ap.add_argument("audio")
    ap.add_argument("stem_dir")
    ap.add_argument("start", type=float, help="where the recitative region starts in the track (s)")
    ap.add_argument("--texts", default="eccovi,andiam", help="which recitatives the track holds")
    ap.add_argument("--into", help="interchange file to add the lines to")
    ap.add_argument("--replace-in", help="with --into: replace all segments of the track whose title "
                                         "contains this text (keeps its title and numbers)")
    ap.add_argument("--title", help="with --into: put the lines in their own track with this title "
                                    "(else they are appended to the Sextet's track)")
    ap.add_argument("--track", type=int)
    ap.add_argument("--disc", type=int, default=1)
    a = ap.parse_args()

    bundle = torchaudio.pipelines.MMS_FA
    model = bundle.get_model(with_star=True)
    vocab = set(bundle.get_dict(star="*")) - {"-", "*"}
    tokenizer, aligner = bundle.get_tokenizer(), bundle.get_aligner()

    voc, _ = librosa.load(str(Path(a.stem_dir) / "vocals.wav"), sr=SR, mono=True)
    region = voc[int(a.start * SR):]
    wav = torch.from_numpy(region / (np.abs(region).max() + 1e-9)).float()[None]
    print(f"{a.label}: recitative region {a.start:.1f}-{a.start + len(region) / SR:.1f} s")
    em, spf = emissions(model, wav)
    print(f"  emissions: {em.shape[1]} frames, {spf * 1000:.1f} ms each")

    # Transcript: words of every speech, a wildcard before, between and after speeches.
    sps = speeches(a.texts.split(","))
    words, owner = ["*"], [None]  # owner: (speech index, chunk index, word text) or None
    for si, (_, chunks, _) in enumerate(sps):
        for ci, (it, _) in enumerate(chunks):
            for tok in it.split():
                w = norm(tok, vocab)
                if w:
                    words.append(w)
                    owner.append((si, ci, tok))
        words.append("*")
        owner.append(None)
    spans = aligner(em[0], tokenizer(words))
    times = []
    for w, sp, o in zip(words, spans, owner):
        if o is None:
            continue
        t0 = a.start + sp[0].start * spf
        t1 = a.start + sp[-1].end * spf
        score = float(np.mean([s.score for s in sp]))
        times.append({"si": o[0], "ci": o[1], "tok": o[2], "start": round(t0, 2), "end": round(t1, 2), "score": score})

    # Lines: a speech is split at phrase-final punctuation; ensembles give one line per
    # character with the same times.
    segs = []
    for si, (chars, chunks, k) in enumerate(sps):
        wt = [w for w in times if w["si"] == si]
        groups, cur = [], []
        for ci, (it, en) in enumerate(chunks):
            cur.append(ci)
            if LINE_END.search(it.strip()) or ci == len(chunks) - 1:
                groups.append(cur)
                cur = []
        for char in chars:
            vid = f"{char}:recit{k}"
            for g in groups:
                gloss = []
                for ci in g:
                    cw = [w for w in wt if w["ci"] == ci]
                    gloss.append({"v": vid, "c": ci, "it": chunks[ci][0], "en": chunks[ci][1],
                                  "start": cw[0]["start"], "end": cw[-1]["end"]})
                segs.append({"start": gloss[0]["start"], "end": gloss[-1]["end"], "character": char,
                             "voice": VOICE[char], "text": " ".join(x["it"] for x in gloss),
                             "translation": " ".join(x["en"] for x in gloss), "gloss": gloss,
                             "verses": {vid: [list(c) for c in chunks]}, "source": "recit-forced-align"})
    segs.sort(key=lambda s: (s["start"], s["voice"]))

    # Checks.
    P6.STEMS = Path(a.stem_dir)
    t, lv, la, f0, vprob, det = P6.analyse_stems()
    heard = np.zeros(len(t), bool)
    for s0, s1 in det:
        heard[(t >= s0) & (t <= s1)] = True
    inside = []
    for w in times:
        m = (t >= w["start"]) & (t <= w["end"])
        inside.append(heard[m].mean() if m.any() else 0.0)
    inside = np.array(inside)
    print(f"\n  {len(times)} words aligned; share of each word's time where singing is heard: "
          f"median {np.median(inside):.0%}, words mostly in silence (<50%): {(inside < .5).sum()}")
    low = sorted(times, key=lambda w: w["score"])[:5]
    print("  least confident words:", ", ".join(f"{w['tok']}@{w['start']:.1f}" for w in low))
    # Register: a men's speech should sit well below the women's (relative, so pitch-
    # tracker octave habits and voice types don't matter). Speeches under 1 s are skipped.
    rows = []
    for si, (chars, chunks, k) in enumerate(sps):
        wt = [w for w in times if w["si"] == si]
        s0, s1 = wt[0]["start"], wt[-1]["end"]
        m = (t >= s0) & (t <= s1) & np.isfinite(f0)
        hz = float(np.nanmedian(f0[m])) if m.sum() >= 10 and s1 - s0 >= 1.0 else float("nan")
        kind = "men" if all(c in MALE for c in chars) else ("women" if not any(c in MALE for c in chars) else "mixed")
        rows.append((chars, chunks, s0, s1, hz, kind))
    women = np.nanmedian([r[4] for r in rows if r[5] == "women"] or [np.nan])
    print(f"\n  speech                       time           median f0   (women's median {women:.0f} Hz)")
    for chars, chunks, s0, s1, hz, kind in rows:
        flag = ""
        if np.isfinite(hz) and kind != "mixed":
            low = hz < 0.75 * women
            flag = "" if low == (kind == "men") else "   <-- register doesn't fit"
        who = "+".join(c.split()[-1][:5] for c in chars)
        print(f"  {who:<12} {chunks[0][0][:15]:<16} {s0:6.1f}-{s1:6.1f}  {hz:6.0f} Hz  {kind:>5}{flag}")

    out = OUT / f"recit.lines.{a.label}.json"
    out.write_text(json.dumps(segs, indent=1, ensure_ascii=False))
    print(f"\n  {len(segs)} lines -> {out}")
    if a.into:
        ic = json.loads(Path(a.into).read_text())
        if a.title:
            tid = f"d{a.disc}t{a.track}"
            track = {"track_id": tid, "title": a.title, "disc_number": a.disc, "track_number": a.track,
                     "duration_seconds": round(len(voc) / SR, 1), "act": "3", "segments": segs}
            ic["tracks"] = [tr for tr in ic["tracks"] if tr.get("track_id") != tid] + [track]
            ic["tracks"].sort(key=lambda tr: (tr.get("disc_number", 1), tr.get("track_number", 0)))
            print(f"  wrote track {tid} into {a.into}")
        elif a.replace_in:
            track = next(tr for tr in ic["tracks"] if a.replace_in in tr["title"])
            track["segments"] = segs
            print(f"  replaced the segments of {track['track_id']} ({track['title'][:60]}) in {a.into}")
        else:
            track = next(tr for tr in ic["tracks"] if "Riconosci" in tr["title"] or "Sestetto" in tr["title"])
            track["segments"] = [s for s in track["segments"] if s.get("source") != "recit-forced-align"] + segs
            print(f"  appended to the Sextet track in {a.into}")
        Path(a.into).write_text(json.dumps(ic, indent=1, ensure_ascii=False))


if __name__ == "__main__":
    main()
