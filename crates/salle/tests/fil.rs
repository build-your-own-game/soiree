//! La forme des messages chiffrés, figée (PHASE-3, lot 4a) : chaque variant de chaque
//! enum de `salle::fil` a ses octets écrits ici, et sa seconde lecture, sans serde ni
//! postcard (SAL-90). Changer la forme d'un message sans monter `fil::PROTOCOLE` fait
//! rougir ce fichier ; un variant ajouté ne compile plus dans les `nom_*` (pas de `_`),
//! et un variant sans octets figés fait rougir `les_octets_figes_couvrent_chaque_variant`.
//!
//! Les octets ont été relevés de l'encodeur, puis chacun relu d'après la forme : la
//! seconde lecture rebâtit la valeur à la main. Les capacités (`jeu_abi::Capacites`)
//! sont relues champ à champ d'après leur forme d'ABI, que `jeu-abi` fige aussi.
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
    reason = "code de test : des longueurs et des numéros petits ; un débordement y serait l'échec du test"
)]

use std::collections::BTreeSet;

use jeu_abi::{Absence, Capacite, Capacites, Instant, Orientation, Pas};
use proptest::prelude::*;
use salle::fil::{
    AttenteCommun, CoupBanc, LimitesSalle, PROTOCOLE, RefusSalle, TraceTelephone, VersAppareil,
    VersAutorite, protocole_d_un_bonjour,
};

// ---------------------------------------------------------------------------
// Les noms des variants : un variant ajouté ne compile plus ici
// ---------------------------------------------------------------------------

fn nom_vers_autorite(m: &VersAutorite) -> &'static str {
    match m {
        VersAutorite::Bonjour { .. } => "VersAutorite::Bonjour",
        VersAutorite::Geste { .. } => "VersAutorite::Geste",
        VersAutorite::Tenu { .. } => "VersAutorite::Tenu",
        VersAutorite::Echo { .. } => "VersAutorite::Echo",
        VersAutorite::ModuleRecu { .. } => "VersAutorite::ModuleRecu",
    }
}

fn nom_vers_appareil(m: &VersAppareil) -> &'static str {
    match m {
        VersAppareil::Accueil { .. } => "VersAppareil::Accueil",
        VersAppareil::Refus(_) => "VersAppareil::Refus",
        VersAppareil::Module { .. } => "VersAppareil::Module",
        VersAppareil::Ressource { .. } => "VersAppareil::Ressource",
        VersAppareil::Vue { .. } => "VersAppareil::Vue",
        VersAppareil::Avis { .. } => "VersAppareil::Avis",
        VersAppareil::Ecarte { .. } => "VersAppareil::Ecarte",
        VersAppareil::Accuse { .. } => "VersAppareil::Accuse",
        VersAppareil::Attente(_) => "VersAppareil::Attente",
        VersAppareil::Echo { .. } => "VersAppareil::Echo",
        VersAppareil::Banc(_) => "VersAppareil::Banc",
    }
}

fn nom_refus(r: RefusSalle) -> &'static str {
    match r {
        RefusSalle::Protocole { .. } => "RefusSalle::Protocole",
        RefusSalle::Pleine => "RefusSalle::Pleine",
        RefusSalle::Fermee => "RefusSalle::Fermee",
        RefusSalle::Remplace => "RefusSalle::Remplace",
    }
}

fn nom_attente(a: AttenteCommun) -> &'static str {
    match a {
        AttenteCommun::Chargement => "AttenteCommun::Chargement",
        AttenteCommun::Panne => "AttenteCommun::Panne",
        AttenteCommun::Reprise => "AttenteCommun::Reprise",
    }
}

fn nom_coup(c: CoupBanc) -> &'static str {
    match c {
        CoupBanc::Geste(_) => "CoupBanc::Geste",
        CoupBanc::Tenu(_) => "CoupBanc::Tenu",
    }
}

const TOUS_LES_VARIANTS: &[&str] = &[
    "VersAutorite::Bonjour",
    "VersAutorite::Geste",
    "VersAutorite::Tenu",
    "VersAutorite::Echo",
    "VersAutorite::ModuleRecu",
    "VersAppareil::Accueil",
    "VersAppareil::Refus",
    "VersAppareil::Module",
    "VersAppareil::Ressource",
    "VersAppareil::Vue",
    "VersAppareil::Avis",
    "VersAppareil::Ecarte",
    "VersAppareil::Accuse",
    "VersAppareil::Attente",
    "VersAppareil::Echo",
    "VersAppareil::Banc",
    "RefusSalle::Protocole",
    "RefusSalle::Pleine",
    "RefusSalle::Fermee",
    "RefusSalle::Remplace",
    "AttenteCommun::Chargement",
    "AttenteCommun::Panne",
    "AttenteCommun::Reprise",
    "CoupBanc::Geste",
    "CoupBanc::Tenu",
];

