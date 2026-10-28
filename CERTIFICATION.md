# Certification : ce que chaque jeu passe avant la soirée

> Pour l'agent de code qui écrit ou modifie un jeu. Comme `MANIFESTE.md`, ce fichier vaut pour tous les jeux du dépôt.

## Pourquoi une certification

Avant de sortir un jeu, Microsoft, Sony et Nintendo le font passer par une liste d'environ 200 exigences : XR chez Microsoft, TRC chez Sony, Lotcheck chez Nintendo. Il faut toutes les passer. Ce n'est pas une chasse aux bugs, c'est un **plancher de qualité commun**. Il porte sur ce que la plateforme impose à tous :
- une manette qui se déconnecte ;
- une console qui sort de veille ;
- un réseau lent ;
- un pseudo bizarre ;
- un texte coupé par le bord de la TV.

Notre plateforme commune, c'est la soirée : une TV, des téléphones sur un Wi-Fi de salon, des amis qui ne lisent pas les consignes. Cette liste en est le plancher. Le manifeste dit comment construire un jeu ; ce fichier dit ce qu'il doit **tenir**.

Deux idées viennent des navigateurs. La suite commune web-platform-tests, que Chrome, Firefox et Safari passent tous, a inspiré une liste unique pour tous les jeux et un même harnais pour tous. ClusterFuzz, le fuzzing qui tourne en continu, a inspiré les entrées au hasard qui cherchent la panne.

## Trois étages

| Étage | Quand | Périmètre | Qui vérifie | Commande |
|---|---|---|---|---|
| **1. Propriétés** | pendant le développement | logique du jeu sans écran | Cargo/libtest et les oracles écrits pour le jeu | `soiree check` ; `PROPTEST_CASES=5000 soiree check` pour élargir les campagnes qui utilisent ce paramètre |
| **2. Certification** | pour éprouver le jeu réel | scénarios et mesures demandés | le harnais, en boîte noire | `soiree certify`, dans le dossier du jeu |
| **3. Le salon** | avant la soirée | appareils, perception et plaisir de jeu | un humain, avec de vrais téléphones | la liste « Étage 3 » plus bas |

- **Étage 1.** Il joue des milliers de suites d'entrées au hasard sur la simulation (`crates/core`) et vérifie des invariants après chaque pas. Un échec est réduit (*shrinking*) au plus petit exemple qui casse encore. Un agent peut corriger ça.
- **Étage 2.** Il lance le vrai serveur et ouvre la TV et 8 téléphones sans écran, pilotés par Playwright : la TV dans le Chrome de l'ordinateur, quatre téléphones dans WebKit et quatre dans Chromium, en alternance. Chaque téléphone passe par son propre proxy [Toxiproxy](https://github.com/Shopify/toxiproxy), qui sait retarder, couper ou avaler la connexion sans rien dire. Toxiproxy ne porte que du TCP : la TV et les téléphones ouvrent leur page avec `?salon=ws`, qui garde salon sur son WebSocket. Le harnais tape au hasard, coupe, gèle, recharge, redémarre l'hôte, et envoie des paquets hostiles. Il ne connaît du jeu que son fichier `cert.json`, décrit plus bas.
- **Étage 3.** Ce qu'aucun simulateur ne reproduit : la suspension d'iOS, un vrai Wi-Fi, le recul du canapé, savoir si c'est drôle.

## Le contrat et les observations

Depuis 0.3.0, les engagements propres au jeu vivent dans `verification.toml`.
`soiree contracts` les combine avec les contrôles communs pour produire le plan ;
`soiree contracts --write-view` actualise leur vue dans le `CERTIFICATION.md` du
jeu. Ce document commun reste la description du socle, pas un rapport de passage.
Le format, la migration et les limites sont dans
[`VERIFICATION.md`](VERIFICATION.md), aussi distribué dans `.soiree/`.

Les méthodes et les résultats restent séparés : un test, une mesure, une preuve
et une justification ne répondent pas aux mêmes questions. Un contrôle absent,
ignoré, partiel ou périmé ne devient pas réussi. Une panne du contrôleur se
distingue d’un bug du jeu. Le code de sortie porte sur les contrôles demandés :
`check --fast` et `certify -- --only …` peuvent réussir pour leur périmètre tout
en laissant le reste en attente. Les passages distincts ne sont pas fusionnés
automatiquement en une certification globale.

