//! La coque du Worker, dans le navigateur : `IndexedDB`, `fetch`, les deux WebSocket, la
//! minuterie, `postMessage`. Tout ce qui décide est dans `salle` ; ce qui est ici ne fait
//! que porter des octets et l'heure.

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "l'heure du navigateur (un f64 de millisecondes, positif) ramenée en microsecondes ; rien ne revient à une partie, qui reçoit l'heure en paramètre"
)]

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;

use jeu_abi::{IdGeste, Oui};
use js_sys::{Array, Function, Promise, Uint8Array};
use liaison::CodeSalle;
use moteur_navigateur::{FabriqueNavigateur, ModuleCharge};
use salle::autorite::{Autorite, Cles, ReglagesSalle, Venue};
use salle::chiffre::{CleDeGarde, Hasard, PaireAutorite, SecretDEntree};
use salle::ecran::{CommandeEcran, DeLaPage, PanneSalle, RapportPanne, VersPage};
use salle::en_cours::{Dire, FabriqueQuiDit};
use salle::simulation::forme_et_empreinte;
use wasm_bindgen::prelude::{Closure, wasm_bindgen};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    BinaryType, DedicatedWorkerGlobalScope, IdbDatabase, IdbOpenDbRequest, IdbRequest,
    IdbTransactionMode, MessageEvent, WebSocket,
};

use crate::enregistrement::Enregistrement;

/// La base `IndexedDB` de l'écran, son magasin, et la clé de l'enregistrement.
const BASE: &str = "soiree";
const MAGASIN: &str = "salle";
const CLE: &str = "salle";

/// La minuterie de secours (P6) : toutes les 4 ms ; le pas se joue sur elle quand la page
/// n'a pas donné d'image depuis [`SANS_IMAGE_MS`], un peu plus d'une image à 60 Hz, ou
/// quand attendre l'image ferait manquer deux pas ([`MARGE_SECOURS_MS`]).
/// Choisi au lot 10a : à 16 ms, un Worker ne se réveille qu'environ toutes les 22 ms
/// (mesure 6), et un réveil sur deux pas manqués fait un `Saut` chez un jeu qui ne
/// rattrape qu'un pas (Punchline, `rattrapage: 1`) ; constaté dans Chrome et Firefox,
/// onglet caché (`soiree-banc arriere-plan`). Puis 8 ms ; abaissée à 4 ms après la
/// vérification du lot 10a : le pas se joue au plus 4 ms après son début, et un réveil
/// en retard de 30 ms (constaté dans Firefox, onglet caché, sur une machine calme) ne
/// fait plus manquer deux pas. 4 ms est le plancher des minuteries répétées (HTML,
/// « timer nesting level »).
const MINUTERIE_MS: i32 = 4;
const SANS_IMAGE_MS: f64 = 20.0;

/// La marge du réveil suivant de la minuterie (lot 10a, correction après la
/// vérification) : même avec des images, la minuterie joue le pas dès qu'attendre son
/// prochain réveil (`MINUTERIE_MS`, plus cette marge pour son retard) laisserait l'horloge
/// atteindre le second pas après la définitive ([`Autorite::secours_presse`]). Sans elle,
/// quand l'image tombe tard dans le pas, ou à la dernière image avant que l'onglet ne se
/// cache, le secours ne jouait qu'après 20 à 28 ms sans image, et un jeu qui ne rattrape
/// qu'un pas sautait (constaté dans Chrome et Firefox au moment où l'onglet se cache).
/// Choisi.
const MARGE_SECOURS_MS: i32 = 4;

/// Un rapport au banc par seconde (mode banc seulement).
const RAPPORT_BANC_MS: f64 = 1_000.0;

/// Le banc : un rapport d'état de plus, une fois par point, quand la partie a joué ce
/// nombre de pas depuis le dernier point (vingt avant le suivant, `POINT_TOUS_LES` valant
/// 600) : le pire moment d'une reprise, que M12 attend. Un rapport par seconde seulement
/// tombait, chez Firefox, toujours au même endroit du cycle (566 à 570 pas), et M12
/// attendait en vain (constaté après la vérification du lot 10a).
const PIRE_MOMENT: u64 = 580;

/// Le banc : un tour plus long que ceci se rapporte (`tour-long`), avec ses durées.
const TOUR_LONG_MS: f64 = 10.0;

/// Le banc : deux pas joués à plus que ceci l'un de l'autre se rapportent (`trou`).
const TROU_MS: f64 = 30.0;

type AutoriteDuWorker = Autorite<FabriqueQuiDit<FabriqueNavigateur>, HasardNavigateur>;

/// Le hasard du navigateur : `crypto.getRandomValues` (SAL-93). Un échec se retient : la
/// coque arrête alors l'autorité, plutôt que de chiffrer sur des zéros.
struct HasardNavigateur {
    crypto: web_sys::Crypto,
    echec: Rc<Cell<bool>>,
}

impl Hasard for HasardNavigateur {
    fn remplir(&mut self, octets: &mut [u8]) {
        for morceau in octets.chunks_mut(65_536) {
            if self
                .crypto
                .get_random_values_with_u8_array(morceau)
                .is_err()
            {
                self.echec.set(true);
            }
        }
    }
}

/// Lequel des deux liens au relais.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Bout {
    TempsReel,
    Garde,
}

/// Une WebSocket et ce qui attend qu'elle s'ouvre.
struct Lien {
    ws: WebSocket,
    ouvert: bool,
    en_attente: Vec<Vec<u8>>,
}

