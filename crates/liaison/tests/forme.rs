//! La forme des trames en clair, figée (PHASE-3, lot 4a) : chaque variant de chaque enum
//! de `liaison` a ses octets écrits ici, et sa seconde lecture, sans serde ni postcard
//! (SAL-90). Changer la forme d'une trame sans monter `PROTOCOLE_RELAIS` fait rougir ce
//! fichier ; un variant ajouté ne compile plus dans les `nom_*` (pas de `_`), et un
//! variant sans octets figés fait rougir `les_octets_figes_couvrent_chaque_variant`.
//!
//! Les octets ont été relevés de l'encodeur, puis chacun relu d'après la forme : la
//! seconde lecture, écrite d'après la forme seule, les relit champ à champ et rebâtit la
//! valeur à la main.
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

use jeu_abi::{ApplicableFil, Duree, ReglesTenuFil};
use liaison::{
    AdresseAnnoncee, AdresseIp, BATTEMENT_MS, Case, CauseFermeture, Classe, CodeSalle, DuRelais,
    LIENS_MAX_SANS_AUTORITE, Lien, PROTOCOLE_RELAIS, REDITE_MAX, REDITE_MIN, RaisonCoupure,
    RefusRelais, Role, SILENCE_MAX, SILENCE_MAX_MS, TRAME_MAX, VersRelais, periode_de_redite,
    protocole_d_un_bonjour,
};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Les noms des variants : un variant ajouté ne compile plus ici
// ---------------------------------------------------------------------------

fn nom_vers_relais(m: &VersRelais) -> &'static str {
    match m {
        VersRelais::Bonjour { .. } => "VersRelais::Bonjour",
        VersRelais::Vers { .. } => "VersRelais::Vers",
        VersRelais::Trame { .. } => "VersRelais::Trame",
        VersRelais::Couper { .. } => "VersRelais::Couper",
        VersRelais::Echo { .. } => "VersRelais::Echo",
        VersRelais::AuRevoir => "VersRelais::AuRevoir",
        VersRelais::Ecrire { .. } => "VersRelais::Ecrire",
        VersRelais::Lister => "VersRelais::Lister",
        VersRelais::Lire { .. } => "VersRelais::Lire",
        VersRelais::Effacer { .. } => "VersRelais::Effacer",
        VersRelais::AjouterLigne { .. } => "VersRelais::AjouterLigne",
        VersRelais::Limites { .. } => "VersRelais::Limites",
    }
}

fn nom_du_relais(m: &DuRelais) -> &'static str {
    match m {
        DuRelais::Accueil { .. } => "DuRelais::Accueil",
        DuRelais::Refus(_) => "DuRelais::Refus",
        DuRelais::Ouvert { .. } => "DuRelais::Ouvert",
        DuRelais::De { .. } => "DuRelais::De",
        DuRelais::Pour { .. } => "DuRelais::Pour",
        DuRelais::Ferme { .. } => "DuRelais::Ferme",
        DuRelais::Encombre { .. } => "DuRelais::Encombre",
        DuRelais::AutoriteAbsente => "DuRelais::AutoriteAbsente",
        DuRelais::AutoritePresente => "DuRelais::AutoritePresente",
        DuRelais::Echo { .. } => "DuRelais::Echo",
        DuRelais::Ecrit { .. } => "DuRelais::Ecrit",
        DuRelais::Cases(_) => "DuRelais::Cases",
        DuRelais::Lu { .. } => "DuRelais::Lu",
    }
}

fn nom_role(r: &Role) -> &'static str {
    match r {
        Role::Autorite { .. } => "Role::Autorite",
        Role::Garde { .. } => "Role::Garde",
        Role::Appareil => "Role::Appareil",
    }
}

fn nom_classe(c: Classe) -> &'static str {
    match c {
        Classe::Fiable => "Classe::Fiable",
        Classe::Remplacable(_) => "Classe::Remplacable",
    }
}

