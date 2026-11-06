//! Les passes de la réécriture (§ 3.6), chacune une fonction pure des octets.
//!
//! Chaque passe se lance seule, sur un module valide : les tests en retirent une à la
//! fois pour voir ce qu'elle garantit. [`crate::reecrire`] les enchaîne. Une passe
//! de code ne valide pas le contrat de § 3.3 : c'est le rôle de [`valider`].

use std::collections::BTreeMap;

use wasm_encoder::{BlockType, ConstExpr, ExportKind, Ieee32, Ieee64, Instruction, ValType};
use wasmparser::Operator;

use crate::analyse::analyser;
use crate::ecriture::{Ajouts, Corps, GlobaleAjoutee, Transformation, reecrire_avec, reprendre};
use crate::{
    CARBURANT, CROISSANCE_BORNEE, CROISSANCE_REFUSEE, Constat, MEMOIRE_MAX, PROFONDEUR,
    PROFONDEUR_ATTEINTE, Refus, nom_de_globale, valider as v,
};

/// Le NaN canonique de `f32` (§ 3.6).
pub const NAN_F32: u32 = 0x7fc0_0000;
/// Le NaN canonique de `f64` (§ 3.6).
pub const NAN_F64: u64 = 0x7ff8_0000_0000_0000;

/// Les octets copiés ou remplis par unité de carburant, en plus du coût de
/// l'instruction : `memory.copy`, `memory.fill` et `memory.init` coûtent
/// `longueur / 64` (§ 3.6).
pub const OCTETS_PAR_CARBURANT: u32 = 64;

/// § 3.1, exactement les exports de § 3.3, un maximum de mémoire, la pile en bas.
pub fn valider(m: &[u8]) -> Result<Constat, Refus> {
    v::examiner(m).map(|e| e.constat)
}

/// Le module avec sa transformation et ses ajouts, depuis une analyse fraîche.
fn passe(
    m: &[u8],
    ajouts: impl FnOnce(&crate::analyse::Analyse<'_>) -> Result<Ajouts, Refus>,
    t: &mut dyn Transformation,
) -> Result<Vec<u8>, Refus> {
    let a = analyser(m)?;
    let ajouts = ajouts(&a)?;
    reecrire_avec(m, &a, &ajouts, t)
}

/// L'indice de la prochaine globale ajoutée.
fn prochaine_globale(a: &crate::analyse::Analyse<'_>, rang: u32) -> Result<u32, Refus> {
    a.nombre_de_globales()?
        .checked_add(rang)
        .ok_or_else(|| Refus::Reecriture {
            quoi: String::from("trop de globales"),
        })
}

// ---------------------------------------------------------------------------
// Globales

struct SansDataDrop;

impl Transformation for SansDataDrop {
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        for (op, _) in instructions {
            if matches!(op, Operator::DataDrop { .. }) {
                corps.pousser(Instruction::Nop);
            } else {
                corps.pousser(reprendre(op)?);
            }
        }
        Ok(())
    }
}

/// Exporte chaque globale mutable d'origine sous `soiree_globale_<n>`, pour copier et
/// restaurer l'état entier.
///
/// `data.drop` y devient `nop` : l'état d'un segment jeté vivrait hors de la mémoire
/// et des globales, et un instantané ne le rendrait pas. Le sens ne change que pour
/// un `memory.init` sur un segment jeté, qui piégeait et copie désormais.
pub fn globales(m: &[u8]) -> Result<Vec<u8>, Refus> {
    passe(
        m,
        |a| {
            let mut ajouts = Ajouts::default();
            for (i, g) in (0u32..).zip(&a.globales) {
                if g.ty.mutable {
                    ajouts
                        .exports
                        .push((nom_de_globale(i), ExportKind::Global, i));
                }
            }
            Ok(ajouts)
        },
        &mut SansDataDrop,
    )
}

// ---------------------------------------------------------------------------
// Carburant

