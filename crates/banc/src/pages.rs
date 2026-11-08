//! Le scénario `soiree-banc pages` (PHASE-3, § 7, ligne du lot 12 ; § 8.2, ligne `pages` ;
//! lot 12, fin) : **chaque page de téléphone de pire cas, à trois tailles d'écran, dans un
//! vrai navigateur**. Un élément qui déborde de sa boîte (`scrollWidth` contre
//! `clientWidth`), ou de la largeur de la page, échoue en nommant sa clé.
//!
//! 1. **La préparation, en natif.** Chaque module est chargé dans wasmtime (`PHASE_3`) ;
//!    chacun de ses pires cas est construit sur une instance nue, comme E16 et `dessin` le
//!    font (`PireCas`, depuis les limites déclarées : CONTRAT, « Le résultat annoncé par
//!    l'auteur est un indice ») ; pour chaque appareil que le pire cas connaît (ceux de
//!    `Construction::appareils`), sa vue de téléphone (`Regarder`), les mêmes octets que
//!    l'autorité enverrait. Les vues identiques ne se posent qu'une fois. Les ressources de
//!    la page sont ce que `Ressources` rend pour chaque asset déclaré, comme la livraison
//!    les envoie (`salle::livraison`). Un jeu sans téléphone est « sans objet ».
//! 2. **Le passage**, dans un navigateur neuf : le relais (en mode banc, sans jeu) sert la
//!    page du téléphone, ouverte sans salle ni fragment ; le banc lui passe le module, les
//!    ressources, puis chaque vue (`window.__banc.morceau`, `window.__banc.pire_cas`, mode
//!    banc seulement). La page fait ce qu'elle fait d'un module reçu par la connexion :
//!    réécrit et compilé (`moteur_navigateur::charger`), `Page` appelée dans le module, le
//!    DOM réconcilié par clé, les polices par `FontFace`. Ni le transport, ni les locaux
//!    d'un joueur (un champ rempli, un choix fait) ne sont joués ici : `soiree --pages` les
//!    joue sur les pages vues en soirée.
//! 3. **Le jugement**, à chacune des trois tailles (`telephones::TAILLES`) : chaque vue
//!    posée, `window.__banc.debordements()` lu, chaque élément qui déborde est une faute
//!    qui nomme le jeu, le pire cas, l'appareil, la taille, la sorte et la clé. Une page qui
//!    fait faute (`Page` qui panique, une page invalide) en est une aussi.
//! 4. **Le contrôle négatif** (`--negatif <module>`) : un fautif de module, dont la page de
//!    pire cas écrit un texte que même une coupure à chaque lettre ne fait pas tenir
//!    (`fautifs::jeux::PageQuiDeborde`, le prénom du record à 512 px). Il est joué dans
//!    le même passage, par le même chemin ; il doit déborder, en nommant sa clé, sinon
//!    « CONTRÔLE NÉGATIF RATÉ » : le passage échoue.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, p le passage, r une issue, e une erreur"
)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use jeu_abi::{Appel, ApplicableFil, RegardFil, Retour, SorteAppel, decoder, encoder};
use moteur_wasmtime::{Moteur, ReglagesMoteur};
use partie::{Fabrique, Instance};
use serde_json::Value as Json;

use crate::horloge::{maintenant, reste};
use crate::navigateur::{Lance, Navigateur};
use crate::soiree_native::port_libre;
use crate::telephones::TAILLES;

/// Le budget d'un appel du banc : le plafond d'un appel (2²⁸), le banc ne juge pas le
/// carburant (comme `dessin`).
const BUDGET: u64 = 1 << 28;

/// Les octets passés à la page par appel de `__banc.morceau` (choisi : 128 Kio, 256 Kio
/// de texte hexadécimal, loin des limites d'un message de CDP ou de `BiDi`).
const MORCEAU: usize = 128 * 1024;

/// Le plus long qu'on attend les polices de la page (`document.fonts.status`) et les images
/// d'une vue (choisi).
const ATTENTE_DES_RESSOURCES: Duration = Duration::from_secs(5);

