//! L'agencement d'un texte dans sa boîte : la coupe en lignes, la taille, la mesure.
//!
//! # La règle
//!
//! - **La taille** est la taille du cadratin, dans le repère de la scène. **Une ligne fait
//!   [`INTERLIGNE`] fois la taille** (1,2, choisi : la hauteur « normale » usuelle).
//! - **Les lignes.** Le texte se coupe d'abord à chaque fin de ligne forcée (`\n`, `\r`,
//!   U+2028, U+2029 et les autres séparateurs de paragraphe), puis, sauf en
//!   `Lignes::Une`, partout où sa `Coupe` le permet, au plus tard possible : une ligne
//!   prend autant que la boîte en laisse passer. `Coupe::Mots` coupe aux occasions de
//!   UAX #14 ; un mot plus large que la boîte reste entier sur sa ligne, et déborde.
//!   `Coupe::Graphemes` coupe entre deux graphèmes, jamais dans un graphème (TL-55).
//! - **Le signe isolé reste avec son mot** (SQ-46) : en `Coupe::Mots`, une espace (U+0020
//!   ou U+00A0) devant `?`, `!`, `:` ou `;` devient une espace fine insécable (U+202F).
//! - **La largeur d'une ligne** est l'étendue de ses glyphes, sans les espaces qui la
//!   finissent : elles sont retirées du texte agencé, comme celles du début d'une ligne
//!   coupée.
//! - **Le texte tient** quand chaque ligne est au plus aussi large que la boîte, que les
//!   lignes sont au plus aussi nombreuses que `Lignes` le permet, et que leur hauteur
//!   (leur nombre fois l'interligne) est au plus celle de la boîte. L'encre d'un glyphe
//!   qui dépasse sa ligne (un accent sur une capitale) ne compte pas.
//! - **`Taille::Budget { max, min }`** : la plus grande taille, au 1/256 près, à laquelle
//!   le texte tient ; cherchée par dichotomie entre `min` et `max`, ce qui suppose qu'un
//!   texte qui tient à une taille tient à toute taille plus petite (vrai sans hinting :
//!   les chasses sont proportionnelles à la taille). S'il ne tient pas à `min`, il se
//!   mesure à `min`, et déborde.
//! - **La place** : les lignes s'alignent dans la largeur de la boîte selon
//!   l'`Alignement`, et leur bloc se centre dans sa hauteur.
//!
//! Le texte agencé ([`Agencement::texte`]) est le texte coupé, une ligne par `\n` : la
//! télé le donne tel quel à `cosmic-text`, sans coupe automatique, à la taille et à
//! l'interligne de l'agencement, et dessine ainsi exactement ce que l'épreuve a mesuré
//! (PHASE-3, § 2.9 ; vérifié glyphe par glyphe au lot 9).

use std::collections::BTreeMap;
use std::fmt;

use cosmic_text::{
    Align, Attrs, Buffer, Fallback, Family, FontSystem, Metrics, Shaping, Wrap, fontdb,
};
use jeu_abi::{Alignement, Cle, Coupe, Fixe, Lignes, Oui, Rect, StyleTexte, Taille};
use unicode_segmentation::UnicodeSegmentation;

use crate::polices::{FAMILLE_DU_COMMUN, Face, Polices};

/// La hauteur d'une ligne, en fois la taille (choisi : la hauteur « normale » usuelle,
/// 1,2 cadratin). Le rendu de la télé la reprend.
pub const INTERLIGNE: f32 = 1.2;

/// Au-delà de ce nombre d'agencements gardés, la mise en page oublie tout et
/// recommence (choisi : de quoi tenir les textes d'un écran de Tier list plein).
pub const AGENCEMENTS_GARDES_MAX: usize = 4096;

/// La langue de la mesure : elle choisit, à police égale, des variantes de glyphes
/// propres à une langue. Fixée, pour ne jamais dépendre de la machine.
const LANGUE: &str = "fr-FR";

