//! Le socle des fautifs : un petit jeu complet et sain, paramétré par son défaut.
//!
//! `Socle<Sain>` passe tous les contrôles écrits de l'épreuve ; chaque autre défaut
//! ([`crate::defauts`]) change une seule chose, pour faire échouer un seul contrôle.
//! Un jeu fautif est ainsi le même jeu que le sain, à une règle près : le contrôle
//! négatif ne mesure que cette règle (OUT-27, HIS-03, SAL-168).
//!
//! # Le jeu
//!
//! Des téléphones rejoignent le salon. L'écran lance une manche : un mot est tiré au
//! hasard, gardé secret ; pendant cinq secondes, chaque joueur tape, et chaque tape
//! compte un point. Une prolongation de deux secondes suit, puis la fin, où le mot
//! se montre. L'écran propose alors « Rejouer » : une nouvelle partie, qui garde les
//! joueurs présents et oublie les partis (PUN-15).
//!
//! Il touche ce que les contrôles lisent : phases (dont une terminale), cas, secret,
//! hasard, refus, échéances, présences, attente, invariants, transition, bots.

use alloc::vec::Vec;
use core::num::NonZeroU32;

use jeu::prelude::*;

use crate::defauts::Defaut;

jeu::limite!(pub Prenom = 12 Capitales);
jeu::limite!(pub Mot = 16 Capitales);

/// Les mots que la manche tire.
pub const MOTS: [&str; 4] = ["ANANAS", "BANANIER", "CAROTTIER", "DATTIER"];
/// La valeur de chaque chiffre d'un `Choisir`.
pub const VALEURS: [u32; 10] = [0, 1, 1, 2, 3, 5, 8, 13, 21, 34];
/// Le plus de joueurs.
pub const JOUEURS_MAX: usize = 6;
/// La durée d'une manche.
const MANCHE: Duree = Duree::s(5);
/// La durée de la prolongation.
pub const PROLONGATION: Duree = Duree::s(2);

/// La déclaration du socle. Un défaut peut en partir pour la changer.
pub const DECLARATION: Declaration = Declaration {
    nom: "socle",
    places: Places::joueurs(1, 6).appareils(8),
    rattrapage: 4,
    fenetre: Applicable::SansObjet(jeu::raison!("tour par tour, le pas vu suffit")),
    antidatage_max: Duree::ms(30),
    pause: Pause::Aucune(jeu::raison!("chaque phase a une échéance ou une action")),
    ecran: FormeEcran::Scene {
        largeur: 16,
        hauteur: 9,
        zone_sure_pour_cent: 90,
        echelle_entiere: Oui::Non,
    },
    telephones: Applicable::Oui(Telephones {
        repartition: Repartition::UnParJoueur,
        tenu: ReglesTenu {
            fraicheur: Applicable::SansObjet(jeu::raison!("aucun état tenu dans ce jeu")),
            age_max: Applicable::SansObjet(jeu::raison!("aucun état tenu dans ce jeu")),
            debit_max: 10,
        },
    }),
    manettes: Applicable::SansObjet(jeu::raison!("aucune manette n'a de rôle ici")),
    prediction: Applicable::SansObjet(jeu::raison!("un geste toutes les secondes suffit")),
    hasard: Applicable::Oui(()),
    secrets: Applicable::Oui(()),
    fin: Applicable::Oui(()),
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
        vue: 4096,
        effets: 8,
        memoire: 64,
        brouillon: 1 << 20,
    },
    diagnostic: CodeTouche::new("KeyD"),
    assets: &[],
    mecaniques: &[Mecanique::TourParTour, Mecanique::Hasard, Mecanique::Texte],
};

