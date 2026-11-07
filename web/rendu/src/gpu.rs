//! Les données que le rendu envoie au GPU, et les programmes qui les lisent (CONTRAT,
//! « Les types de bout en bout » : « Les données envoyées au GPU partagent leur
//! définition avec ses programmes »).
//!
//! Chaque structure est `#[repr(C)]` ; ses octets s'écrivent champ par champ aux
//! décalages que donne `offset_of!`, sans code `unsafe` ni dépendance. Le test
//! `tests/wgsl.rs` lit [`QUADS_WGSL`] par `naga` et compare à ces structures les tailles
//! et les décalages de l'uniforme, et l'emplacement, le type et le décalage de chaque
//! attribut d'un quad : renommer ou déplacer un champ d'un côté fait rougir le test.

use std::mem::{offset_of, size_of};

use wgpu::{VertexAttribute, VertexFormat};

/// Le programme des quads : chaque rectangle, chaque image, chaque module du QR code est
/// un quad instancié, une texture multipliée par une couleur prémultipliée.
pub const QUADS_WGSL: &str = include_str!("quads.wgsl");

/// Un quad, tel que le GPU le lit (un attribut d'instance par champ ; `@location` dans
/// l'ordre des champs).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    /// Le coin haut gauche, en pixels de la cible.
    pub position: [f32; 2],
    /// La largeur et la hauteur, en pixels.
    pub taille: [f32; 2],
    /// Le coin haut gauche dans la texture, de 0 à 1.
    pub uv_origine: [f32; 2],
    /// L'étendue dans la texture, de 0 à 1.
    pub uv_taille: [f32; 2],
    /// La couleur, prémultipliée par son opacité, de 0 à 1.
    pub couleur: [f32; 4],
}

impl Quad {
    /// Ses attributs, aux décalages de la structure : emplacements 0 à 4.
    pub const ATTRIBUTS: [VertexAttribute; 5] = [
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: offset_of!(Quad, position) as u64,
            shader_location: 0,
        },
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: offset_of!(Quad, taille) as u64,
            shader_location: 1,
        },
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: offset_of!(Quad, uv_origine) as u64,
            shader_location: 2,
        },
        VertexAttribute {
            format: VertexFormat::Float32x2,
            offset: offset_of!(Quad, uv_taille) as u64,
            shader_location: 3,
        },
        VertexAttribute {
            format: VertexFormat::Float32x4,
            offset: offset_of!(Quad, couleur) as u64,
            shader_location: 4,
        },
    ];

    /// Le pas d'un quad au suivant, dans le tampon des instances.
    pub const PAS: u64 = size_of::<Quad>() as u64;

    /// Ajoute ses octets à `o`, champ par champ à leurs décalages.
    pub fn ecrire(&self, o: &mut Vec<u8>) {
        let debut = o.len();
        o.resize(debut + size_of::<Quad>(), 0);
        let champs: [(usize, &[f32]); 5] = [
            (offset_of!(Quad, position), &self.position),
            (offset_of!(Quad, taille), &self.taille),
            (offset_of!(Quad, uv_origine), &self.uv_origine),
            (offset_of!(Quad, uv_taille), &self.uv_taille),
            (offset_of!(Quad, couleur), &self.couleur),
        ];
        ecrire_champs(o, debut, &champs);
    }
}

/// L'uniforme des quads : la taille de la cible, en pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cible {
    /// La largeur et la hauteur de la cible, en pixels.
    pub taille: [f32; 2],
    /// Le bourrage : un uniforme fait un multiple de 16 octets.
    pub bourrage: [f32; 2],
}

impl Cible {
    /// Ses octets, champ par champ à leurs décalages.
    pub fn octets(&self) -> Vec<u8> {
        let mut o = vec![0; size_of::<Cible>()];
        let champs: [(usize, &[f32]); 2] = [
            (offset_of!(Cible, taille), &self.taille),
            (offset_of!(Cible, bourrage), &self.bourrage),
        ];
        ecrire_champs(&mut o, 0, &champs);
        o
    }
}

/// Écrit chaque flottant de chaque champ, petit-boutiste (l'ordre du GPU, de WebGPU et
/// de WebGL2), à partir de `debut + décalage`.
fn ecrire_champs(o: &mut [u8], debut: usize, champs: &[(usize, &[f32])]) {
    for (decalage, valeurs) in champs {
        for (k, v) in valeurs.iter().enumerate() {
            let a = debut + decalage + 4 * k;
            if let Some(case) = o.get_mut(a..a + 4) {
                case.copy_from_slice(&v.to_le_bytes());
            }
        }
    }
}
