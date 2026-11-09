//! La mesure du vrai Wi-Fi (PHASE-3, § 8.5, M5 au § 8.6 ; lot 14) : ce que le relais fait
//! pour la page `/#mesure`, sans rien lire ni chiffrer.
//!
//! - **La télé** ouvre `http://localhost:<port>/#mesure` : elle montre un QR vers la page de
//!   mesure des téléphones, et le dernier bilan de chacun (`GET /mesure/bilans`, boucle
//!   locale seulement).
//! - **Un téléphone** ouvre `http://<adresse annoncée>:<port>/mesure` (la page du commun,
//!   en mode mesure) et se relie à `GET /mesure/ws`, une WebSocket à part, qui ne passe ni
//!   par les salles ni par `liaison` : rien de la soirée n'y circule.
//! - Sur cette WebSocket, **le relais renvoie chaque écho** ([`ECHO_OCTETS`] octets reçus,
//!   les mêmes octets suivis de son heure en microsecondes) et **envoie un tic** de son
//!   heure tous les [`TIC_US`] (le rythme des vues d'un jeu à 60 pas par seconde) ; le
//!   téléphone en tire l'aller-retour, la gigue et les rafales dans les deux sens, l'écart
//!   et la dérive de son horloge, et ce que l'arrière-plan fait de sa connexion.
//! - **Un bilan** (un texte JSON du téléphone, au plus [`BILAN_MAX`] octets, sans saut de
//!   ligne) est gardé en mémoire (le dernier de chaque lien, au plus [`BILANS_MAX`] liens) et
//!   ajouté, tel quel et échappé en chaîne, à `<garde>/mesure-wifi.jsonl` (au plus
//!   [`FICHIER_MAX`] octets), où l'hôte le retrouve après la répétition. Le relais ne
//!   l'interprète pas.
//!
//! Ces trames sont en clair : elles ne portent que des heures, des compteurs et le nom du
//! navigateur du téléphone. Une page de mesure n'apprend rien d'une salle.

use std::collections::BTreeMap;

/// Les octets d'un écho, du téléphone au relais : un numéro (`u64`) et l'heure du
/// téléphone (`f64`), petit-boutistes ; le relais n'en lit que la longueur.
pub const ECHO_OCTETS: usize = 16;

/// La réponse à un écho : ses octets, puis l'heure du relais (`u64`, µs).
pub const REPONSE_OCTETS: usize = ECHO_OCTETS + 8;

/// Un tic : l'heure du relais (`u64`, µs), tous les [`TIC_US`].
pub const TIC_OCTETS: usize = 8;

/// L'intervalle des tics : 16 667 µs, un pas à 60 pas par seconde (les vues d'un jeu, P7).
pub const TIC_US: u64 = 16_667;

/// Le plus long bilan : 4 Kio (choisi ; un bilan fait moins de 1 Kio).
pub const BILAN_MAX: usize = 4 * 1024;

/// Les liens dont le relais garde le dernier bilan en mémoire : 64 (choisi ; une soirée a
/// 8 à 15 téléphones, OUT-131). Au-delà, le plus ancien s'oublie (le fichier les garde).
pub const BILANS_MAX: usize = 64;

/// La plus grande taille du fichier des bilans : 16 Mio (choisi, comme un flux de
/// résultats, `garde::FLUX_MAX`). Au-delà, un bilan n'est plus écrit (il reste en mémoire).
pub const FICHIER_MAX: u64 = 16 * 1024 * 1024;

/// Le nom du fichier des bilans, à la racine du dossier de la garde.
pub const FICHIER: &str = "mesure-wifi.jsonl";

/// Le plus long qu'une mesure reste reliée : 30 min (choisi ; la répétition mesure 10 à
/// 20 minutes, puis la page se relie de nouveau si on la relance).
pub const DUREE_MAX_US: u64 = 30 * 60 * 1_000_000;

/// La réponse à un écho, ou rien si les octets n'en sont pas un.
pub fn repondre(echo: &[u8], relais_us: u64) -> Option<[u8; REPONSE_OCTETS]> {
    if echo.len() != ECHO_OCTETS {
        return None;
    }
    let mut r = [0u8; REPONSE_OCTETS];
    r.get_mut(..ECHO_OCTETS)?.copy_from_slice(echo);
    r.get_mut(ECHO_OCTETS..)?
        .copy_from_slice(&relais_us.to_le_bytes());
    Some(r)
}

/// Un tic.
pub const fn tic(relais_us: u64) -> [u8; TIC_OCTETS] {
    relais_us.to_le_bytes()
}

/// L'heure du prochain tic après celui de `prevu`, à `maintenant` : le suivant de la grille,
/// sauf si le relais a pris plus d'une seconde de retard, où la grille repart de maintenant
/// (pas de rafale de tics pour rattraper : un tic en retard est ce que l'on mesure).
pub const fn prochain_tic(prevu: u64, maintenant: u64) -> u64 {
    let suivant = prevu.saturating_add(TIC_US);
    if maintenant.saturating_sub(suivant) > 1_000_000 {
        maintenant.saturating_add(TIC_US)
    } else {
        suivant
    }
}

/// Un bilan que le relais accepte : de 2 à [`BILAN_MAX`] octets, un objet (`{`…`}`), sans
/// caractère de contrôle (ni saut de ligne, qui couperait le fichier).
pub fn bilan_valide(bilan: &str) -> bool {
    (2..=BILAN_MAX).contains(&bilan.len())
        && bilan.starts_with('{')
        && bilan.ends_with('}')
        && !bilan.chars().any(char::is_control)
}

