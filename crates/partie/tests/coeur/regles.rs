//! Les propriétés des règles du cœur (`partie::regles`, § 8.6), éprouvées par
//! `proptest`. Chaque propriété est celle que la documentation de la règle énonce.

use jeu_abi::{
    Cle, Duree, Effet, IdEffet, Instant, NoeudFil, Oui, Pas, ReveilFil, SceneFil, Sensation,
    Signal, Son, SorteNoeud, Tenue, Unite,
};
use partie::regles::{
    self, ArriveeTenu, EtatTenu, IssueTenu, Origine, Placee, ReglesTenu, ValeurTenue,
};
use proptest::prelude::*;

fn inst(pas: u64, fraction: u16) -> Instant {
    Instant {
        pas: Pas::du_fil(pas),
        fraction,
    }
}

prop_compose! {
    fn un_instant()(pas in 0u64..10_000, fraction in any::<u16>()) -> Instant { inst(pas, fraction) }
}

// --- placer, suivre, borner_vu ----------------------------------------------

proptest! {
    /// SAL-129 : ni après la réception, ni avant la demi-latence et la marge ; rien
    /// avant le prochain pas.
    #[test]
    fn placer_borne_fait_et_place(
        fait in un_instant(),
        recu in un_instant(),
        demi in 0u64..200,
        marge in 0u64..200,
        definitif in 0u64..10_000,
    ) {
        let (f, p) = regles::placer(fait, recu, Duree::pas(demi), Duree::pas(marge), Pas::du_fil(definitif));
        prop_assert!(f <= recu);
        let plancher = recu.pas.numero().saturating_sub(demi + marge);
        prop_assert!(f.pas.numero() >= plancher.min(recu.pas.numero()));
        prop_assert!(p.numero() > definitif);
        prop_assert!(p >= f.pas);
        if fait <= recu && fait.pas.numero() > recu.pas.numero().saturating_sub(demi + marge) {
            prop_assert_eq!(f, fait, "dans les bornes, l'instant est gardé");
        }
    }

    /// Monotone : un geste annoncé plus tard n'est jamais placé plus tôt.
    #[test]
    fn placer_est_monotone(
        a in un_instant(),
        b in un_instant(),
        recu in un_instant(),
        demi in 0u64..200,
        definitif in 0u64..10_000,
    ) {
        let (tot, tard) = if a <= b { (a, b) } else { (b, a) };
        let d = Duree::pas(demi);
        let def = Pas::du_fil(definitif);
        let (f1, p1) = regles::placer(tot, recu, d, Duree::ZERO, def);
        let (f2, p2) = regles::placer(tard, recu, d, Duree::ZERO, def);
        prop_assert!(f1 <= f2);
        prop_assert!(p1 <= p2);
    }

    /// L'ordre d'un appareil : jamais avant son arrivée précédente.
    #[test]
    fn suivre_garde_l_ordre(fait in un_instant(), place in 0u64..10_000, prec in proptest::option::of((un_instant(), 0u64..10_000))) {
        let precedente = prec.map(|(f, p)| (f, Pas::du_fil(p)));
        let (f, p) = regles::suivre(fait, Pas::du_fil(place), precedente);
        if let Some((pf, pp)) = precedente {
            prop_assert!(f >= pf && p >= pp);
            prop_assert_eq!(f, fait.max(pf));
            prop_assert_eq!(p, Pas::du_fil(place).max(pp));
        } else {
            prop_assert_eq!((f, p), (fait, Pas::du_fil(place)));
        }
    }

    /// PUN-38 : le vu ne dépasse ni ce qu'on a envoyé, ni ce qu'on annonce.
    #[test]
    fn borner_vu_ne_depasse_rien(vu in any::<u64>(), derniere in any::<u64>()) {
        let b = regles::borner_vu(Pas::du_fil(vu), Pas::du_fil(derniere));
        prop_assert!(b.numero() <= vu && b.numero() <= derniere);
        prop_assert!(b.numero() == vu || b.numero() == derniere);
    }
}

// --- ordonner ----------------------------------------------------------------

fn origine() -> impl Strategy<Value = Origine> {
    prop_oneof![
        Just(Origine::Commun),
        Just(Origine::Ecran),
        (0u32..6).prop_map(Origine::Appareil),
    ]
}

fn entrees() -> impl Strategy<Value = Vec<Placee>> {
    proptest::collection::vec((0u64..4, 0u16..3, origine()), 0..40).prop_map(|v| {
        v.into_iter()
            .enumerate()
            .map(|(i, (pas, fraction, origine))| Placee {
                fait: inst(pas, fraction),
                origine,
                sequence: i as u64,
            })
            .collect()
    })
}

proptest! {
    /// Une permutation, triée par `fait`, qui garde l'ordre d'arrivée d'une même
    /// origine et ne dépend pas de l'ordre où les entrées sont données.
    #[test]
    fn ordonner_est_une_permutation_stable(v in entrees(), graine in any::<[u8; 32]>(), melange in any::<u64>()) {
        let mut a = v.clone();
        regles::ordonner(&mut a, &graine);
        let mut sequences: Vec<u64> = a.iter().map(|p| p.sequence).collect();
        sequences.sort_unstable();
        prop_assert_eq!(sequences, (0..v.len() as u64).collect::<Vec<_>>(), "rien de perdu ni de doublé");
        for w in a.windows(2) {
            prop_assert!(w[0].fait <= w[1].fait, "trié par fait");
            if w[0].fait == w[1].fait && w[0].origine == w[1].origine {
                prop_assert!(w[0].sequence < w[1].sequence, "une même origine garde son ordre");
            }
        }
        // Un autre ordre de départ donne le même résultat.
        let mut b = v.clone();
        let n = b.len();
        if n > 1 {
            let mut x = melange;
            for i in (1..n).rev() {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                b.swap(i, (x >> 33) as usize % (i + 1));
            }
        }
        regles::ordonner(&mut b, &graine);
        prop_assert_eq!(a, b);
    }

    /// CAT-27 : à instant égal, le départage ne dépend que de la graine, du pas et du
    /// rang, et il change avec la graine (il n'avantage pas toujours le même).
    #[test]
    fn departage_ne_depend_que_de_la_graine_du_pas_et_du_rang(graine in any::<[u8; 32]>(), pas in any::<u64>(), r in 0u32..1000) {
        let a = regles::departage(&graine, Pas::du_fil(pas), Origine::Appareil(r));
        prop_assert_eq!(a, regles::departage(&graine, Pas::du_fil(pas), Origine::Appareil(r)));
    }
}

/// Le mélange du départage est celui de `SplitMix64` : sa suite publiée, semée à zéro.
#[test]
fn melanger_rend_la_suite_de_splitmix64() {
    let g: u64 = 0x9e37_79b9_7f4a_7c15;
    assert_eq!(regles::melanger(g), 0xe220_a839_7b1d_cdaf);
    assert_eq!(regles::melanger(g.wrapping_mul(2)), 0x6e78_9e6a_a1b9_65f4);
    assert_eq!(regles::melanger(g.wrapping_mul(3)), 0x06c4_5d18_8009_454f);
}

