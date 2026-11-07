//! Ce que le joueur fait sur la télé, et ce que le jeu demande à la page (PHASE-3, § 2.9 ;
//! lot 11) : les entrées de l'écran (clic d'un bouton, raccourcis du clavier, glisser,
//! champs, fichiers choisis, déposés ou collés), la page qui défile (`FormeEcran::Page`),
//! les composants posés sur la toile, les ressources que le jeu a reçues en partie, et
//! les demandes `Image`, `Lire`, `Rendu` et `Telecharger`.
//!
//! Les événements du DOM vont dans une file ([`Evenement`]) ; la page les traite dès
//! qu'elle ne tient pas déjà son état. Ils ne décident de rien : chaque entrée part au
//! Worker, que le cœur vérifie (SAL-152).

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "des pixels et des heures du navigateur (des f64) ramenés aux types du rendu et du fil ; rien ne revient à une partie que par les entrées, que le cœur vérifie"
)]

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;
use std::rc::Rc;

use jeu_abi::{
    Cle, CodeTouche, Composant, ComposantVuFil, DemandeFil, Deplace, EchecDemande, EcranFil,
    FinSaisie, FormatImage, FormeEcran, IdDemande, IdGeste, ImageRendue, Mods, Oui, PhasePointeur,
    PhaseTouche, Reponse, SceneFil, SorteNoeud, Touche,
};
use js_sys::{Function, Promise};
use rendu::{Boite, Cadre, Contexte};
use salle::ecran::DeLaPage;
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{Document, Event, HtmlElement, Window};

use crate::cibles::{self, Modale};
use crate::composants::{Composants, Evenement, FileEvenements, Sorte};
use crate::demandes::{self, Fichiers};
use crate::deplacables::{Glisser, Plateau, defilement};
use crate::page::{Etat, Page, banc, chaine, heure_us, poster};
use crate::toile::{attendre, maintenant_ms, texte_d_erreur};

/// La hauteur d'une bande d'un rendu hors écran, en pixels (choisie) : une bande par
/// image présentée (TL-57).
const BANDE: u32 = 1024;
/// Une image plus longue que ceci, pendant un rendu hors écran, est comptée lente (le
/// budget d'une image du CONTRAT, 33 ms).
const IMAGE_LENTE_MS: f64 = 33.4;

/// La géométrie de la dernière scène : les boîtes de ses nœuds en pixels de la toile, la
/// page à son haut.
#[derive(Default)]
pub(crate) struct Geometrie {
    pub boites: Vec<Option<Boite>>,
    pub modale: Option<Modale>,
    pub plateau: Option<Plateau>,
}

/// Les images d'un rendu hors écran en cours : la plus longue, et les lentes (M8).
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct MesureImages {
    pub derniere_ms: f64,
    pub plus_longue_ms: f64,
    pub lentes: u32,
    pub images: u32,
}

/// Un téléchargement qui attend le bouton du commun (P24).
struct EnAttente {
    id: IdDemande,
    nom: String,
    octets: Vec<u8>,
    bouton: HtmlElement,
}

/// L'état des entrées et des demandes de la page.
pub(crate) struct Interaction {
    pub file: FileEvenements,
    pub espaceur: HtmlElement,
    pub composants: Composants,
    pub geo: Geometrie,
    pub glisser: Option<Glisser>,
    /// Le pointeur dans la fenêtre, en pixels du document, pendant un glisser.
    pointeur_fenetre: (f64, f64),
    dernier_defilement_ms: f64,
    /// Un glisser actif vient de finir : le clic qui suit n'est pas un clic.
    glisser_fini: bool,
    pub fichiers: Fichiers,
    /// Les octets des ressources reçues : ceux d'un lien.
    pub ressources: BTreeMap<u64, Rc<Vec<u8>>>,
    /// Le banc arrête les coups des bots (`?bots=non`).
    pub sans_bots: bool,
    /// L'option de test du contrôle négatif de `ecran` : `Telecharger` attend que
    /// l'activation du geste soit perdue (`?fautif=activation-perdue`).
    pub activation_perdue: bool,
    dernier_geste_ms: f64,
    a_telecharger: BTreeMap<u64, EnAttente>,
    rang_suivant: u64,
    pub mesure_images: Option<MesureImages>,
    mac: Oui,
    pub dpr: f64,
    boutons_du_commun: HtmlElement,
}

fn style(e: &web_sys::Element, regles: &[(&str, &str)]) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let s = h.style();
        for (n, v) in regles {
            let _: Result<(), JsValue> = s.set_property(n, v);
        }
    }
}

/// Un `bool` du navigateur en [`Oui`] (une macro : aucun `bool` en paramètre).
macro_rules! oui {
    ($b:expr) => {
        if $b { Oui::Oui } else { Oui::Non }
    };
}

