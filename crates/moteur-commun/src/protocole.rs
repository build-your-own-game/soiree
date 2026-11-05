//! Le protocole d'un appel (§ 3.4), le classement d'un piège (§ 4.6), l'empreinte de
//! l'état (§ 3.2), et l'instance qui les porte ([`Protocole`]).

use alloc::format;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

use jeu_abi::SorteAppel;
use partie::{CausePanne, Echec, Faute, Instance, PanneOutil};
use reecriture::{
    CARBURANT, CROISSANCE_BORNEE, CROISSANCE_REFUSEE, MEMOIRE_MAX, PROFONDEUR, Reecrit,
    nom_de_globale,
};

use crate::instantane::Instantane;
use crate::interne;
use crate::machine::{Bits, Machine, Piege};

/// Une page de WebAssembly, en octets.
pub(crate) const OCTETS_PAR_PAGE: u64 = 65_536;

/// Les octets lus d'un coup pour l'empreinte et l'instantané : assez pour que la copie
/// ne coûte rien à côté du hachage, assez peu pour ne pas doubler la mémoire.
const TRANCHE: usize = 1 << 16;

/// Une zone de la mémoire : adresse, capacité, en octets.
pub type Zone = (u32, u32);

/// Les zones de § 3.2, lues dans `Bonjour`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Zones {
    /// La boîte, où le moteur écrit l'appel.
    pub boite: Zone,
    /// Le brouillon, où le module écrit son retour.
    pub brouillon: Zone,
    /// La zone de panique, où le gestionnaire de panique écrit son message.
    pub panique: Zone,
}

/// Ce que le protocole sait d'un module réécrit avant son `Bonjour` : son empreinte, sa
/// pile, les noms de ses globales mutables d'origine, ses plafonds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    empreinte: [u8; 32],
    pile_initiale: u32,
    nom_pile: Option<String>,
    noms_globales: Vec<String>,
    profondeur_max: u32,
    memoire_min: u32,
    memoire_max_permise: u32,
}

impl Plan {
    /// Le plan d'un module réécrit sous `reglages`, dont les globales mutables
    /// d'origine ont ces indices ([`crate::globales_d_origine`]), dans l'ordre.
    pub fn new(reecrit: &Reecrit, reglages: &reecriture::Reglages, globales: &[u32]) -> Plan {
        let (min, max) = reecrit.constat.memoire;
        Plan {
            empreinte: reecrit.empreinte,
            pile_initiale: reecrit.pile_initiale,
            nom_pile: reecrit.pile_globale.map(nom_de_globale),
            noms_globales: globales.iter().copied().map(nom_de_globale).collect(),
            profondeur_max: reglages.profondeur_max,
            memoire_min: min,
            memoire_max_permise: max.unwrap_or(u32::MAX).min(reglages.memoire_plafond_pages),
        }
    }

    /// L'empreinte du module réécrit.
    pub fn empreinte(&self) -> [u8; 32] {
        self.empreinte
    }

    /// La valeur initiale de `__stack_pointer` : la pile occupe `[0, pile_initiale)`.
    pub fn pile_initiale(&self) -> u32 {
        self.pile_initiale
    }

    /// La mémoire minimale du module, en pages.
    pub fn memoire_min(&self) -> u32 {
        self.memoire_min
    }

    /// Le plus petit du maximum déclaré par le module et du plafond du moteur, en
    /// pages : la valeur de `soiree_memoire_max` au chargement.
    pub fn memoire_max_permise(&self) -> u32 {
        self.memoire_max_permise
    }
}

/// Ce que toutes les instances d'un module chargé partagent : son plan, ses zones et
/// la valeur de `soiree_memoire_max`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carte {
    plan: Plan,
    zones: Zones,
    memoire_max: u32,
}

impl Carte {
    pub(crate) fn new(plan: Plan, zones: Zones, memoire_max: u32) -> Carte {
        Carte {
            plan,
            zones,
            memoire_max,
        }
    }

