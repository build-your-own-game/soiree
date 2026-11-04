# Contrat du commun

> Ce document traduit le [manifeste](MANIFESTE.md) en règles. Il s'adresse aux agents qui écrivent un jeu ou le commun, et aux humains qui les relisent. Il est versionné avec le commun : un jeu suit le contrat de la version qu'il a choisie.

Ce contrat décrit le commun que vise le manifeste. Le code actuel n'en suit qu'une partie. Ce que l'épreuve contrôle aujourd'hui est décrit dans [`CERTIFICATION.md`](CERTIFICATION.md), et la portée exacte d'un résultat dans [`VERIFICATION.md`](VERIFICATION.md). Ce contrat a vocation à les absorber.

## Le cadre technique

- Le commun et tous les jeux utilisent la même version de Rust, figée par `rust-toolchain.toml`, en édition 2024.
- **L'enfermement.** Ce qu'on joue tourne enfermé : on constate, sur ce qui est exécuté, à quoi il a accès ; sa mémoire entière est son état ; on peut l'arrêter à tout instant sans dégât. Aujourd'hui, seul WebAssembly le permet sur toutes nos cibles : c'est le module WebAssembly qui passe l'épreuve et que l'on joue. Un autre moyen qui garantirait la même chose pourrait le remplacer.
- Pendant le développement, un agent peut compiler un jeu en natif pour aller plus vite. Cette copie n'a pas les garanties de l'enfermement : elle doit donner les mêmes résultats que le module, et l'épreuve les compare.
- **Le navigateur.** Les jeux tournent dans un navigateur, y compris la partie qui fait foi, sur l'ordinateur de la soirée. Le navigateur est une cible, pas une dépendance : le même module tourne aussi sans écran pour les tests, et pourrait tourner dans un programme natif le jour où un jeu en aurait besoin.
- **Le relais.** Une page ne peut pas accueillir de connexions : un relais met les appareils en relation. C'est un petit programme sur l'ordinateur de la soirée, ou un service sur internet, avec la même interface (voir « Relier les appareils »).
- **Les appareils.** Un joueur n'a besoin que d'un navigateur. Une app peut le remplacer sur un appareil quand elle apporte quelque chose ; un jeu reste toujours jouable sans elle.
- On suppose une machine équipée d'un GPU. Une machine de test qui n'en a pas utilise un GPU simulé en logiciel : il n'existe pas de seconde façon de dessiner.
- Le JavaScript se limite à la colle générée et au strict nécessaire pour atteindre une API du navigateur.

## La forme d'un jeu

Un jeu reçoit des entrées, et rend son nouvel état, ce qu'il montre et ce qu'il demande.

- **Les entrées** sont les actions des joueurs, le temps, le hasard et les réponses à ses demandes. Elles arrivent une à une, dans l'ordre que fixe le commun : celui où les gestes ont été faits, estimé, et non celui où ils sont arrivés. Un geste en retard peut être replacé dans le passé récent, tant que ce passé n'est pas devenu définitif (voir « Réagir sans attendre »).
- **L'état** est tout ce que le jeu possède. La mémoire entière de son module compte comme son état : le commun peut la copier, la restaurer et la comparer. Ce qu'un appareil n'a pas le droit de savoir y est marqué secret, dans les types.
- **Ce qu'il montre** est une vue par appareil, calculée depuis l'état, avec ce qu'il fait entendre. Un effet ponctuel, comme un son ou une vibration, porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas. Il porte aussi l'instant où il doit se produire, pour que le commun compense le retard de l'appareil qui le joue.
- **Ce qu'il demande**, comme une requête sur internet, est exécuté par le commun, et la réponse revient comme une entrée.
- **Un refus ne change rien :** le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte l'entrée.
- **Le temps** avance par pas réguliers, fournis par le commun. Pour rester fluide quelle que soit la fréquence de l'écran, l'affichage glisse entre deux pas, ou prédit le suivant. Glisser coûte jusqu'à un pas de retard ; prédire peut se tromper. Le choix dépend du genre. Un jeu au tour par tour n'a rien de plus à faire.
- **Ce qui doit réagir sans attendre**, comme le personnage d'un FPS ou une carte qu'on glisse sur un téléphone, est prédit par le commun, sur l'appareil (voir « Réagir sans attendre »).
- **Un jeu fournit de quoi se jouer seul :** des bots qui avancent dans une vraie partie, pour que les machines puissent la parcourir.
- **Chaque pas a un budget.** Un jeu qui ne rend pas la main est interrompu, et la partie reprend.
- **Une entrée qui fait échouer le jeu est isolée et écartée**, pour que la reprise ne rejoue pas la même faute en boucle.
- **Une sauvegarde d'une version antérieure** passe par une conversion explicite.

