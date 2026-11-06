//! Ce que l'épreuve joue : le plan (§ 8.8), ses pannes et ses contrôles.

use std::fmt;

use jeu_abi::{Duree, Oui};

/// Un contrôle de l'épreuve (§ 7.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IdControle {
    /// Constat : réécriture acceptée, aucun import, déclaration cohérente, budgets.
    E1,
    /// Parcours : les bots vont jusqu'au bout, sans faute.
    E2,
    /// Couverture : chaque phase et chaque cas atteints.
    E3,
    /// Invariants, à chaque pas définitif ; transition à chaque entrée acceptée.
    E4,
    /// Jamais figée.
    E5,
    /// Refus sans trace.
    E6,
    /// Lecture sans trace.
    E7,
    /// Secrets.
    E8,
    /// Déterminisme.
    E9,
    /// Permutation des appareils.
    E10,
    /// Coupure transparente.
    E11,
    /// Fenêtre.
    E12,
    /// Entrées absurdes.
    E13,
    /// Reprise.
    E14,
    /// Mémoire.
    E15,
    /// Pires cas.
    E16,
    /// Flottants.
    E17,
    /// Gel.
    E18,
    /// Budgets.
    E19,
    /// Deux moteurs : les journaux de l'épreuve repris dans chaque navigateur demandé
    /// donnent, appel par appel, ce qu'ils donnent dans wasmtime (PHASE-3, § 8.3, lot
    /// 3b). Un contrôle long, hors du plan par défaut : `--navigateurs` le demande.
    E20,
}

impl IdControle {
    /// Tous les contrôles, dans l'ordre.
    pub const TOUS: [IdControle; 20] = [
        IdControle::E1,
        IdControle::E2,
        IdControle::E3,
        IdControle::E4,
        IdControle::E5,
        IdControle::E6,
        IdControle::E7,
        IdControle::E8,
        IdControle::E9,
        IdControle::E10,
        IdControle::E11,
        IdControle::E12,
        IdControle::E13,
        IdControle::E14,
        IdControle::E15,
        IdControle::E16,
        IdControle::E17,
        IdControle::E18,
        IdControle::E19,
        IdControle::E20,
    ];

    /// Les contrôles du plan par défaut, standard comme court : tous, sauf E20, un
    /// contrôle long qu'on demande par `--navigateurs` (PHASE-3, § 8.3 et P17 : un code
    /// 0 sans lui suffit pour jouer ; la publication l'attend).
    pub const DU_PLAN: [IdControle; 19] = [
        IdControle::E1,
        IdControle::E2,
        IdControle::E3,
        IdControle::E4,
        IdControle::E5,
        IdControle::E6,
        IdControle::E7,
        IdControle::E8,
        IdControle::E9,
        IdControle::E10,
        IdControle::E11,
        IdControle::E12,
        IdControle::E13,
        IdControle::E14,
        IdControle::E15,
        IdControle::E16,
        IdControle::E17,
        IdControle::E18,
        IdControle::E19,
    ];

    /// Les contrôles écrits : tous, depuis les étapes 5 et 6 du § 8.9, et E20 depuis le
    /// lot 3b de la phase 3.
    pub const ECRITS: [IdControle; 20] = IdControle::TOUS;