fn nom_case(c: Case) -> &'static str {
    match c {
        Case::Point(_) => "Case::Point",
        Case::Journal(_) => "Case::Journal",
    }
}

fn nom_ip(i: AdresseIp) -> &'static str {
    match i {
        AdresseIp::V4(_) => "AdresseIp::V4",
        AdresseIp::V6(_) => "AdresseIp::V6",
    }
}

fn nom_refus(r: RefusRelais) -> &'static str {
    match r {
        RefusRelais::Protocole { .. } => "RefusRelais::Protocole",
        RefusRelais::SalleInconnue => "RefusRelais::SalleInconnue",
        RefusRelais::PreuveFausse => "RefusRelais::PreuveFausse",
        RefusRelais::HorsBoucleLocale => "RefusRelais::HorsBoucleLocale",
        RefusRelais::Pleine => "RefusRelais::Pleine",
        RefusRelais::TropGros => "RefusRelais::TropGros",
        RefusRelais::GardePleine => "RefusRelais::GardePleine",
        RefusRelais::Remplacee => "RefusRelais::Remplacee",
    }
}

fn nom_raison(r: RaisonCoupure) -> &'static str {
    match r {
        RaisonCoupure::Refuse => "RaisonCoupure::Refuse",
        RaisonCoupure::Remplace => "RaisonCoupure::Remplace",
        RaisonCoupure::Illisible => "RaisonCoupure::Illisible",
        RaisonCoupure::PoigneeInachevee => "RaisonCoupure::PoigneeInachevee",
        RaisonCoupure::Fin => "RaisonCoupure::Fin",
    }
}

fn nom_cause(c: CauseFermeture) -> &'static str {
    match c {
        CauseFermeture::AuRevoir => "CauseFermeture::AuRevoir",
        CauseFermeture::Fermee => "CauseFermeture::Fermee",
        CauseFermeture::Muet => "CauseFermeture::Muet",
        CauseFermeture::FilePleine => "CauseFermeture::FilePleine",
        CauseFermeture::TropGros => "CauseFermeture::TropGros",
        CauseFermeture::Illisible => "CauseFermeture::Illisible",
        CauseFermeture::Coupe => "CauseFermeture::Coupe",
    }
}

/// Chaque variant que les octets figés doivent tenir.
const TOUS_LES_VARIANTS: &[&str] = &[
    "VersRelais::Bonjour",
    "VersRelais::Vers",
    "VersRelais::Trame",
    "VersRelais::Couper",
    "VersRelais::Echo",
    "VersRelais::AuRevoir",
    "VersRelais::Ecrire",
    "VersRelais::Lister",
    "VersRelais::Lire",
    "VersRelais::Effacer",
    "VersRelais::AjouterLigne",
    "VersRelais::Limites",
    "DuRelais::Accueil",
    "DuRelais::Refus",
    "DuRelais::Ouvert",
    "DuRelais::De",
    "DuRelais::Pour",
    "DuRelais::Ferme",
    "DuRelais::Encombre",
    "DuRelais::AutoriteAbsente",
    "DuRelais::AutoritePresente",
    "DuRelais::Echo",
    "DuRelais::Ecrit",
    "DuRelais::Cases",
    "DuRelais::Lu",
    "Role::Autorite",
    "Role::Garde",
    "Role::Appareil",
    "Classe::Fiable",
    "Classe::Remplacable",
    "Case::Point",
    "Case::Journal",
    "AdresseIp::V4",
    "AdresseIp::V6",
    "RefusRelais::Protocole",
    "RefusRelais::SalleInconnue",
    "RefusRelais::PreuveFausse",
    "RefusRelais::HorsBoucleLocale",
    "RefusRelais::Pleine",
    "RefusRelais::TropGros",
    "RefusRelais::GardePleine",
    "RefusRelais::Remplacee",
    "RaisonCoupure::Refuse",
    "RaisonCoupure::Remplace",
    "RaisonCoupure::Illisible",
    "RaisonCoupure::PoigneeInachevee",
    "RaisonCoupure::Fin",
    "CauseFermeture::AuRevoir",
    "CauseFermeture::Fermee",
    "CauseFermeture::Muet",
    "CauseFermeture::FilePleine",
    "CauseFermeture::TropGros",
    "CauseFermeture::Illisible",
    "CauseFermeture::Coupe",
];