## Ce que le commun possède

| Domaine | Ce que fait le commun | Ce que le jeu en voit |
|---|---|---|
| Temps et hasard | L'horloge, la graine, les pas réguliers, l'accord des horloges entre appareils | Des entrées |
| Réseau | Relie les appareils par le relais, local ou en ligne, mesure la latence réelle et choisit le meilleur canal disponible. Un état périmé est remplacé par le plus récent ; un événement compte exactement une fois. La partie qui fait foi vit sur une seule machine. | Des appareils qui arrivent, partent et reviennent |
| Prédiction | L'état confirmé, le rejeu des gestes en attente, le lissage des corrections | Les gestes qu'il déclare prédictibles |
| Appareils | Le contrat d'un appareil, et les capacités que chacun déclare | Des capacités, jamais une technologie |
| Identité | Une identité stable par appareil, des prénoms validés | Un identifiant que seul le commun peut créer |
| Entrées | Clavier, souris, manettes, téléphones : un par joueur, un par équipe, ou un seul pour la partie | Des entrées typées |
| Affichage | Un langage de dessin portable : texte, images, 3D. Un jeu qui en a besoin peut parler directement au GPU, de façon explicite. | Ce qu'il décrit |
| Interaction riche | Champs de texte, éléments déplaçables, sélection, presse-papiers. Le commun peut s'appuyer sur le navigateur aujourd'hui et sur son propre code demain, sans que le jeu change. | Des composants |
| Son | Le déblocage, les appareils, et la programmation à l'instant voulu, en compensant leur retard | Des effets décrits, avec leur instant |
| Sauvegardes | Il conserve l'état | Rien à écrire |
| Reprise | Une seule façon de repartir : un appareil retrouve sa place, le commun reprend la partie | Rien |

**Le GPU.** L'information va de l'état vers l'image, jamais dans l'autre sens : le jeu ne relit rien du GPU pour décider. Un bug côté GPU peut donner une image fausse, jamais un score faux. Si le GPU disparaît, le commun reconstruit l'image depuis l'état. Les données envoyées au GPU ont une seule définition, partagée avec ses programmes.

## Relier les appareils

Trois rôles se partagent une partie :

- **La partie qui fait foi** est un rôle, pas une machine. Aujourd'hui, elle tourne dans le navigateur de l'ordinateur de la soirée ; tout appareil capable de faire tourner le module pourrait la tenir.
- **Les appareils** affichent, envoient des gestes et prédisent.
- **Le relais** les met en relation, et rien d'autre.

Le relais ne comprend rien au jeu : il fait passer des octets qu'il ne sait pas lire. Il existe sous deux formes, avec exactement la même interface, et le commun choisit seul :

| Situation | Ce qui se passe |
|---|---|
| Le programme local tourne | Tout passe par lui, sans internet |
| Pas de programme, internet disponible | Le service en ligne met les appareils en relation, puis ils se parlent directement sur le Wi-Fi |
| Le Wi-Fi isole les appareils | Le service en ligne fait passer les messages, avec la latence d'internet |
| Ni programme ni internet | La partie ne peut pas commencer, et le commun le dit clairement |

- **Le relais ne peut ni lire ni modifier la partie.** La télé affiche un QR code qui porte une clé, et tout est chiffré de bout en bout. Un service sur internet n'a pas à être digne de confiance.
- **Le relais reste générique.** Il ne sert qu'une page de démarrage, la même pour tous les jeux. Le module du jeu arrive ensuite par la connexion, depuis la partie qui fait foi : un appareil a toujours exactement sa version.
- **La reprise survit à un onglet qui se recharge.** Un état récent et les entrées qui l'ont suivi sont gardés hors de l'onglet, chiffrés, par exemple par le relais. Un rechargement repart de cet état et rejoue les entrées.
- **La partie qui fait foi continue de tourner** quand son onglet n'est plus au premier plan. Le commun le garantit, ou le signale, et l'épreuve le vérifie sur chaque navigateur visé.

### Le contrat d'un appareil

