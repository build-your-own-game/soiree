//! Les composants de la scène qui sont de vrais éléments du navigateur, posés sur la
//! toile à la boîte de leur nœud (PHASE-3, § 2.9 ; ABI 4 ; lot 11) : `Champ` (un
//! `<input>`), `EnPlace` (un texte éditable sur place), `Selecteur` (un `<input
//! type=file>` invisible sur la boîte que le jeu dessine), `Lien` (un `<a download>` vers
//! les octets de la ressource). La modale et les objets déplaçables n'ont pas d'élément :
//! la page les juge sur les boîtes dessinées (`cibles`, `deplacables`).
//!
//! - Un élément garde son contenu, son curseur, sa sélection et sa composition tant que
//!   sa clé existe (TL-33, PUN-40, HIS-47) ; il ne prend la valeur de la scène que selon
//!   [`crate::saisie::Champ::reecrire`].
//! - Les éléments sous une modale ouverte se cachent : le voile que le jeu dessine sur la
//!   toile ne les couvrirait pas.
//! - Les éléments ne décident de rien : chaque événement va dans la file de la page
//!   ([`Evenement`]), que la page traite quand elle ne tient pas déjà son état (un
//!   `blur()` qu'elle provoque redonne un événement aussitôt).
//! - Un texte s'écrit toujours comme du texte, jamais comme du HTML (TL-35).

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use jeu_abi::{
    Alignement, ComposantVuFil, FinSaisie, Lignes, Mods, Oui, PhasePointeur, PhaseTouche, SceneFil,
    SorteNoeud, StyleTexte, Taille, Unite,
};
use js_sys::{Array, Uint8Array};
use rendu::Boite;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::{Document, Element, Event, HtmlElement, HtmlInputElement, KeyboardEvent};

use crate::saisie::Champ;
use crate::toile::texte_d_erreur;

/// Ce qui arrive à la page, dans l'ordre : la page le traite quand elle ne tient pas son
/// état.
#[derive(Debug)]
pub(crate) enum Evenement {
    /// Un champ a changé (hors composition), ou se termine.
    Saisie {
        /// La clé du champ.
        cle: u64,
        /// Son texte entier.
        texte: String,
        /// Ce qui le termine, ou non.
        fin: FinSaisie,
    },
    /// Des fichiers choisis dans un sélecteur (`Some`), déposés ou collés (`None`).
    Fichiers {
        /// Le sélecteur.
        selecteur: Option<u64>,
        /// Les fichiers.
        fichiers: Vec<web_sys::File>,
    },
    /// Un pointeur, en pixels du document.
    Pointeur {
        /// Ce qui arrive.
        phase: PhasePointeur,
        /// Le pointeur.
        id: i32,
        /// Le bouton.
        bouton: i16,
        /// Sa position.
        x: f64,
        /// Sa position.
        y: f64,
        /// Sa position dans la fenêtre, verticale.
        y_fenetre: f64,
    },
    /// Un clic, en pixels du document.
    Clic {
        /// Sa position.
        x: f64,
        /// Sa position.
        y: f64,
    },
    /// Une touche du clavier.
    Touche {
        /// La touche physique.
        code: String,
        /// Les modificateurs.
        mods: Mods,
        /// Une répétition automatique.
        repetition: Oui,
        /// Enfoncée ou relâchée.
        phase: PhaseTouche,
        /// Une composition est en cours.
        composition: Oui,
        /// Un champ de la page a le focus.
        dans_un_champ: Oui,
        /// L'événement du navigateur : la page empêche son effet par défaut quand la
        /// touche vise un raccourci.
        evenement: KeyboardEvent,
    },
    /// Le bouton du commun d'un téléchargement en attente (P24).
    Telecharger {
        /// Son rang parmi les téléchargements en attente.
        rang: u64,
    },
    /// La page a défilé.
    Defilement,
    /// La fenêtre a changé de taille.
    Taille,
}

/// La file des événements, partagée par les éléments et la page.
pub(crate) type FileEvenements = Rc<RefCell<VecDeque<Evenement>>>;

/// Ce qu'est un élément posé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Sorte {
    Champ,
    EnPlace,
    Selecteur,
    Lien,
}

