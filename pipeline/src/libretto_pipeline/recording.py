"""Recordings: inventory (`add-recording`) and voice separation (`separate`).

The audio library is read only: everything is written under
<work>/<opera>/recordings/<id>/:
  recording.json            the recording, and its tracks in order (disc, track, title, act,
                            duration, source file, audio format)
  mix/<track>.flac          the full mix, mono 22.05 kHz: what the alignment listens to
  stems/<track>/vocals.flac, no_vocals.flac
                            Demucs htdemucs, two stems, 44.1 kHz stereo (as in the experiment)
A track id is d<disc>t<track> ("d10t04"), or t<track> when the files carry no disc number.
"""
import json
import logging
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

from .config import tool_env

log = logging.getLogger(__name__)

_ACT_EN = {"I": 1, "II": 2, "III": 3, "IV": 4}
_ACT_IT = {"primo": 1, "secondo": 2, "terzo": 3, "quarto": 4}


def act_of(title):
    """Act from a track title ("Act II", "Atto Secondo"); 0 for the overture or sinfonia."""
    if m := re.search(r"\bAct (IV|I{1,3})\b", title):
        return _ACT_EN[m.group(1)]
    if m := re.search(r"\bAtto (Primo|Secondo|Terzo|Quarto)\b", title, re.I):
        return _ACT_IT[m.group(1).lower()]
    if re.search(r"Overture|Sinfonia", title, re.I):
        return 0
    return None


def _probe(path):
    out = subprocess.run(["ffprobe", "-v", "error", "-show_entries",
                          "format=duration:format_tags:stream=sample_rate,bits_per_raw_sample,channels",
                          "-of", "json", str(path)], capture_output=True, text=True, check=True)
    d = json.loads(out.stdout)
    tags = {k.lower(): v for k, v in d["format"].get("tags", {}).items()}
    st = d.get("streams", [{}])[0]
    return tags, float(d["format"]["duration"]), st


def inventory(opera, rec):
    """Read the recording's files (tags only) and write recording.json."""
    files = []
    for src in rec["sources"]:
        src = Path(src)
        if not src.is_dir():
            raise SystemExit(f"{rec['id']}: source folder not found: {src}")
        files += [f for f in sorted(src.glob("*.flac")) if rec.get("match", "") in f.name]
    tracks, last_act = [], 0
    for f in files:
        tags, duration, st = _probe(f)
        disc = int(tags["disc"].split("/")[0]) if tags.get("disc") else None
        track = int(tags["track"].split("/")[0])
        title = tags.get("title", f.stem)
        act = act_of(title)
        if act is None:  # a track without an act in its title belongs to the act before
            act = last_act
        last_act = act
        tracks.append({"id": f"d{disc:02d}t{track:02d}" if disc else f"t{track:02d}",
                       "disc": disc, "track": track, "title": title, "act": act,
                       "duration_s": round(duration, 3), "file": str(f),
                       "sample_rate": int(st.get("sample_rate", 0)),
                       "bits": int(st.get("bits_per_raw_sample") or 0)})
    tracks.sort(key=lambda t: (t["disc"] or 0, t["track"]))
    ids = [t["id"] for t in tracks]
    if len(set(ids)) != len(ids):
        raise SystemExit(f"{rec['id']}: duplicate track ids")
    out = opera.work / "recordings" / rec["id"]
    out.mkdir(parents=True, exist_ok=True)
    meta = {k: rec[k] for k in ("id", "album_title", "artist", "conductor", "orchestra", "year",
                                "label", "store") if k in rec}
    acts = {}
    for t in tracks:
        acts[t["act"]] = acts.get(t["act"], 0) + t["duration_s"]
    data = meta | {"tracks": tracks,
                   "total_s": round(sum(t["duration_s"] for t in tracks), 3),
                   "acts": {str(a): round(s, 1) for a, s in sorted(acts.items())}}
    (out / "recording.json").write_text(json.dumps(data, ensure_ascii=False, indent=1))
    log.info(f"{rec['id']}: {len(tracks)} tracks, {data['total_s'] / 60:.1f} min, "
             f"acts {', '.join(f'{a}: {s / 60:.1f} min' for a, s in sorted(acts.items()))}")
    return data


def load(opera, rec_id):
    path = opera.work / "recordings" / rec_id / "recording.json"
    if not path.exists():
        raise SystemExit(f"{rec_id}: run `add-recording {opera.id} {rec_id}` first")
    return json.loads(path.read_text())


def _ffmpeg(*args):
    subprocess.run(["ffmpeg", "-loglevel", "error", "-nostdin", "-y", *map(str, args)], check=True)


def separate(opera, rec_id, device="cpu"):
    """Mix (mono 22.05 kHz FLAC) and Demucs vocal/accompaniment stems (FLAC) for every
    track. Resumable: tracks already done are skipped."""
    data = load(opera, rec_id)
    base = opera.work / "recordings" / rec_id
    tmp = base / "tmp"
    todo = [t for t in data["tracks"] if not (base / "stems" / t["id"] / "no_vocals.flac").exists()]
    total_s = sum(t["duration_s"] for t in todo)
    log.info(f"{rec_id}: {len(data['tracks']) - len(todo)} tracks already separated, "
             f"{len(todo)} to do ({total_s / 60:.1f} min of audio)")
    t0, done_s = time.time(), 0.0
    for i, t in enumerate(todo, 1):
        tid = t["id"]
        (base / "mix").mkdir(parents=True, exist_ok=True)
        (base / "stems" / tid).mkdir(parents=True, exist_ok=True)
        tmp.mkdir(exist_ok=True)
        _ffmpeg("-i", t["file"], "-ac", "1", "-ar", "22050", "-sample_fmt", "s16", base / "mix" / f"{tid}.flac")
        wav = tmp / f"{tid}.wav"
        _ffmpeg("-i", t["file"], "-ac", "2", "-ar", "44100", "-sample_fmt", "s16", wav)
        res = subprocess.run([sys.executable, "-m", "demucs", "--two-stems=vocals", "-n", "htdemucs",
                              "-d", device, "-o", str(tmp / "demucs"), str(wav)],
                             capture_output=True, text=True, env=tool_env())
        if res.returncode != 0:
            log.error(f"{tid}: demucs failed:\n{res.stderr[-2000:]}")
            raise SystemExit(1)
        out = tmp / "demucs" / "htdemucs" / tid
        for stem in ("vocals", "no_vocals"):
            _ffmpeg("-i", out / f"{stem}.wav", "-c:a", "flac", base / "stems" / tid / f"{stem}.flac")
        shutil.rmtree(out)
        wav.unlink()
        done_s += t["duration_s"]
        rate = done_s / (time.time() - t0)
        log.info(f"{rec_id} {tid} ({i}/{len(todo)}): {t['duration_s']:.0f} s of audio; "
                 f"{rate:.1f}x real time, about {(total_s - done_s) / rate / 60:.0f} min left")
    if tmp.exists():
        shutil.rmtree(tmp)
    log.info(f"{rec_id}: all {len(data['tracks'])} tracks separated")


def status(opera, rec_id):
    base = opera.work / "recordings" / rec_id
    if not (base / "recording.json").exists():
        return "not added"
    data = load(opera, rec_id)
    n = len(data["tracks"])
    done = sum((base / "stems" / t["id"] / "no_vocals.flac").exists() for t in data["tracks"])
    return f"{n} tracks, {data['total_s'] / 60:.0f} min; separated {done}/{n}"
