//! La forme entière de la garde, figée (vérification du lot 1, constat 3) : chaque
//! variant de chaque enum gardée de `partie` (`QuoiPlace`, `EcranArrivee`,
//! `ComposantArrivee`, `ReponseArrivee`, `Origine`, `Faute`, avec `Ligne` et
//! `IssueJournal` que `garde.rs` fige déjà), un état du cœur qui a de chaque chose
//! (fiches, tenus, manettes, demandes, vues), et un point dont la chaîne a trois points
//! et deux instantanés. Chacun a ses octets écrits ici et sa seconde lecture, sans serde
//! ni postcard (SAL-90).
//!
//! La première version ne figeait que l'ouverture et un journal, relisait l'état du cœur
//! vide et un seul variant de `QuoiPlace`, de `Faute` et d'`Origine` : un champ ajouté
//! à une fiche ou à un tenu ne faisait rougir aucun test.
//!
//! Les octets ont été relevés de l'encodeur, puis chacun annoté d'après la forme, et la
//! seconde lecture, écrite d'après la forme seule, les relit champ à champ. Ce qui
//! appartient à l'ABI (les capacités, une manette, une touche, un pointeur, une
//! présence) est écrit d'après sa forme d'ABI, figée par `jeu-abi` (`forme_figee`) ; la
//! seconde lecture le compare à ses octets d'ABI. L'instantané du modèle est opaque à
//! la garde : il se range tel que le modèle l'écrit (`InstantaneGarde`) ; celui de
//! wasmtime a ses propres octets figés (`moteur-wasmtime`, `interieur.rs`).
//!
//! Un variant ajouté ne compile plus dans les `nom_*` ci-dessous (pas de `_`), et un
//! variant sans octets figés fait rougir `les_octets_figes_couvrent_chaque_variant`.

use std::collections::BTreeSet;

use jeu_abi::{
    Appareil, Capacites, Cle, CodeTouche, Deplace, EchecDemande, FichierRecu, FinSaisie, Fixe,
    GenrePointeur, IdFichier, IdGeste, Instant, Manette, Mods, Oui, Pas, PhasePointeur,
    PhaseTouche, Pointeur, Presence, RegardFil, ReveilFil, RoleAppareil, TexteCourt, Touche,
};
use partie::modele::{FabriqueModele, GesteModele, InstantaneModele, ReglageModele};
use partie::persistance::{
    InstantaneGarde, VERSION_GARDE, decoder_coeur, decoder_journal, decoder_point, encoder_coeur,
    encoder_lignes, encoder_point,
};
use partie::regles::Origine;
use partie::{
    Arrivee, ArriveePlacee, ComposantArrivee, EcranArrivee, Faute, IssueJournal, Journal, Ligne,
    LigneEntree, Partie, Placement, PointDeReprise, QuoiPlace, ReponseArrivee, ValeurTelephone,
};

use crate::commun::*;
use crate::garde::{Lecteur, cat, h, journal_fige};

type Point = PointDeReprise<InstantaneModele>;

// ---------------------------------------------------------------------------
// Les noms des variants : un variant ajouté ne compile plus ici
// ---------------------------------------------------------------------------

fn nom_quoi(q: &QuoiPlace) -> &'static str {
    match q {
        QuoiPlace::Geste { .. } => "QuoiPlace::Geste",
        QuoiPlace::Tenu { .. } => "QuoiPlace::Tenu",
        QuoiPlace::Manette { .. } => "QuoiPlace::Manette",
        QuoiPlace::Ecran(_) => "QuoiPlace::Ecran",
        QuoiPlace::Presence(_) => "QuoiPlace::Presence",
        QuoiPlace::Reponse { .. } => "QuoiPlace::Reponse",
    }
}

fn nom_ecran(e: &EcranArrivee) -> &'static str {
    match e {
        EcranArrivee::Action(_) => "EcranArrivee::Action",
        EcranArrivee::Touche(_) => "EcranArrivee::Touche",
        EcranArrivee::Pointeur(_) => "EcranArrivee::Pointeur",
        EcranArrivee::Composant(_) => "EcranArrivee::Composant",
    }
}

fn nom_composant(c: &ComposantArrivee) -> &'static str {
    match c {
        ComposantArrivee::Saisie { .. } => "ComposantArrivee::Saisie",
        ComposantArrivee::Deplace(_) => "ComposantArrivee::Deplace",
        ComposantArrivee::Fichiers { .. } => "ComposantArrivee::Fichiers",
    }
}

fn nom_reponse(r: &ReponseArrivee) -> &'static str {
    match r {
        ReponseArrivee::Image(_) => "ReponseArrivee::Image",
        ReponseArrivee::Rendu(_) => "ReponseArrivee::Rendu",
        ReponseArrivee::Lu(_) => "ReponseArrivee::Lu",
        ReponseArrivee::Fait(_) => "ReponseArrivee::Fait",
    }
}

fn nom_origine(o: Origine) -> &'static str {
    match o {
        Origine::Commun => "Origine::Commun",
        Origine::Ecran => "Origine::Ecran",
        Origine::Appareil(_) => "Origine::Appareil",
    }
}

fn nom_faute(f: &Faute) -> &'static str {
    match f {
        Faute::Budget => "Faute::Budget",
        Faute::Profondeur => "Faute::Profondeur",
        Faute::Panique { .. } => "Faute::Panique",
        Faute::Memoire => "Faute::Memoire",
        Faute::HorsMemoire => "Faute::HorsMemoire",
        Faute::SortieInvalide => "Faute::SortieInvalide",
        Faute::Piege { .. } => "Faute::Piege",
        Faute::Attente => "Faute::Attente",
        Faute::VueInvalide { .. } => "Faute::VueInvalide",
    }
}

fn nom_ligne(l: &Ligne) -> &'static str {
    match l {
        Ligne::Arrivees { .. } => "Ligne::Arrivees",
        Ligne::Entree(_) => "Ligne::Entree",
        Ligne::Empreinte { .. } => "Ligne::Empreinte",
        Ligne::Saut { .. } => "Ligne::Saut",
        Ligne::Pause { .. } => "Ligne::Pause",
        Ligne::Vue { .. } => "Ligne::Vue",
        Ligne::Partie { .. } => "Ligne::Partie",
        Ligne::Reprise { .. } => "Ligne::Reprise",
    }
}

