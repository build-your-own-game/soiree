//! La fenêtre et la tête (§ 4.2, § 4.3, § 4.4, D12 ; § 8.9, étape 5), sur le modèle :
//! un geste en retard se place à son instant, un résultat ne s'annonce qu'une fois
//! définitif, demandes et sorties ne partent que du définitif, et l'histoire
//! définitive ne dépend pas de l'ordre d'arrivée dans la fenêtre.

use jeu_abi::{
    ApplicableFil, Duree, EntreeFil, IdGeste, MontreFil, Presence, RegardFil, SceneFil, SorteNoeud,
    Tenue, decoder,
};
use partie::modele::{
    CLE_TOTAL, CLE_TOTAL_DEFINITIF, FabriqueModele, GesteModele, ReglageModele, effet_definitif,
    effet_provisoire, texte_du_total,
};
use partie::{Arrivee, Avancee, Journal, Ligne, Partie, Placement, Tete, Vue};
use proptest::prelude::*;

use crate::commun::*;

/// Le modèle avec une fenêtre de `w` pas.
pub fn avec_fenetre(w: u64, mut r: ReglageModele) -> ReglageModele {
    r.fenetre = ApplicableFil::Oui(Duree::pas(w));
    r
}

/// Le modèle sans temps : seules les arrivées font des entrées.
pub fn dormant() -> ReglageModele {
    let mut r = ReglageModele::ordinaire();
    r.reveil = jeu_abi::ReveilFil::Jamais;
    r
}

/// Un geste fait à `fait`, reçu `retard` pas plus tard ; la demi-latence mesurée vaut
/// le retard, pour que `fait` ne soit pas relevé (SAL-129).
pub fn geste_en_retard(de: u64, g: GesteModele, fait: u64, retard: u64, id: u64) -> Arrivee {
    Arrivee::Geste {
        de: a(de),
        octets: octets(&g),
        fait: instant(fait),
        vu: pas(fait),
        id: IdGeste::du_fil(id),
        demi_aller_retour: Duree::pas(retard),
    }
}

/// L'arrivée d'un appareil.
pub fn arrivee_de(de: u64) -> Arrivee {
    Arrivee::Presence(Presence::Arrivee {
        appareil: a(de),
        role: jeu_abi::RoleAppareil::Telephone,
        capacites: jeu_abi::Capacites::AUCUNE,
    })
}

/// Avance l'horloge de `de` à `a`, un pas à la fois.
pub fn horloge(p: &mut PartieModele, de: u64, a: u64) -> Vec<Avancee> {
    (de..=a).map(|s| p.avancer(pas(s)).unwrap()).collect()
}

/// L'histoire définitive, comparable : les entrées, `quand.recu` effacé (il dit quand
/// le geste est arrivé, pas quand il a été fait), et les empreintes notées.
pub fn histoire(j: &Journal) -> Vec<Ligne> {
    j.lignes()
        .iter()
        .filter_map(|l| match l {
            Ligne::Entree(e) => {
                let mut e = e.clone();
                if let Ok(EntreeFil::Geste {
                    de,
                    octets: o,
                    quand,
                }) = decoder::<EntreeFil<'_>>(&e.octets)
                {
                    let mut q = quand;
                    q.recu = 0;
                    e.octets = octets(&EntreeFil::Geste {
                        de,
                        octets: o,
                        quand: q,
                    });
                }
                Some(Ligne::Entree(e))
            }
            Ligne::Empreinte { .. }
            | Ligne::Partie { .. }
            | Ligne::Saut { .. }
            | Ligne::Reprise { .. } => Some(l.clone()),
            Ligne::Arrivees { .. } | Ligne::Pause { .. } | Ligne::Vue { .. } => None,
        })
        .collect()
}

/// Le texte du nœud de cette clé dans une scène.
fn texte(s: &SceneFil, cle: jeu_abi::Cle) -> Option<String> {
    s.noeuds
        .iter()
        .find(|n| n.cle == cle)
        .and_then(|n| match &n.sorte {
            SorteNoeud::Texte { texte, .. } => Some(texte.clone()),
            SorteNoeud::Rect { .. }
            | SorteNoeud::Image { .. }
            | SorteNoeud::Groupe { .. }
            | SorteNoeud::Bouton { .. }
            | SorteNoeud::CompteARebours { .. }
            | SorteNoeud::Qr { .. }
            | SorteNoeud::Composant { .. }
            | SorteNoeud::Avertissements { .. }
            | SorteNoeud::Surface { .. }
            | SorteNoeud::Definitif => None,
        })
}

