//! La page de la télé, reliée au Worker de l'autorité (PHASE-3, § 2.1, § 2.3, § 2.9 ;
//! lot 10a). Elle lance le Worker, lui poste le signal de chaque image (P6), dessine la
//! scène qu'il envoie, le garde (`salle::chien`) et le relance, lui fait écarter l'entrée
//! qui bloque deux fois (P39), dit « Remplacée » quand un autre onglet tient la salle
//! (P36), pose les marques F8 (SAL-162). En mode banc, elle écrit sur sa console ce que le
//! banc juge (`banc …`), et lui offre ses crochets (`window.__banc`).
//!
//! Le démarrage, les composants, les entrées du jeu, le son, le plein écran et le QR
//! viennent aux lots 10b à 13 : ici, l'écran du commun se réduit à un bandeau de texte.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "l'heure et la taille de la fenêtre du navigateur (des f64 positifs) ramenées en entiers ; rien ne revient à une partie"
)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fmt::Write as _;
use std::rc::Rc;

use jeu_abi::{IdGeste, Oui};
use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use salle::chien::{Chien, Decision, TIC_MS};
use salle::ecran::{CommandeEcran, DeLaPage, PanneSalle, VersPage};
use salle::simulation::reponse_par_defaut;
use wasm_bindgen::prelude::{Closure, wasm_bindgen};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;
use web_sys::{
    Document, Element, HtmlCanvasElement, HtmlElement, KeyboardEvent, MessageEvent, Window, Worker,
    WorkerOptions, WorkerType,
};

use crate::toile::{ChoixMoteur, Ecran, maintenant_ms, texte_d_erreur};

/// Un redessin au plus tous les 250 ms sans scène neuve : les comptes à rebours (choisi).
const REDESSIN_MS: f64 = 250.0;

/// L'état de la page.
struct Page {
    /// Le nom de cet onglet, pour le banc.
    onglet: String,
    fenetre: Window,
    document: Document,
    banc: bool,
    module: Option<[u8; 32]>,
    script: String,
    worker: Option<Worker>,
    /// Le numéro du Worker courant : les messages d'un Worker tué ne comptent plus.
    generation: u64,
    pret: bool,
    ecarter: Option<IdGeste>,
    chien: Chien,
    ecran: Option<Ecran>,
    dessin: Dessin,
    bandeau: Element,
    /// Le bandeau propose-t-il de reprendre ?
    proposition: Proposition,
    relances: u64,
}

type Etat = Rc<RefCell<Page>>;

/// Ce que le bandeau propose.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Proposition {
    Rien,
    /// « Remplacée — Reprendre ici » (P36, SAL-21).
    ReprendreIci,
}

/// Ce qui attend d'être dessiné.
struct Dessin {
    /// Les scènes et les ressources, à passer à l'écran une à une.
    file: VecDeque<Vec<u8>>,
    /// La file est-elle en train de passer ?
    occupe: bool,
    /// Une scène neuve attend son dessin.
    sale: bool,
    dernier_ms: f64,
}

/// Ce que fait la page à chaque image.
type ChaqueImage = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

/// Une ligne pour le banc, sur la console : `banc <onglet> <json>`. L'onglet est un nom
/// tiré à l'ouverture de la page : le banc distingue ainsi deux onglets de l'écran.
fn banc(p: &Page, json: &str) {
    if p.banc {
        web_sys::console::log_1(&JsValue::from_str(&format!("banc {} {json}", p.onglet)));
    }
}

