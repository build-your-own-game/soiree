//! A solo browser game's typed journal, persisted by the common native
//! sink without moving the game state out of its browser. Requests are
//! ordered, bounded, and a failed request stops recording observably.

use salon_core::{
    Ms,
    journal::{Chunk, Line, Submission},
};
use serde::Serialize;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::marker::PhantomData;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{RequestInit, Response};

const QUEUE: usize = 1024;
const BYTE_BUDGET: usize = 4 * 1024 * 1024;
const OWNER_KEY: &str = "salon.journal.owner.v1";
struct Inner {
    path: String,
    session: String,
    queue: VecDeque<Pending>,
    bytes: usize,
    sequence: u64,
    running: bool,
    finished: bool,
    error: Option<String>,
}

struct Pending {
    sequence: u64,
    text: String,
    delivery: Delivery,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Delivery {
    Active,
    Terminal,
}

pub struct Recorder<H, E> {
    inner: Rc<RefCell<Inner>>,
    types: PhantomData<fn() -> (H, E)>,
}

impl<H: Serialize, E: Serialize> Recorder<H, E> {
    pub fn start(path: &str, header: H) -> Result<Self, JsValue> {
        crate::install_tv_diagnostics();
        let random = crate::random_bytes()
            .ok_or_else(|| JsValue::from_str("journal: no browser randomness"))?;
        let recorder = Self {
            inner: Rc::new(RefCell::new(Inner {
                path: path.to_owned(),
                session: format!("{}-{}", owner()?, crate::hex(&random)),
                queue: VecDeque::new(),
                bytes: 0,
                sequence: 0,
                running: false,
                finished: false,
                error: None,
            })),
            types: PhantomData,
        };
        recorder.send(Line::Header(header))?;
        Ok(recorder)
    }
    pub fn record(&self, entry: E) -> Result<(), JsValue> {
        self.send(Line::Entry(entry))
    }
    pub fn marker(&self, at: Ms, note: impl Into<String>) -> Result<(), JsValue> {
        self.send(Line::Marker {
            at,
            note: note.into(),
        })
    }
    pub fn checkpoint(&self, fingerprint: impl Into<String>) -> Result<(), JsValue> {
        self.send(Line::Checkpoint {
            fingerprint: fingerprint.into(),
        })
    }
    pub fn finish(&self, fingerprint: impl Into<String>) -> Result<(), JsValue> {
        self.send(Line::Final {
            fingerprint: fingerprint.into(),
        })?;
        self.inner.borrow_mut().finished = true;
        Ok(())
    }
    pub fn error(&self) -> Option<String> {
        self.inner.borrow().error.clone()
    }
    fn send(&self, line: Line<H, E>) -> Result<(), JsValue> {
        let mut inner = self.inner.borrow_mut();
        if let Some(error) = &inner.error {
            return Err(JsValue::from_str(error));
        }
        if inner.finished {
            return Err(JsValue::from_str("journal already finished"));
        }
        if inner.queue.len() >= QUEUE {
            inner.error = Some("journal queue full: recording stopped".to_owned());
            return Err(JsValue::from_str("journal queue full: recording stopped"));
        }
        let delivery = if matches!(&line, Line::Final { .. }) {
            Delivery::Terminal
        } else {
            Delivery::Active
        };
        let submission = Submission {
            session: inner.session.clone(),
            sequence: inner.sequence,
            line,
        };
        let text = match encode(&submission, BYTE_BUDGET.saturating_sub(inner.bytes)) {
            Ok(text) => text,
            Err(error) => {
                inner.error = Some(error.clone());
                return Err(JsValue::from_str(&error));
            }
        };
        inner.sequence = inner
            .sequence
            .checked_add(1)
            .ok_or_else(|| JsValue::from_str("journal sequence exhausted"))?;
        inner.bytes = inner.bytes.saturating_add(text.len());
        let sequence = inner.sequence.saturating_sub(1);
        inner.queue.push_back(Pending {
            sequence,
            text,
            delivery,
        });
        if !inner.running {
            inner.running = true;
            let shared = Rc::clone(&self.inner);
            spawn_local(async move {
                pump(shared).await;
            });
        }
        Ok(())
    }
}

struct BoundedBuffer {
    bytes: Vec<u8>,
    remaining: usize,
}
impl std::io::Write for BoundedBuffer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > self.remaining {
            return Err(std::io::Error::other(
                "journal byte budget exceeded: recording stopped",
            ));
        }
        self.remaining = self.remaining.saturating_sub(bytes.len());
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize, remaining: usize) -> Result<String, String> {
    let mut output = BoundedBuffer {
        bytes: Vec::new(),
        remaining,
    };
    serde_json::to_writer(&mut output, value).map_err(|error| error.to_string())?;
    String::from_utf8(output.bytes).map_err(|error| error.to_string())
}

