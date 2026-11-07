//! Ce que la garde tient, en octets (PHASE-3, § 2.4 ; `partie::persistance`) : l'aller
//! et retour d'un point, de son journal, de l'ouverture et de l'état du cœur ; la
//! reprise depuis ce qui a été relu, et depuis un journal tronqué (P10) ; les octets
//! figés, relus par une seconde lecture qui n'utilise ni serde ni postcard (SAL-90) ;
//! des octets abîmés refusés sans panique (REM-64).

use std::sync::Arc;

use jeu_abi::{Appareil, IdGeste, Instant, Oui, Pas, RegardFil, SortieFil, TexteCourt};
use partie::modele::{FabriqueModele, GesteModele, InstantaneModele, ReglageModele};
use partie::persistance::{
    self, InstantaneGarde, RefusGarde, SorteGarde, VERSION_GARDE, decoder_coeur, decoder_journal,
    decoder_ouverture, decoder_point, encoder_coeur, encoder_journal, encoder_lignes,
    encoder_ouverture, encoder_point,
};
use partie::regles::Origine;
use partie::{
    ArriveePlacee, Budgets, Faute, IssueJournal, Journal, Ligne, LigneEntree, Ouverture, Partie,
    Placement, PointDeReprise, QuoiPlace, RefusReprise,
};
use proptest::prelude::*;

use crate::commun::*;

type Point = PointDeReprise<InstantaneModele>;

/// Une soirée sur le modèle : des gestes, des tenus, une panique, une nouvelle partie,
/// et des points tout du long. Rend la partie et ses points.
fn une_soiree(pas_total: u64) -> (PartieModele, Vec<Point>) {
    let mut o = ouverture();
    o.point_tous_les = 7;
    let mut p = Partie::ouvrir(FabriqueModele::new(ReglageModele::ordinaire()), o).unwrap();
    let mut points = vec![p.point().unwrap()];
    let mut id = 0;
    for s in 1..=pas_total {
        let g = match s % 11 {
            3 => GesteModele::Paniquer,
            5 => GesteModele::Refuser,
            9 if s == 31 => GesteModele::NouvellePartie,
            _ => GesteModele::Ajouter(u8::try_from(s % 5).unwrap() + 1),
        };
        id += 1;
        let _: Placement = p.arriver(geste(1 + s % 3, g, s, id), instant(s)).unwrap();
        if s % 4 == 0 {
            let _: Placement = p
                .arriver(tenu(2, 0, u8::try_from(s % 4).unwrap(), s, s), instant(s))
                .unwrap();
        }
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
        if s % 6 == 0 {
            points.push(p.point().unwrap());
        }
    }
    (p, points)
}

/// L'ouverture de [`une_soiree`].
fn ouverture_de(_: &PartieModele) -> Ouverture {
    let mut o = ouverture();
    o.point_tous_les = 7;
    o
}

// ---------------------------------------------------------------------------
// Aller et retour, reprise
// ---------------------------------------------------------------------------

/// Un point, sa chaîne, le journal, l'ouverture et l'état du cœur se relisent égaux ; un
/// point cité deux fois redevient un seul `Arc`.
#[test]
fn point_et_journal_aller_retour() {
    let (p, points) = une_soiree(60);
    for pt in &points {
        let relu: Point = decoder_point(&encoder_point(pt)).unwrap();
        assert_eq!(&relu, pt);
        if let (Some(a), Some(b)) = (pt.sain(), pt.periodique())
            && Arc::ptr_eq(a, b)
        {
            let (ra, rb) = (relu.sain().unwrap(), relu.periodique().unwrap());
            assert!(
                Arc::ptr_eq(ra, rb),
                "un point cité deux fois reste un seul point"
            );
        }
    }
    let j = p.journal();
    assert_eq!(&decoder_journal(&encoder_journal(j)).unwrap(), j);
    let o = ouverture_de(&p);
    assert_eq!(decoder_ouverture(&encoder_ouverture(&o)).unwrap(), o);
    assert_eq!(
        &decoder_coeur(&encoder_coeur(p.coeur())).unwrap(),
        p.coeur()
    );
}

