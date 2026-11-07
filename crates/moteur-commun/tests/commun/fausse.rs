//! Une machine fausse, en pur Rust : une mémoire, des globales nommées, et un module
//! simulé qui répond à `Bonjour` et `Decrire` comme la colle de `jeu`, puis joue ce que
//! chaque test lui fait jouer. Elle ne décide rien : tout ce qui se décide est dans le
//! protocole, que ces tests éprouvent sans moteur WebAssembly.

use std::collections::BTreeMap;
use std::sync::Arc;

use jeu_abi::{ABI, Appel, Bonjour, DeclarationFil, Retour, decoder, encoder};
use moteur_commun::{Bits, Carte, Machine, Piege, Plan, Protocole, verifier_chargement};
use partie::{CausePanne, PanneOutil};
use reecriture::{CARBURANT, Constat, Reecrit};

pub const PILE: u32 = 65_536;
pub const BOITE: (u32, u32) = (65_536, 4096);
pub const BROUILLON: (u32, u32) = (69_632, 8192);
pub const PANIQUE: (u32, u32) = (77_824, 4096);
/// L'état du module simulé : au-delà des zones.
pub const ETAT: u32 = 81_920;
pub const PAGES: u64 = 3;
/// Ce que coûte chaque appel simulé, en carburant.
pub const COUT: i64 = 10;

/// Ce que le module simulé fait d'un appel qui n'est ni `Bonjour` ni `Decrire` : il
/// rend les octets de son retour, ou un piège.
pub type Jouer = fn(&mut Fausse, &[u8]) -> Result<Vec<u8>, Piege>;

pub struct Fausse {
    pub memoire: Vec<u8>,
    pub globales: BTreeMap<String, Bits>,
    pub jouer: Jouer,
    /// Ce que le prochain appel laisse avant de piéger, s'il y en a un.
    pub scenario: Option<Scenario>,
    /// Les appels de [`Machine::grandir_jusqu_a`] reçus.
    pub croissances: u64,
}

/// Un appel qui pose des globales, écrit peut-être un message de panique, puis piège :
/// de quoi éprouver le classement.
#[derive(Debug, Clone)]
pub struct Scenario {
    pub globales: Vec<(&'static str, Bits)>,
    pub panique: Option<&'static str>,
    pub piege: Piege,
}

/// Les noms des globales mutables d'origine du module simulé : la pile, et une autre.
pub fn globales_d_origine() -> Vec<u32> {
    vec![0, 1]
}

impl Fausse {
    pub fn new(jouer: Jouer) -> Fausse {
        let mut globales = BTreeMap::new();
        for nom in [
            reecriture::CARBURANT,
            reecriture::PROFONDEUR,
            reecriture::MEMOIRE_MAX,
            reecriture::CROISSANCE_REFUSEE,
            reecriture::CROISSANCE_BORNEE,
        ] {
            let b = if nom == CARBURANT {
                Bits::I64(0)
            } else {
                Bits::I32(0)
            };
            globales.insert(String::from(nom), b);
        }
        globales.insert(reecriture::nom_de_globale(0), Bits::I32(PILE.cast_signed()));
        globales.insert(reecriture::nom_de_globale(1), Bits::I64(7));
        Fausse {
            memoire: vec![0; usize::try_from(PAGES * 65_536).unwrap()],
            globales,
            jouer,
            scenario: None,
            croissances: 0,
        }
    }

    pub fn entier(&self, nom: &str) -> i64 {
        match self.globales[nom] {
            Bits::I32(x) => i64::from(x),
            Bits::I64(x) => x,
            Bits::F32(_) | Bits::F64(_) => 0,
        }
    }

    pub fn poser(&mut self, nom: &str, b: Bits) {
        self.globales.insert(String::from(nom), b);
    }

    /// Le compteur d'état du module simulé, à [`ETAT`].
    pub fn compteur(&self) -> u64 {
        let a = usize::try_from(ETAT).unwrap();
        u64::from_le_bytes(self.memoire[a..a + 8].try_into().unwrap())
    }

    pub fn ajouter(&mut self, n: u64) {
        let a = usize::try_from(ETAT).unwrap();
        let v = self.compteur().wrapping_add(n);
        self.memoire[a..a + 8].copy_from_slice(&v.to_le_bytes());
    }

    fn plage(&mut self, adresse: u32, n: usize) -> Result<&mut [u8], PanneOutil> {
        let a = usize::try_from(adresse).unwrap();
        self.memoire
            .get_mut(a..a + n)
            .ok_or_else(|| PanneOutil::new(CausePanne::Interne, "hors de la mémoire fausse"))
    }
}

pub fn bonjour() -> Bonjour {
    Bonjour {
        abi: ABI,
        formes: 0x5eed,
        boite: BOITE,
        brouillon: BROUILLON,
        panique: PANIQUE,
        version_jeu: (0, 1, 0),
    }
}

pub fn declaration() -> DeclarationFil {
    let mut d = partie::modele::declaration();
    d.limites.brouillon = BROUILLON.1;
    d.limites.memoire = 8;
    d
}

fn encode(r: &Retour<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(r, &mut o).unwrap();
    o
}

impl Machine for Fausse {
    fn neuve(&mut self) -> Result<Fausse, PanneOutil> {
        Ok(Fausse::new(self.jouer))
    }

