//! Web Audio boundary: no queued sounds or unbounded sources.

use super::{Cue, Gate, MAX_VOICES};
use crate::ui::{Listeners, Timer};
use js_sys::Reflect;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    AudioContext, AudioNode, BaseAudioContext, Event, GainNode, HtmlButtonElement, HtmlDivElement,
    HtmlInputElement, KeyboardEvent, OscillatorNode,
};

const DURATION: f64 = 0.18;
const SETTINGS_KEY: &str = "soiree/audio/v1";
#[derive(serde::Serialize, serde::Deserialize)]
struct Settings {
    muted: bool,
    volume: f32,
}
fn settings() -> Settings {
    web_sys::window()
        .and_then(|window| window.local_storage().ok().flatten())
        .and_then(|storage| storage.get_item(SETTINGS_KEY).ok().flatten())
        .and_then(|json| serde_json::from_str::<Settings>(&json).ok())
        .filter(|settings| settings.volume.is_finite() && (0.0..=1.0).contains(&settings.volume))
        .unwrap_or(Settings {
            muted: false,
            volume: 0.6,
        })
}

/// One owner per TV page. Drop stops/disconnects sources, closes the context
/// and removes controls and subscriptions. No network task waits for sound.
pub struct Audio {
    inner: Rc<RefCell<Inner>>,
    controls: HtmlDivElement,
    listeners: Listeners,
    _reaper: Timer,
}
struct Inner {
    context: Option<AudioContext>,
    master: Option<GainNode>,
    voices: Vec<Voice>,
    gate: Gate,
    muted: bool,
    volume: f32,
    resume_pending: bool,
    failed: bool,
    button: HtmlButtonElement,
    context_listeners: Listeners,
}

pub(crate) struct Voice {
    oscillator: OscillatorNode,
    gain: GainNode,
    ends: f64,
}
impl Drop for Voice {
    fn drop(&mut self) {
        let _: Result<(), JsValue> = self.oscillator.stop();
        let _: Result<(), JsValue> = self.oscillator.disconnect();
        let _: Result<(), JsValue> = self.gain.disconnect();
    }
}

/// The exact production graph, also exercised with `OfflineAudioContext`.
pub(crate) fn schedule(
    context: &BaseAudioContext,
    output: &AudioNode,
    cue: Cue,
) -> Result<Voice, JsValue> {
    let now = context.current_time();
    let voice = Voice {
        oscillator: context.create_oscillator()?,
        gain: context.create_gain()?,
        ends: now + DURATION,
    };
    let (start, end) = cue.frequencies();
    voice.oscillator.frequency().set_value_at_time(start, now)?;
    voice
        .oscillator
        .frequency()
        .exponential_ramp_to_value_at_time(end, now + DURATION)?;
    voice.gain.gain().set_value_at_time(0.0, now)?;
    voice
        .gain
        .gain()
        .linear_ramp_to_value_at_time(0.12, now + 0.012)?;
    voice
        .gain
        .gain()
        .linear_ramp_to_value_at_time(0.0, now + DURATION)?;
    voice.oscillator.connect_with_audio_node(&voice.gain)?;
    voice.gain.connect_with_audio_node(output)?;
    voice.oscillator.start_with_when(now)?;
    voice.oscillator.stop_with_when(voice.ends)?;
    Ok(voice)
}
fn hidden() -> bool {
    web_sys::window()
        .and_then(|window| window.document())
        .is_none_or(|document| document.hidden())
}
fn running(context: &AudioContext) -> bool {
    // Safari's "interrupted" has existed outside the WebIDL enum. Every state
    // except exactly running is unavailable, including unknown future states.
    Reflect::get(context, &JsValue::from_str("state"))
        .ok()
        .and_then(|state| state.as_string())
        .is_some_and(|state| state == "running")
}
impl Inner {
    fn new(button: HtmlButtonElement) -> Self {
        let settings = settings();
        Self {
            context: None,
            master: None,
            voices: Vec::new(),
            gate: Gate::default(),
            muted: settings.muted,
            volume: settings.volume,
            resume_pending: false,
            failed: false,
            button,
            context_listeners: Listeners::default(),
        }
    }

