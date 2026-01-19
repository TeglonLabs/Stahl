use super::{
    builtin::{BuiltInModule, MarkdownDoc},
    cache::WeakMemoizationTable,
    engine::Engine,
    register_fn::RegisterFn,
    vm::{
        get_test_mode, list_modules, set_test_mode, VmCore, CALL_CC_DEFINITION,
        CALL_WITH_EXCEPTION_HANDLER_DEFINITION, EVAL_DEFINITION, EVAL_FILE_DEFINITION,
        EVAL_STRING_DEFINITION, EXPAND_SYNTAX_CASE_DEFINITION, EXPAND_SYNTAX_OBJECTS_DEFINITION,
        INSPECT_DEFINITION, MACRO_CASE_BINDINGS_DEFINITION, MATCH_SYNTAX_CASE_DEFINITION,
    },
};
use crate::{
    compiler::modules::stahl_home,
    gc::{shared::ShareableMut, GcMut},
    parser::{
        ast::TryFromStahlValVisitorForExprKind, interner::InternedString, span::Span,
        tryfrom_visitor::TryFromExprKindForStahlVal,
    },
    primitives::{
        bytevectors::bytevector_module,
        fs_module, fs_module_sandbox,
        git::git_module,
        hashmaps::{hashmap_module, HM_CONSTRUCT, HM_GET, HM_INSERT},
        hashsets::hashset_module,
        http::http_module,
        lists::{list_module, UnRecoverableResult},
        numbers::{self, realp},
        port_module,
        ports::{port_module_without_filesystem, EOF_OBJECTP_DEFINITION},
        process::process_module,
        random::random_module,
        string_module, symbol_module,
        tcp::tcp_module,
        time::time_module,
        vectors::{
            immutable_vectors_module, IMMUTABLE_VECTOR_CONSTRUCT_DEFINITION,
            MAKE_VECTOR_DEFINITION, MUTABLE_VECTOR_CLEAR_DEFINITION, MUTABLE_VECTOR_POP_DEFINITION,
            MUTABLE_VECTOR_TO_STRING_DEFINITION, MUT_VECTOR_COPY_DEFINITION,
            MUT_VEC_CONSTRUCT_DEFINITION, MUT_VEC_CONSTRUCT_VEC_DEFINITION,
            MUT_VEC_LENGTH_DEFINITION, MUT_VEC_SET_DEFINITION, MUT_VEC_TO_LIST_DEFINITION,
            VECTOR_FILL_DEFINITION, VEC_LENGTH_DEFINITION,
        },
        ControlOperations, IoFunctions, MetaOperations, StreamOperations, VectorOperations,
    },
    rerrs::ErrorKind,
    rvals::{
        as_underlying_type,
        cycles::{BreadthFirstSearchStahlValVisitor, StahlCycleCollector},
        CustomType, FromStahlVal, StahlString, ITERATOR_FINISHED, NUMBER_EQUALITY_DEFINITION,
    },
    stahl_vm::{
        builtin::{get_function_metadata, get_function_name, Arity, BuiltInFunctionType},
        vm::threads::threading_module,
    },
    values::{
        closed::HeapRef,
        functions::{attach_contract_struct, get_contract, LambdaMetadataTable},
        lists::{List, StahlList},
        structs::{
            build_type_id_module, make_struct_type, struct_update_primitive, StahlResult,
            UserDefinedStruct,
        },
    },
};
use crate::{
    rvals::IntoStahlVal,
    values::structs::{build_option_structs, build_result_structs},
};
use crate::{
    rvals::{Result, StahlVal},
    StahlErr,
};
use compact_str::CompactString;
use fxhash::{FxBuildHasher, FxHashMap, FxHashSet};
use once_cell::sync::Lazy;
use std::cmp::Ordering;
use stahl_parser::{ast::ExprKind, interner::interned_current_memory_usage, parser::SourceId};

#[cfg(not(target_arch = "wasm32"))]
use crate::primitives::polling::polling_module;

#[cfg(target_arch = "wasm32")]
fn polling_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/polling".to_string());

    module
}

#[cfg(feature = "dylibs")]
use crate::stahl_vm::ffi::ffi_module;

macro_rules! ensure_tonicity_two {
    ($check_fn:expr) => {{
        |args: &[StahlVal]| -> Result<StahlVal> {

            if args.is_empty() {
                stop!(ArityMismatch => "expected at least one argument");
            }

            for window in args.windows(2) {
                if let &[left, right] = &window {
                    if !$check_fn(&left, &right) {
                        return Ok(StahlVal::BoolV(false))
                    }
                } else {
                    unreachable!()
                }

            }

            Ok(StahlVal::BoolV(true))

        }
    }};
}

macro_rules! gen_pred {
    ($variant:ident) => {{
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            Ok(if let Some(StahlVal::$variant(..)) = args.first() {
                StahlVal::BoolV(true)
            } else {
                StahlVal::BoolV(false)
            })
        })
    }};

    ($variant1:ident, $variant2:ident) => {{
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if let Some(first) = args.first() {
                match first {
                    StahlVal::$variant1(..) | StahlVal::$variant2(..) => {
                        return Ok(StahlVal::BoolV(true));
                    }
                    _ => {}
                }
            }
            Ok(StahlVal::BoolV(false))
        })
    }};

    // TODO replace this with something better
    ($variant1:ident, $variant2:ident, $variant3:ident, $variant4:ident, $variant5:ident, $variant6: ident, $variant7: ident) => {{
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if let Some(first) = args.first() {
                match first {
                    StahlVal::$variant1(..)
                    | StahlVal::$variant2(..)
                    | StahlVal::$variant3(..)
                    | StahlVal::$variant4(..)
                    | StahlVal::$variant5(..)
                    | StahlVal::$variant6(..)
                    | StahlVal::$variant7(..) => {
                        return Ok(StahlVal::BoolV(true));
                    }
                    _ => {}
                }
            }
            Ok(StahlVal::BoolV(false))
        })
    }};

    ($variant1:ident, $variant2:ident, $variant3:ident, $variant4:ident, $variant5:ident, $variant6:ident) => {{
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if let Some(first) = args.first() {
                match first {
                    StahlVal::$variant1(..)
                    | StahlVal::$variant2(..)
                    | StahlVal::$variant3(..)
                    | StahlVal::$variant4(..)
                    | StahlVal::$variant5(..)
                    | StahlVal::$variant6(..) => {
                        return Ok(StahlVal::BoolV(true));
                    }
                    _ => {}
                }
            }
            Ok(StahlVal::BoolV(false))
        })
    }};
}

pub const CONSTANTS: &[&str] = &[
    "#%prim.+",
    "#%prim.i+",
    "#%prim.f+",
    "#%prim.*",
    "#%prim./",
    "#%prim.-",
    "#%prim.car",
    "#%prim.cdr",
    "#%prim.first",
    "#%prim.rest",
    "#%prim.null?",
    "#%prim.int?",
    "#%prim.float?",
    "#%prim.number?",
    "#%prim.string?",
    "#%prim.symbol?",
    "#%prim.vector?",
    "#%prim.list?",
    "#%prim.integer?",
    "#%prim.boolean?",
    "#%prim.=",
    "#%prim.equal?",
    "#%prim.>",
    "#%prim.>=",
    "#%prim.<",
    "#%prim.<=",
    "#%prim.string-append",
    "#%prim.string->list",
    "#%prim.string-upcase",
    "#%prim.string-lowercase",
    "#%prim.trim",
    "#%prim.trim-start",
    "#%prim.trim-end",
    "#%prim.split-whitespace",
    "#%prim.void",
    "#%prim.list->string",
    "#%prim.concat-symbols",
    "#%prim.string->int",
    "#%prim.even?",
    "#%prim.append",
    "#%prim.length",
    "#%prim.list->string",
    "#%prim.not",
];

#[macro_export]
macro_rules! define_modules {
    ($($name:tt => $func:expr,) * ) => {
        $(
            pub static $name: once_cell::sync::Lazy<BuiltInModule> = once_cell::sync::Lazy::new($func);
        )*
    };
}

#[cfg(feature = "sync")]
define_modules! {
    STAHL_MAP_MODULE => hashmap_module,
    STAHL_SET_MODULE => hashset_module,
    STAHL_LIST_MODULE => list_module,
    STAHL_STRING_MODULE => string_module,
    STAHL_VECTOR_MODULE => vector_module,
    STAHL_IMMUTABLE_VECTOR_MODULE => immutable_vectors_module,
    STAHL_BYTEVECTOR_MODULE => bytevector_module,
    STAHL_STREAM_MODULE => stream_module,
    STAHL_IDENTITY_MODULE => identity_module,
    STAHL_NUMBER_MODULE => number_module,
    STAHL_EQUALITY_MODULE => equality_module,
    STAHL_ORD_MODULE => ord_module,
    STAHL_TRANSDUCER_MODULE => transducer_module,
    STAHL_SYMBOL_MODULE => symbol_module,
    STAHL_IO_MODULE => io_module,
    STAHL_FS_MODULE => fs_module,
    STAHL_FS_MODULE_SB => fs_module_sandbox,
    STAHL_PORT_MODULE => port_module,
    STAHL_PORT_WITHOUT_FS_MODULE => port_module_without_filesystem,
    STAHL_META_MODULE => meta_module,
    STAHL_JSON_MODULE => json_module,
    STAHL_CONSTANTS_MODULE => constants_module,
    STAHL_SYNTAX_MODULE => syntax_module,
    STAHL_SANDBOXED_META_MODULE => sandboxed_meta_module,
    STAHL_PROCESS_MODULE => process_module,
    STAHL_RANDOM_MODULE => random_module,
    STAHL_RESULT_MODULE => build_result_structs,
    STAHL_TYPE_ID_MODULE => build_type_id_module,
    STAHL_OPTION_MODULE => build_option_structs,
    STAHL_THREADING_MODULE => threading_module,
    STAHL_TIME_MODULE => time_module,
    STAHL_MUTABLE_VECTOR_MODULE => mutable_vector_module,
    STAHL_PRIVATE_READER_MODULE => reader_module,
    STAHL_TCP_MODULE => tcp_module,
    STAHL_POLLING_MODULE => polling_module,
    STAHL_HTTP_MODULE => http_module,
    STAHL_PRELUDE_MODULE => prelude,
    STAHL_SB_PRELUDE => sandboxed_prelude,

    STAHL_GIT_MODULE => git_module,
}

