//! Ce qui vit sur le fil d'une instance : le magasin de wasmtime, l'instance, ses
//! exports, et le protocole d'un appel (§ 3.4).

use std::sync::Arc;

use jeu_abi::SorteAppel;
use partie::{CausePanne, Echec, Faute, PanneOutil};
use reecriture::{
    CARBURANT, CROISSANCE_BORNEE, CROISSANCE_REFUSEE, MEMOIRE_MAX, PROFONDEUR, nom_de_globale,
};
use wasmtime::{
    Global, Instance, Memory, Module, Store, StoreLimits, StoreLimitsBuilder, Trap, TypedFunc, Val,
};

/// Une zone de la mémoire : adresse, capacité, en octets.
pub(crate) type Zone = (u32, u32);

/// Les zones de § 3.2, lues dans `Bonjour`, et la pile lue par la réécriture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Zones {
    pub(crate) boite: Zone,
    pub(crate) brouillon: Zone,
    pub(crate) panique: Zone,
}

/// Les quatre zones que le moteur remet à zéro avant chaque appel, et que
/// l'empreinte exclut : la pile, la boîte, le brouillon et la zone de panique. Une
/// seule définition, pour `preparer` et pour ce que l'épreuve compte de leur coût
/// ([`crate::ModuleCharge::octets_remis_a_zero`]).
pub(crate) const fn remises_a_zero(base: &Base, z: Zones) -> [Zone; 4] {
    [(0, base.pile_initiale), z.boite, z.brouillon, z.panique]
}

/// Ce que toutes les instances d'un module partagent.
pub(crate) struct Base {
    pub(crate) module: Module,
    pub(crate) empreinte: [u8; 32],
    pub(crate) pile_initiale: u32,
    pub(crate) pile_globale: Option<u32>,
    /// Les indices des globales mutables d'origine, dans l'ordre.
    pub(crate) globales: Vec<u32>,
    pub(crate) profondeur_max: u32,
    pub(crate) garde_fou_tics: u64,
    /// La pile du fil d'une instance, en octets.
    pub(crate) pile_fil: usize,
}

/// La valeur d'une globale, par ses bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bits {
    I32(i32),
    I64(i64),
    F32(u32),
    F64(u64),
}

impl Bits {
    fn de(v: &Val) -> Option<Bits> {
        match v {
            Val::I32(x) => Some(Bits::I32(*x)),
            Val::I64(x) => Some(Bits::I64(*x)),
            Val::F32(x) => Some(Bits::F32(*x)),
            Val::F64(x) => Some(Bits::F64(*x)),
            Val::V128(_)
            | Val::FuncRef(_)
            | Val::ExternRef(_)
            | Val::AnyRef(_)
            | Val::ExnRef(_)
            | Val::ContRef(_) => None,
        }
    }

    fn vers(self) -> Val {
        match self {
            Bits::I32(x) => Val::I32(x),
            Bits::I64(x) => Val::I64(x),
            Bits::F32(x) => Val::F32(x),
            Bits::F64(x) => Val::F64(x),
        }
    }

    /// Une étiquette de type, puis les bits en petit-boutiste : ce que l'empreinte lit.
    fn octets(self) -> ([u8; 1], [u8; 8]) {
        let (t, b) = match self {
            Bits::I32(x) => (0u8, u64::from(x.cast_unsigned())),
            Bits::I64(x) => (1, x.cast_unsigned()),
            Bits::F32(x) => (2, u64::from(x)),
            Bits::F64(x) => (3, x),
        };
        ([t], b.to_le_bytes())
    }
}

/// L'instantané d'une instance (§ 4.5) : la mémoire entière, sa taille et les
/// globales mutables d'origine. Les globales du moteur n'y sont pas : il les pose
/// avant chaque appel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstantaneWasm {
    pub(crate) module: [u8; 32],
    pub(crate) pages: u64,
    pub(crate) memoire: Arc<[u8]>,
    pub(crate) globales: Arc<[Bits]>,
}

impl InstantaneWasm {
    /// La taille de la mémoire de l'instantané, en pages.
    pub fn pages(&self) -> u64 {
        self.pages
    }
}

/// Une page de WebAssembly, en octets.
const OCTETS_PAR_PAGE: u64 = 65_536;

