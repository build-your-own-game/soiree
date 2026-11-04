//! La colle du module (§ 2.14 et § 8.3) : décoder un [`jeu_abi::Appel`], appeler le jeu,
//! encoder un [`jeu_abi::Retour`]. Le même code sûr sert au module et à la copie native
//! (`jeu-natif`) : seul change le [`Cadre`], qui bascule l'allocateur et nomme les zones.
//!
//! Cachée de la documentation : un jeu n'a rien à y faire.
//!
//! # Les deux modes d'allocation (§ 3.2, D4, D34)
//!
//! - **Une lecture** est en mode brouillon de bout en bout : une vue qui alloue ne touche
//!   pas le tas, donc pas l'état.
//! - **Une écriture** est en mode tas pendant le décodage de ce qui peut entrer dans l'état
//!   et pendant l'appel du jeu ; la colle passe en mode brouillon ([`Sortie::brouillon`])
//!   avant de construire et d'encoder le retour.
//!
//! # La forme canonique
//!
//! `forme` (8 octets, petit-boutiste) ‖ corps ‖ `non_finis` (8 octets, petit-boutiste).
//! `forme` vaut [`forme_canonique`] du type de l'état ; `non_finis` compte les flottants
//! non finis du corps (E17, CAT-05, SQ-68), lisible sans connaître le jeu. Le corps est
//! celui de l'enveloppe : le pas, le compteur d'identifiants, le hasard (un secret que
//! personne ne voit : jamais projeté, D21), puis l'état du jeu.
//!
//! # La zone de panique
//!
//! [`CAPACITE_PANIQUE`] octets : un code (`u32` petit-boutiste, 0 sans panique,
//! [`CODE_PANIQUE`] après une panique), la longueur du message (`u32` petit-boutiste),
//! puis le message en UTF-8, coupé à la capacité entre deux caractères.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use core::marker::PhantomData;
use core::sync::atomic::{AtomicU8, Ordering};

use jeu_abi::{
    ABI, Appareil, Appel, AttenteFil, Bonjour, Cle, ComposantBotFil, ConstatFil, CoupFil, Duree,
    EcranFil, EntreeFil, ErreurFil, EtatFil, FichierBotFil, HeureMurale, IdDemande, IdGeste,
    Illisible, Instant, IssueFil, MontreFil, Oui, Pas, Presence, QuandFil, RegardFil, Retour,
    SorteAppel, SorteOctets, StyleFil, TenuFil, Validation, VueFil,
};

use crate::cache::{Ecrivain, Liseur, forme};
use crate::etape::{
    Avant, Changement, DeEcran, Decision, Entree, Geste, Quand, Sorties, Temps, Tenu,
};
use crate::forme::{Etat, Forme};
use crate::hasard::Hasard;
use crate::scene::Scene;
use crate::secret::{Autorite, Regard, Secret};
use crate::temps::Maintenant;
use crate::traits::{
    Attente, Bot, Construction, Coup, Creation, Demandes, Epreuve, Jeu, Phase, Style, Variantes,
    Verif,
};

/// La capacité de la boîte d'entrée statique du module : une lecture plus grande est
/// refusée, une écriture plus grande reçoit un tampon du tas (§ 3.3).
pub const CAPACITE_BOITE: usize = 64 * 1024;
/// La capacité de la zone de panique (§ 3.2).
pub const CAPACITE_PANIQUE: usize = 4 * 1024;
/// Le code de la zone de panique après une panique : celui de `jeu-abi`, que le
/// moteur lit.
pub const CODE_PANIQUE: u32 = jeu_abi::PANIQUE_CODE;
/// La taille d'un morceau de `Projeter` et de `Canonique`.
pub const TAILLE_MORCEAU: usize = 64 * 1024;

/// Le mode de l'allocateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `dlmalloc` : ce qui est alloué peut entrer dans l'état.
    Tas,
    /// L'arène de brouillon, remise à zéro au début de chaque appel.
    Brouillon,
}

/// Ce que la colle demande à son hôte : basculer l'allocateur, mesurer le tas.
pub trait Modes {
    /// Passe l'allocateur dans ce mode.
    fn bascule(&self, m: Mode);
    /// Les octets vivants du tas, et sa taille.
    fn memoire(&self) -> (u64, u64);
}

/// L'hôte de la copie native : un seul allocateur, rien à basculer, rien à mesurer.
#[derive(Debug, Clone, Copy, Default)]
pub struct SansModes;

impl Modes for SansModes {
    fn bascule(&self, _: Mode) {}
    fn memoire(&self) -> (u64, u64) {
        (0, 0)
    }
}

/// Les zones du module, que `Bonjour` annonce : adresse et capacité.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Zones {
    /// La boîte d'entrée.
    pub boite: (u32, u32),
    /// Le brouillon des lectures.
    pub brouillon: (u32, u32),
    /// La zone de panique.
    pub panique: (u32, u32),
}

/// L'hôte de la colle.
#[derive(Clone, Copy)]
pub struct Cadre<'c> {
    /// L'allocateur.
    pub modes: &'c dyn Modes,
    /// Les zones.
    pub zones: Zones,
}

impl fmt::Debug for Cadre<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cadre")
            .field("zones", &self.zones)
            .finish_non_exhaustive()
    }
}

