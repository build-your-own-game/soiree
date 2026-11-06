//! Le scénario `soiree-banc etalonner` (PHASE-3, § 8.3 et § 8.6, lot 3c) : M1, M10 et
//! la profondeur que tient chaque navigateur (M9), dans de vrais navigateurs.
//!
//! - **M1**, la mesure 1 du harnais refaite par le banc : la capture
//!   (`mesures/navigateur/capture`) a écrit, pour chaque module, le module livré, sa
//!   trace d'étalonnage (`moteur_commun::trace`) et le rejeu chronométré de cette trace
//!   dans wasmtime. Pour chaque module, un navigateur neuf : la page
//!   (`pages/etalonner.html`) charge le module comme l'autorité, chronomètre la mise en
//!   route, la remise à zéro et deux mille `Bonjour` d'un bloc, puis rejoue la trace
//!   deux fois sur le protocole du commun, chaque appel chronométré et comparé à
//!   wasmtime. Le banc en tire les chiffres de la mesure 1 ([`analyser`]) : le coût fixe
//!   d'un appel, la durée d'une unité de carburant sur les appels d'au moins un million
//!   d'unités, le facteur navigateur/wasmtime appel par appel, la remise à zéro d'un
//!   Kio, l'instantané, la restauration, la compilation. `--tours <n>` rejoue le tout
//!   `n` fois, les tours s'enchaînant d'un navigateur à l'autre comme au harnais : les
//!   durées d'un seul tour ne se retrouvent pas d'un lancement à l'autre.
//! - **M10** : le module de `Bonjour` qui boucle, chargé jusqu'à sa faute `Budget`
//!   (`BUDGET_DE_CHARGEMENT` unités), moins un témoin, en ps par unité ; puis la même
//!   boucle sous un plus grand budget. La charge de la machine, notée avant et après,
//!   se choisit dehors (quatorze `yes`, comme au lot 2).
//! - **M9** : des récursions sans fin, à cadres de plus en plus épais : les étages de
//!   `soiree_profondeur` que la pile native du Worker, puis de la page, tient ; puis les
//!   mêmes récursions chargées sous `PHASE_3`, qui doivent toutes être la faute
//!   `Profondeur` avant la pile native.
//!
//! Une mesure n'est pas un jugement, sauf deux choses qu'elle vérifie au passage et
//! qui font le code 1 : un écart avec wasmtime en M1 (carburant, issue, retour, module
//! réécrit ; D32) et, en M9, une récursion sous `PHASE_3` qui n'est pas la faute
//! `Profondeur` ou une récursion sans plafond que la pile native n'arrête pas (§ 8.7).
//! Le résumé les liste sous `defauts` ([`code_de_sortie`]). Le contrôle négatif de M1,
//! `--fautif fausser-apres:<n>`, rejoue la trace sur un moteur qui fausse le carburant
//! passé `n` appels : il doit compter un écart au `n + 1`-ième appel, donc le code 1.
//! Un contrôle qui ne le voit pas, ou pas là, ou qui n'a pas pu être jugé, est une
//! panne de l'outil, comme sous `rejouer --fautif` : le code 2, qui l'emporte sur
//! tout, et le résumé les liste sous `controle_rate`.
//!
//! La règle « mêmes lints » de `web/` et la construction de l'exemple
//! `etalonnage-navigateur` finissent avant qu'un navigateur ne parte (OUT-89). Un
//! navigateur à la fois, un module par navigateur neuf ; rien n'est retenté (OUT-91).
//! Les résultats bruts vont dans `<donnees>/etalonnage/<navigateur>/`, avec la charge
//! avant et après ; l'analyse sur la sortie standard, et dans `analyse.json`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use serde_json::{Value as Json, json};

use crate::horloge::{maintenant, reste};
use crate::machine::{Arret, construire};
use crate::navigateur::{Lance, Navigateur};
use crate::serveur::{Requete, Serveur};
use crate::{commun, web};

/// Une mesure du scénario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mesure {
    /// M1 : la trace de chaque module rejouée.
    M1,
    /// M10 : la boucle vide.
    M10,
    /// M9 : la profondeur que tient le navigateur.
    M9,
}

impl Mesure {
    /// Son nom sur la ligne de commande.
    pub const fn nom(self) -> &'static str {
        match self {
            Mesure::M1 => "m1",
            Mesure::M10 => "m10",
            Mesure::M9 => "m9",
        }
    }

    /// Des noms séparés par des virgules.
    pub fn liste(noms: &str) -> Result<Vec<Mesure>, String> {
        noms.split(',')
            .filter(|n| !n.trim().is_empty())
            .map(|n| {
                [Mesure::M1, Mesure::M10, Mesure::M9]
                    .into_iter()
                    .find(|m| m.nom() == n.trim())
                    .ok_or_else(|| format!("mesure inconnue : {n}"))
            })
            .collect()
    }
}

/// Les réglages du scénario.
#[derive(Debug, Clone)]
pub struct Options {
    /// Construire l'exemple et sa colle avant de jouer.
    pub construire: bool,
    /// Les navigateurs, dans l'ordre.
    pub navigateurs: Vec<Navigateur>,
    /// Les mesures, dans l'ordre.
    pub mesures: Vec<Mesure>,
    /// Le dossier de la capture : un sous-dossier par module.
    pub donnees: PathBuf,
    /// Les modules de M1 ; vide : tous ceux de la capture.
    pub modules: Vec<String>,
    /// Les rejeux de la trace par module (M1).
    pub passes: u32,
    /// Les tours de M1 : chaque tour rejoue chaque module dans un navigateur neuf, et
    /// les tours s'enchaînent d'un navigateur à l'autre (le harnais en avait quatre).
    /// Un tour ne dit pas ce qui revient d'un lancement à l'autre.
    pub tours: u32,
    /// Les chargements de la boucle (M10).
    pub fois: u32,
    /// Le grand budget de la boucle (M10).
    pub budget: u64,
    /// Le nom sous lequel les résultats s'écrivent (`chrome-sous-charge`) ; par défaut,
    /// celui du navigateur (suivi de `-fautif` sous le moteur fautif).
    pub etiquette: Option<String>,
    /// Le contrôle négatif de M1 : la trace rejouée sur le moteur fautif
    /// `fausser-apres:<n>`, qui doit compter un écart au `n + 1`-ième appel.
    pub fautif: Option<u64>,
    /// Le plus long qu'on attend le rapport d'une page.
    pub delai: Duration,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            construire: true,
            navigateurs: vec![Navigateur::Chrome, Navigateur::Firefox],
            mesures: vec![Mesure::M1, Mesure::M10, Mesure::M9],
            donnees: commun().join("target/mesures-navigateur"),
            modules: Vec::new(),
            passes: 2,
            tours: 1,
            fois: 15,
            budget: 1 << 30,
            etiquette: None,
            fautif: None,
            delai: Duration::from_mins(9),
        }
    }
}

