use crate::rvals::StahlHashMap;
use crate::stop;
use crate::values::HashMap;
use crate::{core::utils::declare_const_ref_functions, gc::Gc};
use crate::{
    rvals::{Result, StahlVal},
    stahl_vm::builtin::BuiltInModule,
};

use crate::primitives::VectorOperations;

use stahl_derive::function;

declare_const_ref_functions!(
    HM_CONSTRUCT => hm_construct,
    HM_GET => stahl_hash_ref,
    // HM_EMPTY => hm_empty,
);

pub const HM_INSERT: StahlVal = StahlVal::MutFunc(stahl_hash_insert);

pub(crate) fn hashmap_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/hash".to_string());
    module
        .register_native_fn_definition(HM_CONSTRUCT_DEFINITION)
        .register_value("%keyword-hash", StahlVal::FuncV(hm_construct_keywords))
        .register_native_fn_definition(HASH_INSERT_DEFINITION)
        .register_native_fn_definition(HASH_REF_DEFINITION)
        .register_value("hash-get", StahlVal::FuncV(stahl_hash_ref))
        .register_native_fn_definition(HASH_TRY_GET_DEFINITION)
        .register_native_fn_definition(HASH_LENGTH_DEFINITION)
        .register_native_fn_definition(HASH_CONTAINS_DEFINITION)
        .register_native_fn_definition(KEYS_TO_LIST_DEFINITION)
        .register_native_fn_definition(KEYS_TO_VECTOR_DEFINITION)
        .register_native_fn_definition(VALUES_TO_LIST_DEFINITION)
        .register_native_fn_definition(VALUES_TO_VECTOR_DEFINITION)
        .register_native_fn_definition(CLEAR_DEFINITION)
        .register_native_fn_definition(HM_EMPTY_DEFINITION)
        .register_native_fn_definition(HM_UNION_DEFINITION)
        .register_native_fn_definition(HASH_REMOVE_DEFINITION);
    module
}

/// Creates an immutable hash table with each given `key` mapped to the following `val`.
/// Each key must have a val, so the total number of arguments must be even.
///
///
/// (hash key val ...) -> hash?
///
/// key : hashable?
/// val : any/c
///
/// Note: the keys must be hashable.
///
/// # Examples
/// ```scheme
/// > (hash 'a 10 'b 20)",
///   r#"=> #<hashmap {
///        'a: 10,
///        'b: 20,
///    }>"#,
/// ```
#[stahl_derive::native(name = "hash", arity = "AtLeast(0)")]
pub fn hm_construct(args: &[StahlVal]) -> Result<StahlVal> {
    let mut hm = HashMap::new();

    let mut arg_iter = args.iter().cloned();

    loop {
        match (arg_iter.next(), arg_iter.next()) {
            (Some(key), Some(value)) => {
                if key.is_hashable() {
                    hm.insert(key, value);
                } else {
                    stop!(TypeMismatch => "hash key not hashable!");
                }
            }
            (None, None) => break,
            _ => {
                stop!(ArityMismatch => "hash map must have a value for every key!");
            }
        }
    }

    Ok(StahlVal::HashMapV(Gc::new(hm).into()))
}

pub fn hm_construct_keywords(args: &[StahlVal]) -> Result<StahlVal> {
    let mut hm = HashMap::new();

    let mut arg_iter = args.iter().cloned();

    loop {
        match (arg_iter.next(), arg_iter.next()) {
            (Some(key), Some(value)) => {
                if key.is_hashable() {
                    hm.insert(key, value);
                } else {
                    stop!(TypeMismatch => "hash key not hashable!");
                }
            }
            (None, None) => break,
            _ => {
                stop!(ArityMismatch => "Missing keyword argument!");
            }
        }
    }

    Ok(StahlVal::HashMapV(Gc::new(hm).into()))
}