fn vue_de(vues: &[Vue], r: RegardFil) -> &Vue {
    vues.iter().find(|v| v.regard == r).unwrap()
}

/// § 4.3 : un geste en retard de moins que la fenêtre se place à son instant, pas à
/// sa réception.
#[test]
fn geste_en_retard_place_a_son_instant() {
    let mut p = ouvrir(avec_fenetre(6, dormant()));
    let _: Vec<Avancee> = horloge(&mut p, 1, 13);
    assert_eq!(p.definitif(), pas(7), "la définitive a six pas de retard");
    assert_eq!(p.tete(), Tete::Separee { pas: pas(13) });
    let place = p
        .arriver(
            geste_en_retard(1, GesteModele::Ajouter(1), 10, 4, 1),
            instant(14),
        )
        .unwrap();
    assert_eq!(place, Placement::Place { pas: pas(10) });
    let _: Vec<Avancee> = horloge(&mut p, 14, 16);
    assert_eq!(p.definitif(), pas(10));
    assert_eq!(entrees(p.journal()).len(), 1);
    assert_eq!(entrees(p.journal())[0].place, pas(10));
    assert_eq!(etat(&p).gestes, vec![1]);
    // Au-delà de la fenêtre, le geste se place au premier pas encore libre.
    let place = p
        .arriver(
            geste_en_retard(1, GesteModele::Ajouter(1), 5, 12, 2),
            instant(17),
        )
        .unwrap();
    assert_eq!(place, Placement::Place { pas: pas(11) });
}

/// E12 en petit (§ 8.6, `fenetre_meme_journal_definitif`) : la même soirée, gestes à
/// l'heure ou en retard dans la fenêtre, a la même histoire définitive.
#[test]
fn fenetre_meme_journal_definitif() {
    let gestes = [
        (1, 3, 0),
        (2, 3, 2),
        (3, 4, 5),
        (1, 6, 1),
        (2, 6, 3),
        (3, 6, 3),
        (2, 9, 4),
        (1, 11, 5),
    ];
    let jouer = |retards: bool| {
        let mut p = ouvrir(avec_fenetre(6, ReglageModele::ordinaire()));
        for de in 1..=3 {
            let _: Placement = p.arriver(arrivee_de(de), instant(1)).unwrap();
        }
        let _: Avancee = p.avancer(pas(1)).unwrap();
        for s in 2..=30 {
            for (k, (de, fait, retard)) in gestes.iter().enumerate() {
                let retard = if retards { *retard } else { 0 };
                if fait + retard == s {
                    let g = geste_en_retard(*de, GesteModele::Ajouter(1), *fait, retard, k as u64);
                    let _: Placement = p.arriver(g, instant(s)).unwrap();
                }
            }
            let _: Avancee = p.avancer(pas(s)).unwrap();
        }
        let e = p.empreinte().unwrap();
        (histoire(p.journal()), e, etat(&p).gestes.clone())
    };
    let a_l_heure = jouer(false);
    let en_retard = jouer(true);
    assert_eq!(a_l_heure.2.len(), gestes.len());
    assert_eq!(a_l_heure, en_retard);
}

