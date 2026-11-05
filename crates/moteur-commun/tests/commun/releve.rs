//! Le relevé de E20 (lot 3b) : la reprise des journaux depuis la création, sur le
//! modèle de `partie`, est la même d'une fois à l'autre ; un moteur qui diffère d'un
//! appel se voit à cet appel, et le rejeu s'arrête là ; les formes se relisent
//! strictement.

use jeu_abi::{Appareil, Duree, IdGeste, Instant, Pas, SorteAppel, TexteCourt, encoder};
use moteur_commun::releve::{
    self, Chargement, Ecart, Enregistreuse, Evenement, Issue, REJOUEE, Releve, SoireeJouee,
};
use partie::modele::{Compteurs, GesteModele, InstanceModele, ReglageModele};
use partie::{Arrivee, Budgets, Echec, Fabrique, Faute, Instance, Ouverture, PanneOutil, Partie};
use std::rc::Rc;

/// Le modèle de `partie`, en fabrique qu'on clone.
#[derive(Clone)]
struct Modele(ReglageModele);

impl Fabrique for Modele {
    type I = InstanceModele;

    fn neuve(&mut self) -> Result<InstanceModele, PanneOutil> {
        Ok(InstanceModele::avec(
            self.0.clone(),
            Rc::new(Compteurs::default()),
        ))
    }
}

fn ouverture() -> Ouverture {
    Ouverture {
        graine: [3; 32],
        pas: 0,
        heure: 1_000_000,
        salle: TexteCourt::new("SALLE"),
        budgets: Budgets {
            entree: 1_000,
            lecture: 1_000,
            creation: 1_000,
            canonique: 1_000,
        },
        point_tous_les: 50,
    }
}

fn geste(de: u64, g: GesteModele, fait: u64, id: u64) -> Arrivee {
    let mut octets = Vec::new();
    encoder(&g, &mut octets).unwrap();
    Arrivee::Geste {
        de: Appareil::du_fil(de),
        octets,
        fait: Instant::de(Pas::du_fil(fait)),
        vu: Pas::du_fil(fait),
        id: IdGeste::du_fil(id),
        demi_aller_retour: Duree::ZERO,
    }
}

/// Une soirée du modèle : des gestes acceptés, un refus, une panique, sur 120 pas.
fn soiree() -> SoireeJouee {
    let mut p = Partie::ouvrir(Modele(ReglageModele::ordinaire()), ouverture()).unwrap();
    let gestes = [
        (5, GesteModele::Ajouter(2)),
        (20, GesteModele::Refuser),
        (40, GesteModele::Ajouter(7)),
        (60, GesteModele::Paniquer),
        (80, GesteModele::Ajouter(1)),
    ];
    let mut id = 1;
    for s in 1..=120u64 {
        for (quand, g) in gestes {
            if quand == s {
                let _: partie::Placement = p
                    .arriver(geste(1, g, s, id), Instant::de(Pas::du_fil(s)))
                    .unwrap();
                id += 1;
            }
        }
        let _: partie::Avancee = p.avancer(Pas::du_fil(s)).unwrap();
    }
    SoireeJouee {
        nom: String::from("soirée du modèle"),
        ouverture: ouverture(),
        journal: p.journal().clone(),
    }
}

fn relever(f: Modele, soirees: &[SoireeJouee]) -> Vec<Evenement> {
    let e = Enregistreuse::new(f);
    releve::rejouer(&e, soirees);
    e.evenements()
}

#[test]
fn la_reprise_depuis_la_creation_se_releve_a_l_identique() {
    let s = soiree();
    let a = relever(Modele(ReglageModele::ordinaire()), &[s.clone(), s.clone()]);
    let b = relever(Modele(ReglageModele::ordinaire()), &[s.clone(), s]);
    assert_eq!(a, b);
    assert_eq!(releve::comparer(&a, &b), None);
    // Chaque soirée est reprise jusqu'au bout, entrées, faute, empreinte comprises.
    let fins: Vec<&Evenement> = a
        .iter()
        .filter(|e| matches!(e, Evenement::Fin(_)))
        .collect();
    assert_eq!(fins, [&Evenement::Fin(String::from(REJOUEE)); 2]);
    let (appels, fautes) = releve::compter(&a);
    assert!(appels > 240, "{appels} appels");
    assert!(
        fautes
            .iter()
            .any(|(sorte, n)| sorte == "Panique" && *n >= 2),
        "{fautes:?}"
    );
    assert!(a.iter().any(|e| matches!(
        e,
        Evenement::Appel { nom, entree: Some(_), .. } if nom == "Entree"
    )));
    assert!(
        a.iter()
            .any(|e| matches!(e, Evenement::Empreinte(_, Ok(_))))
    );
    assert!(a.iter().any(|e| matches!(e, Evenement::Restaurer(_, None))));
    assert!(a.iter().any(|e| matches!(
        e,
        Evenement::Appel { nom, .. } if nom == "Canonique"
    )));
}

