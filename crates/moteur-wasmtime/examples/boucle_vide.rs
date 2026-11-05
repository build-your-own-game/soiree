//! M10 (PHASE-3, § 8.6) : la durée d'une unité de carburant sur une boucle vide, dans
//! wasmtime, sur la machine qui le lance.
//!
//! ```text
//! cargo run --release -p moteur-wasmtime --example boucle_vide -- [répétitions]
//! ```
//!
//! Le module de `Bonjour` qui boucle (`moteur_commun::essais::BONJOUR_QUI_BOUCLE`) se
//! charge sous un garde-fou mural de dix minutes : son premier `Bonjour` brûle tout
//! [`BUDGET_DE_CHARGEMENT`] puis tombe en faute `Budget`. Un témoin, le même module
//! dont `soiree_appel` piège dès son entrée, se charge de la même façon : réécriture,
//! compilation, instanciation, un appel. La durée d'une unité est
//! `(médiane de la boucle − médiane du témoin) / BUDGET_DE_CHARGEMENT` ; le carburant
//! brûlé dépasse le budget d'au plus un segment de la boucle.
//!
//! C'est l'unité qui compte pour le garde-fou du chargement (P5) : une boucle vide coûte
//! bien plus par unité que le code d'un jeu (RESTE-PHASE-2, point 2). Pour une mesure
//! sous charge, lancer d'abord quatorze `yes > /dev/null` (dix cœurs occupés), comme
//! au point 2 de RESTE-PHASE-2. Les durées ne valent que pour cette machine, sa charge
//! et wasmtime ; qui le lance écrit la date, la machine et la charge à côté.

#![forbid(unsafe_code)]

use std::process::ExitCode;
use std::time::{Duration, Instant};

use moteur_wasmtime::{BUDGET_DE_CHARGEMENT, Moteur, RefusModule, ReglagesMoteur};
use partie::Faute;

/// Le garde-fou mural de la mesure : dix minutes, pour que seul le carburant arrête la
/// boucle.
const GARDE_FOU_MS: u32 = 600_000;

/// L'heure de la machine : ce programme mesure des durées, c'est son objet, et aucune
/// règle de jeu n'en dépend.
#[expect(
    clippy::disallowed_methods,
    reason = "la mesure M10 chronomètre des chargements : c'est son objet, et rien d'un jeu n'en dépend"
)]
fn maintenant() -> Instant {
    Instant::now()
}

fn mediane(v: &mut [Duration]) -> Duration {
    v.sort_unstable();
    v.get(v.len() / 2).copied().unwrap_or_default()
}

/// Charge `octets` `n` fois ; rend les durées, et vérifie l'issue de chaque chargement.
fn chronometrer(
    moteur: &Moteur,
    octets: &[u8],
    n: usize,
    attendu: fn(&RefusModule) -> bool,
) -> Result<Vec<Duration>, String> {
    let mut durees = Vec::with_capacity(n);
    for _ in 0..n {
        let debut = maintenant();
        let r = moteur.charger(octets);
        durees.push(debut.elapsed());
        match r {
            Err(e) if attendu(&e) => {}
            Err(e) => return Err(format!("issue inattendue : {e}")),
            Ok(_) => return Err(String::from("le module s'est chargé")),
        }
    }
    Ok(durees)
}

fn main() -> ExitCode {
    let n: usize = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(7);
    let source = moteur_commun::essais::BONJOUR_QUI_BOUCLE;
    let temoin = source.replace("(loop $l (br $l))", "");
    let (Ok(boucle), Ok(temoin)) = (wat::parse_str(source), wat::parse_str(&temoin)) else {
        eprintln!("le module d'essai ne se lit pas");
        return ExitCode::from(70);
    };
    let moteur = match Moteur::new(ReglagesMoteur {
        garde_fou_murale_ms: GARDE_FOU_MS,
        ..ReglagesMoteur::PHASE_1
    }) {
        Ok(m) => m,
        Err(p) => {
            eprintln!("moteur : {p}");
            return ExitCode::from(70);
        }
    };
    let budget = |e: &RefusModule| {
        *e == RefusModule::Faute {
            appel: "Bonjour",
            faute: Faute::Budget,
        }
    };
    let piege = |e: &RefusModule| {
        matches!(
            e,
            RefusModule::Faute {
                appel: "Bonjour",
                faute: Faute::Piege { .. }
            }
        )
    };
    let mesures = chronometrer(&moteur, &temoin, n, piege)
        .and_then(|t| Ok((t, chronometrer(&moteur, &boucle, n, budget)?)));
    let (mut t, mut b) = match mesures {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    println!("BUDGET_DE_CHARGEMENT : {BUDGET_DE_CHARGEMENT} unités ; {n} chargements de chaque");
    println!(
        "  témoin (piège dès l'entrée), µs : {:?}",
        t.iter().map(Duration::as_micros).collect::<Vec<_>>()
    );
    println!(
        "  boucle (faute Budget), ms : {:?}",
        b.iter().map(Duration::as_millis).collect::<Vec<_>>()
    );
    let (mt, mb) = (mediane(&mut t), mediane(&mut b));
    let ps = |d: Duration| {
        d.saturating_sub(mt)
            .as_nanos()
            .saturating_mul(1000)
            .checked_div(u128::from(BUDGET_DE_CHARGEMENT))
            .unwrap_or(0)
    };
    println!(
        "  médianes : témoin {} µs, boucle {} ms ; ps par unité : médiane {}, minimum {}, maximum {}",
        mt.as_micros(),
        mb.as_millis(),
        ps(mb),
        b.first().copied().map_or(0, ps),
        b.last().copied().map_or(0, ps),
    );
    ExitCode::SUCCESS
}
