//! La même soirée, autrement : renumérotée (E10), coupée (E11), retardée ou découpée
//! (E12).
//!
//! Une soirée sans panne ni constat (`Sorte::Proprietes`) est jouée une fois, et son
//! script ([`Op`]) gardé. Chaque contrôle en fait une variante, puis rejoue l'original
//! et la variante sur deux parties neuves du module, et compare ce que la propriété
//! promet :
//!
//! | Contrôle | La variante | Ce qui doit rester égal |
//! |---|---|---|
//! | E10 | chaque `Appareil` remplacé par une autre valeur, de même longueur | le résumé après chaque pas, et les vues, à renumérotation près (CAT-27) |
//! | E11 | un appareil coupé moins de `moins_de`, là où l'hypothèse a tenu pendant toute la coupure | le résumé final (PUN-73, SQ-28) |
//! | E12 | des gestes reçus plus tard, mais dans la fenêtre | l'histoire définitive, aux instants de réception près (§ 4.3, D12) |
//! | E12 | chaque pas découpé en tours d'une entrée, les arrivées qui suivent un pas reçues pendant ce pas | le journal, ligne pour ligne (PHASE-3, P7 ; CAT-14) |
//!
//! # Le modèle de la variante
//!
//! Rejouer un script, c'est rejouer les mêmes arrivées : les gestes faits aux mêmes
//! instants, les mêmes octets. Une coupure retient les gestes et les états tenus de
//! l'appareil jusqu'à sa fin, puis les livre dans leur ordre ; le commun annonce son
//! départ à la fermeture de la connexion (0,5 s) et son retour à la fin. Un retard
//! livre un geste plus tard, sans changer l'instant où il a été fait, ni l'ordre des
//! gestes d'un appareil.
//!
//! # Renuméroter des octets
//!
//! Un `Appareil` s'écrit en varint. Chaque appareil reçoit une valeur neuve de même
//! longueur de bits, donc de même varint : remplacer ses octets dans un résumé ou une
//! vue ne décale rien. Une clé de scène ou une identité d'effet, calculée depuis un
//! appareil, ne se renumérote pas ainsi : les vues de l'écran se comparent avec leurs
//! clés renommées par ordre d'apparition.

use std::collections::BTreeMap;

use jeu_abi::{
    Appareil, Appel, Capacites, CauseDepart, Cle, ComposantVuFil, Duree, EntreeFil, IdEffet,
    Instant, MontreFil, Oui, Presence, QuandFil, RegardFil, Retour, SceneFil, SorteNoeud, decoder,
    encoder,
};
use moteur_wasmtime::ModuleCharge;
use partie::{Arrivee, Journal, Ligne, Ouverture, Partie};

use crate::soiree::Op;
use crate::sonde::{Compte, Obs, Sonde, Verifs};
use crate::telephone::FERMETURE;
use crate::tirage::Tirage;

/// Ce qu'un rejeu du script relève.
#[derive(Debug, Default)]
pub(crate) struct Releve {
    /// Après chaque `Avancer` : la cible et le résumé.
    pub(crate) resumes: Vec<(u64, Vec<u8>)>,
    /// À chaque `Regarder` : les vues, par regard.
    pub(crate) vues: Vec<Vec<(RegardFil, Vec<u8>)>>,
    /// Le journal à la fin.
    pub(crate) journal: Option<Journal>,
    /// La forme canonique à la fin.
    pub(crate) canonique: Option<Vec<u8>>,
    /// Le résumé à la fin.
    pub(crate) resume: Option<Vec<u8>>,
    /// Les hypothèses qui tiennent : le pas, l'appareil.
    pub(crate) hypotheses: Vec<(u64, u64)>,
}

/// Ce que le rejeu relève.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Quoi {
    pub(crate) resumes: Oui,
    pub(crate) vues: Oui,
    /// Interroger l'hypothèse de la coupure transparente, tous les `n` pas.
    pub(crate) hypotheses: Option<u64>,
}

impl Quoi {
    pub(crate) const FIN: Quoi = Quoi {
        resumes: Oui::Non,
        vues: Oui::Non,
        hypotheses: None,
    };
}

fn octets<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(v, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// Les octets d'une lecture `Octets` (le résumé).
fn lire_octets(r: &[u8]) -> Option<Vec<u8>> {
    match decoder::<Retour<'_>>(r) {
        Ok(Retour::Octets(o)) => Some(o.to_vec()),
        Ok(_) | Err(_) => None,
    }
}

