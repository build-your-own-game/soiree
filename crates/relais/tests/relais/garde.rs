//! La garde sur disque : écrire durablement (SAL-26, SAL-27), relire, effacer, borner ; un
//! relais relancé garde sa salle, sa preuve et ses cases ; `oublier` (P41) ;
//! l'enregistrement de ce que le relais voit.

use std::time::Duration;

use liaison::{Case, CodeSalle, DuRelais, RefusRelais, Role, VersRelais};
use relais::coeur::{Fait, OpGarde};
use relais::coque;
use relais::enregistrement::{self, Sens};
use relais::garde::{Dossier, nom_de_numero, numero_d_un_nom};
use relais::http::Banc;

use crate::commun::{client, dossier, http, reglages};
use crate::coque::autorite;

const SALLE: CodeSalle = CodeSalle([0xab, 0xcd, 0x01, 0x23]);
const PREUVE: [u8; 32] = [3; 32];
const UN_PEU: Duration = Duration::from_secs(5);

#[test]
fn le_nom_d_une_case_se_lit_et_garde_l_ordre_des_numeros() {
    for n in [0, 7, u64::MAX] {
        assert_eq!(numero_d_un_nom(&nom_de_numero(n)), Some(n));
    }
    assert!(nom_de_numero(9) < nom_de_numero(10));
    for n in [
        "7",
        "00000000000000000007-p",
        ".00000000000000000007.tmp",
        "preuve",
    ] {
        assert_eq!(numero_d_un_nom(n), None, "{n}");
    }
}

/// SAL-26, SAL-27 : une case s'écrit dans un fichier temporaire, synchronisé, renommé ; un
/// fichier temporaire resté d'un arrêt brutal est jeté à la lecture ; un numéro ne nomme
/// qu'une case ; `Effacer` garde ce qui suit.
#[test]
fn une_case_s_ecrit_durablement_se_relit_et_s_efface() {
    let d = Dossier::ouvrir(&dossier("garde-disque")).unwrap();
    d.ecrire(SALLE, Case::Point(0), b"p0").unwrap();
    d.ecrire(SALLE, Case::Journal(1), b"j1").unwrap();
    d.ecrire(SALLE, Case::Journal(2), b"j2").unwrap();
    // Un arrêt brutal pendant l'écriture de la case 3 : son fichier temporaire reste.
    let cases = d.racine().join("ABCD0123").join("cases");
    std::fs::write(cases.join(".00000000000000000003.tmp"), b"ja moitie").unwrap();
    assert_eq!(
        d.lister(SALLE).unwrap(),
        vec![Case::Point(0), Case::Journal(1), Case::Journal(2)]
    );
    assert!(!cases.join(".00000000000000000003.tmp").exists());
    assert_eq!(
        d.lire(SALLE, Case::Journal(1)).unwrap().as_deref(),
        Some(&b"j1"[..])
    );
    assert_eq!(d.lire(SALLE, Case::Point(1)).unwrap(), None);
    // Le même numéro, de l'autre sorte : il remplace.
    d.ecrire(SALLE, Case::Point(2), b"p2").unwrap();
    assert_eq!(
        d.lister(SALLE).unwrap(),
        vec![Case::Point(0), Case::Journal(1), Case::Point(2)]
    );
    d.effacer(SALLE, 2).unwrap();
    assert_eq!(d.lister(SALLE).unwrap(), vec![Case::Point(2)]);
    assert_eq!(d.taille(SALLE).unwrap(), 2);
    // Ce que la coque fait d'une opération.
    assert_eq!(
        d.faire(
            SALLE,
            &OpGarde::Lire {
                case: Case::Point(2)
            }
        ),
        Fait::Lu(Case::Point(2), b"p2".to_vec())
    );
    assert_eq!(
        d.faire(
            SALLE,
            &OpGarde::Lire {
                case: Case::Point(9)
            }
        ),
        Fait::Absente(Case::Point(9))
    );
}