/// La reprise depuis ce que la garde a relu (point et journal décodés) retrouve chaque
/// empreinte notée, et la même partie que la reprise depuis les originaux.
#[test]
fn reprendre_depuis_un_point_decode_meme_empreinte_sur_le_modele() {
    let (mut p, points) = une_soiree(60);
    let fin = p.empreinte().unwrap();
    let journal = decoder_journal(&encoder_journal(p.journal())).unwrap();
    for pt in &points {
        let relu: Point = decoder_point(&encoder_point(pt)).unwrap();
        let mut r = Partie::reprendre(
            FabriqueModele::new(ReglageModele::ordinaire()),
            ouverture_de(&p),
            &relu,
            &journal,
        )
        .unwrap();
        assert_eq!(r.empreinte().unwrap(), fin);
        assert_eq!(r.coeur(), p.coeur());
        assert_eq!(etat(&r), etat(&p));
    }
}

/// P10 : un journal tronqué avant un point s'en reprend comme l'entier, ses indices
/// absolus inchangés ; un point plus vieux que sa base est refusé.
#[test]
fn reprendre_sur_un_journal_tronque() {
    let (p, points) = une_soiree(60);
    let reglage = ReglageModele::ordinaire;
    let milieu = &points[points.len() / 2];
    let premier = &points[1];
    assert!(premier.indice() < milieu.indice());
    let mut tronque = p.journal().clone();
    let base = tronque.tronquer_avant(milieu.indice());
    assert_eq!(base, milieu.indice());
    assert_eq!(tronque.fin(), p.journal().fin());
    assert_eq!(tronque.ligne(base), p.journal().ligne(base));
    let relu = decoder_journal(&encoder_journal(&tronque)).unwrap();
    assert_eq!(relu, tronque);
    let entier = Partie::reprendre(
        FabriqueModele::new(reglage()),
        ouverture_de(&p),
        milieu,
        p.journal(),
    )
    .unwrap();
    let depuis_tronque = Partie::reprendre(
        FabriqueModele::new(reglage()),
        ouverture_de(&p),
        milieu,
        &relu,
    )
    .unwrap();
    assert_eq!(depuis_tronque.coeur(), entier.coeur());
    assert_eq!(etat(&depuis_tronque), etat(&entier));
    assert_eq!(depuis_tronque.journal().fin(), entier.journal().fin());
    let trop_vieux = Partie::reprendre(
        FabriqueModele::new(reglage()),
        ouverture_de(&p),
        premier,
        &relu,
    );
    assert!(matches!(trop_vieux, Err(RefusReprise::PointHorsJournal)));
}

/// P10, en direct : la partie tronque son journal jusqu'à son dernier point au plus,
/// continue, reconstruit une faute, et se reprend encore depuis le journal tronqué.
#[test]
fn une_partie_tronquee_continue_et_se_reprend() {
    let (mut p, _) = une_soiree(40);
    let point = p.point().unwrap();
    let base = p.tronquer_journal_avant(usize::MAX);
    assert_eq!(base, point.indice(), "pas au-delà du dernier point");
    assert_eq!(p.journal().base(), base);
    for s in 41..=70 {
        let g = if s == 50 {
            GesteModele::Paniquer
        } else {
            GesteModele::Ajouter(1)
        };
        let _: Placement = p.arriver(geste(1, g, s, 1000 + s), instant(s)).unwrap();
        let _: partie::Avancee = p.avancer(pas(s)).unwrap();
    }
    assert!(ecartees(p.journal()) >= 1);
    let r = Partie::reprendre(
        FabriqueModele::new(ReglageModele::ordinaire()),
        ouverture_de(&p),
        &point,
        &decoder_journal(&encoder_journal(p.journal())).unwrap(),
    )
    .unwrap();
    assert_eq!(r.coeur(), p.coeur());
    assert_eq!(etat(&r), etat(&p));
}

