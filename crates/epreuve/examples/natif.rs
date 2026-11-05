//! L'épreuve d'un module avec sa copie native : celle de `devine` ou d'un fautif.
//!
//! ```text
//! cargo run -p epreuve --example natif -- <jeu> <module.wasm> [options de soiree-epreuve]
//! ```
//!
//! `<jeu>` est `devine`, ou le nom d'une crate de `exemples/fautifs/modules/`
//! (`sain`, `jamais-fini`, …). Un module seul : l'étape 1 n'est pas faite, et le code
//! de sortie n'est jamais 0. L'épreuve qui fait foi part des sources
//! (`soiree-epreuve <dossier>`), qui compile elle-même cette copie native ; cet exemple
//! éprouve un module déjà compilé, sans rien recompiler.

#![forbid(unsafe_code)]

use std::process::ExitCode;

use epreuve::FabriqueDynamique;
use fautifs::jeux;
use fautifs_statiques::jeux as statiques;
use jeu_natif::FabriqueNative;

fn copie_native(nom: &str) -> Option<Box<dyn FabriqueDynamique>> {
    Some(match nom {
        "devine" => Box::new(FabriqueNative::<devine::Devine>::new()),
        "sain" => Box::new(FabriqueNative::<jeux::Sain>::new()),
        "declaration-incoherente" => {
            Box::new(FabriqueNative::<jeux::DeclarationIncoherente>::new())
        }
        "jamais-fini" => Box::new(FabriqueNative::<jeux::JamaisFini>::new()),
        "phase-inaccessible" => Box::new(FabriqueNative::<jeux::PhaseInaccessible>::new()),
        "invariant-viole" => Box::new(FabriqueNative::<jeux::InvariantViole>::new()),
        "attend-un-absent" => Box::new(FabriqueNative::<jeux::AttendUnAbsent>::new()),
        "refuse-apres-avoir-ecrit" => {
            Box::new(FabriqueNative::<statiques::RefuseApresAvoirEcrit>::new())
        }
        "vue-qui-ecrit" => Box::new(FabriqueNative::<statiques::VueQuiEcrit>::new()),
        "vue-qui-fuit" => Box::new(FabriqueNative::<jeux::VueQuiFuit>::new()),
        "natif-different" => Box::new(FabriqueNative::<jeux::NatifDifferent>::new()),
        "octet-qui-panique" => Box::new(FabriqueNative::<jeux::OctetQuiPanique>::new()),
        "etat-hors-enveloppe" => Box::new(FabriqueNative::<statiques::EtatHorsEnveloppe>::new()),
        "fuit-la-memoire" => Box::new(FabriqueNative::<jeux::FuitLaMemoire>::new()),
        "depend-de-l-appareil" => Box::new(FabriqueNative::<jeux::DependDeLAppareil>::new()),
        "coupure-qui-compte" => Box::new(FabriqueNative::<jeux::CoupureQuiCompte>::new()),
        "avec-fenetre" => Box::new(FabriqueNative::<jeux::AvecFenetre>::new()),
        "compte-le-recu" => Box::new(FabriqueNative::<jeux::CompteLeRecu>::new()),
        "pire-cas-qui-deborde" => Box::new(FabriqueNative::<jeux::PireCasQuiDeborde>::new()),
        "nan-dans-l-etat" => Box::new(FabriqueNative::<jeux::NanDansLEtat>::new()),
        "gel-qui-enchaine" => Box::new(FabriqueNative::<jeux::GelQuiEnchaine>::new()),
        "boucle-au-vote" => Box::new(FabriqueNative::<jeux::BoucleAuVote>::new()),
        "phase-sans-cas" => Box::new(FabriqueNative::<jeux::PhaseSansCas>::new()),
        "invariant-jamais-evalue" => Box::new(FabriqueNative::<jeux::InvariantJamaisEvalue>::new()),
        "retient-un-refus" => Box::new(FabriqueNative::<statiques::RetientUnRefus>::new()),
        "choix-qui-fuit" => Box::new(FabriqueNative::<jeux::ChoixQuiFuit>::new()),
        "secret-dans-un-static" => Box::new(FabriqueNative::<statiques::SecretDansUnStatic>::new()),
        "pire-cas-trop-leger" => Box::new(FabriqueNative::<jeux::PireCasTropLeger>::new()),
        "panique-au-temps" => Box::new(FabriqueNative::<jeux::PaniqueAuTemps>::new()),
        "juge-sur-le-fait" => Box::new(FabriqueNative::<jeux::JugeSurLeFait>::new()),
        "fichier-qui-panique" => Box::new(FabriqueNative::<jeux::FichierQuiPanique>::new()),
        "pire-cas-trop-lourd" => Box::new(FabriqueNative::<jeux::PireCasTropLourd>::new()),
        "fin-trop-lourde" => Box::new(FabriqueNative::<jeux::FinTropLourde>::new()),
        "zones-lourdes" => Box::new(FabriqueNative::<jeux::ZonesLourdes>::new()),
        _ => return None,
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(nom) = args.next() else {
        eprintln!("usage : natif <jeu> <module.wasm> [options de soiree-epreuve]");
        return ExitCode::from(64);
    };
    let Some(mut f) = copie_native(&nom) else {
        eprintln!("jeu inconnu : {nom}");
        return ExitCode::from(64);
    };
    ExitCode::from(epreuve::ligne_de_commande(args, Some(f.as_mut())))
}
