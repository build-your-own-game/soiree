//! Le scénario `mesure-wifi` (PHASE-3, § 7, ligne du lot 14 : « page `/#mesure` du
//! Wi-Fi » ; M5 au § 8.6) : la page de mesure joue de bout en bout dans un vrai
//! navigateur, sur la boucle locale. **Ce n'est pas M5** : M5 se mesure sur un vrai Wi-Fi,
//! avec de vrais téléphones, à la répétition (`conception/REPETITION-PHASE-3.md`). Le
//! banc prouve seulement que ce que l'hôte lira ce jour-là est mesuré, transmis, gardé et
//! montré.
//!
//! Dans chaque navigateur : le relais dans ce processus, en mode banc, qui annonce
//! `192.0.2.1` sans l'écouter (comme le scénario `aide`) ; la télé à
//! `http://localhost:<port>/#mesure`, qui doit montrer le QR de
//! `http://192.0.2.1:<port>/mesure` ; un onglet de téléphone à
//! `http://127.0.0.1:<port>/mesure?duree=<s>`, qui mesure puis envoie son dernier bilan.
//! Le banc juge le bilan que le relais garde (`GET /mesure/bilans`) et son fichier :
//! des échos au rythme voulu, aucun perdu, des tics au rythme des vues, un aller-retour de
//! boucle locale, la télé qui le montre.
//!
//! Le contrôle négatif, joué à chaque passage : le relais retarde chaque écho de
//! [`RETARD_DU_NEGATIF`] (`Reglages::retard_d_echo_du_banc`, au banc seulement) ; la
//! médiane de l'aller-retour du bilan doit en être au moins là, sinon « CONTRÔLE
//! NÉGATIF RATÉ ».

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, p le passage, b le bilan"
)]

use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value as Json;

use crate::horloge::{maintenant, reste};
use crate::navigateur::{Interface, Lance, Navigateur};
use crate::soiree::{PORT_DE_LA_SOIREE, construire_le_paquet, dernier_paquet};
use crate::soiree_native::port_libre;

/// L'adresse que le relais du banc annonce sans l'écouter (une adresse de documentation,
/// comme `aide`).
pub const ADRESSE_DU_BANC: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));

/// Le retard des échos du contrôle négatif : 40 ms (choisi : bien au-dessus d'un
/// aller-retour de boucle locale, quelques millisecondes).
pub const RETARD_DU_NEGATIF: Duration = Duration::from_millis(40);

/// La médiane de l'aller-retour sur la boucle locale doit tenir sous 20 ms (choisi : la
/// boucle en fait moins de 5 ; au-delà, la page ou le relais mesurent mal).
pub const ALLER_RETOUR_BOUCLE_MS: f64 = 20.0;

/// La part des échos et des tics attendus qui doit arriver : 80 % (choisi : les
/// minuteries d'un navigateur sans interface glissent un peu).
pub const PART_ATTENDUE: f64 = 0.8;

/// Ce que le scénario joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// La durée de la mesure du téléphone, en secondes.
    pub duree_s: u32,
    /// Construire le paquet `web/` (sinon, le dernier construit).
    pub construire: bool,
    /// Le dossier de travail.
    pub travail: PathBuf,
    /// Jouer le contrôle négatif.
    pub negatif: bool,
}

/// Ce qu'un passage a constaté.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur et sa version.
    pub navigateur: String,
    /// Ce qui a été vu, sans jugement.
    pub notes: Vec<String>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
    /// Pas exécuté, et pourquoi.
    pub pas_execute: Option<String>,
}

impl fmt::Display for Passage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(r) = &self.pas_execute {
            return writeln!(f, "== {} : {r}", self.navigateur);
        }
        writeln!(f, "== {}", self.navigateur)?;
        for n in &self.notes {
            writeln!(f, "  {n}")?;
        }
        if self.fautes.is_empty() {
            writeln!(f, "  verdict : réussi")
        } else {
            writeln!(f, "  verdict : ÉCHEC")?;
            for e in &self.fautes {
                writeln!(f, "    {e}")?;
            }
            Ok(())
        }
    }
}