/// L'état du socle.
#[jeu::etat]
pub struct Socle<D: Defaut> {
    /// Le défaut : un marqueur sans donnée.
    defaut: D,
    phase: Etape,
    phase_depuis: Pas,
    fin: Option<Echeance>,
    /// Le mot de la manche : `Qui::Personne`, puis `Qui::Tous` à la fin.
    mot: Secret<Texte<Mot>>,
    /// Ce que l'écran montre du mot avant la fin : rien, dans le jeu sain.
    indice: Option<Texte<Mot>>,
    joueurs: Appareils<Joueur>,
    /// Le premier joueur arrivé.
    meneur: Option<Appareil>,
    /// Le numéro de la manche en cours.
    manche: u32,
    /// Les parties finies avant celle-ci, gardées d'une partie à l'autre.
    parties: u32,
    /// Les appareils dont le jeu se souvient hors de ses joueurs : vide, dans le jeu
    /// sain.
    vus: Vec<(Appareil, u64)>,
}

/// Un joueur.
#[jeu::etat]
pub struct Joueur {
    prenom: Texte<Prenom>,
    /// Les points : un par tape, dans le jeu sain.
    points: u32,
    /// Les tapes acceptées.
    tapes: u32,
    /// Le dernier chiffre choisi, par sa valeur.
    choix: u32,
    /// Les points par tape : un flottant de l'état, que E17 lit dans la forme
    /// canonique ; 0 tant qu'il n'a pas tapé, dans le jeu sain.
    moyenne: f32,
    present: Oui,
    /// S'est-il dit prêt, au salon ?
    pret: Oui,
}

/// Ce qu'une nouvelle partie garde.
#[jeu::etat]
pub struct Garde {
    joueurs: Appareils<Joueur>,
    parties: u32,
    /// Les appareils retenus hors des joueurs : vide, dans le jeu sain.
    vus: Vec<(Appareil, u64)>,
}

/// Où en est la partie.
#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum Etape {
    /// On attend que l'écran lance une manche.
    Salon,
    /// Les téléphones tapent.
    Manche,
    /// Encore un peu.
    Prolongation,
    /// Le mot se montre ; l'écran peut rejouer.
    Fin,
}

impl Phase for Etape {
    fn terminale(self) -> Oui {
        match self {
            Etape::Fin => Oui::Oui,
            Etape::Salon | Etape::Manche | Etape::Prolongation => Oui::Non,
        }
    }
}

/// Ce qu'envoie un téléphone.
#[jeu::message]
pub enum GesteTel {
    /// Entrer dans la partie, ou changer de prénom.
    Rejoindre(Texte<Prenom>),
    /// Une tape, pendant la manche.
    Taper,
    /// Choisir un chiffre, de 0 à 9 pour une page honnête.
    Choisir(u8),
    /// Se dire prêt, au salon.
    Pret,
}

/// Ce qu'envoie l'écran.
#[jeu::message]
#[derive(Copy, Eq)]
pub enum ActionEcran {
    /// Lancer une manche.
    Lancer,
    /// Une nouvelle partie, à la fin.
    Rejouer,
}

/// Ce que dit un refus.
#[jeu::message]
pub enum Avis {
    /// Pas pendant cette phase, ou sans être joueur.
    PasMaintenant,
    /// La partie est pleine.
    Pleine,
}

/// Ce que voit un téléphone.
#[jeu::message]
pub enum VueTel {
    /// Choisir un prénom.
    Prenom,
    /// Attendre au salon.
    Attends,
    /// Taper.
    Tape {
        /// Ses points.
        points: u32,
    },
    /// La fin.
    Resultat {
        /// Ses points.
        points: u32,
    },
}

const fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

const STYLE: StyleTexte = StyleTexte {
    police: Cle::POLICE_DU_COMMUN,
    taille: Taille::Budget {
        max: Fixe::unites(3),
        min: Fixe::unites(1),
    },
    couleur: Rgba::BLANC,
    alignement: Alignement::Centre,
    lignes: Lignes::Une,
    coupe: Coupe::Graphemes,
};

/// Le texte d'un petit nombre, sans allouer (une vue alloue dans le brouillon, mais
/// un tableau suffit ici).
fn nombre(n: u32) -> &'static str {
    const N: [&str; 11] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10+"];
    let i = usize::try_from(n).unwrap_or(usize::MAX).min(10);
    N.get(i).copied().unwrap_or("?")
}

