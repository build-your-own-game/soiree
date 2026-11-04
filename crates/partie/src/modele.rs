//! Un jeu écrit directement contre l'enveloppe, sans `jeu` ni WebAssembly : le modèle
//! sur lequel le cœur se teste d'abord (INTERFACE.md, § 8.6 et § 8.9, étape 3).
//!
//! Le jeu est un compteur. Un geste [`GesteModele::Ajouter`] l'augmente, et son
//! identité est gardée dans l'état, dans l'ordre où le jeu l'a reçu : les tests
//! vérifient ainsi l'ordre et « un événement compte exactement une fois ». Les
//! autres gestes provoquent, à la demande d'un test, chacune des issues que le
//! cœur doit savoir traiter : refus, attente d'un secret, panique, budget épuisé,
//! sorties, demandes, fautes en lecture. Les tenus, les manettes, les présences,
//! les réponses et le temps sont notés dans l'état. L'écran offre un bouton,
//! remettre à zéro, et un champ de huit points de code ; il montre le total, une
//! fois hors de tout sous-arbre et une fois sous un sous-arbre `definitif`, avec un
//! effet `Provisoire` et un effet `Definitive` nommés par lui (§ 4.4).
//!
//! Le modèle est plus strict qu'un vrai module sur un point : il rend
//! `Retour::Illisible` quand la sorte annoncée ne correspond pas à l'appel.
//!
//! Ses réglages ([`ReglageModele`]) choisissent sa déclaration ; ses compteurs
//! ([`Compteurs`]), partagés entre la fabrique et ses instances, disent combien
//! d'instances, d'instantanés et de restaurations le cœur a demandés.

use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
use core::cell::Cell;

use jeu_abi::{
    ABI, Accueil, Appel, ApplicableFil, AttenteFil, Bonjour, CauseTenu, Cle, CodeTouche,
    ComposantVuFil, ConstatFil, CoupFil, DeclarationFil, DemandeFil, Duree, EcranFil, EntreeFil,
    ErreurFil, EtatFil, Fixe, Flux, FormeEcran, Illisible, IssueFil, Lignes, Limites, Manette,
    Mecanique, MontreFil, NoeudFil, Oui, PauseFil, Places, Presence, Rect, RegardFil, Reglages,
    ReglesTenuFil, Repartition, Retour, ReveilFil, SceneFil, SorteAppel, SorteNoeud, SorteOctets,
    SortieFil, TelephonesFil, TenuFil, TexteCourt, Unite, Validation, Voulues, VueFil, decoder,
    encoder, fnv1a64_suite,
};
use serde::{Deserialize, Serialize};

use crate::instance::{CausePanne, Echec, Fabrique, Faute, Instance, PanneOutil};

/// L'empreinte des formes du modèle, dans `Bonjour` et la forme canonique.
pub const FORMES_MODELE: u64 = jeu_abi::fnv1a64(b"partie::modele, forme 2");

/// Un geste de téléphone du modèle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GesteModele {
    /// Accepté : ajoute `n` au total, et garde l'identité du geste.
    Ajouter(u8),
    /// Refusé, avec l'avis [`AvisModele::Refuse`].
    Refuser,
    /// L'étape rend `Attente`, comme si un secret manquait.
    Attendre,
    /// Le jeu panique.
    Paniquer,
    /// Le jeu ne rend pas la main : le budget s'épuise.
    Boucler,
    /// Accepté : désormais, chaque `Temps` panique.
    PaniquerAuTemps,
    /// Accepté : la sortie `Oublier` de l'appareil du geste.
    Oublier,
    /// Accepté : la sortie `Retirer` de l'appareil du geste.
    Retirer,
    /// Accepté : la sortie `NouvellePartie`, avec une graine fixe.
    NouvellePartie,
    /// Accepté : le jeu liste une demande neuve, une ligne de résultats.
    Demander,
    /// Accepté : le jeu ne liste plus aucune demande.
    Abandonner,
    /// Accepté : désormais, `Regarder` panique.
    PaniquerEnVue,
    /// Accepté : désormais, la scène de l'écran ne se décode pas.
    VueInvalide,
    /// Accepté : le jeu ne demande plus le temps.
    Dormir,
    /// Accepté : le jeu demande le temps à chaque pas.
    Reveiller,
}

/// L'état tenu d'un téléphone du modèle : un curseur par clé. 0 est sa valeur
/// relâchée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenuModele {
    /// La clé : un tenu par clé et par appareil.
    pub cle: u8,
    /// La valeur ; 0 est relâchée.
    pub valeur: u8,
}

/// L'action de l'écran du modèle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionModele {
    /// Remet le total à zéro.
    Remettre,
}

/// L'avis d'un refus du modèle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AvisModele {
    /// Le geste `Refuser`.
    Refuse,
}

/// La clé du bouton « Remettre » dans la scène du modèle.
pub const CLE_BOUTON: Cle = Cle::du_fil(1);
/// La clé du champ de la scène du modèle.
pub const CLE_CHAMP: Cle = Cle::du_fil(2);
/// La limite du champ, en points de code.
pub const LIMITE_CHAMP: u32 = 8;

