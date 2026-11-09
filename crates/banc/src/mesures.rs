//! `soiree-banc mesures` (PHASE-3, § 8.4 ; M2 et M14 ; lot 13) : **un rapport, pas une
//! porte**. La télé dans un navigateur, la vraie autorité dans son Worker, de vrais
//! gestes :
//!
//! - **M2, de l'entrée au pixel** : des touches par la chaîne du navigateur
//!   (`Input.dispatchKeyEvent`, `input.performActions`), des clics sur les boutons de la
//!   scène (la souris), et les gestes des téléphones natifs (les coups des bots, par leur
//!   page) ; chaque entrée revient en trace (`{"quoi":"trace",…}`, `web/ecran`,
//!   `page/mesure.rs`), découpée par segment ([`salle::trace`]). Quatre conditions dans le
//!   même passage : avec et sans calage (`window.__banc.calage`, P6), avec et sans glisser
//!   (`window.__banc.glisser`, § 2.9 ; la page ne glisse que si un nœud se déplace, et le
//!   rapport dit, par condition, combien de scènes ont vraiment glissé,
//!   `window.__banc.glisses()`). Le rapport donne, par condition, par source et par
//!   segment, la médiane, le 99ᵉ centile et le maximum, jamais une moyenne ; il nomme le
//!   segment qui pèse le plus au 99ᵉ centile quand le budget du CONTRAT (50 ms au clavier,
//!   80 ms au téléphone) n'est pas tenu.
//! - **M14, l'écart des horloges** entre la page et son Worker (`window.__banc.echo`, la
//!   commande `Echo`, au banc seulement), entre la page et un autre onglet de la même
//!   origine (`BroadcastChannel`), et entre la page et le processus du banc (une
//!   évaluation de `performance.timeOrigin + performance.now()`, que le banc encadre de
//!   son heure murale, `horloge::heure_us`) : pour chaque écho (départ, autre, retour),
//!   l'écart est `autre − (départ + retour) / 2`, à un demi-aller-retour près ; on garde
//!   les échos du dixième le plus court, et l'on donne leur écart médian et le plus court
//!   demi-aller-retour. M14 se mesure avant les conditions de M2 (avant tout découpage).
//!
//! Les instants de la page et ceux du Worker sont chacun dans sa base `timeOrigin + now()`
//! (§ 8.4) : un segment qui franchit les deux porte leur écart, que M14 mesure. Les
//! instants d'un téléphone natif sont l'heure de l'autorité qu'il estime par écho, à un
//! demi-aller-retour près (`incertitude` de sa trace).
//!
//! Le banc ne juge rien de ces chiffres (« un rapport, pas une porte ») : il échoue
//! seulement s'il n'a pas pu mesurer (aucune trace, aucun écho), et le dit.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jeu_abi::{Appareil, Pas};
use salle::ecran::{IdTrace, Trace};
use salle::trace::{Distribution, SEGMENTS, Segment, distribution, duree};
use serde_json::Value as Json;

use crate::horloge::heure_us;
use crate::navigateur::Navigateur;
use crate::soiree::{self, Ligne, Session};

/// Ce que `mesures` joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Le module du jeu.
    pub module: PathBuf,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Les entrées de l'écran par condition (touches et clics) ; 0 : aucune.
    pub entrees: u32,
    /// La durée de chaque condition, en secondes, au moins (les téléphones jouent pendant
    /// ce temps).
    pub duree_s: u64,
    /// Les téléphones natifs (sinon, ceux que `soiree` donne au jeu).
    pub telephones: Option<usize>,
    /// Les échos de M14, par sorte.
    pub echos: u32,
    /// Le dossier de travail.
    pub travail: PathBuf,
    /// Construire le paquet de la page.
    pub construire: bool,
    /// Le navigateur avec sa fenêtre : la cadence des images est alors celle de l'écran de
    /// la machine, et non celle de l'affichage virtuel du navigateur sans interface.
    pub interface: crate::navigateur::Interface,
}

/// Une condition de M2 : le pas au signal de l'image ou non, glisser ou non.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Condition {
    /// Le pas joué au signal de l'image (P6).
    pub calage: bool,
    /// La télé glisse (forcé).
    pub glisser: bool,
}

impl Condition {
    /// Son nom au rapport.
    pub fn nom(self) -> String {
        format!(
            "{} calage, {} glisser",
            if self.calage { "avec" } else { "sans" },
            if self.glisser { "avec" } else { "sans" }
        )
    }
}

/// Les quatre conditions, dans l'ordre où le passage les joue.
pub const CONDITIONS: [Condition; 4] = [
    Condition {
        calage: true,
        glisser: false,
    },
    Condition {
        calage: false,
        glisser: false,
    },
    Condition {
        calage: true,
        glisser: true,
    },
    Condition {
        calage: false,
        glisser: true,
    },
];

/// L'écart estimé de deux horloges (M14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ecart {
    /// L'écart médian des échos les plus courts, en microsecondes : l'autre horloge moins
    /// celle de la page.
    pub ecart_us: i64,
    /// Le plus court demi-aller-retour : l'incertitude.
    pub incertitude_us: u64,
    /// Le plus long des échos gardés, en demi-aller-retour.
    pub demi_max_us: u64,
    /// Les échos reçus.
    pub n: usize,
}

