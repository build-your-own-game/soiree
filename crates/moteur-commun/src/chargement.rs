//! Ce que le chargement vérifie, avant toute partie (§ 2.12, § 3.2, § 8.7) : `Bonjour`,
//! les zones, `Decrire`, l'accord de la déclaration.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use jeu_abi::{
    ABI, Appel, ApplicableFil, Bonjour, CodeTouche, DeclarationFil, PANIQUE_ENTETE, PauseFil,
    Retour, SorteAppel, decoder, encoder, raison_suffisante,
};
use partie::{CausePanne, Echec, Faute, PanneOutil};
use reecriture::Refus;

use crate::interne;
use crate::machine::{Bits, Machine};
use crate::protocole::{Carte, OCTETS_PAR_PAGE, Plan, Zones, appeler};

/// Le carburant de `Bonjour` et de `Decrire` au chargement : de quoi décrire un gros
/// jeu, sans laisser une boucle infinie au seul garde-fou mural (PHASE-3, P5 ;
/// RESTE-PHASE-2, point 16). 2²⁴ unités depuis le lot 3c, d'après deux mesures
/// (`MESURES-PHASE-3.md`, « Lot 2 » et « Lot 3c ») :
///
/// - **ce que les modules en brûlent** : `Bonjour` de 3 104 à 3 194 unités, `Decrire`
///   de 7 304 à 9 322, chez `devine`, `ardoise`, Punchline, Sidequest et Tier list.
///   2²⁴ en laisse encore 1 800 fois à `Decrire`. `Creer` a son propre budget
///   (`partie::Budgets::creation`, 2²⁸), que ce budget-ci ne touche pas ;
/// - **ce que coûte une unité sur une boucle vide** (M10, le module de `Bonjour` qui
///   boucle, [`crate::essais::BONJOUR_QUI_BOUCLE`]), le chargement entier compris :
///   dans wasmtime, de 481 à 539 ps sur la machine de l'hôte peu chargée, jusqu'à
///   1 492 ps sous quatorze `yes` (lot 2) ; dans Chrome 156, 415 ps en médiane et au
///   plus 1 886 ; dans **Firefox 158, 1 699 ps en médiane, au plus 2 031, et jusqu'à
///   6 092 ps sous charge** (lot 3c, dans un Worker : le chargement d'un module neuf
///   tourne dans son code de base). À 2²⁶, la valeur du lot 2, Firefox chargé mettait
///   410 ms à épuiser le budget : deux fois et demie sous le garde-fou d'une seconde,
///   et non dix. 2²⁴ unités y prennent au plus 102 ms : dix fois sous lui, dans chaque
///   moteur mesuré. Une boucle dans `Bonjour` est donc la faute `Budget` du jeu, et non
///   une panne de l'outil, même sur une machine chargée.
///
/// Les anciennes valeurs : 2³² (« point de départ, non mesuré »), de 2 à 6,4 s sur une
/// boucle vide dans wasmtime ; 2²⁶ (lot 2), sur wasmtime seul.
pub const BUDGET_DE_CHARGEMENT: u64 = 1 << 24;

/// Pourquoi un module n'est pas chargé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefusModule {
    /// La réécriture le refuse (§ 3.1, § 3.3).
    Reecriture(Refus),
    /// Il parle une autre enveloppe : son numéro, et la plage du moteur.
    Abi {
        /// L'ABI du module.
        module: u32,
        /// La plus ancienne et la plus récente que le moteur parle.
        moteur: (u32, u32),
    },
    /// Sa déclaration ou ses zones ne tiennent pas.
    Declaration(String),
    /// `Bonjour` ou `Decrire` a fauté.
    Faute {
        /// L'appel.
        appel: &'static str,
        /// La faute.
        faute: Faute,
    },
    /// Le moteur ou la machine a échoué.
    Panne(PanneOutil),
}

impl fmt::Display for RefusModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RefusModule::Reecriture(r) => write!(f, "{r}"),
            RefusModule::Abi { module, moteur } => write!(
                f,
                "le module parle l'ABI {module}, le moteur de {} à {}",
                moteur.0, moteur.1
            ),
            RefusModule::Declaration(d) => write!(f, "déclaration refusée : {d}"),
            RefusModule::Faute { appel, faute } => write!(f, "{appel} : {faute}"),
            RefusModule::Panne(p) => write!(f, "{p}"),
        }
    }
}

impl core::error::Error for RefusModule {}

impl From<PanneOutil> for RefusModule {
    fn from(p: PanneOutil) -> RefusModule {
        RefusModule::Panne(p)
    }
}

/// Un refus de la réécriture : celui du module, sauf une réécriture qui produit un
/// module invalide, qui est une panne de l'outil.
impl From<Refus> for RefusModule {
    fn from(r: Refus) -> RefusModule {
        match r {
            Refus::Reecriture { quoi } => {
                RefusModule::Panne(interne(format!("réécriture : {quoi}")))
            }
            autre @ (Refus::Illisible { .. }
            | Refus::Import { .. }
            | Refus::Export { .. }
            | Refus::ExportManquant { .. }
            | Refus::Fonctionnalite { .. }
            | Refus::Instruction { .. }
            | Refus::Depart
            | Refus::Memoire(_)
            | Refus::PileNonEnBas) => RefusModule::Reecriture(autre),
        }
    }
}

