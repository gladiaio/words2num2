# -*- coding: utf-8 -*-
"""words2num2 CLI entrypoint.

$ words2num2 forty two
42
$ words2num2 --lang fr trois cent quatre
304
$ words2num2 --lang=en --to=ordinal twenty-first
21
"""

from __future__ import unicode_literals

import argparse
import sys

from . import CONVERTER_TYPES, Words2NumError, __version__, words2num


def _parser():
    p = argparse.ArgumentParser(
        prog="words2num2",
        description="Parse a number written in words into its numeric value.",
    )
    p.add_argument("text", nargs="+", help="the words to parse, e.g. forty two")
    p.add_argument(
        "--lang", default="en", help="language code (default: en), e.g. fr, pt-BR"
    )
    p.add_argument(
        "--to",
        default="cardinal",
        metavar="TYPE",
        help="conversion type: {} (default: cardinal)".format(
            " | ".join(CONVERTER_TYPES)
        ),
    )
    p.add_argument(
        "--convert-numerical-symbols",
        action="store_true",
        help="read numerical symbols such as ① or ² as their digits",
    )
    p.add_argument(
        "--version", action="version", version="words2num2 {}".format(__version__)
    )
    return p


def main(argv=None):
    args = _parser().parse_args(argv)
    try:
        value = words2num(
            " ".join(args.text),
            lang=args.lang,
            to=args.to,
            convert_numerical_symbols=args.convert_numerical_symbols,
        )
    except (Words2NumError, NotImplementedError) as exc:
        # An unparseable input or an unsupported --lang/--to is a user error:
        # one line on stderr and a non-zero status, not a traceback.
        print("words2num2: error: {}".format(exc), file=sys.stderr)
        return 1
    print(value)
    return 0


if __name__ == "__main__":
    sys.exit(main())
