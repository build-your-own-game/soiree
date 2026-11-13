//! La machine d'un jeu : le crate qui porte ses règles. Même graine et mêmes
//! entrées, même partie, sur l'ordinateur comme dans un navigateur.
//!
//! La console le vérifie de deux façons :
//! - par principe : la machine se compile pour une cible sans système, où
//!   l'horloge, les fichiers, le réseau et le hasard du système n'existent pas ;
//! - par l'exemple : la machine a un exemple `empreinte` qui joue des parties
//!   et affiche l'empreinte de leur état. On le lance en natif, deux fois, puis
//!   en WebAssembly, et les trois sorties sont les mêmes.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::rules::{Metadata, Package};
use crate::wasm;

/// Le nom de l'exemple que la console lance.
const EXAMPLE: &str = "empreinte";
/// La cible WebAssembly qui sait écrire sur la sortie.
const TARGET: &str = "wasm32-wasip1";
/// Une cible sans système : seul le langage y existe.
const BARE: &str = "wasm32v1-none";
/// Ce que `glam` doit activer dans une machine : pas d'instructions propres à
/// un processeur, et les mêmes fonctions mathématiques partout.
const GLAM_FEATURES: [&str; 2] = ["libm", "scalar-math"];

/// Le jeu a une machine, et elle a de quoi se faire vérifier.
pub fn declared(metadata: &Metadata) -> Result<(), String> {
    let machines: Vec<&Package> = metadata.machines().collect();
    if machines.is_empty() {
        return Err(
            "  Aucun crate de ce jeu n'est déclaré comme sa machine.\n  \
             La machine est le crate qui porte les règles du jeu, sans réseau, sans horloge et \
             sans navigateur : on lui donne une graine et des entrées, elle rend la partie. \
             C'est ce qui permet de rejouer une partie à l'identique et de la tester.\n  \
             Déclare-la dans le `Cargo.toml` de ce crate :\n\n      \
             [package.metadata.soiree]\n      machine = true\n\n  \
             Avant de désigner un crate, cherche où vivent vraiment les règles : dans le \
             programme du jeu (états, transitions, minuteurs, tirages) et dans les pages (ce \
             qu'un geste a le droit de faire, où tombe ce qu'on lâche, ce qu'on refuse). Tout \
             ce qui décide de la partie va dans la machine ; le reste ne fait que lui porter \
             des entrées et afficher ce qu'elle rend. Si les règles sont mêlées au réseau ou \
             à l'affichage, sors-les dans un crate à part, par exemple `machine/`.\n  \
             La machine devra ensuite se compiler sans système (`#![no_std]`) et avoir un \
             exemple `examples/empreinte.rs` qui joue des parties et affiche l'empreinte de \
             leur état : `soiree check` dira comment, le moment venu."
                .to_owned(),
        );
    }
    let mut wrong: Vec<String> = Vec::new();
    for machine in machines {
        if machine.is_page() {
            wrong.push(format!(
                "  Le crate {} est une page (`cdylib`) : la machine est un crate à part, que le \
                 jeu et ses pages peuvent emprunter.",
                machine.name
            ));
        }
        if !machine.has_library() {
            wrong.push(format!(
                "  Le crate {} n'a pas de bibliothèque (`src/lib.rs`) : la machine est une \
                 bibliothèque, que le programme du jeu emprunte.",
                machine.name
            ));
        }
        if !machine.has_example(EXAMPLE) {
            wrong.push(format!(
                "  La machine {name} n'a pas d'exemple `{EXAMPLE}` \
                 ({dir}/examples/{EXAMPLE}.rs).\n  \
                 Écris-le : il joue quelques parties entières, avec des graines et des entrées \
                 fixées et variées (tous les coups du jeu, des entrées absurdes aussi), et \
                 affiche au fil de chaque partie des lignes `nom de la partie, étape : \
                 empreinte de l'état` avec `println!`.\n  \
                 L'empreinte est une fonction de la machine (ses champs sont privés) : un \
                 nombre calculé sur tout son état, champ par champ, par leurs octets, les \
                 flottants par leurs bits. Pas de `#[derive(Hash)]` : il compte les longueurs \
                 en `usize`, qui n'a pas la même taille en WebAssembly. Pour la même raison, \
                 l'exemple n'affiche ni ne donne en entrée de valeur tirée de `usize` \
                 (`usize::MAX`, une longueur) : prends des entiers de taille fixe.\n  \
                 `soiree check` le lance en natif et en WebAssembly et compare les lignes : \
                 leur forme est libre. En WebAssembly il tourne dans un interpréteur, bien \
                 plus lent : garde l'exemple à quelques secondes en natif au plus.",
                name = machine.name,
                dir = machine
                    .dir()
                    .strip_prefix(&metadata.workspace_root)
                    .unwrap_or(machine.dir())
                    .display(),
            ));
        }
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("\n"))
    }
}

