//! Les règles du cœur, en fonctions pures : c'est elles que la preuve visera d'abord
//! (INTERFACE.md, § 8.6, D33 ; REPONSES-PROVISOIRES.md, question 18).
//!
//! # La forme de ces règles
//!
//! - **Pures.** Aucune ne lit l'heure, n'appelle une instance ni ne garde d'état
//!   caché : le résultat ne dépend que des arguments. L'état d'un tenu est un
//!   argument (`&mut EtatTenu<V>`), rendu au cœur qui le garde.
//! - **Petites, sans allocation cachée.** Seules [`demandes`], [`fusionner`] et
//!   [`images_citees`] construisent un `Vec`, parce que leur résultat en est un ;
//!   [`valider_scene`] tient une table de travail, de la taille de la scène ; [`a_lancer`] et
//!   [`a_annuler`] donnent le même résultat sans allouer. La valeur d'un tenu est
//!   générique ([`ValeurTenue`]) : la règle ne fait que la déplacer, et une preuve
//!   peut la prendre `Copy`.
//! - **Leurs propriétés sont écrites.** Chaque fonction énonce, dans sa
//!   documentation, ce qui doit rester vrai pour toutes ses entrées. Les tests
//!   `tests/regles.rs` les éprouvent par `proptest` ; une preuve les reprendra
//!   telles quelles.
//!
//! # Ce qui reste à prouver d'abord
//!
//! L'ordre ([`placer`], [`suivre`], [`ordonner`]), « un événement compte exactement
//! une fois » (le cœur ne passe chaque arrivée qu'une fois dans [`ordonner`], et
//! [`tenu`] n'écarte que ce qui est remplacé) et la reprise (le cœur ne dépend que
//! de ces fonctions et du journal).

use alloc::vec::Vec;

use jeu_abi::{
    CauseTenu, Cle, ComposantVuFil, Deplace, Duree, Effet, Fixe, FormeEcran, Instant, Oui,
    PAS_PAR_SECONDE, Pas, Rect, ReglesTenuFil, ReveilFil, SceneFil, SorteNoeud, Tenue, Unite,
    fnv1a64_suite,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::arrivee::{ComposantArrivee, EcranArrivee};

// ---------------------------------------------------------------------------
// Placer et ordonner (§ 4.3)
// ---------------------------------------------------------------------------

/// Où placer une arrivée : son instant `fait`, borné, et le pas `place` où le cœur
/// l'applique (§ 4.3, SAL-129).
///
/// ```text
/// fait  ← clamp(fait annoncé, recu − demi-aller-retour − antidatage_max, recu)
/// place ← max(fait.pas, definitif + 1)
/// ```
///
/// La soustraction est saturée à l'instant zéro, l'addition au plus grand pas.
///
/// # Propriétés
///
/// - `fait ≤ recu` : un appareil n'annonce jamais un geste après sa réception.
/// - `fait ≥ recu − (demi_aller_retour + antidatage_max)`, à la saturation près :
///   un appareil n'antidate pas au-delà de sa demi-latence mesurée et de la marge
///   déclarée (SAL-129). La latence se mesure par écho depuis l'ordinateur de la
///   soirée : un appareil peut la grossir, pas la réduire.
/// - Si le `fait` annoncé est dans ces bornes, il est gardé tel quel.
/// - `place ≥ definitif + 1` : rien ne s'applique à un pas déjà définitif.
/// - `place ≥ fait.pas` : rien ne s'applique avant d'avoir été fait.
/// - Monotone : un `fait` annoncé plus tard ne donne jamais un `fait` ni une
///   `place` plus tôt, à `recu` égal.
pub fn placer(
    fait: Instant,
    recu: Instant,
    demi_aller_retour: Duree,
    antidatage_max: Duree,
    definitif: Pas,
) -> (Instant, Pas) {
    let plancher = reculer(recu, demi_aller_retour.plus(antidatage_max));
    let fait = fait.max(plancher).min(recu);
    let suivant = Pas::du_fil(definitif.numero().saturating_add(1));
    (fait, fait.pas.max(suivant))
}

/// `recu` moins une durée, saturé à l'instant zéro.
fn reculer(recu: Instant, d: Duree) -> Instant {
    match recu.pas.numero().checked_sub(d.en_pas()) {
        Some(p) => Instant {
            pas: Pas::du_fil(p),
            fraction: recu.fraction,
        },
        None => Instant::de(Pas::ZERO),
    }
}

/// Garde l'ordre d'un appareil : une arrivée ne passe jamais avant celle qui l'a
/// précédée depuis le même appareil.
///
/// Le transport livre les événements d'un appareil dans l'ordre où il les a
/// envoyés. Un appareil qui annoncerait des instants décroissants (une horloge
/// réestimée, ou un tricheur) ferait passer son second geste avant le premier
/// dans [`ordonner`]. `suivre` relève donc `fait` et `place` au niveau de
/// l'arrivée précédente du même appareil, `precedente`.
///
/// # Propriétés
///
/// - `fait ≥ precedente.fait` et `place ≥ precedente.place`.
/// - Sans précédente, ou si l'arrivée est déjà après elle, rien ne change.
/// - `fait` n'est relevé qu'au niveau de la précédente, jamais au-delà.
pub fn suivre(fait: Instant, place: Pas, precedente: Option<(Instant, Pas)>) -> (Instant, Pas) {
    match precedente {
        Some((f, p)) => (fait.max(f), place.max(p)),
        None => (fait, place),
    }
}

/// Le vu d'un geste, borné par la dernière vue produite pour son appareil (§ 4.3).
///
/// # Propriétés
///
/// `borner_vu(vu, derniere) ≤ derniere` et `≤ vu` : un appareil ne peut pas
/// prétendre avoir vu une vue qu'on ne lui a pas envoyée (PUN-38, SAL-115).
pub fn borner_vu(vu: Pas, derniere: Pas) -> Pas {
    vu.min(derniere)
}

/// D'où vient une entrée, pour départager deux entrées du même instant.
///
/// Jamais l'[`jeu_abi::Appareil`] exposé, dont la valeur est tirée au hasard :
/// le rang interne d'un appareil est l'ordre où le cœur l'a vu la première fois
/// (CAT-27).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Origine {
    /// Le commun lui-même : une réponse à une demande, un relâchement, le temps.
    Commun,
    /// L'écran de l'ordinateur de la soirée.
    Ecran,
    /// Un appareil, par son rang interne.
    Appareil(u32),
}

