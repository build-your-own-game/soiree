//! L'épreuve d'un module (§ 7, § 8.8) : le constat, les soirées, les rejeux, et un
//! résultat par contrôle.
//!
//! | Contrôle | Où il se lit |
//! |---|---|
//! | E1 | le module chargé, son constat, et une instance nue où se mesure le carburant |
//! | E2, E3 | les soirées principales ; les pires cas, pour connaître les phases et les cas |
//! | E4, E6, E7 | la sonde, à chaque entrée et à chaque lecture des soirées principales |
//! | E5, E8 | après chaque pas qui change l'état, sur des copies du module |
//! | E9 | chaque soirée principale rejouée deux fois sur le module, puis entrée par entrée sur une jumelle et la copie native |
//! | E13 | une soirée de téléphones hostiles et d'arrivées absurdes, et des appels absurdes sur une instance nue |
//! | E14 | les redémarrages de l'autorité, les reprises depuis des points tirés, et le même script sur la copie native |
//! | E15 | une soirée longue, où des appareils vont et viennent |
//! | E10, E11, E12 | une soirée sans panne, dont le script se rejoue renuméroté, coupé, retardé (`proprietes.rs`) |
//! | E16 | chaque pire cas, sur une instance nue (`pires_cas.rs`) |
//! | E17 | la forme canonique et la scène des soirées principales, et des pires cas |
//! | E18 | une soirée de gels pendant les échéances (`gel.rs`) |
//! | E19 | le coût (carburant et coût fixe) de chaque appel et de chaque image des soirées principales, de la vue de chaque pire cas, et de chaque page de E21 (`budgets.rs`) |
//! | E21 | les vues parties vers les téléphones de toutes les soirées, chacune appelée en `Page` sur une instance nue, avec des locaux vides puis tirés au hasard (`pages.rs`) |
//!
//! Une panne déclarée qui n'a pas été constatée rend « pas exécuté » chaque contrôle
//! lu dans les soirées principales : il porterait sur un monde qui n'a pas eu lieu
//! (OUT-26). Un contrôle sans évaluation n'est jamais réussi (OUT-07).

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use jeu_abi::{
    Appel, ApplicableFil, IssueFil, Oui, Retour, SorteAppel, SorteOctets, decoder, encoder,
};
use moteur_wasmtime::{InstanceWasm, ModuleCharge, Moteur};
use partie::{Echec, Fabrique, Faute, Instance, IssueJournal, LieuFaute, PanneOutil, RapportFaute};

use crate::budgets::BUDGETS_E19;
use crate::dynamique::{Effacee, FabriqueDynamique};
use crate::plan::{IdControle, Plan};
use crate::rapport::{IssueControle, Rapport, Rejeu, ResultatControle};
use crate::soiree::{BUDGETS, Bilan, Echantillon, Reglage, Soiree, Sorte, ouverture};
use crate::sonde::{Compte, Verifs};
use crate::tirage::Tirage;

/// Les contrôles lus dans les soirées principales : une panne déclarée et non
/// constatée les rend « pas exécuté ».
const DES_SOIREES: [IdControle; 11] = [
    IdControle::E2,
    IdControle::E3,
    IdControle::E4,
    IdControle::E5,
    IdControle::E6,
    IdControle::E7,
    IdControle::E8,
    IdControle::E9,
    IdControle::E14,
    IdControle::E17,
    IdControle::E19,
];

/// Ce qu'un contrôle a accumulé.
#[derive(Debug, Default)]
struct Acc {
    compte: Compte,
    pannes: Vec<String>,
    /// Ce qui était programmé et n'a pas eu lieu : le contrôle n'a pas tourné comme
    /// prévu, et ne compte pas comme réussi (OUT-26).
    manquees: Vec<String>,
    sans_objet: Option<String>,
    notes: Vec<String>,
    rejeu: Option<Rejeu>,
}

impl Acc {
    fn noter(&mut self, n: impl Into<String>) {
        self.notes.push(n.into());
    }
}

/// E18 : la phase après chaque entrée d'une soirée, et ses gels.
type PhasesEtGels = (Vec<crate::sonde::PhaseVue>, Vec<(u64, u64)>);

struct Epreuve<'m, 'n> {
    module: &'m ModuleCharge,
    natif: Option<Rc<RefCell<&'n mut dyn FabriqueDynamique>>>,
    plan: &'m Plan,
    acc: BTreeMap<IdControle, Acc>,
    joue: Vec<String>,
    pannes_manquees: Vec<String>,
    /// E16 : la vue de l'écran seul la plus lourde des soirées principales : la
    /// soirée, le pas, le carburant.
    vue_ecran_max: Option<(u32, u64, u64)>,
    phases_univers: BTreeSet<u16>,
    cas_univers: Option<u16>,
    /// E3 : le nombre de variantes de `Phase` que le jeu déclare (`Proprietes`).
    phases_declarees: Option<u16>,
    /// E4 : les noms que le jeu déclare exiger (`Epreuve::INVARIANTS`).
    invariants_declares: Option<Vec<String>>,
    /// E4 : combien de fois chaque nom a été évalué, sur toutes les soirées principales.
    invariants_vus: BTreeMap<String, u64>,
    /// E4 : le rejeu de la première soirée principale, pour un invariant qu'aucune
    /// soirée n'a évalué.
    premier_rejeu: Option<Rejeu>,
    /// E19 : le carburant de chaque appel des soirées principales, par nom.
    carburant: BTreeMap<&'static str, Vec<u64>>,
    /// E19 : le coût de chaque image des soirées principales : `(fenêtre + 2)` tours
    /// consécutifs, le coût fixe de chaque entrée compris.
    carburant_images: Vec<u64>,
    /// E19 : le coût fixe d'un appel de ce module, en unités de carburant
    /// ([`crate::budgets::BudgetsE19::cout_fixe`]).
    fixe: u64,
    /// E19 : les tours des soirées principales, et ceux qui ont laissé un pas inachevé
    /// (PHASE-3, P7).
    tours: (u64, u64),
    /// Ce que le monde simulé a fait, sur toutes les soirées.
    monde: crate::plan::MondeJoue,
    /// E18 : la phase après chaque entrée, et les gels, de chaque soirée principale.
    phases_des_soirees: Vec<PhasesEtGels>,
    /// E20 : chaque soirée jouée, son ouverture et son journal, quand E20 est demandé.
    soirees_e20: Vec<moteur_commun::releve::SoireeJouee>,
    /// E21 : les vues parties vers les téléphones, sur toutes les soirées.
    vues_e21: crate::pages::VuesVues,
}

fn encode(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    match encoder(a, &mut o) {
        Ok(()) => o,
        Err(_) => Vec::new(),
    }
}

fn graine_de(plan: &Plan, etiquette: &str) -> [u8; 32] {
    let mut t = Tirage::new(&plan.graine, etiquette);
    let mut g = [0u8; 32];
    for morceau in g.as_chunks_mut::<8>().0 {
        *morceau = t.entier().to_le_bytes();
    }
    g
}

fn rejeu_de(b: &Bilan) -> Rejeu {
    Rejeu {
        graine: b.ouverture.graine,
        journal: b.journal.clone(),
        point: None,
        creations: b.obs.borrow().creations.clone(),
        budgets: BUDGETS,
    }
}

impl Epreuve<'_, '_> {
    fn acc(&mut self, c: IdControle) -> &mut Acc {
        self.acc.entry(c).or_default()
    }

    fn demande(&self, c: IdControle) -> bool {
        self.plan.demande(c)
    }

    /// E1 : la réécriture a accepté le module ; aucun import ; exactement les exports
    /// du contrat ; un maximum de mémoire ; la déclaration lue ; le module livré sous
    /// `TAILLE_MODULE_MAX` (PHASE-3, § 2.8, lot 6) ; le carburant des appels mesuré.
    fn constat(&mut self) {
        let c = self.module.constat().clone();
        let d = self.module.declaration().clone();
        let taille = self.module.taille_livree();
        let a = self.acc(IdControle::E1);
        // PHASE-3, § 2.8 : le Worker refuse au chargement un module plus gros ; un
        // module éprouvé ne doit pas l'être le soir même.
        a.compte.evaluer();
        if taille > moteur_wasmtime::TAILLE_MODULE_MAX {
            a.compte.echouer(format!(
                "le module livré fait {taille} octets, au-delà de TAILLE_MODULE_MAX ({} octets) : le Worker de l'écran le refuserait au chargement",
                moteur_wasmtime::TAILLE_MODULE_MAX
            ));
        }
        a.noter(format!(
            "module livré de {taille} octets ; TAILLE_MODULE_MAX, {} octets (4 Mio, choisi)",
            moteur_wasmtime::TAILLE_MODULE_MAX
        ));
        a.compte.evaluer();
        if !c.imports.is_empty() {
            a.compte
                .echouer(format!("le module importe {:?}", c.imports));
        }
        a.compte.evaluer();
        let exports: BTreeSet<&str> = c.exports.iter().map(String::as_str).collect();
        let attendus: BTreeSet<&str> = ["memory", "soiree_appel", "soiree_boite"].into();
        if exports != attendus {
            a.compte
                .echouer(format!("les exports sont {:?}", c.exports));
        }
        a.compte.evaluer();
        if c.memoire.1.is_none() {
            a.compte
                .echouer(String::from("la mémoire n'a pas de maximum"));
        }
        a.compte.evaluer();
        if u32::from(d.places.joueurs_max()) == 0 || d.places.joueurs_min() > d.places.joueurs_max()
        {
            a.compte
                .echouer(String::from("les places sont incohérentes"));
        }
        a.noter(format!(
            "aucun import ; exports {} ; mémoire de {} à {} pages ; {} globale(s) mutable(s) ; {} famille(s) d'instructions",
            c.exports.join(", "),
            c.memoire.0,
            c.memoire.1.map_or_else(|| String::from("?"), |m| m.to_string()),
            c.globales_mutables,
            c.familles.len()
        ));
        let carburant = self.mesurer_le_carburant();
        let a = self.acc(IdControle::E1);
        match carburant {
            Ok(m) => {
                a.compte.evaluer();
                a.noter(format!(
                    "carburant consommé sur une instance neuve (instructions comptées par la réécriture, déterministe) : {}",
                    m.join(", ")
                ));
            }
            Err(Echec::Faute(f)) => {
                a.compte.evaluer();
                a.compte.echouer(format!(
                    "une instance neuve faute avant la première entrée : {f}"
                ));
            }
            Err(Echec::Panne(p)) => a.pannes.push(p.to_string()),
        }
    }