/// Un élément posé sur la toile.
pub(crate) struct Vu {
    pub sorte: Sorte,
    pub el: HtmlElement,
    pub champ: Champ,
    pub unite: Unite,
    pub limite: u32,
    /// L'action de la validation d'un `Champ`.
    pub fin: Option<Vec<u8>>,
    /// La taille la plus grande d'un fichier, pour un sélecteur.
    pub taille_max: u64,
    /// La ressource d'un lien, et l'adresse de ses octets.
    pub lien: Option<(u64, Option<String>)>,
    /// La composition est-elle en cours ?
    pub composition: Rc<RefCell<bool>>,
    gardes: Vec<Closure<dyn FnMut(Event)>>,
}

/// Les éléments posés.
pub(crate) struct Composants {
    document: Document,
    plan: HtmlElement,
    pub vus: BTreeMap<u64, Vu>,
    file: FileEvenements,
    traiter: Rc<dyn Fn()>,
    /// Les polices enregistrées pour le DOM, par clé : leur famille.
    pub familles: BTreeMap<u64, String>,
}

fn style(e: &Element, regles: &[(&str, &str)]) {
    if let Some(h) = e.dyn_ref::<HtmlElement>() {
        let s = h.style();
        for (n, v) in regles {
            let _: Result<(), wasm_bindgen::JsValue> = s.set_property(n, v);
        }
    }
}

fn px(v: f32) -> String {
    format!("{v}px")
}

fn couleur(c: jeu_abi::Rgba) -> String {
    format!("rgba({},{},{},{})", c.0, c.1, c.2, f32::from(c.3) / 255.0)
}

/// La taille d'un style, en unités du repère : la plus grande d'un budget.
fn taille(t: Taille) -> f32 {
    match t {
        Taille::Fixe(f) | Taille::Budget { max: f, .. } => rendu::liste::unites(f),
    }
}

impl Composants {
    pub(crate) fn new(
        document: Document,
        plan: HtmlElement,
        file: FileEvenements,
        traiter: Rc<dyn Fn()>,
    ) -> Composants {
        Composants {
            document,
            plan,
            vus: BTreeMap::new(),
            file,
            traiter,
            familles: BTreeMap::new(),
        }
    }

    /// Enregistre une police pour le DOM (`FontFace`), sous une famille à elle : un champ
    /// s'écrit dans la police que son style nomme.
    pub(crate) fn poser_police(&mut self, cle: u64, octets: &[u8]) {
        let famille = format!("soiree-{cle:016x}");
        let tampon = Uint8Array::from(octets).buffer();
        if let Ok(f) = web_sys::FontFace::new_with_array_buffer(&famille, &tampon) {
            let _: Result<(), wasm_bindgen::JsValue> = self.document.fonts().add(&f);
            self.familles.insert(cle, famille);
        }
    }

    fn famille(&self, cle: jeu_abi::Cle) -> String {
        self.familles.get(&cle.fil()).map_or_else(
            || String::from("\"Liberation Sans\", sans-serif"),
            |f| format!("\"{f}\", sans-serif"),
        )
    }

