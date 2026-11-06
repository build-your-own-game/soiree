# Le banc des navigateurs

Les mesures 1, 4 et 6 de « Ce que la phase 3 devra mesurer » (`conception/RESTE-PHASE-2.md`), dans les vrais navigateurs de la machine qui le lance. Les résultats et ce qu'ils mesurent sont dans [`../../conception/MESURES-PHASE-3.md`](../../conception/MESURES-PHASE-3.md).

1. **Le carburant et le temps** (mesure 1, M1) : la durée d'une unité de carburant sur les appels lourds, le coût fixe d'un appel, la remise à zéro des zones, l'instantané et la restauration, appel par appel, comparés au même appel dans wasmtime.
2. **Le chargement d'un module** (mesure 4) : sa taille, livré et réécrit, et le temps de la réécriture et de la compilation.
3. **L'arrière-plan** (mesure 6) : la cadence de `setTimeout`, de `requestAnimationFrame`, d'un Worker et d'un AudioContext muet quand l'onglet n'est plus au premier plan.

**Depuis le lot 3c de la phase 3, les mesures 1 et 4 se font par le banc du commun** (`soiree-banc etalonner`, `crates/banc/src/etalonner.rs`), sur le protocole d'appel de `moteur-commun`, et non plus par une page qui le recopiait en JavaScript (`page/banc-coeur.js`, `page/banc.js`, `page/trace.js`, `mesurer.mjs`, `analyser.mjs` et `hors-navigateur.mjs`, retirés). Ce dossier garde la capture, qui est le côté wasmtime de la mesure 1, et la mesure 6 (`cadence.mjs`), que le lot 13 reprendra avec interface.

## Le principe : une même trace, rejouée partout

`capture/` (Rust, hors du workspace du commun) charge chaque module exactement comme la partie le charge (`Moteur::charger`, `ReglagesMoteur::PHASE_3` : validation, réécriture, compilation, `Bonjour`, `Decrire`), écrit le module livré et le module réécrit, puis enregistre une **trace** (`moteur_commun::trace`, une seule définition du format) : chaque appel fait au module, avec ses octets, son budget, le carburant brûlé et une empreinte du retour, et chaque instance neuve, instantané, restauration et instance lâchée, dans l'ordre. La trace contient :

- deux mille `Bonjour` sur une instance nue (le coût fixe d'un appel) ;
- chaque pire cas (`PireCas`), puis sept fois sa vue de l'écran, son état et ses invariants (les appels les plus lourds que le jeu sache construire) ;
- une soirée du plan court (graine `5e…5e`), rejouée entrée par entrée par E9 et E14 sur les instances enregistrées, comme le fait `epreuve/examples/etalonner.rs`.

La capture rejoue ensuite la trace deux fois dans wasmtime, chaque appel chronométré autour de `Instance::appeler`. `soiree-banc etalonner` rejoue **la même trace, dans le même ordre**, dans chaque navigateur, dans un Worker comme l'autorité, sur `moteur_navigateur::charger` et le protocole du commun (l'exemple `etalonnage-navigateur` de `web/moteur-navigateur`). Chaque appel s'y compare donc à lui-même dans wasmtime, et le carburant et le retour de chaque appel sont vérifiés contre ceux de wasmtime.

Le serveur du banc pose COOP et COEP : la page est isolée (`crossOriginIsolated`), ce qui donne aux navigateurs leur horloge la plus fine (5 µs dans Chrome, 20 µs dans Firefox, mesuré).

## Prérequis

- La chaîne Rust du commun (`rust-toolchain.toml`), avec la cible `wasm32-unknown-unknown`, et la CLI `wasm-bindgen` 0.2.129.
- Google Chrome et Firefox dans `/Applications`. Chrome est piloté par le protocole DevTools, Firefox par WebDriver BiDi, tous deux sans interface.
- Safari, piloté par `safaridriver` (WebDriver ; Safari n'a pas de mode sans interface : une fenêtre s'ouvre). L'hôte doit d'abord cocher « Autoriser l'automatisation à distance » (Safari, Réglages, Développeur), et peut-être lancer une fois `safaridriver --enable` (mot de passe administrateur). Sans cela, le banc écrit « pas exécuté » avec le message de `safaridriver`, et ne mesure pas Safari. Le banc ne touche jamais à ce réglage.
- Pour la mesure 6 seulement : node 22 ou plus, sans paquet npm.

## Relancer

Une seule commande lourde à la fois ; avant chacune, `memory_pressure | tail -1` (sous 20 % de mémoire libre, attendre). Jamais de navigateur pendant une compilation.

Depuis `soiree/` :