/// CAT-27 : sur beaucoup de graines, aucun rang ne gagne toujours une égalité.
#[test]
fn departage_ne_favorise_aucun_rang() {
    let mut premiers = [0u32; 2];
    for g in 0u8..200 {
        let graine = [g; 32];
        let a = regles::departage(&graine, Pas::du_fil(1), Origine::Appareil(0));
        let b = regles::departage(&graine, Pas::du_fil(1), Origine::Appareil(1));
        premiers[usize::from(a > b)] += 1;
    }
    assert!(premiers[0] > 50 && premiers[1] > 50, "{premiers:?}");
}

// --- tenu, a_ce_pas, relacher --------------------------------------------------

/// Une valeur tenue de test : 0 est relâchée.
#[derive(Debug, Clone, PartialEq)]
struct V(u8);

impl ValeurTenue for V {
    fn relachee(&self) -> Option<V> {
        Some(V(0))
    }
    fn est_relachee(&self) -> Oui {
        if self.0 == 0 { Oui::Oui } else { Oui::Non }
    }
}

/// Un brouillon : ne se relâche jamais seul.
#[derive(Debug, Clone, PartialEq)]
struct Brouillon(u8);

impl ValeurTenue for Brouillon {
    fn relachee(&self) -> Option<Brouillon> {
        None
    }
    fn est_relachee(&self) -> Oui {
        Oui::Non
    }
}

fn regles_tenu() -> impl Strategy<Value = ReglesTenu> {
    (
        proptest::option::of(1u64..40),
        proptest::option::of(1u64..40),
        0u16..80,
    )
        .prop_map(|(f, a, d)| ReglesTenu {
            fraicheur: f.map(Duree::pas),
            age_max: a.map(Duree::pas),
            debit_max: d,
        })
}

/// Un événement du flux d'un tenu : une arrivée (valeur, numéro, âge) ou un pas qui passe.
#[derive(Debug, Clone)]
enum Ev {
    Arrivee { valeur: u8, numero: u64, age: u64 },
    Pas,
}

fn evenements() -> impl Strategy<Value = Vec<Ev>> {
    proptest::collection::vec(
        prop_oneof![
            3 => (0u8..4, 0u64..30, 0u64..50).prop_map(|(valeur, numero, age)| Ev::Arrivee { valeur, numero, age }),
            2 => Just(Ev::Pas),
        ],
        0..80,
    )
}

proptest! {
    /// REM-13, REM-14, SQ-02, PUN-03 : les propriétés de `tenu` et de `a_ce_pas` sur un
    /// flux quelconque d'arrivées et de pas.
    #[test]
    fn un_tenu_tient_ses_promesses(regles in regles_tenu(), evs in evenements()) {
        let mut e: EtatTenu<V> = EtatTenu::neuf();
        let mut pas = 100u64;
        let mut nouvelle = pas;
        let mut derniere_livraison: Option<u64> = None;
        let mut numero_max: Option<u64> = None;
        // L'instant le plus récent d'une arrivée acceptée : la borne de l'âge (SQ-02).
        let mut fait_tenu: Option<u64> = None;
        for ev in evs {
            let avant = e.clone();
            let livraison = match ev {
                Ev::Arrivee { valeur, numero, age } => {
                    nouvelle = pas;
                    let fait = Instant::de(Pas::du_fil(pas.saturating_sub(age)));
                    let issue = regles::tenu(&mut e, ArriveeTenu { valeur: V(valeur), numero, fait }, &regles, Pas::du_fil(pas));
                    // Le numéro ne fait que monter, et un ancien numéro ne change rien.
                    if numero_max.is_some_and(|n| numero <= n) {
                        prop_assert_eq!(&issue, &IssueTenu::Ancienne);
                        prop_assert_eq!(&e, &avant);
                    } else {
                        numero_max = Some(numero);
                        prop_assert_eq!(e.numero, Some(numero));
                        if !matches!(issue, IssueTenu::TropVieille) {
                            let f = pas.saturating_sub(age);
                            fait_tenu = Some(fait_tenu.map_or(f, |g| g.max(f)));
                        }
                        match &issue {
                            IssueTenu::Livree(_) | IssueTenu::Retenue => {
                                // Le dernier gagne : la valeur arrivée est l'effective ou la retenue.
                                let effective = e.livre.as_ref() == Some(&V(valeur));
                                let retenue = e.retenue.as_ref().map(|r| &r.0) == Some(&V(valeur));
                                prop_assert!(effective || retenue);
                            }
                            IssueTenu::Inchangee => {
                                prop_assert_eq!(e.livre.as_ref(), Some(&V(valeur)));
                                prop_assert!(e.retenue.is_none(), "un état égal plus récent jette le retenu");
                            }
                            IssueTenu::TropVieille => {
                                prop_assert!(valeur != 0, "un relâchement n'est jamais trop vieux");
                                prop_assert!(regles.age_max.is_some_and(|a| age > a.en_pas()));
                            }
                            IssueTenu::Ancienne => prop_assert!(false, "numéro neuf dit ancien"),
                        }
                        if valeur == 0 && avant.livre != Some(V(0)) {
                            prop_assert!(matches!(issue, IssueTenu::Livree(_)), "un relâchement qui change passe tout de suite");
                        }
                    }
                    issue.livraison()
                }
                Ev::Pas => {
                    pas += 1;
                    let l = regles::a_ce_pas(&mut e, &regles, Pas::du_fil(pas), Pas::du_fil(nouvelle));
                    if let Some(l) = &l
                        && l.cause == jeu_abi::CauseTenu::Silence
                    {
                        // Les deux bornes sont strictes (B6) ; l'âge se compte depuis la
                        // dernière redite acceptée (SQ-02, B4).
                        let tue = regles.fraicheur.is_some_and(|f| pas - nouvelle > f.en_pas());
                        let vieille = regles
                            .age_max
                            .is_some_and(|a| fait_tenu.is_some_and(|f| pas - f > a.en_pas()));
                        prop_assert!(tue || vieille);
                        prop_assert_eq!(&l.valeur, &V(0));
                    }
                    l
                }
            };
            if let Some(l) = livraison {
                prop_assert!(avant.livre.as_ref() != Some(&l.valeur), "rien d'égal à l'effective n'est livré");
                prop_assert_eq!(e.livre.as_ref(), Some(&l.valeur));
                // Deux livraisons non relâchées sont séparées de l'intervalle du débit,
                // compté depuis la livraison précédente, quelle qu'elle soit.
                if l.valeur != V(0)
                    && let Some(d) = derniere_livraison
                {
                    prop_assert!(pas - d >= regles.intervalle(), "débit tenu");
                }
                derniere_livraison = Some(pas);
            }
        }
    }

    /// Une valeur retenue part une fois, au plus tard quand le débit le permet.
    #[test]
    fn la_valeur_retenue_part_une_fois(debit in 1u16..60, valeurs in proptest::collection::vec(1u8..5, 1..10)) {
        let regles = ReglesTenu { fraicheur: None, age_max: None, debit_max: debit };
        let mut e: EtatTenu<V> = EtatTenu::neuf();
        let p = Pas::du_fil(10);
        for (n, v) in valeurs.iter().enumerate() {
            let _: IssueTenu<V> = regles::tenu(&mut e, ArriveeTenu { valeur: V(*v), numero: n as u64, fait: Instant::de(p) }, &regles, p);
        }
        let derniere = V(*valeurs.last().unwrap());
        let mut livrees = 0;
        for s in 11..(11 + regles.intervalle() + 5) {
            if regles::a_ce_pas(&mut e, &regles, Pas::du_fil(s), p).is_some() {
                livrees += 1;
            }
        }
        prop_assert!(livrees <= 1);
        prop_assert_eq!(e.livre, Some(derniere), "le dernier gagne");
    }
}