/// Les indices des globales mutables d'origine, lus dans les noms des exports du module
/// réécrit (`soiree_globale_<n>`), triés.
pub fn globales_d_origine<'a>(
    exports: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<u32>, PanneOutil> {
    let mut indices = Vec::new();
    for e in exports {
        if let Some(n) = e.strip_prefix(reecriture::PREFIXE_GLOBALE) {
            indices.push(n.parse().map_err(|_| interne("export de globale"))?);
        }
    }
    indices.sort_unstable();
    Ok(indices)
}

/// Ce que le chargement a vérifié d'un module : ce qu'il dit de lui-même, sa
/// déclaration, ce que ses instances partagent, et le carburant de son chargement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Charge {
    /// Ce que le module a dit de lui-même.
    pub bonjour: Bonjour,
    /// Sa déclaration.
    pub declaration: DeclarationFil,
    /// Ce que ses instances partagent.
    pub carte: Carte,
    /// Le carburant du premier `Bonjour`, et celui de `Decrire` (PHASE-3, lot 2).
    pub carburant: (u64, u64),
}

/// Pose `soiree_memoire_max` sur la sonde, appelle `Bonjour` (une première fois sans
/// zones connues, puis avec elles : la seconde vérifie que la sortie est bien dans le
/// brouillon) et `Decrire`, sous [`BUDGET_DE_CHARGEMENT`] ; vérifie l'ABI, les zones et
/// la déclaration. `m` est une instance neuve du module, qui ne sert qu'à cela.
pub fn verifier_chargement<M: Machine>(m: &mut M, plan: Plan) -> Result<Charge, RefusModule> {
    m.poser_globale(
        reecriture::MEMOIRE_MAX,
        Bits::I32(plan.memoire_max_permise().cast_signed()),
    )?;
    let (bonjour, carburant_bonjour) = lire_bonjour(m, &plan, None)?;
    if bonjour.abi != ABI {
        return Err(RefusModule::Abi {
            module: bonjour.abi,
            moteur: (ABI, ABI),
        });
    }
    let pages = m.pages();
    let zones = zones(&bonjour, plan.pile_initiale(), pages)?;
    if lire_bonjour(m, &plan, Some(zones))?.0 != bonjour {
        return Err(RefusModule::Declaration(String::from(
            "Bonjour ne rend pas deux fois la même chose",
        )));
    }
    let (declaration, carburant_decrire) = lire_declaration(m, &plan, zones)?;
    let memoire_max = accord(
        &declaration,
        &bonjour,
        plan.memoire_min(),
        plan.memoire_max_permise(),
    )?;
    Ok(Charge {
        bonjour,
        declaration,
        carte: Carte::new(plan, zones, memoire_max),
        carburant: (carburant_bonjour, carburant_decrire),
    })
}

fn appel_encode(a: &Appel<'_>) -> Result<Vec<u8>, PanneOutil> {
    let mut v = Vec::new();
    encoder(a, &mut v).map_err(|e| interne(format!("{e}")))?;
    Ok(v)
}

fn refus_d_appel(appel: &'static str, e: Echec) -> RefusModule {
    match e {
        Echec::Faute(faute) => RefusModule::Faute { appel, faute },
        Echec::Panne(p) => RefusModule::Panne(p),
    }
}

/// Une lecture du chargement : ses octets et son carburant.
fn lire<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
    nom: &'static str,
    a: &Appel<'_>,
) -> Result<(Vec<u8>, u64), RefusModule> {
    let rendu = appeler(
        m,
        plan,
        zones,
        SorteAppel::Lecture,
        &appel_encode(a)?,
        BUDGET_DE_CHARGEMENT,
    );
    let octets = rendu.issue.map_err(|e| refus_d_appel(nom, e))?;
    Ok((octets, rendu.carburant))
}

fn lire_bonjour<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
) -> Result<(Bonjour, u64), RefusModule> {
    let (octets, carburant) = lire(m, plan, zones, "Bonjour", &Appel::Bonjour)?;
    match decoder::<Retour<'_>>(&octets) {
        Ok(Retour::Bonjour(b)) => Ok((b, carburant)),
        Ok(Retour::Illisible(e)) => Err(RefusModule::Panne(PanneOutil::new(
            CausePanne::AppelIllisible,
            format!("Bonjour : {e}"),
        ))),
        Ok(_) | Err(_) => Err(RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::SortieInvalide,
        }),
    }
}

fn lire_declaration<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Zones,
) -> Result<(DeclarationFil, u64), RefusModule> {
    let (octets, carburant) = lire(m, plan, Some(zones), "Decrire", &Appel::Decrire)?;
    match decoder::<Retour<'_>>(&octets) {
        Ok(Retour::Decrit(d)) => Ok((*d, carburant)),
        Ok(Retour::Illisible(e)) => Err(RefusModule::Panne(PanneOutil::new(
            CausePanne::AppelIllisible,
            format!("Decrire : {e}"),
        ))),
        Ok(_) | Err(_) => Err(RefusModule::Faute {
            appel: "Decrire",
            faute: Faute::SortieInvalide,
        }),
    }
}

