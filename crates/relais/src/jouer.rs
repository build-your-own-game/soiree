//! `soiree-relais jouer` (PHASE-3, § 2.8, P17, P35 ; lot 10b) : la commande unique du jour
//! J, depuis le dossier d'un jeu.
//!
//! 1. **Construire le module** par `soiree-epreuve --construire <jeu>` : la compilation de
//!    l'étape 1 de l'épreuve, la même invocation de cargo (une même source ne donne les
//!    mêmes octets que par le même chemin de compilation), qui rend l'empreinte BLAKE3 du
//!    module et son chemin. Le relais ne hache rien lui-même.
//! 2. **Chercher son rapport par empreinte** dans le catalogue (`<catalogue>/<empreinte>.json`,
//!    que `soiree-epreuve <jeu> --catalogue <catalogue>` dépose sur un code 0 du plan
//!    standard seulement).
//!    **On ne joue qu'un module éprouvé** (P17) : sans rapport, `jouer` refuse, et dit
//!    comment l'éprouver ; `--non-eprouve` le joue quand même, et l'écran affiche « NON
//!    ÉPROUVÉ » en permanence (question 16).
//! 3. Le paquet de la page (`--paquet`, sinon [`crate::preparer`]), le relais, puis
//!    `open http://localhost:7878/?jeu=<empreinte>` (sauf `--sans-ouvrir`).
//!
//! Ce module ne tient que ce qui se décide (le refus, le catalogue servi, la sortie de
//! `--construire`) : le binaire lance les commandes.

use std::path::{Path, PathBuf};

use crate::paquet::Jeux;

/// Ce que `soiree-epreuve --construire` a rendu : l'empreinte BLAKE3 du module livré, en
/// 64 chiffres hexadécimaux minuscules, et son chemin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Construit {
    /// L'empreinte.
    pub empreinte: String,
    /// Le module.
    pub chemin: PathBuf,
}

/// Lit la dernière ligne non vide de la sortie de `soiree-epreuve --construire` :
/// `<empreinte> <chemin du module>`.
pub fn lire_construit(sortie: &str) -> Result<Construit, String> {
    let ligne = sortie
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .ok_or_else(|| String::from("soiree-epreuve --construire n'a rien rendu"))?;
    let (empreinte, chemin) = ligne
        .trim()
        .split_once(' ')
        .ok_or_else(|| format!("soiree-epreuve --construire : « {ligne} » illisible"))?;
    if empreinte.len() != 64
        || !empreinte
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!(
            "soiree-epreuve --construire : « {empreinte} » n'est pas une empreinte BLAKE3"
        ));
    }
    Ok(Construit {
        empreinte: empreinte.to_owned(),
        chemin: PathBuf::from(chemin.trim()),
    })
}

/// Le catalogue par défaut, à côté de la garde : `$HOME/.soiree/catalogue`.
pub fn catalogue_par_defaut() -> PathBuf {
    std::env::var_os("HOME").map_or_else(
        || PathBuf::from(".soiree/catalogue"),
        |h| PathBuf::from(h).join(".soiree/catalogue"),
    )
}

/// Ce que `jouer` fait d'un module sans rapport d'épreuve (P17).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SansRapport {
    /// Le refuser, en disant comment l'éprouver.
    Refuser,
    /// Le jouer quand même (`--non-eprouve`) : l'écran affiche « NON ÉPROUVÉ ».
    Jouer,
}

/// Ce que `jouer` sert : les modules du catalogue, et le module construit. Un module sans
/// rapport est refusé, sauf [`SansRapport::Jouer`] (P17) ; le refus dit comment
/// l'éprouver.
///
/// `octets` sont ceux du module construit ; s'il est au catalogue avec son rapport, c'est
/// la copie du catalogue qui est servie, et elle doit avoir les mêmes octets (sinon,
/// refusé).
pub fn les_jeux(
    catalogue: &Path,
    construit: &Construit,
    octets: Vec<u8>,
    sans_rapport: SansRapport,
    jeu: &Path,
) -> Result<Jeux, String> {
    let mut jeux = if catalogue.is_dir() {
        Jeux::lire(catalogue)?
    } else {
        Jeux::default()
    };
    if jeux.eprouve(&construit.empreinte) {
        // Le relais ne hache rien : il compare les octets construits à ceux que l'épreuve
        // a déposés sous cette empreinte (après la vérification du lot 10b). Une sortie de
        // `--construire` qui ne dirait pas l'empreinte de ses octets ne fait pas passer un
        // autre module pour l'éprouvé.
        let au_catalogue = jeux.servir(&format!("{}.wasm", construit.empreinte));
        if au_catalogue.is_none_or(|(_, o)| o != octets) {
            return Err(format!(
                "le module construit ({}) n'a pas les octets que le catalogue garde sous son empreinte, dans {} : refusé (P17). Relance soiree-epreuve {} --catalogue {}",
                construit.chemin.display(),
                catalogue.display(),
                jeu.display(),
                catalogue.display()
            ));
        }
        return Ok(jeux);
    }
    if sans_rapport == SansRapport::Refuser {
        return Err(format!(
            "ce module ({}) n'a pas de rapport d'épreuve dans {} : on ne joue qu'un module éprouvé (P17).\n  Éprouve-le : soiree-epreuve {} --catalogue {}\n  ou joue-le sans épreuve, « NON ÉPROUVÉ » affiché à l'écran : soiree-relais jouer {} --non-eprouve",
            construit.empreinte,
            catalogue.display(),
            jeu.display(),
            catalogue.display(),
            jeu.display()
        ));
    }
    jeux.ajouter_non_eprouve(&construit.empreinte, octets)?;
    Ok(jeux)
}

/// Où trouver `soiree-epreuve` : `SOIREE_EPREUVE`, sinon à côté de ce binaire, sinon dans
/// le chemin.
pub fn epreuve() -> PathBuf {
    if let Some(e) = std::env::var_os("SOIREE_EPREUVE") {
        return PathBuf::from(e);
    }
    std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|d| d.join("soiree-epreuve")))
        .filter(|e| e.is_file())
        .unwrap_or_else(|| PathBuf::from("soiree-epreuve"))
}
