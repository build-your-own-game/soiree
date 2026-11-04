//! Une soirée de `devine`, écrite à la main : deux joueurs et un curieux, deux
//! manches, un gel, une demande et sa réponse, un départ et un retour, et des
//! fautes injectées à des endroits choisis.
//!
//! Les fautes, chacune avec ce qu'elle éprouve (§ 4.6) :
//!
//! | Nom | Où | Copie native | Module |
//! |---|---|---|---|
//! | `F1` | la première proposition de A | écrite, puis panique | écrite, puis panique |
//! | `F2` | la première proposition de B | écrite, puis `Budget` | carburant de 100 : `Budget` réel |
//! | `F3` | une vue de 30 000 regards | `Panique`, sans appeler | brouillon épuisé : panique réelle |
//! | `F4` | chaque `Temps` dès le pas 2000 | écrit, puis panique | écrit, puis panique |
//!
//! `F4` écarte trois `Temps` de suite : la partie passe en panne (§ 4.6).

use devine::{ActionEcran, Curseur, GesteTel, Prenom};
use jeu::prelude::{Borne, Texte};
use jeu_abi::{
    Appareil, Appel, Capacites, CauseDepart, Duree, EntreeFil, IdDemande, IdGeste, Instant, Pas,
    Presence, RegardFil, RoleAppareil, decoder, encoder,
};
use partie::{Arrivee, EcranArrivee, Faute, Placement, ReponseArrivee};

use crate::duo::{Duo, Points};
use crate::piste::{Effet, Regle};

/// Les trois téléphones : deux joueurs, et un curieux qui ne rejoint jamais.
pub const A: u64 = 0x1111_2222_3333_4444;
pub const B: u64 = 0x5555_6666_7777_8888;
pub const C: u64 = 0x9999_aaaa_bbbb_cccc;

/// Le pas où commencent les `Temps` fautifs de `F4`.
pub const TEMPS_FAUTIFS: u64 = 2000;
/// Le dernier pas de la soirée.
pub const FIN: u64 = 2600;
/// Le message des paniques injectées.
pub const PANIQUE_INJECTEE: &str = "faute injectée par la piste";

/// La sorte d'instance dont on écrit les règles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cote {
    Natif,
    Module,
}

pub fn octets<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).expect("encoder");
    o
}

fn instant(s: u64) -> Instant {
    Instant::de(Pas::du_fil(s))
}

/// Un geste de téléphone, fait au pas `s` sur la vue `vu`.
pub fn geste(de: u64, g: &GesteTel, s: u64, vu: u64, id: u64) -> Arrivee {
    Arrivee::Geste {
        de: Appareil::du_fil(de),
        octets: octets(g),
        fait: instant(s),
        vu: Pas::du_fil(vu),
        id: IdGeste::du_fil(id),
        demi_aller_retour: Duree::ZERO,
    }
}

pub fn rejoindre(de: u64, nom: &str, s: u64, vu: u64, id: u64) -> Arrivee {
    geste(
        de,
        &GesteTel::Rejoindre(Texte::<Prenom>::nettoyer(nom)),
        s,
        vu,
        id,
    )
}

/// Le curseur d'un téléphone : un tenu, le dernier gagne.
pub fn curseur(de: u64, c: i64, numero: u64, s: u64) -> Arrivee {
    Arrivee::Tenu {
        de: Appareil::du_fil(de),
        octets: octets(&Curseur(Borne::new(c).expect("un chiffre"))),
        numero,
        fait: instant(s),
        demi_aller_retour: Duree::ZERO,
    }
}

pub fn ecran(a: ActionEcran, vu: u64) -> Arrivee {
    Arrivee::Ecran {
        quoi: EcranArrivee::Action(octets(&a)),
        vu: Pas::du_fil(vu),
    }
}

pub fn arrivee(de: u64) -> Arrivee {
    Arrivee::Presence(Presence::Arrivee {
        appareil: Appareil::du_fil(de),
        role: RoleAppareil::Telephone,
        capacites: Capacites::AUCUNE,
    })
}