fn hexa(o: &[u8]) -> String {
    o.iter().fold(String::new(), |mut s, b| {
        let _: core::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

fn lire_hexa(t: &str) -> Option<[u8; 32]> {
    let mut o = [0u8; 32];
    if t.len() != 64 {
        return None;
    }
    for (k, case) in o.iter_mut().enumerate() {
        let d = t.get(k.checked_mul(2)?..k.checked_mul(2)?.checked_add(2)?)?;
        *case = u8::from_str_radix(d, 16).ok()?;
    }
    Some(o)
}

/// Une chaîne JSON.
fn chaine(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if u32::from(c) < 0x20 => {
                let _: core::fmt::Result = write!(o, "\\u{:04x}", u32::from(c));
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

/// L'heure absolue de la page, `performance.timeOrigin + performance.now()`, en µs.
fn heure_us(f: &Window) -> u64 {
    let t = f
        .performance()
        .map_or(0.0, |p| (p.time_origin() + p.now()) * 1000.0);
    if t > 0.0 { t as u64 } else { 0 }
}

fn style(e: &Element, regles: &[(&str, &str)]) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let s = h.style();
        for (n, v) in regles {
            let _: Result<(), JsValue> = s.set_property(n, v);
        }
    }
}

/// Le bandeau du commun : un texte par-dessus la scène, ou rien.
fn dire(p: &Page, texte: Option<&str>) {
    p.bandeau.set_text_content(texte);
    style(
        &p.bandeau,
        &[("display", if texte.is_some() { "block" } else { "none" })],
    );
}

fn poster(p: &Page, m: &DeLaPage<'_>) {
    let (Some(w), Ok(o)) = (p.worker.as_ref(), m.encoder()) else {
        return;
    };
    let tampon = Uint8Array::from(o.as_slice()).buffer();
    let _: Result<(), JsValue> = w.post_message_with_transfer(&tampon, &Array::of1(&tampon));
}

/// Le point d'entrée de la page, que `demarrage.js` appelle avec sa propre adresse (celle
/// du script du Worker).
#[wasm_bindgen]
pub fn demarrer_la_page(script: String) {
    spawn_local(async move {
        if let Err(e) = demarrer(script).await {
            web_sys::console::error_1(&JsValue::from_str(&format!("la télé : {e}")));
        }
    });
}

async fn demarrer(script: String) -> Result<(), String> {
    let fenetre = web_sys::window().ok_or("pas de fenêtre")?;
    let document = fenetre.document().ok_or("pas de document")?;
    let corps = document.body().ok_or("pas de corps")?;
    style(
        &corps,
        &[
            ("margin", "0"),
            ("background", "#000"),
            ("color", "#ddd"),
            ("font-family", "sans-serif"),
            ("overflow", "hidden"),
        ],
    );
    let au_banc = document
        .query_selector("meta[name=soiree-banc]")
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"))
        .is_some_and(|c| c == "oui");
    let recherche = fenetre.location().search().unwrap_or_default();
    let parametre = |nom: &str| {
        recherche
            .trim_start_matches('?')
            .split('&')
            .find_map(|kv| kv.strip_prefix(nom)?.strip_prefix('=').map(str::to_owned))
    };
    let module = parametre("jeu").and_then(|h| lire_hexa(&h));
    let choix = if parametre("moteur").as_deref() == Some("webgl2") {
        ChoixMoteur::WebGl2
    } else {
        ChoixMoteur::Auto
    };
    let (toile, bandeau) = la_toile_et_le_bandeau(&fenetre, &document, &corps)?;
    let mut ecran = Ecran::creer(toile, choix).await?;
    let ceder = ceder(&fenetre);
    ecran.chauffer(ceder.as_ref().unchecked_ref()).await?;
    drop(ceder);
    let onglet = format!("{:08x}", (js_sys::Math::random() * 4_294_967_295.0) as u32);
    let etat: Etat = Rc::new(RefCell::new(Page {
        onglet,
        fenetre: fenetre.clone(),
        document: document.clone(),
        banc: au_banc,
        module,
        script,
        worker: None,
        generation: 0,
        pret: false,
        ecarter: None,
        chien: Chien::new(),
        ecran: Some(ecran),
        dessin: Dessin {
            file: VecDeque::new(),
            occupe: false,
            sale: false,
            dernier_ms: 0.0,
        },
        bandeau,
        proposition: Proposition::Rien,
        relances: 0,
    }));
    if Reflect::get(&document, &JsValue::from_str("wasDiscarded"))
        .ok()
        .and_then(|v| v.as_bool())
        == Some(true)
    {
        // L'onglet avait été abandonné par le navigateur : la partie reprend de la garde.
        banc(&etat.borrow(), "{\"quoi\":\"abandonne\"}");
    }
    lancer_le_worker(&etat);
    ecouter_le_clavier(&etat)?;
    ecouter_le_bandeau(&etat)?;
    quitter(&etat)?;
    battre(&etat)?;
    image(&etat);
    if au_banc {
        crochets(&etat)?;
    }
    let moteur = etat
        .borrow()
        .ecran
        .as_ref()
        .map_or("?", Ecran::sorte_de_moteur);
    banc(
        &etat.borrow(),
        &format!("{{\"quoi\":\"page\",\"moteur\":\"{moteur}\"}}"),
    );
    Ok(())
}

/// La toile, à la taille de la fenêtre, et le bandeau du commun par-dessus.
fn la_toile_et_le_bandeau(
    fenetre: &Window,
    document: &Document,
    corps: &HtmlElement,
) -> Result<(HtmlCanvasElement, Element), String> {
    let toile: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(|e| texte_d_erreur(&e))?
        .dyn_into()
        .map_err(|_| "pas une toile")?;
    let largeur = fenetre
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0) as u32;
    let hauteur = fenetre
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(720.0) as u32;
    toile.set_width(largeur.max(1));
    toile.set_height(hauteur.max(1));
    style(&toile, &[("display", "block")]);
    corps.append_child(&toile).map_err(|e| texte_d_erreur(&e))?;
    let bandeau = document
        .create_element("div")
        .map_err(|e| texte_d_erreur(&e))?;
    style(
        &bandeau,
        &[
            ("position", "fixed"),
            ("left", "0"),
            ("right", "0"),
            ("bottom", "0"),
            ("padding", "1em"),
            ("background", "rgba(0,0,0,0.8)"),
            ("font-size", "24px"),
            ("display", "none"),
        ],
    );
    corps
        .append_child(&bandeau)
        .map_err(|e| texte_d_erreur(&e))?;
    Ok((toile, bandeau))
}

/// Une fonction qui rend une promesse tenue au prochain tour : la page cède la main
/// pendant qu'elle attend le GPU (sans `new Function`, que la politique interdit).
fn ceder(f: &Window) -> Closure<dyn FnMut() -> Promise> {
    let f = f.clone();
    Closure::new(move || {
        let f = f.clone();
        Promise::new(&mut |rendre: Function, _: Function| {
            let _: Result<i32, JsValue> =
                f.set_timeout_with_callback_and_timeout_and_arguments_0(&rendre, 0);
        })
    })
}

/// Lance un Worker neuf : il dira qu'il est là, et la page lui dira de démarrer.
fn lancer_le_worker(etat: &Etat) {
    let mut p = etat.borrow_mut();
    if let Some(w) = p.worker.take() {
        w.terminate();
    }
    p.generation = p.generation.saturating_add(1);
    p.pret = false;
    let options = WorkerOptions::new();
    options.set_type(WorkerType::Module);
    let w = match Worker::new_with_options(&p.script, &options) {
        Ok(w) => w,
        Err(e) => {
            let detail = texte_d_erreur(&e);
            dire(&p, Some(&format!("Le commun ne démarre pas : {detail}")));
            banc(
                &p,
                &format!(
                    "{{\"quoi\":\"panne\",\"detail\":{}}}",
                    chaine(&format!("Worker : {detail}"))
                ),
            );
            return;
        }
    };
    let generation = p.generation;
    let e = Rc::clone(etat);
    let recu = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
        if let Some(b) = m.data().dyn_ref::<js_sys::ArrayBuffer>() {
            du_worker(&e, generation, &Uint8Array::new(b).to_vec());
        }
    });
    w.set_onmessage(Some(recu.as_ref().unchecked_ref()));
    recu.forget();
    let e = Rc::clone(etat);
    let erreur = Closure::<dyn FnMut(web_sys::ErrorEvent)>::new(move |x: web_sys::ErrorEvent| {
        let p = e.borrow();
        let detail = format!("Worker : {} ({}:{})", x.message(), x.filename(), x.lineno());
        web_sys::console::error_1(&JsValue::from_str(&detail));
        banc(
            &p,
            &format!("{{\"quoi\":\"panne\",\"detail\":{}}}", chaine(&detail)),
        );
    });
    w.set_onerror(Some(erreur.as_ref().unchecked_ref()));
    erreur.forget();
    p.worker = Some(w);
    let t = maintenant_ms() as u64;
    p.chien.demarre(t);
}

