//! Les règles d'écriture d'un jeu, vérifiées sur ses sources (CONTRAT, « Les règles
//! d'écriture » ; RESTE-PHASE-1, point 12) : ce que l'étape 1 juge avant de compiler.
//!
//! Chaque fonction est pure : elle lit un texte (un manifeste, un `clippy.toml`, un
//! fichier Rust) et rend ce qui manque ou ce qui assouplit, ligne par ligne. Les
//! références sont celles du commun, embarquées à la compilation de l'épreuve : les
//! `[workspace.lints]` et les profils du `Cargo.toml` de `soiree`, et son
//! `clippy.toml`. Un jeu peut les durcir, jamais les assouplir.

use toml::{Table, Value};

/// Le `Cargo.toml` du commun : ses `[workspace.lints]` font la référence.
pub(crate) const CARGO_DU_COMMUN: &str = include_str!("../../../Cargo.toml");
/// Le `clippy.toml` du commun.
pub(crate) const CLIPPY_DU_COMMUN: &str = include_str!("../../../clippy.toml");

/// Un niveau de lint, du plus souple au plus strict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Niveau {
    Allow,
    Warn,
    Deny,
    Forbid,
}

impl Niveau {
    fn lire(v: &Value) -> Option<Niveau> {
        let texte = match v {
            Value::String(s) => s.as_str(),
            Value::Table(t) => t.get("level")?.as_str()?,
            Value::Integer(_)
            | Value::Float(_)
            | Value::Boolean(_)
            | Value::Datetime(_)
            | Value::Array(_) => return None,
        };
        match texte {
            "allow" => Some(Niveau::Allow),
            "warn" => Some(Niveau::Warn),
            "deny" => Some(Niveau::Deny),
            "forbid" => Some(Niveau::Forbid),
            _ => None,
        }
    }

    const fn nom(self) -> &'static str {
        match self {
            Niveau::Allow => "allow",
            Niveau::Warn => "warn",
            Niveau::Deny => "deny",
            Niveau::Forbid => "forbid",
        }
    }
}

/// Lit un texte TOML en table.
pub(crate) fn table(texte: &str) -> Result<Table, String> {
    texte.parse::<Table>().map_err(|e| e.to_string())
}

/// La table à ce chemin de clés, s'il y en a une.
pub(crate) fn sous<'t>(t: &'t Table, chemin: &[&str]) -> Option<&'t Table> {
    let mut ici = t;
    for cle in chemin {
        ici = ici.get(*cle)?.as_table()?;
    }
    Some(ici)
}

/// Les `[workspace.lints]` du commun.
pub(crate) fn lints_du_commun() -> Result<Table, String> {
    let t = table(CARGO_DU_COMMUN)?;
    sous(&t, &["workspace", "lints"])
        .cloned()
        .ok_or_else(|| String::from("le Cargo.toml du commun n'a pas de [workspace.lints]"))
}

/// Les lints que reçoit un paquet : `[lints]` de son manifeste, ou ceux de son
/// workspace s'il écrit `[lints] workspace = true`. Rend aussi d'où ils viennent.
pub(crate) fn lints_du_paquet(paquet: &Table, racine: &Table) -> (Table, &'static str) {
    match paquet.get("lints") {
        Some(Value::Table(l)) if l.get("workspace").and_then(Value::as_bool) == Some(true) => (
            sous(racine, &["workspace", "lints"])
                .cloned()
                .unwrap_or_default(),
            "[workspace.lints]",
        ),
        Some(Value::Table(l)) => (l.clone(), "[lints] du paquet"),
        Some(_) | None => (Table::new(), "aucun [lints]"),
    }
}

/// Ce qui manque ou assouplit dans les lints d'un jeu, au regard de ceux du commun.
/// Chaque lint du commun doit y être, au même niveau ou plus strict ; un lint que le
/// commun ne nomme pas ne peut pas y être mis à `allow` : il pourrait assouplir un
/// groupe (`pedantic`), et une exception s'écrit `#[expect]`, à l'endroit précis.
pub(crate) fn ecarts_de_lints(reference: &Table, jeu: &Table) -> Vec<String> {
    let mut ecarts = Vec::new();
    for (outil, lints) in reference {
        let Some(lints) = lints.as_table() else {
            continue;
        };
        let du_jeu = jeu.get(outil).and_then(Value::as_table);
        for (lint, v) in lints {
            let Some(attendu) = Niveau::lire(v) else {
                continue;
            };
            match du_jeu.and_then(|t| t.get(lint)).and_then(Niveau::lire) {
                None => ecarts.push(format!(
                    "{outil}::{lint} manque (le commun : {})",
                    attendu.nom()
                )),
                Some(n) if n < attendu => ecarts.push(format!(
                    "{outil}::{lint} assoupli : {} au lieu de {}",
                    n.nom(),
                    attendu.nom()
                )),
                Some(_) => {}
            }
        }
    }
    for (outil, lints) in jeu {
        let Some(lints) = lints.as_table() else {
            continue;
        };
        let du_commun = reference.get(outil).and_then(Value::as_table);
        for (lint, v) in lints {
            if du_commun.is_some_and(|t| t.contains_key(lint)) {
                continue;
            }
            if Niveau::lire(v) == Some(Niveau::Allow) {
                ecarts.push(format!(
                    "{outil}::{lint} mis à allow, que le commun ne nomme pas : écrire #[expect(…, reason = \"…\")] à l'endroit précis"
                ));
            }
        }
    }
    ecarts
}