impl Lien {
    fn envoyer(&mut self, o: Vec<u8>) {
        if self.ouvert {
            let _: Result<(), JsValue> = self.ws.send_with_u8_array(&o);
        } else {
            self.en_attente.push(o);
        }
    }

    fn ouvrir(&mut self) {
        self.ouvert = true;
        for o in core::mem::take(&mut self.en_attente) {
            let _: Result<(), JsValue> = self.ws.send_with_u8_array(&o);
        }
    }
}

/// Ce que le Worker a mesuré de son démarrage (M12), en millisecondes depuis sa création.
#[derive(Default, Clone, Copy)]
struct Demarrage {
    idb: f64,
    module: f64,
    charge: f64,
    en_jeu: f64,
}

/// Où en est le Worker.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Etape {
    /// Il attend `Demarrer`.
    Attente,
    /// Il charge, puis attend la garde.
    Lance,
    /// La partie est ouverte ou reprise.
    EnJeu,
}

/// La partie figée par le banc.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Figee {
    Non,
    /// Elle ne joue plus ; le journal s'écrit.
    Demandee,
    /// Tout est écrit, et le banc a reçu la forme canonique.
    Rapportee,
}

/// L'état de la coque.
struct Coque {
    global: DedicatedWorkerGlobalScope,
    banc: bool,
    etape: Etape,
    figee: Figee,
    /// Un lien au relais s'est fermé : le Worker se tait, la page le relancera.
    perdu: bool,
    autorite: Option<AutoriteDuWorker>,
    temps_reel: Option<Lien>,
    garde: Option<Lien>,
    base: Option<IdbDatabase>,
    enregistrement: Option<Enregistrement>,
    /// Les messages de la page arrivés avant l'autorité.
    page_en_attente: Vec<Vec<u8>>,
    hasard_en_echec: Rc<Cell<bool>>,
    retarder: (u32, u32),
    dernier_tic_ms: f64,
    derniere_image_ms: f64,
    dernier_rapport_ms: f64,
    demarrage: Demarrage,
    /// Le banc : les pas joués au signal d'une image, et par la minuterie de secours.
    par_image: u64,
    par_minuterie: u64,
    /// Le banc : quand le dernier pas a été joué (`maintenant_ms`).
    dernier_pas_ms: f64,
    /// Le banc : les points écrits quand le pire moment a été rapporté.
    pire_rapporte: Option<u64>,
}

type Etat = Rc<RefCell<Coque>>;

/// Ce qui est positif, ou zéro (`f64::max` est hors des règles d'écriture).
fn positif(x: f64) -> f64 {
    if x > 0.0 { x } else { 0.0 }
}

/// L'heure du Worker, en millisecondes depuis sa création.
fn maintenant_ms(global: &DedicatedWorkerGlobalScope) -> f64 {
    global.performance().map_or(0.0, |p| p.now())
}

/// L'horloge du chronomètre des tours (le banc) : `performance.now()` du Worker.
fn chrono_ms() -> f64 {
    js_sys::global()
        .dyn_into::<DedicatedWorkerGlobalScope>()
        .ok()
        .and_then(|g| g.performance())
        .map_or(0.0, |p| p.now())
}

/// L'heure absolue, `performance.timeOrigin + performance.now()`, en microsecondes (§ 2.7).
fn heure_us(global: &DedicatedWorkerGlobalScope) -> u64 {
    positif(
        global
            .performance()
            .map_or(0.0, |p| (p.time_origin() + p.now()) * 1000.0),
    ) as u64
}

/// Poste un message à la page, son `ArrayBuffer` transféré, sans copie.
fn poster(global: &DedicatedWorkerGlobalScope, message: &VersPage<'_>) {
    if let Ok(o) = message.encoder() {
        poster_octets(global, &o);
    }
}

fn poster_octets(global: &DedicatedWorkerGlobalScope, octets: &[u8]) {
    let tampon = Uint8Array::from(octets).buffer();
    let _: Result<(), JsValue> = global.post_message_with_transfer(&tampon, &Array::of1(&tampon));
}

fn panne(global: &DedicatedWorkerGlobalScope, detail: &str) {
    poster(
        global,
        &VersPage::Panne(RapportPanne {
            quoi: PanneSalle::Outil,
            detail,
        }),
    );
}

