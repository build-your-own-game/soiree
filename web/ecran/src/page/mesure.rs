//! L'entrée au pixel, du côté de la page de la télé (PHASE-3, § 2.9, § 8.4 ; lot 13) :
//!
//! - **les traces** : chaque entrée de l'écran part au Worker avec son numéro, l'instant
//!   de son événement (`event.timeStamp`) et celui de son envoi ; le Worker rend sa trace
//!   (`VersPage::Traces`) juste après la scène du tour qui a joué son pas ; la page y
//!   ajoute la scène reçue, le dessin soumis de la première image qui la montre en entier
//!   et le travail du GPU fini (`onSubmittedWorkDone`, OUT-33), puis la verse au panneau
//!   et, en mode banc, au banc (`{"quoi":"trace",…}`) ;
//! - **le panneau de diagnostic**, ouvert et fermé par la touche que déclare le jeu :
//!   le texte de [`salle::trace::Diagnostic`], et, à chaque entrée, un carré blanc dans un
//!   coin pendant au moins 100 ms, pour la caméra de la répétition (REM-117) ;
//! - **glisser** (§ 2.9) : à 100 Hz ou plus (la période des images, mesurée), ou forcé
//!   par le banc (`?glisser=oui`), les nœuds qui glissent vont de la scène précédente à
//!   la dernière en un pas, **quand l'un d'eux se déplace** (`rendu::glisser::
//!   deplace_des_noeuds`) ; sinon la scène se dessine en entier tout de suite. Une entrée
//!   n'a son pixel qu'à la première image où chaque nœud est au moins à sa place du pas
//!   de l'entrée : l'image qui dessine sa scène en entier, ou une image d'un chemin qui
//!   part de sa scène ou d'une plus récente (le pas de retard que coûte glisser, mesuré
//!   par M2). *Corrigé après la vérification du lot 13* : la page glissait après chaque
//!   scène neuve, même sans nœud qui glisse, et retardait d'autant le pixel de toute
//!   scène ; les chiffres « avec glisser » d'ardoise et de Punchline, qui n'ont aucun
//!   nœud qui glisse, ne mesuraient que ce retard.
//!
//! Les instants sont dans la base `timeOrigin + now()` de la page ; ceux du Worker dans
//! la sienne : M14 mesure leur écart.

use std::collections::VecDeque;
use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use jeu_abi::{CodeTouche, Oui, SceneFil};
use salle::ecran::{IdTrace, Trace};
use salle::trace::{Diagnostic, Source};
use web_sys::{Document, Element, HtmlElement};

use super::style;

/// Les sources des entrées numérotées que la page retient (choisi : bien plus que les
/// entrées d'un tour).
const SOURCES_RETENUES: usize = 512;

/// Les traces qui attendent leur dessin ou leur GPU, au plus (choisi) : au-delà, la plus
/// ancienne s'oublie (une page cachée ne dessine plus).
const TRACES_EN_ATTENTE: usize = 512;

/// Le carré blanc d'une entrée reste allumé au moins ce temps (REM-117).
const CARRE_MS: f64 = 100.0;

/// Le panneau se réécrit au plus tous les 250 ms (choisi).
const PANNEAU_MS: f64 = 250.0;

/// Sous cette période d'image (en ms), la télé glisse : 100 Hz et plus (§ 2.9 dit
/// 120 Hz ; 10 ms laisse passer 100 Hz, choisi).
const PERIODE_DE_GLISSER_MS: f64 = 10.0;

/// La durée d'un pas, en millisecondes (`salle::simulation::PAS_US`, 60 pas par seconde) :
/// le chemin d'un glisser.
const PAS_MS: f64 = 16.667;

/// Ce qu'une image dessinée montre d'une scène du jeu : le pas jusqu'où chaque nœud est au
/// moins à sa place (`None` : l'écran du commun), et si c'est une image à mi-chemin d'un
/// glisser.
pub(crate) type Montre = (Option<u64>, bool);

/// Glisser : automatique (à 100 Hz et plus), forcé, ou jamais (le banc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModeGlisser {
    /// D'après la période mesurée des images.
    Auto,
    /// Toujours (`?glisser=oui`, le banc).
    Oui,
    /// Jamais (`?glisser=non`, le banc).
    Non,
}

