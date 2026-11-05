//! E20, « Deux moteurs » (PHASE-3, § 8.2 et § 8.3, lot 3b) : les journaux de l'épreuve
//! repris dans de vrais navigateurs donnent, appel par appel, ce qu'ils donnent dans
//! wasmtime.
//!
//! 1. Les soirées jouées par l'épreuve (leur ouverture et leur journal) sont gardées en
//!    chemin, quand E20 est demandé.
//! 2. Le relevé de wasmtime : chaque soirée reprise depuis sa création, fautes refaites,
//!    sur une fabrique qui écrit chaque appel (`moteur_commun::releve`) ; avec le
//!    chargement du module (son empreinte réécrite, le carburant de `Bonjour` et de
//!    `Decrire`, ou son refus classé). Avant les soirées, deux fautes `Budget`
//!    provoquées (`releve::rejouer`) : wasmtime doit les classer `Budget`, sinon E20
//!    est une panne de l'outil.
//! 3. Un dossier : le module livré, les soirées, le relevé ; puis `soiree-banc rejouer`
//!    en sous-processus (le binaire de l'épreuve ne tire pas les dépendances du banc,
//!    PHASE-3, § 2.2). Il cherche `soiree-banc` dans `SOIREE_BANC`, sinon à côté de
//!    l'exécutable qui tourne.
//! 4. Un résultat par navigateur, que le banc écrit en JSON : réussi, échec (la première
//!    entrée qui diverge), panne de l'outil ou pas exécuté. E20 prend le pire. Le code
//!    de sortie du banc doit s'accorder avec son JSON ; un banc qui s'arrête avant tout
//!    navigateur garde la nature de son arrêt (« pas exécuté » pour un navigateur
//!    absent, panne de l'outil pour la règle « mêmes lints » qui refuse `web/`, un
//!    résultat illisible ou absent).
//!
//! Le dossier est effacé quand E20 réussit ; sinon il reste, avec ce que le banc a dit
//! (`banc.txt`), pour refaire `soiree-banc rejouer <dossier>` à la main (HIS-06).
//!
//! Le contrôle négatif, joué par le banc à chaque E20, est le moteur fautif
//! `fausser-apres:<n>` ; [`crate::CommunFautif::FausserApres`] le fait jouer à la place
//! du vrai moteur, pour montrer que E20 échoue, et lui seul.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use moteur_commun::releve::{self, Chargement, Enregistreuse, Evenement, Releve, SoireeJouee};
use moteur_wasmtime::{ModuleCharge, RefusModule};

use crate::plan::{CommunFautif, IdControle, Plan};
use crate::rapport::{IssueControle, ResultatControle, hex};

/// Ce que E20 a vu dans un navigateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigateurE20 {
    /// Le navigateur, par son nom (`chrome`).
    pub nom: String,
    /// Sa version, telle qu'il la dit.
    pub version: String,
    /// L'issue.
    pub issue: IssueControle,
    /// Ce qui a été vérifié, ou ce qui diverge.
    pub detail: String,
    /// La durée murale dans ce navigateur, de l'ouverture de la page au rapport, en ms.
    pub duree_ms: u64,
    /// La reprise des soirées seule, mesurée par la page, en ms.
    pub rejeu_ms: u64,
    /// Le navigateur entier : son lancement, la page, sa fermeture, en ms.
    pub total_ms: u64,
}

/// L'heure de la machine : E20 dit combien il a duré (RESTE-PHASE-2, point 14), et
/// aucun contrôle n'en dépend.
#[expect(
    clippy::disallowed_methods,
    reason = "E20 mesure sa propre durée murale pour le rapport ; aucun jugement n'en dépend"
)]
fn maintenant() -> Instant {
    Instant::now()
}

fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// Le relevé de wasmtime, et ce que wasmtime n'a pas su reprendre de son propre journal.
fn releve_de_wasmtime(
    module: Option<&ModuleCharge>,
    refus: Option<&RefusModule>,
    soirees: &[SoireeJouee],
) -> (Releve, Vec<String>) {
    let Some(m) = module else {
        let chargement =
            Chargement::Refuse(refus.map_or_else(|| String::from("?"), releve::classer_refus));
        return (
            Releve {
                chargement,
                evenements: Vec::new(),
            },
            Vec::new(),
        );
    };
    let e = Enregistreuse::new(m.fabrique());
    releve::rejouer(&e, soirees);
    let evenements = e.evenements();
    let mut rang = 0usize;
    let mut arrets = Vec::new();
    for ev in &evenements {
        match ev {
            Evenement::Soiree(k) => rang = usize::try_from(*k).unwrap_or(usize::MAX),
            Evenement::Fin(fin) if fin != releve::REJOUEE => {
                let nom = soirees.get(rang).map_or("?", |s| s.nom.as_str());
                arrets.push(format!("{nom} : {fin}"));
            }
            Evenement::Fin(_)
            | Evenement::Neuve(_)
            | Evenement::Appel { .. }
            | Evenement::Instantane(..)
            | Evenement::Restaurer(..)
            | Evenement::Empreinte(..) => {}
        }
    }
    (
        Releve {
            chargement: Chargement::Accepte {
                empreinte: m.reecrit().empreinte,
                carburant: m.carburant_du_chargement(),
            },
            evenements,
        },
        arrets,
    )
}

/// Le binaire `soiree-banc` : `SOIREE_BANC`, sinon à côté de l'exécutable qui tourne.
pub(crate) fn banc() -> Result<PathBuf, String> {
    if let Some(b) = std::env::var_os("SOIREE_BANC") {
        let b = PathBuf::from(b);
        return if b.is_file() {
            Ok(b)
        } else {
            Err(format!("SOIREE_BANC = {} : absent", b.display()))
        };
    }
    let exe = std::env::current_exe().map_err(|e| format!("l'exécutable qui tourne : {e}"))?;
    let b = exe.with_file_name("soiree-banc");
    if b.is_file() {
        Ok(b)
    } else {
        Err(format!(
            "soiree-banc introuvable ({} ; SOIREE_BANC non posé) : `cargo build --release -p banc` dans le commun",
            b.display()
        ))
    }
}

fn issue_du_banc(code: &str) -> IssueControle {
    match code {
        "reussi" => IssueControle::Reussi,
        "echec" => IssueControle::EchecDuJeu,
        "pas_execute" => IssueControle::PasExecute,
        _ => IssueControle::PanneDeLOutil,
    }
}

/// Le pire de deux issues : un échec, puis une panne, puis « pas exécuté ».
fn pire(a: IssueControle, b: &IssueControle) -> IssueControle {
    let rang = |i: &IssueControle| match i {
        IssueControle::EchecDuJeu => 3,
        IssueControle::PanneDeLOutil => 2,
        IssueControle::PasExecute => 1,
        IssueControle::Reussi | IssueControle::SansObjet(_) => 0,
    };
    if rang(b) > rang(&a) { b.clone() } else { a }
}

/// Le banc s'est arrêté avant de rendre un résultat par navigateur : ce que E20 en dit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ArretDuBanc {
    /// L'issue de E20.
    pub issue: IssueControle,
    /// Pourquoi.
    pub detail: String,
}

fn panne(detail: String) -> ArretDuBanc {
    ArretDuBanc {
        issue: IssueControle::PanneDeLOutil,
        detail,
    }
}

/// Le code de sortie du banc doit dire ce que son JSON dit (`soiree-banc`, « Code de
/// sortie ») : 0 si chaque navigateur a réussi, 1 si l'un échoue, 2 sinon. Un code
/// qui ne s'accorde pas, ou un banc tué par un signal, est une panne de l'outil : son
/// JSON ne se croit pas.
pub(crate) fn code_coherent(
    code: Option<i32>,
    navigateurs: &[NavigateurE20],
) -> Result<(), ArretDuBanc> {
    let attendu = if navigateurs
        .iter()
        .any(|n| n.issue == IssueControle::EchecDuJeu)
    {
        1
    } else if navigateurs.iter().all(|n| n.issue == IssueControle::Reussi) {
        0
    } else {
        2
    };
    match code {
        Some(c) if c == attendu => Ok(()),
        Some(c) => Err(panne(format!(
            "le banc rend le code {c}, et son résultat en demande {attendu}"
        ))),
        None => Err(panne(String::from("le banc s'est arrêté sur un signal"))),
    }
}

