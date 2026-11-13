"""Score, clean, placement and timing against the experiment (licensed score, local audio).

Uses outputs already built under work_root; skipped where they are absent. Rebuild with:
  libretto-pipeline score/clean/place/time figaro ...
"""
import json

import numpy as np
import pytest

from libretto_pipeline import clean, score_time
from libretto_pipeline.config import OPERAS, load_opera

OPERA = load_opera("figaro")
EXP = OPERA.score_pdf.parents[1] / "sextet-experiment" / "out"
need = lambda p: pytest.mark.skipif(not p.exists(), reason=f"{p.name} not present")


@need(OPERA.work / "decode" / "pieces" / "no16.score.json")
@need(EXP / "finale2ba.clean.json")
def test_score_and_clean_reproduce_the_experiment_finale():
    dec = json.loads((OPERA.work / "decode" / "pieces" / "no16.score.json").read_text())
    sc = score_time.score_time(dec, OPERA.cast)
    old = json.loads((EXP / "finale2ba.score.json").read_text())
    assert sc["notes"] == old["notes"] and sc["sections"] == old["sections"]
    # The experiment's own text (the repo's no16 text has since taken Bärenreiter's
    # spellings), so this tests that the method is unchanged.
    import importlib.util
    spec = importlib.util.spec_from_file_location("finale2_text", OPERAS.parent / "pipeline" / "experiment" / "finale2_text.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    text = {c.replace("DON BARTOLO", "BARTOLO"): v for c, v in mod.TEXT.items()}
    new, _ = clean.clean_piece(sc["lines"], text, [n for n, _ in OPERA.cast.members])
    ren = lambda l: json.loads(json.dumps({k: v for k, v in l.items() if k not in ("voice", "bar_start", "bar_end")},
                                          ensure_ascii=False).replace("DON BARTOLO", "BARTOLO"))
    key = lambda l: (l["score_start"], l["character"], l["text"])
    exp = json.loads((EXP / "finale2ba.clean.json").read_text())
    assert sorted(map(ren, exp), key=key) == sorted(map(ren, new), key=key)


@need(OPERA.work / "placement" / "georg-solti-1981.json")
@need(OPERA.work / "placement" / "currentzis-2014.json")
def test_placement_agrees_with_track_titles():
    for rec, finale_track in (("georg-solti-1981", "d10t04"), ("currentzis-2014", "t36")):
        d = json.loads((OPERA.work / "placement" / f"{rec}.json").read_text())
        nums = [p for p in d["pieces"] if p["kind"] != "secco"]
        assert len(nums) == 34 and sum(p["titles"] is False for p in nums) <= 1, rec
        fin = next(p for p in nums if p["id"] == "no16")
        assert fin["track_start"][0] == finale_track and fin["track_start"][1] < 5, rec


@need(OPERA.work / "timed" / "georg-solti-1981" / "no16.json")
@need(EXP / "finale2ba.lines.solti.json")
def test_solti_finale_timing_matches_the_experiment():
    new = json.loads((OPERA.work / "timed" / "georg-solti-1981" / "no16.json").read_text())
    # Lines are matched to the experiment's nearest line of the same character in score time:
    # No. 16's text has since been respelled as the score prints it ("dagl'occhi", "Uh che
    # testa!"), so some texts differ, and two lines start up to 0.5 s earlier in the score.
    old = [(l["character"].replace("DON BARTOLO", "BARTOLO"), l["score_start"], l["start"])
           for l in json.loads((EXP / "finale2ba.lines.solti.json").read_text())]
    d = []
    for l in new["lines"]:
        gap, start = min((abs(s - l["score_start"]), t) for c, s, t in old if c == l["character"])
        assert gap < 1.5, (l["character"], l["text"], l["score_start"])
        # The experiment's times start at the finale's first track; the timed file's at the
        # first track the edge search loads (the track before).
        d.append(l["start"] - new["offsets"][new["tracks"].index("d10t04")] - start)
    # The start and end are now refined from the audio, not hand-picked as in the
    # experiment; 2 of 823 lines move by 1-2 s (one of them a respelled line).
    a = np.abs(d)
    assert len(d) == 823 and np.median(a) < 0.05 and np.sum(a > 1.0) <= 3 and np.max(a) < 2.0
