# Experiment scripts (copied, not yet wrapped)

Copied unchanged on 2026-11-11 from `libretto-scratch/sextet-experiment` (branch
`sextet-experiment`, commit 3c3327e), so the pipeline's code lives in one repo. They are the
logic that P4 ("the tool") wraps into stages; until then they run only from the scratch
directory, where their paths and data are.

| Script | Role | Route |
|---|---|---|
| `phase4_align.py` | features, coarse subsequence DTW, MrMsDTW; the shared `align()` | core |
| `phase6_timeline.py` | vocal-stem singing detection; singing agreement and entry checks | core |
| `phase8_text.py` | letter DP: clean decoded syllables against the reference text and gloss | core |
| `phase9_recording.py` | a second recording through the same score lines | core |
| `phase10_recit.py` | MMS_FA forced alignment of secco recitative on the vocal stem | C |
| `ba_finale2_score.py` | Bärenreiter decode -> lines and notes in score time (finale only) | B |
| `finale2_build.py`, `finale2_check.py` | finale build and checks for both recordings | B (and A) |
| `finale2_score.py`, `phase2_compare.py`, `phase5_lines.py`, `phase7_anchors.py` | Ricordi scan (Audiveris) and typeset-Sextet route | A |
| `*_text.py` | hand-written reference text and literal gloss per passage | text |

Left behind in scratch: `ba_vector.py` (now `libretto_pipeline/decode/vector.py`, with the
double-barline fix), `phase1_bars.py` and `phase2_clicks.py` (route A tools only).

Route A is kept here only because `finale2_build.py` still imports `finale2_score.py`; P4
separates them.