fn http(port: u16, chemin: &str) -> Result<String, String> {
    let mut f = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .map_err(|e| format!("{e}"))?;
    f.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("{e}"))?;
    f.write_all(format!("GET {chemin} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n").as_bytes())
        .map_err(|e| format!("{e}"))?;
    let mut o = Vec::new();
    let _: std::io::Result<usize> = f.read_to_end(&mut o);
    let t = String::from_utf8_lossy(&o).into_owned();
    Ok(t.split_once("\r\n\r\n")
        .map(|(_, c)| c.to_owned())
        .unwrap_or_default())
}

fn nombre(v: &Json, chemin: &str) -> Option<f64> {
    v.pointer(chemin).and_then(Json::as_f64)
}

/// Juge un bilan final d'une mesure de `duree_s` secondes ; `retard` est celui que le
/// relais a mis à chaque écho (le contrôle négatif). Rend les fautes.
pub fn juger_le_bilan(b: &Json, duree_s: u32, retard: Option<Duration>) -> Vec<String> {
    let mut fautes = Vec::new();
    let duree = f64::from(duree_s);
    let p50 = nombre(b, "/aller_retour_ms/p50");
    if let Some(r) = retard {
        let attendu = r.as_secs_f64() * 1000.0;
        match p50 {
            Some(m) if m >= attendu => {}
            autre => fautes.push(format!(
                "CONTRÔLE NÉGATIF RATÉ : la médiane de l'aller-retour vaut {autre:?} ms, les échos retardés de {attendu} ms par le relais"
            )),
        }
        return fautes;
    }
    let periode = nombre(b, "/echos/periode_ms").unwrap_or(50.0);
    let echos_attendus = duree * 1000.0 / periode * PART_ATTENDUE;
    match nombre(b, "/echos/recus") {
        Some(n) if n >= echos_attendus => {}
        autre => fautes.push(format!(
            "échos reçus {autre:?}, au moins {echos_attendus:.0} attendus en {duree_s} s"
        )),
    }
    if nombre(b, "/echos/perdus") != Some(0.0) {
        fautes.push(format!(
            "des échos perdus sur la boucle locale : {:?}",
            b.pointer("/echos/perdus")
        ));
    }
    let tics_attendus = duree * 60.0 * PART_ATTENDUE;
    match nombre(b, "/tics/recus") {
        Some(n) if n >= tics_attendus => {}
        autre => fautes.push(format!(
            "tics reçus {autre:?}, au moins {tics_attendus:.0} attendus en {duree_s} s"
        )),
    }
    match p50 {
        Some(m) if m < ALLER_RETOUR_BOUCLE_MS => {}
        autre => fautes.push(format!(
            "la médiane de l'aller-retour sur la boucle locale vaut {autre:?} ms, au-delà de {ALLER_RETOUR_BOUCLE_MS} ms"
        )),
    }
    if b.get("fin") != Some(&Json::Bool(true)) {
        fautes.push(String::from(
            "le dernier bilan ne dit pas la fin de la mesure",
        ));
    }
    if b.get("navigateur")
        .and_then(Json::as_str)
        .is_none_or(str::is_empty)
    {
        fautes.push(String::from("le bilan ne dit pas le navigateur"));
    }
    fautes
}

