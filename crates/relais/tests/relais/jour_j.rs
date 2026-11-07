//! Le jour J (PHASE-3, § 2.8 ; lot 10b) : le catalogue et ses rapports (P17), l'aide
//! (`GET /aide`, SAL-99, SAL-95), l'adresse que le banc annonce sans l'écouter, et
//! `soiree-relais jouer`, qui refuse un module sans rapport et le joue avec
//! `--non-eprouve`.

use std::io::{BufRead, BufReader};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use relais::coque;
use relais::http::Banc;
use relais::jouer::{Construit, SansRapport, les_jeux, lire_construit};
use relais::paquet::Jeux;

use crate::commun::{dossier, http, reglages};

/// Une adresse de documentation (`192.0.2.0/24`), que le banc annonce sans l'écouter.
const ANNONCEE: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));

/// Un rapport d'épreuve réussie au plan standard, comme `soiree-epreuve --catalogue`
/// l'écrit (l'essentiel).
fn rapport(empreinte: &str) -> String {
    format!(
        "{{\"empreinte\":\"{empreinte}\",\"plan\":{{\"nom\":\"standard\",\"soirees\":2}},\"nom\":\"Devine\",\"code\":0,\"verdict\":\"code 0\"}}"
    )
}

/// P17 (lot 10b) : un module et son rapport d'épreuve se lisent ensemble ; le catalogue
/// dit qui est éprouvé ; le rapport se sert à `/jeux/<empreinte>.json`. Un rapport sans
/// module, ou qui nomme un autre module, est refusé au lancement.
#[test]
fn le_catalogue_dit_quel_module_est_eprouve_p17() {
    let d = dossier("catalogue-rapports");
    let (a, b) = ("ab".repeat(32), "cd".repeat(32));
    std::fs::write(d.join(format!("{a}.wasm")), b"\0asm A").unwrap();
    std::fs::write(d.join(format!("{a}.json")), rapport(&a)).unwrap();
    std::fs::write(d.join(format!("{b}.wasm")), b"\0asm B").unwrap();
    let jeux = Jeux::lire(&d).unwrap();
    assert!(jeux.eprouve(&a));
    assert!(!jeux.eprouve(&b));
    assert_eq!(
        jeux.catalogue(),
        format!(
            "{{\"jeux\":[{{\"empreinte\":\"{a}\",\"taille\":6,\"eprouve\":true}},{{\"empreinte\":\"{b}\",\"taille\":6,\"eprouve\":false}}]}}"
        )
    );
    assert_eq!(
        jeux.servir(&format!("{a}.json")).map(|(_, o)| o),
        Some(rapport(&a).into_bytes())
    );
    assert_eq!(jeux.servir(&format!("{b}.json")), None);
    // Servi à la boucle locale, comme le catalogue.
    let mut r = reglages("catalogue-rapports-garde", Banc::Oui);
    r.jeux = jeux;
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let (statut, texte) = http(
        port,
        &format!("GET /jeux/{a}.json HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(texte.ends_with(&rapport(&a)));
    relais.arreter();
    // Le rapport tel que `soiree-epreuve --catalogue` l'écrit, indenté (`"empreinte": "…"`).
    let indente = |plan: &str, code: &str| {
        format!(
            "{{\n  \"empreinte\": \"{b}\",\n  \"plan\": {{\n    \"nom\": \"{plan}\",\n    \"graine\": \"00\"\n  }},\n  \"nom\": \"standard\",\n  \"code\": {code}\n}}\n"
        )
    };
    std::fs::write(d.join(format!("{b}.json")), indente("standard", "0")).unwrap();
    assert!(Jeux::lire(&d).unwrap().eprouve(&b));
    // Après la vérification du lot 10b (P17) : un code 0 du plan court ou d'un plan réglé,
    // ou un code non nul, n'est pas une épreuve réussie, et est refusé au lancement, même
    // d'un jeu qui s'appellerait « standard ».
    for (plan, code) in [
        ("court", "0"),
        ("réglé", "0"),
        ("standard", "1"),
        ("standard", "03"),
    ] {
        std::fs::write(d.join(format!("{b}.json")), indente(plan, code)).unwrap();
        let refus = Jeux::lire(&d).unwrap_err();
        assert!(refus.contains("plan standard"), "{plan} {code} : {refus}");
    }
    std::fs::write(d.join(format!("{b}.json")), indente("standard", "0")).unwrap();
    // Un rapport qui nomme un autre module, ou sans son module.
    std::fs::write(d.join(format!("{b}.json")), rapport(&a)).unwrap();
    assert!(Jeux::lire(&d).unwrap_err().contains("rapport"));
    std::fs::remove_file(d.join(format!("{b}.json"))).unwrap();
    std::fs::remove_file(d.join(format!("{a}.wasm"))).unwrap();
    assert!(Jeux::lire(&d).unwrap_err().contains("n'a pas son module"));
}

/// P17 : sans rapport, `jouer` refuse, et dit comment éprouver le module ; avec
/// `--non-eprouve`, il le sert, non éprouvé. Un module déjà au catalogue avec son rapport
/// reste éprouvé, s'il a les octets que le catalogue garde.
#[test]
fn jouer_refuse_un_module_sans_rapport_p17() {
    let d = dossier("jouer-choix");
    let catalogue = d.join("catalogue");
    let e = "ef".repeat(32);
    let construit = Construit {
        empreinte: e.clone(),
        chemin: d.join("m.wasm"),
    };
    let refus = les_jeux(
        &catalogue,
        &construit,
        b"\0asm".to_vec(),
        SansRapport::Refuser,
        Path::new("."),
    )
    .unwrap_err();
    assert!(refus.contains("--catalogue"), "{refus}");
    assert!(refus.contains("--non-eprouve"), "{refus}");
    let jeux = les_jeux(
        &catalogue,
        &construit,
        b"\0asm".to_vec(),
        SansRapport::Jouer,
        Path::new("."),
    )
    .unwrap();
    assert!(!jeux.eprouve(&e));
    assert!(jeux.catalogue().contains("\"eprouve\":false"));
    std::fs::create_dir_all(&catalogue).unwrap();
    std::fs::write(catalogue.join(format!("{e}.wasm")), b"\0asm").unwrap();
    std::fs::write(catalogue.join(format!("{e}.json")), rapport(&e)).unwrap();
    let jeux = les_jeux(
        &catalogue,
        &construit,
        b"\0asm".to_vec(),
        SansRapport::Refuser,
        Path::new("."),
    )
    .unwrap();
    assert!(jeux.eprouve(&e));
    // *Après la vérification du lot 10b* : des octets construits qui ne sont pas ceux que
    // le catalogue garde sous cette empreinte sont refusés, même avec `--non-eprouve` (le
    // relais ne hache rien ; une sortie de `--construire` qui mentirait sur l'empreinte ne
    // fait pas passer un autre module pour l'éprouvé).
    for sans_rapport in [SansRapport::Refuser, SansRapport::Jouer] {
        let refus = les_jeux(
            &catalogue,
            &construit,
            b"\0asm autre".to_vec(),
            sans_rapport,
            Path::new("."),
        )
        .unwrap_err();
        assert!(refus.contains("n'a pas les octets"), "{refus}");
    }
    // La sortie de `soiree-epreuve --construire`.
    let c = lire_construit(&format!("Étape 1…\n{e} /tmp/m.wasm\n")).unwrap();
    assert_eq!(c.empreinte, e);
    assert_eq!(c.chemin, PathBuf::from("/tmp/m.wasm"));
    assert!(lire_construit("rien d'utile\n").is_err());
    assert!(lire_construit("").is_err());
}

/// SAL-99, SAL-95 (lot 10b) : `GET /aide`, à la boucle locale seulement, dit le réseau,
/// les pages de téléphone sans lien et le pare-feu. L'adresse que le banc annonce sans
/// l'écouter n'existe qu'au banc ; une page demandée d'un `Host` annoncé y compte comme
/// venue du réseau.
#[test]
fn l_aide_se_lit_a_la_boucle_locale_sal99() {
    let mut r = reglages("aide", Banc::Oui);
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let (statut, texte) = http(
        port,
        &format!("GET /aide HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    assert!(texte.contains("\"reseau\":false"), "{texte}");
    assert!(texte.contains("\"pages_sans_lien\":0"), "{texte}");
    assert!(texte.contains("\"pare_feu\":\""), "{texte}");
    // Pas d'un site étranger (SAL-33).
    assert_eq!(
        http(
            port,
            &format!(
                "GET /aide HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: http://ailleurs.example\r\n\r\n"
            )
        )
        .0,
        403
    );
    relais.arreter();
    // Le banc annonce une adresse sans l'écouter.
    r = reglages("aide-annonce", Banc::Oui);
    r.annonce_du_banc = Some(ANNONCEE);
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    assert_eq!(relais.annoncees(), [ANNONCEE]);
    assert!(
        relais.ecoutes().iter().all(|a| a.ip() != ANNONCEE),
        "{:?}",
        relais.ecoutes()
    );
    let (statut, _) = http(
        port,
        &format!("GET /s/ABCD0123 HTTP/1.1\r\nHost: 192.0.2.1:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200);
    let (_, texte) = http(
        port,
        &format!("GET /aide HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    // La page est notée ; elle ne se dit qu'après `PAGE_SANS_LIEN_MS` (tenu par les tests
    // de `relais::aide`, sur une horloge donnée).
    assert!(texte.contains("\"reseau\":true"), "{texte}");
    assert!(texte.contains("\"pages_sans_lien\":0"), "{texte}");
    relais.arreter();
    // Hors du banc, refusée.
    let mut r = reglages("aide-soiree", Banc::Non);
    r.annonce_du_banc = Some(ANNONCEE);
    assert!(coque::lancer(&r).is_err());
}

/// Un faux `soiree-epreuve`, qui « construit » un module donné : il écrit son empreinte
/// et son chemin, comme `--construire`.
fn faux_epreuve(d: &Path, empreinte: &str, module: &Path) -> PathBuf {
    let script = d.join("soiree-epreuve");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"Étape 1 : construire $2\"\necho \"{empreinte} {}\"\n",
            module.display()
        ),
    )
    .unwrap();
    let ok = Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    script
}

/// Un faux `open`, premier dans le chemin du relais : il écrit l'adresse qu'on lui donne
/// dans `ouvert.txt`, au lieu d'ouvrir le navigateur de l'hôte.
fn faux_open(d: &Path) -> PathBuf {
    let bin = d.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let script = bin.join("open");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s' \"$*\" > \"{}.part\" && mv \"{}.part\" \"{}\"\n",
            d.join("ouvert.txt").display(),
            d.join("ouvert.txt").display(),
            d.join("ouvert.txt").display()
        ),
    )
    .unwrap();
    let ok = Command::new("chmod")
        .arg("+x")
        .arg(&script)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    bin
}

/// Le statut d'un processus qui finit dans le délai ; sinon, tué, `None`.
fn finir_dans(
    enfant: &mut std::process::Child,
    delai: Duration,
) -> Option<std::process::ExitStatus> {
    let fin = Instant::now() + delai;
    loop {
        if let Some(s) = enfant.try_wait().unwrap() {
            return Some(s);
        }
        if Instant::now() > fin {
            let _: std::io::Result<()> = enfant.kill();
            let _: std::io::Result<std::process::ExitStatus> = enfant.wait();
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// P17, P35 : `soiree-relais jouer` refuse un module sans rapport (code 1, et la manière
/// de l'éprouver), et le joue avec `--non-eprouve`, en le disant ; le catalogue servi dit
/// alors le module non éprouvé, ce que l'écran affiche en permanence. *Après la
/// vérification du lot 10b* : sans `--sans-ouvrir`, il ouvre l'écran par `open`
/// (ici un faux, premier dans le chemin), à l'adresse du module ; et le faux
/// `soiree-epreuve` annonce la vraie empreinte BLAKE3 des octets qu'il construit.
#[test]
fn soiree_relais_jouer_refuse_puis_joue_non_eprouve_p17() {
    let d = dossier("jouer-binaire");
    let module = d.join("m.wasm");
    std::fs::write(&module, b"\0asm jeu").unwrap();
    // BLAKE3 de `\0asm jeu` (calculé une fois, hors du relais, qui ne hache rien).
    let e = "2cbabdf32d8e0de96a10df59a25440afa99ded992aee19455eeca490fc2b7044";
    let epreuve = faux_epreuve(&d, e, &module);
    let chemin = format!(
        "{}:{}",
        faux_open(&d).display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let ouvert = d.join("ouvert.txt");
    let paquet = d.join("0123456789abcdef");
    std::fs::create_dir_all(&paquet).unwrap();
    std::fs::write(paquet.join("demarrage.js"), b"// demarrage").unwrap();
    let relais = PathBuf::from(env!("CARGO_BIN_EXE_soiree-relais"));
    let commande = |non_eprouve: bool| {
        let mut c = Command::new(&relais);
        c.env("SOIREE_EPREUVE", &epreuve)
            .env("PATH", &chemin)
            .arg("jouer")
            .arg(&d)
            .arg("--catalogue")
            .arg(d.join("catalogue-vide"))
            .args(["--sans-reseau", "--port", "0", "--garde"])
            .arg(d.join("garde"))
            .arg("--paquet")
            .arg(&paquet)
            .stdin(Stdio::null());
        if non_eprouve {
            c.arg("--non-eprouve");
        }
        c
    };
    // Le refus, au plus 20 s : un relais qui démarre au lieu de refuser ne finirait jamais.
    let mut enfant = commande(false)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let statut = finir_dans(&mut enfant, Duration::from_secs(20))
        .expect("le relais a démarré au lieu de refuser un module sans rapport");
    assert_eq!(statut.code(), Some(1));
    let mut erreur = String::new();
    let _: std::io::Result<usize> =
        std::io::Read::read_to_string(&mut enfant.stderr.take().unwrap(), &mut erreur);
    assert!(erreur.contains("pas de rapport d'épreuve"), "{erreur}");
    assert!(erreur.contains("--non-eprouve"), "{erreur}");
    // Avec `--non-eprouve` : le relais démarre, et le dit.
    let mut enfant = commande(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let sortie = enfant.stdout.take().unwrap();
    let mut lignes = BufReader::new(sortie).lines();
    let fin = Instant::now() + Duration::from_secs(20);
    let (mut port, mut non_eprouve) = (None, false);
    while Instant::now() < fin && (port.is_none() || !non_eprouve) {
        let Some(Ok(l)) = lignes.next() else { break };
        if l.contains("NON ÉPROUVÉ") {
            non_eprouve = true;
        }
        if let Some(reste) = l.strip_prefix("l'écran : http://localhost:") {
            port = reste.split('/').next().and_then(|p| p.parse::<u16>().ok());
            assert!(l.ends_with(&format!("/?jeu={e}")), "{l}");
        }
    }
    let lu = port.map(|p| {
        http(
            p,
            &format!("GET /jeux/catalogue HTTP/1.1\r\nHost: localhost:{p}\r\n\r\n"),
        )
    });
    // L'écran ouvert par `open`, à l'adresse que le relais a dite.
    let fin = Instant::now() + Duration::from_secs(10);
    while !ouvert.exists() && Instant::now() < fin {
        std::thread::sleep(Duration::from_millis(50));
    }
    let adresse_ouverte = std::fs::read_to_string(&ouvert).ok();
    let _: std::io::Result<()> = enfant.kill();
    let _: std::io::Result<std::process::ExitStatus> = enfant.wait();
    assert!(non_eprouve, "le relais ne dit pas « NON ÉPROUVÉ »");
    let p = port.expect("le relais n'a pas dit son adresse");
    assert_eq!(
        adresse_ouverte.as_deref(),
        Some(format!("http://localhost:{p}/?jeu={e}").as_str()),
        "jouer n'a pas ouvert l'écran par open"
    );
    let (statut, catalogue) = lu.expect("le relais n'a pas dit son adresse");
    assert_eq!(statut, 200);
    assert!(
        catalogue.ends_with(&format!(
            "{{\"jeux\":[{{\"empreinte\":\"{e}\",\"taille\":8,\"eprouve\":false}}]}}"
        )),
        "{catalogue}"
    );
}