/// Pourquoi un texte ne tient pas dans sa boîte.
#[derive(Debug, Clone, PartialEq)]
pub enum Debord {
    /// Le style ne se mesure pas : une taille nulle ou négative, un `min` au-dessus du
    /// `max`, une boîte de taille négative.
    StyleInvalide(&'static str),
    /// Plus de lignes que `Lignes` n'en permet.
    TropDeLignes {
        /// Les lignes du texte.
        lignes: u32,
        /// Celles que le style permet.
        permises: u32,
    },
    /// Une ligne plus large que la boîte.
    TropLarge {
        /// La ligne la plus large, en unités du repère.
        largeur: f32,
        /// La largeur de la boîte.
        boite: f32,
    },
    /// Des lignes plus hautes, ensemble, que la boîte.
    TropHaut {
        /// Leur hauteur, en unités du repère.
        hauteur: f32,
        /// La hauteur de la boîte.
        boite: f32,
    },
}

impl fmt::Display for Debord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Debord::StyleInvalide(pourquoi) => write!(f, "style qui ne se mesure pas : {pourquoi}"),
            Debord::TropDeLignes { lignes, permises } => {
                write!(f, "{lignes} lignes, pour {permises} permise(s)")
            }
            Debord::TropLarge { largeur, boite } => write!(
                f,
                "une ligne de {largeur:.2} unités de large, pour une boîte de {boite:.2}"
            ),
            Debord::TropHaut { hauteur, boite } => write!(
                f,
                "des lignes de {hauteur:.2} unités de haut, pour une boîte de {boite:.2}"
            ),
        }
    }
}

/// Le texte tient-il dans sa boîte ?
#[derive(Debug, Clone, PartialEq)]
pub enum Tient {
    /// Oui.
    Oui,
    /// Non, et pourquoi.
    Non(Debord),
}

/// Un glyphe placé.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlypheAgence {
    /// La police qui le dessine : celle du texte, ou une police de repli.
    pub police: Cle,
    /// Son numéro dans la police ; 0 est le glyphe absent, dessiné comme une boîte.
    pub glyphe: u16,
    /// Son bord gauche, depuis le bord gauche de la boîte.
    pub x: f32,
    /// Sa chasse.
    pub chasse: f32,
}

/// Une ligne placée.
#[derive(Debug, Clone, PartialEq)]
pub struct LigneAgencee {
    /// Son bord gauche, depuis le bord gauche de la boîte.
    pub gauche: f32,
    /// Sa largeur : l'étendue de ses glyphes.
    pub largeur: f32,
    /// Son haut, depuis le haut de la boîte.
    pub haut: f32,
    /// Ses glyphes, dans l'ordre visuel.
    pub glyphes: Vec<GlypheAgence>,
}

/// Un texte agencé dans sa boîte.
#[derive(Debug, Clone, PartialEq)]
pub struct Agencement {
    /// Le texte coupé, une ligne par `\n`, tel que la télé le dessine.
    pub texte: String,
    /// La taille retenue.
    pub taille: Fixe,
    /// La hauteur d'une ligne, en unités du repère.
    pub interligne: f32,
    /// Les lignes.
    pub lignes: Vec<LigneAgencee>,
    /// Tient-il ?
    pub tient: Tient,
    /// Les glyphes absents de toutes les polices connues, dessinés comme des boîtes :
    /// le diagnostic les compte (PHASE-3, § 2.9).
    pub absents: u32,
    /// La police du style n'est pas connue (refusée, ou jamais déclarée) : le texte est
    /// mesuré dans la police du commun, comme la télé le dessinerait.
    pub police_inconnue: Oui,
}

impl Agencement {
    /// Un texte vide, ou fait d'espaces : aucune ligne, et il tient.
    fn vide(taille: Fixe, interligne: f32, police_inconnue: Oui) -> Agencement {
        Agencement {
            texte: String::new(),
            taille,
            interligne,
            lignes: Vec::new(),
            tient: Tient::Oui,
            absents: 0,
            police_inconnue,
        }
    }

    /// Un style qui ne se mesure pas.
    fn invalide(taille: Fixe, pourquoi: &'static str) -> Agencement {
        Agencement {
            texte: String::new(),
            taille,
            interligne: 0.0,
            lignes: Vec::new(),
            tient: Tient::Non(Debord::StyleInvalide(pourquoi)),
            absents: 0,
            police_inconnue: Oui::Non,
        }
    }
}

/// Les polices de repli : celle du commun, puis toute autre police connue, dans l'ordre
/// où elles ont été ajoutées. Jamais une liste propre à la plateforme : la télé
/// (`wasm32`) et l'épreuve (macOS) cherchent les glyphes au même endroit.
struct Repli;

impl Fallback for Repli {
    fn common_fallback(&self) -> &[&'static str] {
        &[FAMILLE_DU_COMMUN]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, _: unicode_script::Script, _: &str) -> &[&'static str] {
        &[]
    }
}