/// L'écart de deux horloges d'après des échos (départ, autre, retour), en microsecondes :
/// le départ et le retour sur l'horloge de la page, l'autre sur l'autre. Garde le dixième
/// des échos le plus court (au moins un), et rend leur écart médian ; `None` sans écho, ou
/// si un retour précède son départ.
pub fn ecart_par_echo(echos: &[(u64, u64, u64)]) -> Option<Ecart> {
    let mut v: Vec<(u64, i64)> = Vec::new();
    for &(depart, autre, retour) in echos {
        let aller_retour = retour.checked_sub(depart)?;
        let milieu = depart.checked_add(aller_retour / 2)?;
        let ecart = i64::try_from(autre)
            .ok()?
            .checked_sub(i64::try_from(milieu).ok()?)?;
        v.push((aller_retour, ecart));
    }
    v.sort_unstable();
    let garde = v.len().div_ceil(10).max(1);
    let courts = v.get(..garde)?;
    let mut ecarts: Vec<i64> = courts.iter().map(|(_, e)| *e).collect();
    let d = distribution(&mut ecarts)?;
    Some(Ecart {
        ecart_us: d.mediane,
        incertitude_us: courts.first()?.0 / 2,
        demi_max_us: courts.last()?.0 / 2,
        n: v.len(),
    })
}

/// Une trace, relue d'une ligne `trace` de la page ; la source avec elle.
pub fn trace_de(j: &Json) -> Option<(String, Trace)> {
    let n = |k: &str| j.get(k).and_then(Json::as_u64);
    let source = j.get("source").and_then(Json::as_str)?.to_owned();
    let id = match n("appareil") {
        Some(a) => IdTrace::Appareil {
            appareil: Appareil::du_fil(a),
            numero: n("id")?,
        },
        None => IdTrace::Ecran(n("id")?),
    };
    Some((
        source,
        Trace {
            id,
            evenement_us: n("evenement")?,
            envoi_us: n("envoi")?,
            relais: None,
            recu_us: n("recu")?,
            place: Pas::du_fil(n("place")?),
            pas_debut_us: n("pas_debut")?,
            pas_fin_us: n("pas_fin")?,
            vue_fin_us: n("vue_fin")?,
            page_recue_us: n("page_recue")?,
            image_us: n("image")?,
            soumis_us: n("soumis")?,
            gpu_fini_us: n("gpu_fini")?,
            incertitude_us: u32::try_from(n("incertitude")?).unwrap_or(u32::MAX),
        },
    ))
}

/// Ce qu'une condition a mesuré, pour une source.
#[derive(Debug, Clone)]
pub struct ParSource {
    /// La source (`clavier`, `souris`, `telephone`).
    pub source: String,
    /// Les traces reçues.
    pub traces: usize,
    /// Celles qui ont leur pixel.
    pub avec_pixel: usize,
    /// La distribution de chaque segment, sur les traces qui l'ont.
    pub segments: Vec<(Segment, Distribution)>,
    /// La plus grande incertitude d'un téléphone (un demi-aller-retour), en µs.
    pub incertitude_max_us: u32,
}

/// Les distributions de traces, par source.
pub fn resumer(traces: &[(String, Trace)]) -> Vec<ParSource> {
    let mut sources: Vec<String> = traces.iter().map(|(s, _)| s.clone()).collect();
    sources.sort();
    sources.dedup();
    sources
        .into_iter()
        .map(|source| {
            let de: Vec<&Trace> = traces
                .iter()
                .filter(|(s, _)| *s == source)
                .map(|(_, t)| t)
                .collect();
            let segments = SEGMENTS
                .iter()
                .filter_map(|seg| {
                    let mut v: Vec<i64> = de.iter().filter_map(|t| duree(t, *seg)).collect();
                    distribution(&mut v).map(|d| (*seg, d))
                })
                .collect();
            ParSource {
                traces: de.len(),
                avec_pixel: de.iter().filter(|t| t.gpu_fini_us != 0).count(),
                incertitude_max_us: de.iter().map(|t| t.incertitude_us).max().unwrap_or(0),
                segments,
                source,
            }
        })
        .collect()
}

/// Le budget du CONTRAT d'une source, en µs (le clavier et la souris : 50 ms).
fn budget_us(source: &str) -> i64 {
    if source == "telephone" {
        salle::trace::BUDGET_TELEPHONE_US
    } else {
        salle::trace::BUDGET_CLAVIER_US
    }
}

/// Le segment qui pèse le plus au 99ᵉ centile (hors du total), si le total dépasse le
/// budget.
pub fn segment_fautif(p: &ParSource) -> Option<(Segment, i64)> {
    let total = p
        .segments
        .iter()
        .find(|(s, _)| *s == Segment::Total)
        .map(|(_, d)| d.c99)?;
    if total <= budget_us(&p.source) {
        return None;
    }
    p.segments
        .iter()
        .filter(|(s, _)| *s != Segment::Total)
        .max_by_key(|(_, d)| d.c99)
        .map(|(s, d)| (*s, d.c99))
}

/// Ce qu'un passage (un navigateur) a mesuré.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur et sa version.
    pub navigateur: String,
    /// Le moteur de dessin.
    pub moteur: String,
    /// Par condition : son nom, et ce qu'elle a mesuré par source.
    pub conditions: Vec<(String, Vec<ParSource>)>,
    /// M14 : (entre quoi, l'écart).
    pub echos: Vec<(String, Option<Ecart>)>,
    /// Le texte du panneau de diagnostic à la fin.
    pub diagnostic: String,
    /// Ce qui est noté.
    pub notes: Vec<String>,
    /// Ce qui a empêché de mesurer.
    pub fautes: Vec<String>,
    /// Le navigateur n'a pas pu jouer.
    pub pas_execute: Option<String>,
}

/// Des microsecondes en millisecondes, au centième, pour le rapport.
fn ms(us: i64) -> String {
    let signe = if us < 0 { "-" } else { "" };
    let a = us.unsigned_abs();
    format!("{signe}{}.{:02}", a / 1000, (a % 1000) / 10)
}

