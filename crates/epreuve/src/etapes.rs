//! Les étapes de l'épreuve du CONTRAT (« L'épreuve », 1 à 7), faites ou non
//! (RESTE-PHASE-1, points 12 et 13).
//!
//! Le rapport les nomme toutes, chaque fois : une étape qui n'a pas tourné n'est
//! jamais comptée faite (OUT-01). Chacune dit si cette version de l'épreuve l'exige ;
//! le code de sortie n'est 0 que si toutes les étapes exigées sont faites.
//!
//! | Étape | Ce qui la fait | Exigée |
//! |---|---|---|
//! | 1. Compiler | `soiree-epreuve <dossier>` : le module et la copie native compilés par l'épreuve, et les règles d'écriture vérifiées (`crate::sources`) | oui |
//! | 2. Constater | E1 et E9 | oui |
//! | 3. Examiner | rien encore | non, avec sa raison |
//! | 4. Chasser | E2 à E8, E10 à E14, E16 à E18 | oui |
//! | 5. Injecter des bugs | rien dans l'épreuve | non, avec sa raison |
//! | 6. Revérifier les preuves | rien encore | non, avec sa raison |
//! | 7. Mesurer | E15 et E19 | oui |
//!
//! Un module donné tout compilé (`--wasm`) laisse l'étape 1 « pas faite » : le code de
//! sortie ne vaut alors jamais 0.

use std::fmt::Write as _;

use crate::json::Json;
use crate::plan::IdControle;

/// Une étape de l'épreuve du CONTRAT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Etape {
    /// 1. Compiler le jeu depuis ses sources.
    Compiler,
    /// 2. Constater ce à quoi le module a accès, et le déterminisme.
    Constater,
    /// 3. Examiner les questions des leçons.
    Examiner,
    /// 4. Chasser : pannes, réseau, entrées absurdes, bots, lois et invariants.
    Chasser,
    /// 5. Injecter des bugs dans le code du jeu.
    Injecter,
    /// 6. Revérifier les preuves que le jeu apporte.
    Reverifier,
    /// 7. Mesurer les budgets.
    Mesurer,
}

/// Ce que cette version de l'épreuve exige d'une étape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exigence {
    /// Le code de sortie n'est 0 que si elle est faite.
    Exigee,
    /// Elle ne décide pas du code de sortie, pour cette raison ; le rapport la nomme
    /// quand même, « pas faite ».
    NonExigee(&'static str),
}

impl Etape {
    /// Les sept étapes, dans l'ordre du CONTRAT.
    pub const TOUTES: [Etape; 7] = [
        Etape::Compiler,
        Etape::Constater,
        Etape::Examiner,
        Etape::Chasser,
        Etape::Injecter,
        Etape::Reverifier,
        Etape::Mesurer,
    ];

    /// Son numéro dans le CONTRAT.
    pub const fn numero(self) -> u8 {
        match self {
            Etape::Compiler => 1,
            Etape::Constater => 2,
            Etape::Examiner => 3,
            Etape::Chasser => 4,
            Etape::Injecter => 5,
            Etape::Reverifier => 6,
            Etape::Mesurer => 7,
        }
    }

