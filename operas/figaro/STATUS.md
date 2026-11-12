# Le nozze di Figaro: build status

Score: Bärenreiter BA 4565 (vector PDF, licensed). Recordings: Solti 1981 (Decca),
Currentzis 2014 (Sony). Plan: P0–P8 below prepare the full build; the full build follows
P8's regression check. Decisions: `docs/sung-text-pipeline.md` ("Decisions").

## Preparation plan

| Stage | What | State |
|---|---|---|
| P0 | Housekeeping: decisions recorded, branches, decoder fix | done 2026-11-11 (scratch commit pending, see below) |
| P1 | Workspace: `pipeline/` package, config, CLAUDE.md, this file | done 2026-11-11 |
| P2 | Decode the whole score into pieces; manifest; check against the contents | done 2026-11-11, reviewed |
| P3 | Recordings: inventory, mix and Demucs stems (FLAC) for both full operas | done 2026-11-11 |
| P4 | The tool: stages score, clean, place, time, publish; per-piece outputs | code done; finale reproduces the experiment |
| P5 | Placement of every piece in each recording | done 2026-11-11, **waiting for review** |
| P6 | Text: verse list, phrase split, gloss (in session) for the five passages | texts converted from the experiment; score-text rewrite not started |
| P7 | Interchange format: piece id, bars, edition | publish writes 1.1-score with edition and piece; bars and roon-rd check not done |
| P8 | Regression: rebuild the five passages from scratch; route B side test on a recitative | not started |

## P2 result (2026-11-11)

`libretto-pipeline decode figaro` → `<work_root>/figaro/decode/` (manifest.md to review).

- 72 pieces: sinfonia, 29 numbers as 33 pieces (Nos. 18, 20, 27, 28 are an accompanied
  recitative and an aria each), 31 secco recitatives, 7 appendix versions (Nos. 12, 13a,
  15, 18, 23, 28, 28a).
- All 95 contents entries accounted for; every piece found in the contents.
- No bar-count problems in the main score. Two appendix pieces have one each (below).
- The five passages of the experiment decode to the bars in the report (Sextet 140,
  Arietta 79, recitatives 25/19/16, finale 939); the finale's notes and syllables are
  identical to the experiment's decode.

Decoder fixes made while doing this (all in `decode/vector.py`):
- double barline counted as one barline (the experiment counted a 940th bar);
- chorus systems: the system is the longest left-hand line (choir + piano), not the short
  one joining the choir staves;
- an upbeat is bar 0, as the edition numbers it (Nos. 13, 25, 28 aria, appendix arias);
- small inset systems (an ossia in No. 10) are not counted as bars;
- lyric size is matched per staff: full-size 9.2 pt, pages scaled down for big ensembles
  (2.33 × line spacing), choir staves (2.49 ×). The old fixed 9.2 pt missed most of the
  Sextet's words (813 → 2,489 syllables) and No. 29's on its scaled pages; the finale was
  not affected;
- notehead size scales with the staff (choir noteheads are smaller);
- "(partono)"-style stage directions and choir part names are not lyrics; sung asides that
  open a parenthesis are.

Open items from P2:
- `app-no15` (No. 15 as recitative): a second recitative starts mid-system on p. 530 after
  a double barline; pieces are only split at system starts. Appendix only.
- `app-no28a` (Rondo "Al desio"): one bar short at p. 555 (printed 93, counted 92), probably
  the cadenza. Appendix only.
- Secco recitatives put every character on one shared staff, named by cues above the notes.
  The manifest lists the characters; per-syllable speakers are P6's job.
- Chorus staves are mostly unlabelled on continuation systems ("?" in syllables_by_staff).

## P3 result (2026-11-11)

`add-recording` + `separate`: Solti 32 tracks (168 min, 44.1/16), Currentzis 83 tracks
(176.5 min, 192/24). Mix (mono 22.05 kHz FLAC) and Demucs htdemucs two-stem FLACs per
track, CPU at 5.4x real time (about an hour for both; Apple GPU fails). 2.7 GB on disk.
Model weights copied under work_root/.cache (Demucs from the HuggingFace cache, MMS_FA from
torch's), run offline (HF_HUB_OFFLINE).

## P4 / P5 result (2026-11-11)

Stages `score`, `clean`, `place`, `time`, `publish` (code in pipeline/src). Checks:
- score and clean reproduce the experiment's finale exactly (851 / 823 lines);
- `time` on Solti's finale reproduces the experiment: all 823 lines, start differences
  median 0.000 s, max 0.44 s; entries 78/80 matched, median |offset| 0.088 s, 91% within 1 s.

Placement: per act, all pieces (numbers and recitatives) joined in score order and aligned
to the act's audio with one DTW. (Placing number by number failed: one wrong local match,
No. 15 inside the finale, threw the rest of Act II off.) Result, both recordings:
- every one of the 34 numbers placed; track titles agree for 32/33 (Solti) and 33/33
  (Currentzis) where a title names its number. Solti's No. 9 "disagrees" only because
  Decca's track title names Nos. 7 and 8;
- number fit 0.68-0.86; recitative fit 0.47-0.63 (sparse, as expected);
- tempo ratios 0.55-2.61; the extremes are the nominal-tempo rule, not misplacements
  (6/8 Andante "Deh vieni" and "Che soave zeffiretto" taken slowly; the Presto sinfonia
  alla breve). No number is squeezed: neither recording appears to cut a number.
- known positions agree with the experiment's hand-picked ones (Solti finale d10t04-06;
  Currentzis finale t36-t42, "Voi che sapete" t26-t27, "Eccovi" t51).

Known gaps:
- No. 23 "Amanti costanti": unlabelled choir staves inherit names from earlier systems
  instead of being recognised as the chorus (needs staff size). Not in the five passages.
- Roon matching (disc numbers per album, titles vs tags) is P7/P8.

## Pending for the user

- ~~Commit the decoder fix in `libretto-scratch`~~ done 2026-11-11 (313c7b4).
- ~~`libretto/.claude/settings.json`~~ done 2026-11-11, with `settings.local.json`
  (UV_CACHE_DIR, not committed). The scores folder is write-denied in both layers.
- ~~Where `libretto-work` lives~~ marked Dropbox-ignored 2026-11-11 (stays local, no
  longer synced); revisit when the external disk arrives.