/// SAL-07 : une garde pleine refuse l'écriture, typée, et ne jette rien.
#[test]
fn une_garde_pleine_refuse_l_ecriture_sans_rien_jeter_sal07() {
    let d = Dossier::ouvrir(&dossier("garde-pleine")).unwrap();
    d.ecrire(SALLE, Case::Point(0), &[1; 10]).unwrap();
    let op = OpGarde::Ecrire {
        case: Case::Journal(1),
        octets: vec![2; 7],
    };
    assert_eq!(
        d.faire_sous(SALLE, &op, 16),
        Fait::Refus(RefusRelais::GardePleine)
    );
    assert_eq!(d.lister(SALLE).unwrap(), vec![Case::Point(0)]);
    assert_eq!(d.faire_sous(SALLE, &op, 17), Fait::Ecrit(Case::Journal(1)));
    // Réécrire une case ne compte pas deux fois sa taille.
    let op = OpGarde::Ecrire {
        case: Case::Point(0),
        octets: vec![1; 10],
    };
    assert_eq!(d.faire_sous(SALLE, &op, 17), Fait::Ecrit(Case::Point(0)));
    // Ni la remplacer par une case de l'autre sorte, du même numéro.
    let op = OpGarde::Ecrire {
        case: Case::Point(1),
        octets: vec![3; 7],
    };
    assert_eq!(d.faire_sous(SALLE, &op, 17), Fait::Ecrit(Case::Point(1)));
    assert_eq!(d.taille(SALLE).unwrap(), 17);
}

/// SAL-27, et la vérification du lot 5 : une case qui en remplace une autre du même
/// numéro, de l'autre sorte, et dont l'écriture est interrompue, laisse l'ancienne
/// entière ; achevée, elle seule tient le numéro. Un fichier par numéro : le renommage
/// fait tout le remplacement, et il n'y a jamais d'instant où le numéro n'a plus de case
/// (l'ancienne, déjà dite `Ecrit` à l'autorité, perdue avant que la nouvelle soit sur
/// disque), ni où il en a deux.
#[test]
fn une_case_remplacee_interrompue_garde_l_ancienne_entiere_sal27() {
    let d = Dossier::ouvrir(&dossier("garde-remplacee")).unwrap();
    d.ecrire(SALLE, Case::Point(1), b"p1").unwrap();
    d.ecrire(SALLE, Case::Journal(2), b"j2").unwrap();
    let cases = d.racine().join("ABCD0123").join("cases");
    // L'écriture de la case 2 s'interrompt : son fichier temporaire ne peut pas naître.
    let bouchon = cases.join(".00000000000000000002.tmp");
    std::fs::create_dir(&bouchon).unwrap();
    assert!(d.ecrire(SALLE, Case::Point(2), b"p2").is_err());
    assert_eq!(
        d.faire(
            SALLE,
            &OpGarde::Ecrire {
                case: Case::Point(2),
                octets: b"p2".to_vec()
            }
        ),
        Fait::Refus(RefusRelais::GardePleine)
    );
    assert_eq!(
        d.lire(SALLE, Case::Journal(2)).unwrap().as_deref(),
        Some(&b"j2"[..])
    );
    std::fs::remove_dir(&bouchon).unwrap();
    assert_eq!(
        d.lister(SALLE).unwrap(),
        vec![Case::Point(1), Case::Journal(2)]
    );
    // Achevée, elle seule tient le numéro.
    d.ecrire(SALLE, Case::Point(2), b"p2").unwrap();
    assert_eq!(
        d.lister(SALLE).unwrap(),
        vec![Case::Point(1), Case::Point(2)]
    );
    assert_eq!(d.lire(SALLE, Case::Journal(2)).unwrap(), None);
    assert_eq!(
        d.lire(SALLE, Case::Point(2)).unwrap().as_deref(),
        Some(&b"p2"[..])
    );
    let noms: Vec<String> = std::fs::read_dir(&cases)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        noms.iter()
            .filter(|n| n.starts_with("00000000000000000002"))
            .count(),
        1,
        "{noms:?}"
    );
}