/// L'état du jeu modèle, tel que sa forme canonique l'écrit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EtatModele {
    /// La somme des `Ajouter` acceptés depuis la dernière remise.
    pub total: u64,
    /// Le pas de la dernière entrée acceptée.
    pub pas: u64,
    /// Le nombre d'entrées acceptées.
    pub acceptees: u64,
    /// Les identités des `Ajouter` acceptés, dans l'ordre où le jeu les a reçus.
    pub gestes: Vec<u64>,
    /// La dernière valeur de chaque tenu : appareil, clé, valeur.
    pub tenus: Vec<(u64, u8, u8)>,
    /// Les changements de tenus reçus : appareil, clé, valeur, cause.
    pub changements: Vec<(u64, u8, u8, CauseTenu)>,
    /// Le dernier état de chaque manette.
    pub manettes: Vec<(u64, Manette)>,
    /// Les appareils présents, dans l'ordre de leur arrivée.
    pub presents: Vec<u64>,
    /// Le nombre de `Temps` reçus.
    pub temps: u64,
    /// La somme des sauts dits par les `Temps`, en pas.
    pub sauts: u64,
    /// Les demandes listées.
    pub demandes: Vec<u64>,
    /// L'identité de la prochaine demande.
    pub prochain_id: u64,
    /// Les demandes dont la réponse est arrivée, dans l'ordre.
    pub reponses: Vec<u64>,
    /// Les réponses arrivées pour une demande que le jeu ne listait pas.
    pub reponses_inattendues: u64,
    /// Les saisies reçues.
    pub saisies: u64,
    /// Ce que la partie précédente a gardé : son total.
    pub garde: u64,
    /// Quand le jeu veut le temps.
    pub reveil: ReveilFil,
    /// Chaque `Temps` panique-t-il ?
    pub panique_au_temps: Oui,
    /// `Regarder` panique-t-il ?
    pub panique_en_vue: Oui,
    /// La scène de l'écran est-elle illisible ?
    pub vue_invalide: Oui,
}

impl EtatModele {
    fn neuf(pas: u64, reveil: ReveilFil) -> EtatModele {
        EtatModele {
            total: 0,
            pas,
            acceptees: 0,
            gestes: Vec::new(),
            tenus: Vec::new(),
            changements: Vec::new(),
            manettes: Vec::new(),
            presents: Vec::new(),
            temps: 0,
            sauts: 0,
            demandes: Vec::new(),
            prochain_id: 1,
            reponses: Vec::new(),
            reponses_inattendues: 0,
            saisies: 0,
            garde: 0,
            reveil,
            panique_au_temps: Oui::Non,
            panique_en_vue: Oui::Non,
            vue_invalide: Oui::Non,
        }
    }
}

/// Le message de panique du modèle.
pub const MESSAGE_DE_PANIQUE: &str = "le modèle panique à la demande";

/// La graine que rend la sortie `NouvellePartie` du modèle.
pub const GRAINE_NOUVELLE_PARTIE: [u8; 32] = [7; 32];

/// Ce qui choisit la déclaration et le comportement du modèle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReglageModele {
    /// Les pas rattrapés au plus après un gel.
    pub rattrapage: u8,
    /// La fenêtre.
    pub fenetre: ApplicableFil<Duree>,
    /// L'antidatage permis.
    pub antidatage_max: Duree,
    /// Qui met en pause.
    pub pause: PauseFil,
    /// Les règles des tenus des téléphones.
    pub tenu: ReglesTenuFil,
    /// Les manettes.
    pub manettes: ApplicableFil<ReglesTenuFil>,
    /// Les limites.
    pub limites: Limites,
    /// Le réveil à la création.
    pub reveil: ReveilFil,
    /// Un modèle divergent : `Ajouter(n)` ajoute `n + 1`. Pour vérifier qu'un rejeu
    /// sur un autre jeu se voit.
    pub divergent: Oui,
}

impl ReglageModele {
    /// Le réglage ordinaire : sans fenêtre, temps à chaque pas, rattrapage de 3,
    /// tenus sans fraîcheur ni âge, débit de 60, manettes acceptées, pause du commun.
    pub fn ordinaire() -> ReglageModele {
        ReglageModele {
            rattrapage: 3,
            fenetre: sans_objet("le modèle n'a pas de fenêtre"),
            antidatage_max: Duree::ms(30),
            pause: PauseFil::DuCommun,
            tenu: ReglesTenuFil {
                fraicheur: sans_objet("le curseur ne se relâche pas seul"),
                age_max: sans_objet("le dernier curseur compte toujours"),
                debit_max: 60,
            },
            manettes: ApplicableFil::Oui(ReglesTenuFil {
                fraicheur: sans_objet("une manette branchée ne se tait pas"),
                age_max: sans_objet("une manette branchée n'a pas de retard"),
                debit_max: 60,
            }),
            limites: Limites {
                geste: 256,
                file: 32,
                vue: 4096,
                effets: 8,
                memoire: 64,
                brouillon: 1 << 20,
            },
            reveil: ReveilFil::ChaquePas,
            divergent: Oui::Non,
        }
    }
}

fn sans_objet<T>(raison: &str) -> ApplicableFil<T> {
    ApplicableFil::SansObjet(String::from(raison))
}

