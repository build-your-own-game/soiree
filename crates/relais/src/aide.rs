//! L'aide du jour J (PHASE-3, § 2.8 ; lot 10b), sans entrées-sorties : ce que le relais
//! sait des téléphones qui n'arrivent pas, pour que l'écran dise **quoi faire** (CONTRAT,
//! « Le jour où l'on joue » : « Le commun vérifie que les appareils se rejoignent, et dit
//! clairement quoi faire sinon »).
//!
//! - **Une page sans lien** : un téléphone du réseau a ouvert `GET /s/<salle>`, puis
//!   aucune WebSocket n'est venue de son adresse dans [`PAGE_SANS_LIEN_MS`]. La page est
//!   arrivée : le Wi-Fi porte le HTTP ; c'est le lien qui manque (le pare-feu de la
//!   machine, un mandataire, une page qui ne démarre pas).
//! - **Le pare-feu de macOS** (SAL-95, REM-150), lu sans droits par `socketfilterfw` :
//!   s'il est allumé et que le binaire du relais n'y est pas autorisé, la première
//!   connexion d'un téléphone fait apparaître une demande sur le Mac, souvent derrière le
//!   navigateur en plein écran ; un binaire recompilé est une application neuve pour lui.
//!   Ici, seul le jugement de ce que l'outil écrit ; la coque le lance.
//!
//! « Aucun téléphone 60 s après le QR » (SAL-99, SAL-100) et « Pas de réseau » se jugent
//! à l'écran, qui voit le QR et les appareils (`web/ecran`, `aide`).

use std::fmt::Write as _;
use std::net::IpAddr;

/// Une page de téléphone sans WebSocket de son adresse depuis ce temps est « sans lien »
/// (choisi : la page d'un téléphone charge son paquet, quelques Mio, avant de se relier ;
/// quinze secondes sur un Wi-Fi lent).
pub const PAGE_SANS_LIEN_MS: u64 = 15_000;

/// Une page sans lien se dit pendant ce temps, puis s'oublie (choisi : dix minutes, le
/// temps d'arranger le Wi-Fi ou le pare-feu sans que l'écran se taise trop tôt).
pub const SOUVENIR_MS: u64 = 600_000;

/// Les adresses suivies au plus : au-delà, la plus ancienne s'oublie (choisi ; une salle
/// tient 71 appareils, `Places` de Sidequest).
pub const ADRESSES_MAX: usize = 128;

/// Ce que dit le pare-feu de macOS du binaire du relais.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PareFeu {
    /// Le pare-feu est éteint.
    Eteint,
    /// Il est allumé, et ce binaire y est autorisé.
    Autorise,
    /// Il est allumé, et ce binaire n'y est pas encore : macOS demandera à la première
    /// connexion d'un téléphone.
    AAutoriser,
    /// Il est allumé, et ce binaire y est refusé.
    Refuse,
    /// Il refuse toute connexion entrante (« Bloquer toutes les connexions entrantes »).
    BloqueTout,
    /// L'outil n'a pas pu le dire (pas macOS, `socketfilterfw` absent ou illisible).
    Inconnu(String),
}

impl PareFeu {
    /// Son nom, dans le JSON de `GET /aide`.
    pub const fn nom(&self) -> &'static str {
        match self {
            PareFeu::Eteint => "eteint",
            PareFeu::Autorise => "autorise",
            PareFeu::AAutoriser => "a-autoriser",
            PareFeu::Refuse => "refuse",
            PareFeu::BloqueTout => "bloque-tout",
            PareFeu::Inconnu(_) => "inconnu",
        }
    }

    /// Faut-il que l'hôte agisse pour que les téléphones se relient ?
    pub const fn gene(&self) -> bool {
        matches!(
            self,
            PareFeu::AAutoriser | PareFeu::Refuse | PareFeu::BloqueTout
        )
    }
}

/// Juge ce que `socketfilterfw` a écrit, sans droits (SAL-95, REM-150) :
/// `--getglobalstate`, `--getblockall` et `--listapps`, pour le binaire `executable`.
///
/// `--getappblocked` ne sert pas : il dit « permitted » d'un binaire qu'il ne connaît pas
/// (constaté le 2026-11-07, macOS 27.2, sur `/usr/bin/true` et un chemin qui n'existe
/// pas), alors que macOS demandera à sa première connexion entrante.
pub fn juger_le_pare_feu(
    global: &str,
    bloquer_tout: &str,
    applis: &str,
    executable: &str,
) -> PareFeu {
    if global.contains("State = 0") || global.contains("is disabled") {
        return PareFeu::Eteint;
    }
    if !(global.contains("State = 1")
        || global.contains("State = 2")
        || global.contains("is enabled"))
    {
        return PareFeu::Inconnu(format!("état global illisible : {}", global.trim()));
    }
    if bloquer_tout.contains("enabled") || global.contains("State = 2") {
        return PareFeu::BloqueTout;
    }
    // `--listapps` : « <n> : <chemin> », puis « (Allow incoming connections) » ou
    // « (Block incoming connections) » sur la ligne suivante.
    let mut lignes = applis.lines();
    while let Some(l) = lignes.next() {
        let Some((numero, chemin)) = l.split_once(" : ") else {
            continue;
        };
        if numero.trim().parse::<u32>().is_err() || chemin.trim() != executable {
            continue;
        }
        let suite = lignes.next().unwrap_or("");
        return if suite.contains("Allow") {
            PareFeu::Autorise
        } else if suite.contains("Block") {
            PareFeu::Refuse
        } else {
            PareFeu::Inconnu(format!("{executable} : {}", suite.trim()))
        };
    }
    PareFeu::AAutoriser
}