impl<D: Defaut> Socle<D> {
    /// Une manche commence : un mot neuf, caché ; l'échéance née de maintenant.
    fn ouvrir_manche(&mut self, m: &Maintenant<'_>, mot: Texte<Mot>) {
        self.phase = Etape::Manche;
        self.phase_depuis = m.pas();
        self.fin = Some(Echeance::dans(m, MANCHE));
        self.manche = D::numero_de_manche(self.manche);
        self.indice = if D::indice_trop_tot() == Oui::Oui {
            Some(mot.clone())
        } else {
            None
        };
        self.mot = Secret::cacher(Qui::Personne, mot);
        for (_, j) in self.joueurs.iter_mut() {
            j.pret = Oui::Non;
        }
    }

    /// L'échéance est passée.
    fn avancer(&mut self, m: &Maintenant<'_>) {
        self.phase_depuis = m.pas();
        match self.phase {
            Etape::Manche if D::prolonger() == Oui::Oui => {
                self.phase = Etape::Prolongation;
                self.fin = Some(D::echeance_suivante(m, self.fin, D::prolongation()));
            }
            Etape::Manche | Etape::Prolongation => {
                self.fin = None;
                self.indice = None;
                if D::fin_atteinte(self.parties) == Oui::Oui {
                    self.phase = Etape::Fin;
                    self.parties = self.parties.saturating_add(D::decompte_final());
                    self.mot.montrer(Qui::Tous);
                    if D::departager_par_l_appareil() == Oui::Oui {
                        self.bonus_au_plus_petit();
                    }
                } else {
                    self.phase = Etape::Salon;
                }
            }
            Etape::Salon | Etape::Fin => {
                self.fin = None;
            }
        }
    }

    fn est_joueur(&self, a: Appareil) -> bool {
        self.joueurs.get(a).is_some()
    }

    /// Le défaut de E10 : un point de plus, à la fin, au joueur dont l'appareil
    /// s'écrit en premier dans l'ordre des octets. La valeur d'un `Appareil` est tirée
    /// au hasard pour chaque soirée : elle ne départage rien (CAT-27).
    fn bonus_au_plus_petit(&mut self) {
        let premier = self
            .joueurs
            .iter()
            .map(|(a, _)| (jeu::octets(&a), a))
            .min_by(|x, y| x.0.cmp(&y.0))
            .map(|(_, a)| a);
        if let Some(a) = premier
            && let Some(j) = self.joueurs.get_mut(a)
        {
            j.points = j.points.saturating_add(1);
            j.tapes = j.tapes.saturating_add(1);
        }
    }

    /// L'hypothèse de la coupure transparente (PUN-73) : au salon, un joueur qui n'a
    /// plus rien à dire. Une coupure de moins de deux secondes, tout entière au salon
    /// (l'épreuve lit l'hypothèse pendant toute la coupure), ne lui fait rien perdre :
    /// ce qu'il dit au salon, retenu, arrive à son retour, encore au salon, et les
    /// points n'en dépendent pas.
    fn coupure_sans_effet(&self, a: Appareil) -> Oui {
        let tient = self.phase == Etape::Salon
            && self
                .joueurs
                .get(a)
                .is_some_and(|j| D::salon_attend_chacun() == Oui::Non || j.pret == Oui::Oui);
        if tient { Oui::Oui } else { Oui::Non }
    }
}

fn rejoindre<D: Defaut>(
    avant: Avant<'_, Socle<D>>,
    de: Appareil,
    prenom: Texte<Prenom>,
) -> Issue<Socle<D>> {
    if prenom.vide() == Oui::Oui {
        D::avant_refus();
        return avant.refuser(Avis::PasMaintenant);
    }
    let deja = avant.jeu().est_joueur(de);
    if !deja && avant.jeu().joueurs.len() >= JOUEURS_MAX {
        D::avant_refus();
        return avant.refuser(Avis::Pleine);
    }
    let mut e = avant.accepter();
    let j = e.jeu();
    if let Some(joueur) = j.joueurs.get_mut(de) {
        joueur.prenom = prenom;
        joueur.present = Oui::Oui;
    } else {
        let _: Option<Joueur> = j.joueurs.inserer(
            de,
            Joueur {
                prenom,
                points: 0,
                tapes: 0,
                choix: 0,
                moyenne: D::moyenne(0, 0),
                present: Oui::Oui,
                pret: Oui::Non,
            },
        );
    }
    if j.meneur.is_none() {
        j.meneur = Some(de);
    }
    e.fin()
}