/// Besoin 9 des jeux (Sidequest B4, SQ-02) : une valeur faite en retard, mais plus
/// jeune que `age_max`, ne tient que jusqu'à cet âge, même si l'appareil parle encore ;
/// une redite de la même valeur repousse la borne.
#[test]
fn une_valeur_tardive_ne_tient_que_jusqu_a_son_age() {
    let regles = ReglesTenu {
        fraicheur: Some(Duree::pas(27)),
        age_max: Some(Duree::pas(60)),
        debit_max: 60,
    };
    let mut v: EtatTenu<V> = EtatTenu::neuf();
    // Faite au pas 100, reçue au pas 150 : 50 pas de retard, sous l'âge permis.
    let issue = regles::tenu(
        &mut v,
        ArriveeTenu {
            valeur: V(3),
            numero: 1,
            fait: Instant::de(Pas::du_fil(100)),
        },
        &regles,
        Pas::du_fil(150),
    );
    assert!(matches!(issue, IssueTenu::Livree(_)));
    // L'appareil parle encore (la fraîcheur ne joue pas) : la valeur tient jusqu'au
    // pas 160, et se relâche au 161ᵉ.
    for s in 151..=160 {
        assert_eq!(
            regles::a_ce_pas(&mut v, &regles, Pas::du_fil(s), Pas::du_fil(s)),
            None,
            "{s}"
        );
    }
    let l = regles::a_ce_pas(&mut v, &regles, Pas::du_fil(161), Pas::du_fil(161)).unwrap();
    assert_eq!((l.valeur, l.cause), (V(0), jeu_abi::CauseTenu::Silence));
    // Une redite fraîche de la même valeur passe de nouveau, et repousse la borne.
    let issue = regles::tenu(
        &mut v,
        ArriveeTenu {
            valeur: V(3),
            numero: 2,
            fait: Instant::de(Pas::du_fil(170)),
        },
        &regles,
        Pas::du_fil(171),
    );
    assert!(matches!(issue, IssueTenu::Livree(_)));
    let _: IssueTenu<V> = regles::tenu(
        &mut v,
        ArriveeTenu {
            valeur: V(3),
            numero: 3,
            fait: Instant::de(Pas::du_fil(200)),
        },
        &regles,
        Pas::du_fil(201),
    );
    assert_eq!(
        regles::a_ce_pas(&mut v, &regles, Pas::du_fil(240), Pas::du_fil(240)),
        None
    );
    assert!(regles::a_ce_pas(&mut v, &regles, Pas::du_fil(261), Pas::du_fil(261)).is_some());
}

/// Besoin 10 des jeux (Sidequest B6, SQ-01) : à `fraicheur` pile de silence, la valeur
/// tient encore ; elle se relâche au pas suivant, comme dans le jeu d'origine.
#[test]
fn la_fraicheur_est_comprise() {
    let regles = ReglesTenu {
        fraicheur: Some(Duree::ms(450)),
        age_max: None,
        debit_max: 60,
    };
    assert_eq!(Duree::ms(450).en_pas(), 27);
    let mut v: EtatTenu<V> = EtatTenu::neuf();
    let _: IssueTenu<V> = regles::tenu(
        &mut v,
        ArriveeTenu {
            valeur: V(3),
            numero: 1,
            fait: Instant::de(Pas::du_fil(10)),
        },
        &regles,
        Pas::du_fil(10),
    );
    assert_eq!(
        regles::a_ce_pas(&mut v, &regles, Pas::du_fil(37), Pas::du_fil(10)),
        None
    );
    assert!(regles::a_ce_pas(&mut v, &regles, Pas::du_fil(38), Pas::du_fil(10)).is_some());
}

/// SQ-01 : un brouillon ne se relâche jamais seul ; une valeur, une seule fois.
#[test]
fn le_relachement_par_silence_est_unique_et_ne_touche_pas_un_brouillon() {
    let regles = ReglesTenu {
        fraicheur: Some(Duree::pas(5)),
        age_max: None,
        debit_max: 60,
    };
    let mut b: EtatTenu<Brouillon> = EtatTenu::neuf();
    let _: IssueTenu<Brouillon> = regles::tenu(
        &mut b,
        ArriveeTenu {
            valeur: Brouillon(3),
            numero: 0,
            fait: Instant::de(Pas::ZERO),
        },
        &regles,
        Pas::ZERO,
    );
    for s in 1..50 {
        assert!(regles::a_ce_pas(&mut b, &regles, Pas::du_fil(s), Pas::ZERO).is_none());
    }
    let mut v: EtatTenu<V> = EtatTenu::neuf();
    let _: IssueTenu<V> = regles::tenu(
        &mut v,
        ArriveeTenu {
            valeur: V(3),
            numero: 0,
            fait: Instant::de(Pas::ZERO),
        },
        &regles,
        Pas::ZERO,
    );
    let n = (1..50)
        .filter(|s| regles::a_ce_pas(&mut v, &regles, Pas::du_fil(*s), Pas::ZERO).is_some())
        .count();
    assert_eq!(n, 1);
    assert_eq!(v.livre, Some(V(0)));
    // Un relâchement pour départ ou pause : rien si c'est déjà relâché.
    assert!(regles::relacher(&mut v, jeu_abi::CauseTenu::Depart, Pas::du_fil(60)).is_none());
}

/// `livraison` rend la livraison d'une issue livrée, et rien sinon.
#[test]
fn l_issue_d_un_tenu_dit_sa_livraison() {
    let l = regles::Livraison {
        valeur: V(2),
        cause: jeu_abi::CauseTenu::Geste,
        fait: Instant::de(Pas::ZERO),
    };
    assert_eq!(IssueTenu::Livree(l.clone()).livraison(), Some(l));
    for i in [
        IssueTenu::Retenue,
        IssueTenu::Inchangee,
        IssueTenu::Ancienne,
        IssueTenu::TropVieille,
    ] {
        assert_eq!(i.livraison(), None::<regles::Livraison<V>>);
    }
}