/// Un message du Worker.
fn du_worker(etat: &Etat, generation: u64, octets: &[u8]) {
    let mut p = etat.borrow_mut();
    if generation != p.generation {
        return;
    }
    let Ok(m) = VersPage::decoder(octets) else {
        return;
    };
    let t = maintenant_ms() as u64;
    if p.chien.message(t, &m) {
        // Trois réveils de suite en retard de plus de 100 ms (§ 2.3, M6).
        dire(&p, Some("Le commun prend du retard : la partie ralentit."));
        banc(&p, "{\"quoi\":\"retard-signale\"}");
    }
    if !p.pret {
        // Le premier message : le Worker écoute ; il peut démarrer.
        p.pret = true;
        let module = p.module;
        let banc_ = if p.banc { Oui::Oui } else { Oui::Non };
        poster(
            &p,
            &DeLaPage::Demarrer {
                banc: banc_,
                module,
            },
        );
        if let Some(e) = p.ecarter.take() {
            poster(
                &p,
                &DeLaPage::Commande(CommandeEcran::Reprendre { ecarter: Some(e) }),
            );
        }
    }
    match m {
        VersPage::Scene { .. } | VersPage::Ressource { .. } => {
            p.dessin.file.push_back(octets.to_vec());
            drop(p);
            traiter_la_file(etat);
        }
        VersPage::Jeu { module, .. } => {
            // Le jeu passe à l'écran dans l'ordre de la file, avant ses polices et ses
            // scènes.
            banc(
                &p,
                &format!("{{\"quoi\":\"jeu\",\"module\":\"{}\"}}", hexa(&module)),
            );
            p.dessin.file.push_back(octets.to_vec());
            drop(p);
            traiter_la_file(etat);
        }
        VersPage::Demande { id, quoi } => {
            // Les demandes du jeu (rendu, image, lecture, téléchargement) viennent avec
            // l'écran (lot 11) : la page répond comme la soirée en mémoire.
            let quoi = reponse_par_defaut(&quoi);
            poster(&p, &DeLaPage::Reponse { id, quoi });
        }
        VersPage::Panne(r) => sur_panne(&mut p, &r),
        VersPage::Accueil {
            salle,
            fragment,
            adresses,
        } => {
            let json = format!(
                "{{\"quoi\":\"accueil\",\"salle\":\"{salle}\",\"fragment\":{},\"adresses\":{}}}",
                chaine(fragment),
                adresses.len()
            );
            banc(&p, &json);
        }
        VersPage::Cle(g) => {
            if p.banc {
                banc(&p, &format!("{{\"quoi\":\"cle\",\"g\":\"{}\"}}", hexa(&g)));
            } else {
                exporter_la_cle(&p, &g);
            }
        }
        VersPage::Banc(json) => banc(&p, json),
        VersPage::EtatSalle(e) => {
            let json = format!(
                "{{\"quoi\":\"salle\",\"appareils\":{},\"garde\":{}}}",
                e.appareils.len(),
                e.garde
                    .map_or_else(|| String::from("null"), |n| n.to_string())
            );
            banc(&p, &json);
        }
        VersPage::Battement { .. }
        | VersPage::EnCours(_)
        | VersPage::Chargement(_)
        | VersPage::Traces(_) => {}
    }
}