fn taper<D: Defaut>(avant: Avant<'_, Socle<D>>, de: Appareil, quand: Quand) -> Issue<Socle<D>> {
    let j = avant.jeu();
    let en_jeu = matches!(j.phase, Etape::Manche | Etape::Prolongation);
    if !en_jeu || !j.est_joueur(de) {
        D::avant_refus();
        return avant.refuser(Avis::PasMaintenant);
    }
    let points = if D::compte(&quand) == Oui::Oui {
        D::points_par_tape()
    } else {
        0
    };
    let mut e = avant.accepter();
    if let Some(joueur) = e.jeu().joueurs.get_mut(de) {
        joueur.points = joueur.points.saturating_add(points);
        joueur.tapes = joueur.tapes.saturating_add(D::tapes_par_tape());
        joueur.moyenne = D::moyenne(joueur.points, joueur.tapes);
    }
    e.fin()
}

fn choisir<D: Defaut>(avant: Avant<'_, Socle<D>>, de: Appareil, n: u8) -> Issue<Socle<D>> {
    let j = avant.jeu();
    if j.phase != Etape::Manche || !j.est_joueur(de) {
        D::avant_refus();
        return avant.refuser(Avis::PasMaintenant);
    }
    let mut e = avant.accepter();
    let valeur = D::valeur(n).saturating_add(D::compter_le_vote());
    if let Some(joueur) = e.jeu().joueurs.get_mut(de) {
        joueur.choix = valeur;
    }
    e.fin()
}

fn lancer<D: Defaut>(avant: Avant<'_, Socle<D>>) -> Issue<Socle<D>> {
    let j = avant.jeu();
    if j.phase != Etape::Salon {
        return avant.ignorer();
    }
    if D::salon_attend_chacun() == Oui::Oui && !j.joueurs.is_empty() {
        return avant.ignorer();
    }
    commencer(avant)
}

/// Le mot n'est tiré qu'après `accepter`.
fn commencer<D: Defaut>(avant: Avant<'_, Socle<D>>) -> Issue<Socle<D>> {
    let mut e = avant.accepter();
    let p = e.parts();
    let mot = match p.sorties.hasard() {
        Ok(h) => h.choisir(D::mots()).copied().unwrap_or("ANANAS"),
        Err(a) => return e.attendre(a),
    };
    p.jeu.ouvrir_manche(&p.maintenant, Texte::nettoyer(mot));
    e.fin()
}

fn pret<D: Defaut>(avant: Avant<'_, Socle<D>>, de: Appareil) -> Issue<Socle<D>> {
    let j = avant.jeu();
    let attend = D::salon_attend_chacun() == Oui::Oui;
    let deja = j.joueurs.get(de).map(|x| x.pret);
    if !attend || j.phase != Etape::Salon || deja != Some(Oui::Non) {
        D::avant_refus();
        return avant.refuser(Avis::PasMaintenant);
    }
    let mut e = avant.accepter();
    let p = e.parts();
    if let Some(x) = p.jeu.joueurs.get_mut(de) {
        x.pret = Oui::Oui;
    }
    if p.jeu.joueurs.iter().all(|(_, x)| x.pret == Oui::Oui) {
        let mot = match p.sorties.hasard() {
            Ok(h) => h.choisir(D::mots()).copied().unwrap_or("ANANAS"),
            Err(a) => return e.attendre(a),
        };
        p.jeu.ouvrir_manche(&p.maintenant, Texte::nettoyer(mot));
    }
    e.fin()
}