/// Une adresse du réseau qui a ouvert la page d'un téléphone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Page {
    ip: IpAddr,
    /// La dernière page ouverte de cette adresse.
    ouverte_us: u64,
    /// La dernière WebSocket venue de cette adresse.
    relie_us: Option<u64>,
}

/// Ce que le relais sait des téléphones du réseau qui ont ouvert leur page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Aide {
    pages: Vec<Page>,
}

impl Aide {
    /// Rien encore.
    pub const fn new() -> Aide {
        Aide { pages: Vec::new() }
    }

    /// L'entrée de cette adresse, mise à la fin (la plus récente).
    fn entree(&mut self, ip: IpAddr) -> Option<&mut Page> {
        if let Some(k) = self.pages.iter().position(|p| p.ip == ip) {
            let p = self.pages.remove(k);
            self.pages.push(p);
        } else {
            if self.pages.len() >= ADRESSES_MAX {
                let _: Page = self.pages.remove(0);
            }
            self.pages.push(Page {
                ip,
                ouverte_us: 0,
                relie_us: None,
            });
        }
        self.pages.last_mut()
    }

    /// Un téléphone du réseau, à l'adresse `ip`, a ouvert sa page (`GET /s/<salle>`).
    pub fn page(&mut self, ip: IpAddr, t_us: u64) {
        if let Some(p) = self.entree(ip) {
            p.ouverte_us = t_us;
        }
    }

    /// Une WebSocket est venue du réseau, de l'adresse `ip`.
    pub fn lien(&mut self, ip: IpAddr, t_us: u64) {
        if let Some(p) = self.entree(ip) {
            p.relie_us = Some(t_us);
        }
    }

    /// Les adresses dont la dernière page, ouverte depuis plus de [`PAGE_SANS_LIEN_MS`]
    /// et moins de [`SOUVENIR_MS`], n'a été suivie d'aucune WebSocket.
    pub fn pages_sans_lien(&self, t_us: u64) -> usize {
        self.pages
            .iter()
            .filter(|p| p.ouverte_us > 0)
            .filter(|p| {
                let age_ms = t_us.saturating_sub(p.ouverte_us) / 1000;
                (PAGE_SANS_LIEN_MS..SOUVENIR_MS).contains(&age_ms)
            })
            .filter(|p| p.relie_us.is_none_or(|l| l < p.ouverte_us))
            .count()
    }

    /// Le JSON de `GET /aide` : le réseau (au moins une des `annoncees` adresses), les
    /// pages sans lien, le pare-feu.
    pub fn json(&self, t_us: u64, annoncees: usize, pare_feu: &PareFeu) -> String {
        let mut s = format!(
            "{{\"reseau\":{},\"pages_sans_lien\":{},\"pare_feu\":\"{}\"",
            annoncees > 0,
            self.pages_sans_lien(t_us),
            pare_feu.nom()
        );
        if let PareFeu::Inconnu(pourquoi) = pare_feu {
            s.push_str(",\"pare_feu_detail\":\"");
            for c in pourquoi.chars() {
                match c {
                    '"' => s.push_str("\\\""),
                    '\\' => s.push_str("\\\\"),
                    c if u32::from(c) < 0x20 => {
                        let _: core::fmt::Result = write!(s, "\\u{:04x}", u32::from(c));
                    }
                    c => s.push(c),
                }
            }
            s.push('"');
        }
        s.push('}');
        s
    }
}

#[cfg(test)]
mod tests {
    use super::{ADRESSES_MAX, Aide, PAGE_SANS_LIEN_MS, PareFeu, SOUVENIR_MS, juger_le_pare_feu};
    use std::net::{IpAddr, Ipv4Addr};

    const MS: u64 = 1000;