/// Une proposition de cet appareil, telle que le cœur l'envoie au jeu.
fn proposition_de(de: u64) -> impl Fn(&Appel<'_>) -> bool {
    let proposer = octets(&GesteTel::Proposer);
    move |a| match a {
        Appel::Entree { entree, .. } => matches!(
            decoder::<EntreeFil<'_>>(entree),
            Ok(EntreeFil::Geste { de: d, octets, .. }) if d == de && octets == proposer.as_slice()
        ),
        _ => false,
    }
}

/// Un `Temps` placé à ce pas ou après.
fn temps_des(p: u64) -> impl Fn(&Appel<'_>) -> bool {
    move |a| match a {
        Appel::Entree { place, entree, .. } => {
            *place >= p
                && matches!(
                    decoder::<EntreeFil<'_>>(entree),
                    Ok(EntreeFil::Temps { .. })
                )
        }
        _ => false,
    }
}

/// Une vue démesurée : plus de regards que le brouillon du module ne peut en rendre.
fn vue_demesuree(a: &Appel<'_>) -> bool {
    matches!(a, Appel::Regarder { regards } if regards.len() > 1000)
}

fn panique() -> Faute {
    Faute::Panique {
        message: String::from(PANIQUE_INJECTEE),
    }
}

/// Les fautes de la soirée, pour un côté.
pub fn regles(cote: Cote) -> Vec<Regle> {
    let mut r = vec![Regle {
        nom: "F1",
        cible: Box::new(proposition_de(A)),
        effet: Effet::EcrireEtFauter(panique()),
        fois: 1,
    }];
    r.push(Regle {
        nom: "F2",
        cible: Box::new(proposition_de(B)),
        effet: match cote {
            Cote::Natif => Effet::EcrireEtFauter(Faute::Budget),
            Cote::Module => Effet::Budget(100),
        },
        fois: 1,
    });
    if cote == Cote::Natif {
        r.push(Regle {
            nom: "F3",
            cible: Box::new(vue_demesuree),
            effet: Effet::Fauter(Faute::Panique {
                message: String::from("brouillon épuisé, injecté"),
            }),
            fois: 1,
        });
    }
    r.push(Regle {
        nom: "F4",
        cible: Box::new(temps_des(TEMPS_FAUTIFS)),
        effet: Effet::EcrireEtFauter(panique()),
        fois: u32::MAX,
    });
    r
}

/// Ce que la soirée a laissé de quoi vérifier.
pub struct Bilan {
    /// Des points de reprise pris en chemin : à la création, pendant la première
    /// manche, pendant la seconde.
    pub points: Vec<(u64, Points)>,
    /// L'identité de la demande de la première manche.
    pub demande: IdDemande,
    /// Les points pris par `Partie::point`, en plus de ceux du cœur.
    pub points_explicites: u64,
}

