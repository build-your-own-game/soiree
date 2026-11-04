//! Les valeurs validées au décodage (§ 2.3) : ce qui franchit une frontière ne se
//! décode pas invalide, et n'atteint donc jamais l'étape (SAL-117).

use core::fmt;

use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Un oui ou non, partout où un `bool` en paramètre serait permuté sans bruit.
///
/// Encodé `Oui` = 0, `Non` = 1, dans l'ordre de déclaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Oui {
    /// Oui.
    Oui,
    /// Non.
    Non,
}

impl Oui {
    /// `true` pour [`Oui::Oui`].
    pub const fn est_oui(self) -> bool {
        matches!(self, Oui::Oui)
    }

    /// L'inverse.
    #[must_use]
    pub const fn inverse(self) -> Oui {
        match self {
            Oui::Oui => Oui::Non,
            Oui::Non => Oui::Oui,
        }
    }
}

/// Une valeur hors de son domaine, refusée à la construction ou au décodage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HorsBornes {
    /// Ce que la valeur aurait dû être.
    pub attendu: &'static str,
}

impl fmt::Display for HorsBornes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "valeur hors de son domaine : attendu {}", self.attendu)
    }
}

// ---------------------------------------------------------------------------
// Les flottants
// ---------------------------------------------------------------------------

/// Un `f32` fini, ni NaN ni infini (HIS-45, OUT-175, CAT-04, SQ-67).
///
/// Un sous-normal est ramené à 0, et le zéro négatif aussi : deux valeurs qui
/// s'affichent pareil ne se distinguent pas par une division. Comme aucune valeur
/// n'est NaN, l'égalité est une vraie équivalence.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Fini(f32);

impl Eq for Fini {}

impl Fini {
    /// Zéro.
    pub const ZERO: Fini = Fini(0.0);

    /// `x` s'il est fini ; un sous-normal ou un zéro négatif devient 0.
    pub const fn new(x: f32) -> Option<Fini> {
        match x.classify() {
            core::num::FpCategory::Nan | core::num::FpCategory::Infinite => None,
            core::num::FpCategory::Zero | core::num::FpCategory::Subnormal => Some(Fini::ZERO),
            core::num::FpCategory::Normal => Some(Fini(x)),
        }
    }

    /// La valeur.
    pub const fn get(self) -> f32 {
        self.0
    }
}

impl TryFrom<f32> for Fini {
    type Error = HorsBornes;

    fn try_from(x: f32) -> Result<Self, Self::Error> {
        Fini::new(x).ok_or(HorsBornes {
            attendu: "un flottant fini",
        })
    }
}

impl From<Fini> for f32 {
    fn from(f: Fini) -> f32 {
        f.0
    }
}

/// Un axe dans [-1, 1].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "f32", into = "f32")]
pub struct Axe(Fini);

impl Axe {
    /// Le centre.
    pub const ZERO: Axe = Axe(Fini::ZERO);

    /// `x` s'il est fini et dans [-1, 1].
    pub const fn new(x: f32) -> Option<Axe> {
        match Fini::new(x) {
            Some(f) if -1.0 <= f.get() && f.get() <= 1.0 => Some(Axe(f)),
            Some(_) | None => None,
        }
    }

    /// La valeur, dans [-1, 1].
    pub const fn get(self) -> f32 {
        self.0.get()
    }
}

impl TryFrom<f32> for Axe {
    type Error = HorsBornes;

    fn try_from(x: f32) -> Result<Self, Self::Error> {
        Axe::new(x).ok_or(HorsBornes {
            attendu: "un axe fini dans [-1, 1]",
        })
    }
}

impl From<Axe> for f32 {
    fn from(a: Axe) -> f32 {
        a.get()
    }
}

/// Deux axes de norme au plus 1, zone morte de l'appareil déjà appliquée par le
/// commun. La croix (1, 1) n'en est pas un : [`Axe2::ramener`] la ramène à la norme 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Axe2Brut", into = "Axe2Brut")]
pub struct Axe2 {
    x: Axe,
    y: Axe,
}

/// La tolérance sur le carré de la norme : `(0,6 ; 0,8)` vaut 1,000 000 1 en `f32`.
/// Une seule définition, pour la construction et pour le décodage.
const NORME_CARREE_MAX: f32 = 1.0 + 1.0e-6;

/// La forme sur le fil d'un [`Axe2`] : deux `f32`, `x` puis `y`.
#[derive(Clone, Copy, Serialize, Deserialize)]
struct Axe2Brut {
    x: f32,
    y: f32,
}

