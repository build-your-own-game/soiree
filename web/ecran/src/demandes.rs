//! Les demandes du jeu que la page de la télé exécute (PHASE-3, § 2.9 ; lot 11), et les
//! fichiers reçus, dont les octets restent dans la page jusqu'à une demande (TL-14).
//!
//! - `Image` : le fichier décodé par le navigateur (un SVG par un `<img>`, tout autre
//!   format par `createImageBitmap`), recadré en carré au centre, réduit au côté voulu
//!   par une toile hors écran (`drawImage`, lissage « high »), puis encodé
//!   (`convertToBlob`) : WebP à la qualité voulue si le navigateur sait l'écrire, PNG
//!   sinon (le type rendu le dit) ; illisible : `Illisible` (TL-60, TL-61).
//! - `Lire` : la taille vérifiée avant la lecture (TL-14) ; au-delà : `TropGros`.
//! - Un fichier que la page n'a pas (un rechargement les perd) : `Interne` (§ 2.9).
//! - `Telecharger` agit dans l'activation du geste qui l'a demandé (P24) ; sans elle, un
//!   bouton du commun la redemande (page.rs).
//! - `Rendu` se dessine par bandes, dans `page.rs`, qui tient l'écran ; ce module en
//!   encode l'image.
//!
//! `Ligne` ne vient pas ici : l'autorité la donne elle-même au relais (`salle::autorite`).

#![expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    reason = "des tailles de fichiers et de pixels du navigateur (des f64 positifs) ramenées en entiers ; rien ne revient à une partie"
)]

use std::collections::BTreeMap;

use jeu_abi::{EchecDemande, FichierRecu, FormatImage, IdFichier, TexteCourt};
use js_sys::{Array, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Blob, Document, File, OffscreenCanvas, OffscreenCanvasRenderingContext2d};

use crate::toile::{est_un_svg, texte_d_erreur};

/// Les fichiers reçus par la page : leurs octets y restent (TL-14). Les numéros partent
/// d'une base tirée de l'heure de la page : un fichier d'avant un rechargement n'est pas
/// confondu avec un fichier neuf.
pub(crate) struct Fichiers {
    base: u64,
    suivant: u64,
    tenus: BTreeMap<u64, File>,
}

/// Un nom court, coupé à la frontière d'un caractère.
fn court(s: &str) -> TexteCourt {
    let mut fin = s.len().min(TexteCourt::MAX);
    while fin > 0 && !s.is_char_boundary(fin) {
        fin = fin.saturating_sub(1);
    }
    TexteCourt::essayer(s.get(..fin).unwrap_or("")).unwrap_or(TexteCourt::new(""))
}

impl Fichiers {
    pub(crate) fn new(base: u64) -> Fichiers {
        Fichiers {
            base,
            suivant: 0,
            tenus: BTreeMap::new(),
        }
    }

    /// Un numéro neuf, de la même suite (aussi pour un geste qui apporte des fichiers).
    pub(crate) fn numero(&mut self) -> u64 {
        self.suivant = self.suivant.saturating_add(1);
        self.base.saturating_add(self.suivant)
    }

    /// Garde un fichier, et rend ce que le jeu en voit.
    pub(crate) fn recevoir(&mut self, f: File) -> FichierRecu {
        let n = self.numero();
        let recu = FichierRecu {
            id: IdFichier::du_fil(n),
            nom: court(&f.name()),
            genre: court(&f.type_()),
            taille: f.size() as u64,
        };
        let _: Option<File> = self.tenus.insert(n, f);
        recu
    }

    /// Le fichier de ce numéro, si la page l'a.
    pub(crate) fn fichier(&self, id: IdFichier) -> Option<File> {
        self.tenus.get(&id.fil()).cloned()
    }
}

async fn octets_de(blob: &Blob) -> Result<Vec<u8>, String> {
    let b = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|e| texte_d_erreur(&e))?;
    Ok(Uint8Array::new(&b).to_vec())
}

