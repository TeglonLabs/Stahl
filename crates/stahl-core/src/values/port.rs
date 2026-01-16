use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::prelude::*;
use std::io::Cursor;
use std::io::Stderr;
use std::io::{BufReader, BufWriter, Stdin, Stdout};
use std::net::TcpStream;
use std::process::ChildStderr;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::sync::Arc;
use std::sync::Mutex;

use crate::gc::shared::ShareableMut;
use crate::gc::Gc;
use crate::gc::GcMut;
use crate::rvals::Result;
use crate::StahlVal;

// use crate::rvals::{new_rc_ref_cell, RcRefStahlVal};

thread_local! {
    // TODO: This needs to be per engine, not global, and functions should accept the port they use
    // Probably by boxing up the port that gets used
    pub static DEFAULT_OUTPUT_PORT: GcMut<StahlPort> = Gc::new_mut(StahlPort { port: Gc::new_mut(StahlPortRepr::StdOutput(io::stdout())) } );
    pub static CAPTURED_OUTPUT_PORT: GcMut<BufWriter<Vec<u8>>> = Gc::new_mut(BufWriter::new(Vec::new()));
}

#[derive(Debug, Clone)]
pub struct StahlPort {
    pub(crate) port: GcMut<StahlPortRepr>,
}

// pub trait PortLike {
//     fn as_any_ref(&self) -> &dyn Any;
//     fn into_port(self) -> StahlVal;
// }

// impl<T: Write + Send + Sync + 'static> PortLike for T {
//     fn as_any_ref(&self) -> &dyn Any {
//         self as &dyn Any
//     }

//     //
//     fn into_port(self) -> StahlVal {}
// }

// #[derive(Debug)]
pub enum StahlPortRepr {
    FileInput(String, BufReader<File>),
    FileOutput(String, BufWriter<File>),
    StdInput(Stdin),
    StdOutput(Stdout),
    StdError(Stderr),
    ChildStdOutput(BufReader<ChildStdout>),
    ChildStdError(BufReader<ChildStderr>),
    ChildStdInput(BufWriter<ChildStdin>),
    StringInput(Cursor<Vec<u8>>),
    StringOutput(Vec<u8>),

    // TODO: This does not need to be Arc<Mutex<dyn ...>> - it can
    // get away with just Box<dyn ...> - and also it should be dyn Portlike
    // with blanket trait impls to do the thing otherwise.
    DynWriter(Arc<Mutex<dyn Write + Send + Sync>>),
    DynReader(BufReader<Box<dyn Read + Send + Sync>>),
    TcpStream(TcpStream),
    // DynReader(Box<dyn Read>),
    Closed,
}

impl std::fmt::Debug for StahlPortRepr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StahlPortRepr::FileInput(name, w) => {
                f.debug_tuple("FileInput").field(name).field(w).finish()
            }
            StahlPortRepr::FileOutput(name, w) => {
                f.debug_tuple("FileOutput").field(name).field(w).finish()
            }
            StahlPortRepr::StdInput(s) => f.debug_tuple("StdInput").field(s).finish(),
            StahlPortRepr::StdOutput(s) => f.debug_tuple("StdOutput").field(s).finish(),
            StahlPortRepr::StdError(s) => f.debug_tuple("StdError").field(s).finish(),
            StahlPortRepr::ChildStdOutput(s) => f.debug_tuple("ChildStdOutput").field(s).finish(),
            StahlPortRepr::ChildStdError(s) => f.debug_tuple("ChildStdError").field(s).finish(),
            StahlPortRepr::ChildStdInput(s) => f.debug_tuple("ChildStdInput").field(s).finish(),
            StahlPortRepr::StringInput(s) => f.debug_tuple("StringInput").field(s).finish(),
            StahlPortRepr::StringOutput(s) => f.debug_tuple("StringOutput").field(s).finish(),
            StahlPortRepr::DynWriter(_) => f.debug_tuple("DynWriter").field(&"#<opaque>").finish(),
            StahlPortRepr::DynReader(_) => f
                .debug_tuple("DynReader")
                .field(&"#<opaque-reader>")
                .finish(),
            StahlPortRepr::TcpStream(_) => f.debug_tuple("TcpStream").finish(),
            StahlPortRepr::Closed => f.debug_tuple("Closed").finish(),
        }
    }
}

