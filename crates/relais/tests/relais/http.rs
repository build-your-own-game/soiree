//! Qui parle au relais (SAL-33), ses routes (SAL-150), ses places (SAL-143), ses adresses
//! (SAL-76, SAL-77).

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use liaison::{CodeSalle, DuRelais, RefusRelais, Role};
use relais::admission::{
    Admission, CONNEXIONS_MAX, NOUVELLES_PAR_SECONDE, Pair, RAFALE, RESERVE_BOUCLE, RefusAdmission,
};
use relais::adresses::{Interface, choisir};
use relais::coeur::Provenance;
use relais::coque;
use relais::http::{
    Banc, Contexte, Jugement, Requete, Route, boucle_locale_permise, juger_websocket, router,
};

use crate::commun::{dossier, http, ouvrir, reglages};

const SALLE: CodeSalle = CodeSalle([0xab, 0xcd, 0x01, 0x23]);
const UN_PEU: Duration = Duration::from_secs(5);

fn requete(methode: &str, chemin: &str, entetes: &[(&str, &str)]) -> Requete {
    Requete {
        methode: methode.to_owned(),
        chemin: chemin.to_owned(),
        entetes: entetes
            .iter()
            .map(|(n, v)| (n.to_ascii_lowercase(), (*v).to_owned()))
            .collect(),
    }
}

fn contexte(banc: Banc) -> Contexte {
    Contexte {
        port: 7878,
        annoncees: vec![IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))],
        banc,
    }
}

const BOUCLE: IpAddr = IpAddr::V4(Ipv4Addr::LOCALHOST);
const TELEPHONE: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 31));

/// SAL-33, jugé hors de toute connexion : seule l'écran (`Origin`
/// `http://localhost:7878`, depuis la boucle, d'un `Host` local) tient tous les rôles ; un
/// site étranger, ou un rebond DNS, n'ouvre rien ; une page du relais n'est qu'un
/// appareil ; un en-tête de mandataire ferme tout.
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "une table de cas, une ligne par requête"
)]
fn qui_parle_au_relais_se_juge_sur_l_adresse_le_host_et_l_origin_sal33() {
    let c = contexte(Banc::Non);
    let ecran = [
        ("Host", "localhost:7878"),
        ("Origin", "http://localhost:7878"),
    ];
    assert_eq!(
        juger_websocket(BOUCLE, &requete("GET", "/ws", &ecran), &c),
        Jugement::Admise(Provenance::BoucleLocale)
    );
    for (pair, entetes, attendu) in [
        // Un site étranger ouvert sur l'ordinateur de la soirée.
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "https://evil.example"),
            ],
            None,
        ),
        (
            BOUCLE,
            vec![("Host", "127.0.0.1:7878"), ("Origin", "null")],
            None,
        ),
        // Sans Origin, hors du banc.
        (BOUCLE, vec![("Host", "localhost:7878")], None),
        // Un rebond DNS : un nom étranger qui mène à la boucle locale.
        (
            BOUCLE,
            vec![
                ("Host", "evil.example:7878"),
                ("Origin", "http://evil.example:7878"),
            ],
            None,
        ),
        (
            BOUCLE,
            vec![
                ("Host", "evil.example:7878"),
                ("Origin", "http://localhost:7878"),
            ],
            None,
        ),
        // Un mandataire.
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://localhost:7878"),
                ("X-Forwarded-For", "1.2.3.4"),
            ],
            None,
        ),
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://localhost:7878"),
                ("Via", "1.1 proxy"),
            ],
            None,
        ),
        // Deux Origin : on ne choisit pas.
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://localhost:7878"),
                ("Origin", "http://evil.example"),
            ],
            None,
        ),
        // Une page du relais : un appareil seulement.
        (
            BOUCLE,
            vec![
                ("Host", "127.0.0.1:7878"),
                ("Origin", "http://127.0.0.1:7878"),
            ],
            Some(Provenance::Reseau),
        ),
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://192.168.1.20:7878"),
            ],
            Some(Provenance::Reseau),
        ),
        // Un téléphone du Wi-Fi, par l'adresse annoncée.
        (
            TELEPHONE,
            vec![
                ("Host", "192.168.1.20:7878"),
                ("Origin", "http://192.168.1.20:7878"),
            ],
            Some(Provenance::Reseau),
        ),
        // Un téléphone qui se dit l'écran : son adresse le trahit.
        (
            TELEPHONE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://localhost:7878"),
            ],
            None,
        ),
        (
            TELEPHONE,
            vec![
                ("Host", "192.168.1.20:7878"),
                ("Origin", "http://localhost:7878"),
            ],
            None,
        ),
        (TELEPHONE, vec![("Host", "192.168.1.20:7878")], None),
    ] {
        let j = juger_websocket(pair, &requete("GET", "/ws", &entetes), &c);
        match attendu {
            Some(p) => assert_eq!(j, Jugement::Admise(p), "{pair} {entetes:?}"),
            None => assert!(
                matches!(j, Jugement::Etrangere(_)),
                "{pair} {entetes:?} : {j:?}"
            ),
        }
    }
    // Le banc seul ouvre sans Origin, et seulement depuis la boucle.
    let banc = contexte(Banc::Oui);
    assert_eq!(
        juger_websocket(
            BOUCLE,
            &requete("GET", "/ws", &[("Host", "127.0.0.1:7878")]),
            &banc
        ),
        Jugement::Admise(Provenance::BoucleLocale)
    );
    assert!(matches!(
        juger_websocket(
            TELEPHONE,
            &requete("GET", "/ws", &[("Host", "192.168.1.20:7878")]),
            &banc
        ),
        Jugement::Etrangere(_)
    ));
    // Les routes de la boucle locale.
    assert!(boucle_locale_permise(
        BOUCLE,
        &requete("GET", "/", &[("Host", "localhost:7878")]),
        &c
    ));
    for (pair, entetes) in [
        (BOUCLE, vec![("Host", "evil.example:7878")]),
        (
            BOUCLE,
            vec![
                ("Host", "localhost:7878"),
                ("Origin", "http://evil.example"),
            ],
        ),
        (
            BOUCLE,
            vec![("Host", "localhost:7878"), ("Forwarded", "for=1.2.3.4")],
        ),
        (TELEPHONE, vec![("Host", "localhost:7878")]),
    ] {
        assert!(
            !boucle_locale_permise(pair, &requete("GET", "/salle", &entetes), &c),
            "{pair} {entetes:?}"
        );
    }
}

