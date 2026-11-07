//! `soiree-relais preparer` (PHASE-3, § 2.5, § 2.8 ; lot 10b) : construire **une fois** le
//! paquet de la page, que le relais charge ensuite en mémoire (`--paquet`).
//!
//! Le workspace `web/` du commun pour `wasm32-unknown-unknown` (`ecran`, `autorite` et,
//! depuis le lot 12, `telephone`, en `--release`), puis la CLI `wasm-bindgen` à la version épinglée de la crate
//! ([`WASM_BINDGEN`], REM-156), puis `web/demarrage.js`, dans
//! `<cible>/<empreinte>/`. Le banc construit le sien par cette même fonction
//! (`banc::soiree`) : une seule définition du paquet.
//!
//! **L'empreinte du paquet** nomme son dossier, et donc l'adresse de ses fichiers, servis
//! `immutable` : elle n'a qu'à changer quand un fichier change. C'est un FNV-1a de 128
//! bits des noms, des tailles et des octets, écrit ici : le relais ne tire aucune crate de
//! hachage ni de chiffrement (`tests/relais/graphe.rs`), et cette empreinte ne protège
//! rien (le paquet est construit et servi sur la même machine ; le Worker vérifie, lui, le
//! BLAKE3 de chaque module de jeu). 32 chiffres hexadécimaux, la longueur des paquets du
//! lot 10a.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// La version de `wasm-bindgen`, la crate de `web/` comme l'outil (PHASE-3, P32 ;
/// REM-156) : une seule définition, que le banc relit.
pub const WASM_BINDGEN: &str = "0.2.129";

/// Le dossier du commun dont ce relais a été compilé : celui de ses sources, pour
/// `cargo install --git` comme pour un chemin (le paquet se construit de ces sources-là,
/// aux mêmes versions que le relais).
pub fn commun_des_sources() -> PathBuf {
    let ici = Path::new(env!("CARGO_MANIFEST_DIR"));
    ici.parent()
        .and_then(Path::parent)
        .map_or_else(|| ici.to_path_buf(), Path::to_path_buf)
}

/// FNV-1a sur 128 bits (base et premier de la spécification de FNV).
#[derive(Debug, Clone, Copy)]
struct Fnv128(u128);

impl Fnv128 {
    const BASE: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
    const PREMIER: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;

    const fn new() -> Fnv128 {
        Fnv128(Fnv128::BASE)
    }

    fn ajouter(&mut self, octets: &[u8]) {
        for o in octets {
            self.0 ^= u128::from(*o);
            self.0 = self.0.wrapping_mul(Fnv128::PREMIER);
        }
    }
}