    /// Le carburant de chaque appel d'ouverture, sur une instance neuve.
    fn mesurer_le_carburant(&mut self) -> Result<Vec<String>, Echec> {
        let mut i = Fabrique::neuve(&mut self.module.fabrique())?;
        let o = ouverture(self.plan.graine);
        let mut presents = Vec::new();
        encoder(&Vec::<jeu_abi::Presence>::new(), &mut presents).map_err(|e| {
            Echec::Panne(PanneOutil::new(partie::CausePanne::Interne, e.to_string()))
        })?;
        let appels = [
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
            Appel::Etat {
                connues: Vec::new(),
            },
            Appel::Regarder {
                regards: vec![jeu_abi::RegardFil::Ecran],
            },
            Appel::Canonique { morceau: 0 },
            Appel::Memoire,
        ];
        let mut m = Vec::new();
        for a in &appels {
            let _: Vec<u8> = Instance::appeler(&mut i, a.sorte(), &encode(a), BUDGETS.creation)?;
            m.push(format!(
                "{} {}",
                crate::sonde::nom_appel(a),
                Instance::carburant(&i).unwrap_or(0)
            ));
        }
        Ok(m)
    }

    /// Ce que le jeu déclare de ses phases (E3) et de ses invariants (E4), lu par
    /// `Proprietes` sur une instance nue (ABI 3).
    fn declare(&mut self) {
        match self.lire_proprietes() {
            Ok(p) => {
                self.phases_declarees = Some(p.phases);
                self.invariants_declares = Some(p.invariants);
            }
            Err(e) => {
                for c in [IdControle::E3, IdControle::E4] {
                    if self.demande(c) {
                        self.acc(c).pannes.push(e.clone());
                    }
                }
            }
        }
    }

    /// Les phases et les cas que le jeu connaît, lus sur ses pires cas (E3) : une
    /// instance nue, `PireCas` pour chaque indice jusqu'au refus, puis `Etat`.
    fn univers(&mut self) {
        let mut i: Option<InstanceWasm> = None;
        let mut n = 0u16;
        for indice in 0..=u32::from(u16::MAX) {
            if i.is_none() {
                let mut neuve = match Fabrique::neuve(&mut self.module.fabrique()) {
                    Ok(x) => x,
                    Err(p) => {
                        self.acc(IdControle::E3).pannes.push(p.to_string());
                        return;
                    }
                };
                for a in [Appel::Bonjour, Appel::Decrire] {
                    if Instance::appeler(
                        &mut neuve,
                        SorteAppel::Lecture,
                        &encode(&a),
                        BUDGETS.lecture,
                    )
                    .is_err()
                    {
                        return;
                    }
                }
                i = Some(neuve);
            }
            let Some(inst) = i.as_mut() else {
                return;
            };
            let rendu = Instance::appeler(
                inst,
                SorteAppel::Ecriture,
                &encode(&Appel::PireCas { indice }),
                BUDGETS.creation,
            );
            let Ok(r) = rendu else {
                self.acc(IdControle::E3).noter(format!(
                    "le pire cas d'indice {indice} faute : sa phase n'est pas connue (E16)"
                ));
                i = None;
                n = n.saturating_add(1);
                continue;
            };
            if !matches!(decoder::<Retour<'_>>(&r), Ok(Retour::PireCas { .. })) {
                break;
            }
            n = n.saturating_add(1);
            if let Ok(r) = Instance::appeler(
                inst,
                SorteAppel::Lecture,
                &encode(&Appel::Etat {
                    connues: Vec::new(),
                }),
                BUDGETS.lecture,
            ) && let Ok(Retour::Etat(e)) = decoder::<Retour<'_>>(&r)
            {
                self.phases_univers.insert(e.phase_indice);
            }
        }
        self.cas_univers = Some(n);
    }

    fn reglage(&self, sorte: Sorte, graine: [u8; 32], duree: u64) -> Reglage {
        let p = |c| {
            if self.demande(c) { Oui::Oui } else { Oui::Non }
        };
        match sorte {
            Sorte::Principale => Reglage {
                sorte,
                graine,
                duree,
                monde: self.plan.monde,
                pannes: self.plan.pannes.clone(),
                verifs: Verifs {
                    e4: p(IdControle::E4),
                    e6: p(IdControle::E6),
                    e7: p(IdControle::E7),
                    e8: p(IdControle::E8),
                    releve: Oui::Oui,
                    phases: p(IdControle::E18),
                    budgets: p(IdControle::E19),
                },
                e5: p(IdControle::E5),
                e8: p(IdControle::E8),
                e14: p(IdControle::E14),
            },
            Sorte::Hostile | Sorte::Longue => Reglage {
                sorte,
                graine,
                duree,
                monde: self.plan.monde,
                pannes: Vec::new(),
                verifs: Verifs {
                    releve: Oui::Oui,
                    ..Verifs::RIEN
                },
                e5: Oui::Non,
                e8: Oui::Non,
                e14: Oui::Non,
            },
            Sorte::Proprietes | Sorte::Gel => Reglage {
                sorte,
                graine,
                duree,
                monde: self.plan.monde,
                pannes: Vec::new(),
                verifs: Verifs {
                    phases: if sorte == Sorte::Gel {
                        Oui::Oui
                    } else {
                        Oui::Non
                    },
                    ..Verifs::RIEN
                },
                e5: Oui::Non,
                e8: Oui::Non,
                e14: Oui::Non,
            },
        }
    }

    /// Joue une soirée ; une panique de l'outil devient une panne de l'outil. Quand E20
    /// est demandé, la soirée est gardée pour lui (une soirée rejouée à l'identique, comme
    /// la seconde exécution de E9, ne l'est qu'une fois).
    fn jouer(&mut self, r: Reglage) -> Result<Bilan, String> {
        let module = self.module;
        let sorte = r.sorte;
        match catch_unwind(AssertUnwindSafe(move || {
            Soiree::ouvrir(module, r).map(Soiree::jouer)
        })) {
            Ok(b) => {
                if let Ok(bilan) = &b {
                    self.monde.ajouter(&bilan.monde);
                    self.vues_e21.ajouter(&bilan.vues_telephones);
                    if self.plan.demande(IdControle::E20) {
                        self.garder_pour_e20(sorte, bilan);
                    }
                }
                b
            }
            Err(p) => {
                let m = p
                    .downcast_ref::<&str>()
                    .map(|s| String::from(*s))
                    .or_else(|| p.downcast_ref::<String>().cloned())
                    .unwrap_or_default();
                Err(format!("l'épreuve a paniqué : {m}"))
            }
        }
    }

    /// E20 : garde l'ouverture et le journal d'une soirée jouée, sauf s'ils sont ceux
    /// d'une soirée déjà gardée.
    fn garder_pour_e20(&mut self, sorte: Sorte, bilan: &Bilan) {
        if self
            .soirees_e20
            .iter()
            .any(|s| s.ouverture == bilan.ouverture && s.journal == bilan.journal)
        {
            return;
        }
        let quoi = match sorte {
            Sorte::Principale => "soirée principale",
            Sorte::Hostile => "soirée hostile",
            Sorte::Longue => "soirée longue",
            Sorte::Proprietes => "soirée des propriétés",
            Sorte::Gel => "soirée de gels",
        };
        let n = self.soirees_e20.len();
        self.soirees_e20.push(moteur_commun::releve::SoireeJouee {
            nom: format!("{quoi} (la {}ᵉ gardée)", n.saturating_add(1)),
            ouverture: bilan.ouverture.clone(),
            journal: bilan.journal.clone(),
        });
    }

    fn panne_des_soirees(&mut self, quoi: &str, controles: &[IdControle]) {
        for c in controles {
            self.acc(*c).pannes.push(String::from(quoi));
        }
    }

    /// Les soirées principales, et ce qu'elles disent à E2 à E9 et E14.
    fn soirees(&mut self) {
        let mut faits_terminales = 0usize;
        let mut phases = BTreeSet::new();
        let mut cas = BTreeSet::new();
        for k in 0..self.plan.soirees {
            let graine = graine_de(self.plan, &format!("soirée principale {k}"));
            let r = self.reglage(Sorte::Principale, graine, self.plan.duree.en_pas());
            let bilan = match self.jouer(r.clone()) {
                Ok(b) => b,
                Err(e) => {
                    self.panne_des_soirees(&e, &DES_SOIREES);
                    continue;
                }
            };
            self.joue.push(format!(
                "soirée principale {k} : {} pas, {} arrivées, {} lignes de journal{}",
                bilan.pas_joues,
                bilan.arrivees,
                bilan.journal.lignes().len(),
                if bilan.reprises > 0 {
                    format!(
                        ", {} reprise(s) après trois `Temps` écartés (`Partie::repartir`)",
                        bilan.reprises
                    )
                } else {
                    String::new()
                }
            ));
            for p in &bilan.pannes_outil {
                self.panne_des_soirees(p, &DES_SOIREES);
            }
            if !bilan.pannes_sans_objet.is_empty() {
                let noms: Vec<String> = bilan.pannes_sans_objet.iter().map(|p| p.nom()).collect();
                self.joue.push(format!(
                    "soirée principale {k} : pannes sans objet, le jeu n'a pas de téléphone : {}",
                    noms.join(", ")
                ));
            }
            for (panne, constatee, note) in &bilan.pannes {
                if *constatee == Oui::Non {
                    self.pannes_manquees
                        .push(format!("soirée {k} : {} ({note})", panne.nom()));
                }
            }
            if let Some(a) = &bilan.abandon {
                self.panne_ou_note(&bilan, a);
            }
            self.lire_le_bilan(&bilan, k);
            faits_terminales = faits_terminales.max(bilan.faits.terminales.len());
            phases.extend(bilan.faits.phases.iter().copied());
            cas.extend(bilan.faits.cas.iter().copied());
            if self.demande(IdControle::E9) {
                self.determinisme(&bilan, &r, k);
            }
            if self.demande(IdControle::E14) {
                self.reprise_native(&bilan);
            }
        }
        self.parcours(faits_terminales);
        self.couverture(&phases, &cas);
        if self.demande(IdControle::E4) {
            self.invariants_evalues();
        }
        if self.demande(IdControle::E19) {
            self.budgets();
        }
    }