    fn ip(n: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 168, 1, n))
    }

    /// SAL-99 : une page ouverte sans WebSocket ensuite se dit, passé le délai ; une
    /// page suivie de sa WebSocket ne se dit pas ; une page plus récente que la
    /// WebSocket se dit de nouveau.
    #[test]
    fn une_page_sans_lien_se_dit_apres_le_delai_sal99() {
        let mut a = Aide::new();
        let t0 = 1_000_000 * MS;
        a.page(ip(30), t0);
        assert_eq!(a.pages_sans_lien(t0), 0, "tout de suite : rien");
        assert_eq!(a.pages_sans_lien(t0 + (PAGE_SANS_LIEN_MS - 1) * MS), 0);
        assert_eq!(a.pages_sans_lien(t0 + PAGE_SANS_LIEN_MS * MS), 1);
        // Un autre téléphone, qui se relie : il ne compte pas.
        a.page(ip(31), t0);
        a.lien(ip(31), t0 + 3000 * MS);
        assert_eq!(a.pages_sans_lien(t0 + PAGE_SANS_LIEN_MS * MS), 1);
        // Le premier finit par se relier : plus rien.
        a.lien(ip(30), t0 + 20_000 * MS);
        assert_eq!(a.pages_sans_lien(t0 + 30_000 * MS), 0);
        // Il rouvre sa page sans se relier : de nouveau.
        a.page(ip(30), t0 + 40_000 * MS);
        assert_eq!(a.pages_sans_lien(t0 + (40_000 + PAGE_SANS_LIEN_MS) * MS), 1);
        // Et l'aide s'oublie au bout de `SOUVENIR_MS`.
        assert_eq!(a.pages_sans_lien(t0 + (40_000 + SOUVENIR_MS) * MS), 0);
    }

    /// Les adresses suivies sont bornées : la plus ancienne s'oublie.
    #[test]
    fn les_adresses_suivies_sont_bornees() {
        let mut a = Aide::new();
        for k in 0..(ADRESSES_MAX + 10) {
            let n = u32::try_from(k).unwrap();
            a.page(IpAddr::V4(Ipv4Addr::from(0x0a00_0000 + n)), 1);
        }
        assert_eq!(a.pages_sans_lien(1 + PAGE_SANS_LIEN_MS * MS), ADRESSES_MAX);
    }

    #[test]
    fn le_json_de_l_aide() {
        let mut a = Aide::new();
        a.page(ip(30), 1);
        assert_eq!(
            a.json(1 + PAGE_SANS_LIEN_MS * MS, 1, &PareFeu::AAutoriser),
            "{\"reseau\":true,\"pages_sans_lien\":1,\"pare_feu\":\"a-autoriser\"}"
        );
        assert_eq!(
            a.json(1, 0, &PareFeu::Inconnu(String::from("pas \"macOS\""))),
            "{\"reseau\":false,\"pages_sans_lien\":0,\"pare_feu\":\"inconnu\",\"pare_feu_detail\":\"pas \\\"macOS\\\"\"}"
        );
    }

    /// SAL-95, REM-150 : ce que `socketfilterfw` écrit, tel qu'il l'a écrit sur la
    /// machine de l'hôte le 2026-11-07 (macOS 27.2).
    #[test]
    fn le_pare_feu_se_lit_sans_droits_sal95_rem150() {
        let allume = "Firewall is enabled. (State = 1)\n";
        let eteint = "Firewall is disabled. (State = 0)\n";
        let pas_tout = "Firewall has block all state set to disabled.\n";
        let tout = "Firewall has block all state set to enabled.\n";
        let applis = "Total number of apps = 2 \n1 : /Applications/Autre.app/Contents/MacOS/Autre \n             (Allow incoming connections)\n2 : /Users/hote/.cargo/bin/soiree-relais \n             (Block incoming connections)\n";
        let relais = "/Users/hote/.cargo/bin/soiree-relais";
        assert_eq!(
            juger_le_pare_feu(eteint, pas_tout, "", relais),
            PareFeu::Eteint
        );
        assert_eq!(
            juger_le_pare_feu(allume, tout, applis, relais),
            PareFeu::BloqueTout
        );
        assert_eq!(
            juger_le_pare_feu(allume, pas_tout, applis, relais),
            PareFeu::Refuse
        );
        assert_eq!(
            juger_le_pare_feu(
                allume,
                pas_tout,
                applis,
                "/Applications/Autre.app/Contents/MacOS/Autre"
            ),
            PareFeu::Autorise
        );
        // Un binaire neuf, que le pare-feu ne connaît pas : macOS demandera.
        assert_eq!(
            juger_le_pare_feu(allume, pas_tout, applis, "/tmp/neuf/soiree-relais"),
            PareFeu::AAutoriser
        );
        assert!(PareFeu::AAutoriser.gene() && PareFeu::Refuse.gene() && PareFeu::BloqueTout.gene());
        assert!(!PareFeu::Autorise.gene() && !PareFeu::Eteint.gene());
        assert!(matches!(
            juger_le_pare_feu("", "", "", relais),
            PareFeu::Inconnu(_)
        ));
    }
}
