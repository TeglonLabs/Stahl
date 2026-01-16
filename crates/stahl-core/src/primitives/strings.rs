use crate::gc::Gc;
use crate::values::lists::{List, StahlList};

use crate::rvals::{RestArgsIter, Result, StahlByteVector, StahlString, StahlVal};
use crate::stahl_vm::builtin::BuiltInModule;
use crate::{stop, Vector};

use num::{BigInt, Num};
use stahl_derive::{function, native};

/// Strings in Stahl are immutable, fixed length arrays of characters. They are heap allocated, and
/// are implemented under the hood as referenced counted Rust `Strings`. Rust `Strings` are stored
/// as UTF-8 encoded bytes.
#[stahl_derive::define_module(name = "stahl/strings")]
pub fn string_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/strings");
    module
        .register_native_fn_definition(STRING_APPEND_DEFINITION)
        .register_native_fn_definition(TO_STRING_DEFINITION)
        .register_native_fn_definition(STRING_TO_LIST_DEFINITION)
        .register_native_fn_definition(STRING_TO_UPPER_DEFINITION)
        .register_native_fn_definition(STRING_TO_LOWER_DEFINITION)
        .register_native_fn_definition(STRING_LENGTH_DEFINITION)
        .register_native_fn_definition(TRIM_DEFINITION)
        .register_native_fn_definition(TRIM_START_DEFINITION)
        .register_native_fn_definition(TRIM_END_DEFINITION)
        .register_native_fn_definition(SPLIT_WHITESPACE_DEFINITION)
        .register_native_fn_definition(SPLIT_ONCE_DEFINITION)
        .register_native_fn_definition(SPLIT_MANY_DEFINITION)
        .register_native_fn_definition(STRING_TO_INT_DEFINITION)
        .register_native_fn_definition(INT_TO_STRING_DEFINITION)
        .register_native_fn_definition(STRING_TO_SYMBOL_DEFINITION)
        .register_native_fn_definition(STARTS_WITH_DEFINITION)
        .register_native_fn_definition(ENDS_WITH_DEFINITION)
        .register_native_fn_definition(TRIM_END_MATCHES_DEFINITION)
        .register_native_fn_definition(TRIM_START_MATCHES_DEFINITION)
        .register_native_fn_definition(STRING_REF_DEFINITION)
        .register_native_fn_definition(SUBSTRING_DEFINITION)
        .register_native_fn_definition(MAKE_STRING_DEFINITION)
        .register_native_fn_definition(STRING_EQUALS_DEFINITION)
        .register_native_fn_definition(STRING_CI_EQUALS_DEFINITION)
        .register_native_fn_definition(STRING_LESS_THAN_DEFINITION)
        .register_native_fn_definition(STRING_CI_LESS_THAN_DEFINITION)
        .register_native_fn_definition(STRING_LESS_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(STRING_CI_LESS_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(STRING_GREATER_THAN_DEFINITION)
        .register_native_fn_definition(STRING_CI_GREATER_THAN_DEFINITION)
        .register_native_fn_definition(STRING_GREATER_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(STRING_CI_GREATER_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(STRING_CONSTRUCTOR_DEFINITION)
        .register_native_fn_definition(STRING_TO_NUMBER_DEFINITION)
        .register_native_fn_definition(NUMBER_TO_STRING_DEFINITION)
        .register_native_fn_definition(REPLACE_DEFINITION)
        .register_native_fn_definition(CHAR_UPCASE_DEFINITION)
        .register_native_fn_definition(CHAR_DOWNCASE_DEFINITION)
        .register_native_fn_definition(CHAR_IS_DIGIT_DEFINITION)
        .register_native_fn_definition(CHAR_IS_WHITESPACE_DEFINITION)
        .register_native_fn_definition(CHAR_TO_NUMBER_DEFINITION)
        .register_native_fn_definition(CHAR_EQUALS_DEFINITION)
        .register_native_fn_definition(CHAR_GREATER_THAN_DEFINITION)
        .register_native_fn_definition(CHAR_GREATER_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(CHAR_LESS_THAN_DEFINITION)
        .register_native_fn_definition(CHAR_LESS_THAN_EQUAL_TO_DEFINITION)
        .register_native_fn_definition(CHAR_TO_INTEGER_DEFINITION)
        .register_native_fn_definition(INTEGER_TO_CHAR_DEFINITION)
        .register_native_fn_definition(STRING_TO_BYTES_DEFINITION)
        .register_native_fn_definition(STRING_TO_VECTOR_DEFINITION)
        .register_native_fn_definition(STRING_JOIN_DEFINITION)
        .register_native_fn_definition(STRING_CONTAINS_DEFINITION);

    module
}

macro_rules! monotonic {
    ($iter:expr, $compare:expr) => {{
        let mut iter = $iter;

        let Some(mut last)= iter.next().transpose()? else {
            stop!(ArityMismatch => "expected at least one argument");
        };

        let comparator = $compare;

        for maybe_item in iter {
            let item = maybe_item?;

            if comparator(&last, &item) {
                last = item;
            } else {
                return Ok(StahlVal::BoolV(false));
            }
        }

        Ok(StahlVal::BoolV(true))
    }};
}

/// Checks if all characters are equal.
///
/// Requires that all inputs are characters, and will otherwise raise an error.
///
/// (char=? char1 char2 ...) -> bool?
///
/// * char1 : char?
/// * char2 : char?
#[function(name = "char=?", constant = true)]
pub fn char_equals(rest: RestArgsIter<char>) -> Result<StahlVal> {
    monotonic!(rest, |ch1: &_, ch2: &_| ch1 == ch2)
}

fn number_to_string_impl(value: &StahlVal, radix: Option<u32>) -> Result<StahlVal> {
    match value {
        StahlVal::IntV(v) => {
            if let Some(radix) = radix {
                Ok(StahlVal::StringV(
                    radix_fmt::radix(*v, radix as u8).to_string().into(),
                ))
            } else {
                Ok(StahlVal::StringV(v.to_string().into()))
            }
        }
        StahlVal::NumV(n) => Ok(StahlVal::StringV(n.to_string().into())),
        StahlVal::BigNum(n) => Ok(StahlVal::StringV(n.to_string().into())),
        _ => stop!(TypeMismatch => "number->string expects a number type, found: {}", value),
    }
}

/// Converts the given number to a string.
#[function(name = "number->string", constant = true)]
pub fn number_to_string(value: &StahlVal, mut rest: RestArgsIter<'_, isize>) -> Result<StahlVal> {
    let radix = rest.next();

    let radix = if let Some(radix) = radix {
        let radix = radix?;

        if radix < 2 || radix > 16 {
            stop!(ContractViolation => "radix value given to string->number must be between 2 and 16, found: {}", radix);
        }

        Some(radix as u32)
    } else {
        None
    };

    number_to_string_impl(value, radix)
}

fn string_to_number_impl(value: &str, radix: Option<u32>) -> Result<StahlVal> {
    let expr = crate::parser::parser::Parser::parse(value)?;

    if expr.len() != 1 {
        return Ok(StahlVal::BoolV(false));
    }

    let explicit_radix = matches!(
        value.get(0..2),
        Some("#x") | Some("#d") | Some("#o") | Some("#b")
    );

    let implicit_radix = radix.filter(|_| !explicit_radix);

    let number = expr.into_iter().next().unwrap();

    let svalue = StahlVal::try_from(number)?;

    match (svalue, implicit_radix) {
        (StahlVal::IntV(_), Some(radix)) => match isize::from_str_radix(value, radix) {
            Ok(parsed) => Ok(StahlVal::IntV(parsed)),
            Err(_) => Ok(StahlVal::BoolV(false)),
        },
        (val @ StahlVal::IntV(_), None) => Ok(val),
        (StahlVal::BigNum(_), Some(radix)) => match BigInt::from_str_radix(value, radix) {
            Ok(parsed) => Ok(StahlVal::BigNum(Gc::new(parsed))),
            Err(_) => Ok(StahlVal::BoolV(false)),
        },

        (val @ StahlVal::BigNum(_), None) => Ok(val),
        (svalue @ StahlVal::NumV(_), _) => Ok(svalue),
        _ => Ok(StahlVal::BoolV(false)),
    }
}

/// Converts the given string to a number, with an optional radix.
/// On failure, it returns `#f`
///
/// (string->number digits [radix]) -> (or/c number? boolean?)
///
/// * digits : string?
/// * radix : number?
#[function(name = "string->number", constant = true)]
pub fn string_to_number(
    value: &StahlString,
    mut rest: RestArgsIter<'_, isize>,
) -> Result<StahlVal> {
    let radix = rest.next();

    let radix = if let Some(radix) = radix {
        let radix = radix?;

        if radix < 2 || radix > 16 {
            stop!(ContractViolation => "radix value given to string->number must be between 2 and 16, found: {}", radix);
        }

        Some(radix as u32)
    } else {
        None
    };

    match string_to_number_impl(value.as_str(), radix) {
        Ok(v) => Ok(v),
        Err(_) => Ok(StahlVal::BoolV(false)),
    }
}

/// Constructs a string from the given characters
#[function(name = "string")]
pub fn string_constructor(rest: RestArgsIter<'_, char>) -> Result<StahlVal> {
    rest.collect::<Result<String>>().map(|x| x.into())
}

// TODO: avoid allocation in `-ci` variants

macro_rules! impl_str_comparison {
    (-ci, $name:ident, $ext_name:literal, $mode:literal, $op:expr) => {
        #[doc = concat!("Compares strings lexicographically (as in\"", $mode, "\"),")]
        #[doc = "in a case insensitive fashion."]
        #[doc = ""]
        #[doc = concat!("(", $ext_name, " s1 s2 ... ) -> bool?")]
        #[doc = "* s1 : string?"]
        #[doc = "* s2 : string?"]
        #[function(name = $ext_name, constant = true)]
        pub fn $name(rest: RestArgsIter<&StahlString>) -> Result<StahlVal> {
            monotonic!(rest.map(|val| val.map(|s| s.as_str().to_lowercase())), $op)
        }
    };
    ($name:ident, $ext_name:literal, $mode:literal, $op:expr) => {
        #[doc = concat!("Compares strings lexicographically (as in\"", $mode, "\").")]
        #[doc = ""]
        #[doc = concat!("(", $ext_name, " s1 s2 ... ) -> bool?")]
        #[doc = "* s1 : string?"]
        #[doc = "* s2 : string?"]
        #[function(name = $ext_name, constant = true)]
        pub fn $name(rest: RestArgsIter<&StahlString>) -> Result<StahlVal> {
            monotonic!(rest, $op)
        }
    };
}

impl_str_comparison!(
    string_less_than,
    "string<?",
    "less-than",
    |s1: &_, s2: &_| s1 < s2
);
impl_str_comparison!(
    string_less_than_equal_to,
    "string<=?",
    "less-than-equal-to",
    |s1: &_, s2: &_| s1 <= s2
);
impl_str_comparison!(
    string_greater_than,
    "string>?",
    "greater-than",
    |s1: &_, s2: &_| s1 > s2
);
impl_str_comparison!(
    string_greater_than_equal_to,
    "string>=?",
    "greater-than-or-equal",
    |s1: &_, s2: &_| s1 >= s2
);
impl_str_comparison!(
    -ci,
    string_ci_less_than,
    "string-ci<?",
    "less-than",
    |s1: &_, s2: &_| s1 < s2
);
impl_str_comparison!(
    -ci,
    string_ci_less_than_equal_to,
    "string-ci<=?",
    "less-than-or-equal",
    |s1: &_, s2: &_| s1 <= s2
);
impl_str_comparison!(
    -ci,
    string_ci_greater_than,
    "string-ci>?",
    "greater-than",
    |s1: &_, s2: &_| s1 > s2
);
impl_str_comparison!(
    -ci,
    string_ci_greater_than_equal_to,
    "string-ci>=?",
    "greater-than-or-equal",
    |s1: &_, s2: &_| s1 >= s2
);

/// Compares strings for equality.
///
/// (string=? string1 string2 ...) -> bool?
///
/// * string1 : string?
/// * string2 : string?
#[function(name = "string=?", constant = true)]
pub fn string_equals(rest: RestArgsIter<&StahlString>) -> Result<StahlVal> {
    monotonic!(rest, |s1: &_, s2: &_| s1 == s2)
}

/// Compares strings for equality, in a case insensitive fashion.
#[function(name = "string-ci=?", constant = true)]
pub fn string_ci_equals(rest: RestArgsIter<&StahlString>) -> Result<StahlVal> {
    monotonic!(
        rest.map(|val| val.map(|s| s.as_str().to_lowercase())),
        |s1: &_, s2: &_| s1 == s2
    )
}

/// Extracts the nth character out of a given string.
///
/// (string-ref str n) -> char?
///
/// * str : string?
/// * n : int?
#[function(name = "string-ref", constant = true)]
pub fn string_ref(value: &StahlString, index: usize) -> Result<StahlVal> {
    let res = if index < value.len() {
        value.chars().nth(index)
    } else {
        None
    };

    if let Some(ch) = res {
        Ok(StahlVal::CharV(ch))
    } else {
        stop!(Generic => "string-ref: index out of bounds: index: {}, string length: {}", index, value.len());
    }
}

/// Creates a substring slicing the characters between two indices.
///
/// (substring str start end) -> string?
///
/// * str: string?
/// * start : int?
/// * end : int?
///
/// # Examples
/// ```scheme
/// (substring "hello" 1 4) ;; => "ell"
/// (substring "hello" 10 15) ;; => error
/// ```
#[function(name = "substring", constant = true)]
pub fn substring(
    value: &StahlString,
    i: usize,
    mut rest: RestArgsIter<'_, isize>,
) -> Result<StahlVal> {
    let j = rest.next().transpose()?;

    if rest.next().is_some() {
        stop!(ArityMismatch => "substring expects 1 or 2 arguments");
    }

    let range = bounds(value.as_str(), Some(i as isize), j, "substring")?;

    Ok(StahlVal::StringV(value[range].into()))
}

/// Creates a string of a given length, filled with an optional character
/// (which defaults to `#\0`).
///
/// (make-string len [char]) -> string?
///
/// * len : int?
/// * char : char? = #\0
#[function(name = "make-string")]
pub fn make_string(k: usize, mut c: RestArgsIter<'_, char>) -> Result<StahlVal> {
    // If the char is there, we want to take it
    let char = c.next();

    // We want the iterator to be exhaused
    if let Some(next) = c.next() {
        stop!(ArityMismatch => format!("make-string expected 1 or 2 arguments, got an additional argument {}", next?))
    }

    let c = char.unwrap_or(Ok('\0'))?;
    Ok((0..k).into_iter().map(|_| c).collect::<String>().into())
}

/// Replaces all occurrences of a pattern into the given string
///
/// (string-replace str from to) -> string?
///
/// * str : string?
/// * from : string?
/// * to : string?
///
/// # Examples
/// ```scheme
/// (string-replace "hello world" "o" "@") ;; => "hell@ w@rld"
/// ```
#[function(name = "string-replace")]
pub fn replace(value: &StahlString, from: &StahlString, to: &StahlString) -> Result<StahlVal> {
    Ok(StahlVal::StringV(
        value.replace(from.as_str(), to.as_str()).into(),
    ))
}

/// Concatenates all of the inputs to their string representation, separated by spaces.
///
/// (to-string xs ...)
///
/// * xs : any/c
///
/// # Examples
/// ```scheme
/// > (to-string 10) ;; => "10"
/// > (to-string 10 20) ;; => "10 20"
/// > (to-string "hello" "world") ;; => "hello world"
/// ```
#[native(name = "to-string", arity = "AtLeast(0)")]
pub fn to_string(args: &[StahlVal]) -> Result<StahlVal> {
    let mut error_message = String::new();

    if let Some((first, rest)) = args.split_first() {
        error_message.push_str(first.to_string().trim_matches('\"'));

        for arg in rest {
            let error_val = arg.to_string();
            error_message.push(' ');
            error_message.push_str(error_val.trim_matches('\"'));
        }
    }

    Ok(StahlVal::StringV(error_message.into()))
}

/// Converts a string into a symbol.
///
/// (string->symbol string?) -> symbol?
///
/// # Examples
///
/// ```scheme
/// > (string->symbol "FooBar") ;; => 'FooBar
/// ```
#[function(name = "string->symbol", constant = true)]
pub fn string_to_symbol(value: StahlString) -> StahlVal {
    StahlVal::SymbolV(value)
}

/// Converts an integer into a string.
///
/// (int->string int?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (int->string 10) ;; => "10"
/// ```
#[function(name = "int->string")]
pub fn int_to_string(value: isize) -> String {
    format!("{value}")
}

/// Converts a string into an int. Raises an error if the string cannot be converted to an integer.
///
/// (string->int string?) -> int?
///
/// # Examples
///
/// ```scheme
/// > (string->int "100") ;; => 10
/// > (string->int "not-an-int") ;; error
/// ```
#[function(name = "string->int")]
pub fn string_to_int(value: &StahlString) -> Result<StahlVal> {
    let parsed_int = value.parse::<isize>();
    match parsed_int {
        Ok(n) => Ok(StahlVal::IntV(n)),
        Err(_) => {
            stop!(TypeMismatch => "could not convert number to integer");
        }
    }
}

/// Converts a string into a list of characters.
///
/// (string->list s [start] [end]) -> (listof char?)
///
/// * s : string?
/// * start : int? = 0
/// * end : int?
///
/// # Examples
///
/// ```scheme
/// > (string->list "hello") ;; => '(#\h #\e #\l #\l #\o)
/// ```
#[function(name = "string->list")]
pub fn string_to_list(value: &StahlString, mut rest: RestArgsIter<isize>) -> Result<StahlVal> {
    let i = rest.next().transpose()?;
    let j = rest.next().transpose()?;

    if rest.next().is_some() {
        stop!(ArityMismatch => "string->list expects up to 3 arguments");
    }

    let range = bounds(value.as_str(), i, j, "string->list")?;

    Ok(value[range]
        .chars()
        .map(StahlVal::CharV)
        .collect::<List<_>>()
        .into())
}

/// Creates a new uppercased version of the input string
///
/// (string->upper string?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (string->upper "lower") ;; => "LOWER"
/// ```
#[function(name = "string->upper")]
pub fn string_to_upper(value: &StahlString) -> String {
    value.to_uppercase()
}

/// Creates a new lowercased version of the input string
///
/// (string->lower string?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (string->lower "sPonGeBoB tExT") ;; => "spongebob text"
/// ```
#[function(name = "string->lower")]
pub fn string_to_lower(value: &StahlString) -> String {
    value.to_lowercase()
}

/// Returns a new string with the leading and trailing whitespace removed.
///
/// (trim string?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (trim "   foo     ") ;; => "foo"
/// ```
#[function(name = "trim")]
pub fn trim(value: &StahlString) -> String {
    value.trim().into()
}

/// Returns a new string with the leading whitespace removed.
///
/// (trim string?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (trim "   foo     ") ;; => "foo     "
/// ```
#[function(name = "trim-start")]
pub fn trim_start(value: &StahlString) -> String {
    value.trim_start().into()
}

/// Returns a new string with the trailing whitespace removed.
///
/// (trim string?) -> string?
///
/// # Examples
///
/// ```scheme
/// > (trim "   foo     ") ;; => "   foo"
/// ```
#[function(name = "trim-end")]
pub fn trim_end(value: &StahlString) -> String {
    value.trim_end().into()
}

/// Returns a new string with the given `pat` repeatedly removed from the end
/// of the string
///
/// ```scheme
/// (trim-end-matches string? string?) -> string?
/// ```
///
/// # Examples
/// ```scheme
/// > (trim-end-matches "123foo1bar123123" "123") ;; => "123foo1bar"
/// ```
#[function(name = "trim-end-matches")]
pub fn trim_end_matches(value: &StahlString, pat: &StahlString) -> String {
    value.trim_end_matches(pat.as_str()).into()
}

/// Returns a new string with the given `pat` repeatedly removed from the start
/// of the string
///
/// ```scheme
/// (trim-start-matches string? string?) -> string?
/// ```
///
/// # Examples
/// ```scheme
/// > (trim-start-matches "123foo1bar123123" "123") ;; => "foo1bar123123"
/// ```
#[function(name = "trim-start-matches")]
pub fn trim_start_matches(value: &StahlString, pat: &StahlString) -> String {
    value.trim_start_matches(pat.as_str()).into()
}

/// Returns a list of strings from the original string split on the whitespace
///
/// (split-whitespace string?) -> (listof string?)
///
/// # Examples
///
/// ```scheme
/// (split-whitespace "apples bananas fruits veggies") ;; '("apples" "bananas" "fruits" "veggies")
/// ```
#[function(name = "split-whitespace")]
pub fn split_whitespace(value: &StahlString) -> StahlVal {
    let split: List<StahlVal> = value
        .split_whitespace()
        .map(|x| StahlVal::StringV(x.into()))
        .collect();
    split.into()
}

/// Splits a string given a separator at most once, yielding
/// a list with at most 2 elements.
///
/// (split-once str pat) -> string?
///
/// * str : string?
/// * pat : string?
///
/// # Examples
/// ```scheme
/// (split-once "foo,bar,baz" ",") ;; => '("foo" "bar,baz")
/// (split-once "foo|bar|" "|") ;; => '("foo" "bar|")
/// (split-once "" "&") ;; => '("")
/// ```
#[function(name = "split-once")]
pub fn split_once(value: &StahlString, pat: &StahlString) -> StahlVal {
    let split: Option<List<StahlVal>> = value
        .split_once(pat.as_str())
        .map(|(x, y)| vec![StahlVal::StringV(x.into()), StahlVal::StringV(y.into())].into());
    split.into()
}

/// Splits a string given a separator pattern into a list of strings.
///
/// (split-many str pat) -> (listof string?)
///
/// * str : string?
/// * pat : string?
///
/// # Examples
/// ```scheme
/// (split-many "foo,bar,baz" ",") ;; => '("foo" "bar" "baz")
/// (split-many "foo|bar|" "|") ;; => '("foo" "bar" "")
/// (split-many "" "&") ;; => '("")
/// ```
#[function(name = "split-many")]
pub fn split_many(value: &StahlString, pat: &StahlString) -> StahlVal {
    let split: List<StahlVal> = value
        .split(pat.as_str())
        .map(|x| StahlVal::StringV(x.into()))
        .collect();
    split.into()
}

/// Checks if the input string starts with a prefix
///
/// (starts-with? input pattern) -> bool?
///
/// * input : string?
/// * pattern: string?
///
/// # Examples
///
/// ```scheme
/// > (starts-with? "foobar" "foo") ;; => #true
/// > (starts-with? "foobar" "bar") ;; => #false
/// ```
#[function(name = "starts-with?")]
pub fn starts_with(value: &StahlString, prefix: &StahlString) -> bool {
    value.starts_with(prefix.as_str())
}

/// Checks if the input string ends with a given suffix
///
/// (ends-with? input pattern) -> bool?
///
///    input : string?
///    pattern: string?
///
/// # Examples
///
/// ```scheme
/// > (ends-with? "foobar" "foo") ;; => #false
/// > (ends-with? "foobar" "bar") ;; => #true
/// ```
#[function(name = "ends-with?")]
pub fn ends_with(value: &StahlString, suffix: &StahlString) -> bool {
    value.ends_with(suffix.as_str())
}

/// Get the length of the given string in UTF-8 bytes.
///
/// (string-length string?) -> int?
///
/// # Examples
///
/// ```scheme
/// > (string-length "apples") ;; => 6
/// > (string-length "✅") ;; => 3
/// > (string-length "🤖") ;; => 4
/// ```
#[function(name = "string-length")]
pub fn string_length(value: &StahlString) -> usize {
    value.len()
}

/// Concatenates all of the given strings into one
///
/// (string-append strs...) -> string?
///
/// * strs ... : string?
///
/// # Examples
/// ```scheme
/// > (string-append) ;; => ""
/// > (string-append "foo" "bar") ;; => "foobar"
/// ```
#[function(name = "string-append")]
pub fn string_append(mut rest: RestArgsIter<'_, &StahlString>) -> Result<StahlVal> {
    rest.0
        .try_fold("".to_string(), |accum, next| Ok(accum + next?.as_str()))
        .map(|x| StahlVal::StringV(x.into()))
}

macro_rules! impl_char_comparison {
    ($name:ident, $ext_name:literal, $mode:literal, $op:expr) => {
        #[doc = concat!("Compares characters according to their codepoints, in a \"", $mode, "\" fashion.")]
        #[doc = ""]
        #[doc = concat!("(", $ext_name, " char1 char2 ... ) -> bool?")]
        #[doc = "* char1 : char?"]
        #[doc = "* char2 : char?"]
        #[function(name = $ext_name, constant = true)]
        pub fn $name(rest: RestArgsIter<&char>) -> Result<StahlVal> {
            monotonic!(rest, $op)
        }
    };
}

impl_char_comparison!(
    char_less_than,
    "char<?",
    "less-than",
    |ch1: &_, ch2: &_| ch1 < ch2
);
impl_char_comparison!(
    char_less_than_equal_to,
    "char<=?",
    "less-than-or-equal",
    |ch1: &_, ch2: &_| ch1 <= ch2
);
impl_char_comparison!(
    char_greater_than,
    "char>?",
    "greater-than",
    |ch1: &_, ch2: &_| ch1 > ch2
);
impl_char_comparison!(
    char_greater_than_equal_to,
    "char>=?",
    "greater-than-or-equal",
    |ch1: &_, ch2: &_| ch1 >= ch2
);

/// Returns the Unicode codepoint of a given character.
///
/// (char->integer char?) -> integer?
#[function(name = "char->integer")]
pub fn char_to_integer(ch: char) -> u32 {
    ch as u32
}

/// Returns the character corresponding to a given Unicode codepoint.
///
/// (integer->char integer?) -> char?
#[function(name = "integer->char")]
pub fn integer_to_char(int: u32) -> Result<StahlVal> {
    let Some(ch) = char::from_u32(int) else {
        stop!(ConversionError => "integer {} is out of range for a character", int);
    };

    Ok(ch.into())
}

/// Encodes a string as UTF-8 into a bytevector.
///
/// (string->bytes string?) -> bytes?
///
/// # Examples
/// ```scheme
/// (string->bytes "Apple") ;; => (bytes 65 112 112 108 101)
/// ```
#[function(name = "string->bytes", alias = "string->utf8")]
pub fn string_to_bytes(value: &StahlString, mut rest: RestArgsIter<isize>) -> Result<StahlVal> {
    let start = rest.next().transpose()?;
    let end = rest.next().transpose()?;

    if rest.next().is_some() {
        stop!(ArityMismatch => "string->bytes expects up to 3 parameters");
    }

    let range = bounds(value.as_str(), start, end, "string->bytes")?;

    let bytes = value.as_bytes()[range].to_owned();

    Ok(StahlVal::ByteVector(StahlByteVector::new(bytes)))
}

/// Returns a vector containing the characters of a given string
///
/// (string->vector string?) -> vector?
///
/// # Examples
/// ```scheme
/// (string->vector "hello") ;; => '#(#\h #\e #\l #\l #\o)
/// ```
#[function(name = "string->vector")]
pub fn string_to_vector(value: &StahlString, mut rest: RestArgsIter<isize>) -> Result<StahlVal> {
    let start = rest.next().transpose()?;
    let end = rest.next().transpose()?;

    if rest.next().is_some() {
        stop!(ArityMismatch => "string->vector expects up to 3 parameters");
    }

    let range = bounds(value.as_str(), start, end, "string->vector")?;

    let chars: Vector<_> = value[range].chars().map(StahlVal::CharV).collect();

    Ok(StahlVal::VectorV(Gc::new(chars).into()))
}

fn bounds(
    s: &str,
    i: Option<isize>,
    j: Option<isize>,
    name: &str,
) -> Result<std::ops::Range<usize>> {
    use std::iter::once;

    let i = i.unwrap_or(0);

    if i < 0 {
        stop!(ContractViolation => "{}: bounds must be non-negative: left: {}", name, i);
    }

    let i = i as usize;

    if i > s.len() {
        stop!(Generic => "{}: index out of bounds: left bound: {}, string length: {}", name, i, s.len());
    }

    if let Some(j) = j {
        if j < 0 {
            stop!(ContractViolation => "{}: bounds must be non-negative: right: {}", name, j);
        }

        if i > (j as usize) {
            stop!(Generic => "{}: left bound must be less than or equal to the right bound: left: {}, right: {}", name, i, j);
        }
    }

    let j = j.map(|j| j as usize);

    let mut char_offsets = s
        .char_indices()
        .map(|(offset, _)| offset)
        .chain(once(s.len()));

    let Some(start) = char_offsets.nth(i) else {
        stop!(Generic => "{}: index out of bounds: left bound: {}", name, i);
    };

    let Some(j) = j else {
        return Ok(start..s.len());
    };

    let mut char_offsets = once(start).chain(char_offsets);

    let Some(end) = char_offsets.nth(j - i) else {
        stop!(Generic => "{}: index out of bounds: right bound: {}", name, j);
    };

    Ok(start..end)
}

/// Returns the upper case version of a character, if defined by Unicode,
/// or the same character otherwise.
#[function(name = "char-upcase")]
fn char_upcase(c: char) -> char {
    c.to_uppercase().next().unwrap()
}

/// Returns the lower case version of a character, if defined by Unicode,
/// or the same character otherwise.
#[function(name = "char-downcase")]
fn char_downcase(c: char) -> char {
    c.to_lowercase().next().unwrap()
}

/// Returns `#t` if the character is a whitespace character.
#[function(name = "char-whitespace?")]
fn char_is_whitespace(c: char) -> bool {
    c.is_whitespace()
}

/// Returns `#t` if the character is a decimal digit.
#[function(name = "char-digit?")]
fn char_is_digit(c: char) -> bool {
    c.is_digit(10)
}

/// Attemps to convert the character into a decimal digit,
/// and returns `#f` on failure.
#[function(name = "char->number")]
fn char_to_number(c: char) -> Option<u32> {
    c.to_digit(10)
}

/// Joins the given list of strings, with an optional separator.
///
/// (string-join strings [sep]) -> string?
///
/// * strings : (listof string?)
/// * sep : string? = ""
///
/// # Examples
/// ```scheme
/// (string-join '("a" "b" "c")) ;; => "abc"
/// (string-join '("one" "two" "three") ", ") ;; => "one, two, three"
/// ```
#[function(name = "string-join")]
fn string_join(
    strings: StahlList<StahlVal>,
    mut rest: RestArgsIter<'_, &StahlString>,
) -> Result<StahlVal> {
    let mut joined = String::new();

    let len = strings.len();

    let sep = rest.next().transpose()?;

    if rest.next().is_some() {
        todo!()
    }

    for (i, val) in strings.into_iter().enumerate() {
        let StahlVal::StringV(s) = val else {
            stop!(TypeMismatch => "string-join: expected a list of strings");
        };

        joined += s.as_str();

        if i + 1 < len {
            if let Some(sep) = sep.as_ref() {
                joined += sep.as_str();
            }
        }
    }

    Ok(joined.into())
}

/// Searches a string to check if it contains the second argument.
///
/// (string-contains? string? string?) -> bool?
///
/// # Examples
/// ```scheme
/// (string-contains? "hello" "lo") ;;=> #t
/// (string-contains? "hello" "world") ;;=> #f
/// ```
#[function(name = "string-contains?")]
fn string_contains(s: &StahlString, contained: &StahlString) -> bool {
    s.as_str().contains(contained.as_str())
}

#[cfg(test)]
mod string_operation_tests {
    use super::*;
    use crate::rerrs::ErrorKind;

    // TODO combine these 3 macros into one
    macro_rules! apply_tests_arity_too_many {
        ($(($name:expr, $symbol:ident, $func:expr)),* $(,)?) => {
            $(
                #[test]
                pub fn $symbol() {
                    let args = vec![
                        StahlVal::StringV("FOO".into()),
                        StahlVal::StringV("BAR".into()),
                    ];
                    let res = $func(&args);
                    let expected = ErrorKind::ArityMismatch;
                    assert_eq!(res.unwrap_err().kind(), expected);
                }
            )*
        };
    }

    macro_rules! apply_tests_arity_too_few {
        ($(($name:expr, $symbol:ident, $func:expr)),* $(,)?) => {
            $(
                #[test]
                pub fn $symbol() {
                    let args = vec![];
                    let res = $func(&args);
                    let expected = ErrorKind::ArityMismatch;
                    assert_eq!(res.unwrap_err().kind(), expected);
                }
            )*
        };
    }

    macro_rules! apply_tests_bad_arg {
        ($(($name:expr, $symbol:ident, $func:expr)),* $(,)?) => {
            $(
                #[test]
                pub fn $symbol() {
                    let args = vec![StahlVal::NumV(10.0)];
                    let res = $func(&args);
                    let expected = ErrorKind::TypeMismatch;
                    assert_eq!(res.unwrap_err().kind(), expected);
                }
            )*
        };
    }

    apply_tests_arity_too_many! {
        ("string-upcase", string_upper_arity_too_many, stahl_string_to_upper),
        ("string-lowercase", string_lower_arity_too_many, stahl_string_to_lower),
        ("trim", trim_arity_too_many, stahl_trim),
        ("trim-start", trim_start_arity_too_many, stahl_trim_start),
        ("trim-end", trim_end_arity_too_many, stahl_trim_end),
        ("split-whitespace", split_whitespace_arity_too_many, stahl_split_whitespace),
    }

    apply_tests_arity_too_few! {
        ("string-upcase", string_upper_arity_too_few, stahl_string_to_upper),
        ("string-lowercase", string_lower_arity_too_few, stahl_string_to_lower),
        ("trim", trim_arity_too_few, stahl_trim),
        ("trim-start", trim_start_arity_too_few, stahl_trim_start),
        ("trim-end", trim_end_arity_too_few, stahl_trim_end),
        ("string->list", string_to_list_arity_too_few, stahl_string_to_list),
        ("split-whitespace", split_whitespace_arity_too_few, stahl_split_whitespace)
    }

    apply_tests_bad_arg! {
        ("string-upcase", string_upper_arity_takes_string, stahl_string_to_upper),
        ("string-lowercase", string_lower_arity_takes_string, stahl_string_to_lower),
        ("trim", trim_arity_takes_string, stahl_trim),
        ("trim-start", trim_start_arity_takes_string, stahl_trim_start),
        ("trim-end", trim_end_arity_takes_string, stahl_trim_end),
        ("string->list", string_to_list_takes_string, stahl_string_to_list),
        ("string->bytes", string_to_bytes_takes_string, stahl_string_to_bytes),
        ("string->vector", string_to_vector_takes_string, stahl_string_to_vector),
        ("split-whitespace", split_whitespace_arity_takes_string, stahl_split_whitespace)
    }

    #[test]
    fn string_append_test_normal() {
        let args = vec![
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
        ];
        let res = stahl_string_append(&args);
        let expected = StahlVal::StringV("foobar".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn string_append_test_takes_string() {
        let args = vec![StahlVal::CharV('a'), StahlVal::CharV('b')];
        let res = stahl_string_append(&args);
        let expected = ErrorKind::TypeMismatch;
        assert_eq!(res.unwrap_err().kind(), expected);
    }

    #[test]
    fn string_to_upper_normal() {
        let args = vec![StahlVal::StringV("foobarbaz".into())];
        let res = stahl_string_to_upper(&args);
        let expected = StahlVal::StringV("FOOBARBAZ".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn string_to_upper_spaces() {
        let args = vec![StahlVal::StringV("foo bar baz qux".into())];
        let res = stahl_string_to_upper(&args);
        let expected = StahlVal::StringV("FOO BAR BAZ QUX".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn string_to_lower_normal() {
        let args = vec![StahlVal::StringV("FOOBARBAZ".into())];
        let res = stahl_string_to_lower(&args);
        let expected = StahlVal::StringV("foobarbaz".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn string_to_lower_spaces() {
        let args = vec![StahlVal::StringV("FOO BAR BAZ QUX".into())];
        let res = stahl_string_to_lower(&args);
        let expected = StahlVal::StringV("foo bar baz qux".into());
        assert_eq!(res.unwrap(), expected);
    }

    // TODO investigate this, assert_eq! fails without converting to string
    #[test]
    fn string_to_list_normal() {
        let args = vec![StahlVal::StringV("foo".into())];
        let res = stahl_string_to_list(&args);

        let expected = StahlVal::ListV(
            vec![
                StahlVal::CharV('f'),
                StahlVal::CharV('o'),
                StahlVal::CharV('o'),
            ]
            .into(),
        );

        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn string_to_list_empty() {
        let args = vec![StahlVal::StringV("".into())];
        let res = stahl_string_to_list(&args);
        let expected = StahlVal::ListV(List::new());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_normal_no_changes() {
        let args = vec![StahlVal::StringV("foo".into())];
        let res = stahl_trim(&args);
        let expected = StahlVal::StringV("foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_normal_trims_both_sides() {
        let args = vec![StahlVal::StringV("      foo  ".into())];
        let res = stahl_trim(&args);
        let expected = StahlVal::StringV("foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_start_no_changes() {
        let args = vec![StahlVal::StringV("foo".into())];
        let res = stahl_trim_start(&args);
        let expected = StahlVal::StringV("foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_end_no_changes() {
        let args = vec![StahlVal::StringV("foo".into())];
        let res = stahl_trim_end(&args);
        let expected = StahlVal::StringV("foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_start_normal_trims_left_side() {
        let args = vec![StahlVal::StringV("      foo  ".into())];
        let res = stahl_trim_start(&args);
        let expected = StahlVal::StringV("foo  ".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn trim_end_normal_trims_right_side() {
        let args = vec![StahlVal::StringV("      foo  ".into())];
        let res = stahl_trim_end(&args);
        let expected = StahlVal::StringV("      foo".into());
        assert_eq!(res.unwrap(), expected);
    }

    // TODO investigate this one
    #[test]
    fn split_whitespace_no_whitespace() {
        let args = vec![StahlVal::StringV("foo".into())];
        let res = stahl_split_whitespace(&args);

        let expected = StahlVal::ListV(vec![StahlVal::StringV("foo".into())].into());

        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn split_whitespace_some_whitespace() {
        let args = vec![StahlVal::StringV("foo bar baz".into())];
        let res = stahl_split_whitespace(&args);

        let expected = StahlVal::ListV(
            vec![
                StahlVal::StringV("foo".into()),
                StahlVal::StringV("bar".into()),
                StahlVal::StringV("baz".into()),
            ]
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }
}