Un appareil reçoit sa projection et sa vue, envoie des gestes typés, exécute l'étape pour prédire, joue les effets à l'instant voulu et mesure sa latence. Il déclare ses capacités, comme la vibration, les capteurs de mouvement, le son sans geste préalable ou l'écran maintenu allumé.

Une page dans un navigateur et une app remplissent ce contrat en entier ; un clavier ou une manette n'en remplissent que la part des gestes. Tous passent les mêmes pressions. Le jeu ne voit que des capacités, jamais une technologie. Une capacité optionnelle enrichit un jeu, mais il reste jouable avec un simple navigateur.

## Réagir sans attendre

Un geste fait sur un téléphone met des dizaines de millisecondes à atteindre l'ordinateur de la soirée, puis à revenir. Sous le doigt, ce retard se sent. Le commun le cache en prédisant : l'appareil applique le geste tout de suite, puis l'ordinateur de la soirée le confirme ou le corrige. Le même mécanisme permet à l'ordinateur de la soirée de prédire un joueur distant, comme un téléphone qui sert de volant.

- **Une seule partie fait foi**, sur l'ordinateur de la soirée. Elle reçoit des gestes, jamais des prédictions : une prédiction ne décide rien.
- **Le passé récent est provisoire.** Pour rester juste, l'ordinateur de la soirée replace un geste en retard à son moment, dans une fenêtre bornée ; au-delà, le pas est définitif. Un résultat, comme un score ou une élimination, ne s'annonce qu'une fois définitif, sur la télé comme sur les téléphones.
- **Chaque appareil ne reçoit que ce qu'il a le droit de savoir.** Un secret de l'état contient sa valeur sur l'ordinateur de la soirée, et il est vide ailleurs. Le commun envoie à chaque appareil cette projection, et sa vue se calcule depuis elle : un appareil ne peut pas montrer un secret qu'il n'a pas reçu.
- **Prédire, c'est exécuter la même étape**, avec le même module, sur la projection. Quand l'étape a besoin d'un secret absent, elle le dit, et ce morceau attend la confirmation. Un jeu sans secret prédit tout, sans rien écrire de plus.
- **Le jeu choisit ce qui se prédit, geste par geste.** Par défaut, rien ne l'est. Seules les conséquences du geste de l'appareil s'affichent avant confirmation, et un effet prédit à tort s'annule.
- **Le commun possède le mécanisme :** l'état confirmé, le rejeu des gestes en attente, le lissage des corrections et la limite du rejeu.
- **La prédiction peut toujours être coupée.** Sans elle, le jeu reste correct, seulement moins réactif. C'est la voie de secours d'un appareil lent, suspendu ou dont la copie a échoué, et elle passe par la reprise habituelle.

Une prédiction fausse n'est pas un bug : c'est le fonctionnement normal. Ce qui compte, c'est l'effet des corrections sur la sensation. Les machines mesurent leur fréquence ; les joueurs jugent.

## Ce que le réel impose

Certaines vérités ne dépendent pas du code, mais de la physique, du matériel et des humains. Un code parfait ne les change pas. Le commun les connaît, les compense quand il le peut, et les reproduit fidèlement dans ses pressions : une garantie éprouvée sur un réseau idéal porte sur un monde qui n'existe pas. Les ordres de grandeur ci-dessous restent à mesurer et à sourcer.

- **Il n'existe pas de « maintenant » partagé.** Chaque appareil a son horloge, qui dérive. Le commun estime l'écart en continu, par des allers-retours, avec une incertitude d'environ la moitié d'un aller-retour. Les appareils partagent des numéros de pas, jamais une heure. Un appareil ne peut pas prétendre avoir agi plus tôt que la fenêtre ne le permet.
- **Le Wi-Fi d'un téléphone arrive par rafales.** Pour économiser sa batterie, un téléphone regroupe les messages, parfois une centaine de millisecondes d'un coup. Le réseau perd, retarde, double et réordonne.
- **Le canal idéal n'existe pas partout.** WebRTC permet un canal non fiable et non ordonné, mais un pare-feu ou un réseau peut le bloquer sans prévenir. Le commun se replie alors sur WebSocket, où un paquet perdu retient tous les suivants. Il choisit le meilleur canal disponible, et un jeu reste correct sur le pire.
- **Un état et un événement ne vieillissent pas pareil.** Un état périmé, comme la position d'un stick, est remplacé par le plus récent. Un événement, comme un vote, compte exactement une fois, même en retard.
- **Les appareils ajoutent leur propre retard.** Un écran tactile, une télé hors de son mode jeu ou une enceinte Bluetooth peut ajouter des dizaines de millisecondes, parfois plus de cent. Le son se programme en avance pour rester avec l'image.
- **Le navigateur du téléphone impose ses règles :** pas de son avant un premier geste, une page suspendue quand l'écran se verrouille, des fonctions absentes sur certains appareils, comme la vibration dans Safari sur iPhone.
- **Certains réseaux isolent les appareils**, comme un Wi-Fi d'invités : les téléphones n'atteignent pas l'ordinateur de la soirée. Il faut alors passer par le service en ligne.
- **Les joueurs regardent la télé.** Une page téléphone doit pouvoir s'utiliser sans la regarder.
- **La perception a des seuils.** Ce sont eux qui fixent les budgets, pas ce qui est facile à atteindre.

