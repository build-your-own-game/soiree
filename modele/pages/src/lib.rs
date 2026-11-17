//! Les pages du jeu : l'écran à `/`, la télécommande à `/t`.
//!
//! Une page reçoit des vues et des effets, envoie des commandes. Elle ne
//! garde aucune vérité : rechargée, elle retrouve tout dans la vue suivante.
#![cfg(target_arch = "wasm32")]

use leptos::prelude::*;
use messages::{Effect, Up, View};
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
    let (view, set_view) = signal(None::<View>);
    let (connected, set_connected) = signal(false);
    // L'adresse des téléphones : la console la donne à la connexion.
    let (join_url, set_join_url) = signal(String::new());
    // Le dernier joueur fêté : un effet, joué une fois.
    let (cheer, set_cheer) = signal(None::<String>);
    let link = connect::<Up, View, Effect>(role, move |signal| match signal {
        Signal::Open(welcome) => {
            set_connected.set(true);
            set_join_url.set(welcome.join_url);
        }
        Signal::View { view, tick: _ } => set_view.set(Some(view)),
        Signal::Effect(Effect::Cheer { name, taps }) => {
            set_cheer.set(Some(format!("{name} : {taps} !")));
        }
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
        let taps = move || match view.get() {
            Some(View::Remote(remote)) => remote.taps,
            Some(View::Screen(_)) | None => 0,
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
        let players = move || match view.get() {
            Some(View::Screen(screen)) => screen.players,
            Some(View::Remote(_)) | None => Vec::new(),
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
                <p class="cheer">{cheer}</p>
                <p class="status">{status}</p>
            </main>
        }
        .into_any()
    }
}