/// Un débit de 0 se lit 1 par seconde : un tenu finit toujours par passer.
#[test]
fn debit_nul_reste_un_debit() {
    let r = ReglesTenu {
        fraicheur: None,
        age_max: None,
        debit_max: 0,
    };
    assert_eq!(r.intervalle(), 60);
    let r = ReglesTenu { debit_max: 7, ..r };
    assert_eq!(r.intervalle(), 9);
    let r = ReglesTenu {
        debit_max: 200,
        ..r
    };
    assert_eq!(r.intervalle(), 1);
}

// --- cible, reveil -------------------------------------------------------------

proptest! {
    /// CAT-15, SQ-62, CAT-14 : la boucle ne s'emballe jamais, et le numéro suit l'horloge.
    #[test]
    fn cible_ne_depasse_jamais_le_rattrapage(def in 0u64..1000, avance in 0u64..20, voulu in 0u64..5000, r in any::<u8>()) {
        let tete = def + avance;
        let (c, saut) = regles::cible(Pas::du_fil(def), Pas::du_fil(tete), Pas::du_fil(voulu), r);
        let plafond = u64::from(r.max(1));
        prop_assert!(c.numero() >= tete);
        prop_assert!(c.numero() <= tete + plafond);
        prop_assert_eq!(c.numero() + saut.en_pas(), voulu.max(tete));
        if voulu.saturating_sub(tete) <= plafond {
            prop_assert_eq!(saut, Duree::ZERO);
            prop_assert_eq!(c.numero(), voulu.max(tete));
        }
    }

    /// Le même temps découpé en morceaux sous le plafond donne les mêmes pas (CAT-14).
    #[test]
    fn decouper_sous_le_plafond_ne_saute_rien(morceaux in proptest::collection::vec(0u64..4, 0..30)) {
        let mut tete = 0u64;
        for m in morceaux {
            let (c, saut) = regles::cible(Pas::du_fil(tete), Pas::du_fil(tete), Pas::du_fil(tete + m), 3);
            prop_assert_eq!(saut, Duree::ZERO);
            prop_assert_eq!(c.numero(), tete + m);
            tete = c.numero();
        }
    }

    #[test]
    fn le_reveil_dit_quand(pas in any::<u64>(), e in any::<u64>()) {
        prop_assert_eq!(regles::reveil_demande(ReveilFil::ChaquePas, Pas::du_fil(pas)), Oui::Oui);
        prop_assert_eq!(regles::reveil_demande(ReveilFil::Jamais, Pas::du_fil(pas)), Oui::Non);
        let oui = if pas >= e { Oui::Oui } else { Oui::Non };
        prop_assert_eq!(regles::reveil_demande(ReveilFil::A(e), Pas::du_fil(pas)), oui);
    }
}

// --- demandes ------------------------------------------------------------------

proptest! {
    /// TL-44 à TL-46 : exactement les nouvelles, une fois chacune ; exactement les retirées.
    #[test]
    fn demandes_est_exacte(en_cours in proptest::collection::vec(0u64..20, 0..10), listees in proptest::collection::vec(0u64..20, 0..15)) {
        let (lancer, annuler) = regles::demandes(&en_cours, &listees);
        let mut attendu = Vec::new();
        for id in &listees {
            if !en_cours.contains(id) && !attendu.contains(id) {
                attendu.push(*id);
            }
        }
        prop_assert_eq!(lancer, attendu);
        let attendu: Vec<u64> = en_cours.iter().filter(|id| !listees.contains(id)).copied().collect();
        prop_assert_eq!(annuler, attendu);
    }
}

// --- fusionner -----------------------------------------------------------------

fn noeud(cle: u64, parent: Option<u32>, sorte: SorteNoeud) -> NoeudFil {
    NoeudFil::simple(Cle::du_fil(cle), parent, sorte)
}

fn rect(cle: u64, parent: Option<u32>) -> NoeudFil {
    noeud(
        cle,
        parent,
        SorteNoeud::Rect {
            r: jeu_abi::Rect {
                x: jeu_abi::Fixe::unites(0),
                y: jeu_abi::Fixe::unites(0),
                l: jeu_abi::Fixe::unites(1),
                h: jeu_abi::Fixe::unites(1),
            },
            couleur: jeu_abi::Rgba::NOIR,
        },
    )
}

fn effet(id: u64, tenue: Tenue) -> Effet {
    Effet {
        id: IdEffet::de(&id),
        quand: Instant::de(Pas::ZERO),
        quoi: Sensation::Son(Son::Signal(Signal::Debut)),
        tenue,
        peremption: Duree::pas(1),
    }
}

/// L'arbre d'une scène, en profondeur d'abord : chaque nœud avec sa profondeur, ses
/// frères dans leur ordre. Deux listes à plat du même arbre donnent le même.
fn arbre(s: &SceneFil) -> Vec<(usize, NoeudFil)> {
    fn descendre(
        s: &SceneFil,
        parent: Option<u32>,
        profondeur: usize,
        v: &mut Vec<(usize, NoeudFil)>,
    ) {
        for (i, n) in s.noeuds.iter().enumerate() {
            if n.parent == parent {
                let mut sans_parent = n.clone();
                sans_parent.parent = None;
                v.push((profondeur, sans_parent));
                descendre(s, Some(u32::try_from(i).unwrap()), profondeur + 1, v);
            }
        }
    }
    let mut v = Vec::new();
    descendre(s, None, 0, &mut v);
    v
}

/// CAT-41 : la fusion se fait par clé, jamais par position.
#[test]
fn fusion_par_cle_jamais_par_position() {
    // La tête : aux résultats, le sous-arbre définitif 50 en troisième position,
    // avec un enfant de la tête qui ne doit pas paraître.
    let mut tete = SceneFil::vide();
    tete.noeuds = vec![
        rect(1, None),
        rect(2, None),
        noeud(50, None, SorteNoeud::Definitif),
        rect(666, Some(2)),
    ];
    // La définitive : encore à la question, le sous-arbre 50 en première position,
    // avec deux enfants, dont un groupe qui a lui-même un enfant.
    let mut definitive = SceneFil::vide();
    definitive.noeuds = vec![
        noeud(50, None, SorteNoeud::Definitif),
        noeud(
            51,
            Some(0),
            SorteNoeud::Groupe {
                decalage: (jeu_abi::Fixe::unites(0), jeu_abi::Fixe::unites(0)),
            },
        ),
        rect(52, Some(1)),
        rect(3, None),
        rect(53, Some(0)),
    ];
    let f = regles::fusionner(&tete, &definitive);
    let cles: Vec<(u64, Option<u32>)> = f.noeuds.iter().map(|n| (n.cle.fil(), n.parent)).collect();
    assert_eq!(
        cles,
        vec![
            (1, None),
            (2, None),
            (50, None),
            (51, Some(2)),
            (52, Some(3)),
            (53, Some(2))
        ]
    );
    assert!(f.verifier_structure().is_ok());
    // Absent de la définitive : le sous-arbre est vide.
    let f = regles::fusionner(&tete, &SceneFil::vide());
    let cles: Vec<u64> = f.noeuds.iter().map(|n| n.cle.fil()).collect();
    assert_eq!(cles, vec![1, 2, 50]);
}

