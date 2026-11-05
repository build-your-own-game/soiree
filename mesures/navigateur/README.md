# Le banc des navigateurs

Les mesures 1, 4 et 6 de « Ce que la phase 3 devra mesurer » (`conception/RESTE-PHASE-2.md`), dans les vrais navigateurs de la machine qui le lance. Les résultats du 2026-11-05 et ce qu'ils mesurent sont dans [`../../conception/MESURES-PHASE-3.md`](../../conception/MESURES-PHASE-3.md).

1. **Le carburant et le temps** : la durée d'une unité de carburant sur les appels lourds, le coût fixe d'un appel et la remise à zéro des zones, appel par appel, comparés au même appel dans wasmtime.
2. **Le chargement d'un module** (mesure 4) : sa taille, livré et réécrit, compressé ou non, et le temps de `WebAssembly.compile`.
3. **L'arrière-plan** (mesure 6) : la cadence de `setTimeout`, de `requestAnimationFrame`, d'un Worker et d'un AudioContext muet quand l'onglet n'est plus au premier plan.

## Le principe : une même trace, rejouée partout

`capture/` (Rust, hors du workspace du commun) charge chaque module exactement comme la partie le charge (`Moteur::charger`, `ReglagesMoteur::PHASE_1` : validation, réécriture, compilation, `Bonjour`, `Decrire`), écrit le module réécrit, puis enregistre une **trace** : chaque appel fait au module, avec ses octets, son budget, le carburant brûlé et une empreinte du retour, et chaque instance neuve, instantané, restauration et instance lâchée, dans l'ordre. La trace contient :

