//! The demo's page: the TV at `/`, a phone at `/play`.

#[path = "../../demo/src/protocol.rs"]
mod protocol;

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use protocol::{Up, VERSION, View};
use salon_core::{Kind, Refusal, Status};
use salon_web::{Connection, Happening, Options};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

struct Page {
    tv: bool,
    status: Status,
    view: Option<View>,
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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

fn render(page: &Page) -> String {
    let mut html = format!(
        "<div class=\"{}\"><div class=status>{}</div>",
        if page.tv { "tv" } else { "phone" },
        status_text(page.status)
    );
    let Some(view) = &page.view else {
        html.push_str("</div>");
        return html;
    };
    if page.tv {
        let _ = write!(html, "<h1>{} taps</h1>", view.taps);
        if let Some((url, qr)) = &view.join {
            let _ = write!(html, "{qr}<p>{}</p>", escape(url));
        }
    } else {
        let _ = write!(
            html,
            "<button id=tap>Tap</button><p>Toi : {} · Tous : {}</p>",
            view.mine.unwrap_or(0),
            view.taps
        );
    }
    html.push_str("<ul>");
    // The TV lists who is here; a phone also sees who left.
    for player in view.players.iter().filter(|player| player.here || !page.tv) {
        let rtt = player
            .rtt
            .map(|rtt| format!(" · {rtt} ms"))
            .unwrap_or_default();
        let _ = write!(
            html,
            "<li class={}>{} : {}{rtt}</li>",
            if player.here { "here" } else { "away" },
            escape(&player.name),
            player.taps
        );
    }
    html.push_str("</ul></div>");
    html
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let app = document.get_element_by_id("app").ok_or("no #app")?;
    let path = window.location().pathname()?;
    let tv = path == "/";
    let page = Rc::new(RefCell::new(Page {
        tv,
        status: Status::Connecting,
        view: None,
    }));
    let shown = Rc::clone(&page);
    let target = app.clone();
    let connection: Connection<Up, View> = Connection::open(
        Options::for_page(VERSION, if tv { Kind::Tv } else { Kind::Phone }),
        move |happening| {
            let mut page = shown.borrow_mut();
            match happening {
                Happening::Status(status) => page.status = status,
                Happening::State(view) => page.view = Some(view),
                Happening::Reset => page.view = None,
                Happening::Event(_) => {}
            }
            target.set_inner_html(&render(&page));
        },
    );
    app.set_inner_html(&render(&page.borrow()));
    if !tv {
        let tap = Closure::<dyn FnMut(web_sys::Event)>::new(move |event: web_sys::Event| {
            let on_button = event
                .target()
                .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                .is_some_and(|element| element.id() == "tap");
            if on_button {
                let _ = connection.send(&Up::Tap);
            }
        });
        app.add_event_listener_with_callback("click", tap.as_ref().unchecked_ref())?;
        tap.forget();
    }
    Ok(())
}
