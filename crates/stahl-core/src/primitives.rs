pub mod bytevectors;
pub mod contracts;
mod control;
mod fs;
pub mod hashmaps;
pub mod hashsets;
pub mod http;
mod io;
pub mod lists;
pub mod meta_ops;
/// Implements numbers as defined in section 6.2 of the R7RS spec.
pub mod numbers;

#[cfg(not(target_arch = "wasm32"))]
pub mod polling;

pub mod ports;
pub mod process;
pub mod random;
mod streams;
pub mod strings;
mod symbols;
pub mod tcp;
pub mod time;
pub mod transducers;
mod utils;
pub mod vectors;

// This is for boot strapping the package
// manager with an embedded git implementation,
// as to not require depending on the system git.
pub mod git;

use crate::gc::{Gc, GcMut};
use crate::rvals::{FromStahlVal, IntoStahlVal, StahlByteVector};
use crate::rvals::{
    FunctionSignature, PrimitiveAsRef, PrimitiveAsRefMut, StahlHashMap, StahlHashSet, StahlVal,
    StahlVector,
};
use crate::values::closed::HeapRef;
use crate::values::lists::List;
use crate::values::port::StahlPort;
use crate::values::structs::UserDefinedStruct;
use crate::values::Vector;
use crate::{
    rerrs::{ErrorKind, StahlErr},
    rvals::StahlString,
};
pub use control::ControlOperations;
pub use fs::{fs_module, fs_module_sandbox};
pub use io::IoFunctions;
pub use lists::UnRecoverableResult;
pub use meta_ops::MetaOperations;
use num::{BigInt, BigRational, Rational32, ToPrimitive};
pub use numbers::{add_primitive, divide_primitive, multiply_primitive, subtract_primitive};
pub use ports::port_module;
use std::convert::TryFrom;
use std::result;
pub use streams::StreamOperations;
pub use strings::string_module;
pub use symbols::symbol_module;
pub use vectors::VectorOperations;

macro_rules! try_from_impl {
    ($type:ident => $($body:ty),*) => {
        $(
            impl TryFrom<StahlVal> for $body {
                type Error = StahlErr;
                #[inline]
                fn try_from(value: StahlVal) -> result::Result<Self, Self::Error> {
                    match value {
                        StahlVal::$type(x) => Ok(x.clone() as $body),
                        _ => Err(StahlErr::new(ErrorKind::ConversionError, format!("Expected number, found: {}", value))),
                    }
                }
            }

            impl TryFrom<&StahlVal> for $body {
                type Error = StahlErr;
                #[inline]
                fn try_from(value: &StahlVal) -> result::Result<Self, Self::Error> {
                    match value {
                        StahlVal::$type(x) => Ok(x.clone() as $body),
                        _ => Err(StahlErr::new(ErrorKind::ConversionError, format!("Expected number, found: {}", value))),
                    }
                }
            }

            impl FromStahlVal for $body {
                #[inline]
                fn from_stahlval(value: &StahlVal) -> result::Result<Self, StahlErr> {
                    match value {
                        StahlVal::$type(x) => Ok(x.clone() as $body),
                        _ => Err(StahlErr::new(ErrorKind::ConversionError, format!("Expected number, found: {}", value))),
                    }
                }
            }

        )*
    };
}

macro_rules! from_f64 {
    ($($body:ty),*) => {
        $(
            impl From<$body> for StahlVal {
                #[inline]
                fn from(val: $body) -> StahlVal {
                    StahlVal::NumV(val as f64)
                }
            }

            impl IntoStahlVal for $body {
                #[inline]
                fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
                    Ok(StahlVal::NumV(self as f64))
                }
            }
        )*
    };
}

macro_rules! from_for_isize {
    ($($body:ty),*) => {
        $(
            impl From<$body> for StahlVal {
                #[inline]
                fn from(val: $body) -> StahlVal {
                    StahlVal::IntV(val as isize)
                }
            }

            impl IntoStahlVal for $body {
                #[inline]
                fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
                    Ok(StahlVal::IntV(self as isize))
                }
            }
        )*
    };
}

impl From<i64> for StahlVal {
    fn from(value: i64) -> Self {
        if let Ok(converted) = TryInto::<isize>::try_into(value) {
            StahlVal::IntV(converted)
        } else {
            StahlVal::BigNum(Gc::new(value.into()))
        }
    }
}

impl FromStahlVal for u8 {
    #[inline]
    fn from_stahlval(val: &StahlVal) -> crate::rvals::Result<Self> {
        match val {
            StahlVal::IntV(v) => (*v).try_into().map_err(|_err| {
                StahlErr::new(
                    ErrorKind::ConversionError,
                    format!("Unable to convert isize to u8: {}", v),
                )
            }),
            StahlVal::BigNum(n) => n.as_ref().try_into().map_err(|_err| {
                StahlErr::new(
                    ErrorKind::ConversionError,
                    format!("Unable to convert bignum to u8: {:?}", n),
                )
            }),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                format!("Unable to convert stahlval to u8: {}", val),
            )),
        }
    }
}