/// Une panne que le Worker dit : le bandeau la montre ; « Remplacée » arrête le chien de
/// garde et propose de reprendre ici (P36, SAL-21).
fn sur_panne(p: &mut Page, r: &salle::ecran::RapportPanne<'_>) {
    let json = format!(
        "{{\"quoi\":\"panne\",\"sorte\":\"{:?}\",\"detail\":{}}}",
        r.quoi,
        chaine(r.detail)
    );
    banc(p, &json);
    match r.quoi {
        PanneSalle::Relais(liaison::RefusRelais::Remplacee) => {
            p.proposition = Proposition::ReprendreIci;
            p.chien.arrete();
            dire(
                p,
                Some("Cette salle est maintenant ouverte dans un autre onglet — Reprendre ici"),
            );
        }
        PanneSalle::Relais(liaison::RefusRelais::PreuveFausse) => {
            p.chien.arrete();
            dire(
                p,
                Some(
                    "Cette salle a été ouverte dans un autre navigateur, ou ses données ont été effacées : soiree-relais oublier <SALLE>",
                ),
            );
        }
        PanneSalle::Relais(_) | PanneSalle::Garde | PanneSalle::Outil => {
            dire(p, Some(r.detail));
        }
    }
}

/// G, dans un fichier que l'hôte enregistre, sur sa commande seulement (§ 2.4).
fn exporter_la_cle(p: &Page, g: &[u8; 32]) {
    let contenu = Array::of1(&JsValue::from_str(&hexa(g)));
    let Ok(blob) = web_sys::Blob::new_with_str_sequence(&contenu) else {
        return;
    };
    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };
    if let Ok(a) = p.document.create_element("a") {
        let _: Result<(), JsValue> = a.set_attribute("href", &url);
        let _: Result<(), JsValue> = a.set_attribute("download", "cle-de-garde.txt");
        if let Some(h) = a.dyn_ref::<HtmlElement>() {
            h.click();
        }
    }
    let _: Result<(), JsValue> = web_sys::Url::revoke_object_url(&url);
}

