//! Ce que les tests du cœur partagent : ouvrir une partie sur le modèle, fabriquer
//! des arrivées, lire le journal.

use jeu_abi::{
    Appareil, Duree, EntreeFil, IdGeste, Instant, Pas, QuandFil, TexteCourt, decoder, encoder,
};
use partie::modele::{EtatModele, FabriqueModele, GesteModele, ReglageModele, TenuModele};
use partie::{Arrivee, Avancee, Budgets, IssueJournal, Journal, LigneEntree, Ouverture, Partie};

pub type PartieModele = Partie<FabriqueModele>;

pub fn ouverture() -> Ouverture {
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
        point_tous_les: 600,
    }
}

pub fn ouvrir(r: ReglageModele) -> PartieModele {
    Partie::ouvrir(FabriqueModele::new(r), ouverture()).unwrap()
}

pub fn a(n: u64) -> Appareil {
    Appareil::du_fil(n)
}

pub fn instant(pas: u64) -> Instant {
    Instant::de(Pas::du_fil(pas))
}

pub fn pas(n: u64) -> Pas {
    Pas::du_fil(n)
}

pub fn octets<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    o
}

/// Un geste fait à `fait`, sur une vue de ce pas, sans latence.
pub fn geste(de: u64, g: GesteModele, fait: u64, id: u64) -> Arrivee {
    Arrivee::Geste {
        de: a(de),
        octets: octets(&g),
        fait: instant(fait),
        vu: pas(fait),
        id: IdGeste::du_fil(id),
        demi_aller_retour: Duree::ZERO,
    }
}

/// Un tenu du modèle.
pub fn tenu(de: u64, cle: u8, valeur: u8, numero: u64, fait: u64) -> Arrivee {
    Arrivee::Tenu {
        de: a(de),
        octets: octets(&TenuModele { cle, valeur }),
        numero,
        fait: instant(fait),
        demi_aller_retour: Duree::ZERO,
    }
}

/// L'état du jeu modèle de la définitive.
pub fn etat(p: &PartieModele) -> &EtatModele {
    p.definitive().etat().unwrap()
}

/// Avance pas à pas jusqu'à `jusqua`, et rend chaque avancée.
pub fn avancer(p: &mut PartieModele, jusqua: u64) -> Vec<Avancee> {
    let mut v = Vec::new();
    while p.definitif().numero() < jusqua {
        let s = p.definitif().numero() + 1;
        v.push(p.avancer(pas(s)).unwrap());
    }
    v
}

/// Arrive au pas suivant, puis avance d'un pas.
pub fn jouer(p: &mut PartieModele, arrivee: Arrivee) -> Avancee {
    let s = p.definitif().numero() + 1;
    let _: partie::Placement = p.arriver(arrivee, instant(s)).unwrap();
    p.avancer(pas(s)).unwrap()
}

/// Les entrées du journal.
pub fn entrees(j: &Journal) -> Vec<LigneEntree> {
    j.entrees().cloned().collect()
}

/// Le `Quand` du geste de cette identité, tel que la définitive l'a reçu.
pub fn quand_du_geste(j: &Journal, id: u64) -> QuandFil {
    for e in j.entrees() {
        if let Ok(EntreeFil::Geste { quand, .. }) = decoder::<EntreeFil<'_>>(&e.octets)
            && quand.id == id
        {
            return quand;
        }
    }
    panic!("aucun geste d'identité {id} au journal");
}

/// Les entrées écartées.
pub fn ecartees(j: &Journal) -> usize {
    j.entrees()
        .filter(|e| matches!(e.issue, IssueJournal::Ecartee { .. }))
        .count()
}

/// Les `Temps` du journal.
pub fn temps(j: &Journal) -> Vec<(Pas, u64)> {
    j.entrees()
        .filter_map(|e| match decoder::<EntreeFil<'_>>(&e.octets) {
            Ok(EntreeFil::Temps { saut, .. }) => Some((e.place, saut)),
            _ => None,
        })
        .collect()
}