/// Ce que la page mesure et montre de l'entrée au pixel.
pub(crate) struct Mesure {
    banc: bool,
    numero: u64,
    sources: VecDeque<(u64, Source)>,
    /// La dernière scène reçue du Worker : son pas, et quand.
    scene: Option<(u64, u64)>,
    /// Une scène est arrivée depuis les dernières traces.
    scene_neuve: bool,
    attente_dessin: VecDeque<Trace>,
    attente_gpu: VecDeque<(Trace, Arc<AtomicU64>)>,
    /// La dernière image où chaque nœud d'une scène du jeu est au moins à sa place d'un
    /// pas (la scène de ce pas en entier, ou un chemin de glisser qui en part) : ce pas,
    /// son début, son dessin soumis, et la case de son GPU fini. Le Worker poste la scène et les traces
    /// en deux messages : Chrome peut placer une image entre les deux, et la trace se
    /// rattache alors à cette image, déjà faite (constaté au lot 13).
    dernier_dessin: Option<(u64, u64, u64, Arc<AtomicU64>)>,
    diagnostic: Diagnostic,
    /// Le dernier pas définitif que le Worker a dit (`Battement`), et celui de l'image
    /// précédente : les pas par image du panneau.
    definitif: Option<u64>,
    pas_de_l_image: Option<u64>,
    glyphes_absents: u64,
    /// La touche du panneau, que le jeu déclare.
    touche: Option<CodeTouche>,
    ouvert: bool,
    carre_jusqu_a_ms: f64,
    panneau_ecrit_ms: f64,
    panneau: Element,
    carre: Element,
    /// Glisser.
    pub(crate) mode: ModeGlisser,
    periodes: VecDeque<f64>,
    derniere_image_ms: f64,
    /// La scène d'avant la dernière et son pas, quand glisser de l'une à l'autre déplace
    /// un nœud ; et quand la dernière est arrivée à l'écran.
    precedente: Option<(SceneFil, u64)>,
    depuis_ms: f64,
    /// Les scènes où un nœud a glissé, et les images dessinées à mi-chemin : le banc les
    /// lit avant et après chaque condition (`window.__banc.glisses()`), pour dire si la
    /// condition « avec glisser » a vraiment glissé.
    scenes_glissees: u64,
    images_a_mi_chemin: u64,
    /// Les lignes pour le banc, que la page écrit ensuite.
    au_banc: Vec<String>,
    /// Le banc seulement : chaque dessin attend ce temps, en ms, avant de commencer (le
    /// contrôle négatif de `mesures` : le segment du dessin doit le montrer).
    pub(crate) dessin_lent_ms: f64,
}

/// Deux éléments du commun par-dessus tout : le panneau (un `<pre>`), le carré blanc.
fn element(document: &Document, corps: &HtmlElement, balise: &str) -> Result<Element, String> {
    let e = document
        .create_element(balise)
        .map_err(|_| String::from("un élément du diagnostic"))?;
    corps
        .append_child(&e)
        .map_err(|_| String::from("un élément du diagnostic"))?;
    Ok(e)
}

impl Mesure {
    /// Le panneau fermé, le carré éteint.
    pub(crate) fn creer(
        document: &Document,
        corps: &HtmlElement,
        banc: Oui,
        mode: ModeGlisser,
    ) -> Result<Mesure, String> {
        let panneau = element(document, corps, "pre")?;
        style(
            &panneau,
            &[
                ("position", "fixed"),
                ("left", "8px"),
                ("top", "8px"),
                ("margin", "0"),
                ("padding", "8px"),
                ("max-width", "60vw"),
                ("max-height", "90vh"),
                ("overflow", "hidden"),
                ("background", "rgba(0, 0, 0, 0.8)"),
                ("color", "#fff"),
                ("font", "12px/1.3 monospace"),
                ("z-index", "8"),
                ("display", "none"),
                ("pointer-events", "none"),
            ],
        );
        let carre = element(document, corps, "div")?;
        style(
            &carre,
            &[
                ("position", "fixed"),
                ("right", "0"),
                ("bottom", "0"),
                ("width", "48px"),
                ("height", "48px"),
                ("background", "#fff"),
                ("z-index", "8"),
                ("display", "none"),
                ("pointer-events", "none"),
            ],
        );
        Ok(Mesure {
            banc: banc == Oui::Oui,
            numero: 0,
            sources: VecDeque::new(),
            scene: None,
            scene_neuve: false,
            attente_dessin: VecDeque::new(),
            attente_gpu: VecDeque::new(),
            dernier_dessin: None,
            diagnostic: Diagnostic::new(),
            definitif: None,
            pas_de_l_image: None,
            glyphes_absents: 0,
            touche: None,
            ouvert: false,
            carre_jusqu_a_ms: 0.0,
            panneau_ecrit_ms: f64::NEG_INFINITY,
            panneau,
            carre,
            mode,
            periodes: VecDeque::new(),
            derniere_image_ms: f64::NAN,
            precedente: None,
            depuis_ms: 0.0,
            scenes_glissees: 0,
            images_a_mi_chemin: 0,
            au_banc: Vec::new(),
            dessin_lent_ms: 0.0,
        })
    }