    /// Son nom, celui du CONTRAT.
    pub const fn nom(self) -> &'static str {
        match self {
            Etape::Compiler => "Compiler",
            Etape::Constater => "Constater",
            Etape::Examiner => "Examiner",
            Etape::Chasser => "Chasser",
            Etape::Injecter => "Injecter des bugs",
            Etape::Reverifier => "Revérifier les preuves",
            Etape::Mesurer => "Mesurer",
        }
    }

    /// Ce qu'elle demande, en une ligne.
    pub const fn quoi(self) -> &'static str {
        match self {
            Etape::Compiler => {
                "le module et la copie native compilés depuis les sources, et les règles d'écriture vérifiées"
            }
            Etape::Constater => {
                "ce à quoi le module a accès (E1), deux exécutions comparées et la copie native avec le module (E9)"
            }
            Etape::Examiner => {
                "chaque question des leçons liée aux mécaniques déclarées a sa réponse"
            }
            Etape::Chasser => {
                "pannes, réseau, entrées absurdes, ordres inattendus, bots jusqu'à toutes les phases, lois et invariants à chaque pas"
            }
            Etape::Injecter => "des bugs injectés dans le code du jeu, que ses tests doivent voir",
            Etape::Reverifier => "les preuves que le jeu apporte, revérifiées",
            Etape::Mesurer => "les budgets (E19) et la mémoire bornée (E15)",
        }
    }

    /// Ce que cette version de l'épreuve en exige.
    pub const fn exigence(self) -> Exigence {
        match self {
            Etape::Compiler | Etape::Constater | Etape::Chasser | Etape::Mesurer => {
                Exigence::Exigee
            }
            Etape::Examiner => Exigence::NonExigee(
                "la base de leçons n'a pas encore de forme que la machine lise (une mécanique, ses questions), ni le document du jeu une forme de réponse : c'est au mainteneur de les fixer (CONTRAT, « Le document de chaque jeu »). Le CONTRAT la rend pourtant bloquante (« Une question sans réponse bloque l'épreuve ») : ne pas l'exiger est un écart à lui, que le verdict dit",
            ),
            Etape::Injecter => Exigence::NonExigee(
                "le CONTRAT en fait un rapport sans seuil, qu'un humain juge : il ne décide pas du code de sortie ; cette version de l'épreuve ne lance pas les mutations",
            ),
            Etape::Reverifier => Exigence::NonExigee(
                "aucun énoncé de cette version du commun n'est à prouver par un jeu : la première preuve porte sur le cœur de la partie (REPONSES-PROVISOIRES, 18)",
            ),
        }
    }

    /// Les contrôles qui la font, quand ce sont des contrôles.
    pub const fn controles(self) -> &'static [IdControle] {
        match self {
            Etape::Constater => &[IdControle::E1, IdControle::E9],
            Etape::Chasser => &[
                IdControle::E2,
                IdControle::E3,
                IdControle::E4,
                IdControle::E5,
                IdControle::E6,
                IdControle::E7,
                IdControle::E8,
                IdControle::E10,
                IdControle::E11,
                IdControle::E12,
                IdControle::E13,
                IdControle::E14,
                IdControle::E16,
                IdControle::E17,
                IdControle::E18,
            ],
            Etape::Mesurer => &[IdControle::E15, IdControle::E19],
            Etape::Compiler | Etape::Examiner | Etape::Injecter | Etape::Reverifier => &[],
        }
    }
}

/// L'issue d'une étape, ou d'une règle de l'étape 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueEtape {
    /// Faite, et rien n'a échoué.
    Faite,
    /// Faite, et le jeu échoue.
    Echouee,
    /// Pas faite, pour cette raison.
    PasFaite(String),
    /// Faite par chaque contrôle que cette version de l'épreuve sait faire, et rien
    /// n'a échoué ; mais le CONTRAT range dans l'étape ce qui manque ici, et ne la
    /// tient donc pas pour faite (« un contrôle qui n'a pas tourné ne compte pas
    /// comme réussi » ; revue de la phase 2, constat 15).
    FaiteEnPartie(String),
}

impl IssueEtape {
    /// Son nom, pour le rapport.
    pub fn nom(&self) -> &'static str {
        match self {
            IssueEtape::Faite => "faite",
            IssueEtape::Echouee => "échouée",
            IssueEtape::PasFaite(_) => "pas faite",
            IssueEtape::FaiteEnPartie(_) => "faite en partie",
        }
    }

    /// Son nom en JSON.
    pub fn code(&self) -> &'static str {
        match self {
            IssueEtape::Faite => "faite",
            IssueEtape::Echouee => "echouee",
            IssueEtape::PasFaite(_) => "pas_faite",
            IssueEtape::FaiteEnPartie(_) => "faite_en_partie",
        }
    }

    fn depuis_code(code: &str, raison: &str) -> Option<IssueEtape> {
        match code {
            "faite" => Some(IssueEtape::Faite),
            "echouee" => Some(IssueEtape::Echouee),
            "pas_faite" => Some(IssueEtape::PasFaite(String::from(raison))),
            "faite_en_partie" => Some(IssueEtape::FaiteEnPartie(String::from(raison))),
            _ => None,
        }
    }
}

