use crate::gc::Gc;
use crate::rvals::{Result, StahlVal};
use crate::stop;

use crate::values::lazy_stream::LazyStream;

pub struct StreamOperations {}
impl StreamOperations {
    pub fn stream_cons() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.len() != 2 {
                stop!(ArityMismatch => "stream-cons requires 2 argments")
            }

            if let StahlVal::Closure(_) = &args[1] {
                let initial_value = args[0].clone();
                let stream_thunk = args[1].clone();
                Ok(StahlVal::StreamV(Gc::new(LazyStream::new(
                    initial_value,
                    stream_thunk,
                ))))
            } else {
                stop!(TypeMismatch => "stream-cons takes a function in the second position")
            }
        })
    }

    #[inline(always)]
    pub fn empty_stream() -> StahlVal {
        StahlVal::StreamV(Gc::new(LazyStream::new_empty_stream()))
    }

    pub fn stream_empty_huh() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "stream-empty takes 1 argument")
            }
            if let StahlVal::StreamV(s) = &args[0] {
                Ok(s.empty_stream())
            } else {
                stop!(TypeMismatch => "stream-empty? takes a stream")
            }
        })
    }

    pub fn stream_car() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "stream-car takes 1 argument")
            }
            if let StahlVal::StreamV(s) = &args[0] {
                Ok(s.stream_first())
            } else {
                stop!(TypeMismatch => "stream-car takes a stream")
            }
        })
    }

    pub fn stream_cdr() -> StahlVal {
        StahlVal::FuncV(|args: &[StahlVal]| -> Result<StahlVal> {
            if args.len() != 1 {
                stop!(ArityMismatch => "stream-cdr takes 1 argument")
            }
            if let StahlVal::StreamV(s) = &args[0] {
                Ok(s.stream_thunk())
            } else {
                stop!(TypeMismatch => format!("stream-cdr takes a stream, found: {}", &args[0]))
            }
        })
    }
}
