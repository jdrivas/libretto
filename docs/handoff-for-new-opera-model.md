# Handoff: building Figaro, as input for a repeatable new-opera model

*Written 2026-11-18 by the Claude session that built Figaro (2026-11-04 to 2026-11-17), for
a new session that will analyse `libretto` and `roon-rd` and propose a repeatable model for
adding operas. You don't have that conversation; this file is meant to replace it.*

**Sources.** The session transcript (about 260 user messages and 11 context summaries) was
mined for dates, quotes and who decided what. The pipeline and roon-rd code was read at
libretto `pipeline` 9a3c2f3 and roon-rd master e54ed2d (1.5.0). Line numbers below are as of
those commits. Where something is inferred rather than recorded, it says so. Dates are US
Pacific.

**Read alongside this file:**
- `docs/decisions.md` (D1–D25, the authority on decisions);
- `operas/figaro/STATUS.md` (every stage's numbers);
- `docs/how-figaro-was-built.md` (the short version of section 1);
- `docs/sung-text-pipeline.md` (the method, and the operating model agreed 2026-11-08);
- `docs/repertoire-plan.md`;
- `docs/todo.md`;
- `docs/sextet-experiment-plan.md` (the experiment's phase-by-phase record);
- `docs/scaling-notes.md` (an early list of expected problems).

`docs/pipeline.md` and `docs/annotation-schema.md` describe the February approach (the Rust
`crates/`) and are obsolete. `pipeline/README.md` is stale: it only covers `decode`.

## 0. Ground rules you inherit

These are the user's standing rules. They also sit in the repo's `CLAUDE.md`.

- **Licensed scores.** The Bärenreiter PDFs (and Schott later) are personal licensed copies.
  - Never share, upload or commit a score PDF or a full decode (`decode/pieces/*.score.json`).
  - One page or a small excerpt for study is fine. Crop the "Issued for" footer and the order ID.
  - Interchange files (timings, sung text, glosses) may be shared.
  - The user, 2026-11-08: "We shouldn't ever share the score files."
- **The audio library is read-only.** Don't write anything under `~/Dropbox/Audio`. The
  user: "I don't want you to modify those files … I don't want you to write in those
  directories."
- **No hand-marking.** Timings come from the pipeline. Never propose tapping along or
  marking downbeats; the user refused it on 2026-11-05. Don't re-propose anchor snapping
  without new evidence (see section 3).
- **Accuracy target.** "errors of 1 second or less in timing can be safely ignored"
  (2026-11-05).
- **Discussion first** for non-trivial decisions; no code until the user says go.
- **Within an agreed plan, go ahead** (D18): don't ask for OKs; stop only for a real choice or
  a result you can't explain.
- **Commits and versions.**
  - libretto commits go on the `pipeline` branch, which is pre-authorised.
  - roon-rd commits need the user's approval.
  - Never push unless asked. Never bump a version.
  - Stage files by explicit path.
- **Decision log.** Every decision gets an entry in `docs/decisions.md`, in the same commit
  as the change.
- **To-do list.** `docs/todo.md` is the only list of what's next (D25).
- **Every line accounted for.** After any change to timed lines or the viewer, check that
  no line is missing and none is shown twice.
- **"No additional work per recording"** (2026-11-08):
  - no per-recording settings;
  - fixes must be generic;
  - Solti must not get worse.

  D24's `keep_placed_end` and the Solti album alias are the two places this was bent; see
  section 2.

## 1. Timeline, 2026-11-04 to 2026-11-18

### Before: February 2026, dropped

The first tool (Rust, `crates/`) started from a published libretto (Murashev). It spread each
track's lines across the track by word count, using anchor lines. It was dropped for two
reasons:
- the timing was an estimate, not a measurement;
- a libretto is not what is sung: no repetitions, ensembles written out one line after
  another, and translations that follow the poetry.

### Experiment, Nov 4–8: about 4.5 days, the most back-and-forth

- **Nov 4.**
  - The user pivoted: the libretto is "a sort [of] literary distil[l]ation of what's sung".
    Take the text of the vocal parts from the score and show a very literal translation.
  - Claude reframed "get bar-line times from the audio" as audio-to-score DTW on chroma,
    not beat tracking, and wrote `sextet-experiment-plan.md`.
  - Claude could not download from IMSLP (it blocks scripts), so the user downloaded the
    files.
  - The Finale ZIP held only unusable `.mus` files, so the WIMA MIDI became the reference.
  - Phase 4 (MIDI-referenced alignment) passed. User: "pretty impressive … it rights
    itself pretty quick."
- **Nov 5: OMR (route A) failed as-is.**
  - Audiveris on the Ricordi scan found 144 bars against 140 real ones. The typeset scan
    drifted after bar 45.
  - Fixes: slope-constrained coarse DTW (it had collapsed onto the last 46 s), `stripTies`,
    horn transposition. Re-test: median 0.42 s, 94% within 1 s.
  - Lyrics needed Tesseract `ita` data. OCR labels split the score into 67 parts, so notes
    were taken from a non-OCR run and text from an OCR run, and a cast-order DP assigned
    characters.
  - The user set the rules above: no downbeat marking, ±1 s is fine, leave librettos
    behind, the printed score is the source of truth.
  - Claude produced a 31-item failure-mode analysis in chat only. It was never saved.
- **Nov 6: first viewing in roon-rd.**
  - "Things were pretty jumpy … By the end things were pretty off."
  - The user challenged MIDI as ground truth ("more like comparing two different
    recordings") and asked for vocal-stream analysis. That led to the Demucs htdemucs
    vocal stem, a loudness singing detector (pYIN confidence was useless, median 0.06),
    and a timeline page.
  - Anchor snapping was built and tested, but made things worse, and was switched off.
  - The real fix was timing lines from the scan-only map, plus viewer position smoothing
    and a 0.7 s lead. "Lines land quite well."
  - Ensemble cards went through several rounds. The user found a skipped final ensemble
    and a lost Susanna line. Claude had traced only the reported spot; **the
    every-line-accounted check was born here.** User: "It[']s just incredible. That was
    perfect."
  - Currentzis added: it showed Solti's file (same album name), so roon-rd matching by
    length was added.
- **Nov 7.**
  - Currentzis's Sextet drifted after about 3:40 (a 4 s silence at bar 94). The fix was to
    choose among placements by harmonic fit, then singing agreement.
  - Per-phrase highlighting, melisma extension, `FILL_MAX_WORDS` 3→4.
  - Silence trimming: the first version would have cut the Count's sotto voce by 2.6 s, so
    it became −30 dB for ≥1 s.
  - Recitative by forced alignment (**route C**) was born ("yeah. do B" in the user's
    words, Claude's option label).
  - "Voi che sapete": "This approach works far, far, better than what we tried a few months
    ago."
  - The Act II finale on the Ricordi scan needed a shared bar grid and an on-stage penalty.
  - The user offered a Bärenreiter vocal-score PDF.
- **Nov 8.**
  - The Bärenreiter PDF is born-digital vector, so it is decoded directly (**route B**)
    rather than run through OMR. Fixes: indented first system, stems mistaken for
    barlines.
  - Result: 940 bars (every printed number), 18,968 notes, 8,813 syllables.
  - Finale, Ricordi → Bärenreiter: singing agreement 70/74% → 83/88%; entries within 1 s
    62/74% → 91/95%; speeches missing 13 → 0 of 233. User: "near perfect".
  - The user asked for bar locations (D5) and agreed the repertoire plan.
  - Wrote `sung-text-pipeline.md`: the operating model and decisions 1–7.
  - A technical report artifact went through about 10 versions.
  - D7 (gloss in session) decided.
- **Nov 9.** Double-barline fix (found because the user asked for bar numbers in the
  report). Decisions listed with recommendations.
- **Nov 10.** No work.

### Preparation P0–P8, Nov 11–12: about 1.5 days

- **Nov 11.**
  - The user approved D1–D6: "I agree with all of these decisions."
  - About 25 minutes went on sandbox and permission settings for unattended runs.
  - P0–P2: the whole score decodes in about 12 s into 72 pieces, and all 95 contents
    entries are accounted for. Running on the whole score exposed decoder fixes:
    - the fixed 9.2 pt lyric size missed most of the Sextet: 813 → 2,489 syllables after
      making size proportional to staff spacing;
    - chorus systems, upbeats as bar 0, an ossia, stage directions, the page footer read as
      a lyric, "I" lost from "Io!".
  - Also on Nov 11:
    - a "ratio rule" that fixed No. 22 but broke four other pieces was replaced;
    - P3: Demucs on both full recordings, about 1 h on CPU (the Apple GPU fails), 2.7 GB;
    - P5: number-by-number placement put No. 15 inside the finale, so placement became one
      act at a time (D8);
    - the act-wide chroma shift came out 5 semitones off, so shifts were restricted to
      {0, 1, 11} for placement;
    - P6 and P7: texts for the five passages, bar locations, roon-rd's "track 52" rule.
- **Nov 12.**
  - P8, a from-scratch rebuild of the five experiment passages:
    - Solti's Sextet started 24 s early. A wider window made it worse (43 s), so edges are
      refined from the opening and closing 20 s (D9).
    - The route B vs C side test on "Eccovi" (D2).
  - The user saw Solti at 0/0 segments: Roon's album name differs, and length picked the
    empty overture. Fixed by the album alias and a 1 s length rule.
  - Listening passed. The user put the Sextet's wobble down to Roon's one-second clock;
    Claude corrected that it was partly the alignment (0.94 s median entries).

### Act I, Nov 12–13: about 2 days, the heaviest loop

- 19 texts written. Chorus shown once (D10).
- **Automatic catch.** A failing P8 regression test showed `refine_edges` had used a chroma
  shift of 7. The already-approved finale was timed 33 s late, with its first 40 s crammed
  together. Fixed (e1b2cef).
- **Found by the user listening:**
  - Solti's Nos. 2 and 3 were out of sync;
  - "Va là" showed the Act II finale, and the finale showed "La vendetta";
  - Currentzis's "Se vuol ballare" had no text.

  These led to two roon-rd matching rewrites in one evening. The key insight was that Roon
  *truncates* lengths, so a track fits L if it lasts L to L+1 s (D11, 5890dc3, e4ba2a0).
- **Solti No. 2.**
  - Its tempo swung 0.5×→1.9×→0.5×. Claude offered option A (a tempo-limited guide) or
    option B (vocal-onset features). The user said start with A.
  - A didn't help (68%→65%).
  - The real cause: each piece's own key estimate was a fifth or a fourth off (D12).
  - After the fix: No. 2 went from 68% and 1.4 s to 86% and 0.08 s, the Sextet's entries
    from 0.94 s to 0.11 s, and all 26 numbers to 78–94%.
- Both recordings were re-timed and republished at least three times.

### Acts II–IV, Nov 15–16: about a day each

- **Nov 15, Act II.**
  - The user corrected Claude's disc renumbering: Roon shows Solti's discs as 9–11 (D15).
  - "The solti recitatives come out fine," so option B was dropped (D14). The decision log
    started that day.
  - Act II texts. Speakers moved where the decode missed secco cues (D16).
  - Solti's No. 15 scored 71%, but the cause was the detector missing sotto voce singing.
  - The user moved to spot checks (D17) and asked to limit OKs (D18).
- **Nov 15–16, Act III.**
  - No. 23's chorus was given to the soloists, and Times-Roman staff labels were ignored
    (D19). The first layout rule regressed rec09a and was then fixed.
  - Built: 51 pieces, 2,691 segments.
  - User spot check: Solti's No. 21 "quite late", "Dove sono" a little late, Currentzis's
    No. 22 chorus late. Cause: the opening search couldn't follow paces 1.6–2.3× the
    printed tempo (D21).
  - The fix re-timed 4 Solti and 9 Currentzis pieces, some in Act I. Stretching the closing
    too was tried and dropped. Four Solti pieces were kept as built.
- **Nov 16, Act IV.**
  - The user found too many cards in No. 21 and deferred the ensemble card (D22). The user
    chose TUTTI (D23).
  - Act IV built. No. 24's refined end came 40 s early in both recordings (D24).
  - Result: 64 pieces, 3,327 lines per recording. "Everything looks good."
  - Wrote `how-figaro-was-built.md`.

### roon-rd, Nov 16–17

- Merged the sung-text viewer and track matching into roon-rd master.
- Built **libretto mode** on the main page for the user's real target: a Mac mini with a
  15.6" 1920×1080 touch display beside the listening chair.
- Released roon-rd 1.5.0 on Nov 17.
- Next and mute do nothing on one zone. This is pre-existing and not libretto-specific;
  it's in todo.md.
- D25 (one to-do list).

## 2. Decisions D1–D25

Who decided:
- **User**: the user made the call.
- **Claude→user**: Claude proposed and the user approved.
- **Claude**: decided alone and reported afterwards.

Scope:
- **General**: should hold for later operas.
- **Bärenreiter**: depends on this publisher's engraving.
- **Figaro**, **Solti**: specific fixes.
- **Process**: how we work.

| D | Decision | Reasoning | Who | Scope |
|---|---|---|---|---|
| 1 | Route B (decode the born-digital score, DTW to the recording) is the main path; route A (OMR) only for operas with no born-digital score | The decode gives notes, bars, syllables and tempo marks directly. The finale went from 13 to 0 missing speeches, and entries within 1 s from 62–74% to 91–95% | Claude→user (11-11). The user supplied the PDF idea on 11-07 | General, for born-digital scores |
| 2 | Secco recitative by route C (forced alignment of the words on the vocal stem), with the score's text | Side test on "Eccovi": route B's phrase starts were a median 0.72 s off route C's, 21 of 42 over 1 s. Secco has no steady tempo for DTW | Claude→user (11-11). Confirmed by Claude's test, no user verdict | General |
| 3 | Claude writes the verse list, phrase split and literal gloss in the session; the tool checks every sung syllable maps to them. No published libretto needed | The score's text is already 99–100% right, so the job is phrasing and glossing, not transcription | Claude→user (11-11), prompted by the user asking why not take the text from the score | General (Process) |
| 4 | A light restructure first: per-piece outputs, one publish step, stage verbs around the experiment scripts | Rebuild from scratch as the first test (P8) | Claude→user | Process, done |
| 5 | Bar locations (`piece`, `bar_start`/`bar_end`, `edition`) in every segment; format 1.1-score | The user wants to read along in the score | User asked (11-08); Claude→user for doing it now | General |
| 6 | The `libretto` repo holds code and texts; bulk data and scores in `libretto-work` (Dropbox-ignored) | Licensing, size | Claude→user. Dropbox-ignore: user | General. Revisit when the external disk arrives |
| 7 | Gloss written in the session, not by the tool calling the Claude API | No API key or cost; trust first | User (11-08): "for now … write it directly in a session, then we can discuss … (let's make sure to remember…)" | General. **Due at the Così checkpoint** |
| 8 | Placement per act: all pieces in score order, one DTW over the act's joined audio | Number-by-number placement put No. 15 inside the finale and threw off the rest of Act II | Claude (accepted implicitly) | General. Fragile for long acts and cut numbers (section 6) |
| 9 | Refine each placed piece's edges from its first and last 20 s of score | Placement started Solti's Sextet 24 s early | Claude | General |
| 10 | Chorus shown once, read from the top choir staff singing in each bar | One CORO card instead of the same words per staff | Claude→user; user confirmed 11-15: "One for all" | General, but assumes homophonic chorus text |
| 11 | roon-rd matching: album aliases; then exact title → exact album with a fitting length → closest length; a track fits L if it lasts L to L+1 s | Roon passes no disc number, truncates lengths, and its titles differ from the file tags | Claude, driven by the user's bug reports; each roon-rd commit user-approved | General. The alias entry is **Solti**-specific config |
| 12 | Chroma shifts limited to {0, 1, 11} in alignment and edge search | A piece's own key estimate over 12 shifts was a fourth or fifth off | Claude | General, assuming recordings within a semitone of written pitch |
| 13 | Tempo-limited guide (option A) left in code, off (`GUIDE = False`) | It didn't help; the cause was the key | User chose to try A; Claude left it off | General (dormant) |
| 14 | Vocal-stem onsets as alignment features (option B) not pursued | After D12, numbers were at 78–94%; Solti's recitatives sound right | Claude, after the user's "solti recitatives come out fine" | General (dormant) |
| 15 | Interchange keeps the files' disc numbers (Solti 9–11), superseding P7's renumbering | Roon shows 9–11 too; a renumbered file would never match a real disc number | User corrected Claude's wrong assumption | General |
| 16 | Where the decode misses a secco speaker cue, the text file gives the words to Da Ponte's speaker and notes it in `source` | Route C times from the text file's speeches, so the right card must light | Claude (never explicitly approved; no objection) | Rule general; the six or so applications Figaro. Needs a libretto to consult |
| 17 | User spot-checks each act; a full listening when the opera is built | Checks plus spot checks caught every real problem | User (11-15) | Process. Full listening now due |
| 18 | Within an agreed plan Claude proceeds; stops only for a real choice or an unexplained result | The user asked to limit OKs | User asked; Claude wrote the rule | Process |
| 19 | Naming a staff's singer: label (incl. Times-Roman "Il C."/"La C."), else a staff <0.92 of the system's largest is CORO, else carried by position while the staff count holds, else after a layout change the latest singer with that clef | No. 23's chorus went to soloists; No. 29 lost the Count and Countess. A per-piece staff map was rejected as hand work | Claude (inside a user-approved Act III plan) | **Bärenreiter** (fonts, sizes). Revisit for Schott |
| 20 | Orchestral recitatives timed by route B like numbers | They have real music to match. No. 18: 87%, 0.15 s | Claude→user (Act III plan); user called the Solti recitative "quite good" | General |
| 21 | A number's opening is searched for at the recording's pace (tempo ratio × 0.7–1.4) | No. 21 started 50 s late; fast numbers are sung at 1.6–2.3× the printed tempo | Claude, from the user's listening. Keeping four Solti pieces "as built" was also Claude's call | General. The four kept pieces are **Solti**-specific state (section 6) |
| 22 | Simultaneous singing stays as separate cards for now | The user found No. 21 (1:51–2:51) overlapping and suggested one ensemble card, later | User | Figaro-triggered; general once built. **Now** in todo.md |
| 23 | An all-singer passage on shared multi-name staves is one TUTTI card | Before, Susanna, Cherubino, Marcellina and Antonio had no card in No. 29's close | Claude→user (the user picked the recommended option) | General rule, Bärenreiter layout |
| 24 | `[timing] keep_placed_end` lists numbers whose end is not refined (No. 24) | Its refined end came 40 s early in both recordings; two generic guards were tried and failed | Claude | **Figaro**-specific config. Bends "generic fixes only" |
| 25 | `docs/todo.md` is the one list of what's next, roon-rd included | The plan had spread over five places | User asked; Claude proposed | Process |

## 3. Choices made without a log entry

### Kept, with reasons

- **The singing detector.** Loudness on the Demucs vocal stem, relative to the 99.5th
  percentile of both stems: `SING_DB = -24`, `SILENT_DB = -30` (`voice.py:14-15`).
  - pYIN is used for pitch only: its confidence is useless in ensembles.
  - This detector is the basis of every check. **It misses soft and light voices**: Solti's
    "Deh vieni" (62%), Solti's No. 15 opening (sotto voce), Solti's recitatives (10–32%
    "words in silence"), "Andiam" (50/72).
  - Several low scores were the detector, not the timing. Listening decides.
- **Silence trimming.** Lines are cut back only out of clear silences of ≥1 s at −30 dB
  (`trim_to_silences(min_len=0.3, lead_in=0.6)`). The first version used the "not singing"
  level and would have cut sotto voce passages.
- **Placement choice.** Among the coarse placement, "to music end" and "whole track",
  harmonic fit vetoes placements that spread the score over other music, and singing
  agreement decides. It must beat coarse by more than one point. Singing agreement alone
  picked wrongly for Solti (77.8% against 77.1%); harmonic fit caught it (0.719 against
  0.746).
- **Text clean-up** (`clean.py`). A letter-level DP matches decoded syllables to the
  session's verses.
  - Parameters: `MATCH, MISMATCH, OCR_JUNK, REF_SKIP, JUMP = 2, -1, -1, -1, 5`. The jump
    lets it return to a phrase start for repeats.
  - A word counts at ≥60% of its letters (`MIN_WORD_MATCH = 0.6`). Up to 4 dropped words
    are filled in (`FILL_MAX_WORDS`).
  - "sotto|sorta|voce" are stripped as performance directions.
  - The score decides what is sung, when and how often; the session's text fixes spelling
    and phrasing.
- **Lines.**
  - In `score_time.py`: a gap of 0.6 s or 20 syllables or end punctuation ends a line.
  - In `clean.py`: `SPLIT_GAP_S = 2.0` and `PHRASE_MAX_S = 4.0`, added after 20–30 s lines
    appeared.
  - Melisma: notes without a syllable extend the previous one.
- **Tempo from a word table** (`score_time.TEMPO_BPM`, Italian words to bpm). No tempo word
  means "allegro"; an **unknown word silently becomes 120 bpm**. The "nominal tempo" this
  gives is only a starting point: tempo ratios of 0.55–2.61 against it are normal (slow 6/8
  numbers, alla breve Presto).
- **Viewer timing.**
  - Roon reports whole seconds, so position is smoothed between reports (extrapolate up to
    2 s).
  - Lines show 0.7 s early (`?lead=`).
  - Ensemble cards: two voices overlapping 0.5 s start a passage; at most one voice for
    2.5 s ends it; a phrase stays lit 1.0 s after its end.
- **Text policy.**
  - Verses are written once in numbers and repeats are written out in recitatives.
  - The experiment's recitatives (Ricordi or libretto text, letter match as low as 75%) were
    rewritten from Bärenreiter.
  - Track titles are a check, never an input.
  - Appendix versions are decoded but not built (user, 11-11).
- **Piece ids.**
  - `noNN`; `noNN-recit` / `noNN-aria` for split numbers; `recNNa`… for the seccos after
    number NN; `app-…` for appendix pieces.
  - The user asked why the suffixes; they're intentional.
- **Demucs.** htdemucs, two stems, CPU (MPS fails). Weights copied into
  `libretto-work/.cache` and run offline (`HF_HUB_OFFLINE=1`). Stems stored as FLAC.
- **Every-line check.** Run against roon-rd's viewer in headless Chrome, with a mock
  websocket: does every published segment appear exactly once, in the list or in a card?
  **It is not committed code**; the harness lived in session scratch space. STATUS.md
  records only the results.

### Tried and dropped

- **Beat or tempo tracking.** Rejected on day one for direct audio-to-score DTW: drift,
  rubato, no beat in secco.
- **MIDI as the reference.** It passed, but the user pointed out that MIDI timing is like
  another performance. Replaced by the printed score.
- **Route A (Audiveris OMR on scans).** Wrong bar counts on Ricordi, OCR damage, labels
  splitting parts. Patched with a shared bar grid and a cast-order DP, then superseded by the
  vector decode. **Kept as the fallback for operas with no born-digital score.** It is
  archived and not in the pipeline package.
- **Anchor snapping** (`phase7_anchors.py`): entries after silence and register handovers
  used as anchors.
  - Leave-one-out: snapped median 0.44 s against 0.25 s plain.
  - The base alignment is already as precise as the anchors.
  - Behind `USE_SNAPPED_FOR_LINES = False` in the experiment code.
- **Hand-marked ground truth.** Proposed once ("30 minutes of tapping"); the user refused.
- **BS-RoFormer** separation, about +3–4 dB SDR on benchmarks. Held in reserve: Demucs was
  "already that good". It may matter for Strauss and Wagner (section 6).
- **Fixed lyric size** (9.2 pt ±0.25). Replaced by size relative to staff spacing (2.33×,
  2.49× on choir staves).
- **Merging barline clusters within 5 pt.** Reverted. Double barlines merge at 6 pt.
- **A No. 22 "ratio rule"** for lyrics. It broke four other pieces.
- **Number-by-number placement** (became D8). **A wider window** for a mis-started number:
  43 s off, so D9 instead.
- **Stretching the closing edge as well as the opening** (D21). No. 6 ended 24 s and 57 s
  late, because it matched the sparse recitative after the number.
- **Generic end guards for No. 24**: opening pace against span pace; closing match near the
  placed end. Neither separated right ends from wrong ones, hence D24.
- **Route B for secco** (D2) and **route C for orchestral recitatives** (D20, the fallback
  if No. 20 drifts).
- **roon-rd length tolerance of ±1 s.** Replaced by [L−0.01, L+1.0), because Roon
  truncates.
- **Disc renumbering from 1 in P7.** A wrong guess by Claude, reversed by D15.

## 4. The pipeline end to end

**Labels:**
- **A**: automated.
- **S**: semi-automated: the tool runs, and someone reviews or supplies something.
- **M**: manual.

**Paths:**
- Code: `pipeline/src/libretto_pipeline/`.
- Config: `pipeline/config.toml`, a single `work_root = "/Users/jdr/Dropbox/Development/libretto-work"`.
- Outputs: `<work_root>/<opera>/`.
- Install, once: `cd pipeline && uv sync --extra align --extra audio`, plus Demucs and MMS_FA
  weights copied under `work_root/.cache`. **M**. macOS arm64 only.

**Global CLI flags:** `--version`, `--log-level` and `--log-utc` (the user's standard; see
`~/.claude/CLAUDE.md`).

| # | Step and command | What it does | In → out | Mode |
|---|---|---|---|---|
| 0 | Write `operas/<id>/opera.toml` | Score PDF path, `edition`, `first_music_page`, `contents_pages`; per recording `id`, `sources` (folders), `match` (filename substring), `album_title`, `album_title_aliases`, artist and the rest; `[timing] keep_placed_end`; `[[cast]] name, labels[]` (order = `voice`). Figaro adds the pseudo-cast `CORO`, `TUTTI`, `DUE DONNE` | hand-written | **M** |
| 1 | `libretto-pipeline decode <opera>` | Reads the vector PDF with pymupdf (`decode/vector.py`): staves, barlines, noteheads (Maestro glyphs), Palatino lyrics, labels, cues. Splits it into pieces (`pieces.py`) and checks them against the table of contents (`contents.py`). About 12 s for Figaro | PDF → `decode/pieces/<id>.score.json` (**licensed**), `contents.json`, `manifest.json`, `manifest.md` (checks: contents not found, duplicates, problems) | **A**, review the manifest (**S**) |
| 2 | `libretto-pipeline add-recording <opera> [rec]` | ffprobe tags; act from the title (`act_of`: Act/Atto, Overture/Sinfonia) | FLAC folders → `recordings/<rec>/recording.json` (`tracks[{id:"d09t04", disc, track, title, act, duration_s, file}]`, `acts{}`) | **A** |
| 3 | `libretto-pipeline separate <opera> [rec] --device cpu` | Mono 22.05 kHz mix, plus Demucs htdemucs two-stem. About 1 h for both Figaro recordings; resumable | → `recordings/<rec>/mix/<track>.flac`, `stems/<track>/{vocals,no_vocals}.flac` | **A** (slow) |
| 4 | `libretto-pipeline score <opera> [piece]` | Bar lengths from metre and tempo word; each syllable's singer (labels, CORO by staff size, TUTTI, D10/D19/D23); lines per character; notes for DTW | → `score/<pid>.json` (`lines[{start,end,score_start,score_end,bar_start,bar_end,character,voice,text,sylls}]`, `notes`, `sections`, `unattributed`) | **A** |
| 5 | `libretto-pipeline place <opera> [rec] [--act N]` | Per act, joins tracks and pieces and runs one CENS-chroma DTW at 2 Hz, shifts {0,1,11} (D8). Flags `low fit` (<0.65), `tempo ratio` (outside 0.4–2.8), `titles disagree` | → `placement/<rec>.json` (`pieces[{id,start,end,fit,tempo_ratio,tracks,titles,flags}]`; seccos get a `region`) | **A**, review the flags (**S**) |
| 6 | `libretto-pipeline text <opera> <piece>` | Prints the decoded text by speaker in score order, and per character with repeats removed | → stdout Markdown | **A** (draft) |
| 7 | Claude writes `operas/<id>/text/<pid>.json` | `characters: {CHAR: [verse: [[it, en], …]]}` (each verse once, for route B and clean); `speeches: [{characters, phrases:[[it,en]…]}]` in sung order, repeats written out (needed for route C); `source` (provenance and D16 moves). Names must equal the cast names. The literal gloss is word-order English ("I not knew"). 64 files for Figaro | decode draft → text JSON | **M** (in session; D3, D7) |
| 8 | `libretto-pipeline clean <opera> [piece]` | The letter DP matches decoded syllables to the session's verses, assigns words and phrases, times phrases in score seconds, splits lines | → `clean/<pid>.json` (lines with `text`, `translation`, `gloss[]`, `verses{}`; `report{CHAR:{letter_match, verses_not_found}}`). Logs `lowest letter match`, `verses not found` | **A**, read the report and iterate the text to letter match 1.0 (**S**). **No hard threshold** |
| 9 | `libretto-pipeline time <opera> <rec> [piece]` | Route B (numbers, orchestral recitatives): `refine_edges` (D9, D21, D24), then `align.two_pass` MrMsDTW, `map_lines`, `trim_to_silences`, `voice.checks`. Route C (secco): MMS_FA forced alignment of `speeches` on the vocal stem, timing consecutive seccos in a region together; bars from `clean` | → `timed/<rec>/<pid>.json` (`lines`, `checks`: singing_agreement, entries matched/predicted, median offset, within_1s / words_mostly_silent, men_speeches_low) | **A**, review the checks (**S**) |
| 10 | `libretto-pipeline publish <opera> [rec]` | Merges every `timed/<rec>/*.json`; each line goes to the track its start falls in, with track-relative times; ends clamped to the track end | → `interchange/<rec>.interchange.json`, version `1.1-score` | **A** |
| — | `libretto-pipeline build <opera> [--recording R] [piece…]` | score (all pieces), clean, place (**all acts**), time, publish. Needs the decode | | **A**; see section 6 for its hazard |
| — | `libretto-pipeline status <opera>` | Manifest checks; tracks and separation progress per recording | | **A** |
| 11 | Point roon-rd at the folder and listen | `./target/debug/roon-rd server --port 3000 --libretto-dir /Users/jdr/Dropbox/Development/libretto-work/figaro/interchange`. Not recorded anywhere else: the committed `roon-rd/config.toml` still points at an old February file | | **M** |
| 12 | Every-line check, user listening | Headless Chrome against the viewer (not committed); the user's spot checks (D17) | | **M** |

**Regression set:** the P8 passages, `build figaro no12 rec12a no16 no19 rec19a rec19b`. Compare
them with `pipeline/experiment/compare_p8.py` and
`cd pipeline && .venv/bin/python -m pytest -q`. The tests:
- `test_basics.py`;
- `test_figaro_decode.py`: contents accounted for, passage bars and pages, No. 23's choir;
- `test_figaro_build.py`: the finale has 823 lines, median |Δ| < 0.05 s against the experiment.

The Figaro tests skip without the licensed PDF. There is no `regress` verb.

**Planned but not built** (`sung-text-pipeline.md`, "Operating model"):
- verbs: `new <opera> --score <pdf>` (with font and engraver checks), `report`, `regress`;
- per-stage input and version records, so `build` redoes only what's stale;
- "clear stops" on unknown fonts, tempo words or labels;
- `new-opera` and `add-recording` skills;
- a cold-start test.

None of these exist yet. This is the natural core of the model you're proposing.

### Interchange format and how roon-rd uses it

**The interchange file.**
- Top level: `{"version":"1.1-score","opera":{title,composer,language,translation_language,year},"edition", "recording":{album_title, album_title_aliases, artist, conductor, orchestra, year, label}, "tracks":[…]}`.
- Each track: `{track_id:"d9t7", title, disc_number, track_number, duration_seconds (ms precision), act, segments:[…]}`.
- Each segment: `{start, end, score_start, score_end, bar_start, bar_end, character, voice, text, translation, gloss:[{v:"CHAR:verse", c, it, en, start, end,…}], verses:{"CHAR:0":[[it,en],…]}, piece}`.
- Every track of the recording is listed, with empty `segments` where nothing is timed.

**Loading** (`roon-rd/src/libretto/library.rs`).
- `--libretto-dir` (or config / `ROON_RD__LIBRETTO_DIR`) loads every `*.interchange.json`
  recursively.
- `file_id` = the file stem.
- Files are indexed by normalised `album_title` plus its aliases.

**Matching** (`match_track_with_length`, `library.rs:256-381`; D11).
- Roon gives album (`line3`), title (`line1`) and length in whole seconds. **Disc and track
  numbers are always `None`** (`server/mod.rs:2113-2114`), so the numbered-track rule is dead
  code in live use.
- Order: exact title → in an exact-album file, the one track that fits the length → scored
  title fragments (a fitting track beats a better-scoring one) → closest length.
- A track fits L if `L − 0.01 ≤ duration < L + 1.0`.

**Delivery** (`server/mod.rs`).
- `send_libretto_updates` runs for every zone on connect and on every `zones_changed`. It
  sends `{"type":"libretto_update","zone_id","content":LibrettoContent|null}` only when the
  zone's file and track key changes.
- `LibrettoContent = {file_id, track_index, match_method, track_title, track_id, act, album, artist, opera_title, segments}`.
- Positions arrive in `zones_changed` and `seek_updated`.

**Display.**
- `src/server/web/libretto.js` is the shared `LibrettoView`: smoothing, 0.7 s lead, ensemble
  cards, translation aligned/line/off, ensemble stanza/rows.
- `libretto-mode.js` and `.css` are the main page's libretto mode: a book button on a zone
  card, split or full view, auto-return 3 s after there's no libretto unless the album is
  unchanged.
- `/libretto-view` is the older full page with debug details.

## 5. Where the time went, and what I'd do differently

### Where it went

| Phase | Days | What took the time |
|---|---|---|
| Experiment (Nov 4–8) | ~4.5 | Method discovery. The score source changed twice (MIDI → OMR → vector decode), so the finale was built twice. Five viewer rounds on the Sextet's ensemble cards. Two roon-rd matching bugs. Six context compactions |
| Docs and decisions (Nov 8–9) | ~1 | The report (about 10 versions), `sung-text-pipeline.md`, decisions 1–7 |
| P0–P8 (Nov 11–12) | ~1.5 | Decoder fixes exposed only by the whole score; sandbox and permission setup (~25 min); placement (D8); the P8 rebuild and D9 |
| Act I (Nov 12–13) | ~2 | roon-rd matching rewritten twice in an evening; the `refine_edges` shift bug in an approved piece; option A tried; D12; re-timing and republishing at least three times |
| Acts II, III, IV (Nov 15–16) | ~1 each | Texts (the bulk of each act's session time, inferred); D16 speaker moves; D19 decoder fix; D21 re-time of 13 pieces across acts; D24 |
| roon-rd libretto mode (Nov 16–17) | ~1 | Display work for the touch screen |

The trend is the encouraging part. Once the tools settled, an act took about a day: texts,
build, checks, a spot check and one new fix. Claude's own estimate on 11-08 was 1–2 hours of
review for Così or Don Giovanni, given Bärenreiter PDFs. That is an estimate, not measured.
**Così is the first chance to measure it.**

### Bugs, and who found them

**Found by the user listening:**
- the Sextet jumpiness;
- a lost ensemble and a lost Susanna line;
- the wrong recording's file (twice);
- Solti at 0/0;
- "Va là", the finale and "La vendetta" matched to the wrong tracks;
- "Se vuol ballare" without text;
- Solti's Nos. 2 and 3;
- the late starts in Nos. 21 and 22 and "Dove sono";
- too many cards in No. 21.

**Found automatically:**
- the lyric-size shortfall (whole-score decode);
- the `refine_edges` shift (a regression test);
- No. 23's decode (reading the text draft);
- No. 24's end, and the D21 side effect on No. 9 (comparing starts and ends);
- detector false alarms (the checks flag them, and listening clears them).

roon-rd **matching** produced more user-visible failures than timing did.

### What I'd do differently for the next opera

1. **Check roon-rd matching first, before any timing.**
   - Right after `add-recording`, publish an empty interchange (every track, no segments).
   - Play a track or two per disc in Roon and confirm the match method for every track.
     A small endpoint or log line listing Roon's album, title and length against the
     matched track would do.
   - Add `album_title_aliases` then.
   - Half the Act I rework was matching.
2. **Move Figaro specifics out of code into opera.toml**, and make the tool stop on
   anything it doesn't know. Specifically:
   - `timing.MALE` (`timing.py:142`), the register check's men's cast;
   - `decode/report.ABBR`;
   - `CHOIR_PARTS`;
   - an unknown tempo word (now a silent 120 bpm);
   - an unknown font or engraver (now silent garbage);
   - a staff label not in the cast.
3. **Commit the every-line check** as a test on the interchange (every timed line published
   once) and on the viewer's card logic. It is the regression test the user cares most
   about, and it currently isn't in either repo.
4. **Protect pieces that were deliberately kept as built.** A plain `build` re-places all
   acts and re-times everything. That would undo D21's four kept Solti pieces and cut No. 9
   short.
   - Per-stage records and staleness are the real fix.
   - Until then, pin kept pieces in opera.toml, as `keep_placed_end` does.
5. **Run the heavy, uniform steps on the whole opera first:** decode, separate, place. Then
   write texts and time act by act. This worked; keep it.
6. **Start with a regression set** of known-hard passages: an ensemble, a chorus, a secco,
   an aria, a finale. Rebuild it after every tool change. Run it as a single command.
7. **Spot checks from Act I** (D17), driven by a list the tool produces. The list should
   give each weak piece by its checks, each judgment call (D16 moves), and disc, track and
   time for each.
8. **Write texts in the tool or not, decided before Così starts (D7).** Texts are the
   largest remaining manual step. The glosses haven't been reviewed at scale.
9. **Decide in advance what "done" means for an opera.** For example: the full listening
   (D17), the ensemble display (D22), appendix pieces in or out.

## 6. Fragile spots, open questions, assumptions beyond Mozart

### Publisher and engraving: Bärenreiter-specific

- The decoder reads **Finale/Maestro glyph code points** (`vector.py:29-34`), **Palatino** at
  fixed sizes (bar numbers Palatino-Italic 9.0 pt, tempo Palatino-Bold, lyrics upright
  Palatino-Roman only, `vector.py:217-263`), the Times-Roman label exception (D19), the
  `BA \d{3,}` footer, and "every staff has a barline at every bar" (`vector.py:213`).
- **There is no check for an unknown font or engraver.** A different score would decode to
  nothing or to garbage, silently.
- `repertoire-plan.md` already warns that Bärenreiter scores were engraved over decades.
  Check each new PDF's fonts before relying on it, Così's included.
- Only the first lyric line under a staff is kept (`vector.py:271-273`), so strophic second
  verses are dropped.
- **Schott (Wagner): the claim that they are native vector PDFs is from Gemini and
  unverified.** The user will buy one to check.

### Language: Italian-specific

- Hard-coded Italian:
  - tempo words (`score_time.TEMPO_BPM`);
  - act words (`ATTO PRIMO…QUARTO`, four acts max, `pieces.py:21-25`);
  - "Recitativo", "istrumentato", "ed Aria", "Scena", "N°.";
  - "sotto|sorta|voce";
  - choir part names;
  - the `"ii "→"il "` fix;
  - the word class `[a-zà-ù]` (`config.py:29`: no ü, ß);
  - `act_of` (no Akt, Aufzug, Vorspiel, Prelude).
- The lyric reader *ignores italic text because Bärenreiter sets the German translation in
  italic* (`vector.py:14`). A German-language score breaks that assumption at the root.
- Route C uses MMS_FA, which is multilingual. Its performance on German and on Wagnerian
  singing is untested.

### Form: numbers-and-recitatives

- Pieces split at number headings and bar-number restarts, so **a through-composed Wagner
  act becomes one piece.** Consequences:
  - Placement's whole-act DTW and route B's MrMsDTW run on a 1–2.5 hour piece. The
    placement cost matrix is about 12k × 12k at 2 Hz for 100 minutes.
  - `clean.align_letters` allocates (n+1)×(m+1) arrays per character, which could reach
    gigabytes for a long role.
  - D9's 20 s edges, D21's pace search and the "region" between numbers all assume pieces
    of a few minutes.
- Route C and seccos' `region` assume numbers alternating with secco recitatives.
  Wagner has none.
- Only 8 seccos are allowed after a number (`'abcdefgh'`, `pieces.py:136`). Check this
  against Così.

### Versions and cuts

- **Appendix pieces are excluded from placement** (`place.py:74`). A recording that uses an
  alternative aria can't be placed.
- Placement forces **one path through every piece in score order.** A recording that cuts a
  number gets it squeezed somewhere, flagged only by its tempo ratio; seccos are never
  flagged.
- Don Giovanni (Prague/Vienna) is in the plan precisely to force detecting which numbers a
  recording contains.

### Recordings

- Inventory takes `*.flac` only, filtered by a filename substring, and needs a track tag.
  A track whose title names no act inherits the previous act. Acts are assumed to be
  consecutive tracks.
- Tracks that split mid-line lose the tail: publish clamps ends to the track end.
- **Pitch.** D12 allows only 0 or ±1 semitone. Period pitch (Currentzis about −35 cents) is
  handled by tuning estimation. A transposed aria or a recording more than a semitone off
  would fail.
- **The vocal-stem detector** behind every check misses soft and light singing (section 3).
  With Strauss's and Wagner's orchestras it will be worse. `repertoire-plan.md` names
  Elektra as "the worst case for the vocal stem". BS-RoFormer is the reserve option.
- The DTW tempo window is 1/3× to 3× nominal for placement and 0.5–2× for subsequence
  search. D21 showed fast Mozart already needs pace-adjusted search.

### Text and checks

- The clean report's letter match and "verses not found" are **logged, never enforced**.
  D16 moves also show as "verses not found", so intended moves and real gaps look alike.
- Route C's bar lookup keys on each character's speech rank (`timing.py:258`). It loses bars
  silently if the derived verse order differs from what clean saw.
- Chorus and TUTTI text come from the top staff only. That assumes all parts sing the same
  words, which fails for divided choruses or double choirs (Meistersinger's riot,
  Parsifal's Flower Maidens).
- D16 relies on Da Ponte's libretto to fix missed cues. For other operas the equivalent
  reference must be identified.

### roon-rd

- Matching rests on title and a 1 s length window. When the album doesn't match, every file
  is a candidate, and 2-word title fragments can match across operas. **As the library grows,
  the closest-length step is the risky one.** Passing Roon's disc and track numbers (if Roon
  provides them anywhere) would make it far safer.
- `opera.title` is published as `"Wolfgang Amadeus Mozart Le nozze di Figaro"`, composer and
  title concatenated.
- The committed `roon-rd/config.toml` points at an obsolete file. The working launch command
  is in section 4, step 11.
- Next and mute fail on the user's Bedroom dCS zone. This is pre-existing and in todo.md.

### Open questions for the model

1. What is the unit of review? An act worked for Figaro. Wagner's acts are single pieces.
2. Does the gloss move into the tool, via the Claude API (D7)? If so, what review does the
   user want, and at what cost per opera?
3. How does the tool record state, so a rebuild is safe and "kept as built" survives?
4. Where does per-opera configuration end and generic code begin? `keep_placed_end` and
   aliases are config. The cast's voice types and tempo vocabularies should probably be too.
5. What does a new publisher need, and how is it detected: a font map, size ratios, a
   footer pattern, label conventions?
6. How are versions, cuts and appendix pieces placed?
7. How does a fresh session get its context (the cold-start test in `sung-text-pipeline.md`)?
   This document is a stopgap for that.
8. Measured cost per opera: the repertoire plan's checkpoints (after Così, after the second
   Rheingold, after Rosenkavalier) depend on it, and nothing measures it yet.

## 7. State at handoff

- **libretto**:
  - Branch `pipeline`, local only: never pushed, not merged into `main`.
  - 64 text files in `operas/figaro/text/`.
  - Interchange files in `libretto-work/figaro/interchange/` (`georg-solti-1981`,
    `currentzis-2014`), 3,327 segments each.
- **roon-rd**: master at 1.5.0, pushed and released.
- **Next** (`docs/todo.md`):
  1. D22, the ensemble card;
  2. D17, the full listening;
  3. Figaro loose ends: app-no15, app-no28a, the titles check, Solti's Sextet drift;
  4. the roon-rd next/mute fix;
  5. the Così checkpoint: D7, D21/D24, then Così.
- **The user's goals:**
  - all three Da Ponte operas; most of Wagner, including a full Ring (Solti first); Strauss;
  - "a set of tools we can run in an entirely automated fashion", while expecting to work
    with Claude on each opera's new problems for some time;
  - the display is for listening at home without a staged performance, on the touch screen
    beside the chair.
