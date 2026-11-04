//! La déclaration d'un jeu (§ 2.12) et ce qu'il dit de lui-même (§ 2.11).
//!
//! `Declaration`, `Applicable<T>`, `Raison`, `Pause`, `Telephones`, `ReglesTenu` et
//! `Asset` vivent dans `jeu` : ils portent des `&'static str` et la vérification de
//! `raison!` à la compilation. Leur forme possédée, que le moteur et le cœur lisent,
//! est ici. Les types sans texte ([`Places`], [`FormeEcran`], [`Voulues`],
//! [`Limites`]…) servent aux deux côtés.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::scene::Cle;
use crate::temps::Duree;
use crate::valeurs::{CodeTouche, HorsBornes, Oui, TexteCourt};

/// Une raison dit-elle quelque chose : au moins trois mots ? La seule définition,
/// pour `raison!` à la compilation et pour le moteur qui relit une déclaration
/// (HIS-04, HIS-43, OUT-03 à OUT-05). Un mot est une suite de caractères sans
/// espace, tabulation ni fin de ligne.
pub const fn raison_suffisante(raison: &str) -> bool {
    let mut mots: u32 = 0;
    let mut dans_un_mot = false;
    let mut reste = raison.as_bytes();
    while let [premier, suite @ ..] = reste {
        let blanc = matches!(*premier, b' ' | b'\t' | b'\n' | b'\r');
        if !blanc && !dans_un_mot {
            mots = mots.saturating_add(1);
        }
        dans_un_mot = !blanc;
        reste = suite;
    }
    mots >= 3
}

/// Une réponse qui s'applique, ou qui est sans objet avec sa raison. Sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApplicableFil<T> {
    /// La réponse.
    Oui(T),
    /// Sans objet, et pourquoi.
    SansObjet(String),
}

/// Combien de joueurs et d'appareils. Ne se construit pas incohérent : ni
/// `min > max`, ni `max = 0`, ni plus de joueurs que d'appareils (OUT-200, HIS-13).
/// Les constructeurs sont `const` : dans la déclaration, une incohérence est une
/// erreur de compilation. Le décodage refuse de même.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PlacesBrutes", into = "PlacesBrutes")]
pub struct Places {
    joueurs_min: u16,
    joueurs_max: u16,
    appareils_max: u16,
}

/// La forme sur le fil de [`Places`] : trois `u16`.
#[derive(Clone, Copy, Serialize, Deserialize)]
struct PlacesBrutes {
    joueurs_min: u16,
    joueurs_max: u16,
    appareils_max: u16,
}

impl Places {
    /// De `min` à `max` joueurs, autant d'appareils que de joueurs au plus.
    pub const fn joueurs(min: u16, max: u16) -> Places {
        assert!(min <= max, "Places : il faut min <= max");
        assert!(max > 0, "Places : il faut au moins une place");
        Places {
            joueurs_min: min,
            joueurs_max: max,
            appareils_max: max,
        }
    }

    /// Au plus `max` appareils : pas moins que de joueurs.
    #[must_use]
    pub const fn appareils(self, max: u16) -> Places {
        assert!(
            max >= self.joueurs_max,
            "Places : au moins autant d'appareils que de joueurs"
        );
        Places {
            appareils_max: max,
            ..self
        }
    }

    /// Des places relues sur le fil, si elles sont cohérentes.
    pub const fn essayer(joueurs_min: u16, joueurs_max: u16, appareils_max: u16) -> Option<Places> {
        if joueurs_min <= joueurs_max && joueurs_max > 0 && appareils_max >= joueurs_max {
            Some(Places {
                joueurs_min,
                joueurs_max,
                appareils_max,
            })
        } else {
            None
        }
    }

    /// Le moins de joueurs pour jouer.
    pub const fn joueurs_min(self) -> u16 {
        self.joueurs_min
    }

