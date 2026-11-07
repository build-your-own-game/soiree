//! Le scénario `soiree-banc dessin` (PHASE-3, § 8.2, lot 9) : les pires cas de chaque jeu
//! sur la télé, dans un vrai navigateur.
//!
//! 1. La règle « mêmes lints » de `web/`, puis, sauf `--sans-construire`, l'exemple
//!    `dessin-banc` de `web/ecran` pour `wasm32-unknown-unknown` et sa colle.
//! 2. **La préparation, en natif.** Chaque module est chargé dans wasmtime (`PHASE_3`),
//!    chacun de ses pires cas construit sur une instance nue, comme E16 le fait ; sa scène,
//!    la forme de l'écran, ses polices et ses images (déclarées, et citées par la scène)
//!    s'écrivent en messages du Worker (`salle::ecran::VersPage`) sous
//!    `target/banc/dessin/`. Pour chaque scène, le banc calcule ce qu'il attend : le
//!    BLAKE3 des octets canoniques de l'agencement de chaque texte (`mise-en-page`, les
//!    mêmes polices dans le même ordre), et quelques points où relire un pixel (OUT-30) :
//!    le centre d'un rectangle opaque que rien ne recouvre, ou le fond là où rien n'est
//!    posé, au repos (OUT-32 : les nœuds animés et ce qu'ils peuvent recouvrir sont
//!    écartés). Un jeu `charge` synthétique (1 000 rectangles et 1 000 textes, puis une
//!    scène vide) mesure le coût d'un nœud dessiné (M8).
//! 3. **Les passages**, chacun dans un navigateur neuf : le moteur de dessin par défaut
//!    (WebGPU s'il est là), avec la perte de l'appareil (`device.destroy()`) après chaque
//!    jeu ; WebGL2 forcé (`?moteur=webgl2`), avec la perte aussi ; puis le contrôle
//!    négatif, l'option de test du rendu qui oublie, dans chaque scène, le nœud que le
//!    banc a choisi : le dernier `Rect` relu dont l'oubli se voit (sous le point relu,
//!    une couleur connue et différente), sinon le dernier `Rect`.
//!    Dans Chrome, une trace de CDP compte les images perdues par le compositeur (à
//!    constater ; Firefox et Safari ne le permettent pas).
//! 4. **Le jugement**, ici seulement : chaque agencement identique au natif, au bit près ;
//!    chaque tampon dessiné identique à son agencement ; chaque point relu, et sur la
//!    toile, à 2 sur 255 près par canal (choisi) de ce que la scène demande ; autant de
//!    durées que d'images demandées ; **aucune image de pire cas
//!    au-delà de 33 ms de travail du GPU fini** (CONTRAT, « Les budgets ») ; la même image
//!    après la perte du GPU ; le moteur de dessin rapporté. Le contrôle négatif doit
//!    faire échouer la relecture d'au moins un pixel dans chaque jeu où l'oubli se voit,
//!    et rien d'autre.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use jeu_abi::{
    Appel, Cle, Fixe, FormeEcran, NoeudFil, Oui, Pas, Rect, RegardFil, Retour, Rgba, SceneFil,
    SorteAppel, SorteAsset, SorteNoeud, StyleTexte, decoder, encoder,
};
use mise_en_page::{MiseEnPage, Polices, Tient, bouton, compte};
use moteur_wasmtime::{Moteur, ReglagesMoteur};
use partie::{Fabrique, Instance};
use salle::ecran::VersPage;
use serde_json::{Value as Json, json};

use crate::horloge::{maintenant, reste};
use crate::navigateur::{Lance, Navigateur};
use crate::serveur::{Requete, Serveur};
use crate::{commun, machine, web};

/// La tolérance d'un pixel relu, par canal (PHASE-3, annexe des valeurs choisies).
pub const TOLERANCE: u8 = 2;

/// Le budget d'une image, en millisecondes de travail du GPU fini (CONTRAT, « Les
/// budgets » : aucune image au-delà de 33 ms en jeu).
pub const BUDGET_IMAGE_MS: f64 = 33.0;

/// L'adresse que porte le QR code des pires cas : la forme d'une vraie (P35, § 2.6).
pub const ADRESSE_QR: &str =
    "http://192.168.1.23:7878/s/3FA9C2D1#k=Q2xlIHB1YmxpcXVlIGRlIGwnYXV0b3JpdMOp&s=c2VjcmV0";

/// La toile de la télé dans le banc, en pixels.
pub const TOILE: (u32, u32) = (1280, 720);

/// Le plus de points relus par scène, hors le fond.
const SONDES_MAX: usize = 24;

/// Le plus de points de fond relus par scène.
const FONDS_MAX: usize = 8;

/// Le plus petit côté d'un rectangle sondé, en unités du repère : son centre reste loin
/// de ses bords même à l'échelle 0,72 de Punchline.
const COTE_SONDE_MIN: i32 = 6;

/// La distance, en unités, qu'un point de fond garde de toute boîte : l'encre d'un glyphe
/// (un jambage, une approche négative) peut sortir un peu de la boîte de son texte.
const MARGE_DU_FOND: f32 = 8.0;

/// Les points essayés par côté d'un rectangle, pour en trouver un que rien ne recouvre.
const POINTS_PAR_COTE: u8 = 7;

/// Le nom du jeu synthétique qui mesure le coût d'un nœud dessiné (M8).
pub const CHARGE: &str = "charge";

/// Les réglages du scénario.
#[derive(Debug, Clone)]
pub struct Options {
    /// Les modules des jeux.
    pub modules: Vec<PathBuf>,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Construire l'exemple et sa colle avant de jouer.
    pub construire: bool,
    /// Combien d'images par scène.
    pub fois: u32,
    /// Le plus long qu'on attend le rapport d'une page.
    pub delai: Duration,
    /// Joue aussi le jeu synthétique `charge` (M8).
    pub charge: Oui,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            modules: Vec::new(),
            navigateurs: vec![Navigateur::Chrome, Navigateur::Firefox, Navigateur::Safari],
            construire: true,
            fois: 3,
            delai: Duration::from_secs(600),
            charge: Oui::Oui,
        }
    }
}

/// Un point où relire un pixel, dans le repère de la scène, et ce qu'on y attend.
#[derive(Debug, Clone, PartialEq)]
pub struct Sonde {
    /// Le point.
    pub x: f32,
    /// Le point.
    pub y: f32,
    /// La couleur attendue.
    pub attendu: Rgba,
    /// Le nœud dont c'est la couleur, ou `None` pour le fond.
    pub noeud: Option<u32>,
    /// Ce point est-il sur le nœud que le contrôle négatif oublie, avec dessous une
    /// couleur connue et différente : son oubli change-t-il ce pixel ?
    pub sensible: Oui,
}

