//! Deux parties qui font foi, menées du même pas : l'une sur la copie native de
//! `devine`, l'autre sur son module réécrit dans wasmtime. Chaque arrivée, chaque
//! avancée et chaque regard leur est donné à toutes deux, et ce qu'elles rendent est
//! comparé sur-le-champ.
//!
//! Ce qui se compare, et comment :
//!
//! - **Ce que le cœur rend** ([`Placement`], [`Avancee`], [`Vue`]) : égal, sauf le
//!   carburant d'un rapport de faute, que seul le module compte.
//! - **Le journal** : égal ligne pour ligne, sauf la valeur des empreintes notées
//!   toutes les 256 entrées. Une empreinte d'instance ne se compare qu'entre
//!   instances de la même sorte (`partie::Instance`) : le module hache sa mémoire,
//!   la copie native sa forme canonique.
//! - **L'état** : la forme canonique, secrets compris (`Partie::canonique`), égale
//!   octet pour octet ; et l'empreinte de la copie native, BLAKE3 de cette forme,
//!   égale au BLAKE3 de la forme canonique que rend le module.
//! - **L'état du cœur** (tenus, présences, demandes, compteurs) : égal.

use devine::Devine;
use jeu_abi::{Pas, RegardFil, TexteCourt};
use jeu_natif::FabriqueNative;
use moteur_wasmtime::{FabriqueWasm, ModuleCharge};
use partie::{
    Arrivee, Avancee, Budgets, Faute, Instance, Journal, Ligne, Ouverture, Partie, Placement,
    PointDeReprise, RapportFaute, Vue,
};

use crate::piste::{Piste, Regle, Releve};

/// La partie sur la copie native.
pub type Natif = Partie<Piste<FabriqueNative<Devine>>>;
/// La partie sur le module.
pub type Module = Partie<Piste<FabriqueWasm>>;
/// Un point de reprise de chacune.
pub type Points = (
    PointDeReprise<
        <<Piste<FabriqueNative<Devine>> as partie::Fabrique>::I as Instance>::Instantane,
    >,
    PointDeReprise<<<Piste<FabriqueWasm> as partie::Fabrique>::I as Instance>::Instantane>,
);

/// L'ouverture de la soirée : la même pour les deux parties.
pub fn ouverture() -> Ouverture {
    Ouverture {
        graine: [7; 32],
        pas: 0,
        heure: 1_700_000_000_000,
        salle: TexteCourt::new("SALLE"),
        // Large pour le module : seule une règle d'injection le réduit. La copie native
        // l'ignore.
        budgets: Budgets {
            entree: 1 << 30,
            lecture: 1 << 30,
            creation: 1 << 30,
            canonique: 1 << 30,
        },
        point_tous_les: 600,
    }
}

/// Les regards de la soirée : l'écran et les trois téléphones.
pub const TOUS: [RegardFil; 4] = [
    RegardFil::Ecran,
    RegardFil::Appareil(crate::soiree::A),
    RegardFil::Appareil(crate::soiree::B),
    RegardFil::Appareil(crate::soiree::C),
];

/// Les deux parties.
pub struct Duo {
    pub natif: Natif,
    pub module: Module,
    /// Les messages de panique ne se comparent pas : utile quand le module faute
    /// réellement là où la copie native reçoit une faute injectée.
    pub panique_libre: bool,
    /// Le pas de la dernière vue produite : ce qu'un geste peut dire avoir vu.
    pub vu: u64,
    /// Les fautes rapportées, côté copie native puis côté module.
    pub fautes: (Vec<RapportFaute>, Vec<RapportFaute>),
    /// Les avancées des deux, déjà comparées : celles de la copie native.
    pub avancees: Vec<Avancee>,
    /// Le nombre de comparaisons d'état faites (forme canonique, journal, cœur).
    pub comparaisons: u64,
}

impl Duo {
    /// Ouvre les deux parties, chacune avec ses fautes à injecter.
    pub fn ouvrir(charge: &ModuleCharge, natif: Vec<Regle>, module: Vec<Regle>) -> Duo {
        let n = Partie::ouvrir(
            Piste::new(FabriqueNative::<Devine>::new(), natif),
            ouverture(),
        )
        .expect("la partie native s'ouvre");
        let m = Partie::ouvrir(Piste::new(charge.fabrique(), module), ouverture())
            .expect("la partie du module s'ouvre");
        let mut d = Duo {
            natif: n,
            module: m,
            panique_libre: false,
            vu: 0,
            fautes: (Vec::new(), Vec::new()),
            avancees: Vec::new(),
            comparaisons: 0,
        };
        d.comparer("l'ouverture");
        d
    }