/// « Rejouer » : les joueurs partis sont oubliés, puis une nouvelle partie commence.
fn rejouer<D: Defaut>(avant: Avant<'_, Socle<D>>) -> Issue<Socle<D>> {
    if avant.jeu().phase != Etape::Fin {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    let p = e.parts();
    let partis: Vec<Appareil> = p
        .jeu
        .joueurs
        .iter()
        .filter(|(_, j)| j.present == Oui::Non && D::oublier_les_partis() == Oui::Oui)
        .map(|(a, _)| a)
        .collect();
    for a in partis {
        let _: Option<Joueur> = p.jeu.joueurs.retirer(a);
        p.sorties.oublier(a);
    }
    p.jeu.parties = p.jeu.parties.saturating_add(1);
    p.sorties.nouvelle_partie();
    e.fin()
}

fn temps<D: Defaut>(avant: Avant<'_, Socle<D>>) -> Issue<Socle<D>> {
    let Some(fin) = avant.jeu().fin else {
        return avant.ignorer();
    };
    if fin.atteinte(&avant.maintenant()) == Oui::Non {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    let p = e.parts();
    p.jeu.avancer(&p.maintenant);
    e.fin()
}

fn presence<D: Defaut>(avant: Avant<'_, Socle<D>>, p: Presence) -> Issue<Socle<D>> {
    match p {
        Presence::Arrivee { appareil, .. } => {
            if D::retenir() == Oui::Non {
                return avant.ignorer();
            }
            let mut e = avant.accepter();
            let j = e.jeu();
            let n = u64::try_from(j.vus.len()).unwrap_or(u64::MAX);
            j.vus.push((appareil, n));
            e.fin()
        }
        Presence::Depart { appareil, .. } if D::retirer_au_depart() == Oui::Oui => {
            retirer_le_parti(avant, appareil)
        }
        Presence::Depart { appareil, .. } => marquer(avant, appareil, Oui::Non),
        Presence::Retour { appareil, .. } => marquer(avant, appareil, Oui::Oui),
        Presence::Capacites { .. } => avant.ignorer(),
    }
}

/// Le défaut de E11 : un joueur qui part, même une seconde, est retiré de la partie
/// avec ses points (PUN-73).
fn retirer_le_parti<D: Defaut>(avant: Avant<'_, Socle<D>>, a: Appareil) -> Issue<Socle<D>> {
    if !avant.jeu().est_joueur(a) {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    let j = e.jeu();
    let _: Option<Joueur> = j.joueurs.retirer(a);
    if j.meneur == Some(a) {
        j.meneur = j.joueurs.iter().next().map(|(b, _)| b);
    }
    e.fin()
}

fn marquer<D: Defaut>(avant: Avant<'_, Socle<D>>, a: Appareil, present: Oui) -> Issue<Socle<D>> {
    if !avant.jeu().est_joueur(a) {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    if let Some(j) = e.jeu().joueurs.get_mut(a) {
        j.present = present;
    }
    e.fin()
}

impl<D: Defaut> Jeu for Socle<D> {
    type ActionEcran = ActionEcran;
    type GesteTelephone = GesteTel;
    type TenuTelephone = Aucun;
    type VueTelephone = VueTel;
    type Avis = Avis;
    type Phase = Etape;
    type Garde = Garde;
    type Precedent = Aucun;

    const DECLARATION: Declaration = D::DECLARATION;

    fn creer(mut c: Creation<'_, Self>) -> Self {
        D::a_la_creation();
        let pas = c.maintenant().pas();
        let mot = match c.hasard() {
            Ok(h) => h.choisir(D::mots()).copied().unwrap_or("ANANAS"),
            Err(Absent) => "ANANAS",
        };
        let garde = c.garde();
        let (joueurs, parties, vus) = match garde {
            Some(g) => (g.joueurs, g.parties, g.vus),
            None => (Appareils::new(), 0, Vec::new()),
        };
        let meneur = joueurs.iter().next().map(|(a, _)| a);
        Socle {
            defaut: D::neuf(),
            phase: Etape::Salon,
            phase_depuis: pas,
            fin: None,
            mot: Secret::cacher(Qui::Personne, Texte::nettoyer(mot)),
            indice: None,
            joueurs,
            meneur,
            manche: 0,
            parties,
            vus,
        }
    }

    fn appliquer(avant: Avant<'_, Self>, e: Entree<'_, Self>) -> Issue<Self> {
        D::a_chaque_entree();
        match e {
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Rejoindre(prenom),
                ..
            }) => rejoindre(avant, de, prenom),
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Taper,
                quand,
            }) => taper(avant, de, quand),
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Choisir(n),
                ..
            }) => choisir(avant, de, n),
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Pret,
                ..
            }) => pret(avant, de),
            Entree::Ecran(DeEcran::Action {
                action: ActionEcran::Lancer,
                ..
            }) => lancer(avant),
            Entree::Ecran(DeEcran::Action {
                action: ActionEcran::Rejouer,
                ..
            }) => rejouer(avant),
            Entree::Temps(_) => temps(avant),
            Entree::Presence(p) => presence(avant, p),
            Entree::Ecran(
                DeEcran::Touche { .. } | DeEcran::Pointeur { .. } | DeEcran::Composant { .. },
            )
            | Entree::Tenu(_)
            | Entree::Manette(_)
            | Entree::Reponse(..) => avant.ignorer(),
        }
    }

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<ActionEcran>) {
        D::en_vue_ecran();
        let phase = match self.phase {
            Etape::Salon => "Salon",
            Etape::Manche => "Tapez !",
            Etape::Prolongation => "Encore !",
            Etape::Fin => "Fin",
        };
        let _: &mut Noeud = s.texte(Cle::de(&"phase"), phase, rect(4, 0, 8, 1), STYLE);
        if let Some(m) = self.mot.voir(r) {
            let _: &mut Noeud = s.texte(Cle::de(&"mot"), m.as_str(), rect(2, 2, 12, 2), STYLE);
        }
        if let Some(i) = &self.indice {
            let _: &mut Noeud = s.texte(Cle::de(&"indice"), i.as_str(), rect(2, 4, 12, 1), STYLE);
        }
        if D::deborder() == Oui::Oui && self.joueurs.iter().any(|(_, j)| j.points >= 9_999) {
            let _: &mut Noeud = s.texte(Cle::de(&"record"), "Record !", rect(40, 0, 4, 1), STYLE);
        }
        if let Some(n) = D::compteur_affiche() {
            let _: &mut Noeud = s.texte(Cle::de(&"compteur"), nombre(n), rect(0, 8, 2, 1), STYLE);
        }
        for (k, (a, j)) in self.joueurs.iter().enumerate() {
            let y = i32::try_from(k).unwrap_or(0).saturating_add(5).min(8);
            let _: &mut Noeud = s.texte(
                Cle::de(&("joueur", a)),
                j.prenom.as_str(),
                rect(2, y, 6, 1),
                STYLE,
            );
            let _: &mut Noeud = s.texte(
                Cle::de(&("points", a)),
                nombre(j.points),
                rect(9, y, 2, 1),
                STYLE,
            );
        }
        if let Some(fin) = self.fin {
            let _: &mut Noeud = s.compte_a_rebours(
                Cle::de(&"temps"),
                CompteARebours::Court { echeance: fin },
                rect(12, 0, 3, 1),
                STYLE,
            );
        }
        match self.phase {
            Etape::Salon => {
                let _: &mut Noeud = s.bouton(
                    Cle::de(&"lancer"),
                    rect(5, 7, 6, 2),
                    "Lancer",
                    ActionEcran::Lancer,
                );
            }
            Etape::Fin => {
                let _: &mut Noeud = s.bouton(
                    Cle::de(&"rejouer"),
                    rect(5, 7, 6, 2),
                    "Rejouer",
                    ActionEcran::Rejouer,
                );
            }
            Etape::Manche | Etape::Prolongation => {}
        }
    }

    fn vue_telephone(&self, r: &Regard) -> Montre<VueTel> {
        let joueur = r.appareil().and_then(|a| self.joueurs.get(a));
        let vue = match (joueur, self.phase) {
            (None, _) => VueTel::Prenom,
            (Some(_), Etape::Salon) => VueTel::Attends,
            (Some(j), Etape::Manche | Etape::Prolongation) => VueTel::Tape { points: j.points },
            (Some(j), Etape::Fin) => VueTel::Resultat { points: j.points },
        };
        Montre {
            vue,
            effets: Vec::new(),
            reglages: Reglages {
                ecran_allume: Oui::Non,
                fond: None,
                orientation: None,
            },
        }
    }

    fn ressource(&self, _: Cle) -> Option<&[u8]> {
        None
    }

    fn phase(&self) -> Etape {
        self.phase
    }

    fn reveil(&self) -> Reveil {
        self.fin.map_or(Reveil::Jamais, Reveil::A)
    }

    fn attente(&self, a: &mut Attente<'_, Self>) {
        match (self.phase, self.fin) {
            (_, Some(fin)) => a.echeance(fin),
            (Etape::Salon, None) => {
                if D::salon_attend_chacun() == Oui::Oui && !self.joueurs.is_empty() {
                    if D::salon_libre() == Oui::Oui {
                        a.libre(jeu::raison!("le salon attend le « Prêt » de chacun"));
                    }
                    for (qui, j) in self.joueurs.iter() {
                        if j.pret == Oui::Non {
                            a.joueur(qui, &[GesteTel::Pret]);
                        }
                    }
                } else {
                    a.ecran(&[ActionEcran::Lancer]);
                }
            }
            (Etape::Fin, None) => a.ecran(&[ActionEcran::Rejouer]),
            (Etape::Manche | Etape::Prolongation, None) => a.ecran(&[ActionEcran::Lancer]),
        }
    }

    fn accueil(&self) -> Accueil {
        if self.joueurs.len() >= JOUEURS_MAX {
            Accueil::Pleine
        } else {
            Accueil::Ouverte
        }
    }

    fn demandes<'s>(&'s self, _: &mut Demandes<'s>) {}

    fn previsible(_: &GesteTel) -> Prevision {
        Prevision::Non
    }

    fn garder(&self) -> Garde {
        Garde {
            joueurs: self.joueurs.clone(),
            parties: self.parties,
            vus: self.vus.clone(),
        }
    }

    fn convertir(a: Aucun) -> Result<Self, Raison> {
        match a {}
    }
}

