//! La page du téléphone dans le navigateur (PHASE-3, § 2.10 ; lot 12).
//!
//! Tout passe par un seul état (`Tel`), que les événements empruntent un à un : un
//! événement qui arrive pendant qu'un autre est traité (un `blur` déclenché par un nœud
//! retiré, par exemple) est ignoré plutôt que de paniquer. Les événements du DOM sont
//! **délégués** à la racine de la page : chaque élément porte sa clé (`data-cle`), et
//! aucun écouteur ne vit sur un élément du jeu. Aucun style ne s'écrit en ligne dans le
//! HTML : la politique de contenu du relais (`default-src 'self'`) le refuserait ; tout
//! passe par le CSSOM (`style.setProperty`), qu'elle permet.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::disallowed_methods,
    reason = "la coque du téléphone : l'heure, les tailles et les positions du navigateur (des f64) ramenées en entiers ou comparées, et la direction d'un doigt sur la croix ; rien ne revient à une partie sans passer par la page du jeu, qui ne reçoit que des locaux entiers"
)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::rc::Rc;

use jeu_abi::{
    Absence, ApplicableFil, Capacite, Capacites, Cle, CompteAReboursFil, Declencheur, Direction8,
    Directions, EcranTelephone, Marges, MontreFil, Orientation, Sensation, SorteNoeudPage,
    Vibration, decoder,
};
use moteur_navigateur::FabriqueNavigateur;
use salle::appareil::{Appareil, Envoi};
use salle::chiffre::Hasard;
use salle::page::{Action, InstanceDePage, Locaux, PageDuTelephone, Recalcul};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{
    BinaryType, Document, Element, HtmlElement, HtmlInputElement, KeyboardEvent, MessageEvent,
    PointerEvent, Storage, WebSocket, Window,
};

use crate::adresse;
use crate::ecrans::{self, Ecran};
use crate::{retenu, style};

/// Le réveil de la page : l'écho, la redite du tenu, la coupure, les comptes à rebours.
const TIC_MS: i32 = 50;
/// SAL-47 : la reconnexion part de 250 ms et double jusqu'à 8 s, avec du hasard.
const RECONNEXION_MIN_MS: f64 = 250.0;
const RECONNEXION_MAX_MS: f64 = 8_000.0;
/// Relié au relais sans accueil pendant 5 s, la page reconnecte (choisi : au-delà d'une
/// poignée de main et d'un bonjour redit, `salle::appareil::BONJOUR_REDIT_US`).
const SANS_ACCUEIL_MS: f64 = 5_000.0;
/// Les identités d'effet déjà jouées, gardées au plus (P26, SAL-167 ; choisi).
const EFFETS_GARDES: usize = 256;
/// Un balayage de carrousel, au moins (choisi).
const BALAYAGE_PX: f64 = 40.0;

const CLE_JETON: &str = "soiree-jeton";
const CLE_ONGLET: &str = "soiree-onglet";
const CLE_FILE: &str = "soiree-file";
const CLE_EFFETS: &str = "soiree-effets";

type Pagee = PageDuTelephone<InstanceDePage<FabriqueNavigateur>>;

/// Le hasard du navigateur : `crypto.getRandomValues` (SAL-93), qui existe aussi hors
/// contexte sécurisé.
struct HasardNavigateur {
    crypto: Option<web_sys::Crypto>,
}

impl Hasard for HasardNavigateur {
    fn remplir(&mut self, octets: &mut [u8]) {
        for morceau in octets.chunks_mut(65_536) {
            let tire = self
                .crypto
                .as_ref()
                .is_some_and(|c| c.get_random_values_with_u8_array(morceau).is_ok());
            if !tire {
                // Jamais constaté : `getRandomValues` existe partout. On tire alors du
                // hasard faible plutôt que des zéros, et le jeton reste propre au téléphone.
                for o in morceau.iter_mut() {
                    *o = u8::try_from((js_sys::Math::random() * 256.0) as u32 & 0xff).unwrap_or(0);
                }
            }
        }
    }
}

/// Les sortes d'éléments, pour réconcilier par clé et par sorte.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Sorte {
    Colonne,
    Rangee,
    Grille,
    Pile,
    Texte,
    Image,
    Compte,
    Bouton,
    Maintenu,
    Croix,
    Champ,
    Choix,
}

impl Sorte {
    const fn de(s: &SorteNoeudPage) -> Sorte {
        match s {
            SorteNoeudPage::Colonne => Sorte::Colonne,
            SorteNoeudPage::Rangee => Sorte::Rangee,
            SorteNoeudPage::Grille { .. } => Sorte::Grille,
            SorteNoeudPage::Pile => Sorte::Pile,
            SorteNoeudPage::Texte { .. } => Sorte::Texte,
            SorteNoeudPage::Image { .. } => Sorte::Image,
            SorteNoeudPage::CompteARebours { .. } => Sorte::Compte,
            SorteNoeudPage::Bouton { .. } => Sorte::Bouton,
            SorteNoeudPage::Maintenu { .. } => Sorte::Maintenu,
            SorteNoeudPage::Croix { .. } => Sorte::Croix,
            SorteNoeudPage::Champ { .. } => Sorte::Champ,
            SorteNoeudPage::Choix { .. } => Sorte::Choix,
        }
    }

    const fn nom(self) -> &'static str {
        match self {
            Sorte::Colonne => "colonne",
            Sorte::Rangee => "rangee",
            Sorte::Grille => "grille",
            Sorte::Pile => "pile",
            Sorte::Texte => "texte",
            Sorte::Image => "image",
            Sorte::Compte => "compte",
            Sorte::Bouton => "bouton",
            Sorte::Maintenu => "maintenu",
            Sorte::Croix => "croix",
            Sorte::Champ => "champ",
            Sorte::Choix => "choix",
        }
    }

    const fn balise(self) -> &'static str {
        match self {
            Sorte::Bouton | Sorte::Maintenu => "button",
            Sorte::Champ => "input",
            Sorte::Image => "img",
            Sorte::Colonne
            | Sorte::Rangee
            | Sorte::Grille
            | Sorte::Pile
            | Sorte::Texte
            | Sorte::Compte
            | Sorte::Croix
            | Sorte::Choix => "div",
        }
    }
}

/// L'état de la page.
#[expect(
    clippy::struct_excessive_bools,
    reason = "des drapeaux indépendants de la coque"
)]
struct Tel {
    fenetre: Window,
    document: Document,
    racine: HtmlElement,
    voile: HtmlElement,
    voile_texte: HtmlElement,
    reprendre: HtmlElement,
    bandeau: HtmlElement,
    bandeau_jusqu_a: f64,
    adresse_illisible: bool,
    appareil: Option<Appareil<HasardNavigateur>>,
    page: Option<Pagee>,
    panne_module: Option<String>,
    /// L'empreinte du module dont la page est faite : un autre module reçu (la salle a
    /// changé de jeu, lot 12, fin) refait la page.
    module_de_la_page: Option<[u8; 32]>,
    /// Les pages refaites sur un autre module.
    changements: u32,
    /// La clé de `localStorage` du retenu de cette salle et de ce jeu, et ce qui y est
    /// gardé (lot 12, fin : `Locaux::retenu` survit au rechargement).
    retenu: Option<(String, Option<Vec<u8>>)>,
    /// Ce que `localStorage` a rendu du retenu quand la page s'est faite sur son module,
    /// avant toute vue : ce qu'une page rechargée tient du stockage seul, et que le banc
    /// compare à ce qu'elle avait retenu avant (correction de la fin du lot 12 : une page
    /// qui recalcule son retenu depuis la vue le retrouverait sans stockage).
    retenu_repris: Option<Vec<u8>>,
    ws: Option<WebSocket>,
    ws_ouvert: bool,
    /// La dernière fois que la page était accueillie, ou que sa WebSocket s'est ouverte.
    ouvert_depuis_ms: f64,
    generation: u32,
    prochaine_tentative_ms: Option<f64>,
    delai_ms: f64,
    elements: BTreeMap<Cle, (Sorte, HtmlElement)>,
    signatures: BTreeMap<Cle, String>,
    doigts: BTreeMap<i32, Cle>,
    croix: BTreeMap<i32, Cle>,
    ignores: BTreeSet<i32>,
    balayage: Option<(i32, Cle, f64)>,
    compose: bool,
    images: BTreeMap<u64, String>,
    ressources_vues: usize,
    comptes_a_rebours: Vec<(HtmlElement, CompteAReboursFil)>,
    vue: Option<(u64, f64)>,
    effets_joues: Vec<u64>,
    file_gardee: Vec<u8>,
    banc: bool,
    m4: Option<(usize, f64)>,
    files_pleines: u64,
    gestes: u64,
    tenus: u64,
    ecran: Option<Ecran>,
    a_poser: bool,
    capacites: Capacites,
    /// Le banc : jusqu'à quand le réseau de la page est gelé (`__banc.geler`), et la page
    /// d'un texte qui déborde (`__banc.deborder`, le contrôle négatif du scénario `pages`).
    gele_jusqu_a: f64,
    deborde: bool,
    /// Le banc : la page de pire cas que le banc pose (scénario `pages`, lot 12, fin), sans
    /// salle ni relais ; l'écran du commun ne la couvre pas.
    pire_cas: bool,
    /// Le banc : les octets que `__banc.morceau` a reçus, en attente de `__banc.pire_cas`.
    morceaux: Vec<u8>,
    /// Le banc : le pas de la dernière vue de pire cas posée.
    pas_du_pire_cas: u64,
    /// Le banc : ce que la page a envoyé pendant un gel, retenu et non perdu (une
    /// WebSocket ouverte ne perd rien : TCP retient puis livre), parti au dégel sur la même
    /// connexion ; une connexion neuve l'oublie (il appartient à l'ancienne session).
    /// *Correction du lot 12* : au lot 12, ce qui partait pendant un gel se perdait, la
    /// connexion restant ouverte ; une file pleine provoquée sous un gel plus court que
    /// la coupure (P28) ne repartait alors jamais.
    retenues: Vec<Vec<u8>>,
}

type Etat = Rc<RefCell<Tel>>;

fn maintenant_ms(f: &Window) -> f64 {
    f.performance().map_or(0.0, |p| p.now())
}

fn maintenant_us(f: &Window) -> u64 {
    let ms = maintenant_ms(f);
    if ms > 0.0 { (ms * 1000.0) as u64 } else { 0 }
}

