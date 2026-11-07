//! Le composant `Deplacables` de la télé (PHASE-3, § 2.9 ; TL-36 à TL-47 ; lot 11), sans
//! navigateur : du Rust du commun, sur les rectangles dessinés, testé en natif.
//!
//! - Un appui du bouton principal sur un objet commence un geste ; le glisser devient
//!   actif au-delà du seuil, en distance euclidienne depuis l'appui (TL-47). En dessous,
//!   relâcher ne fait rien.
//! - **La place visée se compte dans l'ordre de l'état**, pas dans l'ordre des éléments
//!   affichés (TL-39) : dans la zone sous le pointeur, on parcourt ses objets dans
//!   l'ordre, sans l'objet tenu ; le premier dont le bord haut est sous le pointeur, ou
//!   qui est à sa hauteur et dont le milieu est à sa droite, donne l'indice ; sinon, la
//!   fin de la zone.
//! - Hors de toute zone, la dernière place valide est gardée ; au début, c'est la place
//!   de l'objet (TL-43). Au-dessus de la poubelle, relâcher la vise.
//! - Le jeu reçoit `Survol` quand la place visée change, `Depose` au relâché, `Annule`
//!   à Échap ou à `pointercancel` (INTERFACE, § 6.3).
//! - **Le défilement près du bord** (TL-40 à TL-42) : une vitesse proportionnelle à la
//!   proximité du bord, jusqu'à la vitesse la plus grande au bord même ; la distance
//!   dépend du temps écoulé, jamais du nombre d'images ; un réveil compte au plus le
//!   plafond. Après chaque défilement, la place visée se recalcule (la page rappelle
//!   [`Glisser::deplacer`]).
//!
//! Les longueurs sont des pixels de la toile, la page à son haut (sans défilement) : les
//! boîtes de `rendu::boites` sous un cadre au défilement nul.

#![expect(
    clippy::cast_precision_loss,
    reason = "un plafond en millisecondes ramené en secondes flottantes, pour des pixels : rien ne revient à une partie que par le Deplace, que le cœur vérifie"
)]

use jeu_abi::{Cle, ComposantVuFil, Deplace, Duree, Fixe, SceneFil, SorteNoeud};
use rendu::{Boite, GlisserVu};

/// La largeur de la marque de la place visée, en pixels (choisie).
pub const LARGEUR_DE_LA_MARQUE: f32 = 3.0;

/// Une zone d'objets, et ses objets dans l'ordre de l'état, chacun avec sa boîte.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    /// La zone.
    pub cle: Cle,
    /// Sa boîte.
    pub boite: Boite,
    /// Ses objets, dans l'ordre de l'état.
    pub objets: Vec<(Cle, Boite)>,
}

/// Le composant tel que la dernière scène le porte, ses zones et ses objets posés.
#[derive(Debug, Clone, PartialEq)]
pub struct Plateau {
    /// Le nœud du composant, par sa clé.
    pub cle: Cle,
    /// Les zones.
    pub zones: Vec<Zone>,
    /// La poubelle, s'il y en a une, et sa boîte.
    pub poubelle: Option<(Cle, Boite)>,
    /// Le seuil d'un glisser, en pixels.
    pub seuil: f32,
    /// La largeur du bord qui fait défiler, en pixels.
    pub bord: f32,
    /// La vitesse de défilement la plus grande, en pixels par seconde.
    pub vitesse_max: f32,
    /// Ce qu'un réveil compte au plus, en secondes (TL-42).
    pub plafond_s: f32,
}

fn unites(f: Fixe) -> f32 {
    rendu::liste::unites(f)
}

fn contient(b: &Boite, (x, y): (f32, f32)) -> bool {
    x >= b[0] && x < b[2] && y >= b[1] && y < b[3]
}

impl Plateau {
    /// Le composant `Deplacables` de la scène (le dernier, s'il y en a plusieurs), ses
    /// zones et ses objets posés aux boîtes des nœuds qui portent leurs clés ; `echelle`
    /// : des pixels par unité du repère. `None` sans composant. Une zone, un objet ou la
    /// poubelle dont aucun nœud ne porte la clé ne se prend pas : rien ne le montre.
    pub fn de_la_scene(
        scene: &SceneFil,
        boites: &[Option<Boite>],
        echelle: f32,
    ) -> Option<Plateau> {
        let (cle, vu) = scene.noeuds.iter().rev().find_map(|n| match &n.sorte {
            SorteNoeud::Composant {
                composant: c @ ComposantVuFil::Deplacables { .. },
                ..
            } => Some((n.cle, c)),
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
        })?;
        let ComposantVuFil::Deplacables {
            zones,
            poubelle,
            seuil,
            bord,
            vitesse_max,
            plafond_reveil,
        } = vu
        else {
            return None;
        };
        let boite_de = |c: Cle| {
            scene
                .noeuds
                .iter()
                .zip(boites)
                .find_map(|(n, b)| if n.cle == c { *b } else { None })
        };
        let zones = zones
            .iter()
            .filter_map(|z| {
                Some(Zone {
                    cle: z.cle,
                    boite: boite_de(z.cle)?,
                    objets: z
                        .objets
                        .iter()
                        .filter_map(|o| Some((*o, boite_de(*o)?)))
                        .collect(),
                })
            })
            .collect();
        Some(Plateau {
            cle,
            zones,
            poubelle: poubelle.and_then(|p| Some((p, boite_de(p)?))),
            seuil: unites(*seuil) * echelle,
            bord: unites(*bord) * echelle,
            vitesse_max: unites(*vitesse_max) * echelle,
            plafond_s: secondes(*plafond_reveil),
        })
    }