/// Ce que l'hôte annonce d'un appel, et où la colle écrit le retour.
#[derive(Debug)]
pub struct Sortie<'c> {
    /// Le retour encodé.
    pub octets: Vec<u8>,
    cadre: Cadre<'c>,
    sorte: SorteAppel,
}

impl<'c> Sortie<'c> {
    /// Une sortie vide, pour un appel de cette sorte annoncée.
    pub fn new(cadre: Cadre<'c>, sorte: SorteAppel) -> Sortie<'c> {
        Sortie {
            octets: Vec::new(),
            cadre,
            sorte,
        }
    }

    /// Passe en mode brouillon : tout ce qui suit est de la sortie.
    pub fn brouillon(&self) {
        self.cadre.modes.bascule(Mode::Brouillon);
    }
}

/// La partie entière telle que la tient le module : l'état du jeu, son hasard secret,
/// le compteur d'identifiants, le pas courant.
#[derive(Debug, Clone)]
pub struct Enveloppe<J: Jeu> {
    jeu: J,
    sorties: Sorties,
    pas: Pas,
}

impl<J: Jeu> Enveloppe<J> {
    /// L'état du jeu, pour les tests et la copie native.
    pub fn jeu(&self) -> &J {
        &self.jeu
    }
}

/// L'empreinte de la forme canonique d'un état de type `S` : elle nomme ce qu'un
/// `DepuisCanonique` sait relire, pour ce jeu et pour sa version précédente.
pub const fn forme_canonique<S: Forme>() -> u64 {
    forme::entier(forme::nommee("enveloppe, version 1"), S::EMPREINTE)
}

/// L'empreinte des formes que `Bonjour` annonce : enveloppe, état, et les types associés
/// du jeu (§ 3.8).
pub const fn formes<J: Jeu + Epreuve>() -> u64 {
    let h = forme::entier(forme::nommee("formes"), ABI as u64);
    let h = forme::entier(h, forme_canonique::<J>());
    let h = forme::entier(h, <J::ActionEcran as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::GesteTelephone as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::TenuTelephone as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::VueTelephone as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::Avis as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::Phase as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::Garde as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::Precedent as Forme>::EMPREINTE);
    let h = forme::entier(h, <J::Resume as Forme>::EMPREINTE);
    forme::entier(h, <J::Cas as Forme>::EMPREINTE)
}

/// La taille du brouillon que déclare le jeu.
pub const fn taille_brouillon<J: Jeu>() -> usize {
    J::DECLARATION.limites.brouillon as usize
}

const fn nombre(s: &str) -> u16 {
    let mut n: u16 = 0;
    let mut reste = s.as_bytes();
    while let [c, suite @ ..] = reste {
        if c.is_ascii_digit() {
            n = n
                .saturating_mul(10)
                .saturating_add(c.saturating_sub(b'0') as u16);
        }
        reste = suite;
    }
    n
}

/// La version de la crate `jeu` compilée dans le module.
pub const VERSION_JEU: (u16, u16, u16) = (
    nombre(env!("CARGO_PKG_VERSION_MAJOR")),
    nombre(env!("CARGO_PKG_VERSION_MINOR")),
    nombre(env!("CARGO_PKG_VERSION_PATCH")),
);

/// L'accord de la déclaration et des types (§ 2.12, D28), évalué dans un `const` :
/// un désaccord est une erreur de compilation.
pub const fn verifier_accord<J: Jeu>() {
    let d = &J::DECLARATION;
    let telephones = d.telephones.oui();
    let geste_vide = <J::GesteTelephone as Forme>::VIDE.oui();
    let tenu_vide = <J::TenuTelephone as Forme>::VIDE.oui();
    let vue_vide = <J::VueTelephone as Forme>::VIDE.oui();
    let tout_vide = geste_vide && tenu_vide && vue_vide;
    assert!(
        telephones || tout_vide,
        "accord : `telephones: SansObjet` demande GesteTelephone, TenuTelephone et VueTelephone = Aucun"
    );
    assert!(
        !telephones || !tout_vide,
        "accord : GesteTelephone, TenuTelephone et VueTelephone valent Aucun, donc `telephones: SansObjet`"
    );
    let secrets = d.secrets.oui();
    let etat_secret = <J as Forme>::SECRET.oui();
    assert!(
        !secrets || etat_secret,
        "accord : `secrets: Oui` demande un état qui contient un Secret"
    );
    assert!(
        secrets || !etat_secret,
        "accord : l'état contient un Secret, donc `secrets: Oui`"
    );
    assert!(
        !d.prediction.oui() || telephones,
        "accord : `prediction: Oui` demande `telephones: Oui`"
    );
    assert!(
        jeu_abi::TexteCourt::essayer(d.nom).is_some(),
        "accord : le nom du jeu fait 64 octets au plus, sans caractère de contrôle"
    );
}

fn erreur(quoi: Illisible) -> ErreurFil {
    ErreurFil { ou: 0, quoi }
}

/// Écrit `r` dans la sortie, en mode brouillon.
fn rendre(sortie: &mut Sortie<'_>, r: &Retour<'_>) -> Result<(), ErreurFil> {
    sortie.brouillon();
    sortie.octets.clear();
    if jeu_abi::encoder(r, &mut sortie.octets).is_ok() {
        Ok(())
    } else {
        refuser(sortie, erreur(Illisible::Forme))
    }
}

/// Écrit `Retour::Illisible`, et rend l'erreur.
fn refuser(sortie: &mut Sortie<'_>, e: ErreurFil) -> Result<(), ErreurFil> {
    sortie.brouillon();
    sortie.octets.clear();
    // `Retour::Illisible` s'encode toujours : un variant et deux nombres.
    let _: Result<(), jeu_abi::ErreurEncodage> =
        jeu_abi::encoder(&Retour::Illisible(e), &mut sortie.octets);
    Err(e)
}

fn encoder_vers<T: serde::Serialize + ?Sized>(v: &T) -> Result<Vec<u8>, ErreurFil> {
    let mut o = Vec::new();
    jeu_abi::encoder(v, &mut o).map_err(|_| erreur(Illisible::Forme))?;
    Ok(o)
}

/// Le corps de l'enveloppe, dans un écrivain.
fn ecrire_corps<S: Etat>(
    e: &mut Ecrivain<'_>,
    pas: Pas,
    compteur: u64,
    hasard: &Secret<Hasard>,
    jeu: &S,
) {
    e.varint(u128::from(pas.numero()));
    e.varint(u128::from(compteur));
    hasard.ecrire(e);
    jeu.ecrire(e);
}

/// La forme canonique, ou sa fenêtre `[debut, fin)`, ajoutée à `dans`. Rend la longueur
/// totale.
fn ecrire_canonique<S: Etat>(
    dans: &mut Vec<u8>,
    (debut, fin): (usize, usize),
    pas: Pas,
    compteur: u64,
    hasard: &Secret<Hasard>,
    jeu: &S,
) -> Result<usize, ErreurFil> {
    let mut e = Ecrivain::fenetre(dans, debut, fin);
    e.octets(&forme_canonique::<S>().to_le_bytes());
    ecrire_corps(&mut e, pas, compteur, hasard, jeu);
    let non_finis = e.non_finis();
    e.octets(&non_finis.to_le_bytes());
    if e.echec().is_some() {
        return Err(erreur(Illisible::Forme));
    }
    Ok(e.position())
}

/// La forme canonique entière d'une enveloppe, secrets compris. Pour la copie native.
pub fn canonique<J: Jeu>(env: &Enveloppe<J>) -> Result<Vec<u8>, ErreurFil> {
    let mut o = Vec::new();
    let _: usize = ecrire_canonique(
        &mut o,
        (0, usize::MAX),
        env.pas,
        env.sorties.compteur,
        &env.sorties.hasard,
        &env.jeu,
    )?;
    Ok(o)
}

/// Relit une forme canonique d'un état de type `S`.
fn lire_canonique<S: Etat>(octets: &[u8]) -> Result<(Pas, u64, Secret<Hasard>, S), ErreurFil> {
    let mut l = Liseur::new(octets);
    let tete: [u8; 8] = l
        .octets(8)?
        .try_into()
        .map_err(|_| erreur(Illisible::Fin))?;
    if u64::from_le_bytes(tete) != forme_canonique::<S>() {
        return Err(erreur(Illisible::Version));
    }
    let pas = Pas::du_fil(u64::try_from(l.varint(64)?).map_err(|_| erreur(Illisible::Varint))?);
    let compteur = u64::try_from(l.varint(64)?).map_err(|_| erreur(Illisible::Varint))?;
    let hasard = <Secret<Hasard> as Etat>::lire(&mut l)?;
    let jeu = S::lire(&mut l)?;
    let _: &[u8] = l.octets(8)?;
    if !l.reste().is_empty() {
        return Err(l.erreur(Illisible::OctetsRestants));
    }
    Ok((pas, compteur, hasard, jeu))
}

/// La garde, ou tout `Etat` hors de l'enveloppe : son corps canonique seul.
fn ecrire_etat<S: Etat>(v: &S) -> Result<Vec<u8>, ErreurFil> {
    let mut o = Vec::new();
    let mut e = Ecrivain::tout(&mut o);
    v.ecrire(&mut e);
    if e.echec().is_some() {
        return Err(erreur(Illisible::Forme));
    }
    Ok(o)
}

fn lire_etat<S: Etat>(octets: &[u8]) -> Result<S, ErreurFil> {
    let mut l = Liseur::new(octets);
    let v = S::lire(&mut l)?;
    if l.reste().is_empty() {
        Ok(v)
    } else {
        Err(l.erreur(Illisible::OctetsRestants))
    }
}

fn hasard_declare<J: Jeu>() -> Oui {
    if J::DECLARATION.hasard.oui() {
        Oui::Oui
    } else {
        Oui::Non
    }
}

fn quand(place: u64, q: QuandFil) -> Quand {
    Quand {
        place: Pas::du_fil(place),
        fait: Instant {
            pas: Pas::du_fil(q.fait),
            fraction: q.fraction,
        },
        recu: Pas::du_fil(q.recu),
        vu: Pas::du_fil(q.vu),
        id: IdGeste::du_fil(q.id),
    }
}

/// Une entrée du fil, avec les types du jeu décodés : en mode tas, depuis les octets
/// d'entrée, puisqu'ils peuvent entrer dans l'état.
fn entree_de<J: Jeu>(e: EntreeFil<'_>, place: u64) -> Result<Entree<'_, J>, ErreurFil> {
    Ok(match e {
        EntreeFil::Temps { saut, heure } => Entree::Temps(Temps {
            saut: Duree::pas(saut),
            heure: HeureMurale::du_fil(heure),
        }),
        EntreeFil::Ecran { quoi, quand: q } => {
            let quand = quand(place, q);
            Entree::Ecran(match quoi {
                EcranFil::Action(b) => DeEcran::Action {
                    action: jeu_abi::decoder(b)?,
                    quand,
                },
                EcranFil::Touche(touche) => DeEcran::Touche { touche, quand },
                EcranFil::Pointeur(pointeur) => DeEcran::Pointeur { pointeur, quand },
                EcranFil::Composant(evenement) => DeEcran::Composant { evenement, quand },
            })
        }
        EntreeFil::Geste {
            de,
            octets,
            quand: q,
        } => Entree::Geste(Geste {
            de: Appareil::du_fil(de),
            geste: jeu_abi::decoder(octets)?,
            quand: quand(place, q),
        }),
        EntreeFil::Tenu {
            de,
            octets,
            cause,
            quand: q,
        } => Entree::Tenu(Changement {
            de: Appareil::du_fil(de),
            valeur: jeu_abi::decoder(octets)?,
            cause,
            quand: quand(place, q),
        }),
        EntreeFil::Manette {
            de,
            etat,
            cause,
            quand: q,
        } => Entree::Manette(Changement {
            de: Appareil::du_fil(de),
            valeur: etat,
            cause,
            quand: quand(place, q),
        }),
        EntreeFil::Presence(p) => Entree::Presence(p),
        EntreeFil::Reponse { id, reponse } => Entree::Reponse(IdDemande::du_fil(id), reponse),
    })
}

/// Le même chemin pour le module et pour la copie native : décode `entree`, exécute
/// l'appel, écrit le retour dans `sortie`. Une erreur rendue est déjà écrite comme
/// `Retour::Illisible`.
pub fn appeler<J: Jeu + Epreuve>(
    e: &mut Option<Enveloppe<J>>,
    entree: &[u8],
    sortie: &mut Sortie<'_>,
) -> Result<(), ErreurFil> {
    let () = const { verifier_accord::<J>() };
    let a = match jeu_abi::decoder::<Appel<'_>>(entree) {
        Ok(a) => a,
        Err(err) => return refuser(sortie, err),
    };
    if a.sorte() != sortie.sorte {
        return refuser(sortie, erreur(Illisible::Ordre));
    }
    match executer(e, a, sortie) {
        Ok(()) => Ok(()),
        Err(err) => refuser(sortie, err),
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "un bras par appel de l'enveloppe, dans l'ordre de jeu_abi::Appel"
)]
fn executer<J: Jeu + Epreuve>(
    e: &mut Option<Enveloppe<J>>,
    a: Appel<'_>,
    sortie: &mut Sortie<'_>,
) -> Result<(), ErreurFil> {
    let autorite = Autorite::new();
    match a {
        Appel::Bonjour => {
            let z = sortie.cadre.zones;
            rendre(
                sortie,
                &Retour::Bonjour(Bonjour {
                    abi: ABI,
                    formes: formes::<J>(),
                    boite: z.boite,
                    brouillon: z.brouillon,
                    panique: z.panique,
                    version_jeu: VERSION_JEU,
                }),
            )
        }
        Appel::Decrire => rendre(sortie, &Retour::Decrit(Box::new(J::DECLARATION.fil()))),
        Appel::Creer {
            graine,
            pas,
            heure,
            salle,
            presents,
            garde,
        } => {
            let presents: Vec<Presence> = jeu_abi::decoder(presents)?;
            let garde = match garde {
                Some(o) => Some(lire_etat::<J::Garde>(o)?),
                None => None,
            };
            let mut sorties = Sorties::new(Hasard::depuis_graine(&graine), hasard_declare::<J>());
            let declare = sorties.hasard_declare == Oui::Oui;
            let jeu = {
                let hasard = if declare {
                    sorties.hasard.lire_mut(&autorite).ok()
                } else {
                    None
                };
                J::creer(Creation {
                    maintenant: Maintenant::new(Pas::du_fil(pas)),
                    heure: HeureMurale::du_fil(heure),
                    autorite: &autorite,
                    hasard,
                    presents: &presents,
                    garde,
                    salle,
                })
            };
            *e = Some(Enveloppe {
                jeu,
                sorties,
                pas: Pas::du_fil(pas),
            });
            sortie.brouillon();
            let reveil = e
                .as_ref()
                .map_or(jeu_abi::ReveilFil::Jamais, |env| env.jeu.reveil().fil());
            rendre(
                sortie,
                &Retour::Issue(IssueFil::Acceptee {
                    sorties: Vec::new(),
                    reveil,
                }),
            )
        }
        Appel::Valider { sorte, octets } => {
            let v = valider::<J>(sorte, octets);
            match v {
                Ok(Some((cle, relache, est))) => rendre(
                    sortie,
                    &Retour::Valide(Validation::Valide {
                        tenu: Some(TenuFil {
                            cle: &cle,
                            relache: relache.as_deref(),
                            est_relache: est,
                        }),
                    }),
                ),
                Ok(None) => rendre(sortie, &Retour::Valide(Validation::Valide { tenu: None })),
                Err(err) => rendre(sortie, &Retour::Valide(Validation::Malforme(err))),
            }
        }
        Appel::Entree { place, entree, .. } => {
            let env = e.as_mut().ok_or(erreur(Illisible::Ordre))?;
            let fil: EntreeFil<'_> = jeu_abi::decoder(entree)?;
            let entree = entree_de::<J>(fil, place)?;
            env.sorties.emises.clear();
            env.sorties.hasard_non_declare = Oui::Non;
            let issue = J::appliquer(
                Avant::new(
                    &mut env.jeu,
                    &mut env.sorties,
                    &autorite,
                    Maintenant::new(Pas::du_fil(place)),
                ),
                entree,
            );
            sortie.brouillon();
            let decision = if env.sorties.hasard_non_declare == Oui::Oui {
                Decision::Attente
            } else {
                issue.decision
            };
            match decision {
                Decision::Acceptee => {
                    // Le pas de l'enveloppe est celui de la dernière entrée acceptée : un
                    // refus ne change rien, pas même lui.
                    env.pas = Pas::du_fil(place);
                    let sorties = env.sorties.emises.clone();
                    let reveil = env.jeu.reveil().fil();
                    rendre(
                        sortie,
                        &Retour::Issue(IssueFil::Acceptee { sorties, reveil }),
                    )
                }
                Decision::Refusee(avis) => {
                    let avis = encoder_vers(&avis)?;
                    rendre(sortie, &Retour::Issue(IssueFil::Refusee { avis: &avis }))
                }
                Decision::Ignoree => rendre(sortie, &Retour::Issue(IssueFil::Ignoree)),
                Decision::Attente => rendre(sortie, &Retour::Issue(IssueFil::Attente)),
            }
        }
        Appel::Regarder { regards } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let mut octets: Vec<Vec<u8>> = Vec::with_capacity(regards.len());
            for r in &regards {
                octets.push(vue(&env.jeu, *r)?);
            }
            let vues: Vec<VueFil<'_>> = regards
                .iter()
                .zip(&octets)
                .map(|(r, o)| VueFil {
                    regard: *r,
                    octets: o,
                })
                .collect();
            rendre(sortie, &Retour::Vues(vues))
        }
        Appel::Etat { connues } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            etat(&env.jeu, &connues, &autorite, sortie)
        }
        Appel::Ressources { cles } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let mut trouvees: Vec<(u64, &[u8])> = Vec::new();
            for c in cles {
                let cle = Cle::du_fil(c);
                let octets = J::DECLARATION
                    .assets
                    .iter()
                    .find(|a| a.cle == cle)
                    .map(|a| a.octets)
                    .or_else(|| env.jeu.ressource(cle));
                if let Some(o) = octets {
                    trouvees.push((c, o));
                }
            }
            rendre(sortie, &Retour::Ressources(trouvees))
        }
        Appel::Projeter { regard, morceau } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let r = Regard::du_fil(regard);
            let jeu = env.jeu.projeter(&r);
            let hasard = env.sorties.hasard.projeter(&r);
            morceau_de(sortie, morceau, |dans, fenetre| {
                ecrire_canonique(dans, fenetre, env.pas, env.sorties.compteur, &hasard, &jeu)
            })
        }
        Appel::Charger { projection } => {
            let (pas, compteur, hasard, jeu) = lire_canonique::<J>(projection)?;
            *e = Some(enveloppe_relue(jeu, pas, compteur, hasard));
            rendre(sortie, &Retour::Fait)
        }
        Appel::Canonique { morceau } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            morceau_de(sortie, morceau, |dans, fenetre| {
                ecrire_canonique(
                    dans,
                    fenetre,
                    env.pas,
                    env.sorties.compteur,
                    &env.sorties.hasard,
                    &env.jeu,
                )
            })
        }
        Appel::DepuisCanonique { forme, octets } => {
            if forme == forme_canonique::<J>() {
                let (pas, compteur, hasard, jeu) = lire_canonique::<J>(octets)?;
                *e = Some(enveloppe_relue(jeu, pas, compteur, hasard));
            } else if forme == forme_canonique::<J::Precedent>()
                && !<J::Precedent as Forme>::VIDE.oui()
            {
                let (pas, compteur, hasard, ancien) = lire_canonique::<J::Precedent>(octets)?;
                let jeu = J::convertir(ancien).map_err(|_| erreur(Illisible::Version))?;
                *e = Some(enveloppe_relue(jeu, pas, compteur, hasard));
            } else {
                return Err(erreur(Illisible::Version));
            }
            rendre(sortie, &Retour::Fait)
        }
        Appel::Garder => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let garde = ecrire_etat(&env.jeu.garder())?;
            rendre(sortie, &Retour::Octets(&garde))
        }
        Appel::Resumer => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let resume = encoder_vers(&env.jeu.resume(&autorite))?;
            rendre(sortie, &Retour::Octets(&resume))
        }
        Appel::Verifier => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let mut v = Verif::default();
            env.jeu.invariants(&autorite, &mut v);
            rendre(sortie, &Retour::Verif(constats(&v)))
        }
        Appel::Transition { avant, apres } => {
            let avant: J::Resume = jeu_abi::decoder(avant)?;
            let apres: J::Resume = jeu_abi::decoder(apres)?;
            let mut v = Verif::default();
            J::transition(&avant, &apres, &mut v);
            rendre(sortie, &Retour::Verif(constats(&v)))
        }
        Appel::Bot {
            regard,
            style,
            graine,
            depuis_vue,
        } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            bot(&env.jeu, regard, style, graine, depuis_vue, sortie)
        }
        Appel::PireCas { indice } => {
            let cas = usize::try_from(indice)
                .ok()
                .and_then(|i| J::Cas::TOUTES.get(i))
                .copied()
                .ok_or(erreur(Illisible::Valeur))?;
            let pas = e.as_ref().map_or(Pas::ZERO, |env| env.pas);
            let construction = Construction {
                maintenant: Maintenant::new(pas),
                appareils: J::DECLARATION.places.appareils_max(),
            };
            let jeu = J::pire_cas(cas, &construction);
            match e.as_mut() {
                Some(env) => env.jeu = jeu,
                None => {
                    *e = Some(Enveloppe {
                        jeu,
                        sorties: Sorties::new(
                            Hasard::depuis_graine(&[0; 32]),
                            hasard_declare::<J>(),
                        ),
                        pas,
                    });
                }
            }
            sortie.brouillon();
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let cas = encoder_vers(&env.jeu.cas())?;
            rendre(sortie, &Retour::PireCas { cas: &cas })
        }
        Appel::Memoire => {
            let (vivants, tas) = sortie.cadre.modes.memoire();
            rendre(sortie, &Retour::Memoire { vivants, tas })
        }
        Appel::Proprietes => {
            let p = J::proprietes();
            rendre(
                sortie,
                &Retour::Proprietes(jeu_abi::ProprietesFil {
                    coupure_transparente: p.coupure_transparente.fil(|c| c.moins_de),
                    permutation: p.permutation.fil(|()| ()),
                }),
            )
        }
        Appel::Hypothese { appareil } => {
            let env = e.as_ref().ok_or(erreur(Illisible::Ordre))?;
            let tient = match J::proprietes().coupure_transparente {
                crate::Applicable::Oui(c) => {
                    (c.hypothese)(&env.jeu, &autorite, Appareil::du_fil(appareil))
                }
                crate::Applicable::SansObjet(_) => Oui::Non,
            };
            rendre(sortie, &Retour::Hypothese(tient))
        }
    }
}

