//! Les tests de la salle que nomme PHASE-3, § 8.1 (lot 4b), chacun avec sa leçon : la
//! vraie autorité et de vrais appareils, sur le modèle du cœur, par le relais en
//! mémoire, sans latence sauf où c'est dit, menés à la main et à graine fixe (SAL-09).
//!
//! `cfg(test)` marque tout ce binaire comme du code de test : les fonctions d'aide y
//! ont droit à `unwrap`, `expect`, `panic!` et aux index (CONTRAT, « Les règles
//! d'écriture »).

#![cfg(test)]
#![expect(
    clippy::arithmetic_side_effects,
    reason = "code de test : des instants et des numéros petits ; un débordement y serait l'échec du test"
)]

use jeu_abi::{Appareil, Capacites, Instant, Oui, Pas, Presence, encoder};
use liaison::{Classe, DuRelais, Lien, VersRelais};
use partie::modele::{EtatModele, FabriqueModele, GesteModele, InstantaneModele, TenuModele};
use partie::{Ligne, QuoiPlace};
use proptest::prelude::*;
use salle::autorite::id_geste;
use salle::chiffre::{Initiateur, Session};
use salle::fil::{PROTOCOLE, RefusSalle, TraceTelephone, VersAutorite};
use salle::garde::empreinte_jeton;
use salle::simulation::{Graine, Pannes, Soiree, verifier_l_historique};

type SoireeModele = Soiree<FabriqueModele, fn() -> FabriqueModele>;

fn soiree(banc: Oui) -> SoireeModele {
    Soiree::new(
        FabriqueModele::default as fn() -> FabriqueModele,
        7,
        banc,
        Pannes::AUCUNE,
    )
}

fn attendre(s: &mut SoireeModele, us: u64) {
    let fin = s.t_us + us;
    s.avancer_jusqua(fin);
}

fn etat(s: &SoireeModele) -> EtatModele {
    s.autorite
        .as_ref()
        .and_then(|a| a.partie())
        .and_then(|p| p.definitive().etat())
        .cloned()
        .expect("l'état du modèle")
}

/// L'appareil que la salle a donné au téléphone `k`.
fn appareil(s: &SoireeModele, k: usize) -> Appareil {
    s.autorite
        .as_ref()
        .and_then(salle::autorite::Autorite::registre)
        .and_then(|r| {
            r.par_jeton(&empreinte_jeton(&s.telephones[k].jeton))
                .map(|i| i.appareil)
        })
        .expect("l'appareil du téléphone")
}

/// Un geste `Ajouter(1)` du téléphone `k` ; rend son numéro.
fn ajouter(s: &mut SoireeModele, k: usize) -> u64 {
    let mut geste = Vec::new();
    encoder(&GesteModele::Ajouter(1), &mut geste).unwrap();
    let heure = s.heure_du_telephone(k);
    let numero = s.telephones[k]
        .appareil
        .geste(heure, &geste)
        .expect("le geste part");
    s.pousser(k);
    numero
}

fn tenir(s: &mut SoireeModele, k: usize, valeur: u8) {
    let mut o = Vec::new();
    encoder(&TenuModele { cle: 1, valeur }, &mut o).unwrap();
    let t = s.heure_du_telephone(k);
    s.telephones[k].appareil.tenir(t, &o);
    s.pousser(k);
}

/// Les présences de cet appareil que la partie a jouées, d'après son journal.
fn presences(s: &SoireeModele, a: Appareil) -> Vec<Presence> {
    let j = s
        .autorite
        .as_ref()
        .and_then(|x| x.partie())
        .expect("la partie")
        .journal();
    j.lignes()
        .iter()
        .filter_map(|l| match l {
            Ligne::Arrivees { arrivees, .. } => Some(arrivees.iter()),
            Ligne::Entree(_)
            | Ligne::Empreinte { .. }
            | Ligne::Saut { .. }
            | Ligne::Pause { .. }
            | Ligne::Vue { .. }
            | Ligne::Partie { .. }
            | Ligne::Reprise { .. } => None,
        })
        .flatten()
        .filter_map(|x| match &x.quoi {
            QuoiPlace::Presence(p) if appareil_de(p) == a => Some(*p),
            QuoiPlace::Presence(_)
            | QuoiPlace::Geste { .. }
            | QuoiPlace::Tenu { .. }
            | QuoiPlace::Manette { .. }
            | QuoiPlace::Ecran(_)
            | QuoiPlace::Reponse { .. } => None,
        })
        .collect()
}