    /// E21 : la page de chaque vue partie vers un téléphone (`pages.rs`), et E19 sur
    /// le coût de chacune.
    fn pages(&mut self) {
        let graine = graine_de(self.plan, "E21");
        let vues = std::mem::take(&mut self.vues_e21);
        let module = self.module;
        let Ok(constat) = catch_unwind(AssertUnwindSafe(|| {
            crate::pages::eprouver(module, &vues, &graine)
        })) else {
            self.acc(IdControle::E21)
                .pannes
                .push(String::from("l'épreuve a paniqué en éprouvant les pages"));
            return;
        };
        let fixe = self.fixe;
        if self.demande(IdControle::E19) && !constat.carburants.is_empty() {
            let (c, notes) =
                crate::budgets::juger_les_pages(&BUDGETS_E19, &constat.carburants, fixe);
            let a = self.acc(IdControle::E19);
            a.compte.ajouter(&c);
            a.notes.extend(notes);
        }
        let a = self.acc(IdControle::E21);
        a.compte.ajouter(&constat.compte);
        a.pannes.extend(constat.pannes);
        a.notes.extend(constat.notes);
        if let Some(r) = constat.sans_objet {
            a.sans_objet = Some(r);
        }
    }

    /// La fenêtre déclarée, en pas ; 0 sans fenêtre.
    fn fenetre_declaree(&self) -> u64 {
        match self.module.declaration().fenetre {
            ApplicableFil::Oui(d) => d.en_pas(),
            ApplicableFil::SansObjet(_) => 0,
        }
    }

    /// E19 : le coût des appels et des images des soirées principales.
    fn budgets(&mut self) {
        let fenetre = self.fenetre_declaree();
        let (compte, notes) = crate::budgets::juger(
            &BUDGETS_E19,
            &self.carburant,
            &self.carburant_images,
            fenetre,
            self.fixe,
        );
        let (tours, inacheves) = self.tours;
        let a = self.acc(IdControle::E19);
        a.compte.ajouter(&compte);
        a.notes.extend(notes);
        a.notes.push(format!(
            "{tours} tours de la boucle, dont {inacheves} ont laissé un pas inachevé : ses arriérés (faits sur une vue de plus de {} pas) au-delà de {} par tour, joués aux tours suivants ; chaque tour est jugé, coût fixe compris (PHASE-3, P7)",
            partie::ARRIERE_PAS,
            partie::ARRIERES_PAR_TOUR
        ));
    }

    /// Une soirée arrêtée en chemin : par une faute du jeu (déjà rapportée), ou
    /// par l'outil.
    fn panne_ou_note(&mut self, bilan: &Bilan, abandon: &str) {
        if bilan.fautes.is_empty() && bilan.pannes_outil.is_empty() {
            self.panne_des_soirees(
                &format!("la soirée s'est arrêtée : {abandon}"),
                &DES_SOIREES,
            );
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "le bilan d'une soirée se lit d'un trait, un contrôle après l'autre"
    )]
    fn lire_le_bilan(&mut self, bilan: &Bilan, k: u32) {
        for f in &bilan.fautes {
            let a = self.acc(f.controle);
            a.compte.evaluer();
            a.compte.echouer(format!("soirée {k}, {}", f.detail));
            if a.rejeu.is_none() {
                a.rejeu = Some(rejeu_de(bilan));
            }
        }
        if let Some((pas, c)) = bilan.faits.vue_ecran_max
            && self.vue_ecran_max.is_none_or(|(_, _, m)| c > m)
        {
            self.vue_ecran_max = Some((k, pas, c));
        }
        let obs = bilan.obs.borrow();
        for (ou, f) in &obs.fautes {
            let a = self.acc.entry(IdControle::E2).or_default();
            a.compte.evaluer();
            a.compte.echouer(format!("soirée {k}, {ou} : {f}"));
        }
        for p in &obs.pannes {
            for c in DES_SOIREES {
                self.acc.entry(c).or_default().pannes.push(p.to_string());
            }
        }
        for (nom, v) in &obs.carburant {
            self.carburant
                .entry(nom)
                .or_default()
                .extend(v.iter().copied());
        }
        // Les entrées comptent une fois chacune ; un tour de la boucle, la somme de
        // leurs coûts (carburant et coût fixe) ; une image, `(fenêtre + 2)` tours
        // consécutifs, tours vides compris (PHASE-3, P7 : un pas trop chargé
        // d'arriérés se finit sur plusieurs tours, et chacun est jugé).
        let mut par_tour: BTreeMap<u64, u64> = BTreeMap::new();
        let mut par_pas: BTreeMap<u64, u64> = BTreeMap::new();
        for e in obs.carburant_entrees.values() {
            self.carburant
                .entry("Entree")
                .or_default()
                .push(e.carburant);
            let c = crate::budgets::cout(e.carburant, self.fixe);
            let n = par_tour.entry(e.tour).or_insert(0);
            *n = n.saturating_add(c);
            let n = par_pas.entry(e.place).or_insert(0);
            *n = n.saturating_add(c);
        }
        self.tours.0 = self.tours.0.saturating_add(bilan.tours);
        self.tours.1 = self.tours.1.saturating_add(bilan.tours_inacheves);
        if !par_tour.is_empty() {
            let tours: Vec<crate::budgets::TourJoue> = (1u64..)
                .zip(bilan.tours_joues.iter())
                .map(|(t, (definitif, fini))| crate::budgets::TourJoue {
                    cout: par_tour.get(&t).copied().unwrap_or(0),
                    definitif: *definitif,
                    fini: *fini,
                })
                .collect();
            self.carburant_images.extend(crate::budgets::images(
                &tours,
                &par_pas,
                self.fenetre_declaree(),
            ));
        }
        if !obs.phases.is_empty() {
            self.phases_des_soirees
                .push((obs.phases.clone(), bilan.gels.clone()));
        }
        if !obs.carburant.is_empty() {
            let e19 = self.acc.entry(IdControle::E19).or_default();
            if e19.rejeu.is_none() {
                e19.rejeu = Some(rejeu_de(bilan));
            }
        }
        let pas = bilan.pas_joues;
        let e2 = self.acc.entry(IdControle::E2).or_default();
        // Le parcours se juge sur la soirée entière : son rejeu est celui de la soirée.
        if e2.rejeu.is_none() {
            e2.rejeu = Some(rejeu_de(bilan));
        }
        e2.compte.evaluations = e2.compte.evaluations.saturating_add(pas);
        for (c, compte) in [
            (IdControle::E4, &obs.e4),
            (IdControle::E5, &bilan.e5),
            (IdControle::E6, &obs.e6),
            (IdControle::E7, &obs.e7),
            (IdControle::E8, &bilan.e8),
            (IdControle::E8, &obs.e8),
            (IdControle::E14, &bilan.e14),
            (IdControle::E17, &bilan.e17),
        ] {
            let echecs = compte.echecs;
            let a = self.acc.entry(c).or_default();
            a.compte.ajouter(compte);
            if echecs > 0 && a.rejeu.is_none() {
                a.rejeu = Some(rejeu_de(bilan));
            }
        }
        if obs.e8.evaluations > 0 {
            self.acc.entry(IdControle::E8).or_default().noter(format!(
                "soirée {k} : {} geste(s) ou tenu(s) rejoué(s) avec un autre octet qui change un secret : la projection publique et les vues comparées octet pour octet",
                obs.e8.evaluations
            ));
        } else if obs.verifs.e8 == Oui::Oui {
            // Zéro jugement n'est pas un succès (OUT-07) : la recherche d'octets a seule
            // tourné, et le rapport le dit (revue de la phase 2, constat 2).
            self.acc.entry(IdControle::E8).or_default().noter(format!(
                "soirée {k} : la variation n'a rien jugé ({} entrée(s) rejouée(s) avec un autre octet, {} variante(s) acceptée(s), aucune n'a changé un secret) ; seule la recherche d'octets a tourné",
                obs.variations.0, obs.variations.1
            ));
        }
        if bilan.faits.secrets_non_cherches > 0 {
            self.acc.entry(IdControle::E8).or_default().noter(format!(
                "soirée {k} : {}",
                crate::controles::trop_courts(bilan.faits.secrets_non_cherches)
            ));
        }
        if self.premier_rejeu.is_none() {
            self.premier_rejeu = Some(rejeu_de(bilan));
        }
        for (nom, n) in &obs.invariants {
            let vu = self.invariants_vus.entry(nom.clone()).or_insert(0);
            *vu = vu.saturating_add(*n);
        }
        if !obs.invariants.is_empty() {
            let noms: Vec<String> = obs
                .invariants
                .iter()
                .map(|(n, c)| format!("« {n} » {c} fois"))
                .collect();
            self.acc
                .entry(IdControle::E4)
                .or_default()
                .noter(format!("soirée {k} : {}", noms.join(", ")));
        }
    }

    /// E9 : la même soirée, rejouée de bout en bout, a le même journal, empreintes
    /// comprises ; puis le journal, entrée par entrée, sur une jumelle et la copie
    /// native.
    fn determinisme(&mut self, bilan: &Bilan, reglage: &Reglage, numero: u32) {
        if numero == 0 {
            let mut deux = reglage.clone();
            deux.verifs = Verifs {
                releve: Oui::Oui,
                ..Verifs::RIEN
            };
            deux.e5 = Oui::Non;
            deux.e8 = Oui::Non;
            match self.jouer(deux) {
                Ok(rejouee) => {
                    let acc = self.acc(IdControle::E9);
                    acc.compte.evaluer();
                    if rejouee.journal == bilan.journal {
                        acc.noter(format!(
                            "soirée {numero} jouée deux fois sur le module : même journal, {} lignes",
                            rejouee.journal.lignes().len()
                        ));
                    } else {
                        let lignes = rejouee
                            .journal
                            .lignes()
                            .len()
                            .max(bilan.journal.lignes().len());
                        let difference = crate::script::premiere_difference(
                            &rejouee.journal,
                            &bilan.journal,
                            lignes,
                        )
                        .unwrap_or_else(|| String::from("les empreintes notées diffèrent"));
                        acc.compte.echouer(format!(
                            "la soirée {numero}, rejouée sur le module, lignes'a pas le même journal : {difference}"
                        ));
                        if acc.rejeu.is_none() {
                            acc.rejeu = Some(rejeu_de(bilan));
                        }
                    }
                }
                Err(e) => self.acc(IdControle::E9).pannes.push(e),
            }
        }
        let natif = self.natif.clone();
        let constat = match &natif {
            Some(lignes) => {
                let mut fabrique = lignes.borrow_mut();
                crate::determinisme::rejouer(self.module, Some(&mut **fabrique), bilan)
            }
            None => crate::determinisme::rejouer(self.module, None, bilan),
        };
        let acc = self.acc(IdControle::E9);
        let echecs = constat.e9.echecs;
        acc.compte.ajouter(&constat.e9);
        acc.pannes.extend(constat.pannes);
        acc.noter(format!(
            "soirée {numero} rejouée entrée par entrée ({} entrées) sur une jumelle du module{}",
            constat.entrees,
            if natif.is_some() {
                " et la copie native"
            } else {
                " ; copie native absente"
            }
        ));
        if echecs > 0 && acc.rejeu.is_none() {
            acc.rejeu = Some(rejeu_de(bilan));
        }
    }

