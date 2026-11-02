//! `soiree recherche dette`: what research is owed, by level and by gravity.
//! It informs everywhere; the one place it blocks is « Avant d'étiqueter », in
//! the common repository's `AGENTS.md`, and that step is read, not computed.

use std::path::Path;
use std::process::ExitCode;

use super::Folder;
use super::fiche::{Gravite, Perimetre, SousSysteme, Trou};
use crate::report::Report;
use crate::util;

/// From this level on, a subsystem without a sheet is a debt on its own.
const OWED_LEVEL: u8 = 3;

pub struct OpenHole<'a> {
    pub sheet: &'a str,
    pub niveau: u8,
    pub trou: &'a Trou,
}

pub struct Debt<'a> {
    pub missing: Vec<&'a SousSysteme>,
    pub holes: Vec<OpenHole<'a>>,
    pub unread: Vec<&'a str>,
}

impl<'a> Debt<'a> {
    pub fn of(folder: &'a Folder) -> Self {
        let scope = folder.perimetre;
        let missing = folder
            .registre
            .sous_systemes
            .iter()
            .filter(|s| s.perimetre == scope && s.niveau >= OWED_LEVEL)
            .filter(|s| !folder.sheets.iter().any(|sheet| sheet.stem == s.id))
            .collect();
        let mut holes = Vec::new();
        let mut unread = Vec::new();
        for sheet in &folder.sheets {
            match &sheet.parsed {
                Ok(fiche) => holes.extend(fiche.open().map(|trou| OpenHole {
                    sheet: &fiche.sous_systeme,
                    niveau: fiche.niveau,
                    trou,
                })),
                Err(_) => unread.push(sheet.file.as_str()),
            }
        }
        holes.sort_by_key(|hole| (hole.trou.gravite, hole.sheet));
        Self {
            missing,
            holes,
            unread,
        }
    }

    /// The lines that must be settled or excused before a tag.
    fn level_three(&self) -> usize {
        let holes = self
            .holes
            .iter()
            .filter(|hole| hole.niveau >= OWED_LEVEL)
            .count();
        self.missing.len().saturating_add(holes)
    }

    fn count(&self, gravite: Gravite) -> usize {
        self.holes
            .iter()
            .filter(|hole| hole.trou.gravite == gravite)
            .count()
    }

    /// One line, for `soiree check`.
    pub fn line(&self) -> String {
        if let Some(file) = self.unread.first() {
            return format!("dette de recherche non lue : {file} est illisible");
        }
        if self.missing.is_empty() && self.holes.is_empty() {
            return "dette de recherche : aucune".to_owned();
        }
        format!(
            "dette de recherche : {} sous-système(s) de niveau {OWED_LEVEL} sans fiche, {} trou(s) ouvert(s) ({} haute, {} moyenne, {} faible) ; détail : soiree recherche dette",
            self.missing.len(),
            self.holes.len(),
            self.count(Gravite::Haute),
            self.count(Gravite::Moyenne),
            self.count(Gravite::Faible)
        )
    }
}

pub fn run(dir: &Path) -> Result<ExitCode, String> {
    let dir = util::absolute(dir);
    let folder = Folder::load(&dir)?;
    let debt = Debt::of(&folder);
    let mut report = Report::new();
    report.section(&format!("dette de recherche : {}", dir.display()));
    for file in &debt.unread {
        report.error(format!("{file} illisible : sa dette n'est pas comptée"));
    }
    report.section(&format!(
        "niveau {OWED_LEVEL} sans fiche : {}",
        debt.missing.len()
    ));
    for sous_systeme in &debt.missing {
        report.info(format!(
            "{} : recherche/{}.toml manque",
            sous_systeme.id, sous_systeme.id
        ));
    }
    report.section(&format!("trous ouverts : {}", debt.holes.len()));
    for hole in &debt.holes {
        let place = hole
            .trou
            .lieux
            .first()
            .map(ToString::to_string)
            .unwrap_or_default();
        report.info(format!(
            "{} : {} {} (niveau {}) : {} [{place}]",
            hole.trou.gravite, hole.sheet, hole.trou.id, hole.niveau, hole.trou.titre
        ));
    }
    println!();
    match folder.perimetre {
        Perimetre::Commun => println!(
            "Niveau {OWED_LEVEL} : {} ligne(s) à solder, ou à justifier dans la section de la version de CHANGELOG.md. Cela ne bloque qu'à l'étiquette (AGENTS.md, « Avant d'étiqueter »).",
            debt.level_three()
        ),
        Perimetre::Jeu => {
            println!("Dans un jeu, cette dette reste une information : rien ne bloque.");
        }
    }
    Ok(if debt.unread.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

#[cfg(test)]
mod tests {
    use super::super::Sheet;
    use super::super::fiche::{self, Fiche, Registre};
    use super::*;

    const SHEET: &str = include_str!("../../../recherche/reconnexion.toml");
    const REGISTRY: &str = include_str!("../../../recherche/sous-systemes.toml");

    fn folder(sheets: Vec<Sheet>, perimetre: Perimetre) -> Folder {
        let registre: Registre = fiche::read(REGISTRY, "r.toml").unwrap();
        Folder {
            registre,
            sheets,
            sources: None,
            perimetre,
        }
    }

    fn pilot(text: &str) -> Sheet {
        Sheet {
            file: "recherche/reconnexion.toml".to_owned(),
            stem: "reconnexion".to_owned(),
            parsed: fiche::read(text, "reconnexion.toml"),
        }
    }

    #[test]
    fn missing_sheets_and_open_holes_are_counted_worst_first() {
        let folder = folder(vec![pilot(SHEET)], Perimetre::Commun);
        let debt = Debt::of(&folder);
        let missing: Vec<&str> = debt.missing.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(missing, ["fiabilite", "transport-webrtc", "telecommande"]);
        let parsed: Fiche = fiche::read(SHEET, "reconnexion.toml").unwrap();
        let open = parsed.open().count();
        assert_eq!(debt.holes.len(), open);
        assert!(
            debt.holes
                .windows(2)
                .all(|pair| pair[0].trou.gravite <= pair[1].trou.gravite)
        );
        assert_eq!(debt.level_three(), 3 + open);
        assert!(debt.line().contains("3 sous-système(s)"), "{}", debt.line());
        assert!(
            debt.line().contains(&format!("{open} trou(s) ouvert(s)")),
            "{}",
            debt.line()
        );
    }

    #[test]
    fn a_closed_hole_leaves_the_debt() {
        let closed = SHEET.replace("etat = \"trou\"", "etat = \"couvert\"\ngarantie = [\"C2\"]");
        let folder = folder(vec![pilot(&closed)], Perimetre::Commun);
        let debt = Debt::of(&folder);
        assert_eq!(debt.holes.len(), 0);
        assert!(debt.line().contains("0 trou(s)"), "{}", debt.line());
    }

    #[test]
    fn a_game_owes_no_sheet_and_a_broken_sheet_is_not_a_zero() {
        let folder_of_game = folder(Vec::new(), Perimetre::Jeu);
        let debt = Debt::of(&folder_of_game);
        assert_eq!(debt.line(), "dette de recherche : aucune");
        let broken = folder(vec![pilot("niveau = ")], Perimetre::Jeu);
        let debt = Debt::of(&broken);
        assert!(debt.line().contains("non lue"), "{}", debt.line());
    }
}
