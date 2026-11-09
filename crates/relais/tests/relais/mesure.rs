//! La mesure du vrai Wi-Fi, sur la vraie coque (lot 14, `relais::mesure`, M5) : les échos
//! reviennent avec l'heure du relais, les tics partent au rythme des vues, un bilan est
//! gardé en mémoire et sur disque, chaque route a sa permission (SAL-33), et le retard des
//! échos n'existe qu'au banc.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use relais::coque::{self, Relais};
use relais::http::Banc;
use relais::mesure::{ECHO_OCTETS, FICHIER, REPONSE_OCTETS, TIC_OCTETS};
use tungstenite::client::IntoClientRequest;
use tungstenite::{Bytes, Message, WebSocket};

use crate::commun::{http, reglages};

/// Une WebSocket de mesure, ouverte comme la page d'un téléphone (`Origin` d'une page du
/// relais), ou l'erreur de la poignée.
fn relier(relais: &Relais, origine: &str) -> Result<WebSocket<TcpStream>, String> {
    let port = relais.port();
    let flux = TcpStream::connect(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)).unwrap();
    flux.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut demande = format!("ws://localhost:{port}/mesure/ws")
        .into_client_request()
        .unwrap();
    demande
        .headers_mut()
        .insert("Origin", origine.parse().unwrap());
    tungstenite::client::client(demande, flux)
        .map(|(ws, _)| ws)
        .map_err(|e| format!("{e}"))
}

/// Lit jusqu'à la réponse à un écho ; compte les tics vus en chemin.
fn lire_l_echo(ws: &mut WebSocket<TcpStream>) -> (Vec<u8>, u32) {
    let mut tics = 0;
    loop {
        match ws.read().unwrap() {
            Message::Binary(b) if b.len() == REPONSE_OCTETS => return (b.to_vec(), tics),
            Message::Binary(b) if b.len() == TIC_OCTETS => tics += 1,
            autre @ (Message::Binary(_)
            | Message::Text(_)
            | Message::Ping(_)
            | Message::Pong(_)
            | Message::Close(_)
            | Message::Frame(_)) => panic!("inattendu : {autre:?}"),
        }
    }
}