/// Une règle de l'étape 1, vérifiée ou non.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Regle {
    /// Son nom court (« lints », « clippy wasm32 »…).
    pub nom: String,
    /// Son issue.
    pub issue: IssueEtape,
    /// Ce qui a été vu : ce qui manque, ou ce qui a été vérifié.
    pub detail: String,
}

impl Regle {
    /// Une règle tenue.
    pub fn faite(nom: &str, detail: impl Into<String>) -> Regle {
        Regle {
            nom: String::from(nom),
            issue: IssueEtape::Faite,
            detail: detail.into(),
        }
    }

    /// Une règle que le jeu ne tient pas.
    pub fn echouee(nom: &str, detail: impl Into<String>) -> Regle {
        Regle {
            nom: String::from(nom),
            issue: IssueEtape::Echouee,
            detail: detail.into(),
        }
    }

    /// Une règle que l'épreuve n'a pas pu vérifier.
    pub fn pas_faite(nom: &str, raison: impl Into<String>) -> Regle {
        let raison = raison.into();
        Regle {
            nom: String::from(nom),
            issue: IssueEtape::PasFaite(raison.clone()),
            detail: raison,
        }
    }
}

/// Ce que l'étape 1 a fait : d'où vient le module, et chaque règle.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Compilation {
    /// Le dossier du jeu, tel que la ligne de commande l'a donné.
    pub dossier: String,
    /// La crate de module compilée.
    pub module: String,
    /// Le type du jeu, tel que `jeu::module!` le nomme.
    pub jeu: String,
    /// Les règles, dans l'ordre où elles ont été vérifiées.
    pub regles: Vec<Regle>,
}

/// Échappe un champ d'une ligne du constat : barre oblique inverse, tabulation, fin
/// de ligne.
fn echapper(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\t' => o.push_str("\\t"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c => o.push(c),
        }
    }
    o
}

fn desechapper(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('t') => o.push('\t'),
            Some('n') => o.push('\n'),
            Some('r') => o.push('\r'),
            Some(autre) => o.push(autre),
            None => o.push('\\'),
        }
    }
    o
}

impl Compilation {
    /// L'issue de l'étape 1 : échouée si une règle échoue, pas faite si une règle n'a
    /// pas été vérifiée, faite sinon. Sans aucune règle, pas faite (OUT-07).
    pub fn issue(&self) -> IssueEtape {
        if self.regles.is_empty() {
            return IssueEtape::PasFaite(String::from("aucune règle vérifiée"));
        }
        if self.regles.iter().any(|r| r.issue == IssueEtape::Echouee) {
            return IssueEtape::Echouee;
        }
        let pas_faites: Vec<&str> = self
            .regles
            .iter()
            .filter(|r| matches!(r.issue, IssueEtape::PasFaite(_)))
            .map(|r| r.nom.as_str())
            .collect();
        if pas_faites.is_empty() {
            IssueEtape::Faite
        } else {
            IssueEtape::PasFaite(format!("pas vérifié : {}", pas_faites.join(", ")))
        }
    }

    /// Le constat écrit pour la copie native, que `soiree-epreuve` lance avec
    /// `--sources <fichier>` : une ligne d'en-tête, puis une ligne par règle, champs
    /// séparés par des tabulations.
    pub fn ecrire(&self) -> String {
        let mut s = format!(
            "constat-de-compilation 1\t{}\t{}\t{}\n",
            echapper(&self.dossier),
            echapper(&self.module),
            echapper(&self.jeu)
        );
        for r in &self.regles {
            let raison = match &r.issue {
                IssueEtape::PasFaite(raison) | IssueEtape::FaiteEnPartie(raison) => raison.as_str(),
                IssueEtape::Faite | IssueEtape::Echouee => "",
            };
            let _: Result<(), std::fmt::Error> = writeln!(
                s,
                "{}\t{}\t{}\t{}",
                echapper(&r.nom),
                r.issue.code(),
                echapper(raison),
                echapper(&r.detail)
            );
        }
        s
    }

