//! La coque du relais sur de vraies connexions de la boucle locale : les deux familles,
//! sans réseau, l'adresse jointe, deux fils par lien, la file pleine, le refus qui ferme,
//! le silence.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::time::{Duration, Instant};

use liaison::{
    AdresseIp, CauseFermeture, Classe, CodeSalle, DuRelais, RefusRelais, Role, VersRelais,
};
use relais::coque;
use relais::http::Banc;

use crate::commun::{Client, Recu, client, http, ouvrir, reglages, relais};

const SALLE: CodeSalle = CodeSalle([0xab, 0xcd, 0x01, 0x23]);
const PREUVE: [u8; 32] = [3; 32];
const UN_PEU: Duration = Duration::from_secs(5);

/// Une autorité et sa garde, accueillies, sur la boucle locale.
pub fn autorite(port: u16, preuve: [u8; 32]) -> (Client, liaison::Lien, Client) {
    let mut a = client(port);
    a.bonjour(SALLE, Role::Autorite { preuve });
    let Some(DuRelais::Accueil { lien, .. }) = a.du_relais(UN_PEU) else {
        panic!("l'autorité n'est pas accueillie");
    };
    let mut g = client(port);
    g.bonjour(
        SALLE,
        Role::Garde {
            preuve,
            autorite: lien,
        },
    );
    assert!(matches!(
        g.du_relais(UN_PEU),
        Some(DuRelais::Accueil { .. })
    ));
    (a, lien, g)
}

/// Un appareil accueilli, et son lien.
fn appareil(mut c: Client) -> (Client, liaison::Lien) {
    c.bonjour(SALLE, Role::Appareil);
    let Some(DuRelais::Accueil { lien, .. }) = c.du_relais(UN_PEU) else {
        panic!("l'appareil n'est pas accueilli");
    };
    (c, lien)
}

/// Le prochain message de l'autorité qui n'est ni un écho ni un battement.
fn a_l_autorite(a: &mut Client) -> Option<DuRelais<'static>> {
    a.du_relais(UN_PEU)
}

/// PHASE-3, § 2.5 : si le port est pris dans l'une des deux familles de la boucle locale,
/// le relais refuse de démarrer, haut et clair ; un autre programme sur `[::1]` capterait
/// sinon `localhost`.
#[test]
fn un_port_pris_dans_une_famille_empeche_le_demarrage() {
    let pris = TcpListener::bind(SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), 0)).unwrap();
    let port = pris.local_addr().unwrap().port();
    let mut r = reglages("famille-v6", Banc::Oui);
    r.port = port;
    let e = coque::lancer(&r).err().expect("le relais a démarré");
    assert!(e.0.contains("[::1]"), "{e}");
    let pris = TcpListener::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0)).unwrap();
    let port = pris.local_addr().unwrap().port();
    let mut r = reglages("famille-v4", Banc::Oui);
    r.port = port;
    let e = coque::lancer(&r).err().expect("le relais a démarré");
    assert!(e.0.contains("127.0.0.1"), "{e}");
}

/// SAL-76 : sans adresse sur le réseau local, le relais démarre en boucle locale
/// seulement, et n'annonce rien ; l'écran se sert.
#[test]
fn sans_reseau_le_relais_demarre_en_boucle_locale_sal76() {
    let r = relais("sans-reseau");
    assert_eq!(r.annoncees(), &[] as &[std::net::IpAddr]);
    assert_eq!(r.ecoutes().len(), 2, "{:?}", r.ecoutes());
    let port = r.port();
    let (statut, texte) = http(
        port,
        &format!("GET / HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200, "{texte}");
    assert!(texte.contains("Content-Security-Policy: default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; img-src 'self' blob:; connect-src 'self'"));
    assert!(texte.contains("Cross-Origin-Opener-Policy: same-origin"));
    assert!(texte.contains("Cache-Control: no-store"));
    let mut a = client(port);
    a.bonjour(SALLE, Role::Autorite { preuve: PREUVE });
    assert!(
        matches!(a.du_relais(UN_PEU), Some(DuRelais::Accueil { adresses, .. }) if adresses.is_empty())
    );
    r.arreter();
}

/// SAL-77 : une adresse qu'on n'écoute pas se refuse à voix haute ; une adresse de la
/// boucle locale ne se donne pas aux téléphones.
#[test]
fn une_adresse_qu_on_n_ecoute_pas_se_refuse_a_voix_haute_sal77() {
    let mut r = reglages("adresse-absente", Banc::Oui);
    r.adresse = Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 9)));
    let e = coque::lancer(&r).err().expect("le relais a démarré");
    assert!(e.0.contains("SAL-77") && e.0.contains("203.0.113.9"), "{e}");
    r.adresse = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
    let e = coque::lancer(&r).err().expect("le relais a démarré");
    assert!(e.0.contains("ne peuvent pas joindre"), "{e}");
}

