use std::{cell::RefCell, collections::HashSet};

#[cfg(feature = "sync")]
use std::sync::Mutex;

use crate::{
    compiler::map::SymbolMap,
    gc::{
        shared::{MutContainer, ShareableMut, WeakShared},
        GcMut, Shared, SharedMut,
    },
    rvals::{OpaqueIterator, StahlComplex, StahlVector},
    stahl_vm::vm::{Continuation, ContinuationMark, Synchronizer},
    values::lists::List,
};
use num::{BigInt, BigRational, Rational32};

#[cfg(feature = "sync")]
use once_cell::sync::Lazy;

use stahl_gen::OpCode;

use crate::{
    gc::{unsafe_erased_pointers::OpaqueReference, Gc},
    rvals::{
        cycles::BreadthFirstSearchStahlValVisitor, BoxedAsyncFunctionSignature, CustomType,
        FunctionSignature, FutureResult, MutFunctionSignature, StahlHashMap, StahlHashSet,
        StahlString, Syntax,
    },
    stahl_vm::vm::BuiltInSignature,
    values::functions::ByteCodeLambda,
    StahlVal,
};

use super::{
    functions::BoxedDynFunction,
    lazy_stream::LazyStream,
    port::StahlPort,
    structs::UserDefinedStruct,
    transducers::{Reducer, Transducer},
};

#[derive(Default)]
pub struct GlobalSlotRecycler {
    // Use a hashset to check for free slots.
    // The idea here is that a collection will traverse
    // all active values (excluding roots with this index)
    // and we'll check to make sure these are reachable.
    //
    // If the values are eventually reachable, then we can keep
    // iterating until this is completely drained.
    //
    // If we reach the end of our iteration and this isn't
    // drained, whatever is left is now freeable, and we can make
    // this as free in the symbol map
    slots: HashSet<usize>,

    queue: Vec<StahlVal>,
}

impl GlobalSlotRecycler {
    pub fn free_shadowed_rooted_values(
        roots: &mut Vec<StahlVal>,
        symbol_map: &mut SymbolMap,
        heap: &mut Heap,
    ) {
        let mut recycler = GlobalSlotRecycler::default();

        recycler.recycle(roots, symbol_map, heap);
    }

    // TODO:
    // Take the global roots, without the shadowed values, and iterate over them,
    // push the values back, visit, mark visited, move on.
    pub fn recycle(
        &mut self,
        roots: &mut Vec<StahlVal>,
        symbol_map: &mut SymbolMap,
        heap: &mut Heap,
    ) {
        self.slots.clear();

        // TODO: Right now, after one pass, we'll ignore it forever.
        // we should move it to another stage that we check later.
        for slot in symbol_map
            .free_list
            .shadowed_slots
            .drain(..)
            .chain(symbol_map.free_list.lambda_lifted.drain(..))
        {
            self.slots.insert(slot);
        }

        for (index, root) in roots.iter().enumerate() {
            if !self.slots.contains(&index) {
                self.push_back(root.clone());
            }
        }

        // Actually walk the tree, looking for unreachable stuff
        self.visit();

        // put them back as unreachable
        heap.memory.iter().for_each(|x| x.write().reset());
        heap.vectors.iter().for_each(|x| x.write().reset());

        // Anything that is still remaining will require
        // getting added to the free list that is left.
        for index in self.slots.drain() {
            if index < roots.len() {
                symbol_map.free_list.free_list.push(index);
                roots[index] = StahlVal::Void;
            }
        }
    }
}