/// E20 (PHASE-3, lot 3b) : la reprise qui refait les fautes rejoue chaque entrée
/// écartée, comme la partie l'a jouée (l'appel, la faute, la reconstruction), et
/// retrouve la même partie ; elle fait plus d'appels que la reprise ordinaire, qui les
/// saute. Une faute du journal que l'instance ne refait pas est une divergence, que la
/// reprise ordinaire ne voit pas.
#[test]
fn la_reprise_qui_refait_les_fautes_refait_chacune() {
    let (mut p, points) = une_soiree(60);
    let fin = p.empreinte().unwrap();
    let ecartees_au_journal = ecartees(p.journal());
    assert!(ecartees_au_journal >= 5, "{ecartees_au_journal}");
    let creation = &points[0];
    let ordinaire = FabriqueModele::new(ReglageModele::ordinaire());
    let compteurs_ordinaires = std::rc::Rc::clone(&ordinaire.compteurs);
    let mut r = Partie::reprendre(ordinaire, ouverture_de(&p), creation, p.journal()).unwrap();
    assert_eq!(r.empreinte().unwrap(), fin);
    let refaisant = FabriqueModele::new(ReglageModele::ordinaire());
    let compteurs = std::rc::Rc::clone(&refaisant.compteurs);
    let mut f = Partie::reprendre_en_refaisant_les_fautes(
        refaisant,
        ouverture_de(&p),
        creation,
        p.journal(),
    )
    .unwrap();
    assert_eq!(f.empreinte().unwrap(), fin);
    assert_eq!(f.coeur(), p.coeur());
    assert_eq!(etat(&f), etat(&p));
    // Chaque faute refaite reconstruit la définitive : une restauration de plus chacune.
    assert_eq!(
        compteurs.restaurations.get(),
        compteurs_ordinaires.restaurations.get() + u64::try_from(ecartees_au_journal).unwrap(),
    );
    assert!(compteurs.ecritures.get() > compteurs_ordinaires.ecritures.get());
    // Le journal dit une autre faute que celle que l'instance refait.
    let lignes: Vec<Ligne> = p
        .journal()
        .lignes()
        .iter()
        .cloned()
        .map(|l| match l {
            Ligne::Entree(mut e) if matches!(e.issue, IssueJournal::Ecartee { .. }) => {
                e.issue = IssueJournal::Ecartee {
                    faute: Faute::Budget,
                };
                Ligne::Entree(e)
            }
            l @ (Ligne::Entree(_)
            | Ligne::Arrivees { .. }
            | Ligne::Empreinte { .. }
            | Ligne::Saut { .. }
            | Ligne::Pause { .. }
            | Ligne::Vue { .. }
            | Ligne::Partie { .. }
            | Ligne::Reprise { .. }) => l,
        })
        .collect();
    let autre = decoder_journal(&encoder_lignes(
        p.journal().base(),
        p.journal().pas(),
        &lignes,
    ))
    .unwrap();
    assert!(
        Partie::reprendre(
            FabriqueModele::new(ReglageModele::ordinaire()),
            ouverture_de(&p),
            creation,
            &autre,
        )
        .is_ok(),
        "la reprise ordinaire ne relit pas la faute"
    );
    let refus = Partie::reprendre_en_refaisant_les_fautes(
        FabriqueModele::new(ReglageModele::ordinaire()),
        ouverture_de(&p),
        creation,
        &autre,
    );
    assert!(
        matches!(&refus, Err(RefusReprise::Panne(pa)) if pa.cause == partie::CausePanne::RejeuDivergent),
        "{:?}",
        refus.err()
    );
}

/// Un morceau de journal se raccorde à la fin de celui qu'il prolonge, et pas ailleurs.
#[test]
fn les_morceaux_de_journal_se_raccordent() {
    let (p, _) = une_soiree(30);
    let j = p.journal();
    let coupe = j.fin() / 2;
    let mut debut = j.clone();
    let mut fin = j.clone();
    let _: usize = fin.tronquer_avant(coupe);
    let premier = encoder_lignes(0, Pas::ZERO, &j.lignes()[..coupe]);
    let second = encoder_lignes(coupe, j.pas(), &j.lignes()[coupe..]);
    let mut relu = decoder_journal(&premier).unwrap();
    assert_eq!(relu.fin(), coupe);
    persistance::raccorder(&mut relu, decoder_journal(&second).unwrap()).unwrap();
    assert_eq!(&relu, j);
    let _: usize = debut.tronquer_avant(0);
    let mal = persistance::raccorder(&mut debut, decoder_journal(&premier).unwrap());
    assert!(matches!(mal, Err(RefusGarde::Incoherente(_))));
    assert_eq!(&debut, j, "un raccord refusé ne change rien");
    assert_eq!(fin.base(), coupe);
}

