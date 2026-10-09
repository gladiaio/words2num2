# -*- coding: utf-8 -*-
"""#186: the Rust calls release the GIL, so other threads run meanwhile."""

import threading
import time

from words2num2 import auto_parse_sentence, words2num_sentence


def _ticks_during(call):
    """Count main-thread loop turns strictly inside `call`, run in a thread."""
    span = {}

    def worker():
        span["start"] = time.perf_counter()
        call()
        span["end"] = time.perf_counter()

    t = threading.Thread(target=worker)
    stamps = []
    t.start()
    while t.is_alive():
        stamps.append(time.perf_counter())
    t.join()
    start, end = span["start"], span["end"]
    assert end - start > 0.2, "the call is too short to tell"
    margin = 0.05
    return sum(1 for s in stamps if start + margin < s < end - margin)


def test_words2num_sentence_releases_the_gil():
    text = " ".join(["zweitausend"] * 4000)
    assert _ticks_during(lambda: words2num_sentence(text, lang="de")) > 100


def test_auto_parse_sentence_releases_the_gil():
    text = " ".join(["it costs $1,234.50 for 12 kg"] * 400000)
    assert _ticks_during(lambda: auto_parse_sentence(text, lang="de")) > 100
