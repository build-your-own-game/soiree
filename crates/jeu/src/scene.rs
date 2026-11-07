//! Ce que le jeu montre (§ 2.9) : la scène de l'écran ([`Scene`]), ce qu'un téléphone
//! reçoit ([`Montre`]), et la valeur qui n'apparaît qu'une fois définitive
//! ([`Definitif`]).
//!
//! La scène est une donnée, recalculée depuis l'état à chaque lecture, dessinée par le
//! commun. Elle s'écrit directement dans sa forme sur le fil, [`jeu_abi::SceneFil`] :
//! une liste à plat, chaque nœud après son parent. Ses longueurs sont des `Fixe` (REM-57).

use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use jeu_abi::{
    Animation, ClavierBrut, Cle, CodeTouche, ComposantVuFil, CompteAReboursFil, Duree,
    Echantillonnage, Effet, Fixe, Instant, Lignes, Mods, NoeudFil, Oui, RaccourciFil, Rect,
    Reglages, Rgba, SceneFil, SorteNoeud, StyleTexte, TexteCourt, Unite, ZoneVu,
};

use crate::forme::Message;
use crate::temps::Echeance;

/// Un compte à rebours, dessiné par le commun à chaque image (SQ-36, CAT-99).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompteARebours {
    /// Il court jusqu'à l'échéance.
    Court {
        /// L'échéance.
        echeance: Echeance,
    },
    /// Il est figé, avec ce qui reste.
    Fige {
        /// Ce qui reste.
        restant: Duree,
    },
}

impl CompteARebours {
    pub(crate) const fn fil(self) -> CompteAReboursFil {
        match self {
            CompteARebours::Court { echeance } => CompteAReboursFil::Court {
                echeance: echeance.pas(),
            },
            CompteARebours::Fige { restant } => CompteAReboursFil::Fige { restant },
        }
    }
}

/// Un composant riche de la scène, avec les actions du jeu.
#[derive(Debug, Clone, PartialEq)]
pub enum ComposantVu<A> {
    /// Un champ de texte, qui reste un vrai élément du navigateur (TL-33, PUN-40).
    Champ {
        /// La limite, dans son unité (TL-27).
        limite: u32,
        /// L'unité de la limite.
        unite: Unite,
        /// Combien de lignes.
        lignes: Lignes,
        /// Sa valeur.
        valeur: String,
        /// Le texte d'exemple.
        exemple: String,
        /// L'action envoyée à la validation.
        fin: Option<A>,
    },
    /// Un texte qu'on édite sur place.
    EnPlace {
        /// La limite, dans son unité.
        limite: u32,
        /// L'unité de la limite.
        unite: Unite,
        /// Sa valeur.
        valeur: String,
        /// Son style.
        style: StyleTexte,
    },
    /// Des objets déplaçables entre des zones (TL-36 à TL-47).
    Deplacables {
        /// Les zones, avec leurs objets dans l'ordre de l'état.
        zones: Vec<ZoneVu>,
        /// La zone où un objet déposé disparaît.
        poubelle: Option<Cle>,
        /// La distance avant qu'un appui devienne un glisser.
        seuil: Fixe,
        /// La largeur du bord qui fait défiler.
        bord: Fixe,
        /// La vitesse de défilement la plus grande, par seconde.
        vitesse_max: Fixe,
        /// Le plafond du défilement au réveil.
        plafond_reveil: Duree,
    },
    /// Un sélecteur de fichiers.
    Selecteur {
        /// Les types acceptés.
        genres: Vec<TexteCourt>,
        /// Plusieurs fichiers à la fois.
        multiple: Oui,
        /// Un dépôt n'importe où sur la page.
        depot_partout: Oui,
        /// Un collage.
        collage: Oui,
        /// La taille la plus grande d'un fichier.
        taille_max: u64,
    },
    /// Une fenêtre modale.
    Modale {
        /// L'action envoyée à sa fermeture.
        fermer: A,
        /// Le panneau de la fenêtre ; un clic hors de lui la ferme (ABI 4).
        panneau: Rect,
    },
    /// Un lien de téléchargement d'une ressource.
    Lien {
        /// Ce qui s'y lit.
        nom: TexteCourt,
        /// La ressource.
        ressource: Cle,
        /// Le nom du fichier téléchargé (ABI 4).
        fichier: TexteCourt,
    },
}

