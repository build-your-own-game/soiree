//! Réécrire un module section par section, avec `wasm_encoder::reencode`.
//!
//! Une passe dit trois choses : les globales qu'elle ajoute, les exports qu'elle
//! ajoute, et ce qu'elle fait de chaque corps de fonction. Tout le reste est recopié
//! tel quel. Les globales et les fonctions ajoutées vont à la fin de leur espace
//! d'indices : aucun indice d'origine ne bouge, et aucune passe n'ajoute de
//! fonction ni de type.

use wasm_encoder::reencode::{Error, Reencode, utils};
use wasm_encoder::{
    CodeSection, ConstExpr, ExportKind, ExportSection, Function, GlobalSection, GlobalType,
    Instruction, Module, SectionId, ValType,
};
use wasmparser::{FunctionBody, Operator, Parser};

use crate::Refus;
use crate::analyse::Analyse;

/// Une globale ajoutée par une passe, exportée sous son nom.
pub(crate) struct GlobaleAjoutee {
    pub(crate) nom: String,
    pub(crate) ty: ValType,
    pub(crate) initiale: ConstExpr,
}

/// Ce qu'une passe ajoute aux sections du module.
#[derive(Default)]
pub(crate) struct Ajouts {
    /// Ajoutées à la fin des globales, et exportées.
    pub(crate) globales: Vec<GlobaleAjoutee>,
    /// D'autres exports : nom, sorte, indice.
    pub(crate) exports: Vec<(String, ExportKind, u32)>,
}

/// Le corps réécrit d'une fonction : ses instructions, et les locaux ajoutés.
pub(crate) struct Corps<'a> {
    prochain: u32,
    ajoutes: Vec<ValType>,
    instructions: Vec<Instruction<'a>>,
}

impl<'a> Corps<'a> {
    /// Un local de plus, de ce type ; rend son indice.
    pub(crate) fn local(&mut self, ty: ValType) -> Result<u32, Refus> {
        let i = self.prochain;
        self.prochain = i
            .checked_add(1)
            .filter(|n| *n <= crate::LOCAUX_MAX.saturating_add(16))
            .ok_or_else(|| Refus::Reecriture {
                quoi: String::from("les passes ajoutent plus de locaux que réservé"),
            })?;
        self.ajoutes.push(ty);
        Ok(i)
    }

    /// Ajoute une instruction.
    pub(crate) fn pousser(&mut self, i: Instruction<'a>) {
        self.instructions.push(i);
    }
}

/// Ce qu'une passe fait du corps de chaque fonction.
pub(crate) trait Transformation {
    /// Écrit dans `corps` le code réécrit de la fonction, depuis ses instructions
    /// d'origine et leur position. `reprendre` convertit une instruction lue en
    /// instruction écrite, sans la changer.
    fn fonction<'a>(
        &mut self,
        instructions: Vec<(Operator<'a>, usize)>,
        corps: &mut Corps<'a>,
    ) -> Result<(), Refus>;
}