/// La salle de l'adresse de la page : sans fragment, la page n'en ouvre aucune (choisi,
/// l'adresse de `dessin::ADRESSE_QR`).
const SALLE: &str = "3FA9C2D1";

/// Le multiplicateur des numéros d'appareil des pires cas (`jeu::Construction::appareils`) :
/// le `n`-ième appareil d'un pire cas est `n × NOMBRE_D_OR`.
const NOMBRE_D_OR: u64 = 0x9e37_79b9_7f4a_7c15;

/// Ce que le scénario joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Les modules livrés des jeux.
    pub modules: Vec<PathBuf>,
    /// Le contrôle négatif : un fautif de module dont une page de pire cas déborde.
    pub negatif: Option<PathBuf>,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Construire le paquet `web/` (sinon, le dernier construit).
    pub construire: bool,
    /// Le dossier de travail (les profils des navigateurs).
    pub travail: PathBuf,
}

/// Une vue de téléphone d'un pire cas, et qui la montre.
#[derive(Debug, Clone)]
pub struct VueDePireCas {
    /// Les octets de la vue (`Regarder`).
    pub octets: Vec<u8>,
    /// Les pires cas et les appareils (leur rang, de 1 au plus d'appareils) qui la
    /// montrent : le premier la nomme.
    pub qui: Vec<(u32, u16)>,
}

/// Un jeu préparé : son module, ses ressources de page, ses vues de pire cas.
#[derive(Debug, Clone)]
pub struct JeuDePages {
    /// Son nom (celui de sa déclaration).
    pub nom: String,
    /// Le BLAKE3 du module, en hexadécimal.
    pub empreinte: String,
    /// Le module livré.
    pub module: Vec<u8>,
    /// Les ressources de la page, par clé d'asset.
    pub ressources: Vec<(u64, Vec<u8>)>,
    /// Les vues distinctes.
    pub vues: Vec<VueDePireCas>,
    /// Les pires cas construits.
    pub cas: u32,
    /// Le jeu a-t-il des téléphones ?
    pub telephones: bool,
}

fn encode_appel(a: &Appel<'_>) -> Result<Vec<u8>, String> {
    let mut o = Vec::new();
    encoder(a, &mut o).map_err(|e| e.to_string())?;
    Ok(o)
}

fn lire(i: &mut impl Instance, a: &Appel<'_>) -> Result<Vec<u8>, String> {
    i.appeler(SorteAppel::Lecture, &encode_appel(a)?, BUDGET)
        .map_err(|e| e.to_string())
}

