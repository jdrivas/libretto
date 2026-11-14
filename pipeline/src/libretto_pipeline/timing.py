"""The `time` stage: cleaned lines in score time -> lines in a recording's time, with checks.

Route B (numbers): the experiment's finale2_build.py `rec`: two-pass alignment of the
piece's notes to the joined tracks, lines mapped through it, kept out of clear silences in
the vocal stem, and checked against it.
Route C (secco recitative): the experiment's phase10_recit.py: MMS_FA forced alignment of
the score's text on the vocal stem, a wildcard between speeches.

Times here are on the timeline of the tracks joined in order (`tracks`, `offsets`);
publish splits them back into tracks.
"""
import copy
import json
import logging
import os
import re
import unicodedata

import numpy as np
import pandas as pd

from . import align, voice
from .config import tool_env

log = logging.getLogger(__name__)


def _paths(opera, rec_id, tracks):
    base = opera.work / "recordings" / rec_id
    return ([base / "mix" / f"{t}.flac" for t in tracks],
            [base / "stems" / t / "vocals.flac" for t in tracks],
            [base / "stems" / t / "no_vocals.flac" for t in tracks])


def map_lines(lines, to_audio, score=None):
    from .score_time import bar_of
    timed = []
    for ln in lines:
        seg = copy.deepcopy(ln)
        seg["start"], seg["end"] = round(to_audio(ln["score_start"]), 2), round(to_audio(ln["score_end"]), 2)
        if score is not None:
            seg["bar_start"], seg["bar_end"] = bar_of(score, ln["score_start"]), bar_of(score, ln["score_end"])
        seg["gloss"] = [{**g, "start": round(to_audio(g["score_start"]), 2), "end": round(to_audio(g["score_end"]), 2)}
                        for g in ln["gloss"]]
        timed.append(seg)
    return timed


EDGE_S = 20.0        # score seconds at each end used to refine a placed boundary
EDGE_SEARCH_S = (30.0, 70.0)  # search from this much before to this much after (or mirrored)


def refine_edges(x, tuning, notes, start, end):
    """Refine a placed span's start and end (seconds in x): the piece's opening EDGE_S of
    score located near the start, its closing EDGE_S near the end. An opening or closing
    is far more distinctive against the recitative around it than a whole number is."""
    df = notes.sort_values("start").reset_index(drop=True)
    end_s = float((df["start"] + df["duration"]).max())
    dur = len(x) / align.FS
    out = [start, end]
    for k, (part, w0, w1) in enumerate((
            (df[df["start"] < EDGE_S], start - EDGE_SEARCH_S[0], start + EDGE_SEARCH_S[1]),
            (df[df["start"] + df["duration"] > end_s - EDGE_S], end - EDGE_SEARCH_S[1], end + EDGE_SEARCH_S[0]))):
        w0, w1 = max(0.0, w0), min(dur, w1)
        if len(part) < 3 or w1 - w0 < 2 * EDGE_S:
            continue
        part = part.copy()
        part["start"] -= part["start"].min()
        xw = x[int(w0 * align.FS):int(w1 * align.FS)]
        f0, f1, _, _ = align.locate_excerpt(align.features_audio(xw, tuning)[0], align.features_score(part)[0],
                                            shifts=align.SHIFTS)
        out[k] = w0 + (f0 if k == 0 else f1)
    if out[1] - out[0] < 0.5 * (end - start):  # implausible: keep the placement
        return start, end
    return out[0], out[1]


