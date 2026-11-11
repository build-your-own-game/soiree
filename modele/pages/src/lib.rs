//! Les pages du jeu : l'écran à `/`, la télécommande à `/t`.
#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use messages::{Down, Up};
use telecommande::Role;
use telecommande::web::{Signal, connect};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn start() {
    leptos::mount::mount_to_body(App);
}

#[component]
fn App() -> impl IntoView {
    let is_remote = window()
        .location()
        .pathname()
        .is_ok_and(|path| path.starts_with("/t"));
    let role = if is_remote {
        Role::Remote
    } else {
        Role::Screen
    };
    let (down, set_down) = signal(None::<Down>);
    let (connected, set_connected) = signal(false);
    let link = connect::<Up, Down>(role, move |signal| match signal {
        Signal::Open => set_connected.set(true),
        Signal::Message(message) => set_down.set(Some(message)),
        Signal::Closed => set_connected.set(false),
    });
    // Le lien vit dans la page : on le range là où les boutons le retrouvent.
    let link = StoredValue::new_local(link.ok());
    let send = move |up: Up| {
        link.with_value(|link| {
            if let Some(link) = link {
                link.send(&up);
            }
        });
    };
    let status = move || {
        if connected.get() { "" } else { "Connexion…" }
    };

    if is_remote {
        let taps = move || match down.get() {
            Some(Down::Remote(remote)) => remote.taps,
            Some(Down::Screen(_)) | None => 0,
        };
        view! {
            <main class="remote">
                <input
                    type="text"
                    placeholder="Ton prénom"
                    maxlength="16"
                    on:change=move |event| send(Up::Name(event_target_value(&event)))
                />
                <button on:pointerdown=move |_| send(Up::Tap)>"Tape !"</button>
                <p class="taps">{taps} " taps"</p>
                <p class="status">{status}</p>
            </main>
        }
        .into_any()
    } else {
        let players = move || match down.get() {
            Some(Down::Screen(screen)) => screen.players,
            Some(Down::Remote(_)) | None => Vec::new(),
        };
        let join_url = move || match down.get() {
            Some(Down::Screen(screen)) => screen.join_url,
            Some(Down::Remote(_)) | None => String::new(),
        };
        view! {
            <main class="screen">
                <h1>"Mon jeu"</h1>
                <div class="join">
                    <img class="qr" src="/qr.svg" alt="QR code pour rejoindre" />
                    <span>{join_url}</span>
                </div>
                <ul>
                    {move || {
                        players()
                            .into_iter()
                            .map(|player| {
                                view! {
                                    <li class:away=!player.here>
                                        {player.name} " : " {player.taps}
                                    </li>
                                }
                            })
                            .collect_view()
                    }}
                </ul>
                <p class="status">{status}</p>
            </main>
        }
        .into_any()
    }
}