impl Axe2 {
    /// Le centre.
    pub const ZERO: Axe2 = Axe2 {
        x: Axe::ZERO,
        y: Axe::ZERO,
    };

    /// `(x, y)` si chacun est un [`Axe`] et que la norme ne dépasse pas 1.
    pub fn new(x: f32, y: f32) -> Option<Axe2> {
        let (ax, ay) = (Axe::new(x)?, Axe::new(y)?);
        let (fx, fy) = (ax.get(), ay.get());
        if fx * fx + fy * fy <= NORME_CARREE_MAX {
            Some(Axe2 { x: ax, y: ay })
        } else {
            None
        }
    }

    /// `(x, y)`, ramené à la norme 1 s'il la dépasse. `None` si l'un n'est pas fini.
    pub fn ramener(x: f32, y: f32) -> Option<Axe2> {
        let (fx, fy) = (Fini::new(x)?.get(), Fini::new(y)?.get());
        if let Some(a) = Axe2::new(fx, fy) {
            return Some(a);
        }
        // Diviser d'abord par le plus grand des deux : le carré d'un très grand
        // nombre ne déborde pas vers l'infini.
        // Une comparaison écrite, et non `f32::max`, que les règles interdisent : le
        // même résultat en natif et dans le module.
        let (ax, ay) = (fx.abs(), fy.abs());
        let m = if ax >= ay { ax } else { ay };
        let (nx, ny) = (fx / m, fy / m);
        let n = libm::sqrtf(nx * nx + ny * ny);
        Axe2::new((nx / n).clamp(-1.0, 1.0), (ny / n).clamp(-1.0, 1.0))
    }

    /// L'axe horizontal.
    pub const fn x(self) -> f32 {
        self.x.get()
    }

    /// L'axe vertical.
    pub const fn y(self) -> f32 {
        self.y.get()
    }
}

impl TryFrom<Axe2Brut> for Axe2 {
    type Error = HorsBornes;

    fn try_from(b: Axe2Brut) -> Result<Self, Self::Error> {
        Axe2::new(b.x, b.y).ok_or(HorsBornes {
            attendu: "deux axes finis de norme au plus 1",
        })
    }
}

impl From<Axe2> for Axe2Brut {
    fn from(a: Axe2) -> Axe2Brut {
        Axe2Brut { x: a.x(), y: a.y() }
    }
}

// ---------------------------------------------------------------------------
// Les entiers
// ---------------------------------------------------------------------------

/// Un entier dans [`MIN`, `MAX`], vérifié au décodage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "i64", into = "i64")]
pub struct Borne<const MIN: i64, const MAX: i64>(i64);

impl<const MIN: i64, const MAX: i64> Borne<MIN, MAX> {
    /// Une `Borne` dont `MIN > MAX` ne compile pas.
    const BORNES_ORDONNEES: () = assert!(MIN <= MAX, "Borne<MIN, MAX> demande MIN <= MAX");

    /// `MIN`.
    pub const PLUS_PETIT: Self = {
        let () = Self::BORNES_ORDONNEES;
        Borne(MIN)
    };

    /// `MAX`.
    pub const PLUS_GRAND: Self = {
        let () = Self::BORNES_ORDONNEES;
        Borne(MAX)
    };

    /// `v` s'il est dans [`MIN`, `MAX`].
    pub const fn new(v: i64) -> Option<Self> {
        let () = Self::BORNES_ORDONNEES;
        if MIN <= v && v <= MAX {
            Some(Borne(v))
        } else {
            None
        }
    }

    /// La valeur.
    pub const fn get(self) -> i64 {
        self.0
    }

    /// `v - MIN`, pour indexer un tableau de `MAX - MIN + 1` cases ; saturé à `usize::MAX`.
    pub const fn indice(self) -> usize {
        let d = self.0.abs_diff(MIN);
        if d > usize::MAX as u64 {
            usize::MAX
        } else {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "vérifié juste au-dessus : d tient dans un usize"
            )]
            let i = d as usize;
            i
        }
    }
}

impl<const MIN: i64, const MAX: i64> TryFrom<i64> for Borne<MIN, MAX> {
    type Error = HorsBornes;

    fn try_from(v: i64) -> Result<Self, Self::Error> {
        Borne::new(v).ok_or(HorsBornes {
            attendu: "un entier dans ses bornes",
        })
    }
}