impl Origine {
    /// Le code de l'origine dans le hachage qui départage.
    fn code(self) -> u64 {
        match self {
            Origine::Commun => 0,
            Origine::Ecran => 1,
            Origine::Appareil(r) => u64::from(r).saturating_add(2),
        }
    }
}

/// Ce que [`ordonner`] lit d'une entrée placée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placee {
    /// L'instant borné où elle a été faite.
    pub fait: Instant,
    /// D'où elle vient.
    pub origine: Origine,
    /// Son numéro d'arrivée : unique, croissant dans l'ordre de réception.
    pub sequence: u64,
}

/// Ce qui se range par [`ordonner`].
pub trait Ordonnable {
    /// Ce que l'ordre lit de cette entrée.
    fn placee(&self) -> Placee;
}

impl Ordonnable for Placee {
    fn placee(&self) -> Placee {
        *self
    }
}

/// Le départage de deux entrées du même instant : un hachage de la graine d'ordre,
/// du pas et de l'origine (§ 4.3, CAT-27).
///
/// FNV-1a 64, puis le mélange final de `SplitMix64` : FNV seul diffuse mal ses
/// derniers octets, et deux rangs voisins se départageraient presque toujours dans
/// le même sens (le test `departage_ne_favorise_aucun_rang` l'a montré). Sans
/// prétention cryptographique : la graine reste sur l'ordinateur de la soirée, et le
/// départage n'a qu'à ne dépendre ni de l'arrivée ni de la valeur d'un `Appareil`.
pub fn departage(graine_ordre: &[u8; 32], pas: Pas, origine: Origine) -> u64 {
    let h = fnv1a64_suite(jeu_abi::fnv1a64(b"partie::ordre"), graine_ordre);
    let h = fnv1a64_suite(h, &pas.numero().to_le_bytes());
    melanger(fnv1a64_suite(h, &origine.code().to_le_bytes()))
}

/// Le mélange final de `SplitMix64` (Steele, Lea et Flood, 2014) : chaque bit de
/// l'entrée change la moitié des bits de la sortie.
///
/// # Propriétés
///
/// C'est une bijection de `u64`, et elle rend la suite publiée de `SplitMix64`
/// semé à zéro quand on lui donne les multiples de `0x9e37_79b9_7f4a_7c15`.
pub const fn melanger(x: u64) -> u64 {
    let x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

/// La clé de l'ordre d'une entrée dans un pas.
fn cle_d_ordre(p: Placee, graine_ordre: &[u8; 32]) -> (Instant, u64, u64) {
    (
        p.fait,
        departage(graine_ordre, p.fait.pas, p.origine),
        p.sequence,
    )
}

/// Range les entrées d'un pas (§ 4.3) : par `fait`, puis à instant égal par
/// [`departage`], puis par numéro d'arrivée.
///
/// Le tri est en place et n'alloue pas. La clé est totale dès que les numéros
/// d'arrivée sont distincts : le résultat ne dépend alors pas de l'ordre initial.
///
/// # Propriétés
///
/// - Le résultat est une permutation de l'entrée : rien n'est perdu ni doublé.
/// - Il est trié par `fait`.
/// - Deux entrées de la même origine gardent leur ordre d'arrivée (avec
///   [`suivre`], qui rend `fait` croissant pour une même origine).
/// - À `fait` égal, l'ordre de deux origines ne dépend que de la graine, du pas et
///   de leurs rangs, jamais d'une valeur d'`Appareil` (CAT-27).
pub fn ordonner<T: Ordonnable>(entrees: &mut [T], graine_ordre: &[u8; 32]) {
    entrees.sort_unstable_by_key(|e| cle_d_ordre(e.placee(), graine_ordre));
}

// ---------------------------------------------------------------------------
// Les tenus (§ 4.3, D17)
// ---------------------------------------------------------------------------

/// Une valeur d'un état tenu, telle que le cœur la manipule sans connaître son type.
pub trait ValeurTenue: Clone + PartialEq {
    /// Cette valeur relâchée, ou `None` si elle ne se relâche jamais seule (un
    /// brouillon).
    fn relachee(&self) -> Option<Self>;
    /// Cette valeur est-elle déjà relâchée ?
    fn est_relachee(&self) -> Oui;
}

/// Les règles d'un état tenu, telles que le cœur les applique.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReglesTenu {
    /// Relâché quand le silence de l'appareil dépasse cette durée (SQ-01) ; `None` :
    /// jamais.
    pub fraicheur: Option<Duree>,
    /// Un état fait plus tôt que cet âge ne compte pas, sauf un relâchement ; la valeur
    /// effective se relâche quand sa dernière redite a dépassé cet âge (SQ-02).
    pub age_max: Option<Duree>,
    /// Changements livrés au plus, par seconde, par clé et par appareil (PUN-03).
    pub debit_max: u16,
}

impl ReglesTenu {
    /// Les règles déclarées par le jeu.
    pub fn depuis_fil(r: &ReglesTenuFil) -> ReglesTenu {
        ReglesTenu {
            fraicheur: applicable(&r.fraicheur),
            age_max: applicable(&r.age_max),
            debit_max: r.debit_max,
        }
    }

    /// Le nombre de pas entre deux livraisons d'un même tenu : 60 / `debit_max`,
    /// arrondi au-dessus. Un débit de 0 se lit 1 par seconde : le plus lent, jamais
    /// aucun, pour qu'un tenu finisse toujours par passer.
    pub fn intervalle(&self) -> u64 {
        PAS_PAR_SECONDE.div_ceil(u64::from(self.debit_max.max(1)))
    }
}