impl<'a> BreadthFirstSearchStahlValVisitor for GlobalSlotRecycler {
    type Output = ();

    fn default_output(&mut self) -> Self::Output {}

    fn pop_front(&mut self) -> Option<StahlVal> {
        self.queue.pop()
    }

    fn visit(&mut self) -> Self::Output {
        use StahlVal::*;

        let mut ret = self.default_output();

        while let Some(value) = self.pop_front() {
            if self.slots.is_empty() {
                return;
            }

            ret = match value {
                Closure(c) => self.visit_closure(c),
                BoolV(b) => self.visit_bool(b),
                NumV(n) => self.visit_float(n),
                IntV(i) => self.visit_int(i),
                Rational(x) => self.visit_rational(x),
                BigRational(x) => self.visit_bigrational(x),
                BigNum(b) => self.visit_bignum(b),
                Complex(x) => self.visit_complex(x),
                CharV(c) => self.visit_char(c),
                VectorV(v) => self.visit_immutable_vector(v),
                Void => self.visit_void(),
                StringV(s) => self.visit_string(s),
                FuncV(f) => self.visit_function_pointer(f),
                SymbolV(s) => self.visit_symbol(s),
                StahlVal::Custom(c) => self.visit_custom_type(c),
                HashMapV(h) => self.visit_hash_map(h),
                HashSetV(s) => self.visit_hash_set(s),
                CustomStruct(c) => self.visit_stahl_struct(c),
                PortV(p) => self.visit_port(p),
                IterV(t) => self.visit_transducer(t),
                ReducerV(r) => self.visit_reducer(r),
                FutureFunc(f) => self.visit_future_function(f),
                FutureV(f) => self.visit_future(f),
                StreamV(s) => self.visit_stream(s),
                BoxedFunction(b) => self.visit_boxed_function(b),
                ContinuationFunction(c) => self.visit_continuation(c),
                ListV(l) => self.visit_list(l),
                MutFunc(m) => self.visit_mutable_function(m),
                BuiltIn(b) => self.visit_builtin_function(b),
                MutableVector(b) => self.visit_mutable_vector(b),
                BoxedIterator(b) => self.visit_boxed_iterator(b),
                StahlVal::SyntaxObject(s) => self.visit_syntax_object(s),
                Boxed(b) => self.visit_boxed_value(b),
                Reference(r) => self.visit_reference_value(r),
                HeapAllocated(b) => self.visit_heap_allocated(b),
                Pair(p) => self.visit_pair(p),
                ByteVector(b) => self.visit_bytevector(b),
            };
        }

        ret
    }

    fn push_back(&mut self, value: StahlVal) {
        // TODO: Determine if all numbers should push back.
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
            | StahlVal::ByteVector(_)
            | StahlVal::BigNum(_) => return,
            _ => {
                self.queue.push(value);
            }
        }
    }

    fn visit_bytevector(&mut self, _bytevector: crate::rvals::StahlByteVector) -> Self::Output {}
    fn visit_bignum(&mut self, _: Gc<BigInt>) -> Self::Output {}
    fn visit_complex(&mut self, _: Gc<StahlComplex>) -> Self::Output {}
    fn visit_bool(&mut self, _boolean: bool) -> Self::Output {}
    fn visit_boxed_function(&mut self, _function: Gc<BoxedDynFunction>) -> Self::Output {}
    // TODO: Revisit this when the boxed iterator is cleaned up
    fn visit_boxed_iterator(&mut self, iterator: GcMut<OpaqueIterator>) -> Self::Output {
        self.push_back(iterator.read().root.clone());
    }
    fn visit_boxed_value(&mut self, boxed_value: GcMut<StahlVal>) -> Self::Output {
        self.push_back(boxed_value.read().clone());
    }

    fn visit_builtin_function(&mut self, _function: BuiltInSignature) -> Self::Output {}

    fn visit_char(&mut self, _c: char) -> Self::Output {}
    fn visit_closure(&mut self, closure: Gc<ByteCodeLambda>) -> Self::Output {
        // for heap_ref in closure.heap_allocated.borrow().iter() {
        // todo!()
        // self.mark_heap_reference(&heap_ref.strong_ptr())
        // }

        for capture in closure.captures() {
            self.push_back(capture.clone());
        }

        if let Some(contract) = closure.get_contract_information().as_ref() {
            self.push_back(contract.clone());
        }

        for instruction in closure.body_exp.iter() {
            match instruction.op_code {
                // If this instruction touches this global variable,
                // then we want to mark it as possibly referenced here.
                OpCode::CALLGLOBAL | OpCode::PUSH | OpCode::CALLGLOBALTAIL => {
                    self.slots.remove(&(instruction.payload_size.to_usize()));
                }
                _ => {}
            }
        }
    }
    fn visit_continuation(&mut self, continuation: Continuation) -> Self::Output {
        let continuation = (*continuation.inner.read()).clone();

        match continuation {
            ContinuationMark::Closed(continuation) => {
                for value in &continuation.stack {
                    self.push_back(value.clone());
                }

                for value in &continuation.current_frame.function.captures {
                    self.push_back(value.clone());
                }

                for frame in &continuation.stack_frames {
                    for value in &frame.function.captures {
                        self.push_back(value.clone());
                    }

                    // if let Some(handler) = &frame.handler {
                    //     self.push_back((*handler.as_ref()).clone());
                    // }

                    if let Some(handler) =
                        frame.attachments.as_ref().and_then(|x| x.handler.clone())
                    {
                        self.push_back(handler);
                    }
                }
            }

            ContinuationMark::Open(continuation) => {
                for value in &continuation.current_stack_values {
                    self.push_back(value.clone());
                }

                for value in &continuation.current_frame.function.captures {
                    self.push_back(value.clone());
                }
            }
        }
    }
    // TODO: Come back to this
    fn visit_custom_type(&mut self, custom_type: GcMut<Box<dyn CustomType>>) -> Self::Output {
        let mut queue = MarkAndSweepContext {
            queue: &mut self.queue,
            object_count: 0,
        };

        custom_type.read().visit_children(&mut queue);
    }

    fn visit_float(&mut self, _float: f64) -> Self::Output {}

    fn visit_function_pointer(&mut self, _ptr: FunctionSignature) -> Self::Output {}

    fn visit_future(&mut self, _future: Gc<FutureResult>) -> Self::Output {}

    fn visit_future_function(&mut self, _function: BoxedAsyncFunctionSignature) -> Self::Output {}

    fn visit_hash_map(&mut self, hashmap: StahlHashMap) -> Self::Output {
        for (key, value) in hashmap.iter() {
            self.push_back(key.clone());
            self.push_back(value.clone());
        }
    }

    fn visit_hash_set(&mut self, hashset: StahlHashSet) -> Self::Output {
        for value in hashset.iter() {
            self.push_back(value.clone());
        }
    }

    fn visit_heap_allocated(&mut self, heap_ref: HeapRef<StahlVal>) -> Self::Output {
        let mut queue = MarkAndSweepContext {
            queue: &mut self.queue,
            object_count: 0,
        };

        queue.mark_heap_reference(&heap_ref.strong_ptr());
    }

    fn visit_immutable_vector(&mut self, vector: StahlVector) -> Self::Output {
        for value in vector.iter() {
            self.push_back(value.clone());
        }
    }
    fn visit_int(&mut self, _int: isize) -> Self::Output {}
    fn visit_rational(&mut self, _: Rational32) -> Self::Output {}
    fn visit_bigrational(&mut self, _: Gc<BigRational>) -> Self::Output {}

    fn visit_list(&mut self, list: List<StahlVal>) -> Self::Output {
        for value in list {
            self.push_back(value);
        }
    }

    fn visit_mutable_function(&mut self, _function: MutFunctionSignature) -> Self::Output {}

    fn visit_mutable_vector(&mut self, vector: HeapRef<Vec<StahlVal>>) -> Self::Output {
        let mut queue = MarkAndSweepContext {
            queue: &mut self.queue,
            object_count: 0,
        };

        queue.mark_heap_vector(&vector.strong_ptr())
    }

    fn visit_port(&mut self, _port: StahlPort) -> Self::Output {}

    fn visit_reducer(&mut self, reducer: Gc<Reducer>) -> Self::Output {
        match reducer.as_ref().clone() {
            Reducer::ForEach(f) => self.push_back(f),
            Reducer::Generic(rf) => {
                self.push_back(rf.initial_value);
                self.push_back(rf.function);
            }
            _ => {}
        }
    }

    // TODO: Revisit this
    fn visit_reference_value(&mut self, _reference: Gc<OpaqueReference<'static>>) -> Self::Output {}

    fn visit_stahl_struct(&mut self, stahl_struct: Gc<UserDefinedStruct>) -> Self::Output {
        for field in stahl_struct.fields.iter() {
            self.push_back(field.clone());
        }
    }

    fn visit_stream(&mut self, stream: Gc<LazyStream>) -> Self::Output {
        self.push_back(stream.initial_value.clone());
        self.push_back(stream.stream_thunk.clone());
    }

    fn visit_string(&mut self, _string: StahlString) -> Self::Output {}

    fn visit_symbol(&mut self, _symbol: StahlString) -> Self::Output {}

    fn visit_syntax_object(&mut self, syntax_object: Gc<Syntax>) -> Self::Output {
        if let Some(raw) = syntax_object.raw.clone() {
            self.push_back(raw);
        }

        self.push_back(syntax_object.syntax.clone());
    }

    fn visit_transducer(&mut self, transducer: Gc<Transducer>) -> Self::Output {
        for transducer in transducer.ops.iter() {
            match transducer.clone() {
                crate::values::transducers::Transducers::Map(m) => self.push_back(m),
                crate::values::transducers::Transducers::Filter(v) => self.push_back(v),
                crate::values::transducers::Transducers::Take(t) => self.push_back(t),
                crate::values::transducers::Transducers::Drop(d) => self.push_back(d),
                crate::values::transducers::Transducers::FlatMap(fm) => self.push_back(fm),
                crate::values::transducers::Transducers::Flatten => {}
                crate::values::transducers::Transducers::Window(w) => self.push_back(w),
                crate::values::transducers::Transducers::TakeWhile(tw) => self.push_back(tw),
                crate::values::transducers::Transducers::DropWhile(dw) => self.push_back(dw),
                crate::values::transducers::Transducers::Extend(e) => self.push_back(e),
                crate::values::transducers::Transducers::Cycle => {}
                crate::values::transducers::Transducers::Enumerating => {}
                crate::values::transducers::Transducers::Zipping(z) => self.push_back(z),
                crate::values::transducers::Transducers::Interleaving(i) => self.push_back(i),
            }
        }
    }

    fn visit_void(&mut self) -> Self::Output {}

    fn visit_pair(&mut self, pair: Gc<super::lists::Pair>) -> Self::Output {
        self.push_back(pair.car());
        self.push_back(pair.cdr());
    }
}