fn appareil_de(p: &Presence) -> Appareil {
    match p {
        Presence::Arrivee { appareil, .. }
        | Presence::Depart { appareil, .. }
        | Presence::Retour { appareil, .. }
        | Presence::Capacites { appareil, .. } => *appareil,
    }
}

/// SAL-29 : un bonjour répété sur le même lien (un accueil perdu pousse le téléphone à
/// redemander) redonne le même accueil, et ne change rien d'autre : ni présence, ni
/// session, ni remplacement.
#[test]
fn rejoindre_est_idempotent_sal29() {
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    let a = &s.telephones[k].appareil;
    assert!(a.accueilli());
    assert_eq!(a.comptes().accueils, 1);
    let session = a.session();
    let ap = appareil(&s, k);
    assert_eq!(presences(&s, ap).len(), 1);
    for _ in 0..3 {
        s.telephones[k].appareil.redire_le_bonjour();
        s.pousser(k);
        attendre(&mut s, 200_000);
    }
    let a = &s.telephones[k].appareil;
    assert_eq!(a.comptes().accueils, 4);
    assert_eq!(a.session(), session, "le même accueil");
    assert!(a.accueilli() && a.refus().is_none());
    assert_eq!(presences(&s, ap).len(), 1, "aucune présence de plus");
    assert_eq!(etat(&s).presents, vec![ap.fil()]);
    assert_eq!(s.autorite.as_ref().unwrap().comptes().remplaces, 0);
}

/// SAL-18, SAL-19 : le lien le plus récent gagne, dans l'ordre où le relais a ouvert les
/// liens, et non dans l'ordre d'arrivée des bonjours. Un bonjour resté coincé sur une
/// tentative abandonnée, livré après celui du lien suivant, ne déloge pas ce lien ; et un
/// lien plus récent remplace le courant, qui reçoit « Remplacé » et ne se reconnecte pas
/// seul.
#[test]
fn le_lien_le_plus_recent_gagne_sal19() {
    let mut s = soiree(Oui::Non);
    // La première tentative reste coincée trois secondes.
    let ancien = s.ajouter_telephone();
    let coince = s.t_us + 3_000_000;
    s.retenir(ancien, Oui::Oui, coince);
    let recent = s.retenter(ancien).unwrap();
    attendre(&mut s, 1_000_000);
    assert!(s.telephones[recent].appareil.accueilli());
    assert!(!s.telephones[ancien].appareil.accueilli());
    attendre(&mut s, 4_000_000);
    // Le bonjour tardif de la tentative abandonnée est refusé : il ne déloge personne.
    assert_eq!(
        s.telephones[ancien].appareil.refus(),
        Some(RefusSalle::Remplace)
    );
    assert!(s.telephones[recent].appareil.accueilli());
    assert!(s.telephones[recent].appareil.refus().is_none());
    // Puis un onglet plus récent du même appareil prend la place du courant.
    let plus_recent = s.retenter(recent).unwrap();
    attendre(&mut s, 1_000_000);
    assert!(s.telephones[plus_recent].appareil.accueilli());
    assert_eq!(
        s.telephones[recent].appareil.refus(),
        Some(RefusSalle::Remplace)
    );
    // L'éjecté ne revient pas seul.
    attendre(&mut s, 5_000_000);
    assert!(s.telephones[recent].appareil.fini());
    assert!(s.telephones[plus_recent].appareil.accueilli());
    let ap = appareil(&s, recent);
    assert_eq!(etat(&s).presents, vec![ap.fil()], "un seul appareil");
}

