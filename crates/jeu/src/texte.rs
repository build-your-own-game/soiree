//! Les textes venus d'un appareil (§ 2.3, D22) : [`Texte<L>`] porte sa limite dans
//! son type, avec une seule définition pour les règles et pour le champ qui la saisit
//! (TL-26, TL-27, SQ-40).
//!
//! Le nettoyage est le seul : NFC ; contrôles, ignorables par défaut et catégorie Cf
//! retirés, sauf les marques préfixées de Cf, qui se dessinent (PUN-23) ; liants et
//! sélecteurs gardés seulement dans un emoji (CAT-85) ; mots séparés d'une seule espace
//! (PUN-26) ; coupé entre deux graphèmes, au plus `L::MAX` unités de `L::UNITE`
//! (PUN-24, TL-28, CAT-83). Nettoyer deux fois revient à nettoyer une fois (CAT-80).
//!
//! Le nettoyage embarque `unicode-normalization` et `unicode-segmentation` dans le
//! module : une version du module a une seule version d'Unicode, et un rejeu ne change
//! pas. Les tables d'invisibles et de pictogrammes ci-dessous sont écrites à la main,
//! d'après la catégorie Cf et la propriété `Default_Ignorable_Code_Point` d'Unicode 15 ;
//! elles sont plus larges que l'exact (un bloc d'emoji entier compte comme pictogramme).

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;

use jeu_abi::{Oui, Unite};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use unicode_segmentation::UnicodeSegmentation;

use crate::cache::{Ecrivain, Illisible, Liseur, forme};
use crate::forme::{Contient, Etat, Forme, Message};
use crate::secret::Regard;

/// Ce qu'une police dessine. `Toute` tant qu'aucune table n'est fournie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encre {
    /// Tout caractère est dessiné.
    Toute,
    /// Seuls les points de code de ces intervalles fermés sont dessinés.
    Table(&'static [(u32, u32)]),
}

/// Une limite de texte : une seule définition, pour les règles et pour le champ qui la
/// saisit (TL-26, TL-27, SQ-40). S'écrit avec [`limite!`](crate::limite).
pub trait Limite: 'static {
    /// Le plus d'unités.
    const MAX: u32;
    /// L'unité dans laquelle la limite se compte.
    const UNITE: Unite;
    /// Ce que la police dessine.
    const ENCRE: Encre;
}

/// Une limite de texte, nommée.
///
/// ```
/// jeu::limite!(pub Prenom = 12 Capitales);
/// jeu::limite!(pub Phrase = 80 PointsDeCode);
/// ```
#[macro_export]
macro_rules! limite {
    ($vis:vis $nom:ident = $max:literal $unite:ident) => {
        $crate::limite!($vis $nom = $max $unite, encre $crate::Encre::Toute);
    };
    ($vis:vis $nom:ident = $max:literal $unite:ident, encre $encre:expr) => {
        #[doc = concat!("Une limite de texte : ", stringify!($max), " ", stringify!($unite), ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        $vis enum $nom {}
        impl $crate::Limite for $nom {
            const MAX: u32 = $max;
            const UNITE: $crate::prelude::Unite = $crate::prelude::Unite::$unite;
            const ENCRE: $crate::Encre = $encre;
        }
    };
}

const LIANT: char = '\u{200D}';
const STYLE_TEXTE: char = '\u{FE0E}';
const STYLE_EMOJI: char = '\u{FE0F}';
const TOUCHE: char = '\u{20E3}';

/// Ne prend de place sur aucun écran : les points de code ignorables par défaut
/// d'Unicode, dont les forçages de sens et la cellule braille vide, et le reste de la
/// catégorie Cf (les marques d'annotation interlinéaire, les contrôles égyptiens). Les
/// marques préfixées de Cf (U+0600…) restent : elles se dessinent (PUN-23, CAT-85).
const fn invisible(c: char) -> bool {
    matches!(c,
        '\u{AD}' | '\u{34F}' | '\u{61C}' | '\u{115F}'..='\u{1160}' | '\u{17B4}'..='\u{17B5}'
        | '\u{180B}'..='\u{180F}' | '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}'
        | '\u{2060}'..='\u{206F}' | '\u{2800}' | '\u{3164}' | '\u{FE00}'..='\u{FE0F}'
        | '\u{FEFF}' | '\u{FFA0}' | '\u{FFF0}'..='\u{FFFB}' | '\u{13430}'..='\u{1343F}'
        | '\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}' | '\u{E0000}'..='\u{E0FFF}')
}

/// Les blocs d'où viennent les emoji, plus larges que les emoji eux-mêmes.
const fn pictogramme(c: char) -> bool {
    matches!(c,
        '\u{A9}' | '\u{AE}' | '\u{203C}' | '\u{2049}' | '\u{2122}' | '\u{2139}'
        | '\u{2190}'..='\u{2BFF}' | '\u{3030}' | '\u{303D}' | '\u{3297}' | '\u{3299}'
        | '\u{1F000}'..='\u{1FAFF}')
}

