//! L'écran de démarrage (PHASE-3, § 2.8 ; lot 10b) : **une scène du commun**, du Rust qui
//! produit une `SceneFil`, que `rendu` dessine comme celle d'un jeu. Sans navigateur :
//! testée en natif, ses textes mesurés par `mise-en-page` comme E16 les mesure.
//!
//! Elle montre la liste des jeux du catalogue (nom, places, date de l'épreuve, ou « NON
//! ÉPROUVÉ »), celui que la salle joue, le QR, l'adresse, le code de salle, les téléphones
//! reliés, et l'aide ([`crate::aide`]). Son bouton « Jouer » est **le geste** qu'exigent
//! le plein écran, le son (SAL-174, SAL-175) et le verrou d'écran : la page reconnaît le
//! clic sur lui ([`bouton_sous`]) et fait le reste dans le gestionnaire du clic.
//!
//! Changer de jeu (`CommandeEcran::ChangerDeJeu`) n'est pas encore écrit (lot 12) : la
//! liste dit quel jeu la salle joue ; les autres s'y lisent.

use core::fmt::Write as _;

use jeu_abi::{
    Alignement, Cle, Coupe, Fixe, FormeEcran, Lignes, NoeudFil, Oui, Rect, Rgba, SceneFil,
    SorteNoeud, StyleTexte, Taille,
};

use crate::aide::Message;

/// La forme de l'écran de démarrage : 1920 × 1080, dans une zone sûre de 90 % (HIS-46).
pub const FORME: FormeEcran = FormeEcran::Scene {
    largeur: 1920,
    hauteur: 1080,
    zone_sure_pour_cent: 90,
    echelle_entiere: Oui::Non,
};

/// L'action du bouton « Jouer » : une action du commun, jamais envoyée au jeu.
pub const ACTION_JOUER: &[u8] = b"commun:jouer";

/// Les jeux montrés au plus ; au-delà, « et N autres ».
pub const JEUX_MONTRES: usize = 6;

/// Un jeu du catalogue (`GET /jeux/catalogue`, et son rapport `GET /jeux/<empreinte>.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JeuAuCatalogue {
    /// L'empreinte du module livré.
    pub empreinte: [u8; 32],
    /// Son nom, lu dans son rapport.
    pub nom: Option<String>,
    /// Ses joueurs, au moins et au plus.
    pub joueurs: Option<(u16, u16)>,
    /// La date de son épreuve.
    pub date: Option<String>,
    /// A-t-il un rapport d'épreuve ? Sinon, « NON ÉPROUVÉ » (P17).
    pub eprouve: bool,
}

/// Ce que l'écran de démarrage montre.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Demarrage {
    /// Les jeux du catalogue.
    pub jeux: Vec<JeuAuCatalogue>,
    /// Le jeu que la salle joue (`VersPage::Jeu`).
    pub en_jeu: Option<[u8; 32]>,
    /// Le code de salle.
    pub salle: Option<String>,
    /// L'adresse que les téléphones joignent, sans le fragment (qui ne s'affiche jamais en
    /// clair : il n'est que dans le QR).
    pub adresse: Option<String>,
    /// Le QR se montre-t-il (une adresse annoncée, un fragment) ?
    pub qr: bool,
    /// Les téléphones reliés.
    pub telephones: usize,
    /// L'aide.
    pub aide: Vec<Message>,
}

const BLANC: Rgba = Rgba::BLANC;
const GRIS: Rgba = Rgba(170, 170, 178, 255);
const AIDE: Rgba = Rgba(255, 214, 102, 255);
const ALERTE: Rgba = Rgba(255, 120, 110, 255);
const ACCENT: Rgba = Rgba(90, 170, 255, 255);
const FOND: Rgba = Rgba(14, 14, 20, 255);

fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

fn style(max: i32, min: i32, couleur: Rgba, lignes: Lignes, coupe: Coupe) -> StyleTexte {
    StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: Taille::Budget {
            max: Fixe::unites(max),
            min: Fixe::unites(min),
        },
        couleur,
        alignement: Alignement::Debut,
        lignes,
        coupe,
    }
}

