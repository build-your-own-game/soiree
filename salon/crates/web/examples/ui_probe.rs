//! Browser-only contract fixture; never linked into a game.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use salon_web::ui::{AnimationLoop, Composition, Listener, Listeners, Timer};
use salon_web::{Connection, Options};
use serde_json::json;
use wasm_bindgen::prelude::*;
use web_sys::{Event, EventTarget, KeyboardEvent, KeyboardEventInit};

struct Scope {
    _listener: Listener,
    listeners: Listeners,
    _timeout: Timer,
    _interval: Timer,
    composition: Composition,
}

thread_local! {
    static SCOPE: RefCell<Option<Scope>> = const { RefCell::new(None) };
    static EVENTS: Cell<u32> = const { Cell::new(0) };
    static TIMEOUTS: Cell<u32> = const { Cell::new(0) };
    static INTERVALS: Cell<u32> = const { Cell::new(0) };
    static CONNECTION: RefCell<Option<Connection<u8, u8>>> = const { RefCell::new(None) };
    static CONNECTION_CLONE: RefCell<Option<Connection<u8, u8>>> = const { RefCell::new(None) };
    static SELF_TIMER: RefCell<Option<Timer>> = const { RefCell::new(None) };
    static SELF_FIRED: Cell<u32> = const { Cell::new(0) };
    static FRAMES: Cell<u32> = const { Cell::new(0) };
    static FRAME_LOOP: RefCell<Option<AnimationLoop>> = const { RefCell::new(None) };
}

#[wasm_bindgen]
pub fn connect() {
    let mut options = Options::for_page(1, salon_core::Kind::Phone);
    options.websocket_only = true;
    let connection = Connection::open(options, |_| {});
    CONNECTION_CLONE.with_borrow_mut(|clone| *clone = Some(connection.clone()));
    CONNECTION.with_borrow_mut(|owner| *owner = Some(connection));
}

#[wasm_bindgen]
pub fn drop_connection() {
    CONNECTION.with_borrow_mut(|owner| *owner = None);
}

#[wasm_bindgen]
pub fn drop_connection_clone() {
    CONNECTION_CLONE.with_borrow_mut(|owner| *owner = None);
}

#[wasm_bindgen]
pub fn self_cancelling_timer() -> Result<(), JsValue> {
    let timer = Timer::after(10, || {
        SELF_TIMER.with_borrow_mut(|timer| *timer = None);
        increment(&SELF_FIRED);
    })?;
    SELF_TIMER.with_borrow_mut(|owner| *owner = Some(timer));
    Ok(())
}

#[wasm_bindgen]
pub fn self_fired() -> u32 {
    SELF_FIRED.get()
}

#[wasm_bindgen]
pub fn frame_start(mode: u8) -> Result<(), JsValue> {
    let frames = AnimationLoop::new(move |_| {
        increment(&FRAMES);
        if mode == 1 {
            frame_stop();
        }
    })?;
    FRAME_LOOP.with_borrow_mut(|owner| *owner = Some(frames));
    Ok(())
}

#[wasm_bindgen]
pub fn frame_stop() {
    FRAME_LOOP.with_borrow_mut(|owner| *owner = None);
}

#[wasm_bindgen]
pub fn frames() -> u32 {
    FRAMES.get()
}

fn increment(counter: &'static std::thread::LocalKey<Cell<u32>>) {
    counter.with(|n| n.set(n.get().saturating_add(1)));
}

/// The caller intentionally redispatches once while handling the same event.
/// Its own guard bounds this recursion; the subscription must deliver both.
#[wasm_bindgen]
pub fn reentrant_listener(target: &EventTarget, kind: u8) -> Result<String, JsValue> {
    let calls = Rc::new(Cell::new(0_u32));
    let entering = Cell::new(false);
    let nested_target = target.clone();
    let received = calls.clone();
    let handler = move |_: Event| {
        received.set(received.get().saturating_add(1));
        if entering.replace(true) {
            return;
        }
        if let Ok(event) = Event::new("soiree-ui:reentrant") {
            let _: Result<bool, JsValue> = nested_target.dispatch_event(&event);
        }
        entering.set(false);
    };
    let listeners = Listeners::default();
    let listener = if kind == 1 {
        listeners.listen::<Event>(target, "soiree-ui:reentrant", handler)?;
        None
    } else {
        Some(Listener::new::<Event>(
            target,
            "soiree-ui:reentrant",
            handler,
        )?)
    };
    target.dispatch_event(&Event::new("soiree-ui:reentrant")?)?;
    let during = calls.get();
    drop(listener);
    drop(listeners);
    target.dispatch_event(&Event::new("soiree-ui:reentrant")?)?;
    Ok(json!({ "during": during, "after_drop": calls.get() }).to_string())
}

#[wasm_bindgen]
pub fn mount() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let composition = Composition::default();
    let listeners = Listeners::default();
    let listener = Listener::new::<Event>(&window, "soiree-ui:probe", |_| increment(&EVENTS))?;
    listeners.listen::<Event>(&window, "soiree-ui:probe", |_| increment(&EVENTS))?;
    let start = composition.clone();
    listeners.listen::<Event>(&window, "compositionstart", move |_| start.start())?;
    let end = composition.clone();
    listeners.listen::<Event>(&window, "compositionend", move |_| end.end())?;
    let timeout = Timer::after(200, || increment(&TIMEOUTS))?;
    let interval = Timer::every(10, || increment(&INTERVALS))?;
    SCOPE.with_borrow_mut(|scope| {
        *scope = Some(Scope {
            _listener: listener,
            listeners,
            _timeout: timeout,
            _interval: interval,
            composition,
        });
    });
    Ok(())
}

#[wasm_bindgen]
pub fn unmount() {
    SCOPE.with_borrow_mut(|scope| *scope = None);
}

#[wasm_bindgen]
pub fn clear_listeners() {
    SCOPE.with_borrow(|scope| {
        if let Some(scope) = scope {
            scope.listeners.clear();
        }
    });
}

#[wasm_bindgen]
pub fn dispatch(kind: u8) -> Result<(), JsValue> {
    let name = match kind {
        1 => "compositionstart",
        2 => "compositionend",
        _ => "soiree-ui:probe",
    };
    web_sys::window()
        .ok_or("no window")?
        .dispatch_event(&Event::new(name)?)?;
    Ok(())
}

#[wasm_bindgen]
pub fn blocks_key(kind: u8) -> Result<bool, JsValue> {
    let init = KeyboardEventInit::new();
    init.set_is_composing(kind == 1);
    init.set_key_code(if kind == 2 { 229 } else { 13 });
    let event = KeyboardEvent::new_with_keyboard_event_init_dict("keydown", &init)?;
    Ok(SCOPE.with_borrow(|scope| {
        scope
            .as_ref()
            .is_some_and(|scope| scope.composition.blocks_key(&event))
    }))
}

#[wasm_bindgen]
pub fn stats() -> String {
    json!({
        "events": EVENTS.get(),
        "timeouts": TIMEOUTS.get(),
        "intervals": INTERVALS.get(),
    })
    .to_string()
}