/// SAL-33 sur la vraie coque, sans `--banc` : un site étranger ouvert dans le navigateur
/// de l'ordinateur ne prend pas la salle (`403` avant tout WebSocket) ; une page du
/// relais n'est qu'un appareil (`HorsBoucleLocale`) ; l'écran la prend.
#[test]
fn un_site_etranger_ne_prend_pas_la_salle_sal33() {
    let relais = coque::lancer(&reglages("site-etranger", Banc::Non)).unwrap();
    let port = relais.port();
    let adresse = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let local = format!("localhost:{port}");
    let ecran = format!("http://localhost:{port}");
    let page = format!("http://127.0.0.1:{port}");
    for origine in ["https://evil.example", "http://localhost:1", "null"] {
        assert_eq!(
            ouvrir(adresse, &[("Host", &local), ("Origin", origine)]).err(),
            Some(403),
            "{origine}"
        );
    }
    assert_eq!(
        ouvrir(adresse, &[("Host", &local)]).err(),
        Some(403),
        "sans Origin"
    );
    let mut p = ouvrir(adresse, &[("Host", &local), ("Origin", &page)]).unwrap();
    p.bonjour(SALLE, Role::Autorite { preuve: [1; 32] });
    assert_eq!(
        p.du_relais(UN_PEU),
        Some(DuRelais::Refus(RefusRelais::HorsBoucleLocale))
    );
    let mut e = ouvrir(adresse, &[("Host", &local), ("Origin", &ecran)]).unwrap();
    e.bonjour(SALLE, Role::Autorite { preuve: [1; 32] });
    assert!(matches!(
        e.du_relais(UN_PEU),
        Some(DuRelais::Accueil { .. })
    ));
    relais.arreter();
}

