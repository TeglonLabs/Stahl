pub mod cycles;

use crate::{
    gc::{
        shared::{
            MappedScopedReadContainer, MappedScopedWriteContainer, ScopedReadContainer,
            ScopedWriteContainer, ShareableMut,
        },
        unsafe_erased_pointers::OpaqueReference,
        Gc, GcMut,
    },
    parser::{
        ast::{self, Atom, ExprKind},
        parser::SyntaxObject,
        span::Span,
        tokens::TokenType,
    },
    primitives::numbers::realp,
    rerrs::{ErrorKind, StahlErr},
    stahl_vm::vm::{threads::closure_into_serializable, BuiltInSignature, Continuation},
    values::{
        closed::{Heap, HeapRef, MarkAndSweepContext},
        functions::{BoxedDynFunction, ByteCodeLambda},
        lazy_stream::LazyStream,
        port::{SendablePort, StahlPort},
        structs::{SerializableUserDefinedStruct, UserDefinedStruct},
        transducers::{Reducer, Transducer},
        HashMapConsumingIter, HashSetConsumingIter, StahlPortRepr, VectorConsumingIter,
    },
};
use std::vec::IntoIter;
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    cmp::Ordering,
    convert::TryInto,
    fmt,
    future::Future,
    hash::{Hash, Hasher},
    io::Write,
    ops::Deref,
    pin::Pin,
    rc::Rc,
    result,
    sync::{Arc, Mutex},
    task::Context,
};

// TODO
#[macro_export]
macro_rules! list {
    () => { $crate::rvals::StahlVal::ListV(
        im_lists::list![]
    ) };

    ( $($x:expr),* ) => {{
        $crate::rvals::StahlVal::ListV(vec![$(
            $crate::rvals::IntoStahlVal::into_stahlval($x).unwrap()
        ), *].into())
    }};

    ( $($x:expr ,)* ) => {{
        $crate::rvals::StahlVal::ListV(im_lists::list![$(
            $crate::rvals::IntoStahlVal::into_stahlval($x).unwrap()
        )*])
    }};
}

use bigdecimal::BigDecimal;
use smallvec::SmallVec;
use StahlVal::*;

use crate::values::{HashMap, HashSet, Vector};

use futures_task::noop_waker_ref;
use futures_util::future::Shared;
use futures_util::FutureExt;

use crate::values::lists::List;
use num::{
    bigint::ToBigInt, BigInt, BigRational, FromPrimitive, Rational32, Signed, ToPrimitive, Zero,
};
use stahl_parser::tokens::{IntLiteral, RealLiteral};

use self::cycles::{CycleDetector, IterativeDropHandler};

pub type RcRefStahlVal = Rc<RefCell<StahlVal>>;
pub fn new_rc_ref_cell(x: StahlVal) -> RcRefStahlVal {
    Rc::new(RefCell::new(x))
}

pub type Result<T> = result::Result<T, StahlErr>;
pub type FunctionSignature = fn(&[StahlVal]) -> Result<StahlVal>;
pub type MutFunctionSignature = fn(&mut [StahlVal]) -> Result<StahlVal>;

#[cfg(not(feature = "sync"))]
pub type BoxedAsyncFunctionSignature =
    crate::gc::Shared<Box<dyn Fn(&[StahlVal]) -> Result<FutureResult>>>;

#[cfg(feature = "sync")]
pub type BoxedAsyncFunctionSignature =
    crate::gc::Shared<Box<dyn Fn(&[StahlVal]) -> Result<FutureResult> + Send + Sync + 'static>>;

pub type AsyncSignature = fn(&[StahlVal]) -> FutureResult;

#[cfg(not(feature = "sync"))]
pub type BoxedFutureResult = Pin<Box<dyn Future<Output = Result<StahlVal>>>>;

#[cfg(feature = "sync")]
pub type BoxedFutureResult = Pin<Box<dyn Future<Output = Result<StahlVal>> + Send + 'static>>;

// TODO: Why can't I put sync here?
// #[cfg(feature = "sync")]
// pub type BoxedFutureResult = Pin<Box<dyn Future<Output = Result<StahlVal>> + Send + 'static>>;

#[derive(Clone)]
pub struct FutureResult(Shared<BoxedFutureResult>);

impl FutureResult {
    pub fn new(fut: BoxedFutureResult) -> Self {
        FutureResult(fut.shared())
    }

    pub fn into_shared(self) -> Shared<BoxedFutureResult> {
        self.0
    }
}

// This is an attempt to one off poll a future
// This should enable us to use embedded async functions
// Will require using call/cc w/ a thread queue in stahl, however it should be possible
pub(crate) fn poll_future(mut fut: Shared<BoxedFutureResult>) -> Option<Result<StahlVal>> {
    // If the future has already been awaited (by somebody) get that value instead
    if let Some(output) = fut.peek() {
        return Some(output.clone());
    }

    // Otherwise, go ahead and poll the value to see if its ready
    // The context is going to exist exclusively in Stahl, hidden behind an `await`
    let waker = noop_waker_ref();
    let context = &mut Context::from_waker(waker);

    // Polling requires a pinned future - TODO make sure this is correct
    let mut_fut = Pin::new(&mut fut);

    match Future::poll(mut_fut, context) {
        std::task::Poll::Ready(r) => Some(r),
        std::task::Poll::Pending => None,
    }
}

/// Attempt to cast this custom type down to the underlying type
pub fn as_underlying_type<T: 'static>(value: &dyn CustomType) -> Option<&T> {
    value.as_any_ref().downcast_ref::<T>()
}

pub fn as_underlying_type_mut<T: 'static>(value: &mut dyn CustomType) -> Option<&mut T> {
    value.as_any_ref_mut().downcast_mut::<T>()
}

pub trait Custom: private::Sealed {
    fn fmt(&self) -> Option<std::result::Result<String, std::fmt::Error>> {
        None
    }

    fn into_serializable_stahlval(&mut self) -> Option<SerializableStahlVal> {
        None
    }

    fn as_iterator(&self) -> Option<Box<dyn Iterator<Item = StahlVal>>> {
        None
    }

    fn gc_drop_mut(&mut self, _drop_handler: &mut IterativeDropHandler) {}

    fn gc_visit_children(&self, _context: &mut MarkAndSweepContext) {}

    fn visit_equality(&self, _visitor: &mut cycles::EqualityVisitor) {}

    fn equality_hint(&self, _other: &dyn CustomType) -> bool {
        true
    }

    fn equality_hint_general(&self, _other: &StahlVal) -> bool {
        false
    }
}

#[cfg(not(feature = "sync"))]
pub trait MaybeSendSyncStatic: 'static {}

#[cfg(not(feature = "sync"))]
impl<T: 'static> MaybeSendSyncStatic for T {}

#[cfg(feature = "sync")]
pub trait MaybeSendSyncStatic: Send + Sync + 'static {}

#[cfg(feature = "sync")]
impl<T: Send + Sync + 'static> MaybeSendSyncStatic for T {}

#[cfg(feature = "sync")]
pub trait CustomType: MaybeSendSyncStatic {
    fn as_any_ref(&self) -> &dyn Any;
    fn as_any_ref_mut(&mut self) -> &mut dyn Any;
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    fn inner_type_id(&self) -> TypeId;
    fn display(&self) -> std::result::Result<String, std::fmt::Error> {
        Ok(format!("#<{}>", self.name().to_string()))
    }
    fn as_serializable_stahlval(&mut self) -> Option<SerializableStahlVal> {
        None
    }
    fn drop_mut(&mut self, _drop_handler: &mut IterativeDropHandler) {}
    fn visit_children(&self, _context: &mut MarkAndSweepContext) {}
    fn visit_children_for_equality(&self, _visitor: &mut cycles::EqualityVisitor) {}
    fn check_equality_hint(&self, _other: &dyn CustomType) -> bool {
        true
    }
    fn check_equality_hint_general(&self, _other: &StahlVal) -> bool {
        false
    }
}

#[cfg(not(feature = "sync"))]
pub trait CustomType {
    fn as_any_ref(&self) -> &dyn Any;
    fn as_any_ref_mut(&mut self) -> &mut dyn Any;
    fn name(&self) -> &str {
        std::any::type_name::<Self>()
    }
    fn inner_type_id(&self) -> TypeId;
    fn display(&self) -> std::result::Result<String, std::fmt::Error> {
        Ok(format!("#<{}>", self.name().to_string()))
    }
    fn as_serializable_stahlval(&mut self) -> Option<SerializableStahlVal> {
        None
    }
    fn drop_mut(&mut self, _drop_handler: &mut IterativeDropHandler) {}
    fn visit_children(&self, _context: &mut MarkAndSweepContext) {}
    fn visit_children_for_equality(&self, _visitor: &mut cycles::EqualityVisitor) {}
    fn check_equality_hint(&self, _other: &dyn CustomType) -> bool {
        true
    }
    fn check_equality_hint_general(&self, _other: &StahlVal) -> bool {
        false
    }
}

impl<T: Custom + MaybeSendSyncStatic> CustomType for T {
    fn as_any_ref(&self) -> &dyn Any {
        self as &dyn Any
    }
    fn as_any_ref_mut(&mut self) -> &mut dyn Any {
        self as &mut dyn Any
    }
    fn inner_type_id(&self) -> TypeId {
        std::any::TypeId::of::<Self>()
    }
    fn display(&self) -> std::result::Result<String, std::fmt::Error> {
        if let Some(formatted) = self.fmt() {
            formatted
        } else {
            Ok(format!("#<{}>", self.name().to_string()))
        }
    }

    fn as_serializable_stahlval(&mut self) -> Option<SerializableStahlVal> {
        <T as Custom>::into_serializable_stahlval(self)
    }

    fn drop_mut(&mut self, drop_handler: &mut IterativeDropHandler) {
        self.gc_drop_mut(drop_handler)
    }

    fn visit_children(&self, context: &mut MarkAndSweepContext) {
        self.gc_visit_children(context)
    }

    // TODO: Equality visitor
    fn visit_children_for_equality(&self, visitor: &mut cycles::EqualityVisitor) {
        self.visit_equality(visitor)
    }

    fn check_equality_hint(&self, other: &dyn CustomType) -> bool {
        self.equality_hint(other)
    }

