//! `devine`, le jeu d'exemple de la phase 1 (INTERFACE.md, § 9) : chaque téléphone
//! propose un chiffre ; l'autorité en a tiré un, secret ; à l'échéance, la télé révèle.
//!
//! Il touche chaque partie de l'interface : secret, hasard, refus, échéance, tenu,
//! présence, demande, vue, bots, invariants. Ce que l'exemple montre de la forme :
//!
//! - « déjà proposé » est refusé avant `accepter`, donc sans toucher au générateur ;
//! - un chiffre tiré n'existe qu'après `accepter` ;
//! - la télé ne voit le nombre qu'une fois `montrer(Qui::Tous)` fait par `reveler` ;
//! - l'échéance de la manche naît de `Maintenant` dans `ouvrir_manche` ;
//! - la ligne de résultats est une demande, écrite une fois, et retirée à sa réponse ;
//! - un geste d'une manche passée tombe sur `quand.vu`.
//!
//! Écarts à § 9 :
//!
//! - un `Rejoindre` d'un joueur déjà là ne change que son prénom. Le texte de § 9
//!   remplaçait le joueur, et lui faisait perdre ses points, ce que l'invariant « un
//!   point ne se perd pas » refuse ;
//! - l'invariant « les lignes écrites se retirent » de § 9 (`lignes.len() <= 2`) est
//!   faux, et la règle juste : trois manches révélées sans réponse laissent trois lignes
//!   en attente. § 9 dit « une ligne de résultats par manche, jusqu'à sa réponse », et
//!   § 2.10 qu'une demande existe tant que le jeu la liste : rien ne borne le temps de la
//!   réponse, qui dépend de l'ordinateur de la soirée. Borner les lignes demanderait de
//!   refuser `Lancer` (contre SQ-78, « un échec ne bloque rien ») ou de retirer une ligne
//!   sans réponse (une demande retirée est annulée : contre « écrite une fois »).
//!   L'invariant dit donc ce que la règle garantit : une ligne par manche révélée au
//!   plus, chacune sous sa propre identité.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;

use jeu::prelude::*;

jeu::limite!(pub Prenom = 12 Capitales);

/// La durée d'une manche d'essais.
const ESSAIS: Duree = Duree::s(20);
/// Le flux des résultats.
const RESULTATS: Flux = Flux::new("resultats");
/// Le plus de joueurs.
const JOUEURS_MAX: usize = 8;

/// Un chiffre, de 0 à 9.
pub type Chiffre = Borne<0, 9>;

/// L'état de la partie.
#[jeu::etat]
pub struct Devine {
    phase: Etape,
    phase_depuis: Pas,
    fin: Option<Echeance>,
    /// `Qui::Personne`, puis `Qui::Tous` à la révélation.
    nombre: Secret<Chiffre>,
    joueurs: Appareils<Joueur>,
    manche: u32,
    /// Une ligne de résultats par manche, jusqu'à sa réponse, si tardive soit-elle.
    lignes: Vec<(IdDemande, Vec<u8>)>,
}

/// Un joueur.
#[jeu::etat]
pub struct Joueur {
    prenom: Texte<Prenom>,
    /// Ce qu'il a proposé : lui seul le voit, jusqu'à la révélation.
    essai: Secret<Option<Chiffre>>,
    /// Où il a laissé son curseur : lui seul le voit. Public, il disait l'essai à tous
    /// dès qu'il était proposé (revue de la phase 2, constat 2).
    curseur: Secret<Chiffre>,
    points: u32,
    present: Oui,
}

/// Où en est la partie.
#[jeu::message]
#[derive(Copy, Eq, Variantes)]
pub enum Etape {
    /// On attend que l'écran lance une manche.
    Salon,
    /// Les téléphones proposent.
    Essais,
    /// La télé montre le nombre.
    Revelation,
}

impl Phase for Etape {
    fn terminale(self) -> Oui {
        Oui::Non
    }
}

/// Ce qu'envoie un téléphone.
#[jeu::message]
pub enum GesteTel {
    /// Entrer dans la partie, ou changer de prénom.
    Rejoindre(Texte<Prenom>),
    /// Proposer le chiffre du curseur.
    Proposer,
}