pub enum SendablePort {
    StdInput(Stdin),
    StdOutput(Stdout),
    StdError(Stderr),
    BoxDynWriter(Arc<Mutex<dyn Write + Send + Sync>>),
    Closed,
}

impl SendablePort {
    fn from_port_repr(value: &StahlPortRepr) -> Result<SendablePort> {
        match value {
            StahlPortRepr::StdInput(_) => Ok(SendablePort::StdInput(io::stdin())),
            StahlPortRepr::StdOutput(_) => Ok(SendablePort::StdOutput(io::stdout())),
            StahlPortRepr::StdError(_) => Ok(SendablePort::StdError(io::stderr())),
            StahlPortRepr::Closed => Ok(SendablePort::Closed),
            _ => {
                stop!(Generic => "Unable to send port across threads: {:?}", value)
            }
        }
    }

    pub fn from_port(value: StahlPort) -> Result<SendablePort> {
        Self::from_port_repr(&value.port.read())
    }
}

impl StahlPort {
    pub fn from_sendable_port(value: SendablePort) -> Self {
        match value {
            SendablePort::StdInput(s) => StahlPort {
                port: Gc::new_mut(StahlPortRepr::StdInput(s)),
            },
            SendablePort::StdOutput(s) => StahlPort {
                port: Gc::new_mut(StahlPortRepr::StdOutput(s)),
            },
            SendablePort::StdError(s) => StahlPort {
                port: Gc::new_mut(StahlPortRepr::StdError(s)),
            },
            SendablePort::Closed => StahlPort {
                port: Gc::new_mut(StahlPortRepr::Closed),
            },
            SendablePort::BoxDynWriter(w) => StahlPort {
                port: Gc::new_mut(StahlPortRepr::DynWriter(w)),
            },
        }
    }
}

#[macro_export]
macro_rules! port_read_str_fn(
    ($br: ident, $fn: ident) => {{
        let mut result = String::new();
        let size = $br.$fn(&mut result)?;
        Ok((size, result))
    }};
);

