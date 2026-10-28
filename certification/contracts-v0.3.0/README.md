# Validation des contrats 0.3.0 — avant publication

Le périmètre de code est satisfait pour le modèle, les trois jeux, salon, remote et le catalogue. Les campagnes complètes des jeux ne rapportent ni échec produit ni panne du contrôleur. Elles restent incomplètes sur les critères non exercés ou à examiner. Aucun certificat global n’est déduit de ces passages séparés.

## Contrôles exécutés

108 tests Rust du CLI, 5 tests du noyau, 38 tests Node passent. Verus vérifie 11 fonctions du noyau réellement utilisé par le CLI ; trois mutations qui brisent ses obligations sont rejetées. La preuve suppose des faits fidèlement construits par les adaptateurs.

| Projet | Vérifications du code, secondes | Boucle statique, secondes |
|---|---:|---:|
| modele | 4.253 | — |
| punchline | 12.847 | 1.309 |
| sidequest | 33.622 | 3.317 |
| tier-list | 5.415 | 0.936 |
| salon | 3.421 | — |
| remote | 6.444 | — |
| catalogue | 1.123 | — |

Mesures locales, caches de compilation déjà présents. La première vérification du modèle neuf a pris 93,4 s ; son passage chaud prend environ 4 s. Les campagnes de chaos durent volontairement plusieurs minutes.

## Portée navigateur

| Passage | Contrôleur | Échec produit ou infrastructure | Décision |
|---|---:|---:|---|
| punchline complet | 0 | 0 | non attesté / revue sur certains critères |
| sidequest complet | 0 | 0 | non attesté / revue sur certains critères |
| tier-list-full complet | 0 | 0 | non attesté / revue sur certains critères |
| modele-ui-panic | 0 | 0 | périmètre demandé satisfait |
| punchline-ui-panic | 0 | 0 | périmètre demandé satisfait |
| sidequest-ui-panic | 0 | 0 | périmètre demandé satisfait |

Le modèle neuf est testé en loopback pour U1/C10 : le pare-feu bloque son nouvel exécutable sur le LAN. Le contrôle réel LAN a correctement produit H1/infrastructure en 4,7 s et aucune fausse erreur U1/C10. Le pare-feu n’a pas été modifié.

Les sept exigences communes sans liaison restent Unbound ; les 97 décisions de catalogue ne sont pas inventées. T2 n’a pas encore d’observation positive du harnais. Les appréciations de clarté et de jouabilité restent à revoir. La salle de 71 places de Sidequest n’est pas exercée par huit téléphones ; les exclusions de layout ne constituent pas une preuve visuelle.

## Provenance et conservation

Les trois anciens ensembles `rapport.json`, `rapport.md`, `performance.json` correspondent octet pour octet au snapshot précédant cette implémentation. Les nouvelles mesures portent un nom `harness-performance*.json`. Les rapports complets des jeux sont conservés dans leurs dépôts avec le suffixe `-full` ; les observations ciblées plus récentes gardent leur propre sélection.

Les campagnes complètes Punchline/Sidequest ont précédé la dernière séparation des destinations de mesures et la clarification des consignes. Le code du jeu et le harnais exécuté n’ont pas changé ; leurs identités de binaire sont conservées sans réétiquetage. Les contrôles du code et les passages ciblés finaux portent l’identité du binaire final.

Détails machine : [validation.json](validation.json). Preuve : [policy-proof.json](policy-proof.json). Modèle local : [rapport](modele/verification.md). Ces observations ont été produites avant le commit et l’étiquette de publication ; la publication ne leur attribue pas une nouvelle provenance.
