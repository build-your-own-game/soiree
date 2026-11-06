//! Une soirée en mémoire sur le module d'un jeu (PHASE-3, § 8.1, lot 4b) : la vraie
//! autorité sur wasmtime, de vrais appareils qui jouent les coups des bots du jeu par le
//! vrai chemin (numérotation, file, chiffrement, relais en mémoire), et les pannes tirées
//! de la graine : coupures, rechargements, onglets jumeaux, poignées de main rejouées,
//! autorité tuée puis relancée depuis la garde. Puis une fin calme.
//!
//! ```sh
//! cargo run --release -p salle --example soiree_en_memoire -- <module.wasm> \
//!     [--telephones N] [--duree S] [--graines 1,2,3]
//! ```
//!
//! Juge, pour chaque graine : aucun arrêt de l'autorité ; la chaîne entière des cases
//! écrites s'ouvre case après case et porte un journal d'un seul tenant ; aucun geste n'y
//! est deux fois ; tout geste accusé y est ; à la fin, plus rien n'attend d'accusé ; la
//! garde finale reprend une partie de même forme canonique et de même empreinte que
//! l'autorité vivante ; quelqu'un a joué (un geste d'un téléphone, ou un coup du bot de
//! l'écran). Code 0 si tout tient, 1 sinon, 2 si l'outil n'a pas pu jouer.
//! Sans téléphone (un jeu qui n'en a pas, Tier list), seul l'écran joue, par son bot :
//! la soirée n'a alors pas d'autre panne que l'autorité tuée et relancée.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "un exemple qui rapporte sur la sortie"
)]

use std::process::ExitCode;

use jeu_abi::{ApplicableFil, Oui};
use moteur_wasmtime::{InstantaneWasm, Moteur, ReglagesMoteur};
use salle::horloge::us_de_duree;
use salle::simulation::{Pannes, Soiree, verifier_l_historique};

struct Demande {
    module: String,
    telephones: Option<usize>,
    duree_s: u64,
    graines: Vec<u64>,
}

fn lire(args: &[String]) -> Result<Demande, String> {
    let mut d = Demande {
        module: String::new(),
        telephones: None,
        duree_s: 120,
        graines: vec![1, 2, 3],
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--telephones" => {
                let v = it.next().ok_or("--telephones sans valeur")?;
                d.telephones = Some(v.parse().map_err(|e| format!("--telephones : {e}"))?);
            }
            "--duree" => {
                let v = it.next().ok_or("--duree sans valeur")?;
                d.duree_s = v.parse().map_err(|e| format!("--duree : {e}"))?;
            }
            "--graines" => {
                let v = it.next().ok_or("--graines sans valeur")?;
                d.graines = v
                    .split(',')
                    .map(|g| g.parse().map_err(|e| format!("--graines : {e}")))
                    .collect::<Result<_, _>>()?;
            }
            m if d.module.is_empty() => d.module = String::from(m),
            autre => return Err(format!("argument inconnu : {autre}")),
        }
    }
    if d.module.is_empty() {
        return Err(String::from(
            "usage : soiree_en_memoire <module.wasm> [--telephones N] [--duree S] [--graines 1,2,3]",
        ));
    }
    Ok(d)
}

