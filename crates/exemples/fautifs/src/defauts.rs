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
//! | [`RefuseApresAvoirEcrit`] | un `static` écrit avant chaque refus, montré à l'écran | E6 (et E8, qui voit le `static` à l'écran) |
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
//! | [`PhaseSansCas`] | la prolongation n'arrive jamais, et aucun pire cas n'y mène | E3 |
//! | [`InvariantJamaisEvalue`] | un invariant déclaré, que seul un record ferait évaluer | E4 |
//! | [`RetientUnRefus`] | un `static` écrit avant un refus, lu à la tape suivante du même pas | E6 |
//! | [`ChoixQuiFuit`] | le chiffre choisi, gardé secret pour son joueur, se lit aussi dans le choix public | E8 |
//! | [`PaniqueAuTemps`] | trois `Temps` qui paniquent à la fin d'une manche où joue le bot lent | E2 |
//! | [`JugeSurLeFait`] | un chiffre affiché change chaque seconde ; un choix fait après le changement est cru vu | E4 (monde réel) |
//! | [`FichierQuiPanique`] | le quatrième octet d'un fichier lu au salon, pris sans le vérifier | E13 (monde réel) |
//! | [`PireCasTropLourd`] | à 9 999 points, la vue de l'écran fait deux millions de tours de boucle | E19 (pire cas) |
//! | [`FinTropLourde`] | en fin de partie, la vue de l'écran fait sept cent mille tours de boucle | E19 (lecture) |
//! | [`ZonesLourdes`] | un brouillon de 32 Mio, que le moteur remet à zéro avant chaque appel | E19 (coût fixe) |
//! | [`SecretDansUnStatic`] | le mot tiré recopié dans un `static`, que l'écran montre pendant la manche | E8 |
//! | [`PireCasTropLeger`] | des pires cas sans aucun joueur, plus légers que l'écran d'une soirée | E16 |
//!
//! `JugeSurLeFait` et `FichierQuiPanique` passent dans le monde trop sage de la phase 1 (`Monde::PHASE_1`
//! de l'épreuve) : seul le monde réel les fait échouer.
//!
//! # Les `static` des défauts
//!
//! Cinq défauts tiennent un `static` atomique : c'est leur faute. Dans le module, ce
//! `static` est dans la mémoire, que le moteur copie et restaure avec le reste. Dans
//! la copie native, il est commun au processus : chaque défaut le remet à zéro à la
//! création, pour que la copie native refasse la même histoire que le module tant
//! qu'aucune reprise ne passe par là. Sans fils dans le module, l'atomique s'y compile
//! en lectures et écritures ordinaires : la réécriture n'y voit aucune instruction
//! atomique.

use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use jeu::prelude::*;

use crate::socle::{self, CasDuSocle, Etape, PRENOM_LENT, VALEURS};

/// Ce qu'un défaut peut changer au socle. Chaque méthode a la valeur du jeu sain.
pub trait Defaut: Etat {
    /// Les écrans de la galerie des pires cas : les phases du socle, dans le jeu sain.
    type Cas: CasDuSocle;

    /// La déclaration.
    const DECLARATION: Declaration = socle::DECLARATION;

