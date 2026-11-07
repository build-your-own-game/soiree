//! La garde (PHASE-3, § 2.4, P9) : ce que l'autorité écrit au relais pour repartir, et
//! comment elle le relit.
//!
//! **Une chaîne de cases**, numérotées d'une seule suite, points et journal mêlés. Chaque
//! case est scellée par [`crate::chiffre::sceller_case`] sous la clé de garde G ; ses
//! données associées portent la salle, sa sorte, son numéro et l'empreinte de la case
//! précédente. Le relais ne peut ni lire, ni modifier, ni intervertir, ni rejouer une
//! case ; il peut seulement retenir les dernières, et l'autorité refuse une garde plus
//! vieille que le dernier numéro qu'elle sait écrit ([`relire`], `dernier_n`).
//!
//! - [`Case::Point`] : l'ouverture, le point de reprise de la partie
//!   ([`partie::persistance::encoder_point`]) et le [`Registre`] de la salle.
//! - [`Case::Journal`] : les lignes du journal qui suivent ([`partie::persistance`],
//!   un morceau à base absolue), et le registre tel que ces lignes l'ont fait.
//!
//! **Un point se prend entre deux pas, sur un journal écrit jusqu'à lui** : l'autorité
//! écrit d'abord les lignes jusqu'à la fin du dernier pas, puis le point, dont l'indice
//! est cette fin. Les cases de journal qui le suivent commencent donc à son indice, et
//! la relecture n'a besoin que du dernier point et de ce qui le suit. Après chaque point
//! écrit, l'autorité efface ce qui précède le point d'avant (`VersRelais::Effacer`) : la
//! garde tient au plus deux points et les cases entre eux.
//!
//! **Le registre et les lignes voyagent ensemble.** Un geste n'est accusé à son appareil
//! qu'une fois écrite la case de journal qui le contient (l'accusé durable, SAL-56) ; le
//! registre de cette case dit, pour chaque onglet, le dernier geste ainsi écrit. Une
//! autorité qui reprend depuis la garde reconnaît donc exactement les gestes que la
//! partie reprise contient, et ceux qu'un appareil renvoie au-delà sont neufs (SAL-57).
//!
//! La forme de ce que portent les cases a sa version, [`VERSION_CASES`], et ses octets
//! figés dans `tests/garde.rs`, avec une seconde lecture (SAL-90).

use jeu_abi::{Appareil, ErreurFil, Pas, decoder};
use liaison::{Case, CodeSalle};
use partie::persistance::{
    self, InstantaneGarde, RefusGarde, decoder_journal, decoder_ouverture, decoder_point,
    encoder_lignes, raccorder,
};
use partie::{Journal, Ligne, Ouverture, PointDeReprise};
use serde::{Deserialize, Serialize};

use crate::chiffre::{
    AVANT_LA_PREMIERE_CASE, CleDeGarde, EnteteCase, ErreurChiffre, Hasard, ScellementDeCase,
    empreinte_case, ouvrir_case, sceller_case,
};

/// La version de la forme de ce que portent les cases. Elle monte quand [`Registre`],
/// [`ContenuPoint`] ou [`ContenuJournal`] changent de forme : `tests/garde.rs` la fige.
/// *2 au lot 10a* : le registre porte les marques F8 ([`Marque`]) et les gestes écartés.
pub const VERSION_CASES: u32 = 2;

/// Les marques F8 qu'une salle garde au plus : les plus anciennes s'oublient au-delà
/// (choisi : l'hôte en pose quelques-unes dans une soirée ; une case porte le registre
/// entier, marques comprises).
pub const MARQUES_MAX: usize = 64;

/// Les onglets d'un appareil dont la salle garde le dernier geste : les huit derniers qui
/// ont livré quelque chose (SAL-37). Un onglet absent plus longtemps renvoie ce qu'il
/// avait en vol ; c'est un geste neuf pour la salle, et c'est dit.
pub const ONGLETS_RETENUS: usize = 8;

