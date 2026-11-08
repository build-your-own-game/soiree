//! `soiree-banc ecran tier-list` (PHASE-3, § 8.2, lot 11) : **Tier list jouée sur la
//! télé**, par les vraies entrées du navigateur. La vraie autorité tient la partie dans le
//! Worker (comme `soiree`, lot 10a), les coups des bots arrêtés (`?bots=non`) : le banc
//! joue seul, et vérifie ce que la page montre (`window.__banc.composants()`, les boîtes
//! en pixels du document) et ce que le navigateur a reçu (les fichiers téléchargés).
//!
//! Ce qu'il joue, dans chaque navigateur, et avec WebGL2 forcé dans Chrome :
//!
//! - **l'écran du commun qui couvre le jeu** (la remontée du lot 10b) : sous l'écran de
//!   démarrage, un clic sur un bouton du jeu et des images données à son sélecteur ne lui
//!   vont pas ; le clic sur « Jouer » non plus, une fois l'écran tombé ; ni, après le
//!   rechargement, le clic sur « Cliquer pour reprendre » posé sur le bouton Reset du jeu.
//!   La page dit chaque entrée qui va au jeu (une ligne `au-jeu` : un clic, un glisser qui
//!   commence, des fichiers) ; le banc constate qu'il n'y en a aucune tant que l'écran du
//!   commun couvre le jeu, et qu'il y en a une pour chaque geste du jeu après ;
//! - **l'import** : des images données au sélecteur « Add Images » (`DOM.setFileInputFiles`
//!   dans Chrome, `input.setFiles` dans Firefox), décodées, recadrées et réduites par la
//!   page (`Image`), qui arrivent dans la réserve, dessinées ;
//! - **le glisser** (`Input.dispatchMouseEvent`, `input.performActions`) : une tuile de la
//!   réserve vers le rang S, une autre vers le rang A, une troisième à la poubelle ;
//! - **l'édition en place** du titre (un `EnPlace`), avec des caractères hors d'ASCII ;
//! - **le rechargement en cours de route** : le plateau retrouvé tel quel, images
//!   comprises (renvoyées par l'autorité, TL-14), la partie reprise de la garde ;
//! - **l'export capté** : « Export Image (PNG) », le rendu hors écran (`Rendu`, par
//!   bandes, TL-57), puis le lien du commun cliqué ; le fichier `tierlist.png` arrive dans
//!   le dossier des téléchargements, un vrai PNG de 2 400 pixels de large ;
//! - **la sauvegarde et la restauration** : « Download Backup » (`Telecharger`, dans
//!   l'activation du clic), le fichier capté ; Reset ; le fichier redonné au sélecteur
//!   « Restore Backup » (`Lire`) ; le plateau d'avant revient ;
//! - **le plateau plein d'images** : des images de bruit, qui ne se compressent pas,
//!   jusqu'à la borne du jeu ; sa sauvegarde écrite et relue ; son export ; puis les points
//!   de reprise que le Worker prend, chiffre et envoie, mesurés (M3), et le travail long
//!   (M8 : les images de la page pendant le rendu, les tours longs du Worker, et dans
//!   Chrome les images perdues de la trace).
//!
//! Ses contrôles négatifs : `--fautif activation-perdue` (la page attend que l'activation
//! du geste de `Telecharger` soit perdue) : la sauvegarde n'est pas captée, et le passage
//! échoue pour cette raison ; `--fautif geste-au-jeu` (le clic du geste n'est pas arrêté) :
//! le clic sur « Jouer » va au jeu ; `--fautif ne-couvre-pas` (la page oublie que l'écran
//! du commun couvre le jeu) : le clic et les images donnés sous l'écran de démarrage
//! vont au jeu.
//!
//! `--dpr R` joue la page à un rapport de pixels de `R` (1 par défaut) : la toile est en
//! pixels de l'appareil, les clics et `window.__banc.centre()` en pixels CSS ; le banc
//! constate le rapport, la taille de la toile, et que « Jouer » tombe dans la fenêtre.

#![expect(
    clippy::many_single_char_names,
    reason = "le banc nomme comme ses autres scénarios : o les options, n le navigateur, s la session, p le passage, c les composants"
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::Value as Json;

use crate::chrome::GesteSouris;
use crate::horloge::{maintenant, reste};
use crate::navigateur::Navigateur;
use crate::soiree::{self, Session};

/// Le titre que le banc écrit en place : des caractères hors d'ASCII. Sans caractère hors
/// du plan multilingue de base : Firefox 158 ne tape pas un tel caractère par
/// `input.performActions` (constaté au lot 11 : l'emoji 🎲 n'arrive pas à la page).
pub const NOUVEAU_TITRE: &str = "Banc du lot 11 — été ✓";

/// Les images de l'import ordinaire.
const IMAGES_ORDINAIRES: usize = 6;

/// La largeur de l'image exportée par Tier list, en pixels (1 200 logiques, à l'échelle 2 ;
/// CAHIER de Tier list, 4.8).
const LARGEUR_EXPORT: u32 = 2400;

/// La fenêtre du navigateur, en pixels CSS.
const LARGEUR_FENETRE: u32 = 1280;
/// La hauteur de la fenêtre.
const HAUTEUR_FENETRE: u32 = 1000;

/// Ce que le scénario joue.
#[derive(Debug, Clone)]
pub struct Options {
    /// Le module livré de Tier list.
    pub module: PathBuf,
    /// Les navigateurs.
    pub navigateurs: Vec<Navigateur>,
    /// Jouer aussi Chrome avec WebGL2 forcé (`?moteur=webgl2`).
    pub webgl2: bool,
    /// Le contrôle négatif : `activation-perdue`.
    pub fautif: Option<String>,
    /// Les images de bruit du plateau plein (0 : pas de plateau plein).
    pub images_plein: usize,
    /// Le rapport de pixels de la page (`--dpr`, 1 par défaut ; la remontée du lot 10b).
    pub dpr: f64,
    /// Construire le paquet `web/` (sinon, le dernier construit).
    pub construire: bool,
    /// Le dossier de travail.
    pub travail: PathBuf,
}

/// Ce qu'un passage a fait et constaté.
#[derive(Debug, Clone, Default)]
pub struct Passage {
    /// Le navigateur, sa version, et le moteur voulu.
    pub navigateur: String,
    /// Le moteur de dessin de la page.
    pub moteur: String,
    /// Ce que le banc a constaté à chaque étape.
    pub etapes: Vec<String>,
    /// Les points de reprise mesurés dans le Worker (M3) : les lignes `point`, après le
    /// plateau plein.
    pub points: Vec<Json>,
    /// Le travail long (M8) : les lignes `demande` de rendu, les tours longs, la trace.
    pub travail_long: Vec<String>,
    /// Ce qui n'a pas tenu.
    pub fautes: Vec<String>,
    /// Le passage n'a pas pu jouer.
    pub pas_execute: Option<String>,
}

// ---------------------------------------------------------------------------
// Les images de l'essai : des PNG sans compression (des blocs « stored » de zlib)
// ---------------------------------------------------------------------------

fn crc32(octets: &[&[u8]]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for o in octets {
        for &b in *o {
            c ^= u32::from(b);
            for _ in 0..8 {
                let m = 0u32.wrapping_sub(c & 1);
                c = (c >> 1) ^ (0xedb8_8320 & m);
            }
        }
    }
    !c
}

fn adler32(octets: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in octets {
        a = a.wrapping_add(u32::from(x)) % 65_521;
        b = b.wrapping_add(a) % 65_521;
    }
    (b << 16) | a
}

fn morceau(png: &mut Vec<u8>, sorte: [u8; 4], donnees: &[u8]) {
    png.extend_from_slice(&u32::try_from(donnees.len()).unwrap_or(0).to_be_bytes());
    png.extend_from_slice(&sorte);
    png.extend_from_slice(donnees);
    png.extend_from_slice(&crc32(&[&sorte, donnees]).to_be_bytes());
}