/// Ce qui manque aux profils d'un workspace de jeu (CONTRAT : « le dépassement est
/// vérifié même en release » ; « les dépendances sont optimisées même en debug » ;
/// INTERFACE, § 3.1 : le profil `module`, `panic = "abort"`).
pub(crate) fn ecarts_de_profils(racine: &Table) -> Vec<String> {
    let mut ecarts = Vec::new();
    let vrai = |t: Option<&Table>, cle: &str| t.and_then(|t| t.get(cle)).and_then(Value::as_bool);
    let release = sous(racine, &["profile", "release"]);
    if vrai(release, "overflow-checks") != Some(true) {
        ecarts.push(String::from(
            "[profile.release] n'écrit pas overflow-checks = true",
        ));
    }
    match sous(racine, &["profile", "module"]) {
        None => ecarts.push(String::from("pas de [profile.module]")),
        Some(m) => {
            if m.get("inherits").and_then(Value::as_str) != Some("release") {
                ecarts.push(String::from(
                    "[profile.module] n'écrit pas inherits = \"release\"",
                ));
            }
            if m.get("panic").and_then(Value::as_str) != Some("abort") {
                ecarts.push(String::from(
                    "[profile.module] n'écrit pas panic = \"abort\"",
                ));
            }
            match vrai(Some(m), "overflow-checks") {
                Some(true) => {}
                Some(false) => ecarts.push(String::from(
                    "[profile.module] écrit overflow-checks = false",
                )),
                None if vrai(release, "overflow-checks") == Some(true) => {}
                None => ecarts.push(String::from(
                    "[profile.module] n'a pas overflow-checks = true",
                )),
            }
        }
    }
    // Une exception par paquet qui couperait le contrôle de débordement.
    if let Some(profils) = sous(racine, &["profile"]) {
        for (nom, p) in profils {
            let Some(paquets) = p
                .as_table()
                .and_then(|p| p.get("package"))
                .and_then(Value::as_table)
            else {
                continue;
            };
            for (paquet, reglage) in paquets {
                if reglage
                    .as_table()
                    .and_then(|t| t.get("overflow-checks"))
                    .and_then(Value::as_bool)
                    == Some(false)
                {
                    ecarts.push(format!(
                        "[profile.{nom}.package.{paquet}] écrit overflow-checks = false"
                    ));
                }
            }
        }
    }
    let optimise = sous(racine, &["profile", "dev", "package", "*"])
        .and_then(|t| t.get("opt-level"))
        .is_some_and(|v| match v {
            Value::Integer(n) => *n > 0,
            Value::String(s) => s == "s" || s == "z",
            Value::Float(_)
            | Value::Boolean(_)
            | Value::Datetime(_)
            | Value::Array(_)
            | Value::Table(_) => false,
        });
    if !optimise {
        ecarts.push(String::from(
            "[profile.dev.package.\"*\"] n'optimise pas les dépendances (opt-level)",
        ));
    }
    ecarts
}

/// Ce qu'une configuration de Cargo propre au jeu (`.cargo/config.toml` dans son
/// dépôt) pourrait changer à la compilation sans que le manifeste le dise : des
/// options du compilateur, des profils.
pub(crate) fn ecarts_de_configuration(config: &Table) -> Vec<String> {
    let mut ecarts = Vec::new();
    if sous(config, &["build"]).is_some_and(|b| b.contains_key("rustflags")) {
        ecarts.push(String::from("[build] rustflags"));
    }
    if let Some(cibles) = sous(config, &["target"]) {
        for (cible, t) in cibles {
            if t.as_table().is_some_and(|t| t.contains_key("rustflags")) {
                ecarts.push(format!("[target.{cible}] rustflags"));
            }
        }
    }
    if config.contains_key("profile") {
        ecarts.push(String::from("[profile]"));
    }
    ecarts
}