#[cfg(all(feature = "dylibs", feature = "sync"))]
pub static STAHL_FFI_MODULE: once_cell::sync::Lazy<BuiltInModule> =
    once_cell::sync::Lazy::new(ffi_module);

thread_local! {
    pub static MAP_MODULE: BuiltInModule = hashmap_module();
    pub static SET_MODULE: BuiltInModule = hashset_module();
    pub static LIST_MODULE: BuiltInModule = list_module();
    pub static STRING_MODULE: BuiltInModule = string_module();
    pub static VECTOR_MODULE: BuiltInModule = vector_module();

    pub static IMMUTABLE_VECTOR_MODULE: BuiltInModule = immutable_vectors_module();

    pub static BYTEVECTOR_MODULE: BuiltInModule = bytevector_module();

    pub static STREAM_MODULE: BuiltInModule = stream_module();
    pub static IDENTITY_MODULE: BuiltInModule = identity_module();
    pub static NUMBER_MODULE: BuiltInModule = number_module();
    pub static EQUALITY_MODULE: BuiltInModule = equality_module();
    pub static ORD_MODULE: BuiltInModule = ord_module();
    pub static TRANSDUCER_MODULE: BuiltInModule = transducer_module();
    pub static SYMBOL_MODULE: BuiltInModule = symbol_module();
    pub static IO_MODULE: BuiltInModule = io_module();
    pub static FS_MODULE: BuiltInModule = fs_module();
    pub static FS_MODULE_SB: BuiltInModule = fs_module_sandbox();
    pub static PORT_MODULE: BuiltInModule = port_module();
    pub static PORT_MODULE_WITHOUT_FILESYSTEM: BuiltInModule = port_module_without_filesystem();
    pub static META_MODULE: BuiltInModule = meta_module();
    pub static JSON_MODULE: BuiltInModule = json_module();
    pub static CONSTANTS_MODULE: BuiltInModule = constants_module();
    pub static SYNTAX_MODULE: BuiltInModule = syntax_module();
    pub static SANDBOXED_META_MODULE: BuiltInModule = sandboxed_meta_module();
    pub static PROCESS_MODULE: BuiltInModule = process_module();
    pub static RANDOM_MODULE: BuiltInModule = random_module();
    pub static RESULT_MODULE: BuiltInModule = build_result_structs();
    pub static TYPE_ID_MODULE: BuiltInModule = build_type_id_module();
    pub static OPTION_MODULE: BuiltInModule = build_option_structs();
    pub static TCP_MODULE: BuiltInModule = tcp_module();
    pub static HTTP_MODULE: BuiltInModule = http_module();
    pub static POLLING_MODULE: BuiltInModule = polling_module();

    #[cfg(feature = "dylibs")]
    pub static FFI_MODULE: BuiltInModule = ffi_module();

    pub static PRELUDE_MODULE: BuiltInModule = prelude();
    pub static SB_PRELUDE: BuiltInModule = sandboxed_prelude();

    pub(crate) static PRELUDE_INTERNED_STRINGS: FxHashSet<InternedString> = PRELUDE_MODULE.with(|x| x.names().into_iter().map(|x| x.into()).collect());


    pub static TIME_MODULE: BuiltInModule = time_module();
    pub static THREADING_MODULE: BuiltInModule = threading_module();

    pub static MUTABLE_VECTOR_MODULE: BuiltInModule = mutable_vector_module();
    pub static PRIVATE_READER_MODULE: BuiltInModule = reader_module();

    pub static GIT_MODULE: BuiltInModule = git_module();
}

pub fn prelude() -> BuiltInModule {
    #[cfg(feature = "sync")]
    {
        BuiltInModule::new("stahl/base")
            .with_module(STAHL_MAP_MODULE.clone())
            .with_module(STAHL_SET_MODULE.clone())
            .with_module(STAHL_LIST_MODULE.clone())
            .with_module(STAHL_STRING_MODULE.clone())
            .with_module(STAHL_VECTOR_MODULE.clone())
            .with_module(STAHL_STREAM_MODULE.clone())
            .with_module(STAHL_IDENTITY_MODULE.clone())
            .with_module(STAHL_NUMBER_MODULE.clone())
            .with_module(STAHL_EQUALITY_MODULE.clone())
            .with_module(STAHL_ORD_MODULE.clone())
            .with_module(STAHL_TRANSDUCER_MODULE.clone())
            .with_module(STAHL_SYMBOL_MODULE.clone())
            .with_module(STAHL_IO_MODULE.clone())
            .with_module(STAHL_FS_MODULE.clone())
            .with_module(STAHL_PORT_MODULE.clone())
            .with_module(STAHL_META_MODULE.clone())
            .with_module(STAHL_JSON_MODULE.clone())
            .with_module(STAHL_CONSTANTS_MODULE.clone())
            .with_module(STAHL_SYNTAX_MODULE.clone())
            .with_module(STAHL_PROCESS_MODULE.clone())
            .with_module(STAHL_RESULT_MODULE.clone())
            .with_module(STAHL_OPTION_MODULE.clone())
            .with_module(STAHL_TYPE_ID_MODULE.clone())
            .with_module(STAHL_TIME_MODULE.clone())
            .with_module(STAHL_THREADING_MODULE.clone())
            .with_module(STAHL_BYTEVECTOR_MODULE.clone())
    }

    #[cfg(not(feature = "sync"))]
    {
        BuiltInModule::new("stahl/base")
            .with_module(MAP_MODULE.with(|x| x.clone()))
            .with_module(SET_MODULE.with(|x| x.clone()))
            .with_module(LIST_MODULE.with(|x| x.clone()))
            .with_module(STRING_MODULE.with(|x| x.clone()))
            .with_module(VECTOR_MODULE.with(|x| x.clone()))
            .with_module(STREAM_MODULE.with(|x| x.clone()))
            .with_module(IDENTITY_MODULE.with(|x| x.clone()))
            .with_module(NUMBER_MODULE.with(|x| x.clone()))
            .with_module(EQUALITY_MODULE.with(|x| x.clone()))
            .with_module(ORD_MODULE.with(|x| x.clone()))
            .with_module(TRANSDUCER_MODULE.with(|x| x.clone()))
            .with_module(SYMBOL_MODULE.with(|x| x.clone()))
            .with_module(IO_MODULE.with(|x| x.clone()))
            .with_module(FS_MODULE.with(|x| x.clone()))
            .with_module(PORT_MODULE.with(|x| x.clone()))
            .with_module(META_MODULE.with(|x| x.clone()))
            .with_module(JSON_MODULE.with(|x| x.clone()))
            .with_module(CONSTANTS_MODULE.with(|x| x.clone()))
            .with_module(SYNTAX_MODULE.with(|x| x.clone()))
            .with_module(PROCESS_MODULE.with(|x| x.clone()))
            .with_module(RESULT_MODULE.with(|x| x.clone()))
            .with_module(OPTION_MODULE.with(|x| x.clone()))
            .with_module(TYPE_ID_MODULE.with(|x| x.clone()))
            .with_module(TIME_MODULE.with(|x| x.clone()))
            .with_module(THREADING_MODULE.with(|x| x.clone()))
            .with_module(BYTEVECTOR_MODULE.with(|x| x.clone()))
    }
}

pub fn sandboxed_prelude() -> BuiltInModule {
    #[cfg(feature = "sync")]
    {
        BuiltInModule::new("stahl/base")
            .with_module(STAHL_MAP_MODULE.clone())
            .with_module(STAHL_SET_MODULE.clone())
            .with_module(STAHL_LIST_MODULE.clone())
            .with_module(STAHL_STRING_MODULE.clone())
            .with_module(STAHL_VECTOR_MODULE.clone())
            .with_module(STAHL_STREAM_MODULE.clone())
            .with_module(STAHL_IDENTITY_MODULE.clone())
            .with_module(STAHL_NUMBER_MODULE.clone())
            .with_module(STAHL_EQUALITY_MODULE.clone())
            .with_module(STAHL_ORD_MODULE.clone())
            .with_module(STAHL_TRANSDUCER_MODULE.clone())
            .with_module(STAHL_SYMBOL_MODULE.clone())
            .with_module(STAHL_IO_MODULE.clone())
            .with_module(STAHL_FS_MODULE_SB.clone())
            .with_module(STAHL_PORT_WITHOUT_FS_MODULE.clone())
            .with_module(STAHL_META_MODULE.clone())
            .with_module(STAHL_JSON_MODULE.clone())
            .with_module(STAHL_CONSTANTS_MODULE.clone())
            .with_module(STAHL_SYNTAX_MODULE.clone())
            .with_module(STAHL_RESULT_MODULE.clone())
            .with_module(STAHL_OPTION_MODULE.clone())
            .with_module(STAHL_TYPE_ID_MODULE.clone())
            .with_module(STAHL_TIME_MODULE.clone())
            .with_module(STAHL_THREADING_MODULE.clone())
            .with_module(STAHL_BYTEVECTOR_MODULE.clone())
    }

    #[cfg(not(feature = "sync"))]
    {
        BuiltInModule::new("stahl/base")
            .with_module(MAP_MODULE.with(|x| x.clone()))
            .with_module(SET_MODULE.with(|x| x.clone()))
            .with_module(LIST_MODULE.with(|x| x.clone()))
            .with_module(STRING_MODULE.with(|x| x.clone()))
            .with_module(VECTOR_MODULE.with(|x| x.clone()))
            .with_module(STREAM_MODULE.with(|x| x.clone()))
            .with_module(IDENTITY_MODULE.with(|x| x.clone()))
            .with_module(NUMBER_MODULE.with(|x| x.clone()))
            .with_module(EQUALITY_MODULE.with(|x| x.clone()))
            .with_module(ORD_MODULE.with(|x| x.clone()))
            .with_module(TRANSDUCER_MODULE.with(|x| x.clone()))
            .with_module(SYMBOL_MODULE.with(|x| x.clone()))
            .with_module(IO_MODULE.with(|x| x.clone()))
            .with_module(FS_MODULE_SB.with(|x| x.clone()))
            .with_module(PORT_MODULE_WITHOUT_FILESYSTEM.with(|x| x.clone()))
            .with_module(META_MODULE.with(|x| x.clone()))
            .with_module(JSON_MODULE.with(|x| x.clone()))
            .with_module(CONSTANTS_MODULE.with(|x| x.clone()))
            .with_module(SYNTAX_MODULE.with(|x| x.clone()))
            .with_module(RESULT_MODULE.with(|x| x.clone()))
            .with_module(OPTION_MODULE.with(|x| x.clone()))
            .with_module(TYPE_ID_MODULE.with(|x| x.clone()))
            .with_module(TIME_MODULE.with(|x| x.clone()))
            .with_module(THREADING_MODULE.with(|x| x.clone()))
            .with_module(BYTEVECTOR_MODULE.with(|x| x.clone()))
    }
}