    /// Le plus de joueurs.
    pub const fn joueurs_max(self) -> u16 {
        self.joueurs_max
    }

    /// Le plus d'appareils.
    pub const fn appareils_max(self) -> u16 {
        self.appareils_max
    }
}

impl TryFrom<PlacesBrutes> for Places {
    type Error = HorsBornes;

    fn try_from(b: PlacesBrutes) -> Result<Self, Self::Error> {
        Places::essayer(b.joueurs_min, b.joueurs_max, b.appareils_max).ok_or(HorsBornes {
            attendu: "min <= max, max > 0 et au moins autant d'appareils que de joueurs",
        })
    }
}

impl From<Places> for PlacesBrutes {
    fn from(p: Places) -> PlacesBrutes {
        PlacesBrutes {
            joueurs_min: p.joueurs_min,
            joueurs_max: p.joueurs_max,
            appareils_max: p.appareils_max,
        }
    }
}

/// Qui met la partie en pause. Sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PauseFil {
    /// Le jeu, dans ses règles.
    DuJeu,
    /// Le commun : plus de `Temps`, et chaque tenu relâché, cause `Pause`.
    DuCommun,
    /// Aucune pause, et pourquoi.
    Aucune(String),
}

/// Ce que montre l'écran de l'ordinateur de la soirée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormeEcran {
    /// Une scène à taille fixe, mise à l'échelle dans la zone sûre (HIS-46, PUN-51).
    Scene {
        /// La largeur du repère.
        largeur: u16,
        /// La hauteur du repère.
        hauteur: u16,
        /// La part de l'écran que la zone sûre occupe, en pour cent.
        zone_sure_pour_cent: u8,
        /// Agrandir seulement par un facteur entier.
        echelle_entiere: Oui,
    },
    /// Une page qui défile (Tier list).
    Page {
        /// La largeur la plus grande du contenu.
        largeur_max: u16,
    },
}

/// Les téléphones, sur le fil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelephonesFil {
    /// Combien de téléphones par joueur.
    pub repartition: Repartition,
    /// Les règles de leurs états tenus.
    pub tenu: ReglesTenuFil,
}

/// Combien de téléphones par joueur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Repartition {
    /// Un par joueur.
    UnParJoueur,
    /// Un par équipe.
    UnParEquipe,
    /// Un seul pour la partie.
    UnPourLaPartie,
}

/// Les règles d'un état tenu (§ 4.3), sur le fil. Le cœur les applique.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReglesTenuFil {
    /// Relâché quand le silence de l'appareil dépasse cette durée (SQ-01).
    pub fraicheur: ApplicableFil<Duree>,
    /// Un état fait plus tôt ne compte pas ; un relâchement passe toujours ; la valeur
    /// tenue se relâche quand sa dernière redite a cet âge (SQ-02).
    pub age_max: ApplicableFil<Duree>,
    /// Changements livrés au plus, par seconde, par clé et par appareil (PUN-03).
    pub debit_max: u16,
}

/// Les capacités que le jeu sait employer. Aucune n'est requise : le jeu reste
/// jouable sans (REM-146, OUT-211).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Voulues {
    /// La vibration.
    pub vibration: Oui,
    /// Les capteurs de mouvement.
    pub mouvement: Oui,
    /// Le son sur le téléphone.
    pub son: Oui,
    /// L'écran maintenu allumé.
    pub ecran_allume: Oui,
    /// Le micro.
    pub micro: Oui,
    /// La manette tactile native de l'app.
    pub pad_natif: Oui,
}

/// Les limites que le moteur fait respecter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limites {
    /// Les octets d'un geste ou d'un tenu.
    pub geste: u32,
    /// Les événements en attente, par appareil.
    pub file: u16,
    /// Les octets d'une vue encodée.
    pub vue: u32,
    /// Les effets d'une vue.
    pub effets: u16,
    /// La mémoire du module, en pages de 64 Kio au plus.
    pub memoire: u32,
    /// Les octets de la zone de brouillon des lectures.
    pub brouillon: u32,
}