/// Prépare un module : chaque pire cas, la vue de chaque appareil, les ressources.
pub fn preparer(module: &Path) -> Result<JeuDePages, String> {
    let octets = std::fs::read(module).map_err(|e| format!("{} : {e}", module.display()))?;
    let empreinte = blake3::hash(&octets).to_hex().to_string();
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| e.to_string())?;
    let charge = moteur
        .charger(&octets)
        .map_err(|e| format!("{} : {e:?}", module.display()))?;
    let declaration = charge.declaration().clone();
    let nom = declaration.nom.as_str().to_owned();
    let telephones = matches!(declaration.telephones, ApplicableFil::Oui(_));
    let mut jeu = JeuDePages {
        nom: nom.clone(),
        empreinte,
        module: Vec::new(),
        ressources: Vec::new(),
        vues: Vec::new(),
        cas: 0,
        telephones,
    };
    if !telephones {
        return Ok(jeu);
    }
    let appareils = declaration.places.appareils_max();
    let mut vues: BTreeMap<Vec<u8>, Vec<(u32, u16)>> = BTreeMap::new();
    let mut ordre: Vec<Vec<u8>> = Vec::new();
    for indice in 0..=u32::from(u16::MAX) {
        let mut inst = Fabrique::neuve(&mut charge.fabrique()).map_err(|p| p.to_string())?;
        for a in [Appel::Bonjour, Appel::Decrire] {
            let _: Vec<u8> = lire(&mut inst, &a)?;
        }
        let r = inst
            .appeler(
                SorteAppel::Ecriture,
                &encode_appel(&Appel::PireCas { indice })?,
                BUDGET,
            )
            .map_err(|e| format!("{nom}, pire cas {indice} : {e}"))?;
        if !matches!(decoder::<Retour<'_>>(&r), Ok(Retour::PireCas { .. })) {
            break;
        }
        jeu.cas = jeu.cas.saturating_add(1);
        if indice == 0 {
            for a in &declaration.assets {
                let r = lire(
                    &mut inst,
                    &Appel::Ressources {
                        cles: vec![a.cle.fil()],
                    },
                )?;
                if let Ok(Retour::Ressources(v)) = decoder::<Retour<'_>>(&r) {
                    jeu.ressources
                        .extend(v.iter().map(|(c, o)| (*c, o.to_vec())));
                }
            }
        }
        for n in 1..=appareils {
            let fil = u64::from(n).wrapping_mul(NOMBRE_D_OR);
            let r = lire(
                &mut inst,
                &Appel::Regarder {
                    regards: vec![RegardFil::Appareil(fil)],
                },
            )?;
            let Ok(Retour::Vues(v)) = decoder::<Retour<'_>>(&r) else {
                return Err(format!(
                    "{nom}, pire cas {indice}, appareil {n} : pas de vue"
                ));
            };
            let vue = v
                .first()
                .ok_or_else(|| format!("{nom}, pire cas {indice}, appareil {n} : vue vide"))?
                .octets
                .to_vec();
            let qui = vues.entry(vue.clone()).or_default();
            if qui.is_empty() {
                ordre.push(vue);
            }
            qui.push((indice, n));
        }
    }
    jeu.vues = ordre
        .into_iter()
        .map(|o| {
            let qui = vues.remove(&o).unwrap_or_default();
            VueDePireCas { octets: o, qui }
        })
        .collect();
    jeu.module = octets;
    Ok(jeu)
}

/// Ce qu'un passage a fait et constaté.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur, et sa version.
    pub navigateur: String,
    /// Ce que le banc a constaté sans en juger.
    pub notes: Vec<String>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
    /// Le passage n'a pas pu jouer.
    pub pas_execute: Option<String>,
}

fn hexa(o: &[u8]) -> String {
    use std::fmt::Write as _;
    o.iter().fold(
        String::with_capacity(o.len().saturating_mul(2)),
        |mut s, b| {
            let _: core::fmt::Result = write!(s, "{b:02x}");
            s
        },
    )
}

/// Évalue `js` et rend sa valeur, une chaîne JSON relue s'il le faut.
fn evaluer_json(l: &mut Lance, js: &str) -> Result<Json, String> {
    let v = l.evaluer(0, js)?;
    if let Json::String(t) = &v {
        return Ok(serde_json::from_str(t).unwrap_or(v));
    }
    Ok(v)
}

/// Passe des octets à la page, par morceaux, puis lui dit ce qu'ils sont.
fn passer(l: &mut Lance, octets: &[u8], quoi: &str, cle: &str) -> Result<Json, String> {
    for m in octets.chunks(MORCEAU) {
        let ok = l.evaluer(0, &format!("window.__banc.morceau(\"{}\")", hexa(m)))?;
        if ok != Json::Bool(true) {
            return Err(format!("{quoi} : la page refuse un morceau ({ok})"));
        }
    }
    let r = evaluer_json(
        l,
        &format!(
            "window.__banc.pire_cas({}, {})",
            Json::from(quoi),
            Json::from(cle)
        ),
    )?;
    match r.get("erreur") {
        Some(e) => Err(format!("{quoi} : {}", e.as_str().unwrap_or("?"))),
        None if r.get("ok") == Some(&Json::Bool(true)) => Ok(r),
        None => Err(format!("{quoi} : réponse illisible {r}")),
    }
}