const GC_THRESHOLD: usize = 256 * 1000;
const GC_GROW_FACTOR: usize = 2;
const RESET_LIMIT: usize = 5;

// TODO: Do these roots needs to be truly global?
// Replace this with a lazy static
thread_local! {
    static ROOTS: RefCell<Roots> = RefCell::new(Roots::default());
}

// stash roots in the global area
#[cfg(feature = "sync")]
static GLOBAL_ROOTS: Lazy<Mutex<Roots>> = Lazy::new(|| Mutex::new(Roots::default()));

#[derive(Default)]
pub struct Roots {
    generation: usize,
    offset: usize,
    roots: fxhash::FxHashMap<(usize, usize), StahlVal>,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RootToken {
    generation: usize,
    offset: usize,
}

impl Drop for RootToken {
    #[cfg(not(feature = "sync"))]
    fn drop(&mut self) {
        ROOTS.with(|x| x.borrow_mut().free(self))
    }

    #[cfg(feature = "sync")]
    fn drop(&mut self) {
        GLOBAL_ROOTS.lock().unwrap().free(self)
    }
}

#[derive(Debug)]
pub struct RootedStahlVal {
    value: StahlVal,
    token: RootToken,
}

impl RootedStahlVal {
    pub fn value(&self) -> &StahlVal {
        &self.value
    }
}

impl Roots {
    fn root(&mut self, value: StahlVal) -> RootToken {
        let generation = self.generation;
        let offset = self.offset;

        self.offset += 1;

        self.roots.insert((generation, offset), value);

        RootToken { generation, offset }
    }