impl<D: Defaut> Epreuve for Socle<D> {
    /// Les points, dans l'ordre d'arrivée.
    /// Les points de chaque joueur, par appareil : un joueur oublié à « Rejouer »
    /// quitte le résumé sans que les autres se décalent.
    type Resume = Vec<(Appareil, u32)>;
    type Cas = Etape;

    fn resume(&self, _: &Autorite) -> Vec<(Appareil, u32)> {
        self.joueurs.iter().map(|(a, j)| (a, j.points)).collect()
    }

    fn invariants(&self, _: &Autorite, v: &mut Verif) {
        jeu::exiger!(
            v,
            self.joueurs.iter().all(|(_, j)| j.points <= j.tapes),
            "un point par tape au plus"
        );
        jeu::exiger!(v, self.joueurs.len() <= JOUEURS_MAX, "six joueurs au plus");
        jeu::exiger!(
            v,
            self.meneur.is_none() || !self.joueurs.is_empty(),
            "un meneur est un joueur"
        );
    }

    fn transition(avant: &Vec<(Appareil, u32)>, apres: &Vec<(Appareil, u32)>, v: &mut Verif) {
        jeu::exiger!(
            v,
            avant.iter().all(|(a, p)| {
                apres
                    .iter()
                    .find(|(b, _)| b == a)
                    .is_none_or(|(_, q)| q >= p)
            }),
            "un point ne se perd pas"
        );
    }

