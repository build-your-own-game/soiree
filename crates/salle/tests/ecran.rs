//! La forme des messages entre la page de l'écran et le Worker, figée (PHASE-3, § 2.7,
//! lot 4b) : chaque variant de chaque enum de `salle::ecran` a ses octets écrits ici, et
//! sa seconde lecture, sans serde ni postcard (SAL-90). Changer la forme d'un message sans
//! monter `ecran::PROTOCOLE` fait rougir ce fichier ; un variant ajouté ne compile plus
//! dans les `nom_*`, et un variant sans octets figés fait rougir
//! `les_octets_figes_couvrent_chaque_variant`.
//!
//! Les octets ont été relevés de l'encodeur, puis chacun relu d'après la forme. Les types
//! de `jeu-abi` qu'ils portent (`DemandeFil`, `EcranFil`, `ReponseFil`) ont leur forme
//! figée par `jeu-abi` ; on n'en relit ici que les variants que portent les valeurs.
//!
//! Et, sous proptest, les décodeurs ne paniquent jamais, et ce qui se décode se réencode
//! en un message égal (REM-64).
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "code de test : des instants et des numéros petits ; un débordement y serait l'échec du test"
)]

use std::collections::BTreeSet;

use jeu_abi::{
    Appareil, CodeTouche, DemandeFil, EchecDemande, EcranFil, FormeEcran, IdDemande, IdFichier,
    IdGeste, Oui, Pas, ReponseFil,
};
use liaison::{AdresseAnnoncee, AdresseIp, CodeSalle, Lien, RefusRelais};
use proptest::prelude::*;
use salle::ecran::{
    AppareilVu, CommandeEcran, DeLaPage, EtatSalle, IdTrace, PROTOCOLE, PanneSalle, RapportPanne,
    Trace, VersPage,
};

fn nom_vers_page(m: &VersPage) -> &'static str {
    match m {
        VersPage::Scene { .. } => "VersPage::Scene",
        VersPage::Ressource { .. } => "VersPage::Ressource",
        VersPage::Demande { .. } => "VersPage::Demande",
        VersPage::EtatSalle(_) => "VersPage::EtatSalle",
        VersPage::Battement { .. } => "VersPage::Battement",
        VersPage::Panne(_) => "VersPage::Panne",
        VersPage::Traces(_) => "VersPage::Traces",
        VersPage::EnCours(_) => "VersPage::EnCours",
        VersPage::Chargement(_) => "VersPage::Chargement",
        VersPage::Jeu { .. } => "VersPage::Jeu",
        VersPage::Accueil { .. } => "VersPage::Accueil",
        VersPage::Cle(_) => "VersPage::Cle",
        VersPage::Banc(_) => "VersPage::Banc",
    }
}

fn nom_de_la_page(m: &DeLaPage) -> &'static str {
    match m {
        DeLaPage::Ecran { .. } => "DeLaPage::Ecran",
        DeLaPage::Image { .. } => "DeLaPage::Image",
        DeLaPage::Fichiers(_) => "DeLaPage::Fichiers",
        DeLaPage::Reponse { .. } => "DeLaPage::Reponse",
        DeLaPage::Commande(_) => "DeLaPage::Commande",
        DeLaPage::Demarrer { .. } => "DeLaPage::Demarrer",
    }
}

fn nom_commande(c: CommandeEcran) -> &'static str {
    match c {
        CommandeEcran::ChangerDeJeu { .. } => "CommandeEcran::ChangerDeJeu",
        CommandeEcran::NouvelleSoiree => "CommandeEcran::NouvelleSoiree",
        CommandeEcran::Reprendre { .. } => "CommandeEcran::Reprendre",
        CommandeEcran::Marque => "CommandeEcran::Marque",
        CommandeEcran::ExporterLaCle => "CommandeEcran::ExporterLaCle",
        CommandeEcran::Figer => "CommandeEcran::Figer",
        CommandeEcran::Retarder { .. } => "CommandeEcran::Retarder",
        CommandeEcran::Bots(_) => "CommandeEcran::Bots",
        CommandeEcran::Calage(_) => "CommandeEcran::Calage",
        CommandeEcran::Echo { .. } => "CommandeEcran::Echo",
    }
}