    fn free(&mut self, token: &RootToken) {
        self.roots.remove(&(token.generation, token.offset));
    }

    fn increment_generation(&mut self) {
        self.generation += 1;
    }
}

impl StahlVal {
    pub fn mark_rooted(&self) -> RootToken {
        #[cfg(feature = "sync")]
        {
            GLOBAL_ROOTS.lock().unwrap().root(self.clone())
        }

        #[cfg(not(feature = "sync"))]
        {
            ROOTS.with(|x| x.borrow_mut().root(self.clone()))
        }
    }

    // If we're storing in an external struct that could escape
    // the runtime, we probably want to be marked as rooted
    pub fn as_rooted(&self) -> RootedStahlVal {
        let token = self.mark_rooted();

        RootedStahlVal {
            token,
            value: self.clone(),
        }
    }
}

type HeapValue = SharedMut<HeapAllocated<StahlVal>>;
type HeapVector = SharedMut<HeapAllocated<Vec<StahlVal>>>;

// Maybe uninitialized

struct FreeList {
    elements: Vec<Option<HeapValue>>,
    cursor: usize,
    alloc_count: usize,
}

impl FreeList {
    const EXTEND_CHUNK: usize = 128;

    fn is_heap_full(&self) -> bool {
        self.alloc_count == self.elements.len()
    }

    fn extend_heap(&mut self) {
        self.cursor = self.elements.len();

        self.elements.reserve(Self::EXTEND_CHUNK);
        self.elements
            .extend(std::iter::repeat(None).take(Self::EXTEND_CHUNK));
    }

    fn allocate(&mut self, value: StahlVal) -> HeapRef<StahlVal> {
        // Drain, moving values around...
        // is that expensive?

        let pointer = Shared::new(MutContainer::new(HeapAllocated::new(value)));
        let weak_ptr = Shared::downgrade(&pointer);

        self.elements[self.cursor] = Some(pointer);
        self.alloc_count += 1;

        // Find where to assign the next slot optimistically
        let next_slot = self.elements[self.cursor..]
            .iter()
            .position(Option::is_none);

        if let Some(next_slot) = next_slot {
            self.cursor += next_slot;
        } else {
            //
            if self.is_heap_full() {
                // Extend the heap, move the cursor to the end
                self.extend_heap();
            } else {
                self.cursor = self.elements.iter().position(Option::is_none).unwrap()
            }
        }

        HeapRef { inner: weak_ptr }
    }

    fn collect_on_condition(&mut self, func: fn(&HeapValue) -> bool) -> usize {
        let mut amount_dropped = 0;

        self.elements.iter_mut().for_each(|x| {
            if x.as_ref().map(func).unwrap_or_default() {
                *x = None;
                amount_dropped += 1;
            }
        });

        self.alloc_count -= amount_dropped;

        amount_dropped
    }

    fn weak_collection(&mut self) -> usize {
        self.collect_on_condition(|inner| Shared::weak_count(inner) == 0)
    }

    fn strong_collection(&mut self) -> usize {
        self.collect_on_condition(|inner| !inner.read().is_reachable())
    }
}

// TODO: If this proves to be faster, make these From(Vec<HeapValue>)
#[derive(Copy, Clone)]
enum CurrentSpace {
    From,
    To,
}

/// The heap for stahl currently uses an allocation scheme based on weak references
/// to reference counted pointers. Allocation is just a `Vec<Rc<RefCell<T>>>`, where
/// allocating simply pushes and allocates a value at the end. When we do a collection,
/// we attempt to do a small collection by just dropping any values with no weak counts
/// pointing to it.
#[derive(Clone)]
pub struct Heap {
    memory: Vec<HeapValue>,

    // from_space: Vec<HeapValue>,
    // to_space: Vec<HeapValue>,
    // current: CurrentSpace,
    vectors: Vec<HeapVector>,
    count: usize,
    threshold: usize,
    mark_and_sweep_queue: Vec<StahlVal>,
    maybe_memory_size: usize,
}

impl Heap {
    pub fn new() -> Self {
        Heap {
            memory: Vec::with_capacity(256),

            // from_space: Vec::with_capacity(256),
            // to_space: Vec::with_capacity(256),
            // current: CurrentSpace::From,
            vectors: Vec::with_capacity(256),
            count: 0,
            threshold: GC_THRESHOLD,
            // mark_and_sweep_queue: VecDeque::with_capacity(256),
            mark_and_sweep_queue: Vec::with_capacity(256),
            maybe_memory_size: 0,
        }
    }

    pub fn new_empty() -> Self {
        Heap {
            memory: Vec::new(),
            vectors: Vec::new(),
            count: 0,
            threshold: GC_THRESHOLD,
            mark_and_sweep_queue: Vec::new(),
            maybe_memory_size: 0,
        }
    }