/// Ce que le cœur a demandé aux instances d'une fabrique.
#[derive(Debug, Default)]
pub struct Compteurs {
    /// Les instances fabriquées.
    pub fabriquees: Cell<u64>,
    /// Les instances encore vivantes.
    pub vivantes: Cell<u64>,
    /// Les instantanés pris.
    pub instantanes: Cell<u64>,
    /// Les restaurations.
    pub restaurations: Cell<u64>,
    /// Les appels d'écriture.
    pub ecritures: Cell<u64>,
}

fn incrementer(c: &Cell<u64>) {
    c.set(c.get().saturating_add(1));
}

/// Une instance du jeu modèle.
///
/// Chaque appel qui aboutit consomme 1 de carburant ; `Boucler` consomme tout le
/// budget et rend [`Faute::Budget`].
#[derive(Debug)]
pub struct InstanceModele {
    etat: Option<EtatModele>,
    carburant: Option<u64>,
    reglage: ReglageModele,
    compteurs: Rc<Compteurs>,
}

/// L'instantané du modèle : son état, cloné.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantaneModele(Option<EtatModele>);

impl InstanceModele {
    /// Une instance neuve, avant `Bonjour`, au réglage ordinaire.
    pub fn new() -> InstanceModele {
        InstanceModele::avec(ReglageModele::ordinaire(), Rc::new(Compteurs::default()))
    }

    /// Une instance neuve de ce réglage, qui compte dans ces compteurs.
    pub fn avec(reglage: ReglageModele, compteurs: Rc<Compteurs>) -> InstanceModele {
        incrementer(&compteurs.fabriquees);
        incrementer(&compteurs.vivantes);
        InstanceModele {
            etat: None,
            carburant: None,
            reglage,
            compteurs,
        }
    }

    /// L'état, pour les tests.
    pub fn etat(&self) -> Option<&EtatModele> {
        self.etat.as_ref()
    }
}

impl Default for InstanceModele {
    fn default() -> InstanceModele {
        InstanceModele::new()
    }
}

impl Drop for InstanceModele {
    fn drop(&mut self) {
        let v = &self.compteurs.vivantes;
        v.set(v.get().saturating_sub(1));
    }
}

/// Ce qui fabrique des instances du modèle, et compte ce qu'on leur demande.
#[derive(Debug, Clone)]
pub struct FabriqueModele {
    /// Le réglage des instances.
    pub reglage: ReglageModele,
    /// Les compteurs, partagés avec les instances et avec le test.
    pub compteurs: Rc<Compteurs>,
}

impl FabriqueModele {
    /// Une fabrique de ce réglage, avec des compteurs neufs.
    pub fn new(reglage: ReglageModele) -> FabriqueModele {
        FabriqueModele {
            reglage,
            compteurs: Rc::new(Compteurs::default()),
        }
    }
}

impl Default for FabriqueModele {
    fn default() -> FabriqueModele {
        FabriqueModele::new(ReglageModele::ordinaire())
    }
}

impl Fabrique for FabriqueModele {
    type I = InstanceModele;

    fn neuve(&mut self) -> Result<InstanceModele, PanneOutil> {
        Ok(InstanceModele::avec(
            self.reglage.clone(),
            Rc::clone(&self.compteurs),
        ))
    }
}

impl Instance for InstanceModele {
    type Instantane = InstantaneModele;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        self.carburant = Some(0);
        let mut retour = Vec::new();
        let Ok(a) = decoder::<Appel<'_>>(appel) else {
            illisible(Illisible::Forme, &mut retour)?;
            return Ok(retour);
        };
        if a.sorte() != sorte {
            illisible(Illisible::Ordre, &mut retour)?;
            return Ok(retour);
        }
        if sorte == SorteAppel::Ecriture {
            incrementer(&self.compteurs.ecritures);
        }
        if budget == 0 {
            return Err(Echec::Faute(Faute::Budget));
        }
        match self.executer(&a, &mut retour) {
            Ok(()) => {
                self.carburant = Some(1);
                Ok(retour)
            }
            Err(Echec::Faute(Faute::Budget)) => {
                self.carburant = Some(budget);
                Err(Echec::Faute(Faute::Budget))
            }
            Err(e) => Err(e),
        }
    }

    fn carburant(&self) -> Option<u64> {
        self.carburant
    }

    fn instantane(&mut self) -> Result<InstantaneModele, PanneOutil> {
        incrementer(&self.compteurs.instantanes);
        Ok(InstantaneModele(self.etat.clone()))
    }

    fn restaurer(&mut self, i: &InstantaneModele) -> Result<(), PanneOutil> {
        incrementer(&self.compteurs.restaurations);
        self.etat.clone_from(&i.0);
        Ok(())
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        let mut canonique = Vec::new();
        encoder(&(FORMES_MODELE, &self.etat), &mut canonique)
            .map_err(|e| PanneOutil::new(CausePanne::Interne, alloc::format!("{e}")))?;
        // Quatre FNV-1a salés : assez pour comparer deux modèles, pas une empreinte
        // cryptographique (le modèle n'a pas d'adversaire).
        let mut e = [0u8; 32];
        for (sel, morceau) in (0u8..).zip(e.as_chunks_mut::<8>().0) {
            let h = fnv1a64_suite(jeu_abi::fnv1a64(&[sel]), &canonique);
            morceau.copy_from_slice(&h.to_le_bytes());
        }
        Ok(e)
    }
}