/// SAL-75 : l'adresse qu'apprend l'autorité est celle que la connexion de l'appareil a
/// jointe, famille comprise.
#[test]
fn l_adresse_jointe_est_celle_de_la_connexion_sal75() {
    let r = relais("jointe");
    let port = r.port();
    let (mut a, _, _g) = autorite(port, PREUVE);
    let (_d4, l4) = appareil(client(port));
    let hote6 = format!("[::1]:{port}");
    let c6 = ouvrir(
        SocketAddr::new(IpAddr::V6(Ipv6Addr::LOCALHOST), port),
        &[("Host", &hote6)],
    )
    .unwrap();
    let (_d6, l6) = appareil(c6);
    let mut vus = Vec::new();
    while vus.len() < 2 {
        match a_l_autorite(&mut a) {
            Some(DuRelais::Ouvert {
                lien,
                adresse_jointe,
            }) => vus.push((lien, adresse_jointe)),
            Some(_) => {}
            None => panic!("pas d'Ouvert : {vus:?}"),
        }
    }
    let mut v6 = [0u8; 16];
    v6[15] = 1;
    assert!(
        vus.contains(&(l4, AdresseIp::V4([127, 0, 0, 1]))),
        "{vus:?}"
    );
    assert!(vus.contains(&(l6, AdresseIp::V6(v6))), "{vus:?}");
    r.arreter();
}

/// SAL-146 : un envoi du relais n'attend pas une lecture. Le téléphone ne dit plus rien
/// (et ne rend pas même les battements) : la trame de l'autorité lui arrive quand même,
/// tout de suite.
#[test]
fn un_envoi_n_attend_pas_une_lecture_sal146() {
    let r = relais("deux-fils");
    let port = r.port();
    let (mut a, _, _g) = autorite(port, PREUVE);
    let (mut d, ld) = appareil(client(port));
    d.repond = false;
    assert_eq!(d.du_relais(UN_PEU), Some(DuRelais::AutoritePresente));
    assert!(matches!(
        a_l_autorite(&mut a),
        Some(DuRelais::Ouvert { .. })
    ));
    // Le fil de lecture du téléphone attend ; rien ne vient de lui pendant une seconde.
    std::thread::sleep(Duration::from_millis(800));
    let depart = Instant::now();
    a.message(&VersRelais::Vers {
        lien: ld,
        classe: Classe::Fiable,
        trame: b"une vue",
    });
    let m = d.du_relais(Duration::from_secs(2));
    let duree = depart.elapsed();
    assert_eq!(
        m,
        Some(DuRelais::Pour {
            classe: Classe::Fiable,
            trame: b"une vue"
        })
    );
    // Une borne large, pas une mesure : un relais qui attendrait une lecture ne livrerait
    // rien avant la fermeture du silence (3 s).
    assert!(duree < Duration::from_millis(1500), "{duree:?}");
    r.arreter();
}