/// Rejoue un script sur une partie neuve du module, chaque `Avancer` d'un seul tour
/// (`Partie::avancer`) : les variantes de E10, E11 et E12 se comparent entre elles,
/// sans que le découpage d'un pas, que la soirée fait d'après l'âge des vues, ne s'en
/// mêle.
pub(crate) fn rejouer(
    module: &ModuleCharge,
    ouverture: &Ouverture,
    script: &[Op],
    quoi: Quoi,
) -> Result<Releve, String> {
    let obs = Obs::new(Verifs::RIEN, ouverture.budgets.lecture);
    let mut partie = Partie::ouvrir(Sonde::new(module.fabrique(), &obs), ouverture.clone())
        .map_err(|e| format!("la partie ne s'ouvre pas : {e:?}"))?;
    let mut r = Releve::default();
    let mut presents: Vec<u64> = Vec::new();
    for op in script {
        match op {
            Op::Arriver(a, recu) => {
                let _: partie::Placement = partie
                    .arriver(a.clone(), *recu)
                    .map_err(|p| format!("arriver : {p}"))?;
            }
            Op::Avancer(cible) => {
                let _: partie::Avancee = partie
                    .avancer(*cible)
                    .map_err(|p| format!("avancer : {p}"))?;
                if quoi.resumes == Oui::Oui {
                    let resume = partie
                        .definitive()
                        .lire(&Appel::Resumer)
                        .ok()
                        .and_then(|o| lire_octets(&o))
                        .unwrap_or_default();
                    r.resumes.push((cible.numero(), resume));
                }
                if let Some(n) = quoi.hypotheses
                    && cible.numero().checked_rem(n.max(1)) == Some(0)
                {
                    for a in &presents {
                        let tient = partie
                            .definitive()
                            .lire(&Appel::Hypothese { appareil: *a })
                            .ok()
                            .and_then(|o| match decoder::<Retour<'_>>(&o) {
                                Ok(Retour::Hypothese(t)) => Some(t),
                                Ok(_) | Err(_) => None,
                            });
                        if tient == Some(Oui::Oui) {
                            r.hypotheses.push((cible.numero(), *a));
                        }
                    }
                }
            }
            Op::Regarder(regards) => {
                presents = regards
                    .iter()
                    .filter_map(|g| match g {
                        RegardFil::Appareil(a) => Some(*a),
                        RegardFil::Ecran => None,
                    })
                    .collect();
                let vues = partie
                    .regarder(regards)
                    .map_err(|p| format!("regarder : {p}"))?;
                if quoi.vues == Oui::Oui {
                    r.vues
                        .push(vues.into_iter().map(|v| (v.regard, v.octets)).collect());
                }
            }
            Op::Etat => {
                let _: Result<partie::EtatVisible, partie::Echec> = partie.etat();
            }
            Op::Canonique => {
                let _: Result<Vec<u8>, partie::Echec> = partie.canonique();
            }
            // Les points ne changent ni le journal ni la partie.
            Op::Point => {}
            Op::Redemarrer { .. } => {
                return Err(String::from(
                    "la soirée des propriétés ne redémarre pas l'autorité",
                ));
            }
            Op::Repartir => {
                let _: Oui = partie.repartir().map_err(|p| format!("repartir : {p}"))?;
            }
        }
    }
    r.canonique = partie.canonique().ok();
    r.resume = partie
        .definitive()
        .lire(&Appel::Resumer)
        .ok()
        .and_then(|o| lire_octets(&o));
    r.journal = Some(partie.journal().clone());
    Ok(r)
}

// ---------------------------------------------------------------------------
// E12 : découper un pas (PHASE-3, P7 ; CAT-14)
// ---------------------------------------------------------------------------

/// Le script, chaque arrivée reçue entre deux `Avancer` remontée juste après le
/// premier, avant les vues, l'état et les points de ce pas : rejouée découpée, elle
/// arrive pendant l'avancée, si elle reste inachevée. L'ordre des arrivées entre elles
/// est gardé. Rend aussi le nombre d'arrivées qui suivent un `Avancer`.
pub(crate) fn regrouper(script: &[Op]) -> (Vec<Op>, u64) {
    let mut rendu: Vec<Op> = Vec::with_capacity(script.len());
    let mut autres: Vec<Op> = Vec::new();
    let mut apres_avancer = false;
    let mut remontees = 0u64;
    for op in script {
        match op {
            Op::Avancer(_) => {
                rendu.append(&mut autres);
                rendu.push(op.clone());
                apres_avancer = true;
            }
            Op::Arriver(..) if apres_avancer => {
                rendu.push(op.clone());
                remontees = remontees.saturating_add(1);
            }
            Op::Arriver(..) => rendu.push(op.clone()),
            Op::Regarder(_)
            | Op::Etat
            | Op::Canonique
            | Op::Point
            | Op::Redemarrer { .. }
            | Op::Repartir => {
                if apres_avancer {
                    autres.push(op.clone());
                } else {
                    rendu.push(op.clone());
                }
            }
        }
    }
    rendu.append(&mut autres);
    (rendu, remontees)
}