    /// Ce qu'exige le socle (E4).
    const INVARIANTS: &'static [&'static str] = &socle::INVARIANTS;

    /// Le marqueur.
    fn neuf() -> Self;

    /// Appelée au début de chaque `creer`, avec le pas de la création.
    fn a_la_creation(_pas: Pas) {}

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

    /// Appelée juste avant chaque refus, avec le pas de l'entrée refusée.
    fn avant_refus(&self, _pas: Pas) {}

    /// Appelée juste après qu'une tape est acceptée, avec son pas.
    fn a_la_tape(&mut self, _pas: Pas) {}

    /// Appelée juste après qu'un chiffre choisi est accepté, avec son joueur.
    fn au_choix(&mut self, _de: Appareil, _n: u8) {}

    /// Le chiffre que montrent les téléphones pendant la manche.
    fn chiffre(&self) -> u8 {
        0
    }

    /// Les joueurs des pires cas : autant que la limite, dans le jeu sain.
    fn joueurs_du_pire_cas() -> usize {
        socle::JOUEURS_MAX
    }

    /// Appelée à l'ouverture de chaque manche, avec le mot tiré, avant qu'il ne soit
    /// caché.
    fn au_mot(_mot: &str) {}

    /// Appelée à l'ouverture de chaque manche.
    fn a_l_ouverture(&mut self, _m: &Maintenant<'_>) {}

    /// Le prochain changement du chiffre, pendant la manche : aucun, dans le jeu sain.
    fn tic(&self) -> Option<Echeance> {
        None
    }

    /// Le chiffre change.
    fn au_tic(&mut self, _m: &Maintenant<'_>) {}

    /// Un chiffre choisi se reçoit-il, à cette place ? Toujours, dans le jeu sain.
    fn choix_recevable(&self, _quand: &Quand, _place: Pas) -> Oui {
        Oui::Oui
    }

    /// Ce que le défaut exige en plus des invariants du socle.
    fn exiger(&self, _v: &mut Verif) {}

    /// Le chiffre que choisit le bot : celui qu'il a tiré, dans le jeu sain.
    fn chiffre_du_bot(_montre: u8, tire: u8) -> u8 {
        tire
    }

    /// Le salon propose-t-il un fichier à lire ?
    fn fichiers() -> Oui {
        Oui::Non
    }

    /// Ce que donne un fichier lu : son quatrième octet, s'il en a quatre.
    fn lire_le_fichier(o: &[u8]) -> u32 {
        o.get(3).map_or(0, |b| u32::from(*b))
    }

    /// Appelée quand un `Temps` trouve l'échéance passée, avant toute décision : la
    /// phase, un joueur nommé [`PRENOM_LENT`] est-il là, et le temps jeté que ce
    /// `Temps` porte.
    fn au_temps_echu(_phase: Etape, _lent: Oui, _saut: Duree) {}

    /// Le prénom que prend le bot d'un téléphone de ce style.
    fn prenom_du_bot(_style: Style) -> &'static str {
        "Camille"
    }

    /// Les invariants comptent-ils celui que seul un record ferait évaluer ?
    fn invariant_hors_d_atteinte() -> Oui {
        Oui::Non
    }

    /// Appelée au début de chaque vue de l'écran.
    fn en_vue_ecran() {}

    /// Les tours de boucle que coûte la vue de l'écran, dans cette phase, quand un
    /// joueur a 9 999 points (`record`) ou non : aucun.
    fn tours_de_la_vue(_phase: Etape, _record: Oui) -> u32 {
        0
    }

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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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

/// Les mots de [`VueQuiFuit`] : deux lettres, trois octets avec leur longueur.
const MOTS_COURTS: [&str; 4] = ["AB", "CD", "EF", "GH"];

marqueur!(
    /// E8 : le mot secret, de deux lettres, est copié dans un champ public dès le
    /// début de la manche, que l'écran montre (§ 8.8, `vue_qui_fuit`, CAT-46).
    VueQuiFuit
);
impl Defaut for VueQuiFuit {
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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

marqueur!(
    /// E15 : chaque appareil arrivé est retenu pour toujours (PUN-15, SQ-76).
    FuitLaMemoire
);
impl Defaut for FuitLaMemoire {
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
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
    type Cas = Etape;
    fn neuf() -> Self {
        BoucleAuVote
    }
    fn decompte_final() -> u32 {
        // La somme ne vaut jamais `u32::MAX` : le décompte n'ajoute rien.
        u32::from(tourner(TOURS) == u32::MAX)
    }
}

/// Les écrans de [`PhaseSansCas`] : ceux du socle, sans la prolongation.
#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum CasSansProlongation {
    /// Le salon.
    Salon,
    /// La manche.
    Manche,
    /// La fin.
    Fin,
}

impl CasDuSocle for CasSansProlongation {
    fn de(phase: Etape) -> Self {
        match phase {
            Etape::Salon => CasSansProlongation::Salon,
            // La prolongation n'arrive jamais dans ce jeu : son écran serait la manche.
            Etape::Manche | Etape::Prolongation => CasSansProlongation::Manche,
            Etape::Fin => CasSansProlongation::Fin,
        }
    }
    fn phase(self) -> Etape {
        match self {
            CasSansProlongation::Salon => Etape::Salon,
            CasSansProlongation::Manche => Etape::Manche,
            CasSansProlongation::Fin => Etape::Fin,
        }
    }
}

marqueur!(
    /// E3 : la manche ne passe jamais par la prolongation, et les écrans de la galerie
    /// l'oublient : aucun pire cas n'y mène. Une épreuve qui ne connaîtrait les phases
    /// que par les pires cas ne verrait pas qu'une variante de `Phase` n'est jamais
    /// atteinte (RESTE-PHASE-1, point 23).
    PhaseSansCas
);
impl Defaut for PhaseSansCas {
    type Cas = CasSansProlongation;
    fn neuf() -> Self {
        PhaseSansCas
    }
    fn prolonger() -> Oui {
        Oui::Non
    }
}

/// La déclaration d'invariants de [`InvariantJamaisEvalue`] : celle du socle, et un de
/// plus.
const INVARIANTS_ET_UN_RECORD: [&str; 5] = [
    "un point par tape au plus",
    "six joueurs au plus",
    "un meneur est un joueur",
    "un point ne se perd pas",
    "un record de plus de dix mille points vient après une partie finie",
];

marqueur!(
    /// E4 : un invariant déclaré n'est évalué qu'au-delà de dix mille points, où aucune
    /// soirée n'arrive. Il n'est jamais violé : il n'a jamais tourné (OUT-01,
    /// RESTE-PHASE-1, point 24).
    InvariantJamaisEvalue
);
impl Defaut for InvariantJamaisEvalue {
    type Cas = Etape;
    const INVARIANTS: &'static [&'static str] = &INVARIANTS_ET_UN_RECORD;
    fn neuf() -> Self {
        InvariantJamaisEvalue
    }
    fn invariant_hors_d_atteinte() -> Oui {
        Oui::Oui
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

/// E8 : le chiffre que choisit un joueur est gardé dans un secret qu'il est seul à
/// voir, d'un octet ; le choix public du socle en garde aussi la valeur. Aucune
/// recherche d'octets ne trouve un secret d'un octet (un chiffre, un vote, un
/// booléen) : seule la variation le voit, qui rejoue le même geste avec un autre
/// chiffre et compare ce que voit qui n'a pas le droit de le voir (RESTE-PHASE-1,
/// point 16 ; CAT-46 ; PUN, « qui a voté pour qui »).
#[jeu::etat]
pub struct ChoixQuiFuit {
    /// Le dernier chiffre choisi, pour son joueur seul.
    choix: Secret<u8>,
}
impl Defaut for ChoixQuiFuit {
    type Cas = Etape;
    fn neuf() -> Self {
        ChoixQuiFuit {
            choix: Secret::cacher(Qui::Personne, 0),
        }
    }
    fn au_choix(&mut self, de: Appareil, n: u8) {
        self.choix = Secret::cacher(Qui::Appareil(de), n);
    }
}

marqueur!(
    /// La reprise (§ 4.6 ; INTERFACE, § 8.8, `panique_au_temps`) : à l'échéance d'une
    /// manche où joue un joueur nommé [`PRENOM_LENT`], un `Temps` qui ne porte aucun
    /// temps jeté panique ; trois pas de suite, et la partie tombe en panne. L'épreuve
    /// doit la faire repartir (`Partie::repartir`) : le `Temps` d'après la reprise
    /// porte le temps perdu depuis le point sain, ne panique plus, et la partie
    /// continue. Seul le bot lent de la soirée principale prend ce prénom : la faute n'a
    /// lieu que là, et E2 la juge (« aucune faute en chemin »).
    PaniqueAuTemps
);
impl Defaut for PaniqueAuTemps {
    type Cas = Etape;
    fn neuf() -> Self {
        PaniqueAuTemps
    }
    fn prenom_du_bot(style: Style) -> &'static str {
        match style {
            Style::Lent => PRENOM_LENT,
            Style::Sage | Style::Hostile => "Camille",
        }
    }
    /// C'est le défaut : un `Temps` qui panique, que l'épreuve doit voir et dont la
    /// partie doit repartir.
    fn au_temps_echu(phase: Etape, lent: Oui, saut: Duree) {
        assert!(
            !(phase == Etape::Manche && lent == Oui::Oui && saut == Duree::ZERO),
            "le temps panique à l'échéance de la manche"
        );
    }
}

/// Ce qu'exige [`JugeSurLeFait`] : le socle, et le chiffre.
pub const INVARIANTS_DU_CHIFFRE: [&str; 5] = [
    socle::INVARIANTS[0],
    socle::INVARIANTS[1],
    socle::INVARIANTS[2],
    socle::INVARIANTS[3],
    "un chiffre choisi est le chiffre affiché",
];

/// La plus vieille vue d'où un choix se reçoit encore, en pas, dans [`JugeSurLeFait`].
const VUE_RECENTE: u64 = 30;

/// E4, dans le monde réel : pendant la manche, les téléphones montrent un chiffre qui
/// change chaque seconde, et chaque joueur doit choisir celui qu'il voit. Le jeu croit
/// qu'un choix *fait* après le changement a *vu* le nouveau chiffre : il juge sur
/// `quand.fait`, pas sur `quand.vu`. Il écarte bien un choix venu d'une vue de plus
/// d'une demi-seconde, et sa marge d'antidatage est large : dans le monde de la phase 1,
/// où un bot décidait sur l'état courant et où `fait` était exact, cela suffisait.
/// Dans le monde réel, un bot décide sur la vue que son téléphone a reçue, en retard
/// d'un trajet, et `fait` porte l'erreur de son horloge : un choix fait juste après le
/// changement montre encore l'ancien chiffre, et l'invariant « un chiffre choisi est
/// le chiffre affiché » tombe (RESTE-PHASE-1, points 20 et 21 ; CONTRAT : « Les
/// appareils partagent des numéros de pas, jamais une heure »). La règle juste lit
/// `quand.vu`, le pas de la vue que l'appareil montrait.
#[jeu::etat]
pub struct JugeSurLeFait {
    /// Le chiffre affiché.
    chiffre: u8,
    /// Le pas où il a changé.
    depuis: Pas,
    /// Son prochain changement.
    prochain: Option<Echeance>,
    /// Les chiffres choisis acceptés qui n'étaient pas le chiffre affiché.
    ecarts: u32,
}
impl Defaut for JugeSurLeFait {
    type Cas = Etape;
    const DECLARATION: Declaration = Declaration {
        antidatage_max: Duree::s(10),
        ..socle::DECLARATION
    };
    const INVARIANTS: &'static [&'static str] = &INVARIANTS_DU_CHIFFRE;
    fn neuf() -> Self {
        JugeSurLeFait {
            chiffre: 0,
            depuis: Pas::ZERO,
            prochain: None,
            ecarts: 0,
        }
    }
    fn chiffre(&self) -> u8 {
        self.chiffre
    }
    fn a_l_ouverture(&mut self, m: &Maintenant<'_>) {
        self.chiffre = 1;
        self.depuis = m.pas();
        self.prochain = Some(Echeance::dans(m, Duree::s(1)));
    }
    fn tic(&self) -> Option<Echeance> {
        self.prochain
    }
    fn au_tic(&mut self, m: &Maintenant<'_>) {
        // Trois de plus, modulo dix : deux chiffres de suite diffèrent toujours.
        self.chiffre = self.chiffre.saturating_add(3).checked_rem(10).unwrap_or(0);
        self.depuis = m.pas();
        self.prochain = Some(Echeance::dans(m, Duree::s(1)));
    }
    fn choix_recevable(&self, quand: &Quand, place: Pas) -> Oui {
        let recente = place.numero().saturating_sub(quand.vu.numero()) <= VUE_RECENTE;
        if recente && quand.fait.pas >= self.depuis {
            Oui::Oui
        } else {
            Oui::Non
        }
    }
    fn au_choix(&mut self, _de: Appareil, n: u8) {
        if n != self.chiffre {
            self.ecarts = self.ecarts.saturating_add(1);
        }
    }
    fn exiger(&self, v: &mut Verif) {
        jeu::exiger!(
            v,
            self.ecarts == 0,
            "un chiffre choisi est le chiffre affiché"
        );
    }
    fn chiffre_du_bot(montre: u8, _tire: u8) -> u8 {
        montre
    }
}

