"""The `decode` stage: whole score PDF -> pieces, a manifest, and a decode per piece.

Writes under <work>/<opera>/decode/ (licensed data: stays local, never committed):
  pieces/<id>.score.json   bars, notes, syllables of one piece (bars numbered as printed)
  contents.json            the score's own table of contents
  manifest.json            one entry per piece (see pieces.describe), plus checks
  manifest.md              the same, as a table to review
"""
import json
import logging
import time

import pymupdf

from . import contents, pieces, vector
from .report import manifest_markdown

log = logging.getLogger(__name__)


def run(opera):
    t0 = time.time()
    out = opera.work / "decode"
    (out / "pieces").mkdir(parents=True, exist_ok=True)
    pdf = pymupdf.open(opera.score_pdf)
    log.info(f"decoding {opera.score_pdf.name}: {len(pdf)} PDF pages, music from page {opera.first_music_page}")

    pages, cues = [], {}
    for p in range(opera.first_music_page, len(pdf) + 1):
        page = pdf[p - 1]
        sp = vector.spans(page)
        pages.append((p, vector.read_page(page, p), pieces.page_headings(sp)))
        cues[p] = pieces.page_cues(sp)
        if p % 100 == 0:
            log.debug(f"  page {p}")
    n_sys = sum(len(s) for _, s, _ in pages)
    log.info(f"read {len(pages)} pages, {n_sys} systems in {time.time() - t0:.1f} s")

    toc = contents.read_contents(pdf, opera.contents_pages)
    raw = pieces.split(pages)
    unmatched = pieces.name(raw, toc, opera.printed_page)
    summaries = []
    for pc in raw:
        summary, dec = pieces.describe(pc, opera.cast, cues, opera.printed_page)
        summaries.append(summary)
        (out / "pieces" / f"{summary['id']}.score.json").write_text(
            json.dumps({"id": summary["id"], "edition": opera.edition, **dec}, ensure_ascii=False))
    ids = [s["id"] for s in summaries]
    dupes = sorted({i for i in ids if ids.count(i) > 1})

    checks = {
        "pieces": len(summaries),
        "with_problems": sum(1 for s in summaries if s["problems"]),
        "contents_entries": len(toc),
        "contents_not_found": [{k: e[k] for k in ("page", "kind", "title")} for e in unmatched],
        "pieces_not_in_contents": [s["id"] for s, pc in zip(summaries, raw) if pc["contents"] is None],
        "duplicate_ids": dupes,
    }
    manifest = {"opera": opera.id, "title": opera.title, "edition": opera.edition,
                "score_pdf": opera.score_pdf.name, "checks": checks, "pieces": summaries}
    (out / "contents.json").write_text(json.dumps(toc, ensure_ascii=False, indent=1))
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=1))
    (out / "manifest.md").write_text(manifest_markdown(manifest))
    log.info(f"{checks['pieces']} pieces, {checks['with_problems']} with problems; "
             f"{len(unmatched)} contents entries not found, "
             f"{len(checks['pieces_not_in_contents'])} pieces not in the contents")
    log.info(f"wrote {out} in {time.time() - t0:.1f} s")
    return manifest