    // #[inline(always)]
    // pub fn memory(&mut self) -> &mut Vec<HeapValue> {
    //     match self.current {
    //         CurrentSpace::From => &mut self.from_space,
    //         CurrentSpace::To => &mut self.to_space,
    //     }
    // }

    // Allocate this variable on the heap
    // It explicitly should no longer be on the stack, and variables that
    // reference it should be pointing here now
    pub fn allocate<'a>(
        &mut self,
        value: StahlVal,
        roots: &'a [StahlVal],
        live_functions: impl Iterator<Item = &'a ByteCodeLambda>,
        globals: &'a [StahlVal],
        tls: &'a [StahlVal],
        synchronizer: &'a mut Synchronizer,
    ) -> HeapRef<StahlVal> {
        self.collect(
            Some(value.clone()),
            None,
            roots,
            live_functions,
            globals,
            tls,
            synchronizer,
            false,
        );

        let pointer = Shared::new(MutContainer::new(HeapAllocated::new(value)));
        let weak_ptr = Shared::downgrade(&pointer);

        self.memory.push(pointer);

        HeapRef { inner: weak_ptr }
    }

    pub fn allocate_without_collection<'a>(&mut self, value: StahlVal) -> HeapRef<StahlVal> {
        let pointer = Shared::new(MutContainer::new(HeapAllocated::new(value)));
        let weak_ptr = Shared::downgrade(&pointer);

        self.memory.push(pointer);