impl core::fmt::Display for Passage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "\n== {} ({}) ==", self.navigateur, self.moteur)?;
        if let Some(e) = &self.pas_execute {
            return writeln!(f, "pas exécuté : {e}");
        }
        for (nom, sources) in &self.conditions {
            writeln!(f, "-- {nom}")?;
            for p in sources {
                writeln!(
                    f,
                    "  {} : {} traces, {} avec leur pixel ; incertitude au plus {} ms ; « médiane ; 99ᵉ ; max » en ms",
                    p.source,
                    p.traces,
                    p.avec_pixel,
                    ms(i64::from(p.incertitude_max_us))
                )?;
                for (s, d) in &p.segments {
                    writeln!(
                        f,
                        "    {:<13}{} ; {} ; {}  (n {})",
                        s.nom(),
                        ms(d.mediane),
                        ms(d.c99),
                        ms(d.max),
                        d.n
                    )?;
                }
                if let Some((s, c99)) = segment_fautif(p) {
                    writeln!(
                        f,
                        "    BUDGET DÉPASSÉ au 99ᵉ centile ({} ms) : le segment qui pèse le plus est « {} » ({} ms)",
                        ms(budget_us(&p.source)),
                        s.nom(),
                        ms(c99)
                    )?;
                }
            }
        }
        writeln!(f, "-- M14, l'écart des horloges (l'autre moins la page)")?;
        for (quoi, e) in &self.echos {
            match e {
                Some(e) => writeln!(
                    f,
                    "  {quoi} : {} ms, à {} ms près (demi-aller-retour le plus court ; le plus long gardé {} ms) ; {} échos",
                    ms(e.ecart_us),
                    ms(i64::try_from(e.incertitude_us).unwrap_or(i64::MAX)),
                    ms(i64::try_from(e.demi_max_us).unwrap_or(i64::MAX)),
                    e.n
                )?,
                None => writeln!(f, "  {quoi} : aucun écho")?,
            }
        }
        for n in &self.notes {
            writeln!(f, "note : {n}")?;
        }
        for x in &self.fautes {
            writeln!(f, "FAUTE : {x}")?;
        }
        Ok(())
    }
}

/// Un générateur tiré à graine fixe (les intervalles entre deux entrées) : xorshift.
struct Tirage(u64);

impl Tirage {
    fn entre(&mut self, a: u64, b: u64) -> u64 {
        self.0 ^= self.0.wrapping_shl(13);
        self.0 ^= self.0.wrapping_shr(7);
        self.0 ^= self.0.wrapping_shl(17);
        a.saturating_add(self.0.checked_rem(b.saturating_sub(a).max(1)).unwrap_or(0))
    }
}

/// Clique là où l'écran du commun le propose (« Jouer »), par la vraie chaîne d'entrées,
/// et attend le geste.
fn jouer(s: &mut Session) -> Result<(), String> {
    let valeur = s.evaluer(0, "JSON.stringify(window.__banc.centre())")?;
    let centre: Json = valeur
        .as_str()
        .and_then(|texte| serde_json::from_str(texte).ok())
        .unwrap_or(Json::Null);
    let (Some(x), Some(y)) = (
        centre.get(0).and_then(Json::as_f64),
        centre.get(1).and_then(Json::as_f64),
    ) else {
        return Err(String::from("l'écran du commun ne propose pas « Jouer »"));
    };
    let depuis = s.lignes.len();
    s.navigateur.cliquer(0, x, y)?;
    s.attendre(depuis, Duration::from_secs(20), "le geste", |l| {
        l.quoi() == "geste-fait"
    })
    .map(|_| ())
}

/// Le voile que l'écran du commun montre sur le jeu, d'après sa dernière ligne
/// `ecran-du-commun` (`montre`) ; `None` s'il ne montre rien (lot 13b). Avec la fenêtre de
/// Chrome, session de macOS verrouillée, le plein écran est quitté aussitôt entré, et le
/// voile « Plein écran quitté — cliquer pour revenir » couvre le jeu : chaque clic du banc
/// y va, et non au jeu.
pub fn voile_sur_le_jeu(lignes: &[soiree::Ligne]) -> Option<String> {
    let montre = lignes
        .iter()
        .rev()
        .find(|l| l.quoi() == "ecran-du-commun")?
        .json
        .get("montre")?
        .as_str()?;
    (montre != "rien").then(|| montre.to_owned())
}

/// Les clics qui sont allés au voile du commun depuis la ligne `depuis` : chacun y refait
/// le geste (une ligne `geste`), au lieu d'aller au jeu (lot 13b).
pub fn clics_au_voile(lignes: &[soiree::Ligne], depuis: usize) -> usize {
    lignes
        .iter()
        .skip(depuis)
        .filter(|l| l.quoi() == "geste")
        .count()
}

/// Lot 13b : le jeu est-il à découvert, `quand` (une seconde après « Jouer », puis après
/// M14) ? Le voile du commun sur le jeu (le plein écran quitté), les clics de la souris
/// iraient au voile : la souris ne serait pas mesurée, et c'est une faute, dite (le
/// clavier, lui, va au jeu). `session` dit la session de l'écran, pour le rapport.
pub fn faute_du_voile(lignes: &[soiree::Ligne], quand: &str, session: &str) -> Option<String> {
    voile_sur_le_jeu(lignes).map(|voile| {
        format!(
            "le voile du commun « {voile} » couvre le jeu {quand} (le plein écran quitté ; {session}) : les clics de la souris vont au voile, la souris n'est pas mesurée"
        )
    })
}

/// [`faute_du_voile`] au passage ; rend vrai si le voile couvre le jeu.
fn regarder_le_voile(lignes: &[soiree::Ligne], p: &mut Passage, quand: &str) -> bool {
    let faute = faute_du_voile(lignes, quand, &crate::navigateur::note_de_la_session());
    let couvert = faute.is_some();
    p.fautes.extend(faute);
    couvert
}

/// La faute d'une condition dont des clics sont allés au voile (lot 13b).
fn faute_des_clics_au_voile(c: Condition, au_voile: usize) -> Option<String> {
    (au_voile > 0).then(|| {
        format!(
            "{} : {au_voile} clic(s) de la souris allés au voile du commun et non au jeu : la souris de cette condition ne vaut pas mesure",
            c.nom()
        )
    })
}