/// SAL-17 : un invité qui a capté la première poignée de main d'un téléphone (elle passe
/// en clair sur le Wi-Fi) et la renvoie obtient une réponse, mais aucun effet : ni
/// remplacement du vrai téléphone, ni présence, ni retour d'un téléphone parti. Sa
/// poignée de main inachevée est coupée après 10 s.
#[test]
fn une_poignee_de_main_rejouee_ne_remplace_personne_sal17() {
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    assert!(s.telephones[k].appareil.accueilli());
    let ap = appareil(&s, k);
    let avant = presences(&s, ap);
    assert_eq!(avant.len(), 1);
    s.rejouer_la_poignee(k).expect("une poignée captée");
    attendre(&mut s, 2_000_000);
    let a = &s.telephones[k].appareil;
    assert!(a.accueilli() && a.refus().is_none() && !a.fini());
    assert_eq!(presences(&s, ap), avant);
    let c = s.autorite.as_ref().unwrap().comptes();
    assert_eq!(c.remplaces, 0);
    attendre(&mut s, 9_000_000);
    let c = s.autorite.as_ref().unwrap().comptes();
    assert_eq!(
        c.poignees_inachevees, 1,
        "la poignée de l'invité est coupée"
    );
    assert!(s.telephones[k].appareil.accueilli());
    assert_eq!(etat(&s).presents, vec![ap.fil()]);

    // Le téléphone part ; la même poignée rejouée ne le fait pas revenir.
    s.couper(k, None);
    attendre(&mut s, 1_000_000);
    assert!(etat(&s).presents.is_empty(), "parti");
    s.rejouer_la_poignee(k).expect("une poignée captée");
    attendre(&mut s, 2_000_000);
    assert!(etat(&s).presents.is_empty(), "aucun retour");
    assert_eq!(
        s.autorite.as_ref().unwrap().partie().unwrap().presence(ap),
        Some(Oui::Non)
    );
}

/// Donne une trame à l'autorité sur ce lien, comme le relais l'aurait fait, et rend ce
/// qu'elle envoie aussitôt sur le temps réel.
fn vers_l_autorite(s: &mut SoireeModele, lien: Lien, trame: &[u8]) -> Vec<Vec<u8>> {
    let t = s.t_us;
    let de = DuRelais::De {
        lien,
        classe: Classe::Fiable,
        trame,
        recu_us: t,
        renvoye_us: t,
    }
    .encoder()
    .unwrap();
    let a = s.autorite.as_mut().unwrap();
    a.temps_reel(t, &de);
    a.sorties().temps_reel
}

/// Un lien neuf, dont la poignée de main est faite par quelqu'un qui a le fragment du QR :
/// rend le lien et la session, avant tout bonjour.
fn poignee_faite(s: &mut SoireeModele, graine: u64) -> (Lien, Session) {
    let lien = s.relais.ouvrir();
    let mut h = Graine::new("poignee-faite", graine);
    let (initiateur, premier) = Initiateur::commencer(&s.fragment(), &mut h).unwrap();
    let second = vers_l_autorite(s, lien, &premier)
        .iter()
        .find_map(|o| match VersRelais::decoder(o) {
            Ok(VersRelais::Vers { lien: l, trame, .. }) if l == lien => Some(trame.to_vec()),
            _ => None,
        })
        .expect("la réponse de l'autorité à la poignée de main");
    (lien, initiateur.finir(&second).unwrap())
}

