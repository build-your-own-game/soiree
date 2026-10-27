# Chantiers : la philosophie, les réalisations et les limites restantes

> Ce fichier s'adresse à l'hôte et à l'agent de code qui travaille sur le commun. Il se lit seul : il rappelle la philosophie de `MANIFESTE.md` et `CERTIFICATION.md`, puis consigne les huit chantiers, leurs critères de fin et leurs preuves.

## Le pari

Écrire du code ne coûte presque plus rien. Ce qui limite un jeu, c'est ce qu'on peut **vérifier**, pas ce qu'on peut écrire.

On est plusieurs amis. Chacun fait coder un jeu en Rust par son agent de code, et un soir on y joue tous ensemble chez l'hôte : une TV, des téléphones comme télécommandes, un Wi-Fi de salon. Ce dépôt est le commun : salon (la couche réseau partagée), la télécommande iPhone (`remote/`), les règles, le harnais de certification et le modèle de jeu. Chaque jeu (punchline, sidequest, tier-list…) vit dans son propre dépôt et dépend d'une étiquette de celui-ci.

Trois choses bornent ce qu'un jeu peut viser :

1. ce qu'une machine peut vérifier à la place de l'hôte ;
2. la physique : latence, budget par image, perception humaine ;
3. ce que le navigateur expose.

Ces chantiers font reculer la première limite. Chaque erreur qu'un ami voit le soir même est une erreur qu'aucune machine n'a attrapée avant.

## La philosophie

Trois idées tiennent tout le reste.

### 1. La machine vérifie, pas l'humain

On préfère toujours le choix qui laisse le compilateur, les tests ou un replay attraper l'erreur. Ce sont les agents qui écrivent le code et les tests, donc un humain qui relit ne suffit pas. Ce qui n'est vérifié par aucune machine est considéré comme non vérifié.

### 2. Aucune chaîne n'est un contrat

Toute valeur qui traverse une frontière (deux processus, deux langages, un fichier et le code, deux versions) est écrite et lue par le même type Rust. Un agent qui renomme d'un côté voit casser l'autre à la compilation. C'est ce que veut dire « full stack type-safe » ici.

Le typage a une limite qu'il faut garder en tête : il protège la **forme** des données, pas leur **sens**. Un jeu bien typé peut encore geler sur un iPhone verrouillé, être illisible à 3 m, ou laisser un état absurde arriver.

### 3. Le principe des frontières

> **Nos frontières, on les casse librement, mais bruyamment. Celles du monde, on les traite avec la prudence d'une grande boîte.**

Dans une grande organisation, la douleur vient des frontières entre équipes : une API figée parce que cent clients en dépendent, la rétrocompatibilité, la coordination. Ici, on possède les deux côtés de presque toutes les frontières. Casser est un choix bon marché : le compilateur montre tout ce qui casse, et l'agent de chaque ami répare en quelques minutes.

Cela ne marche qu'à une condition : la casse doit être **bruyante**. Le danger, c'est le changement de sens sans changement de type. Une échéance qui passe de 90 s à 30 s, un message dont le sens dérive, un ordre qui change : rien ne casse, personne ne corrige, et le bug sort le soir même. C'est la loi de Hyrum, et les agents y sont plus exposés que les humains, parce qu'ils s'appuient sur tout ce qui marche.

Deux sortes de frontières échappent à ce confort :

- **Celles qu'on partage à date fixe.** L'app iPhone est installée une fois et parle à tous les jeux. Un changement de son protocole oblige tout le monde à s'aligner avant la soirée.
- **Celles du monde.** Safari, iOS, le Wi-Fi, la TV, des amis qui ne lisent pas les consignes. Personne ne s'aligne sur nous : on teste contre la réalité.

## Où on en est

La forme des données est tenue de bout en bout. Les huit chantiers ajoutent des vérifications du sens et du comportement réel ; les limites qui demandent encore un salon et de vrais téléphones sont indiquées dans les rapports.