async fn pump(shared: Rc<RefCell<Inner>>) {
    loop {
        let request = {
            let mut inner = shared.borrow_mut();
            if let Some(text) = inner.queue.pop_front() {
                Some((inner.path.clone(), inner.session.clone(), text))
            } else {
                inner.running = false;
                None
            }
        };
        let Some((path, session, pending)) = request else {
            return;
        };
        if let Err(error) = post(
            &path,
            &session,
            pending.sequence,
            &pending.text,
            pending.delivery,
        )
        .await
        {
            let mut inner = shared.borrow_mut();
            inner.error = Some(format!("journal delivery failed: {error:?}"));
            inner.running = false;
            inner.queue.clear();
            inner.bytes = 0;
            return;
        }
        let mut inner = shared.borrow_mut();
        inner.bytes = inner.bytes.saturating_sub(pending.text.len());
    }
}

fn owner() -> Result<String, JsValue> {
    let storage = web_sys::window().and_then(|window| window.session_storage().ok().flatten());
    if let Some(value) = storage
        .as_ref()
        .and_then(|storage| storage.get_item(OWNER_KEY).ok().flatten())
        && crate::unhex(&value).is_some()
    {
        return Ok(value);
    }
    let random =
        crate::random_bytes().ok_or_else(|| JsValue::from_str("journal: no browser randomness"))?;
    let owner = crate::hex(&random);
    if let Some(storage) = storage {
        let _: Result<(), JsValue> = storage.set_item(OWNER_KEY, &owner);
    }
    Ok(owner)
}

async fn post(
    path: &str,
    session: &str,
    sequence: u64,
    text: &str,
    delivery: Delivery,
) -> Result<(), JsValue> {
    if text.len() <= 32 * 1024 {
        return request(path, text, delivery).await;
    }
    let mut offset = 0_usize;
    for bytes in text.as_bytes().chunks(8 * 1024) {
        let chunk = Chunk {
            session: session.to_owned(),
            sequence,
            offset,
            total: text.len(),
            bytes: bytes.to_vec(),
        };
        let body =
            serde_json::to_string(&chunk).map_err(|error| JsValue::from_str(&error.to_string()))?;
        offset = offset.saturating_add(bytes.len());
        let piece_delivery = if offset == text.len() {
            delivery
        } else {
            Delivery::Active
        };
        request(path, &body, piece_delivery).await?;
    }
    Ok(())
}

async fn request(path: &str, text: &str, delivery: Delivery) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let init = RequestInit::new();
    init.set_method("POST");
    init.set_body(&JsValue::from_str(text));
    // Bulk recording runs while the page is alive. Only the terminal
    // pagehide request spends the browser's small keepalive quota.
    if delivery == Delivery::Terminal {
        js_sys::Reflect::set(
            init.as_ref(),
            &JsValue::from_str("keepalive"),
            &JsValue::TRUE,
        )?;
    }
    let response = JsFuture::from(window.fetch_with_str_and_init(path, &init))
        .await?
        .dyn_into::<Response>()?;
    // Consume the response before advancing the FIFO, including a terminal
    // keepalive request. Merely awaiting headers can leave request resources
    // charged against the browser's 64KiB keepalive quota.
    let _: JsValue = JsFuture::from(response.text()?).await?;
    if response.ok() {
        Ok(())
    } else {
        Err(JsValue::from_str(&format!("HTTP {}", response.status())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialization_obeys_the_byte_budget_including_json_escapes() {
        let value = "\n".repeat(100);
        assert!(encode(&value, 100).is_err());
        let text = encode(&value, 202).unwrap();
        assert_eq!(text.len(), 202);
        assert!(encode(&"x".repeat(100_000), 4_096).is_err());
    }
}
