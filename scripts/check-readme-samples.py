#!/usr/bin/env python3
"""Runs the Python samples of a README, in the order a reader meets them, against the installed `gil` package.

A sample that does not run is a defect in the README or in the package — the README is what a first user copies.
Fixtures stand in for the files and names a sample says the reader has; they live here, not in the README.

    python scripts/check-readme-samples.py [README.md]            # the installed gil
    uv run --no-project --isolated --with <wheel> python scripts/check-readme-samples.py

The samples of one README share names, as the text does ("the schema above"), and each `##` section starts from the
fixtures it needs. A sample that cannot run as written (a placeholder for the reader's own model, say) is marked with an
HTML comment on the line before it — `<!-- sample: skip — why -->` — and the reason is printed, so a skip is a choice a
reviewer sees. Exit 0 when every other sample runs, 1 otherwise.
"""

from __future__ import annotations

import csv
import json
import re
import sys
import tempfile
import traceback
from datetime import date, timedelta
from pathlib import Path

DEFAULT = Path(__file__).resolve().parent.parent / "bindings" / "python" / "README.md"
SKIP = re.compile(r"<!--\s*sample:\s*skip\b(.*?)-->")
FENCE = re.compile(r"^```(\w*)\s*$")


def tickets(directory: Path) -> None:
    """`tickets.csv` (the table of the README's first section) and the same rows as `settled.jsonl`."""
    teams = {"vpn": "network", "mail": "messaging", "wifi": "network", "invoice": "billing"}
    summaries = {
        "vpn": "vpn drops every hour",
        "mail": "cannot send mail from the phone",
        "wifi": "wifi is slow in the east wing",
        "invoice": "invoice shows the wrong total",
    }
    rows = []
    start = date(2026, 1, 1)
    for i in range(160):
        product = list(teams)[i % len(teams)]
        rows.append(
            {
                "ticket_id": f"t{i:04}",
                "closed_on": (start + timedelta(days=i * 2)).isoformat(),
                "product": product,
                "summary": f"{summaries[product]} ({i})",
                "team": teams[product],
            }
        )
    with (directory / "tickets.csv").open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    with (directory / "settled.jsonl").open("w", encoding="utf-8") as f:
        for row in rows:
            f.write(
                json.dumps(
                    {
                        "id": row["ticket_id"],
                        "settled_at": row["closed_on"] + "T00:00:00.0000000+00:00",
                        "values": {
                            "product": row["product"],
                            "summary": row["summary"],
                            "team": row["team"],
                        },
                        "settlements": {"team": "Enter"},
                    }
                )
                + "\n"
            )


def inbox(directory: Path) -> None:
    """`inbox.jsonl` of the text-sorting section: one line per message a person sorted."""
    topics = {
        "billing": "where is my refund for the last order",
        "shipping": "my parcel has not arrived yet",
        "account": "i cannot sign in to my account",
    }
    with (directory / "inbox.jsonl").open("w", encoding="utf-8") as f:
        for i in range(240):
            category = list(topics)[i % len(topics)]
            f.write(
                json.dumps(
                    {
                        "id": f"m{i}",
                        "settled_at": f"2026-09-{1 + i // 24:02}T{i % 24:02}:00:00.0000000+00:00",
                        "values": {"text": f"{topics[category]} {i}", "category": category},
                        "settlements": {"category": "Enter"},
                    }
                )
                + "\n"
            )


# Where a `##` section starts from: the files it reads. A section not named keeps the files it has.
FIXTURES = {
    "": tickets,
    "Sorting text into categories": inbox,
}


def samples(text: str) -> list[tuple[int, str, str, str | None]]:
    """(line, section, code, skip reason) of every python fence, in order."""
    out: list[tuple[int, str, str, str | None]] = []
    section = ""
    lines = text.splitlines()
    i = 0
    previous = ""
    while i < len(lines):
        line = lines[i]
        if line.startswith("## "):
            section = line[3:].strip()
        found = FENCE.match(line)
        if found:
            body = []
            start = i + 1
            i += 1
            while i < len(lines) and not lines[i].startswith("```"):
                body.append(lines[i])
                i += 1
            if found.group(1) == "python":
                marked = SKIP.search(previous)
                out.append((start, section, "\n".join(body) + "\n", marked.group(1).strip(" —-") if marked else None))
        if line.strip():
            previous = line
        i += 1
    return out


def main() -> int:
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT
    found = samples(path.read_text(encoding="utf-8"))
    failures = 0
    ran = 0
    with tempfile.TemporaryDirectory() as tmp:
        directory = Path(tmp)
        import os

        os.chdir(directory)
        names: dict[str, object] = {"__name__": "__readme__"}
        section = None
        for line, heading, code, skip in found:
            if heading != section:
                section = heading
                FIXTURES.get(heading, lambda d: None)(directory)
                if heading == "" and section is not None:
                    pass
            if skip is not None:
                print(f"skip   {path.name}:{line}  {skip}")
                continue
            try:
                exec(compile(code, f"{path.name}:{line}", "exec"), names)  # noqa: S102
                ran += 1
                print(f"ok     {path.name}:{line}  [{heading or 'top'}]")
            except Exception:
                failures += 1
                print(f"FAILED {path.name}:{line}  [{heading or 'top'}]")
                print(traceback.format_exc(limit=3))
    print(f"{ran} samples ran, {failures} failed, {sum(1 for s in found if s[3] is not None)} skipped by mark")
    return 1 if failures else 0


if __name__ == "__main__":
    raise SystemExit(main())
