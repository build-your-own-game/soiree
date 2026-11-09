//! La page de mesure du Wi-Fi, dans le navigateur d'un téléphone (PHASE-3, § 8.5, M5 ;
//! lot 14) : `http://<adresse annoncée>:<port>/mesure`, ouverte par le QR de la télé
//! (`http://localhost:<port>/#mesure`). Ce qu'elle compte est dans [`crate::mesure`] ; ici,
//! le navigateur : la WebSocket de `GET /mesure/ws`, les minuteries, la visibilité, le
//! texte de la page.
//!
//! `?duree=<s>` (le banc) arrête la mesure après ce temps, envoie le dernier bilan
//! (`"fin":true`) et ferme la connexion ; sans lui, la mesure dure tant que la page est
//! ouverte, et se relie au retour du premier plan.

use std::cell::RefCell;
use std::rc::Rc;

use jeu_abi::Oui;
use wasm_bindgen::prelude::*;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, Document, HtmlElement, MessageEvent, WebSocket, Window};

use crate::mesure::{Centiles, Mesure, PERIODE_BILAN_MS, PERIODE_ECHO_MS};

/// Une reconnexion attend 1 s après une coupure, la page visible (choisi).
const RECONNEXION_MS: i32 = 1_000;

struct Etat {
    fenetre: Window,
    mesure: Mesure,
    ws: Option<WebSocket>,
    generation: u32,
    numero: u64,
    texte: HtmlElement,
    fin_ms: Option<f64>,
    finie: bool,
}

type Partage = Rc<RefCell<Etat>>;

fn maintenant_ms(f: &Window) -> f64 {
    f.performance().map_or(0.0, |p| p.now())
}

fn navigateur(f: &Window) -> String {
    f.navigator().user_agent().unwrap_or_default()
}

fn creer(document: &Document, balise: &str) -> Result<HtmlElement, String> {
    document
        .create_element(balise)
        .map_err(|_| format!("créer <{balise}>"))?
        .dyn_into::<HtmlElement>()
        .map_err(|_| String::from("pas un élément HTML"))
}

fn style(e: &HtmlElement, d: &[(&str, &str)]) {
    let s = e.style();
    for (k, v) in d {
        let _: Result<(), JsValue> = s.set_property(k, v);
    }
}

/// `?duree=<s>` : la durée de la mesure, pour le banc.
fn duree_demandee(f: &Window) -> Option<f64> {
    f.location()
        .search()
        .ok()?
        .trim_start_matches('?')
        .split('&')
        .find_map(|kv| kv.strip_prefix("duree="))?
        .parse::<f64>()
        .ok()
        .filter(|d| d.is_finite() && *d > 0.0)
}

/// Le point d'entrée, que `demarrage.js` appelle sur `/mesure`.
#[wasm_bindgen]
pub fn demarrer_la_mesure() {
    if let Err(e) = demarrer() {
        web_sys::console::error_1(&JsValue::from_str(&format!("la mesure : {e}")));
    }
}