impl InstanceModele {
    fn executer(&mut self, a: &Appel<'_>, sortie: &mut Vec<u8>) -> Result<(), Echec> {
        match a {
            Appel::Bonjour => ecrire(&Retour::Bonjour(bonjour()), sortie),
            Appel::Decrire => ecrire(
                &Retour::Decrit(Box::new(declaration_de(&self.reglage))),
                sortie,
            ),
            Appel::Creer {
                pas,
                presents,
                garde,
                ..
            } => self.creer(*pas, presents, *garde, sortie),
            Appel::Valider { sorte, octets } => repondre_valider(*sorte, octets, sortie),
            Appel::Entree { place, entree, .. } => {
                let divergent = self.reglage.divergent;
                let Some(etat) = self.etat.as_mut() else {
                    return illisible(Illisible::Ordre, sortie);
                };
                let Ok(e) = decoder::<EntreeFil<'_>>(entree) else {
                    return illisible(Illisible::Forme, sortie);
                };
                entrer(etat, *place, &e, divergent, sortie)
            }
            Appel::Regarder { regards } => match self.etat.as_ref() {
                Some(e) if e.panique_en_vue == Oui::Oui => Err(paniquer()),
                Some(e) => repondre_vues(e, regards, sortie),
                None => illisible(Illisible::Ordre, sortie),
            },
            Appel::Etat { connues } => match self.etat.as_ref() {
                Some(e) => repondre_etat(e, connues, sortie),
                None => illisible(Illisible::Ordre, sortie),
            },
            Appel::Ressources { .. } => ecrire(&Retour::Ressources(Vec::new()), sortie),
            Appel::Projeter { .. } | Appel::Canonique { .. } => {
                let mut canonique = Vec::new();
                encoder(&(FORMES_MODELE, &self.etat), &mut canonique).map_err(interne)?;
                ecrire(
                    &Retour::Morceau {
                        octets: &canonique,
                        suivant: None,
                    },
                    sortie,
                )
            }
            Appel::Charger { projection: octets } | Appel::DepuisCanonique { octets, .. } => {
                match decoder::<(u64, Option<EtatModele>)>(octets) {
                    Ok((FORMES_MODELE, etat)) => {
                        self.etat = etat;
                        ecrire(&Retour::Fait, sortie)
                    }
                    Ok(_) => illisible(Illisible::Version, sortie),
                    Err(e) => ecrire(&Retour::Illisible(e), sortie),
                }
            }
            Appel::Garder | Appel::Resumer => {
                let mut total = Vec::new();
                encoder(&self.etat.as_ref().map_or(0, |e| e.total), &mut total).map_err(interne)?;
                ecrire(&Retour::Octets(&total), sortie)
            }
            Appel::Verifier | Appel::Transition { .. } => ecrire(
                &Retour::Verif(vec![("le modèle tient toujours", ConstatFil::Tenu)]),
                sortie,
            ),
            Appel::Bot { .. } => {
                let mut geste = Vec::new();
                encoder(&GesteModele::Ajouter(1), &mut geste).map_err(interne)?;
                ecrire(&Retour::Bot(vec![CoupFil::Geste(&geste)]), sortie)
            }
            Appel::PireCas { .. } => {
                let mut e = EtatModele::neuf(0, ReveilFil::Jamais);
                e.total = u64::MAX;
                self.etat = Some(e);
                let mut cas = Vec::new();
                encoder(&0u16, &mut cas).map_err(interne)?;
                ecrire(&Retour::PireCas { cas: &cas }, sortie)
            }
            Appel::Memoire => ecrire(&Retour::Memoire { vivants: 0, tas: 0 }, sortie),
            // Le modèle ne promet aucune propriété : l'épreuve ne le juge pas.
            Appel::Proprietes => ecrire(
                &Retour::Proprietes(jeu_abi::ProprietesFil {
                    coupure_transparente: sans_objet("le modèle n'est pas éprouvé"),
                    permutation: sans_objet("le modèle n'est pas éprouvé"),
                }),
                sortie,
            ),
            Appel::Hypothese { .. } => ecrire(&Retour::Hypothese(Oui::Non), sortie),
        }
    }

    fn creer(
        &mut self,
        pas: u64,
        presents: &[u8],
        garde: Option<&[u8]>,
        sortie: &mut Vec<u8>,
    ) -> Result<(), Echec> {
        let Ok(presents) = decoder::<Vec<Presence>>(presents) else {
            return illisible(Illisible::Forme, sortie);
        };
        let mut etat = EtatModele::neuf(pas, self.reglage.reveil);
        for p in presents {
            if let Presence::Arrivee { appareil, .. } = p {
                etat.presents.push(appareil.fil());
            }
        }
        if let Some(g) = garde {
            match decoder::<u64>(g) {
                Ok(g) => etat.garde = g,
                Err(e) => return ecrire(&Retour::Illisible(e), sortie),
            }
        }
        let reveil = etat.reveil;
        self.etat = Some(etat);
        ecrire(&acceptee(Vec::new(), reveil), sortie)
    }
}

