//! Le rapport de l'épreuve (§ 7.2, § 8.8) : un résultat par contrôle, lisible par un
//! humain et en JSON.
//!
//! Le rapport distingue réussi, échec du jeu (avec de quoi le rejouer), panne de
//! l'outil, sans objet (avec la raison déclarée) et pas exécuté (OUT-01, OUT-02,
//! HIS-05, HIS-06). Un contrôle sans aucune évaluation n'est jamais réussi (OUT-07).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use partie::{Budgets, Journal};

use crate::json::Json;
use crate::plan::{IdControle, Plan};
use crate::soiree::Point;

/// Les étapes de l'épreuve du CONTRAT (« L'épreuve », étapes 1 à 8) que ce binaire
/// n'exécute pas : il part d'un module déjà compilé, et les leçons, les mutants et
/// les preuves viennent ensuite, dans l'outil (INTERFACE § 7.2). Le rapport les
/// nomme à chaque fois : un code 0 n'est jamais l'épreuve entière du contrat.
pub const ETAPES_NON_EXECUTEES: [(u8, &str); 4] = [
    (
        1,
        "compiler le jeu depuis ses sources (règles d'écriture, overflow-checks, copie native)",
    ),
    (3, "examiner les questions de leçons du CAHIER"),
    (5, "injecter des bugs et voir les tests du jeu échouer"),
    (6, "revérifier les preuves"),
];

/// L'issue d'un contrôle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueControle {
    /// Il a tourné, au moins une fois, et rien n'a échoué.
    Reussi,
    /// Le jeu a échoué : le détail dit où, le rejeu dit comment le refaire.
    EchecDuJeu,
    /// L'outil (le commun, l'épreuve) ou la machine a lâché : rien n'est jugé.
    PanneDeLOutil,
    /// Sans objet pour ce jeu, avec la raison qu'il déclare.
    SansObjet(String),
    /// Il n'a pas tourné, ou pas dans le monde que le plan déclarait.
    PasExecute,
}

impl IssueControle {
    /// Son nom, pour le rapport.
    pub const fn nom(&self) -> &'static str {
        match self {
            IssueControle::Reussi => "réussi",
            IssueControle::EchecDuJeu => "échec du jeu",
            IssueControle::PanneDeLOutil => "panne de l'outil",
            IssueControle::SansObjet(_) => "sans objet",
            IssueControle::PasExecute => "pas exécuté",
        }
    }

    /// Son nom en JSON.
    pub const fn code(&self) -> &'static str {
        match self {
            IssueControle::Reussi => "reussi",
            IssueControle::EchecDuJeu => "echec_du_jeu",
            IssueControle::PanneDeLOutil => "panne_de_l_outil",
            IssueControle::SansObjet(_) => "sans_objet",
            IssueControle::PasExecute => "pas_execute",
        }
    }
}

/// De quoi refaire un échec sur la machine du développeur (§ 4.6 ; HIS-06, HIS-07,
/// OUT-141) : la graine de la soirée et son journal, et, pour rejouer les entrées
/// sans le cœur, l'appel `Creer` de chaque partie.
#[derive(Debug, Clone)]
pub struct Rejeu {
    /// La graine de la soirée.
    pub graine: [u8; 32],
    /// Le journal de la soirée, jusqu'à l'échec.
    pub journal: Journal,
    /// Le point d'où rejouer ; `None` : depuis la création.
    pub point: Option<Point>,
    /// L'appel `Creer` encodé de chaque partie de la soirée, par numéro.
    pub creations: BTreeMap<u32, Vec<u8>>,
    /// Le carburant de chaque sorte d'appel.
    pub budgets: Budgets,
}

/// Le résultat d'un contrôle.
#[derive(Debug, Clone)]
pub struct ResultatControle {
    /// Le contrôle.
    pub id: IdControle,
    /// Son issue.
    pub issue: IssueControle,
    /// Combien de fois il a été évalué.
    pub evaluations: u64,
    /// Ce qu'il a vu : ce qui a échoué, ou ce qui a été vérifié.
    pub detail: String,
    /// De quoi rejouer un échec.
    pub rejeu: Option<Rejeu>,
}