fn applicable(a: &jeu_abi::ApplicableFil<Duree>) -> Option<Duree> {
    match a {
        jeu_abi::ApplicableFil::Oui(d) => Some(*d),
        jeu_abi::ApplicableFil::SansObjet(_) => None,
    }
}

/// Ce que le cœur tient d'un état tenu, par appareil et par clé.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EtatTenu<V> {
    /// La dernière valeur livrée au jeu : la valeur effective.
    pub livre: Option<V>,
    /// Le pas de la dernière livraison, pour le débit.
    pub livre_a: Option<Pas>,
    /// Le plus grand numéro reçu (REM-14).
    pub numero: Option<u64>,
    /// La dernière valeur reçue, retenue par le débit, et l'instant où elle a été faite.
    pub retenue: Option<(V, Instant)>,
    /// L'instant le plus récent où l'appareil a fait la valeur qu'il tient : celui de
    /// sa dernière arrivée acceptée, même inchangée. La valeur effective ne tient que
    /// jusqu'à cet instant plus `age_max` (SQ-02).
    pub fait: Option<Instant>,
}

impl<V> EtatTenu<V> {
    /// Un tenu jamais reçu.
    pub const fn neuf() -> EtatTenu<V> {
        EtatTenu {
            livre: None,
            livre_a: None,
            numero: None,
            retenue: None,
            fait: None,
        }
    }
}

/// Un état tenu qui arrive, déjà validé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArriveeTenu<V> {
    /// La valeur complète.
    pub valeur: V,
    /// Son numéro, qui ne fait que monter, même à travers les reconnexions (REM-14).
    pub numero: u64,
    /// L'instant borné où elle a été faite.
    pub fait: Instant,
}

/// Une valeur à livrer au jeu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Livraison<V> {
    /// La valeur.
    pub valeur: V,
    /// Pourquoi elle change.
    pub cause: CauseTenu,
    /// L'instant où elle a été faite : celui de l'arrivée, ou le pas du cœur pour un
    /// relâchement qu'il synthétise.
    pub fait: Instant,
}

/// Ce que [`tenu`] fait d'une arrivée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueTenu<V> {
    /// Livrée maintenant.
    Livree(Livraison<V>),
    /// Retenue par le débit : elle partira à un pas suivant, sauf si une plus
    /// récente la remplace (le dernier gagne).
    Retenue,
    /// La valeur effective ne change pas.
    Inchangee,
    /// Un numéro qui n'est pas plus grand que le dernier reçu (REM-14).
    Ancienne,
    /// Faite plus tôt que `age_max` (SQ-02, CAT-25).
    TropVieille,
}

impl<V> IssueTenu<V> {
    /// La livraison, s'il y en a une.
    pub fn livraison(self) -> Option<Livraison<V>> {
        match self {
            IssueTenu::Livree(l) => Some(l),
            IssueTenu::Retenue
            | IssueTenu::Inchangee
            | IssueTenu::Ancienne
            | IssueTenu::TropVieille => None,
        }
    }
}

/// Un tenu arrive, au pas `pas` (§ 4.3 ; REM-13, REM-14, SQ-02, PUN-03).
///
/// 1. Un numéro qui n'est pas plus grand que le dernier reçu est écarté.
/// 2. Une valeur faite plus tôt que `age_max` est écartée, sauf un relâchement,
///    qui passe toujours. Ce qui était retenu, plus vieux encore, est jeté aussi.
/// 3. Une valeur égale à la valeur effective ne livre rien, et jette ce qui était
///    retenu : le dernier gagne.
/// 4. Un relâchement passe sans attendre le débit. Sinon la valeur passe si
///    l'intervalle du débit s'est écoulé depuis la dernière livraison ; elle est
///    retenue dans le cas contraire, et remplace ce qui l'était.
///
/// # Propriétés
///
/// - Le numéro tenu ne fait que monter.
/// - Deux livraisons non relâchées d'un même tenu sont séparées d'au moins
///   [`ReglesTenu::intervalle`] pas.
/// - Rien n'est livré qui soit égal à la valeur effective.
/// - Après une arrivée, la valeur effective ou la valeur retenue est la dernière
///   arrivée acceptée : un état périmé est remplacé par le plus récent.
/// - Un relâchement plus récent que le dernier numéro est toujours livré, s'il
///   change la valeur effective.
pub fn tenu<V: ValeurTenue>(
    etat: &mut EtatTenu<V>,
    arrivee: ArriveeTenu<V>,
    regles: &ReglesTenu,
    pas: Pas,
) -> IssueTenu<V> {
    if etat.numero.is_some_and(|n| arrivee.numero <= n) {
        return IssueTenu::Ancienne;
    }
    etat.numero = Some(arrivee.numero);
    let relache = arrivee.valeur.est_relachee();
    if relache == Oui::Non && trop_vieille(arrivee.fait, regles, pas) {
        etat.retenue = None;
        return IssueTenu::TropVieille;
    }
    etat.fait = Some(etat.fait.map_or(arrivee.fait, |f| f.max(arrivee.fait)));
    if etat.livre.as_ref() == Some(&arrivee.valeur) {
        etat.retenue = None;
        return IssueTenu::Inchangee;
    }
    if relache == Oui::Oui || debit_libre(etat, regles, pas) {
        IssueTenu::Livree(livrer(
            etat,
            arrivee.valeur,
            CauseTenu::Geste,
            arrivee.fait,
            pas,
        ))
    } else {
        etat.retenue = Some((arrivee.valeur, arrivee.fait));
        IssueTenu::Retenue
    }
}

/// Faite plus tôt que l'âge permis au pas `pas` ? Un âge égal à `age_max` compte
/// encore (comme la fraîcheur, B6 des jeux).
fn trop_vieille(fait: Instant, regles: &ReglesTenu, pas: Pas) -> bool {
    regles
        .age_max
        .is_some_and(|a| pas.numero().saturating_sub(fait.pas.numero()) > a.en_pas())
}

/// L'intervalle du débit s'est-il écoulé depuis la dernière livraison ?
fn debit_libre<V>(etat: &EtatTenu<V>, regles: &ReglesTenu, pas: Pas) -> bool {
    etat.livre_a
        .is_none_or(|a| pas.numero().saturating_sub(a.numero()) >= regles.intervalle())
}