/// Un nœud de la scène, que ses modificateurs changent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Noeud {
    fil: NoeudFil,
}

impl Noeud {
    /// Glisse entre deux pas, par sa clé.
    pub fn glisse(&mut self) -> &mut Noeud {
        self.fil.glisse = Oui::Oui;
        self
    }

    /// Ne glisse pas à ce pas (TL-23).
    pub fn rupture(&mut self) -> &mut Noeud {
        self.fil.rupture = Oui::Oui;
        self
    }

    /// Aligné sur les pixels entiers (SQ-52).
    pub fn pixel_entier(&mut self) -> &mut Noeud {
        self.fil.pixel_entier = Oui::Oui;
        self
    }

    /// Lisse une correction de prédiction sur cette durée.
    pub fn lisse(&mut self, d: Duree) -> &mut Noeud {
        self.fil.lisse = Some(d);
        self
    }

    /// Une animation, partie d'un instant de la partie (REM-12).
    pub fn anime(&mut self, depuis: Instant, a: Animation) -> &mut Noeud {
        self.fil.anime = Some((depuis, a));
        self
    }

    /// L'opacité, de 0 à 255.
    pub fn opacite(&mut self, o: u8) -> &mut Noeud {
        self.fil.opacite = o;
        self
    }
}

/// L'encodage d'une action ; vide si elle ne s'encode pas, ce que la validation de la
/// scène refusera (§ 6.1).
fn encoder_action<A: Message>(a: &A) -> Vec<u8> {
    let mut o = Vec::new();
    if jeu_abi::encoder(a, &mut o).is_err() {
        o.clear();
    }
    o
}

/// La forme sur le fil d'un composant : ses actions encodées.
fn composant_fil<A: Message>(c: ComposantVu<A>) -> ComposantVuFil {
    match c {
        ComposantVu::Champ {
            limite,
            unite,
            lignes,
            valeur,
            exemple,
            fin,
        } => ComposantVuFil::Champ {
            limite,
            unite,
            lignes,
            valeur,
            exemple,
            fin: fin.as_ref().map(encoder_action),
        },
        ComposantVu::EnPlace {
            limite,
            unite,
            valeur,
            style,
        } => ComposantVuFil::EnPlace {
            limite,
            unite,
            valeur,
            style,
        },
        ComposantVu::Deplacables {
            zones,
            poubelle,
            seuil,
            bord,
            vitesse_max,
            plafond_reveil,
        } => ComposantVuFil::Deplacables {
            zones,
            poubelle,
            seuil,
            bord,
            vitesse_max,
            plafond_reveil,
        },
        ComposantVu::Selecteur {
            genres,
            multiple,
            depot_partout,
            collage,
            taille_max,
        } => ComposantVuFil::Selecteur {
            genres,
            multiple,
            depot_partout,
            collage,
            taille_max,
        },
        ComposantVu::Modale { fermer, panneau } => ComposantVuFil::Modale {
            fermer: encoder_action(&fermer),
            panneau,
        },
        ComposantVu::Lien {
            nom,
            ressource,
            fichier,
        } => ComposantVuFil::Lien {
            nom,
            ressource,
            fichier,
        },
    }
}

/// La scène de l'écran, que le jeu remplit dans `vue_ecran`.
#[derive(Debug)]
pub struct Scene<A: Message> {
    fond: Option<Rgba>,
    noeuds: Vec<Noeud>,
    raccourcis: Vec<RaccourciFil>,
    clavier_brut: ClavierBrut,
    effets: Vec<Effet>,
    /// Le parent des nœuds ajoutés maintenant : le groupe ou le sous-arbre en cours.
    parent: Option<u32>,
    /// Ce que rend un ajout qui n'aurait pas pu se faire : il n'y en a pas, mais une
    /// méthode qui rend `&mut Noeud` doit toujours rendre quelque chose.
    secours: Noeud,
    _actions: PhantomData<A>,
}

