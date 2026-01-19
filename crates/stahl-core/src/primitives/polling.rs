use polling::{Event, Events, Poller};
use std::{cell::RefCell, net::TcpListener, sync::atomic::AtomicUsize};

use crate::{
    rvals::{AsRefStahlVal, Custom, IntoStahlVal, Result},
    stahl_vm::builtin::BuiltInModule,
    StahlVal,
};

impl Custom for Poller {}
impl Custom for Events {}

static KEY_ID: AtomicUsize = AtomicUsize::new(0);

#[stahl_derive::function(name = "fresh-event-id")]
fn fresh_key() -> Result<StahlVal> {
    KEY_ID
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        .into_stahlval()
}

#[stahl_derive::function(name = "make-poller")]
pub fn new_poller() -> Result<StahlVal> {
    Poller::new()?.into_stahlval()
}

#[stahl_derive::function(name = "add-event-interest-read")]
pub fn add_read(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    unsafe {
        Poller::as_ref(poller)?.add(&*(TcpListener::as_ref(socket)?), Event::readable(key))?;
        Ok(StahlVal::Void)
    }
}

#[stahl_derive::function(name = "add-event-interest-write")]
pub fn add_write(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    unsafe {
        Poller::as_ref(poller)?.add(&*(TcpListener::as_ref(socket)?), Event::writable(key))?;
        Ok(StahlVal::Void)
    }
}

#[stahl_derive::function(name = "add-event-interest-all")]
pub fn add_all(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    unsafe {
        Poller::as_ref(poller)?.add(&*(TcpListener::as_ref(socket)?), Event::all(key))?;
        Ok(StahlVal::Void)
    }
}

// Probably, just need to have a thread local events?
thread_local! {
    pub static EVENTS: RefCell<Events> = RefCell::new(Events::new());
}

#[stahl_derive::function(name = "events-clear!")]
pub fn clear_events() {
    EVENTS.with(|x| x.borrow_mut().clear());
}

#[stahl_derive::function(name = "poller-wait")]
pub fn poller_wait(poller: &StahlVal) -> Result<StahlVal> {
    let poller = Poller::as_ref(poller)?;
    EVENTS.with(|x| poller.wait(&mut x.borrow_mut(), None))?;
    Ok(StahlVal::Void)
}

#[stahl_derive::function(name = "events->list")]
pub fn events() -> Result<StahlVal> {
    Ok(StahlVal::ListV(EVENTS.with(|x| {
        x.borrow()
            .iter()
            .map(|x| StahlVal::IntV(x.key as _))
            .collect::<crate::values::lists::List<_>>()
    })))
}

#[stahl_derive::function(name = "modify-event-interest-read!")]
pub fn modify_read(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    Poller::as_ref(poller)?.modify(&*(TcpListener::as_ref(socket)?), Event::readable(key))?;
    Ok(StahlVal::Void)
}

#[stahl_derive::function(name = "modify-event-interest-write!")]
pub fn modify_write(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    Poller::as_ref(poller)?.modify(&*(TcpListener::as_ref(socket)?), Event::writable(key))?;
    Ok(StahlVal::Void)
}

#[stahl_derive::function(name = "modify-event-interest-all!")]
pub fn modify_all(poller: &StahlVal, socket: &StahlVal, key: usize) -> Result<StahlVal> {
    Poller::as_ref(poller)?.modify(&*(TcpListener::as_ref(socket)?), Event::all(key))?;
    Ok(StahlVal::Void)
}

#[stahl_derive::function(name = "poller-delete!")]
pub fn delete_interest(poller: &StahlVal, socket: &StahlVal) -> Result<StahlVal> {
    Poller::as_ref(poller)?.delete(&*(TcpListener::as_ref(socket)?))?;
    Ok(StahlVal::Void)
}

pub fn polling_module() -> BuiltInModule {
    let mut module = BuiltInModule::new("stahl/polling".to_string());

    module
        .register_native_fn_definition(FRESH_KEY_DEFINITION)
        .register_native_fn_definition(NEW_POLLER_DEFINITION)
        .register_native_fn_definition(ADD_READ_DEFINITION)
        .register_native_fn_definition(ADD_WRITE_DEFINITION)
        .register_native_fn_definition(ADD_ALL_DEFINITION)
        .register_native_fn_definition(CLEAR_EVENTS_DEFINITION)
        .register_native_fn_definition(POLLER_WAIT_DEFINITION)
        .register_native_fn_definition(EVENTS_DEFINITION)
        .register_native_fn_definition(CLEAR_EVENTS_DEFINITION)
        .register_native_fn_definition(MODIFY_READ_DEFINITION)
        .register_native_fn_definition(MODIFY_WRITE_DEFINITION)
        .register_native_fn_definition(MODIFY_ALL_DEFINITION)
        .register_native_fn_definition(DELETE_INTEREST_DEFINITION);

    module
}

// unsafe fn test() -> std::result::Result<(), Box<dyn Error>> {
//     // Create a TCP listener.
//     let socket = TcpListener::bind("127.0.0.1:8000")?;
//     socket.set_nonblocking(true)?;
//     let key = 7; // Arbitrary key identifying the socket.

//     // Create a poller and register interest in readability on the socket.
//     let poller = Poller::new()?;
//     poller.add(&socket, Event::readable(key))?;

//     // The event loop.
//     let mut events = Events::new();
//     loop {
//         // Wait for at least one I/O event.
//         events.clear();
//         poller.wait(&mut events, None)?;

//         for ev in events.iter() {
//             // Map the key to the event properly
//             if ev.key == key {
//                 // Perform a non-blocking accept operation.
//                 socket.accept()?;
//                 // Set interest in the next readability event.
//                 poller.modify(&socket, Event::readable(key))?;
//             }
//         }
//     }
// }