    fn pages(&mut self) -> u64 {
        u64::try_from(self.memoire.len()).unwrap() / 65_536
    }

    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil> {
        let n = dans.len();
        dans.copy_from_slice(self.plage(adresse, n)?);
        Ok(())
    }

    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil> {
        self.plage(adresse, octets.len())?.copy_from_slice(octets);
        Ok(())
    }

    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil> {
        self.plage(adresse, usize::try_from(longueur).unwrap())?
            .fill(0);
        Ok(())
    }

    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil> {
        self.croissances += 1;
        self.memoire
            .resize(usize::try_from(pages * 65_536).unwrap(), 0);
        Ok(())
    }

    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil> {
        self.globales
            .get(nom)
            .copied()
            .ok_or_else(|| PanneOutil::new(CausePanne::Interne, format!("globale {nom}")))
    }

    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil> {
        if !self.globales.contains_key(nom) {
            return Err(PanneOutil::new(
                CausePanne::Interne,
                format!("globale {nom}"),
            ));
        }
        self.globales.insert(String::from(nom), b);
        Ok(())
    }

    fn boite(&mut self, taille: i32, _sorte: i32) -> Result<i32, Piege> {
        if taille.cast_unsigned() > BOITE.1 {
            return Ok(0);
        }
        Ok(BOITE.0.cast_signed())
    }

    fn appel(&mut self, taille: i32) -> Result<i64, Piege> {
        let carburant = self.entier(CARBURANT) - COUT;
        self.poser(CARBURANT, Bits::I64(carburant));
        if let Some(s) = self.scenario.take() {
            for (nom, b) in s.globales {
                self.poser(nom, b);
            }
            if let Some(message) = s.panique {
                let p = usize::try_from(PANIQUE.0).unwrap();
                let n = u32::try_from(message.len()).unwrap();
                self.memoire[p..p + 8].copy_from_slice(&jeu_abi::entete_de_panique(n));
                self.memoire[p + 8..p + 8 + message.len()].copy_from_slice(message.as_bytes());
            }
            return Err(s.piege);
        }
        let a = usize::try_from(BOITE.0).unwrap();
        let n = usize::try_from(taille).unwrap();
        let appel = self.memoire[a..a + n].to_vec();
        let retour = match decoder::<Appel<'_>>(&appel) {
            Ok(Appel::Bonjour) => encode(&Retour::Bonjour(bonjour())),
            Ok(Appel::Decrire) => encode(&Retour::Decrit(Box::new(declaration()))),
            _ => (self.jouer)(self, &appel)?,
        };
        let b = usize::try_from(BROUILLON.0).unwrap();
        self.memoire[b..b + retour.len()].copy_from_slice(&retour);
        Ok((i64::from(BROUILLON.0) << 32) | i64::try_from(retour.len()).unwrap())
    }
}

/// Le plan du module simulé.
pub fn plan() -> Plan {
    let reecrit = Reecrit {
        octets: Vec::new(),
        empreinte_originale: [1; 32],
        taille_originale: 0,
        empreinte: [2; 32],
        constat: Constat {
            imports: Vec::new(),
            exports: Vec::new(),
            memoire: (3, Some(8)),
            familles: Vec::new(),
            globales_mutables: 2,
        },
        pile_initiale: PILE,
        pile_globale: Some(0),
    };
    Plan::new(
        &reecrit,
        &reecriture::Reglages {
            profondeur_max: 100,
            memoire_plafond_pages: 64,
        },
        &globales_d_origine(),
    )
}

/// Le module simulé, chargé par le protocole : sa carte.
pub fn carte(jouer: Jouer) -> Arc<Carte> {
    let mut sonde = Fausse::new(jouer);
    let charge = verifier_chargement(&mut sonde, plan()).unwrap();
    assert_eq!(charge.bonjour, bonjour());
    Arc::new(charge.carte)
}

/// Une instance du module simulé.
pub fn instance(jouer: Jouer) -> Protocole<Fausse> {
    Protocole::new(Fausse::new(jouer), carte(jouer)).unwrap()
}

/// Un module qui ajoute 1 à son compteur, et laisse un octet non nul dans chaque zone :
/// la pile, la boîte au-delà de l'appel, le brouillon au-delà du retour, la zone de
/// panique au-delà de son entête.
#[expect(
    clippy::unnecessary_wraps,
    reason = "la forme de `Jouer`, qu'un autre module simulé remplirait d'un piège"
)]
pub fn compter_et_salir(f: &mut Fausse, _appel: &[u8]) -> Result<Vec<u8>, Piege> {
    f.ajouter(1);
    for a in [1000, BOITE.0 + 3000, BROUILLON.0 + 5000, PANIQUE.0 + 3000] {
        f.memoire[usize::try_from(a).unwrap()] = 0xee;
    }
    Ok(encode(&Retour::Fait))
}
