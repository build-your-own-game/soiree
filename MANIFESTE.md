# Manifeste

## Le projet

Ce projet réunit des amis qui font chacun écrire un jeu par leur agent de code, pour y jouer ensemble dans une même pièce. Un ordinateur fait tourner les jeux dans un navigateur et les affiche sur la télévision du salon. Selon le jeu, on joue avec le clavier et la souris de cet ordinateur, avec des manettes ou avec des téléphones : un par joueur, un par équipe, ou un seul pour toute la partie. Un jeu peut être une tier list, un FPS, un Mario Kart ou un jeu de répliques : le mot s'entend au sens le plus large.

Le projet a deux parties. Le commun est écrit une fois et partagé par tous les jeux. Il relie les appareils, fournit le temps et le hasard, affiche, enregistre et reprend après une panne. Il fait aussi passer à chaque jeu une épreuve avant qu'on y joue. Les jeux, eux, peuvent être écrits par n'importe qui, librement, tant qu'ils respectent le contrat du commun.

## Ce que nous testons

Derrière ces jeux, nous testons une idée : écrire du code ne coûte presque plus rien. Si c'est vrai, on peut repenser la façon de construire du logiciel. Le travail ne consiste plus à écrire, mais à dire ce qu'on veut et à soumettre le logiciel aux bonnes pressions. Les agents écrivent le reste.

Les jeux sont notre preuve de concept, et un terrain exigeant. Plusieurs appareils, un réseau imparfait et des joueurs qui remarquent tout : beaucoup de choses peuvent mal se passer. Nous visons la qualité des meilleurs jeux de chaque genre. L'expérience est réussie si les joueurs ne parlent que du plaisir, jamais d'une panne : pas de joueur éjecté, de partie figée ou de score faux. Pour qu'elle prouve quelque chose, il faut que ça marche vraiment.

La première chose que nous voulons garantir : quoi que fasse un jeu, il ne peut ni planter la partie ni la figer.

## Nos hypothèses

- Écrire du code est presque gratuit.
- Les modèles progressent très vite. Nous concevons pour ceux qui arrivent : un problème qui disparaîtrait avec un modèle bien meilleur ne doit pas guider l'architecture.
- Les agents suivent très bien un parcours quand chaque étape peut être vérifiée. Nous exploitons cette capacité.
- Certaines choses restent rares, quel que soit le modèle : savoir ce qu'on veut, connaître le monde réel, garder des contrôles honnêtes. C'est là que va notre attention.

## Le cadre

- Le code est écrit en Rust. Ce langage est sûr en mémoire sans ralentir l'exécution, et son compilateur attrape beaucoup d'erreurs avant qu'un programme ne tourne.
- Les jeux tournent dans un navigateur, sur l'ordinateur de la soirée comme sur les téléphones. Pour relier les appareils, l'ordinateur de la soirée fait tourner un petit programme, ou passe par un service sur internet. Un joueur n'a besoin que d'un navigateur ; une app peut le remplacer quand elle apporte quelque chose. Le navigateur reste une cible parmi d'autres : le même code doit pouvoir tourner ailleurs, et d'abord dans nos tests, sans écran et à vitesse machine.
- Ce qu'on joue tourne enfermé, partout de la même façon : on peut constater ce à quoi il a accès, et il n'atteint rien de ce qu'on ne lui donne pas. Le moyen d'y parvenir, aujourd'hui WebAssembly, est décrit dans le contrat.
- On suppose une machine équipée d'un GPU.

## Pourquoi repartir de zéro

Nous reconstruisons ce dont nous avons besoin plutôt que d'empiler du code existant, pour trois raisons. Nous voulons avoir confiance dans le code qu'on exécute, échapper à des classes entières de bugs que ce code porte avec lui, et gagner une liberté qu'il ne permet pas, pour optimiser ou pour faire ce qu'il interdit.

Repartir de zéro a un prix : on perd ce que les autres ont appris en se heurtant au réel. Ces leçons, nous allons les chercher.

Le code tiers reste permis. Il se choisit consciemment, avec sa raison, et nous cherchons à le réduire. La frontière entre ce que nous reconstruisons et ce que nous gardons bouge avec ce que nous apprenons.

## Savoir le plus tôt possible

Une erreur doit se voir le plus tôt possible : à l'écriture, sinon à la compilation, sinon quand les machines éprouvent le jeu, jamais devant les joueurs.

Aucun outil ne peut juger n'importe quel programme : c'est un résultat mathématique. Nous ne cherchons pas un tel outil. Nous construisons des programmes faits pour être jugés.

Vérifier ne doit pas pour autant étouffer la vitesse d'écriture. Les agents écrivent vite, et des vérifications trop lentes leur retireraient ce gain. Ce qui est rapide accompagne chaque changement ; ce qui est long tourne en continu, à côté.

## Le commun et les jeux

Le commun possède tout ce qui touche au monde : le temps, le hasard, le réseau, les entrées, l'affichage, le son, les sauvegardes, l'identité des joueurs et la reprise après une panne. Un jeu n'y touche jamais directement.

Un jeu reçoit des entrées, et rend son nouvel état, ce qu'il montre et ce qu'il demande. À l'intérieur de cette forme, il est libre : ses règles, son moteur, son allure et sa page de téléphone lui appartiennent. La contrainte sur la forme donne la liberté sur le fond.

Cette forme rend impossibles des familles entières de bugs :

