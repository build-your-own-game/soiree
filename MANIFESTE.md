# Manifeste : jeux de soirée en Rust, repensés de zéro

**Ce document ne doit être modifié que sur demande explicite de l’hôte.**

> Ce fichier s'adresse à l'agent de code (Claude Code, Codex…) qui code ton jeu. L'outil `soiree` l'écrit dans `.soiree/MANIFESTE.md` de chaque jeu, à la version de son `soiree.toml`, et l'`AGENTS.md` du jeu dit de le lire et de l'appliquer.

## Le pari

On est plusieurs amis. Chacun fait coder un jeu par son agent de code. Un soir, on y joue tous ensemble chez l'hôte, sur la TV.

Le pari : écrire du code ne coûte presque plus rien. Prenons cette hypothèse au sérieux : jusqu’où peut-on repenser la fabrication d’un jeu quand le travail d’écriture cesse de dicter nos compromis ? Le code existant et son histoire gardent les traces de ce que le réel a appris aux autres. Récupère ces leçons, puis ose des choix qu’on aurait écartés pour leur coût humain. L’ambition porte autant sur le jeu que sur les moyens de savoir qu’il marche : éliminer des familles d’erreurs par sa construction, donner à la machine les moyens de débusquer celles qui restent. Architecture et méthode de travail se pensent ensemble, au service d’un jeu qui tient ses promesses le soir même. Vérifier prend encore du temps : si chaque changement impose une longue attente, on explore moins et on corrige plus lentement. Le coût des garanties dans la boucle de développement fait donc lui aussi partie de notre exploration.

## Le niveau d'exigence

Vise la qualité d'un jeu commercial de son genre, pas celle d'un prototype de game jam. Un clone de Mario doit avoir la sensation de Mario. Un FPS doit avoir le netcode, la sensation des armes et le rendu d'un vrai FPS. Un jeu à la Jackbox doit être aussi drôle et fluide que Jackbox.

Ce n'est pas la quantité de code qui limite ce que tu peux faire. Ce qui limite, c'est :

1. ce que tu peux **vérifier** (voir *Vérification*) ;
2. la physique : latence, budget par image, perception humaine ;
3. ce que le navigateur expose.

Le jeu reste jouable de bout en bout à tout moment. L'ambition s'empile par couches sur un jeu qui marche, jamais d'un seul coup.

## Ce qui est fixé

### Où ça tourne

- Sur l'ordinateur branché à la TV, dans un navigateur récent en plein écran. *(À confirmer par l'hôte : navigateur et système exacts.)*
- Le jeu est écrit en Rust et compilé en WebAssembly. Le JavaScript se limite à la colle générée (wasm-bindgen) et au strict nécessaire pour atteindre une API du navigateur.
- Tu peux livrer en plus un binaire Rust natif qui tourne sur la même machine : serveur local, relais réseau, service des fichiers. Idéalement, il partage son code avec le jeu.
- Le jeu fonctionne sur le Wi-Fi local, sans serveur internet pour relier les joueurs. Salon choisit le transport et garde les identités ; les garanties et limites sont dans `salon/README.md`. Un jeu peut utiliser internet comme mécanique, sans en faire une condition de connexion.
- La TV, les téléphones, l'app iPhone et les manettes rejoignent le jeu par la couche commune `salon/`, qui gère tout le réseau : lis `salon/README.md`.

### Qui joue, et avec quoi

- De 2 à N joueurs dans la même pièce, devant une TV partagée.
- Entrées disponibles :
  - le clavier et la souris de l'ordinateur hôte ;
  - des manettes, via la Gamepad API. Le modèle exact sera donné dans *Matériel* ; d'ici là, code contre le mapping `standard` ;
  - les téléphones, comme télécommandes. N'hésite pas à t'en servir : écran tactile, saisie de texte, gyroscope, vibration, et un écran privé par joueur.
- Un joueur rejoint la partie en moins de 30 secondes, sans créer de compte.

### Télécommande téléphone

Deux voies disponibles :