impl StahlPortRepr {
    pub fn read_line(&mut self) -> Result<(usize, String)> {
        match self {
            StahlPortRepr::FileInput(_, br) => port_read_str_fn!(br, read_line),
            StahlPortRepr::StdInput(br) => port_read_str_fn!(br, read_line),
            StahlPortRepr::StringInput(s) => port_read_str_fn!(s, read_line),

            StahlPortRepr::ChildStdOutput(br) => {
                port_read_str_fn!(br, read_line)
            }

            StahlPortRepr::DynReader(br) => port_read_str_fn!(br, read_line),

            // StahlPort::ChildStdOutput(br) => port_read_str_fn!(br, read_line),
            // FIXME: fix this and the functions below
            _x => stop!(Generic => "read-line"),
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        match self {
            StahlPortRepr::FileOutput(_, s) => Ok(s.flush()?),
            StahlPortRepr::StdOutput(s) => Ok(s.flush()?),
            StahlPortRepr::ChildStdInput(s) => Ok(s.flush()?),
            StahlPortRepr::StringOutput(s) => Ok(s.flush()?),
            StahlPortRepr::DynWriter(s) => Ok(s.lock().unwrap().flush()?),
            StahlPortRepr::Closed => Ok(()),
            _ => stop!(TypeMismatch => "expected an output port, found: {:?}", self),
        }
    }

    pub fn read_all_str(&mut self) -> Result<(usize, String)> {
        match self {
            StahlPortRepr::FileInput(_, br) => port_read_str_fn!(br, read_to_string),
            StahlPortRepr::StdInput(br) => port_read_str_fn!(br, read_to_string),
            StahlPortRepr::ChildStdOutput(br) => port_read_str_fn!(br, read_to_string),
            StahlPortRepr::ChildStdError(br) => port_read_str_fn!(br, read_to_string),
            StahlPortRepr::DynReader(br) => port_read_str_fn!(br, read_to_string),
            _x => stop!(Generic => "read-all-str"),
        }
    }

    pub fn read_char(&mut self) -> Result<MaybeBlocking<Option<char>>> {
        let mut buf = [0; 4];

        for i in 0..4 {
            let result = self.read_byte()?;

            let b = match result {
                MaybeBlocking::Nonblocking(Some(b)) => b,
                MaybeBlocking::Nonblocking(None) => {
                    if i == 0 {
                        return Ok(MaybeBlocking::Nonblocking(None));
                    } else {
                        stop!(ConversionError => "unable to decode character, found {:?}", &buf[0..=i]);
                    }
                }
                MaybeBlocking::WouldBlock => return Ok(MaybeBlocking::WouldBlock),
            };

            buf[i] = b;

            match std::str::from_utf8(&buf[0..=i]) {
                Ok(s) => return Ok(MaybeBlocking::Nonblocking(s.chars().next())),
                Err(err) if err.error_len().is_some() => {
                    stop!(ConversionError => "unable to decode character, found {:?}", &buf[0..=i]);
                }
                _ => {}
            }
        }

        stop!(ConversionError => "unable to decode character, found {:?}", buf);
    }

    pub fn read_bytes(&mut self, buf: &mut [u8]) -> Result<MaybeBlocking<bool>> {
        let result = match self {
            StahlPortRepr::FileInput(_, reader) => reader.read_exact(buf),
            StahlPortRepr::StdInput(stdin) => stdin.read_exact(buf),
            StahlPortRepr::ChildStdOutput(output) => output.read_exact(buf),
            StahlPortRepr::ChildStdError(output) => output.read_exact(buf),
            StahlPortRepr::StringInput(reader) => reader.read_exact(buf),
            StahlPortRepr::DynReader(reader) => reader.read_exact(buf),
            StahlPortRepr::TcpStream(t) => t.read_exact(buf),
            StahlPortRepr::FileOutput(_, _)
            | StahlPortRepr::StdOutput(_)
            | StahlPortRepr::StdError(_)
            | StahlPortRepr::ChildStdInput(_)
            | StahlPortRepr::StringOutput(_)
            | StahlPortRepr::DynWriter(_) => stop!(ContractViolation => "expected input-port?"),
            StahlPortRepr::Closed => return Ok(MaybeBlocking::Nonblocking(true)),
        };

        if let Err(err) = result {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                return Ok(MaybeBlocking::Nonblocking(false));
            }

            if err.kind() == io::ErrorKind::WouldBlock {
                // TODO: If this would block, do something
                return Ok(MaybeBlocking::WouldBlock);
            }

            return Err(err.into());
        }

        Ok(MaybeBlocking::Nonblocking(true))
    }

