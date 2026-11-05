//! Le rapport de l'épreuve (§ 7.2, § 8.8) : un résultat par contrôle, lisible par un
//! humain et en JSON.
//!
//! Le rapport distingue réussi, échec du jeu (avec de quoi le rejouer), panne de
//! l'outil, sans objet (avec la raison déclarée) et pas exécuté (OUT-01, OUT-02,
//! HIS-05, HIS-06). Un contrôle sans aucune évaluation n'est jamais réussi (OUT-07).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use partie::{Budgets, Journal};

use crate::budgets::BudgetsE19;
use crate::etapes::{Compilation, Etape, Exigence, IssueEtape, ResultatEtape};
use crate::json::Json;
use crate::plan::{IdControle, MondeJoue, Plan};
use crate::soiree::Point;

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
    /// Un résultat par contrôle, de E1 à E20 : tous figurent, demandés ou non.
    pub controles: Vec<ResultatControle>,
    /// La copie native a-t-elle été comparée ?
    pub natif: bool,
    /// Ce que l'épreuve a joué : soirées, pas, entrées.
    pub joue: String,
    /// L'étape 1, quand l'épreuve est partie des sources (`soiree-epreuve <dossier>`) ;
    /// `None` : le module a été donné tout compilé, et l'étape 1 n'est pas faite.
    pub compilation: Option<Compilation>,
    /// Ce que le monde simulé a vraiment fait, sur toutes les soirées jouées.
    pub monde: MondeJoue,
    /// Les budgets que E19 a jugés, provisoires ([`crate::BUDGETS_E19`]).
    pub budgets: BudgetsE19,
    /// Les octets que le moteur remet à zéro avant chaque appel du module, d'où son
    /// coût fixe ; `None` : le module n'est pas chargé.
    pub octets_remis_a_zero: Option<u64>,
    /// E20 : ce que chaque navigateur a vu, un résultat par navigateur (PHASE-3, § 8.3) ;
    /// vide si E20 n'a pas joué.
    pub e20: Vec<crate::deux_moteurs::NavigateurE20>,
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

    /// Le code des seuls contrôles : 0 si chaque contrôle demandé est réussi ou sans
    /// objet ; 1 si le jeu échoue ; 2 si l'outil a lâché ; 3 si un contrôle demandé n'a
    /// pas tourné. Un contrôle qu'une version de l'épreuve n'aurait pas encore écrit
    /// ([`IdControle::ecrit`]) ne changerait pas le code : le texte le rappellerait sous
    /// le verdict.
    pub fn code_des_controles(&self) -> u8 {
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

    /// Les sept étapes de l'épreuve du CONTRAT, faites ou non (RESTE-PHASE-1, point 13).
    /// L'étape 1 vient de [`Rapport::compilation`] ; les étapes 2, 4 et 7, des contrôles
    /// qui les font ([`Etape::controles`]) ; les étapes 3, 5 et 6 ne sont pas faites
    /// par cette version, chacune avec sa raison.
    pub fn etapes(&self) -> Vec<ResultatEtape> {
        Etape::TOUTES
            .into_iter()
            .map(|etape| match etape {
                Etape::Compiler => match &self.compilation {
                    Some(c) => ResultatEtape {
                        etape,
                        issue: c.issue(),
                        detail: format!("depuis {} : la crate {}, `jeu::module!({})`", c.dossier, c.module, c.jeu),
                    },
                    None => ResultatEtape {
                        etape,
                        issue: IssueEtape::PasFaite(String::from(
                            "le module a été donné tout compilé : ni ses sources, ni ses règles d'écriture, ni sa copie native ne sont vus par l'épreuve",
                        )),
                        detail: String::new(),
                    },
                },
                Etape::Constater | Etape::Chasser | Etape::Mesurer => self.etape_des_controles(etape),
                Etape::Examiner | Etape::Injecter | Etape::Reverifier => ResultatEtape {
                    etape,
                    issue: IssueEtape::PasFaite(String::from("cette version de l'épreuve ne la fait pas")),
                    detail: String::new(),
                },
            })
            .collect()
    }

    /// Une étape que des contrôles font : échouée si l'un d'eux échoue, pas faite si
    /// l'un d'eux n'est pas demandé, n'a pas tourné ou a vu l'outil lâcher, faite sinon.
    fn etape_des_controles(&self, etape: Etape) -> ResultatEtape {
        let ids = etape.controles();
        let noms = |f: &dyn Fn(&ResultatControle) -> bool| -> Vec<&'static str> {
            self.controles
                .iter()
                .filter(|r| ids.contains(&r.id) && f(r))
                .map(|r| r.id.nom())
                .collect()
        };
        let echecs = noms(&|r| self.plan.demande(r.id) && r.issue == IssueControle::EchecDuJeu);
        let manquent = noms(&|r| {
            !self.plan.demande(r.id)
                || matches!(
                    r.issue,
                    IssueControle::PasExecute | IssueControle::PanneDeLOutil
                )
        });
        let tous: Vec<&str> = ids.iter().map(|c| c.nom()).collect();
        let mut detail = format!("par {}", tous.join(", "));
        if etape == Etape::Constater && !self.natif {
            detail.push_str(" ; copie native absente : le module n'est comparé qu'à lui-même");
        }
        // Ce que le CONTRAT range dans l'étape, et que cette version ne fait pas.
        let manque_au_contrat = (etape == Etape::Chasser).then_some(
            "sans la comparaison de chaque partie jouée avec et sans prédiction, que le CONTRAT range dans l'étape 4 et que cette version n'a pas (INTERFACE, § 5.3)",
        );
        let issue = if !echecs.is_empty() {
            let _: Result<(), std::fmt::Error> =
                write!(detail, " ; échoue : {}", echecs.join(", "));
            IssueEtape::Echouee
        } else if !manquent.is_empty() {
            IssueEtape::PasFaite(format!(
                "pas demandé, pas exécuté ou panne de l'outil : {}",
                manquent.join(", ")
            ))
        } else if let Some(m) = manque_au_contrat {
            IssueEtape::FaiteEnPartie(String::from(m))
        } else {
            IssueEtape::Faite
        };
        ResultatEtape {
            etape,
            issue,
            detail,
        }
    }

    /// Le code de sortie de `soiree-epreuve` : 1 si le jeu échoue (un contrôle, ou une
    /// règle de l'étape 1) ; 2 si l'outil a lâché ; 3 si un contrôle demandé n'a pas
    /// tourné ou si une étape exigée n'est pas faite ; 0 sinon. Un module donné tout
    /// compilé ne rend donc jamais 0 : l'étape 1 n'est pas faite.
    pub fn code_de_sortie(&self) -> u8 {
        let controles = self.code_des_controles();
        let etapes = self.etapes();
        if controles == 1 || etapes.iter().any(|e| e.issue == IssueEtape::Echouee) {
            1
        } else if controles == 2 {
            2
        } else if controles == 3 || etapes.iter().any(ResultatEtape::manque) {
            3
        } else {
            0
        }
    }

    /// Le verdict d'un code 0 : ce que cette version de l'épreuve sait faire passe, et
    /// ce qu'elle ne fait pas du CONTRAT est nommé, étape par étape. Le code 0 ne dit
    /// pas que le jeu passe l'épreuve du CONTRAT entière (revue de la phase 2,
    /// constat 15).
    pub fn verdict_du_code_0(&self) -> String {
        let reste: Vec<String> = self
            .etapes()
            .into_iter()
            .filter_map(|e| match e.issue {
                IssueEtape::PasFaite(_) => {
                    Some(format!("{}. {} pas faite", e.etape.numero(), e.etape.nom()))
                }
                IssueEtape::FaiteEnPartie(_) => Some(format!(
                    "{}. {} faite en partie",
                    e.etape.numero(),
                    e.etape.nom()
                )),
                IssueEtape::Faite | IssueEtape::Echouee => None,
            })
            .collect();
        if reste.is_empty() {
            String::from("le jeu passe chaque étape du CONTRAT et chaque contrôle demandé")
        } else {
            format!(
                "le jeu passe chaque contrôle demandé et chaque étape que cette version de l'épreuve exige ; elle ne fait pas toute l'épreuve du CONTRAT : {}",
                reste.join(", ")
            )
        }
    }

    /// Le rapport lisible.
    #[expect(
        clippy::too_many_lines,
        reason = "le rapport se lit d'un trait : le plan, chaque contrôle, chaque étape, le verdict"
    )]
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
        if !self.plan.navigateurs.is_empty() {
            let noms: Vec<&str> = self.plan.navigateurs.iter().map(|n| n.nom()).collect();
            let _: Result<(), std::fmt::Error> = writeln!(s, "E20 dans : {}.", noms.join(", "));
        }
        if let Some(c) = self.plan.commun_fautif {
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "Défaut injecté dans le commun (contrôle négatif) : {c:?}."
            );
        }
        let _: Result<(), std::fmt::Error> = writeln!(s, "Joué : {}", self.joue);
        let monde: Vec<String> = self
            .monde
            .compteurs()
            .iter()
            .map(|(n, v)| format!("{n} {v}"))
            .collect();
        let _: Result<(), std::fmt::Error> =
            writeln!(s, "Le monde simulé a fait : {}.", monde.join(", "));
        let _: Result<(), std::fmt::Error> = writeln!(
            s,
            "Budgets de E19 {}.",
            self.budgets.decrire(self.octets_remis_a_zero)
        );
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
        let _: Result<(), std::fmt::Error> = writeln!(s, "Étapes de l'épreuve du CONTRAT :");
        for e in self.etapes() {
            let exigence = match e.etape.exigence() {
                Exigence::Exigee => String::from("exigée"),
                Exigence::NonExigee(raison) => format!("non exigée : {raison}"),
            };
            let raison = match &e.issue {
                IssueEtape::PasFaite(r) | IssueEtape::FaiteEnPartie(r) => format!(" ({r})"),
                IssueEtape::Faite | IssueEtape::Echouee => String::new(),
            };
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "  {}. {} — {}{raison} [{exigence}]",
                e.etape.numero(),
                e.etape.nom(),
                e.issue.nom()
            );
            if !e.detail.is_empty() {
                let _: Result<(), std::fmt::Error> = writeln!(s, "      {}", e.detail);
            }
            if e.etape == Etape::Compiler
                && let Some(c) = &self.compilation
            {
                for r in &c.regles {
                    let _: Result<(), std::fmt::Error> =
                        writeln!(s, "      {} : {}", r.nom, r.issue.nom());
                    for ligne in r.detail.lines().take(12) {
                        let _: Result<(), std::fmt::Error> = writeln!(s, "        {ligne}");
                    }
                }
            }
        }
        let code = self.code_de_sortie();
        let verdict = match code {
            0 => self.verdict_du_code_0(),
            1 => String::from("le jeu échoue"),
            2 => String::from("l'outil a lâché : rien n'est jugé"),
            _ => String::from(
                "une étape exigée n'est pas faite, ou un contrôle écrit et demandé n'a pas tourné",
            ),
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
        if self.compilation.is_none() {
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "Module seul : l'épreuve n'est pas partie des sources (étape 1 pas faite) ; `soiree-epreuve <dossier du jeu>` la fait."
            );
        }
        if !self.natif {
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "Copie native absente : E9 et E14 n'ont comparé que le module à lui-même."
            );
        }
        s
    }

    /// Le rapport en JSON.
    #[expect(
        clippy::too_many_lines,
        reason = "le rapport JSON s'écrit d'un trait, dans l'ordre du rapport lisible"
    )]
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
                    (
                        "navigateurs",
                        Json::Liste(
                            self.plan
                                .navigateurs
                                .iter()
                                .map(|n| Json::texte(n.nom()))
                                .collect(),
                        ),
                    ),
                    (
                        "commun_fautif",
                        self.plan
                            .commun_fautif
                            .map_or(Json::Booleen(false), |c| Json::texte(&format!("{c:?}"))),
                    ),
                ]),
            ),
            ("natif", Json::Booleen(self.natif)),
            (
                "compilation",
                self.compilation
                    .as_ref()
                    .map_or(Json::Booleen(false), Compilation::json),
            ),
            (
                "etapes",
                Json::Liste(
                    self.etapes()
                        .iter()
                        .map(|e| {
                            let mut o = vec![
                                ("etape", Json::Nombre(u64::from(e.etape.numero()))),
                                ("nom", Json::texte(e.etape.nom())),
                                ("quoi", Json::texte(e.etape.quoi())),
                                (
                                    "exigee",
                                    Json::Booleen(e.etape.exigence() == Exigence::Exigee),
                                ),
                                ("issue", Json::texte(e.issue.code())),
                                ("detail", Json::texte(&e.detail)),
                            ];
                            if let IssueEtape::PasFaite(r) | IssueEtape::FaiteEnPartie(r) = &e.issue
                            {
                                o.push(("raison", Json::texte(r)));
                            }
                            if let Exigence::NonExigee(r) = e.etape.exigence() {
                                o.push(("non_exigee", Json::texte(r)));
                            }
                            Json::Objet(o)
                        })
                        .collect(),
                ),
            ),
            ("joue", Json::texte(&self.joue)),
            (
                "monde",
                Json::Objet(
                    self.monde
                        .compteurs()
                        .iter()
                        .map(|(n, v)| (*n, Json::Nombre(*v)))
                        .collect(),
                ),
            ),
            (
                "budgets",
                Json::Objet(vec![
                    ("provisoires", Json::Booleen(true)),
                    ("ps_par_unite", Json::Nombre(self.budgets.ps_par_unite)),
                    ("fixe_ns", Json::Nombre(self.budgets.fixe_ns)),
                    (
                        "ps_par_kio_remis_a_zero",
                        Json::Nombre(self.budgets.ps_par_kio_remis_a_zero),
                    ),
                    ("marge", Json::Nombre(self.budgets.marge)),
                    (
                        "part_du_jeu_pour_cent",
                        Json::Nombre(self.budgets.part_du_jeu_pour_cent),
                    ),
                    (
                        "part_des_entrees_pour_cent",
                        Json::Nombre(self.budgets.part_des_entrees_pour_cent),
                    ),
                    ("image_max", Json::Nombre(self.budgets.image_max)),
                    ("image_courante", Json::Nombre(self.budgets.image_courante)),
                    ("lecture", Json::Nombre(self.budgets.lecture)),
                    ("pire_cas", Json::Nombre(self.budgets.pire_cas)),
                    (
                        "octets_remis_a_zero",
                        self.octets_remis_a_zero
                            .map_or(Json::Booleen(false), Json::Nombre),
                    ),
                    (
                        "cout_fixe",
                        self.octets_remis_a_zero.map_or(Json::Booleen(false), |o| {
                            Json::Nombre(self.budgets.cout_fixe(o))
                        }),
                    ),
                ]),
            ),
            ("controles", Json::Liste(controles)),
            (
                "e20",
                Json::Liste(
                    self.e20
                        .iter()
                        .map(|n| {
                            Json::Objet(vec![
                                ("navigateur", Json::texte(&n.nom)),
                                ("version", Json::texte(&n.version)),
                                ("issue", Json::texte(n.issue.code())),
                                ("detail", Json::texte(&n.detail)),
                                ("duree_ms", Json::Nombre(n.duree_ms)),
                                ("rejeu_ms", Json::Nombre(n.rejeu_ms)),
                                ("total_ms", Json::Nombre(n.total_ms)),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "code_de_sortie",
                Json::Nombre(u64::from(self.code_de_sortie())),
            ),
        ])
        .ecrire()
    }
}
