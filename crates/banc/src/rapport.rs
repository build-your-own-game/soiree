//! Ce qu'une page de test rapporte (`POST /banc/rapport`), et son jugement.
//!
//! Un essai qui n'a pas tourné n'a pas réussi (OUT-01) : une page qui ne rapporte rien,
//! ou qui rapporte une erreur, a échoué (code 1), jamais réussi ; une famille d'essais
//! que le lot demande et que le rapport ne contient pas, un essai de moins ou de plus
//! qu'attendu, ou le rapport d'un autre passage font échouer le jugement. « Pas
//! exécuté » (code 2) est réservé à ce que la machine ne permet pas : un Chrome qui ne
//! part pas, une construction impossible (`crate::machine`).

use serde_json::Value as Json;

/// Les familles des essais de `Machine` que le lot 3a demande (PHASE-3, § 7) : le nom
/// de chaque essai commence par la sienne, suivie de `_`.
pub const FAMILLES_MACHINE: [&str; 6] = [
    "lire",
    "ecrire",
    "remplir",
    "grandir",
    "globales_i64",
    "pieges",
];

/// Le nombre d'essais de `essais-navigateur` (lot 3a) : vingt essais de `Machine`, onze
/// pièges, la table des messages constatée, et le chargement d'une boucle. Un essai qui
/// disparaît du rapport sans qu'on change ce nombre fait échouer le jugement (OUT-01).
pub const ESSAIS_MACHINE: usize = 33;

/// Ce qu'une suite doit rapporter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Suite<'a> {
    /// Les familles qui doivent avoir au moins un essai.
    pub familles: &'a [&'a str],
    /// Le nombre exact d'essais.
    pub essais: usize,
}

/// La suite de `soiree-banc machine`.
pub const SUITE_MACHINE: Suite<'static> = Suite {
    familles: &FAMILLES_MACHINE,
    essais: ESSAIS_MACHINE,
};

/// Un essai rapporté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Essai {
    /// Son nom, qui commence par sa famille.
    pub nom: String,
    /// Réussi ou non.
    pub reussi: bool,
    /// Ce qu'il a constaté, ou pourquoi il échoue.
    pub detail: String,
}

/// Ce que le navigateur a levé pour un scénario, brut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leve {
    /// Le scénario de `essais/machine.wat`.
    pub scenario: i64,
    /// `error.name`.
    pub nom: String,
    /// `error.message`.
    pub message: String,
}

/// Un rapport lu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rapport {
    /// Les essais, dans l'ordre.
    pub essais: Vec<Essai>,
    /// Ce que le navigateur a levé.
    pub leves: Vec<Leve>,
    /// `navigator.userAgent`.
    pub agent: String,
    /// Le défaut injecté que la page a joué (`?fautif=`), `""` pour la vraie machine.
    pub fautif: String,
}

fn texte(v: &Json, cle: &str) -> Result<String, String> {
    v.get(cle)
        .and_then(Json::as_str)
        .map(String::from)
        .ok_or_else(|| format!("« {cle} » manque : {v}"))
}

/// Lit le corps d'un rapport. Une page dont la suite n'a pas tourné rapporte
/// `{ "erreur": … }` : c'est une erreur ici.
pub fn lire(corps: &[u8]) -> Result<Rapport, String> {
    let v: Json = serde_json::from_slice(corps).map_err(|e| format!("rapport illisible : {e}"))?;
    if let Some(e) = v.get("erreur").filter(|e| !e.is_null()) {
        return Err(format!(
            "la suite n'a pas tourné : {}",
            e.as_str().map_or_else(|| e.to_string(), String::from)
        ));
    }
    let mut essais = Vec::new();
    for e in v
        .get("essais")
        .and_then(Json::as_array)
        .ok_or_else(|| String::from("« essais » manque"))?
    {
        essais.push(Essai {
            nom: texte(e, "nom")?,
            reussi: e
                .get("reussi")
                .and_then(Json::as_bool)
                .ok_or_else(|| format!("« reussi » manque : {e}"))?,
            detail: texte(e, "detail")?,
        });
    }
    let mut leves = Vec::new();
    for l in v
        .get("leves")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
    {
        leves.push(Leve {
            scenario: l.get("scenario").and_then(Json::as_i64).unwrap_or(-1),
            nom: texte(l, "nom")?,
            message: texte(l, "message")?,
        });
    }
    Ok(Rapport {
        essais,
        leves,
        agent: v
            .get("agent")
            .and_then(Json::as_str)
            .unwrap_or("?")
            .to_owned(),
        fautif: texte(&v, "fautif")?,
    })
}

