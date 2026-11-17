# How Figaro was built

*Written 2026-11-16, when all four acts were built and spot-checked.*

This document describes the process that built *Le nozze di Figaro*: the order of the
work, how it was reviewed, and how the tools changed along the way. It is meant as a
model for Così fan tutte and the operas after it. The details are in three other places:
- `operas/figaro/STATUS.md` records each stage's results;
- `docs/decisions.md` (D1–D24) records each decision and why it was made;
- the `pipeline` branch history holds the code.

Figaro has two recordings, Solti 1981 (Decca) and Currentzis 2014 (Sony). The score is
Bärenreiter BA 4565, a licensed vector PDF.

## In short

The work had four phases:
1. an earlier approach, dropped;
2. an experiment on a few hard passages;
3. a preparation stage that made the experiment a tool and ran its heavy steps on the whole opera;
4. a loop, one act at a time.

The loop for each act was: texts, build, automatic checks, the user's review, then fixes
and re-timing. Each act exposed something new, and the fix went into the tools. Some fixes
were carried back to acts already built. The review got lighter as the results earned
trust.

## 1. The approach that was dropped (February 2026)

The first libretto tool started from a published libretto. It spread each track's lines
across the track by word count, using track listings and anchor lines. Two problems
ended it:
- **The timing was an estimate**, not a measurement.
- **A libretto is not what is sung.** It leaves out repetitions, it writes simultaneous
  ensemble lines one after another, and its translations follow the poetry rather than
  the sung words.

That led to the new direction: take the text and structure from the score, and the
timing from aligning the recording to the score (`sextet-experiment-plan.md`).

## 2. The experiment (4–8 November)

The experiment tested the idea on the hardest case first: the Sextet, six voices
singing together. It grew to five passages:
- the Sextet (No. 19);
- "Voi che sapete" (No. 12);
- three secco recitatives;
- the Act II finale (No. 16).

It settled the three main methods:
- **Route B, for numbers.** The score is decoded into notes, bars and syllables, and
  aligned to the recording by dynamic time warping on chroma features.
- **Route C, for secco recitatives.** The words are force-aligned to the singing.
  Secco has no steady tempo for route B to follow.
- **Text from the score.** The sung text comes from the Bärenreiter vector PDF, read
  directly, not from OCR or a published libretto.

The user listened to the five passages and approved them. The repertoire plan was agreed
at the end of the experiment (`repertoire-plan.md`).

## 3. Preparation, P0–P8 (11–12 November)

The preparation turned the experiment's scripts into one tool, `libretto-pipeline`, and
ran its heavy steps on the whole opera at once. Those steps work the same way for every
piece, and they are slow, so there was no reason to run them act by act.

| Stage | Work |
|---|---|
| P0–P1 | Housekeeping and the workspace. Bulk outputs go to `libretto-work`, outside the repo. |
| P2 | Decode the **whole score**: 72 pieces, checked against the score's table of contents. Decoder fixes included lyric size on the smaller ensemble pages, chorus systems and upbeat bars. |
| P3 | Separate the **whole recordings** into voice and orchestra (Demucs), for both recordings. |
| P4 | The tool's stages: `score`, `clean`, `place`, `time`, `publish`. |
| P5 | Place **every piece** in each recording. Placing number by number failed, so pieces are placed one act at a time (D8). |
| P6 | Text format and the text draft command. Texts for the five passages. |
| P7 | Bar locations on every line, so the viewer can show where each line is in the score (D5). |
| P8 | Regression: rebuild the five experiment passages from scratch with the new tool. |

The P8 rebuild matched the experiment, and the user passed it in the viewer. It also
found that a number could start 24 s early, so each number's start and end are now
refined from its opening and closing music (D9). The viewer check found that Roon names
Solti's album differently from the files, which led to album aliases and to matching
tracks by length in roon-rd (D11).

## 4. The act loop (12–16 November)

