//! La scène mise à plat : ce que le GPU dessine, dans l'ordre du peintre, en pixels de la
//! toile (PHASE-3, § 2.9). Sans GPU : testée en natif telle quelle.
//!
//! - Les nœuds se lisent dans l'ordre de la liste, chacun après son parent ; un groupe
//!   décale ses enfants ; l'opacité se multiplie de parent en enfant.
//! - Une animation (`anime`) se lit à un instant de la partie (REM-12), ou au repos :
//!   après sa durée, le nœud est à son visuel d'arrivée, sauf `Courbe::Rebond`, qui
//!   revient à son départ (le saut de Sidequest : PHASE-3, § 2.9, *écrit au lot 9*).
//!   L'échelle d'un visuel s'applique autour du centre de la boîte d'une feuille, autour
//!   de l'origine d'un groupe.
//! - `pixel_entier` (SQ-52) pose l'origine du nœud sur un pixel entier ; les bords d'un
//!   rectangle sont toujours arrondis au pixel (aucun lissage des bords).
//! - Un `Bouton` est un fond et un nom, dans le style du commun
//!   (`mise_en_page::bouton`) ; un `CompteARebours`, un texte formé depuis le pas ; un
//!   `Qr`, des modules (SAL-191). `Composant`, `Avertissements`, `Surface`, `Definitif`
//!   et `Groupe` ne dessinent rien eux-mêmes ici (les composants sont des éléments du
//!   DOM, posés par l'écran au lot 11).
//! - L'option de test [`Fautif::OublierLeNoeud`] oublie un nœud de la scène : le
//!   contrôle négatif du scénario `dessin` du banc, qui choisit dans chaque scène un
//!   `Rect` dont l'oubli se voit à un point qu'il relit.

use jeu_abi::{
    Animation, Cle, Courbe, Echantillonnage, Fixe, Instant, NoeudFil, Oui, Pas, Rect, Rgba,
    SceneFil, SorteNoeud, StyleTexte,
};
use mise_en_page::{bouton, compte};

use crate::cadre::{Affine, Cadre, plus_grand, plus_petit};
use crate::qr;

/// Une option de test du rendu : un défaut voulu, pour qu'un contrôle le voie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fautif {
    /// Oublie le nœud de cet indice dans la scène, sans rien changer de ce qu'il
    /// transmet à ses enfants (le contrôle négatif de `dessin`).
    OublierLeNoeud(u32),
}

/// Ce que le rendu sait du moment et du lieu, hors de la scène.
#[derive(Debug, Clone, PartialEq)]
pub struct Contexte {
    /// Le pas estimé de la partie, pour les comptes à rebours.
    pub pas: Pas,
    /// L'instant où lire les animations ; `None` : au repos, toutes finies.
    pub maintenant: Option<Instant>,
    /// L'adresse que le QR code porte, s'il y en a une.
    pub adresse_qr: Option<String>,
    /// Le fond de la toile quand la scène n'en donne pas.
    pub fond_par_defaut: Rgba,
    /// Un défaut voulu, pour un contrôle négatif.
    pub fautif: Option<Fautif>,
    /// Un objet que le composant `Deplacables` fait glisser (lot 11), s'il y en a un.
    pub glisser: Option<GlisserVu>,
}

/// Ce que le rendu dessine d'un glisser (TL-47, lot 11) : l'objet reste à sa place, à
/// 20 % ; une copie à 80 % suit le pointeur ; une marque dit où il tombera. Le jeu ne
/// déplace pas l'objet pendant le glisser (Tier list ignore `Survol`) : la marque le dit à
/// sa place.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlisserVu {
    /// L'objet, par sa clé : les éléments du nœud qui la porte.
    pub objet: Cle,
    /// Le décalage de la copie, en pixels de la toile.
    pub decalage: (f32, f32),
    /// La marque de la place visée, en pixels de la toile, s'il y en a une.
    pub marque: Option<Boite>,
}