    /// Le plan du module.
    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Ses zones.
    pub fn zones(&self) -> Zones {
        self.zones
    }

    /// `soiree_memoire_max` de chaque instance, en pages : `limites.memoire` (§ 8.7).
    pub fn memoire_max(&self) -> u32 {
        self.memoire_max
    }

    /// Les quatre zones que le moteur remet à zéro avant chaque appel, et que
    /// l'empreinte et l'instantané excluent : la pile, la boîte, le brouillon et la zone
    /// de panique. Une seule définition, pour l'appel, l'empreinte, l'instantané et ce
    /// que l'épreuve compte de leur coût ([`Carte::octets_remis_a_zero`]).
    pub fn remises_a_zero(&self) -> [Zone; 4] {
        remises_a_zero(&self.plan, self.zones)
    }

    /// Les octets que le moteur remet à zéro avant chaque appel (§ 3.2). Ce travail
    /// n'est pas compté dans le carburant, qui ne compte que les instructions du
    /// module ; il est le même à chaque appel, et l'épreuve le compte à part dans ce que
    /// juge E19.
    pub fn octets_remis_a_zero(&self) -> u64 {
        self.remises_a_zero()
            .iter()
            .fold(0u64, |a, (_, n)| a.saturating_add(u64::from(*n)))
    }

    /// La mémoire d'une instance de `pages` pages hors des quatre zones : les plages
    /// `(adresse, longueur)` qu'elles laissent, dans l'ordre, sans plage vide. Ce que
    /// l'empreinte lit et ce que l'instantané garde (P3).
    pub fn plages(&self, pages: u64) -> Vec<(u64, u64)> {
        plages(&self.remises_a_zero(), pages)
    }
}

const fn remises_a_zero(plan: &Plan, z: Zones) -> [Zone; 4] {
    [(0, plan.pile_initiale), z.boite, z.brouillon, z.panique]
}

/// Ce que des zones laissent d'une mémoire de `pages` pages.
fn plages(zones: &[Zone; 4], pages: u64) -> Vec<(u64, u64)> {
    let fin = pages.saturating_mul(OCTETS_PAR_PAGE);
    let mut exclues = zones.map(|(a, n)| (u64::from(a), u64::from(a).saturating_add(u64::from(n))));
    exclues.sort_unstable();
    let mut v = Vec::new();
    let mut curseur: u64 = 0;
    for (debut, fin_zone) in exclues.into_iter().chain([(fin, fin)]) {
        let debut = debut.min(fin);
        if debut > curseur {
            v.push((curseur, debut.saturating_sub(curseur)));
        }
        curseur = curseur.max(fin_zone.min(fin));
    }
    v
}

/// Ce que rend un appel : son issue, et le carburant consommé.
pub(crate) struct Rendu {
    pub(crate) issue: Result<Vec<u8>, Echec>,
    pub(crate) carburant: u64,
}

/// La valeur d'une globale entière du moteur.
fn entier<M: Machine>(m: &mut M, nom: &str) -> Result<i64, PanneOutil> {
    m.globale(nom).map(Bits::entier)
}

/// Les globales du moteur, avant chaque appel : le budget, une profondeur nulle, les
/// drapeaux de croissance baissés ; puis les quatre zones que l'empreinte exclut
/// remises à zéro en entier. Elles ne portent ainsi rien d'un appel au suivant : ce
/// qu'une lecture y laisse ne se relit pas (D4, § 3.2), et l'empreinte voit tout ce qui
/// passe d'un appel à l'autre, quelle que soit la taille des zones que le module
/// déclare. `zones` vaut `None` pour le tout premier `Bonjour`, avant qu'on les
/// connaisse.
fn preparer<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
    budget: i64,
) -> Result<(), PanneOutil> {
    m.poser_globale(CARBURANT, Bits::I64(budget))?;
    m.poser_globale(PROFONDEUR, Bits::I32(0))?;
    m.poser_globale(CROISSANCE_REFUSEE, Bits::I32(0))?;
    m.poser_globale(CROISSANCE_BORNEE, Bits::I32(0))?;
    if let Some(z) = zones {
        let memoire = m.pages().saturating_mul(OCTETS_PAR_PAGE);
        for (a, n) in remises_a_zero(plan, z) {
            if u64::from(a).saturating_add(u64::from(n)) > memoire {
                return Err(interne("zone hors de la mémoire"));
            }
            m.remplir_de_zeros(a, n)?;
        }
    }
    Ok(())
}

