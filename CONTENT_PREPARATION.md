# Libretto: Content Preparation & Development Plan

## Libretto Sources

### Le nozze di Figaro (The Marriage of Figaro)

Our primary target. Lorenzo Da Ponte's libretto (1786) is firmly in the public domain.

**Best online sources found:**

| Source | URL | What it offers |
|--------|-----|----------------|
| **opera-arias.com** (Italian) | opera-arias.com/mozart/le-nozze-di-figaro/libretto/ | Full Italian original with musical numbers labeled (`No. 1 - Duettino`, `No. 6 - Aria`, etc.), character names in caps, stage directions inline, `Recitativo` sections marked, acts and scenes numbered. Clean, well-structured text. |
| **opera-arias.com** (English) | opera-arias.com/mozart/le-nozze-di-figaro/libretto/english/ | Full English translation, same structural conventions. Character names in caps, stage directions present, act/scene/number structure preserved. |
| **murashev.com** (side-by-side) | murashev.com/opera/Le_nozze_di_Figaro_libretto_English_Italian | Italian-English side-by-side. Also offers Italian-German and Italian-French pairs. Available as paperback too. |
| **Stanford OperaGlass** | opera.stanford.edu/iu/libretti/figaro.htm | Italian original (currently offline but historically reliable). |
| **Library of Congress / HathiTrust** | catalog.hathitrust.org/Record/009789797 | Historical public domain scan (c.1888) with Italian and English. PDF. |
| **Online Library of Liberty** | oll-resources.s3.amazonaws.com/.../Mozart_1398_EBk_v6.0.pdf | PDF with Italian text and English translation plus musical excerpts. |

**Recommendation:** Use both sites as complementary sources — opera-arias.com for clean structured text, murashev.com for pre-aligned side-by-side translations. See Site Investigation below for technical details on scraping each.

### General Libretto Sources (for future operas)

| Source | Coverage | Format |
|--------|----------|--------|
| **opera-arias.com** | ~470 operas, 2800+ arias | HTML, clean text with structure |
| **murashev.com** | Major repertoire, multi-language | HTML, side-by-side pairs |
| **Stanford OperaGlass** | Broad classical repertoire | HTML (currently down) |
| **librettidopera.it** | Italian opera focus, extensive | HTML, Italian with translations |
| **HathiTrust / Internet Archive** | Historical printed librettos | PDF/scan (OCR quality varies) |
| **Kareol** (kareol.es) | Large Spanish-language resource | HTML, original + Spanish translations |
| **IMSLP** | Scores with embedded lyrics | PDF (musicxml sometimes) |

---

## Site Investigation

### opera-arias.com

**Rendering:** Server-side. The full libretto text is present in the initial HTML response — no JavaScript required to access the content. Our `read_url_content` tests confirmed this: every act's text came through cleanly.

**HTML structure:** Content is in a single page per language per opera. The text is largely plain with minimal HTML markup. Character names, stage directions, act/scene headers, and musical number labels are all present as text with consistent formatting conventions (ALL CAPS for characters, `No. N - Type` for numbers, etc.).

**URL pattern:**
- Italian: `opera-arias.com/mozart/le-nozze-di-figaro/libretto/`
- English: `opera-arias.com/mozart/le-nozze-di-figaro/libretto/english/`
- Generalizes to: `opera-arias.com/{composer}/{opera-slug}/libretto/[{language}/]`

**Coverage:** ~470 operas, 2800+ arias. Broad repertoire.

**Scraping difficulty:** **Easy.** Standard HTTP GET + HTML parsing. The `scraper` crate (Rust equivalent of BeautifulSoup) with CSS selectors should handle extraction cleanly.

**Content quality:** Very good. Clean Unicode, consistent formatting, musical numbers labeled.

### murashev.com

> **Update (2025-02-14):** Our initial assessment that murashev.com was a JavaScript SPA was incorrect. Detailed investigation (see `notes/murashev-api-investigation.md`) revealed it is classic server-rendered HTML. The earlier text-extraction tool simply failed to parse the HTML table structure, giving the false impression that content was loaded dynamically.