fn livrer<V: Clone>(
    etat: &mut EtatTenu<V>,
    valeur: V,
    cause: CauseTenu,
    fait: Instant,
    pas: Pas,
) -> Livraison<V> {
    etat.livre = Some(valeur.clone());
    etat.livre_a = Some(pas);
    etat.retenue = None;
    Livraison {
        valeur,
        cause,
        fait,
    }
}

/// Ce qu'un tenu livre de lui-même au début du pas `pas` : son relâchement si
/// l'appareil s'est tu depuis plus que `fraicheur` (SQ-01), ou si la valeur qu'il
/// tient a été faite il y a plus que `age_max` (SQ-02), cause `Silence` ; sinon la
/// valeur retenue si le débit le permet et qu'elle n'est pas trop vieille.
///
/// `derniere_nouvelle` est le dernier pas où l'appareil a donné signe de vie,
/// pour n'importe laquelle de ses clés. Les deux bornes sont strictes : à 450 ms
/// pile (27 pas) de silence, une direction tient encore, et se relâche au 28ᵉ pas,
/// comme dans le Sidequest d'origine (besoin 10 des jeux, B6 de Sidequest). Une
/// valeur faite en retard, mais plus jeune que `age_max`, ne tient que jusqu'à cet
/// âge : l'appareil qui la tient vraiment la redit (SQ-01, toutes les 150 ms), et
/// chaque redite repousse cette borne (besoin 9 des jeux, B4 de Sidequest).
///
/// # Propriétés
///
/// - Un relâchement par silence n'est livré que si `pas − derniere_nouvelle >
///   fraicheur` ou `pas − fait > age_max`, que la valeur effective n'est pas
///   relâchée et qu'elle se relâche.
/// - Après lui, plus rien n'est livré par silence tant qu'une nouvelle valeur
///   n'a pas été livrée.
/// - Une valeur retenue n'est livrée qu'une fois, et pas avant l'intervalle du débit.
pub fn a_ce_pas<V: ValeurTenue>(
    etat: &mut EtatTenu<V>,
    regles: &ReglesTenu,
    pas: Pas,
    derniere_nouvelle: Pas,
) -> Option<Livraison<V>> {
    let silence = regles
        .fraicheur
        .is_some_and(|f| pas.numero().saturating_sub(derniere_nouvelle.numero()) > f.en_pas());
    let perimee = etat.fait.is_some_and(|f| trop_vieille(f, regles, pas));
    if (silence || perimee)
        && let Some(l) = relacher(etat, CauseTenu::Silence, pas)
    {
        return Some(l);
    }
    if !debit_libre(etat, regles, pas) {
        return None;
    }
    let (valeur, fait) = etat.retenue.take()?;
    if trop_vieille(fait, regles, pas) {
        return None;
    }
    Some(livrer(etat, valeur, CauseTenu::Geste, fait, pas))
}

/// Le relâchement d'un tenu, pour une cause du commun : silence, départ, arrière-plan
/// ou pause (§ 4.1, § 4.3). Ce qui était retenu est jeté.
///
/// # Propriétés
///
/// - Rien n'est livré si la valeur effective est absente, déjà relâchée, ou ne se
///   relâche pas seule.
/// - Sinon la valeur effective devient sa valeur relâchée, et le numéro reste : un
///   état plus ancien ne la remplace pas ensuite (REM-14).
pub fn relacher<V: ValeurTenue>(
    etat: &mut EtatTenu<V>,
    cause: CauseTenu,
    pas: Pas,
) -> Option<Livraison<V>> {
    let actuelle = etat.livre.as_ref()?;
    if actuelle.est_relachee() == Oui::Oui {
        return None;
    }
    let r = actuelle.relachee()?;
    Some(livrer(etat, r, cause, Instant::de(pas), pas))
}

// ---------------------------------------------------------------------------
// Le temps (§ 4.1, § 4.4)
// ---------------------------------------------------------------------------

/// Jusqu'où avancer, et combien de temps jeter (§ 4.1, § 4.4 ; CAT-15, SQ-62, D14).
///
/// Rend `(cible, saut)`. Le cœur joue les pas jusqu'à `cible`, puis le numéro de
/// pas saute de `saut` : les pas sautés ne sont pas joués, et le premier `Temps`
/// qui suit porte la durée jetée. `rattrapage` vaut au moins 1 : un pas avance
/// toujours.
///
/// # Propriétés
///
/// - `tete ≤ cible ≤ tete + max(rattrapage, 1)` : la boucle ne s'emballe jamais.
/// - `cible + saut = max(voulu, tete)` : après le saut, le numéro de pas suit
///   l'horloge, et les appareils partagent le même numéro.
/// - `saut = 0` dès que `voulu − tete ≤ max(rattrapage, 1)` : sous le plafond,
///   découper le même temps autrement donne les mêmes pas (CAT-14).
/// - Rien ne recule : `voulu ≤ tete` rend `(tete, 0)`.
pub fn cible(definitif: Pas, tete: Pas, voulu: Pas, rattrapage: u8) -> (Pas, Duree) {
    let tete = tete.max(definitif);
    let plafond = u64::from(rattrapage.max(1));
    let retard = voulu.numero().saturating_sub(tete.numero());
    if retard <= plafond {
        return (tete.max(voulu), Duree::ZERO);
    }
    let cible = tete.numero().saturating_add(plafond);
    (
        Pas::du_fil(cible),
        Duree::pas(retard.saturating_sub(plafond)),
    )
}

/// Le réveil demande-t-il un `Temps` au pas `pas` (§ 2.4, § 4.1) ?
///
/// # Propriétés
///
/// `ChaquePas` le demande toujours, `Jamais` jamais, `A(e)` dès que `pas ≥ e`.
pub const fn reveil_demande(reveil: ReveilFil, pas: Pas) -> Oui {
    let oui = match reveil {
        ReveilFil::ChaquePas => true,
        ReveilFil::A(e) => pas.numero() >= e,
        ReveilFil::Jamais => false,
    };
    if oui { Oui::Oui } else { Oui::Non }
}