fn enveloppe_relue<J: Jeu>(
    jeu: J,
    pas: Pas,
    compteur: u64,
    hasard: Secret<Hasard>,
) -> Enveloppe<J> {
    let mut sorties = Sorties::new(Hasard::depuis_graine(&[0; 32]), hasard_declare::<J>());
    sorties.hasard = hasard;
    sorties.compteur = compteur;
    Enveloppe { jeu, sorties, pas }
}

/// Un morceau de `TAILLE_MORCEAU` octets d'une forme canonique.
fn morceau_de(
    sortie: &mut Sortie<'_>,
    morceau: u32,
    ecrire: impl FnOnce(&mut Vec<u8>, (usize, usize)) -> Result<usize, ErreurFil>,
) -> Result<(), ErreurFil> {
    let debut = usize::try_from(morceau)
        .ok()
        .and_then(|m| m.checked_mul(TAILLE_MORCEAU))
        .ok_or(erreur(Illisible::Limite))?;
    let fin = debut.saturating_add(TAILLE_MORCEAU);
    let mut octets = Vec::new();
    let total = ecrire(&mut octets, (debut, fin))?;
    if debut > 0 && debut >= total {
        return Err(erreur(Illisible::Limite));
    }
    let suivant = if total > fin {
        morceau.checked_add(1)
    } else {
        None
    };
    rendre(
        sortie,
        &Retour::Morceau {
            octets: &octets,
            suivant,
        },
    )
}

