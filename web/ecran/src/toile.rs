//! La toile de la télé, dans le navigateur (PHASE-3, § 2.9, lot 9) : `wgpu` sur WebGPU
//! quand le navigateur l'expose, WebGL2 sinon ou quand on le force ; le rendu dessine dans
//! sa texture, copiée sur la toile ; le travail du GPU fini se compte
//! (`onSubmittedWorkDone`, OUT-33), jamais les tours de `requestAnimationFrame` ; quand
//! le GPU est perdu (`device.lost`), l'appareil et les atlas se refont et la dernière
//! scène se redessine (CONTRAT, « Le GPU »).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use jeu_abi::{Cle, FormeEcran, Oui, Pas, SceneFil};
use js_sys::{Function, Promise, Uint8Array};
use mise_en_page::Polices;
use rendu::wgpu;
use rendu::{Cadre, Contexte, Dessin, ImageRgba, Relecture, Rendu};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::HtmlCanvasElement;

use crate::messages::{Recu, recevoir};

/// Le moteur de dessin voulu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChoixMoteur {
    /// WebGPU si le navigateur en donne un adaptateur, WebGL2 sinon.
    Auto,
    /// WebGL2, même si WebGPU est là (`?moteur=webgl2` dans la page du banc).
    WebGl2,
}

/// L'horloge de la page, en millisecondes (`performance.now()`), ou 0 sans elle.
pub fn maintenant_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}

/// Une valeur levée par la page, en texte : une chaîne telle quelle ; une erreur, par son
/// nom et son message (une `DOMException`, que `JSON.stringify` rend `{}`, perdait sa
/// cause) ; autre chose, par son JSON.
pub fn texte_d_erreur(e: &JsValue) -> String {
    if let Some(s) = e.as_string() {
        return s;
    }
    let champ = |c: &str| {
        js_sys::Reflect::get(e, &JsValue::from_str(c))
            .ok()
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
    };
    match (champ("name"), champ("message")) {
        (Some(nom), Some(message)) => format!("{nom} : {message}"),
        (Some(seul), None) | (None, Some(seul)) => seul,
        (None, None) => js_sys::JSON::stringify(e)
            .ok()
            .and_then(|s| s.as_string())
            .unwrap_or_else(|| String::from("?")),
    }
}

fn texte(e: &JsValue) -> String {
    texte_d_erreur(e)
}

/// Fait tourner l'appareil jusqu'à `fini`, en rendant la main à la page entre deux tours
/// par `ceder` (une fonction de la page qui rend une `Promise`), au plus `delai_ms`.
pub async fn attendre(
    appareil: &wgpu::Device,
    fini: &dyn Fn() -> bool,
    ceder: &Function,
    delai_ms: f64,
) -> Result<(), String> {
    let debut = maintenant_ms();
    loop {
        let _: Result<wgpu::PollStatus, wgpu::PollError> = appareil.poll(wgpu::PollType::Poll);
        if fini() {
            return Ok(());
        }
        if maintenant_ms() - debut > delai_ms {
            return Err(format!("le GPU n'a pas répondu en {delai_ms} ms"));
        }
        let p: Promise = ceder
            .call0(&JsValue::NULL)
            .map_err(|e| texte(&e))?
            .dyn_into()
            .map_err(|_| String::from("ceder ne rend pas une Promise"))?;
        JsFuture::from(p).await.map_err(|e| texte(&e))?;
    }
}

/// Un appareil sur la toile : surface, adaptateur, appareil, file.
struct Gpu {
    surface: wgpu::Surface<'static>,
    adaptateur: wgpu::Adapter,
    appareil: wgpu::Device,
    file: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    vue: wgpu::TextureFormat,
}

/// Les erreurs du GPU que rien n'a attrapées : gardées, et dites, au lieu de paniquer.
type Erreurs = Arc<Mutex<Vec<String>>>;

