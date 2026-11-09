//! Le style d'un nœud de la page, en CSS, sans navigateur (PHASE-3, § 2.10).
//!
//! Les tailles sont en pixels CSS ou en part de la hauteur visible ; **un champ n'est
//! jamais sous 16 px** (sinon iOS zoome) ; la police est celle d'un asset du jeu, chargée
//! par `FontFace` sous le nom [`famille`], ou celle du commun.

use jeu_abi::{Alignement, Cle, Oui, Rgba, StylePage, TaillePage};

/// Le fond de la page du commun : celui du corps tant que le jeu n'en donne pas
/// (`Reglages::fond` à `None`, ou avant la première vue), et celui des écrans du commun (le
/// voile), qui gardent le leur (D2, lot 14 corrigé ; choix provisoire, laissé au
/// mainteneur : `RESTE-PHASE-3.md`, § 6.2). Le banc relit le même (`banc::pages`).
pub const FOND_DU_COMMUN: &str = "#111";

/// La taille d'un champ, au moins (iOS zoome en dessous).
pub const CHAMP_PX_MIN: u16 = 16;

/// La pile de polices du commun sur le téléphone : celle du système, sans empattement (la
/// police du commun de la télé n'est pas envoyée aux téléphones).
pub const POLICE_DU_COMMUN: &str = "system-ui, -apple-system, \"Segoe UI\", Roboto, sans-serif";

/// Le nom de la famille d'une police d'asset, chargée par `FontFace`.
pub fn famille(cle: Cle) -> String {
    format!("p{:016x}", cle.fil())
}

/// Une couleur en CSS.
pub fn couleur(c: Rgba) -> String {
    let Rgba(rouge, vert, bleu, opacite) = c;
    let alpha = f64::from(opacite) / 255.0;
    format!("rgba({rouge},{vert},{bleu},{alpha:.3})")
}

/// Le fond du corps de la page pour les réglages d'une vue : la couleur que le jeu donne
/// (`Reglages::fond`, « la couleur de fond de la page », INTERFACE § 2.9), sinon celle du
/// commun (D2, lot 14 corrigé).
pub fn fond_de_la_page(fond: Option<Rgba>) -> String {
    fond.map_or_else(|| String::from(FOND_DU_COMMUN), couleur)
}

/// Une taille en CSS ; `min` en pixels (16 pour un champ).
pub fn taille(t: TaillePage, min: u16) -> String {
    match t {
        TaillePage::Px(n) => format!("{}px", n.max(min)),
        TaillePage::Hauteur(p) if min > 0 => format!("max({p}vh, {min}px)"),
        TaillePage::Hauteur(p) => format!("{p}vh"),
    }
}

/// Les déclarations CSS d'un style de page (`propriété`, `valeur`).
pub fn declarations(s: &StylePage, champ: Oui) -> Vec<(&'static str, String)> {
    let mut d = vec![
        (
            "font-family",
            if s.police == Cle::POLICE_DU_COMMUN {
                String::from(POLICE_DU_COMMUN)
            } else {
                format!("{}, {POLICE_DU_COMMUN}", famille(s.police))
            },
        ),
        (
            "font-size",
            taille(s.taille, if champ == Oui::Oui { CHAMP_PX_MIN } else { 0 }),
        ),
        (
            "text-align",
            String::from(match s.alignement {
                Alignement::Debut => "start",
                Alignement::Centre => "center",
                Alignement::Fin => "end",
            }),
        ),
    ];
    if let Some(c) = s.couleur {
        d.push(("color", couleur(c)));
    }
    if let Some(f) = s.fond {
        d.push(("background-color", couleur(f)));
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_champ_n_est_jamais_sous_16_px() {
        let mut s = StylePage::ORDINAIRE;
        s.taille = TaillePage::Px(12);
        let d = declarations(&s, Oui::Oui);
        assert!(d.contains(&("font-size", String::from("16px"))));
        let d = declarations(&s, Oui::Non);
        assert!(d.contains(&("font-size", String::from("12px"))));
        assert_eq!(taille(TaillePage::Hauteur(3), 16), "max(3vh, 16px)");
        assert_eq!(couleur(Rgba(1, 2, 3, 255)), "rgba(1,2,3,1.000)");
        assert_eq!(famille(Cle::du_fil(0xab)), "p00000000000000ab");
    }

    #[test]
    fn le_fond_de_la_page_est_celui_du_jeu_sinon_celui_du_commun_d2() {
        assert_eq!(
            fond_de_la_page(Some(Rgba(200, 180, 40, 255))),
            "rgba(200,180,40,1.000)"
        );
        assert_eq!(fond_de_la_page(None), FOND_DU_COMMUN);
    }
}