fn hexa(octets: &[u8]) -> String {
    octets.iter().fold(String::new(), |mut s, b| {
        let _: core::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

fn texte(valeur: &JsValue) -> String {
    valeur
        .as_string()
        .or_else(|| {
            js_sys::Reflect::get(valeur, &JsValue::from_str("message"))
                .ok()
                .and_then(|m| m.as_string())
        })
        .unwrap_or_else(|| format!("{valeur:?}"))
}

fn octets_de(valeur: &JsValue) -> Option<Vec<u8>> {
    valeur
        .dyn_ref::<js_sys::ArrayBuffer>()
        .map(|b| Uint8Array::new(b).to_vec())
}

/// Le point d'entrée du Worker, que `demarrage.js` appelle une fois la colle chargée. Le
/// Worker écoute la page, puis lui dit qu'il est là (`EnCours(None)` : aucun geste en
/// cours) ; la page répond `Demarrer`.
#[wasm_bindgen]
pub fn demarrer_le_worker() {
    let global: DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
    let etat: Etat = Rc::new(RefCell::new(Coque {
        global: global.clone(),
        banc: false,
        etape: Etape::Attente,
        figee: Figee::Non,
        perdu: false,
        autorite: None,
        temps_reel: None,
        garde: None,
        base: None,
        enregistrement: None,
        page_en_attente: Vec::new(),
        hasard_en_echec: Rc::new(Cell::new(false)),
        retarder: (0, 0),
        dernier_tic_ms: 0.0,
        derniere_image_ms: f64::NEG_INFINITY,
        dernier_rapport_ms: 0.0,
        demarrage: Demarrage::default(),
        par_image: 0,
        par_minuterie: 0,
        dernier_pas_ms: 0.0,
        pire_rapporte: None,
    }));
    let pour_la_page = Rc::clone(&etat);
    let surmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
        if let Some(o) = octets_de(&m.data()) {
            de_la_page(&pour_la_page, &o);
        }
    });
    global.set_onmessage(Some(surmessage.as_ref().unchecked_ref()));
    surmessage.forget();
    poster(&global, &VersPage::EnCours(None));
}

/// Un message de la page.
fn de_la_page(etat: &Etat, octets: &[u8]) {
    let Ok(message) = DeLaPage::decoder(octets) else {
        return;
    };
    match message {
        DeLaPage::Demarrer { banc, module } => {
            let mut coque = etat.borrow_mut();
            if coque.etape != Etape::Attente {
                return;
            }
            coque.etape = Etape::Lance;
            coque.banc = banc == Oui::Oui;
            drop(coque);
            let pour_lancer = Rc::clone(etat);
            spawn_local(async move {
                if let Err(detail) = lancer(&pour_lancer, module).await {
                    let global = pour_lancer.borrow().global.clone();
                    panne(&global, &detail);
                }
            });
        }
        DeLaPage::Commande(CommandeEcran::Figer) => {
            let mut coque = etat.borrow_mut();
            if !coque.banc {
                return;
            }
            coque.figee = Figee::Demandee;
            let heure = heure_us(&coque.global);
            if let Some(autorite) = coque.autorite.as_mut() {
                autorite.ecrire_la_garde(heure);
            }
            drop(coque);
            vider(etat);
        }
        DeLaPage::Commande(CommandeEcran::Retarder { ms, fois }) => {
            let mut coque = etat.borrow_mut();
            if coque.banc {
                coque.retarder = (ms, fois);
            }
        }
        DeLaPage::Image { t_us, .. } => {
            let mut coque = etat.borrow_mut();
            let global = coque.global.clone();
            coque.derniere_image_ms = maintenant_ms(&global);
            if coque.figee != Figee::Non || coque.perdu {
                return;
            }
            let heure = heure_us(&global);
            // Le retard de ce réveil sur l'image qui l'a demandé (§ 2.3, M6).
            let retard_ms = u32::try_from(heure.saturating_sub(t_us) / 1000).unwrap_or(u32::MAX);
            let Some(autorite) = coque.autorite.as_mut() else {
                return;
            };
            let avant = autorite.partie().map(|p| p.definitif().numero());
            autorite.noter_retard(retard_ms);
            autorite.page(heure, octets);
            let rapport = apres_le_tour(&mut coque, "image", avant);
            drop(coque);
            vider(etat);
            if let Some(json) = rapport {
                poster(&global, &VersPage::Banc(&json));
            }
        }
        DeLaPage::Ecran { .. }
        | DeLaPage::Fichiers(_)
        | DeLaPage::Reponse { .. }
        | DeLaPage::Commande(
            CommandeEcran::ChangerDeJeu { .. }
            | CommandeEcran::NouvelleSoiree
            | CommandeEcran::Reprendre { .. }
            | CommandeEcran::Marque
            | CommandeEcran::ExporterLaCle,
        ) => {
            let mut coque = etat.borrow_mut();
            let heure = heure_us(&coque.global);
            if let Some(autorite) = coque.autorite.as_mut() {
                autorite.page(heure, octets);
            } else {
                coque.page_en_attente.push(octets.to_vec());
                return;
            }
            drop(coque);
            vider(etat);
        }
    }
}

// ---------------------------------------------------------------------------
// IndexedDB
// ---------------------------------------------------------------------------

/// Une requête `IndexedDB`, en promesse : son résultat, ou son erreur.
fn en_promesse(requete: &IdbRequest) -> Promise {
    let requete = requete.clone();
    Promise::new(&mut |rendre: Function, refuser: Function| {
        let reussie = requete.clone();
        let reussi = Closure::once(move |_: web_sys::Event| {
            let _: Result<JsValue, JsValue> = rendre.call1(
                &JsValue::NULL,
                &reussie.result().unwrap_or(JsValue::UNDEFINED),
            );
        });
        let ratee = requete.clone();
        let rate = Closure::once(move |_: web_sys::Event| {
            let erreur = ratee
                .error()
                .ok()
                .flatten()
                .map_or(JsValue::from_str("IndexedDB"), JsValue::from);
            let _: Result<JsValue, JsValue> = refuser.call1(&JsValue::NULL, &erreur);
        });
        requete.set_onsuccess(Some(reussi.as_ref().unchecked_ref()));
        requete.set_onerror(Some(rate.as_ref().unchecked_ref()));
        reussi.forget();
        rate.forget();
    })
}

fn erreur_idb(e: &JsValue) -> String {
    format!("IndexedDB : {}", texte(e))
}

async fn ouvrir_la_base(global: &DedicatedWorkerGlobalScope) -> Result<IdbDatabase, String> {
    let fabrique = global
        .indexed_db()
        .map_err(|e| erreur_idb(&e))?
        .ok_or_else(|| String::from("IndexedDB absent de ce Worker"))?;
    let requete: IdbOpenDbRequest = fabrique
        .open_with_u32(BASE, 1)
        .map_err(|e| erreur_idb(&e))?;
    let ouverte = requete.clone();
    let creer = Closure::once(move |_: web_sys::Event| {
        if let Ok(base) = ouverte.result().and_then(JsCast::dyn_into::<IdbDatabase>) {
            let _: Result<web_sys::IdbObjectStore, JsValue> = base.create_object_store(MAGASIN);
        }
    });
    requete.set_onupgradeneeded(Some(creer.as_ref().unchecked_ref()));
    creer.forget();
    let base = JsFuture::from(en_promesse(&requete))
        .await
        .map_err(|e| erreur_idb(&e))?;
    base.dyn_into::<IdbDatabase>()
        .map_err(|_| String::from("IndexedDB : pas une base"))
}

async fn lire_l_enregistrement(base: &IdbDatabase) -> Result<Option<Enregistrement>, String> {
    let transaction = base
        .transaction_with_str(MAGASIN)
        .map_err(|e| erreur_idb(&e))?;
    let magasin = transaction
        .object_store(MAGASIN)
        .map_err(|e| erreur_idb(&e))?;
    let requete = magasin
        .get(&JsValue::from_str(CLE))
        .map_err(|e| erreur_idb(&e))?;
    let valeur = JsFuture::from(en_promesse(&requete))
        .await
        .map_err(|e| erreur_idb(&e))?;
    if valeur.is_undefined() {
        return Ok(None);
    }
    let octets = valeur
        .dyn_ref::<Uint8Array>()
        .map(Uint8Array::to_vec)
        .ok_or_else(|| String::from("l'enregistrement de la salle n'est pas des octets"))?;
    Enregistrement::decoder(&octets)
        .map(Some)
        .map_err(|e| format!("l'enregistrement de la salle ne se relit pas : {e:?}"))
}

/// Écrit l'enregistrement. Les écritures d'un même magasin s'appliquent dans l'ordre de
/// leurs transactions : la plus récente gagne.
fn ecrire_l_enregistrement(
    base: &IdbDatabase,
    enregistrement: &Enregistrement,
) -> Result<IdbRequest, String> {
    let transaction = base
        .transaction_with_str_and_mode(MAGASIN, IdbTransactionMode::Readwrite)
        .map_err(|e| erreur_idb(&e))?;
    let magasin = transaction
        .object_store(MAGASIN)
        .map_err(|e| erreur_idb(&e))?;
    magasin
        .put_with_key(
            &Uint8Array::from(enregistrement.encoder().as_slice()),
            &JsValue::from_str(CLE),
        )
        .map_err(|e| erreur_idb(&e))
}

// ---------------------------------------------------------------------------
// Le démarrage
// ---------------------------------------------------------------------------

async fn charger(global: &DedicatedWorkerGlobalScope, url: &str) -> Result<(u16, Vec<u8>), String> {
    let reponse: web_sys::Response = JsFuture::from(global.fetch_with_str(url))
        .await
        .map_err(|e| format!("{url} : {}", texte(&e)))?
        .dyn_into()
        .map_err(|_| format!("{url} : pas une réponse"))?;
    let statut = reponse.status();
    let tampon = JsFuture::from(reponse.array_buffer().map_err(|e| texte(&e))?)
        .await
        .map_err(|e| format!("{url} : {}", texte(&e)))?;
    Ok((statut, Uint8Array::new(&tampon).to_vec()))
}

/// Le module d'une salle neuve : celui que la page nomme, sinon le premier du catalogue.
async fn module_d_une_salle_neuve(
    global: &DedicatedWorkerGlobalScope,
    demande: Option<[u8; 32]>,
) -> Result<[u8; 32], String> {
    if let Some(module) = demande {
        return Ok(module);
    }
    let (statut, corps) = charger(global, "/jeux/catalogue").await?;
    if statut != 200 {
        return Err(format!("/jeux/catalogue : {statut}"));
    }
    let catalogue = String::from_utf8_lossy(&corps);
    let hex = catalogue
        .split("\"empreinte\":\"")
        .nth(1)
        .and_then(|r| r.get(..64))
        .ok_or_else(|| String::from("le catalogue n'a aucun jeu"))?;
    lire_hexa(hex).ok_or_else(|| format!("empreinte illisible au catalogue : {hex}"))
}

fn lire_hexa(t: &str) -> Option<[u8; 32]> {
    let mut o = [0u8; 32];
    if t.len() != 64 {
        return None;
    }
    for (k, case) in o.iter_mut().enumerate() {
        let debut = k.checked_mul(2)?;
        let chiffres = t.get(debut..debut.checked_add(2)?)?;
        *case = u8::from_str_radix(chiffres, 16).ok()?;
    }
    Some(o)
}

/// Une salle neuve : le code que le relais garde (P41), sinon un code tiré ; des clés
/// neuves (§ 2.6).
async fn salle_neuve(
    global: &DedicatedWorkerGlobalScope,
    hasard: &mut HasardNavigateur,
    module: [u8; 32],
) -> Result<Enregistrement, String> {
    let mut salle = [0u8; 4];
    let (statut, corps) = charger(global, "/salle").await?;
    match statut {
        200 => {
            let code = String::from_utf8_lossy(&corps);
            salle = CodeSalle::lire(code.trim())
                .ok_or_else(|| format!("/salle : un code illisible : {code}"))?
                .0;
        }
        404 => hasard.remplir(&mut salle),
        autre => return Err(format!("/salle : {autre}")),
    }
    let paire =
        PaireAutorite::tirer(hasard).map_err(|e| format!("la paire de l'autorité : {e:?}"))?;
    let secret = SecretDEntree::tirer(hasard);
    let garde = CleDeGarde::tirer(hasard);
    Ok(Enregistrement {
        salle,
        privee: *paire.privee(),
        secret: *secret.octets(),
        garde: *garde.octets(),
        module,
        dernier: None,
    })
}

/// Ouvre un lien au relais, `ws://localhost:<port>/ws`, de la même origine que la page
/// (`connect-src 'self'`, § 2.5).
fn relier(global: &DedicatedWorkerGlobalScope, etat: &Etat, bout: Bout) -> Result<Lien, String> {
    let url = format!("ws://{}/ws", global.location().host());
    let ws = WebSocket::new(&url).map_err(|e| format!("{url} : {}", texte(&e)))?;
    ws.set_binary_type(BinaryType::Arraybuffer);
    let pour_ouvrir = Rc::clone(etat);
    let ouvert = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        let mut coque = pour_ouvrir.borrow_mut();
        let lien = match bout {
            Bout::Garde => coque.garde.as_mut(),
            Bout::TempsReel => coque.temps_reel.as_mut(),
        };
        if let Some(l) = lien {
            l.ouvrir();
        }
    });
    ws.set_onopen(Some(ouvert.as_ref().unchecked_ref()));
    ouvert.forget();
    let pour_recevoir = Rc::clone(etat);
    let recu = Closure::<dyn FnMut(MessageEvent)>::new(move |m: MessageEvent| {
        if let Some(o) = octets_de(&m.data()) {
            du_relais(&pour_recevoir, bout, &o);
        }
    });
    ws.set_onmessage(Some(recu.as_ref().unchecked_ref()));
    recu.forget();
    let pour_fermer = Rc::clone(etat);
    let ferme = Closure::<dyn FnMut(web_sys::Event)>::new(move |_: web_sys::Event| {
        let mut coque = pour_fermer.borrow_mut();
        if coque.perdu {
            return;
        }
        coque.perdu = true;
        let refuse = coque
            .autorite
            .as_ref()
            .and_then(AutoriteDuWorker::arret)
            .is_some();
        let global = coque.global.clone();
        drop(coque);
        // Un refus du relais est déjà dit (`Panne(Relais)`) ; sinon, le lien est perdu, et
        // le Worker se tait : la page le relancera (le chien de garde).
        if !refuse {
            panne(
                &global,
                match bout {
                    Bout::Garde => "le lien de la garde au relais s'est fermé",
                    Bout::TempsReel => "le lien temps réel au relais s'est fermé",
                },
            );
        }
    });
    ws.set_onclose(Some(ferme.as_ref().unchecked_ref()));
    ferme.forget();
    Ok(Lien {
        ws,
        ouvert: false,
        en_attente: Vec::new(),
    })
}

