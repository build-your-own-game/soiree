//! L'écran du commun dans le navigateur (PHASE-3, § 2.4, § 2.8, § 2.9 ; lot 10b) :
//!
//! - **l'écran de démarrage** ([`crate::demarrage`]), dessiné par `rendu` à l'ouverture :
//!   le catalogue (`GET /jeux/catalogue`, et le rapport de chaque module éprouvé), le QR
//!   et l'adresse (`VersPage::Accueil`), la salle, les téléphones reliés
//!   (`VersPage::EtatSalle`), l'aide ([`crate::aide`], `GET /aide` toutes les deux
//!   secondes) ; son bouton « Jouer » est le geste ;
//! - **le geste** : le plein écran, puis `navigator.keyboard.lock(["Escape"])` (P38 :
//!   Chrome seulement ; ailleurs l'API n'existe pas), le contexte audio créé et relancé
//!   (SAL-174, SAL-175), le verrou d'écran (`navigator.wakeLock`) ;
//! - **« Cliquer pour reprendre »** après un rechargement de l'onglet (§ 2.4, point 7) :
//!   le plein écran, le son et le verrou d'écran ne reviennent pas sans geste ; la partie
//!   reprend tout de suite dans le Worker, sous ce voile ;
//! - **« Plein écran quitté — cliquer pour revenir »** (P38) : le navigateur a gardé Échap
//!   pour sortir du plein écran, ou l'hôte en est sorti ;
//! - **« NON ÉPROUVÉ »**, en permanence, si le module joué n'a pas de rapport d'épreuve
//!   (P17, question 16).
//!
//! Le voile et l'insigne sont des éléments du DOM posés sur la toile ; l'écran de
//! démarrage est une scène. `Keyboard Lock` et `Wake Lock` se joignent par `Reflect` :
//! `web-sys` ne les expose que sous `web_sys_unstable_apis`, un réglage de compilation que
//! le commun ne pose pas.

use std::rc::Rc;

use js_sys::{Array, Function, Promise, Reflect};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Element, HtmlElement, MouseEvent, Window};

use jeu_abi::{Oui, SceneFil};
use liaison::{AdresseAnnoncee, AdresseIp, CodeSalle};
use rendu::Cadre;
use salle::ecran::EtatSalle;

use super::{Etat, Page, banc, chaine, hexa, lire_hexa, style};
use crate::aide::{EtatAide, messages};
use crate::demarrage::{
    ACTION_JOUER, Demarrage, FORME, JeuAuCatalogue, bouton_sous, scene, textes,
};
use crate::toile::{maintenant_ms, texte_d_erreur};

/// L'aide se relit au relais toutes les deux secondes (choisi).
const AIDE_MS: i32 = 2_000;

/// La clé de `sessionStorage` qui dit qu'un geste a déjà eu lieu dans cet onglet : un
/// rechargement montre alors « Cliquer pour reprendre », et non l'écran de démarrage.
const GESTE_FAIT: &str = "soiree-geste";

/// Ce que l'écran du commun montre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Montre {
    /// L'écran de démarrage.
    Demarrage,
    /// « Cliquer pour reprendre ».
    Reprendre,
    /// « Plein écran quitté — cliquer pour revenir ».
    PleinEcranQuitte,
    /// La scène du jeu seule.
    Rien,
}

impl Montre {
    const fn nom(self) -> &'static str {
        match self {
            Montre::Demarrage => "demarrage",
            Montre::Reprendre => "reprendre",
            Montre::PleinEcranQuitte => "plein-ecran-quitte",
            Montre::Rien => "rien",
        }
    }
}

/// Un défaut voulu de l'écran du commun, pour un contrôle négatif du banc.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Fautif {
    /// Aucun.
    Aucun,
    /// L'aide se tait (le contrôle négatif du scénario `aide`, § 8.2).
    TaireLAide,
    /// Le clic du geste (« Jouer », le voile) n'est pas arrêté : il va aussi au jeu, une
    /// fois l'écran du commun tombé (un contrôle négatif du scénario `ecran`, remontée du
    /// lot 10b).
    GesteAuJeu,
    /// L'écran du commun ne couvre pas le jeu : clics, glisser et fichiers lui vont
    /// dessous (`page::le_commun_couvre` toujours faux ; un contrôle négatif du scénario
    /// `ecran`, remontée du lot 10b).
    NeCouvrePas,
}

