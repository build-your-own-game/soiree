//! La page du téléphone (PHASE-3, § 2.10, P19 ; ABI 4) : [`Jeu::page`](crate::Jeu::page)
//! est une fonction de la vue reçue ([`Recue`]) et de ce que le joueur fait en ce
//! moment sur sa page ([`Locaux`]), qui remplit une [`Page`].
//!
//! Elle tient ensemble « chaque jeu code la page de téléphone qu'il veut » (CONTRAT) et
//! « ce qu'on joue tourne enfermé » : la page est dans le module, sans `web-sys`, sans
//! réseau ni DOM. Elle ne lit pas l'état, ne prédit rien et ne gère pas la connexion :
//! les écrans « Connexion à la télé… », « Un instant… », « Reprendre ici » sont ceux du
//! commun. Elle s'écrit **en flux** (une colonne, des rangées), pas en coordonnées :
//! la taille des téléphones varie, et le clavier réduit la hauteur visible.
//!
//! La page se recalcule à chaque vue reçue, à chaque frappe, à chaque changement de
//! taille ou d'orientation (REM-44) : elle doit coûter peu. E19 la juge comme une sorte
//! d'appel à part, et E21 vérifie que chaque vue d'une soirée en donne une valide.

use alloc::string::String;
use alloc::vec::Vec;
use core::marker::PhantomData;

use jeu_abi::{
    Capacites, Clavier, Cle, Declencheur, Direction8, Directions, EcranTelephone, Envoi, LocauxFil,
    NoeudPage, Oui, PageFil, Pas, RETENU_MAX, Rgba, SorteNoeudPage, StylePage, TaillePage,
    TenuPage, Unite,
};

use crate::etape::Tenu;
use crate::forme::Message;
use crate::scene::{CompteARebours, Montre};
use crate::texte::{Limite, Texte};
use crate::traits::Jeu;

/// La vue que le téléphone a reçue, et ce que le commun en sait.
#[derive(Debug, Clone, PartialEq)]
pub struct Recue<V: Message> {
    /// La vue, ses effets et ses réglages.
    pub montre: Montre<V>,
    /// Le pas de cette vue.
    pub pas: Pas,
    /// La vue vient-elle de la partie qui fait foi ? Toujours `Oui` en phase 3 : un
    /// téléphone ne prédit pas encore.
    pub confirmee: Oui,
}

/// Ce que le joueur fait en ce moment sur sa page : le commun le garde entre deux
/// appels, pas le jeu. Le carrousel de Sidequest est un `choix` balayable ; aucun jeu
/// n'a besoin d'un état de page à lui.
#[derive(Debug)]
pub struct Locaux<'l> {
    fil: &'l LocauxFil<'l>,
}

impl<'l> Locaux<'l> {
    /// Réservé au commun : les locaux de ce fil.
    #[doc(hidden)]
    pub const fn du_fil(fil: &'l LocauxFil<'l>) -> Locaux<'l> {
        Locaux { fil }
    }

    /// L'écran : largeur et hauteur visibles en pixels CSS, marges sûres, orientation.
    pub fn ecran(&self) -> EcranTelephone {
        self.fil.ecran
    }

    /// Le texte du champ `champ`, nettoyé par la fonction unique (D22), s'il en a un.
    pub fn texte<L: Limite>(&self, champ: Cle) -> Option<Texte<L>> {
        self.fil
            .champs
            .iter()
            .find(|(c, _)| *c == champ)
            .map(|(_, t)| Texte::nettoyer(t))
    }

    /// L'option choisie du choix `cle`, s'il en a une.
    pub fn choix(&self, cle: Cle) -> Option<u32> {
        self.fil
            .choix
            .iter()
            .find(|(c, _)| *c == cle)
            .map(|(_, n)| *n)
    }

    /// Le maintenu `cle` est-il sous un doigt ?
    pub fn appuye(&self, cle: Cle) -> Oui {
        if self.fil.appuyes.contains(&cle) {
            Oui::Oui
        } else {
            Oui::Non
        }
    }

    /// La direction de la croix `croix` ; le centre si elle n'est pas tenue.
    pub fn direction(&self, croix: Cle) -> Direction8 {
        self.fil
            .croix
            .iter()
            .find(|(c, _)| *c == croix)
            .map_or(Direction8::Centre, |(_, d)| *d)
    }

    /// Ce que la page a retenu pour cette salle et ce jeu.
    pub fn retenu(&self) -> Option<&[u8]> {
        self.fil.retenu
    }

    /// Les capacités du téléphone, dites honnêtement.
    pub fn capacites(&self) -> &Capacites {
        &self.fil.capacites
    }
}

