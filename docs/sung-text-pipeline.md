# Sung-text pipeline: as built, and how to build a whole opera

*Step 5 of the agreed plan (take stock), 2026-11-08. This describes the pipeline that produced
every timed number so far, step by step, with its inputs and outputs and what is automatic
and what is manual. It then sets out the approach for step 6, a full build of Le nozze di
Figaro. Background and the detailed findings are in `scaling-notes.md`. The history of each
phase is in `sextet-experiment-plan.md`.*

## What exists today

Code: `libretto-scratch/sextet-experiment/` (Python, `uv`), about 3,500 lines in 18 scripts.
Bulk data: `~/Development/libretto-work/`. Viewer: roon-rd `/libretto-view`, which reads the
interchange files in `sextet-experiment/out/librettos/`.

| Number | Kind | Score used | Timing method | Solti 1981 | Currentzis 2014 |
|---|---|---|---|---|---|
| No. 15 (II) Finale "Esci omai" | ensemble finale, 940 bars | Bärenreiter vector PDF (Ricordi scan kept for comparison) | score alignment | 3 tracks | 7 tracks |
| No. 11 (II) "Voi che sapete" | aria | none | text forced alignment | ✓ | ✓ |
| Recit. "Bravo! che bella voce!" (II) | secco | none | text forced alignment | ✓ | ✓ |
| No. 18 (III) Sextet "Riconosci in questo amplesso" | ensemble, 140 bars | typeset IMSLP PDF, read by Audiveris | score alignment | ✓ | ✓ |
| Recit. "Eccovi, o caro amico" / "Andiam, bel paggio" (III) | secco | none | text forced alignment | same track as Sextet | own tracks |

*Source of these numbers:* the numbers in this table (Nos. 11, 15 and 18) are the Ricordi vocal score's. The 940 bars is our decoder's count. The reference is the Bärenreiter vocal score BA 4565: there the finale is No. 16, with 939 bars (the decoder also counted the closing double barline as a bar), "Voi che sapete" is No. 12 (Arietta), and the Sextet is No. 19. The technical report's §2 table gives Bärenreiter pages and bars for every passage.

All of them pass the every-line-accounted check (no line missing, none shown twice). Your
listening verdicts: the Sextet "lands quite well", and the finale is "near perfect".

## The pipeline as built

There are three routes to a timed number. They share the text clean-up, the vocal stems, the
silence trimming, the checks and the output format.

```
                    ┌──────────── per opera (score time, recording-independent) ────────────┐
 A  scan / typeset  │ PDF ─Audiveris─▶ notes + OCR syllables ─bar grid, who-sings─▶ lines   │
    score           │                                                    │                  │
 B  born-digital    │ PDF ─ba_vector─▶ notes + syllables + bars + tempo ─┤                  │
    score           │                                                    ▼                  │
                    │ reference text (per character, phrases + gloss) ─▶ clean lines ──────┼─┐
                    └───────────────────────────────────────────────────────────────────────┘ │
                    ┌──────────── per recording ─────────────────────────────────────────────┐│
                    │ tracks ─ffmpeg─▶ wav ─Demucs─▶ vocal stem (joined across tracks)       ││
 A, B               │ score notes ─MrMsDTW─▶ score s → audio s ─re-time─────────────────────◀┼┘
 C  no score        │ reference text ─MMS_FA on vocal stem─▶ word times                      │
                    │        ─▶ trim out of silences ─▶ checks ─▶ split into tracks          │
                    │        ─▶ interchange file (edited in place) ─▶ roon-rd                │
                    └────────────────────────────────────────────────────────────────────────┘
```

### Per opera: from the score to clean lines in score time