    /// E14 sur la copie native : le script de la soirée rejoué sur elle, ses
    /// redémarrages et ses reprises.
    fn reprise_native(&mut self, bilan: &Bilan) {
        let Some(n) = self.natif.clone() else {
            return;
        };
        if crate::script::faute_sans_borne_native(&bilan.journal) {
            self.acc(IdControle::E14).noter(
                "copie native non rejouée : le module a fauté par carburant, profondeur ou mémoire, que la copie native ne borne pas",
            );
            return;
        }
        let r = crate::script::rejouer(
            &Effacee(n),
            bilan.ouverture.graine,
            &bilan.script,
            &bilan.journal,
        );
        if let Some(d) = r.divergence_avant {
            let a = self.acc(IdControle::E9);
            a.compte.evaluer();
            a.compte.echouer(format!(
                "la partie jouée sur la copie native diverge de celle du module : {d}"
            ));
            if a.rejeu.is_none() {
                a.rejeu = Some(rejeu_de(bilan));
            }
            self.acc(IdControle::E14).noter(
                "copie native non jugée : elle diverge du module avant toute reprise (voir E9)",
            );
            return;
        }
        let a = self.acc(IdControle::E14);
        if let Some(p) = r.panne {
            a.pannes.push(p);
        }
        let echecs = r.e14.echecs;
        a.compte.ajouter(&r.e14);
        a.noter(format!(
            "copie native : la partie rejouée, {} redémarrage(s), puis ses reprises",
            r.redemarrages
        ));
        if echecs > 0 && a.rejeu.is_none() {
            a.rejeu = Some(rejeu_de(bilan));
        }
    }

    /// E2 : les bots vont jusqu'à une phase terminale, puis une nouvelle partie, puis
    /// une autre fin (OUT-29) ; aucune faute en chemin.
    fn parcours(&mut self, terminales: usize) {
        let fin = self.module.declaration().fin.clone();
        let a = self.acc(IdControle::E2);
        match fin {
            ApplicableFil::SansObjet(r) => {
                a.sans_objet = Some(String::from(r.as_str()));
            }
            ApplicableFil::Oui(()) => {
                a.compte.evaluer();
                match terminales {
                    0 => a
                        .compte
                        .echouer(String::from("aucune phase terminale atteinte")),
                    1 => a.compte.echouer(String::from(
                        "une fin, mais aucune autre après la nouvelle partie",
                    )),
                    n => a.noter(format!("{n} parties finies dans une même soirée")),
                }
            }
        }
    }

    /// E4 : chaque invariant que le jeu déclare est évalué au moins une fois dans les
    /// soirées principales, et aucun nom hors de sa déclaration ne l'est (OUT-01,
    /// RESTE-PHASE-1, point 24). Un invariant écrit dans une branche qui n'arrive
    /// jamais n'est pas tenu : il n'a pas tourné.
    fn invariants_evalues(&mut self) {
        let Some(declares) = self.invariants_declares.clone() else {
            return;
        };
        let vus = self.invariants_vus.clone();
        // Aucune évaluation du tout : les soirées n'ont accepté aucune entrée, et E4 n'a
        // pas tourné. Il reste « pas exécuté » (OUT-07), plutôt que d'échouer sur chaque
        // nom déclaré.
        if vus.values().all(|n| *n == 0) {
            return;
        }
        let rejeu = self.premier_rejeu.clone();
        let a = self.acc(IdControle::E4);
        let mut echecs = 0u32;
        for nom in &declares {
            a.compte.evaluer();
            if vus.get(nom).copied().unwrap_or(0) == 0 {
                echecs = echecs.saturating_add(1);
                a.compte.echouer(format!(
                    "« {nom} », déclaré dans `Epreuve::INVARIANTS`, n'est évalué dans aucune soirée"
                ));
            }
        }
        for nom in vus.keys() {
            a.compte.evaluer();
            if !declares.contains(nom) {
                echecs = echecs.saturating_add(1);
                a.compte.echouer(format!(
                    "« {nom} » est évalué, mais n'est pas déclaré dans `Epreuve::INVARIANTS`"
                ));
            }
        }
        if echecs > 0 && a.rejeu.is_none() {
            a.rejeu = rejeu;
        }
        a.noter(format!(
            "{} invariant(s) déclaré(s), {} nom(s) évalué(s)",
            declares.len(),
            vus.len()
        ));
    }

    /// E3 : chaque phase et chaque cas atteints.
    fn couverture(&mut self, phases: &BTreeSet<u16>, cas: &BTreeSet<u16>) {
        // Toutes les variantes de `Phase`, qu'un pire cas y mène ou non (RESTE-PHASE-1,
        // point 23) : celles que le jeu déclare, celles des pires cas, celles vues.
        let declarees = self.phases_declarees;
        let mut univers_phases: BTreeSet<u16> =
            self.phases_univers.union(phases).copied().collect();
        if let Some(n) = declarees {
            univers_phases.extend(0..n);
        }
        let nb_cas = self.cas_univers;
        let a = self.acc(IdControle::E3);
        for p in &univers_phases {
            a.compte.evaluer();
            if declarees.is_some_and(|n| *p >= n) {
                a.compte.echouer(format!(
                    "la phase d'indice {p} n'est pas dans `Phase::TOUTES`, qui en déclare {}",
                    declarees.unwrap_or(0)
                ));
            } else if !phases.contains(p) {
                a.compte.echouer(format!(
                    "la phase d'indice {p} dans `Phase::TOUTES` n'est jamais atteinte"
                ));
            }
        }
        match nb_cas {
            Some(n) => {
                for c in 0..n {
                    a.compte.evaluer();
                    if !cas.contains(&c) {
                        a.compte.echouer(format!(
                            "le cas d'indice {c} dans `Cas::TOUTES` n'est jamais atteint"
                        ));
                    }
                }
                a.noter(format!(
                    "phases vues {phases:?} sur {univers_phases:?} connues ({}) ; cas vus {cas:?} sur {n}",
                    declarees.map_or_else(
                        || String::from("`Phase::TOUTES` non lue"),
                        |d| format!("`Phase::TOUTES` en déclare {d}")
                    )
                ));
            }
            None => a.noter(String::from("les cas n'ont pas pu être comptés")),
        }
    }

    /// E13 : des appels absurdes sur une instance nue, puis une soirée de téléphones
    /// hostiles et d'arrivées absurdes.
    fn entrees_absurdes(&mut self) {
        let graine = graine_de(self.plan, "absurde");
        let duree = (self.plan.duree.en_pas() / 2).max(3600);
        let r = self.reglage(Sorte::Hostile, graine, duree);
        let bilan = match self.jouer(r) {
            Ok(b) => b,
            Err(e) => {
                self.acc(IdControle::E13).pannes.push(e);
                return;
            }
        };
        self.joue.push(format!(
            "soirée hostile : {} pas, {} arrivées dont {} absurdes",
            bilan.pas_joues, bilan.arrivees, bilan.e13.evaluations
        ));
        let mut t = Tirage::new(&graine, "appels absurdes");
        let nue = self.appels_absurdes(&mut t, &bilan.echantillons);
        let a = self.acc(IdControle::E13);
        a.compte.ajouter(&bilan.e13);
        a.pannes.extend(bilan.pannes_outil.iter().cloned());
        for f in &bilan.fautes {
            a.compte.echouer(f.detail.clone());
            if a.rejeu.is_none() {
                a.rejeu = Some(rejeu_de(&bilan));
            }
        }
        let obs = bilan.obs.borrow();
        for (ou, f) in &obs.fautes {
            a.compte.echouer(format!("{ou} : {f}"));
        }
        match nue {
            Ok(c) => a.compte.ajouter(&c),
            Err(p) => a.pannes.push(p),
        }
    }

