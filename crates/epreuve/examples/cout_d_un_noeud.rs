//! Le coût d'un nœud de scène, en carburant (RESTE-PHASE-2, point 12 ; PHASE-3, § 6 et
//! lot 6) : la vue de l'écran d'`ardoise`, lue après chaque trait écrit.
//!
//! ```text
//! cargo build -p ardoise-module --target wasm32-unknown-unknown --profile module
//! cargo run --release -p epreuve --example cout_d_un_noeud -- <ardoise_module.wasm>
//! ```
//!
//! Chaque trait d'`ardoise` est un rectangle de la scène, et rien d'autre ne change
//! dans la vue quand un trait s'ajoute (hormis, au premier, le bouton « Annuler ») :
//! l'écart de carburant de `Regarder` d'un trait au suivant est le coût d'un nœud
//! `Rect`, sa clé comprise (`Cle::de(&("trait", i))`), son encodage et celui de la
//! réponse. Le carburant compte les instructions du module réécrit : il est le même
//! sur toute machine, et la mesure n'a pas besoin d'une machine calme.
//!
//! Le programme n'utilise que ce que l'ABI 3 et l'ABI 4 ont en commun (`Bonjour`,
//! `Decrire`, `Creer`, `Entree`, `Regarder`) : le même source mesure le commun avant et
//! après le lot 6.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use jeu_abi::{Appel, EcranFil, EntreeFil, QuandFil, RegardFil, TexteCourt, encoder};
use moteur_wasmtime::{Moteur, ReglagesMoteur};
use partie::{Echec, Fabrique, Instance};

/// Le carburant d'un appel de la mesure : celui d'une lecture de la partie, large.
const BUDGET: u64 = 1 << 28;
/// Le pas de la création.
const PAS: u64 = 600;

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    if let Err(e) = encoder(v, &mut o) {
        eprintln!("encodage : {e}");
        o.clear();
    }
    o
}

/// Appelle `a` et rend son carburant.
fn appeler<I: Instance>(i: &mut I, a: &Appel<'_>) -> Result<u64, Echec> {
    let _: Vec<u8> = i.appeler(a.sorte(), &encode(a), BUDGET)?;
    Ok(i.carburant().unwrap_or(0))
}

fn mesurer(chemin: &str) -> Result<(), String> {
    let octets = std::fs::read(chemin).map_err(|e| format!("lire {chemin} : {e}"))?;
    let moteur = Moteur::new(ReglagesMoteur::PHASE_3).map_err(|e| e.to_string())?;
    let module = moteur.charger(&octets).map_err(|e| e.to_string())?;
    let mut i = Fabrique::neuve(&mut module.fabrique()).map_err(|e| e.to_string())?;
    let echec = |e: Echec| format!("{e:?}");
    let presents = encode(&Vec::<jeu_abi::Presence>::new());
    let _: u64 = appeler(&mut i, &Appel::Bonjour).map_err(echec)?;
    let _: u64 = appeler(&mut i, &Appel::Decrire).map_err(echec)?;
    let _: u64 = appeler(
        &mut i,
        &Appel::Creer {
            graine: [7; 32],
            pas: PAS,
            heure: 0,
            salle: TexteCourt::new("ABCD"),
            presents: &presents,
            garde: None,
        },
    )
    .map_err(echec)?;
    let regarder = Appel::Regarder {
        regards: vec![RegardFil::Ecran],
    };
    let mut avant = appeler(&mut i, &regarder).map_err(echec)?;
    println!("traits 0 : Regarder {avant}");
    let mut ecarts = Vec::new();
    for k in 1u64..=u64::try_from(ardoise::TRAITS_MAX).unwrap_or(24) {
        let chiffre = ardoise::Chiffre::new(i64::try_from(k % 10).unwrap_or(0))
            .ok_or_else(|| String::from("chiffre"))?;
        let action = encode(&ardoise::ActionEcran::Ecrire(chiffre));
        let pas = PAS.saturating_add(k);
        let entree = encode(&EntreeFil::Ecran {
            quoi: EcranFil::Action(&action),
            quand: QuandFil {
                fait: pas,
                fraction: 0,
                recu: pas,
                vu: pas,
                id: k,
            },
        });
        let _: u64 = appeler(
            &mut i,
            &Appel::Entree {
                ordre: k,
                place: pas,
                entree: &entree,
            },
        )
        .map_err(echec)?;
        let c = appeler(&mut i, &regarder).map_err(echec)?;
        let ecart = c.saturating_sub(avant);
        println!("traits {k} : Regarder {c}, écart {ecart}");
        // Le premier trait ajoute aussi le bouton « Annuler » : il ne compte pas.
        if k > 1 {
            ecarts.push(ecart);
        }
        avant = c;
    }
    ecarts.sort_unstable();
    let n = ecarts.len();
    let somme: u64 = ecarts.iter().sum();
    println!(
        "un nœud Rect, du 2ᵉ au {}ᵉ trait : {n} écarts, de {} à {}, médiane {}, moyenne {}",
        ardoise::TRAITS_MAX,
        ecarts.first().copied().unwrap_or(0),
        ecarts.last().copied().unwrap_or(0),
        ecarts.get(n / 2).copied().unwrap_or(0),
        somme
            .checked_div(u64::try_from(n).unwrap_or(1))
            .unwrap_or(0)
    );
    Ok(())
}

fn main() -> ExitCode {
    let Some(chemin) = std::env::args().nth(1) else {
        eprintln!("usage : cout_d_un_noeud <ardoise_module.wasm>");
        return ExitCode::from(64);
    };
    match mesurer(&chemin) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(2)
        }
    }
}
