//! Le scénario `soiree-banc rejouer` : E20, « Deux moteurs », dans de vrais navigateurs
//! (PHASE-3, § 8.2 et § 8.3, lot 3b).
//!
//! `soiree-epreuve <jeu> --navigateurs …` joue son plan dans wasmtime, reprend chaque
//! soirée depuis sa création avec un relevé de chaque appel
//! (`moteur_commun::releve`), écrit un dossier, puis lance ce scénario en
//! sous-processus. Le dossier contient :
//!
//! - `module.wasm`, le module livré ;
//! - `soirees.bin`, les soirées de l'épreuve (ouverture et journal de chacune) ;
//! - `releve.bin`, le relevé de wasmtime.
//!
//! Le scénario :
//!
//! 1. La règle « mêmes lints » de `web/`, puis la construction de l'exemple
//!    `rejeu-navigateur` et de sa colle, finie avant qu'un navigateur ne parte
//!    (OUT-89), sauf `--sans-construire`.
//! 2. Pour chaque navigateur, un navigateur neuf sur un profil neuf, et la page
//!    `pages/rejouer.html`, dont le Worker charge le module comme l'autorité le chargera
//!    (réécriture, compilation, `Bonjour`, `Decrire`), reprend chaque soirée par la même
//!    procédure que wasmtime et compare chaque événement au relevé de wasmtime.
//! 3. Deux passages dans la même page : le vrai moteur (ou le moteur fautif que
//!    l'épreuve demande, `--fautif`), puis **le contrôle négatif**, joué à chaque fois :
//!    le moteur fautif `fausser-apres:<n>`, qui doit diverger au `n + 1`-ième appel, sur
//!    le carburant, et pas avant. Sans lui, E20 ne jugerait rien.
//!
//! Le jugement ne croit pas la page sur parole : le relevé qu'elle a obtenu doit avoir
//! l'empreinte (BLAKE3) du fichier que le banc a servi, et autant d'événements.
//!
//! Le résultat, un par navigateur : réussi, échec (la première entrée qui diverge),
//! panne de l'outil (la page n'a pas tourné, le contrôle négatif n'a rien vu), ou pas
//! exécuté (un navigateur que la machine ne permet pas : Safari, tant que l'hôte n'a pas
//! autorisé l'automatisation à distance, avec le message de `safaridriver`). Il va sur
//! la sortie standard, et en JSON dans le fichier `--sortie` que l'épreuve relit.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::horloge::{maintenant, reste};
use crate::machine::{Arret, construire};
use crate::navigateur::{Lance, Navigateur};
use crate::serveur::{Requete, Serveur};
use crate::{commun, web};

/// Le plus d'appels que le moteur fautif du contrôle négatif laisse passer.
pub const FAUSSER_APRES_MAX: u64 = 100;

/// Le préfixe du moteur fautif.
pub const FAUSSER_APRES: &str = "fausser-apres:";

/// Les réglages du scénario.
#[derive(Debug, Clone)]
pub struct Options {
    /// Construire l'exemple et sa colle avant de jouer.
    pub construire: bool,
    /// Les navigateurs, dans l'ordre.
    pub navigateurs: Vec<Navigateur>,
    /// Le moteur fautif que l'épreuve demande à la place du vrai (`fausser-apres:<n>`),
    /// pour le contrôle négatif de E20 dans l'épreuve.
    pub fautif: Option<String>,
    /// Le plus long qu'on attend le rapport d'une page.
    pub delai: Duration,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            construire: true,
            navigateurs: vec![Navigateur::Chrome, Navigateur::Firefox],
            fautif: None,
            delai: Duration::from_mins(30),
        }
    }
}

/// L'issue de E20 dans un navigateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Issue {
    /// Chaque événement est celui de wasmtime, et le contrôle négatif a vu le moteur
    /// fautif.
    Reussi,
    /// Un événement diffère : la première entrée qui diverge est dite.
    Echec,
    /// La page n'a pas tourné, ou le contrôle négatif n'a rien vu : rien n'est jugé.
    Panne,
    /// Le navigateur ne part pas, ou le banc ne sait pas le piloter.
    PasExecute,
}

