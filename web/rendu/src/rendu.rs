//! Le rendu sur le GPU : la scène mise à plat ([`crate::liste`]) dessinée dans une
//! texture, que la télé copie ensuite sur sa toile et que les tests relisent (PHASE-3,
//! § 2.9, lot 9).
//!
//! - **Une cible à soi.** Le rendu dessine dans sa propre texture, [`FORMAT`], de la
//!   taille du cadre ; la page la copie sur la toile (`wgpu::util::TextureBlitter`). Les
//!   pixels relus sont ceux de cette texture, en natif (Metal) comme dans le navigateur
//!   (WebGPU, WebGL2).
//! - **Les quads** (rectangles, images, modules du QR) : un seul programme
//!   ([`crate::gpu::QUADS_WGSL`]), une couleur prémultipliée, mélangée par « source plus
//!   destination fois (1 − alpha) ». Un rectangle est l'image d'un pixel blanc.
//! - **Les textes** : pour chaque texte, l'agencement de `mise-en-page` et le tampon
//!   qu'elle en donne ([`mise_en_page::MiseEnPage::tampon`]), dessinés par `glyphon` dans
//!   les mêmes polices : la télé dessine exactement ce que l'épreuve mesure, et ne coupe
//!   jamais elle-même.
//! - **Les couches** : dans une couche, les quads puis les textes ; une couche après
//!   l'autre, pour garder l'ordre du peintre (`liste::coucher`).
//! - **GPU perdu** : [`Rendu::recreer`] refait tout ce qui vit sur le GPU, sur un nouvel
//!   appareil, depuis ce que le rendu garde en mémoire (les polices, les images) ; la
//!   page redessine la dernière scène (CONTRAT, « Le GPU »).

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use glyphon::{
    Cache, Color, ColorMode, Resolution, SwashCache, TextArea, TextAtlas, TextBounds, TextRenderer,
    Viewport,
};
use jeu_abi::{Cle, Echantillonnage, Rgba, SceneFil};
use mise_en_page::{Agencement, MiseEnPage, Polices};
use wgpu::util::DeviceExt as _;

use crate::cadre::Cadre;
use crate::gpu::{Cible, QUADS_WGSL, Quad};
use crate::liste::{Contexte, Liste, Sorte, mettre_a_plat, premultipliee};

/// Le format de la texture où le rendu dessine : 8 bits par canal, sans conversion sRGB
/// (les couleurs de la scène s'y écrivent telles quelles, comme sur une toile du web).
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// La couleur d'une image citée que le rendu n'a pas (choisi : un gris moyen).
pub const IMAGE_ABSENTE: Rgba = Rgba(128, 128, 128, 255);

/// Une image en mémoire : RGBA de 8 bits, non prémultipliée, ligne par ligne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRgba {
    /// La largeur, en pixels.
    pub largeur: u32,
    /// La hauteur, en pixels.
    pub hauteur: u32,
    /// Les pixels.
    pub rgba: Vec<u8>,
}

impl ImageRgba {
    /// Le pixel `(x, y)`, s'il existe.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.largeur || y >= self.hauteur {
            return None;
        }
        let k =
            usize::try_from((u64::from(y) * u64::from(self.largeur) + u64::from(x)) * 4).ok()?;
        self.rgba.get(k..k + 4)?.try_into().ok()
    }
}

/// Ce qu'une image a dessiné.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dessin {
    /// Les nœuds de la scène.
    pub noeuds: u32,
    /// Les éléments dessinés.
    pub elements: u32,
    /// Les couches.
    pub couches: u32,
    /// Les textes dessinés.
    pub textes: u32,
    /// Les glyphes absents de toutes les polices, dessinés comme des boîtes : le
    /// diagnostic les compte (PHASE-3, § 2.9).
    pub glyphes_absents: u32,
    /// Les images citées que le rendu n'a pas, dessinées en gris.
    pub images_absentes: u32,
}