fn hexa(o: &[u8]) -> String {
    o.iter().fold(String::new(), |mut s, b| {
        let _: core::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

/// Un appel du DOM dont l'échec ne change rien à la page (un attribut, un enfant).
fn ignorer<T>(_: Result<T, JsValue>) {}

fn lire_hexa(t: &str) -> Option<Vec<u8>> {
    if !t.len().is_multiple_of(2) {
        return None;
    }
    (0..t.len())
        .step_by(2)
        .map(|i| {
            t.get(i..i.saturating_add(2))
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        })
        .collect()
}

fn poser_style(e: &HtmlElement, d: &[(&str, &str)]) {
    let s = e.style();
    for (k, v) in d {
        let _: Result<(), JsValue> = s.set_property(k, v);
    }
}

fn creer(document: &Document, balise: &str) -> Result<HtmlElement, String> {
    document
        .create_element(balise)
        .map_err(|_| format!("créer <{balise}>"))?
        .dyn_into::<HtmlElement>()
        .map_err(|_| String::from("pas un élément HTML"))
}

fn stockage_local(f: &Window) -> Option<Storage> {
    f.local_storage().ok().flatten()
}

fn stockage_de_session(f: &Window) -> Option<Storage> {
    f.session_storage().ok().flatten()
}

/// Le jeton de l'appareil : 32 octets tirés une fois, gardés dans `localStorage`. Un
/// stockage cassé donne un appareil neuf, jamais une page qui ne s'ouvre pas (SAL-32).
fn le_jeton(f: &Window, h: &mut HasardNavigateur) -> [u8; 32] {
    let s = stockage_local(f);
    if let Some(v) = s
        .as_ref()
        .and_then(|s| s.get_item(CLE_JETON).ok().flatten())
        && let Some(o) = lire_hexa(&v)
        && let Ok(j) = <[u8; 32]>::try_from(o.as_slice())
    {
        return j;
    }
    let mut j = [0u8; 32];
    h.remplir(&mut j);
    if let Some(s) = s {
        let _: Result<(), JsValue> = s.set_item(CLE_JETON, &hexa(&j));
    }
    j
}

/// L'onglet : gardé dans `sessionStorage`, il survit à un rechargement (P26) ; un onglet
/// neuf (un jumeau) en tire un autre.
fn l_onglet(f: &Window, h: &mut HasardNavigateur) -> u64 {
    let session = stockage_de_session(f);
    if let Some(v) = session
        .as_ref()
        .and_then(|s| s.get_item(CLE_ONGLET).ok().flatten())
        && let Ok(n) = u64::from_str_radix(&v, 16)
    {
        return n;
    }
    let mut o = [0u8; 8];
    h.remplir(&mut o);
    let n = u64::from_le_bytes(o).max(1);
    if let Some(s) = session {
        let _: Result<(), JsValue> = s.set_item(CLE_ONGLET, &format!("{n:x}"));
    }
    n
}

/// Les capacités, dites honnêtement : hors contexte sécurisé, l'écran allumé, le
/// mouvement et le micro sont `Non(ContexteNonSecurise)` (SAL-97, OUT-211) ; la vibration
/// là où `navigator.vibrate` existe (REM-79).
fn capacites(f: &Window) -> Capacites {
    let sur = f.is_secure_context();
    let vibre =
        js_sys::Reflect::has(&f.navigator(), &JsValue::from_str("vibrate")).unwrap_or(false);
    let non_sur = Capacite::Non(Absence::ContexteNonSecurise);
    Capacites {
        vibration: if vibre {
            Capacite::Oui
        } else {
            Capacite::Non(Absence::NonSupportee)
        },
        mouvement: if sur {
            Capacite::Non(Absence::PasEncore)
        } else {
            non_sur
        },
        son_sans_geste: Capacite::Non(Absence::PasEncore),
        ecran_allume: if sur {
            Capacite::Non(Absence::PasEncore)
        } else {
            non_sur
        },
        micro: if sur {
            Capacite::Non(Absence::PasEncore)
        } else {
            non_sur
        },
        pad_natif: Capacite::Non(Absence::NonSupportee),
        orientation: orientation(f),
        prenom_propose: None,
    }
}

fn orientation(f: &Window) -> Orientation {
    let t = f
        .screen()
        .ok()
        .and_then(|s| s.orientation().type_().ok())
        .map(|t| format!("{t:?}"))
        .unwrap_or_default();
    if t.contains("PortraitSecondary") {
        Orientation::PortraitInverse
    } else if t.contains("LandscapePrimary") {
        Orientation::PaysageGauche
    } else if t.contains("LandscapeSecondary") {
        Orientation::PaysageDroite
    } else {
        Orientation::Portrait
    }
}

/// L'écran : `visualViewport` (le clavier réduit la hauteur visible), les marges sûres
/// (`env(safe-area-inset-*)`, lues sur le corps qui les applique), l'orientation.
fn l_ecran(f: &Window, document: &Document) -> EcranTelephone {
    let (l, h) = f.visual_viewport().map_or_else(
        || {
            (
                f.inner_width()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(360.0),
                f.inner_height()
                    .ok()
                    .and_then(|v| v.as_f64())
                    .unwrap_or(640.0),
            )
        },
        |v| (v.width(), v.height()),
    );
    let marge = |cote: &str| -> u16 {
        document
            .body()
            .and_then(|b| f.get_computed_style(&b).ok().flatten())
            .and_then(|s| s.get_property_value(&format!("padding-{cote}")).ok())
            .and_then(|v| v.trim_end_matches("px").parse::<f64>().ok())
            .map_or(0, |v| v.clamp(0.0, 1000.0) as u16)
    };
    EcranTelephone {
        largeur: l.clamp(1.0, 10_000.0) as u16,
        hauteur: h.clamp(1.0, 10_000.0) as u16,
        marges: Marges {
            haut: marge("top"),
            droite: marge("right"),
            bas: marge("bottom"),
            gauche: marge("left"),
        },
        orientation: orientation(f),
    }
}

/// Le point d'entrée, que `demarrage.js` appelle sur `/s/<SALLE>`.
#[wasm_bindgen]
pub fn demarrer_le_telephone() {
    if let Err(e) = demarrer() {
        web_sys::console::error_1(&JsValue::from_str(&format!("le téléphone : {e}")));
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "la page construite d'un bloc, dans l'ordre où elle s'affiche"
)]
fn demarrer() -> Result<(), String> {
    let fenetre = web_sys::window().ok_or("pas de fenêtre")?;
    let document = fenetre.document().ok_or("pas de document")?;
    let corps = document.body().ok_or("pas de corps")?;
    corps.set_text_content(None);
    poser_style(
        &corps,
        &[
            ("margin", "0"),
            (
                "padding",
                "env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left)",
            ),
            ("background", "#111"),
            ("color", "#eee"),
            ("font-family", style::POLICE_DU_COMMUN),
            ("touch-action", "manipulation"),
            ("-webkit-user-select", "none"),
            ("user-select", "none"),
            ("overflow-x", "hidden"),
        ],
    );
    let racine = creer(&document, "div")?;
    racine.set_id("page");
    poser_style(
        &racine,
        &[
            ("display", "flex"),
            ("flex-direction", "column"),
            ("gap", "8px"),
            ("padding", "8px"),
            ("box-sizing", "border-box"),
            ("min-height", "100vh"),
        ],
    );
    let voile = creer(&document, "div")?;
    poser_style(
        &voile,
        &[
            ("position", "fixed"),
            ("inset", "0"),
            ("display", "flex"),
            ("flex-direction", "column"),
            ("align-items", "center"),
            ("justify-content", "center"),
            ("gap", "16px"),
            ("background", "#111"),
            ("font-size", "22px"),
            ("text-align", "center"),
            ("padding", "24px"),
        ],
    );
    let voile_texte = creer(&document, "div")?;
    let reprendre = creer(&document, "button")?;
    reprendre.set_text_content(Some("Reprendre ici"));
    ignorer(reprendre.set_attribute("data-commun", "reprendre"));
    poser_style(
        &reprendre,
        &[
            ("font-size", "20px"),
            ("padding", "12px 20px"),
            ("display", "none"),
        ],
    );
    ignorer(voile.append_child(&voile_texte));
    ignorer(voile.append_child(&reprendre));
    let bandeau = creer(&document, "div")?;
    poser_style(
        &bandeau,
        &[
            ("position", "fixed"),
            ("left", "0"),
            ("right", "0"),
            ("bottom", "0"),
            ("padding", "8px"),
            ("background", "#a33"),
            ("color", "#fff"),
            ("display", "none"),
            ("text-align", "center"),
        ],
    );
    ignorer(corps.append_child(&racine));
    ignorer(corps.append_child(&voile));
    ignorer(corps.append_child(&bandeau));
    let banc = document
        .query_selector("meta[name=soiree-banc]")
        .ok()
        .flatten()
        .and_then(|m| m.get_attribute("content"))
        .is_some_and(|c| c == "oui");
    let location = fenetre.location();
    let chemin = location.pathname().unwrap_or_default();
    let fragment = location.hash().unwrap_or_default();
    let mut hasard = HasardNavigateur {
        crypto: fenetre.crypto().ok(),
    };
    let caps = capacites(&fenetre);
    let (appareil, adresse_illisible) = match adresse::lire(&chemin, &fragment) {
        Ok((salle, f)) => {
            let jeton = le_jeton(&fenetre, &mut hasard);
            let onglet = l_onglet(&fenetre, &mut hasard);
            let mut a = Appareil::new(f, salle, (jeton, onglet), caps, None, hasard);
            a.taire_le_banc();
            if let Some(o) = stockage_de_session(&fenetre)
                .and_then(|s| s.get_item(CLE_FILE).ok().flatten())
                .and_then(|v| lire_hexa(&v))
            {
                let _: bool = a.reprendre_la_file(&o);
            }
            (Some(a), false)
        }
        Err(_) => (None, true),
    };
    let effets_joues: Vec<u64> = stockage_de_session(&fenetre)
        .and_then(|s| s.get_item(CLE_EFFETS).ok().flatten())
        .map(|v| {
            v.split(',')
                .filter_map(|x| u64::from_str_radix(x, 16).ok())
                .collect()
        })
        .unwrap_or_default();
    let etat: Etat = Rc::new(RefCell::new(Tel {
        fenetre: fenetre.clone(),
        document: document.clone(),
        racine: racine.clone(),
        voile,
        voile_texte,
        reprendre,
        bandeau,
        bandeau_jusqu_a: 0.0,
        adresse_illisible,
        appareil,
        page: None,
        panne_module: None,
        module_de_la_page: None,
        changements: 0,
        retenu: None,
        retenu_repris: None,
        ws: None,
        ws_ouvert: false,
        ouvert_depuis_ms: 0.0,
        generation: 0,
        prochaine_tentative_ms: None,
        delai_ms: RECONNEXION_MIN_MS,
        elements: BTreeMap::new(),
        signatures: BTreeMap::new(),
        doigts: BTreeMap::new(),
        croix: BTreeMap::new(),
        ignores: BTreeSet::new(),
        balayage: None,
        compose: false,
        images: BTreeMap::new(),
        ressources_vues: 0,
        comptes_a_rebours: Vec::new(),
        vue: None,
        effets_joues,
        file_gardee: Vec::new(),
        banc,
        m4: None,
        files_pleines: 0,
        gestes: 0,
        tenus: 0,
        ecran: None,
        a_poser: false,
        capacites: caps,
        gele_jusqu_a: 0.0,
        retenues: Vec::new(),
        deborde: false,
        pire_cas: false,
        morceaux: Vec::new(),
        pas_du_pire_cas: 0,
    }));
    ecouter(&etat, &corps)?;
    if banc {
        crochets_du_banc(&etat)?;
    }
    {
        let mut t = etat.borrow_mut();
        montrer_l_ecran(&mut t);
    }
    relier(&etat);
    let pour_tic = Rc::clone(&etat);
    let tic = Closure::<dyn FnMut()>::new(move || tic(&pour_tic));
    fenetre
        .set_interval_with_callback_and_timeout_and_arguments_0(
            tic.as_ref().unchecked_ref(),
            TIC_MS,
        )
        .map_err(|_| "setInterval")?;
    tic.forget();
    Ok(())
}

// ---------------------------------------------------------------------------
// La connexion
// ---------------------------------------------------------------------------

/// Ouvre une WebSocket neuve ; les tentatives lancées avant restent sans effet (SAL-50).
fn relier(etat: &Etat) {
    let Ok(mut t) = etat.try_borrow_mut() else {
        return;
    };
    if t.appareil.as_ref().is_none_or(Appareil::fini) {
        return;
    }
    if let Some(ws) = t.ws.take() {
        let _: Result<(), JsValue> = ws.close();
    }
    t.generation = t.generation.wrapping_add(1);
    t.retenues.clear();
    t.ws_ouvert = false;
    t.prochaine_tentative_ms = None;
    if let Some(a) = t.appareil.as_mut() {
        a.couper();
    }
    let generation = t.generation;
    let hote = t.fenetre.location().host().unwrap_or_default();
    let Ok(ws) = WebSocket::new(&format!("ws://{hote}/ws")) else {
        let now = maintenant_ms(&t.fenetre);
        programmer(&mut t, now);
        return;
    };
    ws.set_binary_type(BinaryType::Arraybuffer);
    let e = Rc::clone(etat);
    let ouvert = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        let Ok(mut t) = e.try_borrow_mut() else {
            return;
        };
        if t.generation != generation {
            return;
        }
        t.ws_ouvert = true;
        t.ouvert_depuis_ms = maintenant_ms(&t.fenetre);
        t.delai_ms = RECONNEXION_MIN_MS;
        if let Some(a) = t.appareil.as_mut() {
            a.relier();
        }
        vider(&mut t);
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
        let octets = js_sys::Uint8Array::new(&tampon).to_vec();
        if maintenant_ms(&t.fenetre) < t.gele_jusqu_a {
            // Le banc gèle le réseau : ce qui arrive se perd, comme sur un Wi-Fi muet.
            return;
        }
        let now = maintenant_us(&t.fenetre);
        if let Some(a) = t.appareil.as_mut() {
            a.recevoir(now, &octets);
        }
        apres(&mut t);
    });
    ws.set_onmessage(Some(recu.as_ref().unchecked_ref()));
    recu.forget();
    let e = Rc::clone(etat);
    let ferme = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        let Ok(mut t) = e.try_borrow_mut() else {
            return;
        };
        if t.generation != generation {
            return;
        }
        t.ws_ouvert = false;
        t.ws = None;
        if let Some(a) = t.appareil.as_mut() {
            a.couper();
        }
        // P28 : le lien perdu est une coupure (un tenu qui déclare une fraîcheur se
        // relâche côté page ; son relâchement part à la reconnexion).
        let now_us = maintenant_us(&t.fenetre);
        if let Some(p) = t.page.as_mut() {
            let r = p.coupure(now_us);
            appliquer(&mut t, &r);
        }
        let now = maintenant_ms(&t.fenetre);
        programmer(&mut t, now);
        montrer_l_ecran(&mut t);
    });
    ws.set_onclose(Some(ferme.as_ref().unchecked_ref()));
    ferme.forget();
    t.ws = Some(ws);
}