/// Une scène préparée.
#[derive(Debug, Clone)]
pub struct ScenePreparee {
    /// L'indice du pire cas.
    pub indice: u32,
    /// Ses nœuds.
    pub noeuds: usize,
    /// Les points à relire.
    pub sondes: Vec<Sonde>,
    /// L'empreinte de l'agencement de chaque texte, par nœud.
    pub agencements: BTreeMap<u32, String>,
    /// Les ressources propres à la scène (les images citées), en fichiers.
    pub ressources: Vec<String>,
    /// Le fichier de la scène.
    pub fichier: String,
    /// Un pire cas (jugé au budget d'une image), ou la charge synthétique.
    pub pire_cas: Oui,
    /// Le nœud que l'option de test du contrôle négatif oublie ([`noeud_a_oublier`]).
    pub oublie: Option<u32>,
}

/// Un jeu préparé.
#[derive(Debug, Clone)]
pub struct JeuPrepare {
    /// Son nom.
    pub nom: String,
    /// L'empreinte de son module.
    pub empreinte: String,
    /// Le fichier de sa forme.
    pub forme: String,
    /// Ses polices et images déclarées, en fichiers.
    pub ressources: Vec<String>,
    /// Ses scènes.
    pub scenes: Vec<ScenePreparee>,
}

fn encode_scene(v: &SceneFil) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    encoder(v, &mut o).map_err(|e| format!("encodage : {e}"))?;
    Ok(o)
}

fn encode_forme(v: FormeEcran) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    encoder(&v, &mut o).map_err(|e| format!("encodage : {e}"))?;
    Ok(o)
}

fn encode_appel(v: &Appel<'_>) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    encoder(v, &mut o).map_err(|e| format!("encodage : {e}"))?;
    Ok(o)
}

fn ecrire(dossier: &Path, nom: &str, octets: &[u8]) -> Result<String, String> {
    std::fs::write(dossier.join(nom), octets)
        .map_err(|e| format!("{} : {e}", dossier.join(nom).display()))?;
    Ok(String::from(nom))
}

fn unites(f: Fixe) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "une longueur du repère, au 1/256 près, tient dans un f32"
    )]
    let x = f.en_unites() as f32;
    x
}

/// Une boîte en unités : gauche, haut, droite, bas.
type Boite = [f32; 4];

fn boite(r: Rect, dx: f32, dy: f32) -> Boite {
    let (x, y) = (unites(r.x) + dx, unites(r.y) + dy);
    [x, y, x + unites(r.l), y + unites(r.h)]
}

fn contient(b: &Boite, x: f32, y: f32) -> bool {
    x >= b[0] && x < b[2] && y >= b[1] && y < b[3]
}

/// Ce que le banc sait d'un nœud pour choisir ses points : sa boîte (ou les boîtes qu'il
/// peut occuper, s'il est animé), s'il est opaque et posé au repos.
#[derive(Debug, Clone)]
struct Place {
    boites: Vec<Boite>,
    /// Couvre tout : un composant sans boîte, une animation qui change d'échelle.
    partout: bool,
    /// Un `Rect` opaque, ni animé ni sous un parent animé ou translucide : sa couleur.
    rect_sur: Option<Rgba>,
    /// Ne dessine rien (ni ne recouvre).
    rien: bool,
}

/// Ce qu'un nœud transmet à ses enfants : ses décalages possibles, si une échelle a
/// changé, s'il est sûr (opaque, sans animation).
type Herite = (Vec<(f32, f32)>, bool, bool);