/// D12 : un résultat ne s'annonce qu'une fois définitif. Les téléphones voient la
/// définitive ; l'écran voit la tête, sauf sous un sous-arbre `definitif` ; ses effets
/// `Provisoire` viennent de la tête, les `Definitive` de la définitive.
#[test]
fn resultat_annonce_au_definitif_seulement() {
    let mut p = ouvrir(avec_fenetre(6, dormant()));
    let _: Vec<Avancee> = horloge(&mut p, 1, 10);
    let _: Placement = p
        .arriver(
            geste_en_retard(1, GesteModele::Ajouter(5), 11, 0, 1),
            instant(11),
        )
        .unwrap();
    let _: Vec<Avancee> = horloge(&mut p, 11, 11);
    let regards = [RegardFil::Ecran, RegardFil::Appareil(1)];
    let vues = p.regarder(&regards).unwrap();
    let ecran = vue_de(&vues, RegardFil::Ecran);
    assert_eq!(ecran.pas, pas(11), "l'écran voit la tête");
    let scene = decoder::<SceneFil>(&ecran.octets).unwrap();
    assert_eq!(texte(&scene, CLE_TOTAL), Some(texte_du_total(5)));
    assert_eq!(
        texte(&scene, CLE_TOTAL_DEFINITIF),
        Some(texte_du_total(0)),
        "le sous-arbre définitif vient de la définitive"
    );
    let provisoires: Vec<_> = scene
        .effets
        .iter()
        .filter(|e| e.tenue == Tenue::Provisoire)
        .map(|e| e.id)
        .collect();
    let definitifs: Vec<_> = scene
        .effets
        .iter()
        .filter(|e| e.tenue == Tenue::Definitive)
        .map(|e| e.id)
        .collect();
    assert_eq!(provisoires, vec![effet_provisoire(5)]);
    assert_eq!(definitifs, vec![effet_definitif(0)]);
    let tel = vue_de(&vues, RegardFil::Appareil(1));
    assert_eq!(tel.pas, pas(5), "le téléphone voit la définitive");
    let montre = decoder::<MontreFil<'_>>(&tel.octets).unwrap();
    assert_eq!(decoder::<u64>(montre.vue).unwrap(), 0);
    // Six pas plus tard, le geste est définitif : tout le monde le voit.
    let _: Vec<Avancee> = horloge(&mut p, 12, 17);
    let vues = p.regarder(&regards).unwrap();
    let scene = decoder::<SceneFil>(&vue_de(&vues, RegardFil::Ecran).octets).unwrap();
    assert_eq!(texte(&scene, CLE_TOTAL_DEFINITIF), Some(texte_du_total(5)));
    let montre = decoder::<MontreFil<'_>>(&vue_de(&vues, RegardFil::Appareil(1)).octets).unwrap();
    assert_eq!(decoder::<u64>(montre.vue).unwrap(), 5);
}

/// D12, § 4.4 : demandes et sorties ne partent que du définitif.
#[test]
fn demandes_et_sorties_du_definitif_seulement() {
    let mut p = ouvrir(avec_fenetre(6, dormant()));
    let _: Vec<Avancee> = horloge(&mut p, 1, 10);
    let _: Placement = p
        .arriver(
            geste_en_retard(1, GesteModele::Demander, 11, 0, 1),
            instant(11),
        )
        .unwrap();
    let _: Placement = p
        .arriver(
            geste_en_retard(2, GesteModele::Oublier, 11, 0, 2),
            instant(11),
        )
        .unwrap();
    let avant = horloge(&mut p, 11, 16);
    assert!(
        avant
            .iter()
            .all(|a| a.lancer.is_empty() && a.oublier.is_empty())
    );
    assert!(
        entrees(p.journal()).is_empty(),
        "rien de la tête au journal"
    );
    let au_definitif = horloge(&mut p, 17, 17);
    assert_eq!(au_definitif[0].lancer.len(), 1);
    assert_eq!(au_definitif[0].oublier, vec![a(2)]);
}

/// § 4.6 : une faute sur la tête n'est ni écrite ni rapportée ; la tête se refait
/// sans l'entrée, et la définitive la juge à son tour, une fois.
#[test]
fn faute_sur_la_tete_jugee_par_la_definitive() {
    let mut p = ouvrir(avec_fenetre(6, dormant()));
    let _: Vec<Avancee> = horloge(&mut p, 1, 10);
    let _: Placement = p
        .arriver(
            geste_en_retard(1, GesteModele::Paniquer, 11, 0, 1),
            instant(11),
        )
        .unwrap();
    let _: Placement = p
        .arriver(
            geste_en_retard(1, GesteModele::Ajouter(2), 11, 0, 2),
            instant(11),
        )
        .unwrap();
    let sur_la_tete = horloge(&mut p, 11, 16);
    assert!(sur_la_tete.iter().all(|a| a.fautes.is_empty()));
    assert_eq!(p.tete(), Tete::Separee { pas: pas(16) });
    let vues = p.regarder(&[RegardFil::Ecran]).unwrap();
    let scene = decoder::<SceneFil>(&vue_de(&vues, RegardFil::Ecran).octets).unwrap();
    assert_eq!(
        texte(&scene, CLE_TOTAL),
        Some(texte_du_total(2)),
        "la tête rejouée sans le geste fautif garde le suivant"
    );
    let definitive = horloge(&mut p, 17, 17);
    assert_eq!(definitive[0].fautes.len(), 1);
    assert_eq!(ecartees(p.journal()), 1);
    assert_eq!(etat(&p).total, 2);
}