impl<const MIN: i64, const MAX: i64> From<Borne<MIN, MAX>> for i64 {
    fn from(b: Borne<MIN, MAX>) -> i64 {
        b.0
    }
}

/// Un compteur qui ne fait que monter, absolu, en boucle sur `u32` : les appuis
/// d'un bouton, une visée cumulée. Un paquet perdu ne décale rien (REM-13).
///
/// Il n'est pas ordonné : après un tour complet, le plus grand n'est pas le plus récent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cumul(u32);

impl Cumul {
    /// Le compteur à zéro.
    pub const ZERO: Cumul = Cumul(0);

    /// Un compteur à cette valeur.
    pub const fn new(n: u32) -> Cumul {
        Cumul(n)
    }

    /// La valeur brute.
    pub const fn get(self) -> u32 {
        self.0
    }

    /// Ce qui s'est ajouté depuis `avant`, en boucle.
    pub const fn depuis(self, avant: Cumul) -> u32 {
        self.0.wrapping_sub(avant.0)
    }

    /// Le compteur augmenté de `n`, en boucle.
    #[must_use]
    pub const fn plus(self, n: u32) -> Cumul {
        Cumul(self.0.wrapping_add(n))
    }
}

/// L'état d'un bouton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appui {
    /// Enfoncé.
    Tenu,
    /// Levé.
    Relache,
}

/// Un bouton tenu : son état, et le nombre d'appuis depuis l'arrivée de l'appareil.
/// Un appui plus court qu'un envoi laisse une trace ; deux appuis comptent deux
/// (REM-16, REM-17).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bouton {
    /// Enfoncé ou levé, à l'instant de l'envoi.
    pub etat: Appui,
    /// Les appuis depuis l'arrivée de l'appareil.
    pub appuis: Cumul,
}

impl Bouton {
    /// Un bouton levé, jamais appuyé.
    pub const REPOS: Bouton = Bouton {
        etat: Appui::Relache,
        appuis: Cumul::ZERO,
    };
}

/// Une manette standard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manette {
    /// Le stick gauche, puis le droit.
    pub sticks: [Axe2; 2],
    /// Les gâchettes, en millièmes de leur course.
    pub gachettes: [Borne<0, 1000>; 2],
    /// Les seize boutons de la disposition standard.
    pub boutons: [Bouton; 16],
}

impl Manette {
    /// Une manette au repos, jamais touchée.
    pub const REPOS: Manette = Manette {
        sticks: [Axe2::ZERO; 2],
        gachettes: [Borne::PLUS_PETIT; 2],
        boutons: [Bouton::REPOS; 16],
    };

    /// Cette manette relâchée : sticks au centre, gâchettes et boutons levés,
    /// compteurs d'appuis gardés (un relâchement n'efface pas un appui déjà fait).
    #[must_use]
    pub fn relachee(&self) -> Manette {
        let mut m = *self;
        m.sticks = [Axe2::ZERO; 2];
        m.gachettes = [Borne::PLUS_PETIT; 2];
        for b in &mut m.boutons {
            b.etat = Appui::Relache;
        }
        m
    }

    /// `Oui` si rien n'est tenu.
    pub fn est_relachee(&self) -> Oui {
        if *self == self.relachee() {
            Oui::Oui
        } else {
            Oui::Non
        }
    }
}

// ---------------------------------------------------------------------------
// Les courts textes du commun
// ---------------------------------------------------------------------------

/// Un court texte du commun (nom de fichier, type, prénom proposé, nom de flux) :
/// UTF-8, 64 octets au plus, sans caractère de contrôle. Le jeu le nettoie en
/// `Texte<L>` s'il le garde. Encodé comme un texte.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TexteCourt {
    octets: [u8; TexteCourt::MAX],
    longueur: u8,
}

impl TexteCourt {
    /// Le nombre d'octets au plus.
    pub const MAX: usize = 64;

    /// Le texte vide.
    pub const VIDE: TexteCourt = TexteCourt {
        octets: [0; TexteCourt::MAX],
        longueur: 0,
    };

