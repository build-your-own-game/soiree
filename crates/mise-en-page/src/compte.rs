//! Le texte d'un compte à rebours, que le commun forme à chaque image depuis l'échéance
//! et le pas estimé (PHASE-3, § 2.9 ; SQ-36, PUN-39) : une seule définition, pour la
//! télé qui le dessine et le banc qui compare son agencement (lot 9).

use jeu_abi::{CompteAReboursFil, PAS_PAR_SECONDE, Pas};

/// Les secondes restantes, arrondies au-dessus ; `m:ss` à partir d'une minute.
pub fn texte(compte: &CompteAReboursFil, pas: Pas) -> String {
    let restant = match compte {
        CompteAReboursFil::Court { echeance } => echeance.numero().saturating_sub(pas.numero()),
        CompteAReboursFil::Fige { restant } => restant.en_pas(),
    };
    let secondes = restant.div_ceil(PAS_PAR_SECONDE);
    if secondes >= 60 {
        format!("{}:{:02}", secondes / 60, secondes % 60)
    } else {
        format!("{secondes}")
    }
}
