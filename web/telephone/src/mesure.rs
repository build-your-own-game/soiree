//! La mesure du vrai Wi-Fi, côté téléphone (PHASE-3, § 8.5, M5 ; lot 14), sans
//! navigateur : ce que la page `/mesure` compte, et le bilan qu'elle envoie au relais
//! (`relais::mesure`).
//!
//! La page envoie un écho toutes les [`PERIODE_ECHO_MS`] (16 octets : un numéro et son
//! heure) ; le relais le renvoie suivi de son heure, et envoie de lui-même un tic de son
//! heure tous les 16,7 ms. D'où :
//!
//! - **l'aller-retour** de chaque écho, en distribution (les centiles au rang le plus
//!   proche, sans moyenne, comme `salle::trace`), et les échos **en rafale** : revenus
//!   après plus de [`RAFALE_MS`] (la `RAFALE` du monde simulé de l'épreuve, à remplacer
//!   par cette mesure) ;
//! - **les écarts entre deux tics reçus** (la descente : ce que vivent les vues d'un jeu),
//!   et les **trous** de plus de [`RAFALE_MS`] ;
//! - **la dérive de l'horloge** du téléphone contre celle du relais, en millionièmes, par
//!   les moindres carrés sur les échos les plus rapides (le quart le plus court : un écho
//!   lent dit mal l'heure) ; l'écart absolu ne veut rien dire (les deux horloges n'ont pas
//!   la même origine) ;
//! - **l'arrière-plan** : combien de fois la page a été cachée, après combien de secondes
//!   la connexion s'est fermée (ou combien de secondes elle est restée ouverte), les
//!   coupures et les reconnexions.
//!
//! Les échos perdus : une WebSocket est sur TCP, rien ne s'y perd sans que la connexion
//! tombe ; un écho envoyé sans réponse avant une coupure compte comme perdu.

#![expect(
    clippy::cast_precision_loss,
    reason = "des comptes et des heures en microsecondes ramenés en f64 pour une distribution : exacts sous 2⁵³ (285 ans) ; rien ne revient à une partie"
)]

use std::collections::VecDeque;
use std::fmt::Write as _;

use jeu_abi::Oui;

/// Un écho toutes les 50 ms (choisi : plus souvent que la redite d'un tenu, 150 ms chez
/// Sidequest ; quinze téléphones font 300 messages par seconde, peu pour un Wi-Fi).
pub const PERIODE_ECHO_MS: u32 = 50;

/// Un bilan toutes les 10 s (choisi).
pub const PERIODE_BILAN_MS: u32 = 10_000;

/// Un écho revenu après plus de 100 ms, ou un trou de plus de 100 ms entre deux tics, est
/// une rafale (la `RAFALE` du monde simulé, `epreuve/src/telephone.rs`, que M5 remplacera).
pub const RAFALE_MS: f64 = 100.0;

/// Les mesures gardées de chaque sorte : 36 000 (choisi : 30 min d'échos à 20 par
/// seconde ; au-delà, la plus ancienne s'oublie). Les compteurs, eux, comptent tout.
pub const GARDEES_MAX: usize = 36_000;

/// La dérive ne se dit que sur des échos étalés sur au moins une minute (choisi).
pub const DERIVE_ETENDUE_MIN_MS: f64 = 60_000.0;

/// Le nom du navigateur gardé au plus (choisi).
pub const NAVIGATEUR_MAX: usize = 160;

/// Une distribution : le nombre, et les centiles au rang le plus proche, en millisecondes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Centiles {
    /// Le nombre de valeurs.
    pub n: usize,
    /// La médiane.
    pub p50: f64,
    /// Le 95ᵉ centile.
    pub p95: f64,
    /// Le 99ᵉ centile.
    pub p99: f64,
    /// Le maximum.
    pub max: f64,
}

/// Les centiles au rang le plus proche (`salle::trace` fait de même), ou rien sans valeur.
pub fn centiles(valeurs: &VecDeque<f64>) -> Option<Centiles> {
    let mut v: Vec<f64> = valeurs.iter().copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let rang = |p: usize| -> f64 {
        let n = v.len();
        // Le rang le plus proche : ⌈p × n / 100⌉, de 1 à n.
        let r = p.saturating_mul(n).div_ceil(100).clamp(1, n);
        v.get(r.saturating_sub(1)).copied().unwrap_or(0.0)
    };
    Some(Centiles {
        n: v.len(),
        p50: rang(50),
        p95: rang(95),
        p99: rang(99),
        max: v.last().copied().unwrap_or(0.0),
    })
}

