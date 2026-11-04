# Mesures de la phase 1

> **Statut.** Mesures du commun de la phase 1 et des trois jeux reconstruits, pour aider le mainteneur à fixer les budgets dans [`CONTRAT.md`](../CONTRAT.md) (question 12 de [`REPONSES-PROVISOIRES.md`](REPONSES-PROVISOIRES.md)). **Ce sont des propositions, pas des budgets.** Les budgets que juge E19 aujourd'hui (`epreuve::BUDGET_ENTREE`, `BUDGET_LECTURE`, `BUDGET_IMAGE`) restent des points de départ choisis, non mesurés.
>
> **Date et machine.** Toutes les mesures du 2026-11-04, entre 13 h 50 et 14 h 45 (GMT), sur la même machine : MacBook Apple M5, 10 cœurs (4 de performance, 6 d'efficacité), 16 Go, macOS 26.6.2 ; rustc 1.99.1, cible `aarch64-apple-darwin` pour le natif ; wasmtime 50.0.1 (Cranelift). Arbres de travail non committés, branche `refonte` de chaque dépôt, à cette date. D'autres agents travaillaient sur la machine : une durée peut porter leur charge.
>
> **Ce qui ne bouge pas d'une machine à l'autre.** Le carburant est compté par la réécriture (`reecriture`, § 3.6) : le même module joué de la même façon brûle le même carburant partout. Les durées, elles, ne valent que pour cette machine, et pour wasmtime en natif : la partie qui fait foi tournera dans un navigateur, dont rien n'est mesuré ici.

## Les modules mesurés

L'empreinte est celle que donne le rapport de l'épreuve (le module livré, avant réécriture).

| Module | Empreinte | Octets livrés | Octets réécrits | Mémoire (pages de 64 Kio) | Chargement¹ |
|---|---|---|---|---|---|
| `devine` | `77baf6fd…` | 419 878 | 587 629 | 36 à 64 | 216 ms |
| Punchline | `b72dcc1c…` | 777 383 | 1 113 609 | 38 à 128 | 403 ms |
| Sidequest | `e78c32ba…` | 636 305 | 930 351 | 52 à 256 | 339 ms |
| Tier list | `4c42e866…` | 739 402 | 1 105 633 | 405 à 4 096 | 415 ms |

¹ `Moteur::charger` : validation, réécriture et compilation Cranelift, mesuré deux fois de suite, la première valeur donnée ; la seconde est plus courte de 1 à 15 %. Chaque module est compilé avec le profil `module` et n'importe rien (section d'imports vide, constaté par E1 et par un lecteur de sections écrit pour l'occasion).

## Le carburant, par sorte d'appel

Plan standard de l'épreuve (deux soirées de 180 s, une soirée longue de 6 h, graine `5e…5e`, onze pannes), avec la copie native de chaque jeu ; chiffres de E19 et E1 du rapport. « Image » : la somme du carburant de `(fenêtre + 2)` pas consécutifs ; la fenêtre vaut 0 pour les quatre jeux.

| Appel | `devine` | Punchline | Sidequest | Tier list |
|---|---|---|---|---|
| `Entree`, médiane | 5 736 | 107 243 | 63 332 | 20 794 |
| `Entree`, 99ᵉ centile | 24 944 | 381 118 | 119 702 | 183 070 |
| `Entree`, maximum | 25 456 | 586 859 | 180 079 | 3 043 210 |
| `Regarder`, 99ᵉ centile | 40 402 | 263 275 | 520 120 | 916 614 |
| `Etat`, 99ᵉ centile | 15 430 | 206 719 | 10 260 | 158 676 |
| `Valider`, 99ᵉ centile | 16 949 | 321 587 | 37 184 | 2 079 |
| `Canonique`, maximum | 61 388 | 319 425 | 213 907 | 13 947 607 |
| `Creer`, maximum | 4 287 | 298 155 | 532 061 | 102 358 |
| Image, 99ᵉ centile | 24 840 | 379 412 | 259 157 | 64 896 |
| Image, maximum | 801 207 | 13 607 972 | 749 958 | 3 043 210 |
| Vue de l'écran du pire cas le plus lourd (E16) | 16 041 | 221 784 | 3 362 249 | 41 643 580 |

Les budgets de E19, pour comparaison : 2²¹ = 2 097 152 par entrée et par lecture (99ᵉ centile), 2²³ = 8 388 608 par image (chaque image, la plus lourde comprise). Avec eux, **les trois jeux échouent E19 sur le plan standard** (voir « L'épreuve des trois jeux ») : Punchline par deux images de 2 pas au-delà de 2²³ (jusqu'à 13 607 972), Sidequest et Tier list par la vue de l'écran de leurs pires cas au-delà du budget d'une lecture.

## La durée d'un appel

Mesurée sur des instances du module réécrit, dans wasmtime, chronométrées autour de chaque `Instance::appeler`, `instantane`, `restaurer` et `empreinte`. Méthode : un programme jetable, hors de tout dépôt, passe à `epreuve::eprouver` une fabrique d'instances wasmtime chronométrées **à la place de la copie native** ; E9 et E14 y rejouent alors la soirée entrée par entrée. Plan court (une soirée de 120 s), contrôles E9, E14 et E19, graine `5e…5e`, build `--release`. Les durées comprennent le passage de l'appel au fil qui porte l'instance, la remise à zéro des zones de pile, de boîte, de brouillon et de panique (`preparer`, à chaque appel) et la copie de la réponse.

| | `devine` | Punchline | Sidequest | Tier list |
|---|---|---|---|---|
| Appels chronométrés | 2 976 | 8 536 | 89 561 | 4 102 |
| `Entree`, médiane | 23,6 µs | 24,6 µs | 33,0 µs | 199,8 µs |
| `Entree`, 99ᵉ centile | 48,2 µs | 41,0 µs | 42,5 µs | 303,7 µs |
| `Entree`, maximum | 55,0 µs | 58,5 µs | 143,0 µs | 332,1 µs |
| `Regarder`, 99ᵉ centile | 27,2 µs | 35,6 µs | 53,3 µs | 314,6 µs |
| `Etat`, médiane | 17,2 µs | 18,5 µs | 24,2 µs | 191,3 µs |
| `Canonique`, 99ᵉ centile | 27,6 µs | 39,6 µs | 40,0 µs | 440,1 µs |
| Premier `Bonjour` | 108 µs | 112 µs | 172 µs | 1 311 µs |
| Droite des moindres carrés² | 21,6 µs + ε | 20,9 µs + 0,040 ns/u | 28,1 µs + 0,041 ns/u | 212,0 µs + 0,045 ns/u |

Le programme a tourné deux fois sur chaque module, à quelques minutes d'écart : le tableau donne le premier passage, la droite ci-dessous et le rejeu le second ; d'un passage à l'autre, les médianes ont bougé de moins de 5 %.

² Durée d'un appel en fonction de son carburant, sur tous les appels sauf le premier `Bonjour` (R² : 0,00 pour `devine`, dont le carburant varie trop peu ; 0,54, 0,60 et 0,29). « u » : une unité de carburant.

Ce qu'on en lit :

- **Un appel coûte d'abord un fixe** : environ 20 à 30 µs sur cette machine, quel que soit son carburant, pour `devine`, Punchline et Sidequest. Le carburant ne coûte qu'environ 0,04 ns par unité : les 586 859 unités de la plus lourde entrée de Punchline font de l'ordre de 25 µs de plus.
- **Tier list paie un fixe dix fois plus lourd** : environ 200 µs par appel, même pour un `Valider` de 1 631 unités. Tier list déclare un brouillon de 24 Mio (`crates/tier-list/src/lib.rs`), que le moteur remet à zéro avant chaque appel. La corrélation est constatée ; la part de cette remise à zéro dans les 200 µs n'a pas été mesurée seule. Ce coût n'entre pas dans le carburant : **un budget de carburant ne borne pas, à lui seul, la durée d'un appel.**
- Une extrapolation, non mesurée : à 0,045 ns par unité, la vue du pire cas de Tier list (41,6 millions d'unités) ferait environ 2 ms, et 2²³ unités environ 0,4 ms de calcul. Le budget d'une image du contrat, 33 ms, laisse donc sur cette machine et dans wasmtime une marge de deux ordres de grandeur au-dessus des budgets de carburant actuels. Rien ne dit ce qu'il en sera dans un navigateur ni sur l'ordinateur de la soirée.

## Les points de reprise

Un instantané est la mémoire entière du module et ses globales (D23) : sa taille est le nombre de pages de la mémoire au moment où il est pris, fois 64 Kio. L'épreuve en prend un tous les 600 pas (`epreuve::POINT_TOUS_LES`, 10 s de partie). Même méthode et même plan que ci-dessus.

| | `devine` | Punchline | Sidequest | Tier list |
|---|---|---|---|---|
| Taille d'un instantané | 37 pages, 2,3 Mio | 39 pages, 2,4 Mio | 53 pages, 3,3 Mio | 406 à 407 pages, 25,4 Mio |
| Prendre un instantané, médiane | 41 µs | 47 µs | 71 µs | 523 µs |
| Prendre un instantané, maximum | 147 µs | 164 µs | 203 µs | 1 687 µs |
| Restaurer, médiane (5 mesures) | 54 µs | 57 µs | 73 µs | 520 µs |
| Empreinte de l'état, médiane | 115 µs | 166 µs | 114 µs | 140 µs |

Octets vivants mesurés par E15 sur la soirée longue de 6 h (plan standard, au plus, au début puis à la fin) : `devine` 1 616 puis 1 601 ; Punchline 12 857 puis 7 012 ; Sidequest 3 809 puis 2 318 ; Tier list 335 282 puis 297 522. Un instantané est donc mille fois plus gros que ce que le jeu garde vivant : il emporte toute la mémoire, brouillon et zones comprises. C'est la question 1 du § 13 (les gros états), que la réponse provisoire laisse ouverte : à 25 Mio par point, un point toutes les 10 s fait 150 Mio par minute de partie si l'on garde tout.

## Le rejeu

La plus longue suite d'écritures (`Creer` puis les `Entree`) vue par une instance chronométrée jamais restaurée, rejouée telle quelle sur une instance neuve, avec les mêmes budgets. Aucune n'a été refusée ni n'a fauté.

| | `devine` | Punchline | Sidequest | Tier list |
|---|---|---|---|---|
| Écritures rejouées | 262 | 347 | 3 502 | 301 |
| Durée du rejeu | 6,4 ms | 9,5 ms | 124 ms | 64 ms |
| Par écriture | 24,4 µs | 27,2 µs | 35,5 µs | 212,7 µs |

Une reprise rejoue au plus 600 pas de journal depuis son point. Sidequest, qui reçoit le temps à chaque pas, y a le plus d'entrées : 600 pas d'entrées seules feraient de l'ordre de 21 ms ; avec ses gestes, davantage. Ce chiffre est déduit, pas mesuré.

## La durée de l'épreuve et des commandes

Depuis zéro (`cargo clean` dans chaque dépôt), `CARGO_BUILD_JOBS=4`, une commande lourde à la fois. Les durées de compilation dépendent aussi de la charge des autres agents.

| Commande | `soiree` | Punchline | Sidequest | Tier list |
|---|---|---|---|---|
| `cargo build --workspace`, à froid | 81 s | — | — | — |
| `cargo test --workspace`, à froid | 150 s³ | 104 s | 103 s | 106 s |
| `cargo clippy --all-targets -- -D warnings` | 31 s | 17 s | 16 s | 17 s |
| `cargo clippy … --target wasm32-unknown-unknown` | — | 14 s | 17 s | 20 s |
| le ou les modules, profil `module` | 23 s (23 modules) | 10 s | 10 s | 15 s |
| l'épreuve en `--release` | 60 s | 58 s | 60 s | 60 s |
| **l'épreuve, plan standard** | 27 s (`devine`) | 50 s | **452 s** | 134 s |
| mémoire résidente maximale de l'épreuve | 608 Mo | 552 Mo | **2,68 Go** | 709 Mo |

³ Dont 110 s pour `epreuve/tests/epreuve` (le plan court sur chaque fautif). Le premier `cargo test` s'est arrêté au premier échec (97 s) ; le chiffre est celui de `--no-fail-fast`, compilation comprise.

Sur le plan standard, un fautif prend de 40 à 58 s, sauf `declaration-incoherente`, refusé au chargement. Sidequest est le seul jeu dont l'épreuve standard dépasse sept minutes, et dont la mémoire résidente dépasse 2 Go : 94 709 lignes de journal par soirée principale, contre 1 313 à 12 162 pour les autres.

## Propositions pour le mainteneur

Elles se discutent ; aucune n'est appliquée.

1. **Ne pas juger le budget d'une image en carburant seul.** Le fixe d'un appel (20 à 200 µs ici) et la remise à zéro des zones n'entrent pas dans le carburant. Une image se jugerait mieux en carburant **et** en nombre d'appels, ou par un coût fixe compté par appel et par Kio de zones.
2. **Relever les budgets de carburant, ou les dire provisoires dans le rapport.** À 0,04 ns par unité dans wasmtime sur cette machine, 2²³ unités font moins d'une milliseconde, quand le contrat donne 33 ms à une image. Le facteur entre wasmtime et le navigateur de l'ordinateur de la soirée n'est pas mesuré ; il faudrait le mesurer avant de fixer quoi que ce soit.
3. **Séparer le budget d'une vue de pire cas du budget d'une lecture courante** (question déjà ouverte par Tier list, `games/tier-list/BESOINS-COMMUN.md`, § 13) : un pire cas est construit pour être le plus lourd.
4. **Décider des gros états (question 1 du § 13)** avant le navigateur : 25 Mio par point de reprise pour Tier list.
5. **Borner la durée de l'épreuve standard**, ou dire ce qu'elle doit coûter : 27 s à 452 s selon le jeu, sur cette machine.