marqueur!(
    /// E13, dans le monde réel : l'écran apporte un fichier au salon, le jeu le lit
    /// (`Demande::Lire`), et prend son quatrième octet sans vérifier qu'il en a quatre.
    /// Le fichier du bot en a seize : dans le monde de la phase 1, qui refusait toute
    /// lecture, et dans les soirées où le monde sert le fichier tel quel ou trop gros,
    /// rien ne casse. Dans la soirée hostile, le monde abîme les fichiers (une fin
    /// perdue, un octet changé, presque rien) : un fichier de moins de quatre octets
    /// fait paniquer le jeu (TL-14, constat de Tier list : aucune soirée ne jouait les
    /// fichiers).
    FichierQuiPanique
);
impl Defaut for FichierQuiPanique {
    type Cas = Etape;
    fn neuf() -> Self {
        FichierQuiPanique
    }
    fn fichiers() -> Oui {
        Oui::Oui
    }
    #[expect(
        clippy::indexing_slicing,
        reason = "la faute de ce jeu fautif : un fichier venu de l'extérieur, lu sans en vérifier la taille"
    )]
    fn lire_le_fichier(o: &[u8]) -> u32 {
        u32::from(o[3])
    }
}

/// Une boucle qui coûte `tours` tours, sans rien changer : son carburant est son seul
/// effet (E19).
pub(crate) fn tourner(tours: u32) -> u32 {
    let mut somme = 0u32;
    for k in 0..tours {
        // Sans `black_box`, l'optimiseur calculerait la somme sans la boucle.
        somme = core::hint::black_box(somme.wrapping_add(k));
    }
    somme
}

