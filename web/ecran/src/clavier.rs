//! Le clavier de la télé (PHASE-3, § 2.9, P38 ; lot 10b), sans navigateur : ce qu'une
//! touche devient pour le jeu. Testé en natif.
//!
//! - **F8 est au commun** (SAL-162, P33) : une marque au journal, jamais au jeu.
//! - **Un raccourci de la scène** (SAL-152) : une touche enfoncée qui a la touche et les
//!   modificateurs d'un raccourci de la dernière scène envoie son action, et rien d'autre
//!   ne s'envoie ; une répétition automatique ne l'envoie que si le raccourci le dit.
//!   `Mods::CMD_OU_CTRL` veut dire Cmd sur Mac, Ctrl ailleurs.
//! - **Le clavier brut**, si la scène le demande : la touche telle quelle, enfoncée et
//!   relâchée.
//! - **Échap en plein écran** (P38) : quand le navigateur garde Échap pour sortir du plein
//!   écran (pas de Keyboard Lock : Firefox, Safari, ou un verrou refusé), le premier Échap
//!   ne va pas au jeu ; l'écran du commun dira « Plein écran quitté — cliquer pour
//!   revenir ». Avec le verrou (Chrome, `navigator.keyboard.lock(["Escape"])`), Échap
//!   arrive au jeu, et un appui long sort du plein écran.
//!
//! Les composants (un champ qui a le focus, une composition en cours : TL-32, TL-34)
//! écartent la touche avant (la file des événements de la page, lot 11) ; un raccourci se
//! cherche par `cibles::raccourci` (lot 11), où Cmd ou Ctrl s'ajoute aux autres
//! modificateurs.

use jeu_abi::{ClavierBrut, CodeTouche, Mods, Oui, PhaseTouche, SceneFil, Touche};

use crate::cibles::raccourci;

/// Où en est le plein écran, pour Échap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PleinEcran {
    /// Hors du plein écran.
    Non,
    /// En plein écran, Échap verrouillé pour la page (Keyboard Lock).
    Verrouille,
    /// En plein écran, Échap gardé par le navigateur.
    SansVerrou,
}

/// Le système de l'ordinateur de la soirée, pour `Mods::CMD_OU_CTRL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Systeme {
    /// macOS : Cmd.
    Mac,
    /// Un autre : Ctrl.
    Autre,
}

/// Ce qu'une touche devient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PourLeJeu {
    /// Rien : la scène n'en veut pas.
    Rien,
    /// Au commun : F8 (une marque).
    Commun,
    /// Au commun : F9, l'écran de démarrage par-dessus la partie, pour changer de jeu
    /// (lot 12, fin).
    Demarrage,
    /// Gardée par le navigateur (Échap qui sort du plein écran, P38).
    Navigateur,
    /// L'action d'un raccourci.
    Action(Vec<u8>),
    /// La touche brute.
    Brute(Touche),
}

/// La touche du commun qui remontre l'écran de démarrage (lot 12, fin ; choisie : F9, à
/// côté de F8, qu'aucun jeu ne porte).
pub const TOUCHE_DEMARRAGE: &str = "F9";

/// Ce que devient la touche `code` (le `code` du DOM, la touche physique), dans `phase`,
/// une répétition automatique ou non, sur la dernière `scene` reçue.
pub fn pour_le_jeu(
    scene: Option<&SceneFil>,
    code: &str,
    mods: Mods,
    phase: PhaseTouche,
    repetition: Oui,
    plein_ecran: PleinEcran,
    systeme: Systeme,
) -> PourLeJeu {
    if code == CodeTouche::F8.as_str() {
        return PourLeJeu::Commun;
    }
    if code == TOUCHE_DEMARRAGE {
        return PourLeJeu::Demarrage;
    }
    if code == CodeTouche::ECHAP.as_str() && plein_ecran == PleinEcran::SansVerrou {
        return PourLeJeu::Navigateur;
    }
    let Some(scene) = scene else {
        return PourLeJeu::Rien;
    };
    // Le raccourci se cherche comme au lot 11 (`cibles::raccourci`) : Cmd ou Ctrl s'ajoute
    // aux autres modificateurs du raccourci (Cmd+Maj+Z, « Rétablir » de Tier list).
    let mac = match systeme {
        Systeme::Mac => Oui::Oui,
        Systeme::Autre => Oui::Non,
    };
    if phase == PhaseTouche::Enfoncee
        && let Some(action) = raccourci(&scene.raccourcis, code, mods, repetition, mac)
    {
        return PourLeJeu::Action(action.to_vec());
    }
    match scene.clavier_brut {
        ClavierBrut::Oui { repetition: r } if repetition == Oui::Non || r == Oui::Oui => {
            CodeTouche::essayer(code).map_or(PourLeJeu::Rien, |code| {
                PourLeJeu::Brute(Touche {
                    code,
                    mods,
                    phase,
                    repetition,
                })
            })
        }
        ClavierBrut::Oui { .. } | ClavierBrut::Non => PourLeJeu::Rien,
    }
}