impl Interaction {
    /// Pose l'espaceur (la hauteur de la page, qui défile) et le plan des composants, et
    /// écoute le document et la fenêtre. `traiter` : ce que les écouteurs appellent après
    /// avoir mis un événement dans la file.
    pub(crate) fn new(
        fenetre: &Window,
        document: &Document,
        corps: &HtmlElement,
        traiter: &Rc<dyn Fn()>,
        (sans_bots, activation_perdue): (bool, bool),
    ) -> Result<Interaction, String> {
        let file: FileEvenements = Rc::new(RefCell::new(VecDeque::new()));
        let creer = |balise: &str| -> Result<HtmlElement, String> {
            document
                .create_element(balise)
                .map_err(|e| texte_d_erreur(&e))?
                .dyn_into()
                .map_err(|_| String::from("pas un élément HTML"))
        };
        let espaceur = creer("div")?;
        style(
            &espaceur,
            &[
                ("position", "relative"),
                ("width", "100%"),
                ("height", "100vh"),
                ("z-index", "1"),
                ("user-select", "none"),
                ("-webkit-user-select", "none"),
                ("touch-action", "none"),
            ],
        );
        corps
            .append_child(&espaceur)
            .map_err(|e| texte_d_erreur(&e))?;
        let boutons_du_commun = creer("div")?;
        style(
            &boutons_du_commun,
            &[
                ("position", "fixed"),
                ("right", "1em"),
                ("bottom", "4em"),
                ("z-index", "3"),
                ("display", "flex"),
                ("flex-direction", "column"),
                ("gap", "0.5em"),
            ],
        );
        corps
            .append_child(&boutons_du_commun)
            .map_err(|e| texte_d_erreur(&e))?;
        let mut composants = Composants::new(
            document.clone(),
            espaceur.clone(),
            Rc::clone(&file),
            Rc::clone(traiter),
        );
        composants.poser_police(Cle::POLICE_DU_COMMUN.fil(), mise_en_page::OCTETS_DU_COMMUN);
        let mac = oui!(
            fenetre
                .navigator()
                .platform()
                .unwrap_or_default()
                .contains("Mac")
        );
        let base = (heure_us(fenetre) & 0x00ff_ffff_ffff_ffff).saturating_mul(16);
        let i = Interaction {
            file,
            espaceur,
            composants,
            geo: Geometrie::default(),
            glisser: None,
            pointeur_fenetre: (0.0, 0.0),
            dernier_defilement_ms: 0.0,
            glisser_fini: false,
            fichiers: Fichiers::new(base),
            ressources: BTreeMap::new(),
            sans_bots,
            activation_perdue,
            dernier_geste_ms: f64::NEG_INFINITY,
            a_telecharger: BTreeMap::new(),
            rang_suivant: 0,
            mesure_images: None,
            mac,
            dpr: rapport_de_pixels(fenetre),
            boutons_du_commun,
        };
        ecouter(fenetre, document, &i.file, traiter)?;
        Ok(i)
    }
}

/// Le rapport de pixels de l'appareil, au moins un demi.
fn rapport_de_pixels(f: &Window) -> f64 {
    let r = f.device_pixel_ratio();
    if r > 0.5 { r } else { 0.5 }
}

/// Les écouteurs du document et de la fenêtre.
#[expect(
    clippy::too_many_lines,
    reason = "chaque écouteur de la page se pose ici, à la suite"
)]
fn ecouter(
    fenetre: &Window,
    document: &Document,
    file: &FileEvenements,
    traiter: &Rc<dyn Fn()>,
) -> Result<(), String> {
    let sur = |cible: &web_sys::EventTarget, nom: &str, f: Box<dyn FnMut(Event)>| {
        let c = Closure::<dyn FnMut(Event)>::new(f);
        cible
            .add_event_listener_with_callback(nom, c.as_ref().unchecked_ref())
            .map_err(|e| texte_d_erreur(&e))?;
        c.forget();
        Ok::<(), String>(())
    };
    for (nom, phase) in [
        ("pointerdown", PhasePointeur::Appui),
        ("pointermove", PhasePointeur::Deplacement),
        ("pointerup", PhasePointeur::Relache),
        ("pointercancel", PhasePointeur::Annule),
    ] {
        let (f, t) = (Rc::clone(file), Rc::clone(traiter));
        sur(
            document,
            nom,
            Box::new(move |e: Event| {
                let Some(p) = e.dyn_ref::<web_sys::PointerEvent>() else {
                    return;
                };
                if !e.is_trusted() {
                    return;
                }
                if phase == PhasePointeur::Appui {
                    // Ni sélection de texte, ni glisser du navigateur (CAHIER, 4.7).
                    e.prevent_default();
                }
                f.borrow_mut().push_back(Evenement::Pointeur {
                    phase,
                    id: p.pointer_id(),
                    bouton: p.button(),
                    x: f64::from(p.page_x()),
                    y: f64::from(p.page_y()),
                    y_fenetre: f64::from(p.client_y()),
                });
                t();
            }),
        )?;
    }
    let (f, t) = (Rc::clone(file), Rc::clone(traiter));
    sur(
        document,
        "click",
        Box::new(move |e: Event| {
            let Some(m) = e.dyn_ref::<web_sys::MouseEvent>() else {
                return;
            };
            // Un clic que la page provoque (un lien de téléchargement) n'est pas un geste.
            if !e.is_trusted() {
                return;
            }
            f.borrow_mut().push_back(Evenement::Clic {
                x: f64::from(m.page_x()),
                y: f64::from(m.page_y()),
            });
            t();
        }),
    )?;
    for (nom, phase) in [
        ("keydown", PhaseTouche::Enfoncee),
        ("keyup", PhaseTouche::Relachee),
    ] {
        let (f, t) = (Rc::clone(file), Rc::clone(traiter));
        sur(
            document,
            nom,
            Box::new(move |e: Event| {
                let Some(k) = e.dyn_ref::<web_sys::KeyboardEvent>() else {
                    return;
                };
                if k.code() == "F8" || !e.is_trusted() {
                    return;
                }
                let champ = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.active_element())
                    .is_some_and(|a| a.has_attribute("data-soiree-champ"));
                f.borrow_mut().push_back(Evenement::Touche {
                    code: k.code(),
                    mods: Mods {
                        maj: oui!(k.shift_key()),
                        ctrl: oui!(k.ctrl_key()),
                        alt: oui!(k.alt_key()),
                        meta: oui!(k.meta_key()),
                    },
                    repetition: oui!(k.repeat()),
                    phase,
                    composition: oui!(k.is_composing() || k.key_code() == 229),
                    dans_un_champ: oui!(champ),
                    evenement: k.clone(),
                });
                t();
            }),
        )?;
    }
    // Un fichier lâché ne remplace jamais la page (TL-53).
    sur(
        document,
        "dragover",
        Box::new(|e: Event| e.prevent_default()),
    )?;
    let (f, t) = (Rc::clone(file), Rc::clone(traiter));
    sur(
        document,
        "drop",
        Box::new(move |e: Event| {
            e.prevent_default();
            let fichiers = e
                .dyn_ref::<web_sys::DragEvent>()
                .and_then(web_sys::DragEvent::data_transfer)
                .and_then(|d| d.files())
                .map(|l| crate::composants::liste(&l))
                .unwrap_or_default();
            if !fichiers.is_empty() {
                f.borrow_mut().push_back(Evenement::Fichiers {
                    selecteur: None,
                    fichiers,
                });
                t();
            }
        }),
    )?;
    let (f, t) = (Rc::clone(file), Rc::clone(traiter));
    sur(
        document,
        "paste",
        Box::new(move |e: Event| {
            let fichiers = e
                .dyn_ref::<web_sys::ClipboardEvent>()
                .and_then(web_sys::ClipboardEvent::clipboard_data)
                .and_then(|d| d.files())
                .map(|l| crate::composants::liste(&l))
                .unwrap_or_default();
            // Sans fichier, le collage reste un collage de texte (CAHIER, 4.5).
            if !fichiers.is_empty() {
                e.prevent_default();
                f.borrow_mut().push_back(Evenement::Fichiers {
                    selecteur: None,
                    fichiers,
                });
                t();
            }
        }),
    )?;
    let (f, t) = (Rc::clone(file), Rc::clone(traiter));
    sur(
        fenetre,
        "scroll",
        Box::new(move |_| {
            f.borrow_mut().push_back(Evenement::Defilement);
            t();
        }),
    )?;
    let (f, t) = (Rc::clone(file), Rc::clone(traiter));
    sur(
        fenetre,
        "resize",
        Box::new(move |_| {
            f.borrow_mut().push_back(Evenement::Taille);
            t();
        }),
    )?;
    Ok(())
}