/// L'instantané d'un module se garde (PHASE-3, § 2.4). Sa forme, à lui : les 32 octets
/// de l'empreinte du module réécrit ; le nombre de pages, en `u64` petit-boutiste ; la
/// mémoire entière, ses octets précédés de leur nombre en `u64` (il vaut les pages fois
/// 65 536) ; le nombre de globales mutables d'origine, en `u32`, puis chacune : une
/// étiquette (0 `i32`, 1 `i64`, 2 `f32`, 3 `f64`) et ses bits en `u64`, comme
/// l'empreinte les lit. La relecture refuse un octet de trop ou de moins.
///
/// La mémoire est entière en phase 3 jusqu'au lot 2, qui en retire les quatre zones
/// remises à zéro (`moteur-commun`, P3).
impl partie::persistance::InstantaneGarde for InstantaneWasm {
    fn ecrire(&self, dans: &mut Vec<u8>) {
        dans.extend_from_slice(&self.module);
        dans.extend_from_slice(&self.pages.to_le_bytes());
        let longueur = u64::try_from(self.memoire.len()).unwrap_or(u64::MAX);
        dans.extend_from_slice(&longueur.to_le_bytes());
        dans.extend_from_slice(&self.memoire);
        let n = u32::try_from(self.globales.len()).unwrap_or(u32::MAX);
        dans.extend_from_slice(&n.to_le_bytes());
        for g in self.globales.iter() {
            let (t, b) = g.octets();
            dans.extend_from_slice(&t);
            dans.extend_from_slice(&b);
        }
    }

    fn lire(octets: &[u8]) -> Result<Self, partie::persistance::RefusGarde> {
        use partie::persistance::RefusGarde;
        let refus = |quoi: &str| RefusGarde::Instantane(format!("instantané de module : {quoi}"));
        let mut reste = octets;
        let mut prendre = |n: usize| -> Result<&[u8], RefusGarde> {
            let (t, r) = reste.split_at_checked(n).ok_or_else(|| refus("coupé"))?;
            reste = r;
            Ok(t)
        };
        let module: [u8; 32] = prendre(32)?.try_into().map_err(|_| refus("empreinte"))?;
        let u64_de = |t: &[u8]| {
            t.try_into()
                .map(u64::from_le_bytes)
                .map_err(|_| refus("entier"))
        };
        let pages = u64_de(prendre(8)?)?;
        let longueur = u64_de(prendre(8)?)?;
        if pages.checked_mul(OCTETS_PAR_PAGE) != Some(longueur) {
            return Err(refus("la mémoire n'a pas la taille de ses pages"));
        }
        let longueur = usize::try_from(longueur).map_err(|_| refus("mémoire trop grande"))?;
        let memoire: Arc<[u8]> = Arc::from(prendre(longueur)?);
        let n: [u8; 4] = prendre(4)?.try_into().map_err(|_| refus("entier"))?;
        let n = u32::from_le_bytes(n);
        let mut globales = Vec::new();
        for _ in 0..n {
            let t = prendre(1)?.first().copied().ok_or_else(|| refus("coupé"))?;
            let b = u64_de(prendre(8)?)?;
            // Les bits hauts d'un `i32` ou d'un `f32` sont nuls à l'écriture : un octet
            // non nul vient d'ailleurs.
            let etroit =
                |b: u64| u32::try_from(b).map_err(|_| refus("bits d'une globale de 32 bits"));
            globales.push(match t {
                0 => Bits::I32(etroit(b)?.cast_signed()),
                1 => Bits::I64(b.cast_signed()),
                2 => Bits::F32(etroit(b)?),
                3 => Bits::F64(b),
                _ => return Err(refus("étiquette de globale inconnue")),
            });
        }
        if !reste.is_empty() {
            return Err(refus("octets en trop"));
        }
        Ok(InstantaneWasm {
            module,
            pages,
            memoire,
            globales: Arc::from(globales),
        })
    }
}

pub(crate) struct Donnees {
    limites: StoreLimits,
}

/// Une instance vivante, sur son fil.
pub(crate) struct Interieur {
    store: Store<Donnees>,
    memoire: Memory,
    boite: TypedFunc<(i32, i32), i32>,
    appel: TypedFunc<i32, i64>,
    carburant: Global,
    profondeur: Global,
    memoire_max: Global,
    refusee: Global,
    bornee: Global,
    globales: Vec<Global>,
    pile: Option<Global>,
}

