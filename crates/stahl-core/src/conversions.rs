use crate::values::lists::List;

use crate::{
    gc::Gc,
    rerrs::ErrorKind,
    rvals::{AsRefStahlValFromUnsized, FromStahlVal, IntoStahlVal, Result},
    StahlErr, StahlVal,
};
use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
};

use crate::values::HashMap as ImmutableHashMap;
use crate::values::HashSet as ImmutableHashSet;

#[cfg(feature = "anyhow")]
mod anyhow_conversion {
    use crate::{rvals::IntoStahlVal, StahlVal};

    impl IntoStahlVal for anyhow::Error {
        fn into_stahlval(self) -> crate::rvals::Result<crate::StahlVal> {
            Ok(StahlVal::StringV(format!("{:#?}", self).into()))
        }
    }
}

impl IntoStahlVal for StahlVal {
    fn into_stahlval(self) -> Result<StahlVal> {
        Ok(self)
    }
}

// impl IntoStahlVal for Result<StahlVal> {
//     fn into_stahlval(self) -> Result<StahlVal> {
//         self
//     }
// }

impl FromStahlVal for Gc<ImmutableHashMap<StahlVal, StahlVal>> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Ok(hm.0.clone())
        } else {
            stop!(TypeMismatch => "Unable to convert Stahlval to HashMap, found: {}", val);
        }
    }
}

// impl IntoStahlVal for str {
//     fn into_stahlval(self) -> Result<StahlVal> {
//         Ok(StahlVal::StringV(self.to_string().into()))
//     }
// }

// impl<T: IntoStahlVal + Clone> IntoStahlVal for Cow<'_, T> {
//     fn into_stahlval(self) -> Result<StahlVal> {
//         match self {
//             Cow::Borrowed(b) => b.clone().into_stahlval(),
//             Cow::Owned(o) => o.into_stahlval(),
//         }
//     }
// }

impl IntoStahlVal for Cow<'_, str> {
    fn into_stahlval(self) -> Result<StahlVal> {
        match self {
            Cow::Borrowed(b) => b.into_stahlval(),
            Cow::Owned(o) => o.into_stahlval(),
        }
    }
}

impl FromStahlVal for Cow<'_, str> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::StringV(s) = val {
            Ok(Cow::Owned(s.to_string()))
        } else {
            stop!(TypeMismatch => "expected string, found {:?}", val)
        }
    }
}

impl<T: IntoStahlVal + Clone> IntoStahlVal for &[T] {
    fn into_stahlval(self) -> Result<StahlVal> {
        self.iter()
            .map(|x| x.clone().into_stahlval())
            .collect::<Result<List<_>>>()
            .map(StahlVal::ListV)
    }
}

// TODO: @Matt - figure out how to get this to actually return the right value. At the moment,
// it seems I can't get this to return the correct value
impl<T: FromStahlVal + Clone> AsRefStahlValFromUnsized<T> for T {
    type Output = Vec<T>;

    fn as_ref_from_unsized(val: &StahlVal) -> Result<Self::Output> {
        if let StahlVal::ListV(v) = val {
            v.iter()
                .map(|x| T::from_stahlval(x))
                .collect::<Result<Vec<_>>>()
        } else {
            stop!(TypeMismatch => "expected list, found: {:?}", val);
        }
    }
}

impl<A: IntoStahlVal, B: IntoStahlVal> IntoStahlVal for (A, B) {
    fn into_stahlval(self) -> Result<StahlVal> {
        Ok(StahlVal::ListV(
            vec![self.0.into_stahlval()?, self.1.into_stahlval()?].into(),
        ))
    }
}

// Vectors should translate into vectors in rust
impl<T: IntoStahlVal> IntoStahlVal for Vec<T> {
    fn into_stahlval(self) -> Result<StahlVal> {
        let vec_vals: Result<Vec<StahlVal>> = self.into_iter().map(|x| x.into_stahlval()).collect();

        match vec_vals {
            Ok(l) => Ok(StahlVal::ListV(l.into())),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Could not convert vector of values to StahlVal list".to_string(),
            )),
        }
    }
}

impl<T: FromStahlVal> FromStahlVal for Vec<T> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        match val {
            StahlVal::ListV(l) => {
                let result_vec_vals: Result<Self> = l
                    .into_iter()
                    .map(|x| FromStahlVal::from_stahlval(x))
                    .collect();

                match result_vec_vals {
                    Ok(x) => Ok(x),
                    _ => Err(StahlErr::new(
                        ErrorKind::ConversionError,
                        "Could not convert StahlVal list to Vector of values".to_string(),
                    )),
                }
            }
            // StahlVal::Pair(_) => {
            //     let result_vec_vals: Result<Self> = StahlVal::iter(val.clone())
            //         .into_iter()
            //         .map(FromStahlVal::from_stahlval)
            //         .collect();