/// Ce qu'on attend d'un passage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attendu {
    /// Tout réussit.
    ToutReussi,
    /// Le contrôle négatif : les essais de cette famille échouent (au moins un), et eux
    /// seuls.
    SeuleEchoue(&'static str),
}

fn famille_de<'a>(nom: &str, familles: &[&'a str]) -> Option<&'a str> {
    familles.iter().copied().find(|f| {
        nom.strip_prefix(f)
            .is_some_and(|reste| reste.starts_with('_'))
    })
}

/// Juge le rapport du passage `fautif` (`""` pour la vraie machine) : rend ce qu'il
/// montre, ou pourquoi il échoue.
pub fn juger(
    r: &Rapport,
    suite: &Suite<'_>,
    fautif: &str,
    attendu: Attendu,
) -> Result<String, String> {
    let passage = |f: &str| {
        if f.is_empty() {
            String::from("de la vraie machine")
        } else {
            format!("du fautif « {f} »")
        }
    };
    if r.fautif != fautif {
        return Err(format!(
            "le rapport est celui {}, le passage celui {}",
            passage(&r.fautif),
            passage(fautif)
        ));
    }
    if r.essais.is_empty() {
        return Err(String::from("aucun essai rapporté"));
    }
    if r.essais.len() != suite.essais {
        return Err(format!(
            "{} essais rapportés, {} attendus",
            r.essais.len(),
            suite.essais
        ));
    }
    let mut vus = std::collections::BTreeSet::new();
    if let Some(e) = r.essais.iter().find(|e| !vus.insert(e.nom.as_str())) {
        return Err(format!("{} rapporté deux fois", e.nom));
    }
    let manquent: Vec<&str> = suite
        .familles
        .iter()
        .copied()
        .filter(|f| !r.essais.iter().any(|e| famille_de(&e.nom, &[f]).is_some()))
        .collect();
    if !manquent.is_empty() {
        return Err(format!("familles sans essai : {}", manquent.join(", ")));
    }
    let echecs: Vec<&Essai> = r.essais.iter().filter(|e| !e.reussi).collect();
    let dire = |es: &[&Essai]| {
        es.iter()
            .map(|e| format!("{} ({})", e.nom, e.detail))
            .collect::<Vec<_>>()
            .join(" ; ")
    };
    match attendu {
        Attendu::ToutReussi => {
            if echecs.is_empty() {
                Ok(format!("{} essais réussis", r.essais.len()))
            } else {
                Err(format!("en échec : {}", dire(&echecs)))
            }
        }
        Attendu::SeuleEchoue(f) => {
            let ailleurs: Vec<&Essai> = echecs
                .iter()
                .copied()
                .filter(|e| famille_de(&e.nom, &[f]).is_none())
                .collect();
            if echecs.is_empty() {
                Err(format!(
                    "le contrôle négatif n'a rien fait échouer dans {f}"
                ))
            } else if !ailleurs.is_empty() {
                Err(format!("en échec hors de {f} : {}", dire(&ailleurs)))
            } else {
                Ok(format!(
                    "{} essais de {f} en échec, et eux seuls : {}",
                    echecs.len(),
                    echecs
                        .iter()
                        .map(|e| e.nom.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Attendu, Essai, FAMILLES_MACHINE, Rapport, Suite, juger, lire};

    const NOMS: [&str; 8] = [
        "lire_a",
        "ecrire_b",
        "remplir_c",
        "grandir_d",
        "globales_i64_e",
        "globales_i64_f",
        "globales_des_autres_types",
        "pieges_g",
    ];

    const SUITE: Suite<'static> = Suite {
        familles: &FAMILLES_MACHINE,
        essais: NOMS.len(),
    };

    fn rapport(fautif: &str, echecs: &[&str]) -> Rapport {
        Rapport {
            essais: NOMS
                .iter()
                .map(|n| Essai {
                    nom: String::from(*n),
                    reussi: !echecs.contains(n),
                    detail: String::new(),
                })
                .collect(),
            leves: Vec::new(),
            agent: String::new(),
            fautif: String::from(fautif),
        }
    }

    #[test]
    fn tout_reussi() {
        assert!(juger(&rapport("", &[]), &SUITE, "", Attendu::ToutReussi).is_ok());
        assert!(juger(&rapport("", &["pieges_g"]), &SUITE, "", Attendu::ToutReussi).is_err());
    }

    /// Une famille absente n'a pas réussi : elle n'a pas tourné (OUT-01).
    #[test]
    fn une_famille_absente_echoue_out01() {
        let mut r = rapport("", &[]);
        r.essais.retain(|e| !e.nom.starts_with("remplir"));
        let suite = Suite {
            essais: r.essais.len(),
            ..SUITE
        };
        let e = juger(&r, &suite, "", Attendu::ToutReussi).unwrap_err();
        assert!(e.contains("remplir"), "{e}");
        r.essais.clear();
        assert!(juger(&r, &suite, "", Attendu::ToutReussi).is_err());
    }

    /// Un essai qui disparaît, même dans une famille qui en garde d'autres, ou qui
    /// revient deux fois, fait échouer : le compte est exact (OUT-01).
    #[test]
    fn un_essai_de_moins_ou_en_double_echoue_out01() {
        let mut r = rapport("", &[]);
        r.essais.retain(|e| e.nom != "globales_i64_f");
        let e = juger(&r, &SUITE, "", Attendu::ToutReussi).unwrap_err();
        assert!(e.contains("7 essais rapportés, 8 attendus"), "{e}");
        let double = r.essais[0].clone();
        r.essais.push(double);
        let e = juger(&r, &SUITE, "", Attendu::ToutReussi).unwrap_err();
        assert!(e.contains("lire_a rapporté deux fois"), "{e}");
    }

    /// Le rapport d'un passage ne vaut pas pour l'autre : un contrôle négatif dont la
    /// page aurait joué la vraie machine ne passe pas, même si ses échecs tombaient bien.
    #[test]
    fn le_rapport_d_un_autre_passage_echoue() {
        let f = Attendu::SeuleEchoue("globales_i64");
        let e = juger(
            &rapport("", &["globales_i64_e"]),
            &SUITE,
            "i64-par-nombre",
            f,
        )
        .unwrap_err();
        assert!(
            e.contains("celui de la vraie machine, le passage celui du fautif « i64-par-nombre »"),
            "{e}"
        );
        assert!(
            juger(
                &rapport("i64-par-nombre", &[]),
                &SUITE,
                "",
                Attendu::ToutReussi
            )
            .is_err()
        );
    }

    #[test]
    fn le_controle_negatif_n_echoue_que_dans_sa_famille() {
        let f = Attendu::SeuleEchoue("globales_i64");
        let n = "i64-par-nombre";
        assert!(juger(&rapport(n, &["globales_i64_e"]), &SUITE, n, f).is_ok());
        // `globales_des_autres_types` n'est pas de la famille `globales_i64`.
        assert!(
            juger(
                &rapport(n, &["globales_i64_e", "globales_des_autres_types"]),
                &SUITE,
                n,
                f
            )
            .is_err()
        );
        assert!(juger(&rapport(n, &[]), &SUITE, n, f).is_err());
    }

    #[test]
    fn un_rapport_d_erreur_n_est_pas_un_succes() {
        assert!(lire(br#"{"erreur":"RuntimeError: unreachable","fautif":""}"#).is_err());
        assert!(lire(b"pas du json").is_err());
        // Sans le passage qu'il dit avoir joué, un rapport ne se lit pas.
        assert!(lire(br#"{"essais":[],"leves":[],"agent":"Chrome"}"#).is_err());
        let r = lire(
            br#"{"essais":[{"nom":"lire_a","reussi":true,"detail":"x"}],"leves":[{"scenario":2,"nom":"RuntimeError","message":"unreachable"}],"agent":"Chrome","fautif":"i64-par-nombre"}"#,
        )
        .unwrap();
        assert_eq!(r.essais.len(), 1);
        assert_eq!(r.leves[0].message, "unreachable");
        assert_eq!(r.fautif, "i64-par-nombre");
    }
}
