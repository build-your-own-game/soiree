//! `soiree`: the tool of the soirée. See `soiree help`.

mod catalogue;
mod certify;
mod check;
mod ci;
mod doctor;
mod dom;
mod embedded;
mod gaps;
mod harness;
mod host_setup;
mod items;
mod lints;
mod markdown;
mod mutants;
mod new;
mod report;
mod status;
mod ui_contract;
mod upgrade;
mod util;
mod verification;

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "soiree : l'outil de la soirée

  soiree doctor [dossier]            ce qui manque à cet ordinateur ; n'installe rien
  soiree new <nom> [--org O] [--dans D]
                                     crée un jeu depuis le modèle
  soiree check [--fast] [--locked] [--published] [dossier…]
                                     règles, manifeste, certifications ; puis fmt,
                                     Clippy natif et WASM, tests (sauf --fast)
  soiree contracts [--write-view] [--migrate REFERENCES.json] [dossier]
                                     plan JSON ; vue lisible ou migration explicite
  soiree certify [dossier] [--harness H] [-- options du harnais]
                                     le harnais complet ; écrit certification/
  soiree mutants [dossier] [--in-diff FICHIER] [--re REGEX] [--timeout S]
                                     mutation du core, à la demande ; rapport diagnostic
  soiree dom <fichier.rs> [--html H…] [--css C…] [--id ID BALISE…] [--data D…] [--attr A…]
                                     écrit le module typé des noms d'une page :
                                     ids, classes, data-*, attributs, propriétés CSS
  soiree upgrade [--dry-run] [dossier] aligne le jeu sur la version de ce binaire
  soiree status [dossier]            dépôts pas commités, pas poussés, sans amont
  soiree host-setup [dossier] [--author P | --all] [--force] [--dry-run] [--source URL]
                                     installe le dossier de l'hôte : AGENTS.md,
                                     .cargo/config.toml, soiree/ et ses jeux
  soiree version

