use std::cell::Cell;

use im_lists::{
    handler::{DefaultDropHandler, DropHandler},
    shared::PointerFamily,
};

use crate::{
    gc::Gc,
    rvals::{FromStahlVal, IntoStahlVal},
    StahlVal,
};

// TODO:
// Builtin immutable pairs
#[derive(Clone, Hash)]
pub struct Pair {
    pub(crate) car: StahlVal,
    pub(crate) cdr: StahlVal,
}

impl Pair {
    pub fn cons(car: StahlVal, cdr: StahlVal) -> Self {
        Pair { car, cdr }
    }

    pub fn car(&self) -> StahlVal {
        self.car.clone()
    }

    pub fn cdr(&self) -> StahlVal {
        self.cdr.clone()
    }
}

impl From<Pair> for StahlVal {
    fn from(pair: Pair) -> Self {
        StahlVal::Pair(Gc::new(pair))
    }
}

impl std::fmt::Debug for Pair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({} . {})", &self.car, &self.cdr)
    }
}

#[cfg(feature = "without-drop-protection")]
type DropHandlerChoice = im_lists::handler::DefaultDropHandler;
#[cfg(not(feature = "without-drop-protection"))]
type DropHandlerChoice = list_drop_handler::ListDropHandler;

thread_local! {
    pub static DEPTH: Cell<usize> = Cell::new(0);
}

pub struct GcPointerType;

impl PointerFamily for GcPointerType {
    type Pointer<T> = Gc<T>;

    fn new<T>(value: T) -> Self::Pointer<T> {
        Gc::new(value)
    }

    fn strong_count<T>(this: &Self::Pointer<T>) -> usize {
        Gc::strong_count(this)
    }

    fn try_unwrap<T>(this: Self::Pointer<T>) -> Option<T> {
        Gc::try_unwrap(this).ok()
    }

    fn get_mut<T>(this: &mut Self::Pointer<T>) -> Option<&mut T> {
        Gc::get_mut(this)
    }

    fn ptr_eq<T>(this: &Self::Pointer<T>, other: &Self::Pointer<T>) -> bool {
        Gc::ptr_eq(this, other)
    }

    fn make_mut<T: Clone>(ptr: &mut Self::Pointer<T>) -> &mut T {
        Gc::make_mut(ptr)
    }

    fn clone<T>(ptr: &Self::Pointer<T>) -> Self::Pointer<T> {
        Gc::clone(ptr)
    }

    fn as_ptr<T>(this: &Self::Pointer<T>) -> *const T {
        Gc::as_ptr(this)
    }
}

#[cfg(not(feature = "without-drop-protection"))]
mod list_drop_handler {

    use std::collections::VecDeque;

    use super::*;

    pub struct ListDropHandler;

    use crate::rvals::cycles::{drop_impls::DROP_BUFFER, IterativeDropHandler};