/// Un asset déclaré, sur le fil. Ses octets se lisent par `Appel::Ressources`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFil {
    /// Sa clé.
    pub cle: Cle,
    /// Ce qu'il est.
    pub sorte: SorteAsset,
    /// Sa taille, en octets.
    pub taille: u64,
}

/// Ce qu'est un asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorteAsset {
    /// Une police.
    Police,
    /// Une image.
    Image,
    /// Un son.
    Son,
}

/// Une mécanique déclarée : elle choisit les questions de la base de leçons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Mecanique {
    /// Temps réel.
    TempsReel,
    /// Physique.
    Physique,
    /// Tour par tour.
    TourParTour,
    /// Vote.
    Vote,
    /// Texte saisi par les joueurs.
    Texte,
    /// Hasard.
    Hasard,
    /// Économie.
    Economie,
    /// Progression.
    Progression,
    /// Égalités à départager.
    Egalites,
    /// Affichage exigeant.
    Affichage,
    /// Plateau.
    Plateau,
    /// Fichiers.
    Fichiers,
}

impl Mecanique {
    /// Toutes les mécaniques, dans l'ordre de déclaration.
    pub const TOUTES: &'static [Mecanique] = &[
        Mecanique::TempsReel,
        Mecanique::Physique,
        Mecanique::TourParTour,
        Mecanique::Vote,
        Mecanique::Texte,
        Mecanique::Hasard,
        Mecanique::Economie,
        Mecanique::Progression,
        Mecanique::Egalites,
        Mecanique::Affichage,
        Mecanique::Plateau,
        Mecanique::Fichiers,
    ];
}

/// La déclaration d'un jeu, possédée : ce que `Appel::Decrire` rend (D36).
///
/// Chaque champ est obligatoire, et aucun n'a de valeur par défaut. Une exemption
/// est une valeur avec sa raison, jamais une absence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclarationFil {
    /// Le nom du jeu.
    pub nom: TexteCourt,
    /// Combien de joueurs et d'appareils.
    pub places: Places,
    /// Les pas rattrapés au plus après un gel (SQ-62).
    pub rattrapage: u8,
    /// La fenêtre du passé provisoire.
    pub fenetre: ApplicableFil<Duree>,
    /// L'antidatage permis au-delà de la demi-latence mesurée (SAL-129).
    pub antidatage_max: Duree,
    /// Qui met en pause.
    pub pause: PauseFil,
    /// Ce que montre l'écran.
    pub ecran: FormeEcran,
    /// Les téléphones.
    pub telephones: ApplicableFil<TelephonesFil>,
    /// Les manettes, et les règles de leur état tenu.
    pub manettes: ApplicableFil<ReglesTenuFil>,
    /// La prédiction sur l'appareil.
    pub prediction: ApplicableFil<()>,
    /// Le hasard.
    pub hasard: ApplicableFil<()>,
    /// Les secrets.
    pub secrets: ApplicableFil<()>,
    /// Une fin de partie.
    pub fin: ApplicableFil<()>,
    /// Les capacités que le jeu sait employer.
    pub capacites: Voulues,
    /// Les limites.
    pub limites: Limites,
    /// La touche du panneau de diagnostic (§ 6.4).
    pub diagnostic: CodeTouche,
    /// Les assets.
    pub assets: Vec<AssetFil>,
    /// Les mécaniques.
    pub mecaniques: Vec<Mecanique>,
}

/// Si la partie accueille un nouvel appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accueil {
    /// Elle accueille.
    Ouverte,
    /// Toutes les places sont prises.
    Pleine,
    /// Elle n'accueille plus.
    Fermee,
}

/// Un geste peut-il s'afficher avant confirmation ?
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prevision {
    /// Oui : l'appareil le prédit.
    Oui,
    /// Non : il attend la confirmation.
    Non,
}