/// Chaque machine se compile pour une cible sans système : ce qui n'existe
/// pas ne peut pas rendre la partie différente d'une fois sur l'autre.
pub fn no_system(metadata: &Metadata) -> Result<(), String> {
    for machine in metadata.machines() {
        let name = &machine.name;
        if let Some(glam) = machine.dependencies.iter().find(|one| one.name == "glam") {
            let missing: Vec<&str> = GLAM_FEATURES
                .into_iter()
                .filter(|feature| !glam.features.iter().any(|one| one == feature))
                .collect();
            if !missing.is_empty() {
                return Err(format!(
                    "  La machine {name} emprunte `glam` sans {}.\n  \
                     Par défaut, `glam` calcule avec des instructions propres au processeur et \
                     les fonctions mathématiques du système : le dernier chiffre change entre \
                     l'ordinateur et le navigateur. Dans une machine, écris :\n\n      \
                     glam = {{ version = \"…\", default-features = false, features = [\"libm\", \
                     \"scalar-math\"] }}",
                    missing
                        .iter()
                        .map(|feature| format!("`{feature}`"))
                        .collect::<Vec<_>>()
                        .join(" ni ")
                ));
            }
        }
        println!("\n── cargo check --package {name} --lib --target {BARE}");
        let status = Command::new("cargo")
            .current_dir(&metadata.workspace_root)
            .args(["check", "--package", name, "--lib", "--target", BARE])
            .status()
            .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
        if !status.success() {
            return Err(format!(
                "  La machine {name} ne se compile pas pour une cible sans système (`{BARE}`).\n  \
                 C'est voulu : sur cette cible, l'horloge, les fichiers, le réseau, le hasard du \
                 système et `HashMap` n'existent pas. Une machine qui s'en passe rejoue la même \
                 partie partout. Par où sortir :\n  \
                 · En tête de `src/lib.rs` : `#![no_std]`, puis `extern crate alloc;`. Les \
                 listes et les textes viennent alors de `alloc` : `use alloc::vec::Vec;`, \
                 `alloc::string::String`, `alloc::collections::BTreeMap`, \
                 `alloc::borrow::ToOwned`, `alloc::format`.\n  \
                 · Les fonctions mathématiques absentes (`sqrt`, `floor`, `round`, `fract`, \
                 `sin`, `powf`…) : celles du crate `libm`, le même code partout \
                 (`libm::sqrtf(x)`, `libm::floorf(x)`, `x - libm::truncf(x)` ; sans `f` pour \
                 un `f64`). Ou `glam` avec `default-features = false, features = \
                 [\"libm\", \"scalar-math\"]`.\n  \
                 · Une dépendance qui demande le système (`can't find crate for std`) : \
                 éteins ses options par défaut (`serde = {{ version = \"1\", default-features = \
                 false, features = [\"derive\", \"alloc\"] }}`, `serde_json = {{ version = \"1\", \
                 default-features = false, features = [\"alloc\"] }}`), et si c'est un crate du jeu \
                 (ses messages, par exemple), passe-le lui aussi en `#![no_std]`. Sinon, sors \
                 de la machine le code qui en a besoin.\n  \
                 · L'horloge et le hasard : le temps et la graine sont des entrées de la \
                 machine. C'est le programme du jeu qui lit l'horloge et tire la graine.\n  \
                 · Les tests et les exemples de la machine gardent le système. Pour les \
                 tests, en tête de `src/lib.rs` : `#[cfg(test)] #[macro_use] extern crate std;`, \
                 puis des imports nommés dans le module de tests (`use std::vec::Vec;`). \
                 N'importe pas tout le prélude de `std` : `panic!` deviendrait ambigu.\n  \
                 Si la cible manque : `rustup target add {BARE}`."
            ));
        }
    }
    Ok(())
}

/// Chaque machine rend la même partie : deux fois en natif, puis en WebAssembly.
pub fn replays(metadata: &Metadata) -> Result<(), String> {
    for machine in metadata.machines() {
        replay(metadata, machine)?;
    }
    Ok(())
}