fn constats(v: &Verif) -> Vec<(&'static str, ConstatFil)> {
    v.constats
        .iter()
        .map(|(n, c)| {
            (
                *n,
                match c {
                    crate::traits::Constat::Tenu => ConstatFil::Tenu,
                    crate::traits::Constat::Viole => ConstatFil::Viole,
                },
            )
        })
        .collect()
}

/// Ce que le moteur doit savoir d'un tenu : sa clé, sa valeur relâchée, s'il l'est déjà.
type InfosTenu = (Vec<u8>, Option<Vec<u8>>, Oui);

fn valider<J: Jeu>(sorte: SorteOctets, octets: &[u8]) -> Result<Option<InfosTenu>, ErreurFil> {
    match sorte {
        SorteOctets::ActionEcran => jeu_abi::decoder::<J::ActionEcran>(octets).map(|_| None),
        SorteOctets::GesteTelephone => jeu_abi::decoder::<J::GesteTelephone>(octets).map(|_| None),
        SorteOctets::TenuTelephone => {
            let t: J::TenuTelephone = jeu_abi::decoder(octets)?;
            let cle = encoder_vers(&t.cle())?;
            let r = t.relache();
            let est = if r.as_ref() == Some(&t) {
                Oui::Oui
            } else {
                Oui::Non
            };
            let relache = match r {
                Some(r) => Some(encoder_vers(&r)?),
                None => None,
            };
            Ok(Some((cle, relache, est)))
        }
    }
}

