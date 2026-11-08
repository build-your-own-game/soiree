//! `soiree-banc aide` (PHASE-3, § 2.8, § 8.2 ; lot 10b) : **le jour J à l'écran**. Le
//! relais (le même code, dans ce processus, avec `--banc`), la vraie page de la télé et
//! son Worker, dans chaque navigateur ; aucun téléphone ne se relie.
//!
//! Ce que le scénario joue, dans chaque navigateur, et ce qu'il exige de l'écran :
//!
//! 1. **Le réseau simulé** : le relais annonce une adresse de documentation (`192.0.2.1`)
//!    sans l'écouter (`Reglages::annonce_du_banc`), pour qu'un QR s'affiche sans rien
//!    ouvrir sur le Wi-Fi. Le module joué n'a pas de rapport : l'écran de démarrage le dit
//!    « NON ÉPROUVÉ », et l'insigne aussi (P17).
//!    - **un `GET /s/<salle>` sans WebSocket ensuite** (d'un `Host` annoncé : le relais du
//!      banc le compte comme venu du réseau) : l'écran dit « Un téléphone a ouvert la page
//!      mais n'a pas pu se relier » ;
//!    - **aucun téléphone en 60 s** après le QR : « Certains Wi-Fi isolent les appareils »
//!      (SAL-99, SAL-100) ;
//!    - **un effet tombé avant le geste est compté perdu** (SAL-174 : chez un jeu qui en
//!      émet un à sa première scène, Punchline) ;
//!    - **le geste** : un clic sur « Jouer », par la vraie chaîne d'entrées du navigateur ;
//!      le plein écran, le verrou d'Échap (P38), le son, le verrou d'écran, chacun dit ;
//!    - **Échap en plein écran** (P38, chez un jeu dont la scène porte un raccourci Échap,
//!      Tier list) : avec le verrou (Chrome), il arrive au jeu (l'autorité place une entrée
//!      de l'écran) et le plein écran reste ; sans verrou (Firefox), il sort du plein écran
//!      sans arriver au jeu, et l'écran du commun montre « Plein écran quitté ». Firefox
//!      sans interface refuse le plein écran au clic du banc : ce point n'y est que « pas
//!      exécuté » ; `--avec-interface` le lance avec sa fenêtre, où il se constate
//!      ([`crate::navigateur::Interface`]) ;
//!    - **l'onglet rechargé** : « Cliquer pour reprendre », la partie reprise sous le voile,
//!      puis le clic qui le lève.
//! 2. **Sans réseau** : le relais n'annonce rien ; il démarre, l'écran n'a pas de QR et
//!    dit « Pas de réseau ». Le module a cette fois un rapport (écrit par le banc, avec
//!    l'empreinte réécrite par wasmtime sous `PHASE_3`) : le Worker vérifie la sienne
//!    contre lui (§ 2.8, point 2), et l'écran ne dit pas « NON ÉPROUVÉ ».
//! 3. **Le contrôle négatif** : le premier passage, l'aide tue (`?fautif=taire-l-aide`,
//!    que la page n'entend qu'au banc) ; le scénario doit y échouer, sur l'aide et elle
//!    seule. Un contrôle qui ne voit pas son écart est une panne de l'outil (« CONTRÔLE
//!    NÉGATIF RATÉ »), jamais un succès.
//!
//! Chaque module donné de plus joue un passage court (le geste, Échap), pour qu'un jeu
//! porte Échap quand le premier ne le porte pas.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, s la session, p le passage, l une ligne, k le rang d'un module"
)]

use std::fmt;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use moteur_wasmtime::{Moteur, ReglagesMoteur};
use serde_json::Value as Json;

use crate::horloge::maintenant;
use crate::navigateur::{Interface, Lance, Navigateur};
use crate::soiree::{Ligne, PORT_DE_LA_SOIREE, Session};
use crate::soiree_native::port_libre;

/// L'adresse que le relais du banc annonce sans l'écouter (une adresse de documentation,
/// RFC 5737).
pub const ADRESSE_DU_BANC: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));

/// Ce que le scénario joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Les modules livrés : le premier joue tout ; les suivants, le geste et Échap.
    pub modules: Vec<PathBuf>,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Construire le paquet `web/` (sinon, le dernier construit).
    pub construire: bool,
    /// Le dossier de travail.
    pub travail: PathBuf,
    /// Jouer aussi le contrôle négatif.
    pub negatif: bool,
    /// Firefox avec sa fenêtre (`--avec-interface`) : sans interface, il refuse le plein
    /// écran au clic du banc, et Échap sans verrou n'y est pas constaté.
    pub interface: Interface,
}

/// Ce qu'un navigateur a fait et constaté.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur, et sa version.
    pub navigateur: String,
    /// Ce qui a été constaté, tenu.
    pub constats: Vec<String>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
    /// Le contrôle négatif : ce qui a échoué comme attendu, ou pourquoi il est raté.
    pub negatif: Option<Result<String, String>>,
    /// Le passage n'a pas pu jouer.
    pub pas_execute: Option<String>,
    /// Ce qui, dans un passage qui a joué, n'a pas pu être constaté dans ce navigateur, et
    /// pourquoi : jamais un succès, jamais une faute du commun.
    pub non_executes: Vec<String>,
}