// ---------------------------------------------------------------------------
// Les refus
// ---------------------------------------------------------------------------

/// Une garde d'une autre version, d'une autre sorte, coupée ou trop longue est refusée,
/// typée.
#[test]
fn une_garde_illisible_est_refusee() {
    let o = encoder_ouverture(&ouverture());
    assert_eq!(&o[..4], &VERSION_GARDE.to_le_bytes());
    let mut autre = o.clone();
    autre[0] = autre[0].wrapping_add(1);
    assert_eq!(
        decoder_ouverture(&autre),
        Err(RefusGarde::Version {
            lue: VERSION_GARDE + 1,
            attendue: VERSION_GARDE
        })
    );
    assert_eq!(
        decoder_journal(&o),
        Err(RefusGarde::Sorte {
            lue: SorteGarde::Ouverture as u8,
            attendue: SorteGarde::Journal
        })
    );
    assert_eq!(decoder_ouverture(&o[..3]), Err(RefusGarde::Court));
    assert!(matches!(
        decoder_ouverture(&o[..o.len() - 1]),
        Err(RefusGarde::Illisible(_))
    ));
    let mut long = o.clone();
    long.push(0);
    assert!(matches!(
        decoder_ouverture(&long),
        Err(RefusGarde::Illisible(_))
    ));
}

/// Un point dont la table se cite elle-même, ou cite hors d'elle, est refusé.
#[test]
fn un_point_incoherent_est_refuse() {
    let mut p = ouvrir(ReglageModele::ordinaire());
    let mut octets = encoder_point(&p.point().unwrap());
    // Le dernier octet : `periodique` du dernier point de la table, `None` (0). Le
    // faire citer le rang 0 le fait citer un point qui n'est pas plus loin que lui.
    let dernier = octets.len() - 1;
    assert_eq!(octets[dernier], 0);
    octets[dernier] = 1;
    octets.push(0);
    assert!(matches!(
        decoder_point::<InstantaneModele>(&octets),
        Err(RefusGarde::Incoherente(_))
    ));
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 512, .. ProptestConfig::default() })]

    /// REM-64 : des octets au hasard, sous un en-tête valide, ne font jamais paniquer
    /// les décodeurs de la garde.
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer_la_garde(
        sorte in 1u8..5,
        corps in proptest::collection::vec(any::<u8>(), 0..256),
    ) {
        let mut o = VERSION_GARDE.to_le_bytes().to_vec();
        o.push(sorte);
        o.extend_from_slice(&corps);
        let _: Result<Point, RefusGarde> = decoder_point(&o);
        let _: Result<Journal, RefusGarde> = decoder_journal(&o);
        let _: Result<Ouverture, RefusGarde> = decoder_ouverture(&o);
        let _: Result<partie::EtatCoeur, RefusGarde> = decoder_coeur(&o);
        let _: Result<InstantaneModele, RefusGarde> = InstantaneModele::lire(&corps);
    }

    /// REM-64 : un message valide abîmé (un octet changé, ou coupé) se relit ou se
    /// refuse, sans panique.
    #[test]
    fn une_garde_abimee_ne_fait_jamais_paniquer(
        pas_total in 1u64..25,
        ou in any::<prop::sample::Index>(),
        par in any::<u8>(),
        couper in any::<bool>(),
    ) {
        let (p, points) = une_soiree(pas_total);
        for mut o in [encoder_journal(p.journal()), encoder_point(&points[points.len() - 1])] {
            let i = ou.index(o.len());
            if couper {
                o.truncate(i);
            } else {
                o[i] ^= par.max(1);
            }
            let _: Result<Journal, RefusGarde> = decoder_journal(&o);
            let _: Result<Point, RefusGarde> = decoder_point(&o);
        }
    }
}

// ---------------------------------------------------------------------------
// Les octets figés (comme `forme_figee` de `jeu-abi`) et la seconde lecture (SAL-90)
// ---------------------------------------------------------------------------

fn ouverture_figee() -> Ouverture {
    Ouverture {
        graine: [7; 32],
        pas: 5,
        heure: 1000,
        salle: TexteCourt::new("3FA9C2D1"),
        budgets: Budgets {
            entree: 1,
            lecture: 2,
            creation: 3,
            canonique: 300,
        },
        point_tous_les: 600,
    }
}

