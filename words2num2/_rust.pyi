# Type stubs for the compiled extension (rust/words2num2-py/src/lib.rs).
# Keep in step with the #[pyfunction] signatures there.
from decimal import Decimal
from typing import Dict, List, Optional, Sequence, Union

from .converters.auto import Quantity

Number = Union[int, float, Decimal]

CONVERTER_TYPES: List[str]

def normalize(text: str, convert_numerical_symbols: bool = False) -> str: ...
def lookup(
    lang: str, text: str, ordinal: bool, negative_words: Sequence[str]
) -> Optional[int]: ...
def supported_langs() -> List[str]: ...
def parse_int(s: str) -> int: ...
def en_to_cardinal(text: str) -> Number: ...
def en_to_ordinal(text: str) -> Number: ...
def en_to_year(text: str) -> Number: ...
def parse_number_string(
    s: str,
    thousands_sep: Optional[str] = None,
    decimal_sep: Optional[str] = None,
    lang: Optional[str] = None,
) -> Union[int, float]: ...
def words2num(
    text: str,
    lang: str = "en",
    to: str = "cardinal",
    kwargs: Optional[Dict[str, object]] = None,
    convert_numerical_symbols: bool = False,
) -> Number: ...
def words2num_sentence(
    sentence: str,
    lang: str = "en",
    to: str = "cardinal",
    kwargs: Optional[Dict[str, object]] = None,
    currency: bool = False,
    convert_numerical_symbols: bool = False,
) -> str: ...
def pluralize(long_form: Optional[str], value: Number) -> Optional[str]: ...
def auto_parse(
    text: str,
    lang: str = "en",
    prefer: Optional[Dict[str, str]] = None,
    thousands_sep: Optional[str] = None,
    decimal_sep: Optional[str] = None,
    convert_numerical_symbols: bool = False,
) -> Quantity: ...
def auto_parse_sentence(
    text: str,
    lang: str = "en",
    prefer: Optional[Dict[str, str]] = None,
    thousands_sep: Optional[str] = None,
    decimal_sep: Optional[str] = None,
    expand: bool = False,
    convert_numerical_symbols: bool = False,
) -> str: ...