        HeapRef { inner: weak_ptr }
    }

    // Allocate a vector explicitly onto the heap
    pub fn allocate_vector<'a>(
        &mut self,
        values: Vec<StahlVal>,
        roots: &'a [StahlVal],
        live_functions: impl Iterator<Item = &'a ByteCodeLambda>,
        globals: &'a [StahlVal],
        tls: &'a [StahlVal],
        synchronizer: &'a mut Synchronizer,
    ) -> HeapRef<Vec<StahlVal>> {
        self.collect(
            None,
            Some(&values),
            roots,
            live_functions,
            globals,
            tls,
            synchronizer,
            false,
        );

        let pointer = Shared::new(MutContainer::new(HeapAllocated::new(values)));
        let weak_ptr = Shared::downgrade(&pointer);

        self.vectors.push(pointer);

        HeapRef { inner: weak_ptr }
    }

    fn vector_cells_allocated(&self) -> usize {
        // self.vectors.iter().map(|x| x.borrow().value.len()).sum()
        self.vectors.len()
    }

    pub fn weak_collection(&mut self) {
        self.memory.retain(|x| Shared::weak_count(x) > 0);
        self.vectors.retain(|x| Shared::weak_count(x) > 0);
    }

    // TODO: Call this in more areas in the VM to attempt to free memory more carefully
    // Also - come up with generational scheme if possible
    pub fn collect<'a>(
        &mut self,
        root_value: Option<StahlVal>,
        root_vector: Option<&Vec<StahlVal>>,
        roots: &'a [StahlVal],
        live_functions: impl Iterator<Item = &'a ByteCodeLambda>,
        globals: &'a [StahlVal],
        tls: &'a [StahlVal],
        synchronizer: &'a mut Synchronizer,
        force_full: bool,
    ) -> usize {
        let memory_size = self.memory.len() + self.vector_cells_allocated();

        if memory_size > self.threshold || force_full {
            log::debug!(target: "gc", "Freeing memory");

            let original_length = memory_size;

            // Do at least one small collection, where we immediately drop
            // anything that has weak counts of 0, meaning there are no alive
            // references and we can avoid doing a full collection
            //
            // In the event that the collection does not yield a substantial
            // change in the heap size, we should also enqueue a larger mark and
            // sweep collection.
            let mut changed = true;
            let mut i = 0;
            while changed && i < 3 {
                let now = std::time::Instant::now();

                log::debug!(target: "gc", "Small collection");
                let prior_len = self.memory.len() + self.vector_cells_allocated();
                log::debug!(target: "gc", "Previous length: {:?}", prior_len);
                self.memory.retain(|x| Shared::weak_count(x) > 0);
                self.vectors.retain(|x| Shared::weak_count(x) > 0);
                let after = self.memory.len() + self.vector_cells_allocated();
                log::debug!(target: "gc", "Objects freed: {:?}", prior_len - after);
                log::debug!(target: "gc", "Small collection time: {:?}", now.elapsed());

                changed = prior_len != after;
                i += 1;
            }

            let post_small_collection_size = self.memory.len() + self.vector_cells_allocated();

            let mut amount = 0;

            // Mark + Sweep!
            if post_small_collection_size as f64 > (0.25 * original_length as f64) || force_full {
                log::debug!(target: "gc", "---- Post small collection, running mark and sweep - heap size filled: {:?} ----", post_small_collection_size as f64 / original_length as f64);

                amount = self.mark_and_sweep(
                    root_value,
                    root_vector,
                    roots,
                    live_functions,
                    globals,
                    tls,
                    synchronizer,
                );
            } else {
                log::debug!(target: "gc", "---- Skipping mark and sweep - heap size filled: {:?} ----", post_small_collection_size as f64 / original_length as f64);
            }

            self.threshold = (self.threshold + self.memory.len() + self.vector_cells_allocated())
                * GC_GROW_FACTOR;

            self.count += 1;

            // Drive it down!
            if self.count > RESET_LIMIT {
                log::debug!(target: "gc", "Shrinking the heap");

                self.threshold = GC_THRESHOLD;
                self.count = 0;

                self.memory.shrink_to(GC_THRESHOLD * GC_GROW_FACTOR);
                self.vectors.shrink_to(GC_THRESHOLD * GC_GROW_FACTOR);
            }

            return amount;
        }

        0
    }

    fn mark_and_sweep<'a>(
        &mut self,
        root_value: Option<StahlVal>,
        root_vector: Option<&Vec<StahlVal>>,
        roots: &'a [StahlVal],
        function_stack: impl Iterator<Item = &'a ByteCodeLambda>,
        globals: &'a [StahlVal],
        tls: &'a [StahlVal],
        synchronizer: &'a mut Synchronizer,
    ) -> usize {
        log::debug!(target: "gc", "Marking the heap");

        #[cfg(feature = "profiling")]
        let now = std::time::Instant::now();

        let mut context = MarkAndSweepContext {
            queue: &mut self.mark_and_sweep_queue,
            object_count: 0,
        };

        // Pause all threads
        synchronizer.stop_threads();
        unsafe {
            synchronizer.enumerate_stacks(&mut context);
        }

        if let Some(root_value) = root_value {
            context.push_back(root_value);
        }

        if let Some(root_vector) = root_vector {
            for value in root_vector {
                context.push_back(value.clone());
            }
        }

        for root in tls {
            context.push_back(root.clone());
        }

        for root in roots {
            context.push_back(root.clone());
        }

        context.visit();

        for root in globals {
            context.push_back(root.clone());
        }

        context.visit();

        for function in function_stack {
            // for heap_ref in function.heap_allocated.borrow().iter() {
            //     context.mark_heap_reference(&heap_ref.strong_ptr())
            // }

            for value in function.captures() {
                context.push_back(value.clone());
            }
        }

        context.visit();

        #[cfg(feature = "sync")]
        {
            GLOBAL_ROOTS
                .lock()
                .unwrap()
                .roots
                .values()
                .for_each(|value| context.push_back(value.clone()))
        }

        #[cfg(not(feature = "sync"))]
        {
            ROOTS.with(|x| {
                x.borrow()
                    .roots
                    .values()
                    .for_each(|value| context.push_back(value.clone()))
            });
        }

        context.visit();

        #[cfg(feature = "profiling")]
        log::debug!(target: "gc", "Mark: Time taken: {:?}", now.elapsed());

        #[cfg(feature = "profiling")]
        let now = std::time::Instant::now();

        let object_count = context.object_count;

        log::debug!(target: "gc", "--- Sweeping ---");
        let prior_len = self.memory.len() + self.vector_cells_allocated();

        // sweep
        self.memory.retain(|x| x.read().is_reachable());
        self.vectors.retain(|x| x.read().is_reachable());

        let after_len = self.memory.len();

        let amount_freed = prior_len - after_len;

        log::debug!(target: "gc", "Freed objects: {:?}", amount_freed);
        log::debug!(target: "gc", "Objects alive: {:?}", after_len);

        // put them back as unreachable
        self.memory.iter().for_each(|x| x.write().reset());
        self.vectors.iter().for_each(|x| x.write().reset());

        #[cfg(feature = "sync")]
        {
            GLOBAL_ROOTS.lock().unwrap().increment_generation();
        }

        #[cfg(not(feature = "sync"))]
        {
            ROOTS.with(|x| x.borrow_mut().increment_generation());
        }

        #[cfg(feature = "profiling")]
        log::debug!(target: "gc", "Sweep: Time taken: {:?}", now.elapsed());

        synchronizer.resume_threads();

        object_count.saturating_sub(amount_freed)
    }
}

pub trait HeapAble: Clone + std::fmt::Debug + PartialEq + Eq {}
impl HeapAble for StahlVal {}
impl HeapAble for Vec<StahlVal> {}

#[derive(Clone, Debug)]
pub struct HeapRef<T: HeapAble> {
    inner: WeakShared<MutContainer<HeapAllocated<T>>>,
}

impl<T: HeapAble> HeapRef<T> {
    pub fn get(&self) -> T {
        self.inner.upgrade().unwrap().read().value.clone()
    }

    pub fn as_ptr_usize(&self) -> usize {
        self.inner.as_ptr() as usize
    }

    pub fn set(&mut self, value: T) -> T {
        let inner = self.inner.upgrade().unwrap();

        let ret = { inner.read().value.clone() };

        inner.write().value = value;
        ret
    }

    pub fn set_and_return(&self, value: T) -> T {
        let inner = self.inner.upgrade().unwrap();

        let mut guard = inner.write();
        std::mem::replace(&mut guard.value, value)
    }

    pub(crate) fn set_interior_mut(&self, value: T) -> T {
        let inner = self.inner.upgrade().unwrap();

        let ret = { inner.read().value.clone() };

        inner.write().value = value;
        ret
    }

