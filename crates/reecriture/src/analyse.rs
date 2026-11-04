//! Ce qu'un module contient, lu une fois, section par section : ce que la validation
//! examine et ce dont les passes ont besoin (le nombre de globales, les paramètres
//! de chaque fonction, le maximum de mémoire).

use wasmparser::{
    CompositeInnerType, DataKind, ExternalKind, FunctionBody, GlobalType, KnownCustom, MemoryType,
    Name, Operator, Parser, Payload, TableType, ValType,
};

use crate::Refus;

/// Une signature de fonction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Signature {
    pub(crate) parametres: Vec<ValType>,
    pub(crate) resultats: Vec<ValType>,
}

/// Une globale, et sa valeur initiale quand c'est une constante entière.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Globale {
    pub(crate) ty: GlobalType,
    pub(crate) initiale: Option<i64>,
    pub(crate) position: usize,
}

/// Un export.
#[derive(Debug, Clone)]
pub(crate) struct Export {
    pub(crate) nom: String,
    pub(crate) sorte: ExternalKind,
    pub(crate) indice: u32,
}

/// Un module lu.
#[derive(Debug, Clone, Default)]
pub(crate) struct Analyse<'a> {
    /// Les types, `None` pour un type qui n'est pas une fonction, avec leur position.
    pub(crate) types: Vec<(usize, Option<Signature>)>,
    /// Le type de chaque fonction définie, dans l'ordre.
    pub(crate) fonctions: Vec<u32>,
    pub(crate) globales: Vec<Globale>,
    pub(crate) memoires: Vec<(usize, MemoryType)>,
    pub(crate) tables: Vec<(usize, TableType)>,
    pub(crate) imports: Vec<(String, String)>,
    pub(crate) exports: Vec<Export>,
    pub(crate) depart: bool,
    /// La position de la section des étiquettes d'exception, s'il y en a une.
    pub(crate) etiquettes: Option<usize>,
    /// Pour chaque segment de données actif : sa position et son adresse constante.
    pub(crate) donnees_actives: Vec<(usize, Option<i64>)>,
    /// L'indice de la globale que la section des noms appelle `__stack_pointer`.
    pub(crate) globale_pile: Option<u32>,
    pub(crate) corps: Vec<FunctionBody<'a>>,
}

impl Analyse<'_> {
    /// Le nombre de globales du module.
    pub(crate) fn nombre_de_globales(&self) -> Result<u32, Refus> {
        u32::try_from(self.globales.len()).map_err(|_| Refus::Illisible {
            position: 0,
            quoi: String::from("trop de globales"),
        })
    }

    /// La signature de la fonction définie d'indice `f`.
    pub(crate) fn signature(&self, f: usize) -> Option<&Signature> {
        let t = usize::try_from(*self.fonctions.get(f)?).ok()?;
        self.types.get(t)?.1.as_ref()
    }
}

fn illisible(e: &wasmparser::BinaryReaderError) -> Refus {
    Refus::Illisible {
        position: e.offset(),
        quoi: e.message().to_string(),
    }
}

/// La valeur d'une expression constante `i32.const` ou `i64.const` seule.
fn constante(expr: &wasmparser::ConstExpr<'_>) -> Option<i64> {
    let mut lecteur = expr.get_operators_reader();
    let premier = lecteur.read().ok()?;
    let fin = lecteur.read().ok()?;
    if !matches!(fin, Operator::End) || !lecteur.eof() {
        return None;
    }
    if let Operator::I32Const { value } = premier {
        Some(i64::from(value))
    } else if let Operator::I64Const { value } = premier {
        Some(value)
    } else {
        None
    }
}

/// Lit le module, section par section. Ne valide pas : `wasmparser::Validator` le
/// fait avant.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "Payload est non exhaustif ; toute autre section est refusée nommément"
)]
#[expect(clippy::too_many_lines, reason = "une branche par section, à la suite")]
pub(crate) fn analyser(m: &[u8]) -> Result<Analyse<'_>, Refus> {
    let mut a = Analyse::default();
    for charge in Parser::new(0).parse_all(m) {
        match charge.map_err(|e| illisible(&e))? {
            Payload::Version {
                encoding, range, ..
            } => {
                if encoding != wasmparser::Encoding::Module {
                    return Err(Refus::Illisible {
                        position: range.start,
                        quoi: String::from("un composant, pas un module"),
                    });
                }
            }
            Payload::TypeSection(s) => {
                for groupe in s.into_iter_with_offsets() {
                    let (position, groupe) = groupe.map_err(|e| illisible(&e))?;
                    for sous in groupe.into_types() {
                        let sig = match &sous.composite_type.inner {
                            CompositeInnerType::Func(f) if !sous.composite_type.shared => {
                                Some(Signature {
                                    parametres: f.params().to_vec(),
                                    resultats: f.results().to_vec(),
                                })
                            }
                            _ => None,
                        };
                        a.types.push((position, sig));
                    }
                }
            }
            Payload::ImportSection(s) => {
                for i in s.into_imports() {
                    let i = i.map_err(|e| illisible(&e))?;
                    a.imports.push((i.module.to_string(), i.name.to_string()));
                }
            }
            Payload::FunctionSection(s) => {
                for f in s {
                    a.fonctions.push(f.map_err(|e| illisible(&e))?);
                }
            }
            Payload::TableSection(s) => {
                for t in s.into_iter_with_offsets() {
                    let (position, t) = t.map_err(|e| illisible(&e))?;
                    a.tables.push((position, t.ty));
                }
            }
            Payload::MemorySection(s) => {
                for mem in s.into_iter_with_offsets() {
                    a.memoires.push(mem.map_err(|e| illisible(&e))?);
                }
            }
            Payload::TagSection(s) => {
                a.etiquettes = Some(s.range().start);
            }
            Payload::GlobalSection(s) => {
                for g in s.into_iter_with_offsets() {
                    let (position, g) = g.map_err(|e| illisible(&e))?;
                    a.globales.push(Globale {
                        ty: g.ty,
                        initiale: constante(&g.init_expr),
                        position,
                    });
                }
            }
            Payload::ExportSection(s) => {
                for e in s {
                    let e = e.map_err(|e| illisible(&e))?;
                    a.exports.push(Export {
                        nom: e.name.to_string(),
                        sorte: e.kind,
                        indice: e.index,
                    });
                }
            }
            Payload::StartSection { .. } => a.depart = true,
            Payload::DataSection(s) => {
                for d in s {
                    let d = d.map_err(|e| illisible(&e))?;
                    if let DataKind::Active { offset_expr, .. } = &d.kind {
                        a.donnees_actives
                            .push((d.range.start, constante(offset_expr)));
                    }
                }
            }
            Payload::CodeSectionEntry(corps) => a.corps.push(corps),
            Payload::CustomSection(s) => {
                if let KnownCustom::Name(noms) = s.as_known() {
                    for sous in noms {
                        // Une section des noms abîmée ne change rien au module : on
                        // l'ignore, comme les moteurs.
                        let Ok(Name::Global(carte)) = sous else {
                            continue;
                        };
                        for n in carte.into_iter().flatten() {
                            if n.name == "__stack_pointer" {
                                a.globale_pile = Some(n.index);
                            }
                        }
                    }
                }
            }
            Payload::ElementSection(_)
            | Payload::DataCountSection { .. }
            | Payload::CodeSectionStart { .. }
            | Payload::End(_) => {}
            autre => {
                let position = autre.as_section().map_or(0, |(_, r)| r.start);
                return Err(Refus::Illisible {
                    position,
                    quoi: String::from("section inconnue d'un module"),
                });
            }
        }
    }
    Ok(a)
}