marqueur!(
    /// E19, sur les pires cas (RESTE-PHASE-1, point 19) : la vue de l'écran fait deux
    /// millions de tours de boucle (de l'ordre de 22 millions d'unités de carburant)
    /// quand un joueur a 9 999 points. Seuls les pires cas en ont autant : les bots
    /// n'y arrivent jamais, et aucune soirée ne paie cette vue. Un jeu qui ménage ses
    /// bots ne ménage pas ses pires cas.
    PireCasTropLourd
);
impl Defaut for PireCasTropLourd {
    type Cas = Etape;
    fn neuf() -> Self {
        PireCasTropLourd
    }
    fn tours_de_la_vue(_: Etape, record: Oui) -> u32 {
        if record == Oui::Oui { TOURS } else { 0 }
    }
}

/// Les tours de la vue de l'écran de [`FinTropLourde`] : de l'ordre de sept
/// millions d'unités de carburant, entre le budget d'une lecture courante et celui
/// d'une vue de pire cas (`epreuve::BUDGETS_E19`).
const TOURS_DE_LA_FIN: u32 = 700_000;

marqueur!(
    /// E19, sur une lecture courante (RESTE-PHASE-1, point 18) : en fin de partie, la
    /// vue de l'écran fait sept cent mille tours de boucle, de l'ordre de sept millions
    /// d'unités. Assez peu pour une vue de pire cas, qui a droit à l'image la plus
    /// longue : son pire cas de fin passe. Trop pour une lecture courante, qui doit
    /// tenir dans une image de 60 par seconde. La rareté (une lecture lourde sous le
    /// 99ᵉ centile, jugée à son maximum) est tenue par le test
    /// `epreuve::budgets::tests::une_lecture_rare_et_lourde_echoue` : les vues de la fin
    /// font ici environ une vue sur dix.
    FinTropLourde
);
impl Defaut for FinTropLourde {
    type Cas = Etape;
    fn neuf() -> Self {
        FinTropLourde
    }
    fn tours_de_la_vue(phase: Etape, _: Oui) -> u32 {
        if phase == Etape::Fin {
            TOURS_DE_LA_FIN
        } else {
            0
        }
    }
}

