# Libretto Interchange Format Specification

## Overview

The Libretto interchange format is a JSON document that pairs opera libretto text with precise timing information relative to audio tracks. A single file describes one complete opera recording (which may span multiple tracks).

## File Extension

`.libretto.json`

## Top-Level Structure

```json
{
  "version": "1.0",
  "opera": { ... },
  "tracks": [ ... ]
}
```

| Field     | Type   | Required | Description |
|-----------|--------|----------|-------------|
| `version` | string | yes      | Format version. Currently `"1.0"`. |
| `opera`   | object | yes      | Metadata about the opera itself. |
| `tracks`  | array  | yes      | One entry per audio track, each containing timed text segments. |

## Opera Object

General metadata about the opera, independent of any particular recording.

```json
{
  "title": "La Bohème",
  "composer": "Giacomo Puccini",
  "librettist": "Luigi Illica & Giuseppe Giacosa",
  "language": "it",
  "translation_language": "en",
  "year": 1896
}
```

| Field                  | Type   | Required | Description |
|------------------------|--------|----------|-------------|
| `title`                | string | yes      | Name of the opera. |
| `composer`             | string | yes      | Composer name. |
| `librettist`           | string | no       | Librettist name(s). |
| `language`             | string | yes      | ISO 639-1 code for the original libretto language (e.g., `"it"`, `"de"`, `"fr"`). |
| `translation_language` | string | no       | ISO 639-1 code for the translation language, if translations are provided. |
| `year`                 | number | no       | Year of the opera's premiere. |

## Track Object

Each track corresponds to one audio file or one track in a digital album. An opera may have one track per act, one per scene, or one per aria — the format accommodates any granularity.

```json
{
  "track_id": "act-1",
  "title": "Act I",
  "album": "La Bohème (Pavarotti, Karajan, 1972)",
  "artist": "Luciano Pavarotti, Herbert von Karajan, Berlin Philharmonic",
  "disc_number": 1,
  "track_number": 1,
  "duration_seconds": 2145.0,
  "act": "I",
  "scene": null,
  "segments": [ ... ]
}
```

| Field              | Type   | Required | Description |
|--------------------|--------|----------|-------------|
| `track_id`         | string | yes      | Unique identifier for this track within the file. Used for cross-referencing in track-map configuration. |
| `title`            | string | yes      | Track title as it appears in the album metadata. Used for matching against playback metadata. |
| `album`            | string | no       | Album name. Aids in matching the correct recording. |
| `artist`           | string | no       | Performer(s). |
| `disc_number`      | number | no       | Disc number in a multi-disc set. |
| `track_number`     | number | no       | Track number on the disc. |
| `duration_seconds` | number | no       | Total track duration in seconds. Useful for validation. |
| `act`              | string | no       | Act identifier (e.g., `"I"`, `"II"`). Informational; segments may also carry act/scene. |
| `scene`            | string | no       | Scene identifier, if the track corresponds to a specific scene. |
| `segments`         | array  | yes      | Ordered array of timed text segments. |

## Segment Object

A segment is the fundamental unit of timed text. It represents a passage sung (or spoken) by one character, or a non-vocal moment (interlude, stage direction).

```json
{
  "start": 15.5,
  "end": 28.3,
  "type": "sung",
  "character": "Rodolfo",
  "text": "Nei cieli bigi guardo fumar dai mille\ncomignoli Parigi.",
  "translation": "In the grey skies I watch the smoke rising\nfrom a thousand chimneys of Paris.",
  "direction": null,
  "act": "I",
  "scene": "1"
}
```

| Field         | Type   | Required | Description |
|---------------|--------|----------|-------------|
| `start`       | number | yes      | Start time in seconds from the beginning of the track. Decimal for sub-second precision. |
| `end`         | number | no       | End time in seconds. If omitted, the segment ends when the next segment's `start` begins. For the last segment, it ends at the track duration. |
| `type`        | string | no       | One of `"sung"`, `"spoken"`, `"interlude"`, `"direction"`. Defaults to `"sung"`. |
| `character`   | string | no       | Name of the character singing or speaking. `null` for interludes or pure stage directions. |
| `text`        | string | no       | Libretto text in the original language. Newlines within the string represent line breaks in the verse. Required unless `type` is `"interlude"` or `"direction"`. |
| `translation` | string | no       | Parallel translation of `text`. Line breaks should correspond to the original. |
| `direction`   | string | no       | Stage direction associated with this moment (e.g., `"Mimi knocks at the door"`). May appear alongside `text` or standalone. |
| `act`         | string | no       | Act identifier. Inherited from the track if omitted. |
| `scene`       | string | no       | Scene identifier. |

### Timing Rules

