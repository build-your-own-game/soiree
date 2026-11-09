//! La télé en `http://localhost:<port>/#mesure` (PHASE-3, § 8.5, M5 ; lot 14) : la mesure
//! du vrai Wi-Fi. Ni Worker, ni partie, ni rendu GPU : un QR vers la page de mesure des
//! téléphones (`http://<adresse annoncée>:<port>/mesure`), et le dernier bilan de chaque
//! téléphone, relus au relais toutes les deux secondes (`GET /mesure/bilans`, boucle
//! locale, `relais::mesure`). L'hôte lit ici, pendant la répétition, ce que le Wi-Fi fait
//! aux téléphones ; le fichier `<garde>/mesure-wifi.jsonl` du relais le garde.
//!
//! Le texte se pose par `textContent` (REM-56) : un bilan vient d'un téléphone, il n'est
//! jamais du HTML.

use std::cell::RefCell;
use std::rc::Rc;

use jeu_abi::Oui;
use js_sys::Reflect;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Element, HtmlElement, Window};

use super::style;
use crate::toile::texte_d_erreur;

/// Les bilans se relisent toutes les deux secondes (choisi, comme l'aide).
const RELIRE_MS: i32 = 2_000;

/// La taille d'un module du QR, en pixels CSS (choisi : un QR de 33 modules fait 264 px).
const MODULE_PX: u32 = 8;

struct Vue {
    fenetre: Window,
    document: Document,
    qr: Element,
    adresse: Element,
    table: Element,
    page: Option<String>,
    banc: Oui,
    rapporte: String,
}

fn champ(v: &JsValue, nom: &str) -> JsValue {
    Reflect::get(v, &JsValue::from_str(nom)).unwrap_or(JsValue::UNDEFINED)
}

fn nombre(v: &JsValue) -> String {
    v.as_f64()
        .map_or_else(|| String::from("—"), |x| format!("{x:.0}"))
}

/// Un élément, son texte, dans un parent.
fn poser(
    document: &Document,
    parent: &Element,
    balise: &str,
    texte: &str,
) -> Result<Element, String> {
    let e = document
        .create_element(balise)
        .map_err(|e| texte_d_erreur(&e))?;
    e.set_text_content(Some(texte));
    parent.append_child(&e).map_err(|e| texte_d_erreur(&e))?;
    Ok(e)
}

/// La vue `/#mesure`, à la place de la télé de la soirée.
pub(super) fn demarrer(
    fenetre: &Window,
    document: &Document,
    corps: &HtmlElement,
    banc: Oui,
) -> Result<(), String> {
    style(
        corps,
        &[
            ("overflow", "auto"),
            ("padding", "24px"),
            ("font-size", "20px"),
        ],
    );
    let racine: Element = corps.clone().into();
    poser(document, &racine, "h1", "Mesure du Wi-Fi")?;
    poser(
        document,
        &racine,
        "p",
        "Chaque téléphone scanne ce QR et laisse la page ouverte dix minutes ou plus, en jouant comme en soirée : verrouiller, changer d'application, revenir. Les bilans arrivent ici toutes les dix secondes, et dans mesure-wifi.jsonl, à côté de la garde du relais.",
    )?;
    let qr = poser(document, &racine, "div", "")?;
    let adresse = poser(document, &racine, "p", "")?;
    let table = poser(document, &racine, "div", "")?;
    if let Ok(h) = table.clone().dyn_into::<HtmlElement>() {
        style(
            &h,
            &[
                ("white-space", "pre"),
                ("font-family", "monospace"),
                ("font-size", "16px"),
            ],
        );
    }
    let vue = Rc::new(RefCell::new(Vue {
        fenetre: fenetre.clone(),
        document: document.clone(),
        qr,
        adresse,
        table,
        page: None,
        banc,
        rapporte: String::new(),
    }));
    relire(&vue);
    let v = Rc::clone(&vue);
    let tic = Closure::<dyn FnMut()>::new(move || relire(&v));
    fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            tic.as_ref().unchecked_ref(),
            RELIRE_MS,
        )
        .map_err(|e| texte_d_erreur(&e))?;
    tic.forget();
    Ok(())
}

/// `GET /mesure/bilans`, puis la vue refaite.
fn relire(vue: &Rc<RefCell<Vue>>) {
    let v = Rc::clone(vue);
    spawn_local(async move {
        let fenetre = v.borrow().fenetre.clone();
        let Ok(r) = JsFuture::from(fenetre.fetch_with_str("/mesure/bilans")).await else {
            return;
        };
        let Ok(r) = r.dyn_into::<web_sys::Response>() else {
            return;
        };
        let Ok(t) = r.text() else {
            return;
        };
        let Some(texte) = JsFuture::from(t).await.ok().and_then(|t| t.as_string()) else {
            return;
        };
        let Ok(json) = js_sys::JSON::parse(&texte) else {
            return;
        };
        montrer(&mut v.borrow_mut(), &json);
    });
}