/// L'enregistrement de la salle : celui d'`IndexedDB`, ou celui d'une salle neuve, écrit.
async fn l_enregistrement(
    global: &DedicatedWorkerGlobalScope,
    base: &IdbDatabase,
    hasard: &mut HasardNavigateur,
    module_demande: Option<[u8; 32]>,
) -> Result<Enregistrement, String> {
    if let Some(e) = lire_l_enregistrement(base).await? {
        return Ok(e);
    }
    let module = module_d_une_salle_neuve(global, module_demande).await?;
    let e = salle_neuve(global, hasard, module).await?;
    if hasard.echec.get() {
        return Err(String::from(
            "crypto.getRandomValues a échoué : aucune clé n'est tirée",
        ));
    }
    let _: IdbRequest = ecrire_l_enregistrement(base, &e)?;
    Ok(e)
}

/// Le module de la salle : téléchargé, son empreinte vérifiée, sa taille bornée, réécrit et
/// compilé sous le garde-fou du chargement (§ 2.8, P5).
async fn le_module(
    global: &DedicatedWorkerGlobalScope,
    module: &[u8; 32],
) -> Result<(ModuleCharge, f64), String> {
    let url = format!("/jeux/{}.wasm", hexa(module));
    let (statut, octets) = charger(global, &url).await?;
    if statut != 200 {
        return Err(format!("{url} : {statut}"));
    }
    if u64::try_from(octets.len()).unwrap_or(u64::MAX) > moteur_commun::TAILLE_MODULE_MAX {
        return Err(format!(
            "{url} : {} octets, au-delà de TAILLE_MODULE_MAX ({})",
            octets.len(),
            moteur_commun::TAILLE_MODULE_MAX
        ));
    }
    if blake3::hash(&octets).as_bytes() != module {
        return Err(format!("{url} : l'empreinte ne correspond pas"));
    }
    let telecharge = maintenant_ms(global);
    // Le garde-fou du chargement couvre la réécriture, la compilation, `Bonjour` et
    // `Decrire` (P5).
    poster(global, &VersPage::Chargement(Oui::Oui));
    let charge = moteur_navigateur::charger(&octets, &reecriture::Reglages::PHASE_3)
        .map_err(|r| format!("le module est refusé au chargement : {r:?}"))?;
    poster(global, &VersPage::Chargement(Oui::Non));
    verifier_le_rapport(global, module, &charge).await?;
    Ok((charge, telecharge))
}