fn nom_issue(i: &IssueJournal) -> &'static str {
    match i {
        IssueJournal::Acceptee { .. } => "IssueJournal::Acceptee",
        IssueJournal::Refusee { .. } => "IssueJournal::Refusee",
        IssueJournal::Ignoree => "IssueJournal::Ignoree",
        IssueJournal::Ecartee { .. } => "IssueJournal::Ecartee",
    }
}

/// Chaque variant que les octets figés doivent tenir. Un variant ajouté dans un `nom_*`
/// et oublié ici, ou ici et sans octets figés, fait rougir la couverture.
const TOUS_LES_VARIANTS: &[&str] = &[
    "QuoiPlace::Geste",
    "QuoiPlace::Tenu",
    "QuoiPlace::Manette",
    "QuoiPlace::Ecran",
    "QuoiPlace::Presence",
    "QuoiPlace::Reponse",
    "EcranArrivee::Action",
    "EcranArrivee::Touche",
    "EcranArrivee::Pointeur",
    "EcranArrivee::Composant",
    "ComposantArrivee::Saisie",
    "ComposantArrivee::Deplace",
    "ComposantArrivee::Fichiers",
    "ReponseArrivee::Image",
    "ReponseArrivee::Rendu",
    "ReponseArrivee::Lu",
    "ReponseArrivee::Fait",
    "Origine::Commun",
    "Origine::Ecran",
    "Origine::Appareil",
    "Faute::Budget",
    "Faute::Profondeur",
    "Faute::Panique",
    "Faute::Memoire",
    "Faute::HorsMemoire",
    "Faute::SortieInvalide",
    "Faute::Piege",
    "Faute::Attente",
    "Faute::VueInvalide",
    "Ligne::Arrivees",
    "Ligne::Entree",
    "Ligne::Empreinte",
    "Ligne::Saut",
    "Ligne::Pause",
    "Ligne::Vue",
    "Ligne::Partie",
    "Ligne::Reprise",
    "IssueJournal::Acceptee",
    "IssueJournal::Refusee",
    "IssueJournal::Ignoree",
    "IssueJournal::Ecartee",
];

