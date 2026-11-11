# Scaling notes: from one number to many operas

Working notes (started 2026-11-07, during the Act II finale test). Goal: a set of tools that
build a time-synced sung-text file for **any opera and any recording, fully automatically**,
given only that the audio files exist and can be found. Everything else, including the score,
is fetched or derived. This doc collects what we have learned so far, where we will stub our
toes, and how the pieces might fit together. The pipeline description made from it (step 5)
is `sung-text-pipeline.md`.

## What "fully automated" has to cover

| Input | Per | Today | Automated source |
|---|---|---|---|
| Audio files | recording | Paths given by hand | Search the audio trees by tags (album, work, conductor); audio is read-only |
| Recording metadata (album, titles, track numbers) | recording | File tags; titles passed by hand | File tags, cross-checked against Roon's browse API (Roon titles differ from the tags) |
| Score (vocal score PDF) | opera | Found and downloaded by hand from IMSLP | IMSLP API: pick a public-domain vocal score (Ricordi, Schirmer, Peters, Breitkopf) |
| Page map (which pages hold which number) | opera | `page_descriptions.json`, from an earlier vision pass | Vision/LLM pass over page images, one call per page |
| Libretto (clean sung text, by speaker) | opera | murashev, by hand per number | A libretto site, parsed into speeches |
| Per-character reference text with literal gloss | opera | Written by hand per number (`*_text.py`) | LLM pass from the libretto, then automatic checks |
| Cast list, voice types, score staff order | opera | Hard-coded per number | Libretto cast list, plus the staff labels in the score |
| Track-to-number map | recording | Implicit (I picked the tracks) | Title match, then audio check (chroma against each number's score) |

## Two kinds of work: per opera and per recording

**Per opera (once; recording-independent)**

1. Fetch the score, then build the page map.
2. Read each number with Audiveris: a notes run, plus a text run with Italian/German/French
   OCR. Cut the PDF into numbers first, so a bad page only spoils its own number.
3. Build the reference text from the libretto: speeches per character, split into phrases,
   with a literal gloss.
4. Work out who sings on each staff. Then build lines from the score: syllables, words and
   lines with score times, cleaned against the reference text.
5. Output one recording-independent "clean lines" file per number, everything in score time.
   This is what phase 8 produces today for the Sextet.

**Per recording (cheap; repeat for every recording)**

1. Find the audio and read its tags. Map tracks to numbers. A number can span several tracks
   (the finale is 3 tracks for Solti, 7 for Currentzis), and one track can hold several
   numbers (Sextet + two recitatives).
2. Decode the audio, separate the vocals (Demucs), and join each number's tracks into one
   continuous timeline.
3. Time each number:
   - **Numbers with a score** (arias, ensembles, finales): score-to-audio alignment, then
     re-time the clean lines through it.
   - **Recitativo secco:** forced alignment of the text to the vocal stem (MMS_FA). No score
     needed, and it worked well on three recitatives.
4. Trim lines out of clear silences. Run the checks. Split the lines back into tracks.
5. Write the interchange file, with titles as Roon reports them.

## Where we will stub our toes (seen so far, or very likely)

**Scores and score reading**
- **IMSLP:** a disclaimer page and download timer; rate limits; copyright varies by country,
  and only public-domain scans are safe to use. File quality varies widely. Some scans are
  full scores, which have transposing instruments and need different handling from vocal
  scores.
- **Audiveris is slow:** about 1 minute per page per run. A 350-page opera at two runs is
  roughly 12 hours of CPU time. It parallelises well across numbers. Results must be cached
  and never re-run unnecessarily.
- **Audiveris makes errors:**
  - wrong bar counts (Ricordi Sextet: 144 found against 140 real);
  - lost or merged lyrics;
  - dynamics and directions read as text ("sotto voce");
  - with OCR on, character labels split the score into many parts. This broke
    transposition in a full score, which is why we use the notes run for notes and the
    text run for text.
- **Vocal scores change their staves from system to system.** One singer, then three, then
  seven; the labels are abbreviated and ambiguous ("C" can be the Count or the Countess).
  Who sings on a staff has to be decided per system, from labels, clefs, score order, and
  how well the staff's words match each character's text. This is being built now for the
  finale.
- **Tempo:** the score is read at a nominal 120 bpm and the alignment absorbs the
  difference. Big tempo changes inside one number (the finale has many) and fermatas or
  pauses strain it. The silence trimming fixes the visible symptoms.
- **Edition differences:** recordings follow critical editions (Bärenreiter); old vocal
  scores (Ricordi) differ in text, sometimes in notes, and in the numbering of numbers.
  Text differences are absorbed by aligning against the reference text.

**Score reading errors: what they are and what causes them** (measured on the Act II
finale, Ricordi scan, 63 pages, 2026-11-07)

Audiveris reads each staff's bar on its own, and its MusicXML gives every bar the total of
the durations it read in that staff. When those durations are wrong, the staves of one
system disagree on how long the bar is. Each part's timing is the running sum of its own
bars, so the parts drift apart: by bar 10 they were up to 17 beats apart. That alone made
the first alignment fail (singing agreement 54%, track 3 empty).

Measured against a shared bar grid (the median reading, smoothed and snapped to a metre),
the bar lengths read were:

| | bars | right | 1 beat long | ½ beat long | ½ beat short | 1 beat short | half or double |
|---|---|---|---|---|---|---|---|
| Voices | 2,230 | 58% | 9% | 8% | 7% | 5% | 7% |
| Piano | 1,438 | 51% | 15% | 10% | 5% | 4% | 7% |

(The grid is itself an estimate, so a few of these "errors" are the grid's.) The dominant
errors and their likely causes:
- **Small, whole-value errors of ½ or 1 beat** are the bulk. They fit misread durations: a
  flag or beam miscounted (an eighth read as a quarter), a missed augmentation dot, or a
  missed or invented rest. Lyrics, hyphens and dynamics printed close to the notes in a
  dense vocal score are a plausible source of the misreads.
- **Piano bars with two voices on a staff** (melody plus accompaniment) go wrong more
  often: 41% of the wrong piano bars have several voices, against 25% of the right ones.
  When the voices aren't separated correctly, their durations add up instead of
  overlapping.
- **Half and double lengths** suggest whole runs of values read at the wrong level, or a
  metre change the grid doesn't follow.
- **Time signatures are mostly missed:** only 3 changes were detected, though the finale
  changes metre several times. Without the right metre, Audiveris can't check that a bar
  adds up, so a wrong bar goes through silently.
- **Tuplets are never recognised** (0% of bars), so any triplet figures come out with the
  wrong length.
- **Barlines:** bar 1 came out 6–10 beats long (a pickup or a missed barline), and the bar
  counts are off. The Ricordi Sextet read 144 bars where there are 140.
- **The text run** splits the score into many parts, because misread staff labels look like
  new instruments. This finale came out as 129 parts for at most 8 singers plus piano, and
  one part holds different singers in different systems.

What we do about it now:
- **The shared bar grid** (`finale2_score.bar_grid`) makes the staves agree at every barline
  and spreads each staff's notes over the bar in proportion. It fixed the drift: singing
  agreement went from 54% to 71%, all three Solti tracks start at the right music, and the
  text match rose from 81–84% to 85–88%, because syllables now come in the right order.
- **Characters are identified per system,** from the order of the staves, scene membership,
  label, clef, and how well the words match each character's text. They aren't taken from
  Audiveris parts.

Still to try:
- Take the metre from the page map or the libretto sections instead of guessing it.
- Have Audiveris check bars against the metre (its rhythm options).
- A cleaner scan or a different edition of the score.
- A second reader to vote with: oemer, or a vision model reading the vocal lines.
- Using the bar grid only between system or page anchors.

**More from the finale**
- **The score has no tempo.** Every bar is read at a nominal 120 bpm, so a Molto andante
  in 3/8 comes out about 3× too short against a 4/4 Allegro, and the alignment squeezes
  it. In both recordings this put "Che sbaglio mai presi" and the lines around it 15–30 s
  early. A second pass that rescales each metre section to the tempo the first pass
  found helped only partly, because the first pass is already wrong there. The tempo
  marks in the score (Molto andante, Allegro…) or the track boundaries could give each
  section a sensible starting tempo.
- **Track boundaries are free evidence.** Labels cut tracks at section starts, and the
  titles often quote the first words ("Susanna, son morta…"). Nothing uses this yet.
- **Page boundaries from the page map and from Audiveris differ** by a system or two, so
  per-page facts (who is on stage) must be soft constraints, not hard ones.
- **Filled-in phrases have no timing of their own.** Phase 8 let them span the whole gap
  to the next matched phrase: 20–30 s lines that overlapped everyone and turned most of
  the finale into ensemble cards. Capping a phrase at two bars and splitting lines at
  gaps fixed that. Lines in cards went from 90% to about 60%, with dialogue back in the
  list.
- **Dialogue on a shared staff:** Ricordi sometimes writes alternating speakers on one
  staff, with names above the notes. The per-staff character assignment can't separate
  them, and phrase-level reassignment by text was mostly noise. That is a large part of
  the 13 speeches never found ("Sei sola?", "Cherubin?"…).
- **A pitch-register check per minute** (`finale2_check.py`) flags slid stretches, but
  few lines are sung alone in an ensemble finale, and pYIN on the Solti stem often jumps
  to its upper limit. A weak signal; it did agree with the known bad spot.

**Born-digital scores: decode, don't recognise** (Bärenreiter BA 4565, 2026-11-08)

The user's own Bärenreiter vocal score is a vector PDF set in Finale. Staff lines and
barlines are drawn lines; notes, clefs, rests and accidentals are Maestro-font characters
at exact positions; lyrics (Italian upright, German italic), staff labels, bar numbers
and tempo marks are real text. `ba_vector.py` decodes the whole finale (110 pages) in 2
seconds:
- 940 bars, matching the printed bar number on every system (the decoder's count; the score
  itself has 939, because the closing double barline was counted as one more bar);
- 18,968 notes, with pitches from staff position, clef, key and accidentals (every
  singer's range checked);
- 8,813 Italian syllables, each attached to its note;
- every staff labelled with its character;
- all 10 tempo and metre changes.

Bar seconds come from the printed tempo mark and metre (one generic table), so slow
sections are no longer squeezed.

| Finale | Ricordi scan + Audiveris | Bärenreiter vector |
|---|---|---|
| Score text letters matching the reference | 82–89% | 99–100% |
| Speeches never found | 13 / 233 | 0 / 233 |
| Singing agreement, Solti / Currentzis | 70% / 74% | 83% / 88% |
| Entries after silence: median offset | 0.63 / 0.60 s | 0.10 / 0.07 s |
| Entries within 1 s | 62% / 74% | 91% / 95% |

Implications for scaling:
- **Prefer a born-digital edition wherever one is owned.** The decoder is specific to the
  engraving software and music font (Finale with Maestro here; Sibelius, Dorico and
  LilyPond use other fonts), so it needs a small font map per engraver.
- **Licensing:** these PDFs are licensed copies (the footer names the buyer), so they stay
  local and nothing derived from their pages is shared. Only timings and our own text go
  into the interchange files.
- **Cross-check: Audiveris on the same clean engraving** (rendered pages, notes run).
  Bar counts came out within 1–2 of the real 940. The staves agree on a bar's length in
  74–79% of bars, against 24% on the Ricordi scan, but the score still splits into
  54–65 parts. So the scan's quality explains most of Audiveris's errors, but even on a
  clean engraving it isn't exact; the vector decode is.
- **Scans stay the fallback.** Take tempo from the printed tempo marks there too; the
  Audiveris text run can read them.

**Recordings**
- **What the recording leaves out or adds:** cut recitatives, the alternative arias
  (Susanna's "Al desio"), omitted or added repeats, ornaments and appoggiaturas. Score
  alignment copes with small differences. A whole cut section needs detecting, through
  poor harmonic fit and singing that doesn't match where the score expects it.
- **Track boundaries don't match number boundaries,** and every label splits them
  differently. The track-to-number step must allow many-to-many.
- **Tuning:** period pitch (Currentzis about A=431) is detected automatically already.
- **Live recordings:** applause, stage noise and audience laughter. Untested.

**Text and translation**
- **The reference text is the biggest manual step today.** Writing the finale's text by
  hand (8 characters, about 250 phrases) took a large share of the effort. An LLM pass can
  produce it from the libretto. The checks:
  - every libretto speech is covered;
  - the phrases rejoin into the libretto text exactly;
  - the gloss has one English phrase per Italian phrase;
  - the score-reading text matches it (letter match rate, as phase 8 reports).
- **Variants per singer** ("confusa/confuso", "li/ci", "conoscerla/mi/vi") need resolving
  into each character's own words.
- **Singspiel (Zauberflöte, Entführung):** the spoken dialogue is often cut or rewritten in
  recordings. Use speech recognition (Whisper) or forced alignment with a loose text match,
  not a fixed text.
- **Other languages** (German, French, Russian): the MMS aligner covers them. Tesseract
  needs the language data, and Russian needs Cyrillic transliteration for the aligner.
- **Chorus:** a "character" with several voices on one staff (SATB). Untested.

**Matching in roon-rd**
- A track missing from the file can match another track: Currentzis track 52 showed the
  Sextet. Titles share the opera prefix and need it stripped before scoring.
- Roon's album and track titles differ from the file tags. Build titles from Roon
  (browse API) or match on track length plus number. Don't trust the tags alone.

**Process**
- **Hidden order dependencies:** phase 8 rewrote a track and dropped phase 10's lines.
  Every stage should write its own per-number output, with one final step merging them into
  the interchange file. No stage should edit another's output.
- **Sextet assumptions are baked in:** the cast list, staff indices, 4/4 time, file names,
  "find the Riconosci track". All of it must move into a per-opera configuration.
- **Storage:** bulk caches outside Dropbox (`~/Development/libretto-work`); audio trees
  read-only; the torch model cache needs access outside the sandbox. The experiment code
  needs the same split.
- **Dependencies:** `torchaudio` and `demucs` aren't in `pyproject.toml` yet; Audiveris is
  a desktop app run in batch mode, with its OCR language data installed by hand.

## Checking without a human

We can't listen to every number of every recording, so each built number gets automatic
checks and a pass, warn or fail. A build report lists the numbers flagged for a listen. The
checks we already have:

| Check | What it catches | Seen so far |
|---|---|---|
| Every line accounted for (in the list or in a card) | Viewer boundary bugs, lost lines | 100% in every track |
| Score-reading text match rate against the reference | Wrong staff → character assignment, bad score reading | 92–97% |
| Singing agreement (score against vocal stem) | Misplaced or stretched alignment | 84–87% |
| Entries after a silence found, and their offset | Timing drift | median 0.2–0.45 s |
| Words or phrases mostly in silence | Forced-alignment errors | 0–5% |
| Speech register against the character's voice type | Wrong speaker turns in recitative | men below 0.75× women's median |
| Reprise contour agreement (where a number repeats material) | Gross aria misalignment | 0.16–0.28 semitones, against a control of about 1.5 |
| Harmonic fit | Score spread over the wrong music, cut sections | separates right from wrong placements |

## Shape of the tools (proposal, for discussion at step 5)

- **One CLI with subcommands per stage**, each reading and writing a cache directory keyed
  by opera, number and recording, and each idempotent: `fetch-score`, `page-map`, `omr`,
  `reference-text`, `score-lines`, `find-recording`, `stems`, `align`, `report`, `publish`.
- **A small manifest per opera,** most of it generated by the first stages and corrected
  only by exception: work id, score source, the list of numbers and their pages, the cast
  in score order with voice types, and which numbers are recitativo secco.
- **A recording manifest generated from tags,** mapping track files to numbers.
- **A build report per opera and recording** (an HTML page): per number, the checks above,
  flagged items, and a link to play that number with the viewer.
- **Stay automatic where it already is.** Hand-marking stays out of the design; the only
  human step is listening to what the report flags.

## Feature ideas (user, 2026-11-08)

- **Bar locations alongside times.** Show where each line sits in the score ("bars 126–129",
  or bar and beat for a phrase), which helps when following along in a printed score.
  - **Born-digital scores:** already available. The vector decode gives every syllable its
    bar number (matching the printed numbers) plus its position within the bar, so lines
    and phrases can carry `bar_start`/`bar_end` in the interchange file at no extra cost.
  - **Scans:** Audiveris's bar numbering drifts (Ricordi finale: 918 bars read against 940
    real). It would need re-anchoring on the printed bar numbers, or on the page/system
    map, before it can be shown.
  - **Edition caveat:** bar numbers differ between editions (Bärenreiter's numbering
    restarts per number; older editions may count differently). Label the edition,
    and offer page/system references too where the score is the user's own copy.
  - **Viewer:** a small bar number beside each line or card in the list, and in the
    stanza view the current bar, perhaps behind a toggle like the translation one.