impl From<usize> for StahlVal {
    #[inline]
    fn from(value: usize) -> Self {
        if value > isize::MAX as usize {
            StahlVal::BigNum(Gc::new(value.into()))
        } else {
            StahlVal::IntV(value as isize)
        }
    }
}

impl IntoStahlVal for usize {
    #[inline]
    fn into_stahlval(self) -> crate::rvals::Result<StahlVal> {
        Ok(StahlVal::from(self))
    }
}

impl IntoStahlVal for i64 {
    #[inline]
    fn into_stahlval(self) -> crate::rvals::Result<StahlVal> {
        Ok(self.into())
    }
}

impl FromStahlVal for i64 {
    fn from_stahlval(val: &StahlVal) -> crate::rvals::Result<Self> {
        match val {
            StahlVal::IntV(v) => (*v).try_into().map_err(|_err| {
                StahlErr::new(
                    ErrorKind::ConversionError,
                    format!("Unable to convert i64 to isize: {}", v),
                )
            }),
            StahlVal::BigNum(n) => n.as_ref().try_into().map_err(|_err| {
                StahlErr::new(
                    ErrorKind::ConversionError,
                    format!("Unable to convert bignum to isize: {:?}", n),
                )
            }),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                format!("Unable to convert stahlval to isize: {}", val),
            )),
        }
    }
}

impl From<char> for StahlVal {
    #[inline]
    fn from(val: char) -> StahlVal {
        StahlVal::CharV(val)
    }
}

impl IntoStahlVal for char {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        Ok(StahlVal::CharV(self))
    }
}

impl FromStahlVal for char {
    fn from_stahlval(val: &StahlVal) -> Result<Self, StahlErr> {
        if let StahlVal::CharV(c) = val {
            Ok(*c)
        } else {
            Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Expected character".to_string(),
            ))
        }
    }
}

impl<T: Into<StahlVal>> From<Option<T>> for StahlVal {
    #[inline]
    fn from(val: Option<T>) -> StahlVal {
        if let Some(s) = val {
            s.into()
        } else {
            StahlVal::BoolV(true)
        }
    }
}

impl<T: IntoStahlVal> IntoStahlVal for Option<T> {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        if let Some(s) = self {
            s.into_stahlval()
        } else {
            Ok(StahlVal::BoolV(false))
        }
    }
}

impl<T: FromStahlVal> FromStahlVal for Option<T> {
    #[inline]
    fn from_stahlval(val: &StahlVal) -> Result<Self, StahlErr> {
        if val.is_truthy() {
            Ok(Some(T::from_stahlval(val)?))
        } else {
            Ok(None)
        }
    }
}

impl FromStahlVal for StahlVal {
    #[inline]
    fn from_stahlval(val: &StahlVal) -> Result<Self, StahlErr> {
        Ok(val.clone())
    }
}

impl FromStahlVal for () {
    fn from_stahlval(val: &StahlVal) -> Result<Self, StahlErr> {
        if let StahlVal::Void = val {
            Ok(())
        } else {
            crate::stop!(ConversionError => "could not convert value to unit type")
        }
    }
}

impl IntoStahlVal for () {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        Ok(StahlVal::Void)
    }
}

impl From<()> for StahlVal {
    #[inline]
    fn from(_: ()) -> StahlVal {
        StahlVal::Void
    }
}

impl IntoStahlVal for Rational32 {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        if self.is_integer() {
            self.numer().into_stahlval()
        } else {
            Ok(StahlVal::Rational(self))
        }
    }
}

impl IntoStahlVal for BigInt {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        match self.to_isize() {
            Some(i) => i.into_stahlval(),
            None => Ok(StahlVal::BigNum(crate::gc::Gc::new(self))),
        }
    }
}

impl IntoStahlVal for BigRational {
    #[inline]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        if self.is_integer() {
            let (n, _) = self.into();
            return n.into_stahlval();
        }
        match (self.numer().to_i32(), self.denom().to_i32()) {
            (Some(n), Some(d)) => Rational32::new(n, d).into_stahlval(),
            _ => Ok(StahlVal::BigRational(Gc::new(self))),
        }
    }
}

from_f64!(f64, f32);
from_for_isize!(i32, i16, i8, u8, u16, u32, u64, isize);
try_from_impl!(NumV => f64, f32);
try_from_impl!(IntV => i32, i16, i8, u16, u32, u64, usize, isize);