fn nom_panne(p: PanneSalle) -> &'static str {
    match p {
        PanneSalle::Outil => "PanneSalle::Outil",
        PanneSalle::Garde => "PanneSalle::Garde",
        PanneSalle::Relais(_) => "PanneSalle::Relais",
    }
}

fn nom_trace(t: IdTrace) -> &'static str {
    match t {
        IdTrace::Ecran(_) => "IdTrace::Ecran",
        IdTrace::Appareil { .. } => "IdTrace::Appareil",
    }
}

const TOUS_LES_VARIANTS: &[&str] = &[
    "VersPage::Scene",
    "VersPage::Ressource",
    "VersPage::Demande",
    "VersPage::EtatSalle",
    "VersPage::Battement",
    "VersPage::Panne",
    "VersPage::Traces",
    "VersPage::EnCours",
    "VersPage::Chargement",
    "VersPage::Jeu",
    "VersPage::Accueil",
    "VersPage::Cle",
    "VersPage::Banc",
    "DeLaPage::Ecran",
    "DeLaPage::Image",
    "DeLaPage::Fichiers",
    "DeLaPage::Reponse",
    "DeLaPage::Commande",
    "DeLaPage::Demarrer",
    "CommandeEcran::ChangerDeJeu",
    "CommandeEcran::NouvelleSoiree",
    "CommandeEcran::Reprendre",
    "CommandeEcran::Marque",
    "CommandeEcran::ExporterLaCle",
    "CommandeEcran::Figer",
    "CommandeEcran::Retarder",
    "CommandeEcran::Bots",
    "CommandeEcran::Calage",
    "CommandeEcran::Echo",
    "PanneSalle::Outil",
    "PanneSalle::Garde",
    "PanneSalle::Relais",
    "IdTrace::Ecran",
    "IdTrace::Appareil",
];

