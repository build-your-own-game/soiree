//! Le scénario `changer` (PHASE-3, § 7, ligne du lot 12 ; § 8.2 ; lot 12, fin) : **changer
//! de jeu sans rien relancer**. Le relais lancé une fois, avec les jeux du catalogue ;
//! l'écran ouvert une fois, sur le premier ; des téléphones natifs qui passent par la page
//! (et, avec `--pages`, des pages dans de vrais navigateurs). Puis, pour chaque autre jeu,
//! depuis l'écran de démarrage, par la vraie chaîne d'entrées : F9 le remontre, un clic
//! sur la ligne du jeu le choisit, un clic sur « Jouer » le joue.
//!
//! Ce que le banc juge, à chaque changement : le Worker ouvre une partie neuve du jeu
//! choisi (ligne `change`, dont le pas suit le dernier de l'ancienne) ; l'écran change de
//! jeu (ligne `jeu`, le module choisi) puis continue de jouer (des lignes `etat` du même
//! onglet, le pas qui avance) ; chaque téléphone natif reçoit le nouveau module et refait
//! sa page dessus, et, si le jeu a des téléphones, rejoue par elle ; chaque page d'un
//! navigateur dit le nouveau module. Et ce qui ne doit pas bouger : la page de l'écran
//! (aucune ligne `page` de plus : elle ne s'est pas rechargée), son Worker (aucune ligne
//! `en-jeu` : il ne s'est pas relancé ; ses tours continuent), le relais (le même, dans ce
//! processus ; aucun natif ne s'y est relié de nouveau). À la fin, la partie figée doit
//! être celle que wasmtime reprend de la garde sur disque avec le module du dernier jeu.
//!
//! Le contrôle négatif : `--fautif changer-sans-livrer` (le Worker change de jeu sans
//! livrer le module aux téléphones) doit échouer sur les téléphones, qui gardent l'ancien.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, s la session, p le passage, l une ligne, h l'empreinte en hexadécimal"
)]

use std::path::Path;
use std::time::Duration;

use moteur_wasmtime::ModuleCharge;
use salle::simulation::reprendre_de_la_garde;
use serde_json::Value as Json;

use crate::horloge::{maintenant, reste};
use crate::navigateur::Navigateur;
use crate::soiree::{
    Ligne, Options, Passage, Session, charger_le_module, controler_les_lignes, debuter,
    ecrire_les_lignes, exporter_la_cle, figer, hexa, juger_les_natifs, ouvrir, ouvrir_les_pages,
    telephones_et_redite,
};

/// Le temps laissé à chaque jeu avant le suivant (choisi : assez pour que les téléphones
/// rejoignent et jouent par la page).
const PAR_JEU: Duration = Duration::from_secs(20);

/// Le plus long qu'un téléphone met à recevoir le nouveau module (choisi : 69 téléphones
/// natifs reçoivent Sidequest, 738 744 octets, en moins de 10 s au lot 12).
const LIVRAISON_MAX: Duration = Duration::from_secs(30);

/// Le nom d'un jeu, lu dans la déclaration de son module.
fn nom_du_jeu(charge: &ModuleCharge) -> String {
    charge.declaration().nom.as_str().to_owned()
}

/// Ce que les téléphones natifs disent de leur module et de leur page.
#[derive(Debug, Clone, Default)]
struct EtatDesNatifs {
    modules: Vec<Option<[u8; 32]>>,
    gestes: Vec<u64>,
    changements: Vec<u64>,
    liens: Vec<Option<u32>>,
}

fn etat_des_natifs(s: &Session) -> EtatDesNatifs {
    let t = s.natifs.troupe();
    EtatDesNatifs {
        modules: t
            .telephones
            .iter()
            .map(|x| x.appareil.module().map(|(e, _)| e))
            .collect(),
        gestes: t
            .telephones
            .iter()
            .map(|x| x.joueur.comptes.gestes_par_la_page)
            .collect(),
        changements: t
            .telephones
            .iter()
            .map(|x| x.joueur.comptes.changements)
            .collect(),
        liens: t
            .telephones
            .iter()
            .map(|x| x.appareil.lien_relais())
            .collect(),
    }
}

/// Pour chaque natif qui vient de refaire sa page sur `cible` (son module, et une page
/// refaite de plus qu'avant le changement), ses gestes à ce moment : ce qu'il enverra
/// ensuite part de la page neuve.
fn noter_les_pages_neuves(
    avant: &EtatDesNatifs,
    e: &EtatDesNatifs,
    cible: &[u8; 32],
    base: &mut [Option<u64>],
) {
    for (k, b) in base.iter_mut().enumerate() {
        let neuve = e.modules.get(k).and_then(|m| m.as_ref()) == Some(cible)
            && e.changements.get(k) > avant.changements.get(k);
        if b.is_none() && neuve {
            *b = e.gestes.get(k).copied();
        }
    }
}

