//! E16 : les pires cas (§ 7.2). Pour chaque `Cas`, sur une instance nue :
//! `cas(pire_cas(c)) = c`, les invariants tiennent, et la scène de l'écran est valide
//! (§ 6.1, `partie::regles::valider_scene`). La mesure des débordements du texte
//! viendra avec le rendu (phase 2).
//!
//! La forme canonique de chaque pire cas passe aussi le constat de E17 : aucun
//! flottant non fini.

use jeu_abi::{
    Appel, Cle, ConstatFil, DeclarationFil, RegardFil, Retour, SceneFil, SorteAppel, SorteAsset,
    decoder, encoder,
};
use moteur_wasmtime::{InstanceWasm, ModuleCharge};
use partie::{Echec, Fabrique, Instance};

use crate::soiree::BUDGETS;
use crate::sonde::{Compte, recoller};

/// De combien, en pour cent, une vue de l'écran des soirées peut dépasser la vue du
/// plus lourd des pires cas sans que E16 n'échoue. Le carburant d'une vue dépend aussi
/// de nombres que le jeu ne choisit pas (un pas, une échéance : un varint d'un octet de
/// plus coûte quelques unités) ; `devine` dépasse ainsi son pire cas de 10 unités sur
/// 16 010 au plan court. Un pire cas qui n'est pas construit depuis les limites (dix
/// tuiles au lieu de trois cents) en est loin. Choisi, non mesuré.
pub(crate) const MARGE_DU_PIRE_CAS_POUR_CENT: u64 = 10;

/// Ce que les pires cas ont montré.
#[derive(Debug, Default)]
pub(crate) struct Constat {
    pub(crate) e16: Compte,
    pub(crate) e17: Compte,
    pub(crate) pannes: Vec<String>,
    pub(crate) cas: u32,
    /// E19 : le carburant de la vue de l'écran de chaque pire cas, par indice. Un
    /// pire cas est construit depuis les limites déclarées : la charge que les bots
    /// du jeu choisissent ne le remplace pas (CONTRAT, « Le résultat annoncé par
    /// l'auteur est un indice »).
    pub(crate) carburant_vue: Vec<(u32, u64)>,
}

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(a, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

fn lire(inst: &mut InstanceWasm, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
    Instance::appeler(inst, SorteAppel::Lecture, &encode(a), BUDGETS.lecture)
}

/// Une instance nue, accueillie.
fn nue(module: &ModuleCharge) -> Result<InstanceWasm, String> {
    let mut i = Fabrique::neuve(&mut module.fabrique()).map_err(|p| p.to_string())?;
    for a in [Appel::Bonjour, Appel::Decrire] {
        let _: Vec<u8> = lire(&mut i, &a).map_err(|e| e.to_string())?;
    }
    Ok(i)
}

/// La scène est-elle valide, dans les limites et le repère déclarés (§ 6.1) ? Les
/// images citées hors des assets sont demandées au jeu.
fn valider(inst: &mut InstanceWasm, d: &DeclarationFil, octets: &[u8]) -> Result<(), String> {
    if octets.len() > usize::try_from(d.limites.vue).unwrap_or(usize::MAX) {
        return Err(format!(
            "la scène fait {} octets, au-delà de {}",
            octets.len(),
            d.limites.vue
        ));
    }
    let scene = decoder::<SceneFil>(octets).map_err(|e| format!("la scène ne se lit pas : {e}"))?;
    if scene.effets.len() > usize::from(d.limites.effets) {
        return Err(format!(
            "{} effets, au-delà de {}",
            scene.effets.len(),
            d.limites.effets
        ));
    }
    let polices: Vec<Cle> = d
        .assets
        .iter()
        .filter(|a| a.sorte == SorteAsset::Police)
        .map(|a| a.cle)
        .collect();
    let mut images: Vec<Cle> = d
        .assets
        .iter()
        .filter(|a| a.sorte == SorteAsset::Image)
        .map(|a| a.cle)
        .collect();
    let citees: Vec<u64> = partie::regles::images_citees(&scene)
        .into_iter()
        .filter(|c| !images.contains(c))
        .map(Cle::fil)
        .collect();
    if !citees.is_empty()
        && let Ok(r) = lire(inst, &Appel::Ressources { cles: citees })
        && let Ok(Retour::Ressources(v)) = decoder::<Retour<'_>>(&r)
    {
        images.extend(v.iter().map(|(c, _)| Cle::du_fil(*c)));
    }
    partie::regles::valider_scene(&scene, &d.ecran, &polices, &images).map_err(|e| e.to_string())
}

/// Le cas que l'état dit être le sien.
fn cas_de(inst: &mut InstanceWasm) -> Result<u16, String> {
    let r = lire(
        inst,
        &Appel::Etat {
            connues: Vec::new(),
        },
    )
    .map_err(|e| e.to_string())?;
    if let Ok(Retour::Etat(e)) = decoder::<Retour<'_>>(&r) {
        Ok(e.cas_indice)
    } else {
        Err(String::from("le retour n'est pas un état"))
    }
}

