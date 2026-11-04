//! Le journal d'une soirée, rejoué entrée par entrée sur quatre instances nues, sans
//! le cœur : la copie native, deux instances du module réécrit (jumelles), et le
//! module brut, sans réécriture. Après chaque entrée, les quatre rendent les mêmes
//! octets, et leur état se compare :
//!
//! - même forme canonique pour les quatre (E9 en petit, et `reecrire_preserve_le_sens`
//!   sur `devine`, § 8.5) ;
//! - même empreinte pour les deux jumelles (`deux_instances_meme_journal_meme_empreinte`,
//!   § 8.7), égale à celle que la partie du module a notée au journal ;
//! - empreinte de la copie native égale au BLAKE3 de la forme canonique, et à celle
//!   que la partie native a notée.
//!
//! Une cinquième instance du module reprend d'un instantané pris à mi-chemin sur une
//! autre, puis rejoue la fin : elle finit avec la même empreinte (§ 4.5).
//!
//! Les lectures que le cœur a faites entre deux entrées (valider, vues, état) ne
//! changent pas l'état (D4) : le rejeu n'appelle que les écritures, et retrouve
//! quand même les empreintes que la partie a notées, après ses fautes et ses
//! restaurations comprises.

use devine::Devine;
use jeu_abi::{Appel, IssueFil, Presence, Retour, decoder, encoder};
use jeu_natif::FabriqueNative;
use moteur_wasmtime::ModuleCharge;
use partie::{Fabrique, Instance, IssueJournal, Journal, Ligne, Ouverture};

use crate::brut::Brut;
use crate::duo::TOUS;

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(a, &mut o).expect("encoder l'appel");
    o
}

/// Une instance nue, quelle qu'elle soit.
trait Nue {
    fn brut(&mut self, a: &Appel<'_>) -> Vec<u8>;

    /// La forme canonique, morceaux recollés.
    fn canonique(&mut self) -> Vec<u8> {
        let mut tout = Vec::new();
        let mut morceau = 0;
        loop {
            let r = self.brut(&Appel::Canonique { morceau });
            match decoder::<Retour<'_>>(&r).expect("un morceau") {
                Retour::Morceau { octets, suivant } => {
                    tout.extend_from_slice(octets);
                    match suivant {
                        Some(s) => morceau = s,
                        None => return tout,
                    }
                }
                autre => panic!("Canonique rend {autre:?}"),
            }
        }
    }
}

/// Le budget des appels du rejeu : celui de l'ouverture de la soirée.
const BUDGET: u64 = 1 << 30;

impl<I: Instance> Nue for I {
    fn brut(&mut self, a: &Appel<'_>) -> Vec<u8> {
        self.appeler(a.sorte(), &encode(a), BUDGET)
            .unwrap_or_else(|e| panic!("{a:?} : {e}"))
    }
}

impl Nue for Brut {
    fn brut(&mut self, a: &Appel<'_>) -> Vec<u8> {
        self.appeler(a.sorte(), &encode(a))
            .unwrap_or_else(|e| panic!("module brut, {a:?} : {e}"))
    }
}

/// Le même appel sur chacune ; rend les retours.
fn a_toutes(nues: &mut [&mut dyn Nue], a: &Appel<'_>) -> Vec<Vec<u8>> {
    nues.iter_mut().map(|n| n.brut(a)).collect()
}

fn tous_egaux(v: &[Vec<u8>], quoi: &str) {
    for (k, r) in v.iter().enumerate().skip(1) {
        assert!(
            *r == v[0],
            "{quoi} : l'instance {k} diffère de la copie native"
        );
    }
}

/// L'issue que dit un retour d'entrée, comme le journal l'écrit.
fn issue(r: &[u8]) -> IssueJournal {
    match decoder::<Retour<'_>>(r).expect("un retour d'entrée") {
        Retour::Issue(IssueFil::Acceptee { sorties, .. }) => IssueJournal::Acceptee { sorties },
        Retour::Issue(IssueFil::Refusee { avis }) => IssueJournal::Refusee {
            avis: avis.to_vec(),
        },
        Retour::Issue(IssueFil::Ignoree) => IssueJournal::Ignoree,
        autre => panic!("issue inattendue : {autre:?}"),
    }
}

/// Ce que le rejeu a vérifié.
#[derive(Debug, Default)]
pub struct Constat {
    /// Les entrées rejouées sur les quatre instances.
    pub entrees: u64,
    /// Les entrées écartées par la partie, que le rejeu saute.
    pub ecartees: u64,
    /// Les empreintes notées au journal, retrouvées.
    pub empreintes: u64,
}