/// Un journal qui a une ligne de chaque sorte, et une issue de chaque sorte.
pub(crate) fn journal_fige() -> Journal {
    let entree = |ordre: u64, issue: IssueJournal| {
        Ligne::Entree(LigneEntree {
            partie: 0,
            ordre,
            place: Pas::du_fil(4),
            octets: vec![9],
            issue,
        })
    };
    let lignes = vec![
        Ligne::Arrivees {
            pas: Pas::du_fil(4),
            arrivees: vec![ArriveePlacee {
                sequence: 1,
                place: Pas::du_fil(4),
                fait: Instant {
                    pas: Pas::du_fil(4),
                    fraction: 2,
                },
                recu: Instant::de(Pas::du_fil(4)),
                vu: Pas::du_fil(3),
                origine: Origine::Appareil(0),
                quoi: QuoiPlace::Geste {
                    de: Appareil::du_fil(7),
                    octets: vec![1, 2],
                    id: IdGeste::du_fil(5),
                },
            }],
        },
        entree(
            0,
            IssueJournal::Acceptee {
                sorties: vec![SortieFil::Oublier(7)],
            },
        ),
        entree(1, IssueJournal::Refusee { avis: vec![1] }),
        entree(2, IssueJournal::Ignoree),
        entree(
            3,
            IssueJournal::Ecartee {
                faute: Faute::Panique {
                    message: String::from("x"),
                },
            },
        ),
        Ligne::Empreinte {
            partie: 0,
            ordre: 255,
            empreinte: [0xab; 32],
        },
        Ligne::Saut {
            de: Pas::du_fil(4),
            a: Pas::du_fil(8),
        },
        Ligne::Pause {
            apres: Pas::du_fil(8),
            en_pause: Oui::Oui,
        },
        Ligne::Vue {
            regard: RegardFil::Appareil(7),
            pas: Pas::du_fil(8),
            apres: Pas::du_fil(8),
        },
        Ligne::Partie {
            numero: 1,
            graine: [1; 32],
        },
        Ligne::Reprise {
            apres: Pas::du_fil(9),
            depuis: 3,
        },
    ];
    let j = decoder_journal(&encoder_lignes(3, Pas::du_fil(9), &lignes)).unwrap();
    assert_eq!(j.base(), 3);
    j
}

/// L'en-tête : la version en `u32` petit-boutiste, puis la sorte.
pub(crate) fn h(sorte: u8) -> Vec<u8> {
    let mut o = VERSION_GARDE.to_le_bytes().to_vec();
    o.push(sorte);
    o
}

pub(crate) fn cat(parties: &[&[u8]]) -> Vec<u8> {
    parties.concat()
}

/// Les octets de l'ouverture figée, écrits à la main d'après la forme (voir
/// `persistance`) : graine (32 octets), pas, heure, salle (longueur puis UTF-8), les
/// quatre budgets, `point_tous_les`, chacun en varint.
fn octets_ouverture() -> Vec<u8> {
    cat(&[
        &h(3),
        &[7; 32],
        &[5],
        &[0xe8, 0x07],
        &[8],
        b"3FA9C2D1",
        &[1, 2, 3, 0xac, 0x02],
        &[0xd8, 0x04],
    ])
}

/// Les octets du journal figé.
fn octets_journal() -> Vec<u8> {
    cat(&[
        &h(2),
        // base 3, pas 9, onze lignes
        &[3, 9, 11],
        // Arrivees (0) : pas 4, une arrivée : séquence 1, place 4, fait (4, 2), reçu
        // (4, 0), vu 3, origine Appareil (2) de rang 0, Geste (0) de 7, octets [1, 2],
        // identité 5.
        &[0, 4, 1, 1, 4, 4, 2, 4, 0, 3, 2, 0, 0, 7, 2, 1, 2, 5],
        // Entree (1) : partie 0, ordre, place 4, octets [9], issue.
        // Acceptee (0) : une sortie, Oublier (0) de 7.
        &[1, 0, 0, 4, 1, 9, 0, 1, 0, 7],
        // Refusee (1) : avis [1].
        &[1, 0, 1, 4, 1, 9, 1, 1, 1],
        // Ignoree (2).
        &[1, 0, 2, 4, 1, 9, 2],
        // Ecartee (3) : Panique (2), « x ».
        &[1, 0, 3, 4, 1, 9, 3, 2, 1, b'x'],
        // Empreinte (2) : partie 0, ordre 255, 32 octets.
        &[2, 0, 0xff, 0x01],
        &[0xab; 32],
        // Saut (3) : de 4 à 8. Pause (4) : après 8, Oui (0). Vue (5) : Appareil (1) 7,
        // pas 8, après 8.
        &[3, 4, 8, 4, 8, 0, 5, 1, 7, 8, 8],
        // Partie (6) : numéro 1, graine.
        &[6, 1],
        &[1; 32],
        // Reprise (7) : après 9, depuis 3.
        &[7, 9, 3],
    ])
}