/// La conversion d'une instruction lue en instruction écrite, à l'identique.
pub(crate) fn reprendre(op: Operator<'_>) -> Result<Instruction<'_>, Refus> {
    wasm_encoder::reencode::RoundtripReencoder
        .instruction(op)
        .map_err(|e| Refus::Reecriture {
            quoi: e.to_string(),
        })
}

/// Le rang d'une section dans l'ordre que le format impose.
fn rang(s: Option<SectionId>) -> u8 {
    match s {
        Some(SectionId::Custom) => 0,
        Some(SectionId::Type) => 1,
        Some(SectionId::Import) => 2,
        Some(SectionId::Function) => 3,
        Some(SectionId::Table) => 4,
        Some(SectionId::Memory) => 5,
        Some(SectionId::Tag) => 6,
        Some(SectionId::Global) => 7,
        Some(SectionId::Export) => 8,
        Some(SectionId::Start) => 9,
        Some(SectionId::Element) => 10,
        Some(SectionId::DataCount) => 11,
        Some(SectionId::Code) => 12,
        Some(SectionId::Data) => 13,
        None => u8::MAX,
    }
}

struct Reecrivain<'t, 'm> {
    analyse: &'t Analyse<'m>,
    ajouts: &'t Ajouts,
    transformation: &'t mut dyn Transformation,
    /// L'indice de la prochaine fonction définie.
    fonction: usize,
    globales_ecrites: bool,
    exports_ecrits: bool,
}

impl Reecrivain<'_, '_> {
    fn ecrire_globales(&mut self, s: &mut GlobalSection) {
        for g in &self.ajouts.globales {
            s.global(
                GlobalType {
                    val_type: g.ty,
                    mutable: true,
                    shared: false,
                },
                &g.initiale,
            );
        }
        self.globales_ecrites = true;
    }

    fn ecrire_exports(&mut self, s: &mut ExportSection) -> Result<(), Error<Refus>> {
        let premiere = self
            .analyse
            .nombre_de_globales()
            .map_err(Error::UserError)?;
        for (i, g) in (0u32..).zip(&self.ajouts.globales) {
            let indice = premiere.checked_add(i).ok_or_else(|| {
                Error::UserError(Refus::Reecriture {
                    quoi: String::from("trop de globales"),
                })
            })?;
            s.export(&g.nom, ExportKind::Global, indice);
        }
        for (nom, sorte, indice) in &self.ajouts.exports {
            s.export(nom, *sorte, *indice);
        }
        self.exports_ecrits = true;
        Ok(())
    }
}

impl Reencode for Reecrivain<'_, '_> {
    type Error = Refus;

    fn parse_global_section(
        &mut self,
        globales: &mut GlobalSection,
        section: wasmparser::GlobalSectionReader<'_>,
    ) -> Result<(), Error<Refus>> {
        utils::parse_global_section(self, globales, section)?;
        self.ecrire_globales(globales);
        Ok(())
    }

    fn parse_export_section(
        &mut self,
        exports: &mut ExportSection,
        section: wasmparser::ExportSectionReader<'_>,
    ) -> Result<(), Error<Refus>> {
        utils::parse_export_section(self, exports, section)?;
        self.ecrire_exports(exports)
    }

    /// Une section absente qu'il faut écrire : à sa place, entre les deux sections
    /// présentes qui l'encadrent.
    fn intersperse_section_hook(
        &mut self,
        module: &mut Module,
        _apres: Option<SectionId>,
        avant: Option<SectionId>,
    ) -> Result<(), Error<Refus>> {
        let a_ajouter = !self.ajouts.globales.is_empty();
        if a_ajouter && !self.globales_ecrites && rang(avant) > rang(Some(SectionId::Global)) {
            let mut s = GlobalSection::new();
            self.ecrire_globales(&mut s);
            module.section(&s);
        }
        let a_exporter = a_ajouter || !self.ajouts.exports.is_empty();
        if a_exporter && !self.exports_ecrits && rang(avant) > rang(Some(SectionId::Export)) {
            let mut s = ExportSection::new();
            self.ecrire_exports(&mut s)?;
            module.section(&s);
        }
        Ok(())
    }

    fn parse_function_body(
        &mut self,
        code: &mut CodeSection,
        corps: FunctionBody<'_>,
    ) -> Result<(), Error<Refus>> {
        let erreur = |quoi: &str| {
            Error::UserError(Refus::Reecriture {
                quoi: String::from(quoi),
            })
        };
        let parametres = self
            .analyse
            .signature(self.fonction)
            .ok_or_else(|| erreur("fonction sans signature"))?
            .parametres
            .len();
        self.fonction = self
            .fonction
            .checked_add(1)
            .ok_or_else(|| erreur("trop de fonctions"))?;
        let mut locaux = Vec::new();
        let mut prochain = u32::try_from(parametres).map_err(|_| erreur("trop de paramètres"))?;
        for l in corps.get_locals_reader()? {
            let (n, ty) = l?;
            prochain = prochain
                .checked_add(n)
                .ok_or_else(|| erreur("trop de locaux"))?;
            locaux.push((n, self.val_type(ty)?));
        }
        let mut instructions = Vec::new();
        let mut lecteur = corps.get_operators_reader()?;
        while !lecteur.eof() {
            instructions.push(lecteur.read_with_offset()?);
        }
        let mut sortie = Corps {
            prochain,
            ajoutes: Vec::new(),
            instructions: Vec::new(),
        };
        self.transformation
            .fonction(instructions, &mut sortie)
            .map_err(Error::UserError)?;
        locaux.extend(sortie.ajoutes.into_iter().map(|t| (1, t)));
        let mut f = Function::new(locaux);
        for i in &sortie.instructions {
            f.instruction(i);
        }
        code.function(&f);
        Ok(())
    }
}

/// Réécrit `m` : les ajouts de la passe, et sa transformation de chaque corps.
pub(crate) fn reecrire_avec(
    m: &[u8],
    analyse: &Analyse<'_>,
    ajouts: &Ajouts,
    transformation: &mut dyn Transformation,
) -> Result<Vec<u8>, Refus> {
    let mut r = Reecrivain {
        analyse,
        ajouts,
        transformation,
        fonction: 0,
        globales_ecrites: false,
        exports_ecrits: false,
    };
    let mut module = Module::new();
    r.parse_core_module(&mut module, Parser::new(0), m)
        .map_err(|e| match e {
            Error::UserError(refus) => refus,
            Error::ParseError(e) => Refus::Illisible {
                position: e.offset(),
                quoi: e.message().to_string(),
            },
            autre @ (Error::CanonicalizedHeapTypeReference
            | Error::InvalidConstExpr
            | Error::InvalidCodeSectionSize
            | Error::UnexpectedNonCoreModuleSection
            | Error::UnexpectedNonComponentSection
            | Error::UnsupportedCoreTypeInComponent) => Refus::Reecriture {
                quoi: autre.to_string(),
            },
        })?;
    Ok(module.finish())
}
