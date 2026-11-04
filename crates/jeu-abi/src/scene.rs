//! Ce que le jeu montre (§ 2.9) : clés, effets, réglages d'un téléphone, et la
//! scène de l'écran dans sa forme sur le fil.
//!
//! `Scene<A>`, `Noeud`, `Montre<V>` et `Definitif<T>`, qui portent les types du
//! jeu, vivent dans `jeu` ; ils s'encodent en [`SceneFil`] et [`crate::MontreFil`].

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::appareils::Orientation;
use crate::format::empreinte_de;
use crate::temps::{Duree, Instant, Pas};
use crate::valeurs::{CodeTouche, Fixe, HorsBornes, Oui, Rect, Rgba, TexteCourt};

use crate::entrees::Mods;

/// Une clé stable, calculée depuis une valeur du jeu, jamais depuis une position
/// (SAL-165) : FNV-1a 64 de son encodage postcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Cle(u64);

impl Cle {
    /// La police que le commun fournit à tout jeu, sans asset déclaré. Sa seule
    /// définition : le cœur l'admet dans `valider_scene`, la colle s'en sert pour la
    /// scène de secours, et un jeu la nomme plutôt que d'en recopier le numéro
    /// (CONTRAT, « Deux morceaux qui s'accordent n'ont qu'une définition »).
    pub const POLICE_DU_COMMUN: Cle = Cle(0);

    /// La clé de cette valeur : la même valeur donne toujours la même clé.
    pub fn de<T: Serialize + ?Sized>(v: &T) -> Cle {
        Cle(empreinte_de(v))
    }

    /// La clé d'un nom, évaluable à la compilation : la même que `Cle::de(nom)`, le
    /// FNV-1a 64 de l'encodage postcard d'un texte (sa longueur en varint, puis ses
    /// octets). Pour une clé dans une constante, comme celles des assets d'une
    /// `Declaration` (besoin 5 des jeux, `conception/BESOINS-DES-JEUX.md`).
    pub const fn nommee(nom: &str) -> Cle {
        let h =
            crate::format::fnv1a64_varint(crate::format::Fnv1a::new().valeur(), nom.len() as u64);
        Cle(crate::format::fnv1a64_suite(h, nom.as_bytes()))
    }

    /// La clé d'un numéro, évaluable à la compilation : la même que `Cle::de(&n)` pour
    /// un `u64` (son varint).
    pub const fn numero(n: u64) -> Cle {
        Cle(crate::format::fnv1a64_varint(
            crate::format::Fnv1a::new().valeur(),
            n,
        ))
    }

    /// Réservé au commun : la clé de ce numéro, relue sur le fil. Un jeu écrit
    /// [`Cle::nommee`] ou [`Cle::numero`].
    #[doc(hidden)]
    pub const fn du_fil(n: u64) -> Cle {
        Cle(n)
    }

    /// Le numéro sur le fil.
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// L'identité sémantique d'un effet : la même chose à jouer porte la même identité,
/// et un réaffichage, une reconnexion ou un rejeu ne la rejoue pas (PUN-66, SQ-79).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct IdEffet(u64);

impl IdEffet {
    /// L'identité de cette valeur : FNV-1a 64 de son encodage postcard.
    pub fn de<T: Serialize + ?Sized>(v: &T) -> IdEffet {
        IdEffet(empreinte_de(v))
    }

    /// L'identité d'un nom, évaluable à la compilation : la même que `IdEffet::de(nom)`.
    pub const fn nomme(nom: &str) -> IdEffet {
        IdEffet(Cle::nommee(nom).fil())
    }