Deux sortes de garanties en découlent. Celles qui portent sur la logique sont absolues : jamais de score faux, jamais de partie figée. Celles qui portent sur la latence sont statistiques, et une part dépend du matériel : on les mesure, on sépare notre part de la sienne, et on règle le matériel avant de jouer.

Une coupure ne se promet pas impossible. Elle se rattrape seule, sans que personne ait rien à faire, et sans rien perdre.

Les leçons de l'ancien salon, dans [`lecons/sources/salon.md`](lecons/sources/salon.md), viennent de ce réel. Reconstruire le code ne veut pas dire les perdre.

## La page téléphone

Chaque jeu code la page de téléphone qu'il veut. Elle reçoit ce qu'elle doit montrer, prédit ou confirmé, et renvoie des actions. Elle ne détient pas l'état du jeu, ne prédit rien elle-même et ne gère pas la connexion : la projection et la prédiction appartiennent au commun, sur l'appareil. Ce qu'elle échange avec le jeu est typé ; son intérieur lui appartient. Ses champs de texte restent de vrais éléments du navigateur, pour garder le clavier du téléphone.

Une page possède ses durées de vie. Ses écouteurs, minuteurs et boucles d'animation appartiennent à une vue et s'arrêtent avec elle. Une réponse asynchrone appartient à l'opération qui l'a demandée : elle ne modifie rien si cette opération a été annulée, remplacée ou détruite. Un élément en cours d'édition garde son identité, sa sélection et sa composition tant que son objet existe.

## Les lois communes

Chaque loi vaut pour tous les jeux. Son niveau dit comment elle est justifiée aujourd'hui, et peut monter avec le temps.

| Loi | Niveau exigé | Peut devenir |
|---|---|---|
| Le jeu n'atteint le monde que par le commun | Forme : le module est enfermé, et le commun constate ce à quoi il a accès | |
| Sûreté mémoire | Forme : Rust sans `unsafe`, dans un module enfermé | |
| Déterminisme | Forme et constat : deux exécutions sont comparées, et la copie native avec le module | |
| La prédiction ne décide jamais | Forme : la partie qui fait foi ne reçoit que des gestes | |
| Un secret ne quitte pas l'ordinateur de la soirée | Forme : des types | |
| Le relais ne lit ni ne modifie la partie | Forme : un chiffrement de bout en bout | |
| Une partie a la même histoire définitive, avec ou sans prédiction | Pression | Preuve |
| Un refus ne change rien | Forme | |
| Deux morceaux qui s'accordent n'ont qu'une définition | Forme : le compilateur | |
| Ce qui vient de l'extérieur est validé à l'entrée | Forme : des types | |
| Le jeu fournit ses bots et ses invariants | Demande dans le code | |
| Ne jamais planter la partie | Pression et reprise | Preuve |
| Toujours rendre la main | Budget par pas | Preuve de terminaison |
| Mémoire bornée | Mesure | Preuve |
| Budgets d'image et de latence | Mesure statistique | |

Le commun permet dès maintenant d'exiger une preuve : il écrit l'énoncé dans l'interface, et le jeu apporte la preuve que son code le respecte. Le premier essai porte sur « ne jamais planter la partie », sur un seul jeu.

## Les règles d'écriture

Ces règles sont un socle. On peut les durcir librement. Pour en assouplir une, il faut une raison écrite à l'endroit précis : `#[expect(lint, reason = "...")]`, jamais `#[allow]`. Les fichiers sont dans `outil/regles/`, embarqués dans l'outil `soiree`.