/// Les zones de `Bonjour` : dans la mémoire minimale, au-dessus de la pile, sans se
/// chevaucher, la zone de panique assez grande pour son entête (§ 3.2).
fn zones(b: &Bonjour, pile: u32, pages: u64) -> Result<Zones, RefusModule> {
    let octets = pages.saturating_mul(OCTETS_PAR_PAGE);
    let z = Zones {
        boite: b.boite,
        brouillon: b.brouillon,
        panique: b.panique,
    };
    let toutes = [
        ("boîte", z.boite),
        ("brouillon", z.brouillon),
        ("panique", z.panique),
    ];
    for (nom, (a, n)) in toutes {
        let fin = u64::from(a).saturating_add(u64::from(n));
        if n == 0 || a < pile || fin > octets {
            return Err(RefusModule::Declaration(format!(
                "zone {nom} [{a}, {fin}) hors de [{pile}, {octets})"
            )));
        }
    }
    for (i, (na, (a, n))) in toutes.iter().enumerate() {
        for (nb, (c, m)) in toutes.iter().skip(i.saturating_add(1)) {
            let fa = u64::from(*a).saturating_add(u64::from(*n));
            let fc = u64::from(*c).saturating_add(u64::from(*m));
            if u64::from(*a) < fc && u64::from(*c) < fa {
                return Err(RefusModule::Declaration(format!(
                    "les zones {na} et {nb} se chevauchent"
                )));
            }
        }
    }
    if z.panique.1 <= PANIQUE_ENTETE {
        return Err(RefusModule::Declaration(String::from(
            "zone de panique plus petite que son entête",
        )));
    }
    Ok(z)
}

/// Ce que le moteur vérifie de la déclaration au chargement (§ 2.12) : ce que
/// `jeu::module!` ne peut pas vérifier en `const`, ou qu'un module écrit sans `jeu`
/// pourrait mentir. Rend `soiree_memoire_max`.
fn accord(
    d: &DeclarationFil,
    b: &Bonjour,
    pages_min: u32,
    max_permis: u32,
) -> Result<u32, RefusModule> {
    let refus = |quoi: String| Err(RefusModule::Declaration(quoi));
    let raisons = [
        ("fenetre", sans_objet(&d.fenetre)),
        ("telephones", sans_objet(&d.telephones)),
        ("manettes", sans_objet(&d.manettes)),
        ("prediction", sans_objet(&d.prediction)),
        ("hasard", sans_objet(&d.hasard)),
        ("secrets", sans_objet(&d.secrets)),
        ("fin", sans_objet(&d.fin)),
        (
            "pause",
            match &d.pause {
                PauseFil::Aucune(r) => Some(r.as_str()),
                PauseFil::DuJeu | PauseFil::DuCommun => None,
            },
        ),
    ];
    if let ApplicableFil::Oui(t) = &d.telephones {
        for (nom, r) in [
            ("telephones.tenu.fraicheur", sans_objet(&t.tenu.fraicheur)),
            ("telephones.tenu.age_max", sans_objet(&t.tenu.age_max)),
        ] {
            if let Some(r) = r
                && !raison_suffisante(r)
            {
                return refus(format!("{nom} : raison creuse « {r} »"));
            }
        }
    }
    for (nom, r) in raisons {
        if let Some(r) = r
            && !raison_suffisante(r)
        {
            return refus(format!("{nom} : raison creuse « {r} » (HIS-04)"));
        }
    }
    if matches!(d.prediction, ApplicableFil::Oui(()))
        && matches!(d.telephones, ApplicableFil::SansObjet(_))
    {
        return refus(String::from("prediction sans téléphones"));
    }
    if d.limites.brouillon != b.brouillon.1 {
        return refus(format!(
            "limites.brouillon vaut {}, la zone de brouillon {}",
            d.limites.brouillon, b.brouillon.1
        ));
    }
    // PHASE-3, P33 (REPONSES-PROVISOIRES 14 ; SAL-162) : F8 pose la marque au journal,
    // et n'est qu'au commun. Un jeu qui la déclare pour son diagnostic ferait de la même
    // touche deux choses, ou perdrait la marque. `CodeTouche` se relit par son texte :
    // la comparaison du texte est celle de la touche.
    if d.diagnostic.as_str() == CodeTouche::F8.as_str() {
        return refus(String::from(
            "diagnostic : F8 est la touche du commun, qui pose la marque au journal ; le jeu en déclare une autre",
        ));
    }
    if d.limites.memoire < pages_min {
        return refus(format!(
            "limites.memoire ({} pages) sous la mémoire minimale du module ({pages_min})",
            d.limites.memoire
        ));
    }
    Ok(d.limites.memoire.min(max_permis))
}

fn sans_objet<T>(a: &ApplicableFil<T>) -> Option<&str> {
    match a {
        ApplicableFil::Oui(_) => None,
        ApplicableFil::SansObjet(r) => Some(r.as_str()),
    }
}
