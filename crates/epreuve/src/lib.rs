//! L'épreuve : la soirée simulée à vitesse machine et ses contrôles (INTERFACE.md,
//! § 7 et § 8.8), et le binaire `soiree-epreuve`.
//!
//! # Ce qu'elle joue
//!
//! Une soirée simulée, sans navigateur ni réseau réels (CAT-112, OUT-83) : la `Partie`
//! du cœur sur une horloge simulée, autant de téléphones simulés que les places le
//! permettent (plus un curieux), un organisateur simulé, et les bots du jeu qui
//! jouent depuis leur regard. Le transport simulé livre ce que le commun livrerait :
//! chaque événement une fois, dans l'ordre par appareil ; les états tenus par
//! rafales, le dernier gagnant ; avec latence, gigue et rafales de Wi-Fi
//! ([`Panne`]). Les pannes sont choisies dans le [`Plan`], mêlées, programmées à un
//! pas tiré, puis constatées avant d'être jugées (OUT-26, OUT-77, OUT-79, SAL-180).
//!
//! # Ce qu'elle contrôle
//!
//! E1 à E19 (§ 7.2), chacun avec son contrôle négatif dans `exemples/fautifs`. Voir
//! le module `epreuve` (le fichier `src/epreuve.rs`) pour ce que lit chaque
//! contrôle.
//!
//! # Ce qu'elle rend
//!
//! Un [`Rapport`] : pour chaque contrôle, réussi, échec du jeu (avec un [`Rejeu`]),
//! panne de l'outil, sans objet (avec la raison déclarée) ou pas exécuté (OUT-01,
//! OUT-02, HIS-05, HIS-06), en texte et en JSON.
//!
//! # Ce qui n'est pas mesuré
//!
//! Les durées du transport (latences, délais de silence, rafales), les budgets de
//! carburant de l'épreuve ([`BUDGETS`]), les budgets que juge E19
//! ([`BUDGET_ENTREE`], [`BUDGET_LECTURE`], [`BUDGET_IMAGE`]), et les seuils de E15
//! et de E18 sont des points de départ, choisis et non mesurés.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod budgets;
mod controles;
mod determinisme;
mod dynamique;
mod epreuve;
mod gel;
mod json;
mod ligne;
mod pires_cas;
mod plan;
mod proprietes;
mod rapport;
mod script;
mod soiree;
mod sonde;
mod telephone;
mod tirage;

pub use budgets::{BUDGET_ENTREE, BUDGET_IMAGE, BUDGET_LECTURE};
pub use dynamique::{FabriqueDynamique, InstanceDynamique};
pub use epreuve::{eprouver, eprouver_octets, rejouer};
pub use ligne::ligne_de_commande;
pub use plan::{IdControle, Panne, Plan};
pub use rapport::{ETAPES_NON_EXECUTEES, IssueControle, Rapport, Rejeu, ResultatControle};
pub use soiree::{BUDGETS, POINT_TOUS_LES, Point};
