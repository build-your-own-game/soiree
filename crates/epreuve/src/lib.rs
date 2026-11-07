//! L'épreuve : la soirée simulée à vitesse machine et ses contrôles (INTERFACE.md,
//! § 7 et § 8.8), et le binaire `soiree-epreuve`.
//!
//! # Ce qu'elle joue
//!
//! Une soirée simulée, sans navigateur ni réseau réels (CAT-112, OUT-83) : la `Partie`
//! du cœur sur une horloge simulée, autant de téléphones simulés que les places le
//! permettent (plus un curieux), un organisateur simulé, et les bots du jeu qui
//! jouent depuis la vue que leur téléphone a reçue. Le transport simulé livre ce que
//! le commun livrerait, sur un réseau qui ressemble au réel ([`Monde`]) : chaque
//! événement une fois, dans l'ordre par appareil, même renvoyé ; les états tenus par
//! rafales, perdus ou réordonnés, le dernier gagnant, redits toutes les 250 ms ; avec
//! latence, gigue, rafales de Wi-Fi, files WebSocket qu'une perte retient, poignées de
//! main ratées, une horloge par téléphone, et des fichiers servis avec de vrais
//! contenus ([`Panne`]). Les pannes sont choisies dans le [`Plan`], mêlées,
//! programmées à un pas tiré, puis constatées avant d'être jugées (OUT-26, OUT-77,
//! OUT-79, SAL-180) ; ce que le monde a fait se lit dans [`MondeJoue`].
//!
//! # Ce qu'elle contrôle
//!
//! E1 à E19 (§ 7.2), et E21, « Pages » (PHASE-3, § 8.3, lot 6), chacun avec son
//! contrôle négatif dans `exemples/fautifs`. Voir le module `epreuve` (le fichier
//! `src/epreuve.rs`) pour ce que lit chaque contrôle. Et E20, « Deux moteurs » (PHASE-3, § 8.3, lot 3b), un contrôle long que
//! `--navigateurs` demande : les journaux de l'épreuve repris dans de vrais navigateurs
//! par `soiree-banc rejouer`, et comparés appel par appel au relevé de wasmtime ; son
//! contrôle négatif est un moteur fautif ([`CommunFautif::FausserApres`]).
//!
//! # Ce qu'elle rend
//!
//! Un [`Rapport`] : pour chaque contrôle, réussi, échec du jeu (avec un [`Rejeu`]),
//! panne de l'outil, sans objet (avec la raison déclarée) ou pas exécuté (OUT-01,
//! OUT-02, HIS-05, HIS-06), en texte et en JSON.
//!
//! # Ce qui n'est pas mesuré
//!
//! Les durées et les proportions du transport (latences, délais de silence, rafales,
//! pertes, poignées ratées, décalage et dérive des horloges, délais des demandes de
//! fichiers), les plafonds de carburant de l'épreuve ([`BUDGETS`]), et les seuils de
//! E15 et de E18 sont des points de départ, choisis et non mesurés. Les budgets que
//! juge E19 ([`BUDGETS_E19`]) sont dérivés du temps d'une image du CONTRAT, par des
//! mesures de wasmtime sur une seule machine et des marges choisies : provisoires,
//! et dits tels dans le rapport.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod budgets;
mod catalogue;
mod controles;
mod determinisme;
mod deux_moteurs;
mod dynamique;
mod ecriture;
mod epreuve;
mod etapes;
mod fichiers;
mod gel;
mod json;
mod ligne;
mod pages;
mod pires_cas;
mod plan;
mod polices;
mod proprietes;
mod rapport;
mod script;
mod soiree;
mod sonde;
mod sources;
mod telephone;
mod tirage;

pub use budgets::{
    ARRIERE_PAS, BUDGETS_E19, BudgetsE19, FIXE_NS, IMAGE_COURANTE_NS, IMAGE_MAX_NS, MARGE_MACHINE,
    MARGE_NAVIGATEUR, MARGE_TELEPHONE, PART_DES_ENTREES_POUR_CENT, PART_DU_JEU_POUR_CENT,
    PS_PAR_KIO_REMIS_A_ZERO, PS_PAR_UNITE,
};
pub use dynamique::{FabriqueDynamique, InstanceDynamique};
pub use epreuve::{eprouver, eprouver_octets, rejouer};
pub use etapes::{Compilation, Etape, Exigence, IssueEtape, Regle, ResultatEtape};
pub use ligne::ligne_de_commande;
pub use plan::{CommunFautif, IdControle, Monde, MondeJoue, Navigateur, Panne, Plan};
pub use rapport::{IssueControle, Rapport, Rejeu, ResultatControle};
pub use soiree::{BUDGETS, POINT_TOUS_LES, Point};
