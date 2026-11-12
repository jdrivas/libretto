"""Unit tests that need no score: version line, log format, decoder helpers, contents parsing."""
import logging
import re

from libretto_pipeline import logs
from libretto_pipeline.config import Cast
from libretto_pipeline.decode import contents, vector
from libretto_pipeline.version import VERSION_STRING


def test_version_line():
    assert re.fullmatch(r"libretto-pipeline \d+\.\d+\.\d+ \d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3} "
                        r"\([0-9a-f]+(-dirty)?\)", VERSION_STRING) or "unknown" in VERSION_STRING


def _line(level, utc=False):
    rec = logging.LogRecord("x", level, __file__, 1, "hello", None, None)
    return logs.LineFormatter(utc).format(rec)


def test_log_line_format_and_alignment():
    info, debug, trace = _line(logging.INFO), _line(logging.DEBUG), _line(logs.TRACE)
    assert re.fullmatch(r"\d{4}-\d\d-\d\d \d\d:\d\d:\d\d\.\d{3} <info>    hello", info)
    assert debug.index("hello") == info.index("hello") == trace.index("hello")
    assert "<warn>" in _line(logging.WARNING)


def test_log_levels_in_order():
    assert list(logs.LEVELS) == ["trace", "debug", "info", "warn", "error", "off"]


def test_double_barline_is_one_barline():
    assert vector.merge_double_barlines([160, 293, 372, 445, 495, 498]) == [160, 293, 372, 445, 495]
    assert vector.merge_double_barlines([100, 200]) == [100, 200]


def test_lyric_sizes():
    assert vector.is_lyric_size(9.2, 3.96) and vector.is_lyric_size(9.0, 3.95)
    assert not vector.is_lyric_size(8.6, 3.96)          # a stage direction on a full page
    assert vector.is_lyric_size(8.4, 3.6)               # a page scaled for a big ensemble
    assert vector.is_lyric_size(7.9, 3.17)              # choir staff
    assert vector._STAGE_DIRECTION.fullmatch("(partono)")
    assert not vector._STAGE_DIRECTION.fullmatch("(Che")  # a sung aside


def test_cast_matching():
    cast = Cast([("IL CONTE", ["il conte", "il c"]), ("LA CONTESSA", ["la contessa", "la c"]),
                 ("SUSANNA", ["susanna", "sus"]), ("DON CURZIO", ["d c"])])
    assert cast.find("Il C.") == {"IL CONTE"}
    assert cast.find("II CONTE LA CONTESSA") == {"IL CONTE", "LA CONTESSA"}
    assert cast.find("SUSANNA") == {"SUSANNA"}
    assert cast.find("D. C.") == {"DON CURZIO"}
    assert cast.find("Coro") == set()


def test_contents_entries_stop_at_german_block():
    text = "\n".join([
        "XII", "Sinfonia . . . . . . . . .  ", "1", "Atto primo", "Scena I",
        "No. 1 Duettino Cinque … dieci (Figaro, Susan-", "na) . . . . . . .  ", "11",
        "Recitativo Cosa stai misurando (Susanna, Figaro) . . . . .  20",
        "Sinfonia . . . . . . . . .  ", "1", "Erster Akt", "Nr. 1 Duettino . . . .  ", "11"])
    entries = contents._entries(text)
    assert [p for _, p in entries] == [1, 11, 20]
    assert entries[1][0].startswith("Atto primo Scena I No. 1 Duettino")