    /// Pose les éléments de la scène aux boîtes de leurs nœuds (pixels de la toile, page à
    /// son haut) ; `dpr` : pixels de la toile par pixel du document ; `echelle` : pixels
    /// de la toile par unité du repère ; `sous_la_modale` : l'indice de la modale ouverte,
    /// sous laquelle les éléments se cachent ; `octets` : les ressources reçues, pour les
    /// liens. Les éléments dont la clé a disparu s'en vont.
    pub(crate) fn poser(
        &mut self,
        scene: &SceneFil,
        boites: &[Option<Boite>],
        sous_la_modale: Option<usize>,
        (dpr, echelle): (f32, f32),
        octets: &BTreeMap<u64, Rc<Vec<u8>>>,
    ) {
        let mut vus_ici = Vec::new();
        for (i, (n, b)) in scene.noeuds.iter().zip(boites).enumerate() {
            let (SorteNoeud::Composant { composant, .. }, Some(b)) = (&n.sorte, b) else {
                continue;
            };
            let sorte = match composant {
                ComposantVuFil::Champ { .. } => Sorte::Champ,
                ComposantVuFil::EnPlace { .. } => Sorte::EnPlace,
                ComposantVuFil::Selecteur { .. } => Sorte::Selecteur,
                ComposantVuFil::Lien { .. } => Sorte::Lien,
                ComposantVuFil::Modale { .. } | ComposantVuFil::Deplacables { .. } => continue,
            };
            let cle = n.cle.fil();
            if self.vus.get(&cle).is_some_and(|v| v.sorte != sorte) {
                self.retirer(cle);
            }
            if !self.vus.contains_key(&cle) {
                match self.creer(cle, sorte) {
                    Ok(v) => {
                        let _: Option<Vu> = self.vus.insert(cle, v);
                    }
                    Err(e) => {
                        web_sys::console::error_1(&e.into());
                        continue;
                    }
                }
            }
            vus_ici.push(cle);
            let cache = sous_la_modale.is_some_and(|m| i < m);
            let famille_de = |s: &StyleTexte| self.famille(s.police);
            let famille = match composant {
                ComposantVuFil::EnPlace { style: s, .. } => Some(famille_de(s)),
                ComposantVuFil::Champ { .. }
                | ComposantVuFil::Selecteur { .. }
                | ComposantVuFil::Lien { .. }
                | ComposantVuFil::Modale { .. }
                | ComposantVuFil::Deplacables { .. } => None,
            };
            if let Some(v) = self.vus.get_mut(&cle) {
                let el: &Element = v.el.as_ref();
                style(
                    el,
                    &[
                        ("left", &px(b[0] / dpr)),
                        ("top", &px(b[1] / dpr)),
                        ("width", &px((b[2] - b[0]) / dpr)),
                        ("height", &px((b[3] - b[1]) / dpr)),
                        ("visibility", if cache { "hidden" } else { "visible" }),
                    ],
                );
                mettre_a_jour(
                    v,
                    composant,
                    &self.document,
                    (dpr, echelle),
                    famille.as_deref(),
                    octets,
                );
            }
        }
        let partis: Vec<u64> = self
            .vus
            .keys()
            .filter(|c| !vus_ici.contains(c))
            .copied()
            .collect();
        for c in partis {
            self.retirer(c);
        }
    }

    /// Les liens dont la ressource vient d'arriver prennent son adresse.
    pub(crate) fn ressource_arrivee(&mut self, cle: u64, octets: &BTreeMap<u64, Rc<Vec<u8>>>) {
        for v in self.vus.values_mut() {
            if let Some((r, url)) = v.lien.as_mut()
                && *r == cle
                && url.is_none()
            {
                *url = adresse(octets.get(&cle));
                if let (Some(u), Some(a)) =
                    (url.as_ref(), v.el.dyn_ref::<web_sys::HtmlAnchorElement>())
                {
                    a.set_href(u);
                }
            }
        }
    }

    fn retirer(&mut self, cle: u64) {
        if let Some(v) = self.vus.remove(&cle) {
            if let Some((_, Some(u))) = &v.lien {
                let _: Result<(), wasm_bindgen::JsValue> = web_sys::Url::revoke_object_url(u);
            }
            v.el.remove();
        }
    }

