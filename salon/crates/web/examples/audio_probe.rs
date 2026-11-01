//! Browser-only contract fixture; never linked into a game.
#![cfg(target_arch = "wasm32")]
use salon_web::audio::{Audio, Cue};
use std::cell::RefCell;
use wasm_bindgen::prelude::*;

thread_local! { static OUTPUT: RefCell<Option<Audio>> = const { RefCell::new(None) }; }
#[wasm_bindgen]
pub fn mount() -> Result<(), JsValue> {
    OUTPUT.with_borrow_mut(|output| {
        *output = Some(Audio::new()?);
        Ok(())
    })
}
#[wasm_bindgen]
pub fn dispose() {
    OUTPUT.with_borrow_mut(|output| *output = None);
}
#[wasm_bindgen]
pub fn cue(sequence: u32, at: f64, now: f64) {
    OUTPUT.with_borrow(|output| {
        if let Some(output) = output {
            output.observe(u64::from(sequence), at, now, Cue::Advance);
        }
    });
}
#[wasm_bindgen]
pub fn discontinuity() {
    OUTPUT.with_borrow(|output| {
        if let Some(output) = output {
            output.discontinuity();
        }
    });
}
#[wasm_bindgen]
pub fn active() -> usize {
    OUTPUT.with_borrow(|output| output.as_ref().map_or(0, Audio::active_voices))
}
#[cfg(feature = "audio-test")]
#[wasm_bindgen]
pub async fn rendered() -> Result<web_sys::AudioBuffer, JsValue> {
    salon_web::audio::render_reference(Cue::Advance).await
}