/// Les places des nœuds d'une scène, dans le repère, au repos : les groupes décalent ;
/// un nœud animé, ou sous un parent animé, peut être à son départ comme à son arrivée.
fn places(scene: &SceneFil, debordent: &BTreeSet<u32>) -> Vec<Place> {
    let mut herite: Vec<Herite> = Vec::with_capacity(scene.noeuds.len());
    let mut sortie = Vec::with_capacity(scene.noeuds.len());
    for n in &scene.noeuds {
        let (mut decalages, mut echelle, mut sur) = n
            .parent
            .and_then(|p| herite.get(usize::try_from(p).ok()?))
            .cloned()
            .unwrap_or((vec![(0.0, 0.0)], false, true));
        sur = sur && n.opacite == u8::MAX;
        if let Some((_, a)) = &n.anime {
            sur = false;
            echelle = echelle || a.de.echelle_pour_mille != 1000 || a.a.echelle_pour_mille != 1000;
            let mut tous = Vec::new();
            for (x, y) in &decalages {
                for v in [a.de, a.a] {
                    tous.push((x + unites(v.decalage.0), y + unites(v.decalage.1)));
                }
            }
            decalages = tous;
        }
        if let SorteNoeud::Groupe { decalage } = &n.sorte {
            for d in &mut decalages {
                d.0 += unites(decalage.0);
                d.1 += unites(decalage.1);
            }
        }
        let r = match &n.sorte {
            SorteNoeud::Rect { r, .. }
            | SorteNoeud::Image { r, .. }
            | SorteNoeud::Texte { boite: r, .. }
            | SorteNoeud::Bouton { boite: r, .. }
            | SorteNoeud::CompteARebours { boite: r, .. }
            | SorteNoeud::Qr { boite: r }
            | SorteNoeud::Avertissements { boite: r }
            | SorteNoeud::Composant { boite: Some(r), .. } => Some(*r),
            SorteNoeud::Composant { boite: None, .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Definitif
            | SorteNoeud::Surface { .. } => None,
        };
        // `Deplacables` ne dessine rien et ne pose aucun élément : du Rust du commun sur
        // les rectangles dessinés (PHASE-3, § 2.9) ; les autres composants sont des
        // éléments du DOM posés sur leur boîte, ou partout s'ils n'en ont pas.
        let rien = matches!(
            n.sorte,
            SorteNoeud::Groupe { .. }
                | SorteNoeud::Definitif
                | SorteNoeud::Surface { .. }
                | SorteNoeud::Composant {
                    boite: None,
                    composant: jeu_abi::ComposantVuFil::Deplacables { .. },
                }
        );
        // Un texte qui ne tient pas dans sa boîte (E16 le dit) déborde, et son encre peut
        // être n'importe où : il couvre tout.
        let deborde = u32::try_from(sortie.len()).is_ok_and(|k| debordent.contains(&k));
        let partout = !rien && (echelle || r.is_none() || deborde);
        let rect_sur = match &n.sorte {
            SorteNoeud::Rect { couleur, .. } if sur && couleur.3 == u8::MAX => Some(*couleur),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        };
        sortie.push(Place {
            boites: r
                .map(|r| decalages.iter().map(|(x, y)| boite(r, *x, *y)).collect())
                .unwrap_or_default(),
            partout,
            rect_sur,
            rien,
        });
        herite.push((decalages, echelle, sur));
    }
    sortie
}

/// Le nœud le plus haut, parmi `places[..fin]`, qui recouvre le point.
fn le_plus_haut(places: &[Place], fin: usize, x: f32, y: f32) -> Option<usize> {
    places
        .get(..fin)?
        .iter()
        .rposition(|p| !p.rien && (p.partout || p.boites.iter().any(|b| contient(b, x, y))))
}

/// Les points où relire un pixel : dans chaque rectangle opaque, posé au repos, assez
/// grand, un point d'une grille que rien ne recouvre (le premier sous lequel la couleur
/// est connue et différente, sinon le premier) ; puis des points de fond sur une grille
/// du repère visible. Le dernier rectangle dont le point a dessous une couleur connue et
/// différente est celui que le contrôle négatif oublie : son point est `sensible`.
pub fn sondes(
    scene: &SceneFil,
    forme: FormeEcran,
    fond_par_defaut: Rgba,
    debordent: &BTreeSet<u32>,
) -> Vec<Sonde> {
    let places = places(scene, debordent);
    // Ce que la toile du banc montre du repère : tout, pour une scène ; le haut, pour une
    // page (une unité par pixel, sans défilement).
    let (largeur_vue, hauteur_vue) = match forme {
        FormeEcran::Scene {
            largeur, hauteur, ..
        } => (f32::from(largeur), f32::from(hauteur)),
        FormeEcran::Page { largeur_max } => (
            f32::from(largeur_max),
            f32::from(u16::try_from(TOILE.1).unwrap_or(u16::MAX)),
        ),
    };
    let visible = |x: f32, y: f32| x >= 0.0 && y >= 0.0 && x < largeur_vue && y < hauteur_vue;
    let fond = scene.fond.unwrap_or(fond_par_defaut);
    let couleur_sous = |i: usize, x: f32, y: f32| -> Option<Rgba> {
        match le_plus_haut(&places, i, x, y) {
            None => Some(fond),
            Some(j) => places.get(j).and_then(|p| p.rect_sur),
        }
    };
    let eloigne = |a: Rgba, b: Rgba| {
        [(a.0, b.0), (a.1, b.1), (a.2, b.2), (a.3, b.3)]
            .iter()
            .any(|(x, y)| x.abs_diff(*y) > TOLERANCE)
    };
    // Chaque rectangle sondé : son point, sa couleur, et si son oubli s'y verrait.
    let mut rects: Vec<(Sonde, bool)> = Vec::new();
    for (i, (n, p)) in scene.noeuds.iter().zip(&places).enumerate() {
        let (SorteNoeud::Rect { r, .. }, Some(c), [b]) =
            (&n.sorte, p.rect_sur, p.boites.as_slice())
        else {
            continue;
        };
        if r.l < Fixe::unites(COTE_SONDE_MIN) || r.h < Fixe::unites(COTE_SONDE_MIN) {
            continue;
        }
        // Les points d'une grille dans le rectangle, à deux unités au moins de ses bords :
        // visibles, et que rien après lui ne recouvre.
        let libre = |x: f32, y: f32| {
            visible(x, y)
                && !places
                    .iter()
                    .skip(i.saturating_add(1))
                    .any(|q| !q.rien && (q.partout || q.boites.iter().any(|b| contient(b, x, y))))
        };
        let marge = 2.0_f32;
        let libres: Vec<(f32, f32)> = (0..POINTS_PAR_COTE)
            .flat_map(|ky| (0..POINTS_PAR_COTE).map(move |kx| (kx, ky)))
            .map(|(kx, ky)| {
                let fx = (f32::from(kx) + 0.5) / f32::from(POINTS_PAR_COTE);
                let fy = (f32::from(ky) + 0.5) / f32::from(POINTS_PAR_COTE);
                (
                    b[0] + marge + (b[2] - b[0] - 2.0 * marge) * fx,
                    b[1] + marge + (b[3] - b[1] - 2.0 * marge) * fy,
                )
            })
            .filter(|&(x, y)| libre(x, y))
            .collect();
        let se_voit =
            |&(x, y): &(f32, f32)| matches!(couleur_sous(i, x, y), Some(d) if eloigne(d, c));
        let (point, voit) = match libres.iter().find(|p| se_voit(p)) {
            Some(p) => (*p, true),
            None => match libres.first() {
                Some(p) => (*p, false),
                None => continue,
            },
        };
        rects.push((
            Sonde {
                x: point.0,
                y: point.1,
                attendu: c,
                noeud: u32::try_from(i).ok(),
                sensible: Oui::Non,
            },
            voit,
        ));
    }
    // Le nœud que le contrôle négatif oublie : le dernier dont l'oubli se voit.
    if let Some((s, _)) = rects.iter_mut().rev().find(|(_, voit)| *voit) {
        s.sensible = Oui::Oui;
    }
    let mut rects: Vec<Sonde> = rects.into_iter().map(|(s, _)| s).collect();
    // Le sensible d'abord, puis un échantillon régulier des autres.
    rects.sort_by_key(|s| s.sensible != Oui::Oui);
    let pas = (rects.len() / SONDES_MAX).max(1);
    let mut choisies: Vec<Sonde> = rects
        .iter()
        .enumerate()
        .filter(|(k, s)| s.sensible == Oui::Oui || k.checked_rem(pas) == Some(0))
        .map(|(_, s)| s.clone())
        .take(SONDES_MAX)
        .collect();
    choisies.extend(sondes_du_fond(&places, fond, largeur_vue, hauteur_vue));
    choisies
}

/// Le nœud que le contrôle négatif oublie dans une scène : celui du point sensible des
/// [`sondes`], sinon le dernier `Rect` (dont l'oubli ne se voit alors à aucun point relu,
/// et qui ne doit rien changer d'autre).
pub fn noeud_a_oublier(scene: &SceneFil, sondes: &[Sonde]) -> Option<u32> {
    sondes
        .iter()
        .find(|s| s.sensible == Oui::Oui)
        .and_then(|s| s.noeud)
        .or_else(|| {
            scene
                .noeuds
                .iter()
                .rposition(|n| matches!(n.sorte, SorteNoeud::Rect { .. }))
                .and_then(|i| u32::try_from(i).ok())
        })
}

/// Des points de fond, sur une grille du repère visible, là où rien n'est posé.
fn sondes_du_fond(places: &[Place], fond: Rgba, largeur_vue: f32, hauteur_vue: f32) -> Vec<Sonde> {
    let mut choisies = Vec::new();
    let mut fonds: usize = 0;
    'grille: for gy in 1..16_u8 {
        for gx in 1..32_u8 {
            let (x, y) = (
                largeur_vue * f32::from(gx) / 32.0,
                hauteur_vue * f32::from(gy) / 16.0,
            );
            // Loin de toute boîte : l'encre d'un glyphe peut dépasser la sienne un peu.
            let pres_d_un_noeud = places.iter().any(|p| {
                !p.rien
                    && (p.partout
                        || p.boites.iter().any(|b| {
                            let m = MARGE_DU_FOND;
                            contient(&[b[0] - m, b[1] - m, b[2] + m, b[3] + m], x, y)
                        }))
            });
            if !pres_d_un_noeud {
                choisies.push(Sonde {
                    x,
                    y,
                    attendu: fond,
                    noeud: None,
                    sensible: Oui::Non,
                });
                fonds = fonds.saturating_add(1);
                if fonds >= FONDS_MAX {
                    break 'grille;
                }
            }
        }
    }
    choisies
}

