//! Ce qu'un jeu montre sur l'étagère de la soirée : son titre, à combien on
//! y joue, et sa jaquette.
//!
//! ```toml
//! [workspace.metadata.soiree]
//! titre = "Course de canards"
//! joueurs = [2, 8]
//! ```
//!
//! La jaquette est `jaquette.png`, à la racine du jeu : une image debout, en
//! 3:4.

use std::fs;
use std::path::Path;

use crate::rules;

/// Le fichier de la jaquette, à la racine du jeu.
pub const COVER: &str = "jaquette.png";
const TITLE_MAX: usize = 40;
const PLAYERS_MAX: i64 = 99;
/// Une jaquette plus étroite est floue sur une télé.
const WIDTH_MIN: u32 = 600;
const COVER_MAX: u64 = 2_000_000;
const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

const EXAMPLE: &str = "  Écris-le dans le Cargo.toml de la racine du jeu :\n\n    \
     [workspace.metadata.soiree]\n    \
     titre = \"Le nom du jeu, comme on le lit sur l'étagère\"\n    \
     joueurs = [2, 8]          # le moins et le plus de joueurs";

const DRAW: &str = "  La jaquette est l'image du jeu sur l'étagère de la soirée, celle qu'on choisit \
     depuis le canapé : `jaquette.png` à la racine du jeu, debout, en 3:4 (900 × 1200 par exemple), \
     avec le titre lisible de loin et le style du jeu.\n  \
     Ton agent peut la fabriquer : une page HTML ou un SVG aux bonnes dimensions, puis une capture \
     en PNG. Tu peux aussi lui donner une image à toi.";

/// La fiche d'un jeu sur l'étagère.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    /// Le moins et le plus de joueurs.
    pub players: (u32, u32),
}

/// Lit la fiche du jeu dans son `Cargo.toml`, ou dit ce qui manque.
pub fn card(root: &Path) -> Result<Card, Vec<String>> {
    let manifest = rules::read_toml(&root.join("Cargo.toml")).map_err(|error| vec![error])?;
    let scope = if manifest.contains_key("workspace") {
        "workspace"
    } else {
        "package"
    };
    let table = manifest
        .get(scope)
        .and_then(|scope| scope.get("metadata")?.get("soiree")?.as_table());
    let Some(table) = table else {
        return Err(vec![format!(
            "Cargo.toml n'a pas de section [{scope}.metadata.soiree]"
        )]);
    };
    let mut wrong = Vec::new();
    let mut text = |key: &str, most: usize| {
        let value = table
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .unwrap_or_default();
        if value.is_empty() {
            wrong.push(format!("`{key}` manque, ou n'est pas un texte"));
        } else if value.chars().count() > most {
            wrong.push(format!(
                "`{key}` fait plus de {most} caractères : sur l'étagère, il serait coupé"
            ));
        }
        value.to_owned()
    };
    let title = text("titre", TITLE_MAX);
    let players = table
        .get("joueurs")
        .and_then(toml::Value::as_array)
        .and_then(|players| match players.as_slice() {
            [least, most] => Some((least.as_integer()?, most.as_integer()?)),
            _ => None,
        })
        .filter(|(least, most)| 1 <= *least && least <= most && *most <= PLAYERS_MAX)
        .and_then(|(least, most)| Some((u32::try_from(least).ok()?, u32::try_from(most).ok()?)));
    if players.is_none() {
        wrong.push(
            "`joueurs` manque, ou n'est pas `[le moins, le plus]` : `[2, 8]`, ou `[1, 1]` pour un \
             jeu en solo"
                .to_owned(),
        );
    }
    match players {
        Some(players) if wrong.is_empty() => Ok(Card { title, players }),
        Some(_) | None => Err(wrong),
    }
}

/// La largeur et la hauteur d'une image PNG, lues dans son en-tête.
fn png_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let rest = bytes.strip_prefix(&PNG)?;
    // La longueur du premier bloc, son nom (`IHDR`), puis les dimensions.
    let (_, rest) = rest.split_first_chunk::<4>()?;
    let rest = rest.strip_prefix(b"IHDR")?;
    let (width, rest) = rest.split_first_chunk::<4>()?;
    let (height, _) = rest.split_first_chunk::<4>()?;
    Some((u32::from_be_bytes(*width), u32::from_be_bytes(*height)))
}