/// Le centre d'un bouton de la scène, par son nom (`window.__banc.composants()`), en
/// pixels CSS.
fn bouton(s: &mut Session, nom: &str) -> Result<Option<(f64, f64)>, String> {
    let v = s.evaluer(0, "window.__banc.composants()")?;
    let j: Json = v
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    Ok(bouton_dans(&j, nom))
}

/// Le centre du bouton `nom` dans ce que `composants()` rend, où qu'il soit.
fn bouton_dans(j: &Json, nom: &str) -> Option<(f64, f64)> {
    match j {
        Json::Object(o) => {
            if o.get("sorte").and_then(Json::as_str) == Some("bouton")
                && o.get("nom").and_then(Json::as_str) == Some(nom)
            {
                let n = |k: &str| o.get(k).and_then(Json::as_f64);
                return Some((n("x")? + n("l")? / 2.0, n("y")? + n("h")? / 2.0));
            }
            o.values().find_map(|v| bouton_dans(v, nom))
        }
        Json::Array(a) => a.iter().find_map(|v| bouton_dans(v, nom)),
        Json::Null | Json::Bool(_) | Json::Number(_) | Json::String(_) => None,
    }
}

/// La cadence de `requestAnimationFrame` dans la page, en images par seconde, sur deux
/// secondes : ce que l'écran donne à la télé (la mesure 6 l'a trouvée à 60 dans Chrome
/// sans interface sur une page simple, à 120 dans Firefox).
fn cadence(s: &mut Session) -> Result<String, String> {
    let v = s.evaluer(
        0,
        "new Promise((ok) => { let n = 0; const t0 = performance.now(); const f = () => { n += 1; if (performance.now() - t0 < 2000) { requestAnimationFrame(f); } else { ok(String((n * 1000 / (performance.now() - t0)).toFixed(1))); } }; requestAnimationFrame(f); })",
    )?;
    Ok(v.as_str().unwrap_or("?").to_owned())
}

/// Les traces lues depuis la ligne `depuis`.
fn traces_depuis(s: &Session, depuis: usize) -> Vec<(String, Trace)> {
    s.lignes
        .iter()
        .skip(depuis)
        .filter(|l| l.quoi() == "trace")
        .filter_map(|l| trace_de(&l.json))
        .collect()
}

/// Les scènes où un nœud a glissé et les images à mi-chemin, depuis le début de la page
/// (`window.__banc.glisses()`).
fn glisses(s: &mut Session) -> Result<(u64, u64), String> {
    let v = s.evaluer(0, "window.__banc.glisses()")?;
    let j: Json = v
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    let n = |k: usize| j.get(k).and_then(Json::as_u64);
    n(0).zip(n(1))
        .ok_or_else(|| String::from("la page ne dit pas ce qui a glissé"))
}

/// Ce que glisser a fait pendant une condition, au rapport : sans scène où un nœud
/// glisse, la condition « avec glisser » est celle « sans glisser » (corrigé après la
/// vérification du lot 13 : ardoise et Punchline n'ont aucun nœud qui glisse, et leurs
/// chiffres « avec glisser » ne mesuraient pas un vrai glisser).
pub fn note_de_glisser(c: Condition, (scenes, images): (u64, u64)) -> String {
    if c.glisser && scenes == 0 {
        format!(
            "{} : aucune scène où un nœud glisse ; glisser n'a rien déplacé, la condition vaut « sans glisser »",
            c.nom()
        )
    } else {
        format!(
            "{} : {scenes} scène(s) où un nœud a glissé, {images} image(s) à mi-chemin",
            c.nom()
        )
    }
}

/// Les traces d'une condition, et ce que glisser y a fait (scènes où un nœud a glissé,
/// images à mi-chemin).
type TracesEtGlisses = (Vec<(String, Trace)>, (u64, u64));

/// Une condition : réglée, puis les entrées de l'écran (touches, et un clic sur
/// « Effacer » une fois sur huit), pendant au moins `duree_s`. Rend les traces, et ce que
/// glisser a fait pendant la condition (scènes où un nœud a glissé, images à mi-chemin).
fn une_condition(
    o: &Options,
    s: &mut Session,
    c: Condition,
    tirage: &mut Tirage,
) -> Result<TracesEtGlisses, String> {
    s.evaluer(0, &format!("window.__banc.calage({}), true", c.calage))?;
    s.evaluer(
        0,
        &format!(
            "window.__banc.glisser(\"{}\"), true",
            if c.glisser { "oui" } else { "non" }
        ),
    )?;
    // Le réglage passe ; les traces d'avant ne comptent pas.
    s.tourner(Duration::from_secs(2))?;
    let depuis = s.lignes.len();
    let glisses_avant = glisses(s)?;
    let debut = crate::horloge::maintenant();
    for k in 0..o.entrees {
        if k % 8 == 7
            && let Some((x, y)) = bouton(s, "Effacer")?
        {
            s.navigateur.cliquer(0, x, y)?;
        } else {
            let chiffre = tirage.entre(0, 10);
            s.navigateur.touche(0, &format!("Digit{chiffre}"))?;
        }
        // Un intervalle tiré de 40 à 140 ms : les entrées tombent à toute phase de
        // l'image.
        s.tourner(Duration::from_millis(tirage.entre(40, 140)))?;
    }
    let reste = Duration::from_secs(o.duree_s)
        .saturating_sub(crate::horloge::maintenant().saturating_duration_since(debut));
    s.tourner(reste)?;
    // Les dernières traces arrivent (leur image, leur GPU).
    s.tourner(Duration::from_secs(1))?;
    let glisses_apres = glisses(s)?;
    Ok((
        traces_depuis(s, depuis),
        (
            glisses_apres.0.saturating_sub(glisses_avant.0),
            glisses_apres.1.saturating_sub(glisses_avant.1),
        ),
    ))
}

/// Le retard du dessin du contrôle négatif, en ms (choisi : deux fois une image à 60 Hz).
pub const DESSIN_LENT_MS: i64 = 33;