/// CAT-41 : un sous-arbre définitif se retrouve par son chemin de clés : la même clé
/// sous un autre parent n'est pas le même sous-arbre.
#[test]
fn fusion_par_chemin_de_cles() {
    let groupe = |cle, parent| {
        noeud(
            cle,
            parent,
            SorteNoeud::Groupe {
                decalage: (jeu_abi::Fixe::unites(0), jeu_abi::Fixe::unites(0)),
            },
        )
    };
    // La tête : le définitif 50 sous le groupe 10.
    let mut tete = SceneFil::vide();
    tete.noeuds = vec![groupe(10, None), noeud(50, Some(0), SorteNoeud::Definitif)];
    // La définitive : un définitif 50 sous le groupe 20, puis le bon, sous 10.
    let mut definitive = SceneFil::vide();
    definitive.noeuds = vec![
        groupe(20, None),
        noeud(50, Some(0), SorteNoeud::Definitif),
        rect(98, Some(1)),
        groupe(10, None),
        noeud(50, Some(3), SorteNoeud::Definitif),
        rect(99, Some(4)),
    ];
    let f = regles::fusionner(&tete, &definitive);
    let cles: Vec<(u64, Option<u32>)> = f.noeuds.iter().map(|n| (n.cle.fil(), n.parent)).collect();
    assert_eq!(cles, vec![(10, None), (50, Some(0)), (99, Some(1))]);
}

/// § 4.4 : les effets `Definitive` viennent de la définitive, les `Provisoire` de la tête.
#[test]
fn effet_definitive_lu_sur_la_definitive() {
    let mut tete = SceneFil::vide();
    tete.effets = vec![effet(1, Tenue::Provisoire), effet(2, Tenue::Definitive)];
    let mut definitive = SceneFil::vide();
    definitive.effets = vec![effet(3, Tenue::Provisoire), effet(4, Tenue::Definitive)];
    let f = regles::fusionner(&tete, &definitive);
    assert_eq!(
        f.effets,
        vec![effet(1, Tenue::Provisoire), effet(4, Tenue::Definitive)]
    );
}

/// § 4.4 : un effet provisoire que la tête ne montre plus disparaît de la scène fusionnée.
#[test]
fn effet_provisoire_retire_annule() {
    let mut tete = SceneFil::vide();
    tete.effets = vec![effet(1, Tenue::Provisoire)];
    let definitive = SceneFil::vide();
    assert_eq!(regles::fusionner(&tete, &definitive).effets.len(), 1);
    let tete_suivante = SceneFil::vide();
    assert_eq!(
        regles::fusionner(&tete_suivante, &definitive).effets,
        Vec::new()
    );
}

proptest! {
    /// `fusionner(s, s)` rend `s`, à l'ordre des effets près, et reste bien formée.
    #[test]
    fn fusionner_une_scene_avec_elle_meme_la_rend(
        parents in proptest::collection::vec((any::<bool>(), any::<u8>(), any::<bool>()), 0..20),
        effets in proptest::collection::vec(any::<bool>(), 0..5),
    ) {
        let mut s = SceneFil::vide();
        for (i, (racine, p, definitif)) in parents.into_iter().enumerate() {
            let conteneurs: Vec<u32> = s.noeuds.iter().enumerate()
                .filter(|(_, n)| matches!(n.sorte, SorteNoeud::Groupe { .. } | SorteNoeud::Definitif))
                .map(|(j, _)| u32::try_from(j).unwrap()).collect();
            let parent = if racine || conteneurs.is_empty() { None } else { Some(conteneurs[usize::from(p) % conteneurs.len()]) };
            let sorte = if definitif { SorteNoeud::Definitif } else { SorteNoeud::Groupe { decalage: (jeu_abi::Fixe::unites(0), jeu_abi::Fixe::unites(0)) } };
            s.noeuds.push(noeud(i as u64, parent, sorte));
        }
        for (i, provisoire) in effets.into_iter().enumerate() {
            s.effets.push(effet(i as u64, if provisoire { Tenue::Provisoire } else { Tenue::Definitive }));
        }
        let f = regles::fusionner(&s, &s);
        prop_assert!(f.verifier_structure().is_ok());
        prop_assert_eq!(arbre(&f), arbre(&s), "le même arbre");
        let mut attendus: Vec<Effet> = s.effets.iter().filter(|e| e.tenue == Tenue::Provisoire).copied().collect();
        attendus.extend(s.effets.iter().filter(|e| e.tenue == Tenue::Definitive).copied());
        prop_assert_eq!(f.effets, attendus);
    }

    /// TL-27 : chaque unité compte comme la page la compte.
    #[test]
    fn longueur_compte_dans_son_unite(t in "\\PC{0,20}") {
        prop_assert_eq!(regles::longueur(&t, Unite::PointsDeCode), t.chars().count() as u64);
        prop_assert_eq!(regles::longueur(&t, Unite::Utf16), t.encode_utf16().count() as u64);
        prop_assert!(regles::longueur(&t, Unite::Graphemes) <= regles::longueur(&t, Unite::PointsDeCode));
        prop_assert!(regles::longueur(&t, Unite::Capitales) >= regles::longueur(&t, Unite::PointsDeCode));
    }
}

/// TL-27 : un « ß » compte deux capitales ; un drapeau, un graphème.
#[test]
fn longueur_capitales_et_graphemes() {
    assert_eq!(regles::longueur("ß", Unite::Capitales), 2);
    assert_eq!(regles::longueur("🇫🇷", Unite::Graphemes), 1);
    assert_eq!(regles::longueur("🇫🇷", Unite::PointsDeCode), 2);
    assert_eq!(regles::longueur("🇫🇷", Unite::Utf16), 4);
}