/// SAL-117, SAL-17 : sur un lien dont la poignée de main est faite, un premier message
/// qui n'est pas le bonjour (un geste, un tenu, un écho, un morceau du module reçu) coupe
/// ce lien, et lui seul, sans rien placer ; un bonjour envoyé ensuite sur ce lien n'est
/// plus écouté. Le téléphone honnête reste accueilli, et rien de lui ne change.
#[test]
fn un_message_avant_le_bonjour_coupe_son_seul_lien_sal117() {
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    assert!(s.telephones[k].appareil.accueilli());
    let ap = appareil(&s, k);
    let avant = presences(&s, ap);
    let tenus_avant = etat(&s).tenus;
    let mut geste = Vec::new();
    encoder(&GesteModele::Ajouter(1), &mut geste).unwrap();
    let mut tenu = Vec::new();
    encoder(&TenuModele { cle: 1, valeur: 3 }, &mut tenu).unwrap();
    let fait = Instant::de(Pas::ZERO);
    let messages = [
        VersAutorite::Geste {
            onglet: 1,
            numero: 1,
            octets: &geste,
            fait,
            vu: Pas::ZERO,
            trace: TraceTelephone {
                evenement_us: 0,
                envoi_us: 0,
                incertitude_us: 0,
            },
        },
        VersAutorite::Tenu {
            numero: 1,
            octets: &tenu,
            fait,
        },
        VersAutorite::Echo {
            t_us: 5,
            echo_us: 0,
        },
        VersAutorite::ModuleRecu {
            empreinte: [0; 32],
            morceau: 0,
        },
    ];
    for (i, m) in messages.iter().enumerate() {
        let (lien, mut session) = poignee_faite(&mut s, i as u64);
        let trame = session
            .sceller(Classe::Fiable, &m.encoder().unwrap())
            .unwrap();
        let sorties = vers_l_autorite(&mut s, lien, &trame);
        let coupes: Vec<Lien> = sorties
            .iter()
            .filter_map(|o| match VersRelais::decoder(o) {
                Ok(VersRelais::Couper { lien, .. }) => Some(lien),
                _ => None,
            })
            .collect();
        assert_eq!(coupes, vec![lien], "{m:?} avant le bonjour coupe son lien");
        // Le bonjour qui suit, sur le même lien, n'est plus écouté : ni accueil, ni rien.
        let bonjour = VersAutorite::Bonjour {
            protocole: PROTOCOLE,
            jeton: [9; 32],
            onglet: 1,
            capacites: Capacites::AUCUNE,
            module_connu: None,
        };
        let trame = session
            .sceller(Classe::Fiable, &bonjour.encoder().unwrap())
            .unwrap();
        assert_eq!(
            vers_l_autorite(&mut s, lien, &trame),
            Vec::<Vec<u8>>::new(),
            "un lien coupé ne dit plus rien"
        );
    }
    attendre(&mut s, 1_000_000);
    let a = s.autorite.as_ref().unwrap();
    assert!(a.arret().is_none());
    let c = a.comptes();
    assert_eq!(c.gestes_places, 0, "aucun geste placé");
    assert_eq!(c.remplaces, 0);
    assert!(
        a.registre()
            .unwrap()
            .par_jeton(&empreinte_jeton(&[9; 32]))
            .is_none(),
        "aucun appareil de plus"
    );
    assert!(s.telephones[k].appareil.accueilli());
    assert!(s.telephones[k].appareil.refus().is_none());
    assert_eq!(presences(&s, ap), avant);
    assert_eq!(etat(&s).presents, vec![ap.fil()]);
    assert_eq!(etat(&s).tenus, tenus_avant, "aucun tenu de plus");
}

/// SAL-38 : une page rechargée repart de zéro pour ses tenus ; l'accueil lui redonne le
/// dernier numéro reçu, et son premier tenu est entendu (sans cela, chacun serait
/// « périmé » sans que rien ne le dise).
#[test]
fn l_accueil_realigne_les_tenus_sal38() {
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    for v in 1..=5 {
        tenir(&mut s, k, v);
        attendre(&mut s, 100_000);
    }
    let ap = appareil(&s, k).fil();
    assert!(etat(&s).tenus.contains(&(ap, 1, 5)));
    let avant = s.telephones[k].appareil.numero_tenu();
    assert!(avant >= 5);
    s.recharger(k, 300_000);
    assert_eq!(s.telephones[k].appareil.numero_tenu(), 0);
    attendre(&mut s, 1_000_000);
    assert!(s.telephones[k].appareil.accueilli());
    assert!(s.telephones[k].appareil.numero_tenu() >= avant, "réaligné");
    tenir(&mut s, k, 9);
    attendre(&mut s, 200_000);
    assert!(
        etat(&s).tenus.contains(&(ap, 1, 9)),
        "le premier tenu de la page rechargée est entendu : {:?}",
        etat(&s).tenus
    );
}