/// Un texte de la scène, tel que le rendu le dessine.
#[derive(Debug)]
pub struct TexteDessine {
    /// L'indice de son nœud.
    pub noeud: u32,
    /// Son agencement.
    pub agencement: Agencement,
    /// Le tampon que `glyphon` dessine.
    pub tampon: glyphon::Buffer,
    /// Le bord gauche du tampon, en pixels.
    pub gauche: f32,
    /// Le haut du tampon (celui de sa première ligne), en pixels.
    pub haut: f32,
    /// Des pixels par unité.
    pub echelle: f32,
    /// Sa couleur.
    pub couleur: Color,
    /// Sa couche.
    pub couche: u32,
}

/// Une image sur le GPU, et ses deux liaisons (lisse, au plus proche).
struct Texture {
    lisse: wgpu::BindGroup,
    proche: wgpu::BindGroup,
}

/// Tout ce qui vit sur le GPU : refait d'un bloc quand il est perdu.
struct Gpu {
    appareil: wgpu::Device,
    file: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    disposition_image: wgpu::BindGroupLayout,
    uniforme: wgpu::Buffer,
    liaison_cible: wgpu::BindGroup,
    echantillonneur_lisse: wgpu::Sampler,
    echantillonneur_proche: wgpu::Sampler,
    blanc: Texture,
    images: BTreeMap<u64, Texture>,
    instances: Option<(wgpu::Buffer, u64)>,
    cible: Option<(wgpu::Texture, wgpu::TextureView, u32, u32)>,
    cache: Cache,
    atlas: TextAtlas,
    vue: Viewport,
    textes: Vec<TextRenderer>,
}

/// Le programme des quads, et ses deux dispositions de liaisons (la cible, l'image).
fn programme(
    appareil: &wgpu::Device,
) -> (
    wgpu::BindGroupLayout,
    wgpu::BindGroupLayout,
    wgpu::RenderPipeline,
) {
    let module = appareil.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("rendu : quads"),
        source: wgpu::ShaderSource::Wgsl(QUADS_WGSL.into()),
    });
    let disposition_cible = appareil.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rendu : cible"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }],
    });
    let disposition_image = appareil.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("rendu : image"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let disposition = appareil.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rendu : quads"),
        bind_group_layouts: &[Some(&disposition_cible), Some(&disposition_image)],
        immediate_size: 0,
    });
    let pipeline = appareil.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rendu : quads"),
        layout: Some(&disposition),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("sommet"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: Quad::PAS,
                step_mode: wgpu::VertexStepMode::Instance,
                attributes: &Quad::ATTRIBUTS,
            })],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleStrip,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fragment"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: FORMAT,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    (disposition_cible, disposition_image, pipeline)
}

