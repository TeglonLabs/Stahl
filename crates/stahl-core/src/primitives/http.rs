use crate::{
    gc::{Gc, ShareableMut},
    rvals::{AsRefStahlVal, Custom, IntoStahlVal, StahlByteVector, StahlHashMap, StahlString},
    stahl_vm::builtin::BuiltInModule,
    StahlVal,
};

use crate::rvals::Result;

pub struct Header {
    pub name: String,
    pub value: Vec<u8>,
}

pub struct StahlRequest {
    method: StahlString,
    path: StahlString,
    version: StahlString,
    // Offset into the buffer where the body starts
    body_offset: usize,
    // Probably just... push down directly into a hashmap?
    // or keep it some kind of key value pair store?
    headers: Vec<Header>,
}

impl Custom for StahlRequest {}

pub struct StahlResponse {
    pub version: u8,
    /// The response code, such as `200`.
    pub code: u16,
    /// The response reason-phrase, such as `OK`.
    ///
    /// Contains an empty string if the reason-phrase was missing or contained invalid characters.
    pub reason: String,
    /// The response headers.
    pub headers: Vec<Header>,

    pub body_offset: usize,
}

impl Custom for StahlResponse {}

#[stahl_derive::function(name = "http-request-method")]
pub fn method(value: &StahlVal) -> Result<StahlVal> {
    StahlRequest::as_ref(value)
        .map(|x| x.method.clone())
        .map(StahlVal::StringV)
}

#[stahl_derive::function(name = "http-request-path")]
pub fn path(value: &StahlVal) -> Result<StahlVal> {
    StahlRequest::as_ref(value)
        .map(|x| x.path.clone())
        .map(StahlVal::StringV)
}

#[stahl_derive::function(name = "http-request-version")]
pub fn version(value: &StahlVal) -> Result<StahlVal> {
    StahlRequest::as_ref(value)
        .map(|x| x.version.clone())
        .map(StahlVal::StringV)
}

#[stahl_derive::function(name = "http-request-body-offset")]
pub fn body_offset(value: &StahlVal) -> Result<StahlVal> {
    StahlRequest::as_ref(value)
        .map(|x| x.body_offset as isize)
        .map(StahlVal::IntV)
}

#[stahl_derive::function(name = "http-request-headers")]
pub fn headers(value: &StahlVal) -> Result<StahlVal> {
    let req = StahlRequest::as_ref(value)?;

    Ok(StahlVal::HashMapV(StahlHashMap(Gc::new(
        req.headers
            .iter()
            .map(|x| {
                (
                    StahlVal::StringV(x.name.clone().into()),
                    StahlVal::ByteVector(StahlByteVector::new(x.value.clone())),
                )
            })
            .collect::<crate::values::HashMap<_, _>>(),
    ))))
}

#[stahl_derive::function(name = "http-response-headers")]
pub fn resp_headers(value: &StahlVal) -> Result<StahlVal> {
    let resp = StahlResponse::as_ref(value)?;

    Ok(StahlVal::HashMapV(StahlHashMap(Gc::new(
        resp.headers
            .iter()
            .map(|x| {
                (
                    StahlVal::StringV(x.name.clone().into()),
                    StahlVal::ByteVector(StahlByteVector::new(x.value.clone())),
                )
            })
            .collect::<crate::values::HashMap<_, _>>(),
    ))))
}

// If not complete, try again?
fn parse_request(buf: &[u8]) -> Result<StahlVal> {
    // Pull more bytes from the stream?
    let mut headers = [httparse::EMPTY_HEADER; 16];
    let mut req = httparse::Request::new(&mut headers);
    let res = req.parse(&buf).unwrap();
    if res.is_complete() {
        let request = StahlRequest {
            method: req.method.unwrap().to_string().into(),
            path: req.path.unwrap().to_string().into(),
            version: req.version.unwrap().to_string().into(),
            body_offset: res.unwrap(),
            headers: headers
                .iter()
                .filter_map(|x| {
                    if *x != httparse::EMPTY_HEADER {
                        Some(Header {
                            name: x.name.to_string(),
                            value: x.value.to_vec(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
        };

        request.into_stahlval()
    } else {
        Ok(StahlVal::BoolV(false))
    }
}

fn parse_response(buf: &[u8]) -> Result<StahlVal> {
    // Pull more bytes from the stream?
    let mut headers = [httparse::EMPTY_HEADER; 64];
    let mut req = httparse::Response::new(&mut headers);
    let res = req.parse(&buf).unwrap();
    if res.is_complete() {
        let request = StahlResponse {
            version: req.version.unwrap(),
            code: req.code.unwrap(),
            reason: req.reason.unwrap().to_string(),
            body_offset: res.unwrap(),
            headers: headers
                .iter()
                .filter_map(|x| {
                    if *x != httparse::EMPTY_HEADER {
                        Some(Header {
                            name: x.name.to_string(),
                            value: x.value.to_vec(),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
        };

        request.into_stahlval()
    } else {
        Ok(StahlVal::BoolV(false))
    }
}

#[stahl_derive::function(name = "http-parse-request")]
pub fn parse_http_request(vector: &StahlByteVector) -> Result<StahlVal> {
    parse_request(&vector.vec.read())
}

#[stahl_derive::function(name = "http-parse-response")]
pub fn parse_http_response(vector: &StahlByteVector) -> Result<StahlVal> {
    parse_response(&vector.vec.read())
}

pub fn http_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/http".to_string());

    module
        .register_native_fn_definition(PARSE_HTTP_REQUEST_DEFINITION)
        .register_native_fn_definition(METHOD_DEFINITION)
        .register_native_fn_definition(VERSION_DEFINITION)
        .register_native_fn_definition(PATH_DEFINITION)
        .register_native_fn_definition(BODY_OFFSET_DEFINITION)
        .register_native_fn_definition(HEADERS_DEFINITION)
        .register_native_fn_definition(RESP_HEADERS_DEFINITION)
        .register_native_fn_definition(PARSE_HTTP_RESPONSE_DEFINITION);

    // module
    //     .register_native_fn_definition(TCP_CONNECT_DEFINITION)
    //     .register_native_fn_definition(TCP_INPUT_PORT_DEFINITION)
    //     .register_native_fn_definition(TCP_OUTPUT_PORT_DEFINITION)
    //     .register_native_fn_definition(TCP_BUFFERED_OUTPUT_PORT_DEFINITION)
    //     .register_native_fn_definition(TCP_LISTEN_DEFINITION)
    //     .register_native_fn_definition(TCP_ACCEPT_DEFINITION);

    module
}