    /// Le jeu chargé déclare sa touche du panneau.
    pub(crate) const fn nouveau_jeu(&mut self, touche: CodeTouche) {
        self.touche = Some(touche);
    }

    /// Cette touche ouvre-t-elle ou ferme-t-elle le panneau ?
    pub(crate) fn est_la_touche(&self, code: &str) -> bool {
        self.touche.as_ref().is_some_and(|t| t.as_str() == code)
    }

    /// Ouvre ou ferme le panneau.
    pub(crate) fn basculer(&mut self) {
        self.ouvert = !self.ouvert;
        self.panneau_ecrit_ms = f64::NEG_INFINITY;
        style(
            &self.panneau,
            &[("display", if self.ouvert { "block" } else { "none" })],
        );
        if !self.ouvert {
            style(&self.carre, &[("display", "none")]);
        }
    }

    /// Le numéro de la prochaine entrée de l'écran, de cette source ; le carré s'allume
    /// si le panneau est ouvert.
    pub(crate) fn numeroter(&mut self, source: Source, maintenant_ms: f64) -> u64 {
        self.numero = self.numero.saturating_add(1);
        if self.sources.len() >= SOURCES_RETENUES {
            let _: Option<(u64, Source)> = self.sources.pop_front();
        }
        self.sources.push_back((self.numero, source));
        if self.ouvert {
            self.carre_jusqu_a_ms = maintenant_ms + CARRE_MS;
            style(&self.carre, &[("display", "block")]);
        }
        self.numero
    }

    /// Une scène reçue du Worker.
    pub(crate) const fn scene_recue(&mut self, pas: u64, t_us: u64) {
        self.scene = Some((pas, t_us));
        self.scene_neuve = true;
    }

    fn source(&self, t: &Trace) -> Source {
        match t.id {
            IdTrace::Ecran(n) => self
                .sources
                .iter()
                .find(|(k, _)| *k == n)
                .map_or(Source::Clavier, |(_, s)| *s),
            IdTrace::Appareil { appareil, .. } => Source::Telephone(appareil.fil()),
        }
    }

    /// Des traces du Worker, juste après la scène de leur tour s'il y en a une : elles
    /// attendent l'image qui dessine cette scène ; sans scène neuve (la vue n'a pas
    /// changé), elles n'ont pas de pixel, et se versent telles quelles.
    pub(crate) fn traces(&mut self, v: Vec<Trace>) {
        let scene = self.scene.filter(|_| self.scene_neuve);
        self.scene_neuve = false;
        for t in v {
            match scene {
                Some((pas, recue)) if pas >= t.place.numero() => {
                    // La scène est-elle déjà dessinée, depuis son arrivée ?
                    let fait = self
                        .dernier_dessin
                        .as_ref()
                        .filter(|(p, image, _, _)| *p >= t.place.numero() && *image >= recue);
                    if let Some((_, image_us, soumis_us, fini)) = fait {
                        let suivie = Trace {
                            page_recue_us: recue,
                            image_us: *image_us,
                            soumis_us: *soumis_us,
                            ..t
                        };
                        let fini = Arc::clone(fini);
                        self.attendre_le_gpu(suivie, fini);
                        continue;
                    }
                    if self.attente_dessin.len() >= TRACES_EN_ATTENTE {
                        let _: Option<Trace> = self.attente_dessin.pop_front();
                    }
                    self.attente_dessin.push_back(Trace {
                        page_recue_us: recue,
                        ..t
                    });
                }
                Some(_) | None => self.verser(t),
            }
        }
    }