impl Gpu {
    fn new(appareil: wgpu::Device, file: wgpu::Queue) -> Gpu {
        let (disposition_cible, disposition_image, pipeline) = programme(&appareil);
        let uniforme = appareil.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rendu : cible"),
            size: std::mem::size_of::<Cible>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let liaison_cible = appareil.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rendu : cible"),
            layout: &disposition_cible,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforme.as_entire_binding(),
            }],
        });
        let echantillonneur = |filtre| {
            appareil.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("rendu : échantillonneur"),
                mag_filter: filtre,
                min_filter: filtre,
                ..wgpu::SamplerDescriptor::default()
            })
        };
        let echantillonneur_lisse = echantillonneur(wgpu::FilterMode::Linear);
        let echantillonneur_proche = echantillonneur(wgpu::FilterMode::Nearest);
        let cache = Cache::new(&appareil);
        let mut atlas =
            TextAtlas::with_color_mode(&appareil, &file, &cache, FORMAT, ColorMode::Web);
        let vue = Viewport::new(&appareil, &cache);
        let premier = TextRenderer::new(
            &mut atlas,
            &appareil,
            wgpu::MultisampleState::default(),
            None,
        );
        let mut gpu = Gpu {
            blanc: Texture {
                lisse: liaison_cible.clone(),
                proche: liaison_cible.clone(),
            },
            appareil,
            file,
            pipeline,
            disposition_image,
            uniforme,
            liaison_cible,
            echantillonneur_lisse,
            echantillonneur_proche,
            images: BTreeMap::new(),
            instances: None,
            cible: None,
            cache,
            atlas,
            vue,
            textes: vec![premier],
        };
        gpu.blanc = gpu.texture(&ImageRgba {
            largeur: 1,
            hauteur: 1,
            rgba: vec![255; 4],
        });
        gpu
    }

    /// Une image sur le GPU, prémultipliée.
    fn texture(&self, image: &ImageRgba) -> Texture {
        let mut rgba = image.rgba.clone();
        for p in rgba.as_chunks_mut::<4>().0 {
            let a = u32::from(p[3]);
            let fois = |c: u8| u8::try_from((u32::from(c) * a + 127) / 255).unwrap_or(u8::MAX);
            *p = [fois(p[0]), fois(p[1]), fois(p[2]), p[3]];
        }
        let taille = wgpu::Extent3d {
            width: image.largeur.max(1),
            height: image.hauteur.max(1),
            depth_or_array_layers: 1,
        };
        let t = self.appareil.create_texture_with_data(
            &self.file,
            &wgpu::TextureDescriptor {
                label: Some("rendu : image"),
                size: taille,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &rgba,
        );
        let vue = t.create_view(&wgpu::TextureViewDescriptor::default());
        let liaison = |e: &wgpu::Sampler| {
            self.appareil.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rendu : image"),
                layout: &self.disposition_image,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&vue),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(e),
                    },
                ],
            })
        };
        Texture {
            lisse: liaison(&self.echantillonneur_lisse),
            proche: liaison(&self.echantillonneur_proche),
        }
    }

    /// La cible, à la taille voulue.
    fn cible(&mut self, largeur: u32, hauteur: u32) -> wgpu::TextureView {
        if let Some((_, v, l, h)) = &self.cible
            && *l == largeur
            && *h == hauteur
        {
            return v.clone();
        }
        let t = self.appareil.create_texture(&wgpu::TextureDescriptor {
            label: Some("rendu : cible"),
            size: wgpu::Extent3d {
                width: largeur.max(1),
                height: hauteur.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let v = t.create_view(&wgpu::TextureViewDescriptor::default());
        self.cible = Some((t, v.clone(), largeur, hauteur));
        v
    }

    /// Le tampon des instances, assez grand.
    fn instances(&mut self, octets: &[u8]) -> wgpu::Buffer {
        let taille = (octets.len() as u64).max(Quad::PAS);
        match &self.instances {
            Some((b, t)) if *t >= taille => {
                self.file.write_buffer(b, 0, octets);
                b.clone()
            }
            _ => {
                let t = taille.next_power_of_two();
                let b = self.appareil.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("rendu : quads"),
                    size: t,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.file.write_buffer(&b, 0, octets);
                self.instances = Some((b.clone(), t));
                b
            }
        }
    }
}

/// Un tirage de quads : une liaison d'image, des instances.
struct Tirage {
    couche: u32,
    image: Option<(u64, Echantillonnage)>,
    debut: u32,
    fin: u32,
}

/// Le rendu de la télé.
pub struct Rendu {
    gpu: Gpu,
    polices: Polices,
    mise_en_page: MiseEnPage,
    images: BTreeMap<u64, ImageRgba>,
    swash: SwashCache,
}

impl std::fmt::Debug for Rendu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rendu")
            .field("polices", &self.polices)
            .field("images", &self.images.len())
            .finish_non_exhaustive()
    }
}

impl Rendu {
    /// Un rendu sur cet appareil, avec ces polices (celle du commun, puis celles du jeu).
    pub fn new(appareil: wgpu::Device, file: wgpu::Queue, polices: Polices) -> Rendu {
        Rendu {
            gpu: Gpu::new(appareil, file),
            mise_en_page: MiseEnPage::new(&polices),
            polices,
            images: BTreeMap::new(),
            swash: SwashCache::new(),
        }
    }

    /// Refait tout ce qui vivait sur le GPU, sur un nouvel appareil : le GPU a été perdu
    /// (`device.lost`). Les polices et les images gardées en mémoire y reviennent ; la
    /// page redessine la dernière scène.
    pub fn recreer(&mut self, appareil: wgpu::Device, file: wgpu::Queue) {
        self.gpu = Gpu::new(appareil, file);
        let images: Vec<(u64, Texture)> = self
            .images
            .iter()
            .map(|(c, i)| (*c, self.gpu.texture(i)))
            .collect();
        self.gpu.images.extend(images);
    }