/// Les textes que la télé dessine pour une scène, dans l'ordre : chaque `Texte`, le nom
/// de chaque `Bouton` (le style du commun), le texte de chaque `CompteARebours` au pas
/// donné ; et l'empreinte de leur agencement.
pub fn agencements(
    scene: &SceneFil,
    pas: Pas,
    mise_en_page: &mut MiseEnPage,
) -> (BTreeMap<u32, String>, BTreeSet<u32>) {
    let mut sortie = BTreeMap::new();
    let mut debordent = BTreeSet::new();
    for (i, n) in scene.noeuds.iter().enumerate() {
        let (texte, b, style): (String, Rect, StyleTexte) = match &n.sorte {
            SorteNoeud::Texte {
                texte,
                boite,
                style,
            } => (texte.clone(), *boite, *style),
            SorteNoeud::Bouton { boite, nom, .. } => (
                nom.clone(),
                bouton::boite_du_nom(*boite),
                bouton::style_du_nom(*boite),
            ),
            SorteNoeud::CompteARebours {
                compte: c,
                boite,
                style,
            } => (compte::texte(c, pas), *boite, *style),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => continue,
        };
        let a = mise_en_page.agencer(&texte, b, &style);
        let h = blake3::hash(&a.octets_canoniques()).to_hex().to_string();
        if let Ok(k) = u32::try_from(i) {
            sortie.insert(k, h);
            if a.tient != Tient::Oui {
                debordent.insert(k);
            }
        }
    }
    (sortie, debordent)
}

/// Un appel de lecture sur une instance.
fn lire(i: &mut impl Instance, a: &Appel<'_>, budget: u64) -> Result<Vec<u8>, String> {
    i.appeler(SorteAppel::Lecture, &encode_appel(a)?, budget)
        .map_err(|e| e.to_string())
}

/// Le budget d'un appel du banc : le plafond d'un appel (2²⁸), le banc ne juge pas le
/// carburant.
const BUDGET: u64 = 1 << 28;

/// Les polices et les images que le jeu déclare, lues par `Ressources` sur une instance
/// créée, écrites en messages du Worker ; les polices s'ajoutent à `polices`, dans
/// l'ordre de la déclaration, comme la télé les ajoutera.
fn ressources_declarees(
    inst: &mut impl Instance,
    declaration: &jeu_abi::DeclarationFil,
    dos: &Path,
    polices: &mut Polices,
) -> Result<Vec<String>, String> {
    let mut ressources = Vec::new();
    for a in &declaration.assets {
        let r = lire(
            inst,
            &Appel::Ressources {
                cles: vec![a.cle.fil()],
            },
            BUDGET,
        )?;
        if let Ok(Retour::Ressources(v)) = decoder::<Retour<'_>>(&r) {
            for (c, o) in v {
                if a.sorte == SorteAsset::Police {
                    polices
                        .ajouter(a.cle, o.to_vec())
                        .map_err(|e| format!("police {c:016x} : {e}"))?;
                }
                let m = VersPage::Ressource { cle: c, octets: o }
                    .encoder()
                    .map_err(|e| e.to_string())?;
                ressources.push(ecrire(dos, &format!("ressource-{c:016x}.bin"), &m)?);
            }
        }
    }
    Ok(ressources)
}

/// Les images qu'une scène cite hors des assets (les images de Tier list), lues par
/// `Ressources`, écrites en messages du Worker.
fn images_citees(
    inst: &mut impl Instance,
    scene: &SceneFil,
    assets: &[Cle],
    dos: &Path,
    indice: u32,
) -> Result<Vec<String>, String> {
    let citees: Vec<u64> = partie::regles::images_citees(scene)
        .into_iter()
        .filter(|c| !assets.contains(c))
        .map(Cle::fil)
        .collect();
    let mut propres = Vec::new();
    if citees.is_empty() {
        return Ok(propres);
    }
    let r = lire(inst, &Appel::Ressources { cles: citees }, BUDGET)?;
    if let Ok(Retour::Ressources(v)) = decoder::<Retour<'_>>(&r) {
        for (c, o) in v {
            let m = VersPage::Ressource { cle: c, octets: o }
                .encoder()
                .map_err(|e| e.to_string())?;
            propres.push(ecrire(
                dos,
                &format!("ressource-{indice}-{c:016x}.bin"),
                &m,
            )?);
        }
    }
    Ok(propres)
}

/// Prépare un module : ses pires cas, sa forme, ses ressources, ce qu'on attend.
pub fn preparer(module: &Path, dossier: &Path) -> Result<JeuPrepare, String> {
    let octets = std::fs::read(module).map_err(|e| format!("{} : {e}", module.display()))?;
    let nom = module
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("jeu")
        .trim_end_matches("_module")
        .replace('_', "-");
    let empreinte = blake3::hash(&octets).to_hex().to_string();
    let dos = dossier.join(&nom);
    std::fs::create_dir_all(&dos).map_err(|e| format!("{} : {e}", dos.display()))?;
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| e.to_string())?;
    let charge = moteur
        .charger(&octets)
        .map_err(|e| format!("{nom} : {e:?}"))?;
    let declaration = charge.declaration().clone();
    let forme = ecrire(&dos, "forme.bin", &encode_forme(declaration.ecran)?)?;
    let mut polices = Polices::du_commun();
    let mut ressources = Vec::new();
    let mut scenes = Vec::new();
    let assets: Vec<Cle> = declaration.assets.iter().map(|a| a.cle).collect();
    for indice in 0..=u32::from(u16::MAX) {
        let mut inst = Fabrique::neuve(&mut charge.fabrique()).map_err(|p| p.to_string())?;
        for a in [Appel::Bonjour, Appel::Decrire] {
            let _: Vec<u8> = lire(&mut inst, &a, BUDGET)?;
        }
        let r = inst
            .appeler(
                SorteAppel::Ecriture,
                &encode_appel(&Appel::PireCas { indice })?,
                BUDGET,
            )
            .map_err(|e| format!("{nom}, pire cas {indice} : {e}"))?;
        if !matches!(decoder::<Retour<'_>>(&r), Ok(Retour::PireCas { .. })) {
            break;
        }
        if indice == 0 {
            ressources = ressources_declarees(&mut inst, &declaration, &dos, &mut polices)?;
        }
        let r = lire(
            &mut inst,
            &Appel::Regarder {
                regards: vec![RegardFil::Ecran],
            },
            BUDGET,
        )?;
        let Ok(Retour::Vues(v)) = decoder::<Retour<'_>>(&r) else {
            return Err(format!("{nom}, pire cas {indice} : pas de vue"));
        };
        let vue = v
            .first()
            .ok_or_else(|| format!("{nom}, pire cas {indice} : vue vide"))?
            .octets
            .to_vec();
        let scene: SceneFil =
            decoder(&vue).map_err(|e| format!("{nom}, pire cas {indice} : {e}"))?;
        let propres = images_citees(&mut inst, &scene, &assets, &dos, indice)?;
        scenes.push(scene_preparee(
            &dos,
            indice,
            &scene,
            &vue,
            declaration.ecran,
            &polices,
            propres,
            Oui::Oui,
        )?);
    }
    Ok(JeuPrepare {
        nom,
        empreinte,
        forme,
        ressources,
        scenes,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "ce qu'une scène préparée réunit, nommé à l'appel"
)]
fn scene_preparee(
    d: &Path,
    indice: u32,
    scene: &SceneFil,
    octets: &[u8],
    forme: FormeEcran,
    polices: &Polices,
    ressources: Vec<String>,
    pire_cas: Oui,
) -> Result<ScenePreparee, String> {
    let m = VersPage::Scene {
        pas: Pas::ZERO,
        t_us: 0,
        octets,
    }
    .encoder()
    .map_err(|e| e.to_string())?;
    let fichier = ecrire(d, &format!("scene-{indice}.bin"), &m)?;
    let mut mise_en_page = MiseEnPage::new(polices);
    let (agencements, debordent) = agencements(scene, Pas::ZERO, &mut mise_en_page);
    let points = sondes(scene, forme, Rgba::NOIR, &debordent);
    Ok(ScenePreparee {
        oublie: noeud_a_oublier(scene, &points),
        indice,
        noeuds: scene.noeuds.len(),
        sondes: points,
        agencements,
        ressources,
        fichier,
        pire_cas,
    })
}