impl Issue {
    /// Son nom en JSON, celui que l'épreuve relit.
    pub const fn code(self) -> &'static str {
        match self {
            Issue::Reussi => "reussi",
            Issue::Echec => "echec",
            Issue::Panne => "panne",
            Issue::PasExecute => "pas_execute",
        }
    }
}

/// Ce que le banc sait du relevé de wasmtime, sans croire la page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attendu {
    /// Le BLAKE3 du relevé de wasmtime, tel qu'il est servi.
    pub empreinte: String,
    /// Ses événements.
    pub evenements: u64,
    /// Ses appels.
    pub appels: u64,
}

impl Attendu {
    /// Lit le relevé de wasmtime.
    pub fn lire(octets: &[u8]) -> Result<Attendu, String> {
        let r = moteur_commun::releve::decoder_releve(octets)
            .map_err(|e| format!("relevé de wasmtime illisible : {e}"))?;
        let (appels, _) = moteur_commun::releve::compter(&r.evenements);
        Ok(Attendu {
            empreinte: moteur_commun::releve::hex(&moteur_commun::releve::empreinte(octets)),
            evenements: u64::try_from(r.evenements.len()).unwrap_or(u64::MAX),
            appels,
        })
    }

    /// Le moteur fautif du contrôle négatif : `n` appels avant de fausser, au plus
    /// [`FAUSSER_APRES_MAX`], au milieu des appels s'il y en a peu ; `None` s'il n'y a
    /// aucun appel à fausser (un module refusé au chargement).
    pub fn negatif(&self) -> Option<u64> {
        (self.appels > 0).then(|| (self.appels / 2).min(FAUSSER_APRES_MAX))
    }
}

/// Le résultat de E20 dans un navigateur.
#[derive(Debug, Clone)]
pub struct Resultat {
    /// Le navigateur.
    pub navigateur: Navigateur,
    /// Sa version, telle qu'il la dit.
    pub version: String,
    /// L'issue.
    pub issue: Issue,
    /// Ce qui a été vérifié, ou ce qui diverge.
    pub detail: String,
    /// La durée murale, de l'ouverture de la page au rapport, en millisecondes.
    pub duree_ms: u64,
    /// La durée murale du navigateur entier : son lancement, la page, sa fermeture.
    pub total_ms: u64,
    /// Ce que la page a mesuré du vrai passage : le chargement et la reprise, en ms.
    pub chargement_ms: f64,
    /// La reprise des soirées.
    pub rejeu_ms: f64,
    /// Les appels comparés, et les fautes classées, par sorte.
    pub appels: u64,
    /// Les fautes classées, par sorte.
    pub fautes: Vec<(String, u64)>,
    /// Ce que la console et les exceptions de la page ont dit.
    pub console: Vec<String>,
}

impl Resultat {
    /// En JSON.
    pub fn json(&self) -> Json {
        json!({
            "navigateur": self.navigateur.nom(),
            "version": self.version,
            "issue": self.issue.code(),
            "detail": self.detail,
            "duree_ms": self.duree_ms,
            "total_ms": self.total_ms,
            "chargement_ms": self.chargement_ms,
            "rejeu_ms": self.rejeu_ms,
            "appels": self.appels,
            "fautes": self.fautes.iter().map(|(s, n)| json!([s, n])).collect::<Vec<_>>(),
            "console": self.console,
        })
    }
}

/// Ce qu'un passage de la page a rapporté.
#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    /// Le moteur joué : `""`, ou le moteur fautif.
    pub fautif: String,
    /// Le chargement : `accepté`, ou le refus.
    pub chargement: String,
    /// Les événements obtenus.
    pub obtenus: u64,
    /// Les appels obtenus.
    pub appels: u64,
    /// Les fautes classées.
    pub fautes: Vec<(String, u64)>,
    /// Le BLAKE3 du relevé obtenu.
    pub empreinte: String,
    /// La première différence, décrite.
    pub ecart: Option<String>,
    /// Le rang, parmi les appels, de l'appel où tombe la différence.
    pub ecart_appel: Option<u64>,
    /// Le chargement, en ms.
    pub chargement_ms: f64,
    /// La reprise, en ms.
    pub rejeu_ms: f64,
}

