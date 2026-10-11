//! PyO3 binder for words2num2 — a thin shim over the pure-Rust
//! [`words2num2_core`] engine.
//!
//! All parsing logic (the reverse-table backend, the English grammar, the
//! sentence walker, `auto_parse`, number-format handling) lives in
//! `words2num2-core`, which has no Python dependency. This crate only:
//!   * exposes the same `#[pyfunction]` set the Python package calls, and
//!   * converts core native values / errors to and from Python objects.

use bigdecimal::BigDecimal;
use num_bigint::BigInt;
use pyo3::exceptions::{PyKeyError, PyNotImplementedError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3::IntoPyObjectExt;
use std::collections::HashMap;
use std::str::FromStr;

use words2num2_core::w2n_formats;
use words2num2_core::w2n_lang_en;
use words2num2_core::w2n_sentence;

// ---------------------------------------------------------------------------
// Python object / error helpers
// ---------------------------------------------------------------------------

/// Raise `words2num2.base.Words2NumError` (a `ValueError` subclass) carrying
/// `msg`, so callers' `except Words2NumError` keeps working. Falls back to a
/// plain `ValueError` if the Python class cannot be imported.
fn words2num_error(py: Python<'_>, msg: String) -> PyErr {
    match py
        .import("words2num2.base")
        .and_then(|b| b.getattr("Words2NumError"))
    {
        Ok(cls) => match cls.call1((msg.clone(),)) {
            Ok(v) => PyErr::from_value(v),
            Err(e) => e,
        },
        Err(_) => PyValueError::new_err(msg),
    }
}

/// Map a core sentence-layer error onto the matching Python exception class.
fn w2n_error_to_pyerr(py: Python<'_>, e: w2n_sentence::W2nError) -> PyErr {
    match e {
        w2n_sentence::W2nError::Words2Num(m) => words2num_error(py, m),
        w2n_sentence::W2nError::NotImplemented(m) => PyNotImplementedError::new_err(m),
        w2n_sentence::W2nError::Key(k) => PyKeyError::new_err(k),
    }
}

/// `decimal.Decimal(s)` — rebuilds an exact `Decimal` from its `str()` form.
fn make_decimal(py: Python<'_>, s: String) -> PyResult<Py<PyAny>> {
    let decimal = py.import("decimal")?.getattr("Decimal")?;
    Ok(decimal.call1((s,))?.unbind())
}

/// Convert a `w2n_lang_en::W2nValue` (`int` / `float` / `Decimal`) to Python.
/// `PyDec`'s `Display` is Python's `str(Decimal)`, so `Decimal(pydec.to_string())`
/// reproduces the value (including a signed zero) exactly.
fn en_value_to_py(py: Python<'_>, v: w2n_lang_en::W2nValue) -> PyResult<Py<PyAny>> {
    use w2n_lang_en::W2nValue;
    match v {
        W2nValue::Int(i) => i.into_py_any(py),
        W2nValue::Float(f) => f.into_py_any(py),
        W2nValue::Dec(d) => make_decimal(py, d.to_string()),
    }
}

/// Convert a `w2n_formats::W2nValue` to Python. `parse_number_string` only ever
/// yields `int` / `float`; the `Decimal` arm is here for completeness.
fn formats_value_to_py(py: Python<'_>, v: w2n_formats::W2nValue) -> PyResult<Py<PyAny>> {
    use w2n_formats::W2nValue;
    match v {
        W2nValue::Int(i) => i.into_py_any(py),
        W2nValue::Float(f) => f.into_py_any(py),
        W2nValue::Dec(d) => make_decimal(py, d.to_string()),
    }
}

/// Convert a `w2n_sentence::W2nValue` (`int` / `float` / `Decimal`) to Python.
fn sentence_value_to_py(py: Python<'_>, v: &w2n_sentence::W2nValue) -> PyResult<Py<PyAny>> {
    use w2n_sentence::W2nValue;
    match v {
        W2nValue::Int(i) => i.clone().into_py_any(py),
        W2nValue::Float(f) => (*f).into_py_any(py),
        W2nValue::Dec(_) => make_decimal(py, v.py_str()),
    }
}

/// Build a core `w2n_sentence::W2nValue` from a Python `int` / `float` /
/// `Decimal` (used by `pluralize`).
fn w2nvalue_from_py(obj: &Bound<'_, PyAny>) -> PyResult<w2n_sentence::W2nValue> {
    use pyo3::types::{PyFloat, PyInt};
    if obj.is_instance_of::<PyInt>() {
        return Ok(w2n_sentence::W2nValue::Int(obj.extract::<BigInt>()?));
    }
    if obj.is_instance_of::<PyFloat>() {
        return Ok(w2n_sentence::W2nValue::Float(obj.extract::<f64>()?));
    }
    // Decimal — go through its own `str()` so the (coefficient, exponent)
    // pair survives exactly.
    let s: String = obj.str()?.extract()?;
    BigDecimal::from_str(&s)
        .map(w2n_sentence::W2nValue::Dec)
        .map_err(|_| PyTypeError::new_err(format!("unsupported value {}", s)))
}

/// Build a `words2num2.converters.auto.Quantity` from the core struct. The
/// dataclass stays defined in Python (it is a public API type users import);
/// only the parsing logic lives in the core.
fn quantity_to_py(py: Python<'_>, q: w2n_sentence::Quantity) -> PyResult<Py<PyAny>> {
    let value = sentence_value_to_py(py, &q.value)?;
    let cls = py
        .import("words2num2.converters.auto")?
        .getattr("Quantity")?;
    let kwargs = PyDict::new(py);
    kwargs.set_item("value", value)?;
    kwargs.set_item("unit", q.unit)?;
    kwargs.set_item("unit_long", q.unit_long)?;
    kwargs.set_item("kind", q.kind)?;
    kwargs.set_item("confidence", q.confidence)?;
    kwargs.set_item("raw", q.raw)?;
    Ok(cls.call((), Some(&kwargs))?.unbind())
}

// ---------------------------------------------------------------------------
// #[pyfunction] surface — identical names/signatures to the historic shim
// ---------------------------------------------------------------------------

/// `_rust.normalize(text)` — `Words2Num_Base._normalize`; numerical symbols
/// stay as written unless `convert_numerical_symbols` (#972).
#[pyfunction]
#[pyo3(signature = (text, convert_numerical_symbols=false))]
fn normalize(text: &str, convert_numerical_symbols: bool) -> String {
    words2num2_core::normalize_public(text, convert_numerical_symbols)
}

/// `_rust.lookup(lang, text, ordinal, negative_words)`.
///
/// Returns `None` when the text is not in the reverse table.
#[pyfunction]
#[pyo3(signature = (lang, text, ordinal, negative_words))]
fn lookup(
    lang: &str,
    text: &str,
    ordinal: bool,
    negative_words: Vec<String>,
) -> PyResult<Option<i64>> {
    words2num2_core::lookup(lang, text, ordinal, &negative_words).map_err(|e| match e {
        words2num2_core::LookupError::NotImplemented(l) => PyNotImplementedError::new_err(l),
    })
}

/// `_rust.supported_langs()`.
#[pyfunction]
fn supported_langs() -> Vec<&'static str> {
    words2num2_core::supported_langs()
}