    /// L'objet sous le point, s'il y en a un.
    pub fn objet_sous(&self, p: (f32, f32)) -> Option<Cle> {
        self.zones
            .iter()
            .flat_map(|z| z.objets.iter())
            .rev()
            .find(|(_, b)| contient(b, p))
            .map(|(c, _)| *c)
    }

    /// La zone et la place d'un objet, comptée sans lui.
    fn place_de(&self, objet: Cle) -> Option<(Cle, u32)> {
        self.zones.iter().find_map(|z| {
            let k = z.objets.iter().position(|(c, _)| *c == objet)?;
            Some((z.cle, u32::try_from(k).unwrap_or(u32::MAX)))
        })
    }

    /// L'objet existe-t-il encore dans le composant ?
    pub fn a_l_objet(&self, objet: Cle) -> bool {
        self.place_de(objet).is_some()
    }

    fn boite_de(&self, objet: Cle) -> Option<Boite> {
        self.zones
            .iter()
            .flat_map(|z| z.objets.iter())
            .find(|(c, _)| *c == objet)
            .map(|(_, b)| *b)
    }
}

fn secondes(d: Duree) -> f32 {
    d.en_ms() as f32 / 1000.0
}

/// La place visée dans une zone, comptée dans l'ordre de l'état, sans l'objet tenu
/// (TL-39) : le premier objet dont le bord haut est sous le pointeur, ou qui est à sa
/// hauteur et dont le milieu est à sa droite ; sinon, la fin.
pub fn place_visee(objets: &[(Cle, Boite)], tenu: Cle, (x, y): (f32, f32)) -> u32 {
    let mut k: u32 = 0;
    for (c, b) in objets {
        if *c == tenu {
            continue;
        }
        let au_dessus = y < b[1];
        let a_sa_hauteur = y >= b[1] && y < b[3];
        let a_gauche_du_milieu = x < f32::midpoint(b[0], b[2]);
        if au_dessus || (a_sa_hauteur && a_gauche_du_milieu) {
            return k;
        }
        k = k.saturating_add(1);
    }
    k
}

/// Le défilement d'un réveil près du bord (TL-40 à TL-42), en pixels : négatif vers le
/// haut. `y` : le pointeur dans la fenêtre, `hauteur` : la fenêtre ; `dt_s` : le temps
/// écoulé depuis le réveil précédent, compté au plus `plafond_s`. La vitesse croît du
/// bord intérieur de la bande (`bord` du bord de la fenêtre) jusqu'à `vitesse_max` au
/// bord même ; hors de la bande, rien.
pub fn defilement(
    y: f32,
    hauteur: f32,
    bord: f32,
    vitesse_max: f32,
    dt_s: f32,
    plafond_s: f32,
) -> f32 {
    if bord <= 0.0 || dt_s <= 0.0 {
        return 0.0;
    }
    let dt = if dt_s > plafond_s { plafond_s } else { dt_s };
    let proximite = if y < bord {
        -(bord - y) / bord
    } else if y > hauteur - bord {
        (y - (hauteur - bord)) / bord
    } else {
        0.0
    };
    let p = proximite.clamp(-1.0, 1.0);
    p * vitesse_max * dt
}

/// Un geste en cours sur un objet.
#[derive(Debug, Clone, PartialEq)]
pub struct Glisser {
    /// L'objet tenu.
    pub objet: Cle,
    /// Le pointeur qui l'a pris : lui seul compte.
    pub pointeur: i32,
    depart: (f32, f32),
    /// Le point saisi, depuis le coin de l'objet.
    saisie: (f32, f32),
    /// Le glisser a-t-il passé le seuil ?
    pub actif: bool,
    /// La dernière place valide (TL-43).
    place: (Cle, u32),
    /// La dernière place dite au jeu.
    dit: Option<(Cle, u32)>,
    /// Le pointeur est-il au-dessus de la poubelle ?
    pub sur_la_poubelle: bool,
    /// La position du pointeur.
    pub position: (f32, f32),
}