/// Returns a new hashmap with the given key removed. Performs a functional
/// update, so the old hash map is still available with the original key value pair.
///
/// (hash-remove map key) -> hash?
///
/// * map : hash?
/// * key : any/c
///
/// # Examples
/// ```scheme
/// > (hash-remove (hash 'a 10 'b 20) 'a)
///
/// => '#hash(('b . 20))
/// ```
#[function(name = "hash-remove")]
pub fn hash_remove(map: &mut StahlVal, key: StahlVal) -> Result<StahlVal> {
    if key.is_hashable() {
        if let StahlVal::HashMapV(StahlHashMap(ref mut m)) = map {
            match Gc::get_mut(m) {
                Some(m) => {
                    m.remove(&key);
                    Ok(std::mem::replace(map, StahlVal::Void))
                }
                None => {
                    let mut m = m.unwrap();
                    m.remove(&key);

                    Ok(StahlVal::HashMapV(Gc::new(m).into()))
                }
            }
        } else {
            stop!(TypeMismatch => "hash-insert expects a hash map, found: {:?}", map);
        }
    } else {
        stop!(TypeMismatch => "hash key not hashable: {:?}", key)
    }
}

/// Returns a new hashmap with the additional key value pair added. Performs a functional update,
/// so the old hash map is still accessible.
///
/// (hash-insert map key val) -> hash?
///
/// * map : hash?
/// * key : any/c
/// * val : any/c
///
/// # Examples
/// ```scheme
/// > (hash-insert (hash 'a 10 'b 20) 'c 30)
///
/// => #<hashmap {
///         'a: 10,
///         'b: 20,
///         'c: 30
///     }>
/// ```
#[function(name = "hash-insert")]
pub fn hash_insert(map: &mut StahlVal, key: StahlVal, value: StahlVal) -> Result<StahlVal> {
    if key.is_hashable() {
        if let StahlVal::HashMapV(StahlHashMap(ref mut m)) = map {
            match Gc::get_mut(m) {
                Some(m) => {
                    m.insert(key, value);
                    Ok(std::mem::replace(map, StahlVal::Void))
                }
                None => Ok(StahlVal::HashMapV(Gc::new(m.update(key, value)).into())),
            }
        } else {
            stop!(TypeMismatch => "hash-insert expects a hash map, found: {:?}", map);
        }
    } else {
        stop!(TypeMismatch => "hash key not hashable: {:?}", key)
    }
}

/// Gets the `key` from the given `map`. Raises an error if the key does not exist. `hash-get` is an alias for this.
///
/// (hash-ref map key) -> any/c
///
/// * map : hash?
/// * key : any/c
///
/// # Examples
/// ```scheme
/// > (hash-ref (hash 'a 10 'b 20) 'b) ;; => 20
/// ```
#[function(name = "hash-ref")]
pub fn hash_ref(map: &Gc<HashMap<StahlVal, StahlVal>>, key: &StahlVal) -> Result<StahlVal> {
    if key.is_hashable() {
        match map.get(key) {
            Some(value) => Ok(value.clone()),
            None => stop!(Generic => "key not found in hash map: {} - map: {:?}", key, map),
        }
    } else {
        stop!(TypeMismatch => "key not hashable: {}", key)
    }
}

/// Gets the `key` from the given `map`. Returns #false if the key does not exist.
///
/// (hash-try-get map key) -> (or any/c #false)
///
/// * map : hash?
/// * key : any/c
///
/// # Examples
///
/// ```scheme
/// > (hash-try-get (hash 'a 10 'b 20) 'b) ;; => 20
/// > (hash-try-get (hash 'a 10 'b 20) 'does-not-exist) ;; => #false
/// ```
#[function(name = "hash-try-get")]
pub fn hash_try_get(map: &Gc<HashMap<StahlVal, StahlVal>>, key: &StahlVal) -> StahlVal {
    match map.get(key) {
        Some(v) => v.clone(),
        None => StahlVal::BoolV(false),
    }
}

/// Returns the number of key value pairs in the map
///
/// (hash-length map) -> (and positive? int?)
///
/// * map : hash?
///
/// # Examples
///
/// ```scheme
/// > (hash-length (hash 'a 10 'b 20)) ;; => 2
/// ```
#[function(name = "hash-length")]
pub fn hash_length(map: &Gc<HashMap<StahlVal, StahlVal>>) -> usize {
    map.len()
}

