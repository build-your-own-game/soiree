//! L'aide de l'écran (PHASE-3, § 2.8 ; lot 10b), sans navigateur : ce que l'écran dit quand
//! les téléphones n'arrivent pas, et **quoi faire** (CONTRAT, « Le jour où l'on joue »).
//! Testée en natif.
//!
//! - **Pas de réseau** (§ 2.5) : le relais n'annonce aucune adresse ; l'écran n'affiche
//!   pas de QR.
//! - **Une page sans lien** : le relais a vu un téléphone ouvrir sa page, puis aucune
//!   WebSocket de lui (`GET /aide`, `relais::aide`).
//! - **Le Wi-Fi isole les appareils** (SAL-99, SAL-100) : aucun téléphone
//!   [`SANS_TELEPHONE_MS`] après que le QR s'est affiché.
//! - **Le pare-feu de macOS** (SAL-95, REM-150) : le relais dit que son binaire n'y est pas
//!   autorisé.

/// Aucun téléphone ce temps après le QR : l'écran parle de l'isolation du Wi-Fi (choisi,
/// § 2.8 : une minute).
pub const SANS_TELEPHONE_MS: u64 = 60_000;

/// Un message de l'aide.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Message {
    /// Aucune adresse sur le réseau local.
    PasDeReseau,
    /// Un téléphone a ouvert la page, sans se relier.
    PageSansLien,
    /// Aucun téléphone une minute après le QR.
    WifiIsole,
    /// Le pare-feu de macOS n'autorise pas le relais.
    PareFeu,
}

impl Message {
    /// Son nom, pour le banc.
    pub const fn nom(self) -> &'static str {
        match self {
            Message::PasDeReseau => "pas-de-reseau",
            Message::PageSansLien => "page-sans-lien",
            Message::WifiIsole => "wifi-isole",
            Message::PareFeu => "pare-feu",
        }
    }

    /// Ce que l'écran dit, et quoi faire.
    pub const fn texte(self) -> &'static str {
        match self {
            Message::PasDeReseau => {
                "Pas de réseau : les téléphones ne peuvent pas se relier ; Tier list se joue sans eux. Branche cet ordinateur au Wi-Fi, puis relance soiree-relais."
            }
            Message::PageSansLien => {
                "Un téléphone a ouvert la page mais n'a pas pu se relier : autorise soiree-relais dans le pare-feu du Mac (Réglages Système › Réseau › Pare-feu), puis recharge la page du téléphone."
            }
            Message::WifiIsole => {
                "Aucun téléphone depuis une minute. Certains Wi-Fi isolent les appareils : vérifie que les téléphones sont sur le même Wi-Fi que cet ordinateur, coupe l'isolation des clients de la box, ou branche un petit routeur."
            }
            Message::PareFeu => {
                "Autorise soiree-relais dans le pare-feu de macOS (Réglages Système › Réseau › Pare-feu) : sinon, macOS demandera à la première connexion d'un téléphone, peut-être derrière le plein écran."
            }
        }
    }
}

/// Ce que l'écran sait pour juger.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EtatAide {
    /// Les adresses que le relais annonce (de `VersPage::Accueil`) ; `None` : pas encore
    /// d'accueil.
    pub adresses: Option<usize>,
    /// Quand le QR s'est affiché, en ms de la page.
    pub qr_depuis_ms: Option<u64>,
    /// Un téléphone a-t-il été présent depuis ?
    pub telephone_vu: bool,
    /// Les pages sans lien que le relais dit (`GET /aide`).
    pub pages_sans_lien: u32,
    /// Le pare-feu gêne-t-il, d'après le relais (`a-autoriser`, `refuse`, `bloque-tout`) ?
    pub pare_feu_gene: bool,
}

/// Les messages à dire maintenant (`maintenant_ms`, l'heure de la page), dans l'ordre.
pub fn messages(e: &EtatAide, maintenant_ms: u64) -> Vec<Message> {
    let mut m = Vec::new();
    if e.adresses == Some(0) {
        m.push(Message::PasDeReseau);
        // Sans réseau, rien d'autre n'a d'objet : aucun téléphone ne viendra.
        return m;
    }
    if e.pages_sans_lien > 0 {
        m.push(Message::PageSansLien);
    }
    if !e.telephone_vu
        && e.qr_depuis_ms
            .is_some_and(|d| maintenant_ms.saturating_sub(d) >= SANS_TELEPHONE_MS)
    {
        m.push(Message::WifiIsole);
    }
    if e.pare_feu_gene {
        m.push(Message::PareFeu);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::{EtatAide, Message, SANS_TELEPHONE_MS, messages};

    /// § 2.5 : sans adresse sur le réseau local, l'écran le dit, et rien d'autre.
    #[test]
    fn sans_reseau_l_ecran_le_dit() {
        let e = EtatAide {
            adresses: Some(0),
            pages_sans_lien: 3,
            pare_feu_gene: true,
            ..EtatAide::default()
        };
        assert_eq!(messages(&e, 10_000_000), [Message::PasDeReseau]);
        assert!(Message::PasDeReseau.texte().starts_with("Pas de réseau"));
    }

    /// SAL-99, SAL-100 : aucun téléphone une minute après le QR ; un téléphone vu fait
    /// taire le message.
    #[test]
    fn aucun_telephone_une_minute_apres_le_qr_sal99() {
        let mut e = EtatAide {
            adresses: Some(1),
            qr_depuis_ms: Some(1_000),
            ..EtatAide::default()
        };
        assert_eq!(messages(&e, 1_000 + SANS_TELEPHONE_MS - 1), []);
        assert_eq!(
            messages(&e, 1_000 + SANS_TELEPHONE_MS),
            [Message::WifiIsole]
        );
        assert!(
            Message::WifiIsole
                .texte()
                .contains("Certains Wi-Fi isolent les appareils")
        );
        e.telephone_vu = true;
        assert_eq!(messages(&e, 1_000 + 10 * SANS_TELEPHONE_MS), []);
        // Sans QR, pas d'attente.
        let e = EtatAide {
            adresses: Some(1),
            ..EtatAide::default()
        };
        assert_eq!(messages(&e, u64::MAX), []);
    }

    /// Une page sans lien, et le pare-feu (SAL-95, REM-150), se disent avec ce qu'il faut
    /// faire.
    #[test]
    fn une_page_sans_lien_et_le_pare_feu_sal95() {
        let e = EtatAide {
            adresses: Some(1),
            pages_sans_lien: 1,
            pare_feu_gene: true,
            telephone_vu: true,
            ..EtatAide::default()
        };
        assert_eq!(messages(&e, 0), [Message::PageSansLien, Message::PareFeu]);
        assert!(
            Message::PageSansLien
                .texte()
                .starts_with("Un téléphone a ouvert la page mais n'a pas pu se relier")
        );
        assert!(
            Message::PareFeu
                .texte()
                .starts_with("Autorise soiree-relais")
        );
    }
}
