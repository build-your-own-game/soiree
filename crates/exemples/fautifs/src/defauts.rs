//! Les défauts : chacun change une seule règle du socle, pour faire échouer un seul
//! contrôle de l'épreuve (INTERFACE.md, § 7.2 et § 8.8).
//!
//! | Défaut | Ce qu'il change | Contrôle visé |
//! |---|---|---|
//! | [`Sain`] | rien | aucun |
//! | [`DeclarationIncoherente`] | `limites.memoire` sous la mémoire minimale du module | E1 |
//! | [`JamaisFini`] | après une première fin, les manches ne finissent plus | E2 |
//! | [`PhaseInaccessible`] | la prolongation n'arrive jamais | E3 |
//! | [`InvariantViole`] | deux points par tape | E4 |
//! | [`AttendUnAbsent`] | le salon attend chaque joueur, même parti | E5 |
//! | [`RefuseApresAvoirEcrit`] | un `static` écrit avant chaque refus, montré à l'écran | E6 |
//! | [`VueQuiEcrit`] | la vue de l'écran écrit un `static` | E7 |
//! | [`VueQuiFuit`] | le mot secret copié dans un champ public dès la manche | E8 |
//! | [`NatifDifferent`] | une règle sous `cfg(target_arch = "wasm32")` | E9 |
//! | [`OctetQuiPanique`] | un chiffre hors de 0 à 9 indexe un tableau de dix | E13 |
//! | [`EtatHorsEnveloppe`] | le numéro de manche tenu dans un `static` | E14 (copie native) |
//! | [`FuitLaMemoire`] | chaque appareil arrivé est retenu pour toujours | E15 |
//! | [`DependDeLAppareil`] | à la fin, un point de plus à l'appareil qui s'écrit en premier | E10 |
//! | [`CoupureQuiCompte`] | un joueur qui part, même une seconde, est retiré avec ses points | E11 |
//! | [`AvecFenetre`] | une fenêtre de douze pas, et rien d'autre : il passe tout | aucun |
//! | [`CompteLeRecu`] | une fenêtre, et une tape reçue plus de deux pas après avoir été faite ne compte pas | E12 |
//! | [`PireCasQuiDeborde`] | à 9 999 points, un « Record ! » hors du repère | E16 |
//! | [`NanDansLEtat`] | la moyenne d'un joueur qui n'a pas tapé vaut NaN | E17 |
//! | [`GelQuiEnchaine`] | la prolongation part de l'échéance de la manche, pas de maintenant | E18 |
//! | [`BoucleAuVote`] | chaque chiffre choisi coûte deux millions de tours de boucle | E19 |
//!
//! # Les `static` des défauts
//!
//! Trois défauts tiennent un `static` atomique : c'est leur faute. Dans le module, ce
//! `static` est dans la mémoire, que le moteur copie et restaure avec le reste. Dans
//! la copie native, il est commun au processus : chaque défaut le remet à zéro à la
//! création, pour que la copie native refasse la même histoire que le module tant
//! qu'aucune reprise ne passe par là. Sans fils dans le module, l'atomique s'y compile
//! en lectures et écritures ordinaires : la réécriture n'y voit aucune instruction
//! atomique.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use jeu::prelude::*;

use crate::socle::{self, VALEURS};

/// Ce qu'un défaut peut changer au socle. Chaque méthode a la valeur du jeu sain.
pub trait Defaut: Etat {
    /// La déclaration.
    const DECLARATION: Declaration = socle::DECLARATION;

    /// Le marqueur.
    fn neuf() -> Self;

    /// Appelée au début de chaque `creer`.
    fn a_la_creation() {}