/// Un appel, selon § 3.4, sur une instance de la machine ; rend aussi le carburant
/// consommé, faute comprise.
pub(crate) fn appeler<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
    sorte: SorteAppel,
    appel: &[u8],
    budget: u64,
) -> Rendu {
    let budget = i64::try_from(budget).unwrap_or(i64::MAX);
    let issue = derouler(m, plan, zones, sorte, appel, budget);
    match entier(m, CARBURANT) {
        Ok(reste) => {
            let consomme = i128::from(budget).saturating_sub(i128::from(reste));
            Rendu {
                issue,
                carburant: u64::try_from(consomme.max(0)).unwrap_or(u64::MAX),
            }
        }
        Err(p) => Rendu {
            issue: Err(Echec::Panne(p)),
            carburant: 0,
        },
    }
}

/// `[a, a + n)` tient-il dans une mémoire de `octets` octets ?
fn dans_la_memoire(a: u32, n: u64, octets: u64) -> bool {
    u64::from(a).saturating_add(n) <= octets
}

fn derouler<M: Machine>(
    m: &mut M,
    plan: &Plan,
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
    preparer(m, plan, zones, budget)?;
    let a = match m.boite(n.cast_signed(), sorte.code()) {
        Ok(a) => a.cast_unsigned(),
        Err(p) => return Err(classer(m, plan, zones, p)),
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
    // La boîte a pu faire grandir la mémoire : sa taille se lit maintenant.
    if !dans_la_memoire(a, u64::from(n), m.pages().saturating_mul(OCTETS_PAR_PAGE)) {
        return Err(Echec::Faute(Faute::HorsMemoire));
    }
    m.ecrire(a, appel)?;
    let r = match m.appel(n.cast_signed()) {
        Ok(r) => r.cast_unsigned(),
        Err(p) => return Err(classer(m, plan, zones, p)),
    };
    if entier(m, CROISSANCE_REFUSEE)? != 0 {
        return Err(Echec::Panne(croissance_refusee()));
    }
    verifier_pile(m, plan)?;
    let adresse = u32::try_from(r >> 32).unwrap_or(u32::MAX);
    let longueur = u32::try_from(r & 0xffff_ffff).unwrap_or(u32::MAX);
    if let Some(z) = zones
        && !dedans((adresse, longueur), z.brouillon)
    {
        return Err(Echec::Faute(Faute::SortieInvalide));
    }
    if !dans_la_memoire(
        adresse,
        u64::from(longueur),
        m.pages().saturating_mul(OCTETS_PAR_PAGE),
    ) {
        return Err(Echec::Faute(Faute::SortieInvalide));
    }
    let taille = usize::try_from(longueur).map_err(|_| Echec::Faute(Faute::SortieInvalide))?;
    let mut retour = vec![0u8; taille];
    m.lire(adresse, &mut retour)?;
    Ok(retour)
}

/// Entre deux appels, `__stack_pointer` a retrouvé sa valeur initiale et
/// `soiree_profondeur` vaut 0 (§ 3.2). Sinon : panne de l'outil.
fn verifier_pile<M: Machine>(m: &mut M, plan: &Plan) -> Result<(), Echec> {
    let profondeur = entier(m, PROFONDEUR)?;
    let pile = match &plan.nom_pile {
        Some(nom) => Some(entier(m, nom)?),
        None => None,
    };
    let attendue = i64::from(plan.pile_initiale.cast_signed());
    if profondeur != 0 || pile.is_some_and(|p| p != attendue) {
        return Err(Echec::Panne(PanneOutil::new(
            CausePanne::PileNonRendue,
            format!("après l'appel : profondeur {profondeur}, pile {pile:?} au lieu de {attendue}"),
        )));
    }
    Ok(())
}

/// Un piège se classe par les globales et la zone de panique (§ 3.4, § 4.6), dans
/// l'ordre de la table de la crate.
pub(crate) fn classer<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
    piege: Piege,
) -> Echec {
    classer_ou_panne(m, plan, zones, piege).unwrap_or_else(Echec::Panne)
}