fn replay(metadata: &Metadata, machine: &Package) -> Result<(), String> {
    let root = &metadata.workspace_root;
    let name = &machine.name;
    println!("\n── la machine {name} : `{EXAMPLE}` en natif, deux fois, puis en WebAssembly");
    let first = native(root, name)?;
    if first.trim().is_empty() {
        return Err(format!(
            "  L'exemple `{EXAMPLE}` de la machine {name} n'affiche rien.\n  \
             Il doit jouer des parties et afficher l'empreinte de leur état, une ligne par étape."
        ));
    }
    let second = native(root, name)?;
    if let Some(difference) = difference(&first, &second) {
        return Err(format!(
            "  Deux lancements de suite, sur cet ordinateur, ne donnent pas la même partie.\n\
             {difference}\n  \
             La machine lit quelque chose qui change d'un lancement à l'autre : l'horloge, le \
             hasard du système, l'ordre de parcours d'une `HashMap`, une adresse mémoire. \
             Tout ce qui varie doit entrer par la graine ou par les entrées."
        ));
    }
    build(root, name)?;
    let file = metadata
        .target_directory
        .join(TARGET)
        .join("release")
        .join("examples")
        .join(format!("{EXAMPLE}.wasm"));
    // Le même programme WebAssembly rend toujours les mêmes lignes : s'il n'a
    // pas changé depuis la dernière comparaison réussie, et la sortie native
    // non plus, on ne le relance pas (l'interpréteur est lent).
    let bytes = std::fs::read(&file).map_err(|error| format!("  {} : {error}", file.display()))?;
    let proof = format!("{:016x} {:016x}\n", print(&bytes), print(first.as_bytes()));
    let memo = metadata
        .target_directory
        .join("soiree")
        .join(format!("{name}.{EXAMPLE}"));
    if std::fs::read_to_string(&memo).ok().as_deref() == Some(proof.as_str()) {
        println!(
            "   {} ligne(s) d'empreinte, identiques (la machine n'a pas changé depuis la \
             dernière comparaison)",
            first.lines().count()
        );
        return Ok(());
    }
    let web = wasm::run(&bytes).map_err(|error| {
        format!(
            "  En WebAssembly, l'exemple `{EXAMPLE}` de la machine {name} s'arrête :\n  {error}"
        )
    })?;
    if let Some(difference) = difference(&first, &web) {
        return Err(format!(
            "  La machine {name} ne joue pas la même partie en natif et en WebAssembly.\n\
             {difference}\n  \
             Cherche, à partir de cette étape : une fonction mathématique du système (`sin`, \
             `cos`, `powf`, `exp`…) dans la machine ou dans une de ses dépendances ; un calcul \
             sur `usize` ou `isize` (64 bits en natif, 32 en WebAssembly), ou une telle valeur \
             que l'exemple affiche ou donne en entrée ; une bibliothèque qui \
             calcule autrement selon le processeur. Réduis l'exemple jusqu'à la première ligne \
             qui diffère : c'est elle qui montre le calcul en cause."
        ));
    }
    println!(
        "   {} ligne(s) d'empreinte, identiques",
        first.lines().count()
    );
    // Sans ce fichier, on recompare la prochaine fois : rien de grave.
    if let Some(dir) = memo.parent() {
        let _: std::io::Result<()> = std::fs::create_dir_all(dir);
    }
    let _: std::io::Result<()> = std::fs::write(&memo, proof);
    Ok(())
}

/// L'empreinte d'un fichier ou d'une sortie (FNV-1a sur 64 bits).
fn print(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |print, byte| {
        (print ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Lance l'exemple sur cet ordinateur et rend ce qu'il affiche.
fn native(root: &Path, name: &str) -> Result<String, String> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["run", "--quiet", "--release", "--package", name])
        .args(["--example", EXAMPLE])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "  L'exemple `{EXAMPLE}` de la machine {name} ne se lance pas ou s'arrête sur une \
             erreur : `cargo run --release -p {name} --example {EXAMPLE}`."
        ))
    }
}

/// Compile l'exemple pour WebAssembly.
fn build(root: &Path, name: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "--quiet", "--release", "--package", name])
        .args(["--example", EXAMPLE, "--target", TARGET])
        .status()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "  L'exemple `{EXAMPLE}` de la machine {name} ne compile pas pour WebAssembly \
             (`{TARGET}`). Si l'erreur vient d'une dépendance de développement (`[dev-dependencies]`), \
             c'est elle qui ne se compile pas pour cette cible : l'exemple doit s'en passer.\n  \
             Une machine ne dépend ni du réseau, ni des fichiers, ni du navigateur : retire la \
             dépendance en cause, ou sors de la machine le code qui en a besoin. Si la cible \
             manque : `rustup target add {TARGET}`."
        ))
    }
}

/// La première ligne qui diffère entre deux sorties.
fn difference(expected: &str, actual: &str) -> Option<String> {
    let mut expected_lines = expected.lines();
    let mut actual_lines = actual.lines();
    let mut number: usize = 1;
    loop {
        let (one, other) = (expected_lines.next(), actual_lines.next());
        if one.is_none() && other.is_none() {
            return None;
        }
        if one != other {
            return Some(format!(
                "  Ligne {number} :\n      d'abord : {}\n      ensuite : {}",
                one.unwrap_or("(plus rien)"),
                other.unwrap_or("(plus rien)")
            ));
        }
        number = number.saturating_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_differing_line_is_named() {
        assert_eq!(difference("a\nb\n", "a\nb\n"), None);
        let text = difference("a\nb\nc\n", "a\nx\nc\n").unwrap();
        assert!(text.contains("Ligne 2"));
        assert!(text.contains("d'abord : b"));
        assert!(text.contains("ensuite : x"));
        assert!(difference("a\n", "a\nb\n").unwrap().contains("(plus rien)"));
    }
}