    /// Un texte du code du commun ou du jeu, à écrire dans une constante : un texte
    /// invalide y est une erreur de compilation.
    #[expect(
        clippy::panic,
        reason = "évalué dans une constante, un texte invalide ne compile pas"
    )]
    pub const fn new(s: &'static str) -> TexteCourt {
        match TexteCourt::essayer(s) {
            Some(t) => t,
            None => panic!("TexteCourt : 64 octets au plus, sans caractère de contrôle"),
        }
    }

    /// `s` s'il fait 64 octets au plus et n'a aucun caractère de contrôle.
    pub const fn essayer(s: &str) -> Option<TexteCourt> {
        let b = s.as_bytes();
        if !sans_controle(b) {
            return None;
        }
        match copier::<{ TexteCourt::MAX }>(b) {
            Some((octets, longueur)) => Some(TexteCourt { octets, longueur }),
            None => None,
        }
    }

    /// Le texte.
    pub fn as_str(&self) -> &str {
        texte_de(&self.octets, self.longueur)
    }
}

impl fmt::Debug for TexteCourt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TexteCourt").field(&self.as_str()).finish()
    }
}

impl Serialize for TexteCourt {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for TexteCourt {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(VisiteurTexte {
            attendu: "64 octets UTF-8 au plus, sans caractère de contrôle",
            construire: TexteCourt::essayer,
        })
    }
}

/// Le nom d'une touche physique, tel que le navigateur le donne (« `KeyW` », « `Space` ») :
/// du texte, pas un contrat entre nos fichiers (CONTRAT, « Les types de bout en
/// bout »). 1 à 24 caractères ASCII visibles. Encodé comme un texte.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CodeTouche {
    octets: [u8; CodeTouche::MAX],
    longueur: u8,
}

impl CodeTouche {
    /// Le nombre d'octets au plus.
    pub const MAX: usize = 24;

    /// La barre d'espace.
    pub const ESPACE: CodeTouche = CodeTouche::new("Space");
    /// Entrée.
    pub const ENTREE: CodeTouche = CodeTouche::new("Enter");
    /// Échap.
    pub const ECHAP: CodeTouche = CodeTouche::new("Escape");
    /// Tabulation.
    pub const TAB: CodeTouche = CodeTouche::new("Tab");
    /// Retour arrière.
    pub const RETOUR_ARRIERE: CodeTouche = CodeTouche::new("Backspace");
    /// Flèche haut.
    pub const HAUT: CodeTouche = CodeTouche::new("ArrowUp");
    /// Flèche bas.
    pub const BAS: CodeTouche = CodeTouche::new("ArrowDown");
    /// Flèche gauche.
    pub const GAUCHE: CodeTouche = CodeTouche::new("ArrowLeft");
    /// Flèche droite.
    pub const DROITE: CodeTouche = CodeTouche::new("ArrowRight");
    /// F8 : la marque au journal, réservée au commun (§ 6.4).
    pub const F8: CodeTouche = CodeTouche::new("F8");

    /// Un nom de touche du code, à écrire dans une constante : un nom invalide y
    /// est une erreur de compilation.
    #[expect(
        clippy::panic,
        reason = "évalué dans une constante, un nom invalide ne compile pas"
    )]
    pub const fn new(nom: &'static str) -> CodeTouche {
        match CodeTouche::essayer(nom) {
            Some(t) => t,
            None => panic!("CodeTouche : 1 à 24 caractères ASCII visibles"),
        }
    }

    /// `nom` s'il fait 1 à 24 caractères ASCII visibles.
    pub const fn essayer(nom: &str) -> Option<CodeTouche> {
        let b = nom.as_bytes();
        if b.is_empty() || !ascii_visible(b) {
            return None;
        }
        match copier::<{ CodeTouche::MAX }>(b) {
            Some((octets, longueur)) => Some(CodeTouche { octets, longueur }),
            None => None,
        }
    }

    /// Le nom.
    pub fn as_str(&self) -> &str {
        texte_de(&self.octets, self.longueur)
    }
}

impl fmt::Debug for CodeTouche {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CodeTouche").field(&self.as_str()).finish()
    }
}

impl Serialize for CodeTouche {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CodeTouche {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_str(VisiteurTexte {
            attendu: "un nom de touche de 1 à 24 caractères ASCII visibles",
            construire: CodeTouche::essayer,
        })
    }
}

/// Lit un texte et le valide par `construire`.
struct VisiteurTexte<T> {
    attendu: &'static str,
    construire: fn(&str) -> Option<T>,
}

impl<T> Visitor<'_> for VisiteurTexte<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.attendu)
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<T, E> {
        (self.construire)(v).ok_or_else(|| E::invalid_value(Unexpected::Str(v), &self))
    }
}