    fn check_equality_hint_general(&self, other: &StahlVal) -> bool {
        self.equality_hint_general(other)
    }
}

impl<T: CustomType + 'static> IntoStahlVal for T {
    fn into_stahlval(self) -> Result<StahlVal> {
        Ok(StahlVal::Custom(Gc::new_mut(Box::new(self))))
    }
}

pub trait IntoSerializableStahlVal {
    fn into_serializable_stahlval(val: &StahlVal) -> Result<SerializableStahlVal>;
}

impl<T: CustomType + Clone + Send + Sync + 'static> IntoSerializableStahlVal for T {
    fn into_serializable_stahlval(val: &StahlVal) -> Result<SerializableStahlVal> {
        if let StahlVal::Custom(v) = val {
            // let left_type = v.borrow().as_any_ref();
            // TODO: @Matt - dylibs cause issues here, as the underlying type ids are different
            // across workspaces and builds
            let left = v.read().as_any_ref().downcast_ref::<T>().cloned();
            let _lifted = left.ok_or_else(|| {
                let error_message = format!(
                    "Type Mismatch: Type of StahlVal: {:?}, did not match the given type: {}",
                    val,
                    std::any::type_name::<Self>()
                );
                StahlErr::new(ErrorKind::ConversionError, error_message)
            });

            todo!()
        } else {
            let error_message = format!(
                "Type Mismatch: Type of StahlVal: {:?} did not match the given type, expecting opaque struct: {}",
                val,
                std::any::type_name::<Self>()
            );

            Err(StahlErr::new(ErrorKind::ConversionError, error_message))
        }
    }
}

// TODO: Marshalling out of the type could also try to yoink from a native stahl struct.
// If possible, we can try to line the constructor up with the fields
impl<T: CustomType + Clone + 'static> FromStahlVal for T {
    fn from_stahlval(val: &StahlVal) -> Result<Self> {
        if let StahlVal::Custom(v) = val {
            // let left_type = v.borrow().as_any_ref();
            // TODO: @Matt - dylibs cause issues here, as the underlying type ids are different
            // across workspaces and builds
            let left = v.read().as_any_ref().downcast_ref::<T>().cloned();
            left.ok_or_else(|| {
                let error_message = format!(
                    "Type Mismatch: Type of StahlVal: {:?}, did not match the given type: {}",
                    val,
                    std::any::type_name::<Self>()
                );
                StahlErr::new(ErrorKind::ConversionError, error_message)
            })
        } else {
            let error_message = format!(
                "Type Mismatch: Type of StahlVal: {:?} did not match the given type, expecting opaque struct: {}",
                val,
                std::any::type_name::<Self>()
            );

            Err(StahlErr::new(ErrorKind::ConversionError, error_message))
        }
    }
}

/// The entry point for turning values into StahlVals
/// The is implemented for most primitives and collections
/// You can also manually implement this for any type, or can optionally
/// get this implementation for a custom struct by using the custom
/// stahl derive.
pub trait IntoStahlVal: Sized {
    fn into_stahlval(self) -> Result<StahlVal>;
}

/// The exit point for turning StahlVals into outside world values
/// This is implement for most primitives and collections
/// You can also manually implement this for any type, or can optionally
/// get this implementation for a custom struct by using the custom
/// stahl derive.
pub trait FromStahlVal: Sized {
    fn from_stahlval(val: &StahlVal) -> Result<Self>;
}

pub trait PrimitiveAsRef<'a>: Sized {
    fn primitive_as_ref(val: &'a StahlVal) -> Result<Self>;
    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self>;
}

pub trait PrimitiveAsRefMut<'a>: Sized {
    fn primitive_as_ref(val: &'a mut StahlVal) -> Result<Self>;
    fn maybe_primitive_as_ref(val: &'a mut StahlVal) -> Option<Self>;
}

pub struct RestArgsIter<'a, T>(
    pub std::iter::Map<std::slice::Iter<'a, StahlVal>, fn(&'a StahlVal) -> Result<T>>,
);

impl<'a, T: PrimitiveAsRef<'a> + 'a> RestArgsIter<'a, T> {
    pub fn new(
        args: std::iter::Map<std::slice::Iter<'a, StahlVal>, fn(&'a StahlVal) -> Result<T>>,
    ) -> Self {
        RestArgsIter(args)
    }

    pub fn from_slice(args: &'a [StahlVal]) -> Result<Self> {
        Ok(RestArgsIter(args.iter().map(T::primitive_as_ref)))
    }
}

impl<'a, T> Iterator for RestArgsIter<'a, T> {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a, T> ExactSizeIterator for RestArgsIter<'a, T> {}

pub struct RestArgs<T: FromStahlVal>(pub Vec<T>);

impl<T: FromStahlVal> RestArgs<T> {
    pub fn new(args: Vec<T>) -> Self {
        RestArgs(args)
    }

    pub fn from_slice(args: &[StahlVal]) -> Result<Self> {
        args.iter()
            .map(|x| T::from_stahlval(x))
            .collect::<Result<Vec<_>>>()
            .map(RestArgs)
    }
}

impl<T: FromStahlVal> std::ops::Deref for RestArgs<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

mod private {

    use std::any::Any;

    pub trait Sealed {}

    impl<T: Any> Sealed for T {}
}

pub enum SRef<'b, T: ?Sized + 'b> {
    Temporary(&'b T),
    Owned(MappedScopedReadContainer<'b, T>),
}

impl<'b, T: ?Sized + 'b> Deref for SRef<'b, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        match self {
            SRef::Temporary(inner) => inner,
            SRef::Owned(inner) => inner,
        }
    }
}

// Can you take a stahl val and execute operations on it by reference
pub trait AsRefStahlVal: Sized {
    type Nursery: Default;

    fn as_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<SRef<'b, Self>>;
}

pub trait AsSlice<T> {
    fn as_slice_repr(&self) -> &[T];
}

impl<T> AsSlice<T> for Vec<T> {
    fn as_slice_repr(&self) -> &[T] {
        self.as_slice()
    }
}

// TODO: Try to incorporate these all into one trait if possible
pub trait AsRefStahlValFromUnsized<T>: Sized {
    type Output: AsSlice<T>;

    fn as_ref_from_unsized(val: &StahlVal) -> Result<Self::Output>;
}

pub trait AsRefMutStahlVal: Sized {
    fn as_mut_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<MappedScopedWriteContainer<'b, Self>>;
}

pub trait AsRefMutStahlValFromRef: Sized {
    fn as_mut_ref_from_ref<'a>(val: &'a StahlVal) -> crate::rvals::Result<&'a mut Self>;
}

pub trait AsRefStahlValFromRef: Sized {
    fn as_ref_from_ref<'a>(val: &'a StahlVal) -> crate::rvals::Result<&'a Self>;
}

impl AsRefStahlVal for UserDefinedStruct {
    type Nursery = ();

    fn as_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<SRef<'b, Self>> {
        if let StahlVal::CustomStruct(l) = val {
            Ok(SRef::Temporary(l))
        } else {
            stop!(TypeMismatch => "Value cannot be referenced as a list")
        }
    }
}

impl<T: CustomType + MaybeSendSyncStatic> AsRefStahlVal for T {
    type Nursery = ();

    fn as_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<SRef<'b, Self>> {
        if let StahlVal::Custom(v) = val {
            let res = ScopedReadContainer::map(v.read(), |x| x.as_any_ref());

            if res.is::<T>() {
                Ok(SRef::Owned(MappedScopedReadContainer::map(res, |x| {
                    x.downcast_ref::<T>().unwrap()
                })))
            } else {
                let error_message = format!(
                    "Type Mismatch: Type of StahlVal: {} did not match the given type: {}",
                    val,
                    std::any::type_name::<Self>()
                );
                Err(StahlErr::new(ErrorKind::ConversionError, error_message))
            }
            // res
        } else {
            let error_message = format!(
                "Type Mismatch: Type of StahlVal: {} did not match the given type: {}",
                val,
                std::any::type_name::<Self>()
            );

            Err(StahlErr::new(ErrorKind::ConversionError, error_message))
        }
    }
}

impl<T: CustomType + MaybeSendSyncStatic> AsRefMutStahlVal for T {
    fn as_mut_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<MappedScopedWriteContainer<'b, Self>> {
        if let StahlVal::Custom(v) = val {
            let res = ScopedWriteContainer::map(v.write(), |x| x.as_any_ref_mut());

            if res.is::<T>() {
                Ok(MappedScopedWriteContainer::map(res, |x| {
                    x.downcast_mut::<T>().unwrap()
                }))
            } else {
                let error_message = format!(
                    "Type Mismatch: Type of StahlVal: {} did not match the given type: {}",
                    val,
                    std::any::type_name::<Self>()
                );
                Err(StahlErr::new(ErrorKind::ConversionError, error_message))
            }
            // res
        } else {
            let error_message = format!(
                "Type Mismatch: Type of StahlVal: {} did not match the given type: {}",
                val,
                std::any::type_name::<Self>()
            );

            Err(StahlErr::new(ErrorKind::ConversionError, error_message))
        }
    }
}

