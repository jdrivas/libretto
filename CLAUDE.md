# libretto: workspace notes for Claude

This repo is where operas are built into time-synced sung-text files for the roon-rd
viewer. Start a session here and say which opera; read `docs/todo.md` (what's next) and
`operas/<opera>/STATUS.md` first.

## What's here

| Path | What |
|---|---|
| `pipeline/` | `libretto-pipeline`, the Python tool that builds an opera stage by stage (README there) |
| `operas/<id>/` | per opera: `opera.toml` (score, edition, cast) and `STATUS.md` (where the build is) |
| `docs/sung-text-pipeline.md` | the method and the operating model |
| `docs/todo.md` | the one list of what's next, here and in roon-rd |
| `docs/decisions.md` | the decision log: every decision, and the ones to revisit |
| `docs/repertoire-plan.md` | which operas, in what order, and the checkpoints |
| `docs/report/` | the technical report on the experiment |
| `crates/` | the earlier Rust libretto model (February word-count approach); not used by the pipeline |

## Rules that hold in every session

- **Licensed scores.** The user's Bärenreiter (and later Schott) PDFs are personal licensed
  copies. Never share, publish or commit a score PDF or a full decode: no share links, public
  locations or artifacts. Storing them on the user's own private cloud drives (Google Drive,
  Dropbox) is allowed (D26). A single page or small
  excerpt for study is fine; crop the "Issued for" footer and the margin order ID. Small
  decoded data in interchange files (the sung text, an `ocr` field) is fine.
- **Audio library is read-only.** Never modify or write anything under `~/Dropbox/Audio`.
- **Bulk and licensed outputs go to `work_root`** (`pipeline/config.toml`), never into git.
  Since 2026-11-18 (D27) that is `libretto-work` in Google Drive on the external disk:
  `~/Google Drive/My Drive/Projects/roon-rd/Libretto/libretto-work`, whose real path is
  `/Volumes/JDR Desktop Attached/Google Drive/Projects/roon-rd/Libretto/libretto-work`.
  Use real paths in settings, because the sandbox matches real paths only (`~/Development`
  is likewise a symlink to `~/Dropbox/Development`). The uv cache is outside Drive, in
  `/Volumes/JDR Desktop Attached/libretto-cache/uv`.
- **No hand-marking.** Timings come from the pipeline. Never propose tapping along or
  marking times by hand; don't re-propose snapping to anchors.
- **Every line accounted for.** After any change to timed lines or the viewer, check that no
  line is missing and none is shown twice.
- **Gloss in the session.** For now Claude writes the verse list, phrase split and literal
  gloss in the session, and the tool checks them. Revisit moving it into the tool (Claude
  API) at the latest at the Così checkpoint.
- **The edition is the reference.** Numbers, scenes, pages and bars follow the decoded
  vocal score (for Figaro, Bärenreiter BA 4565), not Ricordi or a libretto.
- **Decision log.** Every decision made in a session gets an entry in `docs/decisions.md`
  (date, decision, why, alternatives, revisit). It goes in the same commit as the change,
  roon-rd decisions included. When the user asks to come back to a decision, add it to the
  log's "To revisit" table with its trigger, and raise it when the trigger arrives.
- **To-do list.** `docs/todo.md` is the only list of what's next, roon-rd items included.
  When work finishes an item, tick it and move it to "Done" in the same commit. When
  something to do later comes up, add it straight away, with a pointer to its detail. When
  a "To revisit" trigger in decisions.md fires, put that item under "Now". Answer "what's
  next?" from this file, and don't keep a separate list in STATUS.md or in memory.
- **Discussion first** for non-trivial decisions; no code until the user says go. Commit only
  when asked, on a branch, never pushed unless asked. Never bump a version.

## Running

```
cd pipeline && uv sync && .venv/bin/libretto-pipeline decode figaro
```