/// Ce qu'envoie l'écran.
#[jeu::message]
#[derive(Copy, Eq)]
pub enum ActionEcran {
    /// Lancer une manche.
    Lancer,
    /// Rejouer (sans effet ici).
    Rejouer,
}

/// Ce que dit un refus.
#[jeu::message]
pub enum Avis {
    /// Pas pendant cette phase, ou sans être joueur.
    PasMaintenant,
    /// Ce joueur a déjà proposé.
    DejaPropose,
    /// Ce geste vient d'une manche passée.
    TropTard,
}

/// Ce que voit un téléphone.
#[jeu::message]
pub enum VueTel {
    /// Choisir un prénom.
    Prenom,
    /// Attendre.
    Attends,
    /// Choisir un chiffre.
    Choisis {
        /// Le curseur.
        curseur: Chiffre,
    },
    /// Le résultat de la manche.
    Resultat {
        /// Le nombre tiré.
        nombre: Chiffre,
        /// Ce joueur a-t-il trouvé ?
        gagne: Oui,
    },
}

/// Le chiffre que le joueur fait défiler : un tenu, le dernier gagne.
#[jeu::message]
pub struct Curseur(pub Chiffre);

impl Tenu for Curseur {
    type Cle = ();
    fn cle(&self) {}
    // Un curseur ne se relâche jamais seul.
    fn relache(&self) -> Option<Self> {
        None
    }
}

const fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

/// Le grand chiffre, au centre.
const GRAND: Rect = rect(5, 2, 6, 5);
/// Le compte à rebours, en haut.
const HAUT: Rect = rect(5, 0, 6, 1);
/// Le bouton, en bas.
const BAS: Rect = rect(4, 7, 8, 2);
/// Le style des textes.
const STYLE: StyleTexte = StyleTexte {
    police: Cle::POLICE_DU_COMMUN,
    taille: Taille::Budget {
        max: Fixe::unites(4),
        min: Fixe::unites(1),
    },
    couleur: Rgba::BLANC,
    alignement: Alignement::Centre,
    lignes: Lignes::Une,
    coupe: Coupe::Graphemes,
};

/// Le texte d'un chiffre.
fn chiffre(c: Chiffre) -> &'static str {
    const CHIFFRES: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    CHIFFRES.get(c.indice()).copied().unwrap_or("?")
}

/// Les clés de la page du téléphone (PHASE-3, § 2.10).
mod page {
    use jeu::prelude::Cle;

    /// La colonne de la page.
    pub(crate) const COLONNE: Cle = Cle::nommee("page");
    /// Le titre.
    pub(crate) const TITRE: Cle = Cle::nommee("titre");
    /// Le champ du prénom.
    pub(crate) const PRENOM: Cle = Cle::nommee("prénom");
    /// Le bouton qui envoie le prénom.
    pub(crate) const REJOINDRE: Cle = Cle::nommee("rejoindre");
    /// Le carrousel des chiffres.
    pub(crate) const CURSEUR: Cle = Cle::nommee("curseur");
    /// Le bouton qui propose.
    pub(crate) const PROPOSER: Cle = Cle::nommee("proposer");
}

/// Les options du carrousel des chiffres, chacune par la clé de son numéro.
const OPTIONS_CHIFFRES: [(Cle, &str); 10] = [
    (Cle::numero(0), "0"),
    (Cle::numero(1), "1"),
    (Cle::numero(2), "2"),
    (Cle::numero(3), "3"),
    (Cle::numero(4), "4"),
    (Cle::numero(5), "5"),
    (Cle::numero(6), "6"),
    (Cle::numero(7), "7"),
    (Cle::numero(8), "8"),
    (Cle::numero(9), "9"),
];

/// Le style des textes de la page : la police du commun, centrée.
const STYLE_PAGE: StylePage = StylePage {
    alignement: Alignement::Centre,
    ..StylePage::ORDINAIRE
};

impl Devine {
    /// Une manche commence : un nombre neuf, caché ; les essais vidés ; l'échéance née de
    /// maintenant (PUN-34).
    fn ouvrir_manche(&mut self, m: &Maintenant<'_>, nouveau: Chiffre, a: &Autorite) {
        self.phase = Etape::Essais;
        self.phase_depuis = m.pas();
        self.fin = Some(Echeance::dans(m, ESSAIS));
        self.manche = self.manche.saturating_add(1);
        self.nombre = Secret::cacher(Qui::Personne, nouveau);
        for (de, j) in self.joueurs.iter_mut() {
            if let Ok(e) = j.essai.lire_mut(a) {
                *e = None;
            }
            j.essai.montrer(Qui::Appareil(de));
        }
    }

