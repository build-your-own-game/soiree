//! Les polices que la mesure connaît : celle du commun, et celles que le jeu déclare,
//! validées (P22). Jamais une police du système : la mesure de l'épreuve et le dessin de
//! la télé doivent voir les mêmes.

use std::fmt;
use std::sync::Arc;

use cosmic_text::fontdb;
use jeu_abi::Cle;

/// La police du commun (`Cle::POLICE_DU_COMMUN`) : Liberation Sans 2.1.5, romain, sous
/// SIL OFL 1.1 (`polices/OFL-Liberation.txt`). Large couverture latine, le grec, le
/// cyrillique et l'hébreu ; pas de CJK, pas d'emoji : un glyphe absent se dessine comme
/// une boîte, et se compte ([`crate::Agencement::absents`]).
///
/// D'où elle vient : la copie que Tier list a vérifiée (son `BESOINS-COMMUN.md`, § 17 :
/// celle de `LibreOfficeDev`, signature de code valide, table `name` « Version 2.1.5 » et
/// « SIL Open Font License, Version 1.1 »), au même sha256
/// (`76d04c18ea243f426b7de1f3ad208e927008f961dc5945e5aad352d0dfde8ee8`, testé).
pub const OCTETS_DU_COMMUN: &[u8] = include_bytes!("../polices/LiberationSans-Regular.ttf");

/// Le nom de famille de la police du commun, tel que sa table `name` le dit. C'est la
/// première police de repli : un glyphe que la police d'un texte n'a pas se cherche
/// d'abord là.
pub const FAMILLE_DU_COMMUN: &str = "Liberation Sans";

/// Pourquoi une police est refusée (P22 ; E1 le dit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusPolice {
    /// Aucun octet.
    Vide,
    /// Une police WOFF (`wOFF`) : le commun ne lit que les fichiers sfnt.
    Woff,
    /// Une police WOFF2 (`wOF2`) : le commun ne lit que les fichiers sfnt.
    Woff2,
    /// Une collection (`ttcf`) : une police se déclare par fichier.
    Collection,
    /// Des octets qui ne commencent par aucune signature sfnt.
    PasSfnt {
        /// Les quatre premiers octets, ou moins.
        debut: Vec<u8>,
    },
    /// Une signature sfnt, mais des tables que le lecteur de polices ne lit pas.
    Illisible,
    /// La même famille, la même graisse, le même style et la même chasse qu'une autre
    /// police déjà connue, avec d'autres octets : la mesure ne saurait pas laquelle prendre.
    Homonyme {
        /// La police déjà connue.
        autre: Cle,
        /// Sa famille.
        famille: String,
    },
}

impl fmt::Display for RefusPolice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusPolice::Vide => f.write_str("la police n'a aucun octet"),
            RefusPolice::Woff => f.write_str(
                "une police WOFF : le commun ne lit que les polices sfnt, TTF ou OTF (PHASE-3, P22) ; convertissez-la",
            ),
            RefusPolice::Woff2 => f.write_str(
                "une police WOFF2 : le commun ne lit que les polices sfnt, TTF ou OTF (PHASE-3, P22) ; convertissez-la",
            ),
            RefusPolice::Collection => f.write_str(
                "une collection de polices (ttcf) : déclarez chaque police dans son fichier TTF ou OTF",
            ),
            RefusPolice::PasSfnt { debut } => write!(
                f,
                "pas une police sfnt : elle commence par {debut:02x?}, au lieu de 00 01 00 00, « true » ou « OTTO »"
            ),
            RefusPolice::Illisible => {
                f.write_str("une signature sfnt, mais des tables que le lecteur de polices ne lit pas")
            }
            RefusPolice::Homonyme { autre, famille } => write!(
                f,
                "même famille (« {famille} »), même graisse et même style que la police {:016x}, avec d'autres octets : la mesure ne saurait pas laquelle prendre",
                autre.fil()
            ),
        }
    }
}

/// Une police connue : sa clé, ses octets, et ce qui la désigne dans la base de
/// polices.
#[derive(Debug, Clone)]
pub(crate) struct Face {
    pub(crate) cle: Cle,
    pub(crate) octets: Arc<Vec<u8>>,
    pub(crate) famille: String,
    pub(crate) graisse: fontdb::Weight,
    pub(crate) style: fontdb::Style,
    pub(crate) chasse: fontdb::Stretch,
}

