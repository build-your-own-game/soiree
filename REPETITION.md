# Répétition sur vrais appareils

> La fiche de l'hôte pour l'étage 3 de [`CERTIFICATION.md`](CERTIFICATION.md) : 15 min de gestes, puis une partie. Les gestes S1 à S10 et ce qu'on attend de chacun sont là-bas, et seulement là-bas : ouvre-le à côté. Ici : le matériel, l'ordre, ce qu'on note, ce que seul un vrai appareil dira de Sidequest, et quoi faire quand ça casse. C'est un jugement humain, pas un test passé.

**Date** ______ **Jeu** ______ **Commit du jeu** ______ **Commit de soiree** ______ **Wi-Fi** ______

## Matériel

- Mac branché à la TV, **sur secteur** (sous 20 % de batterie, Chrome peut brider l'affichage), son allumé, canapé à 3 m. Aucune certification ni compilation en parallèle.
- 3 joueurs (minimum de Punchline) : un iPhone en Safari, un iPhone avec l'app Télécommande, un Android en Chrome. S'il en manque un, **B** sur le Mac ajoute un robot.
- Le Wi-Fi de la vraie soirée, ni partage de connexion ni réseau invité. Un chronomètre, un stylo.

## La veille, depuis le dossier de l'hôte

1. `soiree status`, `soiree doctor`, puis note `git -C games/punchline rev-parse --short HEAD` et `(cd soiree; git rev-parse --short HEAD)` : ici, le jeu compile contre le `soiree/` local, pas contre l'étiquette.
2. App sur l'iPhone (Xcode 27, mode développeur activé) : `open soiree/remote/ios/Remote.xcodeproj`, choisis l'iPhone et ton équipe dans *Signing & Capabilities* (change l'identifiant `fr.buildyourowngame.remote` s'il est pris), ⌘R. Avec un compte Apple gratuit, elle expire après 7 jours.
3. Lance une fois : `cd games/punchline && SALON_LOG=1 cargo run --release -p punch-host`.
4. **Pare-feu.** À la fenêtre de macOS sur les connexions entrantes, clique **Autoriser**. Elle revient après chaque compilation. Sans cela, au mieux WebRTC est perdu ; le 2 novembre, un jeu non autorisé n'a rien renvoyé sur l'adresse réseau du Mac, alors que `localhost` répondait.
5. Ouvre `http://localhost:8090` dans Chrome et scanne le QR : la page doit s'ouvrir, et le terminal dit le transport (WebRTC ou WebSocket). Ctrl+C.
6. Refais 3 à 5 pour Sidequest (`games/sidequest`, `-p sidequest-host`, `http://localhost:2567`) : c'est un autre binaire, donc une autre fenêtre de pare-feu.

## Le soir, dans cet ordre

Dans `games/punchline` : `SALON_LOG=1 cargo run --release -p punch-host -- --no-build`, puis `http://localhost:8090` sur la TV en plein écran (Ctrl+Cmd+F). Espace sur le Mac lance et fait avancer.

**Échec général, pour S1 à S4 :** plus de 10 s pour revenir, un nom, un personnage ou un score perdu, un rechargement à la main, un écran vide ou figé.

| ☐ | Item | Ici, en plus de `CERTIFICATION.md` | Noter |
|---|---|---|---|
| ☐ | S7 | Safari, l'app (son scanner), Chrome Android. Moins de 30 s chacun, sans compte, pas de salle fantôme. | modèle, OS, durée, transport |
| ☐ | S9 | Le premier geste est Espace sur le Mac. Le son vient de la TV, jamais des téléphones. | quand il arrive |
| ☐ | S8 | Pendant l'écriture et un duel. | écran, photo |
| ☐ | S1 | iPhone Safari, chrono à la main. | secondes |
| ☐ | S2 | Même iPhone. | secondes |
| ☐ | S3 | Même iPhone. | secondes |
| ☐ | S4 | Joue une minute. | ressenti |
| ☐ | S5 | TV et téléphones restent dans la même phase. | écart |
| ☐ | S6 | Réglages iPhone > Confidentialité et sécurité > Réseau local > coupe Télécommande. Rouvre l'app, scanne, puis réactive. « Connexion au jeu… » sans fin est un échec (le code n'a pas de texte dédié). | texte, capture |
| ☐ | S10 | Avec de vrais joueurs, jusqu'au bout. | rires, flottements, un mot de chacun |

**T9, à trancher ici.** Sur l'iPhone Safari, bon Wi-Fi, en pleine manche : regarde une notification ou passe 3 s dans une autre app, puis reviens, trois fois. `wake` coupe alors un lien encore valide dès 1,5 s de silence. Rien n'est perdu, mais une reconnexion a lieu (`recherche/reconnexion.toml`, T9). Note si le joueur la voit, et si ça gêne.

## Ensuite, Sidequest

Ctrl+C, puis `cd games/sidequest && SALON_LOG=1 cargo run --release -p sidequest-host -- --no-build`, TV sur `http://localhost:2567`, **Play** ou Entrée, puis → et ←.