    /// Relit un constat écrit par [`Compilation::ecrire`].
    pub fn lire(texte: &str) -> Option<Compilation> {
        let mut lignes = texte.lines();
        let entete: Vec<&str> = lignes.next()?.split('\t').collect();
        let (Some(&"constat-de-compilation 1"), Some(d), Some(m), Some(j), None) = (
            entete.first(),
            entete.get(1),
            entete.get(2),
            entete.get(3),
            entete.get(4),
        ) else {
            return None;
        };
        let mut c = Compilation {
            dossier: desechapper(d),
            module: desechapper(m),
            jeu: desechapper(j),
            regles: Vec::new(),
        };
        for l in lignes {
            let champs: Vec<&str> = l.split('\t').collect();
            let (Some(nom), Some(code), Some(raison), Some(detail), None) = (
                champs.first(),
                champs.get(1),
                champs.get(2),
                champs.get(3),
                champs.get(4),
            ) else {
                return None;
            };
            c.regles.push(Regle {
                nom: desechapper(nom),
                issue: IssueEtape::depuis_code(code, &desechapper(raison))?,
                detail: desechapper(detail),
            });
        }
        Some(c)
    }

    pub(crate) fn json(&self) -> Json {
        Json::Objet(vec![
            ("dossier", Json::texte(&self.dossier)),
            ("module", Json::texte(&self.module)),
            ("jeu", Json::texte(&self.jeu)),
            (
                "regles",
                Json::Liste(
                    self.regles
                        .iter()
                        .map(|r| {
                            Json::Objet(vec![
                                ("nom", Json::texte(&r.nom)),
                                ("issue", Json::texte(r.issue.code())),
                                ("detail", Json::texte(&r.detail)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }
}

/// Le résultat d'une étape, au rapport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultatEtape {
    /// L'étape.
    pub etape: Etape,
    /// Son issue.
    pub issue: IssueEtape,
    /// Ce qui l'a faite, ou pourquoi elle ne l'est pas.
    pub detail: String,
}

impl ResultatEtape {
    /// Est-elle exigée et pas faite ?
    pub fn manque(&self) -> bool {
        self.etape.exigence() == Exigence::Exigee && matches!(self.issue, IssueEtape::PasFaite(_))
    }
}

#[cfg(test)]
mod tests {
    use super::{Compilation, Etape, Exigence, IssueEtape, Regle};

    #[test]
    fn le_constat_se_relit() {
        let c = Compilation {
            dossier: String::from("games/punchline"),
            module: String::from("punchline-module"),
            jeu: String::from("punchline::Punchline"),
            regles: vec![
                Regle::faite("lints", "identiques\tau commun"),
                Regle::echouee("allow", "src/lib.rs:3 :\n#[allow(x)]"),
                Regle::pas_faite("clippy", "cargo absent \\ introuvable"),
            ],
        };
        assert_eq!(Compilation::lire(&c.ecrire()), Some(c.clone()));
        assert_eq!(c.issue(), IssueEtape::Echouee);
        assert_eq!(Compilation::lire("autre chose"), None);
    }

    /// OUT-07 : une étape 1 sans règle vérifiée n'est pas faite.
    #[test]
    fn sans_regle_pas_faite() {
        assert!(matches!(
            Compilation::default().issue(),
            IssueEtape::PasFaite(_)
        ));
        let c = Compilation {
            regles: vec![
                Regle::faite("a", ""),
                Regle::pas_faite("b", "faute de cargo"),
            ],
            ..Compilation::default()
        };
        assert_eq!(
            c.issue(),
            IssueEtape::PasFaite(String::from("pas vérifié : b"))
        );
    }

    #[test]
    fn chaque_etape_non_exigee_dit_pourquoi() {
        for e in Etape::TOUTES {
            if let Exigence::NonExigee(r) = e.exigence() {
                assert!(!r.is_empty(), "{e:?}");
            }
        }
        let numeros: Vec<u8> = Etape::TOUTES.iter().map(|e| e.numero()).collect();
        assert_eq!(numeros, [1, 2, 3, 4, 5, 6, 7]);
    }
}