/// § 4.4 : la tête ne repart de la définitive que si une arrivée tombe derrière elle.
#[test]
fn tete_rejouee_seulement_derriere_elle() {
    let f = FabriqueModele::new(avec_fenetre(6, dormant()));
    let c = std::rc::Rc::clone(&f.compteurs);
    let mut o = ouverture();
    o.point_tous_les = 0;
    let mut p = Partie::ouvrir(f, o).unwrap();
    let _: Vec<Avancee> = horloge(&mut p, 1, 10);
    let avant = c.restaurations.get();
    // Des présences, reçues à leur pas : la tête n'avait pas encore joué ce pas.
    for s in 11..=20 {
        let _: Placement = p.arriver(arrivee_de(s), instant(s)).unwrap();
        let _: Avancee = p.avancer(pas(s)).unwrap();
    }
    assert_eq!(c.restaurations.get(), avant, "aucun rejeu de la tête");
    let _: Placement = p
        .arriver(
            geste_en_retard(1, GesteModele::Ajouter(1), 18, 3, 1),
            instant(21),
        )
        .unwrap();
    let _: Avancee = p.avancer(pas(21)).unwrap();
    assert_eq!(c.restaurations.get(), avant + 1, "un rejeu pour un retard");
    assert_eq!(c.fabriquees.get(), 2, "la définitive et la tête");
}

/// § 4.1, D14 : un saut vide la fenêtre ; la définitive rattrape la tête, puis les
/// deux sautent ensemble.
#[test]
fn saut_vide_la_fenetre() {
    let mut p = ouvrir(avec_fenetre(6, ReglageModele::ordinaire()));
    let _: Vec<Avancee> = horloge(&mut p, 1, 10);
    assert_eq!(p.definitif(), pas(4));
    let a = p.avancer(pas(100)).unwrap();
    assert_eq!(a.saut, Duree::pas(87));
    assert_eq!(p.definitif(), pas(100));
    let _: Vec<Avancee> = horloge(&mut p, 101, 110);
    assert_eq!(p.definitif(), pas(104));
    let sauts: Vec<u64> = temps(p.journal()).into_iter().map(|(_, s)| s).collect();
    assert_eq!(sauts.iter().filter(|s| **s > 0).count(), 1);
}

/// § 4.5 : une reprise depuis un point retrouve la même histoire définitive, et la
/// tête repart d'elle.
#[test]
fn reprise_avec_fenetre() {
    let jouer = |p: &mut PartieModele, de: u64, a: u64| {
        for s in de..=a {
            if s % 3 == 0 {
                let g = geste_en_retard(1 + s % 2, GesteModele::Ajouter(1), s - 2, 2, s);
                let _: Placement = p.arriver(g, instant(s)).unwrap();
            }
            let _: Avancee = p.avancer(pas(s)).unwrap();
        }
    };
    let mut p = ouvrir(avec_fenetre(6, ReglageModele::ordinaire()));
    jouer(&mut p, 1, 40);
    let point = p.point().unwrap();
    jouer(&mut p, 41, 80);
    let mut r = Partie::reprendre(
        FabriqueModele::new(avec_fenetre(6, ReglageModele::ordinaire())),
        ouverture(),
        &point,
        p.journal(),
    )
    .unwrap();
    assert_eq!(r.definitif(), p.definitif());
    assert_eq!(r.empreinte().unwrap(), p.empreinte().unwrap());
    assert_eq!(etat(&r), etat(&p));
    // Les arrivées de la fenêtre ne sont pas au journal : la reprise ne les a pas, et
    // les appareils les renvoient (REM-13). Sans elles, la suite est la même.
    let suite = |q: &mut PartieModele| {
        let _: Vec<Avancee> = horloge(q, 81, 100);
        (histoire(q.journal()), q.empreinte().unwrap())
    };
    let mut sans = Partie::reprendre(
        FabriqueModele::new(avec_fenetre(6, ReglageModele::ordinaire())),
        ouverture(),
        &point,
        p.journal(),
    )
    .unwrap();
    assert_eq!(suite(&mut r), suite(&mut sans));
}

