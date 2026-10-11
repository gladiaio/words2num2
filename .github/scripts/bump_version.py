"""Bump every version string of a release in one go.

    python .github/scripts/bump_version.py 0.4.1           # Python 0.4.1, crate patch +1
    python .github/scripts/bump_version.py 0.4.0 --core 0.2.0

Rewrites pyproject.toml, words2num2/_version.py, rust/Cargo.toml (workspace version, used by
words2num2-core and words2num2-py), the two entries in rust/Cargo.lock, and turns the
CHANGELOG's `## [Unreleased]` section into `## [<version>] — <today>` under a fresh, empty
`## [Unreleased]`. Prints the new Python and crate versions.
"""

import argparse
import datetime
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]


def sub_once(path, pattern, repl):
    p = ROOT / path
    text = p.read_text(encoding="utf-8")
    new, n = re.subn(pattern, repl, text, count=1, flags=re.M)
    if n != 1:
        sys.exit(f"{path}: pattern {pattern!r} not found")
    p.write_text(new, encoding="utf-8")


def crate_version():
    text = (ROOT / "rust/Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'(?ms)^\[workspace\.package\].*?^version = "([^"]+)"', text)
    if not m:
        sys.exit("rust/Cargo.toml: no [workspace.package] version")
    return m.group(1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("version", help="new Python package version, X.Y.Z")
    ap.add_argument("--core", help="new crate version (default: current crate version, patch + 1)")
    ap.add_argument("--date", default=datetime.date.today().isoformat())
    a = ap.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", a.version):
        sys.exit("version must be X.Y.Z")
    core = a.core
    if not core:
        major, minor, patch = (int(x) for x in crate_version().split("."))
        core = f"{major}.{minor}.{patch + 1}"
    if not re.fullmatch(r"\d+\.\d+\.\d+", core):
        sys.exit("--core must be X.Y.Z")

    sub_once("pyproject.toml", r'^version = "[^"]+"', f'version = "{a.version}"')
    sub_once("words2num2/_version.py", r'^__version__ = "[^"]+"', f'__version__ = "{a.version}"')
    sub_once(
        "rust/Cargo.toml",
        r'(\[workspace\.package\][^\[]*?^version = )"[^"]+"',
        rf'\g<1>"{core}"',
    )
    for crate in ("words2num2-core", "words2num2-py"):
        sub_once(
            "rust/Cargo.lock",
            rf'(^name = "{re.escape(crate)}"\nversion = )"[^"]+"',
            rf'\g<1>"{core}"',
        )
    cl = ROOT / "CHANGELOG.md"
    text = cl.read_text(encoding="utf-8")
    if f"## [{a.version}]" not in text:
        new, n = re.subn(
            r"^## \[Unreleased\]\n",
            f"## [Unreleased]\n\n## [{a.version}] — {a.date}\n",
            text,
            count=1,
            flags=re.M,
        )
        if n != 1:
            sys.exit("CHANGELOG.md: no '## [Unreleased]' heading")
        # An empty [Unreleased] (fixes merged without a changelog line) still gets a body.
        new = new.replace(
            f"## [{a.version}] — {a.date}\n\n## [",
            f"## [{a.version}] — {a.date}\n\nMaintenance release; see the commits since the previous version.\n\n## [",
            1,
        )
        cl.write_text(new, encoding="utf-8")
    print(f"python={a.version} core={core}")


if __name__ == "__main__":
    main()