/// Aucun caractère de contrôle (catégorie Cc) : ni C0, ni DEL, ni C1. `b` est de
/// l'UTF-8 valide ; un caractère C1 s'y écrit `C2 80` à `C2 9F`.
const fn sans_controle(b: &[u8]) -> bool {
    let mut reste = b;
    while let [premier, suite @ ..] = reste {
        if *premier < 0x20 || *premier == 0x7f {
            return false;
        }
        if *premier == 0xc2
            && let [second, ..] = suite
            && 0x80 <= *second
            && *second <= 0x9f
        {
            return false;
        }
        reste = suite;
    }
    true
}

const fn ascii_visible(b: &[u8]) -> bool {
    let mut reste = b;
    while let [premier, suite @ ..] = reste {
        if *premier < 0x21 || *premier > 0x7e {
            return false;
        }
        reste = suite;
    }
    true
}

/// Copie `b` dans un tableau de `N` octets, complété de zéros, avec sa longueur.
const fn copier<const N: usize>(b: &[u8]) -> Option<([u8; N], u8)> {
    if b.len() > N || b.len() > u8::MAX as usize {
        return None;
    }
    let mut octets = [0u8; N];
    let Some((tete, _)) = octets.split_at_mut_checked(b.len()) else {
        return None;
    };
    tete.copy_from_slice(b);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "vérifié en tête : la longueur tient dans un u8"
    )]
    let longueur = b.len() as u8;
    Some((octets, longueur))
}

/// Le texte des `longueur` premiers octets, écrits par `copier` depuis un `&str`.
fn texte_de(octets: &[u8], longueur: u8) -> &str {
    octets
        .get(..usize::from(longueur))
        .and_then(|b| core::str::from_utf8(b).ok())
        .unwrap_or("")
}

// ---------------------------------------------------------------------------
// Les longueurs de la scène
// ---------------------------------------------------------------------------

/// Une longueur de la scène, entière, en 1/256 d'unité du repère. NaN et l'infini
/// n'y sont pas représentables (REM-57).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Fixe(i32);

impl Fixe {
    /// Le nombre de pas d'un `Fixe` dans une unité du repère.
    pub const PAR_UNITE: i32 = 256;
    /// Zéro.
    pub const ZERO: Fixe = Fixe(0);
    /// La plus petite longueur : une valeur saturée.
    pub const MIN: Fixe = Fixe(i32::MIN);
    /// La plus grande longueur : une valeur saturée.
    pub const MAX: Fixe = Fixe(i32::MAX);

    /// `n` unités du repère, saturé.
    pub const fn unites(n: i32) -> Fixe {
        Fixe(n.saturating_mul(Fixe::PAR_UNITE))
    }

    /// `x` unités du repère, arrondi au 1/256 le plus proche et saturé ; NaN donne 0.
    /// L'épreuve compte les valeurs saturées (E17).
    pub fn depuis_f32(x: f32) -> Fixe {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "`as` sature et rend 0 pour NaN : c'est la règle voulue"
        )]
        let brut = libm::roundf(x * 256.0) as i32;
        Fixe(brut)
    }

    /// La valeur en 1/256 d'unité.
    pub const fn brut(self) -> i32 {
        self.0
    }

    /// Une longueur depuis sa valeur en 1/256 d'unité.
    pub const fn depuis_brut(n: i32) -> Fixe {
        Fixe(n)
    }

    /// La valeur en unités du repère, sans perte, pour dessiner.
    pub fn en_unites(self) -> f64 {
        f64::from(self.0) / 256.0
    }

    /// `Oui` pour [`Fixe::MIN`] et [`Fixe::MAX`].
    pub const fn saturee(self) -> Oui {
        if self.0 == i32::MIN || self.0 == i32::MAX {
            Oui::Oui
        } else {
            Oui::Non
        }
    }
}

/// Un rectangle de la scène : coin haut gauche, largeur, hauteur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    /// Le bord gauche.
    pub x: Fixe,
    /// Le bord haut.
    pub y: Fixe,
    /// La largeur.
    pub l: Fixe,
    /// La hauteur.
    pub h: Fixe,
}

/// Une couleur : rouge, vert, bleu, opacité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    /// Noir opaque.
    pub const NOIR: Rgba = Rgba(0, 0, 0, 255);
    /// Blanc opaque.
    pub const BLANC: Rgba = Rgba(255, 255, 255, 255);
    /// Transparent.
    pub const TRANSPARENT: Rgba = Rgba(0, 0, 0, 0);
}