    /// Des octets au hasard dans chaque `Valider` et dans l'enveloppe entière, sur
    /// une instance nue : `Malforme` ou `Illisible`, jamais une faute.
    fn appels_absurdes(
        &mut self,
        tirage: &mut Tirage,
        echantillons: &[(SorteOctets, Vec<u8>)],
    ) -> Result<Compte, String> {
        let mut compte = Compte::default();
        let mut inst =
            Fabrique::neuve(&mut self.module.fabrique()).map_err(|place| place.to_string())?;
        let ouv = ouverture(self.plan.graine);
        let mut presents = Vec::new();
        encoder(&Vec::<jeu_abi::Presence>::new(), &mut presents)
            .map_err(|erreur| erreur.to_string())?;
        for appel in [
            Appel::Bonjour,
            Appel::Decrire,
            Appel::Creer {
                graine: ouv.graine,
                pas: 0,
                heure: ouv.heure,
                salle: ouv.salle,
                presents: &presents,
                garde: None,
            },
        ] {
            let _: Vec<u8> = inst
                .appeler(appel.sorte(), &encode(&appel), BUDGETS.creation)
                .map_err(|erreur| erreur.to_string())?;
        }
        let sortes = [
            SorteOctets::ActionEcran,
            SorteOctets::GesteTelephone,
            SorteOctets::TenuTelephone,
        ];
        for rang in 0..600u64 {
            let sorte = sortes
                .get(usize::try_from(rang.checked_rem(3).unwrap_or(0)).unwrap_or(0))
                .copied()
                .unwrap_or(SorteOctets::GesteTelephone);
            let octets = match echantillons
                .iter()
                .filter(|(sorte_vue, _)| *sorte_vue == sorte)
                .nth(usize::try_from(tirage.sous(8)).unwrap_or(0))
            {
                Some((_, ouv)) if tirage.une_sur(2) => {
                    let mut abime = ouv.clone();
                    let taille = u64::try_from(abime.len()).unwrap_or(0);
                    let place = usize::try_from(tirage.sous(taille.max(1))).unwrap_or(0);
                    let octet = tirage.octet();
                    if let Some(octet_vise) = abime.get_mut(place) {
                        *octet_vise = octet;
                    }
                    abime
                }
                Some(_) | None => {
                    let taille = usize::try_from(tirage.entre(0, 40)).unwrap_or(0);
                    tirage.octets(taille)
                }
            };
            compte.evaluer();
            let appel = Appel::Valider {
                sorte,
                octets: &octets,
            };
            match Instance::appeler(
                &mut inst,
                SorteAppel::Lecture,
                &encode(&appel),
                BUDGETS.lecture,
            ) {
                Ok(retour) => {
                    if !matches!(decoder::<Retour<'_>>(&retour), Ok(Retour::Valide(_))) {
                        compte.echouer(format!(
                            "Valider rend autre chose qu'une validation, sur {octets:?}"
                        ));
                    }
                }
                Err(Echec::Faute(faute)) => {
                    compte.echouer(format!(
                        "Valider faute sur des octets absurdes {octets:?} : {faute}"
                    ));
                    return Ok(compte);
                }
                Err(Echec::Panne(place)) => return Err(place.to_string()),
            }
        }
        for _ in 0..300 {
            let taille = usize::try_from(tirage.entre(0, 64)).unwrap_or(0);
            let octets = tirage.octets(taille);
            let sorte = match decoder::<Appel<'_>>(&octets) {
                Ok(appel) => appel.sorte(),
                Err(_) => SorteAppel::Lecture,
            };
            compte.evaluer();
            match Instance::appeler(&mut inst, sorte, &octets, BUDGETS.lecture) {
                Ok(_) => {}
                Err(Echec::Faute(faute)) => {
                    compte.echouer(format!(
                        "un appel absurde {octets:?} fait fauter le module : {faute}"
                    ));
                    return Ok(compte);
                }
                Err(Echec::Panne(place)) => return Err(place.to_string()),
            }
        }
        Ok(compte)
    }

    /// E15 : une soirée longue ; la pente des octets vivants et des appareils retenus.
    fn memoire(&mut self) {
        let graine = graine_de(self.plan, "longue");
        let r = self.reglage(Sorte::Longue, graine, self.plan.memoire.en_pas());
        let bilan = match self.jouer(r) {
            Ok(b) => b,
            Err(e) => {
                self.acc(IdControle::E15).pannes.push(e);
                return;
            }
        };
        let allees: Vec<String> = bilan
            .allees_et_venues
            .iter()
            .map(|(sorte, n)| format!("{n} {sorte}"))
            .collect();
        self.joue.push(format!(
            "soirée longue : {} pas, {} arrivées, {} mesures de la mémoire",
            bilan.pas_joues,
            bilan.arrivees,
            bilan.memoire.len()
        ));
        // PHASE-3, P10 : ce que le journal d'une longue soirée pèse sans troncature, en
        // lignes et en octets de la garde (`partie::persistance`).
        self.joue.push(format!(
            "journal de la soirée longue : {} lignes, {} octets écrits comme la garde les écrit, sans troncature",
            bilan.journal.lignes().len(),
            partie::persistance::encoder_journal(&bilan.journal).len()
        ));
        let a = self.acc(IdControle::E15);
        a.noter(format!(
            "allées et venues de la soirée longue, chacune suivie de son retour : {}",
            if allees.is_empty() {
                String::from("aucune")
            } else {
                allees.join(", ")
            }
        ));
        a.manquees.extend(bilan.allees_manquees.iter().cloned());
        a.pannes.extend(bilan.pannes_outil.iter().cloned());
        for f in &bilan.fautes {
            a.compte.evaluer();
            a.compte.echouer(f.detail.clone());
            if a.rejeu.is_none() {
                a.rejeu = Some(rejeu_de(&bilan));
            }
        }
        if a.rejeu.is_none() {
            a.rejeu = Some(rejeu_de(&bilan));
        }
        juger_la_memoire(a, &bilan.memoire);
    }

    /// E16 : chaque pire cas, sur une instance nue ; et E17 sur leur forme canonique.
    fn pires_cas(&mut self) {
        let c = crate::pires_cas::eprouver(self.module);
        self.joue.push(format!("pires cas : {}", c.cas));
        let vue_ecran_max = self.vue_ecran_max;
        let a = self.acc(IdControle::E16);
        a.compte.ajouter(&c.e16);
        a.pannes.extend(c.pannes.iter().cloned());
        a.noter(format!(
            "{} cas construits, chacun : son cas, ses invariants, sa scène",
            c.cas
        ));
        // Un pire cas se construit depuis les limites, pour être le plus lourd
        // (INTERFACE, § 2.13) : aucune vue de l'écran des soirées ne doit coûter plus
        // que le plus lourd d'entre eux. L'épreuve ne connaît pas les limites propres
        // au jeu (tuiles, joueurs, textes) ; elle tient au moins celle-là (revue de la
        // phase 2, constat 14).
        let pire = c.carburant_vue.iter().map(|(_, n)| *n).max();
        match (vue_ecran_max, pire) {
            (Some((k, pas, vue)), Some(pire)) => {
                a.compte.evaluer();
                let marge = crate::pires_cas::MARGE_DU_PIRE_CAS_POUR_CENT;
                let plafond = pire.saturating_add(pire.saturating_mul(marge) / 100);
                if vue > plafond {
                    a.compte.echouer(format!(
                        "la vue de l'écran de la soirée {k}, au pas {pas}, coûte {vue} unités, plus que celle de chaque pire cas (au plus {pire}, {marge} % de marge comprise : {plafond}) : un pire cas se construit depuis les limites pour être le plus lourd (INTERFACE, § 2.13)"
                    ));
                } else {
                    a.noter(format!(
                        "la vue de l'écran la plus lourde des soirées ({vue} unités, soirée {k}, pas {pas}) ne dépasse pas de plus de {marge} % celle du plus lourd des pires cas ({pire})"
                    ));
                }
            }
            (None, _) => a.noter(
                "aucune vue de l'écran des soirées n'a été lue par E8 : le pire cas n'est pas comparé à elles",
            ),
            (Some(_), None) => {}
        }
        if self.demande(IdControle::E17) {
            let a = self.acc(IdControle::E17);
            a.compte.ajouter(&c.e17);
            a.noter(format!("et les formes canoniques de {} pires cas", c.cas));
        }
        if self.demande(IdControle::E19) {
            let (compte, notes) =
                crate::budgets::juger_les_pires_cas(&BUDGETS_E19, &c.carburant_vue, self.fixe);
            let a = self.acc(IdControle::E19);
            a.compte.ajouter(&compte);
            a.notes.extend(notes);
        }
    }

    /// Les propriétés que le jeu promet, lues sur une instance nue (ABI 2, puis 3 :
    /// les phases et les invariants déclarés).
    fn lire_proprietes(&self) -> Result<jeu_abi::ProprietesFil, String> {
        let mut i = Fabrique::neuve(&mut self.module.fabrique()).map_err(|p| p.to_string())?;
        for a in [Appel::Bonjour, Appel::Decrire] {
            let _: Vec<u8> = i
                .appeler(SorteAppel::Lecture, &encode(&a), BUDGETS.lecture)
                .map_err(|e| e.to_string())?;
        }
        let r = i
            .appeler(
                SorteAppel::Lecture,
                &encode(&Appel::Proprietes),
                BUDGETS.lecture,
            )
            .map_err(|e| e.to_string())?;
        match decoder::<Retour<'_>>(&r) {
            Ok(Retour::Proprietes(p)) => Ok(p),
            Ok(autre) => Err(format!("`Proprietes` rend {autre:?}")),
            Err(e) => Err(format!("`Proprietes` ne se lit pas : {e}")),
        }
    }

    /// E10, E11, E12 : la soirée des propriétés, puis ses variantes.
    #[expect(
        clippy::too_many_lines,
        reason = "les trois variantes d'une même soirée se lisent ensemble"
    )]
    fn proprietes(&mut self) {
        const LES_TROIS: [IdControle; 3] = [IdControle::E10, IdControle::E11, IdControle::E12];
        let demandes: Vec<IdControle> =
            LES_TROIS.into_iter().filter(|c| self.demande(*c)).collect();
        let props = match self.lire_proprietes() {
            Ok(p) => p,
            Err(e) => {
                self.panne_des_soirees(&e, &demandes);
                return;
            }
        };
        let fenetre = match self.module.declaration().fenetre.clone() {
            ApplicableFil::Oui(d) if d.en_pas() > 0 => Ok(d.en_pas()),
            ApplicableFil::Oui(_) => Err(String::from("fenêtre nulle")),
            ApplicableFil::SansObjet(r) => Err(r),
        };
        let mut a_jouer = Vec::new();
        for c in &demandes {
            let sans_objet = if *c == IdControle::E10 {
                match &props.permutation {
                    ApplicableFil::Oui(()) => None,
                    ApplicableFil::SansObjet(r) => Some(r.clone()),
                }
            } else if *c == IdControle::E11 {
                match &props.coupure_transparente {
                    ApplicableFil::Oui(_) => None,
                    ApplicableFil::SansObjet(r) => Some(r.clone()),
                }
            } else {
                // E12 éprouve aussi le découpage d'un pas, que tout jeu traverse : il
                // n'est jamais sans objet ; sans fenêtre, seul le découpage se juge.
                None
            };
            match sans_objet {
                Some(r) => self.acc(*c).sans_objet = Some(r),
                None => a_jouer.push(*c),
            }
        }
        if a_jouer.is_empty() {
            return;
        }
        let graine = graine_de(self.plan, "propriétés");
        let r = self.reglage(Sorte::Proprietes, graine, self.plan.duree.en_pas());
        let bilan = match self.jouer(r) {
            Ok(b) => b,
            Err(e) => {
                self.panne_des_soirees(&e, &a_jouer);
                return;
            }
        };
        self.joue.push(format!(
            "soirée des propriétés : {} pas, {} arrivées",
            bilan.pas_joues, bilan.arrivees
        ));
        if !bilan.pannes_outil.is_empty() || bilan.abandon.is_some() {
            let quoi = bilan
                .pannes_outil
                .first()
                .cloned()
                .or_else(|| bilan.abandon.clone())
                .unwrap_or_default();
            if bilan.fautes.is_empty() {
                self.panne_des_soirees(&format!("la soirée des propriétés : {quoi}"), &a_jouer);
                return;
            }
        }
        for f in &bilan.fautes {
            for c in &a_jouer {
                let a = self.acc(*c);
                a.compte.evaluer();
                a.compte
                    .echouer(format!("la soirée des propriétés, {}", f.detail));
                if a.rejeu.is_none() {
                    a.rejeu = Some(rejeu_de(&bilan));
                }
            }
        }
        if !bilan.fautes.is_empty() {
            return;
        }
        let ouv = bilan.ouverture.clone();
        let script = bilan.script.clone();
        let quoi = crate::proprietes::Quoi {
            resumes: if a_jouer.contains(&IdControle::E10) {
                Oui::Oui
            } else {
                Oui::Non
            },
            vues: if a_jouer.contains(&IdControle::E10) {
                Oui::Oui
            } else {
                Oui::Non
            },
            hypotheses: a_jouer
                .contains(&IdControle::E11)
                .then_some(crate::proprietes::PERIODE_HYPOTHESE),
        };
        let original = match crate::proprietes::rejouer(self.module, &ouv, &script, quoi) {
            Ok(o) => o,
            Err(e) => {
                self.panne_des_soirees(&e, &a_jouer);
                return;
            }
        };
        let mut t = Tirage::new(&graine, "variantes");
        for c in a_jouer {
            let constat = if c == IdControle::E10 {
                self.permutation(&bilan, &original, &mut t)
            } else if c == IdControle::E11 {
                match props.coupure_transparente {
                    ApplicableFil::Oui(moins_de) => self.coupures(&bilan, &original, moins_de),
                    ApplicableFil::SansObjet(_) => continue,
                }
            } else {
                match self.retarder_et_decouper(&bilan, &original, &fenetre, &mut t) {
                    Ok(None) => {
                        let raison = fenetre.clone().err().unwrap_or_default();
                        self.acc(c).sans_objet = Some(format!(
                            "{raison} ; et aucun pas de la soirée des propriétés n'a deux entrées venues d'arrivées : rien à découper"
                        ));
                        continue;
                    }
                    Ok(Some(constat)) => Ok(constat),
                    Err(e) => Err(e),
                }
            };
            let a = self.acc(c);
            match constat {
                Ok((compte, notes)) => {
                    let echecs = compte.echecs;
                    a.compte.ajouter(&compte);
                    a.notes.extend(notes);
                    if echecs > 0 && a.rejeu.is_none() {
                        a.rejeu = Some(rejeu_de(&bilan));
                    }
                }
                Err(e) => a.pannes.push(e),
            }
        }
    }

    /// E10 : la soirée, renumérotée, a le même résumé et les mêmes vues (CAT-27).
    fn permutation(
        &self,
        bilan: &Bilan,
        original: &crate::proprietes::Releve,
        t: &mut Tirage,
    ) -> Result<(Compte, Vec<String>), String> {
        let appareils: Vec<u64> = bilan
            .regards
            .iter()
            .filter_map(|r| match r {
                jeu_abi::RegardFil::Appareil(a) => Some(*a),
                jeu_abi::RegardFil::Ecran => None,
            })
            .collect();
        let table = crate::proprietes::renumerotation(&appareils, t);
        let script = crate::proprietes::renumeroter(&bilan.script, &table);
        let quoi = crate::proprietes::Quoi {
            resumes: Oui::Oui,
            vues: Oui::Oui,
            hypotheses: None,
        };
        let renumerote = crate::proprietes::rejouer(self.module, &bilan.ouverture, &script, quoi)?;
        let c = crate::proprietes::comparer_permutation(original, &renumerote, &table);
        let notes = vec![format!(
            "{} appareils renumérotés ; {} résumés et {} vues comparés",
            table.len(),
            original.resumes.len(),
            original.vues.iter().map(Vec::len).sum::<usize>()
        )];
        Ok((c, notes))
    }

    /// E11 : des coupures plus courtes que `moins_de`, là où l'hypothèse a tenu
    /// pendant toute la coupure, ne changent pas le résumé final (PUN-73, SQ-28).
    fn coupures(
        &self,
        bilan: &Bilan,
        original: &crate::proprietes::Releve,
        moins_de: jeu_abi::Duree,
    ) -> Result<(Compte, Vec<String>), String> {
        let mut c = Compte::default();
        let mut notes = Vec::new();
        let coupures = crate::proprietes::choisir_coupures(
            &bilan.script,
            &original.hypotheses,
            crate::proprietes::PERIODE_HYPOTHESE,
            moins_de,
            3,
        );
        if coupures.is_empty() {
            notes.push(format!(
                "aucune coupure possible : l'hypothèse a tenu {} fois, jamais pendant toute une coupure, loin d'une présence et de la fin",
                original.hypotheses.len()
            ));
        }
        for k in coupures {
            let script = crate::proprietes::couper(&bilan.script, k);
            let coupee = crate::proprietes::rejouer(
                self.module,
                &bilan.ouverture,
                &script,
                crate::proprietes::Quoi::FIN,
            )?;
            c.evaluer();
            if coupee.resume != original.resume || original.resume.is_none() {
                c.echouer(format!(
                    "une coupure de l'appareil {:#x}, des pas {} à {}, où l'hypothèse tenait, change le résumé de la soirée",
                    k.appareil, k.debut, k.fin
                ));
            } else {
                notes.push(format!(
                    "coupure des pas {} à {} : même résumé",
                    k.debut, k.fin
                ));
            }
        }
        Ok((c, notes))
    }

    /// E12 : la fenêtre, si le jeu en déclare une, puis le découpage d'un pas. `None` :
    /// ni fenêtre, ni pas à découper (aucun pas n'a deux entrées venues d'arrivées,
    /// comme chez un jeu où l'écran seul joue) : E12 est alors sans objet.
    fn retarder_et_decouper(
        &self,
        bilan: &Bilan,
        original: &crate::proprietes::Releve,
        fenetre: &Result<u64, String>,
        tirage: &mut Tirage,
    ) -> Result<Option<(Compte, Vec<String>)>, String> {
        let (mut compte, mut notes) = match fenetre {
            Ok(w) => self.fenetre(bilan, original, *w, tirage)?,
            Err(r) => (
                Compte::default(),
                vec![format!(
                    "fenêtre sans objet ({r}) : seul le découpage d'un pas se juge"
                )],
            ),
        };
        let Some((c, n)) = self.decoupage(bilan)? else {
            if fenetre.is_err() {
                return Ok(None);
            }
            notes.push(String::from(
                "découpage : aucun pas n'a deux entrées venues d'arrivées, rien à découper",
            ));
            return Ok(Some((compte, notes)));
        };
        compte.ajouter(&c);
        notes.extend(n);
        Ok(Some((compte, notes)))
    }

    /// E12 : un pas joué en plusieurs tours, avec des arrivées reçues pendant ce pas,
    /// donne le même journal, ligne pour ligne, que d'un seul tour (PHASE-3, P7 ;
    /// CAT-14). Chaque `Avancer` du script des propriétés y joue des tours d'une entrée
    /// venue d'une arrivée, et les arrivées qui le suivent sont remontées juste après
    /// lui. Le contrôle négatif, `etale-mal`, est un défaut du commun
    /// ([`crate::CommunFautif::EtaleMal`]) : aucun jeu ne sait faire échouer ce que le
    /// commun seul décide. Une soirée où aucune arrivée ne suit un pas découpé compare
    /// encore les pas joués en plusieurs tours, et le dit.
    ///
    /// `None` : aucun tour n'a laissé de pas inachevé, rien à découper.
    fn decoupage(&self, bilan: &Bilan) -> Result<Option<(Compte, Vec<String>)>, String> {
        let mut compte = Compte::default();
        let (script, remontees) = crate::proprietes::regrouper(&bilan.script);
        let (script, coupees) = crate::proprietes::couper_les_rafales(script);
        let entier = crate::proprietes::rejouer_decoupe(
            self.module,
            &bilan.ouverture,
            &script,
            None,
            Oui::Non,
        )?;
        let fautif = if self.plan.commun_fautif == Some(crate::plan::CommunFautif::EtaleMal) {
            Oui::Oui
        } else {
            Oui::Non
        };
        let decoupe = crate::proprietes::rejouer_decoupe(
            self.module,
            &bilan.ouverture,
            &script,
            Some(1),
            fautif,
        )?;
        let mut notes = vec![format!(
            "découpage : {} tours d'une entrée ont laissé un pas inachevé, {} arrivées reçues pendant un pas inachevé, dont {} avant le dernier pas de l'avancée ou son saut ({remontees} remontées après leur pas, {coupees} passées après le pas de leur rafale){}",
            decoupe.inacheves,
            decoupe.pendant,
            decoupe.avant_la_fin,
            if fautif == Oui::Oui {
                " ; contrôle négatif `etale-mal` armé"
            } else {
                ""
            }
        )];
        if decoupe.inacheves == 0 {
            return Ok(None);
        }
        if decoupe.pendant == 0 {
            // Des pas joués en plusieurs tours se comparent encore ; seule la part des
            // arrivées reçues pendant eux n'est pas éprouvée, et le rapport le dit.
            notes.push(String::from(
                "découpage : aucune arrivée n'a suivi un pas découpé dans cette soirée ; seuls les pas joués en plusieurs tours sont comparés",
            ));
        }
        compte.evaluer();
        let (a, b) = (entier.journal.lignes(), decoupe.journal.lignes());
        if let Some(i) = (0..a.len().max(b.len())).find(|i| a.get(*i) != b.get(*i)) {
            let court = |l: Option<&partie::Ligne>| {
                l.map_or_else(
                    || String::from("la fin"),
                    |l| format!("{l:?}").chars().take(160).collect(),
                )
            };
            compte.echouer(format!(
                "un pas découpé en tours ne donne pas le journal d'un seul tour, à la ligne {i} : {} contre {}",
                court(a.get(i)),
                court(b.get(i))
            ));
        } else if let Some(i) = (0..entier.vues.len().max(decoupe.vues.len()))
            .find(|i| entier.vues.get(*i) != decoupe.vues.get(*i))
        {
            // Le journal ne dit pas ce que montre la tête : ses vues se comparent à part.
            compte.echouer(format!(
                "un pas découpé en tours ne donne pas les vues d'un seul tour, au regard {i} du script"
            ));
        }
        Ok(Some((compte, notes)))
    }

    /// E12 : les gestes reçus en retard dans la fenêtre donnent la même histoire
    /// définitive (§ 4.3, D12).
    fn fenetre(
        &self,
        bilan: &Bilan,
        original: &crate::proprietes::Releve,
        largeur: u64,
        tirage: &mut Tirage,
    ) -> Result<(Compte, Vec<String>), String> {
        let mut compte = Compte::default();
        let (script, nombre) = crate::proprietes::retarder(&bilan.script, largeur, tirage);
        let notes = vec![format!(
            "{nombre} gestes reçus plus tard, dans la fenêtre de {largeur} pas"
        )];
        if nombre == 0 {
            return Ok((compte, notes));
        }
        let variante = crate::proprietes::rejouer(
            self.module,
            &bilan.ouverture,
            &script,
            crate::proprietes::Quoi::FIN,
        )?;
        let (Some(avant), Some(apres)) = (&original.journal, &variante.journal) else {
            return Err(String::from("un rejeu n'a pas de journal"));
        };
        compte.evaluer();
        if let Some(d) = crate::proprietes::premiere_difference(
            &crate::proprietes::histoire(avant),
            &crate::proprietes::histoire(apres),
        ) {
            compte.echouer(format!(
                "des gestes en retard dans la fenêtre changent l'histoire définitive : {d}"
            ));
        }
        compte.evaluer();
        if original.canonique != variante.canonique {
            compte.echouer(String::from(
                "des gestes en retard dans la fenêtre changent la forme canonique finale",
            ));
        }
        Ok((compte, notes))
    }

    /// E18 : une soirée de gels pendant les échéances.
    fn gels(&mut self) {
        let graine = graine_de(self.plan, "gels");
        let r = self.reglage(Sorte::Gel, graine, self.plan.duree.en_pas());
        let bilan = match self.jouer(r) {
            Ok(b) => b,
            Err(e) => {
                self.acc(IdControle::E18).pannes.push(e);
                return;
            }
        };
        self.joue.push(format!(
            "soirée des gels : {} pas, {} gel(s) pendant une échéance",
            bilan.pas_joues,
            bilan.gels.len()
        ));
        let a = self.acc(IdControle::E18);
        a.pannes.extend(bilan.pannes_outil.iter().cloned());
        for f in &bilan.fautes {
            a.compte.evaluer();
            a.compte.echouer(f.detail.clone());
        }
        let obs = bilan.obs.borrow();
        let mut soirees: Vec<crate::gel::SoireeGelee<'_>> = self
            .phases_des_soirees
            .iter()
            .map(|(p, g)| (p.as_slice(), g.as_slice()))
            .collect();
        soirees.push((&obs.phases, &bilan.gels));
        let (compte, notes) = crate::gel::juger(&soirees);
        drop(obs);
        let a = self.acc(IdControle::E18);
        if bilan.gels.is_empty() {
            a.noter("aucune échéance assez lointaine où geler la soirée");
        }
        let echecs = compte
            .echecs
            .saturating_add(u64::try_from(bilan.fautes.len()).unwrap_or(0));
        a.compte.ajouter(&compte);
        a.notes.extend(notes);
        if echecs > 0 && a.rejeu.is_none() {
            a.rejeu = Some(rejeu_de(&bilan));
        }
    }

    fn resultats(mut self) -> (Vec<ResultatControle>, String) {
        let mut v = Vec::new();
        for id in IdControle::TOUS {
            let mut a = self.acc.remove(&id).unwrap_or_default();
            let mut detail: Vec<String> = a.compte.details.clone();
            if a.compte.echecs > u64::try_from(a.compte.details.len()).unwrap_or(0) {
                detail.push(format!("{} échec(s) en tout", a.compte.echecs));
            }
            detail.append(&mut a.notes);
            let issue = if !self.plan.demande(id) {
                detail.insert(0, String::from("hors du plan"));
                IssueControle::PasExecute
            } else if !a.pannes.is_empty() {
                let mut p = a.pannes.clone();
                p.dedup();
                detail.splice(0..0, p.into_iter().take(4));
                IssueControle::PanneDeLOutil
            } else if a.compte.echecs > 0 {
                IssueControle::EchecDuJeu
            } else if let Some(r) = a.sans_objet.clone() {
                IssueControle::SansObjet(r)
            } else if !a.manquees.is_empty() {
                detail.insert(
                    0,
                    format!(
                        "des allées et venues programmées n'ont pas eu lieu : {}",
                        a.manquees.join(" ; ")
                    ),
                );
                IssueControle::PasExecute
            } else if DES_SOIREES.contains(&id) && !self.pannes_manquees.is_empty() {
                detail.insert(
                    0,
                    format!(
                        "des pannes déclarées n'ont pas eu lieu : {}",
                        self.pannes_manquees.join(" ; ")
                    ),
                );
                IssueControle::PasExecute
            } else if a.compte.evaluations == 0 {
                detail.insert(0, String::from("aucune évaluation (OUT-07)"));
                IssueControle::PasExecute
            } else {
                IssueControle::Reussi
            };
            let rejeu = match issue {
                IssueControle::EchecDuJeu => a.rejeu,
                IssueControle::Reussi
                | IssueControle::PanneDeLOutil
                | IssueControle::SansObjet(_)
                | IssueControle::PasExecute => None,
            };
            v.push(ResultatControle {
                id,
                issue,
                evaluations: a.compte.evaluations,
                detail: detail.join("\n"),
                rejeu,
            });
        }
        let joue = self.joue.join(" ; ");
        (v, joue)
    }
}

