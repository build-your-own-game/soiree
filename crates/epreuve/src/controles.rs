//! Les petits outils des contrôles : chercher un secret dans une vue (E8), compter
//! les appareils qu'un état retient (E15), fabriquer une arrivée absurde (E13).

use jeu_abi::{
    Appareil, Capacites, CauseDepart, Duree, EchecDemande, IdDemande, IdGeste, Instant, Pas,
    Presence, encoder,
};
use partie::{Arrivee, EcranArrivee, ReponseArrivee};

use crate::tirage::Tirage;
use jeu_abi::SorteOctets;

/// Le plus court secret que E8 cherche dans une vue : en deçà, un octet ou deux se
/// retrouvent partout par hasard (un chiffre, un booléen), et la recherche ne dit
/// rien.
pub(crate) const SECRET_MIN: usize = 4;

/// Le plus court secret à longueur en tête (un texte, une liste) que E8 cherche, sa
/// longueur comprise : un mot de deux lettres, trois octets. Une vue écrit un texte
/// avec sa longueur ; trois octets dont le premier dit exactement le reste ne s'y
/// trouvent guère par hasard. Point de départ, non mesuré.
pub(crate) const SECRET_LONGUEUR_MIN: usize = 3;

/// Les valeurs secrètes qu'une projection a vidées : celles que E8 cherche, et le
/// nombre de celles qui sont trop courtes pour être cherchées (un booléen, un
/// variant, un petit nombre). Ces dernières, E8 ne les voit pas : le rapport le dit.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Caches {
    pub(crate) cherches: Vec<Vec<u8>>,
    pub(crate) trop_courts: u64,
}

/// La fenêtre qui recale la projection sur la forme canonique après un secret vidé.
const RECALAGE: usize = 48;

/// Cette suite d'octets se trouve-t-elle dans `dans` ?
pub(crate) fn trouve(dans: &[u8], quoi: &[u8]) -> bool {
    !quoi.is_empty() && dans.windows(quoi.len()).any(|w| w == quoi)
}

/// Les octets où chercher une fuite : le retour de la lecture entier, qui contient
/// la vue.
pub(crate) fn octets_de_la_vue(retour: &[u8]) -> Vec<u8> {
    retour.to_vec()
}

/// Les valeurs secrètes qu'une projection a vidées, depuis la forme canonique et
/// cette projection, de la même définitive.
///
/// La projection écrit chaque secret invisible comme un `Option` vide (`0`) là où la
/// forme canonique écrit `1` suivi de la valeur. Les deux suites s'alignent donc
/// octet pour octet, sauf à ces endroits : à chaque écart, la valeur est ce qui
/// sépare la forme canonique du point où elle se recale sur la suite de la
/// projection. Rend `None` si les deux ne s'alignent pas ainsi : la recherche n'a
/// alors rien de sûr à chercher.
///
/// Chaque valeur est rendue telle quelle, et, si elle commence par une longueur
/// (un texte, une liste), aussi sans elle : une vue écrit un texte avec sa propre
/// longueur. Seules les valeurs d'au moins [`SECRET_MIN`] octets sont rendues, ou
/// d'au moins [`SECRET_LONGUEUR_MIN`] avec leur longueur ; les autres sont comptées.
pub(crate) fn secrets_caches(canonique: &[u8], projection: &[u8]) -> Option<Caches> {
    let mut i = 0usize;
    let mut j = 0usize;
    let mut trouves = Vec::new();
    let mut trop_courts = 0u64;
    while i < canonique.len() && j < projection.len() {
        let (a, b) = (canonique.get(i)?, projection.get(j)?);
        if a == b {
            i = i.saturating_add(1);
            j = j.saturating_add(1);
            continue;
        }
        if *a != 1 || *b != 0 {
            return None;
        }
        let reste = projection.get(j.saturating_add(1)..)?;
        let debut = i.saturating_add(1);
        // Le premier point où la forme canonique reprend la suite de la projection :
        // jusqu'au bout, sur `RECALAGE` octets, ou jusqu'au secret vidé suivant.
        let fin = (debut..=canonique.len()).find(|&e| {
            let suite = canonique.get(e..).unwrap_or(&[]);
            let commun = suite.iter().zip(reste).take_while(|(x, y)| x == y).count();
            commun == reste.len()
                || commun >= RECALAGE
                || (commun >= 1 && suite.get(commun) == Some(&1) && reste.get(commun) == Some(&0))
        })?;
        let valeur = canonique.get(debut..fin)?;
        let avant = trouves.len();
        // La valeur, puis chaque longueur en tête retirée tant qu'elle dit exactement
        // le reste : la forme canonique d'un `Texte` en a deux (ses octets, puis le
        // texte), une vue n'en écrit qu'une.
        let mut niveau = Some(valeur);
        while let Some(v) = niveau {
            let sans = sans_longueur(v);
            if v.len() >= SECRET_MIN || (sans.is_some() && v.len() >= SECRET_LONGUEUR_MIN) {
                trouves.push(v.to_vec());
            }
            niveau = sans;
        }
        if trouves.len() == avant {
            trop_courts = trop_courts.saturating_add(1);
        }
        i = fin;
        j = j.saturating_add(1);
    }
    (i == canonique.len() && j == projection.len()).then_some(Caches {
        cherches: trouves,
        trop_courts,
    })
}

