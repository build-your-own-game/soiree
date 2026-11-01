# D2 : terminal observé et reprise — 29 octobre 2026

Les contrôles ciblés sont satisfaits. `helper-tests.log` conserve les quatre
tests du helper commun : terminal puis reprise, cycle A↔B sans terminal,
terminal sans reprise et erreurs/attentes bloquées. Une suite qui change
d'écran sans atteindre son terminal échoue au lieu de satisfaire D2.

`punchline.json` conserve un vrai parcours avec TV Chromium et trois pages
téléphone (WebKit, Chromium, WebKit). Dix actions Space d'organisateur
traversent les trois manches sans réponses ni votes, atteignent le podium,
puis un nouveau lobby garde les trois joueurs et une nouvelle écriture en
manche 1 commence. Le négatif effectue trois autres changements de phase mais
ne reconnaît aucun terminal : le helper le refuse. Borne annoncée du parcours
principal : 128 actions et 60 secondes.

`sidequest.json` conserve deux questions libres de dix secondes. Les seules
commandes d'organisateur envoyées sont les deux démarrages ; aucune commande
Finish, aucun Tick injecté ni horloge remplacée. Le premier `RoundResult`
présenté par le GPU compte le joueur connecté. Une nouvelle question accepte
un saut téléphone observé dans la projection présentée et atteint à son tour
son échéance naturelle ; son numéro de manche augmente et deux résultats
existent dans l'API. Borne du premier terminal et de la reprise : 250 attentes
de 100 ms, 35 secondes au total ; le second terminal a une attente de 15 s.

Les scripts, journaux d'hôte, sorties et empreintes après les tests sont
conservés aussi dans `games/<jeu>/certification/terminal-2026-10-29/`. Le
premier échec Sidequest reste dans `failed-welcome/` : le pilote envoyait la
question par API sans entrer dans l'arène avec Play. Le serveur avait accepté
le saut mais la TV montrait toujours l'accueil ; attendre un saut présenté
était donc incorrect. Le correctif est l'unique pression Enter initiale de la
TV, qui change seulement son écran local, puis une salle isolée neuve.

Ces observations sont des témoins bornés en WebSocket loopback. Elles ne
prouvent pas la terminaison pour toutes les entrées, le LAN, des appareils
physiques ou la totalité du plan. Leurs JSON inscrivent explicitement
`universal_termination_proved: false`. Les propriétés de la simulation, le
watchdog pendant le chaos, la nouvelle intégration CLI `progression` et les
campagnes complètes restent des contrôles distincts. Aucun D5 ou autre item
non relié n'est satisfait implicitement par ce parcours.

Référence de méthode : [Specifying Systems, section 14.3.5](https://lamport.azurewebsites.net/tla/book-21-07-04.pdf),
sur les limites d'un modèle fini pour la vivacité et l'intérêt d'un contrôle
négatif qui doit effectivement échouer. Aucun outil TLA+ ni preuve formelle
du jeu n'a été exécuté dans cette passe.