/// La moyenne d'une suite, entière.
fn moyenne(v: &[u64]) -> u64 {
    let n = u64::try_from(v.len()).unwrap_or(0);
    if n == 0 {
        return 0;
    }
    let s: u128 = v.iter().map(|x| u128::from(*x)).sum();
    u64::try_from(s.checked_div(u128::from(n)).unwrap_or(0)).unwrap_or(u64::MAX)
}

/// Un nombre en centièmes, lisible.
fn centiemes(n: u64) -> String {
    format!("{}.{:02}", n / 100, n % 100)
}

/// E15 : après un sixième de la soirée (la mise en route), le dernier tiers des
/// mesures se compare au plus haut du premier tiers. Un état qui ne fuit pas oscille
/// sous ce plafond ; un état qui fuit le dépasse, et de plus en plus. Octets vivants :
/// une moyenne finale au-delà du plafond de plus de 5 % et de plus de 1 Kio est une
/// fuite ; appareils retenus : au-delà de plus d'un demi-appareil (OUT-84, HIS-10,
/// SQ-76, PUN-15). Des seuils de départ, non mesurés sur des jeux réels.
fn juger_la_memoire(a: &mut Acc, mesures: &[Echantillon]) {
    let debut = mesures.len() / 6;
    let utiles = mesures.get(debut..).unwrap_or(&[]);
    let tiers = utiles.len() / 3;
    if tiers == 0 {
        a.noter(format!(
            "{} mesure(s) : trop peu pour une pente (il en faut neuf au moins après la mise en route)",
            mesures.len()
        ));
        return;
    }
    let premiers = utiles.get(..tiers).unwrap_or(&[]);
    let derniers = utiles
        .get(utiles.len().saturating_sub(tiers)..)
        .unwrap_or(&[]);
    let plafond_octets = premiers.iter().map(|e| e.vivants).max().unwrap_or(0);
    let octets_fin = moyenne(&derniers.iter().map(|e| e.vivants).collect::<Vec<_>>());
    // Les appareils, en centièmes.
    let plafond_appareils = premiers
        .iter()
        .map(|e| e.retenus.saturating_mul(100))
        .max()
        .unwrap_or(0);
    let appareils_fin = moyenne(
        &derniers
            .iter()
            .map(|e| e.retenus.saturating_mul(100))
            .collect::<Vec<_>>(),
    );
    let presents_fin = moyenne(
        &derniers
            .iter()
            .map(|e| e.presents.saturating_mul(100))
            .collect::<Vec<_>>(),
    );
    a.compte.evaluer();
    let hausse = octets_fin.saturating_sub(plafond_octets);
    if hausse > 1024 && hausse > plafond_octets / 20 {
        a.compte.echouer(format!(
            "les octets vivants montent : au plus {plafond_octets} au début, {octets_fin} en moyenne à la fin"
        ));
    }
    a.compte.evaluer();
    if appareils_fin > plafond_appareils.saturating_add(50) {
        a.compte.echouer(format!(
            "l'état retient de plus en plus d'appareils : au plus {} au début, {} en moyenne à la fin, pour {} présents",
            centiemes(plafond_appareils),
            centiemes(appareils_fin),
            centiemes(presents_fin)
        ));
    }
    a.noter(format!(
        "{} mesures toutes les cinq minutes ; octets vivants au plus {plafond_octets} au début, {octets_fin} à la fin ; appareils retenus au plus {} au début, {} à la fin",
        mesures.len(),
        centiemes(plafond_appareils),
        centiemes(appareils_fin)
    ));
}