/// Ce qui désigne une police dans la base de la mesure.
#[derive(Debug, Clone)]
struct Designation {
    cle: Cle,
    id: Option<fontdb::ID>,
    famille: String,
    graisse: fontdb::Weight,
    style: fontdb::Style,
    chasse: fontdb::Stretch,
}

/// La clé d'un agencement gardé : ce dont l'agencement dépend, et rien d'autre (ni la
/// clé du nœud, ni la couleur).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CleAgencement {
    texte: String,
    police: u64,
    taille: (u8, i32, i32),
    alignement: u8,
    lignes: (u8, u8),
    coupe: u8,
    boite: (i32, i32),
}

impl CleAgencement {
    fn de(texte: &str, boite: Rect, style: &StyleTexte) -> CleAgencement {
        CleAgencement {
            texte: String::from(texte),
            police: style.police.fil(),
            taille: match style.taille {
                Taille::Fixe(t) => (0, t.brut(), 0),
                Taille::Budget { max, min } => (1, max.brut(), min.brut()),
            },
            alignement: match style.alignement {
                Alignement::Debut => 0,
                Alignement::Centre => 1,
                Alignement::Fin => 2,
            },
            lignes: match style.lignes {
                Lignes::Une => (0, 1),
                Lignes::AuPlus(n) => (1, n),
                Lignes::Libre => (2, 0),
            },
            coupe: match style.coupe {
                Coupe::Mots => 0,
                Coupe::Graphemes => 1,
            },
            boite: (boite.l.brut(), boite.h.brut()),
        }
    }
}

/// La mise en page : les polices chargées, et les agencements déjà faits.
pub struct MiseEnPage {
    systeme: FontSystem,
    polices: Vec<Designation>,
    gardes: BTreeMap<CleAgencement, Agencement>,
}

impl fmt::Debug for MiseEnPage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MiseEnPage")
            .field("polices", &self.polices)
            .field("gardes", &self.gardes.len())
            .finish_non_exhaustive()
    }
}

/// Une longueur du repère, en flottant.
fn unites(f: Fixe) -> f32 {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "une longueur du repère tient dans un f32 au 1/256 près jusqu'à 2^16 unités ; au-delà, l'arrondi ne change pas ce qui tient"
    )]
    let x = (f64::from(f.brut()) / 256.0) as f32;
    x
}

/// Le plus petit de deux flottants, écrit à la main : `f32::min` ne traite pas le zéro
/// signé pareil en natif et dans WebAssembly (`clippy.toml`).
fn plus_petit(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// Le plus grand de deux flottants, écrit à la main (même raison).
fn plus_grand(a: f32, b: f32) -> f32 {
    if b > a { b } else { a }
}

/// Un compte, en flottant.
fn nombre(n: usize) -> f32 {
    f32::from(u16::try_from(n).unwrap_or(u16::MAX))
}

/// Les fins de ligne forcées : celles où `cosmic-text` coupe un paragraphe (séparateurs
/// de paragraphe du bidi), plus U+000B, U+000C et U+2028, que UAX #14 force aussi.
const fn fin_forcee(c: char) -> bool {
    matches!(
        c,
        '\n' | '\r' | '\u{0B}' | '\u{0C}' | '\u{1C}'
            ..='\u{1E}' | '\u{85}' | '\u{2028}' | '\u{2029}'
    )
}

/// Le texte préparé : chaque fin de ligne forcée devient `\n` (`\r\n` n'en fait qu'une) ;
/// en `Coupe::Mots`, l'espace devant un signe isolé devient insécable et fine (SQ-46).
pub(crate) fn preparer(texte: &str, coupe: Coupe) -> String {
    let mut sortie = String::with_capacity(texte.len());
    let mut caracteres = texte.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '\r' && caracteres.peek() == Some(&'\n') {
            continue;
        }
        if fin_forcee(c) {
            sortie.push('\n');
            continue;
        }
        let signe_isole = matches!(caracteres.peek(), Some('?' | '!' | ':' | ';'));
        if coupe == Coupe::Mots && matches!(c, ' ' | '\u{a0}') && signe_isole {
            sortie.push('\u{202f}');
        } else {
            sortie.push(c);
        }
    }
    sortie
}