**Rendering:** Server-side. The full libretto text is present in the initial HTML response. The site uses jQuery 1.7.1 (only for show/hide toggles on navigation sections) and Dreamweaver templates. No AJAX, no SPA framework, no dynamic content loading.

**Side-by-side structure:** A two-column HTML table (`<table width="100%">`), one `<td width="50%">` per language. Each `<tr>` is a **paragraph-aligned segment pair** — the two cells contain the same passage in English and Italian. The entire opera (all 4 acts, 28 musical numbers) is served in a single page with **140 rows**. One HTTP request gets everything.

**Semantic HTML markup:**
- `<span class="act"><act>ACT ONE</act></span>` — Act/section headers (custom `<act>` tag)
- `<b>No. 1: Duettino</b>` (EN) / `<b>N° 1: Duettino</b>` (IT) — Musical number labels
- `<i>...</i>` — Stage directions
- Character names in plain text ALL CAPS (`FIGARO<br />`)
- `<br />` for line breaks within cells

**URL pattern — side-by-side (preferred):**
- `murashev.com/opera/Le_nozze_di_Figaro_libretto_English_Italian`
- Also: `_Italian_English`, `_Italian_German`, `_Italian_French`, `_English_German`, `_English_French`
- The `_Act_N` suffix pages return the full opera regardless — the suffix is for navigation only.

**URL pattern — single language:**
- `murashev.com/opera/Le_nozze_di_Figaro_libretto_Italian`
- Uses a single-column table (`width="80%"`) with 38 rows per act.

**Coverage:** Major repertoire. Multi-language pairs (Italian, English, German, French). Also sells paperback editions via Amazon.

**No robots.txt** — returns 404.

**Scraping difficulty:** **Easy.** Standard HTTP GET + HTML parsing. Same approach as opera-arias.com: `reqwest` + `scraper` crate. No headless browser needed.

### Comparison

| Aspect | opera-arias.com | murashev.com |
|--------|----------------|---------------|
| **Rendering** | Server-side (static HTML) | Server-side (static HTML) |
| **Scraping** | `reqwest` + `scraper` | `reqwest` + `scraper` |
| **Languages per page** | 1 | 2 (side-by-side) |
| **Alignment** | Must align EN/IT post-hoc | Pre-aligned in table rows |
| **Requests per opera** | 2 (IT page + EN page) | 1 (side-by-side page) |
| **Rust crates** | `reqwest` + `scraper` | `reqwest` + `scraper` |
| **Best for** | Clean text, broader coverage | Pre-aligned bilingual pairs |

---

## What Raw Libretto Text Looks Like

Having examined the opera-arias.com texts closely, here is the structure we consistently find:

```
ATTO PRIMO

SCENA I
Figaro con una misura in mano e Susanna allo specchio ...

No. 1 - Duettino

FIGARO misurando
Cinque... dieci.... venti... trenta...
trentasei...quarantatre

SUSANNA specchiandosi
Ora sì ch'io son contenta;
sembra fatto inver per me.
Guarda un po', mio caro Figaro,
guarda adesso il mio cappello.

FIGARO
Sì mio core, or è più bello,
sembra fatto inver per te.

SUSANNA e FIGARO
Ah, il mattino alle nozze vicino
quanto è dolce al mio/tuo tenero sposo
questo bel cappellino vezzoso
che Susanna ella stessa si fe'.

Recitativo

SUSANNA
Cosa stai misurando,
caro il mio Figaretto?
```

### Patterns We Can Exploit

The raw text has remarkably consistent structural signals:

1. **Act markers** — `ATTO PRIMO`, `ATTO SECONDO`, etc. (Italian); `ACT ONE`, `ACT TWO`, etc. (English)
2. **Scene markers** — `SCENA I`, `SCENA II`, etc.; `SCENE ONE`, `SCENE TWO`, etc.
3. **Musical number labels** — `No. 1 - Duettino`, `No. 6 - Aria`, `No. 10 - Aria`, `No. 7 - Terzetto`, etc. These are critical — they correspond to tracks in most recordings.
4. **Character names** — ALL CAPS at the start of a speech: `FIGARO`, `SUSANNA`, `IL CONTE`, `LA CONTESSA`, `CHERUBINO`, etc.
5. **Stage directions** — Lowercase text after a character name on the same line (e.g., `FIGARO misurando`) or standalone descriptive paragraphs between speeches.
6. **Recitative markers** — `Recitativo` / `Recitative` lines explicitly mark the transition from structured musical numbers to recitative.
7. **Ensemble labels** — `SUSANNA e FIGARO`, `SUSANNA, CONTESSA e FIGARO`, etc.
8. **Cast list** — Appears at the top with character names, voice types, and descriptions.

These patterns are regular enough that a parser can handle the bulk of the structuring automatically, with human review for edge cases.

---

## Preparation Workflow

### Step 1: Acquire Raw Text (automated)

The acquisition tool (`libretto acquire`) fetches libretto text from configured sources and normalizes it into clean plain text files.

**What the tool does:**
1. Takes an opera identifier (e.g., `mozart/le-nozze-di-figaro`) and source site.
2. Fetches the HTML (or API response) for each language.
3. Extracts the libretto text, stripping HTML/navigation while preserving structural markers.
4. Normalizes Unicode, whitespace, and line breaks.
5. Writes clean `.txt` files plus a `source.md` with provenance.

**Output:**
```
raw/
  le-nozze-di-figaro/
    italian.txt         # Clean extracted text
    english.txt         # Clean extracted text
    source.md           # URLs, fetch date, attribution, license notes
```

For murashev.com side-by-side mode, the tool can additionally output a `bilingual.json` with the pre-aligned paragraph pairs, which the parser can use to guarantee correct Italian/English segment matching.

### Step 2: Parse into Structured JSON (the Parser)

Write a parser tool that reads the raw text and produces a structured base libretto JSON. The parser should:

1. **Extract the cast list** from the opening section.
2. **Split by act and scene** using the act/scene markers.
3. **Split by musical number** using `No. N - Type` markers and `Recitativo` markers.
4. **Split by character** using ALL-CAPS character names.
5. **Separate stage directions** from sung/spoken text.
6. **Pair Italian and English** segment-by-segment — since both texts share identical structure (same acts, scenes, numbers, characters in the same order), the parser can walk both files in parallel.

The parser doesn't need to be perfect on the first pass. It should produce structured output that a human can quickly review and correct.

#### Parser Output: Base Libretto JSON

The parser produces an **untimed** base libretto file — the reusable, recording-independent text layer described in DESIGN.md:

