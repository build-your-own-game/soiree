# La répétition de la soirée (phase 3)

> **Statut.** La fiche de répétition neuve que demande le lot 14 ([`PHASE-3.md`](PHASE-3.md), § 7 et § 8.5), écrite le 2026-11-09. Elle s'écrit à part : l'ancienne [`REPETITION.md`](../REPETITION.md) décrit l'ancien système et attend la décision du mainteneur (question 17). **Rien de cette fiche n'a encore été fait** : la soirée réelle, les vrais téléphones, le vrai Wi-Fi et la caméra sont des jugements et des mesures humains. Ce que les machines ont déjà vérifié est dans [`LOTS-PHASE-3.md`](LOTS-PHASE-3.md) ; ce qui reste, dans [`RESTE-PHASE-3.md`](RESTE-PHASE-3.md). Chaque chiffre noté ici porte sa date, sa machine, son navigateur et sa version, et le téléphone qui l'a donné ; il remplace une valeur choisie seulement s'il est écrit dans `MESURES-PHASE-3.md` avec ces mentions.

**Date** ______ **Commit de soiree** ______ **Commits des jeux** ______ / ______ / ______
**Ordinateur** (modèle, macOS) ______ **Navigateur de la télé** (nom, version) ______
**Télé** (modèle, mode jeu oui/non, fréquence) ______ **Routeur et bande du Wi-Fi** ______
**Téléphones** (modèle, système, navigateur ; au moins un iPhone en Safari et un Android en Chrome) ______

## Le matériel

- L'ordinateur de la soirée **sur secteur**, **la session ouverte et l'écran allumé** : session verrouillée, Chrome quitte le plein écran aussitôt entré et le voile du commun couvre le jeu (constaté au lot 13b) ; écran éteint, Chrome ne donne que 30 images par seconde (lot 13). Aucune compilation, aucune épreuve ni aucun autre travail lourd pendant la répétition.
- La télé en **mode jeu** si elle en a un, à sa fréquence la plus haute ; le son par la télé.
- 8 à 15 téléphones sur le **Wi-Fi de la vraie soirée** (SAL-102, OUT-131), ni partage de connexion ni réseau invité, sauf si c'est celui du soir. Un Wi-Fi dont on connaît les invités : la page du téléphone arrive en `http`, et un invité actif sur le Wi-Fi pourrait la modifier (§ 2.5, question 18).
- **Un téléphone qui filme au ralenti** (240 images par seconde ou plus), sur un pied ou posé, qui voit à la fois le clavier ou le doigt et l'écran ; un chronomètre ; de quoi noter.

## La veille, depuis `soiree/`

1. Construire les outils, une fois : `cargo build --release -p relais -p epreuve`.
2. Éprouver chaque jeu au plan standard et le déposer au catalogue (seul un code 0 du plan standard y va, P17) :
   ```sh
   target/release/soiree-epreuve ../games/punchline --catalogue ~/.soiree/catalogue
   target/release/soiree-epreuve ../games/sidequest --catalogue ~/.soiree/catalogue
   target/release/soiree-epreuve ../games/tier-list --catalogue ~/.soiree/catalogue
   ```
   Compter une demi-heure (Sidequest seul : 562 s au plan standard, mesuré). Punchline rend aujourd'hui le code 1 au plan court (E2), mais le code 0 au plan standard (référence du lot 7b) : c'est celui qui compte.
3. Préparer le paquet de la page : `target/release/soiree-relais preparer` (il écrit son dossier).
4. **Le pare-feu de macOS** (SAL-95) : lancer une fois `target/release/soiree-relais`, et à la fenêtre « Accepter les connexions entrantes », cliquer **Autoriser**. Elle revient après chaque compilation du relais (question 15). L'aide de l'écran le dit si le pare-feu gêne.
5. **Chrome** : ajouter `localhost:7878` aux sites toujours actifs (Réglages, Performances, économiseur de mémoire) ; ouvrir la télé dans le profil habituel (la clé G de la garde vit dans son stockage, P37).
6. Vérifier **Échap en plein écran** dans le navigateur retenu (P38) : dans Chrome, Échap arrive au jeu et le plein écran reste ; dans Firefox, il sort du plein écran et l'écran du commun dit « Plein écran quitté — cliquer pour revenir ». Dans le plein écran **natif** de macOS (le bouton vert), rien n'a été constaté (lot 10b) : l'essayer, et noter.