impl ast::TryFromStahlValVisitorForExprKind {
    pub fn visit_syntax_object(&mut self, value: &Syntax) -> Result<ExprKind> {
        let span = value.span;

        // dbg!(&span);
        // let source = self.source.clone();
        match &value.syntax {
            // Mutual recursion case
            SyntaxObject(s) => self.visit_syntax_object(s),
            BoolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::BooleanLiteral(*x),
                span,
            )))),
            NumV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                RealLiteral::Float(*x).into(),
                span,
            )))),
            IntV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                RealLiteral::Int(IntLiteral::Small(*x)).into(),
                span,
            )))),
            VectorV(lst) => {
                let items: Result<Vec<ExprKind>> = lst.iter().map(|x| self.visit(x)).collect();
                Ok(ExprKind::List(crate::parser::ast::List::new(items?)))
            }
            StringV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::StringLiteral(x.to_arc_string()),
                span,
            )))),
            SymbolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::Identifier(x.as_str().into()),
                span,
            )))),

            ListV(l) => {
                // Rooted - things operate as normal
                if self.qq_depth == 0 {
                    let maybe_special_form = l.first().and_then(|x| {
                        x.as_symbol()
                            .or_else(|| x.as_syntax_object().and_then(|x| x.syntax.as_symbol()))
                    });

                    match maybe_special_form {
                        Some(x) if x.as_str() == "quote" => {
                            if self.quoted {
                                let items: std::result::Result<Vec<ExprKind>, _> =
                                    l.iter().map(|x| self.visit(x)).collect();

                                return Ok(ExprKind::List(ast::List::new(items?)));
                            }

                            self.quoted = true;

                            let return_value = l
                                .into_iter()
                                .map(|x| self.visit(x))
                                .collect::<std::result::Result<Vec<_>, _>>()?
                                .try_into()?;

                            self.quoted = false;

                            return Ok(return_value);
                        } // "quasiquote" => {
                        //     self.qq_depth += 1;
                        // }
                        // None => {
                        // return Ok(ExprKind::empty());
                        // }
                        _ => {}
                    }
                }

                Ok(l.into_iter()
                    .map(|x| self.visit(x))
                    .collect::<std::result::Result<Vec<_>, _>>()?
                    .try_into()?)
            }

            CharV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::CharacterLiteral(*x),
                span,
            )))),
            _ => stop!(ConversionError => "unable to convert {:?} to expression", &value.syntax),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Syntax {
    pub(crate) raw: Option<StahlVal>,
    pub(crate) syntax: StahlVal,
    span: Span,
}

impl Syntax {
    pub fn new(syntax: StahlVal, span: Span) -> Syntax {
        Self {
            raw: None,
            syntax,
            span,
        }
    }

    pub fn proto(raw: StahlVal, syntax: StahlVal, span: Span) -> Syntax {
        Self {
            raw: Some(raw),
            syntax,
            span,
        }
    }

    pub fn syntax_e(&self) -> StahlVal {
        self.syntax.clone()
    }

    pub fn new_with_source(syntax: StahlVal, span: Span) -> Syntax {
        Self {
            raw: None,
            syntax,
            span,
        }
    }

    pub fn syntax_loc(&self) -> Span {
        self.span
    }

    pub fn syntax_datum(&self) -> StahlVal {
        self.raw.clone().unwrap()
    }

    pub(crate) fn stahlval_to_exprkind(value: &StahlVal) -> Result<ExprKind> {
        match value {
            // Mutual recursion case
            SyntaxObject(s) => s.to_exprkind(),
            BoolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                TokenType::BooleanLiteral(*x),
            )))),
            NumV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                RealLiteral::Float(*x).into(),
            )))),
            IntV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                RealLiteral::Int(IntLiteral::Small(*x)).into(),
            )))),
            VectorV(lst) => {
                let items: Result<Vec<ExprKind>> =
                    lst.iter().map(Self::stahlval_to_exprkind).collect();
                Ok(ExprKind::List(crate::parser::ast::List::new(items?)))
            }
            StringV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                TokenType::StringLiteral(x.to_arc_string()),
            )))),
            // LambdaV(_) => Err("Can't convert from Lambda to expression!"),
            // MacroV(_) => Err("Can't convert from Macro to expression!"),
            SymbolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                TokenType::Identifier(x.as_str().into()),
            )))),
            ListV(l) => {
                let items: Result<Vec<ExprKind>> =
                    l.iter().map(Self::stahlval_to_exprkind).collect();

                Ok(ExprKind::List(crate::parser::ast::List::new(items?)))
            }
            CharV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::default(
                TokenType::CharacterLiteral(*x),
            )))),
            _ => stop!(ConversionError => "unable to convert {:?} to expression", value),
        }
    }

    // TODO: match on self.syntax. If its itself a syntax object, then just recur on that until we bottom out
    // Otherwise, reconstruct the ExprKind and replace the span and source information into the representation
    pub fn to_exprkind(&self) -> Result<ExprKind> {
        let span = self.span;
        // let source = self.source.clone();
        match &self.syntax {
            // Mutual recursion case
            SyntaxObject(s) => s.to_exprkind(),
            BoolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::BooleanLiteral(*x),
                span,
            )))),
            NumV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                RealLiteral::Float(*x).into(),
                span,
            )))),
            IntV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                RealLiteral::Int(IntLiteral::Small(*x)).into(),
                span,
            )))),
            VectorV(lst) => {
                let items: Result<Vec<ExprKind>> =
                    lst.iter().map(Self::stahlval_to_exprkind).collect();
                Ok(ExprKind::List(crate::parser::ast::List::new(items?)))
            }
            StringV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::StringLiteral(x.to_arc_string()),
                span,
            )))),
            // LambdaV(_) => Err("Can't convert from Lambda to expression!"),
            // MacroV(_) => Err("Can't convert from Macro to expression!"),
            SymbolV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::Identifier(x.as_str().into()),
                span,
            )))),
            ListV(l) => {
                let items: Result<Vec<ExprKind>> =
                    l.iter().map(Self::stahlval_to_exprkind).collect();

                Ok(ExprKind::List(crate::parser::ast::List::new(items?)))
            }
            CharV(x) => Ok(ExprKind::Atom(Atom::new(SyntaxObject::new(
                TokenType::CharacterLiteral(*x),
                span,
            )))),
            _ => stop!(ConversionError => "unable to convert {:?} to expression", &self.syntax),
        }
    }
}

impl IntoStahlVal for Syntax {
    fn into_stahlval(self) -> Result<StahlVal> {
        Ok(StahlVal::SyntaxObject(Gc::new(self)))
    }
}

impl AsRefStahlVal for Syntax {
    type Nursery = ();

    fn as_ref<'b, 'a: 'b>(val: &'a StahlVal) -> Result<SRef<'b, Self>> {
        if let StahlVal::SyntaxObject(s) = val {
            Ok(SRef::Temporary(s))
        } else {
            stop!(TypeMismatch => "Value cannot be referenced as a syntax object: {}", val)
        }
    }
}

impl From<Syntax> for StahlVal {
    fn from(val: Syntax) -> Self {
        StahlVal::SyntaxObject(Gc::new(val))
    }
}

// TODO:
// This needs to be a method on the runtime: in order to properly support
// threads
// Tracking issue here: https://github.com/TeglonLabs/Stahl/issues/98

// Values which can be sent to another thread.
// If it cannot be sent to another thread, then we'll error out on conversion.
// TODO: Add boxed dyn functions to this.
// #[derive(PartialEq)]
pub enum SerializableStahlVal {
    Closure(crate::values::functions::SerializedLambda),
    BoolV(bool),
    NumV(f64),
    IntV(isize),
    CharV(char),
    Void,
    StringV(String),
    FuncV(FunctionSignature),
    MutFunc(MutFunctionSignature),
    HashMapV(Vec<(SerializableStahlVal, SerializableStahlVal)>),
    ListV(Vec<SerializableStahlVal>),
    Pair(Box<(SerializableStahlVal, SerializableStahlVal)>),
    VectorV(Vec<SerializableStahlVal>),
    ByteVectorV(Vec<u8>),
    BoxedDynFunction(BoxedDynFunction),
    BuiltIn(BuiltInSignature),
    SymbolV(String),
    Custom(Box<dyn CustomType + Send>),
    CustomStruct(SerializableUserDefinedStruct),
    // Attempt to reuse the storage if possible
    HeapAllocated(usize),
    Port(SendablePort),
    Rational(Rational32),
}

pub enum SerializedHeapRef {
    Serialized(Option<SerializableStahlVal>),
    Closed(HeapRef<StahlVal>),
}

pub struct HeapSerializer<'a> {
    pub heap: &'a mut Heap,
    pub fake_heap: &'a mut std::collections::HashMap<usize, SerializedHeapRef>,
    // After the conversion, we go back through, and patch the values from the fake heap
    // in to each of the values listed here - otherwise, we'll miss cycles
    pub values_to_fill_in: &'a mut std::collections::HashMap<usize, HeapRef<StahlVal>>,

    // Cache the functions that get built
    pub built_functions: &'a mut std::collections::HashMap<u32, Gc<ByteCodeLambda>>,
}