/// Un moteur qui classe un carburant épuisé en piège, et non en `Budget` : ce que E20
/// doit voir dans un navigateur qui lirait mal la globale du carburant.
#[derive(Clone)]
struct BudgetEnPiege(Modele);

struct InstanceBudgetEnPiege(InstanceModele);

impl Instance for InstanceBudgetEnPiege {
    type Instantane = <InstanceModele as Instance>::Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        match self.0.appeler(sorte, appel, budget) {
            Err(Echec::Faute(Faute::Budget)) => Err(Echec::Faute(Faute::Piege {
                code: String::from("navigateur : unreachable"),
            })),
            r => r,
        }
    }

    fn carburant(&self) -> Option<u64> {
        self.0.carburant()
    }

    fn instantane(&mut self) -> Result<Self::Instantane, PanneOutil> {
        self.0.instantane()
    }

    fn restaurer(&mut self, i: &Self::Instantane) -> Result<(), PanneOutil> {
        self.0.restaurer(i)
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        self.0.empreinte()
    }
}

impl Fabrique for BudgetEnPiege {
    type I = InstanceBudgetEnPiege;

    fn neuve(&mut self) -> Result<InstanceBudgetEnPiege, PanneOutil> {
        self.0.neuve().map(InstanceBudgetEnPiege)
    }
}

/// Les journaux des modules d'aujourd'hui ne font refaire que des `Panique` : avant les
/// soirées, le relevé provoque des fautes `Budget` (`Decrire` à la moitié de son
/// carburant, puis à une unité près), pour que cette sorte aussi se compare d'un moteur
/// à l'autre (lot 3b, après la vérification). Un moteur qui la classe autrement se voit
/// à la première, avant toute soirée.
#[test]
fn les_fautes_budget_se_provoquent_avant_les_soirees() {
    let s = soiree();
    let a = relever(
        Modele(ReglageModele::ordinaire()),
        core::slice::from_ref(&s),
    );
    let premiere = a
        .iter()
        .position(|e| matches!(e, Evenement::Soiree(_)))
        .unwrap();
    // Le modèle brûle une unité par appel : `c / 2` et `c - 1` valent 0, une seule faute.
    assert_eq!(releve::budgets_provoques(&a), 1, "{:?}", a.get(..premiere));
    assert!(a.get(..premiere).unwrap().iter().any(|e| matches!(
        e,
        Evenement::Appel { nom, budget: 0, issue: Issue::Faute(Faute::Budget), .. } if nom == "Decrire"
    )));
    let (_, fautes) = releve::compter(&a);
    assert!(
        fautes.iter().any(|(sorte, _)| sorte == "Budget"),
        "{fautes:?}"
    );
    // Sans soirée, rien n'est provoqué.
    assert_eq!(
        relever(Modele(ReglageModele::ordinaire()), &[]),
        Vec::<Evenement>::new()
    );
    // Un moteur qui classe le carburant épuisé en piège diverge là, avant la soirée 0.
    let e = Enregistreuse::comparant(BudgetEnPiege(Modele(ReglageModele::ordinaire())), a.clone());
    releve::rejouer(&e, &[s]);
    let ecart = e.ecart().expect("un écart");
    assert!(ecart.indice < premiere, "{ecart}");
    assert_eq!(ecart.soiree, None);
    assert!(ecart.to_string().contains("une autre issue"), "{ecart}");
    assert!(ecart.to_string().contains("faute"), "{ecart}");
}

/// Un autre moteur qui rend autre chose (le modèle divergent : `Ajouter(n)` ajoute
/// `n + 1`) se voit au premier appel qui diffère, et le rejeu s'arrête là.
#[test]
fn un_moteur_qui_differe_se_voit_a_son_premier_appel() {
    let s = soiree();
    let attendu = relever(
        Modele(ReglageModele::ordinaire()),
        core::slice::from_ref(&s),
    );
    let mut divergent = ReglageModele::ordinaire();
    divergent.divergent = jeu_abi::Oui::Oui;
    let e = Enregistreuse::comparant(Modele(divergent), attendu.clone());
    releve::rejouer(&e, &[s]);
    let ecart = e.ecart().expect("un écart");
    let obtenu = e.evenements();
    assert_eq!(
        obtenu.len(),
        ecart.indice + 1,
        "le rejeu s'arrête à l'écart"
    );
    assert!(obtenu.len() < attendu.len());
    assert_eq!(
        releve::comparer(&attendu, &obtenu).map(|x| x.indice),
        Some(ecart.indice)
    );
    assert_eq!(ecart.soiree, Some(0));
    let texte = ecart.to_string();
    // Les entrées rendent les mêmes octets : le total diffère dans l'état, et la
    // première lecture de l'état (la forme canonique de la fin) le montre.
    assert!(texte.contains("une autre issue"), "{texte}");
    assert!(texte.contains("après l'entrée d'ordre"), "{texte}");
    assert!(texte.contains("Canonique"), "{texte}");
}