/// Éprouve un module chargé (§ 8.8). La copie native, si elle est donnée, est
/// comparée au module (E9) et reprise (E14). E20 demande les octets du module livré :
/// il ne se joue que par [`eprouver_octets`], et reste ici « pas exécuté ».
pub fn eprouver(
    module: &ModuleCharge,
    natif: Option<&mut dyn FabriqueDynamique>,
    plan: &Plan,
) -> Rapport {
    let (mut rapport, _) = eprouver_et_garder(module, natif, plan);
    if plan.demande(IdControle::E20) {
        remplacer(
            &mut rapport,
            ResultatControle {
                id: IdControle::E20,
                issue: IssueControle::PasExecute,
                evaluations: 0,
                detail: String::from(
                    "E20 reprend le module livré dans un navigateur : il ne se joue que depuis ses octets (`eprouver_octets`, `soiree-epreuve`)",
                ),
                rejeu: None,
            },
        );
    }
    rapport
}

/// Remplace le résultat d'un contrôle.
fn remplacer(rapport: &mut Rapport, r: ResultatControle) {
    match rapport.controles.iter_mut().find(|c| c.id == r.id) {
        Some(c) => *c = r,
        None => rapport.controles.push(r),
    }
}

/// [`eprouver`], et les soirées que E20 reprendra.
fn eprouver_et_garder(
    module: &ModuleCharge,
    natif: Option<&mut dyn FabriqueDynamique>,
    plan: &Plan,
) -> (Rapport, Vec<moteur_commun::releve::SoireeJouee>) {
    let a_natif = natif.is_some();
    let mut e = Epreuve {
        module,
        natif: natif.map(|n| Rc::new(RefCell::new(n))),
        plan,
        acc: BTreeMap::new(),
        joue: Vec::new(),
        pannes_manquees: Vec::new(),
        vue_ecran_max: None,
        phases_univers: BTreeSet::new(),
        cas_univers: None,
        phases_declarees: None,
        invariants_declares: None,
        invariants_vus: BTreeMap::new(),
        premier_rejeu: None,
        carburant: BTreeMap::new(),
        carburant_images: Vec::new(),
        fixe: BUDGETS_E19.cout_fixe(module.octets_remis_a_zero()),
        tours: (0, 0),
        phases_des_soirees: Vec::new(),
        monde: crate::plan::MondeJoue::default(),
        soirees_e20: Vec::new(),
        vues_e21: crate::pages::VuesVues::default(),
    };
    if plan.demande(IdControle::E1) {
        e.constat();
    }
    if plan.demande(IdControle::E3) || plan.demande(IdControle::E4) {
        e.declare();
    }
    if plan.demande(IdControle::E3) {
        e.univers();
    }
    // E20 reprend au moins les soirées principales ; E21 éprouve leurs vues.
    let principales: [IdControle; 13] = [
        IdControle::E2,
        IdControle::E3,
        IdControle::E4,
        IdControle::E5,
        IdControle::E6,
        IdControle::E7,
        IdControle::E8,
        IdControle::E9,
        IdControle::E14,
        IdControle::E17,
        IdControle::E19,
        IdControle::E20,
        IdControle::E21,
    ];
    if principales.iter().any(|c| plan.demande(*c)) {
        e.soirees();
    }
    if plan.demande(IdControle::E16) || plan.demande(IdControle::E17) {
        e.pires_cas();
    }
    if [IdControle::E10, IdControle::E11, IdControle::E12]
        .iter()
        .any(|c| plan.demande(*c))
    {
        e.proprietes();
    }
    if plan.demande(IdControle::E18) {
        e.gels();
    }
    if plan.demande(IdControle::E13) {
        e.entrees_absurdes();
    }
    if plan.demande(IdControle::E15) {
        e.memoire();
    }
    if plan.demande(IdControle::E21) {
        e.pages();
    }
    let monde = e.monde;
    let soirees_e20 = std::mem::take(&mut e.soirees_e20);
    let (controles, joue) = e.resultats();
    (
        Rapport {
            module: module.empreinte(),
            plan: plan.clone(),
            controles,
            natif: a_natif,
            joue,
            compilation: None,
            monde,
            budgets: BUDGETS_E19,
            octets_remis_a_zero: Some(module.octets_remis_a_zero()),
            e20: Vec::new(),
        },
        soirees_e20,
    )
}