    /// Son nom court, « E1 » à « E20 ».
    pub const fn nom(self) -> &'static str {
        match self {
            IdControle::E1 => "E1",
            IdControle::E2 => "E2",
            IdControle::E3 => "E3",
            IdControle::E4 => "E4",
            IdControle::E5 => "E5",
            IdControle::E6 => "E6",
            IdControle::E7 => "E7",
            IdControle::E8 => "E8",
            IdControle::E9 => "E9",
            IdControle::E10 => "E10",
            IdControle::E11 => "E11",
            IdControle::E12 => "E12",
            IdControle::E13 => "E13",
            IdControle::E14 => "E14",
            IdControle::E15 => "E15",
            IdControle::E16 => "E16",
            IdControle::E17 => "E17",
            IdControle::E18 => "E18",
            IdControle::E19 => "E19",
            IdControle::E20 => "E20",
        }
    }

    /// Son titre, celui du tableau du § 7.2.
    pub const fn titre(self) -> &'static str {
        match self {
            IdControle::E1 => "Constat",
            IdControle::E2 => "Parcours",
            IdControle::E3 => "Couverture",
            IdControle::E4 => "Invariants",
            IdControle::E5 => "Jamais figée",
            IdControle::E6 => "Refus sans trace",
            IdControle::E7 => "Lecture sans trace",
            IdControle::E8 => "Secrets",
            IdControle::E9 => "Déterminisme",
            IdControle::E10 => "Permutation",
            IdControle::E11 => "Coupure transparente",
            IdControle::E12 => "Fenêtre",
            IdControle::E13 => "Entrées absurdes",
            IdControle::E14 => "Reprise",
            IdControle::E15 => "Mémoire",
            IdControle::E16 => "Pires cas",
            IdControle::E17 => "Flottants",
            IdControle::E18 => "Gel",
            IdControle::E19 => "Budgets",
            IdControle::E20 => "Deux moteurs",
        }
    }

    /// Le contrôle de ce nom, « E1 » à « E20 ».
    pub fn depuis_nom(nom: &str) -> Option<IdControle> {
        IdControle::TOUS
            .into_iter()
            .find(|c| c.nom().eq_ignore_ascii_case(nom.trim()))
    }

    /// Est-il écrit dans cette version de l'épreuve ?
    pub fn ecrit(self) -> bool {
        IdControle::ECRITS.contains(&self)
    }
}

impl fmt::Display for IdControle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.nom())
    }
}

/// Une panne que la soirée simulée fait subir (§ 7.1 ; CONTRAT, « Ce que le réel
/// impose » ; SAL-180, OUT-77, OUT-79). Chacune est choisie dans le plan, programmée à
/// un pas tiré, puis constatée : une panne qui n'a pas eu lieu ne compte pas
/// (OUT-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panne {
    /// La connexion d'un téléphone se ferme : le commun annonce son départ, puis son
    /// retour ; ses événements non confirmés repartent (SAL-15, REM-10).
    Coupure {
        /// La durée de la coupure.
        duree: Duree,
    },
    /// Un téléphone se tait sans fermer sa connexion : le commun ne le sait qu'au
    /// bout de son délai de silence (SQ-01, REM-11).
    Silence {
        /// La durée du silence.
        duree: Duree,
    },
    /// Tout ce qu'un téléphone envoie se perd sans bruit ; ses événements repartent
    /// ensuite, ses états tenus non.
    TrouNoir {
        /// La durée du trou noir.
        duree: Duree,
    },
    /// Le réseau retient les paquets d'un téléphone, puis les livre d'un coup.
    Retenu {
        /// La durée de la rétention.
        duree: Duree,
    },
    /// Le Wi-Fi d'un téléphone livre par rafales de 100 ms, toute la soirée (CONTRAT,
    /// « Le Wi-Fi d'un téléphone arrive par rafales »).
    Rafales,
    /// Un onglet se recharge : départ, ce qui n'était pas parti se perd, sa
    /// numérotation des tenus repart de zéro, retour après une poignée de main
    /// (SAL-38).
    Rechargement,
    /// Un onglet passe en arrière-plan : il relâche ce qu'il tient (REM-11), se fige,
    /// ses vues s'accumulent ; au-delà de cinq secondes le système ferme sa connexion
    /// (SAL-53, OUT-53).
    ArrierePlan {
        /// La durée en arrière-plan.
        duree: Duree,
    },
    /// Un second onglet du même téléphone joue en même temps que le premier (SAL-15,
    /// SAL-154, SQ-88).
    Jumeau,
    /// L'autorité redémarre depuis son dernier point et son journal (§ 4.5).
    RedemarrageAutorite,
    /// L'ordinateur de la soirée se fige : plus de pas, puis le rattrapage et le saut
    /// (§ 4.1, CAT-15, SQ-62).
    Gel {
        /// La durée du gel.
        duree: Duree,
    },
    /// Un téléphone part pour de bon, sans revenir, assez tôt pour que la soirée dure
    /// encore au-delà de la garde d'une place (PUN-13 : 90 s) : le chemin « place
    /// libérée » du jeu est joué là où E2, E4 à E9 et E14 jugent (PUN-37, CAT-78 ;
    /// revue de la phase 2, constat 9).
    Depart,
    /// Le réseau livre deux fois le même geste d'un téléphone, à quelques pas
    /// d'écart : l'ordinateur de la soirée le reconnaît à son identité et le compte une
    /// fois (CONTRAT : « le réseau perd, retarde, double et réordonne » ; PUN-16 ;
    /// revue de la phase 2, constat 16). Constatée au premier geste reçu deux fois.
    Doublon,
}