/// Un jeu sans téléphone (`telephones: SansObjet`) n'a pas de vue d'appareil :
/// `VueTelephone` y vaut `Aucun`, sans valeur. Un regard d'appareil y est donc
/// illisible, et le jeu n'est jamais appelé (besoin 3 des jeux,
/// `conception/BESOINS-DES-JEUX.md`).
fn regard_admis<J: Jeu>(r: RegardFil) -> Result<(), ErreurFil> {
    match (r, &J::DECLARATION.telephones) {
        (RegardFil::Appareil(_), crate::Applicable::SansObjet(_)) => Err(erreur(Illisible::Valeur)),
        (RegardFil::Appareil(_), crate::Applicable::Oui(_)) | (RegardFil::Ecran, _) => Ok(()),
    }
}

fn vue<J: Jeu>(jeu: &J, r: RegardFil) -> Result<Vec<u8>, ErreurFil> {
    regard_admis::<J>(r)?;
    let regard = Regard::du_fil(r);
    match r {
        RegardFil::Ecran => {
            let mut s = Scene::<J::ActionEcran>::new();
            jeu.vue_ecran(&regard, &mut s);
            encoder_vers(&s.fil())
        }
        RegardFil::Appareil(_) => {
            let m = jeu.vue_telephone(&regard);
            let v = encoder_vers(&m.vue)?;
            encoder_vers(&MontreFil {
                vue: &v,
                effets: m.effets,
                reglages: m.reglages,
            })
        }
    }
}