/// Un relais, la télé en `/#mesure`, un téléphone qui mesure `duree_s` : le bilan final
/// (et sa ligne du fichier), et ce que la télé a dit au banc.
#[expect(
    clippy::too_many_lines,
    reason = "une mesure se lit d'un trait, dans l'ordre où elle se joue : le relais, la télé, le téléphone, le bilan"
)]
fn une_mesure(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    (nom, retard, duree_s): (&str, Option<Duration>, u32),
    p: &mut Passage,
) -> Result<Json, String> {
    let dossier = o.travail.join(n.nom()).join(nom);
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    let garde = dossier.join("garde");
    std::fs::create_dir_all(&dossier).map_err(|e| format!("{e}"))?;
    let port = if std::net::TcpListener::bind(("127.0.0.1", PORT_DE_LA_SOIREE)).is_ok()
        && std::net::TcpListener::bind(("::1", PORT_DE_LA_SOIREE)).is_ok()
    {
        PORT_DE_LA_SOIREE
    } else {
        port_libre()?
    };
    let relais = relais::coque::lancer(&relais::coque::Reglages {
        port,
        adresse: None,
        garde: garde.clone(),
        banc: relais::http::Banc::Oui,
        enregistrer: None,
        interfaces: Some(Vec::new()),
        page: None,
        paquet: Some(relais::paquet::Paquet::lire(paquet)?),
        jeux: relais::paquet::Jeux::default(),
        annonce_du_banc: Some(ADRESSE_DU_BANC),
        retard_d_echo_du_banc: retard,
    })
    .map_err(|e| format!("le relais : {e}"))?;
    let profils = o.travail.join("profils");
    let mut navigateur = match Lance::lancer_avec(n, None, &profils, Interface::Sans) {
        Ok(l) => l,
        Err(e) => {
            relais.arreter();
            return Err(format!("pas exécuté : {e}"));
        }
    };
    navigateur.version().clone_into(&mut p.navigateur);
    let r = (|| -> Result<Json, String> {
        navigateur.ouvrir(&format!("http://localhost:{port}/#mesure"))?;
        let mut console = Vec::new();
        let qr_attendu = format!("http://{ADRESSE_DU_BANC}:{port}/mesure");
        let mut vu_qr = false;
        let fin = maintenant()
            .checked_add(Duration::from_secs(30))
            .ok_or("délai")?;
        while !vu_qr && !reste(fin).is_zero() {
            for l in navigateur.ecouter(Duration::from_millis(100))? {
                vu_qr |= l.contains("banc wifi") && l.contains(&format!("\"qr\":\"{qr_attendu}\""));
                console.push(l);
            }
        }
        if !vu_qr {
            return Err(format!(
                "la télé en /#mesure ne montre pas le QR de {qr_attendu} en 30 s ; console {:?}",
                console.iter().rev().take(6).collect::<Vec<_>>()
            ));
        }
        p.notes
            .push(format!("{nom} : la télé montre le QR de {qr_attendu}"));
        navigateur.nouvel_onglet(&format!("http://127.0.0.1:{port}/mesure?duree={duree_s}"))?;
        let fin = maintenant()
            .checked_add(Duration::from_secs(u64::from(duree_s).saturating_add(30)))
            .ok_or("délai")?;
        let bilan = loop {
            for l in navigateur.ecouter(Duration::from_millis(250))? {
                console.push(l);
            }
            let corps = http(port, "/mesure/bilans")?;
            let v: Json = serde_json::from_str(&corps).map_err(|e| format!("bilans : {e}"))?;
            let dernier = v
                .get("bilans")
                .and_then(Json::as_array)
                .and_then(|b| b.last())
                .and_then(|b| b.get("bilan"))
                .and_then(Json::as_str)
                .and_then(|t| serde_json::from_str::<Json>(t).ok());
            if let Some(b) = dernier.filter(|b| b.get("fin") == Some(&Json::Bool(true))) {
                break b;
            }
            if reste(fin).is_zero() {
                return Err(format!(
                    "aucun bilan final en {} s ; bilans {corps} ; console {:?}",
                    u64::from(duree_s).saturating_add(30),
                    console.iter().rev().take(8).collect::<Vec<_>>()
                ));
            }
        };
        // La télé le montre (elle relit toutes les deux secondes, et ne redit au banc que ce
        // qui change : un bilan déjà montré avant le dernier compte).
        let montre_dans = |console: &[String]| {
            console
                .iter()
                .any(|l| l.contains("banc wifi") && !l.contains("\"bilans\":0"))
        };
        let fin = maintenant()
            .checked_add(Duration::from_secs(10))
            .ok_or("délai")?;
        while !montre_dans(&console) && !reste(fin).is_zero() {
            console.extend(navigateur.ecouter(Duration::from_millis(100))?);
        }
        if !montre_dans(&console) {
            p.fautes
                .push(format!("{nom} : la télé ne montre aucun bilan en 10 s"));
        }
        let fichier = std::fs::read_to_string(garde.join(relais::mesure::FICHIER))
            .map_err(|e| format!("{} : {e}", relais::mesure::FICHIER))?;
        p.notes.push(format!(
            "{nom} : {} ligne(s) dans {} ; bilan final {bilan}",
            fichier.lines().count(),
            relais::mesure::FICHIER
        ));
        let erreurs: Vec<&String> = console
            .iter()
            .filter(|l| {
                l.starts_with("exception")
                    || l.contains("console.error")
                    || l.contains("Content Security Policy")
                    || l.contains("Refused to")
            })
            .collect();
        if !erreurs.is_empty() {
            p.fautes.push(format!("{nom} : la console : {erreurs:?}"));
        }
        Ok(bilan)
    })();
    navigateur.fermer();
    relais.arreter();
    r
}

