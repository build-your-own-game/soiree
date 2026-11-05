//! Messages et état (§ 2.2) : les traits [`Forme`], [`Etat`] et [`Message`], et leurs
//! implémentations pour les primitives, les collections et le vocabulaire du commun.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::string::String;
use alloc::vec::Vec;

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::secret::Regard;

/// Le type contient-il quelque chose : un secret, ou rien du tout ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Contient {
    /// Oui.
    Oui,
    /// Non.
    Non,
}

impl Contient {
    /// `true` pour [`Contient::Oui`], en `const`.
    pub const fn oui(self) -> bool {
        matches!(self, Contient::Oui)
    }
}

/// La forme d'un type, calculée à la compilation.
pub trait Forme {
    /// FNV-1a des noms, des champs et des formes des champs (§ 2.2).
    const EMPREINTE: u64;
    /// Le type contient-il un `Secret` ?
    const SECRET: Contient;
    /// Le type est-il [`Aucun`] ?
    const VIDE: Contient;
}

/// Ce qui peut vivre dans l'état. Seules `#[jeu::message]` et `#[jeu::etat]`
/// l'implémentent, en plus des types de `jeu`. Aucun type à mutabilité intérieure ne
/// l'implémente. Le trait n'est pas scellé : c'est l'étape 1 de l'épreuve qui refuse
/// un `impl` écrit à la main dans une crate du jeu (règle « forme calculée »).
///
/// Les deux méthodes cachées sont la forme canonique, scellée : leur écrivain et leur
/// liseur ne se construisent que dans `jeu` (D24).
pub trait Etat: Clone + core::fmt::Debug + Forme + 'static {
    /// La même valeur, chaque secret que `r` ne voit pas vidé.
    #[must_use]
    fn projeter(&self, r: &Regard) -> Self;
    #[doc(hidden)]
    fn ecrire(&self, e: &mut Ecrivain<'_>);
    #[doc(hidden)]
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible>;
    /// Une suite de valeurs, élément par élément ; les octets la copient d'un bloc
    /// (TL-22, besoin 15 des jeux). Les octets écrits sont les mêmes.
    #[doc(hidden)]
    fn ecrire_suite(suite: &[Self], e: &mut Ecrivain<'_>) {
        for v in suite {
            v.ecrire(e);
        }
    }
    /// Une suite de `n` valeurs, élément par élément ; les octets la lisent d'un bloc.
    #[doc(hidden)]
    fn lire_suite(n: usize, l: &mut Liseur<'_>) -> Result<Vec<Self>, Illisible> {
        // Réservé d'après ce qui reste, jamais d'après la longueur annoncée (REM-58).
        let mut v = Vec::with_capacity(n.min(l.reste().len()));
        for _ in 0..n {
            v.push(Self::lire(l)?);
        }
        Ok(v)
    }
}

/// Ce qui traverse une frontière : vue, geste, avis, garde, résumé. Seule la macro
/// `#[jeu::message]` l'implémente, en plus des types de `jeu`.
///
/// Un `Secret`, un `Partage` et un flottant nu ne sont pas des messages : une vue, un
/// avis, un résumé ne peuvent pas en contenir.
pub trait Message: Etat + serde::Serialize + serde::de::DeserializeOwned + Forme {}

/// L'encodage d'un message, celui du fil : pour une ligne de résultats ou un fichier à
/// télécharger. Un message ne contient aucun secret.
pub fn octets<M: Message>(m: &M) -> Vec<u8> {
    let mut o = Vec::new();
    if jeu_abi::encoder(m, &mut o).is_err() {
        o.clear();
    }
    o
}

/// Le type sans valeur. `match x {}` suffit.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Aucun {}

impl Forme for Aucun {
    const EMPREINTE: u64 = forme::nommee("Aucun");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Oui;
}

impl Etat for Aucun {
    fn projeter(&self, _: &Regard) -> Self {
        match *self {}
    }
    fn ecrire(&self, _: &mut Ecrivain<'_>) {
        match *self {}
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Err(l.variant_inconnu())
    }
}

impl Message for Aucun {}

