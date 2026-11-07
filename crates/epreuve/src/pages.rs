//! E21, « Pages » (PHASE-3, § 2.10 et § 8.3 ; lot 6) : la page du téléphone, appelée
//! dans wasmtime sur les vues que les téléphones des soirées ont reçues.
//!
//! - **Chaque vue donne une page valide** : `Appel::Page` sur la vue telle qu'elle est
//!   partie vers le téléphone (une `MontreFil` encodée, et son pas), avec des locaux
//!   vides, ne faute pas, rend une `PageFil`, et cette page passe [`valider_page`]
//!   (REM-57).
//! - **Des locaux tirés au hasard ne font jamais fauter** : pour chaque vue, quelques
//!   locaux tirés (taille et orientation de l'écran, textes hostiles dans les champs de
//!   la page, choix hors de leurs options, maintenus appuyés, croix tenues, octets
//!   retenus au hasard, capacités), et des clés que la page n'a pas.
//! - **Les gestes et les tenus d'une page passent `Valider`** : chaque bouton, chaque
//!   geste d'un champ sur Entrée, le tenu et sa valeur relâchée.
//! - **Aucune page n'est vide** chez un jeu qui a des téléphones (PHASE-3, § 2.10 et
//!   § 8.3 ; armé au lot 7) : une page sans aucun nœud laisserait le téléphone sans
//!   rien à montrer ni à toucher, sans un mot (REM-38). C'est d'abord la page par défaut
//!   de `Jeu::page`, celle d'un jeu qui n'a pas écrit la sienne : le rapport la nomme
//!   quand chaque page est vide. Une page vide pour quelques vues seulement (un écran
//!   oublié) échoue de même.
//!
//! E19 juge en plus le coût de chaque `Page` appelée ici, sur un budget à part : celui
//! d'une lecture, divisé par la marge du téléphone (`budgets.rs`).
//!
//! # Quelles vues
//!
//! Les vues distinctes que les téléphones de toutes les soirées ont reçues, chacune une
//! fois. Au-delà de [`VUES_MAX`], l'épreuve garde celles dont l'empreinte (FNV-1a 64)
//! est la plus petite : un échantillon sans hasard, qui ne tire rien des soirées et ne
//! change donc pas ce qu'elles jouent. Le rapport dit combien de vues distinctes ont été
//! vues, et combien sont éprouvées.

use std::collections::{BTreeMap, BTreeSet};

use jeu_abi::{
    Appel, ApplicableFil, Capacite, Capacites, Cle, DeclarationFil, Direction8, EcranTelephone,
    LocauxFil, Marges, Orientation, PageFil, Pas, RETENU_MAX, Retour, SorteAppel, SorteNoeudPage,
    SorteOctets, Validation, decoder, encoder, fnv1a64, valider_page,
};
use moteur_wasmtime::{InstanceWasm, ModuleCharge};
use partie::{Echec, Fabrique, Instance};

use crate::soiree::BUDGETS;
use crate::sonde::Compte;
use crate::tirage::Tirage;

/// Choisi : le plus de vues distinctes que E21 éprouve. Chacune coûte quelques appels
/// de quelques dizaines de milliers d'unités : E21 reste un contrôle rapide.
pub(crate) const VUES_MAX: usize = 256;
/// Choisi : les locaux tirés au hasard pour chaque vue, en plus des locaux vides.
pub(crate) const LOCAUX_PAR_VUE: u32 = 3;