/// Coupe en deux chaque rafale d'au moins [`RAFALE_A_COUPER`] arrivées jouées par un
/// `Avancer` qu'aucune arrivée ne suit avant l'`Avancer` d'après : sa seconde moitié
/// passe juste après l'`Avancer` qui la jouait, dans l'ordre. Rejoué découpé, le pas de
/// la première moitié reste inachevé, et la seconde arrive pendant lui. Sans cela, un
/// jeu dont les arrivées viennent par rafales espacées ne reçoit jamais rien pendant un
/// pas découpé : Tier list, au lot 1 (vérification du lot 1, constat 5). Rend aussi le
/// nombre d'arrivées déplacées. Le script rendu est rejoué des deux façons : le déplacer
/// ne change pas ce qui se compare.
pub(crate) fn couper_les_rafales(script: Vec<Op>) -> (Vec<Op>, u64) {
    // Les segments : ce qui précède le premier `Avancer`, puis chaque `Avancer` suivi de
    // ce qui vient avant le suivant.
    let mut segments: Vec<Vec<Op>> = vec![Vec::new()];
    for op in script {
        if matches!(op, Op::Avancer(_)) {
            segments.push(Vec::new());
        }
        if let Some(dernier) = segments.last_mut() {
            dernier.push(op);
        }
    }
    let arrivees = |s: &[Op]| s.iter().filter(|o| matches!(o, Op::Arriver(..))).count();
    let comptes: Vec<usize> = segments.iter().map(|s| arrivees(s)).collect();
    let mut deplacees = 0u64;
    for j in 0..segments.len().saturating_sub(1) {
        let (k, suivantes) = (
            comptes.get(j).copied().unwrap_or(0),
            comptes.get(j.saturating_add(1)).copied().unwrap_or(0),
        );
        if k < RAFALE_A_COUPER || suivantes > 0 {
            continue;
        }
        let Some(segment) = segments.get_mut(j) else {
            continue;
        };
        // Les dernières `k / 2` arrivées du segment en sortent, dans l'ordre.
        let mut garder = k.saturating_sub(k / 2);
        let mut sorties = Vec::new();
        segment.retain(|o| {
            if !matches!(o, Op::Arriver(..)) {
                return true;
            }
            if garder > 0 {
                garder = garder.saturating_sub(1);
                return true;
            }
            sorties.push(o.clone());
            false
        });
        deplacees = deplacees.saturating_add(u64::try_from(sorties.len()).unwrap_or(0));
        if let Some(suivant) = segments.get_mut(j.saturating_add(1)) {
            // Juste après son `Avancer`, qui ouvre le segment.
            let apres = usize::from(!suivant.is_empty());
            let _: Vec<Op> = suivant.splice(apres..apres, sorties).collect();
        }
    }
    (segments.into_iter().flatten().collect(), deplacees)
}

/// La plus petite rafale que [`couper_les_rafales`] coupe : il en reste au moins deux
/// arrivées à leur pas, de quoi le laisser inachevé dans un tour d'une entrée, et une
/// au moins arrive pendant lui.
const RAFALE_A_COUPER: usize = 3;

/// Ce qu'un rejeu découpé relève.
pub(crate) struct Decoupe {
    pub(crate) journal: Journal,
    /// Les octets de chaque vue, à chaque `Regarder` : celle de l'écran est celle de la
    /// tête quand le jeu a une fenêtre, et le journal ne la dit pas.
    pub(crate) vues: Vec<Vec<(RegardFil, Vec<u8>)>>,
    /// Les tours qui ont laissé un pas inachevé.
    pub(crate) inacheves: u64,
    /// Les arrivées reçues pendant un pas inachevé.
    pub(crate) pendant: u64,
    /// Celles reçues avant le dernier pas de l'avancée, ou avant son saut.
    pub(crate) avant_la_fin: u64,
}

/// Rejoue un script sur une partie neuve du module. Avec `par_tour`, chaque `Avancer`
/// joue un tour de `par_tour` entrées venues d'une arrivée
/// (`Partie::avancer_decoupe`) ; les arrivées qui suivent arrivent alors pendant
/// l'avancée, si elle reste inachevée, à quelque pas qu'elle en soit et avant son saut ;
/// toute autre opération la finit d'abord. Sans, d'un seul tour. `etale_mal` arme le
/// contrôle négatif du découpage (`Partie::etaler_mal`).
pub(crate) fn rejouer_decoupe(
    module: &ModuleCharge,
    ouverture: &Ouverture,
    script: &[Op],
    par_tour: Option<u32>,
    etale_mal: Oui,
) -> Result<Decoupe, String> {
    let obs = Obs::new(Verifs::RIEN, ouverture.budgets.lecture);
    let mut partie = Partie::ouvrir(Sonde::new(module.fabrique(), &obs), ouverture.clone())
        .map_err(|e| format!("la partie ne s'ouvre pas : {e:?}"))?;
    if etale_mal == Oui::Oui {
        partie.etaler_mal();
    }
    let mut d = Decoupe {
        journal: Journal::clone(partie.journal()),
        vues: Vec::new(),
        inacheves: 0,
        pendant: 0,
        avant_la_fin: 0,
    };
    let finir = |partie: &mut Partie<Sonde<moteur_wasmtime::FabriqueWasm>>| {
        partie
            .finir_le_pas()
            .map(|_| ())
            .map_err(|p| format!("finir le pas : {p}"))
    };
    for op in script {
        match op {
            Op::Arriver(a, recu) => {
                if let (Some(s), Some(reste)) = (partie.pas_en_cours(), partie.avancee_en_cours()) {
                    d.pendant = d.pendant.saturating_add(1);
                    if reste != (s, jeu_abi::Duree::ZERO) {
                        d.avant_la_fin = d.avant_la_fin.saturating_add(1);
                    }
                }
                let _: partie::Placement = partie
                    .arriver(a.clone(), *recu)
                    .map_err(|p| format!("arriver : {p}"))?;
            }
            Op::Avancer(cible) => {
                finir(&mut partie)?;
                match par_tour {
                    None => {
                        let _: partie::Avancee = partie
                            .avancer(*cible)
                            .map_err(|p| format!("avancer : {p}"))?;
                    }
                    Some(n) => {
                        let av = partie
                            .avancer_decoupe(*cible, n)
                            .map_err(|p| format!("avancer : {p}"))?;
                        if av.en_cours.is_some() {
                            d.inacheves = d.inacheves.saturating_add(1);
                        }
                    }
                }
            }
            Op::Regarder(regards) => {
                finir(&mut partie)?;
                let vues = partie
                    .regarder(regards)
                    .map_err(|p| format!("regarder : {p}"))?;
                d.vues
                    .push(vues.into_iter().map(|v| (v.regard, v.octets)).collect());
            }
            Op::Etat => {
                finir(&mut partie)?;
                let _: Result<partie::EtatVisible, partie::Echec> = partie.etat();
            }
            Op::Canonique => {
                finir(&mut partie)?;
                let _: Result<Vec<u8>, partie::Echec> = partie.canonique();
            }
            Op::Point => finir(&mut partie)?,
            Op::Redemarrer { .. } => {
                return Err(String::from(
                    "la soirée des propriétés ne redémarre pas l'autorité",
                ));
            }
            Op::Repartir => {
                finir(&mut partie)?;
                let _: Oui = partie.repartir().map_err(|p| format!("repartir : {p}"))?;
            }
        }
    }
    finir(&mut partie)?;
    d.journal = partie.journal().clone();
    Ok(d)
}