/// Les endroits d'un paragraphe où une ligne peut finir, croissants, la fin comprise.
fn coupures(paragraphe: &str, coupe: Coupe) -> Vec<usize> {
    match coupe {
        Coupe::Mots => unicode_linebreak::linebreaks(paragraphe)
            .map(|(i, _)| i)
            .filter(|i| *i > 0)
            .collect(),
        Coupe::Graphemes => paragraphe
            .grapheme_indices(true)
            .map(|(i, g)| i.saturating_add(g.len()))
            .collect(),
    }
}

/// La largeur des glyphes d'un paragraphe façonné d'un bloc, pour chercher où couper :
/// pour chaque grappe, son début dans le paragraphe et sa chasse, rangés par début, et
/// les chasses cumulées.
struct Chasses {
    debuts: Vec<usize>,
    cumul: Vec<f32>,
}

impl Chasses {
    fn de(mut grappes: Vec<(usize, f32)>) -> Chasses {
        grappes.sort_by_key(|(debut, _)| *debut);
        let mut cumul = Vec::with_capacity(grappes.len().saturating_add(1));
        let mut total = 0.0_f32;
        cumul.push(total);
        for (_, w) in &grappes {
            total += *w;
            cumul.push(total);
        }
        Chasses {
            debuts: grappes.into_iter().map(|(d, _)| d).collect(),
            cumul,
        }
    }

    /// La largeur des grappes qui commencent dans `[a, b)`.
    fn entre(&self, a: usize, b: usize) -> f32 {
        let i = self.debuts.partition_point(|d| *d < a);
        let j = self.debuts.partition_point(|d| *d < b);
        let fin = self.cumul.get(j).copied().unwrap_or(0.0);
        let debut = self.cumul.get(i).copied().unwrap_or(0.0);
        fin - debut
    }
}

/// La longueur de `s` sans les espaces qui le finissent.
fn sans_espaces_finales(s: &str) -> usize {
    s.trim_end().len()
}

/// Coupe un paragraphe en lignes d'au plus `largeur`, au plus tard possible.
fn couper(paragraphe: &str, chasses: &Chasses, coupe: Coupe, largeur: f32) -> Vec<String> {
    let fins = coupures(paragraphe, coupe);
    let mut lignes = Vec::new();
    let mut debut = 0;
    while debut < paragraphe.len() {
        let mut choisie = None;
        for &fin in fins.iter().filter(|f| **f > debut) {
            let morceau = paragraphe.get(debut..fin).unwrap_or("");
            let visible = debut.saturating_add(sans_espaces_finales(morceau));
            if chasses.entre(debut, visible) <= largeur {
                choisie = Some(fin);
            } else {
                // Rien ne tient : le premier morceau, seul sur sa ligne, déborde.
                choisie = choisie.or(Some(fin));
                break;
            }
        }
        let fin = choisie.unwrap_or(paragraphe.len());
        lignes.push(String::from(
            paragraphe.get(debut..fin).unwrap_or("").trim_end(),
        ));
        // Une ligne coupée ne commence pas par des espaces.
        let reste = paragraphe.get(fin..).unwrap_or("");
        debut = fin.saturating_add(reste.len().saturating_sub(reste.trim_start().len()));
    }
    if lignes.is_empty() {
        lignes.push(String::new());
    }
    lignes
}

impl MiseEnPage {
    /// Une mise en page sur ces polices, et aucune autre.
    pub fn new(polices: &Polices) -> MiseEnPage {
        let mut base = fontdb::Database::new();
        let mut designations = Vec::with_capacity(polices.faces.len());
        for face in &polices.faces {
            let Face {
                cle,
                octets,
                famille,
                graisse,
                style,
                chasse,
            } = face;
            let id = base
                .load_font_source(fontdb::Source::Binary(octets.clone()))
                .first()
                .copied();
            designations.push(Designation {
                cle: *cle,
                id,
                famille: famille.clone(),
                graisse: *graisse,
                style: *style,
                chasse: *chasse,
            });
        }
        MiseEnPage {
            systeme: FontSystem::new_with_locale_and_db_and_fallback(
                String::from(LANGUE),
                base,
                Repli,
            ),
            polices: designations,
            gardes: BTreeMap::new(),
        }
    }

    /// La désignation d'une police, celle du commun si elle n'est pas connue ; et si elle
    /// ne l'était pas.
    fn designation(&self, cle: Cle) -> (Option<&Designation>, Oui) {
        match self.polices.iter().find(|d| d.cle == cle) {
            Some(d) => (Some(d), Oui::Non),
            None => (
                self.polices.iter().find(|d| d.cle == Cle::POLICE_DU_COMMUN),
                if cle == Cle::POLICE_DU_COMMUN {
                    Oui::Non
                } else {
                    Oui::Oui
                },
            ),
        }
    }