fn texte(noeuds: &mut Vec<NoeudFil>, cle: Cle, t: String, boite: Rect, s: StyleTexte) {
    noeuds.push(NoeudFil::simple(
        cle,
        None,
        SorteNoeud::Texte {
            texte: t,
            boite,
            style: s,
        },
    ));
}

/// Les quatre premiers et quatre derniers chiffres d'une empreinte, pour nommer un module
/// sans rapport.
fn court(e: &[u8; 32]) -> String {
    let h = |o: &[u8]| {
        o.iter().fold(String::new(), |mut s, b| {
            let _: core::fmt::Result = write!(s, "{b:02x}");
            s
        })
    };
    format!(
        "{}…{}",
        h(e.get(..2).unwrap_or(&[])),
        h(e.get(30..).unwrap_or(&[]))
    )
}

/// Ce qu'une ligne de jeu dit à droite de son nom.
fn detail(j: &JeuAuCatalogue) -> String {
    let joueurs = j.joueurs.map(|(a, b)| {
        if a == b {
            format!("{a} joueur{}", if a > 1 { "s" } else { "" })
        } else {
            format!("{a} à {b} joueurs")
        }
    });
    let epreuve = if j.eprouve {
        j.date
            .as_ref()
            .map_or_else(|| String::from("éprouvé"), |d| format!("éprouvé le {d}"))
    } else {
        String::from("NON ÉPROUVÉ")
    };
    joueurs.map_or(epreuve.clone(), |p| format!("{p} · {epreuve}"))
}

/// Une ligne par jeu (au plus [`JEUX_MONTRES`]), celle du jeu en cours marquée.
fn liste_des_jeux(n: &mut Vec<NoeudFil>, d: &Demarrage) {
    for (k, j) in d.jeux.iter().take(JEUX_MONTRES).enumerate() {
        let rang = i32::try_from(k).unwrap_or(0);
        let y = 250_i32.saturating_add(rang.saturating_mul(85));
        if d.en_jeu == Some(j.empreinte) {
            n.push(NoeudFil::simple(
                Cle::de(&("demarrage.en-jeu", k)),
                None,
                SorteNoeud::Rect {
                    r: rect(80, y.saturating_add(8), 16, 54),
                    couleur: ACCENT,
                },
            ));
        }
        let nom = j.nom.clone().unwrap_or_else(|| court(&j.empreinte));
        texte(
            n,
            Cle::de(&("demarrage.nom", k)),
            nom,
            rect(120, y, 520, 70),
            style(44, 22, BLANC, Lignes::Une, Coupe::Graphemes),
        );
        let couleur = if j.eprouve { GRIS } else { ALERTE };
        texte(
            n,
            Cle::de(&("demarrage.detail", k)),
            detail(j),
            rect(660, y, 520, 70),
            style(30, 14, couleur, Lignes::AuPlus(2), Coupe::Mots),
        );
    }
    if d.jeux.len() > JEUX_MONTRES {
        texte(
            n,
            Cle::nommee("demarrage.autres"),
            format!("et {} autres", d.jeux.len().saturating_sub(JEUX_MONTRES)),
            rect(120, 760, 960, 40),
            style(28, 14, GRIS, Lignes::Une, Coupe::Mots),
        );
    }
}

