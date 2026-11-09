# Mesures de la phase 3 : les navigateurs

> **Statut.** Les mesures 1, 4 et 6 de « Ce que la phase 3 devra mesurer » ([`RESTE-PHASE-2.md`](RESTE-PHASE-2.md)) : le carburant et le temps, le chargement d'un module, la partie qui fait foi en arrière-plan, dans les navigateurs de la machine de l'hôte. Le harnais et la façon de le relancer sont dans [`../mesures/navigateur/README.md`](../mesures/navigateur/README.md). **La fin du fichier est une proposition de budgets, pas des budgets** : le mainteneur les fixe dans `CONTRAT.md` (REPONSES-PROVISOIRES, question 12).
>
> **Date et machine.** Le 2026-11-05, de 06 h 46 à 07 h 22 (UTC), sur la machine de l'hôte : MacBook Apple M5, 10 cœurs (4 de performance, 6 d'efficacité), 16 Go, macOS 26.6.2 (25G83). rustc 1.99 (la chaîne du commun), wasmtime 50.0.1 (Cranelift, sans compilation parallèle : les fonctionnalités de `moteur-wasmtime`). **Google Chrome 156.0.8078.80**, sans interface (`--headless=new`) ; **Mozilla Firefox 158.0.1**, sans interface (`--headless`) ; **Safari 27.2 : non mesuré** (voir plus bas). Pour comparaison seulement, hors navigateur : node v26.12.1 (V8 14.6.202.34), deno 2.9.7 (V8 15.0.245.2), bun 1.3.14 (JavaScriptCore). D'autres agents travaillaient sur la machine : charge moyenne sur une minute de 2,3 à 6,6 pendant les mesures, notée avant et après chaque lancement dans les résultats (OUT-89, HIS-08).
>
> **Modules.** Compilés au profil `module` depuis les arbres de travail de la branche `refonte` à cette date, puis réécrits par `reecriture` (`VERSION` 3, `ReglagesMoteur::PHASE_1`) exactement comme `moteur-wasmtime` les charge : le module réécrit du banc est celui de `ModuleCharge::reecrit`, vérifié égal à celui d'une réécriture seule. Empreintes BLAKE3 des modules livrés : `devine` `ac0c95aa…`, `ardoise` `2964ab9c…`, Punchline `ad1b7b04…`, Sidequest `8112385d…`, Tier list `73552646…`.

## La méthode, en bref

Une même **trace** se rejoue dans wasmtime et dans chaque navigateur, appel par appel, dans le même ordre (le harnais : `mesures/navigateur/capture`). Elle contient, pour chaque module : deux mille `Bonjour` sur une instance nue ; chaque pire cas (`PireCas`) suivi de sept fois sa vue de l'écran, son état et ses invariants ; une soirée du plan court (graine `5e…5e`) rejouée entrée par entrée par E9 et E14, comme `epreuve/examples/etalonner.rs`. En tout 116 775 appels, avec leurs instantanés et restaurations.

- **Dans wasmtime**, chaque appel est chronométré autour de `Instance::appeler` d'une `InstanceWasm` : le passage de l'appel au fil de l'instance, la remise à zéro des zones, l'appel, la copie du retour. C'est ce que la partie paie aujourd'hui. Rejoué deux fois, à 06 h 46.
- **Dans le navigateur**, la page du banc refait le protocole du moteur (`page/banc-coeur.js`, `InstanceJs`, recopié de `interieur.rs`) : budget, profondeur et drapeaux posés, les quatre zones remises à zéro, `soiree_boite`, copie de l'appel, `soiree_appel`, copie du retour ; chronométré de la même façon, sur le fil de la page. Rejoué deux fois. Un navigateur neuf (profil neuf) par module. Les pages sont isolées (COOP et COEP) : granularité de `performance.now()` mesurée à **5 µs dans Chrome, 20 µs dans Firefox** ; une durée d'un appel est donc connue à 5 ou 20 µs près. Les mesures du coût fixe se font aussi d'un bloc de deux mille appels, pour passer sous cette granularité.
- Sauf mention, les chiffres sont ceux de la **seconde passe** (après la mise en route du moteur : compilation paresseuse, montée de niveau) et du **premier tour** (06 h 55 à 06 h 57). Trois autres tours de Chrome et de Firefox (06 h 56, 07 h 20, 07 h 21) disent ce qui bouge d'un lancement à l'autre.
- **Le carburant est le même partout.** Chaque appel rejoué vérifie son carburant, son issue et une empreinte de son retour contre l'enregistrement : **aucun écart**, sur les 116 775 appels, dans les deux passes, dans wasmtime, Chrome (V8), Firefox (SpiderMonkey), et hors navigateur dans node, deno (V8) et bun (JavaScriptCore). C'est une vérification de D32 dans les vrais moteurs : le module réécrit brûle le même carburant et rend les mêmes octets dans quatre compilateurs différents.

## Safari : non mesuré

`safaridriver` a refusé la session. Le premier essai (vers 06 h 15) : « You must enable 'Allow remote automation' in the Developer section of Safari Settings to control Safari via WebDriver. » Les suivants (de 06 h 48 à 06 h 55, puis à 07 h 17) : « The session timed out while connecting to a Safari instance […] Finding or launching a compatible local Safari and waiting for its RWIApplication to appear and become ready. » L'automatisation à distance n'est pas autorisée (`AllowRemoteAutomation` absent des réglages de Safari) ; c'est un réglage de sécurité, à l'hôte de le changer. Le banc sait piloter Safari (`navigateurs.mjs`, `ouvrirSafari`) : une fois le réglage coché, `node mesurer.mjs --navigateurs safari` et `node cadence.mjs --navigateur safari` le mesurent.

bun porte JavaScriptCore, le moteur de Safari : ses chiffres sont donnés à part, pour ordre de grandeur. Ce n'est ni la même version du moteur, ni ses réglages, ni Safari, ni iOS (où toute page tourne dans WebKit) : **ils ne remplacent pas une mesure dans Safari**.

## Mesure 1 : le carburant et le temps

### Le coût fixe d'un appel

« Bonjour » : la durée d'un `Bonjour` (environ 3 100 unités), chronométré un par un ; médiane / 99ᵉ centile / maximum des deux mille de la trace, en µs. « Bloc » : deux mille `Bonjour` chronométrés d'un seul bloc, durée moyenne d'un appel, médiane de trois blocs, avec la remise à zéro des zones puis sans (le banc seul sait la sauter ; le moteur la fait toujours), en µs.

| Module | Zones (Kio) | wasmtime : Bonjour | Chrome : Bonjour | Chrome : bloc, avec / sans remise à zéro | Firefox : Bonjour | Firefox : bloc, avec / sans remise à zéro |
| --- | --- | --- | --- | --- | --- | --- |
| devine | 2 116 | 17,0 / 19,1 / 22,4 | 15 / 20 / 110 | 13,46 / 0,29 | 20 / 20 / 80 | 14,35 / 0,42 |
| ardoise | 2 116 | 17,0 / 19,9 / 26,0 | 15 / 20 / 105 | 13,53 / 0,34 | 20 / 20 / 40 | 14,49 / 0,51 |
| Punchline | 2 116 | 16,3 / 18,8 / 27,3 | 15 / 20 / 595 | 13,38 / 0,32 | 20 / 20 / 100 | 13,63 / 0,42 |
| Sidequest | 3 140 | 24,1 / 28,2 / 34,0 | 20 / 30 / 45 | 19,79 / 0,32 | 20 / 40 / 160 | 20,40 / 0,45 |
| Tier list | 25 668 | 168,6 / 299,6 / 631,3 | 160 / 200 / 1 040 | 162,34 / 0,35 | 160 / 200 / 480 | 179,65 / 0,47 |

La remise à zéro d'un Kio de zones, en ns par Kio. wasmtime : `fill(0)` d'un tampon de la taille des zones, médiane de quinze. Navigateurs : (bloc avec − bloc sans remise à zéro) ; puis `fill(0)` seul d'un tampon de la taille des zones, médiane de quinze, à la granularité de l'horloge près.

| Module | wasmtime | Chrome | Firefox | bun (hors navigateur) |
| --- | --- | --- | --- | --- |
| devine | 6,12 | 6,22 ; 7,09 | 6,58 ; 9,45 | 6,47 ; 6,10 |
| ardoise | 6,12 | 6,24 ; 7,09 | 6,61 ; 9,45 | 6,50 ; 6,10 |
| Punchline | 6,12 | 6,17 ; 7,09 | 6,24 ; 9,45 | 6,54 ; 6,12 |
| Sidequest | 6,12 | 6,20 ; 6,37 | 6,35 ; 6,37 | 6,33 ; 6,12 |
| Tier list | 6,77 | 6,31 ; 8,38 | 6,98 ; 7,79 | 6,52 ; 6,94 |

Ce qu'on en lit :

