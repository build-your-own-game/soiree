//! Le scénario `dessin` du banc dans un vrai navigateur (PHASE-3, lot 9).
//!
//! Une bibliothèque WebAssembly : `wasm-bindgen` la relie à la page `dessin.html` de
//! `soiree-banc dessin`, qui appelle [`dessiner_le_banc`] et envoie ce qu'elle rend à
//! `POST /banc/rapport`. Pour chaque jeu du manifeste, la page reçoit, comme du Worker,
//! la forme de l'écran, les polices et les images (`VersPage::Ressource`), puis chaque
//! scène de pire cas (`VersPage::Scene`) ; elle la dessine plusieurs fois et mesure le
//! travail du GPU fini de chaque image (du début du dessin au rappel
//! `onSubmittedWorkDone`), relit les pixels aux points que le banc a choisis, donne
//! l'empreinte de l'agencement de chaque texte dessiné (le BLAKE3 de ses octets
//! canoniques) et vérifie que le tampon que `glyphon` dessine le porte, glyphe par
//! glyphe. Puis elle détruit l'appareil (`device.destroy()`), attend sa perte, le refait,
//! redessine la dernière scène et relit les mêmes points.
//!
//! Le banc juge tout : la page ne décide d'aucune réussite.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(target_arch = "wasm32")]
mod banc {
    #![expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "le banc : des pixels et des nombres de la page, ramenés aux types du rendu ; rien ne revient à une partie"
    )]

    use std::fmt::Write as _;
    use std::sync::atomic::Ordering;

    use ecran::{ChoixMoteur, Ecran, attendre, maintenant_ms, texte_d_erreur};
    use jeu_abi::{FormeEcran, Oui, decoder};
    use js_sys::{Function, Reflect, Uint8Array};
    use rendu::{Fautif, mettre_a_plat};
    use wasm_bindgen::prelude::wasm_bindgen;
    use wasm_bindgen::{JsCast, JsValue};
    use wasm_bindgen_futures::JsFuture;
    use web_sys::HtmlCanvasElement;

    /// Le plus long qu'on attend le GPU, en millisecondes.
    const DELAI_MS: f64 = 20_000.0;

    fn texte(e: &JsValue) -> String {
        texte_d_erreur(e)
    }

    /// Une chaîne JSON.
    fn chaine(s: &str) -> String {
        let mut o = String::from("\"");
        for c in s.chars() {
            match c {
                '"' => o.push_str("\\\""),
                '\\' => o.push_str("\\\\"),
                c if u32::from(c) < 0x20 => {
                    let _: std::fmt::Result = write!(o, "\\u{:04x}", u32::from(c));
                }
                c => o.push(c),
            }
        }
        o.push('"');
        o
    }

    fn champ(v: &JsValue, nom: &str) -> Result<JsValue, String> {
        Reflect::get(v, &JsValue::from_str(nom)).map_err(|e| format!("{nom} : {}", texte(&e)))
    }

    fn liste(v: &JsValue, nom: &str) -> Result<Vec<JsValue>, String> {
        let a: js_sys::Array = champ(v, nom)?
            .dyn_into()
            .map_err(|_| format!("{nom} n'est pas une liste"))?;
        Ok(a.iter().collect())
    }

    async fn charger(url: &str) -> Result<Vec<u8>, String> {
        let fenetre = web_sys::window().ok_or_else(|| String::from("pas de fenêtre"))?;
        let reponse: web_sys::Response = JsFuture::from(fenetre.fetch_with_str(url))
            .await
            .map_err(|e| format!("{url} : {}", texte(&e)))?
            .dyn_into()
            .map_err(|_| format!("{url} : pas une réponse"))?;
        if !reponse.ok() {
            return Err(format!("{url} : {}", reponse.status()));
        }
        let tampon = JsFuture::from(reponse.array_buffer().map_err(|e| texte(&e))?)
            .await
            .map_err(|e| format!("{url} : {}", texte(&e)))?;
        Ok(Uint8Array::new(&tampon).to_vec())
    }

    async fn recevoir(ecran: &mut Ecran, url: &str) -> Result<(), String> {
        let octets = charger(url).await?;
        ecran.recevoir(&octets).await.map(|_| ())
    }

    /// Dessine la dernière scène et attend le travail du GPU fini ; rend sa durée en ms.
    async fn image(ecran: &mut Ecran, ceder: &Function) -> Result<(f64, rendu::Dessin), String> {
        let debut = maintenant_ms();
        let dessin = ecran.dessiner()?;
        let fin = ecran.quand_fini();
        let f = std::sync::Arc::clone(&fin);
        attendre(
            ecran.appareil(),
            &move || f.load(Ordering::SeqCst) != 0,
            ceder,
            DELAI_MS,
        )
        .await?;
        Ok((f64::from_bits(fin.load(Ordering::SeqCst)) - debut, dessin))
    }

    /// Relit les pixels aux points du repère de la scène ; `null` hors de la toile.
    async fn sonder(
        ecran: &Ecran,
        sondes: &[(f32, f32)],
        ceder: &Function,
    ) -> Result<String, String> {
        let relecture = ecran.relire()?;
        let r = &relecture;
        attendre(ecran.appareil(), &|| r.prete().is_some(), ceder, DELAI_MS).await?;
        let img = relecture.image()?;
        let affine = ecran.cadre().transformation();
        let mut o = String::from("[");
        for (k, (x, y)) in sondes.iter().enumerate() {
            if k > 0 {
                o.push(',');
            }
            let (px, py) = affine.point(*x, *y);
            let lu = if px >= 0.0 && py >= 0.0 {
                img.pixel(px.floor() as u32, py.floor() as u32)
            } else {
                None
            };
            match lu {
                Some(p) => {
                    let _: std::fmt::Result = write!(o, "[{},{},{},{}]", p[0], p[1], p[2], p[3]);
                }
                None => o.push_str("null"),
            }
        }
        o.push(']');
        Ok(o)
    }

    /// Les agencements des textes de la dernière scène, et leurs tampons vérifiés.
    fn agencements(ecran: &mut Ecran) -> (String, Vec<String>) {
        let Some(scene) = ecran.scene().cloned() else {
            return (String::from("[]"), Vec::new());
        };
        let cadre = ecran.cadre();
        let liste = mettre_a_plat(&scene, &cadre, &ecran.contexte);
        let rendu = ecran.rendu();
        let textes = rendu.textes(&liste);
        let mut o = String::from("[");
        let mut ecarts = Vec::new();
        for (k, t) in textes.iter().enumerate() {
            if k > 0 {
                o.push(',');
            }
            let h = blake3::hash(&t.agencement.octets_canoniques());
            let _: std::fmt::Result = write!(o, "[{},{}]", t.noeud, chaine(&h.to_hex()));
            if let Err(e) = rendu
                .mise_en_page()
                .verifier_tampon(&t.agencement, &t.tampon)
            {
                ecarts.push(format!("nœud {} : {e}", t.noeud));
            }
        }
        o.push(']');
        (o, ecarts)
    }

    fn sondes_de(scene: &JsValue) -> Result<Vec<(f32, f32)>, String> {
        let mut s = Vec::new();
        for p in liste(scene, "sondes")? {
            let x = Reflect::get(&p, &JsValue::from(0))
                .ok()
                .and_then(|v| v.as_f64());
            let y = Reflect::get(&p, &JsValue::from(1))
                .ok()
                .and_then(|v| v.as_f64());
            match (x, y) {
                (Some(x), Some(y)) => s.push((x as f32, y as f32)),
                _ => return Err(String::from("une sonde illisible")),
            }
        }
        Ok(s)
    }

    async fn jouer_un_jeu(
        ecran: &mut Ecran,
        jeu: &JsValue,
        fois: u32,
        perte: Oui,
        fautif: Oui,
        ceder: &Function,
    ) -> Result<String, String> {
        let nom = champ(jeu, "nom")?.as_string().unwrap_or_default();
        let forme_url = champ(jeu, "forme")?.as_string().unwrap_or_default();
        let forme: FormeEcran =
            decoder(&charger(&forme_url).await?).map_err(|e| format!("forme illisible : {e}"))?;
        ecran.nouveau_jeu(forme);
        for r in liste(jeu, "ressources")? {
            recevoir(ecran, &r.as_string().unwrap_or_default()).await?;
        }
        let mut o = format!("{{\"nom\":{},\"scenes\":[", chaine(&nom));
        let mut derniere: Vec<(f32, f32)> = Vec::new();
        for (k, sc) in liste(jeu, "scenes")?.iter().enumerate() {
            if k > 0 {
                o.push(',');
            }
            for r in liste(sc, "ressources")? {
                recevoir(ecran, &r.as_string().unwrap_or_default()).await?;
            }
            recevoir(
                ecran,
                &champ(sc, "fichier")?.as_string().unwrap_or_default(),
            )
            .await?;
            let indice = champ(sc, "indice")?.as_f64().unwrap_or(-1.0);
            // Le contrôle négatif : le nœud que le banc a choisi d'oublier dans la scène.
            ecran.contexte.fautif = if fautif == Oui::Oui {
                champ(sc, "oublie")?
                    .as_f64()
                    .map(|n| Fautif::OublierLeNoeud(n as u32))
            } else {
                None
            };
            let mut durees = Vec::new();
            let mut dessin = rendu::Dessin::default();
            for _ in 0..fois {
                let (ms, d) = image(ecran, ceder).await?;
                durees.push(format!("{ms}"));
                dessin = d;
                // Une tâche de la page entre deux images : le compositeur présente.
                let p: js_sys::Promise = ceder
                    .call0(&JsValue::NULL)
                    .map_err(|e| texte(&e))?
                    .dyn_into()
                    .map_err(|_| String::from("ceder"))?;
                JsFuture::from(p).await.map_err(|e| texte(&e))?;
            }
            let sondes = sondes_de(sc)?;
            let lus = sonder(ecran, &sondes, ceder).await?;
            let (agences, ecarts) = agencements(ecran);
            let _: std::fmt::Result = write!(
                o,
                "{{\"indice\":{indice},\"noeuds\":{},\"elements\":{},\"couches\":{},\"textes\":{},\"absents\":{},\"images_absentes\":{},\"durees_ms\":[{}],\"sondes\":{lus},\"agencements\":{agences},\"ecarts_tampon\":[{}],\"erreurs_gpu\":[{}]}}",
                dessin.noeuds,
                dessin.elements,
                dessin.couches,
                dessin.textes,
                dessin.glyphes_absents,
                dessin.images_absentes,
                durees.join(","),
                ecarts
                    .iter()
                    .map(|e| chaine(e))
                    .collect::<Vec<_>>()
                    .join(","),
                ecran
                    .erreurs_gpu()
                    .iter()
                    .map(|e| chaine(e))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            derniere = sondes;
        }
        let _: std::fmt::Result = write!(
            o,
            "],\"images_illisibles\":[{}]",
            ecran
                .images_illisibles()
                .iter()
                .map(|e| chaine(e))
                .collect::<Vec<_>>()
                .join(",")
        );
        if perte == Oui::Oui {
            let _: std::fmt::Result =
                write!(o, ",\"perte\":{}", perdre(ecran, &derniere, ceder).await);
        }
        o.push('}');
        Ok(o)
    }

    /// Détruit l'appareil, attend sa perte, le refait, redessine et relit (M8).
    async fn perdre(ecran: &mut Ecran, sondes: &[(f32, f32)], ceder: &Function) -> String {
        let debut = maintenant_ms();
        ecran.detruire_appareil();
        let e: &Ecran = ecran;
        let vu = attendre(e.appareil(), &|| e.perdu(), ceder, DELAI_MS).await;
        if let Err(err) = vu {
            return format!(
                "{{\"erreur\":{}}}",
                chaine(&format!("la perte n'est pas signalée : {err}"))
            );
        }
        let perdu_ms = maintenant_ms() - debut;
        if let Err(err) = ecran.reprendre().await {
            return format!("{{\"erreur\":{}}}", chaine(&format!("reprise : {err}")));
        }
        let (ms, _) = match image(ecran, ceder).await {
            Ok(x) => x,
            Err(err) => {
                return format!(
                    "{{\"erreur\":{}}}",
                    chaine(&format!("image après la reprise : {err}"))
                );
            }
        };
        let retour_ms = maintenant_ms() - debut;
        match sonder(ecran, sondes, ceder).await {
            Ok(lus) => format!(
                "{{\"perdu_ms\":{perdu_ms},\"image_ms\":{ms},\"retour_ms\":{retour_ms},\"sondes\":{lus}}}"
            ),
            Err(err) => format!(
                "{{\"erreur\":{}}}",
                chaine(&format!("relecture après la reprise : {err}"))
            ),
        }
    }

    async fn jouer(
        manifeste: &JsValue,
        toile: HtmlCanvasElement,
        ceder: &Function,
    ) -> Result<String, String> {
        // Une panique se dit sur la console, que le banc écoute, avant d'arrêter la page.
        std::panic::set_hook(Box::new(|i| {
            web_sys::console::error_1(&JsValue::from_str(&i.to_string()));
        }));
        let choix = match champ(manifeste, "moteur")?.as_string().as_deref() {
            Some("webgl2") => ChoixMoteur::WebGl2,
            _ => ChoixMoteur::Auto,
        };
        let mut ecran = Ecran::creer(toile, choix).await?;
        ecran.chauffer(ceder).await?;
        ecran.contexte.adresse_qr = champ(manifeste, "adresse_qr")?.as_string();
        let fautif =
            if champ(manifeste, "fautif")?.as_string().as_deref() == Some("oublier-un-noeud") {
                Oui::Oui
            } else {
                Oui::Non
            };
        let fois = champ(manifeste, "fois")?.as_f64().unwrap_or(3.0) as u32;
        let perte = if champ(manifeste, "perte")?.as_bool() == Some(true) {
            Oui::Oui
        } else {
            Oui::Non
        };
        let mut o = format!(
            "{{\"moteur\":{},\"sorte\":{},\"jeux\":[",
            chaine(&ecran.moteur()),
            chaine(ecran.sorte_de_moteur())
        );
        for (k, jeu) in liste(manifeste, "jeux")?.iter().enumerate() {
            if k > 0 {
                o.push(',');
            }
            o.push_str(&jouer_un_jeu(&mut ecran, jeu, fois, perte, fautif, ceder).await?);
        }
        o.push_str("]}");
        Ok(o)
    }

    /// Joue le manifeste du banc sur la toile, et rend le rapport en JSON (ou
    /// `{ "erreur": … }`).
    #[wasm_bindgen]
    pub async fn dessiner_le_banc(
        manifeste: JsValue,
        toile: HtmlCanvasElement,
        ceder: Function,
    ) -> String {
        match jouer(&manifeste, toile, &ceder).await {
            Ok(r) => r,
            Err(e) => format!("{{\"erreur\":{}}}", chaine(&e)),
        }
    }
}