/// `cs[i]` est-il gardé ? Les invisibles dont un emoji est fait restent dans un emoji,
/// et nulle part ailleurs : `👩‍👧` garde son liant, `a‍b` le perd (CAT-85).
fn garde(cs: &[char], i: usize) -> bool {
    let Some(&c) = cs.get(i) else {
        return false;
    };
    let avant = |de: usize| i.checked_sub(de).and_then(|j| cs.get(j)).copied();
    let apres = i.checked_add(1).and_then(|j| cs.get(j)).copied();
    let dessine_avant = avant(1).is_some_and(pictogramme);
    match c {
        LIANT => {
            let stylise_avant = avant(1) == Some(STYLE_EMOJI) && avant(2).is_some_and(pictogramme);
            (dessine_avant || stylise_avant) && apres.is_some_and(pictogramme)
        }
        STYLE_EMOJI if apres == Some(TOUCHE) => {
            avant(1).is_some_and(|base| base.is_ascii_digit() || matches!(base, '#' | '*'))
        }
        STYLE_EMOJI | STYLE_TEXTE => dessine_avant,
        autre => !autre.is_control() && !invisible(autre),
    }
}

/// Les caractères visibles d'un mot.
fn visibles(mot: &str) -> String {
    let cs: Vec<char> = mot.chars().collect();
    (0..cs.len())
        .filter(|&i| garde(&cs, i))
        .filter_map(|i| cs.get(i).copied())
        .collect()
}

/// Le poids d'un graphème dans une unité.
fn poids_grapheme(g: &str, unite: Unite) -> u32 {
    let n = match unite {
        // La télé affiche en capitales : « ß » en fait deux, « ﬃ » trois (PUN-24, CAT-82).
        // Les marques combinantes, les liants et les sélecteurs ne prennent pas de place à
        // eux seuls ; un graphème pèse au moins une capitale.
        Unite::Capitales => g
            .chars()
            .flat_map(char::to_uppercase)
            .filter(|c| !is_combining_mark(*c) && !invisible(*c) && *c != TOUCHE)
            .count()
            .max(1),
        Unite::Utf16 => g.encode_utf16().count(),
        Unite::PointsDeCode => g.chars().count(),
        Unite::Graphemes => 1,
    };
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Le poids d'un texte dans une unité.
fn poids(s: &str, unite: Unite) -> u32 {
    s.graphemes(true)
        .map(|g| poids_grapheme(g, unite))
        .fold(0u32, u32::saturating_add)
}

/// Le plus long début de `s` qui pèse au plus `max`, coupé entre deux graphèmes.
fn couper(s: &str, max: u32, unite: Unite) -> &str {
    let mut pese = 0u32;
    let mut fin = 0usize;
    for (debut, g) in s.grapheme_indices(true) {
        pese = pese.saturating_add(poids_grapheme(g, unite));
        if pese > max {
            return s.get(..debut).unwrap_or_default();
        }
        fin = debut.saturating_add(g.len());
    }
    s.get(..fin).unwrap_or_default()
}

/// Le seul nettoyage, pour une limite donnée.
fn nettoyer_brut(brut: &str, max: u32, unite: Unite) -> String {
    // Mot par mot : un caractère retiré entre deux espaces ne les laisse pas toutes les
    // deux (PUN-26).
    let mots: Vec<String> = brut
        .split_whitespace()
        .map(visibles)
        .filter(|m| !m.is_empty())
        .collect();
    let entier: String = mots.join(" ").nfc().collect();
    // Une coupe peut laisser un liant, ou une espace, sans rien après.
    let montre = visibles(couper(&entier, max, unite));
    let montre: String = montre.trim().nfc().collect();
    if montre.chars().all(is_combining_mark) {
        return String::new();
    }
    montre
}

/// Comment un texte se dessine avec une encre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encrable {
    /// Chaque caractère se dessine.
    Tout,
    /// Certains seulement.
    Partiel,
    /// Aucun.
    Rien,
}

/// La seule clé de comparaison du commun : casse ignorée, sans sélecteurs ni liants, la
/// même sur tous les appareils et sans langue (CAT-117, PUN-22). Les accents comptent :
/// `pêche` n'est pas `péché`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct CleTexte(String);

impl CleTexte {
    /// La clé d'un texte quelconque.
    pub fn de(s: &str) -> CleTexte {
        let replie: String = s
            .chars()
            .filter(|c| !matches!(*c, LIANT | STYLE_TEXTE | STYLE_EMOJI | '\u{200C}'))
            .nfc()
            .collect::<String>()
            .to_lowercase()
            .nfc()
            .collect();
        CleTexte(replie)
    }

