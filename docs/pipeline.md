# Libretto Pipeline

## Overview

```
  ONLINE SOURCE (murashev.com or opera-arias.com)
       │
       ▼
  ┌─────────┐    ┌─ bilingual.json ──── structured pairs (parser input)    ┐
  │ acquire │───▶├─ italian.json ─────── monolingual structured (parser)   ├─ acquire
  └─────────┘    ├─ english.json ─────── monolingual structured (parser)   │  outputs
                 ├─ italian.txt ──────── plain text (human convenience)    │
                 ├─ english.txt ──────── plain text (human convenience)    │
                 ├─ source.md ────────── provenance                        │
                 └─ raw*.html ────────── cached HTML                       ┘
                        │
                        │  parse reads (in priority order):
                        │   1. bilingual.json
                        │   2. italian.json + english.json
                        │   3. italian.json or english.json alone
                        ▼
  ┌─────────┐    ┌──────────────────────┐
  │  parse  │───▶│ base.libretto.json   │  Structured: acts, numbers,
  └─────────┘    └──────────┬───────────┘  segments (ID, character, text,
                            │              translation, segment_type, group)
                            │
               ┌────────────┤
               │            │
               ▼            ▼
  ┌──────────────┐   ┌──────────────────────────────────┐
  │ timing init  │──▶│ scaffold.timing.json              │  Scaffold overlay:
  └──────────────┘   │                                   │  one track per number,
                     │ YOU hand-edit:                     │  number_ids pre-filled.
                     │  • track titles (from recording)   │
                     │  • disc/track numbers              │  Hand-edit to match
                     │  • duration_seconds                │  your specific recording.
                     │  • number_ids (split/merge)        │
                     └──────────────┬─────────────────────┘
                                    │
                                    ▼
  ┌────────────────┐  ┌──────────────────────────────────┐
  │ timing resolve │─▶│ *.resolved.timing.json            │  + start_segment_id
  │                │  │                                   │  filled in per track
  │ reads:         │  │                                   │  (anchor matching from
  │  --base        │  │                                   │  quoted text in titles)
  │  --timing      │  └──────────────┬────────────────────┘
  └────────────────┘                 │
                                     ▼
  ┌──────────────────┐ ┌──────────────────────────────────┐
  │ timing estimate  │▶│ *.estimated.timing.json           │  + segment_times[]
  │                  │ │                                   │  filled in: start time
  │ reads:           │ │                                   │  per segment (word-weight
  │  --base          │ │                                   │  proportional, recitative
  │  --timing        │ └──────────────┬────────────────────┘  segments 0.5× discount)
  └──────────────────┘               │
                                     ▼
  ┌────────────────┐  ┌───────────────────────────────────┐
  │ timing merge   │─▶│ *.timed.libretto.json              │  FINAL interchange:
  │                │  │                                    │  self-contained, tracks →
  │ reads:         │  │                                    │  timed segments (start,
  │  --base        │  │                                    │  end, character, text,
  │  --timing      │  │                                    │  translation, type,
  └────────────────┘  └──────────────┬─────────────────────┘  group, act, scene)
                                     │
                                     ▼
                      ┌───────────────────────────────────┐
                      │ roon-rd --libretto <file>          │  Serves timed libretto
                      │                                    │  via /libretto API +
                      │ Browser: /libretto-view            │  WebSocket sync with
                      └────────────────────────────────────┘  Roon playback position
```

## Quick Reference

| Step | Command | Reads | Writes |
|------|---------|-------|--------|
| **1** | `acquire --source murashev` | URL | `bilingual.json`, `italian.txt`, `english.txt`, `source.md`, `raw*.html` |
| **1** | `acquire --source opera-arias` | URL | `{lang}.json`, `{lang}.txt`, `source.md`, `raw_{lang}.html` (per language) |
| **2** | `parse -i <dir>` | `bilingual.json` or `italian.json`+`english.json` | `base.libretto.json` |
| **3** | `timing init` | `base.libretto.json` | `scaffold.timing.json` (hand-edit) |
| **4** | `timing resolve` | `base.libretto.json` + `*.timing.json` | `*.resolved.timing.json` |
| **5** | `timing estimate` | `base.libretto.json` + `*.resolved.timing.json` | `*.estimated.timing.json` |
| **6** | `timing merge` | `base.libretto.json` + `*.estimated.timing.json` | `*.timed.libretto.json` |
| **7** | `roon-rd --libretto` | `*.timed.libretto.json` | Browser UI |

## Notes

- **Steps 4–6** all take `--base` and `--timing` flags. The timing overlay is progressively enriched at each step.
- The `.txt` files from acquire are for **human reading only** — `parse` never uses them.
- `parse` prefers `bilingual.json` (murashev bilingual mode) over separate monolingual `.json` files.
- The timing overlay scaffold from `timing init` requires **hand-editing** to match a specific recording's track structure.
- `timing estimate` applies a **0.5× word-weight discount** to recitative segments (classified from track title keywords).
- `timing merge` enriches `segment_type` to include `"recitative"` and carries through the `group` field for ensemble display.
