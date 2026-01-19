use std::io::BufReader;
use std::net::{TcpListener, TcpStream};

use stahl_derive::function;

use crate::gc::Gc;
use crate::rvals::{AsRefStahlVal, Custom, IntoStahlVal, Result, StahlString, StahlVal};
use crate::stahl_vm::builtin::BuiltInModule;
use crate::values::lists::Pair;
use crate::values::port::StahlPort;
use crate::values::StahlPortRepr;

impl Custom for TcpStream {}
impl Custom for TcpListener {}

// TODO: Maybe include the following as builtins:
// - https://crates.io/crates/httparse
// - https://github.com/hyperium/http
// - Perhaps include hyper as well

#[function(name = "tcp-connect")]
pub fn tcp_connect(addr: StahlString) -> Result<StahlVal> {
    TcpStream::connect(addr.as_str())?.into_stahlval()
}

#[function(name = "tcp-shutdown!")]
pub fn tcp_close(stream: &StahlVal) -> Result<StahlVal> {
    let writer = TcpStream::as_ref(stream)?.try_clone().unwrap();
    writer.shutdown(std::net::Shutdown::Both)?;
    Ok(StahlVal::Void)
}

#[function(name = "tcp-stream-writer")]
pub fn tcp_input_port(stream: &StahlVal) -> Result<StahlVal> {
    let writer = TcpStream::as_ref(stream)?.try_clone().unwrap();
    Ok(StahlVal::new_dyn_writer_port(writer))
}

#[function(name = "tcp-stream-reader")]
pub fn tcp_output_port(stream: &StahlVal) -> Result<StahlVal> {
    let reader = TcpStream::as_ref(stream)?.try_clone().unwrap();
    Ok(StahlVal::PortV(StahlPort {
        port: Gc::new_mut(StahlPortRepr::TcpStream(reader)),
    }))
}

#[function(name = "tcp-stream-buffered-reader")]
pub fn tcp_buffered_output_port(stream: &StahlVal) -> Result<StahlVal> {
    let reader = TcpStream::as_ref(stream)?.try_clone().unwrap();
    Ok(StahlVal::PortV(StahlPort {
        port: Gc::new_mut(StahlPortRepr::DynReader(BufReader::new(Box::new(reader)))),
    }))
}

#[function(name = "tcp-listen")]
pub fn tcp_listen(addr: StahlString) -> Result<StahlVal> {
    TcpListener::bind(addr.as_str())?.into_stahlval()
}

#[function(name = "tcp-accept")]
pub fn tcp_accept(value: &StahlVal) -> Result<StahlVal> {
    TcpListener::as_ref(value)?.accept()?.0.into_stahlval()
}

#[function(name = "tcp-accept-with-addr")]
pub fn tcp_accept_addr(value: &StahlVal) -> Result<StahlVal> {
    let res = TcpListener::as_ref(value)?.accept()?;

    Ok(StahlVal::Pair(Gc::new(Pair::cons(
        res.0.into_stahlval()?,
        res.1.to_string().into_stahlval()?,
    ))))
}

#[function(name = "tcp-listener-set-non-blocking!")]
pub fn tcp_listener_set_non_blocking(value: &StahlVal) -> Result<StahlVal> {
    TcpListener::as_ref(value)?.set_nonblocking(true)?;
    Ok(StahlVal::Void)
}

#[function(name = "tcp-stream-set-non-blocking!")]
pub fn tcp_stream_set_non_blocking(value: &StahlVal) -> Result<StahlVal> {
    TcpStream::as_ref(value)?.set_nonblocking(true)?;
    Ok(StahlVal::Void)
}

pub fn tcp_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/tcp".to_string());

    module
        .register_native_fn_definition(TCP_CONNECT_DEFINITION)
        .register_native_fn_definition(TCP_CLOSE_DEFINITION)
        .register_native_fn_definition(TCP_INPUT_PORT_DEFINITION)
        .register_native_fn_definition(TCP_OUTPUT_PORT_DEFINITION)
        .register_native_fn_definition(TCP_BUFFERED_OUTPUT_PORT_DEFINITION)
        .register_native_fn_definition(TCP_LISTEN_DEFINITION)
        .register_native_fn_definition(TCP_ACCEPT_DEFINITION)
        .register_native_fn_definition(TCP_ACCEPT_ADDR_DEFINITION)
        .register_native_fn_definition(TCP_LISTENER_SET_NON_BLOCKING_DEFINITION)
        .register_native_fn_definition(TCP_STREAM_SET_NON_BLOCKING_DEFINITION);

    module
}
