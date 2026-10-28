use std::cell::RefCell;

use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, EventTarget};

/// An event subscription, removed (and its callback freed) on drop.
///
/// DOM operations can dispatch another event synchronously, including the
/// same event while this handler is running. `Fn` preserves that reentrancy.
/// Keep mutable state in the owner's `Cell` or `RefCell`, and release mutable
/// borrows before calling DOM operations which can dispatch events.
#[must_use = "store the subscription in its owning page or view"]
pub struct Listener {
    target: EventTarget,
    name: String,
    callback: Closure<dyn Fn(Event)>,
}

impl Listener {
    #[expect(
        clippy::disallowed_methods,
        reason = "owned event subscription boundary; Drop unregisters this exact callback"
    )]
    pub fn new<E: JsCast + 'static>(
        target: &impl AsRef<EventTarget>,
        name: &str,
        handler: impl Fn(E) + 'static,
    ) -> Result<Self, JsValue> {
        let target = target.as_ref().clone();
        let callback = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            if let Ok(event) = event.dyn_into::<E>() {
                handler(event);
            }
        });
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            target,
            name: name.to_owned(),
            callback,
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _: Result<(), JsValue> = self.target.remove_event_listener_with_callback(
            &self.name,
            self.callback.as_ref().unchecked_ref(),
        );
    }
}

/// Subscriptions owned by one page or view, including views with `&self` setup.
#[derive(Default)]
pub struct Listeners(RefCell<Vec<Listener>>);

impl Listeners {
    pub fn listen<E: JsCast + 'static>(
        &self,
        target: &impl AsRef<EventTarget>,
        name: &str,
        handler: impl Fn(E) + 'static,
    ) -> Result<(), JsValue> {
        self.0
            .borrow_mut()
            .push(Listener::new(target, name, handler)?);
        Ok(())
    }

    /// Cancel this scope's subscriptions without destroying its owner.
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub fn len(&self) -> usize {
        self.0.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.borrow().is_empty()
    }
}

/// A browser timeout or interval, cancelled on drop.
#[must_use = "store the timer until it fires or its operation is cancelled"]
pub struct Timer {
    id: i32,
    _callback: Closure<dyn FnMut()>,
}

#[derive(Clone, Copy)]
enum Repeat {
    Once,
    Every,
}

impl Timer {
    pub fn after(milliseconds: i32, callback: impl FnMut() + 'static) -> Result<Self, JsValue> {
        Self::new(milliseconds, Repeat::Once, callback)
    }

    pub fn every(milliseconds: i32, callback: impl FnMut() + 'static) -> Result<Self, JsValue> {
        Self::new(milliseconds, Repeat::Every, callback)
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "owned timer boundary; Drop cancels before freeing its callback"
    )]
    fn new(
        milliseconds: i32,
        repeat: Repeat,
        callback: impl FnMut() + 'static,
    ) -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let callback = Closure::<dyn FnMut()>::new(callback);
        let function = callback.as_ref().unchecked_ref();
        let id = match repeat {
            Repeat::Once => {
                window.set_timeout_with_callback_and_timeout_and_arguments_0(function, milliseconds)
            }
            Repeat::Every => window
                .set_interval_with_callback_and_timeout_and_arguments_0(function, milliseconds),
        }?;
        Ok(Self {
            id,
            _callback: callback,
        })
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let Some(window) = web_sys::window() {
            // Browser timers share the same handle pool.
            window.clear_interval_with_handle(self.id);
        }
    }
}