fn indice<T: PartialEq>(toutes: &[T], v: &T) -> u16 {
    toutes
        .iter()
        .position(|x| x == v)
        .and_then(|i| u16::try_from(i).ok())
        .unwrap_or(u16::MAX)
}

fn etat<J: Jeu + Epreuve>(
    jeu: &J,
    connues: &[u64],
    autorite: &Autorite,
    sortie: &mut Sortie<'_>,
) -> Result<(), ErreurFil> {
    let phase = jeu.phase();
    let phase_octets = encoder_vers(&phase)?;
    let mut attente = Attente::<J> {
        echeance: None,
        libre: None,
        joueurs: Vec::new(),
        ecran: Vec::new(),
        autorite,
        _jeu: PhantomData,
    };
    jeu.attente(&mut attente);
    let mut d = Demandes {
        connues,
        toutes: Vec::new(),
        nouvelles: Vec::new(),
    };
    jeu.demandes(&mut d);
    let fil = EtatFil {
        phase: &phase_octets,
        phase_indice: indice(<J::Phase as Variantes>::TOUTES, &phase),
        terminale: phase.terminale(),
        reveil: jeu.reveil().fil(),
        attente: AttenteFil {
            echeance: attente.echeance.map(Pas::numero),
            libre: attente.libre,
            joueurs: attente
                .joueurs
                .iter()
                .map(|(a, g)| (a.fil(), g.iter().map(Vec::as_slice).collect()))
                .collect(),
            ecran: attente.ecran.iter().map(Vec::as_slice).collect(),
        },
        accueil: jeu.accueil(),
        cas_indice: indice(<J::Cas as Variantes>::TOUTES, &jeu.cas()),
        demandes: d.toutes,
        nouvelles: d.nouvelles,
    };
    rendre(sortie, &Retour::Etat(fil))
}