/// Les segments d'une fonction : des suites d'instructions où le contrôle n'entre que
/// par la première, et rendent `(début, coût, tête de boucle)`.
///
/// Un segment commence à l'entrée de la fonction, et juste après chaque instruction
/// où le contrôle peut entrer ou d'où il ne continue pas tout droit : `loop` (la cible
/// d'un `br` vers la boucle), `if` (la branche « alors » n'est pas toujours prise),
/// `else`, `end` (la cible d'un `br` vers un bloc, et la jonction d'un `if`), `br`,
/// `br_table`, `return` et `unreachable` (ce qui les suit ne s'atteint que par un
/// `end` ou un `else`). Aucune autre instruction n'est la cible d'un saut : chaque
/// instruction exécutée a vu le début de son segment, donc son coût, avant elle.
///
/// Le coût est le nombre d'instructions du segment, écrites et non exécutées : un
/// `br_if` pris laisse payées celles qui le suivent, un piège celles d'après lui. Un
/// majorant fixe du travail fait, identique sur toute machine, qui ne compte plus le
/// corps entier d'une grande fonction pour le chemin court qu'elle prend
/// (`conception/BESOINS-DES-JEUX.md`, besoin 13). Une tête de boucle coûte au moins 1,
/// pour qu'un tour coûte toujours.
fn segments(instructions: &[(Operator<'_>, usize)]) -> Vec<(usize, u64, bool)> {
    let mut segments: Vec<(usize, u64, bool)> = vec![(0, 0, false)];
    let dernier = instructions.len().saturating_sub(1);
    for (i, (op, _)) in instructions.iter().enumerate() {
        if let Some(s) = segments.last_mut() {
            s.1 = s.1.saturating_add(1);
        }
        let coupe = matches!(
            op,
            Operator::Loop { .. }
                | Operator::If { .. }
                | Operator::Else
                | Operator::End
                | Operator::Br { .. }
                | Operator::BrTable { .. }
                | Operator::Return
                | Operator::Unreachable
        );
        if coupe && i < dernier {
            segments.push((i.saturating_add(1), 0, matches!(op, Operator::Loop { .. })));
        }
    }
    for s in &mut segments {
        if s.2 {
            s.1 = s.1.max(1);
        }
    }
    segments
}

/// `soiree_carburant -= cout` ; négatif : `unreachable`.
fn bruler(corps: &mut Corps<'_>, g: u32, cout: u64) {
    corps.pousser(Instruction::GlobalGet(g));
    corps.pousser(Instruction::I64Const(
        i64::try_from(cout).unwrap_or(i64::MAX),
    ));
    corps.pousser(Instruction::I64Sub);
    corps.pousser(Instruction::GlobalSet(g));
    verifier_carburant(corps, g);
}

fn verifier_carburant(corps: &mut Corps<'_>, g: u32) {
    corps.pousser(Instruction::GlobalGet(g));
    corps.pousser(Instruction::I64Const(0));
    corps.pousser(Instruction::I64LtS);
    corps.pousser(Instruction::If(BlockType::Empty));
    corps.pousser(Instruction::Unreachable);
    corps.pousser(Instruction::End);
}

struct Carburant {
    globale: u32,
}

impl Transformation for Carburant {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "Operator compte des centaines de variants ; seuls ceux-ci comptent ici"
    )]
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        let segments = segments(&instructions);
        let mut segments = segments.into_iter().peekable();
        let mut longueur = None;
        for (i, (op, _)) in instructions.into_iter().enumerate() {
            if let Some((_, cout, _)) = segments.next_if(|s| s.0 == i)
                && cout > 0
            {
                bruler(corps, self.globale, cout);
            }
            match op {
                Operator::MemoryCopy { .. }
                | Operator::MemoryFill { .. }
                | Operator::MemoryInit { .. } => {
                    // La longueur est au sommet de la pile : on la lit sans la prendre.
                    let n = if let Some(n) = longueur {
                        n
                    } else {
                        let n = corps.local(ValType::I32)?;
                        longueur = Some(n);
                        n
                    };
                    corps.pousser(Instruction::LocalTee(n));
                    corps.pousser(Instruction::GlobalGet(self.globale));
                    corps.pousser(Instruction::LocalGet(n));
                    corps.pousser(Instruction::I64ExtendI32U);
                    corps.pousser(Instruction::I64Const(i64::from(
                        OCTETS_PAR_CARBURANT.trailing_zeros(),
                    )));
                    corps.pousser(Instruction::I64ShrU);
                    corps.pousser(Instruction::I64Sub);
                    corps.pousser(Instruction::GlobalSet(self.globale));
                    verifier_carburant(corps, self.globale);
                    corps.pousser(reprendre(op)?);
                }
                autre => corps.pousser(reprendre(autre)?),
            }
        }
        Ok(())
    }
}