/// La valeur sans sa longueur en tête, si elle en a une qui dit exactement le reste.
fn sans_longueur(v: &[u8]) -> Option<&[u8]> {
    let mut n: u64 = 0;
    for (k, o) in v.iter().enumerate().take(10) {
        let bits = u64::from(o & 0x7f);
        n |= bits.checked_shl(u32::try_from(k.saturating_mul(7)).ok()?)?;
        if o & 0x80 == 0 {
            let reste = v.get(k.saturating_add(1)..)?;
            return (u64::try_from(reste.len()).ok()? == n).then_some(reste);
        }
    }
    None
}

/// Combien des appareils de `ids` la forme canonique cite : leur encodage (un
/// varint de 64 bits tiré au hasard) ne s'y trouve pas par hasard.
pub(crate) fn appareils_retenus(canonique: &[u8], ids: &[u64]) -> u64 {
    let corps = jeu_abi::lire_canonique(canonique).map_or(canonique, |c| c.corps);
    let mut n = 0u64;
    for id in ids {
        let mut o = Vec::new();
        if encoder(&Appareil::du_fil(*id), &mut o).is_ok() && trouve(corps, &o) {
            n = n.saturating_add(1);
        }
    }
    n
}

/// Les longueurs saturées d'une scène (E17) : une longueur à `Fixe::MIN` ou
/// `Fixe::MAX` vient d'un flottant que `Fixe::depuis_f32` a dû saturer (REM-57).
pub(crate) fn fixes_satures(scene: &jeu_abi::SceneFil) -> u64 {
    use jeu_abi::{ComposantVuFil, Fixe, Rect, SorteNoeud, StyleTexte, Taille};
    let mut n = 0u64;
    let mut voir = |f: Fixe| {
        if f.saturee() == jeu_abi::Oui::Oui {
            n = n.saturating_add(1);
        }
    };
    let rect = |r: &Rect, voir: &mut dyn FnMut(Fixe)| {
        voir(r.x);
        voir(r.y);
        voir(r.l);
        voir(r.h);
    };
    let style = |s: &StyleTexte, voir: &mut dyn FnMut(Fixe)| match s.taille {
        Taille::Fixe(t) => voir(t),
        Taille::Budget { max, min } => {
            voir(max);
            voir(min);
        }
    };
    for noeud in &scene.noeuds {
        if let Some((_, a)) = &noeud.anime {
            voir(a.de.decalage.0);
            voir(a.de.decalage.1);
            voir(a.a.decalage.0);
            voir(a.a.decalage.1);
        }
        match &noeud.sorte {
            SorteNoeud::Rect { r, .. }
            | SorteNoeud::Image { r, .. }
            | SorteNoeud::Surface { r } => {
                rect(r, &mut voir);
            }
            SorteNoeud::Texte {
                boite, style: st, ..
            }
            | SorteNoeud::CompteARebours {
                boite, style: st, ..
            } => {
                rect(boite, &mut voir);
                style(st, &mut voir);
            }
            SorteNoeud::Bouton { boite, .. }
            | SorteNoeud::Qr { boite }
            | SorteNoeud::Avertissements { boite } => rect(boite, &mut voir),
            SorteNoeud::Groupe { decalage } => {
                voir(decalage.0);
                voir(decalage.1);
            }
            SorteNoeud::Composant(ComposantVuFil::EnPlace { style: st, .. }) => {
                style(st, &mut voir);
            }
            SorteNoeud::Composant(ComposantVuFil::Deplacables {
                seuil,
                bord,
                vitesse_max,
                ..
            }) => {
                voir(*seuil);
                voir(*bord);
                voir(*vitesse_max);
            }
            SorteNoeud::Composant(
                ComposantVuFil::Champ { .. }
                | ComposantVuFil::Selecteur { .. }
                | ComposantVuFil::Modale { .. }
                | ComposantVuFil::Lien { .. },
            )
            | SorteNoeud::Definitif => {}
        }
    }
    n
}