/// L'opacité de l'objet tenu, à sa place (CAHIER de Tier list, 4.7).
pub const OPACITE_DE_L_OBJET_TENU: f32 = 0.2;
/// L'opacité de la copie qui suit le pointeur (CAHIER de Tier list, 4.7).
pub const OPACITE_DE_LA_COPIE: f32 = 0.8;
/// La couleur de la marque de la place visée (choisie : du blanc, opaque).
pub const MARQUE: Rgba = Rgba(255, 255, 255, 255);

impl Contexte {
    /// Au repos, au pas 0, sans QR code, sur fond noir, sans défaut.
    pub const fn au_repos() -> Contexte {
        Contexte {
            pas: Pas::ZERO,
            maintenant: None,
            adresse_qr: None,
            fond_par_defaut: Rgba::NOIR,
            fautif: None,
            glisser: None,
        }
    }
}

/// Un rectangle en pixels : gauche, haut, droite, bas.
pub type Boite = [f32; 4];

/// Ce qu'un élément dessine.
#[derive(Debug, Clone, PartialEq)]
pub enum Sorte {
    /// Un quad uni, ou une image.
    Quad {
        /// La couleur, prémultipliée, de 0 à 1.
        couleur: [f32; 4],
        /// L'image et son échantillonnage ; `None` : uni.
        image: Option<(Cle, Echantillonnage)>,
    },
    /// Un texte.
    Texte {
        /// Le texte.
        texte: String,
        /// Sa boîte, dans le repère de la scène (la mise en page ne dépend que de sa
        /// taille).
        boite: Rect,
        /// Son style.
        style: StyleTexte,
        /// Des pixels par unité.
        echelle: f32,
        /// L'opacité, de 0 à 1, à multiplier à celle du style.
        opacite: f32,
    },
}

/// Un élément de la scène mise à plat.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    /// L'indice du nœud d'où il vient.
    pub noeud: u32,
    /// Sa boîte, en pixels de la toile.
    pub boite: Boite,
    /// Ce qu'il dessine.
    pub sorte: Sorte,
    /// Sa couche : dans une couche, les quads se dessinent avant les textes ; une couche
    /// passe après la précédente.
    pub couche: u32,
}

/// La scène mise à plat.
#[derive(Debug, Clone, PartialEq)]
pub struct Liste {
    /// Le fond de la toile.
    pub fond: Rgba,
    /// Les éléments, dans l'ordre du peintre.
    pub elements: Vec<Element>,
    /// Les couches.
    pub couches: u32,
    /// Les nœuds de la scène.
    pub noeuds: u32,
}

/// La couleur d'une `Rgba`, prémultipliée par son opacité et par `opacite`.
pub fn premultipliee(c: Rgba, opacite: f32) -> [f32; 4] {
    let a = f32::from(c.3) / 255.0 * opacite;
    [
        f32::from(c.0) / 255.0 * a,
        f32::from(c.1) / 255.0 * a,
        f32::from(c.2) / 255.0 * a,
        a,
    ]
}

/// Une longueur du repère, en flottant.
pub fn unites(f: Fixe) -> f32 {
    (f64::from(f.brut()) / 256.0) as f32
}

/// L'avancement d'une animation, de 0 à 1, sur sa courbe ; `None` : au repos.
fn avancement(depuis: Instant, a: &Animation, maintenant: Option<Instant>) -> f32 {
    let t = match maintenant {
        None => 1.0,
        Some(m) => {
            let ecoule = |i: Instant| i.pas.numero() as f64 + f64::from(i.fraction) / 65_536.0;
            let duree = a.duree.en_pas() as f64;
            if duree <= 0.0 {
                1.0
            } else {
                let x = (ecoule(m) - ecoule(depuis)) / duree;
                if x <= 0.0 {
                    0.0
                } else if x >= 1.0 {
                    1.0
                } else {
                    x as f32
                }
            }
        }
    };
    match a.courbe {
        Courbe::Lineaire => t,
        Courbe::Entree => t * t,
        Courbe::Sortie => 1.0 - (1.0 - t) * (1.0 - t),
        Courbe::EntreeSortie => t * t * (3.0 - 2.0 * t),
        // Un saut : du départ à l'arrivée à mi-chemin, puis retour au départ.
        Courbe::Rebond => 4.0 * t * (1.0 - t),
    }
}

