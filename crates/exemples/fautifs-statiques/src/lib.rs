//! Les cinq jeux fautifs dont la faute tient dans un `static` (INTERFACE.md, § 2.2,
//! § 7.2 et § 8.8) : le même socle que les autres fautifs ([`fautifs::Socle`]), à un
//! défaut près, mais dans une crate à eux.
//!
//! | Défaut | Ce qu'il change | Contrôle visé |
//! |---|---|---|
//! | [`RefuseApresAvoirEcrit`] | un `static` écrit avant chaque refus, montré à l'écran | E6 (et E8, qui voit le `static` à l'écran) |
//! | [`VueQuiEcrit`] | la vue de l'écran écrit un `static` | E7 |
//! | [`EtatHorsEnveloppe`] | le numéro de manche tenu dans un `static` | E14 (copie native) |
//! | [`RetientUnRefus`] | un `static` écrit avant un refus, lu à la tape suivante du même pas | E6 |
//! | [`SecretDansUnStatic`] | le mot tiré recopié dans un `static`, que l'écran montre pendant la manche | E8 |
//!
//! # Pourquoi une crate à part
//!
//! La règle « rien hors de l'état » de l'étape 1 (CONTRAT, « L'épreuve » ; INTERFACE,
//! § 2.2) lit chaque crate du jeu en entier. Tant que ces cinq `static` vivaient dans
//! `fautifs`, que toutes les crates de module des fautifs partagent, chaque fautif
//! échouait l'étape 1 depuis ses sources, `sain` et `avec-fenetre` compris
//! (RESTE-PHASE-2, point 1) : un contrôle négatif ne faisait plus échouer son contrôle
//! « et lui seul » (CONTRAT ; OUT-27, HIS-03, SAL-168). Ici, ils ne sont lus que par
//! les crates de module de ces cinq fautifs.
//!
//! # Ce qu'ils font échouer
//!
//! Depuis leurs sources, chacun de ces cinq fautifs échoue **deux fois** : l'étape 1,
//! dont la règle « rien hors de l'état » nomme son `static`, et le contrôle qu'il vise.
//! C'est voulu : depuis la phase 2, la forme attrape un `static` avant que la pression
//! ne le cherche (revue de la phase 2, constats 1 et 5). Le contrôle visé, lui, se
//! voit encore dans le rapport, et seul s'il est joué par le module et la copie native
//! sans l'étape 1 (`epreuve/tests/epreuve`, exemple `natif`) : il reste le filet d'un
//! `static` que la lecture des sources manquerait (une crate tierce, une macro).
//!
//! # Les `static`
//!
//! Chaque défaut tient un `static` atomique : c'est sa faute. Dans le module, ce
//! `static` est dans la mémoire, que le moteur copie et restaure avec le reste. Dans
//! la copie native, il est commun au processus : chaque défaut le remet à zéro à la
//! création, pour que la copie native refasse la même histoire que le module tant
//! qu'aucune reprise ne passe par là. Sans fils dans le module, l'atomique s'y compile
//! en lectures et écritures ordinaires : la réécriture n'y voit aucune instruction
//! atomique.

#![no_std]
#![forbid(unsafe_code)]

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use fautifs::defauts::Defaut;
use fautifs::socle::{self, Etape};
use jeu::prelude::*;

/// Les jeux, un par défaut, sous le nom de leur crate de module.
pub mod jeux {
    use fautifs::Socle;

    /// E6.
    pub type RefuseApresAvoirEcrit = Socle<super::RefuseApresAvoirEcrit>;
    /// E7.
    pub type VueQuiEcrit = Socle<super::VueQuiEcrit>;
    /// E14.
    pub type EtatHorsEnveloppe = Socle<super::EtatHorsEnveloppe>;
    /// E6 : un `static` écrit avant un refus, qui change la tape suivante.
    pub type RetientUnRefus = Socle<super::RetientUnRefus>;
    /// E8 : un secret recopié dans un `static`, que l'écran montre.
    pub type SecretDansUnStatic = Socle<super::SecretDansUnStatic>;
}