| Frontière | Comment elle est tenue | Reste |
| --- | --- | --- |
| Réseau hôte ↔ écrans | Enums partagées, encodées en `postcard` | — |
| Version du jeu | Calculée depuis les sources (`salon/version.rs`) : une page en cache est refusée | — |
| Rust ↔ page HTML | `soiree dom` génère des constantes typées ; les méthodes à chaînes de `web_sys` sont interdites | Valeurs CSS écrites par Rust |
| Rust ↔ app iPhone | Swift généré depuis Rust (`remote_ffi.swift`) | La couche d'interface Swift |
| Fichiers et `localStorage` | Validés à la lecture par un type (`SavedBoard`…) | — |
| Jeu ↔ harnais | `cert.json`, en chaînes, **exprès** : le harnais attaque en boîte noire | Casse bruyante, acceptable |

La certification a trois étages :

1. **Propriétés** (`cargo test`) : des milliers de suites d'entrées au hasard sur la simulation, avec réduction automatique de l'exemple qui casse.
2. **Harnais** (`soiree certify`) : le vrai serveur, la TV Chromium et 8 téléphones (4 WebKit, 4 Chromium) sans écran, Toxiproxy pour couper ou ralentir, des paquets hostiles, des redémarrages.
3. **Le salon** : 15 min avec de vrais téléphones, pour ce qu'aucune machine ne simule.

Des cibles de fuzz existent dans punchline, tier-list et `remote/`. Salon est testé sur un faux réseau à graine qui perd, retarde, double et coupe.

## Les huit chantiers

Les huit chantiers composent la version `v0.1.0`. Les descriptions « Pourquoi » ci-dessous gardent le constat initial ; le bilan de vérification précise ce qui a été exécuté. Les deux premiers débloquent la boucle « bug vu le soir même → test », dont tout le reste profite.

| # | Chantier | Où | Ce qu'il évite le soir même | Statut |
| --- | --- | --- | --- | --- |
| 1 | Enregistreur de partie | salon + jeux | Un bug vu, impossible à rejouer | Fait, tests et replays vérifiés |
| 2 | Filet anti-panique | salon | Un écran figé sans explication | Fait, récupération WebKit et Chromium vérifiée |
| 3 | Mutation testing, désactivé par défaut | outil `soiree` | Des tests qui passent sans rien vérifier | Fait, premiers rapports ciblés |
| 4 | Simulation déterministe de l'hôte entier | salon + jeux | Les bugs de timing entre jeu et réseau | Fait, campagnes de 2048 graines |
| 5 | Téléphones WebKit dans le harnais | `tools/cert/` | Les bugs propres à Safari | Fait, tier-list solo non applicable |
| 6 | États impossibles | jeux | Des incohérences silencieuses | Fait, refactoré et revu |
| 7 | Budgets mesurés par le harnais | `tools/cert/` + jeux | Un jeu lent qu'on croyait fluide | Fait, chiffres dans les rapports |
| 8 | Manifeste ajusté | `MANIFESTE.md`, `CHANGELOG.md` | Des agents qui suivent des consignes périmées | Fait |

### 1. L'enregistreur de partie

**Pourquoi.** Le principe 3 du manifeste promet des replays « gratuits », puisque la simulation est déterministe. Mais punchline n'enregistre ni la graine ni les entrées d'une vraie partie : un bug vu le soir même ne se rejoue pas. Sidequest a des journaux ; les autres non.

**Comment.** Salon fournit un enregistreur, générique sur le type d'entrée du jeu, écrit en continu sur un thread à part :

- un en-tête : version calculée, graine, réglages, empreinte du contenu ;
- une ligne par entrée : tick, joueur, action, arrivées et départs.

Le piège : le jeu enregistre l'entrée **là où elle entre dans la simulation**, après que l'hôte a fixé leur ordre. Enregistrées au niveau du réseau, les entrées ne reviendraient pas dans le même ordre. Le fichier est écrit et relu par le même type Rust (principe 8). En plus : une touche « bug » sur la TV marque l'instant dans le journal.

**Fini quand.** Chaque jeu écrit un journal par partie, une commande le rejoue et compare l'état final, et chaque jeu a au moins un test qui rejoue un journal versionné.

### 2. Le filet anti-panique

**Pourquoi.** Dans le navigateur, une panique WASM fige la page sans rien dire. Les règles l'interdisent dans notre code, pas dans les dépendances ni dans le code de rendu exempté. Seul sidequest installe un panic hook.

**Comment.** `salon_web` installe un hook commun à tous les jeux. Il envoie le message à l'hôte, qui l'écrit dans son journal, affiche « Petit souci, on recharge » et recharge la page. Le jeton ramène le joueur à sa place (C4). Après quelques recharges rapprochées, il s'arrête sur un message clair, pour ne jamais boucler.

