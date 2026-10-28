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

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicU32, Ordering};

use js_sys::{Function, JSON, Reflect};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::CustomEvent;

pub mod message;

pub use message::{Allowed, HapticEvent, Info, Motion, Orientation, PlayerId, Vibration};

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
pub fn on_orientation(callback: impl Fn(Orientation) + 'static) -> Listener {
    Listener::new(message::EventKind::Orientation, move |detail| {
        if let Some(orientation) = message::read_orientation(&detail) {
            callback(orientation);
        }
    })
}

/// Motion, once per screen refresh, while the returned value is kept.
pub fn motion(callback: impl Fn(Motion) + 'static) -> Motions {
    let listener = Listener::new(message::EventKind::Motion, move |detail| {
        if let Some(sample) = message::read_motion(&detail) {
            callback(sample);
        }
    });
    Motions {
        _listener: listener,
        _source: MotionSource::acquire(),
    }
}

/// Motion keeps coming until the final subscription is dropped. Independent
/// views can listen concurrently; dropping one never stops the other view.
#[must_use = "store the motion subscription while its view needs sensor samples"]
pub struct Motions {
    /// Held for its drop, which stops listening.
    _listener: Listener,
    _source: Rc<MotionSource>,
}

struct MotionSource;

thread_local! {
    static MOTION_SOURCE: RefCell<Weak<MotionSource>> = const { RefCell::new(Weak::new()) };
}

impl MotionSource {
    fn acquire() -> Rc<Self> {
        let (source, start) = MOTION_SOURCE.with_borrow_mut(|current| {
            if let Some(source) = current.upgrade() {
                (source, false)
            } else {
                let source = Rc::new(Self);
                *current = Rc::downgrade(&source);
                (source, true)
            }
        });
        // No RefCell borrow crosses the browser callback boundary.
        if start {
            post(&message::motion(true));
        }
        source
    }
}

impl Drop for MotionSource {
    fn drop(&mut self) {
        post(&message::motion(false));
    }
}

/// A UDP socket to the game's host. Datagrams can be lost or come twice:
/// send whole states, not changes. Closed when dropped.
#[must_use = "store the socket while its page or view needs datagrams"]
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
    pub fn on_message(&mut self, callback: impl Fn(Vec<u8>) + 'static) {
        let id = self.id;
        self.listener = Some(Listener::new(message::EventKind::Datagram, move |detail| {
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
#[must_use = "store the listener until its view is destroyed"]
pub struct Listener {
    _listener: Option<salon_web::ui::Listener>,
}

impl Listener {
    /// `callback` gets the event's detail as JSON.
    fn new(kind: message::EventKind, callback: impl Fn(String) + 'static) -> Self {
        let listener = web_sys::window().and_then(|window| {
            salon_web::ui::Listener::new(&window, kind.name(), move |event: CustomEvent| {
                if let Some(detail) = stringify(&event.detail()) {
                    callback(detail);
                }
            })
            .ok()
        });
        Self {
            _listener: listener,
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