// ---------------------------------------------------------------------------
// Les demandes (§ 2.10)
// ---------------------------------------------------------------------------

/// Les demandes à lancer : listées, pas encore en cours, chacune une fois même si
/// la liste la répète (TL-44 à TL-46, SAL-164).
///
/// N'alloue pas. Le cœur en retire celles qu'il a déjà lancées une fois : une
/// demande ne part qu'une fois par identité, même à travers une reprise (CAT-76).
///
/// # Propriétés
///
/// Rend exactement les éléments de `listees` absents de `en_cours`, dans l'ordre
/// de leur première apparition, sans doublon.
pub fn a_lancer<'a>(en_cours: &'a [u64], listees: &'a [u64]) -> impl Iterator<Item = u64> + 'a {
    listees
        .iter()
        .enumerate()
        .filter(move |&(i, id)| {
            !en_cours.contains(id) && !listees.get(..i).is_some_and(|avant| avant.contains(id))
        })
        .map(|(_, id)| *id)
}

/// Les demandes à annuler : en cours, et que le jeu ne liste plus. Leur réponse,
/// si elle arrive, est jetée avant d'atteindre le jeu (TL-45).
///
/// # Propriétés
///
/// Rend exactement les éléments de `en_cours` absents de `listees`, dans leur ordre.
pub fn a_annuler<'a>(en_cours: &'a [u64], listees: &'a [u64]) -> impl Iterator<Item = u64> + 'a {
    en_cours.iter().filter(|id| !listees.contains(id)).copied()
}

/// Les demandes à lancer et à annuler, rassemblées : [`a_lancer`] puis [`a_annuler`].
pub fn demandes(en_cours: &[u64], listees: &[u64]) -> (Vec<u64>, Vec<u64>) {
    (
        a_lancer(en_cours, listees).collect(),
        a_annuler(en_cours, listees).collect(),
    )
}

// ---------------------------------------------------------------------------
// Les vues (§ 4.4)
// ---------------------------------------------------------------------------

/// La scène de l'écran quand la tête est en avance sur la définitive (§ 4.4).
///
/// La scène de la tête, dont chaque sous-arbre `Definitif` est remplacé par le
/// sous-arbre de même chemin de clés dans la définitive, ou vidé s'il n'y est pas.
/// Les effets `Provisoire` viennent de la tête, les `Definitive` de la définitive.
///
/// # Propriétés
///
/// - La fusion se fait par clé, jamais par position (CAT-41) : déplacer un
///   sous-arbre définitif parmi ses frères ne change pas ce qu'il montre.
/// - Rien de la tête ne paraît sous un nœud `Definitif`.
/// - `fusionner(s, s)` rend le même arbre que `s` : mêmes nœuds, mêmes parents,
///   même ordre entre frères. Dans la liste à plat, le sous-arbre greffé suit
///   aussitôt son nœud `Definitif` ; les effets vont provisoires, puis définitifs.
/// - Le résultat est bien formé : chaque parent précède ses enfants.
pub fn fusionner(tete: &SceneFil, definitive: &SceneFil) -> SceneFil {
    let mut noeuds: Vec<jeu_abi::NoeudFil> = Vec::new();
    // Pour chaque nœud de la tête, son indice dans le résultat, s'il y est.
    let mut place_tete: Vec<Option<u32>> = Vec::new();
    let mut dans_un_definitif: Vec<bool> = Vec::new();
    for n in &tete.noeuds {
        let parent_exclu = n
            .parent
            .and_then(|p| dans_un_definitif.get(usize::try_from(p).ok()?).copied())
            .unwrap_or(false);
        let parent_definitif = n
            .parent
            .and_then(|p| tete.noeuds.get(usize::try_from(p).ok()?))
            .is_some_and(|p| matches!(p.sorte, SorteNoeud::Definitif));
        let exclu = parent_exclu || parent_definitif;
        dans_un_definitif.push(exclu);
        if exclu {
            place_tete.push(None);
            continue;
        }
        let ici = u32::try_from(noeuds.len()).ok();
        let mut copie = n.clone();
        copie.parent = n
            .parent
            .and_then(|p| place_tete.get(usize::try_from(p).ok()?).copied().flatten());
        noeuds.push(copie);
        place_tete.push(ici);
        if matches!(n.sorte, SorteNoeud::Definitif)
            && let (Some(ici), Some(source)) = (ici, chercher(definitive, &chemin(tete, n)))
        {
            greffer(definitive, source, ici, &mut noeuds);
        }
    }
    let effets = tete
        .effets
        .iter()
        .filter(|e| e.tenue == Tenue::Provisoire)
        .chain(
            definitive
                .effets
                .iter()
                .filter(|e| e.tenue == Tenue::Definitive),
        )
        .copied()
        .collect::<Vec<Effet>>();
    SceneFil {
        fond: tete.fond,
        noeuds,
        raccourcis: tete.raccourcis.clone(),
        clavier_brut: tete.clavier_brut,
        effets,
    }
}

/// Le chemin de clés d'un nœud, de la racine à lui.
fn chemin(scene: &SceneFil, n: &jeu_abi::NoeudFil) -> Vec<jeu_abi::Cle> {
    let mut c = Vec::new();
    let mut ici = Some(n);
    // Chaque parent précède son enfant : la remontée termine en au plus `len` pas.
    let mut garde = scene.noeuds.len();
    while let Some(m) = ici {
        c.push(m.cle);
        ici = m
            .parent
            .and_then(|p| scene.noeuds.get(usize::try_from(p).ok()?));
        garde = garde.saturating_sub(1);
        if garde == 0 {
            break;
        }
    }
    c.reverse();
    c
}

/// L'indice du nœud `Definitif` de ce chemin dans la scène, s'il existe.
fn chercher(scene: &SceneFil, voulu: &[jeu_abi::Cle]) -> Option<usize> {
    scene
        .noeuds
        .iter()
        .position(|n| matches!(n.sorte, SorteNoeud::Definitif) && chemin(scene, n) == voulu)
}