1. **Une page web** servie par ton jeu et ouverte via un QR code affiché sur la TV. Elle est disponible tout de suite, et c'est à toi de la coder.
2. **L'app iPhone native de l'hôte** (Android plus tard). Son contrat est dans `remote/CONTRAT.md` ; salon la relie à la page téléphone. Garde la voie web pour les joueurs sans app.

## La méthode : extraire le signal du code existant

C'est la partie la plus importante de ce document.

Une librairie utilisée depuis des années (moteur de rendu, moteur de jeu, framework multijoueur) est une archive de **pression d'adaptation**. Chaque commit, chaque PR, chaque issue, chaque changelog garde la trace d'un moment où la réalité a contredit le design initial : un bug sur un GPU précis, des joueurs qui trouvaient le saut mou, un réseau qui perdait des paquets, une API que personne n'arrivait à utiliser.

Cette expérience s’accumule aussi dans les outils qui mettent le code à l’épreuve. Un simulateur de pannes, un vérificateur ou une suite de conformité portent des connaissances sur ce qui peut mal tourner et sur les moyens de le détecter. Étudie-les avec la même attention que les moteurs : leurs scénarios, leurs garanties, leurs hypothèses et leurs limites. Réutilise leurs outils, leurs tests ou leurs idées lorsque cela renforce notre vérification. Le travail des autres peut ainsi nous apprendre à construire le jeu, mais aussi à poser des questions auxquelles notre propre code n’aurait pas pensé.

Ton travail n'est ni de recopier ces librairies, ni de les ignorer. C'est de **séparer le signal du bruit**, puis de reconstruire.

### Trier chaque leçon