/// Le brouillon de [`ZonesLourdes`], en octets : plus que les 24 Mio de Tier list,
/// pour que le coût fixe déborde nettement, et sur le plan court aussi.
const BROUILLON_LOURD: u32 = 32 << 20;

marqueur!(
    /// E19, par le coût fixe d'un appel (MESURES-PHASE-1, proposition 1) : un brouillon
    /// de 32 Mio, que le moteur remet à zéro avant chaque appel, et rien d'autre. Le
    /// carburant de chaque appel est celui du jeu sain ; c'est le coût fixe, de l'ordre
    /// de 1,75 million d'unités par appel, qui fait déborder les images.
    ZonesLourdes
);
impl Defaut for ZonesLourdes {
    type Cas = Etape;
    const DECLARATION: Declaration = Declaration {
        limites: Limites {
            // Le brouillon, la pile et le tas : 40 Mio (`modules/zones-lourdes/build.rs`).
            memoire: 640,
            brouillon: BROUILLON_LOURD,
            ..socle::DECLARATION.limites
        },
        ..socle::DECLARATION
    };
    fn neuf() -> Self {
        ZonesLourdes
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

marqueur!(
    /// E16 : les pires cas n'ont aucun joueur. Ils passent leur cas, leurs invariants
    /// et leur scène ; mais l'écran d'une soirée, avec ses joueurs, coûte plus que le
    /// plus lourd d'entre eux : ils ne sont pas construits depuis les limites
    /// (INTERFACE, § 2.13 ; revue de la phase 2, constat 14).
    PireCasTropLeger
);
impl Defaut for PireCasTropLeger {
    type Cas = Etape;
    fn neuf() -> Self {
        PireCasTropLeger
    }
    fn joueurs_du_pire_cas() -> usize {
        0
    }
}