    /// Le numéro sur le fil.
    pub const fn fil(self) -> u64 {
        self.0
    }
}

/// Un effet ponctuel : un son, une vibration. C'est une ligne de la vue, comme le reste.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Effet {
    /// Son identité sémantique.
    pub id: IdEffet,
    /// L'instant où il doit se produire, sur l'horloge de la partie.
    pub quand: Instant,
    /// Ce qui se produit.
    pub quoi: Sensation,
    /// Sur quelle instance il se lit (§ 4.4).
    pub tenue: Tenue,
    /// Au-delà de `quand` plus cette durée, l'effet ne se joue plus (SAL-155).
    pub peremption: Duree,
}

/// Sur quelle instance un effet se lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tenue {
    /// Sur la tête : il peut être annulé si la tête ne le montre plus.
    Provisoire,
    /// Sur l'instance définitive seulement.
    Definitive,
}

/// Ce qu'un effet fait sentir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sensation {
    /// Un son.
    Son(Son),
    /// Une vibration.
    Vibration(Vibration),
}

/// Un son.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Son {
    /// Un signal synthétisé par le commun.
    Signal(Signal),
    /// Un son déclaré dans les assets, par sa clé.
    Asset(Cle),
}

/// Les signaux synthétisés par le commun.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Signal {
    /// Quelque chose commence.
    Debut,
    /// La partie avance.
    Avance,
    /// Un résultat tombe.
    Resultat,
    /// C'est fini.
    Fin,
}

/// Les vibrations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vibration {
    /// Une vibration brève.
    Courte,
    /// Une vibration de réussite.
    Succes,
}

/// Les réglages d'un téléphone, envoyés avec sa vue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reglages {
    /// Garder l'écran allumé, si l'appareil le peut.
    pub ecran_allume: Oui,
    /// La couleur de fond de la page.
    pub fond: Option<Rgba>,
    /// L'orientation voulue, si le jeu en veut une.
    pub orientation: Option<Orientation>,
}

impl Reglages {
    /// Rien de particulier : l'écran peut s'éteindre, fond et orientation libres.
    pub const ORDINAIRES: Reglages = Reglages {
        ecran_allume: Oui::Non,
        fond: None,
        orientation: None,
    };
}

/// L'unité dans laquelle une limite de texte se compte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Unite {
    /// Des capitales : un « ß » compte pour deux (« SS »).
    Capitales,
    /// Des unités UTF-16, comme le navigateur les compte.
    Utf16,
    /// Des points de code.
    PointsDeCode,
    /// Des graphèmes, ce que le joueur voit comme un caractère.
    Graphemes,
}

// ---------------------------------------------------------------------------
// La scène sur le fil
// ---------------------------------------------------------------------------

/// La scène de l'écran, telle qu'elle voyage : une liste à plat de nœuds, chacun
/// après son parent. Elle est recalculée depuis l'état à chaque lecture (§ 2.9).
///
/// La liste à plat rend la scène non récursive : la décoder ne peut pas faire
/// déborder la pile du moteur. Le décodage refuse un nœud dont le parent ne le
/// précède pas, ou n'est ni un groupe ni un sous-arbre définitif. Le reste de la
/// validation (clés uniques parmi les frères, bornes, repère, ressources connues,
/// § 6.1) appartient au moteur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SceneBrute")]
pub struct SceneFil {
    /// La couleur de fond, si la scène en a une.
    pub fond: Option<Rgba>,
    /// Les nœuds, chacun après son parent.
    pub noeuds: Vec<NoeudFil>,
    /// Les raccourcis du clavier que la scène porte (SAL-152).
    pub raccourcis: Vec<RaccourciFil>,
    /// Le clavier brut, si la scène le demande.
    pub clavier_brut: ClavierBrut,
    /// Les effets de la scène.
    pub effets: Vec<Effet>,
}

impl SceneFil {
    /// Une scène vide.
    pub const fn vide() -> SceneFil {
        SceneFil {
            fond: None,
            noeuds: Vec::new(),
            raccourcis: Vec::new(),
            clavier_brut: ClavierBrut::Non,
            effets: Vec::new(),
        }
    }