- **Dans le navigateur, le coût fixe d'un appel est la remise à zéro des zones, et presque rien d'autre** : 0,29 à 0,51 µs par appel sans elle (le `Bonjour` lui-même compris), 6,2 à 7,0 ns par Kio avec elle, comme dans wasmtime (6,1 à 6,8). Les 4 à 6 µs de reste que wasmtime paie (MESURES-PHASE-1, « Phase 2 ») sont le passage au fil de l'instance ; le navigateur n'en a pas.
- Chez Tier list, chaque appel coûte donc environ 160 à 180 µs avant toute instruction du jeu, dans les deux navigateurs : 25 Mio remis à zéro (point 10 de RESTE-PHASE-2). Un arriéré de 225 entrées (point 9) en coûterait environ 37 ms : plus qu'une image, à lui seul. Chez `devine`, environ 3 ms.
- Les maxima un par un (jusqu'à 1 040 µs) sont des pauses du navigateur ou de la machine tombées sur un appel, pas le module : voir « Les pauses » ci-dessous.

### La durée d'une unité de carburant

Pour chaque appel d'au moins un million d'unités : (durée − coût fixe du même moteur) / carburant, en picosecondes. Le coût fixe d'un moteur est la médiane de ses deux mille `Bonjour`. « n ; médiane ; 99ᵉ centile ; maximum ». `devine` et `ardoise` n'ont aucun appel d'un million d'unités ; Punchline et Sidequest n'en ont que dans leurs pires cas.

| Module | Appels | wasmtime | Chrome | Firefox | bun (hors navigateur) |
| --- | --- | --- | --- | --- | --- |
| Punchline | pires cas | 7 ; 24 ; 29 ; 29 | 7 ; 28 ; 31 ; 31 | 7 ; 66 ; 69 ; 69 | 7 ; 53 ; 97 ; 97 |
| Sidequest | pires cas | 41 ; 36 ; 40 ; 40 | 41 ; 36 ; 41 ; 41 | 41 ; 63 ; 104 ; 104 | 41 ; 64 ; 79 ; 79 |
| Tier list | pires cas | 67 ; 26 ; 34 ; 34 | 67 ; 29 ; 93 ; 93 | 67 ; 80 ; 336 ; 336 | 67 ; 59 ; 193 ; 193 |
| Tier list | soirée | 280 ; 56 ; 84 ; 105 | 280 ; 37 ; 67 ; 131 | 280 ; 62 ; 105 ; 120 | 280 ; 49 ; 80 ; 92 |

Sur les appels d'au moins cent mille unités (22 891 appels, dont 16 260 de Sidequest), la même mesure donne une médiane de 49 ps dans wasmtime, 35 dans Chrome, 58 dans Firefox ; mais le coût fixe et la granularité de l'horloge y pèsent autant que le calcul (une unité y fait quelques µs pour 15 à 20 µs de fixe), et les 99ᵉ centiles (de 99 à 293 ps) mesurent surtout ce bruit. Le seuil d'un million est celui qu'on retient.

### Le facteur navigateur/wasmtime, appel par appel

La durée de chaque appel dans le navigateur sur la durée **du même appel** dans wasmtime. « Part du carburant » : (durée − coût fixe du navigateur) / (durée − coût fixe de wasmtime), pour les appels dont cette part dépasse 50 µs dans wasmtime (en dessous, la granularité de l'horloge la rend illisible). « n ; médiane ; 99ᵉ centile ; maximum ».

| Module | Chrome ≥ 10⁶ | Chrome, part du carburant ≥ 10⁶ | Chrome ≥ 10⁵ | Firefox ≥ 10⁶ | Firefox, part du carburant ≥ 10⁶ | Firefox ≥ 10⁵ |
| --- | --- | --- | --- | --- | --- | --- |
| devine | — | — | 3 ; 1,14 ; 1,34 ; 1,34 | — | — | 3 ; 1,71 ; 2,15 ; 2,15 |
| ardoise | — | — | 109 ; 0,88 ; 1,27 ; 1,36 | — | — | 109 ; 0,95 ; 2,00 ; 2,01 |
| Punchline | 7 ; 1,16 ; 1,22 ; 1,22 | 7 ; 1,18 ; 1,26 ; 1,26 | 5 385 ; 0,77 ; 1,15 ; 29,21 | 7 ; 2,57 ; 2,72 ; 2,72 | 7 ; 2,72 ; 2,85 ; 2,85 | 5 385 ; 1,24 ; 2,99 ; 27,11 |
| Sidequest | 41 ; 0,97 ; 1,25 ; 1,25 | 41 ; 1,00 ; 1,26 ; 1,26 | 16 260 ; 0,79 ; 1,10 ; 1,51 | 41 ; 1,59 ; 4,24 ; 4,24 | 41 ; 1,74 ; 4,35 ; 4,35 | 16 260 ; 0,99 ; 1,64 ; 16,96 |
| Tier list | 347 ; 0,91 ; 1,42 ; 1,82 | 296 ; 0,77 ; 1,66 ; 1,76 | 1 134 ; 0,96 ; 1,29 ; 1,82 | 347 ; 1,10 ; 2,48 ; 4,42 | 296 ; 1,24 ; 4,07 ; 4,28 | 1 134 ; 1,05 ; 1,97 ; 4,42 |

**Les cinq modules ensemble**, par tour (chaque tour : un navigateur neuf par module, seconde passe ; les 395 appels d'au moins un million d'unités) :

| Tour | Chrome : facteur | Chrome : ps par unité | Firefox : facteur | Firefox : ps par unité |
| --- | --- | --- | --- | --- |
| 1 (06 h 55) | 0,94 ; 1,42 ; 1,82 | 36 ; 67 ; 131 | 1,14 ; 3,99 ; 4,42 | 63 ; 124 ; 336 |
| 2 (06 h 56) | 0,90 ; 1,23 ; 1,71 | 34 ; 52 ; 70 | 1,15 ; 4,98 ; 12,63 | 64 ; 187 ; 678 |
| 3 (07 h 20) | 0,91 ; 1,69 ; 2,73 | 36 ; 74 ; 125 | 1,16 ; 4,21 ; 7,03 | 64 ; 203 ; 455 |
| 4 (07 h 21) | 0,90 ; 1,30 ; 2,97 | 36 ; 60 ; 137 | 1,18 ; 4,13 ; 6,52 | 66 ; 285 ; 673 |
| wasmtime (06 h 46) | 1 | 43 ; 82 ; 105 | 1 | 43 ; 82 ; 105 |

« médiane ; 99ᵉ centile ; maximum ». Hors navigateur, au premier tour : bun (JavaScriptCore) 1,03 ; 3,00 ; 3,92 et 50 ; 97 ; 193 ps ; node 0,96 ; 1,26 ; 4,11 ; deno 0,92 ; 1,14 ; 1,24.

**Par sorte d'appel**, les quatre tours ensemble (appels d'au moins un million d'unités ; « n ; médiane ; 99ᵉ centile ; maximum ») :

| Appels | Chrome | Firefox |
| --- | --- | --- |
| pires cas : `PireCas` (construire le pire cas, une écriture) | 68 ; 1,17 ; 1,41 ; 1,41 | 68 ; 2,60 ; 4,98 ; 4,98 |
| pires cas : `Regarder` (la vue de l'écran, que juge E19) | 252 ; 1,00 ; 1,83 ; 2,97 | 252 ; 1,74 ; 7,03 ; 12,63 |
| pires cas : `Verifier` | 112 ; 1,01 ; 1,49 ; 1,82 | 112 ; 1,75 ; 2,06 ; 4,42 |
| pires cas : `Etat` | 28 ; 1,10 ; 1,16 ; 1,16 | 28 ; 2,00 ; 2,39 ; 2,39 |
| soirée : `Canonique` (Tier list) | 1 096 ; 0,77 ; 1,09 ; 1,48 | 1 096 ; 1,04 ; 1,70 ; 6,52 |
| soirée : `Entree` (Tier list) | 24 ; 1,05 ; 1,38 ; 1,38 | 24 ; 1,37 ; 1,49 ; 1,49 |

**En durée, les vues de l'écran des pires cas de Tier list** (de 7 083 632 à 7 289 169 unités, les plus lourdes lectures de pire cas des cinq traces), au plus sur leurs sept répétitions et les quatre tours : **413 µs dans wasmtime, 1 160 µs dans Chrome, 4 960 µs dans Firefox** (une vue de 7 083 632 unités, au deuxième tour). Sidequest (3 368 735 unités) : 157, 155 et 520 µs. La part que E19 lui donne est de 8,25 ms (33 ms, la moitié au jeu, la moitié de celle-ci à la lecture). L'appel le plus lourd des traces, la construction du pire cas de Sidequest (29 498 510 unités), dure 725 µs dans wasmtime, de 825 à 900 µs dans Chrome, de 2 760 à 2 900 µs dans Firefox.

Ce qu'on en lit :

- **Chrome exécute le module à la vitesse de wasmtime** : facteur médian de 0,90 à 0,94 sur les appels lourds, 99ᵉ centile de 1,23 à 1,69, maximum de 1,71 à 2,97 selon le tour ; une unité y vaut 34 à 36 ps en médiane, au plus 137 ps.
- **Firefox est plus lent, et surtout plus irrégulier** : médiane de 1,14 à 1,18, mais 99ᵉ centile de 4,0 à 5,0 et maximum de 4,4 à 12,6 ; une unité y vaut 63 à 66 ps en médiane, jusqu'à 678 ps. La queue vient des lectures et des constructions des pires cas, des appels longs exécutés peu de fois (sept répétitions) : la vue de l'écran des pires cas y est 1,7 fois plus lente que dans wasmtime en médiane, 7 fois au 99ᵉ centile. C'est précisément l'appel que E19 juge le plus durement.
- **La marge « navigateur » de 2, choisie à la phase 2, ne vaut pas pour tous les navigateurs** : il couvre largement Chrome, pas la queue de Firefox. Mais il s'applique à `PS_PAR_UNITE` (200 ps, deux à quatre fois au-dessus de la médiane de wasmtime) : 200 × 2 = 400 ps d'unité dans un navigateur, sur cette machine, couvre chaque appel lourd mesuré dans Chrome (au plus 137 ps), le 99ᵉ centile de Firefox (au plus 285 ps), pas son maximum (678 ps).
- Sur l'ensemble d'une soirée, légers compris, le navigateur est même plus rapide que wasmtime (facteur médian de 0,78 à 0,97 dans Chrome, de 0,81 à 1,06 dans Firefox) : le coût fixe domine, et le navigateur n'a pas le passage au fil de l'instance.

### Les pauses

Le maximum de la durée d'un appel léger peut être très grand. Sur les quatre tours, dans Chrome, une `Entree` de 5 197 unités de Sidequest a duré 3,7 ms (59 µs dans wasmtime), un `Etat` de 9 912 unités 3,4 ms (28 µs) ; dans Firefox, des `Bonjour` de Tier list sur une instance neuve de 2,0 à 2,5 ms (1,0 à 1,15 ms dans wasmtime, qui y paie aussi la première mémoire de l'instance). Sur 375 536 appels de moins de cent mille unités, 78 ont duré plus d'une milliseconde dans Chrome, 79 dans Firefox. Ces durées ne viennent pas du carburant de l'appel : ce sont des pauses du fil de la page (ramasse-miettes, compilation, machine chargée) tombées pendant lui. Le banc réutilise les tampons de ses instantanés pour ne pas en provoquer lui-même, mais ne sait pas les séparer d'un appel. Elles sont réelles pour la partie : elles tombent dans l'image, sur la part du commun. Elles se mesureront avec le vrai commun.

### La mise en route

Durée totale de chaque rejeu de la trace (somme des appels chronométrés), passe 1 / passe 2, en ms :

| Module | Appels | wasmtime | Chrome | Firefox |
| --- | --- | --- | --- | --- |
| devine | 7 941 | 150 / 147 | 126 / 120 | 132 / 126 |
| ardoise | 4 792 | 89 / 90 | 76 / 74 | 80 / 79 |
| Punchline | 10 881 | 263 / 261 | 228 / 208 | 281 / 293 |
| Sidequest | 86 505 | 2 699 / 2 822 | 2 202 / 2 176 | 2 497 / 2 371 |
| Tier list | 6 656 | 1 226 / 1 360 | 1 249 / 1 220 | 1 320 / 1 279 |

La première passe ne coûte que quelques pour cent de plus : la compilation paresseuse et la montée de niveau se paient sur les premiers appels lourds (la première construction d'un pire cas de Sidequest : 3,46 ms dans Chrome à la première passe, 0,92 ms à la seconde ; celles de Punchline dans Firefox : 0,80 à 0,84 ms, puis 0,52 à 0,54 ms), pas sur la soirée.

## Mesure 4 : le chargement d'un module

### La taille

En ko (1 000 octets). « Livré » : le module compilé au profil `module` ; « réécrit » : après `reecriture`. gzip au niveau 9 et brotli au niveau 11, par node (`zlib`).

| Module | livré | livré, gzip | livré, brotli | réécrit | réécrit, gzip | réécrit, brotli |
| --- | --- | --- | --- | --- | --- | --- |
| devine | 423 | 156 | 119 | 592 | 168 | 126 |
| ardoise | 227 | 63 | 50 | 370 | 74 | 56 |
| Punchline | 780 | 331 | 271 | 1 117 | 355 | 290 |
| Sidequest | 640 | 225 | 171 | 935 | 247 | 190 |
| Tier list | 771 | 278 | 212 | 1 154 | 306 | 233 |

La réécriture ajoute 40 à 63 % d'octets, presque tous compressibles (6 à 12 % après brotli). Ce qui voyage jusqu'aux téléphones (le livré, réécrit chez eux, ou le réécrit) n'est pas décidé ; les deux sont donnés.

### La compilation

En ms. « réécriture » et « Cranelift » : la réécriture seule et `wasmtime::Module::new` seul, avec les réglages de `moteur-wasmtime`, médiane de cinq. Navigateurs : `WebAssembly.compile` sur les octets réécrits, la première fois ; puis la médiane de cinq compilations d'octets rendus uniques (une section personnalisée différente à chaque fois, pour qu'aucun moteur ne reprenne une compilation déjà faite) ; puis la **mise en route** (compiler des octets uniques, instancier, `Bonjour`, `Decrire`), médiane de trois.

| Module | réécriture | Cranelift | Chrome : 1ʳᵉ ; médiane ; mise en route | Firefox : 1ʳᵉ ; médiane ; mise en route | Chrome sans compilation paresseuse : 1ʳᵉ ; médiane ; mise en route | bun : 1ʳᵉ ; médiane ; mise en route |
| --- | --- | --- | --- | --- | --- | --- |
| devine | 36,7 | 138,3 | 0,8 ; 0,5 ; 3,2 | 2,4 ; 2,2 ; 2,3 | 4,8 ; 3,1 ; 2,5 | 1,0 ; 0,6 ; 1,1 |
| ardoise | 30,4 | 119,4 | 0,7 ; 0,6 ; 3,9 | 3,2 ; 1,8 ; 2,1 | 2,8 ; 2,7 ; 2,4 | 1,1 ; 0,6 ; 0,8 |
| Punchline | 73,8 | 297,4 | 0,9 ; 0,8 ; 4,3 | 4,7 ; 4,1 ; 4,3 | 13,0 ; 5,3 ; 4,2 | 4,5 ; 1,2 ; 1,8 |
| Sidequest | 64,6 | 247,2 | 1,6 ; 0,7 ; 4,1 | 4,0 ; 3,5 ; 3,8 | 5,6 ; 4,1 ; 4,0 | 1,4 ; 1,2 ; 1,6 |
| Tier list | 82,4 | 323,9 | 0,9 ; 0,7 ; 6,1 | 4,9 ; 4,5 ; 5,9 | 5,9 ; 4,2 ; 5,7 | 1,6 ; 1,4 ; 2,9 |

Ce qu'on en lit :

- **Sur l'ordinateur de la mesure, un module se compile en quelques millisecondes dans un navigateur** : de 0,5 à 4,9 ms pour `WebAssembly.compile`, de 2 à 6 ms jusqu'à un `Decrire` rendu. Cranelift, dans wasmtime, en prend 120 à 324, sur un seul fil.
- **Chrome compile paresseusement par défaut** : `WebAssembly.compile` y rend la main en moins de 2 ms, et la compilation de chaque fonction se fait à son premier appel. Avec `--js-flags=--no-wasm-lazy-compilation` (hors défaut, étiquette « Chrome sans compilation paresseuse »), la première compilation prend de 2,8 à 13,0 ms : c'est le travail complet du compilateur de base, réparti sur plusieurs fils.
- **Ce qui n'est pas mesuré : les téléphones.** Aucun téléphone n'est relié à cette machine, et le simulateur iOS fait tourner le processeur du Mac. Le temps de compilation sur un téléphone, et donc la borne de taille du point 3 de RESTE-PHASE-2, restent à mesurer avec la page téléphone.

## Mesure 6 : la partie qui fait foi en arrière-plan

La page de cadence (`page/cadence.js`) note chaque réveil de ses horloges : une chaîne de `setTimeout` à 16 ms sur le fil de la page (la boucle d'une partie à 60 pas par seconde), `requestAnimationFrame`, la même chaîne de `setTimeout` dans un Worker dédié, et, dans la variante « audio », un `AudioContext` qui joue un oscillateur à travers un gain nul, avec un `ScriptProcessorNode` de 1 024 échantillons dont chaque rappel est noté. Un navigateur neuf par variante ; la page 30 s au premier plan, puis un autre onglet activé devant elle pendant 420 s (Chrome par le protocole DevTools, `Target.activateTarget` ; Firefox par WebDriver BiDi, `browsingContext.activate`), puis de nouveau devant pendant 15 s. Le son est permis sans geste (Chrome : `--autoplay-policy=no-user-gesture-required` ; Firefox : `media.autoplay.default` à 0), comme après le clic de l'hôte qui lance le jeu. Chrome de 07 h 00 à 07 h 08, Firefox de 07 h 08 à 07 h 16.

**La page est bien cachée** : dans les deux navigateurs sans interface, `document.visibilityState` vaut `hidden` deux secondes après le changement d'onglet, `document.hasFocus()` faux, et `visibilitychange` est reçu à l'aller et au retour. Aucun `freeze` n'a été reçu en sept minutes.

Réveils par seconde, et intervalle entre deux réveils (médiane / maximum, en ms) :

| Navigateur, variante | Horloge | Premier plan | Caché, 1ʳᵉ minute | Caché, minutes 2 à 7 | Caché, intervalle médian / max | De retour |
| --- | --- | --- | --- | --- | --- | --- |
| Chrome, témoin | `setTimeout` (page) | 58,8 | 1,0 | **un réveil par minute** | 1 000 / 60 022 | 58,2 |
| | `requestAnimationFrame` | 60,0 | 0 | 0 | — | 60,0 |
| | Worker | 46,3 | 56,5 | 56,3 à 56,5 | 18 / 103 | 46,5 |
| Chrome, audio muet | `setTimeout` (page) | 58,9 | 1,0 | **un réveil par minute** | 1 000 / 60 002 | 59,0 |
| | `requestAnimationFrame` | 60,0 | 0 | 0 | — | 60,0 |
| | Worker | 46,4 | 46,0 | 45,6 à 46,0 | 22 / 26 | 46,2 |
| | rappels du son | 46,9 | 46,9 | 46,9 | 21 / 28 | 46,8 |
| Firefox, témoin | `setTimeout` (page) | 46,9 | 1,0 | 1,0 | 1 003 / 1 018 | 46,7 |
| | `requestAnimationFrame` | 120,0 | 0,1 | à 64, 128 et 256 s | l'intervalle double : 1, 2, 4 … 128 s | 120,0 |
| | Worker | 46,9 | 46,1 | 46,2 à 46,5 | 22 / 33 | 46,7 |
| Firefox, audio muet | `setTimeout` (page) | 46,8 | 46,4 | 46,1 à 46,4 | 22 / 81 | 46,5 |
| | `requestAnimationFrame` | 120,0 | 0,1 | à 64, 128 et 256 s | l'intervalle double : 1, 2, 4 … 128 s | 120,0 |
| | Worker | 46,8 | 46,4 | 46,1 à 46,4 | 22 / 30 | 46,5 |
| | rappels du son | 46,9 | 46,9 | 46,9 | 21 / 81 | 46,8 |

Dans Chrome, les réveils de la page cachée tombent à 0,78 s, 1,78 s… 59,78 s, puis à 66,8 s, 126,8 s, 186,8 s, 246,8 s, 306,8 s, 366,8 s : une seconde pendant la première minute, puis une minute.

Ce qu'on en lit :

- **Sur le fil de la page, la partie ne continue pas de tourner.** Chrome 156 bride une chaîne de `setTimeout` à un réveil par seconde dès que l'onglet est caché, puis à **un réveil par minute après une minute** cachée. (Le bridage intense que Chrome annonçait en 2021 partait de cinq minutes cachées : lu, non mesuré ; ici, sans interface, c'est une minute.) Firefox 158 la bride à un réveil par seconde, sans aggravation en sept minutes. Une boucle qui rattrape le temps ferait, dans Chrome, 3 600 pas d'un coup chaque minute ; une boucle qui ne rattrape pas serait à l'arrêt (OUT-53, SQ-09).
- **`requestAnimationFrame` s'arrête** dans Chrome, et presque dans Firefox, qui livre encore une image à 1, 2, 4, 8, 16, 32, 64, 128 et 256 s : un intervalle qui double. Une partie menée par les images s'arrête avec l'onglet.
- **Un Worker dédié n'est pas bridé** dans les deux navigateurs : 46 à 56 réveils par seconde en arrière-plan pendant sept minutes, intervalle maximal de 103 ms. C'est la seule horloge mesurée qui garde la partie vivante dans les deux navigateurs sans dépendre du son.
- **Un `AudioContext` muet ne protège la page que dans Firefox.** Firefox ne bride pas les minuteries d'un onglet qui joue un son, même par un gain nul ; Chrome les bride comme s'il n'y avait pas de son. Dans les deux, les rappels du son eux-mêmes continuent à 46,9 par seconde (48 000 / 1 024).
- **Même au premier plan, `setTimeout(16)` ne donne pas 60 réveils par seconde** : 58,8 sur le fil de la page de Chrome, mais 46 à 47 (un intervalle de 22 ms) dans ses Workers et partout dans Firefox. Une boucle de partie ne peut pas compter ses pas sur ses réveils : elle les compte sur l'horloge, et en fait plusieurs par réveil.
- Le `requestAnimationFrame` de Firefox sans interface tourne à 120 images par seconde : sa cadence est celle de son propre affichage virtuel, et ne dit rien d'une télé.

Ce que la mesure ne dit pas, honnêtement : elle est faite **sans interface**, en cachant un onglet derrière un autre. Une fenêtre recouverte ou réduite, l'App Nap de macOS sur un navigateur avec interface (SAL-131, SAL-132), la veille de l'écran, les modes d'économie d'énergie et de mémoire de Chrome, une durée de plus de sept minutes, n'y sont pas ; ni Safari. Les politiques de bridage des minuteries vivent dans le moteur de rendu, pas dans l'affichage : on peut s'attendre à les retrouver avec interface, mais ce n'est pas mesuré.

**Proposition, pour le CONTRAT (« La partie qui fait foi continue de tourner »).** Faire tourner la partie qui fait foi dans un Worker dédié, menée par sa propre horloge et qui rattrape le temps ; signaler sur la télé (et dans le rapport) quand la page est cachée ; et faire de `cadence.mjs` la pression de l'épreuve sur chaque navigateur visé : un Worker qui tombe sous un seuil de réveils en arrière-plan échoue.

## Lot 1 : la longueur du journal d'une soirée longue

Mesuré le 2026-11-05, de 09 h 09 à 09 h 23, sur la machine de l'hôte (MacBook Apple M5, 16 Go, macOS 26.6.2, rustc 1.99.0), sans navigateur ; la charge (`vm.loadavg`) était de 5,7 à 8,0 : d'autres programmes tournaient (dont `coqchk`, un cœur entier). Le nombre de lignes et d'octets ne dépend pas de la charge ; les durées, si.

La soirée longue de E15, plan standard (six heures simulées, 1 296 000 pas, des appareils qui vont et viennent), sans aucune troncature du journal, ses lignes écrites comme la garde les écrit (`partie::persistance::encoder_journal`, `VERSION_GARDE` 1) :

| Jeu | Arrivées | Lignes du journal | Octets de la garde | Octets par ligne |
| --- | --- | --- | --- | --- |
| Punchline | 668 759 | 5 278 242 | 151 838 722 | 28,8 |
| Sidequest | 719 986 | 13 120 681 | 253 116 416 | 19,3 |

*Relevé de nouveau le 2026-11-05 à 11 h 29 et 11 h 52, après la correction du lot 1 (une arrivée reçue pendant une avancée inachevée va après sa fin)* : Sidequest, 13 120 680 lignes et 253 116 411 octets ; Punchline, inchangé. Même commande.

Soit environ 10 lignes par pas chez Sidequest (calculé), et 195 octets de garde par pas : un journal jamais tronqué grandirait de 0,7 Mio par minute de soirée (calculé). C'est ce que P10 borne : la garde tient au plus deux points et une centaine de cases (PHASE-3, § 2.4), et le journal vivant se tronque avant le plus vieux point gardé (`Partie::tronquer_journal_avant`). Le lot 1 ne mesure pas la mémoire de ce journal dans le processus de l'épreuve, seulement ses octets encodés.

Commande (depuis `soiree/`, la ligne « journal de la soirée longue » du rapport) :

```sh
CARGO_BUILD_JOBS=4 cargo run --release -p epreuve --bin soiree-epreuve -- ../games/sidequest
```

## Lot 2 : le chargement, la boucle vide (M10) et le point de Tier list (M3)

Mesuré le 2026-11-05, de 12 h 33 à 12 h 41 (UTC), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), dans wasmtime 50.0.1 (Cranelift), sans navigateur. Le carburant ne dépend pas de la machine ; les durées, si.

### Le carburant du chargement

Les cinq modules, compilés au profil `module` depuis les arbres de travail de la branche `refonte` à cette date (`devine`, `ardoise`, Punchline, Sidequest, Tier list), chargés sous `ReglagesMoteur::PHASE_1` ; `Creer` relevé dans la soirée du plan court rejouée par E9 et E14 (graine `5e…5e`), au plus de tous ses appels. En unités de carburant :

| Module | `Bonjour` | `Decrire` | `Creer` (médiane ; maximum) |
| --- | --- | --- | --- |
| devine | 3 104 | 7 468 | 4 275 ; 4 275 |
| ardoise | 3 161 | 7 304 | 4 097 ; 4 097 |
| Punchline | 3 191 | 9 297 | 292 345 ; 292 345 |
| Sidequest | 3 194 | 7 573 | 317 069 ; 343 002 |
| Tier list | 3 167 | 9 322 | 105 688 ; 105 688 |

Commande (depuis `soiree/`, la ligne « carburant du chargement » et la ligne `Creer` de la soirée) :

```sh
CARGO_BUILD_JOBS=4 cargo run --release -p epreuve --example etalonner -- target/wasm32-unknown-unknown/module/devine_module.wasm target/wasm32-unknown-unknown/module/ardoise_module.wasm ../games/punchline/target/wasm32-unknown-unknown/module/punchline_module.wasm ../games/sidequest/target/wasm32-unknown-unknown/module/sidequest_module.wasm ../games/tier-list/target/wasm32-unknown-unknown/module/tier_list_module.wasm
```

### M10 : une unité de carburant sur une boucle vide

Le module de `Bonjour` qui boucle (`crates/moteur-commun/essais/bonjour-qui-boucle.wat`) chargé sous un garde-fou de dix minutes, moins un témoin qui piège dès l'entrée, divisé par `BUDGET_DE_CHARGEMENT` ; « médiane ; minimum ; maximum » en ps par unité, sur n chargements. La charge est `vm.loadavg` sur une minute, avant et après.

| Budget | Charge | n | ps par unité | Durée de la boucle (ms) |
| --- | --- | --- | --- | --- |
| 2³² | peu chargée (2,4 à 2,8) | 7 | 528 ; 481 ; 539 | 2 070 à 2 317 |
| 2³² | quatorze `yes`, lancés 3 s avant (3,7 puis 13,2) | 7 | 1 249 ; 928 ; 1 492 | 3 989 à 6 412 |
| 2²⁶ | peu chargée (3,4) | 15 | 358 ; 233 ; 597 | 16 à 40 |
| 2²⁶ | quatorze `yes`, lancés 5 s avant (4,5 puis 5,6) | 15 | 903 ; 367 ; 1 033 | 25 à 70 |
| 2²⁶ | quatorze `yes`, lancés 45 s avant (21) | 21 | 775 ; 383 ; 950 | 26 à 64 |

Les quatorze `yes` (`yes > /dev/null`, un par processus) occupent les dix cœurs. **Sous charge, une unité de boucle vide coûte jusqu'à 1 492 ps**, et non 680 comme au point 2 de RESTE-PHASE-2, dont la charge n'avait duré que le temps d'un test de 3 s. À 2²⁶ les durées sont plus dispersées et souvent plus courtes par unité qu'à 2³² : une boucle de quelques dizaines de millisecondes ne voit pas toute la charge. Le budget se juge donc sur le maximum à 2³², 1 492 ps : 2²⁶ unités en 100 ms, dix fois sous le garde-fou d'une seconde.

Commande (depuis `soiree/` ; pour la charge, lancer d'abord quatorze `yes > /dev/null &`, et les arrêter après) :

```sh
CARGO_BUILD_JOBS=4 cargo run --release -p moteur-wasmtime --example boucle_vide -- 15
```

Sous la même charge, `chargement_borne::bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement` (`PHASE_1`, garde-fou d'une seconde, `BUDGET_DE_CHARGEMENT` 2²⁶) a réussi 10 fois sur 10 à 5 s de charge, puis 20 fois sur 20 à 45 s (le binaire de test lancé seul, `target/debug/deps/moteur-… chargement_borne`, compilé avant la charge). À 2³², sans charge, il échoue : `Panne(PanneOutil { cause: GardeFouMural, … })`.

### M3 : la taille d'un point de reprise de Tier list, dans wasmtime

L'instantané hors des quatre zones (P3), le module de Tier list réécrit et chargé sous `PHASE_1`. Le pire cas est celui de `tests/sauvegarde_lourde.rs` de Tier list : le plus lourd plateau que le jeu accepte (`IMAGES_MAX_OCTETS` d'images, ici quatre images incompressibles de 262 144 octets, des tuiles texte jusqu'à `TUILES_MAX`, `RANGS_MAX` rangs, chaque texte à sa limite), sa sauvegarde écrite (« Download Backup »), puis relue (« Restore Backup »).

| État | Mémoire | Zones (remises à zéro, exclues) | Instantané hors des zones | Gardé (`InstantaneGarde`) |
| --- | --- | --- | --- | --- |
| plateau neuf, juste créé | 406 pages (26 607 616 o) | 26 284 032 o | 323 584 o (0,31 Mio) | — |
| pire cas, sauvegarde relue | 670 pages (43 909 120 o) | 26 284 032 o | 17 625 088 o (16,8 Mio), 3 plages | 17 625 193 o |

Le plateau neuf confirme le calcul de P3 (0,31 Mio). **Le pire cas est cinquante-cinq fois plus gros** : la mémoire a grandi de 264 pages pendant la sauvegarde et sa relecture, et le tas de l'allocateur (`dlmalloc`) garde cette place, que l'instantané garde avec lui. Avant le lot 2, l'instantané copiait la mémoire entière : 43 909 120 octets. Relu de la garde et restauré dans une instance neuve, il rend la même empreinte. Le chiffrement et l'envoi de ce point dans le Worker restent à mesurer (lot 11) : R2, P8 et P9 se jugent sur ces 16,8 Mio.

Commande (depuis `games/tier-list/`) :

```sh
CARGO_BUILD_JOBS=4 cargo test --test sauvegarde_lourde le_point -- --nocapture
```

## Lot 3a : le moteur du navigateur dans Chrome

Constaté et mesuré le 2026-11-05, de 15 h 39 à 16 h 03 (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), dans Chrome 156.0.8078.80 sans interface (`--headless=new`, profil neuf), par `soiree-banc machine`. La machine était chargée par d'autres programmes : `vm.loadavg` de 18 à 29 sur une minute (*corrigé après la vérification : « 21 à 29 » ne comprenait pas la charge 18 de 16 h 03*), 32 à 38 % de mémoire libre (`memory_pressure`). Les verdicts n'en dépendent pas ; les durées, si.

### Les messages de V8

Ce que Chrome lève pour chaque piège de `web/moteur-navigateur/essais/machine.wat` (un module non réécrit, appelé sur le fil principal de la page), et ce que wasmtime 50.0.1 rend pour le même scénario (`essais/pieges.txt`, vérifié par `crates/banc/tests/pieges.rs`) :

| Scénario | Chrome 156 (`error.name : error.message`) | wasmtime 50.0.1 |
| --- | --- | --- |
| `unreachable` | `RuntimeError : unreachable` | ``wasm trap: wasm `unreachable` instruction executed`` |
| lecture à 0xFFFFFFF0 | `RuntimeError : memory access out of bounds` | `MemoryOutOfBounds` |
| récursion sans fin | `RangeError : Maximum call stack size exceeded` | `StackOverflow` |
| `i32.div_s` par zéro | `RuntimeError : divide by zero` | `integer divide by zero` |
| `i32.rem_s` par zéro | `RuntimeError : remainder by zero` | `integer divide by zero` |
| `i32::MIN / -1` | `RuntimeError : divide result unrepresentable` | `integer overflow` |
| `i32.trunc_f32_s` d'un NaN | `RuntimeError : float unrepresentable in integer range` | `invalid conversion to integer` |
| `i32.trunc_f32_s` de 1e10 | `RuntimeError : float unrepresentable in integer range` | `integer overflow` |
| `call_indirect` au-delà de la table | `RuntimeError : table index is out of bounds` | `undefined element: out of bounds table access` |
| `call_indirect` vers un élément nul | `RuntimeError : null function` | `uninitialized element` |
| `call_indirect` d'un autre type | `RuntimeError : function signature mismatch` | `indirect call type mismatch` |

Dix messages sur onze désignent un seul piège de wasmtime et y sont ramenés (`moteur_navigateur::MESSAGES`) ; `float unrepresentable in integer range` en désigne deux (`AMBIGUS`). Après chaque piège, l'instance répond encore.

D'autres constats de la même suite : `Memory.prototype.grow` au-delà du maximum lève `RangeError : WebAssembly.Memory.grow(): Maximum memory size exceeded` ; une vraie croissance détache l'ancien tampon (sa longueur tombe à 0), une croissance à taille égale n'est pas demandée et ne le détache pas ; les globales `i64` (`i64::MIN`, `i64::MAX`, ±(2⁵³ + 1)) passent exactement par `BigInt`, les `f32` et `f64` (NaN canoniques et -0 compris) par un nombre ; une globale immuable refuse d'être posée.

### Ce que coûte le scénario

| Mesure | Valeur | Comment |
| --- | --- | --- |
| `soiree-banc machine`, construction incrémentale comprise, deux passages dans deux Chrome neufs | 3,8 s, 10,4 s et 5,1 s (trois exécutions, la dernière à 16 h 03, charge 18) | `/usr/bin/time -l` |
| Mémoire résidente maximale de `soiree-banc` et de ses sous-processus attendus (`cargo`, `wasm-bindgen`), pris un à un | 0,21 à 0,22 Go | `/usr/bin/time -l` |
| Mémoire résidente de Chrome, somme de son processus et de tous ses descendants, au plus haut | 0,46 à 1,22 Go (trois exécutions) ; 1 206 272 et 1 208 624 Kio (1,24 Go) après la correction | un échantillon toutes les 0,1 s de `ps -A -o pid,ppid,rss` ; une somme de RSS compte plusieurs fois les pages partagées : c'est un majorant. La commande qui la refait est ci-dessous (écrite après la vérification : le premier relevé n'en avait pas) |
| Le module de l'exemple `essais-navigateur` (avec `wat`, `reecriture`, `moteur-commun`) | 4 229 970 octets compilé, 3 710 370 après `wasm-bindgen` (le vérificateur, sur les mêmes sources : 3 710 374 ; l'écart de 4 octets n'est pas expliqué). Après la correction (`AMBIGUS` porte ses pièges) : 4 230 413 compilé, 3 710 900 après `wasm-bindgen` | `ls -l web/target/wasm32-unknown-unknown/release/examples/essais_navigateur.wasm target/banc/paquet/essais_navigateur_bg.wasm` |

La compilation de `web/` depuis un dossier neuf est dans « Lot 3a » de `LOTS-PHASE-3.md`.

Commande (depuis `soiree/`) :

```sh
CARGO_BUILD_JOBS=4 cargo run -p banc --bin soiree-banc -- machine
```

La mémoire de Chrome, depuis `soiree/`, `soiree-banc` déjà construit (`CARGO_BUILD_JOBS=4 cargo build -p banc --bin soiree-banc`) : le script lance le scénario, échantillonne toutes les 0,1 s la somme des RSS du Chrome du banc (le processus dont la ligne de commande porte `target/banc/profils`, sans `--type=`) et de tous ses descendants, et dit la plus haute, en Kio.

```sh
cat > rss-chrome.sh <<'FIN'
"$@" & banc=$!
max=0
while kill -0 "$banc" 2>/dev/null; do
  s=$(ps -A -o pid=,ppid=,rss=,command= | awk '
    { pid[NR]=$1; pere[$1]=$2; rss[$1]=$3; if ($0 ~ /target\/banc\/profils/ && $0 !~ /--type=/) racine[$1]=1 }
    END { t=0
      for (i in pid) { p=pid[i]; q=p; while (q in pere && !(q in racine) && q>1) q=pere[q]; if (q in racine) t+=rss[p] }
      print t }')
  [ "$s" -gt "$max" ] && max=$s
  sleep 0.1
done
wait "$banc"; code=$?
echo "Chrome, somme des RSS au plus haut : $max Kio ; soiree-banc : code $code" >&2
exit $code
FIN
/usr/bin/time -l sh rss-chrome.sh target/debug/soiree-banc machine
```

Relevé avec ce script le 2026-11-05 à 16 h 39 et 16 h 40 (même machine, Chrome 156.0.8078.80, `vm.loadavg` de 19 à 27, 31 à 32 % de mémoire libre) : Chrome 1 206 272 et 1 208 624 Kio au plus haut ; `soiree-banc` 0,22 à 0,23 Go (`/usr/bin/time -l`) ; 21,6 s et 17,9 s, construction incrémentale comprise et l'échantillonnage tournant à côté.

## Lot 3b : la durée de E20 (point 14 de RESTE-PHASE-2)

Mesuré le 2026-11-05, de 18 h 23 à 19 h 22 (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), dans Chrome 156.0.8078.80 (`--headless=new`) et Firefox 158.0.1 (`--headless`), profils neufs, par `soiree-epreuve … --court --navigateurs chrome,firefox`, une exécution à la fois, sans compilation à côté. D'autres programmes chargeaient la machine : `vm.loadavg` de 15 à 24 sur une minute au départ de chaque exécution (noté à chaque ligne), 31 à 42 % de mémoire libre. Une seule exécution par module : les durées varient avec la charge (le lancement et la fermeture surtout), aucun chiffre n'est une médiane.

Ce que E20 reprend, sur le plan court : chaque soirée jouée (la principale, l'hostile, la longue d'une heure, celle des propriétés, celle des gels ; la seconde exécution de E9, identique, une fois), depuis sa création, fautes refaites. « Relevé » : la reprise dans wasmtime, faite par l'épreuve. « Page » : de l'ouverture de la page au rapport, deux passages compris (le vrai moteur, puis le moteur fautif du contrôle négatif, qui s'arrête au 101ᵉ appel) ; « reprise » : celle du vrai moteur seule, mesurée par la page (`performance.now()`, Worker) ; « en tout » : lancement et fermeture du navigateur compris. « Banc » : `soiree-banc rejouer` entier, sa construction (la règle « mêmes lints », `cargo build` de l'exemple déjà à jour, `wasm-bindgen`) comprise.

| Module (départ, charge) | Événements, dont appels | Relevé (wasmtime) | Chrome : page, reprise, en tout | Firefox : page, reprise, en tout | Banc (construction) |
| --- | --- | --- | --- | --- | --- |
| `devine` (19 h 19, 23,6) | 13 384, 12 903 | 1,7 s | 2,1 s, 0,76 s, 22,1 s | 3,1 s, 1,2 s, 31,0 s | 59,7 s (4,8 s) |
| `ardoise` (19 h 21, 18,1) | 18 254, 17 777 | 1,2 s | 1,5 s, 0,61 s, 5,5 s | 2,2 s, 1,1 s, 17,8 s | 24,7 s (1,2 s) |
| Punchline (18 h 37, 16,9) | 25 921, 25 318 | 3,8 s | 4,6 s, 2,5 s, 8,6 s | 5,9 s, 3,6 s, 9,8 s | 19,6 s (0,7 s) |
| Sidequest (18 h 23, 15,0) | 497 088, 495 300 | 49,1 s | 60,4 s, 46,9 s, non relevé | 47,7 s, 37,4 s, non relevé | 158,9 s (non relevée) |
| Tier list (18 h 39, 19,6) | 29 798, 29 292 | 13,4 s | 10,4 s, 9,0 s, 17,9 s | 13,4 s, 11,0 s, 23,4 s | 42,5 s (1,1 s) |

Pour Sidequest, le `soiree-banc` lancé était d'avant l'ajout de « en tout » et de la construction au JSON (la commande de construction n'avait refait que `soiree-epreuve`) : ces deux chiffres manquent, le reste est le même code. L'épreuve entière, depuis les sources, compilation de la copie native comprise : Sidequest 792 s (0,72 Go de mémoire résidente au plus pour `soiree-epreuve` et la copie native, `/usr/bin/time -l`, qui ne compte pas les navigateurs), Punchline 135 s (0,52 Go), Tier list 166 s (0,76 Go) ; au lot 3a, le plan court sans E20 prenait 349 s, 35 s et 94 s (sans compilation). La mémoire des navigateurs n'est pas mesurée à ce lot (au lot 3a : Chrome au plus 1,24 Go, un majorant).

Ce qui se lit : la reprise de Sidequest (près de 500 000 appels) prend autant dans un Worker de Chrome ou de Firefox que dans wasmtime, relevé compris ; pour les petits modules, E20 est dominé par le lancement et la fermeture des navigateurs (de 4 à 28 s par navigateur, très variables sous cette charge). Le plan standard n'est pas mesuré avec E20 (deux soirées de trois minutes et six heures de soirée longue : la reprise de la soirée longue y serait six fois plus longue, estimé, non mesuré).

Les 33 modules de `exemples/fautifs`, plan court, de 18 h 43 à 19 h 19 : E20 réussi dans les deux navigateurs pour chacun, de 1 à 2 minutes chacun, épreuve comprise (`soiree-epreuve --wasm`, au plus 1,13 Go de mémoire résidente). Les fautes que leurs journaux font refaire : `Panique` seulement (trois chez `fichier-qui-panique`, six chez `panique-au-temps`) ; `declaration-incoherente` est refusé au chargement, et refusé pareil (« déclaration refusée : limites.memoire (1 pages) sous la mémoire minimale du module (36) »).

Commandes (depuis `soiree/`) :

```sh
CARGO_BUILD_JOBS=4 cargo build --release -p banc
CARGO_BUILD_JOBS=4 cargo build --release -p epreuve --bin soiree-epreuve
CARGO_BUILD_JOBS=4 cargo build -p devine-module -p ardoise-module --target wasm32-unknown-unknown --profile module
target/release/soiree-epreuve --wasm target/wasm32-unknown-unknown/module/devine_module.wasm --court --navigateurs chrome,firefox
target/release/soiree-epreuve ../games/sidequest --court --navigateurs chrome,firefox
# Les fautifs : compilés par `cargo test -p epreuve --test epreuve` dans target/verif-module.
for f in target/verif-module/wasm32-unknown-unknown/module/fautif_*_module.wasm; do
  target/release/soiree-epreuve --wasm $f --court --navigateurs chrome,firefox
done
```

### Après la vérification : E20 remesuré, fautes `Budget` provoquées comprises

Remesuré le 2026-11-05, de 20 h 42 à 21 h 09 (GMT), même machine, mêmes navigateurs (Chrome 156.0.8078.80, Firefox 158.0.1, sans interface, profils neufs), même commande, avec le relevé corrigé (deux fautes `Budget` provoquées avant les soirées : 3 instances neuves et 6 appels de plus par module). `vm.loadavg` de 8 à 13 au départ de chaque exécution (noté à chaque ligne), 31 à 44 % de mémoire libre ; une exécution par module, sans compilation à côté. Les chiffres du tableau précédent ont été pris sous une charge de 15 à 24 : la reprise de Sidequest y prenait de deux à trois fois plus (49,1 s de relevé contre 26,6 s ici) ; le passage du vérificateur, vers 20 h, sous une charge de 5 à 14, donnait pour Sidequest 17,8 s de relevé, 15,5 s et 16,9 s de reprise, 19,2 s et 20,3 s en tout, 40 s de banc. Ces durées dépendent d'abord de la charge de la machine.

| Module (départ, charge) | Événements, dont appels | Relevé (wasmtime) | Chrome : page, reprise, en tout | Firefox : page, reprise, en tout | Banc (construction) |
| --- | --- | --- | --- | --- | --- |
| `devine` (20 h 42, 10,9) | 13 393, 12 909 | 0,42 s | 1,1 s, 0,41 s, 2,0 s | 1,4 s, 0,56 s, 3,9 s | 8,3 s (2,3 s) |
| `ardoise` (20 h 43, 9,8) | 18 263, 17 783 | 0,43 s | 0,93 s, 0,42 s, 1,4 s | 2,0 s, 0,53 s, 4,2 s | 6,0 s (0,32 s) |
| Punchline (21 h 02, 12,9) | 25 930, 25 324 | 1,3 s | 2,1 s, 1,4 s, 3,1 s | 2,1 s, 1,3 s, 3,5 s | 6,9 s (0,26 s) |
| Sidequest (21 h 03, 8,6) | 497 097, 495 306 | 26,6 s | 17,6 s, 16,0 s, 19,8 s | 23,3 s, 21,6 s, 25,5 s | 45,9 s (0,58 s) |
| Tier list (21 h 07, 8,2) | 29 807, 29 298 | 10,1 s | 10,0 s, 8,7 s, 13,7 s | 12,6 s, 10,8 s, 15,4 s | 30,2 s (1,1 s) |

Les chiffres de Sidequest qui manquaient (« en tout » et la construction) sont là : 19,8 s dans Chrome et 25,5 s dans Firefox, lancement et fermeture compris ; 0,58 s de construction. L'épreuve entière, plan court, depuis les sources (la compilation déjà en cache) : Punchline 49 s (0,61 Go de mémoire résidente au plus pour `soiree-epreuve` et la copie native, navigateurs non comptés), Sidequest 235 s (0,92 Go), Tier list 101 s (0,55 Go) ; `devine` 24 s (0,71 Go) et `ardoise` 9,6 s (0,65 Go) en `--wasm`. Les 33 fautifs, de 20 h 43 à 21 h 02 : de 3 à 192 s chacun (`zones-lourdes`, le plus long ; les autres de 23 à 43 s), épreuve comprise, 0,94 Go de mémoire résidente au plus, E20 réussi pour chacun.

### Les messages de SpiderMonkey

Ce que Firefox 158.0.1 lève pour chaque piège de `web/moteur-navigateur/essais/machine.wat`, par `soiree-banc machine --navigateur firefox`, le 2026-11-05 vers 18 h 05 (même machine) :

| Scénario | Firefox 158 | wasmtime 50.0.1 |
| --- | --- | --- |
| `unreachable` | `RuntimeError : unreachable executed` | ``wasm `unreachable` instruction executed`` |
| lecture à 0xFFFFFFF0 | `RuntimeError : index out of bounds` | `MemoryOutOfBounds` |
| récursion sans fin | `InternalError : too much recursion` | `StackOverflow` |
| `i32.div_s` et `i32.rem_s` par zéro | `RuntimeError : integer divide by zero` | `integer divide by zero` |
| `i32::MIN / -1` | `RuntimeError : integer overflow` | `integer overflow` |
| `i32.trunc_f32_s` d'un NaN | `RuntimeError : invalid conversion to integer` | `invalid conversion to integer` |
| `i32.trunc_f32_s` de 1e10 | `RuntimeError : integer overflow` | `integer overflow` |
| `call_indirect` au-delà de la table | `RuntimeError : index out of bounds` | `undefined element: out of bounds table access` |
| `call_indirect` vers un élément nul | `RuntimeError : indirect call to null` | `uninitialized element` |
| `call_indirect` d'un autre type | `RuntimeError : indirect call signature mismatch` | `indirect call type mismatch` |

Six messages désignent un seul piège de wasmtime et y sont ramenés ; `index out of bounds` en désigne deux (la mémoire et la table, `AMBIGUS`). `Memory.prototype.grow` au-delà du maximum : `RangeError : failed to grow memory`. Le reste de la suite (globales `i64` par `BigInt`, détachement du tampon) se comporte comme dans Chrome. Commande : `target/release/soiree-banc machine --navigateur firefox` (10 s, construction comprise, 0,64 Go de mémoire résidente au plus pour `soiree-banc` et ses sous-processus attendus).

## Lot 3c : M9, la mesure 1 refaite par le banc, M10 dans les navigateurs

Mesuré le 2026-11-05, de 22 h 58 à 23 h 36 (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), dans Chrome 156.0.8078.80 (`--headless=new`) et Firefox 158.0.1 (`--headless`), profils neufs, pages isolées (COOP et COEP ; granularité de `performance.now()` mesurée à 5 µs dans Chrome, 20 µs dans Firefox), tout dans un Worker sauf la seconde moitié de M9b. **Safari 27.2 : pas exécuté**, à chaque scénario, avec ce message de `safaridriver` (session refusée au bout de 30 s) : « Could not create a session: The session timed out while connecting to a Safari instance. The following asynchronous operation timed out: Finding or launching a compatible local Safari and waiting for its RWIApplication to appear and become ready. » *Le message dépend du moment* : le 2026-11-06 à 01 h 03 (vérification) et à 01 h 44 (correction), `safaridriver` a refusé tout de suite, avec « Could not create a session: You must enable 'Allow remote automation' in the Developer section of Safari Settings to control Safari via WebDriver. », comme au premier essai du harnais. Le banc relaie le message tel quel ; l'issue, « pas exécuté », ne change pas. L'automatisation à distance n'est pas autorisée sur cette machine ; le banc n'y touche pas (PHASE-3, question 8). La charge est notée à chaque ligne (`vm.loadavg` sur une minute) : de 5 à 10 par d'autres programmes pendant M9 et M1 ; des services de macOS (`fairplaydeviceidentityd`, `mobileassetd`, `duetexpertd`…) l'ont ensuite portée jusqu'à 160 pendant la mesure « sous charge » de M10, et encore 35 pendant sa reprise : les durées de M10 en dépendent. Une seule exécution par mesure, sauf mention.

### M9, première moitié : la profondeur qu'atteignent les jeux

La variante de mesure de la passe de profondeur (`reecriture::reecrire_en_mesurant_la_profondeur`) garde la plus grande valeur de `soiree_profondeur` dans une seconde globale ; le protocole du commun la lit après chaque appel. Chaque module est éprouvé entier, plan standard (deux soirées de trois minutes, soirée longue de six heures, pires cas, sondes, propriétés, gels), sans copie native, sous `ReglagesMoteur::PHASE_1` (4 096 étages permis : rien ne pique). En étages de `soiree_profondeur`, l'appel du moteur exclu. La profondeur ne dépend ni de la machine ni du moteur.

| Module | Profondeur atteinte, épreuve entière | dont au chargement (`Bonjour`, `Decrire`) | Code des contrôles (pour mémoire, sans copie native) |
| --- | --- | --- | --- |
| `devine` | 10 | 7 | 0 |
| `ardoise` | 9 | 7 | 0 |
| Punchline | 15 | 7 | 0 |
| Sidequest | 13 | 7 | 0 |
| Tier list | **18** | 7 | 0 |

De 22 h 58 à 23 h 17, charge de 7,2 à 10,6 ; 1 149 s pour les cinq ; 2,60 Go de mémoire résidente au plus. Commande (depuis `soiree/`, les modules construits au profil `module`, voir `mesures/navigateur/README.md`) :

```sh
CARGO_BUILD_JOBS=4 cargo build --release -p epreuve --example profondeur
target/release/examples/profondeur target/wasm32-unknown-unknown/module/devine_module.wasm target/wasm32-unknown-unknown/module/ardoise_module.wasm ../games/punchline/target/wasm32-unknown-unknown/module/punchline_module.wasm ../games/sidequest/target/wasm32-unknown-unknown/module/sidequest_module.wasm ../games/tier-list/target/wasm32-unknown-unknown/module/tier_list_module.wasm
```

### M9, seconde moitié : la profondeur que tient chaque navigateur

Une récursion sans fin, réécrite sans plafond utile (`profondeur_max` 0x7fff0000), dont chaque étage lit en mémoire et garde vivantes à travers son appel `n` valeurs `i64` : la pile native cède d'abord (`RangeError` dans Chrome, `InternalError: too much recursion` dans Firefox : `Piege::PileNative` à chaque essai), et `soiree_profondeur` dit combien d'étages elle a tenus. Trois essais par cadre sur la même instance ; le plus petit est donné. Jusqu'à 250 valeurs, un appel compte un étage (`reecriture::passes::etages`) : c'est à 250 que la pile tient le moins d'étages. Au-delà, un appel compte plusieurs étages, et le compte de la réécriture suit la pile.

| Valeurs par étage | Chrome, Worker | Chrome, page | Firefox, Worker | Firefox, page |
| --- | --- | --- | --- | --- |
| 0 | 7 936 | 15 475 | 9 235 | 16 792 |
| 16 | 2 646 | 5 159 | 4 313 | 7 840 |
| 64 | 882 | 1 720 | 1 659 | 3 016 |
| 128 | 467 | 911 | 910 | 1 655 |
| **250** | **247** | 480 | **492** | 893 |
| 500 (2 étages par appel) | 250 | 488 | 506 | 918 |
| 1 000 (4) | 252 | 492 | 512 | 932 |
| 4 000 (16 ou 17) | 256 | 496 | 528 | 944 |

Vers 23 h 20, charge 7,7 à 8,6 ; 9 s par navigateur. La pile d'un Worker de Chrome tient environ moitié moins que celle de sa page, et moitié moins que celle d'un Worker de Firefox. **Le Worker le plus court tient 247 étages** ; les 4 096 étages de `PHASE_1` l'auraient laissé épuiser sa pile native avant la faute `Profondeur` du jeu, une panne de l'outil au lieu d'une faute. Le fil principal d'un vrai téléphone (Safari iOS, Chrome Android) n'est pas mesuré : il va sur la fiche humaine (PHASE-3, § 8.5).

Commande : `target/release/soiree-banc etalonner --mesures m9 --navigateurs chrome,firefox,safari`.

**`ReglagesMoteur::PHASE_3`, d'après ces deux moitiés : 64 étages** (`reecriture::Reglages::PHASE_3`) : un facteur 3,5 au-dessus du jeu le plus profond (18), un facteur 3,9 sous le Worker le plus court (247). Il y avait de la place ; ce n'est pas une question au mainteneur.

### La mesure 1, refaite par `soiree-banc etalonner`

La trace de la capture (`capture-navigateur`, refaite sous `PHASE_3` à 23 h 24, 49 s : 116 807 appels pour les cinq modules ; dans wasmtime, aucun écart sur les deux rejeux) rejouée dans un Worker de chaque navigateur, sur `moteur_navigateur::charger` et le protocole du commun, et non plus sur la copie JavaScript du harnais. De 23 h 25 à 23 h 29 (le banc entier : 245 s, les essais de Safari compris), charge de 5,0 à 10,0, de 44 à 66 % de mémoire libre ; un navigateur neuf par module, deux rejeux, le second retenu (comme le harnais) ; **un seul tour**, là où le harnais en avait quatre.

**Le carburant est le même partout** : sur les 116 807 appels, dans les deux rejeux de chaque navigateur, aucun écart de carburant, d'issue ni de retour avec wasmtime ; le module réécrit dans le navigateur a l'empreinte de celui de la capture.

Les cinq modules ensemble, appels d'au moins un million d'unités (395, comme au harnais), « médiane ; 99ᵉ centile ; maximum » :

| | Harnais, quatre tours (2026-11-05, 06 h 55 à 07 h 21, page) | Banc, un tour (23 h 25, Worker) |
| --- | --- | --- |
| wasmtime : ps par unité | 43 ; 82 ; 105 | 40 ; 234 ; 271 |
| Chrome : facteur sur wasmtime | 0,90 à 0,94 ; 1,23 à 1,69 ; 1,71 à 2,97 | 1,07 ; 1,82 ; 3,26 |
| Chrome : ps par unité | 34 à 36 ; 52 à 74 ; 70 à 137 | 39 ; 138 ; 504 |
| Firefox : facteur sur wasmtime | 1,14 à 1,18 ; 3,99 à 4,98 ; 4,42 à 12,63 | 1,49 ; 3,87 ; 13,08 |
| Firefox : ps par unité | 63 à 66 ; 124 à 285 ; 336 à 678 | 73 ; 244 ; 3 600 |

Par module (second rejeu ; « coût fixe » : médiane des deux mille `Bonjour` un par un ; « bloc » : deux mille `Bonjour` d'un bloc, durée moyenne, médiane de trois blocs ; « remise à zéro » : les quatre zones remises à zéro par `Machine::remplir_de_zeros`, dans la mémoire d'une instance, médiane de quinze, sur le Kio) :

| Module | Coût fixe, wasmtime / Chrome / Firefox (µs) | Bloc, Chrome / Firefox (µs) | Remise à zéro, Chrome / Firefox (ns par Kio) | ps par unité ≥ 10⁶, Chrome / Firefox (médiane ; max) | Facteur ≥ 10⁶, Chrome / Firefox (médiane ; max) |
| --- | --- | --- | --- | --- | --- |
| `devine` | 18,9 / 15 / 20 | 15,83 / 19,58 | 7,09 / 9,45 | — | — |
| `ardoise` | 19,1 / 15 / 20 | 16,28 / 17,19 | 7,09 / 9,45 | — | — |
| Punchline | 18,6 / 15 / 20 | 15,02 / 17,29 | 7,09 / 9,45 | 33 ; 36 / 73 ; 86 | 1,18 ; 1,24 / 2,47 ; 2,57 |
| Sidequest | 25,2 / 25 / 20 | 23,20 / 22,65 | 7,96 / 6,37 | 38 ; 48 / 71 ; 103 | 0,94 ; 1,32 / 1,59 ; 3,93 |
| Tier list | 183,2 / 220 / 220 | 184,21 / 175,93 | 7,21 / 6,23 | 40 ; 504 / 74 ; 3 600 | 1,09 ; 3,26 / 1,45 ; 13,08 |

Ce que la mesure 1 du harnais ne disait pas, et que le banc dit : une instance neuve coûte de 20 à 270 µs en médiane (au plus 4,5 ms : une des premières, la compilation paresseuse de Chrome comprise) ; un instantané de 15 à 40 µs (au plus 140 µs, Tier list, 0,7 Mo) ; une restauration de 20 à 250 µs (Tier list, qui remet aussi ses 25 Mio de zones à zéro). La réécriture, dans le navigateur (le commun compilé pour `wasm32`), prend de 52 à 131 ms dans Chrome et de 58 à 164 ms dans Firefox (médiane de cinq) ; la compilation du module réécrit, de 0,4 à 4,8 ms ; la mise en route entière de l'autorité (réécrire, compiler, `Bonjour`, `Decrire`), de 55 à 162 ms, presque tout dans la réécriture. Le harnais compilait un module déjà réécrit : sa « mise en route » (de 2 à 6 ms) ne la comptait pas.

**Retrouve-t-on la mesure 1 ?** *Revu après la vérification* : le paragraphe qui suit jugeait sur un seul tour, et ne retenait du critère que ce qui s'était retrouvé ; la mesure 1 est rejouée en quatre tours, à côté du harnais retiré, dans la section suivante, qui fait foi. Ce qu'on avait écrit : oui pour ce que la mesure 1 a servi à décider :

- le carburant est égal appel par appel (la vérification de D32), comme au harnais ;
- le coût fixe d'un appel est la remise à zéro des zones, et presque rien d'autre : le bloc vaut la remise à zéro à la granularité de l'horloge près (15 à 25 µs pour 2 à 3 Mio, 160 à 185 µs pour les 25 Mio de Tier list) ; la remise à zéro d'un Kio, de 6,2 à 9,5 ns, comme au harnais (6,2 à 9,45) ;
- la durée médiane d'une unité sur les appels lourds : 39 ps dans Chrome (34 à 36 au harnais), 73 dans Firefox (63 à 66), 40 dans wasmtime (43) ; Chrome à la vitesse de wasmtime, Firefox plus lent et surtout irrégulier.

Les queues sont plus hautes qu'au harnais : 99ᵉ centile de Chrome à 138 ps (au plus 74 au harnais), maximum de Firefox à 3 600 ps (678). Elles tombent sur la soirée de Tier list (appels de quelques millions d'unités, dans lesquels une pause du navigateur pèse) et sur ses pires cas ; le second rejeu de wasmtime lui-même y a une queue plus haute que celle du harnais (234 ps au 99ᵉ centile, 82 au harnais), sur la même machine, sous une charge de 5 à 10 au lieu de 2 à 7. Un seul tour ne dit pas ce qui revient d'un lancement à l'autre ; la proposition de budgets de E19 (plus bas) se juge toujours sur les maxima des quatre tours du harnais, que ce tour ne contredit pas en médiane et dépasse en queue dans Firefox.

Commandes : la capture et `soiree-banc etalonner`, dans `mesures/navigateur/README.md` (« Relancer ») ; le détail de chaque page est dans `target/mesures-navigateur/etalonnage/<navigateur>/`.

### La mesure 1 en quatre tours, et le harnais rejoué à côté (correction après la vérification)

La vérification du lot a relevé que les durées d'un seul tour ne se retrouvent pas d'un lancement à l'autre (son tour, à 01 h 0x : médiane de Firefox à 147 ps, de wasmtime à 76 ps), et que le critère avait été lu après coup sur ce qui s'était retrouvé. Le banc joue donc maintenant plusieurs tours (`--tours <n>` : chaque tour rejoue chaque module dans un navigateur neuf, les tours s'enchaînant d'un navigateur à l'autre, comme au harnais). Et pour séparer l'outil de l'état de la machine, **le harnais retiré** (`mesurer.mjs`, `page/banc.js`, `banc-coeur.js`, `trace.js`, `analyser.mjs`, tels que dans le dernier commit, copiés hors du dépôt) **a été rejoué juste après, sur la même capture** : il lit le même `trace.bin` et le même `reecrit.wasm`.

Le 2026-11-06, sur la machine de l'hôte (la même ; Chrome 156.0.8078.80, Firefox 158.0.1, sans interface, profils neufs, pages isolées) : la capture refaite à 01 h 32 (55 s, 0,15 Go ; 116 807 appels, aucun écart dans les deux rejeux de wasmtime) ; le banc de 01 h 33 à 01 h 37 (`soiree-banc etalonner --mesures m9,m1 --tours 4 --navigateurs chrome,firefox`, 197 s, 0,74 Go au plus, navigateurs non comptés), charge de 7,2 à 12,0, de 40 à 64 % de mémoire libre ; le harnais de 01 h 37 à 01 h 41 (`node mesurer.mjs --navigateurs chrome,firefox --tour <n>`, quatre tours, puis `node analyser.mjs --tour <n>`), charge de 7,5 à 9,2 aux deux premiers tours, **16,5 et 27,9** aux deux derniers (d'autres programmes), 45 à 57 % de mémoire libre. Les modules livrés et leurs empreintes réécrites sont ceux du lot (`PHASE_3`).

Les cinq modules ensemble, appels d'au moins un million d'unités (395 par tour), étendue sur les quatre tours de « médiane ; 99ᵉ centile ; maximum », en ps par unité :

| | Harnais, le matin (2026-11-05, 06 h 55 à 07 h 21, page) | Harnais rejoué le soir (01 h 37 à 01 h 41, page) | Banc, quatre tours (01 h 33 à 01 h 37, Worker) |
| --- | --- | --- | --- |
| wasmtime (la capture du moment) | 43 ; 82 ; 105 | 82 ; 179 ; 248 | 82 ; 179 ; 248 (la même capture) |
| Chrome | 34 à 36 ; 52 à 74 ; 70 à 137 | 37 à 48 ; 113 à 142 ; 123 à 527 | 39 à 44 ; 78 à 129 ; **389 à 1 181** |
| Firefox | 63 à 66 ; 124 à 285 ; 336 à 678 | 94 à 143 ; 299 à 402 ; 431 à 900 | 68 à 100 ; 135 à 299 ; **194 à 3 096** |
| Chrome : facteur sur wasmtime | 0,90 à 0,94 ; 1,23 à 1,69 ; 1,71 à 2,97 | 0,82 à 0,95 ; 1,42 à 2,12 ; 1,71 à 3,64 | 0,66 à 0,85 ; 1,15 à 2,25 ; 2,47 à 6,29 |
| Firefox : facteur sur wasmtime | 1,14 à 1,18 ; 3,99 à 4,98 ; 4,42 à 12,63 | 1,32 à 1,66 ; 5,66 à 6,86 ; 7,79 à 10,15 | 0,93 à 1,41 ; 3,54 à 6,92 ; 3,61 à 13,83 |

Ce qui se retrouve, par les deux outils, le même soir :

- **le carburant, l'issue et le retour**, appel par appel : aucun écart sur les 116 807 appels, dans les deux rejeux de chaque tour, dans les deux navigateurs, par le banc comme par le harnais (quarante pages du banc, quarante du harnais), et dans les deux rejeux de wasmtime ;
- **le coût fixe et la remise à zéro** : `Bonjour` un par un de 15 à 25 µs (Tier list, de 170 à 205 µs), le bloc à la remise à zéro près ; la remise à zéro d'un Kio, de 6,4 à 9,5 ns dans Chrome et de 6,4 à 12,7 dans Firefox par le banc, de 6,4 à 10,4 et de 6,4 à 18,7 par le harnais (à la granularité près : un tic de 20 µs de Firefox vaut 9,45 ns par Kio sur 2 Mio de zones) ;
- **la médiane et le 99ᵉ centile d'une unité** : dans Chrome, 39 à 44 ps et 78 à 129 ps par le banc, 37 à 48 et 113 à 142 par le harnais ; dans Firefox, le banc mesure moins que le harnais (68 à 100 contre 94 à 143 ; 135 à 299 contre 299 à 402) : son Worker et ses appels de Rust ne coûtent pas plus que la page du harnais.

Ce qui ne se retrouve pas :

- **les chiffres du matin**, par aucun des deux outils : le harnais rejoué ne retrouve ni ses médianes de Firefox (94 à 143 ps contre 63 à 66), ni ses 99ᵉ centiles de Chrome (113 à 142 contre 52 à 74), ni ses maxima ; wasmtime lui-même, dans la capture du soir, a une médiane de 82 ps (43 le matin ; 40 à 23 h 24, 76 pour la vérification à 01 h 0x). Une durée d'unité, même en médiane, dépend de l'état de la machine d'un facteur deux ; sur ce M5, où quatre cœurs de performance côtoient six cœurs d'efficacité, c'est attendu sous une charge de 8 à 12, mais ce n'est pas mesuré ici ;
- **les maxima** : ceux du banc dépassent ceux du harnais rejoué à côté dans Chrome, à chaque tour (389, 437, 785 et 1 181 ps contre 123, 183, 237 et 527) ; dans Firefox, ils s'étalent davantage (194 à 3 096 contre 431 à 900). Chaque maximum du banc dans Chrome, et trois sur quatre dans Firefox, tombe sur un appel de la soirée de Tier list (de 1 à 3 millions d'unités, environ 100 µs de calcul pour 170 à 240 µs de remise à zéro de 25 Mio) : une pause ou une remise à zéro lente d'une milliseconde y pèse 400 à 800 ps par unité (le quatrième de Firefox, 356 ps au troisième tour, sur les pires cas de Sidequest, dont la médiane monte ce tour-là à 163 ps : un tour lent en entier). Que le Worker du banc en subisse davantage que la page du harnais n'est ni expliqué ni exclu ; sur les pires cas de Tier list (calcul seul, plusieurs millions d'unités), le banc mesure dans Chrome de 33 à 76 ps par unité, au plus 117.

**Le critère du § 7, « `etalonner` retrouve la mesure 1 du harnais », n'est donc tenu qu'en partie** : tenu pour le carburant, le coût fixe, la remise à zéro, la médiane et le 99ᵉ centile ; non tenu pour les maxima. Les décisions que la mesure 1 a prises sur les médianes (Chrome à la vitesse de wasmtime, Firefox plus lent et irrégulier, la remise à zéro comme coût fixe) restent vraies ; celles qui reposent sur un maximum (la proposition de budgets de E19, plus bas) ne tiennent pas sur une seule soirée.

Commandes (depuis `soiree/`, après la capture) : `target/release/soiree-banc etalonner --mesures m1 --tours 4 --navigateurs chrome,firefox`. Le harnais n'est plus dans l'arbre : `git show <commit>:mesures/navigateur/<fichier>` pour les six fichiers ci-dessus, dans un dossier dont `target/mesures-navigateur` (et, pour `analyser.mjs`, `target/wasm32-unknown-unknown` et `../games`) mènent à ceux du commun ; il a été pris au commit `44160f9`. Le texte entier des deux analyses est dans le dossier de travail de la session, non conservé : les chiffres ci-dessus en sont tirés ; les rapports bruts du banc sont dans `target/mesures-navigateur/etalonnage/<navigateur>/tour-<n>/`, ceux du harnais dans `target/mesures-navigateur/resultats/`.

**M9 refait le même soir**, avec les récursions maintenant aussi chargées sous `PHASE_3` (01 h 33, puis 01 h 42 après le contrôle négatif) : les mêmes étages tenus qu'au lot (Worker de Chrome 247, page 480 ; Worker de Firefox 492, page 893), et **chaque récursion, de 0 à 4 000 valeurs par étage, chargée sous `PHASE_3` par `moteur_navigateur::charger`, est la faute `Profondeur`**, dans le Worker comme sur la page, dans les deux navigateurs. Contrôle négatif : chargées sous `PHASE_1` (4 096 étages), elles épuisent la pile native du Worker de Chrome dès 16 valeurs par étage, et celle de sa page dès 64 (`panne de l'outil (Interne) : pile native épuisée avant soiree_profondeur`) : treize défauts, code 1. Les résultats bruts sont dans `target/mesures-navigateur/etalonnage/<navigateur>/m9.json` (ceux du lot n'y étaient pas : ni la vérification ni cette correction ne les ont retrouvés, et les chiffres refaits sont les mêmes).

### La mesure 1 sur machine calme, les deux outils en alternance (fin du lot)

Les tours de la section précédente avaient été pris sous une charge de 7 à 28. Ceux-ci le sont sur une machine calme : chaque tour passe par le portillon de l'hôte en mode calme (`target/hote/lourd --calme`, qui attend une charge sur une minute sous 3 et une mémoire libre d'au moins 25 %, puis lance la commande en priorité basse), et les deux outils alternent, un tour du harnais retiré puis un tour du banc : huit tours valides chacun, tous ceux qui ont été pris (aucun écarté). Le 2026-11-06, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2), Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface, profils neufs, pages isolées ; la capture de 01 h 32 (`PHASE_3`) ; le banc construit à 03 h 02 (l'exemple `etalonnage-navigateur`, `sha256` `7fc15e87314d4f4b…` ; le code de M1 est celui de cette fin du lot). Tours 1 et 2 de 04 h 00 à 04 h 08 (GMT), tours 3 à 8 de 17 h 43 à 17 h 58. Charge au départ et à la fin de chaque tour, notée par le portillon : harnais 2,90 → 5,09 ; 2,64 → 4,86 ; 1,74 → 2,52 ; 2,96 → 3,23 ; 2,79 → 4,56 ; 2,80 → 4,73 ; 2,66 → 3,25 ; 2,83 → 3,10 ; banc 2,65 → 4,03 ; 2,93 → 5,54 ; 2,54 → 4,02 ; 2,50 → 3,90 ; 2,70 → 3,31 ; 2,95 → 3,60 ; 2,90 → 3,23 ; 2,97 → 3,26 (la charge monte pendant un tour : le navigateur lui-même en fait une part). Mémoire libre de 36 à 67 %.

Les cinq modules ensemble, appels d'au moins un million d'unités (395 par tour), « médiane ; 99ᵉ centile ; maximum », en ps par unité :

| Tour | Harnais, Chrome | Banc, Chrome | Harnais, Firefox | Banc, Firefox |
| --- | --- | --- | --- | --- |
| 1 | 43 ; 105 ; 174 | 38 ; 75 ; 501 | 66 ; 120 ; 215 | 68 ; 124 ; 247 |
| 2 | 40 ; 67 ; 149 | 49 ; 100 ; 156 | 69 ; 122 ; 181 | 75 ; 179 ; 3 814 |
| 3 | 36 ; 60 ; 108 | 46 ; 87 ; 310 | 63 ; 120 ; 173 | 65 ; 120 ; 402 |
| 4 | 38 ; 95 ; 175 | 40 ; 82 ; 295 | 74 ; 135 ; 292 | 71 ; 120 ; 154 |
| 5 | 37 ; 71 ; 158 | 36 ; 67 ; 318 | 83 ; 144 ; 269 | 70 ; 120 ; 152 |
| 6 | 42 ; 97 ; 150 | 37 ; 82 ; 84 | 68 ; 107 ; 300 | 66 ; 127 ; 304 |
| 7 | 37 ; 75 ; 149 | 36 ; 64 ; 291 | 69 ; 130 ; 172 | 71 ; 207 ; 323 |
| 8 | 37 ; 95 ; 156 | 38 ; 72 ; 359 | 68 ; 120 ; 179 | 65 ; 120 ; 267 |
| **Étendue** | 36 à 43 ; 60 à 105 ; 108 à 175 | 36 à 49 ; 64 à 100 ; **84 à 501** | 63 à 83 ; 107 à 144 ; 172 à 300 | 65 à 75 ; **120 à 207** ; **152 à 3 814** |

wasmtime (la capture) : 82 ; 179 ; 248. Aucun écart de carburant, d'issue ou de retour, et aucune panne, sur les 116 807 appels, dans les deux rejeux de chaque tour, dans les deux navigateurs, par les deux outils (quatre-vingts pages de chacun : un navigateur neuf par module, par tour et par navigateur).

Ce qui se lit :

- **Les médianes se retrouvent**, à quelques ps près : Firefox, banc 65 à 75 dans l'étendue du harnais (63 à 83) ; Chrome, banc 36 à 49 contre 36 à 43, six tours sur huit dedans, **deux au-dessus** (46 et 49 ps, aux tours 3 et 2). Sur machine calme, les médianes de Firefox au harnais (63 à 83) redescendent vers celles du matin du 2026-11-05 (63 à 66), au lieu des 94 à 143 du soir chargé.
- **Le 99ᵉ centile se retrouve dans Chrome** (64 à 100 contre 60 à 105), **pas dans Firefox** (120 à 207 contre 107 à 144 : deux tours sur huit au-dessus).
- **Les maxima ne se retrouvent pas.** Dans Chrome, six tours sur huit du banc dépassent le plus haut du harnais (291 à 501 ps contre au plus 175), tous sur un appel de la soirée de Tier list, comme sous charge ; le harnais n'y dépasse jamais 175. Dans Firefox, le plus haut, 3 814 ps au deuxième tour, tombe sur la soirée de Tier list ; et dans six tours sur huit, un appel lourd de Punchline y monte de 247 à 432 ps (au plus 87 au harnais, à chacun de ses tours) : une différence qui revient, non expliquée ici, entre le Worker du banc et la page du harnais.
- La machine calme ne referme donc pas l'écart des queues : il ne venait pas seulement de la charge. **Le critère « `etalonner` retrouve la mesure 1 du harnais » n'est pas tenu pour les queues**, ni sous charge ni au calme. Par décision de l'agent principal pour l'hôte, le critère du lot 3c devient « zéro écart de carburant, d'issue et de retour, et médianes dans l'étendue du harnais d'un tour à l'autre » ; les queues se remesurent au lot 13 sur machine calme, et la question va au mainteneur (PHASE-3, question 27). Ce critère est tenu dans Firefox, et dans Chrome à deux tours près sur huit (46 et 49 ps, au-dessus de 43) : c'est dit, pas arrondi.
- **« Sous 3 » ne vaut que pour le départ** (seconde vérification de la fin du lot), et c'est la règle de `lourd --calme` : pendant chaque tour, la charge sur une minute monte, jusqu'à 5,54. Relevée par le banc avant et après chaque module, la plus haute pendant la partie Chrome d'un tour du banc : 3,95 ; **5,26** ; **5,22** ; 2,50 ; 3,92 ; 3,12 ; 2,99 ; 3,14 (tours 1 à 8) : les deux tours au-dessus de 5 sont ceux des deux médianes de Chrome au-dessus de l'étendue (49 et 46). Relevée par le harnais au départ de chaque module seulement (une autre lecture, pas la même mesure), la plus haute pendant sa partie Chrome : 4,34 ; 2,75 ; 2,24 ; 2,97 ; 4,33 ; 3,06 ; 2,66 ; 3,00, pour des médianes de 43, 40, 36, 38, 37, 42, 37, 37 : son tour 5, à 4,33, reste à 37. Un lien entre la charge pendant la partie Chrome et la médiane n'est donc ni établi ni exclu ; ce n'est pas une mesure sous charge au sens de la règle, mais ce n'est pas une machine sous 3 pendant toute la mesure. Aucun tour n'est écarté pour cela : la règle de validité n'était pas posée avant les tours.
- **Les tours 1 et 2 ne sont pas passés par le portillon de l'hôte** (`target/hote/lourd`, écrit à 17 h 38), mais par une copie que le premier agent avait mise dans son dossier temporaire, disparue au redémarrage : elle ne peut pas lui être comparée. Ses lignes de sortie ont la forme exacte de celles du portillon (« j'attends », « départ … mesure sur machine calme », « fin »), et les charges au départ qu'elle a notées (harnais 2,90 et 2,64 ; banc 2,65 et 2,93) sont confirmées par les relevés indépendants du banc (2,65 et 2,93) et du harnais (2,64 au tour 2), sauf au premier module du tour 1 du harnais, où le harnais lit 3,07. Leurs charges sur cinq et quinze minutes allaient de 3,9 à 5,2, et le Firefox du harnais, au tour 1, a tourné sous une charge sur une minute d'environ 5,5. Sans ces deux tours, le verdict ne change pas : médianes de Chrome, banc 36 à 46 contre 36 à 42 (au-dessus) ; de Firefox, banc 65 à 71 contre 63 à 83 (dedans).

Commandes (depuis `soiree/`, en alternant) : `target/hote/lourd --calme target/release/soiree-banc etalonner --mesures m1 --navigateurs chrome,firefox --sans-construire`, puis copier `target/mesures-navigateur/etalonnage/{chrome,firefox}` et `analyse.json` à part ; et, dans un dossier où le harnais est repris du commit `44160f9` (`git archive 44160f9 mesures/navigateur`) avec un lien `target` vers `soiree/target` et un lien `../games` (pour `analyser.mjs`), `target/hote/lourd --calme node mesurer.mjs --navigateurs chrome,firefox --tour <n>`, puis `node analyser.mjs --tour <n>`. Le dossier de travail de cette fin du lot est `target/travail/lot-3c/` (le harnais, `comparer.py` qui tire le tableau ci-dessus des analyses) ; les sorties, les analyses et l'empreinte de chaque tour sont dans `target/preuves/lot-3c/calme-harnais-<n>/`, `calme-banc-<n>/`, `harnais-analyse/` et `comparaison-8-tours.txt` ; les rapports bruts du banc dans `target/mesures-navigateur/etalonnage-calme/tour-<n>/`, ceux du harnais dans `target/mesures-navigateur/resultats/` (hors dépôt).

### M10 dans les navigateurs : une unité sur une boucle vide

Le module de `Bonjour` qui boucle chargé par `moteur_navigateur::charger` jusqu'à sa faute `Budget` (`BUDGET_DE_CHARGEMENT` unités), moins un témoin qui piège dès l'entrée : quinze chargements, « médiane ; minimum ; maximum » en ps par unité. Puis la même boucle par la machine seule, sous un plus grand budget, sur une seule instance (le premier appel y est le plus lent : le code de base du moteur, avant son compilateur optimisant).

| Navigateur (heure, charge) | Chargement, 2²⁶ unités | Chargement, 2²⁴ unités | Machine seule |
| --- | --- | --- | --- |
| Chrome (23 h 29, 9,1) | 415 ; 410 ; 1 880 (boucle au plus 126,5 ms) | — | 2³⁰ : 444 ; 426 ; 454 |
| Firefox (23 h 29, 7,6) | 1 699 ; 1 029 ; 2 031 (au plus 136,9 ms) | — | 2³⁰ : 575 ; 557 ; 1 808 |
| Chrome sous charge (23 h 31, 46 puis 106) | 416 ; 412 ; 1 886 (au plus 126,7 ms) | — | 2²⁸ : 419 ; 416 ; 427 |
| Firefox sous charge (23 h 32, 106 puis 161) | **4 578 ; 3 357 ; 6 092 (au plus 409,9 ms)** | — | 2²⁸ : 2 023 ; 1 725 ; 3 546 |
| Chrome, `BUDGET_DE_CHARGEMENT` baissé (23 h 36, 38) | — | 418 ; 409 ; 1 787 (au plus 30,2 ms) | 2²⁸ : 409 ; 407 ; 413 |
| Firefox, `BUDGET_DE_CHARGEMENT` baissé (23 h 36, 38 puis 35) | — | 1 765 ; 558 ; 2 116 (au plus 35,8 ms) | 2²⁸ : 509 ; 505 ; 1 726 |

« Sous charge » : quatorze `yes > /dev/null` lancés 5 s avant, comme au lot 2 ; mais des services de macOS tournaient en même temps (voir l'en-tête), et la charge sur une minute est montée jusqu'à 161 : ce n'est pas la charge de quatorze `yes` seuls, et elle n'est pas reproductible telle quelle.

Ce qui se lit :

- **Dans Chrome, une unité de boucle vide coûte ce qu'elle coûte dans wasmtime peu chargé** (environ 415 ps en médiane), même sous charge ; un chargement sur quinze, le premier, monte à 1 900 ps.
- **Dans Firefox, un chargement tourne dans le code de base de SpiderMonkey** : 1 700 ps en médiane, quatre fois l'unité de Chrome et trois fois celle de la même boucle une fois optimisée (575) ; et jusqu'à **6 092 ps sous charge**. 2²⁶ unités y ont pris 410 ms : deux fois et demie sous un garde-fou d'une seconde, et non dix, la marge que le lot 2 avait retenue pour wasmtime.
- **Le garde-fou de chargement, revu** : `BUDGET_DE_CHARGEMENT` baisse de 2²⁶ à **2²⁴** (P5, la voie préférée). Au pire mesuré (6 092 ps), 2²⁴ unités prennent 102 ms (calculé) : dix fois sous la seconde ; mesuré après la baisse, sous une charge de 35 à 38, au plus 30 ms dans Chrome et 36 ms dans Firefox. `Decrire` en brûle au plus 9 322 : il lui reste un facteur 1 800. Le garde-fou mural lui-même (une seconde dans wasmtime ; à écrire dans le Worker au lot 10a) ne change pas.
- **`Creer` ne passe pas par ce budget** : il se joue sous `BUDGETS_DE_SOIREE.creation`, 2²⁸ unités. Une boucle dans `Creer` y prendrait au plus 400 ms dans wasmtime chargé (lot 2), et, au pire mesuré dans Firefox (6 092 ps, code de base), 1,64 s (calculé) : sous le garde-fou de partie de 2 s que la page du Worker tiendra (`GARDE_FOU_PARTIE_MS`, choisi), avec un facteur 1,2 seulement. Ce lot ne change pas ce budget (`Creer` de Sidequest en brûle 343 002, 780 fois moins) : il est à revoir avec le garde-fou de partie, au lot 10a, ou par le mainteneur (PHASE-3, question 4).

Commandes : `target/release/soiree-banc etalonner --mesures m10 --navigateurs chrome,firefox` ; sous charge, lancer d'abord quatorze `yes > /dev/null &`, puis `--etiquette <navigateur>-sous-charge --budget 268435456`, et les arrêter après.

## Lot 5 : M11 (une case de garde) et M15 (le relais à 71 liens)

### M11 : le coût d'une case de garde

Le 2026-11-06 à 21 h 39 (GMT), MacBook Apple M5, 16 Go, macOS 27.2 (26B123), disque interne APFS (`/dev/disk3s1s1`, « Solid State : Yes », protocole Apple Fabric) ; `lourd --calme` (charge 2,65 au départ, 2,52 à la fin, 51 % de mémoire libre). Chaque case est écrite comme le relais l'écrit, `relais::garde::ecrire_durable` : fichier temporaire, `File::sync_all`, renommage, dossier ouvert et synchronisé. 64 écritures par taille (8 pour le point), en microsecondes, « médiane ; 99ᵉ centile ; maximum » :

| Taille | Écrite et synchronisée | Sans synchronisation (écrite, renommée) |
|---|---|---|
| 100 o (une case de journal) | 8 000 ; 9 407 ; 13 188 | 244 ; 714 ; 825 |
| 4 Kio | 8 872 ; 9 799 ; 12 659 | 234 ; 640 ; 766 |
| 64 Kio | 8 073 ; 9 061 ; 9 070 | 255 ; 725 ; 830 |
| 1 Mio | 9 951 ; 10 276 ; 11 027 | 434 ; 1 141 ; 3 321 |
| 17 625 193 o (le point de Tier list au pire, M3) | 18 080 ; 20 937 ; 25 951 | 3 274 ; 6 137 ; 7 015 |

Et, sur le même disque, le même soir, par Python (`os.fsync`, puis `fcntl(fd, F_FULLFSYNC)`), sur un fichier de 100 o et de 64 Kio : `fsync` seul 22 et 37 µs en médiane (au plus 25 et 143) ; `F_FULLFSYNC` 3 873 et 3 769 µs (au plus 5 710 et 4 667). Une case synchronisée coûte deux `F_FULLFSYNC` (le fichier, le dossier) : **`File::sync_all` de `std` fait bien `F_FULLFSYNC` sous macOS**, constaté par son coût (la source de `std` n'est pas installée sur la machine pour le lire) ; la synchronisation d'un dossier ouvert en lecture réussit.

Ce qu'elle remplace : « plusieurs ms seraient plausibles » (§ 2.4) ; la cadence de 100 ms tient (au plus dix cases par seconde, moins de 90 ms de disque par seconde, sur le fil de la garde du relais, pas sur celui des salles) ; la fourchette choisie de la simulation (1 à 20 ms, `Pannes::SOIREE`) couvre une case de journal, pas tout à fait le pire point (26 ms au plus, une case toutes les 10 s).

Commande : `cargo build --release -p relais --example mesure_garde`, puis `target/hote/lourd --calme target/release/examples/mesure_garde <dossier> --fois 64` et `target/hote/lourd --calme python3 -I target/travail/lot-5/fsync.py <dossier>` (preuve : `target/preuves/lot-5/m11/`).

### M11, refaite après la vérification du lot 5

La mesure ci-dessus a été prise avec un `mesure_garde` construit avant `cargo fmt`, qui écrivait par `ecrire_durable` des fichiers nommés `<n>-j`. Après la vérification, la garde tient une case par numéro (la sorte dans le premier octet du fichier, PHASE-3, § 2.4), et `mesure_garde` écrit chaque case par `relais::garde::Dossier::ecrire`, comme le relais : l'octet de sorte et les octets dans un fichier temporaire, `File::sync_all`, renommage, dossier synchronisé. Refaite le 2026-11-06 à 23 h 10 (GMT), même machine (MacBook Apple M5, 16 Go, macOS 27.2, 26B123), même disque (`/dev/disk3s1s1`, APFS, « Solid State : Yes », Apple Fabric) ; `lourd --calme` (charge 2,71 au départ et à la fin, 48 % de mémoire libre ; le portillon a d'abord attendu une charge de 3,09) ; `mesure_garde` de sha256 `298e1640…`. 64 écritures par taille (8 pour le point), en microsecondes, « médiane ; 99ᵉ centile ; maximum » :

| Taille | Écrite et synchronisée | Sans synchronisation (écrite, renommée) |
|---|---|---|
| 100 o (une case de journal) | 8 010 ; 9 005 ; 10 720 | 255 ; 629 ; 843 |
| 4 Kio | 8 994 ; 9 993 ; 10 005 | 275 ; 434 ; 438 |
| 64 Kio | 9 185 ; 10 211 ; 10 217 | 291 ; 744 ; 870 |
| 1 Mio | 9 900 ; 11 052 ; 12 102 | 470 ; 918 ; 5 567 |
| 17 625 193 o (le point de Tier list au pire, M3) | 19 200 ; 20 024 ; 20 162 | 3 490 ; 4 762 ; 6 899 |

Par Python, le même soir : `fsync` seul 19 et 28 µs en médiane (100 o, 64 Kio ; au plus 27 et 44) ; `F_FULLFSYNC` 3 853 et 3 781 µs (au plus 9 378 et 4 846). Ce qui se lit est inchangé : une case coûte deux `F_FULLFSYNC`, de 8 à 9 ms en médiane pour une case de journal, au plus 12 ms ; le pire point 19 ms en médiane, au plus 20 ms. La cadence de 100 ms tient.

Commande : `cargo build --release -p relais --example mesure_garde`, puis `target/hote/lourd --calme target/release/examples/mesure_garde <dossier> --fois 64` et `target/hote/lourd --calme python3 -I target/travail/lot-5/fsync.py <dossier>` (preuve : `target/preuves/lot-5/correction/m11/`).

### M15 : les fils et la mémoire du relais à 71 liens

Le 2026-11-06, de 21 h 43 à 21 h 53 (GMT), même machine, pendant `soiree-banc soiree-native` de Sidequest à 71 téléphones, 10 minutes (graine 2, deux `kill -9` du relais), charge 2,08 au départ et 4,05 à la fin (le banc, ses 71 téléphones, son mandataire et l'autorité tournent sur la même machine). Le banc lit le relais toutes les 5 s (`ps -M -p <pid>`, `ps -o rss= -p <pid>`) ; le maximum sur les trois relais successifs : **153 fils**, **14 784 Kio résidents** (14,4 Mio). Les 153 fils : deux par lien (71 téléphones, les onglets jumeaux du moment, l'autorité et sa garde), le fil des salles, celui de la garde, les fils d'écoute ; l'estimation du § 2.5 (environ 150) tient, et la mémoire ne gêne pas : rien ne demande le fil de lecture commun par `poll`. Dans la soirée de Punchline à 8 téléphones : 27 fils, 10 320 Kio. Ce n'est pas une mesure de temps.

Commande : `target/release/soiree-banc soiree-native ../games/sidequest/target/wasm32-unknown-unknown/module/sidequest_module.wasm --telephones 71 --duree 600 --graine 2 --tuer-le-relais 2 --calme 120` (preuve : `target/preuves/lot-5/soiree-native-sidequest-71/`).

Relue après la vérification (2026-11-06, de 23 h 00 à 23 h 10, GMT, même commande avec `--travail`, le relais à sa garde d'un fichier par numéro, charge 1,73 au départ et 3,09 à la fin) : **153 fils**, **14 736 Kio** au plus ; à 8 téléphones (Punchline) : 27 fils, 15 696 Kio (preuves : `target/preuves/lot-5/correction/soiree-native-sidequest-71/`, `soiree-native-punchline/`).

## Lot 6 : le coût d'un nœud de scène, avant et après l'ABI 4

Le carburant compte les instructions du module réécrit (sous `ReglagesMoteur::PHASE_3`) : il est le même sur toute machine, à tout moment ; ce n'est pas une mesure de temps, et elle n'a pas besoin d'une machine calme. Mesuré le 2026-11-06 (avant, 23 h 27 GMT, sur l'arbre du commun à `320b9b1`, propre) et le 2026-11-07 (après, 00 h 00 GMT, sur l'arbre final du lot 6), MacBook Apple M5, rustc 1.99.1, par `epreuve/examples/cout_d_un_noeud.rs` : la vue de l'écran d'`ardoise` (`Regarder`), lue après chaque trait écrit ; chaque trait est un `Rect`, et l'écart d'un trait au suivant est le coût d'un nœud (sa clé `Cle::de(&("trait", i))`, sa copie, son encodage, celui de la réponse). Le premier trait ajoute aussi le bouton « Annuler » et ne compte pas.

| | Module `ardoise` (sha256) | Vue sans trait | Un `Rect`, du 2ᵉ au 24ᵉ trait : médiane ; moyenne ; de … à … |
|---|---|---|---|
| Avant (ABI 3) | `a77e81f5…` | 61 102 | 2 395 ; 2 451 ; 2 373 à 2 989 |
| Après (ABI 4) | `036799a4…` | 62 535 | 2 395 ; 2 455 ; 2 342 à 3 051 |

L'ABI 4 ne change pas le coût d'un nœud (même médiane) ; la vue entière d'`ardoise` coûte 1 433 unités de plus (2,3 %), une fois par scène : `SorteNoeud` a grandi (la boîte d'un composant), et `Scene` porte un nœud de secours de cette taille. Le chiffre de RESTE-PHASE-2 (point 12 : « de 4 500 à 4 800 unités ») date de la phase 1 et du compte par fonction : un rectangle coûte aujourd'hui environ 2 400 unités ; trois écarts dépassent nettement la médiane, aux 4ᵉ, 15ᵉ et 20ᵉ traits, avant (2 721, 2 989, 2 755) comme après (2 743, 3 051, 2 768) ; leur cause n'est pas cherchée ici. Le réduire attend les budgets (question 4).

Commande : `cargo build -p ardoise-module --target wasm32-unknown-unknown --profile module`, puis `target/hote/lourd cargo run --release -p epreuve --example cout_d_un_noeud -- <ardoise_module.wasm>` (preuves : `target/preuves/lot-6/cout-noeud-avant/`, `cout-noeud-apres/`).

**Le coût d'une page** (carburant, relevé par E19 sur les `Page` que E21 appelle, 2026-11-07, plan court des jeux, plan standard de `devine`) : `devine`, 128 pages, 12 156 en médiane, au plus 29 823 ; la page vide de Sidequest (1 024 pages) au plus 26 749 ; la page vide de Punchline (1 024 pages) 32 039 en médiane, 707 303 au 99ᵉ centile, au plus 722 971 : le décodage de la vue de Punchline dans la colle. Avec son coût fixe (155 800), Punchline prend déjà 67 % du budget d'une page (1 302 083, une lecture divisée par une marge de téléphone de 4, choisie) avant d'écrire la sienne. Commande : `target/release/soiree-epreuve <jeu> --court` (preuves : `target/preuves/lot-6/court-*/`, `devine-standard/`).

## Ce que ces mesures ne disent pas

- **L'ordinateur de la soirée.** Toutes les durées sont celles d'un M5 récent. Le CONTRAT ne décrit pas encore l'ordinateur de la soirée (« Le matériel : à renseigner ») ; le banc se relance sur lui tel quel (README).
- **Safari et les téléphones** : non mesurés (voir plus haut ; au lot 3c, le banc pilote Safari, mais `safaridriver` refuse la session).
- **Le rendu, le relais, le décodage** : la page du banc n'a que le module. Ce que le commun prendra sur l'autre moitié de l'image n'est pas mesuré.
- **Un appel isolé de ses pauses** : les maxima par appel comptent les pauses du navigateur.
- **La charge** : d'autres agents tournaient (charge de 2,3 à 6,6) ; les tours 1 à 4 donnent une idée de ce qui bouge, pas une machine au repos.

## Proposition de budgets de E19 et de marge, pour le mainteneur

Aujourd'hui (`crates/epreuve/src/budgets.rs`), une unité de carburant est comptée `PS_PAR_UNITE` × `MARGE_NAVIGATEUR` × `MARGE_MACHINE` = 200 × 2 × 2 = **800 ps**, trois nombres choisis. Les deux premiers se remplacent maintenant par une mesure dans les navigateurs ; le troisième attend l'ordinateur de la soirée.

1. **Remplacer `PS_PAR_UNITE` × `MARGE_NAVIGATEUR` par la durée d'une unité mesurée dans le navigateur visé**, au maximum de tous les appels d'au moins un million d'unités des quatre tours (le CONTRAT juge chaque image, pas un centile) :
   - **Chrome seul** : 137 ps au plus → **150 ps** ;
   - **Chrome et Firefox** : 678 ps au plus (Firefox, une vue de l'écran d'un pire cas) → **700 ps** ; ou, si le mainteneur accepte de juger le pire cas au 99ᵉ centile de Firefox (de 124 à 285 ps selon le tour), **300 ps**.
2. **Garder `MARGE_MACHINE` à 2** tant que le banc n'a pas tourné sur l'ordinateur de la soirée : aucune mesure ne la remplace encore.
3. **Le coût fixe** : la remise à zéro est mesurée de 6,2 à 7,0 ns par Kio dans les navigateurs (9,45 au plus pour un `fill(0)` seul, à 20 µs de granularité près) : garder `PS_PAR_KIO_REMIS_A_ZERO` à 10 ns. Le reste du fixe est de 0,3 à 0,5 µs dans les navigateurs, contre `FIXE_NS` = 10 µs choisis pour wasmtime et son fil : le garder à 10 µs laisse de la place au décodage du commun ; le baisser à 2 µs serait encore quatre fois au-dessus de la mesure.

Ce que cela donne, avec la part du jeu et la part des entrées inchangées (la moitié chacune), en unités de carburant :

| Unité comptée | `pire_cas` et `image_max` (33 ms) | `lecture` et `image_courante` (1/60 s) | Par rapport à aujourd'hui |
| --- | --- | --- | --- |
| aujourd'hui : 200 × 2 × 2 = 800 ps | 10 312 500 | 5 208 333 | — |
| Chrome seul : 150 × 2 = 300 ps | 27 500 000 | 13 888 889 | × 2,67 |
| Chrome et Firefox, au 99ᵉ centile : 300 × 2 = 600 ps | 13 750 000 | 6 944 444 | × 1,33 |
| Chrome et Firefox, au maximum : 700 × 2 = 1 400 ps | 5 892 857 | 2 976 190 | × 0,57 |

Le coût fixe d'un appel, en unités, suit l'unité comptée : chez Tier list (25 668 Kio de zones), `(10 µs + 25 668 × 10 ns) / 150 ps` ≈ 1 778 000 unités au lieu de 1 333 400 aujourd'hui.

Une lecture directe, sans passer par l'unité : les vues de l'écran des pires cas de Tier list (jusqu'à 7 289 169 unités) durent au plus 1,16 ms dans Chrome et 4,96 ms dans Firefox sur cette machine, pour une part de 8,25 ms. Dans Firefox, il ne reste donc qu'un facteur 1,66 pour une machine plus lente que ce M5 ; dans Chrome, un facteur 7.

**Ma proposition** : décider d'abord **quels navigateurs l'ordinateur de la soirée doit tenir** (une question du CONTRAT, « Le matériel »). Si c'est Chrome, passer à 150 ps × `MARGE_MACHINE` 2 : les budgets de E19 grandissent de 2,67, ce qui rendrait à Tier list de la place pour ses tuiles (`TUILES_MAX`, revue de la phase 2, constat 10). Si Firefox doit tenir aussi, garder les budgets d'aujourd'hui (800 ps) : ils couvrent le maximum des quatre tours du harnais du matin dans Firefox (678 ps), mais ne laissent alors qu'un facteur 1,2 pour une machine plus lente que ce M5 ; ou passer à 1 400 ps si cette marge paraît trop mince. *Contredit depuis par le lot 3c* : le même soir, sur la même machine, le maximum dans Firefox est monté à 900 ps par le harnais rejoué, et de 194 à 3 096 ps selon le tour par le banc (1 103 ps au tour de la vérification) ; dans Chrome, jusqu'à 527 ps par le harnais et 1 181 par le banc. Ces maxima tombent sur des appels de la soirée de Tier list où une pause pèse plus que le carburant (« La mesure 1 en quatre tours »), et changent d'un facteur trois d'une soirée à l'autre : **aucun maximum d'une seule soirée ne peut servir d'unité** ; cette proposition n'est pas refaite ici, et la question 4 reste au mainteneur, à trancher sur plusieurs soirées ou sur un centile. Dans tous les cas, mesurer Safari (une case à cocher) avant de trancher, et relancer le banc sur l'ordinateur de la soirée pour remplacer `MARGE_MACHINE`.

## Lot 8 : la durée de E16, qui mesure les textes

Mesuré le 2026-11-07 de 03 h 50 min 43 s à 03 h 50 min 50 s (GMT, d'après `target/preuves/lot-8/mesure-e16/code.txt` ; *corrigé après la vérification :* la première rédaction disait « vers 03 h 55 »), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1), machine calme : le portillon (`target/hote/lourd --calme`) a écrit une charge de 1,91 au départ comme à la fin, 49 % de mémoire libre. `soiree-epreuve` en release, sur le module que le plan standard du lot 8 a éprouvé ; cinq essais par jeu, le processus entier (chargement et compilation du module par wasmtime compris), `E16` seul : les pires cas construits sur une instance nue, leur scène validée, leurs textes mesurés par `mise-en-page`.

| Jeu (module) | Pires cas | Textes mesurés | Durée des cinq essais |
|---|---|---|---|
| Punchline (`82669f77…`) | 10 | 142 | 0,48 ; 0,44 ; 0,44 ; 0,44 ; 0,44 s |
| Sidequest (`62043526…`) | 6 | 426 | 0,39 ; 0,38 ; 0,38 ; 0,38 ; 0,38 s |
| Tier list (`84315a1b…`) | 4 | 1 151 | 0,47 ; 0,46 ; 0,46 ; 0,46 ; 0,46 s |

E16 reste un contrôle rapide : moins d'une demi-seconde par jeu, processus compris, devant 130 à 575 s pour le plan standard entier. La part de la mesure seule n'est pas séparée (aucune option de l'épreuve ne joue les pires cas sans mesurer). Pour refaire : `target/hote/lourd --calme target/travail/lot-8/mesure-e16.sh` (le script est hors dépôt ; il lance `target/release/soiree-epreuve --wasm <module> --controles E16` cinq fois par jeu). La preuve : `target/preuves/lot-8/mesure-e16/`.

## Lot 9 : M8, le rendu de la télé

Mesuré le 2026-11-07 de 06 h 10 min 09 s à 06 h 10 min 34 s (GMT, `target/preuves/lot-9/dessin/code.txt`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), par `target/hote/lourd --calme` : charge de 1,48 au départ (sous 3, machine calme), **4,14 à la fin** (les navigateurs du scénario eux-mêmes, aucune compilation pendant : le paquet est construit avant, `--sans-construire`), 47 puis 45 % de mémoire libre. Chrome 156.0.8078.80 sans interface (WebGPU, puis WebGL2 forcé, sur ANGLE Metal), Firefox 158.0.1 sans interface (WebGL2 : il n'expose pas WebGPU sans interface sur cette machine) ; Safari 27.2 pas exécuté (`safaridriver` refuse la session : « Allow remote automation »). Toile de 1 280 × 720, rapport de pixels 1. Les modules des trois jeux sont ceux du lot 7 (empreintes BLAKE3 au rapport ; sha256 `82669f77…`, `62043526…`, `84315a1b…`). Chaque scène de pire cas est dessinée cinq fois de suite ; une image se mesure du début du dessin (la scène mise à plat, ses textes agencés, ses quads et ses glyphes préparés) au rappel `onSubmittedWorkDone` (le travail du GPU fini, pas la présentation ; sur WebGL2, `wgpu` l'émule à son `poll`, que la page fait à chaque tâche de `MessageChannel`). La première image d'une scène est froide (ses glyphes se dessinent dans l'atlas) ; une image de démarrage, hors jeu, a compilé les programmes avant.

| Navigateur, moteur | Images de pire cas | Premières (froides) : médiane, max | Suivantes : médiane, max | Au-delà de 33 ms | Coût d'un nœud (charge) | Images perdues (trace de CDP) |
|---|---|---|---|---|---|---|
| Chrome, WebGPU | 100 | 3,73 ; 13,08 ms | 0,97 ; 6,36 ms | 0 | 1,97 µs | 1 `DroppedFrame`, 29 `FramePresented` |
| Chrome, WebGL2 forcé | 100 | 4,19 ; 15,63 ms | 1,39 ; 7,39 ms | 0 | 2,12 µs | **64** `DroppedFrame`, 24 `FramePresented` |
| Chrome, WebGPU, contrôle négatif | 100 | 3,59 ; 13,06 ms | 1,17 ; 6,16 ms | 0 | 2,02 µs | 0, 30 |
| Firefox, WebGL2 (par défaut) | 100 | 9,09 ; 22,18 ms | 6,96 ; 15,22 ms | 0 | 3,28 µs | non mesurable |
| Firefox, WebGL2 forcé | 100 | 8,84 ; 23,96 ms | 6,51 ; 16,66 ms | 0 | 3,28 µs | non mesurable |
| Firefox, contrôle négatif | 100 | 8,91 ; 21,28 ms | 6,78 ; 17,00 ms | 0 | 3,40 µs | non mesurable |

- **Aucune image de pire cas au-delà de 33 ms** de travail du GPU fini, dans aucun passage (600 images). La plus longue : 23,96 ms (Firefox, WebGL2 forcé, la première image du pire cas 5 de Sidequest, 810 nœuds) ; dans Chrome, 15,63 ms (WebGL2, la première image du pire cas 1 de Tier list, 905 nœuds, 2 520 glyphes absents).
- **Le coût d'un nœud dessiné** (question 12 de RESTE-PHASE-2) : le jeu synthétique `charge`, 1 000 rectangles et 1 000 textes courts (2 000 nœuds) contre une scène vide, médianes des quatre images chaudes de chacune : de 1,97 à 2,12 µs par nœud dans Chrome, de 3,28 à 3,40 µs dans Firefox. La scène vide coûte à elle seule 0,44 à 0,90 ms dans Chrome, 3,97 à 4,99 ms dans Firefox (l'attente du `poll` émulé de WebGL2 en fait sans doute l'essentiel : non séparé).
- **Les images perdues** : la trace de CDP de Chrome donne des `DroppedFrame` (catégorie `disabled-by-default-devtools.timeline.frame`) : 0 à 1 par passage en WebGPU, **64 en WebGL2 forcé**, pour 24 à 30 `FramePresented`. Ce que sont ces 64 images (le compositeur de Chrome sans interface, la copie de la toile WebGL2, le rythme des dessins du banc, qui ne suit pas `requestAnimationFrame`) n'est pas établi ; le lot 13 (`soiree-banc mesures`, des images au rythme de l'écran) le dira. Firefox et Safari ne donnent pas cette mesure.
- **La perte du GPU** (`device.destroy()`, puis l'appareil, les atlas et les images refaits, la dernière scène redessinée) : signalée en moins de 0,3 ms ; l'image revenue (relecture comprise) en 3,4 à 11,8 ms dans Chrome WebGPU, 6,3 à 17,5 ms en WebGL2, 7,8 à 23,2 ms dans Firefox, dont 1,1 à 16,3 ms pour l'image elle-même ; les pixels relus après la reprise sont ceux d'avant.
- Pour refaire : `cargo build -p banc --bin soiree-banc`, la construction de l'exemple `dessin-banc` (`target/travail/lot-9/final.sh`, étape `construire-banc`), puis `target/hote/lourd --calme target/debug/soiree-banc dessin <les trois modules> --navigateurs chrome,firefox,safari --fois 5 --sans-construire --sortie <fichier.json>`, et `python3 -I target/travail/lot-9/m8.py <fichier.json>` (hors dépôt). La preuve : `target/preuves/lot-9/dessin/` et `dessin.json`.
- **Ce que la mesure ne dit pas** : des images au rythme d'un écran (le banc dessine une image puis attend le GPU, une à la fois) ; le photon (OUT-88) ; Safari ; une machine de soirée autre que celle de l'hôte ; le travail long de Tier list (M8, lot 11).

Les compilations du lot (pas des mesures au calme : charge de 2,4 à 3,8) : `rendu` et `ecran` en natif depuis rien, 49 s et 1,00 Go résidents au plus ; pour `wasm32` en release, 28 s et 0,70 Go (`target/preuves/lot-9/compilation-native/`, `compilation-wasm32/`).

## Lot 10a : M12, la durée d'une reprise au pire moment

Mesuré le 2026-11-07 de 08 h 10 min 39 s à 08 h 16 min 20 s (GMT, `target/preuves/lot-10a/m12-*/code.txt`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), par `target/hote/lourd --calme`, un jeu après l'autre : Punchline (charge 0,98 au départ, 1,86 à la fin, 1,94 au plus pendant), Sidequest (1,86 ; 1,83 ; 1,99), Tier list (1,83 ; 1,98 ; 1,95) ; chaque tour sous 3 du début à la fin, aucun écarté ; 45 à 48 % de mémoire libre ; aucune compilation pendant (le paquet est construit avant, `--sans-construire`). Chrome 156.0.8078.80 sans interface, Firefox 158.0.1 sans interface ; Safari non mesuré. Les modules du lot 7 (sha256 `82669f77…`, `62043526…`, `84315a1b…`), le paquet `web/` `9b384233…` (`target/preuves/lot-10a/soiree/paquet.txt`). Des téléphones natifs jouent les coups des bots (6 chez Punchline et Sidequest, aucun chez Tier list, qui n'en a pas). La règle de validité, posée avant les tours : un tour dont la charge atteint 3 ne vaut pas mesure ; aucun ne l'a atteinte.

**Ce qui est mesuré** : le Worker est tué par le crochet de test de la page (`window.__banc.tuer()`, comme le chien de garde le tue) au pire moment, quand la partie a joué au moins 575 pas depuis le dernier point (`POINT_TOUS_LES` vaut 600) ; le Worker neuf, de sa création à la partie reprise (`performance.now()` du Worker, dont l'origine est sa création) : la colle chargée, IndexedDB lue, le module téléchargé et vérifié, réécrit, compilé, `Bonjour` et `Decrire`, les deux WebSocket ouvertes, la garde listée et lue, puis `Partie::reprendre`, qui rejoue les pas du journal depuis le dernier point en vérifiant chaque empreinte. Chaque reprise a, de plus, la même forme canonique et la même empreinte que celle que wasmtime reprend des mêmes cases (le banc le juge).

| Jeu, navigateur | Pas rejoués | IndexedDB lue | Module téléchargé | Réécrit, compilé, chargé | **Partie reprise** (trois tours) |
|---|---|---|---|---|---|
| Punchline, Chrome | 589, 590, 589 | 16 à 19 ms | 21 à 25 ms | 168 à 174 ms | **213, 234, 239 ms** |
| Punchline, Firefox | 587, 575, 578 | 36 à 40 ms | 40 à 45 ms | 194 à 201 ms | **537, 405, 657 ms** |
| Sidequest, Chrome | 590, 587, 586 | 16 à 17 ms | 22 ms | 158 à 160 ms | **241, 250, 252 ms** |
| Sidequest, Firefox | 588, 577, 576 | 37 à 38 ms | 41 ms | 180 à 182 ms | **573, 651, 568 ms** |
| Tier list, Chrome | 589, 586, 584 | 16 à 18 ms | 24 à 25 ms | 181 à 183 ms | **245, 288, 318 ms** |
| Tier list, Firefox | 587, 576, 577 | 37 ms | 43 ms | 203 à 205 ms | **601, 537, 467 ms** |

(Chaque colonne est l'heure du Worker à la fin de l'étape, depuis sa création ; la dernière est le total.)

- **Au plus 657 ms** de la création du Worker à la partie reprise, au pire moment, dans Firefox (Punchline) ; au plus 318 ms dans Chrome. La réécriture et la compilation en prennent 140 à 160 ms (le module chargé, `moteur_navigateur::charger`) ; le reste, la garde lue et les quelque 590 pas rejoués, 40 à 135 ms dans Chrome, 200 à 460 ms dans Firefox.
- **`POINT_TOUS_LES` ne change pas** (600) : une reprise au pire moment reste sous la seconde ; un point plus fréquent coûterait plus souvent ce que coûte un point dans le tour du Worker (voir `arriere-plan`, plus bas), pour gagner au plus quelques centaines de ms à une reprise.
- **Ce que la mesure ne dit pas** : Tier list sur un plateau plein d'images (l'instantané au pire, M3, 16,8 Mio dans wasmtime) : les bots jouent un plateau presque vide ; une reprise après un rechargement de l'écran (la page refaite, le GPU repris, le paquet servi) : la page n'est pas mesurée ici ; Safari ; une autre machine que celle de l'hôte.
- Pour refaire : `cargo build -p banc --bin soiree-banc`, puis `target/hote/lourd --calme target/debug/soiree-banc soiree <module.wasm> --navigateurs chrome,firefox --m12 3 [--sans-construire]`. La preuve : `target/preuves/lot-10a/m12-punchline/`, `m12-sidequest/`, `m12-tier-list/`.

**Un constat, pas une mesure de temps : les sauts de Firefox onglet caché** (`soiree-banc arriere-plan`, `target/preuves/lot-10a/arriere-plan/`, le 2026-11-07 de 07 h 57 à 08 h 10 GMT, charge 1,77 au départ, 0,98 à la fin, 3,06 au plus : pas une machine calme, et ce n'est pas une durée qu'on donne). Punchline, onglet caché 360 s : Chrome joue 21 587 pas sans aucun `Saut` ; Firefox en joue 21 598, avec **24 sauts d'un pas** (un de trois). 15 d'entre eux tombent au pas qui suit un multiple de 600 (2 401, 5 401, 6 601…) : le tour où le Worker prend un point, le chiffre et l'envoie dure assez, dans Firefox, pour que le réveil suivant ait manqué deux pas, et Punchline ne rattrape qu'un pas (`rattrapage: 1`). Le coût d'un point dans le Worker n'est pas mesuré ici (M3, lot 11) ; il touche R2 et P9 (un second Worker pour le chiffrement) et la question 4.

## Lot 10a, correction après la vérification : M12 remesuré, et les sauts d'`arriere-plan`

**M12, remesuré** sur le code corrigé (le point scellé par tranches, la minuterie de secours à 4 ms ; le Worker envoie en mode banc un rapport de plus au pire moment, `PIRE_MOMENT`, 580 pas depuis le point, où le banc le tue), le 2026-11-07, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface, Safari non mesuré, les mêmes modules (`82669f77…`, `62043526…`, `84315a1b…`), le paquet `web/` `6b6be334…` ; par `target/hote/lourd --calme`, un jeu après l'autre. La règle de validité, posée avant : un tour dont la charge atteint 3 ne vaut pas mesure. Punchline et Sidequest, mesurés d'abord de 11 h 48 à 11 h 51 GMT, ont vu la charge monter à 3,96 et 3,16 : **écartés** (`target/preuves/lot-10a-corr/m12-punchline/`, `m12-sidequest/`), et refaits de 12 h 02 à 12 h 05 (charge 1,82 au départ, 2,38 au plus ; puis 2,38 et 2,90) ; Tier list, de 11 h 51 à 11 h 52 (2,48 au départ et au plus).

| Jeu, navigateur | Pas depuis le point | **Partie reprise** (trois tours) |
|---|---|---|
| Punchline, Chrome | 580, 580, 580 | **217, 233, 228 ms** |
| Punchline, Firefox | 580, 580, 580 | **482, 450, 587 ms** |
| Sidequest, Chrome | 580, 580, 598 | **239, 258, 234 ms** |
| Sidequest, Firefox | 580, 580, 580 | **660, 485, 493 ms** |
| Tier list, Chrome | 580, 580, 599 | **247, 285, 259 ms** |
| Tier list, Firefox | 580, 580, 580 | **536, 499, 437 ms** |

- Au plus **660 ms** dans Firefox, **285 ms** dans Chrome : du même ordre qu'au lot (657 et 318 ms). `POINT_TOUS_LES` reste à 600.
- Pour refaire : `target/hote/lourd --calme target/debug/soiree-banc soiree <module.wasm> --navigateurs chrome,firefox --m12 3 --sans-construire`. Les preuves : `target/preuves/lot-10a-corr/m12-punchline-refait/`, `m12-sidequest-refait/`, `m12-tier-list/`.

**Des constats, pas des mesures de temps** (`soiree-banc arriere-plan`, Punchline, 360 s caché, trois passages par navigateur, lancé par `lourd --calme`, qui n'attend le calme qu'au départ ; les durées sont celles que le Worker rapporte en mode banc, `performance.now()`) :

- **Le tour où un point commence** (l'instantané et l'encodage) : de 10 à 18 ms dans Firefox, de 10 à 14 ms dans Chrome ; **celui qui le finit** (la dernière tranche, l'étiquette et le message `Ecrire`) : au plus 11 ms ; une tranche de 128 Kio ne fait jamais un tour de plus de 10 ms. Avant la correction, le tour qui prenait, scellait et envoyait un point durait de 13 à 33 ms dans Firefox (`target/travail/lot-10a-corr/essai1/`).
- **Les sauts qui restent** suivent un trou de plus de 30 ms entre deux pas joués, le tour lui-même court (le fil du Worker n'a pas tourné), et ils suivent la charge de la machine : charge au plus 3,25 (premier passage de la preuve, minuterie encore à 8 ms, `target/preuves/lot-10a-corr-passage1/`) : Chrome 3 réussis, Firefox 1 passage à 3 sauts ; charge au plus 3,97 (minuterie à 4 ms, `…-passage2/`) : Chrome 3 réussis, Firefox 2 réussis et 1 saut ; charge au plus 5,48 (la preuve finale, `target/preuves/lot-10a-corr/arriere-plan/`) : Chrome 2 réussis et une rafale de 7 sauts en 2,7 s (trous de 30 à 89 ms, et un tour de 23 ms), Firefox 5, 2 et 1 sauts (trous de 30 à 73 ms).
- **Firefox donne encore, de loin en loin, une image à un onglet caché** : de 0 à 3 en 360 s, `document.visibilityState` valant `hidden`.


## Lot 11 : M3, le point de reprise de Tier list au pire dans le Worker, et M8, le travail long

Mesuré le 2026-11-07, de 15 h 35 à 15 h 37 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), dans **Chrome 156.0.8078.80** (sans interface, WebGPU, puis WebGL2 forcé) et **Firefox 158.0.1** (sans interface, WebGL2) ; Safari « pas exécuté » (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché). **Sur machine calme** : `target/hote/lourd --calme`, charge 2,12 au départ, 2,32 à la fin, **2,92 au plus pendant** (sous 3 : la mesure vaut ; règle posée avant le tour). Un seul tour par navigateur (le scénario entier, 102 s) ; le module de Tier list figé (`target/modules-figes/tier-list.wasm`, sha256 `84315a1b…`, commit `9827f39`).

Pour refaire : `cargo build -p banc --bin soiree-banc`, puis `target/hote/lourd --calme target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --plein 64 [--sans-construire]`. La preuve : `target/preuves/lot-11/ecran-mesure/` (la sortie, et `ecran-mesure-travail/*/lignes.txt`, toutes les lignes que la page et le Worker ont écrites). Les durées sont celles que le Worker rapporte en mode banc (`performance.now()` du Worker ; `MesurePoint` de `salle::autorite`, la ligne `point`) et celles de la page (la ligne `demande` du rendu) ; les images perdues, celles de la trace de CDP (Chrome seulement).

**Le pire cas joué** (`soiree-banc ecran tier-list`, après les étapes ordinaires) : des images de bruit de 400 × 300 (qui ne se compressent pas), données au sélecteur par lots de 8, jusqu'à ce que le jeu en refuse un (`IMAGES_MAX_OCTETS`, 1 Mio) ou que les 64 de `--plein 64` soient données : **dans Chrome, les 64 acceptées, aucun lot refusé** (c'est `--plein 64` qui arrête l'import ; la sortie ne porte pas « le dernier lot refusé ») ; **dans Firefox, 56 acceptées, le huitième lot refusé** (son WebP du bruit est plus lourd ; les deux encodent en WebP, constaté dans les sauvegardes). Recompté dans les sauvegardes captées (correction du lot 11, `target/preuves/lot-11/ecran-mesure-travail/profils/captes/`) : le plateau plein de Chrome porte 939 282 octets d'images (69 images, 0,896 Mio), celui de Firefox 919 586 (61 images, 0,877 Mio) ; Chrome est donc à moins d'un lot de la borne (un lot de plus, environ 117 Ko, la passerait), et son plateau est plus lourd que celui de Firefox : le pire cas est atteint en pratique, sans que Chrome ait refusé un lot ; la sauvegarde du plateau plein écrite (1 256 424 et 1 229 728 octets) puis relue ; ses tuiles rangées dans les rangs (69 et 61) ; son export (un PNG de 2 400 × 1 678 et 2 400 × 1 294, 6,7 et 6,1 Mo, que le jeu garde comme image reçue). Les points mesurés sont les deux pris ensuite (aux pas 1 200 et 1 800).

### M3 : le point de reprise

| Navigateur | **Case scellée** | Instantané (tour du point) | **Encodage** (même tour) | Scellement : tranches de 128 Kio, en tout (la plus longue) | Mise en file de l'envoi |
|---|---|---|---|---|---|
| Chrome, WebGPU | **20 177 488 octets** (19,2 Mio) | 3,35 ; 1,48 ms | **59,6 ; 41,0 ms** | 154 tranches : 116 ; 193 ms (2,08 ; 2,09 ms) | 1,70 ; 2,84 ms |
| Chrome, WebGL2 forcé | **20 177 488 octets** | 3,38 ; 2,69 ms | **63,4 ; 47,8 ms** | 154 : 225 ; 233 ms (2,17 ; 2,11 ms) | 2,65 ; 2,43 ms |
| Firefox | **18 866 772 octets** (18,0 Mio) | 3,74 ; 2,20 ms | **112,4 ; 82,2 ms** | 144 : 146 ; 138 ms (2,34 ; 2,32 ms) | 1,22 ; 1,12 ms |

- **La taille** : 18 à 19,2 Mio, plus que les 16,8 Mio de wasmtime au lot 2 (qui ne gardait pas d'image exportée de 6 Mo) ; sous `relais::coeur::CASE_MAX` (64 Mio) et `GARDE_MAX` (256 Mio). Un plateau presque vide (six images rayées, au début du scénario) donne déjà un point de 2,2 Mio (constaté dans les essais du lot, Chrome).
- **Le chiffrement** se fait déjà hors du tour qui prend le point, une tranche par réveil (lot 10a, correction) : **au plus 2,34 ms par tranche**, autour de la frontière des 2 ms de R2.
- **Ce qui reste dans un seul tour, et dépasse de loin les 2 ms de R2** : l'encodage de l'instantané (`jeu_abi::encoder`, octet par octet : 41 à 63 ms dans Chrome, 82 à 112 ms dans Firefox) ; **le tour qui finit le point** (`finir_point` et le message `Ecrire` de 19 Mio : 38 à 86 ms ; lignes `tour` aux pas 1 271, 1 337 et 1 880, `sceller` sans `point`) ; et **la case de journal qui porte une grande entrée** (la réponse `Lire` de la sauvegarde relue, 1,2 Mo, scellée d'un coup : 73 à 134 ms, pas 844 à 852). Pendant ces tours, la partie prend du retard (deux à huit pas) : Tier list n'en souffre pas (rien n'y dépend du temps hors du glisser), mais le contrôle de `arriere-plan` (aucun saut) échouerait sur ce plateau. La décision revient au mainteneur (PHASE-3, question 31) ; rien n'est changé ici.
- **L'envoi** (la mise en file des trames sur la WebSocket de la garde, `WebSocket.send`) : 1,1 à 2,8 ms ; le navigateur envoie ensuite sans le fil du Worker.

### M8 : le travail long de Tier list

| Navigateur | Export ordinaire (2 400 × 1 294, 5 tuiles) | **Export du plateau plein** | Images de la page pendant l'export plein : la plus longue, au-delà de 33 ms | Images perdues (trace de CDP) | Tours du Worker de plus de 33 ms |
|---|---|---|---|---|---|
| Chrome, WebGPU | 71,8 ms, 4 images | **115,1 ms**, 6 images, 69 tuiles | 18,4 ms, aucune | **0** (210 `PipelineReporter`) | 6 (38,8 à 73,0 ms) |
| Chrome, WebGL2 forcé | 64,7 ms, 4 images | **110,9 ms**, 6 images | 18,4 ms, aucune | **1** (216) | 5 (39,4 à 77,1 ms) |
| Firefox | 55,8 ms, 6 images | **122,7 ms**, 14 images, 61 tuiles | 10,1 ms, aucune | non mesurable (Firefox ne donne pas la trace) | 7 (33,8 à 134,2 ms) |

- **Le rendu hors écran ne fait pas sauter d'image à la page** (TL-57) : par bandes de 1 024 pixels, une image présentée entre deux, aucune image de la page au-delà de 33 ms pendant l'export, dans les trois passages ; Chrome WebGL2 a perdu une image (`DroppedFrame`) pendant cet export, Chrome WebGPU aucune. Non jugé par le banc (la ligne du lot demande la mesure) ; le nombre d'images perdues varie d'un tour à l'autre (de 1 à 12 dans les essais du lot, sous une charge de 3 à 5 : pas des mesures).
- **Le travail long est dans le Worker**, comme le veut le § 2.11 : la restauration de la sauvegarde pleine (`avancer` 21 à 33 ms, et la vue du plateau plein qui suit, `vues` 17 à 56 ms, au même tour), la case de journal qui la porte (73 à 134 ms), et les tours des points (M3). Le garde-fou de 2 s (`GARDE_FOU_PARTIE_MS`) en est loin : le plus long tour mesuré fait 134 ms.
- **Ce que la mesure ne dit pas** : Safari ; un plateau de 300 tuiles de texte (la borne `TUILES_MAX`, que les images bornent avant elle) ; le coût d'un nœud dessiné, mesuré au lot 9.

## Lot 12 : M4 (le module sur la page du téléphone) et M7 (la redite du tenu par la vraie page)

Mesuré le 2026-11-07 de 17 h 24 min 49 s à 17 h 28 min 01 s (GMT, `target/preuves/lot-12/soiree-sidequest/code.txt`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), par `target/hote/lourd --calme` : charge 2,63 au départ, 1,87 à la fin, **2,84 au plus pendant** (le portillon ne dit aucun dépassement de 3 : le tour vaut mesure sur machine calme). Chrome 156.0.8078.80 et Firefox 158.0.1, sans interface ; Safari « pas exécuté » (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché). La commande : `target/hote/lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <sidequest_module.wasm> --navigateurs chrome,firefox --pages chrome,firefox,safari --sans-construire`, le paquet construit avant par `soiree-relais preparer`. Le scénario fait tourner en même temps l'écran (un navigateur), deux navigateurs de pages et six téléphones natifs : la charge qu'il relève est aussi la sienne.

**M4, sur la machine de l'hôte** (pas un téléphone) : la page dit, du module reçu entier par la connexion chiffrée à l'instance prête (`moteur_navigateur::charger` sous `PHASE_3` : réécrit, compilé par `new WebAssembly.Module` sur le fil principal, instancié, `Bonjour` et `Decrire`), mesuré par `performance.now()` de la page.

| Module (octets livrés) | Chrome 156, page en émulation tactile | Firefox 158 |
|---|---|---|
| Sidequest (738 744) | 127,6 ms et 129,2 ms (un par passage : l’écran dans Chrome, puis dans Firefox) | 172,0 ms et 185,0 ms |

- La taille téléchargée est celle du module livré (738 744 octets pour Sidequest), en douze morceaux de 64 Kio par la connexion chiffrée ; le temps du téléchargement lui-même n'est pas mesuré ici (la boucle locale).
- **Ce que la mesure ne dit pas** : un vrai téléphone (la fiche humaine, § 8.5 : un processeur plus lent qu'un M5, de combien on ne le sait pas) ; Safari. Les relevés de Punchline (941 989 octets : 142,5 à 206,0 ms) ne valent pas mesure : la charge a atteint 5,57 pendant ce scénario (`target/preuves/lot-12/soiree-punchline/`), et le portillon le dit.
- `TAILLE_MODULE_MAX` (4 Mio) ne change pas : aucune mesure ne le met en cause.

**M7** : les écarts entre deux trames de tenu (`Classe::Remplacable`) qu'un lien de la page a envoyées, à l'horloge du relais (son enregistrement, `--enregistrer`), pendant que le pilote du banc tenait la croix de la manette d'un doigt posé (`Input.dispatchTouchEvent` dans Chrome, un pointeur `touch` de BiDi dans Firefox), 8 s par tenue, passé ses 300 premières ms. Sidequest déclare une fraîcheur de 450 ms : P27 donne une redite de 150 ms.

| Page | Écarts | Minimum | Médiane | 99ᵉ centile | Maximum |
|---|---|---|---|---|---|
| Chrome 156 (premier passage, 4 croix tenues) | 63 | 47,9 ms | 153,5 ms | 1 248,5 ms | 3 503,1 ms |
| Firefox 158 (premier passage, 4 croix tenues) | 116 | 37,6 ms | 167,2 ms | 199,5 ms | 3 503,2 ms |

- **La médiane retrouve la période de P27** (150 ms, à 3,5 ms près dans Chrome et 17 ms dans Firefox, la granularité du réveil de la page étant de 50 ms). Les écarts plus courts sont des changements du tenu (une direction neuve part aussitôt, au débit du jeu) ; les plus longs, des moments où la manette ne déclare plus que l'arrêt (commandes désactivées entre deux phases : la valeur relâchée, qui ne se redit pas, § 2.10), et non des redites manquées : la mesure ne sait pas les distinguer, chiffrée qu'elle est. Au second passage, aucune tenue n'a été notée (le doigt est resté posé jusqu'à la fin) : rien n'en est lu.
- `RAPPEL_TENU` (250 ms) du monde simulé de l'épreuve ne change pas : le passer à `liaison::periode_de_redite` change ce que l'épreuve joue (lot 4a).
- Pour refaire : la commande ci-dessus ; la preuve : `target/preuves/lot-12/soiree-sidequest/`, et l'enregistrement du relais dans `target/travail/lot-12/final-sidequest/chrome/enregistrement.bin`.

## Lot 12, correction après la vérification : M13 relevée, et rien de mesuré au calme

Relevé le 2026-11-07 de 20 h 06 à 20 h 43 (GMT, `target/preuves/lot-12-corr/*/code.txt`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface, Safari « pas exécuté ». Chaque scénario est passé par `target/hote/lourd --calme` (charge sous 3 au départ : de 1,38 à 2,86) et **chacun a dépassé 3 pendant** : Sidequest cinq minutes 3,02, Punchline 3,13 et 3,73, Punchline sans pages 3,24, les 71 appareils 4,47 ; le portillon l'a dit chaque fois. **Rien ici ne vaut mesure sur machine calme** : ce sont des relevés, et M4 et M7 du lot 12 (charge 2,84) restent les mesures. La charge vient pour partie du scénario lui-même (l'écran, deux navigateurs de pages, les natifs ; 69 natifs dans le dernier).

**M13**, la part du commun dans le Worker à chaque tour qui joue un pas (les vues de l'écran et des téléphones et leur chiffrement, la garde ; `DureesDuTour::vues + garde`, répartie par cases de 0,1 ms, `RepartitionDuTour`, rapportée dans les lignes `etat`), relevée sur le Worker qui a joué le plus de tours de chaque passage. Le seuil : la moitié de la part d'image que E19 laisse au commun, 1/60 s × ½ × ½ = 4,17 ms, au 99ᵉ centile.

| Scénario (appareils) | Navigateur de l'écran | Tours | Médiane | 99ᵉ centile | Maximum | Le tour entier (médiane, 99ᵉ, max) |
|---|---|---|---|---|---|---|
| `soiree-71` (69 natifs et 2 pages, 71 téléphones ; voir la réserve) | Chrome 156 | 6 121 | 0,4 ms | 1,7 ms | 4,19 ms | 0,8 ; 5,2 ; 30,6 ms |
| `soiree-71` | Firefox 158 | 6 180 | 0,5 ms | 2,2 ms | 6,94 ms | 1,0 ; 4,6 ; 7,2 ms |
| `soiree-sidequest` (6 natifs, 2 pages) | Chrome 156 | 15 502 | 0,4 ms | 0,7 ms | 5,47 ms | 0,7 ; 1,8 ; 6,8 ms |
| `soiree-sidequest` | Firefox 158 | 15 506 | 0,5 ms | 1,0 ms | 4,36 ms | 0,9 ; 2,4 ; 6,6 ms |

- **La réserve** : dans `soiree-71` (Chrome), le Worker compte 69 à 71 appareils accueillis jusqu'au pas 5 400 environ, puis 2 jusqu'à la fin (12 % de ses tours), alors que l'écran dit 71 téléphones reliés tout du long : la répartition ne porte qu'à 88 % sur des tours à 69-71 appareils, et la chute n'est pas expliquée.
- Les centiles sont au haut de leur case de 0,1 ms (des bornes par excès), le maximum exact.
- **Le point écrit d'un bloc** : aucun tour de point au-delà de 10 ms (le seuil d'une ligne `tour`) dans aucun des passages (au lot 12 : 10 à 22 ms à sceller le point de Punchline).
- **M7, relevé** sur Sidequest cinq minutes (23 croix tenues par passage) : médiane 197 à 198 ms dans Chrome, 168 à 169 ms dans Firefox, pour 150 ms de P27 ; les écarts de 1,75 et 3,5 s sont des moments où la page disait un tenu relâché (`tenu_relache`), qui ne se redit pas.
- **M4, relevé** : Sidequest, 126 à 130 ms dans Chrome, 144 à 200 ms dans Firefox.
- Pour refaire : `target/travail/lot-12-corr/final.sh` (les commandes, une par preuve) ; les enregistrements du relais dans `target/travail/lot-12-corr/final-*/<navigateur>/enregistrement.bin`.

## Fusion du lot 11 dans refonte : M3 et M8 refaits sur le code fusionné

Mesuré le 2026-11-07, de 21 h 47 min 18 s à 21 h 49 min 33 s (UTC), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), dans **Chrome 156.0.8078.80** (sans interface, WebGPU, puis WebGL2 forcé) et **Firefox 158.0.1** (sans interface, WebGL2) ; Safari « pas exécuté » (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché). **Sur machine calme** : `target/hote/lourd --calme`, charge 1,58 au départ, 2,04 à la fin, **2,26 au plus pendant** (sous 3 : la mesure vaut ; règle posée avant le tour). Un seul tour par navigateur (le scénario entier, 135 s de bout en bout) ; le code fusionné (le lot 11 et le lot 12, `LOTS-PHASE-3.md`, « Fusion du lot 11 ») ; le module de Tier list construit depuis ses sources (`9827f39`, sha256 `84315a1b…`, le même qu'aux lots 7 à 12) ; le paquet `web/` `bd7e6599…`.

Pour refaire : `target/travail/fusion-11/final.sh` (la ligne `ecran`), soit `target/hote/lourd --calme target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --sans-construire`. La preuve : `target/preuves/fusion-11/ecran/` ; les lignes de la page et du Worker dans `target/travail/fusion-11/ecran/*/lignes.txt`. Mêmes définitions qu'au lot 11.

Le pire cas joué est le même : Chrome accepte les 64 images (aucun lot refusé), Firefox 56 (le huitième lot refusé).

### M3 : le point de reprise

| Navigateur | **Case scellée** | Instantané (tour du point) | **Encodage** (même tour) | Scellement : tranches, en tout (la plus longue) | Mise en file de l'envoi |
|---|---|---|---|---|---|
| Chrome, WebGPU | **20 177 488 octets** (19,2 Mio) | 5,42 ; 3,02 ms | **27,8 ; 19,4 ms** | 154 tranches : 243 ; 243 ms (2,18 ; 2,10 ms) | 2,13 ; 2,68 ms |
| Chrome, WebGL2 forcé | **20 177 488 octets** | 3,63 ; 2,75 ms | **29,8 ; 22,0 ms** | 154 : 247 ; 249 ms (2,19 ; 2,12 ms) | 3,24 ; 2,67 ms |
| Firefox | **18 866 772 octets** (18,0 Mio) | 3,82 ; 2,46 ms | **30,6 ; 24,8 ms** | 144 : 161 ; 154 ms (2,36 ; 2,76 ms) | 1,24 ; 1,32 ms |

- **L'encodage dans le tour du point a fondu** : 19 à 31 ms, contre 41 à 112 ms au lot 11. C'est le point écrit d'un bloc (`partie::persistance::octets`, correction du lot 12), entré avec la fusion : les mêmes octets (la case a la même taille au même pas), sans un appel de sérialiseur par octet. Il reste au-delà des 2 ms de R2 (question 31).
- **Les tranches** : au plus 2,19 ms dans Chrome, **2,76 ms dans Firefox** (2,34 au lot 11) : un seul tour, la différence n'est pas expliquée ; toujours autour de la frontière des 2 ms de R2.
- **Ce qui reste dans un seul tour** : le tour qui finit le point (`sceller` sans `point`) : 37,7 à 39,8 ms dans Chrome, 80,8 et 84,2 ms dans Firefox ; et la case de journal qui porte la sauvegarde relue (`garde` seule, `via: minuterie`) : 82,1 et 82,2 ms dans Chrome, 127,2 ms dans Firefox. Mêmes ordres qu'au lot 11 (38 à 86 ms, 73 à 134 ms) : la fusion n'y change rien.

### M8 : le travail long de Tier list

| Navigateur | Export ordinaire (2 400 × 1 294) | **Export du plateau plein** | Images de la page pendant l'export plein : la plus longue, au-delà de 33 ms | Images perdues (trace de CDP) | Tours du Worker de plus de 33 ms |
|---|---|---|---|---|---|
| Chrome, WebGPU | 65,6 ms, 4 images | **113,0 ms**, 6 images (2 400 × 1 678) | 16,7 ms, aucune | **1** (222 `PipelineReporter`) | 5 (39,9 à 82,4 ms) |
| Chrome, WebGL2 forcé | 74,0 ms, 4 images | **111,1 ms**, 6 images | 16,7 ms, aucune | **1** (208) | 5 (39,1 à 82,6 ms) |
| Firefox | 64,4 ms, 6 images | **132,2 ms**, 16 images (2 400 × 1 294) | 9,8 ms, aucune | non mesurable | 6 (39,8 à 127,8 ms) |

- Le rendu hors écran ne fait toujours pas sauter d'image à la page (TL-57) ; une image perdue dans chaque passage de Chrome pendant l'export plein (au lot 11 : 0 et 1).
- Le plus long tour mesuré, 127,8 ms (la case de journal de la restauration, Firefox), reste loin du garde-fou de 2 s.
- **Ce que la mesure ne dit pas** : Safari ; un second tour (une seule mesure par navigateur, comme au lot 11).

## Fusion du lot 11, correction : M12 de Tier list sur le code fusionné

Mesuré le 2026-11-07, de 23 h 11 min 30 s à 23 h 12 min 27 s (UTC), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), dans **Chrome 156.0.8078.80** (sans interface, WebGPU) et **Firefox 158.0.1** (sans interface, WebGL2) ; Safari non mesuré. Le code fusionné (`HEAD` `e719045`, `MERGE_HEAD` `2d95152`, `git diff HEAD` hors `.md` `50fae729fc909c24`), le module de Tier list `84315a1b…` (`9827f39`, dépôt propre), le paquet `web/` `bd7e6599…`, `soiree-banc` `441b64b0…`. **La règle de validité, posée avant les tours** : un passage dont la charge atteint 3 ne vaut pas mesure, il est écarté et refait. Deux passages l'ont atteinte et sont **écartés** : à 23 h 04 (2,93 au départ, **3,75** au plus) et à 23 h 08 (2,25 au départ, **3,77** au plus) (`target/preuves/fusion-11-corr/m12-tier-list-ecarte-1/`, `-2/`) ; le troisième, lancé quand la charge est passée sous 1,6 (`target/travail/fusion-11-corr/m12.sh`), vaut : **charge 1,52 au départ, 2,13 à la fin, 2,07 au plus pendant** (relevés du portillon), 51 à 54 % de mémoire libre, rien d'autre des nôtres pendant.

Pour refaire : `target/hote/lourd --calme target/debug/soiree-banc soiree <tier_list_module.wasm> --navigateurs chrome,firefox --m12 3 --sans-construire`. La preuve : `target/preuves/fusion-11-corr/m12-tier-list/`. Mêmes définitions qu'au lot 10a (du Worker neuf à la partie reprise, au pire moment).

| Navigateur | Pas depuis le point | IndexedDB lue | Module téléchargé | Réécrit, compilé, chargé | **Partie reprise** (trois tours) |
|---|---|---|---|---|---|
| Chrome | 580, 598, 580 | 7 à 19 ms | 11 à 26 ms | 156 à 184 ms | **254, 226, 289 ms** |
| Firefox | 580, 578, 580 | 29 à 41 ms | 34 à 47 ms | 202 à 212 ms | **397, 555, 701 ms** |

- Chaque reprise a la même forme que celle de wasmtime (six sur six, le banc le juge).
- **Au plus 701 ms** dans Firefox (au lot 10a, corrigé : 536 ; avant : 601), **289 ms** dans Chrome (285 et 318) : toujours sous la seconde ; le troisième tour de Firefox est le plus long relevé pour Tier list, sur un seul tour au calme, et la différence n'est pas expliquée (le module se charge en autant de temps ; c'est la reprise du journal qui s'allonge).
- **Ce que la mesure ne dit pas** : Tier list sur un plateau plein (les bots jouent un plateau presque vide, comme au lot 10a) ; Safari ; une autre machine.

## Lot 12, fin : M4 de Punchline sur la page du téléphone, au calme

Mesuré le 2026-11-08, de 01 h 10 min 06 s à 01 h 15 min 05 s (UTC), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), pages du téléphone dans **Chrome 156.0.8078.80** (sans interface, émulation tactile 390 × 844) et **Firefox 158.0.1** (sans interface, 360 × 640) ; Safari « pas exécuté ». Le code de la fin du lot 12 (`HEAD` `cda28f3`, la preuve `target/preuves/lot-12-fin/soiree-punchline-2/`, son empreinte dans `empreinte.txt`), le module de Punchline `82669f77…` (941 989 octets). **Charge** (le portillon, `--calme`) : 1,39 au départ, 1,91 à la fin, **2,77 au plus** : une mesure sur machine calme (la règle : un tour qui atteint 3 est écarté ; la preuve `soiree-punchline`, 3,06 au plus, l'est).

Pour refaire : `target/hote/lourd --calme target/debug/soiree-banc soiree <punchline_module.wasm> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 5 --sans-construire`. Même définition qu'au lot 12 : du module reçu entier à l'instance de la page prête (réécriture, compilation, `Bonjour`, `Decrire`), dit par la page.

| Page | Passage (l'écran dans Chrome) | Passage (l'écran dans Firefox) |
|---|---|---|
| Chrome | 141,4 ms | 144,0 ms |
| Firefox | 210,0 ms | 199,0 ms |

- Pour Sidequest (738 744 octets), le lot 12 relevait 127,6 et 129,2 ms (Chrome), 172,0 et 185,0 ms (Firefox) : Punchline, 27 % plus gros, prend 11 % (Chrome) à 15 % (Firefox) de plus.
- M7 n'est pas relevée : Punchline n'a pas de croix.
- **Ce que la mesure ne dit pas** : un vrai téléphone (la page tourne sur l'ordinateur de l'hôte, émulée) ; Safari.

Ce que les autres preuves de la fin du lot rapportent (M4 de Sidequest, M13 à 71 appareils, les allers-retours d'écho des natifs) a dépassé une charge de 3 : relevé dans `LOTS-PHASE-3.md`, « Lot 12, fin », pas revendiqué comme mesure.

## Lot 12, fin, correction : M13 à 71 appareils, au calme

Mesuré le 2026-11-08, de 03 h 55 min 22 s à 03 h 57 min 02 s (`m13-1`) puis de 03 h 57 min 19 s à 04 h 03 min 58 s (`m13-long`) (UTC), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), l'écran dans **Chrome 156.0.8078.80** puis **Firefox 158.0.1** (sans interface), sur l'arbre de la correction de la fin du lot 12 (`HEAD` `cda28f3`, `git diff HEAD` hors `.md` `54fc4b33…`, modules des jeux inchangés : Sidequest `62043526…`). **71 téléphones natifs** par la page, **aucune page dans un navigateur** : c'est ce qui a rendu la mesure possible au calme, les scénarios des 71 appareils avec trois navigateurs (`soiree-71`, sa reprise à cette correction comprise : 3,18 au plus) dépassant toujours une charge de 3. **Règle de validité posée avant les tours** (`target/travail/lot-12-fin-corr/final-3.sh`) : un tour dont le portillon dit la charge la plus haute pendant à 3 ou plus est écarté et refait, au plus trois. Aucun n'a été écarté : `m13-1` charge 2,05 au départ, 1,96 à la fin, **2,64 au plus** ; `m13-long` (`--minutes 3`) 1,90, 1,95, **2,61 au plus**.

Pour refaire : `target/hote/lourd --calme target/debug/soiree-banc soiree <sidequest_module.wasm> --navigateurs chrome,firefox --telephones 71 [--minutes 3] --sans-construire`. Même définition qu'à la correction du lot 12 : `DureesDuTour::vues + garde` par tour qui joue un pas, par cases de 0,1 ms (bornes par excès), sur le Worker qui a joué le plus de tours du passage ; 71 appareils accueillis au dernier rapport dans chaque passage.

| Preuve | Écran | Tours | Part du commun : médiane | 99ᵉ centile | maximum | Le tour entier : médiane | 99ᵉ centile | maximum |
|---|---|---|---|---|---|---|---|---|
| `m13-1` | Chrome | 1 252 | 0,5 ms | 2,0 ms | 2,74 ms | 1,1 ms | 4,1 ms | 7,35 ms |
| `m13-1` | Firefox | 1 247 | 0,7 ms | 2,9 ms | 3,70 ms | 1,5 ms | 5,9 ms | 24,40 ms |
| `m13-long` | Chrome | 8 944 | 0,5 ms | 2,0 ms | 3,97 ms | 1,0 ms | 5,8 ms | 9,50 ms |
| `m13-long` | Firefox | 9 008 | 0,8 ms | 2,4 ms | 6,08 ms | 1,2 ms | 6,6 ms | 10,92 ms |

- **M13 tient** : au 99ᵉ centile, de 2,0 à 2,9 ms, sous les 4,17 ms (la moitié de la part d'image que E19 laisse au commun). Le maximum (6,08 ms dans Firefox) dépasse le seuil, que seul le 99ᵉ centile juge.
- `m13-1` est court (la partie de Sidequest jusqu'à sa première phase terminale, environ 20 s de tours par passage) ; `m13-long` prolonge la partie de 180 s : c'est lui qui fait foi. Les deux disent la même chose.
- Relevés à côté, pas mesurés au calme : `soiree-71` (trois navigateurs, 3,18 au plus) donne 1,9 et 2,3 ms au 99ᵉ centile, du même ordre.
- **Ce que la mesure ne dit pas** : un vrai réseau Wi-Fi (le relais est sur la boucle locale) ; une machine de soirée moins rapide que le M5 ; Safari.

## Lot 13 : M2 (de l'entrée au pixel), M14 (l'écart des horloges), M6 (l'arrière-plan avec interface), et M1 remesurée

Le 2026-11-08, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2), Chrome 156.0.8078.80 et Firefox 158.0.1. **Chaque mesure par `target/hote/lourd --calme`**, une à la fois ; **la règle de validité, posée avant les tours** : un tour ne vaut mesure que si la charge la plus haute pendant, que le portillon écrit (la moyenne d'une minute, relevée toutes les 10 s, le navigateur et le banc compris), reste sous 3 ; sinon il est écarté, gardé sous `<nom>-ecarte-<k>`, et refait (quatre essais au plus ; `target/travail/lot-13/campagne.sh`, son journal `campagne-journal.txt`). Les preuves sont dans `target/preuves/lot-13/<nom>/` (commande, sortie complète, code, durée, empreinte des dépôts) ; la commande qui refait chaque mesure est dans son `commande.txt`. Safari : pas exécuté (le banc n'a pas `safaridriver` permis, question 8). Le lot a été fait par deux agents : le premier (de 05 h 50 à 07 h 56, UTC) a été arrêté par une limite d'usage, le second a repris à 18 h 54 ; les modules sont les mêmes (Punchline `4c3ebf10…`, `ardoise` `400cc5ae…`, en sha256), le paquet de la page aussi (`target/soiree-web/755ccba5…`, construit à 06 h 49 après la dernière modification de `web/`), et `soiree-banc` (`target/release`, 06 h 31).

**L'écran de la machine compte** (constaté à la reprise) : d'après `pmset -g log`, il s'est éteint à 02 h 33, rallumé de 06 h 54 min 33 s à 07 h 01 min 51 s, puis rallumé à 18 h 52. **Chrome sans interface cale ses images sur l'écran de la machine** : écran éteint, la page de la télé n'y reçoit que 30 images par seconde ; écran allumé, 60. Les mesures de Chrome du premier agent ont été faites écran éteint ; celles de Firefox (120 images par seconde, son affichage virtuel, quel que soit l'écran) écran allumé. À la reprise, chaque mesure qui dépend de l'écran passe sous `caffeinate -d` (une assertion qui garde l'écran allumé le temps de la commande ; aucun réglage de la machine n'est changé), et l'empreinte de chaque preuve dit le dernier changement de l'écran.

### M2 : de l'entrée au pixel

> **Corrigé après la vérification du lot 13 : les lignes « avec glisser » des tableaux qui suivent ne mesurent pas un vrai glisser.** `ardoise` et Punchline n'ont aucun nœud `glisse`, et la page d'alors glissait après chaque scène neuve même sans rien à déplacer, en retardant le pixel de la trace jusqu'à la fin du chemin (16,7 ms) : ces lignes mesurent ce retard de la définition de la trace, pas un coût visible (l'image affichée était la même). La page ne glisse plus que si un nœud se déplace ; glisser est mesuré sur Sidequest, le seul module qui glisse, plus bas (« M2 corrigée : glisser »). Les lignes « sans glisser » ne changent pas.

`soiree-banc mesures <module> --navigateurs <n> --entrees 150 --duree 20 --echos 200 --sans-construire` (le clavier et la souris, sur `ardoise` : des chiffres tapés par la chaîne d'entrées du navigateur, un clic sur « Effacer » une fois sur huit, à des intervalles tirés de 40 à 140 ms), et `--entrees 0 --duree 30` sur `punchline` (les gestes de six téléphones natifs, par leur page, à travers le vrai relais et le vrai chiffrement). Navigateurs sans interface. Quatre conditions dans le même passage, puis le contrôle négatif. En ms, « médiane ; 99ᵉ centile ; maximum », sur les traces qui ont leur pixel (le « pixel » est la fin du travail du GPU de la première image qui dessine la scène en entier : PHASE-3, § 8.4). Charge au départ, à la fin, la plus haute pendant : `m2-ardoise-chrome-ecran-allume` 2,53, 2,32, 2,67 (trois passages écartés avant lui : 3,27, 4,16 et 3,99 pendant) ; `m2-punchline-chrome-ecran-allume` 2,32, 1,83, 2,56 ; `m2-ardoise-chrome` (écran éteint) 2,13, 1,71, 2,34 ; `m2-punchline-chrome` (écran éteint les 110 premières secondes, allumé les 35 dernières) 1,71, 2,16, 2,60 ; `m2-ardoise-firefox` 2,16, 1,66, 2,50 ; `m2-punchline-firefox` 2,14, 1,19, 2,14 (un premier passage écarté : 3,30 pendant). Les tableaux entiers, segment par segment : `target/travail/lot-13/tableau-m2-*.md` (`tableau.py` sur chaque `sortie.txt`).

**Chrome sans interface, écran de la machine allumé** (refait à la reprise, `m2-*-chrome-ecran-allume`, le 2026-11-08 de 20 h 23 à 20 h 28 UTC ; la page de la télé reçoit **60 images par seconde**, sous l'écran de démarrage comme en jeu, panneau ouvert ou fermé) :

| Condition | clavier, total | clavier : attente du pas ; vers la page ; attente de l'image ; GPU | souris, total | téléphone, total | téléphone : transport ; attente ; attente de l'image |
|---|---|---|---|---|---|
| avec calage, sans glisser | 25,8 ; 34,5 ; 35,3 (132) | 8,8 ; 17,8 — 1,6 ; 4,8 — 16,5 ; 18,5 — 0,6 ; 0,8 | 33,8 ; 34,8 (18) | 19,9 ; 37,8 ; 38,6 (121) | 1,3 ; 4,2 — 11,5 ; 17,9 — 2,4 ; 18,3 |
| sans calage, sans glisser | 25,3 ; 35,5 ; 35,7 (131) | 9,9 ; 19,6 — 0,02 ; 12,2 — 10,5 ; 18,3 — 0,6 ; 0,8 | 33,3 ; 34,9 | 19,1 ; 30,5 ; 30,5 (98) | 1,4 ; 3,4 — 8,7 ; 19,2 — 7,9 ; 10,3 |
| avec calage, avec glisser | 40,6 ; 51,1 ; 53,2 (130) | 8,3 ; 16,7 — 0,5 ; 3,3 — 32,4 ; 33,8 — 0,5 ; 0,9 | 49,4 ; 51,6 | 49,6 ; 68,6 ; 68,7 (112) | 1,3 ; 4,4 — 10,1 ; 16,5 — 33,1 ; 50,1 |
| sans calage, avec glisser | 41,1 ; 51,0 ; 51,4 (132) | 8,1 ; 18,8 — 5,2 ; 9,3 — 25,7 ; 33,8 — 0,5 ; 0,8 | 50,2 ; 50,9 | 43,7 ; 68,0 ; 68,0 (93) | 1,2 ; 4,1 — 8,7 ; 20,5 — 24,1 ; 55,9 |

Glisser est ici **forcé à 60 Hz** (`window.__banc.glisser("oui")`), ce que la télé ne fait jamais d'elle-même (elle ne glisse qu'à 100 Hz et plus, § 2.9) : ces deux lignes mesurent le coût d'un pas de retard, pas une configuration de soirée.

**Chrome sans interface, écran de la machine éteint** (le premier agent ; la page de la télé reçoit **30 images par seconde**, sous l'écran de démarrage comme en jeu, panneau ouvert ou fermé ; la même chose avec le paquet de la page d'avant le lot, `target/travail/lot-13/essai4-ancien-paquet/`, écran éteint lui aussi) :

| Condition | clavier, total | clavier : attente du pas ; vers la page ; attente de l'image ; GPU | souris, total | téléphone, total | téléphone : transport ; attente ; attente de l'image |
|---|---|---|---|---|---|
| avec calage, sans glisser | 52,6 ; 69,0 ; 71,9 (132) | 8,9 ; 27,7 — 12,4 ; 17,7 — 32,6 ; 34,8 — 1,5 ; 2,0 | 67,4 ; 69,3 (18) | 32,1 ; 48,2 ; 48,2 (99) | 1,4 ; 5,8 — 10,6 ; 22,3 — 11,0 ; 33,9 |
| sans calage, sans glisser | 54,6 ; 71,0 ; 71,1 | 8,3 ; 18,6 — 7,7 ; 26,2 — 32,7 ; 34,9 — 1,5 ; 2,3 | 67,5 ; 68,7 | 36,1 ; 50,3 ; 51,3 (102) | 1,2 ; 5,5 — 10,0 ; 16,8 — 16,3 ; 31,6 |
| avec calage, avec glisser | 59,0 ; 73,3 ; 74,4 | 10,6 ; 24,5 — 11,9 ; 16,0 — 33,2 ; 47,6 — 1,4 ; 3,4 | 67,0 ; 69,6 | 43,3 ; 56,6 ; 56,6 (95) | 1,4 ; 4,4 — 16,3 ; 25,0 — 20,5 ; 46,3 |
| sans calage, avec glisser | 52,4 ; 71,3 ; 71,9 | 8,3 ; 20,4 — 5,7 ; 23,3 — 32,7 ; 35,1 — 1,5 ; 2,1 | 67,4 ; 70,0 | 48,4 ; 70,1 ; 70,2 (123) | 1,3 ; 4,6 — 9,1 ; 18,2 — 28,8 ; 61,5 |

**Firefox sans interface** (écran de la machine allumé ; le repli WebGL2 ; **120 images par seconde**, l'affichage virtuel de Firefox, comme la mesure 6 ; le travail du GPU fini n'y est connu qu'au `poll` de l'image suivante, d'où un segment GPU majoré d'une image, 8,3 ms) :

| Condition | clavier, total | clavier : attente du pas ; vers la page ; attente de l'image ; GPU | souris, total | téléphone, total | téléphone : transport ; attente ; attente de l'image |
|---|---|---|---|---|---|
| avec calage, sans glisser | 25,5 ; 32,9 ; 33,1 (132) | 8,2 ; 16,3 — 0,06 ; 1,0 — 7,6 ; 8,7 — 7,0 ; 14,9 | 22,2 ; 34,1 (18) | 28,1 ; 42,0 ; 42,0 (92) | 1,3 ; 6,9 — 7,3 ; 16,4 — 7,0 ; 8,4 |
| sans calage, sans glisser | 21,3 ; 35,3 ; 35,3 | 7,3 ; 18,6 — 0,06 ; 0,7 — 6,1 ; 8,3 — 7,0 ; 15,6 | 22,6 ; 38,7 | 30,9 ; 40,6 ; 40,6 (107) | 1,5 ; 4,4 — 9,4 ; 17,4 — 5,6 ; 8,2 |
| avec calage, avec glisser | 40,9 ; 49,7 ; 49,7 | 7,6 ; 16,2 — 0,06 ; 0,9 — 24,4 ; 25,2 — 7,0 ; 8,3 | 38,9 ; 48,1 | 42,7 ; 69,9 ; 70,2 (107) | 1,5 ; 5,7 — 7,3 ; 21,5 — 22,8 ; 41,2 |
| sans calage, avec glisser | 38,9 ; 50,7 ; 51,3 | 8,4 ; 18,0 — 0,06 ; 0,9 — 22,7 ; 25,4 — 7,2 ; 8,4 | 38,0 ; 49,0 | 45,7 ; 62,9 ; 63,0 (134) | 1,5 ; 5,4 — 11,6 ; 20,9 — 21,9 ; 39,9 |

**Avec la fenêtre du navigateur** (`--avec-interface`, sur l'écran de la machine, allumé, sous `caffeinate -d` ; `ardoise` seulement, le clavier et la souris ; `m2-ardoise-<n>-interface`, le 2026-11-08 à 22 h 03 et 22 h 05 UTC ; charge au départ, à la fin, la plus haute pendant : Chrome 2,82, 1,92, 2,82 ; Firefox 1,92, 1,81, 2,23 ; aucun passage écarté). L'écran de la machine donne **120 images par seconde** à la page dans les deux navigateurs ; Firefox y dessine par WebGPU (sans interface, par le repli WebGL2) :

| Condition | Chrome, clavier, total | Chrome : attente de l'image ; GPU | Chrome, souris, total | Firefox, clavier, total | Firefox : attente de l'image ; GPU | Firefox, souris, total |
|---|---|---|---|---|---|---|
| avec calage, sans glisser | 20,3 ; 28,5 ; 28,5 (132) | 8,0 ; 9,2 — 2,5 ; 4,1 | 25,3 ; 28,5 (18) | 67,2 ; 124,6 ; 125,5 (132) | 7,8 ; 9,8 — **51,1 ; 103,6** | 55,2 ; 107,2 (18) |
| sans calage, sans glisser | 19,0 ; 32,5 ; 33,3 (129) | 4,1 ; 10,0 — 2,8 ; 3,9 | 18,6 ; 29,7 | 67,1 ; 124,0 ; 133,1 | 3,5 ; 8,7 — **50,7 ; 109,5** | 62,9 ; 100,6 |
| avec calage, avec glisser | 34,3 ; 45,5 ; 46,7 | 23,9 ; 25,9 — 2,4 ; 3,3 | 34,5 ; 43,1 | 80,3 ; 137,5 ; 140,1 | 24,0 ; 26,2 — **48,7 ; 101,3** | 69,6 ; 131,8 |
| sans calage, avec glisser | 34,4 ; 46,7 ; 47,0 | 20,6 ; 26,3 — 2,3 ; 3,0 | 34,8 ; 42,6 | 81,7 ; 134,1 ; 139,0 | 22,2 ; 25,5 — **49,2 ; 101,8** | 97,3 ; 135,0 |

À 120 Hz, glisser est ce que la télé fait d'elle-même (§ 2.9). **Chrome tient le budget du clavier partout** : 28,5 et 32,5 ms au 99ᵉ centile sans glisser, ~~45,5 et 46,7 ms avec~~ (*corrigé après la vérification* : `ardoise` n'a aucun nœud qui glisse, ces deux chiffres ne mesuraient que le retard de la trace ; voir « M2 corrigée : glisser »). **Firefox avec sa fenêtre ne le tient nulle part, selon la définition du pixel** (la fin du travail du GPU, `onSubmittedWorkDone`) : 124 à 137 ms au 99ᵉ centile au clavier ; **le segment qui pèse le plus est le GPU**, 33 à 63 ms en médiane, 84 à 110 ms au 99ᵉ centile, pour un dessin soumis en 0,6 à 1,6 ms et une attente de l'image de 8 à 10 ms (26 avec glisser). Que l'image soit à l'écran si tard n'est **pas établi** : l'instant mesuré est celui où Firefox résout la promesse de `onSubmittedWorkDone`, qui peut suivre la présentation de loin ; seule la caméra le dira (la fiche de répétition). Le contrôle négatif se voit dans les deux (33,15 et 33,18 ms en médiane pour le dessin, au lieu de 0,57 et 0,72).

Le pas et les vues, dans le Worker : 0,2 à 0,3 ms en médiane, au plus 1 ms (le clavier ; 0,03 à 0,06 ms dans Chrome écran allumé), 0,6 à 0,7 ms et au plus 3,4 ms pour le pas de Punchline. L'événement (de `event.timeStamp` au `postMessage`) : 0,2 à 0,4 ms en médiane, au plus 1,4 ms ; il n'est pas mesuré pour la souris (l'événement passe par la file de la page, son instant est celui de l'envoi) ni pour un téléphone natif (pas de DOM). Le dessin lui-même (du début de l'image au dessin soumis) : 0,7 à 2,5 ms en médiane, au plus 6,1 ms. Les traces sans pixel (la scène n'a pas changé : un chiffre de plus sur une ardoise pleine, un geste que Punchline ignore) sont comptées à part : 0 à 4 sur 132 au clavier, 10 à 17 sur 102 à 151 au téléphone.

*Ajouté après la vérification du lot 13* : **des gestes de téléphones ont attendu leur image de 1,3 à 1,9 s**, ceux des pas 17 à 47 de `m2-punchline-chrome-ecran-allume`, la première seconde de la partie, sur les six téléphones (`lignes.txt` de son dossier de travail, lignes 47 à 52) ; ils se rattachent tous à la même image, la première qui dessine la scène du jeu après le clic sur « Jouer ». Le banc ne les compte pas (ils tombent avant la première condition) ; le panneau, qui garde les 200 derniers gestes depuis le début, les montre (99ᵉ centile du téléphone à 1 614 ms à la fin du passage ; de 717 à 985 ms sur Sidequest, à l'essai `target/travail/lot-13-corr/essai-sidequest.txt`). La cause probable est la transition depuis l'écran du commun : tant que la page dessine sa scène, aucune image ne dessine celle du jeu, et la trace attend ; ce n'est pas établi. Le panneau d'une soirée en dira autant après chaque « Jouer » : à regarder au lot 14 (`RESTE-PHASE-3.md`).

**Ce que ces chiffres disent, et ne disent pas** :

- **Le budget du CONTRAT (99 % sous 50 ms au clavier, sous 80 ms au téléphone, le matériel à part) est tenu sans glisser dans les deux navigateurs, écran allumé** : au clavier, 34,5 et 35,5 ms au 99ᵉ centile dans Chrome (60 Hz), 32,9 et 35,3 ms dans Firefox (120 Hz) ; à la souris, 34,8 à 38,7 ms ; au téléphone natif, 30,5 à 42,0 ms. ~~**Avec glisser forcé**, le clavier le dépasse d'un rien (51,0 et 51,1 ms dans Chrome à 60 Hz, 49,7 et 50,7 ms dans Firefox à 120 Hz) : le segment qui pèse le plus est l'attente de l'image (33,8 ms au 99ᵉ centile dans Chrome, 25,2 et 25,4 ms dans Firefox), le pas de retard que coûte glisser ; le téléphone reste sous 80 (63 à 70 ms).~~ (*Corrigé après la vérification* : sans nœud qui glisse, ces chiffres ne mesuraient que le retard de la trace ; refaits sur la page corrigée, « avec glisser » vaut « sans glisser » chez `ardoise`, plus bas.) **Écran éteint, Chrome ne le tient plus au clavier** : 69 à 73 ms au 99ᵉ centile, 67 à 70 ms à la souris, l'attente de l'image d'une page à 30 Hz pesant le plus (32 à 35 ms), puis le passage de la scène du Worker à la page (12 à 26 ms au 99ᵉ centile au clavier, sous 5 ms écran allumé). Le premier agent avait nommé ce segment fautif et l'avait envoyé à `RESTE-PHASE-3.md` : c'était l'écran éteint, que la télé d'une soirée n'a pas ; le point y est réécrit (`LOTS-PHASE-3.md`, « Lot 13 »). Rien n'est corrigé par un seuil.
- **Sans calage** (la minuterie du Worker seule, à 4 ms) : pas plus lent que le calage, et l'attente du pas a la même médiane (7 à 10 ms, la moitié d'un pas) ; au 99ᵉ centile, elle est même plus courte dans Chrome écran éteint (18,6 contre 27,7 ms au clavier) ; écran allumé, 19,6 contre 17,8 ms. Avec la minuterie de secours du lot 10a, qui joue avant l'image quand celle-ci tomberait tard, le calage n'achète rien de mesurable sur cette machine : une donnée pour P6, pas une décision.
- ~~**Glisser** coûte un pas au pixel des nœuds qui glissent, comme prévu : l'attente de l'image passe de 7,6 à 24,4 ms en médiane dans Firefox (clavier), et le total de 25,5 à 40,9 ms. Dans Chrome à 60 Hz (écran allumé, glisser forcé), il coûte une image entière : l'attente de l'image passe de 16,5 à 32,4 ms en médiane, le total au clavier de 25,8 à 40,6 ms. Dans Chrome à 30 Hz (écran éteint), le pas de retard se fond dans l'image de 33 ms. Glisser n'est donc à garder qu'à 100 Hz et plus, comme le fait la télé (§ 2.9).~~ *Faux, corrigé après la vérification* : aucun nœud ne glissait chez `ardoise` et Punchline ; ce retard était celui de la trace, pas de l'image. Le vrai coût de glisser est mesuré sur Sidequest, plus bas.
- **Ce n'est pas la télé de la soirée** : un navigateur sans interface (60 Hz dans Chrome écran allumé, 30 écran éteint ; 120 Hz virtuels dans Firefox), aucune composition par l'écran, aucune dalle ; le « pixel » est le GPU fini. La cadence de l'écran de la machine se mesure `--avec-interface` (plus bas, si elle a pu se faire au calme). Les téléphones sont natifs (pas de navigateur de téléphone, pas de Wi-Fi) : leur transport (1,2 à 1,5 ms en médiane, au plus 8 ms) est celui de la boucle locale, à leur incertitude d'écho près (au plus 1,5 ms).
- **Le contrôle négatif** (le dessin ralenti de 33 ms par `window.__banc.dessin_lent`) se voit dans le segment du dessin : 33,17 ms en médiane au lieu de 0,18 dans Chrome écran allumé (33,15 au lieu de 0,82 écran éteint), 33,28 au lieu de 1,56 dans Firefox. Les traces mesurent ce qu'elles disent.

### M2 corrigée : glisser (après la vérification du lot 13)

**Ce qui était faux** : la page glissait après chaque scène neuve dès que glisser était forcé (ou à 100 Hz et plus), même quand aucun nœud ne glisse, et la trace attendait la fin du chemin (16,7 ms) ; `ardoise` et Punchline n'ont aucun nœud `glisse`. Les lignes « avec glisser » des tableaux plus haut ne mesuraient donc que ce retard de la trace. **Corrigé** (`rendu::glisser::deplace_des_noeuds`, `web/ecran/src/page/mesure.rs`) : la page ne glisse que si un nœud qui glisse se déplace ; le pixel d'une entrée est la première image où chaque nœud est au moins à sa place du pas de l'entrée (sa scène dessinée en entier, ou un chemin qui part d'elle ou d'une plus récente) ; et le banc dit, par condition, combien de scènes ont vraiment glissé (`window.__banc.glisses()`). Refait le 2026-11-08 de 22 h 31 à 22 h 58 UTC, sur la page corrigée (paquet `target/soiree-web/5d4fb859…`, `soiree-banc` reconstruit à 22 h 29), chaque passage par `lourd --calme caffeinate -d` (l'écran de la machine allumé, `pmset` : allumé depuis 18 h 52), **la règle posée avant les tours** (`target/travail/lot-13-corr/campagne.sh`, son journal) : la charge la plus haute pendant **et la charge à la fin** sous 3, quatre essais au plus. Aucun passage écarté pour la charge. Preuves dans `target/preuves/lot-13-corr/<nom>/` ; tableaux entiers dans `target/travail/lot-13-corr/tableau-m2-*.md`. Modules : Sidequest `e80b86a9…` (celui du lot 7b), `ardoise` `400cc5ae…`.

**Un vrai glisser : Sidequest**, le seul module qui marque des nœuds `glisse` (le personnage et sa bulle, `vues.rs`) ; les gestes de ses six téléphones natifs déplacent les personnages. `soiree-banc mesures <sidequest_module.wasm> --navigateurs <n> --entrees 0 --duree 30 --echos 200 --sans-construire [--avec-interface]`. Avec glisser, 1 233 à 1 642 scènes ont vraiment glissé par condition, et 1 239 à 3 201 images ont été dessinées à mi-chemin (deux par pas à 120 Hz) ; sans glisser, aucune. Au téléphone, en ms, « médiane ; 99ᵉ centile ; maximum » du total, puis l'attente de l'image (médiane ; 99ᵉ) :

| Passage (charge au départ ; à la fin ; la plus haute pendant) | images/s | avec calage, sans glisser | sans calage, sans glisser | avec calage, avec glisser | sans calage, avec glisser |
|---|---|---|---|---|---|
| `m2-sidequest-chrome`, sans interface (1,78 ; 1,65 ; 2,36) | 60 | 31,2 ; 42,5 ; 44,2 (126) — 15,1 ; 16,9 | 29,3 ; 40,2 ; 42,9 (118) — 13,8 ; 16,8 | 37,0 ; 49,7 ; 50,1 (110) — 20,4 ; 32,0 | 42,5 ; 54,9 ; 55,2 (101) — 27,5 ; 30,4 |
| `m2-sidequest-firefox`, sans interface, WebGL2 (1,65 ; 2,20 ; 2,69) | 120 | 29,4 ; 43,4 ; 44,4 (133) — 6,7 ; 9,2 | 27,3 ; 43,9 ; 44,2 (121) — 6,2 ; 9,6 | 46,2 ; 59,5 ; 63,0 (128) — 23,4 ; 25,4 | 41,2 ; 57,7 ; 57,7 (107) — 20,2 ; 24,0 |
| `m2-sidequest-chrome-interface`, avec sa fenêtre (2,18 ; 2,09 ; 2,18) | 120 | 24,9 ; 35,8 ; 35,9 (127) — 7,8 ; 9,9 | 19,8 ; 34,4 ; 34,5 (116) — 2,6 ; 8,9 | 39,3 ; 49,3 ; 49,3 (141) — 23,9 ; 25,1 | 33,6 ; 50,7 ; 50,7 (115) — 17,7 ; 24,8 |
| `m2-sidequest-firefox-interface`, avec sa fenêtre, WebGPU (2,09 ; 2,63 ; 2,91) | 120 | 38,4 ; 71,8 ; 101,0 (133) — 7,2 ; 9,2 | 36,4 ; 77,4 ; 78,2 (109) — 3,4 ; 9,6 | 47,2 ; **80,2** ; 93,4 (148) — 23,1 ; 24,9 | 41,9 ; 70,0 ; 89,7 (107) — 17,7 ; 25,4 |

- **Glisser coûte un pas au pixel des gestes qui déplacent un nœud** : l'attente de l'image monte de 14 à 17 ms en médiane à 120 Hz (un pas, 16,7 ms : le chemin d'un pas, puis l'image où la scène suivante part de la sienne), de 5 à 14 ms dans Chrome sans interface à 60 Hz ; au 99ᵉ centile, de 15 à 16 ms. Le total au 99ᵉ centile monte de 7 à 16 ms.
- **Le budget du téléphone (80 ms au 99ᵉ centile) est tenu avec glisser** dans Chrome (49 à 55 ms, avec sa fenêtre ou sans) et dans Firefox sans interface (58 et 60 ms). **Firefox avec sa fenêtre le dépasse d'un rien dans une condition** (avec calage, avec glisser : 80,19 ms) ; le banc nomme le segment qui pèse le plus : **le GPU** (65,2 ms au 99ᵉ centile), comme au clavier plus haut ; sans glisser, Firefox avec sa fenêtre est déjà à 72 et 77 ms au téléphone, le GPU à 53 à 60 ms au 99ᵉ centile. Rien n'est corrigé par un seuil : `RESTE-PHASE-3.md`.
- **Au clavier, aucun module ne glisse** : le coût de glisser n'y est pas mesuré ; d'après le téléphone, ce serait un pas de plus au pixel des seules touches qui déplacent un nœud qui glisse.

**`ardoise` sur la page corrigée : « avec glisser » vaut « sans glisser »**, et le rapport le dit (« aucune scène où un nœud glisse ; glisser n'a rien déplacé »). Au clavier, total « médiane ; 99ᵉ ; max » (n), et à la souris le 99ᵉ centile :

| Passage (charge au départ ; à la fin ; la plus haute pendant) | avec calage, sans glisser | sans calage, sans glisser | avec calage, avec glisser | sans calage, avec glisser | contrôle négatif (dessin, médiane) |
|---|---|---|---|---|---|
| `m2-ardoise-chrome`, sans interface, 60 images/s (2,63 ; 1,09 ; 2,63) | 29,0 ; 38,2 ; 38,5 (132) ; souris 36,3 | 27,8 ; 37,6 ; 38,1 (131) ; souris 36,1 | 27,9 ; 36,9 ; 37,7 (132) ; souris 36,1 | 27,0 ; 36,8 ; 36,9 (130) ; souris 35,1 | 33,16 au lieu de 0,83 |
| `m2-ardoise-firefox`, sans interface, 120 images/s (1,09 ; 1,44 ; 1,55) | 27,2 ; 40,5 ; 41,9 (132) ; souris 35,5 | 24,3 ; 38,1 ; 41,5 (129) ; souris 32,1 | 27,1 ; 42,4 ; 42,4 (129) ; souris 38,3 | 26,0 ; 37,9 ; 42,5 (128) ; souris 32,5 | 33,32 au lieu de 1,44 |
| `m2-ardoise-chrome-interface`, avec sa fenêtre, 120 images/s (2,58 ; 1,69 ; 2,58) | 20,3 ; 29,9 ; 30,4 (107) | 18,9 ; 33,6 ; 34,0 (112) | 19,7 ; 29,1 ; 36,9 (115) | 18,7 ; 32,3 ; 39,4 (104) | 33,17 au lieu de 0,49 |

- Avec glisser, le total au 99ᵉ centile reste dans l'étendue de « sans glisser » de chaque passage (Chrome 36,8 et 36,9 contre 37,6 et 38,2 ; Firefox 37,9 et 42,4 contre 38,1 et 40,5 ; Chrome avec sa fenêtre 29,1 et 32,3 contre 29,9 et 33,6), et l'attente de l'image ne monte plus (Chrome 15,9 contre 16,0 en médiane ; Firefox 7,7 contre 7,6 ; avec la fenêtre 8,0 contre 7,9). Les 45,5 à 46,7 ms (Chrome avec sa fenêtre) et 49,7 à 51,1 ms (sans interface) du premier passage n'existent plus.
- **Avec la fenêtre de Chrome, le plein écran est quitté aussitôt entré**, dans les deux passages de la correction (`m2-ardoise-chrome-interface-plein-ecran-quitte` à 22 h 45, puis `m2-ardoise-chrome-interface` à 22 h 56 ; 87 et 85 lignes « plein écran quitté »), alors qu'il tenait au passage de la reprise (22 h 03, une seule) ; aussi dans `m2-sidequest-chrome-interface` (22 h 37). ~~Cause non trouvée (le plein écran natif de macOS, comme pour Firefox au lot 10b ? l'état du bureau de l'hôte ? pas établi)~~ *cause trouvée au lot 13b : la session de l'écran de l'hôte verrouillée depuis 22 h 24 (plus bas, « Lot 13b »)* ; la page n'a pas changé sur ce point. Le voile du commun (« Plein écran quitté — cliquer pour revenir ») couvre alors le jeu : **les clics sur « Effacer » vont au voile, aucun n'arrive au jeu**, la souris n'a donc aucune trace dans ces deux passages, et l'ardoise, jamais effacée, se remplit : 17 à 29 touches sur 132 n'ont pas de pixel (la scène ne change plus). Le clavier, lui, arrive au jeu ; ses traces avec pixel valent mesure, le voile (un élément du DOM) par-dessus la toile. Le premier passage est gardé sous `-plein-ecran-quitte` et refait une fois sous la même règle ; le second a montré la même chose, et n'a pas été refait de nouveau.
- M14 dans ces passages : la page et son Worker à 0,01 ms au plus ; les processus de Chrome à 0,35 ms au plus ; ceux de Firefox de 0,13 à 0,81 ms (`--echos 200`, mesurés désormais avant les conditions).

### M14 : l'écart des horloges entre processus

Dans les mêmes passages, 200 échos par sorte : l'écart est « l'autre horloge moins celle de la page », la médiane des échos du dixième le plus court, à un demi-aller-retour près.

| Passage | page ↔ Worker de l'autorité | page ↔ processus du banc (et des natifs) | page ↔ un autre onglet, même origine |
|---|---|---|---|
| `m2-ardoise-chrome-ecran-allume` | 0,00 ms, à 0,00 près | 0,12, à 0,03 | 0,08, à 0,02 |
| `m2-punchline-chrome-ecran-allume` | 0,00, à 0,00 | 0,15, à 0,03 | 0,03, à 0,02 |
| `m2-ardoise-chrome` | 0,00 ms, à 0,01 près | −0,02, à 0,05 | 0,01, à 0,04 |
| `m2-punchline-chrome` | 0,00, à 0,02 | 0,04, à 0,05 | 0,05, à 0,05 |
| `m2-ardoise-chrome-interface` | 0,00, à 0,01 | 0,37, à 0,05 | 0,21, à 0,02 |
| `m2-ardoise-firefox` | 0,00, à 0,01 | **0,51**, à 0,07 | **−0,50**, à 0,02 |
| `m2-punchline-firefox` | 0,00, à 0,02 | **0,99**, à 0,07 | **0,26**, à 0,02 |
| `m2-ardoise-firefox-interface` | 0,00, à 0,01 | **2,70**, à 0,08 | **1,67**, à 0,02 |

- **La page et son Worker partagent leur horloge** à la granularité près, dans Chrome comme dans Firefox : les segments qui franchissent la page et le Worker (transport, vers la page) ne portent aucun écart mesurable.
- **Dans Chrome, les processus s'accordent** à 0,4 ms près (0,37 ms au plus, avec sa fenêtre ; à 0,05 ms près dans les deux passages écran éteint).
- **Dans Firefox, un autre processus s'écarte de 0,3 à 2,7 ms**, et pas de la même valeur d'un lancement à l'autre (−0,50, +0,26 puis +1,67 ms pour un autre onglet ; +0,51, +0,99 puis +2,70 ms pour l'horloge murale du banc) : chaque processus ancre son `timeOrigin` à son départ. Un segment qui franchit deux processus de Firefox porte jusqu'à 3 ms d'incertitude de plus ; la trace d'un téléphone natif, dont les instants sont l'heure de l'autorité qu'il estime par écho, n'en dépend pas (son incertitude est son demi-aller-retour).

### M6 : l'arrière-plan avec interface

`soiree-banc arriere-plan <punchline> --navigateurs <n> --cache 480 --avec-interface --mode recouverte|reduite --sans-construire`, sous `caffeinate -d` (l'écran de la machine allumé) : le navigateur avec sa fenêtre sur l'écran de l'hôte, la partie de Punchline avec ses six téléphones natifs ; au bout d'une minute, la fenêtre de l'écran est **recouverte** (une autre fenêtre du même navigateur ouverte et activée par-dessus : `Target.createTarget { newWindow }` puis `Target.activateTarget` dans Chrome, `browsingContext.create { type: window }` puis `activate` dans Firefox) ou **réduite** (`Browser.setWindowBounds { windowState: minimized }` dans Chrome, `browser.setClientWindowState { state: minimized }` dans Firefox), **480 s** (plus de sept minutes), avec trois réveils du Worker gelés de 150 ms vingt secondes avant la fin (le retard injecté, dont les trois sauts doivent se rattacher) ; puis la fenêtre rendue. Le banc juge chaque saut du journal ; ce que la page reçoit d'images se note, sans être jugé (M6 le mesure).

| Passage | Charge au départ ; à la fin ; la plus haute pendant | `visibilityState` | Pas joués pendant les 480 s : par la minuterie ; au signal d'une image | Tours de plus de 10 ms ; trous de 30 ms ou plus (hors retard injecté) | Verdict |
|---|---|---|---|---|---|
| `m6-recouverte-chrome` | 1,93 ; 2,75 ; 2,64 | `visible` | 3 ; 28 549 | 0 (le plus long 0,2 ms) ; 0 | réussi : 3 sauts, tous rattachés au retard injecté |
| `m6-recouverte-firefox` (un passage écarté avant : 4,32 pendant) | 2,43 ; 2,33 ; 2,43 | `visible` | 7 ; 28 617 | 0 (0,4 ms) ; 0 | réussi : 3 sauts, rattachés |
| `m6-reduite-chrome` (deux passages écartés avant : 50,11 et 5,64 pendant) | 2,33 ; 2,08 ; 2,92 | `visible` | 874 ; 27 727 | 0 (0,2 ms) ; 0 | réussi : 3 sauts, rattachés |
| `m6-reduite-firefox` (seconde série, même règle posée avant ; les quatre de la première écartés : 35,85, 3,69, 5,04 et 4,08 pendant) | 1,81 ; 1,83 ; 2,15 | `hidden` | 28 557 ; 0 | 0 (0,5 ms) ; 0 après les deux premières secondes | **échec** : un saut d'un pas, (688, 689), 0,7 s après la réduction, au tour qui suit un réveil de la minuterie revenu au bout de 33,0 ms (tour de 3,1 ms) ; les 3 sauts du retard injecté rattachés |

**Ce que M6 dit, et ne dit pas** :

- **Recouverte, la fenêtre reste `visible` et reçoit ses images** dans les deux navigateurs : la page joue le pas au signal de l'image comme au premier plan (28 549 et 28 617 pas sur 28 750 environ), le Worker n'est pas bridé, aucun tour long, aucun trou. Ni Chrome ni Firefox ne traitent ici une fenêtre recouverte par une autre de leurs fenêtres comme cachée (l'occlusion de macOS ne joue pas, ou ne joue pas sur une fenêtre du même navigateur : pas établi).
- **Réduite, Chrome reste `visible` et reçoit ses images** (27 727 pas au signal d'une image, 874 par la minuterie) : la commande de réduction est acceptée sans erreur, mais la réduction n'est pas constatée par la page ; le banc ne relit pas l'état de la fenêtre (`Browser.getWindowBounds`), et un essai du premier agent, écran éteint, 40 s, avait vu au contraire la minuterie jouer chaque pas (`target/travail/lot-13/essai-m6/`). **Ce que fait Chrome d'une fenêtre vraiment réduite n'est donc pas établi** ; le lot 14 (la fiche de répétition) le regarde à l'œil. *Au lot 13b* (plus bas) : le banc relit l'état ; Chrome dit sa fenêtre `minimized` et la page reste `visible`, reçoit ses images, mais session de l'écran verrouillée, où même un onglet passé derrière reste visible.
- **Réduite, Firefox passe à `hidden`** et ne reçoit plus d'images : la minuterie du Worker joue chaque pas, comme un onglet caché (lot 10a). Le passage valide (seconde série, à 22 h 05, charge au plus 2,15) **échoue d'un saut d'un pas**, juste après la réduction, au tour qui suit un réveil de la minuterie revenu au bout de 33,0 ms : le cas de la question 30, sur machine calme, avec interface. Les quatre passages de la première série (écartés, la machine de l'hôte chargée par d'autres travaux entre 21 h 01 et 21 h 32) ne sont pas des mesures : le deuxième (3,69 pendant) réussi, aucun saut hors du retard injecté ; le premier (35,85 pendant) neuf sauts d'un à douze pas après des trous de 39 à 219 ms ; le troisième (5,04 pendant) un saut d'un pas, (12604, 12605), après un réveil de la minuterie revenu au bout de 38,5 ms, tour de 0,6 ms au plus ; le quatrième s'est arrêté au bout de 149 s, « BiDi fermé par le navigateur » (la fenêtre de Firefox fermée, par qui ou quoi, pas établi). C'est le cas de la question 30.
- **Écran de la machine éteint** (les deux passages « recouverte » du premier agent, 07 h 39 à 07 h 56, `m6-recouverte-<n>-ecran-eteint`, charge au plus 2,24 et 2,32 ; gardés comme constat, pas comme M6) : Chrome ne recevait plus d'images (28 543 pas par la minuterie, 1 au signal d'une image ; huit trous de 30,0 à 30,6 ms, aucun saut) ; Firefox en recevait encore (24 805 au signal d'une image, 3 810 par la minuterie ; sept tours de 9,7 à 10,5 ms, les tranches du scellement d'un point) et **un saut d'un pas inexpliqué**, (10100, 10101), sans tour long ni trou de 30 ms relevé près de ce pas (le banc ne garde que les tours de plus de 10 ms ou après un trou de 30 ms) : versé à la question 30.
- **App Nap** : rien ne l'a montré ; le Worker a joué chaque pas en 480 s dans chaque passage valide. Le seuil de « 100 ms trois fois de suite » (P5, choisi) n'a été franchi que par le retard injecté, chaque fois signalé.
- **Ce que M6 ne dit pas** : un navigateur qui passe en arrière-plan sur une machine qui dort (l'ordinateur de la soirée reste éveillé, P38), les modes d'économie d'énergie de Chrome (non réglés, ceux du profil neuf), Safari (pas exécuté).

Pour refaire : `target/hote/lourd --calme caffeinate -d target/release/soiree-banc arriere-plan <punchline_module.wasm> --navigateurs <n> --cache 480 --avec-interface --mode recouverte|reduite --sans-construire --travail <dossier>` (`target/travail/lot-13/campagne.sh m6`).

### M1 remesurée : le banc et le harnais retiré, en alternance, sur machine calme

Reportée du lot 3c (question 27). `target/hote/lourd --calme /bin/zsh -c "cd target/travail/lot-3c/harnais/mesures/navigateur && node mesurer.mjs --navigateurs chrome,firefox --tour <10+n>"` (le harnais retiré, reconstruit au lot 3c depuis `44160f9` ; son analyse, `node analyser.mjs --tour <10+n>`, copiée sous `target/travail/lot-13/analyse-harnais-tour-<n>.json`) et `target/hote/lourd --calme target/release/soiree-banc etalonner --mesures m1 --navigateurs chrome,firefox --sans-construire` (son `analyse.json` dans la preuve), en alternance, huit tours prévus de chaque, la règle de validité posée avant les tours (la charge la plus haute pendant sous 3 ; quatre essais au plus par tour ; trois le matin). Les cinq modules ensemble, appels d'au moins 10⁶ unités, en ps par unité, « médiane ; 99ᵉ centile ; maximum ». Navigateurs sans interface. Les tours 1 et 2 du harnais et le tour 2 du banc sont du premier agent (05 h 51 à 06 h 00, écran de la machine éteint) ; les autres de la reprise (21 h 45 à 22 h 01, écran allumé) : M1 compte des unités de calcul, et rien n'y a montré l'effet de l'écran, mais ce n'est pas établi. **Valides : huit tours du harnais, sept du banc** (*sept du harnais après la correction* : le tour 1 écarté, plus bas) (le tour 1 du banc n'en a aucun : sept essais écartés en tout, de 3,12 à 5,60 pendant ; écartés aussi, pour les autres tours, dix-neuf essais de 3,01 à 4,23 pendant, et un du matin interrompu par l'arrêt du premier agent). Charge la plus haute pendant des tours valides : 2,30 à 2,93. `target/travail/lot-13/comparer2.py` relit chaque tour valide et compare.

| Tour | harnais, Chrome | banc, Chrome | harnais, Firefox | banc, Firefox |
|---|---|---|---|---|
| 1 (écarté à la correction) | ~~35 ; 69 ; 86~~ | — | ~~69 ; 120 ; 265~~ | — |
| 2 | 36 ; 56 ; 91 | 31 ; 51 ; 57 | 64 ; 106 ; 194 | 62 ; 98 ; 286 |
| 3 | 36 ; 64 ; 136 | 36 ; 65 ; 291 | 68 ; 124 ; 230 | 64 ; 123 ; 225 |
| 4 | 39 ; 78 ; 134 | 36 ; 47 ; 493 | 66 ; 120 ; 120 | 64 ; 120 ; 222 |
| 5 | 37 ; 59 ; 67 | 37 ; 60 ; 430 | 65 ; 133 ; 198 | 65 ; 120 ; 217 |
| 6 | 36 ; 69 ; 87 | 36 ; 49 ; 60 | 70 ; 135 ; 148 | 63 ; 115 ; 592 |
| 7 | 36 ; 71 ; 98 | 28 ; 61 ; 295 | 65 ; 135 ; 318 | 65 ; 106 ; 376 |
| 8 | 36 ; 75 ; 133 | 37 ; 58 ; 318 | 65 ; 120 ; 124 | 66 ; 120 ; 225 |
| **étendue** | **36 à 39 ; 56 à 78 ; 67 à 136** (35 à 39 avec le tour 1) | **28 à 37 ; 47 à 65 ; 57 à 493** | **64 à 70 ; 106 à 135 ; 120 à 318** | **62 à 66 ; 98 à 123 ; 217 à 592** |

- **Aucun écart de carburant, d'issue ou de retour**, aucune panne, aucun défaut, dans chaque tour du banc (« 0 écarts, 0 pannes » pour chaque module et chaque rejeu).
- *Corrigé après la vérification du lot 13* : **le tour 1 du harnais est écarté**. Sa charge la plus haute pendant, écrite par le portillon, est 2,87, mais sa charge à la fin est 3,08 : le portillon relève la charge toutes les 10 s et ne compte pas la valeur finale ; la moyenne d'une minute a donc atteint 3 pendant la dernière minute du tour. Selon la lettre de la règle, le tour valait ; selon son esprit, non. Aucun autre tour gardé n'a une charge de fin de 3 ou plus. **Sept tours valides du harnais, sept du banc.** Sans lui, seule la médiane de Chrome change (36 à 39 au lieu de 35 à 39) ; le reste est identique (`python3 -I target/travail/lot-13-corr/comparer3.py`, qui écarte aussi un tour dont la charge de fin atteint 3 ; sa sortie dans `comparer3-sortie.txt`). La règle de la correction compte désormais la charge de fin.
- **Les médianes** : le banc n'est jamais au-dessus de l'étendue du harnais (Chrome 28 à 37 pour 36 à 39 ; Firefox 62 à 66 pour 64 à 70) ; deux tours de Chrome sont au-dessous (31 et 28). Les deux tours du lot 3c à 46 et 49 ps, au-dessus de 43, ne se retrouvent pas sur machine calme selon cette règle.
- **Les 99ᵉ centiles** : le banc dans l'étendue du harnais ou au-dessous, dans les deux navigateurs (Chrome 47 à 65 pour 56 à 78 ; Firefox 98 à 123 pour 106 à 135).
- **Les maxima ne se retrouvent pas** : Chrome, banc jusqu'à 493 ps, harnais au plus 136 ; Firefox, banc jusqu'à 592, harnais au plus 318. Comme au lot 3c, le maximum du banc tombe dans Chrome sur la soirée de Tier list (sept tours sur sept ; 291 à 493 ps dans cinq tours), et dans Firefox sur un appel lourd de Punchline (six tours sur sept, 217 à 592 ps ; Tier list au tour 7, 376). Le Worker du banc a donc, sur un appel sur des milliers, des pauses que la page du harnais n'a pas ou a moins : la question 27 garde ces chiffres, sans être tranchée.

Pour refaire : `target/travail/lot-13/campagne.sh m1` (sa règle et ses essais), puis `python3 -I target/travail/lot-13/comparer2.py`.

## Lot 13b : la fenêtre réduite de Chrome et le plein écran

Le 2026-11-08, de 23 h 23 à 23 h 37 (UTC), MacBook Apple M5 (10 cœurs, 16 Go), macOS 27.2, Chrome 156.0.8078.80 avec sa fenêtre (`--avec-interface`) ; chaque passage par `target/hote/lourd --calme caffeinate -d`, la règle de validité posée avant les passages (`target/travail/lot-13b/campagne.sh`, son journal `campagne-journal.txt`) : la charge la plus haute pendant **et** la charge de fin, écrites par le portillon, sous 3 ; aucun passage écarté. **La session de l'écran de macOS était verrouillée** pendant toutes ces mesures (`ioreg -n Root -d1`, `IOConsoleUsers` : `CGSSessionScreenIsLocked = Yes` depuis 22 h 24 min 04 s, relevé dans chaque `empreinte.txt`) ; l'écran allumé (`caffeinate -d`). Déverrouiller demande le mot de passe de l'hôte : rien n'a été changé, et ce qui en dépend est dit plus bas.

### Le plein écran quitté aussitôt entré : la session verrouillée

- **La cause** : la session de l'écran de l'hôte est verrouillée depuis 22 h 24 min 04 s (`CGSSessionScreenLockedTime` 1794176644). Le plein écran tenait au passage de 22 h 03 (`m2-ardoise-chrome-interface` du lot 13 : entré à 2 397 ms, quitté seulement à 111 082 ms, à la fermeture) ; il est quitté aussitôt entré dans chaque passage avec la fenêtre de Chrome depuis (22 h 37, 22 h 45, 22 h 56 à la correction du lot 13 ; 23 h 33 et 23 h 37 ici : `plein-ecran` `dedans: true` puis `false` 8 ms après, à 2 349 et 2 357 ms). Ce qu'on en tire : session verrouillée, la fenêtre n'entre pas dans le plein écran de macOS ; Blink l'a déjà annoncé à la page (`fullscreenchange`, `dedans: true`), puis Chrome le quitte. **Ni le banc ni la page** : le geste, le plein écran demandé, le verrou d'Échap et le voile sont ceux de 22 h 03 (le paquet de la page n'a pas changé sur ce point), et la page fait ce que P38 demande d'un plein écran quitté (le voile « Plein écran quitté — cliquer pour revenir »). C'est l'état de la machine, que le banc lit désormais. *Ce qui n'est pas prouvé* : la contre-épreuve, le même passage session déverrouillée après 22 h 24, n'a pas pu être faite (l'hôte absent) ; la cause tient à la coïncidence des heures (tenu dans le seul passage d'avant le verrou, quitté dans chacun des cinq d'après), pas à une expérience dans les deux sens.
- **Ce que le commun corrige** : `soiree-banc mesures` dit la session de l'écran au rapport (`navigateur::note_de_la_session`), regarde une seconde après « Jouer » si le voile du commun couvre le jeu (`mesures::voile_sur_le_jeu`), et compte, dans chaque condition, les clics allés au voile (`mesures::clics_au_voile` : chacun y refait le geste, une ligne `geste`) ; l'un ou l'autre est une **faute** (code 1, « un navigateur n'a pas pu mesurer »). Avant, ces passages rendaient le code 0 avec une souris sans trace.
- **`m2-ardoise-chrome-interface-verrouillee`** (charge 2,15 ; 2,03 ; 2,50 au plus pendant ; code 1) : les cinq fautes dites (le voile une seconde après « Jouer », puis 18, 18, 16 et 16 clics allés au voile dans les quatre conditions), 74 lignes `geste` en tout ; **la souris n'est pas mesurée**. Le clavier arrive au jeu (le voile est un élément du DOM par-dessus la toile, les touches vont au jeu) : 34,5 ms au 99ᵉ centile, avec calage, sans glisser (20,8 en médiane ; 109 traces avec leur pixel sur 132 ; l'ardoise, jamais effacée, se remplit, et les touches sans changement de scène n'ont pas de pixel), 120 images par seconde ; le contrôle négatif se voit (33,17 ms au lieu de 0,50). Ces chiffres du clavier sont pris session verrouillée, le plein écran quitté : un constat, pas la télé de la soirée. Le premier passage (23 h 33, le binaire d'avant les retouches demandées par clippy, même comportement) est gardé sous `-avant-retouche` : mêmes fautes (17, 18, 17, 17 clics au voile).
- **La souris de M2 avec la fenêtre de Chrome n'est donc pas mesurée** : elle demande une session déverrouillée. Pour la refaire : `target/travail/lot-13b/campagne.sh m2` (le suffixe du nom dit la session), ou `target/hote/lourd --calme caffeinate -d target/release/soiree-banc mesures <ardoise_module.wasm> --navigateurs chrome --entrees 150 --duree 20 --echos 200 --avec-interface --sans-construire --travail <dossier>`, et lire d'abord, au rapport, la ligne de la session et l'absence de faute du voile.

### M6 : la fenêtre de Chrome réduite, relue

`soiree-banc arriere-plan <punchline_module.wasm> --navigateurs chrome --cache 480 --avec-interface --mode reduite --sans-construire` : le banc quitte d'abord le plein écran s'il le faut (`chrome::etapes_vers` ; ici la fenêtre ne l'était pas, `arriere-plan` ne fait pas le geste), demande `minimized` par `Browser.setWindowBounds`, **relit l'état** par `Browser.getWindowBounds` jusqu'à 5 s et échoue s'il n'est pas `minimized` (`chrome::juger_l_etat`) ; de même au retour (`normal`).

| Passage | Charge au départ ; à la fin ; la plus haute pendant | État de la fenêtre, relu | `visibilityState` avant ; au début ; à la fin | Pas joués pendant les 480 s : par la minuterie ; au signal d'une image | Verdict |
|---|---|---|---|---|---|
| `m6-reduite-chrome-verrouillee` (23 h 23 à 23 h 31) | 1,94 ; 1,38 ; 2,73 | `normal → minimized`, puis `minimized → normal` | `visible` ; `visible` ; `visible` | 123 ; 28 423 | réussi : 3 sauts, rattachés au retard injecté ; aucun tour de plus de 10 ms (0,3 ms au plus) ; aucun trou hors du retard injecté |

- **Constat du navigateur** : Chrome 156 dit sa fenêtre réduite (`Browser.getWindowBounds` : `minimized`, relu dans les 5 s), et pourtant **la page reste `visible` et reçoit ses images** (28 423 pas au signal d'une image sur 28 546 ; 120 images par seconde), pendant les 480 s. Une fenêtre réduite de Chrome n'est donc pas, ici, un onglet caché : le pas suit les images, comme au premier plan, et le Worker n'est pas bridé.
- **Mais la session de l'écran était verrouillée**, et un essai hors règle (`target/travail/lot-13b/essai-onglet.txt`, 30 s, charge sous 2) montre que, session verrouillée, même un onglet passé derrière un autre dans la fenêtre de Chrome reste `visible` et reçoit ses images (1 596 pas au signal d'une image, 3 par la minuterie ; `arriere-plan --mode onglet --avec-interface` en échec), alors que Chrome sans interface le cache (lot 10a, `arriere-plan` réussi). Ce que Chrome fait d'une fenêtre réduite **session déverrouillée** n'est donc pas établi : la session verrouillée suffit peut-être à tout garder visible. Au lot 13 (`m6-reduite-chrome`, fini à 21 h 01, session encore ouverte), la même commande, qui ne relisait pas l'état, avait vu la page `visible` aussi ; que la fenêtre ait alors été réduite n'est pas su. Le constat vaut pour Chrome 156, sa fenêtre réduite, session verrouillée ; la soirée se joue session ouverte.
- Pour refaire : `target/travail/lot-13b/campagne.sh m6`, de préférence session déverrouillée (le nom du passage le dit), et `--mode onglet --avec-interface` à côté, pour savoir si Chrome cache quoi que ce soit avec sa fenêtre.

### Correction après la vérification : l'écho de M14 avec un autre onglet (aucune mesure de temps)

Du 2026-11-08 à 23 h 54 au 2026-11-09 à 0 h 03 (UTC), même machine, session de l'écran toujours verrouillée (relevé dans chaque empreinte), écran allumé ; Chrome 156.0.8078.80 et Firefox 158.0.1 **sans interface**. Ce sont des **contrôles**, pas des mesures : `target/hote/lourd`, sans `--calme` ; les latences qu'ils impriment ne sont pas reprises ici.

- **La deuxième cause** (trouvée par la vérification du lot 13b, `target/travail/verif-lot-13b/mesures-sans-interface`, binaire `609caea2…`) : `soiree-banc mesures ardoise --navigateurs chrome,firefox --entrees 60 --duree 8 --echos 50` rendait le code 1 dans Chrome sans interface, « 1 clic(s) de la souris allés au voile ». Les lignes de Chrome : le plein écran entré à 2 337 ms ; à partir de 8,9 s (la fin des échos du Worker, puis ceux du banc, puis l'onglet de l'écho ouvert) le Worker ne joue plus ses pas qu'à la minuterie (`par_image` figé à 547, `par_minuterie` de 5 à 68) : l'onglet de l'écran est caché ; il revient (`ecran-du-commun` à 10 284 ms), puis le plein écran est annoncé quitté à 10 467 ms, le voile « plein-ecran-quitte » posé, et le premier clic de la souris de la première condition (13 050 ms) va au voile. `Target.createTarget` ouvre l'onglet **devant** ; caché, l'onglet de l'écran quitte le plein écran. Le même schéma est dans la preuve de la correction du lot 13 (`lot-13-corr/m2-ardoise-chrome`, quitté à 11 954 ms, geste à 14 573 ms). Firefox ne faisait aucune faute. Cela ne doit rien à la session verrouillée.
- **Ce que le commun corrige** : l'onglet de l'écho s'ouvre derrière celui de l'écran (`Lance::nouvel_onglet_derriere` : `Target.createTarget` avec `background` dans Chrome, `browsingContext.create` avec `background` dans Firefox) ; et le voile sur le jeu **après M14**, une demi-seconde après la fin des échos, est une faute (`mesures::faute_du_voile`, « après M14, avant les conditions »), si le voile n'était pas déjà là après « Jouer ».
- **`lot-13b-corr/mesures-sans-interface`** (binaire final `f667bea7…`, la même commande que la vérification ; charge 3,97 au départ, 3,01 à la fin, 6,11 au plus pendant : un contrôle, pas une mesure) : **code 0**, « 2 mesuré(s), 0 en faute ». Chrome : le plein écran entré à 2 328 ms n'est jamais quitté, aucune ligne `geste` après « Jouer », aucun voile ; 7 traces de la souris avec leur pixel dans chaque condition. Firefox : aucune faute. L'écho avec l'autre onglet se fait toujours (0,01 ms à 0,05 ms près dans Chrome ; −1,45 ms à 0,02 ms près dans Firefox, sous charge, pas une mesure). Le passage d'avant (`-avant-place`, binaire `d45961a8…`, avant que le booléen de `onglet` devienne `navigateur::Place` pour clippy, même comportement ; charge 2,03 ; 1,28 ; 2,18) : code 0 aussi, le plein écran entré à 2 355 ms jamais quitté.
- **Avec la fenêtre** (`target/travail/lot-13b-corr/essai-interface.txt`, binaire `d45961a8…`, hors règle, 20 entrées, 3 s par condition, session verrouillée) : l'onglet ouvert derrière est accepté par Chrome et par Firefox avec leur fenêtre (les trois échos de M14 rendus) ; Chrome fait les fautes du voile dès « Jouer » (la session verrouillée, comme au lot 13b), Firefox aucune. Que le plein écran de la fenêtre de Chrome tienne pendant M14 **session ouverte** n'est pas vu : c'est la mesure qui reste à faire (`target/travail/lot-13b/campagne.sh m2`), et la faute « après M14 » la dira si l'onglet de l'écho le fait quitter encore.