    pub(crate) fn strong_ptr(&self) -> SharedMut<HeapAllocated<T>> {
        self.inner.upgrade().unwrap()
    }

    pub(crate) fn ptr_eq(&self, other: &Self) -> bool {
        WeakShared::ptr_eq(&self.inner, &other.inner)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeapAllocated<T: Clone + std::fmt::Debug + PartialEq + Eq> {
    pub(crate) reachable: bool,
    pub(crate) value: T,
}

// Adding generation information should be doable here
// struct Test {
//     pub(crate) reachable: bool,
//     pub(crate) generation: u32,
//     pub(crate) value: StahlVal,
// }

#[test]
fn check_size_of_heap_allocated_value() {
    println!("{:?}", std::mem::size_of::<HeapAllocated<StahlVal>>());
    // println!("{:?}", std::mem::size_of::<Test>());
}

impl<T: Clone + std::fmt::Debug + PartialEq + Eq> HeapAllocated<T> {
    pub fn new(value: T) -> Self {
        Self {
            reachable: false,
            value,
        }
    }

    pub fn is_reachable(&self) -> bool {
        self.reachable
    }

    pub(crate) fn mark_reachable(&mut self) {
        self.reachable = true;
    }

    pub(crate) fn reset(&mut self) {
        self.reachable = false;
    }
}

pub struct MarkAndSweepContext<'a> {
    queue: &'a mut Vec<StahlVal>,
    object_count: usize,
}

impl<'a> MarkAndSweepContext<'a> {
    pub(crate) fn mark_heap_reference(&mut self, heap_ref: &SharedMut<HeapAllocated<StahlVal>>) {
        if heap_ref.read().is_reachable() {
            return;
        }

        {
            heap_ref.write().mark_reachable();
        }

        self.push_back(heap_ref.read().value.clone());
    }

    // Visit the heap vector, mark it as visited!
    pub(crate) fn mark_heap_vector(
        &mut self,
        heap_vector: &SharedMut<HeapAllocated<Vec<StahlVal>>>,
    ) {
        if heap_vector.read().is_reachable() {
            return;
        }

        {
            heap_vector.write().mark_reachable();
        }

        for value in heap_vector.read().value.iter() {
            self.push_back(value.clone());
        }
    }
}

impl<'a> BreadthFirstSearchStahlValVisitor for MarkAndSweepContext<'a> {
    type Output = ();

    fn default_output(&mut self) -> Self::Output {}

    fn pop_front(&mut self) -> Option<StahlVal> {
        self.queue.pop()
    }

    fn push_back(&mut self, value: StahlVal) {
        self.object_count += 1;

        // TODO: Determine if all numbers should push back.
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
            | StahlVal::ByteVector(_)
            | StahlVal::BigNum(_) => return,
            _ => {
                self.queue.push(value);
            }
        }
    }

    fn visit_bytevector(&mut self, _bytevector: crate::rvals::StahlByteVector) -> Self::Output {}
    fn visit_bignum(&mut self, _: Gc<BigInt>) -> Self::Output {}
    fn visit_complex(&mut self, _: Gc<StahlComplex>) -> Self::Output {}
    fn visit_bool(&mut self, _boolean: bool) -> Self::Output {}
    fn visit_boxed_function(&mut self, _function: Gc<BoxedDynFunction>) -> Self::Output {}
    // TODO: Revisit this when the boxed iterator is cleaned up
    fn visit_boxed_iterator(&mut self, iterator: GcMut<OpaqueIterator>) -> Self::Output {
        self.push_back(iterator.read().root.clone());
    }
    fn visit_boxed_value(&mut self, boxed_value: GcMut<StahlVal>) -> Self::Output {
        self.push_back(boxed_value.read().clone());
    }

    fn visit_builtin_function(&mut self, _function: BuiltInSignature) -> Self::Output {}

    fn visit_char(&mut self, _c: char) -> Self::Output {}
    fn visit_closure(&mut self, closure: Gc<ByteCodeLambda>) -> Self::Output {
        // for heap_ref in closure.heap_allocated.borrow().iter() {
        //     self.mark_heap_reference(&heap_ref.strong_ptr())
        // }

        for capture in closure.captures() {
            self.push_back(capture.clone());
        }

        if let Some(contract) = closure.get_contract_information().as_ref() {
            self.push_back(contract.clone());
        }
    }
    fn visit_continuation(&mut self, continuation: Continuation) -> Self::Output {
        // TODO: Don't clone this here!
        let continuation = (*continuation.inner.read()).clone();

        match continuation {
            ContinuationMark::Closed(continuation) => {
                for value in continuation.stack {
                    self.push_back(value);
                }

                for value in &continuation.current_frame.function.captures {
                    self.push_back(value.clone());
                }

                for frame in continuation.stack_frames {
                    for value in &frame.function.captures {
                        self.push_back(value.clone());
                    }

                    // if let Some(handler) = &frame.handler {
                    //     self.push_back((*handler.as_ref()).clone());
                    // }

                    if let Some(handler) =
                        frame.attachments.as_ref().and_then(|x| x.handler.clone())
                    {
                        self.push_back(handler);
                    }
                }
            }

            ContinuationMark::Open(continuation) => {
                for value in &continuation.current_stack_values {
                    self.push_back(value.clone());
                }

                for value in &continuation.current_frame.function.captures {
                    self.push_back(value.clone());
                }
            }
        }
    }
    // TODO: Come back to this
    fn visit_custom_type(&mut self, custom_type: GcMut<Box<dyn CustomType>>) -> Self::Output {
        custom_type.read().visit_children(self);
    }