/// Un PNG RGB de 8 bits, sans filtre ni compression : le banc n'a besoin d'aucune crate
/// pour en écrire un, et une image de bruit ne se compresserait pas mieux.
pub fn png_brut(largeur: u32, hauteur: u32, pixel: impl Fn(u32, u32) -> [u8; 3]) -> Vec<u8> {
    let mut brut = Vec::new();
    for y in 0..hauteur {
        brut.push(0);
        for x in 0..largeur {
            brut.extend_from_slice(&pixel(x, y));
        }
    }
    let mut z = vec![0x78, 0x01];
    let mut blocs = brut.chunks(65_535).peekable();
    while let Some(b) = blocs.next() {
        z.push(u8::from(blocs.peek().is_none()));
        let n = u16::try_from(b.len()).unwrap_or(0);
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&brut).to_be_bytes());
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&largeur.to_be_bytes());
    ihdr.extend_from_slice(&hauteur.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    morceau(&mut png, *b"IHDR", &ihdr);
    morceau(&mut png, *b"IDAT", &z);
    morceau(&mut png, *b"IEND", &[]);
    png
}

/// La largeur et la hauteur d'un PNG, d'après son en-tête.
pub fn taille_png(o: &[u8]) -> Option<(u32, u32)> {
    if o.get(..8)? != b"\x89PNG\r\n\x1a\n" || o.get(12..16)? != b"IHDR" {
        return None;
    }
    let l = u32::from_be_bytes(o.get(16..20)?.try_into().ok()?);
    let h = u32::from_be_bytes(o.get(20..24)?.try_into().ok()?);
    Some((l, h))
}

/// Les images de l'essai.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Images {
    /// Unies et rayées : l'import ordinaire.
    Rayees,
    /// De bruit, qui ne se compressent pas : le plateau plein.
    Bruit,
}

/// Le moteur de dessin voulu pour la page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Moteur {
    /// Celui du navigateur (WebGPU s'il l'expose).
    Defaut,
    /// WebGL2 forcé (`?moteur=webgl2`).
    WebGl2,
}

/// Écrit les images de l'essai : `n` images rayées (les ordinaires), ou de bruit (le
/// plateau plein) ; rend leurs chemins.
fn ecrire_les_images(dossier: &Path, n: usize, sorte: Images) -> Result<Vec<PathBuf>, String> {
    let bruit = sorte == Images::Bruit;
    std::fs::create_dir_all(dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
    let mut chemins = Vec::new();
    for k in 0..n {
        let k32 = u32::try_from(k).unwrap_or(0);
        let png = if bruit {
            // Un générateur à graine fixe (xorshift) : les mêmes images à chaque passage.
            let graine = 0x9e37_79b9u32 ^ k32.wrapping_mul(0x85eb_ca6b);
            png_brut(400, 300, |x, y| {
                let mut v = graine ^ x.wrapping_mul(0x27d4_eb2d) ^ y.wrapping_mul(0x1656_67b1);
                v ^= v << 13;
                v ^= v >> 17;
                v ^= v << 5;
                let o = v.to_le_bytes();
                [o[0], o[1], o[2]]
            })
        } else {
            // Une couleur par image, et des rayures : un recadrage décentré se verrait.
            let teinte = [
                [230, 60, 60],
                [60, 180, 75],
                [60, 90, 230],
                [240, 200, 40],
                [160, 60, 200],
                [40, 200, 200],
            ];
            let c = teinte
                .get(k.checked_rem(teinte.len()).unwrap_or(0))
                .copied()
                .unwrap_or([128, 128, 128]);
            png_brut(
                320,
                200,
                move |x, _| {
                    if (x / 16) % 2 == 0 { c } else { [20, 20, 20] }
                },
            )
        };
        let chemin = dossier.join(format!(
            "{}-{k:03}.png",
            if bruit { "bruit" } else { "image" }
        ));
        std::fs::write(&chemin, &png).map_err(|e| format!("{} : {e}", chemin.display()))?;
        chemins.push(chemin);
    }
    Ok(chemins)
}

// ---------------------------------------------------------------------------
// Ce que la page montre
// ---------------------------------------------------------------------------

/// Ce que la page montre, et où (`window.__banc.composants()`).
fn composants(s: &mut Session) -> Result<Json, String> {
    let v = s.evaluer(0, "window.__banc ? window.__banc.composants() : '{}'")?;
    let texte = v.as_str().unwrap_or("{}");
    serde_json::from_str(texte).map_err(|e| format!("composants illisibles : {e} : {texte}"))
}

/// Attend que la page montre ce que `p` attend, au plus `delai`.
fn attendre_que(
    s: &mut Session,
    delai: Duration,
    quoi: &str,
    p: impl Fn(&Json) -> bool,
) -> Result<Json, String> {
    let fin = maintenant().checked_add(delai).ok_or("délai")?;
    let mut dernier = Json::Null;
    loop {
        s.tourner(Duration::from_millis(100))?;
        match composants(s) {
            Ok(c) => {
                if p(&c) {
                    return Ok(c);
                }
                dernier = c;
            }
            // La page se recharge : `__banc` revient avec elle.
            Err(e) if reste(fin).is_zero() => return Err(e),
            Err(_) => {}
        }
        if reste(fin).is_zero() {
            let court: String = dernier.to_string().chars().take(1500).collect();
            return Err(format!(
                "{quoi} : pas en {delai:?} ; la page montre {court}"
            ));
        }
    }
}

fn elements(c: &Json) -> &[Json] {
    c.get("elements")
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

fn zones(c: &Json) -> &[Json] {
    c.get("zones")
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

fn objets(z: &Json) -> &[Json] {
    z.get("objets")
        .and_then(Json::as_array)
        .map_or(&[], Vec::as_slice)
}

/// Le centre d'une boîte (`x`, `y`, `l`, `h`), en pixels du document.
fn centre(b: &Json) -> (f64, f64) {
    let v = |k: &str| b.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    (v("x") + v("l") / 2.0, v("y") + v("h") / 2.0)
}

fn texte_de<'a>(e: &'a Json, k: &str) -> &'a str {
    e.get(k).and_then(Json::as_str).unwrap_or("")
}

/// Un bouton de ce nom, hors d'une modale qui le couvre.
fn bouton<'a>(c: &'a Json, nom: &str) -> Option<&'a Json> {
    elements(c).iter().find(|e| {
        texte_de(e, "sorte") == "bouton"
            && texte_de(e, "nom") == nom
            && !e
                .get("sous_la_modale")
                .is_some_and(|v| v == &Json::Bool(true))
    })
}

/// La réserve : la dernière zone (les rangs, puis la réserve, dans l'ordre de l'état).
fn reserve(c: &Json) -> Option<&Json> {
    zones(c).last()
}

fn tuiles(c: &Json) -> usize {
    zones(c).iter().map(|z| objets(z).len()).sum()
}

/// Ce qui doit survivre à un rechargement et revenir d'une restauration : pour chaque
/// zone, les images de ses tuiles dans l'ordre (par la clé de leur image, que la boîte de
/// l'objet contient) ; et le titre.
fn plateau(c: &Json) -> String {
    let images: Vec<(&str, (f64, f64))> = elements(c)
        .iter()
        .filter(|e| texte_de(e, "sorte") == "image")
        .map(|e| (texte_de(e, "image"), centre(e)))
        .collect();
    let mut o = String::new();
    for z in zones(c) {
        o.push('[');
        for ob in objets(z) {
            let (x, y) = centre(ob);
            let v = |k: &str| ob.get(k).and_then(Json::as_f64).unwrap_or(0.0);
            let dedans = images.iter().find(|(_, (ix, iy))| {
                (ix - x).abs() < v("l") / 2.0 && (iy - y).abs() < v("h") / 2.0
            });
            o.push_str(dedans.map_or("texte", |(k, _)| k));
            o.push(' ');
        }
        o.push(']');
    }
    let titre = elements(c)
        .iter()
        .find(|e| {
            texte_de(e, "sorte") == "en-place"
                && e.get("y").and_then(Json::as_f64).unwrap_or(1e9) < 200.0
        })
        .map_or("", |e| texte_de(e, "valeur"));
    let _: std::fmt::Result = write!(o, " titre {titre:?}");
    o
}

/// Toutes les images montrées sont-elles décodées et posées ?
fn images_chargees(c: &Json) -> bool {
    elements(c)
        .iter()
        .filter(|e| texte_de(e, "sorte") == "image")
        .all(|e| e.get("chargee") == Some(&Json::Bool(true)))
}