#[test]
fn les_octets_de_la_garde_sont_figes() {
    assert_eq!(
        VERSION_GARDE, 3,
        "les octets de ce fichier sont ceux de la version 3 (ceux qu'il fige n'ont pas changé depuis la 1 : la version 3 ne touche que les fichiers et le rendu, figés dans forme_garde.rs)"
    );
    let mut ecarts = Vec::new();
    let o = encoder_ouverture(&ouverture_figee());
    if o != octets_ouverture() {
        ecarts.push(format!("ouverture : {o:02X?}"));
    }
    if decoder_ouverture(&octets_ouverture()).as_ref() != Ok(&ouverture_figee()) {
        ecarts.push(String::from(
            "ouverture : les octets écrits se relisent autrement",
        ));
    }
    let j = encoder_journal(&journal_fige());
    if j != octets_journal() {
        ecarts.push(format!("journal : {j:02X?}"));
    }
    if decoder_journal(&octets_journal()).as_ref() != Ok(&journal_fige()) {
        ecarts.push(String::from(
            "journal : les octets écrits se relisent autrement",
        ));
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
}

/// Le lecteur indépendant : sans serde, sans postcard, d'après la forme seule.
pub(crate) struct Lecteur<'a> {
    pub(crate) octets: &'a [u8],
}

impl<'a> Lecteur<'a> {
    pub(crate) fn octet(&mut self) -> u8 {
        let (b, reste) = self.octets.split_first().expect("fin des octets");
        self.octets = reste;
        *b
    }

    /// Un varint LEB128 : sept bits par octet, poids faible d'abord.
    pub(crate) fn varint(&mut self) -> u64 {
        let mut v = 0u64;
        for decalage in (0..70).step_by(7) {
            let b = self.octet();
            v |= u64::from(b & 0x7f) << decalage;
            if b & 0x80 == 0 {
                return v;
            }
        }
        panic!("varint trop long");
    }

