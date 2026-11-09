//! Le scénario `soiree-complete` (PHASE-3, § 7, ligne du lot 14 ; § 8.2) : **une soirée
//! entière, comme l'hôte la joue**. Le relais lancé une fois, les jeux au catalogue ;
//! l'écran ouvert une fois, sur l'écran de démarrage ; des téléphones natifs qui passent
//! par la page (et, avec `--pages`, des pages dans de vrais navigateurs). « Jouer » par un
//! vrai clic ; puis chaque jeu à la suite : « On rejoue » deux fois (la sortie
//! `NouvellePartie` du jeu, que son organisateur simulé presse à la fin), puis F9, la ligne
//! du jeu suivant, « Jouer » (le changement que juge [`crate::changer`]).
//!
//! Ce que le banc juge, en plus de chaque changement :
//!
//! - **chaque « On rejoue »** : le numéro de la partie que le Worker dit (`partie`, son
//!   rapport d'état) monte de un, puis de deux, depuis l'ouverture du jeu ; un jeu qui
//!   déclare ne pas avoir de fin (`fin: SansObjet`, Tier list) n'en a pas, et c'est dit ;
//! - **le jour J tient** après le geste, après chaque « On rejoue » et après chaque
//!   changement : la page est en plein écran (`document.fullscreenElement`, relu par le
//!   banc), elle ne l'a jamais quitté depuis le geste (aucune ligne `plein-ecran` hors du
//!   plein écran), l'écran du commun ne montre rien par-dessus le jeu (ni voile, ni écran de
//!   démarrage), et, si Échap était verrouillé au geste (Chrome, P38), il l'est encore ;
//! - à la fin, la partie figée est celle que wasmtime reprend de la garde sur disque avec
//!   le module du dernier jeu.
//!
//! Un navigateur qui refuse le plein écran au clic du banc (Firefox sans interface) ne voit
//! pas ce point jugé, et le rapport le dit ; le reste l'est.
//!
//! Le contrôle négatif : `--fautif oublie-le-plein-ecran` (`?fautif=`, la page au banc
//! seulement) : après « On rejoue », la page sort du plein écran sans voile ni nouvelle
//! demande. Le scénario doit alors y voir une faute du plein écran après « On rejoue » ;
//! sinon, « CONTRÔLE NÉGATIF RATÉ ». Ce fautif oublie au premier « On rejoue » ; la faute
//! notée après le second n'est que la même sortie, encore visible. *Lot 14 corrigé* :
//! `--fautif oublie-le-plein-ecran-au-second` oublie au second seulement, et le passage ne
//! « réussit » que si le plein écran est vu tenu après le premier et perdu après le second.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, s la session, p le passage, l une ligne"
)]

use std::path::Path;
use std::time::Duration;

use jeu_abi::Oui;
use moteur_wasmtime::ModuleCharge;
use serde_json::Value as Json;

use crate::changer::{changer_vers, cliquer, juger_la_garde_finale, nom_du_jeu};
use crate::navigateur::Navigateur;
use crate::soiree::{
    Ligne, Options, Passage, Session, charger_le_module, controler_les_lignes, debuter,
    ecrire_les_lignes, figer, juger_les_natifs, ouvrir, ouvrir_les_pages, telephones_et_redite,
};

/// Le fautif du contrôle négatif (lot 14) : le plein écran oublié au premier « On rejoue ».
pub const FAUTIF: &str = "oublie-le-plein-ecran";

/// Le même, oublié au second « On rejoue » seulement (lot 14 corrigé).
pub const FAUTIF_AU_SECOND: &str = "oublie-le-plein-ecran-au-second";

/// La plus longue attente d'une nouvelle partie (« On rejoue ») : 300 s (choisi ; une
/// partie de Punchline à six téléphones natifs dure environ une minute au lot 5, trois
/// phases terminales en 180 s, et Sidequest finit en deux questions).
const REJOUE_MAX: Duration = Duration::from_secs(300);