// ---------------------------------------------------------------------------
// Les gestes
// ---------------------------------------------------------------------------

/// Le défilement et la hauteur de la fenêtre, en pixels du document.
fn fenetre(s: &mut Session) -> Result<(f64, f64), String> {
    // Une chaîne : `BiDi` ne rend telle quelle qu'une valeur primitive.
    let v = s.evaluer(0, "JSON.stringify([window.scrollY, window.innerHeight])")?;
    let a: Vec<Json> = serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default();
    let n = |k: usize| a.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    Ok((n(0), n(1)))
}

/// Fait défiler la page pour que les ordonnées `ys` (du document) soient dans la
/// fenêtre ; rend le défilement.
fn montrer(s: &mut Session, ys: &[f64]) -> Result<f64, String> {
    let (defile, h) = fenetre(s)?;
    let bas = ys
        .iter()
        .copied()
        .fold(f64::MIN, |a, b| if b > a { b } else { a });
    let haut = ys
        .iter()
        .copied()
        .fold(f64::MAX, |a, b| if b < a { b } else { a });
    // Loin des bords : à moins de 70 pixels, la page défile seule pendant un glisser
    // (TL-40).
    if haut >= defile + 90.0 && bas <= defile + h - 90.0 {
        return Ok(defile);
    }
    let vise = if bas - haut < h - 180.0 {
        haut - (h - (bas - haut)) / 2.0
    } else {
        haut - 90.0
    };
    let vise = if vise > 0.0 { vise } else { 0.0 };
    let _: Json = s.evaluer(0, &format!("window.scrollTo(0, {vise}); 0"))?;
    s.tourner(Duration::from_millis(150))?;
    Ok(fenetre(s)?.0)
}

/// Un clic, par la chaîne d'entrées du navigateur, au point (du document) donné.
fn cliquer(s: &mut Session, (x, y): (f64, f64)) -> Result<(), String> {
    let d = montrer(s, &[y])?;
    s.navigateur.souris(
        0,
        &[
            GesteSouris::Deplace(x, y - d),
            GesteSouris::Appui(x, y - d),
            GesteSouris::Pause(30),
            GesteSouris::Relache(x, y - d),
        ],
    )?;
    s.tourner(Duration::from_millis(100))
}

/// Un point, ou une taille, en pixels CSS.
type Point = (f64, f64);

/// Où l'écran du commun propose de cliquer (`window.__banc.centre()`, en pixels de la
/// fenêtre), et la taille de la fenêtre ; `None` s'il ne propose rien.
fn centre_du_commun(s: &mut Session) -> Result<(Option<Point>, Point), String> {
    // Une chaîne : `BiDi` ne rend une valeur que si elle est primitive.
    let v = s.evaluer(
        0,
        "JSON.stringify([window.__banc.centre(), window.innerWidth, window.innerHeight])",
    )?;
    let v: Json = v
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .unwrap_or(Json::Null);
    let n = |k: usize| v.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    let centre = v
        .get(0)
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .and_then(|a| match a {
            [x, y] => Some((x.as_f64()?, y.as_f64()?)),
            _ => None,
        });
    Ok((centre, (n(1), n(2))))
}

/// Clique sur ce que l'écran du commun propose (« Jouer » sur l'écran de démarrage,
/// « Cliquer pour reprendre » après un rechargement ; lot 10b), par la vraie chaîne
/// d'entrées, et attend que la page dise son geste : tant que l'écran du commun couvre
/// le jeu, aucun clic ni aucune touche ne lui va. `ou` : un autre point du voile que son
/// centre, en pixels de la fenêtre. Le point que la page donne doit tomber dans la
/// fenêtre : à un rapport de pixels autre que 1, des pixels de l'appareil pris pour des
/// pixels CSS l'en sortiraient (la remontée du lot 10b). Rend la ligne `geste-fait`.
fn le_geste(
    s: &mut Session,
    p: &mut Passage,
    pourquoi: &str,
    ou: Option<(f64, f64)>,
) -> Result<Json, String> {
    let (centre, (l, h)) = centre_du_commun(s)?;
    let Some((x, y)) = centre else {
        return Err(format!(
            "{pourquoi} : l'écran du commun ne propose rien à cliquer"
        ));
    };
    if !(x > 0.0 && x < l && y > 0.0 && y < h) {
        return Err(format!(
            "{pourquoi} : la page le place en ({x:.0}, {y:.0}), hors de la fenêtre de {l:.0} × {h:.0} pixels CSS"
        ));
    }
    let (x, y) = ou.unwrap_or((x, y));
    let depuis = s.lignes.len();
    s.navigateur.cliquer(0, x, y)?;
    let l = s.attendre(depuis, Duration::from_secs(20), pourquoi, |l| {
        l.quoi() == "geste-fait"
    })?;
    p.etapes.push(format!(
        "{pourquoi} : le clic en ({x:.0}, {y:.0}) est le geste ; {}",
        l.json
    ));
    Ok(l.json.clone())
}

/// Les lignes `au-jeu` venues depuis `depuis` : les entrées que la page a données au jeu
/// (un clic, un glisser qui commence, des fichiers ; la remontée du lot 10b).
fn au_jeu(s: &Session, depuis: usize) -> Vec<String> {
    lignes(s, depuis, "au-jeu").map(Json::to_string).collect()
}

/// Rien n'est allé au jeu depuis `depuis` (l'écran du commun le couvrait) ; une faute
/// sinon.
fn rien_au_jeu(s: &mut Session, p: &mut Passage, depuis: usize, quoi: &str) -> Result<(), String> {
    // La ligne `au-jeu` d'un clic qui passerait suit le geste dans la même tâche de la
    // page : ce délai la laisse arriver au banc.
    s.tourner(Duration::from_millis(400))?;
    let vu = au_jeu(s, depuis);
    if vu.is_empty() {
        p.etapes
            .push(format!("{quoi} : rien au jeu (aucune ligne au-jeu)"));
    } else {
        p.fautes.push(format!(
            "{quoi} est allé au jeu, que l'écran du commun couvrait : {}",
            vu.join(" ")
        ));
    }
    Ok(())
}

/// La page a-t-elle dit `au-jeu` pour `par` depuis `depuis` ? Une faute sinon : sans ces
/// lignes, « rien au jeu » ne prouverait rien.
fn vu_au_jeu(s: &Session, p: &mut Passage, depuis: usize, par: &str, quoi: &str) {
    let vu = lignes(s, depuis, "au-jeu").any(|j| texte_de(j, "par") == par);
    if !vu {
        p.fautes.push(format!(
            "{quoi} : la page ne dit pas au-jeu (par {par}) ; « rien au jeu » ne prouverait rien"
        ));
    }
}

/// Sous l'écran de démarrage (lot 10b), un clic sur la page du jeu, loin de « Jouer » (le
/// milieu du premier rang, ou à défaut le titre : sous l'écran de démarrage, la page ne
/// défile pas, et les boutons de Tier list sont sous le bas de la fenêtre), et des images
/// données à son sélecteur ne vont pas au jeu, et ne font pas le geste.
fn sous_l_ecran_de_demarrage(
    s: &mut Session,
    p: &mut Passage,
    c: &Json,
    images: &[PathBuf],
) -> Result<(), String> {
    let depuis = s.lignes.len();
    let (jouer, (l, h)) = centre_du_commun(s)?;
    let (jx, jy) = jouer.ok_or("l'écran de démarrage ne propose pas « Jouer »")?;
    let titre = elements(c)
        .iter()
        .find(|e| texte_de(e, "sorte") == "en-place")
        .map(centre);
    let (nom, point) = [
        ("le premier rang", zones(c).first().map(centre)),
        ("le titre", titre),
    ]
    .into_iter()
    .find_map(|(nom, point)| {
        let (x, y) = point?;
        let dedans = x > 0.0 && x < l && y > 0.0 && y < h;
        let loin = (x - jx).abs() > 150.0 || (y - jy).abs() > 80.0;
        (dedans && loin).then_some((nom, (x, y)))
    })
    .ok_or("sous l'écran de démarrage : rien du jeu dans la fenêtre, loin de « Jouer »")?;
    // Ce que la page montre du jeu, que ni le clic ni l'image ne doivent changer : le
    // plateau, et la modale (un clic sur un rang de Tier list l'ouvre).
    let montre = |c: &Json| {
        format!(
            "{} modale {}",
            plateau(c),
            c.get("modale").unwrap_or(&Json::Null)
        )
    };
    let avant = montre(c);
    cliquer(s, point)?;
    s.navigateur.fichiers(
        0,
        "input[type=file][data-genres=\"image/*\"]",
        images.get(..1).unwrap_or(images),
    )?;
    s.tourner(Duration::from_millis(800))?;
    let quoi = format!("sous l'écran de démarrage, le clic sur {nom} et une image donnée");
    if lignes(s, depuis, "geste-fait").next().is_some() {
        p.fautes.push(format!(
            "{quoi} : le clic a fait le geste, loin de « Jouer »"
        ));
    }
    rien_au_jeu(s, p, depuis, &quoi)?;
    let apres = montre(&composants(s)?);
    if apres != avant {
        p.fautes.push(format!(
            "{quoi} : ce que la page montre du jeu a changé, avant {avant} ; après {apres}"
        ));
    }
    Ok(())
}