impl Panne {
    /// Son nom, pour le rapport.
    pub fn nom(self) -> String {
        match self {
            Panne::Coupure { duree } => format!("coupure de {} ms", duree.en_ms()),
            Panne::Silence { duree } => format!("silence de {} ms", duree.en_ms()),
            Panne::TrouNoir { duree } => format!("trou noir de {} ms", duree.en_ms()),
            Panne::Retenu { duree } => format!("paquets retenus {} ms", duree.en_ms()),
            Panne::Rafales => String::from("rafales de Wi-Fi"),
            Panne::Rechargement => String::from("rechargement d'un onglet"),
            Panne::ArrierePlan { duree } => format!("arrière-plan de {} ms", duree.en_ms()),
            Panne::Jumeau => String::from("onglet jumeau"),
            Panne::RedemarrageAutorite => String::from("redémarrage de l'autorité"),
            Panne::Gel { duree } => format!("gel de {} ms", duree.en_ms()),
            Panne::Depart => String::from("départ pour de bon"),
            Panne::Doublon => String::from("geste livré deux fois"),
        }
    }

    /// Est-ce une panne d'un téléphone ? Un jeu sans téléphone (`telephones:
    /// SansObjet`) ne peut pas la vivre : elle y est sans objet, et l'épreuve ne la
    /// programme pas (besoin 2 des jeux, `conception/BESOINS-DES-JEUX.md`).
    pub const fn du_telephone(self) -> Oui {
        match self {
            Panne::Coupure { .. }
            | Panne::Silence { .. }
            | Panne::TrouNoir { .. }
            | Panne::Retenu { .. }
            | Panne::Rafales
            | Panne::Rechargement
            | Panne::ArrierePlan { .. }
            | Panne::Jumeau
            | Panne::Depart
            | Panne::Doublon => Oui::Oui,
            Panne::RedemarrageAutorite | Panne::Gel { .. } => Oui::Non,
        }
    }

    /// La durée pendant laquelle la panne dure, quand elle en a une.
    pub const fn duree(self) -> Option<Duree> {
        match self {
            Panne::Coupure { duree }
            | Panne::Silence { duree }
            | Panne::TrouNoir { duree }
            | Panne::Retenu { duree }
            | Panne::ArrierePlan { duree }
            | Panne::Gel { duree } => Some(duree),
            Panne::Rafales
            | Panne::Rechargement
            | Panne::Jumeau
            | Panne::RedemarrageAutorite
            | Panne::Depart
            | Panne::Doublon => None,
        }
    }
}

/// Ce que la soirée simulée reproduit du réel (CONTRAT, « Ce que le réel impose » ;
/// RESTE-PHASE-1, points 20 à 22). [`Monde::REEL`] est celui de l'épreuve ;
/// [`Monde::PHASE_1`] est le monde trop sage de la phase 1, gardé pour montrer qu'un
/// fautif ne passe plus grâce au réel.
///
/// Ce qui ne se coupe pas : le réseau (canaux WebSocket et WebRTC, pertes,
/// retransmissions qui retiennent la file, accusés perdus et gestes reçus deux fois,
/// poignées de main ratées, rappel des tenus, numérotation qui repart de zéro au
/// rechargement), et la vue que chaque geste emporte, prise quand il est fait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Monde {
    /// Les bots d'un téléphone décident sur la vue que ce téléphone a reçue, et sur
    /// l'état de la définitive d'où elle vient, pas sur l'état courant (point 20).
    /// Sans vue reçue, ils ne font rien.
    pub vues_recues: Oui,
    /// Chaque téléphone a son horloge, décalée et qui dérive ; l'estimation de l'heure
    /// de l'autorité se trompe d'environ un demi-aller-retour, et `fait` porte cette
    /// erreur ; la demi-latence annoncée est mesurée, gigue comprise (point 21).
    pub horloges: Oui,
    /// Les demandes de fichiers (`Image`, `Lire`, `Telecharger`, `Rendu`) sont
    /// servies avec de vrais contenus, abîmés, trop gros et lents, et les fichiers
    /// qu'apporte un bot de l'écran arrivent au jeu (constat de Tier list).
    pub fichiers: Oui,
}