/// Des textes hostiles, pour les champs d'une page : vides, blancs, longs, emoji
/// composés, contrôles, forçages de sens, balisage (PUN-23, CAT-85).
const TEXTES: &[&str] = &[
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

/// Les vues que les téléphones ont reçues, distinctes, au plus [`VUES_MAX`] gardées.
#[derive(Debug, Clone, Default)]
pub(crate) struct VuesVues {
    /// Les vues gardées, par leur empreinte : leur pas, leurs octets.
    gardees: BTreeMap<u64, (u64, Vec<u8>)>,
    /// L'empreinte de chaque vue distincte vue.
    distinctes: BTreeSet<u64>,
}

impl VuesVues {
    /// Note une vue partie vers un téléphone, au pas `pas`.
    pub(crate) fn noter(&mut self, pas: u64, octets: &[u8]) {
        let h = fnv1a64(octets);
        if !self.distinctes.insert(h) {
            return;
        }
        let _: Option<(u64, Vec<u8>)> = self.gardees.insert(h, (pas, octets.to_vec()));
        if self.gardees.len() > VUES_MAX {
            let _: Option<(u64, (u64, Vec<u8>))> = self.gardees.pop_last();
        }
    }

    /// Ajoute les vues d'une autre soirée.
    pub(crate) fn ajouter(&mut self, autre: &VuesVues) {
        for (pas, octets) in autre.gardees.values() {
            self.noter(*pas, octets);
        }
        // Une vue vue ailleurs mais pas gardée reste comptée comme distincte.
        self.distinctes.extend(autre.distinctes.iter().copied());
    }

    /// Le nombre de vues distinctes vues.
    pub(crate) fn distinctes(&self) -> usize {
        self.distinctes.len()
    }
}

/// Ce que E21 a constaté.
#[derive(Debug, Default)]
pub(crate) struct Constat {
    pub(crate) compte: Compte,
    pub(crate) pannes: Vec<String>,
    pub(crate) notes: Vec<String>,
    /// Le carburant de chaque `Page`, pour E19.
    pub(crate) carburants: Vec<u64>,
    /// Sans objet, et pourquoi : un jeu sans téléphone n'a pas de page.
    pub(crate) sans_objet: Option<String>,
}

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(v, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// Une instance nue, après `Bonjour` et `Decrire` : celle d'un téléphone, qui n'appelle
/// jamais `Creer` (PHASE-3, § 2.10).
fn telephone(module: &ModuleCharge) -> Result<InstanceWasm, Echec> {
    let mut i = Fabrique::neuve(&mut module.fabrique()).map_err(Echec::Panne)?;
    for a in [Appel::Bonjour, Appel::Decrire] {
        let _: Vec<u8> = Instance::appeler(&mut i, a.sorte(), &encode(&a), BUDGETS.lecture)?;
    }
    Ok(i)
}

/// Des locaux tirés au hasard pour cette page : la taille et l'orientation de l'écran,
/// des textes hostiles dans ses champs, des choix hors de leurs options, des maintenus
/// appuyés, des croix tenues, des octets retenus, des capacités, et des clés qu'elle
/// n'a pas.
fn tirer<'a>(t: &mut Tirage, page: &PageFil, retenu: &'a [u8]) -> LocauxFil<'a> {
    let mut l: LocauxFil<'a> = LocauxFil::vides();
    let orientations = [
        Orientation::Portrait,
        Orientation::PortraitInverse,
        Orientation::PaysageGauche,
        Orientation::PaysageDroite,
    ];
    let marge = |t: &mut Tirage| u16::try_from(t.sous(48)).unwrap_or(0);
    l.ecran = EcranTelephone {
        largeur: u16::try_from(t.entre(240, 1400)).unwrap_or(360),
        hauteur: u16::try_from(t.entre(200, 1400)).unwrap_or(640),
        marges: Marges {
            haut: marge(t),
            droite: marge(t),
            bas: marge(t),
            gauche: marge(t),
        },
        orientation: orientations
            .get(usize::try_from(t.sous(4)).unwrap_or(0))
            .copied()
            .unwrap_or(Orientation::Portrait),
    };
    let texte = |t: &mut Tirage| {
        TEXTES
            .get(usize::try_from(t.sous(TEXTES.len() as u64)).unwrap_or(0))
            .copied()
            .unwrap_or("")
    };
    for n in &page.noeuds {
        match &n.sorte {
            SorteNoeudPage::Champ { .. } => l.champs.push((n.cle, texte(t))),
            SorteNoeudPage::Choix { options, .. } => {
                // Une option sur quatre hors de la liste.
                let haut = u64::try_from(options.len())
                    .unwrap_or(0)
                    .saturating_add(if t.une_sur(4) { 4 } else { 0 });
                l.choix
                    .push((n.cle, u32::try_from(t.sous(haut.max(1))).unwrap_or(0)));
            }
            SorteNoeudPage::Maintenu { .. } if t.une_sur(2) => l.appuyes.push(n.cle),
            SorteNoeudPage::Croix { .. } => {
                let d = Direction8::TOUTES
                    .get(usize::try_from(t.sous(9)).unwrap_or(0))
                    .copied()
                    .unwrap_or(Direction8::Centre);
                l.croix.push((n.cle, d));
            }
            SorteNoeudPage::Colonne
            | SorteNoeudPage::Rangee
            | SorteNoeudPage::Grille { .. }
            | SorteNoeudPage::Pile
            | SorteNoeudPage::Texte { .. }
            | SorteNoeudPage::Image { .. }
            | SorteNoeudPage::CompteARebours { .. }
            | SorteNoeudPage::Bouton { .. }
            | SorteNoeudPage::Maintenu { .. } => {}
        }
    }
    // Des clés que la page n'a pas : un téléphone garde des locaux d'une page passée.
    let etrangere = Cle::du_fil(t.entier());
    l.champs.push((etrangere, texte(t)));
    l.choix
        .push((etrangere, u32::try_from(t.sous(1 << 20)).unwrap_or(0)));
    l.appuyes.push(etrangere);
    l.croix.push((etrangere, Direction8::Bas));
    if t.une_sur(2) {
        let n = usize::try_from(t.sous(u64::try_from(retenu.len()).unwrap_or(0).saturating_add(1)))
            .unwrap_or(0);
        l.retenu = retenu.get(..n);
    }
    let capacite = |t: &mut Tirage| {
        if t.une_sur(2) {
            Capacite::Oui
        } else {
            Capacite::Non(jeu_abi::Absence::ContexteNonSecurise)
        }
    };
    l.capacites = Capacites {
        vibration: capacite(t),
        mouvement: capacite(t),
        son_sans_geste: capacite(t),
        ecran_allume: capacite(t),
        micro: capacite(t),
        pad_natif: capacite(t),
        orientation: l.ecran.orientation,
        prenom_propose: None,
    };
    l
}