/// `_rust.parse_int(s)`.
#[pyfunction]
fn parse_int(s: &str) -> PyResult<i64> {
    words2num2_core::parse_int(s).map_err(|e| PyValueError::new_err(e.to_string()))
}

/// `_rust.en_to_cardinal(text)`.
#[pyfunction]
fn en_to_cardinal(py: Python<'_>, text: &str) -> PyResult<Py<PyAny>> {
    match words2num2_core::en_to_cardinal(text) {
        Ok(v) => en_value_to_py(py, v),
        Err(e) => Err(words2num_error(py, e.msg)),
    }
}

/// `_rust.en_to_ordinal(text)`.
#[pyfunction]
fn en_to_ordinal(py: Python<'_>, text: &str) -> PyResult<Py<PyAny>> {
    match words2num2_core::en_to_ordinal(text) {
        Ok(v) => en_value_to_py(py, v),
        Err(e) => Err(words2num_error(py, e.msg)),
    }
}

/// `_rust.en_to_year(text)`.
#[pyfunction]
fn en_to_year(py: Python<'_>, text: &str) -> PyResult<Py<PyAny>> {
    match words2num2_core::en_to_year(text) {
        Ok(v) => en_value_to_py(py, v),
        Err(e) => Err(words2num_error(py, e.msg)),
    }
}