    fn bot_telephone(_: &Self, m: &Montre<VueTel>, b: &mut Bot<'_, Self>) {
        let deux = NonZeroU32::MIN.saturating_add(1);
        let trois = deux.saturating_add(1);
        match &m.vue {
            VueTel::Prenom => {
                let nom = match b.style() {
                    Style::Hostile => b.texte_hostile(),
                    Style::Sage | Style::Lent => "Camille",
                };
                b.geste(GesteTel::Rejoindre(Texte::nettoyer(nom)));
            }
            VueTel::Attends => {
                // Un geste hors de propos, de temps en temps : un refus (E6).
                if b.hasard().une_sur(trois) == Oui::Oui {
                    b.geste(GesteTel::Pret);
                } else {
                    b.rien();
                }
            }
            VueTel::Tape { .. } => {
                if b.hasard().une_sur(deux) == Oui::Oui {
                    b.geste(GesteTel::Taper);
                }
                if b.hasard().une_sur(trois) == Oui::Oui {
                    let c = b.hasard().dans::<0, 9>();
                    b.geste(GesteTel::Choisir(u8::try_from(c.get()).unwrap_or(0)));
                }
            }
            VueTel::Resultat { .. } => {
                if b.hasard().une_sur(trois) == Oui::Oui {
                    b.geste(GesteTel::Taper);
                } else {
                    b.rien();
                }
            }
        }
    }