/// SAL-33 : un rebond DNS (un nom étranger qui résout vers la boucle locale) n'atteint ni
/// le WebSocket, ni la page de l'écran, ni `GET /salle`.
#[test]
fn un_rebond_dns_est_refuse_sal33() {
    let relais = coque::lancer(&reglages("rebond", Banc::Oui)).unwrap();
    let port = relais.port();
    let adresse = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let etranger = format!("evil.example:{port}");
    let origine = format!("http://evil.example:{port}");
    assert_eq!(
        ouvrir(adresse, &[("Host", &etranger), ("Origin", &origine)]).err(),
        Some(403)
    );
    assert_eq!(ouvrir(adresse, &[("Host", &etranger)]).err(), Some(403));
    for chemin in ["/", "/salle", "/s/ABCD0123"] {
        let (statut, _) = http(
            port,
            &format!("GET {chemin} HTTP/1.1\r\nHost: {etranger}\r\n\r\n"),
        );
        assert_eq!(statut, 403, "{chemin}");
    }
    let (statut, _) = http(
        port,
        &format!(
            "GET /salle HTTP/1.1\r\nHost: localhost:{port}\r\nX-Forwarded-For: 10.0.0.9\r\n\r\n"
        ),
    );
    assert_eq!(statut, 403);
    relais.arreter();
}