/// Le visuel d'un nœud animé : décalage (unités), échelle, opacité (de 0 à 1).
fn visuel(n: &NoeudFil, maintenant: Option<Instant>) -> (f32, f32, f32, f32) {
    let Some((depuis, a)) = &n.anime else {
        return (0.0, 0.0, 1.0, 1.0);
    };
    let p = avancement(*depuis, a, maintenant);
    let entre = |x: f32, y: f32| x + (y - x) * p;
    (
        entre(unites(a.de.decalage.0), unites(a.a.decalage.0)),
        entre(unites(a.de.decalage.1), unites(a.a.decalage.1)),
        entre(
            f32::from(a.de.echelle_pour_mille) / 1000.0,
            f32::from(a.a.echelle_pour_mille) / 1000.0,
        ),
        entre(f32::from(a.de.opacite), f32::from(a.a.opacite)) / 255.0,
    )
}

/// Ce qu'un nœud transmet à ses enfants.
#[derive(Debug, Clone, Copy)]
struct Herite {
    affine: Affine,
    opacite: f32,
}

/// Une boîte du repère, en pixels, par `affine`, son origine arrondie au pixel si
/// `entier`.
fn en_pixels(affine: &Affine, r: Rect, entier: Oui) -> Boite {
    let (x0, y0) = affine.point(unites(r.x), unites(r.y));
    let (l, h) = (unites(r.l) * affine.echelle, unites(r.h) * affine.echelle);
    if entier == Oui::Oui {
        let (x0, y0) = (x0.round(), y0.round());
        [x0, y0, x0 + l, y0 + h]
    } else {
        [x0, y0, x0 + l, y0 + h]
    }
}

/// Une boîte aux bords arrondis au pixel.
fn arrondie(b: Boite) -> Boite {
    [b[0].round(), b[1].round(), b[2].round(), b[3].round()]
}

/// La transformation d'une feuille animée : échelle autour du centre de sa boîte.
fn feuille(h: &Herite, n: &NoeudFil, boite: Rect, maintenant: Option<Instant>) -> (Affine, f32) {
    let (dx, dy, e, o) = visuel(n, maintenant);
    let (cx, cy) = (
        unites(boite.x) + unites(boite.l) / 2.0,
        unites(boite.y) + unites(boite.h) / 2.0,
    );
    // p' = e (p - c) + c + d
    let local = Affine {
        echelle: e,
        x: cx - e * cx + dx,
        y: cy - e * cy + dy,
    };
    (
        h.affine.apres(&local),
        h.opacite * f32::from(n.opacite) / 255.0 * o,
    )
}