/// Joue la soirée entière sur les deux parties, en comparant à chaque pas.
#[expect(
    clippy::too_many_lines,
    reason = "une soirée se lit d'un trait, dans l'ordre de ses pas"
)]
pub fn jouer(d: &mut Duo) -> Bilan {
    let mut points = vec![(0, d.point())];
    let tous = crate::duo::TOUS;
    let _: Vec<partie::Vue> = d.regarder(&tous);
    let mut gestes = 0;
    let mut prochain = || {
        gestes += 1;
        gestes
    };
    let mut numero_a = 0;
    let mut numero_b = 0;
    let mut demande = None;
    while d.definitif() < FIN {
        let s = d.definitif() + 1;
        let vu = d.vu;
        match s {
            1 => {
                for de in [A, B, C] {
                    assert!(matches!(
                        d.arriver(&arrivee(de), s),
                        Placement::Place { .. }
                    ));
                }
            }
            2 => {
                let _: Placement = d.arriver(&rejoindre(A, "Léa", s, vu, prochain()), s);
                // Un prénom hostile : marques combinantes, invisibles, inversion, drapeau.
                let hostile = "Z\u{0338}\u{0322}o\u{200B}é\u{202E}🇫🇷👩\u{200D}👧";
                let _: Placement = d.arriver(&rejoindre(B, hostile, s, vu, prochain()), s);
                // Le curieux propose sans être joueur, hors d'une manche : refusé.
                let _: Placement = d.arriver(&geste(C, &GesteTel::Proposer, s, vu, prochain()), s);
            }
            // « Rejouer » : la scène du salon ne le porte peut-être pas ; les deux
            // parties en décident de même.
            3 => {
                let _: Placement = d.arriver(&ecran(ActionEcran::Rejouer, vu), s);
            }
            4 | 1320 => {
                let p = d.arriver(&ecran(ActionEcran::Lancer, vu), s);
                assert!(matches!(p, Placement::Place { .. }), "Lancer : {p:?}");
            }
            10 | 11 => {
                numero_a += 1;
                let c = if s == 10 { 4 } else { 7 };
                let _: Placement = d.arriver(&curseur(A, c, numero_a, s), s);
            }
            // F1 écarte la première ; la deuxième est prise ; la troisième est refusée.
            20 | 30 | 31 | 1500 => {
                let _: Placement = d.arriver(&geste(A, &GesteTel::Proposer, s, vu, prochain()), s);
            }
            40 => {
                numero_b += 1;
                let _: Placement = d.arriver(&curseur(B, 3, numero_b, s), s);
            }
            // F2 écarte la première.
            50 | 60 | 1510 => {
                let _: Placement = d.arriver(&geste(B, &GesteTel::Proposer, s, vu, prochain()), s);
            }
            1210 => {
                let p = d.arriver(
                    &Arrivee::Reponse {
                        id: demande.expect("la demande de la première manche"),
                        reponse: ReponseArrivee::Fait(Ok(())),
                    },
                    s,
                );
                assert!(matches!(p, Placement::Place { .. }), "réponse : {p:?}");
            }
            1300 => {
                let _: Placement = d.arriver(
                    &Arrivee::Presence(Presence::Depart {
                        appareil: Appareil::du_fil(B),
                        cause: CauseDepart::Fermeture,
                    }),
                    s,
                );
            }
            1310 => {
                let _: Placement = d.arriver(
                    &Arrivee::Presence(Presence::Retour {
                        appareil: Appareil::du_fil(B),
                        capacites: Capacites::AUCUNE,
                    }),
                    s,
                );
            }
            // Des curseurs qui bougent sans cesse, plus vite que le débit permis :
            // assez d'entrées pour qu'une empreinte soit notée au journal.
            100..=1100 => {
                if s.is_multiple_of(3) {
                    numero_a += 1;
                    let c = i64::try_from(s % 10).expect("un chiffre");
                    let _: Placement = d.arriver(&curseur(A, c, numero_a, s), s);
                }
                if s.is_multiple_of(5) {
                    numero_b += 1;
                    let c = i64::try_from((s / 5) % 10).expect("un chiffre");
                    let _: Placement = d.arriver(&curseur(B, c, numero_b, s), s);
                }
            }
            _ => {}
        }
        if s == 70 {
            // F3 : une vue démesurée. Le module épuise son brouillon et panique ; la
            // dernière vue valide de l'écran est gardée, des deux côtés.
            let avant = d.regarder(&[RegardFil::Ecran]);
            let enorme = vec![RegardFil::Ecran; 30_000];
            let apres = d.regarder(&enorme);
            assert_eq!(apres.len(), enorme.len());
            assert!(
                apres.iter().all(|v| *v == avant[0]),
                "la dernière vue est gardée"
            );
        }
        // Un gel : l'horloge saute de 50 pas d'un coup ; le cœur joue un pas, puis le
        // numéro saute (rattrapage 1, § 4.1).
        let cible = if s == 800 { 850 } else { s };
        let a = d.avancer(cible);
        if s == 800 {
            assert_eq!(a.saut, Duree::pas(50), "le saut après le gel");
            assert_eq!(d.definitif(), 850);
        }
        if let Some((i, _)) = a.lancer.first() {
            assert!(
                demande.is_none(),
                "une seule demande, à la première révélation"
            );
            demande = Some(*i);
        }
        if s == 600 || s == 1400 {
            points.push((s, d.point()));
        }
    }
    Bilan {
        points,
        demande: demande.expect("la première manche lance sa ligne de résultats"),
        points_explicites: 3,
    }
}