    pub fn read_byte(&mut self) -> Result<MaybeBlocking<Option<u8>>> {
        let mut byte = [0];

        let result = match self {
            StahlPortRepr::FileInput(_, reader) => reader.read_exact(&mut byte),
            StahlPortRepr::StdInput(stdin) => stdin.read_exact(&mut byte),
            StahlPortRepr::ChildStdOutput(output) => output.read_exact(&mut byte),
            StahlPortRepr::ChildStdError(output) => output.read_exact(&mut byte),
            StahlPortRepr::StringInput(reader) => reader.read_exact(&mut byte),
            StahlPortRepr::DynReader(reader) => reader.read_exact(&mut byte),
            StahlPortRepr::TcpStream(t) => {
                let amount = t.read(&mut byte)?;

                if amount == 0 {
                    stop!(Generic => "unexpected eof");
                } else {
                    Ok(())
                }
            }
            StahlPortRepr::FileOutput(_, _)
            | StahlPortRepr::StdOutput(_)
            | StahlPortRepr::StdError(_)
            | StahlPortRepr::ChildStdInput(_)
            | StahlPortRepr::StringOutput(_)
            | StahlPortRepr::DynWriter(_) => stop!(ContractViolation => "expected input-port?"),
            StahlPortRepr::Closed => return Ok(MaybeBlocking::Nonblocking(None)),
        };

        if let Err(err) = result {
            if err.kind() == io::ErrorKind::UnexpectedEof {
                return Ok(MaybeBlocking::Nonblocking(None));
            }

            return Err(err.into());
        }

        Ok(MaybeBlocking::Nonblocking(Some(byte[0])))
    }

    pub fn peek_byte(&mut self) -> Result<Option<u8>> {
        let mut buf = [0];

        let result = self.peek(&mut buf)?;

        if result == 0 {
            Ok(None)
        } else {
            Ok(Some(buf[0]))
        }
    }

    // This would not work for `peek_char`, since a `BufRead` will not work for this purpose.
    // We need a way to force-fill the internal buffer up to 4 bytes. `fill_buf()` only tries to
    // fill _if_ the internal buffer is empty, and without any guarantees of returned size.
    fn peek(&mut self, buf: &mut [u8]) -> Result<usize> {
        let copy = |src: &[u8]| {
            let len = src.len().min(buf.len());

            buf.copy_from_slice(&src[0..len]);

            len
        };

        let result = match self {
            StahlPortRepr::FileInput(_, reader) => reader.fill_buf().map(copy),
            StahlPortRepr::StdInput(stdin) => {
                let mut lock = stdin.lock();
                lock.fill_buf().map(copy)
            }
            StahlPortRepr::ChildStdOutput(output) => output.fill_buf().map(copy),
            StahlPortRepr::ChildStdError(output) => output.fill_buf().map(copy),
            StahlPortRepr::StringInput(reader) => reader.fill_buf().map(copy),
            StahlPortRepr::DynReader(reader) => reader.fill_buf().map(copy),
            StahlPortRepr::TcpStream(tcp) => tcp.peek(buf),
            StahlPortRepr::FileOutput(_, _)
            | StahlPortRepr::StdOutput(_)
            | StahlPortRepr::StdError(_)
            | StahlPortRepr::ChildStdInput(_)
            | StahlPortRepr::StringOutput(_)
            | StahlPortRepr::DynWriter(_) => stop!(ContractViolation => "expected input-port?"),
            StahlPortRepr::Closed => return Ok(0),
        }?;

        Ok(result)
    }

    pub fn write_char(&mut self, c: char) -> Result<()> {
        let mut buf = [0; 4];

        let s = c.encode_utf8(&mut buf);

        let _ = self.write(s.as_bytes())?;

        Ok(())
    }

    pub fn write_string_line(&mut self, string: &str) -> Result<()> {
        let _ = self.write(string.as_bytes())?;
        let _ = self.write(b"\n")?;

        Ok(())
    }

    pub fn is_input(&self) -> bool {
        matches!(
            self,
            StahlPortRepr::FileInput(_, _)
                | StahlPortRepr::StdInput(_)
                | StahlPortRepr::ChildStdOutput(_)
                | StahlPortRepr::ChildStdError(_)
                | StahlPortRepr::StringInput(_)
        )
    }

    pub fn is_output(&self) -> bool {
        matches!(
            self,
            StahlPortRepr::FileOutput(_, _)
                | StahlPortRepr::StdOutput(_)
                | StahlPortRepr::StdError(_)
                | StahlPortRepr::DynWriter(_)
                | StahlPortRepr::ChildStdInput(_)
                | StahlPortRepr::StringOutput(_)
        )
    }