La couverture reste à examiner. La liste textuelle ci-dessous, les obligations
résolues par l’outil et les propriétés propres au jeu ne sont pas trois listes
automatiquement équivalentes. Un rapport décrit ce que son plan et ses oracles
ont effectivement vérifié ; il ne démontre pas l’exhaustivité des déclarations.
Les 97 items du catalogue sont des enseignements à examiner : ceux sans décision
explicite restent non revus, sans être déclarés tous applicables ou tous écartés.
Les jugements sur appareils réels gardent leur statut de revue humaine.

## La règle du bug trouvé en jouant

Un bug découvert en jouant est un échec de la certification avant d'être un échec du code. C'est l'esprit des postmortems sans blâme de Google et des *Correction of Errors* d'Amazon. Pour chacun :

1. **Le reproduire.** Un replay, ou une suite d'entrées, qui devient un test de l'étage 1.
2. **Répondre par écrit** dans `LEARNINGS.md` : *pourquoi aucun item de cette liste ne l'a vu ?*
3. **Renforcer la liste.** Ajoute un item, ou durcis un item existant, pour que **tous** les jeux en profitent.

Un test n'est jamais supprimé sans dire ce qui le couvre maintenant : chaque jeu en tient le registre dans la section « Tests retirés au passage à salon » de son `LEARNINGS.md`.

## Lancer le harnais

Le harnais est dans `tools/cert/` du dépôt commun, et l'outil `soiree` l'embarque à sa version. Une fois, sur l'ordinateur, `soiree doctor` dit ce qui manque (Node, Chrome, le WebKit exact de la version de Playwright, [Toxiproxy](https://github.com/Shopify/toxiproxy)). Au premier passage, `soiree certify` écrit le harnais dans un cache par version et y lance `npm ci`. Dans ce dossier de cache indiqué par `soiree doctor`, `npx playwright-core install webkit` installe le WebKit requis. Une autre révision déjà installée ne suffit pas. Ce WebKit attrape les écarts de CSS et d’API, mais ne reproduit pas la suspension d’iOS : l’étage 3 reste nécessaire.

Puis, dans le dossier du jeu (les options après `--` vont au harnais) :

```sh
soiree certify                          # scénarios ordinaires, sans endurance R2
soiree certify -- --only cut,freeze     # quelques scénarios
soiree certify -- --seed 12             # rejoue le hasard d'un passage
soiree certify -- --soak 30             # 30 min d'endurance (R2)
```