/// La pente des moindres carrés de `y` sur `x` (des microsecondes), en millionièmes ; rien
/// sous deux points ou sans étendue de `x`.
pub fn derive_ppm(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 2 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut sxy, mut sxx) = (0.0, 0.0);
    for (x, y) in points {
        sxy += (x - mx) * (y - my);
        sxx += (x - mx) * (x - mx);
    }
    (sxx > 0.0).then(|| sxy / sxx * 1e6)
}

fn garder(v: &mut VecDeque<f64>, x: f64) {
    v.push_back(x);
    while v.len() > GARDEES_MAX {
        v.pop_front();
    }
}

/// Un écho revenu, pour l'horloge : l'heure du téléphone au milieu de l'aller-retour, et
/// l'écart de l'heure du relais à elle, en microsecondes ; et l'aller-retour.
#[derive(Debug, Clone, Copy)]
struct Heure {
    milieu_us: f64,
    ecart_us: f64,
    aller_retour_ms: f64,
}

/// Ce que la page de mesure a compté, depuis son ouverture.
#[derive(Debug, Default)]
pub struct Mesure {
    debut_ms: Option<f64>,
    envoyes: u64,
    recus: u64,
    /// Les échos envoyés sur la connexion en cours, sans réponse encore.
    en_vol: u64,
    perdus: u64,
    aller_retour_ms: VecDeque<f64>,
    rafales_montantes: u64,
    tics: u64,
    dernier_tic_ms: Option<f64>,
    ecarts_tics_ms: VecDeque<f64>,
    trous: u64,
    heures: VecDeque<Heure>,
    caches: u32,
    cache_depuis_ms: Option<f64>,
    fermee_apres_s: Vec<f64>,
    restee_ouverte_s: Vec<f64>,
    ouverte: bool,
    coupures: u32,
    reconnexions: u32,
}

impl Mesure {
    /// Une mesure qui commence à `t_ms` (l'heure du téléphone).
    pub fn new(t_ms: f64) -> Mesure {
        Mesure {
            debut_ms: Some(t_ms),
            ..Mesure::default()
        }
    }

    /// La connexion s'ouvre (la première, ou une reconnexion).
    pub fn ouverte(&mut self) {
        if self.coupures > 0 {
            self.reconnexions = self.reconnexions.saturating_add(1);
        }
        self.ouverte = true;
        self.en_vol = 0;
        self.dernier_tic_ms = None;
    }

    /// La connexion se ferme à `t_ms` : les échos en vol sont perdus ; si la page était
    /// cachée, la fermeture est celle de l'arrière-plan.
    pub fn fermee(&mut self, t_ms: f64) {
        if !self.ouverte {
            return;
        }
        self.ouverte = false;
        self.coupures = self.coupures.saturating_add(1);
        self.perdus = self.perdus.saturating_add(self.en_vol);
        self.en_vol = 0;
        if let Some(c) = self.cache_depuis_ms.take() {
            self.fermee_apres_s.push((t_ms - c) / 1000.0);
        }
    }

    /// Un écho part.
    pub fn echo_envoye(&mut self) {
        self.envoyes = self.envoyes.saturating_add(1);
        self.en_vol = self.en_vol.saturating_add(1);
    }

    /// Un écho revient : envoyé à `envoi_ms`, revenu à `retour_ms` (l'heure du téléphone),
    /// avec l'heure du relais `relais_us`.
    pub fn echo_recu(&mut self, envoi_ms: f64, retour_ms: f64, relais_us: u64) {
        self.recus = self.recus.saturating_add(1);
        self.en_vol = self.en_vol.saturating_sub(1);
        let ar = retour_ms - envoi_ms;
        if !ar.is_finite() || ar < 0.0 {
            return;
        }
        garder(&mut self.aller_retour_ms, ar);
        if ar > RAFALE_MS {
            self.rafales_montantes = self.rafales_montantes.saturating_add(1);
        }
        let milieu_us = f64::midpoint(envoi_ms, retour_ms) * 1000.0;
        self.heures.push_back(Heure {
            milieu_us,
            ecart_us: relais_us as f64 - milieu_us,
            aller_retour_ms: ar,
        });
        while self.heures.len() > GARDEES_MAX {
            self.heures.pop_front();
        }
    }

    /// Un tic reçu à `t_ms`.
    pub fn tic(&mut self, t_ms: f64) {
        self.tics = self.tics.saturating_add(1);
        if let Some(d) = self.dernier_tic_ms {
            let ecart = t_ms - d;
            garder(&mut self.ecarts_tics_ms, ecart);
            if ecart > RAFALE_MS {
                self.trous = self.trous.saturating_add(1);
            }
        }
        self.dernier_tic_ms = Some(t_ms);
    }

    /// La page se cache à `t_ms`.
    pub fn cachee(&mut self, t_ms: f64) {
        self.caches = self.caches.saturating_add(1);
        if self.ouverte {
            self.cache_depuis_ms = Some(t_ms);
        }
    }

