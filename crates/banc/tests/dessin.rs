//! Le jugement du scénario `dessin` (PHASE-3, lot 9), sans navigateur : les points où
//! relire un pixel (OUT-30, au repos : OUT-32), et ce qui fait échouer un rapport.

use std::collections::BTreeSet;

use banc::dessin::{
    BUDGET_IMAGE_MS, JeuPrepare, PASSAGES, ScenePreparee, Sonde, agencements, compter_les_images,
    juger, noeud_a_oublier, sondes,
};
use jeu_abi::{
    Animation, Cle, CompteAReboursFil, Courbe, Duree, Fixe, FormeEcran, Instant, NoeudFil, Oui,
    Pas, Rect, Rgba, SceneFil, SorteNoeud, StyleTexte, Visuel,
};
use mise_en_page::{MiseEnPage, Polices, bouton};
use serde_json::{Value as Json, json};

const FOND: Rgba = Rgba(5, 9, 28, 255);
const FORME: FormeEcran = FormeEcran::Scene {
    largeur: 400,
    hauteur: 200,
    zone_sure_pour_cent: 90,
    echelle_entiere: Oui::Non,
};

fn rect(x: i32, y: i32, l: i32, h: i32) -> Rect {
    Rect {
        x: Fixe::unites(x),
        y: Fixe::unites(y),
        l: Fixe::unites(l),
        h: Fixe::unites(h),
    }
}

fn noeud(k: u64, sorte: SorteNoeud) -> NoeudFil {
    NoeudFil::simple(Cle::du_fil(k), None, sorte)
}

fn style() -> StyleTexte {
    StyleTexte {
        police: Cle::POLICE_DU_COMMUN,
        taille: jeu_abi::Taille::Fixe(Fixe::unites(10)),
        couleur: Rgba::BLANC,
        alignement: jeu_abi::Alignement::Centre,
        lignes: jeu_abi::Lignes::Une,
        coupe: jeu_abi::Coupe::Mots,
    }
}

fn dans(s: &Sonde, r: Rect) -> bool {
    let (x, y) = (f64::from(s.x), f64::from(s.y));
    x >= r.x.en_unites()
        && y >= r.y.en_unites()
        && x < r.x.en_unites() + r.l.en_unites()
        && y < r.y.en_unites() + r.h.en_unites()
}

/// Une scène : un grand rectangle rouge, un vert par-dessus son coin, un texte sur son
/// centre, un rectangle animé (un saut), puis un dernier rectangle bleu sur le rouge.
fn scene() -> SceneFil {
    let mut anime = noeud(
        4,
        SorteNoeud::Rect {
            r: rect(300, 20, 40, 40),
            couleur: Rgba(1, 2, 3, 255),
        },
    );
    anime.anime = Some((
        Instant::de(Pas::ZERO),
        Animation {
            duree: Duree::pas(30),
            de: Visuel {
                decalage: (Fixe::ZERO, Fixe::ZERO),
                echelle_pour_mille: 1000,
                opacite: 255,
            },
            a: Visuel {
                decalage: (Fixe::ZERO, Fixe::unites(100)),
                echelle_pour_mille: 1000,
                opacite: 255,
            },
            courbe: Courbe::Rebond,
        },
    ));
    SceneFil {
        fond: Some(FOND),
        noeuds: vec![
            noeud(
                1,
                SorteNoeud::Rect {
                    r: rect(10, 10, 200, 100),
                    couleur: Rgba(200, 0, 0, 255),
                },
            ),
            noeud(
                2,
                SorteNoeud::Rect {
                    r: rect(10, 10, 60, 40),
                    couleur: Rgba(0, 200, 0, 255),
                },
            ),
            noeud(
                3,
                SorteNoeud::Texte {
                    texte: String::from("Bonjour"),
                    boite: rect(80, 40, 60, 30),
                    style: style(),
                },
            ),
            anime,
            noeud(
                5,
                SorteNoeud::Rect {
                    r: rect(150, 70, 40, 30),
                    couleur: Rgba(0, 0, 200, 255),
                },
            ),
        ],
        ..SceneFil::vide()
    }
}

