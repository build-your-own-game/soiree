//! Les polices que le jeu déclare (PHASE-3, P22, lot 8) : lues par `Ressources` sur une
//! instance créée, comme la télé les lira, puis validées par la mise en page. E1 juge
//! chacune (un fichier sfnt, TTF ou OTF, que la mesure sait lire, à la taille déclarée) ;
//! E16 mesure les textes des pires cas dans celles qui passent.

use jeu_abi::{Appel, Cle, Retour, SorteAsset, decoder, encoder};
use moteur_wasmtime::{InstanceWasm, ModuleCharge};
use partie::{Echec, Fabrique, Instance, PanneOutil};

use crate::soiree::{BUDGETS, ouverture};

/// Les polices du jeu, lues.
#[derive(Debug, Clone)]
pub(crate) struct PolicesLues {
    /// Celle du commun, et celles du jeu qui passent, dans l'ordre de la déclaration :
    /// l'ordre où la télé les ajoutera.
    pub(crate) polices: mise_en_page::Polices,
    /// Les polices déclarées, lues ou non.
    pub(crate) declarees: u32,
    /// E1 : chaque police refusée, et pourquoi.
    pub(crate) refus: Vec<String>,
    /// Ce qui a empêché de les lire, hors de la faute du jeu.
    pub(crate) panne: Option<String>,
    /// Chaque police admise : sa clé, sa famille, sa taille.
    pub(crate) admises: Vec<String>,
}

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(a, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

/// Une instance créée comme au début d'une soirée : `Ressources` demande une partie
/// créée (la colle le refuse avant `Creer`).
fn creee(module: &ModuleCharge, graine: [u8; 32]) -> Result<InstanceWasm, Echec> {
    let mut instance = Fabrique::neuve(&mut module.fabrique())?;
    let o = ouverture(graine);
    let mut presents = Vec::new();
    encoder(&Vec::<jeu_abi::Presence>::new(), &mut presents)
        .map_err(|e| Echec::Panne(PanneOutil::new(partie::CausePanne::Interne, e.to_string())))?;
    let ouvrir = [
        Appel::Bonjour,
        Appel::Decrire,
        Appel::Creer {
            graine: o.graine,
            pas: o.pas,
            heure: o.heure,
            salle: o.salle,
            presents: &presents,
            garde: None,
        },
    ];
    for a in &ouvrir {
        let _: Vec<u8> = Instance::appeler(&mut instance, a.sorte(), &encode(a), BUDGETS.creation)?;
    }
    Ok(instance)
}

/// Lit et valide les polices déclarées. Un jeu sans police déclarée n'a que celle du
/// commun, et rien ne se lit.
pub(crate) fn lire(module: &ModuleCharge, graine: [u8; 32]) -> PolicesLues {
    let declarees: Vec<(Cle, u64)> = module
        .declaration()
        .assets
        .iter()
        .filter(|a| a.sorte == SorteAsset::Police)
        .map(|a| (a.cle, a.taille))
        .collect();
    let mut lues = PolicesLues {
        polices: mise_en_page::Polices::du_commun(),
        declarees: u32::try_from(declarees.len()).unwrap_or(u32::MAX),
        refus: Vec::new(),
        panne: None,
        admises: Vec::new(),
    };
    if declarees.is_empty() {
        return lues;
    }
    let mut instance = match creee(module, graine) {
        Ok(i) => i,
        Err(Echec::Faute(f)) => {
            lues.refus.push(format!(
                "les polices ne se lisent pas : une instance neuve faute avant d'être créée : {f}"
            ));
            return lues;
        }
        Err(Echec::Panne(p)) => {
            lues.panne = Some(p.to_string());
            return lues;
        }
    };
    for (cle, taille) in declarees {
        let a = Appel::Ressources {
            cles: vec![cle.fil()],
        };
        let octets = match Instance::appeler(&mut instance, a.sorte(), &encode(&a), BUDGETS.lecture)
        {
            Ok(r) => match decoder::<Retour<'_>>(&r) {
                Ok(Retour::Ressources(v)) => v
                    .iter()
                    .find(|(c, _)| *c == cle.fil())
                    .map(|(_, o)| o.to_vec()),
                Ok(_) | Err(_) => None,
            },
            Err(Echec::Faute(f)) => {
                lues.refus.push(format!(
                    "la police {:016x} ne se lit pas : `Ressources` faute : {f}",
                    cle.fil()
                ));
                continue;
            }
            Err(Echec::Panne(p)) => {
                lues.panne = Some(p.to_string());
                return lues;
            }
        };
        let Some(octets) = octets else {
            lues.refus.push(format!(
                "la police {:016x} est déclarée, mais `Ressources` ne la rend pas",
                cle.fil()
            ));
            continue;
        };
        if u64::try_from(octets.len()).ok() != Some(taille) {
            lues.refus.push(format!(
                "la police {:016x} est déclarée de {taille} octets, et `Ressources` en rend {}",
                cle.fil(),
                octets.len()
            ));
            continue;
        }
        match lues.polices.ajouter(cle, octets) {
            Ok(()) => lues.admises.push(format!(
                "{:016x} (« {} », {taille} octets)",
                cle.fil(),
                lues.polices.famille(cle).unwrap_or("?")
            )),
            Err(r) => lues.refus.push(format!(
                "la police {:016x} ({taille} octets) est refusée : {r}",
                cle.fil()
            )),
        }
    }
    lues
}
