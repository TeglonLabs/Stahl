use crate::rvals::StahlHashSet;
use crate::rvals::{Result, StahlVal};
use crate::stahl_vm::vm::VmCore;
use crate::values::lists::List;
use crate::values::HashSet;
use crate::{gc::Gc, stahl_vm::builtin::BuiltInModule};
use crate::{stop, Vector};

pub(crate) fn hashset_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/sets");
    module
        .register_native_fn_definition(HS_CONSTRUCT_DEFINITION)
        .register_native_fn_definition(HASHSET_LENGTH_DEFINITION)
        .register_native_fn_definition(HASHSET_CONTAINS_DEFINITION)
        .register_native_fn_definition(HS_INSERT_DEFINITION)
        .register_native_fn_definition(HASHSET_TO_LIST_DEFINITION)
        .register_native_fn_definition(HASHSET_TO_IMMUTABLE_VECTOR_DEFINITION)
        .register_native_fn_definition(HASHSET_TO_MUTABLE_VECTOR_DEFINITION)
        .register_native_fn_definition(HASHSET_CLEAR_DEFINITION)
        .register_native_fn_definition(HASHSET_IS_SUBSET_DEFINITION)
        .register_native_fn_definition(LIST_TO_HASHSET_DEFINITION);
    module
}

/// Constructs a new hash set
///
/// # Examples
/// ```scheme
/// (hashset 10 20 30 40)
/// ```
#[stahl_derive::native(name = "hashset", arity = "AtLeast(0)")]
pub fn hs_construct(args: &[StahlVal]) -> Result<StahlVal> {
    let mut hs = HashSet::new();

    for key in args {
        if key.is_hashable() {
            hs.insert(key.clone());
        } else {
            stop!(TypeMismatch => "hash key not hashable!");
        }
    }

    Ok(StahlVal::HashSetV(Gc::new(hs).into()))
}

/// Get the number of elements in the hashset
///
/// # Examples
/// ```scheme
/// (hashset-length (hashset 10 20 30)) ;; => 3
/// ```
#[stahl_derive::function(name = "hashset-length")]
pub fn hashset_length(hashset: &StahlHashSet) -> usize {
    hashset.len()
}

/// Insert a new element into the hashset. Returns a hashset.
///
/// # Examples
/// ```scheme
/// (define hs (hashset 10 20 30))
/// (define updated (hashset-insert hs 40))
/// (equal? hs (hashset 10 20 30)) ;; => #true
/// (equal? updated (hashset 10 20 30 40)) ;; => #true
/// ```
#[stahl_derive::function(name = "hashset-insert")]
pub fn hs_insert(hashset: &mut StahlVal, value: StahlVal) -> Result<StahlVal> {
    if value.is_hashable() {
        if let StahlVal::HashSetV(StahlHashSet(hs)) = hashset {
            match Gc::get_mut(hs) {
                Some(m) => {
                    m.insert(value);
                    Ok(std::mem::replace(hashset, StahlVal::Void))
                }

                None => Ok(StahlVal::HashSetV(StahlHashSet(Gc::new(hs.update(value))))),
            }
        } else {
            stop!(TypeMismatch => "set insert takes a set")
        }
    } else {
        stop!(TypeMismatch => "hash key not hashable!");
    }
}

/// Test if the hashset contains a given element.
///
/// # Examples
/// ```scheme
/// (hashset-contains? (hashset 10 20) 10) ;; => #true
/// (hashset-contains? (hashset 10 20) "foo") ;; => #false
/// ```
#[stahl_derive::function(name = "hashset-contains?")]
pub fn hashset_contains(hashset: &StahlHashSet, key: &StahlVal) -> Result<StahlVal> {
    if key.is_hashable() {
        Ok(StahlVal::BoolV(hashset.contains(key)))
    } else {
        stop!(TypeMismatch => "hash key not hashable!: {}", key);
    }
}

/// Check if the left set is a subset of the right set
///
/// # Examples
/// ```scheme
/// (hashset-subset? (hash 10) (hashset 10 20)) ;; => #true
/// (hashset-subset? (hash 100) (hashset 30)) ;; => #false
/// ```
#[stahl_derive::function(name = "hashset-subset?")]
pub fn hashset_is_subset(left: &StahlHashSet, right: &StahlHashSet) -> bool {
    left.is_subset(right.0.as_ref())
}

/// Creates a list from this hashset. The order of the list is not guaranteed.
///
/// # Examples
/// ```scheme
/// (hashset->list (hashset 10 20 30)) ;; => '(10 20 30)
/// (hashset->list (hashset 10 20 30)) ;; => '(20 10 30)
/// ```
#[stahl_derive::function(name = "hashset->list")]
pub fn hashset_to_list(hashset: &StahlHashSet) -> StahlVal {
    StahlVal::ListV(hashset.iter().cloned().collect::<List<StahlVal>>())
}

/// Creates an immutable vector from this hashset. The order of the vector is not guaranteed.
///
/// # Examples
/// ```scheme
/// (hashset->immutable-vector (hashset 10 20 30)) ;; => '#(10 20 30)
/// (hashset->immutable-vector (hashset 10 20 30)) ;; => '#(20 10 30)
/// ```
#[stahl_derive::function(name = "hashset->immutable-vector")]
pub fn hashset_to_immutable_vector(hashset: &StahlHashSet) -> StahlVal {
    StahlVal::VectorV(Gc::new(hashset.0.iter().cloned().collect::<Vector<_>>()).into())
}

