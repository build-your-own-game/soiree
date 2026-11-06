//! Le cœur du relais, sans entrées-sorties, sur une horloge simulée (SAL-09) : les rôles,
//! une seule autorité, les refus, les échéances et les battements. Chaque test cite sa
//! leçon.

use liaison::{
    AdresseIp, BATTEMENT_MS, Case, CauseFermeture, Classe, CodeSalle, DuRelais,
    LIENS_MAX_SANS_AUTORITE, Lien, PROTOCOLE_RELAIS, RaisonCoupure, RefusRelais, Role,
    SILENCE_MAX_MS, TRAME_MAX, VersRelais,
};
use proptest::prelude::*;
use relais::coeur::{
    BONJOUR_MAX_US, Coeur, EtatLien, Fait, OpGarde, PreuvesEnMemoire, Provenance, RoleAdmis, Sortie,
};
use relais::file::Place;

const SALLE: CodeSalle = CodeSalle([0x3f, 0xa9, 0xc2, 0xd1]);
const AUTRE: CodeSalle = CodeSalle([1, 2, 3, 4]);
const PREUVE: [u8; 32] = [7; 32];
const JOINTE: AdresseIp = AdresseIp::V4([192, 168, 1, 20]);

/// Le cœur, et ce qu'il a rendu, décodé.
struct Banc {
    c: Coeur<PreuvesEnMemoire>,
    t: u64,
    /// Ce que chaque lien a reçu, dans l'ordre, avec sa place.
    recu: Vec<(Lien, Option<Place>, Vec<u8>)>,
    fermes: Vec<Lien>,
    battus: Vec<Lien>,
    admis: Vec<(Lien, RoleAdmis)>,
    garde: Vec<(CodeSalle, Lien, OpGarde)>,
    prochain: u32,
}

impl Banc {
    fn new() -> Banc {
        Banc {
            c: Coeur::new([9; 16], Vec::new(), PreuvesEnMemoire::default()),
            t: 1_000,
            recu: Vec::new(),
            fermes: Vec::new(),
            battus: Vec::new(),
            admis: Vec::new(),
            garde: Vec::new(),
            prochain: 0,
        }
    }

    fn vider(&mut self) {
        for s in self.c.sorties() {
            match s {
                Sortie::Envoyer {
                    lien,
                    place,
                    octets,
                } => self.recu.push((lien, place, octets)),
                Sortie::Battre { lien } => self.battus.push(lien),
                Sortie::Fermer { lien } => self.fermes.push(lien),
                Sortie::Admis { lien, role } => self.admis.push((lien, role)),
                Sortie::Garde { salle, lien, op } => self.garde.push((salle, lien, op)),
            }
        }
    }

    fn ouvrir_de(&mut self, p: Provenance) -> Lien {
        self.prochain += 1;
        let l = Lien(self.prochain);
        self.c.ouvrir(self.t, l, p, JOINTE);
        self.vider();
        l
    }

    fn ouvrir(&mut self) -> Lien {
        self.ouvrir_de(Provenance::BoucleLocale)
    }

    fn envoyer(&mut self, l: Lien, m: &VersRelais<'_>) {
        let o = m.encoder().unwrap();
        self.c.recevoir(self.t, self.t, l, &o);
        self.vider();
    }

    fn octets(&mut self, l: Lien, o: &[u8]) {
        self.c.recevoir(self.t, self.t, l, o);
        self.vider();
    }

    fn bonjour(&mut self, l: Lien, salle: CodeSalle, role: Role) {
        self.envoyer(
            l,
            &VersRelais::Bonjour {
                protocole: PROTOCOLE_RELAIS,
                salle,
                role,
            },
        );
    }

    /// Une autorité et sa garde, accueillies.
    fn autorite(&mut self, preuve: [u8; 32]) -> (Lien, Lien) {
        let a = self.ouvrir();
        self.bonjour(a, SALLE, Role::Autorite { preuve });
        let g = self.ouvrir();
        self.bonjour(
            g,
            SALLE,
            Role::Garde {
                preuve,
                autorite: a,
            },
        );
        (a, g)
    }