#[cfg(test)]
mod tests {
    use super::{PleinEcran, PourLeJeu, Systeme, pour_le_jeu};
    use jeu_abi::{ClavierBrut, CodeTouche, Mods, Oui, PhaseTouche, RaccourciFil, SceneFil};

    fn scene() -> SceneFil {
        SceneFil {
            raccourcis: vec![
                RaccourciFil {
                    touche: CodeTouche::ECHAP,
                    mods: Mods::AUCUN,
                    repetition: Oui::Non,
                    action: vec![7],
                },
                RaccourciFil {
                    touche: CodeTouche::new("KeyZ"),
                    mods: Mods::CMD_OU_CTRL,
                    repetition: Oui::Oui,
                    action: vec![8],
                },
            ],
            ..SceneFil::vide()
        }
    }

    fn cmd() -> Mods {
        Mods {
            meta: Oui::Oui,
            ..Mods::AUCUN
        }
    }

    /// P38 : Échap arrive au jeu hors du plein écran, et en plein écran sous le verrou
    /// (Chrome) ; sans verrou, le navigateur le garde pour sortir du plein écran, et le jeu
    /// ne le reçoit pas.
    #[test]
    fn echap_en_plein_ecran_p38() {
        let s = scene();
        let echap = |p| {
            pour_le_jeu(
                Some(&s),
                "Escape",
                Mods::AUCUN,
                PhaseTouche::Enfoncee,
                Oui::Non,
                p,
                Systeme::Mac,
            )
        };
        assert_eq!(echap(PleinEcran::Non), PourLeJeu::Action(vec![7]));
        assert_eq!(echap(PleinEcran::Verrouille), PourLeJeu::Action(vec![7]));
        assert_eq!(echap(PleinEcran::SansVerrou), PourLeJeu::Navigateur);
    }

    /// SAL-152, SAL-162 : un raccourci envoie son action, et seulement sur la touche
    /// enfoncée, avec ses modificateurs (Cmd sur Mac, Ctrl ailleurs) ; sa répétition s'il
    /// le dit ; F8 reste au commun ; une touche que la scène ne porte pas ne va nulle part.
    #[test]
    fn un_raccourci_envoie_son_action_sal152() {
        let s = scene();
        let t = |code, mods, phase, rep: bool, mac: bool| {
            let rep = if rep { Oui::Oui } else { Oui::Non };
            let systeme = if mac { Systeme::Mac } else { Systeme::Autre };
            pour_le_jeu(Some(&s), code, mods, phase, rep, PleinEcran::Non, systeme)
        };
        assert_eq!(
            t("KeyZ", cmd(), PhaseTouche::Enfoncee, false, true),
            PourLeJeu::Action(vec![8])
        );
        assert_eq!(
            t("KeyZ", cmd(), PhaseTouche::Enfoncee, true, true),
            PourLeJeu::Action(vec![8])
        );
        assert_eq!(
            t("KeyZ", cmd(), PhaseTouche::Enfoncee, false, false),
            PourLeJeu::Rien
        );
        let ctrl = Mods {
            ctrl: Oui::Oui,
            ..Mods::AUCUN
        };
        assert_eq!(
            t("KeyZ", ctrl, PhaseTouche::Enfoncee, false, false),
            PourLeJeu::Action(vec![8])
        );
        assert_eq!(
            t("KeyZ", Mods::AUCUN, PhaseTouche::Enfoncee, false, true),
            PourLeJeu::Rien
        );
        assert_eq!(
            t("Escape", Mods::AUCUN, PhaseTouche::Relachee, false, true),
            PourLeJeu::Rien
        );
        assert_eq!(
            t("Escape", Mods::AUCUN, PhaseTouche::Enfoncee, true, true),
            PourLeJeu::Rien
        );
        assert_eq!(
            t("F8", Mods::AUCUN, PhaseTouche::Enfoncee, false, true),
            PourLeJeu::Commun
        );
        assert_eq!(
            t("KeyA", Mods::AUCUN, PhaseTouche::Enfoncee, false, true),
            PourLeJeu::Rien
        );
        assert_eq!(
            pour_le_jeu(
                None,
                "Escape",
                Mods::AUCUN,
                PhaseTouche::Enfoncee,
                Oui::Non,
                PleinEcran::Non,
                Systeme::Mac
            ),
            PourLeJeu::Rien
        );
    }