// ---------------------------------------------------------------------------
// E10 : renuméroter (CAT-27)
// ---------------------------------------------------------------------------

/// La longueur en bits d'un nombre : un varint de même longueur de bits a la même
/// longueur d'octets.
const fn bits(n: u64) -> u32 {
    64u32.saturating_sub(n.leading_zeros())
}

/// Une renumérotation : pour chaque appareil, une valeur neuve de même longueur de
/// bits, distincte de toutes les autres.
pub(crate) fn renumerotation(appareils: &[u64], t: &mut Tirage) -> Vec<(u64, u64)> {
    let mut table: Vec<(u64, u64)> = Vec::new();
    for a in appareils {
        let b = bits(*a);
        if b < 2 {
            table.push((*a, *a));
            continue;
        }
        let haut = 1u64 << b.saturating_sub(1);
        let masque = haut.saturating_sub(1);
        let mut n = *a;
        for _ in 0..64 {
            let essai = haut | (t.entier() & masque);
            if essai != *a && !appareils.contains(&essai) && !table.iter().any(|(_, x)| *x == essai)
            {
                n = essai;
                break;
            }
        }
        table.push((*a, n));
    }
    table
}

fn renommer(table: &[(u64, u64)], a: Appareil) -> Appareil {
    Appareil::du_fil(
        table
            .iter()
            .find(|(x, _)| *x == a.fil())
            .map_or(a.fil(), |(_, y)| *y),
    )
}

fn renommer_presence(table: &[(u64, u64)], p: Presence) -> Presence {
    match p {
        Presence::Arrivee {
            appareil,
            role,
            capacites,
        } => Presence::Arrivee {
            appareil: renommer(table, appareil),
            role,
            capacites,
        },
        Presence::Depart { appareil, cause } => Presence::Depart {
            appareil: renommer(table, appareil),
            cause,
        },
        Presence::Retour {
            appareil,
            capacites,
        } => Presence::Retour {
            appareil: renommer(table, appareil),
            capacites,
        },
        Presence::Capacites {
            appareil,
            capacites,
        } => Presence::Capacites {
            appareil: renommer(table, appareil),
            capacites,
        },
    }
}

/// Le même script, chaque appareil renuméroté.
pub(crate) fn renumeroter(script: &[Op], table: &[(u64, u64)]) -> Vec<Op> {
    script
        .iter()
        .map(|op| match op {
            Op::Arriver(a, recu) => {
                let a = match a.clone() {
                    Arrivee::Geste {
                        de,
                        octets,
                        fait,
                        vu,
                        id,
                        demi_aller_retour,
                    } => Arrivee::Geste {
                        de: renommer(table, de),
                        octets,
                        fait,
                        vu,
                        id,
                        demi_aller_retour,
                    },
                    Arrivee::Tenu {
                        de,
                        octets,
                        numero,
                        fait,
                        demi_aller_retour,
                    } => Arrivee::Tenu {
                        de: renommer(table, de),
                        octets,
                        numero,
                        fait,
                        demi_aller_retour,
                    },
                    Arrivee::Manette {
                        de,
                        etat,
                        numero,
                        fait,
                    } => Arrivee::Manette {
                        de: renommer(table, de),
                        etat,
                        numero,
                        fait,
                    },
                    Arrivee::Presence(p) => Arrivee::Presence(renommer_presence(table, p)),
                    autre @ (Arrivee::Ecran { .. } | Arrivee::Reponse { .. }) => autre,
                };
                Op::Arriver(a, *recu)
            }
            Op::Regarder(regards) => Op::Regarder(
                regards
                    .iter()
                    .map(|r| match r {
                        RegardFil::Appareil(a) => {
                            RegardFil::Appareil(renommer(table, Appareil::du_fil(*a)).fil())
                        }
                        RegardFil::Ecran => RegardFil::Ecran,
                    })
                    .collect(),
            ),
            Op::Avancer(_)
            | Op::Etat
            | Op::Canonique
            | Op::Point
            | Op::Redemarrer { .. }
            | Op::Repartir => op.clone(),
        })
        .collect()
}

/// Les octets de chaque appareil, et ceux de sa valeur neuve.
fn motifs(table: &[(u64, u64)]) -> Vec<(Vec<u8>, Vec<u8>)> {
    table
        .iter()
        .map(|(a, b)| (octets(&Appareil::du_fil(*a)), octets(&Appareil::du_fil(*b))))
        .filter(|(a, b)| a.len() == b.len() && a != b)
        .collect()
}

