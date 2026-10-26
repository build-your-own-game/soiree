//! The fake game's free page, written in Rust with `remote-web`: a big
//! button that vibrates and pings the game over UDP, the phone's motion, and
//! a button that turns the page. `cargo xtask page` builds it for
//! `remote-demo` to serve at `/rust`.

use std::cell::RefCell;

use remote_web::{Allowed, Datagram, HapticEvent, Listener, Motions, Orientation, Vibration};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{Closure, wasm_bindgen};
use web_sys::{Document, HtmlElement};

/// The game's UDP port: the same as its web page.
const PORT: u16 = 8095;

/// What must live as long as the page.
struct Page {
    _motion: Motions,
    _orientation: Listener,
    datagram: Datagram,
    taps: u32,
    portrait: bool,
}

thread_local! {
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

#[wasm_bindgen(start)]
pub fn start() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    show(
        &document,
        "info",
        &match remote_web::info() {
            Some(info) => format!(
                "joueur {}…\nprénom « {} »\n{:?}",
                info.player.get(..8).unwrap_or_default(),
                info.name,
                info.orientation
            ),
            None => "Hors de l'app : ni vibration\nni capteurs ni UDP".into(),
        },
    );

    let motion_document = document.clone();
    let motion = remote_web::motion(move |sample| {
        let [x, y, z] = sample.gravity;
        show(
            &motion_document,
            "motion",
            &format!("gravité {x:+.2} {y:+.2} {z:+.2}"),
        );
    });
    let orientation_document = document.clone();
    let orientation = remote_web::on_orientation(move |turned: Orientation| {
        show(
            &orientation_document,
            "info",
            &format!("tourné : {turned:?}"),
        );
    });
    let mut datagram = Datagram::open(PORT);
    let echo_document = document.clone();
    datagram
        .on_message(move |bytes| show(&echo_document, "echo", &String::from_utf8_lossy(&bytes)));
    PAGE.with_borrow_mut(|page| {
        *page = Some(Page {
            _motion: motion,
            _orientation: orientation,
            datagram,
            taps: 0,
            portrait: false,
        });
    });

    on(&document, "buzz", "pointerdown", |document| {
        set_class(document, "buzz", "down");
        remote_web::vibrate(Vibration::Press);
        PAGE.with_borrow_mut(|page| {
            if let Some(page) = page {
                page.taps += 1;
                page.datagram
                    .send(format!("tap {} depuis Rust", page.taps).as_bytes());
            }
        });
    });
    on(&document, "buzz", "pointerup", |document| {
        set_class(document, "buzz", "");
        remote_web::haptic(&[
            HapticEvent::tap(0.0, 0.5, 0.9),
            HapticEvent::tap(0.08, 0.3, 0.9),
        ]);
    });
    on(&document, "turn", "click", |_| {
        let portrait = PAGE.with_borrow_mut(|page| {
            page.as_mut().map(|p| {
                p.portrait = !p.portrait;
                p.portrait
            })
        });
        remote_web::set_orientation(if portrait == Some(true) {
            Allowed::Portrait
        } else {
            Allowed::Landscape
        });
    });
}

fn element(document: &Document, id: &str) -> Option<HtmlElement> {
    document.get_element_by_id(id)?.dyn_into().ok()
}

fn show(document: &Document, id: &str, text: &str) {
    if let Some(element) = element(document, id) {
        element.set_inner_text(text);
    }
}

fn set_class(document: &Document, id: &str, class: &str) {
    if let Some(element) = element(document, id) {
        element.set_class_name(class);
    }
}

/// Calls `action` on each `event` of the element, for the page's lifetime.
fn on(document: &Document, id: &str, event: &str, action: fn(&Document)) {
    let Some(element) = element(document, id) else {
        return;
    };
    let target = document.clone();
    let closure = Closure::<dyn FnMut()>::new(move || action(&target));
    let _ = element.add_event_listener_with_callback(event, closure.as_ref().unchecked_ref());
    // The page never removes its buttons: the listener lives as long as it.
    closure.forget();
}