/// Une arrivée de la fenêtre, pour la propriété : un appareil, un instant, un retard.
#[derive(Debug, Clone, Copy)]
struct Prevu {
    de: u64,
    fait: u64,
    retard: u64,
    n: u8,
}

/// Des gestes de un à quatre appareils, chacun dans l'ordre de ses instants, et un
/// retard par geste, sous la fenêtre.
fn prevus() -> impl Strategy<Value = (u64, Vec<Prevu>, u64)> {
    (2u64..10).prop_flat_map(|w| {
        (
            Just(w),
            proptest::collection::vec((1u64..5, 0u64..4, 0u64..64, 1u8..5), 1..40),
            any::<u64>(),
        )
            .prop_map(|(w, brut, melange)| {
                let mut faits = [2u64; 5];
                let mut recus = [0u64; 5];
                let mut v = Vec::new();
                for (de, pas_en_plus, tirage, n) in brut {
                    let i = usize::try_from(de).unwrap();
                    faits[i] += pas_en_plus;
                    let fait = faits[i];
                    // Le transport livre un appareil dans l'ordre : la réception ne
                    // recule pas, et reste dans la fenêtre.
                    let recu = (fait + tirage % w).max(recus[i]).min(fait + w - 1);
                    recus[i] = recu;
                    v.push(Prevu {
                        de,
                        fait,
                        retard: recu - fait,
                        n,
                    });
                }
                (w, v, melange)
            })
    })
}

/// Joue ces gestes avec cette fenêtre : à leur réception, ou à leur instant. À une même réception, les appareils arrivent dans un ordre tiré de
/// `melange`, chacun dans son ordre.
/// Quand les gestes arrivent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Arrivees {
    /// À leur instant.
    ALHeure,
    /// À leur réception, en retard dans la fenêtre.
    EnRetard,
}

fn soiree_fenetre(w: u64, v: &[Prevu], melange: u64, quand: Arrivees) -> (Vec<Ligne>, [u8; 32]) {
    let retards = quand == Arrivees::EnRetard;
    let mut p = ouvrir(avec_fenetre(w, ReglageModele::ordinaire()));
    for de in 1..=4 {
        let _: Placement = p.arriver(arrivee_de(de), instant(1)).unwrap();
    }
    let _: Avancee = p.avancer(pas(1)).unwrap();
    let fin = v.iter().map(|x| x.fait + x.retard).max().unwrap_or(2) + w + 2;
    for s in 2..=fin {
        let mut du_pas: Vec<(usize, Prevu)> = v
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, x)| x.fait + if retards { x.retard } else { 0 } == s)
            .collect();
        // Un ordre entre appareils tiré au hasard, l'ordre de chacun gardé.
        du_pas.sort_by_key(|(k, x)| {
            (
                partie::regles::melanger(melange ^ x.de.wrapping_mul(0x9e37)),
                *k,
            )
        });
        for (k, x) in du_pas {
            let retard = if retards { x.retard } else { 0 };
            let g = geste_en_retard(x.de, GesteModele::Ajouter(x.n), x.fait, retard, k as u64);
            let _: Placement = p.arriver(g, instant(s)).unwrap();
        }
        let _: Avancee = p.avancer(pas(s)).unwrap();
    }
    let e = p.empreinte().unwrap();
    (histoire(p.journal()), e)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// § 4.3, D12 : l'histoire définitive est la même quel que soit l'ordre d'arrivée
    /// dans la fenêtre : à l'heure, ou en retard de moins que la fenêtre, dans
    /// n'importe quel ordre entre appareils.
    #[test]
    fn l_histoire_definitive_ne_depend_pas_de_l_ordre_d_arrivee((w, v, melange) in prevus()) {
        let a_l_heure = soiree_fenetre(w, &v, 0, Arrivees::ALHeure);
        let en_retard = soiree_fenetre(w, &v, melange, Arrivees::EnRetard);
        prop_assert_eq!(a_l_heure, en_retard);
    }
}