/// Remplace, de gauche à droite, chaque suite d'octets d'un appareil par celle de sa
/// valeur neuve.
pub(crate) fn substituer(o: &[u8], motifs: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let mut rendu = Vec::with_capacity(o.len());
    let mut i = 0usize;
    'octets: while i < o.len() {
        for (a, b) in motifs {
            if o.get(i..i.saturating_add(a.len())) == Some(a.as_slice()) {
                rendu.extend_from_slice(b);
                i = i.saturating_add(a.len());
                continue 'octets;
            }
        }
        if let Some(x) = o.get(i) {
            rendu.push(*x);
        }
        i = i.saturating_add(1);
    }
    rendu
}

/// Le numéro d'apparition d'une clé, donné à sa première vue.
fn rang(table: &mut BTreeMap<u64, u64>, k: u64) -> u64 {
    let n = u64::try_from(table.len()).unwrap_or(u64::MAX);
    *table.entry(k).or_insert(n)
}

/// Une scène dont les clés et les identités d'effet sont renommées par ordre
/// d'apparition ; d'autres octets sont rendus tels quels.
pub(crate) fn normaliser_scene(o: &[u8]) -> Vec<u8> {
    let Ok(mut s) = decoder::<SceneFil>(o) else {
        return o.to_vec();
    };
    let mut cles = BTreeMap::new();
    for n in &mut s.noeuds {
        n.cle = Cle::du_fil(rang(&mut cles, n.cle.fil()));
        if let SorteNoeud::Composant(ComposantVuFil::Deplacables {
            zones, poubelle, ..
        }) = &mut n.sorte
        {
            for z in zones {
                z.cle = Cle::du_fil(rang(&mut cles, z.cle.fil()));
                for objet in &mut z.objets {
                    *objet = Cle::du_fil(rang(&mut cles, objet.fil()));
                }
            }
            if let Some(p) = poubelle {
                *p = Cle::du_fil(rang(&mut cles, p.fil()));
            }
        }
    }
    let mut effets = BTreeMap::new();
    for e in &mut s.effets {
        e.id = IdEffet::de(&rang(&mut effets, e.id.fil()));
    }
    octets(&s)
}

/// La vue d'un téléphone dont les identités d'effet sont renommées par ordre
/// d'apparition.
fn normaliser_montre(o: &[u8]) -> Vec<u8> {
    let Ok(mut m) = decoder::<MontreFil<'_>>(o) else {
        return o.to_vec();
    };
    let mut effets = BTreeMap::new();
    for e in &mut m.effets {
        e.id = IdEffet::de(&rang(&mut effets, e.id.fil()));
    }
    octets(&m)
}

fn normaliser(r: RegardFil, o: &[u8]) -> Vec<u8> {
    match r {
        RegardFil::Ecran => normaliser_scene(o),
        RegardFil::Appareil(_) => normaliser_montre(o),
    }
}

/// E10 : l'original et sa renumérotation ont le même résumé après chaque pas, et les
/// mêmes vues, à renumérotation près.
pub(crate) fn comparer_permutation(
    original: &Releve,
    renumerote: &Releve,
    table: &[(u64, u64)],
) -> Compte {
    let mut c = Compte::default();
    let m = motifs(table);
    for ((pas, a), (_, b)) in original.resumes.iter().zip(&renumerote.resumes) {
        c.evaluer();
        if substituer(a, &m) != *b {
            c.echouer(format!(
                "au pas {pas} : le résumé change quand les appareils sont renumérotés"
            ));
            break;
        }
    }
    if original.resumes.len() != renumerote.resumes.len() {
        c.evaluer();
        c.echouer(String::from("les deux soirées n'ont pas autant de pas"));
    }
    let mut vue_differente = false;
    for (k, (va, vb)) in original.vues.iter().zip(&renumerote.vues).enumerate() {
        for (ra, oa) in va {
            let rb = match ra {
                RegardFil::Appareil(x) => {
                    RegardFil::Appareil(renommer(table, Appareil::du_fil(*x)).fil())
                }
                RegardFil::Ecran => RegardFil::Ecran,
            };
            let Some((_, ob)) = vb.iter().find(|(r, _)| *r == rb) else {
                c.evaluer();
                c.echouer(format!("au regard {k} : la vue de {ra:?} manque"));
                vue_differente = true;
                continue;
            };
            c.evaluer();
            if normaliser(*ra, &substituer(oa, &m)) != normaliser(rb, ob) {
                c.echouer(format!(
                    "au regard {k} : la vue de {ra:?} change quand les appareils sont renumérotés"
                ));
                vue_differente = true;
            }
        }
        if vue_differente {
            break;
        }
    }
    c
}

// ---------------------------------------------------------------------------
// E11 : couper (PUN-73, SQ-28)
// ---------------------------------------------------------------------------

/// Une coupure : l'appareil, son premier pas, le pas où il revient.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Coupure {
    pub(crate) appareil: u64,
    pub(crate) debut: u64,
    pub(crate) fin: u64,
}

/// L'appareil d'une arrivée de téléphone, et si c'est une présence.
fn appareil_de(a: &Arrivee) -> Option<(u64, Oui)> {
    match a {
        Arrivee::Geste { de, .. } | Arrivee::Tenu { de, .. } | Arrivee::Manette { de, .. } => {
            Some((de.fil(), Oui::Non))
        }
        Arrivee::Presence(
            Presence::Arrivee { appareil, .. }
            | Presence::Depart { appareil, .. }
            | Presence::Retour { appareil, .. }
            | Presence::Capacites { appareil, .. },
        ) => Some((appareil.fil(), Oui::Oui)),
        Arrivee::Ecran { .. } | Arrivee::Reponse { .. } => None,
    }
}