/// Un champ de la page : une ligne de texte, dont le commun garde la valeur.
#[derive(Debug, Clone, PartialEq)]
pub struct ChampPage<'a, G> {
    /// La limite du texte, dans son unité : celle du `Texte<L>` qui la lira.
    pub limite: u32,
    /// L'unité de la limite.
    pub unite: Unite,
    /// Le texte d'exemple, quand il est vide.
    pub exemple: &'a str,
    /// Le clavier demandé.
    pub clavier: Clavier,
    /// Le geste envoyé sur Entrée.
    pub entree: Option<G>,
}

impl<'a, G> ChampPage<'a, G> {
    /// Un champ de la limite `L`, sans exemple ni geste, au clavier de texte.
    pub fn de<L: Limite>() -> ChampPage<'a, G> {
        ChampPage {
            limite: L::MAX,
            unite: L::UNITE,
            exemple: "",
            clavier: Clavier::Texte,
            entree: None,
        }
    }
}

/// L'encodage d'un message du jeu ; vide s'il ne s'encode pas.
fn encoder<T: Message>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    if jeu_abi::encoder(v, &mut o).is_err() {
        o.clear();
    }
    o
}

/// La page que le jeu remplit dans `Jeu::page`.
#[derive(Debug)]
pub struct Page<J: Jeu> {
    noeuds: Vec<NoeudPage>,
    tenu: Option<TenuPage>,
    retenir: Option<Vec<u8>>,
    /// Le parent des nœuds ajoutés maintenant : le conteneur en cours.
    parent: Option<u32>,
    _jeu: PhantomData<J>,
}

impl<J: Jeu> Default for Page<J> {
    fn default() -> Self {
        Page::new()
    }
}

impl<J: Jeu> Page<J> {
    /// Une page vide.
    pub fn new() -> Page<J> {
        Page {
            noeuds: Vec::new(),
            tenu: None,
            retenir: None,
            parent: None,
            _jeu: PhantomData,
        }
    }

    /// Sa forme sur le fil.
    pub fn fil(self) -> PageFil {
        PageFil {
            noeuds: self.noeuds,
            tenu: self.tenu,
            retenir: self.retenir,
        }
    }

    fn ajouter(&mut self, cle: Cle, sorte: SorteNoeudPage) {
        self.noeuds.push(NoeudPage {
            cle,
            parent: self.parent,
            sorte,
        });
    }

    fn contenir(&mut self, cle: Cle, sorte: SorteNoeudPage, f: impl FnOnce(&mut Self)) {
        let rang = self.noeuds.len();
        self.ajouter(cle, sorte);
        let avant = self.parent;
        self.parent = u32::try_from(rang).ok();
        f(self);
        self.parent = avant;
    }

    /// Des éléments l'un sous l'autre, ajoutés par `f`.
    pub fn colonne(&mut self, cle: Cle, f: impl FnOnce(&mut Self)) {
        self.contenir(cle, SorteNoeudPage::Colonne, f);
    }

    /// Des éléments côte à côte, ajoutés par `f`.
    pub fn rangee(&mut self, cle: Cle, f: impl FnOnce(&mut Self)) {
        self.contenir(cle, SorteNoeudPage::Rangee, f);
    }

    /// Des éléments en grille de `colonnes` colonnes, ajoutés par `f`.
    pub fn grille(&mut self, cle: Cle, colonnes: u8, f: impl FnOnce(&mut Self)) {
        self.contenir(cle, SorteNoeudPage::Grille { colonnes }, f);
    }

    /// Des éléments superposés dans la même boîte, dans l'ordre : le dernier ajouté
    /// par `f` est dessus (le héros et ses accessoires de Sidequest).
    pub fn pile(&mut self, cle: Cle, f: impl FnOnce(&mut Self)) {
        self.contenir(cle, SorteNoeudPage::Pile, f);
    }

    /// Un texte.
    pub fn texte(&mut self, cle: Cle, t: &str, s: StylePage) {
        self.ajouter(
            cle,
            SorteNoeudPage::Texte {
                texte: String::from(t),
                style: s,
            },
        );
    }