/// La prochaine tentative, à intervalle croissant avec du hasard (SAL-47) ; jamais
/// abandonnée tant que la page est visible (SAL-48) ; aucune après un refus définitif.
fn programmer(t: &mut Tel, now: f64) {
    let definitif = t.appareil.as_ref().is_none_or(|a| {
        a.fini()
            || matches!(
                a.refus_relais(),
                Some(liaison::RefusRelais::Protocole { .. } | liaison::RefusRelais::SalleInconnue)
            )
    });
    if definitif {
        return;
    }
    let hasard = js_sys::Math::random() * t.delai_ms;
    t.prochaine_tentative_ms = Some(now + t.delai_ms + hasard);
    t.delai_ms = (t.delai_ms * 2.0).min(RECONNEXION_MAX_MS);
}

/// Envoie au relais ce que l'appareil a à dire.
fn vider(t: &mut Tel) {
    let Some(a) = t.appareil.as_mut() else {
        return;
    };
    let sorties = a.sorties();
    if !t.ws_ouvert {
        return;
    }
    if maintenant_ms(&t.fenetre) < t.gele_jusqu_a {
        t.retenues.extend(sorties);
        return;
    }
    let retenues = core::mem::take(&mut t.retenues);
    if let Some(ws) = t.ws.as_ref() {
        for o in retenues.into_iter().chain(sorties) {
            let _: Result<(), JsValue> = ws.send_with_u8_array(&o);
        }
    }
}

// ---------------------------------------------------------------------------
// Après chaque message : le module, les ressources, la vue, les effets, l'écran
// ---------------------------------------------------------------------------

fn apres(t: &mut Tel) {
    charger_le_module(t);
    ressources(t);
    let now = maintenant_us(&t.fenetre);
    let vue = t
        .appareil
        .as_ref()
        .and_then(|a| a.vue().map(|(p, v)| (p, v.to_vec())));
    if let Some((pas, montre)) = vue {
        if t.vue.is_none_or(|(p, _)| p != pas.numero()) {
            let ms = maintenant_ms(&t.fenetre);
            t.vue = Some((pas.numero(), ms));
            effets(t, &montre);
        }
        if let Some(p) = t.page.as_mut() {
            let r = p.vue_recue(now, pas, &montre);
            appliquer(t, &r);
        }
    }
    if t.a_poser {
        poser(t);
    }
    garder_la_file(t);
    montrer_l_ecran(t);
    vider(t);
}

/// Le module reçu entier : réécrit et compilé sur la page (M4), puis `Bonjour` et
/// `Decrire` ; la page se pose dès qu'une vue est là. La période de redite du tenu vient
/// de sa déclaration (P27).
fn charger_le_module(t: &mut Tel) {
    // La salle a changé de jeu (§ 2.8, lot 12, fin) : le module de la page n'est plus
    // celui que l'appareil reçoit ou a reçu ; la page se refait sur le nouveau, ses
    // ressources reprises du début.
    let recu = t
        .appareil
        .as_ref()
        .and_then(Appareil::module)
        .map(|(e, _)| e);
    if t.module_de_la_page.is_some() && recu != t.module_de_la_page {
        t.page = None;
        t.panne_module = None;
        t.module_de_la_page = None;
        t.ressources_vues = 0;
        t.changements = t.changements.saturating_add(1);
        t.a_poser = true;
    }
    if t.page.is_some() || t.panne_module.is_some() {
        return;
    }
    let Some((empreinte, octets)) = t.appareil.as_ref().and_then(Appareil::module) else {
        return;
    };
    t.module_de_la_page = Some(empreinte);
    let octets = octets.to_vec();
    let debut = maintenant_ms(&t.fenetre);
    let charge = match moteur_navigateur::charger(&octets, &reecriture::Reglages::PHASE_3) {
        Ok(c) => c,
        Err(e) => {
            t.panne_module = Some(format!("le module est refusé : {e:?}"));
            return;
        }
    };
    let instance = match InstanceDePage::ouvrir(charge.fabrique()) {
        Ok(i) => i,
        Err(e) => {
            t.panne_module = Some(format!("l'instance de la page : {e}"));
            return;
        }
    };
    t.m4 = Some((octets.len(), maintenant_ms(&t.fenetre) - debut));
    let tenu = match &instance.declaration().telephones {
        ApplicableFil::Oui(tel) => Some(tel.tenu.clone()),
        ApplicableFil::SansObjet(_) => None,
    };
    let redite = tenu
        .as_ref()
        .and_then(liaison::periode_de_redite)
        .map(|d| d.en_ms().saturating_mul(1000));
    if let Some(a) = t.appareil.as_mut() {
        a.regler_la_redite(redite);
    }
    let ecran = l_ecran(&t.fenetre, &t.document);
    let mut page = PageDuTelephone::new(instance, Locaux::new(ecran, t.capacites), tenu.as_ref());
    // Ce que la page avait retenu pour cette salle et ce jeu, avant un rechargement.
    let cle = retenu::cle(
        &t.fenetre.location().pathname().unwrap_or_default(),
        &hexa(&empreinte),
    );
    let garde = stockage_local(&t.fenetre)
        .and_then(|s| s.get_item(&cle).ok().flatten())
        .and_then(|v| lire_hexa(&v));
    if let Some(o) = &garde {
        page.reprendre_le_retenu(o.clone());
    }
    t.retenu_repris.clone_from(&garde);
    t.retenu = Some((cle, garde));
    t.page = Some(page);
    if t.banc {
        let (n, ms) = t.m4.unwrap_or((0, 0.0));
        web_sys::console::log_1(&JsValue::from_str(&format!(
            "banc telephone {{\"m4\":{{\"octets\":{n},\"ms\":{ms:.3}}}}}"
        )));
    }
}