    fn reap(&mut self) {
        if let Some(context) = &self.context {
            if running(context) {
                let now = context.current_time();
                self.voices.retain(|voice| voice.ends > now);
            } else {
                self.voices.clear();
            }
        }
    }
    fn refresh(&self) {
        let text = if self.failed {
            "Son indisponible · réessayer"
        } else if self.muted {
            "Son coupé · activer"
        } else if self.context.as_ref().is_some_and(running) {
            "Son activé · couper"
        } else {
            "Activer le son"
        };
        self.button.set_text_content(Some(text));
        self.button
            .set_title("Son de cet écran TV. Le volume et la coupure restent sous votre contrôle.");
    }
    fn save_settings(&self) {
        let Some(storage) =
            web_sys::window().and_then(|window| window.local_storage().ok().flatten())
        else {
            return;
        };
        if let Ok(json) = serde_json::to_string(&Settings {
            muted: self.muted,
            volume: self.volume,
        }) {
            let _: Result<(), JsValue> = storage.set_item(SETTINGS_KEY, &json);
        }
    }
    fn set_volume(&mut self, volume: f32) {
        self.volume = if volume.is_finite() {
            volume.clamp(0.0, 1.0)
        } else {
            0.0
        };
        if let Some(master) = &self.master {
            master
                .gain()
                .set_value(if self.muted { 0.0 } else { self.volume });
        }
    }
}
#[expect(
    clippy::disallowed_methods,
    reason = "self-contained widget owns these literal browser names and nodes; no game file contract crosses this boundary"
)]
fn controls(
    document: &web_sys::Document,
) -> Result<(HtmlDivElement, HtmlButtonElement, HtmlInputElement), JsValue> {
    let controls = document
        .create_element("div")?
        .dyn_into::<HtmlDivElement>()?;
    let button = document
        .create_element("button")?
        .dyn_into::<HtmlButtonElement>()?;
    button.set_type("button");
    let volume = document
        .create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    volume.set_type("range");
    volume.set_min("0");
    volume.set_max("100");
    volume.set_value("60");
    volume.set_attribute("aria-label", "Volume du jeu")?;
    controls.set_attribute("role", "group")?;
    controls.set_attribute("aria-label", "Son du jeu")?;
    controls.set_attribute("style", "position:fixed;right:calc(5% + 8px);top:calc(5% + 8px);z-index:1000;display:flex;align-items:center;gap:6px;padding:5px 8px;border-radius:8px;background:#111d;color:white;font:16px system-ui")?;
    volume.style().set_property("width", "70px")?;
    controls.append_child(&button)?;
    controls.append_child(&volume)?;
    Ok((controls, button, volume))
}

