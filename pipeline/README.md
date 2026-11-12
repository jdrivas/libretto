# libretto-pipeline

Builds time-synced sung-text files ("interchange files") for opera recordings, from the
opera's born-digital vocal score and the recordings. Design and decisions:
`docs/sung-text-pipeline.md`; per-opera progress: `operas/<opera>/STATUS.md`.

## Use

```
cd pipeline
uv sync                                  # core (decode); add --extra align --extra audio later
.venv/bin/libretto-pipeline --version
.venv/bin/libretto-pipeline decode figaro
.venv/bin/libretto-pipeline status figaro
.venv/bin/python -m pytest -q            # the Figaro tests need the licensed score
```

Global options: `--log-level {trace,debug,info,warn,error,off}` (default info), `--log-utc`.
`--version` prints `libretto-pipeline <version> <build time> (<hash>[-dirty])`; the build
metadata is captured at install, so refresh it with `uv sync --reinstall-package libretto-pipeline`.

## Layout

| Path | What |
|---|---|
| `src/libretto_pipeline/cli.py` | the command line (stages are subcommands) |
| `src/libretto_pipeline/decode/` | stage 1: score PDF -> pieces, manifest, per-piece decode |
| `experiment/` | the experiment scripts, copied, to be wrapped into stages (P4) |
| `config.toml` | machine paths: `work_root` |
| `../operas/<id>/opera.toml` | one opera: score, edition, cast |

## Where outputs go

Everything bulky or licensed goes under `work_root/<opera>/` (outside git): score decodes,
audio, stems, timings. Only code, configuration, documentation and (later) the shareable
interchange files are committed. Score PDFs and decodes are never shared.

## Stages so far

**decode** writes `work_root/<opera>/decode/`:
- `manifest.json` / `manifest.md`: one entry per piece (id, kind, act and scene, pages,
  bars, metre and tempo, characters, first words, sections, problems) and the checks
  against the score's own contents;
- `pieces/<id>.score.json`: bars, notes and syllables of each piece, bars numbered as printed
  (an upbeat is bar 0);
- `contents.json`: the score's table of contents.

Piece ids: `no19` (a number), `no18-recit` / `no18-aria` (the two parts of a "Recitativo
istrumentato ed Aria"), `rec19a` (the first secco recitative after No. 19), `sinfonia`, and
`app-…` for the appendix's alternative versions.