fn variants_vers_relais(m: &VersRelais, noms: &mut BTreeSet<&'static str>) {
    noms.insert(nom_vers_relais(m));
    match m {
        VersRelais::Bonjour { role, .. } => {
            noms.insert(nom_role(role));
        }
        VersRelais::Vers { classe, .. } | VersRelais::Trame { classe, .. } => {
            noms.insert(nom_classe(*classe));
        }
        VersRelais::Couper { raison, .. } => {
            noms.insert(nom_raison(*raison));
        }
        VersRelais::Ecrire { case, .. } | VersRelais::Lire { case } => {
            noms.insert(nom_case(*case));
        }
        VersRelais::Echo { .. }
        | VersRelais::AuRevoir
        | VersRelais::Lister
        | VersRelais::Effacer { .. }
        | VersRelais::AjouterLigne { .. }
        | VersRelais::Limites { .. } => {}
    }
}

fn variants_du_relais(m: &DuRelais, noms: &mut BTreeSet<&'static str>) {
    noms.insert(nom_du_relais(m));
    match m {
        DuRelais::Accueil { adresses, .. } => {
            for a in adresses {
                noms.insert(nom_ip(a.ip));
            }
        }
        DuRelais::Refus(r) => {
            noms.insert(nom_refus(*r));
        }
        DuRelais::Ouvert { adresse_jointe, .. } => {
            noms.insert(nom_ip(*adresse_jointe));
        }
        DuRelais::De { classe, .. } | DuRelais::Pour { classe, .. } => {
            noms.insert(nom_classe(*classe));
        }
        DuRelais::Ferme { cause, .. } => {
            noms.insert(nom_cause(*cause));
        }
        DuRelais::Ecrit { case } | DuRelais::Lu { case, .. } => {
            noms.insert(nom_case(*case));
        }
        DuRelais::Cases(cases) => {
            for c in cases {
                noms.insert(nom_case(*c));
            }
        }
        DuRelais::Encombre { .. }
        | DuRelais::AutoriteAbsente
        | DuRelais::AutoritePresente
        | DuRelais::Echo { .. } => {}
    }
}

// ---------------------------------------------------------------------------
// Les valeurs et leurs octets figés
// ---------------------------------------------------------------------------

const SALLE: CodeSalle = CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]);

fn preuve() -> [u8; 32] {
    std::array::from_fn(|i| u8::try_from(i).unwrap())
}