    /// La structure est-elle bien formée : chaque parent précède ses enfants, et
    /// c'est un groupe ou un sous-arbre définitif ?
    pub fn verifier_structure(&self) -> Result<(), HorsBornes> {
        verifier_structure(&self.noeuds)
    }
}

fn verifier_structure(noeuds: &[NoeudFil]) -> Result<(), HorsBornes> {
    for (i, n) in noeuds.iter().enumerate() {
        let Some(p) = n.parent else { continue };
        let parent = usize::try_from(p)
            .ok()
            .filter(|p| *p < i)
            .and_then(|p| noeuds.get(p));
        match parent.map(|q| &q.sorte) {
            Some(SorteNoeud::Groupe { .. } | SorteNoeud::Definitif) => {}
            Some(
                SorteNoeud::Rect { .. }
                | SorteNoeud::Texte { .. }
                | SorteNoeud::Image { .. }
                | SorteNoeud::Bouton { .. }
                | SorteNoeud::CompteARebours { .. }
                | SorteNoeud::Qr { .. }
                | SorteNoeud::Composant(_)
                | SorteNoeud::Avertissements { .. }
                | SorteNoeud::Surface { .. },
            ) => {
                return Err(HorsBornes {
                    attendu: "un parent qui est un groupe ou un sous-arbre définitif",
                });
            }
            None => {
                return Err(HorsBornes {
                    attendu: "un parent qui précède son enfant",
                });
            }
        }
    }
    Ok(())
}

/// La forme sur le fil d'une [`SceneFil`], avant la vérification de sa structure.
#[derive(Deserialize)]
struct SceneBrute {
    fond: Option<Rgba>,
    noeuds: Vec<NoeudFil>,
    raccourcis: Vec<RaccourciFil>,
    clavier_brut: ClavierBrut,
    effets: Vec<Effet>,
}

impl TryFrom<SceneBrute> for SceneFil {
    type Error = HorsBornes;

    fn try_from(b: SceneBrute) -> Result<Self, Self::Error> {
        verifier_structure(&b.noeuds)?;
        Ok(SceneFil {
            fond: b.fond,
            noeuds: b.noeuds,
            raccourcis: b.raccourcis,
            clavier_brut: b.clavier_brut,
            effets: b.effets,
        })
    }
}

/// Un nœud de la scène.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoeudFil {
    /// Sa clé, stable d'une scène à l'autre : la fusion et le glisser passent par elle.
    pub cle: Cle,
    /// L'indice de son parent dans la liste, ou `None` à la racine.
    pub parent: Option<u32>,
    /// Ce qu'il est.
    pub sorte: SorteNoeud,
    /// Glisse entre deux pas, par sa clé.
    pub glisse: Oui,
    /// Ne glisse pas à ce pas (TL-23).
    pub rupture: Oui,
    /// Aligné sur les pixels entiers (SQ-52).
    pub pixel_entier: Oui,
    /// Lisse une correction de prédiction sur cette durée.
    pub lisse: Option<Duree>,
    /// Une animation, partie d'un instant de la partie (REM-12).
    pub anime: Option<(Instant, Animation)>,
    /// L'opacité, de 0 à 255.
    pub opacite: u8,
}

impl NoeudFil {
    /// Un nœud sans modificateur : ni glisse, ni rupture, ni animation, opaque.
    pub const fn simple(cle: Cle, parent: Option<u32>, sorte: SorteNoeud) -> NoeudFil {
        NoeudFil {
            cle,
            parent,
            sorte,
            glisse: Oui::Non,
            rupture: Oui::Non,
            pixel_entier: Oui::Non,
            lisse: None,
            anime: None,
            opacite: u8::MAX,
        }
    }
}