```json
{
  "version": "1.0",
  "opera": {
    "title": "Le nozze di Figaro",
    "title_translation": "The Marriage of Figaro",
    "composer": "Wolfgang Amadeus Mozart",
    "librettist": "Lorenzo Da Ponte",
    "language": "it",
    "translation_language": "en",
    "year": 1786,
    "catalogue": "K. 492"
  },
  "cast": [
    {
      "character": "Il Conte d'Almaviva",
      "voice_type": "baritone",
      "description": "a Spanish grandee"
    },
    {
      "character": "La Contessa d'Almaviva",
      "voice_type": "soprano",
      "description": "his wife"
    }
  ],
  "numbers": [
    {
      "number_id": "overture",
      "number_label": "Overture",
      "type": "instrumental",
      "act": "I",
      "scene": "1",
      "segments": []
    },
    {
      "number_id": "no-1-duettino",
      "number_label": "No. 1 - Duettino",
      "type": "duet",
      "act": "I",
      "scene": "1",
      "segments": [
        {
          "segment_id": "no-1-001",
          "character": "Figaro",
          "direction": "misurando",
          "text": "Cinque... dieci.... venti... trenta...\ntrentasei...quarantatre",
          "translation": "Five ... ten ... twenty ... thirty ...\nThirty-six ... forty-three"
        },
        {
          "segment_id": "no-1-002",
          "character": "Susanna",
          "direction": "specchiandosi",
          "text": "Ora sì ch'io son contenta;\nsembra fatto inver per me.\nGuarda un po', mio caro Figaro,\nguarda adesso il mio cappello.",
          "translation": "Yes, I'm very pleased with that;\nIt seems just made for me.\nTake a look, dear Figaro,\nJust look at this hat of mine."
        },
        {
          "segment_id": "no-1-003",
          "character": "Figaro",
          "text": "Sì mio core, or è più bello,\nsembra fatto inver per te.",
          "translation": "Yes, my dearest, it's very pretty;\nIt looks just made for you."
        },
        {
          "segment_id": "no-1-004",
          "character": "Susanna e Figaro",
          "text": "Ah, il mattino alle nozze vicino\nquanto è dolce al mio/tuo tenero sposo\nquesto bel cappellino vezzoso\nche Susanna ella stessa si fe'.",
          "translation": "On this morning of our wedding\nHow delightful to my (your) dear one\nIs this pretty little hat\nWhich Susanna made herself."
        }
      ]
    },
    {
      "number_id": "rec-1a",
      "number_label": "Recitativo",
      "type": "recitative",
      "act": "I",
      "scene": "1",
      "segments": [
        {
          "segment_id": "rec-1a-001",
          "character": "Susanna",
          "text": "Cosa stai misurando,\ncaro il mio Figaretto?",
          "translation": "What are you measuring,\nMy dearest Figaro?"
        }
      ]
    }
  ]
}
```

#### Why Organize by Musical Number?

This is the crucial structural choice, and it directly serves the timing workflow:

- **Most recordings are tracked by musical number.** A typical Figaro recording has one track per number (or small groups). The musical number is the natural join point between libretto text and audio tracks.
- **Recitatives are often on separate tracks** from the arias/ensembles they precede. Structuring by number makes the track-to-text mapping straightforward.
- **The timing tool can load one number at a time.** The annotator works through `No. 1 - Duettino` (one track), then `Recitativo` (next track), then `No. 2 - Duettino` (next track), etc.
- **Different recordings may group numbers differently** (e.g., some combine a recitative with the following aria into one track). The timing overlay handles this mapping.

### Step 3: Human Review

The parser output is reviewed and corrected:

- Fix any character attribution errors.
- Verify stage directions were separated correctly.
- Check Italian/English segment alignment.
- Correct any OCR or encoding artifacts.

This can be done by editing the JSON directly or (better) through a simple review UI.

### Step 4: Store in the Library

The reviewed base libretto is committed to the library repository.

---

## Repository Structure

```
libretto-library/
│
├── README.md                           # Project overview, how to use, how to contribute
├── CONTRIBUTING.md                     # Contribution guidelines
├── LICENSE                             # CC-BY-SA or similar
│
├── schema/
│   ├── base-libretto.schema.json       # JSON Schema for base libretto files
│   ├── timing-overlay.schema.json      # JSON Schema for timing overlay files
│   └── track-map.schema.json           # JSON Schema for track-map files
│
├── tools/
│   ├── parse-libretto/                 # Parser tool (raw text → base libretto JSON)
│   │   ├── README.md
│   │   └── ...
│   ├── validate/                       # Validation tool (checks schema conformance)
│   │   └── ...
│   └── align/                          # Future: timing/annotation tool
│       └── ...
│
├── operas/
│   ├── mozart/
│   │   ├── le-nozze-di-figaro/
│   │   │   ├── raw/
│   │   │   │   ├── italian.txt         # Raw source text (Italian)
│   │   │   │   ├── english.txt         # Raw source text (English)
│   │   │   │   └── source.md           # Attribution, URLs, license notes
│   │   │   │
│   │   │   ├── base.libretto.json      # Parsed & reviewed base libretto (untimed)
│   │   │   │
│   │   │   └── timings/
│   │   │       ├── README.md           # Notes on available timing overlays
│   │   │       ├── giulini-1959-emi.timing.json
│   │   │       ├── solti-1982-decca.timing.json
│   │   │       └── currentzis-2014-sony.timing.json
│   │   │
│   │   ├── don-giovanni/
│   │   │   ├── raw/ ...
│   │   │   ├── base.libretto.json
│   │   │   └── timings/ ...
│   │   │
│   │   └── cosi-fan-tutte/
│   │       └── ...
│   │
│   ├── puccini/
│   │   ├── la-boheme/
│   │   ├── tosca/
│   │   └── madama-butterfly/
│   │
│   ├── verdi/
│   │   ├── la-traviata/
│   │   ├── rigoletto/
│   │   └── otello/
│   │
│   └── ...
│
└── track-maps/
    ├── README.md                       # How track maps work
    └── examples/
        └── figaro-giulini.track-map.json
```

