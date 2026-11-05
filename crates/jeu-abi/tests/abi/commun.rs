//! Le corpus des tests : un message par variant d'`Appel`, de `Retour` et
//! d'`EntreeFil`, et ses octets écrits à la main, sans l'encodeur (PUN-90).
//!
//! Chaque suite d'octets se lit avec le format décrit en tête de `jeu_abi` : varint
//! LEB128 pour les entiers non signés et les longueurs, zigzag pour les signés,
//! `u8` brut, `f32` en petit-boutiste, numéro de variant en varint.

use jeu_abi::{
    Accueil, Appareil, Appel, ApplicableFil, AssetFil, AttenteFil, Bonjour, Capacites, CauseDepart,
    CauseTenu, Cle, CodeTouche, Composant, ConstatFil, CoupFil, DeclarationFil, DemandeFil, Duree,
    EchecDemande, EcranFil, Effet, EntreeFil, ErreurFil, FinSaisie, Fixe, FormeEcran,
    GenrePointeur, IdEffet, IdFichier, Illisible, Instant, IssueFil, Limites, Manette, Mecanique,
    Mods, MontreFil, NoeudFil, Oui, Pas, PauseFil, PhasePointeur, PhaseTouche, Places, Pointeur,
    Presence, QuandFil, Rect, RegardFil, Reglages, ReglesTenuFil, Repartition, Reponse, Retour,
    ReveilFil, Rgba, RoleAppareil, SceneFil, Sensation, SorteAsset, SorteNoeud, SorteOctets,
    SortieFil, StyleFil, TelephonesFil, TenuFil, Tenue, TexteCourt, Touche, Validation, Vibration,
    Voulues, VueFil, decoder, encoder,
};