/// Checks whether the given map contains the given key. Key must be hashable.
///
/// (hash-contains? map key) -> bool?
///
/// * map : hash?
/// * key : hashable?
///
/// # Example
///
/// ```scheme
/// > (hash-contains? (hash 'a 10 'b 20) 'a) ;; => #true
/// > (hash-contains? (hash 'a 10 'b 20) 'not-there) ;; => #false
/// ```
#[function(name = "hash-contains?")]
pub fn hash_contains(map: &Gc<HashMap<StahlVal, StahlVal>>, key: &StahlVal) -> Result<StahlVal> {
    if key.is_hashable() {
        Ok(StahlVal::BoolV(map.contains_key(key)))
    } else {
        stop!(TypeMismatch => "hash key not hashable!");
    }
}

/// Returns the keys of the given hash map as a list.
///
/// ```scheme
/// (hash-keys->list map) -> (listof hashable?)
/// ```
///
/// * map : hash?
///
/// # Examples
///
/// ```scheme
/// > (hash-keys->list? (hash 'a 'b 20)) ;; => '(a b)
/// ```
#[function(name = "hash-keys->list")]
pub fn keys_to_list(hashmap: &Gc<HashMap<StahlVal, StahlVal>>) -> Result<StahlVal> {
    Ok(StahlVal::ListV(hashmap.keys().cloned().collect()))
}

/// Returns the values of the given hash map as a list
///
/// (hash-values->list? map) -> (listof any/c)?
///
/// map: hash?
///
/// # Examples
/// ```scheme
/// > (hash-values->list? (hash 'a 10 'b 20)),
///   => '(10 20)",
/// ```
#[stahl_derive::function(name = "hash-values->list")]
pub fn values_to_list(hashmap: &Gc<HashMap<StahlVal, StahlVal>>) -> Result<StahlVal> {
    Ok(StahlVal::ListV(hashmap.values().cloned().collect()))
}

/// Returns the keys of the given hash map as an immutable vector
///
/// (hash-keys->vector map) -> (vectorof any/c)?
///
/// map: hash?
///
/// # Examples
/// ```scheme
/// > (hash-keys->vector (hash 'a 10 'b 20)),
///   => ['a 'b]",
/// ```
#[stahl_derive::function(name = "hash-keys->vector")]
pub fn keys_to_vector(hashmap: &Gc<HashMap<StahlVal, StahlVal>>) -> Result<StahlVal> {
    VectorOperations::vec_construct_iter_normal(hashmap.keys().cloned())
}

/// Returns the values of the given hash map as an immutable vector
///
/// (hash-values->vector map) -> (vectorof any/c)?
///
/// map: hash?
///
/// # Examples
/// ```scheme
/// > (hash-keys->vector (hash 'a 10 'b 20)),
///   => [10 10]",
/// ```
#[stahl_derive::function(name = "hash-values->vector")]
pub fn values_to_vector(hashmap: &Gc<HashMap<StahlVal, StahlVal>>) -> Result<StahlVal> {
    VectorOperations::vec_construct_iter_normal(hashmap.values().cloned())
}

/// Clears the entries out of the existing hashmap.
/// Will attempt to reuse the existing memory if there are no other references
/// to the hashmap.
///
/// (hash-clear h) -> hash?
///
/// h: hash?
///
/// # Examples
/// ```scheme
/// > (hash-clear (hash 'a 10 'b 20))
/// => '#hash()
/// ```
#[stahl_derive::function(name = "hash-clear")]
pub fn clear(hashmap: &mut StahlVal) -> Result<StahlVal> {
    if let StahlVal::HashMapV(StahlHashMap(ref mut m)) = hashmap {
        match Gc::get_mut(m) {
            Some(m) => {
                // m.insert(key, value);
                m.clear();
                Ok(std::mem::replace(hashmap, StahlVal::Void))
            }
            None => Ok(StahlVal::HashMapV(Gc::new(HashMap::new()).into())),
        }
    } else {
        stop!(TypeMismatch => "hash-clear expected a hashmap, found: {:?}", hashmap);
    }
}