fn render_as_md(text: String) {
    #[cfg(feature = "markdown")]
    println!("{}", termimad::text(&text));

    #[cfg(not(feature = "markdown"))]
    println!("{}", text);
}

pub fn register_builtin_modules(engine: &mut Engine, sandbox: bool) {
    engine.register_value("std::env::args", StahlVal::ListV(List::new()));

    engine.register_fn("##__module-get", BuiltInModule::get);
    engine.register_fn("%module-get%", BuiltInModule::get);
    engine.register_fn("%#maybe-module-get", BuiltInModule::try_get);

    engine.register_fn("load-from-module!", BuiltInModule::get);

    // Registering values in modules
    engine.register_fn("#%module", BuiltInModule::new::<String>);
    engine.register_fn(
        "#%module-add",
        |module: &mut BuiltInModule, name: StahlString, value: StahlVal| {
            module.register_value(&name, value);
        },
    );

    engine.register_fn("%doc?", BuiltInModule::get_doc);
    engine.register_value("%list-modules!", StahlVal::BuiltIn(list_modules));
    engine.register_fn("%module/lookup-function", BuiltInModule::search);
    engine.register_fn("%string->render-markdown", render_as_md);
    engine.register_fn(
        "%module-bound-identifiers->list",
        BuiltInModule::bound_identifiers,
    );
    engine.register_value("%proto-hash%", HM_CONSTRUCT);
    engine.register_value("%proto-hash-insert%", HM_INSERT);
    engine.register_value("%proto-hash-get%", HM_GET);
    engine.register_value("error!", ControlOperations::error());

    engine.register_value("error", ControlOperations::error());

    engine.register_value("#%error", ControlOperations::error());

    engine.register_value(
        "%memo-table",
        WeakMemoizationTable::new().into_stahlval().unwrap(),
    );
    engine.register_fn("%memo-table-ref", WeakMemoizationTable::get);
    engine.register_fn("%memo-table-set!", WeakMemoizationTable::insert);

    #[cfg(feature = "sync")]
    {
        engine
            .register_module(STAHL_MAP_MODULE.clone())
            .register_module(STAHL_SET_MODULE.clone())
            .register_module(STAHL_LIST_MODULE.clone())
            .register_module(STAHL_STRING_MODULE.clone())
            .register_module(STAHL_VECTOR_MODULE.clone())
            .register_module(STAHL_STREAM_MODULE.clone())
            .register_module(STAHL_IDENTITY_MODULE.clone())
            .register_module(STAHL_NUMBER_MODULE.clone())
            .register_module(STAHL_EQUALITY_MODULE.clone())
            .register_module(STAHL_ORD_MODULE.clone())
            .register_module(STAHL_TRANSDUCER_MODULE.clone())
            .register_module(STAHL_SYMBOL_MODULE.clone())
            .register_module(STAHL_IO_MODULE.clone())
            .register_module(STAHL_PORT_MODULE.clone())
            .register_module(STAHL_FS_MODULE.clone())
            .register_module(STAHL_META_MODULE.clone())
            .register_module(STAHL_JSON_MODULE.clone())
            .register_module(STAHL_CONSTANTS_MODULE.clone())
            .register_module(STAHL_SYNTAX_MODULE.clone())
            .register_module(STAHL_PROCESS_MODULE.clone())
            .register_module(STAHL_RESULT_MODULE.clone())
            .register_module(STAHL_OPTION_MODULE.clone())
            .register_module(STAHL_TYPE_ID_MODULE.clone())
            .register_module(STAHL_PRELUDE_MODULE.clone())
            .register_module(STAHL_TIME_MODULE.clone())
            .register_module(STAHL_RANDOM_MODULE.clone())
            .register_module(STAHL_THREADING_MODULE.clone())
            .register_module(STAHL_BYTEVECTOR_MODULE.clone());

        engine.register_module(STAHL_GIT_MODULE.clone());

        if !sandbox {
            engine
                .register_module(STAHL_TCP_MODULE.clone())
                .register_module(STAHL_HTTP_MODULE.clone())
                .register_module(STAHL_POLLING_MODULE.clone());
        } else {
            engine
                .register_module(STAHL_FS_MODULE_SB.clone())
                .register_module(STAHL_PORT_WITHOUT_FS_MODULE.clone())
                .register_module(STAHL_SB_PRELUDE.clone());
        }

        #[cfg(feature = "dylibs")]
        engine.register_module(STAHL_FFI_MODULE.clone());

        // Private module
        engine.register_module(STAHL_MUTABLE_VECTOR_MODULE.clone());
        engine.register_module(STAHL_PRIVATE_READER_MODULE.clone());
        engine.register_module(STAHL_IMMUTABLE_VECTOR_MODULE.clone());
    }

    #[cfg(not(feature = "sync"))]
    {
        engine
            .register_module(MAP_MODULE.with(|x| x.clone()))
            .register_module(SET_MODULE.with(|x| x.clone()))
            .register_module(LIST_MODULE.with(|x| x.clone()))
            .register_module(STRING_MODULE.with(|x| x.clone()))
            .register_module(VECTOR_MODULE.with(|x| x.clone()))
            .register_module(STREAM_MODULE.with(|x| x.clone()))
            .register_module(IDENTITY_MODULE.with(|x| x.clone()))
            .register_module(NUMBER_MODULE.with(|x| x.clone()))
            .register_module(EQUALITY_MODULE.with(|x| x.clone()))
            .register_module(ORD_MODULE.with(|x| x.clone()))
            .register_module(TRANSDUCER_MODULE.with(|x| x.clone()))
            .register_module(SYMBOL_MODULE.with(|x| x.clone()))
            .register_module(IO_MODULE.with(|x| x.clone()))
            .register_module(FS_MODULE.with(|x| x.clone()))
            .register_module(PORT_MODULE.with(|x| x.clone()))
            .register_module(META_MODULE.with(|x| x.clone()))
            .register_module(JSON_MODULE.with(|x| x.clone()))
            .register_module(CONSTANTS_MODULE.with(|x| x.clone()))
            .register_module(SYNTAX_MODULE.with(|x| x.clone()))
            .register_module(PROCESS_MODULE.with(|x| x.clone()))
            .register_module(RESULT_MODULE.with(|x| x.clone()))
            .register_module(OPTION_MODULE.with(|x| x.clone()))
            .register_module(TYPE_ID_MODULE.with(|x| x.clone()))
            .register_module(PRELUDE_MODULE.with(|x| x.clone()))
            .register_module(TIME_MODULE.with(|x| x.clone()))
            .register_module(RANDOM_MODULE.with(|x| x.clone()))
            .register_module(THREADING_MODULE.with(|x| x.clone()))
            .register_module(BYTEVECTOR_MODULE.with(|x| x.clone()));

        engine.register_module(GIT_MODULE.with(|x| x.clone()));

        if !sandbox {
            engine
                .register_module(TCP_MODULE.with(|x| x.clone()))
                .register_module(HTTP_MODULE.with(|x| x.clone()))
                .register_module(POLLING_MODULE.with(|x| x.clone()));
        } else {
            engine
                .register_module(FS_MODULE_SB.with(|x| x.clone()))
                .register_module(PORT_MODULE_WITHOUT_FILESYSTEM.with(|x| x.clone()))
                .register_module(SB_PRELUDE.with(|x| x.clone()));
        }

        #[cfg(feature = "dylibs")]
        engine.register_module(FFI_MODULE.with(|x| x.clone()));

        // Private module
        engine.register_module(MUTABLE_VECTOR_MODULE.with(|x| x.clone()));
        engine.register_module(PRIVATE_READER_MODULE.with(|x| x.clone()));

        engine.register_module(IMMUTABLE_VECTOR_MODULE.with(|x| x.clone()));
    }
}

pub static MODULE_IDENTIFIERS: Lazy<fxhash::FxHashSet<InternedString>> = Lazy::new(|| {
    let mut set = fxhash::FxHashSet::default();

    // TODO: Consolidate the prefixes and module names into one spot
    set.insert("%-builtin-module-stahl/hash".into());
    set.insert("%-builtin-module-stahl/sets".into());
    set.insert("%-builtin-module-stahl/lists".into());
    set.insert("%-builtin-module-stahl/strings".into());
    set.insert("%-builtin-module-stahl/vectors".into());
    set.insert("%-builtin-module-stahl/immutable-vectors".into());
    set.insert("%-builtin-module-stahl/streams".into());
    set.insert("%-builtin-module-stahl/identity".into());
    set.insert("%-builtin-module-stahl/numbers".into());
    set.insert("%-builtin-module-stahl/equality".into());
    set.insert("%-builtin-module-stahl/ord".into());
    set.insert("%-builtin-module-stahl/transducers".into());
    set.insert("%-builtin-module-stahl/io".into());
    set.insert("%-builtin-module-stahl/filesystem".into());
    set.insert("%-builtin-module-stahl/ports".into());
    set.insert("%-builtin-module-stahl/meta".into());
    set.insert("%-builtin-module-stahl/constants".into());
    set.insert("%-builtin-module-stahl/syntax".into());
    set.insert("%-builtin-module-stahl/process".into());
    set.insert("%-builtin-module-stahl/core/result".into());
    set.insert("%-builtin-module-stahl/core/option".into());
    set.insert("%-builtin-module-stahl/threads".into());
    set.insert("%-builtin-module-stahl/bytevectors".into());
    set.insert("%-builtin-module-stahl/base".into());

    set
});

pub(crate) static PRELUDE_TO_RESERVED_MAP: Lazy<FxHashMap<String, InternedString>> =
    Lazy::new(|| {
        PRELUDE_INTERNED_STRINGS.with(|x| {
            x.iter()
                .map(|x| {
                    (
                        x.resolve().to_string(),
                        (CompactString::new("#%prim.") + x.resolve()).into(),
                    )
                })
                .collect()
        })
    });