/// Le rapport de l'épreuve d'un module.
#[derive(Debug, Clone)]
pub struct Rapport {
    /// L'empreinte du module livré (§ 3.8).
    pub module: [u8; 32],
    /// Le plan joué.
    pub plan: Plan,
    /// Un résultat par contrôle, de E1 à E19 : tous figurent, demandés ou non.
    pub controles: Vec<ResultatControle>,
    /// La copie native a-t-elle été comparée ?
    pub natif: bool,
    /// Ce que l'épreuve a joué : soirées, pas, entrées.
    pub joue: String,
}

pub(crate) fn hex(o: &[u8]) -> String {
    let mut s = String::with_capacity(o.len().saturating_mul(2));
    for b in o {
        let _: Result<(), std::fmt::Error> = write!(s, "{b:02x}");
    }
    s
}

impl Rapport {
    /// Le résultat d'un contrôle.
    pub fn resultat(&self, id: IdControle) -> Option<&ResultatControle> {
        self.controles.iter().find(|r| r.id == id)
    }

    /// Les contrôles demandés qui ne sont ni réussis, ni sans objet.
    pub fn non_reussis(&self) -> Vec<&ResultatControle> {
        self.controles
            .iter()
            .filter(|r| self.plan.demande(r.id))
            .filter(|r| !matches!(r.issue, IssueControle::Reussi | IssueControle::SansObjet(_)))
            .collect()
    }

    /// Les contrôles en échec du jeu.
    pub fn echecs(&self) -> Vec<IdControle> {
        self.controles
            .iter()
            .filter(|r| r.issue == IssueControle::EchecDuJeu)
            .map(|r| r.id)
            .collect()
    }

    /// Le code de sortie de `soiree-epreuve` : 0 si chaque contrôle demandé est réussi
    /// ou sans objet ; 1 si le jeu échoue ; 2 si l'outil a lâché ; 3 si un contrôle
    /// demandé n'a pas tourné. Un contrôle qu'une version de l'épreuve n'aurait pas
    /// encore écrit ([`IdControle::ecrit`]) ne changerait pas le code : le texte le
    /// rappellerait sous le verdict.
    pub fn code_de_sortie(&self) -> u8 {
        let demandes: Vec<&ResultatControle> = self
            .controles
            .iter()
            .filter(|r| self.plan.demande(r.id))
            .collect();
        if demandes
            .iter()
            .any(|r| r.issue == IssueControle::EchecDuJeu)
        {
            1
        } else if demandes
            .iter()
            .any(|r| r.issue == IssueControle::PanneDeLOutil)
        {
            2
        } else if demandes
            .iter()
            .any(|r| r.id.ecrit() && r.issue == IssueControle::PasExecute)
        {
            3
        } else {
            0
        }
    }