impl Monde {
    /// Le monde de l'épreuve.
    pub const REEL: Monde = Monde {
        vues_recues: Oui::Oui,
        horloges: Oui::Oui,
        fichiers: Oui::Oui,
    };

    /// Le monde de la phase 1 : bots sur l'état courant, horloges exactes, fichiers
    /// refusés.
    pub const PHASE_1: Monde = Monde {
        vues_recues: Oui::Non,
        horloges: Oui::Non,
        fichiers: Oui::Non,
    };
}

/// Ce que le monde simulé a vraiment fait, sur toutes les soirées jouées : un
/// comportement du réel qui n'a pas eu lieu n'a rien éprouvé (OUT-26). Le rapport le
/// dit, et les tests de l'épreuve le vérifient.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MondeJoue {
    /// Consultations d'un bot de téléphone sur la vue reçue.
    pub decisions_sur_vue_recue: u64,
    /// … dont celles où la vue reçue n'était pas la dernière que l'autorité avait
    /// envoyée : le bot décidait sur le passé.
    pub decisions_sur_vue_en_retard: u64,
    /// Consultations sautées : le téléphone n'avait encore reçu aucune vue.
    pub sans_vue: u64,
    /// Gestes annoncés plus tôt, ou plus tard, que l'instant vrai.
    pub faits_en_avance: u64,
    /// …
    pub faits_en_retard: u64,
    /// Le plus grand écart entre `fait` annoncé et l'instant vrai, en 1/65 536 de pas.
    pub ecart_d_horloge_max: u64,
    /// Paquets perdus puis retransmis, qui ont retenu leur file.
    pub retransmissions: u64,
    /// Tenus perdus sur le canal non fiable.
    pub tenus_perdus: u64,
    /// Gestes reçus deux fois (un accusé perdu), reconnus et comptés une fois.
    pub gestes_doubles: u64,
    /// Poignées de main ratées avant une reconnexion.
    pub poignees_ratees: u64,
    /// Accueils qui ont réaligné la numérotation des tenus d'une page.
    pub realignements: u64,
    /// Rappels réguliers d'un tenu.
    pub rappels_de_tenu: u64,
    /// Lectures de plusieurs vues d'un coup, au réveil d'une page figée.
    pub vues_en_rafale: u64,
    /// Passages en arrière-plan.
    pub arriere_plans: u64,
    /// Fichiers reçus du sélecteur.
    pub fichiers_recus: u64,
    /// … abîmés (soirée hostile).
    pub fichiers_abimes: u64,
    /// … trop gros.
    pub fichiers_gros: u64,
    /// Demandes servies par le monde.
    pub demandes_servies: u64,
    /// … et réussies.
    pub demandes_reussies: u64,
}

impl MondeJoue {
    /// Ajoute ce qu'une autre soirée a fait.
    pub(crate) fn ajouter(&mut self, o: &MondeJoue) {
        let MondeJoue {
            decisions_sur_vue_recue,
            decisions_sur_vue_en_retard,
            sans_vue,
            faits_en_avance,
            faits_en_retard,
            ecart_d_horloge_max,
            retransmissions,
            tenus_perdus,
            gestes_doubles,
            poignees_ratees,
            realignements,
            rappels_de_tenu,
            vues_en_rafale,
            arriere_plans,
            fichiers_recus,
            fichiers_abimes,
            fichiers_gros,
            demandes_servies,
            demandes_reussies,
        } = *o;
        let s = |a: &mut u64, b: u64| *a = a.saturating_add(b);
        s(&mut self.decisions_sur_vue_recue, decisions_sur_vue_recue);
        s(
            &mut self.decisions_sur_vue_en_retard,
            decisions_sur_vue_en_retard,
        );
        s(&mut self.sans_vue, sans_vue);
        s(&mut self.faits_en_avance, faits_en_avance);
        s(&mut self.faits_en_retard, faits_en_retard);
        self.ecart_d_horloge_max = self.ecart_d_horloge_max.max(ecart_d_horloge_max);
        s(&mut self.retransmissions, retransmissions);
        s(&mut self.tenus_perdus, tenus_perdus);
        s(&mut self.gestes_doubles, gestes_doubles);
        s(&mut self.poignees_ratees, poignees_ratees);
        s(&mut self.realignements, realignements);
        s(&mut self.rappels_de_tenu, rappels_de_tenu);
        s(&mut self.vues_en_rafale, vues_en_rafale);
        s(&mut self.arriere_plans, arriere_plans);
        s(&mut self.fichiers_recus, fichiers_recus);
        s(&mut self.fichiers_abimes, fichiers_abimes);
        s(&mut self.fichiers_gros, fichiers_gros);
        s(&mut self.demandes_servies, demandes_servies);
        s(&mut self.demandes_reussies, demandes_reussies);
    }

