//! `ardoise`, un jeu d'exemple sans téléphone (`telephones: SansObjet`) : l'écran seul
//! écrit des chiffres sur une ardoise, en retire le dernier, ou l'efface.
//!
//! Il existe pour trois besoins que les jeux ont rapportés au commun
//! (`conception/BESOINS-DES-JEUX.md`) et que `devine`, qui a des téléphones, ne
//! couvre pas :
//!
//! - **besoin 1** : son état garde des valeurs à bourrage, copiées depuis la pile
//!   ([`Trait`]), et une table vidée dont la racine vide a un mot jamais écrit
//!   (`comptes`, comme le `Plateau` de Tier list). Sans la passe `pile` de la
//!   réécriture, une lecture entre deux écritures change l'empreinte de sa mémoire, et
//!   E9 et E14 échouent ;
//! - **besoin 2** : le plan de l'épreuve ne lui programme aucune panne de téléphone ;
//!   elles sont sans objet, et ses soirées sont jugées ;
//! - **besoin 3** : la colle ne lui demande jamais la vue d'un appareil ni un bot de
//!   téléphone ; un tel appel est illisible, sans appeler [`Jeu::vue_telephone`].
//!
//! Ce n'est pas un éditeur complet : c'est le plus petit jeu qui ait ces trois traits.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use jeu::prelude::*;

/// Le plus de traits sur l'ardoise.
pub const TRAITS_MAX: usize = 24;
/// Le texte de chaque chiffre.
const CHIFFRES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
/// Les touches de chaque chiffre.
const TOUCHES: [&str; 10] = [
    "Digit0", "Digit1", "Digit2", "Digit3", "Digit4", "Digit5", "Digit6", "Digit7", "Digit8",
    "Digit9",
];

/// Un chiffre, de 0 à 9.
pub type Chiffre = Borne<0, 9>;

/// La profondeur de pile, en octets, sous laquelle l'ardoise trace, et que sa scène
/// salit (besoin 1).
const PROFONDEUR: usize = 12 * 1024;

/// Appelle `f` sous `PROFONDEUR` octets de pile réservés et jamais écrits : son cadre
/// tombe plus bas que tout ce que l'écriture a touché avant lui, là où seule une
/// lecture a pu passer. C'est le cas, dans un vrai jeu, d'une valeur bâtie loin dans
/// l'étape (besoin 1).
#[inline(never)]
fn sous_la_pile<R>(f: impl FnOnce() -> R) -> R {
    let reserve = [const { core::mem::MaybeUninit::<u8>::uninit() }; PROFONDEUR];
    let _: &[core::mem::MaybeUninit<u8>; PROFONDEUR] = core::hint::black_box(&reserve);
    f()
}

/// Écrit un motif sur la pile, plus bas que `PROFONDEUR` : ce que laisse une lecture
/// qui descend loin, comme la scène d'un vrai jeu (besoin 1).
#[inline(never)]
fn salir_la_pile() {
    let motif = [0xa5_u8; PROFONDEUR + 4096];
    let _: &[u8; PROFONDEUR + 4096] = core::hint::black_box(&motif);
}

/// Un trait de l'ardoise. Quand `couleur` vaut `None`, seul son discriminant est
/// écrit : les huit octets de sa valeur, et le bourrage après le discriminant, sont
/// copiés depuis la pile avec le reste (besoin 1).
#[jeu::etat]
pub struct Trait {
    longueur: u64,
    couleur: Option<u64>,
}

/// L'état de l'ardoise.
#[jeu::etat]
pub struct Ardoise {
    traits: Vec<Trait>,
    /// Combien de fois chaque chiffre est écrit. « Effacer » la remplace par une table
    /// neuve, dont la racine vide laisse un mot jamais écrit (besoin 1).
    comptes: BTreeMap<u8, u32>,
    /// Les comptes d'avant chacune des dernières écritures, comme l'historique de Tier
    /// list : la copie d'une table vide y entre avec le mot jamais écrit de sa racine
    /// (besoin 1).
    avant: Vec<BTreeMap<u8, u32>>,
    effacements: u32,
}

/// Où en est la partie : une seule phase, qui ne finit pas.
#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum Etape {
    /// On écrit.
    Edition,
}

impl Phase for Etape {
    fn terminale(self) -> Oui {
        Oui::Non
    }
}

/// Les écrans, pour la galerie des pires cas.
#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum Ecran {
    /// L'ardoise est vierge.
    Vierge,
    /// L'ardoise porte au moins un trait.
    Ecrite,
}