    pub(crate) fn brut(&mut self, n: usize) -> &'a [u8] {
        let (t, reste) = self.octets.split_at(n);
        self.octets = reste;
        t
    }

    pub(crate) fn tranche(&mut self) -> &'a [u8] {
        let n = usize::try_from(self.varint()).unwrap();
        self.brut(n)
    }

    pub(crate) fn option(&mut self) -> Option<u64> {
        match self.octet() {
            0 => None,
            1 => Some(self.varint()),
            x => panic!("option {x}"),
        }
    }

    pub(crate) fn entete(&mut self, sorte: u8) {
        let v = self.brut(4);
        assert_eq!(v, VERSION_GARDE.to_le_bytes());
        assert_eq!(self.octet(), sorte);
    }

    pub(crate) fn fini(&self) -> bool {
        self.octets.is_empty()
    }

    /// L'issue d'une entrée, d'après sa forme, comparée à celle qu'on attend.
    fn issue(&mut self, attendue: &IssueJournal) {
        match (self.varint(), attendue) {
            (0, IssueJournal::Acceptee { sorties }) => {
                assert_eq!(self.varint(), u64::try_from(sorties.len()).unwrap());
                for s in sorties {
                    let SortieFil::Oublier(n) = s else { panic!() };
                    assert_eq!(self.varint(), 0);
                    assert_eq!(self.varint(), *n);
                }
            }
            (1, IssueJournal::Refusee { avis }) => {
                assert_eq!(self.tranche(), avis.as_slice());
            }
            (2, IssueJournal::Ignoree) => {}
            (3, IssueJournal::Ecartee { faute }) => {
                let Faute::Panique { message } = faute else {
                    panic!()
                };
                assert_eq!(self.varint(), 2, "Faute::Panique");
                assert_eq!(self.tranche(), message.as_bytes());
            }
            (autre, i) => panic!("issue {autre} pour {i:?}"),
        }
    }

    /// Un état du cœur sans appareil, sans tenu ni demande, d'après sa forme : rend
    /// `(partie, pas, ordre, réveil)`.
    fn coeur_vide(&mut self) -> (u64, u64, u64, u64) {
        let partie = self.varint();
        let pas = self.varint();
        let ordre = self.varint();
        let _sequence = self.varint();
        let _synthese = self.varint();
        let reveil = self.varint();
        if reveil == 1 {
            let _a = self.varint();
        }
        let _saut = self.varint();
        for _ in 0..2 {
            assert!(self.octet() <= 1, "en_pause, relacher_pour_pause");
        }
        let _temps_ecartes = self.octet();
        for _ in 0..2 {
            assert!(self.octet() <= 1, "en_panne, periodique_confirme");
        }
        assert_eq!(self.varint(), 0, "fiches");
        let _prochain_rang = self.varint();
        assert_eq!(self.varint(), 0, "tenus");
        assert_eq!(self.varint(), 0, "manettes");
        assert_eq!(self.varint(), 0, "demandes");
        assert_eq!(self.varint(), 0, "lancées");
        let _dernier_point = self.varint();
        assert_eq!(self.varint(), 0, "vues");
        (partie, pas, ordre, reveil)
    }
}

/// SAL-90 : le lecteur indépendant relit l'ouverture, champ à champ.
#[test]
fn une_seconde_lecture_relit_l_ouverture() {
    let ouv = ouverture_figee();
    let octets = encoder_ouverture(&ouv);
    let mut lu = Lecteur { octets: &octets };
    lu.entete(3);
    assert_eq!(lu.brut(32), ouv.graine);
    assert_eq!(lu.varint(), ouv.pas);
    assert_eq!(lu.varint(), ouv.heure);
    assert_eq!(lu.tranche(), ouv.salle.as_str().as_bytes());
    for b in [
        ouv.budgets.entree,
        ouv.budgets.lecture,
        ouv.budgets.creation,
        ouv.budgets.canonique,
    ] {
        assert_eq!(lu.varint(), b);
    }
    assert_eq!(lu.varint(), ouv.point_tous_les);
    assert!(lu.fini());
}