/// Met une scène à plat dans un cadre.
pub fn mettre_a_plat(scene: &SceneFil, cadre: &Cadre, contexte: &Contexte) -> Liste {
    let racine = Herite {
        affine: cadre.transformation(),
        opacite: 1.0,
    };
    let oublie = match contexte.fautif {
        Some(Fautif::OublierLeNoeud(n)) => usize::try_from(n).ok(),
        None => None,
    };
    let mut herites: Vec<Herite> = Vec::with_capacity(scene.noeuds.len());
    let mut elements = Vec::new();
    let mut copies = Vec::new();
    for (i, n) in scene.noeuds.iter().enumerate() {
        let h = n
            .parent
            .and_then(|p| herites.get(usize::try_from(p).ok()?))
            .copied()
            .unwrap_or(racine);
        let indice = u32::try_from(i).unwrap_or(u32::MAX);
        let avant = elements.len();
        let transmis = if oublie == Some(i) {
            h
        } else {
            poser(n, indice, h, contexte, &mut elements)
        };
        if let Some(g) = contexte.glisser.as_ref().filter(|g| g.objet == n.cle) {
            for e in elements.get_mut(avant..).unwrap_or(&mut []) {
                let mut copie = e.clone();
                attenuer(e, OPACITE_DE_L_OBJET_TENU);
                attenuer(&mut copie, OPACITE_DE_LA_COPIE);
                let (dx, dy) = g.decalage;
                copie.boite = [
                    copie.boite[0] + dx,
                    copie.boite[1] + dy,
                    copie.boite[2] + dx,
                    copie.boite[3] + dy,
                ];
                copies.push(copie);
            }
        }
        herites.push(transmis);
    }
    // La marque de la place visée, puis la copie, par-dessus tout.
    if let Some(m) = contexte.glisser.as_ref().and_then(|g| g.marque) {
        elements.push(Element {
            noeud: u32::MAX,
            boite: arrondie(m),
            sorte: Sorte::Quad {
                couleur: premultipliee(MARQUE, 1.0),
                image: None,
            },
            couche: 0,
        });
    }
    elements.extend(copies);
    let couches = coucher(&mut elements);
    Liste {
        fond: scene.fond.unwrap_or(contexte.fond_par_defaut),
        elements,
        couches,
        noeuds: u32::try_from(scene.noeuds.len()).unwrap_or(u32::MAX),
    }
}

/// Multiplie l'opacité d'un élément.
fn attenuer(e: &mut Element, facteur: f32) {
    match &mut e.sorte {
        Sorte::Quad { couleur, .. } => {
            for c in couleur.iter_mut() {
                *c *= facteur;
            }
        }
        Sorte::Texte { opacite, .. } => *opacite *= facteur,
    }
}

/// La boîte de chaque nœud de la scène, en pixels de la toile, dans l'ordre de la liste
/// (lot 11) : celle d'une feuille qui en a une (rectangle, image, texte, bouton, compte à
/// rebours, QR, composant posé dans sa boîte, avertissements, surface), animation lue au
/// même instant que le dessin ; `None` pour un groupe, un sous-arbre définitif, un
/// composant sans boîte. L'écran s'en sert pour poser ses éléments du DOM, trouver le
/// bouton sous le pointeur, et les zones et les objets d'un glisser.
pub fn boites(scene: &SceneFil, cadre: &Cadre, contexte: &Contexte) -> Vec<Option<Boite>> {
    let racine = Herite {
        affine: cadre.transformation(),
        opacite: 1.0,
    };
    let mut herites: Vec<Herite> = Vec::with_capacity(scene.noeuds.len());
    let mut boites = Vec::with_capacity(scene.noeuds.len());
    for n in &scene.noeuds {
        let h = n
            .parent
            .and_then(|p| herites.get(usize::try_from(p).ok()?))
            .copied()
            .unwrap_or(racine);
        let r = match &n.sorte {
            SorteNoeud::Rect { r, .. }
            | SorteNoeud::Image { r, .. }
            | SorteNoeud::Surface { r } => Some(*r),
            SorteNoeud::Texte { boite, .. }
            | SorteNoeud::Bouton { boite, .. }
            | SorteNoeud::CompteARebours { boite, .. }
            | SorteNoeud::Qr { boite }
            | SorteNoeud::Avertissements { boite } => Some(*boite),
            SorteNoeud::Composant { boite, .. } => *boite,
            SorteNoeud::Groupe { .. } | SorteNoeud::Definitif => None,
        };
        boites.push(r.map(|r| {
            let (transfo, _) = feuille(&h, n, r, contexte.maintenant);
            en_pixels(&transfo, r, n.pixel_entier)
        }));
        herites.push(transmis(n, h, contexte.maintenant));
    }
    boites
}

