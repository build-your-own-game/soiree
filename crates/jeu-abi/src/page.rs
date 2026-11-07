//! La page du téléphone, sur le fil (PHASE-3, § 2.10, P19 ; ABI 4).
//!
//! La page est une fonction du module : `Appel::Page` lui donne la vue reçue, son pas
//! et ce que le joueur fait en ce moment sur sa page ([`LocauxFil`]) ; elle rend un
//! arbre **en flux** ([`PageFil`]), que le commun dessine en DOM sur le téléphone. Elle
//! ne lit pas l'état, ne prédit rien et ne gère pas la connexion. `Recue`, `Locaux` et
//! `Page`, qui portent les types du jeu, vivent dans `jeu`.
//!
//! # Ce que le commun en vérifie
//!
//! Une page vient du module : le commun la valide comme une entrée venue de
//! l'extérieur (REM-57), d'abord au décodage (chaque nœud après son parent, et ce
//! parent est un conteneur : la liste à plat rend la page non récursive, comme la
//! scène), puis par [`valider_page`] : nombre de nœuds et profondeur bornés, clés
//! uniques parmi les frères, textes, options, colonnes et tailles bornés, images et
//! polices déclarées, gestes et tenus sous `limites.geste`, ce que la page retient
//! sous [`RETENU_MAX`]. Que chaque geste et chaque tenu se décode pour le jeu, c'est
//! `Appel::Valider` qui le dit (E21).
//!
//! # Le carrousel de Sidequest (lot 6)
//!
//! Son cahier (§ 6.1) veut le héros au centre, ses voisins de chaque côté **dans leur
//! couleur**, et des accessoires dont le second se dessine **par-dessus** le premier.
//! Une page en flux n'en a pas les moyens sans un asset par combinaison. D'où, à
//! l'ABI 4, [`SorteNoeudPage::Pile`] (des enfants superposés dans la même boîte, dans
//! l'ordre) et la `teinte` de [`SorteNoeudPage::Image`], appliquée par le commun.

use alloc::string::String;
use alloc::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::appareils::{Capacites, Orientation};
use crate::declaration::{AssetFil, Limites, SorteAsset};
use crate::scene::{Alignement, Cle, CompteAReboursFil, Unite};
use crate::valeurs::{HorsBornes, Oui, Rgba};

/// Choisi : le plus de nœuds d'une page. Un téléphone de 360 px ne montre guère plus
/// d'une centaine d'éléments ; la borne protège le DOM d'une page qui s'emballe.
pub const NOEUDS_PAGE_MAX: usize = 256;
/// Choisi : la profondeur la plus grande d'une page (une colonne, des rangées, une
/// pile : quelques étages suffisent).
pub const PROFONDEUR_PAGE_MAX: usize = 16;
/// Choisi : les octets d'un texte, d'un nom de bouton, d'un exemple ou d'une option.
pub const TEXTE_PAGE_MAX: usize = 1024;
/// Choisi : les options d'un choix.
pub const OPTIONS_MAX: usize = 64;
/// Choisi : les colonnes d'une grille.
pub const COLONNES_MAX: u8 = 12;
/// Choisi : la taille d'un texte ou d'une image en pixels CSS, de 8 à 512.
pub const PX_PAGE: (u16, u16) = (8, 512);
/// Ce qu'une page retient pour une salle et un jeu, au plus (PHASE-3, § 2.10 : 1 Kio).
pub const RETENU_MAX: usize = 1024;

/// L'écran du téléphone, tel que la page le voit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcranTelephone {
    /// La largeur visible, en pixels CSS.
    pub largeur: u16,
    /// La hauteur visible, en pixels CSS : le clavier la réduit (`visualViewport`).
    pub hauteur: u16,
    /// Les marges sûres (`env(safe-area-inset-*)`).
    pub marges: Marges,
    /// L'orientation de l'écran.
    pub orientation: Orientation,
}

impl EcranTelephone {
    /// Un téléphone de 360 × 640, en portrait, sans marge : celui des essais.
    pub const ORDINAIRE: EcranTelephone = EcranTelephone {
        largeur: 360,
        hauteur: 640,
        marges: Marges::AUCUNE,
        orientation: Orientation::Portrait,
    };
}