/// Ce qu'envoie l'écran.
#[jeu::message]
#[derive(Copy, Eq)]
pub enum ActionEcran {
    /// Écrire ce chiffre.
    Ecrire(Chiffre),
    /// Retirer le dernier trait.
    Annuler,
    /// Tout effacer.
    Effacer,
}

/// Ce que dit un refus, à l'écran.
#[jeu::message]
pub enum Avis {
    /// L'ardoise est pleine.
    Pleine,
    /// Il n'y a rien à retirer.
    Vierge,
}

const fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

/// Le style des libellés.
const STYLE: StyleTexte = StyleTexte {
    police: Cle::POLICE_DU_COMMUN,
    taille: Taille::Budget {
        max: Fixe::unites(1),
        min: Fixe::unites(1),
    },
    couleur: Rgba::BLANC,
    alignement: Alignement::Centre,
    lignes: Lignes::Une,
    coupe: Coupe::Graphemes,
};

impl Ardoise {
    /// Une ardoise vierge.
    fn neuve(effacements: u32) -> Ardoise {
        Ardoise {
            traits: Vec::new(),
            comptes: BTreeMap::new(),
            avant: Vec::new(),
            effacements,
        }
    }

    /// Écrit un chiffre. Jamais en ligne : son cadre de pile est sous celui de la colle,
    /// là où une lecture a laissé les siens (besoin 1).
    #[inline(never)]
    fn tracer(&mut self, c: Chiffre) {
        let v = u8::try_from(c.get()).unwrap_or(0);
        // La valeur passe par la pile, avec son bourrage, avant d'entrer dans le tas :
        // ce que les trois jeux ont constaté sur leurs propres valeurs (besoin 1).
        let t = core::hint::black_box(Trait {
            longueur: u64::from(v).saturating_add(1),
            couleur: v.is_multiple_of(2).then_some(u64::from(v)),
        });
        self.traits.push(t);
        if self.avant.len() >= 8 {
            let _: BTreeMap<u8, u32> = self.avant.remove(0);
        }
        self.avant.push(self.comptes.clone());
        let n = self.comptes.entry(v).or_insert(0);
        *n = n.saturating_add(1);
    }

    /// Retire le dernier trait.
    fn retirer_le_dernier(&mut self) {
        let Some(t) = self.traits.pop() else {
            return;
        };
        let v = u8::try_from(t.longueur.saturating_sub(1)).unwrap_or(0);
        if let Some(n) = self.comptes.get_mut(&v) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                let _: Option<u32> = self.comptes.remove(&v);
            }
        }
    }

    /// Les chiffres écrits, dans l'ordre.
    fn chiffres(&self) -> Vec<u8> {
        self.traits
            .iter()
            .map(|t| u8::try_from(t.longueur.saturating_sub(1)).unwrap_or(u8::MAX))
            .collect()
    }
}

fn action(avant: Avant<'_, Ardoise>, a: ActionEcran) -> Issue<Ardoise> {
    let j = avant.jeu();
    match a {
        ActionEcran::Ecrire(c) => {
            if j.traits.len() >= TRAITS_MAX {
                return avant.refuser(Avis::Pleine);
            }
            let mut e = avant.accepter();
            let j = e.jeu();
            sous_la_pile(|| j.tracer(c));
            e.fin()
        }
        ActionEcran::Annuler => {
            if j.traits.is_empty() {
                return avant.refuser(Avis::Vierge);
            }
            let mut e = avant.accepter();
            e.jeu().retirer_le_dernier();
            e.fin()
        }
        ActionEcran::Effacer => {
            if j.traits.is_empty() {
                return avant.ignorer();
            }
            let mut e = avant.accepter();
            let j = e.jeu();
            let effacements = j.effacements.saturating_add(1);
            *j = Ardoise::neuve(effacements);
            e.fin()
        }
    }
}

impl Jeu for Ardoise {
    type ActionEcran = ActionEcran;
    type GesteTelephone = Aucun;
    type TenuTelephone = Aucun;
    type VueTelephone = Aucun;
    type Avis = Avis;
    type Phase = Etape;
    type Garde = ();
    type Precedent = Aucun;