/// L'empreinte d'un jeton, sous laquelle la salle range son appareil : jamais le jeton
/// lui-même, qu'une garde lue ne rend pas.
pub fn empreinte_jeton(jeton: &[u8; 32]) -> [u8; 32] {
    blake3::derive_key("soiree empreinte de jeton 1", jeton)
}

/// Le registre de la salle (PHASE-3, § 2.4) : à quel appareil chaque jeton correspond, et
/// le dernier geste écrit de chacun de ses onglets.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registre {
    /// Les appareils inscrits, dans l'ordre de leur inscription.
    pub appareils: Vec<Inscrit>,
    /// Les marques F8 de la soirée, de la plus ancienne à la plus récente, au plus
    /// [`MARQUES_MAX`] (SAL-162) : `soiree-banc rejouer-garde` rejoue la partie jusqu'à
    /// chacune (§ 2.4, lot 10a).
    pub marques: Vec<Marque>,
    /// Les gestes écartés à la demande de la page, par leur identité (`IdGeste`), au plus
    /// [`MARQUES_MAX`] : ils ont bloqué deux Workers (P39) ; accusés, ils ne sont pas
    /// écrits, et le rapport les nomme.
    pub ecartes: Vec<u64>,
}

/// Une marque F8 : l'hôte a vu quelque chose d'étrange, et la soirée se rejouera jusque-là.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marque {
    /// L'indice absolu du journal où elle tombe : la fin du dernier pas joué quand l'hôte
    /// l'a posée (un bord de pas, où une reprise peut s'arrêter).
    pub indice: u64,
    /// Le dernier pas définitif d'alors.
    pub pas: Pas,
}

/// Un appareil inscrit au registre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inscrit {
    /// L'empreinte de son jeton ([`empreinte_jeton`]).
    pub jeton: [u8; 32],
    /// L'appareil, tiré pour la soirée.
    pub appareil: Appareil,
    /// Ses onglets qui ont livré quelque chose, du plus ancien au plus récent, au plus
    /// [`ONGLETS_RETENUS`].
    pub onglets: Vec<OngletInscrit>,
}

/// Ce que la salle garde d'un onglet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OngletInscrit {
    /// L'onglet.
    pub onglet: u64,
    /// Son dernier geste écrit dans la garde : ceux d'après sont neufs.
    pub dernier_geste: u64,
}

impl Registre {
    /// L'inscrit de ce jeton.
    pub fn par_jeton(&self, empreinte: &[u8; 32]) -> Option<&Inscrit> {
        self.appareils.iter().find(|i| i.jeton == *empreinte)
    }
}

/// Ce que porte une case de point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContenuPoint {
    /// L'ouverture de la soirée ([`partie::persistance::encoder_ouverture`]).
    pub ouverture: Vec<u8>,
    /// Le point de reprise ([`partie::persistance::encoder_point`]).
    pub point: Vec<u8>,
    /// Le registre au point.
    pub registre: Registre,
}

/// Ce que porte une case de journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContenuJournal {
    /// Les lignes ([`partie::persistance::encoder_lignes`]), à base absolue.
    pub lignes: Vec<u8>,
    /// Le registre après ces lignes.
    pub registre: Registre,
}

/// La sorte d'un contenu, le cinquième octet de son en-tête.
const SORTE_POINT: u8 = 0;
const SORTE_JOURNAL: u8 = 1;

fn en_tete(sorte: u8) -> Vec<u8> {
    let mut o = VERSION_CASES.to_le_bytes().to_vec();
    o.push(sorte);
    o
}

/// Encode un contenu : [`VERSION_CASES`] en `u32` petit-boutiste, la sorte, puis le
/// contenu en postcard.
fn encoder_contenu<T: Serialize>(sorte: u8, v: &T) -> Vec<u8> {
    let mut o = en_tete(sorte);
    let debut = o.len();
    if jeu_abi::encoder(v, &mut o).is_err() {
        // Inatteignable pour ces types ; un contenu vide se refuse à la relecture.
        o.truncate(debut);
    }
    o
}