/// Ce qu'est un nœud de la scène.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorteNoeud {
    /// Un rectangle plein.
    Rect {
        /// Où.
        r: Rect,
        /// Sa couleur.
        couleur: Rgba,
    },
    /// Un texte dans une boîte.
    Texte {
        /// Le texte.
        texte: String,
        /// La boîte où il tient.
        boite: Rect,
        /// Son style.
        style: StyleTexte,
    },
    /// Une image, nommée par une clé, jamais par une adresse (TL-15).
    Image {
        /// L'image.
        image: Cle,
        /// Où.
        r: Rect,
        /// Comment l'agrandir.
        ech: Echantillonnage,
    },
    /// Un groupe : ses enfants sont décalés ensemble.
    Groupe {
        /// Le décalage des enfants.
        decalage: (Fixe, Fixe),
    },
    /// Un bouton, et l'action qu'il envoie : rien d'autre ne s'envoie (SAL-152).
    Bouton {
        /// Où.
        boite: Rect,
        /// Ce qui s'y lit.
        nom: String,
        /// L'action du jeu, encodée.
        action: Vec<u8>,
    },
    /// Un compte à rebours, dessiné par le commun (SQ-36, CAT-99).
    CompteARebours {
        /// Ce qu'il compte.
        compte: CompteAReboursFil,
        /// Où.
        boite: Rect,
        /// Son style.
        style: StyleTexte,
    },
    /// Le code QR de l'adresse que les téléphones joignent vraiment (SQ-16).
    Qr {
        /// Où.
        boite: Rect,
    },
    /// Un composant riche : champ, objets déplaçables, sélecteur de fichiers…
    Composant(ComposantVuFil),
    /// Où le commun écrit ses avertissements, dont les refus de l'écran (TL-71).
    Avertissements {
        /// Où.
        boite: Rect,
    },
    /// Une surface de pointeur brut, sans défilement natif.
    Surface {
        /// Où.
        r: Rect,
    },
    /// Un sous-arbre qui ne vient que de l'instance définitive, par clé (§ 4.4).
    Definitif,
}

/// Un raccourci du clavier de l'écran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaccourciFil {
    /// La touche.
    pub touche: CodeTouche,
    /// Les modificateurs.
    pub mods: Mods,
    /// La répétition automatique déclenche-t-elle aussi l'action ?
    pub repetition: Oui,
    /// L'action du jeu, encodée.
    pub action: Vec<u8>,
}

/// Un compte à rebours, sur le fil : l'échéance est un pas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompteAReboursFil {
    /// Il court jusqu'à ce pas.
    Court {
        /// L'échéance.
        echeance: Pas,
    },
    /// Il est figé, avec ce qui reste.
    Fige {
        /// Ce qui reste.
        restant: Duree,
    },
}

/// Le style d'un texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StyleTexte {
    /// La police, par sa clé dans les assets.
    pub police: Cle,
    /// Sa taille.
    pub taille: Taille,
    /// Sa couleur.
    pub couleur: Rgba,
    /// Son alignement.
    pub alignement: Alignement,
    /// Combien de lignes.
    pub lignes: Lignes,
    /// Où couper.
    pub coupe: Coupe,
}

/// La taille d'un texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Taille {
    /// Une taille fixe.
    Fixe(Fixe),
    /// Une taille qui rétrécit pour tenir, entre deux bornes (TL-55, PUN-53).
    Budget {
        /// La plus grande.
        max: Fixe,
        /// La plus petite.
        min: Fixe,
    },
}

/// Combien de lignes un texte peut prendre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Lignes {
    /// Une seule.
    Une,
    /// Au plus ce nombre.
    AuPlus(u8),
    /// Autant qu'il faut.
    Libre,
}

/// Où couper un texte trop long pour sa ligne.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Coupe {
    /// Entre les mots ; le signe isolé reste avec son mot (SQ-46).
    Mots,
    /// Entre deux graphèmes.
    Graphemes,
}

/// L'alignement d'un texte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Alignement {
    /// Au début.
    Debut,
    /// Au centre.
    Centre,
    /// À la fin.
    Fin,
}

