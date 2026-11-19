# A repeatable model for adding operas

*Written 2026-11-18, for discussion before any code. It builds on
`handoff-for-new-opera-model.md` (the history), `how-figaro-was-built.md`, `todo.md` and
`decisions.md` (D1–D25). Nothing here is decided yet. Where this document proposes a rule, it
becomes a decision only when the user agrees, and then it goes in `decisions.md`.*

**Sources.** libretto `pipeline` at ba5013d, roon-rd `libretto-number-match` at e4ba2a0
(master e54ed2d, 1.5.0), and the files on disk on 2026-11-18. Everything marked
**checked** was confirmed against code or files for this document. Everything marked
**inferred** was not.

**File formats.** [Appendix A](#appendix-a) describes every file this document refers to,
with its fields, time bases and quirks. The links below go to its entries.

**One correction to the handoff.** roon-rd's library code is `src/server/library.rs`, not
`src/libretto/library.rs`. The line numbers the handoff gives are right for that file.

## Summary

- **The method works and mostly generalises.** Of the 25 decisions, 15 should hold for any
  opera. Only one is pure Figaro configuration (D24). The real opera-specific risk is
  elsewhere: Figaro's cast, Italian words and Bärenreiter's fonts are written into the
  code (section 2.3), and nothing stops when a new opera doesn't fit them.
- **Figaro's rework was not in the texts.** The 64 text files were written once: only the
  five experiment passages were ever rewritten. The rework went into three things:
  - roon-rd track matching;
  - re-timing whole sets of pieces after a tool fix;
  - decoder fixes found only by decoding the whole score.

  Once the tools settled, an act took about a day.
- **The model** (section 3) does five things:
  - gives each opera one config file and a recording file per recording;
  - moves publisher and language knowledge into profiles;
  - puts eight checkpoints (G0–G7) in a fixed order, with track matching checked **before**
    any timing;
  - lets the tool keep state, so a rebuild can't undo accepted work;
  - defines when an opera is done.
- **Così would break today, on the first command,** in three ways found on disk (section
  4.1):
  - the "Così" in the Solti file names is stored in a different Unicode form, so the
    recording's filename filter selects no files;
  - Solti's titles say "Act 1", which the tool doesn't read, so every track lands in act 0;
  - Roon shows the same album name for Solti's Così and his Figaro.

  None of these is hard to fix. All three would have failed silently or confusingly.
- **Wagner breaks the model's assumptions**, not just its tables: it has no numbers, acts of
  1–2.5 hours, and a German score from a different publisher. Puccini would also need the OMR
  route back (section 4.2). Neither is needed before Così.
- **Before Così**, eight changes, about 6–8 session-days in all (section 5, P1–P8). The
  first four (about 2 days) are the minimum. Everything else waits for Don Giovanni or
  Wagner.

## 1. The pipeline today, end to end

### 1.1 Inputs

| Input | Where | Status for Figaro |
|---|---|---|
| Vocal score, born-digital vector PDF | `libretto-scratch/Barenreiter/` (licensed, never shared) | Bärenreiter BA 4565 |
| Recordings, FLAC | `~/Dropbox/Audio/...` (read-only) | Solti 1981 (32 tracks), Currentzis 2014 (83 tracks) |
| Libretto text | none: the sung text comes from the score (D3) | Da Ponte's libretto consulted only for missed speaker cues (D16) |
| Opera config | `operas/figaro/opera.toml` | hand-written: score paths, recordings, cast labels, one timing exception |
| Model weights | `libretto-work/.cache` (Demucs, MMS_FA) | copied once, run offline |

### 1.2 Steps

**A** = automated, **S** = the tool runs and someone reviews, **M** = manual.

| # | Step | Unit | Mode | Output (under `libretto-work/<opera>/` unless noted) |
|---|---|---|---|---|
| 0 | Write `opera.toml` | opera | M | `operas/<opera>/opera.toml` |
| 1 | `decode` | whole score, about 12 s | A, review the manifest (S) | [`decode/pieces/*.score.json`](#fmt-decode-pieces) (licensed), [`manifest.md`](#fmt-manifest) |
| 2 | `add-recording` | recording | A | [`recordings/<rec>/recording.json`](#fmt-recording) (tracks, act per track) |
| 3 | `separate` | recording, about 30 min each | A | [mono mix and Demucs stems](#fmt-audio) per track |
| 4 | `score` | piece | A | [`score/<piece>.json`](#fmt-score): lines, singers, notes, bars |
| 5 | `place` | act × recording | A, review the flags (S) | [`placement/<rec>.json`](#fmt-placement) |
| 6 | `text` (draft) | piece | A | decoded text by speaker, to stdout |
| 7 | Write the text file | piece | **M** (Claude in session: verses, phrases, gloss) | [`operas/<opera>/text/<piece>.json`](#fmt-text) (in git) |
| 8 | `clean` | piece | A, read the report (S) | [`clean/<piece>.json`](#fmt-clean) |
| 9 | `time` | piece × recording | A, review the checks (S) | [`timed/<rec>/<piece>.json`](#fmt-timed): route B (numbers, D20) or route C (secco, D2) |
| 10 | `publish` | recording | A | [`interchange/<rec>.interchange.json`](#fmt-interchange) |
| 11 | Every-line check | recording | **M**: a headless-Chrome harness in session scratch space, **not committed** | results in STATUS.md only |
| 12 | Listening | act (D17) | **M**: the user, in roon-rd | verdicts in STATUS.md |

`build` runs steps 4, 8, 5, 9 and 10 in that order. **Checked:** it re-places every act on
every run (`cli.py:84`), whatever pieces are named. `status` reports the decode and the
recordings only.

### 1.3 Where roon-rd consumes the output

- **Loading.** roon-rd is started with
  `--libretto-dir <work_root>/figaro/interchange`. It loads every
  [`*.interchange.json`](#fmt-interchange) under
  that directory, recursively, and indexes each file by its album title and aliases
  (`library.rs:137-235`). Each file's id is its file name, so two files with the same name
  would collide. **Checked:** the committed `config.toml` still points at a February file.
- **Matching.** Roon gives the album, the title and the length in whole seconds, never disc
  or track numbers. roon-rd tries, in order (D11, `library.rs:256-510`):
  1. an exact title;
  2. the one track that fits the length, in a file whose album matches exactly;
  3. title fragments, where a track that fits the length beats a better-scoring title;
  4. the closest length.
- **Delivery and display.** The server sends [`libretto_update`](#fmt-roon) per zone. `libretto.js` does
  the smoothing, the 0.7 s lead and the ensemble cards. Libretto mode runs on the touch
  display.

The interface between the two repos is just the interchange file and its album names. That
is a good boundary: nothing in roon-rd knows about scores, pieces or routes.

## 2. What Figaro cost

### 2.1 Time

From the handoff (calendar days, Nov 4–17, inferred from the transcript), and the commit
record (**checked**):

| Phase | Days | Kind of work |
|---|---|---|
| Experiment | 4.5 | Method discovery (MIDI → OMR → vector decode). One-off |
| Report and decisions | 1 | One-off |
| Preparation P0–P8 | 1.5 | Tool building, whole-score decode, separation. Mostly one-off |
| Act I | 2 | Build, plus two roon-rd matching rewrites, plus D12 |
| Acts II, III, IV | 1 each | Texts, build, one new fix per act |
| roon-rd libretto mode | 1 | Display, one-off |

That is about 13 days, of which about 7 were one-off method and tool work, 5 were the opera,
and 1 was the display. **The figure to beat for Così is the per-act day.** No one measured
hours, or the user's own time. That is the gap the cost log in section 3.6 fills.

The libretto `pipeline` branch has 36 commits since Nov 8 (**checked**). The files changed
most often were:
- the text files, 69 file-commits;
- STATUS.md, 19;
- decisions.md, 12;
- `timing.py`, 7;
- `score_time.py` and `cli.py`, 5 each;
- `decode/vector.py`, 4.

### 2.2 Where the rework was

| Area | Evidence (checked unless marked) | Rework |
|---|---|---|
| **roon-rd matching** | 6 of the 15 roon-rd commits since Nov 6 (60b08d2, c43ef00, 1ac0d7d, d486bb7, 5890dc3, e4ba2a0). At least 5 of about 11 user-found bugs (handoff section 5) | The most user-visible. Every bug was found by listening, after timing was done. "Half the Act I rework" (handoff) |
| **Re-timing after tool fixes** | Three whole re-time passes (`retime-shift.log`, `retime-edges.log`, `retime-start.log`); Act I re-timed and republished at least three times; D21 re-timed 13 pieces across Acts I and III | High machine cost, moderate session cost. It left state behind (D21's four pieces kept as built) that a plain `build` would undo |
| **Decoder** | `vector.py` is the most rewritten file (497 lines changed in a 461-line file); 6 fixes at P2, 3 at P6, D19 in Act III | Found by decoding the whole score and by reading text drafts. It cost little once the whole score was decoded first |
| **Texts** | 64 files. Only 5 were ever changed twice, all experiment passages rewritten from Bärenreiter at P6. **No text was reworked once the act loop began** | Zero rework, but it is the largest share of each act's session time (inferred) |
| **Reports and docs** | Report about 10 versions; this handoff | One-off |

**Conclusion.** The steps that ran once and well (decode, separate, texts) are fine as they
are. The cost came from two things, and the model is built around them:
- a check that ran too late (track matching);
- outputs that had no record of what made them (re-timing).

### 2.3 Opera-specific fixes and general rules

**Logged decisions (D1–D25):**

| Class | Decisions | Count |
|---|---|---|
| General: should hold for any opera | D1, D2, D3, D5, D6, D8, D9, D10, D12, D13, D14, D15, D20, D21 (the rule), D22 | 15 |
| General rule, but needs per-opera input | D11 (album aliases), D16 (needs a reference libretto) | 2 |
| Publisher-specific (Bärenreiter layout) | D19, D23 | 2 |
| Opera- or recording-specific | D24 (`keep_placed_end = no24`); D21's four Solti pieces kept as built (state, not config); D11's Solti alias | 1 config, 2 attachments |
| Process | D4, D7, D17, D18, D25 | 5 |

**Not logged, and more dangerous:** Figaro and Italian written into the code. **Checked,
2026-11-18:**

| Hard-coded | Where | What happens for another opera |
|---|---|---|
| Figaro's men, `MALE = {"IL CONTE", "FIGARO", …}` | `timing.py:142` | Così's men count as women in the route C check, silently |
| Figaro cast abbreviations, `ABBR` | `decode/report.py:5-8` | Cosmetic: the manifest shows full names |
| Italian choir part names and "Due Donne", `CHOIR_PARTS` | `decode/vector.py:56-57` | A German part name is read as a sung word |
| Italian tempo words; an unknown word becomes 120 bpm | `score_time.py:16-29` | A wrong nominal tempo, silently. Placement and D21 lean on it |
| Act words `ATTO PRIMO…QUARTO`, four acts at most | `decode/pieces.py:23`, `contents.py:11` | Fails for German, or for more than four acts |
| Act from a track title: `Act I–IV`, `Atto …`, `Overture/Sinfonia` | `recording.py:30-37` | "Act 1" and "Scene I" aren't read, so every track inherits act 0 (Così, Rheingold: section 4) |
| `sotto|sorta|voce` stripped; `"ii "→"il "`; letters `[a-zà-ù]` | `clean.py:16`, `config.py:26-29` | No ü or ß; Italian-only clean-up |
| Bärenreiter fonts and sizes (Palatino, Maestro glyphs, Times-Roman labels, `BA \d{3,}` footer) | `decode/vector.py` | Another engraver decodes to nothing or to garbage, with no warning |
| At most 8 seccos after a number (`'abcdefgh'`) | `decode/pieces.py:136` | An IndexError: at least it stops |
| Appendix pieces never placed | `place.py:74`, `stages.py:24` | An alternative aria in a recording can't be placed |

The rule "fixes must be generic" held for the logged decisions. The hard-coded tables are
the main thing that would have made a second opera expensive.

## 3. The model

### 3.1 Principles

1. **The tool stops instead of guessing.** Any input outside its tables stops it with a
   diagnosis: an unknown font, tempo word, label, act word or track title pattern. Stops are
   where a session takes over (as agreed in `sung-text-pipeline.md`, "Operating model").
2. **Check cheap, user-visible things first.** Matching costs the user minutes before timing,
   and hours after it.
3. **Every output records what made it,** and accepted work is protected. A rebuild redoes
   only what is stale, and never silently replaces something the user accepted.
4. **Knowledge lives in data, not code.** Publisher and language go in profiles, the opera's
   cast and form go in its config, and a recording's facts go in its own file.
5. **Unchanged:**
   - the heavy steps run on the whole opera first;
   - texts and timing go act by act;
   - a spot check per act (D17), with a full listening at the end;
   - Claude goes ahead within an agreed plan and stops only for a real choice (D18).

### 3.2 Folder layout

**Why the work is split across directories (D6, 2026-11-11).** The rule: what is authored
goes in git, and what is generated goes outside it.

- **`libretto/`** (`~/Dropbox/Development/rust/libretto`) is the git repo. It holds
  everything authored and small: the code, each opera's config, the session-written texts,
  STATUS.md and the docs. These need history, and some can't be recreated: the texts are
  hand work.
- **`libretto-work/`** (`~/Dropbox/Development/libretto-work`, one level above `rust/`) is
  not in git and is excluded from Dropbox sync. It holds everything the pipeline generates:
  decodes, stems, placements, timings and interchange files. Figaro's folder is 2.8 GB.
  It's outside the repo for three reasons:
  1. **Licensing.** The decodes are the score in machine-readable form. Kept outside the
     repo, a careless `git add` or push can't publish them.
  2. **Size.** The stems alone are 2.2 GB, too big for git, and not worth syncing through
     Dropbox.
  3. **It can be rebuilt** from the score, the audio and the repo, so losing it costs
     machine time, not work.

  The pipeline finds it through `work_root` in `pipeline/config.toml`. It is given as the
  real path, because `~/Development` is a symlink into Dropbox and the sandbox matches only
  real paths.
- **`libretto-scratch/`** holds the experiment-era code and the licensed score PDFs
  (`Barenreiter/`). `opera.toml` points at the PDF there.

**The weak spot: "it can be rebuilt" isn't quite true.** Some of `libretto-work` is not
reproducible:
- the pieces deliberately kept as built (D21's four Solti pieces);
- the interchange files the user has listened to and accepted.

A full rebuild would give different results, and nothing outside the folder records them.
Since the folder isn't synced, it also has **no backup** today. The `accepted/` records
below (P4) fix the first half, by keeping accepted results as data with their checks. The
backup question belongs with D6's revisit, when the external disk arrives (`todo.md`,
"Later").

The proposed layout:

```
libretto/                                    git, pipeline branch
  operas/<opera>/
    opera.toml              identity, score, form, cast, regression set
    recordings/<rec>.toml   one per recording: sources, match, album names, store, year
    text/<piece>.json       verses, phrases, gloss (session-written, D3/D7)
    STATUS.md               human record: verdicts, judgment calls, cost log
  profiles/
    publisher/baerenreiter.toml   fonts, sizes, footer pattern, staff-size ratios, label fonts
    language/it.toml              tempo words, act/scene words, directions to strip,
                                  choir part names, letters, heading words ("N°", "Recitativo")
  pipeline/                 the tool
  docs/                     method, decisions, todo, this model

libretto-work/                               not in git; licensed and bulky
  <opera>/
    decode/ score/ clean/ placement/ timed/ recordings/   as now
    state/<stage>/<unit>.json    inputs' hashes + tool version for each output
    accepted/<rec>.json          accepted pieces: their output hash, checks, date, who
    report/<rec>.md              checks, flags, spot-check list
  published/<opera>--<rec>.interchange.json   the one directory roon-rd reads
```

The changes from today:
- **Recordings get [their own files](#fmt-proposed).** Adding a recording then never edits
  `opera.toml`.
- **Profiles** take the tables now in the code (section 2.3).
- **[`state/` and `accepted/`](#fmt-proposed)** replace "kept as built" living only in
  STATUS.md and decisions.md.
- **[One `published/` directory](#fmt-proposed)**, with the opera in each file's name. roon-rd then reads
  every opera from one path, and two operas can't produce the same file id. Today's
  `figaro/interchange/` path would hold only Figaro.

### 3.3 The opera config

Figaro's `opera.toml` would gain about ten lines. A Così sketch, with the new fields marked:

```toml
[opera]
title = "Così fan tutte"
composer = "Wolfgang Amadeus Mozart"
librettist = "Lorenzo Da Ponte"
language = "it"                      # also selects profiles/language/it.toml   (new use)
translation_language = "en"

[score]
edition = "Bärenreiter BA …"
publisher = "baerenreiter"           # profiles/publisher/; `new` checks the PDF's fonts against it  (new)
pdf = "…"
first_music_page = …
contents_pages = […, …]

[form]                               # (new)
kind = "numbers"                     # numbers | continuous (Wagner)
placement_span = "act"               # what one placement DTW covers
review_unit = "act"                  # what one spot check covers
appendix = "exclude"                 # exclude | detect (Don Giovanni)

[reference]                          # (new) what D16 consults for a missed speaker cue
libretto = "Da Ponte, 1790"

[regression]                         # (new) the known-hard passages, rebuilt after any tool change
pieces = ["no13", "no18", "no14", "no08", "rec…"]   # sextet, Act I finale, an aria, a chorus, a secco

[timing]
keep_placed_end = []

[[cast]]
name = "FIORDILIGI"
voice = "soprano"                    # (new) replaces timing.MALE
short = "Fior"                       # (new) replaces decode/report.ABBR
labels = ["fiordiligi", "fior"]
# … DORABELLA, GUGLIELMO, FERRANDO, DESPINA, DON ALFONSO …

[[cast]]
name = "CORO"
kind = "chorus"                      # (new) solo (default) | chorus | tutti: replaces the pseudo-cast convention
labels = ["coro"]
```

A recording file, [`recordings/georg-solti-1974.toml`](#fmt-proposed), would hold what `[[recording]]` holds
today: `album_title`, `album_title_aliases`, `artist`, `sources`, `match`, `year`, `label`,
`store`. The year is to be checked.

### 3.4 The checklist, with checkpoints

Each checkpoint (G) is a point where the tool reports and someone decides. **Stop** means
the tool refuses to go on until it's resolved. The times are rough targets for a Mozart
opera.

| Step | What | Who | Checkpoint | Typical time |
|---|---|---|---|---|
| 0 | Choose the score and recordings; the user buys or locates them | user | — | — |
| 1 | `new <opera> --score <pdf> --publisher P`: font and engraver check, scaffold `operas/<opera>/` from a template, STATUS.md with a cost log | A | **G0 profile:** unknown fonts or glyphs stop it. A new publisher means a session writes a profile | minutes; a new publisher, a day or more |
| 2 | Write the cast and form in `opera.toml`, from the score's cast list | Claude | — | 15 min |
| 3 | `decode` the whole score | A | **G1 decode:** every contents entry found, bar counts right, no unknown labels, tempo words or act words. Claude reviews `manifest.md` | 30 min |
| 4 | Write `recordings/<rec>.toml`; `add-recording` | Claude, A | **G2 inventory:** files found; every act in the score has tracks; act durations plausible; titles parsed. Otherwise stop | 15 min |
| 5 | Publish an **empty** [interchange](#fmt-interchange) (every track, no lines). The user plays the first track of each disc and a few others. roon-rd lists, for each, Roon's album, title and length against the matched file, track and method | user, 10 min | **G3 matching:** every played track matches the right file and track by an expected method. Add aliases now | 20 min |
| 6 | `separate` (overnight if need be) and `place`, whole opera | A | **G4 placement:** flags (low fit, tempo ratio, titles disagree, squeezed or missing pieces) explained or fixed | 30 min |
| 7 | **Pilot:** texts, then build the regression set (an ensemble, a chorus, a secco, an aria, a finale) | Claude | **G5 pilot listening:** the user listens to the five passages in both recordings (as P8 did); accepted pieces are recorded | user 30–45 min |
| 8 | **Act loop:** texts for the act; `clean` (stop unless letter match is 1.0, or each gap is listed as a D16 move); `build --act N`; the committed every-line test | Claude, A | — | about a day per Figaro-sized act |
| 9 | [`report`](#fmt-proposed): checks per piece, flags, and a spot-check list (the weakest pieces, D16 moves, a few numbers, each with disc, track and time) | A | **G6 act spot check (D17):** the user listens to the list; problems become generic fixes plus `regress` | user 20–40 min per act |
| 10 | Close the opera | user, Claude | **G7 done:** see below | — |

**Why G3 comes before any timing.** Half of Act I's rework was matching, found by
listening after timing. An empty interchange catches the album and length problems in ten
minutes. It costs roon-rd a small "what did I match" view (section 5, P7).

**What "done" means (G7).** Agree it now so it isn't open-ended. Proposal:
- every main piece is published in every recording, and the committed every-line test
  passes;
- the regression set is accepted;
- each act has passed its spot check;
- a full listening (D17), or a part of it the user names, has passed;
- open items are moved to `todo.md` under the opera, each with a reason;
- the cost log is filled in.

Appendix pieces are out unless a recording uses one.

### 3.5 Tooling or manual

| Becomes tooling | Stays with the session or the user |
|---|---|
| Font and engraver check; profile lookup (G0) | Writing a new publisher or language profile |
| Stops on unknown labels, tempo words, act words, title patterns | Deciding what an unknown word or label means |
| Inventory checks (G2); matching report (G3, roon-rd side) | Playing the tracks for G3 (user) |
| Per-stage state; `build` redoes only what's stale; `accept`; `regress` | Deciding a fix is generic |
| Every-line test (interchange level, and viewer card logic) | — |
| `report` with the spot-check list | Listening (user); judging when the checks and the ear disagree |
| `clean` gate: letter match 1.0 or a listed reason | Writing verses, phrases and gloss (D3/D7, see below); D16 speaker moves |
| Cost log entries from `status` (stage times) | The user's time, entered by hand in STATUS.md |

**D7, the gloss in the tool, is due at the Così checkpoint.** My recommendation: **keep it in
the session for Così.** Così is meant to measure "nothing new"; changing how texts are made
at the same time would confuse the measurement. Log the session time spent on texts, act by
act, and decide D7 after Così with that number. Wagner is where the volume of text (and
German glossing) makes the API route most attractive. This is the user's decision.

### 3.6 Measuring cost

Nothing measures cost today, and the repertoire plan's checkpoints depend on it. Proposal:
STATUS.md gets a cost table, one row per session:

| Date | Phase (G-step or act) | Session hours | User minutes (listening, choices) | New problems | Tool fixes |
|---|---|---|---|---|---|

Claude fills in the session columns, and the user corrects the user column. `status` adds
machine times. After Così, compare it with Figaro's per-act day.

### 3.7 A new recording of an existing opera

A new recording needs none of the per-opera work. The decode, cast, texts and clean outputs
are reused unchanged.

| Step | New opera | New recording |
|---|---|---|
| G0–G1 profile, decode | yes | no |
| Cast, form, texts, clean | yes | no |
| `recordings/<rec>.toml`, add-recording, G2 | yes | yes |
| G3 matching | yes | yes. **More important:** a second recording of the same opera is exactly where Figaro's matching went wrong |
| separate, place, G4 | yes | yes. Cuts and versions are per recording, so G4 is where a new recording shows its differences |
| Pilot (G5) | yes | no: the regression set is rebuilt for the new recording as part of `regress` |
| Time and publish | act by act | all at once |
| Spot check | per act (G6) | **one** list for the whole recording |

Expected cost: machine time, about 30 minutes of separation, plus about 1–2 session hours and
30–60 user minutes. Anything more is a finding. **"No additional work per recording"** then
becomes measurable: a new recording that needs a fix gets a generic fix, and `regress` shows
that every other recording is no worse.

What tends to differ per recording (from Figaro, and expected for Don Giovanni):
- album names (aliases);
- track splits (a line split by a track boundary loses its tail);
- pitch (D12);
- cut seccos or numbers;
- appendix arias;
- the singing detector's response to particular voices.

## 4. Stress tests

### 4.1 Così fan tutte, next

What exists on disk (**checked**, 2026-11-18):
- **Recording:** Solti, CDs 1–3 of the same "Solti – Mozart – The Operas" box as his Figaro.
  It has 43 tracks: overture 4.3 min, Act I 87.1 min, Act II 92.0 min, 183 min in all, so
  apparently uncut.
- **No second Così recording** is in the library (only overtures).
- **No Così score PDF** is in `libretto-scratch/Barenreiter/` yet.

Walking the checklist:

| Step | What happens today | Severity | Model's answer |
|---|---|---|---|
| G0 | No font check. If Così's Bärenreiter PDF was engraved with different software (`repertoire-plan.md` warns of this), the decode is silently empty or garbage | High, unknown until the PDF is in hand | `new` checks fonts against the profile |
| G1 | Tempo words not in the table become 120 bpm silently. Labels for disguises (Despina as doctor and notary; the Albanians) might differ from the cast labels (inferred: not seen yet) | Medium | Stops on unknown words and labels |
| G2 | **`match = "Così fan tutte"` selects no files.** The file names store "ì" decomposed (NFD); a TOML string is composed (NFC), and `in` compares them byte for byte (**checked**) | High: a confusing "0 tracks" | Normalise both to NFC; stop on zero files |
| G2 | **Every track lands in act 0.** The title tags say "Così fan tutte / Act 1 …", and `act_of` reads only Roman `Act I–IV`, so each track inherits the overture's act 0 (**checked** against the tags). Placement per act then has no tracks for Acts I and II | High | Read Arabic numerals and profile words; stop if a score act has no tracks |
| G3 | **Two operas share an album name.** Roon's "Solti: The Operas - Mozart" would be an alias of both Solti files. Where Roon's title doesn't equal the file's tag (true for this box), each file offers its track that fits the length. If a Figaro track fits too, the closest length wins, which is close to a coin toss. Rough estimate: about a 2% chance per track of a Figaro track within the same second, so a wrong-opera match or two in a 43-track opera is plausible (inferred) | Medium-high, user-visible | G3 catches it. Fix: among files that fit equally, prefer the one whose opera title appears in Roon's track title (Roon's titles include the opera name). Passing a disc number would be better, if Roon has one anywhere |
| G3 | roon-rd is launched on `figaro/interchange`, so Così's file wouldn't load. File ids are bare recording ids, so `georg-solti-…` could collide | Low, but certain | The [`published/` directory](#fmt-proposed) and `<opera>--<rec>` names |
| G4 | **Acts twice as long.** Figaro's longest act was 46 min; Così's are 87 and 92. Placement's full DTW at 2 Hz grows about 4× (about 11k × 11k: roughly 1 GB per matrix, perhaps 2–3 GB in all, against about 0.5 GB for Figaro, inferred from `librosa.sequence.dtw`). One wrong local match also has more to throw off (D8's weakness) | Medium | Measure at G4; G4's flags. Splitting an act at a secure point is the fallback |
| G4 | Cut seccos are never flagged; placement forces every piece in. Solti looks complete, so it is not expected to bite here | Low for Solti | Flag pieces squeezed below a minimum pace (also needed for Don Giovanni) |
| G5–G6 | **`MALE` is Figaro's men.** Guglielmo, Ferrando and Don Alfonso would count as women in route C's check | Medium, silent | Cast `voice` |
| G5–G6 | Così is ensemble-heavy (trios, the quintet, the sextet, two long finales), so D22 (the ensemble card) will matter more than in Figaro | Medium, display only | Finish D22 before Così's listening |
| Texts | About as many pieces as Figaro (inferred), Italian, Da Ponte. The same method should carry over unchanged | — | Measure it (section 3.6) |
| Rebuild | Not a Così problem, but a `regress` or a fix applied across operas would re-place and re-time Figaro, undoing D21's four kept pieces and No. 9 | High for Figaro | `state/` and `accepted/` first |
| Second recording | None, so Così can't test "no extra work per recording" | Planning | Buy one, or accept that Così measures only the per-opera cost |

**Verdict.** With P1–P4 from section 5 done (about 2 days), Così should be "nothing new",
apart from the risks of a new PDF and of acts twice as long, which G0 and G4 are there to
catch.

### 4.2 Beyond Mozart

**Wagner (Das Rheingold, Solti, next in the plan).** What's on disk: 34 tracks titled
"Vorspiel" and "Scene I"–"Scene IV". There is no act at all: it is one span of about 150
minutes (**checked** titles; length inferred).

| Assumption | What breaks | What the model needs |
|---|---|---|
| A born-digital Bärenreiter score | Schott vector PDFs are unverified (Gemini's claim). A scan means route A, which is archived and not in the package | G0 decides. A Schott profile if it's vector, route A brought back if not |
| Italian lyrics, upright; the German translation in italic, ignored | A German score inverts the root assumption of the lyric reader | The language profile says which style is sung text |
| Pieces = numbers and seccos | One piece per act or scene, of 1–2.5 hours | `form.kind = "continuous"`. Placement covers a span; **text, timing and review use units of about 5–10 min** cut at scene headings and then at system or page breaks. Section ids like `sc2-p041` instead of `noNN` |
| An act per track title | "Scene I" isn't an act; Rheingold has none | `form.placement_span` defined in opera.toml, with the tracks assigned by title pattern or by order |
| Full-matrix DTW at 2 Hz | 150 min is about 18k × 18k: several GB per matrix | Multiscale or banded DTW for placement (the MrMsDTW route B already uses) |
| `clean` letter DP per character over a piece | Wotan's role over a whole act, at (n+1)(m+1), could reach gigabytes | Fine if the units are 5–10 min |
| D9/D21: a number's 20 s edges | Units are contiguous, so their edges come from the span's alignment | Edge refinement only at span ends |
| Route C for secco | No secco | Route B only: simpler |
| Chorus from the top staff (D10) | Holds for Rheingold (no chorus). Fails for divided choruses (Meistersinger's riot, Parsifal's Flower Maidens) | A later decision |
| Vocal-stem detector | A heavy orchestra will mask voices, so the checks will be weaker | BS-RoFormer in reserve; the bands may need to differ per opera |
| Texts and gloss in session | Many more words, and German→English literal gloss | D7 is most likely to change here |

**Puccini (sung-through; not in the plan).**
- Ricordi vocal scores are scans, so it is route A (OMR) again. That was the experiment's
  weakest step: wrong bar counts and damaged OCR.
- Acts are short (about 25–40 min), so placement is easy.
- Ricordi's rehearsal figures are natural units, about one or two minutes each. They are a
  better fit than page breaks.
- The language profile is the same as Mozart's.
- Conversational overlaps and divided choruses (La bohème Act II) test D10 and D22.

In short: Puccini's problem is the score source, and Wagner's is the form and the scale.

**What generalises.** The checklist, the checkpoints, the state and accept mechanism, the
config split and the matching smoke test carry over unchanged. Two concepts need to become
parameters: **piece** (a number, a scene section or a rehearsal span) and **act** (the span
one placement covers). A third, **profiles**, carries everything about publishers and
languages.

## 5. Code changes, in priority order

Sizes are rough session time:
- **S**: up to half a day;
- **M**: about a day;
- **L**: 2–3 days;
- **XL**: a week or more.

roon-rd changes need the user's approval. Nothing here is started.

### Before Così

| # | Change | Repo | Size | Why |
|---|---|---|---|---|
| P1 | **Recording inventory:** compare `match` in NFC; read `Act 1`/`Act I`/`Atto …` and profile words; stop on zero files, or on a score act with no tracks; recordings in their own files | libretto | S | Così fails here first (4.1) |
| P2 | **Cast and profiles:** `voice`, `short`, `kind` on the cast; `profiles/language/it.toml` (tempo words, act words, directions, choir parts, letters) and `profiles/publisher/baerenreiter.toml` (fonts, sizes, footer, ratios); remove `MALE`, `ABBR`, `CHOIR_PARTS` and the inline tables. Figaro must decode and build identically | libretto | M | Removes the silent Figaro and Italian assumptions |
| P3 | **Stops:** unknown tempo word, label, act word or font; `clean` below letter match 1.0 without a listed reason; more than 8 seccos | libretto | S | The tool stops instead of guessing |
| P4 | **State, accept, safe build:** [a record per output](#fmt-proposed) (input hashes, tool version); `build` redoes only stale outputs and doesn't re-place unless placement's inputs changed; `accept <opera> <rec> [pieces]` records accepted results; a rebuild that would change an accepted piece reports it, and keeps the accepted one unless told otherwise. Record Figaro's current outputs as accepted, including D21's four and No. 24 | libretto | L | Protects Figaro before any cross-opera fix; replaces kept-as-built by memory |
| P5 | **`regress` and the committed every-line test:** rebuild each opera's `[regression]` pieces for every recording and compare with the accepted results; a pytest that every timed line is published exactly once, and that every segment has `bar_start`/`bar_end` unless its text file's `source` records a D16 move (A.11); port the viewer card check from session scratch into roon-rd (a headless test of `libretto.js` card grouping) | both | M | The user's most-valued check, which today exists in neither repo |
| P6 | **`new` and `report`:** scaffold from a template; the font check; [`report <opera> <rec>`](#fmt-proposed) writes checks, flags and the spot-check list with disc, track and time; `status` shows stale, accepted and stopped items | libretto | M | G0, G6 |
| P7 | **roon-rd matching for many operas:** a "what did I match" view or log (Roon album, title, length → file, track, method) for G3; among equally fitting files, prefer the one whose opera title is in Roon's track title; read [`published/`](#fmt-proposed); fix the stale `config.toml` path | roon-rd | M | G3; the shared Solti box album |
| P8 | **Placement at Così's scale:** measure memory and time on Così's Act II; banded DTW only if needed | libretto | S to measure, M if needed | Acts twice as long |

**P1–P4 are the minimum** (about 2 days of the 6–8 for P1–P8). P5–P7 are what make a new
opera cheap. Separately, D22 (the ensemble card) and Figaro's full listening are already
"Now" in `todo.md`, and they come before Così regardless.

### Before Don Giovanni

| # | Change | Size |
|---|---|---|
| D1 | Versions and cuts: detect which numbers a recording contains (placement with optional pieces; appendix pieces as alternatives; flag squeezed pieces) | L |
| D2 | D7 if the user decides for the API: a `gloss` stage writing the text files, with a review sample per act | M–L, plus the API cost per opera |

### Before Wagner

| # | Change | Size |
|---|---|---|
| W1 | Verify Schott's PDF (user buys one); a publisher profile, or bring route A back if it's a scan | M for a profile, XL for route A |
| W2 | German language profile; the lyric style (upright or italic) chosen per profile | S–M |
| W3 | `form.kind = "continuous"`: section units from scene headings and system or page breaks; the placement span from config; section ids | L |
| W4 | Multiscale or banded placement for 2.5-hour spans | M |
| W5 | Vocal separation upgrade (BS-RoFormer) if the checks become unreadable | M |

Puccini would add route A as a supported path (XL). It isn't in the plan.

### Housekeeping, any time

- `pipeline/README.md` is stale (it covers only `decode`). **S.**
- The every-line harness in session scratch space should be retired once P5 lands.
- A cold-start test: start a fresh session in `libretto` and ask "how do I start Così?"
  (`sung-text-pipeline.md`). Do it after P6. **S.**

## 6. Decisions this asks for

These are for the user. Each becomes a D-entry once agreed.

1. **Adopt the checklist and checkpoints G0–G7,** with G3 (matching) before any timing.
2. **What "done" means for an opera** (section 3.4).
3. **Do P1–P4 before Così, or P1–P7?** Recommended: P1–P7. P8 is a measurement taken while
   doing Così.
4. **D7 for Così:** keep the gloss in the session, and decide after Così with the measured
   text hours (recommended).
5. **A second Così recording:** buy one now so Così tests both halves, or accept that it
   measures only the per-opera cost.
6. **The interchange layout change** (`published/`, `<opera>--<rec>`): it needs a roon-rd
   change and a new launch path.

<a id="appendix-a"></a>
## Appendix A. Output formats

*Added 2026-11-18.* This covers every file format the document refers to:
- the pipeline's outputs (A.2–A.11);
- the session-written text files (A.6);
- what roon-rd sends to the viewer (A.12);
- the formats section 3 proposes (A.14).

The shapes were read from Figaro's real files (Solti, with No. 16 and rec19a as examples)
and from the code, at the commits named at the top. Only keys, types and counts were
inspected, never the licensed content. "N×" means a list of N items. Field descriptions
come from the code unless marked otherwise.

### A.1 Overview

| # | File | Written by | Read by | Licence | Figaro size |
|---|---|---|---|---|---|
| [A.2](#fmt-decode-pieces) | `decode/pieces/<piece>.score.json` | `decode` | `score` | **licensed: full decode** | 21 MB, 72 files |
| [A.3](#fmt-manifest) | `decode/contents.json`, `manifest.json`, `manifest.md` | `decode` | `status`, piece lists, review | excerpts only | small |
| [A.4](#fmt-recording) | `recordings/<rec>/recording.json` | `add-recording` | everything per recording | shareable | small |
| [A.5](#fmt-audio) | `recordings/<rec>/mix/*.flac`, `stems/<track>/*.flac` | `separate` | `place`, `time` | derived from the audio library: private | 2.7 GB |
| [A.6](#fmt-text) | `operas/<opera>/text/<piece>.json` (git) | Claude, in session | `clean`, `time` (route C) | shareable | 64 files |
| [A.7](#fmt-score) | `score/<piece>.json` | `score` | `clean`, `place`, `time` | **treat as licensed** (see A.7) | 14 MB |
| [A.8](#fmt-placement) | `placement/<rec>.json` | `place` | `time` | shareable | 56 KB |
| [A.9](#fmt-clean) | `clean/<piece>.json` | `clean` | `time` | shareable | 3.8 MB |
| [A.10](#fmt-timed) | `timed/<rec>/<piece>.json` | `time` | `publish` | shareable | 7.1 MB |
| [A.11](#fmt-interchange) | `interchange/<rec>.interchange.json` | `publish` | roon-rd | shareable | 3.7 MB each |
| [A.12](#fmt-roon) | `libretto_update` message, `LibrettoContent` | roon-rd server | `libretto.js`, `libretto-mode.js` | — | — |
| [A.13](#fmt-logs) | stage logs (`build-act1.log`, `retime-*.log`, …) | every stage | people | shareable | small |
| [A.14](#fmt-proposed) | `recordings/<rec>.toml`, `state/`, `accepted/`, `report/`, `published/` | proposed | — | — | — |

The paths in A.2–A.11 are under `libretto-work/<opera>/` unless shown otherwise.

**Time bases.** Five clocks appear across these files, and the same field name changes
meaning between stages:

| Clock | Meaning | Fields |
|---|---|---|
| bars | bar number + fraction of the bar, as printed | decode `t0`/`t1`; `bar_start`/`bar_end`; `sylls[].bar` |
| score seconds | seconds at the nominal tempo from the tempo words (D21's "printed tempo") | `score_start`/`score_end`; `start`/`end` in `score/` and `clean/`; `notes[].start` |
| act seconds | seconds on the act's tracks joined in order | placement `start`/`end`; placement `acts[].offsets` |
| piece seconds | seconds on the tracks a piece spans, joined in order | `start`/`end` in `timed/`; `timed.offsets` |
| track seconds | seconds from the start of one track | `start`/`end` in the interchange |

**Checked:** No. 16's first line has `start` = 0.658 in `score/` and `clean/` (score
seconds), 161.18 in `timed/` (piece seconds), and its interchange copy is relative to its
track.

<a id="fmt-decode-pieces"></a>
### A.2 `decode/pieces/<piece>.score.json`: the decoded score

One file per piece, written by `decode/vector.py` via `pieces.py`. **Licensed:** this is
the score's full music and text. Never share or commit it.

```
{ id, edition, systems: int, pickup: bool,          // pickup: an upbeat numbered bar 0
  bars:      939× { bar, page, system, timesig: [beats, unit], tempo: str },
  notes:   18968× { staff, label, x, t0, t1, pitch: midi, kind: "black"|"half"|"whole" },
  syllables: 8813× { staff, label, text, t0, t1, x, sys: [page, system], n_staves, clef, choir: bool },
  cues:      141× { staff, label, name, t0, t1, sys, n_staves, clef, choir },
  problems:  [str] }
```

- `t0`/`t1` are in bars.
- `kind` is the notehead shape, so the rhythm is approximate: it comes from engraved
  spacing.
- `label` is the staff's printed label.
- A cue's `name` is a speaker cue above a shared staff; its `t0` equals its `t1`.
- `choir` marks a staff smaller than 0.92 of its system's largest (D19).
- `clef` is one of `treble`, `treble8`, `bass`.
- The piano's notes are included: they are what route B aligns.

<a id="fmt-manifest"></a>
### A.3 `decode/contents.json`, `manifest.json`, `manifest.md`

- **`contents.json`**: the score's table of contents, 95 entries of
  `{act, scene, kind, appendix, number, title, page}`.
- **`manifest.json`**: what the decode found.

```
{ opera, title, edition, score_pdf,
  checks: { pieces, with_problems, contents_entries, contents_not_found: [],
            pieces_not_in_contents: [], duplicate_ids: [] },
  pieces: 72× { id, kind, title, act, scene, appendix,
                pages: [first, last], pdf_pages: [first, last], start_system,
                bars: [first, last], bar_count, pickup, metre: [beats, unit], tempo, tempo_changes,
                systems, characters: [CAST NAME], syllables_by_staff: {CAST NAME: count},
                notes, syllables, first_words, sections: [{system, bar, mark, page}], problems } }
```

- `kind` is one of `sinfonia`, `number`, `secco`, `accompanied`, `aria`, `part`.
- `manifest.md` is the same as a table, for review at G1.
- `first_words` is a short excerpt.
- `stages.main_pieces` reads this file: every non-appendix piece.

<a id="fmt-recording"></a>
### A.4 `recordings/<rec>/recording.json`: the recording's inventory

```
{ id, album_title, album_title_aliases: [str], artist, conductor, orchestra, year, label, store,
  tracks: 32× { id: "d09t01", disc, track, title, act: int, duration_s, file, sample_rate, bits },
  total_s, acts: { "<act>": seconds } }
```

- The metadata is copied from `opera.toml`'s `[[recording]]`.
- `act` comes from the title (`act_of`). A title with no act inherits the previous one
  (section 4.1's Così failure).
- `file` is the absolute path into the read-only audio library.
- **Track ids:** `d09t01` here, but `d9t1` in the interchange (A.11).

<a id="fmt-audio"></a>
### A.5 Audio: `mix/<track>.flac`, `stems/<track>/{vocals,no_vocals}.flac`

- `mix`: mono, 22.05 kHz, 16-bit FLAC. This is what the alignment listens to.
- `stems`: Demucs htdemucs two-stem output, converted to FLAC.
- `separate` resumes by checking that `no_vocals.flac` exists, so a half-written stem
  would be skipped.
- Nothing records which Demucs model or version made a stem.
- These files are the bulk of `libretto-work`: 2.7 GB for Figaro.

<a id="fmt-text"></a>
### A.6 `operas/<opera>/text/<piece>.json`: the session-written text

The one hand-made format, in git. There are two forms:

```
// numbers and orchestral recitatives (route B): each character's verses, each written once
{ piece, written: date, source: str,
  characters: { CAST NAME: [ verse: [ [it, en] phrase, … ], … ] } }

// secco recitatives (route C): speeches in sung order, repeats written out
{ piece, written, source,
  speeches: [ { characters: [CAST NAME], phrases: [ [it, en], … ] } ] }
```

- `en` is the literal word-order gloss.
- `source` records provenance and any D16 speaker moves.
- Character names must equal the cast names, but nothing checks that until `clean`.
- For a route C file, `stages.text_of` derives `characters` from `speeches` (one verse per
  speech).
- There is no schema version, and nothing validates the file before `clean` runs. P3's
  `clean` gate would be the first check on it.

<a id="fmt-score"></a>
### A.7 `score/<piece>.json`: lines and notes in score time

```
{ lines: 851× { start, end, score_start, score_end, bar_start, bar_end,
                character: CAST NAME, voice: int, text,
                sylls: [ { text, start, end, score_start, score_end, bar } ] },
  notes: 18968× { start, duration, pitch, velocity, instrument: CAST NAME | "piano"… },
  sections: [ { bar, score_s, timesig, tempo, bar_s } ],   // a tempo or metre change
  score_end_s, bars: 939× [bar, score_s, bar_s], unattributed: int }
```

- **Time:** `start` equals `score_start` here (score seconds).
- **`voice`** is the character's cast-order index (the viewer uses it for colour and order).
- **`notes`** are in the shape of a MIDI note list (velocity is a constant 80). That is the
  shape synctoolbox expects. `instrument` carries the singer's name.
- **`unattributed`** counts syllables with no singer (D19).
- **Licence:** the lines carry every sung syllable and the notes carry every pitch, so this
  file holds as much of the score as A.2. The licence rule names only
  `decode/pieces/*.score.json`. **Proposed:** treat `score/` as licensed too.

<a id="fmt-placement"></a>
### A.8 `placement/<rec>.json`: where each piece is in the recording

```
{ recording, rate_hz: int,                       // the DTW frame rate
  acts: 5× { act, tracks: [track id], offsets: [act seconds], duration, tuning_cents,
             chroma_shift, fit },
  pieces: 65× { id, kind, act, start, end,          // act seconds
                fit, nominal_s, tempo_ratio,
                tracks: [track id], track_start: [track id, s], track_end: [track id, s],
                titles: null | …, flags: [str],     // flags only on numbers
                region: [start, end], region_tracks: [track id] } }   // secco only
```

- `tempo_ratio` is the placed span over `nominal_s`.
- Flags seen: `titles disagree` (once, Solti's No. 9); `low fit` and `tempo ratio` are also
  possible.
- A secco gets a `region`: the gap between its neighbouring numbers, which route C times
  as a group.
- `flags` is missing on seccos rather than empty, so cut seccos can never be flagged
  (section 4.1).

<a id="fmt-clean"></a>
### A.9 `clean/<piece>.json`: text matched to the decode

```
{ piece,
  lines: 823× { start, end, score_start, score_end, bar_start, bar_end, character, voice,
                text, ocr, translation,
                gloss: [ { v: "CHAR:verse", c: phrase index, it, en,
                           start, end, score_start, score_end } ],
                verses: { "CHAR:verse": [ [it, en], … ] } },
  report: { CAST NAME: { lines, cleaned, letter_match, verses, verses_not_found: [] } } }
```

- **Time:** score seconds, as in A.7.
- **`text`** is the session's spelling. **`ocr`** is the decoded text it replaced (the name
  is left over from the OMR days).
- **`gloss`** splits a line into phrases. A phrase is named by its verse (`v`) and its
  phrase index within the verse (`c`).
- **`verses`** carries the whole verse each line belongs to, so the viewer can show an
  ensemble stanza.
- **`report`** is what G1 and P3 would gate on. Today it is only logged.

<a id="fmt-timed"></a>
### A.10 `timed/<rec>/<piece>.json`: lines in recording time

The route B form (numbers, orchestral recitatives):

```
{ piece, recording, tracks: [track id], offsets: [piece seconds],
  lines: [ clean's line, with start/end/gloss times now in piece seconds ],
  checks: { route: "B", singing_agreement, entries_predicted, entries_matched,
            median_abs_offset_s, within_1s, phrases, phrases_mostly_silent,
            tuning_cents, excerpt: [s, s], placed: [s, s], section_tempo_factors: [..],
            trimmed_starts, trimmed_ends } }
```

The route C form (secco):

```
{ piece, recording, tracks, offsets,
  lines: [ { start, end, character, voice, text, translation, gloss: [ {v, c, it, en, start, end} ],
             verses: { "CHAR:rec19a.0": … }, source: "recit-forced-align",
             bar_start?, bar_end? } ],
  checks: { route: "C", region: [s, s], words, words_mostly_silent, median_word_heard,
            men_speeches_low: str } }
```

The two routes differ in four ways:
- **No score times in route C.** Its lines have no `score_start`/`score_end`, and its gloss
  phrases have none either.
- **Bars only where found.** A route C line gets `bar_start`/`bar_end` only when its words
  are matched back to the decode, so a D16-moved speech has none.
- **Two verse-id schemes.** Route B uses `"CHAR:3"`; route C uses `"CHAR:<piece>.<n>"`.
- **Different checks.** Route C's `men_speeches_low` depends on the hard-coded `MALE`
  set (section 2.3).

`checks` is where every per-piece figure in STATUS.md comes from. Nothing records what
produced the file (placement, text and code versions), which is what P4 adds.

<a id="fmt-interchange"></a>
### A.11 `interchange/<rec>.interchange.json`: what roon-rd loads

Version `1.1-score` (D5).

```
{ version: "1.1-score",
  opera: { title, composer, language, translation_language, year },
  edition,
  recording: { album_title, album_title_aliases, artist, conductor, orchestra, year, label },
  tracks: 32× { track_id: "d9t1", title, disc_number, track_number,
                duration_seconds,                  // ms precision (D11)
                act: "1",                          // a string here
                segments: [ { start, end,          // track seconds
                              score_start?, score_end?, bar_start?, bar_end?,
                              character, voice, text, translation,
                              gloss: [ {v, c, it, en, start, end, score_start?, score_end?} ],
                              verses: { "CHAR:…": [ [it, en], … ] },
                              piece, source? } ] } }
```

**How `publish` builds it (checked):**
- Each line goes to the track its start falls in. Its times become track-relative, and its
  end is clamped to the track's end.
- `ocr` and `sylls` are dropped.
- `piece` is added.
- Segments are sorted by start, then voice.
- Every track of the recording is listed, with empty `segments` where nothing is timed.
  That makes G3's empty interchange already possible.

**Figaro, Solti: 3,327 segments in three shapes:**
- **2,450 route B segments,** which have every field;
- **853 route C segments,** with `source` and no score times;
- **24 segments with no bars:** the D16-moved speeches.

**Quirks worth fixing with the next format version:**

| Quirk | Effect |
|---|---|
| `opera.title` is composer + title concatenated ("Wolfgang Amadeus Mozart Le nozze di Figaro") | The display has to undo it |
| `translation_language` is hard-coded `"en"` | Ignores `opera.toml` |
| `act` is a string here and an integer everywhere else | Inconsistent types |
| `track_id` `d9t1` differs from the pipeline's `d09t01` | Two id schemes for one track |
| A recording with no disc tags gets disc 1 | Cannot be told from a real disc 1 |
| Nothing names the opera or the recording id | roon-rd's file id is the file name (A.14's `published/`) |
| `verses` is repeated in every segment (3,310 carry one verse, 17 carry two) | Most of the 3.7 MB |

**What the viewer reads (checked):**
- `libretto.js` uses `start`, `end`, `character`, `voice`, `text`, `translation`, `gloss`
  and `verses`.
- `libretto-mode.js` shows `piece` and `bar_start`.
- Nothing in roon-rd reads `score_start` or `score_end`.

<a id="fmt-roon"></a>
### A.12 roon-rd to the browser: `libretto_update` and `LibrettoContent`

Sent over the websocket per zone, only when the matched file and track change
(`server/mod.rs`, `send_libretto_updates`):

```
{ type: "libretto_update", zone_id, content: null | LibrettoContent }

LibrettoContent = { file_id, track_index, match_method, track_title, track_id, act,
                    album, artist, opera_title,
                    segments: [ interchange segment, passed through unchanged ] }
```

- `match_method` names the rule that matched the track (album and title, album and length,
  and so on: D11). Logged against Roon's album, title and length, it is the raw material of
  G3's matching report (P7).
- `/libretto`, `/librettos` and `/libretto/:file_id` serve the same data over HTTP for the
  older full-page viewer.

<a id="fmt-logs"></a>
### A.13 Logs

Every stage writes the user's standard log line:
`YYYY-MM-DD HH:MM:SS.mmm <level>   message`, with the `--version` line first.
- The per-piece check figures go to the log at `info` (`stages._write_timed`).
- The build logs in `libretto-work/<opera>/` are copies of that output, kept by hand.
- They are the only record of when a stage ran, which is why section 3.6 proposes a cost
  log.

<a id="fmt-proposed"></a>
### A.14 Proposed formats (section 3; not built)

Sketches for discussion. Each becomes real only with its change in section 5.

**`operas/<opera>/recordings/<rec>.toml`** (P1). The current `[[recording]]` table, one
file per recording:

```toml
id = "georg-solti-1974"
album_title = "…"                         # what the files' tags say
album_title_aliases = ["Solti: The Operas - Mozart"]   # what Roon shows (G3)
artist = "…"; conductor = "…"; orchestra = "…"; year = 1974; label = "Decca"; store = "Qobuz"
sources = ["~/Dropbox/Audio/…/CD 01", "…/CD 02", "…/CD 03"]
match = "Così fan tutte"                   # compared in NFC (P1)
```

**`state/<stage>/<unit>.json`** (P4). One per output, written beside it:

```
{ stage: "time", unit: "georg-solti-1981/no16", output: "timed/georg-solti-1981/no16.json",
  output_sha: "…", tool_version: "libretto-pipeline 0.1.0 … (ab12cd3)", finished: "2026-…",
  inputs: { "placement/georg-solti-1981.json#no16": "sha…", "clean/no16.json": "sha…",
            "opera.toml#timing": "sha…" } }
```

An output is stale when any input's hash differs, or when the code it depends on has a
different version. `status` lists stale outputs; `build` redoes only those.

**`accepted/<rec>.json`** (P4). The user's verdicts, as data:

```
{ recording: "georg-solti-1981",
  pieces: { "no09": { output_sha: "…", checks: { …A.10's checks… }, accepted: "2026-11-16",
                      by: "spot check, Act III", note: "kept as built (D21)" } } }
```

A rebuild that would change an accepted piece's output reports the change and keeps the
accepted file unless told otherwise. `regress` compares new checks with these.

**`report/<rec>.md`** (P6). Generated, never edited:
- the per-piece check table (A.10's `checks`), with the weak pieces against agreed bands;
- the placement flags (A.8);
- the D16 moves, from the text files' `source`;
- the **spot-check list**, each item with its disc, track and time (the D17 practice, made
  a product).

**`libretto-work/published/<opera>--<rec>.interchange.json`** (P7). A.11 unchanged, in one
directory for every opera. The file name makes roon-rd's file id unique. With the next
format version:
- add an `opera.id` and a `recording.id`;
- separate `opera.title` from the composer.

Then roon-rd can say which opera it matched, and break ties between files by opera (the
shared Solti box, section 4.1).
