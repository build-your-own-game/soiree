//! La machine d'un jeu : le crate qui remplit `cartouche::Machine`, les
//! règles du jeu. Même graine et mêmes entrées, même partie, sur
//! l'ordinateur comme dans un navigateur.
//!
//! La console le vérifie de deux façons :
//! - par principe : la machine se compile pour une cible sans système, où
//!   l'horloge, les fichiers, le réseau et le hasard du système n'existent pas ;
//! - par l'exemple : le banc de la console (l'exemple `banc`, le même pour
//!   tous les jeux) joue des parties au hasard. On le lance en natif, deux
//!   fois, puis en WebAssembly, et les trois sorties sont les mêmes ; puis une
//!   fois en natif, plus longtemps (`--long`), où il photographie, reprend et
//!   chronomètre les parties. Un exemple `empreinte` du jeu, s'il en a un, est
//!   lancé comme le banc sans argument.

use std::path::Path;
use std::process::{Command, ExitCode, Stdio};

use crate::cartridge::{self, BENCH};
use crate::rules::{Metadata, Package};
use crate::wasm;

/// L'exemple que le jeu peut garder, qui joue des parties écrites d'avance.
const OWN: &str = "empreinte";
/// La cible WebAssembly qui sait écrire sur la sortie.
const TARGET: &str = "wasm32-wasip1";
/// Une cible sans système : seul le langage y existe.
const BARE: &str = "wasm32v1-none";
/// Ce que `glam` doit activer dans une machine : pas d'instructions propres à
/// un processeur, et les mêmes fonctions mathématiques partout.
const GLAM_FEATURES: [&str; 2] = ["libm", "scalar-math"];
/// La brique dont la machine remplit le contrat, et l'option du banc.
const CARTOUCHE: &str = "cartouche";
const BENCH_FEATURE: &str = "bench";

/// Le jeu a une machine, une seule, et elle a de quoi se faire vérifier.
pub fn declared(metadata: &Metadata) -> Result<(), String> {
    let machines: Vec<&Package> = metadata.machines().collect();
    let machine = match machines.as_slice() {
        [] => return Err(undeclared()),
        [one] => *one,
        [_, _, ..] => {
            let names: Vec<&str> = machines.iter().map(|one| one.name.as_str()).collect();
            return Err(format!(
                "  Plusieurs crates sont déclarés comme la machine du jeu : {}.\n  \
                 Un jeu n'a qu'une machine, le crate qui remplit `cartouche::Machine` : le \
                 programme et le banc la nomment. Garde `[package.metadata.soiree] machine = \
                 true` sur celui-là seulement ; les autres crates peuvent rester, comme \
                 dépendances de la machine.",
                names.join(", ")
            ));
        }
    };
    let name = &machine.name;
    let mut wrong: Vec<String> = Vec::new();
    if machine.is_page() {
        wrong.push(format!(
            "  Le crate {name} est une page (`cdylib`) : la machine est un crate à part, que \
             le programme et les pages empruntent."
        ));
    }
    if !machine.has_library() {
        wrong.push(format!(
            "  Le crate {name} n'a pas de bibliothèque (`src/lib.rs`) : la machine est une \
             bibliothèque, qui exporte le type `Jeu`."
        ));
    }
    wrong.extend(borrows_the_contract(machine));
    if let Some(library) = machine.library() {
        wrong.extend(has_the_bench(metadata, machine, &library));
    }
    if wrong.is_empty() {
        Ok(())
    } else {
        Err(wrong.join("\n"))
    }
}

/// Ce que dit `soiree check` à un jeu qui n'a pas encore déclaré sa machine.
fn undeclared() -> String {
    "  Aucun crate de ce jeu n'est déclaré comme sa machine.\n  \
     La machine est le crate qui remplit le contrat de la console, `cartouche::Machine` : \
     les règles du jeu, sans réseau, sans horloge et sans navigateur. Elle exporte un type \
     `Jeu` ; la console lui donne une graine et tout ce qui arrive à la partie, une entrée \
     après l'autre, et lui demande ce que chaque page doit montrer. C'est ce qui permet à \
     la console de jouer des parties toute seule, de les rejouer à l'identique et de les \
     comparer. `soiree contrat` affiche le contrat entier.\n  \
     Déclare-la dans le `Cargo.toml` de ce crate :\n\n      \
     [package.metadata.soiree]\n      machine = true\n\n  \
     Avant de désigner un crate, cherche où vivent vraiment les règles : dans le programme \
     du jeu (états, transitions, minuteurs, tirages) et dans les pages (ce qu'un geste a le \
     droit de faire, où tombe ce qu'on lâche, ce qu'on refuse). Tout ce qui décide de la \
     partie va dans la machine ; les pages ne font que lui envoyer des commandes et \
     montrer ce qu'elle rend. Si les règles sont mêlées au réseau ou à l'affichage, \
     sors-les dans un crate à part, par exemple `machine/`.\n  \
     Puis lance `soiree rules` : il écrit le banc de la machine (`examples/banc.rs`) et le \
     programme du jeu (`src/main.rs`). `soiree check` dira ensuite ce qui manque encore."
        .to_owned()
}

