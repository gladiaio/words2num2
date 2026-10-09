# Local Testing Guide

This document describes how to set up the repository and run the test
suite locally before pushing changes.

## Prerequisites

- Python 3.10 or newer (`requires-python = ">=3.10"`; CI covers CPython
  3.10–3.15 and PyPy 3.11).
- A stable Rust toolchain (`rustup`): the package is a Rust extension built
  with maturin, so every install from a checkout compiles it.
- `pip` with virtualenv support.

## Setup

```bash
git clone https://github.com/gladiaio/words2num2
cd words2num2

python3 -m venv .venv
source .venv/bin/activate

# Builds the Rust core in place (maturin) and installs the dev tooling.
make install-dev
```

`make install-dev` runs `pip install -e .` (which compiles the core; re-run
it after any change under `rust/`) and installs pytest, the linters and
`num2words2`. The package itself has **no runtime dependencies**;
`num2words2` is only needed by `tests/test_multilang.py`, which renders
numbers with the published package and parses them back (it is skipped
without it).

The Rust core has its own tests, including the round-trip ratchet over
every language:

```bash
cd rust && cargo test --release -p words2num2-core -- --include-ignored
```

## Running tests

### Full suite

```bash
make test
# or
pytest tests/
```

### Verbose with per-test names

```bash
make test-verbose
# or
pytest tests/ -v
```

### A single file or a single test

```bash
pytest tests/test_lang_EN.py
pytest tests/test_auto_parse.py::test_currency
pytest tests/test_auto_parse.py -k pluralize
```

### Coverage

```bash
pytest tests/ --cov=words2num2 --cov-report=term-missing
```

This is what the `CI` workflow runs on every push. Aim for the same
local result before tagging a release.

## Test layout

| File | What it covers |
|---|---|
| `tests/test_lang_EN.py` | Hand-written English grammar parser. |
| `tests/test_dispatch.py` | Locale resolution and aliases. |
| `tests/test_api_contract.py` | Language codes, `to=` modes, keyword arguments, the CLI. |
| `tests/test_multilang.py` | Round trip `words2num(num2words(n, lang)) == n` for every language. Skipped if `num2words2` is not installed. |
| `tests/test_auto_parse.py` | `auto_parse`, `auto_parse_sentence`, `parse_number_string`, currencies, units, separators, pluralization. |
| `tests/test_sentence_*.py` | Sentence mode on real-world text and the currency fold. |
| `tests/test_reference_doctest.py` | Every `>>>` example in `REFERENCE.md`. |
| `rust/words2num2-core/tests/roundtrip.rs` | Rust round-trip ratchet (render, parse back) across all languages. |

## Linting and formatting

```bash
make lint                  # black --check, isort --check-only, flake8, mypy
make format                # apply black + isort
```

`make lint` runs exactly what the CI `Lint` workflow runs, and every one of
those checks fails the workflow. Run `make format` before committing.

## Building the package

```bash
make build
```

Produces an sdist and a wheel for the current platform, e.g.:

```
dist/words2num2-X.Y.Z.tar.gz
dist/words2num2-X.Y.Z-cp38-abi3-macosx_11_0_arm64.whl
```

The wheel is a CPython `abi3` wheel (one per platform for every CPython
3.10+). The `Release` workflow builds the same per platform and tests each
one before publishing.

## Smoke-testing a built wheel

Before tagging a release, verify the wheel installs and imports
cleanly in an isolated environment:

```bash
python3 -m venv /tmp/w2n-smoke
/tmp/w2n-smoke/bin/pip install dist/words2num2-*.whl

cd /tmp  # leave the repo so cwd doesn't shadow the install
/tmp/w2n-smoke/bin/python -c "
from words2num2 import words2num, auto_parse, __version__
print('version:', __version__)
print(words2num('forty-two'))
print(words2num('quarante-deux', lang='fr'))
print(auto_parse('\$12,345.00'))
"
```

> **Note:** when running `python -c` from inside the repo root, the
> current directory is added to `sys.path` and shadows the installed
> package. Always `cd` out before the smoke test.

## Cutting a release

See "Release process" in [CONTRIBUTING.md](CONTRIBUTING.md). In short: bump
the version in both `pyproject.toml` and `words2num2/_version.py` and merge
to `main`; `auto-release.yml` tags it and dispatches `release.yml` (version
check → build → test every native wheel and the sdist → PyPI + GitHub
Release), `crates.yml` and `aur-publish.yml`. Before bumping, run
`make test`, `make lint` and the smoke test above.

## Troubleshooting

**`ImportError: cannot import name '_rust'`.**
The compiled core has not been built in this checkout: run `pip install -e .`
(needs a Rust toolchain).

**`tests/test_multilang.py` is skipped.**
Install `num2words2` (`pip install num2words2`). Only that test file uses
it; the package does not.

**`Words2NumError: cannot parse '...' as a number`.**
The token isn't in the language's vocabulary as the core reads it. See
[CONTRIBUTING.md](CONTRIBUTING.md) for where per-language parsing lives.

**Smoke test prints the wrong version.**
You're running from inside the repo root. `cd /tmp` (or anywhere
outside the repo) before `python -c` — Python's auto-`sys.path[0]`
shadows the installed package with the source tree.

**A release did not reach PyPI.**
Re-run `Auto release` (Actions → Auto release → Run workflow): it
re-dispatches the release for any version that is tagged but not on PyPI.
If it keeps failing, check that:
- The PyPI Trusted Publisher has been configured at
  https://pypi.org/manage/project/words2num2/settings/publishing/ with
  workflow `release.yml` and environment `pypi`.
- The GitHub `pypi` environment exists at
  https://github.com/gladiaio/words2num2/settings/environments.
- The tag matches `v*` exactly (e.g. `v0.2.1`, not `0.2.1`).
