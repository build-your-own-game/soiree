//! M11 (PHASE-3, § 8.6, lot 5) : le coût d'une case de garde écrite comme le relais
//! l'écrit (`relais::garde::Dossier::ecrire` : l'octet de sorte et les octets dans un
//! fichier temporaire, `File::sync_all`, renommage, dossier synchronisé), sur le disque
//! de la machine.
//!
//! ```sh
//! cargo run --release -p relais --example mesure_garde -- <dossier> [--fois N]
//! ```
//!
//! Pour chaque taille (une case de journal de 100 o et de 4 Kio, 64 Kio, 1 Mio, et le
//! point de Tier list au pire mesuré, 17 625 193 octets, M3), `N` écritures (64 par
//! défaut ; 8 pour le point), en microsecondes : médiane, 99ᵉ centile, maximum. Puis la
//! même chose sans aucune synchronisation, pour la part de `sync_all`. Une mesure se
//! prend sur une machine calme (`target/hote/lourd --calme`).

#![expect(clippy::print_stdout, reason = "un exemple qui rapporte sur la sortie")]

use std::path::PathBuf;
use std::process::ExitCode;

use liaison::{Case, CodeSalle};
use relais::garde::{Dossier, nom_de_numero};
use relais::horloge::maintenant_us;

/// La salle de la mesure.
const SALLE: CodeSalle = CodeSalle([0x11, 0x22, 0x33, 0x44]);

fn quantiles(mut v: Vec<u64>) -> (u64, u64, u64) {
    v.sort_unstable();
    let n = v.len();
    let au = |q: usize| {
        v.get(
            q.saturating_mul(n.saturating_sub(1))
                .checked_div(100)
                .unwrap_or(0),
        )
        .copied()
        .unwrap_or(0)
    };
    (au(50), au(99), v.last().copied().unwrap_or(0))
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dossier) = args.first().map(PathBuf::from) else {
        println!("usage : mesure_garde <dossier> [--fois N]");
        return ExitCode::from(2);
    };
    let fois: usize = match args.get(1).map(String::as_str) {
        Some("--fois") => args.get(2).and_then(|n| n.parse().ok()).unwrap_or(64),
        _ => 64,
    };
    let Ok(garde) = Dossier::ouvrir(&dossier) else {
        return ExitCode::from(2);
    };
    // Où vont les cases de la salle ; la variante sans synchronisation y écrit aussi.
    let cases = dossier.join(SALLE.to_string()).join("cases");
    if std::fs::create_dir_all(&cases).is_err() {
        return ExitCode::from(2);
    }
    println!("M11 : {} ; {fois} écritures par taille", dossier.display());
    for (nom, taille, n) in [
        ("journal 100 o", 100usize, fois),
        ("journal 4 Kio", 4096, fois),
        ("64 Kio", 64 * 1024, fois),
        ("1 Mio", 1024 * 1024, fois),
        ("point de Tier list, 16,8 Mio", 17_625_193, fois.min(8)),
    ] {
        let octets = vec![0xa5u8; taille];
        for synchronise in [true, false] {
            let mut durees = Vec::with_capacity(n);
            for k in 0..n {
                let nom_case = nom_de_numero(k as u64);
                let debut = maintenant_us();
                let r = if synchronise {
                    garde.ecrire(SALLE, Case::Journal(k as u64), &octets)
                } else {
                    std::fs::write(cases.join(format!(".{nom_case}.tmp")), &octets).and_then(|()| {
                        std::fs::rename(
                            cases.join(format!(".{nom_case}.tmp")),
                            cases.join(&nom_case),
                        )
                    })
                };
                let fin = maintenant_us();
                if let Err(e) = r {
                    println!("{nom} : {e}");
                    return ExitCode::from(1);
                }
                durees.push(fin.saturating_sub(debut));
            }
            let (m, c99, max) = quantiles(durees);
            println!(
                "{nom:<30} {} : médiane {m} µs, 99e centile {c99} µs, maximum {max} µs",
                if synchronise {
                    "écrite et synchronisée"
                } else {
                    "sans synchronisation  "
                }
            );
        }
    }
    let _: std::io::Result<()> = std::fs::remove_dir_all(&dossier);
    ExitCode::SUCCESS
}
