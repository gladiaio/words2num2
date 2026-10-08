# -*- coding: utf-8 -*-
"""Run every ``>>>`` example in REFERENCE.md (#198).

The reference used to document names that no longer exist
(``CONVERTER_CLASSES``, ``UNITS``, ``words2num2/lang_XX.py``); running its
examples keeps it honest. ``python -m doctest REFERENCE.md`` cannot be used
directly: the closing code fence would be read as expected output, so each
```python block is extracted and run on its own, sharing one namespace.
"""
import doctest
import pathlib
import re

import pytest

import words2num2

REFERENCE = pathlib.Path(__file__).resolve().parent.parent / "REFERENCE.md"
FENCE = re.compile(r"^```python\n(.*?)^```", re.S | re.M)


def _blocks():
    if not REFERENCE.exists():  # e.g. tests copied without the docs
        return []
    text = REFERENCE.read_text(encoding="utf-8")
    out = []
    for m in FENCE.finditer(text):
        line = text.count("\n", 0, m.start()) + 2
        if ">>>" in m.group(1):
            out.append(pytest.param(m.group(1), line, id="REFERENCE.md:%d" % line))
    return out


BLOCKS = _blocks()


@pytest.mark.skipif(not BLOCKS, reason="REFERENCE.md not found next to tests/")
@pytest.mark.parametrize("source, line", BLOCKS)
def test_reference_examples_issue_198(source, line):
    globs = {name: getattr(words2num2, name) for name in words2num2.__all__}
    test = doctest.DocTestParser().get_doctest(
        source, globs, "REFERENCE.md", str(REFERENCE), line - 1
    )
    runner = doctest.DocTestRunner(optionflags=doctest.ELLIPSIS)
    runner.run(test)
    result = runner.summarize(verbose=False)
    assert result.failed == 0, "REFERENCE.md example at line %d failed" % line