/// Les marges sûres d'un écran, en pixels CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marges {
    /// En haut.
    pub haut: u16,
    /// À droite.
    pub droite: u16,
    /// En bas.
    pub bas: u16,
    /// À gauche.
    pub gauche: u16,
}

impl Marges {
    /// Aucune marge.
    pub const AUCUNE: Marges = Marges {
        haut: 0,
        droite: 0,
        bas: 0,
        gauche: 0,
    };
}

/// Une direction de croix, ou le centre (rien d'appuyé).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction8 {
    /// Rien d'appuyé.
    Centre,
    /// En haut.
    Haut,
    /// En haut à droite.
    HautDroite,
    /// À droite.
    Droite,
    /// En bas à droite.
    BasDroite,
    /// En bas.
    Bas,
    /// En bas à gauche.
    BasGauche,
    /// À gauche.
    Gauche,
    /// En haut à gauche.
    HautGauche,
}

impl Direction8 {
    /// Toutes, dans l'ordre.
    pub const TOUTES: [Direction8; 9] = [
        Direction8::Centre,
        Direction8::Haut,
        Direction8::HautDroite,
        Direction8::Droite,
        Direction8::BasDroite,
        Direction8::Bas,
        Direction8::BasGauche,
        Direction8::Gauche,
        Direction8::HautGauche,
    ];
}

/// Les directions d'une croix (REM-121 à REM-124).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Directions {
    /// Haut, droite, bas, gauche.
    Quatre,
    /// Avec les diagonales.
    Huit,
}

/// Quand un bouton envoie son geste. Un buzzer réagit au contact (REM-125).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Declencheur {
    /// Au contact du doigt.
    Contact,
    /// Au relâchement, sur le bouton.
    Relache,
}

/// Le clavier qu'un champ demande au téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Clavier {
    /// Un texte libre.
    Texte,
    /// Un prénom : majuscule au début, sans correction.
    Prenom,
    /// Des chiffres.
    Nombre,
}

/// Ce qu'une page n'a pas pu envoyer ou retenir : jamais une perte silencieuse
/// (INTERFACE § 6.6 ; SAL-07, OUT-197).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Envoi {
    /// Au-delà de la borne.
    TropGros,
    /// La file des gestes est pleine.
    FilePleine,
}

/// La taille d'un texte, ou la hauteur d'une image, sur la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaillePage {
    /// En pixels CSS, de `PX_PAGE.0` à `PX_PAGE.1`. Un champ en dessous de 16 px est
    /// dessiné à 16 px (sinon iOS zoome).
    Px(u16),
    /// Une part de la hauteur visible, de 1 à 100 pour cent.
    Hauteur(u8),
}

/// L'allure d'un élément de la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StylePage {
    /// La police, par sa clé dans les assets, ou celle du commun.
    pub police: Cle,
    /// Sa taille.
    pub taille: TaillePage,
    /// La couleur du texte, ou celle du commun.
    pub couleur: Option<Rgba>,
    /// La couleur du fond, ou aucune.
    pub fond: Option<Rgba>,
    /// L'alignement du texte.
    pub alignement: Alignement,
}

impl StylePage {
    /// La police du commun, 20 px, couleurs du commun, alignée au début.
    pub const ORDINAIRE: StylePage = StylePage {
        police: Cle::POLICE_DU_COMMUN,
        taille: TaillePage::Px(20),
        couleur: None,
        fond: None,
        alignement: Alignement::Debut,
    };
}

/// Ce que le joueur fait en ce moment sur sa page : le commun le garde entre deux
/// appels, pas le jeu (PHASE-3, § 2.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocauxFil<'a> {
    /// L'écran.
    pub ecran: EcranTelephone,
    /// Le texte brut de chaque champ, par sa clé ; le jeu le lit nettoyé.
    #[serde(borrow)]
    pub champs: Vec<(Cle, &'a str)>,
    /// L'option choisie de chaque choix, par sa clé.
    pub choix: Vec<(Cle, u32)>,
    /// Les maintenus sous un doigt.
    pub appuyes: Vec<Cle>,
    /// La direction de chaque croix tenue.
    pub croix: Vec<(Cle, Direction8)>,
    /// Ce que la page a retenu pour cette salle et ce jeu.
    #[serde(borrow)]
    pub retenu: Option<&'a [u8]>,
    /// Les capacités du téléphone, dites honnêtement.
    pub capacites: Capacites,
}