/// L'état de l'écran du commun.
pub(super) struct Commun {
    pub(super) montre: Montre,
    demarrage: Demarrage,
    aide: EtatAide,
    /// L'adresse entière du QR, fragment compris (`http://<ip>:<port>/s/<SALLE>#…`).
    pub(super) qr: Option<String>,
    /// Le plein écran a été demandé par un geste : en sortir montre le voile.
    plein_voulu: bool,
    /// Échap est verrouillé pour la page (Keyboard Lock, P38).
    pub(super) verrou: bool,
    /// Le verrou d'écran tenu (`WakeLockSentinel`).
    reveil: Option<JsValue>,
    /// Le banc seulement : le contrôle négatif du scénario `aide` (« une option qui tait
    /// l'aide »), ou de `ecran` (le geste au jeu, le jeu pas couvert).
    pub(super) fautif: Fautif,
    voile: Element,
    insigne: Element,
    scene: Option<SceneFil>,
    sale: bool,
    rapporte: String,
}

/// Le voile et l'insigne, posés sur la toile.
pub(super) fn creer(
    fenetre: &Window,
    document: &Document,
    corps: &HtmlElement,
    fautif: Fautif,
) -> Result<Commun, String> {
    let voile = document
        .create_element("div")
        .map_err(|e| texte_d_erreur(&e))?;
    style(
        &voile,
        &[
            ("position", "fixed"),
            ("inset", "0"),
            ("display", "none"),
            ("align-items", "center"),
            ("justify-content", "center"),
            ("background", "rgba(0,0,0,0.72)"),
            ("color", "#fff"),
            ("font-size", "48px"),
            ("cursor", "pointer"),
            ("text-align", "center"),
            // Au-dessus de la page du jeu et de ses composants (`interaction`, lot 11 :
            // l'espaceur à 1, le bandeau à 2, les boutons du commun à 3).
            ("z-index", "4"),
        ],
    );
    corps.append_child(&voile).map_err(|e| texte_d_erreur(&e))?;
    let insigne = document
        .create_element("div")
        .map_err(|e| texte_d_erreur(&e))?;
    insigne.set_text_content(Some("NON ÉPROUVÉ"));
    style(
        &insigne,
        &[
            ("position", "fixed"),
            ("top", "12px"),
            ("right", "12px"),
            ("padding", "6px 14px"),
            ("background", "#b3261e"),
            ("color", "#fff"),
            ("font-size", "22px"),
            ("font-weight", "bold"),
            ("display", "none"),
            ("pointer-events", "none"),
            ("z-index", "4"),
        ],
    );
    corps
        .append_child(&insigne)
        .map_err(|e| texte_d_erreur(&e))?;
    let montre = if geste_deja_fait(fenetre) {
        Montre::Reprendre
    } else {
        Montre::Demarrage
    };
    let c = Commun {
        montre,
        demarrage: Demarrage::default(),
        aide: EtatAide::default(),
        qr: None,
        plein_voulu: false,
        verrou: false,
        reveil: None,
        fautif,
        voile,
        insigne,
        scene: None,
        sale: true,
        rapporte: String::new(),
    };
    montrer_le_voile(&c);
    Ok(c)
}

fn geste_deja_fait(fenetre: &Window) -> bool {
    fenetre
        .session_storage()
        .ok()
        .flatten()
        .and_then(|s| s.get_item(GESTE_FAIT).ok().flatten())
        .is_some_and(|v| v == "1")
}

/// Le voile, selon ce que l'écran montre.
fn montrer_le_voile(c: &Commun) {
    let texte = match c.montre {
        Montre::Reprendre => Some("Cliquer pour reprendre"),
        Montre::PleinEcranQuitte => Some("Plein écran quitté — cliquer pour revenir"),
        Montre::Demarrage | Montre::Rien => None,
    };
    c.voile.set_text_content(texte);
    style(
        &c.voile,
        &[("display", if texte.is_some() { "flex" } else { "none" })],
    );
}

/// L'insigne « NON ÉPROUVÉ » : le module joué est-il au catalogue sans rapport ?
fn montrer_l_insigne(c: &Commun) {
    style(
        &c.insigne,
        &[("display", if non_eprouve(c) { "block" } else { "none" })],
    );
}