macro_rules! marqueur {
    ($(#[$doc:meta])* $nom:ident) => {
        $(#[$doc])*
        #[jeu::etat]
        pub struct $nom;
    };
}

/// Les refus comptés par [`RefuseApresAvoirEcrit`].
static REFUS: AtomicU32 = AtomicU32::new(0);

marqueur!(
    /// E6 : un `static` atomique compté avant chaque refus, et montré à l'écran. La
    /// forme canonique ne change pas ; la vue, si (§ 8.8, `refuse_apres_avoir_ecrit`).
    RefuseApresAvoirEcrit
);
impl Defaut for RefuseApresAvoirEcrit {
    type Cas = Etape;
    fn neuf() -> Self {
        RefuseApresAvoirEcrit
    }
    fn a_la_creation(_: Pas) {
        REFUS.store(0, Ordering::Relaxed);
    }
    fn avant_refus(&self, _: Pas) {
        let _: u32 = REFUS.fetch_add(1, Ordering::Relaxed);
    }
    fn compteur_affiche() -> Option<u32> {
        Some(REFUS.load(Ordering::Relaxed))
    }
}

/// La marque que pose la vue de [`VueQuiEcrit`].
static VU: AtomicBool = AtomicBool::new(false);

marqueur!(
    /// E7 : la vue de l'écran écrit un `static`, que chaque entrée efface. Rien ne le
    /// montre ni ne le lit : seule l'empreinte de la mémoire avant et après la lecture
    /// le voit (§ 8.8, `vue_qui_ecrit`).
    VueQuiEcrit
);
impl Defaut for VueQuiEcrit {
    type Cas = Etape;
    fn neuf() -> Self {
        VueQuiEcrit
    }
    fn a_chaque_entree() {
        VU.store(false, Ordering::Relaxed);
    }
    fn en_vue_ecran() {
        // Lue avant d'être écrite : sans lecture, l'optimiseur retirerait le `static`
        // et sa faute avec lui.
        let _: bool = core::hint::black_box(VU.load(Ordering::Relaxed));
        VU.store(true, Ordering::Relaxed);
    }
}

/// Le pas de la création de la partie de [`EtatHorsEnveloppe`].
static CREATION: AtomicU64 = AtomicU64::new(0);

marqueur!(
    /// E14 : le numéro de manche part du pas de la création de la partie, tenu dans un
    /// `static`, hors de l'enveloppe. Le module le copie avec sa mémoire ; la copie
    /// native ne le restaure pas : reprise depuis un point d'une partie plus ancienne
    /// que la dernière, elle numérote ses manches depuis la création de la dernière,
    /// et diverge.
    ///
    /// Le `static` tenait d'abord le nombre de manches ouvertes depuis la création : la
    /// reprise native ne divergeait alors que si la soirée finissait après la première
    /// manche de sa dernière partie, et le plan standard y échappait (RESTE-PHASE-1,
    /// contrôle final). Le pas de la création change à chaque partie : une soirée de
    /// plusieurs parties fait toujours diverger la reprise depuis son premier point.
    EtatHorsEnveloppe
);
impl Defaut for EtatHorsEnveloppe {
    type Cas = Etape;
    fn neuf() -> Self {
        EtatHorsEnveloppe
    }
    fn a_la_creation(pas: Pas) {
        CREATION.store(pas.numero(), Ordering::Relaxed);
    }
    fn numero_de_manche(avant: u32) -> u32 {
        let depuis = u32::try_from(CREATION.load(Ordering::Relaxed)).unwrap_or(u32::MAX);
        depuis.saturating_add(avant).saturating_add(1)
    }
}

/// Le pas et le nombre de tapes du dernier refus de [`RetientUnRefus`].
static DERNIER_REFUS: AtomicU64 = AtomicU64::new(u64::MAX);

/// La clé d'un refus : son pas, et les tapes acceptées avant lui.
fn cle_de_refus(pas: Pas, tapes: u32) -> u64 {
    pas.numero().wrapping_shl(32) | u64::from(tapes)
}

/// E6 : avant chaque refus, un `static` retient son pas et les tapes acceptées
/// jusque-là ; une tape acceptée au même pas, juste après, le lit et laisse une trace
/// dans l'état. Au refus, ni la forme canonique ni les vues ne changent : seul
/// « même avenir » le voit, en rejouant la tape sans le refus (RESTE-PHASE-1,
/// point 11). La clé tient au pas et à l'état : un point de reprise, pris entre deux
/// pas, ne la coupe jamais, et la copie native refait la même histoire.
#[jeu::etat]
pub struct RetientUnRefus {
    /// Les tapes acceptées.
    tapes: u32,
    /// Les tapes acceptées juste après un refus du même pas.
    traces: u32,
}
impl Defaut for RetientUnRefus {
    type Cas = Etape;
    fn neuf() -> Self {
        RetientUnRefus {
            tapes: 0,
            traces: 0,
        }
    }
    fn a_la_creation(_: Pas) {
        DERNIER_REFUS.store(u64::MAX, Ordering::Relaxed);
    }
    fn avant_refus(&self, pas: Pas) {
        DERNIER_REFUS.store(cle_de_refus(pas, self.tapes), Ordering::Relaxed);
    }
    fn a_la_tape(&mut self, pas: Pas) {
        if DERNIER_REFUS.load(Ordering::Relaxed) == cle_de_refus(pas, self.tapes) {
            self.traces = self.traces.saturating_add(1);
        }
        self.tapes = self.tapes.saturating_add(1);
    }
}

/// Le rang du mot de la manche de [`SecretDansUnStatic`], plus un ; 0 avant la
/// première manche.
static MOT: AtomicU32 = AtomicU32::new(0);

marqueur!(
    /// E8 : à l'ouverture de chaque manche, le rang du mot tiré, qui est secret, est
    /// recopié dans un `static` ; l'écran le montre, sans regard. Le `static` n'est pas
    /// dans l'état : la projection ne le vide pas, et une instance qui la recharge
    /// part de sa propre mémoire, où il vaut 0. Avant la correction, E8 partait de la
    /// mémoire de la définitive, et le `static` y passait avec elle (revue de la
    /// phase 2, constat 1 ; INTERFACE, § 2.2). Remis à zéro à la création, pour que
    /// la copie native refasse la même histoire (E9).
    SecretDansUnStatic
);
impl Defaut for SecretDansUnStatic {
    type Cas = Etape;
    fn neuf() -> Self {
        SecretDansUnStatic
    }
    fn a_la_creation(_: Pas) {
        MOT.store(0, Ordering::Relaxed);
    }
    fn au_mot(mot: &str) {
        let rang = socle::MOTS.iter().position(|m| *m == mot).unwrap_or(0);
        MOT.store(
            u32::try_from(rang).unwrap_or(0).saturating_add(1),
            Ordering::Relaxed,
        );
    }
    fn compteur_affiche() -> Option<u32> {
        Some(MOT.load(Ordering::Relaxed))
    }
}
