use crate::rvals::{PrimitiveAsRef, RestArgsIter, Result, StahlString, StahlVal};
use crate::stahl_vm::builtin::BuiltInModule;
use crate::stop;

pub fn symbol_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/symbols");
    module
        .register_value("concat-symbols", SymbolOperations::concat_symbols())
        .register_value("symbol->string", SymbolOperations::symbol_to_string())
        .register_native_fn_definition(SYMBOL_EQUALS_DEFINITION);
    module
}

pub struct StahlSymbol<'a>(&'a StahlString);

impl<'a> PrimitiveAsRef<'a> for StahlSymbol<'a> {
    fn primitive_as_ref(val: &'a StahlVal) -> Result<Self> {
        if let StahlVal::SymbolV(sym) = val {
            Ok(StahlSymbol(sym))
        } else {
            stop!(ConversionError => format!("Cannot convert stahl value: {} to stahl symbol", val))
        }
    }

    fn maybe_primitive_as_ref(val: &'a StahlVal) -> Option<Self> {
        if let StahlVal::SymbolV(sym) = val {
            Some(StahlSymbol(sym))
        } else {
            None
        }
    }
}

#[stahl_derive::function(name = "symbol=?", constant = true)]
pub fn symbol_equals(mut iter: RestArgsIter<StahlSymbol<'_>>) -> Result<StahlVal> {
    let Some(mut prev) = iter.next().transpose()? else {
        stop!(ArityMismatch => "expected at least one argument");
    };

    for item in iter {
        let item = item?;
        if crate::gc::Shared::ptr_eq(&**item.0, &**prev.0) {
            prev = item;
        } else {
            return Ok(StahlVal::BoolV(false));
        }
    }

    Ok(StahlVal::BoolV(true))
}

pub struct SymbolOperations {}
impl SymbolOperations {
    pub fn concat_symbols() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            let mut new_symbol = String::new();

            for arg in args {
                if let StahlVal::SymbolV(quoted_value) = arg {
                    new_symbol.push_str(quoted_value.as_ref());
                } else {
                    let error_message =
                        format!("concat-symbol expected only symbols, found {args:?}");
                    stop!(TypeMismatch => error_message);
                }
            }

            Ok(StahlVal::SymbolV(new_symbol.into()))
        })
    }

    pub fn symbol_to_string() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.len() == 1 {
                match &args[0] {
                    StahlVal::SymbolV(quoted_value) => Ok(StahlVal::StringV(quoted_value.clone())),
                    StahlVal::ListV(_) => Ok(StahlVal::StringV(
                        format!("{:?}", &args[0]).trim_start_matches('\'').into(),
                    )),
                    _ => {
                        let error_message =
                            format!("symbol->string expected a symbol, found {}", &args[0]);
                        stop!(TypeMismatch => error_message)
                    }
                }
            } else {
                stop!(ArityMismatch => "symbol->string expects only one argument")
            }
        })
    }
}

#[cfg(test)]
mod symbol_tests {
    use super::*;
    use crate::throw;

    use crate::rvals::StahlVal::*;

    fn apply_function(func: StahlVal, args: Vec<StahlVal>) -> Result<StahlVal> {
        func.func_or_else(throw!(BadSyntax => "hash tests"))
            .unwrap()(&args)
    }

    #[test]
    fn concat_symbols_normal() {
        let args = vec![
            SymbolV("foo".into()),
            SymbolV("bar".into()),
            SymbolV("baz".into()),
        ];
        let result = apply_function(SymbolOperations::concat_symbols(), args);
        let expected = SymbolV("foobarbaz".into());
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn symbol_to_string_normal() {
        let args = vec![SymbolV("foo".into())];
        let result = apply_function(SymbolOperations::symbol_to_string(), args);
        let expected = StringV("foo".into());
        assert_eq!(result.unwrap(), expected);
    }
}