fn demarrer() -> Result<(), String> {
    let fenetre = web_sys::window().ok_or("pas de fenêtre")?;
    let document = fenetre.document().ok_or("pas de document")?;
    let corps = document.body().ok_or("pas de corps")?;
    corps.set_text_content(None);
    style(
        &corps,
        &[
            ("margin", "0"),
            ("padding", "16px"),
            ("background", "#111"),
            ("color", "#eee"),
            ("font-family", crate::style::POLICE_DU_COMMUN),
        ],
    );
    let titre = creer(&document, "h1")?;
    titre.set_text_content(Some("Mesure du Wi-Fi"));
    style(&titre, &[("font-size", "24px")]);
    corps
        .append_child(&titre)
        .map_err(|_| String::from("titre"))?;
    let consigne = creer(&document, "p")?;
    consigne.set_text_content(Some(
        "Laisse cette page ouverte pendant la mesure. Tu peux verrouiller le téléphone ou changer d'application, puis revenir : c'est aussi mesuré.",
    ));
    corps
        .append_child(&consigne)
        .map_err(|_| String::from("consigne"))?;
    let texte = creer(&document, "pre")?;
    style(
        &texte,
        &[("white-space", "pre-wrap"), ("font-size", "16px")],
    );
    corps
        .append_child(&texte)
        .map_err(|_| String::from("texte"))?;
    let t0 = maintenant_ms(&fenetre);
    let fin_ms = duree_demandee(&fenetre).map(|d| t0 + d * 1000.0);
    let etat: Partage = Rc::new(RefCell::new(Etat {
        fenetre: fenetre.clone(),
        mesure: Mesure::new(t0),
        ws: None,
        generation: 0,
        numero: 0,
        texte,
        fin_ms,
        finie: false,
    }));
    relier(&etat);
    // Les échos, puis l'affichage et la fin, puis les bilans.
    let e = Rc::clone(&etat);
    let echo = Closure::<dyn FnMut()>::new(move || envoyer_un_echo(&e));
    fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            echo.as_ref().unchecked_ref(),
            i32::try_from(PERIODE_ECHO_MS).unwrap_or(50),
        )
        .map_err(|_| String::from("minuterie des échos"))?;
    echo.forget();
    let e = Rc::clone(&etat);
    let seconde = Closure::<dyn FnMut()>::new(move || chaque_seconde(&e));
    fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            seconde.as_ref().unchecked_ref(),
            1_000,
        )
        .map_err(|_| String::from("minuterie de l'affichage"))?;
    seconde.forget();
    let e = Rc::clone(&etat);
    let bilan = Closure::<dyn FnMut()>::new(move || envoyer_le_bilan(&e, Oui::Non));
    fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            bilan.as_ref().unchecked_ref(),
            i32::try_from(PERIODE_BILAN_MS).unwrap_or(10_000),
        )
        .map_err(|_| String::from("minuterie des bilans"))?;
    bilan.forget();
    // La visibilité : caché, puis revenu (et relié de nouveau si la connexion est tombée).
    let e = Rc::clone(&etat);
    let d = document.clone();
    let visible = Closure::<dyn FnMut()>::new(move || {
        let relancer = {
            let Ok(mut t) = e.try_borrow_mut() else {
                return;
            };
            let now = maintenant_ms(&t.fenetre);
            if d.hidden() {
                t.mesure.cachee(now);
                false
            } else {
                t.mesure.visible(now);
                !t.finie && t.ws.is_none()
            }
        };
        if relancer {
            relier(&e);
        }
    });
    document
        .add_event_listener_with_callback("visibilitychange", visible.as_ref().unchecked_ref())
        .map_err(|_| String::from("visibilitychange"))?;
    visible.forget();
    Ok(())
}

/// Une WebSocket neuve vers `GET /mesure/ws`.
fn relier(etat: &Partage) {
    let Ok(mut t) = etat.try_borrow_mut() else {
        return;
    };
    if t.finie {
        return;
    }
    t.generation = t.generation.wrapping_add(1);
    let generation = t.generation;
    let hote = t.fenetre.location().host().unwrap_or_default();
    let Ok(ws) = WebSocket::new(&format!("ws://{hote}/mesure/ws")) else {
        return;
    };
    ws.set_binary_type(BinaryType::Arraybuffer);
    let e = Rc::clone(etat);
    let ouvert = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        if let Ok(mut t) = e.try_borrow_mut()
            && t.generation == generation
        {
            t.mesure.ouverte();
        }
    });
    ws.set_onopen(Some(ouvert.as_ref().unchecked_ref()));
    ouvert.forget();
    let e = Rc::clone(etat);
    let recu = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
        let Ok(mut t) = e.try_borrow_mut() else {
            return;
        };
        if t.generation != generation {
            return;
        }
        let Ok(tampon) = m.data().dyn_into::<js_sys::ArrayBuffer>() else {
            return;
        };
        let now = maintenant_ms(&t.fenetre);
        let o = js_sys::Uint8Array::new(&tampon).to_vec();
        match o.len() {
            relais::TIC => t.mesure.tic(now),
            relais::REPONSE => {
                let envoi = o
                    .get(8..16)
                    .and_then(|b| <[u8; 8]>::try_from(b).ok())
                    .map(f64::from_le_bytes);
                let heure = o
                    .get(16..24)
                    .and_then(|b| <[u8; 8]>::try_from(b).ok())
                    .map(u64::from_le_bytes);
                if let (Some(envoi), Some(heure)) = (envoi, heure) {
                    t.mesure.echo_recu(envoi, now, heure);
                }
            }
            _ => {}
        }
    });
    ws.set_onmessage(Some(recu.as_ref().unchecked_ref()));
    recu.forget();
    let e = Rc::clone(etat);
    let ferme = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        let relancer = {
            let Ok(mut t) = e.try_borrow_mut() else {
                return;
            };
            if t.generation != generation {
                return;
            }
            let now = maintenant_ms(&t.fenetre);
            t.mesure.fermee(now);
            t.ws = None;
            let cachee = t.fenetre.document().is_some_and(|d| d.hidden());
            !t.finie && !cachee
        };
        if relancer {
            let e2 = Rc::clone(&e);
            let plus_tard = Closure::once_into_js(move || relier(&e2));
            if let Ok(t) = e.try_borrow() {
                let _: Result<i32, JsValue> = t
                    .fenetre
                    .set_timeout_with_callback_and_timeout_and_arguments_0(
                        plus_tard.unchecked_ref(),
                        RECONNEXION_MS,
                    );
            }
        }
    });
    ws.set_onclose(Some(ferme.as_ref().unchecked_ref()));
    ferme.forget();
    t.ws = Some(ws);
}

