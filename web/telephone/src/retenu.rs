//! Ce que la page a retenu (`Locaux::retenu`, `PageFil::retenir`), gardé dans
//! `localStorage` d'un rechargement à l'autre (PHASE-3, § 2.10 ; lot 12, fin) : par salle
//! et par jeu, comme `PageFil::retenir` le dit. Ce module décide, sans navigateur, sous
//! quelle clé le garder et quand l'écrire ; la page (`page`) lit et écrit le stockage.

/// Le début de la clé du retenu de la page, suivi de la salle et du module.
pub const CLE_RETENU: &str = "soiree-retenu";

/// La clé de `localStorage` du retenu d'une salle (le chemin de la page, `/s/<SALLE>`) et
/// d'un jeu (l'empreinte du module, en hexadécimal) : une autre salle, ou un autre jeu de
/// la même salle, ne relit pas ce qu'une page avait retenu ailleurs.
pub fn cle(chemin: &str, module_hexa: &str) -> String {
    format!("{CLE_RETENU}:{chemin}:{module_hexa}")
}

/// Ce qu'il faut écrire dans le stockage : le retenu `actuel` de la page, s'il y en a un et
/// qu'il diffère de ce qui y est déjà gardé. Une page qui ne retient rien (encore) n'efface
/// pas ce qu'une page d'avant le rechargement avait gardé.
pub fn a_ecrire<'a>(actuel: Option<&'a [u8]>, garde: Option<&[u8]>) -> Option<&'a [u8]> {
    actuel.filter(|a| Some(*a) != garde)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_cle_nomme_la_salle_et_le_jeu() {
        let a = cle("/s/3FA9C2D1", "ab12");
        assert_eq!(a, "soiree-retenu:/s/3FA9C2D1:ab12");
        assert_ne!(a, cle("/s/3FA9C2D2", "ab12"), "une autre salle");
        assert_ne!(
            a,
            cle("/s/3FA9C2D1", "ab13"),
            "un autre jeu de la même salle"
        );
    }

    #[test]
    fn un_retenu_s_ecrit_quand_il_change_et_jamais_ne_s_efface() {
        assert_eq!(a_ecrire(Some(&[1, 2]), None), Some(&[1u8, 2][..]));
        assert_eq!(a_ecrire(Some(&[1, 2]), Some(&[1, 2])), None, "déjà gardé");
        assert_eq!(a_ecrire(Some(&[3]), Some(&[1, 2])), Some(&[3u8][..]));
        // Une page neuve, avant son premier appel, ne retient rien : elle n'efface pas ce
        // que la page d'avant le rechargement avait gardé.
        assert_eq!(a_ecrire(None, Some(&[1, 2])), None);
        assert_eq!(a_ecrire(None, None), None);
    }
}
