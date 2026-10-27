//! For a game's own page in the iPhone remote (`Remote::Web`), compiled to
//! WASM: what the app adds to a page, in Rust.
//!
//! The app gives a page one door, WebKit's message handler: this crate posts
//! the messages `CONTRAT.md` documents through it, and listens to the events
//! the app sends back (motion, orientation, datagrams). In a plain browser
//! [`info`] is `None` and every call does nothing, so a page keeps working
//! without the app.
//!
//! ```ignore
//! use remote_web::{Vibration, vibrate, motion};
//! vibrate(Vibration::Press);
//! let _motion = motion(|sample| log(sample.gravity));  // runs while kept
//! ```

use std::sync::atomic::{AtomicU32, Ordering};

use js_sys::{Function, JSON, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{Closure, JsValue};
use web_sys::CustomEvent;

pub mod message;

pub use message::{Allowed, HapticEvent, Info, Motion, Orientation, Vibration};

/// Whether the page runs in the app.
pub fn available() -> bool {
    handler().is_some()
}

/// The player and the orientation, from the app; `None` in a browser.
pub fn info() -> Option<Info> {
    let window = web_sys::window()?;
    let value = Reflect::get(&window, &JsValue::from_str(message::NAME)).ok()?;
    message::read_info(&stringify(&value)?)
}

pub fn vibrate(vibration: Vibration) {
    post(&message::vibrate(vibration));
}

/// A pattern of your own: taps and continuous buzzes.
pub fn haptic(events: &[HapticEvent]) {
    post(&message::haptic(events));
}

pub fn set_orientation(allowed: Allowed) {
    post(&message::orientation(allowed));
}

/// Calls `callback` at each orientation change, while the listener is kept.
pub fn on_orientation(mut callback: impl FnMut(Orientation) + 'static) -> Listener {
    Listener::new("orientation", move |detail| {
        if let Some(orientation) = message::read_orientation(&detail) {
            callback(orientation);
        }
    })
}

/// Motion, once per screen refresh, while the returned value is kept.
pub fn motion(mut callback: impl FnMut(Motion) + 'static) -> Motions {
    post(&message::motion(true));
    Motions {
        _listener: Listener::new("motion", move |detail| {
            if let Some(sample) = message::read_motion(&detail) {
                callback(sample);
            }
        }),
    }
}

/// Motion keeps coming until this is dropped.
pub struct Motions {
    /// Held for its drop, which stops listening.
    _listener: Listener,
}

impl Drop for Motions {
    fn drop(&mut self) {
        post(&message::motion(false));
    }
}

/// A UDP socket to the game's host. Datagrams can be lost or come twice:
/// send whole states, not changes. Closed when dropped.
pub struct Datagram {
    id: u32,
    listener: Option<Listener>,
}

impl Datagram {
    pub fn open(port: u16) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(1);
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        post(&message::open(id, port));
        Self { id, listener: None }
    }

    pub fn send(&self, data: &[u8]) {
        post(&message::send(self.id, data));
    }

    /// Calls `callback` with each datagram the host sends back.
    pub fn on_message(&mut self, mut callback: impl FnMut(Vec<u8>) + 'static) {
        let id = self.id;
        self.listener = Some(Listener::new("datagram", move |detail| {
            if let Some((from, bytes)) = message::read_datagram(&detail)
                && from == id
            {
                callback(bytes);
            }
        }));
    }
}

impl Drop for Datagram {
    fn drop(&mut self) {
        post(&message::close(self.id));
    }
}

/// Listens to one of the app's events on `window` until dropped.
pub struct Listener {
    event: String,
    closure: Closure<dyn FnMut(CustomEvent)>,
}

impl Listener {
    /// `callback` gets the event's detail as JSON.
    fn new(name: &str, mut callback: impl FnMut(String) + 'static) -> Self {
        let event = format!("{}:{name}", message::NAME);
        let closure = Closure::<dyn FnMut(CustomEvent)>::new(move |event: CustomEvent| {
            if let Some(detail) = stringify(&event.detail()) {
                callback(detail);
            }
        });
        if let Some(window) = web_sys::window() {
            // Without it, the event never comes: the same as outside the app.
            let _: Result<(), JsValue> =
                window.add_event_listener_with_callback(&event, closure.as_ref().unchecked_ref());
        }
        Self { event, closure }
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            let _: Result<(), JsValue> = window.remove_event_listener_with_callback(
                &self.event,
                self.closure.as_ref().unchecked_ref(),
            );
        }
    }
}

/// `window.webkit.messageHandlers.telecommande`, in the app only.
fn handler() -> Option<JsValue> {
    let mut value: JsValue = web_sys::window()?.into();
    for key in ["webkit", "messageHandlers", message::NAME] {
        value = Reflect::get(&value, &JsValue::from_str(key)).ok()?;
        if value.is_undefined() || value.is_null() {
            return None;
        }
    }
    Some(value)
}

/// Posts a message, given as JSON, to the app; nothing in a browser.
fn post(json: &str) {
    let Some(handler) = handler() else {
        return;
    };
    let (Ok(message), Ok(post)) = (
        JSON::parse(json),
        Reflect::get(&handler, &JsValue::from_str("postMessage")),
    ) else {
        return;
    };
    if let Some(post) = post.dyn_ref::<Function>() {
        // The app's handler returns nothing worth reading.
        let _: Result<JsValue, JsValue> = post.call1(&handler, &message);
    }
}

fn stringify(value: &JsValue) -> Option<String> {
    JSON::stringify(value).ok()?.as_string()
}