/// Les invariants violés.
fn violes(inst: &mut InstanceWasm) -> Result<Vec<String>, String> {
    let r = lire(inst, &Appel::Verifier).map_err(|e| e.to_string())?;
    if let Ok(Retour::Verif(v)) = decoder::<Retour<'_>>(&r) {
        Ok(v.iter()
            .filter(|(_, k)| *k == ConstatFil::Viole)
            .map(|(n, _)| String::from(*n))
            .collect())
    } else {
        Err(String::from("le retour n'est pas une vérification"))
    }
}

/// La scène de l'écran, encodée.
fn scene(inst: &mut InstanceWasm) -> Result<Vec<u8>, String> {
    let r = lire(
        inst,
        &Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
    )
    .map_err(|e| e.to_string())?;
    if let Ok(Retour::Vues(v)) = decoder::<Retour<'_>>(&r)
        && let Some(vue) = v.first()
    {
        Ok(vue.octets.to_vec())
    } else {
        Err(String::from("le retour n'est pas une vue"))
    }
}

/// Ce qu'un pire cas construit doit montrer : son cas, ses invariants, sa scène, et
/// une forme canonique sans flottant non fini.
fn juger(
    constat: &mut Constat,
    inst: &mut InstanceWasm,
    declaration: &DeclarationFil,
    indice: u32,
) {
    constat.e16.evaluer();
    match cas_de(inst) {
        Ok(n) if u32::from(n) == indice => {}
        Ok(n) => constat
            .e16
            .echouer(format!("le pire cas d'indice {indice} se dit du cas {n}")),
        Err(e) => constat
            .e16
            .echouer(format!("l'état du pire cas d'indice {indice} : {e}")),
    }
    constat.e16.evaluer();
    match violes(inst) {
        Ok(v) if v.is_empty() => {}
        Ok(v) => constat.e16.echouer(format!(
            "le pire cas d'indice {indice} viole {}",
            v.join(", ")
        )),
        Err(e) => constat.e16.echouer(format!(
            "les invariants du pire cas d'indice {indice} : {e}"
        )),
    }
    constat.e16.evaluer();
    let scene = scene(inst);
    if scene.is_ok()
        && let Some(c) = Instance::carburant(inst)
    {
        constat.carburant_vue.push((indice, c));
    }
    match scene.and_then(|o| valider(inst, declaration, &o)) {
        Ok(()) => {}
        Err(e) => constat.e16.echouer(format!(
            "la scène du pire cas d'indice {indice} est invalide : {e}"
        )),
    }
    if let Ok(canonique) = recoller(|morceau| lire(inst, &Appel::Canonique { morceau })) {
        constat.e17.evaluer();
        match jeu_abi::lire_canonique(&canonique) {
            Some(f) if f.non_finis == 0 => {}
            Some(f) => constat.e17.echouer(format!(
                "le pire cas d'indice {indice} a {} flottant(s) non fini(s)",
                f.non_finis
            )),
            None => constat.e17.echouer(format!(
                "la forme canonique du pire cas d'indice {indice} n'a pas ses bords"
            )),
        }
    }
}

/// Les pires cas du module, un par indice de `Cas::TOUTES`, jusqu'au premier indice
/// que le module ne connaît pas.
pub(crate) fn eprouver(module: &ModuleCharge) -> Constat {
    let mut constat = Constat::default();
    let declaration = module.declaration().clone();
    for indice in 0..=u32::from(u16::MAX) {
        let mut inst = match nue(module) {
            Ok(i) => i,
            Err(p) => {
                constat.pannes.push(p);
                return constat;
            }
        };
        let rendu = Instance::appeler(
            &mut inst,
            SorteAppel::Ecriture,
            &encode(&Appel::PireCas { indice }),
            BUDGETS.creation,
        );
        match rendu {
            Ok(r) if matches!(decoder::<Retour<'_>>(&r), Ok(Retour::PireCas { .. })) => {
                constat.cas = constat.cas.saturating_add(1);
                juger(&mut constat, &mut inst, &declaration, indice);
            }
            // L'indice est au-delà de `Cas::TOUTES` : `Retour::Illisible`.
            Ok(_) => break,
            Err(Echec::Faute(f)) => {
                constat.cas = constat.cas.saturating_add(1);
                constat.e16.evaluer();
                constat
                    .e16
                    .echouer(format!("le pire cas d'indice {indice} faute : {f}"));
            }
            Err(Echec::Panne(p)) => {
                constat.pannes.push(p.to_string());
                return constat;
            }
        }
    }
    constat
}