/// La transformation qu'un nœud transmet à ses enfants, depuis la racine du cadre : pour
/// porter dans le repère de la toile une boîte d'un composant (le panneau d'une modale).
pub fn transformation_des_enfants(
    scene: &SceneFil,
    noeud: usize,
    cadre: &Cadre,
    contexte: &Contexte,
) -> Affine {
    let mut chaine = Vec::new();
    let mut i = Some(noeud);
    while let Some(k) = i {
        chaine.push(k);
        i = scene
            .noeuds
            .get(k)
            .and_then(|n| n.parent)
            .and_then(|p| usize::try_from(p).ok());
    }
    let mut h = Herite {
        affine: cadre.transformation(),
        opacite: 1.0,
    };
    // Les ancêtres seuls : le nœud lui-même est une feuille, qui transmet ce qu'il reçoit.
    for k in chaine.iter().skip(1).rev() {
        if let Some(n) = scene.noeuds.get(*k) {
            h = transmis(n, h, contexte.maintenant);
        }
    }
    h.affine
}

/// Une boîte du repère, en pixels, par une transformation (le panneau d'une modale).
pub fn boite_en_pixels(affine: &Affine, r: Rect) -> Boite {
    en_pixels(affine, r, Oui::Non)
}

/// Ce qu'un groupe ou un sous-arbre définitif transmet à ses enfants : son décalage, son
/// animation, son opacité.
fn transmis(noeud_fil: &NoeudFil, herite: Herite, maintenant: Option<Instant>) -> Herite {
    match &noeud_fil.sorte {
        SorteNoeud::Groupe { decalage } => {
            let (dx, dy, ech, op) = visuel(noeud_fil, maintenant);
            let mut transfo = herite.affine.decale(unites(decalage.0), unites(decalage.1));
            transfo = transfo.apres(&Affine {
                echelle: ech,
                x: dx,
                y: dy,
            });
            if noeud_fil.pixel_entier == Oui::Oui {
                transfo.x = transfo.x.round();
                transfo.y = transfo.y.round();
            }
            Herite {
                affine: transfo,
                opacite: herite.opacite * f32::from(noeud_fil.opacite) / 255.0 * op,
            }
        }
        SorteNoeud::Definitif => Herite {
            affine: herite.affine,
            opacite: herite.opacite * f32::from(noeud_fil.opacite) / 255.0,
        },
        SorteNoeud::Rect { .. }
        | SorteNoeud::Image { .. }
        | SorteNoeud::Texte { .. }
        | SorteNoeud::Bouton { .. }
        | SorteNoeud::CompteARebours { .. }
        | SorteNoeud::Qr { .. }
        | SorteNoeud::Composant { .. }
        | SorteNoeud::Avertissements { .. }
        | SorteNoeud::Surface { .. } => herite,
    }
}