/// Ce que dure un jeu sans fin (Tier list) avant le suivant : 30 s (choisi).
const JEU_SANS_FIN: Duration = Duration::from_secs(30);

/// Le temps laissé à la page après un « On rejoue » ou un changement, avant de relire le
/// plein écran : 1,5 s (choisi ; le rapport d'état du Worker part une fois par seconde, et
/// le fautif agit à celui qui suit la partie neuve).
const APRES: Duration = Duration::from_millis(1_500);

/// Les « On rejoue » joués par jeu (la ligne du lot 14).
const REJOUES: u64 = 2;

/// Ce que le geste a donné : le plein écran, et le verrou d'Échap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Geste {
    /// Le navigateur a donné le plein écran au clic : il se juge.
    pub plein: bool,
    /// Échap verrouillé (Keyboard Lock, Chrome).
    pub verrou: bool,
}

fn champ<'a>(l: &'a Ligne, nom: &str) -> Option<&'a Json> {
    l.json.get(nom)
}

/// La faute du jour J, s'il ne tient pas à `moment` : `dedans` est ce que le banc relit de
/// `document.fullscreenElement` ; `lignes` sont celles de l'onglet de l'écran depuis le
/// geste. `None` si tout tient.
pub fn faute_du_plein_ecran(
    moment: &str,
    geste: Geste,
    dedans: Oui,
    lignes: &[Ligne],
) -> Option<String> {
    if !geste.plein {
        return None;
    }
    let mut pourquoi = Vec::new();
    if dedans == Oui::Non {
        pourquoi.push(String::from(
            "la page n'est pas en plein écran (document.fullscreenElement est nul)",
        ));
    }
    if let Some(l) = lignes
        .iter()
        .find(|l| l.quoi() == "plein-ecran" && champ(l, "dedans") == Some(&Json::Bool(false)))
    {
        pourquoi.push(format!("le plein écran a été quitté à {} ms", l.ms));
    }
    if let Some(l) = lignes
        .iter()
        .find(|l| l.quoi() == "fautif-plein-ecran-oublie")
    {
        pourquoi.push(format!(
            "la page dit l'avoir oublié (fautif) à {} ms, partie {}",
            l.ms,
            champ(l, "partie").map_or_else(|| String::from("?"), Json::to_string)
        ));
    }
    if let Some(c) = lignes.iter().rev().find(|l| l.quoi() == "ecran-du-commun") {
        let montre = champ(c, "montre").and_then(Json::as_str).unwrap_or("?");
        if montre != "rien" {
            pourquoi.push(format!("l'écran du commun montre « {montre} » sur le jeu"));
        }
        if geste.verrou && champ(c, "verrou") != Some(&Json::Bool(true)) {
            pourquoi.push(String::from("Échap n'est plus verrouillé"));
        }
    }
    (!pourquoi.is_empty()).then(|| format!("{moment} : {}", pourquoi.join(" ; ")))
}

/// Le numéro de partie d'une ligne `etat`.
fn partie(l: &Ligne) -> Option<u64> {
    (l.quoi() == "etat")
        .then(|| champ(l, "partie").and_then(Json::as_u64))
        .flatten()
}

/// Les phases terminales que les rapports d'état ont vues (une fois par seconde : une phase
/// terminale plus courte peut manquer), toutes parties et tous onglets confondus : chaque
/// passage d'un rapport non terminal à un rapport terminal.
fn terminales_vues(lignes: &[Ligne]) -> u32 {
    let mut avant = false;
    let mut n = 0u32;
    for l in lignes.iter().filter(|l| l.quoi() == "etat") {
        let t = champ(l, "terminale") == Some(&Json::Bool(true));
        if t && !avant {
            n = n.saturating_add(1);
        }
        avant = t;
    }
    n
}