#[test]
fn les_points_relus_evitent_ce_qui_recouvre_et_ce_qui_s_anime_out30_out32() {
    let s = sondes(&scene(), FORME, Rgba::NOIR, &BTreeSet::new());
    let du = |n: u32| s.iter().filter(move |p| p.noeud == Some(n));
    // Le rouge est relu, hors du vert, du texte et du bleu qui le recouvrent.
    let rouge: Vec<&Sonde> = du(0).collect();
    assert_eq!(rouge.len(), 1);
    assert_eq!(rouge[0].attendu, Rgba(200, 0, 0, 255));
    for r in [
        rect(10, 10, 60, 40),
        rect(80, 40, 60, 30),
        rect(150, 70, 40, 30),
    ] {
        assert!(!dans(rouge[0], r), "{:?} est recouvert", rouge[0]);
    }
    // Le vert et le bleu, que rien ne recouvre, le sont ; le rectangle animé jamais.
    assert_eq!(du(1).count(), 1);
    assert_eq!(du(4).count(), 1);
    assert_eq!(du(3).count(), 0);
    // Le bleu est le dernier rectangle : l'oublier montre le rouge, qui en diffère.
    assert_eq!(du(4).next().unwrap().sensible, Oui::Oui);
    assert_eq!(du(1).next().unwrap().sensible, Oui::Non);
    // Le fond : loin de toute boîte, animée comprise (au départ comme à l'arrivée).
    let fond: Vec<&Sonde> = s.iter().filter(|p| p.noeud.is_none()).collect();
    assert_ne!(fond.len(), 0);
    for p in fond {
        assert_eq!(p.attendu, FOND);
        for r in [
            rect(2, 2, 216, 116),
            rect(292, 12, 56, 56),
            rect(292, 112, 56, 56),
        ] {
            assert!(!dans(p, r), "{p:?} trop près d'un nœud");
        }
    }
}

#[test]
fn le_noeud_oublie_est_le_dernier_dont_l_oubli_se_voit() {
    // Un dernier rectangle posé sur le texte : sous lui, aucune couleur connue ; son
    // oubli ne se verrait à aucun point relu. Le contrôle négatif oublie encore le bleu.
    let mut scene = scene();
    scene.noeuds.push(noeud(
        6,
        SorteNoeud::Rect {
            r: rect(90, 45, 20, 20),
            couleur: Rgba(250, 250, 0, 255),
        },
    ));
    let s = sondes(&scene, FORME, Rgba::NOIR, &BTreeSet::new());
    let du = |n: u32| s.iter().find(move |p| p.noeud == Some(n));
    assert_eq!(du(5).unwrap().sensible, Oui::Non);
    assert_eq!(du(4).unwrap().sensible, Oui::Oui);
    assert_eq!(s.iter().filter(|p| p.sensible == Oui::Oui).count(), 1);
    assert_eq!(noeud_a_oublier(&scene, &s), Some(4));
    // Sans aucun point sensible, le dernier `Rect`.
    assert_eq!(noeud_a_oublier(&scene, &[]), Some(5));
}

#[test]
fn un_texte_qui_deborde_couvre_tout() {
    let mut debordent = BTreeSet::new();
    debordent.insert(2);
    let s = sondes(&scene(), FORME, Rgba::NOIR, &debordent);
    // Le texte est après le rouge et le vert : ni l'un ni l'autre, ni le fond, n'est relu.
    assert!(s.iter().all(|p| p.noeud == Some(4)), "{s:?}");
}