## 1. La mesure du vrai Wi-Fi (M5), avant de jouer

C'est la mesure qui tranche le Wi-Fi (§ 3 de PHASE-3 : « l'inconnue principale ») et qui remplacera les constantes du monde simulé. La page existe depuis le lot 14 ; le banc l'a jouée de bout en bout sur la boucle locale seulement (`soiree-banc mesure-wifi`), jamais sur un vrai Wi-Fi.

1. `target/release/soiree-relais --paquet <le dossier de l'étape 3>`, puis, sur l'ordinateur de la soirée, ouvrir **`http://localhost:7878/#mesure`** sur la télé.
2. La télé montre un QR (vers `http://<adresse du Mac>:7878/mesure`) et, toutes les deux secondes, le dernier bilan de chaque téléphone. **Pas de QR** (« Pas de réseau ») : le Mac n'a pas d'adresse sur le Wi-Fi ; un téléphone qui scanne et n'arrive jamais : isolation des clients (SAL-99) ou pare-feu.
3. Chaque téléphone scanne le QR et laisse la page ouverte **au moins dix minutes** (la dérive de l'horloge ne se dit qu'au-delà d'une minute d'échos). Pendant ce temps, avec chaque téléphone :
   - **verrouiller** 10 s, puis 1 min, puis 5 min, et revenir ;
   - **changer d'application** (une notification, l'appareil photo) 3 s, puis 30 s, et revenir ;
   - s'éloigner du routeur dans la pièce la plus loin, puis revenir ;
   - tous ensemble, puis un par un.
4. Chaque bilan s'écrit aussi dans **`~/.soiree/garde/mesure-wifi.jsonl`** (une ligne JSON par bilan, toutes les dix secondes par téléphone). Garder ce fichier : c'est la mesure.

Ce qu'on lit, par téléphone, et ce que chaque nombre remplacera (annexe « Les valeurs choisies » de PHASE-3) :

| Le bilan | Ce qu'il dit | Ce qu'il remplace |
|---|---|---|
| `aller_retour_ms` (médiane, 95ᵉ, 99ᵉ, pire) | un écho toutes les 50 ms, aller et retour par le relais | la part du Wi-Fi du budget du téléphone (§ 3 : il reste environ 30 ms au Wi-Fi) ; la gigue de deux pas du monde simulé |
| `echos.rafales` (échos de plus de 100 ms) | les à-coups du Wi-Fi qui s'endort | `RAFALE` (100 ms) |
| `tics.ecart_ms`, `tics.trous` | ce que vivent les vues : un tic du relais tous les 16,7 ms | `RAFALE`, `RETRANSMISSION` (200 ms) |
| `echos.perdus`, `coupures`, `reconnexions` | une WebSocket ne perd rien sans tomber ; une coupure perd ce qui était en vol | `PERTE_FIABLE` (1 sur 40) ; `SILENCE_MAX_MS` (3 s) |
| `derive_ppm` | la dérive de l'horloge du téléphone contre celle du Mac | `DERIVE_MAX_PPM` (200) |
| `arriere_plan.fermee_apres_s`, `restee_ouverte_s` | après combien de secondes cachée la connexion tombe, ou combien elle tient | `FERME_EN_ARRIERE_PLAN` (5 s), `FERMETURE` (0,5 s) |
| le temps que met une page à se relier après le QR (au chronomètre) | | `PAGE_SANS_LIEN_MS` (15 s), « aucun téléphone en 60 s » |

**Ce que la page ne mesure pas** (*ajouté après la vérification du lot 14*), et qui reste choisi après la répétition :
- `DECALAGE_MAX` (une minute) : le bilan n'a aucun champ de décalage d'horloge ; la page ne lit que `performance.now()`, jamais l'horloge murale, et le système non plus (l'heure de l'autorité s'estime par l'écho). C'est une hypothèse du monde simulé ;
- `SYNCHRO` (5 s) : un choix du monde simulé ; `derive_ppm` dit seulement si 5 s suffisent ;
- `BONJOUR_REDIT_US` (2 s) et le lien sans `Bonjour` (10 s) : la page ne fait ni Noise ni `Bonjour` ; `coupures` et `echos.perdus` en donnent une idée, pas la valeur ;
- les valeurs du mandataire de pannes du banc (lot 5) : la page ne joue aucune panne.

