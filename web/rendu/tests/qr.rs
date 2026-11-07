//! Le QR code de l'adresse (SAL-191, SQ-16) : dessiné par le rendu, relu par `rqrr`, un
//! juge qui n'est pas livré (CONTRAT, « Le code tiers »), il rend l'adresse.

#![expect(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::arithmetic_side_effects,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

pub mod commun;

use commun::{dessiner, forme, noeud, rect, rendu, scene};
use jeu_abi::{Rgba, SorteNoeud};
use rendu::{Cadre, Contexte, ImageRgba};

const ADRESSE: &str =
    "http://192.168.1.23:7878/s/3FA9C2D1#k=Q2xlIHB1YmxpcXVlIGRlIGwnYXV0b3JpdMOp&s=c2VjcmV0";

/// Relit les QR codes d'une image.
fn lire(image: &ImageRgba) -> Vec<String> {
    let mut preparee = rqrr::PreparedImage::prepare_from_greyscale(
        image.largeur as usize,
        image.hauteur as usize,
        |x, y| {
            let p = image.pixel(x as u32, y as u32).unwrap();
            ((u16::from(p[0]) + u16::from(p[1]) + u16::from(p[2])) / 3) as u8
        },
    );
    preparee
        .detect_grids()
        .into_iter()
        .map(|g| g.decode().expect("le QR se décode").1)
        .collect()
}

fn qr(contexte: &Contexte, l: u32, h: u32) -> ImageRgba {
    let mut r = rendu();
    let s = scene(
        Rgba(109, 34, 195, 255),
        vec![noeud(
            1,
            None,
            SorteNoeud::Qr {
                boite: rect(40, 10, 60, 60),
            },
        )],
    );
    dessiner(&mut r, &s, &Cadre::new(l, h, forme(160, 90)), contexte)
}

#[test]
fn le_qr_dessine_se_relit_et_rend_l_adresse_sal191() {
    let contexte = Contexte {
        adresse_qr: Some(String::from(ADRESSE)),
        ..Contexte::au_repos()
    };
    // À l'échelle de la télé (1280 × 720), puis petit : des modules de 1 à 2 pixels.
    for (l, h) in [(1280, 720), (320, 180)] {
        assert_eq!(lire(&qr(&contexte, l, h)), [ADRESSE], "à {l} × {h}");
    }
}

#[test]
fn sans_adresse_aucun_qr_n_est_dessine() {
    assert_eq!(
        lire(&qr(&Contexte::au_repos(), 640, 360)),
        Vec::<String>::new()
    );
}

/// Les modules ont la forme d'un QR code (ISO/CEI 18004) : un côté de 17 + 4 v, les
/// trois motifs de position entourés de blanc, les lignes de synchronisation, le module
/// toujours noir ; aucun motif de position dans le coin bas droit. Le contenu, lui, est
/// tenu par la relecture de `rqrr` (`le_qr_dessine_se_relit_et_rend_l_adresse_sal191`).
#[test]
fn les_modules_ont_la_forme_d_un_qr_code() {
    let (n, m) = rendu::qr::modules(ADRESSE).unwrap();
    assert_eq!(m.len(), (n * n) as usize);
    assert!(n >= 21 && (n - 17) % 4 == 0, "côté {n}");
    let noir = |x: i32, y: i32| m[(y * n + x) as usize];
    // Un motif de position : 7 × 7, un anneau noir, un anneau blanc, un carré noir de 3.
    let motif = |x0: i32, y0: i32| {
        (0..7).all(|dy| {
            (0..7).all(|dx| {
                let anneau = dx.min(dy).min(6 - dx).min(6 - dy);
                noir(x0 + dx, y0 + dy) == (anneau != 1)
            })
        })
    };
    assert!(motif(0, 0) && motif(n - 7, 0) && motif(0, n - 7));
    assert!(!motif(n - 7, n - 7));
    // Le séparateur blanc autour de chaque motif.
    for k in 0..8 {
        for (x, y) in [
            (7, k),
            (k, 7),
            (n - 8, k),
            (n - 8 + k, 7),
            (7, n - 8 + k),
            (k, n - 8),
        ] {
            assert!(!noir(x, y), "séparateur ({x}, {y})");
        }
    }
    // Les lignes de synchronisation, noir puis blanc, entre les motifs.
    for k in 8..n - 8 {
        assert_eq!(noir(k, 6), k % 2 == 0, "synchronisation ({k}, 6)");
        assert_eq!(noir(6, k), k % 2 == 0, "synchronisation (6, {k})");
    }
    // Le module noir, à côté du motif bas gauche.
    assert!(noir(8, n - 8));
    // Une autre adresse donne d'autres modules.
    assert_ne!(
        rendu::qr::modules("http://192.168.1.23:7878/s/AUTRE")
            .unwrap()
            .1,
        m
    );
}