/// La charge de la machine : `vm.loadavg`, et la mémoire libre que dit `memory_pressure`.
pub fn charge() -> String {
    let sortie_de = |c: &mut Command| {
        c.output()
            .ok()
            .map(|s| String::from_utf8_lossy(&s.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let moyenne = sortie_de(Command::new("sysctl").args(["-n", "vm.loadavg"]));
    let memoire = sortie_de(&mut Command::new("memory_pressure"));
    let libre = memoire
        .lines()
        .last()
        .and_then(|l| l.rsplit(':').next())
        .unwrap_or("?")
        .trim()
        .to_owned();
    format!("vm.loadavg {moyenne}, mémoire libre {libre}")
}

/// Ce qu'un passage a rapporté.
#[derive(Debug, Clone)]
pub struct Passage {
    /// Le navigateur.
    pub navigateur: Navigateur,
    /// Sa version.
    pub version: String,
    /// La mesure, et le module de M1.
    pub quoi: String,
    /// Le tour (M1 ; 1 pour les autres mesures).
    pub tour: u32,
    /// Le rapport de la page, ou pourquoi il manque.
    pub rapport: Result<Json, String>,
    /// La charge avant et après.
    pub charge: (String, String),
    /// La console de la page.
    pub console: Vec<String>,
}

fn passer(
    n: Navigateur,
    url: &str,
    (quoi, tour): (&str, u32),
    o: &Options,
    profils: &Path,
    recus: &mpsc::Receiver<Requete>,
) -> Passage {
    let mut passage = Passage {
        navigateur: n,
        version: String::new(),
        quoi: String::from(quoi),
        tour,
        rapport: Err(String::from("pas exécuté")),
        charge: (charge(), String::new()),
        console: Vec::new(),
    };
    let mut nav = match Lance::lancer(n, None, profils) {
        Ok(l) => l,
        Err(e) => {
            passage.rapport = Err(format!("pas exécuté : {e}"));
            passage.charge.1 = charge();
            return passage;
        }
    };
    nav.version().clone_into(&mut passage.version);
    while recus.try_recv().is_ok() {}
    let corps = nav.ouvrir(url).and_then(|()| {
        let echeance = maintenant()
            .checked_add(o.delai)
            .ok_or_else(|| String::from("délai"))?;
        loop {
            if let Ok(q) = recus.recv_timeout(Duration::from_millis(200)) {
                passage
                    .console
                    .extend(nav.ecouter(Duration::from_millis(200))?);
                return Ok(q.corps);
            }
            passage
                .console
                .extend(nav.ecouter(Duration::from_millis(50))?);
            if reste(echeance).is_zero() {
                return Err(format!("aucun rapport en {:?}", o.delai));
            }
        }
    });
    nav.fermer();
    passage.charge.1 = charge();
    passage.rapport = corps.and_then(|c| {
        let valeur: Json =
            serde_json::from_slice(&c).map_err(|e| format!("rapport illisible : {e}"))?;
        match valeur.get("erreur").filter(|e| !e.is_null()) {
            Some(e) => Err(format!("la mesure n'a pas tourné : {e}")),
            None => Ok(valeur),
        }
    });
    passage
}

/// Les modules de la capture : chaque sous-dossier qui a une trace et un module livré.
pub fn modules_captures(donnees: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(donnees)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().join("trace.bin").is_file() && e.path().join("livre.wasm").is_file())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    v.sort();
    v
}

fn ecrire_json(chemin: &Path, v: &Json) {
    if let Some(d) = chemin.parent() {
        let _: std::io::Result<()> = std::fs::create_dir_all(d);
    }
    if let Err(e) = std::fs::write(chemin, v.to_string()) {
        eprintln!("{} : {e}", chemin.display());
    }
}

/// Les pages d'une mesure : son nom (`m1-<module>`, `m10`, `m9`) et son adresse.
fn travaux(m: Mesure, modules: &[String], origine: &str, o: &Options) -> Vec<(String, String)> {
    match m {
        Mesure::M1 => modules
            .iter()
            .map(|module| {
                (
                    format!("m1-{module}"),
                    format!(
                        "{origine}/banc/etalonner.html?quoi=etalonner&module={module}&passes={}{}",
                        o.passes,
                        o.fautif.map_or_else(String::new, |n| format!(
                            "&fautif={}{n}",
                            crate::rejouer::FAUSSER_APRES
                        ))
                    ),
                )
            })
            .collect(),
        Mesure::M10 => vec![(
            String::from("m10"),
            format!(
                "{origine}/banc/etalonner.html?quoi=boucle&fois={}&budget={}",
                o.fois, o.budget
            ),
        )],
        Mesure::M9 => vec![(
            String::from("m9"),
            format!("{origine}/banc/etalonner.html?quoi=profondeur"),
        )],
    }
}

/// Le nom sous lequel s'écrivent les résultats d'un navigateur : `--etiquette`, ou son
/// nom, suivi de `-fautif` sous le moteur fautif (qui n'écrase pas une vraie mesure).
fn etiquette(o: &Options, n: Navigateur) -> String {
    o.etiquette.clone().unwrap_or_else(|| match o.fautif {
        None => String::from(n.nom()),
        Some(_) => format!("{}-fautif", n.nom()),
    })
}

/// Joue les mesures demandées dans chaque navigateur, un à la fois, et écrit leurs
/// résultats bruts. Rend les passages.
pub fn jouer(o: &Options) -> Result<Vec<Passage>, Arret> {
    let soiree = commun();
    let web_ = soiree.join("web");
    let ecarts = web::ecarts(&soiree, &web_);
    if !ecarts.is_empty() {
        return Err(Arret::Echec(format!(
            "la règle « mêmes lints » refuse web/ :\n  {}",
            ecarts.join("\n  ")
        )));
    }
    let modules = if o.modules.is_empty() {
        modules_captures(&o.donnees)
    } else {
        o.modules.clone()
    };
    if o.mesures.contains(&Mesure::M1) && modules.is_empty() {
        return Err(Arret::PasExecute(format!(
            "aucune capture sous {} : lancer d'abord capture-navigateur (mesures/navigateur/README.md)",
            o.donnees.display()
        )));
    }
    let banc = soiree.join("target/banc");
    let paquet = banc.join("paquet");
    if o.construire {
        construire(&web_, &paquet, &["etalonnage-navigateur"]).map_err(Arret::PasExecute)?;
    }
    if !paquet.join("etalonnage_navigateur.js").exists() {
        return Err(Arret::PasExecute(format!(
            "{} manque : construire d'abord (sans --sans-construire)",
            paquet.join("etalonnage_navigateur.js").display()
        )));
    }
    let (envoi, recus) = mpsc::channel();
    let serveur = Serveur::demarrer(
        vec![
            (String::from("/banc/"), soiree.join("crates/banc/pages")),
            (String::from("/paquet/"), paquet),
            (String::from("/donnees/"), o.donnees.clone()),
        ],
        envoi,
    )
    .map_err(|e| Arret::PasExecute(format!("serveur : {e}")))?;
    let origine = serveur.origine();
    let profils = banc.join("profils");
    let mut passages = Vec::new();
    let tours = o.tours.max(1);
    // Les tours de M1 s'enchaînent d'un navigateur à l'autre, comme au harnais : la
    // charge de la machine tombe ainsi sur chacun. M10 et M9 ne jouent qu'au premier.
    for tour in 1..=tours {
        for n in &o.navigateurs {
            let etiquette = etiquette(o, *n);
            let sortie = o.donnees.join("etalonnage").join(&etiquette);
            for m in &o.mesures {
                if tour > 1 && *m != Mesure::M1 {
                    continue;
                }
                let dossier = if tours > 1 && *m == Mesure::M1 {
                    sortie.join(format!("tour-{tour}"))
                } else {
                    sortie.clone()
                };
                for (quoi, url) in travaux(*m, &modules, &origine, o) {
                    let nom_du_tour = if tours > 1 && *m == Mesure::M1 {
                        format!(", tour {tour}")
                    } else {
                        String::new()
                    };
                    println!("{} {quoi}{nom_du_tour} : {}", n.nom(), charge());
                    let p = passer(*n, &url, (&quoi, tour), o, &profils, &recus);
                    let brut = match &p.rapport {
                        Ok(v) => v.clone(),
                        Err(e) => json!({ "non_mesure": e }),
                    };
                    ecrire_json(
                        &dossier.join(format!("{quoi}.json")),
                        &json!({
                            "navigateur": n.nom(),
                            "version": p.version,
                            "tour": tour,
                            "charge_avant": p.charge.0,
                            "charge_apres": p.charge.1,
                            "console": p.console,
                            "rapport": brut,
                        }),
                    );
                    println!(
                        "  {} ({}) : {}",
                        n.nom(),
                        p.version,
                        match &p.rapport {
                            Ok(_) => String::from("reçu"),
                            Err(e) => e.clone(),
                        }
                    );
                    passages.push(p);
                }
            }
        }
    }
    serveur.arreter();
    Ok(passages)
}

// L'analyse.

/// Médiane, 99ᵉ centile et maximum d'une suite, au rang le plus proche (comme
/// `centile` d'`etalonner` et l'analyse du harnais) ; `n` valeurs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stats {
    /// Le nombre de valeurs.
    pub n: usize,
    /// La médiane (le rang ⌈n/2⌉).
    pub mediane: f64,
    /// Le 99ᵉ centile (le rang ⌈0,99 n⌉).
    pub p99: f64,
    /// Le maximum.
    pub max: f64,
    /// Le minimum.
    pub min: f64,
}

/// Les statistiques de `v` ; `None` pour une suite vide.
pub fn stats(v: &[f64]) -> Option<Stats> {
    if v.is_empty() {
        return None;
    }
    let mut t = v.to_vec();
    t.sort_by(f64::total_cmp);
    let n = t.len();
    let rang = |num: usize| {
        n.saturating_mul(num)
            .div_ceil(100)
            .clamp(1, n)
            .saturating_sub(1)
    };
    Some(Stats {
        n,
        mediane: *t.get(rang(50))?,
        p99: *t.get(rang(99))?,
        max: *t.last()?,
        min: *t.first()?,
    })
}

/// Une suite de nombres d'un rapport.
fn suite(v: &Json, cle: &str) -> Vec<f64> {
    v.get(cle)
        .and_then(Json::as_array)
        .map(|a| a.iter().filter_map(Json::as_f64).collect())
        .unwrap_or_default()
}

fn nombre(v: &Json, cle: &str) -> f64 {
    v.get(cle).and_then(Json::as_f64).unwrap_or(f64::NAN)
}

/// Un entier de 64 bits en flottant : des carburants et des durées en ns, bien sous 2⁵³.
#[expect(
    clippy::cast_precision_loss,
    reason = "des carburants et des durées en nanosecondes, bien sous 2⁵³ : la conversion est exacte"
)]
fn flottant(x: u64) -> f64 {
    x as f64
}