async fn ouvrir(
    instance: &wgpu::Instance,
    toile: &HtmlCanvasElement,
    perdu: &Arc<AtomicBool>,
    erreurs: &Erreurs,
) -> Result<Gpu, String> {
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(toile.clone()))
        .map_err(|e| format!("surface : {e}"))?;
    let adaptateur = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..wgpu::RequestAdapterOptions::default()
        })
        .await
        .map_err(|e| format!("adaptateur : {e}"))?;
    let (appareil, file) = adaptateur
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("télé"),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adaptateur.limits()),
            ..wgpu::DeviceDescriptor::default()
        })
        .await
        .map_err(|e| format!("appareil : {e}"))?;
    let p = Arc::clone(perdu);
    appareil.set_device_lost_callback(move |_, _| p.store(true, Ordering::SeqCst));
    let e = Arc::clone(erreurs);
    appareil.on_uncaptured_error(Arc::new(move |err: wgpu::Error| {
        if let Ok(mut v) = e.lock() {
            v.push(err.to_string());
        }
    }));
    let (l, h) = (toile.width().max(1), toile.height().max(1));
    let mut config = surface
        .get_default_config(&adaptateur, l, h)
        .ok_or_else(|| String::from("la surface ne se configure pas"))?;
    // La texture du rendu porte des couleurs du web, sans conversion : la toile aussi.
    // Un format sans sRGB si la toile en offre un (WebGPU, WebGL2) ; sinon une vue sans.
    let formats = surface.get_capabilities(&adaptateur).formats;
    if let Some(f) = formats.iter().find(|f| !f.is_srgb()) {
        config.format = *f;
    }
    let vue = config.format.remove_srgb_suffix();
    config.view_formats = if vue == config.format {
        Vec::new()
    } else {
        vec![vue]
    };
    config.present_mode = wgpu::PresentMode::Fifo;
    surface.configure(&appareil, &config);
    Ok(Gpu {
        surface,
        adaptateur,
        appareil,
        file,
        config,
        vue,
    })
}

/// La télé : sa toile, son appareil, son rendu, la dernière scène recue.
pub struct Ecran {
    toile: HtmlCanvasElement,
    instance: wgpu::Instance,
    appareil: Gpu,
    copieur: wgpu::util::TextureBlitter,
    rendu: Rendu,
    polices: Polices,
    scene: Option<SceneFil>,
    forme: FormeEcran,
    /// Ce que le rendu sait du moment (pas, animations, adresse du QR).
    pub contexte: Contexte,
    perdu: Arc<AtomicBool>,
    erreurs: Erreurs,
    illisibles: Vec<String>,
    /// Pour `FormeEcran::Page` : des pixels par unité (le rapport de pixels de
    /// l'appareil, lot 11).
    pub echelle_page: f32,
    /// Pour `FormeEcran::Page` : les unités défilées en haut (le défilement du
    /// navigateur, lot 11).
    pub defilement: f32,
}