// Once crossed over the line, convert BACK into a StahlVal
// This should be infallible.
pub fn from_serializable_value(ctx: &mut HeapSerializer, val: SerializableStahlVal) -> StahlVal {
    match val {
        SerializableStahlVal::Closure(c) => {
            if c.captures.is_empty() {
                if let Some(already_made) = ctx.built_functions.get(&c.id) {
                    StahlVal::Closure(already_made.clone())
                } else {
                    let id = c.id;
                    let value = Gc::new(ByteCodeLambda::from_serialized(ctx, c));

                    // Save those as well
                    // Probably need to just do this for all
                    ctx.built_functions.insert(id, value.clone());
                    StahlVal::Closure(value)
                }
            } else {
                StahlVal::Closure(Gc::new(ByteCodeLambda::from_serialized(ctx, c)))
            }
        }
        SerializableStahlVal::BoolV(b) => StahlVal::BoolV(b),
        SerializableStahlVal::NumV(n) => StahlVal::NumV(n),
        SerializableStahlVal::IntV(i) => StahlVal::IntV(i),
        SerializableStahlVal::CharV(c) => StahlVal::CharV(c),
        SerializableStahlVal::Void => StahlVal::Void,
        SerializableStahlVal::Rational(r) => StahlVal::Rational(r),
        SerializableStahlVal::StringV(s) => StahlVal::StringV(s.into()),
        SerializableStahlVal::FuncV(f) => StahlVal::FuncV(f),
        SerializableStahlVal::MutFunc(f) => StahlVal::MutFunc(f),
        SerializableStahlVal::HashMapV(h) => StahlVal::HashMapV(
            Gc::new(
                h.into_iter()
                    .map(|(k, v)| {
                        (
                            from_serializable_value(ctx, k),
                            from_serializable_value(ctx, v),
                        )
                    })
                    .collect::<HashMap<_, _>>(),
            )
            .into(),
        ),
        SerializableStahlVal::ListV(v) => StahlVal::ListV(
            v.into_iter()
                .map(|x| from_serializable_value(ctx, x))
                .collect(),
        ),
        SerializableStahlVal::VectorV(v) => StahlVal::VectorV(StahlVector(Gc::new(
            v.into_iter()
                .map(|x| from_serializable_value(ctx, x))
                .collect(),
        ))),
        SerializableStahlVal::BoxedDynFunction(f) => StahlVal::BoxedFunction(Gc::new(f)),
        SerializableStahlVal::BuiltIn(f) => StahlVal::BuiltIn(f),
        SerializableStahlVal::SymbolV(s) => StahlVal::SymbolV(s.into()),
        SerializableStahlVal::Custom(b) => StahlVal::Custom(Gc::new_mut(b)),
        SerializableStahlVal::CustomStruct(s) => {
            StahlVal::CustomStruct(Gc::new(UserDefinedStruct {
                fields: {
                    let fields = s
                        .fields
                        .into_iter()
                        .map(|x| from_serializable_value(ctx, x));

                    // fields.collect()

                    // let mut recycle: crate::values::recycler::Recycle<Vec<_>> =
                    //     crate::values::recycler::Recycle::new();

                    let mut recycle: crate::values::recycler::Recycle<SmallVec<_>> =
                        crate::values::recycler::Recycle::new();

                    recycle.extend(fields);

                    recycle
                },
                type_descriptor: s.type_descriptor,
            }))
        }
        SerializableStahlVal::Port(p) => StahlVal::PortV(StahlPort::from_sendable_port(p)),
        SerializableStahlVal::HeapAllocated(v) => {
            // todo!()

            if let Some(mut guard) = ctx.fake_heap.get_mut(&v) {
                match &mut guard {
                    SerializedHeapRef::Serialized(value) => {
                        let value = std::mem::take(value);

                        if let Some(value) = value {
                            let value = from_serializable_value(ctx, value);
                            let allocation = ctx.heap.allocate_without_collection(value);

                            ctx.fake_heap
                                .insert(v, SerializedHeapRef::Closed(allocation.clone()));

                            StahlVal::HeapAllocated(allocation)
                        } else {
                            // println!("If we're getting here - it means the value from the heap has already
                            // been converting. if so, we should do something...");

                            let fake_allocation =
                                ctx.heap.allocate_without_collection(StahlVal::Void);

                            ctx.values_to_fill_in.insert(v, fake_allocation.clone());

                            StahlVal::HeapAllocated(fake_allocation)
                        }
                    }

                    SerializedHeapRef::Closed(c) => StahlVal::HeapAllocated(c.clone()),
                }
            } else {
                // Shouldn't silently fail here, but we will... for now

                let allocation = ctx.heap.allocate_without_collection(StahlVal::Void);

                ctx.fake_heap
                    .insert(v, SerializedHeapRef::Closed(allocation.clone()));

                StahlVal::HeapAllocated(allocation)
            }
        }
        SerializableStahlVal::Pair(pair) => {
            let (car, cdr) = *pair;

            crate::values::lists::Pair::cons(
                from_serializable_value(ctx, car),
                from_serializable_value(ctx, cdr),
            )
            .into()
        }
        SerializableStahlVal::ByteVectorV(bytes) => {
            StahlVal::ByteVector(StahlByteVector::new(bytes))
        }
    }
}

// The serializable value needs to refer to the original heap -
// that way can reference the original stuff easily.