/// Lit le JSON que `soiree-banc rejouer` écrit : un résultat par navigateur, et la durée
/// de sa construction (la règle « mêmes lints » et l'exemple du navigateur), en ms.
///
/// Un arrêt avant tout navigateur garde sa nature : `pas_execute` (un navigateur ou un
/// fichier absent, une construction ratée) reste « pas exécuté » ; `echec` (la règle
/// « mêmes lints » qui refuse `web/`) est une panne de l'outil, puisque c'est le
/// commun qui est en défaut, pas le jeu ; un JSON illisible aussi.
pub(crate) fn lire_le_banc(texte: &str) -> Result<(Vec<NavigateurE20>, u64), ArretDuBanc> {
    let v: serde_json::Value = serde_json::from_str(texte)
        .map_err(|e| panne(format!("résultat du banc illisible : {e}")))?;
    if let Some(a) = v.get("arret").and_then(serde_json::Value::as_str) {
        let detail = v
            .get("detail")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        return Err(ArretDuBanc {
            issue: if a == "pas_execute" {
                IssueControle::PasExecute
            } else {
                IssueControle::PanneDeLOutil
            },
            detail: format!("le banc s'arrête ({a}) : {detail}"),
        });
    }
    let liste = v
        .get("navigateurs")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| panne(String::from("résultat du banc sans « navigateurs »")))?;
    let texte_de = |n: &serde_json::Value, cle: &str| {
        n.get(cle)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let construction = v
        .get("construction_ms")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let navigateurs = liste
        .iter()
        .map(|n| {
            // `as` sature : une durée négative ou illisible donne 0, la fraction est
            // tronquée.
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "une durée en ms, sous 2^53, ramenée à l'entier par une conversion saturée"
            )]
            let rejeu_ms = n
                .get("rejeu_ms")
                .and_then(serde_json::Value::as_f64)
                .unwrap_or(0.0) as u64;
            NavigateurE20 {
                nom: texte_de(n, "navigateur"),
                version: texte_de(n, "version"),
                issue: issue_du_banc(&texte_de(n, "issue")),
                detail: texte_de(n, "detail"),
                duree_ms: n
                    .get("duree_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                rejeu_ms,
                total_ms: n
                    .get("total_ms")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
            }
        })
        .collect();
    Ok((navigateurs, construction))
}

fn ecrire(dossier: &Path, nom: &str, octets: &[u8]) -> Result<(), String> {
    let f = dossier.join(nom);
    std::fs::write(&f, octets).map_err(|e| format!("{} : {e}", f.display()))
}

fn resultat(issue: IssueControle, evaluations: u64, detail: String) -> ResultatControle {
    ResultatControle {
        id: IdControle::E20,
        issue,
        evaluations,
        detail,
        rejeu: None,
    }
}