/// Chaque `VersRelais`, et ses octets.
fn vers_relais_figes() -> Vec<(VersRelais<'static>, &'static str)> {
    let couper = |raison| VersRelais::Couper {
        lien: Lien(7),
        raison,
    };
    vec![
        (
            VersRelais::Bonjour {
                protocole: 1,
                salle: SALLE,
                role: Role::Autorite { preuve: preuve() },
            },
            "00013fa9c2d100000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        ),
        (
            VersRelais::Bonjour {
                protocole: 300,
                salle: SALLE,
                role: Role::Garde {
                    preuve: preuve(),
                    autorite: Lien(200),
                },
            },
            "00ac023fa9c2d101000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1fc801",
        ),
        (
            VersRelais::Bonjour {
                protocole: 1,
                salle: SALLE,
                role: Role::Appareil,
            },
            "00013fa9c2d102",
        ),
        (
            VersRelais::Vers {
                lien: Lien(7),
                classe: Classe::Fiable,
                trame: &[0xaa, 0xbb],
            },
            "01070002aabb",
        ),
        (
            VersRelais::Trame {
                classe: Classe::Remplacable(3),
                trame: &[0xcc],
            },
            "02010301cc",
        ),
        (couper(RaisonCoupure::Refuse), "030700"),
        (couper(RaisonCoupure::Remplace), "030701"),
        (couper(RaisonCoupure::Illisible), "030702"),
        (couper(RaisonCoupure::PoigneeInachevee), "030703"),
        (couper(RaisonCoupure::Fin), "030704"),
        (
            VersRelais::Echo {
                t_envoi_us: 1_000_000,
            },
            "04c0843d",
        ),
        (VersRelais::AuRevoir, "05"),
        (
            VersRelais::Ecrire {
                case: Case::Point(2),
                octets: &[1, 2, 3],
            },
            "06000203010203",
        ),
        (VersRelais::Lister, "07"),
        (
            VersRelais::Lire {
                case: Case::Journal(129),
            },
            "08018101",
        ),
        (VersRelais::Effacer { avant: 5 }, "0905"),
        (
            VersRelais::AjouterLigne {
                flux: "scores",
                ligne: b"ok",
            },
            "0a0673636f726573026f6b",
        ),
        (VersRelais::Limites { liens_max: 75 }, "0b4b"),
    ]
}

/// Chaque `DuRelais`, et ses octets.
fn du_relais_figes() -> Vec<(DuRelais<'static>, &'static str)> {
    let refus = DuRelais::Refus;
    let ferme = |cause| DuRelais::Ferme {
        lien: Lien(9),
        cause,
    };
    vec![
        (
            DuRelais::Accueil {
                epoque: [0x11; 16],
                lien: Lien(3),
                adresses: vec![
                    AdresseAnnoncee {
                        ip: AdresseIp::V4([192, 168, 1, 20]),
                        port: 7878,
                    },
                    AdresseAnnoncee {
                        ip: AdresseIp::V6([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]),
                        port: 7878,
                    },
                ],
            },
            "0011111111111111111111111111111111030200c0a80114c63d01fe800000000000000000000000000001c63d",
        ),
        (refus(RefusRelais::Protocole { relais: 1 }), "010001"),
        (refus(RefusRelais::SalleInconnue), "0101"),
        (refus(RefusRelais::PreuveFausse), "0102"),
        (refus(RefusRelais::HorsBoucleLocale), "0103"),
        (refus(RefusRelais::Pleine), "0104"),
        (refus(RefusRelais::TropGros), "0105"),
        (refus(RefusRelais::GardePleine), "0106"),
        (refus(RefusRelais::Remplacee), "0107"),
        (
            DuRelais::Ouvert {
                lien: Lien(9),
                adresse_jointe: AdresseIp::V4([192, 168, 1, 20]),
            },
            "020900c0a80114",
        ),
        (
            DuRelais::De {
                lien: Lien(9),
                classe: Classe::Remplacable(1),
                trame: &[0xde, 0xad],
                recu_us: 128,
                renvoye_us: 129,
            },
            "0309010102dead80018101",
        ),
        (
            DuRelais::Pour {
                classe: Classe::Fiable,
                trame: &[0xbe],
            },
            "040001be",
        ),
        (ferme(CauseFermeture::AuRevoir), "050900"),
        (ferme(CauseFermeture::Fermee), "050901"),
        (ferme(CauseFermeture::Muet), "050902"),
        (ferme(CauseFermeture::FilePleine), "050903"),
        (ferme(CauseFermeture::TropGros), "050904"),
        (ferme(CauseFermeture::Illisible), "050905"),
        (ferme(CauseFermeture::Coupe), "050906"),
        (
            DuRelais::Encombre {
                lien: Lien(9),
                octets_en_attente: 65_536,
            },
            "0609808004",
        ),
        (DuRelais::AutoriteAbsente, "07"),
        (DuRelais::AutoritePresente, "08"),
        (
            DuRelais::Echo {
                t_envoi_us: 5,
                t_relais_us: 6,
            },
            "090506",
        ),
        (
            DuRelais::Ecrit {
                case: Case::Point(0),
            },
            "0a0000",
        ),
        (
            DuRelais::Cases(vec![Case::Point(4), Case::Journal(5), Case::Journal(6)]),
            "0b03000401050106",
        ),
        (
            DuRelais::Lu {
                case: Case::Journal(6),
                octets: &[9, 8],
            },
            "0c0106020908",
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

// ---------------------------------------------------------------------------
// Les octets figés
// ---------------------------------------------------------------------------

#[test]
fn les_octets_des_trames_vers_le_relais_sont_figes() {
    for (m, attendu) in vers_relais_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(VersRelais::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_des_trames_du_relais_sont_figes() {
    for (m, attendu) in du_relais_figes() {
        assert_eq!(hexa(&m.encoder().unwrap()), attendu, "{m:?}");
        let octets = depuis_hexa(attendu);
        assert_eq!(DuRelais::decoder(&octets).unwrap(), m);
    }
}

#[test]
fn les_octets_figes_couvrent_chaque_variant() {
    let mut noms = BTreeSet::new();
    for (m, _) in vers_relais_figes() {
        variants_vers_relais(&m, &mut noms);
    }
    for (m, _) in du_relais_figes() {
        variants_du_relais(&m, &mut noms);
    }
    let tous: BTreeSet<&str> = TOUS_LES_VARIANTS.iter().copied().collect();
    assert_eq!(noms, tous);
}

/// SAL-118 : un octet de trop, ou un de moins, est un refus.
#[test]
fn une_trame_figee_allongee_ou_raccourcie_est_refusee_sal118() {
    for (_, h) in vers_relais_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(VersRelais::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(VersRelais::decoder(&o).is_err(), "{h} - 1");
    }
    for (_, h) in du_relais_figes() {
        let mut o = depuis_hexa(h);
        o.push(0);
        assert!(DuRelais::decoder(&o).is_err(), "{h} + 00");
        o.truncate(o.len() - 2);
        assert!(DuRelais::decoder(&o).is_err(), "{h} - 1");
    }
}

// ---------------------------------------------------------------------------
// La seconde lecture, sans serde ni postcard (SAL-90)
// ---------------------------------------------------------------------------

/// Une lecture de la forme écrite à la main : varints LEB128, octets bruts, longueurs
/// préfixées, variants numérotés dans l'ordre de déclaration.
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

    fn u16(&mut self) -> u16 {
        u16::try_from(self.varint()).unwrap()
    }

    fn u32(&mut self) -> u32 {
        u32::try_from(self.varint()).unwrap()
    }

    fn lien(&mut self) -> Lien {
        Lien(self.u32())
    }

    fn classe(&mut self) -> Classe {
        match self.varint() {
            0 => Classe::Fiable,
            1 => Classe::Remplacable(self.octet()),
            v => panic!("classe {v}"),
        }
    }

    fn case(&mut self) -> Case {
        match self.varint() {
            0 => Case::Point(self.varint()),
            1 => Case::Journal(self.varint()),
            v => panic!("case {v}"),
        }
    }

    fn role(&mut self) -> Role {
        match self.varint() {
            0 => Role::Autorite {
                preuve: self.brut(),
            },
            1 => Role::Garde {
                preuve: self.brut(),
                autorite: self.lien(),
            },
            2 => Role::Appareil,
            v => panic!("rôle {v}"),
        }
    }

    fn ip(&mut self) -> AdresseIp {
        match self.varint() {
            0 => AdresseIp::V4(self.brut()),
            1 => AdresseIp::V6(self.brut()),
            v => panic!("adresse {v}"),
        }
    }

    fn refus(&mut self) -> RefusRelais {
        match self.varint() {
            0 => RefusRelais::Protocole { relais: self.u16() },
            1 => RefusRelais::SalleInconnue,
            2 => RefusRelais::PreuveFausse,
            3 => RefusRelais::HorsBoucleLocale,
            4 => RefusRelais::Pleine,
            5 => RefusRelais::TropGros,
            6 => RefusRelais::GardePleine,
            7 => RefusRelais::Remplacee,
            v => panic!("refus {v}"),
        }
    }

    fn raison(&mut self) -> RaisonCoupure {
        match self.varint() {
            0 => RaisonCoupure::Refuse,
            1 => RaisonCoupure::Remplace,
            2 => RaisonCoupure::Illisible,
            3 => RaisonCoupure::PoigneeInachevee,
            4 => RaisonCoupure::Fin,
            v => panic!("raison {v}"),
        }
    }

    fn cause(&mut self) -> CauseFermeture {
        match self.varint() {
            0 => CauseFermeture::AuRevoir,
            1 => CauseFermeture::Fermee,
            2 => CauseFermeture::Muet,
            3 => CauseFermeture::FilePleine,
            4 => CauseFermeture::TropGros,
            5 => CauseFermeture::Illisible,
            6 => CauseFermeture::Coupe,
            v => panic!("cause {v}"),
        }
    }

    fn vers_relais(&mut self) -> VersRelais<'a> {
        match self.varint() {
            0 => VersRelais::Bonjour {
                protocole: self.u16(),
                salle: CodeSalle(self.brut()),
                role: self.role(),
            },
            1 => VersRelais::Vers {
                lien: self.lien(),
                classe: self.classe(),
                trame: self.tranche(),
            },
            2 => VersRelais::Trame {
                classe: self.classe(),
                trame: self.tranche(),
            },
            3 => VersRelais::Couper {
                lien: self.lien(),
                raison: self.raison(),
            },
            4 => VersRelais::Echo {
                t_envoi_us: self.varint(),
            },
            5 => VersRelais::AuRevoir,
            6 => VersRelais::Ecrire {
                case: self.case(),
                octets: self.tranche(),
            },
            7 => VersRelais::Lister,
            8 => VersRelais::Lire { case: self.case() },
            9 => VersRelais::Effacer {
                avant: self.varint(),
            },
            10 => VersRelais::AjouterLigne {
                flux: std::str::from_utf8(self.tranche()).unwrap(),
                ligne: self.tranche(),
            },
            11 => VersRelais::Limites {
                liens_max: self.u16(),
            },
            v => panic!("VersRelais {v}"),
        }
    }

    fn du_relais(&mut self) -> DuRelais<'a> {
        match self.varint() {
            0 => {
                let epoque = self.brut();
                let lien = self.lien();
                let n = self.varint();
                let adresses = (0..n)
                    .map(|_| AdresseAnnoncee {
                        ip: self.ip(),
                        port: self.u16(),
                    })
                    .collect();
                DuRelais::Accueil {
                    epoque,
                    lien,
                    adresses,
                }
            }
            1 => DuRelais::Refus(self.refus()),
            2 => DuRelais::Ouvert {
                lien: self.lien(),
                adresse_jointe: self.ip(),
            },
            3 => DuRelais::De {
                lien: self.lien(),
                classe: self.classe(),
                trame: self.tranche(),
                recu_us: self.varint(),
                renvoye_us: self.varint(),
            },
            4 => DuRelais::Pour {
                classe: self.classe(),
                trame: self.tranche(),
            },
            5 => DuRelais::Ferme {
                lien: self.lien(),
                cause: self.cause(),
            },
            6 => DuRelais::Encombre {
                lien: self.lien(),
                octets_en_attente: self.u32(),
            },
            7 => DuRelais::AutoriteAbsente,
            8 => DuRelais::AutoritePresente,
            9 => DuRelais::Echo {
                t_envoi_us: self.varint(),
                t_relais_us: self.varint(),
            },
            10 => DuRelais::Ecrit { case: self.case() },
            11 => {
                let n = self.varint();
                DuRelais::Cases((0..n).map(|_| self.case()).collect())
            }
            12 => DuRelais::Lu {
                case: self.case(),
                octets: self.tranche(),
            },
            v => panic!("DuRelais {v}"),
        }
    }
}

#[test]
fn une_seconde_lecture_relit_chaque_trame_sal90() {
    for (m, h) in vers_relais_figes() {
        let octets = depuis_hexa(h);
        let mut l = Lecteur { octets: &octets };
        assert_eq!(l.vers_relais(), m, "{h}");
        assert!(l.octets.is_empty(), "{h} : octets restants");
    }
    for (m, h) in du_relais_figes() {
        let octets = depuis_hexa(h);
        let mut l = Lecteur { octets: &octets };
        assert_eq!(l.du_relais(), m, "{h}");
        assert!(l.octets.is_empty(), "{h} : octets restants");
    }
}

// ---------------------------------------------------------------------------
// Les constantes
// ---------------------------------------------------------------------------

/// Les constantes du lien, figées. `SILENCE_MAX` et `REDITE_MAX` sont aussi celles du
/// monde simulé de l'épreuve, qui les relit (`epreuve/src/telephone.rs`, P27) : 180 et
/// 15 pas, ses valeurs d'avant le lot 4a.
#[test]
fn les_constantes_du_lien_sont_figees() {
    assert_eq!(PROTOCOLE_RELAIS, 1);
    assert_eq!(TRAME_MAX, 262_144);
    assert_eq!(BATTEMENT_MS, 250);
    assert_eq!(SILENCE_MAX_MS, 3_000);
    assert_eq!(SILENCE_MAX, Duree::pas(180));
    assert_eq!(SILENCE_MAX.en_ms(), u64::from(SILENCE_MAX_MS));
    assert_eq!(LIENS_MAX_SANS_AUTORITE, 16);
    assert_eq!(REDITE_MIN, Duree::pas(6));
    assert_eq!(REDITE_MAX, Duree::pas(15));
}

fn regles(fraicheur: Option<Duree>, age_max: Option<Duree>) -> ReglesTenuFil {
    let fil = |d: Option<Duree>| match d {
        Some(d) => ApplicableFil::Oui(d),
        None => ApplicableFil::SansObjet("sans".into()),
    };
    ReglesTenuFil {
        fraicheur: fil(fraicheur),
        age_max: fil(age_max),
        debit_max: 20,
    }
}

/// P27 : le tiers de la plus courte des deux bornes, entre 100 et 250 ms ; rien sans
/// l'une ni l'autre.
#[test]
fn la_periode_de_redite_est_le_tiers_de_la_plus_courte_borne() {
    // Sidequest : une fraîcheur de 450 ms et un âge d'une seconde (SQ-01) : 150 ms.
    let sidequest = regles(Some(Duree::ms(450)), Some(Duree::s(1)));
    assert_eq!(periode_de_redite(&sidequest), Some(Duree::pas(9)));
    assert_eq!(periode_de_redite(&sidequest).unwrap().en_ms(), 150);
    // Punchline : ni fraîcheur ni âge.
    assert_eq!(periode_de_redite(&regles(None, None)), None);
    // Une seule des deux bornes suffit.
    assert_eq!(
        periode_de_redite(&regles(None, Some(Duree::ms(600)))),
        Some(Duree::pas(12))
    );
    assert_eq!(
        periode_de_redite(&regles(Some(Duree::ms(600)), None)),
        Some(Duree::pas(12))
    );
    // Les bornes de la période.
    assert_eq!(
        periode_de_redite(&regles(Some(Duree::ms(30)), None)),
        Some(REDITE_MIN)
    );
    assert_eq!(
        periode_de_redite(&regles(Some(Duree::s(10)), Some(Duree::s(20)))),
        Some(REDITE_MAX)
    );
    // Le tiers s'arrondit au pas inférieur : une redite ne dépasse jamais le tiers.
    assert_eq!(
        periode_de_redite(&regles(Some(Duree::pas(35)), None)),
        Some(Duree::pas(11))
    );
}

// ---------------------------------------------------------------------------
// SAL-40 : le protocole d'un bonjour cassé
// ---------------------------------------------------------------------------

/// SAL-40 : un bonjour d'un autre protocole se reconnaît même quand le reste ne se lit
/// plus ; un bonjour cassé du protocole courant reste du bruit, et rien d'autre qu'un
/// bonjour n'annonce de protocole.
#[test]
fn un_bonjour_d_un_autre_protocole_se_lit_meme_casse_sal40() {
    let autre = VersRelais::Bonjour {
        protocole: 300,
        salle: SALLE,
        role: Role::Appareil,
    }
    .encoder()
    .unwrap();
    // Le protocole, puis la salle coupée : illisible, mais d'un autre protocole.
    let casse = &autre[..4];
    assert!(VersRelais::decoder(casse).is_err());
    assert_eq!(protocole_d_un_bonjour(casse), Some(300));
    // Un bonjour du protocole courant, cassé : il annonce 1, et le relais n'y voit que
    // du bruit.
    let courant = depuis_hexa(vers_relais_figes()[2].1);
    assert_eq!(
        protocole_d_un_bonjour(&courant[..3]),
        Some(PROTOCOLE_RELAIS)
    );
    // Aucune autre trame n'annonce de protocole.
    for (m, h) in vers_relais_figes() {
        let o = depuis_hexa(h);
        let attendu = match m {
            VersRelais::Bonjour { protocole, .. } => Some(protocole),
            VersRelais::Vers { .. }
            | VersRelais::Trame { .. }
            | VersRelais::Couper { .. }
            | VersRelais::Echo { .. }
            | VersRelais::AuRevoir
            | VersRelais::Ecrire { .. }
            | VersRelais::Lister
            | VersRelais::Lire { .. }
            | VersRelais::Effacer { .. }
            | VersRelais::AjouterLigne { .. }
            | VersRelais::Limites { .. } => None,
        };
        assert_eq!(protocole_d_un_bonjour(&o), attendu, "{h}");
    }
    assert_eq!(protocole_d_un_bonjour(&[]), None);
    assert_eq!(protocole_d_un_bonjour(&[0]), None);
    assert_eq!(protocole_d_un_bonjour(&[0, 0x80]), None);
}

// ---------------------------------------------------------------------------
// REM-64 : jamais de panique, et ce qui se lit se réécrit
// ---------------------------------------------------------------------------

fn toutes_les_trames_figees() -> Vec<Vec<u8>> {
    vers_relais_figes()
        .into_iter()
        .map(|(_, h)| depuis_hexa(h))
        .chain(du_relais_figes().into_iter().map(|(_, h)| depuis_hexa(h)))
        .collect()
}

/// REM-64 : ce qui se décode se réencode en octets qui redonnent le même message.
fn relire(octets: &[u8]) -> Result<(), TestCaseError> {
    if let Ok(m) = VersRelais::decoder(octets) {
        let r = m.encoder().unwrap();
        prop_assert_eq!(VersRelais::decoder(&r).unwrap(), m);
    }
    if let Ok(m) = DuRelais::decoder(octets) {
        let r = m.encoder().unwrap();
        prop_assert_eq!(DuRelais::decoder(&r).unwrap(), m);
    }
    let _: Option<u16> = protocole_d_un_bonjour(octets);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// REM-64, SAL-117 : des octets quelconques sont une trame ou un refus, jamais une
    /// panique.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_les_decodeurs_rem64(
        octets in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        relire(&octets)?;
    }

    /// REM-64 : une trame figée abîmée (un octet changé, des octets coupés ou ajoutés)
    /// est une trame ou un refus, jamais une panique.
    #[test]
    fn une_trame_abimee_ne_fait_jamais_paniquer_rem64(
        rang in 0usize..44,
        position in any::<prop::sample::Index>(),
        valeur in any::<u8>(),
        coupe in any::<prop::sample::Index>(),
        ajout in proptest::collection::vec(any::<u8>(), 0..8),
    ) {
        let trames = toutes_les_trames_figees();
        let mut o = trames[rang % trames.len()].clone();
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