/// Les ressources de la page reçues : les polices par `FontFace`, les images en `Blob`.
fn ressources(t: &mut Tel) {
    let Some(a) = t.appareil.as_ref() else {
        return;
    };
    let nouvelles: Vec<(u64, Vec<u8>)> = a
        .ressources()
        .iter()
        .skip(t.ressources_vues)
        .cloned()
        .collect();
    if nouvelles.is_empty() {
        return;
    }
    t.ressources_vues = t.ressources_vues.saturating_add(nouvelles.len());
    for (cle, octets) in nouvelles {
        ajouter_une_ressource(t, cle, &octets);
    }
    t.a_poser = true;
}

/// Une ressource de la page : une police (sfnt) par `FontFace`, une image en `Blob`. Le
/// scénario `pages` du banc passe par ici aussi (`__banc.pire_cas`).
fn ajouter_une_ressource(t: &mut Tel, cle: u64, octets: &[u8]) {
    let tete = octets.get(..4).unwrap_or_default();
    let police = matches!(tete, [0, 1, 0, 0] | b"OTTO" | b"true");
    if police {
        if let Ok(ff) =
            web_sys::FontFace::new_with_u8_array(&style::famille(Cle::du_fil(cle)), octets)
        {
            let _: Result<(), JsValue> = t.document.fonts().add(&ff);
            let _: Result<js_sys::Promise, JsValue> = ff.load();
        }
        return;
    }
    let mime = if octets.starts_with(b"\x89PNG") {
        "image/png"
    } else if octets.starts_with(&[0xff, 0xd8]) {
        "image/jpeg"
    } else if octets.starts_with(b"GIF8") {
        "image/gif"
    } else if octets.starts_with(b"RIFF") {
        "image/webp"
    } else {
        "image/svg+xml"
    };
    let tableau = js_sys::Array::new();
    tableau.push(&js_sys::Uint8Array::from(octets));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(mime);
    if let Ok(b) = web_sys::Blob::new_with_u8_array_sequence_and_options(&tableau, &options)
        && let Ok(url) = web_sys::Url::create_object_url_with_blob(&b)
    {
        t.images.insert(cle, url);
    }
}

/// Les effets de la vue : la vibration, là où elle existe ; une identité ne se rejoue pas,
/// même après un rechargement (P26, SAL-167, PUN-66).
fn effets(t: &mut Tel, montre: &[u8]) {
    let Ok(m) = decoder::<MontreFil<'_>>(montre) else {
        return;
    };
    let mut change = false;
    for e in &m.effets {
        let id = e.id.fil();
        if t.effets_joues.contains(&id) {
            continue;
        }
        t.effets_joues.push(id);
        change = true;
        if let Sensation::Vibration(v) = e.quoi
            && t.capacites.vibration == Capacite::Oui
        {
            let ms = match v {
                Vibration::Courte => 40,
                Vibration::Succes => 120,
            };
            let _: bool = t.fenetre.navigator().vibrate_with_duration(ms);
        }
    }
    if change {
        let debut = t.effets_joues.len().saturating_sub(EFFETS_GARDES);
        t.effets_joues.drain(..debut);
        if let Some(s) = stockage_de_session(&t.fenetre) {
            let v: Vec<String> = t.effets_joues.iter().map(|i| format!("{i:x}")).collect();
            let _: Result<(), JsValue> = s.set_item(CLE_EFFETS, &v.join(","));
        }
    }
}

fn garder_la_file(t: &mut Tel) {
    let Some(a) = t.appareil.as_ref() else {
        return;
    };
    let g = a.garder_la_file();
    if g != t.file_gardee {
        if let Some(s) = stockage_de_session(&t.fenetre) {
            let _: Result<(), JsValue> = s.set_item(CLE_FILE, &hexa(&g));
        }
        t.file_gardee = g;
    }
}

/// Le retenu de la page, gardé dans `localStorage` quand il change (lot 12, fin) : par salle
/// et par jeu, comme `PageFil::retenir` le dit ([`retenu`]). Un stockage cassé le laisse en
/// mémoire.
fn garder_le_retenu(t: &mut Tel) {
    let Some((cle, garde)) = t.retenu.as_mut() else {
        return;
    };
    let actuel = t.page.as_ref().and_then(|p| p.retenu());
    if let Some(o) = retenu::a_ecrire(actuel, garde.as_deref()) {
        if let Some(s) = stockage_local(&t.fenetre) {
            let _: Result<(), JsValue> = s.set_item(cle, &hexa(o));
        }
        *garde = Some(o.to_vec());
    }
}

/// Ce qu'un recalcul de la page demande : le tenu à l'appareil, les doigts lâchés (REM-23),
/// la page reposée.
fn appliquer(t: &mut Tel, r: &Recalcul) {
    garder_le_retenu(t);
    let now = maintenant_us(&t.fenetre);
    if let Some(tenu) = &r.tenu
        && let Some(a) = t.appareil.as_mut()
    {
        a.tenir(now, tenu);
        if r.tenu_relache {
            // Un tenu relâché ne se redit pas (§ 2.10, P27).
            a.taire_la_redite();
        }
        t.tenus = t.tenus.saturating_add(1);
    }
    for c in &r.laches {
        let doigts: Vec<i32> = t
            .doigts
            .iter()
            .chain(t.croix.iter())
            .filter(|(_, k)| *k == c)
            .map(|(p, _)| *p)
            .collect();
        for p in doigts {
            t.doigts.remove(&p);
            t.croix.remove(&p);
            t.ignores.insert(p);
        }
    }
    if r.page_changee {
        t.a_poser = true;
    }
    if t.a_poser {
        poser(t);
    }
    vider(t);
}

fn geste(t: &mut Tel, octets: &[u8]) {
    let now = maintenant_us(&t.fenetre);
    let Some(a) = t.appareil.as_mut() else {
        return;
    };
    match a.geste(now, octets) {
        Ok(_) => t.gestes = t.gestes.saturating_add(1),
        Err(Envoi::FilePleine) => {
            // SAL-07 : une file pleine se dit, jamais une perte silencieuse.
            t.files_pleines = t.files_pleines.saturating_add(1);
            t.bandeau
                .set_text_content(Some("Trop de gestes en attente : patiente un instant"));
            poser_style(&t.bandeau, &[("display", "block")]);
            t.bandeau_jusqu_a = maintenant_ms(&t.fenetre) + 2_000.0;
        }
        Err(Envoi::TropLong) => {}
    }
    garder_la_file(t);
    vider(t);
}

// ---------------------------------------------------------------------------
// Le réveil
// ---------------------------------------------------------------------------

fn tic(etat: &Etat) {
    let relancer = {
        let Ok(mut t) = etat.try_borrow_mut() else {
            return;
        };
        let now_ms = maintenant_ms(&t.fenetre);
        let now = maintenant_us(&t.fenetre);
        if let Some(a) = t.appareil.as_mut() {
            a.reveiller(now);
        }
        // P28 : la coupure, jugée sur l'écho resté sans réponse.
        let coupe = t.appareil.as_ref().is_some_and(|a| a.coupe(now));
        if coupe && let Some(p) = t.page.as_mut() {
            let r = p.coupure(now);
            appliquer(&mut t, &r);
        }
        if let Some(p) = t.page.as_mut()
            && let Some(tenu) = p.reveiller(now)
        {
            let relache = p.relache(Some(&tenu));
            if let Some(a) = t.appareil.as_mut() {
                a.tenir(now, &tenu);
                if relache {
                    a.taire_la_redite();
                }
            }
        }
        comptes_a_rebours(&t, now_ms);
        if t.bandeau_jusqu_a > 0.0 && now_ms > t.bandeau_jusqu_a {
            t.bandeau_jusqu_a = 0.0;
            poser_style(&t.bandeau, &[("display", "none")]);
        }
        vider(&mut t);
        let gele = now_ms < t.gele_jusqu_a;
        let du = t.prochaine_tentative_ms.is_some_and(|p| now_ms >= p);
        // Relié au relais sans accueil depuis trop longtemps (un bonjour perdu) : on
        // reconnecte, comme sans réponse à l'écho (SAL-47), sans attendre la fermeture.
        let accueilli = t.appareil.as_ref().is_some_and(Appareil::accueilli);
        if accueilli {
            t.ouvert_depuis_ms = now_ms;
        }
        let sans_accueil =
            t.ws_ouvert && !accueilli && now_ms - t.ouvert_depuis_ms > SANS_ACCUEIL_MS;
        !gele && (du || (coupe && t.ws_ouvert) || sans_accueil)
    };
    if relancer {
        relier(etat);
    }
}

fn comptes_a_rebours(t: &Tel, now_ms: f64) {
    let Some((pas, depuis)) = t.vue else {
        return;
    };
    #[expect(
        clippy::cast_precision_loss,
        reason = "un pas tient dans un f64 à la seconde près"
    )]
    let pas_estime = pas as f64 + (now_ms - depuis) * 60.0 / 1000.0;
    for (e, c) in &t.comptes_a_rebours {
        let restant_s = match c {
            #[expect(clippy::cast_precision_loss, reason = "idem")]
            CompteAReboursFil::Court { echeance } => {
                ((echeance.numero() as f64 - pas_estime) / 60.0).max(0.0)
            }
            #[expect(clippy::cast_precision_loss, reason = "idem")]
            CompteAReboursFil::Fige { restant } => restant.en_ms() as f64 / 1000.0,
        };
        let s = restant_s.ceil() as u64;
        let texte = if s >= 60 {
            format!("{}:{:02}", s / 60, s % 60)
        } else {
            format!("{s}")
        };
        if e.text_content().as_deref() != Some(texte.as_str()) {
            e.set_text_content(Some(&texte));
        }
    }
}

// ---------------------------------------------------------------------------
// Les écrans du commun
// ---------------------------------------------------------------------------

fn montrer_l_ecran(t: &mut Tel) {
    if t.pire_cas {
        // La page de pire cas du banc n'a ni salle ni relais : rien ne la couvre.
        t.ecran = None;
        poser_style(&t.voile, &[("display", "none")]);
        return;
    }
    let e = ecrans::Etat {
        adresse_illisible: t.adresse_illisible || t.panne_module.is_some(),
        refus: t.appareil.as_ref().and_then(Appareil::refus),
        refus_relais: t.appareil.as_ref().and_then(Appareil::refus_relais),
        illisibles: t.appareil.as_ref().map_or(0, |a| a.comptes().illisibles),
        accueilli: t.appareil.as_ref().is_some_and(Appareil::accueilli),
        attente: t.appareil.as_ref().and_then(Appareil::attente),
        page: t.page.as_ref().is_some_and(|p| p.page().is_some()),
    };
    let choisi = ecrans::choisir(&e);
    if choisi == t.ecran {
        return;
    }
    t.ecran = choisi;
    match choisi {
        Some(ec) => {
            t.voile_texte.set_text_content(Some(ec.texte()));
            poser_style(&t.voile, &[("display", "flex")]);
            poser_style(
                &t.reprendre,
                &[(
                    "display",
                    if ec == Ecran::Remplace {
                        "block"
                    } else {
                        "none"
                    },
                )],
            );
        }
        None => poser_style(&t.voile, &[("display", "none")]),
    }
}

