//! `soiree publish` : met le jeu sur GitHub, dans l'organisation de la
//! soirée. C'est sa sauvegarde, et c'est là que l'hôte le récupère : un jeu
//! qui est dans l'organisation est proposé à la soirée.

use std::path::Path;

use crate::sh;

/// L'organisation GitHub où vivent la console et les jeux.
pub const ORG: &str = "build-your-own-game";

pub fn run(dir: &Path) -> Result<(), String> {
    let root = dir
        .canonicalize()
        .map_err(|error| format!("{} : {error}", dir.display()))?;
    if !root.join("Cargo.toml").is_file() {
        return Err(format!(
            "{} n'est pas un jeu : lance `soiree publish` à la racine du jeu, là où est son Cargo.toml.",
            root.display()
        ));
    }
    let name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| crate::new::is_valid_name(name))
        .ok_or_else(|| {
            format!(
                "le dossier du jeu donne son nom au dépôt : renomme {} en minuscules, chiffres et tirets.",
                root.display()
            )
        })?;

    let login = account(&root)?;
    membership(&root, &login)?;

    if !sh::quiet(&root, "git", &["rev-parse", "--git-dir"])
        && !sh::run(&root, "git", &["init", "--initial-branch", "main"])
    {
        return Err("git init a échoué : voir le message ci-dessus.".to_owned());
    }
    let has_commit = sh::quiet(&root, "git", &["rev-parse", "--verify", "HEAD"]);
    if !has_commit {
        let committed = sh::run(&root, "git", &["add", "--all"])
            && sh::run(&root, "git", &["commit", "--message", "Le jeu de départ"]);
        if !committed {
            return Err(
                "le premier commit a échoué : voir le message de git ci-dessus. S'il demande qui tu es, \
                 suis ce qu'il propose (git config user.name et user.email), puis relance `soiree publish`."
                    .to_owned(),
            );
        }
    }

    let repository = format!("{ORG}/{name}");
    if sh::quiet(&root, "git", &["remote", "get-url", "origin"]) {
        if !sh::run(&root, "git", &["push", "--set-upstream", "origin", "HEAD"]) {
            return Err("git push a échoué : voir le message ci-dessus.".to_owned());
        }
    } else if !sh::run(
        &root,
        "gh",
        &[
            "repo",
            "create",
            &repository,
            "--private",
            "--source",
            ".",
            "--remote",
            "origin",
            "--push",
        ],
    ) {
        return Err(format!(
            "le dépôt {repository} n'a pas pu être créé : voir le message ci-dessus. \
             Si ce nom est déjà pris, renomme le dossier du jeu et relance."
        ));
    }
    println!("\nTon jeu est sur GitHub : https://github.com/{repository}");
    println!("L'hôte le récupère de là. Il sera joué à la soirée s'il passe `soiree check`.");
    if sh::output(&root, "git", &["status", "--porcelain"])
        .is_some_and(|changes| !changes.is_empty())
    {
        println!(
            "Des changements ne sont pas encore commités : ils ne sont pas partis. \
             Committe-les, puis relance `soiree publish`."
        );
    }
    Ok(())
}

/// Le pseudo GitHub de la personne, ou ce qu'il lui manque pour en avoir un.
fn account(root: &Path) -> Result<String, String> {
    if !sh::quiet(root, "git", &["--version"]) {
        return Err("git manque sur cet ordinateur : https://git-scm.com/downloads".to_owned());
    }
    if !sh::quiet(root, "gh", &["--version"]) {
        return Err(
            "l'outil de GitHub (`gh`) manque sur cet ordinateur : https://cli.github.com\n\
             Installe-le, puis relance `soiree publish`."
                .to_owned(),
        );
    }
    sh::output(root, "gh", &["api", "user", "--jq", ".login"]).ok_or_else(|| {
        "tu n'es pas connecté à GitHub sur cet ordinateur.\n\
         Pas encore de compte ? Crée-le : https://github.com/signup\n\
         Puis lance `gh auth login`, réponds à ses questions, et relance `soiree publish`."
            .to_owned()
    })
}

/// La personne est-elle dans l'organisation ? Sinon, quoi faire.
fn membership(root: &Path, login: &str) -> Result<(), String> {
    let path = format!("user/memberships/orgs/{ORG}");
    match sh::output(root, "gh", &["api", &path, "--jq", ".state"]).as_deref() {
        Some("active") => Ok(()),
        Some("pending") => Err(format!(
            "l'hôte t'a invité dans l'organisation {ORG}, mais tu n'as pas encore accepté.\n\
             Accepte ici : https://github.com/orgs/{ORG}/invitation\n\
             Puis relance `soiree publish`."
        )),
        Some(_) | None => Err(format!(
            "tu n'es pas encore dans l'organisation {ORG}, où vivent les jeux de la soirée.\n\
             Donne ton pseudo GitHub à l'hôte : {login}\n\
             Il t'invite, tu acceptes l'invitation reçue par mail, puis tu relances `soiree publish`."
        )),
    }
}