    /// Les mots que la manche tire.
    fn mots() -> &'static [&'static str] {
        &socle::MOTS
    }

    /// La durée de la prolongation.
    fn prolongation() -> Duree {
        socle::PROLONGATION
    }

    /// Le salon dit-il en plus que rien n'avance seul (`Attente::libre`) ?
    fn salon_libre() -> Oui {
        Oui::Non
    }

    /// Le coût du décompte final, quand la partie entre en `Fin` : rend 0.
    fn decompte_final() -> u32 {
        0
    }

    /// Appelée au début de chaque `appliquer`, avant toute décision.
    fn a_chaque_entree() {}

    /// Appelée juste avant chaque refus.
    fn avant_refus() {}

    /// Appelée au début de chaque vue de l'écran.
    fn en_vue_ecran() {}

    /// Un nombre que l'écran montre en bas à gauche.
    fn compteur_affiche() -> Option<u32> {
        None
    }

    /// Les points d'une tape.
    fn points_par_tape() -> u32 {
        1
    }

    /// Les tapes que compte une tape.
    fn tapes_par_tape() -> u32 {
        1
    }

    /// La valeur d'un chiffre choisi.
    fn valeur(n: u8) -> u32 {
        VALEURS.get(usize::from(n)).copied().unwrap_or(0)
    }

    /// La manche passe-t-elle par la prolongation ?
    fn prolonger() -> Oui {
        Oui::Oui
    }

    /// La partie finit-elle, après `parties` parties finies ?
    fn fin_atteinte(_parties: u32) -> Oui {
        Oui::Oui
    }

    /// Au salon, la manche attend-elle que chaque joueur se dise prêt ?
    fn salon_attend_chacun() -> Oui {
        Oui::Non
    }

    /// « Rejouer » oublie-t-il les joueurs partis (PUN-15) ?
    fn oublier_les_partis() -> Oui {
        Oui::Oui
    }

    /// Le mot se copie-t-il dans un champ public dès la manche ?
    fn indice_trop_tot() -> Oui {
        Oui::Non
    }

    /// Le numéro de la manche qui suit la manche `avant`.
    fn numero_de_manche(avant: u32) -> u32 {
        avant.saturating_add(1)
    }

    /// Le jeu retient-il chaque appareil arrivé ?
    fn retenir() -> Oui {
        Oui::Non
    }

    /// L'échéance de la phase qui suit, au réveil d'une échéance passée : née de
    /// maintenant, dans le jeu sain (PUN-34).
    fn echeance_suivante(m: &Maintenant<'_>, _precedente: Option<Echeance>, d: Duree) -> Echeance {
        Echeance::dans(m, d)
    }

    /// À la fin, un point de plus au joueur dont l'appareil s'écrit en premier ?
    fn departager_par_l_appareil() -> Oui {
        Oui::Non
    }

    /// Les points par tape, dans l'état.
    fn moyenne(points: u32, tapes: u32) -> f32 {
        let p = f32::from(u16::try_from(points).unwrap_or(u16::MAX));
        let t = f32::from(u16::try_from(tapes).unwrap_or(u16::MAX));
        if tapes == 0 { 0.0 } else { p / t }
    }

    /// Une tape compte-t-elle ? Toujours, dans le jeu sain.
    fn compte(_quand: &Quand) -> Oui {
        Oui::Oui
    }

    /// Ce qu'ajoute le décompte d'un chiffre choisi : rien, et aussitôt.
    fn compter_le_vote() -> u32 {
        0
    }

    /// Un joueur qui part est-il retiré de la partie, avec ses points ?
    fn retirer_au_depart() -> Oui {
        Oui::Non
    }

    /// À 9 999 points, l'écran montre-t-il un « Record ! » hors du repère ?
    fn deborder() -> Oui {
        Oui::Non
    }

    /// Le jeu promet-il la coupure transparente (E11) ?
    fn coupure_transparente() -> Oui {
        Oui::Oui
    }
}