/// L'empreinte d'un paquet : FNV-1a 128 des fichiers dans l'ordre de leur nom, chacun par
/// son nom, sa taille (8 octets, petit-boutiste) et ses octets ; 32 chiffres hexadécimaux.
pub fn empreinte_du_paquet(fichiers: &[(String, Vec<u8>)]) -> String {
    let mut tries: Vec<&(String, Vec<u8>)> = fichiers.iter().collect();
    tries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut h = Fnv128::new();
    for (nom, octets) in tries {
        h.ajouter(nom.as_bytes());
        h.ajouter(
            &u64::try_from(octets.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        h.ajouter(octets);
    }
    let mut s = String::with_capacity(32);
    for o in h.0.to_be_bytes() {
        let _: core::fmt::Result = write!(s, "{o:02x}");
    }
    s
}

/// Lance une commande ; rend sa sortie standard, ou une erreur qui la nomme, avec la fin
/// de sa sortie d'erreur.
fn lancer(c: &mut Command, quoi: &str) -> Result<String, String> {
    let sortie = c
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{quoi} : {e}"))?;
    if sortie.status.success() {
        return Ok(String::from_utf8_lossy(&sortie.stdout).into_owned());
    }
    let erreur = String::from_utf8_lossy(&sortie.stderr);
    let fin: Vec<&str> = erreur.lines().rev().take(20).collect();
    Err(format!(
        "{quoi} : {} ; {}",
        sortie.status,
        fin.into_iter().rev().collect::<Vec<_>>().join("\n")
    ))
}

/// La CLI `wasm-bindgen` installée est-elle à la version de la crate (REM-156) ?
pub fn verifier_wasm_bindgen() -> Result<(), String> {
    let version = lancer(
        Command::new("wasm-bindgen").arg("--version"),
        "wasm-bindgen --version",
    )?;
    if version.split_whitespace().nth(1) != Some(WASM_BINDGEN) {
        return Err(format!(
            "wasm-bindgen {} installé, la crate est à {WASM_BINDGEN} : `cargo install wasm-bindgen-cli --version {WASM_BINDGEN}` (REM-156)",
            version.trim()
        ));
    }
    Ok(())
}

/// Construit le paquet de la page depuis le commun `commun` (son dossier `web/`), dans
/// `cible/<empreinte>/` (par défaut `<commun>/target/soiree-web/`) ; rend ce dossier.
/// Un paquet de même empreinte déjà là est remplacé par le neuf, aux mêmes octets.
pub fn construire_le_paquet(commun: &Path, cible: Option<&Path>) -> Result<PathBuf, String> {
    let web = commun.join("web");
    if !web.join("Cargo.toml").is_file() {
        return Err(format!(
            "{} : pas de workspace web/ (le commun dont le relais a été compilé ? `--commun <dossier>`)",
            web.display()
        ));
    }
    verifier_wasm_bindgen()?;
    let racine = cible.map_or_else(|| commun.join("target/soiree-web"), Path::to_path_buf);
    let compilation = web.join("target");
    let mut cargo = Command::new(std::env::var("CARGO").unwrap_or_else(|_| String::from("cargo")));
    cargo
        .current_dir(&web)
        .args([
            "build",
            "-p",
            "ecran",
            "-p",
            "autorite",
            "-p",
            "telephone",
            "--lib",
            "--target",
            "wasm32-unknown-unknown",
            "--release",
            "--target-dir",
        ])
        .arg(&compilation);
    let _: String = lancer(
        &mut cargo,
        "cargo build de ecran, autorite et telephone (web/, wasm32)",
    )?;
    let brouillon = racine.join("brouillon");
    let _: std::io::Result<()> = std::fs::remove_dir_all(&brouillon);
    std::fs::create_dir_all(&brouillon).map_err(|e| format!("{} : {e}", brouillon.display()))?;
    for nom in ["ecran", "autorite", "telephone"] {
        let wasm = compilation.join(format!("wasm32-unknown-unknown/release/{nom}.wasm"));
        let _: String = lancer(
            Command::new("wasm-bindgen")
                .args(["--target", "web", "--no-typescript", "--out-dir"])
                .arg(&brouillon)
                .arg(&wasm),
            &format!("wasm-bindgen de {nom}"),
        )?;
    }
    std::fs::copy(web.join("demarrage.js"), brouillon.join("demarrage.js"))
        .map_err(|e| format!("demarrage.js : {e}"))?;
    let mut fichiers = Vec::new();
    for entree in std::fs::read_dir(&brouillon).map_err(|e| format!("{e}"))? {
        let chemin = entree.map_err(|e| format!("{e}"))?.path();
        let nom = chemin
            .file_name()
            .and_then(|f| f.to_str())
            .ok_or_else(|| format!("{} : un nom illisible", chemin.display()))?
            .to_owned();
        let octets = std::fs::read(&chemin).map_err(|e| format!("{} : {e}", chemin.display()))?;
        fichiers.push((nom, octets));
    }
    let dossier = racine.join(empreinte_du_paquet(&fichiers));
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    std::fs::rename(&brouillon, &dossier).map_err(|e| format!("{} : {e}", dossier.display()))?;
    Ok(dossier)
}

/// Le paquet construit le plus récemment sous `racine` (un dossier nommé par une
/// empreinte de 32 chiffres).
pub fn dernier_paquet(racine: &Path) -> Result<PathBuf, String> {
    let mut paquets: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(racine)
        .map_err(|e| format!("{} : {e}", racine.display()))?
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_str().is_some_and(|n| n.len() == 32))
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .collect();
    paquets.sort();
    paquets
        .pop()
        .map(|(_, p)| p)
        .ok_or_else(|| format!("aucun paquet sous {}", racine.display()))
}

#[cfg(test)]
mod tests {
    use super::{Fnv128, empreinte_du_paquet};

    /// FNV-1a 128 : les valeurs publiées pour le texte vide et « a ».
    #[test]
    fn fnv1a_128_rend_les_valeurs_publiees() {
        let mut h = Fnv128::new();
        h.ajouter(b"");
        assert_eq!(h.0, 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d);
        let mut h = Fnv128::new();
        h.ajouter(b"a");
        assert_eq!(h.0, 0xd228_cb69_6f1a_8caf_7891_2b70_4e4a_8964);
    }

    /// L'empreinte ne dépend pas de l'ordre où les fichiers sont lus, et change avec un
    /// octet, un nom ou une frontière entre deux fichiers.
    #[test]
    fn l_empreinte_du_paquet_suit_chaque_fichier() {
        let a = (String::from("a.js"), b"ab".to_vec());
        let b = (String::from("b.wasm"), b"c".to_vec());
        let e = empreinte_du_paquet(&[a.clone(), b.clone()]);
        assert_eq!(e.len(), 32);
        assert_eq!(e, empreinte_du_paquet(&[b.clone(), a.clone()]));
        assert_ne!(
            e,
            empreinte_du_paquet(&[(String::from("a.js"), b"aa".to_vec()), b.clone()])
        );
        assert_ne!(
            e,
            empreinte_du_paquet(&[
                (String::from("a.js"), b"a".to_vec()),
                (String::from("b.wasm"), b"bc".to_vec())
            ])
        );
        assert_ne!(
            e,
            empreinte_du_paquet(&[(String::from("x.js"), b"ab".to_vec()), b])
        );
    }
}
