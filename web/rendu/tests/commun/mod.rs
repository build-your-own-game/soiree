//! Ce que les tests du rendu partagent : un appareil Metal, et la relecture des pixels.

#![expect(
    clippy::expect_used,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

use jeu_abi::{Cle, Fixe, FormeEcran, NoeudFil, Oui, Rect, Rgba, SceneFil, SorteNoeud};
use mise_en_page::Polices;
use rendu::wgpu;
use rendu::{Cadre, Contexte, ImageRgba, Rendu};

/// Un appareil sur le GPU de la machine (Metal).
pub fn appareil() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adaptateur =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("un adaptateur Metal");
    pollster::block_on(adaptateur.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("un appareil Metal")
}

/// Un rendu neuf, avec la police du commun.
pub fn rendu() -> Rendu {
    let (a, f) = appareil();
    Rendu::new(a, f, Polices::du_commun())
}

/// Relit la cible, en attendant le GPU.
pub fn relire(r: &Rendu) -> ImageRgba {
    let relecture = r.relire().expect("une cible à relire");
    while relecture.prete().is_none() {
        r.appareil()
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("poll");
    }
    relecture.image().expect("l'image relue")
}

/// Dessine et relit.
pub fn dessiner(r: &mut Rendu, scene: &SceneFil, cadre: &Cadre, contexte: &Contexte) -> ImageRgba {
    r.dessiner(scene, cadre, contexte).expect("le dessin");
    relire(r)
}

/// Une scène de `l` × `h` unités, sans zone sûre ni échelle entière.
pub fn forme(l: u16, h: u16) -> FormeEcran {
    FormeEcran::Scene {
        largeur: l,
        hauteur: h,
        zone_sure_pour_cent: 100,
        echelle_entiere: Oui::Non,
    }
}

/// Une boîte de `x`, `y`, `l`, `h` unités.
pub fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

/// Un nœud simple de clé `k`.
pub fn noeud(k: u64, parent: Option<u32>, sorte: SorteNoeud) -> NoeudFil {
    NoeudFil::simple(Cle::du_fil(k), parent, sorte)
}

/// Une scène sur ce fond.
pub fn scene(fond: Rgba, noeuds: Vec<NoeudFil>) -> SceneFil {
    SceneFil {
        fond: Some(fond),
        noeuds,
        ..SceneFil::vide()
    }
}

/// Deux pixels égaux à `tolerance` près par canal.
pub fn proches(a: [u8; 4], b: [u8; 4], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}