/// Un carburant faussé d'une unité suffit : c'est ce que fait le moteur fautif de E20.
#[test]
fn un_carburant_fausse_d_une_unite_se_voit() {
    let s = soiree();
    let attendu = relever(
        Modele(ReglageModele::ordinaire()),
        core::slice::from_ref(&s),
    );
    let mut fausse = attendu.clone();
    let k = fausse
        .iter()
        .position(|e| matches!(e, Evenement::Appel { nom, .. } if nom == "Entree"))
        .unwrap();
    if let Some(Evenement::Appel { carburant, .. }) = fausse.get_mut(k) {
        *carburant = carburant.map(|c| c + 1);
    }
    let e = Enregistreuse::comparant(Modele(ReglageModele::ordinaire()), fausse);
    releve::rejouer(&e, &[s]);
    let ecart: Ecart = e.ecart().unwrap();
    assert_eq!(ecart.indice, k);
    assert!(
        ecart.to_string().contains("un carburant de 1 au lieu de 2"),
        "{ecart}"
    );
}

#[test]
fn un_releve_plus_court_ou_plus_long_est_un_ecart() {
    let s = soiree();
    let attendu = relever(
        Modele(ReglageModele::ordinaire()),
        core::slice::from_ref(&s),
    );
    let court = attendu.get(..attendu.len() - 3).unwrap().to_vec();
    let x = releve::comparer(&attendu, &court).unwrap();
    assert_eq!((x.indice, x.obtenu), (attendu.len() - 3, None));
    let x = releve::comparer(&court, &attendu).unwrap();
    assert!(x.attendu.is_none() && x.obtenu.is_some());
    // En chemin : l'attendu a trois événements de plus que ce que le rejeu produit.
    let mut long = attendu.clone();
    long.extend(attendu.iter().take(3).cloned());
    let e = Enregistreuse::comparant(Modele(ReglageModele::ordinaire()), long);
    releve::rejouer(&e, &[s]);
    let x = e.ecart().unwrap();
    assert_eq!((x.indice, x.obtenu), (attendu.len(), None));
}

#[test]
fn les_formes_se_relisent_strictement() {
    let s = soiree();
    let soirees = vec![s.clone(), s];
    let o = releve::encoder_soirees(&soirees);
    assert_eq!(releve::decoder_soirees(&o).unwrap(), soirees);
    let mut evenements = relever(Modele(ReglageModele::ordinaire()), &soirees);
    // Chaque sorte d'événement, et chaque issue.
    evenements.extend([
        Evenement::Instantane(3, Some(String::from("Interne"))),
        Evenement::Restaurer(3, Some(String::from("CroissanceRefusee"))),
        Evenement::Empreinte(3, Err(String::from("Interne"))),
        Evenement::Appel {
            instance: 9,
            nom: String::from("?"),
            budget: 7,
            appel: [1; 8],
            entree: None,
            carburant: None,
            issue: Issue::Panne(String::from("PileNonRendue")),
        },
        Evenement::Appel {
            instance: 9,
            nom: String::from("Entree"),
            budget: 7,
            appel: [2; 8],
            entree: Some((4, 5)),
            carburant: Some(u64::MAX),
            issue: Issue::Faute(Faute::Piege {
                code: String::from("navigateur : unreachable"),
            }),
        },
        Evenement::Fin(String::from("reprise : point hors du journal")),
    ]);
    for chargement in [
        Chargement::Accepte {
            empreinte: [9; 32],
            carburant: (3104, 7304),
        },
        Chargement::Refuse(String::from("Bonjour : Budget")),
    ] {
        let r = Releve {
            chargement,
            evenements: evenements.clone(),
        };
        let o = releve::encoder_releve(&r);
        assert_eq!(releve::decoder_releve(&o).unwrap(), r);
        let mut trop = o.clone();
        trop.push(0);
        assert!(releve::decoder_releve(&trop).is_err());
        for n in [0, 7, 8, o.len() / 2, o.len() - 1] {
            assert!(
                releve::decoder_releve(o.get(..n).unwrap()).is_err(),
                "coupé à {n}"
            );
        }
    }
    assert!(releve::decoder_soirees(o.get(..o.len() - 1).unwrap()).is_err());
    let mut trop = o;
    trop.push(1);
    assert!(releve::decoder_soirees(&trop).is_err());
}
