//! La page du téléphone, sans son DOM (PHASE-3, § 2.10 ; lot 12).
//!
//! **Sans entrées-sorties**, comme l'appareil : la coque (la page `web/telephone` dans le
//! navigateur, les téléphones natifs du banc sur wasmtime) lui dit ce que fait le doigt et
//! ce qui arrive, et rend au [`crate::appareil::Appareil`] ce que la page envoie. Une seule
//! définition, pour que les téléphones natifs du banc passent **par la page** (§ 8.1) comme
//! ceux des navigateurs.
//!
//! - [`InstanceDePage`] : une instance du module, sur n'importe quel moteur
//!   (`partie::Fabrique`), qui n'a vu que `Bonjour` et `Decrire` et ne voit plus que `Page`
//!   (jamais `Creer` ni `Ressources`, § 2.10). Une page qui faute, ou invalide (REM-57), ne
//!   remplace pas la précédente : le téléphone la garde et le compte (§ 2.10) ; une
//!   instance qui a fauté se refait à l'appel suivant.
//! - [`PageDuTelephone`] : **les locaux appartiennent au commun** (l'écran, les champs, les
//!   choix, les maintenus appuyés, les croix, le retenu), qui les garde entre deux appels ;
//!   `Page` se rappelle à chaque vue, chaque saisie, chaque changement de taille. Ce qui en
//!   sort : le tenu, quand il change (le dernier gagne, au plus `debit_max` par seconde,
//!   REM-13, REM-15), les gestes des boutons et de l'Entrée, ce que la page retient.
//! - **Un geste tenu quand la page change** (INTERFACE § 6.6 ; REM-23) : si la nouvelle page
//!   n'a plus la clé du `Maintenu` ou de la `Croix` sous le doigt, le commun lâche ce doigt
//!   avant de poser la page : elle se recalcule sans lui, son tenu relâché part aussitôt, et
//!   le doigt resté sur l'écran ne compte plus jusqu'à ce qu'il se relève
//!   ([`Recalcul::laches`]).
//! - **L'arrière-plan** (REM-11) et **la coupure** (P28) : voir
//!   [`PageDuTelephone::arriere_plan`] et [`PageDuTelephone::coupure`].
//! - Pour le banc : [`PageDuTelephone::chercher_le_geste`] trouve sur la page le chemin d'un
//!   coup de bot (le bouton qui porte ce geste, au besoin après avoir tapé dans les champs
//!   les textes que le geste porte et tourné les choix qui le font), et
//!   [`PageDuTelephone::chercher_le_tenu`] la croix, les maintenus et les choix qui donnent
//!   ce tenu ; [`PageDuTelephone::approcher_le_tenu`] le tenu le plus proche qu'un pouce
//!   puisse donner quand aucun ne le donne exactement ; [`PageDuTelephone::singe`] joue un
//!   geste au hasard, à graine, sur ce que la page montre (§ 8.1).

use std::collections::{BTreeMap, BTreeSet};

use jeu_abi::{
    Appel, ApplicableFil, Capacites, Cle, DeclarationFil, Declencheur, Direction8, Directions,
    EcranTelephone, LocauxFil, Oui, PageFil, Pas, Retour, SorteAppel, SorteNoeudPage, decoder,
    encoder, valider_page,
};
use partie::{BUDGETS_DE_SOIREE, Echec, Fabrique, Instance};

use crate::appareil::{Appareil, CoupRecu};
use crate::chiffre::Hasard;

/// P28 : sans réponse à son écho pendant 1,5 s, un téléphone se dit coupé.
pub const COUPURE_US: u64 = 1_500_000;

/// Le plus d'essais que [`PageDuTelephone::chercher_le_geste`] fait pour trouver les textes
/// d'un geste (choisi : chacun est un appel `Page`).
pub const ESSAIS_MAX: usize = 64;

/// Le plus d'essais que [`PageDuTelephone::chercher_le_geste`] fait en tournant les choix
/// de la page (choisi, correction du lot 12 : le sélecteur de Sidequest a cinq choix, de 2 à
/// 9 options chacun ; une passe en coûte une trentaine, et deux ou trois passes suffisent).
pub const ESSAIS_DES_CHOIX: usize = 192;

/// Des textes hostiles, que le singe tape dans les champs : vides, blancs, longs, emoji
/// composés, contrôles, forçages de sens, balisage (PUN-23, CAT-85 ; les mêmes que E21).
pub const TEXTES_DU_SINGE: &[&str] = &[
    "",
    "   ",
    "Léa",
    "👩‍👩‍👧‍👦",
    "\u{202E}tnemelc\u{202C}",
    "a\u{200B}b\u{2060}c",
    "<script>alert(1)</script>",
    "\u{0007}\u{001B}[31m",
    "ßßßßßßßßßßßß",
    "e\u{301}e\u{301}e\u{301}",
    "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
];

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    let _: Result<(), _> = encoder(v, &mut o);
    o
}

/// Une instance du module, pour la page seulement.
pub struct InstanceDePage<F: Fabrique> {
    fabrique: F,
    instance: Option<F::I>,
    declaration: DeclarationFil,
    /// Les `Page` qui ont fauté, ou rendu une page invalide (§ 2.10).
    pub fautes: u64,
    /// Le carburant du dernier `Page`.
    pub carburant: Option<u64>,
}

fn neuve<F: Fabrique>(f: &mut F) -> Result<(F::I, DeclarationFil), Echec> {
    let mut i = f.neuve().map_err(Echec::Panne)?;
    let budget = BUDGETS_DE_SOIREE.lecture;
    let _: Vec<u8> = i.appeler(SorteAppel::Lecture, &encode(&Appel::Bonjour), budget)?;
    let decrit = i.appeler(SorteAppel::Lecture, &encode(&Appel::Decrire), budget)?;
    match decoder::<Retour<'_>>(&decrit) {
        Ok(Retour::Decrit(d)) => Ok((i, *d)),
        Ok(_) | Err(_) => Err(Echec::Panne(partie::PanneOutil::new(
            partie::CausePanne::AppelIllisible,
            "Decrire ne rend pas une déclaration",
        ))),
    }
}

impl<F: Fabrique> InstanceDePage<F> {
    /// Une instance neuve : `Bonjour`, puis `Decrire`.
    pub fn ouvrir(mut fabrique: F) -> Result<InstanceDePage<F>, Echec> {
        let (i, declaration) = neuve(&mut fabrique)?;
        Ok(InstanceDePage {
            fabrique,
            instance: Some(i),
            declaration,
            fautes: 0,
            carburant: None,
        })
    }

    /// La déclaration du jeu.
    pub const fn declaration(&self) -> &DeclarationFil {
        &self.declaration
    }

    /// `Page` sur cette vue, avec ces locaux : une page valide, ou ce qui ne va pas.
    pub fn page(
        &mut self,
        montre: &[u8],
        pas: Pas,
        locaux: &LocauxFil<'_>,
    ) -> Result<PageFil, String> {
        if self.instance.is_none() {
            match neuve(&mut self.fabrique) {
                Ok((i, _)) => self.instance = Some(i),
                Err(e) => return Err(e.to_string()),
            }
        }
        let Some(i) = self.instance.as_mut() else {
            return Err(String::from("aucune instance"));
        };
        let appel = Appel::Page {
            montre,
            pas,
            locaux: locaux.clone(),
        };
        let rendu = i.appeler(
            SorteAppel::Lecture,
            &encode(&appel),
            BUDGETS_DE_SOIREE.lecture,
        );
        self.carburant = i.carburant();
        let octets = match rendu {
            Ok(o) => o,
            Err(e) => {
                self.fautes = self.fautes.saturating_add(1);
                self.instance = None;
                return Err(e.to_string());
            }
        };
        let page = match decoder::<Retour<'_>>(&octets) {
            Ok(Retour::Page(p)) => p,
            Ok(Retour::Illisible(e)) => {
                self.fautes = self.fautes.saturating_add(1);
                return Err(format!("la page refuse la vue : {e}"));
            }
            Ok(_) | Err(_) => {
                self.fautes = self.fautes.saturating_add(1);
                return Err(String::from("Page ne rend pas une page"));
            }
        };
        if let Err(d) = valider_page(&page, &self.declaration.assets, &self.declaration.limites) {
            self.fautes = self.fautes.saturating_add(1);
            return Err(format!("page invalide : {d}"));
        }
        Ok(page)
    }
}

/// Ce qu'appeler `Page` demande : une instance, ou un faux dans les tests.
pub trait Pager {
    /// `Page` sur cette vue, avec ces locaux.
    fn page(&mut self, montre: &[u8], pas: Pas, locaux: &LocauxFil<'_>) -> Result<PageFil, String>;
}

impl<F: Fabrique> Pager for InstanceDePage<F> {
    fn page(&mut self, montre: &[u8], pas: Pas, locaux: &LocauxFil<'_>) -> Result<PageFil, String> {
        InstanceDePage::page(self, montre, pas, locaux)
    }
}

/// Ce que les locaux gardent, possédé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locaux {
    /// L'écran.
    pub ecran: EcranTelephone,
    /// Le texte brut de chaque champ.
    pub champs: BTreeMap<Cle, String>,
    /// L'option choisie de chaque choix.
    pub choix: BTreeMap<Cle, u32>,
    /// Les maintenus sous un doigt.
    pub appuyes: BTreeSet<Cle>,
    /// La direction de chaque croix tenue (jamais `Centre` : une croix lâchée n'y est pas).
    pub croix: BTreeMap<Cle, Direction8>,
    /// Ce que la page a retenu pour cette salle et ce jeu.
    pub retenu: Option<Vec<u8>>,
    /// Les capacités, dites honnêtement.
    pub capacites: Capacites,
}