/// § 2.8, point 2 (lot 10b) : **l'empreinte réécrite contre le rapport d'épreuve** du
/// module (`GET /jeux/<empreinte>.json`, que `soiree-epreuve --catalogue` écrit sur un code
/// 0) : la réécriture de ce Worker doit donner, octet pour octet, le module que l'épreuve a
/// joué (la même `reecriture::VERSION`, les mêmes réglages `PHASE_3`). Un écart est une
/// panne de l'outil. Un module sans rapport (`404`) n'est joué que par `soiree-relais jouer
/// --non-eprouve`, et l'écran l'affiche « NON ÉPROUVÉ » (P17).
async fn verifier_le_rapport(
    global: &DedicatedWorkerGlobalScope,
    module: &[u8; 32],
    charge: &ModuleCharge,
) -> Result<(), String> {
    let url = format!("/jeux/{}.json", hexa(module));
    let (statut, corps) = charger(global, &url).await?;
    match statut {
        404 => return Ok(()),
        200 => {}
        autre => return Err(format!("{url} : {autre}")),
    }
    let texte = String::from_utf8(corps).map_err(|_| format!("{url} : pas de l'UTF-8"))?;
    let rapport = js_sys::JSON::parse(&texte)
        .map_err(|e| format!("{url} : JSON illisible : {}", self::texte(&e)))?;
    let champ = |nom: &str| js_sys::Reflect::get(&rapport, &JsValue::from_str(nom)).ok();
    let version = champ("reecriture").and_then(|v| v.as_f64());
    if version != Some(f64::from(reecriture::VERSION)) {
        return Err(format!(
            "{url} : le rapport est d'une autre réécriture ({version:?}, ce commun : {}) : l'épreuve est à refaire",
            reecriture::VERSION
        ));
    }
    let attendue = champ("reecrite").and_then(|v| v.as_string());
    let obtenue = hexa(&charge.reecrit().empreinte);
    if attendue.as_deref() != Some(obtenue.as_str()) {
        return Err(format!(
            "{url} : l'empreinte réécrite dans ce navigateur ({obtenue}) n'est pas celle du rapport d'épreuve ({attendue:?}) (§ 2.8)"
        ));
    }
    Ok(())
}