/// Ce qu'une page envoie, à valider : chaque geste (boutons, champs sur Entrée), puis le
/// tenu et sa valeur relâchée.
fn a_valider(page: &PageFil) -> Vec<(SorteOctets, Cle, Vec<u8>)> {
    let mut v = Vec::new();
    for n in &page.noeuds {
        match &n.sorte {
            SorteNoeudPage::Bouton { geste, .. } => {
                v.push((SorteOctets::GesteTelephone, n.cle, geste.clone()));
            }
            SorteNoeudPage::Champ {
                entree: Some(g), ..
            } => v.push((SorteOctets::GesteTelephone, n.cle, g.clone())),
            SorteNoeudPage::Colonne
            | SorteNoeudPage::Rangee
            | SorteNoeudPage::Grille { .. }
            | SorteNoeudPage::Pile
            | SorteNoeudPage::Texte { .. }
            | SorteNoeudPage::Image { .. }
            | SorteNoeudPage::CompteARebours { .. }
            | SorteNoeudPage::Maintenu { .. }
            | SorteNoeudPage::Croix { .. }
            | SorteNoeudPage::Champ { entree: None, .. }
            | SorteNoeudPage::Choix { .. } => {}
        }
    }
    if let Some(t) = &page.tenu {
        v.push((SorteOctets::TenuTelephone, Cle::du_fil(0), t.octets.clone()));
        if let Some(r) = &t.relache {
            v.push((SorteOctets::TenuTelephone, Cle::du_fil(0), r.clone()));
        }
    }
    v
}