fn variants(m: &VersAppareil, noms: &mut BTreeSet<&'static str>) {
    noms.insert(nom_vers_appareil(m));
    match m {
        VersAppareil::Refus(r) => {
            noms.insert(nom_refus(*r));
        }
        VersAppareil::Attente(a) => {
            noms.insert(nom_attente(*a));
        }
        VersAppareil::Banc(c) => {
            noms.insert(nom_coup(*c));
        }
        VersAppareil::Accueil { .. }
        | VersAppareil::Module { .. }
        | VersAppareil::Ressource { .. }
        | VersAppareil::Vue { .. }
        | VersAppareil::Avis { .. }
        | VersAppareil::Ecarte { .. }
        | VersAppareil::Accuse { .. }
        | VersAppareil::Echo { .. } => {}
    }
}

// ---------------------------------------------------------------------------
// Les valeurs et leurs octets figés
// ---------------------------------------------------------------------------

fn jeton() -> [u8; 32] {
    std::array::from_fn(|i| u8::try_from(i).unwrap().wrapping_mul(7))
}

/// Des capacités qui ont de chaque forme : présente, absente pour chaque cause.
fn capacites_variees() -> Capacites {
    Capacites {
        vibration: Capacite::Oui,
        mouvement: Capacite::Non(Absence::ContexteNonSecurise),
        son_sans_geste: Capacite::Non(Absence::PasEncore),
        ecran_allume: Capacite::Non(Absence::ContexteNonSecurise),
        micro: Capacite::Non(Absence::Refusee),
        pad_natif: Capacite::Non(Absence::NonSupportee),
        orientation: Orientation::PaysageDroite,
        prenom_propose: None,
    }
}

fn instant(pas: u64, fraction: u16) -> Instant {
    Instant {
        pas: Pas::du_fil(pas),
        fraction,
    }
}