/// Le rapport d'un jeu dont le module ne s'est pas compilé, ou dont l'étape 1 n'a pas
/// pu commencer : chaque contrôle « pas exécuté », et l'étape 1 telle qu'elle s'est
/// arrêtée.
pub(crate) fn sans_module(plan: &Plan, compilation: crate::etapes::Compilation) -> Rapport {
    let controles = IdControle::TOUS
        .into_iter()
        .map(|id| ResultatControle {
            id,
            issue: IssueControle::PasExecute,
            evaluations: 0,
            detail: String::from("rien n'est joué : l'étape 1 s'est arrêtée avant le module"),
            rejeu: None,
        })
        .collect();
    Rapport {
        module: [0; 32],
        plan: plan.clone(),
        controles,
        natif: false,
        joue: String::from("rien : l'étape 1 s'est arrêtée avant le module"),
        compilation: Some(compilation),
        monde: crate::plan::MondeJoue::default(),
        budgets: BUDGETS_E19,
        octets_remis_a_zero: None,
        e20: Vec::new(),
    }
}

/// Éprouve les octets d'un module : le charge (E1), puis [`eprouver`]. Un module
/// refusé au chargement échoue E1, et rien d'autre ne tourne, sauf E20 s'il est
/// demandé : chaque navigateur doit refuser le module pareil.
pub fn eprouver_octets(
    moteur: &Moteur,
    octets: &[u8],
    natif: Option<&mut dyn FabriqueDynamique>,
    plan: &Plan,
) -> Rapport {
    match moteur.charger(octets) {
        Ok(m) => {
            let (mut rapport, soirees) = eprouver_et_garder(&m, natif, plan);
            if plan.demande(IdControle::E20) {
                let (r, navigateurs) =
                    crate::deux_moteurs::juger(octets, Some(&m), None, &soirees, plan);
                remplacer(&mut rapport, r);
                rapport.e20 = navigateurs;
            }
            rapport
        }
        Err(refus) => {
            let issue_e1 = match &refus {
                moteur_wasmtime::RefusModule::Panne(_) => IssueControle::PanneDeLOutil,
                moteur_wasmtime::RefusModule::Reecriture(_)
                | moteur_wasmtime::RefusModule::Abi { .. }
                | moteur_wasmtime::RefusModule::Declaration(_)
                | moteur_wasmtime::RefusModule::Faute { .. } => IssueControle::EchecDuJeu,
            };
            let controles = IdControle::TOUS
                .into_iter()
                .map(|id| {
                    if id == IdControle::E1 && plan.demande(id) {
                        ResultatControle {
                            id,
                            issue: issue_e1.clone(),
                            evaluations: 1,
                            detail: format!("le moteur refuse le module : {refus}"),
                            rejeu: None,
                        }
                    } else {
                        ResultatControle {
                            id,
                            issue: IssueControle::PasExecute,
                            evaluations: 0,
                            detail: String::from("le module est refusé au chargement (E1)"),
                            rejeu: None,
                        }
                    }
                })
                .collect();
            let mut rapport = Rapport {
                module: *blake3::hash(octets).as_bytes(),
                plan: plan.clone(),
                controles,
                natif: natif.is_some(),
                joue: String::from("rien : le module est refusé"),
                compilation: None,
                monde: crate::plan::MondeJoue::default(),
                budgets: BUDGETS_E19,
                octets_remis_a_zero: None,
                e20: Vec::new(),
            };
            if plan.demande(IdControle::E20) {
                let (r, navigateurs) =
                    crate::deux_moteurs::juger(octets, None, Some(&refus), &[], plan);
                remplacer(&mut rapport, r);
                rapport.e20 = navigateurs;
            }
            rapport
        }
    }
}

/// Rejoue un échec seul (§ 8.8, `rejouer`) : depuis la création de la partie de la
/// première entrée écartée du journal (ou depuis le point du rejeu), les entrées qui
/// la précèdent, puis elle. Rend la faute qu'elle refait, ou une panne de l'outil si
/// elle ne la refait pas.
#[expect(
    clippy::too_many_lines,
    reason = "le rejeu se lit d'un trait : la cible, l'instance, les entrées qui la précèdent, puis elle"
)]
pub fn rejouer(module: &ModuleCharge, rejeu: &Rejeu) -> Result<RapportFaute, PanneOutil> {
    let divergence = |quoi: String| PanneOutil::new(partie::CausePanne::RejeuDivergent, quoi);
    let lignes = rejeu.journal.lignes();
    let depuis = rejeu
        .point
        .as_ref()
        .map_or(0, partie::PointDeReprise::indice);
    let (indice, cible) = lignes
        .iter()
        .enumerate()
        .skip(depuis)
        .find_map(|(indice, ligne)| match ligne {
            partie::Ligne::Entree(entree)
                if matches!(entree.issue, IssueJournal::Ecartee { .. }) =>
            {
                Some((indice, entree.clone()))
            }
            partie::Ligne::Entree(_)
            | partie::Ligne::Arrivees { .. }
            | partie::Ligne::Empreinte { .. }
            | partie::Ligne::Saut { .. }
            | partie::Ligne::Pause { .. }
            | partie::Ligne::Vue { .. }
            | partie::Ligne::Partie { .. }
            | partie::Ligne::Reprise { .. } => None,
        })
        .ok_or_else(|| divergence(String::from("le journal n'a aucune entrée écartée")))?;
    let mut inst = Fabrique::neuve(&mut module.fabrique())?;
    for appel in [Appel::Bonjour, Appel::Decrire] {
        let _: Vec<u8> = inst
            .appeler(appel.sorte(), &encode(&appel), rejeu.budgets.lecture)
            .map_err(|entree| divergence(format!("{entree}")))?;
    }
    if let Some(point) = &rejeu.point {
        Instance::restaurer(&mut inst, point.instantane())?;
    } else {
        {
            let creer = rejeu.creations.get(&cible.partie).ok_or_else(|| {
                divergence(format!("pas de création pour la partie {}", cible.partie))
            })?;
            let _: Vec<u8> = inst
                .appeler(SorteAppel::Ecriture, creer, rejeu.budgets.creation)
                .map_err(|entree| divergence(format!("Creer : {entree}")))?;
        }
    }
    for ligne in lignes.iter().take(indice).skip(depuis) {
        if let partie::Ligne::Entree(entree) = ligne
            && entree.partie == cible.partie
            && !matches!(entree.issue, IssueJournal::Ecartee { .. })
        {
            let appel = Appel::Entree {
                ordre: entree.ordre,
                place: entree.place.numero(),
                entree: &entree.octets,
            };
            let rendu = Instance::appeler(
                &mut inst,
                SorteAppel::Ecriture,
                &encode(&appel),
                rejeu.budgets.entree,
            );
            if !matches!(&rendu, Ok(octets) if matches!(decoder::<Retour<'_>>(octets), Ok(Retour::Issue(_))))
            {
                return Err(divergence(format!(
                    "ligne'entrée d'ordre {} ne se rejoue pas comme au journal",
                    entree.ordre
                )));
            }
        }
    }
    let appel = Appel::Entree {
        ordre: cible.ordre,
        place: cible.place.numero(),
        entree: &cible.octets,
    };
    match Instance::appeler(
        &mut inst,
        SorteAppel::Ecriture,
        &encode(&appel),
        rejeu.budgets.entree,
    ) {
        Err(Echec::Faute(faute)) => Ok(RapportFaute {
            faute,
            lieu: LieuFaute::Entree {
                partie: cible.partie,
                ordre: cible.ordre,
                place: cible.place,
                octets: cible.octets.clone(),
            },
            carburant: Instance::carburant(&inst),
            graine: rejeu.graine,
            point: depuis,
            jusqu_a: indice.saturating_add(1),
        }),
        Ok(octets)
            if matches!(
                decoder::<Retour<'_>>(&octets),
                Ok(Retour::Issue(IssueFil::Attente))
            ) =>
        {
            Ok(RapportFaute {
                faute: Faute::Attente,
                lieu: LieuFaute::Entree {
                    partie: cible.partie,
                    ordre: cible.ordre,
                    place: cible.place,
                    octets: cible.octets.clone(),
                },
                carburant: Instance::carburant(&inst),
                graine: rejeu.graine,
                point: depuis,
                jusqu_a: indice.saturating_add(1),
            })
        }
        Ok(_) => Err(divergence(format!(
            "ligne'entrée d'ordre {} ne refait pas sa faute",
            cible.ordre
        ))),
        Err(Echec::Panne(point)) => Err(point),
    }
}