/// Ce qu'un piège du module devient, si ni les globales ni la zone de panique n'en
/// disent davantage.
enum Reste {
    HorsMemoire,
    PileNative,
    Autre(String),
}

fn classer_ou_panne<M: Machine>(
    m: &mut M,
    plan: &Plan,
    zones: Option<Zones>,
    piege: Piege,
) -> Result<Echec, PanneOutil> {
    let reste = match piege {
        Piege::Moteur(quoi) => return Ok(Echec::Panne(interne(quoi))),
        Piege::Interrompu => {
            return Ok(Echec::Panne(PanneOutil::new(
                CausePanne::GardeFouMural,
                "l'appel a dépassé la durée murale du garde-fou",
            )));
        }
        Piege::HorsMemoire => Reste::HorsMemoire,
        Piege::PileNative => Reste::PileNative,
        Piege::Autre(code) => Reste::Autre(code),
    };
    if entier(m, CROISSANCE_REFUSEE)? != 0 {
        return Ok(Echec::Panne(croissance_refusee()));
    }
    if entier(m, CARBURANT)? < 0 {
        return Ok(Echec::Faute(Faute::Budget));
    }
    if entier(m, PROFONDEUR)? > i64::from(plan.profondeur_max) {
        return Ok(Echec::Faute(Faute::Profondeur));
    }
    if entier(m, CROISSANCE_BORNEE)? != 0 {
        return Ok(Echec::Faute(Faute::Memoire));
    }
    if let Some(z) = zones
        && let Some(message) = panique(m, z)?
    {
        return Ok(Echec::Faute(Faute::Panique { message }));
    }
    Ok(match reste {
        Reste::HorsMemoire => Echec::Faute(Faute::HorsMemoire),
        // La profondeur devait piquer avant la pile native (§ 8.7) : un réglage du
        // moteur faux, pas une faute du jeu.
        Reste::PileNative => Echec::Panne(interne(
            "pile native épuisée avant soiree_profondeur : la pile du fil est trop petite",
        )),
        Reste::Autre(code) => Echec::Faute(Faute::Piege { code }),
    })
}