/// M5 : un écho revient avec ses octets et l'heure du relais ; les tics partent tous les
/// 16,7 ms (SAL-43 pour le rythme qu'on mesure : celui des vues) ; un bilan est gardé.
#[test]
fn les_echos_reviennent_les_tics_partent_et_un_bilan_se_garde() {
    let r = reglages("mesure", Banc::Oui);
    let garde = r.garde.clone();
    let relais = coque::lancer(&r).unwrap();
    let port = relais.port();
    let mut ws = relier(&relais, &format!("http://127.0.0.1:{port}")).expect("reliée");
    let echo: Vec<u8> = (1..=u8::try_from(ECHO_OCTETS).unwrap()).collect();
    let avant = Instant::now();
    ws.send(Message::Binary(Bytes::from(echo.clone()))).unwrap();
    let (reponse, _) = lire_l_echo(&mut ws);
    assert!(avant.elapsed() < Duration::from_secs(1));
    assert_eq!(&reponse[..ECHO_OCTETS], echo.as_slice());
    let heure = u64::from_le_bytes(reponse[ECHO_OCTETS..].try_into().unwrap());
    assert!(heure > 0, "l'heure du relais");
    // Une demi-seconde de tics : une trentaine au rythme des vues, jamais une rafale.
    let debut = Instant::now();
    let mut tics = Vec::new();
    while debut.elapsed() < Duration::from_millis(500) {
        if let Message::Binary(b) = ws.read().unwrap()
            && b.len() == TIC_OCTETS
        {
            tics.push(u64::from_le_bytes(b[..].try_into().unwrap()));
        }
    }
    assert!((20..=40).contains(&tics.len()), "{} tics", tics.len());
    for paire in tics.windows(2) {
        let ecart = paire[1] - paire[0];
        assert!((10_000..=60_000).contains(&ecart), "écart de {ecart} µs");
    }
    // Un bilan : gardé, rendu à la télé, écrit sur disque.
    let bilan = "{\"ua\":\"un \\\"téléphone\\\"\",\"echos\":1}";
    ws.send(Message::Text(bilan.into())).unwrap();
    let fin = Instant::now() + Duration::from_secs(5);
    let corps = loop {
        let (statut, texte) = http(
            port,
            &format!("GET /mesure/bilans HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
        );
        assert_eq!(statut, 200, "{texte}");
        let corps = texte.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
        if corps.contains("echos") || Instant::now() > fin {
            break corps;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let v: serde_json::Value = serde_json::from_str(&corps).expect("du JSON");
    let bilans = v
        .get("bilans")
        .and_then(serde_json::Value::as_array)
        .unwrap();
    assert_eq!(bilans.len(), 1, "{corps}");
    assert_eq!(
        bilans[0].get("bilan").and_then(serde_json::Value::as_str),
        Some(bilan)
    );
    // Sans réseau : aucune adresse où ouvrir la page.
    assert_eq!(
        v.get("pages")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(0)
    );
    let fichier = std::fs::read_to_string(garde.join(FICHIER)).unwrap();
    let ligne: serde_json::Value = serde_json::from_str(fichier.lines().next().unwrap()).unwrap();
    assert_eq!(
        ligne.get("bilan").and_then(serde_json::Value::as_str),
        Some(bilan)
    );
    // Un bilan sur deux lignes ferme la mesure, sans rien écrire.
    ws.send(Message::Text("{\"a\":1}\n{\"b\":2}".into()))
        .unwrap();
    let fin = Instant::now() + Duration::from_secs(5);
    loop {
        match ws.read() {
            Ok(Message::Binary(_)) if Instant::now() < fin => {}
            Ok(autre) => {
                assert!(matches!(autre, Message::Close(_)), "{autre:?}");
                break;
            }
            Err(_) => break,
        }
    }
    assert_eq!(
        std::fs::read_to_string(garde.join(FICHIER)).unwrap(),
        fichier
    );
    relais.arreter();
}

/// SAL-33 : la page de mesure est à tous (un `Host` local ou annoncé) ; les bilans, à la
/// boucle locale seulement ; la WebSocket, à une page du relais.
#[test]
fn chaque_route_de_la_mesure_a_sa_permission_sal33() {
    let relais = coque::lancer(&reglages("mesure-routes", Banc::Non)).unwrap();
    let port = relais.port();
    let (statut, texte) = http(
        port,
        &format!("GET /mesure HTTP/1.1\r\nHost: localhost:{port}\r\n\r\n"),
    );
    assert_eq!(statut, 200, "{texte}");
    let (statut, _) = http(
        port,
        "GET /mesure HTTP/1.1\r\nHost: soiree.example:80\r\n\r\n",
    );
    assert_eq!(statut, 403, "un Host étranger (rebond DNS)");
    let (statut, _) = http(
        port,
        &format!(
            "GET /mesure/bilans HTTP/1.1\r\nHost: localhost:{port}\r\nOrigin: http://soiree.example\r\n\r\n"
        ),
    );
    assert_eq!(statut, 403, "une Origin étrangère");
    assert!(relier(&relais, "http://soiree.example").is_err());
    assert!(relier(&relais, &format!("http://127.0.0.1:{port}")).is_ok());
    relais.arreter();
}

/// Le retard des échos sert au contrôle négatif du banc, et à rien d'autre.
#[test]
fn le_retard_des_echos_n_existe_qu_au_banc() {
    let mut r = reglages("mesure-retard", Banc::Non);
    r.retard_d_echo_du_banc = Some(Duration::from_millis(40));
    assert!(coque::lancer(&r).is_err());
    let mut r = reglages("mesure-retard-banc", Banc::Oui);
    r.retard_d_echo_du_banc = Some(Duration::from_millis(40));
    let relais = coque::lancer(&r).unwrap();
    let mut ws = relier(&relais, &format!("http://127.0.0.1:{}", relais.port())).unwrap();
    let avant = Instant::now();
    ws.send(Message::Binary(Bytes::from(vec![0u8; ECHO_OCTETS])))
        .unwrap();
    let _: (Vec<u8>, u32) = lire_l_echo(&mut ws);
    assert!(avant.elapsed() >= Duration::from_millis(40));
    relais.arreter();
}