/// Un type dont la forme canonique est son encodage de message.
macro_rules! comme_message {
    ($($t:ty => $nom:literal),* $(,)?) => {$(
        impl Forme for $t {
            const EMPREINTE: u64 = forme::nommee($nom);
            const SECRET: Contient = Contient::Non;
            const VIDE: Contient = Contient::Non;
        }
        impl Etat for $t {
            fn projeter(&self, _: &Regard) -> Self {
                self.clone()
            }
            fn ecrire(&self, e: &mut Ecrivain<'_>) {
                e.message(self);
            }
            fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
                l.message()
            }
        }
        impl Message for $t {}
    )*};
}

// Le vocabulaire du commun (`jeu-abi`) : sa forme est figée par `forme_figee` ; son
// nom suffit à l'empreinte. Leur forme canonique est leur encodage, relu strictement,
// donc validé : un `Fini` relu ne peut pas être NaN.
comme_message! {
    jeu_abi::Pas => "Pas",
    jeu_abi::Instant => "Instant",
    jeu_abi::Duree => "Duree",
    jeu_abi::HeureMurale => "HeureMurale",
    jeu_abi::Appareil => "Appareil",
    jeu_abi::Presence => "Presence",
    jeu_abi::RoleAppareil => "RoleAppareil",
    jeu_abi::CauseDepart => "CauseDepart",
    jeu_abi::Capacites => "Capacites",
    jeu_abi::Capacite => "Capacite",
    jeu_abi::Absence => "Absence",
    jeu_abi::Orientation => "Orientation",
    jeu_abi::IdGeste => "IdGeste",
    jeu_abi::IdFichier => "IdFichier",
    jeu_abi::IdDemande => "IdDemande",
    jeu_abi::CauseTenu => "CauseTenu",
    jeu_abi::Touche => "Touche",
    jeu_abi::Mods => "Mods",
    jeu_abi::PhaseTouche => "PhaseTouche",
    jeu_abi::Pointeur => "Pointeur",
    jeu_abi::GenrePointeur => "GenrePointeur",
    jeu_abi::PhasePointeur => "PhasePointeur",
    jeu_abi::FinSaisie => "FinSaisie",
    jeu_abi::Deplace => "Deplace",
    jeu_abi::FichierRecu => "FichierRecu",
    jeu_abi::Cle => "Cle",
    jeu_abi::IdEffet => "IdEffet",
    jeu_abi::Effet => "Effet",
    jeu_abi::Tenue => "Tenue",
    jeu_abi::Sensation => "Sensation",
    jeu_abi::Son => "Son",
    jeu_abi::Signal => "Signal",
    jeu_abi::Vibration => "Vibration",
    jeu_abi::Reglages => "Reglages",
    jeu_abi::Unite => "Unite",
    jeu_abi::Fixe => "Fixe",
    jeu_abi::Rect => "Rect",
    jeu_abi::Rgba => "Rgba",
    jeu_abi::Oui => "Oui",
    jeu_abi::Fini => "Fini",
    jeu_abi::Axe => "Axe",
    jeu_abi::Axe2 => "Axe2",
    jeu_abi::Cumul => "Cumul",
    jeu_abi::Appui => "Appui",
    jeu_abi::Bouton => "Bouton",
    jeu_abi::Manette => "Manette",
    jeu_abi::TexteCourt => "TexteCourt",
    jeu_abi::CodeTouche => "CodeTouche",
    jeu_abi::Flux => "Flux",
    jeu_abi::EchecDemande => "EchecDemande",
    jeu_abi::FormatImage => "FormatImage",
    jeu_abi::Accueil => "Accueil",
    jeu_abi::Prevision => "Prevision",
    jeu_abi::Mecanique => "Mecanique",
    jeu_abi::Places => "Places",
    jeu_abi::StyleTexte => "StyleTexte",
    jeu_abi::Taille => "Taille",
    jeu_abi::Lignes => "Lignes",
    jeu_abi::Coupe => "Coupe",
    jeu_abi::Alignement => "Alignement",
    jeu_abi::Echantillonnage => "Echantillonnage",
    jeu_abi::Animation => "Animation",
    jeu_abi::Visuel => "Visuel",
    jeu_abi::Courbe => "Courbe",
    jeu_abi::ClavierBrut => "ClavierBrut",
    jeu_abi::ZoneVu => "ZoneVu",
}

impl<const MIN: i64, const MAX: i64> Forme for jeu_abi::Borne<MIN, MAX> {
    const EMPREINTE: u64 =
        forme::entier(forme::de("Borne", MIN.cast_unsigned()), MAX.cast_unsigned());
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}