/// L'issue d'un `Page` : sa page, ou ce qui l'a empêchée.
enum Issue {
    Page(PageFil),
    /// Le jeu a fauté, ou rendu autre chose qu'une page valide : un échec de E21.
    Echec(String),
    /// Le commun ou la machine : une panne de l'outil.
    Panne(String),
}

struct Juge<'m> {
    module: &'m ModuleCharge,
    declaration: &'m DeclarationFil,
    instance: Option<InstanceWasm>,
    constat: Constat,
    /// Les pages valides, et les vides parmi elles : le nombre, et le pas de la vue de
    /// la première (lot 7).
    valides: u64,
    vides: (u64, Option<u64>),
}

impl Juge<'_> {
    fn instance(&mut self) -> Result<&mut InstanceWasm, String> {
        if self.instance.is_none() {
            match telephone(self.module) {
                Ok(i) => self.instance = Some(i),
                Err(Echec::Faute(f)) => {
                    return Err(format!(
                        "une instance de téléphone faute à son accueil : {f}"
                    ));
                }
                Err(Echec::Panne(p)) => return Err(p.to_string()),
            }
        }
        self.instance
            .as_mut()
            .ok_or_else(|| String::from("aucune instance"))
    }

    /// Appelle `Page` sur cette vue avec ces locaux.
    fn page(&mut self, pas: u64, montre: &[u8], locaux: LocauxFil<'_>) -> Issue {
        let instance = match self.instance() {
            Ok(i) => i,
            Err(e) => return Issue::Panne(e),
        };
        let appel = Appel::Page {
            montre,
            pas: Pas::du_fil(pas),
            locaux,
        };
        let rendu = Instance::appeler(
            instance,
            SorteAppel::Lecture,
            &encode(&appel),
            BUDGETS.lecture,
        );
        let carburant = Instance::carburant(instance).unwrap_or(0);
        match rendu {
            Ok(octets) => {
                self.constat.carburants.push(carburant);
                self.lire(pas, &octets)
            }
            Err(Echec::Faute(f)) => {
                // Une instance qui a fauté se refait : la vue suivante part d'une neuve.
                self.instance = None;
                Issue::Echec(format!("la page de la vue du pas {pas} faute : {f}"))
            }
            Err(Echec::Panne(p)) => {
                self.instance = None;
                Issue::Panne(p.to_string())
            }
        }
    }

    /// Le retour d'un `Page` : une page valide, ou ce qui ne va pas.
    fn lire(&self, pas: u64, octets: &[u8]) -> Issue {
        match decoder::<Retour<'_>>(octets) {
            Ok(Retour::Page(page)) => {
                match valider_page(&page, &self.declaration.assets, &self.declaration.limites) {
                    Ok(()) => Issue::Page(page),
                    Err(d) => {
                        Issue::Echec(format!("la page de la vue du pas {pas} est invalide : {d}"))
                    }
                }
            }
            Ok(Retour::Illisible(e)) => Issue::Echec(format!(
                "la page refuse la vue du pas {pas}, telle qu'elle est partie vers le téléphone : {e}"
            )),
            Ok(_) => Issue::Echec(format!(
                "à la vue du pas {pas}, Page rend autre chose qu'une page"
            )),
            Err(e) => Issue::Echec(format!(
                "à la vue du pas {pas}, le retour de Page ne se relit pas : {e}"
            )),
        }
    }

    /// Chaque geste et chaque tenu de la page passent `Valider`.
    fn valider(&mut self, pas: u64, page: &PageFil) {
        for (sorte, cle, octets) in a_valider(page) {
            self.constat.compte.evaluer();
            let i = match self.instance() {
                Ok(i) => i,
                Err(e) => {
                    self.constat.pannes.push(e);
                    return;
                }
            };
            let appel = Appel::Valider {
                sorte,
                octets: &octets,
            };
            let quoi = match sorte {
                SorteOctets::TenuTelephone => String::from("le tenu de la page"),
                SorteOctets::GesteTelephone | SorteOctets::ActionEcran => {
                    format!("le geste du nœud {:x}", cle.fil())
                }
            };
            match Instance::appeler(i, SorteAppel::Lecture, &encode(&appel), BUDGETS.lecture) {
                Ok(o) => match decoder::<Retour<'_>>(&o) {
                    Ok(Retour::Valide(Validation::Valide { .. })) => {}
                    Ok(Retour::Valide(Validation::Malforme(e))) => self.constat.compte.echouer(
                        format!("à la vue du pas {pas}, {quoi} ne passe pas Valider : {e}"),
                    ),
                    Ok(_) | Err(_) => self.constat.compte.echouer(format!(
                        "à la vue du pas {pas}, Valider de {quoi} ne rend pas de validation"
                    )),
                },
                Err(Echec::Faute(f)) => {
                    self.instance = None;
                    self.constat.compte.echouer(format!(
                        "à la vue du pas {pas}, Valider de {quoi} faute : {f}"
                    ));
                }
                Err(Echec::Panne(p)) => {
                    self.instance = None;
                    self.constat.pannes.push(p.to_string());
                }
            }
        }
    }

    /// Aucune page vide (lot 7) : une seule évaluation, sur toutes les pages valides,
    /// pour que la page par défaut se dise en une ligne et non une fois par vue.
    fn page_vide(&mut self) {
        if self.valides == 0 {
            return;
        }
        self.constat.compte.evaluer();
        if let Some(e) = juger_les_vides(self.valides, self.vides) {
            self.constat.compte.echouer(e);
        }
    }

    fn juger(&mut self, issue: Issue, pas: u64) -> Option<PageFil> {
        self.constat.compte.evaluer();
        match issue {
            Issue::Page(p) => {
                self.valides = self.valides.saturating_add(1);
                if p.noeuds.is_empty() {
                    self.vides.0 = self.vides.0.saturating_add(1);
                    let _: &mut u64 = self.vides.1.get_or_insert(pas);
                }
                self.valider(pas, &p);
                Some(p)
            }
            Issue::Echec(e) => {
                self.constat.compte.echouer(e);
                None
            }
            Issue::Panne(p) => {
                self.constat.pannes.push(p);
                None
            }
        }
    }
}