fn repondre_valider(genre: SorteOctets, octets: &[u8], sortie: &mut Vec<u8>) -> Result<(), Echec> {
    match valider(genre, octets) {
        Ok(Some(t)) => {
            let mut cle = Vec::new();
            encoder(&t.cle, &mut cle).map_err(interne)?;
            let mut relache = Vec::new();
            encoder(
                &TenuModele {
                    cle: t.cle,
                    valeur: 0,
                },
                &mut relache,
            )
            .map_err(interne)?;
            let tenu = TenuFil {
                cle: &cle,
                relache: Some(&relache),
                est_relache: if t.valeur == 0 { Oui::Oui } else { Oui::Non },
            };
            ecrire(
                &Retour::Valide(Validation::Valide { tenu: Some(tenu) }),
                sortie,
            )
        }
        Ok(None) => ecrire(&Retour::Valide(Validation::Valide { tenu: None }), sortie),
        Err(e) => ecrire(&Retour::Valide(Validation::Malforme(e)), sortie),
    }
}

fn repondre_vues(
    etat: &EtatModele,
    regards: &[RegardFil],
    sortie: &mut Vec<u8>,
) -> Result<(), Echec> {
    let mut octets: Vec<Vec<u8>> = Vec::new();
    for r in regards {
        octets.push(vue(*r, etat)?);
    }
    let vues: Vec<VueFil<'_>> = regards
        .iter()
        .zip(&octets)
        .map(|(r, o)| VueFil {
            regard: *r,
            octets: o,
        })
        .collect();
    ecrire(&Retour::Vues(vues), sortie)
}

fn repondre_etat(etat: &EtatModele, connues: &[u64], sortie: &mut Vec<u8>) -> Result<(), Echec> {
    let mut phase = Vec::new();
    encoder(&0u8, &mut phase).map_err(interne)?;
    let mut remettre = Vec::new();
    encoder(&ActionModele::Remettre, &mut remettre).map_err(interne)?;
    let nouvelles = etat
        .demandes
        .iter()
        .filter(|id| !connues.contains(id))
        .map(|id| {
            (
                *id,
                DemandeFil::Ligne {
                    flux: Flux::new("modele"),
                    octets: &[],
                },
            )
        })
        .collect();
    let e = EtatFil {
        phase: &phase,
        phase_indice: 0,
        terminale: Oui::Non,
        reveil: etat.reveil,
        attente: AttenteFil {
            echeance: None,
            libre: None,
            joueurs: Vec::new(),
            ecran: vec![remettre.as_slice()],
        },
        accueil: Accueil::Ouverte,
        cas_indice: 0,
        demandes: etat.demandes.clone(),
        nouvelles,
    };
    ecrire(&Retour::Etat(e), sortie)
}

/// Une entrée sur l'état du modèle.
fn entrer(
    etat: &mut EtatModele,
    place: u64,
    e: &EntreeFil<'_>,
    divergent: Oui,
    sortie: &mut Vec<u8>,
) -> Result<(), Echec> {
    let mut sorties = Vec::new();
    match e {
        EntreeFil::Temps { saut, .. } => {
            if etat.panique_au_temps == Oui::Oui {
                return Err(paniquer());
            }
            etat.temps = etat.temps.saturating_add(1);
            etat.sauts = etat.sauts.saturating_add(*saut);
        }
        EntreeFil::Geste { de, octets, quand } => match decoder::<GesteModele>(octets) {
            Ok(g) => {
                if let Some(r) = geste(etat, g, *de, quand.id, divergent, &mut sorties)? {
                    return ecrire(&Retour::Issue(r), sortie);
                }
            }
            Err(e) => return ecrire(&Retour::Illisible(e), sortie),
        },
        EntreeFil::Tenu {
            de, octets, cause, ..
        } => match decoder::<TenuModele>(octets) {
            Ok(t) => {
                etat.tenus.retain(|(a, c, _)| !(*a == *de && *c == t.cle));
                etat.tenus.push((*de, t.cle, t.valeur));
                etat.changements.push((*de, t.cle, t.valeur, *cause));
            }
            Err(e) => return ecrire(&Retour::Illisible(e), sortie),
        },
        EntreeFil::Manette { de, etat: m, .. } => {
            etat.manettes.retain(|(a, _)| *a != *de);
            etat.manettes.push((*de, *m));
        }
        EntreeFil::Ecran {
            quoi: EcranFil::Action(octets),
            ..
        } => match decoder::<ActionModele>(octets) {
            Ok(ActionModele::Remettre) => etat.total = 0,
            Err(e) => return ecrire(&Retour::Illisible(e), sortie),
        },
        EntreeFil::Ecran {
            quoi: EcranFil::Composant(jeu_abi::Composant::Saisie { .. }),
            ..
        } => etat.saisies = etat.saisies.saturating_add(1),
        EntreeFil::Ecran {
            quoi:
                EcranFil::Touche(_)
                | EcranFil::Pointeur(_)
                | EcranFil::Composant(
                    jeu_abi::Composant::Deplace(_) | jeu_abi::Composant::Fichiers { .. },
                ),
            ..
        } => {
            return ecrire(&Retour::Issue(IssueFil::Ignoree), sortie);
        }
        EntreeFil::Presence(p) => presence(etat, p),
        EntreeFil::Reponse { id, .. } => {
            if etat.demandes.contains(id) {
                etat.demandes.retain(|d| d != id);
                etat.reponses.push(*id);
            } else {
                etat.reponses_inattendues = etat.reponses_inattendues.saturating_add(1);
            }
        }
    }
    etat.pas = place;
    etat.acceptees = etat.acceptees.saturating_add(1);
    ecrire(&acceptee(sorties, etat.reveil), sortie)
}