    /// La page revient à `t_ms` : si la connexion a tenu tout ce temps caché, il se note.
    pub fn visible(&mut self, t_ms: f64) {
        if let Some(c) = self.cache_depuis_ms.take() {
            self.restee_ouverte_s.push((t_ms - c) / 1000.0);
        }
        // Après le retour, le premier écart de tics dirait le temps caché, pas le Wi-Fi.
        self.dernier_tic_ms = None;
    }

    /// La dérive de l'horloge du téléphone contre celle du relais, en millionièmes (le
    /// relais avance de tant par rapport au téléphone), sur le quart des échos les plus
    /// rapides ; rien sous 10 échos, ni sur moins de [`DERIVE_ETENDUE_MIN_MS`] (une horloge
    /// lue à la milliseconde près, comme celle de Firefox, ne dit rien d'une dérive en
    /// quelques secondes : 150 millionièmes sur 6 s, constaté au banc).
    pub fn derive(&self) -> Option<f64> {
        let (Some(premier), Some(dernier)) = (self.heures.front(), self.heures.back()) else {
            return None;
        };
        if self.heures.len() < 10
            || dernier.milieu_us - premier.milieu_us < DERIVE_ETENDUE_MIN_MS * 1000.0
        {
            return None;
        }
        let mut ar: Vec<f64> = self.heures.iter().map(|h| h.aller_retour_ms).collect();
        ar.sort_by(f64::total_cmp);
        let seuil = ar
            .get(ar.len().saturating_sub(1) / 4)
            .copied()
            .unwrap_or(f64::MAX);
        let points: Vec<(f64, f64)> = self
            .heures
            .iter()
            .filter(|h| h.aller_retour_ms <= seuil)
            .map(|h| (h.milieu_us, h.ecart_us))
            .collect();
        derive_ppm(&points)
    }

    /// Les échos envoyés, revenus, perdus (dont ceux d'une connexion tombée).
    pub const fn echos(&self) -> (u64, u64, u64) {
        (self.envoyes, self.recus, self.perdus)
    }

    /// L'aller-retour, en distribution.
    pub fn aller_retour(&self) -> Option<Centiles> {
        centiles(&self.aller_retour_ms)
    }

    /// Les écarts entre deux tics reçus, en distribution.
    pub fn ecarts_tics(&self) -> Option<Centiles> {
        centiles(&self.ecarts_tics_ms)
    }

    /// Le bilan, un objet JSON sur une ligne, pour le relais (`relais::mesure`) : le
    /// navigateur (`navigator.userAgent`, tronqué), la durée, les échos, l'aller-retour,
    /// les tics, l'horloge, l'arrière-plan. `fin` dit le dernier bilan d'une mesure finie.
    pub fn bilan(&self, navigateur: &str, t_ms: f64, fin: Oui) -> String {
        let duree_s = self.debut_ms.map_or(0.0, |d| (t_ms - d) / 1000.0);
        let fin = fin == Oui::Oui;
        let mut s = String::from("{");
        let _: core::fmt::Result = write!(
            s,
            "\"navigateur\":{},\"duree_s\":{},\"fin\":{fin},",
            chaine(navigateur, NAVIGATEUR_MAX),
            nombre(duree_s)
        );
        let _: core::fmt::Result = write!(
            s,
            "\"echos\":{{\"periode_ms\":{PERIODE_ECHO_MS},\"envoyes\":{},\"recus\":{},\"perdus\":{},\"rafales\":{}}},",
            self.envoyes, self.recus, self.perdus, self.rafales_montantes
        );
        let _: core::fmt::Result = write!(
            s,
            "\"aller_retour_ms\":{},",
            json_centiles(self.aller_retour().as_ref())
        );
        let _: core::fmt::Result = write!(
            s,
            "\"tics\":{{\"recus\":{},\"trous\":{},\"ecart_ms\":{}}},",
            self.tics,
            self.trous,
            json_centiles(self.ecarts_tics().as_ref())
        );
        let _: core::fmt::Result = write!(
            s,
            "\"derive_ppm\":{},",
            self.derive().map_or_else(|| String::from("null"), nombre)
        );
        let liste = |v: &[f64]| -> String {
            let n: Vec<String> = v.iter().copied().map(nombre).collect();
            format!("[{}]", n.join(","))
        };
        let _: core::fmt::Result = write!(
            s,
            "\"arriere_plan\":{{\"caches\":{},\"fermee_apres_s\":{},\"restee_ouverte_s\":{}}},\"coupures\":{},\"reconnexions\":{}}}",
            self.caches,
            liste(&self.fermee_apres_s),
            liste(&self.restee_ouverte_s),
            self.coupures,
            self.reconnexions
        );
        s
    }
}