impl Locaux {
    /// Rien de saisi, sur cet écran, avec ces capacités.
    pub const fn new(ecran: EcranTelephone, capacites: Capacites) -> Locaux {
        Locaux {
            ecran,
            champs: BTreeMap::new(),
            choix: BTreeMap::new(),
            appuyes: BTreeSet::new(),
            croix: BTreeMap::new(),
            retenu: None,
            capacites,
        }
    }

    /// Sur le fil.
    pub fn fil(&self) -> LocauxFil<'_> {
        LocauxFil {
            ecran: self.ecran,
            champs: self.champs.iter().map(|(c, t)| (*c, t.as_str())).collect(),
            choix: self.choix.iter().map(|(c, n)| (*c, *n)).collect(),
            appuyes: self.appuyes.iter().copied().collect(),
            croix: self.croix.iter().map(|(c, d)| (*c, *d)).collect(),
            retenu: self.retenu.as_deref(),
            capacites: self.capacites,
        }
    }

    fn tient(&self) -> bool {
        !self.appuyes.is_empty() || !self.croix.is_empty()
    }
}

/// Ce qu'un recalcul de la page donne à la coque.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Recalcul {
    /// La page a changé (à poser dans le DOM).
    pub page_changee: bool,
    /// Le tenu à envoyer maintenant (`Appareil::tenir`), s'il a changé et que le débit le
    /// permet.
    pub tenu: Option<Vec<u8>>,
    /// Ce tenu est la valeur relâchée : il part une fois, puis ne se redit plus
    /// (`Appareil::taire_la_redite`) ; un tenu ne se redit que tant qu'il n'est pas
    /// relâché (§ 2.10, P27).
    pub tenu_relache: bool,
    /// Les clés de maintenus et de croix lâchées parce que la page ne les a plus (REM-23)
    /// ou par l'arrière-plan et la coupure : le doigt qui les tenait ne compte plus jusqu'à
    /// ce qu'il se relève.
    pub laches: Vec<Cle>,
    /// Une page en faute ou invalide : la précédente reste (§ 2.10).
    pub faute: Option<String>,
}

/// Ce qu'un appui sur la page envoie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Le geste d'un bouton ou de l'Entrée d'un champ.
    Geste(Vec<u8>),
    /// Rien : la clé n'est pas un bouton (ou pas à ce moment).
    Rien,
}

/// Les comptes de la page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptesPage {
    /// Les appels `Page`.
    pub pages: u64,
    /// Les pages gardées faute d'une valide (§ 2.10).
    pub fautes: u64,
    /// Les doigts lâchés parce que la page n'avait plus leur clé (REM-23).
    pub laches_rem23: u64,
    /// Les relâchements de l'arrière-plan (REM-11).
    pub arriere_plans: u64,
    /// Les relâchements d'une coupure, pour un tenu qui déclare une fraîcheur (P28).
    pub coupures: u64,
    /// Les tenus retenus par le débit (REM-15), partis plus tard.
    pub tenus_retenus: u64,
}

/// La page du téléphone : ses locaux, sa dernière page, son tenu.
pub struct PageDuTelephone<P: Pager> {
    pager: P,
    locaux: Locaux,
    vue: Option<(Pas, Vec<u8>)>,
    page: Option<PageFil>,
    tenu_envoye: Option<Vec<u8>>,
    tenu_en_attente: Option<Vec<u8>>,
    prochain_tenu_us: u64,
    intervalle_tenu_us: u64,
    fraicheur: bool,
    comptes: ComptesPage,
}

impl<P: Pager> PageDuTelephone<P> {
    /// Ce que la page a retenu (`PageFil::retenir`), pour que la coque le garde (lot 12,
    /// fin : `localStorage` dans le navigateur, par salle et par jeu).
    pub fn retenu(&self) -> Option<&[u8]> {
        self.locaux.retenu.as_deref()
    }

    /// Ce que la coque avait gardé du retenu de cette salle et de ce jeu, rendu à la page
    /// neuve (une page rechargée) avant son premier appel.
    pub fn reprendre_le_retenu(&mut self, octets: Vec<u8>) {
        self.locaux.retenu = Some(octets);
    }

    /// Une page sans vue encore. `tenu` : les règles du tenu des téléphones du jeu
    /// (`TelephonesFil::tenu`), d'où le débit et la fraîcheur.
    pub fn new(
        pager: P,
        locaux: Locaux,
        tenu: Option<&jeu_abi::ReglesTenuFil>,
    ) -> PageDuTelephone<P> {
        let debit = tenu.map_or(0, |t| t.debit_max);
        PageDuTelephone {
            pager,
            locaux,
            vue: None,
            page: None,
            tenu_envoye: None,
            tenu_en_attente: None,
            prochain_tenu_us: 0,
            intervalle_tenu_us: if debit == 0 {
                0
            } else {
                1_000_000_u64.div_euclid(u64::from(debit))
            },
            fraicheur: tenu.is_some_and(|t| matches!(t.fraicheur, ApplicableFil::Oui(_))),
            comptes: ComptesPage::default(),
        }
    }

    /// Le moteur de la page.
    pub fn pager(&mut self) -> &mut P {
        &mut self.pager
    }

    /// Les locaux.
    pub const fn locaux(&self) -> &Locaux {
        &self.locaux
    }

    /// La page posée.
    pub const fn page(&self) -> Option<&PageFil> {
        self.page.as_ref()
    }

    /// La vue reçue.
    pub fn vue(&self) -> Option<(Pas, &[u8])> {
        self.vue.as_ref().map(|(p, v)| (*p, v.as_slice()))
    }

    /// Les comptes.
    pub const fn comptes(&self) -> ComptesPage {
        self.comptes
    }

    /// Le dernier tenu rendu à l'appareil.
    pub fn tenu_envoye(&self) -> Option<&[u8]> {
        self.tenu_envoye.as_deref()
    }

    fn appeler(&mut self, locaux: &Locaux) -> Option<Result<PageFil, String>> {
        let (pas, montre) = self.vue.as_ref()?;
        self.comptes.pages = self.comptes.pages.saturating_add(1);
        Some(self.pager.page(montre, *pas, &locaux.fil()))
    }

    /// Les clés que tient le doigt et que cette page n'a plus, à leur sorte.
    fn cles_disparues(&self, page: &PageFil) -> Vec<Cle> {
        let maintenus: BTreeSet<Cle> = page
            .noeuds
            .iter()
            .filter(|n| matches!(n.sorte, SorteNoeudPage::Maintenu { .. }))
            .map(|n| n.cle)
            .collect();
        let croix: BTreeSet<Cle> = page
            .noeuds
            .iter()
            .filter(|n| matches!(n.sorte, SorteNoeudPage::Croix { .. }))
            .map(|n| n.cle)
            .collect();
        self.locaux
            .appuyes
            .iter()
            .filter(|c| !maintenus.contains(c))
            .chain(self.locaux.croix.keys().filter(|c| !croix.contains(c)))
            .copied()
            .collect()
    }

    /// Recalcule la page (une vue, une saisie, une taille), à l'heure `t_us`.
    pub fn recalculer(&mut self, t_us: u64) -> Recalcul {
        self.recalculer_avec(t_us, Vec::new())
    }

    fn recalculer_avec(&mut self, t_us: u64, mut laches: Vec<Cle>) -> Recalcul {
        let mut r = Recalcul::default();
        let locaux = self.locaux.clone();
        let Some(rendu) = self.appeler(&locaux) else {
            r.laches = laches;
            return r;
        };
        let mut page = match rendu {
            Ok(p) => p,
            Err(e) => {
                self.comptes.fautes = self.comptes.fautes.saturating_add(1);
                r.faute = Some(e);
                r.laches = laches;
                return r;
            }
        };
        // REM-23 : une clé tenue que la page n'a plus est lâchée avant de poser la page.
        let disparues = self.cles_disparues(&page);
        if !disparues.is_empty() {
            for c in &disparues {
                self.locaux.appuyes.remove(c);
                self.locaux.croix.remove(c);
            }
            self.comptes.laches_rem23 = self
                .comptes
                .laches_rem23
                .saturating_add(u64::try_from(disparues.len()).unwrap_or(u64::MAX));
            laches.extend(disparues);
            let locaux = self.locaux.clone();
            match self.appeler(&locaux) {
                Some(Ok(p)) => page = p,
                Some(Err(e)) => {
                    self.comptes.fautes = self.comptes.fautes.saturating_add(1);
                    r.faute = Some(e);
                }
                None => {}
            }
        }
        if let Some(octets) = &page.retenir {
            self.locaux.retenu = Some(octets.clone());
        }
        r.page_changee = self.page.as_ref() != Some(&page);
        let tenu = page.tenu.as_ref().map(|t| t.octets.clone());
        self.page = Some(page);
        r.laches = laches;
        if let Some(t) = tenu
            && self.tenu_envoye.as_ref() != Some(&t)
            && self.tenu_en_attente.as_ref() != Some(&t)
        {
            self.tenu_en_attente = Some(t);
        }
        r.tenu = self.tenu_du_debit(t_us);
        r.tenu_relache = r.tenu.is_some() && self.relache(r.tenu.as_deref());
        r
    }

    /// Ce tenu est-il la valeur relâchée de la page posée ?
    pub fn relache(&self, tenu: Option<&[u8]>) -> bool {
        tenu.is_some()
            && self
                .page
                .as_ref()
                .and_then(|p| p.tenu.as_ref())
                .and_then(|tp| tp.relache.as_deref())
                == tenu
    }