/// SAL-90 : le lecteur indépendant relit le journal, chaque ligne par son variant.
#[test]
fn une_seconde_lecture_relit_le_journal() {
    let journal = journal_fige();
    let octets = encoder_journal(&journal);
    let mut lu = Lecteur { octets: &octets };
    lu.entete(2);
    assert_eq!(lu.varint(), 3);
    assert_eq!(lu.varint(), journal.pas().numero());
    assert_eq!(lu.varint(), u64::try_from(journal.lignes().len()).unwrap());
    let mut variants = Vec::new();
    for ligne in journal.lignes() {
        let variant = lu.varint();
        variants.push(variant);
        match (variant, ligne) {
            (0, Ligne::Arrivees { pas, arrivees }) => {
                assert_eq!(lu.varint(), pas.numero());
                assert_eq!(lu.varint(), u64::try_from(arrivees.len()).unwrap());
                for a in arrivees {
                    assert_eq!(lu.varint(), a.sequence);
                    assert_eq!(lu.varint(), a.place.numero());
                    assert_eq!(lu.varint(), a.fait.pas.numero());
                    assert_eq!(lu.varint(), u64::from(a.fait.fraction));
                    assert_eq!(lu.varint(), a.recu.pas.numero());
                    assert_eq!(lu.varint(), u64::from(a.recu.fraction));
                    assert_eq!(lu.varint(), a.vu.numero());
                    assert_eq!(lu.varint(), 2, "Origine::Appareil");
                    let Origine::Appareil(rang) = a.origine else {
                        panic!()
                    };
                    assert_eq!(lu.varint(), u64::from(rang));
                    let QuoiPlace::Geste { de, octets, id } = &a.quoi else {
                        panic!()
                    };
                    assert_eq!(lu.varint(), 0, "QuoiPlace::Geste");
                    assert_eq!(lu.varint(), de.fil());
                    assert_eq!(lu.tranche(), octets.as_slice());
                    assert_eq!(lu.varint(), id.fil());
                }
            }
            (1, Ligne::Entree(e)) => {
                assert_eq!(lu.varint(), u64::from(e.partie));
                assert_eq!(lu.varint(), e.ordre);
                assert_eq!(lu.varint(), e.place.numero());
                assert_eq!(lu.tranche(), e.octets.as_slice());
                lu.issue(&e.issue);
            }
            (
                2,
                Ligne::Empreinte {
                    partie,
                    ordre,
                    empreinte,
                },
            ) => {
                assert_eq!(lu.varint(), u64::from(*partie));
                assert_eq!(lu.varint(), *ordre);
                assert_eq!(lu.brut(32), empreinte);
            }
            (3, Ligne::Saut { de, a }) => {
                assert_eq!(lu.varint(), de.numero());
                assert_eq!(lu.varint(), a.numero());
            }
            (4, Ligne::Pause { apres, en_pause }) => {
                assert_eq!(lu.varint(), apres.numero());
                assert_eq!(lu.octet(), u8::from(*en_pause != Oui::Oui));
            }
            (5, Ligne::Vue { regard, pas, apres }) => {
                let RegardFil::Appareil(n) = regard else {
                    panic!()
                };
                assert_eq!(lu.varint(), 1, "RegardFil::Appareil");
                assert_eq!(lu.varint(), *n);
                assert_eq!(lu.varint(), pas.numero());
                assert_eq!(lu.varint(), apres.numero());
            }
            (6, Ligne::Partie { numero, graine }) => {
                assert_eq!(lu.varint(), u64::from(*numero));
                assert_eq!(lu.brut(32), graine);
            }
            (7, Ligne::Reprise { apres, depuis }) => {
                assert_eq!(lu.varint(), apres.numero());
                assert_eq!(lu.varint(), u64::try_from(*depuis).unwrap());
            }
            (autre, ligne) => panic!("variant {autre} pour {ligne:?}"),
        }
    }
    assert_eq!(variants, [0, 1, 1, 1, 1, 2, 3, 4, 5, 6, 7]);
    assert!(lu.fini());
}

/// SAL-90 : le lecteur indépendant relit un point et l'état du cœur.
#[test]
fn une_seconde_lecture_relit_le_point() {
    // Le point d'une partie neuve : un seul instantané, deux points (le point, et le
    // point sain qui est aussi le dernier point à intervalle, cité deux fois).
    let mut partie = ouvrir(ReglageModele::ordinaire());
    let pt = partie.point().unwrap();
    let octets = encoder_point(&pt);
    let mut lu = Lecteur { octets: &octets };
    lu.entete(1);
    assert_eq!(lu.varint(), 1, "un seul instantané, dédoublonné");
    let mut instantane = Vec::new();
    pt.instantane().ecrire(&mut instantane);
    assert_eq!(lu.tranche(), instantane.as_slice());
    assert_eq!(lu.varint(), 2, "deux points");
    // Le point lui-même.
    assert_eq!(lu.varint(), 0, "son instantané");
    assert_eq!(lu.coeur_vide(), (0, 0, 0, 0));
    assert_eq!(lu.varint(), u64::try_from(pt.indice()).unwrap());
    assert_eq!(lu.option(), Some(1), "sain : le second point");
    assert_eq!(lu.option(), Some(1), "periodique : le même");
    // Le point sain.
    assert_eq!(lu.varint(), 0, "le même instantané");
    assert_eq!(lu.coeur_vide(), (0, 0, 0, 0));
    let _indice = lu.varint();
    assert_eq!(lu.option(), None);
    assert_eq!(lu.option(), None);
    assert!(lu.fini());

    // L'état du cœur seul, d'après la même forme.
    let octets = encoder_coeur(partie.coeur());
    let mut lu = Lecteur { octets: &octets };
    lu.entete(4);
    assert_eq!(lu.coeur_vide(), (0, 0, 0, 0));
    assert!(lu.fini());
}