/// Une globale `i64` `soiree_carburant`, décrémentée au début de chaque segment (voir
/// `segments` : l'entrée de la fonction, chaque tête de boucle, chaque branche et
/// chaque jonction) du nombre d'instructions du segment ; `memory.copy`,
/// `memory.fill` et `memory.init` coûtent en plus leur longueur divisée par 64. Négative :
/// `unreachable`. Toujours rendre la main, et une faute de budget se rejoue à
/// l'identique (SQ-59).
pub fn carburant(m: &[u8]) -> Result<Vec<u8>, Refus> {
    let a = analyser(m)?;
    let globale = prochaine_globale(&a, 0)?;
    let ajouts = Ajouts {
        globales: vec![GlobaleAjoutee {
            nom: String::from(CARBURANT),
            ty: ValType::I64,
            initiale: ConstExpr::i64_const(0),
        }],
        exports: Vec::new(),
    };
    reecrire_avec(m, &a, &ajouts, &mut Carburant { globale })
}

// ---------------------------------------------------------------------------
// Profondeur

/// La pile native qu'un étage de `soiree_profondeur` représente, en octets. Le
/// moteur accorde `(profondeur_max + 16)` étages de pile native par appel (§ 8.7).
pub const OCTETS_PAR_ETAGE: u32 = 4096;

/// La pile native comptée pour chaque valeur qu'un cadre peut garder vivante (un
/// paramètre, un local, une place de la pile d'opérandes), en octets : le double de
/// la place d'une valeur `i64` ou `f64` que Cranelift range dans le cadre. Point de
/// départ, non mesuré sur des modules réels.
pub const OCTETS_PAR_VALEUR: u32 = 16;

/// Le nombre d'étages d'un appel à une fonction qui a `valeurs` paramètres et
/// locaux, plus la plus grande hauteur de sa pile d'opérandes : un au moins, et
/// davantage pour un cadre épais. Calculé sur le module seul, il est le même sur
/// tous les moteurs.
pub fn etages(valeurs: u64) -> u32 {
    let octets = valeurs.saturating_mul(u64::from(OCTETS_PAR_VALEUR));
    let n = octets.div_ceil(u64::from(OCTETS_PAR_ETAGE)).max(1);
    // Assez pour piquer au premier appel, et assez peu pour rester un `i32`.
    u32::try_from(n).unwrap_or(u32::MAX).min(0x7fff_ffff)
}

struct Profondeur {
    globale: u32,
    /// La globale de la profondeur atteinte, dans la variante de mesure
    /// ([`profondeur_mesuree`]) seulement.
    atteinte: Option<u32>,
    max: u32,
    /// Les étages de chaque fonction définie, par indice ; une fonction importée
    /// (un module de test) compte comme un `call_indirect`.
    etages: BTreeMap<u32, u32>,
    /// Ceux d'un `call_indirect` : le plus grand de toutes les fonctions.
    indirect: u32,
}

