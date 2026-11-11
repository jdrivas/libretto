# Sextet Experiment Plan: Score → Bar Times → Synchronized Lyrics

*Drafted 2026-11-04.*

## Why

Two separate problems hurt the current libretto viewer:

1. **Text fidelity.** A libretto is a literary distillation of what's sung. It leaves out repetitions, writes simultaneous ensemble lines one after another, and its translations follow the poetry rather than the sung words.
2. **Timing.** Segment start times within a track are word-count proportional estimates, not real times.

The proposed approach uses the score as the source of structure and text, and aligns the recording to it for timing:

```
score (PDF) ──OMR──► MusicXML (bar structure, notes)
                         │
score pages ──vision──► lyrics per measure
                         │
recording (FLAC) ──audio-to-score alignment (DTW on chroma)──► bar → seconds
                         │
                         ▼
segment → measure → start time  ──► interchange file ──► roon-rd viewer
```

We use audio-to-score alignment rather than tempo/beat extraction. Tempo integration drifts, and it fails on rubato, fermatas and recitativo secco. Dynamic time warping (DTW) on chroma features matches every point against the actual sound, so it doesn't drift.

This experiment tests each step on a single number, the Sextet (No. 18/19, "Riconosci in questo amplesso"; No. 18 in the Ricordi vocal score, No. 19 in Bärenreiter BA 4565, which is the reference), before committing to the whole opera.

## Why the Sextet