/// Checks whether the hash map is empty
///
/// (hash-empty? m) -> bool?
///
/// m: hash?
///
/// # Examples
/// ```scheme
/// > (hash-empty? (hash 'a 10)) ;; => #f
/// > (hash-emptY? (hash)) ;; => #true
/// ```
#[stahl_derive::function(name = "hash-empty?")]
pub fn hm_empty(hm: &Gc<HashMap<StahlVal, StahlVal>>) -> Result<StahlVal> {
    Ok(StahlVal::BoolV(hm.is_empty()))
}

/// Constructs the union of two hashmaps, keeping the values
/// in the left map when the keys exist in both maps.
///
/// Will reuse memory where possible.
///
/// (hash-union l r) -> hash?
///
/// # Examples
/// ```scheme
/// > (hash-union (hash 'a 10) (hash 'b 20)) ;; => '#hash((a . 10) (b . 20))
/// ```
#[stahl_derive::function(name = "hash-union")]
pub fn hm_union(mut hml: &mut StahlVal, mut hmr: &mut StahlVal) -> Result<StahlVal> {
    match (&mut hml, &mut hmr) {
        (
            StahlVal::HashMapV(StahlHashMap(ref mut l)),
            StahlVal::HashMapV(StahlHashMap(ref mut r)),
        ) => match (Gc::get_mut(l), Gc::get_mut(r)) {
            (None, None) => {
                let hml = l.unwrap();
                let hmr = r.unwrap();
                Ok(StahlVal::HashMapV(Gc::new(hml.union(hmr)).into()))
            }
            (None, Some(r_map)) => {
                let right_side_value = std::mem::take(r_map);

                *r_map = l.unwrap().union(right_side_value);

                return Ok(std::mem::replace(hmr, StahlVal::Void));
            }
            (Some(l_map), None) => {
                let left_side_value = std::mem::take(l_map);

                *l_map = left_side_value.union(r.unwrap());

                return Ok(std::mem::replace(hml, StahlVal::Void));
            }
            (Some(l_map), Some(r_map)) => {
                let left_side_value = std::mem::take(l_map);
                let right_side_value = std::mem::take(r_map);

                *l_map = left_side_value.union(right_side_value);

                return Ok(std::mem::replace(hml, StahlVal::Void));
            }
        },

        _ => {
            stop!(TypeMismatch => "hash-union expects two hash maps, found: {:?} and {:?}", hml, hmr)
        }
    }
}

#[cfg(test)]
mod hashmap_tests {
    use super::*;

    #[cfg(not(feature = "sync"))]
    use im_rc::hashmap;

    #[cfg(not(feature = "sync"))]
    use im_rc::vector;

    #[cfg(feature = "sync")]
    use im::hashmap;

    #[cfg(feature = "sync")]
    use im::vector;

    use crate::rvals::{StahlString, StahlVal::*};