fn trace(id: IdTrace) -> Trace {
    Trace {
        id,
        evenement_us: 1,
        envoi_us: 2,
        relais: Some((3, 4)),
        recu_us: 5,
        place: Pas::du_fil(6),
        pas_debut_us: 7,
        pas_fin_us: 8,
        vue_fin_us: 9,
        page_recue_us: 10,
        image_us: 12,
        soumis_us: 11,
        gpu_fini_us: 300,
        incertitude_us: 2_000,
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "une valeur figée par variant, dans une seule liste"
)]
fn vers_page_figes() -> Vec<(VersPage<'static>, &'static str)> {
    vec![
        (
            VersPage::Scene {
                pas: Pas::du_fil(60),
                t_us: 1_000,
                octets: &[1, 2],
            },
            "003ce807020102",
        ),
        (
            VersPage::Ressource {
                cle: 1 << 40,
                octets: &[7],
            },
            "018080808080200107",
        ),
        (
            VersPage::Demande {
                id: IdDemande::du_fil(3),
                quoi: DemandeFil::Lire {
                    fichier: IdFichier::du_fil(4),
                    max: 1_000,
                },
            },
            "02030204e807",
        ),
        (
            VersPage::Demande {
                id: IdDemande::du_fil(1 << 32),
                quoi: DemandeFil::Telecharger {
                    nom: "a.txt",
                    octets: &[9],
                },
            },
            "0280808080100305612e7478740109",
        ),
        (
            VersPage::EtatSalle(EtatSalle {
                salle: CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]),
                lien: Some(Lien(5)),
                appareils: vec![AppareilVu {
                    appareil: Appareil::du_fil(300),
                    present: Oui::Oui,
                    lien: None,
                }],
                garde: Some(12),
            }),
            "033fa9c2d1010501ac020000010c",
        ),
        (
            VersPage::EtatSalle(EtatSalle {
                salle: CodeSalle([0; 4]),
                lien: None,
                appareils: vec![AppareilVu {
                    appareil: Appareil::du_fil(1),
                    present: Oui::Non,
                    lien: Some(Lien(2)),
                }],
                garde: None,
            }),
            "030000000000010101010200",
        ),
        (
            VersPage::Battement {
                pas: Pas::du_fil(61),
                retard_ms: 3,
            },
            "043d03",
        ),
        (
            VersPage::Panne(RapportPanne {
                quoi: PanneSalle::Outil,
                detail: "x",
            }),
            "05000178",
        ),
        (
            VersPage::Panne(RapportPanne {
                quoi: PanneSalle::Garde,
                detail: "",
            }),
            "050100",
        ),
        (
            VersPage::Panne(RapportPanne {
                quoi: PanneSalle::Relais(RefusRelais::Remplacee),
                detail: "é",
            }),
            "05020702c3a9",
        ),
        (
            VersPage::Traces(vec![
                trace(IdTrace::Ecran(1)),
                trace(IdTrace::Appareil {
                    appareil: Appareil::du_fil(300),
                    numero: 2,
                }),
            ]),
            "06020001010201030405060708090a0c0bac02d00f01ac0202010201030405060708090a0c0bac02d00f",
        ),
        (VersPage::EnCours(None), "0700"),
        (VersPage::EnCours(Some(IdGeste::du_fil(77))), "07014d"),
        (VersPage::Chargement(Oui::Oui), "0800"),
        (VersPage::Chargement(Oui::Non), "0801"),
        (
            VersPage::Jeu {
                module: [0xcd; 32],
                forme: FormeEcran::Scene {
                    largeur: 1920,
                    hauteur: 1080,
                    zone_sure_pour_cent: 90,
                    echelle_entiere: Oui::Non,
                },
                diagnostic: CodeTouche::new("KeyD"),
            },
            "09cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd00800fb8085a01044b657944",
        ),
        (
            VersPage::Jeu {
                module: [0; 32],
                forme: FormeEcran::Page { largeur_max: 1100 },
                diagnostic: CodeTouche::new("F2"),
            },
            "09000000000000000000000000000000000000000000000000000000000000000001cc08024632",
        ),
        (
            VersPage::Accueil {
                salle: CodeSalle([1, 2, 3, 4]),
                fragment: "ab.c",
                adresses: vec![
                    AdresseAnnoncee {
                        ip: AdresseIp::V4([192, 168, 1, 20]),
                        port: 7878,
                    },
                    AdresseAnnoncee {
                        ip: AdresseIp::V6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
                        port: 7878,
                    },
                ],
            },
            "0a010203040461622e630200c0a80114c63d0100000000000000000000000000000001c63d",
        ),
        (
            VersPage::Cle([0xee; 32]),
            "0beeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        ),
        (VersPage::Banc("{}"), "0c027b7d"),
    ]
}

fn de_la_page_figes() -> Vec<(DeLaPage<'static>, &'static str)> {
    vec![
        (
            DeLaPage::Ecran {
                quoi: EcranFil::Action(&[5]),
                vu: Pas::du_fil(9),
                t_us: 100,
                numero: 3,
                envoi_us: 200,
            },
            "00000105096403c801",
        ),
        (
            DeLaPage::Image {
                t_us: 16_667,
                periode_us: 16_667,
            },
            "019b82019b8201",
        ),
        (DeLaPage::Fichiers(Vec::new()), "0200"),
        (
            DeLaPage::Reponse {
                id: IdDemande::du_fil(3),
                quoi: ReponseFil::Lu(Ok(&[1])),
            },
            "030302000101",
        ),
        (
            DeLaPage::Reponse {
                id: IdDemande::du_fil(4),
                quoi: ReponseFil::Fait(Err(EchecDemande::Refuse)),
            },
            "0304030102",
        ),
        (
            DeLaPage::Commande(CommandeEcran::ChangerDeJeu { module: [0xab; 32] }),
            "0400abababababababababababababababababababababababababababababababab",
        ),
        (DeLaPage::Commande(CommandeEcran::NouvelleSoiree), "0401"),
        (
            DeLaPage::Commande(CommandeEcran::Reprendre {
                ecarter: Some(IdGeste::du_fil(5)),
            }),
            "04020105",
        ),
        (
            DeLaPage::Commande(CommandeEcran::Reprendre { ecarter: None }),
            "040200",
        ),
        (DeLaPage::Commande(CommandeEcran::Marque), "0403"),
        (DeLaPage::Commande(CommandeEcran::ExporterLaCle), "0404"),
        (DeLaPage::Commande(CommandeEcran::Figer), "0405"),
        (
            DeLaPage::Commande(CommandeEcran::Retarder { ms: 150, fois: 3 }),
            "0406960103",
        ),
        (DeLaPage::Commande(CommandeEcran::Bots(Oui::Non)), "040701"),
        (
            DeLaPage::Commande(CommandeEcran::Calage(Oui::Non)),
            "040801",
        ),
        (
            DeLaPage::Commande(CommandeEcran::Echo { t_us: 300 }),
            "0409ac02",
        ),
        (
            DeLaPage::Demarrer {
                banc: Oui::Oui,
                module: Some([0x11; 32]),
            },
            "0500011111111111111111111111111111111111111111111111111111111111111111",
        ),
        (
            DeLaPage::Demarrer {
                banc: Oui::Non,
                module: None,
            },
            "050100",
        ),
    ]
}

