//! Les traces de l'entrée au pixel, découpées par segment (PHASE-3, § 3 et § 8.4 ;
//! lot 13) : ce que le panneau de diagnostic montre, et ce que `soiree-banc mesures`
//! rapporte (M2). Sans entrées-sorties : la page et le banc y versent les traces, le
//! texte du panneau en sort.
//!
//! Une [`Trace`] porte les instants d'une entrée, dans la base `timeOrigin + now()` de
//! chaque contexte, en microsecondes : l'événement et l'envoi (la page de l'écran, ou le
//! téléphone qui les ramène à l'heure de l'autorité par écho), la réception, le pas et
//! les vues (le Worker), la scène reçue, le dessin soumis et le travail du GPU fini (la
//! page). **Un segment qui franchit deux contextes porte l'écart de leurs horloges**
//! (M14) : sa durée est signée, et peut être négative.
//!
//! **Le « pixel » est la fin du travail du GPU**, pas le photon (OUT-88) ; la part du
//! matériel (la dalle tactile, la télé) se mesure à part, à la caméra, et le panneau la
//! garde sur une ligne à part (§ 2.9).

use std::collections::{BTreeMap, VecDeque};
use std::fmt::Write as _;

use crate::ecran::{IdTrace, Trace};

/// Les gestes que le panneau garde par source (§ 2.9 : « les 200 derniers gestes »).
pub const GESTES_PAR_SOURCE: usize = 200;

/// Le budget du CONTRAT au 99ᵉ centile, depuis un clavier, en microsecondes (« 99 % des
/// gestes en moins de 50 ms », le matériel à part).
pub const BUDGET_CLAVIER_US: i64 = 50_000;

/// Le budget du CONTRAT au 99ᵉ centile, depuis un téléphone en Wi-Fi local (80 ms).
pub const BUDGET_TELEPHONE_US: i64 = 80_000;

/// Un segment du chemin d'une entrée (§ 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Segment {
    /// De l'événement à son envoi : le gestionnaire de la page de l'écran, ou la page du
    /// téléphone.
    Evenement,
    /// De l'envoi à la réception par l'autorité : `postMessage` pour l'écran ; le
    /// chiffrement, l'envoi, le relais et la boucle locale pour un téléphone.
    Transport,
    /// De la réception au début du tour qui joue son pas : l'attente du pas (P6).
    Attente,
    /// Le pas joué (`avancer_borne`).
    Pas,
    /// Les vues.
    Vues,
    /// Des vues à la scène reçue par la page.
    VersLaPage,
    /// De la scène reçue au début de l'image qui la dessine.
    AttenteImage,
    /// Du début de l'image au dessin soumis.
    Dessin,
    /// Du dessin soumis au travail du GPU fini.
    Gpu,
    /// De l'événement au travail du GPU fini.
    Total,
}

/// Les segments, dans l'ordre du chemin, puis le total.
pub const SEGMENTS: [Segment; 10] = [
    Segment::Evenement,
    Segment::Transport,
    Segment::Attente,
    Segment::Pas,
    Segment::Vues,
    Segment::VersLaPage,
    Segment::AttenteImage,
    Segment::Dessin,
    Segment::Gpu,
    Segment::Total,
];

impl Segment {
    /// Son nom, au panneau et au rapport du banc.
    pub const fn nom(self) -> &'static str {
        match self {
            Segment::Evenement => "evenement",
            Segment::Transport => "transport",
            Segment::Attente => "attente",
            Segment::Pas => "pas",
            Segment::Vues => "vues",
            Segment::VersLaPage => "vers-la-page",
            Segment::AttenteImage => "attente-image",
            Segment::Dessin => "dessin",
            Segment::Gpu => "gpu",
            Segment::Total => "total",
        }
    }

    /// Ses deux bornes dans la trace : (début, fin).
    const fn bornes(self, t: &Trace) -> (u64, u64) {
        match self {
            Segment::Evenement => (t.evenement_us, t.envoi_us),
            Segment::Transport => (t.envoi_us, t.recu_us),
            Segment::Attente => (t.recu_us, t.pas_debut_us),
            Segment::Pas => (t.pas_debut_us, t.pas_fin_us),
            Segment::Vues => (t.pas_fin_us, t.vue_fin_us),
            Segment::VersLaPage => (t.vue_fin_us, t.page_recue_us),
            Segment::AttenteImage => (t.page_recue_us, t.image_us),
            Segment::Dessin => (t.image_us, t.soumis_us),
            Segment::Gpu => (t.soumis_us, t.gpu_fini_us),
            Segment::Total => (t.evenement_us, t.gpu_fini_us),
        }
    }
}