/// Les chemins d'une liste de `disallowed-*` : chaque entrée est un texte, ou une table
/// avec `path`.
fn chemins(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|l| {
            l.iter()
                .filter_map(|e| match e {
                    Value::String(s) => Some(s.clone()),
                    Value::Table(t) => t.get("path").and_then(Value::as_str).map(String::from),
                    Value::Integer(_)
                    | Value::Float(_)
                    | Value::Boolean(_)
                    | Value::Datetime(_)
                    | Value::Array(_) => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Ce qui manque ou assouplit dans le `clippy.toml` d'un jeu, au regard de celui du
/// commun. Chaque clé du commun doit y être : une liste d'interdits (`disallowed-*`)
/// au moins aussi longue, chemin pour chemin ; un `allow-*` à `true` seulement si le
/// commun l'y met ; `max-fn-params-bools` au plus celui du commun ; toute autre valeur,
/// la même. Une clé que le commun ne pose pas n'est permise que si elle interdit
/// (`disallowed-*`) : l'épreuve ne sait pas dire si une autre durcit ou assouplit.
pub(crate) fn ecarts_de_clippy(reference: &Table, jeu: &Table) -> Vec<String> {
    let mut ecarts = Vec::new();
    for (cle, attendu) in reference {
        let Some(v) = jeu.get(cle) else {
            ecarts.push(format!("{cle} manque"));
            continue;
        };
        if cle.starts_with("disallowed-") {
            let du_jeu = chemins(v);
            let manquent: Vec<String> = chemins(attendu)
                .into_iter()
                .filter(|c| !du_jeu.contains(c))
                .collect();
            if !manquent.is_empty() {
                ecarts.push(format!("{cle} n'interdit pas {}", manquent.join(", ")));
            }
            continue;
        }
        let tient = match (attendu, v) {
            (Value::Boolean(a), Value::Boolean(b)) if cle.starts_with("allow-") => *b <= *a,
            (Value::Integer(a), Value::Integer(b)) if cle == "max-fn-params-bools" => b <= a,
            (a, b) => a == b,
        };
        if !tient {
            ecarts.push(format!("{cle} vaut {v} au lieu de {attendu}"));
        }
    }
    for cle in jeu.keys() {
        if !reference.contains_key(cle) && !cle.starts_with("disallowed-") {
            ecarts.push(format!(
                "{cle} : une clé que le commun ne pose pas, dont l'épreuve ne sait pas si elle durcit ou assouplit"
            ));
        }
    }
    ecarts
}

/// Le texte Rust sans ses commentaires ni le contenu de ses textes et caractères :
/// remplacés par des espaces, les fins de ligne gardées. Les attributs y restent
/// lisibles, et un `allow` écrit dans un commentaire ou un texte n'y est plus.
#[expect(
    clippy::too_many_lines,
    reason = "un seul passage sur le texte, un bras par sorte de jeton : commentaires, textes bruts, textes, caractères"
)]
pub(crate) fn code_seul(source: &str) -> String {
    let c: Vec<char> = source.chars().collect();
    let mut o = String::with_capacity(source.len());
    let blanc = |o: &mut String, x: char| o.push(if x == '\n' { '\n' } else { ' ' });
    let mut i = 0usize;
    let at = |k: usize| c.get(k).copied();
    while let Some(x) = at(i) {
        let suivant = at(i.saturating_add(1));
        if x == '/' && suivant == Some('/') {
            while let Some(y) = at(i) {
                if y == '\n' {
                    break;
                }
                o.push(' ');
                i = i.saturating_add(1);
            }
            continue;
        }
        if x == '/' && suivant == Some('*') {
            let mut profondeur = 0u32;
            while let Some(y) = at(i) {
                let z = at(i.saturating_add(1));
                if y == '/' && z == Some('*') {
                    profondeur = profondeur.saturating_add(1);
                    o.push_str("  ");
                    i = i.saturating_add(2);
                    continue;
                }
                if y == '*' && z == Some('/') {
                    profondeur = profondeur.saturating_sub(1);
                    o.push_str("  ");
                    i = i.saturating_add(2);
                    if profondeur == 0 {
                        break;
                    }
                    continue;
                }
                blanc(&mut o, y);
                i = i.saturating_add(1);
            }
            continue;
        }
        // Un texte brut : r"…", r#"…"#, br#"…"#.
        let brut = x == 'r' && matches!(suivant, Some('"' | '#'))
            || x == 'b'
                && suivant == Some('r')
                && matches!(at(i.saturating_add(2)), Some('"' | '#'));
        let precede_ident =
            i > 0 && at(i.saturating_sub(1)).is_some_and(|p| p.is_alphanumeric() || p == '_');
        if brut && !precede_ident {
            let mut k = if x == 'b' {
                i.saturating_add(2)
            } else {
                i.saturating_add(1)
            };
            let mut dieses = 0usize;
            while at(k) == Some('#') {
                dieses = dieses.saturating_add(1);
                k = k.saturating_add(1);
            }
            if at(k) == Some('"') {
                for _ in i..=k {
                    o.push(' ');
                }
                i = k.saturating_add(1);
                while let Some(y) = at(i) {
                    if y == '"' && (1..=dieses).all(|d| at(i.saturating_add(d)) == Some('#')) {
                        for _ in 0..=dieses {
                            o.push(' ');
                        }
                        i = i.saturating_add(dieses).saturating_add(1);
                        break;
                    }
                    blanc(&mut o, y);
                    i = i.saturating_add(1);
                }
                continue;
            }
        }
        if x == '"' {
            o.push('"');
            i = i.saturating_add(1);
            while let Some(y) = at(i) {
                if y == '\\' {
                    o.push(' ');
                    i = i.saturating_add(1);
                    if let Some(z) = at(i) {
                        blanc(&mut o, z);
                    }
                    i = i.saturating_add(1);
                    continue;
                }
                if y == '"' {
                    o.push('"');
                    i = i.saturating_add(1);
                    break;
                }
                blanc(&mut o, y);
                i = i.saturating_add(1);
            }
            continue;
        }
        if x == '\'' {
            // Un caractère ('a', '\n', '\u{…}') ; sinon une durée de vie ('a).
            let fin = if suivant == Some('\\') {
                (i.saturating_add(2)..c.len()).find(|&k| at(k) == Some('\''))
            } else if at(i.saturating_add(2)) == Some('\'') {
                Some(i.saturating_add(2))
            } else {
                None
            };
            if let Some(f) = fin {
                for _ in i..=f {
                    o.push(' ');
                }
                i = f.saturating_add(1);
                continue;
            }
        }
        o.push(x);
        i = i.saturating_add(1);
    }
    o
}

/// Un attribut du code : interne (`#![…]`) ou non, son contenu entre crochets, et sa
/// ligne (à partir de 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Attribut {
    pub(crate) interne: bool,
    pub(crate) contenu: String,
    pub(crate) ligne: usize,
}

/// Les attributs d'un texte passé par [`code_seul`].
pub(crate) fn attributs(code: &str) -> Vec<Attribut> {
    let c: Vec<char> = code.chars().collect();
    let mut tous = Vec::new();
    let mut ligne = 1usize;
    let mut i = 0usize;
    while let Some(&x) = c.get(i) {
        if x == '\n' {
            ligne = ligne.saturating_add(1);
        }
        if x != '#' {
            i = i.saturating_add(1);
            continue;
        }
        let mut k = i.saturating_add(1);
        let saute = |k: &mut usize| {
            while c.get(*k).is_some_and(|y| y.is_whitespace() && *y != '\n') {
                *k = k.saturating_add(1);
            }
        };
        saute(&mut k);
        let interne = c.get(k) == Some(&'!');
        if interne {
            k = k.saturating_add(1);
            saute(&mut k);
        }
        if c.get(k) != Some(&'[') {
            i = i.saturating_add(1);
            continue;
        }
        let debut = k.saturating_add(1);
        let mut profondeur = 1u32;
        let mut j = debut;
        let mut lignes = 0usize;
        while let Some(&y) = c.get(j) {
            match y {
                '[' | '(' | '{' => profondeur = profondeur.saturating_add(1),
                ']' | ')' | '}' => profondeur = profondeur.saturating_sub(1),
                '\n' => lignes = lignes.saturating_add(1),
                _ => {}
            }
            if profondeur == 0 {
                break;
            }
            j = j.saturating_add(1);
        }
        let contenu: String = c.get(debut..j).unwrap_or(&[]).iter().collect();
        tous.push(Attribut {
            interne,
            contenu,
            ligne,
        });
        ligne = ligne.saturating_add(lignes);
        i = j.saturating_add(1);
    }
    tous
}

/// Les mots d'un attribut : identifiants et ponctuation, sans les espaces.
fn mots(contenu: &str) -> Vec<String> {
    let mut m = Vec::new();
    let mut mot = String::new();
    for x in contenu.chars() {
        if x.is_alphanumeric() || x == '_' {
            mot.push(x);
            continue;
        }
        if !mot.is_empty() {
            m.push(std::mem::take(&mut mot));
        }
        if !x.is_whitespace() {
            m.push(String::from(x));
        }
    }
    if !mot.is_empty() {
        m.push(mot);
    }
    m
}

/// Où est un fichier Rust, pour [`assouplissements`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lieu {
    /// Le code du jeu : `src/`, `build.rs`, les exemples.
    Code,
    /// Un fichier de `tests/`.
    Tests,
}