    /// Change de polices : la mise en page repart de zéro, et l'atlas des glyphes, dont
    /// les clés nomment les polices, est refait.
    pub fn poser_polices(&mut self, polices: Polices) {
        self.mise_en_page = MiseEnPage::new(&polices);
        self.polices = polices;
        let g = &mut self.gpu;
        g.atlas =
            TextAtlas::with_color_mode(&g.appareil, &g.file, &g.cache, FORMAT, ColorMode::Web);
        g.textes = vec![TextRenderer::new(
            &mut g.atlas,
            &g.appareil,
            wgpu::MultisampleState::default(),
            None,
        )];
    }

    /// Les polices du rendu.
    pub fn polices(&self) -> &Polices {
        &self.polices
    }

    /// Pose une image, nommée par sa clé.
    pub fn poser_image(&mut self, cle: Cle, image: ImageRgba) {
        let t = self.gpu.texture(&image);
        self.gpu.images.insert(cle.fil(), t);
        self.images.insert(cle.fil(), image);
    }

    /// Le rendu a-t-il cette image (posée, et décodée) ?
    pub fn a_l_image(&self, cle: Cle) -> bool {
        self.gpu.images.contains_key(&cle.fil())
    }

    /// Oublie toutes les images.
    pub fn oublier_images(&mut self) {
        self.images.clear();
        self.gpu.images.clear();
    }

    /// L'appareil du rendu.
    pub fn appareil(&self) -> &wgpu::Device {
        &self.gpu.appareil
    }

    /// La file du rendu.
    pub fn file(&self) -> &wgpu::Queue {
        &self.gpu.file
    }

    /// La texture de la dernière image dessinée, s'il y en a une.
    pub fn cible(&self) -> Option<&wgpu::Texture> {
        self.gpu.cible.as_ref().map(|(t, _, _, _)| t)
    }

    /// La mise en page du rendu : celle qui agence les textes qu'il dessine.
    pub fn mise_en_page(&mut self) -> &mut MiseEnPage {
        &mut self.mise_en_page
    }

    /// Les textes d'une scène mise à plat, agencés, avec le tampon que le rendu dessine.
    pub fn textes(&mut self, liste: &Liste) -> Vec<TexteDessine> {
        let mut sortie = Vec::new();
        for e in &liste.elements {
            let Sorte::Texte {
                texte,
                boite,
                style,
                echelle,
                opacite,
            } = &e.sorte
            else {
                continue;
            };
            let agencement = self.mise_en_page.agencer(texte, *boite, style);
            let haut0 = agencement.lignes.first().map_or(0.0, |l| l.haut);
            let tampon = self.mise_en_page.tampon(&agencement, *boite, style);
            let c = style.couleur;
            let a = (f32::from(c.3) * opacite).round().clamp(0.0, 255.0) as u8;
            sortie.push(TexteDessine {
                noeud: e.noeud,
                gauche: e.boite[0],
                haut: e.boite[1] + haut0 * echelle,
                echelle: *echelle,
                couleur: Color::rgba(c.0, c.1, c.2, a),
                couche: e.couche,
                agencement,
                tampon,
            });
        }
        sortie
    }

    /// Dessine une scène dans la cible, à la taille du cadre, et soumet le travail au GPU.
    pub fn dessiner(
        &mut self,
        scene: &SceneFil,
        cadre: &Cadre,
        contexte: &Contexte,
    ) -> Result<Dessin, String> {
        let liste = mettre_a_plat(scene, cadre, contexte);
        self.dessiner_liste(&liste, cadre)
    }