/// SAL-26 sur la vraie coque : `Ecrit` ne part qu'une fois la case dans son fichier, aux
/// octets près ; puis `Lister`, `Lire` et `Effacer`.
#[test]
fn ecrit_ne_part_qu_une_fois_la_case_sur_disque_sal26() {
    let r = reglages("ecrit", Banc::Oui);
    let racine = r.garde.clone();
    let relais = coque::lancer(&r).unwrap();
    let (mut _a, _, mut g) = autorite(relais.port(), PREUVE);
    let d = Dossier::ouvrir(&racine).unwrap();
    for (case, o) in [
        (Case::Point(0), &b"point zero"[..]),
        (Case::Journal(1), b"journal un"),
        (Case::Point(2), b"point deux"),
    ] {
        g.message(&VersRelais::Ecrire { case, octets: o });
        assert_eq!(g.du_relais(UN_PEU), Some(DuRelais::Ecrit { case }));
        assert_eq!(d.lire(SALLE, case).unwrap().as_deref(), Some(o));
    }
    g.message(&VersRelais::Lister);
    assert_eq!(
        g.du_relais(UN_PEU),
        Some(DuRelais::Cases(vec![
            Case::Point(0),
            Case::Journal(1),
            Case::Point(2)
        ]))
    );
    g.message(&VersRelais::Lire {
        case: Case::Journal(1),
    });
    assert_eq!(
        g.du_relais(UN_PEU),
        Some(DuRelais::Lu {
            case: Case::Journal(1),
            octets: b"journal un"
        })
    );
    g.message(&VersRelais::Effacer { avant: 2 });
    g.message(&VersRelais::Lister);
    assert_eq!(
        g.du_relais(UN_PEU),
        Some(DuRelais::Cases(vec![Case::Point(2)]))
    );
    relais.arreter();
}

/// Le lien de la garde a sa propre borne (PHASE-3, § 2.4, lot 5) : une case de point de
/// Tier list au pire mesuré (16,8 Mio, M3) passe, au-delà de `TRAME_MAX` ; les autres
/// liens gardent la leur.
#[test]
fn une_case_plus_grande_qu_une_trame_passe_par_la_garde() {
    let r = reglages("grosse-case", Banc::Oui);
    let relais = coque::lancer(&r).unwrap();
    let (mut a, _, mut g) = autorite(relais.port(), PREUVE);
    let gros = vec![0xc3; 17_625_193];
    g.message(&VersRelais::Ecrire {
        case: Case::Point(0),
        octets: &gros,
    });
    assert_eq!(
        g.du_relais(Duration::from_secs(20)),
        Some(DuRelais::Ecrit {
            case: Case::Point(0)
        })
    );
    g.message(&VersRelais::Lire {
        case: Case::Point(0),
    });
    assert!(
        matches!(g.du_relais(Duration::from_secs(20)), Some(DuRelais::Lu { octets, .. }) if octets.len() == gros.len())
    );
    // Sur le lien de l'autorité, la même taille ferme le lien (le WebSocket la refuse).
    a.message(&VersRelais::Echo { t_envoi_us: 1 });
    assert!(matches!(a.du_relais(UN_PEU), Some(DuRelais::Echo { .. })));
    a.message(&VersRelais::Ecrire {
        case: Case::Point(1),
        octets: &gros,
    });
    assert!(a.attendre_la_fin(Duration::from_secs(10)));
    relais.arreter();
}

