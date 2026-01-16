use crate::rvals::StahlString;
use crate::values::lists::List;
use crate::values::HashMap;
use crate::{
    gc::Gc,
    rerrs::StahlErr,
    rvals::{FromStahlVal, IntoStahlVal, Result, StahlVal},
};
use serde_json::{Map, Number, Value};
use std::convert::{TryFrom, TryInto};
use stahl_derive::function;

/// Deserializes a JSON string into a Stahl value.
///
/// (string->jsexpr json) -> any/c
///
/// * json : string?
///
/// # Examples
/// ```scheme
/// (string->jsexpr "{\"foo\": [3]}") ;; => '#hash((foo . (3)))
/// ```
#[function(name = "string->jsexpr")]
pub fn string_to_jsexpr(value: &StahlString) -> Result<StahlVal> {
    // let unescaped = unescape(&value);
    let unescaped = value;
    let res: std::result::Result<Value, _> = serde_json::from_str(unescaped.as_str());

    match res {
        Ok(res) => res.try_into(),
        Err(e) => stop!(Generic => format!("string->jsexpr failed: {e}")),
    }
}

/// Serializes a Stahl value into a string.
///
/// (value->jsexpr-string any/c) -> string?
///
/// # Examples
/// ```scheme
/// (value->jsexpr-string `(,(hash "foo" #t))) ;; => "[{\"foo\":true}]"
/// ```
#[function(name = "value->jsexpr-string")]
pub fn serialize_val_to_string(value: StahlVal) -> Result<StahlVal> {
    let serde_value: Value = value.try_into()?;
    let serialized_value = serde_value.to_string();
    Ok(StahlVal::StringV(serialized_value.into()))
}

// required to parse each string
#[allow(unused)]
fn unescape(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        result.push(if ch != '\\' {
            ch
        } else {
            match chars.next() {
                // Some('u') => {
                //     let value = chars
                //         .by_ref()
                //         .take(4)
                //         .fold(0, |acc, c| acc * 16 + c.to_digit(16).unwrap());
                //     char::from_u32(value).unwrap()
                // }
                Some('b') => '\x08',
                Some('f') => '\x0c',
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                Some(ch) => ch,
                _ => panic!("Malformed escape"),
            }
        })
    }
    result
}

impl TryFrom<Map<String, Value>> for StahlVal {
    type Error = StahlErr;
    fn try_from(map: Map<String, Value>) -> std::result::Result<Self, Self::Error> {
        let mut hm = HashMap::new();
        for (key, value) in map {
            hm.insert(StahlVal::SymbolV(key.into()), value.try_into()?);
        }
        Ok(StahlVal::HashMapV(Gc::new(hm).into()))
    }
}

impl TryFrom<Value> for StahlVal {
    type Error = StahlErr;
    fn try_from(val: Value) -> std::result::Result<Self, Self::Error> {
        match val {
            Value::Null => Ok(StahlVal::Void),
            Value::Bool(t) => Ok(StahlVal::BoolV(t)),
            Value::Number(n) => <StahlVal>::try_from(n),
            Value::String(s) => Ok(StahlVal::StringV(s.into())),
            Value::Array(v) => Ok(StahlVal::ListV(
                v.into_iter()
                    .map(<StahlVal>::try_from)
                    .collect::<Result<List<StahlVal>>>()?,
            )),
            Value::Object(m) => m.try_into(),
        }
    }
}

// TODO
impl TryFrom<Number> for StahlVal {
    type Error = StahlErr;
    fn try_from(n: Number) -> std::result::Result<Self, Self::Error> {
        let result = n.as_f64().unwrap();
        Ok(StahlVal::NumV(result))
    }
}

impl IntoStahlVal for Value {
    fn into_stahlval(self) -> Result<StahlVal> {
        self.try_into()
    }
}

impl FromStahlVal for Value {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        val.clone().try_into()
    }
}

// Attempt to serialize to json?
// It would be better to straight implement the deserialize method
// Honestly... this is not great
impl TryFrom<StahlVal> for Value {
    type Error = StahlErr;
    fn try_from(val: StahlVal) -> std::result::Result<Self, Self::Error> {
        match val {
            StahlVal::BoolV(b) => Ok(Value::Bool(b)),
            StahlVal::NumV(n) => Ok(Value::Number(Number::from_f64(n).unwrap())),
            StahlVal::IntV(n) => Ok(Value::Number(Number::from(n))),
            StahlVal::CharV(c) => Ok(Value::String(c.to_string())),
            // StahlVal::Pair(_) => Ok(Value::Array(
            //     StahlVal::iter(val)
            //         .map(|x| x.try_into())
            //         .collect::<Result<Vec<_>>>()?,
            // )),
            StahlVal::ListV(l) => Ok(Value::Array(
                l.into_iter()
                    .map(|x| x.try_into())
                    .collect::<Result<Vec<_>>>()?,
            )),
            StahlVal::VectorV(v) => Ok(Value::Array(
                v.iter()
                    .map(|x| x.clone().try_into())
                    .collect::<Result<Vec<_>>>()?,
            )),
            StahlVal::Void => stop!(Generic => "void not serializable"),
            StahlVal::StringV(s) => Ok(Value::String(s.to_string())),
            StahlVal::FuncV(_) => stop!(Generic => "function not serializable"),
            // StahlVal::LambdaV(_) => stop!(Generic => "function not serializable"),
            // StahlVal::MacroV(_) => stop!(Generic => "macro not serializable"),
            StahlVal::SymbolV(s) => Ok(Value::String(s.to_string())),
            StahlVal::Custom(_) => stop!(Generic => "generic struct not serializable"),
            StahlVal::HashMapV(hm) => {
                let mut map: Map<String, Value> = Map::new();
                for (key, value) in hm.iter() {
                    map.insert(key.clone().try_into()?, value.clone().try_into()?);
                }
                Ok(Value::Object(map))
            }
            StahlVal::HashSetV(hs) => Ok(Value::Array(
                hs.iter()
                    .map(|x| x.clone().try_into())
                    .collect::<Result<Vec<_>>>()?,
            )),
            // StahlVal::StructV(_) => stop!(Generic => "built in struct not serializable yet"),
            _ => stop!(Generic => "type not serializable"),
            // StahlVal::StructClosureV(_, _) => {}
            // StahlVal::PortV(_) => {}
            // StahlVal::Closure(_) => {}
            // StahlVal::IterV(_) => {}
            // StahlVal::FutureFunc(_) => {}
            // StahlVal::FutureV(_) => {}
            // StahlVal::StreamV(_) => {}
        }

        // unimplemented!()
    }
}

#[cfg(test)]
mod json_tests {
    use super::*;

    use crate::rvals::StahlVal::*;

    #[cfg(not(feature = "sync"))]
    use im_rc::hashmap;

    #[cfg(feature = "sync")]
    use im::hashmap;

    #[test]
    fn test_string_to_jsexpr() {
        let json_expr = r#"{"a":"applesauce","b":"bananas"}"#;

        let result = string_to_jsexpr(&json_expr.into());

        let expected = StahlVal::HashMapV(
            Gc::new(hashmap! {
                SymbolV("a".into()) => StringV("applesauce".into()),
                SymbolV("b".into()) => StringV("bananas".into())
            })
            .into(),
        );

        assert_eq!(result.unwrap(), expected);
    }
}