/// La durée d'un segment d'une trace, en microsecondes, signée ; `None` si l'une de ses
/// bornes manque (0 : une entrée dont la scène n'a pas changé n'a ni scène reçue, ni
/// image, ni dessin, ni travail du GPU).
pub fn duree(t: &Trace, s: Segment) -> Option<i64> {
    let (debut, fin) = s.bornes(t);
    if debut == 0 || fin == 0 {
        return None;
    }
    let (d, f) = (i64::try_from(debut).ok()?, i64::try_from(fin).ok()?);
    f.checked_sub(d)
}

/// La trace a-t-elle son pixel (la fin du travail du GPU) ?
pub const fn a_son_pixel(t: &Trace) -> bool {
    t.gpu_fini_us != 0
}

/// Une distribution : le compte, le plus petit, la médiane, les 95ᵉ et 99ᵉ centiles et le
/// plus grand ; jamais une moyenne (§ 8.4). Les centiles sont au rang le plus proche
/// (`⌈p·n/100⌉`, sans interpolation) : une valeur vraiment observée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distribution {
    /// Le nombre de valeurs.
    pub n: usize,
    /// La plus petite.
    pub min: i64,
    /// La médiane (le rang `⌈n/2⌉`).
    pub mediane: i64,
    /// Le 95ᵉ centile.
    pub c95: i64,
    /// Le 99ᵉ centile.
    pub c99: i64,
    /// La plus grande.
    pub max: i64,
}

/// Le centile `p` (de 1 à 100) de valeurs triées, au rang le plus proche.
fn centile(triees: &[i64], p: usize) -> Option<i64> {
    let n = triees.len();
    let rang = n.checked_mul(p)?.div_ceil(100);
    triees.get(rang.checked_sub(1)?).copied()
}

/// La distribution de ces valeurs ; `None` s'il n'y en a aucune.
pub fn distribution(valeurs: &mut [i64]) -> Option<Distribution> {
    valeurs.sort_unstable();
    Some(Distribution {
        n: valeurs.len(),
        min: *valeurs.first()?,
        mediane: centile(valeurs, 50)?,
        c95: centile(valeurs, 95)?,
        c99: centile(valeurs, 99)?,
        max: *valeurs.last()?,
    })
}

/// D'où vient un geste, au panneau (§ 2.9 : « par source (clavier, souris, chaque
/// téléphone) »).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// Une touche de l'écran.
    Clavier,
    /// Un clic, un glisser ou un composant de l'écran.
    Souris,
    /// Un téléphone, par son numéro (`Appareil::fil`).
    Telephone(u64),
}

impl Source {
    /// La source d'une trace de l'écran, selon ce que la page en sait.
    pub const fn d_un_appareil(t: &Trace) -> Option<Source> {
        match t.id {
            IdTrace::Appareil { appareil, .. } => Some(Source::Telephone(appareil.fil())),
            IdTrace::Ecran(_) => None,
        }
    }

    /// Le budget du CONTRAT au 99ᵉ centile pour cette source.
    pub const fn budget_us(self) -> i64 {
        match self {
            Source::Clavier | Source::Souris => BUDGET_CLAVIER_US,
            Source::Telephone(_) => BUDGET_TELEPHONE_US,
        }
    }
}

/// Ce que le panneau garde : les derniers gestes de chaque source, l'histogramme des pas
/// par image, et ce qu'aucune trace ne dit.
#[derive(Debug, Clone, Default)]
pub struct Diagnostic {
    gestes: BTreeMap<Source, VecDeque<Trace>>,
    /// Les images dessinées selon le nombre de pas depuis la précédente : 0, 1, 2, 3, et
    /// 4 ou plus.
    pas_par_image: [u64; 5],
    /// Les entrées dont la scène n'a pas changé : elles n'ont pas de pixel.
    sans_pixel: u64,
}