fn hexa(o: &[u8]) -> String {
    use std::fmt::Write;
    o.iter().fold(String::new(), |mut s, b| {
        let _: std::fmt::Result = write!(s, "{b:02x}");
        s
    })
}

fn depuis_hexa(t: &str) -> Vec<u8> {
    t.as_bytes()
        .chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn les_octets_des_messages_vers_la_page_sont_figes() {
    for (m, attendu) in vers_page_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(VersPage::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_des_messages_de_la_page_sont_figes() {
    for (m, attendu) in de_la_page_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(DeLaPage::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_figes_couvrent_chaque_variant() {
    let mut noms = BTreeSet::new();
    for (m, _) in vers_page_figes() {
        noms.insert(nom_vers_page(&m));
        match &m {
            VersPage::Panne(r) => {
                noms.insert(nom_panne(r.quoi));
            }
            VersPage::Traces(v) => {
                for t in v {
                    noms.insert(nom_trace(t.id));
                }
            }
            VersPage::Scene { .. }
            | VersPage::Ressource { .. }
            | VersPage::Demande { .. }
            | VersPage::EtatSalle(_)
            | VersPage::Battement { .. }
            | VersPage::EnCours(_)
            | VersPage::Chargement(_)
            | VersPage::Jeu { .. }
            | VersPage::Accueil { .. }
            | VersPage::Cle(_)
            | VersPage::Banc(_) => {}
        }
    }
    for (m, _) in de_la_page_figes() {
        noms.insert(nom_de_la_page(&m));
        if let DeLaPage::Commande(c) = m {
            noms.insert(nom_commande(c));
        }
    }
    let tous: BTreeSet<&str> = TOUS_LES_VARIANTS.iter().copied().collect();
    assert_eq!(noms, tous);
}

#[test]
fn le_protocole_de_l_ecran_est_fige() {
    assert_eq!(PROTOCOLE, 4);
}

/// SAL-118 : un octet de trop, ou un de moins, est un refus.
#[test]
fn un_message_fige_allonge_ou_raccourci_est_refuse_sal118() {
    for (_, h) in vers_page_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(VersPage::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(VersPage::decoder(&o).is_err(), "{h} - 1");
    }
    for (_, h) in de_la_page_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(DeLaPage::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(DeLaPage::decoder(&o).is_err(), "{h} - 1");
    }
}

// ---------------------------------------------------------------------------
// La seconde lecture, sans serde ni postcard (SAL-90)
// ---------------------------------------------------------------------------

struct Lecteur<'a>(&'a [u8]);

impl<'a> Lecteur<'a> {
    fn octet(&mut self) -> u8 {
        let (b, reste) = self.0.split_first().expect("fin des octets");
        self.0 = reste;
        *b
    }

    fn varint(&mut self) -> u64 {
        let mut v = 0u64;
        for decalage in (0..70).step_by(7) {
            let b = self.octet();
            v |= u64::from(b & 0x7f) << decalage;
            if b & 0x80 == 0 {
                return v;
            }
        }
        panic!("varint trop long");
    }

    fn u32(&mut self) -> u32 {
        u32::try_from(self.varint()).unwrap()
    }

    fn brut<const N: usize>(&mut self) -> [u8; N] {
        let (t, reste) = self.0.split_at(N);
        self.0 = reste;
        t.try_into().unwrap()
    }

    fn octets(&mut self) -> &'a [u8] {
        let n = usize::try_from(self.varint()).unwrap();
        let (t, reste) = self.0.split_at(n);
        self.0 = reste;
        t
    }

    fn texte(&mut self) -> &'a str {
        std::str::from_utf8(self.octets()).unwrap()
    }

    fn option<T>(&mut self, f: impl FnOnce(&mut Self) -> T) -> Option<T> {
        match self.octet() {
            0 => None,
            1 => Some(f(self)),
            v => panic!("option {v}"),
        }
    }

    fn oui(&mut self) -> Oui {
        match self.octet() {
            0 => Oui::Oui,
            1 => Oui::Non,
            v => panic!("Oui {v}"),
        }
    }

    fn pas(&mut self) -> Pas {
        Pas::du_fil(self.varint())
    }

    fn demande(&mut self) -> DemandeFil<'a> {
        match self.octet() {
            2 => DemandeFil::Lire {
                fichier: IdFichier::du_fil(self.varint()),
                max: self.varint(),
            },
            3 => DemandeFil::Telecharger {
                nom: self.texte(),
                octets: self.octets(),
            },
            v => panic!("DemandeFil {v} : pas dans les valeurs figées"),
        }
    }

    fn reponse(&mut self) -> ReponseFil<'a> {
        match self.octet() {
            2 => match self.octet() {
                0 => ReponseFil::Lu(Ok(self.octets())),
                v => panic!("Lu {v}"),
            },
            3 => match self.octet() {
                1 => ReponseFil::Fait(Err(match self.octet() {
                    2 => EchecDemande::Refuse,
                    v => panic!("EchecDemande {v}"),
                })),
                v => panic!("Fait {v}"),
            },
            v => panic!("ReponseFil {v} : pas dans les valeurs figées"),
        }
    }

    fn trace(&mut self) -> Trace {
        let id = match self.octet() {
            0 => IdTrace::Ecran(self.varint()),
            1 => IdTrace::Appareil {
                appareil: Appareil::du_fil(self.varint()),
                numero: self.varint(),
            },
            v => panic!("IdTrace {v}"),
        };
        Trace {
            id,
            evenement_us: self.varint(),
            envoi_us: self.varint(),
            relais: self.option(|l| (l.varint(), l.varint())),
            recu_us: self.varint(),
            place: self.pas(),
            pas_debut_us: self.varint(),
            pas_fin_us: self.varint(),
            vue_fin_us: self.varint(),
            page_recue_us: self.varint(),
            image_us: self.varint(),
            soumis_us: self.varint(),
            gpu_fini_us: self.varint(),
            incertitude_us: self.u32(),
        }
    }

    fn vers_page(&mut self) -> VersPage<'a> {
        match self.octet() {
            0 => VersPage::Scene {
                pas: self.pas(),
                t_us: self.varint(),
                octets: self.octets(),
            },
            1 => VersPage::Ressource {
                cle: self.varint(),
                octets: self.octets(),
            },
            2 => VersPage::Demande {
                id: IdDemande::du_fil(self.varint()),
                quoi: self.demande(),
            },
            3 => {
                let salle = CodeSalle(self.brut::<4>());
                let lien = self.option(|l| Lien(l.u32()));
                let n = self.varint();
                let appareils = (0..n)
                    .map(|_| AppareilVu {
                        appareil: Appareil::du_fil(self.varint()),
                        present: self.oui(),
                        lien: self.option(|l| Lien(l.u32())),
                    })
                    .collect();
                let garde = self.option(Lecteur::varint);
                VersPage::EtatSalle(EtatSalle {
                    salle,
                    lien,
                    appareils,
                    garde,
                })
            }
            4 => VersPage::Battement {
                pas: self.pas(),
                retard_ms: self.u32(),
            },
            5 => {
                let quoi = match self.octet() {
                    0 => PanneSalle::Outil,
                    1 => PanneSalle::Garde,
                    2 => PanneSalle::Relais(match self.octet() {
                        7 => RefusRelais::Remplacee,
                        v => panic!("RefusRelais {v} : pas dans les valeurs figées"),
                    }),
                    v => panic!("PanneSalle {v}"),
                };
                VersPage::Panne(RapportPanne {
                    quoi,
                    detail: self.texte(),
                })
            }
            6 => {
                let n = self.varint();
                VersPage::Traces((0..n).map(|_| self.trace()).collect())
            }
            7 => VersPage::EnCours(self.option(|l| IdGeste::du_fil(l.varint()))),
            8 => VersPage::Chargement(self.oui()),
            9 => VersPage::Jeu {
                module: self.brut::<32>(),
                forme: match self.octet() {
                    0 => FormeEcran::Scene {
                        largeur: u16::try_from(self.varint()).unwrap(),
                        hauteur: u16::try_from(self.varint()).unwrap(),
                        zone_sure_pour_cent: self.octet(),
                        echelle_entiere: self.oui(),
                    },
                    1 => FormeEcran::Page {
                        largeur_max: u16::try_from(self.varint()).unwrap(),
                    },
                    v => panic!("FormeEcran {v}"),
                },
                diagnostic: CodeTouche::essayer(self.texte()).unwrap(),
            },
            10 => {
                let salle = CodeSalle(self.brut::<4>());
                let fragment = self.texte();
                let n = self.varint();
                let adresses = (0..n)
                    .map(|_| AdresseAnnoncee {
                        ip: match self.octet() {
                            0 => AdresseIp::V4(self.brut::<4>()),
                            1 => AdresseIp::V6(self.brut::<16>()),
                            v => panic!("AdresseIp {v}"),
                        },
                        port: u16::try_from(self.varint()).unwrap(),
                    })
                    .collect();
                VersPage::Accueil {
                    salle,
                    fragment,
                    adresses,
                }
            }
            11 => VersPage::Cle(self.brut::<32>()),
            12 => VersPage::Banc(self.texte()),
            v => panic!("VersPage {v}"),
        }
    }

    fn de_la_page(&mut self) -> DeLaPage<'a> {
        match self.octet() {
            0 => {
                let quoi = match self.octet() {
                    0 => EcranFil::Action(self.octets()),
                    v => panic!("EcranFil {v} : pas dans les valeurs figées"),
                };
                DeLaPage::Ecran {
                    quoi,
                    vu: self.pas(),
                    t_us: self.varint(),
                    numero: self.varint(),
                    envoi_us: self.varint(),
                }
            }
            1 => DeLaPage::Image {
                t_us: self.varint(),
                periode_us: self.u32(),
            },
            2 => {
                assert_eq!(self.varint(), 0, "pas de fichier dans les valeurs figées");
                DeLaPage::Fichiers(Vec::new())
            }
            3 => DeLaPage::Reponse {
                id: IdDemande::du_fil(self.varint()),
                quoi: self.reponse(),
            },
            4 => DeLaPage::Commande(match self.octet() {
                0 => CommandeEcran::ChangerDeJeu {
                    module: self.brut::<32>(),
                },
                1 => CommandeEcran::NouvelleSoiree,
                2 => CommandeEcran::Reprendre {
                    ecarter: self.option(|l| IdGeste::du_fil(l.varint())),
                },
                3 => CommandeEcran::Marque,
                4 => CommandeEcran::ExporterLaCle,
                5 => CommandeEcran::Figer,
                6 => CommandeEcran::Retarder {
                    ms: self.u32(),
                    fois: self.u32(),
                },
                7 => CommandeEcran::Bots(self.oui()),
                8 => CommandeEcran::Calage(self.oui()),
                9 => CommandeEcran::Echo {
                    t_us: self.varint(),
                },
                v => panic!("CommandeEcran {v}"),
            }),
            5 => DeLaPage::Demarrer {
                banc: self.oui(),
                module: self.option(Lecteur::brut::<32>),
            },
            v => panic!("DeLaPage {v}"),
        }
    }
}

