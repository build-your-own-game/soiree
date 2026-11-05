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

## Ce que ces mesures ne disent pas

- **L'ordinateur de la soirée.** Toutes les durées sont celles d'un M5 récent. Le CONTRAT ne décrit pas encore l'ordinateur de la soirée (« Le matériel : à renseigner ») ; le banc se relance sur lui tel quel (README).
- **Safari et les téléphones** : non mesurés (voir plus haut).
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

**Ma proposition** : décider d'abord **quels navigateurs l'ordinateur de la soirée doit tenir** (une question du CONTRAT, « Le matériel »). Si c'est Chrome, passer à 150 ps × `MARGE_MACHINE` 2 : les budgets de E19 grandissent de 2,67, ce qui rendrait à Tier list de la place pour ses tuiles (`TUILES_MAX`, revue de la phase 2, constat 10). Si Firefox doit tenir aussi, garder les budgets d'aujourd'hui (800 ps) : ils couvrent le maximum mesuré dans Firefox (678 ps), mais ne laissent alors qu'un facteur 1,2 pour une machine plus lente que ce M5 ; ou passer à 1 400 ps si cette marge paraît trop mince. Dans tous les cas, mesurer Safari (une case à cocher) avant de trancher, et relancer le banc sur l'ordinateur de la soirée pour remplacer `MARGE_MACHINE`.