    /// L'échéance est passée : le nombre et les essais se montrent, les points se
    /// comptent.
    fn reveler(&mut self, m: &Maintenant<'_>, a: &Autorite) {
        self.phase = Etape::Revelation;
        self.phase_depuis = m.pas();
        self.fin = None;
        self.nombre.montrer(Qui::Tous);
        let nombre = self.nombre.lire(a).ok().copied();
        for (_, j) in self.joueurs.iter_mut() {
            if nombre.is_some() && j.essai.lire(a).ok().copied().flatten() == nombre {
                j.points = j.points.saturating_add(1);
            }
            j.essai.montrer(Qui::Tous);
        }
    }

    /// La ligne de résultats de la manche : son numéro, puis les points de chacun.
    fn ligne_de_resultats(&self) -> Vec<u8> {
        let points: Vec<u32> = self.joueurs.iter().map(|(_, j)| j.points).collect();
        jeu::octets(&(self.manche, points))
    }

    /// Ce joueur a-t-il déjà proposé dans cette manche ?
    fn a_deja_propose(&self, a: &Autorite, qui: Appareil) -> Oui {
        let propose = self
            .joueurs
            .get(qui)
            .and_then(|j| j.essai.lire(a).ok().copied().flatten())
            .is_some();
        if propose { Oui::Oui } else { Oui::Non }
    }
}

/// Proposer le chiffre du curseur : refusé avant `accepter` s'il est trop tard, si le
/// téléphone n'est pas un joueur, ou s'il a déjà proposé, donc sans toucher au
/// générateur (PUN-27).
fn proposer(avant: Avant<'_, Devine>, de: Appareil, quand: Quand) -> Issue<Devine> {
    let j = avant.jeu();
    if j.phase != Etape::Essais || quand.vu < j.phase_depuis {
        return avant.refuser(Avis::TropTard);
    }
    let Some(joueur) = j.joueurs.get(de) else {
        return avant.refuser(Avis::PasMaintenant);
    };
    match joueur.essai.lire(avant.autorite()) {
        Ok(Some(_)) => return avant.refuser(Avis::DejaPropose),
        Ok(None) => {}
        Err(a) => return avant.attendre(a),
    }
    let mut e = avant.accepter();
    let p = e.parts();
    if let Some(joueur) = p.jeu.joueurs.get_mut(de) {
        let curseur = joueur.curseur.lire(p.autorite).ok().copied();
        if let (Some(c), Ok(essai)) = (curseur, joueur.essai.lire_mut(p.autorite)) {
            *essai = Some(c);
        }
    }
    e.fin()
}

/// Entrer dans la partie, ou changer de prénom.
fn rejoindre(avant: Avant<'_, Devine>, de: Appareil, prenom: Texte<Prenom>) -> Issue<Devine> {
    if prenom.vide() == Oui::Oui {
        return avant.refuser(Avis::PasMaintenant);
    }
    let deja = avant.jeu().joueurs.get(de).is_some();
    if !deja && avant.jeu().joueurs.len() >= JOUEURS_MAX {
        return avant.refuser(Avis::PasMaintenant);
    }
    let mut e = avant.accepter();
    if let Some(j) = e.jeu().joueurs.get_mut(de) {
        j.prenom = prenom;
        j.present = Oui::Oui;
    } else {
        let joueur = Joueur {
            prenom,
            essai: Secret::cacher(Qui::Appareil(de), None),
            curseur: Secret::cacher(Qui::Appareil(de), Chiffre::PLUS_PETIT),
            points: 0,
            present: Oui::Oui,
        };
        let _: Option<Joueur> = e.jeu().joueurs.inserer(de, joueur);
    }
    e.fin()
}