fn interne(quoi: impl Into<String>) -> PanneOutil {
    PanneOutil::new(CausePanne::Interne, quoi)
}

/// Ce que rend un appel : son issue, et le carburant consommé.
pub(crate) struct Rendu {
    pub(crate) issue: Result<Vec<u8>, Echec>,
    pub(crate) carburant: u64,
}

impl Interieur {
    /// Une instance neuve, `soiree_memoire_max` posé.
    pub(crate) fn new(
        base: &Base,
        memoire_max: u32,
        machine: Option<usize>,
    ) -> Result<Interieur, PanneOutil> {
        let mut limites = StoreLimitsBuilder::new();
        if let Some(octets) = machine {
            limites = limites.memory_size(octets);
        }
        let mut store = Store::new(
            base.module.engine(),
            Donnees {
                limites: limites.build(),
            },
        );
        store.limiter(|d| &mut d.limites);
        store.set_epoch_deadline(base.garde_fou_tics);
        let instance = Instance::new(&mut store, &base.module, &[])
            .map_err(|e| PanneOutil::new(CausePanne::Instanciation, e.to_string()))?;
        let globale = |store: &mut Store<Donnees>, nom: &str| {
            instance
                .get_global(&mut *store, nom)
                .ok_or_else(|| interne(format!("export {nom} absent du module réécrit")))
        };
        let carburant = globale(&mut store, CARBURANT)?;
        let profondeur = globale(&mut store, PROFONDEUR)?;
        let max = globale(&mut store, MEMOIRE_MAX)?;
        let refusee = globale(&mut store, CROISSANCE_REFUSEE)?;
        let bornee = globale(&mut store, CROISSANCE_BORNEE)?;
        let mut globales = Vec::with_capacity(base.globales.len());
        for i in &base.globales {
            globales.push(globale(&mut store, &nom_de_globale(*i))?);
        }
        let pile = match base.pile_globale {
            Some(i) => Some(globale(&mut store, &nom_de_globale(i))?),
            None => None,
        };
        let memoire = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| interne("mémoire absente du module réécrit"))?;
        let boite = instance
            .get_typed_func::<(i32, i32), i32>(&mut store, "soiree_boite")
            .map_err(|e| interne(e.to_string()))?;
        let appel = instance
            .get_typed_func::<i32, i64>(&mut store, "soiree_appel")
            .map_err(|e| interne(e.to_string()))?;
        max.set(&mut store, Val::I32(memoire_max.cast_signed()))
            .map_err(|e| interne(e.to_string()))?;
        Ok(Interieur {
            store,
            memoire,
            boite,
            appel,
            carburant,
            profondeur,
            memoire_max: max,
            refusee,
            bornee,
            globales,
            pile,
        })
    }

    pub(crate) fn pages(&self) -> u64 {
        self.memoire.size(&self.store)
    }

    fn entier(&mut self, g: Global) -> i64 {
        match g.get(&mut self.store) {
            Val::I32(x) => i64::from(x),
            Val::I64(x) => x,
            Val::F32(_)
            | Val::F64(_)
            | Val::V128(_)
            | Val::FuncRef(_)
            | Val::ExternRef(_)
            | Val::AnyRef(_)
            | Val::ExnRef(_)
            | Val::ContRef(_) => 0,
        }
    }

    fn poser(&mut self, g: Global, v: Val) -> Result<(), PanneOutil> {
        g.set(&mut self.store, v)
            .map_err(|e| interne(e.to_string()))
    }

    /// Les globales du moteur, avant chaque appel : le budget, une profondeur nulle,
    /// les drapeaux de croissance baissés ; puis les quatre zones que l'empreinte
    /// exclut remises à zéro en entier (la pile, la boîte, le brouillon, la zone de
    /// panique). Elles ne portent ainsi rien d'un appel au suivant : ce qu'une lecture
    /// y laisse ne se relit pas (D4, § 3.2), et l'empreinte voit tout ce qui passe
    /// d'un appel à l'autre, quelle que soit la taille des zones que le module déclare.
    fn preparer(
        &mut self,
        base: &Base,
        zones: Option<Zones>,
        budget: i64,
    ) -> Result<(), PanneOutil> {
        self.poser(self.carburant, Val::I64(budget))?;
        self.poser(self.profondeur, Val::I32(0))?;
        self.poser(self.refusee, Val::I32(0))?;
        self.poser(self.bornee, Val::I32(0))?;
        if let Some(z) = zones {
            let memoire = self.memoire.data_mut(&mut self.store);
            for (a, n) in remises_a_zero(base, z) {
                let debut = usize::try_from(a).map_err(|_| interne("zone hors de la mémoire"))?;
                let fin = debut.saturating_add(usize::try_from(n).unwrap_or(usize::MAX));
                memoire
                    .get_mut(debut..fin)
                    .ok_or_else(|| interne("zone hors de la mémoire"))?
                    .fill(0);
            }
        }
        self.store.set_epoch_deadline(base.garde_fou_tics);
        Ok(())
    }

    /// Un appel, selon § 3.4. `zones` vaut `None` pour le tout premier `Bonjour`,
    /// avant qu'on les connaisse.
    pub(crate) fn appeler(
        &mut self,
        base: &Base,
        zones: Option<Zones>,
        sorte: SorteAppel,
        appel: &[u8],
        budget: u64,
    ) -> Rendu {
        let budget = i64::try_from(budget).unwrap_or(i64::MAX);
        let issue = self.derouler(base, zones, sorte, appel, budget);
        let reste = self.entier(self.carburant);
        let consomme = i128::from(budget).saturating_sub(i128::from(reste));
        Rendu {
            issue,
            carburant: u64::try_from(consomme.max(0)).unwrap_or(u64::MAX),
        }
    }

    fn derouler(
        &mut self,
        base: &Base,
        zones: Option<Zones>,
        sorte: SorteAppel,
        appel: &[u8],
        budget: i64,
    ) -> Result<Vec<u8>, Echec> {
        let refusee = || {
            Echec::Panne(PanneOutil::new(
                CausePanne::BoiteRefusee,
                format!("{} octets refusés par la boîte", appel.len()),
            ))
        };
        let n = u32::try_from(appel.len()).map_err(|_| refusee())?;
        if let Some(z) = zones
            && sorte == SorteAppel::Lecture
            && n > z.boite.1
        {
            return Err(refusee());
        }
        self.preparer(base, zones, budget)?;
        let a = match self
            .boite
            .call(&mut self.store, (n.cast_signed(), sorte.code()))
        {
            Ok(a) => a.cast_unsigned(),
            Err(e) => return Err(self.classer(base, zones, &e)),
        };
        if a == 0 {
            return Err(refusee());
        }
        // Une lecture écrit dans la boîte, et nulle part ailleurs : écrire ailleurs
        // toucherait l'état (D4).
        if let Some(z) = zones
            && sorte == SorteAppel::Lecture
            && !dedans((a, n), z.boite)
        {
            return Err(Echec::Faute(Faute::SortieInvalide));
        }
        let debut = usize::try_from(a).map_err(|_| Echec::Faute(Faute::HorsMemoire))?;
        let fin = debut
            .checked_add(appel.len())
            .ok_or(Echec::Faute(Faute::HorsMemoire))?;
        self.memoire
            .data_mut(&mut self.store)
            .get_mut(debut..fin)
            .ok_or(Echec::Faute(Faute::HorsMemoire))?
            .copy_from_slice(appel);
        let r = match self.appel.call(&mut self.store, n.cast_signed()) {
            Ok(r) => r.cast_unsigned(),
            Err(e) => return Err(self.classer(base, zones, &e)),
        };
        if self.entier(self.refusee) != 0 {
            return Err(Echec::Panne(croissance_refusee()));
        }
        self.verifier_pile(base)?;
        let adresse = u32::try_from(r >> 32).unwrap_or(u32::MAX);
        let longueur = u32::try_from(r & 0xffff_ffff).unwrap_or(u32::MAX);
        if let Some(z) = zones
            && !dedans((adresse, longueur), z.brouillon)
        {
            return Err(Echec::Faute(Faute::SortieInvalide));
        }
        let debut = usize::try_from(adresse).map_err(|_| Echec::Faute(Faute::SortieInvalide))?;
        let fin = debut
            .checked_add(
                usize::try_from(longueur).map_err(|_| Echec::Faute(Faute::SortieInvalide))?,
            )
            .ok_or(Echec::Faute(Faute::SortieInvalide))?;
        self.memoire
            .data(&self.store)
            .get(debut..fin)
            .map(<[u8]>::to_vec)
            .ok_or(Echec::Faute(Faute::SortieInvalide))
    }

    /// Entre deux appels, `__stack_pointer` a retrouvé sa valeur initiale et
    /// `soiree_profondeur` vaut 0 (§ 3.2). Sinon : panne de l'outil.
    fn verifier_pile(&mut self, base: &Base) -> Result<(), Echec> {
        let profondeur = self.entier(self.profondeur);
        let pile = self.pile.map(|g| self.entier(g));
        let attendue = i64::from(base.pile_initiale.cast_signed());
        if profondeur != 0 || pile.is_some_and(|p| p != attendue) {
            return Err(Echec::Panne(PanneOutil::new(
                CausePanne::PileNonRendue,
                format!(
                    "après l'appel : profondeur {profondeur}, pile {pile:?} au lieu de {attendue}"
                ),
            )));
        }
        Ok(())
    }

    /// Un piège se classe par les globales et la zone de panique (§ 3.4, § 4.6).
    fn classer(&mut self, base: &Base, zones: Option<Zones>, e: &wasmtime::Error) -> Echec {
        let Some(piege) = e.downcast_ref::<Trap>().copied() else {
            return Echec::Panne(interne(format!("erreur de wasmtime : {e}")));
        };
        if piege == Trap::Interrupt {
            return Echec::Panne(PanneOutil::new(
                CausePanne::GardeFouMural,
                "l'appel a dépassé la durée murale du garde-fou",
            ));
        }
        if self.entier(self.refusee) != 0 {
            return Echec::Panne(croissance_refusee());
        }
        if self.entier(self.carburant) < 0 {
            return Echec::Faute(Faute::Budget);
        }
        if self.entier(self.profondeur) > i64::from(base.profondeur_max) {
            return Echec::Faute(Faute::Profondeur);
        }
        if self.entier(self.bornee) != 0 {
            return Echec::Faute(Faute::Memoire);
        }
        if let Some(message) = zones.and_then(|z| self.panique(z)) {
            return Echec::Faute(Faute::Panique { message });
        }
        if piege == Trap::MemoryOutOfBounds {
            return Echec::Faute(Faute::HorsMemoire);
        }
        if piege == Trap::StackOverflow {
            // La profondeur devait piquer avant la pile native (§ 8.7) : un réglage du
            // moteur faux, pas une faute du jeu.
            return Echec::Panne(interne(
                "pile native épuisée avant soiree_profondeur : la pile du fil est trop petite",
            ));
        }
        Echec::Faute(Faute::Piege {
            code: piege.to_string(),
        })
    }

    /// Le message de la zone de panique, si le gestionnaire de panique en a écrit un.
    fn panique(&self, z: Zones) -> Option<String> {
        let memoire = self.memoire.data(&self.store);
        let debut = usize::try_from(z.panique.0).ok()?;
        let capacite = usize::try_from(z.panique.1).ok()?;
        let zone = memoire.get(debut..debut.checked_add(capacite)?)?;
        let octets = jeu_abi::lire_panique(zone)?;
        Some(String::from_utf8_lossy(octets).into_owned())
    }

    pub(crate) fn instantane(&mut self, base: &Base) -> Result<InstantaneWasm, PanneOutil> {
        let mut globales = Vec::with_capacity(self.globales.len());
        for g in self.globales.clone() {
            globales.push(
                Bits::de(&g.get(&mut self.store))
                    .ok_or_else(|| interne("globale d'un type inattendu"))?,
            );
        }
        Ok(InstantaneWasm {
            module: base.empreinte,
            pages: self.pages(),
            memoire: Arc::from(self.memoire.data(&self.store)),
            globales: Arc::from(globales),
        })
    }

    /// Remet l'état d'un instantané. La mémoire doit avoir au plus la taille de
    /// l'instantané : l'appelant remplace sinon l'instance par une neuve.
    pub(crate) fn restaurer(&mut self, i: &InstantaneWasm) -> Result<(), PanneOutil> {
        let pages = self.pages();
        if pages > i.pages {
            return Err(interne("restaurer dans une mémoire plus grande"));
        }
        if pages < i.pages {
            self.memoire
                .grow(&mut self.store, i.pages.saturating_sub(pages))
                .map_err(|e| {
                    PanneOutil::new(
                        CausePanne::CroissanceRefusee,
                        format!("restaurer : la machine refuse la mémoire ({e})"),
                    )
                })?;
        }
        let memoire = self.memoire.data_mut(&mut self.store);
        if memoire.len() != i.memoire.len() {
            return Err(interne("taille de mémoire différente après croissance"));
        }
        memoire.copy_from_slice(&i.memoire);
        if self.globales.len() != i.globales.len() {
            return Err(interne("instantané d'un autre module"));
        }
        for (g, v) in self.globales.clone().into_iter().zip(i.globales.iter()) {
            self.poser(g, v.vers())?;
        }
        self.poser(self.carburant, Val::I64(0))?;
        self.poser(self.profondeur, Val::I32(0))?;
        self.poser(self.refusee, Val::I32(0))?;
        self.poser(self.bornee, Val::I32(0))
    }

    /// BLAKE3 de la mémoire hors des quatre zones exclues, de sa taille et des
    /// globales mutables d'origine (§ 3.2).
    pub(crate) fn empreinte(&mut self, base: &Base, zones: Zones) -> Result<[u8; 32], PanneOutil> {
        let mut h = blake3::Hasher::new();
        h.update(b"soiree, empreinte de l'etat, 1");
        h.update(&self.pages().to_le_bytes());
        for g in self.globales.clone() {
            let b = Bits::de(&g.get(&mut self.store))
                .ok_or_else(|| interne("globale d'un type inattendu"))?;
            let (t, bits) = b.octets();
            h.update(&t);
            h.update(&bits);
        }
        let memoire = self.memoire.data(&self.store);
        let mut exclues = [
            (0, base.pile_initiale),
            zones.boite,
            zones.brouillon,
            zones.panique,
        ]
        .map(|(a, n)| (u64::from(a), u64::from(a).saturating_add(u64::from(n))));
        exclues.sort_unstable();
        let mut curseur: u64 = 0;
        let fin = u64::try_from(memoire.len()).unwrap_or(u64::MAX);
        for (debut, fin_zone) in exclues.into_iter().chain([(fin, fin)]) {
            let debut = debut.min(fin);
            if debut > curseur {
                let tranche = usize::try_from(curseur)
                    .ok()
                    .zip(usize::try_from(debut).ok())
                    .and_then(|(a, b)| memoire.get(a..b))
                    .ok_or_else(|| interne("zone hors de la mémoire"))?;
                h.update(tranche);
            }
            curseur = curseur.max(fin_zone.min(fin));
        }
        Ok(*h.finalize().as_bytes())
    }

    /// Pour les tests de l'instance : la valeur de `soiree_memoire_max`.
    pub(crate) fn memoire_max(&mut self) -> i64 {
        self.entier(self.memoire_max)
    }
}