/// Relit le plein écran, et juge le jour J à `moment`.
fn constater(
    s: &mut Session,
    p: &mut Passage,
    (onglet, depuis_le_geste): (&str, usize),
    geste: Geste,
    moment: &str,
) -> Result<(), String> {
    s.tourner(APRES)?;
    // Une chaîne : `BiDi` ne rend une valeur que si elle est primitive.
    let dedans = if s
        .evaluer(0, "String(document.fullscreenElement !== null)")?
        .as_str()
        == Some("true")
    {
        Oui::Oui
    } else {
        Oui::Non
    };
    let lignes: Vec<Ligne> = s
        .lignes
        .iter()
        .skip(depuis_le_geste)
        .filter(|l| l.onglet == onglet)
        .cloned()
        .collect();
    match faute_du_plein_ecran(moment, geste, dedans, &lignes) {
        Some(f) => p.fautes.push(f),
        None if geste.plein => p
            .notes
            .push(format!("{moment} : plein écran tenu, rien sur le jeu")),
        None => {}
    }
    Ok(())
}

/// Les « On rejoue » d'un jeu, depuis la ligne `ouverture` (la partie du jeu ouverte : le
/// geste pour le premier, la ligne `change` pour les autres).
fn rejouer(
    s: &mut Session,
    p: &mut Passage,
    (onglet, depuis_le_geste, ouverture): (&str, usize, usize),
    (nom, charge): (&str, &ModuleCharge),
    geste: Geste,
) -> Result<(), String> {
    if let jeu_abi::ApplicableFil::SansObjet(r) = &charge.declaration().fin {
        p.notes.push(format!(
            "{nom} : « On rejoue » sans objet ({}) ; joué {} s",
            r.as_str(),
            JEU_SANS_FIN.as_secs()
        ));
        s.tourner(JEU_SANS_FIN)?;
        return constater(
            s,
            p,
            (onglet, depuis_le_geste),
            geste,
            &format!("{nom}, après {} s de jeu", JEU_SANS_FIN.as_secs()),
        );
    }
    let debut = s.lignes.get(ouverture).map_or(0, |l| l.ms);
    for k in 1..=REJOUES {
        let o = onglet.to_owned();
        let vue = s.attendre(
            ouverture,
            REJOUE_MAX,
            &format!("{nom} : « On rejoue » n° {k} (la partie {k})"),
            move |l| l.onglet == o && partie(l).is_some_and(|n| n >= k),
        )?;
        // L'heure de la ligne lue par le banc : le rapport d'état part une fois par
        // seconde, et le banc peut lire en retard (pendant un changement de jeu).
        p.notes.push(format!(
            "{nom} : « On rejoue » n° {k} vu {:.1} s après l'ouverture du jeu (le rapport d'état, une fois par seconde)",
            Duration::from_millis(u64::try_from(vue.ms.saturating_sub(debut)).unwrap_or(u64::MAX))
                .as_secs_f64()
        ));
        constater(
            s,
            p,
            (onglet, depuis_le_geste),
            geste,
            &format!("{nom}, après « On rejoue » n° {k}"),
        )?;
    }
    Ok(())
}

/// Le geste « Jouer », par un vrai clic, et ce qu'il a donné.
fn jouer(s: &mut Session, p: &mut Passage) -> Result<(Geste, usize), String> {
    let depuis = s.lignes.len();
    cliquer(s, None, "« Jouer »")?;
    let g = s.attendre(depuis, Duration::from_secs(20), "le geste", |l| {
        l.quoi() == "geste-fait"
    })?;
    let geste = Geste {
        plein: champ(&g, "plein_ecran") == Some(&Json::Bool(true)),
        verrou: champ(&g, "verrou_echap") == Some(&Json::Bool(true)),
    };
    if !geste.plein {
        p.notes.push(format!(
            "le navigateur a refusé le plein écran au clic du banc ({}) : le plein écran n'est pas jugé ici",
            champ(&g, "plein_ecran").map_or_else(|| String::from("?"), Json::to_string)
        ));
    }
    Ok((geste, depuis))
}