/// SAL-06 et SAL-07 : la file d'un appareil qui ne lit plus est bornée ; pleine, elle
/// ferme le lien, et l'autorité l'apprend (`FilePleine`) : rien ne se perd en silence, et
/// le téléphone reviendra recevoir l'état entier.
#[test]
fn une_file_pleine_ferme_son_lien_sal06_sal07() {
    let r = relais("file-pleine");
    let port = r.port();
    let (mut a, _, _g) = autorite(port, PREUVE);
    let (mut d, ld) = appareil(client(port));
    assert_eq!(d.du_relais(UN_PEU), Some(DuRelais::AutoritePresente));
    assert!(matches!(
        a_l_autorite(&mut a),
        Some(DuRelais::Ouvert { .. })
    ));
    // Le téléphone ne lit plus. L'autorité lui envoie 32 Mio.
    let trame = vec![0x5a; 200 * 1024];
    for _ in 0..160 {
        a.message(&VersRelais::Vers {
            lien: ld,
            classe: Classe::Fiable,
            trame: &trame,
        });
    }
    let fin = Instant::now() + Duration::from_secs(20);
    let mut encombre = false;
    loop {
        assert!(Instant::now() < fin, "aucune fermeture en 20 s");
        match a_l_autorite(&mut a) {
            Some(DuRelais::Encombre { lien, .. }) if lien == ld => encombre = true,
            Some(DuRelais::Ferme { lien, cause }) if lien == ld => {
                assert_eq!(cause, CauseFermeture::FilePleine);
                break;
            }
            Some(_) | None => {}
        }
    }
    assert!(
        encombre,
        "l'autorité n'a pas appris que la file se remplissait"
    );
    r.arreter();
}

/// SAL-148 : un refus ferme sa connexion ; le relais ne garde rien d'un pair refusé.
#[test]
fn un_refus_ferme_sa_connexion_sal148() {
    let r = relais("refus");
    let port = r.port();
    let mut d = client(port);
    d.bonjour(SALLE, Role::Appareil);
    assert_eq!(
        d.du_relais(UN_PEU),
        Some(DuRelais::Refus(RefusRelais::SalleInconnue))
    );
    // Fermée par le refus lui-même, tout de suite : bien avant que le silence (3 s) ne
    // ferme un lien qui ne lit plus.
    assert!(
        d.attendre_la_fin(Duration::from_millis(1500)),
        "la connexion refusée reste ouverte"
    );
    r.arreter();
}

/// SAL-43 : le relais bat sur chaque lien ; un lien qui ne rend pas ses battements est
/// fermé après `SILENCE_MAX_MS`, et l'autorité l'apprend ; un lien qui les rend reste.
#[test]
fn le_silence_ferme_un_lien_qui_ne_rend_pas_ses_battements_sal43() {
    let r = relais("silence");
    let port = r.port();
    let (mut a, _, mut g) = autorite(port, PREUVE);
    let (mut muet, lm) = appareil(client(port));
    muet.repond = false;
    let (mut vivant, lv) = appareil(client(port));
    let depart = Instant::now();
    let mut battements = 0;
    let mut ferme = None;
    while depart.elapsed() < Duration::from_millis(4500) {
        if vivant.recevoir(Duration::from_millis(20)) == Recu::Ping {
            battements += 1;
        }
        let _: Recu = muet.recevoir(Duration::from_millis(1));
        let _: Recu = g.recevoir(Duration::from_millis(1));
        if let Recu::Binaire(o) = a.recevoir(Duration::from_millis(20))
            && let Ok(DuRelais::Ferme { lien, cause }) = DuRelais::decoder(&o)
        {
            assert_eq!(lien, lm, "le lien vivant a été fermé");
            assert_eq!(cause, CauseFermeture::Muet);
            ferme = Some(depart.elapsed());
        }
    }
    let ferme = ferme.expect("le lien muet n'a pas été fermé");
    assert!(
        ferme >= Duration::from_millis(2900),
        "fermé trop tôt : {ferme:?}"
    );
    // De l'ordre de quatre battements par seconde (250 ms), en 4,5 s.
    assert!((12..=20).contains(&battements), "{battements} battements");
    let _: liaison::Lien = lv;
    r.arreter();
}
