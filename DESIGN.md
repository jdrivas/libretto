# Liberetto: Real-Time Opera Libretto Display

## Overview

Liberetto solves the problem of displaying opera libretto text synchronized with music playback. The system has two distinct parts:

1. **Annotation & Interchange** — Given a plain-text libretto, produce a time-annotated interchange format that maps text segments to timestamps within a music track.
2. **Synchronized Display** — Consume the interchange format and display the appropriate libretto text in real time during playback, initially integrated with `roon-rd`.

---

## Part 1: Annotation & Interchange Format

### Problem

We start with the full text of an opera libretto and one or more audio tracks (typically individual acts or scenes). We need a way to:

- Segment the libretto into display units (lines, couplets, or passages).
- Associate each segment with a time range (start time, and optionally end time) relative to the corresponding audio track.
- Capture metadata: which character is singing, stage directions, act/scene structure.

### Annotation Workflow

Annotating a libretto is inherently manual or semi-automated work. The operator listens to the recording and marks when each passage begins. Possible approaches:

- **Manual tap-along tool** — A simple utility that plays the audio and lets the operator tap a key each time the next libretto segment begins. The tool records the timestamp for each tap and pairs it with the next un-timed segment. This is the simplest v1 approach.
- **Forced alignment (future)** — Speech-to-text forced alignment tools (e.g., Aeneas, Gentle, or Montreal Forced Aligner) can automatically align known text to audio. Opera singing is significantly harder than speech for these tools, but it may work for recitative passages and could serve as a starting point that is then hand-corrected.
- **AI-assisted (future)** — Large audio models or music-aware transcription systems could potentially produce rough alignments that are then refined manually.

For v1, the manual tap-along approach is the most pragmatic.

### Interchange Format

The interchange format is a JSON file that fully describes the timed libretto for a single track (or a set of tracks comprising a complete opera).

#### Design Goals

- **Human-readable and hand-editable** — JSON with clear structure.
- **Sufficient metadata** — Track identification, language, character names, act/scene structure.
- **Time precision** — Millisecond-resolution timestamps (floating-point seconds).
- **Extensible** — Additional fields can be added without breaking consumers.

#### Proposed Schema

```json
{
  "version": "1.0",
  "opera": {
    "title": "La Bohème",
    "composer": "Giacomo Puccini",
    "language": "it",
    "translation_language": "en"
  },
  "tracks": [
    {
      "track_id": "roon:track:12345",
      "title": "Act I",
      "album": "La Bohème (Pavarotti, Karajan)",
      "artist": "Luciano Pavarotti",
      "duration_seconds": 2145.0,
      "segments": [
        {
          "start": 0.0,
          "end": 15.5,
          "character": "Marcello",
          "text": "Questo Mar Rosso mi ammollisce e assidera\ncome se addosso mi piovesse in stille.",
          "translation": "This Red Sea drenches and chills me\nas if it were dripping down on me.",
          "stage_direction": null,
          "act": "I",
          "scene": "1"
        },
        {
          "start": 15.5,
          "end": 28.3,
          "character": "Rodolfo",
          "text": "Nei cieli bigi guardo fumar dai mille\ncomignoli Parigi.",
          "translation": "In the grey skies I watch the smoke rising\nfrom a thousand chimneys of Paris.",
          "stage_direction": null,
          "act": "I",
          "scene": "1"
        }
      ]
    }
  ]
}
```

#### Key Fields

- **`track_id`** — Identifies the audio track. For Roon, this could be a combination of album name + track title, or a Roon-specific ID. We need to determine the best way to match a libretto file to a playing track (see Part 2).
- **`start` / `end`** — Seconds from the beginning of the track. `end` is optional; if omitted, the segment ends when the next segment begins.
- **`character`** — Who is singing. `null` for orchestral interludes or stage directions.
- **`text`** — The libretto text in the original language. Newlines within the string represent line breaks in the verse.
- **`translation`** — Optional parallel translation.
- **`stage_direction`** — Optional stage direction text (e.g., *"Rodolfo enters"*).
- **`act` / `scene`** — Structural location within the opera.

#### Track Matching

A critical question: how do we match the interchange file to a track that is currently playing? Options:

1. **By album + track title** — The simplest approach. Match the `album` and `title` fields from the interchange file against the now-playing metadata from Roon. Fuzzy matching may be needed since metadata varies between releases.
2. **By Roon track/album ID** — More precise but ties the file to a specific Roon library entry. Could be captured during the annotation step.
3. **By user assignment** — The user explicitly associates a libretto file with a track or album in a configuration file.

For v1, option 3 (user assignment via config) is simplest and most reliable, with option 1 as a convenience fallback.

#### File Organization

```
liberetto/
  data/
    la-boheme-karajan/
      metadata.json          # Opera-level metadata
      act1.libretto.json     # Per-track interchange files
      act2.libretto.json
      ...
    tosca-callas/
      ...
  config/
    track-map.json           # Maps Roon track identifiers to libretto files
```

---

## Part 2: Synchronized Display via roon-rd

### roon-rd Capabilities (Relevant Summary)

`roon-rd` is a Rust-based Roon extension with these key properties:

- **Server mode** runs an Axum HTTP server (default port 3000) with REST API + WebSocket.
- **WebSocket pushes real-time events:**
  - `zones_changed` — Full zone state including track name, artist, album, `position_seconds`, `length_seconds`, image key.
  - `seek_updated` — Per-zone `seek_position` updates (typically every second during playback).
  - `queue_changed` — Queue state changes.
  - `connection_changed` — Roon Core connection status.
- **REST endpoints** provide `/now-playing`, `/zones`, `/seek/:zone_id`, etc.
- **The SPA** is embedded in the server binary as pure HTML/CSS/JS — no framework.
- **Zone data includes:** `zone_id`, `zone_name`, `state` (Playing/Paused/Stopped/Loading), `track`, `artist`, `album`, `position_seconds`, `length_seconds`.

The `seek_updated` WebSocket event is the key synchronization primitive — it provides the current playback position approximately once per second.

### Integration Approach

The goal is to add libretto display as a feature of roon-rd, not as a separate application. This keeps the real-time playback data and the display in the same system.

#### Data Flow

```
Libretto JSON files
        │
        ▼
   roon-rd server ──loads──▶ in-memory libretto data
        │                         │
        │  WebSocket              │  lookup by track
        │  (seek_updated)         │
        ▼                         ▼
   SPA (browser) ◄─── current segment for position
        │
        ▼
   Libretto display panel
```

#### Server-Side Changes

1. **Libretto storage** — roon-rd loads libretto JSON files from a configurable directory at startup (and/or on demand via API).
2. **Track-to-libretto mapping** — A configuration file (`track-map.json`) maps track identifiers to libretto files. When a zone starts playing a track, the server checks if a libretto is available.
3. **New REST endpoints:**
   - `GET /libretto/:zone_id` — Returns the full libretto data for the currently playing track in the given zone (if available), or 404.
   - `GET /libretto/:zone_id/at/:seconds` — Returns the active segment(s) at the given timestamp. Useful for initial sync.
4. **New WebSocket message type:**
   - `libretto_available` — Sent when a track with an associated libretto starts playing. Contains the zone_id and libretto metadata.
   - Alternatively, include a `has_libretto: bool` flag in the existing `zones_changed` payload.

#### Client-Side (SPA) Changes

1. **Libretto panel** — A new UI panel (or overlay) that displays the current libretto text. Shown only when a libretto is available for the playing track.
2. **Sync logic:**
   - On receiving `libretto_available`, fetch the full libretto via REST.
   - On each `seek_updated` event, binary-search the segments array to find the active segment for the current `seek_position`.
   - Update the display with the current segment's text, character name, and optional translation.
   - Smooth scrolling/transitions between segments.
3. **Display considerations:**
   - Show the current line prominently, with surrounding context (previous/next lines) dimmed.
   - Character name displayed above or beside the text.
   - Translation shown below the original text (toggleable).
   - Auto-scroll with the music; manual scroll pauses auto-follow temporarily.
   - Fullscreen-friendly layout for dedicated display use.

#### Seek and Track-Change Handling

- **Seek:** When the user seeks (via Roon or the SPA), the next `seek_updated` event will carry the new position. The SPA re-syncs the libretto display immediately.
- **Track change:** A `zones_changed` event with a new track triggers a check for a libretto associated with the new track. If found, the libretto panel updates; if not, it hides.
- **Pause/Stop:** The libretto display freezes at the last known position on pause, and hides (or resets) on stop.

