//! Le côté navigateur : le lien d'une page vers la console.
//!
//! ```ignore
//! let link = telecommande::web::connect::<Command, View, Effect>(Role::Remote, |signal| match signal {
//!     Signal::Open(welcome) => { /* connecté : welcome.join_url, welcome.ticks_per_second */ }
//!     Signal::View { view, tick } => { /* montre ce qu'il faut montrer maintenant */ }
//!     Signal::Effect(effect) => { /* un son, une vibration */ }
//!     Signal::Closed => { /* coupé : le lien se rouvre tout seul */ }
//! })?;
//! link.send(&Command::Tap);
//! ```
//!
//! Ce qui est lourd (une image, un son) ne voyage pas dans une commande : la
//! page le confie à la console, qui rend un ticket, et la commande porte le
//! ticket.
//!
//! ```ignore
//! telecommande::web::upload(&bytes, "image/webp", move |kept| match kept {
//!     Ok(content) => link.send(&Command::AddImage(content)),
//!     Err(why) => { /* le dire au joueur */ }
//! });
//! // Pour le montrer : <img src="{content_url(content)}">
//! ```
//!
//! La page garde son identité dans le navigateur : rechargée ou revenue de
//! veille, elle arrive à la machine sous le même numéro de [`crate::Page`].
//! Pour essayer plusieurs joueurs dans un seul navigateur, ajoute `?j=1`,
//! `?j=2`… à l'adresse : chaque numéro est un joueur différent.

use std::cell::RefCell;
use std::marker::PhantomData;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::js_sys::Object;
use web_sys::{Blob, CloseEvent, MessageEvent, UrlSearchParams, WebSocket, XmlHttpRequest};

use crate::{Content, REPLACED, Role, Welcome, Wire};

/// Le délai avant de rouvrir un lien coupé.
const RETRY_MS: i32 = 1000;

/// Ce qui arrive à la page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal<View, Effect> {
    /// Le lien est ouvert et la console s'est présentée.
    Open(Welcome),
    /// Ce que la page doit montrer maintenant.
    View { view: View, tick: u64 },
    /// À jouer une fois.
    Effect(Effect),
    /// Le lien est coupé. Il se rouvre tout seul, sauf si la même page s'est
    /// ouverte dans un autre onglet : c'est alors l'autre qui joue.
    Closed,
}

enum Raw {
    Text(String),
    Closed,
}