| Origine de la pression | Exemples | Que faire |
|---|---|---|
| Physique, matériel, perception humaine | interpolation entre pas de simulation fixes ; compensation de latence ; débordement de texture en pixel art ; sensation du saut (coyote time, tampon d'entrée) | **Garder.** Ça s'applique aussi à toi. |
| Usage réel | reconnexion d'un joueur après une coupure ; audio débloqué au premier geste ; manette qui se déconnecte en pleine partie | **Garder.** Quelqu'un a souffert pour l'apprendre. |
| Accident du langage ou du runtime | contournements du ramasse-miettes de JS ; tout en mono-thread ; pièges de `this` | **Jeter**, après avoir vérifié qu'il n'existe pas d'équivalent en Rust (allocations dans la boucle chaude, par exemple). |
| Héritage et compatibilité | support de vieux navigateurs ou de WebGL1 ; API gelée pour des milliers d'utilisateurs ; options de configuration pour tous les cas | **Jeter.** Tu n'as qu'un seul cas d'usage : ce jeu, ce soir-là. |

### Procédure

Pour chaque sous-système important (rendu, physique, réseau, entrées, audio, animation, interface, game feel) :

1. Choisis 2 à 4 références sérieuses, si possible écrites dans des langages et à des époques différents.
2. Lis le code **et** son historique : `git log` sur les fichiers clés, les PR les plus discutées, les issues fermées avec beaucoup de commentaires, les changelogs majeurs, les postmortems et les talks des auteurs.
3. Note chaque leçon dans `LEARNINGS.md`, avec sa source et sa catégorie.
4. Reconstruis le sous-système en Rust pour **ton** jeu, en gardant les leçons universelles et rien d'autre.
5. Utiliser une crate existante reste un choix légitime quand elle encode déjà ces leçons (par exemple `wgpu` pour l'accès au GPU, ou `rapier` pour la physique). Décide-le consciemment, et écris pourquoi.

Les jeux existants sont aussi une source : projets publics de décompilation, analyses de netcode, études image par image de mécaniques. Reproduis les mécaniques et la sensation, mais crée tes propres assets.

### Points de départ (liste non exhaustive)

- **Rendu** : wgpu, Bevy (`bevy_render`), Godot, three.js, Filament.
- **Moteurs complets** : Bevy, Godot, Phaser (beaucoup de leçons propres au navigateur), id Tech (Quake, open source).
- **Physique** : Rapier, Box2D v3, Jolt.
- **Réseau multijoueur** : netcode de Quake 3, documentation réseau du Source Engine de Valve, GGPO (rollback), Colyseus, talks GDC sur Overwatch et Rocket League, articles de Glenn Fiedler (*Gaffer on Games*).
- **Entrées** : SDL (`gamecontrollerdb`), Gamepad API, Steam Input.
- **Game feel** : les écrits de Maddy Thorson sur Celeste, le talk *Juice it or lose it*, le livre *Game Feel* de Steve Swink.
- **ECS** : Bevy ECS, flecs.
- **Interface en mode immédiat** : egui, Dear ImGui.

## Principes d'ingénierie

On repense tout de zéro, mais toujours avec une raison. Si ton jeu a une meilleure raison de faire autrement, fais autrement et écris pourquoi.

**Nos frontières, on les casse librement, mais bruyamment. Celles du monde, on les traite avec la prudence d'une grande boîte.** On possède les jeux et le commun : une rupture de type se répare chez tous les auteurs. Un changement de sens sans rupture de type est plus dangereux : il se documente dans « Change de sens » du changelog et possède un test. Le protocole de l'app iPhone, installé pour la soirée, impose un alignement explicite. Safari, iOS, le Wi-Fi et la TV ne s'alignent pas sur nous : on les éprouve.

Les types protègent la forme des données ; les tests, les replays et les mesures protègent leur sens.

Une interface possède aussi des durées de vie. Ses listeners, timers et boucles
d’animation appartiennent à une page ou une vue, et s’arrêtent avec elle.
Une réponse asynchrone appartient à l’opération qui l’a demandée : elle ne
modifie rien si cette opération a été annulée, remplacée ou détruite. Une
fermeture suivie d’une réouverture ne rend pas l’ancien résultat valable.
Les éléments en cours d’édition conservent leur identité, leur sélection et
leur composition tant que leur objet logique existe. Les valeurs dérivées
se calculent depuis l’état faisant autorité ; un effet ponctuel possède une
identité qui empêche de le rejouer lors d’un simple réaffichage.

Écris-le dans une section `## Écarts au manifeste` de ton `LEARNINGS.md`, avec un point numéroté par principe ci-dessous. Chaque point commence par **Respecté.**, par **Écart.** suivi d'une « Raison : », ou par **À faire.**

1. **La machine vérifie, pas l'humain.** Préfère les choix qui laissent le compilateur, les tests et les replays attraper les erreurs : types forts, états impossibles non représentables, aucun `unwrap` dans la boucle de jeu sans raison écrite.
2. **Simulation à pas fixe, rendu interpolé.** La logique avance par ticks fixes et le rendu interpole entre eux. C'est ce qui rend la sensation de jeu indépendante de la fréquence de l'écran.
3. **Déterminisme par défaut.** Une partie, c'est une graine, un état initial et un flux d'entrées horodatées. Cela rend possibles les replays, la reproduction de bugs, le rollback et les tests. Un replay exige encore un enregistrement effectif des entrées dans leur ordre d’application, avec l’état initial et une empreinte de l’état à comparer. Attention aux flottants entre WASM et natif, à l'ordre d'itération des `HashMap`, et à toute source de hasard sans graine.
4. **Le jeu tourne sans écran.** La simulation compile en natif sans GPU ni navigateur, pour les tests, les bots et le serveur.
5. **Pas de compromis par habitude.** Tu contrôles le serveur local : tu peux envoyer les en-têtes COOP/COEP et utiliser les threads WASM (`SharedArrayBuffer`). Les joueurs ont tous lancé le jeu exprès : un temps de chargement compte beaucoup moins que sur le web public. Vise WebGPU directement.
6. **Le réseau suit la physique.** Quel que soit le chemin (Wi-Fi local ou internet), mesure sa latence réelle et conçois le jeu en fonction. Envoie les entrées sur un canal non fiable et non ordonné (WebRTC DataChannel) plutôt qu'en TCP. Garde un état faisant autorité sur une seule machine. Jette les entrées périmées au lieu de les rejouer.
7. **Des budgets mesurés, pas supposés.** Points de départ, à ajuster selon le genre :
   - au moins 60 images par seconde (120 si l'écran le permet), et aucune image au-delà de 33 ms en jeu ;
   - de l'entrée au pixel : moins de 50 ms depuis une manette ou un clavier, moins de 80 ms depuis un téléphone en Wi-Fi local ;
   - moins de 30 s entre le scan du QR code et le premier mouvement.

   Mesure-les dans le jeu, avec un overlay de debug, et note les résultats.
8. **Aucune chaîne n'est un contrat.** Toute valeur qui traverse une frontière (entre deux processus, deux langages, un fichier et le code, ou deux versions) est écrite et lue par le même type Rust, ou validée pour en devenir un dès son entrée. Le compilateur garde alors toute la pile, et un agent qui renomme d'un côté voit casser l'autre. Concrètement :
   - les messages entre l'hôte et les écrans sont des enums partagées (`Up`, `Down`), jamais du JSON écrit à la main ;
   - la page ne se lit pas par chaîne : `soiree dom` tire de son HTML et de son CSS un module de constantes typées (ids, classes, attributs, propriétés CSS, balises), et les règles interdisent les méthodes à chaînes de `web_sys` ailleurs. Rust ne peut nommer que ce que la page déclare ;
   - les valeurs d’interface que Rust écrit ont leur type : couleur, longueur, nombre, durée, image. Une propriété CSS générée n’accepte que sa catégorie ; une variable personnalisée déclare cette catégorie dans sa feuille avec `soiree-type`. La conversion en texte se fait à la frontière du navigateur, avec validation des valeurs extérieures ;
   - le contrat entre une page libre et l’app iPhone a une définition Rust unique, utilisée par le SDK et par l’app à travers ses liaisons Swift générées. Les objets JavaScript sont validés à l’entrée ; le code Swift ne redéfinit ni leurs clés ni leurs règles. Des tests exécutent les vraies liaisons et les scripts émis ;
   - la version du jeu que vérifie salon se calcule depuis ses sources (`salon/version.rs`) : une page restée en cache après un changement est refusée, au lieu de décoder de travers ;
   - les données lues à l'exécution (contenu, sauvegardes, `localStorage`) passent par un type qui les valide, et leurs fichiers sont vérifiés par un test.

   Le vocabulaire du navigateur lui-même (noms d'événements, de touches) reste en chaînes : ce n'est pas un contrat entre nos fichiers. Le harnais de certification aussi : il attaque le jeu en boîte noire, et partager nos types lui ferait partager nos erreurs.

## Outillage commun

Tous les jeux utilisent la même version de Rust, pour que n'importe lequel compile sur la machine de l'hôte sans surprise. Rust stable est rétrocompatible : la dernière version stable est aussi la plus mûre. On la fige jusqu'à la soirée.

Ces règles sont un socle. Tu peux les durcir librement. Pour en assouplir une, il faut une raison écrite, à l'endroit précis : `#[expect(lint, reason = "...")]`, jamais `#[allow]`.

Les fichiers eux-mêmes sont dans `outil/regles/` du dépôt commun, embarqués dans l'outil `soiree` de chaque version. `soiree new` et `soiree upgrade` les écrivent dans le jeu ; `soiree check` vérifie qu'ils y sont. Ce qu'ils imposent, et pourquoi :

- **`rust-toolchain.toml`** fige la version de Rust, avec `rustfmt`, `clippy` et la cible `wasm32-unknown-unknown`.
- **`Cargo.toml`** (dans un workspace, sous `[workspace.lints]`, chaque crate reprenant `[lints] workspace = true`) :
  - l'édition 2024 ;
  - pas de code `unsafe` ;
  - Clippy `all` et `pedantic` en erreur, sauf trois lints de documentation d'API publique, du bruit dans du code de jeu ;
  - aucune exception muette : `#[allow]` est interdit, `#[expect]` doit donner sa raison ;
  - ni `unwrap`, ni `expect`, ni `panic!`, ni index hors bornes, ni `dbg!`, ni `todo!` : chacun arrête la partie ;
  - ni `_ =>` sur une enum : le variant qu'on ajoutera demain doit casser la compilation là où il faut le traiter, pas tomber en silence dans le cas par défaut ;
  - ni paramètre `bool` : deux booléens s'inversent sans que le compilateur le voie, une enum à deux variants ou deux méthodes non ;
  - ni arithmétique entière qui peut déborder : avec le dépassement vérifié en release, elle arrête la partie. On choisit `checked_`, `saturating_` ou `wrapping_` selon ce que veut dire le débordement. Le code de rendu et les tests peuvent s'en exempter en tête de crate ou de module, avec leur raison ;
  - ni conversion entier → flottant qui perd des chiffres sans le dire ;
  - ni valeur jetée sans son type : `let _: T = …`, jamais `.ok()` pour faire taire un `Result` ;
  - ni accès à la page par chaîne (`get_element_by_id`, `set_attribute`, `class_list`, `set_inner_html`, `style().set_property`…) : le module de `soiree dom` à la place (principe 8) ;
  - les dépendances optimisées même en debug, sans quoi un jeu est injouable ;
  - le dépassement d'entier vérifié même en release : c'est un bug.
- **`clippy.toml`** protège le déterminisme (principe 3) : ni `HashMap` ni `HashSet` (ordre d'itération aléatoire), ni `Instant::now` ni `SystemTime::now` (le temps entre dans la simulation comme une entrée), ni les fonctions mathématiques des flottants dont le résultat diffère entre WASM et natif (`sin`, `cos`, `powf`… : utilise le crate `libm`). Les tests, eux, ont droit à `unwrap`, `expect`, `panic!` et aux index.

Ces interdictions protègent la simulation. Si le code de rendu a besoin d'une `HashMap` ou de `sin`, utilise `#[expect(clippy::disallowed_types, reason = "...")]`, ou donne au crate de rendu son propre `clippy.toml`.

Avant de déclarer une étape finie, `soiree check`, dans le dossier du jeu, passe sans erreur. Il vérifie tout ce qu'une machine peut vérifier dans ce document :

- les règles ci-dessus, dans la version de `soiree.toml` ;
- une simulation (`crates/core`) qui ne dépend ni du navigateur ni du GPU ;
- aucun JavaScript écrit à la main dans `web/` ;
- les chemins cités dans `AGENTS.md` et `README.md`, qui doivent exister ;
- la section « Écarts au manifeste » ;
- un `cert.json` lisible par le harnais de `CERTIFICATION.md`, et le `CERTIFICATION.md` du jeu, dont chaque test nommé existe.

Le modèle fournit aussi une CI liée à la version du commun. `soiree check`
refuse une CI absente ou périmée et exige que `cert.json` exerce les propriétés
d’interface applicables, ou explique leur non-applicabilité. Les primitives
communes sont testées dans le commun ; leur intégration est exercée dans les
jeux par `soiree certify`. Le mode CI `soiree check --locked --published`
refuse un salon local ou une autre étiquette : ce qui marche chez l’hôte doit
également se construire depuis les dépendances publiées. La mise à niveau
se fait avec l’outil de la version cible, puis `soiree upgrade` ; le code
signalé par les nouveaux contrôles reste à adapter.

Les contrôles possèdent leurs propres cas négatifs : une faute injectée doit
faire échouer la vraie commande et sa CI. Un contrôle non exécuté ou déclaré
non applicable n’est jamais présenté comme une propriété vérifiée.

Il lance aussi `cargo fmt --check`, Clippy en natif et en WASM avec `-D warnings`, et `cargo test` ; `soiree check --fast` les saute, pour la boucle serrée. Le reste (états impossibles, sensation, validité d'une raison) demande toujours un jugement.

## Vérification

C'est le vrai goulot. Tu dois savoir que ton jeu marche sans que l'hôte ait à y jouer à ta place.

### La portée d'une garantie

Une garantie associe une propriété précise, un domaine d'application et une justification. Sa portée dépend de plusieurs questions distinctes :

- **La propriété** : ce qui est affirmé et ce qui constituerait une violation.
- **L'application** : tous les jeux, ou seulement ceux qui possèdent certaines mécaniques : réseau, physique, votes, sauvegarde…
- **La responsabilité** : ce qui est fourni par le commun, ce qui dépend du jeu et de son intégration, ce qui relève des intentions de l'auteur.
- **La justification** : types, preuve formelle, tests, mesures ou raisonnement explicite.
- **Les hypothèses et les limites** : les conditions nécessaires, les versions et configurations concernées, les cas hors du périmètre établi.

« Obligatoire », « commun à tous les jeux » et « prouvé » disent des choses différentes. Une exigence commune peut être éprouvée par des tests ; une règle propre à un jeu peut être formellement prouvée. La reconnexion concerne les jeux avec des appareils connectés ; l'annulation concerne ceux qui proposent un historique. Leur absence dans un autre jeu ne constitue pas une défaillance.

Le déterminisme illustre l'importance du périmètre. Il peut être un principe commun pour l'état logique : même version, même contenu, même état initial, même état du générateur aléatoire et même suite ordonnée d'entrées donnent le même état. Une graine seule ne suffit pas : le temps et les résultats extérieurs peuvent aussi influencer les règles. L'identité entre natif et WASM est une propriété supplémentaire ; celle des pixels et des délais physiques du navigateur en est encore une autre.

### Des moyens de vérification complémentaires

| Moyen | Ce qu'il apporte | Ce qui reste à justifier |
|---|---|---|
| Types et API | Certaines erreurs de représentation ou d'usage deviennent impossibles à exprimer. | Les propriétés réellement encodées, la validation aux frontières et le passage par ces constructions. |
| Preuve formelle | Une propriété est établie sur du code ou un modèle, pour les cas de son périmètre et sous ses hypothèses. | Le sens de la spécification, la validité des hypothèses et le lien avec le code effectivement utilisé. |
| Tests, simulations et mesures | Des comportements sont exercés, des contre-exemples recherchés et l'intégration confrontée au réel. | Les cas non explorés et la capacité des contrôles à reconnaître une erreur. |
| Explicitation des choix | Les alternatives, les exclusions et le raisonnement deviennent examinables ; les oublis sont plus faciles à repérer. | La justesse des arguments et les éléments qui les soutiennent. |

Ces moyens peuvent se renforcer. Une API peut faire respecter une précondition dont dépend une preuve ; le harnais peut éprouver l'intégration du composant. Une garantie du noyau ne suffit pas à établir que la page affiche le bon résultat. Inversement, observer le jeu en boîte noire ne démontre pas la correction de toutes ses transitions internes.

La preuve formelle trouve sa place dans cet ensemble. La machine peut contrôler une preuve sans que l'hôte relise toute l'implémentation. Il reste une confiance dans l'outillage, dans les hypothèses retenues et dans la correspondance entre la spécification et le jeu voulu. Une fonction prouvée sous une précondition n'apporte sa garantie à l'appelant que si cette condition est respectée. Des portions supposées correctes ou exclues de la preuve limitent sa portée. L'utilisation d'un vérificateur ne constitue donc pas, à elle seule, une garantie globale sur un jeu.

Le commun peut fournir des garanties réutilisables à travers des interfaces précises. Les règles originales et leur intégration appartiennent à chaque jeu. Un outil peut vérifier une propriété exprimée ; il ne peut pas déduire du seul code que cette propriété correspond à l'intention de l'auteur.

L'explicitation répond à une autre source d'erreurs : les décisions prises sans examiner leurs conséquences ou les cas oubliés. Une justification reliée à des références, des exemples et des vérifications rend ces erreurs plus détectables. Elle peut néanmoins être convaincante et fausse. De même, la présence d'un test ne suffit pas à établir qu'il s'exécute ou qu'il détecte le défaut annoncé. La confiance dépend de ce que les arguments et les contrôles permettent effectivement d'établir.

### Exigences, déclarations et résultats

Trois éléments interviennent dans une vérification :

- **L'exigence** définit la propriété attendue, ses conditions d'application et le niveau de justification demandé.
- **La déclaration du jeu** décrit les mécaniques présentes, les choix effectués et les moyens proposés pour répondre à l'exigence.
- **Le résultat** établit ce qui a réellement été exécuté ou démontré, sur quelle version et avec quelles limites.

La conformité d'une déclaration n'établit pas la correction du logiciel. Écrire « prouvé » ne produit pas une preuve ; une propriété testée reste testée sur le domaine exercé. Une exigence non applicable, un contrôle pas encore exécuté, un échec du logiciel et une panne du moyen de vérification sont des situations différentes.

La confiance repose sur le lien entre ces trois éléments : une affirmation compréhensible, des arguments et des vérifications qui la concernent réellement, et des hypothèses identifiables. Les journaux, les replays, les simulations, les mutations et les essais sur les appareils apportent chacun des éléments à cette justification. Les preuves formelles peuvent renforcer certaines propriétés de cet ensemble.

### Les moyens de vérification

- **Bots et entrées scriptées** : le jeu peut se jouer tout seul avec N joueurs simulés, sans écran et en accéléré.
- **Replays** : chaque partie enregistre les entrées à leur arrivée dans la simulation ; une commande rejoue et compare l’état. Chaque bug trouvé devient un fichier de replay sans données personnelles, puis un test. Les journaux réels restent locaux, ignorés par Git et supprimés après trente jours. Une panne du journal ne bloque jamais la partie.
- **Simulation de l’hôte entier** : le jeu et les vrais clients salon tournent sur la même boucle que l’hôte, avec une horloge et un réseau simulés à graine. Une campagne plus longue reste disponible à la demande.
- **Mutations des tests** : `soiree mutants` vérifie à la demande que les tests voient des bugs injectés. Cette commande reste hors de `check` et de `certify`.
- **Captures** : ouvre le jeu dans un vrai navigateur, joue, prends des captures d'écran et compare-les aux meilleurs jeux du genre.
- **Faux téléphones et fausses manettes** : un mode qui simule 8 télécommandes depuis un seul ordinateur.
- **Overlay de diagnostic** : images par seconde, temps par image, latence d'entrée par joueur, état du réseau. Il s'active d'une seule touche.
- **Une vraie session de jeu** avant de déclarer le jeu fini : lance-le, rejoins avec un téléphone, et joue une partie entière.
- **La certification** : `CERTIFICATION.md` liste ce que chaque jeu doit tenir le soir même (reconnexions, entrées hostiles, textes vicieux, déroulement, endurance). Ses tests de propriétés tournent dans `cargo test`. Son harnais, `soiree certify` dans le dossier du jeu, attaque le jeu en boîte noire à partir de son `cert.json`.

## Le soir même

Le seul contrat avec la soirée :

- Une seule commande lance le jeu depuis le repo. Documente-la dans le README.
- Le jeu s'ouvre à une adresse locale, en plein écran sur la TV, et affiche comment le rejoindre.
- On peut relancer une partie sans rien redémarrer.

## Matériel

- **Manettes** : *à venir, l'hôte donnera la référence.*
- **Écran** : *à confirmer (résolution, fréquence).*
- **Ordinateur hôte** : *à confirmer.*
- **Réseau** : Wi-Fi local ; les références du routeur et les mesures sur le matériel de la soirée restent à renseigner.

## Télécommande native (iPhone)

L’app iPhone commune existe dans `remote/`. Son contrat, ses capacités et ses exemples sont décrits dans `remote/CONTRAT.md`. Chaque jeu conserve une page téléphone jouable dans un navigateur. Les modifications du protocole de l’app se coordonnent avant la soirée.

## Ce que ce document n'impose pas

En dehors de l'outillage commun : ni moteur, ni architecture, ni crate. Deux jeux qui suivent ce document peuvent n'avoir aucune ligne de code en commun.