/// Lancer une manche : le chiffre n'est tiré qu'après `accepter`.
fn lancer(avant: Avant<'_, Devine>, quand: Quand) -> Issue<Devine> {
    let j = avant.jeu();
    if j.phase == Etape::Essais || quand.vu < j.phase_depuis {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    let p = e.parts();
    let nouveau = match p.sorties.hasard() {
        Ok(h) => h.dans::<0, 9>(),
        Err(a) => return e.attendre(a),
    };
    p.jeu.ouvrir_manche(&p.maintenant, nouveau, p.autorite);
    e.fin()
}

/// Le temps : à l'échéance, la révélation, et une ligne de résultats demandée.
fn temps(avant: Avant<'_, Devine>) -> Issue<Devine> {
    let j = avant.jeu();
    let Some(fin) = j.fin else {
        return avant.ignorer();
    };
    if fin.atteinte(&avant.maintenant()) == Oui::Non {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    let p = e.parts();
    p.jeu.reveler(&p.maintenant, p.autorite);
    let id = p.sorties.id();
    let ligne = p.jeu.ligne_de_resultats();
    p.jeu.lignes.push((id, ligne));
    e.fin()
}

/// Un départ ou un retour : le jeu note la présence, et ne libère rien (le commun non
/// plus).
fn marquer(avant: Avant<'_, Devine>, a: Appareil, present: Oui) -> Issue<Devine> {
    if avant.jeu().joueurs.get(a).is_none() {
        return avant.ignorer();
    }
    let mut e = avant.accepter();
    if let Some(j) = e.jeu().joueurs.get_mut(a) {
        j.present = present;
    }
    e.fin()
}

impl Jeu for Devine {
    type ActionEcran = ActionEcran;
    type GesteTelephone = GesteTel;
    type TenuTelephone = Curseur;
    type VueTelephone = VueTel;
    type Avis = Avis;
    type Phase = Etape;
    type Garde = Appareils<Joueur>;
    type Precedent = Aucun;

    const DECLARATION: Declaration = Declaration {
        nom: "devine",
        places: Places::joueurs(1, 8).appareils(16),
        rattrapage: 1,
        fenetre: Applicable::SansObjet(jeu::raison!("tour par tour, le pas vu suffit")),
        antidatage_max: Duree::ms(30),
        pause: Pause::Aucune(jeu::raison!("chaque phase a une échéance")),
        ecran: FormeEcran::Scene {
            largeur: 16,
            hauteur: 9,
            zone_sure_pour_cent: 90,
            echelle_entiere: Oui::Non,
        },
        telephones: Applicable::Oui(Telephones {
            repartition: Repartition::UnParJoueur,
            tenu: ReglesTenu {
                fraicheur: Applicable::SansObjet(jeu::raison!(
                    "le curseur reste où le joueur l'a laissé"
                )),
                age_max: Applicable::SansObjet(jeu::raison!(
                    "seul le dernier curseur compte, même tardif"
                )),
                debit_max: 10,
            },
        }),
        manettes: Applicable::SansObjet(jeu::raison!("aucune manette n'a de rôle ici")),
        prediction: Applicable::SansObjet(jeu::raison!(
            "un échange toutes les quelques secondes suffit"
        )),
        hasard: Applicable::Oui(()),
        secrets: Applicable::Oui(()),
        fin: Applicable::SansObjet(jeu::raison!("les manches s'enchaînent sans fin de partie")),
        capacites: Voulues {
            vibration: Oui::Non,
            mouvement: Oui::Non,
            son: Oui::Non,
            ecran_allume: Oui::Oui,
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
        mecaniques: &[Mecanique::Hasard, Mecanique::Texte, Mecanique::TourParTour],
    };

    fn creer(mut c: Creation<'_, Self>) -> Self {
        let nombre = match c.hasard() {
            Ok(h) => h.dans::<0, 9>(),
            Err(Absent) => Chiffre::PLUS_PETIT,
        };
        let pas = c.maintenant().pas();
        Devine {
            phase: Etape::Salon,
            phase_depuis: pas,
            fin: None,
            nombre: Secret::cacher(Qui::Personne, nombre),
            joueurs: c.garde().unwrap_or_else(Appareils::new),
            manche: 0,
            lignes: Vec::new(),
        }
    }

    fn appliquer(avant: Avant<'_, Self>, e: Entree<'_, Self>) -> Issue<Self> {
        match e {
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Proposer,
                quand,
            }) => proposer(avant, de, quand),
            Entree::Geste(Geste {
                de,
                geste: GesteTel::Rejoindre(prenom),
                ..
            }) => rejoindre(avant, de, prenom),
            Entree::Tenu(Changement {
                de,
                valeur: Curseur(c),
                ..
            }) => {
                if avant.jeu().joueurs.get(de).is_none() {
                    return avant.ignorer();
                }
                let mut e = avant.accepter();
                if let Some(j) = e.jeu().joueurs.get_mut(de) {
                    j.curseur = Secret::cacher(Qui::Appareil(de), c);
                }
                e.fin()
            }
            Entree::Ecran(DeEcran::Action {
                action: ActionEcran::Lancer,
                quand,
            }) => lancer(avant, quand),
            Entree::Temps(_) => temps(avant),
            Entree::Reponse(id, Reponse::Fait(_)) => {
                let mut e = avant.accepter();
                // Un échec ne bloque rien (SQ-78).
                e.jeu().lignes.retain(|(i, _)| *i != id);
                e.fin()
            }
            Entree::Presence(Presence::Depart { appareil, .. }) => {
                marquer(avant, appareil, Oui::Non)
            }
            Entree::Presence(Presence::Retour { appareil, .. }) => {
                marquer(avant, appareil, Oui::Oui)
            }
            Entree::Presence(Presence::Arrivee { .. } | Presence::Capacites { .. })
            | Entree::Ecran(
                DeEcran::Action {
                    action: ActionEcran::Rejouer,
                    ..
                }
                | DeEcran::Touche { .. }
                | DeEcran::Pointeur { .. }
                | DeEcran::Composant { .. },
            )
            | Entree::Reponse(_, Reponse::Image(_) | Reponse::Rendu(_) | Reponse::Lu(_))
            | Entree::Manette(_) => avant.ignorer(),
        }
    }

    fn vue_ecran(&self, r: &Regard, s: &mut Scene<ActionEcran>) {
        if let Some(n) = self.nombre.voir(r) {
            let _: &mut Noeud =
                s.texte(Cle::de(&("nombre", self.manche)), chiffre(*n), GRAND, STYLE);
        }
        if let Some(fin) = self.fin {
            let _: &mut Noeud = s.compte_a_rebours(
                Cle::de(&"temps"),
                CompteARebours::Court { echeance: fin },
                HAUT,
                STYLE,
            );
        }
        let _: &mut Noeud = s.bouton(
            Cle::de(&"lancer"),
            BAS,
            "Lancer (Espace)",
            ActionEcran::Lancer,
        );
        s.raccourci(
            CodeTouche::ESPACE,
            Mods::AUCUN,
            Oui::Non,
            ActionEcran::Lancer,
        );
        s.effet(Effet {
            id: IdEffet::de(&("phase", self.manche, self.phase)),
            quand: Instant::de(self.phase_depuis),
            quoi: Sensation::Son(Son::Signal(Signal::Avance)),
            tenue: Tenue::Definitive,
            peremption: Duree::ms(250),
        });
    }

    fn vue_telephone(&self, r: &Regard) -> Montre<VueTel> {
        let joueur = r.appareil().and_then(|a| self.joueurs.get(a));
        let vue = match (joueur, self.phase) {
            (None, _) => VueTel::Prenom,
            (Some(_), Etape::Salon) => VueTel::Attends,
            (Some(j), Etape::Essais) => match j.essai.voir(r) {
                Some(Some(_)) => VueTel::Attends,
                Some(None) | None => VueTel::Choisis {
                    curseur: j.curseur.voir(r).copied().unwrap_or(Chiffre::PLUS_PETIT),
                },
            },
            (Some(j), Etape::Revelation) => match self.nombre.voir(r) {
                Some(n) => VueTel::Resultat {
                    nombre: *n,
                    gagne: if j.essai.voir(r).copied().flatten() == Some(*n) {
                        Oui::Oui
                    } else {
                        Oui::Non
                    },
                },
                None => VueTel::Attends,
            },
        };
        Montre {
            vue,
            effets: Vec::new(),
            reglages: Reglages {
                ecran_allume: Oui::Oui,
                fond: None,
                orientation: None,
            },
        }
    }

    fn ressource(&self, _: Cle) -> Option<&[u8]> {
        None
    }

    /// La page du téléphone (PHASE-3, § 2.10) : le prénom, l'attente, le carrousel des
    /// chiffres (son choix est le curseur tenu) et le résultat. Elle ne lit que la vue
    /// et les locaux.
    fn page(vue: &Recue<VueTel>, l: &Locaux<'_>, p: &mut Page<Self>) {
        p.colonne(page::COLONNE, |p| match vue.montre.vue {
            VueTel::Prenom => {
                p.texte(page::TITRE, "Ton prénom ?", STYLE_PAGE);
                let prenom = l
                    .texte::<Prenom>(page::PRENOM)
                    .unwrap_or_else(|| Texte::nettoyer(""));
                p.champ(
                    page::PRENOM,
                    ChampPage {
                        exemple: "Léa",
                        clavier: Clavier::Prenom,
                        entree: Some(GesteTel::Rejoindre(prenom.clone())),
                        ..ChampPage::de::<Prenom>()
                    },
                );
                p.bouton(
                    page::REJOINDRE,
                    "C'est moi",
                    GesteTel::Rejoindre(prenom),
                    Declencheur::Relache,
                    STYLE_PAGE,
                );
            }
            VueTel::Attends => p.texte(page::TITRE, "La manche va commencer…", STYLE_PAGE),
            VueTel::Choisis { curseur } => {
                p.texte(page::TITRE, "Choisis un chiffre", STYLE_PAGE);
                p.choix(page::CURSEUR, &OPTIONS_CHIFFRES, Oui::Oui);
                // Le choix du carrousel est le curseur ; une option hors des dix garde
                // celui de la vue.
                let choisi = l
                    .choix(page::CURSEUR)
                    .and_then(|n| Chiffre::new(i64::from(n)))
                    .unwrap_or(curseur);
                p.tenir(Curseur(choisi));
                p.bouton(
                    page::PROPOSER,
                    "Proposer",
                    GesteTel::Proposer,
                    Declencheur::Contact,
                    STYLE_PAGE,
                );
            }
            VueTel::Resultat { nombre, gagne } => {
                let titre = match gagne {
                    Oui::Oui => "Trouvé !",
                    Oui::Non => "Raté…",
                };
                p.texte(page::TITRE, titre, STYLE_PAGE);
                p.texte(Cle::nommee("nombre"), chiffre(nombre), STYLE_PAGE);
            }
        });
    }

    fn phase(&self) -> Etape {
        self.phase
    }

    fn reveil(&self) -> Reveil {
        self.fin.map_or(Reveil::Jamais, Reveil::A)
    }

    fn attente(&self, a: &mut Attente<'_, Self>) {
        match self.fin {
            Some(fin) => {
                a.echeance(fin);
                // Les joueurs présents qui n'ont pas encore proposé. Leur essai est un
                // secret : l'attente le lit par son autorité, sans rien montrer à
                // personne (besoin 7 des jeux, `conception/BESOINS-DES-JEUX.md`).
                let attendus: Vec<Appareil> = self
                    .joueurs
                    .iter()
                    .filter(|(_, j)| {
                        j.present == Oui::Oui && matches!(j.essai.lire(a.autorite()), Ok(None))
                    })
                    .map(|(de, _)| de)
                    .collect();
                for de in attendus {
                    a.joueur(de, &[GesteTel::Proposer]);
                }
            }
            None => a.ecran(&[ActionEcran::Lancer]),
        }
    }

    fn accueil(&self) -> Accueil {
        if self.joueurs.len() >= JOUEURS_MAX {
            Accueil::Pleine
        } else {
            Accueil::Ouverte
        }
    }

    fn demandes<'s>(&'s self, d: &mut Demandes<'s>) {
        for (id, ligne) in &self.lignes {
            d.ajouter(*id, || Demande::Ligne {
                flux: RESULTATS,
                octets: ligne,
            });
        }
    }

    fn previsible(_: &GesteTel) -> Prevision {
        Prevision::Non
    }

    fn garder(&self) -> Appareils<Joueur> {
        self.joueurs.clone()
    }

    fn convertir(a: Aucun) -> Result<Self, Raison> {
        match a {}
    }
}

