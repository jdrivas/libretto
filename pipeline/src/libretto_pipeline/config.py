"""Workspace and opera configuration.

  pipeline/config.toml        machine paths (work_root: where bulk outputs go)
  operas/<id>/opera.toml      one opera: titles, score, edition, cast
"""
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
PIPELINE = REPO / "pipeline"
OPERAS = REPO / "operas"


def workspace():
    return tomllib.loads((PIPELINE / "config.toml").read_text())


@dataclass
class Cast:
    """Characters and the staff labels and cues that name them."""
    members: list = field(default_factory=list)  # [(name, [alternatives])]

    def find(self, text):
        t = " " + re.sub(r"\s+", " ", text.lower().replace(".", " ")).replace("ii ", "il ") + " "
        out = set()
        for name, alts in self.members:
            if any(re.search(rf"(?<![a-zà-ù]){re.escape(a)}(?![a-zà-ù])", t) for a in alts):
                out.add(name)
        return out

    def order(self, name):
        names = [n for n, _ in self.members]
        return names.index(name) if name in names else len(names)


@dataclass
class Opera:
    id: str
    title: str
    composer: str
    language: str
    score_pdf: Path
    edition: str
    first_music_page: int
    contents_pages: list
    cast: Cast
    work: Path

    def printed_page(self, pdf_page):
        return pdf_page - self.first_music_page + 1


def load_opera(opera_id):
    path = OPERAS / opera_id / "opera.toml"
    if not path.exists():
        raise SystemExit(f"no such opera: {path}")
    c = tomllib.loads(path.read_text())
    work_root = Path(workspace()["work_root"]).expanduser()
    cast = Cast([(m["name"], [a.lower() for a in m["labels"]]) for m in c.get("cast", [])])
    first, last = c["score"]["contents_pages"]
    return Opera(id=opera_id, title=c["opera"]["title"], composer=c["opera"]["composer"],
                 language=c["opera"]["language"], score_pdf=Path(c["score"]["pdf"]).expanduser(),
                 edition=c["score"]["edition"], first_music_page=c["score"]["first_music_page"],
                 contents_pages=list(range(first, last + 1)), cast=cast, work=work_root / opera_id)