fn bot<J: Jeu + Epreuve>(
    jeu: &J,
    regard: RegardFil,
    style: StyleFil,
    graine: u64,
    depuis_vue: u64,
    sortie: &mut Sortie<'_>,
) -> Result<(), ErreurFil> {
    regard_admis::<J>(regard)?;
    let r = Regard::du_fil(regard);
    let mut semence = [0u8; 32];
    if let Some(debut) = semence.first_chunk_mut::<8>() {
        *debut = graine.to_le_bytes();
    }
    let mut b = Bot::<J> {
        regard: r,
        style: match style {
            StyleFil::Sage => Style::Sage,
            StyleFil::Hostile => Style::Hostile,
            StyleFil::Lent => Style::Lent,
        },
        hasard: Hasard::depuis_graine(&semence),
        depuis_vue: Duree::pas(depuis_vue),
        coups: Vec::new(),
        _jeu: PhantomData,
    };
    // Le bot ne voit que la projection de son regard : il ne peut pas tricher (D29).
    let vu = jeu.projeter(&r);
    match regard {
        RegardFil::Ecran => J::bot_ecran(&vu, &mut b),
        RegardFil::Appareil(_) => {
            let m = vu.vue_telephone(&r);
            J::bot_telephone(&vu, &m, &mut b);
        }
    }
    let fichiers: Vec<Vec<FichierBotFil<'_>>> = b
        .coups
        .iter()
        .map(|c| match c {
            Coup::Composant(crate::traits::ComposantBot::Fichiers(f)) => f
                .iter()
                .map(|f| FichierBotFil {
                    nom: &f.nom,
                    genre: &f.genre,
                    octets: &f.octets,
                })
                .collect(),
            Coup::Composant(_)
            | Coup::Geste(_)
            | Coup::Tenu(_)
            | Coup::Action(_)
            | Coup::Touche(_)
            | Coup::Rien => Vec::new(),
        })
        .collect();
    let coups: Vec<CoupFil<'_>> = b
        .coups
        .iter()
        .zip(fichiers)
        .map(|(c, f)| match c {
            Coup::Geste(o) => CoupFil::Geste(o),
            Coup::Tenu(o) => CoupFil::Tenu(o),
            Coup::Action(o) => CoupFil::Action(o),
            Coup::Touche(t) => CoupFil::Touche(*t),
            Coup::Composant(crate::traits::ComposantBot::Saisie { champ, texte, fin }) => {
                CoupFil::Composant(ComposantBotFil::Saisie {
                    champ: *champ,
                    texte,
                    fin: *fin,
                })
            }
            Coup::Composant(crate::traits::ComposantBot::Deplace(d)) => {
                CoupFil::Composant(ComposantBotFil::Deplace(*d))
            }
            Coup::Composant(crate::traits::ComposantBot::Fichiers(_)) => {
                CoupFil::Composant(ComposantBotFil::Fichiers(f))
            }
            Coup::Rien => CoupFil::Rien,
        })
        .collect();
    rendre(sortie, &Retour::Bot(coups))
}