fn croissance_refusee() -> PanneOutil {
    PanneOutil::new(
        CausePanne::CroissanceRefusee,
        "un memory.grow permis a échoué : la machine manque de mémoire",
    )
}

/// `[a, a + n)` est-il dans la zone ?
fn dedans((a, n): Zone, (z, c): Zone) -> bool {
    let fin = u64::from(a).saturating_add(u64::from(n));
    a >= z && fin <= u64::from(z).saturating_add(u64::from(c))
}

#[cfg(test)]
mod tests {
    //! La forme de l'instantané gardé, figée (vérification du lot 1, constat 3) : ses
    //! octets écrits à la main, et une seconde lecture qui ne passe pas par
    //! [`InstantaneGarde::lire`] (SAL-90). Changer la forme fait rougir l'un et l'autre,
    //! et demande de monter `partie::persistance::VERSION_GARDE`.

    use std::sync::Arc;

    use partie::persistance::{InstantaneGarde, RefusGarde, VERSION_GARDE};

    use super::{Bits, InstantaneWasm};

    /// Une page dont le premier et le dernier octet sont marqués, et une globale de
    /// chaque sorte.
    fn instantane_fige() -> InstantaneWasm {
        let mut memoire = vec![0u8; 65_536];
        memoire[0] = 0xaa;
        memoire[65_535] = 0x55;
        InstantaneWasm {
            module: [3; 32],
            pages: 1,
            memoire: Arc::from(memoire),
            globales: Arc::from(vec![
                Bits::I32(-2),
                Bits::I64(-3),
                Bits::F32(0x3f80_0000),
                Bits::F64(0x4000_0000_0000_0000),
            ]),
        }
    }