- Pas de code `unsafe`.
- Clippy `all` et `pedantic` en erreur, sauf trois lints de documentation d'API publique.
- Ni `unwrap`, ni `expect`, ni `panic!`, ni index hors bornes, ni `dbg!`, ni `todo!` : chacun arrête la partie.
- Ni `_ =>` sur une enum : le variant ajouté demain doit casser la compilation là où il faut le traiter.
- Ni paramètre `bool` : deux booléens s'inversent sans que le compilateur le voie.
- Ni arithmétique entière qui peut déborder : on choisit `checked_`, `saturating_` ou `wrapping_` selon ce que veut dire le débordement. Le dépassement est vérifié même en release.
- Ni conversion d'entier en flottant qui perd des chiffres sans le dire.
- Ni valeur jetée sans son type : `let _: T = …`, jamais `.ok()` pour faire taire un `Result`.
- Ni `HashMap` ni `HashSet`, dont l'ordre d'itération diffère entre natif et WebAssembly.
- Ni `Instant::now` ni `SystemTime::now` : le temps entre comme une entrée. Seul le module du commun qui possède l'horloge les lit.
- Ni les fonctions mathématiques des flottants dont le résultat diffère entre WebAssembly et natif (`sin`, `cos`, `powf`…) : on utilise `libm`.
- Les dépendances sont optimisées même en debug, sans quoi un jeu est injouable.

Les règles sur `HashMap`, l'horloge et `libm` ne garantissent pas le déterminisme : dans le module, l'enfermement le garantit. Elles font que la copie native donne les mêmes résultats que le module, et elles signalent l'erreur dès l'écriture.

Les tests ont droit à `unwrap`, `expect`, `panic!` et aux index. Le code de rendu peut s'exempter des règles de déterminisme et d'arithmétique, en tête de crate ou de module, avec sa raison.

## Les types de bout en bout

Deux morceaux qui doivent s'accorder n'ont qu'une seule définition. Si un agent renomme d'un côté, l'autre cesse de compiler.

- Les messages entre l'ordinateur de la soirée et les autres appareils sont des types Rust partagés, jamais du texte écrit à la main.
- Les secrets de l'état sont marqués dans ses types : la projection envoyée à un appareil se calcule, elle ne s'écrit pas à la main.
- La version d'un jeu se calcule depuis ses sources. Une page restée en cache après un changement est refusée, au lieu de décoder de travers.
- Les données envoyées au GPU partagent leur définition avec ses programmes.
- Les fichiers de contenu sont validés à leur chargement et vérifiés par un test.
- Le contrat entre une page et l'app iPhone a une définition Rust unique, dont les liaisons Swift sont générées.
- Une donnée d'une version antérieure passe par une conversion explicite.

Les types protègent la forme des données, pas leur sens. Le vocabulaire du navigateur lui-même, comme les noms d'événements ou de touches, reste du texte : ce n'est pas un contrat entre nos fichiers.

## Le code tiers

Le code tiers est permis. Il se choisit consciemment, et sa raison s'écrit à côté de la dépendance. Une crate qui encode déjà les leçons du réel, comme `wgpu` pour le GPU ou `rapier` pour la physique, est un choix légitime. Nous cherchons à réduire ce code avec le temps.

Une bibliothèque mûre peut aussi servir de juge sans être livrée : on compare notre code au sien dans les tests, et un agent corrige jusqu'à ce qu'ils s'accordent.

## Chercher les leçons

Pour chaque sous-système important, comme le rendu, la physique, le réseau, les entrées, l'audio, l'animation, l'interface ou la sensation de jeu :

1. Choisir deux à quatre références sérieuses, si possible écrites dans des langages et à des époques différents.
2. Lire leur code **et** leur historique : les fichiers clés, les pull requests les plus discutées, les issues fermées avec beaucoup de commentaires, les changelogs, les postmortems et les talks des auteurs.
3. Écrire chaque leçon avec sa source et son origine.
4. Reconstruire pour notre cas, en gardant les leçons universelles et rien d'autre.

Toute source qui informe du réel compte aussi : une partie jouée, une vidéo de joueurs, nos propres pannes. Les jeux existants sont une source pour les mécaniques et la sensation ; nous créons nos propres assets.