fn non_eprouve(c: &Commun) -> bool {
    c.demarrage.en_jeu.is_some_and(|m| {
        c.demarrage
            .jeux
            .iter()
            .any(|j| j.empreinte == m && !j.eprouve)
    })
}

/// La scène du commun à dessiner, si l'écran de démarrage est montré (refaite si
/// quelque chose a changé).
pub(super) fn scene_a_dessiner(p: &mut Page) -> Option<SceneFil> {
    if p.commun.montre != Montre::Demarrage {
        return None;
    }
    if p.commun.sale || p.commun.scene.is_none() {
        p.commun.demarrage.aide = if p.commun.fautif == Fautif::TaireLAide {
            Vec::new()
        } else {
            messages(&p.commun.aide, maintenant_ms() as u64)
        };
        p.commun.scene = Some(scene(&p.commun.demarrage));
        p.commun.sale = false;
        rapporter(p);
    }
    p.commun.scene.clone()
}

/// Dit au banc ce que l'écran du commun montre, quand cela change.
pub(super) fn rapporter(p: &mut Page) {
    let textes_: Vec<String> = p
        .commun
        .scene
        .as_ref()
        .filter(|_| p.commun.montre == Montre::Demarrage)
        .map(|s| textes(s).into_iter().map(chaine).collect())
        .unwrap_or_default();
    let aide: Vec<String> = p
        .commun
        .demarrage
        .aide
        .iter()
        .map(|m| format!("\"{}\"", m.nom()))
        .collect();
    let json = format!(
        "{{\"quoi\":\"ecran-du-commun\",\"montre\":\"{}\",\"qr\":{},\"aide\":[{}],\"non_eprouve\":{},\"plein_ecran\":{},\"verrou\":{},\"textes\":[{}]}}",
        p.commun.montre.nom(),
        p.commun.montre == Montre::Demarrage && p.commun.demarrage.qr,
        aide.join(","),
        non_eprouve(&p.commun),
        p.document.fullscreen_element().is_some(),
        p.commun.verrou,
        textes_.join(",")
    );
    if json != p.commun.rapporte {
        banc(p, &json);
        p.commun.rapporte = json;
    }
}

/// Le banc : oublie le dernier rapport, pour que le prochain se redise même inchangé.
pub(super) fn oublier_le_rapport(p: &mut Page) {
    p.commun.rapporte.clear();
}

/// L'adresse d'un téléphone, pour l'écran : `http://<ip>:<port>/s/<SALLE>`.
fn adresse_du_telephone(a: &AdresseAnnoncee, salle: CodeSalle) -> String {
    let hote = match a.ip {
        AdresseIp::V4(o) => std::net::Ipv4Addr::from(o).to_string(),
        AdresseIp::V6(o) => format!("[{}]", std::net::Ipv6Addr::from(o)),
    };
    format!("http://{hote}:{}/s/{salle}", a.port)
}

/// `VersPage::Accueil` : le QR, l'adresse, la salle.
pub(super) fn accueil(
    p: &mut Page,
    salle: CodeSalle,
    fragment: &str,
    adresses: &[AdresseAnnoncee],
) {
    let c = &mut p.commun;
    c.demarrage.salle = Some(salle.to_string());
    c.aide.adresses = Some(adresses.len());
    if let Some(a) = adresses.first() {
        let adresse = adresse_du_telephone(a, salle);
        c.qr = Some(format!("{adresse}#{fragment}"));
        c.demarrage.adresse = Some(adresse);
        c.demarrage.qr = true;
        if c.aide.qr_depuis_ms.is_none() {
            c.aide.qr_depuis_ms = Some(maintenant_ms() as u64);
        }
    } else {
        c.qr = None;
        c.demarrage.adresse = None;
        c.demarrage.qr = false;
    }
    c.sale = true;
    if let Some(e) = p.ecran.as_mut() {
        e.contexte.adresse_qr.clone_from(&p.commun.qr);
    }
}

/// `VersPage::EtatSalle` : les téléphones reliés.
pub(super) fn etat_salle(p: &mut Page, e: &EtatSalle) {
    let presents = e.appareils.iter().filter(|a| a.present == Oui::Oui).count();
    if presents > 0 {
        p.commun.aide.telephone_vu = true;
    }
    if presents != p.commun.demarrage.telephones {
        p.commun.demarrage.telephones = presents;
        p.commun.sale = true;
    }
}