```sh
# 1. Les modules, au profil module (comme l'étape 1 de l'épreuve).
CARGO_BUILD_JOBS=4 cargo build -p devine-module -p ardoise-module --target wasm32-unknown-unknown --profile module
(cd ../games/punchline && CARGO_BUILD_JOBS=4 cargo build -p punchline-module --target wasm32-unknown-unknown --profile module)
(cd ../games/sidequest && CARGO_BUILD_JOBS=4 cargo build -p sidequest-module --target wasm32-unknown-unknown --profile module)
(cd ../games/tier-list && CARGO_BUILD_JOBS=4 cargo build -p tier-list-module --target wasm32-unknown-unknown --profile module)

# 2. La capture (quelques secondes de compilation si le commun est déjà compilé).
(cd mesures/navigateur/capture && CARGO_BUILD_JOBS=4 CARGO_TARGET_DIR=../../../target cargo build --release)

# 3. Réécrire, enregistrer, rejouer dans wasmtime (environ 40 s pour les cinq).
target/release/capture-navigateur target/mesures-navigateur \
  devine=target/wasm32-unknown-unknown/module/devine_module.wasm \
  ardoise=target/wasm32-unknown-unknown/module/ardoise_module.wasm \
  punchline=../games/punchline/target/wasm32-unknown-unknown/module/punchline_module.wasm \
  sidequest=../games/sidequest/target/wasm32-unknown-unknown/module/sidequest_module.wasm \
  tier-list=../games/tier-list/target/wasm32-unknown-unknown/module/tier_list_module.wasm

# 4. Le banc (construit d'abord, puis aucun navigateur pendant une compilation).
CARGO_BUILD_JOBS=4 cargo build --release -p banc

# 5. M1 (et la mesure 4), M10 et M9 : un navigateur neuf par module. Les durées d'un
#    seul tour ne se retrouvent pas d'un lancement à l'autre : M1 sur quatre tours,
#    comme le harnais. Code 1 dès un écart de carburant avec wasmtime, ou une récursion
#    de M9 sous PHASE_3 qui n'est pas la faute Profondeur ; 2 si tout n'a pas été mesuré.
target/release/soiree-banc etalonner --tours 4 --navigateurs chrome,firefox,safari
# Le contrôle négatif de M1 : un moteur qui fausse le carburant passé 100 appels ; il
# doit rendre le code 1, le premier écart à l'appel 101 (2, panne de l'outil, s'il ne
# le voit pas là).
target/release/soiree-banc etalonner --mesures m1 --modules devine --fautif fausser-apres:100 --sans-construire
# Une seule mesure, ou sous charge (lancer d'abord quatorze `yes > /dev/null &`) :
target/release/soiree-banc etalonner --mesures m10 --etiquette chrome-sous-charge --navigateurs chrome --sans-construire
```

La mesure 6, depuis `soiree/mesures/navigateur/` (environ 8 min par navigateur : 30 s devant, 420 s derrière, 15 s devant ; **au premier plan** : un processus lancé en arrière-plan par un agent a été vu avec une priorité abaissée, état `SN` de `ps`, ce qui peut changer le réveil des minuteries, SAL-132) :

```sh
node cadence.mjs --navigateur chrome
node cadence.mjs --navigateur firefox
node cadence.mjs --navigateur safari
node analyser-cadence.mjs
```

Tout ce qui est produit va dans `soiree/target/mesures-navigateur/` (hors de tout dépôt) : `<module>/` (module livré et réécrit, `meta.json`, `trace.bin`, `wasmtime.json`), `etalonnage/<navigateur>/` (ce que chaque page a rapporté, avec la charge avant et après), `etalonnage/analyse.json`, `cadence/<navigateur>-<variante>.json`. Les profils temporaires des navigateurs sont créés sous `soiree/target/banc/profils/` puis effacés.

Pour mesurer **l'ordinateur de la soirée**, lancer les mêmes commandes sur lui, avec ses navigateurs : c'est la mesure qui remplacerait `MARGE_MACHINE` (`crates/epreuve/src/budgets.rs`).

## Les fichiers

| Fichier | Rôle |
|---|---|
| `capture/src/main.rs` | réécrit, enregistre la trace (`moteur_commun::trace`), rejoue dans wasmtime |
| `page/cadence.html`, `page/cadence.js`, `page/cadence-worker.js` | la page de la mesure 6 et son Worker |
| `serveur.mjs` | sert la page de la mesure 6 (COOP, COEP), reçoit ses résultats |
| `navigateurs.mjs` | lance Chrome, Firefox et Safari pour la mesure 6 |
| `cadence.mjs`, `analyser-cadence.mjs` | la mesure 6, et son tableau |

Le reste est dans le commun : `crates/banc/src/etalonner.rs` (le scénario et son analyse), `crates/banc/pages/etalonner.html` et `etalonner-worker.js` (la page et son Worker), `web/moteur-navigateur/examples/etalonnage_navigateur.rs` (ce qui tourne dans le navigateur).

## Ce que le banc ne sait pas faire

- **Les téléphones** : aucun téléphone n'est relié à cette machine. Le simulateur iOS fait tourner le processeur du Mac : il ne dirait rien du temps de compilation sur un téléphone.
- **Un vrai navigateur avec interface en arrière-plan** : la mesure 6 cache un onglet derrière un autre, dans un navigateur sans interface. Une fenêtre recouverte ou réduite, l'App Nap de macOS sur un navigateur avec interface (SAL-131, SAL-132), la mise en veille de l'écran, les modes d'économie de Chrome, n'y sont pas.
- **Séparer une pause du navigateur d'un appel** : une pause du ramasse-miettes ou du fil tombe sur l'appel en cours. Les maxima par appel la comptent.
- **Hors navigateur** : node, deno et bun ne se mesurent plus (`hors-navigateur.mjs` rejouait la trace avec le protocole recopié en JavaScript, retiré). Leurs chiffres du 2026-11-05 restent dans `MESURES-PHASE-3.md`, pour comparaison seulement.