/// Copie les descendants du nœud `source` de `scene` sous le nœud `cible` du résultat.
fn greffer(scene: &SceneFil, source: usize, cible: u32, noeuds: &mut Vec<jeu_abi::NoeudFil>) {
    // Pour chaque nœud de `scene`, son indice dans le résultat s'il descend de `source`.
    let mut place: Vec<Option<u32>> = Vec::new();
    for n in &scene.noeuds {
        let parent = n.parent.and_then(|p| usize::try_from(p).ok());
        let nouveau_parent = match parent {
            Some(p) if p == source => Some(cible),
            Some(p) => place.get(p).copied().flatten(),
            None => None,
        };
        // Chaque parent précède ses enfants : seuls des nœuds après `source` ont un
        // nouveau parent.
        if let Some(np) = nouveau_parent {
            let ici = u32::try_from(noeuds.len()).ok();
            let mut copie = n.clone();
            copie.parent = Some(np);
            noeuds.push(copie);
            place.push(ici);
        } else {
            place.push(None);
        }
    }
}

// ---------------------------------------------------------------------------
// Valider ce qui s'affiche (§ 6.1, D26)
// ---------------------------------------------------------------------------

/// La police que le commun fournit à tout jeu, sans asset déclaré :
/// [`Cle::POLICE_DU_COMMUN`], définie dans `jeu-abi`, que la colle et les jeux nomment
/// aussi.
pub const POLICE_DU_COMMUN: Cle = Cle::POLICE_DU_COMMUN;

/// Le plus de nœuds d'une scène. Un point de départ, non mesuré : la scène de Tier
/// list, la plus chargée des trois jeux, en compte quelques centaines.
pub const NOEUDS_MAX: usize = 4096;

/// La plus grande profondeur d'un nœud : la racine est à 1. Un point de départ, non
/// mesuré.
pub const PROFONDEUR_MAX: usize = 32;

/// La marge autour du repère, en pour cent de chaque côté de sa largeur et de sa
/// hauteur : un nœud peut en sortir pour glisser vers l'écran, pas au-delà. Un point
/// de départ, non mesuré.
pub const MARGE_POUR_CENT: i64 = 50;

/// Pourquoi une scène ne s'affiche pas (§ 6.1). Elle est alors une faute du jeu, et
/// la scène précédente reste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefautScene {
    /// Plus de [`NOEUDS_MAX`] nœuds.
    TropDeNoeuds {
        /// Combien.
        noeuds: usize,
    },
    /// Un nœud plus profond que [`PROFONDEUR_MAX`].
    TropProfonde {
        /// Sa clé.
        cle: Cle,
    },
    /// Deux frères ont la même clé : la fusion et le glisser, qui passent par elle,
    /// les confondraient (SAL-165, CAT-41).
    CleEnDouble {
        /// La clé.
        cle: Cle,
    },
    /// Une largeur ou une hauteur négative.
    TailleNegative {
        /// La clé du nœud.
        cle: Cle,
    },
    /// Un nœud hors du repère et de sa marge ([`MARGE_POUR_CENT`]).
    HorsDuRepere {
        /// La clé du nœud.
        cle: Cle,
    },
    /// Une police qui n'est ni celle du commun, ni un asset déclaré.
    PoliceInconnue {
        /// La clé du nœud.
        cle: Cle,
        /// La police.
        police: Cle,
    },
    /// Une image que ni les assets, ni le jeu ne connaissent.
    ImageInconnue {
        /// La clé du nœud.
        cle: Cle,
        /// L'image.
        image: Cle,
    },
}

impl core::fmt::Display for DefautScene {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DefautScene::TropDeNoeuds { noeuds } => {
                write!(f, "{noeuds} nœuds, au-delà de {NOEUDS_MAX}")
            }
            DefautScene::TropProfonde { cle } => write!(
                f,
                "le nœud {:#x} est plus profond que {PROFONDEUR_MAX}",
                cle.fil()
            ),
            DefautScene::CleEnDouble { cle } => {
                write!(f, "deux frères ont la clé {:#x}", cle.fil())
            }
            DefautScene::TailleNegative { cle } => {
                write!(f, "le nœud {:#x} a une taille négative", cle.fil())
            }
            DefautScene::HorsDuRepere { cle } => {
                write!(f, "le nœud {:#x} sort du repère et de sa marge", cle.fil())
            }
            DefautScene::PoliceInconnue { cle, police } => write!(
                f,
                "le nœud {:#x} écrit avec une police inconnue {:#x}",
                cle.fil(),
                police.fil()
            ),
            DefautScene::ImageInconnue { cle, image } => write!(
                f,
                "le nœud {:#x} montre une image inconnue {:#x}",
                cle.fil(),
                image.fil()
            ),
        }
    }
}

/// Les bornes du repère, marge comprise, en 1/256 d'unité : `(x min, x max, y min,
/// y max)`. Une page n'a pas de bas.
fn bornes_du_repere(ecran: FormeEcran) -> (i64, i64, i64, i64) {
    let unite = i64::from(Fixe::PAR_UNITE);
    let (largeur, hauteur) = match ecran {
        FormeEcran::Scene {
            largeur, hauteur, ..
        } => (i64::from(largeur), Some(i64::from(hauteur))),
        FormeEcran::Page { largeur_max } => (i64::from(largeur_max), None),
    };
    let l = largeur.saturating_mul(unite);
    let mx = l.saturating_mul(MARGE_POUR_CENT) / 100;
    match hauteur {
        Some(h) => {
            let h = h.saturating_mul(unite);
            let my = h.saturating_mul(MARGE_POUR_CENT) / 100;
            (
                0i64.saturating_sub(mx),
                l.saturating_add(mx),
                0i64.saturating_sub(my),
                h.saturating_add(my),
            )
        }
        None => (
            0i64.saturating_sub(mx),
            l.saturating_add(mx),
            0i64.saturating_sub(mx),
            i64::MAX,
        ),
    }
}

