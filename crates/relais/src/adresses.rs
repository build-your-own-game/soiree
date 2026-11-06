//! Les adresses où le relais écoute et qu'il annonce aux téléphones (PHASE-3, § 2.5).
//!
//! - **La route par défaut est une supposition, pas un fait** (SAL-76) : le relais lit
//!   les interfaces de la machine (`if-addrs`) et annonce chaque adresse joignable du
//!   réseau local (IPv4 d'abord). Sans aucune, il écoute en boucle locale seulement, et
//!   le dit : « Pas de réseau ».
//! - **Une adresse qu'on n'écoute pas se refuse à voix haute** (SAL-77) : `--adresse`
//!   nomme une adresse d'une interface de la machine, ou le relais ne démarre pas.
//! - **L'adresse annoncée à un téléphone est celle qu'il a jointe** (SAL-75) : la coque
//!   la lit sur chaque connexion (`local_addr`), et l'autorité la reçoit dans `Ouvert`.

use std::net::IpAddr;

use liaison::AdresseIp;

/// Une adresse d'une interface de la machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Interface {
    /// Le nom de l'interface (`en0`, `lo0`…).
    pub nom: String,
    /// L'adresse.
    pub ip: IpAddr,
}

/// Les adresses des interfaces de la machine, telles que le système les dit maintenant.
pub fn interfaces() -> Result<Vec<Interface>, String> {
    let liste = if_addrs::get_if_addrs().map_err(|e| format!("lire les interfaces : {e}"))?;
    Ok(liste
        .into_iter()
        .map(|i| Interface {
            ip: i.ip(),
            nom: i.name,
        })
        .collect())
}

/// Une adresse que les téléphones du réseau local peuvent joindre : ni la boucle locale,
/// ni une adresse de lien local (qui demande un nom d'interface dans l'adresse), ni
/// l'adresse nulle, ni une adresse de groupe.
pub fn joignable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_multicast()
                || v4.is_broadcast())
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unicast_link_local()
                || v6.is_unspecified()
                || v6.is_multicast())
        }
    }
}

/// Les adresses que le relais écoute et annonce, hors de la boucle locale.
///
/// Sans `--adresse` : chaque adresse joignable des interfaces, IPv4 d'abord, sans
/// doublon ; aucune, et le relais démarre en boucle locale seulement (SAL-76). Avec
/// `--adresse` : elle seule, si une interface la porte et qu'elle est joignable, sinon
/// un refus qui dit pourquoi (SAL-77).
pub fn choisir(interfaces: &[Interface], forcee: Option<IpAddr>) -> Result<Vec<IpAddr>, String> {
    if let Some(ip) = forcee {
        if !joignable(ip) {
            return Err(format!(
                "--adresse {ip} : les téléphones ne peuvent pas joindre cette adresse (boucle locale, lien local, adresse nulle ou de groupe)"
            ));
        }
        if !interfaces.iter().any(|i| i.ip == ip) {
            return Err(format!(
                "--adresse {ip} : aucune interface de cette machine n'a cette adresse ; le relais n'y écouterait pas, et le QR mènerait nulle part (SAL-77)"
            ));
        }
        return Ok(vec![ip]);
    }
    let mut v4: Vec<IpAddr> = Vec::new();
    let mut v6: Vec<IpAddr> = Vec::new();
    for i in interfaces {
        if !joignable(i.ip) {
            continue;
        }
        let liste = if i.ip.is_ipv4() { &mut v4 } else { &mut v6 };
        if !liste.contains(&i.ip) {
            liste.push(i.ip);
        }
    }
    v4.sort();
    v6.sort();
    v4.extend(v6);
    Ok(v4)
}

/// Une adresse en octets, telle que `liaison` la porte.
pub fn en_octets(ip: IpAddr) -> AdresseIp {
    match ip {
        IpAddr::V4(v4) => AdresseIp::V4(v4.octets()),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => AdresseIp::V4(v4.octets()),
            None => AdresseIp::V6(v6.octets()),
        },
    }
}