/// Poste une entrée de l'écran au Worker.
fn entree(p: &Page, quoi: EcranFil<'_>) {
    let vu = p
        .ecran
        .as_ref()
        .map_or(jeu_abi::Pas::ZERO, crate::toile::Ecran::pas);
    poster(
        p,
        &DeLaPage::Ecran {
            quoi,
            vu,
            t_us: heure_us(&p.fenetre),
        },
    );
}

/// Traite les événements de la file, tant que la page ne tient pas déjà son état.
pub(crate) fn traiter(etat: &Etat) {
    loop {
        let Ok(mut p) = etat.try_borrow_mut() else {
            return;
        };
        let Some(ev) = p.jeu.file.borrow_mut().pop_front() else {
            return;
        };
        traiter_un(&mut p, ev);
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "chaque événement de la page se traite ici, d'un seul match"
)]
fn traiter_un(p: &mut Page, ev: Evenement) {
    match ev {
        Evenement::Saisie { cle, texte, fin } => {
            let Some(v) = p.jeu.composants.vus.get_mut(&cle) else {
                return;
            };
            let (unite, limite) = (v.unite, v.limite);
            let t = v.champ.envoyer(&texte, unite, limite).to_owned();
            let action = match (v.sorte, fin) {
                (Sorte::Champ, FinSaisie::Entree) => v.fin.clone(),
                _ => None,
            };
            entree(
                p,
                EcranFil::Composant(Composant::Saisie {
                    champ: Cle::du_fil(cle),
                    texte: &t,
                    fin,
                }),
            );
            if let Some(a) = action {
                entree(p, EcranFil::Action(&a));
            }
        }
        Evenement::Fichiers {
            selecteur,
            fichiers,
        } => fichiers_recus(p, selecteur, fichiers),
        Evenement::Pointeur {
            phase,
            id,
            bouton,
            x,
            y,
            y_fenetre,
        } => pointeur(p, phase, (id, bouton), (x, y), y_fenetre),
        Evenement::Clic { x, y } => {
            p.jeu.dernier_geste_ms = maintenant_ms();
            if core::mem::take(&mut p.jeu.glisser_fini) {
                return;
            }
            let dpr = p.jeu.dpr;
            let point = ((x * dpr) as f32, (y * dpr) as f32);
            let action = p.ecran.as_ref().and_then(|e| {
                let s = e.scene()?;
                cibles::clic(s, &p.jeu.geo.boites, p.jeu.geo.modale.as_ref(), point)
                    .map(<[u8]>::to_vec)
            });
            if let Some(a) = action {
                entree(p, EcranFil::Action(&a));
            }
        }
        Evenement::Touche {
            code,
            mods,
            repetition,
            phase,
            composition,
            dans_un_champ,
            evenement,
        } => {
            if phase == PhaseTouche::Enfoncee {
                p.jeu.dernier_geste_ms = maintenant_ms();
            }
            // Pendant une composition, aucune touche n'est un raccourci (TL-32) ; un
            // raccourci cède au champ qui a le focus (TL-34).
            if composition == Oui::Oui || dans_un_champ == Oui::Oui {
                return;
            }
            if phase == PhaseTouche::Enfoncee
                && code == "Escape"
                && let Some(g) = p.jeu.glisser.take()
            {
                evenement.prevent_default();
                if let Some(d) = g.annuler() {
                    entree(p, EcranFil::Composant(Composant::Deplace(d)));
                }
                p.dessin.sale = true;
                return;
            }
            let Some(scene) = p.ecran.as_ref().and_then(|e| e.scene()).cloned() else {
                return;
            };
            if phase == PhaseTouche::Enfoncee
                && let Some(a) =
                    cibles::raccourci(&scene.raccourcis, &code, mods, repetition, p.jeu.mac)
            {
                evenement.prevent_default();
                entree(p, EcranFil::Action(a));
            }
            if let (jeu_abi::ClavierBrut::Oui { .. }, Some(c)) =
                (scene.clavier_brut, CodeTouche::essayer(&code))
            {
                entree(
                    p,
                    EcranFil::Touche(Touche {
                        code: c,
                        mods,
                        phase,
                        repetition,
                    }),
                );
            }
        }
        Evenement::Telecharger { rang } => {
            p.jeu.dernier_geste_ms = maintenant_ms();
            if let Some(t) = p.jeu.a_telecharger.remove(&rang) {
                t.bouton.remove();
                donner(p, t.id, &t.nom, &t.octets, "bouton");
            }
        }
        Evenement::Defilement => {
            if let Some(id) = p.jeu.glisser.as_ref().map(|g| g.pointeur) {
                // La place visée se recalcule après chaque défilement (TL-40).
                let (cx, cy) = p.jeu.pointeur_fenetre;
                let (sx, sy) = defile(&p.fenetre);
                pointeur(
                    p,
                    PhasePointeur::Deplacement,
                    (id, 0),
                    (cx + sx, cy + sy),
                    cy,
                );
            }
            p.dessin.sale = true;
        }
        Evenement::Taille => {
            regler_la_taille(p);
            geometrie(p);
            p.dessin.sale = true;
        }
    }
}