/// Les pas, en pas de partie, entre deux lectures de l'hypothèse de la coupure
/// transparente dans la soirée sans coupure (E11). Une seule définition, publique et
/// documentée : [`jeu_abi::PERIODE_HYPOTHESE`], que le jeu lit aussi (`jeu::PERIODE_HYPOTHESE`,
/// Sidequest B10).
pub(crate) const PERIODE_HYPOTHESE: u64 = jeu_abi::PERIODE_HYPOTHESE.en_pas();

/// Les coupures à essayer : sur un appareil sans présence annoncée autour, chacune
/// plus courte que `moins_de`, au plus `n`, espacées, et là où l'hypothèse a tenu
/// **pendant toute la coupure** dans la soirée sans coupure : à chaque lecture, tous
/// les `periode` pas, de son début à la première lecture après sa fin (besoin 8 des
/// jeux, `conception/BESOINS-DES-JEUX.md` ; Sidequest B3, SQ-28). Une hypothèse qui ne
/// se sait qu'à la fin, comme « personne n'a poussé ce joueur », se juge ainsi.
pub(crate) fn choisir_coupures(
    script: &[Op],
    hypotheses: &[(u64, u64)],
    periode: u64,
    moins_de: Duree,
    n: usize,
) -> Vec<Coupure> {
    let periode = periode.max(1);
    let tenues: std::collections::BTreeSet<(u64, u64)> = hypotheses.iter().copied().collect();
    let tient_toute = |debut: u64, fin: u64, qui: u64| {
        let derniere = fin.div_ceil(periode).saturating_mul(periode);
        let mut p = debut;
        while p <= derniere {
            if !tenues.contains(&(p, qui)) {
                return false;
            }
            p = p.saturating_add(periode);
        }
        true
    };
    let duree = moins_de.en_pas().saturating_sub(1).min(300);
    if duree == 0 {
        return Vec::new();
    }
    let mut presences: Vec<(u64, u64)> = Vec::new();
    for op in script {
        if let Op::Arriver(a, recu) = op
            && let Some((qui, Oui::Oui)) = appareil_de(a)
        {
            presences.push((qui, recu.pas.numero()));
        }
    }
    let fin_du_script = script
        .iter()
        .filter_map(|op| match op {
            Op::Avancer(p) => Some(p.numero()),
            Op::Arriver(..)
            | Op::Regarder(_)
            | Op::Etat
            | Op::Canonique
            | Op::Point
            | Op::Redemarrer { .. }
            | Op::Repartir => None,
        })
        .max()
        .unwrap_or(0);
    let mut choisies: Vec<Coupure> = Vec::new();
    for (pas, qui) in hypotheses {
        if choisies.len() >= n {
            break;
        }
        let fin = pas.saturating_add(duree);
        let marge = fin.saturating_add(FERMETURE).saturating_add(60);
        if marge >= fin_du_script {
            continue;
        }
        let presence_autour = presences
            .iter()
            .any(|(a, p)| a == qui && *p >= pas.saturating_sub(60) && *p <= marge);
        let trop_pres = choisies
            .iter()
            .any(|c| c.fin.saturating_add(600) > *pas && c.debut < marge);
        if !presence_autour && !trop_pres && tient_toute(*pas, fin, *qui) {
            choisies.push(Coupure {
                appareil: *qui,
                debut: *pas,
                fin,
            });
        }
    }
    choisies
}

/// Le même script, l'appareil coupé de `debut` à `fin` : ses gestes et ses états
/// tenus de la coupure retenus jusqu'à sa fin, son départ annoncé à la fermeture de
/// la connexion, son retour à la fin.
pub(crate) fn couper(script: &[Op], c: Coupure) -> Vec<Op> {
    let mut rendu = Vec::with_capacity(script.len().saturating_add(4));
    let mut retenus: Vec<Arrivee> = Vec::new();
    let depart = c.debut.saturating_add(FERMETURE);
    let annonce_depart = c.fin > depart;
    let mut depart_fait = false;
    let mut retour_fait = false;
    let appareil = Appareil::du_fil(c.appareil);
    for op in script {
        if let Op::Avancer(cible) = op {
            let p = cible.numero();
            if annonce_depart && !depart_fait && p >= depart {
                depart_fait = true;
                rendu.push(Op::Arriver(
                    Arrivee::Presence(Presence::Depart {
                        appareil,
                        cause: CauseDepart::Fermeture,
                    }),
                    Instant::de(*cible),
                ));
            }
            if !retour_fait && p >= c.fin {
                retour_fait = true;
                if annonce_depart {
                    rendu.push(Op::Arriver(
                        Arrivee::Presence(Presence::Retour {
                            appareil,
                            capacites: Capacites::AUCUNE,
                        }),
                        Instant::de(*cible),
                    ));
                }
                for a in retenus.drain(..) {
                    rendu.push(Op::Arriver(a, Instant::de(*cible)));
                }
            }
        }
        if let Op::Arriver(a, recu) = op {
            let p = recu.pas.numero();
            if p >= c.debut
                && p < c.fin
                && matches!(appareil_de(a), Some((qui, Oui::Non)) if qui == c.appareil)
            {
                retenus.push(a.clone());
                continue;
            }
        }
        rendu.push(op.clone());
    }
    rendu
}

// ---------------------------------------------------------------------------
// E12 : retarder dans la fenêtre (§ 4.3, D12)
// ---------------------------------------------------------------------------