| Step | Script | Input | Output | Automatic? |
|---|---|---|---|---|
| 1. Get the score | none | IMSLP (Sextet: typeset; finale: Ricordi scan); your Bärenreiter PDF | PDF in `libretto-work` | **Manual:** found, downloaded and cut to page ranges by hand |
| 2a. Read the score (scan or typeset) | Audiveris batch: one notes run and one OCR run | page range of the PDF | MusicXML (`omr*`) | Automatic, about 1 min per page per run |
| 2b. Decode the score (vector) | `ba_vector.py <pdf> <pages>` | PDF page range | `ba_score.json`: bars with metre and tempo, notes, syllables on notes, staff labels | Automatic, 2 s for 110 pages. Specific to Finale with the Maestro font |
| 3. Reference text | `sextet_text.py`, `finale2_text.py` | libretto (murashev) and the score | per character: speeches → phrases → literal gloss | **Manual:** written in a session (by me) per number. The largest manual step |
| 4a. Lines from the reading (A) | `phase5_lines.py` (Sextet); `finale2_score.py` (finale) | MusicXML, cast, reference text | lines with syllables in score seconds; staff → character | Automatic, but the cast, staff order, pages on stage (`ON_STAGE`) and 120 bpm are hard-coded per number |
| 4b. Lines from the decode (B) | `ba_finale2_score.py` | `ba_score.json` | lines and notes in score seconds, from printed tempo and metre (`TEMPO_BPM`) | Automatic. Label → character table hard-coded |
| 5. Clean against the reference | `phase8_text.py` (Sextet); `finale2_build.py clean` | lines, reference text | clean lines: reference spelling, `gloss`, `translation`, `verses`; long filled-in phrases capped and split at gaps | Automatic |

### Per recording: timing

| Step | Script | Input | Output | Automatic? |
|---|---|---|---|---|
| 6. Find the audio | none | audio library (read-only) | file paths, titles, track numbers | **Manual:** paths, titles and disc/track numbers typed into the commands or the `RECORDINGS` table |
| 7. Decode and separate | ffmpeg, Demucs htdemucs (two stems), in `phase9_recording.py` and by hand for the finale | track files | wav and vocal stem per track; joined timeline plus `tracks.json` offsets | Automatic once given the tracks. The finale joins were run by hand |
| 8a. Align the score (A, B) | `phase4_align.align` (MrMsDTW), called by `phase9_recording.py`, `finale2_build.py rec` | score notes, recording | warping path score s → audio s | Automatic: coarse subsequence DTW picks the placement, vetoed by harmonic fit, then decided by singing agreement; tuning (A=431) detected. Finale: two passes with section tempo factors |
| 8b. Force-align the text (C) | `phase10_recit.py` (MMS_FA, wildcard between speeches) | reference text, vocal stem, **region start (s)** | word and phrase times | Automatic except the region start and the choice of texts (`--texts`), given by hand |
| 9. Re-time, trim, check | inside 8a/8b scripts; `phase6_timeline` helpers | lines, path, vocal stem | lines in audio time, cut back out of clear silences; check figures | Automatic |
| 10. Write the interchange file | the same scripts | existing interchange file | its tracks' segments replaced, or new tracks appended | Automatic but order-dependent: each script edits the shared file in place |
| 11. Verify display | headless-Chrome every-line check against roon-rd | interchange file | missing/twice counts per track | Automatic, run by hand after each change |

### Scripts that are now history

- `phase1_bars.py` and the MIDI path in `phase4_align.py`: the Sextet MIDI served as ground
  truth for testing the score-only route. The pipeline no longer needs MIDI.
- `phase2_*`: the Audiveris evaluation.
- `phase6_timeline.py`: a diagnostic page. Its stem-analysis helpers (silences, singing
  spans, entries) are used by everything.
- `phase7_anchors.py`: anchor snapping, switched off. The Sextet chain still reads its output
  file, with snapping disabled.
- `finale2_check.py`: register per minute. A weak signal.

## Results by route

| | A: Sextet (typeset) | A: finale (Ricordi scan) | B: finale (Bärenreiter) | C: recitatives, aria |
|---|---|---|---|---|
| Score text matching the reference | 92–97% | 82–89% | 99–100% | n/a |
| Speeches never found | not measured | 13 / 233 | 0 / 233 | n/a |
| Singing agreement | 85% / 85% | 70% / 74% | 83% / 88% | n/a |
| Entries after silence: median offset | 0.42 / 0.35 s | 0.63 / 0.60 s | 0.10 / 0.07 s | words in silence 0–5% |
| Entries within 1 s | 82% / 89% | 62% / 74% | 91% / 95% | register check per speech passes |

Figures are Solti / Currentzis. Numbering is Bärenreiter (NMA); the Ricordi score numbers differently.

Conclusions:
- **Route B is the best by a clear margin.** It is also the only route whose score knows the
  real bar numbers and the tempo marks, and it takes seconds instead of hours.
- **Route C works for secco recitative, and it timed an aria too** ("Voi che sapete", repeats
  written out). It needs the region in the track and the exact sung text, repeats included.
  It's the right tool where the music is sparse or the score is missing. It isn't the right
  tool for ensembles.
- **Route A is the fallback** for operas with no born-digital score. Scan quality decides most
  of its error.