### Naming Conventions

- **Opera directories** — Lowercase, hyphenated: `le-nozze-di-figaro`, `la-boheme`, `don-giovanni`
- **Composer directories** — Lowercase: `mozart`, `puccini`, `verdi`
- **Base libretto** — Always `base.libretto.json` (one per opera)
- **Timing overlays** — `{conductor}-{year}-{label}.timing.json` (e.g., `giulini-1959-emi.timing.json`)
- **Raw text** — `italian.txt`, `english.txt`, `german.txt`, etc. in `raw/`

### What Goes in a Timing Overlay

The timing overlay references the base libretto by its segment IDs and adds recording-specific data:

```json
{
  "version": "1.0",
  "base_libretto": "mozart/le-nozze-di-figaro/base.libretto.json",
  "recording": {
    "conductor": "Carlo Maria Giulini",
    "orchestra": "Philharmonia Orchestra",
    "year": 1959,
    "label": "EMI",
    "album_title": "Le nozze di Figaro (Giulini)"
  },
  "contributors": [
    { "name": "Jane Doe", "role": "timing", "date": "2026-02-15" }
  ],
  "track_timings": [
    {
      "track_title": "Overture",
      "disc_number": 1,
      "track_number": 1,
      "duration_seconds": 258.0,
      "number_ids": ["overture"],
      "segment_times": []
    },
    {
      "track_title": "Cinque... dieci... venti...",
      "disc_number": 1,
      "track_number": 2,
      "duration_seconds": 195.0,
      "number_ids": ["no-1-duettino"],
      "segment_times": [
        { "segment_id": "no-1-001", "start": 0.0 },
        { "segment_id": "no-1-002", "start": 12.5 },
        { "segment_id": "no-1-003", "start": 28.0 },
        { "segment_id": "no-1-004", "start": 35.5 }
      ]
    },
    {
      "track_title": "Cosa stai misurando",
      "disc_number": 1,
      "track_number": 3,
      "duration_seconds": 310.0,
      "number_ids": ["rec-1a", "no-2-duettino"],
      "segment_times": [
        { "segment_id": "rec-1a-001", "start": 0.0 },
        { "segment_id": "rec-1a-002", "start": 5.2 }
      ]
    }
  ]
}
```

Key points:
- **`number_ids`** maps recording tracks to musical numbers in the base libretto. A single track may contain multiple numbers (e.g., a recitative followed by an aria).
- **`segment_times`** has only `segment_id` and `start` — the text, character, and translation come from the base libretto. No duplication.
- **`end` is implicit** — derived from the next segment's `start` or the track duration.

---

## Rust Tooling Architecture

All tooling is implemented in Rust as a single CLI binary: `libretto`.

### Crate / Workspace Structure