/// Ce qui assouplit une règle sans le dire dans un fichier Rust : un `allow` dans le
/// code (CONTRAT : « `#[expect(lint, reason = "...")]`, jamais `#[allow]` »), un `allow`
/// sans raison dans les tests, ou un `expect` sans raison. Dans `tests/`, un `allow`
/// qui dit sa raison est permis : un module commun à plusieurs fichiers de test ne peut
/// pas écrire `expect(dead_code)`, que l'un remplit et l'autre non (Sidequest,
/// `tests/commun/mod.rs`). Rend la ligne et l'attribut.
pub(crate) fn assouplissements(source: &str, lieu: Lieu) -> Vec<(usize, String)> {
    let code = code_seul(source);
    let mut trouves = Vec::new();
    for a in attributs(&code) {
        let m = mots(&a.contenu);
        let appel = |nom: &str| {
            m.windows(2)
                .any(|w| matches!(w, [x, p] if x == nom && p == "("))
        };
        let premier = m.first().map(String::as_str);
        let dans_cfg = premier == Some("cfg_attr");
        let allow = premier == Some("allow") || (dans_cfg && appel("allow"));
        let expect = premier == Some("expect") || (dans_cfg && appel("expect"));
        let raison = m.iter().any(|x| x == "reason");
        let texte = format!(
            "#{}[{}]",
            if a.interne { "!" } else { "" },
            a.contenu.split_whitespace().collect::<Vec<_>>().join(" ")
        );
        if allow && lieu == Lieu::Tests && raison {
            continue;
        }
        if allow {
            trouves.push((
                a.ligne,
                format!("{texte} : un allow (écrire #[expect(…, reason = \"…\")])"),
            ));
        } else if expect && !raison {
            trouves.push((a.ligne, format!("{texte} : un expect sans raison")));
        }
    }
    trouves
}

/// La racine d'une crate de règles a-t-elle cet attribut interne, à ce mot près
/// (`forbid(unsafe_code)`, `no_std`) ? Un `cfg_attr` qui le pose sous condition ne
/// compte pas pour `forbid`.
pub(crate) fn a_l_attribut(source: &str, attendu: Attendu) -> bool {
    let code = code_seul(source);
    attributs(&code).into_iter().filter(|a| a.interne).any(|a| {
        let m = mots(&a.contenu);
        match attendu {
            Attendu::InterditUnsafe => {
                m.first().map(String::as_str) == Some("forbid")
                    && m.iter().any(|x| x == "unsafe_code")
            }
            Attendu::SansStd => m.iter().any(|x| x == "no_std"),
        }
    })
}

/// L'attribut qu'une racine de crate de règles doit porter (INTERFACE, § 2.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Attendu {
    /// `#![forbid(unsafe_code)]`.
    InterditUnsafe,
    /// `#![no_std]`, ou sous `cfg_attr`.
    SansStd,
}

