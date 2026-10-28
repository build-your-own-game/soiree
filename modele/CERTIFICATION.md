# {{nom}} : la certification propre à ce jeu

> Ce que **ce jeu-ci** doit tenir, en plus des items communs de `.soiree/CERTIFICATION.md`. L'agent du jeu écrit ces items au fil des règles (voir `AGENTS.md`).

Les engagements, leurs références exactes de tests et les décisions du catalogue
ont leur source dans `verification.toml`. Le bloc ci-dessous est une vue générée ;
il se met à jour depuis ce contrat. Un item du catalogue sans décision explicite
reste non revu, et un rapport ancien ne vaut pas résultat courant.

<!-- soiree:guarantees:start -->

## Les items

| # | Exigence | Contrôles | Origine |
|---|---|---|---|
| J1 | Un compteur ne revient jamais à zéro par débordement. | `score_increments_without_wrapping` |  |
| J2 | Un joueur absent ne marque pas et retrouve son score au retour. | `absent_players_cannot_score_and_return_keeps_score` |  |
| J3 | Un replay restitue tout l’état et refuse une empreinte différente. | `a_versioned_journal_replays_the_complete_state` |  |
| J5 | Même des arrivées simultanées ne dépassent pas la capacité du jeu. | `capacity_is_enforced_in_the_simulation`, `simultaneous_arrivals_never_exceed_capacity` |  |
| J4 | Coupures et duplications réseau ne dupliquent ni joueur ni point. | `whole_host_is_deterministic_across_thousands_of_network_seeds` |  |

## Décisions du catalogue


<!-- soiree:guarantees:end -->