/// Le rectangle d'un nœud, s'il en a un.
const fn rect_du_noeud(s: &SorteNoeud) -> Option<Rect> {
    match s {
        SorteNoeud::Rect { r, .. } | SorteNoeud::Image { r, .. } | SorteNoeud::Surface { r } => {
            Some(*r)
        }
        SorteNoeud::Texte { boite, .. }
        | SorteNoeud::Bouton { boite, .. }
        | SorteNoeud::CompteARebours { boite, .. }
        | SorteNoeud::Qr { boite }
        | SorteNoeud::Avertissements { boite } => Some(*boite),
        SorteNoeud::Groupe { .. } | SorteNoeud::Composant(_) | SorteNoeud::Definitif => None,
    }
}

/// Valide une scène de l'écran comme une entrée venue de l'extérieur (§ 6.1, D26) :
/// nombre de nœuds et profondeur bornés, clés uniques parmi les frères, tailles
/// positives, chaque nœud dans le repère et sa marge (décalages des groupes
/// compris), polices et images connues.
///
/// `polices` et `images` sont les clés connues en plus de [`POLICE_DU_COMMUN`] : les
/// assets déclarés, et les images que le jeu a dit connaître (`Ressources`).
///
/// La taille encodée et le nombre d'effets se vérifient avant, sur les octets, par
/// le cœur (`limites.vue`, `limites.effets`). Les effets `Definitive` d'une scène
/// de la tête ne s'affichent pas : la fusion ([`fusionner`]) prend ceux de la
/// définitive.
///
/// # Propriétés
///
/// - Une scène vide est valide.
/// - Le résultat ne dépend pas de l'ordre des frères, sauf pour dire lequel de deux
///   défauts est nommé.
/// - Une scène valide le reste si l'on retire une feuille.
pub fn valider_scene(
    scene: &SceneFil,
    ecran: &FormeEcran,
    polices: &[Cle],
    images: &[Cle],
) -> Result<(), DefautScene> {
    let nombre = scene.noeuds.len();
    if nombre > NOEUDS_MAX {
        return Err(DefautScene::TropDeNoeuds { noeuds: nombre });
    }
    let (x_min, x_max, y_min, y_max) = bornes_du_repere(*ecran);
    // Pour chaque nœud : sa profondeur et le décalage de ses enfants.
    let mut profondeur: Vec<usize> = Vec::with_capacity(nombre);
    let mut decalage: Vec<(i64, i64)> = Vec::with_capacity(nombre);
    // Les clés vues, par parent : (parent, clé), triées pour chercher vite.
    let mut freres: Vec<(Option<u32>, u64)> = Vec::with_capacity(nombre);
    for noeud in &scene.noeuds {
        let parent = noeud.parent.and_then(|p| usize::try_from(p).ok());
        let (p_prof, (dx, dy)) = parent
            .and_then(|p| Some((*profondeur.get(p)?, *decalage.get(p)?)))
            .unwrap_or((0, (0, 0)));
        let ici = p_prof.saturating_add(1);
        if ici > PROFONDEUR_MAX {
            return Err(DefautScene::TropProfonde { cle: noeud.cle });
        }
        let cle = (noeud.parent, noeud.cle.fil());
        match freres.binary_search(&cle) {
            Ok(_) => return Err(DefautScene::CleEnDouble { cle: noeud.cle }),
            Err(i) => freres.insert(i, cle),
        }
        let enfants = match noeud.sorte {
            SorteNoeud::Groupe { decalage: (gx, gy) } => (
                dx.saturating_add(i64::from(gx.brut())),
                dy.saturating_add(i64::from(gy.brut())),
            ),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant(_)
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => (dx, dy),
        };
        profondeur.push(ici);
        decalage.push(enfants);
        if let Some(r) = rect_du_noeud(&noeud.sorte) {
            let (largeur, hauteur) = (i64::from(r.l.brut()), i64::from(r.h.brut()));
            if largeur < 0 || hauteur < 0 {
                return Err(DefautScene::TailleNegative { cle: noeud.cle });
            }
            let gauche = dx.saturating_add(i64::from(r.x.brut()));
            let haut = dy.saturating_add(i64::from(r.y.brut()));
            if gauche < x_min
                || haut < y_min
                || gauche.saturating_add(largeur) > x_max
                || haut.saturating_add(hauteur) > y_max
            {
                return Err(DefautScene::HorsDuRepere { cle: noeud.cle });
            }
        }
        let police = match &noeud.sorte {
            SorteNoeud::Texte { style, .. } | SorteNoeud::CompteARebours { style, .. } => {
                Some(style.police)
            }
            SorteNoeud::Composant(ComposantVuFil::EnPlace { style, .. }) => Some(style.police),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant(
                ComposantVuFil::Champ { .. }
                | ComposantVuFil::Deplacables { .. }
                | ComposantVuFil::Selecteur { .. }
                | ComposantVuFil::Modale { .. }
                | ComposantVuFil::Lien { .. },
            )
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        };
        if let Some(p) = police
            && p != POLICE_DU_COMMUN
            && !polices.contains(&p)
        {
            return Err(DefautScene::PoliceInconnue {
                cle: noeud.cle,
                police: p,
            });
        }
        if let SorteNoeud::Image { image, .. } = noeud.sorte
            && !images.contains(&image)
        {
            return Err(DefautScene::ImageInconnue {
                cle: noeud.cle,
                image,
            });
        }
    }
    Ok(())
}

/// Les images qu'une scène cite, chacune une fois.
pub fn images_citees(scene: &SceneFil) -> Vec<Cle> {
    let mut v: Vec<Cle> = Vec::new();
    for n in &scene.noeuds {
        if let SorteNoeud::Image { image, .. } = n.sorte
            && !v.contains(&image)
        {
            v.push(image);
        }
    }
    v
}

// ---------------------------------------------------------------------------
// Ce que l'écran peut envoyer (§ 2.6, SAL-152)
// ---------------------------------------------------------------------------

