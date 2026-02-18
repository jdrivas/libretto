# Libretto Timing Editor — Design Document

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

## 2. Timing Editor Design (libretto-edit)

### Goal

While listening to music and watching the libretto scroll, mark segment boundary
times, audition the results, and save corrections back to the interchange file.

### 2.1 Data Model

#### What we capture

Tapping "end of segment N" marks the **boundary between N and N+1** — which is the
start time of segment N+1. We store this as:

```json
{
  "start": 45.2,
  "manual": true,
  "character": "SUSANNA",
  "text": "Cosa stai misurando...",
  "translation": "What are you measuring..."
}
```

The `manual` flag (default `false`, omitted when false) distinguishes hand-set from
estimated start times. This is the only new field on `InterchangeSegment`.

```rust
// In interchange.rs
#[serde(default, skip_serializing_if = "is_false")]
pub manual: bool,
```

#### Why `manual` on `start` (not `end` or `duration`)

- The fundamental quantity is "when does this segment begin" — which `start` already represents
- End of segment N = start of segment N+1 (contiguous music)
- Duration is derived: `end - start`
- A single boolean flag tells us what's estimated vs hand-set
- Recomputation is clean: redistribute non-manual segments between manual boundaries

### 2.2 Recomputation Model

Manual starts partition a track's segments into **estimation zones**:

```
seg 0 [M]    seg 1    seg 2    seg 3 [M]    seg 4    seg 5 [M]    seg 6    seg 7
|__________ zone A ____________|_________ zone B _____|_________ zone C _________|
  duration = seg3.start - seg0.start
```

- `[M]` = manual start
- Zone boundaries are always manual starts (plus track start and track end)
- Non-manual segments within each zone get word-weight proportional distribution
  (reusing the existing `distribute_segments` logic from `score_merge.rs`)
- Direction segments get zero weight; groups collapse to max weight (existing behavior)

Recomputation happens **server-side** because the word-weight estimation logic
is already in Rust and the server is the authoritative data store.

### 2.3 Server Changes

#### AppState

```rust
pub struct AppState {
    pub roon_client: Arc<Mutex<RoonClient>>,
    pub carousel_interval_ms: u32,
    // Changes from Arc<Value> to mutable:
    pub libretto: Option<Arc<RwLock<EditableLibretto>>>,
}

pub struct EditableLibretto {
    pub data: serde_json::Value,      // The full interchange JSON
    pub file_path: PathBuf,           // Where to save
    pub undo_stack: Vec<EditAction>,   // For undo
    pub redo_stack: Vec<EditAction>,   // For redo
}
```

#### New API Endpoints

| Method | Path | Body | Description |
|--------|------|------|-------------|
| `POST` | `/libretto/edit` | `{ track_index, segment_index, start }` | Set manual start time for a segment; triggers recomputation; returns updated track |
| `POST` | `/libretto/clear` | `{ track_index, segment_index }` | Clear manual flag on a segment; re-estimate; returns updated track |
| `POST` | `/libretto/save` | — | Persist current state to disk (original file path) |
| `POST` | `/libretto/undo` | — | Revert last edit; returns updated track |
| `POST` | `/libretto/redo` | — | Re-apply undone edit; returns updated track |

#### New WebSocket Message

After any edit/clear/undo/redo, the server broadcasts:

```json
{
  "type": "libretto_updated",
  "track_index": 1,
  "segments": [ ... ]   // Full updated segment array for the affected track
}
```

This ensures all connected clients stay in sync.

### 2.4 SPA: Edit Mode

#### Mode Toggle

The existing `/libretto-view` page gains an **edit mode toggle** (keyboard shortcut `E`).
In edit mode:

- Keyboard shortcuts are captured for timing taps
- Each segment shows its manual/estimated status
- The selected segment gets nudge controls
- A toolbar appears with save/undo/redo controls

Alternatively, a separate `/libretto-edit` page could be used — but a mode toggle
avoids context-switching since the visual layout is identical.

#### Edit Mode Layout

```
┌─────────────────────────────────────────────────────────────────────────┐
│  ┌──── Header (same as view mode) ──────────────────────────────────┐  │
│  │  Album — Artist — 32 tracks                                      │  │
│  │  Le nozze di Figaro, K. 492, Act I: No. 1, Duet...              │  │
│  │  Matched: d1t2 — Cinque... dieci... (disc 1, track 2, 28 segs)  │  │
│  │  1:23 / 5:45 — seg 4/28 @ 1:15 → 1:31                          │  │
│  │  ████████░░░░░░░░░░░░░░░░░░  ──── [Undo] [Redo] [Save] [Edit●] │  │
│  └──────────────────────────────────────────────────────────────────┘  │
│                                                                         │
│  ┌──── Segment List ────────────────────────────────────────────────┐  │
│  │                                                                   │  │
│  │  past  0:00 ● FIGARO   Cinque, dieci...    Five, ten...          │  │
│  │                                                                   │  │
│  │  ┌─ active (selected) ────────────────────────────────────────┐  │  │
│  │  │ 1:15 ○ SUSANNA  Trenta, trentasei...  Thirty, thirty-six  │  │  │
│  │  │       [−0.5] [−0.1] [+0.1] [+0.5]            [▶ Audition] │  │  │
│  │  └────────────────────────────────────────────────────────────┘  │  │
│  │                                                                   │  │
│  │  future 1:31 ○ FIGARO   Ed intanto io...    And meanwhile I...   │  │
│  │                                                                   │  │
│  └───────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────┘

Legend:  ● = manual start    ○ = estimated start
```

