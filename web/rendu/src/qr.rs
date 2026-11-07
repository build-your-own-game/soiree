//! Le QR code de l'adresse que les téléphones joignent (SQ-16), dessiné en carrés
//! (SAL-191) : `qrcodegen` en donne les modules, le rendu les pose en quads. Le test
//! `tests/qr.rs` relit le QR dessiné par `rqrr` et retrouve l'adresse.
//!
//! Le code est un carré centré dans la boîte, avec sa marge de quatre modules (la zone
//! calme de la norme), blanc, les modules noirs ; la taille d'un module est un nombre
//! entier de pixels dès qu'il en vaut au moins un, pour des bords nets.

use qrcodegen::{QrCode, QrCodeEcc};

use crate::liste::{Boite, Element, Sorte, premultipliee};
use jeu_abi::Rgba;

/// La zone calme autour des modules, en modules.
pub const MARGE: i32 = 4;

/// Les modules du QR code d'une adresse : son côté, et chaque module, noir ou non, ligne
/// par ligne. `None` si l'adresse ne tient dans aucun QR code.
pub fn modules(adresse: &str) -> Option<(i32, Vec<bool>)> {
    let code = QrCode::encode_text(adresse, QrCodeEcc::Medium).ok()?;
    let n = code.size();
    let mut m = Vec::new();
    for y in 0..n {
        for x in 0..n {
            m.push(code.get_module(x, y));
        }
    }
    Some((n, m))
}

/// Pose le QR code d'une adresse dans une boîte en pixels : un fond blanc, puis les
/// modules noirs, une suite de modules d'une même ligne en un seul quad.
pub fn elements(adresse: &str, b: Boite, opacite: f32, noeud: u32, sortie: &mut Vec<Element>) {
    let Some((n, m)) = modules(adresse) else {
        return;
    };
    let cote_boite = if b[2] - b[0] < b[3] - b[1] {
        b[2] - b[0]
    } else {
        b[3] - b[1]
    };
    let total = (n + 2 * MARGE) as f32;
    let mut module = cote_boite / total;
    if module >= 1.0 {
        module = module.floor();
    }
    let cote = module * total;
    let x0 = (b[0] + (b[2] - b[0] - cote) / 2.0).round();
    let y0 = (b[1] + (b[3] - b[1] - cote) / 2.0).round();
    let quad = |boite: Boite, c: Rgba| Element {
        noeud,
        boite,
        sorte: Sorte::Quad {
            couleur: premultipliee(c, opacite),
            image: None,
        },
        couche: 0,
    };
    sortie.push(quad([x0, y0, x0 + cote, y0 + cote], Rgba::BLANC));
    let marge = MARGE as f32 * module;
    for y in 0..n {
        let mut x = 0;
        while x < n {
            let noir = |x: i32| {
                usize::try_from(y * n + x)
                    .ok()
                    .and_then(|k| m.get(k))
                    .copied()
                    .unwrap_or(false)
            };
            if !noir(x) {
                x += 1;
                continue;
            }
            let debut = x;
            while x < n && noir(x) {
                x += 1;
            }
            let gauche = x0 + marge + debut as f32 * module;
            let haut = y0 + marge + y as f32 * module;
            sortie.push(quad(
                [
                    gauche,
                    haut,
                    gauche + (x - debut) as f32 * module,
                    haut + module,
                ],
                Rgba::NOIR,
            ));
        }
    }
}