/// Le rapport de pixels de la page, la largeur de sa fenêtre (pixels CSS) et de sa toile
/// (pixels de l'appareil), constatés contre `--dpr`.
fn rapport_de_pixels(s: &mut Session, p: &mut Passage, voulu: f64) -> Result<(), String> {
    let v = s.evaluer(
        0,
        "JSON.stringify([window.devicePixelRatio, window.innerWidth, Math.max(0, ...[...document.querySelectorAll('canvas')].map(c => c.width))])",
    )?;
    let v: Vec<Json> = serde_json::from_str(v.as_str().unwrap_or("[]")).unwrap_or_default();
    let n = |k: usize| v.get(k).and_then(Json::as_f64).unwrap_or(0.0);
    let (r, l, t) = (n(0), n(1), n(2));
    if (r - voulu).abs() > 1e-6 {
        p.fautes.push(format!(
            "le rapport de pixels de la page vaut {r}, pas {voulu}"
        ));
    } else if (t - (l * r).floor()).abs() > 1.0 {
        p.fautes.push(format!(
            "la toile fait {t} pixels de large, pour une fenêtre de {l} pixels CSS au rapport {r} : pas en pixels de l'appareil"
        ));
    } else {
        p.etapes.push(format!(
            "rapport de pixels {r} : la fenêtre fait {l} pixels CSS de large, la toile {t} pixels de l'appareil"
        ));
    }
    Ok(())
}

/// Le banc écoute les événements de pointeur que la page reçoit, pour dire, quand un
/// glisser n'a pas l'effet attendu, ce que la page a vraiment reçu.
const ECOUTER_LES_POINTEURS: &str = "(() => { if (!window.__pointeurs) { window.__pointeurs = []; for (const n of ['pointerdown','pointermove','pointerup','pointercancel']) document.addEventListener(n, e => { window.__pointeurs.push([n, e.button, e.buttons, Math.round(e.pageX), Math.round(e.pageY), Math.round(e.clientY), Math.round(window.scrollY), window.innerHeight, e.isTrusted]); if (window.__pointeurs.length > 40) window.__pointeurs.shift(); }, true); } return 0; })()";