    fn visit_float(&mut self, _float: f64) -> Self::Output {}

    fn visit_function_pointer(&mut self, _ptr: FunctionSignature) -> Self::Output {}

    fn visit_future(&mut self, _future: Gc<FutureResult>) -> Self::Output {}

    fn visit_future_function(&mut self, _function: BoxedAsyncFunctionSignature) -> Self::Output {}

    fn visit_hash_map(&mut self, hashmap: StahlHashMap) -> Self::Output {
        for (key, value) in hashmap.iter() {
            self.push_back(key.clone());
            self.push_back(value.clone());
        }
    }

    fn visit_hash_set(&mut self, hashset: StahlHashSet) -> Self::Output {
        for value in hashset.iter() {
            self.push_back(value.clone());
        }
    }

    fn visit_heap_allocated(&mut self, heap_ref: HeapRef<StahlVal>) -> Self::Output {
        self.mark_heap_reference(&heap_ref.strong_ptr());
    }

    fn visit_immutable_vector(&mut self, vector: StahlVector) -> Self::Output {
        for value in vector.iter() {
            self.push_back(value.clone());
        }
    }
    fn visit_int(&mut self, _int: isize) -> Self::Output {}
    fn visit_rational(&mut self, _: Rational32) -> Self::Output {}
    fn visit_bigrational(&mut self, _: Gc<BigRational>) -> Self::Output {}

    fn visit_list(&mut self, list: List<StahlVal>) -> Self::Output {
        for value in list {
            self.push_back(value);
        }
    }

    fn visit_mutable_function(&mut self, _function: MutFunctionSignature) -> Self::Output {}

    fn visit_mutable_vector(&mut self, vector: HeapRef<Vec<StahlVal>>) -> Self::Output {
        self.mark_heap_vector(&vector.strong_ptr())
    }

    fn visit_port(&mut self, _port: StahlPort) -> Self::Output {}

    fn visit_reducer(&mut self, reducer: Gc<Reducer>) -> Self::Output {
        match reducer.as_ref().clone() {
            Reducer::ForEach(f) => self.push_back(f),
            Reducer::Generic(rf) => {
                self.push_back(rf.initial_value);
                self.push_back(rf.function);
            }
            _ => {}
        }
    }

    // TODO: Revisit this
    fn visit_reference_value(&mut self, _reference: Gc<OpaqueReference<'static>>) -> Self::Output {}

    fn visit_stahl_struct(&mut self, stahl_struct: Gc<UserDefinedStruct>) -> Self::Output {
        for field in stahl_struct.fields.iter() {
            self.push_back(field.clone());
        }
    }

    fn visit_stream(&mut self, stream: Gc<LazyStream>) -> Self::Output {
        self.push_back(stream.initial_value.clone());
        self.push_back(stream.stream_thunk.clone());
    }

    fn visit_string(&mut self, _string: StahlString) -> Self::Output {}

    fn visit_symbol(&mut self, _symbol: StahlString) -> Self::Output {}

    fn visit_syntax_object(&mut self, syntax_object: Gc<Syntax>) -> Self::Output {
        if let Some(raw) = syntax_object.raw.clone() {
            self.push_back(raw);
        }

        self.push_back(syntax_object.syntax.clone());
    }

    fn visit_transducer(&mut self, transducer: Gc<Transducer>) -> Self::Output {
        for transducer in transducer.ops.iter() {
            match transducer.clone() {
                crate::values::transducers::Transducers::Map(m) => self.push_back(m),
                crate::values::transducers::Transducers::Filter(v) => self.push_back(v),
                crate::values::transducers::Transducers::Take(t) => self.push_back(t),
                crate::values::transducers::Transducers::Drop(d) => self.push_back(d),
                crate::values::transducers::Transducers::FlatMap(fm) => self.push_back(fm),
                crate::values::transducers::Transducers::Flatten => {}
                crate::values::transducers::Transducers::Window(w) => self.push_back(w),
                crate::values::transducers::Transducers::TakeWhile(tw) => self.push_back(tw),
                crate::values::transducers::Transducers::DropWhile(dw) => self.push_back(dw),
                crate::values::transducers::Transducers::Extend(e) => self.push_back(e),
                crate::values::transducers::Transducers::Cycle => {}
                crate::values::transducers::Transducers::Enumerating => {}
                crate::values::transducers::Transducers::Zipping(z) => self.push_back(z),
                crate::values::transducers::Transducers::Interleaving(i) => self.push_back(i),
            }
        }
    }

    fn visit_void(&mut self) -> Self::Output {}

    fn visit_pair(&mut self, pair: Gc<super::lists::Pair>) -> Self::Output {
        self.push_back(pair.car());
        self.push_back(pair.cdr());
    }
}
