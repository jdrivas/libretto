# Multi-Opera Library Management — Design Document

## 1. Current Architecture: libretto-view

### Overview

The libretto viewer is a single-page application (SPA) embedded as a raw HTML string
(`LIBRETTO_VIEW_HTML`) in the roon-rd Rust server at `roon-rd/src/server/mod.rs`.
It provides synchronized libretto display during Roon music playback.

### System Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│  Roon Core                                                              │
│  ┌──────────────┐                                                       │
│  │  Transport   │  ← controls playback, reports position                │
│  └──────┬───────┘                                                       │
│         │ Roon API (subscription events)                                │
└─────────┼───────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  roon-rd Server (Rust / Axum)                            port 3000      │
│                                                                         │
│  ┌─────────────────────┐   ┌──────────────────────────────────────┐    │
│  │   AppState           │   │  Routes                              │    │
│  │                      │   │                                      │    │
│  │  roon_client:        │   │  GET  /libretto-view → HTML page     │    │
│  │    Arc<Mutex<        │   │  GET  /libretto      → JSON data     │    │
│  │      RoonClient>>    │   │  GET  /now-playing   → zone state    │    │
│  │                      │   │  WS   /ws            → real-time     │    │
│  │  libretto_json:      │   │                        push events   │    │
│  │    Option<Arc<       │   │  POST /seek/:zone_id → seek playback │    │
│  │      serde_json::    │   │                                      │    │
│  │      Value>>         │   └──────────────────────────────────────┘    │
│  │                      │                                               │
│  └─────────────────────┘                                               │
│                                                                         │
│  Startup:                                                               │
│    --libretto <path>  →  read file  →  parse JSON  →  Arc<Value>       │
│                          (read-only, served as-is via GET /libretto)    │
└────────────────────┬────────────────────────────────────────────────────┘
                     │
          ┌──────────┴──────────┐
          │  HTTP + WebSocket   │
          ▼                     ▼