async fn lancer(etat: &Etat, module_demande: Option<[u8; 32]>) -> Result<(), String> {
    let global = etat.borrow().global.clone();
    let echec = Rc::clone(&etat.borrow().hasard_en_echec);
    let crypto = global
        .crypto()
        .map_err(|e| format!("crypto : {}", texte(&e)))?;
    let mut hasard = HasardNavigateur {
        crypto: crypto.clone(),
        echec: Rc::clone(&echec),
    };
    let base = ouvrir_la_base(&global).await?;
    let enregistrement = l_enregistrement(&global, &base, &mut hasard, module_demande).await?;
    let t_idb = maintenant_ms(&global);
    let (charge, t_module) = le_module(&global, &enregistrement.module).await?;
    let t_charge = maintenant_ms(&global);
    poster(
        &global,
        &VersPage::Jeu {
            module: enregistrement.module,
            forme: charge.declaration().ecran,
        },
    );
    let temps_reel = relier(&global, etat, Bout::TempsReel)?;
    let garde = relier(&global, etat, Bout::Garde)?;
    let pour_dire = global.clone();
    let dire: Dire = Rc::new(move |id: Option<IdGeste>| poster(&pour_dire, &VersPage::EnCours(id)));
    let cles = Cles {
        paire: PaireAutorite::depuis_privee(enregistrement.privee)
            .map_err(|e| format!("la paire de l'autorité : {e:?}"))?,
        secret: SecretDEntree::depuis_octets(enregistrement.secret),
        garde: CleDeGarde::depuis_octets(enregistrement.garde),
    };
    let mut reglages = ReglagesSalle::soiree(CodeSalle(enregistrement.salle));
    if etat.borrow().banc {
        reglages.banc = Oui::Oui;
    }
    let mut autorite = Autorite::demarrer(
        FabriqueQuiDit::new(charge.fabrique(), dire),
        cles,
        reglages,
        HasardNavigateur { crypto, echec },
        enregistrement.dernier,
    );
    if reglages.banc == Oui::Oui {
        autorite.chronometrer(chrono_ms);
    }
    let en_attente = {
        let mut coque = etat.borrow_mut();
        coque.temps_reel = Some(temps_reel);
        coque.garde = Some(garde);
        coque.base = Some(base);
        coque.enregistrement = Some(enregistrement);
        coque.autorite = Some(autorite);
        coque.demarrage = Demarrage {
            idb: t_idb,
            module: t_module,
            charge: t_charge,
            en_jeu: 0.0,
        };
        coque.dernier_tic_ms = maintenant_ms(&global);
        core::mem::take(&mut coque.page_en_attente)
    };
    for o in en_attente {
        de_la_page(etat, &o);
    }
    vider(etat);
    let pour_la_minuterie = Rc::clone(etat);
    let minuterie = Closure::<dyn FnMut()>::new(move || tic(&pour_la_minuterie));
    global
        .set_interval_with_callback_and_timeout_and_arguments_0(
            minuterie.as_ref().unchecked_ref(),
            MINUTERIE_MS,
        )
        .map_err(|e| format!("setInterval : {}", texte(&e)))?;
    minuterie.forget();
    Ok(())
}

// ---------------------------------------------------------------------------
// La boucle
// ---------------------------------------------------------------------------