/// Le jugement du contrôle négatif, d'après la médiane du segment du dessin au clavier,
/// en µs, dans la première condition (`normal`) et dessin ralenti (`ralenti`) : la note
/// s'il se voit (au moins les trois quarts du retard), la faute sinon, ou si l'une des
/// deux manque.
pub fn juger_le_controle(normal: Option<i64>, ralenti: Option<i64>) -> Result<String, String> {
    match (normal, ralenti) {
        (Some(normal), Some(ralenti)) if ralenti.saturating_sub(normal) >= DESSIN_LENT_MS * 750 => {
            Ok(format!(
                "contrôle négatif : le dessin ralenti de {DESSIN_LENT_MS} ms se voit (segment du dessin, médiane {} ms au lieu de {} ms)",
                ms(ralenti),
                ms(normal)
            ))
        }
        (normal, ralenti) => Err(format!(
            "CONTRÔLE NÉGATIF RATÉ : le dessin ralenti de {DESSIN_LENT_MS} ms ne se voit pas dans le segment du dessin (médianes {normal:?} puis {ralenti:?} µs)"
        )),
    }
}

/// Le contrôle négatif : chaque dessin de la page ralenti de [`DESSIN_LENT_MS`]
/// (`window.__banc.dessin_lent`, au banc seulement), avec calage, sans glisser. Le segment
/// du dessin du clavier doit monter d'au moins les trois quarts de ce retard en médiane
/// par rapport à la première condition ; sinon les traces ne mesurent pas ce qu'elles
/// disent, et c'est une faute (« CONTRÔLE NÉGATIF RATÉ »).
fn controle_negatif(
    o: &Options,
    s: &mut Session,
    p: &mut Passage,
    tirage: &mut Tirage,
) -> Result<(), String> {
    if o.entrees == 0 {
        p.notes.push(String::from(
            "contrôle négatif : pas joué (aucune entrée de l'écran)",
        ));
        return Ok(());
    }
    s.evaluer(
        0,
        &format!("window.__banc.dessin_lent({DESSIN_LENT_MS}), true"),
    )?;
    let lente = Condition {
        calage: true,
        glisser: false,
    };
    let courte = Options {
        entrees: o.entrees.div_ceil(3),
        duree_s: 0,
        ..o.clone()
    };
    let traces = une_condition(&courte, s, lente, tirage);
    s.evaluer(0, "window.__banc.dessin_lent(0), true")?;
    let lentes = resumer(&traces?.0);
    let mediane = |sources: &[ParSource]| {
        sources
            .iter()
            .find(|x| x.source == "clavier")
            .and_then(|x| x.segments.iter().find(|(seg, _)| *seg == Segment::Dessin))
            .map(|(_, d)| d.mediane)
    };
    let avant = p.conditions.first().and_then(|(_, v)| mediane(v));
    match juger_le_controle(avant, mediane(&lentes)) {
        Ok(note) => p.notes.push(note),
        Err(faute) => p.fautes.push(faute),
    }
    p.conditions.push((
        String::from("contrôle négatif : dessin ralenti, avec calage, sans glisser"),
        lentes,
    ));
    Ok(())
}

/// Les échos de la page à son Worker : `window.__banc.echo()`, puis les lignes `echo`.
fn echos_du_worker(s: &mut Session, n: u32) -> Result<Vec<(u64, u64, u64)>, String> {
    let depuis = s.lignes.len();
    for _ in 0..n {
        s.evaluer(0, "window.__banc.echo(), true")?;
        s.tourner(Duration::from_millis(5))?;
    }
    s.tourner(Duration::from_millis(500))?;
    Ok(s.lignes
        .iter()
        .skip(depuis)
        .filter(|l| l.quoi() == "echo")
        .filter_map(|l| {
            let n = |k: &str| l.json.get(k).and_then(Json::as_u64);
            Some((n("page")?, n("worker")?, n("retour")?))
        })
        .collect())
}

/// Des millisecondes de la page (des `f64` de JSON) en microsecondes.
fn us(x: &Json) -> Option<u64> {
    let v = x.as_f64()? * 1000.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "un instant positif du navigateur, en microsecondes depuis 1970"
    )]
    let r = (v.is_finite() && v > 0.0).then_some(v as u64);
    r
}

/// Les échos de la page au processus du banc : chaque évaluation encadrée par l'heure
/// murale du banc. Ici, la page est « l'autre » ; l'écart rendu est retourné ensuite.
fn echos_du_banc(s: &mut Session, n: u32) -> Result<Vec<(u64, u64, u64)>, String> {
    let mut v = Vec::new();
    for _ in 0..n {
        let depart = heure_us();
        let t = s.evaluer(0, "performance.timeOrigin + performance.now()")?;
        let retour = heure_us();
        if let Some(t) = us(&t) {
            v.push((depart, t, retour));
        }
    }
    Ok(v)
}

/// Les échos de la page à un autre onglet de la même origine, par `BroadcastChannel`.
fn echos_d_un_onglet(s: &mut Session, n: u32) -> Result<Vec<(u64, u64, u64)>, String> {
    let url = format!("http://localhost:{}/banc-echo", s.port);
    // Derrière l'onglet de l'écran, qui reste devant, visible et en plein écran
    // (correction du lot 13b).
    let autre = s.navigateur.nouvel_onglet_derriere(&url)?;
    s.tourner(Duration::from_secs(1))?;
    s.evaluer(
        autre,
        "window.__c = new BroadcastChannel('banc-echo'); window.__c.onmessage = (e) => window.__c.postMessage([e.data, performance.timeOrigin + performance.now()]); true",
    )?;
    let js = format!(
        "(async () => {{ const c = new BroadcastChannel('banc-echo'); const r = []; for (let i = 0; i < {n}; i++) {{ const t0 = performance.timeOrigin + performance.now(); const v = await new Promise((ok) => {{ c.onmessage = (e) => ok(e.data); c.postMessage(i); }}); r.push([t0, v[1], performance.timeOrigin + performance.now()]); }} c.close(); return JSON.stringify(r); }})()"
    );
    let r = s.evaluer(0, &js);
    let _: Result<(), String> = s.navigateur.fermer_onglet(autre);
    let j: Json = r?
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    Ok(j.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|e| Some((us(e.get(0)?)?, us(e.get(1)?)?, us(e.get(2)?)?)))
                .collect()
        })
        .unwrap_or_default())
}