impl Transformation for Profondeur {
    #[expect(
        clippy::wildcard_enum_match_arm,
        reason = "Operator compte des centaines de variants ; seuls les appels comptent ici"
    )]
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        let g = self.globale;
        for (op, _) in instructions {
            let etages = match op {
                Operator::Call { function_index } => Some(
                    self.etages
                        .get(&function_index)
                        .copied()
                        .unwrap_or(self.indirect),
                ),
                Operator::CallIndirect { .. } => Some(self.indirect),
                _ => None,
            };
            if let Some(n) = etages {
                let n = n.cast_signed();
                corps.pousser(Instruction::GlobalGet(g));
                corps.pousser(Instruction::I32Const(n));
                corps.pousser(Instruction::I32Add);
                corps.pousser(Instruction::GlobalSet(g));
                if let Some(a) = self.atteinte {
                    // La variante de mesure : la plus grande profondeur vue.
                    corps.pousser(Instruction::GlobalGet(g));
                    corps.pousser(Instruction::GlobalGet(a));
                    corps.pousser(Instruction::I32GtU);
                    corps.pousser(Instruction::If(BlockType::Empty));
                    corps.pousser(Instruction::GlobalGet(g));
                    corps.pousser(Instruction::GlobalSet(a));
                    corps.pousser(Instruction::End);
                }
                corps.pousser(Instruction::GlobalGet(g));
                corps.pousser(Instruction::I32Const(self.max.cast_signed()));
                corps.pousser(Instruction::I32GtU);
                corps.pousser(Instruction::If(BlockType::Empty));
                corps.pousser(Instruction::Unreachable);
                corps.pousser(Instruction::End);
                corps.pousser(reprendre(op)?);
                corps.pousser(Instruction::GlobalGet(g));
                corps.pousser(Instruction::I32Const(n));
                corps.pousser(Instruction::I32Sub);
                corps.pousser(Instruction::GlobalSet(g));
            } else {
                corps.pousser(reprendre(op)?);
            }
        }
        Ok(())
    }
}

/// Une globale `i32` `soiree_profondeur`, augmentée avant chaque `call` et
/// `call_indirect` des [`etages`] de la fonction appelée et diminuée d'autant au
/// retour ; au-delà de `max` : `unreachable`. Un dépassement de pile au même endroit
/// sur tous les moteurs, et avant la pile native même quand les cadres sont épais :
/// un étage représente [`OCTETS_PAR_ETAGE`] octets, et une fonction compte autant
/// d'étages que ses valeurs vivantes en demandent. Un `call_indirect` compte les
/// étages de la plus épaisse des fonctions.
///
/// Le compte est fait à l'appel, et non à l'entrée de la fonction : chaque sortie
/// (fin, `return`, branche vers l'étiquette de la fonction) se décrémente alors
/// d'elle-même, au retour. L'appel du moteur lui-même ne compte pas : `max` est le
/// nombre d'appels imbriqués sous lui.
pub fn profondeur(m: &[u8], max: u32) -> Result<Vec<u8>, Refus> {
    profondeur_et_mesure(m, max, Variante::Partie)
}

/// La passe de profondeur de la partie, ou sa variante de mesure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Variante {
    /// [`profondeur`] : le module de la partie.
    Partie,
    /// [`profondeur_mesuree`] : la profondeur atteinte, en plus (M9).
    Mesure,
}

/// La variante de mesure de [`profondeur`], **réservée au banc et aux mesures** (M9,
/// PHASE-3, § 2.3) : le même compte, et une seconde globale exportée,
/// [`PROFONDEUR_ATTEINTE`], qui garde le plus grand `soiree_profondeur` vu depuis la
/// création de l'instance. Le moteur ne la pose ni ne la remet jamais à zéro : il la lit
/// après chaque appel. Le carburant est compté avant cette passe : la variante brûle le
/// même carburant et rend les mêmes octets que le module de la partie ; seuls ses octets
/// réécrits, donc son empreinte, diffèrent. Un module réécrit ainsi ne sert jamais à une
/// partie.
pub fn profondeur_mesuree(m: &[u8], max: u32) -> Result<Vec<u8>, Refus> {
    profondeur_et_mesure(m, max, Variante::Mesure)
}