/// Le défilement de la fenêtre, en pixels du document.
fn defile(f: &Window) -> (f64, f64) {
    (f.scroll_x().unwrap_or(0.0), f.scroll_y().unwrap_or(0.0))
}

fn pointeur(
    p: &mut Page,
    phase: PhasePointeur,
    (id, bouton): (i32, i16),
    (x, y): (f64, f64),
    y_fenetre: f64,
) {
    let dpr = p.jeu.dpr;
    let point = ((x * dpr) as f32, (y * dpr) as f32);
    let (sx, _) = defile(&p.fenetre);
    p.jeu.pointeur_fenetre = (x - sx, y_fenetre);
    let Some(plateau) = p.jeu.geo.plateau.as_ref() else {
        return;
    };
    let mut dire: Option<Deplace> = None;
    match phase {
        PhasePointeur::Appui => {
            p.jeu.dernier_geste_ms = maintenant_ms();
            if bouton == 0 && p.jeu.glisser.is_none() {
                p.jeu.glisser = Glisser::appui(plateau, point, id);
                p.jeu.dernier_defilement_ms = maintenant_ms();
            }
        }
        PhasePointeur::Deplacement => {
            if let Some(g) = p.jeu.glisser.as_mut().filter(|g| g.pointeur == id) {
                dire = g.deplacer(plateau, point);
                p.dessin.sale |= g.actif;
            }
        }
        PhasePointeur::Relache => {
            if let Some(mut g) = p.jeu.glisser.take_if(|g| g.pointeur == id) {
                // Le relâché vise là où le pointeur lâche ; le survol qui en vient ne se
                // dit pas, le dépôt le suit.
                let _: Option<Deplace> = g.deplacer(plateau, point);
                dire = g.relacher(plateau);
                p.jeu.glisser_fini = g.actif;
                p.dessin.sale = true;
            }
        }
        PhasePointeur::Annule => {
            if let Some(g) = p.jeu.glisser.take_if(|g| g.pointeur == id) {
                dire = g.annuler();
                p.dessin.sale = true;
            }
        }
    }
    if let Some(d) = dire {
        entree(p, EcranFil::Composant(Composant::Deplace(d)));
    }
}

/// Le défilement près du bord pendant un glisser (TL-40 à TL-42), à chaque image : la
/// distance dépend du temps écoulé, un réveil compte au plus le plafond.
pub(crate) fn defiler_au_bord(p: &mut Page) {
    let Some((bord, vitesse_max, plafond_s)) = p
        .jeu
        .geo
        .plateau
        .as_ref()
        .map(|pl| (pl.bord, pl.vitesse_max, pl.plafond_s))
    else {
        return;
    };
    if !p.jeu.glisser.as_ref().is_some_and(|g| g.actif) {
        p.jeu.dernier_defilement_ms = maintenant_ms();
        return;
    }
    let maintenant = maintenant_ms();
    let dt = ((maintenant - p.jeu.dernier_defilement_ms) / 1000.0) as f32;
    p.jeu.dernier_defilement_ms = maintenant;
    let dpr = p.jeu.dpr as f32;
    let hauteur = p
        .fenetre
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let y = p.jeu.pointeur_fenetre.1 as f32;
    let dy = defilement(y * dpr, hauteur * dpr, bord, vitesse_max, dt, plafond_s) / dpr;
    if dy.abs() >= 0.01 {
        p.fenetre.scroll_by_with_x_and_y(0.0, f64::from(dy));
    }
}

/// Des fichiers choisis dans un sélecteur, ou déposés ou collés (`None`, s'il y a un
/// sélecteur qui les accepte) : gardés par la page, et leur description au jeu.
fn fichiers_recus(p: &mut Page, selecteur: Option<u64>, fichiers: Vec<web_sys::File>) {
    let Some(scene) = p.ecran.as_ref().and_then(|e| e.scene()) else {
        return;
    };
    // La taille la plus grande admise : celle du sélecteur nommé, ou du plus large des
    // sélecteurs qui acceptent un dépôt ou un collage.
    let taille_max = scene
        .noeuds
        .iter()
        .filter_map(|n| {
            let SorteNoeud::Composant {
                composant:
                    ComposantVuFil::Selecteur {
                        depot_partout,
                        collage,
                        taille_max,
                        ..
                    },
                ..
            } = &n.sorte
            else {
                return None;
            };
            let vise = match selecteur {
                Some(c) => n.cle.fil() == c,
                None => *depot_partout == Oui::Oui || *collage == Oui::Oui,
            };
            vise.then_some(*taille_max)
        })
        .max();
    let Some(taille_max) = taille_max else {
        return;
    };
    let gardes: Vec<web_sys::File> = fichiers
        .into_iter()
        .filter(|f| f.size() <= taille_max as f64)
        .collect();
    if gardes.is_empty() {
        return;
    }
    let geste = IdGeste::du_fil(p.jeu.fichiers.numero());
    let recus: Vec<jeu_abi::FichierRecu> = gardes
        .into_iter()
        .map(|f| p.jeu.fichiers.recevoir(f))
        .collect();
    let n = recus.len();
    entree(
        p,
        EcranFil::Composant(Composant::Fichiers {
            geste,
            selecteur: selecteur.map(Cle::du_fil),
            fichiers: recus,
        }),
    );
    banc(
        p,
        &format!(
            "{{\"quoi\":\"fichiers\",\"selecteur\":{},\"n\":{n}}}",
            selecteur.map_or_else(|| String::from("null"), |c| format!("\"{c:016x}\""))
        ),
    );
}