/// Le même script, des gestes reçus plus tard, chacun encore dans la fenêtre de `w`
/// pas, l'ordre de chaque appareil gardé, jamais au-delà d'une présence du même
/// appareil. Rend le script et le nombre de gestes retardés.
pub(crate) fn retarder(script: &[Op], w: u64, t: &mut Tirage) -> (Vec<Op>, u64) {
    // Le pas de chaque présence, par appareil.
    let mut presences: Vec<(u64, u64)> = Vec::new();
    for op in script {
        if let Op::Arriver(a, recu) = op
            && let Some((qui, Oui::Oui)) = appareil_de(a)
        {
            presences.push((qui, recu.pas.numero()));
        }
    }
    let mut dernier: BTreeMap<u64, u64> = BTreeMap::new();
    let mut a_livrer: Vec<(u64, Arrivee)> = Vec::new();
    let mut rendu = Vec::with_capacity(script.len());
    let mut retardes = 0u64;
    for op in script {
        match op {
            Op::Arriver(
                Arrivee::Geste {
                    de,
                    octets,
                    fait,
                    vu,
                    id,
                    demi_aller_retour,
                },
                recu,
            ) => {
                let qui = de.fil();
                let recu_pas = recu.pas.numero();
                let deja = recu_pas.saturating_sub(fait.pas.numero());
                let prochaine_presence = presences
                    .iter()
                    .filter(|(a, p)| *a == qui && *p > recu_pas)
                    .map(|(_, p)| *p)
                    .min()
                    .unwrap_or(u64::MAX);
                let plafond = fait
                    .pas
                    .numero()
                    .saturating_add(w.saturating_sub(1))
                    .min(prochaine_presence.saturating_sub(1));
                let voulu = recu_pas.saturating_add(t.sous(w.saturating_sub(deja).max(1)));
                let minimum = dernier.get(&qui).copied().unwrap_or(0).max(recu_pas);
                let livre = voulu.min(plafond).max(minimum);
                let _: Option<u64> = dernier.insert(qui, livre);
                let extra = livre.saturating_sub(recu_pas);
                if extra > 0 {
                    retardes = retardes.saturating_add(1);
                }
                a_livrer.push((
                    livre,
                    Arrivee::Geste {
                        de: *de,
                        octets: octets.clone(),
                        // Le cœur ne garde pas un `fait` après sa réception (§ 4.3) : une
                        // horloge en avance l'annonce plus tard que `recu`. Le même geste
                        // reçu plus tard garderait cet instant-là : on le borne comme le
                        // cœur l'a borné, pour que seule l'arrivée change.
                        fait: (*fait).min(*recu),
                        vu: *vu,
                        id: *id,
                        demi_aller_retour: demi_aller_retour.plus(Duree::pas(extra)),
                    },
                ));
            }
            Op::Avancer(cible) => {
                let p = cible.numero();
                let (maintenant, plus_tard): (Vec<_>, Vec<_>) = std::mem::take(&mut a_livrer)
                    .into_iter()
                    .partition(|(l, _)| *l <= p);
                a_livrer = plus_tard;
                for (_, a) in maintenant {
                    rendu.push(Op::Arriver(a, Instant::de(*cible)));
                }
                rendu.push(op.clone());
            }
            Op::Arriver(..)
            | Op::Regarder(_)
            | Op::Etat
            | Op::Canonique
            | Op::Point
            | Op::Redemarrer { .. }
            | Op::Repartir => rendu.push(op.clone()),
        }
    }
    (rendu, retardes)
}

/// Une entrée, l'instant de sa réception effacé, et, hors des gestes, son identité :
/// celle d'un geste vient de l'appareil et reste ; celle d'une entrée de l'écran ou
/// d'un tenu est son numéro d'arrivée, que l'ordre des arrivées change.
fn sans_reception(o: &[u8]) -> Vec<u8> {
    let effacer = |q: QuandFil, id: Oui| QuandFil {
        recu: 0,
        id: if id == Oui::Oui { 0 } else { q.id },
        ..q
    };
    match decoder::<EntreeFil<'_>>(o) {
        Ok(EntreeFil::Geste {
            de,
            octets: g,
            quand,
        }) => octets(&EntreeFil::Geste {
            de,
            octets: g,
            quand: effacer(quand, Oui::Non),
        }),
        Ok(EntreeFil::Ecran { quoi, quand }) => octets(&EntreeFil::Ecran {
            quoi,
            quand: effacer(quand, Oui::Oui),
        }),
        Ok(EntreeFil::Tenu {
            de,
            octets: t,
            cause,
            quand,
        }) => octets(&EntreeFil::Tenu {
            de,
            octets: t,
            cause,
            quand: effacer(quand, Oui::Oui),
        }),
        Ok(EntreeFil::Manette {
            de,
            etat,
            cause,
            quand,
        }) => octets(&EntreeFil::Manette {
            de,
            etat,
            cause,
            quand: effacer(quand, Oui::Oui),
        }),
        Ok(EntreeFil::Temps { .. } | EntreeFil::Presence(_) | EntreeFil::Reponse { .. })
        | Err(_) => o.to_vec(),
    }
}