/// Une entrée de l'écran est-elle permise par la dernière scène envoyée (SAL-152) ?
///
/// - une action doit être celle d'un bouton, d'un raccourci, de la fin d'un champ
///   ou de la fermeture d'une modale de la scène ;
/// - une touche brute n'arrive que si la scène a demandé le clavier brut, et une
///   répétition seulement si elle l'a demandée aussi ;
/// - un pointeur brut demande une surface ;
/// - une saisie vise un champ de la scène et tient dans sa limite, comptée dans
///   son unité ;
/// - un objet déplacé et sa zone sont ceux d'un composant de la scène ;
/// - des fichiers demandent un sélecteur, un seul s'il n'est pas multiple, et
///   chacun tient dans sa taille.
///
/// # Propriétés
///
/// Rien que la scène ne porte ne passe : une scène vide n'admet aucune entrée.
pub fn admise_par_la_scene(scene: &SceneFil, quoi: &EcranArrivee) -> Oui {
    let oui = match quoi {
        EcranArrivee::Action(a) => action_presente(scene, a),
        EcranArrivee::Touche(t) => match scene.clavier_brut {
            jeu_abi::ClavierBrut::Non => false,
            jeu_abi::ClavierBrut::Oui { repetition } => {
                t.repetition == Oui::Non || repetition == Oui::Oui
            }
        },
        EcranArrivee::Pointeur(_) => scene
            .noeuds
            .iter()
            .any(|n| matches!(n.sorte, SorteNoeud::Surface { .. })),
        EcranArrivee::Composant(c) => composant_admis(scene, c),
    };
    if oui { Oui::Oui } else { Oui::Non }
}

fn action_presente(scene: &SceneFil, a: &[u8]) -> bool {
    scene.raccourcis.iter().any(|r| r.action == a)
        || scene.noeuds.iter().any(|n| match &n.sorte {
            SorteNoeud::Bouton { action, .. } => action == a,
            SorteNoeud::Composant(ComposantVuFil::Champ { fin: Some(f), .. }) => f == a,
            SorteNoeud::Composant(ComposantVuFil::Modale { fermer }) => fermer == a,
            SorteNoeud::Rect { .. }
            | SorteNoeud::Texte { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant(
                ComposantVuFil::Champ { fin: None, .. }
                | ComposantVuFil::EnPlace { .. }
                | ComposantVuFil::Deplacables { .. }
                | ComposantVuFil::Selecteur { .. }
                | ComposantVuFil::Lien { .. },
            )
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => false,
        })
}

fn composants(scene: &SceneFil) -> impl Iterator<Item = (jeu_abi::Cle, &ComposantVuFil)> {
    scene.noeuds.iter().filter_map(|n| match &n.sorte {
        SorteNoeud::Composant(c) => Some((n.cle, c)),
        SorteNoeud::Rect { .. }
        | SorteNoeud::Texte { .. }
        | SorteNoeud::Image { .. }
        | SorteNoeud::Groupe { .. }
        | SorteNoeud::Bouton { .. }
        | SorteNoeud::CompteARebours { .. }
        | SorteNoeud::Qr { .. }
        | SorteNoeud::Avertissements { .. }
        | SorteNoeud::Surface { .. }
        | SorteNoeud::Definitif => None,
    })
}

fn composant_admis(scene: &SceneFil, c: &ComposantArrivee) -> bool {
    match c {
        ComposantArrivee::Saisie { champ, texte, .. } => {
            composants(scene).any(|(cle, vu)| match vu {
                ComposantVuFil::Champ { limite, unite, .. }
                | ComposantVuFil::EnPlace { limite, unite, .. } => {
                    cle == *champ && longueur(texte, *unite) <= u64::from(*limite)
                }
                ComposantVuFil::Deplacables { .. }
                | ComposantVuFil::Selecteur { .. }
                | ComposantVuFil::Modale { .. }
                | ComposantVuFil::Lien { .. } => false,
            })
        }
        ComposantArrivee::Deplace(d) => composants(scene).any(|(_, vu)| match vu {
            ComposantVuFil::Deplacables {
                zones, poubelle, ..
            } => deplace_admis(d, zones, *poubelle),
            ComposantVuFil::Champ { .. }
            | ComposantVuFil::EnPlace { .. }
            | ComposantVuFil::Selecteur { .. }
            | ComposantVuFil::Modale { .. }
            | ComposantVuFil::Lien { .. } => false,
        }),
        ComposantArrivee::Fichiers { fichiers, .. } => composants(scene).any(|(_, vu)| match vu {
            ComposantVuFil::Selecteur {
                multiple,
                taille_max,
                ..
            } => {
                (*multiple == Oui::Oui || fichiers.len() <= 1)
                    && fichiers.iter().all(|f| f.taille <= *taille_max)
            }
            ComposantVuFil::Champ { .. }
            | ComposantVuFil::EnPlace { .. }
            | ComposantVuFil::Deplacables { .. }
            | ComposantVuFil::Modale { .. }
            | ComposantVuFil::Lien { .. } => false,
        }),
    }
}

fn deplace_admis(d: &Deplace, zones: &[jeu_abi::ZoneVu], poubelle: Option<jeu_abi::Cle>) -> bool {
    let objet_existe = |o: jeu_abi::Cle| zones.iter().any(|z| z.objets.contains(&o));
    let zone_existe = |c: jeu_abi::Cle| zones.iter().any(|z| z.cle == c) || poubelle == Some(c);
    match d {
        Deplace::Survol { objet, zone, .. } | Deplace::Depose { objet, zone, .. } => {
            objet_existe(*objet) && zone_existe(*zone)
        }
        Deplace::Annule { objet } => objet_existe(*objet),
    }
}

/// La longueur d'un texte dans une unité (§ 2.3, TL-27).
pub fn longueur(texte: &str, unite: Unite) -> u64 {
    let n = match unite {
        Unite::Capitales => texte.chars().flat_map(char::to_uppercase).count(),
        Unite::Utf16 => texte.encode_utf16().count(),
        Unite::PointsDeCode => texte.chars().count(),
        Unite::Graphemes => texte.graphemes(true).count(),
    };
    u64::try_from(n).unwrap_or(u64::MAX)
}