impl LocauxFil<'_> {
    /// Rien de saisi, rien d'appuyé, rien de retenu, sur un écran ordinaire.
    pub const fn vides() -> LocauxFil<'static> {
        LocauxFil {
            ecran: EcranTelephone::ORDINAIRE,
            champs: Vec::new(),
            choix: Vec::new(),
            appuyes: Vec::new(),
            croix: Vec::new(),
            retenu: None,
            capacites: Capacites::AUCUNE,
        }
    }
}

/// La page du téléphone, sur le fil : un arbre en flux, chaque nœud après son parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "PageBrute")]
pub struct PageFil {
    /// Les nœuds, chacun après son parent.
    pub noeuds: Vec<NoeudPage>,
    /// L'état tenu que la page déclare maintenant, et sa valeur relâchée, calculée
    /// dans le module par `Tenu::relache`.
    pub tenu: Option<TenuPage>,
    /// Ce que la page veut retenir pour cette salle et ce jeu.
    pub retenir: Option<Vec<u8>>,
}

impl PageFil {
    /// Une page vide : celle d'un jeu qui n'en écrit pas.
    pub const fn vide() -> PageFil {
        PageFil {
            noeuds: Vec::new(),
            tenu: None,
            retenir: None,
        }
    }

    /// La structure est-elle bien formée : chaque parent précède ses enfants, et
    /// c'est un conteneur ?
    pub fn verifier_structure(&self) -> Result<(), HorsBornes> {
        verifier_structure(&self.noeuds)
    }
}

