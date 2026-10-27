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

| Étage | Quand | Durée | Qui vérifie | Commande |
|---|---|---|---|---|
| **1. Propriétés** | à chaque commit | secondes | `cargo test` | `cargo test` ; `PROPTEST_CASES=5000 cargo test` pour chercher plus loin |
| **2. Certification** | avant de déclarer une étape finie | minutes | le harnais, en boîte noire | `node tools/cert/cert.mjs games/<jeu>` |
| **3. Le salon** | avant la soirée | 15 min | un humain, avec de vrais téléphones | la liste « Étage 3 » plus bas |

- **Étage 1.** Il joue des milliers de suites d'entrées au hasard sur la simulation (`crates/core`) et vérifie des invariants après chaque pas. Un échec est réduit (*shrinking*) au plus petit exemple qui casse encore. Un agent peut corriger ça.
- **Étage 2.** Il lance le vrai serveur et ouvre la TV et 8 téléphones dans le Chrome de l'ordinateur, sans écran, pilotés par Playwright. Chaque téléphone passe par son propre proxy [Toxiproxy](https://github.com/Shopify/toxiproxy), qui sait retarder, couper ou avaler la connexion sans rien dire. Toxiproxy ne porte que du TCP : la TV et les téléphones ouvrent leur page avec `?salon=ws`, qui garde salon sur son WebSocket. Le harnais tape au hasard, coupe, gèle, recharge, redémarre l'hôte, et envoie des paquets hostiles. Il ne connaît du jeu que son fichier `cert.json`, décrit plus bas.
- **Étage 3.** Ce qu'aucun simulateur ne reproduit : la suspension d'iOS, un vrai Wi-Fi, le recul du canapé, savoir si c'est drôle.

## La règle du bug trouvé en jouant

Un bug découvert en jouant est un échec de la certification avant d'être un échec du code. C'est l'esprit des postmortems sans blâme de Google et des *Correction of Errors* d'Amazon. Pour chacun :

1. **Le reproduire.** Un replay, ou une suite d'entrées, qui devient un test de l'étage 1.
2. **Répondre par écrit** dans `LEARNINGS.md` : *pourquoi aucun item de cette liste ne l'a vu ?*
3. **Renforcer la liste.** Ajoute un item, ou durcis un item existant, pour que **tous** les jeux en profitent.

Un test n'est jamais supprimé sans dire ce qui le couvre maintenant : chaque jeu en tient le registre dans la section « Tests retirés au passage à salon » de son `LEARNINGS.md`.

## Lancer le harnais

Une fois, sur l'ordinateur :

```sh
brew install toxiproxy
(cd tools/cert && npm install)
```

Puis, depuis la racine du dépôt :

```sh
node tools/cert/cert.mjs games/<jeu>                    # tous les scénarios (5 à 8 min)
node tools/cert/cert.mjs games/<jeu> --only cut,freeze  # quelques scénarios
node tools/cert/cert.mjs games/<jeu> --seed 12          # rejoue le hasard d'un passage
node tools/cert/cert.mjs games/<jeu> --soak 30          # 30 min d'endurance (R2)
```

Les scénarios, dans l'ordre : `join`, `rtc`, `layout`, `cut`, `freeze`, `silent`, `reload`, `twin`, `hostile`, `jitter`, `monkey`, `restart`, `storage`. La TV lance la partie (`start`) après `rtc`. Seul `rtc` ouvre un téléphone sans `?salon=ws` : l'hôte tourne avec `SALON_LOG=1`, et sa sortie dit si WebRTC l'a porté ; si la salle est pleine, seul le transport est jugé, jamais le refus du joueur en trop. Un écran n'est « reconnecté » que s'il reçoit de l'hôte un `Welcome` ou un paquet du jeu : un refus seul ne compte pas (C7). Un téléphone au stockage refusé ou corrompu arrive en appareil neuf, et le jeu garde un moment la place qu'il avait : `storage` ne le fait donc entrer en appareil neuf que s'il n'avait pas de place (après `restart`, une partie neuve), sinon il lui rend son stockage et sa place, pour ne jamais asseoir N + 1 joueurs sur N places ; à la fin, chaque téléphone doit avoir retrouvé une place. `hostile` ne mesure que sur une connexion encore ouverte au moment d'envoyer (E2 : jamais une fermeture antérieure). Le rapport s'affiche à la fin, avec une ligne par item qui garde **toutes** ses notes, les ✗ d'abord puis les ~, les ✓ et les – (une note répétée, par l'endurance par exemple, n'y est qu'une fois, avec son nombre), et s'enregistre dans `rapport.md` à côté des captures. Le harnais sort en erreur si un item est ✗.

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

**Une limite de Toxiproxy.** Un proxy qui avale les paquets transmet quand même la fermeture décidée par l'hôte, alors qu'un vrai Wi-Fi mort ne la transmettrait jamais. Et l'hôte de salon renonce après le même silence que la page. Le scénario `silent` n'avale donc que ce que l'hôte envoie : l'hôte entend toujours la page et ne ferme rien, et seule une page qui a son propre délai de silence s'en aperçoit (C3).

## Les exigences

**Format d'un item.** Chaque item a :
- un identifiant ;
- l'exigence ;
- son origine (l'expérience dont elle vient) ;
- qui la vérifie : **1** pour un test de propriété, **2** pour un scénario du harnais (`scénario`), **3** pour le salon.

**salon** dans cette colonne : la garantie est celle de la couche réseau commune (`salon/crates/core`), qu'aucun jeu ne réécrit. Elle est jouée par `salon/crates/core/tests/` sur un faux réseau à graine, qui perd, retarde, double et coupe ; le harnais la vérifie encore jeu par jeu.

**Si un item ne s'applique pas** à un jeu, son `LEARNINGS.md` le dit, dans la section « Écarts au manifeste ». Exemple : pas de téléphones dans la tier-list.

### C — Connexion

Jackbox admet que sa reconnexion marche « 90 % du temps », et c'est la première plainte des joueurs. Sur iOS, un téléphone verrouillé perd ses sockets sans que la page le sache (WebKit 247943 : `onclose` n'est jamais appelé). Chez Xbox, ce sont XR-112 (sortie de veille) et XR-115 (manette retirée).

| # | Exigence | Origine | Vérifié par |
|---|---|---|---|
| C1 | Un téléphone neuf rejoint la partie en moins de 30 s, sans compte. | manifeste | 2 `join` + `rtc` |
| C2 | Après une coupure courte (3 s), le téléphone retrouve **sa** place seul, en moins de 10 s, avec son nom, son personnage et son score. | Jackbox, XR-115 | salon, 1, 2 `cut` |
| C3 | Après une coupure **silencieuse** (rien n'est fermé : la page gèle, les paquets disparaissent), l'hôte constate l'absence en moins de 20 s, et le téléphone se reconnecte au réveil en moins de 10 s. Si le Wi-Fi meurt pendant que la page tourne, la page s'en aperçoit **d'elle-même** en moins de 20 s, sans attendre un `onclose` qui n'arrivera jamais. | iOS (TN2277), WebKit 247943, XR-112 | salon, 2 `freeze` + `silent` |
| C4 | Recharger la page en pleine manche rend la même place, grâce au jeton. | Jackbox, Kahoot | salon (le retour de C2, sans test à lui), 2 `reload` |
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

Les **textes vicieux** sont ceux de `tools/cert/nasty.json` :
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
| D1 | La même graine et les mêmes entrées donnent la même partie, écran par écran. | manifeste (principe 3) | 1 |
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