    /// Le tenu en attente, s'il peut partir : au plus `debit_max` changements par seconde
    /// (REM-15) ; sinon il attend [`PageDuTelephone::reveiller`].
    fn tenu_du_debit(&mut self, t_us: u64) -> Option<Vec<u8>> {
        let t = self.tenu_en_attente.take()?;
        // Un relâchement passe toujours (SQ-02).
        let relache = self
            .page
            .as_ref()
            .and_then(|p| p.tenu.as_ref())
            .and_then(|tp| tp.relache.as_ref())
            == Some(&t);
        if t_us < self.prochain_tenu_us && !relache {
            self.comptes.tenus_retenus = self.comptes.tenus_retenus.saturating_add(1);
            self.tenu_en_attente = Some(t);
            return None;
        }
        self.prochain_tenu_us = t_us.saturating_add(self.intervalle_tenu_us);
        self.tenu_envoye = Some(t.clone());
        Some(t)
    }

    /// Un réveil : le tenu que le débit retenait part, s'il le peut.
    pub fn reveiller(&mut self, t_us: u64) -> Option<Vec<u8>> {
        self.tenu_du_debit(t_us)
    }

    /// Une vue reçue (SAL-28 : entière).
    pub fn vue_recue(&mut self, t_us: u64, pas: Pas, montre: &[u8]) -> Recalcul {
        if self
            .vue
            .as_ref()
            .is_some_and(|(p, v)| *p == pas && v == montre)
        {
            return Recalcul::default();
        }
        self.vue = Some((pas, montre.to_vec()));
        self.recalculer(t_us)
    }

    /// L'écran change de taille ou d'orientation (REM-44).
    pub fn ecran(&mut self, t_us: u64, ecran: EcranTelephone) -> Recalcul {
        if self.locaux.ecran == ecran {
            return Recalcul::default();
        }
        self.locaux.ecran = ecran;
        self.recalculer(t_us)
    }

    /// Le joueur tape dans un champ (la valeur brute ; le jeu la lit nettoyée).
    pub fn taper(&mut self, t_us: u64, cle: Cle, texte: &str) -> Recalcul {
        if self.locaux.champs.get(&cle).map(String::as_str) == Some(texte) {
            return Recalcul::default();
        }
        self.locaux.champs.insert(cle, String::from(texte));
        self.recalculer(t_us)
    }

    /// Le joueur choisit une option.
    pub fn choisir(&mut self, t_us: u64, cle: Cle, option: u32) -> Recalcul {
        if self.locaux.choix.get(&cle) == Some(&option) {
            return Recalcul::default();
        }
        self.locaux.choix.insert(cle, option);
        self.recalculer(t_us)
    }

    /// Un doigt se pose sur un maintenu (`true`) ou s'en relève (`false`).
    pub fn maintenir(&mut self, t_us: u64, cle: Cle, appuye: Oui) -> Recalcul {
        let change = if appuye == Oui::Oui {
            self.locaux.appuyes.insert(cle)
        } else {
            self.locaux.appuyes.remove(&cle)
        };
        if !change {
            return Recalcul::default();
        }
        self.recalculer(t_us)
    }

    /// La direction d'une croix (`Centre` : lâchée).
    pub fn diriger(&mut self, t_us: u64, cle: Cle, d: Direction8) -> Recalcul {
        let avant = self.locaux.croix.get(&cle).copied();
        let apres = (d != Direction8::Centre).then_some(d);
        if avant == apres {
            return Recalcul::default();
        }
        match apres {
            Some(d) => self.locaux.croix.insert(cle, d),
            None => self.locaux.croix.remove(&cle),
        };
        self.recalculer(t_us)
    }

    fn noeud(&self, cle: Cle) -> Option<&SorteNoeudPage> {
        self.page
            .as_ref()?
            .noeuds
            .iter()
            .find(|n| n.cle == cle)
            .map(|n| &n.sorte)
    }

    /// Un bouton touché (`Contact`) ou relâché dessus (`Relache`) : son geste s'il part à
    /// ce moment-là (REM-125).
    pub fn bouton(&self, cle: Cle, quand: Declencheur) -> Action {
        match self.noeud(cle) {
            Some(SorteNoeudPage::Bouton {
                geste, quand: q, ..
            }) if *q == quand => Action::Geste(geste.clone()),
            _ => Action::Rien,
        }
    }

    /// Entrée dans un champ : son geste, s'il en a un.
    pub fn entree(&self, cle: Cle) -> Action {
        match self.noeud(cle) {
            Some(SorteNoeudPage::Champ {
                entree: Some(g), ..
            }) => Action::Geste(g.clone()),
            _ => Action::Rien,
        }
    }

    /// Tout ce que le doigt tient se lâche, et la page se recalcule.
    fn tout_lacher(&mut self, t_us: u64) -> Recalcul {
        let laches: Vec<Cle> = self
            .locaux
            .appuyes
            .iter()
            .chain(self.locaux.croix.keys())
            .copied()
            .collect();
        self.locaux.appuyes.clear();
        self.locaux.croix.clear();
        self.recalculer_avec(t_us, laches)
    }

    /// L'arrière-plan (`visibilitychange` caché, `pagehide`, `freeze`, `blur`) : les
    /// maintenus et les croix se lèvent, la page calcule son relâchement, qui part
    /// aussitôt (REM-11). La coque cesse ensuite de redire (`Appareil::taire_la_redite`).
    pub fn arriere_plan(&mut self, t_us: u64) -> Recalcul {
        if !self.locaux.tient() {
            return Recalcul::default();
        }
        self.comptes.arriere_plans = self.comptes.arriere_plans.saturating_add(1);
        self.tout_lacher(t_us)
    }

    /// La coupure (P28) : pour un tenu qui déclare une fraîcheur (Sidequest), l'écho resté
    /// sans réponse [`COUPURE_US`] relâche côté page, et le tenu reste relâché après la
    /// reconnexion jusqu'à un nouvel appui (le doigt resté posé ne compte plus) ; sans
    /// fraîcheur (Punchline), rien ne se relâche (INTERFACE § 6.6, REM-10). Le tenu relâché
    /// part à la reconnexion : l'appareil redit son tenu après chaque accueil.
    pub fn coupure(&mut self, t_us: u64) -> Recalcul {
        if !self.fraicheur || !self.locaux.tient() {
            return Recalcul::default();
        }
        self.comptes.coupures = self.comptes.coupures.saturating_add(1);
        self.tout_lacher(t_us)
    }

    /// Les clés de cette page, à leur sorte : pour le singe et les essais.
    fn noeuds_de(&self, f: impl Fn(&SorteNoeudPage) -> bool) -> Vec<Cle> {
        self.page.as_ref().map_or_else(Vec::new, |p| {
            p.noeuds
                .iter()
                .filter(|n| f(&n.sorte))
                .map(|n| n.cle)
                .collect()
        })
    }