/// SAL-152 : une action admise est celle d'un bouton, d'un raccourci, de la fin d'un
/// champ ou de la fermeture d'une modale de la scène, et aucune autre.
#[test]
fn les_actions_admises_sont_celles_de_la_scene() {
    use partie::EcranArrivee;
    let rect0 = jeu_abi::Rect {
        x: jeu_abi::Fixe::unites(0),
        y: jeu_abi::Fixe::unites(0),
        l: jeu_abi::Fixe::unites(1),
        h: jeu_abi::Fixe::unites(1),
    };
    let action = |a: u8| EcranArrivee::Action(vec![a]);
    let seule = |sorte: SorteNoeud| {
        let mut s = SceneFil::vide();
        s.noeuds.push(noeud(1, None, sorte));
        s
    };
    let bouton = seule(SorteNoeud::Bouton {
        boite: rect0,
        nom: String::from("b"),
        action: vec![1],
    });
    let champ = seule(SorteNoeud::Composant(jeu_abi::ComposantVuFil::Champ {
        limite: 4,
        unite: Unite::PointsDeCode,
        lignes: jeu_abi::Lignes::Une,
        valeur: String::new(),
        exemple: String::new(),
        fin: Some(vec![2]),
    }));
    let modale = seule(SorteNoeud::Composant(jeu_abi::ComposantVuFil::Modale {
        fermer: vec![3],
    }));
    let mut raccourci = SceneFil::vide();
    raccourci.raccourcis.push(jeu_abi::RaccourciFil {
        touche: jeu_abi::CodeTouche::new("Enter"),
        mods: jeu_abi::Mods::AUCUN,
        repetition: Oui::Non,
        action: vec![4],
    });
    for (scene, bonne) in [(&bouton, 1u8), (&champ, 2), (&modale, 3), (&raccourci, 4)] {
        for a in 0u8..6 {
            let attendu = if a == bonne { Oui::Oui } else { Oui::Non };
            assert_eq!(
                regles::admise_par_la_scene(scene, &action(a)),
                attendu,
                "action {a}, attendue {bonne}"
            );
        }
    }
}

/// SAL-152 : un pointeur demande une surface ; un objet déplacé, sa zone ; des fichiers,
/// un sélecteur qui les accepte.
#[test]
fn les_composants_admis_sont_ceux_de_la_scene() {
    use partie::{ComposantArrivee, EcranArrivee};
    let rect0 = jeu_abi::Rect {
        x: jeu_abi::Fixe::unites(0),
        y: jeu_abi::Fixe::unites(0),
        l: jeu_abi::Fixe::unites(1),
        h: jeu_abi::Fixe::unites(1),
    };
    let mut s = SceneFil::vide();
    s.noeuds
        .push(noeud(1, None, SorteNoeud::Surface { r: rect0 }));
    s.noeuds.push(noeud(
        2,
        None,
        SorteNoeud::Composant(jeu_abi::ComposantVuFil::Deplacables {
            zones: vec![jeu_abi::ZoneVu {
                cle: Cle::du_fil(30),
                objets: vec![Cle::du_fil(31)],
            }],
            poubelle: Some(Cle::du_fil(39)),
            seuil: jeu_abi::Fixe::unites(1),
            bord: jeu_abi::Fixe::unites(1),
            vitesse_max: jeu_abi::Fixe::unites(1),
            plafond_reveil: Duree::pas(1),
        }),
    ));
    s.noeuds.push(noeud(
        3,
        None,
        SorteNoeud::Composant(jeu_abi::ComposantVuFil::Selecteur {
            genres: Vec::new(),
            multiple: Oui::Non,
            depot_partout: Oui::Non,
            collage: Oui::Non,
            taille_max: 100,
        }),
    ));
    let pointeur = EcranArrivee::Pointeur(jeu_abi::Pointeur {
        id: 0,
        genre: jeu_abi::GenrePointeur::Doigt,
        bouton: 0,
        phase: jeu_abi::PhasePointeur::Appui,
        pos: (jeu_abi::Fixe::unites(0), jeu_abi::Fixe::unites(0)),
    });
    assert_eq!(regles::admise_par_la_scene(&s, &pointeur), Oui::Oui);
    let deplace = |objet: u64, zone: u64| {
        EcranArrivee::Composant(ComposantArrivee::Deplace(jeu_abi::Deplace::Depose {
            objet: Cle::du_fil(objet),
            zone: Cle::du_fil(zone),
            indice: 0,
        }))
    };
    assert_eq!(regles::admise_par_la_scene(&s, &deplace(31, 30)), Oui::Oui);
    assert_eq!(
        regles::admise_par_la_scene(&s, &deplace(31, 39)),
        Oui::Oui,
        "la poubelle est une zone"
    );
    assert_eq!(
        regles::admise_par_la_scene(&s, &deplace(32, 30)),
        Oui::Non,
        "objet inconnu"
    );
    assert_eq!(
        regles::admise_par_la_scene(&s, &deplace(31, 33)),
        Oui::Non,
        "zone inconnue"
    );
    let annule = |objet| {
        EcranArrivee::Composant(ComposantArrivee::Deplace(jeu_abi::Deplace::Annule {
            objet: Cle::du_fil(objet),
        }))
    };
    assert_eq!(regles::admise_par_la_scene(&s, &annule(31)), Oui::Oui);
    assert_eq!(regles::admise_par_la_scene(&s, &annule(30)), Oui::Non);
    let fichiers = |tailles: &[u64]| {
        EcranArrivee::Composant(ComposantArrivee::Fichiers {
            geste: jeu_abi::IdGeste::du_fil(0),
            fichiers: tailles
                .iter()
                .map(|t| jeu_abi::FichierRecu {
                    id: jeu_abi::IdFichier::du_fil(0),
                    nom: jeu_abi::TexteCourt::new("f"),
                    genre: jeu_abi::TexteCourt::new("image/png"),
                    taille: *t,
                })
                .collect(),
        })
    };
    assert_eq!(regles::admise_par_la_scene(&s, &fichiers(&[100])), Oui::Oui);
    assert_eq!(
        regles::admise_par_la_scene(&s, &fichiers(&[101])),
        Oui::Non,
        "trop gros"
    );
    assert_eq!(
        regles::admise_par_la_scene(&s, &fichiers(&[1, 1])),
        Oui::Non,
        "un seul s'il n'est pas multiple"
    );
}

/// SAL-152 : une scène vide n'admet aucune entrée de l'écran.
#[test]
fn une_scene_vide_n_admet_rien() {
    use partie::{ComposantArrivee, EcranArrivee};
    let vide = SceneFil::vide();
    let entrees = [
        EcranArrivee::Action(vec![0]),
        EcranArrivee::Touche(jeu_abi::Touche {
            code: jeu_abi::CodeTouche::new("Space"),
            mods: jeu_abi::Mods::AUCUN,
            phase: jeu_abi::PhaseTouche::Enfoncee,
            repetition: Oui::Non,
        }),
        EcranArrivee::Pointeur(jeu_abi::Pointeur {
            id: 0,
            genre: jeu_abi::GenrePointeur::Souris,
            bouton: 0,
            phase: jeu_abi::PhasePointeur::Appui,
            pos: (jeu_abi::Fixe::unites(0), jeu_abi::Fixe::unites(0)),
        }),
        EcranArrivee::Composant(ComposantArrivee::Deplace(jeu_abi::Deplace::Annule {
            objet: Cle::du_fil(1),
        })),
        EcranArrivee::Composant(ComposantArrivee::Fichiers {
            geste: jeu_abi::IdGeste::du_fil(0),
            fichiers: Vec::new(),
        }),
    ];
    for e in &entrees {
        assert_eq!(regles::admise_par_la_scene(&vide, e), Oui::Non, "{e:?}");
    }
    // Le clavier brut demandé admet une touche, mais pas une répétition non demandée.
    let mut s = SceneFil::vide();
    s.clavier_brut = jeu_abi::ClavierBrut::Oui {
        repetition: Oui::Non,
    };
    assert_eq!(regles::admise_par_la_scene(&s, &entrees[1]), Oui::Oui);
    let EcranArrivee::Touche(mut t) = entrees[1].clone() else {
        unreachable!()
    };
    t.repetition = Oui::Oui;
    assert_eq!(
        regles::admise_par_la_scene(&s, &EcranArrivee::Touche(t)),
        Oui::Non
    );
}