- un jeu qui traite un événement à la fois ne connaît pas deux actions simultanées ;
- un jeu qui reçoit le temps ne peut pas se tromper d'horloge ;
- une image recalculée depuis l'état ne peut pas se désynchroniser de cet état.

Plutôt que de prévoir chaque panne, nous les ramenons toutes vers une même façon de repartir, éprouvée en permanence.

## Un jeu déterministe

Un jeu est une fonction pure : il ne dépend que de ce qu'il reçoit. Depuis le même état, les mêmes entrées dans le même ordre donnent toujours la même partie. Le temps et le hasard lui arrivent comme des entrées. Il n'a aucun autre accès au monde, et son enfermement le garantit sans qu'il ait à le promettre.

C'est ce choix qui rend le reste possible : rejouer une partie pour comprendre un bug, reprendre après une panne, faire réagir un téléphone sans attendre l'ordinateur de la soirée, et laisser des machines jouer à notre place, en accéléré, des milliers de parties. Un jeu fournit donc aussi de quoi se jouer seul.

## La méthode

Face à une erreur connue, nous cherchons d'abord une forme qui la rend impossible. Si aucune n'existe, nous la mettons sous pression : une preuve, un test, un fuzzer, une simulation, ou une leçon que les agents appliquent. Une leçon seulement écrite aide, mais ne compte jamais comme vérifiée.

Les leçons viennent de tout ce qui nous informe du réel : le code et l'historique d'autres projets, nos propres pannes, une partie jouée, une vidéo où des joueurs se plaignent d'un comportement. Nous pouvons en tirer nos propres pressions, tant qu'elles restent fidèles à ce réel. Ce qui casse pendant une partie s'enregistre, pour devenir facilement une nouvelle pression.

Les pressions s'organisent en étages :

- celles du commun valent pour tous les jeux ;
- d'autres valent pour une mécanique, comme la physique ou le vote ;
- d'autres valent pour un seul jeu ;
- certaines restent de la connaissance que l'agent applique sans test.

Une leçon monte d'un étage quand elle revient, et c'est un humain qui décide qu'elle se généralise.

Deux morceaux qui doivent s'accorder n'ont qu'une seule définition, que le compilateur fait respecter. Ce qui vient de l'extérieur est validé dès son entrée. Chaque choix important s'écrit avec sa raison, dans le commun comme dans les jeux.

## Le contrat entre le commun et les jeux

Entre le commun et chaque jeu, il y a un contrat. Il impose aux agents un parcours dont chaque étape laisse une trace contrôlable. Il donne deux sortes de garanties :

- **la garantie d'examen :** chaque question a été posée, et a reçu une réponse avec sa raison, même « sans objet » ;
- **la garantie de résultat :** la réponse a été vérifiée, par une preuve ou par une pression.

Le contrat demande toujours la première, et la seconde chaque fois que c'est possible.

Le jeu dit ce qui doit rester vrai, et le commun essaie de le casser. Le commun donne ses garanties par les types ; dans l'autre sens, il ne croit rien sur parole : il constate, ou il revérifie. Le code source d'un jeu est toujours disponible, et l'épreuve qui compte est celle qu'on relance soi-même.

Chaque loi du commun a un niveau de justification, qui peut monter avec le temps : imposée par la forme, demandée dans le code, éprouvée par des pressions, ou exigée sous forme de preuve. Le commun permet dès maintenant d'exiger une preuve, et il l'exigera quand les modèles la rendront facile.

## La confiance

Nous faisons confiance aux contrôles, jamais à l'auteur, qu'il soit humain ou agent. Ce qui est prouvé est acquis et n'a pas à être refait ; ce qui est seulement testé reste sous pression.

Une preuve ne répond qu'à la question qu'on lui pose, et on peut se tromper de question. C'est pourquoi les intentions s'écrivent pour être relues.

## L'évolution

Les lois et les leçons du commun sont versionnées ensemble. Chaque jeu reste sur la version qu'il a choisie : il peut lire les leçons les plus récentes, mais n'est jugé que sur celles de sa version. Plus le commun se renforce, plus il vaut la peine de le suivre, et les agents rendent ce passage facile.

Une leçon a la même forme à tous les étages : elle naît là où on l'a découverte, et monte telle quelle. Un agent la propose au commun, et le mainteneur du commun l'accepte ou la refuse. Une leçon devient une loi par sa décision. Une loi peut aussi être assouplie ou retirée, avec sa raison. Nos propres frontières, nous les cassons librement, mais en le disant.

Une app, comme l'app iPhone, peut remplacer le navigateur sur un appareil quand elle apporte quelque chose, comme la vibration. Elle remplit le même contrat qu'une page et suit la même méthode, mais un jeu reste toujours jouable sans elle.

## Ce qui reste humain

Le plaisir se juge en jouant, et seuls les humains en décident. Ils décident aussi de ce qui mérite de devenir une règle.

Ce document garde l'esprit. Les détails, comme les versions, les budgets ou les outils, vivent dans le contrat du commun, [`CONTRAT.md`](CONTRAT.md). Ce document ne se modifie qu'à la demande explicite du mainteneur.

Repartir de zéro libère le code, pas le monde. Le Wi-Fi coupe, les horloges dérivent, une télé ajoute son retard, un téléphone se met en veille au milieu d'une manche. Nos pressions doivent ressembler à ce monde-là, sinon nos garanties portent sur un monde qui n'existe pas.