    /// Les croix de la page, et leurs directions.
    fn les_croix(&self) -> Vec<(Cle, Directions)> {
        self.page.as_ref().map_or_else(Vec::new, |p| {
            p.noeuds
                .iter()
                .filter_map(|x| {
                    if let SorteNoeudPage::Croix { directions } = x.sorte {
                        Some((x.cle, directions))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }

    /// Le bouton ou le champ de cette page dont le geste est `octets`.
    fn porteur(page: &PageFil, octets: &[u8]) -> Option<(Cle, Option<Declencheur>)> {
        page.noeuds.iter().find_map(|n| {
            if let SorteNoeudPage::Bouton { geste, quand, .. } = &n.sorte
                && geste == octets
            {
                return Some((n.cle, Some(*quand)));
            }
            if let SorteNoeudPage::Champ {
                entree: Some(g), ..
            } = &n.sorte
                && g == octets
            {
                return Some((n.cle, None));
            }
            None
        })
    }

    /// Le chemin d'un coup de bot sur la page (§ 8.1, lot 12) : le bouton (ou l'Entrée du
    /// champ) qui porte exactement ce geste, tel quel, ou après avoir tapé dans les champs,
    /// dans l'ordre de la page, des textes que porte le geste (des chaînes postcard
    /// contiguës), puis tourné les choix de la page (un carrousel, une couleur) comme un
    /// joueur les tourne. Les textes tapés et les options choisies restent dans les locaux,
    /// comme pour un joueur. Rend le recalcul, ou rien si aucun chemin n'est trouvé en
    /// [`ESSAIS_MAX`] appels pour les textes et [`ESSAIS_DES_CHOIX`] pour les choix.
    ///
    /// *Correction du lot 12* : sans les choix, le `Rejoindre` de Sidequest (un prénom et un
    /// look tiré au hasard parmi les options du sélecteur) n'avait aucun chemin, et cinq
    /// téléphones natifs sur six n'envoyaient aucun geste de toute la soirée.
    pub fn chercher_le_geste(&mut self, t_us: u64, octets: &[u8]) -> Option<Recalcul> {
        let page = self.page.clone()?;
        if Self::porteur(&page, octets).is_some() {
            return Some(Recalcul::default());
        }
        // Le meilleur départ pour les choix : les textes qui rapprochent le plus le geste
        // d'un bouton de celui du bot (rien de tapé, s'il n'y en a aucun).
        let mut depart = (self.locaux.clone(), proximite_des_porteurs(&page, octets));
        let champs = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Champ { .. }));
        if !champs.is_empty() {
            let mut essais = 0usize;
            'textes: for chaine in chaines(octets) {
                for fenetre in chaine.windows(champs.len().min(chaine.len()).max(1)) {
                    if essais >= ESSAIS_MAX {
                        break 'textes;
                    }
                    essais = essais.saturating_add(1);
                    let mut l = self.locaux.clone();
                    for (c, t) in champs.iter().zip(fenetre) {
                        l.champs.insert(*c, t.clone());
                    }
                    if let Some(Ok(p)) = self.appeler(&l) {
                        if Self::porteur(&p, octets).is_some() {
                            self.locaux = l;
                            return Some(self.recalculer(t_us));
                        }
                        let proche = proximite_des_porteurs(&p, octets);
                        if proche > depart.1 {
                            depart = (l, proche);
                        }
                    }
                }
            }
        }
        let l = self.tourner_les_choix(depart.0, octets)?;
        self.locaux = l;
        Some(self.recalculer(t_us))
    }

    /// Tourne les choix de la page, l'un après l'autre, en gardant pour chacun l'option qui
    /// rapproche le plus un geste de la page de `octets` (une montée, choix par choix ; les
    /// choix se relisent après chaque option gardée, car une page peut en changer : la
    /// couleur de Sidequest dépend du personnage). Rend les locaux où un bouton ou une
    /// Entrée porte exactement `octets`, ou rien.
    fn tourner_les_choix(&mut self, depart: Locaux, octets: &[u8]) -> Option<Locaux> {
        let mut l = depart;
        let Ok(mut page) = self.appeler(&l)? else {
            return None;
        };
        let mut proche = proximite_des_porteurs(&page, octets);
        let mut essais = 0usize;
        for _passe in 0..3 {
            let mut mieux = false;
            let mut i = 0usize;
            while let Some((c, k)) = choix_de(&page).get(i).copied() {
                i = i.saturating_add(1);
                let mut garde: Option<(Locaux, PageFil, Proximite)> = None;
                for opt in 0..k {
                    let opt = u32::try_from(opt).ok()?;
                    if l.choix.get(&c) == Some(&opt) {
                        continue;
                    }
                    if essais >= ESSAIS_DES_CHOIX {
                        return None;
                    }
                    essais = essais.saturating_add(1);
                    let mut l2 = l.clone();
                    l2.choix.insert(c, opt);
                    let Some(Ok(p)) = self.appeler(&l2) else {
                        continue;
                    };
                    if Self::porteur(&p, octets).is_some() {
                        return Some(l2);
                    }
                    let s = proximite_des_porteurs(&p, octets);
                    if s > garde.as_ref().map_or(proche, |g| g.2) {
                        garde = Some((l2, p, s));
                    }
                }
                if let Some((l2, p, s)) = garde {
                    l = l2;
                    page = p;
                    proche = s;
                    mieux = true;
                }
            }
            if !mieux {
                return None;
            }
        }
        None
    }

    /// Le geste `octets` part-il de la page posée ? Rend son porteur.
    pub fn porteur_du_geste(&self, octets: &[u8]) -> Option<(Cle, Option<Declencheur>)> {
        Self::porteur(self.page.as_ref()?, octets)
    }

    /// Le chemin d'un tenu de bot : la croix et les maintenus qui donnent exactement ce
    /// tenu, posés dans les locaux comme un doigt le ferait. Rend le recalcul, ou rien.
    pub fn chercher_le_tenu(&mut self, t_us: u64, octets: &[u8]) -> Option<Recalcul> {
        let page = self.page.clone()?;
        if page.tenu.as_ref().is_some_and(|t| t.octets == octets) {
            return Some(Recalcul::default());
        }
        let croix: Vec<(Cle, Directions)> = page
            .noeuds
            .iter()
            .filter_map(|n| {
                if let SorteNoeudPage::Croix { directions } = n.sorte {
                    Some((n.cle, directions))
                } else {
                    None
                }
            })
            .collect();
        let maintenus = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Maintenu { .. }));
        let maintenus: Vec<Cle> = maintenus.into_iter().take(3).collect();
        let directions: Vec<Option<(Cle, Direction8)>> = match croix.first() {
            Some((c, ds)) => directions_de(*ds)
                .into_iter()
                .map(|d| Some((*c, d)))
                .collect(),
            None => vec![None],
        };
        let sous_ensembles = 1u32 << maintenus.len();
        for d in &directions {
            for masque in 0..sous_ensembles {
                let mut l = self.locaux.clone();
                l.croix.clear();
                l.appuyes.clear();
                if let Some((c, d)) = d
                    && *d != Direction8::Centre
                {
                    l.croix.insert(*c, *d);
                }
                for (i, m) in maintenus.iter().enumerate() {
                    if masque & (1 << i) != 0 {
                        l.appuyes.insert(*m);
                    }
                }
                if let Some(Ok(p)) = self.appeler(&l)
                    && p.tenu.as_ref().is_some_and(|t| t.octets == octets)
                {
                    self.locaux = l;
                    return Some(self.recalculer(t_us));
                }
            }
        }
        // Un choix (un carrousel qui fait le tenu) : chaque option, seule.
        let les_choix: Vec<(Cle, usize)> = page
            .noeuds
            .iter()
            .filter_map(|n| {
                if let SorteNoeudPage::Choix { options, .. } = &n.sorte {
                    Some((n.cle, options.len()))
                } else {
                    None
                }
            })
            .take(2)
            .collect();
        // Des textes dans les champs (un brouillon tenu à chaque frappe, PUN-01) : les
        // chaînes que porte le tenu, dans l'ordre des champs de la page.
        let champs = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Champ { .. }));
        if !champs.is_empty() {
            let mut essais = 0usize;
            for chaine in chaines(octets) {
                for fenetre in chaine.windows(champs.len().min(chaine.len()).max(1)) {
                    if essais >= ESSAIS_MAX {
                        break;
                    }
                    essais = essais.saturating_add(1);
                    let mut l = self.locaux.clone();
                    for (c, t) in champs.iter().zip(fenetre) {
                        l.champs.insert(*c, t.clone());
                    }
                    if let Some(Ok(p)) = self.appeler(&l)
                        && p.tenu.as_ref().is_some_and(|t| t.octets == octets)
                    {
                        self.locaux = l;
                        return Some(self.recalculer(t_us));
                    }
                }
            }
        }
        for (c, k) in les_choix {
            for opt in 0..k.min(ESSAIS_MAX) {
                let mut l = self.locaux.clone();
                l.choix.insert(c, u32::try_from(opt).unwrap_or(0));
                if let Some(Ok(p)) = self.appeler(&l)
                    && p.tenu.as_ref().is_some_and(|t| t.octets == octets)
                {
                    self.locaux = l;
                    return Some(self.recalculer(t_us));
                }
            }
        }
        None
    }

    /// Le tenu le plus proche de `octets` que la croix et les maintenus de la page donnent,
    /// quand aucun ne le donne exactement (*correction du lot 12*) : un bot de Sidequest
    /// marche vers une zone dans une direction continue, qu'une croix en huit directions ne
    /// donne pas ; un joueur pousse alors la direction la plus proche. Le banc seulement :
    /// la proximité ne se lit que sur des tenus de même longueur faits de flottants `f32`
    /// finis (une direction, un stick), à la distance euclidienne ; tout autre tenu n'a pas
    /// de plus proche, et rien n'est rendu. Rend le recalcul, ou rien.
    pub fn approcher_le_tenu(&mut self, t_us: u64, octets: &[u8]) -> Option<Recalcul> {
        let cible = flottants(octets)?;
        let page = self.page.clone()?;
        let croix = page.noeuds.iter().find_map(|n| {
            if let SorteNoeudPage::Croix { directions } = n.sorte {
                Some((n.cle, directions))
            } else {
                None
            }
        });
        let maintenus: Vec<Cle> = self
            .noeuds_de(|s| matches!(s, SorteNoeudPage::Maintenu { .. }))
            .into_iter()
            .take(3)
            .collect();
        let directions: Vec<Option<(Cle, Direction8)>> = match croix {
            Some((c, ds)) => directions_de(ds)
                .into_iter()
                .map(|d| Some((c, d)))
                .collect(),
            None => vec![None],
        };
        let sous_ensembles = 1u32 << maintenus.len();
        let mut meilleur: Option<(f64, Locaux)> = None;
        for d in &directions {
            for masque in 0..sous_ensembles {
                let mut l = self.locaux.clone();
                l.croix.clear();
                l.appuyes.clear();
                if let Some((c, d)) = d
                    && *d != Direction8::Centre
                {
                    l.croix.insert(*c, *d);
                }
                for (i, m) in maintenus.iter().enumerate() {
                    if masque & (1 << i) != 0 {
                        l.appuyes.insert(*m);
                    }
                }
                let Some(Ok(p)) = self.appeler(&l) else {
                    continue;
                };
                let Some(f) = p.tenu.as_ref().and_then(|t| flottants(&t.octets)) else {
                    continue;
                };
                if f.len() != cible.len() {
                    continue;
                }
                let ecart: f64 = f
                    .iter()
                    .zip(&cible)
                    .map(|(a, b)| {
                        let e = f64::from(*a) - f64::from(*b);
                        e * e
                    })
                    .sum();
                if meilleur.as_ref().is_none_or(|(m, _)| ecart < *m) {
                    meilleur = Some((ecart, l));
                }
            }
        }
        let (_, l) = meilleur?;
        self.locaux = l;
        Some(self.recalculer(t_us))
    }

    /// Le singe de page (§ 8.1) : un geste au hasard, à graine, sur ce que la page montre :
    /// un bouton, le corpus hostile tapé dans un champ (puis Entrée), une option, une
    /// croix tenue ou lâchée, un maintenu. Rend le recalcul et le geste parti, s'il y en a.
    pub fn singe(&mut self, t_us: u64, hasard: &mut impl Hasard) -> (Recalcul, Option<Vec<u8>>) {
        let mut tire = [0u8; 8];
        hasard.remplir(&mut tire);
        let alea = u64::from_le_bytes(tire);
        let tirer = |combien: usize, sel: u64| -> usize {
            let k = u64::try_from(combien.max(1)).unwrap_or(1);
            usize::try_from(sel.checked_rem(k).unwrap_or(0)).unwrap_or(0)
        };
        let boutons = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Bouton { .. }));
        let champs = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Champ { .. }));
        let les_choix: Vec<(Cle, usize)> = self.page.as_ref().map_or_else(Vec::new, |p| {
            p.noeuds
                .iter()
                .filter_map(|x| {
                    if let SorteNoeudPage::Choix { options, .. } = &x.sorte {
                        Some((x.cle, options.len()))
                    } else {
                        None
                    }
                })
                .collect()
        });
        let croix_de_la_page = self.les_croix();
        let maintenus = self.noeuds_de(|s| matches!(s, SorteNoeudPage::Maintenu { .. }));
        let sorte = sorte_du_singe(
            [
                !boutons.is_empty(),
                !champs.is_empty(),
                !les_choix.is_empty(),
                !croix_de_la_page.is_empty(),
                !maintenus.is_empty(),
            ],
            alea,
        );
        match sorte {
            // Un bouton, une fois sur `SINGE_BOUTON_UN_SUR` de ces tirages (fin du lot 12).
            0 if !boutons.is_empty() && (alea >> 48) % SINGE_BOUTON_UN_SUR != 0 => {
                (Recalcul::default(), None)
            }
            0 if !boutons.is_empty() => {
                let c = boutons.get(tirer(boutons.len(), alea >> 8)).copied();
                let g = c.and_then(|c| {
                    if let Some(SorteNoeudPage::Bouton { geste, .. }) = self.noeud(c) {
                        Some(geste.clone())
                    } else {
                        None
                    }
                });
                (Recalcul::default(), g)
            }
            1 if !champs.is_empty() => {
                let Some(c) = champs.get(tirer(champs.len(), alea >> 8)).copied() else {
                    return (Recalcul::default(), None);
                };
                let t = TEXTES_DU_SINGE
                    .get(tirer(TEXTES_DU_SINGE.len(), alea >> 24))
                    .copied()
                    .unwrap_or("");
                let r = self.taper(t_us, c, t);
                let g = match self.entree(c) {
                    Action::Geste(g) if (alea >> 40) % 2 == 0 => Some(g),
                    Action::Geste(_) | Action::Rien => None,
                };
                (r, g)
            }
            2 if !les_choix.is_empty() => {
                let (c, k) = les_choix
                    .get(tirer(les_choix.len(), alea >> 8))
                    .copied()
                    .unwrap_or((Cle::du_fil(0), 1));
                // Une option hors de la liste, de loin en loin : le jeu doit le supporter.
                let opt = u32::try_from(tirer(k.saturating_add(1), alea >> 24)).unwrap_or(0);
                (self.choisir(t_us, c, opt), None)
            }
            3 if !croix_de_la_page.is_empty() => {
                let (c, ds) = croix_de_la_page
                    .get(tirer(croix_de_la_page.len(), alea >> 8))
                    .copied()
                    .unwrap_or((Cle::du_fil(0), Directions::Quatre));
                let toutes = directions_de(ds);
                let d = toutes
                    .get(tirer(toutes.len(), alea >> 24))
                    .copied()
                    .unwrap_or(Direction8::Centre);
                (self.diriger(t_us, c, d), None)
            }
            4 if !maintenus.is_empty() => {
                let c = maintenus
                    .get(tirer(maintenus.len(), alea >> 8))
                    .copied()
                    .unwrap_or(Cle::du_fil(0));
                let appuye = if self.locaux.appuyes.contains(&c) {
                    Oui::Non
                } else {
                    Oui::Oui
                };
                (self.maintenir(t_us, c, appuye), None)
            }
            _ => (Recalcul::default(), None),
        }
    }
}