/// Comment agrandir une image.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Echantillonnage {
    /// En lissant.
    Lisse,
    /// Au plus proche, pour des pixels nets.
    PlusProche,
}

/// Une animation d'un nœud, d'un visuel à un autre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Animation {
    /// Sa durée.
    pub duree: Duree,
    /// D'où elle part.
    pub de: Visuel,
    /// Où elle arrive.
    pub a: Visuel,
    /// Sa courbe.
    pub courbe: Courbe,
}

/// L'allure d'un nœud à un moment d'une animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visuel {
    /// Le décalage.
    pub decalage: (Fixe, Fixe),
    /// L'échelle, en millièmes.
    pub echelle_pour_mille: u16,
    /// L'opacité, de 0 à 255.
    pub opacite: u8,
}

/// La courbe d'une animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Courbe {
    /// Linéaire.
    Lineaire,
    /// Lente au départ.
    Entree,
    /// Lente à l'arrivée.
    Sortie,
    /// Lente aux deux bouts.
    EntreeSortie,
    /// Avec un rebond.
    Rebond,
}

/// Le clavier brut de l'écran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClavierBrut {
    /// Pas de touches brutes : seuls les raccourcis de la scène.
    Non,
    /// Les touches brutes arrivent au jeu.
    Oui {
        /// Les répétitions automatiques aussi.
        repetition: Oui,
    },
}

/// Un composant riche de la scène, sur le fil : ses actions sont encodées.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComposantVuFil {
    /// Un champ de texte, qui reste un vrai élément du navigateur (TL-33, PUN-40).
    Champ {
        /// La limite du texte, dans son unité (TL-27).
        limite: u32,
        /// L'unité de la limite.
        unite: Unite,
        /// Combien de lignes.
        lignes: Lignes,
        /// Sa valeur.
        valeur: String,
        /// Le texte d'exemple, quand il est vide.
        exemple: String,
        /// L'action envoyée à la validation, encodée.
        fin: Option<Vec<u8>>,
    },
    /// Un texte qu'on édite sur place.
    EnPlace {
        /// La limite du texte, dans son unité.
        limite: u32,
        /// L'unité de la limite.
        unite: Unite,
        /// Sa valeur.
        valeur: String,
        /// Son style.
        style: StyleTexte,
    },
    /// Des objets déplaçables entre des zones (TL-36 à TL-47).
    Deplacables {
        /// Les zones, avec leurs objets dans l'ordre de l'état.
        zones: Vec<ZoneVu>,
        /// La zone où un objet déposé disparaît.
        poubelle: Option<Cle>,
        /// La distance avant qu'un appui devienne un glisser.
        seuil: Fixe,
        /// La largeur du bord qui fait défiler.
        bord: Fixe,
        /// La vitesse de défilement la plus grande, par seconde.
        vitesse_max: Fixe,
        /// Le plafond du défilement au réveil.
        plafond_reveil: Duree,
    },
    /// Un sélecteur de fichiers.
    Selecteur {
        /// Les types acceptés.
        genres: Vec<TexteCourt>,
        /// Plusieurs fichiers à la fois.
        multiple: Oui,
        /// Accepter un dépôt n'importe où sur la page.
        depot_partout: Oui,
        /// Accepter un collage.
        collage: Oui,
        /// La taille la plus grande d'un fichier, en octets.
        taille_max: u64,
    },
    /// Une fenêtre modale.
    Modale {
        /// L'action envoyée à sa fermeture, encodée.
        fermer: Vec<u8>,
    },
    /// Un lien de téléchargement d'une ressource.
    Lien {
        /// Ce qui s'y lit.
        nom: TexteCourt,
        /// La ressource.
        ressource: Cle,
    },
}

/// Une zone d'objets déplaçables.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneVu {
    /// La zone.
    pub cle: Cle,
    /// Ses objets, dans l'ordre de l'état.
    pub objets: Vec<Cle>,
}