// ---------------------------------------------------------------------------
// Le rendu DOM, par clé
// ---------------------------------------------------------------------------

fn poser(t: &mut Tel) {
    t.a_poser = false;
    let Some(page) = t.page.as_ref().and_then(|p| p.page().cloned()) else {
        return;
    };
    let champs = t
        .page
        .as_ref()
        .map(|p| p.locaux().champs.clone())
        .unwrap_or_default();
    let choix = t
        .page
        .as_ref()
        .map(|p| p.locaux().choix.clone())
        .unwrap_or_default();
    let mut vus = BTreeSet::new();
    let mut derniers: BTreeMap<usize, Element> = BTreeMap::new();
    let mut rangs: BTreeMap<usize, u32> = BTreeMap::new();
    t.comptes_a_rebours.clear();
    for n in &page.noeuds {
        let sorte = Sorte::de(&n.sorte);
        let el = match t.elements.get(&n.cle) {
            Some((s, e)) if *s == sorte => e.clone(),
            _ => {
                let Ok(e) = creer(&t.document, sorte.balise()) else {
                    continue;
                };
                ignorer(e.set_attribute("data-cle", &format!("{:016x}", n.cle.fil())));
                ignorer(e.set_attribute("data-sorte", sorte.nom()));
                if let Some((_, ancien)) = t.elements.insert(n.cle, (sorte, e.clone())) {
                    ancien.remove();
                }
                t.signatures.remove(&n.cle);
                if sorte == Sorte::Champ
                    && let Some(i) = e.dyn_ref::<HtmlInputElement>()
                {
                    // La valeur se pose à la création seulement : jamais réécrite sous
                    // le doigt, ni pendant une composition (§ 2.10).
                    i.set_value(champs.get(&n.cle).map_or("", String::as_str));
                }
                e
            }
        };
        vus.insert(n.cle);
        mettre_a_jour(t, &el, n.cle, &n.sorte, &choix);
        // L'ordre : l'élément suit son frère précédent dans son parent ; il ne bouge que
        // s'il n'y est pas (un champ déplacé perdrait le focus).
        let parent: Element = match n.parent {
            Some(p) => page
                .noeuds
                .get(usize::try_from(p).unwrap_or(usize::MAX))
                .and_then(|pn| t.elements.get(&pn.cle))
                .map_or_else(|| t.racine.clone().into(), |(_, e)| e.clone().into()),
            None => t.racine.clone().into(),
        };
        let indice = n
            .parent
            .map_or(usize::MAX, |p| usize::try_from(p).unwrap_or(usize::MAX));
        ranger_dans_une_pile(&page.noeuds, n.parent, &el, &mut rangs);
        let precedent = derniers.get(&indice).cloned();
        let bien_place = match &precedent {
            Some(p) => el.previous_element_sibling().as_ref() == Some(p),
            None => {
                el.parent_element().as_ref() == Some(&parent)
                    && el.previous_element_sibling().is_none()
            }
        } && el.parent_element().as_ref() == Some(&parent);
        if !bien_place {
            let reference = match &precedent {
                Some(p) => p.next_sibling(),
                None => parent.first_child(),
            };
            let _: Result<web_sys::Node, JsValue> = parent.insert_before(&el, reference.as_ref());
        }
        derniers.insert(indice, el.into());
    }
    if t.deborde
        && let Ok(e) = creer(&t.document, "div")
    {
        // Le contrôle négatif du scénario `pages` : un texte d'un seul mot, plus large que
        // tout écran, dans une boîte de la largeur de la page.
        ignorer(e.set_attribute("data-cle", "00000000deb0deb0"));
        ignorer(e.set_attribute("data-sorte", "texte"));
        e.set_text_content(Some(&"W".repeat(400)));
        poser_style(&e, &[("overflow", "hidden"), ("white-space", "nowrap")]);
        ignorer(t.racine.append_child(&e));
        if let Some((_, ancien)) = t
            .elements
            .insert(Cle::du_fil(0xdeb0_deb0), (Sorte::Texte, e))
        {
            ancien.remove();
        }
        vus.insert(Cle::du_fil(0xdeb0_deb0));
    }
    let partis: Vec<Cle> = t
        .elements
        .keys()
        .filter(|c| !vus.contains(c))
        .copied()
        .collect();
    for c in partis {
        if let Some((_, e)) = t.elements.remove(&c) {
            e.remove();
        }
        t.signatures.remove(&c);
    }
}

/// Un enfant de pile se superpose aux autres dans la même case, au-dessus de ceux qui le
/// précèdent dans la page (correction du lot 12 : posé seulement quand la pile se mettait à
/// jour, un enfant créé après elle, comme une image de l'avatar refaite sous le bouton
/// « Sauter » de Sidequest, n'avait ni sa case ni son rang, et le doigt touchait l'image au
/// lieu du bouton).
fn ranger_dans_une_pile(
    noeuds: &[jeu_abi::NoeudPage],
    parent: Option<u32>,
    el: &HtmlElement,
    rangs: &mut BTreeMap<usize, u32>,
) {
    let Some(indice) = parent.and_then(|p| usize::try_from(p).ok()) else {
        return;
    };
    if !noeuds
        .get(indice)
        .is_some_and(|pn| matches!(pn.sorte, SorteNoeudPage::Pile))
    {
        return;
    }
    let rang = rangs.entry(indice).or_insert(0u32);
    *rang = rang.saturating_add(1);
    let z = rang.to_string();
    poser_style(el, &[("grid-area", "1 / 1"), ("z-index", &z)]);
}

#[expect(
    clippy::too_many_lines,
    reason = "une branche par sorte de nœud, chacune courte"
)]
fn mettre_a_jour(
    t: &mut Tel,
    el: &HtmlElement,
    cle: Cle,
    s: &SorteNoeudPage,
    choix: &BTreeMap<Cle, u32>,
) {
    match s {
        SorteNoeudPage::Colonne => poser_style(
            el,
            &[
                ("display", "flex"),
                ("flex-direction", "column"),
                ("gap", "8px"),
            ],
        ),
        SorteNoeudPage::Rangee => poser_style(
            el,
            &[
                ("display", "flex"),
                ("flex-direction", "row"),
                ("flex-wrap", "wrap"),
                ("gap", "8px"),
            ],
        ),
        SorteNoeudPage::Grille { colonnes } => {
            let g = format!("repeat({colonnes}, minmax(0, 1fr))");
            poser_style(
                el,
                &[
                    ("display", "grid"),
                    ("grid-template-columns", &g),
                    ("gap", "8px"),
                ],
            );
        }
        SorteNoeudPage::Pile => {
            poser_style(el, &[("display", "grid"), ("place-items", "center")]);
            // Les enfants d'une pile se superposent dans la même case de la grille.
            let enfants = el.children();
            for i in 0..enfants.length() {
                if let Some(e) = enfants
                    .item(i)
                    .and_then(|e| e.dyn_into::<HtmlElement>().ok())
                {
                    poser_style(&e, &[("grid-area", "1 / 1")]);
                }
            }
        }
        SorteNoeudPage::Texte { texte, style: st } => {
            if el.text_content().as_deref() != Some(texte.as_str()) {
                el.set_text_content(Some(texte));
            }
            appliquer_le_style(el, st, jeu_abi::Oui::Non);
            poser_style(el, &[("overflow-wrap", "anywhere")]);
        }
        SorteNoeudPage::Image {
            image,
            hauteur,
            teinte,
        } => {
            let url = t.images.get(&image.fil()).cloned().unwrap_or_default();
            let h = style::taille(*hauteur, 0);
            // La teinte, appliquée par le commun : l'image sert de masque à sa couleur.
            if let Some(c) = teinte {
                {
                    // Sans `src` : la politique de contenu refuse `data:`, et l'image n'est
                    // que le masque de sa couleur.
                    ignorer(el.remove_attribute("src"));
                    let masque = format!("url({url})");
                    poser_style(
                        el,
                        &[
                            ("height", &h),
                            ("aspect-ratio", "1"),
                            ("background-color", &style::couleur(*c)),
                            ("-webkit-mask-image", &masque),
                            ("mask-image", &masque),
                            ("-webkit-mask-size", "contain"),
                            ("mask-size", "contain"),
                            ("-webkit-mask-repeat", "no-repeat"),
                            ("mask-repeat", "no-repeat"),
                        ],
                    );
                }
            } else {
                if el.get_attribute("src").as_deref() != Some(url.as_str()) && !url.is_empty() {
                    ignorer(el.set_attribute("src", &url));
                }
                poser_style(el, &[("height", &h)]);
            }
        }
        SorteNoeudPage::CompteARebours { compte, style: st } => {
            appliquer_le_style(el, st, jeu_abi::Oui::Non);
            t.comptes_a_rebours.push((el.clone(), *compte));
        }
        SorteNoeudPage::Bouton { nom, style: st, .. } => {
            if el.text_content().as_deref() != Some(nom.as_str()) {
                el.set_text_content(Some(nom));
            }
            appliquer_le_style(el, st, jeu_abi::Oui::Non);
            poser_style(
                el,
                &[
                    ("padding", "12px"),
                    ("min-height", "44px"),
                    ("touch-action", "manipulation"),
                    // Un nom d'un seul mot plus large que la page (une réponse tapée,
                    // montrée au vote de Punchline) se coupe, comme un texte (fin du
                    // lot 12 : vu déborder à 768 × 1024 par le scénario `pages`).
                    ("overflow-wrap", "anywhere"),
                ],
            );
        }
        SorteNoeudPage::Maintenu { nom, style: st } => {
            if el.text_content().as_deref() != Some(nom.as_str()) {
                el.set_text_content(Some(nom));
            }
            appliquer_le_style(el, st, jeu_abi::Oui::Non);
            poser_style(
                el,
                &[
                    ("padding", "16px"),
                    ("min-height", "56px"),
                    ("touch-action", "none"),
                    ("overflow-wrap", "anywhere"),
                ],
            );
        }
        SorteNoeudPage::Croix { directions } => {
            ignorer(el.set_attribute(
                "data-directions",
                if *directions == Directions::Huit {
                    "8"
                } else {
                    "4"
                },
            ));
            poser_style(
                el,
                &[
                    ("width", "min(60vw, 240px)"),
                    ("height", "min(60vw, 240px)"),
                    ("border-radius", "50%"),
                    (
                        "background",
                        "radial-gradient(circle, #555 0 18%, #333 19% 100%)",
                    ),
                    ("touch-action", "none"),
                    ("align-self", "center"),
                ],
            );
        }
        SorteNoeudPage::Champ {
            exemple, clavier, ..
        } => {
            if let Some(i) = el.dyn_ref::<HtmlInputElement>() {
                if i.placeholder() != *exemple {
                    i.set_placeholder(exemple);
                }
                let (mode, majuscules) = match clavier {
                    jeu_abi::Clavier::Nombre => ("numeric", "off"),
                    jeu_abi::Clavier::Prenom => ("text", "words"),
                    jeu_abi::Clavier::Texte => ("text", "sentences"),
                };
                ignorer(el.set_attribute("inputmode", mode));
                ignorer(el.set_attribute("autocapitalize", majuscules));
                ignorer(el.set_attribute("autocomplete", "off"));
                ignorer(el.set_attribute("enterkeyhint", "send"));
            }
            poser_style(
                el,
                &[
                    ("font-size", &format!("{}px", style::CHAMP_PX_MIN.max(18))),
                    ("padding", "8px"),
                    ("box-sizing", "border-box"),
                    ("width", "100%"),
                    ("user-select", "text"),
                    ("-webkit-user-select", "text"),
                ],
            );
        }
        SorteNoeudPage::Choix { options, balayable } => {
            let actuel = choix.get(&cle).copied().unwrap_or(0);
            let signature = format!(
                "{actuel}|{balayable:?}|{}",
                options
                    .iter()
                    .map(|(_, n)| n.as_str())
                    .collect::<Vec<_>>()
                    .join("\u{1f}")
            );
            if t.signatures.get(&cle) == Some(&signature) {
                return;
            }
            t.signatures.insert(cle, signature);
            el.set_text_content(None);
            poser_style(
                el,
                &[
                    ("display", "flex"),
                    ("gap", "6px"),
                    ("flex-wrap", "wrap"),
                    ("align-items", "center"),
                ],
            );
            if *balayable == jeu_abi::Oui::Oui {
                let nom = options
                    .get(usize::try_from(actuel).unwrap_or(usize::MAX))
                    .map_or("", |(_, n)| n.as_str());
                for (pas, texte) in [("-1", "‹"), ("0", nom), ("1", "›")] {
                    if let Ok(b) = creer(&t.document, if pas == "0" { "div" } else { "button" }) {
                        b.set_text_content(Some(texte));
                        ignorer(b.set_attribute("data-pas", pas));
                        poser_style(
                            &b,
                            &[
                                ("padding", "10px"),
                                ("font-size", "20px"),
                                ("flex", if pas == "0" { "1" } else { "0" }),
                                ("text-align", "center"),
                            ],
                        );
                        ignorer(el.append_child(&b));
                    }
                }
                poser_style(el, &[("touch-action", "pan-y")]);
            } else {
                for (i, (_, nom)) in options.iter().enumerate() {
                    if let Ok(b) = creer(&t.document, "button") {
                        b.set_text_content(Some(nom));
                        ignorer(b.set_attribute("data-option", &i.to_string()));
                        let choisi = u32::try_from(i).ok() == Some(actuel);
                        poser_style(
                            &b,
                            &[
                                ("padding", "10px"),
                                ("font-size", "18px"),
                                ("outline", if choisi { "3px solid #fc0" } else { "none" }),
                            ],
                        );
                        ignorer(el.append_child(&b));
                    }
                }
            }
        }
    }
}