fn verifier_structure(noeuds: &[NoeudPage]) -> Result<(), HorsBornes> {
    for (i, n) in noeuds.iter().enumerate() {
        let Some(p) = n.parent else { continue };
        let parent = usize::try_from(p)
            .ok()
            .filter(|p| *p < i)
            .and_then(|p| noeuds.get(p));
        match parent {
            Some(q) if q.sorte.conteneur() => {}
            Some(_) => {
                return Err(HorsBornes {
                    attendu: "un parent qui est une colonne, une rangée, une grille ou une pile",
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

/// La forme sur le fil d'une [`PageFil`], avant la vérification de sa structure.
#[derive(Deserialize)]
struct PageBrute {
    noeuds: Vec<NoeudPage>,
    tenu: Option<TenuPage>,
    retenir: Option<Vec<u8>>,
}

impl TryFrom<PageBrute> for PageFil {
    type Error = HorsBornes;

    fn try_from(b: PageBrute) -> Result<Self, Self::Error> {
        verifier_structure(&b.noeuds)?;
        Ok(PageFil {
            noeuds: b.noeuds,
            tenu: b.tenu,
            retenir: b.retenir,
        })
    }
}

/// Le tenu d'une page : sa valeur encodée, et sa valeur relâchée s'il se relâche seul.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenuPage {
    /// La valeur, encodée par le jeu.
    pub octets: Vec<u8>,
    /// Sa valeur relâchée, encodée, ou `None` s'il ne se relâche jamais seul.
    pub relache: Option<Vec<u8>>,
}

/// Un nœud de la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoeudPage {
    /// Sa clé, stable d'une page à l'autre : le rendu réconcilie par elle, jamais par
    /// position (SAL-165).
    pub cle: Cle,
    /// L'indice de son parent dans la liste, ou `None` à la racine.
    pub parent: Option<u32>,
    /// Ce qu'il est.
    pub sorte: SorteNoeudPage,
}

/// Ce qu'est un nœud de la page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SorteNoeudPage {
    /// Ses enfants, l'un sous l'autre.
    Colonne,
    /// Ses enfants, côte à côte.
    Rangee,
    /// Ses enfants, en grille de tant de colonnes.
    Grille {
        /// Les colonnes, de 1 à [`COLONNES_MAX`].
        colonnes: u8,
    },
    /// Ses enfants, superposés dans la même boîte, dans l'ordre : le dernier par-dessus.
    Pile,
    /// Un texte, posé par `textContent` (REM-56).
    Texte {
        /// Le texte.
        texte: String,
        /// Son allure.
        style: StylePage,
    },
    /// Une image déclarée dans les assets.
    Image {
        /// L'image.
        image: Cle,
        /// Sa hauteur ; sa largeur suit ses proportions.
        hauteur: TaillePage,
        /// Une teinte, appliquée par le commun, ou aucune.
        teinte: Option<Rgba>,
    },
    /// Un compte à rebours, dessiné par le commun (SQ-36).
    CompteARebours {
        /// Ce qu'il compte.
        compte: CompteAReboursFil,
        /// Son allure.
        style: StylePage,
    },
    /// Un bouton, et le geste qu'il envoie, déjà encodé : l'autorité le revalide.
    Bouton {
        /// Ce qui s'y lit.
        nom: String,
        /// Le geste du jeu, encodé.
        geste: Vec<u8>,
        /// Quand il part.
        quand: Declencheur,
        /// Son allure.
        style: StylePage,
    },
    /// Un bouton que le joueur tient : le commun le dit appuyé dans les locaux.
    Maintenu {
        /// Ce qui s'y lit.
        nom: String,
        /// Son allure.
        style: StylePage,
    },
    /// Une croix : le commun en dit la direction dans les locaux.
    Croix {
        /// Quatre ou huit directions.
        directions: Directions,
    },
    /// Un champ de texte d'une ligne ; le commun garde sa valeur dans les locaux.
    Champ {
        /// La limite du texte, dans son unité.
        limite: u32,
        /// L'unité de la limite.
        unite: Unite,
        /// Le texte d'exemple, quand il est vide.
        exemple: String,
        /// Le clavier demandé.
        clavier: Clavier,
        /// Le geste envoyé sur Entrée, encodé.
        entree: Option<Vec<u8>>,
    },
    /// Un choix parmi des options ; balayable, c'est un carrousel.
    Choix {
        /// Les options, chacune par sa clé.
        options: Vec<(Cle, String)>,
        /// Se balaie-t-il du doigt ?
        balayable: Oui,
    },
}

impl SorteNoeudPage {
    /// Ce nœud contient-il d'autres nœuds ?
    pub const fn conteneur(&self) -> bool {
        match self {
            SorteNoeudPage::Colonne
            | SorteNoeudPage::Rangee
            | SorteNoeudPage::Grille { .. }
            | SorteNoeudPage::Pile => true,
            SorteNoeudPage::Texte { .. }
            | SorteNoeudPage::Image { .. }
            | SorteNoeudPage::CompteARebours { .. }
            | SorteNoeudPage::Bouton { .. }
            | SorteNoeudPage::Maintenu { .. }
            | SorteNoeudPage::Croix { .. }
            | SorteNoeudPage::Champ { .. }
            | SorteNoeudPage::Choix { .. } => false,
        }
    }
}

/// Pourquoi une page est invalide. Le téléphone garde alors la page précédente et le
/// compte au diagnostic (PHASE-3, § 2.10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefautPage {
    /// Plus de [`NOEUDS_PAGE_MAX`] nœuds.
    TropDeNoeuds {
        /// Leur nombre.
        noeuds: usize,
    },
    /// Plus profond que [`PROFONDEUR_PAGE_MAX`].
    TropProfonde {
        /// Le nœud trop profond.
        cle: Cle,
    },
    /// Deux frères ont la même clé.
    CleEnDouble {
        /// La clé.
        cle: Cle,
    },
    /// Un texte, un nom, un exemple ou une option au-delà de [`TEXTE_PAGE_MAX`] octets.
    TexteTropLong {
        /// Le nœud.
        cle: Cle,
    },
    /// Une taille, des colonnes, une limite ou des options hors de leurs bornes.
    HorsBornes {
        /// Le nœud.
        cle: Cle,
        /// Ce qui est hors bornes.
        quoi: &'static str,
    },
    /// Une image ou une police qui n'est pas déclarée.
    AssetInconnu {
        /// Le nœud.
        cle: Cle,
        /// L'asset.
        asset: Cle,
    },
    /// Un geste au-delà de `limites.geste`.
    GesteHorsBornes {
        /// Le nœud.
        cle: Cle,
    },
    /// Un tenu au-delà de `limites.geste`.
    TenuHorsBornes,
    /// Ce que la page retient dépasse [`RETENU_MAX`].
    RetenuTropGros {
        /// Sa taille.
        octets: usize,
    },
}

impl core::fmt::Display for DefautPage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DefautPage::TropDeNoeuds { noeuds } => {
                write!(f, "{noeuds} nœuds, au-delà de {NOEUDS_PAGE_MAX}")
            }
            DefautPage::TropProfonde { cle } => write!(
                f,
                "le nœud {:x} est au-delà de {PROFONDEUR_PAGE_MAX} étages",
                cle.fil()
            ),
            DefautPage::CleEnDouble { cle } => {
                write!(f, "la clé {:x} est deux fois parmi des frères", cle.fil())
            }
            DefautPage::TexteTropLong { cle } => write!(
                f,
                "un texte du nœud {:x} dépasse {TEXTE_PAGE_MAX} octets",
                cle.fil()
            ),
            DefautPage::HorsBornes { cle, quoi } => {
                write!(f, "le nœud {:x} : {quoi} hors de ses bornes", cle.fil())
            }
            DefautPage::AssetInconnu { cle, asset } => write!(
                f,
                "le nœud {:x} cite l'asset {:x}, qui n'est pas déclaré",
                cle.fil(),
                asset.fil()
            ),
            DefautPage::GesteHorsBornes { cle } => {
                write!(f, "le geste du nœud {:x} dépasse limites.geste", cle.fil())
            }
            DefautPage::TenuHorsBornes => {
                write!(f, "le tenu de la page dépasse limites.geste")
            }
            DefautPage::RetenuTropGros { octets } => write!(
                f,
                "la page retient {octets} octets, au-delà de {RETENU_MAX}"
            ),
        }
    }
}