    #[test]
    fn hm_construct_normal() {
        let args = [
            StringV("foo".into()),
            StringV("bar".into()),
            StringV("foo2".into()),
            StringV("bar2".into()),
        ];
        let res = hm_construct(&args);
        let expected = StahlVal::HashMapV(
            Gc::new(hashmap! {
                StringV("foo".into()) => StringV("bar".into()),
                StringV("foo2".into()) => StringV("bar2".into())
            })
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_construct_with_duplicates() {
        let args = [
            StringV("foo".into()),
            StringV("bar".into()),
            StringV("foo2".into()),
            StringV("bar2".into()),
            StringV("foo".into()),
            StringV("bar".into()),
            StringV("foo2".into()),
            StringV("bar2".into()),
        ];
        let res = hm_construct(&args);
        let expected = StahlVal::HashMapV(
            Gc::new(hashmap! {
                StringV("foo".into()) => StringV("bar".into()),
                StringV("foo2".into()) => StringV("bar2".into())
            })
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_insert_from_empty() {
        let mut args = [
            HashMapV(Gc::new(hashmap![]).into()),
            StringV("foo".into()),
            StringV("bar".into()),
        ];
        let res = stahl_hash_insert(&mut args);
        let expected = StahlVal::HashMapV(
            Gc::new(hashmap! {
                StringV("foo".into()) => StringV("bar".into())
            })
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_get_found() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("foo".into()),
        ];
        let res = stahl_hash_ref(&args);
        let expected = StringV("bar".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_get_error() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("garbage".into()),
        ];
        let res = stahl_hash_ref(&args);
        assert!(res.is_err());
    }

    #[test]
    fn hm_try_get_found() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("foo".into()),
        ];
        let res = stahl_hash_try_get(&args);
        let expected = StringV("bar".into());
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_try_get_error() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("garbage".into()),
        ];
        let res = stahl_hash_contains(&args);
        let expected = StahlVal::BoolV(false);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_contains_true() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("foo".into()),
        ];
        let res = stahl_hash_contains(&args);
        let expected = StahlVal::BoolV(true);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_contains_false() {
        let args = [
            HashMapV(
                Gc::new(hashmap! {
                    StringV("foo".into()) => StringV("bar".into())
                })
                .into(),
            ),
            StringV("bar".into()),
        ];
        let res = stahl_hash_contains(&args);
        let expected = StahlVal::BoolV(false);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hm_keys_to_vector_normal() {
        let args = vec![HashMapV(
            Gc::new(hashmap! {
                StringV("foo".into()) => StringV("bar".into()),
                StringV("bar".into()) => StringV("baz".into()),
                StringV("baz".into()) => StringV("quux".into())
            })
            .into(),
        )];
        let res = stahl_keys_to_vector(&args);
        let expected = vector![
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("baz".into()),
        ]
        .into();

        // pull out the vectors and sort them
        // let unwrapped_res: StahlVal = (*res.unwrap()).clone();
        // let unwrapped_expected: StahlVal = (*expected).clone();

        let mut res_vec_string: Vec<StahlString> = if let StahlVal::VectorV(v) = res.unwrap() {
            v.iter()
                .map(|x| {
                    if let StahlVal::StringV(ref s) = x {
                        s.clone()
                    } else {
                        panic!("test failed")
                    }
                })
                .collect()
        } else {
            panic!("test failed")
        };

        let mut expected_vec_string: Vec<StahlString> = if let StahlVal::VectorV(v) = expected {
            v.iter()
                .map(|x| {
                    if let StahlVal::StringV(ref s) = x {
                        s.clone()
                    } else {
                        panic!("test failed")
                    }
                })
                .collect()
        } else {
            panic!("test failed")
        };

        res_vec_string.sort();
        expected_vec_string.sort();

        assert_eq!(res_vec_string, expected_vec_string);
    }

    #[test]
    fn hm_values_to_vector_normal() {
        let args = vec![HashMapV(
            Gc::new(hashmap! {
                StringV("foo".into()) => StringV("bar".into()),
                StringV("bar".into()) => StringV("baz".into()),
                StringV("baz".into()) => StringV("quux".into())
            })
            .into(),
        )];
        let res = stahl_values_to_vector(&args);
        let expected = vector![
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("baz".into()),
            StahlVal::StringV("quux".into()),
        ]
        .into();

        // pull out the vectors and sort them

        let mut res_vec_string: Vec<StahlString> = if let StahlVal::VectorV(v) = res.unwrap() {
            v.iter()
                .map(|x| {
                    if let StahlVal::StringV(ref s) = x {
                        s.clone()
                    } else {
                        panic!("test failed")
                    }
                })
                .collect()
        } else {
            panic!("test failed")
        };

        let mut expected_vec_string: Vec<StahlString> = if let StahlVal::VectorV(v) = expected {
            v.iter()
                .map(|x| {
                    if let StahlVal::StringV(ref s) = x {
                        s.clone()
                    } else {
                        panic!("test failed")
                    }
                })
                .collect()
        } else {
            panic!("test failed")
        };

        res_vec_string.sort();
        expected_vec_string.sort();

        assert_eq!(res_vec_string, expected_vec_string);
    }
}