Repasse S7, S9, S8, S1, S3, S5, S10. Le paquet par défaut est `games/sidequest/questions/soiree.fr.json` : 30 questions françaises de 30 s, soit 15 min sans compter les résultats.
- **S8 :** les accents du tableau se lisent à 3 m.
- **S10 :** les 30 questions, jusqu'au tableau final. Note la question où ça s'essouffle, et si les joueurs réclament un compte à rebours : ni la TV ni le téléphone n'en affichent, et l'ajouter reste à décider.

### Ce que seul un vrai appareil dira

Rien de ceci n'a été essayé sur un vrai iPhone : tout vient de la lecture du code et d'un faux client (partie « La télécommande native » du `LEARNINGS.md` de Sidequest). Un KO ici ne casse pas un item : c'est une décision à prendre, avec ce qu'on a noté.

| ☐ | Essai | Geste | On veut savoir | Noter |
|---|---|---|---|---|
| ☐ | Page dans l'app (WKWebView) | Scanne le QR depuis l'app, choisis ton personnage. Ferme l'app de force, rouvre-la. | Le prénom est prérempli, la page reste en portrait, et l'app retrouve son personnage. | ce qui manque |
| ☐ | Bascule page-pad | Choisis le personnage : l'app ferme la page et passe au stick, en paysage. | Le temps de bascule et de rotation. La TV ne montre aucun départ. | secondes |
| ☐ | Retour page-pad | « Écrire », envoie un message. Refais-le, mais laisse un brouillon jusqu'à la question suivante. | Le message envoyé ramène au stick. La question suivante aussi, et le brouillon perdu ne gêne pas. | gêne, oui ou non |
| ☐ | Tenue | Joue une manche en paysage. | La prise, la taille des libellés « Sauter » et « Écrire ». | ressenti |
| ☐ | Zone morte au pouce | Marche lentement, puis à fond. | 12 % de zone morte et pleine vitesse dès 90 % de la course sont des valeurs de départ, dans `games/sidequest/crates/host/src/pad.rs` : à régler au pouce. | valeurs voulues |
| ☐ | Vibration | Marque un point sans regarder la TV. | Le point se sent, sur le pad seulement. | sentie, oui ou non |
| ☐ | Pad en TCP contre page en WebRTC | Deux iPhones côte à côte, l'app au pad et Safari en WebRTC (le terminal le dit), sur un Wi-Fi qui perd : loin de la borne, ou Réglages > Développeur > Network Link Conditioner, profil *Very Bad Network*. Allez ensemble vers une zone, puis gardez le pouce immobile. | Si le stick natif tarde sur la page. Une perte de 200 ms peut lâcher le personnage un instant, pouce immobile : le code le laisse supposer, rien ne l'a mesuré. | écart, lâchers |
| ☐ | Safari qui suspend | Dans S1, chronomètre de l'ouverture au retour à sa place. Puis verrouille pendant qu'un iPhone rejoint, juste après le scan, et rouvre. | Un retour près de 6 s veut dire que la page n'a pas vu le temps du sommeil (T5 de `recherche/reconnexion.toml`). Une tentative qui ne repart pas au réveil, c'est T4. | secondes, chaque fois |
| ☐ | Appui très bref | Sur Safari, tape la croix le plus brièvement possible. | S'il peut disparaître, et si ça gêne : TR-03 reste `unreviewed` pour cette raison. | gêne, oui ou non |
| ☐ | Du téléphone au pixel | Filme au ralenti le téléphone et la TV dans le même cadre. Compte aussi du scan du QR au premier mouvement. | Les deux mesures qui manquent à Sidequest. | millisecondes, secondes |

## Enfin, Tier list

`cd games/tier-list && cargo run --release -p tier-host`, sur `http://localhost:8080`. Pas de téléphone : S8 et S5 seulement.

## Noter un échec

Heure, geste, appareil, photo de l'écran, ce que dit le terminal. **F8 sur la TV** marque le moment dans le journal. Passe le point en KO et continue. Plus tard, avec ton agent : le reproduire par un test, écrire dans `LEARNINGS.md` pourquoi aucun item ne l'a vu, puis ajouter ou durcir un item.

## Relancer

- **Téléphone coincé :** note KO, recharge sa page ; il doit retrouver sa place. Sinon, rescanne le QR.
- **Le jeu tombe :** Ctrl+C, puis la commande du soir. Les téléphones reviennent seuls, la partie repart de zéro et chacun redonne son prénom.
- **La page ne s'ouvre pas :** même Wi-Fi ? Pare-feu autorisé ? QR faux : préfixe la commande par `SALON_PUBLIC_URL=http://<adresse-du-mac>:8090`.
- **Port pris :** `lsof -nP -iTCP:8090 -sTCP:LISTEN` donne le processus, puis `kill <numéro>`.
- **Reprendre :** sans changement de code, repars du point KO. Après une correction, nouvelle fenêtre du pare-feu : repasse le KO, S1, S3, S7, et toute la liste si `soiree/` a changé.
- **Même échec deux fois :** arrête de relancer ce point, il faut une correction.