/// M14 avant tout découpage (la ligne du lot, § 8.4 ; corrigé après la vérification du
/// lot 13 : les échos venaient après les quatre conditions) ; sorti de `passage` au lot
/// 13b, tel quel.
fn m14(o: &Options, s: &mut Session, p: &mut Passage) -> Result<(), String> {
    let du_worker = echos_du_worker(s, o.echos)?;
    p.echos.push((
        String::from("le Worker de l'autorité"),
        ecart_par_echo(&du_worker),
    ));
    let du_banc = echos_du_banc(s, o.echos)?;
    // L'écho du banc a l'horloge du banc aux deux bouts : l'écart rendu est la page
    // moins le banc ; on le retourne, pour dire le banc moins la page.
    p.echos.push((
        String::from("le processus du banc (et des téléphones natifs)"),
        ecart_par_echo(&du_banc).map(|e| Ecart {
            ecart_us: e.ecart_us.saturating_neg(),
            ..e
        }),
    ));
    let d_un_onglet = echos_d_un_onglet(s, o.echos)?;
    p.echos.push((
        String::from("un autre onglet, même origine"),
        ecart_par_echo(&d_un_onglet),
    ));
    // Les segments `transport` et `vers-la-page` d'une entrée de l'écran vont d'une
    // horloge à l'autre (la page, son Worker) : ils portent l'écart de M14, que le
    // rapport donne à côté, et non dans l'incertitude de chaque trace (celle d'un
    // téléphone : son demi-aller-retour).
    if let Some((_, Some(e))) = p.echos.first() {
        p.notes.push(format!(
            "les segments entre la page et son Worker (transport et vers-la-page d'une entrée de l'écran) portent en plus l'écart de M14 : {} ms, à {} ms près",
            ms(e.ecart_us),
            ms(i64::try_from(e.incertitude_us).unwrap_or(i64::MAX))
        ));
    }
    Ok(())
}

fn passage(
    o: &Options,
    n: Navigateur,
    (paquet, module): (&Path, &[u8]),
    charge: &moteur_wasmtime::ModuleCharge,
) -> Result<(Passage, Vec<Ligne>, Vec<String>), String> {
    let so = soiree::Options {
        module: o.module.clone(),
        navigateurs: vec![n],
        telephones: o.telephones,
        construire: false,
        duree_max_s: 0,
        travail: o.travail.clone(),
        m12: 0,
        cache_s: 0,
        fois: 1,
        pages: Vec::new(),
        duree_min_s: 0,
        retardataires: false,
        autres_jeux: Vec::new(),
        fautif: None,
        interface: o.interface,
        cacher: soiree::Cacher::Onglet,
        complete: false,
    };
    let (telephones, redite) = soiree::telephones_et_redite(&so, charge);
    let dossier = o.travail.join(n.nom());
    let mut s = soiree::ouvrir(&so, n, paquet, module, &dossier, (redite, ""))?;
    let mut p = Passage {
        navigateur: s.navigateur.version().to_owned(),
        ..Passage::default()
    };
    let mut soiree_p = soiree::Passage::default();
    let deroule = (|| -> Result<(), String> {
        let _: String = soiree::debuter(&mut s, &mut soiree_p, telephones)?;
        p.moteur.clone_from(&soiree_p.moteur);
        let avant = cadence(&mut s)?;
        if o.interface == crate::navigateur::Interface::Avec {
            p.notes.push(crate::navigateur::note_de_la_session());
        }
        jouer(&mut s)?;
        s.tourner(Duration::from_secs(1))?;
        let couvert = regarder_le_voile(&s.lignes, &mut p, "une seconde après « Jouer »");
        let apres = cadence(&mut s)?;
        p.notes.push(format!(
            "requestAnimationFrame dans la page : {avant} images/s sous l'écran de démarrage, {apres} en jeu, panneau fermé"
        ));
        p.notes.push(format!(
            "{telephones} téléphone(s) natif(s) ; {} entrée(s) de l'écran par condition ; au moins {} s par condition",
            o.entrees, o.duree_s
        ));
        // Le panneau ouvert par la touche que le jeu déclare, par la chaîne d'entrées du
        // navigateur : le carré blanc s'allume à chaque entrée, et son texte est lu à la
        // fin.
        let touche = charge.declaration().diagnostic;
        if let Err(e) = s.navigateur.touche(0, touche.as_str()) {
            p.notes.push(format!(
                "le panneau ne s'ouvre pas par sa touche {} : {e}",
                touche.as_str()
            ));
        }
        s.tourner(Duration::from_secs(1))?;
        let ouvert = cadence(&mut s)?;
        p.notes.push(format!(
            "requestAnimationFrame dans la page : {ouvert} images/s en jeu, panneau ouvert"
        ));
        m14(o, &mut s, &mut p)?;
        // Correction du lot 13b : l'écho avec un autre onglet passait cet onglet devant
        // celui de l'écran, qui, caché, quittait le plein écran (sans interface aussi) ; le
        // voile prenait alors le premier clic de la souris des conditions. L'onglet s'ouvre
        // maintenant derrière ; que le jeu soit encore à découvert se regarde ici, une
        // demi-seconde après (la sortie du plein écran arrive en retard sur l'onglet revenu
        // devant), et seulement si le voile n'était pas déjà là après « Jouer ».
        s.tourner(Duration::from_millis(500))?;
        if !couvert {
            let _: bool = regarder_le_voile(&s.lignes, &mut p, "après M14, avant les conditions");
        }
        let mut tirage = Tirage(0x5eed_1a7e_0013);
        for c in CONDITIONS {
            let depuis = s.lignes.len();
            let (traces, glisse) = une_condition(o, &mut s, c, &mut tirage)?;
            if traces.is_empty() {
                p.fautes.push(format!("{} : aucune trace reçue", c.nom()));
            }
            if let Some(faute) = faute_des_clics_au_voile(c, clics_au_voile(&s.lignes, depuis)) {
                p.fautes.push(faute);
            }
            p.notes.push(note_de_glisser(c, glisse));
            p.conditions.push((c.nom(), resumer(&traces)));
        }
        controle_negatif(o, &mut s, &mut p, &mut tirage)?;
        let texte = s.evaluer(0, "window.__banc.diagnostic()")?;
        texte.as_str().unwrap_or("").clone_into(&mut p.diagnostic);
        Ok(())
    })();
    if let Err(e) = deroule {
        p.fautes.push(e);
    }
    p.fautes.extend(soiree_p.fautes);
    let (lignes, autres) = (s.lignes.clone(), s.autres.clone());
    s.fermer();
    soiree::ecrire_les_lignes(&dossier, &lignes, &autres);
    Ok((p, lignes, autres))
}

