// Les quads du rendu de la télé (PHASE-3, § 2.9) : rectangles, images, modules du QR.
// Chaque quad est une instance ; ses quatre sommets forment une bande de deux triangles.
// Les structures ont leur double Rust dans `gpu.rs` (`Quad`, `Cible`), comparé par
// `tests/wgsl.rs` (CONTRAT, « Les types de bout en bout »).

struct Cible {
    taille: vec2<f32>,
    bourrage: vec2<f32>,
};

@group(0) @binding(0) var<uniform> cible: Cible;
@group(1) @binding(0) var image: texture_2d<f32>;
@group(1) @binding(1) var echantillonneur: sampler;

struct Quad {
    @location(0) position: vec2<f32>,
    @location(1) taille: vec2<f32>,
    @location(2) uv_origine: vec2<f32>,
    @location(3) uv_taille: vec2<f32>,
    @location(4) couleur: vec4<f32>,
};

struct Sortie {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) couleur: vec4<f32>,
};

@vertex
fn sommet(@builtin(vertex_index) i: u32, q: Quad) -> Sortie {
    // (0, 0), (1, 0), (0, 1), (1, 1) : la bande du quad.
    let coin = vec2<f32>(f32(i & 1u), f32((i >> 1u) & 1u));
    let p = q.position + coin * q.taille;
    var s: Sortie;
    s.position = vec4<f32>(
        p.x / cible.taille.x * 2.0 - 1.0,
        1.0 - p.y / cible.taille.y * 2.0,
        0.0,
        1.0,
    );
    s.uv = q.uv_origine + coin * q.uv_taille;
    s.couleur = q.couleur;
    return s;
}

@fragment
fn fragment(s: Sortie) -> @location(0) vec4<f32> {
    // La texture est prémultipliée, la couleur aussi : le produit l'est.
    return textureSample(image, echantillonneur, s.uv) * s.couleur;
}