/// SAL-56 : un geste n'est accusé qu'une fois écrite la case de journal qui le contient
/// (l'accusé durable) ; avant, l'autorité qui meurt le perdrait, et le téléphone, qui
/// l'aurait jeté de sa file, ne le renverrait pas.
#[test]
fn un_geste_n_est_accuse_qu_une_fois_ecrit_sal56() {
    // Le relais met de 5 à 30 ms à écrire une case, et elle n'est écrite (dans la garde,
    // dans l'historique, et dite par `Ecrit`) qu'au bout de ce temps : un accusé envoyé
    // dès l'`Ecrire`, avant l'`Ecrit`, arrive au téléphone pendant que sa case s'écrit
    // encore. Et un `Ecrit` arrive après des gestes placés que sa case ne contient pas.
    let mut s: SoireeModele = Soiree::new(
        FabriqueModele::default as fn() -> FabriqueModele,
        7,
        Oui::Non,
        Pannes {
            garde_us: (5_000, 30_000),
            ..Pannes::AUCUNE
        },
    );
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    let mut numeros = Vec::new();
    // Les instants où une case s'écrit pendant que le téléphone attend un accusé : la
    // fenêtre où un accusé trop tôt se verrait.
    let mut en_attente = 0;
    for tour in 0..400 {
        if tour % 7 == 0 {
            numeros.push(ajouter(&mut s, k));
        }
        attendre(&mut s, 1_000);
        if s.cases_en_ecriture() > 0 && s.telephones[k].appareil.en_file() > 0 {
            en_attente += 1;
        }
        let accuse = s.telephones[k].appareil.accuse();
        if accuse == 0 {
            continue;
        }
        let h = verifier_l_historique::<InstantaneModele>(
            &s.cle_de_garde(),
            s.salle(),
            &s.relais.historique(),
        )
        .unwrap();
        let ap = appareil(&s, k);
        let onglet = s.telephones[k].appareil.onglet();
        for n in 1..=accuse {
            assert!(
                h.gestes.contains(&id_geste(ap, onglet, n).fil()),
                "le geste {n} est accusé avant d'être écrit"
            );
        }
    }
    assert!(numeros.len() > 50);
    assert!(s.telephones[k].appareil.accuse() > 40);
    assert!(
        en_attente > 20,
        "la fenêtre n'a été vue que {en_attente} fois"
    );
}

/// Combien de fois le modèle a reçu ce geste.
fn recu(s: &SoireeModele, k: usize, n: u64) -> usize {
    let id = id_geste(appareil(s, k), s.telephones[k].appareil.onglet(), n).fil();
    etat(s).gestes.iter().filter(|&&g| g == id).count()
}