impl<const MIN: i64, const MAX: i64> Etat for jeu_abi::Borne<MIN, MAX> {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.message(self);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        l.message()
    }
}

impl<const MIN: i64, const MAX: i64> Message for jeu_abi::Borne<MIN, MAX> {}

/// Les entiers et les primitives : leur forme canonique est écrite à la main, la même
/// que leur encodage postcard.
macro_rules! entier_non_signe {
    ($($t:ty => $nom:literal, $bits:literal),* $(,)?) => {$(
        impl Forme for $t {
            const EMPREINTE: u64 = forme::nommee($nom);
            const SECRET: Contient = Contient::Non;
            const VIDE: Contient = Contient::Non;
        }
        impl Etat for $t {
            fn projeter(&self, _: &Regard) -> Self {
                *self
            }
            fn ecrire(&self, e: &mut Ecrivain<'_>) {
                e.varint(u128::from(*self));
            }
            fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
                let v = l.varint($bits)?;
                <$t>::try_from(v).map_err(|_| l.erreur(jeu_abi::Illisible::Varint))
            }
        }
        impl Message for $t {}
    )*};
}

macro_rules! entier_signe {
    ($($t:ty => $nom:literal, $bits:literal),* $(,)?) => {$(
        impl Forme for $t {
            const EMPREINTE: u64 = forme::nommee($nom);
            const SECRET: Contient = Contient::Non;
            const VIDE: Contient = Contient::Non;
        }
        impl Etat for $t {
            fn projeter(&self, _: &Regard) -> Self {
                *self
            }
            fn ecrire(&self, e: &mut Ecrivain<'_>) {
                e.signe(i128::from(*self));
            }
            fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
                let v = l.signe($bits)?;
                <$t>::try_from(v).map_err(|_| l.erreur(jeu_abi::Illisible::Varint))
            }
        }
        impl Message for $t {}
    )*};
}

entier_non_signe! {
    u16 => "u16", 16,
    u32 => "u32", 32,
    u64 => "u64", 64,
    u128 => "u128", 128,
}

entier_signe! {
    i16 => "i16", 16,
    i32 => "i32", 32,
    i64 => "i64", 64,
    i128 => "i128", 128,
}

impl Forme for u8 {
    const EMPREINTE: u64 = forme::nommee("u8");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for u8 {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.octets(&[*self]);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        l.octet()
    }
    fn ecrire_suite(suite: &[u8], e: &mut Ecrivain<'_>) {
        e.octets(suite);
    }
    fn lire_suite(n: usize, l: &mut Liseur<'_>) -> Result<Vec<u8>, Illisible> {
        Ok(l.octets(n)?.to_vec())
    }
}
impl Message for u8 {}

impl Forme for i8 {
    const EMPREINTE: u64 = forme::nommee("i8");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for i8 {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.octets(&self.to_le_bytes());
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(l.octet()?.cast_signed())
    }
}
impl Message for i8 {}

// `usize` n'est ni `Etat` ni `Message` : 64 bits dans la copie native, 32 dans le
// module, les mêmes octets y seraient acceptés d'un côté et refusés de l'autre
// (CONTRAT, cadre technique ; loi Déterminisme). Un jeu écrit `u32` ou `u64`.

impl Forme for bool {
    const EMPREINTE: u64 = forme::nommee("bool");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for bool {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.octets(&[u8::from(*self)]);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        l.booleen()
    }
}
impl Message for bool {}

impl Forme for char {
    const EMPREINTE: u64 = forme::nommee("char");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for char {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.varint(u128::from(u32::from(*self)));
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let v = l.varint(32)?;
        u32::try_from(v)
            .ok()
            .and_then(char::from_u32)
            .ok_or(l.erreur(jeu_abi::Illisible::Texte))
    }
}
impl Message for char {}