def time_number(opera, rec_id, pid, tracks, excerpt=None, window=None):
    """Route B for one piece over the given consecutive tracks (times on their joined
    timeline).
    excerpt: (t0, t1) where the piece starts and ends, taken as given.
    window: (w0, w1) the placed span; its start and end are refined (refine_edges), as
      placement is only accurate to seconds (24 s for Solti's Sextet, after a recitative),
      and a fixed start that early squeezes the opening into the music before it.
    Neither: from the first to the last loud moment."""
    score = json.loads((opera.work / "score" / f"{pid}.json").read_text())
    lines = json.loads((opera.work / "clean" / f"{pid}.json").read_text())["lines"]
    mix, voc, acc = _paths(opera, rec_id, tracks)
    x, tuning, offsets = align.load_mix(mix)
    placed = None
    if window is not None:
        placed = window
        excerpt = refine_edges(x, tuning, pd.DataFrame(score["notes"]), *window)
    if excerpt is None:
        import librosa
        rms = librosa.feature.rms(y=x, hop_length=align.FS // 10)[0]
        loud = np.where(20 * np.log10(rms / rms.max() + 1e-9) > -30)[0]
        excerpt = (loud[0] / 10.0, loud[-1] / 10.0)
    df = pd.DataFrame(score["notes"]).sort_values("start").reset_index(drop=True)
    end_s = float((df["start"] + df["duration"]).max())
    bounds = [s["score_s"] for s in score["sections"]] + [end_s]
    to_audio, factor = align.two_pass(x, tuning, df, bounds, excerpt, f"{rec_id} {pid}")
    timed = map_lines(lines, to_audio, score)
    t, lv, la, f0, vprob, det = voice.analyse(voc, acc, pitch=False)
    trimmed = voice.trim_to_silences(timed, voice.silences(t, lv))
    timed.sort(key=lambda s: s["start"])
    notes = {}
    for ln in lines:
        for g in ln["gloss"]:
            notes.setdefault(ln["character"], []).append((to_audio(g["score_start"]), to_audio(g["score_end"])))
    chk = voice.checks(t, det, lv, timed, notes, excerpt[0], excerpt[1])
    chk |= {"route": "B", "tuning_cents": float(tuning), "excerpt": [round(e, 2) for e in excerpt],
            "placed": [round(e, 2) for e in placed] if placed else None,
            "section_tempo_factors": [round(float(f), 2) for f in factor],
            "trimmed_ends": sum(k == "end" for _, k, _, _ in trimmed),
            "trimmed_starts": sum(k == "start" for _, k, _, _ in trimmed)}
    return {"piece": pid, "recording": rec_id, "tracks": tracks, "offsets": [round(o, 3) for o in offsets],
            "lines": timed, "checks": chk}


# ---------- route C ----------

SR = 16000
CHUNK_S, OVERLAP_S = 30.0, 2.0
LINE_END = re.compile(r"[.;:?!]$|\.\.\.$|…$")
MALE = {"IL CONTE", "FIGARO", "BARTOLO", "BASILIO", "DON CURZIO", "ANTONIO"}


def _norm(word, vocab):
    w = unicodedata.normalize("NFD", word.lower().replace("’", "'"))
    w = "".join(ch for ch in w if unicodedata.category(ch) != "Mn")
    return "".join(ch for ch in w if ch in vocab)


def _emissions(model, wav):
    import torch
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
    return torch.cat(parts)[None], ratio / SR


def time_recitatives(opera, rec_id, pieces, tracks, region):
    """Route C for one or more consecutive secco pieces sharing a region of the joined
    tracks. pieces: [(piece id, speeches)] in order; speeches as in the text files.
    Returns {piece id: result}."""
    os.environ.update(tool_env())
    import librosa
    import torch
    import torchaudio

    order = [n for n, _ in opera.cast.members]
    bundle = torchaudio.pipelines.MMS_FA
    model = bundle.get_model(with_star=True)
    vocab = set(bundle.get_dict(star="*")) - {"-", "*"}
    tokenizer, aligner = bundle.get_tokenizer(), bundle.get_aligner()
    _, voc_paths, acc_paths = _paths(opera, rec_id, tracks)
    voc = np.concatenate([librosa.load(str(p), sr=SR, mono=True)[0] for p in voc_paths])
    offsets, t = [], 0.0
    for p in voc_paths:
        offsets.append(t)
        t += librosa.get_duration(path=str(p))
    r0, r1 = region
    seg = voc[int(r0 * SR):int(r1 * SR)]
    wav = torch.from_numpy(seg / (np.abs(seg).max() + 1e-9)).float()[None]
    em, spf = _emissions(model, wav)

    sps = [(pid, k, sp) for pid, speeches in pieces for k, sp in enumerate(speeches)]
    words, owner = ["*"], [None]
    for si, (_, _, sp) in enumerate(sps):
        for ci, (it, _) in enumerate(sp["phrases"]):
            for tok in it.split():
                w = _norm(tok, vocab)
                if w:
                    words.append(w)
                    owner.append((si, ci, tok))
        words.append("*")
        owner.append(None)
    spans = aligner(em[0], tokenizer(words))
    times = []
    for w, sp, o in zip(words, spans, owner):
        if o is not None:
            times.append({"si": o[0], "ci": o[1], "tok": o[2], "start": round(r0 + sp[0].start * spf, 2),
                          "end": round(r0 + sp[-1].end * spf, 2), "score": float(np.mean([s.score for s in sp]))})

    t_, lv, la, f0, vprob, det = voice.analyse(voc_paths, acc_paths)
    heard = np.zeros(len(t_), bool)
    for s0, s1 in det:
        heard[(t_ >= s0) & (t_ <= s1)] = True
    from .score_time import bar_of
    out = {}
    for pid, _ in pieces:
        # Bars: where each phrase is in the score, from the piece's cleaned score lines
        # (clean numbers a character's speeches 0, 1, ... in order, as the text lists them).
        where, score = {}, None
        clean_path = opera.work / "clean" / f"{pid}.json"
        if clean_path.exists():
            score = json.loads((opera.work / "score" / f"{pid}.json").read_text())
            for ln in json.loads(clean_path.read_text())["lines"]:
                for g in ln["gloss"]:
                    where.setdefault((g["v"], g["c"]), (g["score_start"], g["score_end"]))
        rank = {}
        segs, inside, regs = [], [], []
        for si, (p, k, sp) in enumerate(sps):
            if p != pid:
                continue
            wt = [w for w in times if w["si"] == si]
            for w in wt:
                m = (t_ >= w["start"]) & (t_ <= w["end"])
                inside.append(heard[m].mean() if m.any() else 0.0)
            groups, cur = [], []
            for ci, (it, en) in enumerate(sp["phrases"]):
                cur.append(ci)
                if LINE_END.search(it.strip()) or ci == len(sp["phrases"]) - 1:
                    groups.append(cur)
                    cur = []
            for char in sp["characters"]:
                vid = f"{char}:{pid}.{k}"
                vi = rank[char] = rank.get(char, -1) + 1
                for g in groups:
                    gloss = []
                    for ci in g:
                        cw = [w for w in wt if w["ci"] == ci]
                        if cw:
                            gloss.append({"v": vid, "c": ci, "it": sp["phrases"][ci][0], "en": sp["phrases"][ci][1],
                                          "start": cw[0]["start"], "end": cw[-1]["end"]})
                    if gloss:
                        seg = {"start": gloss[0]["start"], "end": gloss[-1]["end"], "character": char,
                               "voice": order.index(char) if char in order else len(order),
                               "text": " ".join(x["it"] for x in gloss),
                               "translation": " ".join(x["en"] for x in gloss), "gloss": gloss,
                               "verses": {vid: [list(c) for c in sp["phrases"]]}, "source": "recit-forced-align"}
                        sc_t = [where[(f"{char}:{vi}", x["c"])] for x in gloss if (f"{char}:{vi}", x["c"]) in where]
                        if sc_t and score:
                            seg["bar_start"] = bar_of(score, min(a for a, _ in sc_t))
                            seg["bar_end"] = bar_of(score, max(b for _, b in sc_t))
                        segs.append(seg)
            if wt:
                s0, s1 = wt[0]["start"], wt[-1]["end"]
                m = (t_ >= s0) & (t_ <= s1) & np.isfinite(f0)
                hz = float(np.nanmedian(f0[m])) if m.sum() >= 10 and s1 - s0 >= 1.0 else float("nan")
                kind = ("men" if all(c in MALE for c in sp["characters"])
                        else "women" if not any(c in MALE for c in sp["characters"]) else "mixed")
                regs.append((kind, hz))
        women = np.nanmedian([h for k, h in regs if k == "women"] or [np.nan])
        men_low = [h < 0.75 * women for k, h in regs if k == "men" and np.isfinite(h)]
        segs.sort(key=lambda s: (s["start"], s["voice"]))
        inside = np.array(inside) if inside else np.zeros(0)
        out[pid] = {"piece": pid, "recording": rec_id, "tracks": tracks, "offsets": [round(o, 3) for o in offsets],
                    "lines": segs,
                    "checks": {"route": "C", "region": [round(r0, 2), round(r1, 2)],
                               "words": int(len(inside)),
                               "words_mostly_silent": int((inside < 0.5).sum()),
                               "median_word_heard": round(float(np.median(inside)), 3) if len(inside) else None,
                               "men_speeches_low": f"{sum(men_low)}/{len(men_low)}"}}
    return out
