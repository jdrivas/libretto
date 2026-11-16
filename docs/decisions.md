# Decision log

One entry per decision, newest last, numbered for reference ("D12"). Each entry gives
the date, the decision, why it was made, the alternatives considered, and when or whether
to revisit it. Append a new entry whenever a decision is made in a session.

To change a decision, don't edit the old entry: add a new one that supersedes it, and
mark the old one "superseded by Dn".

## To revisit

Entries that are flagged to come back to, and when. Raise each one when its trigger arrives.

| Entry | Revisit when |
|---|---|
| D7 Gloss written in the session | once the glosses are trusted; at the Così checkpoint at the latest |
| D13 Tempo-limited guide (option A) left off | if a number drifts and the key shift is right |
| D14 Vocal-stem onsets (option B) not pursued | if a number or recitative is mistimed and D12 doesn't explain it |
| D17 Spot check per act; full listening later | when all of Figaro is built |

## Entries

### D1. Route B is the main path for Figaro (2026-11-11)
Every number is timed from the Bärenreiter decode, and the Sextet and "Voi che sapete" are
re-timed from it. Route A stays only for operas without a born-digital score.
*Why:* the decode gives notes, bars and syllables directly. *Revisit:* no.

### D2. Secco recitative by route C, with the score's text (2026-11-11; confirmed 2026-11-12)
The recitative text and speakers come from the decode, and every timed word gets its bar.
*Why:* the P8 side test on "Eccovi" showed route B's phrase starts were a median 0.72 s
off route C's (Solti), with 21 of 42 over 1 s. *Alternatives:* route B for everything.
*Revisit:* no.

### D3. Verse list, phrase split and gloss in a session (2026-11-11)
The tool decodes the sung syllables. Claude writes each character's verse list (each line
once, in order), the phrase split and the literal gloss in one pass, and the tool checks
that every sung syllable maps to a verse and phrase. No published libretto is needed.

### D4. A light restructure first (2026-11-11)
Per-number outputs, a single publish step, and stage verbs wrapping the existing scripts.
The first test was a from-scratch rebuild of the five passages (P8).

### D5. Bar locations in the interchange now (2026-11-11)
Every segment carries its number, `bar_start`/`bar_end` and the edition, with format
version 1.1-score. The viewer shows them later.

### D6. Workspace (2026-11-11)
The `libretto` repo holds the pipeline and texts. Scores and bulk data go in `libretto-work`,
which is Dropbox-ignored, and the plan is in `operas/figaro/STATUS.md`. *Revisit:* move
`libretto-work` when the external disk arrives.

### D7. Gloss in the session, not inside the tool (2026-11-08)
The tool stops after `text`. Claude writes the phrase split and gloss in a session, and the
tool checks them. *Alternative:* the tool calls the Claude API itself, which runs unattended
but needs an API key and costs money per opera. *Revisit:* once the glosses are trusted, at
the Così checkpoint at the latest.

### D8. Placement per act, not per number (2026-11-11)
All of an act's pieces, numbers and recitatives, are joined in score order and aligned to
the act's audio with one DTW. *Why:* placing number by number failed. One wrong local match
(No. 15 inside the finale) threw the rest of Act II off.

### D9. Refine each placed piece's edges from its first and last 20 s of score (2026-11-12)
*Why:* placement could start a number 24 s early after a recitative (Solti's Sextet).

### D10. Chorus shown once, from the top choir staff (2026-11-12)
The score stage takes CORO text from the top choir staff that is singing in each bar.
*Alternative:* a line per choir staff, which repeats the same words in the CORO card.
*Confirmed 2026-11-15* after the user listened to and watched Nos. 8-9 in both recordings:
"one for all". *Revisit:* no. It applies to every chorus (No. 23 and later operas).

### D11. roon-rd track matching (2026-11-12, refined 2026-11-13; roon-rd 5890dc3, e4ba2a0)
An `album_title_aliases` entry in opera.toml covers Roon's album names, for example "Solti:
The Operas - Mozart". Then:
1. an exact title match;
2. else a track whose length fits, in a file whose album Roon matches exactly;
3. else the closest length.

A track fits Roon's length L if it lasts from L to L + 1 s, because Roon truncates to
whole seconds. Durations are published to the millisecond. *Why:* Roon passes no disc
number, and its titles differ from the file tags, so matching on titles alone picked the
wrong track or an empty one.

### D12. Chroma shifts restricted to {0, 1, 11} in alignment and edges (2026-11-13)
That is, the score's key or a semitone either side, as placement already used. *Why:* a
piece's own estimate over all 12 shifts was sometimes a fourth or fifth off (Solti's Nos.
2-3 and several 20 s edges), and the alignment then matched the wrong music. *Revisit:*
only if a recording is pitched further off, for example a transposed aria.

### D13. Tempo-limited guide (option A) left off (2026-11-13)
It is in align.py behind `GUIDE = False`. *Why:* it didn't help (Solti No. 2 dropped to
60-65%), because the cause was the key (D12). *Revisit:* if a number drifts while its key
shift is right.

### D14. Vocal-stem onsets (option B) not pursued (2026-11-15)
*Why:* after D12, every Act I number is at 78-94% agreement, and the Solti recitatives
sound right even though the check flags 10-32% of their words as in silence. That flag
comes from the detector missing low voices. *Revisit:* if a number or recitative is mistimed
and D12 doesn't explain it.

### D15. The interchange keeps the files' disc numbers (2026-11-15)
For example, Solti's discs stay 9-11. This supersedes P7's renumbering from 1. *Why:* Roon
shows them as 9-11 too. If Roon ever passes a disc number, a renumbered file would match
nothing.

### D16. Missed speaker cues in secco follow Da Ponte's libretto (2026-11-12; recorded 2026-11-15)
A secco recitative puts every character on one shared staff, named by cues above the notes.
Where the decode misses a cue and runs one character's words into another's speech, the text
file gives them to the speaker in Da Ponte's libretto. The file's `source` names the bars and
the change. Otherwise the text follows the decode. *Why:* the words are timed by route C from
the text file's speeches, so the right singer's card must light up. *Consequence:* a moved
speech has no bar numbers ("verses not found" in the clean report), because no decoded line
of that character carries it. *Alternatives:* follow the decode as printed, which mislabels
the singer. Applied in rec05a, rec08a and rec09a (Act I) and in rec11a, rec13a and rec13b
(Act II).

### D17. Listening: a spot check per act, with a full listening later (2026-11-15)
After each act is built, the user spot-checks it in the viewer (the weak pieces from the
checks, the moved speakers, a few numbers) and we go on. A full listening comes later.
*Why:* the checks plus a spot check have caught every real problem so far (the key shift,
track matching); a full listening of each act would hold up the build. *Revisit:* when the
whole opera is built, before calling Figaro done: plan the full listening then.