    /// Une image déclarée dans les assets, de cette hauteur.
    pub fn image(&mut self, cle: Cle, image: Cle, h: TaillePage) {
        self.ajouter(
            cle,
            SorteNoeudPage::Image {
                image,
                hauteur: h,
                teinte: None,
            },
        );
    }

    /// Une image déclarée, teintée par le commun (un personnage dans sa couleur).
    pub fn image_teintee(&mut self, cle: Cle, image: Cle, h: TaillePage, teinte: Rgba) {
        self.ajouter(
            cle,
            SorteNoeudPage::Image {
                image,
                hauteur: h,
                teinte: Some(teinte),
            },
        );
    }

    /// Un compte à rebours, dessiné par le commun (SQ-36).
    pub fn compte_a_rebours(&mut self, cle: Cle, c: CompteARebours, s: StylePage) {
        self.ajouter(
            cle,
            SorteNoeudPage::CompteARebours {
                compte: c.fil(),
                style: s,
            },
        );
    }

    /// Un bouton, et le geste qu'il envoie, au contact ou au relâchement (REM-125).
    /// L'autorité revalide le geste à son arrivée.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de PHASE-3, § 2.10, prend le geste par valeur, comme un jeu l'écrit"
    )]
    pub fn bouton(
        &mut self,
        cle: Cle,
        nom: &str,
        geste: J::GesteTelephone,
        quand: Declencheur,
        s: StylePage,
    ) {
        self.ajouter(
            cle,
            SorteNoeudPage::Bouton {
                nom: String::from(nom),
                geste: encoder(&geste),
                quand,
                style: s,
            },
        );
    }

    /// Un bouton que le joueur tient : [`Locaux::appuye`] le dit.
    pub fn maintenu(&mut self, cle: Cle, nom: &str, s: StylePage) {
        self.ajouter(
            cle,
            SorteNoeudPage::Maintenu {
                nom: String::from(nom),
                style: s,
            },
        );
    }

    /// Une croix de quatre ou huit directions : [`Locaux::direction`] la dit
    /// (REM-121 à REM-124).
    pub fn croix(&mut self, cle: Cle, d: Directions) {
        self.ajouter(cle, SorteNoeudPage::Croix { directions: d });
    }

    /// Un champ d'une ligne : [`Locaux::texte`] le lit.
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de PHASE-3, § 2.10, prend le champ par valeur, comme un jeu l'écrit"
    )]
    pub fn champ(&mut self, cle: Cle, c: ChampPage<'_, J::GesteTelephone>) {
        self.ajouter(
            cle,
            SorteNoeudPage::Champ {
                limite: c.limite,
                unite: c.unite,
                exemple: String::from(c.exemple),
                clavier: c.clavier,
                entree: c.entree.as_ref().map(encoder),
            },
        );
    }

    /// Un choix parmi des options, balayable ou non : [`Locaux::choix`] le dit.
    pub fn choix(&mut self, cle: Cle, options: &[(Cle, &str)], balayable: Oui) {
        self.ajouter(
            cle,
            SorteNoeudPage::Choix {
                options: options
                    .iter()
                    .map(|(c, t)| (*c, String::from(*t)))
                    .collect(),
                balayable,
            },
        );
    }

    /// L'état tenu que la page déclare maintenant. Le commun l'envoie s'il a changé,
    /// et le relâche lui-même (sa valeur relâchée vient de `Tenu::relache`).
    #[expect(
        clippy::needless_pass_by_value,
        reason = "l'API de PHASE-3, § 2.10, prend le tenu par valeur, comme un jeu l'écrit"
    )]
    pub fn tenir(&mut self, t: J::TenuTelephone) {
        self.tenu = Some(TenuPage {
            octets: encoder(&t),
            relache: t.relache().as_ref().map(encoder),
        });
    }

    /// Retient ces octets pour cette salle et ce jeu, au plus [`RETENU_MAX`] :
    /// [`Locaux::retenu`] les rendra, après un rechargement aussi.
    pub fn retenir(&mut self, octets: &[u8]) -> Result<(), Envoi> {
        if octets.len() > RETENU_MAX {
            return Err(Envoi::TropGros);
        }
        self.retenir = Some(octets.to_vec());
        Ok(())
    }
}