/// Les scènes et les ressources passent à l'écran une à une, dans l'ordre : décoder une
/// image est asynchrone.
fn traiter_la_file(etat: &Etat) {
    let mut p = etat.borrow_mut();
    if p.dessin.occupe || p.dessin.file.is_empty() {
        return;
    }
    p.dessin.occupe = true;
    drop(p);
    let e = Rc::clone(etat);
    spawn_local(async move {
        loop {
            let (suivant, ecran) = {
                let mut p = e.borrow_mut();
                let Some(o) = p.dessin.file.pop_front() else {
                    p.dessin.occupe = false;
                    return;
                };
                (o, p.ecran.take())
            };
            let Some(mut ecran) = ecran else {
                e.borrow_mut().dessin.occupe = false;
                return;
            };
            if let Ok(VersPage::Jeu { forme, .. }) = VersPage::decoder(&suivant) {
                ecran.nouveau_jeu(forme);
                e.borrow_mut().ecran = Some(ecran);
                continue;
            }
            let r = ecran.recevoir(&suivant).await;
            let mut p = e.borrow_mut();
            p.ecran = Some(ecran);
            match r {
                Ok(crate::messages::Recu::Scene { .. }) => p.dessin.sale = true,
                Ok(_) => {}
                Err(x) => web_sys::console::error_1(&JsValue::from_str(&format!("la télé : {x}"))),
            }
        }
    });
}