/// La fenêtre a changé : la toile à sa taille, en pixels de l'appareil.
pub(crate) fn regler_la_taille(p: &mut Page) {
    let dpr = rapport_de_pixels(&p.fenetre);
    p.jeu.dpr = dpr;
    let l = p
        .fenetre
        .inner_width()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(1280.0);
    let h = p
        .fenetre
        .inner_height()
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(720.0);
    if let Some(e) = p.ecran.as_mut() {
        e.regler_taille((l * dpr) as u32, (h * dpr) as u32);
        e.echelle_page = dpr as f32;
    }
    if let Some(t) = p.toile.as_ref() {
        style(
            t,
            &[("width", &format!("{l}px")), ("height", &format!("{h}px"))],
        );
    }
}

/// La géométrie de la dernière scène, et ce qui en dépend : les composants posés, la
/// hauteur de la page qui défile, un glisser dont l'objet a disparu (Reset, restauration).
pub(crate) fn geometrie(p: &mut Page) {
    let Some(e) = p.ecran.as_ref() else {
        return;
    };
    let Some(scene) = e.scene() else {
        return;
    };
    let cadre = e.cadre_fixe();
    let boites = rendu::boites(scene, &cadre, &e.contexte);
    let modale = cibles::modale(scene, &cadre, &e.contexte);
    let echelle = cadre.transformation().echelle;
    let plateau = crate::deplacables::Plateau::de_la_scene(scene, &boites, echelle);
    let dpr = p.jeu.dpr as f32;
    let bas = boites
        .iter()
        .flatten()
        .map(|b| b[3])
        .fold(0.0f32, |a, b| if b > a { b } else { a });
    let page = matches!(e.forme(), FormeEcran::Page { .. });
    let corps = p.document.body();
    if page {
        let haut = format!("{}px", (bas / dpr).ceil());
        style(&p.jeu.espaceur, &[("height", &haut)]);
        if let Some(c) = corps.as_ref() {
            style(c, &[("overflow-y", "auto"), ("overflow-x", "hidden")]);
        }
    } else {
        style(&p.jeu.espaceur, &[("height", "100vh")]);
        if let Some(c) = corps.as_ref() {
            style(c, &[("overflow", "hidden")]);
        }
    }
    let scene = scene.clone();
    p.jeu.composants.poser(
        &scene,
        &boites,
        modale.as_ref().map(|m| m.indice),
        (dpr, echelle),
        &p.jeu.ressources,
    );
    if let Some(g) = p.jeu.glisser.as_ref()
        && plateau.as_ref().is_none_or(|pl| !pl.a_l_objet(g.objet))
    {
        // L'objet a disparu (Reset, restauration, fenêtre ouverte) : le glisser s'arrête
        // sans rien dire, le jeu l'a déjà défait.
        p.jeu.glisser = None;
    }
    p.jeu.geo = Geometrie {
        boites,
        modale,
        plateau,
    };
}

/// Avant chaque dessin : le défilement, et ce que le rendu dessine d'un glisser.
pub(crate) fn avant_de_dessiner(p: &mut Page) {
    let (_, sy) = defile(&p.fenetre);
    let dpr = p.jeu.dpr as f32;
    let vu = match (p.jeu.glisser.as_ref(), p.jeu.geo.plateau.as_ref()) {
        (Some(g), Some(pl)) => g.vu(pl),
        _ => None,
    };
    if let Some(e) = p.ecran.as_mut() {
        let defile_px = sy as f32 * dpr;
        e.defilement = if e.echelle_page > 0.0 {
            defile_px / e.echelle_page
        } else {
            0.0
        };
        e.contexte.glisser = vu.map(|mut v| {
            if let Some(m) = v.marque.as_mut() {
                m[1] -= defile_px;
                m[3] -= defile_px;
            }
            v
        });
    }
}

/// Une police reçue : enregistrée pour le DOM (les champs s'écrivent dans sa famille).
pub(crate) fn police_recue(p: &mut Page, cle: u64, octets: &[u8]) {
    p.jeu.composants.poser_police(cle, octets);
}

/// Une image reçue : ses octets gardés (un lien les donne).
pub(crate) fn image_recue(p: &mut Page, cle: u64, octets: &[u8]) {
    let _: Option<Rc<Vec<u8>>> = p.jeu.ressources.insert(cle, Rc::new(octets.to_vec()));
    p.jeu.composants.ressource_arrivee(cle, &p.jeu.ressources);
}

/// Répond à une demande.
fn repondre(p: &Page, id: IdDemande, quoi: Reponse<'_>) {
    poster(p, &DeLaPage::Reponse { id, quoi });
}

fn echec(e: EchecDemande) -> String {
    format!("{e:?}")
}