fn st(s: Option<Stats>, chiffres: usize) -> String {
    s.map_or_else(
        || String::from("—"),
        |s| {
            format!(
                "{} ; {:.c$} ; {:.c$} ; {:.c$}",
                s.n,
                s.mediane,
                s.p99,
                s.max,
                c = chiffres
            )
        },
    )
}

/// Ce qu'un module de la capture dit de wasmtime : la trace, les durées du second rejeu.
struct Capture {
    /// Par appel de la trace : section, nom, carburant.
    appels: Vec<(u8, u8, u64)>,
    /// La durée de chaque appel du second rejeu de wasmtime, en µs.
    wasmtime_us: Vec<f64>,
    meta: Json,
}

fn lire_capture(dossier: &Path) -> Result<Capture, String> {
    let trace = std::fs::read(dossier.join("trace.bin"))
        .map_err(|e| format!("{} : {e}", dossier.join("trace.bin").display()))?;
    let appels = moteur_commun::trace::lire(&trace)?
        .into_iter()
        .filter_map(|e| match e {
            moteur_commun::trace::Evenement::Appel {
                section,
                nom,
                carburant,
                ..
            } => Some((section, nom, carburant)),
            moteur_commun::trace::Evenement::Neuve { .. }
            | moteur_commun::trace::Evenement::Instantane { .. }
            | moteur_commun::trace::Evenement::Restaurer { .. }
            | moteur_commun::trace::Evenement::Oublier { .. }
            | moteur_commun::trace::Evenement::Lacher { .. } => None,
        })
        .collect();
    let lire = |f: &str| -> Result<Json, String> {
        let o = std::fs::read(dossier.join(f))
            .map_err(|e| format!("{} : {e}", dossier.join(f).display()))?;
        serde_json::from_slice(&o).map_err(|e| format!("{f} : {e}"))
    };
    let wt = lire("wasmtime.json")?;
    let wasmtime_us = wt
        .get("passes")
        .and_then(Json::as_array)
        .and_then(|p| p.get(1).or_else(|| p.first()))
        .and_then(|p| p.get("durees_ns"))
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Json::as_u64)
                .map(|x| flottant(x) / 1000.0)
                .collect()
        })
        .unwrap_or_default();
    Ok(Capture {
        appels,
        wasmtime_us,
        meta: lire("meta.json")?,
    })
}

/// Le seuil des appels lourds, et celui des appels très lourds, en unités.
const LOURD: u64 = 100_000;
const TRES_LOURD: u64 = 1_000_000;