    const DECLARATION: Declaration = Declaration {
        nom: "ardoise",
        places: Places::joueurs(1, 1).appareils(1),
        rattrapage: 1,
        fenetre: Applicable::SansObjet(jeu::raison!("l'écran seul fait foi, sans réseau")),
        antidatage_max: Duree::ms(30),
        pause: Pause::Aucune(jeu::raison!("rien ne bouge sans un geste")),
        ecran: FormeEcran::Scene {
            largeur: 16,
            hauteur: 9,
            zone_sure_pour_cent: 90,
            echelle_entiere: Oui::Non,
        },
        telephones: Applicable::SansObjet(jeu::raison!("l'écran seul écrit sur l'ardoise")),
        manettes: Applicable::SansObjet(jeu::raison!("aucune manette n'a de rôle ici")),
        prediction: Applicable::SansObjet(jeu::raison!("un seul écran, qui fait foi")),
        hasard: Applicable::SansObjet(jeu::raison!("une ardoise ne tire rien au sort")),
        secrets: Applicable::SansObjet(jeu::raison!("tout se voit sur un seul écran")),
        fin: Applicable::SansObjet(jeu::raison!("une ardoise n'a pas de fin de partie")),
        capacites: Voulues {
            vibration: Oui::Non,
            mouvement: Oui::Non,
            son: Oui::Non,
            ecran_allume: Oui::Non,
            micro: Oui::Non,
            pad_natif: Oui::Non,
        },
        limites: Limites {
            geste: 256,
            file: 32,
            vue: 16_384,
            effets: 8,
            memoire: 64,
            brouillon: 1 << 20,
        },
        diagnostic: CodeTouche::new("KeyD"),
        assets: &[],
        mecaniques: &[Mecanique::Plateau],
    };

    fn creer(_: Creation<'_, Self>) -> Self {
        Ardoise::neuve(0)
    }

    fn appliquer(avant: Avant<'_, Self>, e: Entree<'_, Self>) -> Issue<Self> {
        match e {
            Entree::Ecran(DeEcran::Action { action: a, .. }) => action(avant, a),
            Entree::Geste(g) => match g.geste {},
            Entree::Tenu(t) => match t.valeur {},
            Entree::Ecran(
                DeEcran::Touche { .. } | DeEcran::Pointeur { .. } | DeEcran::Composant { .. },
            )
            | Entree::Temps(_)
            | Entree::Manette(_)
            | Entree::Presence(_)
            | Entree::Reponse(..) => avant.ignorer(),
        }
    }

    fn vue_ecran(&self, _: &Regard, s: &mut Scene<ActionEcran>) {
        salir_la_pile();
        s.fond(Rgba(20, 40, 30, 255));
        for (i, t) in (0i32..).zip(&self.traits) {
            let couleur = match t.couleur {
                Some(_) => Rgba::BLANC,
                None => Rgba(200, 200, 120, 255),
            };
            let col = i.checked_rem(8).unwrap_or(0);
            let ligne = i.checked_div(8).unwrap_or(0);
            let _: &mut Noeud = s.rect(
                Cle::de(&("trait", i)),
                rect(col.saturating_add(4), ligne.saturating_add(1), 1, 1),
                couleur,
            );
        }
        for (i, nom) in (0i64..).zip(CHIFFRES) {
            let Some(c) = Chiffre::new(i) else {
                continue;
            };
            let x = i32::try_from(i).unwrap_or(0).saturating_add(3);
            let _: &mut Noeud = s.bouton(
                Cle::de(&("chiffre", i)),
                rect(x, 7, 1, 1),
                nom,
                ActionEcran::Ecrire(c),
            );
            if let Some(touche) = TOUCHES.get(c.indice()) {
                s.raccourci(
                    CodeTouche::new(touche),
                    Mods::AUCUN,
                    Oui::Non,
                    ActionEcran::Ecrire(c),
                );
            }
        }
        if !self.traits.is_empty() {
            let _: &mut Noeud = s.bouton(
                Cle::de(&"annuler"),
                rect(13, 7, 2, 1),
                "Annuler",
                ActionEcran::Annuler,
            );
            let _: &mut Noeud = s.bouton(
                Cle::de(&"effacer"),
                rect(1, 7, 2, 1),
                "Effacer",
                ActionEcran::Effacer,
            );
        }
        let _: &mut Noeud = s.texte(
            Cle::de(&"effacements"),
            CHIFFRES
                .get(usize::try_from(self.effacements.min(9)).unwrap_or(0))
                .copied()
                .unwrap_or("9"),
            rect(14, 0, 1, 1),
            STYLE,
        );
    }