/// Le signal de chaque image au Worker (P6), et le dessin quand la scène a changé.
fn image(etat: &Etat) {
    let suivante: ChaqueImage = Rc::new(RefCell::new(None));
    let s = Rc::clone(&suivante);
    let e = Rc::clone(etat);
    *suivante.borrow_mut() = Some(Closure::new(move |_: f64| {
        let mut p = e.borrow_mut();
        let t_us = heure_us(&p.fenetre);
        poster(
            &p,
            &DeLaPage::Image {
                t_us,
                periode_us: 16_667,
            },
        );
        let maintenant = maintenant_ms();
        let du = p.dessin.sale || maintenant - p.dessin.dernier_ms >= REDESSIN_MS;
        if du && let Some(ecran) = p.ecran.as_mut() {
            if ecran.scene().is_some()
                && let Err(x) = ecran.dessiner()
            {
                web_sys::console::error_1(&JsValue::from_str(&format!("la télé : {x}")));
            }
            p.dessin.sale = false;
            p.dessin.dernier_ms = maintenant;
        }
        if let Some(f) = s.borrow().as_ref() {
            let _: Result<i32, JsValue> = p
                .fenetre
                .request_animation_frame(f.as_ref().unchecked_ref());
        }
    }));
    let p = etat.borrow();
    if let Some(f) = suivante.borrow().as_ref() {
        let _: Result<i32, JsValue> = p
            .fenetre
            .request_animation_frame(f.as_ref().unchecked_ref());
    }
}

/// Le chien de garde, sur les minuteries de la page (REM-02).
fn battre(etat: &Etat) -> Result<(), String> {
    let e = Rc::clone(etat);
    let tic = Closure::<dyn FnMut()>::new(move || {
        let mut p = e.borrow_mut();
        let t = maintenant_ms() as u64;
        match p.chien.tic(t) {
            Decision::Rien => {}
            Decision::Relancer {
                garde_fou_ms,
                ecarter,
            } => {
                // Un arrêt mural : une panne de l'outil, jamais une faute du jeu.
                p.relances = p.relances.saturating_add(1);
                p.ecarter = ecarter;
                let json = format!(
                    "{{\"quoi\":\"relance\",\"garde_fou_ms\":{garde_fou_ms},\"ecarter\":{}}}",
                    ecarter
                        .map_or_else(|| String::from("null"), |g| format!("\"{:016x}\"", g.fil()))
                );
                banc(&p, &json);
                drop(p);
                lancer_le_worker(&e);
            }
            Decision::Panne { garde_fou_ms } => {
                if let Some(w) = p.worker.take() {
                    w.terminate();
                }
                dire(&p, Some("Le commun est en panne — Reprendre"));
                banc(
                    &p,
                    &format!("{{\"quoi\":\"commun-en-panne\",\"garde_fou_ms\":{garde_fou_ms}}}"),
                );
            }
        }
    });
    let i32_ms = i32::try_from(TIC_MS).unwrap_or(100);
    etat.borrow()
        .fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            tic.as_ref().unchecked_ref(),
            i32_ms,
        )
        .map_err(|e| texte_d_erreur(&e))?;
    tic.forget();
    Ok(())
}

/// F8 : une marque au journal (SAL-162), par la vraie chaîne du clavier.
fn ecouter_le_clavier(etat: &Etat) -> Result<(), String> {
    let e = Rc::clone(etat);
    let touche = Closure::<dyn FnMut(KeyboardEvent)>::new(move |k: KeyboardEvent| {
        if k.code() == "F8" && !k.repeat() {
            k.prevent_default();
            let p = e.borrow();
            poster(&p, &DeLaPage::Commande(CommandeEcran::Marque));
            banc(&p, "{\"quoi\":\"marque\"}");
        }
    });
    etat.borrow()
        .document
        .add_event_listener_with_callback("keydown", touche.as_ref().unchecked_ref())
        .map_err(|e| texte_d_erreur(&e))?;
    touche.forget();
    Ok(())
}

/// La page s'en va (rechargée, fermée) : elle tue son Worker elle-même, et ne lit plus
/// rien de lui. Sans cela, Firefox ferme les WebSocket du Worker avant de le tuer, et le
/// Worker dirait à une page qui part que son lien est perdu.
fn quitter(etat: &Etat) -> Result<(), String> {
    let e = Rc::clone(etat);
    let part = Closure::<dyn FnMut()>::new(move || {
        let mut p = e.borrow_mut();
        p.generation = p.generation.saturating_add(1);
        p.chien.arrete();
        if let Some(w) = p.worker.take() {
            w.terminate();
        }
    });
    // `beforeunload` vient avant que Firefox ne ferme les WebSocket ; `pagehide`, s'il
    // n'est pas venu (une page restaurée puis quittée).
    for evenement in ["beforeunload", "pagehide"] {
        etat.borrow()
            .fenetre
            .add_event_listener_with_callback(evenement, part.as_ref().unchecked_ref())
            .map_err(|e| texte_d_erreur(&e))?;
    }
    part.forget();
    Ok(())
}