pub fn builtin_to_reserved(ident: &str) -> InternedString {
    if let Some(value) = PRELUDE_TO_RESERVED_MAP.get(ident) {
        *value
    } else {
        (CompactString::new("#%prim.") + ident).into()
    }
}

// TODO: Do the same for the single threaded version as well

pub(crate) fn constant_primitives(
) -> crate::values::HashMap<InternedString, StahlVal, FxBuildHasher> {
    #[cfg(feature = "sync")]
    {
        CONSTANT_PRIMITIVES.clone()
    }

    #[cfg(not(feature = "sync"))]
    {
        CONSTANT_PRIMITIVES.with(|x| x.clone())
    }
}

#[cfg(feature = "sync")]
pub static CONSTANT_PRIMITIVES: Lazy<
    crate::values::HashMap<InternedString, StahlVal, FxBuildHasher>,
> = Lazy::new(|| STAHL_PRELUDE_MODULE.constant_funcs());

#[cfg(not(feature = "sync"))]
thread_local! {
    pub static CONSTANT_PRIMITIVES: crate::values::HashMap<InternedString, StahlVal, FxBuildHasher> = {
        PRELUDE_MODULE.with(|x| x.constant_funcs())
    };

}

// TODO: Make the prelude string generation lazy - so that
// the first time we load (stahl/base) we don't have to regenerate
// the string. Probably just need a lazy static for loading 'stahl/base'
// and then reference that directly.
pub static ALL_MODULES: &str = r#"
    (require-builtin stahl/hash)
    (require-builtin stahl/sets)
    (require-builtin stahl/lists)
    (require-builtin stahl/strings)
    (require-builtin stahl/symbols)
    (require-builtin stahl/vectors)
    (require-builtin stahl/immutable-vectors)
    (require-builtin stahl/streams)
    (require-builtin stahl/identity)
    (require-builtin stahl/numbers)
    (require-builtin stahl/equality)
    (require-builtin stahl/ord)
    (require-builtin stahl/transducers)
    (require-builtin stahl/io)
    (require-builtin stahl/filesystem)
    (require-builtin stahl/ports)
    (require-builtin stahl/meta)
    (require-builtin stahl/json)
    (require-builtin stahl/constants)
    (require-builtin stahl/syntax)
    (require-builtin stahl/process)
    (require-builtin stahl/core/result)
    (require-builtin stahl/core/option)
    (require-builtin stahl/core/types)
    (require-builtin stahl/threads)
    (require-builtin stahl/bytevectors)

    (require-builtin stahl/hash as #%prim.)
    (require-builtin stahl/sets as #%prim.)
    (require-builtin stahl/lists as #%prim.)
    (require-builtin stahl/strings as #%prim.)
    (require-builtin stahl/symbols as #%prim.)
    (require-builtin stahl/vectors as #%prim.)
    (require-builtin stahl/immutable-vectors as #%prim.)
    (require-builtin stahl/streams as #%prim.)
    (require-builtin stahl/identity as #%prim.)
    (require-builtin stahl/numbers as #%prim.)
    (require-builtin stahl/equality as #%prim.)
    (require-builtin stahl/ord as #%prim.)
    (require-builtin stahl/transducers as #%prim.)
    (require-builtin stahl/io as #%prim.)
    (require-builtin stahl/filesystem as #%prim.)
    (require-builtin stahl/ports as #%prim.)
    (require-builtin stahl/meta as #%prim.)
    (require-builtin stahl/json as #%prim.)
    (require-builtin stahl/constants as #%prim.)
    (require-builtin stahl/syntax as #%prim.)
    (require-builtin stahl/process as #%prim.)
    (require-builtin stahl/core/result as #%prim.)
    (require-builtin stahl/core/option as #%prim.)
    (require-builtin stahl/core/types as #%prim.)
    (require-builtin stahl/threads as #%prim.)
    (require-builtin stahl/bytevectors as #%prim.)
"#;

pub static ALL_MODULES_RESERVED: &str = r#"
    (require-builtin stahl/hash as #%prim.)
    (require-builtin stahl/sets as #%prim.)
    (require-builtin stahl/lists as #%prim.)
    (require-builtin stahl/strings as #%prim.)
    (require-builtin stahl/symbols as #%prim.)
    (require-builtin stahl/vectors as #%prim.)
    (require-builtin stahl/immutable-vectors as #%prim.)
    (require-builtin stahl/streams as #%prim.)
    (require-builtin stahl/identity as #%prim.)
    (require-builtin stahl/numbers as #%prim.)
    (require-builtin stahl/equality as #%prim.)
    (require-builtin stahl/ord as #%prim.)
    (require-builtin stahl/transducers as #%prim.)
    (require-builtin stahl/io as #%prim.)
    (require-builtin stahl/filesystem as #%prim.)
    (require-builtin stahl/ports as #%prim.)
    (require-builtin stahl/meta as #%prim.)
    (require-builtin stahl/json as #%prim.)
    (require-builtin stahl/constants as #%prim.)
    (require-builtin stahl/syntax as #%prim.)
    (require-builtin stahl/process as #%prim.)
    (require-builtin stahl/core/result as #%prim.)
    (require-builtin stahl/core/option as #%prim.)
    (require-builtin stahl/core/types as #%prim.)
    (require-builtin stahl/threads as #%prim.)
    (require-builtin stahl/bytevectors as #%prim.)
"#;

pub static SANDBOXED_MODULES: &str = r#"
    (require-builtin stahl/hash)
    (require-builtin stahl/sets)
    (require-builtin stahl/lists)
    (require-builtin stahl/strings)
    (require-builtin stahl/symbols)
    (require-builtin stahl/vectors)
    (require-builtin stahl/immutable-vectors)
    (require-builtin stahl/streams)
    (require-builtin stahl/identity)
    (require-builtin stahl/numbers)
    (require-builtin stahl/equality)
    (require-builtin stahl/ord)
    (require-builtin stahl/transducers)
    (require-builtin stahl/io)
    (require-builtin stahl/meta)
    (require-builtin stahl/json)
    (require-builtin stahl/constants)
    (require-builtin stahl/syntax)
"#;

// TODO: Clean this up a lot
fn vector_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/vectors");
    module
        .register_native_fn_definition(MUT_VEC_CONSTRUCT_DEFINITION)
        .register_native_fn_definition(MUT_VEC_CONSTRUCT_VEC_DEFINITION)
        .register_native_fn_definition(MAKE_VECTOR_DEFINITION)
        .register_native_fn_definition(MUT_VEC_TO_LIST_DEFINITION)
        .register_native_fn_definition(VECTOR_FILL_DEFINITION)
        .register_native_fn_definition(MUT_VECTOR_COPY_DEFINITION)
        .register_value("vector-push!", VectorOperations::mut_vec_push())
        .register_native_fn_definition(MUT_VEC_LENGTH_DEFINITION)
        .register_native_fn_definition(VEC_LENGTH_DEFINITION)
        .register_value("vector-append!", VectorOperations::mut_vec_append())
        .register_value("mut-vector-ref", VectorOperations::mut_vec_get())
        .register_native_fn_definition(MUT_VEC_SET_DEFINITION)
        // Immutable vector operations
        .register_native_fn_definition(IMMUTABLE_VECTOR_CONSTRUCT_DEFINITION)
        .register_value("push-front", VectorOperations::vec_cons())
        .register_value("pop-front", VectorOperations::vec_car())
        .register_value("vec-rest", VectorOperations::vec_cdr())
        .register_value("null?", VectorOperations::list_vec_null())
        .register_value("push", VectorOperations::vec_push())
        .register_value("range-vec", VectorOperations::vec_range())
        .register_value("vec-append", VectorOperations::vec_append())
        // TODO: This has to be cleaned up
        .register_value("vector-ref", VectorOperations::vec_ref())
        .register_native_fn_definition(MUTABLE_VECTOR_CLEAR_DEFINITION)
        .register_native_fn_definition(MUTABLE_VECTOR_TO_STRING_DEFINITION)
        .register_native_fn_definition(MUTABLE_VECTOR_POP_DEFINITION);
    module
}

#[stahl_derive::function(name = "not", constant = true)]
fn not(value: &StahlVal) -> bool {
    matches!(value, StahlVal::BoolV(false))
}

#[stahl_derive::function(name = "string?", constant = true)]
fn stringp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::StringV(_))
}

#[stahl_derive::function(name = "list?", constant = true)]
fn listp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::ListV(_))
}

#[stahl_derive::function(name = "vector?", constant = true)]
fn vectorp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::VectorV(_) | StahlVal::MutableVector(_))
}

#[stahl_derive::function(name = "symbol?", constant = true)]
fn symbolp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::SymbolV(_))
}

#[stahl_derive::function(name = "hash?", constant = true)]
fn hashp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::HashMapV(_))
}

#[stahl_derive::function(name = "set?", constant = true)]
fn hashsetp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::HashSetV(_))
}

#[stahl_derive::function(name = "continuation?", constant = true)]
fn continuationp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::ContinuationFunction(_))
}

#[stahl_derive::function(name = "boolean?", constant = true)]
fn booleanp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::BoolV(_))
}

#[stahl_derive::function(name = "bool?", constant = true)]
fn boolp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::BoolV(_))
}

#[stahl_derive::function(name = "void?", constant = true)]
fn voidp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::Void)
}

#[stahl_derive::function(name = "struct?", constant = true)]
fn structp(value: &StahlVal) -> bool {
    if let StahlVal::CustomStruct(s) = value {
        s.is_transparent()
    } else {
        false
    }
}

#[stahl_derive::function(name = "port?", constant = true)]
fn portp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::PortV(..))
}

#[stahl_derive::function(name = "#%private-struct?", constant = true)]
fn private_structp(value: &StahlVal) -> bool {
    matches!(value, StahlVal::CustomStruct(_))
}

#[stahl_derive::function(name = "error-object?", constant = true)]
fn error_objectp(value: &StahlVal) -> bool {
    let StahlVal::Custom(val) = value else {
        return false;
    };

    // let cell: &RefCell<_> = &*val;
    as_underlying_type::<StahlErr>(val.read().as_ref()).is_some()
}

#[stahl_derive::function(name = "function?", constant = true)]
fn functionp(value: &StahlVal) -> bool {
    matches!(
        value,
        StahlVal::Closure(_)
            | StahlVal::FuncV(_)
            // | StahlVal::ContractedFunction(_)
            | StahlVal::BoxedFunction(_)
            | StahlVal::ContinuationFunction(_)
            | StahlVal::MutFunc(_)
            | StahlVal::BuiltIn(_)
    )
}

#[stahl_derive::function(name = "procedure?", constant = true)]
fn procedurep(value: &StahlVal) -> bool {
    if let StahlVal::CustomStruct(s) = value {
        return s.maybe_proc().map(|x| procedurep(x)).unwrap_or(false);
    }

    matches!(
        value,
        StahlVal::Closure(_)
            | StahlVal::FuncV(_)
            | StahlVal::BoxedFunction(_)
            | StahlVal::ContinuationFunction(_)
            | StahlVal::MutFunc(_)
            | StahlVal::BuiltIn(_)
    )
}
fn identity_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/identity");
    module
        .register_native_fn_definition(NOT_DEFINITION)
        .register_native_fn_definition(numbers::COMPLEXP_DEFINITION)
        .register_native_fn_definition(numbers::EXACT_INTEGERP_DEFINITION)
        .register_native_fn_definition(numbers::FLOATP_DEFINITION)
        .register_native_fn_definition(numbers::INTEGERP_DEFINITION)
        .register_native_fn_definition(numbers::INTP_DEFINITION)
        .register_native_fn_definition(numbers::NUMBERP_DEFINITION)
        .register_native_fn_definition(numbers::RATIONALP_DEFINITION)
        .register_native_fn_definition(numbers::REALP_DEFINITION)
        .register_native_fn_definition(STRINGP_DEFINITION)
        .register_native_fn_definition(LISTP_DEFINITION)
        .register_native_fn_definition(VECTORP_DEFINITION)
        .register_native_fn_definition(SYMBOLP_DEFINITION)
        .register_native_fn_definition(HASHP_DEFINITION)
        .register_native_fn_definition(HASHSETP_DEFINITION)
        .register_native_fn_definition(CONTINUATIONP_DEFINITION)
        .register_native_fn_definition(BOOLEANP_DEFINITION)
        .register_native_fn_definition(BOOLP_DEFINITION)
        .register_native_fn_definition(VOIDP_DEFINITION)
        .register_native_fn_definition(STRUCTP_DEFINITION)
        .register_native_fn_definition(PORTP_DEFINITION)
        .register_native_fn_definition(EOF_OBJECTP_DEFINITION)
        .register_native_fn_definition(PRIVATE_STRUCTP_DEFINITION)
        .register_native_fn_definition(ERROR_OBJECTP_DEFINITION)
        .register_value("mutable-vector?", gen_pred!(MutableVector))
        .register_value("immutable-vector?", gen_pred!(VectorV))
        .register_value("char?", gen_pred!(CharV))
        .register_value("future?", gen_pred!(FutureV))
        .register_native_fn_definition(FUNCTIONP_DEFINITION)
        .register_native_fn_definition(PROCEDUREP_DEFINITION)
        .register_value(
            "atom?",
            gen_pred!(NumV, IntV, StringV, SymbolV, BoolV, CharV),
        );
    module
}

fn stream_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/streams");
    module
        .register_value("stream-cons", StreamOperations::stream_cons())
        .register_value("empty-stream", StreamOperations::empty_stream())
        .register_value("stream-empty?", StreamOperations::stream_empty_huh())
        .register_value("stream-car", StreamOperations::stream_car())
        .register_value("#%stream-cdr", StreamOperations::stream_cdr());
    module
}

// fn contract_module() -> BuiltInModule {
//     let mut module = BuiltInModule::new("stahl/contracts");
//     module
//         .register_value("bind/c", contracts::BIND_CONTRACT_TO_FUNCTION)
//         .register_value("make-flat/c", contracts::MAKE_FLAT_CONTRACT)
//         .register_value(
//             "make-dependent-function/c",
//             contracts::MAKE_DEPENDENT_CONTRACT,
//         )
//         .register_value("make-function/c", contracts::MAKE_FUNCTION_CONTRACT)
//         .register_value("make/c", contracts::MAKE_C);

//     module
// }

fn number_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/numbers");
    module
        .register_native_fn_definition(numbers::ADD_PRIMITIVE_DEFINITION)
        .register_native_fn_definition(numbers::FLOAT_ADD_DEFINITION)
        .register_native_fn_definition(numbers::MULTIPLY_PRIMITIVE_DEFINITION)
        .register_native_fn_definition(numbers::DIVIDE_PRIMITIVE_DEFINITION)
        .register_native_fn_definition(numbers::SUBTRACT_PRIMITIVE_DEFINITION)
        .register_native_fn_definition(numbers::EVEN_DEFINITION)
        .register_native_fn_definition(numbers::ODD_DEFINITION)
        .register_native_fn_definition(numbers::ARITHMETIC_SHIFT_DEFINITION)
        .register_native_fn_definition(numbers::ABS_DEFINITION)
        .register_native_fn_definition(numbers::NANP_DEFINITION)
        .register_native_fn_definition(numbers::ZEROP_DEFINITION)
        .register_native_fn_definition(numbers::POSITIVEP_DEFINITION)
        .register_native_fn_definition(numbers::NEGATIVEP_DEFINITION)
        .register_native_fn_definition(numbers::CEILING_DEFINITION)
        .register_native_fn_definition(numbers::DENOMINATOR_DEFINITION)
        .register_native_fn_definition(numbers::EXACTP_DEFINITION)
        .register_native_fn_definition(numbers::EXACT_DEFINITION)
        .register_native_fn_definition(numbers::EXACT_TO_INEXACT_DEFINITION)
        .register_native_fn_definition(numbers::EXACT_INTEGER_SQRT_DEFINITION)
        .register_native_fn_definition(numbers::EXPT_DEFINITION)
        .register_native_fn_definition(numbers::EXP_DEFINITION)
        .register_native_fn_definition(numbers::FINITEP_DEFINITION)
        .register_native_fn_definition(numbers::FLOOR_DEFINITION)
        .register_native_fn_definition(numbers::INEXACTP_DEFINITION)
        .register_native_fn_definition(numbers::INEXACT_TO_EXACT_DEFINITION)
        .register_native_fn_definition(numbers::INFINITEP_DEFINITION)
        .register_native_fn_definition(numbers::LOG_DEFINITION)
        .register_native_fn_definition(numbers::MAGNITUDE_DEFINITION)
        .register_native_fn_definition(numbers::REAL_PART_DEFINITION)
        .register_native_fn_definition(numbers::IMAG_PART_DEFINITION)
        .register_native_fn_definition(numbers::NUMERATOR_DEFINITION)
        .register_native_fn_definition(numbers::QUOTIENT_DEFINITION)
        .register_native_fn_definition(numbers::MODULO_DEFINITION)
        .register_native_fn_definition(numbers::REMAINDER_DEFINITION)
        .register_native_fn_definition(numbers::ROUND_DEFINITION)
        .register_native_fn_definition(numbers::SQUARE_DEFINITION)
        .register_native_fn_definition(numbers::SQRT_DEFINITION)
        .register_native_fn_definition(numbers::SIN_DEFINITION)
        .register_native_fn_definition(numbers::COS_DEFINITION)
        .register_native_fn_definition(numbers::TAN_DEFINITION)
        .register_native_fn_definition(numbers::ASIN_DEFINITION)
        .register_native_fn_definition(numbers::ACOS_DEFINITION)
        .register_native_fn_definition(numbers::ATAN_DEFINITION);

    module
}

#[inline(always)]
pub fn equality_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    Ok(StahlVal::BoolV(args.windows(2).all(|x| x[0] == x[1])))
}

pub fn gte_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    if args.is_empty() {
        stop!(ArityMismatch => "expected at least one argument");
    }

    Ok(StahlVal::BoolV(args.windows(2).all(|x| {
        x[0].partial_cmp(&x[1])
            .map(|x| x != Ordering::Less)
            .unwrap_or(false)
    })))
}

#[inline(always)]
pub fn lte_primitive(args: &[StahlVal]) -> Result<StahlVal> {
    if args.is_empty() {
        stop!(ArityMismatch => "expected at least one argument");
    }

    Ok(StahlVal::BoolV(args.windows(2).all(|x| {
        x[0].partial_cmp(&x[1])
            .map(|x| x != Ordering::Greater)
            .unwrap_or(false)
    })))
}

fn equality_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/equality");
    module
        .register_value(
            "equal?",
            StahlVal::FuncV(ensure_tonicity_two!(|a, b| a == b)),
        )
        .register_value(
            "eqv?",
            StahlVal::FuncV(ensure_tonicity_two!(
                |a: &StahlVal, b: &StahlVal| a.ptr_eq(b)
            )),
        )
        .register_value(
            "eq?",
            StahlVal::FuncV(ensure_tonicity_two!(
                |a: &StahlVal, b: &StahlVal| a.ptr_eq(b)
            )),
        )
        .register_native_fn_definition(NUMBER_EQUALITY_DEFINITION);

    // TODO: Replace this with just numeric equality!
    // .register_value("=", StahlVal::FuncV(ensure_tonicity_two!(|a, b| a == b)));

    module
}

/// Real numbers ordering module.
#[stahl_derive::define_module(name = "stahl/ord")]
fn ord_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/ord");

    fn ensure_real(x: &StahlVal) -> Result<&StahlVal> {
        realp(x).then(|| x).ok_or_else(|| {
            StahlErr::new(ErrorKind::TypeMismatch, "expected real numbers".to_owned())
        })
    }

    fn ord_internal(
        args: &[StahlVal],
        ordering_f: impl Fn(Option<Ordering>) -> bool,
    ) -> Result<StahlVal> {
        match args {
            [x] => {
                ensure_real(x)?;
                Ok(StahlVal::BoolV(true))
            }
            [x, rest @ ..] => {
                let mut left = ensure_real(x)?;
                for r in rest {
                    let right = ensure_real(r)?;
                    if !ordering_f(left.partial_cmp(right)) {
                        return Ok(StahlVal::BoolV(false));
                    }
                    left = right;
                }
                Ok(StahlVal::BoolV(true))
            }
            _ => stop!(ArityMismatch => "expected at least one argument"),
        }
    }

    /// Compares real numbers to check if any number is greater than the subsequent.
    ///
    /// (> x . rest) -> bool?
    ///
    /// * x : real? - The first real number to compare.
    /// * rest : real? - The rest of the numbers to compare.
    ///
    /// # Examples
    /// ```scheme
    /// > (> 1) ;; => #t
    /// > (> 3 2) ;; => #t
    /// > (> 1 1) ;; => #f
    /// > (> 3/2 1.5) ;; => #f
    /// > (> 3/2 1.4) ;; => #t
    /// > (> 3 4/2 1) ;; #t
    /// ```
    #[stahl_derive::native(name = ">", arity = "AtLeast(1)")]
    fn greater_than(args: &[StahlVal]) -> Result<StahlVal> {
        ord_internal(args, |o| matches!(o, Some(Ordering::Greater)))
    }

    /// Compares real numbers to check if any number is greater than or equal than the subsequent.
    ///
    /// (>= x . rest) -> bool?
    ///
    /// * x : real? - The first real number to compare.
    /// * rest : real? - The rest of the numbers to compare.
    ///
    /// # Examples
    /// ```scheme
    /// > (>= 1) ;; => #t
    /// > (>= 3 2) ;; => #t
    /// > (>= 2 3) ;; => #f
    /// > (>= 3/2 1.5) ;; => #t
    /// > (>= 3/2 1.4) ;; => #t
    /// > (>= 2 4/2 1) ;; #t
    /// ```
    #[stahl_derive::native(name = ">=", arity = "AtLeast(1)")]
    fn greater_than_equal(args: &[StahlVal]) -> Result<StahlVal> {
        ord_internal(args, |o| {
            matches!(o, Some(Ordering::Greater | Ordering::Equal))
        })
    }

    /// Compares real numbers to check if any number is less than the subsequent.
    ///
    /// (< x . rest) -> bool?
    ///
    /// * x : real? - The first real number to compare.
    /// * rest : real? - The rest of the numbers to compare.
    ///
    /// # Examples
    /// ```scheme
    /// > (< 1) ;; => #t
    /// > (< 3 2) ;; => #f
    /// > (< 2 3) ;; => #t
    /// > (< 3/2 1.5) ;; => #f
    /// > (< 2.5 3/2) ;; => #t
    /// > (< 2 5/2 3) ;; #t
    /// ```
    #[stahl_derive::native(name = "<", arity = "AtLeast(1)")]
    fn less_than(args: &[StahlVal]) -> Result<StahlVal> {
        ord_internal(args, |o| matches!(o, Some(Ordering::Less)))
    }

    /// Compares real numbers to check if any number is less than or equal than the subsequent.
    ///
    /// (<= x . rest) -> bool?
    ///
    /// * x : real? - The first real number to compare.
    /// * rest : real? - The rest of the numbers to compare.
    ///
    /// # Examples
    /// ```scheme
    /// > (<= 1) ;; => #t
    /// > (<= 3 2) ;; => #f
    /// > (<= 2 3) ;; => #t
    /// > (<= 3/2 1.5) ;; => #t
    /// > (<= 2.5 3/2) ;; => #f
    /// > (<= 2 6/2 3) ;; #t
    /// ```
    #[stahl_derive::native(name = "<=", arity = "AtLeast(1)")]
    fn less_than_equal(args: &[StahlVal]) -> Result<StahlVal> {
        ord_internal(args, |o| {
            matches!(o, Some(Ordering::Less | Ordering::Equal))
        })
    }

    module
        .register_native_fn_definition(GREATER_THAN_DEFINITION)
        .register_native_fn_definition(GREATER_THAN_EQUAL_DEFINITION)
        .register_native_fn_definition(LESS_THAN_DEFINITION)
        .register_native_fn_definition(LESS_THAN_EQUAL_DEFINITION);
    module
}

pub fn transducer_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/transducers");

    use crate::primitives::transducers::*;

    module
        .register_native_fn("compose", compose, Arity::AtLeast(0))
        .register_native_fn("mapping", map, Arity::Exact(1))
        .register_native_fn("flattening", flatten, Arity::Exact(0))
        .register_native_fn("flat-mapping", flat_map, Arity::Exact(1))
        .register_native_fn("filtering", filter, Arity::Exact(1))
        .register_native_fn("taking", take, Arity::Exact(1))
        .register_native_fn("dropping", dropping, Arity::Exact(1))
        .register_native_fn("extending", extending, Arity::Exact(1))
        .register_native_fn("enumerating", enumerating, Arity::Exact(0))
        .register_native_fn("zipping", zipping, Arity::Exact(1))
        .register_native_fn("interleaving", interleaving, Arity::Exact(1))
        .register_value("into-sum", crate::values::transducers::INTO_SUM)
        .register_value("into-product", crate::values::transducers::INTO_PRODUCT)
        .register_value("into-max", crate::values::transducers::INTO_MAX)
        .register_value("into-min", crate::values::transducers::INTO_MIN)
        .register_value("into-count", crate::values::transducers::INTO_COUNT)
        .register_value("into-list", crate::values::transducers::INTO_LIST)
        .register_value("into-vector", crate::values::transducers::INTO_VECTOR)
        .register_value("into-hashmap", crate::values::transducers::INTO_HASHMAP)
        .register_value("into-hashset", crate::values::transducers::INTO_HASHSET)
        .register_value("into-string", crate::values::transducers::INTO_STRING)
        .register_value("into-last", crate::values::transducers::INTO_LAST)
        .register_value("into-for-each", crate::values::transducers::FOR_EACH)
        .register_value("into-nth", crate::values::transducers::NTH)
        .register_value("into-reducer", crate::values::transducers::REDUCER);
    module
}

fn io_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/io");
    module
        .register_value("stdout-simple-displayln", IoFunctions::displayln())
        .register_value("read-to-string", IoFunctions::read_to_string());

    module
}

pub const VOID_DOC: MarkdownDoc = MarkdownDoc::from_str(
    "The void value, returned by many forms with side effects, such as `define`.",
);

/// Miscellaneous constants
#[stahl_derive::define_module(name = "stahl/constants")]
fn constants_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/constants");
    module.register_value("void", StahlVal::Void);
    module.register_doc("void", VOID_DOC);
    module
}

fn get_environment_variable(var: String) -> Result<StahlVal> {
    std::env::var(var)
        .map(|x| x.into_stahlval().unwrap())
        .map_err(|x| StahlErr::new(ErrorKind::Generic, x.to_string()))
}

fn maybe_get_environment_variable(var: String) -> StahlResult<StahlVal, StahlErr> {
    get_environment_variable(var).into()
}

fn sandboxed_meta_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/meta");
    module
        // .register_value("assert!", MetaOperations::assert_truthy())
        .register_value("active-object-count", MetaOperations::active_objects())
        // .register_value("memory-address", MetaOperations::memory_address())
        // .register_value("async-exec", MetaOperations::exec_async())
        // .register_value("poll!", MetaOperations::poll_value())
        // .register_value("block-on", MetaOperations::block_on())
        // .register_value("join!", MetaOperations::join_futures())
        // .register_value("struct-ref", struct_ref())
        // .register_value("struct->list", struct_to_list())
        // .register_value("struct->vector", struct_to_vector())
        .register_fn("value->string", super::meta::value_to_string)
        // .register_value("expand!", StahlVal::FuncV(super::meta::expand_macros))
        // .register_value("read!", StahlVal::FuncV(super::meta::read))
        // .register_value("eval!", StahlVal::FuncV(super::meta::eval))
        // TODO: @Matt -> implement the traits for modules as well
        // .register_fn("Engine::new", super::meta::EngineWrapper::new)
        .register_fn("eval!", super::meta::eval)
        .register_fn("value->iterator", crate::rvals::value_into_iterator)
        .register_value("iter-next!", StahlVal::FuncV(crate::rvals::iterator_next));
    // .register_fn("run!", super::meta::EngineWrapper::call)
    // .register_fn("get-value", super::meta::EngineWrapper::get_value)
    // .register_fn("env-var", get_environment_variable);
    module
}

/// Returns the message of an error object.
///
/// (error-object-message error?) -> string?
#[stahl_derive::function(name = "error-object-message")]
fn error_object_message(val: &StahlVal) -> Result<StahlVal> {
    let StahlVal::Custom(custom) = val else {
        stop!(TypeMismatch => "error-object-message: expected an error object");
    };

    let value = custom.read();

    let Some(error) = as_underlying_type::<StahlErr>(value.as_ref()) else {
        stop!(TypeMismatch => "error-object-message: expected an error object");
    };

    Ok(error.message().to_string().into())
}

fn lookup_function_name(value: StahlVal) -> Option<StahlVal> {
    match value {
        StahlVal::BoxedFunction(f) => f.name().map(|x| x.into_stahlval().unwrap()),
        StahlVal::FuncV(f) => get_function_name(f).map(|x| x.name.into_stahlval().unwrap()),
        StahlVal::BuiltIn(f) => {
            get_function_metadata(super::builtin::BuiltInFunctionType::Context(f as _))
                .map(|x| x.name().into_stahlval().unwrap())
        }
        StahlVal::MutFunc(f) => {
            get_function_metadata(super::builtin::BuiltInFunctionType::Mutable(f as _))
                .map(|x| x.name().into_stahlval().unwrap())
        }
        _ => None,
    }
}

fn lookup_doc(value: StahlVal) -> bool {
    match value {
        // StahlVal::BoxedFunction(f) => ,
        StahlVal::FuncV(f) => {
            let metadata = get_function_metadata(BuiltInFunctionType::Reference(f));

            if let Some(data) = metadata.and_then(|x| x.doc) {
                println!("{}", data);
                true
            } else {
                false
            }
        }
        StahlVal::MutFunc(f) => {
            let metadata = get_function_metadata(BuiltInFunctionType::Mutable(f));
            if let Some(data) = metadata.and_then(|x| x.doc) {
                println!("{}", data);
                true
            } else {
                false
            }
        }
        StahlVal::BuiltIn(f) => {
            let metadata = get_function_metadata(BuiltInFunctionType::Context(f));
            if let Some(data) = metadata.and_then(|x| x.doc) {
                println!("{}", data);
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

// Only works with fixed size arity functions
fn arity(value: StahlVal) -> UnRecoverableResult {
    match value {
        StahlVal::Closure(c) => {
            // Ok(StahlVal::IntV(c.arity() as isize)).into()

            if let Some(StahlVal::CustomStruct(s)) = c.get_contract_information() {
                let guard = s;
                if guard.name().resolve() == "FunctionContract" {
                    if let StahlVal::ListV(l) = &guard.fields[0] {
                        Ok(StahlVal::IntV(l.len() as isize)).into()
                    } else {
                        stahlerr!(TypeMismatch => "Unable to find the arity for the given function")
                            .into()
                    }
                } else if guard.name().resolve() == "FlatContract" {
                    Ok(StahlVal::IntV(1)).into()
                } else {
                    // This really shouldn't happen
                    Ok(StahlVal::IntV(c.arity() as isize)).into()
                }
            } else {
                Ok(StahlVal::IntV(c.arity() as isize)).into()
            }
        }
        StahlVal::BoxedFunction(f) => f
            .get_arity()
            .map(|x| StahlVal::IntV(x as isize))
            .ok_or(StahlErr::new(
                ErrorKind::TypeMismatch,
                "Unable to find the arity for the given function".to_string(),
            ))
            // .ok_or(stahlerr!(TypeMismatch => "Unable to find the arity for the give function"))
            .into(),

        // Lookup the function signature metadata, return the arity payload
        StahlVal::FuncV(f) => {
            let metadata = get_function_name(f);

            metadata
                .map(|x| x.arity)
                .ok_or(StahlErr::new(
                    ErrorKind::TypeMismatch,
                    "Unable to find the arity for the given function".to_string(),
                ))
                .and_then(|x| x.into_stahlval())
                .into()
        }

        // Ok(StahlVal::IntV(f.get_arity()))
        _ => stahlerr!(TypeMismatch => "Unable to find the arity for the given function").into(),
    }
}

// Only works with fixed size arity functions
fn is_multi_arity(value: StahlVal) -> UnRecoverableResult {
    match value {
        StahlVal::Closure(c) => Ok(StahlVal::BoolV(c.is_multi_arity)).into(),
        _ => stahlerr!(TypeMismatch => "Unable to find the arity for the given function").into(),
    }
}

struct MutableVector {
    vector: Vec<StahlVal>,
}

impl MutableVector {
    fn new() -> Self {
        Self { vector: Vec::new() }
    }

    fn vector_push(&mut self, value: StahlVal) {
        self.vector.push(value);
    }

    fn vector_pop(&mut self) -> Option<StahlVal> {
        self.vector.pop()
    }

    fn vector_set(&mut self, index: usize, value: StahlVal) {
        self.vector[index] = value;
    }

    fn vector_ref(&self, index: usize) -> StahlVal {
        self.vector[index].clone()
    }

    fn vector_len(&self) -> usize {
        self.vector.len()
    }

    fn vector_to_list(&self) -> StahlVal {
        StahlVal::ListV(self.vector.clone().into())
    }

    fn vector_is_empty(&self) -> bool {
        self.vector.is_empty()
    }

    fn vector_from_list(lst: List<StahlVal>) -> Self {
        Self {
            vector: lst.into_iter().collect(),
        }
    }
}

impl crate::rvals::Custom for MutableVector {
    fn gc_visit_children(&self, context: &mut crate::values::closed::MarkAndSweepContext) {
        for value in &self.vector {
            context.push_back(value.clone());
        }
    }

    fn visit_equality(&self, visitor: &mut crate::rvals::cycles::EqualityVisitor) {
        for value in &self.vector {
            visitor.push_back(value.clone());
        }
    }

    // Compare the two for equality otherwise
    fn equality_hint(&self, other: &dyn crate::rvals::CustomType) -> bool {
        if let Some(other) = as_underlying_type::<MutableVector>(other) {
            self.vector.len() == other.vector.len()
        } else {
            false
        }
    }
}

struct Reader {
    buffer: String,
    offset: usize,
}

impl crate::rvals::Custom for Reader {}

impl Reader {
    fn create_reader() -> Reader {
        Self {
            buffer: String::new(),
            offset: 0,
        }
    }

    fn push_string(&mut self, input: crate::rvals::StahlString) {
        self.buffer.push_str(input.as_str());
    }

    fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    fn read_one_impl(&mut self, finisher: fn(ExprKind) -> Result<StahlVal>) -> Result<StahlVal> {
        if let Some(buffer) = self.buffer.get(self.offset..) {
            let mut parser = crate::parser::parser::Parser::new_flat(buffer, SourceId::none());

            if let Some(raw) = parser.next() {
                let next = if let Ok(next) = raw {
                    next
                } else {
                    return Ok(StahlVal::Void);
                };

                self.offset += parser.offset();

                let result = finisher(next);

                if let Some(remaining) = self.buffer.get(self.offset..) {
                    for _ in remaining.chars().take_while(|x| x.is_whitespace()) {
                        self.offset += 1;
                    }
                }

                if self.offset == self.buffer.len() {
                    self.buffer.clear();
                    self.offset = 0;
                }

                result
            } else {
                // No value, keep reading
                Ok(StahlVal::Void)
            }
        } else {
            // TODO: This needs to get fixed
            Ok(crate::primitives::ports::eof())
        }
    }

    fn read_one(&mut self) -> Result<StahlVal> {
        self.read_one_impl(TryFromExprKindForStahlVal::try_from_expr_kind_quoted)
    }

    fn read_one_syntax_object(&mut self) -> Result<StahlVal> {
        self.read_one_impl(
            crate::parser::tryfrom_visitor::SyntaxObjectFromExprKind::try_from_expr_kind,
        )
    }
}

fn reader_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("#%private/stahl/reader");

    module
        .register_fn("new-reader", Reader::create_reader)
        .register_fn("reader-push-string", Reader::push_string)
        .register_fn("reader-read-one", Reader::read_one)
        .register_fn("reader-empty?", Reader::is_empty)
        .register_fn(
            "reader-read-one-syntax-object",
            Reader::read_one_syntax_object,
        );

    module
}

fn mutable_vector_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("#%private/stahl/mvector");

    module
        .register_fn("make-mutable-vector", MutableVector::new)
        .register_fn("mutable-vector-ref", MutableVector::vector_ref)
        .register_fn("mutable-vector-set!", MutableVector::vector_set)
        .register_fn("mutable-vector-pop!", MutableVector::vector_pop)
        .register_fn("mutable-vector-push!", MutableVector::vector_push)
        .register_fn("mutable-vector-len", MutableVector::vector_len)
        .register_fn("mutable-vector->list", MutableVector::vector_to_list)
        .register_fn("mutable-vector-empty?", MutableVector::vector_is_empty)
        .register_fn("mutable-vector-from-list", MutableVector::vector_from_list);

    module
}

#[stahl_derive::function(name = "#%unbox")]
pub fn unbox_mutable(value: &HeapRef<StahlVal>) -> StahlVal {
    value.get()
}

#[stahl_derive::function(name = "#%set-box!")]
pub fn set_box_mutable(value: &HeapRef<StahlVal>, update: StahlVal) -> StahlVal {
    value.set_and_return(update)
}

#[stahl_derive::function(name = "unbox")]
pub fn plain_unbox_mutable(value: &HeapRef<StahlVal>) -> StahlVal {
    value.get()
}

#[stahl_derive::function(name = "set-box!")]
pub fn plain_set_box_mutable(value: &HeapRef<StahlVal>, update: StahlVal) -> StahlVal {
    value.set_and_return(update)
}

fn gc_collection(ctx: &mut VmCore, args: &[StahlVal]) -> Option<Result<StahlVal>> {
    if args.len() != 0 {
        return Some(Err(
            throw!(ArityMismatch => "gc-collect expects 0 arguments, found: {}", args.len())(),
        ));
    }

    let count = ctx.gc_collect();

    Some(Ok(StahlVal::IntV(count as _)))
}

fn make_mutable_box(ctx: &mut VmCore, args: &[StahlVal]) -> Option<Result<StahlVal>> {
    if args.len() != 1 {
        return Some(Err(
            throw!(ArityMismatch => "box expects one argument, found: {}", args.len())(),
        ));
    }

    let allocated_var = ctx.thread.heap.lock().unwrap().allocate(
        args[0].clone(), // TODO: Could actually move off of the stack entirely
        &ctx.thread.stack,
        ctx.thread.stack_frames.iter().map(|x| x.function.as_ref()),
        ctx.thread.global_env.roots().as_slice(),
        &ctx.thread.thread_local_storage,
        &mut ctx.thread.synchronizer,
    );

    Some(Ok(StahlVal::HeapAllocated(allocated_var)))
}

#[stahl_derive::function(name = "unbox-strong")]
pub fn unbox(value: &GcMut<StahlVal>) -> StahlVal {
    value.read().clone()
}

#[stahl_derive::function(name = "set-strong-box!")]
pub fn set_box(value: &GcMut<StahlVal>, update_to: StahlVal) {
    *value.write() = update_to;
}

pub fn black_box(_: &[StahlVal]) -> Result<StahlVal> {
    Ok(StahlVal::Void)
}

#[stahl_derive::function(name = "struct->list")]
pub fn struct_to_list(value: &UserDefinedStruct) -> Result<StahlVal> {
    if value.is_transparent() {
        // Ok(StahlVal::ListV((*value.fields).clone().into()))
        Ok(StahlVal::ListV((*value.fields).iter().cloned().collect()))
    } else {
        Ok(StahlVal::BoolV(false))
    }
}

fn meta_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/meta");
    module
        .register_value("#%black-box", StahlVal::FuncV(black_box))
        .register_value(
            "#%function-ptr-table",
            LambdaMetadataTable::new().into_stahlval().unwrap(),
        )
        .register_fn("#%function-ptr-table-add", LambdaMetadataTable::add)
        .register_fn("#%function-ptr-table-get", LambdaMetadataTable::get)
        .register_fn("#%private-cycle-collector", StahlCycleCollector::from_root)
        .register_fn("#%private-cycle-collector-get", StahlCycleCollector::get)
        .register_fn(
            "#%private-cycle-collector-values",
            StahlCycleCollector::values,
        )
        .register_value("assert!", MetaOperations::assert_truthy())
        .register_value("active-object-count", MetaOperations::active_objects())
        .register_value("memory-address", MetaOperations::memory_address())
        // .register_value("async-exec", MetaOperations::exec_async())
        .register_value("poll!", MetaOperations::poll_value())
        .register_value("block-on", MetaOperations::block_on())
        .register_value(
            "local-executor/block-on",
            MetaOperations::block_on_with_local_executor(),
        )
        .register_value("join!", MetaOperations::join_futures())
        .register_fn(
            "#%struct-property-ref",
            |value: &UserDefinedStruct, key: StahlVal| UserDefinedStruct::get(value, &key),
        )
        .register_native_fn_definition(STRUCT_TO_LIST_DEFINITION)
        .register_value("expand!", StahlVal::FuncV(super::meta::expand_macros))
        .register_value("read!", StahlVal::FuncV(super::meta::read))
        .register_value(
            "current-function-span",
            StahlVal::BuiltIn(super::vm::current_function_span),
        )
        .register_value("error-with-span", error_with_src_loc())
        .register_value("raise-error-with-span", error_from_error_with_span())
        .register_value("raise-error", raise_error_from_error())
        .register_native_fn_definition(CALL_CC_DEFINITION)
        .register_native_fn_definition(EVAL_DEFINITION)
        .register_native_fn_definition(EVAL_FILE_DEFINITION)
        .register_native_fn_definition(EXPAND_SYNTAX_OBJECTS_DEFINITION)
        .register_native_fn_definition(MATCH_SYNTAX_CASE_DEFINITION)
        .register_native_fn_definition(EXPAND_SYNTAX_CASE_DEFINITION)
        .register_native_fn_definition(MACRO_CASE_BINDINGS_DEFINITION)
        .register_native_fn_definition(EVAL_STRING_DEFINITION)
        .register_native_fn_definition(CALL_WITH_EXCEPTION_HANDLER_DEFINITION)
        .register_value("breakpoint!", StahlVal::BuiltIn(super::vm::breakpoint))
        .register_native_fn_definition(INSPECT_DEFINITION)
        // TODO: Come back to this
        .register_native_fn_definition(super::vm::EMIT_EXPANDED_FILE_DEFINITION)
        .register_native_fn_definition(super::vm::LOAD_EXPANDED_FILE_DEFINITION)
        .register_value(
            "#%environment-length",
            StahlVal::BuiltIn(super::vm::environment_offset),
        )
        .register_value(
            "call-with-current-continuation",
            StahlVal::BuiltIn(super::vm::call_cc),
        )
        .register_fn("eval!", super::meta::eval)
        .register_fn("value->string", super::meta::value_to_string)
        // TODO: @Matt -> implement the traits for modules as well
        .register_fn("Engine::new", super::meta::EngineWrapper::new)
        .register_fn("Engine::clone", super::meta::EngineWrapper::deep_copy)
        .register_fn("Engine::add-module", super::meta::EngineWrapper::add_module)
        .register_fn("Engine::modules->list", super::meta::EngineWrapper::modules)
        .register_fn(
            "Engine::raise_error",
            super::meta::EngineWrapper::raise_error,
        )
        .register_value("set-test-mode!", StahlVal::BuiltIn(set_test_mode))
        .register_value("get-test-mode", StahlVal::BuiltIn(get_test_mode))
        .register_fn("run!", super::meta::EngineWrapper::call)
        // .register_fn("get-value", super::meta::EngineWrapper::get_value)
        .register_fn("value->iterator", crate::rvals::value_into_iterator)
        .register_value("iter-next!", StahlVal::FuncV(crate::rvals::iterator_next))
        // Check whether the iterator is done
        .register_value("#%iterator-finished", ITERATOR_FINISHED.with(|x| x.clone()))
        .register_value("%iterator?", gen_pred!(BoxedIterator))
        .register_fn("env-var", get_environment_variable)
        .register_fn("maybe-get-env-var", maybe_get_environment_variable)
        // TODO: Maybe just remove this, or provide a stahl wrapper in place of this
        .register_fn("set-env-var!", |name, val| unsafe {
            std::env::set_var::<String, String>(name, val)
        })
        .register_fn("arity?", arity)
        .register_fn("function-name", lookup_function_name)
        .register_fn("#%native-fn-ptr-doc", lookup_doc)
        .register_fn("multi-arity?", is_multi_arity)
        .register_value("make-struct-type", StahlVal::FuncV(make_struct_type))
        .register_value(
            "#%struct-update",
            StahlVal::MutFunc(struct_update_primitive),
        )
        .register_fn("box-strong", StahlVal::boxed)
        .register_native_fn_definition(UNBOX_DEFINITION)
        .register_native_fn_definition(SET_BOX_DEFINITION)
        .register_value("#%box", StahlVal::BuiltIn(make_mutable_box))
        .register_value("#%gc-collect", StahlVal::BuiltIn(gc_collection))
        .register_value("box", StahlVal::BuiltIn(make_mutable_box))
        .register_native_fn_definition(SET_BOX_MUTABLE_DEFINITION)
        .register_native_fn_definition(UNBOX_MUTABLE_DEFINITION)
        .register_native_fn_definition(PLAIN_UNBOX_MUTABLE_DEFINITION)
        .register_native_fn_definition(PLAIN_SET_BOX_MUTABLE_DEFINITION)
        .register_value(
            "attach-contract-struct!",
            StahlVal::FuncV(attach_contract_struct),
        )
        .register_value("get-contract-struct", StahlVal::FuncV(get_contract))
        .register_fn("current-os!", || std::env::consts::OS)
        .register_fn(
            "#%build-dylib",
            |_args: Vec<String>, _env_vars: Vec<(String, String)>| {
                #[cfg(feature = "dylib-build")]
                cargo_stahl_lib::run(_args, _env_vars).ok()
            },
        )
        .register_fn("feature-dylib-build?", || cfg!(feature = "dylib-build"))
        .register_native_fn_definition(COMMAND_LINE_DEFINITION)
        .register_native_fn_definition(ERROR_OBJECT_MESSAGE_DEFINITION)
        .register_fn("stahl-home-location", stahl_home)
        .register_fn("%#interner-memory-usage", interned_current_memory_usage);

    #[cfg(not(feature = "dylibs"))]
    module.register_native_fn_definition(super::engine::LOAD_MODULE_NOOP_DEFINITION);

    // TODO: Remove
    #[cfg(feature = "dylibs")]
    module.register_native_fn_definition(crate::stahl_vm::dylib::LOAD_MODULE_DEFINITION);

    module
}

/// Returns the command line passed to this process,
/// including the command name as first argument.
#[stahl_derive::function(name = "command-line")]
fn command_line() -> StahlList<String> {
    std::env::args().collect()
}

/// De/serialization from/to JSON.
#[stahl_derive::define_module(name = "stahl/json")]
fn json_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/json");
    module
        .register_native_fn_definition(crate::values::json_vals::STRING_TO_JSEXPR_DEFINITION)
        .register_native_fn_definition(
            crate::values::json_vals::SERIALIZE_VAL_TO_STRING_DEFINITION,
        );
    module
}

fn syntax_to_module_impl(ctx: &mut VmCore, args: &[StahlVal]) -> Result<StahlVal> {
    if let StahlVal::SyntaxObject(s) = &args[0] {
        let span = s.syntax_loc();
        let source = span.source_id();

        if let Some(source) = source {
            let path = ctx.thread.sources.get_path(&source);
            return path
                .map(|x| x.to_str().unwrap().to_string())
                .into_stahlval();
        }
    }

    Ok(StahlVal::BoolV(false))
}

#[stahl_derive::context(name = "syntax-originating-file", arity = "Exact(1)")]
fn syntax_to_module(ctx: &mut VmCore, args: &[StahlVal]) -> Option<Result<StahlVal>> {
    Some(syntax_to_module_impl(ctx, args))
}

fn syntax_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/syntax");
    module
        .register_fn("syntax->datum", crate::rvals::Syntax::syntax_datum)
        .register_fn("syntax-loc", crate::rvals::Syntax::syntax_loc)
        .register_fn("syntax/loc", crate::rvals::Syntax::new)
        .register_fn("syntax-span", crate::rvals::Syntax::syntax_loc)
        .register_fn("span-file-id", |span: Span| span.source_id.map(|x| x.0))
        .register_fn("#%syntax/raw", crate::rvals::Syntax::proto)
        .register_fn("syntax-e", crate::rvals::Syntax::syntax_e)
        .register_value("syntax?", gen_pred!(SyntaxObject))
        .register_fn("#%debug-syntax->exprkind", |value| {
            let expr = TryFromStahlValVisitorForExprKind::root(&value);

            match expr {
                Ok(v) => {
                    println!("{}", v.to_pretty(60));
                }
                Err(e) => {
                    println!("{}", e);
                }
            }
        })
        .register_native_fn_definition(SYNTAX_TO_MODULE_DEFINITION);
    module
}

// #[derive(Clone, Copy)]
// pub struct SourceLocation {
//     span: Span,
//     source: Option<usize>,
// }

// impl Custom for SourceLocation {}

// TODO: Add integration for native functions to just write something like:
// pub fn dummy(args: RestArgs) where RestArgs just derefs to &[StahlVal] and the arguments
// can be selected that way

pub fn error_with_src_loc() -> StahlVal {
    StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
        let mut error_message = String::new();

        if args.len() < 2 {
            stop!(ArityMismatch => "error-with-span expects at least 2 arguments - the span and the error message")
        }

        let span = Span::from_stahlval(&args[0])?;

        if !args[1..].is_empty() {
            for arg in &args[1..] {
                let error_val = arg.to_string();
                error_message.push(' ');
                error_message.push_str(error_val.trim_matches('\"'));
            }

            stop!(Generic => error_message; span);
        } else {
            stop!(ArityMismatch => "error-with-span takes at least one argument"; span);
        }
    })
}

pub fn error_from_error_with_span() -> StahlVal {
    StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
        if args.len() != 2 {
            stop!(ArityMismatch => "raise-error-with-span expects at least 2 arguments - the error object and the span")
        }

        let mut stahl_error = StahlErr::from_stahlval(&args[0])?;

        // stahl_error.span()

        if let Some(span) = stahl_error.span() {
            stahl_error.push_span_context_to_stack_trace_if_trace_exists(span);
        }

        let span = Span::from_stahlval(&args[1])?;

        Err(stahl_error.with_span(span))
    })
}

pub fn raise_error_from_error() -> StahlVal {
    StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
        let stahl_error = StahlErr::from_stahlval(&args[0])?;

        Err(stahl_error)
    })
}

// Be able to introspect on the modules - probably just need to add a modules
// field on the vm, or use a wrapped type with modules to find things
// TODO: Add magic number for modules. - key to magic number, do pointer equality
fn _lookup_doc(_ctx: &mut VmCore, _args: &[StahlVal]) -> Result<StahlVal> {
    // for value in ctx.thread.global_env.bindings_vec.iter() {
    //     if let
    // }

    todo!()
}