// TODO: Use the cycle detector instead
pub fn into_serializable_value(
    val: StahlVal,
    serialized_heap: &mut std::collections::HashMap<usize, SerializableStahlVal>,
    visited: &mut std::collections::HashSet<usize>,
) -> Result<SerializableStahlVal> {
    // dbg!(&serialized_heap);

    match val {
        StahlVal::Closure(c) => closure_into_serializable(&c, serialized_heap, visited)
            .map(SerializableStahlVal::Closure),
        StahlVal::BoolV(b) => Ok(SerializableStahlVal::BoolV(b)),
        StahlVal::NumV(n) => Ok(SerializableStahlVal::NumV(n)),
        StahlVal::IntV(n) => Ok(SerializableStahlVal::IntV(n)),
        StahlVal::CharV(c) => Ok(SerializableStahlVal::CharV(c)),
        StahlVal::Void => Ok(SerializableStahlVal::Void),
        StahlVal::StringV(s) => Ok(SerializableStahlVal::StringV(s.to_string())),
        StahlVal::FuncV(f) => Ok(SerializableStahlVal::FuncV(f)),
        StahlVal::ListV(l) => Ok(SerializableStahlVal::ListV(
            l.into_iter()
                .map(|x| into_serializable_value(x, serialized_heap, visited))
                .collect::<Result<_>>()?,
        )),
        StahlVal::Pair(pair) => Ok(SerializableStahlVal::Pair(Box::new((
            into_serializable_value(pair.car.clone(), serialized_heap, visited)?,
            into_serializable_value(pair.cdr.clone(), serialized_heap, visited)?,
        )))),
        StahlVal::BoxedFunction(f) => Ok(SerializableStahlVal::BoxedDynFunction((*f).clone())),
        StahlVal::BuiltIn(f) => Ok(SerializableStahlVal::BuiltIn(f)),
        StahlVal::SymbolV(s) => Ok(SerializableStahlVal::SymbolV(s.to_string())),
        StahlVal::MutFunc(f) => Ok(SerializableStahlVal::MutFunc(f)),
        StahlVal::HashMapV(v) => Ok(SerializableStahlVal::HashMapV(
            v.0.unwrap()
                .into_iter()
                .map(|(k, v)| {
                    let kprime = into_serializable_value(k, serialized_heap, visited)?;
                    let vprime = into_serializable_value(v, serialized_heap, visited)?;

                    Ok((kprime, vprime))
                })
                .collect::<Result<_>>()?,
        )),

        StahlVal::Custom(c) => {
            if let Some(output) = c.write().as_serializable_stahlval() {
                Ok(output)
            } else {
                stop!(Generic => "Custom type not allowed to be moved across threads!")
            }
        }

        StahlVal::CustomStruct(s) => Ok(SerializableStahlVal::CustomStruct(
            SerializableUserDefinedStruct {
                fields: s
                    .fields
                    .iter()
                    .cloned()
                    .map(|x| into_serializable_value(x, serialized_heap, visited))
                    .collect::<Result<Vec<_>>>()?,
                type_descriptor: s.type_descriptor,
            },
        )),

        StahlVal::PortV(p) => SendablePort::from_port(p).map(SerializableStahlVal::Port),

        // If there is a cycle, this could cause problems?
        StahlVal::HeapAllocated(h) => {
            // We should pick it up on the way back the recursion
            if visited.contains(&h.as_ptr_usize())
                && !serialized_heap.contains_key(&h.as_ptr_usize())
            {
                // println!("Already visited: {}", h.as_ptr_usize());

                Ok(SerializableStahlVal::HeapAllocated(h.as_ptr_usize()))
            } else {
                visited.insert(h.as_ptr_usize());

                if serialized_heap.contains_key(&h.as_ptr_usize()) {
                    // println!("Already exists in map: {}", h.as_ptr_usize());

                    Ok(SerializableStahlVal::HeapAllocated(h.as_ptr_usize()))
                } else {
                    // println!("Trying to insert: {} @ {}", h.get(), h.as_ptr_usize());

                    let value = into_serializable_value(h.get(), serialized_heap, visited);

                    let value = match value {
                        Ok(v) => v,
                        Err(e) => {
                            // println!("{}", e);
                            return Err(e);
                        }
                    };

                    serialized_heap.insert(h.as_ptr_usize(), value);

                    // println!("Inserting: {}", h.as_ptr_usize());

                    Ok(SerializableStahlVal::HeapAllocated(h.as_ptr_usize()))
                }
            }
        }

        StahlVal::VectorV(vector) => Ok(SerializableStahlVal::VectorV(
            vector
                .iter()
                .cloned()
                .map(|val| into_serializable_value(val, serialized_heap, visited))
                .collect::<Result<_>>()?,
        )),

        StahlVal::ByteVector(bytes) => {
            Ok(SerializableStahlVal::ByteVectorV(bytes.vec.read().clone()))
        }

        StahlVal::Rational(r) => Ok(SerializableStahlVal::Rational(r)),

        illegal => stop!(Generic => "Type not allowed to be moved across threads!: {}", illegal),
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct StahlMutableVector(pub(crate) Gc<RefCell<Vec<StahlVal>>>);

#[derive(Clone, PartialEq, Eq)]
pub struct StahlVector(pub(crate) Gc<Vector<StahlVal>>);

impl Deref for StahlVector {
    type Target = Vector<StahlVal>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Gc<Vector<StahlVal>>> for StahlVector {
    fn from(value: Gc<Vector<StahlVal>>) -> Self {
        StahlVector(value)
    }
}

#[derive(Clone, PartialEq)]
pub struct StahlHashMap(pub(crate) Gc<HashMap<StahlVal, StahlVal>>);

impl Deref for StahlHashMap {
    type Target = HashMap<StahlVal, StahlVal>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Gc<HashMap<StahlVal, StahlVal>>> for StahlHashMap {
    fn from(value: Gc<HashMap<StahlVal, StahlVal>>) -> Self {
        StahlHashMap(value)
    }
}

#[derive(Clone, PartialEq)]
pub struct StahlHashSet(pub(crate) Gc<HashSet<StahlVal>>);

impl Deref for StahlHashSet {
    type Target = HashSet<StahlVal>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<Gc<HashSet<StahlVal>>> for StahlHashSet {
    fn from(value: Gc<HashSet<StahlVal>>) -> Self {
        StahlHashSet(value)
    }
}

pub enum TypeKind {
    Any,
    Bool,
    Num,
    Int,
    Char,
    Vector(Box<TypeKind>),
    Void,
    String,
    Function,
    HashMap(Box<TypeKind>, Box<TypeKind>),
    HashSet(Box<TypeKind>),
    List(Box<TypeKind>),
}

/// A value as represented in the runtime.
#[derive(Clone)]
pub enum StahlVal {
    /// Represents a bytecode closure.
    Closure(Gc<ByteCodeLambda>),
    /// Represents a boolean value.
    BoolV(bool),
    /// Represents a number, currently only f64 numbers are supported.
    NumV(f64),
    /// Represents an integer.
    IntV(isize),
    /// Represents a rational number.
    Rational(Rational32),
    /// Represents a character type
    CharV(char),
    /// Vectors are represented as `im_rc::Vector`'s, which are immutable
    /// data structures
    VectorV(StahlVector),
    /// Void return value
    Void,
    /// Represents strings
    StringV(StahlString),
    /// Represents built in rust functions
    FuncV(FunctionSignature),
    /// Represents a symbol, internally represented as `String`s
    SymbolV(StahlString),
    /// Container for a type that implements the `Custom Type` trait. (trait object)
    Custom(GcMut<Box<dyn CustomType>>), // TODO: @Matt - consider using just a mutex here, to relax some of the bounds?
    // Embedded HashMap
    HashMapV(StahlHashMap),
    // Embedded HashSet
    HashSetV(StahlHashSet),
    /// Represents a scheme-only struct
    CustomStruct(Gc<UserDefinedStruct>),
    /// Represents a port object
    PortV(StahlPort),
    /// Generic iterator wrapper
    IterV(Gc<Transducer>),
    /// Reducers
    ReducerV(Gc<Reducer>),
    /// Async Function wrapper
    FutureFunc(BoxedAsyncFunctionSignature),
    // Boxed Future Result
    FutureV(Gc<FutureResult>),
    // A stream of `StahlVal`.
    StreamV(Gc<LazyStream>),
    /// Custom closure
    BoxedFunction(Gc<BoxedDynFunction>),
    // Continuation
    ContinuationFunction(Continuation),
    // Function Pointer
    // #[cfg(feature = "jit")]
    // CompiledFunction(Box<JitFunctionPointer>),
    // List
    ListV(crate::values::lists::List<StahlVal>),
    // Holds a pair that contains 2 `StahlVal`.
    Pair(Gc<crate::values::lists::Pair>),
    // Mutable functions
    MutFunc(MutFunctionSignature),
    // Built in functions
    BuiltIn(BuiltInSignature),
    // Mutable vector
    MutableVector(HeapRef<Vec<StahlVal>>),
    // This should delegate to the underlying iterator - can allow for faster raw iteration if possible
    // Should allow for polling just a raw "next" on underlying elements
    BoxedIterator(GcMut<OpaqueIterator>),
    // Contains a syntax object.
    SyntaxObject(Gc<Syntax>),
    // Mutable storage, with Gc backing
    // Boxed(HeapRef),
    Boxed(GcMut<StahlVal>),
    // Holds a StahlVal on the heap.
    HeapAllocated(HeapRef<StahlVal>),
    // TODO: This itself, needs to be boxed unfortunately.
    Reference(Gc<OpaqueReference<'static>>),
    // Like IntV but supports larger values.
    BigNum(Gc<BigInt>),
    // Like Rational but supports larger numerators and denominators.
    BigRational(Gc<BigRational>),
    // A complex number.
    Complex(Gc<StahlComplex>),
    // Byte vectors
    ByteVector(StahlByteVector),
}

#[cfg(feature = "sync")]
#[test]
fn check_send_sync() {
    let value = StahlVal::IntV(10);

    let handle = std::thread::spawn(move || value);

    handle.join().unwrap();
}

#[derive(Clone)]
pub struct StahlByteVector {
    pub(crate) vec: GcMut<Vec<u8>>,
}

impl StahlByteVector {
    pub fn new(vec: Vec<u8>) -> Self {
        Self {
            vec: Gc::new_mut(vec),
        }
    }
}

impl PartialEq for StahlByteVector {
    fn eq(&self, other: &Self) -> bool {
        *(self.vec.read()) == *(other.vec.read())
    }
}

impl Eq for StahlByteVector {}

impl Hash for StahlByteVector {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.vec.read().hash(state);
    }
}

/// Contains a complex number.
///
/// TODO: Optimize the contents of complex value. Holding `StahlVal` makes it easier to use existing
/// operations but a more specialized representation may be faster.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct StahlComplex {
    /// The real part of the complex number.
    pub re: StahlVal,
    /// The imaginary part of the complex number.
    pub im: StahlVal,
}

impl StahlComplex {
    pub fn new(real: StahlVal, imaginary: StahlVal) -> StahlComplex {
        StahlComplex {
            re: real,
            im: imaginary,
        }
    }

    /// Returns `true` if the imaginary part is negative.
    fn imaginary_is_negative(&self) -> bool {
        match &self.im {
            NumV(x) => x.is_negative(),
            IntV(x) => x.is_negative(),
            Rational(x) => x.is_negative(),
            BigNum(x) => x.is_negative(),
            StahlVal::BigRational(x) => x.is_negative(),
            _ => unreachable!(),
        }
    }
}

impl IntoStahlVal for StahlComplex {
    #[inline(always)]
    fn into_stahlval(self) -> Result<StahlVal> {
        Ok(match self.im {
            NumV(n) if n.is_zero() => self.re,
            IntV(0) => self.re,
            _ => StahlVal::Complex(Gc::new(self)),
        })
    }
}

impl fmt::Display for StahlComplex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.imaginary_is_negative() {
            write!(f, "{re}{im}i", re = self.re, im = self.im)
        } else {
            write!(f, "{re}+{im}i", re = self.re, im = self.im)
        }
    }
}

impl StahlVal {
    // TODO: Re-evaluate this - should this be buffered?
    pub fn new_dyn_writer_port(port: impl Write + Send + Sync + 'static) -> StahlVal {
        StahlVal::PortV(StahlPort {
            port: Gc::new_mut(StahlPortRepr::DynWriter(Arc::new(Mutex::new(port)))),
        })
    }

    pub fn anonymous_boxed_function(
        function: std::sync::Arc<
            dyn Fn(&[StahlVal]) -> crate::rvals::Result<StahlVal> + Send + Sync + 'static,
        >,
    ) -> StahlVal {
        StahlVal::BoxedFunction(Gc::new(BoxedDynFunction {
            function,
            name: None,
            arity: None,
        }))
    }

    pub fn as_box(&self) -> Option<HeapRef<StahlVal>> {
        if let StahlVal::HeapAllocated(heap_ref) = self {
            Some(heap_ref.clone())
        } else {
            None
        }
    }

    pub fn as_box_to_inner(&self) -> Option<StahlVal> {
        self.as_box().map(|x| x.get())
    }

    pub fn as_ptr_usize(&self) -> Option<usize> {
        match self {
            // Closure(_) => todo!(),
            // BoolV(_) => todo!(),
            // NumV(_) => todo!(),
            // IntV(_) => todo!(),
            // CharV(_) => todo!(),
            // VectorV(_) => todo!(),
            // Void => todo!(),
            // StringV(_) => todo!(),
            // FuncV(_) => todo!(),
            // SymbolV(_) => todo!(),
            // StahlVal::Custom(_) => todo!(),
            // HashMapV(_) => todo!(),
            // HashSetV(_) => todo!(),
            CustomStruct(c) => Some(c.as_ptr() as usize),
            // PortV(_) => todo!(),
            // IterV(_) => todo!(),
            // ReducerV(_) => todo!(),
            // FutureFunc(_) => todo!(),
            // FutureV(_) => todo!(),
            // StreamV(_) => todo!(),
            // BoxedFunction(_) => todo!(),
            // ContinuationFunction(_) => todo!(),
            ListV(l) => Some(l.as_ptr_usize()),
            // MutFunc(_) => todo!(),
            // BuiltIn(_) => todo!(),
            // MutableVector(_) => todo!(),
            // BoxedIterator(_) => todo!(),
            // StahlVal::SyntaxObject(_) => todo!(),
            // Boxed(_) => todo!(),
            HeapAllocated(h) => Some(h.as_ptr_usize()),
            // Reference(_) => todo!(),
            // BigNum(_) => todo!(),
            _ => None,
        }
    }

    // pub(crate) fn children_mut<'a>(&'a mut self) -> impl IntoIterator<Item = StahlVal> {
    //     match self {
    //         Self::CustomStruct(inner) => {
    //             if let Some(inner) = inner.get_mut() {
    //                 std::mem::take(&mut inner.borrow_mut().fields)
    //             } else {
    //                 std::iter::empty()
    //             }
    //         }
    //         _ => todo!(),
    //     }
    // }
}

// TODO: Consider unboxed value types, for optimized usages when compiling segments of code.
// If we can infer the types from the concrete functions used, we don't need to have unboxed values -> We also
// can use concrete forms of the underlying functions as well.
// #[derive(Clone)]
// pub enum UnboxedStahlVal {
//     /// Represents a boolean value
//     BoolV(bool),
//     /// Represents a number, currently only f64 numbers are supported
//     NumV(f64),
//     /// Represents an integer
//     IntV(isize),
//     /// Represents a character type
//     CharV(char),
//     /// Vectors are represented as `im_rc::Vector`'s, which are immutable
//     /// data structures
//     VectorV(Vector<StahlVal>),
//     /// Void return value
//     Void,
//     /// Represents strings
//     StringV(StahlString),
//     /// Represents built in rust functions
//     FuncV(FunctionSignature),
//     /// Represents a symbol, internally represented as `String`s
//     SymbolV(StahlString),
//     /// Container for a type that implements the `Custom Type` trait. (trait object)
//     Custom(Gc<RefCell<Box<dyn CustomType>>>),
//     // Embedded HashMap
//     HashMapV(HashMap<StahlVal, StahlVal>),
//     // Embedded HashSet
//     HashSetV(HashSet<StahlVal>),
//     /// Represents a scheme-only struct
//     // StructV(Gc<StahlStruct>),
//     /// Alternative implementation of a scheme-only struct
//     CustomStruct(Gc<RefCell<UserDefinedStruct>>),
//     // Represents a special rust closure
//     // StructClosureV(Box<StahlStruct>, StructClosureSignature),
//     // StructClosureV(Box<StructClosure>),
//     /// Represents a port object
//     PortV(StahlPort),
//     /// Represents a bytecode closure
//     Closure(Gc<ByteCodeLambda>),
//     /// Generic iterator wrapper
//     IterV(Gc<Transducer>),
//     /// Reducers
//     ReducerV(Gc<Reducer>),
//     // Reducer(Reducer)
//     // Generic IntoIter wrapper
//     // Promise(Gc<StahlVal>),
//     /// Async Function wrapper
//     FutureFunc(BoxedAsyncFunctionSignature),
//     // Boxed Future Result
//     FutureV(Gc<FutureResult>),

//     StreamV(Gc<LazyStream>),
//     // Break the cycle somehow
//     // EvaluationEnv(Weak<RefCell<Env>>),
//     /// Contract
//     Contract(Gc<ContractType>),
//     /// Contracted Function
//     ContractedFunction(Gc<ContractedFunction>),
//     /// Custom closure
//     BoxedFunction(BoxedFunctionSignature),
//     // Continuation
//     ContinuationFunction(Gc<Continuation>),
//     // List
//     ListV(List<StahlVal>),
//     // Mutable functions
//     MutFunc(MutFunctionSignature),
//     // Built in functions
//     BuiltIn(BuiltInSignature),
//     // Mutable vector
//     MutableVector(Gc<RefCell<Vec<StahlVal>>>),
//     // This should delegate to the underlying iterator - can allow for faster raw iteration if possible
//     // Should allow for polling just a raw "next" on underlying elements
//     BoxedIterator(Gc<RefCell<BuiltInDataStructureIterator>>),

//     SyntaxObject(Gc<Syntax>),

//     // Mutable storage, with Gc backing
//     Boxed(HeapRef),
// }

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct StahlString(Gc<String>);

impl Deref for StahlString {
    type Target = crate::gc::Shared<String>;

    fn deref(&self) -> &Self::Target {
        &self.0 .0
    }
}

#[cfg(not(feature = "sync"))]
impl From<Arc<String>> for StahlString {
    fn from(value: Arc<String>) -> Self {
        StahlString(Gc(Rc::new((*value).clone())))
    }
}

impl StahlString {
    pub(crate) fn to_arc_string(&self) -> Arc<String> {
        #[cfg(feature = "sync")]
        {
            self.0 .0.clone()
        }
        #[cfg(not(feature = "sync"))]
        Arc::new(self.0.unwrap())
    }
}

impl From<&str> for StahlString {
    fn from(val: &str) -> Self {
        StahlString(Gc::new(val.to_string()))
    }
}

impl From<&String> for StahlString {
    fn from(val: &String) -> Self {
        StahlString(Gc::new(val.to_owned()))
    }
}

impl From<String> for StahlString {
    fn from(val: String) -> Self {
        StahlString(Gc::new(val))
    }
}

impl From<crate::gc::Shared<String>> for StahlString {
    fn from(val: crate::gc::Shared<String>) -> Self {
        StahlString(Gc(val))
    }
}

impl From<Gc<String>> for StahlString {
    fn from(val: Gc<String>) -> Self {
        StahlString(val)
    }
}

impl From<StahlString> for crate::gc::Shared<String> {
    fn from(value: StahlString) -> Self {
        value.0 .0
    }
}

impl From<StahlString> for Gc<String> {
    fn from(value: StahlString) -> Self {
        value.0
    }
}

impl std::fmt::Display for StahlString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

impl std::fmt::Debug for StahlString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0.as_str())
    }
}