fn profondeur_et_mesure(m: &[u8], max: u32, variante: Variante) -> Result<Vec<u8>, Refus> {
    let a = analyser(m)?;
    let globale = prochaine_globale(&a, 0)?;
    let mut globales = vec![GlobaleAjoutee {
        nom: String::from(PROFONDEUR),
        ty: ValType::I32,
        initiale: ConstExpr::i32_const(0),
    }];
    let atteinte = if variante == Variante::Mesure {
        globales.push(GlobaleAjoutee {
            nom: String::from(PROFONDEUR_ATTEINTE),
            ty: ValType::I32,
            initiale: ConstExpr::i32_const(0),
        });
        Some(prochaine_globale(&a, 1)?)
    } else {
        None
    };
    let ajouts = Ajouts {
        globales,
        exports: Vec::new(),
    };
    let etages: BTreeMap<u32, u32> = v::cadres(m)?
        .into_iter()
        .map(|c| {
            (
                c.indice,
                etages(c.valeurs.saturating_add(u64::from(c.hauteur))),
            )
        })
        .collect();
    let indirect = etages.values().copied().max().unwrap_or(1);
    reecrire_avec(
        m,
        &a,
        &ajouts,
        &mut Profondeur {
            globale,
            atteinte,
            max,
            etages,
            indirect,
        },
    )
}

// ---------------------------------------------------------------------------
// NaN

/// Le type flottant que rend une opération qui peut produire un NaN, et dont la
/// charge utile n'est pas fixée par la norme. `abs`, `neg`, `copysign`, les
/// chargements et les réinterprétations ne touchent que des bits : ils sont exacts.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Operator compte des centaines de variants ; seuls ceux-ci comptent ici"
)]
fn produit_un_nan(op: &Operator<'_>) -> Option<ValType> {
    match op {
        Operator::F32Add
        | Operator::F32Sub
        | Operator::F32Mul
        | Operator::F32Div
        | Operator::F32Sqrt
        | Operator::F32Min
        | Operator::F32Max
        | Operator::F32Ceil
        | Operator::F32Floor
        | Operator::F32Trunc
        | Operator::F32Nearest
        | Operator::F32DemoteF64 => Some(ValType::F32),
        Operator::F64Add
        | Operator::F64Sub
        | Operator::F64Mul
        | Operator::F64Div
        | Operator::F64Sqrt
        | Operator::F64Min
        | Operator::F64Max
        | Operator::F64Ceil
        | Operator::F64Floor
        | Operator::F64Trunc
        | Operator::F64Nearest
        | Operator::F64PromoteF32 => Some(ValType::F64),
        _ => None,
    }
}

/// Les locaux temporaires d'une fonction, un par type flottant.
#[derive(Default)]
struct Temporaires {
    f32: Option<u32>,
    f64: Option<u32>,
}

struct Nan;

impl Transformation for Nan {
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        let mut locaux = Temporaires::default();
        for (op, _) in instructions {
            let ty = produit_un_nan(&op);
            corps.pousser(reprendre(op)?);
            let Some(ty) = ty else {
                continue;
            };
            // x == x ? x : NaN canonique.
            let (place, egal, canon) = if ty == ValType::F32 {
                (
                    &mut locaux.f32,
                    Instruction::F32Eq,
                    Instruction::F32Const(Ieee32::new(NAN_F32)),
                )
            } else {
                (
                    &mut locaux.f64,
                    Instruction::F64Eq,
                    Instruction::F64Const(Ieee64::new(NAN_F64)),
                )
            };
            let t = if let Some(t) = place {
                *t
            } else {
                let t = corps.local(ty)?;
                *place = Some(t);
                t
            };
            corps.pousser(Instruction::LocalTee(t));
            corps.pousser(canon);
            corps.pousser(Instruction::LocalGet(t));
            corps.pousser(Instruction::LocalGet(t));
            corps.pousser(egal);
            corps.pousser(Instruction::Select);
        }
        Ok(())
    }
}