/// `Lire` : la taille vérifiée avant la lecture (TL-14).
pub(crate) async fn lire(f: Option<File>, max: u64) -> Result<Vec<u8>, EchecDemande> {
    let f = f.ok_or(EchecDemande::Interne)?;
    if f.size() > max as f64 {
        return Err(EchecDemande::TropGros);
    }
    octets_de(&f).await.map_err(|_| EchecDemande::Illisible)
}

/// Une toile hors écran et son contexte 2d.
pub(crate) fn toile(
    l: u32,
    h: u32,
) -> Result<(OffscreenCanvas, OffscreenCanvasRenderingContext2d), String> {
    let t = OffscreenCanvas::new(l.max(1), h.max(1)).map_err(|e| texte_d_erreur(&e))?;
    let c: OffscreenCanvasRenderingContext2d = t
        .get_context("2d")
        .map_err(|e| texte_d_erreur(&e))?
        .ok_or_else(|| String::from("pas de contexte 2d"))?
        .dyn_into()
        .map_err(|_| String::from("contexte 2d inattendu"))?;
    Ok((t, c))
}

/// Encode une toile hors écran : PNG, ou WebP si le navigateur sait l'écrire (sinon il
/// rend du PNG, et le type du résultat le dit).
pub(crate) async fn encoder(t: &OffscreenCanvas, format: FormatImage) -> Result<Vec<u8>, String> {
    let options = web_sys::ImageEncodeOptions::new();
    match format {
        FormatImage::Png => options.set_type("image/png"),
        FormatImage::WebpSinonPng { qualite } => {
            options.set_type("image/webp");
            options.set_quality(f64::from(qualite.min(100)) / 100.0);
        }
    }
    let p = t
        .convert_to_blob_with_options(&options)
        .map_err(|e| texte_d_erreur(&e))?;
    let blob: Blob = JsFuture::from(p)
        .await
        .map_err(|e| texte_d_erreur(&e))?
        .dyn_into()
        .map_err(|_| String::from("convertToBlob ne rend pas un Blob"))?;
    octets_de(&blob).await
}

/// Une image décodée par le navigateur, à dessiner : un SVG par un `<img>`, le reste par
/// `createImageBitmap`.
enum Source {
    Svg(web_sys::HtmlImageElement, String),
    Bitmap(web_sys::ImageBitmap),
}

async fn decoder(f: &File) -> Result<Source, String> {
    let fenetre = web_sys::window().ok_or_else(|| String::from("pas de fenêtre"))?;
    let debut = octets_de(
        &f.slice_with_i32_and_i32(0, 64)
            .map_err(|e| texte_d_erreur(&e))?,
    )
    .await?;
    if est_un_svg(&debut) || f.type_() == "image/svg+xml" {
        let url = web_sys::Url::create_object_url_with_blob(f).map_err(|e| texte_d_erreur(&e))?;
        let img = web_sys::HtmlImageElement::new().map_err(|e| texte_d_erreur(&e))?;
        img.set_src(&url);
        JsFuture::from(img.decode())
            .await
            .map_err(|e| texte_d_erreur(&e))?;
        return Ok(Source::Svg(img, url));
    }
    let p = fenetre
        .create_image_bitmap_with_blob(f)
        .map_err(|e| texte_d_erreur(&e))?;
    let b: web_sys::ImageBitmap = JsFuture::from(p)
        .await
        .map_err(|e| texte_d_erreur(&e))?
        .dyn_into()
        .map_err(|_| String::from("createImageBitmap ne rend pas une ImageBitmap"))?;
    Ok(Source::Bitmap(b))
}