/// Un nombre JSON, au dixième (toujours fini).
fn nombre(x: f64) -> String {
    if x.is_finite() {
        format!("{x:.1}")
    } else {
        String::from("null")
    }
}

fn json_centiles(c: Option<&Centiles>) -> String {
    c.map_or_else(
        || String::from("null"),
        |c| {
            format!(
                "{{\"n\":{},\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{}}}",
                c.n,
                nombre(c.p50),
                nombre(c.p95),
                nombre(c.p99),
                nombre(c.max)
            )
        },
    )
}

/// Une chaîne JSON d'au plus `max` caractères, sans caractère de contrôle (le relais
/// refuserait le bilan).
fn chaine(texte: &str, max: usize) -> String {
    let mut s = String::from("\"");
    for c in texte.chars().filter(|c| !c.is_control()).take(max) {
        if c == '"' || c == '\\' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_centiles_sont_au_rang_le_plus_proche() {
        let v: VecDeque<f64> = (1..=100).map(f64::from).collect();
        let c = centiles(&v).expect("des valeurs");
        assert_eq!(
            (c.n, c.p50, c.p95, c.p99, c.max),
            (100, 50.0, 95.0, 99.0, 100.0)
        );
        let un: VecDeque<f64> = [7.0].into_iter().collect();
        let c = centiles(&un).expect("une valeur");
        assert_eq!((c.p50, c.p99, c.max), (7.0, 7.0, 7.0));
        assert_eq!(centiles(&VecDeque::new()), None);
    }

    #[test]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "des heures de test, positives et sous 2⁵³ µs"
    )]
    fn la_derive_se_lit_sur_les_echos_rapides() {
        // Le relais avance de 50 µs par seconde du téléphone (50 ppm), à 3 ms près de
        // gigue sur les échos lents, qui ne comptent pas.
        let mut m = Mesure::new(0.0);
        // 400 échos sur 100 s.
        for k in 0..400u32 {
            let envoi = f64::from(k) * 250.0;
            let lent = k % 4 != 0;
            let ar = if lent { 30.0 } else { 4.0 };
            let milieu_us = (envoi + ar / 2.0) * 1000.0;
            let relais = 1_000_000.0 + milieu_us * (1.0 + 50e-6) + if lent { 3000.0 } else { 0.0 };
            m.echo_envoye();
            m.echo_recu(envoi, envoi + ar, relais as u64);
        }
        let d = m.derive().expect("une dérive");
        assert!((d - 50.0).abs() < 1.0, "{d}");
        // Sur moins d'une minute, rien.
        let mut court = Mesure::new(0.0);
        for k in 0..100u32 {
            let envoi = f64::from(k) * 50.0;
            court.echo_envoye();
            court.echo_recu(envoi, envoi + 4.0, 1_000_000 + (envoi * 1000.0) as u64);
        }
        assert_eq!(court.derive(), None);
        assert_eq!(derive_ppm(&[(1.0, 1.0)]), None);
        assert_eq!(derive_ppm(&[(1.0, 1.0), (1.0, 2.0)]), None);
    }

    #[test]
    fn rafales_trous_coupures_et_arriere_plan_se_comptent() {
        let mut m = Mesure::new(0.0);
        m.ouverte();
        m.echo_envoye();
        m.echo_recu(0.0, 150.0, 1);
        m.echo_envoye();
        m.echo_recu(50.0, 60.0, 2);
        for t in [0.0, 16.7, 33.4, 250.0, 266.7] {
            m.tic(t);
        }
        // Deux échos en vol quand la page se cache, puis la connexion tombe 5 s après.
        m.echo_envoye();
        m.echo_envoye();
        m.cachee(1_000.0);
        m.fermee(6_000.0);
        m.visible(20_000.0);
        m.ouverte();
        m.cachee(30_000.0);
        m.visible(42_500.0);
        let b = m.bilan("Mozilla/5.0 \"iPhone\"\n", 60_000.0, Oui::Oui);
        assert!(
            b.contains("\"navigateur\":\"Mozilla/5.0 \\\"iPhone\\\"\""),
            "{b}"
        );
        assert!(
            b.contains(
                "\"echos\":{\"periode_ms\":50,\"envoyes\":4,\"recus\":2,\"perdus\":2,\"rafales\":1}"
            ),
            "{b}"
        );
        assert!(b.contains("\"tics\":{\"recus\":5,\"trous\":1,"), "{b}");
        assert!(b.contains("\"arriere_plan\":{\"caches\":2,\"fermee_apres_s\":[5.0],\"restee_ouverte_s\":[12.5]}"), "{b}");
        assert!(b.ends_with("\"coupures\":1,\"reconnexions\":1}"), "{b}");
        assert!(b.contains("\"duree_s\":60.0,\"fin\":true"), "{b}");
        assert!(!b.chars().any(char::is_control));
    }
}