struct Handlers {
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

/// Le lien vers la console. Il vit aussi longtemps que la page.
pub struct Link<Command> {
    inner: Rc<Inner>,
    command: PhantomData<fn(Command)>,
}

impl<Command> Clone for Link<Command> {
    fn clone(&self) -> Self {
        Self {
            inner: Rc::clone(&self.inner),
            command: PhantomData,
        }
    }
}

impl<Command: Serialize> Link<Command> {
    /// Envoie une commande à la machine. Sans effet tant que le lien est coupé.
    pub fn send(&self, command: &Command) {
        let text = match serde_json::to_string(command) {
            Ok(text) => text,
            Err(error) => return log(&format!("commande impossible à écrire : {error}")),
        };
        if let Some(socket) = self.inner.socket.borrow().as_ref()
            && socket.ready_state() == WebSocket::OPEN
        {
            let _: Result<(), JsValue> = socket.send_with_str(&text);
        }
    }
}

/// Ouvre le lien vers la console qui a servi cette page, et le rouvre à
/// chaque coupure. `on_signal` est appelé pour tout ce qui arrive : d'abord
/// [`Signal::Open`], puis la vue de la page.
pub fn connect<Command, View, Effect>(
    role: Role,
    mut on_signal: impl FnMut(Signal<View, Effect>) + 'static,
) -> Result<Link<Command>, JsValue>
where
    View: DeserializeOwned + 'static,
    Effect: DeserializeOwned + 'static,
{
    let on_raw = move |raw| match raw {
        Raw::Closed => on_signal(Signal::Closed),
        Raw::Text(text) => match serde_json::from_str::<Wire<View, Effect>>(&text) {
            Ok(Wire::Welcome(welcome)) => on_signal(Signal::Open(welcome)),
            Ok(Wire::View { tick, view }) => on_signal(Signal::View { view, tick }),
            Ok(Wire::Effect(effect)) => on_signal(Signal::Effect(effect)),
            Err(error) => log(&format!("message illisible de la console : {error}")),
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
        command: PhantomData,
    })
}

fn open(inner: &Rc<Inner>) {
    let Ok(socket) = WebSocket::new(&inner.url) else {
        return retry_later(inner);
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
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    *inner.socket.borrow_mut() = Some(socket);
    *inner.handlers.borrow_mut() = Some(Handlers {
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

/// L'adresse du lien : la console qui a servi la page, l'identité de la page et
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

/// L'adresse d'un contenu confié à la console : pour un `<img src>`, un
/// `<audio src>`. Un ticket qui ne mène à rien donne une adresse qui ne
/// répond pas : la page laisse un vide.
pub fn content_url(content: Content) -> String {
    format!("/contenu/{}", content.name())
}

/// Confie des octets à la console. `content_type` dit ce que c'est
/// (`image/webp`). `on_done` reçoit le ticket à mettre dans une commande, ou
/// pourquoi la console n'en a pas voulu.
pub fn upload(
    bytes: &[u8],
    content_type: &str,
    on_done: impl FnOnce(Result<Content, String>) + 'static,
) {
    post(Some(content_type), on_done, |request| {
        request.send_with_opt_u8_array(Some(bytes))
    });
}

/// Confie à la console ce que le navigateur tient déjà : un fichier choisi,
/// le dessin d'un `<canvas>` (`toBlob`). Son type est celui qu'il annonce.
pub fn upload_blob(blob: &Blob, on_done: impl FnOnce(Result<Content, String>) + 'static) {
    post(None, on_done, |request| {
        request.send_with_opt_blob(Some(blob))
    });
}

const UNSENT: &str = "le contenu n'a pas pu partir";

type Done = Box<dyn FnOnce(Result<Content, String>)>;

/// Envoie un contenu à `/contenu` et appelle `on_done` une seule fois : à la
/// fin de l'envoi, réussi ou non, ou tout de suite s'il n'a pas pu partir.
fn post(
    content_type: Option<&str>,
    on_done: impl FnOnce(Result<Content, String>) + 'static,
    send: impl FnOnce(&XmlHttpRequest) -> Result<(), JsValue>,
) {
    let done: Rc<RefCell<Option<Done>>> = Rc::new(RefCell::new(Some(Box::new(on_done))));
    let call = |done: &RefCell<Option<Done>>, outcome| {
        let waiting = done.borrow_mut().take();
        if let Some(waiting) = waiting {
            waiting(outcome);
        }
    };
    let outcome = XmlHttpRequest::new().and_then(|request| {
        request.open("POST", "/contenu")?;
        if let Some(content_type) = content_type {
            request.set_request_header("Content-Type", content_type)?;
        }
        // `loadend` arrive une fois, que l'envoi ait réussi ou non.
        let ended = {
            let done = Rc::clone(&done);
            let request = request.clone();
            Closure::once_into_js(move || call(&done, answer(&request)))
        };
        request.set_onloadend(Some(ended.unchecked_ref()));
        send(&request)
    });
    if outcome.is_err() {
        call(&done, Err(UNSENT.to_owned()));
    }
}

fn answer(request: &XmlHttpRequest) -> Result<Content, String> {
    let text = request.response_text().ok().flatten().unwrap_or_default();
    match request.status() {
        Ok(200) => serde_json::from_str(&text)
            .map_err(|error| format!("réponse illisible de la console : {error}")),
        Ok(0) | Err(_) => Err("la console ne répond pas".to_owned()),
        Ok(_) if text.is_empty() => Err("la console a refusé ce contenu".to_owned()),
        Ok(_) => Err(text),
    }
}

fn log(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(message));
}
