//! Ce qu'un moteur WebAssembly sait faire d'une instance du module réécrit : le reste
//! est le protocole du commun ([`crate::Protocole`]).

use alloc::string::String;

use partie::PanneOutil;

/// La valeur d'une globale, par ses bits : ce qu'un moteur lit et pose, ce que
/// l'instantané garde et ce que l'empreinte lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bits {
    /// Un `i32`.
    I32(i32),
    /// Un `i64`.
    I64(i64),
    /// Les bits d'un `f32`.
    F32(u32),
    /// Les bits d'un `f64`.
    F64(u64),
}

impl Bits {
    /// Une étiquette de type (0 `i32`, 1 `i64`, 2 `f32`, 3 `f64`), puis les bits en
    /// `u64` petit-boutiste, les bits hauts d'un `i32` ou d'un `f32` à zéro : ce que
    /// l'empreinte lit, et ce que la garde écrit.
    pub(crate) fn octets(self) -> ([u8; 1], [u8; 8]) {
        let (t, b) = match self {
            Bits::I32(x) => (0u8, u64::from(x.cast_unsigned())),
            Bits::I64(x) => (1, x.cast_unsigned()),
            Bits::F32(x) => (2, u64::from(x)),
            Bits::F64(x) => (3, x),
        };
        ([t], b.to_le_bytes())
    }

    /// La valeur d'une globale entière, en `i64` ; 0 pour un flottant, que le moteur ne
    /// lit jamais comme un entier.
    pub(crate) fn entier(self) -> i64 {
        match self {
            Bits::I32(x) => i64::from(x),
            Bits::I64(x) => x,
            Bits::F32(_) | Bits::F64(_) => 0,
        }
    }
}

/// Un piège brut, tel que le moteur le constate ; le protocole le classe ensuite par les
/// globales et la zone de panique ([`crate::Protocole`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piege {
    /// Un accès hors de la mémoire.
    HorsMemoire,
    /// La pile native du moteur est épuisée (wasmtime : `StackOverflow` ; les moteurs
    /// de Chrome et de Safari : `RangeError` ; celui de Firefox : `InternalError`).
    PileNative,
    /// Le garde-fou mural a interrompu l'appel.
    Interrompu,
    /// Un autre piège, nommé par le moteur (`unreachable`, division par zéro…).
    Autre(String),
    /// Le moteur a échoué hors du module : ce n'est pas un piège du jeu.
    Moteur(String),
}

/// Les primitives d'un moteur sur une instance du module réécrit, sans rien décider.
///
/// Une adresse et une longueur sont en octets. Lire ou écrire hors de la mémoire est une
/// panne de l'outil : le protocole vérifie les bornes avant de demander. Un appel du
/// protocole, de [`Machine::boite`] à la fin de [`Machine::appel`], est borné en entier
/// par le garde-fou mural du moteur (INTERFACE, § 4.6), qui rend alors
/// [`Piege::Interrompu`] : l'échéance se pose dans `boite`, et `appel` ne la repose pas.
/// Le protocole appelle toujours `boite` avant `appel`.
pub trait Machine: Sized {
    /// Une instance neuve du même module, sur la même machine : rien n'y a été appelé,
    /// ses globales sont celles d'origine. Une mémoire ne rétrécit pas : restaurer un
    /// instantané plus petit en demande une (§ 8.6).
    fn neuve(&mut self) -> Result<Self, PanneOutil>;

    /// La taille de la mémoire, en pages de 64 Kio.
    fn pages(&mut self) -> u64;

    /// Copie `dans.len()` octets de la mémoire depuis `adresse`.
    fn lire(&mut self, adresse: u32, dans: &mut [u8]) -> Result<(), PanneOutil>;

    /// Écrit `octets` dans la mémoire à `adresse`.
    fn ecrire(&mut self, adresse: u32, octets: &[u8]) -> Result<(), PanneOutil>;

    /// Remet à zéro `longueur` octets de la mémoire depuis `adresse`.
    fn remplir_de_zeros(&mut self, adresse: u32, longueur: u32) -> Result<(), PanneOutil>;

    /// Fait grandir la mémoire jusqu'à `pages` pages. Un refus de la machine est une
    /// panne de la machine (`CroissanceRefusee`).
    fn grandir_jusqu_a(&mut self, pages: u64) -> Result<(), PanneOutil>;

    /// La valeur de la globale exportée sous `nom`.
    fn globale(&mut self, nom: &str) -> Result<Bits, PanneOutil>;

    /// Pose la globale exportée sous `nom`.
    fn poser_globale(&mut self, nom: &str, b: Bits) -> Result<(), PanneOutil>;

    /// Appelle `soiree_boite(taille, sorte)` ; rend l'adresse de la boîte, 0 pour un
    /// refus.
    fn boite(&mut self, taille: i32, sorte: i32) -> Result<i32, Piege>;

    /// Appelle `soiree_appel(taille)` ; rend l'adresse et la longueur du retour,
    /// l'adresse dans les 32 bits hauts.
    fn appel(&mut self, taille: i32) -> Result<i64, Piege>;
}
