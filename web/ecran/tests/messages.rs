//! Ce que la page de la télé fait d'un message du Worker, en natif.

#![expect(
    clippy::unwrap_used,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

use ecran::messages::{Recu, recevoir};
use jeu_abi::{Cle, Pas, Rgba, SceneFil, encoder};
use mise_en_page::{OCTETS_DU_COMMUN, Polices};
use salle::ecran::VersPage;

fn message(m: &VersPage<'_>) -> Vec<u8> {
    m.encoder().unwrap()
}

#[test]
fn une_scene_recue_se_lit() {
    let scene = SceneFil {
        fond: Some(Rgba::BLANC),
        ..SceneFil::vide()
    };
    let mut o = Vec::new();
    encoder(&scene, &mut o).unwrap();
    let mut polices = Polices::du_commun();
    let r = recevoir(
        &message(&VersPage::Scene {
            pas: Pas::du_fil(3),
            t_us: 0,
            octets: &o,
        }),
        &mut polices,
    )
    .unwrap();
    assert_eq!(
        r,
        Recu::Scene {
            pas: Pas::du_fil(3),
            scene
        }
    );
}

#[test]
fn une_police_s_ajoute_une_image_se_decode_un_woff2_est_refuse() {
    let mut polices = Polices::du_commun();
    // Une seconde copie de la police du commun sous une autre clé : mêmes octets, admise.
    let r = recevoir(
        &message(&VersPage::Ressource {
            cle: 9,
            octets: OCTETS_DU_COMMUN,
        }),
        &mut polices,
    )
    .unwrap();
    assert_eq!(r, Recu::Police(Cle::du_fil(9)));
    assert_eq!(polices.cles().count(), 2);
    let png = [0x89, b'P', b'N', b'G', 13, 10, 26, 10];
    let r = recevoir(
        &message(&VersPage::Ressource {
            cle: 10,
            octets: &png,
        }),
        &mut polices,
    )
    .unwrap();
    assert_eq!(
        r,
        Recu::Image {
            cle: Cle::du_fil(10),
            octets: png.to_vec()
        }
    );
    let woff2 = [b'w', b'O', b'F', b'2', 0, 1, 0, 0];
    let e = recevoir(
        &message(&VersPage::Ressource {
            cle: 11,
            octets: &woff2,
        }),
        &mut polices,
    )
    .unwrap_err();
    assert!(e.contains("WOFF2"), "{e}");
    assert!(recevoir(&[0xff, 0xff], &mut polices).is_err());
}

/// Un son déclaré (lot 10b) se reconnaît à sa signature, et se garde pour la page, qui le
/// décode quand un effet le joue ; il n'est ni une police ni une image.
#[test]
fn un_son_se_garde_pour_la_page() {
    use ecran::messages::est_un_son;
    let mut wave = b"RIFF\x24\0\0\0WAVEfmt ".to_vec();
    wave.extend_from_slice(&[0; 16]);
    for o in [
        wave.as_slice(),
        b"OggS\0\x02".as_slice(),
        b"fLaC\0\0".as_slice(),
        b"ID3\x04\0".as_slice(),
        [0xff, 0xfb, 0x90, 0x64].as_slice(),
    ] {
        assert!(est_un_son(o), "{o:?}");
    }
    for o in [
        b"\x89PNG\r\n\x1a\n".as_slice(),
        b"<svg".as_slice(),
        b"RIFF\0\0\0\0WEBP".as_slice(),
        b"".as_slice(),
    ] {
        assert!(!est_un_son(o), "{o:?}");
    }
    let mut polices = Polices::du_commun();
    let r = recevoir(
        &message(&VersPage::Ressource {
            cle: 5,
            octets: &wave,
        }),
        &mut polices,
    )
    .unwrap();
    assert_eq!(
        r,
        Recu::Son {
            cle: Cle::du_fil(5),
            octets: wave.clone()
        }
    );
}