/// La machine emprunte `cartouche` sans le banc, et son exemple `banc` avec.
fn borrows_the_contract(machine: &Package) -> Vec<String> {
    let name = &machine.name;
    let cartouche = |kind: Option<&str>| {
        machine
            .dependencies
            .iter()
            .find(|one| one.name == CARTOUCHE && one.kind.as_deref() == kind)
    };
    let with_bench = |one: &crate::rules::Dependency| {
        one.features.iter().any(|feature| feature == BENCH_FEATURE)
    };
    let mut wrong: Vec<String> = Vec::new();
    match cartouche(None) {
        None => wrong.push(format!(
            "  La machine {name} n'emprunte pas la brique `cartouche`, dont elle remplit le \
             contrat. À la racine, dans `[workspace.dependencies]` du `Cargo.toml` :\n\n      \
             cartouche = {{ {source} }}\n\n  \
             et dans le `Cargo.toml` de la machine :\n\n      \
             [dependencies]\n      cartouche.workspace = true",
            source = crate::new::brick_source()
        )),
        Some(one) if with_bench(one) => wrong.push(format!(
            "  La machine {name} emprunte `cartouche` avec l'option `{BENCH_FEATURE}` dans \
             `[dependencies]`. Le banc demande le système, et la machine s'en passe : retire \
             `{BENCH_FEATURE}` de `[dependencies]` (et de `[workspace.dependencies]` s'il y est). \
             Il va dans `[dev-dependencies]`, pour l'exemple `{BENCH}` seulement."
        )),
        Some(_) => {}
    }
    if !cartouche(Some("dev")).is_some_and(with_bench) {
        wrong.push(format!(
            "  L'exemple `{BENCH}` de la machine {name} emprunte le banc de la brique \
             `cartouche`. Dans le `Cargo.toml` de la machine :\n\n      \
             [dev-dependencies]\n      \
             cartouche = {{ workspace = true, features = [\"{BENCH_FEATURE}\"] }}"
        ));
    }
    wrong
}

