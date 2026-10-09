# Contributing to words2num2

`words2num2` is the inverse of [num2words2](https://github.com/gladiaio/num2words2).
Contributions are very welcome.

All parsing lives in the Rust core, `rust/words2num2-core`; the
`words2num2/` Python package is a thin binder over it
(`rust/words2num2-py` is the PyO3 glue). There are no per-language Python
modules.

## Where to start

| What you want to do | Where to look |
|---|---|
| Fix English parsing | `rust/words2num2-core/src/w2n_lang_en.rs` |
| Fix another language | `rust/words2num2-core/src/lib.rs` (reverse lookup table, scale composition) and `w2n_sentence.rs` (per-language dispatch) |
| Sentence mode | `rust/words2num2-core/src/w2n_sentence.rs` |
| Add a unit or currency (auto-parse) | `UNITS` / `CURRENCIES` in `rust/words2num2-core/src/w2n_sentence.rs` |
| Spoken currency amounts | `rust/words2num2-core/src/w2n_currency.rs` |
| Per-locale separator defaults | `NUMBER_FORMAT_DEFAULTS` in `rust/words2num2-core/src/w2n_formats.rs` |
| Python signatures and docstrings | `words2num2/__init__.py`, plus `words2num2/_rust.pyi` for the extension's types |

For local setup and testing, see [LOCAL_TESTING.md](LOCAL_TESTING.md). For
the full API surface, see [REFERENCE.md](REFERENCE.md).

## How a language is parsed

English has a hand-written grammar. Every other language is read through a
reverse lookup table, built lazily by rendering `-1..10000` with the
embedded num2words2 renderer (`num2words2-core`) and inverting it; larger
numbers are composed from the language's scale words, derived the same way.
Prefer deriving per-language data from the renderer over hand lists; add a
per-language table only where the language needs one (ordinal endings,
clitics, gender forms).

Add tests next to the code (Rust unit tests) or in `tests/*.py`, and run the
round-trip ratchet (`rust/words2num2-core/tests/roundtrip.rs`).

## Coding style

- **Black + isort** with the project's `pyproject.toml` config —
  `make format` applies both.
- **Flake8** with the project's `.flake8`, and **mypy** on `words2num2`.
- `make lint` runs all four checks exactly as the CI `Lint` workflow does,
  and each one fails the workflow.
- Rust: `cargo fmt` and `cargo clippy` in `rust/`.

The package has **no runtime dependencies** (`dependencies = []`): the
num2words2 renderer is compiled in. Keep it that way.

## Running the test suite

```bash
make install-dev
make test
```

Detail in [LOCAL_TESTING.md](LOCAL_TESTING.md).

## Release process

A release is a version bump merged into `main`: set the same version in
`pyproject.toml` and `words2num2/_version.py`. On that merge the
`Auto release` workflow (`.github/workflows/auto-release.yml`) tags
`vX.Y.Z` and dispatches the `Release` workflow (checks the tag,
`pyproject.toml` and `_version.py` agree, builds the wheels and sdist, runs
the test suite against each native wheel and the sdist, then creates a
GitHub Release and uploads to PyPI via Trusted Publishing), the crates.io
publish and the AUR bump. It does nothing when the version is already on
PyPI, and fails when the two version files disagree. When the version is
tagged but not on PyPI (a failed release), it re-dispatches the release on
the existing tag: re-run it from the Actions tab to retry.

Tagging by hand still works (`git tag vX.Y.Z && git push origin vX.Y.Z`),
for a release that is not main's head; the tag must match both version
files. Pre-flight checklist:

- [ ] All tests pass (`make test`).
- [ ] `CHANGELOG.md` updated.
- [ ] README / REFERENCE updated if public API changed.
- [ ] Smoke test on a built wheel from outside the repo.
