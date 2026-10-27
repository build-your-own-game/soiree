# Manifeste : jeux de soirée en Rust, repensés de zéro

> Ce fichier s'adresse à l'agent de code (Claude Code, Codex…) qui code ton jeu. L'outil `soiree` l'écrit dans `.soiree/MANIFESTE.md` de chaque jeu, à la version de son `soiree.toml`, et l'`AGENTS.md` du jeu dit de le lire et de l'appliquer.

## Le pari

On est plusieurs amis. Chacun fait coder un jeu par son agent de code. Un soir, on y joue tous ensemble chez l'hôte, sur la TV.

Le pari : écrire du code ne coûte presque plus rien. Les choix d'ingénierie que l'industrie évite parce qu'ils demandent trop de travail humain redeviennent possibles. Ce document ne te donne pas de stack. Il te donne les contraintes réelles, une méthode pour extraire la sagesse du code existant, et un niveau d'exigence.

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
- Le réseau n'est pas encore tranché : Wi-Fi local, internet, ou les deux (voir *Matériel*). Un jeu peut aussi utiliser internet en direct comme mécanique de jeu.
- La TV, les téléphones, l'app iPhone et les manettes rejoignent le jeu par la couche commune `salon/`, qui gère tout le réseau : lis `salon/README.md`.

### Qui joue, et avec quoi

- De 2 à N joueurs dans la même pièce, devant une TV partagée.
- Entrées disponibles :
  - le clavier et la souris de l'ordinateur hôte ;
  - des manettes, via la Gamepad API. Le modèle exact sera donné dans *Matériel* ; d'ici là, code contre le mapping `standard` ;
  - les téléphones, comme télécommandes. N'hésite pas à t'en servir : écran tactile, saisie de texte, gyroscope, vibration, et un écran privé par joueur.
- Un joueur rejoint la partie en moins de 30 secondes, sans créer de compte.

### Télécommande téléphone

Deux voies possibles :

1. **Une page web** servie par ton jeu et ouverte via un QR code affiché sur la TV. Elle est disponible tout de suite, et c'est à toi de la coder.
2. **L'app iPhone native de l'hôte** (Android plus tard). Son protocole reste à définir dans *Télécommande native*. Tant qu'elle n'existe pas, utilise la voie 1.

## La méthode : extraire le signal du code existant

C'est la partie la plus importante de ce document.

Une librairie utilisée depuis des années (moteur de rendu, moteur de jeu, framework multijoueur) est une archive de **pression d'adaptation**. Chaque commit, chaque PR, chaque issue, chaque changelog garde la trace d'un moment où la réalité a contredit le design initial : un bug sur un GPU précis, des joueurs qui trouvaient le saut mou, un réseau qui perdait des paquets, une API que personne n'arrivait à utiliser.

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

Écris-le dans une section `## Écarts au manifeste` de ton `LEARNINGS.md`, avec un point numéroté par principe ci-dessous. Chaque point commence par **Respecté.**, par **Écart.** suivi d'une « Raison : », ou par **À faire.**