    /// Une image vient d'être dessinée et soumise à `soumis_us` : chaque nœud y est au
    /// moins à sa place du pas `au_moins` (la scène de ce pas dessinée en entier, ou un
    /// chemin de glisser qui part de la scène de ce pas, `mi_chemin`) ; `None` pour l'écran
    /// du commun, ou pendant un chemin qui part d'une scène sans pas connu. Chaque trace
    /// qui attendait ce pas, ou un plus ancien, attend maintenant le GPU.
    pub(crate) fn dessine(
        &mut self,
        (au_moins, mi_chemin): Montre,
        (image_us, soumis_us): (u64, u64),
        fini: &Arc<AtomicU64>,
        glyphes_absents: u32,
    ) {
        self.glyphes_absents = u64::from(glyphes_absents);
        if mi_chemin {
            self.images_a_mi_chemin = self.images_a_mi_chemin.saturating_add(1);
        }
        let Some(pas) = au_moins else {
            return;
        };
        self.dernier_dessin = Some((pas, image_us, soumis_us, Arc::clone(fini)));
        let (pretes, restent): (VecDeque<Trace>, VecDeque<Trace>) =
            core::mem::take(&mut self.attente_dessin)
                .into_iter()
                .partition(|t| t.place.numero() <= pas);
        self.attente_dessin = restent;
        for t in pretes {
            let suivie = Trace {
                image_us,
                soumis_us,
                ..t
            };
            self.attendre_le_gpu(suivie, Arc::clone(fini));
        }
    }

    /// Une trace dessinée attend la fin du travail de son GPU.
    fn attendre_le_gpu(&mut self, t: Trace, fini: Arc<AtomicU64>) {
        if self.attente_gpu.len() >= TRACES_EN_ATTENTE {
            let _: Option<(Trace, Arc<AtomicU64>)> = self.attente_gpu.pop_front();
        }
        self.attente_gpu.push_back((t, fini));
    }

    /// Faut-il faire tourner l'appareil du GPU (le repli WebGL2 relève ses rappels au
    /// `poll`) ?
    pub(crate) fn attend_le_gpu(&self) -> bool {
        !self.attente_gpu.is_empty()
    }