/// Le singe n'appuie sur un bouton tiré qu'une fois sur ce nombre (choisi : le rythme
/// d'avant la fusion du lot 11, où le bouton était une sorte sur cinq). Fin du lot 12 : la
/// sorte tirée parmi ce que la page offre, une page qui n'offre que des boutons voyait le
/// singe appuyer à chacun de ses tours (700 ms) ; au salon de Punchline, le singe devenu VIP
/// lançait la partie (« Tout le monde est là ») puis l'annulait (« Attends, on annule ! »),
/// sans fin, et aucune partie ne commençait (`soiree punchline`, deux passages, 400 s
/// chacun ; reproduit en mémoire par `soiree_en_memoire --singe-attend`).
pub const SINGE_BOUTON_UN_SUR: u64 = 5;

/// La sorte de geste du singe (0 un bouton, 1 un champ, 2 un choix, 3 une croix, 4 un
/// maintenu), tirée par `alea` parmi celles que la page offre ; `u64::MAX` si elle n'en
/// offre aucune. Fusion du lot 11 : un tirage parmi les cinq perdait son tour sur une page
/// qui n'en offre que deux (le champ et le bouton de devine), et le singe d'une soirée de
/// devine ne jouait qu'une fois, ou jamais, selon la graine.
fn sorte_du_singe(offertes: [bool; 5], alea: u64) -> u64 {
    let sortes: Vec<u64> = offertes
        .iter()
        .zip(0u64..)
        .filter_map(|(&oui, sorte)| oui.then_some(sorte))
        .collect();
    let n = u64::try_from(sortes.len()).unwrap_or(1).max(1);
    sortes
        .get(usize::try_from(alea.checked_rem(n).unwrap_or(0)).unwrap_or(0))
        .copied()
        .unwrap_or(u64::MAX)
}

/// § 8.1, le singe partage le salon (lot 12, fin) : le salon est au complet quand chaque
/// autre téléphone natif de la soirée a vu accuser un premier geste (`accuses`, le
/// dernier geste accusé de chacun, [`Appareil::accuse`]). Le banc ne connaît pas le jeu :
/// un geste accusé est ce qu'il sait de « ce téléphone a rejoint ». Avant, le singe ne
/// joue rien : à la fusion du lot 11, le singe devenu VIP du salon de Punchline lançait
/// chaque partie 5 à 45 pas après l'ouverture, avant que les autres natifs aient
/// rejoint, et un natif resté sur « pas de la partie » ne recevait aucun coup du bot.
/// Une fois au complet, le salon le reste (l'appelant garde la réponse) : une partie
/// relancée garde sa table.
pub fn salon_au_complet(accuses: impl IntoIterator<Item = u64>) -> bool {
    accuses.into_iter().all(|a| a > 0)
}

/// Ce qu'un joueur de page a fait des coups du bot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ComptesJoueur {
    /// Les gestes du bot partis d'un bouton ou d'une Entrée de la page.
    pub gestes_par_la_page: u64,
    /// Les gestes du bot qu'aucun chemin de la page ne porte : non envoyés.
    pub gestes_sans_chemin: u64,
    /// Les tenus du bot que la page a donnés.
    pub tenus_par_la_page: u64,
    /// Les tenus du bot qu'aucune croix, aucun maintenu ni aucun choix ne donne exactement,
    /// remplacés par le plus proche que la page donne ([`PageDuTelephone::approcher_le_tenu`]).
    pub tenus_approches: u64,
    /// Les tenus du bot qu'aucune croix, aucun maintenu ni aucun choix ne donne, ni de près.
    pub tenus_sans_chemin: u64,
    /// Les gestes partis du singe.
    pub gestes_du_singe: u64,
    /// Les tours que le singe a joués, geste parti ou non (un retenu n'en joue aucun).
    pub tours_du_singe: u64,
    /// Les gestes de la page refusés par l'appareil (file pleine, trop long).
    pub refuses: u64,
    /// Les files pleines (SAL-07), dans `refuses`.
    pub files_pleines: u64,
    /// Les pages refaites sur un autre module : la salle a changé de jeu (lot 12, fin).
    pub changements: u64,
}

/// Un téléphone natif du banc qui passe **par la page** (§ 8.1, lot 12) : l'instance du
/// téléphone (le module reçu de l'autorité, chargé par la fabrique de la coque) appelle
/// `Page` ; les coups du bot partent du bouton qui les porte (ou de la croix, du
/// maintenu, du choix qui donnent le tenu) ; un singe, s'il est armé, joue au hasard.
pub struct JoueurDePage<F: Fabrique> {
    page: Option<PageDuTelephone<InstanceDePage<F>>>,
    singe: Option<u64>,
    prochain_singe_us: u64,
    /// Le singe attend que le salon soit au complet ([`salon_au_complet`]) : il ne joue
    /// aucun geste tant que c'est vrai.
    singe_retenu: Oui,
    /// L'empreinte du module dont la page est faite : un autre module reçu (la salle a
    /// changé de jeu, lot 12, fin) refait la page.
    module_de_la_page: Option<[u8; 32]>,
    /// Ce qu'il a fait.
    pub comptes: ComptesJoueur,
    /// L'instance de la page n'a pas pu s'ouvrir : pourquoi.
    pub panne: Option<String>,
}

impl<F: Fabrique> Default for JoueurDePage<F> {
    fn default() -> Self {
        JoueurDePage {
            page: None,
            singe: None,
            prochain_singe_us: 0,
            singe_retenu: Oui::Non,
            module_de_la_page: None,
            comptes: ComptesJoueur::default(),
            panne: None,
        }
    }
}