impl Forme for () {
    const EMPREINTE: u64 = forme::nommee("()");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for () {
    fn projeter(&self, _: &Regard) -> Self {}
    fn ecrire(&self, _: &mut Ecrivain<'_>) {}
    fn lire(_: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(())
    }
}
impl Message for () {}

// Les flottants nus vivent dans l'état, jamais dans un message : une vue n'en a pas
// (§ 3.7), et un geste porte un `Fini` ou un `Axe`, validés au décodage.
impl Forme for f32 {
    const EMPREINTE: u64 = forme::nommee("f32");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for f32 {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.f32(*self);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let b = l.octets(4)?;
        let t: [u8; 4] = b
            .try_into()
            .map_err(|_| l.erreur(jeu_abi::Illisible::Fin))?;
        Ok(f32::from_bits(u32::from_le_bytes(t)))
    }
}

impl Forme for f64 {
    const EMPREINTE: u64 = forme::nommee("f64");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for f64 {
    fn projeter(&self, _: &Regard) -> Self {
        *self
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.f64(*self);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let b = l.octets(8)?;
        let t: [u8; 8] = b
            .try_into()
            .map_err(|_| l.erreur(jeu_abi::Illisible::Fin))?;
        Ok(f64::from_bits(u64::from_le_bytes(t)))
    }
}

impl Forme for String {
    const EMPREINTE: u64 = forme::nommee("String");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for String {
    fn projeter(&self, _: &Regard) -> Self {
        self.clone()
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.texte(self);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(String::from(l.texte()?))
    }
}
impl Message for String {}

/// Le plus d'éléments de taille nulle qu'une suite relue peut annoncer. Un tel élément
/// peut n'occuper aucun octet : sans borne, quelques octets abîmés annonçant 2⁶⁴
/// éléments feraient tourner la lecture sans fin (REM-58, découvert par
/// `octets_abimes_sans_panique`). Aucun état raisonnable n'en tient davantage.
pub const SUITE_DE_VIDES_MAX: usize = 1 << 16;

/// La longueur d'une suite d'éléments de type `T`. Un élément de taille nulle peut
/// n'occuper aucun octet : sa suite peut être plus longue que ce qui reste à lire,
/// jusqu'à [`SUITE_DE_VIDES_MAX`].
fn longueur<T>(l: &mut Liseur<'_>) -> Result<usize, Illisible> {
    if size_of::<T>() > 0 {
        l.longueur()
    } else {
        let n = l.longueur_sans_borne()?;
        if n > SUITE_DE_VIDES_MAX {
            return Err(l.erreur(jeu_abi::Illisible::Limite));
        }
        Ok(n)
    }
}

impl<T: Forme> Forme for Vec<T> {
    const EMPREINTE: u64 = forme::de("Vec", T::EMPREINTE);
    const SECRET: Contient = T::SECRET;
    const VIDE: Contient = Contient::Non;
}
impl<T: Etat> Etat for Vec<T> {
    fn projeter(&self, r: &Regard) -> Self {
        self.iter().map(|v| v.projeter(r)).collect()
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.longueur(self.len());
        T::ecrire_suite(self, e);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let n = longueur::<T>(l)?;
        T::lire_suite(n, l)
    }
}
impl<T: Message> Message for Vec<T> {}

impl<T: Forme> Forme for Box<T> {
    const EMPREINTE: u64 = forme::de("Box", T::EMPREINTE);
    const SECRET: Contient = T::SECRET;
    const VIDE: Contient = T::VIDE;
}
impl<T: Etat> Etat for Box<T> {
    fn projeter(&self, r: &Regard) -> Self {
        Box::new(T::projeter(self, r))
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        T::ecrire(self, e);
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        Ok(Box::new(T::lire(l)?))
    }
}
impl<T: Message> Message for Box<T> {}

impl<T: Forme> Forme for Option<T> {
    const EMPREINTE: u64 = forme::de("Option", T::EMPREINTE);
    const SECRET: Contient = T::SECRET;
    const VIDE: Contient = Contient::Non;
}
impl<T: Etat> Etat for Option<T> {
    fn projeter(&self, r: &Regard) -> Self {
        self.as_ref().map(|v| v.projeter(r))
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        match self {
            None => e.variant(0),
            Some(v) => {
                e.variant(1);
                v.ecrire(e);
            }
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        match l.octet()? {
            0 => Ok(None),
            1 => Ok(Some(T::lire(l)?)),
            _ => Err(l.erreur(jeu_abi::Illisible::Option)),
        }
    }
}
impl<T: Message> Message for Option<T> {}

impl<T: Forme, const N: usize> Forme for [T; N] {
    const EMPREINTE: u64 = forme::entier(forme::de("tableau", T::EMPREINTE), N as u64);
    const SECRET: Contient = T::SECRET;
    const VIDE: Contient = Contient::Non;
}
impl<T: Etat, const N: usize> Etat for [T; N] {
    fn projeter(&self, r: &Regard) -> Self {
        self.each_ref().map(|v| v.projeter(r))
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        for v in self {
            v.ecrire(e);
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let mut v = Vec::with_capacity(N.min(l.reste().len()));
        for _ in 0..N {
            v.push(T::lire(l)?);
        }
        v.try_into()
            .map_err(|_: Vec<T>| l.erreur(jeu_abi::Illisible::Fin))
    }
}
impl<T: Message, const N: usize> Message for [T; N] where
    [T; N]: serde::Serialize + serde::de::DeserializeOwned
{
}

macro_rules! tuple {
    ($($t:ident $i:tt),+) => {
        impl<$($t: Forme),+> Forme for ($($t,)+) {
            const EMPREINTE: u64 = {
                let h = forme::nommee("tuple");
                $( let h = forme::entier(h, $t::EMPREINTE); )+
                h
            };
            const SECRET: Contient = {
                let c = Contient::Non;
                $( let c = forme::ou(c, $t::SECRET); )+
                c
            };
            const VIDE: Contient = Contient::Non;
        }
        impl<$($t: Etat),+> Etat for ($($t,)+) {
            fn projeter(&self, r: &Regard) -> Self {
                ($( self.$i.projeter(r), )+)
            }
            fn ecrire(&self, e: &mut Ecrivain<'_>) {
                $( self.$i.ecrire(e); )+
            }
            fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
                Ok(($( $t::lire(l)?, )+))
            }
        }
        impl<$($t: Message),+> Message for ($($t,)+) {}
    };
}

tuple!(A 0);
tuple!(A 0, B 1);
tuple!(A 0, B 1, C 2);
tuple!(A 0, B 1, C 2, D 3);
tuple!(A 0, B 1, C 2, D 3, E 4);
tuple!(A 0, B 1, C 2, D 3, E 4, F 5);
tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6);
tuple!(A 0, B 1, C 2, D 3, E 4, F 5, G 6, H 7);

impl<K: Forme, V: Forme> Forme for BTreeMap<K, V> {
    const EMPREINTE: u64 = forme::entier(forme::de("BTreeMap", K::EMPREINTE), V::EMPREINTE);
    const SECRET: Contient = forme::ou(K::SECRET, V::SECRET);
    const VIDE: Contient = Contient::Non;
}
impl<K: Message + Ord, V: Etat> Etat for BTreeMap<K, V> {
    fn projeter(&self, r: &Regard) -> Self {
        self.iter()
            .map(|(k, v)| (k.clone(), v.projeter(r)))
            .collect()
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.longueur(self.len());
        for (k, v) in self {
            k.ecrire(e);
            v.ecrire(e);
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let nombre = longueur::<(K, V)>(l)?;
        let mut table = BTreeMap::new();
        for _ in 0..nombre {
            let cle = K::lire(l)?;
            let valeur = V::lire(l)?;
            // Une clé en double serait une forme qu'aucun état n'écrit.
            if table.insert(cle, valeur).is_some() {
                return Err(l.erreur(jeu_abi::Illisible::Valeur));
            }
        }
        Ok(table)
    }
}
impl<K: Message + Ord, V: Message> Message for BTreeMap<K, V> {}

impl<K: Forme> Forme for BTreeSet<K> {
    const EMPREINTE: u64 = forme::de("BTreeSet", K::EMPREINTE);
    const SECRET: Contient = K::SECRET;
    const VIDE: Contient = Contient::Non;
}
impl<K: Message + Ord> Etat for BTreeSet<K> {
    fn projeter(&self, _: &Regard) -> Self {
        self.clone()
    }
    fn ecrire(&self, e: &mut Ecrivain<'_>) {
        e.longueur(self.len());
        for k in self {
            k.ecrire(e);
        }
    }
    fn lire(l: &mut Liseur<'_>) -> Result<Self, Illisible> {
        let n = longueur::<K>(l)?;
        let mut m = BTreeSet::new();
        for _ in 0..n {
            if !m.insert(K::lire(l)?) {
                return Err(l.erreur(jeu_abi::Illisible::Valeur));
            }
        }
        Ok(m)
    }
}
impl<K: Message + Ord> Message for BTreeSet<K> {}
