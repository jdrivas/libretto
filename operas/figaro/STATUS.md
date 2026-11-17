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
| P6 | Text: verse list, phrase split, gloss (in session) for the five passages | done 2026-11-11, **waiting for review** |
| P7 | Interchange format: piece id, bars, edition; roon-rd matching | pipeline side done 2026-11-11; roon-rd change awaiting approval |
| P8 | Regression: rebuild the five passages from scratch; route B side test on a recitative | done 2026-11-12; listening check passed |

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

## P6 result (2026-11-11)

`libretto-pipeline text figaro <piece>` prints a piece's decoded text by speaker (the draft
the session writes from). Texts in operas/figaro/text/:
- the three recitatives rewritten from Bärenreiter's text, speakers and punctuation
  ("oh caro amico", "sì rimoti", "del denar", "al gusto mio", "cosa nova", ...); letter
  match against the decoded score 75-91% -> 100%;
- the finale and Sextet respelled where Bärenreiter differs from the libretto (19 words:
  "Cogl'uomin", "diventaste/foste", "de' nostri vassalli", "Uh che testa!", "di bon core",
  "ei se la sposa", ...); two glosses changed with the meaning; "Voi che sapete" already
  matched.
Decoder fixes found on the way: speaker cues stacked above a stage direction (window
22 -> 40 pt, never above the staff above); the page footer "BA 4565 b" was read as a lyric
on scaled ensemble pages; "I" was dropped as a choir part name, losing "Io!".

## P7 result (2026-11-11)

- Every segment carries `piece` and `bar_start`/`bar_end` (bar + fraction, as printed in
  BA 4565). Route B from its score times; route C by cleaning the recitative's decoded
  syllables against the same text, so each phrase has a score position.
- `publish` writes version 1.1-score with `edition` and lists every track of the recording
  (empty where nothing is timed yet). roon-rd loads the file unchanged ("32 tracks").
  (It first renumbered discs from 1; Roon in fact shows Solti's as 9-11, like the files,
  so since 2026-11-15 the files' disc numbers are kept.)
- First real route C run (Solti, "Eccovi" + "Andiam"): word times agree with the
  experiment's to about 0.1 s. "Andiam" shows 50/72 words "mostly in silence": its two
  light voices sit below the singing detector's threshold (the experiment placed the
  words identically); a limit of the check, not the timing.
- roon-rd (branch libretto-number-match, not committed): when the album matches and the
  file numbers its tracks, a disc/track number the file lacks means "not in this file",
  not a title-matching fallback (the "track 52" case).

## P8 result (2026-11-12)

`libretto-pipeline decode figaro` then `build figaro no12 rec12a no16 no19 rec19a rec19b`
from the score PDF and the stems, both recordings. Checks against the vocal stem
(compare: pipeline/experiment/compare_p8.py):

| Passage | Solti | Currentzis |
|---|---|---|
| No. 16 finale (B) | agreement 84%, entries 79/79, median 0.12 s, 92% within 1 s | 88%, 79/79, 0.06 s, 96% |
| No. 19 Sextet (B) | 84%, 11/11, 0.94 s, 64% (experiment, same check: 85%, 18/21, 0.40 s, 83%) | 89%, 13/14, 0.10 s, 85% (experiment 87%, 25/25, 0.33 s, 80%) |
| No. 12 "Voi che sapete" (B) | 87%, 12/12, 0.13 s, 100% | 91%, 13/14, 0.12 s, 100% |
| rec12a "Bravo!" (C) | 6/110 words in silence | 4/110 |
| rec19a "Eccovi" (C) | 14/95 | 3/95 |
| rec19b "Andiam" (C) | 50/72 (light voices below the detector; same times as the experiment) | 3/72 |