## What is still manual

These are the steps that stop it from running unattended, roughly in order of effort:

1. **Reference text and gloss**, written per number. With route B the score's own Italian is
   already 99–100% right, so the reference is needed mainly for the **gloss**, the verse
   structure and per-singer variants. That changes the job: from transcription to
   "split the score's words into phrases and gloss them". An LLM pass can do that, with the
   checks in `scaling-notes.md`.
2. **Locating the score and cutting it into numbers.** For the Bärenreiter PDF, cutting can be
   automatic: number headings, bar numbers restarting at 1, and the "Recitativo" marks are
   real text.
3. **Per-number configuration in code:**
   - the cast and labels;
   - who is on stage per page;
   - the `RECORDINGS` table;
   - file names (`finale2*`, `sextet*`) and the phase scripts' Sextet assumptions.
4. **Finding the tracks, titles and regions.** Paths, titles, disc/track numbers, and where a
   recitative starts inside a track, are all given by hand today.
5. **Running the steps in the right order**, and re-running the every-line check.

What is already automatic and should stay so: alignment and its placement choice, tuning,
stems, cleaning against the reference, trimming, the checks, and the track split.

## Weak points found in the review

- **Shared interchange files are edited in place** by phase 8, phase 9, phase 10 and
  `finale2_build`. Order matters: phase 8 dropped phase 10's lines once. This has to change
  before a 30-number build.
- **Module-level state:** for example `P6.STEMS` is set from outside before calling helpers,
  and `BA = "--ba" in sys.argv` is read at import. That's fine for experiments and fragile in
  a batch.
- **Two copies of most logic:** the Sextet chain (phases 5–9) and the finale chain
  (`finale2_*`) do the same jobs with different code. The finale chain is the more general
  one: joined tracks, sections, line splitting.
- **Generic tempo table:** one bpm per tempo word. It was good enough for the finale. A whole
  opera will meet marks the table lacks ("Allegro con spirito", "Andante con moto"…), which
  fall back to 120 silently. They should be reported.
- **Dependencies:** `torchaudio` and `demucs` are installed but not in `pyproject.toml`.
  Audiveris is outside it altogether. With route B as the main path, Audiveris drops out of
  the Figaro build.
- **roon-rd matching:** a track missing from the file can match another track through the
  shared opera prefix (Currentzis t52 showed the Sextet). Strip the prefix before scoring.

## Build approach for step 6: all of Figaro

### Decision needed: route B as the main path for Figaro (recommended)

Decode the whole Bärenreiter PDF. Time every number with a score, sextet included, from it.
Keep route C for secco recitative, and keep route A only for operas without a born-digital
score.

| | For | Against |
|---|---|---|
| **B for everything with a score** (recommended) | best measured accuracy; real bar numbers (enables the bar-location feature); tempo from the score; no OMR hours; one code path | decoder tied to Finale with Maestro (fine for Bärenreiter); the Sextet and arias already timed get re-done |
| Keep what's built; B only for new numbers | no rework | two score sources and two bar numberings in one opera; Sextet stays at route A accuracy |

**Recitativo secco:** the Bärenreiter score contains the recitatives with notes and text, so
two options exist:
- **Route C (forced alignment):** proven on three recitatives. It needs the region start,
  which the score alignment of the neighbouring numbers can supply.
- **Route B score alignment:** untested on secco. Sparse continuo chords give the DTW little
  to hold on to, and singers take recitative freely.

Recommendation: route C. Feed it the score's own recitative text, so no transcription is
needed. Take its region from the track and the neighbouring numbers. Compare with route B on
one recitative as a side test.

### Proposed shape (for discussion before any code)

One Figaro build, in stages, each writing its own files under
`libretto-work/figaro/<stage>/`. Only the last stage writes the interchange files.

1. **`decode`:** `ba_vector` over the whole PDF.
   - Output: one `score.json` per number.
   - Numbers are cut at their printed headings. Each has its bars, notes, syllables,
     tempo, metre, and whether it is secco.
   - Check: printed bar numbers on every system; unknown tempo words and staff labels
     reported.
2. **`text`:** per number, the score's syllables become words and phrases per character.
   - An LLM pass splits them into verses and phrases and writes the literal gloss.
   - Checks:
     - the phrases rejoin into the score text;
     - one gloss per phrase;
     - every libretto speech is covered.
   - Output: `text.json` per number. This is what replaces the hand-written `*_text.py`.