/// Encode le contenu d'une case de point.
pub fn encoder_contenu_point(c: &ContenuPoint) -> Vec<u8> {
    encoder_contenu(SORTE_POINT, c)
}

/// Encode le contenu d'une case de journal.
pub fn encoder_contenu_journal(c: &ContenuJournal) -> Vec<u8> {
    encoder_contenu(SORTE_JOURNAL, c)
}

/// Pourquoi un contenu ne se relit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusContenu {
    /// Moins de cinq octets.
    Court,
    /// Une autre version de la forme.
    Version {
        /// Celle des octets.
        lue: u32,
    },
    /// Une autre sorte que celle de la case.
    Sorte {
        /// L'octet lu.
        lue: u8,
    },
    /// Le contenu ne se lit pas.
    Illisible(ErreurFil),
}

fn contenu(octets: &[u8], sorte: u8) -> Result<&[u8], RefusContenu> {
    let Some((tete, reste)) = octets.split_first_chunk::<5>() else {
        return Err(RefusContenu::Court);
    };
    let [v0, v1, v2, v3, sorte_lue] = *tete;
    let lue = u32::from_le_bytes([v0, v1, v2, v3]);
    if lue != VERSION_CASES {
        return Err(RefusContenu::Version { lue });
    }
    if sorte_lue != sorte {
        return Err(RefusContenu::Sorte { lue: sorte_lue });
    }
    Ok(reste)
}

/// Relit le contenu d'une case de point, strictement.
pub fn decoder_contenu_point(octets: &[u8]) -> Result<ContenuPoint, RefusContenu> {
    decoder(contenu(octets, SORTE_POINT)?).map_err(RefusContenu::Illisible)
}

/// Relit le contenu d'une case de journal, strictement.
pub fn decoder_contenu_journal(octets: &[u8]) -> Result<ContenuJournal, RefusContenu> {
    decoder(contenu(octets, SORTE_JOURNAL)?).map_err(RefusContenu::Illisible)
}

/// Le numéro d'une case.
pub const fn numero(c: Case) -> u64 {
    match c {
        Case::Point(n) | Case::Journal(n) => n,
    }
}

/// Où la chaîne en est : le numéro de la prochaine case, l'empreinte de la dernière, et
/// les points écrits, du plus ancien au plus récent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suite {
    /// Le numéro de la prochaine case.
    pub prochain: u64,
    /// L'empreinte de la dernière case ([`empreinte_case`]), ou
    /// [`AVANT_LA_PREMIERE_CASE`].
    pub precedente: [u8; 32],
    /// Les numéros des points de la chaîne, du plus ancien au plus récent.
    pub points: Vec<u64>,
}

impl Suite {
    /// Une chaîne neuve : aucune case.
    pub const fn neuve() -> Suite {
        Suite {
            prochain: 0,
            precedente: AVANT_LA_PREMIERE_CASE,
            points: Vec::new(),
        }
    }
}

/// Une case de point en train de se sceller ([`Ecrivain::commencer_point`]).
#[derive(Debug)]
pub struct PointEnScellement {
    case: Case,
    precedente: [u8; 32],
    scellement: ScellementDeCase,
}

impl PointEnScellement {
    /// La case qu'il prendra.
    pub const fn case(&self) -> Case {
        self.case
    }

    /// Chiffre la tranche suivante, d'au plus `octets_max` octets. Rend vrai quand tout
    /// est chiffré (il reste à le finir).
    pub fn avancer(&mut self, octets_max: usize) -> bool {
        self.scellement.avancer(octets_max)
    }
}

/// Ce qui écrit la chaîne : il scelle chaque case à la suite de la précédente, et suit
/// les accusés du relais (`DuRelais::Ecrit`).
#[derive(Debug, Clone)]
pub struct Ecrivain {
    salle: CodeSalle,
    suite: Suite,
    /// Le plus haut numéro que le relais dit écrit, s'il y en a un.
    ecrit: Option<u64>,
    /// Le numéro de la dernière case de point envoyée, et s'il faut effacer avant le
    /// point d'avant quand elle sera écrite.
    point_en_vol: Option<u64>,
}