/// Les noms des variants que ce journal contient, à toute profondeur.
fn variants_du_journal(j: &Journal, noms: &mut BTreeSet<&'static str>) {
    for l in j.lignes() {
        noms.insert(nom_ligne(l));
        match l {
            Ligne::Arrivees { arrivees, .. } => {
                for a in arrivees {
                    noms.insert(nom_origine(a.origine));
                    noms.insert(nom_quoi(&a.quoi));
                    match &a.quoi {
                        QuoiPlace::Ecran(e) => {
                            noms.insert(nom_ecran(e));
                            if let EcranArrivee::Composant(c) = e {
                                noms.insert(nom_composant(c));
                            }
                        }
                        QuoiPlace::Reponse { reponse, .. } => {
                            noms.insert(nom_reponse(reponse));
                        }
                        QuoiPlace::Geste { .. }
                        | QuoiPlace::Tenu { .. }
                        | QuoiPlace::Manette { .. }
                        | QuoiPlace::Presence(_) => {}
                    }
                }
            }
            Ligne::Entree(e) => {
                noms.insert(nom_issue(&e.issue));
                if let IssueJournal::Ecartee { faute } = &e.issue {
                    noms.insert(nom_faute(faute));
                }
            }
            Ligne::Empreinte { .. }
            | Ligne::Saut { .. }
            | Ligne::Pause { .. }
            | Ligne::Vue { .. }
            | Ligne::Partie { .. }
            | Ligne::Reprise { .. } => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Les valeurs figées
// ---------------------------------------------------------------------------

/// Une arrivée placée au pas 4, faite à (4, 2), reçue à (4, 0), sur la vue du pas 3.
fn placee(origine: Origine, quoi: QuoiPlace) -> ArriveePlacee {
    ArriveePlacee {
        sequence: 1,
        place: Pas::du_fil(4),
        fait: Instant {
            pas: Pas::du_fil(4),
            fraction: 2,
        },
        recu: Instant::de(Pas::du_fil(4)),
        vu: Pas::du_fil(3),
        origine,
        quoi,
    }
}

/// Une arrivée de chaque sorte, et de chaque origine : celles d'un appareil, puis
/// celles de l'écran et du commun.
fn arrivees_figees() -> Vec<(&'static str, ArriveePlacee)> {
    let mut v = arrivees_d_appareil_figees();
    v.extend(arrivees_de_l_ecran_et_du_commun_figees());
    v
}

fn arrivees_d_appareil_figees() -> Vec<(&'static str, ArriveePlacee)> {
    vec![
        (
            "QuoiPlace::Geste",
            placee(
                Origine::Appareil(0),
                QuoiPlace::Geste {
                    de: Appareil::du_fil(7),
                    octets: vec![1, 2],
                    id: IdGeste::du_fil(5),
                },
            ),
        ),
        (
            "QuoiPlace::Tenu",
            placee(
                Origine::Appareil(0),
                QuoiPlace::Tenu {
                    de: Appareil::du_fil(7),
                    numero: 3,
                    valeur: ValeurTelephone {
                        octets: vec![4],
                        cle: vec![5],
                        relache: Some(vec![6]),
                        est_relache: Oui::Non,
                    },
                },
            ),
        ),
        (
            "QuoiPlace::Manette",
            placee(
                Origine::Appareil(1),
                QuoiPlace::Manette {
                    de: Appareil::du_fil(8),
                    numero: 2,
                    etat: Manette::REPOS,
                },
            ),
        ),
    ]
}

fn arrivees_de_l_ecran_et_du_commun_figees() -> Vec<(&'static str, ArriveePlacee)> {
    let ecran = |e: EcranArrivee| placee(Origine::Ecran, QuoiPlace::Ecran(e));
    let composant = |c: ComposantArrivee| ecran(EcranArrivee::Composant(c));
    let reponse =
        |r: ReponseArrivee| placee(Origine::Commun, QuoiPlace::Reponse { id: 9, reponse: r });
    vec![
        ("EcranArrivee::Action", ecran(EcranArrivee::Action(vec![3]))),
        (
            "EcranArrivee::Touche",
            ecran(EcranArrivee::Touche(Touche {
                code: CodeTouche::ENTREE,
                mods: Mods::AUCUN,
                phase: PhaseTouche::Enfoncee,
                repetition: Oui::Non,
            })),
        ),
        (
            "EcranArrivee::Pointeur",
            ecran(EcranArrivee::Pointeur(Pointeur {
                id: 1,
                genre: GenrePointeur::Doigt,
                bouton: 0,
                phase: PhasePointeur::Appui,
                pos: (Fixe::unites(1), Fixe::unites(2)),
            })),
        ),
        (
            "ComposantArrivee::Saisie",
            composant(ComposantArrivee::Saisie {
                champ: Cle::du_fil(4),
                texte: String::from("ab"),
                fin: FinSaisie::Entree,
            }),
        ),
        (
            "ComposantArrivee::Deplace",
            composant(ComposantArrivee::Deplace(Deplace::Annule {
                objet: Cle::du_fil(5),
            })),
        ),
        (
            "ComposantArrivee::Fichiers",
            composant(ComposantArrivee::Fichiers {
                geste: IdGeste::du_fil(6),
                fichiers: vec![FichierRecu {
                    id: IdFichier::du_fil(1),
                    nom: TexteCourt::new("a"),
                    genre: TexteCourt::new("b"),
                    taille: 10,
                }],
            }),
        ),
        (
            "QuoiPlace::Presence",
            placee(
                Origine::Appareil(0),
                QuoiPlace::Presence(Presence::Arrivee {
                    appareil: Appareil::du_fil(7),
                    role: RoleAppareil::Telephone,
                    capacites: Capacites::AUCUNE,
                }),
            ),
        ),
        (
            "ReponseArrivee::Image",
            reponse(ReponseArrivee::Image(Ok(vec![1]))),
        ),
        (
            "ReponseArrivee::Rendu",
            reponse(ReponseArrivee::Rendu(Err(EchecDemande::Refuse))),
        ),
        (
            "ReponseArrivee::Lu",
            reponse(ReponseArrivee::Lu(Ok(vec![]))),
        ),
        (
            "ReponseArrivee::Fait",
            reponse(ReponseArrivee::Fait(Ok(()))),
        ),
    ]
}

/// Une faute de chaque sorte.
fn fautes_figees() -> Vec<(&'static str, Faute)> {
    vec![
        ("Faute::Budget", Faute::Budget),
        ("Faute::Profondeur", Faute::Profondeur),
        (
            "Faute::Panique",
            Faute::Panique {
                message: String::from("x"),
            },
        ),
        ("Faute::Memoire", Faute::Memoire),
        ("Faute::HorsMemoire", Faute::HorsMemoire),
        ("Faute::SortieInvalide", Faute::SortieInvalide),
        (
            "Faute::Piege",
            Faute::Piege {
                code: String::from("y"),
            },
        ),
        ("Faute::Attente", Faute::Attente),
        (
            "Faute::VueInvalide",
            Faute::VueInvalide {
                detail: String::from("z"),
            },
        ),
    ]
}

fn ligne_de_l_arrivee(a: ArriveePlacee) -> Ligne {
    Ligne::Arrivees {
        pas: Pas::du_fil(4),
        arrivees: vec![a],
    }
}

fn ligne_de_la_faute(faute: Faute) -> Ligne {
    Ligne::Entree(LigneEntree {
        partie: 0,
        ordre: 0,
        place: Pas::du_fil(4),
        octets: vec![9],
        issue: IssueJournal::Ecartee { faute },
    })
}

/// Un morceau de journal d'une seule ligne : base 0, pas 4.
fn morceau(l: Ligne) -> Journal {
    decoder_journal(&encoder_lignes(0, Pas::du_fil(4), &[l])).unwrap()
}

/// Une partie qui a de chaque chose dans son cœur : deux appareils annoncés (un
/// téléphone, une manette), un tenu, une manette tenue, une demande lancée, une vue ; le
/// modèle dort, et rien ne confirme le point à intervalle : le point pris au pas 3 cite
/// le point d'ouverture comme point sain et celui du pas 2 comme dernier point à
/// intervalle.
fn une_partie_pleine() -> (PartieModele, Point) {
    let mut r = ReglageModele::ordinaire();
    r.reveil = ReveilFil::Jamais;
    let mut o = ouverture();
    o.point_tous_les = 2;
    let mut p = Partie::ouvrir(FabriqueModele::new(r), o).unwrap();
    for (n, role) in [(1, RoleAppareil::Telephone), (2, RoleAppareil::Manette)] {
        let presence = Presence::Arrivee {
            appareil: a(n),
            role,
            capacites: Capacites::AUCUNE,
        };
        let _: Placement = p.arriver(Arrivee::Presence(presence), instant(1)).unwrap();
    }
    let _: partie::Avancee = p.avancer(pas(1)).unwrap();
    let _: Placement = p
        .arriver(geste(1, GesteModele::Demander, 2, 1), instant(2))
        .unwrap();
    let _: Placement = p.arriver(tenu(1, 0, 2, 1, 2), instant(2)).unwrap();
    let manette = Arrivee::Manette {
        de: a(2),
        etat: Manette::REPOS,
        numero: 1,
        fait: instant(2),
    };
    let _: Placement = p.arriver(manette, instant(2)).unwrap();
    let _: partie::Avancee = p.avancer(pas(2)).unwrap();
    let _: Vec<partie::Vue> = p.regarder(&[RegardFil::Appareil(1)]).unwrap();
    let _: partie::Avancee = p.avancer(pas(3)).unwrap();
    let pt = p.point().unwrap();
    (p, pt)
}

// ---------------------------------------------------------------------------
// Les octets figés
// ---------------------------------------------------------------------------

/// `Capacites::AUCUNE`, d'après sa forme d'ABI : six `Capacite::Non` (1) de cause
/// `NonSupportee` (0), l'orientation `Portrait` (0), aucun prénom (0).
fn octets_capacites_aucunes() -> Vec<u8> {
    cat(&[&[1, 0].repeat(6), &[0, 0]])
}

/// `Manette::REPOS`, d'après sa forme d'ABI : les deux sticks, quatre axes à 0.0
/// (chacun un `f32` de quatre octets), deux gâchettes à 0, seize boutons `Relache` (1)
/// jamais appuyés (0).
fn octets_manette_au_repos() -> Vec<u8> {
    cat(&[&[0; 16], &[0, 0], &[1, 0].repeat(16)])
}

/// Le début de chaque morceau d'une arrivée : l'en-tête d'un journal, base 0, pas 4,
/// une ligne `Arrivees` (0) du pas 4 qui en a une ; puis l'arrivée : séquence 1, place 4,
/// fait (4, 2), reçu (4, 0), vu 3.
fn debut_d_arrivee() -> Vec<u8> {
    cat(&[&h(2), &[0, 4, 1], &[0, 4, 1], &[1, 4, 4, 2, 4, 0, 3]])
}

/// Les octets de chaque arrivée figée, après [`debut_d_arrivee`] : l'origine (`Commun`
/// 0, `Ecran` 1, `Appareil` 2 et son rang), puis le variant de `QuoiPlace` et son contenu.
fn octets_arrivee(nom: &str) -> Vec<u8> {
    let suite: Vec<u8> = match nom {
        // Appareil 0 ; Geste (0) : de 7, octets [1, 2], identité 5.
        "QuoiPlace::Geste" => vec![2, 0, 0, 7, 2, 1, 2, 5],
        // Appareil 0 ; Tenu (1) : de 7, numéro 3, la valeur : octets [4], clé [5],
        // relâchée Some (1) [6], pas encore relâchée (Non, 1).
        "QuoiPlace::Tenu" => vec![2, 0, 1, 7, 3, 1, 4, 1, 5, 1, 1, 6, 1],
        // Appareil 1 ; Manette (2) : de 8, numéro 2, la manette au repos.
        "QuoiPlace::Manette" => cat(&[&[2, 1, 2, 8, 2], &octets_manette_au_repos()]),
        // Écran ; Ecran (3), Action (0) : [3].
        "EcranArrivee::Action" => vec![1, 3, 0, 1, 3],
        // Écran ; Ecran (3), Touche (1) : le code « Enter », aucun modificateur (quatre
        // Non), enfoncée (0), pas une répétition (Non).
        "EcranArrivee::Touche" => cat(&[&[1, 3, 1, 5], b"Enter", &[1, 1, 1, 1, 0, 1]]),
        // Écran ; Ecran (3), Pointeur (2) : identité 1, doigt (1), bouton 0, appui (0),
        // position (1, 2) en `Fixe` (256 et 512, en zigzag).
        "EcranArrivee::Pointeur" => vec![1, 3, 2, 1, 1, 0, 0, 0x80, 0x04, 0x80, 0x08],
        // Écran ; Ecran (3), Composant (3), Saisie (0) : champ 4, « ab », Entree (1).
        "ComposantArrivee::Saisie" => vec![1, 3, 3, 0, 4, 2, b'a', b'b', 1],
        // Écran ; Ecran (3), Composant (3), Deplace (1) : Annule (2), objet 5.
        "ComposantArrivee::Deplace" => vec![1, 3, 3, 1, 2, 5],
        // Écran ; Ecran (3), Composant (3), Fichiers (2) : geste 6, un fichier :
        // identité 1, nom « a », genre « b », taille 10.
        "ComposantArrivee::Fichiers" => vec![1, 3, 3, 2, 6, 1, 1, 1, b'a', 1, b'b', 10],
        // Appareil 0 ; Presence (4), Arrivee (0) : appareil 7, téléphone (0), aucune
        // capacité.
        "QuoiPlace::Presence" => cat(&[&[2, 0, 4, 0, 7, 0], &octets_capacites_aucunes()]),
        // Commun ; Reponse (5) : demande 9, Image (0), Ok (0) [1].
        "ReponseArrivee::Image" => vec![0, 5, 9, 0, 0, 1, 1],
        // Commun ; Reponse (5) : demande 9, Rendu (1), Err (1) Refuse (2).
        "ReponseArrivee::Rendu" => vec![0, 5, 9, 1, 1, 2],
        // Commun ; Reponse (5) : demande 9, Lu (2), Ok (0), aucun octet.
        "ReponseArrivee::Lu" => vec![0, 5, 9, 2, 0, 0],
        // Commun ; Reponse (5) : demande 9, Fait (3), Ok (0).
        "ReponseArrivee::Fait" => vec![0, 5, 9, 3, 0],
        autre => panic!("aucun octet figé pour {autre}"),
    };
    cat(&[&debut_d_arrivee(), &suite])
}

/// Les octets de chaque faute figée : l'en-tête d'un journal, base 0, pas 4, une ligne
/// `Entree` (1) : partie 0, ordre 0, place 4, octets [9], `Ecartee` (3) ; puis la faute
/// par son variant, et son texte s'il en a un.
fn octets_faute(nom: &str) -> Vec<u8> {
    let faute: Vec<u8> = match nom {
        "Faute::Budget" => vec![0],
        "Faute::Profondeur" => vec![1],
        "Faute::Panique" => vec![2, 1, b'x'],
        "Faute::Memoire" => vec![3],
        "Faute::HorsMemoire" => vec![4],
        "Faute::SortieInvalide" => vec![5],
        "Faute::Piege" => vec![6, 1, b'y'],
        "Faute::Attente" => vec![7],
        "Faute::VueInvalide" => vec![8, 1, b'z'],
        autre => panic!("aucun octet figé pour {autre}"),
    };
    cat(&[&h(2), &[0, 4, 1], &[1, 0, 0, 4, 1, 9, 3], &faute])
}

/// Une fiche de [`une_partie_pleine`] : l'appareil `n`, de rang `n - 1`, annoncé avec ce
/// rôle (0 téléphone, 1 manette) et aucune capacité, présent (Oui, 0), sa dernière
/// arrivée faite à (2, 0) et placée au pas 2, sa dernière nouvelle au pas 2.
fn octets_fiche(n: u8, role: u8) -> Vec<u8> {
    cat(&[
        &[n, n - 1, 1, role],
        &octets_capacites_aucunes(),
        &[0, 1, 2, 0, 2, 2],
    ])
}

/// La vue de l'appareil 1 est-elle dans l'état du cœur ? Au pas 3, oui ; au pas 2, pas
/// encore.
#[derive(Debug, Clone, Copy)]
enum Vue {
    Avec,
    Sans,
}

/// L'état du cœur de [`une_partie_pleine`], sans l'en-tête, au pas `pas` : avec la vue
/// de l'appareil 1 (au pas 3), ou sans (au pas 2).
fn octets_coeur_plein(pas: u8, vue: Vue) -> Vec<u8> {
    let vues: &[u8] = match vue {
        Vue::Avec => &[1, 1, 1, 2],
        Vue::Sans => &[0],
    };
    cat(&[
        // Partie 0, pas, ordre 5, séquence 5, synthèse 0, réveil Jamais (2), saut 0, ni
        // pause ni relâchement de pause (Non, Non), aucun Temps écarté (l'octet 0), ni
        // panne ni point à intervalle confirmé (Non, Non).
        &[0, pas, 5, 5, 0, 2, 0, 1, 1, 0, 1, 1],
        // Deux fiches, puis le prochain rang, 2.
        &[2],
        &octets_fiche(1, 0),
        &octets_fiche(2, 1),
        &[2],
        // Un tenu : appareil 1, rang 0, clé [0], son état : livré Some (1) d'une valeur
        // d'octets [0, 2], de clé [0], relâchée Some (1) [0, 0], pas relâchée (Non) ;
        // livré au pas 2 (Some), numéro 1 (Some), rien de retenu (None), fait Some à
        // (2, 0).
        &[1, 1, 0, 1, 0],
        &[1, 2, 0, 2, 1, 0, 1, 2, 0, 0, 1],
        &[1, 2, 1, 1, 0, 1, 2, 0],
        // Une manette : appareil 2, rang 1, son état : livrée Some (1) au repos ; livrée
        // au pas 2, numéro 1, rien de retenu, faite à (2, 0).
        &[1, 2, 1, 1],
        &octets_manette_au_repos(),
        &[1, 2, 1, 1, 0, 1, 2, 0],
        // Une demande : 1, pas répondue (Non). Lancées : [1]. Dernier point au pas 2.
        &[1, 1, 1],
        &[1, 1],
        &[2],
        // Les vues : (Appareil (1) 1, pas 2), ou aucune.
        vues,
    ])
}

/// L'état du cœur d'ouverture, sans l'en-tête : tout à zéro, réveil Jamais, rien de
/// vu.
fn octets_coeur_d_ouverture() -> Vec<u8> {
    vec![0, 0, 0, 0, 0, 2, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// Le point de [`une_partie_pleine`] : deux instantanés (celui des pas 2 et 3, que le
/// point et le dernier point à intervalle partagent, puis celui d'ouverture), chacun
/// tel que le modèle l'écrit, précédé de sa longueur ; puis trois points : le point
/// (instantané 0, son cœur, indice 8, sain : le point 2, à intervalle : le point 1), le
/// point à intervalle du pas 2 (instantané 0, indice 7, rien de cité), le point
/// d'ouverture (instantané 1, indice 0, rien de cité).
fn octets_point(pt: &Point) -> Vec<u8> {
    let ecrit = |p: &Point| {
        let mut o = Vec::new();
        p.instantane().ecrire(&mut o);
        o
    };
    let pas_2 = ecrit(pt);
    let ouverture = ecrit(pt.sain().unwrap().as_ref());
    let longueur = |o: &[u8]| vec![u8::try_from(o.len()).unwrap()];
    cat(&[
        &h(1),
        &[2],
        &longueur(&pas_2),
        &pas_2,
        &longueur(&ouverture),
        &ouverture,
        &[3],
        &[0],
        &octets_coeur_plein(3, Vue::Avec),
        &[8, 1, 2, 1, 1],
        &[0],
        &octets_coeur_plein(2, Vue::Sans),
        &[7, 0, 0],
        &[1],
        &octets_coeur_d_ouverture(),
        &[0, 0, 0],
    ])
}

#[test]
fn les_octets_de_chaque_variant_sont_figes() {
    assert_eq!(
        VERSION_GARDE, 1,
        "les octets de ce fichier sont ceux de la version 1"
    );
    let mut ecarts = Vec::new();
    let mut comparer = |nom: &str, ligne: Ligne, attendu: Vec<u8>| {
        let encode = encoder_lignes(0, Pas::du_fil(4), std::slice::from_ref(&ligne));
        if encode != attendu {
            ecarts.push(format!("{nom} : encodé {encode:?}, attendu {attendu:?}"));
        }
        match decoder_journal(&attendu) {
            Ok(j) if j.lignes() == [ligne] => {}
            autre => ecarts.push(format!("{nom} : les octets écrits se relisent {autre:?}")),
        }
    };
    for (nom, a) in arrivees_figees() {
        comparer(nom, ligne_de_l_arrivee(a), octets_arrivee(nom));
    }
    for (nom, f) in fautes_figees() {
        comparer(nom, ligne_de_la_faute(f), octets_faute(nom));
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
}

#[test]
fn les_octets_du_coeur_et_du_point_sont_figes() {
    let (p, pt) = une_partie_pleine();
    let mut ecarts = Vec::new();
    let coeur = cat(&[&h(4), &octets_coeur_plein(3, Vue::Avec)]);
    if encoder_coeur(p.coeur()) != coeur {
        ecarts.push(format!("cœur : encodé {:?}", encoder_coeur(p.coeur())));
    }
    if decoder_coeur(&coeur).as_ref() != Ok(p.coeur()) {
        ecarts.push(String::from(
            "cœur : les octets écrits se relisent autrement",
        ));
    }
    let point = octets_point(&pt);
    if encoder_point(&pt) != point {
        ecarts.push(format!("point : encodé {:?}", encoder_point(&pt)));
    }
    match decoder_point::<InstantaneModele>(&point) {
        Ok(relu) if relu == pt => {}
        autre => ecarts.push(format!("point : les octets écrits se relisent {autre:?}")),
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
}

/// Chaque variant de [`TOUS_LES_VARIANTS`] a ses octets figés, ici ou dans le journal
/// figé de `garde.rs`, et il n'y en a pas d'autre.
#[test]
fn les_octets_figes_couvrent_chaque_variant() {
    let mut noms = BTreeSet::new();
    variants_du_journal(&journal_fige(), &mut noms);
    for (nom, a) in arrivees_figees() {
        let j = morceau(ligne_de_l_arrivee(a));
        variants_du_journal(&j, &mut noms);
        assert!(
            noms.contains(nom),
            "{nom} : le cas ne tient pas son variant"
        );
    }
    for (nom, f) in fautes_figees() {
        variants_du_journal(&morceau(ligne_de_la_faute(f)), &mut noms);
        assert!(
            noms.contains(nom),
            "{nom} : le cas ne tient pas son variant"
        );
    }
    let attendus: BTreeSet<&str> = TOUS_LES_VARIANTS.iter().copied().collect();
    let manquent: Vec<_> = attendus.difference(&noms).collect();
    let en_trop: Vec<_> = noms.difference(&attendus).collect();
    assert!(
        manquent.is_empty() && en_trop.is_empty(),
        "sans octets figés : {manquent:?} ; absents de la liste : {en_trop:?}"
    );
}

// ---------------------------------------------------------------------------
// La seconde lecture (SAL-90)
// ---------------------------------------------------------------------------

/// Encode une valeur de l'ABI : ses octets sont figés par `jeu-abi`.
fn abi<T: serde::Serialize>(v: &T) -> Vec<u8> {
    octets(v)
}

/// Une fiche, d'après sa forme : appareil, rang, rôle et capacités s'il s'est annoncé,
/// présent, dernière arrivée, dernière nouvelle.
#[derive(Debug, PartialEq, Eq)]
struct FicheLue {
    appareil: u64,
    rang: u64,
    role: Option<u64>,
    present: u8,
    derniere: Option<(u64, u64, u64)>,
    derniere_nouvelle: u64,
}

/// Un état tenu, d'après sa forme : la valeur livrée (ses octets lus par `valeur`),
/// le pas de la livraison, le numéro, la valeur retenue et son instant, l'instant fait.
#[derive(Debug, PartialEq, Eq)]
struct TenuLu<V> {
    livre: Option<V>,
    livre_a: Option<u64>,
    numero: Option<u64>,
    retenue: Option<(V, (u64, u64))>,
    fait: Option<(u64, u64)>,
}

/// Une valeur de téléphone, d'après sa forme.
#[derive(Debug, PartialEq, Eq)]
struct ValeurLue {
    octets: Vec<u8>,
    cle: Vec<u8>,
    relache: Option<Vec<u8>>,
    est_relache: u8,
}

/// Un état du cœur, d'après sa forme, champ à champ.
#[derive(Debug, PartialEq, Eq)]
struct CoeurLu {
    entete: [u64; 7],
    drapeaux: [u8; 5],
    fiches: Vec<FicheLue>,
    prochain_rang: u64,
    tenus: Vec<(u64, u64, Vec<u8>, TenuLu<ValeurLue>)>,
    manettes: Vec<(u64, u64, TenuLu<Vec<u8>>)>,
    demandes: Vec<(u64, u8)>,
    lancees: Vec<u64>,
    dernier_point: u64,
    vues: Vec<(u64, u64)>,
}

impl Lecteur<'_> {
    fn oui(&mut self) -> u8 {
        let b = self.octet();
        assert!(b <= 1, "Oui (0) ou Non (1), pas {b}");
        b
    }

    fn instant(&mut self) -> (u64, u64) {
        (self.varint(), self.varint())
    }

    /// Une option : 0 None, 1 Some puis la valeur.
    fn optionnel<T>(&mut self, lire: impl FnOnce(&mut Self) -> T) -> Option<T> {
        match self.octet() {
            0 => None,
            1 => Some(lire(self)),
            x => panic!("option {x}"),
        }
    }

    /// Des octets de l'ABI qu'on attend, tels que `jeu-abi` les fige.
    fn abi(&mut self, attendus: &[u8]) {
        assert_eq!(self.brut(attendus.len()), attendus);
    }

    fn valeur(&mut self) -> ValeurLue {
        ValeurLue {
            octets: self.tranche().to_vec(),
            cle: self.tranche().to_vec(),
            relache: self.optionnel(|l| l.tranche().to_vec()),
            est_relache: self.oui(),
        }
    }

    fn tenu_lu<V>(&mut self, mut valeur: impl FnMut(&mut Self) -> V) -> TenuLu<V> {
        TenuLu {
            livre: self.optionnel(&mut valeur),
            livre_a: self.optionnel(Lecteur::varint),
            numero: self.optionnel(Lecteur::varint),
            retenue: self.optionnel(|l| {
                let v = valeur(l);
                (v, l.instant())
            }),
            fait: self.optionnel(Lecteur::instant),
        }
    }

    /// Un état du cœur, d'après sa forme. Les capacités annoncées et les manettes sont
    /// de l'ABI : on attend celles du scénario.
    fn coeur(&mut self, capacites: &[u8], manette: &[u8]) -> CoeurLu {
        let mut entete = [0; 7];
        for (i, e) in entete.iter_mut().enumerate() {
            *e = self.varint();
            // Le réveil `A(e)` porte son pas.
            if i == 5 && *e == 1 {
                let _a = self.varint();
            }
        }
        let mut drapeaux = [0; 5];
        drapeaux[0] = self.oui();
        drapeaux[1] = self.oui();
        drapeaux[2] = self.octet();
        drapeaux[3] = self.oui();
        drapeaux[4] = self.oui();
        let fiches = (0..self.varint())
            .map(|_| FicheLue {
                appareil: self.varint(),
                rang: self.varint(),
                role: self.optionnel(|l| {
                    let r = l.varint();
                    l.abi(capacites);
                    r
                }),
                present: self.oui(),
                derniere: self.optionnel(|l| {
                    let (p, f) = l.instant();
                    (p, f, l.varint())
                }),
                derniere_nouvelle: self.varint(),
            })
            .collect();
        let prochain_rang = self.varint();
        let tenus = (0..self.varint())
            .map(|_| {
                let appareil = self.varint();
                let rang = self.varint();
                let cle = self.tranche().to_vec();
                (appareil, rang, cle, self.tenu_lu(Lecteur::valeur))
            })
            .collect();
        let manettes = (0..self.varint())
            .map(|_| {
                let appareil = self.varint();
                let rang = self.varint();
                let etat = self.tenu_lu(|l| l.brut(manette.len()).to_vec());
                (appareil, rang, etat)
            })
            .collect();
        let demandes = (0..self.varint())
            .map(|_| (self.varint(), self.oui()))
            .collect();
        let lancees = (0..self.varint()).map(|_| self.varint()).collect();
        let dernier_point = self.varint();
        let vues = (0..self.varint())
            .map(|_| {
                let regard = match self.varint() {
                    0 => u64::MAX,
                    1 => self.varint(),
                    x => panic!("regard {x}"),
                };
                (regard, self.varint())
            })
            .collect();
        CoeurLu {
            entete,
            drapeaux,
            fiches,
            prochain_rang,
            tenus,
            manettes,
            demandes,
            lancees,
            dernier_point,
            vues,
        }
    }

    /// Une arrivée placée, d'après sa forme, comparée à celle qu'on attend.
    fn arrivee(&mut self, attendue: &ArriveePlacee) {
        let ArriveePlacee {
            sequence,
            place,
            fait,
            recu,
            vu,
            origine,
            quoi,
        } = attendue;
        assert_eq!(self.varint(), *sequence);
        assert_eq!(self.varint(), place.numero());
        assert_eq!(
            self.instant(),
            (fait.pas.numero(), u64::from(fait.fraction))
        );
        assert_eq!(
            self.instant(),
            (recu.pas.numero(), u64::from(recu.fraction))
        );
        assert_eq!(self.varint(), vu.numero());
        match (self.varint(), origine) {
            (0, Origine::Commun) | (1, Origine::Ecran) => {}
            (2, Origine::Appareil(r)) => assert_eq!(self.varint(), u64::from(*r)),
            (autre, o) => panic!("origine {autre} pour {o:?}"),
        }
        match (self.varint(), quoi) {
            (0, QuoiPlace::Geste { de, octets, id }) => {
                assert_eq!(self.varint(), de.fil());
                assert_eq!(self.tranche(), octets.as_slice());
                assert_eq!(self.varint(), id.fil());
            }
            (1, QuoiPlace::Tenu { de, numero, valeur }) => {
                assert_eq!(self.varint(), de.fil());
                assert_eq!(self.varint(), *numero);
                let ValeurTelephone {
                    octets,
                    cle,
                    relache,
                    est_relache,
                } = valeur;
                let lue = self.valeur();
                assert_eq!(
                    lue,
                    ValeurLue {
                        octets: octets.clone(),
                        cle: cle.clone(),
                        relache: relache.clone(),
                        est_relache: u8::from(*est_relache != Oui::Oui),
                    }
                );
            }
            (2, QuoiPlace::Manette { de, numero, etat }) => {
                assert_eq!(self.varint(), de.fil());
                assert_eq!(self.varint(), *numero);
                self.abi(&abi(etat));
            }
            (3, QuoiPlace::Ecran(e)) => self.ecran(e),
            (4, QuoiPlace::Presence(p)) => self.abi(&abi(p)),
            (5, QuoiPlace::Reponse { id, reponse }) => {
                assert_eq!(self.varint(), *id);
                self.reponse(reponse);
            }
            (autre, q) => panic!("variant {autre} pour {q:?}"),
        }
    }

    fn ecran(&mut self, e: &EcranArrivee) {
        match (self.varint(), e) {
            (0, EcranArrivee::Action(a)) => assert_eq!(self.tranche(), a.as_slice()),
            (1, EcranArrivee::Touche(t)) => self.abi(&abi(t)),
            (2, EcranArrivee::Pointeur(p)) => self.abi(&abi(p)),
            (3, EcranArrivee::Composant(c)) => match (self.varint(), c) {
                (0, ComposantArrivee::Saisie { champ, texte, fin }) => {
                    assert_eq!(self.varint(), champ.fil());
                    assert_eq!(self.tranche(), texte.as_bytes());
                    self.abi(&abi(fin));
                }
                (1, ComposantArrivee::Deplace(d)) => self.abi(&abi(d)),
                (2, ComposantArrivee::Fichiers { geste, fichiers }) => {
                    assert_eq!(self.varint(), geste.fil());
                    assert_eq!(self.varint(), u64::try_from(fichiers.len()).unwrap());
                    for f in fichiers {
                        self.abi(&abi(f));
                    }
                }
                (autre, c) => panic!("composant {autre} pour {c:?}"),
            },
            (autre, e) => panic!("écran {autre} pour {e:?}"),
        }
    }

    /// Une réponse : son variant, puis un `Result` (Ok 0, Err 1 et l'échec de l'ABI).
    fn reponse(&mut self, r: &ReponseArrivee) {
        let resultat = |l: &mut Self, r: Result<&[u8], &EchecDemande>| match (l.varint(), r) {
            (0, Ok(o)) => assert_eq!(l.tranche(), o),
            (1, Err(e)) => l.abi(&abi(e)),
            (autre, r) => panic!("résultat {autre} pour {r:?}"),
        };
        match (self.varint(), r) {
            (0, ReponseArrivee::Image(x))
            | (1, ReponseArrivee::Rendu(x))
            | (2, ReponseArrivee::Lu(x)) => {
                resultat(self, x.as_deref());
            }
            (3, ReponseArrivee::Fait(x)) => match (self.varint(), x) {
                (0, Ok(())) => {}
                (1, Err(e)) => self.abi(&abi(e)),
                (autre, x) => panic!("résultat {autre} pour {x:?}"),
            },
            (autre, r) => panic!("réponse {autre} pour {r:?}"),
        }
    }

    fn faute(&mut self, attendue: &Faute) {
        match (self.varint(), attendue) {
            (0, Faute::Budget)
            | (1, Faute::Profondeur)
            | (3, Faute::Memoire)
            | (4, Faute::HorsMemoire)
            | (5, Faute::SortieInvalide)
            | (7, Faute::Attente) => {}
            (2, Faute::Panique { message: t })
            | (6, Faute::Piege { code: t })
            | (8, Faute::VueInvalide { detail: t }) => assert_eq!(self.tranche(), t.as_bytes()),
            (autre, f) => panic!("faute {autre} pour {f:?}"),
        }
    }

    /// Le début d'un morceau d'une ligne : l'en-tête d'un journal, base 0, pas 4, une
    /// ligne de ce variant.
    fn morceau_d_une_ligne(&mut self, variant: u64) {
        self.entete(2);
        assert_eq!((self.varint(), self.varint(), self.varint()), (0, 4, 1));
        assert_eq!(self.varint(), variant);
    }
}

/// SAL-90 : le lecteur indépendant relit chaque arrivée et chaque faute figées.
#[test]
fn une_seconde_lecture_relit_chaque_variant() {
    for (nom, a) in arrivees_figees() {
        let octets = octets_arrivee(nom);
        let mut lu = Lecteur { octets: &octets };
        lu.morceau_d_une_ligne(0);
        assert_eq!(
            (lu.varint(), lu.varint()),
            (4, 1),
            "{nom} : pas 4, une arrivée"
        );
        lu.arrivee(&a);
        assert!(lu.fini(), "{nom} : des octets en trop");
    }
    for (nom, f) in fautes_figees() {
        let octets = octets_faute(nom);
        let mut lu = Lecteur { octets: &octets };
        lu.morceau_d_une_ligne(1);
        assert_eq!(
            (lu.varint(), lu.varint(), lu.varint()),
            (0, 0, 4),
            "{nom} : partie, ordre, place"
        );
        assert_eq!(lu.tranche(), [9]);
        assert_eq!(lu.varint(), 3, "{nom} : Ecartee");
        lu.faute(&f);
        assert!(lu.fini(), "{nom} : des octets en trop");
    }
}

/// L'état du cœur de [`une_partie_pleine`] au pas `pas`, tel qu'on l'attend d'après le
/// scénario : avec la vue de l'appareil 1, ou sans.
fn coeur_attendu(pas: u64, vue: Vue) -> CoeurLu {
    fn tenu<V>(livre: V) -> TenuLu<V> {
        TenuLu {
            livre: Some(livre),
            livre_a: Some(2),
            numero: Some(1),
            retenue: None,
            fait: Some((2, 0)),
        }
    }
    let fiche = |n: u64| FicheLue {
        appareil: n,
        rang: n - 1,
        role: Some(n - 1),
        present: 0,
        derniere: Some((2, 0, 2)),
        derniere_nouvelle: 2,
    };
    CoeurLu {
        entete: [0, pas, 5, 5, 0, 2, 0],
        drapeaux: [1, 1, 0, 1, 1],
        fiches: vec![fiche(1), fiche(2)],
        prochain_rang: 2,
        tenus: vec![(
            1,
            0,
            vec![0],
            tenu(ValeurLue {
                octets: vec![0, 2],
                cle: vec![0],
                relache: Some(vec![0, 0]),
                est_relache: 1,
            }),
        )],
        manettes: vec![(2, 1, tenu(abi(&Manette::REPOS)))],
        demandes: vec![(1, 1)],
        lancees: vec![1],
        dernier_point: 2,
        vues: match vue {
            Vue::Avec => vec![(1, 2)],
            Vue::Sans => vec![],
        },
    }
}

/// SAL-90 : le lecteur indépendant relit un état du cœur qui a de chaque chose, et le
/// point dont la chaîne a trois points et deux instantanés.
#[test]
fn une_seconde_lecture_relit_le_coeur_et_le_point_pleins() {
    let (p, pt) = une_partie_pleine();
    let capacites = abi(&Capacites::AUCUNE);
    let manette = abi(&Manette::REPOS);

    let octets = encoder_coeur(p.coeur());
    let mut lu = Lecteur { octets: &octets };
    lu.entete(4);
    assert_eq!(lu.coeur(&capacites, &manette), coeur_attendu(3, Vue::Avec));
    assert!(lu.fini());

    let octets = encoder_point(&pt);
    let mut lu = Lecteur { octets: &octets };
    lu.entete(1);
    assert_eq!(
        lu.varint(),
        2,
        "deux instantanés : les pas 2 et 3 partagent le leur"
    );
    for q in [&pt, pt.sain().unwrap().as_ref()] {
        let mut o = Vec::new();
        q.instantane().ecrire(&mut o);
        assert_eq!(lu.tranche(), o.as_slice());
    }
    assert_eq!(lu.varint(), 3, "trois points");
    // Le point, au pas 3.
    assert_eq!(lu.varint(), 0);
    assert_eq!(lu.coeur(&capacites, &manette), coeur_attendu(3, Vue::Avec));
    assert_eq!(lu.varint(), u64::try_from(pt.indice()).unwrap());
    assert_eq!(
        lu.option(),
        Some(2),
        "sain : le point d'ouverture, le dernier"
    );
    assert_eq!(lu.option(), Some(1), "à intervalle : celui du pas 2");
    // Le point à intervalle du pas 2 : la vue de l'appareil 1 vient après lui.
    assert_eq!(lu.varint(), 0, "le même instantané que le point");
    assert_eq!(lu.coeur(&capacites, &manette), coeur_attendu(2, Vue::Sans));
    let periodique = pt.periodique().unwrap();
    assert_eq!(lu.varint(), u64::try_from(periodique.indice()).unwrap());
    assert_eq!((lu.option(), lu.option()), (None, None));
    // Le point d'ouverture.
    assert_eq!(lu.varint(), 1);
    let ouverture = lu.coeur(&capacites, &manette);
    assert_eq!(ouverture.entete, [0, 0, 0, 0, 0, 2, 0]);
    assert!(ouverture.fiches.is_empty() && ouverture.tenus.is_empty());
    assert_eq!(lu.varint(), 0, "indice 0");
    assert_eq!((lu.option(), lu.option()), (None, None));
    assert!(lu.fini());
}