impl<F: Fabrique> JoueurDePage<F> {
    /// Un singe : un geste au hasard toutes les `periode_us` (et les coups du bot, s'il en
    /// reçoit, par la page aussi).
    pub fn singe(periode_us: u64) -> JoueurDePage<F> {
        JoueurDePage {
            singe: Some(periode_us.max(1)),
            ..JoueurDePage::default()
        }
    }

    /// Retient le singe (`Oui`) ou le laisse jouer (`Non`) : retenu, il ne joue aucun
    /// geste au hasard ; les coups du bot passent toujours. § 8.1, le singe partage le
    /// salon : le banc le retient tant que [`salon_au_complet`] ne l'a pas dit.
    pub const fn retenir_le_singe(&mut self, retenu: Oui) {
        self.singe_retenu = retenu;
    }

    /// La salle a changé de jeu (§ 2.8, lot 12, fin) : le module de la page n'est plus
    /// celui que l'appareil reçoit ou a reçu (`recu`) ; la page sera refaite sur le nouveau.
    fn suivre_le_module(&mut self, recu: Option<[u8; 32]>) {
        if self.module_de_la_page.is_some() && recu != self.module_de_la_page {
            self.page = None;
            self.panne = None;
            self.module_de_la_page = None;
            self.comptes.changements = self.comptes.changements.saturating_add(1);
        }
    }

    /// La page, si le module est arrivé.
    pub fn page(&mut self) -> Option<&mut PageDuTelephone<InstanceDePage<F>>> {
        self.page.as_mut()
    }

    fn appliquer<H: Hasard>(appareil: &mut Appareil<H>, t_us: u64, r: &Recalcul) {
        if let Some(t) = &r.tenu {
            appareil.tenir(t_us, t);
            if r.tenu_relache {
                appareil.taire_la_redite();
            }
        }
    }

    fn envoyer<H: Hasard>(&mut self, appareil: &mut Appareil<H>, t_us: u64, g: &[u8]) -> bool {
        match appareil.geste(t_us, g) {
            Ok(_) => true,
            Err(e) => {
                self.comptes.refuses = self.comptes.refuses.saturating_add(1);
                if e == crate::appareil::Envoi::FilePleine {
                    self.comptes.files_pleines = self.comptes.files_pleines.saturating_add(1);
                }
                false
            }
        }
    }

    /// Un réveil du téléphone, après ce qu'il a reçu : la page suit la vue, joue les coups
    /// du bot et le singe, laisse partir le tenu que le débit retenait, relâche sur une
    /// coupure (P28). `fabriquer` donne la fabrique du module reçu (appelée une fois).
    pub fn jouer<H: Hasard, S: Hasard>(
        &mut self,
        t_us: u64,
        appareil: &mut Appareil<H>,
        fabriquer: impl FnOnce(&[u8]) -> Option<F>,
        hasard: &mut S,
    ) {
        self.suivre_le_module(appareil.module().map(|(e, _)| e));
        if self.page.is_none() && self.panne.is_none() {
            let Some((empreinte, module)) = appareil.module() else {
                // Sans module, les coups du bot n'ont pas de page.
                let n = appareil.prendre_les_coups().len();
                self.comptes.gestes_sans_chemin = self
                    .comptes
                    .gestes_sans_chemin
                    .saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
                return;
            };
            self.module_de_la_page = Some(empreinte);
            match fabriquer(module).map(InstanceDePage::ouvrir) {
                Some(Ok(i)) => {
                    let tenu = match &i.declaration().telephones {
                        ApplicableFil::Oui(t) => Some(t.tenu.clone()),
                        ApplicableFil::SansObjet(_) => None,
                    };
                    self.page = Some(PageDuTelephone::new(
                        i,
                        Locaux::new(EcranTelephone::ORDINAIRE, Capacites::AUCUNE),
                        tenu.as_ref(),
                    ));
                }
                Some(Err(e)) => self.panne = Some(e.to_string()),
                None => self.panne = Some(String::from("le module reçu ne se charge pas")),
            }
        }
        let Some(mut page) = self.page.take() else {
            // Sans page, les coups du bot n'ont aucun chemin.
            let n = appareil.prendre_les_coups().len();
            self.comptes.gestes_sans_chemin = self
                .comptes
                .gestes_sans_chemin
                .saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
            return;
        };
        if let Some((pas, montre)) = appareil.vue() {
            let montre = montre.to_vec();
            let r = page.vue_recue(t_us, pas, &montre);
            Self::appliquer(appareil, t_us, &r);
        }
        for coup in appareil.prendre_les_coups() {
            match coup {
                CoupRecu::Geste(g) => match page.chercher_le_geste(t_us, &g) {
                    Some(r) => {
                        Self::appliquer(appareil, t_us, &r);
                        if page.porteur_du_geste(&g).is_some() && self.envoyer(appareil, t_us, &g) {
                            self.comptes.gestes_par_la_page =
                                self.comptes.gestes_par_la_page.saturating_add(1);
                        }
                    }
                    None => {
                        self.comptes.gestes_sans_chemin =
                            self.comptes.gestes_sans_chemin.saturating_add(1);
                    }
                },
                CoupRecu::Tenu(t) => {
                    if let Some(r) = page.chercher_le_tenu(t_us, &t) {
                        Self::appliquer(appareil, t_us, &r);
                        self.comptes.tenus_par_la_page =
                            self.comptes.tenus_par_la_page.saturating_add(1);
                    } else if let Some(r) = page.approcher_le_tenu(t_us, &t) {
                        Self::appliquer(appareil, t_us, &r);
                        self.comptes.tenus_approches =
                            self.comptes.tenus_approches.saturating_add(1);
                    } else {
                        self.comptes.tenus_sans_chemin =
                            self.comptes.tenus_sans_chemin.saturating_add(1);
                    }
                }
            }
        }
        if let Some(periode) = self.singe
            && self.singe_retenu == Oui::Non
            && t_us >= self.prochain_singe_us
        {
            self.prochain_singe_us = t_us.saturating_add(periode);
            self.comptes.tours_du_singe = self.comptes.tours_du_singe.saturating_add(1);
            let (r, g) = page.singe(t_us, hasard);
            Self::appliquer(appareil, t_us, &r);
            if let Some(g) = g
                && self.envoyer(appareil, t_us, &g)
            {
                self.comptes.gestes_du_singe = self.comptes.gestes_du_singe.saturating_add(1);
            }
        }
        if appareil.coupe(t_us) {
            let r = page.coupure(t_us);
            Self::appliquer(appareil, t_us, &r);
        }
        if let Some(t) = page.reveiller(t_us) {
            appareil.tenir(t_us, &t);
            if page.relache(Some(&t)) {
                appareil.taire_la_redite();
            }
        }
        self.page = Some(page);
    }
}

/// La proximité d'un geste à celui qu'on cherche (le banc seulement) : de même longueur
/// d'abord, puis le plus d'octets égaux à la même place (de même longueur), ou le plus
/// long début et la plus longue fin communs (sinon).
type Proximite = (bool, usize);