    /// Les octets de [`instantane_fige`], d'après la forme : l'empreinte, les pages en
    /// `u64` petit-boutiste, la longueur de la mémoire en `u64` (65 536), la mémoire, le
    /// nombre de globales en `u32`, puis chacune : son étiquette, ses bits en `u64`
    /// petit-boutiste (les bits hauts d'un `i32` ou d'un `f32` à zéro).
    fn octets_figes() -> Vec<u8> {
        let mut memoire = vec![0u8; 65_536];
        memoire[0] = 0xaa;
        memoire[65_535] = 0x55;
        [
            &[3u8; 32][..],
            &[1, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 1, 0, 0, 0, 0, 0],
            &memoire,
            &[4, 0, 0, 0],
            &[0, 0xfe, 0xff, 0xff, 0xff, 0, 0, 0, 0],
            &[1, 0xfd, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff],
            &[2, 0, 0, 0x80, 0x3f, 0, 0, 0, 0],
            &[3, 0, 0, 0, 0, 0, 0, 0, 0x40],
        ]
        .concat()
    }

    #[test]
    fn les_octets_de_l_instantane_sont_figes() {
        assert_eq!(
            VERSION_GARDE, 1,
            "les octets de ce test sont ceux de la version 1"
        );
        let mut o = Vec::new();
        instantane_fige().ecrire(&mut o);
        assert!(o == octets_figes(), "l'instantané s'écrit autrement");
        assert!(
            InstantaneWasm::lire(&octets_figes()) == Ok(instantane_fige()),
            "les octets figés se relisent autrement"
        );
    }