/// `_rust.parse_number_string(s, thousands_sep, decimal_sep, lang)`.
#[pyfunction]
#[pyo3(signature = (s, thousands_sep=None, decimal_sep=None, lang=None))]
fn parse_number_string(
    py: Python<'_>,
    s: &str,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    lang: Option<&str>,
) -> PyResult<Py<PyAny>> {
    match w2n_formats::parse_number_string(s, thousands_sep, decimal_sep, lang) {
        Ok(v) => formats_value_to_py(py, v),
        Err(e) => Err(words2num_error(py, e.0)),
    }
}

/// `TypeError: <func>() got an unexpected keyword argument 'precision'` for the
/// first key of a non-empty `kwargs`, the message Python itself gives (#199).
fn reject_kwargs(func: &str, kwargs: Option<&Bound<'_, PyDict>>) -> PyResult<()> {
    if let Some((key, _)) = kwargs.and_then(|d| d.iter().next()) {
        return Err(PyTypeError::new_err(format!(
            "{func}() got an unexpected keyword argument {}",
            key.repr()?
        )));
    }
    Ok(())
}

/// `_rust.words2num(text, lang, to, kwargs)` — the public single-token entry.
///
/// Mirrors `words2num2.words2num(text, lang, to, **kwargs)`: the core owns the
/// whole dispatch (`_resolve_lang`, the en-vs-reverse-table choice, the `to`
/// mode selection). No conversion takes extra keyword arguments, so any key in
/// `kwargs` is a `TypeError` naming it, as Python's own would.
#[pyfunction]
#[pyo3(signature = (text, lang="en", to="cardinal", kwargs=None, convert_numerical_symbols=false))]
fn words2num(
    py: Python<'_>,
    text: &str,
    lang: &str,
    to: &str,
    kwargs: Option<Bound<'_, PyDict>>,
    convert_numerical_symbols: bool,
) -> PyResult<Py<PyAny>> {
    reject_kwargs("words2num", kwargs.as_ref())?;
    let (text, lang, to) = (text.to_owned(), lang.to_owned(), to.to_owned());
    // Pure Rust from here: other Python threads run meanwhile (#186).
    match py.detach(move || {
        words2num2_core::with_numerical_symbols(convert_numerical_symbols, || words2num2_core::words2num(&text, &lang, &to))
    }) {
        Ok(v) => en_value_to_py(py, v),
        Err(e) => Err(w2n_error_to_pyerr(py, e)),
    }
}

/// `_rust.words2num_sentence(sentence, lang, to, kwargs)`.
#[pyfunction]
#[pyo3(signature = (sentence, lang="en", to="cardinal", kwargs=None, currency=false, convert_numerical_symbols=false, date=false, region=None, date_format="written"))]
#[allow(clippy::too_many_arguments)]
fn words2num_sentence(
    py: Python<'_>,
    sentence: &str,
    lang: &str,
    to: &str,
    kwargs: Option<Bound<'_, PyDict>>,
    currency: bool,
    convert_numerical_symbols: bool,
    date: bool,
    region: Option<String>,
    date_format: &str,
) -> PyResult<String> {
    // A stray keyword used to make every conversion fail silently, handing the
    // sentence back unchanged (#201); it is a TypeError like in `words2num`.
    reject_kwargs("words2num_sentence", kwargs.as_ref())?;
    let (sentence, lang, to) = (sentence.to_owned(), lang.to_owned(), to.to_owned());
    let date_format = date_format.to_owned();
    // A long transcript takes a while: release the GIL for the walk (#186).
    py.detach(move || {
        words2num2_core::with_numerical_symbols(convert_numerical_symbols, || {
            w2n_sentence::words2num_sentence_full(&sentence, &lang, &to, false, currency, date, region.as_deref(), &date_format)
        })
    })
        .map_err(|e| w2n_error_to_pyerr(py, e))
}