/// Attend que `js` rende `true`, au plus `delai`.
fn attendre(l: &mut Lance, js: &str, delai: Duration) -> Result<bool, String> {
    let fin = maintenant().checked_add(delai).ok_or("délai")?;
    loop {
        if l.evaluer(0, js)? == Json::Bool(true) {
            return Ok(true);
        }
        if reste(fin).is_zero() {
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Pose chaque vue d'un jeu à chaque taille ; rend les débordements, nommés.
fn juger_un_jeu(
    l: &mut Lance,
    jeu: &JeuDePages,
    notes: &mut Vec<String>,
) -> Result<Vec<String>, String> {
    let mut debords = Vec::new();
    passer(l, &jeu.module, "module", "")?;
    for (c, o) in &jeu.ressources {
        passer(l, o, "ressource", &format!("{c:x}"))?;
    }
    if !attendre(
        l,
        "document.fonts.status === 'loaded'",
        ATTENTE_DES_RESSOURCES,
    )? {
        notes.push(format!(
            "{} : les polices de la page ne sont pas toutes chargées en {ATTENTE_DES_RESSOURCES:?}",
            jeu.nom
        ));
    }
    let mut posees = 0u32;
    // Les nœuds des pages posées, au moins et au plus, et les pages vides : une page qui
    // n'a rien posé ne juge rien.
    let (mut moins, mut plus, mut vides) = (u64::MAX, 0u64, 0u32);
    for (largeur, hauteur) in TAILLES {
        l.telephone(0, largeur, hauteur)?;
        std::thread::sleep(Duration::from_millis(300));
        for v in &jeu.vues {
            let (indice, n) = v.qui.first().copied().unwrap_or((0, 0));
            let nommer = format!(
                "{} : pire cas {indice}, appareil {n} (et {} autres), à {largeur} × {hauteur}",
                jeu.nom,
                v.qui.len().saturating_sub(1)
            );
            match passer(l, &v.octets, "vue", "") {
                Ok(r) => {
                    let k = r.get("noeuds").and_then(Json::as_u64).unwrap_or(0);
                    moins = moins.min(k);
                    plus = plus.max(k);
                    if k == 0 {
                        vides = vides.saturating_add(1);
                    }
                }
                Err(e) => {
                    debords.push(format!("{nommer} : {e}"));
                    continue;
                }
            }
            posees = posees.saturating_add(1);
            let _: bool = attendre(
                l,
                "Array.from(document.images).every(i => i.complete)",
                ATTENTE_DES_RESSOURCES,
            )?;
            let d = evaluer_json(l, "window.__banc.debordements()")?;
            for e in d
                .get("elements")
                .and_then(Json::as_array)
                .cloned()
                .unwrap_or_default()
            {
                debords.push(format!(
                    "{nommer} : {} {} déborde (scroll {}, client {}, droite {}, page {})",
                    e.get("sorte").and_then(Json::as_str).unwrap_or("?"),
                    e.get("cle").and_then(Json::as_str).unwrap_or("?"),
                    e.get("scroll").unwrap_or(&Json::Null),
                    e.get("client").unwrap_or(&Json::Null),
                    e.get("droite").unwrap_or(&Json::Null),
                    e.get("largeur").unwrap_or(&Json::Null),
                ));
            }
            if let Some([sw, cw]) = d.get("page").and_then(Json::as_array).map(Vec::as_slice)
                && sw.as_i64().unwrap_or(0) > cw.as_i64().unwrap_or(0).saturating_add(1)
            {
                debords.push(format!("{nommer} : la page déborde ({sw} contre {cw})"));
            }
        }
    }
    notes.push(format!(
        "{} : {} pire(s) cas, {} vue(s) de téléphone distincte(s), {posees} page(s) posée(s) et jugée(s) aux trois tailles, de {} à {plus} nœuds, {vides} vide(s) ; {} ressource(s) de page",
        jeu.nom,
        jeu.cas,
        jeu.vues.len(),
        if moins == u64::MAX { 0 } else { moins },
        jeu.ressources.len()
    ));
    if posees > 0 && plus == 0 {
        debords.push(format!(
            "{} : aucune page de pire cas posée n'a de nœud : rien n'a été jugé",
            jeu.nom
        ));
    }
    Ok(debords)
}

/// Un passage dans le navigateur `n` : les jeux, puis le contrôle négatif.
fn passage(
    o: &Options,
    n: Navigateur,
    paquet: &Path,
    jeux: &[JeuDePages],
    negatif: Option<&JeuDePages>,
) -> Passage {
    let mut p = Passage {
        navigateur: n.nom().to_owned(),
        ..Passage::default()
    };
    let r = (|| -> Result<(), String> {
        let dossier = o.travail.join(n.nom());
        let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
        let vide = dossier.join("jeux");
        std::fs::create_dir_all(&vide).map_err(|e| format!("{e}"))?;
        let port = port_libre()?;
        let relais = relais::coque::lancer(&relais::coque::Reglages {
            port,
            adresse: None,
            garde: dossier.join("garde"),
            banc: relais::http::Banc::Oui,
            enregistrer: None,
            interfaces: Some(Vec::new()),
            page: None,
            paquet: Some(relais::paquet::Paquet::lire(paquet)?),
            jeux: relais::paquet::Jeux::lire(&vide)?,
            annonce_du_banc: None,
        })
        .map_err(|e| format!("le relais : {e}"))?;
        let mut nav = match Lance::lancer(n, None, &o.travail.join("profils")) {
            Ok(nav) => nav,
            Err(e) => {
                relais.arreter();
                return Err(format!("pas exécuté : {e}"));
            }
        };
        nav.version().clone_into(&mut p.navigateur);
        let r = (|| -> Result<(), String> {
            nav.ouvrir(&format!("http://127.0.0.1:{port}/s/{SALLE}"))?;
            if !attendre(
                &mut nav,
                "typeof window.__banc === 'object' && typeof window.__banc.pire_cas === 'function'",
                Duration::from_secs(30),
            )? {
                return Err(String::from(
                    "la page du téléphone n'a pas ses crochets du banc en 30 s",
                ));
            }
            for jeu in jeux {
                if !jeu.telephones {
                    p.notes
                        .push(format!("{} : sans téléphone, sans objet", jeu.nom));
                    continue;
                }
                let debords = juger_un_jeu(&mut nav, jeu, &mut p.notes)?;
                p.fautes.extend(debords);
            }
            if let Some(fautif) = negatif {
                let vus = juger_un_jeu(&mut nav, fautif, &mut p.notes)?;
                if vus.is_empty() {
                    p.fautes.push(format!(
                        "CONTRÔLE NÉGATIF RATÉ : aucune page de pire cas de {} ne déborde",
                        fautif.nom
                    ));
                } else {
                    p.notes.push(format!(
                        "contrôle négatif ({}, {}…) : {} débordement(s) vu(s), comme attendu :",
                        fautif.nom,
                        fautif.empreinte.get(..16).unwrap_or_default(),
                        vus.len()
                    ));
                    p.notes
                        .extend(vus.iter().take(12).map(|x| format!("  {x}")));
                }
            } else {
                p.notes
                    .push(String::from("contrôle négatif : non joué (sans --negatif)"));
            }
            Ok(())
        })();
        nav.fermer();
        relais.arreter();
        r
    })();
    match r {
        Ok(()) => {}
        Err(e) if e.starts_with("pas exécuté") => p.pas_execute = Some(e),
        Err(e) => p.fautes.push(e),
    }
    p
}

/// Prépare les jeux, puis joue un passage dans chaque navigateur.
pub fn jouer(o: &Options, soiree: &Path) -> Result<Vec<Passage>, String> {
    let mut jeux = Vec::new();
    for m in &o.modules {
        jeux.push(preparer(m)?);
    }
    let negatif = o.negatif.as_deref().map(preparer).transpose()?;
    let paquet = if o.construire {
        crate::soiree::construire_le_paquet(soiree)?
    } else {
        crate::soiree::dernier_paquet(soiree)?
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    let mut passages = Vec::new();
    for n in &o.navigateurs {
        if *n == Navigateur::Safari {
            passages.push(Passage {
                navigateur: n.nom().to_owned(),
                pas_execute: Some(String::from(
                    "pas exécuté : le banc n'évalue rien dans Safari (`safaridriver`)",
                )),
                ..Passage::default()
            });
            continue;
        }
        passages.push(passage(o, *n, &paquet, &jeux, negatif.as_ref()));
    }
    Ok(passages)
}

impl core::fmt::Display for Passage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "== {}", self.navigateur)?;
        if let Some(e) = &self.pas_execute {
            return writeln!(f, "  {e}");
        }
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