impl fmt::Display for Passage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "\n== aide — {}", self.navigateur)?;
        if let Some(r) = &self.pas_execute {
            return writeln!(f, "pas exécuté : {r}");
        }
        for c in &self.constats {
            writeln!(f, "  constaté : {c}")?;
        }
        for x in &self.non_executes {
            writeln!(f, "  PAS EXÉCUTÉ : {x}")?;
        }
        match &self.negatif {
            Some(Ok(r)) => writeln!(
                f,
                "  contrôle négatif (l'aide tue) : rouge comme attendu — {r}"
            )?,
            Some(Err(r)) => writeln!(f, "  CONTRÔLE NÉGATIF RATÉ : {r}")?,
            None => writeln!(f, "  contrôle négatif : pas joué")?,
        }
        if self.fautes.is_empty() {
            writeln!(f, "réussi")
        } else {
            for x in &self.fautes {
                writeln!(f, "  FAUTE : {x}")?;
            }
            writeln!(f, "échec")
        }
    }
}

impl Passage {
    /// Le passage tient-il, contrôle négatif compris ?
    pub fn reussi(&self) -> bool {
        self.pas_execute.is_none()
            && self.fautes.is_empty()
            && !matches!(self.negatif, Some(Err(_)))
    }
}

/// Comment le relais d'une session se présente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reseau {
    /// Une adresse annoncée sans écoute : un QR.
    Simule,
    /// Rien d'annoncé : « Pas de réseau ».
    Aucun,
}

/// Le module a-t-il un rapport au catalogue de la session ?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Rapport {
    /// Aucun : « NON ÉPROUVÉ ».
    Aucun,
    /// Celui du banc ([`rapport_du_banc`]).
    DuBanc,
}

/// Comment une session s'ouvre.
#[derive(Debug, Clone, Copy)]
struct Ouverture<'a> {
    reseau: Reseau,
    rapport: Rapport,
    fautif: Option<&'a str>,
}

/// Le rapport d'un module, écrit par le banc : la forme de `soiree-epreuve --catalogue`,
/// l'empreinte réécrite par wasmtime sous `PHASE_3`. **Pas une épreuve** : le verdict le
/// dit ; il ne sert qu'au contrôle du Worker (§ 2.8, point 2) et à l'écran. Il se dit du
/// plan standard, au code 0, parce que le relais ne lit que ces rapports (P17, après la
/// vérification du lot 10b).
fn rapport_du_banc(module: &[u8]) -> Result<String, String> {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| format!("wasmtime : {e}"))?;
    let charge = moteur
        .charger(module)
        .map_err(|e| format!("le module ne se charge pas : {e}"))?;
    let d = charge.declaration();
    Ok(serde_json::json!({
        "empreinte": blake3::hash(module).to_hex().as_str(),
        "reecrite": blake3::Hash::from(charge.reecrit().empreinte).to_hex().as_str(),
        "reecriture": reecriture::VERSION,
        "date": "2026-11-07",
        "plan": { "nom": "standard" },
        "nom": d.nom.as_str(),
        "places": {
            "joueurs_min": d.places.joueurs_min(),
            "joueurs_max": d.places.joueurs_max(),
            "appareils_max": d.places.appareils_max(),
        },
        "code": 0,
        "verdict": "écrit par soiree-banc aide : pas une épreuve",
    })
    .to_string())
}

/// Ouvre une session : le relais (réseau simulé ou aucun), le module avec ou sans rapport,
/// le navigateur sur l'écran (avec `fautif` au besoin).
fn ouvrir(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    dossier: &Path,
    ouverture: Ouverture<'_>,
) -> Result<Session, String> {
    let Ouverture {
        reseau,
        rapport,
        fautif,
    } = ouverture;
    let _: std::io::Result<()> = std::fs::remove_dir_all(dossier);
    let jeux = dossier.join("jeux");
    std::fs::create_dir_all(&jeux).map_err(|e| format!("{e}"))?;
    let empreinte = blake3::hash(module).to_hex();
    std::fs::write(jeux.join(format!("{empreinte}.wasm")), module).map_err(|e| format!("{e}"))?;
    if rapport == Rapport::DuBanc {
        std::fs::write(
            jeux.join(format!("{empreinte}.json")),
            rapport_du_banc(module)?,
        )
        .map_err(|e| format!("{e}"))?;
    }
    let garde = dossier.join("garde");
    let enregistrement = dossier.join("enregistrement.bin");
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
        enregistrer: Some(enregistrement.clone()),
        interfaces: Some(Vec::new()),
        page: None,
        paquet: Some(relais::paquet::Paquet::lire(paquet)?),
        jeux: relais::paquet::Jeux::lire(&jeux)?,
        annonce_du_banc: (reseau == Reseau::Simule).then_some(ADRESSE_DU_BANC),
    })
    .map_err(|e| format!("le relais : {e}"))?;
    let profils = o.travail.join("profils");
    // `--avec-interface` ne vise que Firefox, qui refuse le plein écran sans fenêtre
    // (lot 10b) ; Chrome reste sans interface, comme avant le lot 13.
    let interface = if n == Navigateur::Firefox {
        o.interface
    } else {
        crate::navigateur::Interface::Sans
    };
    let mut navigateur = match Lance::lancer_avec(n, None, &profils, interface) {
        Ok(l) => l,
        Err(e) => {
            relais.arreter();
            return Err(format!("pas exécuté : {e}"));
        }
    };
    let mut url = format!("http://localhost:{port}/?jeu={empreinte}");
    if let Some(f) = fautif {
        url.push_str("&fautif=");
        url.push_str(f);
    }
    if let Err(e) = navigateur.ouvrir(&url) {
        navigateur.fermer();
        relais.arreter();
        return Err(e);
    }
    Ok(Session {
        relais: Some(relais),
        port,
        garde,
        enregistrement,
        navigateur,
        lignes: Vec::new(),
        autres: Vec::new(),
        natifs: crate::soiree::Natifs::lancer(port, maintenant(), Vec::new()),
        redite_us: None,
        salle: None,
        fragment: None,
        depart: maintenant(),
        graine: 1,
        pages: Vec::new(),
        prochain_pilotage: maintenant(),
        relais_lance: maintenant(),
        relais_us0: relais::horloge::maintenant_us(),
    })
}