/// Les tailles des trames du relais (`relais::mesure`), que la page ne peut pas importer
/// (le relais n'est pas dans `web/`) : un test natif de `crate::mesure` ne les tient pas,
/// le scénario `mesure-wifi` du banc, si.
mod relais {
    /// Un tic : l'heure du relais.
    pub(super) const TIC: usize = 8;
    /// La réponse à un écho : ses 16 octets, puis l'heure du relais.
    pub(super) const REPONSE: usize = 24;
}

fn envoyer_un_echo(etat: &Partage) {
    let Ok(mut t) = etat.try_borrow_mut() else {
        return;
    };
    let Some(ws) = t.ws.clone() else {
        return;
    };
    if ws.ready_state() != WebSocket::OPEN || t.finie {
        return;
    }
    t.numero = t.numero.wrapping_add(1);
    let mut o = [0u8; 16];
    if let Some(b) = o.get_mut(..8) {
        b.copy_from_slice(&t.numero.to_le_bytes());
    }
    let now = maintenant_ms(&t.fenetre);
    if let Some(b) = o.get_mut(8..) {
        b.copy_from_slice(&now.to_le_bytes());
    }
    if ws.send_with_u8_array(&o).is_ok() {
        t.mesure.echo_envoye();
    }
}

fn envoyer_le_bilan(etat: &Partage, fin: Oui) {
    let Ok(t) = etat.try_borrow() else {
        return;
    };
    let Some(ws) = t.ws.as_ref() else {
        return;
    };
    if ws.ready_state() != WebSocket::OPEN {
        return;
    }
    let now = maintenant_ms(&t.fenetre);
    let b = t.mesure.bilan(&navigateur(&t.fenetre), now, fin);
    let _: Result<(), JsValue> = ws.send_with_str(&b);
}

fn ligne(nom: &str, c: Option<&Centiles>) -> String {
    c.map_or_else(
        || format!("{nom} : rien encore"),
        |c| {
            format!(
                "{nom} : médiane {:.0} ms, 99 % sous {:.0} ms, au pire {:.0} ms ({} mesures)",
                c.p50, c.p99, c.max, c.n
            )
        },
    )
}

/// L'affichage, et la fin d'une mesure bornée par `?duree=`.
fn chaque_seconde(etat: &Partage) {
    let finir = {
        let Ok(t) = etat.try_borrow() else {
            return;
        };
        let now = maintenant_ms(&t.fenetre);
        !t.finie && t.fin_ms.is_some_and(|f| now >= f)
    };
    if finir {
        envoyer_le_bilan(etat, Oui::Oui);
        if let Ok(mut t) = etat.try_borrow_mut() {
            t.finie = true;
            if let Some(ws) = t.ws.take() {
                let _: Result<(), JsValue> = ws.close();
            }
        }
    }
    let Ok(t) = etat.try_borrow() else {
        return;
    };
    let (envoyes, recus, perdus) = t.mesure.echos();
    let etat_du_lien = if t.finie {
        "mesure finie : merci"
    } else if t
        .ws
        .as_ref()
        .is_some_and(|w| w.ready_state() == WebSocket::OPEN)
    {
        "relié"
    } else {
        "pas relié"
    };
    let texte = [
        format!("{etat_du_lien} ; échos {recus} reçus sur {envoyes}, {perdus} perdus"),
        ligne("aller-retour", t.mesure.aller_retour().as_ref()),
        ligne("entre deux tics", t.mesure.ecarts_tics().as_ref()),
        t.mesure.derive().map_or_else(
            || String::from("horloge : pas encore assez d'échos"),
            |d| format!("horloge : {d:.0} millionièmes d'écart avec la soirée"),
        ),
    ]
    .join("\n");
    t.texte.set_text_content(Some(&texte));
}