/// Ce que l'analyse des modules d'un tour dans un navigateur retient pour l'ensemble.
#[derive(Default)]
struct Ensemble {
    facteurs: Vec<f64>,
    ps: Vec<f64>,
    ps_wasmtime: Vec<f64>,
    /// Ce qui contredit wasmtime : un écart de carburant, d'issue ou de retour, une
    /// panne du rejeu, un module réécrit autrement, un nombre d'appels différent. Une
    /// mesure n'est pas un jugement, mais D32 en est un : un seul défaut fait le code 1.
    defauts: Vec<String>,
    /// Sous `--fautif` : ce qui dit que le contrôle négatif n'a pas vu son écart au
    /// `n + 1`-ième appel, ou n'a pas pu être jugé. Une panne de l'outil (le code 2).
    controle_rate: Vec<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "l'analyse d'un module se lit d'un trait, dans l'ordre des tableaux de la mesure 1"
)]
fn analyser_un_module(
    sortie: &mut String,
    (module, tour): (&str, Option<u32>),
    c: &Capture,
    r: &Json,
    ensemble: &mut Ensemble,
    fautif: Option<u64>,
) -> Json {
    let titre = tour.map_or_else(|| String::from(module), |t| format!("{module}, tour {t}"));
    let passes = r
        .get("passes")
        .and_then(Json::as_array)
        .cloned()
        .unwrap_or_default();
    let passe = passes
        .get(1)
        .or_else(|| passes.first())
        .cloned()
        .unwrap_or(Json::Null);
    let nav = suite(&passe, "appels_us");
    let wt = &c.wasmtime_us;
    let _: std::fmt::Result = writeln!(sortie, "\n#### {titre}");
    if nav.len() != c.appels.len() || wt.len() != c.appels.len() {
        ensemble.defauts.push(format!(
            "{titre} : {} appels dans la trace, {} rejoués, {} dans wasmtime",
            c.appels.len(),
            nav.len(),
            wt.len()
        ));
        if let Some(n) = fautif {
            ensemble.controle_rate.push(format!(
                "{titre} : contrôle négatif fausser-apres:{n} non jugé, le nombre d'appels diffère"
            ));
        }
        let _: std::fmt::Result = writeln!(
            sortie,
            "appels : {} dans la trace, {} rejoués dans le navigateur, {} dans wasmtime : rien n'est comparé",
            c.appels.len(),
            nav.len(),
            wt.len()
        );
        return json!({ "erreur": "nombre d'appels différent" });
    }
    let indices = |pred: &dyn Fn(u8, u64) -> bool| -> Vec<usize> {
        c.appels
            .iter()
            .enumerate()
            .filter(|(_, (s, _, carb))| pred(*s, *carb))
            .map(|(k, _)| k)
            .collect()
    };
    let prendre = |v: &[f64], ks: &[usize]| -> Vec<f64> {
        ks.iter().filter_map(|k| v.get(*k).copied()).collect()
    };
    let bonjours = indices(&|s, _| s == moteur_commun::trace::BONJOURS);
    let fixe_nav = stats(&prendre(&nav, &bonjours));
    let fixe_wt = stats(&prendre(wt, &bonjours));
    let f_nav = fixe_nav.map_or(f64::NAN, |s| s.mediane);
    let f_wt = fixe_wt.map_or(f64::NAN, |s| s.mediane);
    let lourds =
        |seuil: u64, sections: &[u8]| indices(&|s, carb| sections.contains(&s) && carb >= seuil);
    let pires_et_soiree = [
        moteur_commun::trace::PIRES_CAS,
        moteur_commun::trace::SOIREE,
    ];
    let par_unite = |v: &[f64], fixe: f64, ks: &[usize]| -> Vec<f64> {
        ks.iter()
            .filter_map(|k| {
                let d = v.get(*k)?;
                let (_, _, carb) = c.appels.get(*k)?;
                Some((d - fixe) * 1e6 / flottant(*carb))
            })
            .collect()
    };
    let tres = lourds(TRES_LOURD, &pires_et_soiree);
    let ps_nav = par_unite(&nav, f_nav, &tres);
    let ps_wt = par_unite(wt, f_wt, &tres);
    let facteur = |ks: &[usize]| -> Vec<f64> {
        ks.iter()
            .filter_map(|k| Some(nav.get(*k)? / wt.get(*k)?))
            .collect()
    };
    let facteurs_tres = facteur(&tres);
    let facteurs_lourds = facteur(&lourds(LOURD, &pires_et_soiree));
    let part: Vec<f64> = tres
        .iter()
        .filter_map(|k| {
            let w = wt.get(*k)? - f_wt;
            let d = nav.get(*k)? - f_nav;
            (w >= 50.0).then_some(d / w)
        })
        .collect();
    let soiree_tous = facteur(&indices(&|s, _| s == moteur_commun::trace::SOIREE));
    ensemble.facteurs.extend(&facteurs_tres);
    ensemble.ps.extend(&ps_nav);
    ensemble.ps_wasmtime.extend(&ps_wt);
    let kio = nombre(r, "octets_remis_a_zero") / 1024.0;
    let bloc = stats(&suite(r, "bloc_avec_ms")).map_or(f64::NAN, |s| s.mediane * 1000.0);
    let zero_memoire = nombre(r, "remise_a_zero_memoire_ms");
    let zero_seul = nombre(r, "remise_a_zero_seul_ms");
    let compiler = suite(r, "compiler_ms");
    let compiler_suivantes = stats(compiler.get(1..).unwrap_or_default());
    let totaux: Vec<f64> = passes
        .iter()
        .map(|p| suite(p, "appels_us").iter().sum::<f64>() / 1000.0)
        .collect();
    let ecarts: Vec<String> = passes
        .iter()
        .map(|p| {
            let e = p.get("ecarts").and_then(Json::as_u64).unwrap_or(u64::MAX);
            let pannes = p.get("pannes").and_then(Json::as_array).map_or(0, Vec::len);
            format!("{e} écarts, {pannes} pannes")
        })
        .collect();
    let sans_defaut = |p: &Json| {
        p.get("ecarts").and_then(Json::as_u64) == Some(0)
            && p.get("pannes").and_then(Json::as_array).map_or(0, Vec::len) == 0
    };
    if passes.is_empty() || !passes.iter().all(sans_defaut) {
        ensemble.defauts.push(format!(
            "{titre} : carburant, issue ou retour différent de wasmtime, ou rejeu en panne ({})",
            if ecarts.is_empty() {
                String::from("aucune passe")
            } else {
                ecarts.join(" / ")
            }
        ));
    }
    let premier = passes
        .iter()
        .find_map(|p| p.get("premier_ecart").and_then(Json::as_str))
        .unwrap_or("aucun");
    if let Some(n) = fautif {
        ensemble
            .controle_rate
            .extend(controle_negatif(&titre, n, &passes, c.appels.len()));
    }
    let empreinte = r
        .get("empreinte_reecrite")
        .and_then(Json::as_str)
        .unwrap_or("?");
    let attendue = c
        .meta
        .get("empreinte_reecrite")
        .and_then(Json::as_str)
        .unwrap_or("?");
    if empreinte != attendue {
        ensemble.defauts.push(format!(
            "{titre} : module réécrit {empreinte}, la capture {attendue}"
        ));
    }
    let w = |s: &mut String, l: String| {
        let _: std::fmt::Result = writeln!(s, "{l}");
    };
    w(
        sortie,
        format!(
            "- module réécrit : {} octets, profondeur permise {}, empreinte {}… ({})",
            r.get("octets_reecrits").and_then(Json::as_u64).unwrap_or(0),
            r.get("profondeur_max").and_then(Json::as_u64).unwrap_or(0),
            empreinte.get(..16).unwrap_or(empreinte),
            if empreinte == attendue {
                "celle de la capture"
            } else {
                "AUTRE que celle de la capture : rien n'est comparable"
            }
        ),
    );
    w(
        sortie,
        format!(
            "- carburant et retour comparés à wasmtime, par passe : {} ; premier écart : {premier}",
            ecarts.join(" / ")
        ),
    );
    if let Some(n) = fautif {
        w(
            sortie,
            format!(
                "- contrôle négatif, moteur fautif fausser-apres:{n} : premier écart attendu à l'appel {}, vu, par passe, à l'appel {}",
                n.saturating_add(1),
                passes
                    .iter()
                    .map(|p| p
                        .get("premier_ecart_appel")
                        .and_then(Json::as_u64)
                        .map_or_else(|| String::from("—"), |a| a.to_string()))
                    .collect::<Vec<_>>()
                    .join(" / ")
            ),
        );
    }
    w(
        sortie,
        format!(
            "- coût fixe, `Bonjour` un par un, µs (n ; médiane ; 99ᵉ ; max) : wasmtime {} ; navigateur {} ; bloc de {} : {bloc:.2} µs par appel, dont la remise à zéro des zones {:.2} µs",
            st(fixe_wt, 1),
            st(fixe_nav, 1),
            r.get("bonjours").and_then(Json::as_u64).unwrap_or(0),
            zero_memoire * 1000.0
        ),
    );
    w(
        sortie,
        format!(
            "- remise à zéro d'un Kio, ns ({kio:.0} Kio) : par la machine, dans la mémoire {:.2} ; `fill(0)` seul {:.2} ; wasmtime (capture) {:.2}",
            zero_memoire * 1e6 / kio,
            zero_seul * 1e6 / kio,
            nombre(&c.meta, "remise_a_zero_ns") / kio
        ),
    );
    w(
        sortie,
        format!(
            "- ps par unité, appels ≥ 10⁶ (pires cas et soirée) : wasmtime {} ; navigateur {}",
            st(stats(&ps_wt), 0),
            st(stats(&ps_nav), 0)
        ),
    );
    for (nom, section) in [
        ("pires cas", moteur_commun::trace::PIRES_CAS),
        ("soirée", moteur_commun::trace::SOIREE),
    ] {
        let ks = lourds(TRES_LOURD, &[section]);
        if !ks.is_empty() {
            w(
                sortie,
                format!(
                    "  - {nom} : wasmtime {} ; navigateur {}",
                    st(stats(&par_unite(wt, f_wt, &ks)), 0),
                    st(stats(&par_unite(&nav, f_nav, &ks)), 0)
                ),
            );
        }
    }
    w(
        sortie,
        format!(
            "- facteur navigateur/wasmtime : ≥ 10⁶ {} ; part du carburant ≥ 10⁶ {} ; ≥ 10⁵ {} ; soirée, tous les appels {}",
            st(stats(&facteurs_tres), 2),
            st(stats(&part), 2),
            st(stats(&facteurs_lourds), 2),
            st(stats(&soiree_tous), 2)
        ),
    );
    w(
        sortie,
        format!(
            "- instance neuve, instantané, restauration, µs : {} ; {} ; {} (instantanés de {} octets au plus)",
            st(stats(&suite(&passe, "neuves_us")), 1),
            st(stats(&suite(&passe, "instantanes_us")), 1),
            st(stats(&suite(&passe, "restaurations_us")), 1),
            stats(&suite(&passe, "instantanes_octets")).map_or(0.0, |s| s.max)
        ),
    );
    w(
        sortie,
        format!(
            "- réécriture, médiane de {} : {:.1} ms ; compilation : la première {:.1} ms, les suivantes (octets uniques) {} ; mise en route (réécrire, compiler, `Bonjour`, `Decrire`) {} ms",
            suite(r, "reecrire_ms").len(),
            stats(&suite(r, "reecrire_ms")).map_or(f64::NAN, |s| s.mediane),
            compiler.first().copied().unwrap_or(f64::NAN),
            st(compiler_suivantes, 1),
            st(stats(&suite(r, "mise_en_route_ms")), 1)
        ),
    );
    w(
        sortie,
        format!(
            "- rejeu entier, somme des appels, ms, par passe : {} (wasmtime, second rejeu : {:.0})",
            totaux
                .iter()
                .map(|t| format!("{t:.0}"))
                .collect::<Vec<_>>()
                .join(" / "),
            wt.iter().sum::<f64>() / 1000.0
        ),
    );
    json!({
        "module": module,
        "tour": tour.unwrap_or(1),
        "empreinte_egale": empreinte == attendue,
        "ecarts": ecarts,
        "fixe_wasmtime_us": fixe_wt.map(|s| [s.mediane, s.p99, s.max]),
        "fixe_navigateur_us": fixe_nav.map(|s| [s.mediane, s.p99, s.max]),
        "bloc_us": bloc,
        "remise_a_zero_ns_par_kio": [zero_memoire * 1e6 / kio, zero_seul * 1e6 / kio],
        "ps_wasmtime": stats(&ps_wt).map(|s| [s.mediane, s.p99, s.max]),
        "ps_navigateur": stats(&ps_nav).map(|s| [s.mediane, s.p99, s.max]),
        "facteur_tres_lourds": stats(&facteurs_tres).map(|s| [s.mediane, s.p99, s.max]),
        "totaux_ms": totaux,
    })
}

