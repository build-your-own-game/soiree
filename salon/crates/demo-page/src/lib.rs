//! The demo's page: the TV at `/`, a phone at `/play`.

pub mod page;

use std::cell::RefCell;
use std::rc::Rc;

use salon_core::{Kind, Refusal, Status};
use salon_demo::protocol::{Up, VERSION, View};
use salon_web::{Connection, Happening, Options};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{Element, Node};

thread_local! {
    static RUNTIME: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

struct Runtime {
    _page: Rc<RefCell<Page>>,
    _connection: Connection<Up, View>,
    _listeners: salon_web::ui::Listeners,
}

struct Page {
    tv: bool,
    status: Status,
    view: Option<View>,
}

fn status_text(status: Status) -> &'static str {
    match status {
        Status::Refused(Refusal::Full | Refusal::Closed) => "La partie est pleine.",
        Status::Refused(Refusal::Version) => "Cette page n'est plus à jour : recharge-la.",
        Status::Refused(Refusal::NotAllowed) => "Seul l'ordinateur du jeu peut être la TV.",
        Status::Refused(Refusal::Removed) => "Le jeu t'a retiré de la partie.",
        Status::Connecting => "Connexion…",
        Status::Connected => "Connecté",
        Status::Reconnecting => "Reconnexion…",
        Status::HostGone => "L'hôte redémarre…",
        Status::Replaced => "Ouvert ailleurs",
        Status::Quit => "Parti",
    }
}

/// A new element of `tag` holding `text`.
fn text<T: JsCast + AsRef<Node>>(tag: page::Tag<T>, text: &str) -> Result<T, JsValue> {
    let element = tag.create()?;
    AsRef::<Node>::as_ref(&element).set_text_content(Some(text));
    Ok(element)
}

/// What `page` shows, in place of what `app` held.
fn render(page: &Page, app: &Element) -> Result<(), JsValue> {
    let root = page::tag::DIV.create()?;
    page::class::TV.set(&root, page.tv)?;
    let status = text(page::tag::DIV, status_text(page.status))?;
    page::class::STATUS.add_to(&status)?;
    root.append_child(&status)?;
    if let Some(view) = &page.view {
        if page.tv {
            let taps = text(page::tag::H1, &format!("{} taps", view.taps))?;
            root.append_child(&taps)?;
            if let Some(join) = &view.join {
                let code = salon_web::qr(&join.qr)?;
                root.append_child(&code)?;
                let url = text(page::tag::P, &join.url)?;
                root.append_child(&url)?;
            }
        } else {
            let tap = text(page::tag::BUTTON, "Tap")?;
            page::id::TAP.give(&tap);
            root.append_child(&tap)?;
            let counts = format!("Toi : {} · Tous : {}", view.mine.unwrap_or(0), view.taps);
            let counts = text(page::tag::P, &counts)?;
            root.append_child(&counts)?;
        }
        let list = page::tag::UL.create()?;
        // The TV lists who is here; a phone also sees who left.
        for player in view.players.iter().filter(|player| player.here || !page.tv) {
            let rtt = player
                .rtt
                .map(|rtt| format!(" · {rtt} ms"))
                .unwrap_or_default();
            let item = text(
                page::tag::LI,
                &format!("{} : {}{rtt}", player.name, player.taps),
            )?;
            page::class::AWAY.set(&item, !player.here)?;
            list.append_child(&item)?;
        }
        root.append_child(&list)?;
    }
    app.replace_children_with_node_1(&root);
    Ok(())
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let app = page::id::APP.get()?;
    let path = window.location().pathname()?;
    let tv = path == "/";
    let page = Rc::new(RefCell::new(Page {
        tv,
        status: Status::Connecting,
        view: None,
    }));
    let shown = Rc::downgrade(&page);
    let target = app.clone();
    let connection: Connection<Up, View> = Connection::open(
        Options::for_page(VERSION, if tv { Kind::Tv } else { Kind::Phone }),
        move |happening| {
            let Some(shown) = shown.upgrade() else {
                return;
            };
            let mut page = shown.borrow_mut();
            match happening {
                Happening::Status(status) => page.status = status,
                Happening::State(view) => page.view = Some(view),
                Happening::Reset => page.view = None,
                Happening::Event(_) => {}
            }
            // A page that cannot draw has nothing better to do: it waits
            // for the next state.
            let _: Result<(), JsValue> = render(&page, &target);
        },
    );
    render(&page.borrow(), &app)?;
    let listeners = salon_web::ui::Listeners::default();
    if !tv {
        let link = connection.clone();
        listeners.listen(&app, "click", move |event: web_sys::Event| {
            let on_button = event
                .target()
                .and_then(|target| target.dyn_into::<Element>().ok())
                .is_some_and(|element| page::id::TAP.is(&element));
            if on_button {
                // Not connected: the tap is lost, like on a remote.
                let _: Result<(), salon_core::ClientSendError> = link.send(&Up::Tap);
            }
        })?;
    }
    RUNTIME.with(|runtime| {
        runtime.replace(Some(Runtime {
            _page: page,
            _connection: connection,
            _listeners: listeners,
        }))
    });
    Ok(())
}