const fn taille_valide(t: TaillePage) -> bool {
    match t {
        TaillePage::Px(px) => px >= PX_PAGE.0 && px <= PX_PAGE.1,
        TaillePage::Hauteur(p) => p >= 1 && p <= 100,
    }
}

fn octets_valides(o: &[u8], limites: &Limites) -> bool {
    u32::try_from(o.len()).is_ok_and(|n| n <= limites.geste)
}

/// Une police déclarée (ou celle du commun) et une taille dans ses bornes.
fn valider_style(noeud: Cle, s: &StylePage, assets: &[AssetFil]) -> Result<(), DefautPage> {
    if s.police != Cle::POLICE_DU_COMMUN && !declare(assets, s.police, SorteAsset::Police) {
        return Err(DefautPage::AssetInconnu {
            cle: noeud,
            asset: s.police,
        });
    }
    if !taille_valide(s.taille) {
        return Err(DefautPage::HorsBornes {
            cle: noeud,
            quoi: "la taille du texte",
        });
    }
    Ok(())
}

fn declare(assets: &[AssetFil], cle: Cle, sorte: SorteAsset) -> bool {
    assets.iter().any(|a| a.cle == cle && a.sorte == sorte)
}

fn valider_texte(noeud: Cle, t: &str) -> Result<(), DefautPage> {
    if t.len() > TEXTE_PAGE_MAX {
        Err(DefautPage::TexteTropLong { cle: noeud })
    } else {
        Ok(())
    }
}