/// Joue la soirée de cette graine, et rend ce qui n'a pas tenu.
fn jouer_et_juger<F: FnMut() -> moteur_wasmtime::FabriqueWasm>(
    s: &mut Soiree<moteur_wasmtime::FabriqueWasm, F>,
    graine: u64,
    telephones: usize,
    duree_s: u64,
) -> Vec<String> {
    let b = s.jouer(telephones, duree_s.saturating_mul(1_000_000));
    let mut fautes: Vec<String> = Vec::new();
    if let Some(a) = &b.arret {
        fautes.push(format!("l'autorité s'est arrêtée : {a}"));
    }
    if b.redemarrages == 0 {
        fautes.push(String::from("aucun redémarrage de l'autorité"));
    }
    if b.gestes_emis == 0 && b.autorite.coups_ecran == 0 {
        fautes.push(String::from(
            "personne n'a joué : ni geste d'un téléphone, ni coup du bot de l'écran",
        ));
    }
    let historique = s.relais.historique();
    match verifier_l_historique::<InstantaneWasm>(&s.cle_de_garde(), s.salle(), &historique) {
        Ok(h) => {
            if !h.en_double.is_empty() {
                fautes.push(format!("{} geste(s) écrits deux fois", h.en_double.len()));
            }
            for (k, t) in s.telephones.iter().enumerate() {
                let jumeau = s.telephones.iter().take(k).any(|u| u.jeton == t.jeton);
                if !jumeau && t.appareil.en_file() > 0 {
                    fautes.push(format!(
                        "téléphone {k} : {} geste(s) jamais accusés",
                        t.appareil.en_file()
                    ));
                }
            }
            // Tout geste accusé est écrit ; un appareil introuvable est une faute.
            fautes.extend(s.accuses_non_ecrits(&h));
            println!(
                "  graine {graine} : {} cases, {} points, {} lignes, {} gestes écrits",
                h.cases,
                h.points,
                h.lignes,
                h.gestes.len()
            );
        }
        Err(e) => fautes.push(format!("la chaîne de la soirée : {e}")),
    }
    if let Err(e) = s.comparer_a_la_garde() {
        fautes.push(format!("la garde finale : {e}"));
    }
    println!(
        "  graine {graine} : {} pas, {} redémarrages, {} coupures, {} rechargements, {} jumeaux, {} poignées rejouées ; {} gestes émis, {} accusés ; {:?} ; {} demandes",
        b.pas,
        b.redemarrages,
        b.coupures,
        b.rechargements,
        b.jumeaux,
        b.poignees_rejouees,
        b.gestes_emis,
        b.gestes_accuses,
        b.autorite,
        b.demandes,
    );
    fautes
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let d = match lire(&args) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let octets = match std::fs::read(&d.module) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("lire {} : {e}", d.module);
            return ExitCode::from(2);
        }
    };
    let moteur = match Moteur::new(ReglagesMoteur::PHASE_3) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("le moteur : {e}");
            return ExitCode::from(2);
        }
    };
    let charge = match moteur.charger(&octets) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("le module est refusé : {e}");
            return ExitCode::from(2);
        }
    };
    let declaration = charge.declaration();
    let telephones = d.telephones.unwrap_or(match declaration.telephones {
        ApplicableFil::Oui(_) => usize::from(declaration.places.appareils_max()).min(8),
        ApplicableFil::SansObjet(_) => 0,
    });
    let redite_us = match &declaration.telephones {
        ApplicableFil::Oui(t) => liaison::periode_de_redite(&t.tenu).map(us_de_duree),
        ApplicableFil::SansObjet(_) => None,
    };
    println!(
        "soirée en mémoire : {} ({} octets, blake3 {}), {telephones} téléphone(s), {} s, graines {:?}",
        declaration.nom.as_str(),
        octets.len(),
        blake3::hash(&octets).to_hex(),
        d.duree_s,
        d.graines
    );
    let mut echecs = 0u32;
    for &graine in &d.graines {
        let mut s = Soiree::new(|| charge.fabrique(), graine, Oui::Oui, Pannes::SOIREE);
        // Les téléphones redisent leur tenu à la période du jeu (P27).
        s.avec_redite(redite_us);
        let fautes = jouer_et_juger(&mut s, graine, telephones, d.duree_s);
        if fautes.is_empty() {
            println!("  graine {graine} : réussie");
        } else {
            echecs = echecs.saturating_add(1);
            for f in fautes {
                println!("  graine {graine} : ÉCHEC : {f}");
            }
        }
    }
    if echecs == 0 {
        println!("réussie");
        ExitCode::SUCCESS
    } else {
        println!("{echecs} graine(s) en échec");
        ExitCode::from(1)
    }
}
