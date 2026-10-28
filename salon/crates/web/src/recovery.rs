//! Recovery uses browser-native beacon and reload callbacks: no Rust
//! callback has to run after the WASM instance has trapped.

use js_sys::{Function, Reflect};
use salon_core::journal::{Diagnostic, PanicReport};
use serde::{Deserialize, Serialize};
use std::cell::Cell;
use wasm_bindgen::prelude::*;

const KEY: &str = "salon.panic-recovery.v1";
const WINDOW: u64 = 60_000;
const LIMIT: u8 = 3;
thread_local! { static INSTALLED: Cell<bool> = const { Cell::new(false) };
static TV_INSTALLED: Cell<bool> = const { Cell::new(false) };
static LISTENERS: crate::ui::Listeners = crate::ui::Listeners::default(); }

#[derive(Default, Serialize, Deserialize)]
struct Attempts {
    since: u64,
    count: u8,
}
impl Attempts {
    fn add(self, now: u64) -> Self {
        if now.saturating_sub(self.since) >= WINDOW || now < self.since {
            Self {
                since: now,
                count: 1,
            }
        } else {
            Self {
                since: self.since,
                count: self.count.saturating_add(1),
            }
        }
    }
}

/// Safe to call more than once, including before opening a connection.
/// Solo pages use this too. Three recoveries within a minute are allowed;
/// the fourth leaves a clear error instead of a reload loop.
pub fn install_panic_hook() {
    if INSTALLED.with(|installed| installed.replace(true)) {
        return;
    }
    std::panic::set_hook(Box::new(|info| recover(info.to_string())));
    install_test_trigger();
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Date.now is finite epoch milliseconds, below u64 for the lifetime of this app"
)]
fn wall_ms() -> u64 {
    js_sys::Date::now().max(0.0) as u64
}

#[expect(
    clippy::disallowed_methods,
    reason = "panic recovery must invoke a native JS reload after the Rust runtime has trapped"
)]
fn recover(message: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let storage = window.session_storage().ok().flatten();
    let previous = storage
        .as_ref()
        .and_then(|storage| storage.get_item(KEY).ok().flatten())
        .and_then(|text| serde_json::from_str::<Attempts>(&text).ok())
        .unwrap_or_default();
    let attempts = previous.add(wall_ms());
    // Without durable loop accounting automatic reload is unsafe: stop.
    let remembered = storage.as_ref().is_some_and(|storage| {
        serde_json::to_string(&attempts).is_ok_and(|text| storage.set_item(KEY, &text).is_ok())
    });
    let report = PanicReport {
        message,
        page: window.location().pathname().unwrap_or_default(),
        at: crate::now(),
        attempt: attempts.count,
    };
    report_diagnostic(&Diagnostic::Panic(report));
    let reload = remembered && attempts.count <= LIMIT;
    if let Some(body) = window.document().and_then(|document| document.body()) {
        body.set_text_content(Some(if reload {
            "Petit souci, on recharge…"
        } else {
            "Le jeu rencontre un problème répété. Relancez-le depuis l’ordinateur de l’hôte."
        }));
    }
    if reload {
        let location: JsValue = window.location().into();
        if let Ok(reload) = Reflect::get(&location, &JsValue::from_str("reload"))
            .and_then(wasm_bindgen::JsCast::dyn_into::<Function>)
        {
            let reload = reload.bind0(&location);
            let _: Result<i32, JsValue> =
                window.set_timeout_with_callback_and_timeout_and_arguments_0(&reload, 250);
        }
    }
}

fn report_diagnostic(report: &Diagnostic) {
    if let Some(window) = web_sys::window()
        && let Ok(text) = serde_json::to_string(report)
    {
        let _: Result<bool, JsValue> = window
            .navigator()
            .send_beacon_with_opt_str(salon_core::route::DIAGNOSTIC, Some(&text));
    }
}

/// Installs the shared F8 bug marker on a TV, including solo games.
pub fn install_tv_diagnostics() {
    install_panic_hook();
    if TV_INSTALLED.with(|installed| installed.replace(true)) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let _: Result<(), JsValue> = LISTENERS.with(|listeners| {
        listeners.listen::<web_sys::KeyboardEvent>(&window, "keydown", |event| {
            if event.key() == "F8" && !event.repeat() {
                event.prevent_default();
                let page = web_sys::window()
                    .and_then(|window| window.location().pathname().ok())
                    .unwrap_or_default();
                report_diagnostic(&Diagnostic::Marker {
                    at: crate::now(),
                    page,
                    note: "Bug marqué depuis la TV (F8)".to_owned(),
                });
            }
        })
    });
}

/// The harness must exercise an actual WASM panic. Registering the trigger
/// requires its explicit URL flag; dispatch alone on a normal page does
/// nothing. Merely reloading with the flag never triggers another panic.
#[expect(
    clippy::panic,
    reason = "explicit certification fault injection exercises the real WASM panic hook"
)]
fn install_test_trigger() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let search = window.location().search().unwrap_or_default();
    let Some(nonce) = test_nonce(&search).map(str::to_owned) else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let _: Result<(), JsValue> = LISTENERS.with(|listeners| {
        listeners.listen::<web_sys::Event>(&document, "salon:test-panic", move |_| {
            panic!("salon certification panic {nonce}")
        })
    });
}

fn test_nonce(search: &str) -> Option<&str> {
    let pairs = || search.trim_start_matches('?').split('&');
    if !pairs().any(|value| value == "salon-test-panic=1") {
        return None;
    }
    let nonce = pairs()
        .find_map(|value| value.strip_prefix("salon-test-id="))
        .unwrap_or("manual");
    (!nonce.is_empty()
        && nonce.len() <= 64
        && nonce.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
    .then_some(nonce)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fault_injection_nonce_is_explicit_and_safe_for_one_line_correlation() {
        assert_eq!(
            test_nonce("?salon-test-panic=1&salon-test-id=page-123"),
            Some("page-123")
        );
        assert_eq!(test_nonce("?salon-test-id=page-123"), None);
        assert_eq!(
            test_nonce("?salon-test-panic=1&salon-test-id=evil%0Aline"),
            None
        );
        assert_eq!(test_nonce("?salon-test-panic=1&salon-test-id="), None);
        assert_eq!(test_nonce("?salon-test-panic=1"), Some("manual"));
        assert!(
            test_nonce(&format!(
                "?salon-test-panic=1&salon-test-id={}",
                "x".repeat(65)
            ))
            .is_none()
        );
    }
    #[test]
    fn reload_guard_stops_a_storm_and_resets_after_quiet() {
        let mut attempts = Attempts::default();
        for count in 1..=4 {
            attempts = attempts.add(1_000);
            assert_eq!(attempts.count, count);
        }
        assert!(attempts.count > LIMIT);
        attempts = attempts.add(61_000);
        assert_eq!(attempts.count, 1);
        assert_eq!(attempts.since, 61_000);
        attempts = attempts.add(50);
        assert_eq!(attempts.count, 1);
    }
}