/// Joue E20 : le relevé de wasmtime, puis `soiree-banc rejouer` dans chaque navigateur
/// du plan. Rend le résultat de E20 et ce que chaque navigateur a vu.
#[expect(
    clippy::too_many_lines,
    reason = "E20 se lit d'un trait : le relevé, le dossier, le banc, le jugement"
)]
pub(crate) fn juger(
    octets: &[u8],
    module: Option<&ModuleCharge>,
    refus: Option<&RefusModule>,
    soirees: &[SoireeJouee],
    plan: &Plan,
) -> (ResultatControle, Vec<NavigateurE20>) {
    if plan.navigateurs.is_empty() {
        return (
            resultat(
                IssueControle::PasExecute,
                0,
                String::from("aucun navigateur demandé (--navigateurs chrome,firefox)"),
            ),
            Vec::new(),
        );
    }
    let debut = maintenant();
    let (releve_wasmtime, arrets) = releve_de_wasmtime(module, refus, soirees);
    let duree_wasmtime = ms(debut.elapsed());
    let (appels, fautes) = releve::compter(&releve_wasmtime.evenements);
    // Les fautes `Budget` provoquées avant les soirées : sur un module accepté qui a
    // joué, wasmtime doit en faire des `Budget`, sinon rien de cette sorte ne se
    // compare, et E20 ne peut pas le taire.
    let provoquees = releve::budgets_provoques(&releve_wasmtime.evenements);
    if module.is_some() && !soirees.is_empty() && provoquees == 0 {
        return (
            resultat(
                IssueControle::PanneDeLOutil,
                0,
                String::from(
                    "wasmtime ne classe pas Budget les fautes provoquées avant les soirées (Decrire à la moitié de son carburant, puis à une unité près) : la sorte Budget ne se comparerait pas",
                ),
            ),
            Vec::new(),
        );
    }
    if !arrets.is_empty() {
        return (
            resultat(
                IssueControle::PanneDeLOutil,
                0,
                format!(
                    "wasmtime ne reprend pas son propre journal depuis la création, fautes refaites : {} ; rien n'est comparé au-delà",
                    arrets.join(" ; ")
                ),
            ),
            Vec::new(),
        );
    }
    let banc = match banc() {
        Ok(b) => b,
        Err(e) => return (resultat(IssueControle::PasExecute, 0, e), Vec::new()),
    };
    let empreinte =
        module.map_or_else(|| *blake3::hash(octets).as_bytes(), ModuleCharge::empreinte);
    let dossier = std::env::temp_dir().join(format!(
        "soiree-e20-{}-{}",
        hex(empreinte.get(..8).unwrap_or_default()),
        std::process::id()
    ));
    let releve_octets = releve::encoder_releve(&releve_wasmtime);
    let prepare = std::fs::create_dir_all(&dossier)
        .map_err(|e| format!("{} : {e}", dossier.display()))
        .and_then(|()| ecrire(&dossier, "module.wasm", octets))
        .and_then(|()| ecrire(&dossier, "soirees.bin", &releve::encoder_soirees(soirees)))
        .and_then(|()| ecrire(&dossier, "releve.bin", &releve_octets));
    if let Err(e) = prepare {
        return (resultat(IssueControle::PasExecute, 0, e), Vec::new());
    }
    let sortie = dossier.join("resultat.json");
    let noms: Vec<&str> = plan.navigateurs.iter().map(|n| n.nom()).collect();
    let mut c = Command::new(&banc);
    c.arg("rejouer")
        .arg(&dossier)
        .args(["--navigateurs", &noms.join(",")])
        .arg("--sortie")
        .arg(&sortie);
    if let Some(CommunFautif::FausserApres(n)) = plan.commun_fautif {
        c.args(["--fautif", &format!("fausser-apres:{n}")]);
    }
    let journal_du_banc = std::fs::File::create(dossier.join("banc.txt"));
    match journal_du_banc {
        Ok(f) => {
            c.stdout(Stdio::from(f));
        }
        Err(_) => {
            c.stdout(Stdio::null());
        }
    }
    let debut_banc = maintenant();
    let statut = c.stdin(Stdio::null()).status();
    let duree_banc = ms(debut_banc.elapsed());
    let lu = match statut {
        Err(e) => Err(ArretDuBanc {
            issue: IssueControle::PanneDeLOutil,
            detail: format!("lancer {} : {e}", banc.display()),
        }),
        Ok(s) => match std::fs::read_to_string(&sortie) {
            Ok(t) => lire_le_banc(&t).and_then(|(n, construction)| {
                code_coherent(s.code(), &n).map(|()| (n, construction))
            }),
            Err(e) => Err(ArretDuBanc {
                issue: IssueControle::PanneDeLOutil,
                detail: format!(
                    "le banc rend {} sans écrire {} : {e}",
                    s.code()
                        .map_or_else(|| String::from("sur un signal"), |c| format!("le code {c}")),
                    sortie.display()
                ),
            }),
        },
    };
    let (navigateurs, construction_ms) = match lu {
        Ok(n) => n,
        Err(a) => {
            return (
                resultat(
                    a.issue,
                    0,
                    format!("{} ; le dossier reste : {}", a.detail, dossier.display()),
                ),
                Vec::new(),
            );
        }
    };
    let mut issue = IssueControle::Reussi;
    let mut evaluations = 0u64;
    let mut lignes = vec![format!(
        "relevé de wasmtime : {} soirée(s), {} événements, dont {appels} appels ; fautes : {} (dont {provoquees} Budget provoquées avant les soirées) ; en {duree_wasmtime} ms ; le banc en {duree_banc} ms, dont {construction_ms} ms de construction",
        soirees.len(),
        releve_wasmtime.evenements.len(),
        if fautes.is_empty() {
            String::from("aucune")
        } else {
            fautes
                .iter()
                .map(|(s, n)| format!("{s} × {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    )];
    if navigateurs.len() != plan.navigateurs.len() {
        issue = IssueControle::PanneDeLOutil;
        lignes.push(format!(
            "le banc rend {} navigateur(s), le plan en demande {}",
            navigateurs.len(),
            plan.navigateurs.len()
        ));
    }
    for n in &navigateurs {
        issue = pire(issue, &n.issue);
        if n.issue == IssueControle::Reussi {
            evaluations = evaluations.saturating_add(appels.max(1));
        }
        lignes.push(format!(
            "{} ({}) : {} en {} ms, reprise {} ms, {} ms lancement et fermeture compris — {}",
            n.nom,
            n.version,
            n.issue.nom(),
            n.duree_ms,
            n.rejeu_ms,
            n.total_ms,
            n.detail
        ));
    }
    if issue == IssueControle::Reussi {
        let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    } else {
        lignes.push(format!(
            "le dossier reste, pour `soiree-banc rejouer {}` : {}",
            dossier.display(),
            dossier.join("banc.txt").display()
        ));
    }
    (resultat(issue, evaluations, lignes.join("\n")), navigateurs)
}

#[cfg(test)]
mod tests {
    use super::{IssueControle, code_coherent, lire_le_banc, pire};

    #[test]
    fn le_resultat_du_banc_se_lit() {
        let (n, construction) = lire_le_banc(
            r#"{"construction_ms": 640, "navigateurs": [
                {"navigateur": "chrome", "version": "HeadlessChrome/156", "issue": "reussi", "detail": "égal", "duree_ms": 1200, "rejeu_ms": 340.6},
                {"navigateur": "firefox", "version": "firefox 158.0.1", "issue": "echec", "detail": "événement 3", "duree_ms": 900, "rejeu_ms": 12.0},
                {"navigateur": "safari", "version": "", "issue": "pas_execute", "detail": "lot 3c", "duree_ms": 0, "rejeu_ms": 0}
            ]}"#,
        )
        .unwrap();
        assert_eq!((n.len(), construction), (3, 640));
        assert_eq!(n[0].issue, IssueControle::Reussi);
        assert_eq!(n[0].rejeu_ms, 340);
        assert_eq!(n[1].issue, IssueControle::EchecDuJeu);
        assert_eq!(n[2].issue, IssueControle::PasExecute);
        // Un arrêt garde sa nature : un navigateur absent n'est pas exécuté ; la règle
        // « mêmes lints » qui refuse `web/`, ou un JSON illisible, est une panne de
        // l'outil, jamais un succès ni un échec du jeu.
        let issue = |t: &str| lire_le_banc(t).map(|_| ()).unwrap_err().issue;
        assert_eq!(
            issue(r#"{"arret": "pas_execute", "detail": "Chrome absent"}"#),
            IssueControle::PasExecute
        );
        assert_eq!(
            issue(r#"{"arret": "echec", "detail": "la règle « mêmes lints » refuse web/"}"#),
            IssueControle::PanneDeLOutil
        );
        assert_eq!(issue("{}"), IssueControle::PanneDeLOutil);
        assert_eq!(issue("pas du json"), IssueControle::PanneDeLOutil);
    }

    /// Le code de sortie du banc doit dire ce que son JSON dit ; sinon, ou tué par un
    /// signal, c'est une panne de l'outil.
    #[test]
    fn le_code_du_banc_doit_s_accorder() {
        let (n, _) = lire_le_banc(
            r#"{"navigateurs": [
                {"navigateur": "chrome", "issue": "reussi"},
                {"navigateur": "firefox", "issue": "echec"}
            ]}"#,
        )
        .unwrap();
        assert!(code_coherent(Some(1), &n).is_ok());
        for faux in [Some(0), Some(2), Some(101), None] {
            assert_eq!(
                code_coherent(faux, &n).unwrap_err().issue,
                IssueControle::PanneDeLOutil,
                "{faux:?}"
            );
        }
        let reussis = n.get(..1).unwrap();
        assert!(code_coherent(Some(0), reussis).is_ok());
        assert!(code_coherent(Some(1), reussis).is_err());
        assert!(code_coherent(Some(2), &[]).is_err());
    }

    /// Un échec l'emporte sur une panne, qui l'emporte sur « pas exécuté » : E20 n'est
    /// réussi que si chaque navigateur l'est.
    #[test]
    fn e20_prend_le_pire() {
        let i = pire(IssueControle::Reussi, &IssueControle::PasExecute);
        assert_eq!(i, IssueControle::PasExecute);
        let i = pire(i, &IssueControle::PanneDeLOutil);
        assert_eq!(i, IssueControle::PanneDeLOutil);
        let i = pire(i, &IssueControle::Reussi);
        assert_eq!(i, IssueControle::PanneDeLOutil);
        assert_eq!(
            pire(i, &IssueControle::EchecDuJeu),
            IssueControle::EchecDuJeu
        );
    }
}