- A clean typeset PDF exists (IMSLP #115717, vector, Finale-produced, 33 pages).
- The original Finale engraving files exist (IMSLP #230082), which gives an exact reference score.
- The Ricordi vocal score scan (IMSLP #873093) contains it, so we can measure scan quality against that reference.
- It's a six-voice ensemble, the hardest display case.
- Earlier grouping work exists in `libretto-scratch/score-exploration/`.

## Inputs

| Input | Location |
|---|---|
| Solti 1981 Sextet track (FLAC) | `roon-rd/Audio/10-10 - Sir Georg Solti - ... Sextet. Riconosci in questo amplesso – Recit. Eccovi, o caro amico – Andiam, bel paggio.flac` (gitignored) |
| Typeset Sextet PDF | `libretto-scratch/score-exploration/IMSLP115717-WIMA.17b9-ricon.pdf` |
| Ricordi vocal score scan | `libretto-scratch/score-exploration/IMSLP873093-PMLP3845-Ricordi_Le_nozze_di_Figaro.pdf` |
| Finale source (IMSLP #230082) | **To download by hand** (IMSLP disclaimer/timer) |
| Solti interchange file | `libretto-scratch/working/librettos/georg-solti-1981.interchange.json` |

Note: the track also contains the recitative "Eccovi, o caro amico" and "Andiam, bel paggio" after the Sextet. The alignment must handle a score that covers only part of the track.

Experiment workspace: `libretto-scratch/sextet-experiment/`.

## Phases (in execution order)

Phase 4 (alignment) runs right after Phase 1. If alignment fails even with a perfect reference score, the approach is dead regardless of scan quality, so that answer comes first.

### Phase 0: Setup (½ day)

- Homebrew: `uv`, `ffmpeg`, Audiveris (OMR).
- `uv` project in the workspace with Python 3.12: `librosa`, `synctoolbox`, `music21` and/or `partitura`, `numpy`, `soundfile`.
- Download the IMSLP #230082 Finale ZIP.

### Phase 1: Reference score (½ day)

**Update 2026-11-04:** the IMSLP ZIP (#230082) contains only Finale 2007 `.mus` files (full score plus per-part files), with no MusicXML, and nothing free opens `.mus`. But the same engraver's **MIDI** (`WIMA.f0ac-ricon.mid`, in the Sextet's IMSLP audio section) is an exact reference for notes and bar structure. It has 15 tracks: 8 orchestral (flute, oboe, bassoon, horn, violins I/II, viola, cello) and 6 named vocal parts (Soprano 1/2, Tenor, Baritone, Bass 1/2), plus tempo and time-signature maps. It has no lyrics. At notated tempos it runs 306.6 s; the Solti track is 393 s, and the difference is roughly the trailing recitative. Files are in `libretto-scratch/sextet-experiment/source/`.

The reference splits in two:

- **Notes and bars (for Phase 4):** the MIDI. Derive measure boundaries from its time-signature map, then check the measure count against the typeset PDF.
- **Lyrics (for Phase 5):** Audiveris on the typeset PDF, then hand-correct the lyrics only, or use the Phase 3 vision transcription of the typeset pages. Notes don't need correcting, since the MIDI covers them.

Map MIDI vocal parts to characters (Soprano 1/2 = Susanna/Marcellina, Tenor = Curzio, and so on; confirm against the score's staff labels).

- **Outputs:** `sextet.bars.json` (measure → MIDI time), `sextet.parts.json` (MIDI track → character).

### Phase 4: Audio alignment (1 day)

- Convert the FLAC to WAV; compute chroma features for the recording and for the reference score (vocal lines included, since they carry the recitatives).
- Align with `synctoolbox` (DTW, plus onset-based refinement if available). Handle the trailing recitative with subsequence alignment or by trimming the track to the end of the Sextet.
- **Output:** `sextet.bars.csv` (measure number → seconds).
- **Check by ear:** render a click track at the computed barlines mixed over the recording; David listens for clicks landing on the bars and for drift. Claude can't process audio, so this check is human.

**Result 2026-11-04: pass.** Script: `phase4_align.py`. Coarse subsequence DTW placed the Sextet at 0.4–283.6 s of the 393 s track; the remainder is the recitative. Fine MrMsDTW alignment against the reference MIDI gave a performed tempo of about 121 bpm median (MIDI notated: 110), with no cumulative drift. The two sharp slowdowns (bars 85, 94) line up with ritardandi/held pauses in the MIDI tempo map, so they are musical, not errors. Bar 140's tempo is an artifact, because the MIDI ends on the last note rather than at the barline. Listening check (David): "not perfect, but where it's late, early, or slightly confused it rights itself pretty quick." Errors stay local at roughly beat level, which is good enough for line-level lyric display. Possible later tuning: weight the vocal parts more in the score features, and refine locally with onsets.

### Phase 2: Scanning accuracy (½ day)

- Run Audiveris on (a) the typeset Sextet PDF and (b) the Sextet pages of the Ricordi scan.
- Script a comparison against the reference: measure count, per-measure pitch/onset similarity, lyric syllable accuracy.
- Re-run Phase 4 alignment using the scan-derived MusicXML (b) and compare bar times with the reference-derived ones.
- **Output:** accuracy table, plus bar-time differences.

**Result 2026-11-05: fail as-is (Audiveris 5.11, default settings).** Scripts: `phase2_compare.py`; the shared `align()` is in `phase4_align.py`. Ricordi Sextet = No. 40, scan pages 219–234.

| | Ricordi scan (16 pp) | Typeset PDF (32 pp, sheets 2–33) |
|---|---|---|
| Bars detected | 144 (real 140); local errors both ways, e.g. p.219: 20 found vs 16 real | 140 ✅ |
| Pitch content | Pitch-class profile close to MIDI ✅ | Not separately checked |
| Lyrics | 0 captured ❌ | 0 captured ❌ |
| Alignment vs MIDI reference | Bar numbering unusable (scanned bar 144 ≈ ref bar 132) | Bars 1–45 within ~0.2–0.5 s; bars 46–140 drift 0.5–1.5 s late |

Tie-breaker: onset strength at bar times. MIDI-aligned 2.14; typeset-OMR-aligned 2.00 for bars 1–45 but 1.76 for bars 46–140, which is at baseline (half-beat-shifted 1.77, random 1.83). So the MIDI alignment is right, and alignment from an Audiveris score degrades badly after the first third.

Likely causes: misread rhythms and pitches; transposing instruments (horns in F, octave-treble tenor clef) not handled; vocal staves that change from system to system in the Ricordi layout; no lyric OCR (check Tesseract language data).

Also fixed along the way: coarse subsequence DTW now uses slope-constrained steps (tempo 0.5×–2×). Without it a sparse score collapsed onto the last 46 s of the track. The MIDI result is unchanged (no bar moved more than 0.04 s).

**Re-test 2026-11-05: typeset scan passes once loaded correctly.** Two of the drift causes above were bugs in my loader, not Audiveris: tied notes weren't merged (fixed with `stripTies`), and the horns in F weren't transposed. With both fixed, typeset-scan bar times against the MIDI-based ones: median 0.42 s, max 1.61 s, 94% within 1 s, 100% within 2 s. The Ricordi scan's bar count is still wrong.

**Lyrics OCR 2026-11-05:** Audiveris needs Tesseract language data (installed `ita` and `eng` in `~/Library/Application Support/AudiverisLtd/audiveris/tessdata/`, plus the constant `org.audiveris.omr.text.Language.defaultSpecification=ita+eng`). Typeset: 1,464 syllables, including some dynamics misread as text, which are filtered out. Ricordi: 1,155. The text has OCR damage ("pub" for "può", run-together words, missing syllables). Side effect: with OCR on, misread character labels split the score into 67 parts. That broke part-based transposition, and its alignment drifted 2–3 s late by the end. So **take notes from the non-OCR run and text from the OCR run** (bar-by-bar mapping in `phase7_anchors.py`).

### Phase 5 result: end to end (2026-11-05/06)

`phase5_lines.py` produces sung-text lines from the OCR score. Characters are identified from cast order, OCR'd staff labels and clefs, and all six came out correct. Syllables are joined into words, and lines break at rests of 1 beat or more, at punctuation, or at 9 words: 236 lines in total. A copy of the Solti interchange file with the Sextet track replaced by these lines is served by roon-rd (`--libretto-dir …/sextet-experiment/out/librettos`).

- **First viewing** (timed by the OCR-run alignment, viewer without smoothing): the first lines worked and quick exchanges read well, but the ensembles were jumpy and lines were "pretty off" by the end.
- **Second viewing (2026-11-06)**, with timing from the non-OCR scan run and two viewer fixes: smooth playback position between Roon's whole-second reports, and lines shown 0.7 s early (`?lead=` URL parameter). Verdict: **"Lines land quite well."** Milestone 1 timing is met for the Sextet with a score-only pipeline: no MIDI, no hand-marking.

### Phase 6: vocal timeline diagnostic (2026-11-06)

`phase6_timeline.py` separates the vocals with Demucs htdemucs (46 s for the Sextet on CPU). It uses the vocal-stem loudness as the singing/silence signal; pYIN's own confidence is too low in ensembles to use. It runs pYIN pitch on the vocal stem and writes `out/timeline/index.html`, an interactive scrolling timeline with audio. The user found the separation "very good".

| | MIDI-based alignment | Scan-only alignment |
|---|---|---|
| Score and vocal stem agree on singing vs silence | 87% of frames | 84% |
| Entries after silence: median offset | 0.19 s | 0.46 s |
| Entries within 1 s | 11/12 | 15/17 |

Of 161 character phrase starts, 20 follow a silence and 27 are direct handovers between voices; both kinds are detectable. The other 114 are voices joining others already singing, and they aren't detectable with a single-pitch tracker. From about 3:07 to the end, someone is always singing.

### Phase 7: anchor snapping (2026-11-06): no gain, switched off

`phase7_anchors.py` detects entries after silence (loudness) and register handovers (pitch jumps) and uses them as anchors to correct the scan-only alignment. 41 candidates, 32 found, 24 kept. Leave-one-out at the anchors: scan-only median 0.25 s (100% within 1 s), snapped 0.44 s (92%). The base alignment is already as precise as the anchor detection, so snapping only adds noise. Kept behind `USE_SNAPPED_FOR_LINES` in case a weaker score or recording needs it. Considered BS-RoFormer instead of Demucs (cleaner vocal stem, roughly +3–4 dB SDR on benchmarks); not needed for now.

### Phase 8: clean text, literal translation, ensemble display (2026-11-06)

- **Text clean-up (`phase8_text.py`, `sextet_text.py`).** Each character's sung text is written once, as the score has it, split into short phrases with a very literal English gloss. Each character's OCR stream is aligned to it letter by letter. The DP may jump back to a phrase start, because singers repeat phrases. The OCR decides what is sung when and how often; the reference only fixes spelling. 92–97% of OCR letters matched. Rules: performance directions ("sotto voce") are stripped; a word counts only if 60% of its letters match; dropped words are filled in, going with whichever neighbour shares their phrase; a phrase picked up mid-way after a repeat gets its opening words back. The result is 220 lines, each with `gloss` (aligned phrases), `translation` and `verses`.
- **Viewer (roon-rd).** Switches in the header, remembered per browser:
  - **Translation: Aligned / Line / Off.** Aligned puts each Italian phrase over its literal English and is the default.
  - **Ensemble: Stanza / Rows.** Stanza shows each character's text once per passage, fixed, with a highlight moving to the phrase being sung. Rows shows each character's last three lines.
  - Ensemble passages are computed once per track: two voices overlapping for at least 0.5 s starts a passage, and at most one voice for 2.5 s ends it.
  - The viewer now also follows a single Roon zone. Before, other zones' "no match" messages replaced the matched track.
- **Layout revised the same day: ensembles as cards in the scrolling list** (commit `f5ab2e2` on roon-rd branch `sung-text-viewer`). Switching between the list and a header panel was distracting, so the list became one continuous document with each ensemble passage as a stanza card in its place. Passages end just after the last overlap, and boundary rules decide which edge lines belong to a card. A check that every line ends up either in the list or in a card is the regression test for those rules: two boundary bugs (a skipped final ensemble, a lost Susanna solo) were found that way. User verdict: "It's just incredible. That was perfect."
- **Kept in reserve:** a fixed-stage layout (a row per singer, always) and "score systems" (lanes per character, words in time order, page turns), plus stanza extras (a fading trail of recent phrases, repeat counts).
- **User reaction:** "That is amazing. I'm almost speechless." Earlier, line-based ensemble rows were "hard to follow"; they changed about once a second.

### Phase 9: second recording, Currentzis / MusicAeterna 2014 (2026-11-07)

`phase9_recording.py` re-times the prepared lines for any recording. Lines carry `score_start`/`score_end` (score position), so only the alignment is per recording. Nothing was done by hand. The tuning was −35 cents (period pitch, about A=431), handled automatically.

- **First run:** right up to about 3:40, then behind. The coarse search had placed the Sextet's end at 5:22 instead of 5:40, because Currentzis's finale is much broader, and the last 20 bars were squeezed (0.73× Solti's duration against 1.15–1.3× elsewhere).
- **Fix: choose the score's placement automatically** from the coarse placement, "to music end" and "whole track". Harmonic fit (score chroma against the audio it maps to) vetoes placements that spread the score over other music; singing agreement with the vocal stem then decides, and must improve on coarse by more than one point. Singing agreement alone picked wrongly for Solti (stretched over the following recitative, 77.8% against 77.1%); harmonic fit catches that (0.719 against 0.746).
- **Results:** Currentzis 85% singing agreement, entries after silence 19/19 found, median 0.35 s, 89% within 1 s. Solti re-checked: coarse placement kept, 85%, 0.42 s, 82%.
- **roon-rd** (commit `c43ef00`): Roon reports both recordings' albums alike, so the matcher now tells recordings apart by track length.
- **User verdict:** "Yes this works."
- **Still there:** around bar 94 (Currentzis about 3:42) the vocal stem goes silent for about 4 s where the score has three characters singing. Probably a dramatic pause or a big ritardando. The alignment loses 2–4 s and recovers by about 4:00.

### To look at next (user, 2026-11-07)

1. **Ensemble highlight lingers:** in ensembles some phrases stay highlighted after they've been sung. Likely causes: the 1 s hold after a line's last note (`ENS_HOLD_S`); lines that contain long internal rests; or a line's `end` taken from its last syllable when a repeat continues without a rest.
2. **The lead into the last ensemble is a bit off in both recordings.** This is the bar ~94 area (ritardando, then "Al dolce contento"). Since it affects both recordings, suspect the score side first: the ritardando/fermata handling in alignment, the passage boundary rules, or line timing at the passage start (absorbed "dirà.", Count entering at 204.0).

### Agreed plan from here (user, 2026-11-07)

1. Put the experiment scripts (`libretto-scratch/sextet-experiment`) under git.
2. Fix the two issues above, checking every line, not only the reported spots.
3. The recitative after the Sextet (rest of the same track). This tests recitativo secco and a track holding several score sections.
4. One aria from another act, to catch anything Sextet-specific.
5. **Take stock.** Review the whole system and write up the current pipeline and build approach: each step, its inputs and outputs, what is automatic and what is still manual. Fine-tune it only where there's a clear reason.
6. Test those assumptions on a full opera build.

**Status 2026-11-08:** steps 1–4 are done, plus the Act II finale as an extra ensemble test.
- Voi che sapete and the Bravo recitative: committed `ffa9831`.
- The finale: committed `3c3327e`. The user's verdict was "near perfect", using the
  Bärenreiter vector decode.

**Step 5 written up (2026-11-08):** `sung-text-pipeline.md`. It covers the pipeline as built, what is manual, the weak points, and the proposed Figaro build, with open questions.

**Previously noted:** step 5. Write up the pipeline, starting from `scaling-notes.md`
(score-reading errors, the born-digital decode, the feature ideas). Open decision for it:
make the Bärenreiter decode the main path for Figaro, and re-time the Sextet, the arias
and the recitatives from it too (recommended).

### Noted for the full opera build (user, 2026-11-07)

- **A track missing from the file can match the wrong track.** Currentzis track 52 ("Andiam, andiam, bel paggio") isn't in the file yet, and playing it showed the Sextet. With no exact title match, fragment scoring falls back on the opera prefix every Roon-style title shares ("Le nozze di Figaro, K. 492: Atto Terzo: …"). Fix: strip that prefix from interchange titles too before scoring (or require a match on the distinctive part), so a missing track shows "no libretto" instead of another track's text.
- **Pipeline order:** phase8 rewrites Solti's Sextet track and drops the recitative lines that phase10 appended; phase10 must run after it.

### Open issues after the Sextet (2026-11-06)

1. ~~**Ensemble display.**~~ Done in Phase 8 (Stanza and Rows views; pending the user's choice between them after more listening).
2. **The rest of the track.** The Solti track runs 6:33. The Sextet ends at about 4:43 (bar 140), and the remaining ~2 minutes are the recitative "Eccovi, o caro amico" and "Andiam, bel paggio", which currently shows nothing. This is P2 (a track holds several score sections) and P3 for recitativo secco. Inverted strengths there: harmonic alignment is weakest (continuo only, free rhythm), but the vocal stem is strongest (one voice at a time with frequent pauses, so loudness entries and register handovers are plentiful).
3. ~~**OCR text damage.**~~ Done for the Sextet in Phase 8, using a hand-written reference text. Scaling question: where does a clean per-character reference come from for a whole opera? Options: a clean libretto source for spelling only, or a vision/LLM pass over the score pages.
4. ~~**Literal translation (P5).**~~ Done by hand for the Sextet (very literal, phrase-aligned). Scaling question: the same, plus a review pass.

### Phase 3: Lyrics via vision (½ day)

- Claude transcribes the vocal text from the Ricordi Sextet page images, per system, with measure numbers and character names.
- Compare against the reference lyrics: is each line placed in the right measures?
- This tests the hybrid of OMR for notes and vision for text.

### Phase 5: End to end (½ day)

- Map Sextet libretto segments to measures (from Phase 1 or 3 lyrics), derive segment start times from `sextet.bars.csv`, and write them into the interchange file.
- Play the track in Roon and watch the roon-rd libretto viewer. Ensemble display can stay rough for now.
- Optional: produce a close, literal English translation of the sung text to check how it reads in the viewer.

## Success criteria

| Question | Pass if |
|---|---|
| Does alignment work on opera? | Clicks land on the barlines by ear for most of the number; no cumulative drift |
| Are scanned scores good enough? | Bar times from the Ricordi scan within ~1 s of the reference |
| Are vision lyrics good enough? | Nearly all lines placed in the correct measures |
| User experience | The viewer highlights the line being sung, within a beat or two |

## Estimated effort

About 3–4 days total, with a stop/continue decision after each phase.

## If it works: scaling to the whole opera

- Batch OMR of the Ricordi scan (about 350 pages), with scripted checks of measure counts per number.
- Track ↔ measure-range mapping for each CD track (mostly from the numbers in the track titles; check against durations).
- Recitativo secco is expected to be the weakest area for alignment; evaluate separately.
- Our own literal translation of the score-derived text, replacing the published translation.

## Related docs

- `audio-forced-alignment.md`: earlier notes on lyric/voice forced alignment (Demucs + WhisperX). Superseded for timing by audio-to-score alignment, but still relevant for recordings without a score.
- `libretto-scratch/score-exploration/README.md`: score availability findings and MusicXML grouping scripts.