/// Le message de la zone de panique, si le gestionnaire de panique en a écrit un.
fn panique<M: Machine>(m: &mut M, z: Zones) -> Result<Option<String>, PanneOutil> {
    let (debut, capacite) = z.panique;
    if !dans_la_memoire(
        debut,
        u64::from(capacite),
        m.pages().saturating_mul(OCTETS_PAR_PAGE),
    ) {
        return Ok(None);
    }
    let Ok(taille) = usize::try_from(capacite) else {
        return Ok(None);
    };
    let mut zone = vec![0u8; taille];
    m.lire(debut, &mut zone)?;
    Ok(jeu_abi::lire_panique(&zone).map(|o| String::from_utf8_lossy(o).into_owned()))
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

/// Lit `[debut, debut + longueur)` par tranches, et donne chacune à `f`.
fn par_tranches<M: Machine>(
    m: &mut M,
    debut: u64,
    longueur: u64,
    mut f: impl FnMut(&[u8]),
) -> Result<(), PanneOutil> {
    let mut tampon = vec![0u8; TRANCHE];
    let mut curseur = debut;
    let fin = debut.saturating_add(longueur);
    while curseur < fin {
        let n = usize::try_from(fin.saturating_sub(curseur))
            .unwrap_or(TRANCHE)
            .min(TRANCHE);
        let adresse = u32::try_from(curseur).map_err(|_| interne("adresse hors de 4 Gio"))?;
        let t = tampon
            .get_mut(..n)
            .ok_or_else(|| interne("tranche hors du tampon"))?;
        m.lire(adresse, t)?;
        f(t);
        curseur = curseur.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
    Ok(())
}

/// Les globales mutables d'origine, dans l'ordre.
fn globales<M: Machine>(m: &mut M, plan: &Plan) -> Result<Vec<Bits>, PanneOutil> {
    plan.noms_globales.iter().map(|n| m.globale(n)).collect()
}

/// BLAKE3 de la taille de la mémoire, des globales mutables d'origine et de la mémoire
/// hors des quatre zones exclues (§ 3.2).
pub(crate) fn empreinte<M: Machine>(m: &mut M, carte: &Carte) -> Result<[u8; 32], PanneOutil> {
    let mut h = blake3::Hasher::new();
    h.update(b"soiree, empreinte de l'etat, 1");
    let pages = m.pages();
    h.update(&pages.to_le_bytes());
    for b in globales(m, &carte.plan)? {
        let (t, bits) = b.octets();
        h.update(&t);
        h.update(&bits);
    }
    for (debut, longueur) in carte.plages(pages) {
        par_tranches(m, debut, longueur, |t| {
            h.update(t);
        })?;
    }
    Ok(*h.finalize().as_bytes())
}

/// Le module réécrit sur une machine, comme `partie::Instance` : le protocole d'un appel
/// autour des primitives du moteur.
pub struct Protocole<M: Machine> {
    machine: M,
    carte: Arc<Carte>,
    carburant: Option<u64>,
    creees: u64,
}

impl<M: Machine> Protocole<M> {
    /// Une instance d'un module chargé sur une machine neuve : `soiree_memoire_max` posé
    /// à `limites.memoire`.
    pub fn new(mut machine: M, carte: Arc<Carte>) -> Result<Protocole<M>, PanneOutil> {
        poser_memoire_max(&mut machine, carte.memoire_max)?;
        Ok(Protocole {
            machine,
            carte,
            carburant: None,
            creees: 1,
        })
    }

    /// Ce que l'instance sait de son module.
    pub fn carte(&self) -> &Carte {
        &self.carte
    }

    /// Le nombre d'instances de la machine créées pour celle-ci : 1, plus une par
    /// restauration dans une mémoire plus grande.
    pub fn instances_creees(&self) -> u64 {
        self.creees
    }

    /// La taille de la mémoire, en pages.
    pub fn pages(&mut self) -> u64 {
        self.machine.pages()
    }

    /// La valeur de `soiree_memoire_max`, en pages.
    pub fn memoire_max(&mut self) -> Result<i64, PanneOutil> {
        entier(&mut self.machine, MEMOIRE_MAX)
    }

    /// La machine, pour un moteur qui la règle entre deux appels.
    pub fn machine(&mut self) -> &mut M {
        &mut self.machine
    }
}

fn poser_memoire_max<M: Machine>(m: &mut M, pages: u32) -> Result<(), PanneOutil> {
    m.poser_globale(MEMOIRE_MAX, Bits::I32(pages.cast_signed()))
}

impl<M: Machine> Instance for Protocole<M> {
    type Instantane = Instantane;

    fn appeler(&mut self, sorte: SorteAppel, appel: &[u8], budget: u64) -> Result<Vec<u8>, Echec> {
        let rendu = appeler(
            &mut self.machine,
            &self.carte.plan,
            Some(self.carte.zones),
            sorte,
            appel,
            budget,
        );
        self.carburant = Some(rendu.carburant);
        rendu.issue
    }

    fn carburant(&self) -> Option<u64> {
        self.carburant
    }

    /// La mémoire hors des quatre zones, sa taille et les globales mutables d'origine
    /// (P3). Les globales du moteur n'y sont pas : il les pose avant chaque appel.
    fn instantane(&mut self) -> Result<Instantane, PanneOutil> {
        let globales = globales(&mut self.machine, &self.carte.plan)?;
        let pages = self.machine.pages();
        let reste = self.carte.plages(pages);
        let total = reste.iter().fold(0u64, |a, (_, n)| a.saturating_add(*n));
        let mut octets = Vec::with_capacity(usize::try_from(total).unwrap_or(0));
        for (debut, longueur) in &reste {
            par_tranches(&mut self.machine, *debut, *longueur, |t| {
                octets.extend_from_slice(t);
            })?;
        }
        Ok(Instantane::new(
            self.carte.plan.empreinte,
            pages,
            reste,
            octets,
            globales,
        ))
    }

    /// Remet l'état d'un instantané : la mémoire hors des zones, sa taille, les
    /// globales ; les quatre zones remises à zéro. Une mémoire ne rétrécit pas :
    /// restaurer un instantané plus petit se fait dans une instance neuve (§ 8.6).
    fn restaurer(&mut self, i: &Instantane) -> Result<(), PanneOutil> {
        if i.module() != self.carte.plan.empreinte {
            return Err(interne("instantané d'un autre module"));
        }
        if i.plages() != self.carte.plages(i.pages()).as_slice() {
            return Err(interne("instantané d'une autre forme que ce module"));
        }
        if self.machine.pages() > i.pages() {
            self.machine = self.machine.neuve()?;
            poser_memoire_max(&mut self.machine, self.carte.memoire_max)?;
            self.creees = self.creees.saturating_add(1);
        }
        let pages = self.machine.pages();
        if pages < i.pages() {
            self.machine.grandir_jusqu_a(i.pages())?;
        }
        if self.machine.pages() != i.pages() {
            return Err(interne("taille de mémoire différente après croissance"));
        }
        let mut reste = i.octets();
        for (debut, longueur) in i.plages() {
            let n = usize::try_from(*longueur).map_err(|_| interne("plage trop longue"))?;
            let (plage, apres) = reste
                .split_at_checked(n)
                .ok_or_else(|| interne("instantané coupé"))?;
            reste = apres;
            let adresse = u32::try_from(*debut).map_err(|_| interne("adresse hors de 4 Gio"))?;
            self.machine.ecrire(adresse, plage)?;
        }
        for (a, n) in self.carte.remises_a_zero() {
            self.machine.remplir_de_zeros(a, n)?;
        }
        if self.carte.plan.noms_globales.len() != i.globales().len() {
            return Err(interne("instantané d'un autre module"));
        }
        for (nom, b) in self.carte.plan.noms_globales.iter().zip(i.globales()) {
            self.machine.poser_globale(nom, *b)?;
        }
        self.machine.poser_globale(CARBURANT, Bits::I64(0))?;
        self.machine.poser_globale(PROFONDEUR, Bits::I32(0))?;
        self.machine
            .poser_globale(CROISSANCE_REFUSEE, Bits::I32(0))?;
        self.machine.poser_globale(CROISSANCE_BORNEE, Bits::I32(0))
    }

    fn empreinte(&mut self) -> Result<[u8; 32], PanneOutil> {
        empreinte(&mut self.machine, &self.carte)
    }
}

#[cfg(test)]
mod tests {
    use super::{Zone, plages};

    /// Les plages d'une mémoire hors de ses zones : dans l'ordre, sans plage vide, la
    /// fin de la mémoire comprise ; une pile nulle ne laisse rien devant.
    #[test]
    fn les_plages_sont_le_reste_des_zones() {
        let zones: [Zone; 4] = [(0, 100), (300, 50), (100, 100), (500, 12)];
        assert_eq!(
            plages(&zones, 1),
            [(200, 100), (350, 150), (512, 65_536 - 512)]
        );
        let sans_pile: [Zone; 4] = [(0, 0), (10, 10), (20, 10), (65_526, 10)];
        assert_eq!(plages(&sans_pile, 1), [(0, 10), (30, 65_496)]);
    }
}