/// Une trame du relais.
fn du_relais(etat: &Etat, bout: Bout, octets: &[u8]) {
    let mut coque = etat.borrow_mut();
    let heure = heure_us(&coque.global);
    let Some(autorite) = coque.autorite.as_mut() else {
        return;
    };
    match bout {
        Bout::Garde => autorite.garde(heure, octets),
        Bout::TempsReel => autorite.temps_reel(heure, octets),
    }
    drop(coque);
    vider(etat);
}

/// La minuterie de secours.
fn tic(etat: &Etat) {
    let mut coque = etat.borrow_mut();
    let global = coque.global.clone();
    let maintenant = maintenant_ms(&global);
    let attendu = coque.dernier_tic_ms + f64::from(MINUTERIE_MS);
    let retard_ms = positif(maintenant - attendu) as u32;
    coque.dernier_tic_ms = maintenant;
    let mut au_banc: Vec<String> = Vec::new();
    if let (ms, fois @ 1..) = coque.retarder {
        // Le banc : un fil gelé, `ms` millisecondes, `fois` réveils de suite (§ 8.3). Le
        // banc reçoit le pas où il gèle : le saut qui suit est voulu, et lui seul.
        coque.retarder = (ms, fois.saturating_sub(1));
        let pas = coque
            .autorite
            .as_ref()
            .and_then(|a| a.partie())
            .map(|p| p.definitif().numero());
        au_banc.push(format!(
            "{{\"quoi\":\"retarde\",\"pas\":{},\"ms\":{ms}}}",
            nombre(pas)
        ));
        let fin = maintenant + f64::from(ms);
        while maintenant_ms(&global) < fin {}
    }
    let sans_image = maintenant - coque.derniere_image_ms >= SANS_IMAGE_MS;
    let heure = heure_us(&global);
    let avance_us = u64::from(MINUTERIE_MS.saturating_add(MARGE_SECOURS_MS).unsigned_abs()) * 1000;
    let presse = coque
        .autorite
        .as_ref()
        .is_some_and(|a| a.secours_presse(heure, avance_us));
    if coque.figee == Figee::Non && !coque.perdu && (sans_image || presse) {
        // Pas d'image (la page est cachée), ou l'image viendrait trop tard : la minuterie
        // fait le pas (P6, § 2.3).
        if let Some(autorite) = coque.autorite.as_mut() {
            let avant = autorite.partie().map(|p| p.definitif().numero());
            autorite.noter_retard(retard_ms);
            autorite.reveiller(heure);
            au_banc.extend(apres_le_tour(&mut coque, "minuterie", avant));
        }
    }
    let mut rapporter = coque.banc && maintenant - coque.dernier_rapport_ms >= RAPPORT_BANC_MS;
    if rapporter {
        coque.dernier_rapport_ms = maintenant;
    }
    if coque.banc
        && let Some(a) = coque.autorite.as_ref()
        && a.depuis_le_point().is_some_and(|d| d >= PIRE_MOMENT)
        && coque.pire_rapporte != Some(a.comptes().points)
    {
        coque.pire_rapporte = Some(a.comptes().points);
        rapporter = true;
    }
    drop(coque);
    vider(etat);
    for json in &au_banc {
        poster(&global, &VersPage::Banc(json));
    }
    if rapporter {
        rapport_d_etat(etat);
    }
}

/// Le banc, après un réveil qui a pu jouer un pas : compte qui l'a joué (l'image ou la
/// minuterie), et rend la ligne d'un tour long ou d'un trou entre deux pas, s'il y en a.
fn apres_le_tour(coque: &mut Coque, via: &str, avant: Option<u64>) -> Option<String> {
    if !coque.banc {
        return None;
    }
    let maintenant = maintenant_ms(&coque.global);
    let autorite = coque.autorite.as_ref()?;
    let apres = autorite.partie().map(|p| p.definitif().numero());
    let d = autorite.durees_du_tour();
    if apres == avant {
        return None;
    }
    if via == "image" {
        coque.par_image = coque.par_image.saturating_add(1);
    } else {
        coque.par_minuterie = coque.par_minuterie.saturating_add(1);
    }
    let trou = maintenant - d.total - coque.dernier_pas_ms;
    let premier = coque.dernier_pas_ms == 0.0;
    coque.dernier_pas_ms = maintenant;
    if d.total < TOUR_LONG_MS && (premier || trou < TROU_MS) {
        return None;
    }
    Some(format!(
        "{{\"quoi\":\"tour\",\"via\":\"{via}\",\"pas\":{},\"trou_ms\":{trou:.1},\"ms\":{:.1},\"avancer\":{:.1},\"vues\":{:.1},\"garde\":{:.1},\"instantane\":{:.1},\"sceller\":{:.1},\"point\":{}}}",
        nombre(apres),
        d.total,
        d.avancer,
        d.vues,
        d.garde,
        d.instantane,
        d.sceller,
        d.point
    ))
}