// Check that stahl values aren't growing without us knowing
const _ASSERT_SMALL: () = assert!(std::mem::size_of::<StahlVal>() <= 16);

#[test]
fn check_size_of_stahlval() {
    assert_eq!(std::mem::size_of::<StahlVal>(), 16);
}

pub struct Chunks {
    remaining: IntoIter<char>,
}

impl Chunks {
    fn new(s: StahlString) -> Self {
        Chunks {
            remaining: s.chars().collect::<Vec<_>>().into_iter(),
        }
    }
}

pub struct OpaqueIterator {
    pub(crate) root: StahlVal,
    iterator: BuiltInDataStructureIterator,
}

impl Custom for OpaqueIterator {
    fn fmt(&self) -> Option<std::result::Result<String, std::fmt::Error>> {
        Some(Ok(format!("#<iterator>")))
    }
}

// TODO: Convert this to just a generic custom type. This does not have to be
// a special enum variant.
pub enum BuiltInDataStructureIterator {
    List(crate::values::lists::ConsumingIterator<StahlVal>),
    Vector(VectorConsumingIter<StahlVal>),
    Set(HashSetConsumingIter<StahlVal>),
    Map(HashMapConsumingIter<StahlVal, StahlVal>),
    String(Chunks),
    #[cfg(not(feature = "sync"))]
    Opaque(Box<dyn Iterator<Item = StahlVal>>),
    #[cfg(feature = "sync")]
    Opaque(Box<dyn Iterator<Item = StahlVal> + Send + Sync + 'static>),
}

impl BuiltInDataStructureIterator {
    pub fn into_boxed_iterator(self, value: StahlVal) -> StahlVal {
        StahlVal::BoxedIterator(Gc::new_mut(OpaqueIterator {
            root: value,
            iterator: self,
        }))
    }
}

impl BuiltInDataStructureIterator {
    pub fn from_iterator<
        T: IntoStahlVal + MaybeSendSyncStatic,
        I: Iterator<Item = T> + MaybeSendSyncStatic,
        S: IntoIterator<Item = T, IntoIter = I> + MaybeSendSyncStatic,
    >(
        value: S,
    ) -> Self {
        Self::Opaque(Box::new(
            value
                .into_iter()
                .map(|x| x.into_stahlval().expect("This shouldn't fail!")),
        ))
    }
}

impl Iterator for BuiltInDataStructureIterator {
    type Item = StahlVal;

    fn next(&mut self) -> Option<StahlVal> {
        match self {
            Self::List(l) => l.next(),
            Self::Vector(v) => v.next(),
            Self::String(s) => s.remaining.next().map(StahlVal::CharV),
            Self::Set(s) => s.next(),
            Self::Map(s) => s.next().map(|x| StahlVal::ListV(vec![x.0, x.1].into())),
            Self::Opaque(s) => s.next(),
        }
    }
}

pub fn value_into_iterator(val: StahlVal) -> Option<StahlVal> {
    let root = val.clone();
    match val {
        StahlVal::ListV(l) => Some(BuiltInDataStructureIterator::List(l.into_iter())),
        StahlVal::VectorV(v) => Some(BuiltInDataStructureIterator::Vector(
            (*v).clone().into_iter(),
        )),
        StahlVal::StringV(s) => Some(BuiltInDataStructureIterator::String(Chunks::new(s))),
        StahlVal::HashSetV(s) => Some(BuiltInDataStructureIterator::Set((*s).clone().into_iter())),
        StahlVal::HashMapV(m) => Some(BuiltInDataStructureIterator::Map((*m).clone().into_iter())),
        _ => None,
    }
    .map(|iterator| BuiltInDataStructureIterator::into_boxed_iterator(iterator, root))
}

thread_local! {
    pub static ITERATOR_FINISHED: StahlVal = StahlVal::SymbolV("done".into());
}

pub fn iterator_next(args: &[StahlVal]) -> Result<StahlVal> {
    match &args[0] {
        StahlVal::BoxedIterator(b) => match b.write().iterator.next() {
            Some(v) => Ok(v),
            None => Ok(ITERATOR_FINISHED.with(|x| x.clone())),
        },
        _ => stop!(TypeMismatch => "Unexpected argument"),
    }
}

impl StahlVal {
    pub fn boxed(value: StahlVal) -> StahlVal {
        StahlVal::Boxed(Gc::new_mut(value))
    }

    pub(crate) fn ptr_eq(&self, other: &StahlVal) -> bool {
        match (self, other) {
            // Integers are a special case of ptr eq -> if integers are equal? they are also eq?
            (IntV(l), IntV(r)) => l == r,
            (NumV(l), NumV(r)) => l == r,
            (BoolV(l), BoolV(r)) => l == r,
            (CharV(l), CharV(r)) => l == r,
            (VectorV(l), VectorV(r)) => Gc::ptr_eq(&l.0, &r.0),
            (Void, Void) => true,
            (StringV(l), StringV(r)) => crate::gc::Shared::ptr_eq(l, r),
            (FuncV(l), FuncV(r)) => *l as usize == *r as usize,
            (SymbolV(l), SymbolV(r)) => crate::gc::Shared::ptr_eq(l, r),
            (StahlVal::Custom(l), StahlVal::Custom(r)) => Gc::ptr_eq(l, r),
            (HashMapV(l), HashMapV(r)) => Gc::ptr_eq(&l.0, &r.0),
            (HashSetV(l), HashSetV(r)) => Gc::ptr_eq(&l.0, &r.0),
            (PortV(l), PortV(r)) => Gc::ptr_eq(&l.port, &r.port),
            (Closure(l), Closure(r)) => Gc::ptr_eq(l, r),
            (IterV(l), IterV(r)) => Gc::ptr_eq(l, r),
            (ReducerV(l), ReducerV(r)) => Gc::ptr_eq(l, r),
            #[allow(clippy::vtable_address_comparisons)]
            (FutureFunc(l), FutureFunc(r)) => crate::gc::Shared::ptr_eq(l, r),
            (FutureV(l), FutureV(r)) => Gc::ptr_eq(l, r),
            (StreamV(l), StreamV(r)) => Gc::ptr_eq(l, r),
            (BoxedFunction(l), BoxedFunction(r)) => Gc::ptr_eq(l, r),
            (ContinuationFunction(l), ContinuationFunction(r)) => Continuation::ptr_eq(l, r),
            (ListV(l), ListV(r)) => {
                l.ptr_eq(r) || l.storage_ptr_eq(r) || l.is_empty() && r.is_empty()
            }
            (MutFunc(l), MutFunc(r)) => *l as usize == *r as usize,
            (BuiltIn(l), BuiltIn(r)) => *l as usize == *r as usize,
            (MutableVector(l), MutableVector(r)) => HeapRef::ptr_eq(l, r),
            (BigNum(l), BigNum(r)) => Gc::ptr_eq(l, r),
            (ByteVector(l), ByteVector(r)) => Gc::ptr_eq(&l.vec, &r.vec),
            (_, _) => false,
        }
    }
}

impl Hash for StahlVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            BoolV(b) => b.hash(state),
            NumV(n) => n.to_string().hash(state),
            IntV(i) => i.hash(state),
            Rational(f) => f.hash(state),
            BigNum(n) => n.hash(state),
            BigRational(f) => f.hash(state),
            Complex(x) => x.hash(state),
            CharV(c) => c.hash(state),
            ListV(l) => l.hash(state),
            CustomStruct(s) => s.hash(state),
            VectorV(v) => v.hash(state),
            v @ Void => v.hash(state),
            StringV(s) => s.hash(state),
            FuncV(s) => (*s as *const FunctionSignature).hash(state),
            SymbolV(sym) => {
                "symbol".hash(state);
                sym.hash(state);
            }
            Closure(b) => b.hash(state),
            HashMapV(hm) => hm.hash(state),
            IterV(s) => s.hash(state),
            HashSetV(hs) => hs.hash(state),
            SyntaxObject(s) => s.raw.hash(state),
            Pair(p) => (&**p).hash(state),
            ByteVector(v) => (&*v).hash(state),
            _ => unimplemented!("Attempted to hash unsupported value: {self:?}"),
        }
    }
}