| Origine de la leçon | Exemples | Que faire |
|---|---|---|
| Physique, matériel, perception humaine | Interpolation entre pas fixes, compensation de latence, sensation du saut | Garder |
| Usage réel | Reconnexion après une coupure, audio débloqué au premier geste, manette qui se déconnecte | Garder |
| Accident d'un langage ou d'un runtime | Contournements du ramasse-miettes de JavaScript, tout en un seul fil | Jeter, après avoir vérifié qu'il n'existe pas d'équivalent en Rust |
| Héritage et compatibilité | Vieux navigateurs, API gelée pour des milliers d'utilisateurs | Jeter : nous n'avons qu'un cas d'usage |

Points de départ :

- **Rendu :** wgpu, Bevy, Godot, three.js, Filament.
- **Moteurs complets :** Bevy, Godot, Phaser, id Tech.
- **Physique :** Rapier, Box2D, Jolt.
- **Réseau :** les leçons de l'ancien salon et de l'ancienne app iPhone, dans [`lecons/sources/salon.md`](lecons/sources/salon.md) et [`lecons/sources/remote.md`](lecons/sources/remote.md), le netcode de Quake 3, la documentation réseau du Source Engine, GGPO, Colyseus, les articles de Glenn Fiedler.
- **Entrées :** SDL et sa base de manettes, la Gamepad API, Steam Input.
- **Sensation de jeu :** les écrits de Maddy Thorson sur Celeste, le talk *Juice it or lose it*, le livre *Game Feel* de Steve Swink.
- **Simulation de pannes et vérification :** les outils qui éprouvent le code des autres, avec leurs scénarios, leurs garanties et leurs limites.

## Les leçons et leur évolution

Une leçon a la même forme à tous les étages : ce qui arrive, d'où on le sait, les mécaniques concernées, la question qu'elle pose, et la pression qui la vérifie si elle existe.

| Étage | Exemple | Comment elle s'applique |
|---|---|---|
| Commun | Un refus ne change rien ; le jeu survit à une coupure | Tous les jeux, à l'épreuve |
| Mécanique | Avec de la physique : personne ne sort de l'arène, aucune valeur absurde | Les jeux qui déclarent cette mécanique |
| Jeu | Un appui donne un seul saut | Un invariant de ce jeu |
| Connaissance | Ce qui rend un saut agréable dans les bons jeux | L'agent la lit et l'applique, sans test |

Une leçon monte d'un étage quand elle revient. Un agent la propose par une pull request sur le dépôt du commun, avec sa source et sa pression éventuelle. Le mainteneur du commun l'accepte ou la refuse, et elle entre dans la version suivante. Une leçon devient une loi par sa décision. Une loi peut être assouplie ou retirée, avec sa raison écrite dans `CHANGELOG.md`.

Les leçons sont versionnées avec le commun. Un agent peut lire les plus récentes pour s'en inspirer, mais son jeu n'est jugé que sur celles de sa version. Le catalogue actuel, dans `certifications/`, en est la première version.

Ce qui casse pendant une partie s'enregistre. Le commun rend facile d'en faire une pression ; un humain décide si elle se généralise.

## Le document de chaque jeu

Chaque jeu tient un document en trois parties :

- **Les intentions :** ce que le jeu veut et pourquoi. C'est la partie qu'un humain relit.
- **Les connaissances :** ce que l'agent a appris en cherchant, avec ses sources. Elles n'ont pas valeur de garantie.
- **Les invariants :** ce que la machine vérifie, et comment.

Il y répond aussi à chaque question que la base de leçons pose pour ses mécaniques : la leçon est adoptée, ou elle est sans objet, avec sa raison. Une question sans réponse bloque l'épreuve. Une applicabilité inconnue n'est jamais une exclusion.

Aujourd'hui, `LEARNINGS.md` et `verification.toml` tiennent ce rôle.

## L'épreuve

Avant qu'on y joue, chaque jeu passe l'épreuve du commun. Elle part du code source. Le résultat annoncé par l'auteur est un indice ; celui qui compte est l'épreuve qu'on relance soi-même.