/// Pose les éléments d'un nœud ; rend ce qu'il transmet à ses enfants (une feuille
/// transmet ce qu'elle a reçu : elle n'a pas d'enfant, la structure le garantit).
fn poser(
    noeud_fil: &NoeudFil,
    indice: u32,
    herite: Herite,
    contexte: &Contexte,
    elements: &mut Vec<Element>,
) -> Herite {
    let maintenant = contexte.maintenant;
    let quad = |boite: Boite, couleur: [f32; 4], image| Element {
        noeud: indice,
        boite,
        sorte: Sorte::Quad { couleur, image },
        couche: 0,
    };
    match &noeud_fil.sorte {
        SorteNoeud::Groupe { .. } | SorteNoeud::Definitif => {
            return transmis(noeud_fil, herite, maintenant);
        }
        SorteNoeud::Rect { r, couleur } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *r, maintenant);
            let bo = arrondie(en_pixels(&transfo, *r, noeud_fil.pixel_entier));
            elements.push(quad(bo, premultipliee(*couleur, op), None));
        }
        SorteNoeud::Image { image, r, ech } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *r, maintenant);
            let bo = en_pixels(&transfo, *r, noeud_fil.pixel_entier);
            elements.push(quad(bo, [op, op, op, op], Some((*image, *ech))));
        }
        SorteNoeud::Texte {
            texte,
            boite,
            style,
        } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *boite, maintenant);
            elements.push(texte_element(
                indice,
                &transfo,
                noeud_fil.pixel_entier,
                texte,
                *boite,
                *style,
                op,
            ));
        }
        SorteNoeud::Bouton { boite, nom, .. } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *boite, maintenant);
            let bo = arrondie(en_pixels(&transfo, *boite, noeud_fil.pixel_entier));
            elements.push(quad(bo, premultipliee(bouton::FOND_DU_BOUTON, op), None));
            let nom_boite = bouton::boite_du_nom(*boite);
            let style = bouton::style_du_nom(*boite);
            elements.push(texte_element(
                indice,
                &transfo,
                noeud_fil.pixel_entier,
                nom,
                nom_boite,
                style,
                op,
            ));
        }
        SorteNoeud::CompteARebours {
            compte: compte_fil,
            boite,
            style,
        } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *boite, maintenant);
            let chaine = compte::texte(compte_fil, contexte.pas);
            elements.push(texte_element(
                indice,
                &transfo,
                noeud_fil.pixel_entier,
                &chaine,
                *boite,
                *style,
                op,
            ));
        }
        SorteNoeud::Qr { boite } => {
            let (transfo, op) = feuille(&herite, noeud_fil, *boite, maintenant);
            if let Some(adresse) = &contexte.adresse_qr {
                let bo = en_pixels(&transfo, *boite, noeud_fil.pixel_entier);
                qr::elements(adresse, bo, op, indice, elements);
            }
        }
        SorteNoeud::Composant { .. }
        | SorteNoeud::Avertissements { .. }
        | SorteNoeud::Surface { .. } => {}
    }
    herite
}

/// Un texte posé.
fn texte_element(
    noeud: u32,
    a: &Affine,
    entier: Oui,
    texte: &str,
    boite: Rect,
    style: StyleTexte,
    opacite: f32,
) -> Element {
    Element {
        noeud,
        boite: en_pixels(a, boite, entier),
        sorte: Sorte::Texte {
            texte: String::from(texte),
            boite,
            style,
            echelle: a.echelle,
            opacite,
        },
        couche: 0,
    }
}

/// Deux boîtes se recouvrent-elles (d'une aire non nulle) ?
fn recouvre(a: &Boite, b: &Boite) -> bool {
    plus_grand(a[0], b[0]) < plus_petit(a[2], b[2])
        && plus_grand(a[1], b[1]) < plus_petit(a[3], b[3])
}

/// Range les éléments en couches, en gardant l'ordre du peintre : dans une couche, les
/// quads passent avant les textes. Un quad monte au-dessus de tout texte antérieur qu'il
/// recouvre ; un texte reste dans la couche du plus haut élément antérieur qu'il
/// recouvre. Rend le nombre de couches.
fn coucher(elements: &mut [Element]) -> u32 {
    let mut couches = 0;
    for i in 0..elements.len() {
        let (avant, reste) = elements.split_at_mut(i);
        let Some(e) = reste.first_mut() else { break };
        let quad = matches!(e.sorte, Sorte::Quad { .. });
        let mut c = 0;
        for p in avant.iter().filter(|p| recouvre(&p.boite, &e.boite)) {
            let texte_avant = matches!(p.sorte, Sorte::Texte { .. });
            let au_moins = if quad && texte_avant {
                p.couche + 1
            } else {
                p.couche
            };
            c = c.max(au_moins);
        }
        e.couche = c;
        couches = couches.max(c + 1);
    }
    couches
}