/// Une demande du jeu (§ 2.9) : chacune répond, réussie ou en échec, et se dit au banc.
#[expect(
    clippy::too_many_lines,
    reason = "chaque sorte de demande se traite ici, d'un seul match"
)]
pub(crate) fn demande(etat: &Etat, p: &mut Page, id: IdDemande, quoi: &DemandeFil<'_>) {
    match quoi {
        DemandeFil::Image {
            fichier,
            carre,
            format,
        } => {
            let fichier = p.jeu.fichiers.fichier(*fichier);
            let (carre, format) = (*carre, *format);
            let suite = Rc::clone(etat);
            spawn_local(async move {
                let debut = maintenant_ms();
                let issue = demandes::image(fichier, carre, format).await;
                let p = suite.borrow();
                let json = format!(
                    "{{\"quoi\":\"demande\",\"sorte\":\"image\",\"id\":{},\"issue\":{},\"detail\":{},\"octets\":{},\"ms\":{:.1}}}",
                    id.fil(),
                    chaine(
                        &issue
                            .as_ref()
                            .map_or_else(|err| echec(err.0), |_| String::from("ok"))
                    ),
                    chaine(issue.as_ref().err().map_or("", |e| e.1.as_str())),
                    issue.as_ref().map_or(0, Vec::len),
                    maintenant_ms() - debut
                );
                banc(&p, &json);
                repondre(
                    &p,
                    id,
                    Reponse::Image(issue.as_deref().map_err(|err| err.0)),
                );
            });
        }
        DemandeFil::Lire { fichier, max } => {
            let fichier = p.jeu.fichiers.fichier(*fichier);
            let max = *max;
            let suite = Rc::clone(etat);
            spawn_local(async move {
                let issue = demandes::lire(fichier, max).await;
                let p = suite.borrow();
                let json = format!(
                    "{{\"quoi\":\"demande\",\"sorte\":\"lire\",\"id\":{},\"issue\":{},\"octets\":{}}}",
                    id.fil(),
                    chaine(
                        &issue
                            .as_ref()
                            .map_or_else(|err| echec(*err), |_| String::from("ok"))
                    ),
                    issue.as_ref().map_or(0, Vec::len)
                );
                banc(&p, &json);
                repondre(&p, id, Reponse::Lu(issue.as_deref().map_err(|err| *err)));
            });
        }
        DemandeFil::Rendu {
            scene,
            echelle,
            format,
        } => {
            let (scene, echelle, format) = (scene.clone(), *echelle, *format);
            let suite = Rc::clone(etat);
            spawn_local(async move {
                let debut = maintenant_ms();
                let issue = rendre(&suite, &scene, echelle, format).await;
                let mut p = suite.borrow_mut();
                let mesure = p.jeu.mesure_images.take().unwrap_or_default();
                let (octets, non) = match &issue {
                    Ok((oct, non_d, _)) => (oct.len(), *non_d),
                    Err(_) => (0, 0),
                };
                let (larg, haut) = issue.as_ref().map_or((0, 0), |(_, _, t)| *t);
                let json = format!(
                    "{{\"quoi\":\"demande\",\"sorte\":\"rendu\",\"id\":{},\"issue\":{},\"octets\":{octets},\"non_dessinees\":{non},\"l\":{larg},\"h\":{haut},\"ms\":{:.1},\"images\":{},\"image_max_ms\":{:.1},\"images_lentes\":{}}}",
                    id.fil(),
                    chaine(
                        &issue
                            .as_ref()
                            .map_or_else(Clone::clone, |_| String::from("ok"))
                    ),
                    maintenant_ms() - debut,
                    mesure.images,
                    mesure.plus_longue_ms,
                    mesure.lentes
                );
                banc(&p, &json);
                let reponse = match &issue {
                    Ok((oct, non_d, _)) => Reponse::Rendu(Ok(ImageRendue {
                        octets: oct,
                        non_dessinees: *non_d,
                    })),
                    Err(_) => Reponse::Rendu(Err(EchecDemande::Interne)),
                };
                repondre(&p, id, reponse);
                p.dessin.sale = true;
            });
        }
        DemandeFil::Telecharger { nom, octets } => {
            let (nom, octets) = (String::from(*nom), octets.to_vec());
            if p.jeu.activation_perdue {
                // L'option de test du contrôle négatif : la page attend que l'activation
                // du geste soit perdue, puis fait comme sans elle.
                let suite = Rc::clone(etat);
                spawn_local(async move {
                    for _ in 0..200 {
                        if !demandes::activation(f64::NEG_INFINITY, maintenant_ms()) {
                            break;
                        }
                        dormir(100).await;
                    }
                    let mut p = suite.borrow_mut();
                    sans_geste(&mut p, &suite, id, nom, octets);
                });
                return;
            }
            if demandes::activation(p.jeu.dernier_geste_ms, maintenant_ms()) {
                donner(p, id, &nom, &octets, "geste");
            } else {
                sans_geste(p, etat, id, nom, octets);
            }
        }
        // L'autorité donne les lignes au relais elle-même.
        DemandeFil::Ligne { .. } => repondre(p, id, Reponse::Fait(Ok(()))),
    }
}

/// Donne le fichier, répond, et le dit au banc.
fn donner(p: &Page, id: IdDemande, nom: &str, octets: &[u8], via: &str) {
    let r = demandes::donner(&p.document, nom, octets);
    let json = format!(
        "{{\"quoi\":\"telechargement\",\"id\":{},\"nom\":{},\"octets\":{},\"via\":\"{via}\",\"issue\":{}}}",
        id.fil(),
        chaine(nom),
        octets.len(),
        chaine(r.as_ref().map_or_else(String::as_str, |()| "ok"))
    );
    banc(p, &json);
    repondre(p, id, Reponse::Fait(r.map_err(|_| EchecDemande::Refuse)));
}