    /// Les quads d'une scène mise à plat, rangés par couche puis dans l'ordre, et leurs
    /// tirages, un par suite de quads d'une même liaison.
    fn quads(&self, liste: &Liste, dessin: &mut Dessin) -> (Vec<u8>, Vec<Tirage>) {
        let mut octets = Vec::new();
        let mut tirages: Vec<Tirage> = Vec::new();
        let mut n: u32 = 0;
        for c in 0..liste.couches {
            for e in liste.elements.iter().filter(|e| e.couche == c) {
                let Sorte::Quad { couleur, image } = &e.sorte else {
                    continue;
                };
                let (image, couleur) = match image {
                    Some((cle, ech)) if self.gpu.images.contains_key(&cle.fil()) => {
                        (Some((cle.fil(), *ech)), *couleur)
                    }
                    Some(_) => {
                        dessin.images_absentes += 1;
                        (None, premultipliee(IMAGE_ABSENTE, couleur[3]))
                    }
                    None => (None, *couleur),
                };
                Quad {
                    position: [e.boite[0], e.boite[1]],
                    taille: [e.boite[2] - e.boite[0], e.boite[3] - e.boite[1]],
                    uv_origine: [0.0, 0.0],
                    uv_taille: [1.0, 1.0],
                    couleur,
                }
                .ecrire(&mut octets);
                match tirages.last_mut() {
                    Some(t) if t.couche == c && t.image == image && t.fin == n => t.fin += 1,
                    _ => tirages.push(Tirage {
                        couche: c,
                        image,
                        debut: n,
                        fin: n + 1,
                    }),
                }
                n += 1;
            }
        }
        (octets, tirages)
    }

    /// Dessine une scène déjà mise à plat.
    pub fn dessiner_liste(&mut self, liste: &Liste, cadre: &Cadre) -> Result<Dessin, String> {
        let mut dessin = Dessin {
            noeuds: liste.noeuds,
            elements: u32::try_from(liste.elements.len()).unwrap_or(u32::MAX),
            couches: liste.couches,
            ..Dessin::default()
        };
        let (octets, tirages) = self.quads(liste, &mut dessin);
        let textes = self.textes(liste);
        dessin.textes = u32::try_from(textes.len()).unwrap_or(u32::MAX);
        dessin.glyphes_absents = textes.iter().map(|t| t.agencement.absents).sum();

        let vue = self.gpu.cible(cadre.largeur, cadre.hauteur);
        let instances = self.gpu.instances(&octets);
        self.gpu.file.write_buffer(
            &self.gpu.uniforme,
            0,
            &Cible {
                taille: [cadre.largeur.max(1) as f32, cadre.hauteur.max(1) as f32],
                bourrage: [0.0, 0.0],
            }
            .octets(),
        );
        let g = &mut self.gpu;
        g.vue.update(
            &g.file,
            Resolution {
                width: cadre.largeur.max(1),
                height: cadre.hauteur.max(1),
            },
        );
        while g.textes.len() < usize::try_from(liste.couches).unwrap_or(0) {
            let r = TextRenderer::new(
                &mut g.atlas,
                &g.appareil,
                wgpu::MultisampleState::default(),
                None,
            );
            g.textes.push(r);
        }
        for (c, r) in g.textes.iter_mut().enumerate() {
            let zones = textes
                .iter()
                .filter(|t| usize::try_from(t.couche).ok() == Some(c))
                .map(|t| TextArea {
                    buffer: &t.tampon,
                    left: t.gauche,
                    top: t.haut,
                    scale: t.echelle,
                    bounds: TextBounds::default(),
                    default_color: t.couleur,
                    custom_glyphs: &[],
                });
            r.prepare(
                &g.appareil,
                &g.file,
                self.mise_en_page.systeme(),
                &mut g.atlas,
                &g.vue,
                zones,
                &mut self.swash,
            )
            .map_err(|e| format!("glyphon : {e}"))?;
        }
        let encodeur = encoder(g, liste, &vue, &instances, &tirages)?;
        g.file.submit([encodeur.finish()]);
        g.atlas.trim();
        Ok(dessin)
    }

    /// Appelle `rappel` quand le GPU a fini le travail soumis jusqu'ici
    /// (`onSubmittedWorkDone`) : pas qu'une image a été présentée (PHASE-3, § 2.9).
    pub fn quand_fini(&self, rappel: impl FnOnce() + Send + 'static) {
        self.gpu.file.on_submitted_work_done(rappel);
    }

