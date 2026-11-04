//! `charger` : ce qui est refusé avant toute partie, et ce qui est vérifié (§ 8.7).

use jeu_abi::{ABI, ApplicableFil};
use moteur_wasmtime::{Moteur, RefusModule};
use partie::Faute;
use reecriture::Refus;

use crate::commun::{BROUILLON, COMPTER, Colle, PILE, REGLAGES};

fn refus(c: &Colle) -> RefusModule {
    match Moteur::new(REGLAGES).unwrap().charger(&c.octets()) {
        Ok(_) => panic!("le module devait être refusé"),
        Err(e) => e,
    }
}

#[test]
fn module_conforme_charge() {
    let m = Colle::new(COMPTER).charger();
    assert_eq!(m.bonjour().abi, ABI);
    assert_eq!(m.bonjour().brouillon, BROUILLON);
    assert_eq!(m.declaration().limites.memoire, 16);
    assert_eq!(m.memoire_max(), 16);
    assert_eq!(m.constat().imports, Vec::<(String, String)>::new());
    assert_eq!(m.reecrit().pile_initiale, PILE);
    assert_eq!(m.empreinte(), m.reecrit().empreinte_originale);
}

/// Un import caché en tête d'un module qui a tout le reste : refusé, et nommé.
#[test]
fn module_avec_import_refuse() {
    let texte = Colle::new(COMPTER).wat().replacen(
        "(module\n",
        "(module\n  (import \"env\" \"maintenant\" (func (result i64)))\n",
        1,
    );
    let r = Moteur::new(REGLAGES)
        .unwrap()
        .charger(&wat::parse_str(&texte).unwrap());
    assert_eq!(
        r.err(),
        Some(RefusModule::Reecriture(Refus::Import {
            module: String::from("env"),
            nom: String::from("maintenant"),
        }))
    );
}

#[test]
fn export_manquant_refuse() {
    let texte = Colle::new(COMPTER)
        .wat()
        .replace(r#"(export "soiree_boite")"#, "");
    let r = Moteur::new(REGLAGES)
        .unwrap()
        .charger(&wat::parse_str(&texte).unwrap());
    assert_eq!(
        r.err(),
        Some(RefusModule::Reecriture(Refus::ExportManquant {
            nom: "soiree_boite"
        }))
    );
}

#[test]
fn abi_inconnue_refusee() {
    let mut c = Colle::new(COMPTER);
    c.bonjour.abi = 99;
    assert_eq!(
        refus(&c),
        RefusModule::Abi {
            module: 99,
            moteur: (ABI, ABI)
        }
    );
}

/// Une raison creuse, un brouillon qui ne correspond pas, une prédiction sans
/// téléphone, une mémoire déclarée plus petite que le module : refusés au chargement
/// (HIS-04, OUT-03).
#[test]
fn declaration_incoherente_refusee() {
    let mut cas = Vec::new();
    let mut c = Colle::new(COMPTER);
    c.declaration.hasard = ApplicableFil::SansObjet(String::from("n/a"));
    cas.push(c);
    let mut c = Colle::new(COMPTER);
    c.declaration.limites.brouillon = 1024;
    cas.push(c);
    let mut c = Colle::new(COMPTER);
    c.declaration.prediction = ApplicableFil::Oui(());
    c.declaration.telephones = ApplicableFil::SansObjet(String::from("aucun téléphone ici"));
    cas.push(c);
    let mut c = Colle::new(COMPTER);
    c.declaration.limites.memoire = 2;
    cas.push(c);
    for c in &cas {
        assert!(
            matches!(refus(c), RefusModule::Declaration(_)),
            "{:?}",
            refus(c)
        );
    }
}

#[test]
fn zones_qui_chevauchent_la_pile_ou_entre_elles_refusees() {
    let mut c = Colle::new(COMPTER);
    c.bonjour.boite = (1024, 4096);
    assert!(matches!(refus(&c), RefusModule::Declaration(_)));
    let mut c = Colle::new(COMPTER);
    c.bonjour.panique = (BROUILLON.0, 4096);
    assert!(matches!(refus(&c), RefusModule::Declaration(_)));
    let mut c = Colle::new(COMPTER);
    c.bonjour.panique = (135_168, 8);
    assert!(matches!(refus(&c), RefusModule::Declaration(_)));
    let mut c = Colle::new(COMPTER);
    c.bonjour.brouillon = (69_632, 10 << 20);
    c.declaration.limites.brouillon = 10 << 20;
    assert!(matches!(refus(&c), RefusModule::Declaration(_)));
}

/// `Bonjour` qui boucle ou qui panique : le module est refusé, avec la faute.
#[test]
fn bonjour_qui_boucle_refuse_avec_sa_faute() {
    let texte = Colle::new(COMPTER).wat().replace(
        "(func (export \"soiree_appel\") (param $n i32) (result i64)",
        "(func (export \"soiree_appel\") (param $n i32) (result i64) (loop $l (br $l))",
    );
    let r = Moteur::new(REGLAGES)
        .unwrap()
        .charger(&wat::parse_str(&texte).unwrap());
    assert_eq!(
        r.err(),
        Some(RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::Budget
        })
    );
}

/// `limites.memoire` au-delà du plafond du moteur : le plafond gagne.
#[test]
fn memoire_max_borne_par_le_plafond() {
    let mut c = Colle::new(COMPTER);
    c.memoire = (4, 1000);
    c.declaration.limites.memoire = 500;
    let m = c.charger();
    assert_eq!(m.memoire_max(), REGLAGES.reecriture.memoire_plafond_pages);
}
