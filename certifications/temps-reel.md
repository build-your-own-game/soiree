# Temps réel et sensation des commandes (`TR`)

> Pour un jeu où le temps compte : plateformes, course, bagarre, réflexes, buzzer, rythme. Ce fichier porte sur ce que la simulation fait des entrées et du temps, pas sur leur transport : la latence, les paquets perdus et la reconnexion sont l'affaire de `salon/`. Lis aussi `egalites.md` (deux entrées dans le même tick) et `physique.md`.

Le manifeste impose déjà la simulation à pas fixe et le temps comme entrée (principes 2 et 3). Ces items vérifient que le jeu en tire vraiment les garanties, et que les commandes ont la sensation des bons jeux du genre.

### TR-01 · Le même temps, découpé autrement en images, donne exactement la même partie
- **Mécaniques :** toute boucle qui accumule le temps réel et fait avancer des ticks ; tout minuteur, usure, recharge ou chrono.
- **Le bug d'origine :** dans le moteur de Quake 3 (Enemy Territory), sans `pmove_fixed`, la hauteur du saut dépend de la fréquence d'images, et 333 images/s sautent plus haut ([source](https://etjump.readthedocs.io/en/3.5.3/getting_started.html)). Dans la bêta PC de Fallout 76 (2018), débloquer la fréquence d'images faisait courir plus vite ([source](https://kotaku.com/fallout-76-betas-physics-are-tied-to-its-framerate-1830140345)). Dans Dark Souls 2, les armes s'usaient bien plus vite à 60 images/s qu'à 30, et la correction a pris plus d'un an ([source](https://www.kitguru.net/gaming/matthew-wilson/that-dark-souls-2-weapon-durability-bug-has-finally-been-fixed/)). Defold comptait 1/60 s par synchronisation verticale, d'où un jeu 2,4 fois trop rapide sur un écran à 144 Hz ([source](https://forum.defold.com/t/update-frequency-bug-for-windows-def-3262-solved/19302)).
- **Propriété :** pour toute suite d'entrées horodatées et tout découpage d'une même durée T en images (rafales d'images nulles, pics de 250 ms, cadences de 24 à 144 Hz), l'état final est identique au bit près, et le nombre de ticks exécutés vaut `T / dt` arrondi vers le bas.
- **Test :** générer `Vec<u32>` de durées d'images (en µs), puis une seconde découpe de même somme ; faire tourner la boucle d'accumulation du jeu (la vraie, sortie dans `crates/core`) sur les deux et comparer `state_hash`.
  ```rust
  proptest! {
      #![proptest_config(config())]
      #[test]
      fn frame_rate_never_changes_the_game(seed in any::<u64>(), inputs in inputs(), frames in frames(), cut in any::<prop::sample::Index>()) {
          let other = resplit(&frames, cut); // même somme, autre découpe
          prop_assert_eq!(run(seed, &inputs, &frames).state_hash(), run(seed, &inputs, &other).state_hash());
      }
  }
  ```
- **Adapter :** si ta boucle d'accumulation vit dans le crate web, sors-la dans `core` pour la tester. Chaque compteur de jeu (usure, recharge, chrono) doit compter en ticks ou en temps simulé, jamais en images de rendu. Voisins : TR-02, D4 du commun (hash natif et WASM).

### TR-02 · Après un gel de l'hôte, la boucle ne s'emballe pas pour rattraper le retard
- **Mécaniques :** la boucle à pas fixe sur l'hôte ou la TV.
- **Le bug d'origine :** Glenn Fiedler décrit la « spirale de la mort » : si un pas coûte plus de temps réel qu'il n'en simule, le retard grandit, la boucle simule encore plus de pas, et le jeu gèle ; le remède est de plafonner le nombre de pas par image ([source](https://gafferongames.com/post/fix_your_timestep/)).
- **Propriété :** pour toute suite de durées d'images contenant des pics (de 0 à 10 s, onglet caché, Mac qui sort de veille), une image exécute au plus `MAX_STEPS` ticks, l'accumulateur reste borné, et le temps simulé ne dépasse jamais le temps réel.
- **Test :** générer des durées d'images avec des pics rares ; après chaque image, vérifier le nombre de ticks exécutés et la valeur de l'accumulateur.
- **Adapter :** décide ce qui arrive au temps perdu au-delà du plafond (jeté, le jeu ralentit) et écris-le. Pour un jeu par échéances (quiz), l'équivalent est C6 et D2 du commun. Voisins : TR-01, TR-10 (pause).

### TR-03 · Un appui plus court qu'un tick n'est jamais perdu
- **Mécaniques :** saut, tir, buzzer, tout bouton qui déclenche une action à l'appui.
- **Le bug d'origine :** dans Godot, une action pressée puis relâchée entre deux ticks de physique n'était jamais vue comme « juste pressée » ; le correctif garde l'heure de l'appui et celle du relâchement séparément ([source](https://github.com/godotengine/godot/commit/a3ef092f18842b369183f43772fc47830e2924ff)). Dans Defold (issue #5044), un relâchement suivi d'un nouvel appui dans la même image ne produisait aucun message ([source](https://github.com/defold/defold/issues/5044)).
- **Propriété :** pour toute action A et tout tick t, une file d'entrées contenant `Appui(A)` puis `Relâché(A)` entre t−1 et t déclenche A exactement une fois ; plus généralement, le nombre de déclenchements de A égale le nombre d'appuis, quelle que soit leur durée.
- **Test :** générer des paires appui/relâchement de largeur 0 à 3 ticks, placées au hasard ; compter les déclenchements.
- **Adapter :** si le téléphone envoie un état (« bouton tenu ») plutôt que des événements, l'appui court peut disparaître entre deux états : envoie un compteur d'appuis dans l'état, ou un événement fiable pour les actions ponctuelles. Voisins : TR-04, TR-07 (un tap donne le saut minimal).

### TR-04 · Un appui n'est jamais vu deux fois, et un bouton tenu ne se répète pas tout seul
- **Mécaniques :** saut, tir, validation, tout « juste pressé ».
- **Le bug d'origine :** dans Bevy (issue #6183, ouverte), quand le pas fixe tourne deux fois dans une image, `just_pressed` est vu deux fois, et zéro fois quand il ne tourne pas ([source](https://github.com/bevyengine/bevy/issues/6183)). Dans la bibliothèque Coffee (issue #127), la répétition automatique du clavier de macOS produisait de nouveaux appuis pendant qu'on tenait la touche ([source](https://github.com/hecrj/coffee/issues/127)).
- **Propriété :** pour toute suite d'événements où `Appui(A)` est reçu k fois sans `Relâché(A)` entre eux, A est déclenché une seule fois ; pour tout nombre de ticks par image (0, 1, plusieurs), chaque appui est vu exactement une fois.
- **Test :** générer des doublons d'appui et des cadences d'images variées ; compter les déclenchements, et comparer avec la même suite sans doublons.
- **Adapter :** un message renvoyé par le réseau est un doublon comme un autre ; le même raisonnement vaut pour les votes (VOTE-02). Voisins : TR-03, ECO-11.

### TR-05 · Un saut pressé juste après avoir quitté un rebord part quand même, et une seule fois
- **Mécaniques :** jeux de plateformes, tout saut depuis le sol (« coyote time »).
- **Le bug d'origine :** Maddy Thorson explique que Celeste laisse sauter un court instant après avoir quitté un rebord, pour pardonner un appui un peu tardif ([source](https://maddymakesgames.com/articles/celeste_and_forgiveness/index.html)). Dans le mini-jeu JunimoKart de Stardew Valley, la grâce était testée dans la fonction de saut, mais cette fonction n'était appelée qu'au sol : la grâce ne servait jamais. Corrigé en 1.6 ([source](https://forums.stardewvalley.net/threads/bug-coyote-time-not-working-in-junimokart-because-of-a-bug.20671)).
- **Propriété :** pour tout k ≤ la fenêtre de grâce (en ticks), un joueur qui quitte le sol en marchant et presse saut k ticks plus tard saute ; au-delà, il ne saute pas ; pour toute suite d'entrées, il n'y a jamais deux sauts sans contact avec le sol entre les deux (hors double saut voulu).
- **Test :** niveau d'un seul rebord ; générer la vitesse de marche et l'instant de l'appui. Vérifier aussi qu'après un vrai saut, la grâce ne donne pas un second saut.
- **Adapter :** remplace le rebord par toute perte de support (plateforme qui disparaît, tête d'un autre joueur). Voisins : TR-06, TR-07.

### TR-06 · Un saut pressé juste avant d'atterrir part à l'atterrissage, et une seule fois
- **Mécaniques :** plateformes, tout bouton pressé un peu trop tôt (tampon d'entrée).
- **Le bug d'origine :** dans Celeste, un saut pressé peu avant d'atterrir part à l'image exacte de l'atterrissage ; le code consomme ce tampon (`Input.Jump.ConsumeBuffer()`) ([source](https://maddymakesgames.com/articles/celeste_and_forgiveness/index.html)). Le blog de GameMaker décrit la même marge, de 4 images ([source](https://gamemaker.io/en/blog/flynn-advanced-jump-mechanics)).
- **Propriété :** pour tout appui à N ticks avant l'atterrissage, si N ≤ `BUFFER`, le saut part au tick de l'atterrissage, une fois ; si N > `BUFFER`, il ne part pas ; un seul appui ne déclenche jamais deux sauts sur deux atterrissages successifs.
- **Test :** générer la hauteur de chute et N ; vérifier le tick du saut. Enchaîner deux rebonds pour vérifier que le tampon est vidé.
- **Adapter :** le tampon vaut pour d'autres actions : attaque pendant une animation, virage avant un carrefour. Voisins : TR-05, TR-03.

### TR-07 · Tenir le saut plus longtemps ne donne jamais un saut plus bas
- **Mécaniques :** saut à hauteur variable, charge d'un tir, tout effet qui dépend de la durée d'appui.
- **Le bug d'origine :** dans Celeste, tant que le minuteur de saut variable tourne et que le bouton est tenu, la vitesse verticale est maintenue ; relâcher coupe la poussée (`VarJumpTime`, `varJumpTimer`) ([source](https://raw.githubusercontent.com/NoelFB/Celeste/master/Source/Player/Player.cs)). Steve Swink rappelle que dans Mario, tenir le bouton plus longtemps donnait un saut plus haut ([source](https://www.gamedeveloper.com/design/game-feel-the-secret-ingredient)).
- **Propriété :** pour toutes durées de maintien h1 < h2, le sommet du saut avec h1 est au plus celui avec h2 ; au-delà de la fenêtre, le sommet ne change plus ; un tap de 0 tick donne le saut minimal (lien avec TR-03).
- **Test :** générer h de 0 à deux fois la fenêtre ; mesurer le sommet par simulation ; vérifier la monotonie sur des paires.
- **Adapter :** remplace la hauteur par la grandeur que ton jeu module (puissance d'un tir, distance d'un lancer). Voisins : TR-05, TR-06.

### TR-08 · Aller en diagonale n'est jamais plus rapide (ni plus lent) qu'en ligne droite
- **Mécaniques :** déplacement à quatre ou huit directions, croix directionnelle du téléphone, joystick virtuel.
- **Le bug d'origine :** dans Minecraft (MC-271065), la diagonale n'était pas normalisée en marche accroupie et allait 40 % plus vite ; corrigé en 25w02a, puis rouvert ([source](https://mojira.dev/MC-271065)).
- **Propriété :** pour tout angle θ et toute amplitude a dans [0, 1], la vitesse atteinte vaut `a × VMAX` à ε près, quel que soit θ ; pour toute combinaison de directions discrètes, la norme de l'entrée appliquée est au plus 1.
- **Test :** générer θ et a, plus les huit directions de la croix ; simuler jusqu'à la vitesse de croisière et la mesurer.
- **Adapter :** si ton jeu veut des diagonales plus rapides (style rétro), écris-le et teste cette règle-là. Voisins : TR-09, PHY-03.

### TR-09 · Une valeur d'axe absurde ou un bruit au repos ne fait pas bouger le personnage
- **Mécaniques :** joystick virtuel, gyroscope, inclinaison, toute entrée analogique venue d'un téléphone.
- **Le bug d'origine :** dans l'API Gamepad (w3c/gamepad, issue #150), une manette ne renvoyait que −∞, 0 ou +∞ dans Chrome ([source](https://github.com/w3c/gamepad/issues/150)). Dans OpenMW (issue #5502), une zone morte trop petite faisait dériver la caméra et le personnage sans que personne touche à rien ([source](https://gitlab.com/OpenMW/openmw/-/issues/5502)). Chez nous, Sidequest génère des axes hors bornes et absurdes dans ses sessions ([`sidequest/crates/core/tests/certification/model.rs`](https://github.com/build-your-own-game/sidequest/blob/main/crates/core/tests/certification/model.rs), `axis`).
- **Propriété :** pour toute entrée `(x, y)` parmi tous les flottants (NaN, ±∞, ±1e30, sous-normaux, hors de [−1, 1]), l'état reste fini et le déplacement par tick est au plus `VMAX × dt` ; pour toute entrée de norme inférieure à la zone morte, la position ne change pas sur N ticks.
- **Test :** générer les axes avec ces valeurs aux extrémités ; vérifier le déplacement par tick et la finitude. Pour la zone morte, générer un bruit de faible amplitude pendant N ticks.
- **Adapter :** remplace l'axe par toutes tes entrées numériques (angle d'un lancer, puissance). La zone morte se règle sur le téléphone, mais se vérifie dans le cœur. Voisins : PHY-04, TR-08.

### TR-10 · Une pause gèle tout, et une nouvelle partie ne commence jamais en pause
- **Mécaniques :** pause de l'organisateur, menus, écrans de transition qui doivent arrêter le jeu.
- **Le bug d'origine :** dans Microsoft Flight Simulator 2024, les chronos de course continuaient pendant la pause ([source](https://forums.flightsimulator.com/t/rally-race-timer-continues-while-game-paused/710465)). Dans GNOME Nibbles, la pause restait enregistrée après le lancement d'une nouvelle partie, et le correctif la remet à zéro ([source](https://mail.gnome.org/archives/commits-list/2020-June/msg04510.html)). Chez nous, Sidequest repousse l'échéance d'une manche exactement du temps passé en pause (`the_deadline_is_within_the_duration`).
- **Propriété :** pour tout état et toute durée de pause P, l'état après « pause, P, reprise » est égal à l'état avant la pause, à l'horloge près : minuteurs, positions, chronos, échéances décalées de P exactement ; les entrées tenues au moment de la pause sont relâchées ; une nouvelle partie part toujours sans pause.
- **Test :** générer une partie, insérer une pause à un instant au hasard, attendre P, reprendre ; comparer avec la même partie sans pause, en décalant les instants suivants de P.
- **Adapter :** liste tout ce qui compte le temps dans ton état. Voisins : TR-02, PROG-03 (rien ne survit à une relance).

### TR-11 · Une direction tenue cesse d'agir dès qu'on n'en a plus de nouvelles, et une vieille direction ne fait plus bouger
- **Mécaniques :** toute commande tenue envoyée par un téléphone : direction, accélérateur, visée.
- **Le bug d'origine :** chez nous, Sidequest Live (le hackathon d'origine) : un paquet bloqué derrière TCP et livré en retard a fait bouger un joueur de 22 px. Sidequest borne donc l'âge de chaque direction (une seconde, sur l'horloge de l'hôte), et lâche une direction tenue 450 ms après sa dernière nouvelle ; un « stop » passe toujours ([`LEARNINGS.md` de Sidequest](https://github.com/build-your-own-game/sidequest/blob/main/LEARNINGS.md), « Leçons triées » et « Salon », test `a_press_delivered_late_does_not_move` dans [`sidequest/crates/core/tests/original/movement.rs`](https://github.com/build-your-own-game/sidequest/blob/main/crates/core/tests/original/movement.rs)).
- **Propriété :** pour toute suite d'entrées, une direction posée il y a plus de `MAX_AGE` ne déplace personne ; une direction tenue sans rafraîchissement depuis `DEAD_MAN` est relâchée ; un relâchement est toujours accepté, quel que soit son âge.
- **Test :** générer des directions avec leur heure de pose, livrées avec un retard tiré entre 0 et 3 s ; vérifier qu'aucune direction trop vieille ne change la position, et qu'un joueur sans nouvelles s'arrête au plus tard à `DEAD_MAN`.
- **Adapter :** salon ne livre que l'état le plus récent, mais il peut renvoyer un état ancien tel quel (page gelée) ou passer par le WebSocket de secours après un blocage : la borne d'âge reste au jeu. Pour un jeu sans commande tenue, écarte l'item. Voisins : TR-09, C3 du commun.

### TR-12 · Une fenêtre de réussite est symétrique, monotone, et ses bords sont fixés une fois pour toutes
- **Mécaniques :** jeux de rythme, réflexes (« appuie quand ça passe au vert »), tir au bon moment, QTE.
- **Le bug d'origine :** le wiki officiel d'osu! documente qu'en mode standard l'erreur est arrondie et les bornes tronquées, si bien que les fenêtres réelles peuvent être plus étroites que les formules d'environ 1,5 ms de chaque côté, et que le mode mania compare avec ≤ là où le standard compare avec < ([source](https://osu.ppy.sh/wiki/en/Gameplay/Judgement/osu%21)).
- **Propriété :** pour tout décalage d (en ticks ou en ms entières), `juge(+d) == juge(−d)` ; `juge` ne s'améliore jamais quand |d| grandit ; au bord W, `juge(±W)` suit la règle choisie (inclusive ou exclusive) et la même partout.
- **Test :** générer d autour de chaque bord (W−1, W, W+1) et au hasard ; vérifier symétrie, monotonie et bords.
- **Adapter :** remplace les fenêtres par celles de ton jeu, et le décalage par l'écart entre l'appui (heure de l'hôte, estimée par salon) et la cible. Voisins : EGA-03 (deux réussites au même tick), TR-03.