// --- valider_scene (§ 6.1, D26) ---------------------------------------------

/// Le repère des scènes de ces tests : 16 × 9 unités.
const REPERE: jeu_abi::FormeEcran = jeu_abi::FormeEcran::Scene {
    largeur: 16,
    hauteur: 9,
    zone_sure_pour_cent: 90,
    echelle_entiere: Oui::Non,
};

fn rect_en(cle: u64, parent: Option<u32>, x: i32, y: i32, l: i32, h: i32) -> NoeudFil {
    noeud(
        cle,
        parent,
        SorteNoeud::Rect {
            r: jeu_abi::Rect {
                x: jeu_abi::Fixe::unites(x),
                y: jeu_abi::Fixe::unites(y),
                l: jeu_abi::Fixe::unites(l),
                h: jeu_abi::Fixe::unites(h),
            },
            couleur: jeu_abi::Rgba::NOIR,
        },
    )
}

fn groupe_en(cle: u64, parent: Option<u32>, dx: i32, dy: i32) -> NoeudFil {
    noeud(
        cle,
        parent,
        SorteNoeud::Groupe {
            decalage: (jeu_abi::Fixe::unites(dx), jeu_abi::Fixe::unites(dy)),
        },
    )
}

fn scene_de(noeuds: Vec<NoeudFil>) -> SceneFil {
    let mut s = SceneFil::vide();
    s.noeuds = noeuds;
    s
}

fn valider(s: &SceneFil) -> Result<(), regles::DefautScene> {
    regles::valider_scene(s, &REPERE, &[], &[])
}

/// SAL-165, CAT-41 : deux frères n'ont jamais la même clé ; la même clé sous deux
/// parents différents est permise.
#[test]
fn scene_cle_en_double_parmi_les_freres_refusee() {
    let s = scene_de(vec![rect(1, None), rect(1, None)]);
    assert_eq!(
        valider(&s),
        Err(regles::DefautScene::CleEnDouble {
            cle: Cle::du_fil(1)
        })
    );
    let s = scene_de(vec![
        groupe_en(10, None, 0, 0),
        groupe_en(20, None, 0, 0),
        rect(1, Some(0)),
        rect(1, Some(1)),
    ]);
    assert_eq!(valider(&s), Ok(()));
    assert_eq!(valider(&SceneFil::vide()), Ok(()));
}

/// § 6.1 : chaque nœud dans le repère et sa marge, décalages des groupes compris.
#[test]
fn scene_hors_du_repere_refusee() {
    // La marge : la moitié du repère de chaque côté.
    assert_eq!(
        valider(&scene_de(vec![rect_en(1, None, -8, -4, 1, 1)])),
        Ok(())
    );
    assert_eq!(
        valider(&scene_de(vec![rect_en(1, None, 22, 0, 2, 1)])),
        Ok(())
    );
    assert_eq!(
        valider(&scene_de(vec![rect_en(1, None, 23, 0, 2, 1)])),
        Err(regles::DefautScene::HorsDuRepere {
            cle: Cle::du_fil(1)
        })
    );
    // Chaque bord, et non le seul bord droit : à gauche, en haut, en bas. Le repère
    // fait 16 × 9 et sa marge la moitié : x dans [-8, 24], y dans [-4,5 ; 13,5].
    for (x, y, l, h) in [(-9, 0, 1, 1), (0, -5, 1, 1), (0, 13, 1, 1), (0, 12, 1, 2)] {
        assert_eq!(
            valider(&scene_de(vec![rect_en(1, None, x, y, l, h)])),
            Err(regles::DefautScene::HorsDuRepere {
                cle: Cle::du_fil(1)
            }),
            "({x}, {y}, {l}, {h})"
        );
    }
    assert_eq!(
        valider(&scene_de(vec![rect_en(1, None, 0, 12, 1, 1)])),
        Ok(())
    );
    // Le même nœud, dedans, sort par le décalage de ses groupes.
    let s = scene_de(vec![
        groupe_en(10, None, 10, 0),
        groupe_en(11, Some(0), 10, 0),
        rect_en(1, Some(1), 4, 0, 1, 1),
    ]);
    assert_eq!(
        valider(&s),
        Err(regles::DefautScene::HorsDuRepere {
            cle: Cle::du_fil(1)
        })
    );
    // Une taille négative, en largeur comme en hauteur.
    for (l, h) in [(-1, 1), (1, -1)] {
        assert_eq!(
            valider(&scene_de(vec![rect_en(1, None, 2, 2, l, h)])),
            Err(regles::DefautScene::TailleNegative {
                cle: Cle::du_fil(1)
            })
        );
    }
    // Une page n'a pas de bas.
    let page = jeu_abi::FormeEcran::Page { largeur_max: 40 };
    let s = scene_de(vec![rect_en(1, None, 0, 5000, 40, 1)]);
    assert_eq!(regles::valider_scene(&s, &page, &[], &[]), Ok(()));
}

/// § 6.1 : profondeur et nombre de nœuds bornés.
#[test]
fn scene_trop_profonde_ou_trop_grande_refusee() {
    let mut noeuds = vec![groupe_en(0, None, 0, 0)];
    for k in 1..=regles::PROFONDEUR_MAX as u64 {
        noeuds.push(groupe_en(k, Some(u32::try_from(k - 1).unwrap()), 0, 0));
    }
    assert_eq!(
        valider(&scene_de(noeuds)),
        Err(regles::DefautScene::TropProfonde {
            cle: Cle::du_fil(regles::PROFONDEUR_MAX as u64)
        })
    );
    let beaucoup = (0..=regles::NOEUDS_MAX as u64)
        .map(|k| rect(k, None))
        .collect();
    assert!(matches!(
        valider(&scene_de(beaucoup)),
        Err(regles::DefautScene::TropDeNoeuds { .. })
    ));
}

