use std::collections::HashMap;

use crate::values::lists::List;
use weak_table::WeakKeyHashMap;

use crate::{rvals::Custom, values::functions::ByteCodeLambda, StahlVal};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct FunctionArgs {
    function: StahlVal,
    arguments: Vec<StahlVal>,
}

#[derive(Clone, Debug)]
// For now this has... no capacity, and no eviction strategy
pub struct MemoizationTable {
    table: HashMap<FunctionArgs, StahlVal>,
}

impl MemoizationTable {
    pub fn new() -> Self {
        Self {
            table: HashMap::default(),
        }
    }

    pub fn insert(&mut self, function: StahlVal, arguments: Vec<StahlVal>, value: StahlVal) {
        self.table.insert(
            FunctionArgs {
                function,
                arguments,
            },
            value,
        );
    }

    pub fn get(&self, function: StahlVal, arguments: Vec<StahlVal>) -> Option<StahlVal> {
        self.table
            .get(&FunctionArgs {
                function,
                arguments,
            })
            .cloned()
    }
}

pub struct WeakMemoizationTable {
    #[cfg(not(feature = "sync"))]
    table: WeakKeyHashMap<std::rc::Weak<ByteCodeLambda>, HashMap<List<StahlVal>, StahlVal>>,

    #[cfg(feature = "sync")]
    table: WeakKeyHashMap<std::sync::Weak<ByteCodeLambda>, HashMap<List<StahlVal>, StahlVal>>,
}

impl WeakMemoizationTable {
    pub fn new() -> Self {
        Self {
            table: WeakKeyHashMap::default(),
        }
    }

    pub fn insert(
        &mut self,
        function: StahlVal,
        arguments: List<StahlVal>,
        value: StahlVal,
    ) -> crate::rvals::Result<()> {
        // println!("Inserting args: {:?}", arguments);

        if let StahlVal::Closure(l) = function {
            if let Some(map) = self.table.get_mut(&l) {
                map.insert(arguments, value);
            } else {
                let mut map = HashMap::new();
                map.insert(arguments, value);

                self.table.insert(l.0, map);
            }
        } else {
            stop!(TypeMismatch => "memoization table expected a function, found: {:?}", function);
        }

        Ok(())
    }

    pub fn get(
        &self,
        function: StahlVal,
        arguments: List<StahlVal>,
    ) -> crate::rvals::Result<Option<StahlVal>> {
        if let StahlVal::Closure(l) = function {
            Ok(self.table.get(&l).and_then(|x| x.get(&arguments)).cloned())
        } else {
            stop!(TypeMismatch => "memoization table expected a function, found: {:?}", function);
        }
    }
}

impl Custom for WeakMemoizationTable {}
