use std::cell::Cell;
use std::rc::{Rc, Weak};

use wasm_bindgen::{JsCast, JsValue, closure::Closure};

struct Frame {
    id: Cell<Option<i32>>,
    active: Cell<bool>,
    callback: Closure<dyn FnMut(f64)>,
}

impl Frame {
    #[expect(
        clippy::disallowed_methods,
        reason = "owned animation boundary; Drop cancels the pending frame and callbacks capture only Weak"
    )]
    fn request(&self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let id = window.request_animation_frame(self.callback.as_ref().unchecked_ref())?;
        self.id.set(Some(id));
        Ok(())
    }
}

/// A recurring animation-frame callback owned by one page, view or gesture.
///
/// Dropping it cancels the pending browser frame. Its callback captures only
/// Weak, so the loop does not keep itself alive. A callback may drop its owner;
/// in that case it completes this frame and schedules no subsequent frame.
#[must_use = "store the animation loop in its owning page, view or gesture"]
pub struct AnimationLoop(Rc<Frame>);

impl AnimationLoop {
    pub fn new(mut handler: impl FnMut(f64) + 'static) -> Result<Self, JsValue> {
        let frame = Rc::new_cyclic(|weak: &Weak<Frame>| {
            let weak = weak.clone();
            Frame {
                id: Cell::new(None),
                active: Cell::new(true),
                callback: Closure::new(move |time: f64| {
                    if let Some(frame) = weak.upgrade().filter(|frame| frame.active.get()) {
                        frame.id.set(None);
                        handler(time);
                        if frame.active.get() && frame.request().is_err() {
                            frame.active.set(false);
                        }
                    }
                }),
            }
        });
        frame.request()?;
        Ok(Self(frame))
    }
}

impl Drop for AnimationLoop {
    fn drop(&mut self) {
        self.0.active.set(false);
        if let Some(id) = self.0.id.take()
            && let Some(window) = web_sys::window()
        {
            let _: Result<(), JsValue> = window.cancel_animation_frame(id);
        }
    }
}