/// `VersPage::Jeu` : le module que la salle joue.
pub(super) fn jeu(p: &mut Page, module: [u8; 32]) {
    p.commun.demarrage.en_jeu = Some(module);
    p.commun.sale = true;
    montrer_l_insigne(&p.commun);
    rapporter(p);
}

/// Un texte de `GET` (la boucle locale), ou rien.
async fn lire(fenetre: &Window, url: &str) -> Option<(u16, String)> {
    let r: web_sys::Response = JsFuture::from(fenetre.fetch_with_str(url))
        .await
        .ok()?
        .dyn_into()
        .ok()?;
    let statut = r.status();
    let t = JsFuture::from(r.text().ok()?).await.ok()?.as_string()?;
    Some((statut, t))
}

fn champ(v: &JsValue, nom: &str) -> JsValue {
    Reflect::get(v, &JsValue::from_str(nom)).unwrap_or(JsValue::UNDEFINED)
}

fn nombre_u16(v: &JsValue) -> Option<u16> {
    v.as_f64()
        .filter(|x| *x >= 0.0 && *x <= f64::from(u16::MAX))
        .map(|x| x as u16)
}

/// Le catalogue, et le rapport de chaque module éprouvé (nom, joueurs, date).
async fn lire_le_catalogue(fenetre: &Window) -> Vec<JeuAuCatalogue> {
    let Some((200, t)) = lire(fenetre, "/jeux/catalogue").await else {
        return Vec::new();
    };
    let Ok(v) = js_sys::JSON::parse(&t) else {
        return Vec::new();
    };
    let mut jeux = Vec::new();
    for j in Array::from(&champ(&v, "jeux")).iter() {
        let Some(empreinte) = champ(&j, "empreinte")
            .as_string()
            .and_then(|h| lire_hexa(&h))
        else {
            continue;
        };
        let eprouve = champ(&j, "eprouve").as_bool() == Some(true);
        let mut jeu = JeuAuCatalogue {
            empreinte,
            nom: None,
            joueurs: None,
            date: None,
            eprouve,
        };
        if eprouve
            && let Some((200, r)) = lire(fenetre, &format!("/jeux/{}.json", hexa(&empreinte))).await
            && let Ok(r) = js_sys::JSON::parse(&r)
        {
            jeu.nom = champ(&r, "nom").as_string();
            jeu.date = champ(&r, "date").as_string();
            let places = champ(&r, "places");
            jeu.joueurs = nombre_u16(&champ(&places, "joueurs_min"))
                .zip(nombre_u16(&champ(&places, "joueurs_max")));
        }
        jeux.push(jeu);
    }
    jeux
}

/// Lit le catalogue, puis l'aide du relais toutes les deux secondes, et refait l'aide
/// (l'heure compte : « aucun téléphone depuis une minute »).
pub(super) fn sonder(etat: &Etat) -> Result<(), String> {
    let e = Rc::clone(etat);
    spawn_local(async move {
        let fenetre = e.borrow().fenetre.clone();
        let jeux = lire_le_catalogue(&fenetre).await;
        let mut p = e.borrow_mut();
        p.commun.demarrage.jeux = jeux;
        p.commun.sale = true;
        montrer_l_insigne(&p.commun);
        rapporter(&mut p);
    });
    let e = Rc::clone(etat);
    let tic = Closure::<dyn FnMut()>::new(move || {
        let e = Rc::clone(&e);
        spawn_local(async move {
            let fenetre = e.borrow().fenetre.clone();
            let lu = lire(&fenetre, "/aide").await;
            let mut p = e.borrow_mut();
            if let Some((200, t)) = lu
                && let Ok(v) = js_sys::JSON::parse(&t)
            {
                p.commun.aide.pages_sans_lien = champ(&v, "pages_sans_lien")
                    .as_f64()
                    .map_or(0, |x| x as u32);
                p.commun.aide.pare_feu_gene = matches!(
                    champ(&v, "pare_feu").as_string().as_deref(),
                    Some("a-autoriser" | "refuse" | "bloque-tout")
                );
            }
            let maintenant = if p.commun.fautif == Fautif::TaireLAide {
                Vec::new()
            } else {
                messages(&p.commun.aide, maintenant_ms() as u64)
            };
            if maintenant != p.commun.demarrage.aide {
                p.commun.sale = true;
            }
            if p.commun.montre != Montre::Demarrage {
                // L'aide se dit aussi au banc hors de l'écran de démarrage : elle s'y
                // dira de nouveau quand il se montrera.
                p.commun.demarrage.aide = maintenant;
                rapporter(&mut p);
            }
        });
    });
    etat.borrow()
        .fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            tic.as_ref().unchecked_ref(),
            AIDE_MS,
        )
        .map_err(|e| texte_d_erreur(&e))?;
    tic.forget();
    Ok(())
}