    fn appareil(&mut self) -> Lien {
        let l = self.ouvrir_de(Provenance::Reseau);
        self.bonjour(l, SALLE, Role::Appareil);
        l
    }

    fn avancer(&mut self, us: u64) {
        self.t += us;
        self.c.tic(self.t);
        self.vider();
    }

    /// Ce que ce lien a reçu, décodé, et l'oublie.
    fn pour(&mut self, l: Lien) -> Vec<DuRelais<'static>> {
        let mut v = Vec::new();
        let mut reste = Vec::new();
        for (x, p, o) in self.recu.drain(..) {
            if x == l {
                let m = DuRelais::decoder(&o).unwrap();
                v.push(possede(m));
            } else {
                reste.push((x, p, o));
            }
        }
        self.recu = reste;
        v
    }

    fn ferme(&self, l: Lien) -> bool {
        self.fermes.contains(&l)
    }
}

/// Un `DuRelais` qui ne garde que ce qu'il faut aux tests, sans emprunt.
fn possede(m: DuRelais<'_>) -> DuRelais<'static> {
    match m {
        DuRelais::Accueil {
            epoque,
            lien,
            adresses,
        } => DuRelais::Accueil {
            epoque,
            lien,
            adresses,
        },
        DuRelais::Refus(r) => DuRelais::Refus(r),
        DuRelais::Ouvert {
            lien,
            adresse_jointe,
        } => DuRelais::Ouvert {
            lien,
            adresse_jointe,
        },
        DuRelais::De {
            lien,
            classe,
            trame,
            recu_us,
            renvoye_us,
        } => DuRelais::De {
            lien,
            classe,
            trame: Vec::leak(trame.to_vec()),
            recu_us,
            renvoye_us,
        },
        DuRelais::Pour { classe, trame } => DuRelais::Pour {
            classe,
            trame: Vec::leak(trame.to_vec()),
        },
        DuRelais::Ferme { lien, cause } => DuRelais::Ferme { lien, cause },
        DuRelais::Encombre {
            lien,
            octets_en_attente,
        } => DuRelais::Encombre {
            lien,
            octets_en_attente,
        },
        DuRelais::AutoriteAbsente => DuRelais::AutoriteAbsente,
        DuRelais::AutoritePresente => DuRelais::AutoritePresente,
        DuRelais::Echo {
            t_envoi_us,
            t_relais_us,
        } => DuRelais::Echo {
            t_envoi_us,
            t_relais_us,
        },
        DuRelais::Ecrit { case } => DuRelais::Ecrit { case },
        DuRelais::Cases(c) => DuRelais::Cases(c),
        DuRelais::Lu { case, octets } => DuRelais::Lu {
            case,
            octets: Vec::leak(octets.to_vec()),
        },
    }
}

fn accueil(m: &DuRelais<'_>, l: Lien) -> bool {
    matches!(m, DuRelais::Accueil { lien, .. } if *lien == l)
}

/// Ce que l'autorité et les appareils attendent du relais (PHASE-3, § 2.5, écrit au lot
/// 4b d'après `RelaisEnMemoire`) : l'appareil accueilli apprend si l'autorité est là ;
/// l'autorité qui se présente reçoit un `Ouvert` par appareil déjà là, chacun
/// `AutoritePresente` ; les trames vont de l'un à l'autre, chacune à sa place.
#[test]
fn l_autorite_et_les_appareils_se_trouvent_comme_dans_le_relais_en_memoire() {
    let mut b = Banc::new();
    let d1 = {
        // La salle n'existe pas encore : l'appareil est refusé, typé.
        let d = b.ouvrir_de(Provenance::Reseau);
        b.bonjour(d, SALLE, Role::Appareil);
        assert_eq!(b.pour(d), vec![DuRelais::Refus(RefusRelais::SalleInconnue)]);
        assert!(b.ferme(d));
        let (a, _) = b.autorite(PREUVE);
        let accueil_a = b.pour(a);
        assert!(accueil(&accueil_a[0], a), "{accueil_a:?}");
        let d1 = b.appareil();
        let m = b.pour(d1);
        assert!(accueil(&m[0], d1));
        assert_eq!(m[1], DuRelais::AutoritePresente);
        assert_eq!(
            b.pour(a),
            vec![DuRelais::Ouvert {
                lien: d1,
                adresse_jointe: JOINTE
            }]
        );
        // Un appareil vers l'autorité ; une vue remplaçable de l'autorité vers lui.
        b.envoyer(
            d1,
            &VersRelais::Trame {
                classe: Classe::Remplacable(0),
                trame: b"tenu",
            },
        );
        let place = b.recu.iter().find(|(l, _, _)| *l == a).unwrap().1;
        assert_eq!(
            place,
            Some(Place {
                origine: d1.0,
                canal: 0
            })
        );
        let de = b.pour(a);
        assert!(
            matches!(&de[0], DuRelais::De { lien, classe: Classe::Remplacable(0), trame, .. } if *lien == d1 && *trame == b"tenu"),
            "{de:?}"
        );
        b.envoyer(
            a,
            &VersRelais::Vers {
                lien: d1,
                classe: Classe::Fiable,
                trame: b"vue",
            },
        );
        assert_eq!(
            b.pour(d1),
            vec![DuRelais::Pour {
                classe: Classe::Fiable,
                trame: b"vue"
            }]
        );
        d1
    };
    // Une autorité qui se coupe d'un appareil : il se ferme, elle l'apprend.
    let a = b.c.autorite(SALLE).unwrap();
    b.envoyer(
        a,
        &VersRelais::Couper {
            lien: d1,
            raison: RaisonCoupure::Illisible,
        },
    );
    assert!(b.ferme(d1));
    assert_eq!(
        b.pour(a),
        vec![DuRelais::Ferme {
            lien: d1,
            cause: CauseFermeture::Coupe
        }]
    );
    // Un appareil d'une autre salle ne reçoit rien de celle-ci.
    let x = b.ouvrir_de(Provenance::Reseau);
    b.bonjour(x, AUTRE, Role::Appareil);
    assert_eq!(b.pour(x), vec![DuRelais::Refus(RefusRelais::SalleInconnue)]);
}

/// SAL-18 et P36 : une seconde autorité remplace la première. L'ancienne reçoit
/// `Refus(Remplacee)` sur ses deux liens, qui se ferment ; les appareils ne voient pas
/// d'absence, mais la nouvelle autorité, qui refait la poignée de main avec chacun.
#[test]
fn une_seconde_autorite_remplace_la_premiere_sal18() {
    let mut b = Banc::new();
    let (a1, g1) = b.autorite(PREUVE);
    let d = b.appareil();
    b.recu.clear();
    let (a2, g2) = b.autorite(PREUVE);
    assert_eq!(b.pour(a1), vec![DuRelais::Refus(RefusRelais::Remplacee)]);
    assert_eq!(b.pour(g1), vec![DuRelais::Refus(RefusRelais::Remplacee)]);
    assert!(b.ferme(a1) && b.ferme(g1));
    assert_eq!(b.pour(d), vec![DuRelais::AutoritePresente]);
    let m = b.pour(a2);
    assert!(accueil(&m[0], a2));
    assert!(m.contains(&DuRelais::Ouvert {
        lien: d,
        adresse_jointe: JOINTE
    }));
    assert!(accueil(&b.pour(g2)[0], g2));
    assert_eq!(b.c.autorite(SALLE), Some(a2));
    // Ce que l'appareil envoie va à la nouvelle, plus à l'ancienne.
    b.envoyer(
        d,
        &VersRelais::Trame {
            classe: Classe::Fiable,
            trame: b"g",
        },
    );
    assert_eq!(b.pour(a1), Vec::<DuRelais<'_>>::new());
    assert_eq!(b.pour(a2).len(), 1);
    // L'ancienne n'écrit plus à l'appareil.
    b.envoyer(
        a1,
        &VersRelais::Vers {
            lien: d,
            classe: Classe::Fiable,
            trame: b"vieille",
        },
    );
    assert_eq!(b.pour(d), Vec::<DuRelais<'_>>::new());
}

/// SAL-19 : « la plus récente » se lit dans l'ordre d'ouverture des liens. Un bonjour
/// d'autorité resté coincé sur un lien plus ancien, livré après celui du lien suivant,
/// ne déloge pas l'autorité courante.
#[test]
fn un_bonjour_d_autorite_coince_sur_un_lien_plus_ancien_ne_remplace_personne_sal19() {
    let mut b = Banc::new();
    let ancien = b.ouvrir();
    let recent = b.ouvrir();
    b.bonjour(recent, SALLE, Role::Autorite { preuve: PREUVE });
    b.bonjour(ancien, SALLE, Role::Autorite { preuve: PREUVE });
    assert_eq!(
        b.pour(ancien),
        vec![DuRelais::Refus(RefusRelais::Remplacee)]
    );
    assert!(b.ferme(ancien));
    assert!(!b.ferme(recent));
    assert_eq!(b.c.autorite(SALLE), Some(recent));
}

/// SAL-20 : une garde d'une autorité passée n'écrit plus. Ni ce que l'ancienne garde
/// envoie après le remplacement, ni une garde qui nomme l'ancienne autorité, ni la
/// réponse du disque à une demande de l'ancienne ne passent.
#[test]
fn un_ecrire_d_une_autorite_remplacee_est_refuse_sal20() {
    let mut b = Banc::new();
    let (a1, g1) = b.autorite(PREUVE);
    b.envoyer(
        g1,
        &VersRelais::Ecrire {
            case: Case::Point(0),
            octets: b"p0",
        },
    );
    assert_eq!(b.garde.len(), 1, "la garde courante écrit");
    let (_, g2) = b.autorite(PREUVE);
    b.garde.clear();
    b.recu.clear();
    // L'ancienne garde, fermée, n'écrit plus.
    b.envoyer(
        g1,
        &VersRelais::Ecrire {
            case: Case::Journal(1),
            octets: b"j1",
        },
    );
    assert_eq!(b.garde, Vec::new());
    // La réponse du disque à sa demande d'avant ne lui va pas.
    b.c.garde_faite(g1, Fait::Ecrit(Case::Point(0)));
    b.vider();
    assert_eq!(b.pour(g1), Vec::<DuRelais<'_>>::new());
    // Une garde qui nomme l'ancienne autorité est refusée.
    let g3 = b.ouvrir();
    b.bonjour(
        g3,
        SALLE,
        Role::Garde {
            preuve: PREUVE,
            autorite: a1,
        },
    );
    assert_eq!(b.pour(g3), vec![DuRelais::Refus(RefusRelais::Remplacee)]);
    // La nouvelle écrit, et reçoit ses réponses.
    b.envoyer(g2, &VersRelais::Lister);
    assert_eq!(b.garde, vec![(SALLE, g2, OpGarde::Lister)]);
    b.c.garde_faite(g2, Fait::Cases(vec![Case::Point(0)]));
    b.vider();
    assert_eq!(b.pour(g2), vec![DuRelais::Cases(vec![Case::Point(0)])]);
}

/// SAL-23 : quand l'autorité s'arrête, chaque appareil l'apprend ; quand une autre
/// revient, chacun l'apprend aussi, et elle reçoit leurs liens.
#[test]
fn quand_l_autorite_part_chaque_appareil_l_apprend_sal23() {
    let mut b = Banc::new();
    let (a, g) = b.autorite(PREUVE);
    let d1 = b.appareil();
    let d2 = b.appareil();
    b.recu.clear();
    b.envoyer(a, &VersRelais::AuRevoir);
    assert!(b.ferme(a));
    assert_eq!(b.pour(d1), vec![DuRelais::AutoriteAbsente]);
    assert_eq!(b.pour(d2), vec![DuRelais::AutoriteAbsente]);
    // Sa garde reste ouverte : ce qu'elle a donné au disque s'y achève.
    assert!(!b.ferme(g));
    // La connexion d'une autorité qui meurt sans au revoir : de même.
    let (a2, _) = b.autorite(PREUVE);
    assert_eq!(b.pour(d1), vec![DuRelais::AutoritePresente]);
    assert_eq!(b.pour(d2), vec![DuRelais::AutoritePresente]);
    b.c.ferme(a2, CauseFermeture::Fermee);
    b.vider();
    assert_eq!(b.pour(d1), vec![DuRelais::AutoriteAbsente]);
    assert_eq!(b.pour(d2), vec![DuRelais::AutoriteAbsente]);
    // Un appareil qui arrive sans autorité l'apprend aussi.
    let d3 = b.appareil();
    let m = b.pour(d3);
    assert_eq!(m[1], DuRelais::AutoriteAbsente);
}

/// SAL-42 et SAL-148 : chaque refus est typé, définitif, et ferme sa connexion.
#[test]
fn chaque_refus_est_type_et_ferme_son_lien_sal42_sal148() {
    let mut b = Banc::new();
    let verifier = |b: &mut Banc, l: Lien, r: RefusRelais| {
        assert_eq!(b.pour(l), vec![DuRelais::Refus(r)], "{r:?}");
        assert!(b.ferme(l), "{r:?} ne ferme pas son lien");
        // Plus rien de lui ne compte.
        b.envoyer(l, &VersRelais::Echo { t_envoi_us: 1 });
        assert_eq!(b.pour(l), Vec::<DuRelais<'_>>::new());
    };
    // Un autre protocole, lisible ou cassé (SAL-40).
    let l = b.ouvrir();
    b.envoyer(
        l,
        &VersRelais::Bonjour {
            protocole: 2,
            salle: SALLE,
            role: Role::Appareil,
        },
    );
    verifier(
        &mut b,
        l,
        RefusRelais::Protocole {
            relais: PROTOCOLE_RELAIS,
        },
    );
    let l = b.ouvrir();
    b.octets(l, &[0, 9, 0xff, 0xff, 0xff]);
    verifier(
        &mut b,
        l,
        RefusRelais::Protocole {
            relais: PROTOCOLE_RELAIS,
        },
    );
    let l = b.ouvrir();
    b.bonjour(l, SALLE, Role::Appareil);
    verifier(&mut b, l, RefusRelais::SalleInconnue);
    // Seule la boucle locale tient l'autorité (SAL-33).
    let l = b.ouvrir_de(Provenance::Reseau);
    b.bonjour(l, SALLE, Role::Autorite { preuve: PREUVE });
    verifier(&mut b, l, RefusRelais::HorsBoucleLocale);
    let (a, g) = b.autorite(PREUVE);
    let l = b.ouvrir_de(Provenance::Reseau);
    b.bonjour(
        l,
        SALLE,
        Role::Garde {
            preuve: PREUVE,
            autorite: a,
        },
    );
    verifier(&mut b, l, RefusRelais::HorsBoucleLocale);
    // Une autre preuve (P41).
    let l = b.ouvrir();
    b.bonjour(l, SALLE, Role::Autorite { preuve: [8; 32] });
    verifier(&mut b, l, RefusRelais::PreuveFausse);
    let l = b.ouvrir();
    b.bonjour(
        l,
        SALLE,
        Role::Garde {
            preuve: [8; 32],
            autorite: a,
        },
    );
    verifier(&mut b, l, RefusRelais::PreuveFausse);
    // La salle pleine : la borne d'avant `Limites`, puis celle de l'autorité.
    let appareils: Vec<Lien> = (0..LIENS_MAX_SANS_AUTORITE).map(|_| b.appareil()).collect();
    assert!(appareils.iter().all(|l| !b.ferme(*l)));
    let l = b.appareil();
    verifier(&mut b, l, RefusRelais::Pleine);
    b.envoyer(a, &VersRelais::Limites { liens_max: 75 });
    assert_eq!(b.c.liens_max(SALLE), 75);
    let l = b.appareil();
    assert!(!b.ferme(l));
    // Une trame trop grosse de l'autorité.
    let gros = vec![0u8; TRAME_MAX as usize + 1];
    b.recu.clear();
    b.envoyer(
        a,
        &VersRelais::Vers {
            lien: l,
            classe: Classe::Fiable,
            trame: &gros,
        },
    );
    b.recu.retain(|(x, _, _)| *x == a);
    verifier(&mut b, a, RefusRelais::TropGros);
    // La garde pleine, que dit le disque.
    let (_, g2) = b.autorite(PREUVE);
    b.recu.clear();
    b.envoyer(
        g2,
        &VersRelais::Ecrire {
            case: Case::Point(0),
            octets: b"p",
        },
    );
    b.c.garde_faite(g2, Fait::Refus(RefusRelais::GardePleine));
    b.vider();
    verifier(&mut b, g2, RefusRelais::GardePleine);
    // Remplacée (SAL-18).
    assert!(b.ferme(g));
}

/// SAL-117 du côté du relais : un appareil dont un message ne se lit pas ne coûte que son
/// lien, et l'autorité l'apprend ; un trop gros aussi.
#[test]
fn un_message_illisible_ou_trop_gros_ne_coute_que_son_lien() {
    let mut b = Banc::new();
    let (a, _) = b.autorite(PREUVE);
    let d1 = b.appareil();
    let d2 = b.appareil();
    let d3 = b.appareil();
    b.recu.clear();
    b.octets(d1, &[0xee, 0xee]);
    let gros = vec![0u8; TRAME_MAX as usize + 1];
    b.envoyer(
        d2,
        &VersRelais::Trame {
            classe: Classe::Fiable,
            trame: &gros,
        },
    );
    assert!(b.ferme(d1) && b.ferme(d2) && !b.ferme(d3));
    assert_eq!(
        b.pour(a),
        vec![
            DuRelais::Ferme {
                lien: d1,
                cause: CauseFermeture::Illisible
            },
            DuRelais::Ferme {
                lien: d2,
                cause: CauseFermeture::TropGros
            }
        ]
    );
    // Un premier message qui n'est pas un bonjour : fermé, sans refus.
    let l = b.ouvrir();
    b.envoyer(l, &VersRelais::Lister);
    assert!(b.ferme(l));
    assert_eq!(b.pour(l), Vec::<DuRelais<'_>>::new());
}

/// SAL-43 : seul un battement dit qu'un pair est vivant. Le relais bat sur chaque lien
/// toutes les `BATTEMENT_MS` ; un lien qui ne rend rien depuis `SILENCE_MAX_MS` est
/// fermé, et l'autorité l'apprend ; un battement rendu le garde.
#[test]
fn le_silence_se_voit_et_chaque_lien_bat_sal43() {
    let mut b = Banc::new();
    let (autorite, garde) = b.autorite(PREUVE);
    let muet = b.appareil();
    let vivant = b.appareil();
    b.recu.clear();
    let battement = u64::from(BATTEMENT_MS) * 1000;
    let silence = u64::from(SILENCE_MAX_MS) * 1000;
    let mut ecoule = 0;
    while ecoule < silence {
        b.avancer(10_000);
        ecoule += 10_000;
        b.c.vivant(b.t, autorite);
        b.c.vivant(b.t, garde);
        b.c.vivant(b.t, vivant);
    }
    // Un battement par lien et par `BATTEMENT_MS`, ni plus ni moins.
    let attendus = usize::try_from(silence / battement).unwrap();
    for l in [autorite, garde, muet, vivant] {
        let n = b.battus.iter().filter(|x| **x == l).count();
        assert!(
            n == attendus || n + 1 == attendus,
            "{l:?} : {n} battements, {attendus} attendus"
        );
    }
    assert!(!b.ferme(muet), "fermé avant SILENCE_MAX_MS");
    b.avancer(20_000);
    assert!(b.ferme(muet));
    assert!(!b.ferme(vivant) && !b.ferme(autorite) && !b.ferme(garde));
    assert_eq!(
        b.pour(autorite),
        vec![DuRelais::Ferme {
            lien: muet,
            cause: CauseFermeture::Muet
        }]
    );
}

/// SAL-49 : toute attente a son échéance. Un lien sans bonjour en 10 s est fermé.
#[test]
fn un_lien_sans_bonjour_se_ferme_en_dix_secondes_sal49() {
    let mut b = Banc::new();
    let l = b.ouvrir();
    // Vivant, il rend ses battements ; mais il ne dit jamais bonjour.
    for _ in 0..(BONJOUR_MAX_US / 10_000 - 1) {
        b.avancer(10_000);
        b.c.vivant(b.t, l);
    }
    assert!(!b.ferme(l));
    b.avancer(10_000);
    assert!(b.ferme(l));
    assert_eq!(b.c.etat(l), Some(EtatLien::Ferme));
}

/// SAL-75 : l'adresse que l'autorité apprend pour un appareil est celle qu'il a jointe,
/// lien par lien.
#[test]
fn l_adresse_jointe_par_chaque_appareil_va_a_l_autorite_sal75() {
    let mut b = Banc::new();
    let (a, _) = b.autorite(PREUVE);
    b.recu.clear();
    b.prochain += 1;
    let d = Lien(b.prochain);
    let v6 = AdresseIp::V6([0xfd, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
    b.c.ouvrir(b.t, d, Provenance::Reseau, v6);
    b.bonjour(d, SALLE, Role::Appareil);
    b.recu.retain(|(x, _, _)| *x == a);
    assert_eq!(
        b.pour(a),
        vec![DuRelais::Ouvert {
            lien: d,
            adresse_jointe: v6
        }]
    );
}

/// L'écho du relais : l'aller-retour jusqu'à lui seul, à son heure.
#[test]
fn l_echo_rend_l_instant_et_l_heure_du_relais() {
    let mut b = Banc::new();
    let (a, _) = b.autorite(PREUVE);
    b.recu.clear();
    b.t = 55_000;
    b.envoyer(a, &VersRelais::Echo { t_envoi_us: 42 });
    assert_eq!(
        b.pour(a),
        vec![DuRelais::Echo {
            t_envoi_us: 42,
            t_relais_us: 55_000
        }]
    );
}

/// Un relais relancé reprend les téléphones avant que l'autorité revienne : la salle
/// dont la preuve est sur disque est connue (P14).
#[test]
fn un_relais_relance_reprend_les_telephones_avant_l_autorite() {
    let mut preuves = PreuvesEnMemoire::default();
    preuves.0.insert(SALLE, PREUVE);
    let mut b = Banc::new();
    b.c = Coeur::new([1; 16], Vec::new(), preuves);
    let d = b.appareil();
    let m = b.pour(d);
    assert!(accueil(&m[0], d));
    assert_eq!(m[1], DuRelais::AutoriteAbsente);
    let (a, _) = b.autorite(PREUVE);
    assert_eq!(b.pour(d), vec![DuRelais::AutoritePresente]);
    assert!(b.pour(a).contains(&DuRelais::Ouvert {
        lien: d,
        adresse_jointe: JOINTE
    }));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// REM-64 : des octets au hasard, sur un lien de chaque rôle, ne font jamais
    /// paniquer le cœur, et ne touchent aucun autre lien qu'en le fermant.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_le_coeur_rem64(
        messages in proptest::collection::vec((0usize..4, proptest::collection::vec(any::<u8>(), 0..64)), 1..32)
    ) {
        let mut b = Banc::new();
        let (a, g) = b.autorite(PREUVE);
        let d = b.appareil();
        let x = b.ouvrir();
        for (qui, o) in &messages {
            let l = [a, g, d, x][*qui];
            b.octets(l, o);
            b.avancer(1_000);
        }
        let _: Vec<Sortie> = b.c.sorties();
    }
}