fn vers_autorite_figes() -> Vec<(VersAutorite<'static>, &'static str)> {
    vec![
        (
            VersAutorite::Bonjour {
                protocole: 1,
                jeton: jeton(),
                onglet: 2,
                capacites: Capacites::AUCUNE,
                module_connu: None,
            },
            "000100070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d902010001000100010001000100000000",
        ),
        (
            VersAutorite::Bonjour {
                protocole: 70_000,
                jeton: jeton(),
                onglet: 300,
                capacites: capacites_variees(),
                module_connu: Some([0xee; 32]),
            },
            "00f0a20400070e151c232a31383f464d545b626970777e858c939aa1a8afb6bdc4cbd2d9ac020001010103010101020100030001eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        ),
        (
            VersAutorite::Geste {
                onglet: 2,
                numero: 5,
                octets: &[1, 2],
                fait: instant(600, 32_768),
                vu: Pas::du_fil(599),
                trace: TraceTelephone {
                    evenement_us: 10,
                    envoi_us: 200,
                    incertitude_us: 3_000,
                },
            },
            "010205020102d804808002d7040ac801b817",
        ),
        (
            VersAutorite::Tenu {
                numero: 9,
                octets: &[4],
                fait: instant(7, 1),
            },
            "020901040701",
        ),
        (
            VersAutorite::Echo {
                t_us: 1_000,
                echo_us: 0,
            },
            "03e80700",
        ),
        (
            VersAutorite::ModuleRecu {
                empreinte: [0xab; 32],
                morceau: 3,
            },
            "04abababababababababababababababababababababababababababababababab03",
        ),
    ]
}

fn vers_appareil_figes() -> Vec<(VersAppareil<'static>, &'static str)> {
    vec![
        (
            VersAppareil::Accueil {
                session: 4,
                dernier_geste: 5,
                dernier_tenu: Some(6),
                pas: Pas::du_fil(1_234),
                heure: 99,
                module: Some([0x5a; 32]),
                limites: LimitesSalle {
                    geste: 256,
                    file: 32,
                },
            },
            "0004050106d20963015a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a800220",
        ),
        (
            VersAppareil::Accueil {
                session: 0,
                dernier_geste: 0,
                dernier_tenu: None,
                pas: Pas::ZERO,
                heure: 0,
                module: None,
                limites: LimitesSalle { geste: 1, file: 1 },
            },
            "000000000000000101",
        ),
        (
            VersAppareil::Refus(RefusSalle::Protocole { autorite: 1 }),
            "010001",
        ),
        (VersAppareil::Refus(RefusSalle::Pleine), "0101"),
        (VersAppareil::Refus(RefusSalle::Fermee), "0102"),
        (VersAppareil::Refus(RefusSalle::Remplace), "0103"),
        (
            VersAppareil::Module {
                empreinte: [0x5a; 32],
                morceau: 0,
                total: 6,
                octets: &[0x00, 0x61, 0x73, 0x6d],
            },
            "025a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a0006040061736d",
        ),
        (
            VersAppareil::Ressource {
                cle: 1 << 40,
                octets: &[7],
            },
            "038080808080200107",
        ),
        (
            VersAppareil::Vue {
                pas: Pas::du_fil(60),
                montre: &[1, 2, 3],
            },
            "043c03010203",
        ),
        (VersAppareil::Avis { octets: &[] }, "0500"),
        (VersAppareil::Ecarte { numero: 5 }, "0605"),
        (VersAppareil::Accuse { jusqu_a: 4 }, "0704"),
        (VersAppareil::Attente(AttenteCommun::Chargement), "0800"),
        (VersAppareil::Attente(AttenteCommun::Panne), "0801"),
        (VersAppareil::Attente(AttenteCommun::Reprise), "0802"),
        (
            VersAppareil::Echo {
                t_us: 1_001,
                echo_us: 1_000,
            },
            "09e907e807",
        ),
        (VersAppareil::Banc(CoupBanc::Geste(&[3])), "0a000103"),
        (VersAppareil::Banc(CoupBanc::Tenu(&[4, 5])), "0a01020405"),
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

// ---------------------------------------------------------------------------
// Les octets figés
// ---------------------------------------------------------------------------

#[test]
fn les_octets_des_messages_vers_l_autorite_sont_figes() {
    for (m, attendu) in vers_autorite_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(VersAutorite::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_des_messages_vers_un_appareil_sont_figes() {
    for (m, attendu) in vers_appareil_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(VersAppareil::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_figes_couvrent_chaque_variant() {
    let mut noms = BTreeSet::new();
    for (m, _) in vers_autorite_figes() {
        noms.insert(nom_vers_autorite(&m));
    }
    for (m, _) in vers_appareil_figes() {
        variants(&m, &mut noms);
    }
    let tous: BTreeSet<&str> = TOUS_LES_VARIANTS.iter().copied().collect();
    assert_eq!(noms, tous);
}

#[test]
fn le_protocole_du_fil_est_fige() {
    assert_eq!(PROTOCOLE, 1);
}

/// SAL-118 : un octet de trop, ou un de moins, est un refus.
#[test]
fn un_message_fige_allonge_ou_raccourci_est_refuse_sal118() {
    for (_, h) in vers_autorite_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(VersAutorite::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(VersAutorite::decoder(&o).is_err(), "{h} - 1");
    }
    for (_, h) in vers_appareil_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(VersAppareil::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(VersAppareil::decoder(&o).is_err(), "{h} - 1");
    }
}

// ---------------------------------------------------------------------------
// La seconde lecture, sans serde ni postcard (SAL-90)
// ---------------------------------------------------------------------------

struct Lecteur<'a> {
    octets: &'a [u8],
}

impl<'a> Lecteur<'a> {
    fn octet(&mut self) -> u8 {
        let (b, reste) = self.octets.split_first().expect("fin des octets");
        self.octets = reste;
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

    fn u16(&mut self) -> u16 {
        u16::try_from(self.varint()).unwrap()
    }

    fn brut<const N: usize>(&mut self) -> [u8; N] {
        let (t, reste) = self.octets.split_at(N);
        self.octets = reste;
        t.try_into().unwrap()
    }

    fn tranche(&mut self) -> &'a [u8] {
        let n = usize::try_from(self.varint()).unwrap();
        let (t, reste) = self.octets.split_at(n);
        self.octets = reste;
        t
    }

    fn option<T>(&mut self, lire: impl FnOnce(&mut Self) -> T) -> Option<T> {
        match self.octet() {
            0 => None,
            1 => Some(lire(self)),
            v => panic!("option {v}"),
        }
    }

    fn pas(&mut self) -> Pas {
        Pas::du_fil(self.varint())
    }

    fn instant(&mut self) -> Instant {
        Instant {
            pas: self.pas(),
            fraction: self.u16(),
        }
    }

    /// `jeu_abi::Capacite` : 0 présente, 1 absente, suivie de sa cause.
    fn capacite(&mut self) -> Capacite {
        match self.varint() {
            0 => Capacite::Oui,
            1 => Capacite::Non(match self.varint() {
                0 => Absence::NonSupportee,
                1 => Absence::ContexteNonSecurise,
                2 => Absence::Refusee,
                3 => Absence::PasEncore,
                v => panic!("absence {v}"),
            }),
            v => panic!("capacité {v}"),
        }
    }

    /// `jeu_abi::Capacites`, sans prénom proposé (les valeurs figées n'en ont pas).
    fn capacites(&mut self) -> Capacites {
        let vibration = self.capacite();
        let mouvement = self.capacite();
        let son_sans_geste = self.capacite();
        let ecran_allume = self.capacite();
        let micro = self.capacite();
        let pad_natif = self.capacite();
        let orientation = match self.varint() {
            0 => Orientation::Portrait,
            1 => Orientation::PortraitInverse,
            2 => Orientation::PaysageGauche,
            3 => Orientation::PaysageDroite,
            v => panic!("orientation {v}"),
        };
        assert_eq!(self.octet(), 0, "un prénom proposé");
        Capacites {
            vibration,
            mouvement,
            son_sans_geste,
            ecran_allume,
            micro,
            pad_natif,
            orientation,
            prenom_propose: None,
        }
    }

    fn vers_autorite(&mut self) -> VersAutorite<'a> {
        match self.varint() {
            0 => VersAutorite::Bonjour {
                protocole: self.u32(),
                jeton: self.brut(),
                onglet: self.varint(),
                capacites: self.capacites(),
                module_connu: self.option(Lecteur::brut),
            },
            1 => VersAutorite::Geste {
                onglet: self.varint(),
                numero: self.varint(),
                octets: self.tranche(),
                fait: self.instant(),
                vu: self.pas(),
                trace: TraceTelephone {
                    evenement_us: self.varint(),
                    envoi_us: self.varint(),
                    incertitude_us: self.u32(),
                },
            },
            2 => VersAutorite::Tenu {
                numero: self.varint(),
                octets: self.tranche(),
                fait: self.instant(),
            },
            3 => VersAutorite::Echo {
                t_us: self.varint(),
                echo_us: self.varint(),
            },
            4 => VersAutorite::ModuleRecu {
                empreinte: self.brut(),
                morceau: self.u32(),
            },
            v => panic!("VersAutorite {v}"),
        }
    }

    fn vers_appareil(&mut self) -> VersAppareil<'a> {
        match self.varint() {
            0 => VersAppareil::Accueil {
                session: self.u32(),
                dernier_geste: self.varint(),
                dernier_tenu: self.option(Lecteur::varint),
                pas: self.pas(),
                heure: self.varint(),
                module: self.option(Lecteur::brut),
                limites: LimitesSalle {
                    geste: self.u32(),
                    file: self.u16(),
                },
            },
            1 => VersAppareil::Refus(match self.varint() {
                0 => RefusSalle::Protocole {
                    autorite: self.u32(),
                },
                1 => RefusSalle::Pleine,
                2 => RefusSalle::Fermee,
                3 => RefusSalle::Remplace,
                v => panic!("refus {v}"),
            }),
            2 => VersAppareil::Module {
                empreinte: self.brut(),
                morceau: self.u32(),
                total: self.u32(),
                octets: self.tranche(),
            },
            3 => VersAppareil::Ressource {
                cle: self.varint(),
                octets: self.tranche(),
            },
            4 => VersAppareil::Vue {
                pas: self.pas(),
                montre: self.tranche(),
            },
            5 => VersAppareil::Avis {
                octets: self.tranche(),
            },
            6 => VersAppareil::Ecarte {
                numero: self.varint(),
            },
            7 => VersAppareil::Accuse {
                jusqu_a: self.varint(),
            },
            8 => VersAppareil::Attente(match self.varint() {
                0 => AttenteCommun::Chargement,
                1 => AttenteCommun::Panne,
                2 => AttenteCommun::Reprise,
                v => panic!("attente {v}"),
            }),
            9 => VersAppareil::Echo {
                t_us: self.varint(),
                echo_us: self.varint(),
            },
            10 => VersAppareil::Banc(match self.varint() {
                0 => CoupBanc::Geste(self.tranche()),
                1 => CoupBanc::Tenu(self.tranche()),
                v => panic!("coup {v}"),
            }),
            v => panic!("VersAppareil {v}"),
        }
    }
}

#[test]
fn une_seconde_lecture_relit_chaque_message_sal90() {
    for (m, h) in vers_autorite_figes() {
        let octets = depuis_hexa(h);
        let mut l = Lecteur { octets: &octets };
        assert_eq!(l.vers_autorite(), m, "{h}");
        assert!(l.octets.is_empty(), "{h} : octets restants");
    }
    for (m, h) in vers_appareil_figes() {
        let octets = depuis_hexa(h);
        let mut l = Lecteur { octets: &octets };
        assert_eq!(l.vers_appareil(), m, "{h}");
        assert!(l.octets.is_empty(), "{h} : octets restants");
    }
}

// ---------------------------------------------------------------------------
// SAL-40 : le protocole d'un bonjour cassé
// ---------------------------------------------------------------------------

/// SAL-40 : une page d'un autre protocole se reconnaît même quand son bonjour ne se lit
/// plus ; rien d'autre qu'un bonjour n'annonce de protocole.
#[test]
fn le_bonjour_d_une_page_d_un_autre_protocole_se_lit_meme_casse_sal40() {
    let autre = depuis_hexa(vers_autorite_figes()[1].1);
    let casse = &autre[..10];
    assert!(VersAutorite::decoder(casse).is_err());
    assert_eq!(protocole_d_un_bonjour(casse), Some(70_000));
    for (m, h) in vers_autorite_figes() {
        let attendu = match m {
            VersAutorite::Bonjour { protocole, .. } => Some(protocole),
            VersAutorite::Geste { .. }
            | VersAutorite::Tenu { .. }
            | VersAutorite::Echo { .. }
            | VersAutorite::ModuleRecu { .. } => None,
        };
        assert_eq!(protocole_d_un_bonjour(&depuis_hexa(h)), attendu, "{h}");
    }
    assert_eq!(protocole_d_un_bonjour(&[]), None);
    assert_eq!(protocole_d_un_bonjour(&[0, 0x80, 0x80]), None);
    assert_eq!(
        protocole_d_un_bonjour(&[0, 0xff, 0xff, 0xff, 0xff, 0x10]),
        None,
        "au-delà d'un u32"
    );
}

// ---------------------------------------------------------------------------
// REM-64 : jamais de panique, et ce qui se lit se réécrit
// ---------------------------------------------------------------------------

fn tous_les_messages_figes() -> Vec<Vec<u8>> {
    vers_autorite_figes()
        .into_iter()
        .map(|(_, h)| depuis_hexa(h))
        .chain(
            vers_appareil_figes()
                .into_iter()
                .map(|(_, h)| depuis_hexa(h)),
        )
        .collect()
}

fn relire(octets: &[u8]) -> Result<(), TestCaseError> {
    if let Ok(m) = VersAutorite::decoder(octets) {
        let r = m.encoder().unwrap();
        prop_assert_eq!(VersAutorite::decoder(&r).unwrap(), m);
    }
    if let Ok(m) = VersAppareil::decoder(octets) {
        let r = m.encoder().unwrap();
        prop_assert_eq!(VersAppareil::decoder(&r).unwrap(), m);
    }
    let _: Option<u32> = protocole_d_un_bonjour(octets);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// REM-64, SAL-117 : des octets quelconques sont un message ou un refus, jamais une
    /// panique.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_les_decodeurs_rem64(
        octets in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        relire(&octets)?;
    }

    /// REM-64 : un message figé abîmé est un message ou un refus, jamais une panique.
    #[test]
    fn un_message_abime_ne_fait_jamais_paniquer_rem64(
        rang in 0usize..24,
        position in any::<prop::sample::Index>(),
        valeur in any::<u8>(),
        coupe in any::<prop::sample::Index>(),
        ajout in proptest::collection::vec(any::<u8>(), 0..8),
    ) {
        let messages = tous_les_messages_figes();
        let mut o = messages[rang % messages.len()].clone();
        let i = position.index(o.len());
        o[i] = valeur;
        relire(&o)?;
        let c = coupe.index(o.len() + 1);
        o.truncate(c);
        relire(&o)?;
        o.extend_from_slice(&ajout);
        relire(&o)?;
    }
}