/// Le QR des modules, en carrés du DOM.
fn dessiner_le_qr(vue: &Vue, page: &str) {
    vue.qr.set_text_content(None);
    let Some((n, m)) = rendu::qr::modules(page) else {
        return;
    };
    let marge = rendu::qr::MARGE;
    let cote = n.saturating_add(marge.saturating_mul(2));
    if let Ok(h) = vue.qr.clone().dyn_into::<HtmlElement>() {
        style(
            &h,
            &[
                ("display", "grid"),
                (
                    "grid-template-columns",
                    &format!("repeat({cote}, {MODULE_PX}px)"),
                ),
                ("width", "max-content"),
                ("background", "#fff"),
            ],
        );
    }
    for y in 0..cote {
        for x in 0..cote {
            let (mx, my) = (x.saturating_sub(marge), y.saturating_sub(marge));
            let dedans = x >= marge && y >= marge && mx < n && my < n;
            let noir = dedans
                && usize::try_from(my.saturating_mul(n).saturating_add(mx))
                    .ok()
                    .and_then(|k| m.get(k).copied())
                    .unwrap_or(false);
            let Ok(c) = vue.document.create_element("div") else {
                return;
            };
            if let Ok(h) = c.clone().dyn_into::<HtmlElement>() {
                style(
                    &h,
                    &[
                        ("width", &format!("{MODULE_PX}px")),
                        ("height", &format!("{MODULE_PX}px")),
                        ("background", if noir { "#000" } else { "#fff" }),
                    ],
                );
            }
            let _: Result<web_sys::Node, JsValue> = vue.qr.append_child(&c);
        }
    }
}

/// Une ligne de la table pour un bilan (un objet JSON que le téléphone a écrit).
fn ligne_d_un_bilan(lien: &JsValue, bilan: &JsValue) -> String {
    let ar = champ(bilan, "aller_retour_ms");
    let tics = champ(bilan, "tics");
    let ecart = champ(&tics, "ecart_ms");
    let echos = champ(bilan, "echos");
    let ap = champ(bilan, "arriere_plan");
    let navigateur = champ(bilan, "navigateur").as_string().unwrap_or_default();
    let court: String = navigateur.chars().take(48).collect();
    let fermee = js_sys::JSON::stringify(&champ(&ap, "fermee_apres_s"))
        .ok()
        .and_then(|s| s.as_string())
        .unwrap_or_default();
    format!(
        "{:>4} {:>6} s  aller-retour {:>4} / {:>4} / {:>5} ms  tics {:>4} / {:>5} ms, {:>3} trous  rafales {:>3}  perdus {:>3}  dérive {:>5} ppm  caché {:>2}, fermé après {fermee}  {court}",
        nombre(lien),
        nombre(&champ(bilan, "duree_s")),
        nombre(&champ(&ar, "p50")),
        nombre(&champ(&ar, "p99")),
        nombre(&champ(&ar, "max")),
        nombre(&champ(&ecart, "p99")),
        nombre(&champ(&ecart, "max")),
        nombre(&champ(&tics, "trous")),
        nombre(&champ(&echos, "rafales")),
        nombre(&champ(&echos, "perdus")),
        nombre(&champ(bilan, "derive_ppm")),
        nombre(&champ(&ap, "caches")),
    )
}

fn montrer(vue: &mut Vue, json: &JsValue) {
    let pages = js_sys::Array::from(&champ(json, "pages"));
    let page = pages.get(0).as_string();
    if page != vue.page {
        if let Some(p) = &page {
            dessiner_le_qr(vue, p);
            vue.adresse.set_text_content(Some(p));
        } else {
            vue.qr.set_text_content(None);
            vue.adresse.set_text_content(Some(
                "Pas de réseau : le relais n'annonce aucune adresse, les téléphones ne peuvent pas venir.",
            ));
        }
        vue.page.clone_from(&page);
    }
    let bilans = js_sys::Array::from(&champ(json, "bilans"));
    let mut lignes = vec![format!(
        "{} téléphone(s). Colonnes : lien, durée, aller-retour médiane / 99 % / pire, écart entre tics 99 % / pire, trous de plus de 100 ms, échos de plus de 100 ms, échos perdus, dérive de l'horloge, fois caché.",
        bilans.length()
    )];
    for b in bilans.iter() {
        let texte = champ(&b, "bilan").as_string().unwrap_or_default();
        let bilan = js_sys::JSON::parse(&texte).unwrap_or(JsValue::NULL);
        lignes.push(ligne_d_un_bilan(&champ(&b, "lien"), &bilan));
    }
    vue.table.set_text_content(Some(&lignes.join("\n")));
    if vue.banc == Oui::Oui {
        let json = format!(
            "{{\"quoi\":\"mesure-wifi\",\"qr\":{},\"bilans\":{}}}",
            page.as_ref().map_or_else(
                || String::from("null"),
                |p| js_sys::JSON::stringify(&JsValue::from_str(p))
                    .ok()
                    .and_then(|s| s.as_string())
                    .unwrap_or_default()
            ),
            bilans.length()
        );
        if json != vue.rapporte {
            web_sys::console::log_1(&JsValue::from_str(&format!("banc wifi {json}")));
            vue.rapporte = json;
        }
    }
}