/// Une chaîne JSON : `"` et `\` échappés (le bilan n'a aucun caractère de contrôle).
fn chaine(texte: &str) -> String {
    let mut s = String::with_capacity(texte.len().saturating_add(2));
    s.push('"');
    for c in texte.chars() {
        if c == '"' || c == '\\' {
            s.push('\\');
        }
        s.push(c);
    }
    s.push('"');
    s
}

/// La ligne du fichier pour un bilan : `{"lien":…,"recu_us":…,"bilan":"<le bilan>"}`.
pub fn ligne(lien: u32, recu_us: u64, bilan: &str) -> String {
    format!(
        "{{\"lien\":{lien},\"recu_us\":{recu_us},\"bilan\":{}}}",
        chaine(bilan)
    )
}

/// Les derniers bilans, par lien.
#[derive(Debug, Default)]
pub struct Bilans {
    derniers: BTreeMap<u32, (u64, String)>,
}

impl Bilans {
    /// Garde le dernier bilan de `lien` ; au-delà de [`BILANS_MAX`] liens, oublie celui du
    /// plus petit numéro (le plus ancien lien).
    pub fn garder(&mut self, lien: u32, recu_us: u64, bilan: &str) {
        self.derniers.insert(lien, (recu_us, bilan.to_owned()));
        while self.derniers.len() > BILANS_MAX {
            let Some(premier) = self.derniers.keys().next().copied() else {
                break;
            };
            self.derniers.remove(&premier);
        }
    }

    /// Les liens gardés.
    pub fn len(&self) -> usize {
        self.derniers.len()
    }

    /// Aucun bilan.
    pub fn is_empty(&self) -> bool {
        self.derniers.is_empty()
    }

    /// `GET /mesure/bilans` : les adresses où les téléphones ouvrent la page de mesure
    /// (`http://<adresse>:<port>/mesure`), l'heure du relais, et les derniers bilans.
    pub fn en_json(&self, pages: &[String], maintenant_us: u64) -> String {
        let pages: Vec<String> = pages.iter().map(|p| chaine(p)).collect();
        let bilans: Vec<String> = self
            .derniers
            .iter()
            .map(|(lien, (t, b))| ligne(*lien, *t, b))
            .collect();
        format!(
            "{{\"pages\":[{}],\"maintenant_us\":{maintenant_us},\"bilans\":[{}]}}",
            pages.join(","),
            bilans.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_echo_revient_avec_l_heure_du_relais() {
        let echo: Vec<u8> = (0..16).collect();
        let r = repondre(&echo, 0x0102_0304_0506_0708).expect("un écho");
        assert_eq!(r.get(..16), Some(echo.as_slice()));
        assert_eq!(r.get(16..), Some(&[8u8, 7, 6, 5, 4, 3, 2, 1][..]));
        assert_eq!(repondre(&echo[..15], 1), None);
        assert_eq!(repondre(&[0; 17], 1), None);
        assert_eq!(tic(1), [1, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn les_tics_suivent_leur_grille_sans_rafale_de_rattrapage() {
        assert_eq!(prochain_tic(1_000, 1_000), 1_000 + TIC_US);
        // Un peu de retard : la grille tient (le tic suivant part aussitôt).
        assert_eq!(prochain_tic(1_000, 200_000), 1_000 + TIC_US);
        // Plus d'une seconde de retard : la grille repart de maintenant.
        assert_eq!(prochain_tic(1_000, 2_000_000), 2_000_000 + TIC_US);
    }

    #[test]
    fn un_bilan_est_un_objet_sur_une_ligne_et_s_echappe() {
        assert!(bilan_valide("{\"a\":1}"));
        assert!(!bilan_valide("{\"a\":1}\n"));
        assert!(!bilan_valide("{\"a\":\"\u{7}\"}"));
        assert!(!bilan_valide("[1]"));
        assert!(!bilan_valide(&format!(
            "{{\"a\":\"{}\"}}",
            "x".repeat(BILAN_MAX)
        )));
        let bilan = "{\"ua\":\"Mozilla \\\"x\\\" \\\\ y\"}";
        let l = ligne(3, 9, bilan);
        let v: serde_json::Value = serde_json::from_str(&l).expect("une ligne JSON");
        assert_eq!(v.get("lien").and_then(serde_json::Value::as_u64), Some(3));
        assert_eq!(
            v.get("recu_us").and_then(serde_json::Value::as_u64),
            Some(9)
        );
        assert_eq!(
            v.get("bilan").and_then(serde_json::Value::as_str),
            Some(bilan)
        );
    }

    #[test]
    fn la_memoire_des_bilans_est_bornee() {
        let mut b = Bilans::default();
        let n = u32::try_from(BILANS_MAX).expect("64 liens");
        for lien in 0..n + 5 {
            b.garder(lien, u64::from(lien), "{}");
        }
        b.garder(7, 99, "{\"dernier\":1}");
        assert_eq!(b.len(), BILANS_MAX);
        let j = b.en_json(&[String::from("http://192.0.2.1:7878/mesure")], 5);
        assert!(
            j.starts_with("{\"pages\":[\"http://192.0.2.1:7878/mesure\"],\"maintenant_us\":5,\"bilans\":[{\"lien\":5,"),
            "{j}"
        );
        assert!(j.contains("{\"lien\":7,\"recu_us\":99,"), "{j}");
        assert!(
            !j.contains("\"lien\":0,"),
            "le plus ancien est oublié : {j}"
        );
    }
}