```
libretto/
├── Cargo.toml                      # Workspace root
├── crates/
│   ├── libretto-cli/              # CLI entry point (clap)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   │
│   ├── libretto-acquire/          # Web scraping / text acquisition
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── opera_arias.rs      # opera-arias.com adapter
│   │       ├── murashev.rs         # murashev.com adapter
│   │       └── normalize.rs        # Text normalization (Unicode, whitespace)
│   │
│   ├── libretto-parse/            # Raw text → structured base libretto JSON
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── cast.rs             # Cast list extraction
│   │       ├── structure.rs        # Act/scene/number splitting
│   │       ├── segments.rs         # Character/text/direction splitting
│   │       └── align.rs            # Italian/English parallel alignment
│   │
│   ├── libretto-model/            # Shared data types (serde structs)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── base_libretto.rs    # BaseLibretto, Number, Segment, Cast, etc.
│   │       └── timing_overlay.rs   # TimingOverlay, TrackTiming, etc.
│   │
│   └── libretto-validate/         # Schema/semantic validation
│       ├── Cargo.toml
│       └── src/
│           └── lib.rs
```

### Key Rust Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing |
| `reqwest` (with `rustls`) | HTTP client for fetching web pages |
| `scraper` | HTML parsing with CSS selectors (both sites) |
| `regex` | Text pattern matching (act/scene/character markers) |
| `serde` / `serde_json` | JSON serialization of base libretto and timing overlay |
| `unicode-normalization` | NFC normalization for Italian diacritics |
| `tokio` | Async runtime (required by reqwest) |
| `chrono` | Timestamps for source provenance |
| `thiserror` / `anyhow` | Error handling |
| `tracing` | Logging |

### CLI Commands

```bash
# Acquire raw text from a source
libretto acquire --source opera-arias --opera mozart/le-nozze-di-figaro --lang it,en
libretto acquire --source murashev --opera mozart/le-nozze-di-figaro --lang it+en

# Parse raw text into base libretto JSON
libretto parse --input raw/ --output base.libretto.json

# Validate a base libretto or timing overlay
libretto validate base.libretto.json
libretto validate timings/giulini-1959-emi.timing.json --base base.libretto.json
```

---

## Development Plan

### Phase 0: Murashev.com Investigation ✅ COMPLETE

See `notes/murashev-api-investigation.md` for detailed findings. Key result: murashev.com is **not** a SPA — it's classic server-rendered HTML. The full libretto text is in the initial HTTP response as a two-column `<table>` with pre-aligned bilingual paragraph rows. Scraping uses the same `reqwest` + `scraper` approach as opera-arias.com. No headless browser needed.

### Phase 1: Project Scaffolding (~1 day)

1. Initialize the Cargo workspace under `libretto/`.
2. Create the crate structure: `libretto-cli`, `libretto-model`, `libretto-acquire`, `libretto-parse`, `libretto-validate`.
3. Define the core data model in `libretto-model`: `BaseLibretto`, `Opera`, `Cast`, `Number`, `Segment`, `TimingOverlay`, etc. as Rust structs with serde derive.
4. Set up `clap` CLI skeleton with subcommands: `acquire`, `parse`, `validate`.
5. Add basic tests.

### Phase 2: Acquisition Tool (~2-3 days)

Build `libretto acquire` with per-source adapters.

**opera-arias.com adapter:**
1. Fetch page with `reqwest`.
2. Parse HTML with `scraper`, extracting the main content area.
3. Walk the DOM, converting to clean text while preserving structural markers.
4. Handle multi-page or single-page formats.
5. Normalize Unicode (`unicode-normalization` crate, NFC form).
6. Write `italian.txt`, `english.txt`, `source.md`.

**murashev.com adapter:**
1. Fetch the side-by-side page with `reqwest` (one request per opera).
2. Parse HTML with `scraper`, selecting the libretto table: `table[width="100%"][border="0"]`.
3. Iterate `<tr>` rows. Each row has two `<td width="50%">` cells: Language 1 (EN) and Language 2 (IT).
4. Within each cell, extract structural markers: `<act>` tags → act boundaries, `<b>` tags → musical numbers, `<i>` tags → stage directions, ALL-CAPS text → character names.
5. Split on `<br />` for line-level granularity.
6. Normalize Unicode.
7. Write `italian.txt`, `english.txt`, `bilingual.json` (pre-aligned pairs), `source.md`.

