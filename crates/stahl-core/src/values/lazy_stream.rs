use crate::rvals::StahlVal;

#[derive(Clone)]
pub struct LazyStream {
    pub initial_value: StahlVal, // argument to stream
    pub stream_thunk: StahlVal,  // function to get the next value
    pub empty_stream: bool,
}

impl LazyStream {
    // Perhaps do some error checking here in order to determine
    // if the arguments passed are actually valid
    pub fn new(initial_value: StahlVal, stream_thunk: StahlVal) -> Self {
        LazyStream {
            initial_value,
            stream_thunk,
            empty_stream: false,
        }
    }

    pub fn new_empty_stream() -> Self {
        LazyStream {
            initial_value: StahlVal::Void,
            stream_thunk: StahlVal::Void,
            empty_stream: true,
        }
    }

    // Should return the value in the `initial_value` field
    // is equivalent to calling (stream-first stream)
    pub fn stream_first(&self) -> StahlVal {
        self.initial_value.clone()
    }

    // `stream_thunk` should be a thunk that return the next `LazyStream`
    //  this should just return a new `LazyStream`
    pub fn stream_thunk(&self) -> StahlVal {
        self.stream_thunk.clone()
    }

    pub fn empty_stream(&self) -> StahlVal {
        StahlVal::BoolV(self.empty_stream)
    }
}