/// Appelle une méthode d'un objet du navigateur par son nom ; rend son résultat.
fn appeler(objet: &JsValue, methode: &str, arguments: &Array) -> Result<JsValue, String> {
    let f: Function = Reflect::get(objet, &JsValue::from_str(methode))
        .map_err(|e| texte_d_erreur(&e))?
        .dyn_into()
        .map_err(|_| format!("{methode} n'existe pas ici"))?;
    f.apply(objet, arguments).map_err(|e| texte_d_erreur(&e))
}

/// **Le geste**, dans le gestionnaire d'un clic : le plein écran, puis le verrou d'Échap
/// (P38) ; le son (SAL-174) ; le verrou d'écran. Le voile tombe, la scène du jeu se montre.
pub(super) fn geste(etat: &Etat) {
    let (plein, audio, reveil, fenetre) = {
        let mut p = etat.borrow_mut();
        p.commun.montre = Montre::Rien;
        p.commun.plein_voulu = true;
        p.commun.sale = true;
        montrer_le_voile(&p.commun);
        if let Ok(Some(s)) = p.fenetre.session_storage() {
            let _: Result<(), JsValue> = s.set_item(GESTE_FAIT, "1");
        }
        let racine: JsValue = p
            .document
            .document_element()
            .map_or(JsValue::UNDEFINED, JsValue::from);
        let plein = appeler(&racine, "requestFullscreen", &Array::new());
        let audio = p.son.au_geste();
        let reveil = demander_le_reveil(&p.fenetre);
        rapporter(&mut p);
        banc(&p, "{\"quoi\":\"geste\"}");
        (plein, audio, reveil, p.fenetre.clone())
    };
    let e = Rc::clone(etat);
    spawn_local(async move {
        let plein = match plein {
            Ok(promesse) => match promesse.dyn_into::<Promise>() {
                Ok(p) => JsFuture::from(p)
                    .await
                    .map(|_| ())
                    .map_err(|x| texte_d_erreur(&x)),
                Err(_) => Ok(()),
            },
            Err(x) => Err(x),
        };
        let verrou = if plein.is_ok() {
            verrouiller_echap(&fenetre).await
        } else {
            Err(String::from("pas de plein écran"))
        };
        let audio = match audio {
            Some(p) => JsFuture::from(p)
                .await
                .map(|_| ())
                .map_err(|x| texte_d_erreur(&x)),
            None => Err(String::from("pas de contexte audio")),
        };
        let reveil = match reveil {
            Ok(p) => JsFuture::from(p).await.map_err(|x| texte_d_erreur(&x)),
            Err(x) => Err(x),
        };
        {
            let mut p = e.borrow_mut();
            p.commun.verrou = verrou.is_ok();
            p.commun.reveil = reveil.as_ref().ok().cloned();
            let issue = |r: &Result<(), String>| match r {
                Ok(()) => String::from("true"),
                Err(x) => chaine(x),
            };
            let json = format!(
                "{{\"quoi\":\"geste-fait\",\"plein_ecran\":{},\"verrou_echap\":{},\"audio\":{},\"son\":\"{}\",\"reveil\":{}}}",
                issue(&plein),
                issue(&verrou),
                issue(&audio),
                p.son.nom_de_l_etat(),
                issue(&reveil.map(|_| ()))
            );
            banc(&p, &json);
            rapporter(&mut p);
        }
        super::audio::HautParleur::decoder(&e).await;
    });
}