/// Après chaque opération flottante qui peut produire un NaN, un NaN devient le NaN
/// canonique (`0x7fc0_0000`, `0x7ff8_0000_0000_0000`) : la seule source de
/// non-déterminisme qui reste sans fils ni SIMD (SQ-68).
pub fn nan(m: &[u8]) -> Result<Vec<u8>, Refus> {
    passe(m, |_| Ok(Ajouts::default()), &mut Nan)
}

// ---------------------------------------------------------------------------
// Croissance

struct Croissance {
    max: u32,
    refusee: u32,
    bornee: u32,
}

impl Transformation for Croissance {
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        let mut locaux: Option<(u32, u32)> = None;
        for (op, _) in instructions {
            if !matches!(op, Operator::MemoryGrow { mem: 0 }) {
                corps.pousser(reprendre(op)?);
                continue;
            }
            let (d, r) = if let Some(l) = locaux {
                l
            } else {
                let l = (corps.local(ValType::I32)?, corps.local(ValType::I32)?);
                locaux = Some(l);
                l
            };
            // memory.size + delta <= soiree_memoire_max, en 64 bits : sans débordement.
            corps.pousser(Instruction::LocalSet(d));
            corps.pousser(Instruction::MemorySize(0));
            corps.pousser(Instruction::I64ExtendI32U);
            corps.pousser(Instruction::LocalGet(d));
            corps.pousser(Instruction::I64ExtendI32U);
            corps.pousser(Instruction::I64Add);
            corps.pousser(Instruction::GlobalGet(self.max));
            corps.pousser(Instruction::I64ExtendI32U);
            corps.pousser(Instruction::I64LeU);
            corps.pousser(Instruction::If(BlockType::Result(ValType::I32)));
            corps.pousser(Instruction::LocalGet(d));
            corps.pousser(Instruction::MemoryGrow(0));
            corps.pousser(Instruction::LocalTee(r));
            corps.pousser(Instruction::I32Const(-1));
            corps.pousser(Instruction::I32Eq);
            corps.pousser(Instruction::If(BlockType::Empty));
            corps.pousser(Instruction::I32Const(1));
            corps.pousser(Instruction::GlobalSet(self.refusee));
            corps.pousser(Instruction::End);
            corps.pousser(Instruction::LocalGet(r));
            corps.pousser(Instruction::Else);
            corps.pousser(Instruction::I32Const(1));
            corps.pousser(Instruction::GlobalSet(self.bornee));
            corps.pousser(Instruction::I32Const(-1));
            corps.pousser(Instruction::End);
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Pile

struct Pile {
    /// L'indice de `soiree_boite`.
    boite: u32,
    /// L'indice de `__stack_pointer`.
    pointeur: u32,
    /// L'indice de la fonction que la passe lit maintenant.
    vue: u32,
}

impl Transformation for Pile {
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus> {
        if self.vue == self.boite {
            // sorte == 1 (une écriture) : memory.fill(0, 0, __stack_pointer).
            corps.pousser(Instruction::LocalGet(1));
            corps.pousser(Instruction::I32Const(1));
            corps.pousser(Instruction::I32Eq);
            corps.pousser(Instruction::If(BlockType::Empty));
            corps.pousser(Instruction::I32Const(0));
            corps.pousser(Instruction::I32Const(0));
            corps.pousser(Instruction::GlobalGet(self.pointeur));
            corps.pousser(Instruction::MemoryFill(0));
            corps.pousser(Instruction::End);
        }
        self.vue = self.vue.saturating_add(1);
        for (op, _) in instructions {
            corps.pousser(reprendre(op)?);
        }
        Ok(())
    }
}

/// Au début de `soiree_boite(taille, sorte)`, quand `sorte` vaut 1 (une écriture,
/// § 3.3), la pile entière `[0, __stack_pointer)` est remise à zéro.
///
/// Une valeur bâtie sur la pile puis copiée dans le tas emporte les octets que ses
/// champs ne couvrent pas : bourrage, variant vide, mot jamais écrit. Ces octets
/// viennent de ce que l'appel d'avant a laissé sur la pile. Sans cette passe, une
/// lecture faite entre deux écritures change la mémoire de l'état, et deux
/// exécutions du même journal, l'une avec des lectures et l'autre sans (une reprise,
/// une jumelle), n'ont plus la même empreinte (§ 3.2 ; constaté sur les trois jeux le
/// 2026-11-04, voir `conception/BESOINS-DES-JEUX.md`, besoin 1). Avec elle, la
/// mémoire hors des zones exclues ne dépend que des écritures.
///
/// La passe vient après le carburant : la remise à zéro est le fait du commun, elle
/// ne coûte rien au jeu. Un module sans pile (`pile` vaut `None`) ne change pas.
/// `boite` est l'indice de `soiree_boite`, que [`valider`] a constaté avec son type
/// `(i32, i32) -> i32`.
pub fn pile(m: &[u8], pile: Option<u32>) -> Result<Vec<u8>, Refus> {
    let a = analyser(m)?;
    let Some(pile) = pile else {
        return Ok(m.to_vec());
    };
    let boite = a
        .exports
        .iter()
        .find(|e| e.nom == v::BOITE && e.sorte == wasmparser::ExternalKind::Func)
        .map(|e| e.indice)
        .ok_or(Refus::ExportManquant { nom: v::BOITE })?;
    reecrire_avec(
        m,
        &a,
        &Ajouts::default(),
        &mut Pile {
            boite,
            pointeur: pile,
            vue: 0,
        },
    )
}

/// Chaque `memory.grow` vérifie `memory.size + delta <= soiree_memoire_max`, sinon
/// rend -1 et pose `soiree_croissance_bornee` : la mémoire bornée est une faute du
/// jeu. Un échec du vrai `memory.grow` sous ce plafond pose
/// `soiree_croissance_refusee` : une machine à court de mémoire est une panne de la
/// machine.
///
/// `soiree_memoire_max` vaut d'abord le plus petit de `plafond` et du maximum déclaré
/// par le module : sous elle, seul un refus de la machine fait échouer `memory.grow`.
pub fn croissance(m: &[u8], plafond: u32) -> Result<Vec<u8>, Refus> {
    let a = analyser(m)?;
    let declare = a
        .memoires
        .first()
        .and_then(|(_, mem)| mem.maximum)
        .map_or(u32::MAX, |p| u32::try_from(p).unwrap_or(u32::MAX));
    let initiale = plafond.min(declare);
    let max = prochaine_globale(&a, 0)?;
    let refusee = prochaine_globale(&a, 1)?;
    let bornee = prochaine_globale(&a, 2)?;
    let ajouts = Ajouts {
        globales: vec![
            GlobaleAjoutee {
                nom: String::from(MEMOIRE_MAX),
                ty: ValType::I32,
                initiale: ConstExpr::i32_const(initiale.cast_signed()),
            },
            GlobaleAjoutee {
                nom: String::from(CROISSANCE_REFUSEE),
                ty: ValType::I32,
                initiale: ConstExpr::i32_const(0),
            },
            GlobaleAjoutee {
                nom: String::from(CROISSANCE_BORNEE),
                ty: ValType::I32,
                initiale: ConstExpr::i32_const(0),
            },
        ],
        exports: Vec::new(),
    };
    reecrire_avec(
        m,
        &a,
        &ajouts,
        &mut Croissance {
            max,
            refusee,
            bornee,
        },
    )
}
