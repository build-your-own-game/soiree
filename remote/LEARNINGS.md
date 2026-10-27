# Learnings — Télécommande

Catégories du manifeste :
- **P** : physique, matériel ou perception, gardé ;
- **U** : usage réel, gardé ;
- **A** : accident du langage ou du runtime, jeté ;
- **L** : héritage ou compatibilité, jeté.

Trois sources ont été étudiées :
1. **Les manettes tactiles d'émulateurs**, avec leur historique (commits, issues, PR) : Delta / DeltaCore (iOS), PPSSPP, Dolphin Android, Mupen64Plus-AE (M64AE), RetroArch.
2. **Les manettes virtuelles du commerce** : Apple (HIG *Game controls*, Touch Controller de la WWDC26, GCVirtualController de la WWDC21), la Touch Adaptation Kit de Microsoft (TAK, plus de 200 jeux adaptés pour Xbox Cloud Gaming) et AirConsole. S'y ajoute Baldauf et al., ACM TOMM 2015 : un téléphone qui pilote un jeu affiché sur un autre écran, exactement notre cas.
3. **Le matériel N64 et les jeux** : wiki n64brew, libultra, manuel développeur Nintendo, ROM de test mimi, décompilations de Super Mario 64, Ocarina of Time, Majora's Mask, Mario Kart 64 et GoldenEye.