/// La scène de l'écran de démarrage.
pub fn scene(d: &Demarrage) -> SceneFil {
    let mut n = Vec::new();
    texte(
        &mut n,
        Cle::nommee("demarrage.titre"),
        String::from("Soirée"),
        rect(80, 50, 900, 100),
        style(80, 40, BLANC, Lignes::Une, Coupe::Mots),
    );
    texte(
        &mut n,
        Cle::nommee("demarrage.jeux"),
        String::from("Les jeux"),
        rect(80, 180, 1000, 50),
        style(36, 20, GRIS, Lignes::Une, Coupe::Mots),
    );
    if d.jeux.is_empty() {
        texte(
            &mut n,
            Cle::nommee("demarrage.aucun"),
            String::from("Aucun jeu au catalogue."),
            rect(120, 250, 960, 60),
            style(40, 20, ALERTE, Lignes::Une, Coupe::Mots),
        );
    }
    liste_des_jeux(&mut n, d);
    n.push(NoeudFil::simple(
        Cle::nommee("demarrage.jouer"),
        None,
        SorteNoeud::Bouton {
            boite: rect(80, 840, 460, 130),
            nom: String::from("Jouer"),
            action: ACTION_JOUER.to_vec(),
        },
    ));
    // La colonne de droite : le QR, l'adresse, la salle, les téléphones.
    if d.qr {
        n.push(NoeudFil::simple(
            Cle::nommee("demarrage.qr"),
            None,
            SorteNoeud::Qr {
                boite: rect(1340, 80, 420, 420),
            },
        ));
    }
    if let Some(a) = &d.adresse {
        texte(
            &mut n,
            Cle::nommee("demarrage.adresse"),
            a.clone(),
            rect(1240, 520, 620, 80),
            style(30, 14, BLANC, Lignes::AuPlus(2), Coupe::Graphemes),
        );
    }
    if let Some(s) = &d.salle {
        texte(
            &mut n,
            Cle::nommee("demarrage.salle"),
            format!("Salle {s}"),
            rect(1240, 610, 620, 50),
            style(34, 16, GRIS, Lignes::Une, Coupe::Mots),
        );
    }
    let telephones = match d.telephones {
        0 => String::from("Aucun téléphone relié"),
        1 => String::from("1 téléphone relié"),
        t => format!("{t} téléphones reliés"),
    };
    texte(
        &mut n,
        Cle::nommee("demarrage.telephones"),
        telephones,
        rect(1240, 670, 620, 50),
        style(34, 16, GRIS, Lignes::Une, Coupe::Mots),
    );
    for (k, m) in d.aide.iter().enumerate() {
        let rang = i32::try_from(k).unwrap_or(0);
        texte(
            &mut n,
            Cle::de(&("demarrage.aide", m.nom())),
            String::from(m.texte()),
            rect(
                600,
                740_i32.saturating_add(rang.saturating_mul(100)),
                1260,
                92,
            ),
            style(26, 12, AIDE, Lignes::AuPlus(3), Coupe::Mots),
        );
    }
    SceneFil {
        fond: Some(FOND),
        noeuds: n,
        ..SceneFil::vide()
    }
}

/// Les textes de la scène, dans l'ordre : ce que le banc lit de ce que l'écran montre.
pub fn textes(s: &SceneFil) -> Vec<&str> {
    s.noeuds
        .iter()
        .filter_map(|n| {
            if let SorteNoeud::Texte { texte, .. } = &n.sorte {
                Some(texte.as_str())
            } else if let SorteNoeud::Bouton { nom, .. } = &n.sorte {
                Some(nom.as_str())
            } else {
                None
            }
        })
        .collect()
}