fn analyser_m10(sortie: &mut String, r: &Json) -> Json {
    let budget = nombre(r, "budget_de_chargement");
    let temoin = stats(&suite(r, "temoin_ms"));
    let t = temoin.map_or(f64::NAN, |s| s.mediane);
    let ps: Vec<f64> = suite(r, "boucle_ms")
        .iter()
        .map(|b| (b - t) * 1e9 / budget)
        .collect();
    let grand = nombre(r, "grand_budget");
    let petite = stats(&suite(r, "brute_petite_ms")).map_or(f64::NAN, |s| s.mediane);
    let brute: Vec<f64> = suite(r, "brute_ms")
        .iter()
        .map(|b| (b - petite) * 1e9 / grand)
        .collect();
    let s = stats(&ps);
    let sb = stats(&brute);
    let _: std::fmt::Result = writeln!(
        sortie,
        "- chargement du module de `Bonjour` qui boucle (BUDGET_DE_CHARGEMENT, {budget:.0} unités), ps par unité (n ; médiane ; min ; max) : {} ; témoin, médiane {t:.2} ms ; boucle {} ms\n- la même boucle, machine seule, {grand:.0} unités : {} ps par unité",
        s.map_or_else(
            || String::from("—"),
            |s| format!("{} ; {:.0} ; {:.0} ; {:.0}", s.n, s.mediane, s.min, s.max)
        ),
        st(stats(&suite(r, "boucle_ms")), 1),
        sb.map_or_else(
            || String::from("—"),
            |s| format!("{} ; {:.0} ; {:.0} ; {:.0}", s.n, s.mediane, s.min, s.max)
        ),
    );
    json!({
        "ps_chargement": s.map(|s| [s.mediane, s.min, s.max]),
        "ps_grand_budget": sb.map(|s| [s.mediane, s.min, s.max]),
    })
}

/// Les étages tenus par un fil, cadre par cadre : le plus petit de ses essais, et
/// l'issue de chacun.
fn etages_tenus(r: &Json) -> Vec<(u64, f64, f64, Vec<String>)> {
    r.get("cadres")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .map(|c| {
            let essais = c
                .get("essais")
                .and_then(Json::as_array)
                .cloned()
                .unwrap_or_default();
            let etages: Vec<f64> = essais
                .iter()
                .filter_map(|e| e.get("etages")?.as_f64())
                .collect();
            let appels: Vec<f64> = essais
                .iter()
                .filter_map(|e| Some(flottant(e.get("appels")?.as_u64()?)))
                .collect();
            let pieges = essais
                .iter()
                .filter_map(|e| e.get("piege")?.as_str().map(String::from))
                .collect();
            let min = stats(&etages).map_or(f64::NAN, |s| s.min);
            let par_appel = match (etages.first(), appels.first()) {
                (Some(e), Some(a)) if *a > 0.0 => e / a,
                _ => f64::NAN,
            };
            (
                c.get("locaux").and_then(Json::as_u64).unwrap_or(0),
                min,
                par_appel,
                pieges,
            )
        })
        .collect()
}

/// Les défauts des essais sans plafond de M9 : chacun doit s'arrêter sur la pile native
/// (`PileNative`). Un essai qui rend, ou tombe sur un autre piège, ne mesure pas la
/// pile du fil : la profondeur tenue qu'on en tirerait est fausse, et `PHASE_3` se
/// réglerait sur elle (lot 3c, vérification de la correction). Aucun cadre, ou un cadre
/// sans essai, est aussi un défaut.
fn defauts_des_cadres(cadres: &[(u64, f64, f64, Vec<String>)]) -> Vec<String> {
    let mut defauts: Vec<String> = cadres
        .iter()
        .filter_map(|(locaux, _, _, pieges)| {
            let autres: Vec<&str> = pieges
                .iter()
                .map(String::as_str)
                .filter(|p| *p != "PileNative")
                .collect();
            if pieges.is_empty() {
                Some(format!("{locaux} valeurs par étage : aucun essai"))
            } else if autres.is_empty() {
                None
            } else {
                Some(format!(
                    "{locaux} valeurs par étage, sans plafond : {}, et non la pile native",
                    autres.join(", ")
                ))
            }
        })
        .collect();
    if cadres.is_empty() {
        defauts.push(String::from("aucun cadre essayé sans plafond"));
    }
    defauts
}

/// Le contrôle négatif de M1 : sous le moteur fautif `fausser-apres:<n>`, chaque passe
/// doit compter un écart, et le premier au `n + 1`-ième appel de la trace. L'écart
/// lui-même est déjà un défaut (il fait le code 1, ce que le contrôle vérifie) ; ici,
/// ce qui dirait que le contrôle n'a pas vu ce qu'il devait voir : une panne de
/// l'outil, qui fait le code 2 et non le 1 que le contrôle réussi rend.
fn controle_negatif(titre: &str, n: u64, passes: &[Json], appels: usize) -> Vec<String> {
    let attendu = n.saturating_add(1);
    if u64::try_from(appels).unwrap_or(u64::MAX) < attendu {
        return vec![format!(
            "{titre} : contrôle négatif fausser-apres:{n} sans objet, la trace n'a que {appels} appels"
        )];
    }
    if passes.is_empty() {
        return vec![format!(
            "{titre} : contrôle négatif fausser-apres:{n} : aucune passe"
        )];
    }
    passes
        .iter()
        .enumerate()
        .filter_map(|(k, p)| {
            let ecarts = p.get("ecarts").and_then(Json::as_u64).unwrap_or(0);
            let premier = p.get("premier_ecart_appel").and_then(Json::as_u64);
            (ecarts == 0 || premier != Some(attendu)).then(|| {
                format!(
                    "{titre} : le contrôle négatif fausser-apres:{n} n'a pas vu son écart au {attendu}ᵉ appel (passe {} : {ecarts} écarts, le premier à l'appel {})",
                    k.saturating_add(1),
                    premier.map_or_else(|| String::from("—"), |a| a.to_string())
                )
            })
        })
        .collect()
}

/// Le code de sortie de `soiree-banc etalonner`, d'après le résumé d'[`analyser`] et
/// les passages : 2 d'abord si le contrôle négatif de M1 (`--fautif`) n'a pas vu son
/// écart au bon appel, ou n'a pas pu être jugé (`controle_rate` : une panne de
/// l'outil, comme sous `rejouer --fautif`, que le code 1 d'un contrôle réussi ne doit
/// jamais cacher) ; puis 1 dès un défaut (un écart avec wasmtime, D32 ; une récursion
/// de M9 sous `PHASE_3` qui n'est pas la faute `Profondeur`, ou sans plafond que la
/// pile native n'arrête pas, § 8.7), qui l'emporte sur « pas exécuté », comme un échec
/// de `rejouer` ; sinon 0 si tout a été mesuré, 2 si un passage ne l'a pas été.
pub fn code_de_sortie(resume: &Json, passages: &[Passage]) -> u8 {
    let compte = |cle: &str| resume.get(cle).and_then(Json::as_array).map_or(0, Vec::len);
    if compte("controle_rate") > 0 {
        2
    } else if compte("defauts") > 0 {
        1
    } else if passages.iter().all(|p| p.rapport.is_ok()) {
        0
    } else {
        2
    }
}

/// Les récursions de M9 réécrites sous `reecriture::Reglages::PHASE_3` et chargées par
/// `moteur_navigateur::charger` sur le même fil : chacune doit être la faute
/// `Profondeur` du jeu, jamais la pile native (une panne de l'outil, § 8.7). Rend le
/// texte, et un défaut par récursion qui ne l'est pas.
fn sous_phase_3(x: &Json) -> (String, Vec<String>) {
    let Some(essais) = x.get("sous_phase_3").and_then(Json::as_array) else {
        return (
            String::from("non essayé"),
            vec![String::from(
                "les récursions sous PHASE_3 n'ont pas été essayées",
            )],
        );
    };
    let mut defauts = Vec::new();
    let texte = essais
        .iter()
        .map(|e| {
            let locaux = e.get("locaux").and_then(Json::as_u64).unwrap_or(0);
            let issue = e.get("issue").and_then(Json::as_str).unwrap_or("?");
            if issue != "Profondeur" {
                defauts.push(format!(
                    "{locaux} valeurs par étage sous PHASE_3 : {issue}, et non la faute Profondeur"
                ));
            }
            format!("{locaux} : {issue}")
        })
        .collect::<Vec<_>>()
        .join(" ; ");
    if essais.is_empty() {
        defauts.push(String::from("aucune récursion essayée sous PHASE_3"));
    }
    (texte, defauts)
}