/// `navigator.keyboard.lock(["Escape"])` (Keyboard Lock, P38) : Chrome seulement.
async fn verrouiller_echap(fenetre: &Window) -> Result<(), String> {
    let clavier = Reflect::get(&fenetre.navigator(), &JsValue::from_str("keyboard"))
        .map_err(|e| texte_d_erreur(&e))?;
    if clavier.is_undefined() || clavier.is_null() {
        return Err(String::from("navigator.keyboard absent (Keyboard Lock)"));
    }
    let promesse: Promise = appeler(
        &clavier,
        "lock",
        &Array::of1(&Array::of1(&JsValue::from_str("Escape"))),
    )?
    .dyn_into()
    .map_err(|_| String::from("keyboard.lock ne rend pas de promesse"))?;
    JsFuture::from(promesse)
        .await
        .map(|_| ())
        .map_err(|e| texte_d_erreur(&e))
}

/// `navigator.wakeLock.request("screen")` : la promesse, ou pourquoi il n'y en a pas.
fn demander_le_reveil(fenetre: &Window) -> Result<Promise, String> {
    let w = Reflect::get(&fenetre.navigator(), &JsValue::from_str("wakeLock"))
        .map_err(|e| texte_d_erreur(&e))?;
    if w.is_undefined() || w.is_null() {
        return Err(String::from("navigator.wakeLock absent"));
    }
    appeler(&w, "request", &Array::of1(&JsValue::from_str("screen")))?
        .dyn_into()
        .map_err(|_| String::from("wakeLock.request ne rend pas de promesse"))
}

/// Le clic sur « Jouer », écouté sur le document, à la capture (voir [`ecouter`]).
fn ecouter_jouer(etat: &Etat, toile: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    let pour_le_clic = Rc::clone(etat);
    let la_toile = toile.clone();
    let clic = Closure::<dyn FnMut(MouseEvent)>::new(move |souris: MouseEvent| {
        if !souris.is_trusted() {
            return;
        }
        let (jouer, arreter) = {
            let Ok(page) = pour_le_clic.try_borrow() else {
                return;
            };
            let arreter = page.commun.fautif != Fautif::GesteAuJeu;
            let jouer = page.commun.montre == Montre::Demarrage
                && page.commun.scene.as_ref().is_some_and(|s| {
                    let a = Cadre::new(la_toile.width(), la_toile.height(), FORME).transformation();
                    let dpr = page.jeu.dpr;
                    let (x, y) = (
                        f64::from(souris.client_x()) * dpr,
                        f64::from(souris.client_y()) * dpr,
                    );
                    let echelle = f64::from(a.echelle);
                    echelle > 0.0
                        && bouton_sous(
                            s,
                            (x - f64::from(a.x)) / echelle,
                            (y - f64::from(a.y)) / echelle,
                        ) == Some(ACTION_JOUER)
                });
            (jouer, arreter)
        };
        if jouer {
            if arreter {
                souris.stop_propagation();
            }
            geste(&pour_le_clic);
        }
    });
    etat.borrow()
        .document
        .add_event_listener_with_callback_and_bool("click", clic.as_ref().unchecked_ref(), true)
        .map_err(|e| texte_d_erreur(&e))?;
    clic.forget();
    Ok(())
}