- Segments must be ordered by `start` time within a track.
- Segments must not overlap: a segment's `start` must be ≥ the previous segment's `end` (or `start`, if `end` is omitted).
- Gaps between segments are permitted and represent moments with no displayed text (orchestral passages, etc.).
- Times are floating-point seconds with arbitrary precision. Typical annotation will be accurate to ±0.5 seconds; sub-second precision allows refinement.

### Ensemble / Simultaneous Singing

When multiple characters sing simultaneously (duets, trios, choruses), there are two options:

**Option A — Single segment with combined text:**

```json
{
  "start": 120.0,
  "end": 145.0,
  "type": "sung",
  "character": "Mimi & Rodolfo",
  "text": "O soave fanciulla, o dolce viso\ndi mite circonfuso alba lunar,\nin te ravviso il sogno ch'io vorrei sempre sognar!",
  "translation": "O lovely girl, o sweet face\nbathed in the soft moonlight,\nI see in you the dream I would always dream!"
}
```

**Option B — Parallel segments with overlapping times:**

```json
[
  {
    "start": 120.0,
    "end": 135.0,
    "character": "Rodolfo",
    "text": "O soave fanciulla, o dolce viso..."
  },
  {
    "start": 122.0,
    "end": 140.0,
    "character": "Mimi",
    "text": "Ah! tu sol comandi, amor!..."
  }
]
```

Option B is more precise but requires the display layer to handle overlapping segments. For v1, Option A (combined text with a compound character name) is simpler. The format permits both; consumers should handle overlapping start times gracefully.

## Track Matching Configuration

A separate file maps playback metadata to libretto files. This decouples the libretto content from any particular playback system.

**`track-map.json`**:

```json
{
  "mappings": [
    {
      "match": {
        "album": "La Bohème",
        "title": "Act I"
      },
      "libretto_file": "la-boheme-karajan/act1.libretto.json",
      "track_id": "act-1"
    },
    {
      "match": {
        "album": "La Bohème",
        "title": "Act II"
      },
      "libretto_file": "la-boheme-karajan/act2.libretto.json",
      "track_id": "act-2"
    }
  ]
}
```

| Field                | Type   | Required | Description |
|----------------------|--------|----------|-------------|
| `match.album`        | string | no       | Album name to match (substring or exact). |
| `match.title`        | string | no       | Track title to match. |
| `match.artist`       | string | no       | Artist to match. |
| `match.disc_number`  | number | no       | Disc number to match. |
| `match.track_number` | number | no       | Track number to match. |
| `libretto_file`      | string | yes      | Path to the `.libretto.json` file, relative to the data directory. |
| `track_id`           | string | yes      | The `track_id` within the libretto file that corresponds to this audio track. |

Matching is performed by the consumer (roon-rd) against the now-playing metadata. All provided `match` fields must match for the mapping to apply. Fields not specified are wildcards.

## Complete Example

A minimal but complete example for a single track with a few segments:

```json
{
  "version": "1.0",
  "opera": {
    "title": "Tosca",
    "composer": "Giacomo Puccini",
    "librettist": "Luigi Illica & Giuseppe Giacosa",
    "language": "it",
    "translation_language": "en"
  },
  "tracks": [
    {
      "track_id": "act-1",
      "title": "Act I",
      "album": "Tosca (Callas, De Sabata, 1953)",
      "duration_seconds": 2532.0,
      "segments": [
        {
          "start": 0.0,
          "end": 8.0,
          "type": "interlude",
          "direction": "Three chords. The curtain rises."
        },
        {
          "start": 8.0,
          "end": 22.5,
          "type": "spoken",
          "character": "Angelotti",
          "text": "Ah! Finalmente!",
          "translation": "Ah! At last!"
        },
        {
          "start": 22.5,
          "end": 45.0,
          "type": "direction",
          "direction": "Angelotti, in prison garb, enters breathlessly through the side door."
        },
        {
          "start": 45.0,
          "end": 68.0,
          "type": "sung",
          "character": "Angelotti",
          "text": "Nel terror mio stolto\nvedea ceffi di birro in ogni volto.",
          "translation": "In my foolish terror\nI saw the face of a constable in every face."
        }
      ]
    }
  ]
}
```

## Validation Rules

A conforming file must satisfy:

1. `version` is present and is a recognized version string.
2. `opera.title`, `opera.composer`, and `opera.language` are present and non-empty.
3. `tracks` is a non-empty array.
4. Each track has a non-empty `track_id` and a non-empty `segments` array.
5. Each segment has a `start` value ≥ 0.
6. Segments within a track are ordered by `start` (ascending).
7. No segment's `start` is less than the previous segment's `end` (if `end` is specified).
8. Segments with `type` of `"sung"` or `"spoken"` have non-empty `text`.
9. All `track_id` values within the file are unique.