    /// Lance la relecture de la cible : la copie vers un tampon, puis sa lecture.
    /// L'appelant fait tourner l'appareil (`poll`) jusqu'à ce que [`Relecture::prete`]
    /// le dise.
    pub fn relire(&self) -> Result<Relecture, String> {
        let (texture, _, largeur, hauteur) = self
            .gpu
            .cible
            .as_ref()
            .ok_or_else(|| String::from("rien n'a encore été dessiné"))?;
        let par_ligne = (largeur * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let tampon = self.gpu.appareil.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rendu : relecture"),
            size: u64::from(par_ligne) * u64::from(*hauteur),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encodeur =
            self.gpu
                .appareil
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("rendu : relecture"),
                });
        encodeur.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &tampon,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(par_ligne),
                    rows_per_image: Some(*hauteur),
                },
            },
            wgpu::Extent3d {
                width: *largeur,
                height: *hauteur,
                depth_or_array_layers: 1,
            },
        );
        self.gpu.file.submit([encodeur.finish()]);
        let etat: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
        let e = Arc::clone(&etat);
        tampon.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            if let Ok(mut x) = e.lock() {
                *x = Some(r.map_err(|e| e.to_string()));
            }
        });
        Ok(Relecture {
            tampon,
            largeur: *largeur,
            hauteur: *hauteur,
            par_ligne,
            etat,
        })
    }
}

/// Encode la passe d'une image : le fond, puis chaque couche, ses quads puis ses textes.
fn encoder(
    g: &Gpu,
    liste: &Liste,
    vue: &wgpu::TextureView,
    instances: &wgpu::Buffer,
    tirages: &[Tirage],
) -> Result<wgpu::CommandEncoder, String> {
    let fond = liste.fond;
    let mut encodeur = g
        .appareil
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("rendu : image"),
        });
    {
        let mut passe = encodeur.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rendu : image"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: vue,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: f64::from(fond.0) / 255.0,
                        g: f64::from(fond.1) / 255.0,
                        b: f64::from(fond.2) / 255.0,
                        a: f64::from(fond.3) / 255.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        for c in 0..liste.couches {
            passe.set_pipeline(&g.pipeline);
            passe.set_bind_group(0, &g.liaison_cible, &[]);
            passe.set_vertex_buffer(0, instances.slice(..));
            for t in tirages.iter().filter(|t| t.couche == c) {
                let liaison = match t.image {
                    Some((cle, Echantillonnage::Lisse)) => g.images.get(&cle).map(|x| &x.lisse),
                    Some((cle, Echantillonnage::PlusProche)) => {
                        g.images.get(&cle).map(|x| &x.proche)
                    }
                    None => Some(&g.blanc.proche),
                };
                if let Some(l) = liaison {
                    passe.set_bind_group(1, l, &[]);
                    passe.draw(0..4, t.debut..t.fin);
                }
            }
            if let Some(r) = g.textes.get(usize::try_from(c).unwrap_or(usize::MAX)) {
                r.render(&g.atlas, &g.vue, &mut passe)
                    .map_err(|e| format!("glyphon : {e}"))?;
            }
        }
    }
    Ok(encodeur)
}

/// Une relecture de la cible en cours.
#[derive(Debug)]
pub struct Relecture {
    tampon: wgpu::Buffer,
    largeur: u32,
    hauteur: u32,
    par_ligne: u32,
    etat: Arc<Mutex<Option<Result<(), String>>>>,
}

impl Relecture {
    /// La lecture est-elle finie, et comment ?
    pub fn prete(&self) -> Option<Result<(), String>> {
        self.etat.lock().ok().and_then(|x| x.clone())
    }

    /// L'image relue ; la lecture doit être finie.
    pub fn image(self) -> Result<ImageRgba, String> {
        match self.prete() {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(format!("relecture : {e}")),
            None => return Err(String::from("relecture : pas finie")),
        }
        let mut rgba = Vec::with_capacity(
            usize::try_from(u64::from(self.largeur) * u64::from(self.hauteur) * 4).unwrap_or(0),
        );
        {
            let vue = self
                .tampon
                .slice(..)
                .get_mapped_range()
                .map_err(|e| format!("relecture : {e}"))?;
            let ligne = usize::try_from(self.largeur * 4).unwrap_or(0);
            let pas = usize::try_from(self.par_ligne).unwrap_or(0);
            for y in 0..usize::try_from(self.hauteur).unwrap_or(0) {
                let debut = y * pas;
                rgba.extend_from_slice(vue.get(debut..debut + ligne).unwrap_or(&[]));
            }
        }
        self.tampon.unmap();
        Ok(ImageRgba {
            largeur: self.largeur,
            hauteur: self.hauteur,
            rgba,
        })
    }
}