impl<A: Message> Default for Scene<A> {
    fn default() -> Self {
        Scene::new()
    }
}

impl<A: Message> Scene<A> {
    /// Une scène vide.
    pub fn new() -> Scene<A> {
        Scene {
            fond: None,
            noeuds: Vec::new(),
            raccourcis: Vec::new(),
            clavier_brut: ClavierBrut::Non,
            effets: Vec::new(),
            parent: None,
            secours: Noeud {
                fil: NoeudFil::simple(Cle::du_fil(0), None, SorteNoeud::Definitif),
            },
            _actions: PhantomData,
        }
    }

    /// Sa forme sur le fil.
    pub fn fil(self) -> SceneFil {
        SceneFil {
            fond: self.fond,
            noeuds: self.noeuds.into_iter().map(|n| n.fil).collect(),
            raccourcis: self.raccourcis,
            clavier_brut: self.clavier_brut,
            effets: self.effets,
        }
    }

    fn ajouter(&mut self, cle: Cle, sorte: SorteNoeud) -> &mut Noeud {
        self.noeuds.push(Noeud {
            fil: NoeudFil::simple(cle, self.parent, sorte),
        });
        match self.noeuds.last_mut() {
            Some(n) => n,
            None => &mut self.secours,
        }
    }

    /// Un nœud qui en contient d'autres : `f` les ajoute.
    fn contenir(
        &mut self,
        cle: Cle,
        sorte: SorteNoeud,
        f: impl FnOnce(&mut Scene<A>),
    ) -> &mut Noeud {
        let rang = self.noeuds.len();
        let _: &mut Noeud = self.ajouter(cle, sorte);
        let avant = self.parent;
        self.parent = u32::try_from(rang).ok();
        f(self);
        self.parent = avant;
        match self.noeuds.get_mut(rang) {
            Some(n) => n,
            None => &mut self.secours,
        }
    }

    /// La couleur de fond.
    pub fn fond(&mut self, c: Rgba) {
        self.fond = Some(c);
    }

    /// Un rectangle plein.
    pub fn rect(&mut self, cle: Cle, r: Rect, couleur: Rgba) -> &mut Noeud {
        self.ajouter(cle, SorteNoeud::Rect { r, couleur })
    }

    /// Un texte dans une boîte.
    pub fn texte(&mut self, cle: Cle, texte: &str, boite: Rect, style: StyleTexte) -> &mut Noeud {
        self.ajouter(
            cle,
            SorteNoeud::Texte {
                texte: String::from(texte),
                boite,
                style,
            },
        )
    }

    /// Une image, nommée par une clé, jamais par une adresse (TL-15).
    pub fn image(&mut self, cle: Cle, image: Cle, r: Rect, ech: Echantillonnage) -> &mut Noeud {
        self.ajouter(cle, SorteNoeud::Image { image, r, ech })
    }

    /// Un groupe : ses enfants, ajoutés par `f`, sont décalés ensemble.
    pub fn groupe(
        &mut self,
        cle: Cle,
        decalage: (Fixe, Fixe),
        f: impl FnOnce(&mut Scene<A>),
    ) -> &mut Noeud {
        self.contenir(cle, SorteNoeud::Groupe { decalage }, f)
    }

