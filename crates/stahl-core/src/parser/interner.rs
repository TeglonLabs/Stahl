pub use stahl_parser::interner::*;

use crate::{rvals::StahlString, StahlVal};

impl From<InternedString> for StahlVal {
    fn from(value: InternedString) -> Self {
        StahlVal::StringV(value.into())
    }
}

impl From<InternedString> for StahlString {
    fn from(value: InternedString) -> Self {
        value.resolve().into()
    }
}