#[test]
fn les_agencements_attendus_comprennent_boutons_et_comptes() {
    let mut scene = scene();
    let b = rect(10, 150, 100, 30);
    scene.noeuds.push(noeud(
        6,
        SorteNoeud::Bouton {
            boite: b,
            nom: String::from("Valider"),
            action: vec![1],
        },
    ));
    scene.noeuds.push(noeud(
        7,
        SorteNoeud::CompteARebours {
            compte: CompteAReboursFil::Court {
                echeance: Pas::du_fil(600),
            },
            boite: rect(200, 150, 50, 30),
            style: style(),
        },
    ));
    let mut m = MiseEnPage::new(&Polices::du_commun());
    let (a, debordent) = agencements(&scene, Pas::ZERO, &mut m);
    assert_eq!(a.keys().copied().collect::<Vec<_>>(), [2, 5, 6]);
    assert!(debordent.is_empty());
    let nom = m.agencer("Valider", bouton::boite_du_nom(b), &bouton::style_du_nom(b));
    assert_eq!(
        a[&5],
        blake3::hash(&nom.octets_canoniques()).to_hex().to_string()
    );
    let compte = m.agencer("10", rect(200, 150, 50, 30), &style());
    assert_eq!(
        a[&6],
        blake3::hash(&compte.octets_canoniques())
            .to_hex()
            .to_string()
    );
}

/// Un jeu préparé d'une scène, et le rapport fidèle qu'une page en donnerait.
fn jeu_et_rapport() -> (JeuPrepare, Json) {
    let scene = scene();
    let mut m = MiseEnPage::new(&Polices::du_commun());
    let (agences, debordent) = agencements(&scene, Pas::ZERO, &mut m);
    let points = sondes(&scene, FORME, Rgba::NOIR, &debordent);
    let jeu = JeuPrepare {
        nom: String::from("essai"),
        empreinte: String::new(),
        forme: String::from("forme.bin"),
        ressources: Vec::new(),
        scenes: vec![ScenePreparee {
            indice: 0,
            noeuds: scene.noeuds.len(),
            sondes: points.clone(),
            agencements: agences.clone(),
            ressources: Vec::new(),
            fichier: String::from("scene-0.bin"),
            pire_cas: Oui::Oui,
            oublie: Some(4),
        }],
    };
    let lus: Vec<Json> = points
        .iter()
        .map(|p| json!([p.attendu.0, p.attendu.1, p.attendu.2, p.attendu.3]))
        .collect();
    let rapport = json!({
        "moteur": "essai",
        "sorte": "webgpu",
        "jeux": [{
            "nom": "essai",
            "scenes": [{
                "indice": 0, "noeuds": 5, "durees_ms": [12.0, 2.0, 2.0],
                "sondes": lus,
                "agencements": agences.iter().map(|(n, h)| json!([n, h])).collect::<Vec<_>>(),
                "ecarts_tampon": [], "erreurs_gpu": [],
            }],
            "images_illisibles": [],
            "perte": { "perdu_ms": 0.1, "image_ms": 3.0, "retour_ms": 5.0, "sondes": lus },
        }],
    });
    (jeu, rapport)
}

#[test]
fn un_rapport_fidele_reussit() {
    let (jeu, rapport) = jeu_et_rapport();
    let (v, constats) = juger(&rapport, &[jeu], PASSAGES[0], 3);
    assert!(v.is_ok(), "{v:?}");
    assert!(
        constats.iter().any(|c| c.contains("GPU perdu")),
        "{constats:?}"
    );
}

/// Un défaut injecté dans un rapport : son nom, ce qu'il change, ce que l'échec dit.
type Abimer = (&'static str, fn(&mut Json), &'static str);