/// `Telecharger` hors d'un geste : un bouton du commun la redemande (P24).
fn sans_geste(p: &mut Page, etat: &Etat, id: IdDemande, nom: String, octets: Vec<u8>) {
    let Ok(element) = p.document.create_element("button") else {
        return;
    };
    let Ok(bouton) = element.dyn_into::<HtmlElement>() else {
        return;
    };
    bouton.set_text_content(Some(&format!("Télécharger « {nom} »")));
    style(
        &bouton,
        &[
            ("font-size", "18px"),
            ("padding", "0.6em 1em"),
            ("background", "#333"),
            ("color", "#fff"),
            ("border", "1px solid #888"),
            ("cursor", "pointer"),
        ],
    );
    let rang = p.jeu.rang_suivant;
    p.jeu.rang_suivant = rang.saturating_add(1);
    let _: Result<(), JsValue> = bouton.set_attribute("data-telechargement", &rang.to_string());
    let (file_, suite) = (Rc::clone(&p.jeu.file), Rc::clone(etat));
    let clic = Closure::<dyn FnMut(Event)>::new(move |ev: Event| {
        ev.stop_propagation();
        if ev.is_trusted() {
            file_
                .borrow_mut()
                .push_back(Evenement::Telecharger { rang });
            traiter(&suite);
        }
    });
    let _: Result<(), JsValue> =
        bouton.add_event_listener_with_callback("click", clic.as_ref().unchecked_ref());
    clic.forget();
    let _: Result<web_sys::Node, JsValue> = p.jeu.boutons_du_commun.append_child(&bouton);
    banc(
        p,
        &format!(
            "{{\"quoi\":\"telechargement-en-attente\",\"id\":{},\"nom\":{}}}",
            id.fil(),
            chaine(&nom)
        ),
    );
    p.jeu.a_telecharger.insert(
        rang,
        EnAttente {
            id,
            nom,
            octets,
            bouton,
        },
    );
}

/// Une promesse tenue après `ms` millisecondes.
async fn dormir(ms: i32) {
    let p = Promise::new(&mut |tenir: Function, _: Function| {
        if let Some(w) = web_sys::window() {
            let _: Result<i32, JsValue> =
                w.set_timeout_with_callback_and_timeout_and_arguments_0(&tenir, ms);
        }
    });
    let _: Result<JsValue, JsValue> = JsFuture::from(p).await;
}

/// Une promesse tenue à la prochaine image (`requestAnimationFrame`).
async fn image_suivante() {
    let p = Promise::new(&mut |tenir: Function, _: Function| {
        if let Some(w) = web_sys::window() {
            let _: Result<i32, JsValue> = w.request_animation_frame(&tenir);
        }
    });
    let _: Result<JsValue, JsValue> = JsFuture::from(p).await;
}

/// `Rendu` (§ 2.9, TL-56, TL-57) : la scène dessinée hors de la toile, par bandes, une
/// image présentée entre deux bandes ; puis encodée. Rend l'image, le nombre de ses images
/// que le rendu n'a pas (TL-56), et sa taille en pixels.
#[expect(
    clippy::too_many_lines,
    reason = "un rendu hors écran se lit d'un bloc : l'étendue, puis chaque bande, puis l'encodage"
)]
async fn rendre(
    etat: &Etat,
    scene: &SceneFil,
    echelle: u8,
    format: FormatImage,
) -> Result<(Vec<u8>, u32, (u32, u32)), String> {
    let ech = f32::from(echelle.max(1));
    let (larg, haut, cote_max, non) = {
        let p = etat.borrow();
        let ecran = p.ecran.as_ref().ok_or("pas d'écran")?;
        // L'étendue de la scène, à l'échelle 1 : le coin haut gauche du repère est celui
        // de l'image.
        let un = Cadre::new(
            1,
            1,
            FormeEcran::Scene {
                largeur: 1,
                hauteur: 1,
                zone_sure_pour_cent: 100,
                echelle_entiere: Oui::Non,
            },
        );
        let boites = rendu::boites(scene, &un, &Contexte::au_repos());
        let (mut lx, mut ly) = (0.0f32, 0.0f32);
        for b in boites.iter().flatten() {
            lx = if b[2] > lx { b[2] } else { lx };
            ly = if b[3] > ly { b[3] } else { ly };
        }
        let non = scene
            .noeuds
            .iter()
            .filter(
                |n| matches!(&n.sorte, SorteNoeud::Image { image, .. } if !ecran.a_l_image(*image)),
            )
            .count();
        (
            (lx * ech).ceil() as u32,
            (ly * ech).ceil() as u32,
            ecran.cote_max(),
            u32::try_from(non).unwrap_or(u32::MAX),
        )
    };
    if larg == 0 || haut == 0 {
        return Err(String::from("une scène vide"));
    }
    if larg > cote_max {
        return Err(format!(
            "{larg} pixels de large, au-delà des {cote_max} d'une texture"
        ));
    }
    let (toile, ctx) = demandes::toile(larg, haut)?;
    let bande = BANDE.min(cote_max).max(1);
    let ceder = Closure::<dyn FnMut() -> Promise>::new(|| {
        Promise::new(&mut |tenir: Function, _: Function| {
            if let Some(w) = web_sys::window() {
                let _: Result<i32, JsValue> =
                    w.set_timeout_with_callback_and_timeout_and_arguments_0(&tenir, 0);
            }
        })
    });
    etat.borrow_mut().jeu.mesure_images = Some(MesureImages {
        derniere_ms: maintenant_ms(),
        ..MesureImages::default()
    });
    let mut y0 = 0u32;
    while y0 < haut {
        let hb = bande.min(haut.saturating_sub(y0));
        let (relecture, appareil) = {
            let mut tenue = etat.borrow_mut();
            let p = &mut *tenue;
            let ecran = p.ecran.as_mut().ok_or("pas d'écran")?;
            let cadre = Cadre {
                echelle_page: ech,
                defilement: y0 as f32 / ech,
                ..Cadre::new(
                    larg,
                    hb,
                    FormeEcran::Page {
                        largeur_max: u16::MAX,
                    },
                )
            };
            let relec = ecran.dessiner_hors_ecran(scene, &cadre)?;
            p.dessin.sale = true;
            (relec, ecran.appareil().clone())
        };
        attendre(
            &appareil,
            &|| relecture.prete().is_some(),
            ceder.as_ref().unchecked_ref(),
            20_000.0,
        )
        .await?;
        let image = relecture.image()?;
        let donnees = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(&image.rgba),
            larg,
            hb,
        )
        .map_err(|e| texte_d_erreur(&e))?;
        ctx.put_image_data(&donnees, 0.0, f64::from(y0))
            .map_err(|e| texte_d_erreur(&e))?;
        y0 = y0.saturating_add(hb);
        // Une image présentée entre deux bandes (TL-57).
        image_suivante().await;
    }
    let octets = demandes::encoder(&toile, format).await?;
    Ok((octets, non, (larg, haut)))
}