/// Le jeu synthétique `charge` (M8) : 1 000 rectangles et 1 000 textes courts, puis une
/// scène vide, sur une scène de 1 920 × 1 080.
pub fn preparer_la_charge(dossier: &Path) -> Result<JeuPrepare, String> {
    let d = dossier.join(CHARGE);
    std::fs::create_dir_all(&d).map_err(|e| format!("{} : {e}", d.display()))?;
    let forme = FormeEcran::Scene {
        largeur: 1920,
        hauteur: 1080,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Non,
    };
    let style = StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: jeu_abi::Taille::Fixe(Fixe::unites(12)),
        couleur: Rgba::BLANC,
        alignement: jeu_abi::Alignement::Centre,
        lignes: jeu_abi::Lignes::Une,
        coupe: jeu_abi::Coupe::Mots,
    };
    let mut noeuds = Vec::new();
    for k in 0..1000_i32 {
        let (colonne, ligne) = (k.rem_euclid(40), k.div_euclid(40));
        let (x, y) = (colonne.saturating_mul(48), ligne.saturating_mul(43));
        let r = Rect {
            x: Fixe::unites(x),
            y: Fixe::unites(y),
            l: Fixe::unites(46),
            h: Fixe::unites(41),
        };
        let cle = u64::try_from(k).unwrap_or(0);
        let teinte = u8::try_from(k % 200).unwrap_or(0);
        noeuds.push(NoeudFil::simple(
            Cle::du_fil(cle.saturating_mul(2).saturating_add(1)),
            None,
            SorteNoeud::Rect {
                r,
                couleur: Rgba(teinte, 40, 120, 255),
            },
        ));
        noeuds.push(NoeudFil::simple(
            Cle::du_fil(cle.saturating_mul(2).saturating_add(2)),
            None,
            SorteNoeud::Texte {
                texte: format!("n°{k}"),
                boite: r,
                style,
            },
        ));
    }
    let pleine = SceneFil {
        fond: Some(Rgba(5, 9, 28, 255)),
        noeuds,
        ..SceneFil::vide()
    };
    let vide = SceneFil {
        fond: Some(Rgba(5, 9, 28, 255)),
        ..SceneFil::vide()
    };
    let polices = Polices::du_commun();
    let mut scenes = Vec::new();
    for (indice, s) in [(0, &pleine), (1, &vide)] {
        scenes.push(scene_preparee(
            &d,
            indice,
            s,
            &encode_scene(s)?,
            forme,
            &polices,
            Vec::new(),
            Oui::Non,
        )?);
    }
    Ok(JeuPrepare {
        nom: String::from(CHARGE),
        empreinte: String::new(),
        forme: ecrire(&d, "forme.bin", &encode_forme(forme)?)?,
        ressources: Vec::new(),
        scenes,
    })
}