fn proximite(a: &[u8], cible: &[u8]) -> Proximite {
    if a.len() == cible.len() {
        return (true, a.iter().zip(cible).filter(|(x, y)| x == y).count());
    }
    let debut = a.iter().zip(cible).take_while(|(x, y)| x == y).count();
    let fin = a
        .iter()
        .rev()
        .zip(cible.iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let court = a.len().min(cible.len());
    (false, debut.saturating_add(fin).min(court))
}

/// La meilleure proximité des gestes que porte cette page (boutons et Entrées).
fn proximite_des_porteurs(page: &PageFil, cible: &[u8]) -> Proximite {
    page.noeuds
        .iter()
        .filter_map(|n| {
            if let SorteNoeudPage::Bouton { geste, .. } = &n.sorte {
                return Some(proximite(geste, cible));
            }
            if let SorteNoeudPage::Champ {
                entree: Some(g), ..
            } = &n.sorte
            {
                return Some(proximite(g, cible));
            }
            None
        })
        .max()
        .unwrap_or((false, 0))
}

/// Les choix de cette page, et leur nombre d'options.
fn choix_de(page: &PageFil) -> Vec<(Cle, usize)> {
    page.noeuds
        .iter()
        .filter_map(|n| {
            if let SorteNoeudPage::Choix { options, .. } = &n.sorte {
                Some((n.cle, options.len()))
            } else {
                None
            }
        })
        .collect()
}

/// Ces octets lus comme des `f32` (petit-boutiens, comme postcard les écrit), s'ils en sont
/// un nombre entier et tous finis ; sinon rien.
fn flottants(o: &[u8]) -> Option<Vec<f32>> {
    if o.is_empty() || !o.len().is_multiple_of(4) {
        return None;
    }
    let v: Vec<f32> = o
        .as_chunks::<4>()
        .0
        .iter()
        .map(|c| f32::from_le_bytes(*c))
        .collect();
    v.iter().all(|x| x.is_finite()).then_some(v)
}

/// Les directions d'une croix, `Centre` compris.
fn directions_de(ds: Directions) -> Vec<Direction8> {
    match ds {
        Directions::Huit => Direction8::TOUTES.to_vec(),
        Directions::Quatre => vec![
            Direction8::Centre,
            Direction8::Haut,
            Direction8::Droite,
            Direction8::Bas,
            Direction8::Gauche,
        ],
    }
}

/// Lit un entier variable de postcard (LEB128, au plus 10 octets).
fn varint(o: &[u8]) -> Option<(usize, usize)> {
    let mut v: u64 = 0;
    for (i, b) in o.iter().take(10).enumerate() {
        let bas = u64::from(b & 0x7f);
        v |= bas.checked_shl(u32::try_from(i.saturating_mul(7)).ok()?)?;
        if b & 0x80 == 0 {
            return Some((usize::try_from(v).ok()?, i.saturating_add(1)));
        }
    }
    None
}

/// Les suites de chaînes postcard contiguës (longueur, puis UTF-8) qu'on lit dans ces
/// octets, chacune d'au moins une chaîne non vide ; les plus longues d'abord, puis celles
/// qui finissent le plus tard (les textes d'un geste sont souvent à sa fin).
fn chaines(o: &[u8]) -> Vec<Vec<String>> {
    let lire = |i: usize| -> Option<(String, usize)> {
        let reste = o.get(i..)?;
        let (n, l) = varint(reste)?;
        let fin = l.checked_add(n)?;
        let s = core::str::from_utf8(reste.get(l..fin)?).ok()?;
        Some((String::from(s), i.saturating_add(fin)))
    };
    let mut toutes: Vec<(usize, Vec<String>)> = Vec::new();
    for debut in 0..o.len() {
        let mut i = debut;
        let mut suite = Vec::new();
        while let Some((s, apres)) = lire(i) {
            if apres == i {
                break;
            }
            suite.push(s);
            i = apres;
        }
        if suite.iter().any(|s| !s.is_empty()) {
            toutes.push((i, suite));
        }
    }
    toutes.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(b.0.cmp(&a.0)));
    toutes.dedup_by(|a, b| a.1 == b.1);
    toutes.into_iter().map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeu_abi::{NoeudPage, StylePage, TenuPage};

    /// Une page écrite à la main : un maintenu `M` tant que la vue vaut `[1]`, rien sinon ;
    /// le tenu dit si `M` est appuyé (`[1]`) ou non (`[0]`, sa valeur relâchée).
    struct FausseePage;

    const M: Cle = Cle::du_fil(7);

    impl Pager for FausseePage {
        fn page(&mut self, montre: &[u8], _: Pas, l: &LocauxFil<'_>) -> Result<PageFil, String> {
            let mut noeuds = vec![NoeudPage {
                cle: Cle::du_fil(1),
                parent: None,
                sorte: SorteNoeudPage::Colonne,
            }];
            if montre == [1] {
                noeuds.push(NoeudPage {
                    cle: M,
                    parent: Some(0),
                    sorte: SorteNoeudPage::Maintenu {
                        nom: String::from("Tiens"),
                        style: StylePage::ORDINAIRE,
                    },
                });
            }
            let appuye = l.appuyes.contains(&M);
            Ok(PageFil {
                noeuds,
                tenu: Some(TenuPage {
                    octets: vec![u8::from(appuye)],
                    relache: Some(vec![0]),
                }),
                retenir: None,
            })
        }
    }

    fn regles(debit: u16, fraicheur: Oui) -> jeu_abi::ReglesTenuFil {
        jeu_abi::ReglesTenuFil {
            fraicheur: if fraicheur == Oui::Oui {
                ApplicableFil::Oui(jeu_abi::Duree::ms(450))
            } else {
                ApplicableFil::SansObjet(String::from("essai"))
            },
            age_max: ApplicableFil::SansObjet(String::from("essai")),
            debit_max: debit,
        }
    }

    fn page(debit: u16, fraicheur: Oui) -> PageDuTelephone<FausseePage> {
        let r = regles(debit, fraicheur);
        PageDuTelephone::new(
            FausseePage,
            Locaux::new(EcranTelephone::ORDINAIRE, Capacites::AUCUNE),
            Some(&r),
        )
    }

    /// REM-23 : la page change et n'a plus la clé du maintenu sous le doigt. Le commun lâche
    /// ce doigt avant de poser la page : le tenu relâché part aussitôt, et la clé est dite
    /// lâchée (le doigt resté posé ne compte plus jusqu'à ce qu'il se relève).
    #[test]
    fn un_maintenu_dont_la_cle_disparait_est_relache_rem23() {
        let mut p = page(0, Oui::Non);
        let r = p.vue_recue(0, Pas::ZERO, &[1]);
        assert_eq!(r.tenu, Some(vec![0]));
        let r = p.maintenir(10, M, Oui::Oui);
        assert_eq!(r.tenu, Some(vec![1]));
        // La vue change : plus de maintenu.
        let r = p.vue_recue(20, Pas::du_fil(1), &[2]);
        assert_eq!(r.laches, [M]);
        assert_eq!(r.tenu, Some(vec![0]), "le relâchement part aussitôt");
        assert_eq!(p.locaux().appuyes.len(), 0);
        assert_eq!(p.comptes().laches_rem23, 1);
        // La clé revient : le doigt resté posé ne compte pas, rien n'est appuyé.
        let r = p.vue_recue(30, Pas::du_fil(2), &[1]);
        assert_eq!(r.laches, Vec::<Cle>::new());
        assert_eq!(r.tenu, None, "le tenu reste relâché");
    }

    /// Même page, la clé gardée : le geste continue (REM-23).
    #[test]
    fn un_maintenu_dont_la_cle_reste_continue() {
        let mut p = page(0, Oui::Non);
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[1]);
        let _: Recalcul = p.maintenir(10, M, Oui::Oui);
        let r = p.vue_recue(20, Pas::du_fil(1), &[1, 0][..1]);
        assert_eq!(r.laches, Vec::<Cle>::new());
        let r = p.vue_recue(30, Pas::du_fil(2), &[1]);
        assert_eq!(r.laches, Vec::<Cle>::new());
        assert!(p.locaux().appuyes.contains(&M));
    }

    /// REM-15 : au plus `debit_max` changements par seconde ; le relâchement passe toujours.
    #[test]
    fn le_debit_retient_les_tenus_mais_pas_le_relachement_rem15() {
        let mut p = page(2, Oui::Non);
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[1]);
        let r = p.maintenir(1_000, M, Oui::Oui);
        assert_eq!(r.tenu, None, "retenu : 500 ms depuis le dernier");
        assert_eq!(p.reveiller(500_000), Some(vec![1]));
        let r = p.maintenir(500_001, M, Oui::Non);
        assert_eq!(r.tenu, Some(vec![0]), "le relâchement passe");
    }

    /// REM-11 : l'arrière-plan lâche ce que le doigt tient ; P28 : la coupure aussi, mais
    /// seulement pour un tenu qui déclare une fraîcheur.
    #[test]
    fn l_arriere_plan_et_la_coupure_relachent_rem11_p28() {
        let mut p = page(0, Oui::Non);
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[1]);
        let _: Recalcul = p.maintenir(1, M, Oui::Oui);
        assert_eq!(p.coupure(2), Recalcul::default(), "sans fraîcheur, rien");
        let r = p.arriere_plan(3);
        assert_eq!((r.tenu, r.laches), (Some(vec![0]), vec![M]));
        let mut p = page(0, Oui::Oui);
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[1]);
        let _: Recalcul = p.maintenir(1, M, Oui::Oui);
        let r = p.coupure(2);
        assert_eq!((r.tenu, r.laches), (Some(vec![0]), vec![M]));
        assert_eq!(p.comptes().coupures, 1);
    }

    /// Un sélecteur écrit à la main, comme celui de Sidequest : un personnage (3 options),
    /// une couleur dont la clé dépend du personnage (4 options), un champ de prénom, et un
    /// bouton dont le geste porte les trois : `[9, longueur, prénom…, personnage, couleur]`.
    struct FauxSelecteur;

    const PERSONNAGE: Cle = Cle::du_fil(20);
    const PRENOM: Cle = Cle::du_fil(21);
    const MOI: Cle = Cle::du_fil(22);

    fn couleur(personnage: u32) -> Cle {
        Cle::du_fil(u64::from(personnage).saturating_add(30))
    }

    fn options(n: u64) -> Vec<(Cle, String)> {
        (0..n)
            .map(|i| (Cle::du_fil(i.saturating_add(100)), format!("o{i}")))
            .collect()
    }

    fn geste_du_selecteur(prenom: &str, personnage: u8, couleur: u8) -> Vec<u8> {
        let mut g = vec![9u8, u8::try_from(prenom.len()).unwrap()];
        g.extend_from_slice(prenom.as_bytes());
        g.extend_from_slice(&[personnage, couleur]);
        g
    }

    impl Pager for FauxSelecteur {
        fn page(&mut self, _: &[u8], _: Pas, l: &LocauxFil<'_>) -> Result<PageFil, String> {
            let lire = |c: Cle| l.choix.iter().find(|(k, _)| *k == c).map_or(0, |(_, n)| *n);
            let p = lire(PERSONNAGE);
            let c = lire(couleur(p));
            let prenom = l
                .champs
                .iter()
                .find(|(k, _)| *k == PRENOM)
                .map_or("", |(_, t)| *t);
            let noeud = |cle, sorte| NoeudPage {
                cle,
                parent: Some(0),
                sorte,
            };
            let noeuds = vec![
                NoeudPage {
                    cle: Cle::du_fil(1),
                    parent: None,
                    sorte: SorteNoeudPage::Colonne,
                },
                noeud(
                    PERSONNAGE,
                    SorteNoeudPage::Choix {
                        options: options(3),
                        balayable: Oui::Oui,
                    },
                ),
                noeud(
                    couleur(p),
                    SorteNoeudPage::Choix {
                        options: options(4),
                        balayable: Oui::Non,
                    },
                ),
                noeud(
                    PRENOM,
                    SorteNoeudPage::Champ {
                        limite: 20,
                        unite: jeu_abi::Unite::PointsDeCode,
                        exemple: String::new(),
                        clavier: jeu_abi::Clavier::Prenom,
                        entree: None,
                    },
                ),
                noeud(
                    MOI,
                    SorteNoeudPage::Bouton {
                        nom: String::from("C'est moi"),
                        geste: geste_du_selecteur(
                            prenom,
                            u8::try_from(p).unwrap(),
                            u8::try_from(c).unwrap(),
                        ),
                        quand: Declencheur::Relache,
                        style: StylePage::ORDINAIRE,
                    },
                ),
            ];
            Ok(PageFil {
                noeuds,
                tenu: None,
                retenir: None,
            })
        }
    }

    /// Correction du lot 12 : le geste d'un bot qui porte un prénom **et** des options
    /// (le `Rejoindre` de Sidequest, un look tiré au hasard) a son chemin sur la page : le
    /// prénom tapé, puis chaque choix tourné, même celui dont la clé dépend d'un autre.
    #[test]
    fn un_geste_porte_par_des_choix_trouve_son_chemin() {
        let mut p = PageDuTelephone::new(
            FauxSelecteur,
            Locaux::new(EcranTelephone::ORDINAIRE, Capacites::AUCUNE),
            None,
        );
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[0]);
        for (personnage, c) in [(2u8, 3u8), (1, 0), (0, 2)] {
            let cible = geste_du_selecteur("Léa", personnage, c);
            assert!(
                p.chercher_le_geste(1, &cible).is_some(),
                "aucun chemin pour {cible:?}"
            );
            assert_eq!(p.porteur_du_geste(&cible).map(|(c, _)| c), Some(MOI));
        }
        // Un geste qu'aucune option ne fait n'a pas de chemin.
        assert!(
            p.chercher_le_geste(2, &geste_du_selecteur("Léa", 7, 0))
                .is_none()
        );
    }

    /// Une croix en huit directions dont le tenu est `(x, y)` en `f32`.
    struct FausseCroix;

    const CROIX: Cle = Cle::du_fil(40);

    impl Pager for FausseCroix {
        fn page(&mut self, _: &[u8], _: Pas, l: &LocauxFil<'_>) -> Result<PageFil, String> {
            let d = l
                .croix
                .iter()
                .find(|(k, _)| *k == CROIX)
                .map_or(Direction8::Centre, |(_, d)| *d);
            let (x, y): (f32, f32) = match d {
                Direction8::Centre => (0.0, 0.0),
                Direction8::Haut => (0.0, -1.0),
                Direction8::HautDroite => (1.0, -1.0),
                Direction8::Droite => (1.0, 0.0),
                Direction8::BasDroite => (1.0, 1.0),
                Direction8::Bas => (0.0, 1.0),
                Direction8::BasGauche => (-1.0, 1.0),
                Direction8::Gauche => (-1.0, 0.0),
                Direction8::HautGauche => (-1.0, -1.0),
            };
            let mut octets = x.to_le_bytes().to_vec();
            octets.extend_from_slice(&y.to_le_bytes());
            Ok(PageFil {
                noeuds: vec![
                    NoeudPage {
                        cle: Cle::du_fil(1),
                        parent: None,
                        sorte: SorteNoeudPage::Colonne,
                    },
                    NoeudPage {
                        cle: CROIX,
                        parent: Some(0),
                        sorte: SorteNoeudPage::Croix {
                            directions: Directions::Huit,
                        },
                    },
                ],
                tenu: Some(TenuPage {
                    octets,
                    relache: Some(vec![0; 8]),
                }),
                retenir: None,
            })
        }
    }

    fn direction(x: f32, y: f32) -> Vec<u8> {
        let mut o = x.to_le_bytes().to_vec();
        o.extend_from_slice(&y.to_le_bytes());
        o
    }

    /// Correction du lot 12 : une direction continue qu'aucune des huit ne donne n'a pas
    /// de chemin exact ; la plus proche la remplace, comme un pouce la pousserait. Un tenu
    /// qui n'est pas fait de flottants n'a pas de plus proche.
    #[test]
    fn un_tenu_continu_prend_la_direction_la_plus_proche() {
        let mut p = PageDuTelephone::new(
            FausseCroix,
            Locaux::new(EcranTelephone::ORDINAIRE, Capacites::AUCUNE),
            None,
        );
        let _: Recalcul = p.vue_recue(0, Pas::ZERO, &[0]);
        let cible = direction(0.9, 0.3);
        assert!(p.chercher_le_tenu(1, &cible).is_none(), "pas exact");
        assert!(p.approcher_le_tenu(2, &cible).is_some());
        assert_eq!(
            p.locaux().croix.get(&CROIX).copied(),
            Some(Direction8::Droite)
        );
        assert!(p.approcher_le_tenu(3, &direction(-0.6, -0.7)).is_some());
        assert_eq!(
            p.locaux().croix.get(&CROIX).copied(),
            Some(Direction8::HautGauche)
        );
        assert!(p.approcher_le_tenu(4, &[1, 2, 3]).is_none());
        assert!(p.approcher_le_tenu(5, &[0xff; 8]).is_none(), "NaN");
    }

    /// SAL-07 (correction du lot 12, `FilePleine` jamais provoqué au lot 12) : un téléphone
    /// natif qui passe par la page et dont la file est pleine ne perd rien en silence :
    /// chaque geste refusé se compte, chez lui (`refuses`, `files_pleines`) comme chez son
    /// appareil, et ne prend pas de numéro ; la file reste à sa limite.
    #[test]
    fn une_file_pleine_se_dit_et_se_compte_sal07() {
        use crate::chiffre::{ClePublique, Fragment, SecretDEntree};
        let mut a = Appareil::new(
            Fragment {
                secret: SecretDEntree::depuis_octets([1; 16]),
                autorite: ClePublique([2; 32]),
            },
            liaison::CodeSalle([1, 2, 3, 4]),
            ([3; 32], 9),
            Capacites::AUCUNE,
            None,
            crate::simulation::Graine::new("essai", 1),
        );
        let mut j = JoueurDePage::<moteur_wasmtime::FabriqueWasm>::default();
        let limite = crate::appareil::FILE_AVANT_ACCUEIL;
        for _ in 0..limite {
            assert!(j.envoyer(&mut a, 0, b"g"));
        }
        for _ in 0..5 {
            assert!(!j.envoyer(&mut a, 0, b"g"), "la file est pleine");
        }
        assert_eq!((j.comptes.refuses, j.comptes.files_pleines), (5, 5));
        assert_eq!(a.comptes().files_pleines, 5);
        assert_eq!(a.en_file(), limite);
        assert_eq!(a.comptes().gestes, u64::try_from(limite).unwrap());
    }

    /// Fin du lot 12 : sur une page qui n'offre qu'un bouton, le singe n'y appuie qu'un tour
    /// sur `SINGE_BOUTON_UN_SUR` environ (de 15 à 25 % de 2 000 tours) : à chaque tour, il
    /// annulait chaque départ de Punchline.
    #[test]
    fn le_singe_n_appuie_qu_un_tour_sur_cinq_sur_un_bouton() {
        struct UnBouton;
        impl Pager for UnBouton {
            fn page(&mut self, _: &[u8], _: Pas, _: &LocauxFil<'_>) -> Result<PageFil, String> {
                Ok(PageFil {
                    noeuds: vec![
                        NoeudPage {
                            cle: Cle::du_fil(1),
                            parent: None,
                            sorte: SorteNoeudPage::Colonne,
                        },
                        NoeudPage {
                            cle: Cle::du_fil(2),
                            parent: Some(0),
                            sorte: SorteNoeudPage::Bouton {
                                nom: String::from("Attends, on annule !"),
                                geste: vec![7],
                                quand: jeu_abi::Declencheur::Relache,
                                style: StylePage::ORDINAIRE,
                            },
                        },
                    ],
                    tenu: None,
                    retenir: None,
                })
            }
        }
        let mut p = PageDuTelephone::new(
            UnBouton,
            Locaux::new(EcranTelephone::ORDINAIRE, Capacites::AUCUNE),
            None,
        );
        let _: Recalcul = p.vue_recue(0, Pas::du_fil(1), &[1]);
        let mut h = crate::simulation::Graine::new("singe", 3);
        let appuis = (0..2_000u64)
            .filter(|t| p.singe(*t, &mut h).1.is_some())
            .count();
        assert!(
            (300..=500).contains(&appuis),
            "{appuis} appuis sur 2 000 tours"
        );
    }

    /// Fusion du lot 11 : le singe tire sa sorte de geste parmi celles que la page offre,
    /// jamais une qu'elle n'a pas ; sur une page qui n'offre rien, aucune.
    #[test]
    fn le_singe_tire_parmi_ce_que_la_page_offre() {
        // Le champ et le bouton de devine : chaque tirage tombe sur l'un ou l'autre.
        let devine = [true, true, false, false, false];
        for alea in 0..64 {
            assert!(super::sorte_du_singe(devine, alea) <= 1, "{alea}");
        }
        assert_eq!(super::sorte_du_singe(devine, 0), 0);
        assert_eq!(super::sorte_du_singe(devine, 1), 1);
        // Une croix seule.
        for alea in 0..8 {
            assert_eq!(
                super::sorte_du_singe([false, false, false, true, false], alea),
                3
            );
        }
        assert_eq!(super::sorte_du_singe([false; 5], 7), u64::MAX);
    }

    #[test]
    fn les_chaines_d_un_geste_se_lisent() {
        // Variant 3, un u32 (5), une liste de deux textes.
        let mut o = vec![3u8, 5, 2, 3];
        o.extend_from_slice(b"abc");
        o.push(2);
        o.extend_from_slice(b"de");
        let c = chaines(&o);
        assert!(
            c.iter()
                .any(|s| s == &[String::from("abc"), String::from("de")])
        );
    }
}