/// Le centre que l'écran du commun propose pour `jeu` (sa ligne de la liste) ou pour
/// « Jouer » (`None`), en pixels de la fenêtre.
fn centre(s: &mut Session, jeu: Option<&[u8; 32]>) -> Result<Option<(f64, f64)>, String> {
    let arg = jeu.map_or_else(String::new, |e| format!("\"{}\"", hexa(e)));
    // Une chaîne : `BiDi` ne rend une valeur que si elle est primitive.
    let c = s.evaluer(0, &format!("JSON.stringify(window.__banc.centre({arg}))"))?;
    let c: Json = c
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    Ok(c.as_array().map(Vec::as_slice).and_then(|a| match a {
        [x, y] => Some((x.as_f64()?, y.as_f64()?)),
        _ => None,
    }))
}

/// Clique, par la vraie chaîne d'entrées, sur ce que l'écran du commun propose.
fn cliquer(s: &mut Session, jeu: Option<&[u8; 32]>, quoi: &str) -> Result<(), String> {
    let (x, y) = centre(s, jeu)?.ok_or_else(|| format!("{quoi} : rien à cliquer"))?;
    s.navigateur.montrer(0)?;
    s.navigateur.cliquer(0, x, y)
}

/// Le dernier pas que l'écran dit, et les tours de son Worker.
fn pas_et_tours(s: &Session, onglet: &str) -> (u64, u64) {
    s.lignes
        .iter()
        .rev()
        .find(|l| l.onglet == onglet && l.quoi() == "etat")
        .map_or((0, 0), |l| {
            (
                l.json.get("pas").and_then(Json::as_u64).unwrap_or(0),
                l.json
                    .pointer("/m13/commun/tours")
                    .and_then(Json::as_u64)
                    .unwrap_or(0),
            )
        })
}