/// La machine a le banc de la console, tel que `soiree rules` l'écrit.
fn has_the_bench(metadata: &Metadata, machine: &Package, library: &str) -> Option<String> {
    let name = &machine.name;
    let path = cartridge::bench_path(machine);
    let shown = metadata.shown(&path);
    match std::fs::read_to_string(&path) {
        Err(_) => Some(format!(
            "  La machine {name} n'a pas le banc de la console ({shown}). Lance `soiree rules` : \
             il l'écrit. Le banc est le même pour tous les jeux : il joue des parties au hasard \
             avec le `Jeu` de la machine, les photographie, les reprend, et compare le natif et \
             le WebAssembly."
        )),
        Ok(text) if text != cartridge::bench_file(library) => Some(format!(
            "  {shown} n'est pas le banc de la console : ce fichier est le même dans tous les \
             jeux. Lance `soiree rules` : il le réécrit. Pour aller là où le hasard ne va pas, \
             écris plutôt des tests, ou un exemple à toi, `examples/{OWN}.rs`, que `soiree \
             check` lance aussi."
        )),
        Ok(_) if !machine.has_example(BENCH) => Some(format!(
            "  {shown} est là, mais cargo ne le voit pas comme un exemple de la machine {name}. \
             Dans son `Cargo.toml`, retire `autoexamples = false`, ou déclare-le :\n\n      \
             [[example]]\n      name = \"{BENCH}\""
        )),
        Ok(_) => None,
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

/// Les exemples de la machine que la console lance.
#[derive(Clone, Copy)]
enum Example {
    /// Le banc de la console, le même pour tous les jeux.
    Bench,
    /// L'exemple du jeu, qui joue des parties écrites d'avance.
    Own,
}

impl Example {
    fn name(self) -> &'static str {
        match self {
            Example::Bench => BENCH,
            Example::Own => OWN,
        }
    }

    /// Ce que l'exemple affiche, ligne par ligne.
    fn lines(self) -> &'static str {
        match self {
            Example::Bench => "ligne(s) du banc",
            Example::Own => "ligne(s) d'empreinte",
        }
    }

    /// Ce qu'on dit quand il n'affiche rien.
    fn silent(self, machine: &str) -> String {
        match self {
            Example::Bench => format!(
                "  Le banc de la machine {machine} n'affiche rien. Lance `soiree rules` : il \
                 réécrit `examples/{BENCH}.rs`."
            ),
            Example::Own => format!(
                "  L'exemple `{OWN}` de la machine {machine} n'affiche rien.\n  \
                 Il doit jouer des parties et afficher l'empreinte de leur état, une ligne par \
                 étape (`cartouche::fingerprint`)."
            ),
        }
    }

    /// Ce qu'on dit quand il s'arrête sur une erreur en natif.
    fn stopped(self, machine: &str) -> String {
        match self {
            Example::Bench => format!(
                "  Le banc de la machine {machine} s'arrête : une partie a planté, et son \
                 explication est juste au-dessus. Pour relancer le banc seul : \
                 `cargo run --release -p {machine} --example {BENCH}`."
            ),
            Example::Own => format!(
                "  L'exemple `{OWN}` de la machine {machine} ne se lance pas ou s'arrête sur une \
                 erreur : `cargo run --release -p {machine} --example {OWN}`."
            ),
        }
    }

    /// Par où chercher quand le natif et le WebAssembly diffèrent.
    fn narrowing(self) -> &'static str {
        match self {
            Example::Bench => {
                "La première ligne qui diffère dit dans quelle partie, et à quel pas, les deux \
                 se séparent : c'est là que se trouve le calcul en cause."
            }
            Example::Own => {
                "Réduis l'exemple jusqu'à la première ligne qui diffère : c'est elle qui montre \
                 le calcul en cause."
            }
        }
    }
}

/// Chaque machine rend la même partie : deux fois en natif, puis en
/// WebAssembly ; et le banc, plus long, ne trouve rien à redire.
pub fn replays(metadata: &Metadata) -> Result<(), String> {
    for machine in metadata.machines() {
        compare(metadata, machine, Example::Bench)?;
        long(&metadata.workspace_root, &machine.name)?;
        if machine.has_example(OWN) {
            compare(metadata, machine, Example::Own)?;
        }
    }
    Ok(())
}

/// Le banc long : il photographie, reprend, chronomètre. Son explication,
/// s'il refuse, passe telle quelle sur la sortie d'erreur.
fn long(root: &Path, name: &str) -> Result<(), String> {
    println!("\n── la machine {name} : `{BENCH} --long`, en natif");
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["run", "--quiet", "--release", "--package", name])
        .args(["--example", BENCH, "--", "--long"])
        .status()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "  L'explication du banc est juste au-dessus. Pour relancer le banc seul :\n\n      \
             cargo run --release -p {name} --example {BENCH} -- --long"
        ))
    }
}