- deux mille `Bonjour` sur une instance nue (le coût fixe d'un appel) ;
- chaque pire cas (`PireCas`), puis sept fois sa vue de l'écran, son état et ses invariants (les appels les plus lourds que le jeu sache construire) ;
- une soirée du plan court (graine `5e…5e`), rejouée entrée par entrée par E9 et E14 sur les instances enregistrées, comme le fait `epreuve/examples/etalonner.rs`.

La capture rejoue ensuite la trace deux fois dans wasmtime, chaque appel chronométré autour de `Instance::appeler`. La page du banc (`page/banc.js`) rejoue **la même trace, dans le même ordre**, dans chaque navigateur, avec le protocole d'appel du moteur recopié (`page/banc-coeur.js`, `InstanceJs` : budget, profondeur, drapeaux, remise à zéro des quatre zones, `soiree_boite`, copie, `soiree_appel`, copie du retour). Chaque appel s'y compare donc à lui-même dans wasmtime, et le carburant et le retour de chaque appel sont vérifiés contre ceux de wasmtime.

Le serveur (`serveur.mjs`) pose COOP et COEP : la page est isolée (`crossOriginIsolated`), ce qui donne aux navigateurs leur horloge la plus fine. Granularité mesurée par la page (`granularite()`) : 5 µs dans Chrome, 20 µs dans Firefox. Sans isolement, les navigateurs la disent plus grossière ; le banc ne l'a pas mesuré.

## Prérequis

- La chaîne Rust du commun (`rust-toolchain.toml`), avec la cible `wasm32-unknown-unknown`.
- node 22 ou plus (`fetch` et `WebSocket` intégrés). **Aucun paquet npm** : rien à installer.
- Google Chrome et Firefox dans `/Applications`. Chrome est piloté par le protocole DevTools, Firefox par WebDriver BiDi, tous deux sans interface.
- Safari, piloté par `safaridriver` (WebDriver ; Safari n'a pas de mode sans interface : une fenêtre s'ouvre). L'hôte doit d'abord cocher « Autoriser l'automatisation à distance » (Safari, Réglages, Développeur), et peut-être lancer une fois `safaridriver --enable` (mot de passe administrateur). Sans cela, le banc écrit « non mesuré » avec le message de `safaridriver`, et ne mesure pas Safari.
- Facultatif, pour comparaison seulement : deno et bun (`hors-navigateur.mjs`).

## Relancer

Chaque commande tient sous dix minutes. Une seule commande lourde à la fois ; avant chacune, `memory_pressure | tail -1` (sous 20 % de mémoire libre, attendre). **Les lancer au premier plan** : un processus lancé en arrière-plan par un agent a été vu avec une priorité abaissée (état `SN` de `ps`), ce qui peut changer le réveil des minuteries (SAL-132) et fausser la mesure 6.

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
```

Puis depuis `soiree/mesures/navigateur/` :

```sh
# 4. Mesures 1 et 4 : un navigateur neuf par module (environ 15 s par navigateur).
node mesurer.mjs --navigateurs chrome,firefox,safari --tour 1
node mesurer.mjs --navigateurs chrome,firefox --tour 2        # d'autres tours : ce qui bouge
# Facultatif : Chrome sans compilation paresseuse (hors défaut), et hors navigateur.
node mesurer.mjs --navigateurs chrome --etiquette chrome-sans-paresse --drapeaux-chrome "--js-flags=--no-wasm-lazy-compilation"
node hors-navigateur.mjs node
deno run --allow-read --allow-write --allow-env --allow-sys hors-navigateur.mjs deno
bun hors-navigateur.mjs bun

# 5. Mesure 6 : environ 8 min par navigateur (30 s devant, 420 s derrière, 15 s devant).
node cadence.mjs --navigateur chrome
node cadence.mjs --navigateur firefox
node cadence.mjs --navigateur safari

# 6. Les tableaux.
node analyser.mjs --tour 1
node analyser-cadence.mjs
```

Tout ce qui est produit va dans `soiree/target/mesures-navigateur/` (hors de tout dépôt) : `<module>/` (module réécrit, `meta.json`, `trace.bin`, `wasmtime.json`), `resultats/<navigateur>/<module>-<tour>.json`, `cadence/<navigateur>-<variante>.json`, `analyse.json`. Les profils temporaires des navigateurs y sont créés puis effacés.

Pour mesurer **l'ordinateur de la soirée**, lancer les mêmes commandes sur lui, avec ses navigateurs : c'est la mesure qui remplacerait `MARGE_MACHINE` (`crates/epreuve/src/budgets.rs`).

## Les fichiers

| Fichier | Rôle |
|---|---|
| `capture/src/main.rs` | réécrit, enregistre la trace, rejoue dans wasmtime ; le format de la trace est documenté sur `ecrire_trace` |
| `page/trace.js` | lit la trace (navigateur, node, deno, bun) |
| `page/banc-coeur.js` | le protocole d'appel du moteur, le rejeu chronométré, la compilation, la remise à zéro, les blocs de `Bonjour` |
| `page/banc.html`, `page/banc.js` | la page du banc : charge, lance, renvoie au serveur |
| `page/cadence.html`, `page/cadence.js`, `page/cadence-worker.js` | la page de la mesure 6 et son Worker |
| `serveur.mjs` | sert les pages et les données (COOP, COEP), reçoit les résultats |
| `navigateurs.mjs` | lance Chrome, Firefox et Safari, sans interface quand ils le permettent, et les pilote |
| `mesurer.mjs` | mesures 1 et 4, un navigateur neuf par module |
| `cadence.mjs` | mesure 6 |
| `hors-navigateur.mjs` | le même banc dans node, deno ou bun, pour comparaison seulement |
| `analyser.mjs`, `analyser-cadence.mjs` | les tableaux de `MESURES-PHASE-3.md` |

## Ce que le banc ne sait pas faire

- **Les téléphones** : aucun téléphone n'est relié à cette machine. Le simulateur iOS fait tourner le processeur du Mac : il ne dirait rien du temps de compilation sur un téléphone.
- **Un vrai navigateur avec interface en arrière-plan** : la mesure 6 cache un onglet derrière un autre, dans un navigateur sans interface (la page se voit bien `hidden`). Une fenêtre recouverte ou réduite, l'App Nap de macOS sur un navigateur avec interface (SAL-131, SAL-132), la mise en veille de l'écran, les modes d'économie de Chrome, n'y sont pas.
- **Séparer une pause du navigateur d'un appel** : une pause du ramasse-miettes ou du fil de la page tombe sur l'appel en cours. Les maxima par appel la comptent ; le banc réutilise les tampons de ses instantanés pour ne pas en provoquer lui-même.