impl Ecran {
    /// Ouvre la télé sur une toile.
    pub async fn creer(toile: HtmlCanvasElement, choix: ChoixMoteur) -> Result<Ecran, String> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = match choix {
            ChoixMoteur::Auto => wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL,
            ChoixMoteur::WebGl2 => wgpu::Backends::GL,
        };
        let instance = wgpu::util::new_instance_with_webgpu_detection(desc).await;
        let perdu = Arc::new(AtomicBool::new(false));
        let erreurs: Erreurs = Arc::new(Mutex::new(Vec::new()));
        let appareil = ouvrir(&instance, &toile, &perdu, &erreurs).await?;
        let copieur = wgpu::util::TextureBlitter::new(&appareil.appareil, appareil.vue);
        let polices = Polices::du_commun();
        let rendu = Rendu::new(
            appareil.appareil.clone(),
            appareil.file.clone(),
            polices.clone(),
        );
        Ok(Ecran {
            toile,
            instance,
            appareil,
            copieur,
            rendu,
            polices,
            scene: None,
            forme: FormeEcran::Scene {
                largeur: 1920,
                hauteur: 1080,
                zone_sure_pour_cent: 90,
                echelle_entiere: Oui::Non,
            },
            contexte: Contexte::au_repos(),
            perdu,
            erreurs,
            illisibles: Vec::new(),
            echelle_page: 1.0,
            defilement: 0.0,
        })
    }

    /// Une image de démarrage, hors jeu : un rectangle et un texte, dessinés et attendus,
    /// pour que les programmes du GPU soient compilés avant la première image d'un jeu
    /// (WebGL2 les compile à leur premier usage : 85 ms constatés à la première image dans
    /// Chrome, lot 9). La scène recue, s'il y en a une, reste la dernière.
    pub async fn chauffer(&mut self, ceder: &Function) -> Result<(), String> {
        let style = jeu_abi::StyleTexte {
            police: Cle::POLICE_DU_COMMUN,
            taille: jeu_abi::Taille::Fixe(jeu_abi::Fixe::unites(16)),
            couleur: jeu_abi::Rgba::BLANC,
            alignement: jeu_abi::Alignement::Debut,
            lignes: jeu_abi::Lignes::Une,
            coupe: jeu_abi::Coupe::Mots,
        };
        let boite = jeu_abi::Rect {
            x: jeu_abi::Fixe::ZERO,
            y: jeu_abi::Fixe::ZERO,
            l: jeu_abi::Fixe::unites(64),
            h: jeu_abi::Fixe::unites(32),
        };
        let demarrage = SceneFil {
            fond: Some(jeu_abi::Rgba::NOIR),
            noeuds: vec![
                jeu_abi::NoeudFil::simple(
                    Cle::du_fil(1),
                    None,
                    jeu_abi::SorteNoeud::Rect {
                        r: boite,
                        couleur: jeu_abi::Rgba(40, 40, 40, 255),
                    },
                ),
                jeu_abi::NoeudFil::simple(
                    Cle::du_fil(2),
                    None,
                    jeu_abi::SorteNoeud::Texte {
                        texte: String::from("0"),
                        boite,
                        style,
                    },
                ),
            ],
            ..SceneFil::vide()
        };
        let recue = self.scene.replace(demarrage);
        let dessin = self.dessiner();
        let fin = self.quand_fini();
        let f = Arc::clone(&fin);
        let attendu = attendre(
            self.appareil(),
            &move || f.load(Ordering::SeqCst) != 0,
            ceder,
            20_000.0,
        )
        .await;
        self.scene = recue;
        dessin.map(|_| ())?;
        attendu
    }

    /// Le moteur de dessin et l'adaptateur, tels que `wgpu` les dit.
    pub fn moteur(&self) -> String {
        let i = self.appareil.adaptateur.get_info();
        format!(
            "{:?} ; {} ; {} ; {}",
            i.backend, i.name, i.driver, i.driver_info
        )
    }

    /// Le moteur seul : `webgpu` ou `webgl2`.
    pub fn sorte_de_moteur(&self) -> &'static str {
        match self.appareil.adaptateur.get_info().backend {
            wgpu::Backend::BrowserWebGpu => "webgpu",
            wgpu::Backend::Gl => "webgl2",
            wgpu::Backend::Noop
            | wgpu::Backend::Vulkan
            | wgpu::Backend::Metal
            | wgpu::Backend::Dx12 => "autre",
        }
    }

    /// Passe à un autre jeu : la police du commun seule, aucune image, aucune scène, et
    /// la forme de son écran.
    pub fn nouveau_jeu(&mut self, forme: FormeEcran) {
        self.polices = Polices::du_commun();
        self.rendu.poser_polices(self.polices.clone());
        self.rendu.oublier_images();
        self.scene = None;
        self.forme = forme;
    }

    /// Les erreurs du GPU que rien n'a attrapées depuis la dernière fois.
    pub fn erreurs_gpu(&self) -> Vec<String> {
        self.erreurs
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }

    /// Les images que le navigateur n'a pas su décoder, depuis la dernière fois.
    pub fn images_illisibles(&mut self) -> Vec<String> {
        std::mem::take(&mut self.illisibles)
    }

    /// Change la forme de l'écran, celle que le jeu déclare.
    pub fn regler_forme(&mut self, forme: FormeEcran) {
        self.forme = forme;
    }

    /// Le cadre : la toile, à sa taille en pixels, la forme, et pour une page, l'échelle
    /// et le défilement.
    pub fn cadre(&self) -> Cadre {
        Cadre {
            echelle_page: self.echelle_page,
            defilement: self.defilement,
            ..Cadre::new(self.toile.width(), self.toile.height(), self.forme)
        }
    }

    /// Le cadre de la page à son haut, sans défilement : les boîtes des éléments du DOM
    /// posés sur la toile, en pixels du document fois le rapport de pixels (lot 11).
    pub fn cadre_fixe(&self) -> Cadre {
        Cadre {
            defilement: 0.0,
            ..self.cadre()
        }
    }

    /// La forme de l'écran du jeu.
    pub fn forme(&self) -> FormeEcran {
        self.forme
    }

    /// Donne à la toile une taille en pixels (la fenêtre a changé, lot 11).
    pub fn regler_taille(&mut self, largeur: u32, hauteur: u32) {
        self.toile.set_width(largeur.max(1));
        self.toile.set_height(hauteur.max(1));
    }

    /// Le rendu a-t-il cette image (posée, et décodée) ?
    pub fn a_l_image(&self, cle: Cle) -> bool {
        self.rendu.a_l_image(cle)
    }

    /// Le plus grand côté d'une texture sur cet appareil : un rendu hors écran se dessine
    /// par bandes qui y tiennent (lot 11).
    pub fn cote_max(&self) -> u32 {
        self.appareil.appareil.limits().max_texture_dimension_2d
    }

    /// Le rendu.
    pub fn rendu(&mut self) -> &mut Rendu {
        &mut self.rendu
    }

    /// La dernière scène recue.
    pub fn scene(&self) -> Option<&SceneFil> {
        self.scene.as_ref()
    }

    /// Reçoit un message du Worker : une scène se garde (et son pas sert aux comptes à
    /// rebours), une police s'ajoute au rendu, une image se décode et se pose.
    pub async fn recevoir(&mut self, octets: &[u8]) -> Result<Recu, String> {
        let r = self.recevoir_sans_attendre(octets)?;
        if let Recu::Image { cle, octets } = &r {
            let image = decoder_image(octets).await;
            self.poser_image(*cle, image);
        }
        Ok(r)
    }

    /// [`Ecran::recevoir`], sans décoder une image : la page la décode sans tenir l'écran
    /// (le dessin continue pendant ce temps), puis la pose par [`Ecran::poser_image`]
    /// (lot 11).
    pub fn recevoir_sans_attendre(&mut self, octets: &[u8]) -> Result<Recu, String> {
        let r = recevoir(octets, &mut self.polices)?;
        match &r {
            Recu::Scene { pas, scene } => {
                self.contexte.pas = *pas;
                self.scene = Some(scene.clone());
            }
            Recu::Police(_) => self.rendu.poser_polices(self.polices.clone()),
            // Une image se décode hors d'ici (`recevoir`, ou la page sans tenir l'écran,
            // lot 11) ; un son se garde dans la page, qui le décode quand un effet le joue
            // (lot 10b).
            Recu::Image { .. } | Recu::Son { .. } | Recu::Laisse(_) => {}
        }
        Ok(r)
    }

    /// Pose une image décodée ; une image que le navigateur ne décode pas n'arrête rien :
    /// elle se dessine comme une image absente, et se dit.
    pub fn poser_image(&mut self, cle: Cle, image: Result<ImageRgba, String>) {
        match image {
            Ok(image) => self.rendu.poser_image(cle, image),
            Err(e) => self.illisibles.push(format!("{:016x} : {e}", cle.fil())),
        }
    }

    /// Dessine une scène dans la cible du rendu, hors de la toile, et lance sa relecture
    /// (une bande d'un rendu hors écran, la demande `Rendu`, lot 11). La toile n'est pas
    /// touchée ; sa prochaine image refait la cible à sa taille.
    pub fn dessiner_hors_ecran(
        &mut self,
        scene: &SceneFil,
        cadre: &Cadre,
    ) -> Result<Relecture, String> {
        let _: Dessin = self.rendu.dessiner(scene, cadre, &Contexte::au_repos())?;
        self.rendu.relire()
    }

    /// Dessine la dernière scène, la copie sur la toile et la présente. Rend ce qui a été
    /// dessiné ; le travail est soumis au GPU, pas fini.
    pub fn dessiner(&mut self) -> Result<Dessin, String> {
        let scene = self
            .scene
            .clone()
            .ok_or_else(|| String::from("aucune scène recue"))?;
        let cadre = self.cadre();
        if (self.appareil.config.width, self.appareil.config.height)
            != (cadre.largeur, cadre.hauteur)
        {
            self.appareil.config.width = cadre.largeur.max(1);
            self.appareil.config.height = cadre.hauteur.max(1);
            self.appareil
                .surface
                .configure(&self.appareil.appareil, &self.appareil.config);
        }
        let dessin = self.rendu.dessiner(&scene, &cadre, &self.contexte)?;
        let image = match self.appareil.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            autre @ (wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost
            | wgpu::CurrentSurfaceTexture::Validation) => {
                return Err(format!("la toile ne donne pas d'image : {autre:?}"));
            }
        };
        let vue = image.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.appareil.vue),
            ..wgpu::TextureViewDescriptor::default()
        });
        let source = self
            .rendu
            .cible()
            .ok_or_else(|| String::from("le rendu n'a pas de cible"))?
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encodeur =
            self.appareil
                .appareil
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("télé : copie sur la toile"),
                });
        self.copieur
            .copy(&self.appareil.appareil, &mut encodeur, &source, &vue);
        self.appareil.file.submit([encodeur.finish()]);
        self.appareil.file.present(image);
        Ok(dessin)
    }

    /// Dessine la dernière scène à `f` millièmes du glisser depuis `precedente` (§ 2.9,
    /// lot 13) : les nœuds qui glissent à mi-chemin, le reste tel quel. La dernière scène
    /// reste celle qu'on garde.
    pub fn dessiner_entre(&mut self, precedente: &SceneFil, f: u32) -> Result<Dessin, String> {
        let derniere = self
            .scene
            .as_ref()
            .ok_or_else(|| String::from("aucune scène recue"))?;
        let entre = rendu::glisser::entre(precedente, derniere, f);
        let derniere = self.scene.replace(entre);
        let r = self.dessiner();
        self.scene = derniere;
        r
    }

    /// Dessine une scène du commun (l'écran de démarrage, lot 10b) dans sa forme, à la place
    /// de celle du jeu, qui reste la dernière reçue et se redessinera ensuite.
    pub fn dessiner_le_commun(
        &mut self,
        scene: SceneFil,
        forme: FormeEcran,
    ) -> Result<Dessin, String> {
        let du_jeu = self.scene.replace(scene);
        let forme_du_jeu = core::mem::replace(&mut self.forme, forme);
        let pas = core::mem::replace(&mut self.contexte.pas, Pas::ZERO);
        let r = self.dessiner();
        self.scene = du_jeu;
        self.forme = forme_du_jeu;
        self.contexte.pas = pas;
        r
    }

    /// Le travail du GPU soumis jusqu'ici est-il fini ? Rend une case où l'heure de la
    /// fin (`performance.now()`, en bits de `f64`) s'écrira, 0 en attendant.
    pub fn quand_fini(&self) -> Arc<AtomicU64> {
        let fin = Arc::new(AtomicU64::new(0));
        let f = Arc::clone(&fin);
        self.rendu.quand_fini(move || {
            f.store(maintenant_ms().to_bits(), Ordering::SeqCst);
        });
        fin
    }

    /// L'appareil, pour le faire tourner.
    pub fn appareil(&self) -> &wgpu::Device {
        &self.appareil.appareil
    }

    /// Lance la relecture de la dernière image dessinée.
    pub fn relire(&self) -> Result<Relecture, String> {
        self.rendu.relire()
    }

    /// Détruit l'appareil (`device.destroy()`), comme un GPU perdu : le contrôle de la
    /// reprise (M8).
    pub fn detruire_appareil(&self) {
        self.appareil.appareil.destroy();
    }

    /// Le GPU a-t-il été perdu depuis la dernière reprise ?
    pub fn perdu(&self) -> bool {
        self.perdu.load(Ordering::SeqCst)
    }

    /// Refait l'appareil, la configuration de la toile, le copieur et tout ce que le rendu
    /// avait sur le GPU ; la page redessine ensuite la dernière scène.
    pub async fn reprendre(&mut self) -> Result<(), String> {
        let perdu = Arc::new(AtomicBool::new(false));
        let appareil = ouvrir(&self.instance, &self.toile, &perdu, &self.erreurs).await?;
        self.copieur = wgpu::util::TextureBlitter::new(&appareil.appareil, appareil.vue);
        self.rendu
            .recreer(appareil.appareil.clone(), appareil.file.clone());
        self.appareil = appareil;
        self.perdu = perdu;
        Ok(())
    }

    /// Le pas que le rendu prend pour les comptes à rebours.
    pub fn pas(&self) -> Pas {
        self.contexte.pas
    }
}