/// `_rust.pluralize(long_form, value)`.
#[pyfunction]
#[pyo3(signature = (long_form, value))]
fn pluralize(long_form: Option<&str>, value: Bound<'_, PyAny>) -> PyResult<Option<String>> {
    let v = w2nvalue_from_py(&value)?;
    Ok(w2n_sentence::pluralize(long_form, &v))
}

/// `_rust.auto_parse(text, lang, prefer, thousands_sep, decimal_sep)`.
#[pyfunction]
#[pyo3(signature = (text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None, convert_numerical_symbols=false))]
fn auto_parse(
    py: Python<'_>,
    text: &str,
    lang: &str,
    prefer: Option<HashMap<String, String>>,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    convert_numerical_symbols: bool,
) -> PyResult<Py<PyAny>> {
    let prefer = prefer.unwrap_or_default();
    let (text, lang) = (text.to_owned(), lang.to_owned());
    let (ts, ds) = (thousands_sep.map(str::to_owned), decimal_sep.map(str::to_owned));
    let parsed = py.detach(move || {
        words2num2_core::with_numerical_symbols(convert_numerical_symbols, || {
            w2n_sentence::auto_parse(&text, &lang, &prefer, ts.as_deref(), ds.as_deref())
        })
    });
    match parsed {
        Ok(q) => quantity_to_py(py, q),
        Err(e) => Err(w2n_error_to_pyerr(py, e)),
    }
}

/// `_rust.auto_parse_sentence(text, lang, prefer, thousands_sep, decimal_sep,
/// expand)`.
#[pyfunction]
#[pyo3(signature = (text, lang="en", prefer=None, thousands_sep=None, decimal_sep=None, expand=false, convert_numerical_symbols=false))]
#[allow(clippy::too_many_arguments)]
fn auto_parse_sentence(
    py: Python<'_>,
    text: &str,
    lang: &str,
    prefer: Option<HashMap<String, String>>,
    thousands_sep: Option<&str>,
    decimal_sep: Option<&str>,
    expand: bool,
    convert_numerical_symbols: bool,
) -> PyResult<String> {
    let prefer = prefer.unwrap_or_default();
    let (text, lang) = (text.to_owned(), lang.to_owned());
    let (ts, ds) = (thousands_sep.map(str::to_owned), decimal_sep.map(str::to_owned));
    py.detach(move || {
        words2num2_core::with_numerical_symbols(convert_numerical_symbols, || {
            w2n_sentence::auto_parse_sentence(&text, &lang, &prefer, ts.as_deref(), ds.as_deref(), expand)
        })
    })
    .map_err(|e| w2n_error_to_pyerr(py, e))
}

#[pymodule]
fn _rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(normalize, m)?)?;
    m.add_function(wrap_pyfunction!(lookup, m)?)?;
    m.add_function(wrap_pyfunction!(supported_langs, m)?)?;
    m.add_function(wrap_pyfunction!(parse_int, m)?)?;
    m.add_function(wrap_pyfunction!(en_to_cardinal, m)?)?;
    m.add_function(wrap_pyfunction!(en_to_ordinal, m)?)?;
    m.add_function(wrap_pyfunction!(en_to_year, m)?)?;
    m.add_function(wrap_pyfunction!(parse_number_string, m)?)?;
    m.add_function(wrap_pyfunction!(words2num, m)?)?;
    m.add_function(wrap_pyfunction!(words2num_sentence, m)?)?;
    m.add_function(wrap_pyfunction!(pluralize, m)?)?;
    m.add_function(wrap_pyfunction!(auto_parse, m)?)?;
    m.add_function(wrap_pyfunction!(auto_parse_sentence, m)?)?;
    // The accepted `to=` modes, from the one list the core checks against.
    m.add("CONVERTER_TYPES", w2n_sentence::CONVERTER_TYPES.to_vec())?;
    Ok(())
}