3. **`lines`:** score-time clean lines per number, using today's `ba_finale2_score` and
   `clean` logic, plus `bar_start`/`bar_end` per line and phrase.
4. **`recording`:** from the audio tags, list the tracks, join them into one timeline per act,
   and run Demucs once per track. Output: a recording manifest with each track's offset,
   title, and Roon title.
5. **`place`:** find where each number sits in the act timeline.
   - Numbers are aligned in score order. Each one searches only after the previous
     number's end, which gives the many-to-many track mapping for free.
   - Secco regions are the gaps between them.
6. **`time`:** route B for numbers, route C for secco, then trim and run the checks.
   Output: per-number timed lines plus check figures.
7. **`publish`:** merge into one interchange file per recording, with Roon's titles. Then run
   the every-line check.
8. **`report`:** one HTML page per recording. It shows each number's checks, flags anything
   outside the bands seen so far, and links each number to the viewer. The entry figures must
   always show matched / predicted next to them. Today, entries more than 3 s off silently drop
   out of the median and the within-1 s figures. The measures are defined in §10.1 of the
   report.

Start with Solti, since it's a full studio recording. Then run Currentzis with no changes to
the code: that is the real test of "no per-recording work".

### Full orchestral scores: not needed

The vocal score covers what we need:
- the harmony and rhythm the alignment follows;
- the sung text and who sings it;
- the bars and tempo marks.