/// Un passage de `soiree-complete` : chaque jeu, « On rejoue » deux fois, puis le suivant.
pub(crate) fn soiree_complete(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    module: &[u8],
    charge: &ModuleCharge,
) -> Result<Passage, String> {
    let (telephones, redite) = telephones_et_redite(o, charge);
    let dossier = o.travail.join(n.nom());
    let mut s = ouvrir(o, n, paquet, module, &dossier, (redite, ""))?;
    let mut p = Passage {
        navigateur: s.navigateur.version().to_owned(),
        port: s.port,
        chaine_non_relue: true,
        ..Passage::default()
    };
    p.notes.push(crate::navigateur::note_de_la_session());
    let mut jeux: Vec<(Vec<u8>, ModuleCharge)> = Vec::new();
    for autre in &o.autres_jeux {
        let octets = std::fs::read(autre).map_err(|e| format!("{} : {e}", autre.display()))?;
        let c = charger_le_module(&octets)?;
        jeux.push((octets, c));
    }
    let mut dernier = charger_le_module(module)?;
    let mut plein_juge = Oui::Non;
    let deroule = (|| -> Result<Option<Ligne>, String> {
        let onglet = debuter(&mut s, &mut p, telephones)?;
        if !o.pages.is_empty() {
            ouvrir_les_pages(o, &mut s, &mut p)?;
        }
        let (geste, depuis_le_geste) = jouer(&mut s, &mut p)?;
        plein_juge = if geste.plein { Oui::Oui } else { Oui::Non };
        let premier = nom_du_jeu(charge);
        constater(
            &mut s,
            &mut p,
            (&onglet, depuis_le_geste),
            geste,
            &format!("{premier}, après « Jouer »"),
        )?;
        rejouer(
            &mut s,
            &mut p,
            (&onglet, depuis_le_geste, depuis_le_geste),
            (&premier, charge),
            geste,
        )?;
        for (octets, c) in jeux.drain(..) {
            let cible = *blake3::hash(&octets).as_bytes();
            let nom = nom_du_jeu(&c);
            let tel = matches!(c.declaration().telephones, jeu_abi::ApplicableFil::Oui(_));
            let avant = s.lignes.len();
            changer_vers(&mut s, &mut p, &onglet, (&cible, &nom, tel))?;
            let ouverture = s
                .lignes
                .iter()
                .enumerate()
                .skip(avant)
                .find(|(_, l)| l.onglet == onglet && l.quoi() == "change")
                .map_or(avant, |(k, _)| k);
            constater(
                &mut s,
                &mut p,
                (&onglet, depuis_le_geste),
                geste,
                &format!("{nom}, après le changement"),
            )?;
            rejouer(
                &mut s,
                &mut p,
                (&onglet, depuis_le_geste, ouverture),
                (&nom, &c),
                geste,
            )?;
            dernier = c;
        }
        figer(&mut s, 0, &onglet).map(Some)
    })();
    let fige = match deroule {
        Ok(f) => f,
        Err(e) => {
            p.fautes.push(e);
            None
        }
    };
    p.terminales = terminales_vues(&s.lignes);
    juger_les_natifs(&s, &mut p);
    controler_les_lignes(&s, &mut p, &[]);
    juger_la_garde_finale(&mut s, &mut p, fige, &dernier);
    match o.fautif.as_deref() {
        Some(FAUTIF) => juger_le_negatif(&mut p, plein_juge, Oui::Non),
        Some(FAUTIF_AU_SECOND) => juger_le_negatif(&mut p, plein_juge, Oui::Oui),
        _ => {}
    }
    p.lignes = s.lignes.len();
    ecrire_les_lignes(&dossier, &s.lignes, &s.autres);
    s.fermer();
    Ok(p)
}