/// « Reprendre ici » (Remplacée), ou « Reprendre » (le commun en panne) : un Worker neuf.
fn reprendre_ici(etat: &Etat) {
    {
        let mut p = etat.borrow_mut();
        p.proposition = Proposition::Rien;
        p.chien = Chien::new();
        dire(&p, None);
    }
    lancer_le_worker(etat);
}

/// Les crochets du banc, sur `window.__banc` (mode banc seulement).
fn crochets(etat: &Etat) -> Result<(), String> {
    let o = Object::new();
    let poser = |nom: &str, f: &JsValue| {
        Reflect::set(&o, &JsValue::from_str(nom), f)
            .map(|_| ())
            .map_err(|e| texte_d_erreur(&e))
    };
    let e = Rc::clone(etat);
    let tuer = Closure::<dyn FnMut()>::new(move || {
        // Le Worker tué comme le chien de garde le tue, et relancé de même.
        {
            let p = e.borrow();
            banc(&p, "{\"quoi\":\"tue\"}");
        }
        lancer_le_worker(&e);
    });
    poser("tuer", tuer.as_ref())?;
    tuer.forget();
    let e = Rc::clone(etat);
    let figer = Closure::<dyn FnMut()>::new(move || {
        poster(&e.borrow(), &DeLaPage::Commande(CommandeEcran::Figer));
    });
    poser("figer", figer.as_ref())?;
    figer.forget();
    let e = Rc::clone(etat);
    let retarder = Closure::<dyn FnMut(u32, u32)>::new(move |ms: u32, fois: u32| {
        poster(
            &e.borrow(),
            &DeLaPage::Commande(CommandeEcran::Retarder { ms, fois }),
        );
    });
    poser("retarder", retarder.as_ref())?;
    retarder.forget();
    let e = Rc::clone(etat);
    let cle = Closure::<dyn FnMut()>::new(move || {
        poster(
            &e.borrow(),
            &DeLaPage::Commande(CommandeEcran::ExporterLaCle),
        );
    });
    poser("exporter_la_cle", cle.as_ref())?;
    cle.forget();
    let e = Rc::clone(etat);
    let ici = Closure::<dyn FnMut()>::new(move || reprendre_ici(&e));
    poser("reprendre_ici", ici.as_ref())?;
    ici.forget();
    let e = Rc::clone(etat);
    let pas = Closure::<dyn FnMut() -> f64>::new(move || {
        e.borrow().ecran.as_ref().map_or(0.0, |x| {
            f64::from(u32::try_from(x.pas().numero()).unwrap_or(u32::MAX))
        })
    });
    poser("pas", pas.as_ref())?;
    pas.forget();
    let p = etat.borrow();
    Reflect::set(&p.fenetre, &JsValue::from_str("__banc"), &o)
        .map(|_| ())
        .map_err(|e| texte_d_erreur(&e))
}

/// Un clic sur le bandeau, quand il propose de reprendre (« Remplacée — Reprendre ici »,
/// « Le commun est en panne — Reprendre ») : un Worker neuf.
fn ecouter_le_bandeau(etat: &Etat) -> Result<(), String> {
    let e = Rc::clone(etat);
    let clic = Closure::<dyn FnMut()>::new(move || {
        let reprendre = {
            let p = e.borrow();
            p.proposition == Proposition::ReprendreIci || p.chien.en_panne()
        };
        if reprendre {
            reprendre_ici(&e);
        }
    });
    etat.borrow()
        .bandeau
        .add_event_listener_with_callback("click", clic.as_ref().unchecked_ref())
        .map_err(|e| texte_d_erreur(&e))?;
    clic.forget();
    Ok(())
}