impl Audio {
    /// `AudioContext` is created inside a trusted gesture, not during startup.
    pub fn new() -> Result<Self, JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let (controls, button, volume) = controls(&document)?;
        let inner = Rc::new(RefCell::new(Inner::new(button.clone())));
        volume.set_value(&(inner.borrow().volume * 100.0).to_string());
        inner.borrow().refresh();
        let listeners = Listeners::default();
        // Native range/button keys belong to this widget, not the game's
        // document/window shortcuts (Enter/Space must not advance a round).
        listeners
            .listen::<KeyboardEvent>(&controls, "keydown", |event| event.stop_propagation())?;
        let weak = Rc::downgrade(&inner);
        let control_target = controls.clone();
        listeners.listen::<Event>(&document, "pointerdown", move |event| {
            if event.is_trusted()
                && event.target().is_none_or(|target| {
                    !control_target.contains(target.dyn_ref::<web_sys::Node>())
                })
            {
                unlock(&weak);
            }
        })?;
        let weak = Rc::downgrade(&inner);
        let control_target = controls.clone();
        listeners.listen::<KeyboardEvent>(&document, "keydown", move |event| {
            if event.is_trusted()
                && !event.repeat()
                && !event.is_composing()
                && event.target().is_none_or(|target| {
                    !control_target.contains(target.dyn_ref::<web_sys::Node>())
                })
            {
                unlock(&weak);
            }
        })?;
        let weak = Rc::downgrade(&inner);
        listeners.listen::<Event>(&button, "click", move |event| {
            if !event.is_trusted() {
                return;
            }
            let Some(inner) = weak.upgrade() else {
                return;
            };
            {
                let mut state = inner.borrow_mut();
                if state.context.as_ref().is_some_and(running) && !state.muted {
                    state.muted = true;
                    state.voices.clear();
                    let volume = state.volume;
                    state.set_volume(volume);
                    state.save_settings();
                    state.refresh();
                    return;
                }
                state.muted = false;
                let volume = state.volume;
                state.set_volume(volume);
                state.save_settings();
            }
            unlock(&weak);
        })?;
        let weak = Rc::downgrade(&inner);
        listeners.listen::<Event>(&volume, "input", move |event| {
            let Some(inner) = weak.upgrade() else {
                return;
            };
            let Some(input) = event
                .target()
                .and_then(|target| target.dyn_into::<HtmlInputElement>().ok())
            else {
                return;
            };
            let mut state = inner.borrow_mut();
            state.set_volume(input.value().parse::<f32>().unwrap_or(0.0) / 100.0);
            state.save_settings();
        })?;
        let weak = Rc::downgrade(&inner);
        listeners.listen::<Event>(&document, "visibilitychange", move |_| {
            if hidden() {
                suspend(&weak);
            } else {
                resume_existing(&weak);
            }
        })?;
        let weak = Rc::downgrade(&inner);
        listeners.listen::<Event>(&window, "pagehide", move |_| suspend(&weak))?;
        let weak = Rc::downgrade(&inner);
        listeners.listen::<Event>(&window, "pageshow", move |_| resume_existing(&weak))?;
        let weak = Rc::downgrade(&inner);
        let reaper = Timer::every(200, move || {
            if let Some(inner) = weak.upgrade() {
                inner.borrow_mut().reap();
            }
        })?;
        document.body().ok_or("no body")?.append_child(&controls)?;
        Ok(Self {
            inner,
            controls,
            listeners,
            _reaper: reaper,
        })
    }
    /// Observe before checking availability: blocked and muted cues are consumed.
    pub fn observe(&self, sequence: u64, at_ms: f64, now_ms: f64, cue: Cue) {
        let mut inner = self.inner.borrow_mut();
        if !inner.gate.observe(sequence, at_ms, now_ms) {
            return;
        }
        inner.reap();
        if hidden() || inner.muted || inner.voices.len() >= MAX_VOICES {
            return;
        }
        let Some(context) = &inner.context else {
            return;
        };
        if !running(context) {
            return;
        }
        let Some(master) = &inner.master else {
            return;
        };
        if let Ok(voice) = schedule(context, master, cue) {
            inner.voices.push(voice);
        } else {
            inner.failed = true;
            inner.voices.clear();
            inner.refresh();
        }
    }
    pub fn active_voices(&self) -> usize {
        self.inner.borrow().voices.len()
    }

    /// A new connection/room starts silently without changing player settings.
    pub fn discontinuity(&self) {
        let mut inner = self.inner.borrow_mut();
        inner.gate.discontinuity();
        inner.voices.clear();
    }
}
fn unlock(weak: &Weak<RefCell<Inner>>) {
    let Some(inner) = weak.upgrade() else {
        return;
    };
    {
        let mut state = inner.borrow_mut();
        if state.muted {
            return;
        }
        if state.resume_pending
            || state.context.as_ref().is_some_and(|context| {
                Reflect::get(context, &JsValue::from_str("state"))
                    .ok()
                    .and_then(|value| value.as_string())
                    .is_some_and(|value| value == "closed")
            })
        {
            state.voices.clear();
            state.context_listeners.clear();
            if let Some(master) = state.master.take() {
                let _: Result<(), JsValue> = master.disconnect();
            }
            if let Some(context) = state.context.take() {
                consume(context.close());
            }
            state.resume_pending = false;
        }
        if state.context.is_none() {
            let prepared = AudioContext::new().and_then(|context| {
                let prepared = context.create_gain().and_then(|master| {
                    master.gain().set_value(state.volume);
                    master.connect_with_audio_node(&context.destination())?;
                    Ok(master)
                });
                match prepared {
                    Ok(master) => Ok((context, master)),
                    Err(error) => {
                        consume(context.close());
                        Err(error)
                    }
                }
            });
            if let Ok((context, master)) = prepared {
                let weak = weak.clone();
                if state
                    .context_listeners
                    .listen::<Event>(&context, "statechange", move |_| {
                        if let Some(inner) = weak.upgrade() {
                            let mut state = inner.borrow_mut();
                            state.reap();
                            state.refresh();
                        }
                    })
                    .is_err()
                {
                    consume(context.close());
                    state.failed = true;
                    state.refresh();
                    return;
                }
                state.context = Some(context);
                state.master = Some(master);
                state.failed = false;
            } else {
                state.failed = true;
                state.refresh();
                return;
            }
        }
    }
    resume_existing(weak);
}
fn resume_existing(weak: &Weak<RefCell<Inner>>) {
    let Some(inner) = weak.upgrade() else {
        return;
    };
    let mut state = inner.borrow_mut();
    state.refresh();
    if hidden() || state.muted || state.resume_pending {
        return;
    }
    let Some(context) = &state.context else {
        return;
    };
    if running(context) {
        return;
    }
    let expected = context.clone();
    if let Ok(promise) = context.resume() {
        state.resume_pending = true;
        let weak = weak.clone();
        spawn_local(async move {
            let result = JsFuture::from(promise).await;
            if let Some(inner) = weak.upgrade() {
                let mut state = inner.borrow_mut();
                if state.context.as_ref() != Some(&expected) {
                    return;
                }
                state.resume_pending = false;
                state.failed = result.is_err();
                state.refresh();
            }
        });
    } else {
        state.failed = true;
        state.refresh();
    }
}
fn consume(result: Result<js_sys::Promise, JsValue>) {
    if let Ok(promise) = result {
        spawn_local(async move {
            let _: Result<JsValue, JsValue> = JsFuture::from(promise).await;
        });
    }
}
fn suspend(weak: &Weak<RefCell<Inner>>) {
    if let Some(inner) = weak.upgrade() {
        let mut state = inner.borrow_mut();
        state.voices.clear();
        if let Some(context) = &state.context {
            consume(context.suspend());
        }
    }
}
impl Drop for Audio {
    fn drop(&mut self) {
        self.listeners.clear();
        self.controls.remove();
        let mut state = self.inner.borrow_mut();
        state.context_listeners.clear();
        state.voices.clear();
        if let Some(master) = state.master.take() {
            let _: Result<(), JsValue> = master.disconnect();
        }
        if let Some(context) = state.context.take() {
            consume(context.close());
        }
    }
}

/// Test-only entry: exact production oscillator/envelope, real browser DSP.
#[cfg(feature = "audio-test")]
pub async fn render_reference(cue: Cue) -> Result<web_sys::AudioBuffer, JsValue> {
    let context =
        web_sys::OfflineAudioContext::new_with_number_of_channels_and_length_and_sample_rate(
            1, 24_000, 48_000.0,
        )?;
    let voice = schedule(&context, &context.destination(), cue)?;
    let buffer = JsFuture::from(context.start_rendering()?)
        .await?
        .dyn_into()?;
    drop(voice);
    Ok(buffer)
}