    fn ecouter(
        el: &HtmlElement,
        gardes: &mut Vec<Closure<dyn FnMut(Event)>>,
        nom: &str,
        f: impl FnMut(Event) + 'static,
    ) -> Result<(), String> {
        let c = Closure::<dyn FnMut(Event)>::new(f);
        el.add_event_listener_with_callback(nom, c.as_ref().unchecked_ref())
            .map_err(|e| texte_d_erreur(&e))?;
        gardes.push(c);
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "un élément se crée d'un bloc, avec chacun de ses écouteurs"
    )]
    fn creer(&self, cle: u64, sorte: Sorte) -> Result<Vu, String> {
        let balise = match sorte {
            Sorte::Champ | Sorte::Selecteur => "input",
            Sorte::EnPlace => "div",
            Sorte::Lien => "a",
        };
        let el: HtmlElement = self
            .document
            .create_element(balise)
            .map_err(|e| texte_d_erreur(&e))?
            .dyn_into()
            .map_err(|_| String::from("pas un élément HTML"))?;
        let _: Result<(), wasm_bindgen::JsValue> =
            el.set_attribute("data-cle", &format!("{cle:016x}"));
        style(
            &el,
            &[
                ("position", "absolute"),
                ("box-sizing", "border-box"),
                ("margin", "0"),
            ],
        );
        let composition = Rc::new(RefCell::new(false));
        let mut gardes = Vec::new();
        // Rien d'un élément ne va à la toile : ni le clic ni l'appui.
        for nom in ["pointerdown", "click", "mousedown"] {
            Self::ecouter(&el, &mut gardes, nom, |e: Event| e.stop_propagation())?;
        }
        let texte_de = |el: &HtmlElement| -> String {
            el.dyn_ref::<HtmlInputElement>()
                .map_or_else(|| el.inner_text(), HtmlInputElement::value)
        };
        match sorte {
            Sorte::Champ | Sorte::EnPlace => {
                let _: Result<(), wasm_bindgen::JsValue> =
                    el.set_attribute("data-soiree-champ", "oui");
                if sorte == Sorte::EnPlace {
                    let _: Result<(), wasm_bindgen::JsValue> =
                        el.set_attribute("contenteditable", "plaintext-only");
                    let _: Result<(), wasm_bindgen::JsValue> =
                        el.set_attribute("spellcheck", "false");
                    style(
                        &el,
                        &[
                            ("outline-offset", "2px"),
                            ("overflow", "hidden"),
                            ("cursor", "text"),
                        ],
                    );
                } else {
                    let _: Result<(), wasm_bindgen::JsValue> = el.set_attribute("type", "text");
                    style(
                        &el,
                        &[
                            ("background", "transparent"),
                            ("border", "1px solid rgba(255,255,255,0.35)"),
                            ("color", "#fff"),
                            ("mix-blend-mode", "difference"),
                            ("padding", "0 6px"),
                        ],
                    );
                }
                let (f, c) = (Rc::clone(&self.file), Rc::clone(&self.traiter));
                let comp = Rc::clone(&composition);
                let e2 = el.clone();
                Self::ecouter(&el, &mut gardes, "input", move |ev: Event| {
                    let compose = ev
                        .dyn_ref::<web_sys::InputEvent>()
                        .is_some_and(web_sys::InputEvent::is_composing);
                    if compose || *comp.borrow() {
                        return;
                    }
                    f.borrow_mut().push_back(Evenement::Saisie {
                        cle,
                        texte: texte_de(&e2),
                        fin: FinSaisie::EnCours,
                    });
                    c();
                })?;
                let comp = Rc::clone(&composition);
                Self::ecouter(&el, &mut gardes, "compositionstart", move |_| {
                    *comp.borrow_mut() = true;
                })?;
                let (f, c) = (Rc::clone(&self.file), Rc::clone(&self.traiter));
                let comp = Rc::clone(&composition);
                let e2 = el.clone();
                Self::ecouter(&el, &mut gardes, "compositionend", move |_| {
                    *comp.borrow_mut() = false;
                    f.borrow_mut().push_back(Evenement::Saisie {
                        cle,
                        texte: texte_de(&e2),
                        fin: FinSaisie::EnCours,
                    });
                    c();
                })?;
                let (f, c) = (Rc::clone(&self.file), Rc::clone(&self.traiter));
                let comp = Rc::clone(&composition);
                let e2 = el.clone();
                let en_place = sorte == Sorte::EnPlace;
                Self::ecouter(&el, &mut gardes, "keydown", move |ev: Event| {
                    let Some(k) = ev.dyn_ref::<KeyboardEvent>() else {
                        return;
                    };
                    // Pendant une composition, aucune touche n'est une validation (TL-32).
                    if k.is_composing() || *comp.borrow() || k.key_code() == 229 {
                        return;
                    }
                    if k.code() == "F8" {
                        return;
                    }
                    // Les touches d'un champ lui appartiennent (TL-34) : elles ne vont pas
                    // aux raccourcis de la page.
                    ev.stop_propagation();
                    match k.key().as_str() {
                        "Enter" if !(en_place && k.shift_key()) => {
                            k.prevent_default();
                            f.borrow_mut().push_back(Evenement::Saisie {
                                cle,
                                texte: texte_de(&e2),
                                fin: FinSaisie::Entree,
                            });
                            c();
                            if en_place {
                                e2.blur().unwrap_or(());
                            }
                        }
                        "Escape" => {
                            k.prevent_default();
                            e2.blur().unwrap_or(());
                        }
                        _ => {}
                    }
                })?;
                let (f, c) = (Rc::clone(&self.file), Rc::clone(&self.traiter));
                let e2 = el.clone();
                Self::ecouter(&el, &mut gardes, "blur", move |_| {
                    f.borrow_mut().push_back(Evenement::Saisie {
                        cle,
                        texte: texte_de(&e2),
                        fin: FinSaisie::PerteDeFocus,
                    });
                    c();
                })?;
            }
            Sorte::Selecteur => {
                let _: Result<(), wasm_bindgen::JsValue> = el.set_attribute("type", "file");
                style(
                    &el,
                    &[("opacity", "0"), ("cursor", "pointer"), ("padding", "0")],
                );
                let (f, c) = (Rc::clone(&self.file), Rc::clone(&self.traiter));
                let e2 = el.clone();
                Self::ecouter(&el, &mut gardes, "change", move |_| {
                    let Some(i) = e2.dyn_ref::<HtmlInputElement>() else {
                        return;
                    };
                    let fichiers = i.files().map(|l| liste(&l)).unwrap_or_default();
                    // Remis à zéro : le même fichier peut se choisir une seconde fois (TL-54).
                    i.set_value("");
                    if fichiers.is_empty() {
                        return;
                    }
                    f.borrow_mut().push_back(Evenement::Fichiers {
                        selecteur: Some(cle),
                        fichiers,
                    });
                    c();
                })?;
            }
            Sorte::Lien => {
                style(
                    &el,
                    &[
                        ("display", "flex"),
                        ("align-items", "center"),
                        ("justify-content", "center"),
                        ("color", "#fff"),
                        ("text-decoration", "none"),
                        ("font-family", "\"Liberation Sans\", sans-serif"),
                        ("cursor", "pointer"),
                        ("overflow", "hidden"),
                    ],
                );
            }
        }
        self.plan
            .append_child(&el)
            .map_err(|e| texte_d_erreur(&e))?;
        Ok(Vu {
            sorte,
            el,
            champ: Champ::default(),
            unite: Unite::Utf16,
            limite: 0,
            fin: None,
            taille_max: u64::MAX,
            lien: None,
            composition,
            gardes,
        })
    }
}