impl Diagnostic {
    /// Un panneau vide.
    pub fn new() -> Diagnostic {
        Diagnostic::default()
    }

    /// Note une trace complète. Au-delà de [`GESTES_PAR_SOURCE`] gestes d'une source, le
    /// plus ancien s'oublie.
    pub fn noter(&mut self, source: Source, t: Trace) {
        if !a_son_pixel(&t) {
            self.sans_pixel = self.sans_pixel.saturating_add(1);
        }
        let v = self.gestes.entry(source).or_default();
        if v.len() >= GESTES_PAR_SOURCE {
            let _: Option<Trace> = v.pop_front();
        }
        v.push_back(t);
    }

    /// Note une image dessinée, `pas` pas après la précédente.
    pub fn noter_une_image(&mut self, pas: u64) {
        let k = usize::try_from(pas).map_or(4, |p| p.min(4));
        if let Some(c) = self.pas_par_image.get_mut(k) {
            *c = c.saturating_add(1);
        }
    }

    /// Les sources vues, dans leur ordre.
    pub fn sources(&self) -> Vec<Source> {
        self.gestes.keys().copied().collect()
    }

    /// La distribution d'un segment pour une source, sur ses gestes gardés.
    pub fn distribution(&self, source: Source, s: Segment) -> Option<Distribution> {
        let mut v: Vec<i64> = self
            .gestes
            .get(&source)?
            .iter()
            .filter_map(|t| duree(t, s))
            .collect();
        distribution(&mut v)
    }

    /// Le texte du panneau (§ 2.9) : par source, les 50ᵉ, 95ᵉ et 99ᵉ centiles de chaque
    /// segment, en millisecondes, et la ligne du budget ; l'histogramme des pas par
    /// image ; le moteur de dessin ; les glyphes absents ; une ligne à part pour le
    /// matériel, jamais mêlée au reste. Le même texte part au banc.
    pub fn texte(&self, moteur: &str, glyphes_absents: u64) -> String {
        let mut s = String::from("Diagnostic de la soirée (F8 : une marque au journal)\n");
        for source in self.sources() {
            let n = self.gestes.get(&source).map_or(0, VecDeque::len);
            let nom = match source {
                Source::Clavier => String::from("clavier"),
                Source::Souris => String::from("souris"),
                Source::Telephone(a) => format!("téléphone {a}"),
            };
            let _: core::fmt::Result = writeln!(s, "{nom} : {n} gestes (50ᵉ ; 95ᵉ ; 99ᵉ, ms)");
            for seg in SEGMENTS {
                if let Some(d) = self.distribution(source, seg) {
                    let _: core::fmt::Result = writeln!(
                        s,
                        "  {:<13}{} ; {} ; {}",
                        seg.nom(),
                        ms(d.mediane),
                        ms(d.c95),
                        ms(d.c99)
                    );
                }
            }
            if let Some(d) = self.distribution(source, Segment::Total) {
                let budget = source.budget_us();
                let _: core::fmt::Result = writeln!(
                    s,
                    "  budget       99 % sous {} ms : {}",
                    ms(budget),
                    if d.c99 <= budget { "tenu" } else { "DÉPASSÉ" }
                );
            }
        }
        let [p0, p1, p2, p3, p4] = self.pas_par_image;
        let _: core::fmt::Result = writeln!(
            s,
            "pas par image : 0 → {p0} ; 1 → {p1} ; 2 → {p2} ; 3 → {p3} ; 4 et plus → {p4}"
        );
        let _: core::fmt::Result = writeln!(
            s,
            "entrées sans pixel (scène inchangée) : {}",
            self.sans_pixel
        );
        let _: core::fmt::Result = writeln!(s, "moteur de dessin : {moteur}");
        let _: core::fmt::Result = writeln!(s, "glyphes absents : {glyphes_absents}");
        s.push_str(
            "matériel (dalle, télé, enceintes) : à part, mesuré à la répétition par une caméra ; non mesuré ici\n",
        );
        s
    }
}

/// Des microsecondes en millisecondes, au dixième, pour le texte.
fn ms(us: i64) -> String {
    let signe = if us < 0 { "-" } else { "" };
    let a = us.unsigned_abs();
    format!("{signe}{}.{}", a / 1000, (a % 1000) / 100)
}