/// L'histoire définitive, comparable d'une variante à l'autre : les entrées, sans
/// leurs instants de réception ([`sans_reception`]), les empreintes, les parties, les
/// sauts.
pub(crate) fn histoire(j: &Journal) -> Vec<Ligne> {
    j.lignes()
        .iter()
        .filter_map(|l| match l {
            Ligne::Entree(e) => {
                let mut e = e.clone();
                e.octets = sans_reception(&e.octets);
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

/// La première différence de deux histoires.
pub(crate) fn premiere_difference(gauche: &[Ligne], droite: &[Ligne]) -> Option<String> {
    let longueur = gauche.len().max(droite.len());
    (0..longueur).find_map(|rang| match (gauche.get(rang), droite.get(rang)) {
        (Some(g), Some(d)) if g == d => None,
        (g, d) => {
            let court = |ligne: Option<&Ligne>| {
                ligne.map_or_else(
                    || String::from("la fin"),
                    |l| format!("{l:?}").chars().take(160).collect(),
                )
            };
            Some(format!(
                "entrée {rang} de l'histoire définitive : {} contre {}",
                court(g),
                court(d)
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use jeu_abi::{Duree, Pas};

    use super::{PERIODE_HYPOTHESE, choisir_coupures, couper_les_rafales};
    use crate::soiree::Op;

    /// Un script de `pas` pas, sans arrivée.
    fn script(pas: u64) -> Vec<Op> {
        (1..=pas).map(|p| Op::Avancer(Pas::du_fil(p))).collect()
    }

    /// Besoin 8 des jeux (Sidequest B3, SQ-28) : une hypothèse vraie au début de la
    /// coupure, mais fausse avant sa fin, ne laisse pas couper ; une hypothèse vraie
    /// pendant toute la coupure, oui.
    #[test]
    fn l_hypothese_doit_tenir_pendant_toute_la_coupure() {
        let s = script(3000);
        let qui = 7;
        let p = PERIODE_HYPOTHESE;
        // Vraie de 100 à 200 seulement : une coupure de 300 pas n'y tient pas.
        let breve: Vec<(u64, u64)> = (10..=20).map(|k| (k * p, qui)).collect();
        assert_eq!(choisir_coupures(&s, &breve, p, Duree::s(10), 3), Vec::new());
        // Vraie de 100 à 500 : la coupure qui commence à 100 finit à 400 (300 pas au
        // plus), et la lecture de sa fin tient encore.
        let longue: Vec<(u64, u64)> = (10..=50).map(|k| (k * p, qui)).collect();
        let c = choisir_coupures(&s, &longue, p, Duree::s(10), 3);
        assert_eq!(
            c.first().map(|c| (c.appareil, c.debut, c.fin)),
            Some((qui, 100, 400)),
            "{c:?}"
        );
        assert_eq!(c.len(), 1, "{c:?}");
        // Un trou d'une seule lecture au milieu suffit à l'écarter.
        let trouee: Vec<(u64, u64)> = longue.iter().copied().filter(|(q, _)| *q != 250).collect();
        let c = choisir_coupures(&s, &trouee, p, Duree::s(10), 3);
        assert!(c.iter().all(|c| c.debut > 250 || c.fin < 250), "{c:?}");
    }

    /// Une arrivée reconnaissable à son appareil.
    fn arrivee(n: u64) -> Op {
        Op::Arriver(
            partie::Arrivee::Presence(jeu_abi::Presence::Depart {
                appareil: jeu_abi::Appareil::du_fil(n),
                cause: jeu_abi::CauseDepart::Silence,
            }),
            jeu_abi::Instant::de(Pas::du_fil(1)),
        )
    }

    /// Chaque opération, par son `Debug` : `Op` ne se compare pas.
    fn noms(s: &[Op]) -> Vec<String> {
        s.iter().map(|o| format!("{o:?}")).collect()
    }

    /// Vérification du lot 1, constat 5 : une rafale de quatre arrivées qu'aucune arrivée
    /// ne suit avant l'`Avancer` d'après est coupée, sa seconde moitié juste après
    /// l'`Avancer` qui la jouait ; une rafale suivie d'arrivées, ou de deux arrivées
    /// seulement, ne l'est pas.
    #[test]
    fn une_rafale_que_rien_ne_suit_est_coupee() {
        let avancer = |p: u64| Op::Avancer(Pas::du_fil(p));
        let rafale = vec![
            arrivee(1),
            arrivee(2),
            arrivee(3),
            arrivee(4),
            avancer(1),
            Op::Etat,
            avancer(2),
        ];
        let (coupe, n) = couper_les_rafales(rafale);
        let attendu = vec![
            arrivee(1),
            arrivee(2),
            avancer(1),
            arrivee(3),
            arrivee(4),
            Op::Etat,
            avancer(2),
        ];
        assert_eq!((noms(&coupe), n), (noms(&attendu), 2));
        let suivie = vec![
            arrivee(1),
            arrivee(2),
            arrivee(3),
            arrivee(4),
            avancer(1),
            arrivee(5),
            avancer(2),
        ];
        let (pareil, n) = couper_les_rafales(suivie.clone());
        assert_eq!((noms(&pareil), n), (noms(&suivie), 0));
        let trois = vec![arrivee(1), arrivee(2), arrivee(3), avancer(1), avancer(2)];
        let (coupe, n) = couper_les_rafales(trois);
        let attendu = vec![arrivee(1), arrivee(2), avancer(1), arrivee(3), avancer(2)];
        assert_eq!((noms(&coupe), n), (noms(&attendu), 1));
        let courte = vec![arrivee(1), arrivee(2), avancer(1), avancer(2)];
        let (pareil, n) = couper_les_rafales(courte.clone());
        assert_eq!((noms(&pareil), n), (noms(&courte), 0));
    }
}