    /// La clé de la police d'un glyphe.
    fn police_du_glyphe(&self, id: fontdb::ID) -> Cle {
        self.polices
            .iter()
            .find(|d| d.id == Some(id))
            .map_or(Cle::POLICE_DU_COMMUN, |d| d.cle)
    }

    /// Agence un texte dans sa boîte, selon son style. Le même texte, dans la même boîte
    /// et le même style, s'agence toujours pareil ; l'agencement est gardé.
    pub fn agencer(&mut self, texte: &str, boite: Rect, style: &StyleTexte) -> Agencement {
        let cle = CleAgencement::de(texte, boite, style);
        if let Some(a) = self.gardes.get(&cle) {
            return a.clone();
        }
        let a = self.agencer_sans_garder(texte, boite, style);
        if self.gardes.len() >= AGENCEMENTS_GARDES_MAX {
            self.gardes.clear();
        }
        let _: Option<Agencement> = self.gardes.insert(cle, a.clone());
        a
    }

    fn agencer_sans_garder(&mut self, texte: &str, boite: Rect, style: &StyleTexte) -> Agencement {
        if boite.l.brut() < 0 || boite.h.brut() < 0 {
            return Agencement::invalide(Fixe::ZERO, "une boîte de taille négative");
        }
        let prepare = preparer(texte, style.coupe);
        match style.taille {
            Taille::Fixe(t) if t.brut() <= 0 => {
                Agencement::invalide(t, "une taille nulle ou négative")
            }
            Taille::Fixe(t) => self.essayer(&prepare, boite, style, t),
            Taille::Budget { min, .. } if min.brut() <= 0 => {
                Agencement::invalide(min, "un min nul ou négatif")
            }
            Taille::Budget { max, min } if min.brut() > max.brut() => {
                Agencement::invalide(min, "un min au-dessus du max")
            }
            Taille::Budget { max, min } => {
                let au_max = self.essayer(&prepare, boite, style, max);
                if au_max.tient == Tient::Oui {
                    return au_max;
                }
                let au_min = self.essayer(&prepare, boite, style, min);
                if au_min.tient != Tient::Oui {
                    return au_min;
                }
                // Dichotomie : `bas` tient, `haut` ne tient pas.
                let (mut bas, mut haut, mut meilleur) = (min.brut(), max.brut(), au_min);
                while haut.saturating_sub(bas) > 1 {
                    let milieu = bas.saturating_add(haut.saturating_sub(bas) / 2);
                    let essai = self.essayer(&prepare, boite, style, Fixe::depuis_brut(milieu));
                    if essai.tient == Tient::Oui {
                        bas = milieu;
                        meilleur = essai;
                    } else {
                        haut = milieu;
                    }
                }
                meilleur
            }
        }
    }

    /// Façonne des lignes sans les couper, à une taille : un passage par ligne.
    fn faconner(
        &mut self,
        texte: &str,
        taille: f32,
        largeur: Option<f32>,
        alignement: Option<Align>,
        designation: Option<&Designation>,
    ) -> Buffer {
        let mut tampon = Buffer::new(&mut self.systeme, Metrics::new(taille, taille * INTERLIGNE));
        tampon.set_wrap(Wrap::None);
        tampon.set_size(largeur, None);
        let attributs = match designation {
            Some(d) => Attrs::new()
                .family(Family::Name(&d.famille))
                .weight(d.graisse)
                .style(d.style)
                .stretch(d.chasse),
            None => Attrs::new().family(Family::Name(FAMILLE_DU_COMMUN)),
        };
        tampon.set_text(texte, &attributs, Shaping::Advanced, alignement);
        tampon.shape_until_scroll(&mut self.systeme, false);
        tampon
    }