**Fini quand.** Un scénario du harnais fait paniquer une page et attend que le joueur retrouve sa place.

### 3. Le mutation testing, désactivé par défaut

**Pourquoi.** Les agents écrivent le code *et* les tests : qui vérifie que les tests attrapent vraiment un bug ? `cargo-mutants` modifie le code (un `<` devient `<=`, une fonction renvoie sa valeur par défaut) et relance les tests. Un mutant qui survit, c'est du code dont aucun test ne remarque le changement.

**La contrainte.** Intégré au code, pas du code mort, mais désactivé par défaut, et hors des certifications obligatoires s'il prend du temps.

**Comment.**

- Une commande dédiée de l'outil `soiree`, jamais lancée par `soiree check` ni `soiree certify`. Elle vise `crates/core` par défaut, et peut ne muter que le code modifié (`--in-diff`).
- Un rapport des mutants survivants, rangé avec les autres rapports du jeu, qui ne bloque rien.
- Des exclusions possibles, chacune avec sa raison écrite, dans l'esprit de `#[expect(reason)]`.
- **Pour qu'il ne pourrisse pas** : les tests de l'outil `soiree` lancent la commande sur le modèle de jeu, sur un périmètre minuscule. Le code est exercé à chaque changement du commun, sans coûter une seconde aux jeux.

**Fini quand.** Le modèle a un passage vert dans les tests de `soiree`, et chaque jeu a un premier rapport.

### 4. La simulation déterministe de l'hôte entier

**Pourquoi.** Chaque pièce est testée seule. Le fuzz attaque un décodeur ou la simulation ; salon est testé sans jeu ; le harnais teste tout, mais en temps réel, en minutes, et sans pouvoir rejouer au bit près. Personne ne teste la boucle de l'hôte (jeu, salon, reconnexions, échéances) sur une horloge simulée. C'est là que vivent les bugs du type « le téléphone revient pile quand la manche finit ».

**Comment.** Comme FoundationDB et TigerBeetle : salon expose son hôte derrière une horloge et un réseau abstraits. Un test lance le jeu, salon et N bots dans un seul processus, sur le faux réseau à graine, et vérifie les invariants après chaque pas. Une graine qui casse se rejoue au bit près, devient un journal (chantier 1), puis un test. C'est le plus gros chantier : il touche l'API de salon et la boucle de chaque hôte.

**Fini quand.** Chaque jeu fait tourner des milliers de graines dans `cargo test`, et une variable d'environnement en lance beaucoup plus.

### 5. Des téléphones WebKit dans le harnais

**Pourquoi.** La plupart des téléphones seront des iPhone, donc WebKit. Le harnais n'ouvre que Chromium (`tools/cert/browser.mjs`), alors que Playwright sait piloter WebKit.