/// Un passage : la mesure, puis le contrôle négatif.
fn passage(o: &Options, n: Navigateur, paquet: &Path) -> Passage {
    let mut p = Passage {
        navigateur: n.nom().to_owned(),
        ..Passage::default()
    };
    match une_mesure(o, n, paquet, ("mesure", None, o.duree_s), &mut p) {
        Ok(b) => p.fautes.extend(juger_le_bilan(&b, o.duree_s, None)),
        Err(e) if e.starts_with("pas exécuté") => {
            p.pas_execute = Some(e);
            return p;
        }
        Err(e) => p.fautes.push(e),
    }
    if o.negatif {
        let duree = o.duree_s.min(6);
        match une_mesure(
            o,
            n,
            paquet,
            ("negatif", Some(RETARD_DU_NEGATIF), duree),
            &mut p,
        ) {
            Ok(b) => {
                let f = juger_le_bilan(&b, duree, Some(RETARD_DU_NEGATIF));
                if f.is_empty() {
                    p.notes.push(format!(
                        "contrôle négatif : les échos retardés de {} ms se voient (médiane {:?} ms)",
                        RETARD_DU_NEGATIF.as_millis(),
                        nombre(&b, "/aller_retour_ms/p50")
                    ));
                }
                p.fautes.extend(f);
            }
            Err(e) => p.fautes.push(format!("contrôle négatif : {e}")),
        }
    }
    p
}

/// Joue le scénario dans chaque navigateur.
pub fn jouer(o: &Options, soiree: &Path) -> Result<Vec<Passage>, String> {
    let paquet = if o.construire {
        construire_le_paquet(soiree)?
    } else {
        dernier_paquet(soiree)?
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    Ok(o.navigateurs
        .iter()
        .map(|n| passage(o, *n, &paquet))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bilan(p50: f64, recus: u32, tics: u32, perdus: u32) -> Json {
        serde_json::from_str(&format!(
            "{{\"navigateur\":\"Mozilla\",\"duree_s\":12.0,\"fin\":true,\"echos\":{{\"periode_ms\":50,\"envoyes\":{recus},\"recus\":{recus},\"perdus\":{perdus},\"rafales\":0}},\"aller_retour_ms\":{{\"n\":{recus},\"p50\":{p50},\"p95\":2.0,\"p99\":3.0,\"max\":4.0}},\"tics\":{{\"recus\":{tics},\"trous\":0,\"ecart_ms\":null}},\"derive_ppm\":null,\"arriere_plan\":{{\"caches\":0,\"fermee_apres_s\":[],\"restee_ouverte_s\":[]}},\"coupures\":0,\"reconnexions\":0}}"
        ))
        .expect("json")
    }

    #[test]
    fn un_bilan_de_boucle_locale_se_juge() {
        assert_eq!(
            juger_le_bilan(&bilan(1.2, 230, 700, 0), 12, None),
            Vec::<String>::new()
        );
        let f = juger_le_bilan(&bilan(1.2, 100, 700, 0), 12, None);
        assert!(f.iter().any(|f| f.starts_with("échos reçus")), "{f:?}");
        let f = juger_le_bilan(&bilan(1.2, 230, 100, 2), 12, None);
        assert!(f.iter().any(|f| f.starts_with("tics reçus")), "{f:?}");
        assert!(f.iter().any(|f| f.starts_with("des échos perdus")), "{f:?}");
        let f = juger_le_bilan(&bilan(25.0, 230, 700, 0), 12, None);
        assert!(f.iter().any(|f| f.contains("au-delà de 20 ms")), "{f:?}");
    }

    #[test]
    fn le_negatif_doit_voir_le_retard_des_echos() {
        assert_eq!(
            juger_le_bilan(&bilan(41.0, 100, 300, 0), 6, Some(RETARD_DU_NEGATIF)),
            Vec::<String>::new()
        );
        let f = juger_le_bilan(&bilan(1.5, 100, 300, 0), 6, Some(RETARD_DU_NEGATIF));
        assert!(
            f.iter().any(|f| f.starts_with("CONTRÔLE NÉGATIF RATÉ")),
            "{f:?}"
        );
    }
}