impl StahlVal {
    #[inline(always)]
    pub fn is_truthy(&self) -> bool {
        match &self {
            StahlVal::BoolV(false) => false,
            _ => true,
        }
    }

    #[inline(always)]
    pub fn is_future(&self) -> bool {
        matches!(self, StahlVal::FutureV(_))
    }

    pub fn is_hashable(&self) -> bool {
        matches!(
            self,
            BoolV(_)
                | IntV(_)
                | CharV(_)
                // | Pair(_)
                | VectorV(_)
                | StringV(_)
                | SymbolV(_)
                | HashMapV(_)
                | Closure(_)
                | ListV(_)
                | FuncV(_)
                | CustomStruct(_)
        )
    }

    pub fn is_function(&self) -> bool {
        matches!(
            self,
            BoxedFunction(_)
                | Closure(_)
                | FuncV(_)
                // | ContractedFunction(_)
                | BuiltIn(_)
                | MutFunc(_)
        )
    }

    // pub fn is_contract(&self) -> bool {
    //     matches!(self, Contract(_))
    // }

    pub fn empty_hashmap() -> StahlVal {
        StahlVal::HashMapV(Gc::new(HashMap::new()).into())
    }
}

impl StahlVal {
    // pub fn res_iterator

    pub fn list_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<&List<StahlVal>, E> {
        match self {
            Self::ListV(v) => Ok(v),
            _ => Err(err()),
        }
    }

    pub fn list(&self) -> Option<&List<StahlVal>> {
        match self {
            Self::ListV(l) => Some(l),
            _ => None,
        }
    }

    pub fn pair(&self) -> Option<&Gc<crate::values::lists::Pair>> {
        match self {
            Self::Pair(p) => Some(p),
            _ => None,
        }
    }

    pub fn bool_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<bool, E> {
        match self {
            Self::BoolV(v) => Ok(*v),
            _ => Err(err()),
        }
    }

    pub fn int_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<isize, E> {
        match self {
            Self::IntV(v) => Ok(*v),
            _ => Err(err()),
        }
    }

    pub fn num_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<f64, E> {
        match self {
            Self::NumV(v) => Ok(*v),
            _ => Err(err()),
        }
    }

    pub fn char_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<char, E> {
        match self {
            Self::CharV(v) => Ok(*v),
            _ => Err(err()),
        }
    }

    /// Vector does copy on the value to return
    pub fn vector_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<Vector<StahlVal>, E> {
        match self {
            Self::VectorV(v) => Ok(v.0.unwrap()),
            _ => Err(err()),
        }
    }

    pub fn void_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<(), E> {
        match self {
            Self::Void => Ok(()),
            _ => Err(err()),
        }
    }

    pub fn string_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<&str, E> {
        match self {
            Self::StringV(v) => Ok(v),
            _ => Err(err()),
        }
    }

    pub fn func_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<&FunctionSignature, E> {
        match self {
            Self::FuncV(v) => Ok(v),
            _ => Err(err()),
        }
    }

    pub fn boxed_func_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<&BoxedDynFunction, E> {
        match self {
            Self::BoxedFunction(v) => Ok(v),
            _ => Err(err()),
        }
    }

    // pub fn contract_or_else<E, F: FnOnce() -> E>(
    //     &self,
    //     err: F,
    // ) -> std::result::Result<Gc<ContractType>, E> {
    //     match self {
    //         Self::Contract(c) => Ok(c.clone()),
    //         _ => Err(err()),
    //     }
    // }

    pub fn closure_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<Gc<ByteCodeLambda>, E> {
        match self {
            Self::Closure(c) => Ok(c.clone()),
            _ => Err(err()),
        }
    }

    pub fn symbol_or_else<E, F: FnOnce() -> E>(&self, err: F) -> std::result::Result<&str, E> {
        match self {
            Self::SymbolV(v) => Ok(v),
            _ => Err(err()),
        }
    }

    pub fn clone_symbol_or_else<E, F: FnOnce() -> E>(
        &self,
        err: F,
    ) -> std::result::Result<String, E> {
        match self {
            Self::SymbolV(v) => Ok(v.to_string()),
            _ => Err(err()),
        }
    }