#[test]
fn chaque_ecart_fait_echouer_le_rapport() {
    let (jeu, rapport) = jeu_et_rapport();
    let ecarts: [Abimer; 9] = [
        (
            "agencement",
            |r| r["jeux"][0]["scenes"][0]["agencements"][0][1] = json!("00"),
            "agencements différents",
        ),
        (
            "pixel",
            |r| {
                // Trois de plus sur un canal : au-delà de la tolérance de deux.
                let c = &mut r["jeux"][0]["scenes"][0]["sondes"][0][1];
                *c = json!(c.as_u64().unwrap() + 3);
            },
            "pixel",
        ),
        (
            "budget",
            |r| r["jeux"][0]["scenes"][0]["durees_ms"][0] = json!(BUDGET_IMAGE_MS + 0.5),
            "au-delà de 33 ms",
        ),
        (
            "GPU",
            |r| r["jeux"][0]["scenes"][0]["erreurs_gpu"] = json!(["Validation Error"]),
            "erreur du GPU",
        ),
        (
            "perte",
            |r| r["jeux"][0]["perte"] = json!({ "erreur": "la perte n'est pas signalée" }),
            "perte du GPU",
        ),
        (
            "tampon",
            |r| r["jeux"][0]["scenes"][0]["ecarts_tampon"] = json!(["ligne 0"]),
            "tampon",
        ),
        (
            "point non rapporté",
            |r| {
                let _: Option<Json> = r["jeux"][0]["scenes"][0]["sondes"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            },
            "points rapportés pour",
        ),
        (
            "point hors de la toile",
            |r| r["jeux"][0]["scenes"][0]["sondes"][0] = Json::Null,
            "hors de la toile",
        ),
        (
            "durée manquante",
            |r| r["jeux"][0]["scenes"][0]["durees_ms"] = json!([12.0, 2.0]),
            "durée(s) d'image rapportée(s) pour 3",
        ),
    ];
    for (quoi, abimer, attendu) in ecarts {
        let mut r = rapport.clone();
        abimer(&mut r);
        let (v, _) = juger(&r, std::slice::from_ref(&jeu), PASSAGES[0], 3);
        let e = v.expect_err(quoi);
        assert!(e.contains(attendu), "{quoi} : {e}");
    }
    // Deux près par canal : encore juste ; trois : faux.
    let mut r = rapport.clone();
    let v0 = r["jeux"][0]["scenes"][0]["sondes"][0][1].as_u64().unwrap();
    r["jeux"][0]["scenes"][0]["sondes"][0][1] = json!(v0 + 2);
    assert!(
        juger(&r, std::slice::from_ref(&jeu), PASSAGES[0], 3)
            .0
            .is_ok()
    );
    // WebGL2 forcé mais WebGPU rapporté.
    let (v, _) = juger(&rapport, std::slice::from_ref(&jeu), PASSAGES[1], 3);
    assert!(v.unwrap_err().contains("WebGL2 forcé"));
}

#[test]
fn le_controle_negatif_doit_voir_l_oubli_et_rien_d_autre() {
    let (jeu, rapport) = jeu_et_rapport();
    let negatif = PASSAGES[2];
    assert_eq!(negatif.fautif, "oublier-un-noeud");
    // Le rapport fidèle, sous le contrôle négatif : l'oubli ne se voit pas, échec.
    let (v, _) = juger(&rapport, std::slice::from_ref(&jeu), negatif, 3);
    assert!(v.unwrap_err().contains("ne se voit à aucun pixel"));
    // Le point du bleu montre le rouge : l'oubli se voit, réussi.
    let k = jeu.scenes[0]
        .sondes
        .iter()
        .position(|p| p.noeud == Some(4))
        .unwrap();
    let mut r = rapport.clone();
    r["jeux"][0]["scenes"][0]["sondes"][k] = json!([200, 0, 0, 255]);
    assert!(juger(&r, std::slice::from_ref(&jeu), negatif, 3).0.is_ok());
    // Un autre pixel qui change aussi : échec.
    let autre = usize::from(k == 0);
    r["jeux"][0]["scenes"][0]["sondes"][autre] = json!([9, 9, 9, 255]);
    let (v, _) = juger(&r, std::slice::from_ref(&jeu), negatif, 3);
    assert!(v.unwrap_err().contains("hors du nœud oublié"));
}

#[test]
fn la_trace_compte_les_images_perdues() {
    let ev = vec![
        json!({ "name": "DroppedFrame", "ph": "I" }),
        json!({ "name": "DroppedFrame", "ph": "I" }),
        json!({ "name": "FramePresented" }),
        json!({ "name": "Autre" }),
    ];
    let c = compter_les_images(&ev);
    assert_eq!(c["dropped_frame"], 2);
    assert_eq!(c["par_nom"]["FramePresented"], 1);
    assert_eq!(c["evenements"], 4);
}
