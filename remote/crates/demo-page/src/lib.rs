//! The fake game's free page, written in Rust with `remote-web`: a big
//! button that vibrates and pings the game over UDP, the phone's motion, and
//! a button that turns the page. `cargo xtask page` builds it for
//! `remote-demo` to serve at `/rust`.

pub mod page;

use std::cell::RefCell;

use page::Id;
use remote_web::{Allowed, Datagram, HapticEvent, Listener, Motions, Orientation, Vibration};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};
use web_sys::{EventTarget, HtmlElement};

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
    static LISTENERS: salon_web::ui::Listeners = salon_web::ui::Listeners::default();
    static PAGE: RefCell<Option<Page>> = const { RefCell::new(None) };
}

#[wasm_bindgen(start)]
pub fn start() {
    show(
        page::id::INFO,
        &match remote_web::info() {
            Some(info) => format!(
                "joueur {}…\nprénom « {} »\n{:?}",
                info.player.to_string().get(..8).unwrap_or_default(),
                info.name,
                info.orientation
            ),
            None => "Hors de l'app : ni vibration\nni capteurs ni UDP".into(),
        },
    );

    let motion = remote_web::motion(move |sample| {
        let [x, y, z] = sample.gravity;
        show(
            page::id::MOTION,
            &format!("gravité {x:+.2} {y:+.2} {z:+.2}"),
        );
    });
    let orientation = remote_web::on_orientation(move |turned: Orientation| {
        show(page::id::INFO, &format!("tourné : {turned:?}"));
    });
    let mut datagram = Datagram::open(PORT);
    datagram.on_message(move |bytes| show(page::id::ECHO, &String::from_utf8_lossy(&bytes)));
    PAGE.with_borrow_mut(|page| {
        *page = Some(Page {
            _motion: motion,
            _orientation: orientation,
            datagram,
            taps: 0,
            portrait: false,
        });
    });

    on(page::id::BUZZ, "pointerdown", || {
        // Without the class, the button only looks unpressed.
        if let Ok(buzz) = page::id::BUZZ.get() {
            let _: Result<(), JsValue> = page::class::DOWN.add_to(&buzz);
        }
        remote_web::vibrate(Vibration::Press);
        PAGE.with_borrow_mut(|page| {
            if let Some(page) = page {
                page.taps = page.taps.saturating_add(1);
                page.datagram
                    .send(format!("tap {} depuis Rust", page.taps).as_bytes());
            }
        });
    });
    on(page::id::BUZZ, "pointerup", || {
        if let Ok(buzz) = page::id::BUZZ.get() {
            let _: Result<(), JsValue> = page::class::DOWN.remove_from(&buzz);
        }
        remote_web::haptic(&[
            HapticEvent::tap(0.0, 0.5, 0.9),
            HapticEvent::tap(0.08, 0.3, 0.9),
        ]);
    });
    on(page::id::TURN, "click", || {
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

fn show<T: JsCast + AsRef<HtmlElement>>(id: Id<T>, text: &str) {
    // A missing element: the page shows less, and goes on.
    if let Ok(element) = id.get() {
        AsRef::<HtmlElement>::as_ref(&element).set_inner_text(text);
    }
}

/// Calls `action` on each `event` of the element, for the page's lifetime.
fn on<T: JsCast + AsRef<EventTarget>>(id: Id<T>, event: &str, action: fn()) {
    let Ok(element) = id.get() else {
        return;
    };
    let target: &EventTarget = element.as_ref();
    let _: Result<(), JsValue> = LISTENERS
        .with(|listeners| listeners.listen(target, event, move |_: web_sys::Event| action()));
}
