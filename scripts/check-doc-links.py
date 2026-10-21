"""Check that every relative link in the repository's Markdown points at something that exists.

Usage: python3 scripts/check-doc-links.py

Reads every tracked *.md file (git ls-files), follows inline links and reference definitions that are not URLs,
and reports a link whose file or folder is missing, or whose #anchor names no heading in the target document
(GitHub's heading ids). Every README.md here is also a package page (NuGet, crates.io, PyPI), where a relative
file link has nothing to resolve against, so a README may link within itself but must give other files as URLs.
Prints `file:line: target — reason` for each and exits 1 when any is broken.
Standard library only, so CI runs it without installing anything.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
INLINE = re.compile(r"!?\[(?:[^\]\\]|\\.)*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
REFERENCE = re.compile(r"^\s{0,3}\[[^\]]+\]:\s*<?(\S+?)>?(?:\s+.*)?$")
CODE_SPAN = re.compile(r"`+[^`]*`+")
HEADING = re.compile(r"^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$")
SCHEME = re.compile(r"^[a-z][a-z0-9+.-]*:", re.IGNORECASE)


def tracked_markdown() -> list[Path]:
    out = subprocess.run(
        ["git", "ls-files", "-z", "*.md"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    return [ROOT / name for name in out.split("\0") if name]


def prose_lines(path: Path) -> list[tuple[int, str]]:
    """The document's lines outside fenced code blocks, with their 1-based numbers."""
    lines: list[tuple[int, str]] = []
    fence: str | None = None
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        stripped = line.lstrip()
        marker = stripped[:3]
        if marker in ("```", "~~~"):
            if fence is None:
                fence = marker
            elif marker == fence:
                fence = None
            continue
        if fence is None:
            lines.append((number, line))
    return lines


def slug(text: str) -> str:
    """GitHub's heading id: lower case, markup and punctuation dropped, spaces to hyphens."""
    text = re.sub(r"!?\[([^\]]*)\]\([^)]*\)", r"\1", text)  # a link keeps its text
    text = text.replace("`", "").lower()
    text = re.sub(r"[^\w\- ]", "", text)
    return text.replace(" ", "-")


def anchors(path: Path, cache: dict[Path, set[str]]) -> set[str]:
    if path not in cache:
        seen: dict[str, int] = {}
        ids: set[str] = set()
        for _, line in prose_lines(path):
            match = HEADING.match(line)
            if not match:
                continue
            base = slug(match.group(2))
            count = seen.get(base, 0)
            seen[base] = count + 1
            ids.add(base if count == 0 else f"{base}-{count}")
        cache[path] = ids
    return cache[path]


def problems(path: Path, cache: dict[Path, set[str]]) -> list[tuple[int, str, str]]:
    found: list[tuple[int, str, str]] = []
    for number, line in prose_lines(path):
        text = CODE_SPAN.sub("", line)
        targets = INLINE.findall(text)
        reference = REFERENCE.match(text)
        if reference:
            targets.append(reference.group(1))
        for target in targets:
            if SCHEME.match(target):
                continue
            file, _, anchor = target.partition("#")
            if file and path.name == "README.md":
                found.append((number, target, "relative file link in a packaged README — use a URL"))
                continue
            dest = (path.parent / unquote(file)).resolve() if file else path
            if not dest.exists():
                found.append((number, target, "no such file"))
            elif anchor and dest.suffix == ".md" and unquote(anchor).lower() not in anchors(dest, cache):
                found.append((number, target, "no such heading"))
    return found


def main() -> int:
    cache: dict[Path, set[str]] = {}
    broken = 0
    for path in tracked_markdown():
        for number, target, reason in problems(path, cache):
            print(f"{path.relative_to(ROOT).as_posix()}:{number}: {target} — {reason}")
            broken += 1
    if broken:
        print(f"{broken} broken link(s)")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