/// La signature d'un fichier sfnt : vérifiée avant toute lecture des tables.
fn signature(octets: &[u8]) -> Result<(), RefusPolice> {
    let Some(debut) = octets.get(..4) else {
        return if octets.is_empty() {
            Err(RefusPolice::Vide)
        } else {
            Err(RefusPolice::PasSfnt {
                debut: octets.to_vec(),
            })
        };
    };
    match debut {
        [0, 1, 0, 0] | b"true" | b"OTTO" => Ok(()),
        b"wOFF" => Err(RefusPolice::Woff),
        b"wOF2" => Err(RefusPolice::Woff2),
        b"ttcf" => Err(RefusPolice::Collection),
        autre => Err(RefusPolice::PasSfnt {
            debut: autre.to_vec(),
        }),
    }
}

/// Lit une police sfnt : exactement une face, que le lecteur de polices de la mesure
/// sait lire.
fn lire(cle: Cle, octets: Arc<Vec<u8>>) -> Result<Face, RefusPolice> {
    signature(&octets)?;
    let mut base = fontdb::Database::new();
    let ids = base.load_font_source(fontdb::Source::Binary(octets.clone()));
    let [id] = ids.as_slice() else {
        return Err(RefusPolice::Illisible);
    };
    let info = base.face(*id).ok_or(RefusPolice::Illisible)?;
    let famille = info
        .families
        .first()
        .map(|(nom, _)| nom.clone())
        .ok_or(RefusPolice::Illisible)?;
    Ok(Face {
        cle,
        octets,
        famille,
        graisse: info.weight,
        style: info.style,
        chasse: info.stretch,
    })
}

/// La police est-elle un fichier sfnt (TTF ou OTF) que la mesure sait lire ? C'est la
/// règle de E1 (P22), et celle de la télé quand elle charge les polices d'un jeu.
pub fn valider_police(octets: &[u8]) -> Result<(), RefusPolice> {
    lire(Cle::POLICE_DU_COMMUN, Arc::new(octets.to_vec())).map(|_| ())
}

/// Les polices d'une mesure : celle du commun d'abord, puis celles du jeu, dans l'ordre
/// où elles s'ajoutent. L'ordre compte : c'est celui où la mesure cherche un glyphe
/// absent, après la police du texte et celle du commun ; la télé ajoute les polices
/// dans l'ordre de la déclaration, comme l'épreuve.
#[derive(Debug, Clone)]
pub struct Polices {
    pub(crate) faces: Vec<Face>,
}

impl Polices {
    /// La police du commun, seule.
    pub fn du_commun() -> Polices {
        let commun = lire(Cle::POLICE_DU_COMMUN, Arc::new(OCTETS_DU_COMMUN.to_vec()));
        // La police du commun est lue par `la_police_du_commun_se_lit` : ce cas ne se
        // produit pas, et une mesure sans elle se replierait sur aucune police.
        Polices {
            faces: commun.into_iter().collect(),
        }
    }

    /// Ajoute une police du jeu, sous sa clé : refusée si elle n'est pas sfnt (P22), ou si
    /// une autre police connue porte son nom, sa graisse et son style avec d'autres octets.
    pub fn ajouter(&mut self, cle: Cle, octets: Vec<u8>) -> Result<(), RefusPolice> {
        let face = lire(cle, Arc::new(octets))?;
        if let Some(autre) = self.faces.iter().find(|f| {
            f.famille == face.famille
                && f.graisse == face.graisse
                && f.style == face.style
                && f.chasse == face.chasse
                && f.octets != face.octets
        }) {
            return Err(RefusPolice::Homonyme {
                autre: autre.cle,
                famille: autre.famille.clone(),
            });
        }
        self.faces.retain(|f| f.cle != cle);
        self.faces.push(face);
        Ok(())
    }

    /// Les clés des polices connues, celle du commun d'abord.
    pub fn cles(&self) -> impl Iterator<Item = Cle> + '_ {
        self.faces.iter().map(|f| f.cle)
    }

    /// La famille d'une police connue.
    pub fn famille(&self, cle: Cle) -> Option<&str> {
        self.faces
            .iter()
            .find(|f| f.cle == cle)
            .map(|f| f.famille.as_str())
    }
}