/// `Image` : décoder, recadrer en carré au centre, réduire, encoder (TL-60). L'échec dit
/// aussi pourquoi, pour le banc.
pub(crate) async fn image(
    fichier: Option<File>,
    carre: u16,
    format: FormatImage,
) -> Result<Vec<u8>, (EchecDemande, String)> {
    let fichier = fichier.ok_or((EchecDemande::Interne, String::from("fichier inconnu")))?;
    let source = decoder(&fichier)
        .await
        .map_err(|e| (EchecDemande::Illisible, e))?;
    let (larg, haut) = match &source {
        Source::Svg(i, _) => (i.natural_width(), i.natural_height()),
        Source::Bitmap(b) => (b.width(), b.height()),
    };
    if larg == 0 || haut == 0 || carre == 0 {
        return Err((
            EchecDemande::Illisible,
            format!("{larg} × {haut}, carré {carre}"),
        ));
    }
    let cote = larg.min(haut);
    let (sx, sy) = (
        f64::from(larg.saturating_sub(cote) / 2),
        f64::from(haut.saturating_sub(cote) / 2),
    );
    let cote_px = f64::from(carre);
    let (hors_ecran, ctx) =
        toile(u32::from(carre), u32::from(carre)).map_err(|e| (EchecDemande::Interne, e))?;
    let _: Result<bool, JsValue> = Reflect::set(
        &ctx,
        &JsValue::from_str("imageSmoothingQuality"),
        &JsValue::from_str("high"),
    );
    let cote_src = f64::from(cote);
    let dessin = match &source {
        Source::Svg(i, _) => ctx
            .draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                i, sx, sy, cote_src, cote_src, 0.0, 0.0, cote_px, cote_px,
            ),
        Source::Bitmap(b) => ctx
            .draw_image_with_image_bitmap_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                b, sx, sy, cote_src, cote_src, 0.0, 0.0, cote_px, cote_px,
            ),
    };
    match source {
        Source::Svg(_, url) => {
            let _: Result<(), JsValue> = web_sys::Url::revoke_object_url(&url);
        }
        Source::Bitmap(b) => b.close(),
    }
    dessin.map_err(|e| (EchecDemande::Illisible, texte_d_erreur(&e)))?;
    encoder(&hors_ecran, format)
        .await
        .map_err(|e| (EchecDemande::Interne, e))
}

/// L'activation d'un geste du joueur est-elle encore là (`navigator.userActivation`) ?
/// Sans l'API, la page se fie à son dernier geste vrai, s'il date de moins de quatre
/// secondes (les navigateurs en gardent cinq).
pub(crate) fn activation(dernier_geste_ms: f64, maintenant_ms: f64) -> bool {
    let api = web_sys::window()
        .map(|w| w.navigator())
        .and_then(|n| Reflect::get(&n, &JsValue::from_str("userActivation")).ok())
        .filter(|u| !u.is_undefined() && !u.is_null());
    match api {
        Some(u) => Reflect::get(&u, &JsValue::from_str("isActive"))
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        None => maintenant_ms - dernier_geste_ms < 4_000.0,
    }
}

/// Donne un fichier à télécharger : un lien `download` vers ses octets, cliqué. Son
/// adresse est rendue une minute plus tard (la rendre tout de suite annule le
/// téléchargement dans Firefox).
pub(crate) fn donner(document: &Document, nom: &str, octets: &[u8]) -> Result<(), String> {
    let blob = Blob::new_with_u8_array_sequence(&Array::of1(&Uint8Array::from(octets)))
        .map_err(|e| texte_d_erreur(&e))?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(|e| texte_d_erreur(&e))?;
    let a: web_sys::HtmlAnchorElement = document
        .create_element("a")
        .map_err(|e| texte_d_erreur(&e))?
        .dyn_into()
        .map_err(|_| String::from("pas un lien"))?;
    a.set_href(&url);
    a.set_download(nom);
    let corps = document
        .body()
        .ok_or_else(|| String::from("pas de corps"))?;
    corps.append_child(&a).map_err(|e| texte_d_erreur(&e))?;
    a.click();
    a.remove();
    if let Some(w) = web_sys::window() {
        let rendre = wasm_bindgen::closure::Closure::once_into_js(move || {
            let _: Result<(), JsValue> = web_sys::Url::revoke_object_url(&url);
        });
        let _: Result<i32, JsValue> =
            w.set_timeout_with_callback_and_timeout_and_arguments_0(rendre.unchecked_ref(), 60_000);
    }
    Ok(())
}