/// Le manifeste que la page lit.
pub fn manifeste(jeux: &[JeuPrepare], fois: u32) -> Json {
    json!({
        "adresse_qr": ADRESSE_QR,
        "fois": fois,
        "toile": [TOILE.0, TOILE.1],
        "jeux": jeux.iter().map(|j| json!({
            "nom": j.nom,
            "forme": format!("/dessin/{}/{}", j.nom, j.forme),
            "ressources": j.ressources.iter().map(|r| format!("/dessin/{}/{r}", j.nom)).collect::<Vec<_>>(),
            "scenes": j.scenes.iter().map(|s| json!({
                "indice": s.indice,
                "fichier": format!("/dessin/{}/{}", j.nom, s.fichier),
                "ressources": s.ressources.iter().map(|r| format!("/dessin/{}/{r}", j.nom)).collect::<Vec<_>>(),
                "sondes": s.sondes.iter().map(|p| [p.x, p.y]).collect::<Vec<_>>(),
                "oublie": s.oublie,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// Un passage : un moteur, un défaut ou non, la perte du GPU ou non.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sorte {
    /// `auto` ou `webgl2`.
    pub moteur: &'static str,
    /// `""` ou `oublier-un-noeud`.
    pub fautif: &'static str,
    /// Détruire l'appareil après chaque jeu.
    pub perte: Oui,
}

/// Les passages de chaque navigateur.
pub const PASSAGES: [Sorte; 3] = [
    Sorte {
        moteur: "auto",
        fautif: "",
        perte: Oui::Oui,
    },
    Sorte {
        moteur: "webgl2",
        fautif: "",
        perte: Oui::Oui,
    },
    Sorte {
        moteur: "auto",
        fautif: "oublier-un-noeud",
        perte: Oui::Non,
    },
];

/// Ce qu'un passage a donné.
#[derive(Debug, Clone)]
pub struct Passage {
    /// Le navigateur.
    pub navigateur: Navigateur,
    /// Sa version.
    pub version: String,
    /// La sorte de passage.
    pub sorte: Sorte,
    /// Le rapport de la page, s'il est arrivé.
    pub rapport: Option<Json>,
    /// Ce que la console a dit.
    pub console: Vec<String>,
    /// La trace de Chrome : images présentées, perdues, et ce qui l'a dit.
    pub trace: Option<Json>,
    /// Le jugement.
    pub verdict: Result<String, String>,
    /// Ce qui a été constaté (moteur, durées, pixels), pour le rapport.
    pub constats: Vec<String>,
}

/// Compte, dans une trace de Chrome, les images de la composition
/// ([`IMAGES_DE_LA_TRACE`]), et garde un exemple d'image perdue. `DroppedFrame` est
/// l'image que le compositeur dit perdue (constaté au lot 9 dans Chrome sans interface :
/// la catégorie `disabled-by-default-devtools.timeline.frame` la donne ; PHASE-3, § 2.9).
pub fn compter_les_images(evenements: &[Json]) -> Json {
    let mut par_nom: BTreeMap<&str, u64> = IMAGES_DE_LA_TRACE.iter().map(|n| (*n, 0)).collect();
    let mut exemple = Json::Null;
    for e in evenements {
        let Some(nom) = e.get("name").and_then(Json::as_str) else {
            continue;
        };
        if let Some(n) = par_nom.get_mut(nom) {
            *n = n.saturating_add(1);
            if nom == "DroppedFrame" && exemple.is_null() {
                exemple = e.clone();
            }
        }
    }
    json!({
        "evenements": evenements.len(),
        "dropped_frame": par_nom.get("DroppedFrame").copied().unwrap_or(0),
        "par_nom": par_nom,
        "exemple_dropped_frame": exemple,
    })
}

/// Les événements de la trace de Chrome que le banc compte : l'image perdue
/// (`DroppedFrame`), présentée (`FramePresented`, `Display::FrameDisplayed`), et ce que
/// l'ordonnanceur de la composition a sauté.
pub const IMAGES_DE_LA_TRACE: [&str; 6] = [
    "DroppedFrame",
    "FramePresented",
    "Display::FrameDisplayed",
    "PipelineReporter",
    "Scheduler::BeginFrameDropped",
    "Scheduler::MissedBeginFrameDropped",
];

/// Deux pixels proches à `tolerance` près par canal.
fn proche(lu: &Json, attendu: Rgba, tolerance: u8) -> Option<bool> {
    let canaux: Vec<u64> = lu.as_array()?.iter().filter_map(Json::as_u64).collect();
    let [rouge, vert, bleu, alpha] = canaux.as_slice() else {
        return None;
    };
    let pres = |x: u64, y: u8| x.abs_diff(u64::from(y)) <= u64::from(tolerance);
    Some(
        pres(*rouge, attendu.0)
            && pres(*vert, attendu.1)
            && pres(*bleu, attendu.2)
            && pres(*alpha, attendu.3),
    )
}

/// Ce que les sondes lues d'une scène montrent.
#[derive(Debug, Default)]
struct Lecture {
    /// Les points relus sur la toile.
    relues: usize,
    /// Dont ceux qui sont exactement la couleur attendue (0 d'écart, constaté).
    exactes: usize,
    /// Les points rapportés hors de la toile.
    hors: usize,
    /// Les écarts, chacun en une ligne.
    ecarts: Vec<String>,
    /// Les écarts sur une sonde sensible.
    sensibles: usize,
}

/// Juge les sondes lues d'une scène. Une sonde que la page ne rapporte pas, ou rapporte
/// hors de la toile (`null`), est un écart : le banc ne choisit que des points visibles,
/// et la page doit rendre un pixel pour chacun.
fn juger_sondes(lus: &[Json], sondes: &[Sonde], oublie: Option<u32>) -> Lecture {
    let mut l = Lecture::default();
    if lus.len() != sondes.len() {
        l.ecarts.push(format!(
            "{} points rapportés pour {} demandés",
            lus.len(),
            sondes.len()
        ));
    }
    for (k, s) in sondes.iter().enumerate() {
        let Some(lu) = lus.get(k) else {
            continue;
        };
        if proche(lu, s.attendu, 0) == Some(true) {
            l.exactes = l.exactes.saturating_add(1);
        }
        match (lu, proche(lu, s.attendu, TOLERANCE)) {
            (Json::Null, _) => {
                l.hors = l.hors.saturating_add(1);
                l.ecarts.push(format!(
                    "({:.1}, {:.1}) : rapporté hors de la toile",
                    s.x, s.y
                ));
            }
            (_, Some(true)) => l.relues = l.relues.saturating_add(1),
            (lu, _) => {
                l.relues = l.relues.saturating_add(1);
                if s.sensible == Oui::Oui {
                    l.sensibles = l.sensibles.saturating_add(1);
                }
                // Sous le contrôle négatif, un point du nœud oublié peut changer : ce
                // n'est pas un écart du rendu (et il est compté à part s'il est sensible).
                if oublie.is_some() && s.noeud == oublie {
                    continue;
                }
                l.ecarts.push(format!(
                    "({:.1}, {:.1}) {} : lu {lu}, attendu {:?}",
                    s.x,
                    s.y,
                    s.noeud
                        .map_or_else(|| String::from("fond"), |n| format!("nœud {n}")),
                    s.attendu
                ));
            }
        }
    }
    l
}

/// Ce qu'un jeu a montré dans un passage.
#[derive(Debug, Default)]
struct Bilan {
    textes: usize,
    relues: usize,
    exactes: usize,
    hors: usize,
    sensibles: usize,
    sensibles_ratees: usize,
    pire_ms: f64,
}

/// La plus longue de deux durées, écrit à la main (`clippy.toml`).
fn plus_long(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Les agencements rapportés d'une scène, par nœud.
fn agencements_lus(l: &Json) -> BTreeMap<u32, String> {
    let mut lus = BTreeMap::new();
    for a in l
        .get("agencements")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        if let (Some(n), Some(h)) = (
            a.get(0)
                .and_then(Json::as_u64)
                .and_then(|n| u32::try_from(n).ok()),
            a.get(1).and_then(Json::as_str),
        ) {
            lus.insert(n, String::from(h));
        }
    }
    lus
}

/// Juge les durées des images d'une scène : autant que d'images demandées (une durée
/// manquante passerait le budget sans avoir été mesurée), aucune image de pire cas
/// au-delà du budget. Rend les durées lues.
fn juger_durees(
    nom: &str,
    s: &ScenePreparee,
    l: &Json,
    fois: u32,
    bilan: &mut Bilan,
    echecs: &mut Vec<String>,
) -> Vec<f64> {
    let durees: Vec<f64> = l
        .get("durees_ms")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .filter_map(Json::as_f64)
        .collect();
    let pire = durees.iter().copied().fold(0.0_f64, plus_long);
    if durees.len() != usize::try_from(fois).unwrap_or(usize::MAX) {
        echecs.push(format!(
            "{nom}, pire cas {} : {} durée(s) d'image rapportée(s) pour {fois} images demandées",
            s.indice,
            durees.len()
        ));
    }
    if s.pire_cas == Oui::Oui {
        bilan.pire_ms = plus_long(bilan.pire_ms, pire);
        if pire > BUDGET_IMAGE_MS {
            echecs.push(format!(
                "{nom}, pire cas {} : une image de {pire:.1} ms de travail du GPU fini, au-delà de {BUDGET_IMAGE_MS} ms (CONTRAT, « Les budgets ») ; images : {durees:?}",
                s.indice
            ));
        }
    }
    durees
}

/// Juge une scène rapportée ; rend ce qu'elle a montré, en une ligne.
fn juger_scene(
    nom: &str,
    s: &ScenePreparee,
    l: &Json,
    (sorte, fois): (Sorte, u32),
    bilan: &mut Bilan,
    echecs: &mut Vec<String>,
) -> String {
    // Les agencements, au bit près.
    let lus = agencements_lus(l);
    bilan.textes = bilan.textes.saturating_add(lus.len());
    if lus != s.agencements {
        let premier = s
            .agencements
            .iter()
            .find(|(n, h)| lus.get(n) != Some(h))
            .map(|(n, _)| *n)
            .or_else(|| lus.keys().find(|n| !s.agencements.contains_key(n)).copied());
        echecs.push(format!(
            "{nom}, pire cas {} : agencements différents du natif ({} agencés en natif, {} rapportés ; premier écart au nœud {premier:?})",
            s.indice,
            s.agencements.len(),
            lus.len()
        ));
    }
    for e in l
        .get("erreurs_gpu")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        echecs.push(format!(
            "{nom}, pire cas {} : erreur du GPU : {e}",
            s.indice
        ));
    }
    for e in l
        .get("ecarts_tampon")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        echecs.push(format!(
            "{nom}, pire cas {} : tampon ≠ agencement : {e}",
            s.indice
        ));
    }
    // Les pixels.
    let vide = Vec::new();
    let lus_px = l.get("sondes").and_then(Json::as_array).unwrap_or(&vide);
    let oublie = if sorte.fautif.is_empty() {
        None
    } else {
        s.oublie
    };
    let Lecture {
        relues,
        exactes,
        hors,
        ecarts,
        sensibles: ratees,
    } = juger_sondes(lus_px, &s.sondes, oublie);
    bilan.relues = bilan.relues.saturating_add(relues);
    bilan.exactes = bilan.exactes.saturating_add(exactes);
    bilan.hors = bilan.hors.saturating_add(hors);
    bilan.sensibles = bilan
        .sensibles
        .saturating_add(s.sondes.iter().filter(|x| x.sensible == Oui::Oui).count());
    bilan.sensibles_ratees = bilan.sensibles_ratees.saturating_add(ratees);
    if sorte.fautif.is_empty() {
        for e in ecarts {
            echecs.push(format!("{nom}, pire cas {} : pixel {e}", s.indice));
        }
    } else if !ecarts.is_empty() {
        echecs.push(format!(
            "{nom}, pire cas {} : sous le contrôle négatif, des pixels hors du nœud oublié diffèrent : {}",
            s.indice,
            ecarts.join(" ; ")
        ));
    }
    let durees = juger_durees(nom, s, l, fois, bilan, echecs);
    let nombre = |c: &str| l.get(c).and_then(Json::as_u64).unwrap_or(0);
    format!(
        "{nom}, scène {} : {} nœuds, {} couches, {} textes, {} glyphes absents, {} images absentes ; images {durees:?} ms",
        s.indice,
        nombre("noeuds"),
        nombre("couches"),
        nombre("textes"),
        nombre("absents"),
        nombre("images_absentes"),
    )
}

/// Juge la perte du GPU rapportée pour un jeu ; rend ce qu'elle a montré.
fn juger_perte(j: &JeuPrepare, lu: &Json, echecs: &mut Vec<String>) -> Option<String> {
    match lu.get("perte") {
        Some(p) if p.get("erreur").is_none() => {
            let derniere = j.scenes.last().map_or(&[][..], |s| s.sondes.as_slice());
            let vide = Vec::new();
            let lus = p.get("sondes").and_then(Json::as_array).unwrap_or(&vide);
            for e in juger_sondes(lus, derniere, None).ecarts {
                echecs.push(format!("{} : après la perte du GPU, pixel {e}", j.nom));
            }
            let ms = |c: &str| p.get(c).and_then(Json::as_f64).unwrap_or(f64::NAN);
            Some(format!(
                "{} : GPU perdu signalé en {:.1} ms, image revenue en {:.1} ms (dont {:.1} ms de travail du GPU pour l'image)",
                j.nom,
                ms("perdu_ms"),
                ms("retour_ms"),
                ms("image_ms"),
            ))
        }
        Some(p) => {
            echecs.push(format!(
                "{} : la perte du GPU : {}",
                j.nom,
                p.get("erreur").unwrap_or(&Json::Null)
            ));
            None
        }
        None => {
            echecs.push(format!("{} : la perte du GPU n'est pas rapportée", j.nom));
            None
        }
    }
}

/// Juge un jeu rapporté.
fn juger_jeu(
    j: &JeuPrepare,
    lu: &Json,
    (sorte, fois): (Sorte, u32),
    echecs: &mut Vec<String>,
    constats: &mut Vec<String>,
) -> Bilan {
    let mut bilan = Bilan::default();
    let vide = Vec::new();
    let scenes_lues = lu.get("scenes").and_then(Json::as_array).unwrap_or(&vide);
    if scenes_lues.len() != j.scenes.len() {
        echecs.push(format!(
            "{} : {} scènes rapportées, {} préparées",
            j.nom,
            scenes_lues.len(),
            j.scenes.len()
        ));
    }
    for (s, l) in j.scenes.iter().zip(scenes_lues) {
        constats.push(juger_scene(&j.nom, s, l, (sorte, fois), &mut bilan, echecs));
    }
    let mut illisibles: usize = 0;
    for e in lu
        .get("images_illisibles")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        // Une image que le navigateur ne décode pas se dessine comme absente : un constat,
        // pas un écart du rendu (les pires cas de Tier list portent des PNG faits d'un
        // en-tête et de zéros, que rien ne décode).
        illisibles = illisibles.saturating_add(1);
        if illisibles == 1 {
            constats.push(format!(
                "{} : une image que la télé ne décode pas, par exemple : {e}",
                j.nom
            ));
        }
    }
    if illisibles > 0 {
        constats.push(format!(
            "{} : {illisibles} image(s) que la télé ne décode pas, dessinée(s) comme absentes",
            j.nom
        ));
    }
    constats.push(format!(
        "{} : {} agencements comparés, {} pixels relus ({} hors de la toile ; {} exacts, à 0 près), {} sensibles à l'oubli, dont {} différents ; pire image de pire cas {:.1} ms",
        j.nom,
        bilan.textes,
        bilan.relues,
        bilan.hors,
        bilan.exactes,
        bilan.sensibles,
        bilan.sensibles_ratees,
        bilan.pire_ms
    ));
    if bilan.relues == 0 && j.scenes.iter().any(|s| !s.sondes.is_empty()) {
        echecs.push(format!("{} : aucun pixel relu sur la toile", j.nom));
    }
    if sorte.perte == Oui::Oui {
        constats.extend(juger_perte(j, lu, echecs));
    }
    bilan
}

/// Juge le rapport d'un passage, d'après ce que le banc a préparé et le nombre d'images
/// demandées par scène (`--fois`).
pub fn juger(
    rapport: &Json,
    jeux: &[JeuPrepare],
    sorte: Sorte,
    fois: u32,
) -> (Result<String, String>, Vec<String>) {
    let mut constats = Vec::new();
    if let Some(e) = rapport.get("erreur") {
        return (Err(format!("la page n'a pas joué : {e}")), constats);
    }
    let moteur = rapport.get("sorte").and_then(Json::as_str).unwrap_or("?");
    constats.push(format!(
        "moteur : {moteur} ({})",
        rapport.get("moteur").and_then(Json::as_str).unwrap_or("?")
    ));
    let mut echecs = Vec::new();
    if sorte.moteur == "webgl2" && moteur != "webgl2" {
        echecs.push(format!("WebGL2 forcé, mais le moteur est {moteur}"));
    }
    let vide = Vec::new();
    let jeux_lus = rapport
        .get("jeux")
        .and_then(Json::as_array)
        .unwrap_or(&vide);
    let mut sensibles_par_jeu = Vec::new();
    let mut pire_ms: f64 = 0.0;
    for j in jeux {
        let Some(lu) = jeux_lus
            .iter()
            .find(|x| x.get("nom").and_then(Json::as_str) == Some(j.nom.as_str()))
        else {
            echecs.push(format!("{} : absent du rapport", j.nom));
            continue;
        };
        let bilan = juger_jeu(j, lu, (sorte, fois), &mut echecs, &mut constats);
        pire_ms = plus_long(pire_ms, bilan.pire_ms);
        if !sorte.fautif.is_empty() && bilan.sensibles > 0 {
            sensibles_par_jeu.push((j.nom.clone(), bilan.sensibles_ratees));
        }
    }
    if !sorte.fautif.is_empty() {
        if sensibles_par_jeu.is_empty() {
            echecs.push(String::from(
                "contrôle négatif : aucun jeu n'a de pixel où l'oubli se voit",
            ));
        }
        for (nom, n) in &sensibles_par_jeu {
            if *n == 0 {
                echecs.push(format!(
                    "contrôle négatif : l'oubli d'un nœud ne se voit à aucun pixel de {nom}"
                ));
            }
        }
    }
    if !echecs.is_empty() {
        return (Err(echecs.join("\n")), constats);
    }
    let quoi = if sorte.fautif.is_empty() {
        format!("tout tient ; pire image de pire cas {pire_ms:.1} ms")
    } else {
        format!(
            "l'oubli se voit : {}",
            sensibles_par_jeu
                .iter()
                .map(|(n, k)| format!("{n} {k} pixel(s)"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    (Ok(quoi), constats)
}

/// Les catégories de la trace de Chrome où la composition dit ses images.
const CATEGORIES: [&str; 5] = [
    "disabled-by-default-devtools.timeline.frame",
    "devtools.timeline",
    "cc",
    "viz",
    "benchmark",
];

fn passer(
    options: &Options,
    choisi: Navigateur,
    sorte: Sorte,
    origine: &str,
    profils: &Path,
    recus: &mpsc::Receiver<Requete>,
    jeux: &[JeuPrepare],
) -> Passage {
    let mut p = Passage {
        navigateur: choisi,
        version: String::new(),
        sorte,
        rapport: None,
        console: Vec::new(),
        trace: None,
        verdict: Err(String::from("pas exécuté")),
        constats: Vec::new(),
    };
    let mut nav = match Lance::lancer(choisi, None, profils) {
        Ok(c) => c,
        Err(e) => {
            p.verdict = Err(format!("pas exécuté : {e}"));
            return p;
        }
    };
    nav.version().clone_into(&mut p.version);
    if let Some(c) = nav.chrome()
        && let Err(e) = c.tracer(&CATEGORIES)
    {
        p.console.push(format!("trace : {e}"));
    }
    let perte = if sorte.perte == Oui::Oui { "1" } else { "0" };
    let url = format!(
        "{origine}/banc/dessin.html?moteur={}&fautif={}&perte={perte}",
        sorte.moteur, sorte.fautif
    );
    let corps = nav.ouvrir(&url).and_then(|()| {
        let echeance = maintenant()
            .checked_add(options.delai)
            .ok_or_else(|| String::from("délai"))?;
        loop {
            if let Ok(q) = recus.recv_timeout(Duration::from_millis(100)) {
                p.console.extend(nav.ecouter(Duration::from_millis(200))?);
                return Ok(q.corps);
            }
            p.console.extend(nav.ecouter(Duration::from_millis(100))?);
            if reste(echeance).is_zero() {
                return Err(format!("aucun rapport en {:?}", options.delai));
            }
        }
    });
    if let Some(c) = nav.chrome() {
        p.trace = Some(match c.fin_de_trace(Duration::from_secs(60)) {
            Ok(ev) => compter_les_images(&ev),
            Err(e) => json!({ "erreur": e }),
        });
    }
    nav.fermer();
    match corps.and_then(|c| {
        serde_json::from_slice::<Json>(&c).map_err(|e| format!("rapport illisible : {e}"))
    }) {
        Ok(r) => {
            let (v, constats) = juger(&r, jeux, sorte, options.fois);
            p.verdict = v;
            p.constats = constats;
            p.rapport = Some(r);
        }
        Err(e) => p.verdict = Err(e),
    }
    p
}

/// Pourquoi le scénario s'arrête avant ses passages.
pub use crate::machine::Arret;

/// Prépare les jeux, construit la page, et joue chaque passage dans chaque navigateur.
pub fn jouer(o: &Options) -> Result<(Vec<JeuPrepare>, Vec<Passage>), Arret> {
    let soiree = commun();
    let web_ = soiree.join("web");
    let mut ecarts = web::ecarts(&soiree, &web_);
    let lire = |f: &Path| std::fs::read_to_string(f).unwrap_or_default();
    match web::dependances_tierces(&soiree, "mise-en-page") {
        Ok(racines) => ecarts.extend(web::ecarts_de_versions(
            &lire(&soiree.join("Cargo.lock")),
            &lire(&web_.join("Cargo.lock")),
            &racines,
        )),
        Err(e) => ecarts.push(e),
    }
    if !ecarts.is_empty() {
        return Err(Arret::Echec(format!(
            "la règle « mêmes lints », ou celle des mêmes versions de la mesure, refuse web/ :\n  {}",
            ecarts.join("\n  ")
        )));
    }
    let banc = soiree.join("target/banc");
    let paquet = banc.join("paquet");
    if o.construire {
        machine::construire_de(&web_, &paquet, "ecran", &["dessin-banc"])
            .map_err(Arret::PasExecute)?;
    }
    if !paquet.join("dessin_banc.js").exists() {
        return Err(Arret::PasExecute(format!(
            "{} manque : construire d'abord (sans --sans-construire)",
            paquet.join("dessin_banc.js").display()
        )));
    }
    let donnees = banc.join("dessin");
    let _: std::io::Result<()> = std::fs::remove_dir_all(&donnees);
    std::fs::create_dir_all(&donnees).map_err(|e| Arret::PasExecute(e.to_string()))?;
    let mut jeux = Vec::new();
    for m in &o.modules {
        jeux.push(preparer(m, &donnees).map_err(Arret::Echec)?);
    }
    if o.charge == Oui::Oui {
        jeux.push(preparer_la_charge(&donnees).map_err(Arret::Echec)?);
    }
    std::fs::write(
        donnees.join("manifeste.json"),
        manifeste(&jeux, o.fois).to_string(),
    )
    .map_err(|e| Arret::PasExecute(e.to_string()))?;
    let (envoi, recus) = mpsc::channel();
    let serveur = Serveur::demarrer(
        vec![
            (String::from("/banc/"), soiree.join("crates/banc/pages")),
            (String::from("/paquet/"), paquet),
            (String::from("/dessin/"), donnees),
        ],
        envoi,
    )
    .map_err(|e| Arret::PasExecute(format!("serveur : {e}")))?;
    let origine = serveur.origine();
    let profils = banc.join("profils");
    let mut passages = Vec::new();
    for n in &o.navigateurs {
        for s in PASSAGES {
            let p = passer(o, *n, s, &origine, &profils, &recus, &jeux);
            let pas_execute = matches!(&p.verdict, Err(e) if e.starts_with("pas exécuté"));
            passages.push(p);
            // Un navigateur que la machine ne permet pas ne se relance pas trois fois.
            if pas_execute {
                break;
            }
        }
    }
    serveur.arreter();
    Ok((jeux, passages))
}