/// Un geste ; rend l'issue s'il n'est pas accepté, ou la faute.
fn geste(
    etat: &mut EtatModele,
    g: GesteModele,
    de: u64,
    id: u64,
    divergent: Oui,
    sorties: &mut Vec<SortieFil>,
) -> Result<Option<IssueFil<'static>>, Echec> {
    match g {
        GesteModele::Ajouter(n) => {
            let plus = if divergent == Oui::Oui {
                u64::from(n).saturating_add(1)
            } else {
                u64::from(n)
            };
            etat.total = etat.total.saturating_add(plus);
            etat.gestes.push(id);
        }
        GesteModele::Refuser => {
            return Ok(Some(IssueFil::Refusee { avis: AVIS_REFUSE }));
        }
        GesteModele::Attendre => return Ok(Some(IssueFil::Attente)),
        GesteModele::Paniquer => return Err(paniquer()),
        GesteModele::Boucler => return Err(Echec::Faute(Faute::Budget)),
        GesteModele::PaniquerAuTemps => etat.panique_au_temps = Oui::Oui,
        GesteModele::Oublier => sorties.push(SortieFil::Oublier(de)),
        GesteModele::Retirer => sorties.push(SortieFil::Retirer(de)),
        GesteModele::NouvellePartie => sorties.push(SortieFil::NouvellePartie {
            graine: GRAINE_NOUVELLE_PARTIE,
        }),
        GesteModele::Demander => {
            etat.demandes.push(etat.prochain_id);
            etat.prochain_id = etat.prochain_id.saturating_add(1);
        }
        GesteModele::Abandonner => etat.demandes.clear(),
        GesteModele::PaniquerEnVue => etat.panique_en_vue = Oui::Oui,
        GesteModele::VueInvalide => etat.vue_invalide = Oui::Oui,
        GesteModele::Dormir => etat.reveil = ReveilFil::Jamais,
        GesteModele::Reveiller => etat.reveil = ReveilFil::ChaquePas,
    }
    Ok(None)
}

/// L'avis `AvisModele::Refuse`, encodé : son numéro de variant, 0.
const AVIS_REFUSE: &[u8] = &[0];

fn presence(etat: &mut EtatModele, p: &Presence) {
    match *p {
        Presence::Arrivee { appareil, .. } | Presence::Retour { appareil, .. } => {
            let n = appareil.fil();
            if !etat.presents.contains(&n) {
                etat.presents.push(n);
            }
        }
        Presence::Depart { appareil, .. } => etat.presents.retain(|a| *a != appareil.fil()),
        Presence::Capacites { .. } => {}
    }
}

/// Ce que le modèle dit d'octets d'appareil : un tenu bien formé rend sa valeur.
fn valider(sorte: SorteOctets, octets: &[u8]) -> Result<Option<TenuModele>, ErreurFil> {
    match sorte {
        SorteOctets::GesteTelephone => decoder::<GesteModele>(octets).map(|_| None),
        SorteOctets::ActionEcran => decoder::<ActionModele>(octets).map(|_| None),
        SorteOctets::TenuTelephone => decoder::<TenuModele>(octets).map(Some),
    }
}

/// La scène de l'écran du modèle : un bouton « Remettre » et un champ.
pub fn scene() -> SceneFil {
    let mut remettre = Vec::new();
    // L'encodage d'une enum sans champ ne peut pas échouer ; vide, l'action ne
    // serait admise par aucune entrée.
    let _: Result<(), jeu_abi::ErreurEncodage> = encoder(&ActionModele::Remettre, &mut remettre);
    let zero = Rect {
        x: Fixe::unites(0),
        y: Fixe::unites(0),
        l: Fixe::unites(1),
        h: Fixe::unites(1),
    };
    let mut s = SceneFil::vide();
    s.noeuds.push(NoeudFil::simple(
        CLE_BOUTON,
        None,
        SorteNoeud::Bouton {
            boite: zero,
            nom: String::from("Remettre"),
            action: remettre,
        },
    ));
    s.noeuds.push(NoeudFil::simple(
        CLE_CHAMP,
        None,
        SorteNoeud::Composant(ComposantVuFil::Champ {
            limite: LIMITE_CHAMP,
            unite: Unite::PointsDeCode,
            lignes: Lignes::Une,
            valeur: String::new(),
            exemple: String::new(),
            fin: None,
        }),
    ));
    s
}

/// La clé du texte du total, dans la scène de l'écran.
pub const CLE_TOTAL: Cle = Cle::du_fil(3);
/// La clé du sous-arbre définitif de la scène de l'écran (§ 4.4).
pub const CLE_DEFINITIF: Cle = Cle::du_fil(4);
/// La clé du total sous le sous-arbre définitif.
pub const CLE_TOTAL_DEFINITIF: Cle = Cle::du_fil(5);

/// Le texte d'un total, dans une scène.
pub fn texte_du_total(total: u64) -> String {
    alloc::format!("{total}")
}