---

## Open Questions

1. **Interpolation between seek updates** — `seek_updated` arrives ~once per second. Should the SPA interpolate between updates using a local timer for smoother scrolling? Likely yes for a polished experience.
2. **Multi-track operas** — An opera may span many tracks (e.g., one track per aria/scene). The interchange format supports this via the `tracks` array, but the track-change logic needs to handle sequential libretto progression across tracks.
3. **Ensemble/duet passages** — When multiple characters sing simultaneously, how should the text be structured? Possibly parallel `text` fields or a structured array of character+text pairs within a single segment.
4. **Libretto management UI** — For v1, libretto files are hand-edited JSON. A future version could include a web UI for the tap-along annotation workflow.
5. **Character highlighting** — Color-coding or visual differentiation per character would improve readability, especially in ensemble scenes.
6. **Supertitle mode vs. full-text mode** — Opera houses show one or two lines at a time (supertitle style). An alternative is a full scrolling text with the current position highlighted. Both modes could be supported.
7. **Language selection** — If translations are available in multiple languages, the user should be able to choose which to display (or original only).

---

## Crowdsourcing Libretto Content

The value of Liberetto scales directly with the breadth of its libretto library. One person cannot realistically annotate the entire opera repertoire. Crowdsourcing is essential — but it needs to be easy to contribute, easy to consume, and resistant to quality problems.

### The Core Challenge

A timed libretto has two independent layers of work:

1. **The text itself** — Libretto text, character assignments, act/scene structure, and translations. This is largely a solved problem: authoritative libretto texts exist in the public domain for most operas (pre-1930), and many fan sites maintain high-quality transcriptions. The text layer is highly reusable across recordings.
2. **The timing** — Timestamps that sync text to a specific recording. This is recording-specific: a Karajan Bohème and a Bernstein Bohème have different tempi, cuts, and phrasing. Each recording needs its own timing pass.

Separating these two layers is the key architectural insight for crowdsourcing. A contributor who transcribes the text of Tosca benefits everyone. A contributor who times that text to one recording benefits anyone who owns that recording. The text work is done once; the timing work fans out per recording.

### Two-Layer Format

Extend the interchange format to support **untimed libretto files** alongside timed ones:

- **Base libretto** (`tosca.libretto-text.json`) — Contains the opera metadata, act/scene structure, character assignments, text, and translations, but *no timing information*. Segments have `text` and `character` but no `start`/`end`. This is the reusable, recording-independent layer.
- **Timing overlay** (`tosca-callas-1953.libretto.json`) — References a base libretto and adds timing for a specific recording. Contains the track structure (album, track titles, durations) and a per-segment `start`/`end` that maps onto the base libretto's segment sequence.

This means a contributor can:
- Submit a base libretto without needing any audio (pure text work).
- Submit a timing overlay for a recording they own, referencing an existing base libretto.

### Making It Easy to Contribute

#### 1. Web-Based Annotation Tool

A browser-based tool lowers the barrier to near zero — no software to install, works on any platform.

- **Text entry mode** — Paste or import a libretto. The tool helps structure it into segments, assign characters, and mark act/scene boundaries. Could import from common formats (plain text, PDF, HTML from libretto sites). The output is a base libretto file.
- **Timing mode** — Load a base libretto and an audio file (or connect to a streaming service). The tool plays the audio and the contributor taps/clicks to advance through segments. Keyboard shortcuts for fine adjustment (nudge ±0.1s, ±0.5s, ±1s). The output is a timing overlay.
- **Review/correction mode** — Play back a timed libretto and see the text scroll in sync. Click on any segment to adjust its timing. This lets a second contributor refine someone else's rough timing pass.

#### 2. GitHub-Based Repository

Host the libretto library as a public GitHub repository. This gives us:

- **Version control** — Full history of every change.
- **Pull requests** — Contributors fork, add or improve a libretto, and submit a PR. Reviewers can play back the result before merging.
- **Issues** — Anyone can request a specific opera/recording or report timing errors.
- **Discoverability** — GitHub search, README indices, topic tags.
- **No infrastructure cost** — GitHub hosts it for free.

Repository structure:

```
liberetto-library/
  operas/
    puccini/
      la-boheme/
        base.libretto-text.json
        timings/
          karajan-1972-decca.libretto.json
          bernstein-1988-dg.libretto.json
      tosca/
        base.libretto-text.json
        timings/
          de-sabata-1953-emi.libretto.json
    verdi/
      la-traviata/
        ...
  translations/
    la-boheme.en.json
    la-boheme.de.json
  README.md
  CONTRIBUTING.md
```

#### 3. Contribution Guidelines

Lower friction with clear documentation:

- **Templates** — Provide empty template files for base librettos and timing overlays. A contributor fills in the blanks.
- **Validation CLI** — A command-line tool that checks a libretto file for schema conformance, segment ordering, timing gaps, and other common errors. Run it before submitting a PR.
- **Partial contributions welcome** — A contributor can submit Act I of a four-act opera. A contributor can submit rough timings (±2 seconds) that someone else later refines. Incremental progress is better than waiting for perfection.
- **Credit** — Each file includes a `contributors` field listing who did the text, timing, and translation work.

#### 4. Import from Existing Sources

Much of the text work already exists:

- **Opera libretto sites** — Sites like librettidopera.it, opera-arias.com, and the Aria Database have thousands of transcribed librettos. With permission or where public domain, these could be bulk-imported and structured.
- **Subtitle files** — Some opera recordings on video have `.srt` or `.ass` subtitle files. These already contain timed text, though the timing corresponds to a video release, not necessarily the same audio recording. Still, a useful starting point.
- **MusicXML / MEI** — Some operas have digitized scores in MusicXML format, which contains lyrics attached to notes. This could be a source for text segmentation, and potentially even rough timing via score-following algorithms.

### Making It Easy to Consume

#### 1. Built-In Library Browser

The roon-rd SPA could include a "Libretto Library" panel:

- Browse available librettos by composer, opera, and recording.
- See which of the user's Roon library tracks have matching timed librettos.
- One-click download from the public repository into the local data directory.
- Show status: "Timed for your recording", "Timed for a different recording (may not sync perfectly)", "Text only (no timing available)".

#### 2. Automatic Matching

When a track starts playing, roon-rd automatically checks for a matching timed libretto:

- **Exact match** — Album name, track title, and duration all match a timing overlay. High confidence; display immediately.
- **Fuzzy match** — Same opera and act, but a different recording. Timings will drift but may be close enough to be useful, especially if durations are similar. Offer to display with a warning.
- **Text-only match** — A base libretto exists but no timing for this recording. Could display the full text without auto-scrolling, or prompt the user to contribute a timing pass.

#### 3. Package Distribution

For users who don't want to deal with Git:

- **Downloadable bundles** — Periodic releases of the full library as a `.zip` or `.tar.gz`, downloadable from the GitHub releases page or a simple website.
- **In-app sync** — roon-rd could pull updates from the repository on demand (similar to how package managers work). A simple `GET` to the GitHub API to check for new/updated files, then download what's needed.
- **Offline-first** — All libretto data is stored locally. No network dependency during playback.

#### 4. Recording-Agnostic Fallback

If a user has a recording that nobody has timed, they can still benefit:

- Display the untimed base libretto as a scrollable document alongside playback.
- Offer a "follow along" mode where the user manually taps to advance (essentially a personal timing session), with the option to save and contribute the resulting timing.

### Quality and Trust

- **Ratings/flags** — Contributors or users can flag timing errors on specific segments. A simple issue template: "Segment X in file Y is off by ~3 seconds."
- **Multiple timing passes** — Allow multiple timing overlays for the same recording. The "best" one (most reviewed, most recently updated) gets used by default, but users can switch.
- **Automated sanity checks** — CI on the repository validates every PR: schema conformance, segment ordering, reasonable timing gaps (e.g., flag any segment shorter than 0.5s or longer than 120s as likely errors).

---

## v1 Milestones

1. **Define and validate the interchange format** — Create a sample libretto JSON for one act of an opera by hand.
2. **Build a tap-along annotation tool** — Simple CLI or web tool: play audio, tap to advance, output timestamped JSON.
3. **Add libretto loading to roon-rd server** — Read libretto files, match to tracks, expose via REST/WebSocket.
4. **Add libretto display to roon-rd SPA** — Render synchronized text in the browser, driven by `seek_updated` events.
5. **Polish** — Smooth scrolling, character labels, translation toggle, fullscreen layout.
