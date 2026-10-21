"""Builds the evaluation stream from the NCBI BioSample full dump.

    uv run python benchmarks/biosample/prepare.py biosample_set.xml.gz --out human.jsonl.gz

The dump is ``biosample_set.xml.gz`` from https://ftp.ncbi.nlm.nih.gov/biosample/. The stream holds one record per
submission batch of the ``Human.1.0`` package — a batch is the records one owner submitted on one day, and its first
record is the one with the earliest ``submission_date`` (then the lowest ``id``). Records are written in that order, one
JSON object per line:

    {"id": 2615835, "date": "2014-02-03T13:34:39.640", "owner": "...",
     "targets": {"age": "1 year", "sex": "female", "tissue": "blood"},
     "context": [["isolate", "45.01"]]}

``targets`` holds the six compared fields (by their harmonized attribute name, first non-empty value, trimmed and lower
cased); ``context`` holds every other non-empty attribute as (submitted name, trimmed value).
"""

from __future__ import annotations

import argparse
import gzip
import json
import re
import sys
import time
import xml.etree.ElementTree as ET
from collections.abc import Iterator
from typing import BinaryIO

PACKAGE = "Human.1.0"

# The six compared fields: the harmonized attributes filled most often among the batches' first records, leaving out
# names, identifiers, providers, sample types, dates, places and free text (sample_name, sample_title, isolate,
# bioproject_accession, biomaterial_provider, sample_type, collection_date, geo_loc_name, lat_lon, description) and any
# whose distinct values exceed half its filled records.
SLOTS = ("tissue", "sex", "age", "cell_line", "cell_type", "disease")

# Values that mean "no value", compared lower cased after trimming. The list is the "n/a" entry of the synonym table in
# the LAFF replication package (Belgacem et al., "A Machine Learning Approach for Automated Filling of Categorical
# Fields in Data Entry Forms", ACM TOSEM 2022, doi:10.1145/3533021; https://figshare.com/articles/software/14866167),
# which evaluates form filling on the same BioSample package.
EMPTY = frozenset(
    {
        "",
        "-",
        "_",
        "a sexual",
        "missing",
        "n/a",
        "na",
        "nan",
        "neuter",
        "none",
        "not",
        "not applicable",
        "not available",
        "not collected",
        "not determined",
        "not provided",
        "not reported",
        "null",
        "tissue",
        "unaffected",
        "unknown",
        "unspecified",
    }
)

PACKAGE_TAG = re.compile(rb"<Package[^>]*>([^<]*)</Package>")


def normalize(value: str | None) -> str | None:
    """Trimmed and lower cased, or ``None`` when the value means no value."""
    v = (value or "").strip().lower()
    return None if v in EMPTY else v


def blocks(f: BinaryIO, package: str) -> Iterator[bytes]:
    """The XML of each ``<BioSample>`` of ``package``.

    The dump writes each record as lines from one that starts with ``<BioSample `` to one that starts with
    ``</BioSample>``, so only the records of the package are parsed as XML.
    """
    block: list[bytes] = []
    found: str | None = None
    inside = False
    for line in f:
        if line.startswith(b"<BioSample "):
            if inside:
                raise ValueError("a <BioSample> starts inside another")
            inside, block, found = True, [line], None
            continue
        if not inside:
            continue
        block.append(line)
        if found is None and b"<Package" in line:
            m = PACKAGE_TAG.search(line)
            found = m.group(1).decode() if m else ""
        if line.startswith(b"</BioSample>"):
            inside = False
            if found == package:
                yield b"".join(block)


def row(el: ET.Element) -> dict:
    """One record as a stream row: the compared fields and the rest of its attributes."""
    targets: dict[str, str] = {}
    context: list[list[str]] = []
    for a in el.iterfind("Attributes/Attribute"):
        harmonized = a.get("harmonized_name")
        name = a.get("attribute_name") or harmonized or ""
        if harmonized in SLOTS:
            if harmonized not in targets:
                v = normalize(a.text)
                if v is not None:
                    targets[harmonized] = v
            continue
        if normalize(a.text) is not None:
            context.append([name, (a.text or "").strip()])
    return {
        "id": int(el.get("id") or 0),
        "date": el.get("submission_date") or "",
        "owner": el.findtext("Owner/Name") or "",
        "targets": targets,
        "context": context,
    }


def first_of_batches(f: BinaryIO, package: str) -> list[dict]:
    """The first record of each (owner, submission day) batch, in submission order."""
    first: dict[tuple[str, str], dict] = {}
    seen = 0
    start = time.monotonic()
    for xml in blocks(f, package):
        el = ET.fromstring(xml)
        seen += 1
        date = el.get("submission_date") or ""
        rid = int(el.get("id") or 0)
        key = (el.findtext("Owner/Name") or "", date[:10])
        held = first.get(key)
        if held is None or (date, rid) < (held["date"], held["id"]):
            first[key] = row(el)
        if seen % 200_000 == 0:
            print(
                f"{seen:,} {package} records, {len(first):,} batches, "
                f"{time.monotonic() - start:,.0f}s",
                file=sys.stderr,
                flush=True,
            )
    print(f"{seen:,} {package} records, {len(first):,} batches", file=sys.stderr)
    return sorted(first.values(), key=lambda r: (r["date"], r["id"]))


def main() -> int:
    ap = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    ap.add_argument("dump", help="biosample_set.xml.gz")
    ap.add_argument("--out", required=True, help="the stream to write (.jsonl.gz)")
    ap.add_argument("--package", default=PACKAGE)
    args = ap.parse_args()
    with gzip.open(args.dump, "rb") as f:
        rows = first_of_batches(f, args.package)
    with gzip.open(args.out, "wt", encoding="utf-8") as w:
        for r in rows:
            w.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"{len(rows):,} rows -> {args.out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