fn analyser_m9(sortie: &mut String, r: &Json, defauts: &mut Vec<String>) -> Json {
    let mut resume = serde_json::Map::new();
    for fil in ["worker", "page"] {
        let Some(x) = r.get(fil) else {
            defauts.push(format!("M9, {fil} : aucun rapport"));
            continue;
        };
        if let Some(e) = x.get("erreur") {
            let _: std::fmt::Result = writeln!(sortie, "- {fil} : la mesure n'a pas tourné : {e}");
            defauts.push(format!("M9, {fil} : la mesure n'a pas tourné"));
            continue;
        }
        let (texte_phase_3, defauts_phase_3) = sous_phase_3(x);
        let _: std::fmt::Result = writeln!(
            sortie,
            "- {fil}, chargées sous PHASE_3 ({} étages permis), issue par valeurs par étage : {texte_phase_3}",
            x.get("profondeur_max_phase_3")
                .and_then(Json::as_u64)
                .map_or_else(|| String::from("?"), |p| p.to_string())
        );
        defauts.extend(
            defauts_phase_3
                .into_iter()
                .map(|d| format!("M9, {fil} : {d}")),
        );
        let cadres = etages_tenus(x);
        defauts.extend(
            defauts_des_cadres(&cadres)
                .into_iter()
                .map(|d| format!("M9, {fil} : {d}")),
        );
        let tenue = cadres
            .iter()
            .map(|(_, m, _, _)| *m)
            .fold(f64::INFINITY, |a, b| if b < a { b } else { a });
        let toutes_natives = cadres
            .iter()
            .all(|(_, _, _, p)| p.iter().all(|x| x == "PileNative"));
        let _: std::fmt::Result = writeln!(
            sortie,
            "- {fil} : {} ; tenue (le plus petit) : {tenue:.0} étages{}",
            cadres
                .iter()
                .map(|(l, m, e, _)| format!("{l} valeurs : {m:.0} étages ({e:.0} par appel)"))
                .collect::<Vec<_>>()
                .join(" ; "),
            if toutes_natives {
                ""
            } else {
                " ; DÉFAUT : un essai ne s'est pas arrêté sur la pile native"
            }
        );
        let _: Option<Json> = resume.insert(
            String::from(fil),
            json!({ "tenue": tenue, "toutes_natives": toutes_natives,
                    "sous_phase_3": x.get("sous_phase_3").cloned().unwrap_or(Json::Null),
                    "cadres": cadres.iter().map(|(l, m, e, p)| json!([l, m, e, p])).collect::<Vec<_>>() }),
        );
    }
    Json::Object(resume)
}

/// Les statistiques des modules ensemble d'un tour (ou de tous) : médiane, 99ᵉ
/// centile et maximum du facteur, des ps par unité du navigateur et de wasmtime.
fn ensemble_json(e: &Ensemble) -> Json {
    json!({
        "facteur": stats(&e.facteurs).map(|s| [s.mediane, s.p99, s.max]),
        "ps": stats(&e.ps).map(|s| [s.mediane, s.p99, s.max]),
        "ps_wasmtime": stats(&e.ps_wasmtime).map(|s| [s.mediane, s.p99, s.max]),
    })
}

/// « de a à b » sur les tours, ou la seule valeur s'ils la partagent.
fn etendue(v: &[f64], chiffres: usize) -> String {
    let mut t = v.to_vec();
    t.sort_by(f64::total_cmp);
    let (Some(min), Some(max)) = (t.first(), t.last()) else {
        return String::from("—");
    };
    if format!("{min:.chiffres$}") == format!("{max:.chiffres$}") {
        format!("{min:.chiffres$}")
    } else {
        format!("{min:.chiffres$} à {max:.chiffres$}")
    }
}

/// Les tours d'un navigateur résumés comme au harnais : pour le facteur et les ps par
/// unité, l'étendue sur les tours de la médiane, du 99ᵉ centile et du maximum.
fn resumer_les_tours(sortie: &mut String, tours: &std::collections::BTreeMap<u32, Ensemble>) {
    let par_tour = |f: &dyn Fn(&Ensemble) -> Option<Stats>| -> [Vec<f64>; 3] {
        let s: Vec<Stats> = tours.values().filter_map(f).collect();
        [
            s.iter().map(|x| x.mediane).collect(),
            s.iter().map(|x| x.p99).collect(),
            s.iter().map(|x| x.max).collect(),
        ]
    };
    for (nom, f, chiffres) in [
        (
            "facteur navigateur/wasmtime",
            &(|e: &Ensemble| stats(&e.facteurs)) as &dyn Fn(&Ensemble) -> Option<Stats>,
            2,
        ),
        ("ps par unité, navigateur", &|e: &Ensemble| stats(&e.ps), 0),
        (
            "ps par unité, wasmtime",
            &|e: &Ensemble| stats(&e.ps_wasmtime),
            0,
        ),
    ] {
        let [m, p, x] = par_tour(f);
        let _: std::fmt::Result = writeln!(
            sortie,
            "- {nom}, sur les {} tours : médiane {} ; 99ᵉ {} ; max {}",
            tours.len(),
            etendue(&m, chiffres),
            etendue(&p, chiffres),
            etendue(&x, chiffres)
        );
    }
}

