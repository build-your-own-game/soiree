# {{nom}} : la certification propre à ce jeu

> Ce que **ce jeu-ci** doit tenir, en plus des items communs de `.soiree/CERTIFICATION.md`. L'agent du jeu écrit ces items au fil des règles (voir `AGENTS.md`).

## Format

`soiree check` lit deux sections de ce fichier, « Les items » et « Items du catalogue écartés », et refuse ce qui n'a pas leur format. Le reste du fichier est libre. Les blocs de code et les commentaires HTML ne sont pas lus.

**« Les items »** contient un seul tableau, avec ces colonnes, dans cet ordre :

| Colonne | Contenu | Obligatoire |
|---|---|---|
| `#` | l'identifiant : `J` suivi d'un numéro (`J1`, `J2`…), unique, jamais réutilisé | oui |
| `Exigence` | une phrase qui dit ce que le joueur constate, pas comment le code le fait | oui |
| `Test` | le nom du test de propriétés qui la vérifie, entre accents graves : `` `late_vote_is_ignored` ``. Le chemin du module est permis (`` `tests::late_vote_is_ignored` ``) ; seul le dernier segment est cherché. Plusieurs tests : séparés par des virgules. | oui |
| `Origine` | l'identifiant de l'item du catalogue commun dont celui-ci est une adaptation, entre accents graves : `` `VOTE-03` `` (2 à 5 majuscules, un tiret, 2 chiffres). Vide si l'item est propre au jeu. La colonne elle-même est facultative. | non |

**« Items du catalogue écartés »** est facultative. Elle liste les items du catalogue commun (`.soiree/certifications/`) qui visent une mécanique de ce jeu mais ne s'y appliquent pas, un par ligne, avec sa raison : `- VOTE-02 : on vote à main levée, il n'y a pas de vote secret.`

`soiree check` vérifie :
- chaque identifiant est unique et chaque champ obligatoire est rempli ;
- chaque test nommé existe dans le code (`fn late_vote_is_ignored`) ; `cargo test`, qu'il lance aussi, vérifie qu'il passe ;
- chaque origine et chaque item écarté a le format d'un identifiant du catalogue ; si `.soiree/certifications/` existe, il doit y être (un avertissement, pour l'instant).

Un item retiré n'est pas effacé : barre son identifiant (`~~J3~~`) et dis dans `LEARNINGS.md` ce qui le couvre maintenant. Son test n'est plus cherché.

Exemple (les blocs de code ne sont pas lus) :

```markdown
## Les items

| # | Exigence | Test | Origine |
|---|---|---|---|
| J1 | Un vote arrivé après la fin du tour ne compte pas. | `late_vote_is_ignored` | `VOTE-03` |
| J2 | Une réponse identique à celle d'un autre joueur est refusée, accents et casse compris. | `duplicate_answer_is_refused` | |
| J3 | Deux joueurs qui sautent sur la même plateforme à la même image ne se traversent pas. | `same_frame_jumps_collide`, `collision_is_symmetric` | `PHY-01` |

## Items du catalogue écartés

- VOTE-02 : on vote à main levée devant la TV, il n'y a pas de vote secret.
```

## Les items

| # | Exigence | Test | Origine |
|---|---|---|---|

## Items du catalogue écartés