/// Un relais relancé garde le code de sa salle (`GET /salle`), la preuve de son autorité
/// et sa garde (P14, P41) : la même autorité revient, une autre preuve est refusée.
#[test]
fn un_relais_relance_garde_sa_salle_sa_preuve_et_sa_garde() {
    let r = reglages("relance", Banc::Oui);
    let relais = coque::lancer(&r).unwrap();
    let (_a, _, mut g) = autorite(relais.port(), PREUVE);
    g.message(&VersRelais::Ecrire {
        case: Case::Point(0),
        octets: b"p0",
    });
    assert!(g.du_relais(UN_PEU).is_some());
    relais.arreter();
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let (statut, texte) = http(
        port,
        &format!("GET /salle HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(texte.ends_with("ABCD0123"), "{texte}");
    let mut x = client(port);
    x.bonjour(SALLE, Role::Autorite { preuve: [4; 32] });
    assert_eq!(
        x.du_relais(UN_PEU),
        Some(DuRelais::Refus(RefusRelais::PreuveFausse))
    );
    let (_a, _, mut g) = autorite(port, PREUVE);
    g.message(&VersRelais::Lister);
    assert_eq!(
        g.du_relais(UN_PEU),
        Some(DuRelais::Cases(vec![Case::Point(0)]))
    );
    relais.arreter();
}

/// P41 : `soiree-relais oublier <salle>`, tapé pendant que le relais tourne, efface la
/// preuve et la garde de la salle ; l'autorité suivante enregistre la sienne sous le même
/// code, et trouve une garde vide.
#[test]
fn oublier_efface_la_preuve_et_la_garde_et_garde_le_code_p41() {
    let r = reglages("oublier", Banc::Oui);
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let (_a, _, mut g) = autorite(port, PREUVE);
    g.message(&VersRelais::Ecrire {
        case: Case::Point(0),
        octets: b"p0",
    });
    assert!(g.du_relais(UN_PEU).is_some());
    let mut x = client(port);
    x.bonjour(SALLE, Role::Autorite { preuve: [4; 32] });
    assert_eq!(
        x.du_relais(UN_PEU),
        Some(DuRelais::Refus(RefusRelais::PreuveFausse))
    );
    assert!(Dossier::ouvrir(&r.garde).unwrap().oublier(SALLE).unwrap());
    // Le code reste celui du relais, avant même que l'autorité suivante ne s'enregistre
    // (qui le réécrirait).
    let (statut, texte) = http(
        port,
        &format!("GET /salle HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(texte.ends_with("ABCD0123"), "{texte}");
    let (_b, _, mut g2) = autorite(port, [4; 32]);
    g2.message(&VersRelais::Lister);
    assert_eq!(g2.du_relais(UN_PEU), Some(DuRelais::Cases(Vec::new())));
    let (statut, texte) = http(
        port,
        &format!("GET /salle HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(texte.ends_with("ABCD0123"), "{texte}");
    relais.arreter();
}

/// L'enregistrement note tout ce que le relais voit, trame par trame, dans les deux sens.
#[test]
fn l_enregistrement_note_tout_ce_que_le_relais_voit() {
    let mut r = reglages("enregistrement", Banc::Oui);
    let chemin = r.garde.join("enregistrement.bin");
    r.enregistrer = Some(chemin.clone());
    let relais = coque::lancer(&r).unwrap();
    let (mut a, _, _g) = autorite(relais.port(), PREUVE);
    a.message(&VersRelais::Echo { t_envoi_us: 77 });
    assert!(a.du_relais(UN_PEU).is_some());
    relais.arreter();
    let (notes, complet) = enregistrement::lire(&std::fs::read(&chemin).unwrap());
    assert!(complet);
    let bonjour = VersRelais::Bonjour {
        protocole: liaison::PROTOCOLE_RELAIS,
        salle: SALLE,
        role: Role::Autorite { preuve: PREUVE },
    }
    .encoder()
    .unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.sens == Sens::Recu && n.octets == bonjour)
    );
    let echo = VersRelais::Echo { t_envoi_us: 77 }.encoder().unwrap();
    assert!(
        notes
            .iter()
            .any(|n| n.sens == Sens::Recu && n.octets == echo)
    );
    assert!(notes.iter().any(|n| n.sens == Sens::Envoye
        && matches!(DuRelais::decoder(&n.octets), Ok(DuRelais::Accueil { .. }))));
    assert!(notes.iter().filter(|n| n.sens == Sens::Ouvert).count() >= 2);
    // Une note coupée à la fin (un `kill -9`) se lit jusqu'à elle.
    let o = std::fs::read(&chemin).unwrap();
    let (coupees, complet) = enregistrement::lire(&o[..o.len() - 3]);
    assert!(!complet);
    assert_eq!(coupees.len(), notes.len() - 1);
}
