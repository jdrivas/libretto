# Annotation Schema: Three-Input Enrichment

## Overview

The base libretto can be enriched from three independent sources before any per-recording timing work begins:

1. **Full libretto** (Murashev) → segments, text, characters, translations
2. **Vocal score** (Ricordi PDF) → group tags, authoritative number types, tempo markings
3. **Track listings** (one or more recordings) → anchor patterns for track-to-segment resolution

This document describes the schema changes to support this convergent annotation model.

## Design Principles

- **`NumberType` carries musical classification** — recitative vs aria vs ensemble is a property of the musical number, not the individual segment.
- **`SegmentType` stays as content-type** — Sung/Spoken/Direction/Interlude describe what kind of content the segment holds, not its musical context.
- **`Segment.group` is the only segment-level annotation from the score** — it tags segments that overlap in ensemble numbers.
- **The estimate and merge steps use parent number type** for recitative detection, eliminating the fragile track-title heuristic.

## Schema Changes

### 1. `NumberType` — add variants

```rust
pub enum NumberType {
    Overture,
    Aria,
    Arietta,                  // NEW: small aria (e.g., Barbarina's "L'ho perduta")
    Duet,
    Duettino,
    Terzetto,
    Quartet,
    Quintet,
    Sextet,
    Cavatina,
    Canzone,
    Chorus,
    Finale,
    Recitative,               // secco recitative (keyboard/continuo)
    RecitativeAccompagnato,   // NEW: accompanied recitative (orchestral, more metered)
    March,                    // NEW: e.g., wedding march in Act III
    Other,
}
```

**Why split recitative?** Secco recitatives are freely timed (singers have enormous tempo flexibility, ~2× pace of sung text). Accompagnato recitatives are more metered, closer to aria pacing. Different timing discounts.

### 2. `MusicalNumber` — add `tempo_marking`

```rust
pub struct MusicalNumber {
    pub id: String,
    pub label: String,
    pub number_type: NumberType,
    pub act: String,
    pub scene: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo_marking: Option<String>,   // NEW: e.g., "Allegro", "Andante"
    pub segments: Vec<Segment>,
}
```

### 3. `BaseLibretto` — add helper method

```rust
impl BaseLibretto {
    /// Look up a segment's parent number type.
    pub fn segment_number_type(&self, segment_id: &str) -> Option<&NumberType> { ... }
}
```

### 4. No changes to `Segment` or `SegmentType`

`Segment.group` already exists. `SegmentType` stays as Sung/Spoken/Direction/Interlude.

### 5. `estimate.rs` refactoring

Replace `resolve_section_marks` (track-title heuristic) with parent-number-type lookup:
- Segment in `Recitative` number → full `RECITATIVE_DISCOUNT` (0.5)
- Segment in `RecitativeAccompagnato` number → partial discount (0.75)
- All other segments → no discount

### 6. `merge.rs` refactoring

Replace `classify_title_anchors` usage with parent-number-type lookup:
- If parent number is `Recitative` or `RecitativeAccompagnato`, set interchange `segment_type` to `"recitative"`.

### 7. Track anchor registry (future, separate file)

Not part of `base.libretto.json`. A separate JSON file mapping common track title patterns to segment IDs, built from multiple recordings. This keeps the base libretto clean (describes the composition) while the registry describes how recordings reference it.

## Timing Weight Model (after refactoring)

| Number Type              | Weight Multiplier | Rationale                           |
|--------------------------|-------------------|-------------------------------------|
| Recitative (secco)       | 0.5×              | Freely timed, ~2× speech pace      |
| RecitativeAccompagnato   | 0.75×             | More metered, between recit & aria  |
| Aria, Duet, etc.         | 1.0×              | Normal sung pacing                  |
| Direction, Interlude     | MIN_WEIGHT (0.5)  | Fixed minimum, no text              |

## Backward Compatibility

All changes are additive:
- New `NumberType` variants serialize as new snake_case strings; old JSON without them still deserializes correctly via `Other`.
- `tempo_marking` is `Option<String>` with `skip_serializing_if`, so absent in old JSON.
- Internal refactoring of estimate/merge has no serialization impact.