Sans dossier : le dossier courant. Le harnais de cette version est embarqué : certify
l'écrit dans un cache et y lance npm ci la première fois ; --harness ou SOIREE_HARNESS
désignent une copie de travail.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("soiree : {error}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

fn run(args: &[String]) -> Result<ExitCode, String> {
    let Some((command, rest)) = args.split_first() else {
        println!("{HELP}");
        return Ok(ExitCode::SUCCESS);
    };
    match command.as_str() {
        "help" | "-h" | "--help" => {
            println!("{HELP}");
            Ok(ExitCode::SUCCESS)
        }
        "version" | "-V" | "--version" => {
            println!("soiree {}", embedded::VERSION);
            Ok(ExitCode::SUCCESS)
        }
        "check" => {
            let mut options = check::Options::default();
            let mut dirs = Vec::new();
            for arg in rest {
                match arg.as_str() {
                    "--fast" => options.fast = true,
                    "--locked" => options.locked = true,
                    "--published" => options.published = true,
                    flag if flag.starts_with('-') => {
                        return Err(format!("check : option inconnue {flag}"));
                    }
                    dir => dirs.push(PathBuf::from(dir)),
                }
            }
            if dirs.is_empty() {
                dirs.push(PathBuf::from("."));
            }
            Ok(check::run(&dirs, options))
        }
        "status" => Ok(status::run(&one_dir(rest)?)),
        "contracts" => contracts(rest),
        "doctor" => Ok(doctor::run(&one_dir(rest)?)),
        "new" => {
            let mut name = None;
            let mut org = embedded::ORG.to_owned();
            let mut parent = PathBuf::from(".");
            let mut rest = rest.iter();
            while let Some(arg) = rest.next() {
                match arg.as_str() {
                    "--org" => org.clone_from(rest.next().ok_or("new : --org sans valeur")?),
                    "--dans" => {
                        parent = PathBuf::from(rest.next().ok_or("new : --dans sans dossier")?);
                    }
                    flag if flag.starts_with('-') => {
                        return Err(format!("new : option inconnue {flag}"));
                    }
                    value if name.is_none() => name = Some(value.to_owned()),
                    value => return Err(format!("new : argument en trop {value}")),
                }
            }
            let name = name.ok_or("new : il manque le nom du jeu")?;
            Ok(new::run(&name, &parent, &org))
        }
        "certify" => {
            let mut parts = rest.splitn(2, |a| a == "--");
            let own = parts.next().unwrap_or_default();
            let extra = parts.next().unwrap_or_default();
            let mut harness: Option<PathBuf> = None;
            let mut dirs = Vec::new();
            let mut own = own.iter();
            while let Some(arg) = own.next() {
                match arg.as_str() {
                    "--harness" => {
                        harness = Some(PathBuf::from(
                            own.next().ok_or("certify : --harness sans dossier")?,
                        ));
                    }
                    flag if flag.starts_with('-') => {
                        return Err(format!(
                            "certify : option inconnue {flag} (celles du harnais vont après --)"
                        ));
                    }
                    _ => dirs.push(arg.clone()),
                }
            }
            Ok(certify::run(&one_dir(&dirs)?, harness.as_deref(), extra))
        }
        "mutants" => Ok(mutants::run(rest)),
        "dom" => Ok(dom::run(rest)),
        "upgrade" => upgrade::run(rest),
        "host-setup" => Ok(host_setup::run(&host_setup_options(rest)?)),
        other => Err(format!("commande inconnue : {other}")),
    }
}

fn contracts(args: &[String]) -> Result<ExitCode, String> {
    let mut write = false;
    let mut migration = None;
    let mut dirs = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--write-view" => write = true,
            "--migrate" => {
                migration = Some(PathBuf::from(
                    args.next().ok_or("--migrate attend REFERENCES.json")?,
                ));
            }
            flag if flag.starts_with('-') => {
                return Err(format!("contracts : option inconnue {flag}"));
            }
            dir => dirs.push(dir.to_owned()),
        }
    }
    let dir = one_dir(&dirs)?;
    if let Some(references) = migration {
        let count = verification::migrate::run(&dir, &references)?;
        println!("{count} engagements migrés ; aucune exécution importée");
    }
    let contract = verification::load(&dir)?;
    if write {
        verification::render::write_view(&dir, &contract)?;
    }
    let config = util::read(&dir.join("cert.json"))?;
    let config = serde_json::from_str(&config).map_err(|e| e.to_string())?;
    let plan = verification::resolve::resolve(&contract, &config);
    println!(
        "{}",
        serde_json::to_string_pretty(&plan).map_err(|e| e.to_string())?
    );
    Ok(if plan.diagnostics.is_empty() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn host_setup_options(args: &[String]) -> Result<host_setup::Options, String> {
    let mut options = host_setup::Options::new(PathBuf::from("."));
    let mut dirs = Vec::new();
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--force" => options.force = true,
            "--dry-run" => options.dry_run = true,
            "--all" => options.all = true,
            "--author" => {
                options.author = Some(
                    rest.next()
                        .ok_or("host-setup : --author sans pseudo")?
                        .clone(),
                );
            }
            "--source" => {
                options
                    .source
                    .clone_from(rest.next().ok_or("host-setup : --source sans adresse")?);
            }
            flag if flag.starts_with('-') => {
                return Err(format!("host-setup : option inconnue {flag}"));
            }
            _ => dirs.push(arg.clone()),
        }
    }
    options.dir = one_dir(&dirs)?;
    Ok(options)
}

/// An optional single folder, the current one by default.
fn one_dir(args: &[String]) -> Result<PathBuf, String> {
    match args {
        [] => Ok(PathBuf::from(".")),
        [dir] if !dir.starts_with('-') => Ok(PathBuf::from(dir)),
        _ => Err(format!(
            "un seul dossier attendu, reçu : {}",
            args.join(" ")
        )),
    }
}

#[cfg(test)]
mod testdir {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    /// A new, empty folder for one test, in `target-essais/tests` (ignored by git).
    pub fn fresh(name: &str) -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/tests")
            .join(format!(
                "soiree-test-{}-{name}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        // Nothing there yet, most often.
        let _: std::io::Result<()> = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }
}