/// À chaque image, pendant un rendu hors écran : la plus longue, et les lentes (M8).
pub(crate) fn compter_l_image(p: &mut Page) {
    if let Some(m) = p.jeu.mesure_images.as_mut() {
        let t = maintenant_ms();
        let d = t - m.derniere_ms;
        m.derniere_ms = t;
        m.images = m.images.saturating_add(1);
        if d > m.plus_longue_ms {
            m.plus_longue_ms = d;
        }
        if d > IMAGE_LENTE_MS {
            m.lentes = m.lentes.saturating_add(1);
        }
    }
}

/// Le banc : ce que la page montre et où, en pixels du document (`window.__banc
/// .composants()`) : les boutons, les composants posés, les zones et les objets du
/// glisser, la modale, le pas de la scène.
#[expect(
    clippy::too_many_lines,
    reason = "chaque sorte d'élément que le banc lit s'écrit ici, d'un seul match"
)]
pub(crate) fn composants_au_banc(p: &Page) -> String {
    let Some(e) = p.ecran.as_ref() else {
        return String::from("{}");
    };
    let Some(scene) = e.scene() else {
        return String::from("{}");
    };
    let dpr = p.jeu.dpr as f32;
    let boite = |b: &Boite| {
        format!(
            "\"x\":{:.1},\"y\":{:.1},\"l\":{:.1},\"h\":{:.1}",
            b[0] / dpr,
            b[1] / dpr,
            (b[2] - b[0]) / dpr,
            (b[3] - b[1]) / dpr
        )
    };
    let mut elements = Vec::new();
    for (i, (n, b)) in scene.noeuds.iter().zip(&p.jeu.geo.boites).enumerate() {
        let Some(b) = b else { continue };
        let cle = format!("{:016x}", n.cle.fil());
        let sous = p.jeu.geo.modale.as_ref().is_some_and(|m| i < m.indice);
        let ligne = match &n.sorte {
            SorteNoeud::Bouton { nom, .. } => {
                Some(format!("\"sorte\":\"bouton\",\"nom\":{}", chaine(nom)))
            }
            SorteNoeud::Image { image, .. } => Some(format!(
                "\"sorte\":\"image\",\"image\":\"{:016x}\",\"chargee\":{}",
                image.fil(),
                e.a_l_image(*image)
            )),
            SorteNoeud::Texte { texte, .. } => {
                Some(format!("\"sorte\":\"texte\",\"texte\":{}", chaine(texte)))
            }
            SorteNoeud::Composant { composant, .. } => Some(match composant {
                ComposantVuFil::Champ { valeur, .. } => {
                    format!("\"sorte\":\"champ\",\"valeur\":{}", chaine(valeur))
                }
                ComposantVuFil::EnPlace { valeur, .. } => {
                    format!("\"sorte\":\"en-place\",\"valeur\":{}", chaine(valeur))
                }
                ComposantVuFil::Selecteur {
                    genres, multiple, ..
                } => format!(
                    "\"sorte\":\"selecteur\",\"genres\":{},\"multiple\":{}",
                    chaine(
                        &genres
                            .iter()
                            .map(jeu_abi::TexteCourt::as_str)
                            .collect::<Vec<_>>()
                            .join(",")
                    ),
                    *multiple == Oui::Oui
                ),
                ComposantVuFil::Lien {
                    nom,
                    fichier,
                    ressource,
                } => format!(
                    "\"sorte\":\"lien\",\"nom\":{},\"fichier\":{},\"pret\":{}",
                    chaine(nom.as_str()),
                    chaine(fichier.as_str()),
                    p.jeu.ressources.contains_key(&ressource.fil())
                ),
                ComposantVuFil::Modale { .. } => String::from("\"sorte\":\"modale\""),
                ComposantVuFil::Deplacables { .. } => continue,
            }),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        };
        if let Some(l) = ligne {
            elements.push(format!(
                "{{\"cle\":\"{cle}\",{l},{},\"sous_la_modale\":{sous}}}",
                boite(b)
            ));
        }
    }
    let mut zones = String::new();
    if let Some(pl) = p.jeu.geo.plateau.as_ref() {
        for (k, z) in pl.zones.iter().enumerate() {
            if k > 0 {
                zones.push(',');
            }
            let objets: Vec<String> = z
                .objets
                .iter()
                .map(|(c, b)| format!("{{\"cle\":\"{:016x}\",{}}}", c.fil(), boite(b)))
                .collect();
            let _: core::fmt::Result = write!(
                zones,
                "{{\"cle\":\"{:016x}\",{},\"objets\":[{}]}}",
                z.cle.fil(),
                boite(&z.boite),
                objets.join(",")
            );
        }
    }
    let poubelle = p
        .jeu
        .geo
        .plateau
        .as_ref()
        .and_then(|pl| pl.poubelle)
        .map_or_else(
            || String::from("null"),
            |(c, b)| format!("{{\"cle\":\"{:016x}\",{}}}", c.fil(), boite(&b)),
        );
    let modale = p.jeu.geo.modale.as_ref().map_or_else(
        || String::from("null"),
        |m| format!("{{{}}}", boite(&m.panneau)),
    );
    format!(
        "{{\"pas\":{},\"elements\":[{}],\"zones\":[{zones}],\"poubelle\":{poubelle},\"modale\":{modale},\"glisser\":{},\"attente\":{}}}",
        e.pas().numero(),
        elements.join(","),
        p.jeu.glisser.as_ref().is_some_and(|g| g.actif),
        p.jeu.a_telecharger.len()
    )
}