/// § 6.1, RESTE-PHASE-1 (point 25, cargo-mutants) : les bornes elles-mêmes sont
/// permises. Exactement `NOEUDS_MAX` nœuds, exactement `PROFONDEUR_MAX` étages, une
/// taille nulle, et un nœud qui touche le bord de la marge de chaque côté passent ;
/// un 1/256 d'unité plus loin, non.
#[test]
fn scene_aux_bornes_exactes_acceptee() {
    let juste = (0..regles::NOEUDS_MAX as u64)
        .map(|k| rect(k, None))
        .collect();
    assert_eq!(valider(&scene_de(juste)), Ok(()));
    let mut noeuds = vec![groupe_en(0, None, 0, 0)];
    for k in 1..regles::PROFONDEUR_MAX as u64 {
        noeuds.push(groupe_en(k, Some(u32::try_from(k - 1).unwrap()), 0, 0));
    }
    assert_eq!(noeuds.len(), regles::PROFONDEUR_MAX);
    assert_eq!(valider(&scene_de(noeuds)), Ok(()));
    // Une taille nulle n'est pas négative.
    for (l, h) in [(0, 1), (1, 0), (0, 0)] {
        assert_eq!(
            valider(&scene_de(vec![rect_en(1, None, 2, 2, l, h)])),
            Ok(()),
            "({l}, {h})"
        );
    }
    // Le repère fait 16 × 9 et sa marge la moitié : x dans [-8, 24], y dans
    // [-4,5 ; 13,5]. Chaque bord touché exactement passe ; un 1/256 d'unité plus loin,
    // dehors.
    let au_bord = |x: i32, y: i32| {
        let u = jeu_abi::Fixe::PAR_UNITE;
        noeud(
            1,
            None,
            SorteNoeud::Rect {
                r: jeu_abi::Rect {
                    x: jeu_abi::Fixe::depuis_brut(x),
                    y: jeu_abi::Fixe::depuis_brut(y),
                    l: jeu_abi::Fixe::depuis_brut(u),
                    h: jeu_abi::Fixe::depuis_brut(u),
                },
                couleur: jeu_abi::Rgba::NOIR,
            },
        )
    };
    let u = jeu_abi::Fixe::PAR_UNITE;
    // (x, y) au bord, et le pas d'un 1/256 vers l'extérieur.
    for (x, y, dx, dy) in [
        (-8 * u, 0, -1, 0),
        (23 * u, 0, 1, 0),
        (0, -4 * u - u / 2, 0, -1),
        (0, 12 * u + u / 2, 0, 1),
    ] {
        assert_eq!(
            valider(&scene_de(vec![au_bord(x, y)])),
            Ok(()),
            "({x}, {y})"
        );
        assert_eq!(
            valider(&scene_de(vec![au_bord(x + dx, y + dy)])),
            Err(regles::DefautScene::HorsDuRepere {
                cle: Cle::du_fil(1)
            }),
            "({}, {})",
            x + dx,
            y + dy
        );
    }
}

/// § 6.1 : polices et images connues : la police du commun, les assets, et les images
/// que le jeu dit connaître.
#[test]
fn scene_police_ou_image_inconnue_refusee() {
    let texte = |police: u64| {
        noeud(
            1,
            None,
            SorteNoeud::Texte {
                texte: String::from("A"),
                boite: jeu_abi::Rect {
                    x: jeu_abi::Fixe::unites(0),
                    y: jeu_abi::Fixe::unites(0),
                    l: jeu_abi::Fixe::unites(1),
                    h: jeu_abi::Fixe::unites(1),
                },
                style: jeu_abi::StyleTexte {
                    police: Cle::du_fil(police),
                    taille: jeu_abi::Taille::Fixe(jeu_abi::Fixe::unites(1)),
                    couleur: jeu_abi::Rgba::NOIR,
                    alignement: jeu_abi::Alignement::Debut,
                    lignes: jeu_abi::Lignes::Une,
                    coupe: jeu_abi::Coupe::Mots,
                },
            },
        )
    };
    assert_eq!(valider(&scene_de(vec![texte(0)])), Ok(()));
    // La police du commun n'a qu'une définition, que les jeux nomment
    // (`Cle::POLICE_DU_COMMUN`) plutôt que d'en recopier le numéro.
    assert_eq!(regles::POLICE_DU_COMMUN, Cle::POLICE_DU_COMMUN);
    assert_eq!(
        valider(&scene_de(vec![texte(Cle::POLICE_DU_COMMUN.fil())])),
        Ok(())
    );
    assert_eq!(
        valider(&scene_de(vec![texte(9)])),
        Err(regles::DefautScene::PoliceInconnue {
            cle: Cle::du_fil(1),
            police: Cle::du_fil(9)
        })
    );
    assert_eq!(
        regles::valider_scene(&scene_de(vec![texte(9)]), &REPERE, &[Cle::du_fil(9)], &[]),
        Ok(())
    );
    let image = scene_de(vec![noeud(
        2,
        None,
        SorteNoeud::Image {
            image: Cle::du_fil(77),
            r: jeu_abi::Rect {
                x: jeu_abi::Fixe::unites(0),
                y: jeu_abi::Fixe::unites(0),
                l: jeu_abi::Fixe::unites(1),
                h: jeu_abi::Fixe::unites(1),
            },
            ech: jeu_abi::Echantillonnage::Lisse,
        },
    )]);
    assert_eq!(regles::images_citees(&image), vec![Cle::du_fil(77)]);
    assert!(matches!(
        valider(&image),
        Err(regles::DefautScene::ImageInconnue { .. })
    ));
    assert_eq!(
        regles::valider_scene(&image, &REPERE, &[], &[Cle::du_fil(77)]),
        Ok(())
    );
}

proptest! {
    /// Une scène valide le reste si l'on retire une feuille.
    #[test]
    fn scene_valide_sans_une_feuille(n in 1usize..30, retire in any::<prop::sample::Index>()) {
        let noeuds: Vec<NoeudFil> = (0..n as u64)
            .map(|k| rect_en(k, None, i32::try_from(k % 16).unwrap(), 0, 1, 1))
            .collect();
        prop_assert_eq!(valider(&scene_de(noeuds.clone())), Ok(()));
        let mut moins = noeuds;
        let _: NoeudFil = moins.remove(retire.index(n));
        prop_assert_eq!(valider(&scene_de(moins)), Ok(()));
    }
}

/// § 6.1 : la scène du modèle passe la validation du cœur, qui la rend à l'écran.
#[test]
fn la_scene_du_modele_est_valide() {
    let mut p = partie::Partie::ouvrir(
        partie::modele::FabriqueModele::default(),
        crate::commun::ouverture(),
    )
    .unwrap();
    let vues = p.regarder(&[jeu_abi::RegardFil::Ecran]).unwrap();
    assert_eq!(vues.len(), 1);
    assert_eq!(p.avancer(Pas::du_fil(1)).unwrap().fautes, Vec::new());
}