    /// Un geste annoncé fait à `annonce`, fait vraiment à `vrai` : l'erreur d'horloge.
    pub(crate) fn noter_fait(&mut self, annonce: (u64, u16), vrai: (u64, u16)) {
        let f = |(p, x): (u64, u16)| {
            i128::from(p)
                .saturating_mul(1 << 16)
                .saturating_add(i128::from(x))
        };
        let e = f(annonce).saturating_sub(f(vrai));
        if e < 0 {
            self.faits_en_avance = self.faits_en_avance.saturating_add(1);
        }
        if e > 0 {
            self.faits_en_retard = self.faits_en_retard.saturating_add(1);
        }
        let abs = u64::try_from(e.unsigned_abs()).unwrap_or(u64::MAX);
        self.ecart_d_horloge_max = self.ecart_d_horloge_max.max(abs);
    }

    /// Ses compteurs, nommés, dans l'ordre : pour le rapport.
    pub fn compteurs(&self) -> [(&'static str, u64); 19] {
        [
            ("decisions_sur_vue_recue", self.decisions_sur_vue_recue),
            (
                "decisions_sur_vue_en_retard",
                self.decisions_sur_vue_en_retard,
            ),
            ("sans_vue", self.sans_vue),
            ("faits_en_avance", self.faits_en_avance),
            ("faits_en_retard", self.faits_en_retard),
            ("ecart_d_horloge_max", self.ecart_d_horloge_max),
            ("retransmissions", self.retransmissions),
            ("tenus_perdus", self.tenus_perdus),
            ("gestes_doubles", self.gestes_doubles),
            ("poignees_ratees", self.poignees_ratees),
            ("realignements", self.realignements),
            ("rappels_de_tenu", self.rappels_de_tenu),
            ("vues_en_rafale", self.vues_en_rafale),
            ("arriere_plans", self.arriere_plans),
            ("fichiers_recus", self.fichiers_recus),
            ("fichiers_abimes", self.fichiers_abimes),
            ("fichiers_gros", self.fichiers_gros),
            ("demandes_servies", self.demandes_servies),
            ("demandes_reussies", self.demandes_reussies),
        ]
    }
}

/// Ce que l'épreuve joue (§ 8.8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// Les soirées principales, chacune avec ses pannes.
    pub soirees: u32,
    /// La durée de chaque soirée principale.
    pub duree: Duree,
    /// La graine du plan : celle de chaque soirée s'en déduit. La même graine rejoue
    /// les mêmes soirées, pas pour pas.
    pub graine: [u8; 32],
    /// Les pannes de chaque soirée principale.
    pub pannes: Vec<Panne>,
    /// Les contrôles demandés. Un périmètre réduit reste visible dans le rapport :
    /// chaque contrôle y figure, et ceux qui ne sont pas demandés sont « pas exécuté »
    /// (OUT-10).
    pub controles: Vec<IdControle>,
    /// La durée de la soirée longue de E15 : six heures au § 7.2.
    pub memoire: Duree,
    /// Le monde simulé : [`Monde::REEL`], sauf pour montrer ce qu'il change.
    pub monde: Monde,
    /// Un défaut injecté dans le commun, pour le contrôle négatif d'un contrôle qui
    /// éprouve le commun à travers le jeu : `None`, sauf dans les tests de l'épreuve, et
    /// le moteur fautif de E20 (`--moteur-fautif`).
    pub commun_fautif: Option<CommunFautif>,
    /// Les navigateurs où E20 reprend les journaux (`--navigateurs`) ; vide : E20 n'est
    /// pas joué.
    pub navigateurs: Vec<Navigateur>,
}