impl Epreuve for Devine {
    /// Les points, dans l'ordre d'arrivée.
    type Resume = Vec<u32>;
    type Cas = Etape;
    const INVARIANTS: &'static [&'static str] = &[
        "au plus un point par manche",
        "une ligne de résultats par manche révélée, au plus",
        "huit joueurs au plus",
        "un point ne se perd pas",
    ];

    fn resume(&self, _: &Autorite) -> Vec<u32> {
        self.joueurs.iter().map(|(_, j)| j.points).collect()
    }

    fn invariants(&self, _: &Autorite, v: &mut Verif) {
        jeu::exiger!(
            v,
            self.joueurs.iter().all(|(_, j)| j.points <= self.manche),
            "au plus un point par manche"
        );
        // Une réponse peut tarder autant que l'ordinateur de la soirée : les lignes ne se
        // bornent pas en nombre, mais chacune vient d'une manche révélée, sous sa propre
        // identité (CAT-76).
        let distinctes = self.lignes.iter().enumerate().all(|(k, (id, _))| {
            self.lignes
                .iter()
                .skip(k.saturating_add(1))
                .all(|(autre, _)| autre != id)
        });
        let manches = usize::try_from(self.manche).unwrap_or(usize::MAX);
        jeu::exiger!(
            v,
            distinctes && self.lignes.len() <= manches,
            "une ligne de résultats par manche révélée, au plus"
        );
        jeu::exiger!(v, self.joueurs.len() <= JOUEURS_MAX, "huit joueurs au plus");
    }

    fn transition(avant: &Vec<u32>, apres: &Vec<u32>, v: &mut Verif) {
        jeu::exiger!(
            v,
            avant.iter().zip(apres).all(|(a, b)| b >= a),
            "un point ne se perd pas"
        );
    }

    fn bot_telephone(_: &Self, m: &Montre<VueTel>, b: &mut Bot<'_, Self>) {
        match &m.vue {
            VueTel::Prenom => {
                let t = b.texte_hostile();
                b.geste(GesteTel::Rejoindre(Texte::nettoyer(t)));
            }
            VueTel::Choisis { .. } if b.depuis_vue() > Duree::ms(800) => {
                let c = b.hasard().dans::<0, 9>();
                b.tenir(Curseur(c));
                b.geste(GesteTel::Proposer);
            }
            VueTel::Choisis { .. } | VueTel::Attends | VueTel::Resultat { .. } => b.rien(),
        }
    }

    fn bot_ecran(j: &Self, b: &mut Bot<'_, Self>) {
        if j.fin.is_none() && b.depuis_vue() > Duree::s(2) {
            b.action(ActionEcran::Lancer);
        } else {
            b.rien();
        }
    }

    fn cas(&self) -> Etape {
        self.phase
    }

    /// Huit prénoms de douze capitales larges, des points à 9 999, pour chaque étape.
    fn pire_cas(c: Etape, construction: &Construction<'_>) -> Self {
        let m = construction.maintenant();
        let mut joueurs = Appareils::new();
        for (i, a) in construction.appareils().take(JOUEURS_MAX).enumerate() {
            let essai = match c {
                Etape::Salon | Etape::Essais => None,
                Etape::Revelation => Chiffre::new(i64::try_from(i % 10).unwrap_or(0)),
            };
            let qui = match c {
                Etape::Salon | Etape::Essais => Qui::Appareil(a),
                Etape::Revelation => Qui::Tous,
            };
            let _: Option<Joueur> = joueurs.inserer(
                a,
                Joueur {
                    prenom: Texte::nettoyer("WWWWWWWWWWWW"),
                    essai: Secret::cacher(qui, essai),
                    curseur: Secret::cacher(Qui::Appareil(a), Chiffre::PLUS_GRAND),
                    points: 9_999,
                    present: Oui::Oui,
                },
            );
        }
        let mut nombre = Secret::cacher(Qui::Personne, Chiffre::PLUS_GRAND);
        if c == Etape::Revelation {
            nombre.montrer(Qui::Tous);
        }
        Devine {
            phase: c,
            phase_depuis: m.pas(),
            fin: match c {
                Etape::Essais => Some(Echeance::dans(&m, ESSAIS)),
                Etape::Salon | Etape::Revelation => None,
            },
            nombre,
            joueurs,
            manche: 9_999,
            lignes: Vec::new(),
        }
    }

    fn proprietes() -> Proprietes<Self> {
        Proprietes {
            coupure_transparente: Applicable::Oui(Coupure {
                moins_de: Duree::s(10),
                hypothese: |j, a, qui| j.a_deja_propose(a, qui),
            }),
            permutation: Applicable::Oui(()),
        }
    }
}
