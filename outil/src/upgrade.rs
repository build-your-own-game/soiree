//! Upgrade to this installed binary's version; preserve comments and hardening.
use crate::embedded::{self, VERSION, rules};
use crate::util::{self, Result};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use toml_edit::{DocumentMut, Item};

struct Plan {
    files: Vec<(PathBuf, String)>,
    repositories: BTreeSet<String>,
    packages: BTreeSet<String>,
}

pub fn run(args: &[String]) -> Result<ExitCode> {
    let mut dir = PathBuf::from(".");
    let mut dry_run = false;
    let mut provided = false;
    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            flag if flag.starts_with('-') => {
                return Err(format!("upgrade : option inconnue {flag}"));
            }
            path if !provided => {
                dir = PathBuf::from(path);
                provided = true;
            }
            _ => return Err("upgrade : un seul dossier attendu".into()),
        }
    }
    let dir = util::absolute(&dir);
    let plan = plan(&dir)?;
    println!("Passage à soiree {VERSION}, la version de ce binaire :");
    for (path, _) in &plan.files {
        println!("  {}", util::shown(path, &dir));
    }
    println!("  .soiree/ ; cargo update des seules dépendances communes ; soiree check");
    if dry_run {
        return Ok(ExitCode::SUCCESS);
    }
    for repository in &plan.repositories {
        let tag = format!("refs/tags/v{VERSION}");
        if !util::run(
            &dir,
            "git",
            &["ls-remote", "--exit-code", "--tags", repository, &tag],
        )?
        .status
        .success()
        {
            return Err(format!(
                "upgrade : {repository} n'a pas publié v{VERSION} ; aucun fichier modifié"
            ));
        }
    }
    for (path, contents) in &plan.files {
        util::write(path, contents)?;
    }
    embedded::write_docs(&dir)?;
    if !plan.packages.is_empty() {
        let mut args = vec!["update".to_owned()];
        for package in plan.packages {
            args.extend(["--package".into(), package]);
        }
        let refs: Vec<_> = args.iter().map(String::as_str).collect();
        let output = util::run(&dir, "cargo", &refs)?;
        if !output.status.success() {
            return Err(format!(
                "cargo update : {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }
    println!(
        "Consulte « Passer à cette version » dans .soiree/CHANGELOG.md ; adapte le code signalé par le contrôle."
    );
    Ok(crate::check::run(&[dir], crate::check::Options::default()))
}

fn document(path: &Path) -> Result<DocumentMut> {
    util::read(path)?
        .parse()
        .map_err(|error| format!("{} : {error}", path.display()))
}

fn plan(dir: &Path) -> Result<Plan> {
    let mut version = document(&dir.join("soiree.toml"))?;
    let current = version
        .get("version")
        .and_then(Item::as_str)
        .ok_or("soiree.toml : version absente")?;
    let current =
        semver::Version::parse(current).map_err(|error| format!("soiree.toml : {error}"))?;
    let target = semver::Version::parse(VERSION).map_err(|error| error.to_string())?;
    if current > target {
        return Err(format!(
            "upgrade refuse de rétrograder {current} vers {target} ; installe le binaire de la version voulue"
        ));
    }
    version
        .as_table_mut()
        .insert("version", toml_edit::value(VERSION));
    let mut cargo = document(&dir.join("Cargo.toml"))?;
    let mut repositories = BTreeSet::new();
    let mut packages = BTreeSet::new();
    dependencies(cargo.as_item_mut(), &mut repositories, &mut packages);
    let mut members = Vec::new();
    if cargo.contains_key("workspace") {
        for path in workspace_manifests(dir)? {
            if path == dir.join("Cargo.toml") {
                continue;
            }
            let original = util::read(&path)?;
            let mut member: DocumentMut = original
                .parse()
                .map_err(|error| format!("{} : {error}", path.display()))?;
            dependencies(member.as_item_mut(), &mut repositories, &mut packages);
            let updated = member.to_string();
            if updated != original {
                members.push((path, updated));
            }
        }
    }
    let expected: DocumentMut = rules::CARGO
        .parse()
        .map_err(|e| format!("règles Cargo : {e}"))?;
    for key in ["package", "lints"] {
        let target: &mut dyn toml_edit::TableLike = if cargo.contains_key("workspace") {
            cargo
                .get_mut("workspace")
                .and_then(Item::as_table_like_mut)
                .ok_or("workspace doit être une table")?
        } else {
            cargo.as_table_mut()
        };
        merge_key(
            target,
            key,
            expected.get(key).ok_or("règles Cargo incomplètes")?,
        );
    }
    merge_key(
        cargo.as_table_mut(),
        "profile",
        expected.get("profile").ok_or("règles profile absentes")?,
    );
    let mut clippy = optional(dir, "clippy.toml")?;
    let expected: DocumentMut = rules::CLIPPY
        .parse()
        .map_err(|e| format!("règles Clippy : {e}"))?;
    merge(clippy.as_item_mut(), expected.as_item());
    let mut toolchain = optional(dir, "rust-toolchain.toml")?;
    let expected: DocumentMut = rules::TOOLCHAIN
        .parse()
        .map_err(|e| format!("toolchain : {e}"))?;
    merge(toolchain.as_item_mut(), expected.as_item());
    let workflow = dir.join(crate::ci::PATH);
    if workflow.exists() && !util::read(&workflow)?.starts_with("# @generated by soiree:") {
        return Err(format!(
            "{} est personnalisé : conserve-le sous un autre nom avant upgrade",
            workflow.display()
        ));
    }
    let mut files = vec![
        (dir.join("soiree.toml"), version.to_string()),
        (dir.join("Cargo.toml"), cargo.to_string()),
        (dir.join("clippy.toml"), clippy.to_string()),
        (dir.join("rust-toolchain.toml"), toolchain.to_string()),
        (workflow, crate::ci::workflow()?),
    ];
    files.extend(members);
    Ok(Plan {
        files,
        repositories,
        packages,
    })
}

/// Cargo expands workspace globs and inherited membership for us. `--no-deps`
/// does not resolve dependencies; `--offline --locked` also forbids fetching
/// or changing a lockfile during planning, including before its first lock.
fn workspace_manifests(dir: &Path) -> Result<BTreeSet<PathBuf>> {
    let output = util::run(
        dir,
        "cargo",
        &[
            "metadata",
            "--no-deps",
            "--offline",
            "--locked",
            "--format-version",
            "1",
        ],
    )?;
    if !output.status.success() {
        return Err(format!(
            "upgrade : découverte du workspace par cargo metadata --no-deps : {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("upgrade : métadonnées du workspace illisibles : {error}"))?;
    let members: BTreeSet<_> = metadata
        .get("workspace_members")
        .and_then(serde_json::Value::as_array)
        .ok_or("upgrade : workspace_members absent des métadonnées Cargo")?
        .iter()
        .map(serde_json::Value::as_str)
        .collect::<Option<_>>()
        .ok_or("upgrade : identifiant de membre illisible dans les métadonnées Cargo")?;
    let mut manifests = BTreeSet::new();
    for package in metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("upgrade : packages absent des métadonnées Cargo")?
        .iter()
        .filter(|package| {
            package
                .get("id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|id| members.contains(id))
        })
    {
        let manifest = package
            .get("manifest_path")
            .and_then(serde_json::Value::as_str)
            .ok_or("upgrade : manifest_path absent d'un membre du workspace")?;
        let path = std::fs::canonicalize(manifest)
            .map_err(|error| format!("upgrade : {manifest} : {error}"))?;
        if !path.starts_with(dir) {
            return Err(format!(
                "upgrade : membre du workspace hors du dossier du jeu : {} ; aucun fichier modifié",
                path.display()
            ));
        }
        manifests.insert(path);
    }
    if manifests.len() != members.len() {
        return Err("upgrade : manifestes des membres du workspace incomplets".into());
    }
    Ok(manifests)
}

fn optional(dir: &Path, name: &str) -> Result<DocumentMut> {
    if dir.join(name).exists() {
        document(&dir.join(name))
    } else {
        Ok(DocumentMut::new())
    }
}

fn merge_key(target: &mut dyn toml_edit::TableLike, key: &str, expected: &Item) {
    if let Some(actual) = target.get_mut(key) {
        merge(actual, expected);
    } else {
        target.insert(key, expected.clone());
    }
}

fn dependencies(
    item: &mut Item,
    repositories: &mut BTreeSet<String>,
    packages: &mut BTreeSet<String>,
) {
    let Some(table) = item.as_table_like_mut() else {
        return;
    };
    for (key, child) in table.iter_mut() {
        if key.ends_with("dependencies") {
            if let Some(deps) = child.as_table_like_mut() {
                for (alias, spec) in deps.iter_mut() {
                    let name = spec
                        .get("package")
                        .and_then(Item::as_str)
                        .unwrap_or(alias.get())
                        .to_owned();
                    if (name.starts_with("salon-") || name.starts_with("remote-"))
                        && let Some(spec) = spec.as_table_like_mut()
                        && let Some(repository) =
                            spec.get("git").and_then(Item::as_str).map(str::to_owned)
                    {
                        repositories.insert(repository);
                        packages.insert(name);
                        spec.remove("rev");
                        spec.remove("branch");
                        spec.insert("tag", toml_edit::value(format!("v{VERSION}")));
                    }
                }
            }
        } else {
            dependencies(child, repositories, packages);
        }
    }
}

fn merge(actual: &mut Item, expected: &Item) {
    if let Some(table) = expected.as_table_like() {
        if actual.is_none() {
            *actual = expected.clone();
            return;
        }
        if let Some(target) = actual.as_table_like_mut() {
            for (key, value) in table.iter() {
                if let Some(existing) = target.get_mut(key) {
                    if crate::lints::stricter_permission(key, existing.as_bool(), value.as_bool()) {
                        continue;
                    }
                    merge(existing, value);
                } else {
                    target.insert(key, value.clone());
                }
            }
            return;
        }
    }
    if let (Some(target), Some(wanted)) = (actual.as_array_mut(), expected.as_array()) {
        for value in wanted {
            if !target.iter().any(|v| {
                identity(v)
                    .zip(identity(value))
                    .is_some_and(|(a, b)| a == b)
                    || v.to_string() == value.to_string()
            }) {
                target.push(value.clone());
            }
        }
        return;
    }
    if level(actual)
        .zip(level(expected))
        .is_some_and(|(a, b)| a >= b)
    {
        return;
    }
    *actual = expected.clone();
}
fn identity(value: &toml_edit::Value) -> Option<&str> {
    value
        .as_inline_table()
        .and_then(|table| table.get("path"))
        .and_then(toml_edit::Value::as_str)
        .or_else(|| value.as_str())
}
fn level(item: &Item) -> Option<usize> {
    item.as_str()
        .or_else(|| item.get("level").and_then(Item::as_str))
        .and_then(|s| {
            ["allow", "warn", "deny", "forbid"]
                .iter()
                .position(|v| *v == s)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upgrade_preserves_comments_and_stricter_local_rules() {
        let dir = crate::testdir::fresh("upgrade");
        util::write(
            &dir.join("soiree.toml"),
            "# author\nversion = \"0.1.0\"\nname = \"duck\"\n",
        )
        .unwrap();
        util::write(&dir.join("Cargo.toml"), "# game rules\n[package]\nname = \"duck\"\nversion = \"0.1.0\"\n[lints.rust]\nunsafe_code = \"forbid\"\n[dependencies]\nsalon-web = { git = \"https://github.com/build-your-own-game/soiree\", tag = \"v0.1.0\" }\n").unwrap();
        util::write(
            &dir.join("clippy.toml"),
            "# stricter\nallow-unwrap-in-tests = false\ndisallowed-types = [\"extra::Type\"]\n",
        )
        .unwrap();
        let plan = plan(&dir).unwrap();
        let cargo = &plan
            .files
            .iter()
            .find(|(p, _)| p.ends_with("Cargo.toml"))
            .unwrap()
            .1;
        assert!(cargo.contains("# game rules") && cargo.contains("unsafe_code = \"forbid\""));
        assert!(cargo.contains(&format!("v{VERSION}")));
        assert!(
            plan.files
                .iter()
                .find(|(p, _)| p.ends_with("clippy.toml"))
                .unwrap()
                .1
                .contains("extra::Type")
        );
        assert!(
            plan.files
                .iter()
                .find(|(p, _)| p.ends_with("clippy.toml"))
                .unwrap()
                .1
                .contains("allow-unwrap-in-tests = false")
        );
        assert!(
            util::read(&dir.join("Cargo.toml"))
                .unwrap()
                .contains("v0.1.0"),
            "planning mutated sources"
        );
    }

    #[test]
    fn upgrade_refuses_a_downgrade_before_planning_other_files() {
        let dir = crate::testdir::fresh("downgrade");
        util::write(&dir.join("soiree.toml"), "version = \"999.0.0\"\n").unwrap();
        assert!(plan(&dir).is_err());
        assert_eq!(
            util::read(&dir.join("soiree.toml")).unwrap(),
            "version = \"999.0.0\"\n"
        );
    }
}