fn texte(v: &Json, cle: &str) -> Result<String, String> {
    v.get(cle)
        .and_then(Json::as_str)
        .map(String::from)
        .ok_or_else(|| format!("« {cle} » manque : {v}"))
}

fn entier(v: &Json, cle: &str) -> Result<u64, String> {
    v.get(cle)
        .and_then(Json::as_u64)
        .ok_or_else(|| format!("« {cle} » manque : {v}"))
}

/// Lit le rapport de la page : `{ passages: [...] }`, ou une erreur si la page n'a pas
/// tourné (`{ erreur }`).
pub fn lire(corps: &[u8]) -> Result<Vec<Passage>, String> {
    let v: Json = serde_json::from_slice(corps).map_err(|e| format!("rapport illisible : {e}"))?;
    if let Some(e) = v.get("erreur").filter(|e| !e.is_null()) {
        return Err(format!(
            "le rejeu n'a pas tourné : {}",
            e.as_str().map_or_else(|| e.to_string(), String::from)
        ));
    }
    let mut passages = Vec::new();
    for p in v
        .get("passages")
        .and_then(Json::as_array)
        .ok_or_else(|| String::from("« passages » manque"))?
    {
        if let Some(e) = p.get("erreur").filter(|e| !e.is_null()) {
            return Err(format!(
                "un passage n'a pas tourné : {}",
                e.as_str().map_or_else(|| e.to_string(), String::from)
            ));
        }
        let mut fautes = Vec::new();
        for f in p
            .get("fautes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
        {
            let sorte = f.get(0).and_then(Json::as_str).unwrap_or("?");
            let n = f.get(1).and_then(Json::as_u64).unwrap_or(0);
            fautes.push((String::from(sorte), n));
        }
        passages.push(Passage {
            fautif: texte(p, "fautif")?,
            chargement: texte(p, "chargement")?,
            obtenus: entier(p, "obtenus")?,
            appels: entier(p, "appels")?,
            fautes,
            empreinte: texte(p, "empreinte")?,
            ecart: p.get("ecart").and_then(Json::as_str).map(String::from),
            ecart_appel: p.get("ecart_appel").and_then(Json::as_u64),
            chargement_ms: p.get("chargement_ms").and_then(Json::as_f64).unwrap_or(0.0),
            rejeu_ms: p.get("rejeu_ms").and_then(Json::as_f64).unwrap_or(0.0),
        });
    }
    Ok(passages)
}

/// Les passages que la page joue : le moteur demandé (`""` le vrai), puis le contrôle
/// négatif s'il y a des appels à fausser.
pub fn passes(attendu: &Attendu, fautif: Option<&str>) -> Vec<String> {
    let mut v = vec![String::from(fautif.unwrap_or_default())];
    if let Some(n) = attendu.negatif() {
        v.push(format!("{FAUSSER_APRES}{n}"));
    }
    v
}

/// Juge les passages d'une page : le premier est le moteur demandé, le second le
/// contrôle négatif. Rend l'issue et ce qui a été vérifié.
pub fn juger(attendu: &Attendu, fautif: Option<&str>, passages: &[Passage]) -> (Issue, String) {
    let voulus = passes(attendu, fautif);
    let joues: Vec<&str> = passages.iter().map(|p| p.fautif.as_str()).collect();
    if joues != voulus.iter().map(String::as_str).collect::<Vec<_>>() {
        return (
            Issue::Panne,
            format!("la page a joué {joues:?}, le banc demandait {voulus:?}"),
        );
    }
    let mut notes = Vec::new();
    // Le contrôle négatif d'abord : s'il ne voit rien, rien n'est jugé.
    if let (Some(n), Some(negatif)) = (attendu.negatif(), passages.get(1)) {
        let vu = negatif
            .ecart
            .as_deref()
            .is_some_and(|e| e.contains("carburant"))
            && negatif.ecart_appel == Some(n);
        if !vu {
            return (
                Issue::Panne,
                format!(
                    "le contrôle négatif n'a pas vu le moteur fautif {FAUSSER_APRES}{n} (attendu : un écart de carburant à l'appel {n}) : {} : E20 ne juge rien",
                    negatif.ecart.as_deref().unwrap_or("aucun écart")
                ),
            );
        }
        notes.push(format!(
            "contrôle négatif : le moteur fautif {FAUSSER_APRES}{n} diverge à l'appel {n}, sur le carburant"
        ));
    } else {
        notes.push(String::from(
            "contrôle négatif sans objet : aucun appel à fausser (module refusé au chargement)",
        ));
    }
    let Some(vrai) = passages.first() else {
        return (Issue::Panne, String::from("aucun passage"));
    };
    if let Some(e) = &vrai.ecart {
        return (
            Issue::Echec,
            format!("{} ; le premier écart : {e}", vrai.chargement),
        );
    }
    if vrai.empreinte != attendu.empreinte || vrai.obtenus != attendu.evenements {
        return (
            Issue::Panne,
            format!(
                "la page ne voit aucun écart, mais son relevé ({} événements, {}) n'est pas celui de wasmtime ({} événements, {})",
                vrai.obtenus, vrai.empreinte, attendu.evenements, attendu.empreinte
            ),
        );
    }
    let fautes: Vec<String> = vrai
        .fautes
        .iter()
        .map(|(s, n)| format!("{s} × {n}"))
        .collect();
    notes.insert(
        0,
        format!(
            "module {} ; {} événements, dont {} appels, égaux à ceux de wasmtime (relevé {}…) ; fautes classées pareil : {}",
            vrai.chargement,
            attendu.evenements,
            attendu.appels,
            attendu.empreinte.get(..16).unwrap_or_default(),
            if fautes.is_empty() {
                String::from("aucune")
            } else {
                fautes.join(", ")
            }
        ),
    );
    (Issue::Reussi, notes.join(" ; "))
}

/// Un navigateur : neuf, la page, son rapport, puis fermé.
fn passer(
    n: Navigateur,
    o: &Options,
    origine: &str,
    profils: &Path,
    recus: &mpsc::Receiver<Requete>,
    attendu: &Attendu,
) -> Resultat {
    let mut r = Resultat {
        navigateur: n,
        version: String::new(),
        issue: Issue::PasExecute,
        detail: String::new(),
        duree_ms: 0,
        total_ms: 0,
        chargement_ms: 0.0,
        rejeu_ms: 0.0,
        appels: 0,
        fautes: Vec::new(),
        console: Vec::new(),
    };
    let lance = maintenant();
    let mut nav = match Lance::lancer(n, None, profils) {
        Ok(l) => l,
        Err(e) => {
            r.detail = format!("pas exécuté : {e}");
            return r;
        }
    };
    nav.version().clone_into(&mut r.version);
    // Un rapport d'un passage précédent ne compte pas pour celui-ci.
    while recus.try_recv().is_ok() {}
    let voulus = passes(attendu, o.fautif.as_deref());
    let url = format!("{origine}/banc/rejouer.html?passes={}", voulus.join(","));
    let debut = maintenant();
    let corps = nav.ouvrir(&url).and_then(|()| {
        let echeance = maintenant()
            .checked_add(o.delai)
            .ok_or_else(|| String::from("délai"))?;
        loop {
            if let Ok(q) = recus.recv_timeout(Duration::from_millis(200)) {
                r.console.extend(nav.ecouter(Duration::from_millis(200))?);
                return Ok(q.corps);
            }
            r.console.extend(nav.ecouter(Duration::from_millis(50))?);
            if reste(echeance).is_zero() {
                return Err(format!("aucun rapport en {:?}", o.delai));
            }
        }
    });
    r.duree_ms = u64::try_from(debut.elapsed().as_millis()).unwrap_or(u64::MAX);
    nav.fermer();
    r.total_ms = u64::try_from(lance.elapsed().as_millis()).unwrap_or(u64::MAX);
    match corps.and_then(|c| lire(&c)) {
        Err(e) => {
            r.issue = Issue::Panne;
            r.detail = e;
        }
        Ok(passages) => {
            if let Some(p) = passages.first() {
                r.chargement_ms = p.chargement_ms;
                r.rejeu_ms = p.rejeu_ms;
                r.appels = p.appels;
                r.fautes.clone_from(&p.fautes);
            }
            (r.issue, r.detail) = juger(attendu, o.fautif.as_deref(), &passages);
        }
    }
    r
}

/// Joue E20 sur le dossier `dossier`, dans chaque navigateur, un à la fois. Rend un
/// résultat par navigateur, et la durée de la construction (la règle « mêmes lints »
/// et l'exemple), en ms.
pub fn jouer(dossier: &Path, o: &Options) -> Result<(Vec<Resultat>, u64), Arret> {
    let releve = std::fs::read(dossier.join("releve.bin")).map_err(|e| {
        Arret::PasExecute(format!("{} : {e}", dossier.join("releve.bin").display()))
    })?;
    for f in ["module.wasm", "soirees.bin"] {
        if !dossier.join(f).is_file() {
            return Err(Arret::PasExecute(format!(
                "{} manque",
                dossier.join(f).display()
            )));
        }
    }
    let attendu = Attendu::lire(&releve).map_err(Arret::PasExecute)?;
    let debut = maintenant();
    let soiree = commun();
    let web_ = soiree.join("web");
    let ecarts = web::ecarts(&soiree, &web_);
    if !ecarts.is_empty() {
        return Err(Arret::Echec(format!(
            "la règle « mêmes lints » refuse web/ :\n  {}",
            ecarts.join("\n  ")
        )));
    }
    let banc = soiree.join("target/banc");
    let paquet = banc.join("paquet");
    if o.construire {
        construire(&web_, &paquet, &["rejeu-navigateur"]).map_err(Arret::PasExecute)?;
    }
    if !paquet.join("rejeu_navigateur.js").exists() {
        return Err(Arret::PasExecute(format!(
            "{} manque : construire d'abord (sans --sans-construire)",
            paquet.join("rejeu_navigateur.js").display()
        )));
    }
    let construction_ms = u64::try_from(debut.elapsed().as_millis()).unwrap_or(u64::MAX);
    let (envoi, recus) = mpsc::channel();
    let serveur = Serveur::demarrer(
        vec![
            (String::from("/banc/"), soiree.join("crates/banc/pages")),
            (String::from("/paquet/"), paquet),
            (String::from("/e20/"), PathBuf::from(dossier)),
        ],
        envoi,
    )
    .map_err(|e| Arret::PasExecute(format!("serveur : {e}")))?;
    let origine = serveur.origine();
    let profils = banc.join("profils");
    let resultats = o
        .navigateurs
        .iter()
        .map(|n| passer(*n, o, &origine, &profils, &recus, &attendu))
        .collect();
    serveur.arreter();
    Ok((resultats, construction_ms))
}

#[cfg(test)]
mod tests {
    use super::{Attendu, Issue, Passage, juger, lire, passes};

    fn attendu() -> Attendu {
        Attendu {
            empreinte: "ab".repeat(32),
            evenements: 1000,
            appels: 800,
        }
    }

    fn vrai() -> Passage {
        Passage {
            fautif: String::new(),
            chargement: String::from("accepté"),
            obtenus: 1000,
            appels: 800,
            fautes: vec![(String::from("Panique"), 2)],
            empreinte: "ab".repeat(32),
            ecart: None,
            ecart_appel: None,
            chargement_ms: 3.0,
            rejeu_ms: 40.0,
        }
    }

    fn negatif() -> Passage {
        Passage {
            fautif: String::from("fausser-apres:100"),
            obtenus: 150,
            appels: 101,
            empreinte: "cd".repeat(32),
            ecart: Some(String::from(
                "événement 140, soirée 0 : un carburant de 9 au lieu de 10, …",
            )),
            ecart_appel: Some(100),
            ..vrai()
        }
    }

    #[test]
    fn le_vrai_moteur_egal_et_le_negatif_vu_reussit() {
        let (issue, detail) = juger(&attendu(), None, &[vrai(), negatif()]);
        assert_eq!(issue, Issue::Reussi, "{detail}");
        assert!(detail.contains("Panique × 2"), "{detail}");
        assert!(detail.contains("contrôle négatif"), "{detail}");
    }

    #[test]
    fn un_ecart_est_un_echec_qui_le_nomme() {
        let mut v = vrai();
        v.ecart = Some(String::from("événement 7 : une autre issue"));
        let (issue, detail) = juger(&attendu(), None, &[v, negatif()]);
        assert_eq!(issue, Issue::Echec);
        assert!(detail.contains("événement 7"), "{detail}");
    }

    /// OUT-01 : la page dit « aucun écart », mais son relevé n'est pas celui que le banc
    /// a servi ; ou le contrôle négatif n'a rien vu : rien n'est jugé.
    #[test]
    fn la_page_n_est_pas_crue_sur_parole() {
        let mut v = vrai();
        v.empreinte = "00".repeat(32);
        assert_eq!(juger(&attendu(), None, &[v, negatif()]).0, Issue::Panne);
        let mut v = vrai();
        v.obtenus = 999;
        assert_eq!(juger(&attendu(), None, &[v, negatif()]).0, Issue::Panne);
        let mut n = negatif();
        n.ecart = None;
        assert_eq!(juger(&attendu(), None, &[vrai(), n]).0, Issue::Panne);
        let mut n = negatif();
        n.ecart_appel = Some(99);
        assert_eq!(juger(&attendu(), None, &[vrai(), n]).0, Issue::Panne);
        let mut n = negatif();
        n.ecart = Some(String::from("une autre issue"));
        assert_eq!(juger(&attendu(), None, &[vrai(), n]).0, Issue::Panne);
        // Un passage qui manque, ou d'un autre moteur.
        assert_eq!(juger(&attendu(), None, &[vrai()]).0, Issue::Panne);
        assert_eq!(
            juger(&attendu(), None, &[negatif(), negatif()]).0,
            Issue::Panne
        );
    }

    /// Le moteur fautif que l'épreuve demande (son contrôle négatif de E20) : le vrai
    /// passage diverge, c'est un échec.
    #[test]
    fn le_moteur_fautif_de_l_epreuve_fait_echouer() {
        let mut v = negatif();
        v.fautif = String::from("fausser-apres:3");
        let (issue, _) = juger(&attendu(), Some("fausser-apres:3"), &[v, negatif()]);
        assert_eq!(issue, Issue::Echec);
    }

    #[test]
    fn le_negatif_suit_le_nombre_d_appels() {
        let mut a = attendu();
        assert_eq!(passes(&a, None), ["", "fausser-apres:100"]);
        a.appels = 30;
        assert_eq!(passes(&a, None), ["", "fausser-apres:15"]);
        a.appels = 0;
        assert_eq!(passes(&a, None), [""]);
        let (issue, detail) = juger(
            &a,
            None,
            &[Passage {
                obtenus: 1000,
                ..vrai()
            }],
        );
        assert_eq!(issue, Issue::Reussi, "{detail}");
        assert!(detail.contains("sans objet"), "{detail}");
    }

    #[test]
    fn un_rapport_d_erreur_n_est_pas_un_succes() {
        assert!(lire(br#"{"erreur": "RangeError: Maximum call stack size exceeded"}"#).is_err());
        assert!(lire(br#"{"passages": [{"erreur": "fautif inconnu"}]}"#).is_err());
        assert!(lire(b"{}").is_err());
        assert!(lire(b"pas du json").is_err());
    }
}