#### Per-Segment Indicators

- **● (filled dot, gold)** — manual start time (hand-set)
- **○ (hollow dot, gray)** — estimated start time
- **Nudge buttons** — appear on the selected segment: `[−0.5] [−0.1] [+0.1] [+0.5]`
- **Audition button** — seeks Roon to this segment's start time

#### Keyboard Shortcuts

| Key | Action |
|-----|--------|
| **Space** or **Enter** | Mark boundary: set start of *next* segment to current interpolated position |
| **Backspace** or **Ctrl+Z** | Undo last edit |
| **Ctrl+Shift+Z** | Redo |
| **←** / **→** | Nudge selected segment ±0.1s |
| **Shift+←** / **Shift+→** | Nudge selected segment ±0.5s |
| **A** | Audition: seek Roon to selected segment's start |
| **E** | Toggle edit mode on/off |
| **Ctrl+S** / **Cmd+S** | Save to disk |
| **↑** / **↓** | Move selection to previous/next segment |
| **Click** on a segment | Select it |

### 2.5 Timestamp Precision

Roon sends `seek_updated` approximately once per second. Human reaction time adds
~200–500ms. Without mitigation, captured times could be off by up to ~1.5s.

**Client-side interpolation** solves this:

```javascript
let lastSeekTime = 0;
let lastSeekTimestamp = 0;

function currentPosition() {
    const elapsed = (Date.now() - lastSeekTimestamp) / 1000;
    return lastSeekTime + elapsed;
}
```

This gives ~50ms precision at the moment of the tap. Nudge controls handle fine
adjustment afterward.

### 2.6 Expected Workflow

1. Start roon-rd: `roon-rd serve --libretto georg-solti-1981.interchange.json`
2. Open `/libretto-view` in browser, press **E** to enter edit mode
3. Start playback in Roon
4. Watch the libretto scroll; listen for segment boundaries
5. When the next segment begins, press **Space** — this marks the boundary
6. Server recomputes estimated times for non-manual segments, pushes update via WS
7. Continue tapping through the track
8. After a pass, use **↑↓** to navigate to suspect segments
9. Press **A** to audition (Roon seeks to that segment's start)
10. Use **←→** / **Shift+←→** to nudge timing
11. Press **Ctrl+S** to save to disk
12. Repeat for next track

### 2.7 Persistence Strategy

#### Recommended: Overwrite in-place with `manual` flags

The interchange file loaded by `--libretto` is the file we save back to. The `manual`
flags on segments make it possible to:

- Identify which times are hand-set vs estimated
- Re-run estimation on non-manual segments at any time
- Extract manual timings if we later want a separate corrections file

#### Alternative: Sidecar file

Save to `<original>.edited.interchange.json`. Non-destructive, but adds file management
complexity. Could be a future option via `POST /libretto/save-as`.

#### Alternative: Corrections overlay

Save only the manual timings as a separate JSON file. Cleanest separation, but requires
a merge step to produce a usable interchange file. Deferred.

### 2.8 Open Design Decisions

1. **Mode toggle vs separate page** — Toggle on `/libretto-view` (recommended) vs
   dedicated `/libretto-edit` page. Toggle is simpler and avoids context-switching.

2. **Auto-advance after tap** — After pressing Space, should the selected segment
   automatically advance to the next one? This makes rapid tap-through natural
   (listen, tap, listen, tap...). Recommended: yes.

3. **Recomputation scope** — Recompute within the full track, or within the current
   score number? Track-level is simpler and more predictable. Number-level would
   preserve different estimation rates for recitative vs aria sections within a track.

4. **Undo granularity** — Full session undo stack (recommended) vs per-segment.
   Each edit (set manual, clear manual, nudge) is one undo step.

5. **Save behavior** — Always save to the loaded file path (simple). Could add
   "save as" later if needed.

6. **Multi-client** — If two browsers are open, edits from one should be visible
   in the other via the `libretto_updated` WebSocket broadcast. No conflict
   resolution needed for single-user use.

---

## 3. Relationship to Multi-Opera Library Support

The timing editor assumes the multi-opera library architecture is already in place.
See [multi-opera-library-design.md](multi-opera-library-design.md) for the full
design of `LibrettoLibrary`, server-side album matching, and `--libretto-dir` support.

Key dependencies:
- **`file_id`**: The editor needs to identify which interchange file it's editing
  (for save-back). The library provides `file_id` → `file_path` mapping.
- **`LibrettoLibrary` with `RwLock`**: The editor adds mutability on top of the
  library's read-only index.
- **WebSocket `libretto_match`**: Server-side matching tells the SPA which file
  and track to edit, removing ambiguity in a multi-file environment.