    fn bot_ecran(j: &Self, b: &mut Bot<'_, Self>) {
        match j.phase {
            // Quatre secondes au salon : assez pour qu'une coupure de moins de deux
            // secondes y tienne tout entière (E11, l'hypothèse lue pendant toute la
            // coupure).
            Etape::Salon if b.depuis_vue() > Duree::s(4) => b.action(ActionEcran::Lancer),
            Etape::Fin if b.depuis_vue() > Duree::s(2) => b.action(ActionEcran::Rejouer),
            Etape::Salon | Etape::Fin | Etape::Manche | Etape::Prolongation => b.rien(),
        }
    }

    fn cas(&self) -> Etape {
        self.phase
    }

    fn pire_cas(c: Etape, construction: &Construction<'_>) -> Self {
        let m = construction.maintenant();
        let mut joueurs = Appareils::new();
        for a in construction.appareils().take(JOUEURS_MAX) {
            let _: Option<Joueur> = joueurs.inserer(
                a,
                Joueur {
                    prenom: Texte::nettoyer("WWWWWWWWWWWW"),
                    points: 9_999,
                    tapes: 9_999,
                    choix: 34,
                    moyenne: D::moyenne(9_999, 9_999),
                    present: Oui::Oui,
                    pret: Oui::Non,
                },
            );
        }
        let meneur = joueurs.iter().next().map(|(a, _)| a);
        let mut mot = Secret::cacher(Qui::Personne, Texte::nettoyer("CAROTTIER"));
        if c == Etape::Fin {
            mot.montrer(Qui::Tous);
        }
        Socle {
            defaut: D::neuf(),
            phase: c,
            phase_depuis: m.pas(),
            fin: match c {
                Etape::Manche => Some(Echeance::dans(&m, MANCHE)),
                Etape::Prolongation => Some(Echeance::dans(&m, D::prolongation())),
                Etape::Salon | Etape::Fin => None,
            },
            mot,
            indice: None,
            joueurs,
            meneur,
            manche: 9_999,
            parties: 9_999,
            vus: Vec::new(),
        }
    }

    fn proprietes() -> Proprietes<Self> {
        Proprietes {
            coupure_transparente: if D::coupure_transparente() == Oui::Oui {
                Applicable::Oui(Coupure {
                    moins_de: Duree::s(2),
                    hypothese: |j, _, a| j.coupure_sans_effet(a),
                })
            } else {
                Applicable::SansObjet(jeu::raison!(
                    "une tape reçue tard ne compte pas : une coupure peut changer les points"
                ))
            },
            permutation: Applicable::Oui(()),
        }
    }
}