            //     match result_vec_vals {
            //         Ok(x) => Ok(x),
            //         _ => Err(StahlErr::new(
            //             ErrorKind::ConversionError,
            //             "Could not convert StahlVal list to Vector of values".to_string(),
            //         )),
            //     }
            // }
            StahlVal::VectorV(v) => {
                let result_vec_vals: Result<Self> =
                    v.iter().map(FromStahlVal::from_stahlval).collect();
                match result_vec_vals {
                    Ok(x) => Ok(x),
                    _ => Err(StahlErr::new(
                        ErrorKind::ConversionError,
                        "Could not convert StahlVal list to Vector of values".to_string(),
                    )),
                }
            } // TODO
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Could not convert StahlVal list to Vector of values".to_string(),
            )),
        }
    }
}

impl<T: FromStahlVal> FromStahlVal for Box<[T]> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        match val {
            StahlVal::ListV(l) => {
                let result_vec_vals: Result<Self> = l
                    .into_iter()
                    .map(|x| FromStahlVal::from_stahlval(x))
                    .collect();

                match result_vec_vals {
                    Ok(x) => Ok(x),
                    _ => Err(StahlErr::new(
                        ErrorKind::ConversionError,
                        "Could not convert StahlVal list to Vector of values".to_string(),
                    )),
                }
            }
            StahlVal::VectorV(v) => {
                let result_vec_vals: Result<Self> =
                    v.iter().map(FromStahlVal::from_stahlval).collect();
                match result_vec_vals {
                    Ok(x) => Ok(x),
                    _ => Err(StahlErr::new(
                        ErrorKind::ConversionError,
                        "Could not convert StahlVal list to Vector of values".to_string(),
                    )),
                }
            } // TODO
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Could not convert StahlVal list to Vector of values".to_string(),
            )),
        }
    }
}

impl FromStahlVal for Box<str> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::StringV(s) = val {
            Ok(s.as_str().into())
        } else {
            stop!(TypeMismatch => "Unable to convert {} into Box<str>", val);
        }
    }
}

// HashMap
impl<K: IntoStahlVal, V: IntoStahlVal> IntoStahlVal for HashMap<K, V> {
    fn into_stahlval(mut self) -> Result<StahlVal> {
        let mut hm = ImmutableHashMap::new();
        for (key, val) in self.drain() {
            hm.insert(key.into_stahlval()?, val.into_stahlval()?);
        }
        Ok(StahlVal::HashMapV(Gc::new(hm).into()))
    }
}

impl<K: FromStahlVal + Eq + std::hash::Hash, V: FromStahlVal> FromStahlVal for HashMap<K, V> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        // todo!()
        if let StahlVal::HashMapV(hm) = val {
            let mut h = HashMap::new();
            for (key, value) in hm.0.unwrap().into_iter() {
                h.insert(K::from_stahlval(&key)?, V::from_stahlval(&value)?);
            }
            Ok(h)
        } else {
            Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Could not convert StahlVal to HashMap".to_string(),
            ))
        }
    }
}

// BTreeMap
// impl<K: IntoStahlVal, V: IntoStahlVal> IntoStahlVal for BTreeMap<K, V> {
//     fn into_stahlval(self) -> Result<StahlVal> {
//         let mut hm = im_rc::HashMap::new();
//         for (key, val) in self.drain() {
//             hm.insert(key.into_stahlval()?, val.into_stahlval()?);
//         }
//         Ok(StahlVal::HashMapV(Gc::new(hm)))
//     }
// }

// impl<K: FromStahlVal, V: FromStahlVal> FromStahlVal for BTreeMap<K, V> {
//     fn from_stahlval(val: StahlVal) -> Result<Self> {
//         todo!()
//     }
// }

impl<A: FromStahlVal, B: FromStahlVal> FromStahlVal for (A, B) {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::ListV(l) = val {
            if l.len() != 2 {
                return Err(StahlErr::new(ErrorKind::ConversionError, format!("Could not convert stahlval to (A, B): {:?} - list was not length of 2, found length: {}", val, l.len())));
            }

            Ok((
                A::from_stahlval(l.get(0).unwrap())?,
                B::from_stahlval(l.get(1).unwrap())?,
            ))
        } else {
            Err(StahlErr::new(
                ErrorKind::ConversionError,
                format!("Could not convert StahlVal to (A, B): {:?}", val),
            ))
        }
    }
}

// HashSet
impl<K: IntoStahlVal> IntoStahlVal for HashSet<K> {
    fn into_stahlval(mut self) -> Result<StahlVal> {
        let mut hs = ImmutableHashSet::new();
        for value in self.drain() {
            hs.insert(value.into_stahlval()?);
        }
        Ok(StahlVal::HashSetV(Gc::new(hs).into()))
    }
}