/// Joue `mesures` dans chaque navigateur ; écrit `mesures.txt` sous le dossier de travail.
pub fn jouer_les_mesures(o: &Options, soiree_dir: &Path) -> Result<Vec<Passage>, String> {
    let module = std::fs::read(&o.module).map_err(|e| format!("{} : {e}", o.module.display()))?;
    let charge = soiree::charger_le_module(&module)?;
    let paquet = if o.construire {
        soiree::construire_le_paquet(soiree_dir)?
    } else {
        soiree::dernier_paquet(soiree_dir)?
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    let mut passages = Vec::new();
    for n in &o.navigateurs {
        match passage(o, *n, (&paquet, &module), &charge) {
            Ok((p, _, _)) => passages.push(p),
            Err(e) if e.starts_with("pas exécuté") => passages.push(Passage {
                navigateur: n.nom().to_owned(),
                pas_execute: Some(e),
                ..Passage::default()
            }),
            Err(e) => passages.push(Passage {
                navigateur: n.nom().to_owned(),
                fautes: vec![e],
                ..Passage::default()
            }),
        }
    }
    let mut texte = String::new();
    for p in &passages {
        let _: core::fmt::Result = write!(texte, "{p}");
        let _: core::fmt::Result = write!(texte, "\n-- le panneau de diagnostic\n{}", p.diagnostic);
    }
    let _: std::io::Result<()> = std::fs::write(o.travail.join("mesures.txt"), texte);
    Ok(passages)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lot 13b : le voile du commun sur le jeu se lit dans la dernière ligne
    /// `ecran-du-commun`, et chaque clic qui y va laisse une ligne `geste` (les lignes du
    /// passage `m2-ardoise-chrome-interface` de la correction du lot 13, abrégées).
    #[test]
    fn le_voile_sur_le_jeu_et_les_clics_qui_y_vont_se_disent() {
        let ligne = |texte: &str| soiree::Ligne {
            onglet: String::from("975b816f"),
            json: serde_json::from_str(texte).unwrap(),
            ms: 2400,
        };
        let mut lignes = vec![
            ligne(r#"{"quoi":"ecran-du-commun","montre":"demarrage"}"#),
            ligne(r#"{"quoi":"geste"}"#),
            ligne(r#"{"quoi":"ecran-du-commun","montre":"rien","plein_ecran":true}"#),
        ];
        assert_eq!(voile_sur_le_jeu(&lignes), None);
        assert_eq!(voile_sur_le_jeu(&[]), None);
        let depuis = lignes.len();
        lignes.push(ligne(r#"{"quoi":"plein-ecran","dedans":false}"#));
        lignes.push(ligne(
            r#"{"quoi":"ecran-du-commun","montre":"plein-ecran-quitte","plein_ecran":false}"#,
        ));
        assert_eq!(
            voile_sur_le_jeu(&lignes).as_deref(),
            Some("plein-ecran-quitte")
        );
        assert_eq!(clics_au_voile(&lignes, depuis), 0);
        lignes.push(ligne(r#"{"quoi":"geste"}"#));
        lignes.push(ligne(r#"{"quoi":"touche","code":"Digit9","au":"jeu"}"#));
        lignes.push(ligne(r#"{"quoi":"geste"}"#));
        assert_eq!(clics_au_voile(&lignes, depuis), 2);
        assert_eq!(clics_au_voile(&lignes, 0), 3);
    }

    /// Correction du lot 13b : le voile qui tombe sur le jeu après M14 (l'écho avec un
    /// autre onglet, passé devant celui de l'écran) se dit, avec le moment et la session ;
    /// les lignes du contrôle sans interface de la vérification du lot 13b
    /// (`target/travail/verif-lot-13b/mesures-sans-interface`, Chrome), abrégées : le plein
    /// écran entré à 2 337 ms, quitté à 10 467 ms, pendant M14.
    #[test]
    fn le_voile_tombe_apres_m14_se_dit() {
        let ligne = |ms: u128, texte: &str| soiree::Ligne {
            onglet: String::from("c23ce4e2"),
            json: serde_json::from_str(texte).unwrap(),
            ms,
        };
        let mut lignes = vec![
            ligne(2329, r#"{"quoi":"geste"}"#),
            ligne(
                2329,
                r#"{"quoi":"ecran-du-commun","montre":"rien","plein_ecran":true,"verrou":true}"#,
            ),
            ligne(2337, r#"{"quoi":"plein-ecran","dedans":true}"#),
        ];
        assert_eq!(
            faute_du_voile(&lignes, "une seconde après « Jouer »", "s"),
            None
        );
        lignes.push(ligne(10467, r#"{"quoi":"plein-ecran","dedans":false}"#));
        lignes.push(ligne(
            10467,
            r#"{"quoi":"ecran-du-commun","montre":"plein-ecran-quitte","plein_ecran":false,"verrou":false}"#,
        ));
        let faute = faute_du_voile(
            &lignes,
            "après M14, avant les conditions",
            "session ouverte",
        );
        assert_eq!(
            faute.as_deref(),
            Some(
                "le voile du commun « plein-ecran-quitte » couvre le jeu après M14, avant les conditions (le plein écran quitté ; session ouverte) : les clics de la souris vont au voile, la souris n'est pas mesurée"
            )
        );
    }

    /// M14 : un écart connu se retrouve, à un demi-aller-retour près, et les échos les
    /// plus longs (une page occupée) ne le déplacent pas.
    #[test]
    fn l_ecart_d_un_echo_se_retrouve_par_les_echos_les_plus_courts() {
        let decalage: i64 = 470;
        let mut echos = Vec::new();
        for k in 0..100u64 {
            let depart = 1_000_000 + k * 10_000;
            // L'aller et le retour : 20 µs chacun, et une fois sur cinq la page occupée
            // 3 ms au retour.
            let aller = 20;
            let retour = if k % 5 == 0 { 3_000 } else { 20 };
            let autre = u64::try_from(i64::try_from(depart + aller).unwrap() + decalage).unwrap();
            echos.push((depart, autre, depart + aller + retour));
        }
        let e = ecart_par_echo(&echos).unwrap();
        assert_eq!(e.ecart_us, decalage);
        assert_eq!((e.incertitude_us, e.demi_max_us, e.n), (20, 20, 100));
        assert_eq!(ecart_par_echo(&[]), None);
        assert_eq!(
            ecart_par_echo(&[(10, 5, 9)]),
            None,
            "un retour avant son départ"
        );
    }

    /// Une ligne `trace` de la page se relit en trace ; celle d'un téléphone garde son
    /// appareil et son incertitude.
    #[test]
    fn une_ligne_de_trace_se_relit() {
        let j: Json = serde_json::from_str(
            r#"{"quoi":"trace","source":"telephone","id":4,"appareil":2,"place":30,"evenement":100,"envoi":110,"recu":900,"pas_debut":2000,"pas_fin":2100,"vue_fin":2200,"page_recue":2300,"image":9000,"soumis":9500,"gpu_fini":11000,"incertitude":450}"#,
        )
        .unwrap();
        let (source, t) = trace_de(&j).unwrap();
        assert_eq!(source, "telephone");
        assert_eq!(
            t.id,
            IdTrace::Appareil {
                appareil: Appareil::du_fil(2),
                numero: 4
            }
        );
        assert_eq!((t.place.numero(), t.incertitude_us), (30, 450));
        assert_eq!(duree(&t, Segment::Total), Some(10_900));
        assert_eq!(duree(&t, Segment::AttenteImage), Some(6_700));
    }

    /// Le contrôle négatif rate quand le dessin ralenti ne se voit pas, ou pas assez, ou
    /// quand une médiane manque ; il passe au-delà des trois quarts du retard (corrigé
    /// après la vérification du lot 13 : le chemin « CONTRÔLE NÉGATIF RATÉ » n'était
    /// couvert par aucun test).
    #[test]
    fn le_controle_negatif_rate_quand_le_ralenti_ne_se_voit_pas() {
        let seuil = DESSIN_LENT_MS * 750;
        assert!(juger_le_controle(Some(180), Some(33_170)).is_ok());
        assert!(juger_le_controle(Some(1_000), Some(1_000 + seuil)).is_ok());
        for (normal, ralenti) in [
            (Some(1_000), Some(1_000 + seuil - 1)),
            (Some(180), Some(180)),
            (Some(33_000), Some(200)),
            (None, Some(33_170)),
            (Some(180), None),
            (None, None),
        ] {
            let faute = juger_le_controle(normal, ralenti).unwrap_err();
            assert!(faute.starts_with("CONTRÔLE NÉGATIF RATÉ"), "{faute}");
        }
    }

    /// Une condition « avec glisser » où rien n'a glissé se dit telle (corrigé après la
    /// vérification du lot 13) ; une condition qui a glissé dit combien.
    #[test]
    fn une_condition_avec_glisser_sans_rien_qui_glisse_se_dit() {
        let avec = Condition {
            calage: true,
            glisser: true,
        };
        let sans = Condition {
            calage: true,
            glisser: false,
        };
        assert!(note_de_glisser(avec, (0, 0)).contains("glisser n'a rien déplacé"));
        assert!(note_de_glisser(avec, (12, 30)).contains("12 scène(s) où un nœud a glissé"));
        assert!(!note_de_glisser(sans, (0, 0)).contains("rien déplacé"));
    }

    /// Au-delà du budget du CONTRAT au 99ᵉ centile, le segment qui pèse le plus se nomme ;
    /// en deçà, aucun.
    #[test]
    fn le_segment_fautif_se_nomme_au_dela_du_budget() {
        let lente = |id: u64, image: u64| {
            (
                String::from("clavier"),
                Trace {
                    id: IdTrace::Ecran(id),
                    evenement_us: 1_000,
                    envoi_us: 1_100,
                    relais: None,
                    recu_us: 1_200,
                    place: Pas::du_fil(1),
                    pas_debut_us: 5_000,
                    pas_fin_us: 5_100,
                    vue_fin_us: 5_200,
                    page_recue_us: 5_300,
                    image_us: image,
                    soumis_us: image + 500,
                    gpu_fini_us: image + 2_000,
                    incertitude_us: 0,
                },
            )
        };
        let vite = resumer(&[lente(1, 10_000), lente(2, 12_000)]);
        assert_eq!(segment_fautif(&vite[0]), None);
        let lent = resumer(&[lente(1, 10_000), lente(2, 70_000)]);
        assert_eq!(
            segment_fautif(&lent[0]),
            Some((Segment::AttenteImage, 64_700))
        );
    }
}
