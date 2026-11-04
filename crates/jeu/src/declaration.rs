//! La déclaration (§ 2.12) : chaque champ est obligatoire, et `Declaration` n'a pas de
//! `Default`. Un champ ajouté par une version du commun casse la compilation de chaque
//! jeu, là où il faut répondre. Une exemption est une valeur avec sa raison, jamais une
//! absence (D27 ; HIS-04, HIS-43, OUT-03, OUT-04, OUT-05).

use alloc::string::String;
use alloc::vec::Vec;

use jeu_abi::{
    ApplicableFil, AssetFil, Cle, CodeTouche, DeclarationFil, Duree, FormeEcran, Limites,
    Mecanique, PauseFil, Places, ReglesTenuFil, Repartition, SorteAsset, TelephonesFil, TexteCourt,
    Voulues,
};

/// Une réponse qui s'applique, ou qui est sans objet avec sa raison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicable<T> {
    /// La réponse.
    Oui(T),
    /// Sans objet, et pourquoi.
    SansObjet(Raison),
}

impl<T> Applicable<T> {
    /// S'applique-t-elle ? En `const`.
    pub const fn oui(&self) -> bool {
        matches!(self, Applicable::Oui(_))
    }

    pub(crate) fn fil<U>(&self, f: impl FnOnce(&T) -> U) -> ApplicableFil<U> {
        match self {
            Applicable::Oui(v) => ApplicableFil::Oui(f(v)),
            Applicable::SansObjet(r) => ApplicableFil::SansObjet(String::from(r.0)),
        }
    }
}

/// Une raison qui dit quelque chose : au moins trois mots. Vérifiée à la compilation par
/// [`raison!`](crate::raison).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Raison(&'static str);

impl Raison {
    /// Une raison de trois mots au moins. Dans une constante, une raison creuse ne
    /// compile pas ; [`raison!`](crate::raison) l'y met.
    pub const fn new(s: &'static str) -> Raison {
        assert!(
            jeu_abi::raison_suffisante(s),
            "raison! : une raison dit quelque chose, en trois mots au moins (HIS-04, OUT-03)"
        );
        Raison(s)
    }

    /// Le texte de la raison.
    pub const fn texte(&self) -> &'static str {
        self.0
    }
}

/// Une raison vérifiée à la compilation : `raison!("tour par tour, le pas vu suffit")`.
#[macro_export]
macro_rules! raison {
    ($s:literal) => {{
        const R: $crate::Raison = $crate::Raison::new($s);
        R
    }};
}

/// Qui met la partie en pause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pause {
    /// Le jeu, dans ses règles.
    DuJeu,
    /// Le commun : plus de `Temps`, et chaque tenu relâché, cause `Pause`.
    DuCommun,
    /// Aucune pause, et pourquoi.
    Aucune(Raison),
}

/// Les téléphones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Telephones {
    /// Combien par joueur.
    pub repartition: Repartition,
    /// Les règles de leurs états tenus.
    pub tenu: ReglesTenu,
}

/// Les règles d'un état tenu (§ 4.3), appliquées par le cœur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReglesTenu {
    /// Relâché quand le silence de l'appareil dépasse cette durée (SQ-01) : à 450 ms
    /// pile, une direction tient encore.
    pub fraicheur: Applicable<Duree>,
    /// Un état plus vieux ne compte pas ; un relâchement passe toujours ; et la valeur
    /// tenue se relâche quand sa dernière redite par l'appareil a cet âge (SQ-02).
    pub age_max: Applicable<Duree>,
    /// Par seconde, par clé et par appareil (PUN-03).
    pub debit_max: u16,
}

impl ReglesTenu {
    fn fil(&self) -> ReglesTenuFil {
        ReglesTenuFil {
            fraicheur: self.fraicheur.fil(|d| *d),
            age_max: self.age_max.fil(|d| *d),
            debit_max: self.debit_max,
        }
    }
}

/// Un asset déclaré : ses octets sont compilés dans le module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Asset {
    /// Sa clé, par laquelle une scène le cite.
    pub cle: Cle,
    /// Ce qu'il est.
    pub sorte: SorteAsset,
    /// Ses octets.
    pub octets: &'static [u8],
}

/// La déclaration d'un jeu. Chaque champ est obligatoire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Declaration {
    /// Le nom du jeu : 64 octets au plus.
    pub nom: &'static str,
    /// Combien de joueurs et d'appareils.
    pub places: Places,
    /// Les pas rattrapés au plus après un gel (SQ-62).
    pub rattrapage: u8,
    /// La fenêtre du passé provisoire.
    pub fenetre: Applicable<Duree>,
    /// L'antidatage permis au-delà de la demi-latence mesurée (SAL-129).
    pub antidatage_max: Duree,
    /// Qui met en pause.
    pub pause: Pause,
    /// Ce que montre l'écran.
    pub ecran: FormeEcran,
    /// Les téléphones.
    pub telephones: Applicable<Telephones>,
    /// Les manettes, et les règles de leur état tenu.
    pub manettes: Applicable<ReglesTenu>,
    /// La prédiction sur l'appareil.
    pub prediction: Applicable<()>,
    /// Le hasard.
    pub hasard: Applicable<()>,
    /// Les secrets.
    pub secrets: Applicable<()>,
    /// Une fin de partie.
    pub fin: Applicable<()>,
    /// Les capacités que le jeu sait employer ; aucune n'est requise (REM-146, OUT-211).
    pub capacites: Voulues,
    /// Les limites.
    pub limites: Limites,
    /// La touche du panneau de diagnostic (§ 6.4).
    pub diagnostic: CodeTouche,
    /// Les assets.
    pub assets: &'static [Asset],
    /// Les mécaniques.
    pub mecaniques: &'static [Mecanique],
}

impl Declaration {
    /// Sa forme possédée, que rend `Appel::Decrire` (D36).
    pub fn fil(&self) -> DeclarationFil {
        DeclarationFil {
            nom: TexteCourt::essayer(self.nom).unwrap_or(TexteCourt::VIDE),
            places: self.places,
            rattrapage: self.rattrapage,
            fenetre: self.fenetre.fil(|d| *d),
            antidatage_max: self.antidatage_max,
            pause: match self.pause {
                Pause::DuJeu => PauseFil::DuJeu,
                Pause::DuCommun => PauseFil::DuCommun,
                Pause::Aucune(r) => PauseFil::Aucune(String::from(r.0)),
            },
            ecran: self.ecran,
            telephones: self.telephones.fil(|t| TelephonesFil {
                repartition: t.repartition,
                tenu: t.tenu.fil(),
            }),
            manettes: self.manettes.fil(ReglesTenu::fil),
            prediction: self.prediction.fil(|()| ()),
            hasard: self.hasard.fil(|()| ()),
            secrets: self.secrets.fil(|()| ()),
            fin: self.fin.fil(|()| ()),
            capacites: self.capacites,
            limites: self.limites,
            diagnostic: self.diagnostic,
            assets: self
                .assets
                .iter()
                .map(|a| AssetFil {
                    cle: a.cle,
                    sorte: a.sorte,
                    taille: a.octets.len() as u64,
                })
                .collect::<Vec<_>>(),
            mecaniques: self.mecaniques.to_vec(),
        }
    }
}
