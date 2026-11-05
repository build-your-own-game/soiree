//! Les fautes et les pannes (§ 4.6) : chaque module hostile est arrêté proprement,
//! classé, et au même endroit à chaque exécution.

use std::fmt::Write as _;

use moteur_wasmtime::{InstanceWasm, Moteur, ReglagesMoteur};
use partie::{CausePanne, Echec, Fabrique, Faute, Instance, Responsable};

use crate::commun::{COMPTER, Colle, ECRITURE, LECTURE, REGLAGES, entree, lecture};

fn instance(c: &Colle) -> InstanceWasm {
    c.charger().fabrique().neuve().unwrap()
}

/// L'issue d'une entrée, et le carburant consommé.
fn jouer(c: &Colle, budget: u64) -> (Result<Vec<u8>, Echec>, Option<u64>) {
    let mut i = instance(c);
    let r = i.appeler(ECRITURE, &entree(1), budget);
    (r, i.carburant())
}

/// Deux moteurs, deux chargements, deux instances : la même issue, le même
/// carburant (SQ-59, HIS-07).
fn meme_endroit(c: &Colle, budget: u64) -> Echec {
    let (a, ca) = jouer(c, budget);
    let (b, cb) = jouer(c, budget);
    assert_eq!(a, b);
    assert_eq!(ca, cb);
    a.expect_err("l'entrée devait échouer")
}

const BOUCLE: &str = "(loop $l (br $l)) (unreachable)";

#[test]
fn boucle_infinie_faute_budget() {
    let c = Colle::new(BOUCLE);
    assert_eq!(meme_endroit(&c, 100_000), Echec::Faute(Faute::Budget));
    let (_, carburant) = jouer(&c, 100_000);
    assert!(carburant.unwrap() >= 100_000);
}

/// Un carburant très grand sur une boucle infinie : seul le garde-fou l'arrête, et
/// c'est une panne de l'outil (HIS-06, OUT-11).
#[test]
fn garde_fou_mural_panne_de_l_outil() {
    let moteur = Moteur::new(ReglagesMoteur {
        garde_fou_murale_ms: 100,
        ..REGLAGES
    })
    .unwrap();
    let mut i = moteur
        .charger(&Colle::new(BOUCLE).octets())
        .unwrap()
        .fabrique()
        .neuve()
        .unwrap();
    let Err(Echec::Panne(p)) = i.appeler(ECRITURE, &entree(1), u64::MAX) else {
        panic!("le garde-fou devait arrêter l'appel");
    };
    assert_eq!(p.cause, CausePanne::GardeFouMural);
    assert_eq!(p.responsable(), Responsable::Outil);
}

const RECURSION: &str = "(call $r (i32.const 0))";
const R: &str =
    "(func $r (param i32) (result i64) (call $r (i32.add (local.get 0) (i32.const 1))))";

#[test]
fn recursion_faute_profondeur() {
    let c = Colle::new(RECURSION).extra(R);
    assert_eq!(
        meme_endroit(&c, 10_000_000),
        Echec::Faute(Faute::Profondeur)
    );
}

/// Des cadres natifs lourds, beaucoup de locaux vivants, sous les réglages de la
/// phase 1 : `soiree_profondeur` pique avant la pile native (§ 8.7).
#[test]
fn profondeur_pique_avant_la_pile_native() {
    let mut locaux = String::new();
    let mut somme = String::from("(local.get 0)");
    for k in 1..40 {
        write!(locaux, " (local $l{k} i64)").unwrap();
        somme =
            format!("(i64.add {somme} (local.tee $l{k} (i64.mul (local.get 0) (i64.const {k}))))");
    }
    let lourd = format!(
        "(func $r (param i64) (result i64){locaux}
           (i64.add {somme} (call $r (i64.add (local.get 0) (i64.const 1)))))"
    );
    let c = Colle::new("(call $r (i64.const 0))").extra(&lourd);
    let moteur = Moteur::new(ReglagesMoteur::PHASE_1).unwrap();
    let mut i = moteur
        .charger(&c.octets())
        .unwrap()
        .fabrique()
        .neuve()
        .unwrap();
    assert_eq!(
        i.appeler(ECRITURE, &entree(1), u64::MAX >> 2),
        Err(Echec::Faute(Faute::Profondeur))
    );
}