fn compare(metadata: &Metadata, machine: &Package, example: Example) -> Result<(), String> {
    let root = &metadata.workspace_root;
    let name = &machine.name;
    let file_name = example.name();
    println!("\n── la machine {name} : `{file_name}` en natif, deux fois, puis en WebAssembly");
    let first = native(root, name, example)?;
    if first.trim().is_empty() {
        return Err(example.silent(name));
    }
    let second = native(root, name, example)?;
    if let Some(difference) = difference(&first, &second) {
        return Err(format!(
            "  Deux lancements de suite, sur cet ordinateur, ne donnent pas la même partie.\n\
             {difference}\n  \
             La machine lit quelque chose qui change d'un lancement à l'autre : l'horloge, le \
             hasard du système, l'ordre de parcours d'une `HashMap`, une adresse mémoire. \
             Tout ce qui varie doit entrer par la graine ou par les entrées."
        ));
    }
    build(root, name, file_name)?;
    let file = metadata
        .target_directory
        .join(TARGET)
        .join("release")
        .join("examples")
        .join(format!("{file_name}.wasm"));
    // Le même programme WebAssembly rend toujours les mêmes lignes : s'il n'a
    // pas changé depuis la dernière comparaison réussie, et la sortie native
    // non plus, on ne le relance pas (l'interpréteur est lent).
    let bytes = std::fs::read(&file).map_err(|error| format!("  {} : {error}", file.display()))?;
    let proof = format!("{:016x} {:016x}\n", print(&bytes), print(first.as_bytes()));
    let memo = metadata
        .target_directory
        .join("soiree")
        .join(format!("{name}.{file_name}"));
    if std::fs::read_to_string(&memo).ok().as_deref() == Some(proof.as_str()) {
        println!(
            "   {} {}, identiques (la machine n'a pas changé depuis la dernière comparaison)",
            first.lines().count(),
            example.lines()
        );
        return Ok(());
    }
    let web = wasm::run(&bytes).map_err(|error| {
        format!("  En WebAssembly, `{file_name}` de la machine {name} s'arrête :\n  {error}")
    })?;
    if let Some(difference) = difference(&first, &web) {
        return Err(format!(
            "  La machine {name} ne joue pas la même partie en natif et en WebAssembly.\n\
             {difference}\n  \
             Cherche, à partir de cette étape : une fonction mathématique du système (`sin`, \
             `cos`, `powf`, `exp`…) dans la machine ou dans une de ses dépendances ; un calcul \
             sur `usize` ou `isize` (64 bits en natif, 32 en WebAssembly), ou une telle valeur \
             dans l'état, dans une commande ou dans ce que l'exemple affiche ; une bibliothèque \
             qui calcule autrement selon le processeur. {}",
            example.narrowing()
        ));
    }
    println!(
        "   {} {}, identiques",
        first.lines().count(),
        example.lines()
    );
    // Sans ce fichier, on recompare la prochaine fois : rien de grave.
    if let Some(dir) = memo.parent() {
        let _: std::io::Result<()> = std::fs::create_dir_all(dir);
    }
    let _: std::io::Result<()> = std::fs::write(&memo, proof);
    Ok(())
}

/// `soiree rejouer <fichier>` : le banc refait la partie d'un journal, à
/// l'identique. Rend le code de sortie du banc.
pub fn replay_journal(args: &[String]) -> Result<ExitCode, String> {
    let [file] = args else {
        return Err("usage : soiree rejouer <fichier>\n  \
             Le fichier est le journal d'une partie : la console les écrit dans `.soiree/`, à la \
             racine du jeu. Lance la commande dans le dossier du jeu."
            .to_owned());
    };
    let file = Path::new(file)
        .canonicalize()
        .map_err(|error| format!("{file} : impossible de lire ce journal ({error})"))?;
    let metadata = crate::rules::metadata(Path::new("."))?;
    let Some(machine) = cartridge::machine(&metadata) else {
        return Err(
            "ce jeu n'a pas une machine déclarée, et une seule : lance `soiree check`, il dit \
             quoi corriger."
                .to_owned(),
        );
    };
    let name = &machine.name;
    let file_text = file.to_string_lossy();
    println!("── cargo run --release -p {name} --example {BENCH} -- --rejouer {file_text}");
    let status = Command::new("cargo")
        .current_dir(&metadata.workspace_root)
        .args(["run", "--quiet", "--release", "--package", name])
        .args(["--example", BENCH, "--", "--rejouer", &file_text])
        .status()
        .map_err(|error| format!("impossible de lancer cargo : {error}"))?;
    Ok(status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map_or(ExitCode::FAILURE, ExitCode::from))
}

/// L'empreinte d'un fichier ou d'une sortie (FNV-1a sur 64 bits).
fn print(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |print, byte| {
        (print ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Lance l'exemple sur cet ordinateur et rend ce qu'il affiche. Ce qu'il
/// écrit sur sa sortie d'erreur passe tel quel.
fn native(root: &Path, name: &str, example: Example) -> Result<String, String> {
    let output = Command::new("cargo")
        .current_dir(root)
        .args(["run", "--quiet", "--release", "--package", name])
        .args(["--example", example.name()])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(example.stopped(name))
    }
}

/// Compile l'exemple pour WebAssembly.
fn build(root: &Path, name: &str, example: &str) -> Result<(), String> {
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "--quiet", "--release", "--package", name])
        .args(["--example", example, "--target", TARGET])
        .status()
        .map_err(|error| format!("  impossible de lancer cargo : {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "  L'exemple `{example}` de la machine {name} ne compile pas pour WebAssembly \
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