/// SAL-57 et SAL-56 : un geste en vol quand l'autorité meurt compte exactement une fois.
/// Placé mais pas encore écrit, il est perdu avec elle : le téléphone, qui n'a pas reçu
/// d'accusé, le renvoie à la suivante, qui le place. Écrit, mais son accusé perdu, il est
/// dans la garde : la suivante le reconnaît, l'accuse, et ne le replace pas.
#[test]
fn un_geste_compte_une_fois_a_travers_le_redemarrage_de_l_autorite_sal57() {
    // Placé, pas écrit.
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    let n = ajouter(&mut s, k);
    s.vider_le_reseau();
    assert_eq!(s.autorite.as_ref().unwrap().comptes().gestes_places, 1);
    s.tuer_l_autorite(500_000);
    attendre(&mut s, 3_000_000);
    assert!(s.telephones[k].appareil.accueilli());
    assert_eq!(s.telephones[k].appareil.accuse(), n);
    assert_eq!(s.telephones[k].appareil.en_file(), 0);
    assert_eq!(recu(&s, k, n), 1);

    // Écrit, accusé perdu : l'autorité meurt juste après avoir écrit la case qui le
    // contient, avant toute autre case.
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    let loin = s.t_us + 60_000_000;
    s.retenir(k, Oui::Non, loin);
    let n = ajouter(&mut s, k);
    let id = id_geste(appareil(&s, k), s.telephones[k].appareil.onglet(), n).fil();
    let mut pas = 0;
    loop {
        let h = verifier_l_historique::<InstantaneModele>(
            &s.cle_de_garde(),
            s.salle(),
            &s.relais.historique(),
        )
        .unwrap();
        if h.gestes.contains(&id) {
            break;
        }
        attendre(&mut s, 1_000);
        pas += 1;
        assert!(pas < 1_000, "le geste n'est jamais écrit");
    }
    assert_eq!(
        s.telephones[k].appareil.accuse(),
        0,
        "son accusé est retenu"
    );
    s.tuer_l_autorite(300_000);
    s.couper(k, Some(600_000));
    attendre(&mut s, 3_000_000);
    assert!(s.telephones[k].appareil.accueilli());
    assert_eq!(
        s.telephones[k].appareil.accuse(),
        n,
        "accusé par la suivante"
    );
    assert_eq!(s.telephones[k].appareil.en_file(), 0);
    assert_eq!(recu(&s, k, n), 1);
    s.comparer_a_la_garde().unwrap();

    // Renvoyés alors qu'ils sont déjà placés : ignorés, comptés une fois.
    let mut s = soiree(Oui::Non);
    let k = s.ajouter_telephone();
    attendre(&mut s, 1_000_000);
    let loin = s.t_us + 60_000_000;
    s.retenir(k, Oui::Non, loin);
    let numeros: Vec<u64> = (0..3).map(|_| ajouter(&mut s, k)).collect();
    attendre(&mut s, 500_000);
    s.telephones[k].appareil.renvoyer_la_file();
    s.pousser(k);
    attendre(&mut s, 500_000);
    assert_eq!(s.autorite.as_ref().unwrap().comptes().gestes_en_double, 3);
    for n in numeros {
        assert_eq!(recu(&s, k, n), 1, "geste {n}");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// REM-64, SAL-117 : des octets au hasard sur les deux liens de l'autorité, de la
    /// page, ou en trames d'un appareil (un lien connu ou non), ne la font jamais
    /// paniquer ; une trame qui ne se déchiffre pas ne coûte que son lien, et le
    /// téléphone honnête reste accueilli. Des octets au hasard du relais ne font pas
    /// paniquer un appareil.
    #[test]
    fn des_octets_au_hasard_ne_font_paniquer_ni_l_autorite_ni_un_appareil_rem64(
        brut in proptest::collection::vec(any::<u8>(), 0..120),
        trame in proptest::collection::vec(any::<u8>(), 0..120),
        lien in 0u32..6,
    ) {
        let mut s = soiree(Oui::Non);
        let k = s.ajouter_telephone();
        attendre(&mut s, 500_000);
        prop_assert!(s.telephones[k].appareil.accueilli());
        let honnete = s.telephones[k].lien.unwrap();
        let t = s.t_us;
        let de = DuRelais::De {
            lien: Lien(lien),
            classe: Classe::Fiable,
            trame: &trame,
            recu_us: 0,
            renvoye_us: 0,
        }
        .encoder()
        .unwrap();
        let a = s.autorite.as_mut().unwrap();
        a.temps_reel(t, &brut);
        a.garde(t, &brut);
        a.page(t, &brut);
        if Lien(lien) != honnete {
            a.temps_reel(t, &de);
        }
        let _: salle::autorite::Sorties = a.sorties();
        s.telephones[k].appareil.recevoir(t, &brut);
        s.pousser(k);
        attendre(&mut s, 500_000);
        prop_assert!(s.autorite.as_ref().unwrap().arret().is_none());
        prop_assert!(s.telephones[k].appareil.accueilli());
    }
}
