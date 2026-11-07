//! Les données envoyées au GPU partagent leur définition avec ses programmes (CONTRAT,
//! « Les types de bout en bout ») : le WGSL des quads, lu par `naga`, a les mêmes
//! tailles, décalages, emplacements et types que les structures `#[repr(C)]` de
//! `rendu::gpu`.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    clippy::cast_possible_truncation,
    clippy::arithmetic_side_effects,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

use std::mem::{offset_of, size_of};

use naga::{Binding, ScalarKind, TypeInner, VectorSize};
use rendu::gpu::{Cible, QUADS_WGSL, Quad};
use rendu::wgpu::VertexFormat;

fn module() -> naga::Module {
    naga::front::wgsl::parse_str(QUADS_WGSL).expect("le WGSL des quads se lit")
}

/// Le type naga d'un format de sommet : (composantes, sorte, octets par composante).
fn forme_du_format(f: VertexFormat) -> (u8, ScalarKind, u8) {
    match f {
        VertexFormat::Float32x2 => (2, ScalarKind::Float, 4),
        VertexFormat::Float32x4 => (4, ScalarKind::Float, 4),
        autre => panic!("format non prévu par le test : {autre:?}"),
    }
}

#[test]
fn l_uniforme_de_la_cible_a_la_forme_de_sa_structure() {
    let m = module();
    let mut layouter = naga::proc::Layouter::default();
    layouter.update(m.to_ctx()).unwrap();
    let (h, ty) = m
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Cible"))
        .expect("le WGSL déclare Cible");
    let TypeInner::Struct { members, span } = &ty.inner else {
        panic!("Cible n'est pas une structure");
    };
    assert_eq!(*span as usize, size_of::<Cible>());
    assert_eq!(layouter[h].size as usize, size_of::<Cible>());
    let attendus = [
        ("taille", offset_of!(Cible, taille)),
        ("bourrage", offset_of!(Cible, bourrage)),
    ];
    assert_eq!(members.len(), attendus.len());
    for (membre, (nom, decalage)) in members.iter().zip(attendus) {
        assert_eq!(membre.name.as_deref(), Some(nom));
        assert_eq!(membre.offset as usize, decalage, "{nom}");
        assert_eq!(layouter[membre.ty].size, 8, "{nom} : un vec2<f32>");
    }
}

#[test]
fn chaque_attribut_d_un_quad_a_son_emplacement_et_son_type() {
    let m = module();
    let sommet = m
        .entry_points
        .iter()
        .find(|e| e.name == "sommet")
        .expect("le WGSL a son point d'entrée sommet");
    // Le quad est le second argument, une structure d'entrée.
    let quad = sommet
        .function
        .arguments
        .iter()
        .find_map(|a| match &m.types[a.ty].inner {
            TypeInner::Struct { members, .. } if m.types[a.ty].name.as_deref() == Some("Quad") => {
                Some(members.clone())
            }
            _ => None,
        })
        .expect("sommet lit un Quad");
    let noms = ["position", "taille", "uv_origine", "uv_taille", "couleur"];
    assert_eq!(quad.len(), Quad::ATTRIBUTS.len());
    assert_eq!(noms.len(), Quad::ATTRIBUTS.len());
    let decalages = [
        offset_of!(Quad, position),
        offset_of!(Quad, taille),
        offset_of!(Quad, uv_origine),
        offset_of!(Quad, uv_taille),
        offset_of!(Quad, couleur),
    ];
    for (((membre, attribut), nom), decalage) in
        quad.iter().zip(&Quad::ATTRIBUTS).zip(noms).zip(decalages)
    {
        assert_eq!(membre.name.as_deref(), Some(nom));
        let Some(Binding::Location { location, .. }) = membre.binding else {
            panic!("{nom} n'a pas d'emplacement");
        };
        assert_eq!(location, attribut.shader_location, "{nom}");
        assert_eq!(attribut.offset as usize, decalage, "{nom}");
        let TypeInner::Vector { size, scalar } = m.types[membre.ty].inner else {
            panic!("{nom} n'est pas un vecteur");
        };
        let composantes = match size {
            VectorSize::Bi => 2,
            VectorSize::Tri => 3,
            VectorSize::Quad => 4,
        };
        assert_eq!(
            (composantes, scalar.kind, scalar.width),
            forme_du_format(attribut.format),
            "{nom}"
        );
    }
    // Le pas des instances est la taille de la structure, sans trou à la fin.
    assert_eq!(Quad::PAS as usize, size_of::<Quad>());
    let fin = Quad::ATTRIBUTS
        .last()
        .map(|a| a.offset as usize + 16)
        .unwrap();
    assert_eq!(fin, size_of::<Quad>());
}

#[test]
fn les_octets_d_un_quad_sont_a_leurs_decalages() {
    let q = Quad {
        position: [1.0, 2.0],
        taille: [3.0, 4.0],
        uv_origine: [5.0, 6.0],
        uv_taille: [7.0, 8.0],
        couleur: [9.0, 10.0, 11.0, 12.0],
    };
    let mut o = Vec::new();
    q.ecrire(&mut o);
    assert_eq!(o.len(), size_of::<Quad>());
    let lu = |d: usize| f32::from_le_bytes(o[d..d + 4].try_into().unwrap());
    assert_eq!(lu(offset_of!(Quad, uv_taille) + 4), 8.0);
    assert_eq!(lu(offset_of!(Quad, couleur) + 12), 12.0);
    let c = Cible {
        taille: [640.0, 360.0],
        bourrage: [0.0, 0.0],
    }
    .octets();
    assert_eq!(c.len(), size_of::<Cible>());
    assert_eq!(f32::from_le_bytes(c[4..8].try_into().unwrap()), 360.0);
}