/// Le clic : sur le bouton « Jouer » de l'écran de démarrage, ou sur le voile.
///
/// Le clic sur « Jouer » s'écoute sur le document : depuis le lot 11, l'espaceur de la
/// page du jeu couvre la toile, qui ne reçoit plus de clic. La toile est en pixels de
/// l'appareil (`interaction::regler_la_taille`), le clic en pixels CSS. Un clic que la
/// page provoque (le lien d'un téléchargement, pendant qu'elle tient son état) n'est pas
/// un geste. Le clic du geste (« Jouer », le voile) s'arrête là : il ne va pas, après,
/// aux écouteurs du jeu (`interaction`), qui le prendraient pour un clic sur sa scène
/// une fois le voile tombé ; « Jouer » s'écoute donc à la capture. Le scénario
/// `ecran tier-list` le constate (aucune ligne `au-jeu` au clic du geste) ;
/// `?fautif=geste-au-jeu` est son contrôle négatif.
pub(super) fn ecouter(etat: &Etat, toile: &web_sys::HtmlCanvasElement) -> Result<(), String> {
    ecouter_jouer(etat, toile)?;
    let e = Rc::clone(etat);
    let sur_le_voile = Closure::<dyn FnMut(MouseEvent)>::new(move |souris: MouseEvent| {
        let (voile, arreter) = {
            let p = e.borrow();
            (
                matches!(
                    p.commun.montre,
                    Montre::Reprendre | Montre::PleinEcranQuitte
                ),
                p.commun.fautif != Fautif::GesteAuJeu,
            )
        };
        if voile {
            if arreter {
                souris.stop_propagation();
            }
            geste(&e);
        }
    });
    etat.borrow()
        .commun
        .voile
        .add_event_listener_with_callback("click", sur_le_voile.as_ref().unchecked_ref())
        .map_err(|e| texte_d_erreur(&e))?;
    sur_le_voile.forget();
    // Sortir du plein écran (Échap gardé par le navigateur, P38) montre le voile.
    let e = Rc::clone(etat);
    let change = Closure::<dyn FnMut()>::new(move || {
        let mut p = e.borrow_mut();
        let dedans = p.document.fullscreen_element().is_some();
        if !dedans {
            p.commun.verrou = false;
            if p.commun.plein_voulu && p.commun.montre == Montre::Rien {
                p.commun.montre = Montre::PleinEcranQuitte;
                montrer_le_voile(&p.commun);
            }
        }
        let json = format!("{{\"quoi\":\"plein-ecran\",\"dedans\":{dedans}}}");
        banc(&p, &json);
        rapporter(&mut p);
    });
    etat.borrow()
        .document
        .add_event_listener_with_callback("fullscreenchange", change.as_ref().unchecked_ref())
        .map_err(|e| texte_d_erreur(&e))?;
    change.forget();
    // Le verrou d'écran tombe quand l'onglet se cache ; il se redemande à son retour.
    let e = Rc::clone(etat);
    let visible = Closure::<dyn FnMut()>::new(move || {
        let (rendre, fenetre) = {
            let p = e.borrow();
            (
                !p.document.hidden() && p.commun.plein_voulu && p.commun.reveil.is_some(),
                p.fenetre.clone(),
            )
        };
        if rendre && let Ok(promesse) = demander_le_reveil(&fenetre) {
            let e = Rc::clone(&e);
            spawn_local(async move {
                let r = JsFuture::from(promesse).await.ok();
                e.borrow_mut().commun.reveil = r;
            });
        }
    });
    etat.borrow()
        .document
        .add_event_listener_with_callback("visibilitychange", visible.as_ref().unchecked_ref())
        .map_err(|e| texte_d_erreur(&e))?;
    visible.forget();
    Ok(())
}

/// Le centre d'un élément de l'écran du commun, en pixels de la fenêtre, pour le banc :
/// le bouton « Jouer » (lu dans la scène de démarrage), ou le voile.
pub(super) fn centre_pour_le_banc(
    p: &Page,
    toile: &web_sys::HtmlCanvasElement,
) -> Option<(f64, f64)> {
    match p.commun.montre {
        Montre::Demarrage => {
            let boite = p.commun.scene.as_ref()?.noeuds.iter().find_map(|n| {
                if let jeu_abi::SorteNoeud::Bouton { boite, action, .. } = &n.sorte
                    && action.as_slice() == ACTION_JOUER
                {
                    Some(*boite)
                } else {
                    None
                }
            })?;
            let a = Cadre::new(toile.width(), toile.height(), FORME).transformation();
            let cx = boite.x.en_unites() + boite.l.en_unites() / 2.0;
            let cy = boite.y.en_unites() + boite.h.en_unites() / 2.0;
            let e = f64::from(a.echelle);
            // La toile est en pixels de l'appareil (lot 11), la fenêtre en pixels CSS.
            let dpr = p.jeu.dpr;
            Some((
                (cx * e + f64::from(a.x)) / dpr,
                (cy * e + f64::from(a.y)) / dpr,
            ))
        }
        Montre::Reprendre | Montre::PleinEcranQuitte => {
            let w = p.fenetre.inner_width().ok()?.as_f64()?;
            let h = p.fenetre.inner_height().ok()?.as_f64()?;
            Some((w / 2.0, h / 2.0))
        }
        Montre::Rien => None,
    }
}