fn appliquer_le_style(el: &HtmlElement, s: &jeu_abi::StylePage, champ: jeu_abi::Oui) {
    for (k, v) in style::declarations(s, champ) {
        let _: Result<(), JsValue> = el.style().set_property(k, &v);
    }
}

// ---------------------------------------------------------------------------
// Les événements, délégués à la racine
// ---------------------------------------------------------------------------

fn cle_de(e: &web_sys::Event) -> Option<(Cle, Sorte, HtmlElement)> {
    let cible = e.target()?.dyn_into::<Element>().ok()?;
    let el = cible
        .closest("[data-cle]")
        .ok()??
        .dyn_into::<HtmlElement>()
        .ok()?;
    let cle = u64::from_str_radix(&el.get_attribute("data-cle")?, 16).ok()?;
    let sorte = match el.get_attribute("data-sorte")?.as_str() {
        "bouton" => Sorte::Bouton,
        "maintenu" => Sorte::Maintenu,
        "croix" => Sorte::Croix,
        "champ" => Sorte::Champ,
        "choix" => Sorte::Choix,
        _ => Sorte::Texte,
    };
    Some((Cle::du_fil(cle), sorte, el))
}

/// La direction d'un doigt sur une croix : une zone morte au centre, puis quatre ou huit
/// secteurs.
fn direction(el: &HtmlElement, x: f64, y: f64) -> Direction8 {
    let r = el.get_bounding_client_rect();
    let (dx, dy) = (
        x - (r.left() + r.width() / 2.0),
        (r.top() + r.height() / 2.0) - y,
    );
    let rayon = r.width().min(r.height()) / 2.0;
    if dx.hypot(dy) < rayon * 0.25 {
        return Direction8::Centre;
    }
    let huit = el.get_attribute("data-directions").as_deref() == Some("8");
    let angle = dy.atan2(dx).to_degrees().rem_euclid(360.0);
    if huit {
        let s = ((angle + 22.5) / 45.0).floor() as i64 % 8;
        match s {
            0 => Direction8::Droite,
            1 => Direction8::HautDroite,
            2 => Direction8::Haut,
            3 => Direction8::HautGauche,
            4 => Direction8::Gauche,
            5 => Direction8::BasGauche,
            6 => Direction8::Bas,
            _ => Direction8::BasDroite,
        }
    } else {
        let s = ((angle + 45.0) / 90.0).floor() as i64 % 4;
        match s {
            0 => Direction8::Droite,
            1 => Direction8::Haut,
            2 => Direction8::Gauche,
            _ => Direction8::Bas,
        }
    }
}

fn sur<E: JsCast + 'static>(
    cible: &web_sys::EventTarget,
    nom: &str,
    etat: &Etat,
    f: fn(&mut Tel, &E),
) -> Result<(), String> {
    let e = Rc::clone(etat);
    let c = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Ok(mut t) = e.try_borrow_mut() else {
            return;
        };
        if let Some(ev) = ev.dyn_ref::<E>() {
            f(&mut t, ev);
        }
    });
    cible
        .add_event_listener_with_callback(nom, c.as_ref().unchecked_ref())
        .map_err(|_| format!("écouter {nom}"))?;
    c.forget();
    Ok(())
}

fn ecouter(etat: &Etat, corps: &HtmlElement) -> Result<(), String> {
    let racine: web_sys::EventTarget = corps.clone().into();
    sur::<PointerEvent>(&racine, "pointerdown", etat, pointeur_pose)?;
    sur::<PointerEvent>(&racine, "pointermove", etat, pointeur_bouge)?;
    for nom in ["pointerup", "pointercancel", "lostpointercapture"] {
        sur::<PointerEvent>(&racine, nom, etat, pointeur_leve)?;
    }
    sur::<web_sys::MouseEvent>(&racine, "click", etat, clic)?;
    sur::<web_sys::Event>(&racine, "input", etat, saisie)?;
    sur::<web_sys::Event>(&racine, "compositionstart", etat, |t, _| t.compose = true)?;
    sur::<web_sys::Event>(&racine, "compositionend", etat, |t, e| {
        t.compose = false;
        saisie(t, e);
    })?;
    sur::<KeyboardEvent>(&racine, "keydown", etat, touche)?;
    let (fenetre, document) = {
        let t = etat.borrow();
        (t.fenetre.clone(), t.document.clone())
    };
    let f: web_sys::EventTarget = fenetre.clone().into();
    let d: web_sys::EventTarget = document.into();
    // REM-11 : l'arrière-plan lève ce que le doigt tient, puis plus rien ne se redit.
    for (cible, nom) in [(&f, "pagehide"), (&f, "blur"), (&d, "freeze")] {
        sur::<web_sys::Event>(cible, nom, etat, |t, _| arriere_plan(t))?;
    }
    sur::<web_sys::Event>(&d, "visibilitychange", etat, |t, _| {
        if t.document.visibility_state() == web_sys::VisibilityState::Visible {
            retour(t);
        } else {
            arriere_plan(t);
        }
    })?;
    // SAL-52 : au retour, un écho ; sans réponse, la reconnexion suit (le réveil).
    for (cible, nom) in [
        (&f, "pageshow"),
        (&f, "focus"),
        (&f, "online"),
        (&d, "resume"),
    ] {
        sur::<web_sys::Event>(cible, nom, etat, |t, _| retour(t))?;
    }
    if let Some(v) = fenetre.visual_viewport() {
        let v: web_sys::EventTarget = v.into();
        sur::<web_sys::Event>(&v, "resize", etat, |t, _| taille(t))?;
    }
    sur::<web_sys::Event>(&f, "resize", etat, |t, _| taille(t))?;
    Ok(())
}

fn arriere_plan(t: &mut Tel) {
    let now = maintenant_us(&t.fenetre);
    t.doigts.clear();
    t.croix.clear();
    if let Some(p) = t.page.as_mut() {
        let r = p.arriere_plan(now);
        appliquer(t, &r);
    }
    if let Some(a) = t.appareil.as_mut() {
        a.taire_la_redite();
    }
    vider(t);
}

fn retour(t: &mut Tel) {
    if !t.ws_ouvert {
        t.prochaine_tentative_ms = Some(maintenant_ms(&t.fenetre));
    }
}

fn taille(t: &mut Tel) {
    let now = maintenant_us(&t.fenetre);
    let ecran = l_ecran(&t.fenetre, &t.document);
    if let Some(p) = t.page.as_mut() {
        let r = p.ecran(now, ecran);
        appliquer(t, &r);
    }
}