A full score would add instrumentation, but at a cost:
- many staves that thin out and merge from system to system;
- transposing instruments (Wagner's horns, trumpets and clarinets);
- a much larger decode.

All that buys little: the alignment already lands entries at a median of 0.07–0.10 s. A full
score becomes worth it in two cases:
1. **The vocal stem is too dirty under a Wagner or Strauss orchestra.** Knowing what the
   orchestra plays at each moment allows score-informed separation, or a check that discounts
   bleed from instruments in the singer's register.
2. **Bar and page references for the edition the user reads along with.** If that's a
   miniature full score, the bar location should point into it.

### Licensing in the build

- The decoded notes and syllables stay in `libretto-work` and are never committed.
- The interchange files carry only timings, Da Ponte's public-domain words and our own
  glosses. Bar numbers are facts about the work's structure in that edition. Label them with
  the edition ("BA 4565") rather than reproducing anything from its pages.

### Operating model: a new opera starts with a session

*Agreed 2026-11-08; details to settle.* A new opera or recording is built by the user starting
Claude Code in one workspace directory and saying "let's start a new opera" (or "add this
recording"). The workspace holds the inputs, the per-opera files and the code, or sits beside
the code.

The procedure lives in a **program**, the pipeline tool. Judgment lives in the **session**. The
session's context comes mainly from the tool's own state and reports, with a few short files on
top. Working together in sessions remains the expected way to handle what each new opera or
recording breaks.

#### The tool holds the procedure

The pipeline tool is one command-line program wrapping the eight stages. Its verbs:

| Verb | Does |
|---|---|
| `new <opera> --score <pdf>` | Checks the PDF (text layer, music font, engraver), creates the opera's folder and manifest, and runs `decode` |
| `add-recording <opera> <album>` | Finds the files by tag and Roon title, and records the recording in the manifest |
| `build <opera> [<rec>] [--number N]` | Runs whatever stages are out of date, for the whole opera or a single number |
| `status <opera>` | Shows where the opera stands: stages done, what's stale, open stops, the last verdicts |
| `report <opera> <rec>` | Writes the per-recording HTML report: checks against the bands, flags, links to the viewer |
| `regress` | Rebuilds the reference recordings (Solti first) and compares them against the last accepted results |

Properties the tool must have:

- **State on disk.** Each stage writes its outputs, plus a small record of its inputs and code
  version, under `libretto-work/<opera>/`. `build` redoes only what has changed. `status` reads
  the same records, so where an opera stands is never something a session has to remember.
- **Checks and bands in the tool.** "Is this good?" is answered by the report, not by
  judgment. Bands move only when listening supports it.
- **Clear stops.** When an input is outside what the tool knows, it stops with a specific
  diagnosis rather than guessing. Examples:
  - an unknown music font or engraver;
  - a tempo word or staff label not in its tables;
  - a number it cannot place;
  - checks outside the fail band.

  Stops are where a session takes over.
- **Fast troubleshooting.** It can rebuild one number for one recording on its own. Diagnostic
  output can be turned on per number: the timeline page and check figures. `regress` turns
  "Solti must come out no worse" into one command.
- **Grown, not designed ahead.** It starts as the stages plus state, reports and stops.
  Recurring decisions become automatic only after an opera or two shows what recurs. Until
  Così is done, most failures will be new and will change the code.

#### The session holds the judgment

These stay with the user and Claude working together:
- diagnosing a stop or a flagged number;
- deciding whether a fix is generic, writing it, and running `regress`;
- adding a font map for a new engraver;
- reading glosses;
- listening, and deciding when listening and the checks disagree;
- choosing editions.

#### What gives a fresh session its context

Claude's auto-memory is stored per starting directory, so memories made in the roon-rd sessions
will not load in a new workspace. The durable rules move into files:

1. **`CLAUDE.md` at the workspace root** (loaded automatically every session). It is short and
   holds:
   - what the project is, and the directory layout;
   - "start with `status`, then read the report";
   - the standing rules:
     - licensing: never share score PDFs or full decodes; one page is fine for study;
     - the audio library is read-only, and bulk output goes to `libretto-work`;
     - no hand-marking of timing, and don't revisit anchor snapping;
     - discussion before non-trivial changes; no version bumps;
     - "no additional work" per recording; any fix must be generic and pass `regress`;
   - pointers to this document, `repertoire-plan.md` and the report.
2. **`new-opera` and `add-recording` skills.** Short runbooks that name the tool's verbs in
   order, what a good result looks like at each step, and where to stop and ask the user (for
   example, to read a sample of the gloss).
3. **`operas/<opera>/STATUS.md`.** Records what the tool cannot: decisions made and why, the
   user's listening verdicts with dates, and open problems. The machine state lives in the
   tool's records.
4. **`.claude/settings.json` in the workspace.** Gives access to the extra directories (code,
   `libretto-work`, docs), write access to `libretto-work`, and a deny on writing under the
   audio library, so a new session has the same tool access without re-approving it.
5. **A cold-start test.** After setting this up, and again at each checkpoint in
   `repertoire-plan.md`, start a fresh session in the workspace and ask it how to begin a new
   opera and where Figaro stands. Any gap in its answer is a gap in the files or the tool.

#### Where and when

The likely workspace is the `libretto` repo, with the pipeline code moved in from
`libretto-scratch`. Scores and bulk data stay outside git in `libretto-work`, found through a
path in the config.

The tool is the step 6 restructure, given a driver, so it is built during the Figaro build. The
plan is:
- the from-scratch rebuild of the already-built passages is its first test;
- the full Figaro build is its second;
- Così is the first opera started the new way, from a fresh session.

### Decisions

Questions 1–6 were decided on 2026-11-11, all as recommended; question 7 on 2026-11-08.

1. **Route B is the main path for Figaro.** Every number is timed from the Bärenreiter decode,
   and the Sextet and "Voi che sapete" are re-timed from it. Route A stays only for operas
   without a born-digital score.
2. **Secco recitative by route C, with the score's text.** The recitative text and speakers
   come from the decode, and every timed word gets its bar. Route B gets a side test on one
   recitative.
3. **Verse list, phrase split and gloss, in a session.** The tool decodes the sung syllables.
   Claude turns them into each character's verse list (each line once, in order) and writes
   the phrase split and gloss in the same pass. The tool checks that every sung syllable
   maps to a verse and phrase. No published libretto is needed. (This replaces the earlier
   question about an LLM pass inside the tool; see 7.)
4. **A light restructure first:** per-number outputs, a single publish step, and stage verbs
   wrapping the existing scripts. Its first test is the from-scratch rebuild of the five
   passages.
5. **Bar locations go in now:** the number, `bar_start`/`bar_end` and the edition on every
   segment, with a new format version string. The viewer shows them later.
6. **The workspace is the `libretto` repo,** with the pipeline code copied in from
   `libretto-scratch`, and scores and bulk data in `libretto-work`. It is set up during the
   restructure. The preparation plan (P0–P8) is in `operas/figaro/STATUS.md`.
7. ~~The gloss inside the tool, or in a session?~~ **Decided 2026-11-08: in a session for
   now.** The tool stops after `text`, Claude writes the phrase split and gloss in a session,
   and the tool checks them.

   **Revisit:** once the glosses are trusted, discuss whether the tool should call the Claude
   API itself. That would make the step fully unattended, at the cost of an API key and a
   charge per opera. Raise this at the first checkpoint (after Così) at the latest.
