//! Real SDK fixture for shared native resource ownership.

use std::cell::{Cell, RefCell};

use remote_web::Motions;
use wasm_bindgen::prelude::*;

thread_local! {
    static FIRST: RefCell<Option<Motions>> = const { RefCell::new(None) };
    static SECOND: RefCell<Option<Motions>> = const { RefCell::new(None) };
    static FIRST_SAMPLES: Cell<u32> = const { Cell::new(0) };
    static SECOND_SAMPLES: Cell<u32> = const { Cell::new(0) };
}

#[wasm_bindgen]
pub fn subscribe(slot: u8) {
    let (owner, samples) = if slot == 0 {
        (&FIRST, &FIRST_SAMPLES)
    } else {
        (&SECOND, &SECOND_SAMPLES)
    };
    let motion = remote_web::motion(move |_| samples.with(|n| n.set(n.get().saturating_add(1))));
    owner.with_borrow_mut(|owner| *owner = Some(motion));
}

#[wasm_bindgen]
pub fn unsubscribe(slot: u8) {
    let owner = if slot == 0 { &FIRST } else { &SECOND };
    owner.with_borrow_mut(|owner| *owner = None);
}

#[wasm_bindgen]
pub fn samples(slot: u8) -> u32 {
    let samples = if slot == 0 {
        &FIRST_SAMPLES
    } else {
        &SECOND_SAMPLES
    };
    samples.get()
}