    /// Un bouton, et l'action qu'il envoie : rien d'autre ne s'envoie (SAL-152).
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de § 2.9 prend l'action par valeur, comme un jeu l'écrit"
    )]
    pub fn bouton(&mut self, cle: Cle, boite: Rect, nom: &str, action: A) -> &mut Noeud {
        let action = encoder_action(&action);
        self.ajouter(
            cle,
            SorteNoeud::Bouton {
                boite,
                nom: String::from(nom),
                action,
            },
        )
    }

    /// Un raccourci du clavier de l'écran.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de § 2.9 prend l'action par valeur, comme un jeu l'écrit"
    )]
    pub fn raccourci(&mut self, touche: CodeTouche, mods: Mods, repetition: Oui, action: A) {
        self.raccourcis.push(RaccourciFil {
            touche,
            mods,
            repetition,
            action: encoder_action(&action),
        });
    }

    /// Un compte à rebours, dessiné par le commun (SQ-36, CAT-99).
    pub fn compte_a_rebours(
        &mut self,
        cle: Cle,
        c: CompteARebours,
        boite: Rect,
        style: StyleTexte,
    ) -> &mut Noeud {
        self.ajouter(
            cle,
            SorteNoeud::CompteARebours {
                compte: c.fil(),
                boite,
                style,
            },
        )
    }

    /// Le code QR de l'adresse que les téléphones joignent vraiment (SQ-16).
    pub fn qr(&mut self, cle: Cle, boite: Rect) -> &mut Noeud {
        self.ajouter(cle, SorteNoeud::Qr { boite })
    }

    /// Un composant riche, sans boîte : son groupe le place, comme avant l'ABI 4.
    pub fn composant(&mut self, cle: Cle, c: ComposantVu<A>) -> &mut Noeud {
        self.ajouter(
            cle,
            SorteNoeud::Composant {
                boite: None,
                composant: composant_fil(c),
            },
        )
    }

    /// Un composant riche dans cette boîte (ABI 4), que le cœur vérifie dans le repère
    /// comme celle d'un texte (Tier list, BESOINS-COMMUN § 7 ; le chat de Sidequest).
    pub fn composant_dans(&mut self, cle: Cle, boite: Rect, c: ComposantVu<A>) -> &mut Noeud {
        self.ajouter(
            cle,
            SorteNoeud::Composant {
                boite: Some(boite),
                composant: composant_fil(c),
            },
        )
    }

    /// Où le commun écrit ses avertissements, dont les refus de l'écran (TL-71).
    pub fn avertissements(&mut self, cle: Cle, boite: Rect) {
        let _: &mut Noeud = self.ajouter(cle, SorteNoeud::Avertissements { boite });
    }

    /// Les touches brutes du clavier.
    pub fn clavier_brut(&mut self, c: ClavierBrut) {
        self.clavier_brut = c;
    }

    /// Une surface de pointeur brut, sans défilement natif.
    pub fn surface(&mut self, cle: Cle, r: Rect) {
        let _: &mut Noeud = self.ajouter(cle, SorteNoeud::Surface { r });
    }

    /// Ce sous-arbre ne vient que de l'instance définitive, par clé (§ 4.4).
    pub fn definitif(&mut self, cle: Cle, f: impl FnOnce(&mut Scene<A>)) {
        let _: &mut Noeud = self.contenir(cle, SorteNoeud::Definitif, f);
    }

    /// Un effet de la scène.
    pub fn effet(&mut self, e: Effet) {
        self.effets.push(e);
    }
}

/// Ce qu'un téléphone reçoit : sa vue, ses effets, ses réglages.
#[jeu::message]
pub struct Montre<V: Message> {
    /// La vue, du type du jeu.
    pub vue: V,
    /// Les effets, chacun avec son identité (PUN-66, SQ-79).
    pub effets: Vec<Effet>,
    /// Les réglages du téléphone.
    pub reglages: Reglages,
}

impl<V: Message> Montre<V> {
    /// Une vue sans effet, avec des réglages ordinaires.
    pub fn de(vue: V) -> Montre<V> {
        Montre {
            vue,
            effets: Vec::new(),
            reglages: Reglages::ORDINAIRES,
        }
    }
}

/// Une valeur d'une vue de téléphone qui ne s'affiche qu'une fois définitive
/// (prédiction, § 5). Sur la partie qui fait foi, elle l'est toujours.
#[jeu::message]
pub struct Definitif<T: Message> {
    cle: Cle,
    valeur: Option<T>,
}

impl<T: Message> Definitif<T> {
    /// La valeur `valeur`, nommée par `cle`.
    pub fn de(cle: Cle, valeur: T) -> Definitif<T> {
        Definitif {
            cle,
            valeur: Some(valeur),
        }
    }

    /// La valeur ; sur la page, `None` tant qu'elle n'est pas sûre.
    pub fn valeur(&self) -> Option<&T> {
        self.valeur.as_ref()
    }

    /// Sa clé.
    pub fn cle(&self) -> Cle {
        self.cle
    }
}