    impl DropHandler<im_lists::list::GenericList<StahlVal, PointerType, 4, 2, Self>>
        for ListDropHandler
    {
        fn drop_handler(obj: &mut im_lists::list::GenericList<StahlVal, PointerType, 4, 2, Self>) {
            if obj.strong_count() == 1 {
                if obj.is_empty() {
                    return;
                }

                if DROP_BUFFER
                    .try_with(|drop_buffer| {
                        if let Ok(mut drop_buffer) = drop_buffer.try_borrow_mut() {
                            // Optimistically check what these values are. If they're
                            // primitives, then we can just skip pushing them back
                            // entirely.
                            for value in std::mem::take(obj).draining_iterator() {
                                match &value {
                                    StahlVal::BoolV(_)
                                    | StahlVal::NumV(_)
                                    | StahlVal::IntV(_)
                                    | StahlVal::CharV(_)
                                    | StahlVal::Void
                                    | StahlVal::StringV(_)
                                    | StahlVal::FuncV(_)
                                    | StahlVal::SymbolV(_)
                                    | StahlVal::FutureFunc(_)
                                    | StahlVal::FutureV(_)
                                    | StahlVal::BoxedFunction(_)
                                    | StahlVal::MutFunc(_)
                                    | StahlVal::BuiltIn(_)
                                    | StahlVal::BigNum(_) => continue,
                                    _ => {
                                        drop_buffer.push_back(value);
                                    }
                                }
                            }

                            IterativeDropHandler::bfs(&mut drop_buffer);
                        } else {
                            let mut drop_buffer = VecDeque::new();

                            for value in std::mem::take(obj).draining_iterator() {
                                match &value {
                                    StahlVal::BoolV(_)
                                    | StahlVal::NumV(_)
                                    | StahlVal::IntV(_)
                                    | StahlVal::CharV(_)
                                    | StahlVal::Void
                                    | StahlVal::StringV(_)
                                    | StahlVal::FuncV(_)
                                    | StahlVal::SymbolV(_)
                                    | StahlVal::FutureFunc(_)
                                    | StahlVal::FutureV(_)
                                    | StahlVal::BoxedFunction(_)
                                    | StahlVal::MutFunc(_)
                                    | StahlVal::BuiltIn(_)
                                    | StahlVal::BigNum(_) => continue,
                                    _ => {
                                        drop_buffer.push_back(value);
                                    }
                                }
                            }

                            IterativeDropHandler::bfs(&mut drop_buffer);
                        }
                    })
                    .is_err()
                {
                    let mut drop_buffer = VecDeque::new();
                    for value in std::mem::take(obj).draining_iterator() {
                        match &value {
                            StahlVal::BoolV(_)
                            | StahlVal::NumV(_)
                            | StahlVal::IntV(_)
                            | StahlVal::CharV(_)
                            | StahlVal::Void
                            | StahlVal::StringV(_)
                            | StahlVal::FuncV(_)
                            | StahlVal::SymbolV(_)
                            | StahlVal::FutureFunc(_)
                            | StahlVal::FutureV(_)
                            | StahlVal::BoxedFunction(_)
                            | StahlVal::MutFunc(_)
                            | StahlVal::BuiltIn(_)
                            | StahlVal::BigNum(_) => continue,
                            _ => {
                                drop_buffer.push_back(value);
                            }
                        }
                    }

                    IterativeDropHandler::bfs(&mut drop_buffer);
                }
            }
        }
    }
}

// #[cfg(not(feature = "sync"))]
// type PointerType = im_lists::shared::RcPointer;

// #[cfg(feature = "sync")]
// type PointerType = im_lists::shared::ArcPointer;

type PointerType = GcPointerType;

pub type StahlList<T> = im_lists::list::GenericList<T, PointerType, 4, 2, DefaultDropHandler>;

pub type List<T> = im_lists::list::GenericList<T, PointerType, 4, 2, DropHandlerChoice>;

pub type ConsumingIterator<T> =
    im_lists::list::ConsumingIter<T, PointerType, 4, 2, DropHandlerChoice>;

impl<T: FromStahlVal + Clone, D: im_lists::handler::DropHandler<Self>> FromStahlVal
    for im_lists::list::GenericList<T, PointerType, 4, 2, D>
{
    fn from_stahlval(val: &StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::ListV(l) = val {
            l.iter().map(T::from_stahlval).collect()
        } else {
            stop!(TypeMismatch => "Unable to convert StahlVal to List, found: {}", val);
        }
    }
}

impl<T: IntoStahlVal + Clone, D: im_lists::handler::DropHandler<Self>> IntoStahlVal
    for im_lists::list::GenericList<T, PointerType, 4, 2, D>
{
    fn into_stahlval(self) -> crate::rvals::Result<StahlVal> {
        self.into_iter()
            .map(|x| x.into_stahlval())
            .collect::<crate::rvals::Result<List<_>>>()
            .map(StahlVal::ListV)
    }
}