Firefox et Safari donnent `performance.now()` à la milliseconde près (vu dans Firefox au banc) : un aller-retour de moins d'une milliseconde s'y lit 0 ou 1.

## 2. La soirée, dans l'ordre

Depuis le dossier d'un jeu (`games/punchline`) : `../../soiree/target/release/soiree-relais jouer --paquet <le dossier de l'étape 3>`. L'écran de démarrage s'ouvre sur la télé (catalogue, QR, aide). Les téléphones scannent ; **Jouer** (le geste : plein écran, son, verrou d'écran) ; on joue ; **On rejoue** deux fois ; **F9** remontre l'écran de démarrage, on choisit Sidequest, **Jouer** ; de même pour Tier list. C'est ce que `soiree-banc soiree-complete` joue (lot 14), avec des téléphones natifs.

| ☐ | Ce qu'on fait | Ce qu'on attend | Noter |
|---|---|---|---|
| ☐ | Chaque téléphone scanne le QR | moins de 30 s chacun, sans compte ; l'aide dit ce qui ne va pas (« pare-feu », « certains Wi-Fi isolent les appareils ») | modèle, durée, message |
| ☐ | Jouer | plein écran, son, rien du commun sur le jeu ; un iPhone verrouillé puis réveillé revient (SAL-53) | |
| ☐ | Regarder chaque page de téléphone | elle prend le fond que le jeu donne (Punchline : la couleur du personnage, ou la pâte grise avant ; Sidequest : son encre), et ses textes s'y lisent ; les écrans du commun (« Reconnexion… ») gardent leur fond sombre (D2, fait après la vérification du lot 14 ; vu au banc dans Chrome et Firefox de bureau seulement) | ce qui se lit mal, sur quel téléphone |
| ☐ | On rejoue, deux fois par jeu | la partie repart sans rien recharger ; le plein écran reste | |
| ☐ | F9, l'autre jeu, Jouer | les téléphones passent au nouveau jeu sans recharger (10 s au plus pour 69 natifs au banc) | durée par téléphone |
| ☐ | Une partie de 45 minutes (REM-71) | aucune page tuée, aucune latence qui monte par à-coups | moment, ressenti |
| ☐ | La fenêtre de la télé recouverte par une autre application, puis réduite | la partie continue (mesuré au banc, session verrouillée seulement ; Firefox réduite saute un pas, question 30) | ce qu'on voit au retour |
| ☐ | Un téléphone qui recharge sa page, un autre qui quitte et revient | il retrouve son siège et son personnage | |
| ☐ | Le plaisir (MANIFESTE) | | rires, flottements, un mot de chacun |

## 3. La caméra : quand l'image est vraiment à l'écran

Le banc mesure de l'entrée **à la fin du travail du GPU**, pas au photon (OUT-88, § 8.4). Deux questions restent, que seule la caméra tranche :

- **Firefox avec sa fenêtre** : le segment du GPU (de la soumission à `onSubmittedWorkDone`) y pèse 84 à 110 ms au 99ᵉ centile, et met le clavier à 124–137 ms (lot 13) ; dans Chrome avec sa fenêtre, 28,5 à 33,6 ms. On ne sait pas si l'image est à l'écran si tard (question 32).
- **La part du matériel** : la dalle tactile, la télé, son mode jeu, l'enceinte (CONTRAT, « Le matériel » ; question 14).

La méthode, sur `ardoise` ou Punchline, le **panneau de diagnostic ouvert** (la touche que déclare le jeu) : à chaque entrée, un **carré blanc de 48 px s'allume en bas à droite pendant au moins 100 ms** (REM-117).

1. Filmer au ralenti, dans le même cadre, la touche (ou le doigt sur le téléphone) et le coin du carré blanc, sur l'écran du Mac puis sur la télé.
2. Trente appuis espacés d'au moins une demi-seconde, dans **Chrome** avec sa fenêtre, puis dans **Firefox** avec sa fenêtre, à la même fréquence d'écran.
3. Pour chaque appui, compter les images du film entre le contact (la touche au fond, le doigt posé) et la première image où le carré est blanc ; convertir en millisecondes (à 240 images par seconde, une image vaut 4,2 ms).
4. Noter à côté ce que le panneau dit pour la même source (médiane et 99ᵉ centile du total, et du segment du GPU).

Ce qu'on en tire : la différence entre la caméra et le panneau, dans Chrome, est la part du matériel (clavier, écran, télé). Si, dans Firefox, la caméra donne à peu près ce qu'elle donne dans Chrome alors que le panneau dit 100 ms de plus, `onSubmittedWorkDone` arrive après le photon et ne doit pas dire le pixel (question 32) ; si la caméra voit aussi ces 100 ms, Firefox avec sa fenêtre ne tient pas le budget du clavier, et c'est au mainteneur de dire s'il reste un navigateur de la soirée (question 6). Même méthode pour un téléphone (le doigt et la télé dans le cadre), avec le budget de 80 ms.

## 4. Les gestes qui attendent après « Jouer »

Au banc (lot 13, M2 sur Punchline), les gestes des téléphones de la première seconde attendent leur image **de 0,7 à 1,9 s**. **La cause est établie au lot 14** (`RESTE-PHASE-3.md`, « La latence ») : ces gestes sont faits **avant** « Jouer », pendant que l'écran de démarrage du commun couvre le jeu ; la page ne dessine la scène du jeu qu'après le geste, et leur pixel est la première image du jeu après « Jouer ». Le panneau d'une vraie soirée les compte, après chaque « Jouer ».

À la répétition : laisser les téléphones jouer (choisir un prénom, un personnage) pendant que l'écran de démarrage est encore là, puis « Jouer », puis ouvrir le panneau tout de suite. Noter si sa première ligne du téléphone dit un 99ᵉ centile de plus d'une seconde, et si un joueur a vu quoi que ce soit d'anormal (il ne voyait pas le jeu avant « Jouer »). C'est la question 33 : ces gestes comptent-ils au budget ?

## 5. Ce que seuls de vrais appareils diront

- **Les vrais téléphones** (OUT-121) : la pile du fil principal (le module y tourne sous `PHASE_3`) ; le temps de compilation du module sur la page (M4, aucune borne de temps) ; la durée de `Page` (question 22, la marge de 4 choisie) ; ce que l'ITP de Safari efface d'une soirée à l'autre (le personnage retenu de Sidequest, question 23). Pour chaque modèle : le temps entre le scan et la page prête, au chronomètre.
- **Le gel d'iOS** au verrouillage et au réveil (SAL-53) : un tenu de Sidequest relâché, aucun geste perdu.
- **Le retard de la télé et du son, filmé** (SAL-179) : un effet de son et son image, dans le même film.
- **La fenêtre de la télé vraiment recouverte, ou réduite**, session ouverte (lot 13b : le banc ne l'a mesurée que session verrouillée). Et, la session ouverte, la machine peut finir ce que le lot 13b n'a pas pu : `target/travail/lot-13b/campagne.sh m2 m6` (la souris de M2 avec la fenêtre de Chrome, M6 réduite) ; c'est une mesure de la machine, mais elle demande l'hôte présent et la session ouverte.
- **La demande du pare-feu** au premier lancement (SAL-95), et ce que l'aide en dit.
- **Safari** sur la télé : le banc ne le pilote pas tant que « Autoriser l'automatisation à distance » n'est pas coché (question 8) ; à la répétition, l'ouvrir à la main et jouer une partie.

## Quand ça casse

- Un téléphone qui n'arrive pas : lire l'aide de l'écran de démarrage (F9) ; elle nomme la cause (pas de réseau, pare-feu, isolation des clients).
- La télé figée ou un bandeau « le commun est en panne » : recharger l'onglet ; la partie reprend de la garde (« Cliquer pour reprendre »). Noter l'heure.
- Un moment étrange : **F8** pose une marque au journal (SAL-162) ; après la soirée, `target/release/soiree-banc rejouer-garde ~/.soiree/garde --cle <la clé exportée par le diagnostic> --module <le module>` rejoue la garde jusqu'à chaque marque.
- Après la soirée : garder `~/.soiree/garde` (la garde et `mesure-wifi.jsonl`), le film, et cette fiche remplie ; les chiffres vont dans `MESURES-PHASE-3.md`, avec leurs mentions, et les constats dans `RESTE-PHASE-3.md`.