impl<K: FromStahlVal + Eq + std::hash::Hash> FromStahlVal for HashSet<K> {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::HashSetV(hs) = val {
            let mut h = HashSet::new();
            for k in hs.0.unwrap().into_iter() {
                h.insert(K::from_stahlval(&k)?);
            }
            Ok(h)
        } else {
            Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Could not convert StahlVal to HashSet".to_string(),
            ))
        }
    }
}

// BTreeSet
// impl<K: IntoStahlVal> IntoStahlVal for BTreeSet<K> {
//     fn into_stahlval(self) -> Result<StahlVal> {
//         todo!()
//     }
// }

// impl<K: FromStahlVal> FromStahlVal for BTreeSet<K> {
//     fn from_stahlval(val: StahlVal) -> Result<Self> {
//         todo!()
//     }
// }

#[cfg(test)]
mod conversion_tests {

    use super::*;

    #[cfg(not(feature = "sync"))]
    use im_rc::vector;

    #[cfg(not(feature = "sync"))]
    use im_rc::hashmap;

    #[cfg(not(feature = "sync"))]
    use im_rc::hashset;

    #[cfg(feature = "sync")]
    use im::vector;

    #[cfg(feature = "sync")]
    use im::hashmap;

    #[cfg(feature = "sync")]
    use im::hashset;

    #[test]
    fn vec_into_list() {
        let input_vec = vec![1, 2];
        // let expected = StahlVal::Pair(Gc::new(ConsCell::new(
        //     StahlVal::IntV(1),
        //     Some(Gc::new(ConsCell::new(StahlVal::IntV(2), None))),
        // )));

        let expected = StahlVal::ListV(vec![StahlVal::IntV(1), StahlVal::IntV(2)].into());

        assert_eq!(input_vec.into_stahlval().unwrap(), expected)
    }

    #[test]
    fn vec_from_list() {
        let input_list = StahlVal::ListV(vec![StahlVal::IntV(1), StahlVal::IntV(2)].into());

        let expected = vec![1, 2];
        let result = <Vec<i32>>::from_stahlval(&input_list).unwrap();

        assert_eq!(result, expected)
    }

    #[test]
    fn vec_from_vector() {
        let input_vector = vector![StahlVal::IntV(1), StahlVal::IntV(2)].into();

        let expected = vec![1, 2];
        let result = <Vec<i32>>::from_stahlval(&input_vector).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn vec_from_stahlval_error() {
        let input = StahlVal::IntV(2);
        assert!(<Vec<i32>>::from_stahlval(&input).is_err());
    }

    #[test]
    fn hashmap_into_stahlval() {
        let mut input = HashMap::new();
        input.insert("foo".to_string(), "bar".to_string());
        input.insert("foo2".to_string(), "bar2".to_string());

        let expected = StahlVal::HashMapV(
            Gc::new(hashmap! {
                StahlVal::StringV("foo".into()) => StahlVal::StringV("bar".into()),
                StahlVal::StringV("foo2".into()) => StahlVal::StringV("bar2".into())
            })
            .into(),
        );

        assert_eq!(input.into_stahlval().unwrap(), expected);
    }

    #[test]
    fn hashmap_from_stahlval_hashmap() {
        let input = StahlVal::HashMapV(
            Gc::new(hashmap! {
                StahlVal::StringV("foo".into()) => StahlVal::StringV("bar".into()),
                StahlVal::StringV("foo2".into()) => StahlVal::StringV("bar2".into())
            })
            .into(),
        );

        let mut expected = HashMap::new();
        expected.insert("foo".to_string(), "bar".to_string());
        expected.insert("foo2".to_string(), "bar2".to_string());

        assert_eq!(
            <HashMap<String, String>>::from_stahlval(&input).unwrap(),
            expected
        );
    }

    #[test]
    fn hashset_into_stahlval() {
        let mut input = HashSet::new();
        input.insert("foo".to_string());
        input.insert("bar".to_string());

        let expected = StahlVal::HashSetV(
            Gc::new(hashset! {
                StahlVal::StringV("foo".into()),
                StahlVal::StringV("bar".into())
            })
            .into(),
        );

        assert_eq!(input.into_stahlval().unwrap(), expected);
    }

    #[test]
    fn hashset_from_stahlval_hashset() {
        let input = StahlVal::HashSetV(
            Gc::new(hashset! {
                StahlVal::StringV("foo".into()),
                StahlVal::StringV("bar".into())
            })
            .into(),
        );

        let mut expected = HashSet::new();
        expected.insert("foo".to_string());
        expected.insert("bar".to_string());

        assert_eq!(<HashSet<String>>::from_stahlval(&input).unwrap(), expected);
    }
}