    /// Un raccourci `CMD_OU_CTRL` avec Maj (« Rétablir », Cmd+Maj+Z, de Tier list) arrive
    /// au jeu, Cmd sur Mac et Ctrl ailleurs, et un raccourci prime sur le clavier brut :
    /// rien d'autre ne part (la fusion des lots 10b et 11).
    #[test]
    fn cmd_maj_et_le_raccourci_avant_le_clavier_brut() {
        let mut s = scene();
        s.raccourcis.push(RaccourciFil {
            touche: CodeTouche::new("KeyZ"),
            mods: Mods {
                maj: Oui::Oui,
                ..Mods::CMD_OU_CTRL
            },
            repetition: Oui::Non,
            action: vec![9],
        });
        s.clavier_brut = ClavierBrut::Oui {
            repetition: Oui::Oui,
        };
        let t = |mods, systeme| {
            pour_le_jeu(
                Some(&s),
                "KeyZ",
                mods,
                PhaseTouche::Enfoncee,
                Oui::Non,
                PleinEcran::Non,
                systeme,
            )
        };
        let cmd_maj = Mods {
            maj: Oui::Oui,
            ..cmd()
        };
        let ctrl_maj = Mods {
            maj: Oui::Oui,
            ctrl: Oui::Oui,
            ..Mods::AUCUN
        };
        assert_eq!(t(cmd_maj, Systeme::Mac), PourLeJeu::Action(vec![9]));
        assert_eq!(t(ctrl_maj, Systeme::Autre), PourLeJeu::Action(vec![9]));
        assert_eq!(t(cmd(), Systeme::Mac), PourLeJeu::Action(vec![8]));
        assert!(matches!(
            t(ctrl_maj, Systeme::Mac),
            PourLeJeu::Brute(k) if k.code.as_str() == "KeyZ"
        ));
    }

    /// Le clavier brut, si la scène le demande : enfoncée et relâchée, la répétition
    /// seulement si elle la veut.
    #[test]
    fn le_clavier_brut() {
        let mut s = scene();
        s.clavier_brut = ClavierBrut::Oui {
            repetition: Oui::Non,
        };
        let t = |phase, rep| {
            pour_le_jeu(
                Some(&s),
                "KeyA",
                Mods::AUCUN,
                phase,
                rep,
                PleinEcran::Non,
                Systeme::Mac,
            )
        };
        assert!(
            matches!(t(PhaseTouche::Enfoncee, Oui::Non), PourLeJeu::Brute(k) if k.code.as_str() == "KeyA")
        );
        assert!(
            matches!(t(PhaseTouche::Relachee, Oui::Non), PourLeJeu::Brute(k) if k.phase == PhaseTouche::Relachee)
        );
        assert_eq!(t(PhaseTouche::Enfoncee, Oui::Oui), PourLeJeu::Rien);
    }
}