/// Le contrôle négatif : sous le fautif, le passage doit avoir vu le plein écran perdu
/// après « On rejoue », et rien d'autre ; il est alors « réussi » (le négatif a vu son
/// défaut), sinon en échec (« CONTRÔLE NÉGATIF RATÉ »). Sous le fautif du second
/// (`au_second`), le plein écran doit être perdu après « On rejoue » n° 2, et une faute
/// après le n° 1 reste une faute (le fautif n'a encore rien fait). Un navigateur qui ne
/// donne pas le plein écran au banc ne peut pas le voir : pas exécuté.
pub fn juger_le_negatif(p: &mut Passage, plein_juge: Oui, au_second: Oui) {
    if plein_juge == Oui::Non {
        p.pas_execute = Some(String::from(
            "pas exécuté : le navigateur refuse le plein écran au clic du banc, le contrôle négatif ne peut rien voir",
        ));
        return;
    }
    let attendu = if au_second == Oui::Oui {
        "après « On rejoue » n° 2"
    } else {
        "après « On rejoue »"
    };
    let (vues, autres): (Vec<String>, Vec<String>) = p
        .fautes
        .drain(..)
        .partition(|f| f.contains(attendu) && f.contains("plein écran"));
    if vues.is_empty() {
        p.fautes.push(format!(
            "CONTRÔLE NÉGATIF RATÉ : le plein écran oublié {attendu} n'a pas été vu"
        ));
    } else {
        for v in vues {
            p.notes.push(format!("contrôle négatif vu : {v}"));
        }
    }
    p.fautes.extend(autres);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ligne(ms: u128, json: &str) -> Ligne {
        Ligne {
            onglet: String::from("cee52f63"),
            json: serde_json::from_str(json).expect("json"),
            ms,
        }
    }

    /// `verrou` : `"true"` ou `"false"`, tel que la page l'écrit.
    fn commun(montre: &str, verrou: &str) -> String {
        format!(
            "{{\"quoi\":\"ecran-du-commun\",\"montre\":\"{montre}\",\"plein_ecran\":true,\"verrou\":{verrou}}}"
        )
    }

    const CHROME: Geste = Geste {
        plein: true,
        verrou: true,
    };

    #[test]
    fn le_jour_j_tient_ou_dit_ce_qui_manque() {
        let tient = [
            ligne(1, "{\"quoi\":\"plein-ecran\",\"dedans\":true}"),
            ligne(2, &commun("rien", "true")),
        ];
        assert_eq!(
            faute_du_plein_ecran("après « Jouer »", CHROME, Oui::Oui, &tient),
            None
        );
        // Relu hors du plein écran.
        let f = faute_du_plein_ecran("après « On rejoue » n° 1", CHROME, Oui::Non, &tient)
            .expect("une faute");
        assert!(f.contains("fullscreenElement est nul"), "{f}");
        assert!(f.starts_with("après « On rejoue » n° 1 : "), "{f}");
        // Quitté puis rendu : la sortie reste une faute.
        let quitte = [
            ligne(1, "{\"quoi\":\"plein-ecran\",\"dedans\":true}"),
            ligne(5, "{\"quoi\":\"plein-ecran\",\"dedans\":false}"),
            ligne(6, "{\"quoi\":\"plein-ecran\",\"dedans\":true}"),
            ligne(7, &commun("rien", "true")),
        ];
        let f = faute_du_plein_ecran("m", CHROME, Oui::Oui, &quitte).expect("une faute");
        assert!(f.contains("quitté à 5 ms"), "{f}");
        // Le voile, ou l'écran de démarrage, sur le jeu ; Échap déverrouillé.
        let voile = [ligne(1, &commun("plein-ecran-quitte", "false"))];
        let f = faute_du_plein_ecran("m", CHROME, Oui::Oui, &voile).expect("une faute");
        assert!(f.contains("« plein-ecran-quitte »"), "{f}");
        assert!(f.contains("Échap n'est plus verrouillé"), "{f}");
        // Firefox : pas de verrou au geste, rien à reprocher au verrou.
        let firefox = Geste {
            plein: true,
            verrou: false,
        };
        let sans_verrou = [ligne(1, &commun("rien", "false"))];
        assert_eq!(
            faute_du_plein_ecran("m", firefox, Oui::Oui, &sans_verrou),
            None
        );
        // Le fautif se nomme.
        let fautif = [
            ligne(1, &commun("rien", "true")),
            ligne(9, "{\"quoi\":\"fautif-plein-ecran-oublie\",\"partie\":1}"),
        ];
        let f = faute_du_plein_ecran("m", CHROME, Oui::Non, &fautif).expect("une faute");
        assert!(f.contains("oublié (fautif) à 9 ms, partie 1"), "{f}");
    }

    #[test]
    fn sans_plein_ecran_au_clic_rien_ne_se_juge() {
        let refuse = Geste {
            plein: false,
            verrou: false,
        };
        let lignes = [ligne(1, &commun("plein-ecran-quitte", "false"))];
        assert_eq!(faute_du_plein_ecran("m", refuse, Oui::Non, &lignes), None);
    }

    #[test]
    fn le_negatif_doit_voir_le_plein_ecran_oublie_apres_on_rejoue() {
        let mut vu = Passage {
            fautes: vec![
                String::from(
                    "Punchline, après « On rejoue » n° 1 : la page n'est pas en plein écran (document.fullscreenElement est nul)",
                ),
                String::from("autre chose"),
            ],
            ..Passage::default()
        };
        juger_le_negatif(&mut vu, Oui::Oui, Oui::Non);
        assert_eq!(vu.fautes, vec![String::from("autre chose")]);
        assert!(
            vu.notes
                .iter()
                .any(|n| n.starts_with("contrôle négatif vu"))
        );
        let mut rate = Passage::default();
        juger_le_negatif(&mut rate, Oui::Oui, Oui::Non);
        assert!(
            rate.fautes
                .iter()
                .any(|f| f.starts_with("CONTRÔLE NÉGATIF RATÉ"))
        );
        let mut sans = Passage::default();
        juger_le_negatif(&mut sans, Oui::Non, Oui::Non);
        assert!(sans.pas_execute.is_some());
    }

    #[test]
    fn le_negatif_du_second_veut_l_oubli_apres_le_n2_et_pas_avant() {
        let n = |k: u32| {
            format!(
                "Punchline, après « On rejoue » n° {k} : la page n'est pas en plein écran (document.fullscreenElement est nul)"
            )
        };
        let mut vu = Passage {
            fautes: vec![n(2)],
            ..Passage::default()
        };
        juger_le_negatif(&mut vu, Oui::Oui, Oui::Oui);
        assert!(vu.fautes.is_empty(), "{:?}", vu.fautes);
        // Perdu dès le n° 1 : le fautif du second n'y était pour rien, c'est une faute.
        let mut tot = Passage {
            fautes: vec![n(1), n(2)],
            ..Passage::default()
        };
        juger_le_negatif(&mut tot, Oui::Oui, Oui::Oui);
        assert_eq!(tot.fautes, vec![n(1)]);
        // Le fautif du premier vu seul ne suffit pas au fautif du second.
        let mut premier = Passage {
            fautes: vec![n(1)],
            ..Passage::default()
        };
        juger_le_negatif(&mut premier, Oui::Oui, Oui::Oui);
        assert!(
            premier
                .fautes
                .iter()
                .any(|f| f.starts_with("CONTRÔLE NÉGATIF RATÉ"))
        );
    }

    #[test]
    fn le_numero_de_partie_se_lit_du_rapport_d_etat() {
        assert_eq!(
            partie(&ligne(1, "{\"quoi\":\"etat\",\"partie\":2}")),
            Some(2)
        );
        assert_eq!(
            partie(&ligne(1, "{\"quoi\":\"etat\",\"partie\":null}")),
            None
        );
        assert_eq!(partie(&ligne(1, "{\"quoi\":\"jeu\",\"partie\":2}")), None);
    }
}
