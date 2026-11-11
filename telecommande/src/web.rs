//! Le côté navigateur : le lien d'une page vers le jeu.
//!
//! ```ignore
//! let link = telecommande::web::connect::<Up, Down>(Role::Remote, |signal| match signal {
//!     Signal::Open => { /* connecté */ }
//!     Signal::Message(down) => { /* affiche ce que le jeu envoie */ }
//!     Signal::Closed => { /* coupé : le lien se rouvre tout seul */ }
//! })?;
//! link.send(&Up::Tap);
//! ```
//!
//! La page garde son identité dans le navigateur : rechargée ou revenue de
//! veille, elle arrive au jeu sous le même [`crate::Id`]. Pour essayer
//! plusieurs joueurs dans un seul navigateur, ajoute `?j=1`, `?j=2`… à
//! l'adresse : chaque numéro est un joueur différent.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::js_sys::Object;
use web_sys::{CloseEvent, MessageEvent, UrlSearchParams, WebSocket};

use crate::{REPLACED, Role};

/// Le délai avant de rouvrir un lien coupé.
const RETRY_MS: i32 = 1000;

/// Ce qui arrive à la page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal<Down> {
    /// Le lien est ouvert : le jeu va envoyer ce que la page doit afficher.
    Open,
    /// Le jeu a envoyé un message.
    Message(Down),
    /// Le lien est coupé. Il se rouvre tout seul, sauf si la même page s'est
    /// ouverte dans un autre onglet : c'est alors l'autre qui joue.
    Closed,
}

enum Raw {
    Open,
    Text(String),
    Closed,
}

struct Handlers {
    _open: Closure<dyn FnMut()>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _close: Closure<dyn FnMut(CloseEvent)>,
}

struct Inner {
    url: String,
    socket: RefCell<Option<WebSocket>>,
    handlers: RefCell<Option<Handlers>>,
    retry: RefCell<Option<Closure<dyn FnMut()>>>,
    on_raw: RefCell<Box<dyn FnMut(Raw)>>,
}

impl Inner {
    fn emit(&self, raw: Raw) {
        if let Ok(mut on_raw) = self.on_raw.try_borrow_mut() {
            on_raw(raw);
        }
    }
}

/// Le lien vers le jeu. Il vit aussi longtemps que la page.
pub struct Link<Up> {
    inner: Rc<Inner>,
    up: PhantomData<fn(Up)>,
}

impl<Up> Clone for Link<Up> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            up: PhantomData,
        }
    }
}

impl<Up: Serialize> Link<Up> {
    /// Envoie un message au jeu. Sans effet tant que le lien est coupé.
    pub fn send(&self, message: &Up) {
        let text = match serde_json::to_string(message) {
            Ok(text) => text,
            Err(error) => return log(&format!("message impossible à écrire : {error}")),
        };
        if let Some(socket) = self.inner.socket.borrow().as_ref()
            && socket.ready_state() == WebSocket::OPEN
        {
            let _: Result<(), JsValue> = socket.send_with_str(&text);
        }
    }
}

/// Ouvre le lien vers le jeu qui a servi cette page, et le rouvre à chaque
/// coupure. `on_signal` est appelé pour tout ce qui arrive.
pub fn connect<Up, Down: DeserializeOwned + 'static>(
    role: Role,
    mut on_signal: impl FnMut(Signal<Down>) + 'static,
) -> Result<Link<Up>, JsValue> {
    let on_raw = move |raw| match raw {
        Raw::Open => on_signal(Signal::Open),
        Raw::Closed => on_signal(Signal::Closed),
        Raw::Text(text) => match serde_json::from_str(&text) {
            Ok(message) => on_signal(Signal::Message(message)),
            Err(error) => log(&format!("message illisible du jeu : {error}")),
        },
    };
    let inner = Rc::new(Inner {
        url: url(role)?,
        socket: RefCell::new(None),
        handlers: RefCell::new(None),
        retry: RefCell::new(None),
        on_raw: RefCell::new(Box::new(on_raw)),
    });
    open(&inner);
    Ok(Link {
        inner,
        up: PhantomData,
    })
}

fn open(inner: &Rc<Inner>) {
    let Ok(socket) = WebSocket::new(&inner.url) else {
        return retry_later(inner);
    };
    let on_open = {
        let inner = Rc::clone(inner);
        Closure::<dyn FnMut()>::new(move || inner.emit(Raw::Open))
    };
    let on_message = {
        let inner = Rc::clone(inner);
        Closure::<dyn FnMut(MessageEvent)>::new(move |event: MessageEvent| {
            if let Some(text) = event.data().as_string() {
                inner.emit(Raw::Text(text));
            }
        })
    };
    let on_close = {
        let inner = Rc::clone(inner);
        Closure::<dyn FnMut(CloseEvent)>::new(move |event: CloseEvent| {
            inner.emit(Raw::Closed);
            // Remplacé par un autre onglet de la même page : celui-ci s'arrête là.
            if event.code() != REPLACED {
                retry_later(&inner);
            }
        })
    };
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    *inner.socket.borrow_mut() = Some(socket);
    *inner.handlers.borrow_mut() = Some(Handlers {
        _open: on_open,
        _message: on_message,
        _close: on_close,
    });
}

fn retry_later(inner: &Rc<Inner>) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let again = {
        let inner = Rc::clone(inner);
        Closure::<dyn FnMut()>::new(move || open(&inner))
    };
    let _: Result<i32, JsValue> = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        again.as_ref().unchecked_ref(),
        RETRY_MS,
    );
    *inner.retry.borrow_mut() = Some(again);
}

/// L'adresse du lien : le jeu qui a servi la page, l'identité de la page et
/// son rôle.
fn url(role: Role) -> Result<String, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("pas de fenêtre"))?;
    let location = window.location();
    let scheme = if location.protocol()? == "https:" {
        "wss"
    } else {
        "ws"
    };
    let host = location.host()?;
    let seat = UrlSearchParams::new_with_str(&location.search()?)?
        .get("j")
        .unwrap_or_default();
    let key = format!("soiree-id-{}-{seat}", role.as_str());
    // Sans stockage (navigation privée stricte), la page est neuve à chaque fois.
    let storage = window.local_storage().ok().flatten();
    let known = storage
        .as_ref()
        .and_then(|storage| storage.get_item(&key).ok().flatten())
        .and_then(|text| text.parse::<u64>().ok());
    let id = if let Some(id) = known {
        id
    } else {
        let mut bytes = [0_u8; 8];
        let _: Object = window
            .crypto()?
            .get_random_values_with_u8_array(&mut bytes)?;
        let id = u64::from_le_bytes(bytes);
        if let Some(storage) = &storage {
            let _: Result<(), JsValue> = storage.set_item(&key, &id.to_string());
        }
        id
    };
    Ok(format!(
        "{scheme}://{host}/ws?id={id}&role={}",
        role.as_str()
    ))
}

fn log(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}
