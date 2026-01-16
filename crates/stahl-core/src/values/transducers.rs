use crate::gc::Gc;
use crate::rvals::Result;
use crate::StahlVal;

use crate::core::utils::{arity_check, declare_const_ref_functions};

// Make a transducer actually contain an option to a rooted value, otherwise
// it is a source agnostic transformer on the (eventual) input
#[derive(Clone, PartialEq, Hash)]
pub struct Transducer {
    // root: Gc<StahlVal>,
    pub ops: Vec<Transducers>,
}

impl Transducer {
    pub fn new() -> Self {
        Transducer { ops: Vec::new() }
    }

    pub fn append(&mut self, mut other: Self) {
        self.ops.append(&mut other.ops)
    }

    pub fn push(&mut self, t: Transducers) {
        self.ops.push(t);
    }
}

impl Default for Transducer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, PartialEq, Hash)]
pub enum Transducers {
    Map(StahlVal),          // function
    Filter(StahlVal),       // function
    Take(StahlVal),         // integer
    Drop(StahlVal),         // integer
    FlatMap(StahlVal),      // function
    Flatten,                // Takes nothing
    Window(StahlVal),       // integer
    TakeWhile(StahlVal),    // function
    DropWhile(StahlVal),    // function
    Extend(StahlVal),       // Collection
    Cycle,                  // Continue forever
    Enumerating,            // turns (a b c) into ((0 a) (1 b) (2 c))
    Zipping(StahlVal),      // Combine with another iterator, either a Collection or a Transducer
    Interleaving(StahlVal), // Interleave with another interator, either a Collection or a Transducer
}

// This should just describe how a sequence of values can be reduced
// assert that the function passed in has an arity of 2
// and the initival
#[derive(Clone, Debug)]
pub struct ReducerFunc {
    pub(crate) initial_value: StahlVal,
    pub(crate) function: StahlVal,
}

impl ReducerFunc {
    fn new(initial_value: StahlVal, function: StahlVal) -> Self {
        ReducerFunc {
            initial_value,
            function,
        }
    }
}

// Defines how to collect a function
// defaults to the same input type?

#[derive(Clone, Debug)]
pub enum Reducer {
    // Sum the sequence
    Sum,
    // Multiply the sequence
    Multiply,
    // Find the Max of the sequence
    Max,
    // Find the min of the sequence
    Min,
    // Count the elements in the sequence
    Count,
    // Give the nth elements
    Nth(usize),
    // Collect into a list
    List,
    // Collect into a vector
    Vector,
    // Collect into a hash map
    HashMap,
    // Collect into a hash set
    HashSet,
    // Collect into a string
    String,
    // Consumes the iterator, giving the last value
    Last,
    // For-each -> calls a function for each value in the sequence
    ForEach(StahlVal),
    // Collect according to the function
    Generic(ReducerFunc),
}

macro_rules! into_collection {
    ($($name:tt => $collection:tt),* $(,)? ) => {
        $ (
            fn $name(args: &[StahlVal]) -> Result<StahlVal> {
                arity_check!($name, args, 0);
                Ok(StahlVal::ReducerV(Gc::new(Reducer::$collection)))
            }
        ) *
    }
}

declare_const_ref_functions! {
    INTO_SUM => into_sum,
    INTO_PRODUCT => into_multiply,
    INTO_MAX => into_max,
    INTO_MIN => into_min,
    INTO_COUNT => into_count,
    INTO_LIST => into_list,
    INTO_VECTOR => into_vector,
    INTO_HASHMAP => into_hashmap,
    INTO_HASHSET => into_hashset,
    INTO_STRING => into_string,
    INTO_LAST => into_last,
    FOR_EACH => for_each,
    REDUCER => generic,
    NTH => nth,
}

into_collection! {
    into_sum => Sum,
    into_multiply => Multiply,
    into_max => Max,
    into_min => Min,
    into_count => Count,
    into_list => List,
    into_vector => Vector,
    into_hashmap => HashMap,
    into_hashset => HashSet,
    into_string => String,
    into_last => Last
}

fn for_each(args: &[StahlVal]) -> Result<StahlVal> {
    arity_check!(for_each, args, 1);
    let function = args[0].clone();
    Ok(StahlVal::ReducerV(Gc::new(Reducer::ForEach(function))))
}

fn generic(args: &[StahlVal]) -> Result<StahlVal> {
    arity_check!(reducer, args, 2);
    let function = args[0].clone();
    let initial_value = args[1].clone();
    Ok(StahlVal::ReducerV(Gc::new(Reducer::Generic(
        ReducerFunc::new(initial_value, function),
    ))))
}

fn nth(args: &[StahlVal]) -> Result<StahlVal> {
    arity_check!(nth, args, 1);

    let number = args[0].clone();

    if let StahlVal::IntV(n) = number {
        if n < 0 {
            stop!(TypeMismatch => format!("nth expected a (postive) integer, found: {number}"));
        }
        Ok(StahlVal::ReducerV(Gc::new(Reducer::Nth(n as usize))))
    } else {
        stop!(TypeMismatch => format!("nth expected a (postive) integer, found: {number}"))
    }
}

// Reducer functions