1. **La machine vérifie, pas l'humain.** Préfère les choix qui laissent le compilateur, les tests et les replays attraper les erreurs : types forts, états impossibles non représentables, aucun `unwrap` dans la boucle de jeu sans raison écrite.
2. **Simulation à pas fixe, rendu interpolé.** La logique avance par ticks fixes et le rendu interpole entre eux. C'est ce qui rend la sensation de jeu indépendante de la fréquence de l'écran.
3. **Déterminisme par défaut.** Une partie, c'est une graine, un état initial et un flux d'entrées horodatées. Tu obtiens gratuitement les replays, la reproduction de bugs, le rollback et les tests. Attention aux flottants entre WASM et natif, à l'ordre d'itération des `HashMap`, et à toute source de hasard sans graine.
4. **Le jeu tourne sans écran.** La simulation compile en natif sans GPU ni navigateur, pour les tests, les bots et le serveur.
5. **Pas de compromis par habitude.** Tu contrôles le serveur local : tu peux envoyer les en-têtes COOP/COEP et utiliser les threads WASM (`SharedArrayBuffer`). Les joueurs ont tous lancé le jeu exprès : un temps de chargement compte beaucoup moins que sur le web public. Vise WebGPU directement.
6. **Le réseau suit la physique.** Quel que soit le chemin (Wi-Fi local ou internet), mesure sa latence réelle et conçois le jeu en fonction. Envoie les entrées sur un canal non fiable et non ordonné (WebRTC DataChannel) plutôt qu'en TCP. Garde un état faisant autorité sur une seule machine. Jette les entrées périmées au lieu de les rejouer.
7. **Des budgets mesurés, pas supposés.** Points de départ, à ajuster selon le genre :
   - au moins 60 images par seconde (120 si l'écran le permet), et aucune image au-delà de 33 ms en jeu ;
   - de l'entrée au pixel : moins de 50 ms depuis une manette ou un clavier, moins de 80 ms depuis un téléphone en Wi-Fi local ;
   - moins de 30 s entre le scan du QR code et le premier mouvement.

   Mesure-les dans le jeu, avec un overlay de debug, et note les résultats.

## Outillage commun

Tous les jeux utilisent la même version de Rust, pour que n'importe lequel compile sur la machine de l'hôte sans surprise. Rust stable est rétrocompatible : la dernière version stable est aussi la plus mûre. On la fige jusqu'à la soirée.

Ces règles sont un socle. Tu peux les durcir librement. Pour en assouplir une, il faut une raison écrite, à l'endroit précis : `#[expect(lint, reason = "...")]`, jamais `#[allow]`.

Les fichiers eux-mêmes sont dans `outil/regles/` du dépôt commun, embarqués dans l'outil `soiree` de chaque version. `soiree new` et `soiree upgrade` les écrivent dans le jeu ; `soiree check` vérifie qu'ils y sont. Ce qu'ils imposent, et pourquoi :

- **`rust-toolchain.toml`** fige la version de Rust, avec `rustfmt`, `clippy` et la cible `wasm32-unknown-unknown`.
- **`Cargo.toml`** (dans un workspace, sous `[workspace.lints]`, chaque crate reprenant `[lints] workspace = true`) :
  - l'édition 2024 ;
  - pas de code `unsafe` ;
  - Clippy `all` et `pedantic` en erreur, sauf quatre lints qui ne sont que du bruit dans du code de jeu (conversions entier → flottant, documentation d'API publique) ;
  - aucune exception muette : `#[allow]` est interdit, `#[expect]` doit donner sa raison ;
  - ni `unwrap`, ni `expect`, ni `panic!`, ni index hors bornes, ni `dbg!`, ni `todo!` : chacun arrête la partie ;
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

Il lance aussi `cargo fmt --check`, Clippy en natif et en WASM avec `-D warnings`, et `cargo test` ; `soiree check --fast` les saute, pour la boucle serrée. Le reste (états impossibles, sensation, validité d'une raison) demande toujours un jugement.

## Vérification

C'est le vrai goulot. Tu dois savoir que ton jeu marche sans que l'hôte ait à y jouer à ta place.

- **Bots et entrées scriptées** : le jeu peut se jouer tout seul avec N joueurs simulés, sans écran et en accéléré.
- **Replays** : chaque bug trouvé devient un fichier de replay, puis un test.
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
- **Réseau** : *à décider : Wi-Fi local, éventuellement avec du matériel dédié, ou passage par internet.*

## Télécommande native (iPhone)

*À définir.* L'hôte prévoit une app iPhone qui servira de télécommande pour tous les jeux. Ce qu'elle exposera, et la manière dont ton jeu s'y connectera, seront décrits ici. D'ici là, fais ta télécommande en page web.

## Ce que ce document n'impose pas

En dehors de l'outillage commun : ni moteur, ni architecture, ni crate. Deux jeux qui suivent ce document peuvent n'avoir aucune ligne de code en commun.