/// Les octets de la jaquette, ou ce qui ne va pas.
fn cover(root: &Path) -> Result<Vec<u8>, String> {
    let path = root.join(COVER);
    let Ok(bytes) = fs::read(&path) else {
        return Err(format!("{COVER} manque à la racine du jeu"));
    };
    let Some((width, height)) = png_size(&bytes) else {
        return Err(format!("{COVER} n'est pas une image PNG"));
    };
    if u64::from(width).saturating_mul(4) != u64::from(height).saturating_mul(3) {
        return Err(format!(
            "{COVER} fait {width} × {height} : il faut une image debout en 3:4, comme 900 × 1200"
        ));
    }
    if width < WIDTH_MIN {
        return Err(format!(
            "{COVER} fait {width} × {height} : trop petite pour une télé, il faut au moins \
             {WIDTH_MIN} de large"
        ));
    }
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > COVER_MAX {
        return Err(format!(
            "{COVER} pèse plus de 2 Mo : enregistre-la plus légère"
        ));
    }
    Ok(bytes)
}

/// Le jeu a une fiche complète et une jaquette à lui.
pub fn check(root: &Path) -> Result<(), String> {
    let mut fixes = Vec::new();
    let card = card(root);
    // Le jeu de départ garde sa jaquette : c'est lui qu'on reconnaît.
    let is_starter = card
        .as_ref()
        .is_ok_and(|card| card.title == crate::new::TITLE);
    if let Err(wrong) = &card {
        fixes.push(format!("  {}\n{EXAMPLE}", wrong.join("\n  ")));
    }
    let fault = match cover(root) {
        Ok(bytes) if !is_starter && crate::new::starter_file(COVER) == Some(bytes.as_slice()) => {
            Some(format!(
                "{COVER} est encore la jaquette du jeu de départ : ce jeu a la sienne"
            ))
        }
        Ok(_) => None,
        Err(fault) => Some(fault),
    };
    if let Some(fault) = fault {
        fixes.push(format!("  {fault}\n{DRAW}"));
    }
    if fixes.is_empty() {
        Ok(())
    } else {
        Err(fixes.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(name: &str, manifest: &str, cover: Option<&[u8]>) -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("soiree-fiche-{name}-{}", std::process::id()));
        let _: std::io::Result<()> = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Cargo.toml"), manifest).unwrap();
        if let Some(cover) = cover {
            fs::write(root.join(COVER), cover).unwrap();
        }
        root
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG.to_vec();
        bytes.extend_from_slice(&13_u32.to_be_bytes());
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    const FULL: &str = "[workspace]\n[workspace.metadata.soiree]\ntitre = \"Canards\"\n\
                        joueurs = [2, 8]\n";

    #[test]
    fn a_full_card_and_a_cover_of_its_own_pass() {
        let root = game("complet", FULL, Some(&png(900, 1200)));
        assert_eq!(
            card(&root),
            Ok(Card {
                title: "Canards".to_owned(),
                players: (2, 8),
            })
        );
        assert_eq!(check(&root), Ok(()));
    }

    #[test]
    fn what_is_missing_is_named_with_what_to_write() {
        let root = game("vide", "[workspace]\n", None);
        let refusal = check(&root).unwrap_err();
        assert!(refusal.contains("[workspace.metadata.soiree]"));
        assert!(refusal.contains("jaquette.png manque"));

        let half =
            "[workspace]\n[workspace.metadata.soiree]\ntitre = \"Canards\"\njoueurs = [8, 2]\n";
        let root = game("moitie", half, Some(&png(800, 600)));
        let refusal = check(&root).unwrap_err();
        assert!(refusal.contains("`joueurs` manque"));
        assert!(refusal.contains("800 × 600"));
        assert!(!refusal.contains("`titre`"));
    }

    #[test]
    fn the_starter_cover_is_refused_in_any_other_game() {
        let cover = crate::new::starter_file(COVER).unwrap();
        let root = game("depart", FULL, Some(cover));
        assert!(check(&root).unwrap_err().contains("jeu de départ"));
        let starter = FULL.replace("Canards", crate::new::TITLE);
        let root = game("modele", &starter, Some(cover));
        assert_eq!(check(&root), Ok(()));
    }
}