/// Une récursion de `appels` étages dont chaque cadre garde `locaux` valeurs `i64`
/// vivantes à travers l'appel : un cadre épais.
fn cadres_epais(locaux: u32, appels: u32) -> Colle {
    let mut decl = String::new();
    let mut poser = String::new();
    let mut somme = String::from("(i64.const 0)");
    for k in 0..locaux {
        write!(decl, " (local $l{k} i64)").unwrap();
        write!(
            poser,
            " (local.set $l{k} (i64.load (i32.const {})))",
            8u32.checked_mul(k)
                .and_then(|o| o.checked_add(200_000))
                .unwrap()
        )
        .unwrap();
        somme = format!("(i64.add {somme} (local.get $l{k}))");
    }
    let r = format!(
        "(func $r (param $d i32) (result i64){decl}{poser}
           (i64.add
             (if (result i64) (i32.lt_u (local.get $d) (i32.const {appels}))
               (then (call $r (i32.add (local.get $d) (i32.const 1))))
               (else (i64.const 0)))
             {somme}))"
    );
    Colle::new("(drop (call $r (i32.const 0))) (call $fait)").extra(&r)
}

/// Des cadres de plus de 4 Kio sous `soiree_profondeur` : la récursion se termine,
/// ou la profondeur pique, et jamais la pile native d'abord (§ 8.7). Sans quoi une
/// récursion permise devient une panne de l'outil, et à une autre profondeur sur un
/// autre moteur.
#[test]
fn cadres_epais_jamais_la_pile_native() {
    let moteur = Moteur::new(ReglagesMoteur::PHASE_1).unwrap();
    for (locaux, appels) in [
        (40, 4000),
        (450, 4000),
        (600, 2500),
        (600, 4000),
        (2000, 2500),
        (5000, 1000),
    ] {
        let mut i = moteur
            .charger(&cadres_epais(locaux, appels).octets())
            .unwrap()
            .fabrique()
            .neuve()
            .unwrap();
        let r = i.appeler(ECRITURE, &entree(1), u64::MAX >> 2);
        assert!(
            matches!(r, Ok(_) | Err(Echec::Faute(Faute::Profondeur))),
            "{locaux} locaux, {appels} appels : {r:?}"
        );
    }
}

/// Le gestionnaire de panique écrit son code, la longueur et le message dans la
/// zone de panique, puis `unreachable`.
const PANIQUE: &str = "
    (i32.store (i32.const 135168) (i32.const 1))
    (i32.store (i32.const 135172) (i32.const 23))
    (memory.copy (i32.const 135176) (i32.const 190000) (i32.const 23))
    (unreachable)";