/// SAL-150 : le relais ne sert jamais un fichier hors de sa table ; un chemin se compare à
/// des noms, jamais au disque.
#[test]
fn aucun_fichier_hors_de_la_table_n_est_servi_sal150() {
    for chemin in [
        "/../Cargo.toml",
        "/web/../../Cargo.toml",
        "/%2e%2e/%2e%2e/etc/passwd",
        "/s/../../etc/passwd",
        "/s/abcd0123",
        "/s/ABCD0123/../..",
        "/salle/../preuve",
        "//etc/passwd",
        "/ABCD0123/preuve",
    ] {
        let r = router(&requete("GET", chemin, &[]));
        assert!(
            matches!(r, Route::Introuvable | Route::Web(_)),
            "{chemin} : {r:?}"
        );
    }
    assert_eq!(
        router(&requete("GET", "/?rendu=webgl2", &[])),
        Route::Demarrage
    );
    assert_eq!(router(&requete("POST", "/", &[])), Route::Methode);
    assert_eq!(
        router(&requete("GET", "/s/ABCD0123", &[])),
        Route::Telephone(SALLE)
    );
    // Sur la vraie coque : la garde (sa preuve) est dans le dossier, et rien ne la sert.
    let r = reglages("table", Banc::Oui);
    let racine = r.garde.clone();
    relais::garde::Dossier::ouvrir(&racine)
        .unwrap()
        .enregistrer_preuve(SALLE, [0x41; 32])
        .unwrap();
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    for chemin in [
        "/ABCD0123/preuve",
        "/web/ABCD0123/preuve",
        "/web/../ABCD0123/preuve",
        "/salle/../ABCD0123/preuve",
    ] {
        let (statut, texte) = http(
            port,
            &format!("GET {chemin} HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
        );
        assert_eq!(statut, 404, "{chemin}");
        assert!(!texte.contains("AAAA"), "{chemin} : {texte}");
    }
    relais.arreter();
}

/// SAL-143 : une rafale de connexions venues du réseau n'empêche pas l'ordinateur de la
/// soirée de joindre son propre relais ; SAL-78 : le réseau est limité par seconde.
#[test]
fn la_boucle_locale_garde_ses_places_sal143() {
    let mut a = Admission::new();
    let mut t = 0;
    let mut reseau = 0;
    while reseau < CONNEXIONS_MAX {
        t += 1_000_000 / NOUVELLES_PAR_SECONDE;
        if a.admettre(t, Pair::Reseau).is_err() {
            break;
        }
        reseau += 1;
    }
    assert_eq!(reseau, CONNEXIONS_MAX - RESERVE_BOUCLE);
    assert_eq!(
        a.admettre(t + 1_000_000, Pair::Reseau),
        Err(RefusAdmission::Pleine)
    );
    for _ in 0..RESERVE_BOUCLE {
        assert_eq!(a.admettre(t, Pair::Boucle), Ok(()));
    }
    assert_eq!(a.admettre(t, Pair::Boucle), Err(RefusAdmission::Pleine));
    // Une place du réseau qui se libère ne lui revient pas tant que la boucle tient la
    // réserve.
    a.liberer(Pair::Reseau);
    assert_eq!(
        a.admettre(t, Pair::Reseau),
        Err(RefusAdmission::Pleine),
        "la réserve"
    );
    for _ in 0..RESERVE_BOUCLE {
        a.liberer(Pair::Boucle);
    }
    assert_eq!(a.admettre(t + 1_000_000, Pair::Reseau), Ok(()));
    // La rafale, puis une par seconde au rythme.
    let mut b = Admission::new();
    let admises = (0..RAFALE + 10)
        .filter(|_| b.admettre(5, Pair::Reseau).is_ok())
        .count();
    assert_eq!(admises as u64, RAFALE);
    assert_eq!(b.admettre(5, Pair::Reseau), Err(RefusAdmission::TropVite));
    assert_eq!(
        b.admettre(5 + 1_000_000 / NOUVELLES_PAR_SECONDE + 1, Pair::Reseau),
        Ok(())
    );
    assert_eq!(
        b.admettre(5, Pair::Boucle),
        Ok(()),
        "la boucle n'est pas limitée"
    );
}

fn interface(nom: &str, ip: &str) -> Interface {
    Interface {
        nom: nom.to_owned(),
        ip: ip.parse().unwrap(),
    }
}

/// SAL-76 : la route par défaut est une supposition. Le relais annonce chaque adresse
/// joignable de ses interfaces, IPv4 d'abord, sans la boucle ni les liens locaux ; sans
/// aucune, il n'annonce rien et démarre quand même.
#[test]
fn les_adresses_annoncees_sont_celles_des_interfaces_sal76() {
    let toutes = [
        interface("lo0", "127.0.0.1"),
        interface("lo0", "::1"),
        interface("en0", "fe80::1"),
        interface("en0", "2a01:e0a:1::5"),
        interface("en0", "192.168.1.20"),
        interface("utun0", "10.8.0.2"),
        interface("en5", "169.254.3.3"),
        interface("en0", "192.168.1.20"),
    ];
    let a: Vec<String> = choisir(&toutes, None)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(a, ["10.8.0.2", "192.168.1.20", "2a01:e0a:1::5"]);
    assert_eq!(choisir(&toutes[..3], None), Ok(Vec::new()));
}

/// SAL-77 : `--adresse` nomme une adresse d'une interface, joignable, ou le relais ne
/// démarre pas, et dit pourquoi.
#[test]
fn une_adresse_imposee_doit_etre_celle_d_une_interface_sal77() {
    let toutes = [
        interface("lo0", "127.0.0.1"),
        interface("en0", "192.168.1.20"),
    ];
    let ip: IpAddr = "192.168.1.20".parse().unwrap();
    assert_eq!(choisir(&toutes, Some(ip)), Ok(vec![ip]));
    let e = choisir(&toutes, Some("192.168.1.21".parse().unwrap())).unwrap_err();
    assert!(e.contains("aucune interface"), "{e}");
    let e = choisir(&toutes, Some("2001:db8::1".parse().unwrap())).unwrap_err();
    assert!(e.contains("aucune interface"), "{e}");
    let e = choisir(&toutes, Some("127.0.0.1".parse().unwrap())).unwrap_err();
    assert!(e.contains("ne peuvent pas joindre"), "{e}");
}

/// Un paquet et un catalogue dans des dossiers neufs : `<empreinte>/demarrage.js` et un
/// `.wasm`, et un module livré sous son empreinte.
fn paquet_et_jeux(nom: &str) -> (relais::paquet::Paquet, relais::paquet::Jeux, String) {
    let d = dossier(nom);
    let p = d.join("0123456789abcdef");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("demarrage.js"), b"// demarrage").unwrap();
    std::fs::write(p.join("ecran_bg.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let j = d.join("jeux");
    std::fs::create_dir_all(&j).unwrap();
    let empreinte = "ab".repeat(32);
    std::fs::write(j.join(format!("{empreinte}.wasm")), b"\0asm module").unwrap();
    (
        relais::paquet::Paquet::lire(&p).unwrap(),
        relais::paquet::Jeux::lire(&j).unwrap(),
        empreinte,
    )
}

/// SAL-150, P16 (lot 10a) : le paquet de la page se sert de sa table, à tous, `immutable`
/// sous son empreinte ; les modules livrés et le catalogue, à la boucle locale seulement ;
/// la page de démarrage charge `demarrage.js` du paquet, sans script en ligne, et dit le
/// banc à la page seulement avec `--banc`. Rien d'autre ne se sert.
#[test]
fn le_paquet_et_les_modules_se_servent_de_la_table_sal150() {
    let (paquet, jeux, empreinte) = paquet_et_jeux("paquet");
    let mut r = reglages("paquet", Banc::Oui);
    r.paquet = Some(paquet.clone());
    r.jeux = jeux.clone();
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let get = |chemin: &str, entetes: &str| {
        http(
            port,
            &format!("GET {chemin} HTTP/1.1\r\nHost: localhost:{port}\r\n{entetes}\r\n"),
        )
    };
    let (statut, page) = get("/", "");
    assert_eq!(statut, 200);
    assert!(
        page.contains(
            "<script type=\"module\" src=\"/web/0123456789abcdef/demarrage.js\"></script>"
        )
    );
    assert!(page.contains("<meta name=\"soiree-banc\" content=\"oui\">"));
    assert!(page.contains("connect-src 'self'"), "{page}");
    assert!(page.contains("Cross-Origin-Embedder-Policy: require-corp"));
    let (statut, js) = get("/web/0123456789abcdef/demarrage.js", "");
    assert_eq!(statut, 200);
    assert!(js.contains("Content-Type: text/javascript"));
    assert!(js.contains("immutable"));
    assert!(js.ends_with("// demarrage"));
    let (statut, wasm) = get("/web/0123456789abcdef/ecran_bg.wasm", "");
    assert_eq!(statut, 200);
    assert!(wasm.contains("Content-Type: application/wasm"));
    for chemin in [
        "/web/fedcba9876543210/demarrage.js",
        "/web/0123456789abcdef/absent.js",
        "/web/0123456789abcdef/../ecran_bg.wasm",
        "/web/0123456789abcdef",
        "/jeux/absent.wasm",
        "/jeux/../Cargo.toml",
    ] {
        assert_eq!(get(chemin, "").0, 404, "{chemin}");
    }
    let (statut, catalogue) = get("/jeux/catalogue", "");
    assert_eq!(statut, 200);
    assert!(catalogue.ends_with(&format!(
        "{{\"jeux\":[{{\"empreinte\":\"{empreinte}\",\"taille\":11,\"eprouve\":false}}]}}"
    )));
    let (statut, module) = get(&format!("/jeux/{empreinte}.wasm"), "");
    assert_eq!(statut, 200);
    assert!(module.ends_with("\0asm module"));
    // La boucle locale seulement, d'un `Host` local, sans `Origin` étrangère (SAL-33).
    assert_eq!(
        get("/jeux/catalogue", "Origin: http://ailleurs.example\r\n").0,
        403
    );
    assert_eq!(
        http(
            port,
            "GET /jeux/catalogue HTTP/1.1\r\nHost: ailleurs.example\r\n\r\n"
        )
        .0,
        403
    );
    // Le paquet, à tous ceux qui parlent d'un `Host` permis ; pas d'un rebond DNS.
    assert_eq!(
        http(
            port,
            "GET /web/0123456789abcdef/demarrage.js HTTP/1.1\r\nHost: ailleurs.example\r\n\r\n"
        )
        .0,
        403
    );
    relais.arreter();
    // Sans le banc, la page ne le dit pas.
    let mut r = reglages("paquet-soiree", Banc::Non);
    r.paquet = Some(paquet);
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let (statut, page) = http(
        port,
        &format!("GET / HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(!page.contains("soiree-banc"));
    relais.arreter();
}

/// Un paquet ou un catalogue mal formé se refuse au lancement, haut et clair.
#[test]
fn un_paquet_ou_un_catalogue_mal_forme_se_refuse() {
    let d = dossier("paquet-refus");
    let p = d.join("pas-une-empreinte");
    std::fs::create_dir_all(&p).unwrap();
    std::fs::write(p.join("demarrage.js"), b"").unwrap();
    assert!(relais::paquet::Paquet::lire(&p).is_err());
    let p = d.join("0123456789abcdef");
    std::fs::create_dir_all(&p).unwrap();
    assert!(
        relais::paquet::Paquet::lire(&p)
            .unwrap_err()
            .contains("demarrage.js")
    );
    std::fs::write(p.join("demarrage.js"), b"").unwrap();
    std::fs::write(p.join("nom avec espace.js"), b"").unwrap();
    assert!(relais::paquet::Paquet::lire(&p).is_err());
    let j = d.join("jeux");
    std::fs::create_dir_all(&j).unwrap();
    std::fs::write(j.join("module.wasm"), b"").unwrap();
    assert!(relais::paquet::Jeux::lire(&j).is_err());
}