    pub fn as_isize(&self) -> Option<isize> {
        match self {
            Self::IntV(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        self.as_isize()
            .and_then(|x| if x >= 0 { Some(x as usize) } else { None })
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::BoolV(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_future(&self) -> Option<Shared<BoxedFutureResult>> {
        match self {
            Self::FutureV(v) => Some(v.clone().unwrap().into_shared()),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&StahlString> {
        match self {
            Self::StringV(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<&StahlString> {
        match self {
            Self::SymbolV(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_syntax_object(&self) -> Option<&Syntax> {
        match self {
            Self::SyntaxObject(s) => Some(s),
            _ => None,
        }
    }

    // pub fn custom_or_else<E, F: FnOnce() -> E>(
    //     &self,
    //     err: F,
    // ) -> std::result::Result<&Box<dyn CustomType>, E> {
    //     match self {
    //         Self::Custom(v) => Ok(&v),
    //         _ => Err(err()),
    //     }
    // }

    // pub fn struct_or_else<E, F: FnOnce() -> E>(
    //     &self,
    //     err: F,
    // ) -> std::result::Result<&StahlStruct, E> {
    //     match self {
    //         Self::StructV(v) => Ok(v),
    //         _ => Err(err()),
    //     }
    // }

    pub fn closure_arity(&self) -> Option<usize> {
        if let StahlVal::Closure(c) = self {
            Some(c.arity())
        } else {
            None
        }
    }
}

impl StahlVal {
    pub const INT_ZERO: StahlVal = StahlVal::IntV(0);
    pub const INT_ONE: StahlVal = StahlVal::IntV(1);
    pub const INT_TWO: StahlVal = StahlVal::IntV(2);
}

impl Eq for StahlVal {}

fn integer_float_equality(int: isize, float: f64) -> bool {
    let converted = float as isize;

    if float == converted as f64 {
        int == converted
    } else {
        false
    }
}

fn bignum_float_equality(bigint: &Gc<num::BigInt>, float: f64) -> bool {
    if float.fract() == 0.0 {
        if let Some(promoted) = bigint.to_f64() {
            promoted == float
        } else {
            false
        }
    } else {
        false
    }
}

#[stahl_derive::function(name = "=", constant = true)]
pub fn number_equality(left: &StahlVal, right: &StahlVal) -> Result<StahlVal> {
    let result = match (left, right) {
        (IntV(l), IntV(r)) => l == r,
        (NumV(l), NumV(r)) => l == r,
        (IntV(l), NumV(r)) | (NumV(r), IntV(l)) => integer_float_equality(*l, *r),
        (Rational(l), Rational(r)) => l == r,
        (Rational(l), NumV(r)) | (NumV(r), Rational(l)) => l.to_f64().unwrap() == *r,
        (BigNum(l), BigNum(r)) => l == r,
        (BigNum(l), NumV(r)) | (NumV(r), BigNum(l)) => bignum_float_equality(l, *r),
        (BigRational(l), BigRational(r)) => l == r,
        (BigRational(l), NumV(r)) | (NumV(r), BigRational(l)) => l.to_f64().unwrap() == *r,
        // The below should be impossible as integers/bignums freely convert into each
        // other. Similar for int/bignum/rational/bigrational.
        (Rational(_), IntV(_))
        | (IntV(_), Rational(_))
        | (Rational(_), BigNum(_))
        | (BigNum(_), Rational(_))
        | (Rational(_), BigRational(_))
        | (BigRational(_), Rational(_)) => false,
        (BigRational(_), IntV(_))
        | (IntV(_), BigRational(_))
        | (BigRational(_), BigNum(_))
        | (BigNum(_), BigRational(_)) => false,
        (IntV(_), BigNum(_)) | (BigNum(_), IntV(_)) => false,
        (Complex(x), Complex(y)) => {
            number_equality(&x.re, &y.re)? == BoolV(true)
                && number_equality(&x.im, &y.re)? == BoolV(true)
        }
        (Complex(_), _) | (_, Complex(_)) => false,
        _ => stop!(TypeMismatch => "= expects two numbers, found: {:?} and {:?}", left, right),
    };
    Ok(BoolV(result))
}

impl PartialOrd for StahlVal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // TODO: Attempt to avoid converting to f64 for cases below as it may lead to precision loss
        // at tiny and large values.
        match (self, other) {
            // Comparison of matching `StahlVal` variants:
            (IntV(x), IntV(y)) => x.partial_cmp(y),
            (BigNum(x), BigNum(y)) => x.partial_cmp(y),
            (Rational(x), Rational(y)) => x.partial_cmp(y),
            (BigRational(x), BigRational(y)) => x.partial_cmp(y),
            (NumV(x), NumV(y)) => x.partial_cmp(y),
            (StringV(s), StringV(o)) => s.partial_cmp(o),
            (CharV(l), CharV(r)) => l.partial_cmp(r),

            // Comparison of `IntV`, means promoting to the rhs type
            (IntV(x), BigNum(y)) => x
                .to_bigint()
                .expect("integers are representable by bigint")
                .partial_cmp(y),
            (IntV(x), Rational(y)) => {
                // Since we have platform-dependent type for rational conditional compilation is required to find
                // the common ground
                #[cfg(target_pointer_width = "32")]
                {
                    let x_rational = num::Rational32::new_raw(*x as i32, 1);
                    x_rational.partial_cmp(y)
                }
                #[cfg(target_pointer_width = "64")]
                {
                    let x_rational = num::Rational64::new_raw(*x as i64, 1);
                    x_rational.partial_cmp(&num::Rational64::new_raw(
                        *y.numer() as i64,
                        *y.denom() as i64,
                    ))
                }
            }
            (IntV(x), BigRational(y)) => {
                let x_rational = BigRational::from_integer(
                    x.to_bigint().expect("integers are representable by bigint"),
                );
                x_rational.partial_cmp(y)
            }
            (IntV(x), NumV(y)) => (*x as f64).partial_cmp(y),

            // BigNum comparisons means promoting to BigInt for integers, BigRational for ratios,
            // or Decimal otherwise
            (BigNum(x), IntV(y)) => x
                .as_ref()
                .partial_cmp(&y.to_bigint().expect("integers are representable by bigint")),
            (BigNum(x), Rational(y)) => {
                let x_big_rational = BigRational::from_integer(x.unwrap());
                let y_big_rational = BigRational::new_raw(
                    y.numer()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                    y.denom()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                );
                x_big_rational.partial_cmp(&y_big_rational)
            }
            (BigNum(x), BigRational(y)) => {
                let x_big_rational = BigRational::from_integer(x.unwrap());
                x_big_rational.partial_cmp(&y)
            }
            (BigNum(x), NumV(y)) => {
                let x_decimal = BigDecimal::new(x.unwrap(), 0);
                let y_decimal_opt = BigDecimal::from_f64(*y);
                y_decimal_opt.and_then(|y_decimal| x_decimal.partial_cmp(&y_decimal))
            }

            // Rationals require rationals, regular or bigger versions; for float it will be divided to float as well
            (Rational(x), IntV(y)) => {
                // Same as before, but opposite direction
                #[cfg(target_pointer_width = "32")]
                {
                    let y_rational = num::Rational32::new_raw(*y as i32, 1);
                    x.partial_cmp(&y_rational)
                }
                #[cfg(target_pointer_width = "64")]
                {
                    let y_rational = num::Rational64::new_raw(*y as i64, 1);
                    num::Rational64::new_raw(*x.numer() as i64, *x.denom() as i64)
                        .partial_cmp(&y_rational)
                }
            }
            (Rational(x), BigNum(y)) => {
                let x_big_rational = BigRational::new_raw(
                    x.numer()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                    x.denom()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                );
                let y_big_rational = BigRational::from_integer(y.unwrap());
                x_big_rational.partial_cmp(&y_big_rational)
            }
            (Rational(x), BigRational(y)) => {
                let x_big_rational = BigRational::new_raw(
                    x.numer()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                    x.denom()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                );
                x_big_rational.partial_cmp(&y)
            }
            (Rational(x), NumV(y)) => (*x.numer() as f64 / *x.denom() as f64).partial_cmp(y),

            // The most capacious set, but need to cover float case with BigDecimal anyways
            (BigRational(x), IntV(y)) => {
                let y_rational = BigRational::from_integer(
                    y.to_bigint().expect("integers are representable by bigint"),
                );
                x.as_ref().partial_cmp(&y_rational)
            }
            (BigRational(x), BigNum(y)) => {
                let y_big_rational = BigRational::from_integer(y.unwrap());
                x.as_ref().partial_cmp(&y_big_rational)
            }
            (BigRational(x), Rational(y)) => {
                let y_big_rational = BigRational::new_raw(
                    y.numer()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                    y.denom()
                        .to_bigint()
                        .expect("integers are representable by bigint"),
                );
                x.as_ref().partial_cmp(&y_big_rational)
            }
            (BigRational(x), NumV(y)) => {
                let x_decimal =
                    BigDecimal::new(x.numer().clone(), 0) / BigDecimal::new(x.denom().clone(), 0);
                let y_decimal_opt = BigDecimal::from_f64(*y);
                y_decimal_opt.and_then(|y_decimal| x_decimal.partial_cmp(&y_decimal))
            }

            // The opposite of all float cases above
            (NumV(x), IntV(y)) => x.partial_cmp(&(*y as f64)),
            (NumV(x), BigNum(y)) => {
                let x_decimal_opt = BigDecimal::from_f64(*x);
                let y_decimal = BigDecimal::new(y.unwrap(), 0);
                x_decimal_opt.and_then(|x_decimal| x_decimal.partial_cmp(&y_decimal))
            }
            (NumV(x), Rational(y)) => x.partial_cmp(&(*y.numer() as f64 / *y.denom() as f64)),
            (NumV(x), BigRational(y)) => {
                let x_decimal_opt = BigDecimal::from_f64(*x);
                let y_decimal =
                    BigDecimal::new(y.numer().clone(), 0) / BigDecimal::new(y.denom().clone(), 0);
                x_decimal_opt.and_then(|x_decimal| x_decimal.partial_cmp(&y_decimal))
            }

            (l, r) => {
                // All real numbers (not complex) should have order defined.
                debug_assert!(
                    !(realp(l) && realp(r)),
                    "Numbers {l:?} and {r:?} should implement partial_cmp"
                );
                // Unimplemented for other types
                None
            }
        }
    }
}

impl fmt::Display for StahlVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        CycleDetector::detect_and_display_cycles(self, f)
    }
}

impl fmt::Debug for StahlVal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // at the top level, print a ' if we are
        // trying to print a symbol or list
        match self {
            SymbolV(_) | ListV(_) | VectorV(_) => write!(f, "'")?,
            _ => (),
        };
        // display_helper(self, f)

        CycleDetector::detect_and_display_cycles(self, f)
    }
}

#[cfg(test)]
mod or_else_tests {

    use super::*;

    #[cfg(feature = "sync")]
    use im::vector;

    #[cfg(not(feature = "sync"))]
    use im_rc::vector;

    #[test]
    fn bool_or_else_test_good() {
        let input = StahlVal::BoolV(true);
        assert_eq!(input.bool_or_else(throw!(Generic => "test")).unwrap(), true);
    }

    #[test]
    fn bool_or_else_test_bad() {
        let input = StahlVal::CharV('f');
        assert!(input.bool_or_else(throw!(Generic => "test")).is_err());
    }

    #[test]
    fn num_or_else_test_good() {
        let input = StahlVal::NumV(10.0);
        assert_eq!(input.num_or_else(throw!(Generic => "test")).unwrap(), 10.0);
    }

    #[test]
    fn num_or_else_test_bad() {
        let input = StahlVal::CharV('f');
        assert!(input.num_or_else(throw!(Generic => "test")).is_err());
    }

    #[test]
    fn char_or_else_test_good() {
        let input = StahlVal::CharV('f');
        assert_eq!(input.char_or_else(throw!(Generic => "test")).unwrap(), 'f');
    }

    #[test]
    fn char_or_else_test_bad() {
        let input = StahlVal::NumV(10.0);
        assert!(input.char_or_else(throw!(Generic => "test")).is_err());
    }

    #[test]
    fn vector_or_else_test_good() {
        let input: StahlVal = vector![StahlVal::IntV(1)].into();
        assert_eq!(
            input.vector_or_else(throw!(Generic => "test")).unwrap(),
            vector![StahlVal::IntV(1)]
        );
    }

    #[test]
    fn vector_or_else_bad() {
        let input = StahlVal::CharV('f');
        assert!(input.vector_or_else(throw!(Generic => "test")).is_err());
    }

    #[test]
    fn void_or_else_test_good() {
        let input = StahlVal::Void;
        assert_eq!(input.void_or_else(throw!(Generic => "test")).unwrap(), ())
    }

    #[test]
    fn void_or_else_test_bad() {
        let input = StahlVal::StringV("foo".into());
        assert!(input.void_or_else(throw!(Generic => "test")).is_err());
    }

    #[test]
    fn string_or_else_test_good() {
        let input = StahlVal::StringV("foo".into());
        assert_eq!(
            input.string_or_else(throw!(Generic => "test")).unwrap(),
            "foo".to_string()
        );
    }

    #[test]
    fn string_or_else_test_bad() {
        let input = StahlVal::Void;
        assert!(input.string_or_else(throw!(Generic => "test")).is_err())
    }

    #[test]
    fn symbol_or_else_test_good() {
        let input = StahlVal::SymbolV("foo".into());
        assert_eq!(
            input.symbol_or_else(throw!(Generic => "test")).unwrap(),
            "foo".to_string()
        );
    }

    #[test]
    fn symbol_or_else_test_bad() {
        let input = StahlVal::Void;
        assert!(input.symbol_or_else(throw!(Generic => "test")).is_err())
    }

    #[test]
    fn num_and_char_are_not_ordered() {
        assert_eq!(StahlVal::IntV(0).partial_cmp(&StahlVal::CharV('0')), None);
        assert_eq!(StahlVal::NumV(0.0).partial_cmp(&StahlVal::CharV('0')), None);
        assert_eq!(
            StahlVal::BigNum(Gc::new(BigInt::default())).partial_cmp(&StahlVal::CharV('0')),
            None
        );
    }

    #[test]
    fn number_cmp() {
        let less_cases = [
            (StahlVal::IntV(-10), StahlVal::IntV(1)),
            (
                StahlVal::IntV(-10),
                StahlVal::BigNum(Gc::new(BigInt::from(1))),
            ),
            (StahlVal::NumV(-10.0), StahlVal::IntV(1)),
            (StahlVal::IntV(-10), StahlVal::NumV(1.0)),
            (
                StahlVal::BigNum(Gc::new(BigInt::from(-10))),
                StahlVal::BigNum(Gc::new(BigInt::from(1))),
            ),
            (
                StahlVal::NumV(-10.0),
                StahlVal::BigNum(Gc::new(BigInt::from(1))),
            ),
        ];
        for (l, r) in less_cases {
            assert_eq!(l.partial_cmp(&r), Some(Ordering::Less));
            assert_eq!(r.partial_cmp(&l), Some(Ordering::Greater));
        }
        let equal_cases = [
            StahlVal::IntV(-10),
            StahlVal::NumV(-10.0),
            StahlVal::BigNum(Gc::new(BigInt::from(-10))),
            // Added to test that the number is equal even if it points to a different object.
            StahlVal::BigNum(Gc::new(BigInt::from(-10))),
        ]
        .into_iter();
        for (l, r) in equal_cases.clone().zip(equal_cases.clone()) {
            assert_eq!(l.partial_cmp(&r), Some(Ordering::Equal));
        }
    }
}