    /// SAL-90 : une lecture d'après la forme seule, sans `lire`.
    #[test]
    fn une_seconde_lecture_relit_l_instantane() {
        let attendu = instantane_fige();
        let mut o = Vec::new();
        attendu.ecrire(&mut o);
        let (empreinte, suite) = o.split_at(32);
        assert_eq!(empreinte, attendu.module);
        let (pages, suite) = suite.split_at(8);
        assert_eq!(u64::from_le_bytes(pages.try_into().unwrap()), 1);
        let (longueur, suite) = suite.split_at(8);
        assert_eq!(u64::from_le_bytes(longueur.try_into().unwrap()), 65_536);
        let (memoire, suite) = suite.split_at(65_536);
        assert!(memoire == &*attendu.memoire, "la mémoire");
        let (nombre, mut suite) = suite.split_at(4);
        assert_eq!(u32::from_le_bytes(nombre.try_into().unwrap()), 4);
        let mut globales = Vec::new();
        for _ in 0..4 {
            let (globale, apres) = suite.split_at(9);
            suite = apres;
            let bits = u64::from_le_bytes(globale[1..].try_into().unwrap());
            globales.push(match globale[0] {
                0 => Bits::I32(i32::from_le_bytes(
                    bits.to_le_bytes()[..4].try_into().unwrap(),
                )),
                1 => Bits::I64(i64::from_le_bytes(bits.to_le_bytes())),
                2 => Bits::F32(u32::try_from(bits).unwrap()),
                3 => Bits::F64(bits),
                etiquette => panic!("étiquette {etiquette}"),
            });
        }
        assert!(suite.is_empty(), "des octets en trop");
        assert_eq!(globales, *attendu.globales);
    }

    /// Une relecture stricte : un octet de trop ou de moins, une mémoire qui n'a pas la
    /// taille de ses pages, une étiquette inconnue, des bits hauts dans un `i32`.
    #[test]
    fn un_instantane_abime_est_refuse() {
        // L'étiquette de la première globale, après l'empreinte, les pages, la longueur,
        // la mémoire et le nombre de globales ; puis le cinquième octet de ses bits.
        const ETIQUETTE: usize = 65_588;
        const BIT_HAUT: usize = 65_593;
        let bon = octets_figes();
        let refuse = |o: &[u8]| matches!(InstantaneWasm::lire(o), Err(RefusGarde::Instantane(_)));
        let mut trop = bon.clone();
        trop.push(0);
        assert!(refuse(&trop));
        assert!(refuse(&bon[..bon.len().saturating_sub(1)]));
        let mut pages = bon.clone();
        pages[32] = 2;
        assert!(refuse(&pages));
        let mut etiquette = bon.clone();
        etiquette[ETIQUETTE] = 4;
        assert!(refuse(&etiquette));
        let mut haut = bon.clone();
        haut[BIT_HAUT] = 1;
        assert!(refuse(&haut));
    }
}