// ---------------------------------------------------------------------------
// Les zones et la panique
// ---------------------------------------------------------------------------

/// Une zone statique de taille fixe du module : boîte, panique. Des `AtomicU8`, pour
/// que le moteur y écrive et que la colle y lise sans `static mut`.
pub struct Zone<const N: usize>([AtomicU8; N]);

impl<const N: usize> Zone<N> {
    /// Une zone de zéros, dans `.bss`.
    pub const fn new() -> Zone<N> {
        Zone([const { AtomicU8::new(0) }; N])
    }

    /// Son adresse.
    pub fn adresse(&self) -> *mut u8 {
        self.0.as_ptr().cast::<u8>().cast_mut()
    }

    /// Sa capacité.
    pub const fn capacite(&self) -> usize {
        N
    }

    /// Écrit des octets à partir de `debut`, coupés à la capacité.
    pub fn ecrire(&self, debut: usize, octets: &[u8]) {
        for (case, b) in self.0.iter().skip(debut).zip(octets) {
            case.store(*b, Ordering::Relaxed);
        }
    }

    /// Lit `n` octets à partir de `debut`.
    pub fn lire(&self, debut: usize, n: usize) -> Vec<u8> {
        self.0
            .iter()
            .skip(debut)
            .take(n)
            .map(|c| c.load(Ordering::Relaxed))
            .collect()
    }
}

impl<const N: usize> Default for Zone<N> {
    fn default() -> Self {
        Zone::new()
    }
}

/// Un écrivain de texte dans une zone, sans allouer : la panique peut venir d'une
/// mémoire épuisée.
struct EcritureZone<'z, const N: usize> {
    zone: &'z Zone<N>,
    position: usize,
}

impl<const N: usize> fmt::Write for EcritureZone<'_, N> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let place = N.saturating_sub(self.position);
        // Coupé entre deux caractères : le message reste de l'UTF-8.
        let mut n = s.len().min(place);
        while n > 0 && !s.is_char_boundary(n) {
            n = n.saturating_sub(1);
        }
        let morceau = s.as_bytes().get(..n).unwrap_or(&[]);
        self.zone.ecrire(self.position, morceau);
        self.position = self.position.saturating_add(n);
        Ok(())
    }
}

/// La taille de l'entête de la zone de panique, en `usize`.
const ENTETE: usize = jeu_abi::PANIQUE_ENTETE as usize;

/// Écrit une panique dans la zone, dans la forme de `jeu_abi::lire_panique` : le
/// code, la longueur, puis le message.
pub fn ecrire_panique<const N: usize>(zone: &Zone<N>, message: &dyn fmt::Display) {
    let mut w = EcritureZone {
        zone,
        position: ENTETE,
    };
    let _: fmt::Result = fmt::write(&mut w, format_args!("{message}"));
    let longueur = u32::try_from(w.position.saturating_sub(ENTETE)).unwrap_or(u32::MAX);
    zone.ecrire(0, &jeu_abi::entete_de_panique(longueur));
}

/// Efface le code de la zone de panique, au début d'un appel.
pub fn effacer_panique<const N: usize>(zone: &Zone<N>) {
    zone.ecrire(0, &[0; ENTETE]);
}

/// Relit une zone de panique : le message, s'il y a eu une panique et qu'il est en
/// UTF-8 (`jeu_abi::lire_panique`).
pub fn lire_panique(octets: &[u8]) -> Option<&str> {
    core::str::from_utf8(jeu_abi::lire_panique(octets)?).ok()
}

// ---------------------------------------------------------------------------
// L'arène du brouillon
// ---------------------------------------------------------------------------

/// Une réservation dans une arène qui ne fait que monter : l'adresse et le nouveau haut,
/// ou rien si elle ne tient pas. La seule arithmétique de l'allocateur de brouillon,
/// testée en natif.
pub const fn reserver(
    debut: usize,
    fin: usize,
    haut: usize,
    taille: usize,
    alignement: usize,
) -> Option<(usize, usize)> {
    if alignement == 0 || !alignement.is_power_of_two() {
        return None;
    }
    let Some(depart) = haut.checked_next_multiple_of(alignement) else {
        return None;
    };
    if depart < debut {
        return None;
    }
    let Some(arrivee) = depart.checked_add(taille) else {
        return None;
    };
    if arrivee > fin {
        return None;
    }
    Some((depart, arrivee))
}

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// Une enveloppe autour d'un état construit hors d'une partie, pour les tests.
#[doc(hidden)]
pub fn enveloppe_de_test<J: Jeu>(jeu: J) -> Enveloppe<J> {
    Enveloppe {
        jeu,
        sorties: Sorties::new(Hasard::depuis_graine(&[0; 32]), hasard_declare::<J>()),
        pas: Pas::ZERO,
    }
}
