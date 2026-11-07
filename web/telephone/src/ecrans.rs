//! Les écrans du commun sur le téléphone (PHASE-3, § 2.10) : « Connexion à la télé… »,
//! « Un instant… », « Remplacé — Reprendre ici », « Version différente, recharge »,
//! « Salle pleine », « Scanne le nouveau QR code ». **Un seul message à la fois, qui nomme
//! la vraie cause** (REM-69) : une cause définitive l'emporte sur une attente.

use liaison::RefusRelais;
use salle::fil::{AttenteCommun, RefusSalle};

/// Un écran du commun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecran {
    /// Le lien à la télé n'est pas (encore) fait.
    Connexion,
    /// Relié, mais le jeu se charge, l'autorité reprend, ou la page n'est pas encore là.
    UnInstant,
    /// Un onglet plus récent du même téléphone a pris la place (SAL-21).
    Remplace,
    /// Le protocole de la page n'est pas celui de la télé.
    Version,
    /// La salle a toutes ses places.
    Pleine,
    /// L'adresse ou les clés du QR ne sont pas celles de cette soirée.
    NouveauQr,
}

impl Ecran {
    /// Le texte de l'écran.
    pub const fn texte(self) -> &'static str {
        match self {
            Ecran::Connexion => "Connexion à la télé…",
            Ecran::UnInstant => "Un instant…",
            Ecran::Remplace => "Remplacé — Reprendre ici",
            Ecran::Version => "Version différente, recharge",
            Ecran::Pleine => "Salle pleine",
            Ecran::NouveauQr => "Scanne le nouveau QR code",
        }
    }

    /// Le nom de l'écran, pour le banc.
    pub const fn nom(self) -> &'static str {
        match self {
            Ecran::Connexion => "connexion",
            Ecran::UnInstant => "un-instant",
            Ecran::Remplace => "remplace",
            Ecran::Version => "version",
            Ecran::Pleine => "pleine",
            Ecran::NouveauQr => "nouveau-qr",
        }
    }
}

/// Après tant de réponses de poignée de main que la clé du QR n'ouvre pas, la page dit
/// « Scanne le nouveau QR code » (choisi : une réponse illisible isolée peut venir d'un
/// relais qui rejoue).
pub const ILLISIBLES_AVANT_NOUVEAU_QR: u64 = 3;

/// Ce que la page sait, pour choisir son écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Etat {
    /// L'adresse ne se lit pas.
    pub adresse_illisible: bool,
    /// Le refus final de l'autorité.
    pub refus: Option<RefusSalle>,
    /// Le refus du relais.
    pub refus_relais: Option<RefusRelais>,
    /// Les trames de l'autorité qui ne s'ouvrent pas (une autre clé).
    pub illisibles: u64,
    /// Accueilli par l'autorité.
    pub accueilli: bool,
    /// Ce que le commun fait attendre.
    pub attente: Option<AttenteCommun>,
    /// Une page du jeu est posée.
    pub page: bool,
}

/// L'écran du commun à montrer, ou aucun : la page du jeu.
pub fn choisir(e: &Etat) -> Option<Ecran> {
    if e.adresse_illisible {
        return Some(Ecran::NouveauQr);
    }
    match e.refus {
        Some(RefusSalle::Remplace) => return Some(Ecran::Remplace),
        Some(RefusSalle::Protocole { .. }) => return Some(Ecran::Version),
        Some(RefusSalle::Pleine) => return Some(Ecran::Pleine),
        Some(RefusSalle::Fermee) => return Some(Ecran::NouveauQr),
        None => {}
    }
    match e.refus_relais {
        Some(RefusRelais::Protocole { .. }) => return Some(Ecran::Version),
        Some(RefusRelais::Pleine) => return Some(Ecran::Pleine),
        Some(RefusRelais::SalleInconnue) => return Some(Ecran::NouveauQr),
        Some(
            RefusRelais::PreuveFausse
            | RefusRelais::HorsBoucleLocale
            | RefusRelais::TropGros
            | RefusRelais::GardePleine
            | RefusRelais::Remplacee,
        )
        | None => {}
    }
    if !e.accueilli && e.illisibles >= ILLISIBLES_AVANT_NOUVEAU_QR {
        return Some(Ecran::NouveauQr);
    }
    if !e.accueilli {
        return Some(Ecran::Connexion);
    }
    if e.attente.is_some() || !e.page {
        return Some(Ecran::UnInstant);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// REM-69 : une seule cause, la vraie ; un refus définitif l'emporte sur l'attente.
    #[test]
    fn un_seul_message_qui_nomme_la_vraie_cause_rem69() {
        let jeu = Etat {
            accueilli: true,
            page: true,
            ..Etat::default()
        };
        assert_eq!(choisir(&jeu), None);
        assert_eq!(choisir(&Etat::default()), Some(Ecran::Connexion));
        assert_eq!(
            choisir(&Etat { page: false, ..jeu }),
            Some(Ecran::UnInstant)
        );
        assert_eq!(
            choisir(&Etat {
                attente: Some(AttenteCommun::Reprise),
                ..jeu
            }),
            Some(Ecran::UnInstant)
        );
        assert_eq!(
            choisir(&Etat {
                refus: Some(RefusSalle::Remplace),
                attente: Some(AttenteCommun::Chargement),
                ..jeu
            }),
            Some(Ecran::Remplace)
        );
        assert_eq!(
            choisir(&Etat {
                refus_relais: Some(RefusRelais::Protocole { relais: 9 }),
                ..Etat::default()
            }),
            Some(Ecran::Version)
        );
        assert_eq!(
            choisir(&Etat {
                refus: Some(RefusSalle::Pleine),
                ..Etat::default()
            }),
            Some(Ecran::Pleine)
        );
        assert_eq!(
            choisir(&Etat {
                illisibles: ILLISIBLES_AVANT_NOUVEAU_QR,
                ..Etat::default()
            }),
            Some(Ecran::NouveauQr)
        );
        assert_eq!(
            choisir(&Etat {
                adresse_illisible: true,
                ..jeu
            }),
            Some(Ecran::NouveauQr)
        );
    }
}