/// La forme d'une crate de module (INTERFACE, § 2.1 et § 2.14) : au plus un attribut
/// interne qui pose `no_std`, puis `jeu::module!(<chemin du jeu>);`, et rien d'autre.
/// Rend le chemin du type du jeu, tel qu'il est écrit, ou ce qui ne va pas.
pub(crate) fn forme_du_module(source: &str) -> Result<String, String> {
    let code = code_seul(source);
    let mut reste = code.clone();
    for a in attributs(&code) {
        if !a.interne {
            return Err(format!(
                "ligne {} : un attribut #[{}] ; une crate de module n'a que `jeu::module!`",
                a.ligne,
                a.contenu.trim()
            ));
        }
        let m = mots(&a.contenu);
        let sans_std = m == ["no_std"]
            || (m.first().map(String::as_str) == Some("cfg_attr")
                && m.last().map(String::as_str) == Some(")")
                && m.iter().rev().nth(1).map(String::as_str) == Some("no_std"));
        if !sans_std {
            return Err(format!(
                "ligne {} : l'attribut #![{}] ; seuls `no_std` et `cfg_attr(…, no_std)` sont permis",
                a.ligne,
                a.contenu.trim()
            ));
        }
        let texte = format!("[{}]", a.contenu);
        if let Some(p) = reste.find(&texte) {
            let avant = reste.get(..p).unwrap_or("");
            let pos = avant.rfind('#').unwrap_or(p);
            reste.replace_range(pos..p.saturating_add(texte.len()), "");
        }
    }
    let serre: String = reste.chars().filter(|x| !x.is_whitespace()).collect();
    let corps = serre
        .strip_prefix("::")
        .unwrap_or(&serre)
        .strip_prefix("jeu::module!(")
        .and_then(|r| r.strip_suffix(");"))
        .ok_or_else(|| {
            String::from("la crate de module n'est pas `jeu::module!(<chemin du jeu>);` seul")
        })?;
    let premier = corps.split("::").next().unwrap_or("");
    let ident = |s: &str| {
        s.chars()
            .next()
            .is_some_and(|x| x.is_alphabetic() || x == '_')
            && s.chars().all(|x| x.is_alphanumeric() || x == '_')
    };
    if !ident(premier) || corps.contains('(') || corps.contains(';') || corps.contains('{') {
        return Err(format!(
            "`jeu::module!({corps})` ne nomme pas un type par son chemin"
        ));
    }
    // Le chemin tel qu'écrit, aux espaces près.
    Ok(corps.to_owned())
}

/// La crate que nomme le premier segment du chemin du jeu.
pub(crate) fn crate_du_chemin(chemin: &str) -> &str {
    chemin.split("::").next().unwrap_or(chemin)
}

/// Ce qui garde une valeur hors de l'état, dans un fichier du code d'un jeu : un
/// `static` (atomique, `Cell`, `OnceLock`, `static mut`…) ou un `thread_local!`.
/// INTERFACE, § 2.2 : « un `static` à mutabilité intérieure dans la crate du jeu est
/// refusé ». Tout `static` l'est ici, même sans mutabilité intérieure : un `const`
/// dit la même chose sans rien garder, et l'épreuve ne sait pas lire, à travers un
/// alias de type, si un `static` en a une. Rend la ligne et ce qui est trouvé.
///
/// Pourquoi une forme : un `static` passe d'une instance du module à l'autre avec sa
/// mémoire, sans être dans l'état ; une vue peut y lire un secret recopié pendant une
/// entrée, un refus y laisser une trace qui ne se montre que bien plus tard (revue de
/// la phase 2, constats 1 et 5). Aucune pression ne voit tout ce qu'un `static` peut
/// faire ; sa forme se voit dans le texte.
pub(crate) fn hors_de_l_etat(source: &str) -> Vec<(usize, String)> {
    let j = jetons(&code_seul(source));
    let mut trouves = Vec::new();
    for (k, (ligne, mot)) in j.iter().enumerate() {
        let suivant = j.get(k.saturating_add(1)).map(|(_, m)| m.as_str());
        if mot == "static" {
            trouves.push((
                *ligne,
                String::from("un `static` : une valeur gardée hors de l'état (écrire un `const`)"),
            ));
        } else if mot == "thread_local" && suivant == Some("!") {
            trouves.push((
                *ligne,
                String::from("un `thread_local!` : une valeur gardée hors de l'état"),
            ));
        }
    }
    trouves
}

/// Les mots d'un texte passé par [`code_seul`], avec leur ligne : identifiants,
/// durées de vie (`'a`, d'un seul mot) et signes, un par un.
fn jetons(code: &str) -> Vec<(usize, String)> {
    let mut j = Vec::new();
    let mut ligne = 1usize;
    let mut mot = String::new();
    let mut debut = 1usize;
    let fin_de_mot = |j: &mut Vec<(usize, String)>, mot: &mut String, debut: usize| {
        if !mot.is_empty() {
            j.push((debut, std::mem::take(mot)));
        }
    };
    for x in code.chars() {
        let de_mot = x.is_alphanumeric() || x == '_';
        if de_mot && !mot.is_empty() {
            mot.push(x);
            continue;
        }
        if de_mot && mot.is_empty() {
            debut = ligne;
            mot.push(x);
            continue;
        }
        if x == '\'' {
            fin_de_mot(&mut j, &mut mot, debut);
            // Une durée de vie : l'apostrophe et le nom qui la suit font un mot.
            debut = ligne;
            mot.push(x);
            continue;
        }
        fin_de_mot(&mut j, &mut mot, debut);
        if x == '\n' {
            ligne = ligne.saturating_add(1);
        } else if !x.is_whitespace() {
            j.push((ligne, String::from(x)));
        }
    }
    fin_de_mot(&mut j, &mut mot, debut);
    j
}