Les scénarios, dans l'ordre : `join`, `rtc`, `performance`, `panic`, `layout`, `cut`, `freeze`, `silent`, `reload`, `twin`, `hostile`, `jitter`, `monkey`, `restart`, `storage`. La TV lance la partie (`start`) après les scénarios de salon : `join`, `rtc`, `performance` et `panic`. Les sondes de performance décrivent donc des actions possibles avant le lancement. Seul `rtc` ouvre un téléphone sans `?salon=ws` : l'hôte tourne avec `SALON_LOG=1`, et sa sortie dit si WebRTC l'a porté ; si la salle est pleine, seul le transport est jugé, jamais le refus du joueur en trop. Un écran n'est « reconnecté » que s'il reçoit de l'hôte un `Welcome` ou un paquet du jeu : un refus seul ne compte pas (C7). Un téléphone au stockage refusé ou corrompu arrive en appareil neuf, et le jeu garde un moment la place qu'il avait : `storage` ne le fait donc entrer en appareil neuf que s'il n'avait pas de place (après `restart`, une partie neuve), sinon il lui rend son stockage et sa place, pour ne jamais asseoir N + 1 joueurs sur N places ; à la fin, chaque téléphone doit avoir retrouvé une place. `hostile` ne mesure que sur une connexion encore ouverte au moment d'envoyer (E2 : jamais une fermeture antérieure). Le rapport s'affiche à la fin, avec une ligne par item qui garde **toutes** ses notes, les ✗ d'abord puis les ~, les ✓ et les – (une note répétée, par l'endurance par exemple, n'y est qu'une fois, avec son nombre), et s'enregistre dans `rapport.md` à côté des captures. Le harnais conserve ses observations, mais l’évaluateur commun décide si le périmètre demandé est satisfait. `soiree certify` écrit `certification/verification.json` et `certification/verification.md` : plan, sélection, observations, hypothèses et provenance des entrées. Le détail brut reste dans `certification/harness-observations.json` et, lorsqu’il est produit, `certification/harness-observations.md`. Ces rapports datés accompagnent le code ; aucun ancien rapport n’est repris comme une observation fraîche. Le harnais écrit aussi `performance.json` à côté de ses captures : intervalles entre images, débits, temps pour rejoindre depuis l’ouverture et latences entrée → rendu, avec leurs budgets et leur moteur de navigateur. Les nombres figurent aussi dans le rapport Markdown.

Si le harnais doit fermer de force un contexte après une commande native bloquée, le passage s’arrête avec **H1**, une panne de l’infrastructure de test. Les observations déjà terminées restent dans le rapport ; les scénarios restants sont explicitement non vérifiés. Un contexte détruit ne doit jamais produire ensuite de faux verdicts sur la reconnexion ou le stockage. Le diagnostic conserve l’opération native, son historique récent et l’état observable de la page.

Le harnais garde trois oracles pendant tous les scénarios :
- aucune exception dans aucune page ;
- aucune panique dans l'hôte, qui tourne toujours ;
- la TV dessine toujours.

## Le fichier `cert.json`

Chaque jeu décrit en quelques lignes comment le harnais le lance et le rejoint. C'est le seul endroit où le harnais apprend quelque chose du jeu. Le plus petit est celui de la démo de salon (`salon/cert.json`) : on y rejoint sans aucune étape.

```json
{
  "launch": ["cargo", "run", "--release", "-p", "punch-host"],
  "relaunch": ["cargo", "run", "--release", "-p", "punch-host", "--", "--no-build"],
  "env": { "PUNCH_PORT": "{port}" },
  "ready": "Punchline ready",
  "tv": "/",
  "phone": "/play",
  "socket": "/salon/ws",
  "players": { "min": 3, "max": 8 },
  "join": [
    { "fill": "input", "text": "{name}" },
    { "press": "Enter" }
  ],
  "joined": "document.querySelector('input') === null",
  "advance": { "key": " " }
}
```

- **`launch`**, **`relaunch`** et **`env`** décrivent le lancement. `{port}` y est remplacé par le port choisi par le harnais, et `{tmp}` par un dossier temporaire (pour les résultats qu'un jeu enregistre). `relaunch` sert au redémarrage en pleine partie, sans recompiler.
- **`ready`** est la ligne de la sortie de l'hôte qui dit qu'il écoute. **`capture`** lit une valeur dans cette sortie, comme un code de salle : `{ "room": "Room ([0-9A-F]+)" }` remplace ensuite `{room}` dans les chemins.
- **`tv`** et **`phone`** sont les chemins des pages ; le harnais y ajoute `salon=ws`, à la suite de la requête s'il y en a une (`/play?room=X&salon=ws`). **`socket`** est le chemin du WebSocket de salon, `/salon/ws` : le harnais y envoie des paquets binaires de salon (postcard), vrais, mutés ou inventés.
- **`join`** liste les étapes pour rejoindre depuis un téléphone : `fill` (un champ et son texte, `{name}` étant le prénom), `click` (un sélecteur), `press` (une touche). **`joined`** est une expression évaluée dans la page téléphone, vraie une fois le joueur dans la partie.
- **`start`** lance la partie depuis la TV, et **`advance`** la fait avancer (passer une étape), pour la parcourir vite. Chacun est `{ "key": "…" }` ou `{ "text": "Play" }` (le bouton qui porte ce texte).
- **`players.max`** est la taille de la salle (R3).
- **`layout: false`**, avec **`layout_reason`**, saute la vérification de la zone sûre : une TV dessinée sur un canvas, une page qui n'est pas une TV.
- Un jeu sans téléphone (`"phone": null`) passe seulement les items qui le concernent : le singe joue alors sur sa page.
- **`ui`** décrit les contrôles d’interface U1 ci-dessous. Chaque catégorie
  `editing`, `lifecycle`, `async`, `effects` est couverte par un scénario ou
  une commande de test, ou porte une raison précise dans `not_applicable`.
  Une absence de test n’est pas une non-applicabilité.
- **`performance.probes`** décrit au moins une action et son effet visible causal sur la TV. Chaque sonde donne **`screen`** (`"phone"` ou `"tv"`), **`phone`** (numéro à partir de 1, par défaut 1), **`action`** (`{ "key": "…" }`, `{ "click": "sélecteur" }`, `{ "fill": "sélecteur", "text": "…" }` ou `{ "tap": [x, y] }`) et **`rendered`**, une expression JavaScript qui lit la valeur affichée concernée. **`prepare`** peut contenir des actions de préparation, exécutées avant le chronomètre. Un changement de cette valeur est observé entre deux occasions de rendu après l’entrée réelle. Les horloges des pages sont alignées par `performance.timeOrigin + performance.now()` ; on ne soustrait jamais les `performance.now()` de deux pages.
- **`painted`** peut donner une expression qui lit un compteur de présentations réussies, pour l’oracle de l’écran vivant. Le canvas de Sidequest utilise les événements post-rendu capturés par le harnais (`window.__certPaints`) ; un simple `requestAnimationFrame` ne prouve pas que son GPU dessine. Les pages DOM gardent le compteur de disponibilité de la boucle navigateur.
- **`performance.setup`** peut contenir des étapes `{ "screen": "tv", "action": { "key": "Enter" } }` (ou `screen: "phone"`, avec `phone`) pour afficher la scène avant la mesure. Cette préparation est terminée avant le prélèvement des images et le chronomètre des sondes.
- Pour **`rendered`**, vise une projection précise de l’effet de l’action (texte d’un élément, position du joueur concerné), jamais une horloge, un compteur d’images ou toute mutation DOM quelconque. Un canvas peut publier cette projection après son dessin ; un état mis à jour seulement dans la simulation ne prouve pas un rendu. Une action sans effet rend la sonde rouge au lieu de produire une latence inventée.
- Les budgets par défaut sont **33 ms** pour l’image la plus lente, **60 images/s**, **80 ms** du téléphone au rendu et **50 ms** du clavier/souris au rendu. `performance.max_frame_ms`, `performance.min_fps` et `performance.probes[].budget_ms` permettent les ajustements de genre prévus par le manifeste ; leur raison et leurs valeurs se documentent dans « Écarts au manifeste ». Le harnais prélève 3 s d’images après le chargement, puis un second échantillon pendant `monkey`, avec les téléphones qui envoient des gestes réels en jeu ; ce second échantillon précède les captures et ne comprend ni gel injecté ni redimensionnement. Le rapport distingue `frames_lobby` et `frames_gameplay`, et indique le nombre de gestes pendant la fenêtre en jeu. Il mesure la cadence `requestAnimationFrame`, pas le temps GPU ni l’arrivée physique d’un photon sur la TV.

Effectue les mesures sur secteur, sans autre certification ou compilation en parallèle.
Sous 20 % sur batterie, l’économiseur de Chrome peut limiter tous les rendus à
30 Hz, y compris une page vide ; ce passage ne mesure alors pas les performances
nominales du jeu. Le budget reste inchangé et la certification doit être relancée
sur secteur. [Implémentation Chromium](https://raw.githubusercontent.com/chromium/chromium/main/chrome/browser/performance_manager/user_tuning/battery_saver_mode_manager.cc).

Exemple de sonde pour une action qui modifie une valeur affichée :

```json
"performance": {
  "probes": [{
    "screen": "tv",
    "action": { "click": "#increment" },
    "rendered": "document.querySelector('#counter').textContent"
  }]
}
```

Le singe ne génère Retour arrière sur un téléphone que lorsqu’un vrai champ texte modifiable a le focus, comme le clavier à l’écran ; sur WebKit cette touche hors champ commande l’historique du navigateur. Le clavier d’un jeu de bureau conserve cette touche.

### Contrôles d’interface U1

`soiree certify -- --only ui` exécute U1 avant le début de partie. Les
scénarios déclaratifs tournent dans WebKit et Chromium ; leurs assertions
observent le jeu en boîte noire. Exemple :

```json
"ui": {
  "scenarios": [{
    "name": "Un message reçu conserve le champ en cours d’édition",
    "screen": "phone",
    "covers": ["editing"],
    "steps": [
      {"wait": "document.querySelector('#name') !== null"},
      {"focus": "#name"},
      {"remember": "field", "read": "document.querySelector('#name')"},
      {"fill": "#name", "text": "é🙂"},
      {"select": "#name", "start": 1, "end": 3},
      {"composition": "#name", "phase": "start"},
      {"composing_key": "#name", "key": "Enter"},
      {"unchanged": "field", "read": "document.querySelector('#name')"},
      {"composition": "#name", "phase": "end", "text": "é🙂"}
    ]
  }],
  "checks": [{
    "name": "Fins d’import dans l’ordre inverse",
    "covers": ["async", "lifecycle", "effects"],
    "command": ["node", "tools/ui-check.mjs", "{url}"]
  }],
  "not_applicable": {}
}
```

`screen` vaut `tv` ou `phone` ; `phone` choisit le numéro dans chaque moteur.
Un `url` ouvre une page indépendante. `focus`, `select`, `composition`
(`start`, `update`, `end`), `composing_key`, `dispatch` (`click`, `input`,
`change`, `blur`), `remember`/`read`, `unchanged`/`read`, `assert` et `wait`
complètent les gestes ordinaires. Les valeurs mémorisées restent dans la
page : on peut comparer l’identité d’un vrai nœud DOM, pas seulement son
texte. Les assertions lisent des observations indépendantes ; elles ne
doivent pas remplacer la logique du jeu par un faux état de test.

Une commande de `checks` tourne depuis le dépôt du jeu, avec une durée
bornée. `{url}` est l’URL de la TV lancée. `SOIREE_CERT_BROWSER_MODULE`
contient l’URL du module navigateur du harnais installé, importable avec
`await import(process.env.SOIREE_CERT_BROWSER_MODULE)` ; le jeu n’a pas à
chercher un dépôt voisin ni à installer une autre version de Playwright.
`SOIREE_CERT_TMP` fournit le dossier de captures. Une erreur, un signal ou
une expiration fait échouer U1 même si la commande a imprimé un succès.
`success` peut exiger une expression régulière dans la sortie combinée,
par exemple `"success":"test result: ok\\. 1 passed"` pour un test Rust
nommé : un filtre qui ne sélectionne plus aucun test ne donne pas de vert.

Une catégorie absente du jeu se justifie par exemple avec
`"not_applicable":{"editing":"aucun champ éditable"}`. Pour les catégories
présentes, tester le nettoyage à la destruction, les réponses périmées
(succès et erreur), l’identité et la sélection pendant l’édition, la
composition et l’absence d’effets ponctuels dupliqués lors d’un réaffichage.
Les événements de composition synthétiques éprouvent les handlers ; les
vrais claviers IME et iPhone restent vérifiés au salon.

Les étapes ordinaires (`join`, coupure, silence, rechargement, stockage et onglets concurrents) traversent les deux moteurs. Le gel complet de JavaScript (`freeze`) utilise explicitement un téléphone Chromium, car Playwright n’expose les sessions CDP que pour Chromium. La trajectoire tactile détaillée du singe utilise CDP dans Chromium ; dans WebKit, les taps restent tactiles et les glissements utilisent un seul drag natif à la souris, faute d’API publique de déplacement tactile. Aucun tap parasite ne précède ce drag. Ce dernier cas exerce les contrôles de pointeur et ne prétend pas simuler le toucher d’iOS. Les scripts de stockage sont installés et retirés par l’API Playwright commune aux deux moteurs.

WebKit peut différer l’accusé de réception d’un survol jusqu’à une occasion
de rendu. Le début d’un drag envoie donc le mouvement initial puis l’appui
dans l’ordre du transport, avant d’attendre **les deux** accusés ; l’appui
libère le survol différé. Un test natif retient une feuille CSS : l’ancien
enchaînement attend son chargement, tandis que le drag complet doit finir
avant de la libérer, avec ses événements de pointeur et souris dans l’ordre.
Les erreurs de chaque appel et du relâchement restent des échecs. Ce contrôle
établit ce piège du protocole ; il ne prouve pas que chaque blocage historique
du pilote avait cette cause. Les délais et l’échec H1/E6 d’un geste réellement
bloqué restent inchangés.


**Une limite de Toxiproxy.** Un proxy qui avale les paquets transmet quand même la fermeture décidée par l'hôte, alors qu'un vrai Wi-Fi mort ne la transmettrait jamais. Et l'hôte de salon renonce après le même silence que la page. Le scénario `silent` n'avale donc que ce que l'hôte envoie : l'hôte entend toujours la page et ne ferme rien, et seule une page qui a son propre délai de silence s'en aperçoit (C3). Le harnais observe les appels natifs `WebSocket` avant les scripts du jeu : la page doit appeler `close()` sur sa connexion encore `OPEN` avant tout événement natif `close` ou `error`, puis construire une nouvelle connexion vers la même adresse en moins de 20 s. Un contrôle négatif ferme le socket côté serveur et ne satisfait pas cet oracle. Le signal `page.websocket` de l’inspecteur WebKit peut attendre la fin d’un upgrade avalé ; sa date n’est donc pas la date de la tentative de la page.

## Les exigences

**Format d'un item.** Chaque item a :
- un identifiant ;
- l'exigence ;
- son origine (l'expérience dont elle vient) ;
- qui la vérifie : **1** pour un test de propriété, **2** pour un scénario du harnais (`scénario`), **3** pour le salon.

**salon** dans cette colonne : la garantie est celle de la couche réseau commune (`salon/crates/core`), qu'aucun jeu ne réécrit. Elle est jouée par `salon/crates/core/tests/` sur un faux réseau à graine, qui perd, retarde, double et coupe ; le harnais la vérifie encore jeu par jeu.

**Si un item ne s'applique pas**, la raison doit être explicite et cohérente avec
le jeu. Les décisions du catalogue vivent dans `verification.toml` ; leurs
enseignements et les écarts au manifeste se détaillent dans `LEARNINGS.md`.
L’applicabilité d’un minimum commun reste résolue par la politique de l’outil,
notamment depuis `cert.json`. Une phrase d’exclusion ne peut pas retirer ce
minimum. Un cas qui n’est pas encore établi reste inconnu ou à revoir.

### P — Performance mesurée

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| P1 | La cadence avant partie et pendant le jeu sous entrées tient le budget du genre ; par défaut 60 images/s et aucune image au-delà de 33 ms. Le rapport garde le nombre d’images, les p50/p95 et le maximum. | manifeste, principe 7 | 2 `performance` + `monkey` |
| P2 | Une entrée réelle produit son effet visible causal sur la TV en moins de 80 ms depuis le téléphone, ou 50 ms depuis le clavier/souris, sauf ajustement documenté du genre. Une action sans effet échoue. | manifeste, principe 7 | 2 `performance`, sondes de `cert.json` |

### C — Connexion

Jackbox admet que sa reconnexion marche « 90 % du temps », et c'est la première plainte des joueurs. Sur iOS, un téléphone verrouillé perd ses sockets sans que la page le sache (WebKit 247943 : `onclose` n'est jamais appelé). Chez Xbox, ce sont XR-112 (sortie de veille) et XR-115 (manette retirée).

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| C1 | Un téléphone neuf rejoint la partie en moins de 30 s, sans compte. | manifeste | 2 `join` + `rtc` |
| C2 | Après une coupure courte (3 s), le téléphone retrouve **sa** place seul, en moins de 10 s, avec son nom, son personnage et son score. | Jackbox, XR-115 | salon, 1, 2 `cut` |
| C3 | Après une coupure **silencieuse** (rien n'est fermé : la page gèle, les paquets disparaissent), l'hôte constate l'absence en moins de 20 s, et le téléphone se reconnecte au réveil en moins de 10 s. Si le Wi-Fi meurt pendant que la page tourne, la page s'en aperçoit **d'elle-même** en moins de 20 s, sans attendre un `onclose` qui n'arrivera jamais. | iOS (TN2277), WebKit 247943, XR-112 | salon, 2 `freeze` + `silent` |
| C4 | Recharger la page en pleine manche rend la même place, grâce au jeton. | Jackbox, Kahoot | salon (le retour de C2, sans test à lui), 2 `reload` |
| C10 | Une panique WASM sur un téléphone est transmise au journal de l’hôte, recharge la page et rend au joueur le même appareil, le même jeton et sa place. Des paniques répétées arrêtent les rechargements avec un message clair. | chantier anti-panique | salon, 2 `panic` (une injection dans chaque moteur), tests du garde de rechargement |
| C5 | Deux onglets pour le même joueur : le dernier gagne, et l'autre ne se reconnecte pas en boucle. | Sidequest Live (code 4001) | salon, 2 `twin` |
| C6 | Un joueur parti pour de bon ne bloque pas la partie : chaque phase finit à son échéance. | Jackbox, AirConsole | 1 |
| C7 | L'hôte qui redémarre en pleine partie ne laisse aucun écran figé. La TV et les téléphones se reconnectent, ou disent clairement quoi faire. | rocketcrab #70 | salon, 2 `restart` |
| C8 | Une salle pleine, une partie déjà lancée ou une mauvaise version donnent un message clair au joueur, jamais un « lobby fantôme ». | Drawphone #57 | salon, 1, 2 `join` |
| C9 | Les données gardées par le navigateur (jeton, sauvegarde), corrompues ou inaccessibles, n'empêchent pas la page de s'ouvrir. | rocketcrab #105 | 1, 2 `storage` |

### E — Entrées hostiles

Sur un serveur qui fait autorité, une panique termine la soirée. Les clones de Jackbox tombent sur un message qui arrive pour une partie déjà finie (Drawphone #124), ou trop gros (Drawphone #94). Chez Xbox, c'est XR-074 : ni plantage ni blocage sur un réseau mauvais.

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| E1 | Un message illisible (octets au hasard, paquet tronqué ou muté, frame texte) est ignoré. L'hôte et les autres joueurs continuent. | ClusterFuzz, Drawphone #124 | salon, 1, 2 `hostile` |
| E2 | Un message plus gros que la limite (64 Kio) ferme cette connexion, et elle seule. | Drawphone #94 | salon (la limite de 64 Kio est dans `salon-host`, sans test à lui), 2 `hostile` |
| E3 | Un téléphone qui inonde l'hôte (des milliers de messages par seconde) ne ralentit pas les autres. | Colyseus (limiteur) | salon, 2 `hostile` |
| E4 | Un téléphone ne peut ni devenir la TV, ni prendre la place d'un autre avec un jeton inventé. Le premier paquet de la TV, rejoué depuis le réseau, reçoit un refus court et sa connexion se ferme en 3 s, sans jamais l'écran de la TV. | red team | salon, 2 `hostile` |
| E5 | Une action hors de sa phase, ou d'un joueur qui n'est pas concerné, est refusée sans rien changer. Exemples : voter pour soi, répondre après l'échéance, agir dans une partie finie. | Drawphone #124 | 1 |
| E6 | 8 téléphones qui tapent et écrivent au hasard pendant toute une partie ne font apparaître aucune exception, ni dans les pages ni dans l'hôte. | gremlins.js, Android monkey | 2 `monkey` |

### T — Texte et affichage

Pour les noms, c'est Xbox XR-046 : les gamertags, même modernes, s'affichent correctement. Pour le bord de l'écran, c'est la zone sûre de la TV : les 90 % intérieurs. Pour les textes plus longs que prévu, c'est la pseudo-localisation de Netflix et d'Android, et le bug de Punchline (`LEARNINGS.md`, « Mise en page déterministe »).

Les **textes vicieux** sont ceux de `tools/cert/nasty.json`, dans le dépôt commun :
- des emoji composés (`👨‍👩‍👧‍👦`, `🤦🏼‍♂️`) ;
- des lettres surchargées d'accents (« Zalgo ») ;
- de l'arabe et de l'hébreu, qui s'écrivent de droite à gauche ;
- un mot de 80 lettres sans espace ;
- du HTML (`<b>`, `<img onerror>`) ;
- `{joueur}` ;
- des espaces seuls, et une chaîne vide.

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| T1 | Chaque limite de longueur est appliquée par l'hôte, pas seulement par la page, et comptée de la même façon des deux côtés. | Drawphone #134 | 1 |
| T2 | Aucun texte vicieux ne fait planter un écran, n'injecte du HTML, ni ne sort de son cadre. | XR-046, Sivonen (« la longueur d'une chaîne ») | 1, 2 `monkey` + `layout` |
| T3 | Le texte de la TV reste dans la zone sûre, aux formats 16:9, 4:3, ultra-large et portrait, de 1280×720 à 3840×2160. | Xbox (zone sûre), Punchline | 2 `layout` |
| T4 | Sur le téléphone, un champ de saisie a un texte d'au moins 16 px : sinon iOS zoome et la page se décale. | Safari iOS | 2 `layout` |

### D — Déroulement et déterminisme

Age of Empires appelait la désynchronisation « le bug têtu », jusqu'à la sortie du jeu. Factorio compare un hash de l'état à chaque tick. Riot rejoue chaque jour 2 000 à 3 000 parties de League of Legends.

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| D1 | Le même état initial, les mêmes contenus et versions, la même graine et les mêmes entrées externes datées et ordonnées donnent les mêmes états et effets logiques. Le rendu au pixel près et les délais physiques demandent des contrôles distincts. | manifeste (principe 3) | 1 |
| D2 | Une partie se termine toujours, quelles que soient les entrées, les absences et les déconnexions : aucun état ne bloque. | Drawphone #70 | 1, 2 `monkey` |
| D3 | Les invariants du jeu tiennent après chaque pas. Exemples : scores cohérents, un joueur par place, chaque consigne attribuée. Le jeu les énumère dans son test. | Sea of Thieves (tests d'acteurs) | 1 |
| D4 | Pour une simulation en temps réel, le hash de l'état est le même en natif et en WASM sur un replay. | Factorio, Fiedler, League of Legends | 1 |
| D5 | On relance une partie sans rien redémarrer. | manifeste (« Le soir même ») | 1, 2 `monkey` |

### R — Durée et réseau dégradé

Avant la sortie de Diablo IV, Blizzard a organisé un « Server Slam » : un week-end ouvert à tous pour éprouver ses serveurs. Pour le réseau dégradé, la référence est Gaffer on Games : la latence qui varie se sent plus que la latence moyenne.

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| R1 | Avec 200 ms de latence variable (±100 ms) sur chaque téléphone, la partie reste jouable de bout en bout. | AoE, Gaffer on Games | 2 `jitter` |
| R2 | Pendant 30 min de partie avec du chaos, la mémoire de l'hôte n'enfle pas : sa pente, de la 5ᵉ à la dernière minute, projetée sur une soirée de 6 h, reste sous 256 Mio. Son nombre de threads ne grandit pas. | Server Slam | 2 `--soak 30` |
| R3 | La salle pleine (le maximum de joueurs) se joue, et le joueur en trop reçoit un refus clair. | Drawphone #70 | 2 `join` |

### Étage 3 — Le salon (un humain, de vrais appareils, 15 min)

À cocher avant chaque soirée. Ce que le harnais ne sait pas simuler :

| # | Geste | On attend |
|---|---|---|
| S1 | Verrouiller un iPhone 60 s en pleine manche, puis le rouvrir. | Le téléphone revient à sa place tout seul (C3). |
| S2 | Passer 30 s dans une autre app, puis revenir. | Idem. |
| S3 | Couper le Wi-Fi du téléphone 10 s depuis le Centre de contrôle. | Idem (C2). |
| S4 | Activer le mode économie d'énergie. | Rien ne change. |
| S5 | Sur la TV, changer d'onglet ou de fenêtre 1 min. | Le jeu n'a pas ralenti : Chrome bride les minuteurs des onglets cachés. |
| S6 | Dans l'app Télécommande, refuser la permission « réseau local ». | Un message clair, pas un écran vide. |
| S7 | Rejoindre avec un iPhone (Safari et l'app) et un Android (Chrome). | Tout le monde joue. |
| S8 | Lire la TV depuis le canapé, à 3 m. | Tout est lisible, et rien n'est coupé par le bord de l'écran. |
| S9 | Le premier son. | Il joue dès le premier geste (déblocage de l'audio). |
| S10 | Une partie entière avec de vrais joueurs. | Elle est drôle. C'est le seul item qu'aucune machine ne vérifiera. |

## Sources

- Exigences Xbox (XR), publiques : <https://devdocs.xbox.com/publishing/certification/xbox-requirements>. Zone sûre : <https://devdocs.xbox.com/build/core-features/graphics/overviews/screen-areas>
- Sea of Thieves, tests automatisés (GDC 2019) : <https://media.gdcvault.com/gdc2019/presentations/Masella_Robert_AutomatedTestingOf.pdf>
- web-platform-tests : <https://web-platform-tests.org/> ; ClusterFuzz : <https://google.github.io/clusterfuzz/>
- Reconnexion chez Jackbox : <https://www.jackboxgames.com/blog/responding-to-your-questions-from-the-jackbox-customer-survey>. Connexion instable chez AirConsole : <https://developers.airconsole.com/handling-unstable-connections>
- iOS et les sockets suspendus : <https://developer.apple.com/library/archive/technotes/tn2277/_index.html> ; <https://bugs.webkit.org/show_bug.cgi?id=247943>
- Bugs des clones : <https://github.com/tannerkrewson/drawphone/issues> ; <https://github.com/tannerkrewson/rocketcrab/issues>
- Déterminisme : Factorio <https://factorio.com/blog/post/fff-47> ; League of Legends <https://www.riotgames.com/en/news/determinism-league-legends-fixing-divergences> ; Age of Empires <https://www.gamedeveloper.com/programming/1500-archers-on-a-28-8-network-programming-in-age-of-empires-and-beyond>
- La longueur d'une chaîne : <https://hsivonen.fi/string-length/>
- Postmortems sans blâme : <https://sre.google/sre-book/postmortem-culture/>