1. **Compiler** le jeu depuis ses sources.
2. **Constater** ce à quoi le module a accès, comparer deux exécutions pour le déterminisme, et la copie native avec le module si elle existe.
3. **Examiner :** chaque question liée aux mécaniques déclarées a sa réponse.
4. **Chasser :** soumettre le jeu à des pannes, à un réseau fidèle au réel, à des entrées absurdes et à des ordres d'événements inattendus. Le faire parcourir par ses bots jusqu'à toutes ses phases, et vérifier à chaque pas les lois communes et les invariants du jeu. Jouer chaque partie avec et sans prédiction, et comparer leur histoire définitive.
5. **Injecter des bugs** dans le code du jeu, pour vérifier que ses tests les voient. Le résultat est un rapport sans seuil, qu'un humain juge.
6. **Revérifier les preuves** que le jeu apporte.
7. **Mesurer** les budgets.

Un contrôle qui n'a pas tourné ne compte pas comme réussi. Un contrôle doit savoir échouer : une faute injectée doit le faire échouer. Une exigence sans objet, un contrôle pas encore exécuté, un échec du jeu et une panne de l'outil sont des situations différentes, et le rapport les distingue.

Les contrôles rapides accompagnent chaque changement ; les longs tournent en continu, à côté. Seule la publication attend les longs.

Les gestes sur de vrais appareils restent des jugements humains. Leur fiche est dans [`REPETITION.md`](REPETITION.md).

## Les budgets

Ce sont des points de départ, à ajuster selon le genre :

- sur l'ordinateur de la soirée, au moins 60 images par seconde, 120 si l'écran le permet, et aucune image au-delà de 33 ms en jeu ;
- de l'entrée au pixel, 99 % des gestes en moins de 50 ms depuis une manette ou un clavier, et en moins de 80 ms depuis un téléphone en Wi-Fi local.

Une latence se mesure en distribution, jamais en moyenne. Le rapport sépare la part du commun et du jeu de celle du matériel, comme l'écran tactile, la télé ou l'enceinte, qui se règle avant de jouer. Les budgets se mesurent dans le jeu, avec un affichage de diagnostic qui s'active d'une seule touche.

## Le jour où l'on joue

- Une seule commande lance le jeu depuis son dépôt, et le README la documente.
- Le jeu s'ouvre dans un navigateur, en plein écran, et affiche comment le rejoindre.
- Le commun vérifie que les appareils se rejoignent, et dit clairement quoi faire sinon.
- On peut relancer une partie sans rien redémarrer.

## Les versions

Une version est une étiquette sur le dépôt du commun. Elle fige ensemble les lois, les leçons, ce contrat, l'outil et le modèle de jeu. Chaque jeu reste sur la version qu'il a choisie, et passe à la suivante avec `soiree upgrade`.

Nos propres frontières, nous les cassons librement, mais en le disant : chaque version a sa section dans `CHANGELOG.md`, avec les étapes exactes pour s'adapter. Un changement de sens sans rupture de type est plus dangereux qu'une rupture : il s'y documente aussi, et possède un test. Les frontières du monde, comme Safari, iOS, le Wi-Fi ou la télévision, ne s'alignent pas sur nous : nous les éprouvons.

## Les apps

Une app remplit le contrat d'un appareil quand elle apporte une capacité que le navigateur n'a pas, comme la vibration sur iPhone. Elle passe les mêmes pressions qu'une page, et suit la même méthode : une définition unique entre Rust et sa propre langue, des formes qui suppriment des familles de bugs, des pressions. Sur iPhone, une app ne peut pas compiler de code à la volée : pour faire tourner le module, elle passe par une vue web ou par un interpréteur. Un jeu reste toujours jouable dans un navigateur. Un changement du protocole d'une app se coordonne avant qu'on joue.

Le contrat de l'ancienne app iPhone est devenu des leçons, dans [`lecons/sources/remote.md`](lecons/sources/remote.md). Celui de la nouvelle app s'écrira avec elle.

## Le matériel

À renseigner par le mainteneur : les manettes, l'écran (résolution, fréquence et mode jeu), l'ordinateur, le réseau et les enceintes.

## Questions ouvertes

- La forme exacte de l'interface d'un jeu, à concevoir.
- La forme exacte des secrets et de la projection.
- La durée de la fenêtre avant qu'un pas devienne définitif, selon le genre.
- La forme exacte du relais et de son interface.
- La page d'un jeu dans une app : une vue web, ou le langage de dessin du commun rendu en natif.
- Le moment où un jeu est prêt.
- Ce que l'on reconstruit en premier, et ce que l'on garde de l'existant.
- La fusion de ce contrat avec `CERTIFICATION.md` et `VERIFICATION.md`.
- Le nom du document de chaque jeu.