**Comment.** Une partie des 8 téléphones dans WebKit, avec le profil iPhone de Playwright (taille d'écran, tactile) ; `soiree doctor` vérifie qu'il est installé. Limite : ce WebKit n'est pas Safari iOS. Il attrape les différences de CSS et d'API, pas la mise en veille d'iOS, qui reste à l'étage 3.

**Fini quand.** Les trois jeux passent `soiree certify` avec des téléphones WebKit.

### 6. Les états impossibles

**Pourquoi.** Un type qui admet une combinaison absurde oblige chaque lecteur à décider quoi en faire, et chacun décide différemment. Rien ne plante : c'est incohérent sans bruit. Punchline le note lui-même : `Outcome { swept, forfeit, jinx }` admet un forfait et un jinx à la fois, et `Player { bot, presenter }` un présentateur humain.

**Comment.** Ce qui s'exclut devient une enum ; ce qui se combine reste en champs séparés. Par exemple `enum Fin { Jouée { balayage: Balayage }, Forfait, Jinx }` et `enum Siège { Humain, Bot, Présentateur }`. La forme exacte se décide sur le sens réel du jeu. Des invariants que les tests de propriétés vérifient aujourd'hui deviennent des garanties du compilateur.

**Fini quand.** Les états impossibles listés dans les « Écarts au manifeste » de chaque jeu ont disparu, ou chacun a sa raison.

### 7. Les budgets mesurés par le harnais

**Pourquoi.** Le principe 7 fixe des budgets : 60 images par seconde, aucune image au-delà de 33 ms, moins de 80 ms du téléphone au pixel. Punchline ne les mesure pas, sidequest en partie. Non mesurés, ce sont des souhaits.

**Comment.** Le harnais mesure ce qu'il voit de l'extérieur : le temps entre un geste sur un téléphone et le changement sur la TV, la durée des images, le temps entre l'ouverture de la page et « rejoint ». Il écrit les chiffres dans le rapport, et échoue au-delà du budget. L'overlay de chaque jeu reste pour l'humain.

**Fini quand.** Le rapport de certification de chaque jeu contient ses chiffres.

### 8. Le manifeste ajusté

**Pourquoi.** Il dit « à définir » pour des choses qui existent : la télécommande native, le choix du réseau. Un agent qui le lit suit des consignes fausses.

**Comment.** Garder la philosophie et les contraintes, sans décrire ce qui est en place. Les décisions s'écrivent comme prises, avec un renvoi vers leur doc (`salon/README.md`, `remote/CONTRAT.md`). On y ajoute le principe des frontières, et une rubrique « Change de sens » au `CHANGELOG.md`, pour rendre bruyants les changements qui ne changent aucun type.

**Fini quand.** Plus aucune section ne dit « à définir » pour ce qui existe.

## Bilan de vérification — 27 octobre 2026

Le commun, le modèle rendu et les trois jeux ont passé `soiree check`, avec
Clippy natif et WASM et les tests Rust. Le catalogue a 97 items valides.
Les 48 tests de l'outil passent, dont une campagne réelle de trois mutants
du modèle, tous détectés. Les 17 tests Node du harnais passent. Les premiers
rapports des jeux détectent respectivement un mutant Punchline, un Sidequest
et deux tier-list ; leur périmètre est explicitement limité dans chaque rapport.

Chaque jeu et le modèle ont un passage complet de `soiree certify`, graine
`20261027`, sans échec bloquant. Les JSON attestent que leurs sources et le
harnais n'ont pas changé pendant chaque passage. Mesures sur cette machine,
sur secteur, avant partie et pendant les gestes aléatoires :

| Cible | Cadence des deux fenêtres | Plus grand intervalle | Entrée → rendu mesurée | Rapports |
| --- | --- | --- | --- | --- |
| Modèle jouable | 60 images/s | 16,67 ms | WebKit 25,4 ms ; Chromium 23,5 ms | [complet](certification/modele/rapport.md), [C9 corrigé](certification/modele/storage-regression/README.md) |
| Punchline | 60 images/s | 16,67 ms | téléphone WebKit 22,4 ms ; TV 37,4 ms | `games/punchline/certification/` dans le dossier de l'hôte |
| Sidequest | 60 images/s | 16,67 ms | WebKit 41,9 ms ; Chromium 45,8 ms, projection après rendu GPU | `games/sidequest/certification/` dans le dossier de l'hôte |
| Tier-list solo | 60 images/s | 16,67 ms | clavier/souris 26,5 ms | `games/tier-list/certification/` dans le dossier de l'hôte |

Le modèle et Punchline conservent leur rapport complet, puis un complément
`storage-regression/` sur les mêmes sources. Ce complément vérifie la dernière
correction isolée de C9 : le harnais restaure l'identité dans le document suivant,
attend un nouveau Welcome et exige huit téléphones sur huit à leur place.
Les deux empreintes de harnais et la portée des passages figurent dans ces
compléments ; les mesures complètes antérieures restent conservées. Sidequest
et tier-list ont exécuté directement le harnais avec cette correction.

La revue croisée a couvert les journaux et leurs anciens chemins, les frontières
de simulation, les invariants des jeux, les adaptateurs de l'hôte, le modèle,
le harnais et les changements logiques de la télécommande. Aucun code mort ni
doublon métier restant n'a été confirmé dans ce périmètre. L'ancien writer de
replay Sidequest et le lecteur moderne redondant ont été retirés ; son lecteur
historique reste utilisé par les fixtures et le fuzz. La vraie salle et les
simulations utilisent le même `Driver`. Les tirages des bots Punchline, la
rotation de deux parties, les restaurations de tier-list et l'appui maintenu
du modèle pendant une mise à jour d'un autre joueur ont leur vérification.

Les `~` et `–` des rapports restent visibles : jugement de lisibilité et de
jouabilité, essais sur de vrais iPhone et TV, endurance optionnelle de trente
minutes, capacité de 71 joueurs Sidequest non remplie par ce passage à huit.
Le redémarrage du serveur Punchline ouvre une partie neuve et demande à nouveau
les prénoms. Tier-list conserve son fonctionnement solo. Le WebKit de Playwright
ne remplace pas la mise en veille réelle d'iOS.

La version `v0.1.0` publie le commun vérifié. Les jeux de l'hôte doivent pointer
sur cette étiquette publiée avant leur propre push. Le `[patch]` reste un outil
de développement local ; la vérification de publication se fait aussi hors du
dossier de l'hôte, sans ce patch, selon `AGENTS.md`.

## Les chantiers pour plus tard

Quatre idées retenues mais remises à plus tard : pour l'instant, on s'attaque au code.

| Chantier | L'idée | Pourquoi plus tard |
| --- | --- | --- |
| Testeurs IA | Un modèle de vision juge une capture de la TV réduite comme vue à 3 m ; un agent joue une partie et dit où il a hésité. Un signal, jamais un verrou. | Le code d'abord |
| Red team par agent | Un adversaire qui ne connaît que le protocole et `cert.json`, et cherche intelligemment à casser, là où le singe tape au hasard. | Plus utile une fois les chantiers 1 et 4 faits |
| WebRTC sous chaos | Toxiproxy ne porte que du TCP, donc le harnais force `?salon=ws`. Le chemin du soir même, WebRTC, n'est éprouvé que par le faux réseau de salon. Il faudrait brider le réseau au niveau du système (`dnctl`/`pfctl` sur Mac), ce qui demande sudo. | La logique de reconnexion est déjà testée ; seul manque le comportement de la pile WebRTC du navigateur |
| Fuzzing planifié | Les cibles existent, personne ne les fait tourner longtemps. Sur des dépôts publics, GitHub Actions peut les lancer la nuit, jusqu'à 6 h par tâche, sans toucher à l'ordinateur de l'hôte. | Pas réaliste sur la machine locale |

## Ce qu'on ne fait pas, et pourquoi

- **On n'outille pas le goût.** La sensation de jeu et l'humour restent la liberté de chaque auteur. Le commun peut fournir des outils (journal, replay qu'on revoit), jamais imposer un style. S10, « c'est drôle », reste jugé par les amis.
- **On ne surspécifie pas.** Le manifeste dit la philosophie et les contraintes ; ce qui est en place se lit dans le code et dans sa doc. Un texte qui décrit l'implémentation vieillit et finit par mentir aux agents.
- **On ne protège pas nos propres frontières par la rétrocompatibilité.** On les casse quand il le faut, bruyamment, et chaque ami réaligne son jeu avec son agent. Seule exception : le protocole de l'app iPhone, partagé par tous à la date de la soirée.
- **Rien de lent ne bloque la boucle serrée.** Ce qui prend des minutes (mutants, longues campagnes de graines) existe, est exercé, mais se lance à la demande.

## Décisions de l’hôte — 27 octobre 2026

- Les huit chantiers sont dans le périmètre, parallélisés lorsque leurs fichiers et contrats le permettent ; une revue complète de la logique, des anciens chemins et du code inutilisé suit l’intégration.
- Les journaux réels restent locaux, ignorés par Git, pendant trente jours. Les fixtures versionnées ne contiennent que des noms et contenus fictifs.
- Les trois jeux et le modèle reçoivent les intégrations applicables. Tier-list reste un éditeur solo : sa simulation et ses replays portent sur la session et l’historique ; téléphone et reconnexion de joueur ne s’y appliquent pas.
- Les mutants sont exercés sur un petit cas réel du modèle dans les tests de l’outil, sans tâche hebdomadaire. Chaque jeu reçoit un premier rapport dont le périmètre est explicite.
- Le harnais utilise quatre téléphones WebKit et quatre Chromium ; un signal de canvas ne compte que lorsqu’il projette ce qui a été effectivement dessiné.
- La session externe DOM/certification est prise en compte : aucun module de DOM généré existant n’est modifié à la main, et les certifications avec mesures s’exécutent sans compilations concurrentes.