/// Joue un changement vers `cible` depuis l'écran de démarrage, et juge ce qui doit suivre.
#[expect(
    clippy::too_many_lines,
    reason = "un changement se lit d'un trait, dans l'ordre où il se joue"
)]
fn changer_vers(
    s: &mut Session,
    p: &mut Passage,
    onglet: &str,
    (cible, nom, telephones_du_jeu): (&[u8; 32], &str, bool),
) -> Result<(), String> {
    let avant = etat_des_natifs(s);
    let (pas_avant, tours_avant) = pas_et_tours(s, onglet);
    let pages_avant = s.lignes.iter().filter(|l| l.quoi() == "page").count();
    // F9 : l'écran de démarrage par-dessus la partie.
    let depuis = s.lignes.len();
    s.navigateur.montrer(0)?;
    s.navigateur.touche(0, "F9")?;
    s.attendre(
        depuis,
        Duration::from_secs(10),
        "l'écran de démarrage (F9)",
        |l| {
            l.quoi() == "ecran-du-commun"
                && l.json.get("montre").and_then(Json::as_str) == Some("demarrage")
        },
    )?;
    s.tourner(Duration::from_millis(300))?;
    // La ligne du jeu, puis « Jouer ».
    let depuis = s.lignes.len();
    cliquer(s, Some(cible), &format!("la ligne de {nom}"))?;
    let h = hexa(cible);
    let hc = h.clone();
    s.attendre(
        depuis,
        Duration::from_secs(10),
        &format!("{nom} choisi"),
        move |l| {
            l.quoi() == "choisi" && l.json.get("module").and_then(Json::as_str) == Some(hc.as_str())
        },
    )?;
    let depuis = s.lignes.len();
    cliquer(s, None, "« Jouer »")?;
    let hc = h.clone();
    let o = onglet.to_owned();
    let change = s.attendre(
        depuis,
        Duration::from_secs(30),
        &format!("la partie neuve de {nom}"),
        move |l| l.onglet == o && l.quoi() == "change",
    )?;
    let o = onglet.to_owned();
    s.attendre(
        depuis,
        Duration::from_secs(10),
        &format!("l'écran passe à {nom}"),
        move |l| {
            l.onglet == o
                && l.quoi() == "jeu"
                && l.json.get("module").and_then(Json::as_str) == Some(hc.as_str())
        },
    )?;
    if let Some(r) = s
        .lignes
        .iter()
        .skip(depuis)
        .find(|l| l.quoi() == "change-refuse")
    {
        p.fautes
            .push(format!("{nom} : le changement est refusé : {}", r.json));
    }
    let pas_change = change.json.get("pas").and_then(Json::as_u64).unwrap_or(0);
    if pas_change <= pas_avant {
        p.fautes.push(format!(
            "{nom} : la partie neuve commence au pas {pas_change}, l'ancienne en était au pas {pas_avant} : le pas a reculé"
        ));
    }
    // Les téléphones natifs reçoivent le module, et refont leur page. *Correction de la fin
    // du lot 12* : les gestes de chaque natif se comptent depuis qu'il a refait sa page sur
    // le nouveau module, pas depuis le changement ; au lot 12, fin, ceux que l'ancienne page
    // envoyait encore comptaient pour le nouveau jeu (le contrôle négatif disait « 4 sur 6
    // ont joué par sa page », les six restés sur Punchline).
    let mut depuis_la_page_neuve: Vec<Option<u64>> = vec![None; avant.gestes.len()];
    let fin = maintenant().checked_add(LIVRAISON_MAX).ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(250))?;
        let e = etat_des_natifs(s);
        noter_les_pages_neuves(&avant, &e, cible, &mut depuis_la_page_neuve);
        if e.modules.iter().all(|m| m.as_ref() == Some(cible)) {
            break;
        }
        if reste(fin).is_zero() {
            let manquent: Vec<String> = e
                .modules
                .iter()
                .enumerate()
                .filter(|(_, m)| m.as_ref() != Some(cible))
                .map(|(k, m)| {
                    format!(
                        "{k} ({})",
                        m.map_or_else(|| String::from("aucun"), |x| hexa(&x))
                    )
                })
                .collect();
            p.fautes.push(format!(
                "{nom} : des téléphones natifs n'ont pas reçu son module en {LIVRAISON_MAX:?} : {}",
                manquent.join(", ")
            ));
            break;
        }
    }
    // Le jeu joue un moment ; les téléphones rejouent par la page.
    s.tourner(PAR_JEU)?;
    let apres = etat_des_natifs(s);
    for (k, (a, b)) in avant.changements.iter().zip(&apres.changements).enumerate() {
        if b <= a && apres.modules.get(k).and_then(|m| m.as_ref()) == Some(cible) {
            p.fautes.push(format!(
                "{nom} : le téléphone natif {k} a reçu le module, mais n'a pas refait sa page"
            ));
        }
    }
    noter_les_pages_neuves(&avant, &apres, cible, &mut depuis_la_page_neuve);
    if telephones_du_jeu {
        let rejouent = depuis_la_page_neuve
            .iter()
            .zip(&apres.gestes)
            .filter(|(base, b)| base.is_some_and(|a| **b > a))
            .count();
        // Chaque natif rejoue par la page neuve (§ 8.2, ligne `changer`).
        if rejouent < apres.gestes.len() {
            p.fautes.push(format!(
                "{nom} : {rejouent} téléphones natifs sur {} seulement ont joué par la page du nouveau jeu, depuis qu'ils l'ont refaite",
                apres.gestes.len()
            ));
        }
        p.notes.push(format!(
            "{nom} : {rejouent} téléphones natifs sur {} ont joué par sa page, depuis qu'ils l'ont refaite",
            apres.gestes.len()
        ));
    }
    if avant.liens != apres.liens {
        p.fautes.push(format!(
            "{nom} : des natifs se sont reliés de nouveau au relais ({:?} → {:?})",
            avant.liens, apres.liens
        ));
    }
    // Les pages des navigateurs disent le nouveau module.
    for k in 0..s.pages.len() {
        let pg = s.pages.get_mut(k).ok_or("pas de page")?;
        let e = pg.etat_de(pg.actif)?;
        let module = e.get("module").and_then(Json::as_str).unwrap_or("");
        let changements = e.get("changements").and_then(Json::as_u64).unwrap_or(0);
        if module == h && changements > 0 {
            p.notes.push(format!(
                "{nom} : page {} sur le nouveau module ({changements} page(s) refaite(s))",
                pg.nom
            ));
        } else {
            p.fautes.push(format!(
                "{nom} : la page {} n'est pas passée au nouveau module : {e}",
                pg.nom
            ));
        }
    }
    // Rien de relancé.
    let (pas_apres, tours_apres) = pas_et_tours(s, onglet);
    if pas_apres <= pas_change {
        p.fautes.push(format!(
            "{nom} : l'écran ne joue plus (pas {pas_apres} après le changement au pas {pas_change})"
        ));
    }
    if tours_apres <= tours_avant {
        p.fautes.push(format!(
            "{nom} : les tours du Worker repartent de zéro ({tours_avant} puis {tours_apres}) : il a été relancé"
        ));
    }
    let pages_apres = s.lignes.iter().filter(|l| l.quoi() == "page").count();
    if pages_apres != pages_avant {
        p.fautes
            .push(format!("{nom} : la page de l'écran s'est rechargée"));
    }
    if s.lignes.iter().skip(depuis).any(|l| l.quoi() == "en-jeu") {
        p.fautes.push(format!(
            "{nom} : le Worker est reparti (une ligne `en-jeu`) au lieu de changer de jeu"
        ));
    }
    p.notes.push(format!(
        "changé pour {nom} : la partie neuve au pas {pas_change} (l'ancienne au pas {pas_avant}), l'écran au pas {pas_apres} {} s plus tard, Worker à {tours_apres} tours ; natifs sur le module : {}/{}",
        PAR_JEU.as_secs(),
        apres.modules.iter().filter(|m| m.as_ref() == Some(cible)).count(),
        apres.modules.len()
    ));
    Ok(())
}