/// Des octets abîmés : un à trois octets changés, coupés ou allongés.
fn abimer(tirage: &mut Tirage, origine: &[u8]) -> Vec<u8> {
    let mut abime = origine.to_vec();
    match tirage.sous(5) {
        // Le dernier octet, souvent une valeur : un chiffre, un indice, un compteur.
        4 => {
            let octet = tirage.octet();
            if let Some(dernier) = abime.last_mut() {
                *dernier = octet;
            }
        }
        0 => {
            let taille =
                usize::try_from(tirage.sous(u64::try_from(abime.len()).unwrap_or(0))).unwrap_or(0);
            abime.truncate(taille);
        }
        1 => {
            let taille = usize::try_from(tirage.entre(1, 8)).unwrap_or(1);
            abime.extend(tirage.octets(taille));
        }
        _ => {
            for _ in 0..tirage.entre(1, 3) {
                let taille = u64::try_from(abime.len()).unwrap_or(0);
                if taille == 0 {
                    break;
                }
                let rang = usize::try_from(tirage.sous(taille)).unwrap_or(0);
                let octet = tirage.octet();
                if let Some(place) = abime.get_mut(rang) {
                    *place = octet;
                }
            }
        }
    }
    abime
}

/// Une arrivée absurde (E13) : des octets au hasard ou abîmés dans chaque champ de
/// l'enveloppe, des appareils inconnus, des présences et des réponses hors de
/// propos, des instants hors de toute borne (SAL-117, REM-58, REM-59, SAL-129).
#[expect(
    clippy::too_many_lines,
    reason = "un bras par sorte d'arrivée absurde, à lire ensemble"
)]
pub(crate) fn arrivee_absurde(
    tirage: &mut Tirage,
    connus: &[u64],
    echantillons: &[(SorteOctets, Vec<u8>)],
    pas: u64,
) -> Arrivee {
    let nombre = u64::try_from(connus.len()).unwrap_or(0);
    let un_connu = |tirage: &mut Tirage| {
        connus
            .get(usize::try_from(tirage.sous(nombre)).unwrap_or(0))
            .copied()
            .unwrap_or(1)
    };
    let gestes: Vec<&Vec<u8>> = echantillons
        .iter()
        .filter(|(sorte, _)| *sorte == SorteOctets::GesteTelephone)
        .map(|(_, octets_vus)| octets_vus)
        .collect();
    let avant = Instant::de(Pas::du_fil(pas.saturating_sub(1)));
    let geste = |de: u64, octets: Vec<u8>, tirage: &mut Tirage| Arrivee::Geste {
        de: Appareil::du_fil(de),
        octets,
        fait: avant,
        vu: Pas::du_fil(pas.saturating_sub(1)),
        id: IdGeste::du_fil(tirage.entier()),
        demi_aller_retour: Duree::pas(1),
    };
    match tirage.sous(9) {
        0 | 1 => {
            let base = gestes
                .get(
                    usize::try_from(tirage.sous(u64::try_from(gestes.len()).unwrap_or(0)))
                        .unwrap_or(0),
                )
                .map_or_else(Vec::new, |octets_vus| (*octets_vus).clone());
            let octets_vus = abimer(tirage, &base);
            let de = un_connu(tirage);
            geste(de, octets_vus, tirage)
        }
        2 => {
            let taille = usize::try_from(tirage.entre(0, 48)).unwrap_or(0);
            let octets_vus = tirage.octets(taille);
            let de = un_connu(tirage);
            geste(de, octets_vus, tirage)
        }
        3 => {
            let taille = usize::try_from(tirage.entre(0, 32)).unwrap_or(0);
            Arrivee::Tenu {
                de: Appareil::du_fil(un_connu(tirage)),
                octets: tirage.octets(taille),
                numero: tirage.entier(),
                fait: avant,
                demi_aller_retour: Duree::pas(tirage.entier()),
            }
        }
        4 => {
            let taille = usize::try_from(tirage.entre(0, 16)).unwrap_or(0);
            Arrivee::Ecran {
                quoi: EcranArrivee::Action(tirage.octets(taille)),
                vu: Pas::du_fil(tirage.entier()),
            }
        }
        5 => {
            let base = gestes
                .first()
                .map_or_else(Vec::new, |octets_vus| (*octets_vus).clone());
            Arrivee::Geste {
                de: Appareil::du_fil(un_connu(tirage)),
                octets: base,
                fait: Instant {
                    pas: Pas::du_fil(tirage.entier()),
                    fraction: tirage.fraction(),
                },
                vu: Pas::du_fil(tirage.entier()),
                id: IdGeste::du_fil(tirage.entier()),
                demi_aller_retour: Duree::pas(tirage.entier()),
            }
        }
        6 => {
            let base = gestes
                .first()
                .map_or_else(Vec::new, |octets_vus| (*octets_vus).clone());
            geste(tirage.entier() | 1, base, tirage)
        }
        7 => {
            let inconnu = Appareil::du_fil(tirage.entier() | 1);
            Arrivee::Presence(match tirage.sous(3) {
                0 => Presence::Depart {
                    appareil: inconnu,
                    cause: CauseDepart::Silence,
                },
                1 => Presence::Retour {
                    appareil: inconnu,
                    capacites: Capacites::AUCUNE,
                },
                _ => Presence::Capacites {
                    appareil: Appareil::du_fil(un_connu(tirage)),
                    capacites: Capacites::AUCUNE,
                },
            })
        }
        _ => {
            let taille = usize::try_from(tirage.entre(0, 16)).unwrap_or(0);
            Arrivee::Reponse {
                id: IdDemande::du_fil(tirage.entier()),
                reponse: match tirage.sous(2) {
                    0 => ReponseArrivee::Lu(Ok(tirage.octets(taille))),
                    _ => ReponseArrivee::Fait(Err(EchecDemande::Annule)),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{appareils_retenus, secrets_caches, trouve};

    #[test]
    fn un_secret_vide_se_retrouve() {
        // forme ‖ … 1, [6, A N A N A S] … ‖ fin
        let c = [
            9, 9, 1, 6, b'A', b'N', b'A', b'N', b'A', b'S', 7, 7, 1, 3, 5, 5,
        ];
        let p = [9, 9, 0, 7, 7, 0, 5, 5];
        let caches = secrets_caches(&c, &p).expect("alignées");
        let s = &caches.cherches;
        assert!(s.contains(&b"ANANAS".to_vec()), "{s:?}");
        assert!(
            s.contains(&vec![6, b'A', b'N', b'A', b'N', b'A', b'S']),
            "{s:?}"
        );
        // Le secret d'un octet n'est pas cherché, mais il est compté.
        assert!(!s.iter().any(|v| v == &[3]));
        assert_eq!(caches.trop_courts, 1);
    }

    /// CAT-46 : un mot secret de deux lettres (trois octets avec sa longueur) est
    /// cherché, et retrouvé dans une vue qui le recopie.
    #[test]
    fn un_mot_de_deux_lettres_se_cherche() {
        // forme ‖ … 1, [3, [2, A B]] … ‖ fin : un `Texte` dans la forme canonique,
        // ses octets puis le texte, chacun avec sa longueur.
        let c = [9, 9, 1, 3, 2, b'A', b'B', 7, 7];
        let p = [9, 9, 0, 7, 7];
        let caches = secrets_caches(&c, &p).expect("alignées");
        assert_eq!(
            caches.cherches,
            vec![vec![3, 2, b'A', b'B'], vec![2, b'A', b'B']]
        );
        assert_eq!(caches.trop_courts, 0);
        // La vue écrit le texte avec sa seule longueur.
        assert!(trouve(&[0, 5, 2, b'A', b'B', 1], &caches.cherches[1]));
    }

    #[test]
    fn deux_suites_qui_ne_s_alignent_pas() {
        assert_eq!(secrets_caches(&[1, 2, 3], &[4, 5]), None);
        assert_eq!(
            secrets_caches(&[1, 2, 3], &[1, 2, 3]),
            Some(super::Caches::default())
        );
    }

    #[test]
    fn recherche_et_comptage() {
        assert!(trouve(b"le mot ANANAS", b"ANANAS"));
        assert!(!trouve(b"le mot", b""));
        let mut o = Vec::new();
        jeu_abi::encoder(&jeu_abi::Appareil::du_fil(0xdead_beef_0123), &mut o).expect("encoder");
        let mut c = vec![0u8; 8];
        c.extend_from_slice(&o);
        c.extend_from_slice(&[0u8; 8]);
        assert_eq!(appareils_retenus(&c, &[0xdead_beef_0123, 0x0bad_cafe]), 1);
    }
}