/// Rejoue les journaux des deux parties (égaux à leurs empreintes près).
#[expect(
    clippy::too_many_lines,
    reason = "le rejeu se lit d'un trait : création, entrées, reprise à mi-chemin"
)]
pub fn rejouer(
    ouverture: &Ouverture,
    charge: &ModuleCharge,
    jn: &Journal,
    jm: &Journal,
) -> Constat {
    let mut fabrique = charge.fabrique();
    let mut natif = FabriqueNative::<Devine>::new()
        .neuve()
        .expect("copie native");
    let mut module = fabrique.neuve().expect("instance du module");
    let mut jumelle = fabrique.neuve().expect("instance jumelle du module");
    let mut brut = Brut::new(crate::module::octets());

    // Bonjour nomme les zones du module, que la copie native n'a pas.
    let bonjours = [
        module.brut(&Appel::Bonjour),
        jumelle.brut(&Appel::Bonjour),
        brut.brut(&Appel::Bonjour),
    ];
    tous_egaux(&bonjours, "Bonjour");
    let _: Vec<u8> = natif.brut(&Appel::Bonjour);
    let mut nues: [&mut dyn Nue; 4] = [&mut natif, &mut module, &mut jumelle, &mut brut];
    tous_egaux(&a_toutes(&mut nues, &Appel::Decrire), "Decrire");
    let mut presents = Vec::new();
    encoder(&Vec::<Presence>::new(), &mut presents).expect("encoder");
    let creer = Appel::Creer {
        graine: ouverture.graine,
        pas: ouverture.pas,
        heure: ouverture.heure,
        salle: ouverture.salle,
        presents: &presents,
        garde: None,
    };
    tous_egaux(&a_toutes(&mut nues, &creer), "Creer");

    let entrees: Vec<usize> = jn
        .lignes()
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            matches!(l, Ligne::Entree(e) if !matches!(e.issue, IssueJournal::Ecartee { .. }))
        })
        .map(|(k, _)| k)
        .collect();
    let mi = entrees[entrees.len() / 2];
    let mut instantane = None;
    let mut constat = Constat::default();
    assert_eq!(jn.lignes().len(), jm.lignes().len());
    for (k, (ln, lm)) in jn.lignes().iter().zip(jm.lignes()).enumerate() {
        match (ln, lm) {
            (Ligne::Entree(e), _) if matches!(e.issue, IssueJournal::Ecartee { .. }) => {
                constat.ecartees += 1;
            }
            (Ligne::Entree(e), _) => {
                let mut nues: [&mut dyn Nue; 4] =
                    [&mut natif, &mut module, &mut jumelle, &mut brut];
                let a = Appel::Entree {
                    ordre: e.ordre,
                    place: e.place.numero(),
                    entree: &e.octets,
                };
                let r = a_toutes(&mut nues, &a);
                tous_egaux(&r, &format!("l'entrée d'ordre {}", e.ordre));
                assert_eq!(
                    issue(&r[0]),
                    e.issue,
                    "issue de l'entrée d'ordre {}",
                    e.ordre
                );
                let lectures = [
                    Appel::Etat { connues: vec![] },
                    Appel::Regarder {
                        regards: TOUS.to_vec(),
                    },
                ];
                for l in &lectures {
                    tous_egaux(
                        &a_toutes(&mut nues, l),
                        &format!("{l:?} après l'ordre {}", e.ordre),
                    );
                }
                let formes: Vec<Vec<u8>> = nues.iter_mut().map(|n| n.canonique()).collect();
                tous_egaux(
                    &formes,
                    &format!("la forme canonique après l'ordre {}", e.ordre),
                );
                let en = natif.empreinte().expect("empreinte native");
                assert_eq!(en, *blake3::hash(&formes[0]).as_bytes());
                let em = module.empreinte().expect("empreinte du module");
                assert_eq!(
                    em,
                    jumelle.empreinte().expect("empreinte de la jumelle"),
                    "deux instances, même journal : même empreinte, après l'ordre {}",
                    e.ordre
                );
                if k == mi {
                    instantane = Some(module.instantane().expect("instantané à mi-chemin"));
                }
                constat.entrees += 1;
            }
            (
                Ligne::Empreinte {
                    empreinte: n,
                    ordre,
                    ..
                },
                Ligne::Empreinte { empreinte: m, .. },
            ) => {
                assert_eq!(
                    *n,
                    natif.empreinte().expect("empreinte native"),
                    "ordre {ordre}"
                );
                assert_eq!(
                    *m,
                    module.empreinte().expect("empreinte du module"),
                    "ordre {ordre}"
                );
                constat.empreintes += 1;
            }
            (Ligne::Empreinte { .. }, autre) => {
                panic!("ligne {k} : {autre:?} contre une empreinte")
            }
            _ => {}
        }
    }

    // Une instance neuve reprend de l'instantané de mi-chemin, et rejoue la fin.
    let mut reprise = fabrique.neuve().expect("instance de reprise");
    let _: Vec<u8> = reprise.brut(&Appel::Bonjour);
    let _: Vec<u8> = reprise.brut(&Appel::Decrire);
    reprise
        .restaurer(&instantane.expect("un instantané à mi-chemin"))
        .expect("restaurer");
    for l in jn.lignes().iter().skip(mi + 1) {
        if let Ligne::Entree(e) = l
            && !matches!(e.issue, IssueJournal::Ecartee { .. })
        {
            let r = reprise.brut(&Appel::Entree {
                ordre: e.ordre,
                place: e.place.numero(),
                entree: &e.octets,
            });
            assert_eq!(issue(&r), e.issue);
        }
    }
    assert_eq!(
        reprise.empreinte().expect("empreinte de la reprise"),
        module.empreinte().expect("empreinte du module"),
        "la reprise d'un instantané finit comme l'original"
    );
    assert_eq!(reprise.canonique(), natif.canonique());
    constat
}