impl Glisser {
    /// Un appui du bouton principal en `p` : un geste commence s'il touche un objet.
    pub fn appui(plateau: &Plateau, p: (f32, f32), pointeur: i32) -> Option<Glisser> {
        let objet = plateau.objet_sous(p)?;
        let place = plateau.place_de(objet)?;
        let b = plateau.boite_de(objet)?;
        Some(Glisser {
            objet,
            pointeur,
            depart: p,
            saisie: (p.0 - b[0], p.1 - b[1]),
            actif: false,
            place,
            dit: None,
            sur_la_poubelle: false,
            position: p,
        })
    }

    /// Le pointeur bouge en `p` : le glisser devient actif au-delà du seuil, la place
    /// visée se recalcule ; rend le `Survol` à dire au jeu quand elle change.
    pub fn deplacer(&mut self, plateau: &Plateau, p: (f32, f32)) -> Option<Deplace> {
        self.position = p;
        if !self.actif {
            let (dx, dy) = (p.0 - self.depart.0, p.1 - self.depart.1);
            if dx * dx + dy * dy <= plateau.seuil * plateau.seuil {
                return None;
            }
            self.actif = true;
        }
        self.sur_la_poubelle = plateau.poubelle.is_some_and(|(_, b)| contient(&b, p));
        let vise = if self.sur_la_poubelle {
            plateau.poubelle.map(|(c, _)| (c, 0))
        } else {
            plateau
                .zones
                .iter()
                .rev()
                .find(|z| contient(&z.boite, p))
                .map(|z| (z.cle, place_visee(&z.objets, self.objet, p)))
        };
        if let Some(v) = vise
            && !self.sur_la_poubelle
        {
            self.place = v;
        }
        let a_dire = vise.unwrap_or(self.place);
        if self.dit == Some(a_dire) {
            return None;
        }
        self.dit = Some(a_dire);
        Some(Deplace::Survol {
            objet: self.objet,
            zone: a_dire.0,
            indice: a_dire.1,
        })
    }

    /// Le pointeur lâche : un glisser actif dépose l'objet à la place visée, ou dans la
    /// poubelle ; sous le seuil, rien.
    pub fn relacher(&self, plateau: &Plateau) -> Option<Deplace> {
        if !self.actif {
            return None;
        }
        let (zone, indice) = match (self.sur_la_poubelle, plateau.poubelle) {
            (true, Some((c, _))) => (c, 0),
            _ => self.place,
        };
        Some(Deplace::Depose {
            objet: self.objet,
            zone,
            indice,
        })
    }

    /// Échap, `pointercancel` : un glisser actif s'annule.
    pub const fn annuler(&self) -> Option<Deplace> {
        if self.actif {
            Some(Deplace::Annule { objet: self.objet })
        } else {
            None
        }
    }

    /// Ce que le rendu en dessine : la copie au pointeur, la marque de la place visée.
    pub fn vu(&self, plateau: &Plateau) -> Option<GlisserVu> {
        if !self.actif {
            return None;
        }
        let b = plateau.boite_de(self.objet)?;
        let decalage = (
            self.position.0 - self.saisie.0 - b[0],
            self.position.1 - self.saisie.1 - b[1],
        );
        let marque = if self.sur_la_poubelle {
            None
        } else {
            marque(plateau, self.objet, self.place)
        };
        Some(GlisserVu {
            objet: self.objet,
            decalage,
            marque,
        })
    }
}

/// La marque de la place `(zone, indice)` : un trait vertical au bord gauche de l'objet
/// qui y est (sans l'objet tenu), ou au bord droit du dernier, ou au début de la zone
/// vide.
fn marque(plateau: &Plateau, tenu: Cle, (zone, indice): (Cle, u32)) -> Option<Boite> {
    let z = plateau.zones.iter().find(|z| z.cle == zone)?;
    let autres: Vec<&Boite> = z
        .objets
        .iter()
        .filter(|(c, _)| *c != tenu)
        .map(|(_, b)| b)
        .collect();
    let demi = LARGEUR_DE_LA_MARQUE / 2.0;
    let k = usize::try_from(indice).unwrap_or(usize::MAX);
    let (x, haut, bas) = match (autres.get(k), autres.last()) {
        (Some(b), _) => (b[0], b[1], b[3]),
        (None, Some(b)) => (b[2], b[1], b[3]),
        (None, None) => {
            let hauteur = z.boite[3] - z.boite[1];
            (z.boite[0] + demi + 1.0, z.boite[1], z.boite[1] + hauteur)
        }
    };
    Some([x - demi, haut, x + demi, bas])
}