/// Creates a mutable vector from this hashset. The order of the vector is not guaranteed.
///
/// # Examples
/// ```scheme
/// (hashset->vector (hashset 10 20 30)) ;; => '#(10 20 30)
/// (hashset->vector (hashset 10 20 30)) ;; => '#(20 10 30)
/// ```
#[stahl_derive::context(name = "hashset->vector", arity = "Exact(1)")]
pub fn hashset_to_mutable_vector(ctx: &mut VmCore, args: &[StahlVal]) -> Option<Result<StahlVal>> {
    fn hashset_to_mutable_vector_impl(ctx: &mut VmCore, args: &[StahlVal]) -> Result<StahlVal> {
        if args.len() != 1 {
            stop!(ArityMismatch => "hashset->vector takes 1 argument")
        }

        let hashset = &args[0];

        if let StahlVal::HashSetV(hs) = hashset {
            Ok(ctx.make_mutable_vector(hs.iter().cloned().collect()))
        } else {
            stop!(TypeMismatch => "hashset->vector takes a hashset")
        }
    }

    Some(hashset_to_mutable_vector_impl(ctx, args))
}

/// Clears the hashset and returns the passed in hashset.
/// This first checks if there are no other references to this hashset,
/// and if there aren't, clears that allocation. Given that there are
/// functional updates, this is only relevant if there are no more
/// references to a given hashset, and you want to reuse its allocation.
#[stahl_derive::function(name = "hashset-clear")]
pub fn hashset_clear(hashset: &mut StahlVal) -> Result<StahlVal> {
    if let StahlVal::HashSetV(StahlHashSet(hs)) = hashset {
        match Gc::get_mut(hs) {
            Some(m) => {
                m.clear();
                Ok(std::mem::replace(hashset, StahlVal::Void))
            }
            None => Ok(StahlVal::HashSetV(Gc::new(HashSet::new()).into())),
        }
    } else {
        stop!(TypeMismatch => format!("hashset-clear takes a hashset, found: {}", hashset))
    }
}

/// Convert the given list into a hashset.
///
/// # Examples
/// ```scheme
/// (list 10 20 30) ;; => (hashset 10 20 30)
/// ```
#[stahl_derive::function(name = "list->hashset")]
pub fn list_to_hashset(l: &List<StahlVal>) -> StahlVal {
    StahlVal::HashSetV(Gc::new(l.iter().cloned().collect::<HashSet<_>>()).into())
}

#[cfg(test)]
mod hashset_tests {
    use crate::rvals::StahlString;

    use super::*;

    #[cfg(not(feature = "sync"))]
    use im_rc::vector;

    #[cfg(feature = "sync")]
    use im::vector;

    #[test]
    fn hs_construct_normal() {
        let args = [
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("foo2".into()),
            StahlVal::StringV("bar2".into()),
        ];
        let res = hs_construct(&args);
        let expected = StahlVal::HashSetV(
            Gc::new(
                vec![
                    StahlVal::StringV("foo".into()),
                    StahlVal::StringV("bar".into()),
                    StahlVal::StringV("foo2".into()),
                    StahlVal::StringV("bar2".into()),
                ]
                .into_iter()
                .map(Gc::new)
                .collect::<HashSet<_>>(),
            )
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hs_construct_with_duplicates() {
        let args = [
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("foo2".into()),
            StahlVal::StringV("bar2".into()),
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("foo2".into()),
            StahlVal::StringV("bar2".into()),
        ];
        let res = hs_construct(&args);
        let expected = StahlVal::HashSetV(
            Gc::new(
                vec![
                    StahlVal::StringV("foo".into()),
                    StahlVal::StringV("bar".into()),
                    StahlVal::StringV("foo2".into()),
                    StahlVal::StringV("bar2".into()),
                ]
                .into_iter()
                .map(Gc::new)
                .collect::<HashSet<_>>(),
            )
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hs_insert_from_empty() {
        let mut args = [
            StahlVal::HashSetV(Gc::new(HashSet::new()).into()),
            StahlVal::StringV("foo".into()),
        ];
        let res = stahl_hs_insert(&mut args);
        let expected = StahlVal::HashSetV(
            Gc::new(
                vec![StahlVal::StringV("foo".into())]
                    .into_iter()
                    .map(Gc::new)
                    .collect::<HashSet<_>>(),
            )
            .into(),
        );
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hs_contains_true() {
        let args = [
            StahlVal::HashSetV(
                Gc::new(
                    vec![StahlVal::StringV("foo".into())]
                        .into_iter()
                        .map(Gc::new)
                        .collect::<HashSet<_>>(),
                )
                .into(),
            ),
            StahlVal::StringV("foo".into()),
        ];
        let res = stahl_hashset_contains(&args);
        let expected = StahlVal::BoolV(true);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hs_contains_false() {
        let args = [
            StahlVal::HashSetV(
                Gc::new(
                    vec![StahlVal::StringV("foo".into())]
                        .into_iter()
                        .map(Gc::new)
                        .collect::<HashSet<_>>(),
                )
                .into(),
            ),
            StahlVal::StringV("bar".into()),
        ];
        let res = stahl_hashset_contains(&args);
        let expected = StahlVal::BoolV(false);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn hs_keys_to_vector_normal() {
        let args = [StahlVal::HashSetV(
            Gc::new(
                vec![
                    StahlVal::StringV("foo".into()),
                    StahlVal::StringV("bar".into()),
                    StahlVal::StringV("baz".into()),
                ]
                .into_iter()
                .collect::<HashSet<_>>(),
            )
            .into(),
        )];
        let res = stahl_hashset_to_immutable_vector(&args);
        let expected = vector![
            StahlVal::StringV("foo".into()),
            StahlVal::StringV("bar".into()),
            StahlVal::StringV("baz".into()),
        ]
        .into();

        // pull out the vectors and sort them
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
}
