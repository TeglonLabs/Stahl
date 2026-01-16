use crate::{list, rvals::FromStahlVal, rvals::IntoStahlVal};

use crate::rvals::StahlVal;

use super::parser::SourceId;

pub use stahl_parser::span::Span;

impl IntoStahlVal for Span {
    fn into_stahlval(self) -> crate::rvals::Result<crate::StahlVal> {
        Ok(list![self.start, self.end, self.source_id])
    }
}

impl FromStahlVal for Span {
    fn from_stahlval(val: &crate::StahlVal) -> crate::rvals::Result<Self> {
        if let StahlVal::ListV(l) = val {
            if l.len() != 3 {
                stop!(ConversionError => "cannot convert to a span object: {}", val);
            }

            Ok(Span {
                start: usize::from_stahlval(l.get(0).unwrap())?,
                end: usize::from_stahlval(l.get(1).unwrap())?,
                source_id: l
                    .get(2)
                    .map(Option::<u32>::from_stahlval)
                    .map(|x| x.transpose())
                    .flatten()
                    .transpose()?
                    .map(SourceId),
            })
        } else {
            stop!(ConversionError => "cannot convert to a span object: {}", val)
        }
    }
}