/// L'identité de l'effet `Provisoire` d'un total : un son de tape.
pub fn effet_provisoire(total: u64) -> jeu_abi::IdEffet {
    jeu_abi::IdEffet::de(&("tape", total))
}

/// L'identité de l'effet `Definitive` d'un total : un son de résultat.
pub fn effet_definitif(total: u64) -> jeu_abi::IdEffet {
    jeu_abi::IdEffet::de(&("resultat", total))
}

/// La scène de l'écran pour cet état : celle de [`scene`], plus le total, le même
/// total sous un sous-arbre définitif, et deux effets nommés par lui, l'un
/// `Provisoire`, l'autre `Definitive` (§ 4.4).
pub fn scene_de(etat: &EtatModele) -> SceneFil {
    let mut s = scene();
    let style = jeu_abi::StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: jeu_abi::Taille::Fixe(Fixe::unites(1)),
        couleur: jeu_abi::Rgba(255, 255, 255, 255),
        alignement: jeu_abi::Alignement::Centre,
        lignes: Lignes::Une,
        coupe: jeu_abi::Coupe::Graphemes,
    };
    let boite = Rect {
        x: Fixe::unites(2),
        y: Fixe::unites(2),
        l: Fixe::unites(4),
        h: Fixe::unites(1),
    };
    let texte = |cle, parent| {
        NoeudFil::simple(
            cle,
            parent,
            SorteNoeud::Texte {
                texte: texte_du_total(etat.total),
                boite,
                style,
            },
        )
    };
    s.noeuds.push(texte(CLE_TOTAL, None));
    let definitif = u32::try_from(s.noeuds.len()).ok();
    s.noeuds
        .push(NoeudFil::simple(CLE_DEFINITIF, None, SorteNoeud::Definitif));
    s.noeuds.push(texte(CLE_TOTAL_DEFINITIF, definitif));
    for (id, tenue) in [
        (effet_provisoire(etat.total), jeu_abi::Tenue::Provisoire),
        (effet_definitif(etat.total), jeu_abi::Tenue::Definitive),
    ] {
        s.effets.push(jeu_abi::Effet {
            id,
            quand: jeu_abi::Instant::de(jeu_abi::Pas::du_fil(etat.pas)),
            quoi: jeu_abi::Sensation::Son(jeu_abi::Son::Signal(jeu_abi::Signal::Avance)),
            tenue,
            peremption: Duree::ms(250),
        });
    }
    s
}

/// La vue d'un regard : la scène pour l'écran, le total pour un téléphone.
fn vue(r: RegardFil, etat: &EtatModele) -> Result<Vec<u8>, Echec> {
    let mut octets = Vec::new();
    match r {
        RegardFil::Ecran if etat.vue_invalide == Oui::Oui => {
            octets.extend_from_slice(&[0xff, 0xff]);
        }
        RegardFil::Ecran => encoder(&scene_de(etat), &mut octets).map_err(interne)?,
        RegardFil::Appareil(_) => {
            let mut v = Vec::new();
            encoder(&etat.total, &mut v).map_err(interne)?;
            encoder(
                &MontreFil {
                    vue: &v,
                    effets: Vec::new(),
                    reglages: Reglages::ORDINAIRES,
                },
                &mut octets,
            )
            .map_err(interne)?;
        }
    }
    Ok(octets)
}

fn acceptee(sorties: Vec<SortieFil>, reveil: ReveilFil) -> Retour<'static> {
    Retour::Issue(IssueFil::Acceptee { sorties, reveil })
}

fn paniquer() -> Echec {
    Echec::Faute(Faute::Panique {
        message: MESSAGE_DE_PANIQUE.to_owned(),
    })
}

fn ecrire(r: &Retour<'_>, sortie: &mut Vec<u8>) -> Result<(), Echec> {
    encoder(r, sortie).map_err(interne)
}

/// Remplace la sortie par `Retour::Illisible`, comme un module qui ne lit pas l'appel.
fn illisible(quoi: Illisible, sortie: &mut Vec<u8>) -> Result<(), Echec> {
    sortie.clear();
    ecrire(&Retour::Illisible(ErreurFil { ou: 0, quoi }), sortie)
}

fn interne(e: jeu_abi::ErreurEncodage) -> Echec {
    Echec::Panne(PanneOutil::new(CausePanne::Interne, alloc::format!("{e}")))
}

fn bonjour() -> Bonjour {
    Bonjour {
        abi: ABI,
        formes: FORMES_MODELE,
        boite: (0, 0),
        brouillon: (0, 0),
        panique: (0, 0),
        version_jeu: (0, 0, 0),
    }
}

/// La déclaration du modèle au réglage ordinaire.
pub fn declaration() -> DeclarationFil {
    declaration_de(&ReglageModele::ordinaire())
}