Each act went through the same steps:
1. **Texts.** For each piece, the session wrote the sung text by speaker, the phrase
   split and a literal English gloss, from the decoded score (D3, D7). The tool checks
   that every decoded syllable is accounted for.
2. **Build.** Each piece was timed in both recordings and published to the viewer's
   files.
3. **Automatic checks:**
   - how much of each piece's lines fall where the vocal stem has singing;
   - whether each singer's entries match onsets in the vocal stem;
   - in recitatives, how many words fall in silence;
   - that every line is published exactly once.
4. **The user's review** in roon-rd's viewer, listening to both recordings.
5. **Fixes and re-timing.** A tool fix was re-run on every piece it affects, including
   pieces in earlier acts.

### What each act exposed

| Act | Problem | Change to the tools |
|---|---|---|
| I | Alignment sometimes matched music a fourth or fifth away: a piece's own key estimate was wrong | Try only shifts of 0 and ±1 semitone, as placement does (D12). Solti's Sextet entries went from 0.94 s to 0.11 s. |
| I | Roon passes no disc number and rounds track lengths | roon-rd matches by exact title, then by album and length (D11) |
| I | Chorus lines repeated once per choir staff | Read the chorus once, from the top choir staff (D10) |
| II | The decode missed some speaker cues in recitatives | Give the speech to Da Ponte's speaker and note it (D16) |
| III | Unlabelled choir staves took the soloists' names, and one font of staff labels was ignored | Decoder: choir staves recognised by size, Times-Roman labels read (D19) |
| III | Fast numbers started late (No. 21 by 50 s): the opening was searched for only at the printed tempo | Search the opening at the recording's own pace (D21). This also re-timed four Act I numbers. |
| III | Orchestral recitatives: route B or C? | Route B, like numbers (D20) |
| IV | The Finale's closing tutti appeared once per singer | One TUTTI card for all-singer passages (D23) |
| IV | No. 24's end was found 40 s early in both recordings | A short list of numbers keep their placed end (D24) |

### How the review changed

- **Act I** had a full listening of both recordings, recitatives included.
- **From Act II**, a spot check was enough (D17). Claude listed:
  - the weakest pieces by the checks;
  - the speeches it had given to another speaker;
  - a few other numbers;

  each with its disc, track and time. The user listened to those.
- **Fewer stops (D18).** Within an agreed plan, Claude went ahead with texts, builds,
  checks, fixes and commits. It stopped only for a real choice, which it brought with a
  recommendation, or for a result it could not explain.
- **Limits of the checks.** The checks flag a piece; listening decides. Several low
  scores turned out to be the voice detector missing soft or light singing, not bad
  timing. Examples: Solti's "Deh vieni", Solti's No. 15 opening, and many of Solti's
  recitatives.

As the tools settled, each act went faster. Acts II, III and IV were each built and
checked in about a day.

## Result

64 pieces are published for each recording, 3,327 lines each, every line once. Each line
has its time, speaker, bar location in BA 4565, Italian text and English gloss.

Before Figaro is called done:
- **D22:** show simultaneous singing as one ensemble card. No. 21 is the first case.
- **D17:** a full listening of the whole opera, after D22.
- **Two appendix pieces** the decoder doesn't handle yet: app-no15 and app-no28a.
- **The titles check.**
- **roon-rd:** merge the branch `libretto-number-match` into master, which needs the
  user's approval.

## What carries over to the next opera

- **Run the heavy steps on the whole opera first.** Decode, separate and place, then
  check the decode against the score's table of contents before writing any text.
- **Begin with a small regression set** of known-hard passages, and rebuild it after
  any tool change.
- **Go act by act** for text and timing, with automatic checks, then a spot check of the
  weak pieces and the judgment calls.
- **Record every decision as it is made** (`decisions.md`), with a revisit trigger where
  it is provisional.
- **Expect each act to find one new problem.** Fix it in the tool, not by hand, and
  re-time what it affects.

Così, next in the repertoire plan, is the test of how much of this is now routine.