## Stick

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Un vrai stick va jusqu'à ~85 sur un axe et ~(70, 70) dans un cran diagonal : le gate est un octogone, pas un cercle. Avec un cercle, les diagonales lisent 60, et GoldenEye ou Ocarina of Time, qui lisent chaque axe séparément, perdent ~20 % de vitesse en diagonale | mimi `range_test.c` ; manuel Nintendo §3-5 ; décompilations ; Dolphin db75509ec5 | P | **Gardé** : gate octogonal 85 / 70 en Rust et dans le dessin. |
| Les jeux ont leur propre zone morte : 7 dans SM64, OoT et MM, 5 dans GoldenEye, un carré de 12 dans MK64. En ajouter une grosse les empile (PPSSPP #15022) | décompilations ; PPSSPP #15022 | P | **Gardé** : 3 % seulement, contre le tremblement du doigt, remis à l'échelle pour éviter un saut. |
| Réponse linéaire : personne ne s'est plaint de la courbe, seulement de l'amplitude | les cinq émulateurs | U | **Gardé.** |
| Trop peu de course est la plainte n°1 (Delta : ~15 pt, #380 et #527). Bonne valeur : 12 à 14 % de la hauteur | Delta, M64AE, Dolphin | P | **Gardé** : 50 pt pour la pleine déflexion sur un axe. |
| Placer le stick sous le pouce plutôt qu'à une place fixe | HIG Apple ; TAK `relative` ; Baldauf 2015 (le stick fixe fait dériver quand on regarde la TV) ; Dolphin | U | **Gardé**, en version bornée (ligne suivante). |
| Mais toucher *autour* du stick doit le pousser dans cette direction tout de suite | PPSSPP #14410 ; « zone de recentrage » de RetroArch | U | **Gardé** : un pouce posé à moins de 0,6 course du repos devient le centre ; plus loin, le centre s'arrête sur ce cercle. |
| Toute une moitié d'écran attrape le stick, pas un cercle autour de son dessin | Touch Controller (WWDC26) : `.leftSide` | U | **Gardé** : toute la moitié gauche hors croix et boutons. |
| Le dessin du stick qui saute sous le pouce donne l'impression d'un bug | retour de l'hôte au premier essai | U | **Gardé** : le recentrage est invisible. Le socle reste fixe, seul le bouton du stick bouge. |
| Le doigt qui a pris le stick le garde partout et n'appuie jamais sur un bouton | PPSSPP 4ec1a8d12 ; Dolphin ; M64AE ; RetroArch | U | **Gardé.** |
| Aucun projet ne fait suivre le centre quand le doigt dépasse le bord | les cinq émulateurs | U | **Gardé** : bloqué au gate. |
| Vibrer pendant que le stick bouge déplaît | Dolphin PR #13220 | U | Une seule vibration en touchant le gate, réarmée sous 85 %. |
| Plus tard : un léger aimantage vers les 8 crans | mimi, M64AE 0300f244 | P | **À essayer** sur un vrai iPhone. |

## Boutons

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Glisser d'un bouton à l'autre appuie sur le nouveau et relâche l'ancien | PPSSPP, M64AE, Delta, RetroArch | U | **Gardé.** |
| Un pouce entre A, B et Z appuie sur les deux voisins : c'est voulu sur la N64 | skin N64 de Delta ; « ABXY overlap » de RetroArch #14565 | P | **Gardé** : A, B et Z en triangle, 20 pt entre voisins. Les boutons C, eux, ne se combinent jamais. |
| Rouler le pouce d'un bouton à l'autre ne doit pas relâcher le premier trop tôt | Delta #445 ; PPSSPP #14490 | U | **Gardé** : dans la zone commune, les deux restent appuyés. |
| Un bouton tenu garde une marge plus large, sinon il clignote sur son bord | RetroArch #15534 | P | **Gardé** : 15 pt pour un nouvel appui, 30 pt pour un bouton déjà tenu. |
| Z se tient avec l'index pendant que le pouce gauche dirige. Sur un téléphone, seuls les deux pouces sont libres | TAK (deux entrées à la fois) ; M64AE met Z à gauche, ce qui bloque le stick ; Delta le met à droite | P / U | **Gardé** : Z à droite, avec A et B. |
| Au moins 44 pt pour un contrôle fréquent. Tailles en points depuis les bords de la zone sûre, pas en part de l'écran | HIG Apple ; Touch Controller | P | **Gardé** : tout est en points, testé sur 4 iPhones. |
| Un bouton appuyé est caché sous le pouce : il faut un halo plus large que lui | HIG ; PPSSPP a9a45481c | U | **Gardé** : halo de la couleur du bouton. |
| Start en haut au centre comme sur la vraie manette, et rien d'important en bas au centre (geste « accueil ») | n64brew ; PPSSPP f42f7ac9d | P / U | **Gardé.** |

## Dessin d'un appui

Mesuré chez nous : un écran filmé dans le simulateur montrait une tape rapide pendant une seule image (8 ms), voire aucune, puis halo, assombrissement et ombre disparaissaient d'un coup. C'était le « scintillement ».

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| L'appui s'affiche tout de suite : aucun émulateur n'anime le passage à l'état appuyé, ni n'échange la latence contre un effet | PPSSPP `GamepadEmu.cpp` ; Delta 1ef4247b2 et 633dfa869 ; Dolphin ; RetroArch `alpha_mod` | U | **Gardé** : aucune animation à l'appui. |
| En dessous d'environ 100 ms, l'œil cumule la luminosité d'un flash (loi de Bloch). Des flashs à pleine force de durées variables se lisent comme un scintillement | loi de Bloch | P | **Gardé** : un bouton reste dessiné appuyé au moins 60 ms. Le jeu, lui, reçoit le relâchement à l'instant. |
| Les kits d'interface ajoutent un fondu au relâchement ou un temps d'affichage minimum | Android `RippleForeground` (au moins 225 ms, puis fondu de 150 ms) ; Unity uGUI (100 ms) ; Chromium iOS (70 ms) ; Flutter | U | **Gardé** : le bouton revient en 60 ms, le halo s'efface en 120 ms. |
| Un gros changement à chaque appui distrait : PPSSPP a réduit l'agrandissement de ×2 à ×1,5 | PPSSPP #12987, a9a45481c | U | **Gardé** : halo réduit à 2,4 fois le rayon et 40 % d'opacité. |
| Ombres et flous sont des passes de rendu supplémentaires : en changer le rayon à chaque appui coûte | doc Apple (ProMotion, `shadowPath`) | P | **Gardé** : rayon d'ombre fixe, seuls décalage et opacité changent. Plus de flou sur le halo de L et R. |
| Le simulateur, piloté à la souris, ne rend pas comme un vrai iPhone | — | A | **À vérifier** sur un vrai iPhone (Instruments, rendu hors écran). |

## Croix

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Les directions droites l'emportent : 50° pour les axes, 40° pour les diagonales | RetroArch (sensibilité diagonale de 80 % par défaut) ; Delta, Dolphin, PPSSPP | P | **Gardé.** |
| La direction reste tant que le doigt ne se lève pas, même hors de la croix | PPSSPP ce4705586 (« Sticky D-Pad ») | U | **Gardé.** |
| Un mode 4 directions pour les jeux à menus | TAK `exclusive` | U | **À faire** quand un jeu le demandera. |

## Vibrations et iOS

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Un impact moyen est « trop fort, flou ». Mieux : impact léger à l'appui, tic de sélection au relâchement | Delta #57 ; MAME4iOS 2d449ec | P | **Gardé.** |
| Créer le générateur une fois et le préparer : sa préparation dure quelques secondes et chaque vibration la consomme | Delta ac0d42b ; doc Apple de `prepare()` | P | **Gardé** : préparation quand un doigt se pose et après chaque vibration. |
| On doit pouvoir couper les vibrations | HIG *Playing haptics* | U | **À faire** (réglage). |
| Les gesture recognizers retardent les touchers près des bords, même avec les gestes système différés | Delta 39cc979 ; Apple Forums 654645 | P | **Gardé** : `touchesBegan`/`touchesMoved` bruts dans une `UIView`, gestes système différés partout. **À mesurer** sur un vrai iPhone. |
| Un toucher annulé relâche tout | Delta ; PPSSPP | U | **Gardé** (et `release_all` quand l'app passe en arrière-plan). |
| SwiftUI se limite à 60 Hz sur un iPhone ProMotion sans `CADisableMinimumFrameDurationOnPhone` | source secondaire | P | **Gardé** (`ios/Info.plist`). |
| Les touchers prédits (`predictedTouches`) sont prévus pour le dessin et sont provisoires | Apple | A | **Jeté.** |
| GCVirtualController a une disposition figée (un stick et quatre boutons par côté) | WWDC21 | L | **Jeté** : la N64 a trop de boutons. |

## Connexion et format envoyé aux jeux

Le contrat complet pour les jeux est dans `CONTRAT.md`.

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| C'est le jeu qui choisit la télécommande, et il en change selon la phase de jeu | Jackbox (chaque phase a son écran de téléphone) ; décision de l'hôte | U | **Gardé** : pas de menu dans l'app. `GameMessage::Show`, à tout moment. |
| Un preset et une composition sont la même chose : des contrôles placés depuis des repères de l'écran | Touch Controller d'Apple (9 repères, décalages en points) | U | **Gardé** : `Layout`, et les presets en sont des exemples. |
| Il faut aussi une voie sans aucune contrainte | demande de l'hôte | U | **Gardé** : `Remote::Web`, la page du jeu dans WebKit. WebKit est le seul moteur autorisé à exécuter du code téléchargé à pleine vitesse sur iOS. |
| Le QR du jeu suffit : même hôte, même port, chemin `/remote`, même requête | décision, pour ne rien demander de plus aux jeux | U | **Gardé** (`join.rs`, testé). |
| Les entrées sont un état, pas des événements : un seul état en attente, le dernier gagne, au plus 60 envois par seconde, et un rappel régulier | Sidequest (`movement-sender.ts`, 13 paquets → 1) | P | **Gardé** (`session.rs`) : à chaque changement, toutes les 250 ms sinon, avec un `seq`. |
| Le dessin, lui, est fait d'événements qui doivent tous arriver, dans l'ordre | scribble.rs | P | **Gardé** : le WebSocket les livre ; ils partent avant l'état. |
| L'identité survit à une reconnexion, sans compte | Sidequest (`PartyRoom.ts`, jeton anonyme) | U | **Gardé** : 16 octets gardés par l'app, renvoyés dans chaque `Hello`. |
| Reconnexion avec attente croissante : 0,5 s, doublée, plafonnée à 5 s | Sidequest | U | **Gardé** (`GameConnection.swift`). |
| Ce qui vient du réseau ne doit pas pouvoir faire planter | manifeste durci (`expect_used`, `panic`, `indexing_slicing`) | P | **Gardé** : une trame illisible est ignorée, un contrôle mal formé (identifiant au-delà de 31, taille négative ou infinie) est écarté, testé. |
| La vraie manette N64 envoie 4 octets (boutons de libultra, X et Y signés) | libultra ; libjoybus | L | **Jeté** : tous les presets parlent le même `InputState`. Le gate octogonal, lui, reste. |
| Une pression plus courte qu'une image ne doit pas se perdre | TAK, déduction | P | **Gardé** : le cœur traite chaque toucher, et l'état part dès la prochaine image. |
| Safari sur iPhone n'a aucune vibration, demande la permission des capteurs à chaque visite, ne garde pas le plein écran ni l'orientation, et ne parle pas UDP | WebKit | P | **Gardé** : la page libre reçoit tout cela par des messages à l'app et des événements en retour (`CONTRAT.md`). |
| La seule porte entre une page et l'app est un objet JavaScript de WebKit ; une page en Rust y passe par la colle que génère `wasm-bindgen` | WebKit ; manifeste (« JavaScript limité à la colle générée ») | P | **Gardé** : aucun JavaScript écrit à la main dans l'app, qui n'injecte qu'une donnée. Le crate `remote-web` donne le tout en Rust ; les messages bruts restent documentés pour les autres langages. |
| Une page qui en remplace une autre ne doit rien en hériter (orientation demandée, données injectées) | constaté au premier essai | U | **Gardé** : une vue web neuve par page, qui repart en paysage ; l'orientation est redite à la fin du chargement, car un tour pendant le chargement arrive avant que la page écoute. |
| Le micro et la caméra marchent dans une page de l'app, mais WebKit redemande la permission après iOS | API `WKUIDelegate` d'Apple | U | **Gardé** : accordés sans seconde question aux pages de l'hôte du jeu, et à elles seules. |
| Une page vient du réseau : ses demandes de vibration ou de sockets ne doivent rien pouvoir casser | déduction | P | **Gardé** : 64 événements et 5 s par vibration au plus, 8 sockets, datagrammes de 65 507 octets au plus, UDP vers l'hôte du jeu seulement. |
| Une composition vient du jeu, donc du réseau : un contrôle placé à `f32::MAX` ou un stick de course infime mettaient le bouton du stick à NaN, que UIKit refuse | tests de propriétés (`CERTIFICATION.md`, E1) | P | **Gardé** : un contrôle centré à plus d'un écran du bord est écarté, et la course d'un stick vaut au moins 1 pt (`Layout::resolve`). |
| Donner à du contenu web l'accès à des fonctions natives demande en principe l'accord d'Apple sur l'App Store public | App Review Guidelines 4.7.2 | U | **Écart assumé** sur TestFlight ; à revoir avant une publication publique. |

Seuils de perception :
- un appui en retard de moins d'environ 60 ms passe inaperçu (Jota et al., CHI 2013) ;
- un glisser se perçoit dès quelques ms (Ng et al., UIST 2012).

Le bouton du stick dessiné sur le téléphone doit donc suivre le doigt dans la même image. Le budget de 80 ms du manifeste s'applique au trajet jusqu'à la TV.

## Les autres télécommandes

Trois recherches de plus, une par famille :
- **le buzzer, le pad deux pouces et la croix** : les skins NES et Game Boy de Delta (en points, sur une grille de 812×375), les overlays de RetroArch, Buzz! (pilote `hid-sony` de Linux), Kahoot, les règles de *Jeopardy!*, AirConsole ;
- **le volant et le pointeur** : SuperTuxKart, PPSSPP, Dolphin (`EmulateIMUCursor`), RetroArch, les écrits de Jibb Smart (GyroWiki, JoyShockMapper), libogc, le filtre 1€, CoreMotion ;
- **le dessin** : JackboxDrawer (le format des dessins Jackbox), scribble.rs, perfect-freehand, tldraw, Excalidraw, signature_pad, la doc Apple des touchers.

### Pad deux pouces, croix, buzzer

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Peu de boutons, gros, aux coins ou sous les pouces, pour jouer sans regarder | AirConsole | P | **Gardé** : le moteur commun, des presets à 2 à 4 boutons de jeu. |
| A et B sur une diagonale de 40 à 45°, B en bas à gauche, assez proches pour qu'un pouce sur la jointure presse les deux | Delta (NES 45°, Game Boy ~39°) ; RetroArch | U | **Gardé** : 40° pour le pad, 45° pour la croix, 16 à 20 pt d'écart. |
| Start et Select petits, au centre, sans zone élargie : on ne doit pas les toucher par accident | Delta ; RetroArch | U | **Gardé** : boutons « mineurs » sans marge, 28 pt minimum au lieu de 44. |
| Croix de 130 à 150 pt, centre à ~100 pt du bord | Delta | P | **Gardé** : 150 pt, centre à 110 pt. |
| Turbo, Game Boy en portrait, skins interchangeables | Delta, Nostalgia.NES | L | **Jeté.** |
| Le buzzer réagit au contact, pas au relâchement | Delta (`touchesBegan`) | P | **Gardé** : tout l'écran est le bouton. |
| La lumière du buzzer annonce la décision de l'hôte, pas l'appui : le joueur voit « appuyé » tout de suite, puis « premier » ou « trop tard » quand le jeu le dit | Buzz! (`buzz_set_leds`) | U | **À faire** avec la connexion : états Attente / Armé / Premier / Trop tard. |
| Buzzer trop tôt bloque environ 0,25 s, sinon les joueurs martèlent | *Jeopardy!* (règle de 1985) | U | **À faire** côté jeu. |
| Le Wi-Fi a une longue traîne (p90 ~20 ms, p99 ~250 ms) : l'ordre d'arrivée ment sur les courses serrées. Il faut l'heure du toucher, recalée sur l'horloge de l'hôte | Microsoft Research ; algorithme de Cristian | P | **À faire** avec la connexion : on garde déjà `UITouch.timestamp`. |

### Volant

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Lire la gravité fusionnée par iOS, pas l'accéléromètre brut ni des angles d'Euler | PPSSPP f84e63f7 ; doc CoreMotion | P | **Gardé** (`deviceMotion.gravity`). |
| L'angle = élévation du grand côté du téléphone : il ne dépend pas de l'inclinaison avant-arrière, donc le volant marche tenu droit ou à plat | PPSSPP `TiltEventProcessor` ; SuperTuxKart | P | **Gardé**, testé. |
| Braquage complet entre 19,5° (SuperTuxKart) et 32° (PPSSPP), zone morte de ~2,3° | code de SuperTuxKart et PPSSPP | U | **Gardé** : 25° et 2,5°, linéaire. **À faire** : un réglage de sensibilité. |
| Téléphone presque à plat : l'angle ne veut plus rien dire, on garde le dernier | SuperTuxKart | P | **Gardé** (sous 0,3 g dans le plan de l'écran). |
| Pas de calibration : la gravité est absolue | SuperTuxKart, PPSSPP | P | **Gardé.** |
| Mario Kart Wii ne lit que 15 niveaux de braquage : la finesse ne fait pas le plaisir | Dolphin, bug 8756 | U | On envoie une valeur continue, le jeu arrondit s'il veut. |
| Le simulateur n'a pas de capteurs | Apple | — | Message affiché ; **à vérifier sur un vrai iPhone**, signe compris. |

### Pointeur

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Pointeur relatif, à partir de la vitesse de rotation. Pas le magnétomètre, qu'une TV perturbe | Dolphin ; déduction | P | **Gardé.** |
| Gauche-droite mesuré autour de la vraie verticale (« player space ») : même sensation quelle que soit l'inclinaison | Jibb Smart, GyroWiki | P | **Gardé** (vitesse de rotation projetée sur la gravité). |
| Environ 30° pour traverser l'écran (Dolphin 25°, JoyShockMapper ~45°), mêmes degrés par point en hauteur | Dolphin ; JoyShockMapper | P | **Gardé** : 30°. |
| Le curseur s'arrête au bord, ce qui recentre comme une souris ; un bouton Centrer | Dolphin `EmulateIMUCursor` | U | **Gardé.** |
| Lissage « par paliers » : lissé en dessous de ~5°/s (tremblement), jamais au-dessus | Jibb Smart | U | **Gardé** (0,125 s). |
| Un trou dans les échantillons n'est pas un mouvement | SuperTuxKart (dt plafonné) | A / P | **Gardé** : au-delà de 0,1 s, ignoré. |
| Un glisser du doigt en secours (pas de gyroscope, réglage fin) | Dolphin Android | U | **Gardé** : tout l'écran hors boutons. |
| Envoyer la position absolue du curseur, pas des écarts : un paquet perdu ne décale rien | déduction | P | **Gardé** (`PointerState`). |
| Le signe du « vers le haut » dépend de l'orientation de l'interface | RetroArch `cocoa_input.m` | P | **Gardé** ; **à vérifier sur un vrai iPhone**. |

### Dessin

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| Des traits en coordonnées entières dans une toile fixe, pas des images : Jackbox est passé du PNG aux vecteurs | JackboxDrawer | P / L | **Gardé** : toile 4096×2304 (16:9 comme la TV), entiers signés pour qu'un trait déborde sans être coupé (scribble.rs). |
| Traits avec identifiant : Annuler et Effacer deviennent triviaux | scribble.rs #279 (sans identifiant, il faut deviner les traits) | L | **Gardé** : `DrawEvent` Begin / Point / End / Undo / Clear. |
| Un jeu qui change de télécommande en plein trait doit quand même recevoir la fin du trait : le socket « livre tous » les traits | tests de propriétés (`CERTIFICATION.md`) | U | **Gardé** : avant de remplacer la télécommande, la session lâche tous les doigts et envoie ce qui restait dans l'ancienne toile, `End` compris (`Session::let_go`). |
| Un seul doigt dessine | scribble.rs | U | **Gardé.** |
| UIKit livre les touchers à ~60 Hz alors que l'écran échantillonne à 120 Hz et plus : il faut les touchers fusionnés | doc Apple (`coalescedTouches`) | P | **Gardé.** |
| Les touchers prédits servent seulement à l'affichage local, jamais envoyés | doc Apple | A | **Pas encore utilisés.** |
| Ignorer les points à moins de ~1,5 pt, lisser par une moyenne dans le temps (pas par échantillon), finir sur le vrai dernier point | perfect-freehand, signature_pad, tldraw | P | **Gardé** (12 ms). |
| Une tape fait un point | perfect-freehand | U | **Gardé.** |
| Épaisseur constante : pour deviner, lisible bat joli | Excalidraw (option ajoutée en 2026) | U | **Gardé.** |
| Dessiner des courbes par les milieux, pas des segments | scribble.rs #368 | P | **Gardé** (côté Swift). |
| Peu d'outils : 8 à 12 couleurs, 3 tailles, Annuler, Effacer ; pas de pot de peinture ni de formes | scribble.rs, Drawful 2 | U | **Gardé** : 7 couleurs (44 pt chacune), 3 tailles. Effacer demande deux appuis en 2 s. |

## Écarts au manifeste

Un point par principe d'ingénierie du manifeste : **Respecté.**, **Écart.** (avec sa « Raison : ») ou **À faire.** `tools/check.sh`, à la racine du dépôt, vérifie que les sept y sont.

1. **À faire.** Le clippy du manifeste passe, sans `unwrap` ni index qui peut paniquer, et ce qui vient d'un jeu est validé avant usage : trames illisibles ignorées, contrôles mal formés écartés, identifiants bornés à 31, le tout testé. Reste la coquille Swift, hors lints et hors tests : la durée d'affichage minimale d'un appui, le choix des vibrations, l'attribution des doigts, la reconnexion.
2. **Écart.** Pas de simulation à pas fixe. Raison : la télécommande n'est pas un jeu ; elle lit les doigts et envoie un état, et la simulation vit dans chaque jeu. Le temps entre tout de même comme paramètre dans les gestes (dessin, pointeur).
3. **Respecté.** Le temps est une entrée, la trigonométrie passe par `libm`, tout est en `BTreeMap`, et les tests tournent à temps fixe.
4. **Respecté.** `remote-core` ne dépend que de `remote-protocol` et `libm`, et ses tests tournent sans iOS.
5. **Écart.** Raison : c'est une app iOS native, pas une page web ; COOP/COEP, WebGPU et les threads WASM ne s'y appliquent pas.
6. **Écart.** Le transport est un WebSocket, donc TCP, pas un canal non fiable. Raison : un seul canal porte l'état et les traits de dessin, qui doivent arriver tous et dans l'ordre, et chaque jeu a déjà un serveur WebSocket ; en Wi-Fi local, la perte est rare. Pour limiter le retard d'un paquet bloqué : l'état est complet (le dernier suffit), numéroté (`seq`), envoyé au plus 60 fois par seconde, et le faux jeu coupe Nagle (`TCP_NODELAY`). `Hello` porte la version du protocole, et `remote-protocol` décode tout côté jeu. Passer l'état en UDP reste possible si la latence mesurée l'exige (point 7).
7. **À faire.** La latence entre le toucher, l'envoi et la TV n'est pas mesurée (voir « Vibrations et iOS »).
