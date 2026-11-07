//! L'adresse de la page : `/s/<SALLE>` et le fragment du QR (PHASE-3, § 2.6).

use liaison::CodeSalle;
use salle::chiffre::Fragment;

/// Ce qui ne va pas dans l'adresse : la page dit alors « Scanne le nouveau QR code ».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdresseIllisible {
    /// Le chemin n'est pas `/s/<8 chiffres hexadécimaux majuscules>` (SAL-119).
    Salle,
    /// Le fragment manque, ou ne se lit pas.
    Fragment,
}

/// La salle et le fragment, lus du chemin et du fragment (`location.hash`, avec ou sans
/// son `#`). Le navigateur n'envoie jamais le fragment au relais.
pub fn lire(chemin: &str, fragment: &str) -> Result<(CodeSalle, Fragment), AdresseIllisible> {
    let code = chemin
        .strip_prefix("/s/")
        .map(|c| c.trim_end_matches('/'))
        .ok_or(AdresseIllisible::Salle)?;
    let salle = CodeSalle::lire(code).ok_or(AdresseIllisible::Salle)?;
    let f = fragment.strip_prefix('#').unwrap_or(fragment);
    let f = Fragment::lire(f).map_err(|_| AdresseIllisible::Fragment)?;
    Ok((salle, f))
}

#[cfg(test)]
mod tests {
    use super::*;
    use salle::chiffre::{ClePublique, SecretDEntree};

    #[test]
    fn la_salle_et_le_fragment_se_lisent_sal119() {
        let f = Fragment {
            secret: SecretDEntree::depuis_octets([7; 16]),
            autorite: ClePublique([9; 32]),
        };
        let texte = f.ecrire();
        let (salle, lu) = lire("/s/3FA9C2D1", &format!("#{texte}")).expect("lisible");
        assert_eq!(salle, CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]));
        assert_eq!(lu.ecrire(), texte);
        assert_eq!(
            lire("/s/3fa9c2d1", &texte).err(),
            Some(AdresseIllisible::Salle)
        );
        assert_eq!(
            lire("/x/3FA9C2D1", &texte).err(),
            Some(AdresseIllisible::Salle)
        );
        assert_eq!(
            lire("/s/3FA9C2D1", "").err(),
            Some(AdresseIllisible::Fragment)
        );
        assert_eq!(
            lire("/s/3FA9C2D1", &format!("{texte}=")).err(),
            Some(AdresseIllisible::Fragment)
        );
    }
}
