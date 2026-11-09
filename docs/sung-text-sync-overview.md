# Sung-Text Sync for Opera Recordings: Overview

*Drafted 2026-11-05. High-level description, to be broken into problem statements and then a plan.*

## What we're building

A **time-synced lyrics view for opera**, like the synced lyrics Roon and other players show for popular music. While a recording plays, the viewer shows **the words actually being sung**, roughly when they're sung, together with a **literal translation**.

It should work for **any recording** of a work we've prepared, **automatically**: no listening, tapping or hand-annotating per recording.

## Milestones

1. **First working system:** sung text is displayed roughly in time with the singing, **ideally within a 1–2 second window** of when it's sung. This is the bar for "working".
2. **Add-ons after that:**
   - **Literal translation** alongside the sung text. It's in scope and wanted; how to display it is still open (interlinear, side by side, toggle, …).
   - **Highlighting** of the text currently being sung, at line level first, possibly word level later.

## What changes from the libretto approach

| Before | Now |
|---|---|
| Text from a published libretto (a literary distillation of what's sung) | Text from the **score's vocal parts**: what's actually sung, repetitions included |
| Segment timing estimated per recording, with manual correction planned | Timing derived **automatically** from the recording |
| Libretto segments (paragraphs, stage directions) | **Sung lines**, like popular-music lyrics |
| Concern for spelling and text variants between sources | **Not a concern**: whatever the score says is what we show |

## Accuracy target

**Milestone 1: within about 1–2 seconds** of when the text is sung. Timing errors of a second or less are treated as negligible until experience says otherwise. Tighter timing matters mainly for the highlighting add-on.

## The approach in one picture

```
            built once per work                      automatic, per recording
 ┌────────────────────────────────────┐   ┌──────────────────────────────────────┐
 │ SCORE                              │   │ RECORDING (tracks in Roon)           │
 │  └─ sung text per character        │   │  └─ which score range is in each     │
 │      placed in bars                │──►│     track (number(s), bar range)     │
 │  └─ notes per bar (for alignment)  │   │  └─ where each bar lands in time     │
 └────────────────────────────────────┘   └──────────────────────────────────────┘
                                                        │
                                                        ▼
                                  DISPLAY (roon-rd): line start = time of the bar
                                  (and beat) where its first word is sung;
                                  highlight follows the playback position
```

Three layers:

1. **Work layer: once per opera.** From the score, build a "sung-text score": for each number, its bars in order, the notes in each bar (enough to align against audio), and the sung text in each bar, by character. Group the text into displayable lines.
2. **Recording layer: automatic, per recording.** (a) Work out which part of the score each track contains. Tracks don't line up with numbers: the Solti Sextet track also holds the following recitative. (b) Find when each bar happens in that track, using audio-to-score alignment.
3. **Display layer: roon-rd.** Combine the two to get a start time for each line. Show the lines due at the current playback position, including simultaneous lines in ensembles, with their literal translation. Highlighting is a later add-on.

A **literal translation** of the sung text is produced once per work, alongside layer 1. It's recording-independent, like the text itself.

## Design principles

- **The score is the single source of truth** for both text and musical structure.
- **No per-recording manual work.** Preparing a work may involve some one-time effort; adding a recording must not.
- **Robust over precise.** A 1–2 s window is enough for milestone 1. Prefer methods that degrade gracefully, for example falling back to estimates between known anchor points where alignment is weak, rather than precise methods that fail badly.
- **Show nothing rather than something wrong** when a track can't be matched with confidence.

## The core problems

These are the seeds for detailed problem statements.

**P1. Sung text and bar structure from a score (work layer).**
Finding usable score sources (machine-readable scores where they exist, scans otherwise); extracting the sung text with its bar positions; knowing which character sings each line; handling repetitions; grouping syllables into displayable lines; covering recitatives as well as numbers.

**P2. Track ↔ score correspondence (recording layer).**
Identifying the work and the number(s) in each track from Roon's metadata, and from the audio if needed. Handling tracks that contain several numbers or only part of one, plus cuts, alternate arias and reordered scenes.

**P3. Bar → time (recording layer).**
Aligning each track's audio to the score's notes. This depends on how good the score's notes are, and is hardest in recitatives (improvised keyboard accompaniment) and in passages full of fermatas and rubato.

**P4. Display and playback sync (roon-rd).**
Roon reports playback position in whole seconds, about once a second, and the viewer currently doesn't interpolate between updates. Also: output latency (dCS/Network Bridge buffering), showing ensembles with several singers at once, and switching lines slightly before they're sung so the reader keeps up.

**P5. Literal translation (work layer).**
Producing a close, literal translation of the sung text, line by line, including the repetitions and fragments that a sung line contains. How it's displayed alongside the original is an open design question.

## What we already know

From the Sextet experiment ([sextet-experiment-plan.md](sextet-experiment-plan.md)):

- **P3 works when the score is good.** With an accurate MIDI, audio-to-score alignment gave bar times that wobble locally but recover quickly, comfortably within the ±1 s target, with no drift.
- **Automatic scanning isn't good enough as-is.** Audiveris got bar structure wrong on the old Ricordi scan, its alignment degraded after the first third even on clean typesetting, and it captured no lyrics at all.
- **Machine-readable scores are the key enabler.** Where one exists (as the Sextet MIDI did), P3 is close to solved.
- **P4 has a known gap:** whole-second position updates with no interpolation in the viewer.

## Open questions

- How much of Figaro (about 28 numbers plus recitatives) exists as machine-readable scores?
- Can sung text with bar positions be read reliably from page images (vision), including which character sings it?
- How should recitatives be handled, where both the score's chords and the alignment are weakest?
- How do we prove "any recording": what does a second Figaro recording need beyond running the pipeline?
- How should the literal translation be displayed with the sung text?

## Out of scope for now

Librettos as a text source; per-recording hand annotation (tapping, downbeat marking); normalizing spelling or text variants; crowdsourcing. Highlighting is deferred to after milestone 1.

## What success looks like (milestone 1)

1. Play any track of the Solti Figaro in Roon, and the roon-rd viewer shows the sung text within about 1–2 seconds of when it's sung, with no manual work for that recording.
2. Do the same with a **second recording** of Figaro, with nothing more than running the pipeline.