    /// Le rapport lisible.
    pub fn texte(&self) -> String {
        let mut s = String::new();
        let _: Result<(), std::fmt::Error> = writeln!(s, "Épreuve du module {}", hex(&self.module));
        let _: Result<(), std::fmt::Error> = writeln!(
            s,
            "Plan : {} soirée(s) de {} s, graine {}, {} panne(s) ; soirée longue de {} s ; copie native {}.",
            self.plan.soirees,
            self.plan.duree.en_ms() / 1000,
            hex(&self.plan.graine),
            self.plan.pannes.len(),
            self.plan.memoire.en_ms() / 1000,
            if self.natif { "comparée" } else { "absente" },
        );
        let _: Result<(), std::fmt::Error> = writeln!(s, "Joué : {}", self.joue);
        for r in &self.controles {
            let raison = match &r.issue {
                IssueControle::SansObjet(raison) => format!(" ({raison})"),
                IssueControle::Reussi
                | IssueControle::EchecDuJeu
                | IssueControle::PanneDeLOutil
                | IssueControle::PasExecute => String::new(),
            };
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "{:>3} {:<22} {}{raison} — {} évaluation(s)",
                r.id.nom(),
                r.id.titre(),
                r.issue.nom(),
                r.evaluations
            );
            for ligne in r.detail.lines() {
                let _: Result<(), std::fmt::Error> = writeln!(s, "      {ligne}");
            }
            if let Some(j) = &r.rejeu {
                let _: Result<(), std::fmt::Error> = writeln!(
                    s,
                    "      rejeu : graine {}, {} lignes de journal",
                    hex(&j.graine),
                    j.journal.lignes().len()
                );
            }
        }
        let code = self.code_de_sortie();
        let verdict = match code {
            0 => "le jeu passe les contrôles écrits qui sont demandés",
            1 => "le jeu échoue",
            2 => "l'outil a lâché : rien n'est jugé",
            _ => "un contrôle écrit et demandé n'a pas tourné",
        };
        let _: Result<(), std::fmt::Error> = writeln!(s, "Verdict : {verdict} (code {code}).");
        let pas_ecrits: Vec<&str> = self
            .controles
            .iter()
            .filter(|r| self.plan.demande(r.id) && !r.id.ecrit())
            .map(|r| r.id.nom())
            .collect();
        if !pas_ecrits.is_empty() {
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "Demandés mais pas encore écrits, donc pas exécutés et jamais comptés réussis : {}.",
                pas_ecrits.join(", ")
            );
        }
        let etapes: Vec<String> = ETAPES_NON_EXECUTEES
            .iter()
            .map(|(n, e)| format!("{n}. {e}"))
            .collect();
        let _: Result<(), std::fmt::Error> = writeln!(
            s,
            "Étapes de l'épreuve du CONTRAT que ce binaire n'exécute pas, jamais comptées réussies : {}.",
            etapes.join(" ; ")
        );
        if !self.natif {
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "Copie native absente : E9 et E14 n'ont comparé que le module à lui-même."
            );
        }
        s
    }

    /// Le rapport en JSON.
    pub fn json(&self) -> String {
        let controles = self
            .controles
            .iter()
            .map(|r| {
                let mut o = vec![
                    ("id", Json::texte(r.id.nom())),
                    ("titre", Json::texte(r.id.titre())),
                    ("demande", Json::Booleen(self.plan.demande(r.id))),
                    ("issue", Json::texte(r.issue.code())),
                    ("evaluations", Json::Nombre(r.evaluations)),
                    ("detail", Json::texte(&r.detail)),
                ];
                if let IssueControle::SansObjet(raison) = &r.issue {
                    o.push(("raison", Json::texte(raison)));
                }
                if let Some(j) = &r.rejeu {
                    o.push((
                        "rejeu",
                        Json::Objet(vec![
                            ("graine", Json::texte(&hex(&j.graine))),
                            (
                                "lignes",
                                Json::Nombre(u64::try_from(j.journal.lignes().len()).unwrap_or(0)),
                            ),
                            (
                                "parties",
                                Json::Nombre(u64::try_from(j.creations.len()).unwrap_or(0)),
                            ),
                        ]),
                    ));
                }
                Json::Objet(o)
            })
            .collect();
        let pannes = self
            .plan
            .pannes
            .iter()
            .map(|p| Json::texte(&p.nom()))
            .collect();
        let demandes = self
            .plan
            .controles
            .iter()
            .map(|c| Json::texte(c.nom()))
            .collect();
        Json::Objet(vec![
            ("module", Json::texte(&hex(&self.module))),
            (
                "plan",
                Json::Objet(vec![
                    ("soirees", Json::Nombre(u64::from(self.plan.soirees))),
                    ("duree_ms", Json::Nombre(self.plan.duree.en_ms())),
                    ("graine", Json::texte(&hex(&self.plan.graine))),
                    ("pannes", Json::Liste(pannes)),
                    ("controles", Json::Liste(demandes)),
                    ("memoire_ms", Json::Nombre(self.plan.memoire.en_ms())),
                ]),
            ),
            ("natif", Json::Booleen(self.natif)),
            (
                "etapes_non_executees",
                Json::Liste(
                    ETAPES_NON_EXECUTEES
                        .iter()
                        .map(|(n, e)| {
                            Json::Objet(vec![
                                ("etape", Json::Nombre(u64::from(*n))),
                                ("quoi", Json::texte(e)),
                            ])
                        })
                        .collect(),
                ),
            ),
            ("joue", Json::texte(&self.joue)),
            ("controles", Json::Liste(controles)),
            (
                "code_de_sortie",
                Json::Nombre(u64::from(self.code_de_sortie())),
            ),
        ])
        .ecrire()
    }
}