impl Drop for Vu {
    fn drop(&mut self) {
        self.gardes.clear();
    }
}

/// Les fichiers d'une liste du navigateur.
pub(crate) fn liste(l: &web_sys::FileList) -> Vec<web_sys::File> {
    (0..l.length()).filter_map(|k| l.get(k)).collect()
}

/// Une adresse `blob:` vers des octets.
fn adresse(octets: Option<&Rc<Vec<u8>>>) -> Option<String> {
    let o = octets?;
    let blob =
        web_sys::Blob::new_with_u8_array_sequence(&Array::of1(&Uint8Array::from(o.as_slice())))
            .ok()?;
    web_sys::Url::create_object_url_with_blob(&blob).ok()
}

/// Met un élément à jour d'après son composant.
#[expect(
    clippy::too_many_lines,
    reason = "un élément se met à jour d'un seul match sur son composant"
)]
fn mettre_a_jour(
    vu: &mut Vu,
    composant: &ComposantVuFil,
    document: &Document,
    (dpr, echelle): (f32, f32),
    famille: Option<&str>,
    octets: &BTreeMap<u64, Rc<Vec<u8>>>,
) {
    let focus = document
        .active_element()
        .is_some_and(|ancre| ancre.is_same_node(Some(vu.el.as_ref())));
    let compose = *vu.composition.borrow();
    let oui = |vrai: bool| if vrai { Oui::Oui } else { Oui::Non };
    match composant {
        ComposantVuFil::Champ {
            limite,
            unite,
            valeur,
            exemple,
            fin,
            ..
        } => {
            vu.limite = *limite;
            vu.unite = *unite;
            vu.fin.clone_from(fin);
            if let Some(entree) = vu.el.dyn_ref::<HtmlInputElement>() {
                entree.set_placeholder(exemple);
                if *unite == Unite::Utf16 {
                    entree.set_max_length(i32::try_from(*limite).unwrap_or(i32::MAX));
                }
                let haut = entree.offset_height();
                style(
                    entree,
                    &[(
                        "font",
                        &format!(
                            "{}px \"Liberation Sans\", sans-serif",
                            (f64::from(haut) * 0.45).round()
                        ),
                    )],
                );
                let affiche = entree.value();
                if vu
                    .champ
                    .reecrire(valeur, oui(focus), oui(compose), &affiche)
                {
                    entree.set_value(valeur);
                }
            }
        }
        ComposantVuFil::EnPlace {
            limite,
            unite,
            valeur,
            style: st,
        } => {
            vu.limite = *limite;
            vu.unite = *unite;
            let taille_css = taille(st.taille) * echelle / dpr;
            let aligne = match st.alignement {
                Alignement::Debut => "left",
                Alignement::Centre => "center",
                Alignement::Fin => "right",
            };
            let blanc = match st.lignes {
                Lignes::Une => "pre",
                Lignes::AuPlus(_) | Lignes::Libre => "pre-wrap",
            };
            style(
                &vu.el,
                &[
                    ("font-family", famille.unwrap_or("sans-serif")),
                    ("font-size", &px(taille_css)),
                    ("line-height", "1.2"),
                    ("color", &couleur(st.couleur)),
                    ("text-align", aligne),
                    ("white-space", blanc),
                    ("overflow-wrap", "anywhere"),
                    ("min-width", "1em"),
                ],
            );
            let affiche = vu.el.inner_text();
            if vu
                .champ
                .reecrire(valeur, oui(focus), oui(compose), &affiche)
            {
                // Du texte, jamais du HTML (TL-35).
                vu.el.set_text_content(Some(valeur));
            }
        }
        ComposantVuFil::Selecteur {
            genres,
            multiple,
            taille_max,
            ..
        } => {
            vu.taille_max = *taille_max;
            if let Some(entree) = vu.el.dyn_ref::<HtmlInputElement>() {
                let accept: Vec<&str> = genres.iter().map(jeu_abi::TexteCourt::as_str).collect();
                entree.set_accept(&accept.join(","));
                entree.set_multiple(*multiple == Oui::Oui);
                let _: Result<(), wasm_bindgen::JsValue> =
                    entree.set_attribute("data-genres", &accept.join(","));
            }
        }
        ComposantVuFil::Lien {
            nom,
            ressource,
            fichier,
        } => {
            if let Some(ancre) = vu.el.dyn_ref::<web_sys::HtmlAnchorElement>() {
                ancre.set_download(fichier.as_str());
                if ancre.text_content().as_deref() != Some(nom.as_str()) {
                    ancre.set_text_content(Some(nom.as_str()));
                }
                let haut = ancre.offset_height();
                style(
                    ancre,
                    &[(
                        "font-size",
                        &format!("{}px", (f64::from(haut) * 0.4).round()),
                    )],
                );
                let res = ressource.fil();
                let change = vu.lien.as_ref().is_none_or(|(x, _)| *x != res);
                if change {
                    if let Some((_, Some(url_))) = &vu.lien {
                        let _: Result<(), wasm_bindgen::JsValue> =
                            web_sys::Url::revoke_object_url(url_);
                    }
                    let url = adresse(octets.get(&res));
                    match &url {
                        Some(url_) => ancre.set_href(url_),
                        None => {
                            let _: Result<(), wasm_bindgen::JsValue> =
                                ancre.remove_attribute("href");
                        }
                    }
                    vu.lien = Some((res, url));
                }
            }
        }
        ComposantVuFil::Modale { .. } | ComposantVuFil::Deplacables { .. } => {}
    }
}