**Shared concerns:**
- Disk-based HTTP response cache (don't re-fetch pages we've already downloaded).
- Rate limiting (configurable delay between requests).
- `source.md` generation with URL, fetch timestamp, and attribution.

### Phase 3: Parser Tool (~3-5 days)

Build `libretto parse` to transform raw text into structured JSON.

**Parser responsibilities:**
1. Extract the cast list from the opening section.
2. Split text into acts/scenes using markers.
3. Identify musical numbers (`No. N - Type`) and recitative blocks.
4. Within each number, split by character (ALL-CAPS detection).
5. Separate stage directions from sung text.
6. Generate `segment_id` values.
7. Walk Italian and English files in parallel, pairing segments.
8. If `bilingual.json` exists (from murashev.com), use its pre-aligned pairs for higher-confidence matching.
9. Output a `BaseLibretto` serialized as JSON.

**Testing:** Unit tests for each parsing stage, plus an integration test that parses the full Figaro Italian text and checks the output structure.

### Phase 4: Validation Tool (~1-2 days)

1. Schema validation: check that JSON files deserialize correctly into the model types.
2. Semantic validation:
   - All `segment_id` values are unique within a libretto.
   - Musical numbers are ordered by act/scene.
   - Character names in segments match the cast list.
   - Timing overlays reference valid segment IDs from the base libretto.
   - No unreasonably short (<0.5s) or long (>120s) timed segments.
3. Report validation errors with file/line context.

### Phase 5: Le nozze di Figaro End-to-End (~2-3 days)

1. Run `libretto acquire` on both sources for Figaro.
2. Run `libretto parse` on the acquired text.
3. Run `libretto validate` on the output.
4. Human review and correction of the base libretto.
5. Commit the reviewed `base.libretto.json`.

### Phase 6: Repository Setup (~1 day)

1. Create the `libretto-library` GitHub repository (content only, separate from the Rust tooling).
2. Set up the directory structure.
3. Add README.md, CONTRIBUTING.md, LICENSE.
4. Add the reviewed Figaro base libretto.
5. Set up CI with `libretto validate` running on PRs.

### Phase 7: Second Opera (validation, ~2-3 days)

Parse and review a second opera to validate that the tooling and format generalize:
- **Don Giovanni** (Mozart/Da Ponte) — same librettist, similar structure, tests robustness.
- **La Bohème** (Puccini/Illica & Giacosa) — different era, different conventions, tests flexibility.

---

## What "Ready for Timing Tool Ingestion" Looks Like

At the end of this plan, we have:

1. **A reviewed base libretto** for Le nozze di Figaro (and at least one more opera) in `base.libretto.json` — structured by musical number, with segment IDs, character attributions, Italian text, English translations, and stage directions.

2. **A defined timing overlay schema** — so the timing tool knows exactly what to produce: a list of `{ segment_id, start }` pairs per track, plus track metadata.

3. **A Rust CLI tool (`libretto`)** with three commands:
   - `acquire` — automated web scraping to fetch raw libretto text.
   - `parse` — transform raw text into structured base libretto JSON.
   - `validate` — check files for schema conformance and semantic correctness.

4. **A repeatable pipeline** — adding a new opera is: `acquire` → `parse` → review → `validate` → commit.

The timing tool (to be built later) will:
- Load a `base.libretto.json` and display the segments one at a time.
- Play audio (or sync with roon-rd playback).
- Let the operator tap to mark each segment's start time.
- Output a `timing.json` overlay file referencing the base libretto's segment IDs.

The base libretto is the input; the timing overlay is the output. Everything in this plan is about getting the input right.
