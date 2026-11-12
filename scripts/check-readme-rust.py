#!/usr/bin/env python3
"""Builds and runs the Rust samples of a crate README exactly as a reader would paste them.

A `cargo test --doc` run passes a sample that a reader cannot use: rustdoc hides `# ` lines and wraps a sample that has
no `fn main` — but crates.io and GitHub show the hidden lines, and the reader gets no wrapper. This check puts every
```rust block of a README, verbatim, into the `src/main.rs` of a fresh cargo project that depends on the crate by path,
and runs it. A block that does not compile or exits non-zero is a defect in the README or in the crate.

    python scripts/check-readme-rust.py [crates/<name>/README.md ...]    # default: every crate README that has a block

A block meant for a library (it has no `main` — the exported functions of a C ABI, say) is marked with
`<!-- sample: lib -->` on the line before it: it goes into `src/lib.rs` and must build. A block that cannot run as
written is marked `<!-- sample: skip — why -->` instead, and the reason is printed, so a skip is a choice a reviewer
sees.
Exit 0 when every other block runs, 1 otherwise. Set CARGO_TARGET_DIR to reuse build output between runs.
"""

from __future__ import annotations

import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKIP = re.compile(r"<!--\s*sample:\s*skip\b(.*?)-->")
LIB = re.compile(r"<!--\s*sample:\s*lib\s*-->")
EDITION = re.compile(r'^edition\s*=\s*"([^"]+)"', re.MULTILINE)
FENCE = re.compile(r"^```(\w*)\s*$")
NAME = re.compile(r'^name\s*=\s*"([^"]+)"', re.MULTILINE)


def shown(readme: Path) -> str:
    """The README's path from the repository root, or the path as given when it lies elsewhere."""
    try:
        return str(readme.relative_to(ROOT))
    except ValueError:
        return str(readme)


def rust_blocks(readme: Path) -> list[tuple[int, str, str | None, bool]]:
    """(line of the opening fence, the block as written, the skip reason or None, library?) for every ```rust block."""
    blocks: list[tuple[int, str, str | None, bool]] = []
    lines = readme.read_text(encoding="utf-8").split("\n")
    i = 0
    while i < len(lines):
        m = FENCE.match(lines[i])
        if m and m.group(1) == "rust":
            skip = SKIP.search(lines[i - 1]) if i else None
            library = bool(LIB.search(lines[i - 1])) if i else False
            j = i + 1
            while j < len(lines) and not lines[j].startswith("```"):
                j += 1
            blocks.append(
                (
                    i + 1,
                    "\n".join(lines[i + 1 : j]) + "\n",
                    skip.group(1).strip(" —-") if skip else None,
                    library,
                )
            )
            i = j
        i += 1
    return blocks


def run(readme: Path, target_dir: str) -> int:
    crate = readme.parent
    package = NAME.search((crate / "Cargo.toml").read_text(encoding="utf-8")).group(1)
    edition = EDITION.search((ROOT / "Cargo.toml").read_text(encoding="utf-8")).group(1)
    failures = 0
    for line, source, skip, library in rust_blocks(readme):
        label = f"{shown(readme)}:{line}"
        target = "lib" if library else "main"
        if skip is not None:
            print(f"skip  {label} — {skip}")
            continue
        with tempfile.TemporaryDirectory() as tmp:
            project = Path(tmp)
            (project / "src").mkdir()
            (project / "Cargo.toml").write_text(
                f'[package]\nname = "readme-sample"\nversion = "0.0.0"\nedition = "{edition}"\n\n'
                f'[dependencies]\n{package} = {{ path = "{crate.as_posix()}" }}\n\n[workspace]\n',
                encoding="utf-8",
            )
            (project / "src" / f"{target}.rs").write_text(source, encoding="utf-8")
            env = {
                **os.environ,
                "CARGO_TARGET_DIR": target_dir,
                "CARGO_TERM_COLOR": "never",
            }
            done = subprocess.run(
                ["cargo", "build" if library else "run", "--quiet"],
                cwd=project,
                env=env,
                capture_output=True,
                text=True,
                check=False,
            )
        if done.returncode == 0:
            print(f"ok    {label}")
        else:
            failures += 1
            print(
                f"FAIL  {label} — pasted verbatim into src/{target}.rs of a new project that depends on `{package}`"
            )
            print(
                "\n".join(
                    "      " + l
                    for l in (done.stderr or done.stdout).strip().split("\n")[-25:]
                )
            )
    return failures


def main(argv: list[str]) -> int:
    readmes = [Path(a).resolve() for a in argv] or sorted(
        (ROOT / "crates").glob("*/README.md")
    )
    readmes = [r for r in readmes if rust_blocks(r)]
    if not readmes:
        print("no Rust blocks found")
        return 1
    with tempfile.TemporaryDirectory() as scratch:
        target_dir = os.environ.get("CARGO_TARGET_DIR") or scratch
        failures = sum(run(r, target_dir) for r in readmes)
    print(f"{'FAILED' if failures else 'passed'}: {failures} Rust sample(s) failed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
