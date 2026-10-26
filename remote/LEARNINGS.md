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

## Format envoyé aux jeux

| Leçon | Source | Cat. | Conséquence ici |
|---|---|---|---|
| 4 octets comme la vraie manette : boutons (bits de libultra), puis X et Y signés, Y positif vers le haut | `os_cont.h` de libultra ; libjoybus ; Mario Kart 64 | P | **Gardé** (`remote-protocol`). |
| La vraie manette lève un bit « reset » pendant L+R+Start pour se recalibrer | n64brew Joybus | L | **Jeté** : rien à recalibrer, le bit reste à 0. |
| Une pression plus courte qu'une image ne doit pas se perdre | TAK, déduction | P | Le cœur traite chaque toucher. **À garder** quand on enverra au jeu. |

Seuils de perception :
- un appui en retard de moins d'environ 60 ms passe inaperçu (Jota et al., CHI 2013) ;
- un glisser se perçoit dès quelques ms (Ng et al., UIST 2012).

Le bouton du stick dessiné sur le téléphone doit donc suivre le doigt dans la même image. Le budget de 80 ms du manifeste s'applique au trajet jusqu'à la TV.