    pub fn get_output(&self) -> Result<Option<Vec<u8>>> {
        let buf: &Vec<u8> = if let StahlPortRepr::StringOutput(s) = self {
            s
        } else {
            return Ok(None);
        };

        Ok(Some(buf.clone()))
    }

    pub fn close_output_port(&mut self) -> Result<()> {
        if self.is_output() {
            *self = StahlPortRepr::Closed;
            Ok(())
        } else {
            stop!(TypeMismatch => "close-output-port expects an output port, found: {:?}", self)
        }
    }

    pub fn close_input_port(&mut self) -> Result<()> {
        if self.is_input() {
            *self = StahlPortRepr::Closed;
            Ok(())
        } else {
            stop!(TypeMismatch => "close-input-port expects an input port, found: {:?}", self)
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        macro_rules! write_and_flush(
            ($br: expr) => {{
                let result = $br.write(buf)?;
                $br.flush()?;
                result
            }};
        );

        let result = match self {
            StahlPortRepr::FileOutput(_, writer) => write_and_flush![writer],
            StahlPortRepr::StdOutput(writer) => write_and_flush![writer],
            StahlPortRepr::StdError(writer) => write_and_flush![writer],
            StahlPortRepr::ChildStdInput(writer) => write_and_flush![writer],
            StahlPortRepr::StringOutput(writer) => write_and_flush![writer],
            StahlPortRepr::DynWriter(writer) => write_and_flush![writer.lock().unwrap()],
            // TODO: Should tcp streams be both input and output ports?
            StahlPortRepr::TcpStream(tcp) => tcp.write(buf)?,
            StahlPortRepr::FileInput(_, _)
            | StahlPortRepr::StdInput(_)
            | StahlPortRepr::DynReader(_)
            | StahlPortRepr::ChildStdOutput(_)
            | StahlPortRepr::ChildStdError(_)
            | StahlPortRepr::StringInput(_) => stop!(ContractViolation => "expected output-port?"),
            StahlPortRepr::Closed => stop!(Io => "port is closed"),
        };

        Ok(result)
    }
}

pub enum MaybeBlocking<T> {
    Nonblocking(T),
    WouldBlock,
}

impl StahlPort {
    pub fn new_textual_file_input(path: &str) -> Result<StahlPort> {
        let file = OpenOptions::new().read(true).open(path)?;

        Ok(StahlPort {
            port: Gc::new_mut(StahlPortRepr::FileInput(
                path.to_string(),
                BufReader::new(file),
            )),
        })
    }

    pub fn new_textual_file_output(path: &str) -> Result<StahlPort> {
        let file = OpenOptions::new()
            .truncate(true)
            .write(true)
            .create(true)
            .open(path)?;

        Ok(StahlPort {
            port: Gc::new_mut(StahlPortRepr::FileOutput(
                path.to_string(),
                BufWriter::new(file),
            )),
        })
    }

    pub fn new_input_port_string(string: String) -> StahlPort {
        StahlPort {
            port: Gc::new_mut(StahlPortRepr::StringInput(Cursor::new(string.into_bytes()))),
        }
    }

    pub fn new_input_port_bytevector(vec: Vec<u8>) -> StahlPort {
        StahlPort {
            port: Gc::new_mut(StahlPortRepr::StringInput(Cursor::new(vec))),
        }
    }

    pub fn new_output_port_string() -> StahlPort {
        StahlPort {
            port: Gc::new_mut(StahlPortRepr::StringOutput(Vec::new())),
        }
    }

    //
    // Read functions
    //
    pub fn read_line(&self) -> Result<(usize, String)> {
        self.port.write().read_line()
    }

    // TODO: Implement the rest of the flush methods
    pub fn flush(&self) -> Result<()> {
        self.port.write().flush()
    }

    pub fn read_all_str(&self) -> Result<(usize, String)> {
        self.port.write().read_all_str()
    }