/// Les traits que seules les macros de `jeu` implémentent (D24).
const TRAITS_SCELLES: [&str; 3] = ["Forme", "Etat", "Message"];

/// Ce qui écrit la forme d'un état à la main, dans un fichier du code d'un jeu : un
/// `impl` de `Forme`, `Etat` ou `Message` (seules `#[jeu::etat]` et `#[jeu::message]`
/// les écrivent, D24), un nom qui les cache (`Etat as E`), ou l'écrivain et le liseur
/// de la forme canonique (`jeu::cache`, `Ecrivain`, `Liseur`). CONTRAT : « la
/// projection se calcule, elle ne s'écrit pas à la main » ; un `projeter` écrit à la
/// main peut ne rien vider, et E8 ne cherche que ce que la projection vide (revue de
/// la phase 2, constat 4). Rend la ligne et ce qui est trouvé.
pub(crate) fn formes_a_la_main(source: &str) -> Vec<(usize, String)> {
    let j = jetons(&code_seul(source));
    let mot = |k: usize| j.get(k).map_or("", |(_, m)| m.as_str());
    let mut trouves = Vec::new();
    for (k, (ligne, m)) in j.iter().enumerate() {
        let suivant = mot(k.saturating_add(1));
        if TRAITS_SCELLES.contains(&m.as_str())
            && suivant == "as"
            && mot(k.saturating_add(2)) != "_"
        {
            trouves.push((
                *ligne,
                format!("`{m} as …` : un nom qui cache un trait de `jeu`"),
            ));
        } else if m == "Ecrivain" || m == "Liseur" {
            trouves.push((
                *ligne,
                format!(
                    "`{m}` : la forme canonique s'écrit par `#[jeu::etat]` ou `#[jeu::message]`"
                ),
            ));
        } else if m == "cache"
            && k >= 3
            && mot(k.saturating_sub(1)) == ":"
            && mot(k.saturating_sub(2)) == ":"
            && mot(k.saturating_sub(3)) == "jeu"
        {
            trouves.push((
                *ligne,
                String::from("`jeu::cache` : la forme canonique, scellée (D24)"),
            ));
        } else if m == "impl"
            && let Some(nom) = trait_de_l_impl(&j, k)
            && TRAITS_SCELLES.contains(&nom.as_str())
        {
            trouves.push((
                *ligne,
                format!("un `impl {nom}` écrit à la main : seules `#[jeu::etat]` et `#[jeu::message]` l'écrivent"),
            ));
        }
    }
    trouves
}

/// Le trait d'un `impl` qui commence au mot `debut` : le dernier nom avant `for`,
/// hors des chevrons ; `None` pour un `impl` sans trait. Un trait nommé par une
/// variable de macro (`impl $t for`) rend son nom avec le `$`.
fn trait_de_l_impl(j: &[(usize, String)], debut: usize) -> Option<String> {
    let mut chevrons = 0u32;
    let mut dernier: Option<String> = None;
    let mut precedent = "";
    for (_, m) in j.get(debut.saturating_add(1)..)? {
        match m.as_str() {
            "<" => chevrons = chevrons.saturating_add(1),
            ">" if precedent != "-" => chevrons = chevrons.saturating_sub(1),
            "{" | ";" | "where" if chevrons == 0 => return None,
            "for" if chevrons == 0 => return dernier,
            nom if chevrons == 0
                && nom
                    .chars()
                    .next()
                    .is_some_and(|x| x.is_alphabetic() || x == '_') =>
            {
                dernier = Some(if precedent == "$" {
                    format!("${nom}")
                } else {
                    String::from(nom)
                });
            }
            _ => {}
        }
        precedent = m.as_str();
    }
    None
}

/// Ce que le `build.rs` d'une crate du jeu a dit à Cargo, et qui changerait la
/// compilation sans que le manifeste le dise : toute directive autre que
/// `rerun-if-changed`, `rerun-if-env-changed`, `warning`, `error` et
/// `rustc-check-cfg`. Pour la crate de module (`module`), seule s'y ajoute
/// `rustc-link-arg-cdylib=--max-memory=<octets>`, le maximum de mémoire du module
/// (§ 3.1). `sortie` est le fichier `output` que Cargo garde de chaque exécution d'un
/// `build.rs` : ce qu'il a vraiment dit, et non ce que son texte semble dire (revue de
/// la phase 2, constat 6 ; la règle vaut celle de [`ecarts_de_configuration`]).
pub(crate) fn directives_de_build(sortie: &str, module: jeu_abi::Oui) -> Vec<String> {
    let mut trouves = Vec::new();
    for ligne in sortie.lines() {
        let Some(directive) = ligne
            .strip_prefix("cargo::")
            .or_else(|| ligne.strip_prefix("cargo:"))
        else {
            continue;
        };
        let (cle, valeur) = directive.split_once('=').unwrap_or((directive, ""));
        let sans_effet = matches!(
            cle,
            "rerun-if-changed" | "rerun-if-env-changed" | "warning" | "error" | "rustc-check-cfg"
        );
        let memoire = module == jeu_abi::Oui::Oui
            && cle == "rustc-link-arg-cdylib"
            && valeur
                .strip_prefix("--max-memory=")
                .is_some_and(|n| !n.is_empty() && n.chars().all(|x| x.is_ascii_digit()));
        if !sans_effet && !memoire {
            trouves.push(format!("`cargo::{cle}={valeur}`"));
        }
    }
    trouves
}