/// L'action du bouton de la scène sous le point `(x, y)`, dans le repère de la scène (les
/// boutons à la racine : l'écran du commun n'a pas de groupe).
pub fn bouton_sous(s: &SceneFil, x: f64, y: f64) -> Option<&[u8]> {
    s.noeuds.iter().find_map(|n| {
        let SorteNoeud::Bouton { boite, action, .. } = &n.sorte else {
            return None;
        };
        let (bx, by) = (boite.x.en_unites(), boite.y.en_unites());
        let dedans = n.parent.is_none()
            && x >= bx
            && y >= by
            && x < bx + boite.l.en_unites()
            && y < by + boite.h.en_unites();
        dedans.then_some(action.as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::{ACTION_JOUER, Demarrage, JeuAuCatalogue, bouton_sous, scene, textes};
    use crate::aide::Message;
    use jeu_abi::{SorteNoeud, StyleTexte};
    use mise_en_page::{MiseEnPage, Polices, Tient};

    fn pire_cas() -> Demarrage {
        let jeux = (0..8u8)
            .map(|k| JeuAuCatalogue {
                empreinte: [k; 32],
                nom: Some(if k == 1 {
                    String::from("Un jeu au nom vraiment très long, pour voir")
                } else {
                    format!("Jeu {k}")
                }),
                joueurs: Some((1, 71)),
                date: Some(String::from("2026-11-07")),
                eprouve: k != 2,
            })
            .collect();
        Demarrage {
            jeux,
            en_jeu: Some([1; 32]),
            salle: Some(String::from("3FA9C2D1")),
            adresse: Some(String::from(
                "http://[fd12:3456:789a:1::42]:7878/s/3FA9C2D1",
            )),
            qr: true,
            telephones: 71,
            aide: vec![Message::PageSansLien, Message::WifiIsole, Message::PareFeu],
        }
    }

    /// Chaque texte de l'écran de démarrage tient dans sa boîte, sans glyphe absent, au
    /// pire cas (huit jeux, un nom long, une adresse IPv6, trois messages d'aide), mesuré
    /// comme E16 mesure un jeu (PHASE-3, § 2.9).
    #[test]
    fn chaque_texte_tient_au_pire_cas() {
        let mut m = MiseEnPage::new(&Polices::du_commun());
        for d in [
            pire_cas(),
            Demarrage {
                aide: vec![Message::PasDeReseau],
                qr: false,
                ..Demarrage::default()
            },
        ] {
            let s = scene(&d);
            s.verifier_structure().unwrap();
            for n in &s.noeuds {
                if let SorteNoeud::Texte {
                    texte,
                    boite,
                    style,
                } = &n.sorte
                {
                    let a = m.agencer(texte, *boite, style as &StyleTexte);
                    assert_eq!(a.tient, Tient::Oui, "{texte}");
                    assert_eq!(a.absents, 0, "{texte}");
                }
            }
        }
    }

    /// La scène dit ce qu'il faut : la liste (« NON ÉPROUVÉ » pour un module sans
    /// rapport), le QR, la salle, les téléphones, l'aide ; sans réseau, pas de QR.
    #[test]
    fn la_scene_dit_la_salle_et_l_aide() {
        let s = scene(&pire_cas());
        let t = textes(&s);
        assert!(t.contains(&"Jouer"));
        assert!(t.contains(&"Salle 3FA9C2D1"));
        assert!(t.contains(&"71 téléphones reliés"));
        assert!(t.iter().any(|x| x.contains("NON ÉPROUVÉ")));
        assert!(
            t.iter()
                .any(|x| x.contains("1 à 71 joueurs · éprouvé le 2026-11-07"))
        );
        assert!(t.iter().any(|x| x.starts_with("et 2 autres")));
        assert!(t.contains(&Message::WifiIsole.texte()));
        assert!(
            s.noeuds
                .iter()
                .any(|n| matches!(n.sorte, SorteNoeud::Qr { .. }))
        );
        let sans = scene(&Demarrage {
            aide: vec![Message::PasDeReseau],
            ..Demarrage::default()
        });
        assert!(
            !sans
                .noeuds
                .iter()
                .any(|n| matches!(n.sorte, SorteNoeud::Qr { .. }))
        );
        assert!(textes(&sans).contains(&Message::PasDeReseau.texte()));
        assert!(textes(&sans).contains(&"Aucun jeu au catalogue."));
    }

    /// Le bouton « Jouer » se trouve sous son clic, et nulle part ailleurs.
    #[test]
    fn le_bouton_jouer_se_trouve_sous_le_clic() {
        let s = scene(&Demarrage::default());
        assert_eq!(bouton_sous(&s, 300.0, 900.0), Some(ACTION_JOUER));
        assert_eq!(bouton_sous(&s, 79.0, 900.0), None);
        assert_eq!(bouton_sous(&s, 1500.0, 300.0), None);
    }
}
