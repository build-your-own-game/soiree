//! De vrais fichiers, pour les bots (§ 2.13, `ComposantBot::Fichiers`).
//!
//! Le commun décode les images qu'apporte un bot comme celles d'un joueur : une image
//! factice (une signature PNG suivie de n'importe quoi) lui revient `Illisible`. Pour
//! qu'une soirée de l'épreuve joue aussi le chemin d'une image lue, recadrée et
//! réduite (TL-60), un bot apporte un vrai PNG : [`png_uni`] en écrit un, sans
//! compression, en `no_std`, assez petit pour un bot.

use alloc::vec::Vec;

use jeu_abi::Rgba;

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Le plus grand bloc non compressé de DEFLATE.
const BLOC: usize = 65_535;

/// Le CRC-32 des PNG (ISO 3309), bit à bit : rien ne presse dans un bot.
fn crc32(morceaux: &[&[u8]]) -> u32 {
    let mut c: u32 = 0xffff_ffff;
    for m in morceaux {
        for &b in *m {
            c ^= u32::from(b);
            for _ in 0..8 {
                let masque = (c & 1).wrapping_neg();
                c = (c >> 1) ^ (0xedb8_8320 & masque);
            }
        }
    }
    !c
}

/// L'Adler-32 du flux zlib.
fn adler32(o: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in o {
        a = a
            .wrapping_add(u32::from(x))
            .checked_rem(65_521)
            .unwrap_or(0);
        b = b.wrapping_add(a).checked_rem(65_521).unwrap_or(0);
    }
    (b << 16) | a
}

fn morceau(o: &mut Vec<u8>, sorte: [u8; 4], donnees: &[u8]) {
    let n = u32::try_from(donnees.len()).unwrap_or(u32::MAX);
    o.extend_from_slice(&n.to_be_bytes());
    o.extend_from_slice(&sorte);
    o.extend_from_slice(donnees);
    o.extend_from_slice(&crc32(&[&sorte, donnees]).to_be_bytes());
}

/// Un PNG uni de `largeur` × `hauteur` pixels, de cette couleur : RVBA de 8 bits,
/// sans filtre ni compression (des blocs DEFLATE « stockés »). Un côté nul est lu 1.
/// Il pèse un peu plus de `4 × largeur × hauteur` octets : un bot en apporte de
/// petits.
pub fn png_uni(largeur: u16, hauteur: u16, couleur: Rgba) -> Vec<u8> {
    let (l, h) = (largeur.max(1), hauteur.max(1));
    let px = [couleur.0, couleur.1, couleur.2, couleur.3];
    // Une rangée : son filtre (aucun), puis ses pixels.
    let mut rangee = Vec::from([0u8]);
    for _ in 0..l {
        rangee.extend_from_slice(&px);
    }
    let mut brut = Vec::with_capacity(rangee.len().saturating_mul(usize::from(h)));
    for _ in 0..h {
        brut.extend_from_slice(&rangee);
    }
    let mut z = Vec::from([0x78, 0x01]);
    let mut blocs = brut.chunks(BLOC).peekable();
    while let Some(b) = blocs.next() {
        z.push(u8::from(blocs.peek().is_none()));
        let n = u16::try_from(b.len()).unwrap_or(u16::MAX);
        z.extend_from_slice(&n.to_le_bytes());
        z.extend_from_slice(&(!n).to_le_bytes());
        z.extend_from_slice(b);
    }
    z.extend_from_slice(&adler32(&brut).to_be_bytes());
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&u32::from(l).to_be_bytes());
    ihdr.extend_from_slice(&u32::from(h).to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    let mut o = Vec::from(SIGNATURE);
    morceau(&mut o, *b"IHDR", &ihdr);
    morceau(&mut o, *b"IDAT", &z);
    morceau(&mut o, *b"IEND", &[]);
    o
}