fn pointeur_pose(t: &mut Tel, e: &PointerEvent) {
    if let Some(cible) = e.target().and_then(|c| c.dyn_into::<Element>().ok())
        && cible.get_attribute("data-commun").as_deref() == Some("reprendre")
    {
        return;
    }
    let Some((cle, sorte, el)) = cle_de(e) else {
        return;
    };
    let now = maintenant_us(&t.fenetre);
    let id = e.pointer_id();
    match sorte {
        Sorte::Bouton => {
            // L'état appuyé s'affiche dans le même gestionnaire (REM-90, REM-116).
            poser_style(&el, &[("filter", "brightness(0.75)")]);
            if let Some(p) = t.page.as_ref()
                && let Action::Geste(g) = p.bouton(cle, Declencheur::Contact)
            {
                geste(t, &g);
            }
        }
        Sorte::Maintenu => {
            let _: Result<(), JsValue> = el.set_pointer_capture(id);
            poser_style(&el, &[("filter", "brightness(0.75)")]);
            t.doigts.insert(id, cle);
            if let Some(p) = t.page.as_mut() {
                let r = p.maintenir(now, cle, jeu_abi::Oui::Oui);
                appliquer(t, &r);
            }
            e.prevent_default();
        }
        Sorte::Croix => {
            let _: Result<(), JsValue> = el.set_pointer_capture(id);
            t.croix.insert(id, cle);
            let d = direction(&el, f64::from(e.client_x()), f64::from(e.client_y()));
            if let Some(p) = t.page.as_mut() {
                let r = p.diriger(now, cle, d);
                appliquer(t, &r);
            }
            e.prevent_default();
        }
        Sorte::Choix => t.balayage = Some((id, cle, f64::from(e.client_x()))),
        Sorte::Colonne
        | Sorte::Rangee
        | Sorte::Grille
        | Sorte::Pile
        | Sorte::Texte
        | Sorte::Image
        | Sorte::Compte
        | Sorte::Champ => {}
    }
}

fn pointeur_bouge(t: &mut Tel, e: &PointerEvent) {
    let id = e.pointer_id();
    if t.ignores.contains(&id) {
        return;
    }
    let Some(cle) = t.croix.get(&id).copied() else {
        return;
    };
    let Some((_, el)) = t.elements.get(&cle).cloned() else {
        return;
    };
    let now = maintenant_us(&t.fenetre);
    let d = direction(&el, f64::from(e.client_x()), f64::from(e.client_y()));
    if let Some(p) = t.page.as_mut() {
        let r = p.diriger(now, cle, d);
        appliquer(t, &r);
    }
}

fn pointeur_leve(t: &mut Tel, e: &PointerEvent) {
    let id = e.pointer_id();
    let now = maintenant_us(&t.fenetre);
    if let Some((_, _, el)) = cle_de(e) {
        poser_style(&el, &[("filter", "none")]);
    }
    // REM-23 : un doigt lâché par le commun ne compte plus jusqu'à ce qu'il se relève.
    if t.ignores.remove(&id) {
        return;
    }
    if let Some(cle) = t.doigts.remove(&id)
        && let Some(p) = t.page.as_mut()
    {
        let r = p.maintenir(now, cle, jeu_abi::Oui::Non);
        appliquer(t, &r);
    }
    if let Some(cle) = t.croix.remove(&id)
        && let Some(p) = t.page.as_mut()
    {
        let r = p.diriger(now, cle, Direction8::Centre);
        appliquer(t, &r);
    }
    if e.type_() == "pointerup"
        && let Some((pid, cle, x0)) = t.balayage.take()
        && pid == id
    {
        let dx = f64::from(e.client_x()) - x0;
        if dx.abs() >= BALAYAGE_PX {
            tourner(t, cle, if dx < 0.0 { 1 } else { -1 });
        }
    }
}

/// Le carrousel tourne d'une option.
fn tourner(t: &mut Tel, cle: Cle, pas: i64) {
    let Some(p) = t.page.as_ref() else {
        return;
    };
    let n = p
        .page()
        .and_then(|pg| pg.noeuds.iter().find(|x| x.cle == cle))
        .and_then(|x| {
            if let SorteNoeudPage::Choix { options, .. } = &x.sorte {
                Some(options.len())
            } else {
                None
            }
        })
        .unwrap_or(0);
    if n == 0 {
        return;
    }
    let actuel = i64::from(p.locaux().choix.get(&cle).copied().unwrap_or(0));
    let n = i64::try_from(n).unwrap_or(1);
    let suivant = u32::try_from(actuel.saturating_add(pas).rem_euclid(n.max(1))).unwrap_or(0);
    let now = maintenant_us(&t.fenetre);
    if let Some(p) = t.page.as_mut() {
        let r = p.choisir(now, cle, suivant);
        appliquer(t, &r);
    }
}

fn clic(t: &mut Tel, e: &web_sys::MouseEvent) {
    let cible = e.target().and_then(|c| c.dyn_into::<Element>().ok());
    if cible
        .as_ref()
        .and_then(|c| c.get_attribute("data-commun"))
        .as_deref()
        == Some("reprendre")
    {
        // « Reprendre ici » : sur un geste du joueur, jamais de lui-même (SAL-18, SAL-21).
        if let Some(a) = t.appareil.as_mut() {
            a.reprendre_ici();
        }
        t.prochaine_tentative_ms = Some(maintenant_ms(&t.fenetre));
        t.delai_ms = RECONNEXION_MIN_MS;
        montrer_l_ecran(t);
        return;
    }
    let Some((cle, sorte, _)) = cle_de(e) else {
        return;
    };
    let now = maintenant_us(&t.fenetre);
    match sorte {
        Sorte::Bouton => {
            if let Some(p) = t.page.as_ref()
                && let Action::Geste(g) = p.bouton(cle, Declencheur::Relache)
            {
                geste(t, &g);
            }
        }
        Sorte::Choix => {
            let c = cible.as_ref();
            if let Some(i) = c
                .and_then(|c| c.get_attribute("data-option"))
                .and_then(|v| v.parse::<u32>().ok())
            {
                if let Some(p) = t.page.as_mut() {
                    let r = p.choisir(now, cle, i);
                    appliquer(t, &r);
                }
            } else if let Some(pas) = c
                .and_then(|c| c.get_attribute("data-pas"))
                .and_then(|v| v.parse::<i64>().ok())
                && pas != 0
            {
                tourner(t, cle, pas);
            }
        }
        Sorte::Colonne
        | Sorte::Rangee
        | Sorte::Grille
        | Sorte::Pile
        | Sorte::Texte
        | Sorte::Image
        | Sorte::Compte
        | Sorte::Maintenu
        | Sorte::Croix
        | Sorte::Champ => {}
    }
}

fn saisie(t: &mut Tel, e: &web_sys::Event) {
    if t.compose {
        return;
    }
    let Some((cle, Sorte::Champ, el)) = cle_de(e) else {
        return;
    };
    let Some(champ) = el.dyn_ref::<HtmlInputElement>() else {
        return;
    };
    let now = maintenant_us(&t.fenetre);
    let v = champ.value();
    if let Some(p) = t.page.as_mut() {
        let r = p.taper(now, cle, &v);
        appliquer(t, &r);
    }
}

fn touche(t: &mut Tel, e: &KeyboardEvent) {
    if e.key() != "Enter" || e.is_composing() {
        return;
    }
    let Some((cle, Sorte::Champ, _)) = cle_de(e) else {
        return;
    };
    e.prevent_default();
    if let Some(p) = t.page.as_ref()
        && let Action::Geste(g) = p.entree(cle)
    {
        geste(t, &g);
    }
}

// ---------------------------------------------------------------------------
// Les crochets du banc (mode banc seulement)
// ---------------------------------------------------------------------------

