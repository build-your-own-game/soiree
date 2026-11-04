//! Les règles d'écriture du commun (`clippy.toml`, CONTRAT « Les règles d'écriture ») :
//! ce qu'elles doivent interdire pour que la copie native donne les mêmes résultats
//! que le module.

const CLIPPY: &str = include_str!("../../../clippy.toml");

/// `f32::min` et `f32::max` passent par `fminnm` et `fmaxnm` sur aarch64, et par
/// une comparaison suivie d'un `select` dans le module : `min(-0.0, 0.0)` rend
/// `-0.0` d'un côté et `+0.0` de l'autre (revue du 2026-11-04, Apple M5). Les
/// règles les interdisent, comme les fonctions de `libm` qui diffèrent.
#[test]
fn min_et_max_des_flottants_sont_interdits() {
    for methode in ["f32::min", "f32::max", "f64::min", "f64::max"] {
        let entree = format!("{{ path = \"{methode}\",");
        assert!(CLIPPY.contains(&entree), "{methode} manque à clippy.toml");
    }
}