/// Envoie ce que l'autorité a à envoyer, et suit ce qui en découle : l'entrée en jeu (les
/// ressources de l'écran, la reprise rapportée au banc), le dernier numéro écrit gardé
/// dans `IndexedDB`, la partie figée.
fn vider(etat: &Etat) {
    let mut coque = etat.borrow_mut();
    let global = coque.global.clone();
    let Some(autorite) = coque.autorite.as_mut() else {
        return;
    };
    let sorties = autorite.sorties();
    let dernier = autorite.dernier_ecrit();
    let arretee = autorite.arret().is_some();
    let en_jeu = autorite.partie().is_some();
    let garde_a_jour = autorite.garde_a_jour();
    if let Some(l) = coque.temps_reel.as_mut() {
        for o in sorties.temps_reel {
            l.envoyer(o);
        }
    }
    if let Some(l) = coque.garde.as_mut() {
        for o in sorties.garde {
            l.envoyer(o);
        }
    }
    for o in &sorties.page {
        poster_octets(&global, o);
    }
    if coque.hasard_en_echec.get() && !arretee {
        coque.perdu = true;
        drop(coque);
        panne(
            &global,
            "crypto.getRandomValues a échoué : l'autorité s'arrête",
        );
        return;
    }
    // Le dernier numéro écrit, gardé avec G (P37).
    if let Some(e) = coque.enregistrement.as_mut()
        && dernier > e.dernier
    {
        e.dernier = dernier;
        let e = e.clone();
        if let Some(base) = coque.base.as_ref() {
            let _: Result<IdbRequest, String> = ecrire_l_enregistrement(base, &e);
        }
    }
    if coque.etape == Etape::Lance && en_jeu {
        coque.etape = Etape::EnJeu;
        coque.demarrage.en_jeu = maintenant_ms(&global);
        drop(coque);
        entrer_en_jeu(etat);
        return;
    }
    if coque.figee == Figee::Demandee && garde_a_jour {
        coque.figee = Figee::Rapportee;
        drop(coque);
        rapporter_la_forme(etat, "fige");
    }
}

/// La partie est ouverte ou reprise : le banc reçoit sa forme canonique avant qu'elle
/// n'avance, et les polices et les images de l'écran partent à la page (§ 2.9).
fn entrer_en_jeu(etat: &Etat) {
    let banc = etat.borrow().banc;
    if banc {
        rapporter_la_forme(etat, "en-jeu");
    }
    let mut coque = etat.borrow_mut();
    let global = coque.global.clone();
    let Some(partie) = coque
        .autorite
        .as_mut()
        .and_then(AutoriteDuWorker::partie_mut)
    else {
        return;
    };
    let cles: Vec<u64> = partie
        .declaration()
        .assets
        .iter()
        .map(|a| a.cle.fil())
        .collect();
    for cle in cles {
        if let Ok(ressources) = partie.ressources(&[cle]) {
            for (c, o) in ressources {
                poster(&global, &VersPage::Ressource { cle: c, octets: &o });
            }
        }
    }
}

/// Le banc : la forme canonique et l'empreinte de la partie, d'où elle vient, et le temps
/// du démarrage (M12).
fn rapporter_la_forme(etat: &Etat, quoi: &str) {
    let mut coque = etat.borrow_mut();
    let global = coque.global.clone();
    let mesures = coque.demarrage;
    let Some(autorite) = coque.autorite.as_mut() else {
        return;
    };
    let venue = match autorite.venue() {
        Some(Venue::DeLaGarde { derniere_case }) => format!("{derniere_case}"),
        Some(Venue::Neuve) | None => String::from("null"),
    };
    let dernier = nombre(autorite.dernier_ecrit());
    let Some(partie) = autorite.partie_mut() else {
        return;
    };
    let pas = partie.definitif().numero();
    let forme = match forme_et_empreinte(partie) {
        Ok((f, e)) => format!("\"forme\":\"{}\",\"empreinte\":\"{}\"", hexa(&f), hexa(&e)),
        Err(e) => format!("\"erreur\":{}", chaine(&e)),
    };
    let json = format!(
        "{{\"quoi\":\"{quoi}\",\"relue\":{venue},\"dernier\":{dernier},\"pas\":{pas},{forme},\"ms\":{{\"idb\":{:.1},\"module\":{:.1},\"charge\":{:.1},\"en_jeu\":{:.1}}}}}",
        mesures.idb, mesures.module, mesures.charge, mesures.en_jeu
    );
    drop(coque);
    poster(&global, &VersPage::Banc(&json));
}

/// Le banc, une fois par seconde : où en est la partie.
fn rapport_d_etat(etat: &Etat) {
    let mut coque = etat.borrow_mut();
    let global = coque.global.clone();
    let Some(autorite) = coque.autorite.as_mut() else {
        return;
    };
    let comptes = autorite.comptes();
    let depuis = autorite.depuis_le_point();
    let dernier = autorite.dernier_ecrit();
    let arret = autorite.arret().map(|r| format!("{r:?}"));
    let terminale = autorite
        .partie_mut()
        .and_then(|p| p.etat().ok())
        .is_some_and(|e| e.terminale == Oui::Oui);
    let pas = autorite.partie().map(|p| p.definitif().numero());
    let (par_image, par_minuterie) = (coque.par_image, coque.par_minuterie);
    let json = format!(
        "{{\"quoi\":\"etat\",\"par_image\":{par_image},\"par_minuterie\":{par_minuterie},\"pas\":{},\"terminale\":{terminale},\"depuis_le_point\":{},\"dernier\":{},\"points\":{},\"cases\":{},\"gestes\":{},\"marques\":{},\"ecartes\":{},\"ecran\":{},\"arret\":{}}}",
        nombre(pas),
        nombre(depuis),
        nombre(dernier),
        comptes.points,
        comptes.cases_journal,
        comptes.gestes_places,
        comptes.marques,
        comptes.gestes_ecartes,
        comptes.entrees_ecran,
        arret.map_or_else(|| String::from("null"), |r| chaine(&r)),
    );
    drop(coque);
    poster(&global, &VersPage::Banc(&json));
}

fn nombre(n: Option<u64>) -> String {
    n.map_or_else(|| String::from("null"), |n| n.to_string())
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