#[test]
fn panique_faute_avec_message() {
    let c = Colle::new(PANIQUE).extra(r#"(data (i32.const 190000) "le jeu panique, voilà.")"#);
    assert_eq!(
        meme_endroit(&c, 1_000_000),
        Echec::Faute(Faute::Panique {
            message: String::from("le jeu panique, voilà.")
        })
    );
}

/// Un `unreachable` sans message n'est pas une panique : c'est un piège, nommé.
#[test]
fn unreachable_sans_message_est_un_piege() {
    let c = Colle::new("(unreachable)");
    let Echec::Faute(Faute::Piege { code }) = meme_endroit(&c, 1_000_000) else {
        panic!()
    };
    assert!(code.contains("unreachable"), "{code}");
    let c = Colle::new("(i64.div_s (i64.const 1) (i64.const 0))");
    let Echec::Faute(Faute::Piege { code }) = meme_endroit(&c, 1_000_000) else {
        panic!()
    };
    assert!(code.contains("divide by zero"), "{code}");
}

/// La croissance sans fin : `memory.grow` refusé au-delà de `limites.memoire`, puis
/// l'allocateur panique. Une faute de mémoire, pas une panique (§ 4.6).
const CROISSANCE: &str = "
    (loop $l (br_if $l (i32.ne (memory.grow (i32.const 1)) (i32.const -1))))
    (i32.store (i32.const 135168) (i32.const 1))
    (i32.store (i32.const 135172) (i32.const 0))
    (unreachable)";

#[test]
fn memoire_au_dela_faute_memoire() {
    let c = Colle::new(CROISSANCE);
    assert_eq!(meme_endroit(&c, 10_000_000), Echec::Faute(Faute::Memoire));
    // La mémoire s'est arrêtée à `limites.memoire`.
    let mut i = instance(&c);
    let _: Result<Vec<u8>, Echec> = i.appeler(ECRITURE, &entree(1), 10_000_000);
    assert_eq!(i.pages().unwrap(), 16);
}

/// Une croissance refusée, que le jeu traite sans paniquer, n'est pas une faute.
#[test]
fn croissance_refusee_et_traitee_n_est_pas_une_faute() {
    let c = Colle::new("(drop (memory.grow (i32.const 1000))) (call $fait)");
    assert!(jouer(&c, 1_000_000).0.is_ok());
}

/// Une machine qui refuse une croissance que le module a le droit de faire : une
/// panne de la machine, jamais une faute du jeu.
#[test]
fn croissance_refusee_par_la_machine_panne_de_la_machine() {
    let c = Colle::new("(drop (memory.grow (i32.const 4))) (call $fait)");
    let mut f = c.charger().fabrique();
    f.simuler_une_machine_limitee(6 * 65_536);
    let mut i = f.neuve().unwrap();
    let Err(Echec::Panne(p)) = i.appeler(ECRITURE, &entree(1), 1_000_000) else {
        panic!("la machine devait refuser")
    };
    assert_eq!(p.cause, CausePanne::CroissanceRefusee);
    assert_eq!(p.responsable(), Responsable::Machine);
}

#[test]
fn acces_hors_memoire_faute() {
    let c = Colle::new("(i64.load (i32.const -16))");
    assert_eq!(
        meme_endroit(&c, 1_000_000),
        Echec::Faute(Faute::HorsMemoire)
    );
}

/// Un module qui ne remet pas `__stack_pointer` : panne de l'outil, car c'est la
/// colle du commun qui tient la pile (§ 3.2).
#[test]
fn pile_non_rendue_panne_de_l_outil() {
    let c = Colle::new("(global.set $__stack_pointer (i32.const 60000)) (call $fait)");
    let Err(Echec::Panne(p)) = jouer(&c, 1_000_000).0 else {
        panic!()
    };
    assert_eq!(p.cause, CausePanne::PileNonRendue);
}

#[test]
fn sortie_hors_du_brouillon_faute() {
    for r in [
        "(i64.const 10)",
        "(i64.const 0x0000_0002_0000_0010)",
        "(i64.const -1)",
        "(i64.const 0x0002_1fff_0000_1000)",
    ] {
        let c = Colle::new(r);
        assert_eq!(
            jouer(&c, 1_000_000).0,
            Err(Echec::Faute(Faute::SortieInvalide)),
            "{r}"
        );
    }
}

/// Une lecture plus grande que la boîte est refusée avant d'écrire : panne de
/// l'outil, puisque c'est le commun qui l'a encodée (§ 3.4).
#[test]
fn lecture_trop_grande_refusee_avant_d_ecrire() {
    let mut i = instance(&Colle::new(COMPTER));
    let avant = i.empreinte().unwrap();
    let grande = vec![0u8; 5000];
    let Err(Echec::Panne(p)) = i.appeler(LECTURE, &grande, 1_000_000) else {
        panic!()
    };
    assert_eq!(p.cause, CausePanne::BoiteRefusee);
    // Une écriture de cette taille, la boîte la refuse aussi (la colle n'a pas de tas).
    let Err(Echec::Panne(p)) = i.appeler(ECRITURE, &grande, 1_000_000) else {
        panic!()
    };
    assert_eq!(p.cause, CausePanne::BoiteRefusee);
    assert_eq!(i.empreinte().unwrap(), avant);
    assert!(i.appeler(LECTURE, &lecture(), 1_000_000).is_ok());
}

/// Une lecture que la boîte place hors d'elle-même toucherait l'état : refusée.
#[test]
fn lecture_hors_de_la_boite_refusee() {
    let texte = Colle::new(COMPTER).wat().replace(
        "    (i32.const 65536))\n  (func (export \"soiree_appel\")",
        "    (if (result i32) (local.get $sorte) (then (i32.const 65536)) (else (i32.const 200000))))\n  (func (export \"soiree_appel\")",
    );
    // Bonjour est une lecture : le module est refusé dès le chargement.
    let r = Moteur::new(REGLAGES)
        .unwrap()
        .charger(&wat::parse_str(&texte).unwrap());
    assert!(
        matches!(
            r,
            Err(moteur_wasmtime::RefusModule::Faute {
                faute: Faute::SortieInvalide,
                ..
            })
        ),
        "{:?}",
        r.err()
    );
}

/// Après une faute, l'instance restaurée repart comme avant.
#[test]
fn apres_une_faute_restaurer_et_continuer() {
    let c = Colle::new(
        "(if (i32.eq (i32.load8_u (i32.const 65537)) (i32.const 9)) (then (loop $l (br $l))))
         (i64.store (i32.const 200000) (i64.add (i64.load (i32.const 200000)) (i64.const 1)))
         (call $fait)",
    );
    let mut i = instance(&c);
    i.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    let point = i.instantane().unwrap();
    let e = i.empreinte().unwrap();
    assert_eq!(
        i.appeler(ECRITURE, &entree(9), 10_000),
        Err(Echec::Faute(Faute::Budget))
    );
    i.restaurer(&point).unwrap();
    assert_eq!(i.empreinte().unwrap(), e);
    i.appeler(ECRITURE, &entree(2), 1_000_000).unwrap();
    let mut j = instance(&c);
    j.appeler(ECRITURE, &entree(1), 1_000_000).unwrap();
    j.appeler(ECRITURE, &entree(2), 1_000_000).unwrap();
    assert_eq!(i.empreinte().unwrap(), j.empreinte().unwrap());
}