#[cfg(test)]
mod tests {
    use super::{
        Attendu, CLIPPY_DU_COMMUN, a_l_attribut, assouplissements, code_seul, directives_de_build,
        ecarts_de_clippy, ecarts_de_lints, ecarts_de_profils, forme_du_module, formes_a_la_main,
        hors_de_l_etat, lints_du_commun, table,
    };
    use jeu_abi::Oui;

    /// Revue de la phase 2, constats 1 et 5 (INTERFACE, § 2.2) : un `static` atomique
    /// où une entrée recopie un secret, qu'une vue lit sans regard, ou où un refus
    /// laisse une trace ; un `thread_local!`. Une durée de vie `'static`, un `static`
    /// dans un commentaire ou un texte, ne comptent pas.
    #[test]
    fn un_static_garde_hors_de_l_etat_se_voit() {
        let source = "use core::sync::atomic::AtomicU8;\n\
                      /// Un static dans un commentaire ne compte pas.\n\
                      static NOMBRE: AtomicU8 = AtomicU8::new(0);\n\
                      const T: &'static str = \"static X: u8 = 0;\";\n\
                      fn f(x: &'static str) -> &'static str { x }\n\
                      pub(crate) static mut M: u32 = 0;\n\
                      std::thread_local! { static C: core::cell::Cell<u8> = const { core::cell::Cell::new(0) }; }\n";
        let lignes: Vec<usize> = hors_de_l_etat(source).iter().map(|(l, _)| *l).collect();
        assert_eq!(lignes, [3, 6, 7, 7], "{:?}", hors_de_l_etat(source));
        assert_eq!(
            hors_de_l_etat("const A: [u8; 3] = [1, 2, 3];\nfn g<T: 'static>() {}\n").len(),
            0
        );
    }

    /// Revue de la phase 2, constat 4 (CONTRAT : « la projection se calcule, elle ne
    /// s'écrit pas à la main ») : un `impl Etat` ou `impl Forme` écrit à la main, un
    /// nom qui cache le trait, l'écrivain de la forme canonique. Un `impl` d'un autre
    /// trait pour un type nommé `Message`, un `use jeu::Etat as _`, passent.
    #[test]
    fn une_forme_ecrite_a_la_main_se_voit() {
        let source = "use jeu::cache::{Ecrivain, Liseur};\n\
                      impl jeu::Forme for Salle { const EMPREINTE: u64 = 0; }\n\
                      impl<T: Clone> Etat for Salle<T> where T: for<'a> Fn(&'a u8) {}\n\
                      impl core::fmt::Display for Message {}\n\
                      use jeu::Etat as _;\n\
                      use jeu::Message as M;\n\
                      impl ::jeu::Message for Vote {}\n";
        let lignes: Vec<usize> = formes_a_la_main(source).iter().map(|(l, _)| *l).collect();
        assert_eq!(
            lignes,
            [1, 1, 1, 2, 3, 6, 7],
            "{:?}",
            formes_a_la_main(source)
        );
        assert_eq!(
            formes_a_la_main(
                "#[jeu::etat]\npub struct S { a: u8 }\nimpl Jeu for S {}\nimpl Phase for P {}\n"
            )
            .len(),
            0
        );
    }

    /// Revue de la phase 2, constat 6 : un `build.rs` qui passe une option à l'éditeur
    /// de liens, un `cfg` ou une variable au compilateur ; le maximum de mémoire de la
    /// crate de module, et les directives sans effet sur la compilation, passent.
    #[test]
    fn un_build_rs_qui_change_la_compilation_se_voit() {
        let sain =
            "cargo::rerun-if-changed=build.rs\ncargo::rustc-link-arg-cdylib=--max-memory=4194304\n";
        assert_eq!(directives_de_build(sain, Oui::Oui).len(), 0);
        assert_eq!(directives_de_build(sain, Oui::Non).len(), 1);
        let fautif = "cargo:rerun-if-changed=src/question.rs\n\
                      cargo::warning=un avis\n\
                      cargo::rustc-link-arg-cdylib=--export=soiree_porte\n\
                      cargo::rustc-cfg=triche\n\
                      cargo:rustc-env=CLE=1\n\
                      cargo::rustc-link-arg=--max-memory=1\n\
                      une ligne qui n'est pas une directive\n";
        assert_eq!(
            directives_de_build(fautif, Oui::Oui).len(),
            4,
            "{:?}",
            directives_de_build(fautif, Oui::Oui)
        );
    }

    #[test]
    fn les_lints_du_commun_passent_les_lints_du_commun() {
        let r = lints_du_commun().expect("les lints du commun");
        assert_eq!(ecarts_de_lints(&r, &r), Vec::<String>::new());
        assert!(r.get("clippy").and_then(|c| c.get("unwrap_used")).is_some());
    }