/// Un message de l'un des types figés.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Appel(Appel<'static>),
    Retour(Retour<'static>),
    Entree(EntreeFil<'static>),
    Montre(MontreFil<'static>),
    Scene(SceneFil),
}

impl Message {
    pub fn encoder(&self) -> Vec<u8> {
        let mut o = Vec::new();
        match self {
            Message::Appel(m) => encoder(m, &mut o),
            Message::Retour(m) => encoder(m, &mut o),
            Message::Entree(m) => encoder(m, &mut o),
            Message::Montre(m) => encoder(m, &mut o),
            Message::Scene(m) => encoder(m, &mut o),
        }
        .unwrap();
        o
    }

    /// Décode `octets` comme le type de ce message, et dit s'il retrouve ce message.
    pub fn relire(&self, octets: &[u8]) -> Result<bool, ErreurFil> {
        Ok(match self {
            Message::Appel(m) => decoder::<Appel<'_>>(octets)? == *m,
            Message::Retour(m) => decoder::<Retour<'_>>(octets)? == *m,
            Message::Entree(m) => decoder::<EntreeFil<'_>>(octets)? == *m,
            Message::Montre(m) => decoder::<MontreFil<'_>>(octets)? == *m,
            Message::Scene(m) => decoder::<SceneFil>(octets)? == *m,
        })
    }
}

/// Un cas du corpus.
pub struct Cas {
    pub nom: &'static str,
    pub message: Message,
    pub attendu: Vec<u8>,
}

/// Des octets écrits en hexadécimal, séparés par des espaces ; `AB*32` répète.
pub fn h(s: &str) -> Vec<u8> {
    let mut o = Vec::new();
    for jeton in s.split_whitespace() {
        let (octet, fois) = match jeton.split_once('*') {
            Some((x, n)) => (x, n.parse::<usize>().unwrap()),
            None => (jeton, 1),
        };
        let b = u8::from_str_radix(octet, 16).unwrap();
        o.extend(std::iter::repeat_n(b, fois));
    }
    o
}

/// Le `QuandFil` des entrées du corpus, et ses octets.
pub const QUAND: QuandFil = QuandFil {
    fait: 10,
    fraction: 0x8000,
    recu: 11,
    vu: 9,
    id: 1,
};
/// fait 10, fraction 32 768 (80 80 02), reçu 11, vu 9, id 1.
pub const Q: &str = "0A 80 80 02 0B 09 01";

pub fn declaration() -> DeclarationFil {
    DeclarationFil {
        nom: TexteCourt::new("d"),
        places: Places::joueurs(1, 8).appareils(16),
        rattrapage: 1,
        fenetre: ApplicableFil::SansObjet("a b c".to_owned()),
        antidatage_max: Duree::pas(2),
        pause: PauseFil::DuCommun,
        ecran: FormeEcran::Scene {
            largeur: 16,
            hauteur: 9,
            zone_sure_pour_cent: 90,
            echelle_entiere: Oui::Non,
        },
        telephones: ApplicableFil::Oui(TelephonesFil {
            repartition: Repartition::UnParJoueur,
            tenu: ReglesTenuFil {
                fraicheur: ApplicableFil::Oui(Duree::ms(450)),
                age_max: ApplicableFil::Oui(Duree::s(1)),
                debit_max: 10,
            },
        }),
        manettes: ApplicableFil::SansObjet("x y z".to_owned()),
        prediction: ApplicableFil::Oui(()),
        hasard: ApplicableFil::Oui(()),
        secrets: ApplicableFil::Oui(()),
        fin: ApplicableFil::Oui(()),
        capacites: Voulues {
            vibration: Oui::Non,
            mouvement: Oui::Non,
            son: Oui::Non,
            ecran_allume: Oui::Non,
            micro: Oui::Non,
            pad_natif: Oui::Non,
        },
        limites: Limites {
            geste: 256,
            file: 32,
            vue: 4096,
            effets: 8,
            memoire: 64,
            brouillon: 1 << 20,
        },
        diagnostic: CodeTouche::new("KeyD"),
        assets: vec![AssetFil {
            cle: Cle::du_fil(3),
            sorte: SorteAsset::Image,
            taille: 10,
        }],
        mecaniques: vec![Mecanique::Hasard, Mecanique::Texte],
    }
}

/// Les octets de `declaration()`, champ par champ.
const DECLARATION: &str = concat!(
    "01 64 ",                         // nom : 1 octet, « d »
    "01 08 10 ",                      // places : 1, 8, 16
    "01 ",                            // rattrapage
    "01 05 61 20 62 20 63 ",          // fenetre : SansObjet, « a b c »
    "02 ",                            // antidatage_max : 2 pas
    "01 ",                            // pause : DuCommun
    "00 10 09 5A 01 ",                // ecran : Scene 16 × 9, 90 %, echelle_entiere Non
    "00 00 00 1B 00 3C 0A ", // telephones : Oui, UnParJoueur, fraîcheur 27 pas, âge 60 pas, débit 10
    "01 05 78 20 79 20 7A ", // manettes : SansObjet, « x y z »
    "00 00 00 00 ",          // prediction, hasard, secrets, fin : Oui(())
    "01 01 01 01 01 01 ",    // capacites : six Non
    "80 02 20 80 20 08 40 80 80 40 ", // limites : 256, 32, 4096, 8, 64, 1 048 576
    "04 4B 65 79 44 ",       // diagnostic : « KeyD »
    "01 03 01 0A ",          // assets : un, clé 3, Image, 10 octets
    "02 05 04",              // mecaniques : Hasard, Texte
);

#[expect(clippy::too_many_lines, reason = "un cas par variant, écrit à la main")]
pub fn corpus() -> Vec<Cas> {
    let cas = |nom, message, attendu: String| Cas {
        nom,
        message,
        attendu: h(&attendu),
    };
    let a = |nom, m, s: &str| cas(nom, Message::Appel(m), s.to_owned());
    let r = |nom, m, s: &str| cas(nom, Message::Retour(m), s.to_owned());
    let e = |nom, m, s: String| cas(nom, Message::Entree(m), s);
    vec![
        // --- Appel ---
        a("Appel::Bonjour", Appel::Bonjour, "00"),
        a("Appel::Decrire", Appel::Decrire, "01"),
        a(
            "Appel::Creer",
            Appel::Creer {
                graine: [0xAB; 32],
                pas: 300,
                heure: 1,
                salle: TexteCourt::new("AB12"),
                presents: &[0],
                garde: Some(&[5, 6]),
            },
            // graine, pas 300, heure 1, salle « AB12 », présents [], garde [5, 6].
            "02 AB*32 AC 02 01 04 41 42 31 32 01 00 01 02 05 06",
        ),
        a(
            "Appel::Valider",
            Appel::Valider {
                sorte: SorteOctets::GesteTelephone,
                octets: &[9],
            },
            "03 01 01 09",
        ),
        a(
            "Appel::Entree",
            Appel::Entree {
                ordre: 128,
                place: 7,
                entree: &[0xFF],
            },
            "04 80 01 07 01 FF",
        ),
        a(
            "Appel::Regarder",
            Appel::Regarder {
                regards: vec![RegardFil::Ecran, RegardFil::Appareil(5)],
            },
            "05 02 00 01 05",
        ),
        a(
            "Appel::Etat",
            Appel::Etat {
                connues: vec![1, 2],
            },
            "06 02 01 02",
        ),
        a(
            "Appel::Ressources",
            Appel::Ressources {
                cles: vec![u64::MAX],
            },
            "07 01 FF*9 01",
        ),
        a(
            "Appel::Projeter",
            Appel::Projeter {
                regard: RegardFil::Appareil(1),
                morceau: 2,
            },
            "08 01 01 02",
        ),
        a(
            "Appel::Charger",
            Appel::Charger { projection: &[] },
            "09 00",
        ),
        a("Appel::Canonique", Appel::Canonique { morceau: 0 }, "0A 00"),
        a(
            "Appel::DepuisCanonique",
            Appel::DepuisCanonique {
                forme: 1,
                octets: &[2],
            },
            "0B 01 01 02",
        ),
        a("Appel::Garder", Appel::Garder, "0C"),
        a("Appel::Resumer", Appel::Resumer, "0D"),
        a("Appel::Verifier", Appel::Verifier, "0E"),
        a(
            "Appel::Transition",
            Appel::Transition {
                avant: &[1],
                apres: &[],
            },
            "0F 01 01 00",
        ),
        a(
            "Appel::Bot",
            Appel::Bot {
                regard: RegardFil::Ecran,
                style: StyleFil::Hostile,
                graine: 3,
                depuis_vue: 4,
            },
            "10 00 01 03 04",
        ),
        a("Appel::PireCas", Appel::PireCas { indice: 1 }, "11 01"),
        a("Appel::Memoire", Appel::Memoire, "12"),
        // ABI 2 : les propriétés promises, pour E10 et E11.
        a("Appel::Proprietes", Appel::Proprietes, "13"),
        a(
            "Appel::Hypothese",
            Appel::Hypothese { appareil: 300 },
            "14 AC 02",
        ),
        // --- Retour ---
        r(
            "Retour::Bonjour",
            Retour::Bonjour(Bonjour {
                abi: 1,
                formes: 2,
                boite: (16, 32),
                brouillon: (64, 128),
                panique: (256, 4096),
                version_jeu: (0, 1, 2),
            }),
            "00 01 02 10 20 40 80 01 80 02 80 20 00 01 02",
        ),
        r(
            "Retour::Decrit",
            Retour::Decrit(Box::new(declaration())),
            &format!("01 {DECLARATION}"),
        ),
        r(
            "Retour::Issue::Acceptee",
            Retour::Issue(IssueFil::Acceptee {
                sorties: vec![
                    SortieFil::Oublier(4),
                    SortieFil::NouvellePartie { graine: [1; 32] },
                ],
                reveil: ReveilFil::A(600),
            }),
            "02 00 02 00 04 02 01*32 01 D8 04",
        ),
        r(
            "Retour::Issue::Refusee",
            Retour::Issue(IssueFil::Refusee { avis: &[7] }),
            "02 01 01 07",
        ),
        r(
            "Retour::Issue::Ignoree",
            Retour::Issue(IssueFil::Ignoree),
            "02 02",
        ),
        r(
            "Retour::Issue::Attente",
            Retour::Issue(IssueFil::Attente),
            "02 03",
        ),
        r(
            "Retour::Valide::Valide",
            Retour::Valide(Validation::Valide {
                tenu: Some(TenuFil {
                    cle: &[],
                    relache: Some(&[0]),
                    est_relache: Oui::Non,
                }),
            }),
            "03 00 01 00 01 01 00 01",
        ),
        r(
            "Retour::Valide::Malforme",
            Retour::Valide(Validation::Malforme(ErreurFil {
                ou: 3,
                quoi: Illisible::Valeur,
            })),
            "03 01 03 07",
        ),
        r(
            "Retour::Vues",
            Retour::Vues(vec![VueFil {
                regard: RegardFil::Appareil(2),
                octets: &[1, 2],
            }]),
            "04 01 01 02 02 01 02",
        ),
        r(
            "Retour::Etat",
            Retour::Etat(jeu_abi::EtatFil {
                phase: &[0],
                phase_indice: 1,
                terminale: Oui::Non,
                reveil: ReveilFil::Jamais,
                attente: AttenteFil {
                    echeance: Some(5),
                    libre: Some("ok"),
                    joueurs: vec![(2, vec![&[3]])],
                    ecran: vec![],
                },
                accueil: Accueil::Pleine,
                cas_indice: 0,
                demandes: vec![9],
                nouvelles: vec![(
                    9,
                    DemandeFil::Lire {
                        fichier: IdFichier::du_fil(4),
                        max: 100,
                    },
                )],
            }),
            concat!(
                "05 01 00 01 01 02 ",         // phase [0], indice 1, Non, Jamais
                "01 05 01 02 6F 6B ",         // échéance 5, libre « ok »
                "01 02 01 01 03 00 ",         // joueurs [(2, [[3]])], écran []
                "01 00 01 09 01 09 02 04 64"  // Pleine, cas 0, [9], [(9, Lire 4 100)]
            ),
        ),
        r(
            "Retour::Ressources",
            Retour::Ressources(vec![(5, &[1])]),
            "06 01 05 01 01",
        ),
        r(
            "Retour::Morceau",
            Retour::Morceau {
                octets: &[1],
                suivant: Some(2),
            },
            "07 01 01 01 02",
        ),
        r("Retour::Fait", Retour::Fait, "08"),
        r("Retour::Octets", Retour::Octets(&[0xAA]), "09 01 AA"),
        r(
            "Retour::Verif",
            Retour::Verif(vec![("ok", ConstatFil::Viole)]),
            "0A 01 02 6F 6B 01",
        ),
        r(
            "Retour::Bot",
            Retour::Bot(vec![CoupFil::Geste(&[1]), CoupFil::Rien]),
            "0B 02 00 01 01 05",
        ),
        r("Retour::PireCas", Retour::PireCas { cas: &[2] }, "0C 01 02"),
        r(
            "Retour::Memoire",
            Retour::Memoire {
                vivants: 1,
                tas: 65536,
            },
            "0D 01 80 80 04",
        ),
        r(
            "Retour::Illisible",
            Retour::Illisible(ErreurFil {
                ou: 0,
                quoi: Illisible::OctetsRestants,
            }),
            "0E 00 01",
        ),
        r(
            "Retour::Proprietes",
            Retour::Proprietes(jeu_abi::ProprietesFil {
                coupure_transparente: ApplicableFil::Oui(Duree::pas(600)),
                permutation: ApplicableFil::SansObjet(String::from("non")),
                phases: 3,
                invariants: vec![String::from("ok")],
            }),
            // ABI 3 : `phases` (03) et `invariants` (01, puis 02 6F 6B) à la suite.
            "0F 00 D8 04 01 03 6E 6F 6E 03 01 02 6F 6B",
        ),
        r("Retour::Hypothese", Retour::Hypothese(Oui::Non), "10 01"),
        // --- EntreeFil ---
        e(
            "EntreeFil::Temps",
            EntreeFil::Temps {
                saut: 0,
                heure: 1000,
            },
            "00 00 E8 07".to_owned(),
        ),
        e(
            "EntreeFil::Ecran::Action",
            EntreeFil::Ecran {
                quoi: EcranFil::Action(&[1]),
                quand: QUAND,
            },
            format!("01 00 01 01 {Q}"),
        ),
        e(
            "EntreeFil::Ecran::Touche",
            EntreeFil::Ecran {
                quoi: EcranFil::Touche(Touche {
                    code: CodeTouche::ESPACE,
                    mods: Mods::MAJ,
                    phase: PhaseTouche::Enfoncee,
                    repetition: Oui::Non,
                }),
                quand: QUAND,
            },
            // « Space », Maj seule, Enfoncee, répétition Non.
            format!("01 01 05 53 70 61 63 65 00 01 01 01 00 01 {Q}"),
        ),
        e(
            "EntreeFil::Ecran::Pointeur",
            EntreeFil::Ecran {
                quoi: EcranFil::Pointeur(Pointeur {
                    id: 1,
                    genre: GenrePointeur::Doigt,
                    bouton: 0,
                    phase: PhasePointeur::Deplacement,
                    pos: (Fixe::depuis_brut(256), Fixe::depuis_brut(-1)),
                }),
                quand: QUAND,
            },
            // id 1, Doigt, bouton 0, Deplacement, x zigzag(256) = 512, y zigzag(-1) = 1.
            format!("01 02 01 01 00 01 80 04 01 {Q}"),
        ),
        e(
            "EntreeFil::Ecran::Composant",
            EntreeFil::Ecran {
                quoi: EcranFil::Composant(Composant::Saisie {
                    champ: Cle::du_fil(1),
                    texte: "é",
                    fin: FinSaisie::Entree,
                }),
                quand: QUAND,
            },
            format!("01 03 00 01 02 C3 A9 01 {Q}"),
        ),
        e(
            "EntreeFil::Geste",
            EntreeFil::Geste {
                de: 3,
                octets: &[4],
                quand: QUAND,
            },
            format!("02 03 01 04 {Q}"),
        ),
        e(
            "EntreeFil::Tenu",
            EntreeFil::Tenu {
                de: 3,
                octets: &[],
                cause: CauseTenu::Silence,
                quand: QUAND,
            },
            format!("03 03 00 01 {Q}"),
        ),
        e(
            "EntreeFil::Manette",
            EntreeFil::Manette {
                de: 2,
                etat: Manette::REPOS,
                cause: CauseTenu::Geste,
                quand: QUAND,
            },
            // Deux sticks de deux f32 nuls, deux gâchettes à 0, seize boutons
            // (Relache, 0 appui), cause Geste.
            format!("04 02 00*16 00 00 {} 00 {Q}", "01 00 ".repeat(16)),
        ),
        e(
            "EntreeFil::Presence",
            EntreeFil::Presence(Presence::Arrivee {
                appareil: Appareil::du_fil(7),
                role: RoleAppareil::Manette,
                capacites: Capacites::AUCUNE,
            }),
            // Six Non(NonSupportee), Portrait, pas de prénom.
            format!("05 00 07 01 {} 00 00", "01 00 ".repeat(6)),
        ),
        e(
            "EntreeFil::Presence::Depart",
            EntreeFil::Presence(Presence::Depart {
                appareil: Appareil::du_fil(7),
                cause: CauseDepart::Voulu,
            }),
            "05 01 07 02".to_owned(),
        ),
        e(
            "EntreeFil::Reponse::Fait",
            EntreeFil::Reponse {
                id: 5,
                reponse: Reponse::Fait(Ok(())),
            },
            "06 05 03 00".to_owned(),
        ),
        e(
            "EntreeFil::Reponse::Lu",
            EntreeFil::Reponse {
                id: 1,
                reponse: Reponse::Lu(Err(EchecDemande::TropGros)),
            },
            "06 01 02 01 00".to_owned(),
        ),
        // --- Ce que portent les vues ---
        cas(
            "MontreFil",
            Message::Montre(MontreFil {
                vue: &[1],
                effets: vec![Effet {
                    id: IdEffet::de(&()),
                    quand: Instant {
                        pas: Pas::du_fil(3),
                        fraction: 1,
                    },
                    quoi: Sensation::Vibration(Vibration::Succes),
                    tenue: Tenue::Definitive,
                    peremption: Duree::pas(15),
                }],
                reglages: Reglages::ORDINAIRES,
            }),
            // L'identité de l'effet est FNV-1a 64 de l'encodage vide de `()` : la
            // valeur de départ cbf29ce484222325, en varint.
            "01 01 01 A5 C6 88 A1 C8 9C A7 F9 CB 01 03 01 01 01 01 0F 01 00 00".to_owned(),
        ),
        cas(
            "SceneFil",
            Message::Scene(SceneFil {
                fond: Some(Rgba::NOIR),
                noeuds: vec![
                    NoeudFil::simple(
                        Cle::du_fil(1),
                        None,
                        SorteNoeud::Groupe {
                            decalage: (Fixe::ZERO, Fixe::ZERO),
                        },
                    ),
                    NoeudFil::simple(
                        Cle::du_fil(2),
                        Some(0),
                        SorteNoeud::Rect {
                            r: Rect {
                                x: Fixe::ZERO,
                                y: Fixe::ZERO,
                                l: Fixe::unites(1),
                                h: Fixe::unites(1),
                            },
                            couleur: Rgba::BLANC,
                        },
                    ),
                ],
                raccourcis: vec![],
                clavier_brut: jeu_abi::ClavierBrut::Non,
                effets: vec![],
            }),
            concat!(
                "01 00 00 00 FF 02 ",                         // fond noir, deux nœuds
                "01 00 03 00 00 01 01 01 00 00 FF ",          // clé 1, racine, Groupe (0, 0)
                "02 01 00 00 00 00 80 04 80 04 FF FF FF FF ", // clé 2, parent 0, Rect 1 × 1 blanc
                "01 01 01 00 00 FF ",                         // sans modificateur, opaque
                "00 00 00"                                    // ni raccourci, ni clavier, ni effet
            )
            .to_owned(),
        ),
    ]
}