impl Ecrivain {
    /// L'écrivain d'une chaîne, à sa suite.
    pub const fn new(salle: CodeSalle, suite: Suite) -> Ecrivain {
        Ecrivain {
            salle,
            suite,
            ecrit: None,
            point_en_vol: None,
        }
    }

    /// Où la chaîne en est.
    pub const fn suite(&self) -> &Suite {
        &self.suite
    }

    /// Le plus haut numéro que le relais dit écrit : ce que la coque garde à côté de G,
    /// pour refuser ensuite une garde plus vieille (P37).
    pub const fn dernier_ecrit(&self) -> Option<u64> {
        self.ecrit
    }

    fn sceller(
        &mut self,
        g: &CleDeGarde,
        case: Case,
        clair: &[u8],
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<Vec<u8>, ErreurChiffre> {
        let entete = EnteteCase {
            salle: self.salle,
            case,
            precedente: self.suite.precedente,
        };
        let scellee = sceller_case(g, &entete, clair, h)?;
        self.suite.precedente = empreinte_case(&scellee);
        self.suite.prochain = self.suite.prochain.saturating_add(1);
        Ok(scellee)
    }

    /// Scelle une case de point : rend la case et ses octets, à envoyer par
    /// `VersRelais::Ecrire`. D'un coup : [`Ecrivain::commencer_point`], puis
    /// [`Ecrivain::finir_point`].
    pub fn point<S: InstantaneGarde + PartialEq>(
        &mut self,
        g: &CleDeGarde,
        ouverture: &Ouverture,
        point: &PointDeReprise<S>,
        registre: &Registre,
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<(Case, Vec<u8>), ErreurChiffre> {
        let p = self.commencer_point(g, ouverture, point, registre, h)?;
        self.finir_point(p)
    }

    /// Commence à sceller une case de point, qui prendra le numéro suivant de la chaîne
    /// (lot 10a, correction après la vérification) : le clair est encodé et le nonce
    /// tiré ; le chiffrement se fait par tranches ([`PointEnScellement::avancer`]), sur
    /// plusieurs réveils du Worker. **Aucune autre case ne se scelle avant
    /// [`Ecrivain::finir_point`]** : elle suit celle-ci dans la chaîne.
    pub fn commencer_point<S: InstantaneGarde + PartialEq>(
        &self,
        g: &CleDeGarde,
        ouverture: &Ouverture,
        point: &PointDeReprise<S>,
        registre: &Registre,
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<PointEnScellement, ErreurChiffre> {
        let case = Case::Point(self.suite.prochain);
        let clair = encoder_contenu_point(&ContenuPoint {
            ouverture: persistance::encoder_ouverture(ouverture),
            point: persistance::encoder_point(point),
            registre: registre.clone(),
        });
        let entete = EnteteCase {
            salle: self.salle,
            case,
            precedente: self.suite.precedente,
        };
        Ok(PointEnScellement {
            case,
            precedente: self.suite.precedente,
            scellement: ScellementDeCase::commencer(g, &entete, &clair, h)?,
        })
    }

    /// Finit le scellement d'un point commencé, et le met dans la chaîne : rend la case
    /// et ses octets. Refusé (`Interne`) si une autre case s'est scellée entre-temps.
    pub fn finir_point(&mut self, p: PointEnScellement) -> Result<(Case, Vec<u8>), ErreurChiffre> {
        if numero(p.case) != self.suite.prochain || p.precedente != self.suite.precedente {
            return Err(ErreurChiffre::Interne);
        }
        let (scellee, empreinte) = p.scellement.finir();
        self.suite.precedente = empreinte;
        self.suite.prochain = self.suite.prochain.saturating_add(1);
        self.suite.points.push(numero(p.case));
        self.point_en_vol = Some(numero(p.case));
        Ok((p.case, scellee))
    }

    /// Scelle une case de journal : les lignes d'indice absolu `base` et suivants, avec
    /// le dernier pas définitif `pas`.
    pub fn journal(
        &mut self,
        g: &CleDeGarde,
        (base, pas, lignes): (usize, Pas, &[Ligne]),
        registre: &Registre,
        h: &mut (impl Hasard + ?Sized),
    ) -> Result<(Case, Vec<u8>), ErreurChiffre> {
        let case = Case::Journal(self.suite.prochain);
        let clair = encoder_contenu_journal(&ContenuJournal {
            lignes: encoder_lignes(base, pas, lignes),
            registre: registre.clone(),
        });
        let scellee = self.sceller(g, case, &clair, h)?;
        Ok((case, scellee))
    }

    /// Le relais dit cette case écrite. Rend, si c'est la case de point la plus récente,
    /// le numéro avant lequel effacer : celui du point d'avant (`VersRelais::Effacer`).
    /// Un accusé d'une case qui n'a pas été envoyée ne compte pas.
    pub fn ecrit(&mut self, case: Case) -> Option<u64> {
        let n = numero(case);
        if n >= self.suite.prochain {
            return None;
        }
        self.ecrit = Some(self.ecrit.map_or(n, |e| e.max(n)));
        if self.point_en_vol == Some(n) && matches!(case, Case::Point(_)) {
            self.point_en_vol = None;
            let position = self.suite.points.iter().position(|&p| p == n)?;
            let avant = *self.suite.points.get(position.checked_sub(1)?)?;
            // Les points d'avant celui-là s'effacent avec leurs cases.
            self.suite.points.retain(|&p| p >= avant);
            return Some(avant);
        }
        None
    }
}

/// Pourquoi une garde ne se relit pas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusDeGarde {
    /// Aucune case : une salle neuve, ou une garde perdue si l'on en savait une.
    Vide,
    /// La garde s'arrête avant le dernier numéro que l'autorité sait écrit : le relais
    /// a retenu des cases, ou c'est une garde d'avant (PHASE-3, § 2.4).
    PlusVieille {
        /// Le plus haut numéro lu, s'il y en a un.
        lue: Option<u64>,
        /// Le dernier numéro que l'autorité sait écrit.
        connue: u64,
    },
    /// Deux cases du même numéro.
    Doublon(u64),
    /// Il manque une case dans la chaîne.
    Trouee {
        /// Le numéro qui manque.
        manque: u64,
    },
    /// Aucun point dans la garde.
    SansPoint,
    /// Une case qui ne s'ouvre pas sous l'en-tête où on l'attend : abîmée, d'une autre
    /// salle, intervertie, d'une autre sorte que son nom, ou qui ne suit pas la
    /// précédente.
    Scellee(Case),
    /// Le contenu d'une case ne se lit pas.
    Contenu(Case, RefusContenu),
    /// Ce que porte une case ne se relit pas (`partie::persistance`).
    Partie(Case, RefusGarde),
}

/// Une garde relue : de quoi reprendre la partie (`Partie::reprendre`) et la salle.
#[derive(Debug, Clone)]
pub struct GardeLue<S> {
    /// L'ouverture de la soirée.
    pub ouverture: Ouverture,
    /// Le dernier point écrit.
    pub point: PointDeReprise<S>,
    /// Le journal qui le suit, à base de son indice.
    pub journal: Journal,
    /// Le registre de la dernière case.
    pub registre: Registre,
    /// Où la chaîne en est : l'autorité qui reprend écrit à sa suite.
    pub suite: Suite,
}

/// Le point d'où la garde reprend, et un journal vide à son indice.
fn lire_le_point<S: InstantaneGarde + Clone>(
    case: Case,
    clair: &[u8],
) -> Result<GardeLue<S>, RefusDeGarde> {
    let c = decoder_contenu_point(clair).map_err(|r| RefusDeGarde::Contenu(case, r))?;
    let ouverture = decoder_ouverture(&c.ouverture).map_err(|r| RefusDeGarde::Partie(case, r))?;
    let point = decoder_point::<S>(&c.point).map_err(|r| RefusDeGarde::Partie(case, r))?;
    let vide = encoder_lignes(point.indice(), point.pas(), &[]);
    let journal = decoder_journal(&vide).map_err(|r| RefusDeGarde::Partie(case, r))?;
    Ok(GardeLue {
        ouverture,
        point,
        journal,
        registre: c.registre,
        suite: Suite::neuve(),
    })
}

/// Relit une garde : ses cases, dans n'importe quel ordre, telles que le relais les a
/// rendues (`DuRelais::Lu`). `dernier_n` est le dernier numéro que l'autorité sait écrit
/// (gardé à côté de G) ; une garde qui s'arrête avant est refusée.
///
/// La chaîne se vérifie depuis le dernier point jusqu'à la dernière case : chacune doit
/// s'ouvrir sous l'empreinte de la précédente. La case qui précède ce point n'est que
/// hachée (sa propre précédente peut être effacée) ; la changer empêche le point de
/// s'ouvrir.
pub fn relire<S: InstantaneGarde + Clone>(
    g: &CleDeGarde,
    salle: CodeSalle,
    cases: &[(Case, Vec<u8>)],
    dernier_n: Option<u64>,
) -> Result<GardeLue<S>, RefusDeGarde> {
    let mut rangees: Vec<&(Case, Vec<u8>)> = cases.iter().collect();
    rangees.sort_by_key(|(c, _)| numero(*c));
    let plus_haut = rangees.last().map(|(c, _)| numero(*c));
    if let Some(connue) = dernier_n
        && plus_haut.is_none_or(|h| h < connue)
    {
        return Err(RefusDeGarde::PlusVieille {
            lue: plus_haut,
            connue,
        });
    }
    if rangees.is_empty() {
        return Err(RefusDeGarde::Vide);
    }
    for paire in rangees.windows(2) {
        if let [(a, _), (b, _)] = paire {
            let (a, b) = (numero(*a), numero(*b));
            if a == b {
                return Err(RefusDeGarde::Doublon(a));
            }
            if b != a.saturating_add(1) {
                return Err(RefusDeGarde::Trouee {
                    manque: a.saturating_add(1),
                });
            }
        }
    }
    let k = rangees
        .iter()
        .rposition(|(c, _)| matches!(c, Case::Point(_)))
        .ok_or(RefusDeGarde::SansPoint)?;
    let mut precedente = if let Some((_, octets)) = k.checked_sub(1).and_then(|i| rangees.get(i)) {
        empreinte_case(octets)
    } else {
        {
            let n = rangees
                .get(k)
                .map(|(c, _)| numero(*c))
                .ok_or(RefusDeGarde::SansPoint)?;
            if n != 0 {
                return Err(RefusDeGarde::Trouee {
                    manque: n.saturating_sub(1),
                });
            }
            AVANT_LA_PREMIERE_CASE
        }
    };
    let mut lue: Option<GardeLue<S>> = None;
    let mut points = Vec::new();
    for (case, _) in &rangees {
        if matches!(case, Case::Point(_)) {
            points.push(numero(*case));
        }
    }
    for (case, octets) in rangees.iter().skip(k) {
        let case = *case;
        let entete = EnteteCase {
            salle,
            case,
            precedente,
        };
        let clair = ouvrir_case(g, &entete, octets).map_err(|_| RefusDeGarde::Scellee(case))?;
        precedente = empreinte_case(octets);
        match (case, lue.as_mut()) {
            (Case::Point(_), None) => lue = Some(lire_le_point(case, &clair)?),
            (Case::Journal(_), Some(l)) => {
                let c =
                    decoder_contenu_journal(&clair).map_err(|r| RefusDeGarde::Contenu(case, r))?;
                let morceau =
                    decoder_journal(&c.lignes).map_err(|r| RefusDeGarde::Partie(case, r))?;
                raccorder(&mut l.journal, morceau).map_err(|r| RefusDeGarde::Partie(case, r))?;
                l.registre = c.registre;
            }
            // Le dernier point est la première case relue, et aucun autre ne le suit.
            (Case::Point(_), Some(_)) | (Case::Journal(_), None) => {
                return Err(RefusDeGarde::Scellee(case));
            }
        }
    }
    let mut l = lue.ok_or(RefusDeGarde::SansPoint)?;
    l.suite = Suite {
        prochain: plus_haut.map_or(0, |h| h.saturating_add(1)),
        precedente,
        points,
    };
    Ok(l)
}

/// Relit la chaîne **entière** d'une soirée, depuis sa première case (l'historique que le
/// banc relève dans l'enregistrement du relais) : le premier point, puis tout le journal
/// qui le suit, raccordé morceau après morceau, chaque point suivant vérifié à sa place
/// (au bord du journal écrit) sans le relire. De quoi rejouer toute la soirée par
/// `Partie::reprendre`, chaque empreinte notée comprise (`soiree-banc soiree`, lot 10a).
/// Le registre rendu est celui de la dernière case.
pub fn relire_l_historique<S: InstantaneGarde + Clone>(
    g: &CleDeGarde,
    salle: CodeSalle,
    historique: &[(Case, Vec<u8>)],
) -> Result<GardeLue<S>, RefusDeGarde> {
    let mut precedente = AVANT_LA_PREMIERE_CASE;
    let mut lue: Option<GardeLue<S>> = None;
    let mut points = Vec::new();
    for (rang, (case, octets)) in historique.iter().enumerate() {
        let case = *case;
        if numero(case) != u64::try_from(rang).unwrap_or(u64::MAX) {
            return Err(RefusDeGarde::Trouee {
                manque: u64::try_from(rang).unwrap_or(u64::MAX),
            });
        }
        let entete = EnteteCase {
            salle,
            case,
            precedente,
        };
        let clair = ouvrir_case(g, &entete, octets).map_err(|_| RefusDeGarde::Scellee(case))?;
        precedente = empreinte_case(octets);
        match (case, lue.as_mut()) {
            (Case::Point(numero_du_point), None) => {
                points.push(numero_du_point);
                lue = Some(lire_le_point(case, &clair)?);
            }
            (Case::Point(numero_du_point), Some(deja)) => {
                points.push(numero_du_point);
                let c =
                    decoder_contenu_point(&clair).map_err(|r| RefusDeGarde::Contenu(case, r))?;
                let p = decoder_point::<S>(&c.point).map_err(|r| RefusDeGarde::Partie(case, r))?;
                if p.indice() != deja.journal.fin() {
                    return Err(RefusDeGarde::Scellee(case));
                }
                deja.registre = c.registre;
            }
            (Case::Journal(_), Some(deja)) => {
                let c =
                    decoder_contenu_journal(&clair).map_err(|r| RefusDeGarde::Contenu(case, r))?;
                let morceau =
                    decoder_journal(&c.lignes).map_err(|r| RefusDeGarde::Partie(case, r))?;
                raccorder(&mut deja.journal, morceau).map_err(|r| RefusDeGarde::Partie(case, r))?;
                deja.registre = c.registre;
            }
            (Case::Journal(_), None) => return Err(RefusDeGarde::SansPoint),
        }
    }
    let mut l = lue.ok_or(RefusDeGarde::SansPoint)?;
    l.suite = Suite {
        prochain: u64::try_from(historique.len()).unwrap_or(u64::MAX),
        precedente,
        points,
    };
    Ok(l)
}

/// Le journal coupé à l'indice absolu `fin` (une marque, au bord d'un pas), écrit quand le
/// dernier pas définitif était `pas` ; `None` si `fin` n'est pas dans le journal.
pub fn journal_jusqu_a(journal: &Journal, fin: usize, pas: Pas) -> Option<Journal> {
    let base = journal.base();
    let lignes = journal.lignes().get(..fin.checked_sub(base)?)?;
    decoder_journal(&encoder_lignes(base, pas, lignes)).ok()
}
