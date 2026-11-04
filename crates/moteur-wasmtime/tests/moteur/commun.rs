//! Une colle écrite à la main en `wat`, à la place de `jeu::module!` : la pile dans
//! la première page, la boîte, le brouillon et la zone de panique ensuite, et des
//! réponses à `Bonjour` et `Decrire` encodées ici par `jeu-abi`, puis posées dans les
//! données du module. Tout autre appel passe à `$jouer`, que chaque test écrit.

use std::fmt::Write as _;

use jeu_abi::{ABI, Appel, Bonjour, DeclarationFil, Retour, encoder};
use moteur_wasmtime::{ModuleCharge, Moteur, ReglagesMoteur};
use partie::SorteAppel;

pub const PILE: u32 = 65_536;
pub const BOITE: (u32, u32) = (65_536, 4096);
pub const BROUILLON: (u32, u32) = (69_632, 65_536);
pub const PANIQUE: (u32, u32) = (135_168, 4096);
/// Les réponses encodées, hors des zones.
const REPONSES: u32 = 140_000;

/// Les réglages des tests : un garde-fou court, une profondeur petite.
pub const REGLAGES: ReglagesMoteur = ReglagesMoteur {
    reecriture: reecriture::Reglages {
        profondeur_max: 500,
        memoire_plafond_pages: 64,
    },
    garde_fou_murale_ms: 2000,
};

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

/// La déclaration du modèle de `partie`, avec les limites de cette colle.
pub fn declaration() -> DeclarationFil {
    let mut d = partie::modele::declaration();
    d.limites.brouillon = BROUILLON.1;
    d.limites.memoire = 16;
    d
}

/// Ce qu'un module de test répond, et ce qu'il fait du reste.
pub struct Colle {
    pub bonjour: Bonjour,
    pub declaration: DeclarationFil,
    /// La mémoire : minimum et maximum, en pages.
    pub memoire: (u32, u32),
    /// Le corps de `$jouer (param $n i32) (result i64)`.
    pub jouer: String,
    /// D'autres fonctions, globales ou données.
    pub extra: String,
}

impl Colle {
    pub fn new(jouer: &str) -> Colle {
        Colle {
            bonjour: bonjour(),
            declaration: declaration(),
            memoire: (4, 64),
            jouer: String::from(jouer),
            extra: String::new(),
        }
    }

    pub fn extra(mut self, extra: &str) -> Colle {
        self.extra = String::from(extra);
        self
    }

    pub fn wat(&self) -> String {
        let b = encode(&Retour::Bonjour(self.bonjour));
        let d = encode(&Retour::Decrit(Box::new(self.declaration.clone())));
        let f = encode(&Retour::Fait);
        let ab = REPONSES;
        let ad = REPONSES + 64;
        let af = REPONSES + 4000;
        let (min, max) = self.memoire;
        let (boite, capacite) = BOITE;
        let brouillon = BROUILLON.0;
        format!(
            r#"(module
  (memory (export "memory") {min} {max})
  (global $__stack_pointer (mut i32) (i32.const {PILE}))
  (data (i32.const {ab}) "{eb}")
  (data (i32.const {ad}) "{ed}")
  (data (i32.const {af}) "{ef}")
  (func $repondre (param $ou i32) (param $n i32) (result i64)
    (memory.copy (i32.const {brouillon}) (local.get $ou) (local.get $n))
    (i64.or (i64.shl (i64.const {brouillon}) (i64.const 32)) (i64.extend_i32_u (local.get $n))))
  (func $fait (result i64) (call $repondre (i32.const {af}) (i32.const {lf})))
  (func (export "soiree_boite") (param $n i32) (param $sorte i32) (result i32)
    (if (i32.gt_u (local.get $n) (i32.const {capacite})) (then (return (i32.const 0))))
    (i32.const {boite}))
  (func (export "soiree_appel") (param $n i32) (result i64)
    (block $autre
      (block $decrire
        (block $bonjour
          (br_table $bonjour $decrire $autre (i32.load8_u (i32.const {boite}))))
        (return (call $repondre (i32.const {ab}) (i32.const {lb}))))
      (return (call $repondre (i32.const {ad}) (i32.const {ld}))))
    (call $jouer (local.get $n)))
  (func $jouer (param $n i32) (result i64)
    {jouer})
  {extra})"#,
            eb = echapper(&b),
            ed = echapper(&d),
            ef = echapper(&f),
            lb = b.len(),
            ld = d.len(),
            lf = f.len(),
            jouer = self.jouer,
            extra = self.extra,
        )
    }

    pub fn octets(&self) -> Vec<u8> {
        let texte = self.wat();
        wat::parse_str(&texte).unwrap_or_else(|e| panic!("{e}\n{texte}"))
    }

    pub fn charger(&self) -> ModuleCharge {
        Moteur::new(REGLAGES)
            .unwrap()
            .charger(&self.octets())
            .unwrap_or_else(|e| panic!("module refusé : {e}"))
    }
}

fn encode(v: &Retour<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    assert!(o.len() < 3900, "réponse trop longue pour la colle");
    o
}

fn echapper(octets: &[u8]) -> String {
    let mut s = String::new();
    for o in octets {
        write!(s, "\\{o:02x}").unwrap();
    }
    s
}

/// Un appel d'écriture quelconque, que `$jouer` reçoit.
pub fn entree(ordre: u64) -> Vec<u8> {
    let mut v = Vec::new();
    encoder(
        &Appel::Entree {
            ordre,
            place: ordre,
            entree: &[],
        },
        &mut v,
    )
    .unwrap();
    v
}

/// Un appel de lecture quelconque, que `$jouer` reçoit.
pub fn lecture() -> Vec<u8> {
    let mut v = Vec::new();
    encoder(&Appel::Memoire, &mut v).unwrap();
    v
}

pub const ECRITURE: SorteAppel = SorteAppel::Ecriture;
pub const LECTURE: SorteAppel = SorteAppel::Lecture;

/// Un `$jouer` qui ajoute l'ordre de l'entrée (son deuxième octet) au compteur, puis
/// rend `Fait`.
pub const COMPTER: &str = "
    (i64.store (i32.const 200000)
      (i64.add (i64.load (i32.const 200000)) (i64.load8_u (i32.const 65537))))
    (call $fait)";