    /// À chaque image : les traces dont le GPU a fini se versent ; le carré s'éteint ; le
    /// panneau se réécrit.
    pub(crate) fn chaque_image(&mut self, (origine_ms, maintenant_ms): (f64, f64), moteur: &str) {
        while let Some((_, fini)) = self.attente_gpu.front() {
            let f = f64::from_bits(fini.load(Ordering::SeqCst));
            if f <= 0.0 {
                break;
            }
            let Some((t, _)) = self.attente_gpu.pop_front() else {
                break;
            };
            let gpu = (origine_ms + f) * 1000.0;
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "un instant positif en microsecondes, l'heure du navigateur ; rien ne revient à une partie"
            )]
            let gpu_fini_us = if gpu > 0.0 { gpu as u64 } else { 0 };
            self.verser(Trace { gpu_fini_us, ..t });
        }
        if self.carre_jusqu_a_ms > 0.0 && maintenant_ms >= self.carre_jusqu_a_ms {
            self.carre_jusqu_a_ms = 0.0;
            style(&self.carre, &[("display", "none")]);
        }
        if self.ouvert && maintenant_ms - self.panneau_ecrit_ms >= PANNEAU_MS {
            self.panneau_ecrit_ms = maintenant_ms;
            let texte = self.texte(moteur);
            self.panneau.set_text_content(Some(&texte));
        }
    }

    /// Le texte du panneau.
    pub(crate) fn texte(&self, moteur: &str) -> String {
        let mut t = self.diagnostic.texte(moteur, self.glyphes_absents);
        if let Some((m, n)) = self.periode() {
            let _: core::fmt::Result =
                writeln!(t, "images : période médiane {m:.1} ms ({n} dernières)");
        }
        t.push_str(match self.mode {
            ModeGlisser::Auto if self.glisse() => "glisser : oui (100 Hz ou plus)",
            ModeGlisser::Auto => "glisser : non (moins de 100 Hz)",
            ModeGlisser::Oui => "glisser : forcé",
            ModeGlisser::Non => "glisser : jamais",
        });
        let _: core::fmt::Result = writeln!(
            t,
            " ; {} scène(s) où un nœud a glissé, {} image(s) à mi-chemin",
            self.scenes_glissees, self.images_a_mi_chemin
        );
        t
    }

    /// Les lignes pour le banc depuis le dernier appel.
    pub(crate) fn au_banc(&mut self) -> Vec<String> {
        core::mem::take(&mut self.au_banc)
    }

    fn verser(&mut self, t: Trace) {
        let source = self.source(&t);
        if self.banc {
            let (id, appareil) = match t.id {
                IdTrace::Ecran(n) => (n, None),
                IdTrace::Appareil { appareil, numero } => (numero, Some(appareil.fil())),
            };
            let sorte = match source {
                Source::Clavier => "clavier",
                Source::Souris => "souris",
                Source::Telephone(_) => "telephone",
            };
            self.au_banc.push(format!(
                "{{\"quoi\":\"trace\",\"source\":\"{sorte}\",\"id\":{id},\"appareil\":{},\"place\":{},\"evenement\":{},\"envoi\":{},\"recu\":{},\"pas_debut\":{},\"pas_fin\":{},\"vue_fin\":{},\"page_recue\":{},\"image\":{},\"soumis\":{},\"gpu_fini\":{},\"incertitude\":{}}}",
                appareil.map_or_else(|| String::from("null"), |a| a.to_string()),
                t.place.numero(),
                t.evenement_us,
                t.envoi_us,
                t.recu_us,
                t.pas_debut_us,
                t.pas_fin_us,
                t.vue_fin_us,
                t.page_recue_us,
                t.image_us,
                t.soumis_us,
                t.gpu_fini_us,
                t.incertitude_us
            ));
        }
        self.diagnostic.noter(source, t);
    }

    /// Le Worker a joué jusqu'au pas `pas` (son battement).
    pub(crate) const fn battement(&mut self, pas: u64) {
        self.definitif = Some(pas);
    }

    /// La médiane des dernières périodes d'image, en ms, et leur nombre.
    fn periode(&self) -> Option<(f64, usize)> {
        let mut v: Vec<f64> = self.periodes.iter().copied().collect();
        v.sort_by(f64::total_cmp);
        v.get(v.len() / 2).map(|m| (*m, v.len()))
    }

    /// Une image commence, à `maintenant_ms` : sa période se note (la médiane des 30
    /// dernières dit si l'écran va à 120 Hz), et les pas que le Worker a joués depuis
    /// l'image précédente (le panneau).
    pub(crate) fn image(&mut self, maintenant_ms: f64) {
        if let Some(d) = self.definitif {
            if let Some(avant) = self.pas_de_l_image {
                self.diagnostic.noter_une_image(d.saturating_sub(avant));
            }
            self.pas_de_l_image = Some(d);
        }
        let d = maintenant_ms - self.derniere_image_ms;
        self.derniere_image_ms = maintenant_ms;
        if d.is_finite() && d > 0.0 && d < 100.0 {
            if self.periodes.len() >= 30 {
                let _: Option<f64> = self.periodes.pop_front();
            }
            self.periodes.push_back(d);
        }
    }

    /// La télé glisse-t-elle ?
    pub(crate) fn glisse(&self) -> bool {
        match self.mode {
            ModeGlisser::Oui => true,
            ModeGlisser::Non => false,
            ModeGlisser::Auto => self
                .periode()
                .is_some_and(|(m, n)| n >= 10 && m < PERIODE_DE_GLISSER_MS),
        }
    }

    /// Une scène neuve, `derniere`, arrive à l'écran ; `avant` est celle qu'elle remplace,
    /// avec son pas (donnée quand la télé glisse seulement). Elle n'est gardée que si
    /// glisser de l'une à l'autre déplace un nœud : sinon, aucun chemin, et la dernière
    /// scène se dessine en entier dès l'image suivante.
    pub(crate) fn scene_a_l_ecran(
        &mut self,
        avant: Option<(SceneFil, u64)>,
        derniere: &SceneFil,
        maintenant_ms: f64,
    ) {
        self.precedente = avant.filter(|(a, _)| rendu::glisser::deplace_des_noeuds(a, derniere));
        if self.precedente.is_some() {
            self.scenes_glissees = self.scenes_glissees.saturating_add(1);
        }
        self.depuis_ms = maintenant_ms;
    }

    /// Les scènes où un nœud a glissé, et les images dessinées à mi-chemin, depuis le
    /// début (le banc).
    pub(crate) const fn glisses(&self) -> (u64, u64) {
        (self.scenes_glissees, self.images_a_mi_chemin)
    }

    /// Où en est le glisser à `maintenant_ms` : la scène d'avant, son pas, et la fraction
    /// du chemin, en millièmes ; `None` quand la dernière scène se dessine en entier.
    pub(crate) fn chemin(&self, maintenant_ms: f64) -> Option<(&SceneFil, u64, u32)> {
        let (p, pas) = self.precedente.as_ref().map(|(s, n)| (s, *n))?;
        let f = (maintenant_ms - self.depuis_ms) / PAS_MS * 1000.0;
        if !f.is_finite() || f >= f64::from(rendu::glisser::PLEIN) {
            return None;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "une fraction positive, en millièmes, sous 1 000"
        )]
        let m = if f > 0.0 { f as u32 } else { 0 };
        Some((p, pas, m))
    }
}