/// La règle de la page vide (lot 7), sur `valides` pages valides dont `vides.0` sans
/// aucun nœud, la première à la vue du pas `vides.1` : rien si aucune n'est vide ; la
/// page par défaut de `Jeu::page` si toutes le sont ; sinon, les vues oubliées.
fn juger_les_vides(valides: u64, vides: (u64, Option<u64>)) -> Option<String> {
    let (n, premiere) = vides;
    if n == 0 {
        None
    } else if n >= valides {
        Some(format!(
            "la page est celle par défaut de Jeu::page, vide : aucun nœud sur chacune des {n} pages, pour un jeu qui a des téléphones ; le téléphone n'aurait rien à montrer ni à toucher (PHASE-3, § 2.10)"
        ))
    } else {
        Some(format!(
            "{n} page(s) vide(s) sur {valides} valides, la première à la vue du pas {} : le téléphone n'y aurait rien à montrer ni à toucher",
            premiere.unwrap_or(0)
        ))
    }
}

/// E21 sur ces vues, avec des locaux tirés de `graine`.
pub(crate) fn eprouver(module: &ModuleCharge, vues: &VuesVues, graine: &[u8; 32]) -> Constat {
    let declaration = module.declaration();
    if let ApplicableFil::SansObjet(r) = &declaration.telephones {
        return Constat {
            sans_objet: Some(format!("le jeu n'a pas de téléphone : {r}")),
            ..Constat::default()
        };
    }
    let mut juge = Juge {
        module,
        declaration,
        instance: None,
        constat: Constat::default(),
        valides: 0,
        vides: (0, None),
    };
    let mut t = Tirage::new(graine, "E21, des locaux au hasard");
    let retenu = t.octets(RETENU_MAX);
    let mut pages = 0u64;
    let mut noeuds_max = 0usize;
    for (pas, montre) in vues.gardees.values() {
        let issue = juge.page(*pas, montre, LocauxFil::vides());
        let Some(page) = juge.juger(issue, *pas) else {
            continue;
        };
        pages = pages.saturating_add(1);
        noeuds_max = noeuds_max.max(page.noeuds.len());
        for _ in 0..LOCAUX_PAR_VUE {
            let locaux = tirer(&mut t, &page, &retenu);
            let issue = juge.page(*pas, montre, locaux);
            let _: Option<PageFil> = juge.juger(issue, *pas);
        }
    }
    juge.page_vide();
    let gardees = vues.gardees.len();
    juge.constat.notes.push(format!(
        "{} vue(s) de téléphone distincte(s) dans les soirées, {gardees} éprouvée(s) (au plus {VUES_MAX}, les plus petites empreintes), chacune avec des locaux vides puis {LOCAUX_PAR_VUE} tirés au hasard ; {pages} page(s) valide(s) avec des locaux vides, jusqu'à {noeuds_max} nœuds ; {} page(s) vide(s) sur {} valide(s)",
        vues.distinctes(),
        juge.vides.0,
        juge.valides
    ));
    if gardees == 0 {
        juge.constat.pannes.push(String::from(
            "aucune vue de téléphone dans les soirées : les pages n'ont pas été éprouvées",
        ));
    }
    juge.constat
}