    #[expect(
        clippy::panic,
        reason = "sans téléphone, la colle ne demande jamais cette vue (besoin 3), et `Aucun` n'a pas de valeur"
    )]
    fn vue_telephone(&self, _: &Regard) -> Montre<Aucun> {
        panic!("ardoise n'a pas de téléphone")
    }

    fn ressource(&self, _: Cle) -> Option<&[u8]> {
        None
    }

    fn phase(&self) -> Etape {
        Etape::Edition
    }

    fn reveil(&self) -> Reveil {
        Reveil::Jamais
    }

    fn attente(&self, a: &mut Attente<'_, Self>) {
        a.libre(jeu::raison!(
            "une ardoise n'avance que sous la main de l'écran"
        ));
    }

    fn accueil(&self) -> Accueil {
        Accueil::Fermee
    }

    fn demandes<'s>(&'s self, _: &mut Demandes<'s>) {}

    fn previsible(g: &Aucun) -> Prevision {
        match *g {}
    }

    fn garder(&self) {}

    fn convertir(a: Aucun) -> Result<Self, Raison> {
        match a {}
    }
}

impl Epreuve for Ardoise {
    /// Les chiffres écrits, et le nombre d'effacements.
    type Resume = (Vec<u8>, u32);
    type Cas = Ecran;

    fn resume(&self, _: &Autorite) -> (Vec<u8>, u32) {
        (self.chiffres(), self.effacements)
    }

    fn invariants(&self, _: &Autorite, v: &mut Verif) {
        jeu::exiger!(
            v,
            self.traits.len() <= TRAITS_MAX,
            "vingt-quatre traits au plus"
        );
        let total = self
            .comptes
            .values()
            .fold(0u64, |s, n| s.saturating_add(u64::from(*n)));
        jeu::exiger!(
            v,
            usize::try_from(total).is_ok_and(|t| t == self.traits.len()),
            "les comptes disent les traits"
        );
        jeu::exiger!(
            v,
            self.comptes.values().all(|n| *n > 0),
            "un chiffre sans trait n'a pas de compte"
        );
    }

    fn transition(avant: &(Vec<u8>, u32), apres: &(Vec<u8>, u32), v: &mut Verif) {
        let (a, b) = (avant.0.len(), apres.0.len());
        let un_de_plus_ou_de_moins = a.abs_diff(b) == 1;
        let effacee = b == 0 && apres.1 == avant.1.saturating_add(1);
        jeu::exiger!(
            v,
            un_de_plus_ou_de_moins || effacee,
            "un trait à la fois, ou tout effacé"
        );
    }

    fn bot_telephone(_: &Self, m: &Montre<Aucun>, _: &mut Bot<'_, Self>) {
        match m.vue {}
    }

    fn bot_ecran(j: &Self, b: &mut Bot<'_, Self>) {
        if b.depuis_vue() < Duree::ms(300) {
            b.rien();
            return;
        }
        // Surtout écrire, parfois retirer, rarement effacer : l'ardoise se remplit, et,
        // pleine, un chiffre de plus est refusé (E6 en voit le refus).
        let tirage = b.hasard().dans::<0, 99>().get();
        let c = Chiffre::new(tirage.checked_rem(10).unwrap_or(0)).unwrap_or(Chiffre::PLUS_PETIT);
        let a = match tirage {
            70..=84 if !j.traits.is_empty() => ActionEcran::Annuler,
            85 if j.traits.len() > 4 => ActionEcran::Effacer,
            0..=49 if j.traits.len() >= TRAITS_MAX => ActionEcran::Effacer,
            _ => ActionEcran::Ecrire(c),
        };
        b.action(a);
    }

    fn cas(&self) -> Ecran {
        if self.traits.is_empty() {
            Ecran::Vierge
        } else {
            Ecran::Ecrite
        }
    }

    /// L'ardoise vierge, et l'ardoise pleine de neuf.
    fn pire_cas(c: Ecran, _: &Construction<'_>) -> Self {
        let mut a = Ardoise::neuve(u32::MAX);
        if c == Ecran::Ecrite {
            for _ in 0..TRAITS_MAX {
                a.tracer(Chiffre::PLUS_GRAND);
            }
        }
        a
    }

    fn proprietes() -> Proprietes<Self> {
        Proprietes {
            coupure_transparente: Applicable::SansObjet(jeu::raison!(
                "aucun appareil ne joue, l'écran seul"
            )),
            permutation: Applicable::SansObjet(jeu::raison!(
                "aucun appareil ne joue, l'écran seul"
            )),
        }
    }
}