impl TryFrom<StahlVal> for String {
    type Error = StahlErr;
    #[inline]
    fn try_from(value: StahlVal) -> result::Result<Self, Self::Error> {
        match value {
            StahlVal::StringV(ref x) => Ok(x.to_string()),
            StahlVal::SymbolV(ref x) => Ok(x.to_string()),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Expected string".to_string(),
            )),
        }
    }
}

impl From<StahlVal> for Gc<StahlVal> {
    #[inline]
    fn from(val: StahlVal) -> Self {
        Gc::new(val)
    }
}

impl From<Gc<StahlVal>> for StahlVal {
    #[inline]
    fn from(val: Gc<StahlVal>) -> Self {
        (*val).clone()
    }
}

impl FromStahlVal for String {
    #[inline]
    fn from_stahlval(val: &StahlVal) -> Result<Self, StahlErr> {
        match val {
            StahlVal::StringV(s) | StahlVal::SymbolV(s) => Ok(s.to_string()),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                format!("Expected string, found: {val}"),
            )),
        }
    }
}

impl TryFrom<&StahlVal> for String {
    type Error = StahlErr;
    #[inline]
    fn try_from(value: &StahlVal) -> result::Result<Self, Self::Error> {
        match value {
            StahlVal::StringV(x) => Ok(x.to_string()),
            StahlVal::SymbolV(x) => Ok(x.to_string()),
            _ => Err(StahlErr::new(
                ErrorKind::ConversionError,
                "Expected string".to_string(),
            )),
        }
    }
}

impl From<String> for StahlVal {
    #[inline]
    fn from(val: String) -> StahlVal {
        StahlVal::StringV(val.into())
    }
}

impl IntoStahlVal for &str {
    #[inline]
    fn into_stahlval(self) -> crate::rvals::Result<StahlVal> {
        Ok(StahlVal::StringV(self.into()))
    }
}

impl FromStahlVal for StahlString {
    #[inline]
    fn from_stahlval(val: &StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::StringV(s) = val {
            Ok(s.clone())
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl string", val))
        }
    }
}