/// Un navigateur où E20 reprend les journaux de l'épreuve (PHASE-3, § 8.3) : c'est
/// `soiree-banc rejouer` qui le lance et le pilote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Navigateur {
    /// Chrome, par CDP.
    Chrome,
    /// Firefox, par `WebDriver` `BiDi`.
    Firefox,
    /// Safari, par `safaridriver` : « pas exécuté », avec son message, tant que l'hôte
    /// n'a pas autorisé l'automatisation à distance (PHASE-3, question 8).
    Safari,
}

impl Navigateur {
    /// Son nom sur la ligne de commande, celui que `soiree-banc` lit.
    pub const fn nom(self) -> &'static str {
        match self {
            Navigateur::Chrome => "chrome",
            Navigateur::Firefox => "firefox",
            Navigateur::Safari => "safari",
        }
    }

    /// Le navigateur de ce nom.
    pub fn depuis_nom(nom: &str) -> Option<Navigateur> {
        [Navigateur::Chrome, Navigateur::Firefox, Navigateur::Safari]
            .into_iter()
            .find(|n| n.nom().eq_ignore_ascii_case(nom.trim()))
    }
}

/// Un défaut injecté dans le commun : le contrôle négatif d'un contrôle que nul jeu ne
/// sait faire échouer, parce que c'est le commun qu'il éprouve à travers le jeu
/// (PHASE-3, § 8.3 ; AGENTS.md, « Les contrôles et leurs négatifs »).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunFautif {
    /// `etale-mal` : une arrivée reçue pendant un pas découpé entre dans ce pas au lieu
    /// d'aller au suivant (`partie::Partie::etaler_mal`). E12 doit échouer, et lui seul.
    EtaleMal,
    /// `fausser_apres(n)` : le moteur du navigateur, passé `n` appels, lit le carburant
    /// restant d'une unité de trop (PHASE-3, § 8.3, lot 3b). Seul E20 joue ce moteur :
    /// E20 doit échouer, et lui seul.
    FausserApres(u64),
}

impl Plan {
    /// Les pannes de chaque soirée principale, par défaut : toutes celles du § 7.1,
    /// mêlées (OUT-77). Des points de départ, non mesurés.
    pub fn pannes_standard() -> Vec<Panne> {
        vec![
            Panne::Coupure {
                duree: Duree::ms(1500),
            },
            Panne::Coupure {
                duree: Duree::s(20),
            },
            Panne::Silence { duree: Duree::s(6) },
            Panne::TrouNoir { duree: Duree::s(2) },
            Panne::Retenu {
                duree: Duree::ms(800),
            },
            Panne::Rafales,
            Panne::Rechargement,
            Panne::ArrierePlan { duree: Duree::s(8) },
            Panne::Jumeau,
            Panne::RedemarrageAutorite,
            Panne::Gel { duree: Duree::s(3) },
            Panne::Gel {
                duree: Duree::s(40),
            },
            Panne::Depart,
            Panne::Doublon,
        ]
    }

    /// Le plan de l'épreuve, pour `soiree-epreuve` : deux soirées de trois minutes,
    /// toutes les pannes, tous les contrôles sauf E20 (`--navigateurs`), six heures pour
    /// E15.
    pub fn standard(graine: [u8; 32]) -> Plan {
        Plan {
            soirees: 2,
            duree: Duree::s(180),
            graine,
            pannes: Plan::pannes_standard(),
            controles: IdControle::DU_PLAN.to_vec(),
            memoire: Duree::s(6 * 3600),
            monde: Monde::REEL,
            commun_fautif: None,
            navigateurs: Vec::new(),
        }
    }

    /// Un plan court, pour les tests : une soirée de deux minutes, toutes les pannes,
    /// tous les contrôles sauf E20, et une soirée longue d'une heure. Il ne remplace pas le
    /// plan standard : E15 y regarde une pente sur une heure, pas sur six.
    pub fn court(graine: [u8; 32]) -> Plan {
        Plan {
            soirees: 1,
            duree: Duree::s(120),
            graine,
            pannes: Plan::pannes_standard(),
            controles: IdControle::DU_PLAN.to_vec(),
            memoire: Duree::s(3600),
            monde: Monde::REEL,
            commun_fautif: None,
            navigateurs: Vec::new(),
        }
    }

    /// Ce contrôle est-il demandé ?
    pub fn demande(&self, c: IdControle) -> bool {
        self.controles.contains(&c)
    }
}