/// La déclaration du modèle : un compteur, sans secret ni hasard.
pub fn declaration_de(r: &ReglageModele) -> DeclarationFil {
    DeclarationFil {
        nom: TexteCourt::new("modele"),
        places: Places::joueurs(1, 8).appareils(16),
        rattrapage: r.rattrapage,
        fenetre: r.fenetre.clone(),
        antidatage_max: r.antidatage_max,
        pause: r.pause.clone(),
        ecran: FormeEcran::Scene {
            largeur: 16,
            hauteur: 9,
            zone_sure_pour_cent: 90,
            echelle_entiere: Oui::Non,
        },
        telephones: ApplicableFil::Oui(TelephonesFil {
            repartition: Repartition::UnParJoueur,
            tenu: r.tenu.clone(),
        }),
        manettes: r.manettes.clone(),
        prediction: sans_objet("le modèle ne prédit rien"),
        hasard: sans_objet("le modèle ne tire rien"),
        secrets: sans_objet("le modèle n'a aucun secret"),
        fin: sans_objet("le compteur ne finit jamais"),
        capacites: Voulues {
            vibration: Oui::Non,
            mouvement: Oui::Non,
            son: Oui::Non,
            ecran_allume: Oui::Non,
            micro: Oui::Non,
            pad_natif: Oui::Non,
        },
        limites: r.limites,
        diagnostic: CodeTouche::new("KeyD"),
        assets: Vec::new(),
        mecaniques: vec![Mecanique::TourParTour],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jeu_abi::QuandFil;

    fn appeler(i: &mut InstanceModele, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
        let mut octets = Vec::new();
        encoder(a, &mut octets).unwrap();
        i.appeler(a.sorte(), &octets, 1_000)
    }

    fn geste(i: &mut InstanceModele, g: GesteModele) -> Result<Vec<u8>, Echec> {
        let mut g_octets = Vec::new();
        encoder(&g, &mut g_octets).unwrap();
        let quand = QuandFil {
            fait: 1,
            fraction: 0,
            recu: 1,
            vu: 0,
            id: 1,
        };
        let mut e = Vec::new();
        encoder(
            &EntreeFil::Geste {
                de: 9,
                octets: &g_octets,
                quand,
            },
            &mut e,
        )
        .unwrap();
        appeler(
            i,
            &Appel::Entree {
                ordre: 0,
                place: 1,
                entree: &e,
            },
        )
    }

    fn cree() -> InstanceModele {
        let mut i = InstanceModele::new();
        let mut presents = Vec::new();
        encoder(&Vec::<Presence>::new(), &mut presents).unwrap();
        let _: Vec<u8> = appeler(
            &mut i,
            &Appel::Creer {
                graine: [0; 32],
                pas: 0,
                heure: 0,
                salle: TexteCourt::new("TEST"),
                presents: &presents,
                garde: None,
            },
        )
        .unwrap();
        i
    }

    #[test]
    fn le_modele_rend_chaque_issue() {
        let mut i = cree();
        let r = geste(&mut i, GesteModele::Ajouter(3)).unwrap();
        assert!(matches!(
            decoder::<Retour<'_>>(&r).unwrap(),
            Retour::Issue(IssueFil::Acceptee { .. })
        ));
        assert_eq!(i.etat().unwrap().total, 3);
        assert_eq!(i.etat().unwrap().gestes, vec![1]);
        let r = geste(&mut i, GesteModele::Refuser).unwrap();
        assert!(matches!(
            decoder::<Retour<'_>>(&r).unwrap(),
            Retour::Issue(IssueFil::Refusee { .. })
        ));
        assert_eq!(
            geste(&mut i, GesteModele::Paniquer),
            Err(Echec::Faute(Faute::Panique {
                message: MESSAGE_DE_PANIQUE.to_owned()
            }))
        );
        assert_eq!(
            geste(&mut i, GesteModele::Boucler),
            Err(Echec::Faute(Faute::Budget))
        );
    }

    #[test]
    fn l_avis_du_refus_est_celui_du_modele() {
        let mut a = Vec::new();
        encoder(&AvisModele::Refuse, &mut a).unwrap();
        assert_eq!(a, AVIS_REFUSE);
    }

    #[test]
    fn instantane_puis_restaurer_rend_la_meme_empreinte() {
        let mut i = cree();
        let avant = i.empreinte().unwrap();
        let s = i.instantane().unwrap();
        let _: Vec<u8> = geste(&mut i, GesteModele::Ajouter(1)).unwrap();
        assert_ne!(i.empreinte().unwrap(), avant);
        i.restaurer(&s).unwrap();
        assert_eq!(i.empreinte().unwrap(), avant);
    }

    #[test]
    fn une_sorte_fausse_est_illisible() {
        let mut i = InstanceModele::new();
        let mut octets = Vec::new();
        encoder(&Appel::Bonjour, &mut octets).unwrap();
        let r = i.appeler(SorteAppel::Ecriture, &octets, 1).unwrap();
        assert!(matches!(
            decoder::<Retour<'_>>(&r).unwrap(),
            Retour::Illisible(ErreurFil {
                quoi: Illisible::Ordre,
                ..
            })
        ));
    }

    #[test]
    fn la_declaration_du_modele_fait_l_aller_retour() {
        let d = declaration();
        let mut octets = Vec::new();
        encoder(&d, &mut octets).unwrap();
        assert_eq!(decoder::<DeclarationFil>(&octets).unwrap(), d);
    }

    #[test]
    fn la_scene_du_modele_se_relit() {
        let mut octets = Vec::new();
        encoder(&scene(), &mut octets).unwrap();
        assert_eq!(decoder::<SceneFil>(&octets).unwrap(), scene());
    }
}