    /// La clé, comme texte.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Forme for CleTexte {
    const EMPREINTE: u64 = forme::nommee("CleTexte");
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}
impl Etat for CleTexte {
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
impl Message for CleTexte {}

/// Un texte nettoyé, sous sa limite `L`.
///
/// Au décodage, un `Texte<L>` n'est accepté que s'il est déjà nettoyé et sous sa
/// limite : une page honnête n'est jamais refusée, une page hostile l'est toujours.
pub struct Texte<L: Limite> {
    texte: String,
    _limite: PhantomData<L>,
}

impl<L: Limite> Texte<L> {
    /// Le seul nettoyage. Nettoyer deux fois revient à nettoyer une fois.
    pub fn nettoyer(brut: &str) -> Texte<L> {
        Texte {
            texte: nettoyer_brut(brut, L::MAX, L::UNITE),
            _limite: PhantomData,
        }
    }

    /// Le texte.
    pub fn as_str(&self) -> &str {
        &self.texte
    }

    /// Est-il vide ?
    pub fn vide(&self) -> Oui {
        if self.texte.is_empty() {
            Oui::Oui
        } else {
            Oui::Non
        }
    }

    /// La seule clé de comparaison (CAT-117).
    pub fn cle(&self) -> CleTexte {
        CleTexte::de(&self.texte)
    }

    /// Le poids, en unités de `L::UNITE`.
    pub fn poids(&self) -> u32 {
        poids(&self.texte, L::UNITE)
    }

    /// Ce que l'encre de `L` en dessine. Les espaces ne comptent pas.
    pub fn encre(&self) -> Encrable {
        let Encre::Table(table) = L::ENCRE else {
            return Encrable::Tout;
        };
        let (mut oui, mut non) = (0usize, 0usize);
        for c in self.texte.chars().filter(|c| !c.is_whitespace()) {
            let n = u32::from(c);
            if table.iter().any(|(a, b)| *a <= n && n <= *b) {
                oui = oui.saturating_add(1);
            } else {
                non = non.saturating_add(1);
            }
        }
        match (oui, non) {
            (_, 0) => Encrable::Tout,
            (0, _) => Encrable::Rien,
            _ => Encrable::Partiel,
        }
    }

    /// Ajoute un suffixe, et raccourcit le texte entre graphèmes pour qu'il tienne
    /// (PUN-25). Un suffixe qui commence par un blanc garde une espace devant lui :
    /// `MMMMMMMMMMMM` suivi de `" 2"` donne `MMMMMMMMMM 2` sous 12 capitales, et non
    /// `MMMMMMMMMMM2` (besoin 4 des jeux, `conception/BESOINS-DES-JEUX.md`). Un suffixe
    /// trop long pour la limite est lui-même coupé.
    #[must_use]
    pub fn avec_suffixe(&self, suffixe: &str) -> Texte<L> {
        let s = nettoyer_brut(suffixe, L::MAX, L::UNITE);
        let joint = if !s.is_empty() && suffixe.starts_with(char::is_whitespace) {
            let mut j = String::from(" ");
            j.push_str(&s);
            j
        } else {
            s
        };
        let reste = L::MAX.saturating_sub(poids(&joint, L::UNITE));
        let debut = couper(&self.texte, reste, L::UNITE);
        let mut tout = String::from(debut.trim_end());
        tout.push_str(&joint);
        Texte::nettoyer(&tout)
    }

    /// Le même texte sous une autre limite, s'il y tient tel quel.
    pub fn borne<M: Limite>(&self) -> Option<Texte<M>> {
        let t = Texte::<M>::nettoyer(&self.texte);
        (t.texte == self.texte).then_some(t)
    }

    /// Ce texte est-il déjà nettoyé pour `L` ?
    fn propre(s: &str) -> bool {
        nettoyer_brut(s, L::MAX, L::UNITE) == s
    }
}

impl<L: Limite> Clone for Texte<L> {
    fn clone(&self) -> Self {
        Texte {
            texte: self.texte.clone(),
            _limite: PhantomData,
        }
    }
}

impl<L: Limite> PartialEq for Texte<L> {
    fn eq(&self, autre: &Self) -> bool {
        self.texte == autre.texte
    }
}

impl<L: Limite> Eq for Texte<L> {}

impl<L: Limite> fmt::Debug for Texte<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Texte").field(&self.texte).finish()
    }
}

impl<L: Limite> serde::Serialize for Texte<L> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.texte)
    }
}

impl<'de, L: Limite> serde::Deserialize<'de> for Texte<L> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = <&str>::deserialize(d)?;
        if Texte::<L>::propre(s) {
            Ok(Texte {
                texte: String::from(s),
                _limite: PhantomData,
            })
        } else {
            Err(serde::de::Error::custom(
                "un texte qui n'est pas nettoyé, ou au-delà de sa limite",
            ))
        }
    }
}

const fn unite_rang(u: Unite) -> u64 {
    match u {
        Unite::Capitales => 0,
        Unite::Utf16 => 1,
        Unite::PointsDeCode => 2,
        Unite::Graphemes => 3,
    }
}

impl<L: Limite> Forme for Texte<L> {
    const EMPREINTE: u64 = forme::entier(forme::de("Texte", L::MAX as u64), unite_rang(L::UNITE));
    const SECRET: Contient = Contient::Non;
    const VIDE: Contient = Contient::Non;
}

impl<L: Limite> Etat for Texte<L> {
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

impl<L: Limite> Message for Texte<L> {}