    #[test]
    fn un_lint_assoupli_ou_manquant_se_voit() {
        let r = lints_du_commun().expect("les lints du commun");
        let mut jeu = r.clone();
        let clippy = jeu
            .get_mut("clippy")
            .and_then(toml::Value::as_table_mut)
            .expect("clippy");
        let _: Option<toml::Value> =
            clippy.insert(String::from("unwrap_used"), toml::Value::from("warn"));
        let _: Option<toml::Value> = clippy.remove("indexing_slicing");
        let _: Option<toml::Value> =
            clippy.insert(String::from("float_cmp"), toml::Value::from("allow"));
        // Durcir est permis.
        let _: Option<toml::Value> = clippy.insert(
            String::from("must_use_candidate"),
            toml::Value::from("deny"),
        );
        let e = ecarts_de_lints(&r, &jeu);
        assert_eq!(e.len(), 3, "{e:?}");
        assert!(
            e.iter().any(|x| x.contains("unwrap_used assoupli")),
            "{e:?}"
        );
        assert!(
            e.iter().any(|x| x.contains("indexing_slicing manque")),
            "{e:?}"
        );
        assert!(
            e.iter().any(|x| x.contains("float_cmp mis à allow")),
            "{e:?}"
        );
    }

    #[test]
    fn les_profils_du_commun_passent() {
        let r = table(super::CARGO_DU_COMMUN).expect("le manifeste du commun");
        assert_eq!(ecarts_de_profils(&r), Vec::<String>::new());
        let sans = table("[profile.module]\ninherits = \"release\"\npanic = \"abort\"\n[profile.release.package.x]\noverflow-checks = false\n")
            .expect("toml");
        let e = ecarts_de_profils(&sans);
        assert!(
            e.iter()
                .any(|x| x.contains("[profile.release] n'écrit pas")),
            "{e:?}"
        );
        assert!(
            e.iter()
                .any(|x| x.contains("[profile.module] n'a pas overflow-checks")),
            "{e:?}"
        );
        assert!(
            e.iter()
                .any(|x| x.contains("package.x] écrit overflow-checks = false")),
            "{e:?}"
        );
        assert!(e.iter().any(|x| x.contains("n'optimise pas")), "{e:?}");
    }

    #[test]
    fn le_clippy_du_commun_passe_et_un_assouplissement_se_voit() {
        let r = table(CLIPPY_DU_COMMUN).expect("le clippy.toml du commun");
        assert_eq!(ecarts_de_clippy(&r, &r), Vec::<String>::new());
        let texte = CLIPPY_DU_COMMUN
            .replace("allow-dbg-in-tests = false", "")
            .replace("max-fn-params-bools = 0", "max-fn-params-bools = 3")
            .replace("{ path = \"f32::powf\"", "{ path = \"f32::autre\"");
        let mut jeu = table(&texte).expect("toml");
        let _: Option<toml::Value> = jeu.insert(
            String::from("too-many-lines-threshold"),
            toml::Value::from(500),
        );
        let _: Option<toml::Value> = jeu.insert(
            String::from("disallowed-macros"),
            toml::Value::Array(Vec::new()),
        );
        let e = ecarts_de_clippy(&r, &jeu);
        assert!(
            e.iter().any(|x| x.contains("max-fn-params-bools vaut 3")),
            "{e:?}"
        );
        assert!(
            e.iter().any(|x| x.contains("n'interdit pas f32::powf")),
            "{e:?}"
        );
        assert!(
            e.iter().any(|x| x.contains("too-many-lines-threshold")),
            "{e:?}"
        );
        assert!(!e.iter().any(|x| x.contains("disallowed-macros")), "{e:?}");
    }

    #[test]
    fn un_allow_se_trouve_hors_des_commentaires_et_des_textes() {
        let source = r##"
//! #[allow(dead_code)] dans un commentaire
#![forbid(unsafe_code)]
const T: &str = "#[allow(x)]";
const B: &str = r#"#[allow(y)]"#;
/* #[allow(z)] /* imbriqué */ */
#[allow(clippy::unwrap_used)]
fn a<'x>(c: char) -> bool { c == '#' }
#[expect(clippy::panic)]
fn b() {}
#[expect(clippy::panic, reason = "dit")]
fn c() {}
#[cfg_attr(test, allow(unused))]
fn d() {}
"##;
        let a = assouplissements(source, super::Lieu::Code);
        assert_eq!(
            assouplissements(
                "#![allow(dead_code, reason = \"un module commun\")]",
                super::Lieu::Tests
            ),
            Vec::new()
        );
        assert_eq!(
            assouplissements("#![allow(dead_code)]", super::Lieu::Tests).len(),
            1
        );
        let lignes: Vec<usize> = a.iter().map(|(l, _)| *l).collect();
        assert_eq!(lignes, [7, 9, 13], "{a:?}");
        assert!(a_l_attribut(source, Attendu::InterditUnsafe));
        assert!(!a_l_attribut(source, Attendu::SansStd));
        assert_eq!(code_seul("'a' x").trim(), "x");
    }

    #[test]
    fn la_forme_du_module() {
        let deux_lignes = "//! Le module.\n\n#![cfg_attr(target_arch = \"wasm32\", no_std)]\njeu::module!(punchline::Punchline);\n";
        assert_eq!(
            forme_du_module(deux_lignes).as_deref(),
            Ok("punchline::Punchline")
        );
        assert_eq!(
            forme_du_module("#![no_std]\njeu::module!(fautifs::jeux::Sain);").as_deref(),
            Ok("fautifs::jeux::Sain")
        );
        assert!(forme_du_module("jeu::module!(a::B);\nfn f() {}").is_err());
        assert!(forme_du_module("#![allow(unsafe_code)]\njeu::module!(a::B);").is_err());
        assert!(forme_du_module("#[no_mangle]\njeu::module!(a::B);").is_err());
        assert!(forme_du_module("jeu::module!(a::B; fn x() {});").is_err());
    }
}