#[cfg(test)]
mod tests {
    use super::*;

    /// La page vide (lot 7) : aucune, rien ; toutes, la page par défaut nommée ; une
    /// partie, les vues oubliées, avec la première.
    #[test]
    fn la_page_vide_se_dit_par_defaut_ou_par_vue() {
        assert_eq!(juger_les_vides(12, (0, None)), None);
        let toutes = juger_les_vides(12, (12, Some(3))).unwrap_or_default();
        assert!(
            toutes.starts_with("la page est celle par défaut de Jeu::page"),
            "{toutes}"
        );
        let une = juger_les_vides(12, (1, Some(7))).unwrap_or_default();
        assert!(
            une.starts_with("1 page(s) vide(s) sur 12 valides, la première à la vue du pas 7"),
            "{une}"
        );
    }

    /// L'échantillon garde les plus petites empreintes, chaque vue distincte une fois, et
    /// compte toutes les vues distinctes vues.
    #[test]
    fn les_vues_gardees_sont_les_plus_petites_empreintes() {
        let mut v = VuesVues::default();
        for k in 0..(VUES_MAX as u64 + 40) {
            v.noter(k, &k.to_le_bytes());
            v.noter(k, &k.to_le_bytes());
        }
        assert_eq!(v.distinctes(), VUES_MAX + 40);
        assert_eq!(v.gardees.len(), VUES_MAX);
        let mut toutes: Vec<u64> = (0..(VUES_MAX as u64 + 40))
            .map(|k| fnv1a64(&k.to_le_bytes()))
            .collect();
        toutes.sort_unstable();
        let gardees: Vec<u64> = v.gardees.keys().copied().collect();
        assert_eq!(gardees, toutes[..VUES_MAX]);
        // Deux soirées réunies : le même échantillon que d'une seule traite.
        let mut a = VuesVues::default();
        let mut b = VuesVues::default();
        for k in 0..(VUES_MAX as u64 + 40) {
            if k % 2 == 0 {
                a.noter(k, &k.to_le_bytes());
            } else {
                b.noter(k, &k.to_le_bytes());
            }
        }
        a.ajouter(&b);
        assert_eq!(a.distinctes(), v.distinctes());
        assert_eq!(
            a.gardees.keys().collect::<Vec<_>>(),
            v.gardees.keys().collect::<Vec<_>>()
        );
    }
}