┌─────────────────────────────────────────────────────────────────────────┐
│  Browser SPA  (libretto-view)                                           │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ On Load                                                         │   │
│  │   fetch('/libretto') → librettoData                             │   │
│  │   connectWebSocket('/ws')                                       │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ WebSocket Messages                                              │   │
│  │                                                                 │   │
│  │  zones_changed  ──→  handleZoneUpdate(zones)                    │   │
│  │                        ├── find playing zone                    │   │
│  │                        ├── matchTrack(roonTitle) → track        │   │
│  │                        ├── renderTrack(track)                   │   │
│  │                        └── updateHighlight(position)            │   │
│  │                                                                 │   │
│  │  seek_updated   ──→  updateHighlight(seekPosition)              │   │
│  │                        ├── findSegmentIndex(track, position)    │   │
│  │                        ├── classify segments: past/active/future│   │
│  │                        ├── highlight active (expand to group)   │   │
│  │                        ├── auto-scroll to active                │   │
│  │                        └── update progress bar                  │   │
│  │                                                                 │   │
│  │  connection_changed ──→ refetch /now-playing                    │   │
│  └─────────────────────────────────────────────────────────────────┘   │
│                                                                         │
│  ┌─────────────────────────────────────────────────────────────────┐   │
│  │ Rendering                                                       │   │
│  │                                                                 │   │
│  │  ┌──── Header (sticky) ──────────────────────────────────────┐  │   │
│  │  │  Album — Artist — N tracks                                │  │   │
│  │  │  Roon Track Title (bold, 22px)                            │  │   │
│  │  │  Matched: Libretto Track Title (gold, disc/track/segs)    │  │   │
│  │  │  1:23 / 5:45 — seg 4/28 @ 1:15 → 1:31                   │  │   │
│  │  │  ████████░░░░░░░░░░░░░░░░░░  (progress bar)              │  │   │
│  │  └───────────────────────────────────────────────────────────┘  │   │
│  │                                                                 │   │
│  │  ┌──── Segment List (scrollable) ────────────────────────────┐  │   │
│  │  │                                                           │  │   │
│  │  │  past     0:00  FIGARO    Cinque, dieci...  Five, ten...  │  │   │
│  │  │  past     0:08  SUSANNA   Trenta, trentasei Thirty, 36   │  │   │
│  │  │                                                           │  │   │
│  │  │  ┌── ensemble-group (border-left, shared start) ──────┐  │  │   │
│  │  │  │ active  0:15  FIGARO    Ah guarda...   Ah look...   │  │  │   │
│  │  │  │ active        SUSANNA   Se a caso...   If maybe...  │  │  │   │
│  │  │  └────────────────────────────────────────────────────┘  │  │   │
│  │  │                                                           │  │   │
│  │  │  future   0:23  FIGARO    Ed io...         And I...      │  │   │
│  │  │  future   0:31  SUSANNA   Bravo signor...  Bravo sir...  │  │   │
│  │  │                                                           │  │   │
│  │  └───────────────────────────────────────────────────────────┘  │   │
│  └─────────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────────┘
```

### Segment Layout (per row)

```
┌─────────┬────────────┬──────────────────────────┬──────────────────────────┐
│ seg-start│ character   │ text (original)          │ translation (italic)     │
│  1:23   │   FIGARO   │ Cinque... dieci...       │ Five... ten...           │
│  (mono) │  (gold,    │ (18px, flex:1)           │ (15px, flex:1, #888)     │
│  (11px) │   right)   │                          │                          │
└─────────┴────────────┴──────────────────────────┴──────────────────────────┘
   52px      120px       ◄──────── .texts (flex, gap:24px) ────────────────►
```

For **direction** segments (stage directions), the text spans the full `.texts` width
with the `.direction-text` class instead of the two-column text/translation layout.

For **ensemble groups**, consecutive segments sharing the same `group` tag are wrapped
in a `.ensemble-group` container with a gold left border. The `seg-start` time is shown
only for the first segment in the group; subsequent members get `visibility: hidden`.

### Key Functions

| Function | Purpose |
|----------|---------|
| `loadLibretto()` | `fetch('/libretto')` → stores `librettoData` |
| `matchTrack(roonTitle)` | Fuzzy-match Roon's track title to a libretto track. Strips opera prefix, normalizes punctuation, falls back to word-overlap scoring (threshold 0.25). |
| `findSegmentIndex(track, pos)` | Linear scan to find last segment with `start <= pos` |
| `buildGroupMap(segments)` | Builds array mapping each segment index → `{start, end}` of its ensemble group (or `null`) |
| `renderTrack(track)` | Generates full DOM for all segments, wrapping groups in `.ensemble-group` containers |
| `renderSegmentInner(seg, i, isGroupCont)` | Builds HTML for one segment row |
| `updateHighlight(position)` | Classifies all segments as past/active/future, expands active to group, scrolls |
| `handleZoneUpdate(zones)` | Entry point for zone data; finds playing zone, matches track, renders, highlights |
| `connectWebSocket()` | Connects to `/ws`, handles `zones_changed`, `seek_updated`, `connection_changed`; auto-reconnects on close |

### Data Flow Summary

1. **Startup**: Server reads `--libretto <path>` → `serde_json::Value` → `Arc<Value>` in `AppState`
2. **Page load**: SPA fetches `GET /libretto` (full JSON), connects `WS /ws`
3. **Playback**: Roon Core → roon-rd → WS `zones_changed` (with track title + position)
4. **Track match**: SPA `matchTrack()` fuzzy-matches Roon title → interchange track
5. **Render**: `renderTrack()` builds segment DOM for matched track
6. **Scroll**: WS `seek_updated` (~1/s) → `updateHighlight()` → classify + scroll + progress bar
7. **Manual scroll**: User scroll disables auto-scroll for 5 seconds

---

## 2. Multi-Opera Library Design

### Problem

Currently roon-rd takes `--libretto <single-file>`, loads one interchange JSON, and
matches tracks only within that one file. To be useful during everyday listening, the
server needs to hold librettos for many recordings of many operas and automatically
display the right one for whatever track Roon is playing.

### 2.1 File Organization

A **libretto library directory** containing interchange files, one per recording:

```
~/libretto-library/
  mozart/
    le-nozze-di-figaro/
      georg-solti-1981.interchange.json
      karl-bohm-1968.interchange.json
      rene-jacobs-2004.interchange.json
    don-giovanni/
      carlo-maria-giulini-1959.interchange.json
  puccini/
    la-boheme/
      herbert-von-karajan-1972.interchange.json
  verdi/
    la-traviata/
      carlos-kleiber-1977.interchange.json
```

The directory structure is for human navigation — the server doesn't care about nesting.
It recursively scans for `*.interchange.json` files.

Alternatively, a flat directory works fine:

```
~/libretto-library/
  figaro-solti-1981.interchange.json
  figaro-bohm-1968.interchange.json
  boheme-karajan-1972.interchange.json
```

### 2.2 Server-Side: Library Index

#### Startup

Replace `--libretto <file>` with `--libretto-dir <directory>` (keep `--libretto <file>`
as a convenience shorthand that also works):

```
roon-rd serve --libretto-dir ~/libretto-library
```

At startup, the server:

1. Recursively finds all `*.interchange.json` files
2. Parses each into `InterchangeLibretto` (or `serde_json::Value`)
3. Builds a **track index** for fast lookup

#### Track Index

```rust
struct LibrettoLibrary {
    /// All loaded interchange files, keyed by a stable ID (e.g., file stem)
    files: HashMap<String, LoadedLibretto>,
    /// Index: normalized album name → list of (file_id, track_index) pairs
    album_index: HashMap<String, Vec<TrackRef>>,
}

struct LoadedLibretto {
    data: serde_json::Value,       // Full interchange JSON
    file_path: PathBuf,            // For saving (editor)
    opera_title: String,           // For display
    album: Option<String>,         // From first track
    artist: Option<String>,        // From first track
}

struct TrackRef {
    file_id: String,               // Which interchange file
    track_index: usize,            // Index into that file's tracks array
    title: String,                 // Normalized track title for matching
    album: Option<String>,         // Album name from track metadata
    artist: Option<String>,        // Artist from track metadata
    disc_number: Option<u32>,
    track_number: Option<u32>,
    duration_seconds: Option<f64>,
}
```

The index is `O(1)` by album name, then linear within the (small) list of tracks
for that album.

#### AppState Change

```rust
pub struct AppState {
    pub roon_client: Arc<Mutex<RoonClient>>,
    pub carousel_interval_ms: u32,
    pub library: Option<Arc<RwLock<LibrettoLibrary>>>,  // replaces libretto_json
}
```

### 2.3 Track Matching: Two-Level Strategy

Roon's now-playing data provides: `track` (title), `album`, `artist`,
`position_seconds`, `length_seconds`.

#### Level 1: Album Match

Normalize the Roon album name and look up in `album_index`. This narrows candidates
to tracks from the same recording. Album name is the strongest discriminator — it
distinguishes Solti's Figaro from Böhm's Figaro.

```
Roon album: "Le nozze di Figaro (Solti)"
  → normalize → "le nozze di figaro solti"
  → album_index lookup → georg-solti-1981.interchange.json
```

Normalization: lowercase, strip punctuation, collapse whitespace. Fuzzy matching
(word overlap) for cases where album names differ slightly between Roon metadata
and interchange metadata.

#### Level 2: Track Match (within album)

Once we've identified the interchange file, use the existing `matchTrack()` logic
(prefix stripping, word overlap) to find the specific track. This is essentially
unchanged from today.

#### Level 3: Fallback — Cross-Album Match

If no album match, try matching the track title across all loaded files. This is
lower confidence and may produce false matches (e.g., two recordings of the same
aria). Use duration as a tiebreaker: prefer the track whose `duration_seconds` is
closest to the Roon track's `length_seconds`.

#### Matching Flow Diagram

```
Roon now-playing: { album, track, artist, length_seconds }
         │
         ▼
┌─────────────────────────┐
│ Normalize album name    │
│ Look up album_index     │
└──────────┬──────────────┘
           │
     ┌─────┴─────┐
     │ Found?    │
     ├── yes ────┼──→ matchTrack() within that file's tracks
     │           │      │
     │           │      ├── matched → USE IT
     │           │      └── no match → fall through to Level 3
     │           │
     └── no  ────┼──→ Fuzzy album match (word overlap, threshold 0.5)
                 │      │
                 │      ├── matched → matchTrack() within that file
                 │      └── no match → Level 3
                 │
                 ▼
         Level 3: scan ALL tracks across all files
           best = highest (title_score × 0.7 + duration_score × 0.3)
           if best > threshold → USE IT (with lower confidence)
           else → no libretto available
```

### 2.4 API Changes

| Endpoint | Change |
|----------|--------|
| `GET /libretto` | Returns the **matched** interchange data for the currently playing track (auto-detected). Or returns a manifest of all loaded files if no track is playing. |
| `GET /librettos` | **New.** Returns a manifest: list of loaded interchange files with opera title, album, artist, track count, file ID. |
| `GET /libretto/:file_id` | **New.** Returns a specific interchange file by ID. |

The SPA can either:
- **Option A (lazy)**: Fetch only the matched file when a track starts playing.
  Server-side matching via `GET /libretto/match?album=X&track=Y`.
- **Option B (eager)**: Fetch the manifest, then fetch individual files on demand.
- **Option C (hybrid)**: Fetch the full manifest at startup; server pushes the
  matched file ID via WebSocket when a track changes.

**Recommended: Option C.** The manifest is tiny (just metadata). When `zones_changed`
fires, the server includes `libretto_file_id` and `libretto_track_index` in the
payload. The SPA fetches the full file only when the file ID changes (not on every
track change within the same recording).

#### Updated WebSocket Messages

```json
// zones_changed (augmented)
{
  "type": "zones_changed",
  "now_playing": [{
    "zone_id": "...",
    "track": "Le nozze di Figaro, K. 492, Act I: No. 1...",
    "album": "Le nozze di Figaro",
    "artist": "Solti / LPO",
    "position_seconds": 45.2,
    "length_seconds": 312.0,
    "libretto_match": {                    // NEW — server-side match result
      "file_id": "georg-solti-1981",
      "track_index": 1,
      "confidence": "album",              // "album", "fuzzy_album", "cross_album", or null
      "opera": "Le nozze di Figaro"
    }
  }]
}
```

The SPA receives `libretto_match`, fetches the file if not already cached, and
renders the matched track. This moves the heavy matching logic to the server
(where it has access to the full index) and keeps the SPA simple.

### 2.5 SPA Changes

Minimal changes to the SPA:

1. **Cache loaded interchange files** by `file_id` (in-memory JS object)
2. **On `zones_changed`**: check `libretto_match.file_id`
   - If same as current → just match track within cached data (as today)
   - If different → `fetch('/libretto/' + fileId)`, cache it, render
   - If null → show "no libretto available"
3. **Header display**: show opera title, album, match confidence
4. **Remove client-side album matching** — server handles it

### 2.6 Hot Reload (Optional)

Watch the libretto directory for file changes (new files, modified files):

- Use `notify` crate for filesystem watching
- On change: reload the affected file, rebuild index
- Push `libretto_library_changed` via WebSocket so the SPA can re-fetch manifest

This is a nice-to-have for the editing workflow: save an interchange file from the
pipeline, and roon-rd picks it up without restart.

### 2.7 CLI Changes

```
roon-rd serve --libretto-dir ~/libretto-library    # directory of interchange files
roon-rd serve --libretto figaro.interchange.json    # single file (convenience, as today)
roon-rd serve                                       # no libretto support
```

Both `--libretto-dir` and `--libretto` could be specified simultaneously (the single
file is just added to the library).

---

## 3. Sequencing: Multi-Opera Before Timing Editor

### Recommendation

**Do multi-opera library support before the timing editor.** Here's the reasoning:

### 3.1 Pipeline Stress-Testing

Building interchange files for multiple operas and multiple Figaro recordings will
exercise the full pipeline (grouped_score_libretto → score_merge → interchange) on
diverse data. This will surface bugs and edge cases in:

- Score page matching for different score editions
- Boundary detection for recordings with different track structures
- Timing estimation across different tempi and cuts
- Character extraction from varied track title formats

Fixing these pipeline issues first means the timing editor will be refining
**better** initial estimates, reducing the amount of manual correction needed.

### 3.2 Architecture Foundation

The multi-file library architecture (`LibrettoLibrary`, `TrackRef` index, server-side
matching) establishes the right `AppState` structure. The timing editor needs mutable
access to interchange data — it's cleaner to add mutability to a multi-file architecture
from the start than to build a single-file editor and then retrofit multi-file support.

Specifically:
- The editor needs to know **which file** it's editing (to save back). Multi-file
  support naturally provides `file_id` → `file_path` mapping.
- Recomputation after a manual edit needs the segments for the affected track.
  The library index already provides `file_id + track_index` → segments.
- The `libretto_updated` WebSocket message needs to identify which file changed.
  Multi-file support provides the `file_id` vocabulary.

### 3.3 No Negative Interaction

Building multi-opera first does **not** make the timing editor harder:
- The editor adds `RwLock` mutability and edit endpoints on top of the library.
- The `manual` flag on `InterchangeSegment` is purely additive.
- Server-side recomputation operates on one track at a time regardless of how
  many files are loaded.

Building the editor first **would** require rework:
- A single-file editor would store `file_path` directly in `AppState`. Multi-file
  support would then need to refactor this into a library structure.
- The SPA's `loadLibretto()` would need to change from "fetch the one file" to
  "fetch from library by ID" — better to do this once.

### 3.4 Immediate Everyday Value

Multi-opera support makes the viewer useful during normal listening sessions
immediately, even with estimated timings. You can queue up Figaro, then Don
Giovanni, then Bohème, and librettos appear for all of them. The timing editor
is about refinement — high value but not blocking daily use.

### 3.5 Suggested Next Steps

1. **Run the pipeline on 2–3 more operas** (e.g., Don Giovanni, La Bohème, Tosca)
   and a second Figaro recording (e.g., Böhm 1968) — exercises the pipeline
2. **Implement `--libretto-dir`** and the `LibrettoLibrary` index in roon-rd
3. **Move track matching to server-side** (album-first strategy)
4. **Update SPA** for multi-file support (cache by file_id, show opera/album in header)
5. **Then** build the timing editor on top of this foundation