/// Ces octets sont-ils un SVG ? (`<svg` ou `<?xml` au début, après d'éventuels blancs
/// et une marque d'ordre des octets.)
pub fn est_un_svg(octets: &[u8]) -> bool {
    let debut: Vec<u8> = octets
        .iter()
        .skip_while(|c| matches!(c, b' ' | b'\t' | b'\r' | b'\n' | 0xef | 0xbb | 0xbf))
        .take(5)
        .copied()
        .collect();
    debut.starts_with(b"<svg") || debut.starts_with(b"<?xml")
}

/// Les pixels d'une source dessinée sur une toile hors écran : RGBA de 8 bits, non
/// prémultipliée.
fn pixels(
    l: u32,
    h: u32,
    dessiner: impl FnOnce(&web_sys::OffscreenCanvasRenderingContext2d) -> Result<(), JsValue>,
) -> Result<ImageRgba, String> {
    let hors = web_sys::OffscreenCanvas::new(l.max(1), h.max(1)).map_err(|e| texte(&e))?;
    let contexte: web_sys::OffscreenCanvasRenderingContext2d = hors
        .get_context("2d")
        .map_err(|e| texte(&e))?
        .ok_or_else(|| String::from("pas de contexte 2d"))?
        .dyn_into()
        .map_err(|_| String::from("contexte 2d inattendu"))?;
    dessiner(&contexte).map_err(|e| texte(&e))?;
    let donnees = contexte
        .get_image_data(0.0, 0.0, f64::from(l.max(1)), f64::from(h.max(1)))
        .map_err(|e| texte(&e))?;
    Ok(ImageRgba {
        largeur: l.max(1),
        hauteur: h.max(1),
        rgba: donnees.data().0,
    })
}