    pub fn read_char(&self) -> Result<MaybeBlocking<Option<char>>> {
        self.port.write().read_char()
    }

    pub fn read_byte(&self) -> Result<MaybeBlocking<Option<u8>>> {
        self.port.write().read_byte()
    }

    pub fn read_bytes(&self, amount: usize) -> Result<MaybeBlocking<Vec<u8>>> {
        // TODO: This is going to allocate unnecessarily
        let mut buf = vec![0; amount];
        match self.port.write().read_bytes(&mut buf)? {
            MaybeBlocking::Nonblocking(_) => Ok(MaybeBlocking::Nonblocking(buf)),
            MaybeBlocking::WouldBlock => Ok(MaybeBlocking::WouldBlock),
        }
    }

    pub fn read_bytes_into_buf(&self, buf: &mut [u8]) -> Result<MaybeBlocking<bool>> {
        self.port.write().read_bytes(buf)
    }

    pub fn peek_byte(&self) -> Result<Option<u8>> {
        self.port.write().peek_byte()
    }

    //
    // Write functions
    //
    pub fn write_char(&self, c: char) -> Result<()> {
        self.port.write().write_char(c)
    }

    pub fn write(&self, buf: &[u8]) -> Result<()> {
        let _ = self.port.write().write(buf)?;

        Ok(())
    }

    pub fn write_string_line(&self, string: &str) -> Result<()> {
        self.port.write().write_string_line(string)
    }

    //
    // Checks
    //
    pub fn is_input(&self) -> bool {
        self.port.read().is_input()
    }

    pub fn is_output(&self) -> bool {
        self.port.read().is_output()
    }

    pub fn default_current_input_port() -> Self {
        StahlPort {
            port: Gc::new_mut(StahlPortRepr::StdInput(io::stdin())),
        }
    }

    pub fn default_current_output_port() -> Self {
        if cfg!(test) {
            // Write out to thread safe port
            StahlPort {
                port: Gc::new_mut(StahlPortRepr::DynWriter(Arc::new(Mutex::new(
                    BufWriter::new(Vec::new()),
                )))),
            }
        } else {
            StahlPort {
                port: Gc::new_mut(StahlPortRepr::StdOutput(io::stdout())),
            }
        }
    }

    pub fn default_current_error_port() -> Self {
        if cfg!(test) {
            // Write out to thread safe port
            StahlPort {
                port: Gc::new_mut(StahlPortRepr::DynWriter(Arc::new(Mutex::new(
                    BufWriter::new(Vec::new()),
                )))),
            }
        } else {
            StahlPort {
                port: Gc::new_mut(StahlPortRepr::StdError(io::stderr())),
            }
        }
    }

    pub fn get_output(&self) -> Result<Option<Vec<u8>>> {
        self.port.write().get_output()
    }

    pub fn close_output_port(&self) -> Result<()> {
        self.port.write().close_output_port()
    }

    pub fn close_input_port(&self) -> Result<()> {
        self.port.write().close_input_port()
    }
}

#[cfg(not(feature = "sync"))]
thread_local! {
    pub static WOULD_BLOCK_OBJECT: once_cell::unsync::Lazy<(crate::StahlVal,
        super::structs::StructTypeDescriptor)>= once_cell::unsync::Lazy::new(|| {
        super::structs::make_struct_singleton("would-block".into())
    });
}

#[cfg(feature = "sync")]
pub static WOULD_BLOCK_OBJECT: once_cell::sync::Lazy<(
    crate::StahlVal,
    super::structs::StructTypeDescriptor,
)> = once_cell::sync::Lazy::new(|| super::structs::make_struct_singleton("eof".into()));

pub fn would_block() -> StahlVal {
    #[cfg(feature = "sync")]
    {
        WOULD_BLOCK_OBJECT.0.clone()
    }

    #[cfg(not(feature = "sync"))]
    {
        WOULD_BLOCK_OBJECT.with(|eof| eof.0.clone())
    }
}