fn json(s: &str) -> String {
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

fn etat_json(t: &Tel) -> String {
    let a = t.appareil.as_ref();
    let p = t.page.as_ref();
    let page = p.and_then(|p| p.page());
    let comptes = p.map(PageDuTelephone::comptes).unwrap_or_default();
    let (recus, total) = a.map_or((0, 0), Appareil::progres_du_module);
    format!(
        "{{\"ecran\":{},\"accueilli\":{},\"ws\":{},\"module\":{},\"progres\":[{recus},{total}],\"panne\":{},\"noeuds\":{},\"gestes\":{},\"tenus\":{},\"files_pleines\":{},\"file_max\":{},\"accuse\":{},\"en_file\":{},\"dernier_geste\":{},\"onglet\":\"{:x}\",\"m4\":{},\"pages\":{},\"fautes\":{},\"laches_rem23\":{},\"coupures\":{},\"arriere_plans\":{},\"tenu\":{},\"tenu_relache\":{},\"refus\":{},\"vue\":{},\"lien\":{},\"changements\":{},\"retenu\":{},\"retenu_repris\":{},\"retenu_cle\":{}}}",
        t.ecran
            .map_or_else(|| String::from("null"), |e| json(e.nom())),
        a.is_some_and(Appareil::accueilli),
        t.ws_ouvert,
        a.and_then(Appareil::module)
            .map_or_else(|| String::from("null"), |(e, _)| json(&hexa(&e))),
        t.panne_module
            .as_deref()
            .map_or_else(|| String::from("null"), json),
        page.map_or(0, |pg| pg.noeuds.len()),
        t.gestes,
        a.map_or(0, |a| a.comptes().tenus),
        t.files_pleines,
        a.map_or(0, Appareil::file_max),
        a.map_or(0, Appareil::accuse),
        a.map_or(0, Appareil::en_file),
        a.map_or(0, Appareil::dernier_geste),
        a.map_or(0, Appareil::onglet),
        t.m4.map_or_else(
            || String::from("null"),
            |(n, ms)| format!("{{\"octets\":{n},\"ms\":{ms:.3}}}")
        ),
        comptes.pages,
        comptes.fautes,
        comptes.laches_rem23,
        comptes.coupures,
        comptes.arriere_plans,
        p.and_then(|p| p.tenu_envoye())
            .map_or_else(|| String::from("null"), |o| json(&hexa(o))),
        p.is_some_and(|p| p.relache(p.tenu_envoye())),
        a.and_then(Appareil::refus)
            .map_or_else(|| String::from("null"), |r| json(&format!("{r:?}"))),
        t.vue.map_or(0, |(p, _)| p),
        a.and_then(Appareil::lien_relais)
            .map_or_else(|| String::from("null"), |l| l.to_string()),
        t.changements,
        p.and_then(|p| p.retenu())
            .map_or_else(|| String::from("null"), |o| json(&hexa(o))),
        t.retenu_repris
            .as_deref()
            .map_or_else(|| String::from("null"), |o| json(&hexa(o))),
        t.retenu
            .as_ref()
            .map_or_else(|| String::from("null"), |(c, _)| json(c)),
    )
}

fn noeuds_json(t: &Tel) -> String {
    let Some(page) = t.page.as_ref().and_then(|p| p.page()) else {
        return String::from("[]");
    };
    let champs = t
        .page
        .as_ref()
        .map(|p| p.locaux().champs.clone())
        .unwrap_or_default();
    let v: Vec<String> = page
        .noeuds
        .iter()
        .map(|n| {
            let (sorte, nom) = match &n.sorte {
                SorteNoeudPage::Bouton { nom, .. } | SorteNoeudPage::Maintenu { nom, .. } => {
                    (Sorte::de(&n.sorte).nom(), nom.clone())
                }
                SorteNoeudPage::Texte { texte, .. } => ("texte", texte.clone()),
                SorteNoeudPage::Champ { .. } => {
                    ("champ", champs.get(&n.cle).cloned().unwrap_or_default())
                }
                SorteNoeudPage::Choix { options, .. } => ("choix", options.len().to_string()),
                autre @ (SorteNoeudPage::Colonne
                | SorteNoeudPage::Rangee
                | SorteNoeudPage::Grille { .. }
                | SorteNoeudPage::Pile
                | SorteNoeudPage::Image { .. }
                | SorteNoeudPage::CompteARebours { .. }
                | SorteNoeudPage::Croix { .. }) => (Sorte::de(autre).nom(), String::new()),
            };
            format!(
                "{{\"cle\":\"{:016x}\",\"sorte\":\"{sorte}\",\"nom\":{}}}",
                n.cle.fil(),
                json(&nom)
            )
        })
        .collect();
    format!("[{}]", v.join(","))
}

fn centre_json(t: &Tel, cle: &str) -> String {
    let el = if cle == "reprendre" {
        Some(t.reprendre.clone())
    } else {
        u64::from_str_radix(cle, 16)
            .ok()
            .and_then(|c| t.elements.get(&Cle::du_fil(c)))
            .map(|(_, e)| e.clone())
    };
    let Some(el) = el else {
        return String::from("null");
    };
    el.scroll_into_view_with_bool(false);
    let r = el.get_bounding_client_rect();
    format!(
        "{{\"x\":{:.1},\"y\":{:.1},\"l\":{:.1},\"h\":{:.1}}}",
        r.left() + r.width() / 2.0,
        r.top() + r.height() / 2.0,
        r.width(),
        r.height()
    )
}

/// Le scénario `pages` (§ 8.2) : un élément qui déborde de sa boîte (`scrollWidth` contre
/// `clientWidth`), ou de la largeur de la page, se nomme par sa clé.
fn debordements_json(t: &Tel) -> String {
    let largeur = t
        .document
        .document_element()
        .map_or(0, |d| d.client_width());
    let mut v = Vec::new();
    for (cle, (sorte, el)) in &t.elements {
        let (sw, cw) = (el.scroll_width(), el.client_width());
        let r = el.get_bounding_client_rect();
        let hors = r.right() > f64::from(largeur) + 1.0 || r.left() < -1.0;
        if sw > cw.saturating_add(1) || hors {
            v.push(format!(
                "{{\"cle\":\"{:016x}\",\"sorte\":\"{}\",\"scroll\":{sw},\"client\":{cw},\"droite\":{:.1},\"largeur\":{largeur}}}",
                cle.fil(),
                sorte.nom(),
                r.right()
            ));
        }
    }
    let page = t
        .document
        .document_element()
        .map_or((0, 0), |d| (d.scroll_width(), d.client_width()));
    format!(
        "{{\"elements\":[{}],\"page\":[{},{}]}}",
        v.join(","),
        page.0,
        page.1
    )
}

/// Le scénario `pages` (§ 8.2 ; lot 12, fin) : le banc pose ici, sans salle ni relais, les
/// pages de pire cas que le module construit depuis ses limites. `quoi` dit ce que sont les
/// octets reçus par `__banc.morceau` : `module` (le module livré, réécrit et compilé comme
/// celui que la connexion apporte, et la page faite dessus), `ressource` (une ressource de
/// la page, de clé `cle` en hexadécimal) ou `vue` (une vue de téléphone, que la page reçoit
/// comme celle d'une soirée, à la taille d'écran du moment). Rend du JSON : `ok`, ou
/// `erreur`.
fn pire_cas(t: &mut Tel, quoi: &str, cle: &str) -> String {
    let octets = core::mem::take(&mut t.morceaux);
    t.pire_cas = true;
    montrer_l_ecran(t);
    let r = match quoi {
        "module" => pire_cas_module(t, &octets),
        "ressource" => u64::from_str_radix(cle, 16)
            .map_err(|e| format!("clé illisible : {e}"))
            .map(|c| {
                ajouter_une_ressource(t, c, &octets);
                String::new()
            }),
        "vue" => pire_cas_vue(t, &octets),
        autre => Err(format!("pire cas : « {autre} » inconnu")),
    };
    match r {
        Ok(plus) => format!("{{\"ok\":true{plus}}}"),
        Err(e) => format!("{{\"erreur\":{}}}", json(&e)),
    }
}

fn pire_cas_module(t: &mut Tel, octets: &[u8]) -> Result<String, String> {
    let charge = moteur_navigateur::charger(octets, &reecriture::Reglages::PHASE_3)
        .map_err(|e| format!("le module est refusé : {e:?}"))?;
    let instance = InstanceDePage::ouvrir(charge.fabrique())
        .map_err(|e| format!("l'instance de la page : {e}"))?;
    let tenu = match &instance.declaration().telephones {
        ApplicableFil::Oui(tel) => Some(tel.tenu.clone()),
        ApplicableFil::SansObjet(_) => None,
    };
    let ecran = l_ecran(&t.fenetre, &t.document);
    t.page = Some(PageDuTelephone::new(
        instance,
        Locaux::new(ecran, t.capacites),
        tenu.as_ref(),
    ));
    t.images.clear();
    t.pas_du_pire_cas = 0;
    Ok(String::new())
}

fn pire_cas_vue(t: &mut Tel, montre: &[u8]) -> Result<String, String> {
    let now = maintenant_us(&t.fenetre);
    let ecran = l_ecran(&t.fenetre, &t.document);
    t.pas_du_pire_cas = t.pas_du_pire_cas.saturating_add(1);
    let pas = jeu_abi::Pas::du_fil(t.pas_du_pire_cas);
    let p = t.page.as_mut().ok_or("pas de module")?;
    let mut r = p.ecran(now, ecran);
    let r2 = p.vue_recue(now, pas, montre);
    if let Some(f) = r2.faute {
        return Err(format!(
            "la page de cette vue est en faute ou invalide : {f}"
        ));
    }
    r.page_changee |= r2.page_changee;
    let noeuds = p.page().map_or(0, |pg| pg.noeuds.len());
    t.a_poser = true;
    appliquer(t, &r);
    Ok(format!(",\"noeuds\":{noeuds}"))
}

fn crochets_du_banc(etat: &Etat) -> Result<(), String> {
    let objet = js_sys::Object::new();
    let fenetre = etat.borrow().fenetre.clone();
    let poser = |nom: &str, f: &js_sys::Function| {
        let _: Result<bool, JsValue> = js_sys::Reflect::set(&objet, &JsValue::from_str(nom), f);
    };
    let e = Rc::clone(etat);
    let etat_f = Closure::<dyn Fn() -> String>::new(move || {
        e.try_borrow()
            .map_or_else(|_| String::from("null"), |t| etat_json(&t))
    });
    poser("etat", etat_f.as_ref().unchecked_ref());
    etat_f.forget();
    let e = Rc::clone(etat);
    let noeuds = Closure::<dyn Fn() -> String>::new(move || {
        e.try_borrow()
            .map_or_else(|_| String::from("[]"), |t| noeuds_json(&t))
    });
    poser("noeuds", noeuds.as_ref().unchecked_ref());
    noeuds.forget();
    let e = Rc::clone(etat);
    let centre = Closure::<dyn Fn(String) -> String>::new(move |c: String| {
        e.try_borrow()
            .map_or_else(|_| String::from("null"), |t| centre_json(&t, &c))
    });
    poser("centre", centre.as_ref().unchecked_ref());
    centre.forget();
    let e = Rc::clone(etat);
    let deb = Closure::<dyn Fn() -> String>::new(move || {
        e.try_borrow()
            .map_or_else(|_| String::from("null"), |t| debordements_json(&t))
    });
    poser("debordements", deb.as_ref().unchecked_ref());
    deb.forget();
    let e = Rc::clone(etat);
    let geler = Closure::<dyn Fn(f64) -> bool>::new(move |ms: f64| {
        e.try_borrow_mut().is_ok_and(|mut t| {
            t.gele_jusqu_a = maintenant_ms(&t.fenetre) + ms;
            true
        })
    });
    poser("geler", geler.as_ref().unchecked_ref());
    geler.forget();
    let e = Rc::clone(etat);
    let deborder = Closure::<dyn Fn() -> bool>::new(move || {
        e.try_borrow_mut().is_ok_and(|mut t| {
            t.deborde = true;
            crate::page::poser(&mut t);
            true
        })
    });
    poser("deborder", deborder.as_ref().unchecked_ref());
    deborder.forget();
    let e = Rc::clone(etat);
    let morceau = Closure::<dyn Fn(String) -> bool>::new(move |h: String| {
        e.try_borrow_mut().is_ok_and(|mut t| match lire_hexa(&h) {
            Some(o) => {
                t.morceaux.extend_from_slice(&o);
                true
            }
            None => false,
        })
    });
    poser("morceau", morceau.as_ref().unchecked_ref());
    morceau.forget();
    let e = Rc::clone(etat);
    let pc = Closure::<dyn Fn(String, String) -> String>::new(move |quoi: String, cle: String| {
        e.try_borrow_mut().map_or_else(
            |_| String::from("{\"erreur\":\"occupée\"}"),
            |mut t| pire_cas(&mut t, &quoi, &cle),
        )
    });
    poser("pire_cas", pc.as_ref().unchecked_ref());
    pc.forget();
    js_sys::Reflect::set(&fenetre, &JsValue::from_str("__banc"), &objet)
        .map_err(|_| "window.__banc")?;
    Ok(())
}