/// Ce que la page a reçu des pointeurs, les derniers.
fn pointeurs_recus(s: &mut Session) -> String {
    s.evaluer(0, "JSON.stringify(window.__pointeurs || [])")
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// Un glisser d'un point à un autre (du document) : l'appui, un pas au-delà du seuil, le
/// chemin en dix pas, une pause, le relâché.
fn glisser(s: &mut Session, de: (f64, f64), a: (f64, f64)) -> Result<(), String> {
    let _: Json = s.evaluer(0, ECOUTER_LES_POINTEURS)?;
    let d = montrer(s, &[de.1, a.1])?;
    let mut gestes = vec![
        GesteSouris::Deplace(de.0, de.1 - d),
        GesteSouris::Appui(de.0, de.1 - d),
        GesteSouris::Pause(40),
        GesteSouris::Deplace(de.0 + 8.0, de.1 - d + 8.0),
        GesteSouris::Pause(40),
    ];
    for k in 1..=10u32 {
        let t = f64::from(k) / 10.0;
        gestes.push(GesteSouris::Deplace(
            de.0 + (a.0 - de.0) * t,
            de.1 - d + (a.1 - de.1) * t,
        ));
        gestes.push(GesteSouris::Pause(20));
    }
    gestes.push(GesteSouris::Pause(120));
    gestes.push(GesteSouris::Relache(a.0, a.1 - d));
    s.navigateur.souris(0, &gestes)?;
    s.tourner(Duration::from_millis(150))
}

// ---------------------------------------------------------------------------
// Les fichiers téléchargés
// ---------------------------------------------------------------------------

fn fichiers_du_dossier(d: &Path) -> Vec<(String, u64)> {
    let mut v: Vec<(String, u64)> = std::fs::read_dir(d)
        .map(|l| {
            l.filter_map(Result::ok)
                .filter_map(|e| Some((e.file_name().into_string().ok()?, e.metadata().ok()?.len())))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Attend un fichier téléchargé neuf dont le nom commence par `debut` et finit par `fin`
/// (le navigateur ajoute « (1) » à un second fichier du même nom), complet (ni
/// `.crdownload` ni `.part`, de taille stable) ; rend son chemin et ses octets.
fn attendre_le_fichier(
    s: &mut Session,
    avant: &[(String, u64)],
    (debut, fin): (&str, &str),
    delai: Duration,
) -> Result<(PathBuf, Vec<u8>), String> {
    let dossier = s.navigateur.telechargements()?;
    let echeance = maintenant().checked_add(delai).ok_or("délai")?;
    let mut vu: Option<(String, u64)> = None;
    loop {
        s.tourner(Duration::from_millis(200))?;
        let neuf = fichiers_du_dossier(&dossier).into_iter().find(|(n, _)| {
            n.starts_with(debut) && n.ends_with(fin) && !avant.iter().any(|(a, _)| a == n)
        });
        if let Some((n, t)) = neuf {
            if vu.as_ref().is_some_and(|(m, u)| *m == n && *u == t) && t > 0 {
                // Sorti du dossier des téléchargements : le suivant du même nom y
                // arrivera neuf (un navigateur écrase, l'autre numérote).
                let chemin = dossier.join(&n);
                let garde = dossier
                    .parent()
                    .and_then(Path::parent)
                    .ok_or("pas de dossier de travail")?
                    .join("captes");
                std::fs::create_dir_all(&garde).map_err(|e| format!("{e}"))?;
                let rang = fichiers_du_dossier(&garde).len();
                let capte = garde.join(format!("{rang:02}-{n}"));
                std::fs::rename(&chemin, &capte)
                    .map_err(|e| format!("{} : {e}", chemin.display()))?;
                let o = std::fs::read(&capte).map_err(|e| format!("{} : {e}", capte.display()))?;
                return Ok((capte, o));
            }
            vu = Some((n, t));
        }
        if reste(echeance).is_zero() {
            return Err(format!(
                "aucun fichier « {debut}…{fin} » téléchargé en {delai:?} ; le dossier : {:?}",
                fichiers_du_dossier(&dossier)
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// Le passage
// ---------------------------------------------------------------------------

/// Les lignes du banc d'une sorte, depuis l'indice `depuis`.
fn lignes<'a>(s: &'a Session, depuis: usize, quoi: &'a str) -> impl Iterator<Item = &'a Json> {
    s.lignes
        .iter()
        .skip(depuis)
        .filter(move |l| l.quoi() == quoi)
        .map(|l| &l.json)
}

/// L'import : les images données au sélecteur « Add Images » arrivent dans la réserve,
/// dessinées. Rend les composants d'après.
fn importer(
    s: &mut Session,
    p: &mut Passage,
    images: &[PathBuf],
    attendues: usize,
    delai: Duration,
) -> Result<Json, String> {
    let depuis = s.lignes.len();
    let avant = composants(s)?;
    let dans_la_reserve = reserve(&avant).map_or(0, |z| objets(z).len());
    let debut = maintenant();
    s.navigateur
        .fichiers(0, "input[type=file][data-genres=\"image/*\"]", images)?;
    let c = attendre_que(s, delai, "les images importées dans la réserve", |c| {
        reserve(c).map_or(0, |z| objets(z).len()) >= dans_la_reserve.saturating_add(attendues)
            && images_chargees(c)
    })?;
    let faites: Vec<&Json> = lignes(s, depuis, "demande")
        .filter(|j| texte_de(j, "sorte") == "image")
        .collect();
    let ratees = faites
        .iter()
        .filter(|j| texte_de(j, "issue") != "ok")
        .count();
    p.etapes.push(format!(
        "import : {} fichier(s) donnés, {} demande(s) Image ({} en échec), {} tuile(s) dans la réserve, en {:.1} s",
        images.len(),
        faites.len(),
        ratees,
        reserve(&c).map_or(0, |z| objets(z).len()),
        maintenant().saturating_duration_since(debut).as_secs_f64()
    ));
    Ok(c)
}

/// Le contrôle négatif voulu, s'il y en a un.
fn requete(o: &Options, moteur: Moteur) -> String {
    let mut r = String::from("&bots=non");
    if moteur == Moteur::WebGl2 {
        r.push_str("&moteur=webgl2");
    }
    if let Some(f) = &o.fautif {
        let _: std::fmt::Result = write!(r, "&fautif={f}");
    }
    r
}

#[expect(
    clippy::too_many_lines,
    reason = "le scénario se lit d'un bloc, dans l'ordre où il se joue"
)]
fn derouler(s: &mut Session, p: &mut Passage, o: &Options, moteur: Moteur) -> Result<(), String> {
    // Une fenêtre où tient la page de Tier list (1 120 pixels de large) et, d'un même
    // regard, un rang et la réserve loin des bords où la page défile seule (70 pixels,
    // TL-40).
    s.navigateur
        .fenetre(0, LARGEUR_FENETRE, HAUTEUR_FENETRE, o.dpr)?;
    let mut sp = soiree::Passage::default();
    let _: String = soiree::debuter(s, &mut sp, 0)?;
    p.moteur.clone_from(&sp.moteur);
    p.fautes.extend(sp.fautes);
    if moteur == Moteur::WebGl2 && p.moteur != "webgl2" {
        p.fautes.push(format!(
            "WebGL2 forcé, mais la page dessine en {}",
            p.moteur
        ));
    }
    let c = attendre_que(s, Duration::from_secs(60), "la scène de Tier list", |c| {
        bouton(c, "Export Image (PNG)").is_some() && reserve(c).is_some()
    })?;
    rapport_de_pixels(s, p, o.dpr)?;
    // L'écran de démarrage couvre la partie jusqu'au geste (lot 10b) : rien ne va au jeu
    // dessous, ni le clic sur « Jouer » (la remontée du lot 10b).
    let images = ecrire_les_images(&o.travail.join("images"), IMAGES_ORDINAIRES, Images::Rayees)?;
    sous_l_ecran_de_demarrage(s, p, &c, &images)?;
    let avant_jouer = s.lignes.len();
    let _: Json = le_geste(s, p, "« Jouer »", None)?;
    rien_au_jeu(s, p, avant_jouer, "le clic sur « Jouer »")?;
    let rangs = zones(&c).len().saturating_sub(1);
    p.etapes.push(format!(
        "la scène : {rangs} rangs et la réserve, {} éléments posés",
        elements(&c).len()
    ));

    // L'import.
    let depuis = s.lignes.len();
    let _: Json = importer(s, p, &images, IMAGES_ORDINAIRES, Duration::from_secs(60))?;
    vu_au_jeu(s, p, depuis, "fichiers", "l'import");

    // Le glisser : deux tuiles de la réserve vers les rangs S et A, une à la poubelle.
    for (k, rang) in [(0usize, 0usize), (0, 1)] {
        let c = composants(s)?;
        let de = reserve(&c)
            .and_then(|z| objets(z).get(k))
            .map(centre)
            .ok_or("la réserve est vide")?;
        let dans = zones(&c).get(rang).map_or(0, |z| objets(z).len());
        let a = zones(&c).get(rang).map(centre).ok_or("pas de rang")?;
        let depuis = s.lignes.len();
        glisser(s, de, a)?;
        let apres = attendre_que(
            s,
            Duration::from_secs(10),
            "la tuile glissée dans son rang",
            |c| zones(c).get(rang).map_or(0, |z| objets(z).len()) == dans.saturating_add(1),
        )
        .map_err(|e| {
            let recus = pointeurs_recus(s);
            format!("{e} ; glissée de {de:?} à {a:?} (document) ; la page a reçu {recus}")
        })?;
        p.etapes.push(format!(
            "glisser : une tuile de la réserve au rang {rang}, {} tuile(s) au plateau",
            tuiles(&apres)
        ));
        vu_au_jeu(s, p, depuis, "appui", "le glisser");
    }
    {
        let c = composants(s)?;
        let total = tuiles(&c);
        let de = reserve(&c)
            .and_then(|z| objets(z).first())
            .map(centre)
            .ok_or("la réserve est vide")?;
        // L'appui, au-delà du seuil : la poubelle apparaît ; puis le relâché sur elle.
        // La tuile et la poubelle dans la fenêtre, si la scène la montre déjà.
        let ys: Vec<f64> = std::iter::once(de.1)
            .chain(
                c.get("poubelle")
                    .filter(|v| !v.is_null())
                    .map(|b| centre(b).1),
            )
            .collect();
        let d = montrer(s, &ys)?;
        s.navigateur.souris(
            0,
            &[
                GesteSouris::Deplace(de.0, de.1 - d),
                GesteSouris::Appui(de.0, de.1 - d),
                GesteSouris::Pause(40),
                GesteSouris::Deplace(de.0 + 10.0, de.1 - d + 10.0),
                GesteSouris::Pause(60),
            ],
        )?;
        let pendant = attendre_que(
            s,
            Duration::from_secs(5),
            "la poubelle pendant le glisser",
            |c| {
                c.get("glisser") == Some(&Json::Bool(true))
                    && c.get("poubelle").is_some_and(|v| !v.is_null())
            },
        )?;
        let poubelle = pendant
            .get("poubelle")
            .map(centre)
            .ok_or("pas de poubelle")?;
        let (d, h) = fenetre(s)?;
        let y = poubelle.1 - d;
        if !(0.0..h).contains(&y) {
            return Err(format!(
                "la poubelle n'est pas dans la fenêtre : {poubelle:?}, défilement {d}"
            ));
        }
        s.navigateur.souris(
            0,
            &[
                GesteSouris::Deplace(poubelle.0, y),
                GesteSouris::Pause(120),
                GesteSouris::Relache(poubelle.0, y),
            ],
        )?;
        let apres = attendre_que(s, Duration::from_secs(10), "la tuile jetée", |c| {
            tuiles(c).saturating_add(1) == total && c.get("glisser") == Some(&Json::Bool(false))
        })?;
        p.etapes.push(format!(
            "glisser : une tuile à la poubelle, {} tuile(s) au plateau",
            tuiles(&apres)
        ));
    }

    // L'édition en place du titre.
    {
        let c = composants(s)?;
        let titre = elements(&c)
            .iter()
            .find(|e| texte_de(e, "sorte") == "en-place")
            .cloned()
            .ok_or("pas de titre en place")?;
        let cle = texte_de(&titre, "cle").to_owned();
        cliquer(s, centre(&titre))?;
        let focus = s.evaluer(
            0,
            "(() => { const e = document.activeElement; if (!e) return ''; const r = document.createRange(); r.selectNodeContents(e); const s = getSelection(); s.removeAllRanges(); s.addRange(r); return e.getAttribute('data-cle') || ''; })()",
        )?;
        if focus.as_str() != Some(cle.as_str()) {
            return Err(format!(
                "le clic sur le titre ne lui donne pas le focus : {focus}"
            ));
        }
        s.navigateur.texte(0, NOUVEAU_TITRE)?;
        s.tourner(Duration::from_millis(200))?;
        s.navigateur.touche(0, "Enter")?;
        let apres = attendre_que(
            s,
            Duration::from_secs(10),
            "le titre écrit en place",
            |c| {
                elements(c)
                    .iter()
                    .any(|e| texte_de(e, "cle") == cle && texte_de(e, "valeur") == NOUVEAU_TITRE)
            },
        )?;
        let _: &Json = &apres;
        p.etapes.push(format!(
            "édition en place : le titre vaut {NOUVEAU_TITRE:?}"
        ));
    }

    // Le rechargement en cours de route.
    {
        let avant = plateau(&composants(s)?);
        let depuis = s.lignes.len();
        s.navigateur.recharger(0)?;
        let l = s.attendre(depuis, Duration::from_secs(60), "la partie reprise", |l| {
            l.quoi() == "en-jeu"
        })?;
        if l.json.get("relue").is_none_or(Json::is_null) {
            p.fautes.push(format!(
                "après le rechargement, la partie n'est pas reprise de la garde : {}",
                l.json
            ));
        }
        // « Cliquer pour reprendre » couvre la partie reprise jusqu'au geste (lot 10b). Le
        // banc clique sur le voile là où le jeu a son bouton Reset : le clic du geste ne
        // va pas au jeu, et le plateau reste (la remontée du lot 10b).
        let c = attendre_que(
            s,
            Duration::from_secs(60),
            "le bouton Reset sous « Cliquer pour reprendre »",
            |c| bouton(c, "Reset").is_some(),
        )?;
        let (rx, ry) = bouton(&c, "Reset").map(centre).ok_or("pas de Reset")?;
        let d = montrer(s, &[ry])?;
        let (_, h) = fenetre(s)?;
        if !(0.0..h).contains(&(ry - d)) {
            return Err(format!(
                "le bouton Reset n'est pas dans la fenêtre sous le voile : ({rx}, {ry}), défilement {d}"
            ));
        }
        let _: Json = le_geste(s, p, "« Cliquer pour reprendre »", Some((rx, ry - d)))?;
        rien_au_jeu(
            s,
            p,
            depuis,
            "le clic sur « Cliquer pour reprendre », posé sur Reset",
        )?;
        let c = attendre_que(
            s,
            Duration::from_secs(60),
            "le plateau retrouvé après le rechargement",
            |c| images_chargees(c) && plateau(c) == avant,
        );
        match c {
            Ok(_) => p.etapes.push(format!(
                "rechargement : le plateau retrouvé tel quel, images comprises (reprise de la case {})",
                l.json.get("relue").unwrap_or(&Json::Null)
            )),
            Err(e) => {
                let maintenant_ = plateau(&composants(s)?);
                return Err(format!("{e} ; avant {avant} ; après {maintenant_}"));
            }
        }
    }

    // L'export capté.
    let telechargements = s.navigateur.telechargements()?;
    exporter(s, p, &telechargements, "export")?;

    // La sauvegarde et la restauration.
    let sauvegarde = sauvegarder(s, p, &telechargements, "sauvegarde")?;
    {
        let avant = plateau(&composants(s)?);
        let c = composants(s)?;
        let depuis = s.lignes.len();
        cliquer(s, bouton(&c, "Reset").map(centre).ok_or("pas de Reset")?)?;
        let _: Json = attendre_que(
            s,
            Duration::from_secs(10),
            "le plateau remis à zéro",
            |c| plateau(c) != avant,
        )?;
        vu_au_jeu(s, p, depuis, "clic", "le clic sur Reset");
        s.navigateur.fichiers(
            0,
            "input[type=file][data-genres=\".json,application/json\"]",
            std::slice::from_ref(&sauvegarde),
        )?;
        let _: Json = attendre_que(s, Duration::from_secs(30), "le plateau restauré", |c| {
            images_chargees(c) && plateau(c) == avant
        })?;
        p.etapes.push(String::from(
            "restauration : après Reset, la sauvegarde captée redonnée à « Restore Backup » ramène le plateau d'avant",
        ));
    }

    // Le plateau plein d'images : M3 et M8.
    if o.images_plein > 0 {
        plein(s, p, o, &telechargements)?;
    }
    Ok(())
}

/// « Export Image (PNG) », puis le lien du commun cliqué : `tierlist.png` arrive.
fn exporter(s: &mut Session, p: &mut Passage, dossier: &Path, nom: &str) -> Result<(), String> {
    let depuis = s.lignes.len();
    let c = composants(s)?;
    cliquer(
        s,
        bouton(&c, "Export Image (PNG)")
            .map(centre)
            .ok_or("pas d'export")?,
    )?;
    let c = attendre_que(
        s,
        Duration::from_secs(120),
        "l'image exportée et son lien",
        |c| {
            elements(c)
                .iter()
                .any(|e| texte_de(e, "sorte") == "lien" && e.get("pret") == Some(&Json::Bool(true)))
        },
    )?;
    let rendu: Vec<Json> = lignes(s, depuis, "demande")
        .filter(|j| texte_de(j, "sorte") == "rendu")
        .cloned()
        .collect();
    for r in &rendu {
        if texte_de(r, "issue") != "ok" {
            p.fautes.push(format!("{nom} : le rendu a échoué : {r}"));
        }
        p.travail_long.push(format!("{nom} : rendu {r}"));
    }
    if rendu.is_empty() {
        p.fautes.push(format!("{nom} : aucune demande Rendu"));
    }
    let lien = elements(&c)
        .iter()
        .find(|e| texte_de(e, "sorte") == "lien")
        .cloned()
        .ok_or("pas de lien")?;
    let avant = fichiers_du_dossier(dossier);
    cliquer(s, centre(&lien))?;
    let (chemin, octets) =
        attendre_le_fichier(s, &avant, ("tierlist", ".png"), Duration::from_secs(30))?;
    match taille_png(&octets) {
        Some((l, h)) if l == LARGEUR_EXPORT => p.etapes.push(format!(
            "{nom} : « {} » capté, un PNG de {l} × {h}, {} octets",
            chemin.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
            octets.len()
        )),
        autre => p.fautes.push(format!(
            "{nom} : le fichier capté n'est pas le PNG attendu de {LARGEUR_EXPORT} pixels de large : {autre:?}"
        )),
    }
    let c = composants(s)?;
    cliquer(s, bouton(&c, "Close").map(centre).ok_or("pas de croix")?)?;
    let _: Json = attendre_que(
        s,
        Duration::from_secs(10),
        "la fenêtre d'export fermée",
        |c| c.get("modale").is_none_or(Json::is_null),
    )?;
    Ok(())
}

/// « Download Backup » : `tierlist-backup.json` arrive, dans l'activation du clic.
fn sauvegarder(
    s: &mut Session,
    p: &mut Passage,
    dossier: &Path,
    nom: &str,
) -> Result<PathBuf, String> {
    let depuis = s.lignes.len();
    let avant = fichiers_du_dossier(dossier);
    let c = composants(s)?;
    cliquer(
        s,
        bouton(&c, "Download Backup")
            .map(centre)
            .ok_or("pas de Download Backup")?,
    )?;
    let capte = attendre_le_fichier(
        s,
        &avant,
        ("tierlist-backup", ".json"),
        Duration::from_secs(30),
    );
    let en_attente = lignes(s, depuis, "telechargement-en-attente").count();
    let (chemin, octets) = capte.map_err(|e| {
        format!("{nom} : la sauvegarde n'est pas captée ({en_attente} téléchargement(s) laissé(s) au bouton du commun) : {e}")
    })?;
    let j: Json = serde_json::from_slice(&octets)
        .map_err(|e| format!("{nom} : sauvegarde illisible : {e}"))?;
    // Le format est celui du jeu (sa version lui appartient) : le banc exige l'enveloppe.
    let version = j.get("version").and_then(Json::as_u64);
    if j.get("kind").and_then(Json::as_str) != Some("tier-list-backup")
        || version.is_none()
        || j.get("board").is_none_or(|b| !b.is_object())
    {
        p.fautes.push(format!(
            "{nom} : le fichier capté n'est pas une sauvegarde de Tier list"
        ));
    }
    let via: Vec<String> = lignes(s, depuis, "telechargement")
        .map(|j| texte_de(j, "via").to_owned())
        .collect();
    p.etapes.push(format!(
        "{nom} : « {} » capté, {} octets, format {} (téléchargement par {})",
        chemin.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        octets.len(),
        version.unwrap_or(0),
        via.join(", ")
    ));
    Ok(chemin)
}

/// La sauvegarde d'un plateau, ses tuiles de la réserve rangées dans les rangs, tour à
/// tour : celle que le jeu aurait écrite après autant de glissers, dans sa forme exacte
/// (`sauvegarde::octets_avec` de Tier list : les clés dans cet ordre, sans blanc, les
/// textes échappés par `serde_json`), le contenu des tuiles recopié tel quel. Le jeu
/// refuse un fichier qu'il n'aurait pas écrit lui-même (TL-11) : il dit ainsi si elle
/// l'est.
fn ranger_la_sauvegarde(octets: &[u8]) -> Result<Vec<u8>, String> {
    let texte = std::str::from_utf8(octets).map_err(|e| format!("{e}"))?;
    let j: Json = serde_json::from_str(texte).map_err(|e| format!("{e}"))?;
    let b = j.get("board").ok_or("pas de plateau")?;
    let nombres = |v: Option<&Json>| -> Vec<u64> {
        v.and_then(Json::as_array)
            .map(|a| a.iter().filter_map(Json::as_u64).collect())
            .unwrap_or_default()
    };
    let rangs = b
        .get("rows")
        .and_then(Json::as_array)
        .ok_or("pas de rangs")?;
    let mut items: Vec<Vec<u64>> = rangs.iter().map(|r| nombres(r.get("items"))).collect();
    for (k, t) in nombres(b.get("pool")).into_iter().enumerate() {
        if let Some(r) = items.get_mut(k.checked_rem(rangs.len()).unwrap_or(0)) {
            r.push(t);
        }
    }
    // Le texte brut de la table des contenus, recopié : de `,"items":{` à `,"next_item":`.
    let debut = texte
        .find(r#"],"pool":["#)
        .and_then(|i| {
            texte
                .get(i..)?
                .find(r#","items":{"#)
                .map(|j| i.saturating_add(j))
        })
        .ok_or("pas de table des contenus")?;
    let fin = texte.rfind(r#","next_item":"#).ok_or("pas de compteur")?;
    let contenus = texte
        .get(debut..)
        .and_then(|t| t.get(..fin.saturating_sub(debut)))
        .ok_or("table")?;
    let queue = texte.get(fin..).ok_or("queue")?;
    let ecrire = |v: &Json| serde_json::to_string(v).unwrap_or_default();
    let mut o = format!(
        r#"{{"kind":"tier-list-backup","version":{},"board":{{"title":{},"rows":["#,
        ecrire(j.get("version").unwrap_or(&Json::Null)),
        ecrire(b.get("title").unwrap_or(&Json::Null))
    );
    for (k, (r, t)) in rangs.iter().zip(&items).enumerate() {
        if k > 0 {
            o.push(',');
        }
        let liste: Vec<String> = t.iter().map(u64::to_string).collect();
        let _: std::fmt::Result = write!(
            o,
            r#"{{"id":{},"label":{},"color":{},"items":[{}]}}"#,
            ecrire(r.get("id").unwrap_or(&Json::Null)),
            ecrire(r.get("label").unwrap_or(&Json::Null)),
            ecrire(r.get("color").unwrap_or(&Json::Null)),
            liste.join(",")
        );
    }
    o.push_str(r#"],"pool":[]"#);
    o.push_str(contenus);
    o.push_str(queue);
    Ok(o.into_bytes())
}

/// Le plateau plein d'images de bruit, sa sauvegarde écrite et relue, son export ; puis
/// les points de reprise mesurés (M3) et le travail long (M8).
fn plein(s: &mut Session, p: &mut Passage, o: &Options, dossier: &Path) -> Result<(), String> {
    let depuis = s.lignes.len();
    let bruit = ecrire_les_images(&o.travail.join("bruit"), o.images_plein, Images::Bruit)?;
    let avant = tuiles(&composants(s)?);
    let debut = maintenant();
    // Par lots de huit, jusqu'à ce que le jeu en refuse un : il refuse un geste entier
    // qui dépasserait sa borne (`IMAGES_MAX_OCTETS`), et une image encodée en PNG (un
    // navigateur sans encodeur WebP) pèse plus qu'en WebP : la borne vient plus tôt.
    let mut donnees = 0usize;
    let mut refuse = false;
    for lot in bruit.chunks(8) {
        let deja = lignes(s, depuis, "demande")
            .filter(|j| texte_de(j, "sorte") == "image")
            .count();
        let tuiles_avant = tuiles(&composants(s)?);
        s.navigateur
            .fichiers(0, "input[type=file][data-genres=\"image/*\"]", lot)?;
        donnees = donnees.saturating_add(lot.len());
        let fin = maintenant()
            .checked_add(Duration::from_secs(60))
            .ok_or("délai")?;
        loop {
            s.tourner(Duration::from_millis(200))?;
            let faites = lignes(s, depuis, "demande")
                .filter(|j| texte_de(j, "sorte") == "image")
                .count();
            if faites >= deja.saturating_add(lot.len()) {
                break;
            }
            if reste(fin).is_zero() {
                return Err(format!(
                    "plateau plein : {} demande(s) Image sur {} en 60 s",
                    faites.saturating_sub(deja),
                    lot.len()
                ));
            }
        }
        // La réponse entrée, le jeu ajoute (ou refuse) au pas suivant.
        let c = attendre_que(
            s,
            Duration::from_secs(10),
            "le lot ajouté ou refusé",
            |c| {
                images_chargees(c)
                    && (tuiles(c) >= tuiles_avant.saturating_add(lot.len())
                        || elements(c)
                            .iter()
                            .any(|e| texte_de(e, "texte").contains("is full")))
            },
        )?;
        if tuiles(&c) < tuiles_avant.saturating_add(lot.len()) {
            refuse = true;
            break;
        }
    }
    s.tourner(Duration::from_secs(1))?;
    let c2 = composants(s)?;
    p.etapes.push(format!(
        "plateau plein : {donnees} images de bruit données par lots de 8, {} tuile(s) de plus acceptées par le jeu{}, en {:.1} s",
        tuiles(&c2).saturating_sub(avant),
        if refuse { " (le dernier lot refusé : le plateau est plein)" } else { "" },
        maintenant().saturating_duration_since(debut).as_secs_f64()
    ));
    // La sauvegarde du plateau plein, écrite puis relue (le pire cas de M3).
    let fichier = sauvegarder(s, p, dossier, "sauvegarde du plateau plein")?;
    let relue = s.lignes.len();
    s.navigateur.fichiers(
        0,
        "input[type=file][data-genres=\".json,application/json\"]",
        std::slice::from_ref(&fichier),
    )?;
    let l = s.attendre(
        relue,
        Duration::from_secs(60),
        "la sauvegarde pleine relue",
        |l| l.quoi() == "demande" && texte_de(&l.json, "sorte") == "lire",
    )?;
    // Lue entière par la page, puis décodée par le jeu : la même que le plateau ouvert, il
    // le dit (« This backup is the tier list already open. », `DEJA_OUVERTE`) ; une
    // sauvegarde illisible ou refusée dirait autre chose (correction du lot 11).
    let taille = std::fs::metadata(&fichier)
        .map_err(|e| format!("{e}"))?
        .len();
    let lus = l.json.get("octets").and_then(Json::as_u64).unwrap_or(0);
    if texte_de(&l.json, "issue") != "ok" || lus != taille {
        return Err(format!(
            "la sauvegarde pleine n'est pas relue entière : {} ({taille} octets sur disque)",
            l.json
        ));
    }
    let c = attendre_que(
        s,
        Duration::from_secs(30),
        "le jeu dit la sauvegarde pleine déjà ouverte",
        |c| {
            elements(c)
                .iter()
                .any(|e| texte_de(e, "texte").contains("is the tier list already open"))
        },
    )?;
    p.etapes.push(format!(
        "plateau plein : sa sauvegarde ({taille} octets) redonnée à « Restore Backup », lue entière, et le jeu la dit déjà ouverte ({} tuiles)",
        tuiles(&c)
    ));
    // Les tuiles rangées dans les rangs (la réserve n'est pas exportée) : la même
    // sauvegarde, ses tuiles dans les rangs, restaurée.
    let pleine = std::fs::read(&fichier).map_err(|e| format!("{e}"))?;
    let rangee = fichier.with_file_name("tierlist-backup-range.json");
    std::fs::write(&rangee, ranger_la_sauvegarde(&pleine)?).map_err(|e| format!("{e}"))?;
    let total = tuiles(&composants(s)?);
    s.navigateur.fichiers(
        0,
        "input[type=file][data-genres=\".json,application/json\"]",
        std::slice::from_ref(&rangee),
    )?;
    let c = attendre_que(
        s,
        Duration::from_secs(60),
        "les tuiles rangées dans les rangs",
        |c| {
            reserve(c).is_some_and(|z| objets(z).is_empty())
                && tuiles(c) == total
                && images_chargees(c)
        },
    )?;
    p.etapes.push(format!(
        "plateau plein : ses {} tuiles rangées dans les rangs (une sauvegarde que le jeu accepte comme la sienne)",
        tuiles(&c)
    ));
    // L'export du plateau plein, la trace de Chrome autour (M8).
    let trace = s.navigateur.chrome().map(|c| {
        c.tracer(&[
            "disabled-by-default-devtools.timeline.frame",
            "devtools.timeline",
        ])
    });
    let r = exporter(s, p, dossier, "export du plateau plein");
    if let Some(Ok(())) = trace
        && let Some(c) = s.navigateur.chrome()
    {
        match c.fin_de_trace(Duration::from_secs(60)) {
            Ok(evenements) => {
                let compte = crate::dessin::compter_les_images(&evenements);
                p.travail_long.push(format!(
                    "export du plateau plein : trace de Chrome, {} image(s) perdue(s) (DroppedFrame), {}",
                    compte.get("dropped_frame").unwrap_or(&Json::Null),
                    compte.get("par_nom").unwrap_or(&Json::Null)
                ));
            }
            Err(e) => p.travail_long.push(format!("trace de Chrome : {e}")),
        }
    }
    r?;
    // Les points pris après la relecture : au moins deux (un point part toutes les 600
    // pas, si l'empreinte a changé ; l'export en change une).
    let fin = maintenant()
        .checked_add(Duration::from_secs(40))
        .ok_or("délai")?;
    loop {
        s.tourner(Duration::from_millis(500))?;
        if lignes(s, relue, "point").count() >= 2 || reste(fin).is_zero() {
            break;
        }
    }
    p.points = lignes(s, relue, "point").cloned().collect();
    if p.points.is_empty() {
        p.fautes.push(String::from(
            "plateau plein : aucun point de reprise rapporté par le Worker (M3)",
        ));
    }
    for t in lignes(s, depuis, "tour") {
        let ms = t.get("ms").and_then(Json::as_f64).unwrap_or(0.0);
        if ms >= 10.0 {
            p.travail_long.push(format!("tour long du Worker : {t}"));
        }
    }
    Ok(())
}

fn passage(o: &Options, n: Navigateur, moteur: Moteur, paquet: &Path, module: &[u8]) -> Passage {
    let webgl2 = moteur == Moteur::WebGl2;
    let dpr = if (o.dpr - 1.0).abs() > f64::EPSILON {
        format!(" (dpr {})", o.dpr)
    } else {
        String::new()
    };
    let mut p = Passage {
        navigateur: format!(
            "{}{}{dpr}",
            n.nom(),
            if webgl2 { " (WebGL2 forcé)" } else { "" }
        ),
        ..Passage::default()
    };
    let so = soiree::Options {
        module: o.module.clone(),
        navigateurs: vec![n],
        telephones: Some(0),
        construire: false,
        duree_max_s: 0,
        travail: o.travail.clone(),
        m12: 0,
        cache_s: 0,
        fois: 1,
        pages: Vec::new(),
        duree_min_s: 0,
        retardataires: false,
        autres_jeux: Vec::new(),
        fautif: None,
        interface: crate::navigateur::Interface::Sans,
        cacher: soiree::Cacher::Onglet,
    };
    let dossier = o.travail.join(format!(
        "{}{}",
        n.nom(),
        if webgl2 { "-webgl2" } else { "" }
    ));
    let mut s = match soiree::ouvrir(
        &so,
        n,
        paquet,
        module,
        &dossier,
        (None, &requete(o, moteur)),
    ) {
        Ok(s) => s,
        Err(e) if e.starts_with("pas exécuté") => {
            p.pas_execute = Some(e);
            return p;
        }
        Err(e) => {
            p.fautes.push(e);
            return p;
        }
    };
    p.navigateur = format!("{} {}", p.navigateur, s.navigateur.version());
    if let Err(e) = derouler(&mut s, &mut p, o, moteur) {
        p.fautes.push(e);
    }
    let _: Result<(), String> = s.tourner(Duration::from_millis(500));
    for l in &s.lignes {
        match l.quoi() {
            "panne" | "relance" | "commun-en-panne" => {
                p.fautes.push(format!("la page dit : {}", l.json));
            }
            _ => {}
        }
    }
    let lignes: String = s
        .lignes
        .iter()
        .map(|l| format!("{} {} {}\n", l.ms, l.onglet, l.json))
        .chain(s.autres.iter().map(|a| format!("autre {a}\n")))
        .collect();
    let _: std::io::Result<()> = std::fs::create_dir_all(&dossier);
    let _: std::io::Result<()> = std::fs::write(dossier.join("lignes.txt"), lignes);
    s.fermer();
    p
}

/// Joue le scénario dans chaque navigateur (et dans Chrome avec WebGL2 forcé, si voulu).
pub fn jouer(o: &Options, soiree_dir: &Path) -> Result<Vec<Passage>, String> {
    let module = std::fs::read(&o.module).map_err(|e| format!("{} : {e}", o.module.display()))?;
    let _: moteur_wasmtime::ModuleCharge = soiree::charger_le_module(&module)?;
    let paquet = if o.construire {
        soiree::construire_le_paquet(soiree_dir)?
    } else {
        soiree::dernier_paquet(soiree_dir)?
    };
    std::fs::create_dir_all(&o.travail).map_err(|e| format!("{e}"))?;
    // Un chemin absolu : le navigateur lit les fichiers donnés à un sélecteur depuis son
    // propre dossier courant.
    let mut o = o.clone();
    o.travail =
        std::fs::canonicalize(&o.travail).map_err(|e| format!("{} : {e}", o.travail.display()))?;
    let o = &o;
    let mut passages = Vec::new();
    for n in &o.navigateurs {
        passages.push(passage(o, *n, Moteur::Defaut, &paquet, &module));
        if o.webgl2 && *n == Navigateur::Chrome {
            passages.push(passage(o, *n, Moteur::WebGl2, &paquet, &module));
        }
    }
    Ok(passages)
}

impl core::fmt::Display for Passage {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if let Some(e) = &self.pas_execute {
            return writeln!(f, "== {} : {e}", self.navigateur);
        }
        writeln!(
            f,
            "== {} (moteur de dessin {})",
            self.navigateur, self.moteur
        )?;
        for e in &self.etapes {
            writeln!(f, "  {e}")?;
        }
        for pt in &self.points {
            writeln!(f, "  M3, point : {pt}")?;
        }
        for t in &self.travail_long {
            writeln!(f, "  M8, {t}")?;
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

#[cfg(test)]
mod tests {
    use super::{png_brut, taille_png};

    /// Le PNG sans compression du banc a un en-tête que l'on relit (le navigateur le
    /// décode : le scénario le constate).
    #[test]
    fn un_png_brut_dit_sa_taille() {
        let o = png_brut(70, 3, |x, y| {
            [u8::try_from(x).unwrap(), u8::try_from(y).unwrap(), 0]
        });
        assert_eq!(taille_png(&o), Some((70, 3)));
        assert_eq!(taille_png(b"pas un png"), None);
    }
}