#[test]
fn une_seconde_lecture_relit_chaque_message_sal90() {
    for (m, h) in vers_page_figes() {
        let o = depuis_hexa(h);
        let mut l = Lecteur(&o);
        assert_eq!(l.vers_page(), m, "{h}");
        assert!(l.0.is_empty(), "{h}");
    }
    for (m, h) in de_la_page_figes() {
        let o = depuis_hexa(h);
        let mut l = Lecteur(&o);
        assert_eq!(l.de_la_page(), m, "{h}");
        assert!(l.0.is_empty(), "{h}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// REM-64 : des octets au hasard ne font jamais paniquer les décodeurs ; ce qui se
    /// décode se réencode en un message égal.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_les_decodeurs_rem64(
        o in proptest::collection::vec(any::<u8>(), 0..160),
    ) {
        if let Ok(m) = VersPage::decoder(&o) {
            let e = m.encoder().unwrap();
            prop_assert_eq!(VersPage::decoder(&e).unwrap(), m);
        }
        if let Ok(m) = DeLaPage::decoder(&o) {
            let e = m.encoder().unwrap();
            prop_assert_eq!(DeLaPage::decoder(&e).unwrap(), m);
        }
    }

    /// REM-64 : un message figé abîmé à un octet ne fait jamais paniquer.
    #[test]
    fn un_message_abime_ne_fait_jamais_paniquer_rem64(
        k in 0usize..40,
        position in any::<usize>(),
        octet in any::<u8>(),
    ) {
        let tous: Vec<&str> = vers_page_figes()
            .into_iter()
            .map(|(_, h)| h)
            .chain(de_la_page_figes().into_iter().map(|(_, h)| h))
            .collect();
        let mut o = depuis_hexa(tous[k % tous.len()]);
        let i = position % o.len();
        o[i] = octet;
        let _: Result<VersPage, _> = VersPage::decoder(&o);
        let _: Result<DeLaPage, _> = DeLaPage::decoder(&o);
    }
}

/// Lot 11 : les ressources qu'une scène cite, que l'autorité envoie à la page avant elle
/// quand la page ne les a pas (une image reçue en partie, TL-14, n'est pas un asset
/// déclaré) : l'image de chaque nœud `Image`, la ressource de chaque `Lien`, une fois.
#[test]
fn les_images_citees_par_une_scene() {
    use jeu_abi::{
        Cle, ComposantVuFil, Echantillonnage, Fixe, NoeudFil, Rect, Rgba, SceneFil, SorteNoeud,
        TexteCourt,
    };
    let r = Rect {
        x: Fixe::ZERO,
        y: Fixe::ZERO,
        l: Fixe::unites(10),
        h: Fixe::unites(10),
    };
    let mut s = SceneFil::vide();
    for (c, image) in [(1, 70), (2, 71), (3, 70)] {
        s.noeuds.push(NoeudFil::simple(
            Cle::du_fil(c),
            None,
            SorteNoeud::Image {
                r,
                image: Cle::du_fil(image),
                ech: Echantillonnage::Lisse,
            },
        ));
    }
    s.noeuds.push(NoeudFil::simple(
        Cle::du_fil(4),
        None,
        SorteNoeud::Rect {
            r,
            couleur: Rgba::BLANC,
        },
    ));
    s.noeuds.push(NoeudFil::simple(
        Cle::du_fil(5),
        None,
        SorteNoeud::Composant {
            boite: Some(r),
            composant: ComposantVuFil::Lien {
                nom: TexteCourt::new("Download"),
                ressource: Cle::du_fil(72),
                fichier: TexteCourt::new("tierlist.png"),
            },
        },
    ));
    assert_eq!(
        salle::ecran::images_citees(&s),
        BTreeSet::from([70u64, 71, 72])
    );
}