The finale and recitatives agree with the experiment's lines to a median 0.00-0.01 s.
Found and fixed on the way: placement can start a number 24 s early after a recitative
(Solti's Sextet), so route B now refines each placed start and end from the piece's opening
and closing 20 s of score. Solti's Sextet remains weaker than the experiment's on entries.

Route B side test (decision 2), "Eccovi": route B's phrase starts differ from route C's by a
median 0.72 s (Solti; 21/42 over 1 s) and 0.14 s (Currentzis). Route C stays for secco.

Listening check in the viewer (2026-11-12): passed, both recordings. Solti first showed
0/0 segments: Roon calls its album "Solti: The Operas - Mozart" and its titles differ from
the file's tags, so roon-rd's title matching picked an empty track. Fixed by an
`album_title_aliases` entry in opera.toml (carried into recording.json and the interchange)
and, in roon-rd (5890dc3), a title match within 1 s of the playing length wins. Solti's
Sextet drifts a little at times and recovers; left for now (see its entry figures above).

## Act I (2026-11-12/13)

Texts for Nos. 1-10 and their recitatives written in session (three lines follow the
libretto over the decode's staff; see the commit). The chorus is read from its top choir
staff (Nos. 8-9 letter match 0.69 -> 1.0). Built for both recordings.

Fixed on the way:
- **Chroma shift** (the main one): a piece's own key estimate over all 12 shifts was a
  fourth or fifth off (Solti Nos. 2-3; the 20 s edges of Solti's finale and Nos. 3-4 and
  Currentzis's finale), and the alignment then matched the wrong music. Alignment and edges
  now try shifts {0, 1, 11}, as placement does. Every number in both recordings: singing
  agreement 78-94%, entries median 0.03-0.26 s; Solti's Sextet 0.94 -> 0.11 s (92% within
  1 s), so its drift should be gone.
- **roon-rd matching** (e4ba2a0): Roon truncates lengths to whole seconds and passes no disc
  number; a track fits Roon's length L if it lasts L to L + 1 s. Exact title, then an
  exact-album file's fitting track, then length. Publish writes durations to the ms.
- A tempo-limited guide for the fine pass (option A) is in align.py, off (`GUIDE`): it did
  not help, as the cause was the key.

Listening check (2026-11-15): passed, both recordings, recitatives included (Solti's
recitatives show 10-32% of words "in silence" against Currentzis's 3-15%, but sound right:
the detector, not the timing). Option B (vocal-stem onsets as alignment features) not
needed.

## Act II (2026-11-15)

Texts for Nos. 11, 13-15 and their seven recitatives written in session (four speeches moved
to Da Ponte's speaker where the decode missed a cue: decisions.md D16). Built for both
recordings; Act II is complete with Nos. 12 and 16 from P8. Letter match 1.0 in every number.

| | Solti | Currentzis |
|---|---|---|
| No. 11 | 87%, entries 0.20 s | 89%, 0.22 s |
| No. 13 | 90%, 0.03 s | 82%, 0.07 s |
| No. 14 | 88%, 0.10 s | 87%, 0.14 s |
| No. 15 | 71%, 0.16 s | 91%, 0.06 s |
| Recitatives, words in silence | 3-22% (rec15a 12/25) | 1-14% (rec15a 10/25) |

Solti's No. 15 is the detector, not the timing: Susanna's sotto voce opening (bars 5-10)
sits at the orchestra's level in the vocal stem, and the first line is placed 0.2 s after
the voice appears. Every line published once (2264 segments in each recording). Spot check
passed 2026-11-15 (D17: a full listening comes when the opera is built).

## Act III (2026-11-15)

Texts for the 15 remaining pieces written in session, after a decoder fix (D19): No. 23's
choir staves are smaller and were unlabelled, so they took the soloists' names, and "Il C."
and "La C." are Times-Roman labels on about 100 systems, which the decoder had ignored.
Speeches moved per D16: rec17a, rec18a, rec22a (three), plus "Rimandate il sigillo"
(rec21a), which the decode does not read. Built for both recordings with rec09a (its
decoded speakers improved): 51 pieces, 2691 segments each, every line once.

| | Solti | Currentzis |
|---|---|---|
| No. 17 | 86%, entries 0.08 s | 87%, 0.07 s |
| No. 18 recit. / aria | 87%, 0.15 s / 84%, 0.11 s | 87%, 0.16 s / 81%, 0.12 s |
| No. 20 recit. / aria | 79%, 0.28 s / 82%, 0.16 s | 76%, 0.32 s / 80%, 0.18 s |
| No. 21 | 80%, 0.50 s | 84%, 0.51 s |
| No. 22 | 84%, 0.19 s | 65%, 0.34 s |
| No. 23 | 86%, 0.16 s | 91%, 0.12 s |
| Recitatives, words in silence | 0-17% | 0-43% (rec17a 36/84) |

The orchestral recitatives stay with route B: No. 18's is as good as a number, and No. 20's
is weaker but not bad. Weakest: No. 21 (the Countess's first "sotto i pini", bars 18-21, may
be early; the final "il capirà" exchange is squeezed at the end) and Currentzis's No. 22.

Spot check (2026-11-16): Solti's No. 21 "quite late", "Dove sono" entrance a little late,
Currentzis's No. 22 chorus starting late; No. 20 recitative (Solti) "quite good", No. 23
"Amanti costanti" (Currentzis) fine. Cause: a number's start is refined by finding its
opening, and that search couldn't follow paces 1.6-2.3x the printed tempo (No. 21's start
landed 50 s late). Fixed (D21) and re-timed; 2691 segments each, every line once:

| | Solti | Currentzis |
|---|---|---|
| No. 18 aria | 85%, 0.11 s | 83%, 0.08 s |
| No. 20 aria | 83%, 0.12 s (starts 4 s earlier) | 81%, 0.11 s |
| No. 20 recit. | (as built) | 77%, 0.30 s |
| No. 21 | 85%, 0.19 s (starts 49 s earlier) | 84%, 0.10 s |
| No. 22 | (as built) | 75%, 0.11 s (starts 16 s earlier) |
| No. 23 | 87%, 0.16 s | (as built) |
| Act I: Nos. 5, 8, 9, 10 | (as built) | 86% / 88% / 82% / 89%, 0.01-0.12 s |

## Act IV (2026-11-16)

Texts for the 13 pieces written in session. Speeches moved per D16: rec24a ("L’ho
perduta, cugino" to Barbarina), rec27a ("Parmi umida la notte" to the Countess), rec28a
("La la la la lera" to Cherubino, "Ahi me meschina!" to the Countess). No. 29's "Perdono"
and closing tutti are one TUTTI card (D23). No. 24 keeps its placed end (D24). Built for
both recordings: 64 pieces, 3327 segments each, every line once.

| | Solti | Currentzis |
|---|---|---|
| No. 24 | 90%, entries 0.07 s | 88%, 0.12 s |
| No. 25 | 83%, 0.10 s | 80%, 0.08 s |
| No. 26 | 78%, 0.15 s | 86%, 0.07 s |
| No. 27 recit. / aria | 71%, 0.56 s / 93%, 0.11 s | 79%, 0.24 s / 93%, 0.05 s |
| No. 28 recit. / aria | 78%, 0.20 s / 62%, 0.35 s | 86%, 0.30 s / 81%, 0.10 s |
| No. 29 | 82%, 0.09 s | 84%, 0.06 s |
| Recitatives, words in silence | 1-57% (rec27a 48/84) | 3-27% |

Solti's "Deh vieni" at 62% is the detector: Popp's soft singing stays well above the
orchestra in the vocal stem through the "silent" phrases, and the line gaps match its dips.

Spot check (2026-11-16): the user found everything good. All of Figaro is built; the D17
full listening and D22 are now due.

## Pending for the user

- ~~Listen to and watch the choruses (shown once, from the top choir staff)~~ done
  2026-11-15: works as it is (decisions.md D10).

- ~~Commit the decoder fix in `libretto-scratch`~~ done 2026-11-11 (313c7b4).
- ~~`libretto/.claude/settings.json`~~ done 2026-11-11, with `settings.local.json`
  (UV_CACHE_DIR, not committed). The scores folder is write-denied in both layers.
- ~~Where `libretto-work` lives~~ marked Dropbox-ignored 2026-11-11 (stays local, no
  longer synced); revisit when the external disk arrives.