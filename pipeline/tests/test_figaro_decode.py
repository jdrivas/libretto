"""Decode of the user's Bärenreiter Figaro (licensed, local only). Skipped without the PDF.

Pins what the experiment established (the five passages' bars and the finale's decode)
and what P2 checked by hand: every contents entry accounted for, choruses read.
"""
import json
import logging

import pytest

from libretto_pipeline import decode
from libretto_pipeline.config import load_opera

OPERA = load_opera("figaro")
pytestmark = pytest.mark.skipif(not OPERA.score_pdf.exists(), reason="licensed score not present")


@pytest.fixture(scope="module")
def manifest():
    logging.disable(logging.INFO)
    try:
        return decode.run(OPERA)
    finally:
        logging.disable(logging.NOTSET)


def piece(m, pid):
    return next(p for p in m["pieces"] if p["id"] == pid)


def test_contents_all_accounted_for(manifest):
    c = manifest["checks"]
    assert c["contents_not_found"] == [] and c["pieces_not_in_contents"] == [] and c["duplicate_ids"] == []


def test_experiment_passages(manifest):
    expect = {"no19": (1, 140, 342), "rec19a": (1, 19, 361), "rec19b": (1, 16, 364),
              "no12": (1, 79, 147), "rec12a": (1, 25, 152), "no16": (1, 939, 199)}
    for pid, (b0, b1, page) in expect.items():
        p = piece(manifest, pid)
        assert (p["bars"][0], p["bars"][1], p["pages"][0]) == (b0, b1, page), pid


def test_main_score_has_no_bar_problems(manifest):
    assert [p["id"] for p in manifest["pieces"] if p["problems"] and not p["appendix"]] == []


def test_upbeat_is_bar_zero(manifest):
    assert piece(manifest, "no13")["bars"][0] == 0 and piece(manifest, "no13")["pickup"]


def test_choruses_and_ensembles_have_words(manifest):
    for pid in ("no08", "no09", "no22"):
        assert piece(manifest, pid)["syllables"] > 100, pid
    assert set(piece(manifest, "no19")["characters"]) >= {"MARCELLINA", "FIGARO", "BARTOLO",
                                                          "DON CURZIO", "IL CONTE", "SUSANNA"}


def test_finale_matches_experiment_decode(manifest):
    old = OPERA.work.parent / "finale2-ba" / "ba_score.json"
    if not old.exists():
        pytest.skip("experiment decode not present")
    old = json.loads(old.read_text())
    new = json.loads((OPERA.work / "decode" / "pieces" / "no16.score.json").read_text())
    strip = lambda xs: [{k: v for k, v in x.items() if k != "page"} for x in xs]
    assert strip(old["notes"]) == strip(new["notes"])
    assert old["syllables"] == new["syllables"]
    assert strip(old["bars"][:-1]) == strip(new["bars"])  # the experiment counted a 940th bar