/// Ce qu'un nœud porte : ses textes, son style, ses assets, ses gestes, ses bornes.
fn valider_noeud(n: &NoeudPage, assets: &[AssetFil], limites: &Limites) -> Result<(), DefautPage> {
    let hors_bornes = |quoi| DefautPage::HorsBornes { cle: n.cle, quoi };
    match &n.sorte {
        SorteNoeudPage::Colonne
        | SorteNoeudPage::Rangee
        | SorteNoeudPage::Pile
        | SorteNoeudPage::Croix { .. } => {}
        SorteNoeudPage::Grille { colonnes } => {
            if *colonnes == 0 || *colonnes > COLONNES_MAX {
                return Err(hors_bornes("les colonnes"));
            }
        }
        SorteNoeudPage::Texte { texte, style } => {
            valider_texte(n.cle, texte)?;
            valider_style(n.cle, style, assets)?;
        }
        SorteNoeudPage::Image { image, hauteur, .. } => {
            if !declare(assets, *image, SorteAsset::Image) {
                return Err(DefautPage::AssetInconnu {
                    cle: n.cle,
                    asset: *image,
                });
            }
            if !taille_valide(*hauteur) {
                return Err(hors_bornes("la hauteur de l'image"));
            }
        }
        SorteNoeudPage::CompteARebours { style, .. } => valider_style(n.cle, style, assets)?,
        SorteNoeudPage::Bouton {
            nom, geste, style, ..
        } => {
            valider_texte(n.cle, nom)?;
            valider_style(n.cle, style, assets)?;
            if !octets_valides(geste, limites) {
                return Err(DefautPage::GesteHorsBornes { cle: n.cle });
            }
        }
        SorteNoeudPage::Maintenu { nom, style } => {
            valider_texte(n.cle, nom)?;
            valider_style(n.cle, style, assets)?;
        }
        SorteNoeudPage::Champ {
            limite,
            exemple,
            entree,
            ..
        } => {
            valider_texte(n.cle, exemple)?;
            if *limite == 0 || usize::try_from(*limite).map_or(true, |l| l > TEXTE_PAGE_MAX) {
                return Err(hors_bornes("la limite du champ"));
            }
            if let Some(g) = entree
                && !octets_valides(g, limites)
            {
                return Err(DefautPage::GesteHorsBornes { cle: n.cle });
            }
        }
        SorteNoeudPage::Choix { options, .. } => {
            if options.is_empty() || options.len() > OPTIONS_MAX {
                return Err(hors_bornes("le nombre d'options"));
            }
            let mut vues: Vec<u64> = Vec::with_capacity(options.len());
            for (c, t) in options {
                valider_texte(n.cle, t)?;
                match vues.binary_search(&c.fil()) {
                    Ok(_) => return Err(DefautPage::CleEnDouble { cle: *c }),
                    Err(i) => vues.insert(i, c.fil()),
                }
            }
        }
    }
    Ok(())
}

/// Valide une page comme une entrée venue de l'extérieur (PHASE-3, § 2.10 ; REM-57) :
/// nombre de nœuds et profondeur bornés, clés uniques parmi les frères, textes,
/// options, colonnes, tailles et limites bornés, chaque image déclarée comme image et
/// chaque police comme police (ou celle du commun), chaque geste et le tenu sous
/// `limites.geste`, ce que la page retient sous [`RETENU_MAX`].
///
/// La structure (chaque parent avant ses enfants, et conteneur) est déjà tenue par le
/// décodage.
///
/// # Propriétés
///
/// - Une page vide est valide.
/// - Une page valide le reste si l'on retire une feuille.
pub fn valider_page(
    page: &PageFil,
    assets: &[AssetFil],
    limites: &Limites,
) -> Result<(), DefautPage> {
    let nombre = page.noeuds.len();
    if nombre > NOEUDS_PAGE_MAX {
        return Err(DefautPage::TropDeNoeuds { noeuds: nombre });
    }
    let mut profondeur: Vec<usize> = Vec::with_capacity(nombre);
    let mut freres: Vec<(Option<u32>, u64)> = Vec::with_capacity(nombre);
    for n in &page.noeuds {
        let p = n
            .parent
            .and_then(|p| usize::try_from(p).ok())
            .and_then(|p| profondeur.get(p).copied())
            .unwrap_or(0);
        let ici = p.saturating_add(1);
        if ici > PROFONDEUR_PAGE_MAX {
            return Err(DefautPage::TropProfonde { cle: n.cle });
        }
        profondeur.push(ici);
        let cle = (n.parent, n.cle.fil());
        match freres.binary_search(&cle) {
            Ok(_) => return Err(DefautPage::CleEnDouble { cle: n.cle }),
            Err(i) => freres.insert(i, cle),
        }
        valider_noeud(n, assets, limites)?;
    }
    if let Some(t) = &page.tenu
        && (!octets_valides(&t.octets, limites)
            || t.relache
                .as_ref()
                .is_some_and(|r| !octets_valides(r, limites)))
    {
        return Err(DefautPage::TenuHorsBornes);
    }
    if let Some(r) = &page.retenir
        && r.len() > RETENU_MAX
    {
        return Err(DefautPage::RetenuTropGros { octets: r.len() });
    }
    Ok(())
}