/// Analyse les passages : les chiffres de M1 (par module et par tour, puis les modules
/// ensemble, tour par tour et sur tous les tours), M10 et M9, par navigateur. Rend le
/// texte et le résumé JSON ; `defauts`, dans le résumé, dit ce qui contredit wasmtime
/// ou `PHASE_3` (le code 1 de `soiree-banc etalonner`).
#[expect(
    clippy::too_many_lines,
    reason = "l'analyse d'un navigateur se lit d'un trait, dans l'ordre de sa sortie"
)]
pub fn analyser(o: &Options, passages: &[Passage]) -> (String, Json) {
    let mut sortie = String::new();
    let mut resume = Vec::new();
    let mut tous_les_defauts = Vec::new();
    let mut tous_les_rates = Vec::new();
    let plusieurs_tours = o.tours > 1;
    for n in &o.navigateurs {
        let les_siens: Vec<&Passage> = passages.iter().filter(|p| p.navigateur == *n).collect();
        let Some(premier) = les_siens.first() else {
            continue;
        };
        let agent = les_siens
            .iter()
            .find_map(|p| p.rapport.as_ref().ok()?.get("agent")?.as_str())
            .unwrap_or("?");
        let _: std::fmt::Result =
            writeln!(sortie, "\n### {} ({}), {}", n.nom(), premier.version, agent);
        let mut tours: std::collections::BTreeMap<u32, Ensemble> =
            std::collections::BTreeMap::new();
        let mut defauts = Vec::new();
        let mut rates = Vec::new();
        let mut modules = Vec::new();
        let mut m10 = Json::Null;
        let mut m9 = Json::Null;
        for p in &les_siens {
            let _: std::fmt::Result = writeln!(
                sortie,
                "\n{}{} — charge avant : {} ; après : {}",
                p.quoi,
                if plusieurs_tours && p.quoi.starts_with("m1-") {
                    format!(", tour {}", p.tour)
                } else {
                    String::new()
                },
                p.charge.0,
                p.charge.1
            );
            let r = match &p.rapport {
                Ok(r) => r,
                Err(e) => {
                    let _: std::fmt::Result = writeln!(sortie, "- non mesuré : {e}");
                    continue;
                }
            };
            let _: std::fmt::Result = writeln!(
                sortie,
                "- isolée (COOP, COEP) : {}{}",
                r.get("isole").and_then(Json::as_bool).unwrap_or(false),
                r.get("granularite_ms").and_then(Json::as_f64).map_or_else(
                    String::new,
                    |g| format!(" ; granularité de performance.now() {:.1} µs", g * 1000.0)
                )
            );
            if let Some(module) = p.quoi.strip_prefix("m1-") {
                let ensemble = tours.entry(p.tour).or_default();
                match lire_capture(&o.donnees.join(module)) {
                    Ok(c) => {
                        modules.push(analyser_un_module(
                            &mut sortie,
                            (module, plusieurs_tours.then_some(p.tour)),
                            &c,
                            r,
                            ensemble,
                            o.fautif,
                        ));
                    }
                    Err(e) => {
                        let _: std::fmt::Result = writeln!(sortie, "- capture illisible : {e}");
                        ensemble
                            .defauts
                            .push(format!("{module} : capture illisible : {e}"));
                        if let Some(n) = o.fautif {
                            ensemble.controle_rate.push(format!(
                                "{module} : contrôle négatif fausser-apres:{n} non jugé, capture illisible"
                            ));
                        }
                    }
                }
            } else if p.quoi == "m10" {
                m10 = analyser_m10(&mut sortie, r);
            } else if p.quoi == "m9" {
                m9 = analyser_m9(&mut sortie, r, &mut defauts);
            }
        }
        let mut cumul = Ensemble::default();
        for (t, e) in &tours {
            if e.facteurs.is_empty() {
                continue;
            }
            let _: std::fmt::Result = writeln!(
                sortie,
                "\n#### Les modules ensemble{}, appels ≥ 10⁶ (n ; médiane ; 99ᵉ ; max)\n- facteur navigateur/wasmtime : {}\n- ps par unité : navigateur {} ; wasmtime {}",
                if plusieurs_tours {
                    format!(", tour {t}")
                } else {
                    String::new()
                },
                st(stats(&e.facteurs), 2),
                st(stats(&e.ps), 0),
                st(stats(&e.ps_wasmtime), 0)
            );
            cumul.facteurs.extend(&e.facteurs);
            cumul.ps.extend(&e.ps);
            cumul.ps_wasmtime.extend(&e.ps_wasmtime);
        }
        if plusieurs_tours && !cumul.facteurs.is_empty() {
            let _: std::fmt::Result = writeln!(
                sortie,
                "\n#### Les tours ensemble\n- tous les appels ≥ 10⁶ de tous les tours : facteur {} ; ps par unité, navigateur {}",
                st(stats(&cumul.facteurs), 2),
                st(stats(&cumul.ps), 0)
            );
            resumer_les_tours(&mut sortie, &tours);
        }
        for e in tours.values_mut() {
            defauts.append(&mut e.defauts);
            rates.append(&mut e.controle_rate);
        }
        if !rates.is_empty() {
            let _: std::fmt::Result = writeln!(
                sortie,
                "\n#### CONTRÔLE NÉGATIF RATÉ ({}) : panne de l'outil, le code 2\n- {}",
                rates.len(),
                rates.join("\n- ")
            );
        }
        if !defauts.is_empty() {
            let _: std::fmt::Result = writeln!(
                sortie,
                "\n#### DÉFAUTS ({})\n- {}",
                defauts.len(),
                defauts.join("\n- ")
            );
        }
        tous_les_defauts.extend(defauts.iter().map(|d| format!("{} : {d}", n.nom())));
        tous_les_rates.extend(rates.iter().map(|d| format!("{} : {d}", n.nom())));
        resume.push(json!({
            "navigateur": n.nom(),
            "version": premier.version,
            "agent": agent,
            "modules": modules,
            "ensemble": ensemble_json(&cumul),
            "tours": tours.iter().map(|(t, e)| {
                let mut j = ensemble_json(e);
                if let Some(o) = j.as_object_mut() {
                    let _: Option<Json> = o.insert(String::from("tour"), json!(t));
                }
                j
            }).collect::<Vec<_>>(),
            "defauts": defauts,
            "controle_rate": rates,
            "m10": m10,
            "m9": m9,
        }));
    }
    if let Some(n) = o.fautif
        && !resume
            .iter()
            .any(|j| j["modules"].as_array().is_some_and(|m| !m.is_empty()))
        && tous_les_rates.is_empty()
        && passages.iter().any(|p| p.rapport.is_ok())
    {
        tous_les_rates.push(format!(
            "contrôle négatif fausser-apres:{n} : aucun module de M1 rejoué"
        ));
    }
    let resume = json!({
        "navigateurs": resume,
        "defauts": tous_les_defauts,
        "controle_rate": tous_les_rates,
    });
    ecrire_json(&o.donnees.join("etalonnage").join("analyse.json"), &resume);
    (sortie, resume)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use std::path::{Path, PathBuf};

    use jeu_abi::SorteAppel;
    use moteur_commun::trace::{self, Evenement};
    use serde_json::Value as Json;

    use super::{
        Mesure, Options, Passage, analyser, code_de_sortie, defauts_des_cadres, etages_tenus,
        etendue, sous_phase_3, stats,
    };
    use crate::navigateur::Navigateur;

    /// Une capture d'essai de `n` `Bonjour` sur une instance, sous `dossier/devine`,
    /// telle que `capture-navigateur` l'écrit : la trace, le rejeu de wasmtime, `meta`.
    fn capture_d_essai(dossier: &Path, n: u32) -> PathBuf {
        let module = dossier.join("devine");
        std::fs::create_dir_all(&module).unwrap();
        let mut evenements = vec![Evenement::Neuve { inst: 0 }];
        for _ in 0..n {
            evenements.push(Evenement::Appel {
                inst: 0,
                section: trace::BONJOURS,
                nom: 0,
                sorte: SorteAppel::Lecture,
                budget: 1 << 20,
                octets: vec![0],
                carburant: 3000,
                issue: trace::ISSUE_RENDU,
                sortie_longueur: 4,
                sortie_fnv: 7,
            });
        }
        std::fs::write(module.join("trace.bin"), trace::ecrire(&evenements)).unwrap();
        let durees: Vec<u64> = (0..n).map(|_| 20_000).collect();
        let wasmtime = json!({ "passes": [{ "durees_ns": durees }, { "durees_ns": durees }] });
        std::fs::write(module.join("wasmtime.json"), wasmtime.to_string()).unwrap();
        let meta = json!({ "empreinte_reecrite": "ab12", "remise_a_zero_ns": 1000 });
        std::fs::write(module.join("meta.json"), meta.to_string()).unwrap();
        module
    }

    /// Ce que la page rapporte de M1 sur `n` appels : deux passes, avec `ecarts`
    /// écarts, le premier au `premier`-ième appel.
    fn rapport_m1(n: u32, ecarts: u64, premier: Option<u64>) -> Json {
        let appels: Vec<f64> = (0..n).map(|_| 20.0).collect();
        let passe = json!({ "appels_us": appels, "ecarts": ecarts, "pannes": [],
                            "premier_ecart_appel": premier });
        json!({ "empreinte_reecrite": "ab12", "octets_remis_a_zero": 2048,
                "passes": [passe, passe] })
    }

    fn passage(rapport: Json) -> Passage {
        Passage {
            navigateur: Navigateur::Chrome,
            version: String::from("essai"),
            quoi: String::from("m1-devine"),
            tour: 1,
            rapport: Ok(rapport),
            charge: (String::new(), String::new()),
            console: Vec::new(),
        }
    }

    fn liste(resume: &Json, cle: &str) -> Vec<String> {
        resume[cle]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d.as_str().unwrap().to_owned())
            .collect()
    }

    fn defauts(resume: &Json) -> Vec<String> {
        liste(resume, "defauts")
    }

    /// Un écart avec wasmtime en M1 fait le code 1 de `soiree-banc etalonner`, de la
    /// page au code de sortie ; le contrôle négatif (`--fautif fausser-apres:<n>`) doit
    /// voir son écart au `n + 1`-ième appel, et un contrôle qui ne voit rien, ou trop
    /// tôt, se dit (lot 3c, vérification de la correction : rien ne prouvait qu'un
    /// écart de M1 compte). Il fait le code 2, panne de l'outil, et non le 1 d'un
    /// contrôle réussi (lot 3c, seconde vérification de la fin : sous `--fautif`, un
    /// contrôle qui ne voyait rien rendait 1 comme un contrôle qui voit son écart).
    #[test]
    fn un_ecart_avec_wasmtime_fait_le_code_1() {
        let dossier = std::env::temp_dir().join(format!("banc-etalonner-{}", std::process::id()));
        let _: PathBuf = capture_d_essai(&dossier, 5);
        let mut o = Options {
            navigateurs: vec![Navigateur::Chrome],
            mesures: vec![Mesure::M1],
            donnees: dossier.clone(),
            modules: vec![String::from("devine")],
            ..Options::default()
        };
        let juger = |o: &Options, r: Json| {
            let p = [passage(r)];
            let (_, resume) = analyser(o, &p);
            (
                code_de_sortie(&resume, &p),
                defauts(&resume),
                liste(&resume, "controle_rate"),
            )
        };
        // Sans écart : code 0.
        assert_eq!(
            juger(&o, rapport_m1(5, 0, None)),
            (0, Vec::new(), Vec::new())
        );
        // Un écart : code 1, quel que soit le moteur.
        let (code, d, rate) = juger(&o, rapport_m1(5, 1, Some(4)));
        assert_eq!((code, d.len(), rate.len()), (1, 1, 0), "{d:?} {rate:?}");
        // Le contrôle négatif qui voit son écart au bon appel : code 1, un seul
        // défaut, l'écart, et aucun contrôle raté.
        o.fautif = Some(3);
        let (code, d, rate) = juger(&o, rapport_m1(5, 2, Some(4)));
        assert_eq!((code, d.len()), (1, 1), "{d:?}");
        assert!(d[0].contains("différent de wasmtime"), "{d:?}");
        assert!(rate.is_empty(), "{rate:?}");
        // Le contrôle négatif qui ne voit rien : code 2, une ligne par passe.
        let (code, d, rate) = juger(&o, rapport_m1(5, 0, None));
        assert_eq!(code, 2, "{d:?} {rate:?}");
        assert!(d.is_empty(), "{d:?}");
        assert!(
            rate.iter()
                .all(|x| x.contains("n'a pas vu son écart au 4ᵉ appel")),
            "{rate:?}"
        );
        assert_eq!(rate.len(), 2, "une ligne par passe : {rate:?}");
        // Trop tôt : l'écart est un défaut, mais le contrôle raté l'emporte, code 2.
        let (code, d, rate) = juger(&o, rapport_m1(5, 3, Some(2)));
        assert_eq!((code, d.len(), rate.len()), (2, 1, 2), "{d:?} {rate:?}");
        // Une trace trop courte pour le moteur fautif : code 2.
        o.fautif = Some(5);
        let (code, _, rate) = juger(&o, rapport_m1(5, 0, None));
        assert_eq!(code, 2);
        assert!(rate[0].contains("sans objet"), "{rate:?}");
        // Un nombre d'appels différent : le contrôle n'est pas jugé, code 2.
        o.fautif = Some(3);
        let (code, d, rate) = juger(&o, rapport_m1(4, 2, Some(4)));
        assert_eq!((code, d.len(), rate.len()), (2, 1, 1), "{d:?} {rate:?}");
        assert!(rate[0].contains("non jugé"), "{rate:?}");
        // Sous --fautif, aucun module de M1 rejoué : code 2.
        let m9 = Passage {
            quoi: String::from("m9"),
            ..passage(json!({}))
        };
        let (_, resume) = analyser(&o, std::slice::from_ref(&m9));
        assert_eq!(code_de_sortie(&resume, &[m9]), 2);
        assert!(liste(&resume, "controle_rate")[0].contains("aucun module"));
        // Un passage non mesuré, sans défaut : code 2.
        o.fautif = None;
        let p = [Passage {
            rapport: Err(String::from("pas exécuté : essai")),
            ..passage(Json::Null)
        }];
        let (_, resume) = analyser(&o, &p);
        assert_eq!(code_de_sortie(&resume, &p), 2);
        let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    }

    /// En M9, un essai sans plafond qui ne s'arrête pas sur la pile native est un
    /// défaut, pas un avertissement : la profondeur tenue qu'on en tirerait ne mesure
    /// pas la pile du fil (lot 3c, vérification de la correction).
    #[test]
    fn m9_un_essai_hors_de_la_pile_native_est_un_defaut() {
        let r = json!({ "cadres": [
            { "locaux": 0, "essais": [
                { "piege": "PileNative", "etages": 9000, "appels": 9000 } ] },
            { "locaux": 250, "essais": [
                { "piege": "PileNative", "etages": 250, "appels": 250 },
                { "piege": "rendu 3", "etages": 0, "appels": 1 } ] },
            { "locaux": 500, "essais": [] } ] });
        let d = defauts_des_cadres(&etages_tenus(&r));
        assert_eq!(d.len(), 2, "{d:?}");
        assert!(
            d[0].starts_with("250 valeurs") && d[0].contains("rendu 3"),
            "{d:?}"
        );
        assert!(
            d[1].starts_with("500 valeurs") && d[1].contains("aucun essai"),
            "{d:?}"
        );
        assert_eq!(defauts_des_cadres(&[]), ["aucun cadre essayé sans plafond"]);
        let bon = json!({ "cadres": [
            { "locaux": 0, "essais": [{ "piege": "PileNative", "etages": 1, "appels": 1 }] } ] });
        assert_eq!(
            defauts_des_cadres(&etages_tenus(&bon)),
            Vec::<String>::new()
        );
    }

    /// Le rang le plus proche, comme l'analyse du harnais : la médiane de quatre valeurs
    /// est la deuxième, le 99ᵉ centile de cent la quatre-vingt-dix-neuvième.
    #[test]
    fn les_statistiques_au_rang_le_plus_proche() {
        let s = stats(&[4.0, 1.0, 3.0, 2.0]).unwrap();
        assert!((s.mediane - 2.0).abs() < 1e-12 && (s.max - 4.0).abs() < 1e-12);
        assert!((s.min - 1.0).abs() < 1e-12 && s.n == 4);
        let cent: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = stats(&cent).unwrap();
        assert!((s.p99 - 99.0).abs() < 1e-12 && (s.mediane - 50.0).abs() < 1e-12);
        assert!(stats(&[]).is_none());
    }

    #[test]
    fn les_noms_des_mesures() {
        assert_eq!(Mesure::liste("m1,m9").unwrap(), [Mesure::M1, Mesure::M9]);
        assert!(Mesure::liste("m2").is_err());
    }

    /// La profondeur tenue est le plus petit des essais, et un essai qui ne s'arrête pas
    /// sur la pile native se voit.
    #[test]
    fn la_profondeur_tenue_est_le_plus_petit_essai() {
        let r = json!({ "cadres": [
            { "locaux": 0, "essais": [
                { "piege": "PileNative", "etages": 9000, "appels": 9000 },
                { "piege": "PileNative", "etages": 8000, "appels": 8000 } ] },
            { "locaux": 500, "essais": [
                { "piege": "Autre", "etages": 600, "appels": 300 } ] } ] });
        let c = etages_tenus(&r);
        assert_eq!(c.len(), 2);
        assert!((c[0].1 - 8000.0).abs() < 1e-9);
        assert!((c[1].2 - 2.0).abs() < 1e-9);
        assert_eq!(c[1].3, ["Autre"]);
    }

    /// Une récursion sous `PHASE_3` qui n'est pas la faute `Profondeur`, ou des
    /// récursions jamais essayées, sont des défauts : `PHASE_3` ne tiendrait plus sous la
    /// pile du fil (lot 3c, correction après la vérification, constat 4).
    #[test]
    fn sous_phase_3_tout_ce_qui_n_est_pas_profondeur_est_un_defaut() {
        let bon = json!({ "sous_phase_3": [
            { "locaux": 0, "issue": "Profondeur" }, { "locaux": 250, "issue": "Profondeur" } ] });
        assert_eq!(sous_phase_3(&bon).1, Vec::<String>::new());
        let pile = json!({ "sous_phase_3": [
            { "locaux": 0, "issue": "Profondeur" },
            { "locaux": 250, "issue": "panne de l'outil : pile native épuisée" } ] });
        assert_eq!(sous_phase_3(&pile).1.len(), 1);
        assert_eq!(sous_phase_3(&json!({})).1.len(), 1);
        assert_eq!(sous_phase_3(&json!({ "sous_phase_3": [] })).1.len(), 1);
    }

    #[test]
    fn l_etendue_sur_les_tours() {
        assert_eq!(etendue(&[36.2, 34.4, 35.9], 0), "34 à 36");
        assert_eq!(etendue(&[1.001, 1.004], 2), "1.00");
        assert_eq!(etendue(&[], 0), "—");
    }
}