pub enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<'a, L: PrimitiveAsRef<'a>, R: PrimitiveAsRef<'a>> PrimitiveAsRef<'a> for Either<L, R> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        let left_type_name = std::any::type_name::<L>();
        let right_type_name = std::any::type_name::<R>();

        let error_thunk = crate::throw!(ConversionError => format!("Cannot convert stahl value to the specified type: {} or {}", left_type_name, right_type_name));

        Self::maybe_primitive_as_ref(val).ok_or_else(error_thunk)
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        L::maybe_primitive_as_ref(val)
            .map(Either::Left)
            .or_else(|| R::maybe_primitive_as_ref(val).map(Either::Right))
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlByteVector {
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        Self::maybe_primitive_as_ref(val).ok_or_else(
            crate::throw!(ConversionError => format!("Cannot convert value to bytevector: {}", val)),
        )
    }

    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::ByteVector(s) = val {
            Some(s)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a UserDefinedStruct {
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        Self::maybe_primitive_as_ref(val).ok_or_else(
            crate::throw!(ConversionError => format!("Cannot convert value to struct: {}", val)),
        )
    }

    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::CustomStruct(s) = val {
            Some(s)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a GcMut<StahlVal> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::Boxed(c) = val {
            Ok(c)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl boxed value", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::Boxed(c) = val {
            Some(c)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a HeapRef<StahlVal> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HeapAllocated(b) = val {
            Ok(b)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl box", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::HeapAllocated(b) = val {
            Some(b)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a char {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::CharV(c) = val {
            Ok(c)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl character", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::CharV(c) = val {
            Some(c)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for char {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::CharV(c) = val {
            Ok(*c)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl character", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::CharV(c) = val {
            Some(*c)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for isize {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::IntV(i) = val {
            Ok(*i)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl int", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::IntV(i) = val {
            Some(*i)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a Gc<Vector<StahlVal>> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::VectorV(p) = val {
            Ok(&p.0)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl vector", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::VectorV(p) = val {
            Some(&p.0)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlVector {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::VectorV(p) = val {
            Ok(p)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl vector", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::VectorV(p) = val {
            Some(p)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a Gc<crate::values::HashSet<StahlVal>> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HashSetV(p) = val {
            Ok(&p.0)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl hashset", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::HashSetV(p) = val {
            Some(&p.0)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlHashSet {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HashSetV(p) = val {
            Ok(p)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl hashset", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::HashSetV(p) = val {
            Some(p)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a HeapRef<Vec<StahlVal>> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::MutableVector(p) = val {
            Ok(p)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl mutable vector", val))
        }
    }

    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::MutableVector(p) = val {
            Some(p)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlPort {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::PortV(p) = val {
            Ok(p)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl port", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::PortV(p) = val {
            Some(p)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a List<StahlVal> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::ListV(l) = val {
            Ok(l)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl list", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::ListV(l) = val {
            Some(l)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlVal {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        Ok(val)
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        Some(val)
    }
}

impl<'a> PrimitiveAsRefMut<'a> for &'a mut StahlVal {
    #[inline(always)]
    fn primitive_as_ref(val: &'a mut StahlVal) -> crate::rvals::Result<Self> {
        Ok(val)
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a mut StahlVal) -> Option<Self> {
        Some(val)
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlString {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::StringV(s) = val {
            Ok(s)
        } else {
            crate::stop!(TypeMismatch => format!("Cannot convert stahl value: {} to stahl string", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::StringV(s) = val {
            Some(s)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a Gc<crate::values::HashMap<StahlVal, StahlVal>> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Ok(&hm.0)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to hashmap", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Some(&hm.0)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRefMut<'a> for &'a mut Gc<crate::values::HashMap<StahlVal, StahlVal>> {
    #[inline(always)]
    fn primitive_as_ref(val: &'a mut StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Ok(&mut hm.0)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to hashmap", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a mut StahlVal) -> Option<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Some(&mut hm.0)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRefMut<'a> for &'a mut StahlByteVector {
    #[inline(always)]
    fn primitive_as_ref(val: &'a mut StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::ByteVector(hm) = val {
            Ok(hm)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to bytevector", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a mut StahlVal) -> Option<Self> {
        if let StahlVal::ByteVector(hm) = val {
            Some(hm)
        } else {
            None
        }
    }
}

impl<'a> PrimitiveAsRef<'a> for &'a StahlHashMap {
    #[inline(always)]
    fn primitive_as_ref(val: &'a StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Ok(hm)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {} to hashmap", val))
        }
    }

    #[inline(always)]
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::HashMapV(hm) = val {
            Some(hm)
        } else {
            None
        }
    }
}

impl IntoStahlVal for String {
    #[inline(always)]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        Ok(StahlVal::StringV(self.into()))
    }
}

impl IntoStahlVal for StahlString {
    #[inline(always)]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        Ok(StahlVal::StringV(self))
    }
}

impl From<String> for Gc<StahlVal> {
    #[inline(always)]
    fn from(val: String) -> Gc<StahlVal> {
        Gc::new(val.into())
    }
}

impl From<bool> for StahlVal {
    #[inline(always)]
    fn from(val: bool) -> StahlVal {
        StahlVal::BoolV(val)
    }
}

impl FromStahlVal for bool {
    #[inline(always)]
    fn from_stahlval(val: &StahlVal) -> crate::rvals::Result<bool> {
        if let StahlVal::BoolV(b) = val {
            Ok(*b)
        } else {
            crate::stop!(ConversionError => format!("Cannot convert stahl value: {val} to boolean"))
        }
    }
}

impl IntoStahlVal for bool {
    #[inline(always)]
    fn into_stahlval(self) -> Result<StahlVal, StahlErr> {
        Ok(StahlVal::BoolV(self))
    }
}

impl From<Vector<StahlVal>> for StahlVal {
    #[inline(always)]
    fn from(val: Vector<StahlVal>) -> StahlVal {
        StahlVal::VectorV(Gc::new(val).into())
    }
}

impl From<FunctionSignature> for StahlVal {
    fn from(val: FunctionSignature) -> StahlVal {
        StahlVal::FuncV(val)
    }
}

#[cfg(test)]
mod try_from_tests {

    use super::*;

    #[test]
    fn from_char() {
        assert_eq!(StahlVal::from('c'), StahlVal::CharV('c'));
    }

    #[test]
    fn from_stahlval_char() {
        assert_eq!(char::from_stahlval(&StahlVal::CharV('c')).unwrap(), 'c')
    }

    #[test]
    fn into_stahlval_char() {
        assert_eq!('c'.into_stahlval().unwrap(), StahlVal::CharV('c'))
    }

    #[test]
    fn from_stahlval_usize() {
        assert_eq!(usize::from_stahlval(&StahlVal::IntV(10)).unwrap(), 10)
    }

    #[test]
    fn from_stahlval_i32() {
        assert_eq!(i32::from_stahlval(&StahlVal::IntV(32)).unwrap(), 32)
    }

    #[test]
    fn into_stahlval_i32() {
        assert_eq!(32.into_stahlval().unwrap(), StahlVal::IntV(32))
    }

    #[test]
    fn from_bool() {
        assert_eq!(StahlVal::from(true), StahlVal::BoolV(true));
    }

    #[test]
    fn try_from_stahlval_string() {
        let expected = "foo".to_string();
        let input = StahlVal::StringV("foo".into());

        let res = String::try_from(input);
        assert_eq!(res.unwrap(), expected);
    }

    #[test]
    fn try_from_stahlval_ref_string() {
        let expected = "foo".to_string();
        let input = StahlVal::StringV("foo".into());

        let res = String::try_from(&input);
        assert_eq!(res.unwrap(), expected);
    }
}