    /// Les lignes placées d'un texte façonné, et ses glyphes absents.
    fn placer(&self, tampon: &Buffer, haut0: f32, interligne: f32) -> (Vec<LigneAgencee>, u32) {
        let mut agencees = Vec::new();
        let mut absents: u32 = 0;
        for passe in tampon.layout_runs() {
            let gauche = passe
                .glyphs
                .iter()
                .map(|g| g.x)
                .fold(f32::INFINITY, plus_petit);
            let droite = passe
                .glyphs
                .iter()
                .map(|g| g.x + g.w)
                .fold(f32::NEG_INFINITY, plus_grand);
            let (gauche, largeur) = if passe.glyphs.is_empty() {
                (0.0, 0.0)
            } else {
                (gauche, droite - gauche)
            };
            let glyphes: Vec<GlypheAgence> = passe
                .glyphs
                .iter()
                .map(|g| GlypheAgence {
                    police: self.police_du_glyphe(g.font_id),
                    glyphe: g.glyph_id,
                    x: g.x,
                    chasse: g.w,
                })
                .collect();
            absents = absents.saturating_add(
                u32::try_from(glyphes.iter().filter(|g| g.glyphe == 0).count()).unwrap_or(u32::MAX),
            );
            agencees.push(LigneAgencee {
                gauche,
                largeur,
                haut: haut0 + nombre(agencees.len()) * interligne,
                glyphes,
            });
        }
        (agencees, absents)
    }

    /// Les lignes d'un texte préparé, à une taille : ses paragraphes, coupés selon sa
    /// `Coupe` sauf en `Lignes::Une`.
    fn lignes(
        &mut self,
        prepare: &str,
        taille: f32,
        largeur: f32,
        style: &StyleTexte,
        designation: Option<&Designation>,
    ) -> Vec<String> {
        match style.lignes {
            Lignes::Une => prepare
                .split('\n')
                .map(|p| String::from(p.trim_end()))
                .collect(),
            Lignes::AuPlus(_) | Lignes::Libre => {
                let bloc = self.faconner(prepare, taille, None, None, designation);
                let paragraphes: Vec<&str> = prepare.split('\n').collect();
                let mut grappes: Vec<Vec<(usize, f32)>> = vec![Vec::new(); paragraphes.len()];
                for passe in bloc.layout_runs() {
                    if let Some(g) = grappes.get_mut(passe.line_i) {
                        g.extend(passe.glyphs.iter().map(|x| (x.start, x.w)));
                    }
                }
                paragraphes
                    .iter()
                    .zip(grappes)
                    .flat_map(|(p, g)| couper(p, &Chasses::de(g), style.coupe, largeur))
                    .collect()
            }
        }
    }

    /// Agence à une taille donnée, et dit si le texte tient.
    fn essayer(
        &mut self,
        prepare: &str,
        boite: Rect,
        style: &StyleTexte,
        taille: Fixe,
    ) -> Agencement {
        let taille_u = unites(taille);
        let (largeur, hauteur_boite) = (unites(boite.l), unites(boite.h));
        let interligne = taille_u * INTERLIGNE;
        let (designation, police_inconnue) = self.designation(style.police);
        let designation = designation.cloned();
        if prepare.trim().is_empty() {
            return Agencement::vide(taille, interligne, police_inconnue);
        }
        let lignes = self.lignes(prepare, taille_u, largeur, style, designation.as_ref());
        let texte = lignes.join("\n");
        let alignement = match style.alignement {
            Alignement::Debut => Align::Left,
            Alignement::Centre => Align::Center,
            Alignement::Fin => Align::Right,
        };
        let tampon = self.faconner(
            &texte,
            taille_u,
            Some(largeur),
            Some(alignement),
            designation.as_ref(),
        );
        let compte = lignes.len();
        let hauteur = nombre(compte) * interligne;
        let haut0 = (hauteur_boite - hauteur) / 2.0;
        let (agencees, absents) = self.placer(&tampon, haut0, interligne);
        let permises = match style.lignes {
            Lignes::Une => 1,
            Lignes::AuPlus(k) => u32::from(k),
            Lignes::Libre => u32::MAX,
        };
        let plus_large = agencees.iter().map(|x| x.largeur).fold(0.0_f32, plus_grand);
        let tient = if u32::try_from(compte).unwrap_or(u32::MAX) > permises {
            Tient::Non(Debord::TropDeLignes {
                lignes: u32::try_from(compte).unwrap_or(u32::MAX),
                permises,
            })
        } else if plus_large > largeur {
            Tient::Non(Debord::TropLarge {
                largeur: plus_large,
                boite: largeur,
            })
        } else if hauteur > hauteur_boite {
            Tient::Non(Debord::TropHaut {
                hauteur,
                boite: hauteur_boite,
            })
        } else {
            Tient::Oui
        };
        Agencement {
            texte,
            taille,
            interligne,
            lignes: agencees,
            tient,
            absents,
            police_inconnue,
        }
    }
}