/// Un passage de `changer` : le premier jeu, puis chacun des autres.
pub(crate) fn changer(
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
        ..Passage::default()
    };
    let mut jeux: Vec<(Vec<u8>, ModuleCharge)> = Vec::new();
    for autre in &o.autres_jeux {
        let octets = std::fs::read(autre).map_err(|e| format!("{} : {e}", autre.display()))?;
        let c = charger_le_module(&octets)?;
        jeux.push((octets, c));
    }
    let mut dernier = (module.to_vec(), charger_le_module(module)?);
    let deroule = (|| -> Result<Option<Ligne>, String> {
        let onglet = debuter(&mut s, &mut p, telephones)?;
        if !o.pages.is_empty() {
            ouvrir_les_pages(o, &mut s, &mut p)?;
        }
        // « Jouer » sur l'écran de démarrage : le premier jeu.
        let depuis = s.lignes.len();
        cliquer(&mut s, None, "« Jouer »")?;
        s.attendre(depuis, Duration::from_secs(20), "le geste", |l| {
            l.quoi() == "geste-fait"
        })?;
        s.tourner(PAR_JEU)?;
        p.notes.push(format!(
            "{} : joué {} s",
            nom_du_jeu(charge),
            PAR_JEU.as_secs()
        ));
        for (octets, c) in jeux.drain(..) {
            let cible = *blake3::hash(&octets).as_bytes();
            let nom = nom_du_jeu(&c);
            let tel = matches!(c.declaration().telephones, jeu_abi::ApplicableFil::Oui(_));
            changer_vers(&mut s, &mut p, &onglet, (&cible, &nom, tel))?;
            dernier = (octets, c);
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
    juger_les_natifs(&s, &mut p);
    controler_les_lignes(&s, &mut p, &[]);
    // La garde finale reprend, avec le module du dernier jeu, la partie figée.
    if let Some(fige) = fige {
        let nom = fige.onglet.clone();
        match exporter_la_cle(&mut s, 0, &nom) {
            Ok(g) => {
                let du_worker = format!(
                    "{}/{}",
                    fige.json.get("forme").and_then(Json::as_str).unwrap_or(""),
                    fige.json
                        .get("empreinte")
                        .and_then(Json::as_str)
                        .unwrap_or("")
                );
                let disque = s
                    .salle
                    .ok_or_else(|| String::from("pas de salle"))
                    .and_then(|salle| s.garde_sur_disque(salle).map(|d| (salle, d)));
                match disque.and_then(|(salle, d)| {
                    reprendre_de_la_garde(dernier.1.fabrique(), &g, salle, &d, None)
                }) {
                    Ok((f, e)) if format!("{}/{}", hexa(&f), hexa(&e)) == du_worker => {
                        p.notes.push(format!(
                            "la garde finale reprend, dans wasmtime avec le module de {}, la partie figée",
                            nom_du_jeu(&dernier.1)
                        ));
                    }
                    Ok((f, e)) => p.fautes.push(format!(
                        "la garde finale reprend une autre partie : wasmtime {}/{}, le Worker figé {du_worker}",
                        hexa(&f),
                        hexa(&e)
                    )),
                    Err(e) => p.fautes.push(format!(
                        "la garde finale ne se reprend pas avec le module du dernier jeu : {e}"
                    )),
                }
            }
            Err(e) => p.fautes.push(e),
        }
    }
    p.lignes = s.lignes.len();
    ecrire_les_lignes(&dossier, &s.lignes, &s.autres);
    s.fermer();
    Ok(p)
}