macro_rules! marqueur {
    ($(#[$doc:meta])* $nom:ident) => {
        $(#[$doc])*
        #[jeu::etat]
        pub struct $nom;
    };
}

marqueur!(
    /// Le jeu sain : il passe tous les contrôles écrits.
    Sain
);
impl Defaut for Sain {
    fn neuf() -> Self {
        Sain
    }
}

marqueur!(
    /// E1 : `limites.memoire` vaut une page, sous la mémoire minimale du module. Le
    /// moteur refuse le module au chargement.
    DeclarationIncoherente
);
impl Defaut for DeclarationIncoherente {
    const DECLARATION: Declaration = Declaration {
        limites: Limites {
            memoire: 1,
            ..socle::DECLARATION.limites
        },
        ..socle::DECLARATION
    };
    fn neuf() -> Self {
        DeclarationIncoherente
    }
}

marqueur!(
    /// E2 : la première partie finit ; après « Rejouer », les manches ramènent au
    /// salon sans jamais finir. La phase terminale est atteinte (E3 tient), mais pas
    /// une seconde fin.
    JamaisFini
);
impl Defaut for JamaisFini {
    fn neuf() -> Self {
        JamaisFini
    }
    fn fin_atteinte(parties: u32) -> Oui {
        if parties == 0 { Oui::Oui } else { Oui::Non }
    }
}

marqueur!(
    /// E3 : la manche ne passe jamais par la prolongation.
    PhaseInaccessible
);
impl Defaut for PhaseInaccessible {
    fn neuf() -> Self {
        PhaseInaccessible
    }
    fn prolonger() -> Oui {
        Oui::Non
    }
}

marqueur!(
    /// E4 : deux points par tape, contre l'invariant « un point par tape au plus ».
    InvariantViole
);
impl Defaut for InvariantViole {
    fn neuf() -> Self {
        InvariantViole
    }
    fn points_par_tape() -> u32 {
        2
    }
}

marqueur!(
    /// E5 : dès qu'un joueur existe, le salon attend le « Prêt » de chacun, sans
    /// échéance, même de ceux qui sont partis, que « Rejouer » garde (CAT-78). Il se
    /// dit en plus `libre` : `libre` lève l'exigence d'une fin, pas celle d'un
    /// attendu présent.
    AttendUnAbsent
);
impl Defaut for AttendUnAbsent {
    fn neuf() -> Self {
        AttendUnAbsent
    }
    fn salon_libre() -> Oui {
        Oui::Oui
    }
    fn salon_attend_chacun() -> Oui {
        Oui::Oui
    }
    fn oublier_les_partis() -> Oui {
        Oui::Non
    }
}

/// Les refus comptés par [`RefuseApresAvoirEcrit`].
static REFUS: AtomicU32 = AtomicU32::new(0);

marqueur!(
    /// E6 : un `static` atomique compté avant chaque refus, et montré à l'écran. La
    /// forme canonique ne change pas ; la vue, si (§ 8.8, `refuse_apres_avoir_ecrit`).
    RefuseApresAvoirEcrit
);
impl Defaut for RefuseApresAvoirEcrit {
    fn neuf() -> Self {
        RefuseApresAvoirEcrit
    }
    fn a_la_creation() {
        REFUS.store(0, Ordering::Relaxed);
    }
    fn avant_refus() {
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

/// Les mots de [`VueQuiFuit`] : deux lettres, trois octets avec leur longueur.
const MOTS_COURTS: [&str; 4] = ["AB", "CD", "EF", "GH"];

marqueur!(
    /// E8 : le mot secret, de deux lettres, est copié dans un champ public dès le
    /// début de la manche, que l'écran montre (§ 8.8, `vue_qui_fuit`, CAT-46).
    VueQuiFuit
);
impl Defaut for VueQuiFuit {
    fn neuf() -> Self {
        VueQuiFuit
    }
    fn mots() -> &'static [&'static str] {
        &MOTS_COURTS
    }
    fn indice_trop_tot() -> Oui {
        Oui::Oui
    }
}

marqueur!(
    /// E9 : une tape compte deux tapes dans la copie native, une dans le module
    /// (§ 8.8, `natif_different`).
    NatifDifferent
);
impl Defaut for NatifDifferent {
    fn neuf() -> Self {
        NatifDifferent
    }
    fn tapes_par_tape() -> u32 {
        if cfg!(target_arch = "wasm32") { 1 } else { 2 }
    }
}

marqueur!(
    /// E13 : un `Choisir` hors de 0 à 9, qu'aucune page honnête n'envoie, indexe un
    /// tableau de dix valeurs et panique.
    OctetQuiPanique
);
impl Defaut for OctetQuiPanique {
    fn neuf() -> Self {
        OctetQuiPanique
    }
    #[expect(
        clippy::indexing_slicing,
        reason = "la faute de ce jeu fautif : un index venu d'un appareil, non borné"
    )]
    fn valeur(n: u8) -> u32 {
        VALEURS[usize::from(n)]
    }
}

/// Le prochain numéro de manche de [`EtatHorsEnveloppe`].
static MANCHES: AtomicU32 = AtomicU32::new(0);

marqueur!(
    /// E14 : le numéro de manche vient d'un `static`, hors de l'enveloppe. Le module le
    /// copie avec sa mémoire ; la copie native ne le restaure pas, et sa reprise
    /// diverge.
    EtatHorsEnveloppe
);
impl Defaut for EtatHorsEnveloppe {
    fn neuf() -> Self {
        EtatHorsEnveloppe
    }
    fn a_la_creation() {
        MANCHES.store(0, Ordering::Relaxed);
    }
    fn numero_de_manche(_: u32) -> u32 {
        MANCHES.fetch_add(1, Ordering::Relaxed).saturating_add(1)
    }
}

marqueur!(
    /// E15 : chaque appareil arrivé est retenu pour toujours (PUN-15, SQ-76).
    FuitLaMemoire
);
impl Defaut for FuitLaMemoire {
    fn neuf() -> Self {
        FuitLaMemoire
    }
    fn retenir() -> Oui {
        Oui::Oui
    }
}

marqueur!(
    /// E10 : à la fin, un point de plus au joueur dont l'appareil s'écrit en premier
    /// dans l'ordre des octets : une valeur tirée au hasard pour chaque soirée, qui ne
    /// départage rien (§ 8.8, `depend_de_l_appareil`, CAT-27).
    DependDeLAppareil
);
impl Defaut for DependDeLAppareil {
    fn neuf() -> Self {
        DependDeLAppareil
    }
    fn departager_par_l_appareil() -> Oui {
        Oui::Oui
    }
}

marqueur!(
    /// E11 : un joueur qui part, même le temps d'une coupure d'une seconde, est retiré
    /// de la partie avec ses points (§ 8.8, `coupure_qui_compte`, PUN-73).
    CoupureQuiCompte
);
impl Defaut for CoupureQuiCompte {
    fn neuf() -> Self {
        CoupureQuiCompte
    }
    fn retirer_au_depart() -> Oui {
        Oui::Oui
    }
}

/// La déclaration du socle, avec une fenêtre de douze pas (200 ms).
const AVEC_FENETRE: Declaration = Declaration {
    fenetre: Applicable::Oui(Duree::pas(12)),
    ..socle::DECLARATION
};

marqueur!(
    /// Sain, avec une fenêtre de douze pas : la tête et la définitive se séparent, et
    /// rien d'autre ne change. Il passe tous les contrôles, E12 compris.
    AvecFenetre
);
impl Defaut for AvecFenetre {
    const DECLARATION: Declaration = AVEC_FENETRE;
    fn neuf() -> Self {
        AvecFenetre
    }
}

marqueur!(
    /// E12 : avec une fenêtre, une tape reçue plus de deux pas après avoir été faite ne
    /// compte pas. La règle lit `quand.recu`, l'instant d'arrivée : un geste en retard
    /// dans la fenêtre change l'histoire définitive (§ 4.3, D12).
    CompteLeRecu
);
impl Defaut for CompteLeRecu {
    const DECLARATION: Declaration = AVEC_FENETRE;
    fn neuf() -> Self {
        CompteLeRecu
    }
    fn compte(quand: &Quand) -> Oui {
        let retard = quand.recu.numero().saturating_sub(quand.fait.pas.numero());
        if retard <= 2 { Oui::Oui } else { Oui::Non }
    }
    // Sa règle fait compter l'instant d'arrivée : une coupure change les points, et le
    // jeu ne la promet pas transparente.
    fn coupure_transparente() -> Oui {
        Oui::Non
    }
}

marqueur!(
    /// E16 : à 9 999 points, l'écran écrit « Record ! » hors du repère. Seul le pire
    /// cas y arrive (une soirée de six heures en reste loin) : sa scène est invalide
    /// (§ 6.1).
    PireCasQuiDeborde
);
impl Defaut for PireCasQuiDeborde {
    fn neuf() -> Self {
        PireCasQuiDeborde
    }
    fn deborder() -> Oui {
        Oui::Oui
    }
}

marqueur!(
    /// E17 : la moyenne d'un joueur qui n'a pas encore tapé vaut NaN, gardée dans
    /// l'état (§ 8.8, `nan_dans_l_etat`, CAT-05). La constante `f32::NAN` a les mêmes
    /// bits partout : seul E17 la voit.
    NanDansLEtat
);
impl Defaut for NanDansLEtat {
    fn neuf() -> Self {
        NanDansLEtat
    }
    fn moyenne(points: u32, tapes: u32) -> f32 {
        if tapes == 0 {
            f32::NAN
        } else {
            let p = f32::from(u16::try_from(points).unwrap_or(u16::MAX));
            p / f32::from(u16::try_from(tapes).unwrap_or(u16::MAX))
        }
    }
}

marqueur!(
    /// E18 : la prolongation, de dix secondes, part de l'échéance de la manche
    /// (`repoussee`), pas de maintenant. Après un gel qui passe l'échéance de trois
    /// secondes, la prolongation perd ces trois secondes : PUN-34 exactement.
    GelQuiEnchaine
);
impl Defaut for GelQuiEnchaine {
    fn neuf() -> Self {
        GelQuiEnchaine
    }
    fn prolongation() -> Duree {
        Duree::s(10)
    }
    fn echeance_suivante(m: &Maintenant<'_>, precedente: Option<Echeance>, d: Duree) -> Echeance {
        precedente.map_or_else(|| Echeance::dans(m, d), |e| e.repoussee(d))
    }
}

/// Les tours de la boucle de [`BoucleAuVote`].
const TOURS: u32 = 1 << 21;

marqueur!(
    /// E19 : le décompte final, une fois par partie, sur le `Temps` qui la fait
    /// entrer en `Fin`, coûte deux millions de tours de boucle : sous le budget
    /// d'arrêt de l'épreuve, bien au-delà de celui d'une entrée, et trop rare pour
    /// peser sur le 99ᵉ centile (§ 8.8, `boucle_au_vote`).
    BoucleAuVote
);
impl Defaut for BoucleAuVote {
    fn neuf() -> Self {
        BoucleAuVote
    }
    fn decompte_final() -> u32 {
        let mut somme = 0u32;
        for k in 0..TOURS {
            // Sans `black_box`, l'optimiseur calculerait la somme sans la boucle.
            somme = core::hint::black_box(somme.wrapping_add(k));
        }
        // La somme ne vaut jamais `u32::MAX` : le décompte n'ajoute rien.
        u32::from(somme == u32::MAX)
    }
}