/// La page d'un téléphone demandée sans WebSocket ensuite, d'un `Host` annoncé.
fn page_sans_lien(port: u16, salle: &str) -> Result<u16, String> {
    let mut f = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port))
        .map_err(|e| format!("{e}"))?;
    f.set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("{e}"))?;
    f.write_all(
        format!("GET /s/{salle} HTTP/1.1\r\nHost: {ADRESSE_DU_BANC}:{port}\r\n\r\n").as_bytes(),
    )
    .map_err(|e| format!("{e}"))?;
    let mut o = Vec::new();
    let _: std::io::Result<usize> = f.read_to_end(&mut o);
    let t = String::from_utf8_lossy(&o);
    t.split(' ')
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| format!("réponse illisible : {t}"))
}

fn champ<'a>(l: &'a Ligne, nom: &str) -> Option<&'a Json> {
    l.json.get(nom)
}

fn est_commun(l: &Ligne) -> bool {
    l.quoi() == "ecran-du-commun"
}

fn aide_de(l: &Ligne) -> Vec<String> {
    champ(l, "aide")
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Json::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn textes_de(l: &Ligne) -> Vec<String> {
    champ(l, "textes")
        .and_then(Json::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Json::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// La dernière ligne `quoi` de la session.
fn derniere<'a>(s: &'a Session, quoi: &str) -> Option<&'a Ligne> {
    s.lignes.iter().rev().find(|l| l.quoi() == quoi)
}

/// Le début : la page, l'accueil, la partie en jeu ; rend l'onglet, le code de salle et
/// l'indice de la ligne d'accueil.
fn debuter(s: &mut Session, p: &mut Passage) -> Result<(String, usize), String> {
    s.attendre(0, Duration::from_secs(60), "la page", |l| {
        l.quoi() == "page"
    })?;
    let accueil = s.attendre(0, Duration::from_secs(60), "l'accueil", |l| {
        l.quoi() == "accueil"
    })?;
    let salle = champ(&accueil, "salle")
        .and_then(Json::as_str)
        .unwrap_or("")
        .to_owned();
    let indice = s
        .lignes
        .iter()
        .position(|l| l.quoi() == "accueil")
        .unwrap_or(0);
    let en_jeu = s.attendre(0, Duration::from_secs(60), "la partie en jeu", |l| {
        l.quoi() == "en-jeu" || l.quoi() == "panne"
    })?;
    if en_jeu.quoi() == "panne" {
        p.fautes
            .push(format!("le Worker est en panne : {}", en_jeu.json));
    }
    Ok((salle, indice))
}

/// Clique sur ce que l'écran du commun propose (le bouton « Jouer », le voile), par la
/// vraie chaîne d'entrées ; attend que la page dise son geste.
fn cliquer_le_commun(
    s: &mut Session,
    p: &mut Passage,
    pourquoi: &str,
) -> Result<Option<Ligne>, String> {
    // Une chaîne : `BiDi` ne rend une valeur que si elle est primitive.
    let centre = s.evaluer(0, "JSON.stringify(window.__banc.centre())")?;
    let centre: Json = centre
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    let Some([x, y]) = centre.as_array().map(Vec::as_slice).and_then(|a| match a {
        [x, y] => Some([x.as_f64()?, y.as_f64()?]),
        _ => None,
    }) else {
        p.fautes.push(format!(
            "{pourquoi} : l'écran du commun ne propose rien à cliquer"
        ));
        return Ok(None);
    };
    // L'onglet devant, et la page qui a le focus : Firefox refuse le plein écran à un
    // onglet qui ne l'a pas.
    s.navigateur.montrer(0)?;
    let focus = s.evaluer(0, "document.hasFocus()")?;
    let depuis = s.lignes.len();
    s.navigateur.cliquer(0, x, y)?;
    let fait = s.attendre(depuis, Duration::from_secs(20), pourquoi, |l| {
        l.quoi() == "geste-fait"
    });
    match fait {
        Ok(l) => {
            p.constats.push(format!(
                "{pourquoi} : le clic en ({x:.0}, {y:.0}) est un geste : {}",
                l.json
            ));
            if champ(&l, "plein_ecran") != Some(&Json::Bool(true)) {
                // Ce que le navigateur a dit du refus, sur sa console.
                let dit: Vec<&String> = s
                    .autres
                    .iter()
                    .filter(|a| {
                        let a = a.to_lowercase();
                        a.contains("fullscreen") || a.contains("plein écran")
                    })
                    .collect();
                p.constats.push(format!(
                    "  la page avait le focus : {focus} ; la console sur le plein écran : {dit:?}"
                ));
            }
            Ok(Some(l))
        }
        Err(e) => {
            p.fautes.push(e);
            Ok(None)
        }
    }
}

/// Combien de temps le plein écran doit tenir, sans aucune touche, avant Échap (choisi,
/// sans mesure).
const TENU_SANS_ECHAP_MS: u64 = 1500;

/// Échap, juste après le geste : selon le verrou que la page dit tenir (P38).
fn echap(
    s: &mut Session,
    p: &mut Passage,
    geste: &Ligne,
    constats: &mut Constats,
) -> Result<Option<bool>, String> {
    let dedans = s.evaluer(0, "document.fullscreenElement !== null")?;
    if dedans != Json::Bool(true) && champ(geste, "plein_ecran") == Some(&Json::Bool(true)) {
        // Donné au clic, puis quitté avant tout appui (constaté au contrôle négatif de la
        // correction du lot 10b : le plein écran natif de macOS, dans Firefox avec sa
        // fenêtre) : ce n'est pas un refus, et rien ne dit qu'Échap en sortirait.
        p.fautes.push(String::from(
            "le plein écran, donné au clic, est quitté sans Échap avant l'appui : Échap n'est pas jugé",
        ));
        return Ok(Some(false));
    }
    if dedans != Json::Bool(true) {
        let refus = champ(geste, "plein_ecran").cloned().unwrap_or(Json::Null);
        p.constats.push(format!(
            "Échap : le navigateur a refusé le plein écran au clic ({refus}) ; Échap n'est pas jugé ici"
        ));
        constats.plein_refuse = Some(refus.to_string());
        return Ok(None);
    }
    // Le plein écran doit tenir sans Échap : sinon, sa sortie ne dirait rien d'Échap
    // (après la vérification du lot 10b : le plein écran natif de macOS, dans Firefox
    // avec sa fenêtre, était quitté aussitôt entré, sans aucune touche).
    s.tourner(Duration::from_millis(TENU_SANS_ECHAP_MS))?;
    if s.evaluer(0, "document.fullscreenElement !== null")? != Json::Bool(true) {
        p.fautes.push(format!(
            "le plein écran est quitté sans Échap, moins de {TENU_SANS_ECHAP_MS} ms après le geste : Échap n'est pas jugé"
        ));
        return Ok(Some(false));
    }
    let verrou = champ(geste, "verrou_echap") == Some(&Json::Bool(true));
    // L'entrée de l'écran que l'autorité a placée, avant Échap.
    let avant = derniere(s, "etat")
        .and_then(|l| champ(l, "ecran"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    let depuis = s.lignes.len();
    s.navigateur.touche_echap(0)?;
    let touche = s.attendre(depuis, Duration::from_secs(5), "la touche Échap", |l| {
        l.quoi() == "touche" && champ(l, "code").and_then(Json::as_str) == Some("Escape")
    });
    s.tourner(Duration::from_millis(2500))?;
    let au = touche
        .as_ref()
        .ok()
        .and_then(|l| champ(l, "au").and_then(Json::as_str).map(str::to_owned));
    let encore = s.evaluer(0, "document.fullscreenElement !== null")? == Json::Bool(true);
    let apres = derniere(s, "etat")
        .and_then(|l| champ(l, "ecran"))
        .and_then(Json::as_u64)
        .unwrap_or(0);
    let voile = s.lignes.iter().skip(depuis).any(|l| {
        est_commun(l) && champ(l, "montre").and_then(Json::as_str) == Some("plein-ecran-quitte")
    });
    match (verrou, au.as_deref()) {
        (_, Some("personne" | "navigateur") | None) if !verrou && !encore => {
            // Sans verrou : le navigateur a gardé Échap ; la page n'a rien envoyé.
            if voile && apres == avant {
                p.constats.push(String::from(
                    "Échap sans verrou : le plein écran est quitté, rien n'arrive au jeu, l'écran du commun montre « Plein écran quitté — cliquer pour revenir » (P38)",
                ));
                // Le voile, cliqué, rend le plein écran (après la vérification du lot 10b).
                let Some(g) = cliquer_le_commun(s, p, "« Plein écran quitté », après Échap")?
                else {
                    return Ok(Some(false));
                };
                if champ(&g, "plein_ecran") == Some(&Json::Bool(true)) {
                    p.constats.push(String::from(
                        "  le clic sur le voile, après Échap, rend le plein écran",
                    ));
                    Ok(Some(true))
                } else {
                    p.fautes.push(format!(
                        "le clic sur « Plein écran quitté », après Échap, ne rend pas le plein écran : {}",
                        g.json
                    ));
                    Ok(Some(false))
                }
            } else {
                p.fautes.push(format!(
                    "Échap sans verrou : voile {voile}, entrées de l'écran {avant} → {apres}"
                ));
                Ok(Some(false))
            }
        }
        (true, Some("jeu")) => {
            if encore && apres > avant {
                p.constats.push(format!(
                    "Échap sous Keyboard Lock : il arrive au jeu (entrées de l'écran placées {avant} → {apres}), le plein écran reste (P38)"
                ));
                Ok(Some(true))
            } else {
                p.fautes.push(format!(
                    "Échap sous Keyboard Lock : plein écran encore {encore}, entrées de l'écran {avant} → {apres}"
                ));
                Ok(Some(false))
            }
        }
        (_, Some("personne")) => {
            p.constats.push(String::from(
                "Échap : la scène de ce jeu ne porte pas de raccourci Échap ; non jugé avec ce module",
            ));
            Ok(None)
        }
        (v, a) => {
            p.fautes.push(format!(
                "Échap : verrou {v}, la page l'a donné à {a:?}, plein écran encore {encore}, voile {voile}, entrées {avant} → {apres}"
            ));
            Ok(Some(false))
        }
    }
}

/// Le clic sur « Cliquer pour reprendre » rend le son, et, la page visible, le plein écran
/// et le verrou d'écran (§ 2.4, point 7). Une page qui se dit cachée (constaté dans Chrome
/// sans interface, après le rechargement d'une page en plein écran) ne peut obtenir ni l'un
/// ni l'autre : c'est dit, non jugé.
fn juger_la_reprise(p: &mut Passage, g: &Ligne, visibilite: &Json, plein_refuse: Option<&str>) {
    let visible = visibilite.as_str() == Some("visible");
    let audio = champ(g, "son").and_then(Json::as_str) == Some("en-marche");
    let plein = champ(g, "plein_ecran") == Some(&Json::Bool(true));
    let reveil = champ(g, "reveil") == Some(&Json::Bool(true));
    if !audio {
        p.fautes.push(format!(
            "« Cliquer pour reprendre » ne rend pas le son : {}",
            g.json
        ));
    } else if plein_refuse.is_some() && reveil && !plein {
        p.constats.push(String::from(
            "le clic rend le son et le verrou d'écran ; le plein écran, que ce navigateur refuse au banc dès le premier clic, n'y est pas jugé",
        ));
    } else if visible && !(plein && reveil) {
        p.fautes.push(format!(
            "« Cliquer pour reprendre », la page visible, ne rend pas le plein écran et le verrou d'écran : {}",
            g.json
        ));
    } else if visible {
        p.constats.push(String::from(
            "le clic rend le son, le plein écran et le verrou d'écran",
        ));
    } else {
        p.constats.push(String::from(
            "le clic rend le son ; la page se dit cachée : le plein écran et le verrou d'écran n'y sont pas jugés",
        ));
    }
}

/// Les effets comptés par la page jusqu'ici : (perdus, joués, contexte).
fn son(s: &Session) -> (u64, u64, String) {
    derniere(s, "son").map_or((0, 0, String::from("?")), |l| {
        (
            champ(l, "perdus").and_then(Json::as_u64).unwrap_or(0),
            champ(l, "joues").and_then(Json::as_u64).unwrap_or(0),
            champ(l, "contexte")
                .and_then(Json::as_str)
                .unwrap_or("?")
                .to_owned(),
        )
    })
}

/// Ce qu'un passage avec réseau simulé a constaté de l'aide (pour le contrôle négatif).
#[derive(Debug, Default)]
struct Aide {
    page_sans_lien: bool,
    wifi_isole: bool,
}

/// L'aide, attendue : la page sans lien, puis aucun téléphone en 60 s.
fn juger_l_aide(
    s: &mut Session,
    p: &mut Passage,
    salle: &str,
    accueil: usize,
) -> Result<Aide, String> {
    let mut a = Aide::default();
    let statut = page_sans_lien(s.port, salle)?;
    if statut != 200 {
        p.fautes.push(format!("GET /s/{salle} : {statut}"));
    }
    let delai_page = relais::aide::PAGE_SANS_LIEN_MS.saturating_add(12_000);
    let depuis = s.lignes.len();
    match s.attendre(
        depuis,
        Duration::from_millis(delai_page),
        "« Un téléphone a ouvert la page mais n'a pas pu se relier »",
        |l| est_commun(l) && aide_de(l).iter().any(|m| m == "page-sans-lien"),
    ) {
        Ok(l) => {
            let dit = textes_de(&l)
                .iter()
                .any(|t| t.starts_with("Un téléphone a ouvert la page mais n'a pas pu se relier"));
            if dit {
                a.page_sans_lien = true;
                p.constats.push(format!(
                    "un GET /s/{salle} sans WebSocket : l'écran dit « Un téléphone a ouvert la page mais n'a pas pu se relier » ({} ms après)",
                    l.ms.saturating_sub(s.lignes.get(depuis.saturating_sub(1)).map_or(0, |x| x.ms))
                ));
            } else {
                p.fautes.push(String::from(
                    "l'aide « page sans lien » n'est pas dans les textes de l'écran",
                ));
            }
        }
        Err(e) => p.fautes.push(e),
    }
    // Aucun téléphone en 60 s après le QR (l'accueil).
    let accueil_ms = s.lignes.get(accueil).map_or(0, |l| l.ms);
    let ecoule = maintenant().saturating_duration_since(s.depart).as_millis();
    let fin = accueil_ms
        .saturating_add(u128::from(ecran_sans_telephone_ms()))
        .saturating_add(12_000);
    let reste = Duration::from_millis(u64::try_from(fin.saturating_sub(ecoule)).unwrap_or(0));
    match s.attendre(
        accueil,
        reste.max(Duration::from_secs(1)),
        "« Certains Wi-Fi isolent les appareils »",
        |l| est_commun(l) && aide_de(l).iter().any(|m| m == "wifi-isole"),
    ) {
        Ok(l) => {
            let apres = l.ms.saturating_sub(accueil_ms);
            if u128::from(ecran_sans_telephone_ms()) > apres {
                p.fautes.push(format!(
                    "« le Wi-Fi isole » dit trop tôt : {apres} ms après le QR"
                ));
            } else if textes_de(&l)
                .iter()
                .any(|t| t.contains("Certains Wi-Fi isolent les appareils"))
            {
                a.wifi_isole = true;
                p.constats.push(format!(
                    "aucun téléphone : « Certains Wi-Fi isolent les appareils » à l'écran, {apres} ms après le QR (SAL-99)"
                ));
            } else {
                p.fautes.push(String::from(
                    "l'aide « Wi-Fi isolé » n'est pas dans les textes de l'écran",
                ));
            }
        }
        Err(e) => p.fautes.push(e),
    }
    Ok(a)
}

/// La minute de l'aide, une seule définition (`ecran::aide::SANS_TELEPHONE_MS`, que le
/// banc ne tire pas : `web/` est un autre workspace). Tenue égale par un test du banc
/// qui lit la source de `web/ecran/src/aide.rs`.
pub const fn ecran_sans_telephone_ms() -> u64 {
    60_000
}

/// Le premier passage : le réseau simulé, l'aide, le son, le geste, Échap, le
/// rechargement. Rend l'aide constatée.
fn passage_reseau(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    p: &mut Passage,
    fautif: Option<&str>,
    constats: &mut Constats,
) -> Result<Aide, String> {
    let dossier = o.travail.join(format!(
        "{}-reseau{}",
        n.nom(),
        if fautif.is_some() { "-negatif" } else { "" }
    ));
    let ouverture = Ouverture {
        reseau: Reseau::Simule,
        rapport: Rapport::Aucun,
        fautif,
    };
    let mut s = ouvrir(o, n, paquet, module, &dossier, ouverture)?;
    p.navigateur = s.navigateur.version().to_owned();
    let r = jouer_le_reseau(&mut s, p, fautif, constats);
    s.fermer();
    r
}

/// Ce que joue [`passage_reseau`], la session ouverte.
fn jouer_le_reseau(
    s: &mut Session,
    p: &mut Passage,
    fautif: Option<&str>,
    constats: &mut Constats,
) -> Result<Aide, String> {
    let (salle, accueil) = debuter(s, p)?;
    // L'écran de démarrage : le QR, « NON ÉPROUVÉ ».
    let demarrage = s.attendre(0, Duration::from_secs(30), "l'écran de démarrage", |l| {
        est_commun(l)
            && champ(l, "montre").and_then(Json::as_str) == Some("demarrage")
            && champ(l, "qr") == Some(&Json::Bool(true))
            && champ(l, "non_eprouve") == Some(&Json::Bool(true))
    });
    match demarrage {
        Ok(l) if textes_de(&l).iter().any(|t| t.contains("NON ÉPROUVÉ")) => {
            p.constats.push(format!(
                "l'écran de démarrage : le QR, la salle {salle}, le module « NON ÉPROUVÉ » (P17)"
            ));
        }
        Ok(l) => p.fautes.push(format!(
            "l'écran de démarrage ne dit pas « NON ÉPROUVÉ » : {}",
            l.json
        )),
        Err(e) => p.fautes.push(e),
    }
    let aide = juger_l_aide(s, p, &salle, accueil)?;
    if fautif.is_some() {
        return Ok(aide);
    }
    // Le son avant le geste.
    let (perdus, joues, contexte) = son(s);
    if perdus > 0 && joues == 0 && contexte == "aucun" {
        constats.effet_perdu = true;
        p.constats.push(format!(
            "avant le geste : {perdus} effet(s) compté(s) perdu(s), aucun joué, aucun contexte audio (SAL-174, SAL-175)"
        ));
    } else if perdus == 0 && joues == 0 {
        p.constats.push(String::from(
            "avant le geste : ce jeu n'a émis aucun effet ; non jugé avec ce module",
        ));
    } else {
        p.fautes.push(format!(
            "avant le geste : {perdus} perdu(s), {joues} joué(s), contexte {contexte}"
        ));
    }
    // Le geste, puis Échap.
    if let Some(g) = cliquer_le_commun(s, p, "« Jouer »")?
        && let Some(ok) = echap(s, p, &g, constats)?
    {
        constats.echap |= ok;
    }
    sortir_du_plein_ecran(s, p)?;
    // L'onglet rechargé : « Cliquer pour reprendre ».
    let depuis = s.lignes.len();
    s.navigateur.recharger(0)?;
    let voile = s.attendre(
        depuis,
        Duration::from_secs(60),
        "« Cliquer pour reprendre »",
        |l| est_commun(l) && champ(l, "montre").and_then(Json::as_str) == Some("reprendre"),
    );
    let reprise = s.attendre(depuis, Duration::from_secs(60), "la partie reprise", |l| {
        l.quoi() == "en-jeu"
    });
    match (voile, reprise) {
        (Ok(_), Ok(r)) if champ(&r, "relue").is_some_and(|v| !v.is_null()) => {
            // L'onglet devant, comme l'hôte le verrait.
            s.navigateur.montrer(0)?;
            s.tourner(Duration::from_millis(500))?;
            let etat = s.evaluer(0, "document.visibilityState")?;
            p.constats.push(format!(
                "rechargé : « Cliquer pour reprendre » sur la partie reprise de la garde (§ 2.4, point 7) ; la page dit {etat}"
            ));
            if let Some(g) = cliquer_le_commun(s, p, "« Cliquer pour reprendre »")? {
                juger_la_reprise(p, &g, &etat, constats.plein_refuse.as_deref());
            }
        }
        (v, r) => p
            .fautes
            .push(format!("rechargé : voile {v:?}, reprise {r:?}")),
    }
    Ok(aide)
}

/// Le passage sans réseau : le relais démarre, sans QR, et le dit ; le module a son
/// rapport, que le Worker vérifie.
fn passage_sans_reseau(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    p: &mut Passage,
) -> Result<(), String> {
    let dossier = o.travail.join(format!("{}-sans-reseau", n.nom()));
    let ouverture = Ouverture {
        reseau: Reseau::Aucun,
        rapport: Rapport::DuBanc,
        fautif: None,
    };
    let mut s = ouvrir(o, n, paquet, module, &dossier, ouverture)?;
    let r = jouer_sans_reseau(&mut s, p);
    s.fermer();
    r
}

/// Ce que joue [`passage_sans_reseau`], la session ouverte.
fn jouer_sans_reseau(s: &mut Session, p: &mut Passage) -> Result<(), String> {
    let _: (String, usize) = debuter(s, p)?;
    let accueil = derniere(s, "accueil").cloned();
    if accueil
        .as_ref()
        .and_then(|l| champ(l, "adresses"))
        .and_then(Json::as_u64)
        != Some(0)
    {
        p.fautes.push(format!(
            "sans réseau, l'accueil annonce des adresses : {accueil:?}"
        ));
    }
    match s.attendre(0, Duration::from_secs(30), "« Pas de réseau »", |l| {
        est_commun(l) && aide_de(l).iter().any(|m| m == "pas-de-reseau")
    }) {
        Ok(l) => {
            let qr = champ(&l, "qr") == Some(&Json::Bool(true));
            let dit = textes_de(&l).iter().any(|t| t.starts_with("Pas de réseau"));
            let non_eprouve = champ(&l, "non_eprouve") == Some(&Json::Bool(true));
            if !qr && dit && !non_eprouve {
                p.constats.push(String::from(
                    "sans adresse locale : le relais démarre, l'écran n'a pas de QR et dit « Pas de réseau » ; le module a son rapport, son empreinte réécrite vérifiée par le Worker, et l'écran ne dit pas « NON ÉPROUVÉ »",
                ));
            } else {
                p.fautes.push(format!(
                    "sans réseau : QR {qr}, dit {dit}, non éprouvé {non_eprouve}"
                ));
            }
        }
        Err(e) => p.fautes.push(e),
    }
    if let Some(l) = derniere(s, "panne") {
        p.fautes
            .push(format!("une panne, sans réseau : {}", l.json));
    }
    Ok(())
}

/// Un passage court d'un autre module : le geste, Échap, le son.
fn passage_court(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    k: usize,
    p: &mut Passage,
    constats: &mut Constats,
) -> Result<(), String> {
    let dossier = o.travail.join(format!("{}-module-{k}", n.nom()));
    let ouverture = Ouverture {
        reseau: Reseau::Aucun,
        rapport: Rapport::Aucun,
        fautif: None,
    };
    let mut s = ouvrir(o, n, paquet, module, &dossier, ouverture)?;
    let r = jouer_court(&mut s, k, p, constats);
    s.fermer();
    r
}

/// Ce que joue [`passage_court`], la session ouverte.
fn jouer_court(
    s: &mut Session,
    k: usize,
    p: &mut Passage,
    constats: &mut Constats,
) -> Result<(), String> {
    let _: (String, usize) = debuter(s, p)?;
    s.attendre(0, Duration::from_secs(30), "l'écran de démarrage", |l| {
        est_commun(l) && champ(l, "montre").and_then(Json::as_str) == Some("demarrage")
    })?;
    s.tourner(Duration::from_secs(2))?;
    let (perdus, joues, _) = son(s);
    if perdus > 0 && joues == 0 {
        constats.effet_perdu = true;
    }
    if let Some(g) = cliquer_le_commun(s, p, &format!("« Jouer » (module {k})"))?
        && let Some(ok) = echap(s, p, &g, constats)?
    {
        constats.echap |= ok;
    }
    sortir_du_plein_ecran(s, p)
}

/// Le plein écran quitté autrement que par Échap (`document.exitFullscreen()`, comme
/// l'hôte le quitterait par le menu) : l'écran du commun montre « Plein écran quitté —
/// cliquer pour revenir », et le clic sur le voile rend le plein écran.
fn sortir_du_plein_ecran(s: &mut Session, p: &mut Passage) -> Result<(), String> {
    if s.evaluer(0, "document.fullscreenElement !== null")? != Json::Bool(true) {
        return Ok(());
    }
    let depuis = s.lignes.len();
    let _: Json = s.evaluer(0, "document.exitFullscreen().then(() => true)")?;
    match s.attendre(
        depuis,
        Duration::from_secs(10),
        "« Plein écran quitté »",
        |l| {
            est_commun(l) && champ(l, "montre").and_then(Json::as_str) == Some("plein-ecran-quitte")
        },
    ) {
        Ok(_) => {
            if let Some(g) =
                cliquer_le_commun(s, p, "« Plein écran quitté — cliquer pour revenir »")?
            {
                if champ(&g, "plein_ecran") == Some(&Json::Bool(true)) {
                    p.constats.push(String::from(
                        "le plein écran quitté (par document.exitFullscreen) : le voile « Plein écran quitté — cliquer pour revenir », dont le clic le rend",
                    ));
                } else {
                    p.fautes.push(format!(
                        "le clic sur « Plein écran quitté » ne rend pas le plein écran : {}",
                        g.json
                    ));
                }
            }
        }
        Err(e) => p.fautes.push(e),
    }
    Ok(())
}

/// Ce qui doit être constaté au moins une fois par navigateur, sur l'un des modules.
#[derive(Debug, Default)]
struct Constats {
    echap: bool,
    effet_perdu: bool,
    /// Ce que le navigateur a répondu au plein écran quand il l'a refusé à un vrai clic.
    plein_refuse: Option<String>,
}

/// Joue le scénario dans chaque navigateur.
pub fn jouer(o: &Options, soiree: &Path) -> Result<Vec<Passage>, String> {
    let paquet = if o.construire {
        crate::soiree::construire_le_paquet(soiree)?
    } else {
        crate::soiree::dernier_paquet(soiree)?
    };
    let modules: Vec<Vec<u8>> = o
        .modules
        .iter()
        .map(|m| std::fs::read(m).map_err(|e| format!("{} : {e}", m.display())))
        .collect::<Result<_, _>>()?;
    let Some(premier) = modules.first() else {
        return Err(String::from("aucun module"));
    };
    let mut passages = Vec::new();
    for n in &o.navigateurs {
        let mut p = Passage {
            navigateur: n.nom().to_owned(),
            ..Passage::default()
        };
        let mut constats = Constats::default();
        let r = passage_reseau(o, *n, &paquet, premier, &mut p, None, &mut constats)
            .and_then(|_| passage_sans_reseau(o, *n, &paquet, premier, &mut p))
            .and_then(|()| {
                for (k, m) in modules.iter().enumerate().skip(1) {
                    passage_court(o, *n, &paquet, m, k, &mut p, &mut constats)?;
                }
                Ok(())
            });
        match r {
            Err(e) if e.starts_with("pas exécuté") => {
                p.pas_execute = Some(e);
                passages.push(p);
                continue;
            }
            Err(e) => p.fautes.push(e),
            Ok(()) => {}
        }
        if !constats.echap
            && let Some(refus) = &constats.plein_refuse
        {
            // Seul Firefox sans interface a une raison de refuser, que le banc ne lève pas
            // (lot 10b) ; ailleurs, et avec sa fenêtre, un refus est une faute, jamais un
            // « pas exécuté » (après la vérification du lot 10b).
            if (o.interface, *n) == (Interface::Sans, Navigateur::Firefox) {
                p.non_executes.push(format!(
                    "Échap en plein écran : {} sans interface refuse le plein écran à un vrai clic du banc ({refus}) ; le comportement d'Échap n'y est pas constaté (le constater : soiree-banc aide … --avec-interface)",
                    p.navigateur
                ));
            } else {
                p.fautes.push(format!(
                    "Échap en plein écran : {} refuse le plein écran à un vrai clic du banc ({refus}){} ; Échap n'est pas jugé",
                    p.navigateur,
                    if o.interface == Interface::Avec && *n == Navigateur::Firefox {
                        ", avec sa fenêtre"
                    } else {
                        ""
                    }
                ));
            }
        } else if !constats.echap {
            p.fautes.push(String::from(
                "Échap en plein écran n'a été jugé sur aucun module (il faut un jeu dont la scène porte un raccourci Échap, Tier list)",
            ));
        }
        if !constats.effet_perdu {
            p.fautes.push(String::from(
                "aucun effet tombé avant le geste n'a été constaté perdu (il faut un jeu qui en émet un à sa première scène, Punchline)",
            ));
        }
        if o.negatif {
            let mut q = Passage::default();
            let mut c = Constats::default();
            let r = passage_reseau(
                o,
                *n,
                &paquet,
                premier,
                &mut q,
                Some("taire-l-aide"),
                &mut c,
            );
            p.negatif = Some(match r {
                Ok(a) if !a.page_sans_lien && !a.wifi_isole && !q.fautes.is_empty() => {
                    let aide_seule = q
                        .fautes
                        .iter()
                        .all(|f| f.contains("pas pu se relier") || f.contains("Wi-Fi isolent"));
                    if aide_seule {
                        Ok(format!("{} faute(s), toutes de l'aide tue", q.fautes.len()))
                    } else {
                        Err(format!("il échoue pour une autre raison : {:?}", q.fautes))
                    }
                }
                Ok(a) => Err(format!(
                    "l'aide tue n'a pas été vue : {a:?}, fautes {:?}",
                    q.fautes
                )),
                Err(e) => Err(format!("l'outil n'a pas joué le contrôle négatif : {e}")),
            });
        }
        passages.push(p);
    }
    Ok(passages)
}

#[cfg(test)]
mod tests {
    use super::ecran_sans_telephone_ms;

    /// La minute de l'aide est celle de l'écran (`web/ecran/src/aide.rs`) : une seule
    /// définition, relue (le banc ne tire pas le workspace `web/`).
    #[test]
    fn la_minute_de_l_aide_est_celle_de_l_ecran() {
        let source = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/ecran/src/aide.rs"),
        )
        .unwrap();
        let attendu = format!(
            "pub const SANS_TELEPHONE_MS: u64 = {};",
            ecran_sans_telephone_ms()
                .to_string()
                .as_bytes()
                .rchunks(3)
                .rev()
                .map(|c| std::str::from_utf8(c).unwrap())
                .collect::<Vec<_>>()
                .join("_")
        );
        assert!(source.contains(&attendu), "{attendu}");
    }
}