/// Décode une image par le navigateur (P23) : un SVG par un élément `<img>`, tout autre
/// format par `createImageBitmap` ; puis ses pixels.
pub async fn decoder_image(octets: &[u8]) -> Result<ImageRgba, String> {
    let fenetre = web_sys::window().ok_or_else(|| String::from("pas de fenêtre"))?;
    let tableau = js_sys::Array::of1(&Uint8Array::from(octets));
    if est_un_svg(octets) {
        let sorte = web_sys::BlobPropertyBag::new();
        sorte.set_type("image/svg+xml");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&tableau, &sorte)
            .map_err(|e| texte(&e))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| texte(&e))?;
        let img = web_sys::HtmlImageElement::new().map_err(|e| texte(&e))?;
        img.set_src(&url);
        let decode = JsFuture::from(img.decode()).await;
        let _: Result<(), JsValue> = web_sys::Url::revoke_object_url(&url);
        decode.map_err(|e| format!("SVG illisible : {}", texte(&e)))?;
        let (l, h) = (img.natural_width(), img.natural_height());
        return pixels(l, h, |c| {
            c.draw_image_with_html_image_element_and_dw_and_dh(
                &img,
                0.0,
                0.0,
                f64::from(l.max(1)),
                f64::from(h.max(1)),
            )
        });
    }
    let blob = web_sys::Blob::new_with_u8_array_sequence(&tableau).map_err(|e| texte(&e))?;
    let promesse = fenetre
        .create_image_bitmap_with_blob(&blob)
        .map_err(|e| texte(&e))?;
    let bitmap: web_sys::ImageBitmap = JsFuture::from(promesse)
        .await
        .map_err(|e| format!("image illisible : {}", texte(&e)))?
        .dyn_into()
        .map_err(|_| String::from("createImageBitmap ne rend pas une ImageBitmap"))?;
    let (l, h) = (bitmap.width(), bitmap.height());
    let image = pixels(l, h, |c| c.draw_image_with_image_bitmap(&bitmap, 0.0, 0.0));
    bitmap.close();
    image
}