    /// Le dernier pas définitif, le même des deux côtés.
    pub fn definitif(&self) -> u64 {
        assert_eq!(self.natif.definitif(), self.module.definitif());
        self.natif.definitif().numero()
    }

    /// Une arrivée, reçue au début du pas `s`.
    pub fn arriver(&mut self, a: &Arrivee, s: u64) -> Placement {
        let recu = jeu_abi::Instant::de(Pas::du_fil(s));
        let pn = self
            .natif
            .arriver(a.clone(), recu)
            .expect("arriver, copie native");
        let pm = self
            .module
            .arriver(a.clone(), recu)
            .expect("arriver, module");
        assert_eq!(pn, pm, "placement de {a:?}");
        pn
    }

    /// Avance jusqu'au pas `cible`, compare, et regarde s'il s'est passé quelque chose.
    pub fn avancer(&mut self, cible: u64) -> Avancee {
        let avant = self.natif.journal().lignes().len();
        let an = self
            .natif
            .avancer(Pas::du_fil(cible))
            .expect("avancer, copie native");
        let am = self
            .module
            .avancer(Pas::du_fil(cible))
            .expect("avancer, module");
        for r in &am.fautes {
            assert!(r.carburant.is_some(), "le module cite le carburant : {r:?}");
        }
        assert_eq!(
            self.normaliser(an.clone()),
            self.normaliser(am.clone()),
            "avancée jusqu'au pas {cible}"
        );
        self.fautes.0.extend(an.fautes.iter().cloned());
        self.fautes.1.extend(am.fautes.iter().cloned());
        self.avancees.push(an.clone());
        if self.natif.journal().lignes().len() != avant {
            let tous = TOUS;
            let _: Vec<Vue> = self.regarder(&tous);
            self.comparer(&format!("le pas {cible}"));
        }
        an
    }

    /// Les vues de ces regards, des deux côtés.
    pub fn regarder(&mut self, regards: &[RegardFil]) -> Vec<Vue> {
        let vn = self
            .natif
            .regarder(regards)
            .expect("regarder, copie native");
        let vm = self.module.regarder(regards).expect("regarder, module");
        assert!(vn == vm, "les vues diffèrent au pas {}", self.definitif());
        if let Some(v) = vn.first() {
            self.vu = v.pas.numero();
        }
        // Une faute en lecture se rapporte à la prochaine avancée : rien à relever ici.
        vn
    }

    /// Un point de reprise de chacune, maintenant.
    pub fn point(&mut self) -> Points {
        (
            self.natif.point().expect("point, copie native"),
            self.module.point().expect("point, module"),
        )
    }

    /// Ce que les instances de chaque côté ont reçu.
    pub fn releves(&self) -> (Releve, Releve) {
        (
            self.natif.fabrique().releve(),
            self.module.fabrique().releve(),
        )
    }

    /// Compare l'état des deux parties.
    pub fn comparer(&mut self, ou: &str) {
        assert_eq!(
            sans_empreintes(self.natif.journal()),
            sans_empreintes(self.module.journal()),
            "journal après {ou}"
        );
        assert!(
            self.natif.coeur() == self.module.coeur(),
            "état du cœur après {ou}"
        );
        let cn = self
            .natif
            .canonique()
            .expect("forme canonique, copie native");
        let cm = self.module.canonique().expect("forme canonique, module");
        assert!(cn == cm, "forme canonique après {ou}");
        assert_eq!(
            self.natif.empreinte().expect("empreinte, copie native"),
            *blake3::hash(&cm).as_bytes(),
            "l'empreinte native est le BLAKE3 de la forme canonique du module, après {ou}"
        );
        self.comparaisons += 1;
    }

    fn normaliser(&self, mut a: Avancee) -> Avancee {
        for r in &mut a.fautes {
            r.carburant = None;
            if self.panique_libre
                && let Faute::Panique { message } = &mut r.faute
            {
                message.clear();
            }
        }
        a
    }
}

/// Les lignes du journal, la valeur des empreintes notées mise à zéro.
pub fn sans_empreintes(j: &Journal) -> Vec<Ligne> {
    j.lignes()
        .iter()
        .map(|l| match l {
            Ligne::Empreinte { partie, ordre, .. } => Ligne::Empreinte {
                partie: *partie,
                ordre: *ordre,
                empreinte: [0; 32],
            },
            autre => autre.clone(),
        })
        .collect()
}
