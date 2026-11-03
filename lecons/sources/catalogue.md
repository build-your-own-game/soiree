# Leçons du catalogue de certification

> Source couverte : le catalogue `soiree/certifications/` de l'ancien code, c'est-à-dire `README.md` et les dix fichiers de mécaniques (`physique`, `temps-reel`, `egalites`, `vote-quiz`, `tour-par-tour`, `economie`, `progression`, `texte`, `hasard`, `affichage`). Il comptait 97 items. Ce fichier les garde tous, sous la forme d'une leçon du [contrat](../../CONTRAT.md).

## Ce que j'ai lu

- `MANIFESTE.md` et `CONTRAT.md`, en entier : ils font foi pour le classement.
- `certifications/README.md` et les dix fichiers, en entier. Le dossier n'a pas changé depuis l'étiquette `avant-refonte` (503b386). Son historique : le commit 7ff2bd3 (27 octobre 2026) crée les 97 items ; 6764852 et c9ea75c ne touchent que le README (passage de `CERTIFICATION.md` à `verification.toml`). Aucun item n'a été retiré ni renuméroté.
- Les lignes de `soiree/CERTIFICATION.md` que le catalogue cite (C2, C3, C6, C7, C9, D1, D2, D4, D5, E5, T1, T2, T3, R2), pour comprendre ses renvois.
- Les `catalogue_decisions` et les `guarantees` des `verification.toml` de `games/punchline`, `games/sidequest` et `games/tier-list` : chacun décide des 97 items. Les décisions disent quels items ont une pression réelle, et deux d'entre elles révèlent des défauts du catalogue.
- L'emplacement des tests et des sections de `LEARNINGS.md` que le catalogue cite, pour donner des chemins précis.

## Ce que je n'ai pas pu lire

- Les sources externes (issues, wikis, articles) n'ont pas été rouvertes : leur contenu est ici tel que le catalogue l'a résumé. Le catalogue disait les avoir ouvertes et lues ; il signale lui-même les sources faibles, et je garde ces signalements.
- Le corps des tests des jeux : je les ai localisés, pas relus. Une pression citée ici existe ; je n'ai pas vérifié qu'elle échoue quand on casse la règle.
- Le reste de `CERTIFICATION.md`, `VERIFICATION.md` et les `LEARNINGS.md` des jeux : ce sont d'autres sources, hors de cette tâche.

## Comment lire ce fichier

- Chaque item du catalogue devient au moins une leçon, avec son identifiant d'origine entre parenthèses.
- Sept items mêlaient deux leçons qui ne vivent pas au même étage : PHY-04, TR-09, VOTE-06, VOTE-09, TOUR-07, ECO-04 et PROG-02. Chacun est coupé en deux leçons, qui gardent le même identifiant d'origine.
- Dix-sept leçons viennent du README, des introductions des fichiers et des décisions des jeux (CAT-105 à CAT-121).
- Les renvois du type « C6 du commun » désignent les items de l'ancien `CERTIFICATION.md`.
- « Aucun de nos jeux » veut dire : les trois jeux ont écarté l'item comme sans objet. La leçon n'a alors jamais été éprouvée chez nous.

Compte : 121 leçons. Impossible par la forme : 10. Pression du commun : 27. Question de mécanique : 75. Question de jeu : 0. Connaissance : 8. Obsolète : 1.

Ce que le classement montre :

- Le temps, les entrées, la graine, les prénoms et les sauvegardes passent au commun. Les items qui en parlaient deviennent des pressions du commun, ou disparaissent par la forme.
- La règle « un événement à la fois » ne règle pas l'équité. Le manifeste dit qu'un jeu qui traite un événement à la fois ne connaît pas deux actions simultanées. C'est vrai des données, pas des règles : traiter deux coups mortels l'un après l'autre reproduit le bug d'Entelect (CAT-28).
- Aucun item n'est propre à un seul jeu : le catalogue avait été écrit pour se répéter d'un jeu à l'autre.

---

## Physique (`physique.md`)

### CAT-01 · Un corps rapide ne traverse jamais un mur fin sans le toucher (PHY-01)
- **Ce qui arrive :** avec un pas trop grand, un objet rapide passe d'un côté d'un mur à l'autre entre deux pas, sans jamais le toucher. Même lent, un coin qui tourne peut aller plus vite que la marge de détection et traverser un sol en triangles.
- **D'où on le sait :** `soiree/certifications/physique.md:7`. Glenn Fiedler, « Fix your timestep » (https://gafferongames.com/post/fix_your_timestep/). Rapier, issue #524, 2023 (https://github.com/dimforge/rapier/issues/524). Wiki de DDNet, « Collision » (https://wiki.ddnet.org/wiki/Collision).
- **Mécaniques :** physique : projectiles, chutes, dash, tout corps dont la vitesse par pas approche l'épaisseur d'un obstacle.
- **Question :** ton objet le plus rapide peut-il franchir ton obstacle le plus fin en un pas ? Fais-tu une détection continue, ou bornes-tu la vitesse par pas sous l'épaisseur minimale, et l'as-tu écrit ?
- **Pression :** propriété générée : un mur de l'épaisseur minimale, un corps lancé vers lui à toute vitesse jusqu'au plafond, sous tout angle ; après chaque pas, le corps est du même côté qu'au départ ou en contact. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-02 · Marcher sur un sol plat fait de tuiles ne bloque jamais à une jointure (PHY-02)
- **Ce qui arrive :** une boîte qui glisse sur deux arêtes alignées est arrêtée à leur jonction : en détection discrète, la plus courte sortie du chevauchement pointe vers l'arrière (« collision fantôme »).
- **D'où on le sait :** `soiree/certifications/physique.md:29`. Erin Catto, Box2D, « Ghost collisions », 2020 (https://box2d.org/posts/2020/06/ghost-collisions/).
- **Mécaniques :** physique : sols et murs en tuiles, niveaux en blocs alignés, boîtes de collision rectangulaires, pentes raccordées.
- **Question :** un sol plat découpé en N tuiles donne-t-il la même trajectoire qu'un sol d'un seul tenant ?
- **Pression :** propriété : N de 1 à 50, largeurs au hasard de somme fixe ; même entrée « avancer » sur les deux sols ; positions égales pas par pas (à ε près en flottants), vitesse horizontale jamais nulle avant le bout. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-03 · Aucune combinaison d'entrées ne dépasse le plafond de vitesse, dans un sens comme dans l'autre (PHY-03)
- **Ce qui arrive :** un plafond appliqué dans un seul sens, ou à l'accélération au lieu de la vitesse, laisse la vitesse grandir sans fin. Dans Super Mario 64, le saut en longueur multiplie la vitesse par 1,5 et ne la plafonne qu'en positif : à reculons, elle explose (le « BLJ »). Le bunny hopping de Quake vient d'une limite posée sur l'accélération.
- **D'où on le sait :** `soiree/certifications/physique.md:36`. Code décompilé de Super Mario 64, cas `ACT_LONG_JUMP` (https://github.com/n64decomp/sm64/blob/master/src/game/mario.c). Wikipédia, « Bunny hopping » (https://en.wikipedia.org/wiki/Bunny_hopping). Chez nous : `nobody_outruns_the_speed_cap`, `games/sidequest/crates/core/tests/certification/invariants.rs:21`.
- **Mécaniques :** physique : accélération, sauts enchaînés, rebonds, poussées entre joueurs, boosts.
- **Question :** pour chaque objet, quel est le plafond de `|vx|`, `|vy|` et `|v|`, y compris dans les directions négatives et pendant un boost ?
- **Pression :** invariant vérifié après chaque pas des sessions générées, avec un générateur qui vise les cas limites (saut à chaque pas, demi-tours, appui contre un mur, foule qui pousse). Sidequest J2 (`random_sessions_keep_every_invariant`, `normalizes_diagonals_and_bounds_speed`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-04 · Une entrée non finie n'entre jamais dans la simulation (PHY-04, côté entrées)
- **Ce qui arrive :** une valeur `NaN` venue d'un appareil passe un refus écrit `x <= 0.0`, qui est faux pour `NaN`. Elle entre dans l'état. Chez Sidequest, un « stop » la mémorisait ensuite, et le replay ne se refaisait plus au bit près.
- **D'où on le sait :** `soiree/certifications/physique.md:43`. `games/sidequest/LEARNINGS.md:175` (« Bail `NaN` »). Tests `a_lease_that_is_not_a_number_is_refused` (`games/sidequest/crates/core/tests/certification/sessions.rs:256`) et `a_stop_with_a_nan_lease_replays_bit_for_bit` (`games/sidequest/crates/core/tests/replays.rs`).
- **Mécaniques :** toute entrée qui porte un nombre : axe, durée, angle, puissance, bail.
- **Question :** un type d'entrée que le commun livre au jeu peut-il porter `NaN`, `±∞`, un sous-normal ou `-0.0` ?
- **Pression :** générateur d'entrées qui place `NaN`, `-NaN`, `±∞`, `±1e300`, sous-normaux et `-0.0` dans chaque champ numérique. Sidequest J3.
- **Sous la nouvelle forme :** impossible par la forme. Ce qui vient de l'extérieur est validé à l'entrée, par des types (`CONTRAT.md:135`) : un nombre fini est un type que seul le commun construit. Il reste une connaissance utile partout : refuser par `!(x > 0.0)`, jamais par `x <= 0.0`.
- **Étage :** commun.

### CAT-05 · Aucune position, vitesse ou force ne devient NaN ou infinie (PHY-04, côté simulation)
- **Ce qui arrive :** une force `NaN` passe dans la vitesse, la position, puis la détection de collisions, et contamine les corps en contact. Une friction infinie multipliée par zéro donne `NaN`.
- **D'où on le sait :** `soiree/certifications/physique.md:43`. Godot 4.4, issue #104359 (https://github.com/godotengine/godot/issues/104359). Box2D, issue #434 (https://github.com/erincatto/box2d/issues/434).
- **Mécaniques :** physique, et toute simulation en flottants.
- **Question :** chaque champ flottant de ton état reste-t-il fini après chaque pas, y compris aux réglages extrêmes autorisés ?
- **Pression :** après chaque pas, parcourir l'état et vérifier `is_finite()`. Sidequest J43 (`a_long_evening_never_diverges`, `games/sidequest/crates/core/tests/original/physics.rs:25`).
- **Sous la nouvelle forme :** question de mécanique. La forme garantit des entrées finies, pas des calculs finis. Le constat de déterminisme du commun (deux exécutions comparées) verra une dérive de `NaN`, pas sa cause.
- **Étage :** mécanique.

### CAT-06 · Une direction de longueur nulle ou deux centres confondus ne corrompent rien (PHY-05)
- **Ce qui arrive :** normaliser une vitesse nulle donne `NaN`. Deux corps au même centre n'ont pas de normale de séparation. Dans Rapier, un pas avec `dt = 0` donnait des positions `NaN` aux corps reliés par une articulation.
- **D'où on le sait :** `soiree/certifications/physique.md:50`. Rapier, issue #659, 2024 (https://github.com/dimforge/rapier/issues/659) ; issue #77 (https://github.com/dimforge/rapier/issues/77). Chez nous : `separates_exact_overlapping_spawns_without_nan`, `games/sidequest/crates/core/tests/original/physics.rs:86`.
- **Mécaniques :** physique : normalisation de vecteurs, normale de contact, corps superposés.
- **Question :** pour une entrée `(0, 0)` ou `(-0.0, 0.0)`, la direction calculée est-elle finie, et le corps reste-t-il immobile ? Deux corps de même centre se séparent-ils selon une règle écrite ?
- **Pression :** générer des paires de corps au même centre ; après un pas, positions finies et distinctes. Sidequest J4.
- **Sous la nouvelle forme :** question de mécanique. La moitié « pas de durée nulle » disparaît : le commun fournit des pas réguliers (`CONTRAT.md:27`). Elle devient : un pas sans entrée ne fait pas bouger un corps au repos.
- **Étage :** mécanique.

### CAT-07 · Aucun corps ne sort du monde ; s'il tombe dans un trou, il revient en jeu en un temps borné (PHY-06)
- **Ce qui arrive :** un kart passe sous la piste, on ajoute des plans de remise en piste, et le bug revient au même endroit. Un objet en chute libre sans sol fait déborder un calcul de grille en `i32` et paniquer le moteur.
- **D'où on le sait :** `soiree/certifications/physique.md:57`. SuperTuxKart, issue #1817 (https://github.com/supertuxkart/stk-code/issues/1817). Rapier, issue #77 (https://github.com/dimforge/rapier/issues/77). Chez nous : `players_stay_on_the_floor`, `games/sidequest/crates/core/tests/certification/invariants.rs:20`.
- **Mécaniques :** physique : arènes fermées, plateformes avec vide, pistes, objets éjectés.
- **Question :** quelle est la boîte de ton monde, et en combien de pas un corps sorti est-il replacé, ou éliminé ?
- **Pression :** générateur qui pousse vers les bords et les trous, avec des poussées de foule ; après chaque pas, position dans la boîte ou compteur « hors jeu depuis » au plus K. Sidequest J1 (`keeps_a_crowded_group_inside_the_map`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-08 · Sans entrée, l'énergie totale n'augmente jamais (PHY-07)
- **Ce qui arrive :** des balles aux chocs élastiques accélèrent peu à peu jusqu'à disparaître, parce que la séparation des corps qui se chevauchent ajoute de l'énergie. Un pendule de Newton parfaitement élastique gagne de l'énergie.
- **D'où on le sait :** `soiree/certifications/physique.md:64`. Godot 3.1 à 4.2, issue #29639 (https://github.com/godotengine/godot/issues/29639). Rapier, issue #630, 2024 (https://github.com/dimforge/rapier/issues/630).
- **Mécaniques :** physique : rebonds, chocs entre joueurs, balles, billard, pendules, restitution.
- **Question :** qu'est-ce qui se conserve ou décroît dans ton jeu sans entrée (énergie, vitesse totale, hauteur des rebonds) ?
- **Pression :** 1 à 20 corps au hasard, restitution au plus 1, aucune entrée, 600 pas : l'énergie au pas t+1 est au plus celle du pas t, à ε près ; une balle lâchée ne rebondit jamais plus haut que son départ. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-09 · Un tas d'objets laissé seul finit par se poser, sans trembler ni s'enfoncer (PHY-08)
- **Ce qui arrive :** des corps empilés dans une fosse se chevauchent nettement et tremblent sans fin. Monter la physique à 180 Hz ne fait qu'atténuer. Le bug est resté ouvert de 2015 à 2021.
- **D'où on le sait :** `soiree/certifications/physique.md:71`. Godot, issue #2092 (https://github.com/godotengine/godot/issues/2092).
- **Mécaniques :** physique : empilements, caisses, joueurs entassés dans un coin, foule dans une zone.
- **Question :** une pile de N corps lâchés sans entrée se pose-t-elle en au plus T pas, avec une pénétration toujours sous la tolérance ?
- **Pression :** N au plus 30, positions de départ au hasard au-dessus d'un sol ; pénétration maximale mesurée à chaque pas, vitesses sous ε après T pas. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-10 · Deux joueurs ne restent jamais coincés l'un dans l'autre (PHY-09)
- **Ce qui arrive :** le code de collision coince deux personnages l'un dans l'autre pendant plusieurs pas. Le rapporteur soupçonne le correctif qui empêchait de se traverser.
- **D'où on le sait :** `soiree/certifications/physique.md:78`. Teeworlds, issue #1261 (https://github.com/teeworlds/teeworlds/issues/1261). Chez nous : Sidequest place chaque apparition au premier point libre à 28 px de tout le monde.
- **Mécaniques :** physique : collisions entre joueurs, apparition, téléportation, poussées.
- **Question :** un chevauchement entre deux corps disparaît-il en au plus K pas ? Deux points d'apparition peuvent-ils se recouvrir ?
- **Pression :** départs superposés exactement ou en partie, entrées au hasard, 2 à 8 joueurs ; compter les pas consécutifs de chevauchement par paire. Sidequest J5 (`never_spawns_on_top_of_anyone_even_past_the_grid`, `games/sidequest/crates/core/tests/original/rules.rs`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-11 · Un corps poussé contre un mur n'entre jamais dedans ni ne passe de l'autre côté (PHY-10)
- **Ce qui arrive :** un corps cinématique ne garde pas sa marge et finit superposé au corps qu'il pousse. Un personnage à inertie infinie fait traverser les murs aux objets qu'il pousse.
- **D'où on le sait :** `soiree/certifications/physique.md:85`. Godot 3.1 à 3.5, issue #31981 (https://github.com/godotengine/godot/issues/31981). KidsCanCode, « Character vs rigid » (https://kidscancode.org/godot_recipes/4.x/physics/character_vs_rigid/).
- **Mécaniques :** physique : pousser un objet ou un joueur, écrasement entre deux corps ou contre un mur, plateformes qui écrasent.
- **Question :** que se passe-t-il quand un corps est écrasé : le pousseur s'arrête, ou le corps est éliminé ou replacé ? La règle est-elle écrite ?
- **Pression :** scène « mur, corps, 1 à 3 pousseurs du même côté » ; après chaque pas, le corps poussé est du même côté du mur et hors de lui. Sidequest J1 (le bord de l'arène est son seul solide).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-12 · Un joueur posé sur une plateforme mobile ne passe pas au travers et ne flotte pas au-dessus (PHY-11)
- **Ce qui arrive :** sur une plateforme verticale à sens unique, le joueur s'enfonce d'un pixel en montée puis tombe au travers, et flotte d'un pixel en descente.
- **D'où on le sait :** `soiree/certifications/physique.md:92`. Forum Godot, plateformes mobiles et collisions à sens unique (https://forum.godotengine.org/t/issue-with-moving-platforms-and-one-way-collisions/25265) ; l'option « Sync to Physics » arrive en 3.1.
- **Mécaniques :** physique : plateformes qui montent, descendent ou glissent, ascenseurs, tapis roulants, dos d'un autre joueur, véhicule.
- **Question :** sans saut, le bas du joueur reste-t-il égal au haut de son support à chaque pas ?
- **Pression :** vitesse, sens de la plateforme et instant d'atterrissage tirés ; comparer les deux hauteurs à chaque pas pendant une période. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-13 · Une longue partie ou un réglage extrême ne fait jamais diverger la simulation (PHY-12)
- **Ce qui arrive :** après plusieurs jours, un serveur voit ses rotations et translations passer à des millions, puis à `NaN`, puis planter. Un sol très large (demi-largeur de 1e6 à 1e8) fait gagner de l'énergie aux objets, et certains le traversent.
- **D'où on le sait :** `soiree/certifications/physique.md:99`. Box2D, issue #508 (https://github.com/erincatto/box2d/issues/508). Rapier, issue #629 (https://github.com/dimforge/rapier/issues/629).
- **Mécaniques :** physique : simulation qui tourne toute une soirée, mondes très grands, masses très différentes.
- **Question :** sur une soirée de 6 h (environ 1,3 million de pas à 60 Hz), positions et vitesses restent-elles sous leurs bornes ? Aux réglages extrêmes autorisés, CAT-05, CAT-07 et CAT-08 tiennent-elles encore ?
- **Pression :** test long, lancé à côté et non à chaque changement. Sidequest J43 (`a_long_evening_never_diverges`), sur 100 000 pas et non le million.
- **Sous la nouvelle forme :** question de mécanique. Le commun fait tourner les longues pressions en continu (`MANIFESTE.md:45`) et mesure la mémoire ; la divergence des nombres reste au jeu.
- **Étage :** mécanique.

---

## Temps réel (`temps-reel.md`)

### CAT-14 · Le même temps, découpé autrement en images, donne exactement la même partie (TR-01)
- **Ce qui arrive :** un jeu qui avance d'un pas par image va plus ou moins vite selon l'écran. Quake 3 sans `pmove_fixed` saute plus haut à 333 images/s. Dans la bêta de Fallout 76, débloquer la fréquence faisait courir plus vite. Dans Dark Souls 2, les armes s'usaient bien plus vite à 60 images/s qu'à 30, et la correction a pris plus d'un an. Defold comptait 1/60 s par synchronisation verticale : 2,4 fois trop vite sur un écran à 144 Hz.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:7`. ETJump, documentation (https://etjump.readthedocs.io/en/3.5.3/getting_started.html). Kotaku sur Fallout 76 (https://kotaku.com/fallout-76-betas-physics-are-tied-to-its-framerate-1830140345). KitGuru sur Dark Souls 2 (https://www.kitguru.net/gaming/matthew-wilson/that-dark-souls-2-weapon-durability-bug-has-finally-been-fixed/). Forum Defold, DEF-3262 (https://forum.defold.com/t/update-frequency-bug-for-windows-def-3262-solved/19302).
- **Mécaniques :** toute boucle qui accumule le temps réel ; tout minuteur, usure, recharge, chrono ; toute animation de page.
- **Question :** pour une même durée T, découpée en images de n'importe quelle façon (rafales d'images nulles, pics de 250 ms, 24 à 144 Hz), la boucle du commun livre-t-elle exactement `T / dt` pas, arrondi vers le bas ? Une animation de page qui ne passe pas par les pas avance-t-elle au temps et non à l'image ?
- **Pression :** propriété sur la boucle d'accumulation : deux découpes de même somme donnent le même hash d'état. Sidequest J45 (`the_steps_due_do_not_depend_on_how_time_is_cut`, `games/sidequest/crates/host/src/schedule.rs:85`). Tier-list J4 (`autoscroll_distance_does_not_depend_on_display_frequency`), pour une animation de page.
- **Sous la nouvelle forme :** pression du commun. Le temps avance par pas réguliers, fournis par le commun (`CONTRAT.md:27`) : un jeu ne voit jamais les images, il ne peut pas compter en images. La boucle qui transforme des images en pas est au commun, écrite et éprouvée une fois pour tous. Reste au jeu ce qu'il dessine lui-même sur sa page.
- **Étage :** commun.

### CAT-15 · Après un gel, la boucle ne s'emballe pas pour rattraper le retard (TR-02)
- **Ce qui arrive :** si un pas coûte plus de temps réel qu'il n'en simule, le retard grandit, la boucle simule encore plus de pas, et le jeu gèle : la « spirale de la mort ». Le remède est un plafond de pas par image. Dans le réel : onglet caché, Mac qui sort de veille, pics de 10 s.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:24`. Glenn Fiedler, « Fix your timestep » (https://gafferongames.com/post/fix_your_timestep/).
- **Mécaniques :** la boucle à pas fixe sur l'ordinateur de la soirée ; toute partie à échéances.
- **Question :** après un gel, combien de pas le commun rattrape-t-il par image, et que devient le temps au-delà (jeté, la partie ralentit) ? Une échéance qui tombe pendant le gel enchaîne-t-elle plusieurs phases d'un coup ?
- **Pression :** durées d'images avec des pics rares de 0 à 10 s ; après chaque image, au plus `MAX_STEPS` pas, accumulateur borné, temps simulé jamais en avance sur le temps réel. Sidequest J7 (`a_stalled_host_catches_up_a_few_steps_and_drops_the_rest`, `games/sidequest/crates/host/src/schedule.rs:70`). Punchline J5 (`a_long_freeze_does_not_run_through_the_phases_it_slept_over`, `a_late_tick_does_not_shorten_the_next_phase`, `games/punchline/crates/core/src/tests.rs`). Tier-list J4 (un réveil compte pour 50 ms au plus).
- **Sous la nouvelle forme :** pression du commun. Le commun fournit les pas et donne un budget à chacun (`CONTRAT.md:27`, `CONTRAT.md:30`). Il garantit aussi que la partie qui fait foi tourne quand son onglet passe au second plan, ou le signale (`CONTRAT.md:72`).
- **Étage :** commun.

### CAT-16 · Un appui plus court qu'un pas n'est jamais perdu (TR-03)
- **Ce qui arrive :** une action pressée puis relâchée entre deux pas n'est jamais vue comme « juste pressée ». Un relâchement suivi d'un nouvel appui dans la même image ne produit aucun message. Chez Sidequest, la direction est envoyée comme un état : un appui plus court qu'un envoi peut disparaître.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:31`. Godot, commit a3ef092 (https://github.com/godotengine/godot/commit/a3ef092f18842b369183f43772fc47830e2924ff) : le correctif garde l'heure de l'appui et celle du relâchement. Defold, issue #5044 (https://github.com/defold/defold/issues/5044). Sidequest a laissé l'item « unreviewed » : « à juger avec de vrais téléphones » (`games/sidequest/verification.toml`, décision TR-03).
- **Mécaniques :** saut, tir, buzzer, tout bouton qui déclenche à l'appui.
- **Question :** le commun livre-t-il chaque appui comme un événement, même plus court qu'un pas ou qu'un intervalle d'envoi du téléphone ? Le jeu déclenche-t-il à l'événement, sans échantillonner un état « tenu » ?
- **Pression :** paires appui/relâchement de 0 à 3 pas placées au hasard : le nombre de déclenchements égale le nombre d'appuis. Aucun de nos jeux ne l'a tenue.
- **Sous la nouvelle forme :** pression du commun. Le commun possède les entrées (`CONTRAT.md:43`) et distingue un état d'un événement : un événement compte exactement une fois (`CONTRAT.md:101`). Un appui est un événement. Le jeu, qui reçoit les entrées une à une (`CONTRAT.md:22`), le voit forcément.
- **Étage :** commun.

### CAT-17 · Un appui n'est jamais vu deux fois, et un bouton tenu ne se répète pas tout seul (TR-04)
- **Ce qui arrive :** quand le pas fixe tourne deux fois dans une image, `just_pressed` est vu deux fois, et zéro fois quand il ne tourne pas. La répétition automatique du clavier de macOS produit de nouveaux appuis pendant qu'on tient la touche. Un message renvoyé par le réseau est un doublon comme un autre.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:38`. Bevy, issue #6183, ouverte (https://github.com/bevyengine/bevy/issues/6183). Coffee, issue #127 (https://github.com/hecrj/coffee/issues/127).
- **Mécaniques :** saut, tir, validation, tout « juste pressé ».
- **Question :** le commun écarte-t-il la répétition automatique du système et les renvois du réseau ? Un jeu qui veut la répétition (Ctrl+Z dans un éditeur, comme la tier-list) peut-il la demander ?
- **Pression :** doublons d'appui et cadences d'images variées ; chaque appui est vu une fois. Sidequest J9 (`jumps_have_a_strict_750_ms_cooldown`, `games/sidequest/crates/core/tests/original/rules.rs`), pour le saut seulement.
- **Sous la nouvelle forme :** pression du commun. La moitié Bevy disparaît par la forme : le jeu reçoit chaque entrée une fois, il ne relit pas un drapeau à chaque image. Restent la répétition du système et les doublons du réseau, que le commun traite pour tous (`CONTRAT.md:39`).
- **Étage :** commun.

### CAT-18 · Un saut pressé juste après avoir quitté un rebord part quand même, et une seule fois (TR-05)
- **Ce qui arrive :** sans grâce, un appui un peu tardif après le rebord ne saute pas, et le joueur le vit comme une injustice. Celeste laisse sauter un court instant après le rebord (« coyote time »). Dans JunimoKart (Stardew Valley), la grâce était testée dans la fonction de saut, appelée seulement au sol : elle ne servait jamais, jusqu'à la 1.6.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:45`. Maddy Thorson, « Celeste and forgiveness » (https://maddymakesgames.com/articles/celeste_and_forgiveness/index.html). Forum Stardew Valley (https://forums.stardewvalley.net/threads/bug-coyote-time-not-working-in-junimokart-because-of-a-bug.20671).
- **Mécaniques :** saut, plateformes, toute perte de support.
- **Question :** quelle est ta fenêtre de grâce en pas ? Après un vrai saut, la grâce peut-elle donner un second saut ?
- **Pression :** un niveau d'un seul rebord ; vitesse de marche et instant de l'appui tirés ; saut si k est au plus la fenêtre, pas au-delà ; jamais deux sauts sans contact au sol entre eux. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-19 · Un saut pressé juste avant d'atterrir part à l'atterrissage, et une seule fois (TR-06)
- **Ce qui arrive :** un saut pressé un peu trop tôt est perdu. Celeste le garde en tampon et le fait partir à l'image exacte de l'atterrissage, puis vide le tampon. GameMaker décrit la même marge, de 4 images.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:52`. Maddy Thorson, même article. Blog de GameMaker (https://gamemaker.io/en/blog/flynn-advanced-jump-mechanics).
- **Mécaniques :** saut, et tout bouton pressé un peu trop tôt : attaque pendant une animation, virage avant un carrefour.
- **Question :** quelle est la taille de ton tampon ? Un seul appui peut-il déclencher deux sauts sur deux atterrissages successifs ?
- **Pression :** hauteur de chute et avance N tirées ; le saut part au pas de l'atterrissage si N est au plus le tampon ; deux rebonds enchaînés vident le tampon. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-20 · Tenir le saut plus longtemps ne donne jamais un saut plus bas (TR-07)
- **Ce qui arrive :** dans les bons jeux, la durée d'appui module la hauteur : Celeste maintient la vitesse verticale tant que le minuteur de saut variable tourne et que le bouton est tenu ; Mario sautait plus haut quand on tenait plus longtemps. Une modulation non monotone se sent tout de suite.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:59`. Code source de Celeste, `VarJumpTime` (https://raw.githubusercontent.com/NoelFB/Celeste/master/Source/Player/Player.cs). Steve Swink, « Game feel » (https://www.gamedeveloper.com/design/game-feel-the-secret-ingredient).
- **Mécaniques :** saut à hauteur variable, charge d'un tir, tout effet qui dépend de la durée d'appui.
- **Question :** pour deux maintiens h1 < h2, le sommet avec h1 est-il au plus celui avec h2 ? Au-delà de la fenêtre, change-t-il encore ? Un tap de 0 pas donne-t-il le saut minimal ?
- **Pression :** h tiré de 0 à deux fois la fenêtre, sommet mesuré par simulation, monotonie vérifiée sur des paires. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-21 · Aller en diagonale n'est jamais plus rapide, ni plus lent, qu'en ligne droite (TR-08)
- **Ce qui arrive :** une diagonale non normalisée additionne deux axes et va environ 40 % plus vite. Dans Minecraft, en marche accroupie ; corrigé en 25w02a, puis rouvert.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:66`. Minecraft, MC-271065 (https://mojira.dev/MC-271065).
- **Mécaniques :** déplacement à quatre ou huit directions, croix du téléphone, joystick virtuel.
- **Question :** pour tout angle et toute amplitude a dans [0, 1], la vitesse atteinte vaut-elle `a × VMAX` ? Si ton jeu veut des diagonales plus rapides (style rétro), l'as-tu écrit ?
- **Pression :** angle et amplitude tirés, plus les huit directions de la croix ; vitesse de croisière mesurée. Sidequest J6 (`normalizes_diagonals_and_bounds_speed`, `games/sidequest/crates/core/tests/original/physics.rs`).
- **Sous la nouvelle forme :** question de mécanique. Si l'axe typé du commun garantit une norme au plus 1, la moitié « entrée » passe au commun ; la vitesse qui en sort reste au jeu.
- **Étage :** mécanique.

### CAT-22 · Une valeur d'axe absurde ne fait jamais bouger le personnage (TR-09, valeurs absurdes)
- **Ce qui arrive :** dans Chrome, une manette ne renvoyait que −∞, 0 ou +∞ par l'API Gamepad. Un téléphone hostile ou cassé peut envoyer `NaN`, `1e30` ou une valeur hors de [−1, 1].
- **D'où on le sait :** `soiree/certifications/temps-reel.md:73`. w3c/gamepad, issue #150 (https://github.com/w3c/gamepad/issues/150). Chez nous : le générateur `axis`, `games/sidequest/crates/core/tests/certification/model.rs:124`.
- **Mécaniques :** joystick virtuel, gyroscope, inclinaison, toute entrée analogique, angle ou puissance d'un lancer.
- **Question :** un axe livré par le commun peut-il sortir de [−1, 1] ou être non fini ?
- **Pression :** axes générés parmi tous les flottants ; état fini et déplacement par pas au plus `VMAX × dt`. Sidequest J3 (`rejects_malformed_movement`).
- **Sous la nouvelle forme :** impossible par la forme. Les entrées sont typées et validées à l'entrée (`CONTRAT.md:43`, `CONTRAT.md:135`) : le jeu ne reçoit qu'un axe fini et borné. La leçon du réel reste au commun, qui doit la connaître en écrivant ce type : le navigateur peut livrer n'importe quoi.
- **Étage :** commun.

### CAT-23 · Un bruit au repos ne fait pas bouger le personnage (TR-09, zone morte)
- **Ce qui arrive :** une zone morte trop petite fait dériver la caméra et le personnage sans que personne touche à rien.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:73`. OpenMW, issue #5502 (https://gitlab.com/OpenMW/openmw/-/issues/5502).
- **Mécaniques :** manettes, joystick virtuel, gyroscope, toute entrée analogique.
- **Question :** le commun applique-t-il une zone morte, réglée selon l'appareil, avant de livrer un axe ? Un bruit de faible amplitude pendant N pas laisse-t-il la position inchangée ?
- **Pression :** bruit sous la zone morte pendant N pas, position inchangée. Aucun de nos jeux : la manette de Sidequest n'envoie que −1, 0 ou 1.
- **Sous la nouvelle forme :** pression du commun. Les manettes et les téléphones sont au commun (`CONTRAT.md:43`) ; leur bruit au repos est un fait du matériel, le même pour tous les jeux.
- **Étage :** commun.

### CAT-24 · Une pause gèle tout, et une nouvelle partie ne commence jamais en pause (TR-10)
- **Ce qui arrive :** dans Microsoft Flight Simulator 2024, les chronos de course continuaient pendant la pause. Dans GNOME Nibbles, la pause restait enregistrée après le lancement d'une nouvelle partie.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:80`. Forum de Flight Simulator (https://forums.flightsimulator.com/t/rally-race-timer-continues-while-game-paused/710465). Liste des commits de GNOME, juin 2020 (https://mail.gnome.org/archives/commits-list/2020-June/msg04510.html). Chez nous : `the_deadline_is_within_the_duration` (`games/sidequest/crates/core/tests/certification/invariants.rs:28`) et `pause_freezes_everyone_and_shifts_the_deadline` (`games/sidequest/crates/core/tests/original/rules.rs`).
- **Mécaniques :** pause de l'organisateur, menus, écrans de transition.
- **Question :** qui possède la pause ? Si c'est le commun : gèle-t-il tous les pas, relâche-t-il les entrées tenues, et une nouvelle partie part-elle sans pause ? Si un jeu tient sa propre pause : tout ce qui compte le temps dans son état est-il décalé exactement de la durée de la pause ?
- **Pression :** insérer une pause de durée P à un instant au hasard, reprendre, comparer à la même partie sans pause, instants décalés de P. Sidequest J10.
- **Sous la nouvelle forme :** pression du commun. Le jeu ne connaît le temps que par les pas du commun (`CONTRAT.md:27`) : une pause qui suspend les pas gèle tout par construction, minuteurs compris. Le contrat ne dit pas encore qui possède la pause.
- **Étage :** commun.

### CAT-25 · Une direction tenue cesse d'agir dès qu'on n'en a plus de nouvelles, et une vieille direction ne fait plus bouger (TR-11)
- **Ce qui arrive :** au hackathon d'origine de Sidequest, un paquet bloqué derrière TCP et livré en retard a fait bouger un joueur de 22 px. Une page gelée peut aussi laisser une direction tenue sans fin.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:87`. `games/sidequest/LEARNINGS.md:23-24` et `:191` (« Salon ») : âge borné à une seconde sur l'horloge de l'hôte, direction lâchée 450 ms après sa dernière nouvelle, « stop » toujours accepté. Tests `a_press_delivered_late_does_not_move` (`games/sidequest/crates/core/tests/original/movement.rs:44`), `a_press_resent_with_its_old_time_stops_when_its_budget_runs_out`, `stops_after_a_controller_disappears`.
- **Mécaniques :** toute commande tenue venue d'un téléphone : direction, accélérateur, visée.
- **Question :** le commun borne-t-il l'âge d'un état tenu, et le relâche-t-il quand l'appareil se tait ? Un relâchement passe-t-il toujours, quel que soit son âge ?
- **Pression :** directions livrées avec un retard tiré entre 0 et 3 s ; aucune direction trop vieille ne change la position ; un joueur sans nouvelles s'arrête au plus tard après le délai. Sidequest J8.
- **Sous la nouvelle forme :** pression du commun. Un état périmé est remplacé par le plus récent (`CONTRAT.md:101`), et le repli sur WebSocket retient tous les paquets derrière un paquet perdu (`CONTRAT.md:100`). L'âge d'un état et le relâchement sans nouvelles se règlent une fois, dans le commun ; le jeu ne voit qu'une direction à jour.
- **Étage :** commun.

### CAT-26 · Une fenêtre de réussite est symétrique, monotone, et ses bords sont fixés une fois pour toutes (TR-12)
- **Ce qui arrive :** dans osu!, l'erreur est arrondie et les bornes tronquées : les fenêtres réelles sont plus étroites que les formules d'environ 1,5 ms de chaque côté. Le mode mania compare avec ≤ là où le mode standard compare avec <.
- **D'où on le sait :** `soiree/certifications/temps-reel.md:94`. Wiki officiel d'osu!, « Judgement » (https://osu.ppy.sh/wiki/en/Gameplay/Judgement/osu%21).
- **Mécaniques :** rythme, réflexes, tir au bon moment, QTE.
- **Question :** pour tout décalage d, `juge(+d)` vaut-il `juge(−d)` ? Le jugement s'améliore-t-il jamais quand |d| grandit ? Le bord est-il inclusif ou exclusif, et le même partout ?
- **Pression :** d tiré autour de chaque bord (W−1, W, W+1) et au hasard ; symétrie, monotonie, bords. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique. L'instant du geste est estimé par le commun ; la fenêtre et son jugement sont au jeu.
- **Étage :** mécanique.

---

## Égalités et simultanéité (`egalites.md`)

### CAT-27 · Le numéro d'un joueur ne décide jamais seul qui gagne une simultanéité (EGA-01)
- **Ce qui arrive :** dans les premiers Super Smash Bros., les joueurs sont traités dans l'ordre des manettes : le port le plus bas gagne les prises simultanées et les objets ramassés au même instant. Brawl est passé au hasard. Dans RuneScape Classic, le plus petit numéro de connexion ramassait l'objet disputé et frappait le premier.
- **D'où on le sait :** `soiree/certifications/egalites.md:7`, et l'idée commune du fichier (`egalites.md:5`). SSBWiki, « Port priority » (https://www.ssbwiki.com/Port_priority). RuneScape Classic Wiki, « Player ID » (https://classic.runescape.wiki/w/Player_ID). Chez nous : `the_order_of_arrival_changes_nothing`, `games/punchline/crates/core/src/tests/properties.rs:1737`.
- **Mécaniques :** ramassage, coups, buzz, ligne d'arrivée dans le même pas ; toute boucle `for player in players`.
- **Question :** l'ordre que le commun fixe entre deux gestes du même instant dépend-il de l'identité des appareils ? Le résultat change-t-il quand le commun renumérote les identités qu'il crée ?
- **Pression :** propriété d'équivariance : simuler une permutation des joueurs donne la permutation du résultat. Punchline J30.
- **Sous la nouvelle forme :** pression du commun. Le commun fixe l'ordre des entrées (`CONTRAT.md:22`) et crée seul les identifiants (`CONTRAT.md:42`). Il peut départager deux gestes du même instant par la graine, puis rejouer une partie avec des identifiants permutés et comparer les vues de chaque appareil, pour tous les jeux. Ce qu'un jeu fait à l'intérieur d'un pas reste à lui : voir CAT-28.
- **Étage :** commun.

### CAT-28 · Des actions simultanées sont toutes évaluées sur l'état d'avant, puis appliquées ensemble (EGA-02)
- **Ce qui arrive :** dans le tournoi de bots d'Entelect (bataille navale), quand le dernier navire d'un joueur coulait pendant un tour, le dernier tir de l'adversaire était rejeté : le joueur traité en premier gagnait. La 1.0.2 évalue tous les ordres avant toute mort. Dans Smash, deux coups de dégâts proches qui se rencontrent s'annulent tous les deux.
- **D'où on le sait :** `soiree/certifications/egalites.md:25`. Forum d'Entelect (https://forum.entelect.co.za/t/same-turn-kill-shot-loss-or-draw/78). SSBWiki, « Clang » (https://www.ssbwiki.com/Clang).
- **Mécaniques :** coups mortels échangés, deux joueurs sur la dernière carte, tirs croisés, tour simultané.
- **Question :** inverser l'ordre de traitement des gestes d'un même pas change-t-il le résultat ? Deux coups mortels au même pas donnent-ils un double K.O. ou une règle symétrique écrite ?
- **Pression :** scènes où deux actions se contrarient dans le même pas, résolues dans l'ordre et à l'envers, puis comparées. Punchline J30.
- **Sous la nouvelle forme :** question de mécanique. Le manifeste dit qu'un jeu qui traite un événement à la fois ne connaît pas deux actions simultanées (`MANIFESTE.md:55`). C'est vrai des données, pas de l'équité : appliquer deux coups mortels l'un après l'autre reproduit le bug d'Entelect. Un jeu qui veut une résolution simultanée rassemble les gestes d'un pas, les évalue sur un instantané, puis applique tout.
- **Étage :** mécanique.

### CAT-29 · Le classement d'une course suit les temps, et deux arrivées identiques reçoivent la même chose (EGA-03)
- **Ce qui arrive :** sur des arrivées serrées, le 2ᵉ affiche un meilleur temps que le 1er. Deux karts arrivés ensemble n'ont pas les mêmes points : le jeu n'a pas de notion de même position et départage toujours par une interpolation sous l'image.
- **D'où on le sait :** `soiree/certifications/egalites.md:32`. SuperTuxKart, issues #5107 (https://github.com/supertuxkart/stk-code/issues/5107) et #4571 (https://github.com/supertuxkart/stk-code/issues/4571).
- **Mécaniques :** courses, chronos, « premier qui trouve », buzzer, tout classement par instant d'arrivée.
- **Question :** quelle est la résolution d'une arrivée (le pas, ou l'instant interpolé), et la tiens-tu partout, affichage compris ? Deux temps égaux à cette résolution reçoivent-ils les mêmes rangs et points ?
- **Pression :** 2 à 8 coureurs franchissant la ligne dans le même pas, à des fractions variées dont des fractions identiques ; monotonie et égalités. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-30 · Des scores égaux ne sont jamais présentés comme la victoire de l'un (EGA-04)
- **Ce qui arrive :** une partie de Battle for Wesnoth finie par la limite de tours, avec un or égal, annonçait l'avantage au joueur 1 et la défaite au second : le calcul n'avait pas de cas d'égalité. Corrigé en 2018 (PR #3447).
- **D'où on le sait :** `soiree/certifications/egalites.md:39`. Wesnoth, issue #1341 (https://github.com/wesnoth/wesnoth/issues/1341).
- **Mécaniques :** fin de partie, fin de manche, duels, toute annonce d'un gagnant.
- **Question :** que fait ton jeu d'une égalité en tête (couronne partagée, manche de départage, critère secondaire) ? Le texte annoncé est-il tiré du même résultat ?
- **Pression :** forcer des scores égaux en tête (deux, trois, tous) ; vérifier le résultat et sa permutation. Punchline J20 (`equal_scores_share_their_rank`, `every_winner_wears_a_crown_wherever_their_row_falls`). Sidequest J30 (`equal_scores_share_their_rank_and_the_win`, `the_headline_says_what_the_standings_say`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-31 · Le classement ne dépend que des scores et de la règle de départage (EGA-05)
- **Ce qui arrive :** dans ClassQuiz, un joueur nommé « 4242 » reste en tête avec 0 point, parce que les clés numériques d'un objet JavaScript passent en premier ; corrigé en triant un tableau par score (PR #516). Le même projet classait un 2/10 devant un 5/10. En Rust, une `BTreeMap` indexée par nom trie par nom : le même piège sous une autre forme.
- **D'où on le sait :** `soiree/certifications/egalites.md:46`. ClassQuiz, issues #487 (https://github.com/mawoka-myblock/ClassQuiz/issues/487) et #107 (https://github.com/mawoka-myblock/ClassQuiz/issues/107).
- **Mécaniques :** tableau des scores, podium, classement en direct.
- **Question :** renommer les joueurs (noms numériques, vides, Unicode) ou les faire arriver dans un autre ordre change-t-il le classement ailleurs qu'entre ex æquo ?
- **Pression :** noms générés dont des nombres et des textes vicieux, scores avec collisions ; classer, renommer, réordonner, reclasser. Punchline J20, Sidequest J30.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-32 · Deux scores égaux ont toujours le même rang affiché (EGA-06)
- **Ce qui arrive :** dans le greffon Gamification de Discourse, les scores égaux étaient numérotés à la suite : un utilisateur apparaissait 30ᵉ au lieu de 32ᵉ. Un réglage de méthode de rang est arrivé en 2025.
- **D'où on le sait :** `soiree/certifications/egalites.md:53`. Discourse Meta (https://meta.discourse.org/t/display-same-rank-when-score-is-the-same/267579). Wikipédia, « Ranking », classement « 1224 » (https://en.wikipedia.org/wiki/Ranking).
- **Mécaniques :** classement, podium, « 3ᵉ sur 8 » sur le téléphone.
- **Question :** le rang vaut-il `1 + nombre de scores strictement plus grands` (ou le rang dense, si c'est écrit) ? L'ordre d'affichage des ex æquo est-il stable, et le même sur la télé et sur les téléphones ?
- **Pression :** scores tirés dans un petit intervalle pour forcer les égalités ; rang de chaque vue comparé à la formule. Punchline J20, Sidequest J30.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-33 · Une égalité ne bloque jamais la phase qui doit la trancher (EGA-07)
- **Ce qui arrive :** un système de vote se bloque en cas d'égalité. Source faible : l'issue n'a qu'un titre. Un revote sans borne laisse deux joueurs têtus bloquer la soirée.
- **D'où on le sait :** `soiree/certifications/egalites.md:60`. EvokeNet, moodle-block_missionmap, issue #41 (https://github.com/EvokeNet/moodle-block_missionmap/issues/41). Chez nous : Punchline réécrit sa règle de score dans le test et y traite le duel sans aucun vote (`expected_outcome`, `games/punchline/crates/core/src/tests/properties.rs:751`).
- **Mécaniques :** vote d'élimination, vote du meilleur, élection d'un rôle, départage, revote.
- **Question :** pour toute répartition des votes, égalités à deux ou à n et absence totale comprises, la phase produit-elle un résultat défini et passe-t-elle au plus tard à son échéance ? Le nombre de revotes est-il borné ?
- **Pression :** urnes aux comptes tirés dans {0, 1, 2} ; après l'échéance, la phase a changé et le résultat suit la règle. Punchline J13 (`a_duel_with_no_answer_pays_nobody`, `a_duel_pays_what_the_table_says`).
- **Sous la nouvelle forme :** question de mécanique. Le commun chasse les parties figées en général (CAT-78) ; forcer une égalité demande un scénario propre au vote.
- **Étage :** mécanique.

### CAT-34 · Quand un minuteur expire, le résultat tient compte de ce que l'autre pouvait encore faire, et le texte dit la même chose (EGA-08)
- **Ce qui arrive :** sur Lichess, des parties perdues au temps contre un adversaire qui ne pouvait plus mater étaient comptées comme défaites au lieu de nulles. Plus tard, ces parties étaient bien nulles, mais le PGN écrivait que l'un gagnait au temps. La règle FIDE 6.9 dit qu'un joueur dont le temps est écoulé perd, sauf si l'adversaire ne peut plus gagner.
- **D'où on le sait :** `soiree/certifications/egalites.md:67`. Lichess, issues #3617 (https://github.com/lichess-org/lila/issues/3617) et #10339 (https://github.com/lichess-org/lila/issues/10339). Manuel de la FIDE (https://handbook.fide.com/chapter/E012023).
- **Mécaniques :** pendule par joueur, temps de réflexion, défaite au temps, abandon.
- **Question :** le résultat est-il une fonction de l'état, de qui a expiré et de ce que les autres peuvent encore obtenir ? Le libellé est-il calculé depuis ce résultat, jamais à part ?
- **Pression :** états finaux et expirations au hasard, comparés à un modèle écrit dans le test ; libellé comparé au résultat. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique. Une vue recalculée depuis l'état ne peut pas se désynchroniser de l'état (`MANIFESTE.md:57`), mais deux fonctions qui décident chacune du gagnant le peuvent.
- **Étage :** mécanique.

---

## Vote et quiz (`vote-quiz.md`)

### CAT-35 · Un joueur a au plus une réponse et un vote par question, et « tout le monde a répondu » compte des joueurs distincts (VOTE-01)
- **Ce qui arrive :** dans ClassQuiz, le bouton d'envoi ne se désactivait pas et chaque envoi comptait. La question se fermait quand le nombre de réponses atteignait le nombre de joueurs : un seul joueur pouvait la fermer pour tous. Une vérification côté serveur a été ajoutée (PR #455).
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:7`. ClassQuiz, issues #447 (https://github.com/mawoka-myblock/ClassQuiz/issues/447) et #441 (https://github.com/mawoka-myblock/ClassQuiz/issues/441).
- **Mécaniques :** réponses, votes, mises, cartes jouées, fin anticipée d'une phase.
- **Question :** ce que ton jeu compte une fois par joueur l'est-il vraiment une fois, même quand le joueur envoie deux fois ? La fin anticipée compte-t-elle des joueurs distincts ?
- **Pression :** insérer des copies d'actions déjà envoyées à des positions au hasard ; après chaque pas, compter par (joueur, question). Punchline J27 (`votes_split_points_and_a_sweep_earns_the_bonus`). Sidequest J11 (`finishes_at_the_deadline_from_the_last_positions`).
- **Sous la nouvelle forme :** question de mécanique. Le commun écarte les renvois du réseau ; deux envois voulus par le joueur sont deux gestes, et restent au jeu.
- **Étage :** mécanique.

### CAT-36 · Renvoyer le même vote ne le change pas (VOTE-02)
- **Ce qui arrive :** dans Secret Hitler, un clic sur Ja ou Nein inversait le vote. Sur une connexion lente, les joueurs recliquaient, et leur vote se désélectionnait 10 à 15 s plus tard, au point de relancer des parties.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:14`. Secret Hitler, issue #1596 (https://github.com/cozuya/secret-hitler/issues/1596).
- **Mécaniques :** votes qu'on peut changer, « oui / non », prêt / pas prêt, coché / décoché, toute bascule.
- **Question :** une action envoie-t-elle l'état voulu (`Vote(x)`, `Retirer`), jamais « inverse » ? L'appliquer deux fois donne-t-il l'état d'une fois ?
- **Pression :** chaque action de vote répétée 1 à 3 fois, comparée à la suite sans répétition. Punchline J27.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-37 · Les points d'une manche ne sont comptés qu'une fois, et les totaux sont la somme des parts (VOTE-03)
- **Ce qui arrive :** les totaux d'équipe comptaient deux fois les points de quiz (4 158 au lieu de 2 108), alors que les lignes individuelles étaient justes : deux passes d'agrégation attribuaient les mêmes points.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:21`. OWASP CTF-in-a-box, issue #520 (https://github.com/OWASP/owasp-ctf-in-a-box/issues/520). Chez nous : `gains_add_up` et `scores_follow_gains`, `games/punchline/crates/core/src/tests/properties.rs:427` et `:450`.
- **Mécaniques :** points par vote, bonus, totaux d'équipe, cumul des manches.
- **Question :** le score de chaque joueur est-il, à chaque pas, la somme de son journal de gains ? Un duel peut-il être payé deux fois ?
- **Pression :** un journal des gains et un modèle de la règle de score tenus dans le test, sans le code du jeu (CAT-121). Punchline J13. Sidequest J12 (`navigates_the_deck_like_the_load_test`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-38 · Le bulletin d'un joueur parti suit une règle écrite, et un revote repart d'une urne vide (VOTE-04)
- **Ce qui arrive :** dans « wrongest », quand un joueur partait pendant le vote, tout le monde revotait, mais les votes déjà émis restaient comptés. Corrigé par le commit ddff0dd.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:28`. wrongest, issue #23, 2015 (https://github.com/AnotherDole/wrongest/issues/23). Punchline note que le titre et l'exemple de l'item annulent le bulletin du parti, alors que sa propriété demande seulement de suivre une règle écrite ; il a adopté l'item « à l'inverse de son titre » (`games/punchline/verification.toml`, décision VOTE-04).
- **Mécaniques :** vote avec départs en cours de manche, revote, majorité des présents.
- **Question :** que vaut le bulletin d'un joueur parti, puis revenu : gardé ou annulé ? Chaque votant a-t-il au plus un bulletin ? Un revote commence-t-il sans bulletin ?
- **Pression :** suites `Vote | Départ | Retour | Pas` pendant la phase de vote ; résultat recalculé par le modèle du test. Punchline J15 (`a_ballot_outlives_the_phone_that_cast_it`, `games/punchline/crates/core/src/tests/promises.rs`). Sidequest J24.
- **Sous la nouvelle forme :** question de mécanique. « Parti » et « revenu » sont des faits du commun (`CONTRAT.md:39`) ; ce que vaut le bulletin est une règle du jeu.
- **Étage :** mécanique.

### CAT-39 · Si le joueur au cœur du vote part, la manche se termine quand même avec un résultat défini (VOTE-05)
- **Ce qui arrive :** dans un clone du Caméléon, si le caméléon quitte la partie pendant le vote, elle ne se termine jamais. Ici, ce n'est pas un votant qui manque : c'est le sujet même du vote.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:35`. jwayn/chameleon, issue #4, ouverte (https://github.com/jwayn/chameleon/issues/4).
- **Mécaniques :** accusé, imposteur, juge, dessinateur, auteur d'une réponse, tout rôle dont la manche dépend.
- **Question :** quels sont tes rôles clés, et que prévoit la règle quand l'un d'eux part, à n'importe quel instant de la manche ?
- **Pression :** un départ du rôle clé à un indice au hasard ; faire passer le temps ; fin de manche et résultat vérifiés. Punchline J14 (`a_duelist_who_leaves_for_good_still_has_a_result`, `games/punchline/crates/core/src/tests/promises.rs`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-40 · Un buzz simultané donne la main à exactement un joueur, qui ne se la fait pas reprendre (VOTE-06, la main)
- **Ce qui arrive :** lors d'une course au buzzer, personne n'obtient la main, et la question ne peut plus être passée.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:42`. qbreader, issue #153 (https://github.com/qbreader/website/issues/153).
- **Mécaniques :** buzzer, « premier qui répond », « premier qui touche ».
- **Question :** pour tout lot de buzz du même pas, dans n'importe quel ordre, exactement un joueur a-t-il la main ? Ne change-t-elle que sur une mauvaise réponse, une fin de temps ou un départ ?
- **Pression :** plusieurs buzz par pas, mélangés, et des départs du détenteur ; toujours zéro ou un détenteur ; un buzz arrivé après n'écrase rien. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique. Le départage des égalités vient de la graine (CAT-27) ; l'instant du geste est estimé par le commun.
- **Étage :** mécanique.

### CAT-41 · Un écran ne montre jamais un résultat qu'il retire ensuite (VOTE-06, l'affichage)
- **Ce qui arrive :** en buzz quasi simultané, l'écran du rapporteur montre d'abord son propre buzz, puis le remplace par celui de l'autre joueur.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:42`. qbreader, issue #180 (https://github.com/qbreader/website/issues/180).
- **Mécaniques :** buzzer, et tout résultat qu'un appareil montrerait avant la partie qui fait foi.
- **Question :** un appareil peut-il annoncer une main, un score ou une élimination avant que le pas soit définitif ?
- **Pression :** jouer chaque partie avec et sans prédiction, et comparer leur histoire définitive (`CONTRAT.md:249`). Aucune pression ne vérifie encore qu'un résultat n'est montré qu'une fois définitif.
- **Sous la nouvelle forme :** pression du commun. Un résultat ne s'annonce qu'une fois définitif, sur la télé comme sur les téléphones (`CONTRAT.md:85`), et seules les conséquences du geste de l'appareil s'affichent avant confirmation (`CONTRAT.md:88`). Le mécanisme de prédiction est au commun.
- **Étage :** commun.

### CAT-42 · Chaque consigne va au bon nombre de joueurs, et chacun en reçoit autant que les autres (VOTE-07)
- **Ce qui arrive :** dans une partie à quatre d'un clone de Quiplash, les joueurs n'avaient pas le même nombre de questions. Corrigé par la PR #28. Source faible : l'issue n'a que son titre.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:49`. yodigi7/quiplash, issue #18 (https://github.com/yodigi7/quiplash/issues/18). Chez nous : `each_prompt_goes_to_two_players`, `games/punchline/crates/core/src/tests/properties.rs:421`.
- **Mécaniques :** distribution de consignes, de duels, de mots, de dessins à deviner.
- **Question :** pour tout nombre de joueurs autorisé et toute graine, chaque consigne va-t-elle au nombre prévu de joueurs distincts ? Chacun reçoit-il le même nombre, sans doublon, sans s'affronter lui-même ? Que dit la règle pour un nombre impair ?
- **Pression :** n tiré entre le minimum et le maximum, graine tirée, arrivées et départs avant le lancement. Punchline J30 (`every_player_gets_two_prompts_shared_with_two_different_rivals`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-43 · Un contenu écrit par les joueurs est joué exactement une fois (VOTE-08)
- **Ce qui arrive :** en mode « choisissez vos mots », le mot d'un joueur n'est jamais joué et celui d'un autre apparaît deux fois. Le mainteneur n'a pas pu le reproduire.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:56`. Drawphone, issue #30 (https://github.com/tannerkrewson/drawphone/issues/30).
- **Mécaniques :** mots proposés par les joueurs, dessins qui circulent, réponses qui deviennent des consignes.
- **Question :** l'attribution des contributions est-elle une permutation ? Deux joueurs qui écrivent le même mot font-ils deux contributions, comparées par identifiant et non par texte ? Qu'est-ce qui remplace une contribution manquante ?
- **Pression :** contributions avec doublons volontaires et absents ; le multiensemble joué égale le multiensemble retenu. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-44 · Un geste fait pendant une phase ne s'applique jamais à la suivante (VOTE-09, le retard)
- **Ce qui arrive :** chez Punchline, `Vote { choice }` ne nommait pas son duel. Un vote parti pendant un duel et arrivé après le passage au suivant aurait compté pour le suivant. Chaque phase a reçu son numéro, salon estampille chaque action de la phase que l'écran voyait, et l'hôte refuse les actions d'une phase passée.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:63`. `games/punchline/LEARNINGS.md:138` (« Les actions périmées »). Tests `a_late_vote_does_not_land_on_the_next_duel` (`games/punchline/crates/host/src/room/properties.rs:592`) et `phase_number_follows_the_phase` (`games/punchline/crates/core/src/tests/properties.rs:448`). Sidequest J15 (`a_command_from_an_older_phase_is_dropped`, `games/sidequest/crates/host/tests/e2e.rs`).
- **Mécaniques :** enchaînements rapides de votes, duels successifs, toute action qui ne nomme pas ce qu'elle vise.
- **Question :** un geste plus vieux que la fenêtre de replacement porte-t-il le pas de la projection sur laquelle il a été fait, pour que le jeu puisse le refuser ? Une action du jeu nomme-t-elle ce qu'elle vise ?
- **Pression :** actions estampillées d'une phase courante ou ancienne ; les anciennes ne changent rien. Punchline J30, Sidequest J15. Sous la nouvelle forme : l'épreuve livre des gestes en retard au-delà de la fenêtre.
- **Sous la nouvelle forme :** pression du commun. Le commun ordonne les gestes selon le moment où ils ont été faits, et replace un geste en retard à son moment dans une fenêtre bornée (`CONTRAT.md:22`, `CONTRAT.md:85`) : dans la fenêtre, le vote tombe sur son duel par construction. Au-delà, un appareil ne peut pas prétendre avoir agi plus tôt (`CONTRAT.md:98`) : le geste est placé plus tard, peut-être dans la phase suivante. C'est au commun de dire au jeu ce que l'appareil voyait.
- **Étage :** commun.

### CAT-45 · Personne ne vote pour sa propre réponse (VOTE-09, l'auteur)
- **Ce qui arrive :** un auteur qui vote pour sa propre réponse se donne des points. L'ancien commun citait « voter pour soi » parmi les actions à refuser (E5).
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:63`. Chez nous : `votes_come_from_voters`, `games/punchline/crates/core/src/tests/properties.rs:425`.
- **Mécaniques :** votes sur des réponses écrites par les joueurs, duels.
- **Question :** un vote peut-il venir d'un auteur de l'une des réponses qu'il départage ?
- **Pression :** invariant après chaque pas des sessions générées. Punchline J30.
- **Sous la nouvelle forme :** question de mécanique. Un refus ne change rien par la forme (`CONTRAT.md:26`) ; décider qu'un vote est interdit reste une règle du jeu.
- **Étage :** mécanique.

### CAT-46 · Aucun secret ne fuit dans la vue d'un joueur avant sa révélation (VOTE-10)
- **Ce qui arrive :** dans boardgame.io, le filtre des secrets nettoyait l'état du jeu, mais les secrets passaient encore par l'historique d'annulation (`ctx._undo`) et par l'état initial. Corrigé par la PR #400.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:70`. boardgame.io, issue #399 (https://github.com/boardgameio/boardgame.io/issues/399). Chez nous : `no_screen_shows_a_secret_early`, `games/punchline/crates/core/src/tests/promises.rs:550`, avec des valeurs sentinelles.
- **Mécaniques :** votes cachés, auteur anonyme, rôles secrets, mot à faire deviner, main de cartes.
- **Question :** la vue de la télé se calcule-t-elle elle aussi depuis une projection sans secret, alors qu'elle s'affiche sur la machine qui les détient ? Copier un secret dans un champ public (historique, journal, état initial) est-il possible sans le déclarer ?
- **Pression :** secrets à valeurs sentinelles (« SECRET-7f3a », « ZORGLUB ») ; après chaque pas, chercher les sentinelles dans chaque vue sérialisée. Punchline J30.
- **Sous la nouvelle forme :** impossible par la forme, pour les appareils. Un secret est marqué dans les types ; la projection envoyée à chaque appareil se calcule, elle ne s'écrit pas à la main ; un appareil ne peut pas montrer un secret qu'il n'a pas reçu (`CONTRAT.md:86`, `CONTRAT.md:170`, loi `CONTRAT.md:130`). Il n'y a plus de filtre à oublier. La télé reste une question ouverte : elle s'affiche sur l'ordinateur de la soirée, qui détient les secrets.
- **Étage :** commun.

### CAT-47 · Une question sans aucune réponse ni aucun vote se compte sans planter (VOTE-11)
- **Ce qui arrive :** ClassQuiz plantait quand personne ne répondait à une question. Corrigé par le commit f1c463c.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:77`. ClassQuiz, issue #82 (https://github.com/mawoka-myblock/ClassQuiz/issues/82). Chez nous, la règle de Punchline : sans vote, personne ne marque ; avec une seule réponse, l'autre gagne par forfait ; avec deux réponses identiques, personne ne marque (`games/punchline/LEARNINGS.md:78`, « Nos choix »).
- **Mécaniques :** quiz, votes, fin de phase par le temps alors que personne n'a joué.
- **Question :** quels sont tes cas vides (personne n'a répondu, dessiné, deviné ; une seule équipe a joué), et que dit la règle pour chacun, sans division par zéro ni `NaN` ? L'écran de révélation a-t-il quelque chose de défini à montrer ?
- **Pression :** générateur de sessions avec un poids fort sur le silence total d'une phase ; points comparés au modèle de règle. Punchline J13 (`a_duel_with_no_answer_pays_nobody`). Sidequest J14 (`time_alone_ends_every_round`).
- **Sous la nouvelle forme :** question de mécanique. Le commun reprend une partie qui plante (`CONTRAT.md:30-31`), mais une division par zéro reste un score faux.
- **Étage :** mécanique.

---

## Tour par tour (`tour-par-tour.md`)

### CAT-48 · Seul le joueur dont c'est le tour agit, annulation comprise (TOUR-01)
- **Ce qui arrive :** dans boardgame.io, le joueur 1 pouvait annuler pendant le tour du joueur 2 et défaire ses coups : rien ne vérifiait qui demandait l'annulation. Corrigé par la PR #408. Le correctif, trop strict avec les étapes, a ensuite été assoupli.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:7`. boardgame.io, issues #345 (https://github.com/boardgameio/boardgame.io/issues/345) et #565 (https://github.com/boardgameio/boardgame.io/issues/565).
- **Mécaniques :** coups, pioche, fin de tour, annulation, étapes simultanées.
- **Question :** une action de tour, annulation comprise, n'est-elle acceptée que d'un joueur actif, et n'annule-t-elle que ses propres coups du tour en cours ?
- **Pression :** `Coup(p) | Annuler(p) | FinDeTour(p)` avec p tiré parmi tous les joueurs et un inconnu ; auteur de chaque action acceptée vérifié ; les refusées laissent l'état identique, graine comprise. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique. L'identité de l'auteur vient du commun et ne se falsifie pas (`CONTRAT.md:42`) ; un refus ne change rien (`CONTRAT.md:26`). Qui a le droit d'agir reste une règle du jeu.
- **Étage :** mécanique.

### CAT-49 · Annuler un coup rend exactement l'état d'avant, jusqu'aux détails (TOUR-02)
- **Ce qui arrive :** sur Lichess, après un retour arrière, une proposition de nulle restait attachée au numéro de coup et réapparaissait au coup suivant (corrigé par la PR #18016). Dans Wesnoth, rappeler une unité puis annuler lui laissait 0 point de mouvement : la remise à zéro du rappel n'était pas défaite.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:14`. Lichess, issue #17937 (https://github.com/lichess-org/lila/issues/17937). Wesnoth, issue #11174 (https://github.com/wesnoth/wesnoth/issues/11174). Chez nous : `random_sessions_obey_the_undo_laws`, `games/tier-list/crates/core/src/tests/properties.rs:446`.
- **Mécaniques :** annulation, retour arrière, rétablir.
- **Question :** pour tout état atteignable et tout coup annulable, annuler après appliquer rend-il l'état d'avant, par égalité complète (propositions en cours, compteurs, minuteurs, graine) ? Annuler puis rétablir revient-il au coup appliqué ?
- **Pression :** préfixe d'actions au hasard, puis le coup, puis l'annulation ; égalité dérivée sur tout l'état, sans liste de champs. Tier-list J12 (modèle indépendant, compteurs compris).
- **Sous la nouvelle forme :** question de mécanique. Le commun peut copier et comparer la mémoire entière du module (`CONTRAT.md:23`), ce qui rend l'égalité complète facile à vérifier.
- **Étage :** mécanique.

### CAT-50 · Une annulation ne fait jamais revenir un joueur parti (TOUR-03)
- **Ce qui arrive :** dans boardgame.io, un joueur parti restait dans l'ordre des tours et bloquait les autres. La PR #1258 le retire de l'ordre et des joueurs actifs, et recale l'historique d'annulation pour qu'on ne revienne pas avant son départ.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:21`. boardgame.io, issue #1088 (https://github.com/boardgameio/boardgame.io/issues/1088) et PR #1258 (https://github.com/boardgameio/boardgame.io/pull/1258).
- **Mécaniques :** annulation, joueurs qui quittent en cours de partie.
- **Question :** à partir de quand un joueur absent est-il « parti » pour ton jeu (Punchline : 90 s) ? Après son départ, une annulation peut-elle le remettre dans l'ordre de jeu ?
- **Pression :** départs intercalés entre des coups et des rafales d'annulations ; ordre de jeu vérifié après chaque pas. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-51 · Le tour passe toujours à un joueur présent, sinon la manche se termine (TOUR-04)
- **Ce qui arrive :** une partie de Wesnoth dont le premier camp était vide produisait des commandes de tour illégales, puis une désynchronisation (corrigé en 2024). Dans Drawphone, un joueur remplacé par le robot « fantôme » ne jouait jamais, et la partie restait bloquée.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:28`. Wesnoth, issue #3899 (https://github.com/wesnoth/wesnoth/issues/3899). Drawphone, issue #85 (https://github.com/tannerkrewson/drawphone/issues/85).
- **Mécaniques :** ordre des tours, sièges vides, joueurs partis, robots remplaçants.
- **Question :** après chaque transition, le joueur courant est-il présent, ou la manche finie ? Un robot remplaçant joue-t-il avant l'échéance ?
- **Pression :** sièges vides au départ, départs en cours, dont celui du joueur courant ; la partie avance en un temps borné sans aucun humain. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-52 · Une fin de phase n'avance jamais le tour deux fois, et chaque joueur joue une fois par tour de table (TOUR-05)
- **Ce qui arrive :** dans boardgame.io, le compteur de tours sautait de 1 à 3 à la fin d'une phase, car la fin de phase déclenchait la fin de tour et inversement. Ailleurs, finir une phase finissait le tour sans avancer le joueur : le même joueur jouait deux tours de suite.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:35`. boardgame.io, issues #193 (https://github.com/boardgameio/boardgame.io/issues/193) et #517 (https://github.com/boardgameio/boardgame.io/issues/517).
- **Mécaniques :** phases qui contiennent des tours.
- **Question :** chaque transition avance-t-elle le compteur de tours d'au plus un ? Sur n tours sans départ, chaque joueur présent joue-t-il exactement une fois ? Si ton ordre change (sens inversé, tour passé), la règle est-elle écrite ?
- **Pression :** configurations de phases et suites d'actions ; la suite des joueurs courants est une rotation. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-53 · Deux causes de fin de tour dans le même pas finissent un seul tour, sans perdre la demande explicite (TOUR-06)
- **Ce qui arrive :** dans boardgame.io, avec une limite d'un coup, une fin de tour qui désignait le joueur suivant était ignorée : la limite avait déjà clos le tour, et la demande était jetée comme un doublon. Ailleurs, une fin de tour demandée au début d'une phase était jetée sans erreur ; la PR #1266 en fait une erreur explicite.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:42`. boardgame.io, issues #924, ouverte (https://github.com/boardgameio/boardgame.io/issues/924), et #1237 (https://github.com/boardgameio/boardgame.io/issues/1237).
- **Mécaniques :** limite de coups, fin demandée, échéance, départ du joueur courant.
- **Question :** quels sont tes déclencheurs de fin de tour, et leur priorité écrite ? Quand plusieurs coïncident, exactement un tour se termine-t-il ? Une demande ignorée donne-t-elle un refus observable, jamais un silence ?
- **Pression :** pas qui combinent limite, fin demandée avec joueur suivant, échéance et départ ; nombre de tours finis et joueur suivant vérifiés. Sidequest J12 (échéance, fin et paquets doublés dans le même pas ne terminent qu'une manche).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-54 · Une condition de fin se lit sur l'état à jour (TOUR-07, l'ordre des transitions)
- **Ce qui arrive :** dans boardgame.io, une fin d'étape était traitée après le début du tour suivant, et retirait le dernier joueur actif du nouveau tour.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:49`. boardgame.io, issue #899 (https://github.com/boardgameio/boardgame.io/issues/899).
- **Mécaniques :** fins de tour en cascade, phases qui s'enchaînent, conditions « tout le monde a joué ».
- **Question :** chaque transition interne lit-elle l'état laissé par la précédente, jamais un état d'avant ? Une fin d'étape peut-elle toucher le tour suivant ?
- **Pression :** actions simultanées où le dernier joueur termine ; la fin se produit dans le même pas. Punchline J29. Voisine de CAT-77, qui porte sur le moment de la fin et non sur l'ordre des transitions.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-55 · Une entrée ne déclenche qu'un nombre borné de transitions (TOUR-07, la boucle)
- **Ce qui arrive :** le correctif de l'issue #899 a créé une boucle infinie : une condition de fin qui modifiait les joueurs actifs se redéclenchait sans fin, et le détecteur de boucles ne la voyait pas. Corrigé par la PR #980.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:49`. boardgame.io, issue #979 (https://github.com/boardgameio/boardgame.io/issues/979).
- **Mécaniques :** toute machine à phases dont une transition peut en déclencher une autre ; phases instantanées enchaînées (titre, puis consigne).
- **Question :** combien de transitions internes une entrée peut-elle déclencher au plus ? L'épreuve signale-t-elle comme une faute du jeu un pas interrompu par son budget ?
- **Pression :** compter les transitions déclenchées par chaque entrée (compteur ou journal), au plus une constante. Punchline J29 ; Sidequest écrit qu'une commande fait au plus deux transitions (`games/sidequest/verification.toml`, décision TOUR-07).
- **Sous la nouvelle forme :** pression du commun. Chaque pas a un budget ; un jeu qui ne rend pas la main est interrompu, et l'entrée qui le fait échouer est isolée et écartée (`CONTRAT.md:30-31`). La boucle ne fige plus la partie. Elle reste une faute, que l'épreuve doit rapporter, pas seulement survivre.
- **Étage :** commun.

### CAT-56 · La partie peut finir à tout moment, même avant le premier tour, et le coup final compte (TOUR-08)
- **Ce qui arrive :** dans boardgame.io, une condition de fin déjà vraie à l'initialisation faisait planter le moteur (corrigé par la PR #888). Ailleurs, le dernier coup de la partie manquait au journal (corrigé par la PR #88).
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:56`. boardgame.io, issues #788 (https://github.com/boardgameio/boardgame.io/issues/788) et #80 (https://github.com/boardgameio/boardgame.io/issues/80).
- **Mécaniques :** conditions de victoire, paquet vide au départ, partie à zéro ou un joueur, fin immédiate.
- **Question :** chacune de tes conditions de fin, satisfaite au tout premier pas, mène-t-elle à un état « fini » ? Le score et le journal comptent-ils le coup qui a déclenché la fin ? Après la fin, toute action est-elle refusée sans rien changer ?
- **Pression :** départs dégénérés (zéro, un ou deux joueurs, paquet vide, objectif à 0) et actions après la fin. Punchline J29 (`a_game_with_nothing_in_it_still_ends`). Sidequest J28 (`a_party_cannot_end_before_its_first_round`, `the_crowd_scores_and_the_round_that_ends_the_party_counts`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-57 · Un compteur à seuil déclenche son effet exactement au seuil, ni avant, ni après (TOUR-09)
- **Ce qui arrive :** dans Secret Hitler, quand un veto portait le compteur d'élections ratées à 3, le jeu organisait une nouvelle élection au lieu d'appliquer l'effet prévu (corrigé en 0.5.0). Dans Wesnoth, un succès prévu pour le tour 40 se déclenchait au tour 39, à cause d'un `-1` en trop.
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:63`. Secret Hitler, issue #68 (https://github.com/cozuya/secret-hitler/issues/68). Wesnoth, issue #11326 (https://github.com/wesnoth/wesnoth/issues/11326).
- **Mécaniques :** limite de tours, échecs consécutifs, vies, « au bout de 3 refus ».
- **Question :** l'effet d'un seuil N se produit-il dans la transition où le compteur atteint N, par tous les chemins (veto, échec, expiration) ? Une limite de L tours autorise-t-elle exactement L tours ?
- **Pression :** modèle minimal du compteur dans le test ; suites d'échecs et de réussites par tous les chemins ; bornes en N−1, N et N+1. Tier-list J12 (`history_keeps_exactly_the_last_200_steps`, à 199, 200 et 201). Sidequest J12. Punchline J29.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

---

## Économie et conservation (`economie.md`)

### CAT-58 · Un objet n'est jamais perdu ni dupliqué : chacun est à exactement un endroit (ECO-01)
- **Ce qui arrive :** une sauvegarde de la tier-list plaçait une tuile deux fois ; le test des sauvegardes abîmées l'a trouvée. Dans Luanti, poser un objet incompatible sur une pile laissait au curseur une copie de la pile remplacée.
- **D'où on le sait :** `soiree/certifications/economie.md:7`. `games/tier-list/LEARNINGS.md:107` (« Écarts au manifeste », point 1). Tests `random_actions_keep_the_board_consistent` (`games/tier-list/crates/core/src/tests/properties.rs:413`) et `a_save_placing_an_item_twice_is_refused` (`:719`). Luanti, issue #8229, ouverte (https://github.com/luanti-org/luanti/issues/8229).
- **Mécaniques :** cartes en main, sur la table, dans la pioche ; tuiles placées ou en réserve ; objets portés ou au sol.
- **Question :** chaque objet, par identifiant, est-il dans exactement un conteneur ? Le nombre de chaque sorte est-il celui du départ, plus les créations et moins les destructions de la règle ? Le doublon est-il impossible à représenter (un seul `BTreeMap<ObjetId, Lieu>` plutôt qu'une liste par conteneur) ?
- **Pression :** après chaque pas, parcourir tous les conteneurs et compter les identifiants ; actions qui prennent, posent, échangent, scindent, fusionnent entre conteneurs pleins, vides et inexistants. Tier-list J1.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-59 · Un échange est appliqué des deux côtés ou pas du tout (ECO-02)
- **Ce qui arrive :** dans Minecraft Java (1.14 et suivantes), le « void trading » : on décharge le villageois avant de fermer la fenêtre d'échange ; le joueur garde ce qu'il a reçu, le villageois n'enregistre jamais l'échange.
- **D'où on le sait :** `soiree/certifications/economie.md:14`. Minecraft Wiki, « Block and item duplication » (https://minecraft.wiki/w/Tutorials/Block_and_item_duplication).
- **Mécaniques :** échanges entre joueurs, achats, dons, vols, toute action à deux parties.
- **Question :** un départ, une fin de manche ou un changement de phase entre « proposer », « accepter » et « conclure » laisse-t-il un échange à moitié appliqué ?
- **Pression :** échanges en plusieurs étapes avec des interruptions au hasard ; CAT-58 vérifiée après chaque pas. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique. Un échange fait d'une seule entrée est atomique par la forme : un état, une transition. Un échange en plusieurs étapes reste une règle du jeu.
- **Étage :** mécanique.

### CAT-60 · Un échange impossible est refusé sans rien faire disparaître, et livre exactement ce qui était offert (ECO-03)
- **Ce qui arrive :** dans Veloren, un échange avec un marchand, inventaire plein, aboutissait sans erreur, et les objets s'évaporaient. Ailleurs, l'offre retenait des positions dans l'inventaire et non des objets : trier pendant l'échange changeait ce qui était offert.
- **D'où on le sait :** `soiree/certifications/economie.md:21`. Veloren, issues #2172 (https://gitlab.com/api/v4/projects/veloren%2Fveloren/issues/2172) et #1631 (https://gitlab.com/api/v4/projects/veloren%2Fveloren/issues/1631).
- **Mécaniques :** inventaires bornés, mains limitées, offres puis conclusions, réordonnancements.
- **Question :** une offre désigne-t-elle des objets par identifiant, jamais par position ? Un échange qui ne tient pas est-il refusé ?
- **Pression :** inventaires presque pleins, offres, tris et déplacements, puis conclusions ; identifiants livrés vérifiés. Tier-list J13 (`a_refused_move_leaves_the_tile_where_it_was`).
- **Sous la nouvelle forme :** question de mécanique. « Un refus ne change rien » est une forme (`CONTRAT.md:26`) ; désigner par identifiant reste au jeu.
- **Étage :** mécanique.

### CAT-61 · Une sauvegarde puis un rechargement ne créent ni ne perdent rien (ECO-04, la sauvegarde)
- **Ce qui arrive :** dans Luanti, des objets rangés dans un coffre juste avant un redémarrage du serveur se retrouvaient à la fois dans le coffre et dans l'inventaire, sans doute par deux sauvegardes désaccordées (joueur et carte). Une sauvegarde de la tier-list dont le compteur d'identifiants était en retard aurait donné un identifiant déjà pris à la tuile suivante.
- **D'où on le sait :** `soiree/certifications/economie.md:28`. Luanti, issue #4661 (https://github.com/luanti-org/luanti/issues/4661). Chez nous : `a_save_with_a_stale_counter_never_reuses_an_id`, `games/tier-list/crates/core/src/tests/properties.rs:691`.
- **Mécaniques :** sauvegarde, rechargement, reprise après un redémarrage.
- **Question :** la conversion d'une sauvegarde d'une version antérieure conserve-t-elle chaque objet, et laisse-t-elle chaque compteur au-delà des identifiants utilisés ?
- **Pression :** aller-retour de sauvegarde à un pas au hasard, comparé à l'état d'avant. Tier-list J1 (`boards_survive_a_save_and_load`, `damaged_saves_load_as_nothing_or_a_consistent_board`).
- **Sous la nouvelle forme :** impossible par la forme. Le commun conserve l'état, le jeu n'a rien à écrire (`CONTRAT.md:47`), et l'état est la mémoire entière du module, que le commun copie et restaure (`CONTRAT.md:23`). Il n'y a plus deux sauvegardes à désaccorder, ni de compteur oublié par une sérialisation écrite à la main. Reste la conversion d'une version antérieure (`CONTRAT.md:32`), que la question vise.
- **Étage :** commun.

### CAT-62 · Une fin de manche pendant qu'un objet est en transit ne le crée ni ne le perd (ECO-04, l'objet en transit)
- **Ce qui arrive :** les duplications célèbres sont des oublis du bilan à un moment de transition : échange, mort, fin de manche. Un objet tenu, lancé ou en cours de pose quand la manche finit peut finir à deux endroits, ou nulle part.
- **D'où on le sait :** `soiree/certifications/economie.md:28` et l'idée commune du fichier (`economie.md:5`).
- **Mécaniques :** objets tenus, lancés ou en cours de pose ; fin de manche ; retour au salon.
- **Question :** pour chaque transition qui peut survenir pendant qu'un objet est en transit, où finit l'objet ?
- **Pression :** forcer des fins de manche pendant les actions de déplacement ; CAT-58 vérifiée après chaque pas. Punchline J28 (`a_duel_pays_once_whatever_ends_it`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-63 · L'effet d'une action n'est jamais écrasé par une copie périmée de l'état (ECO-05)
- **Ce qui arrive :** dans Luanti, un objet qui tuait son utilisateur restait dans son emplacement : la mort vidait l'inventaire, puis le moteur réécrivait l'ancienne pile par-dessus. On dupliquait l'objet. En Rust, le piège s'écrit `let pile = state.pile.clone(); …; state.pile = pile;`.
- **D'où on le sait :** `soiree/certifications/economie.md:35`. Luanti, issue #6546 (https://github.com/luanti-org/luanti/issues/6546). La tier-list l'a adapté à la copie périmée d'un autre onglet (J10, `a_tab_never_replaces_a_save_it_has_not_seen`).
- **Mécaniques :** actions qui déclenchent une cascade : élimination qui vide la main, fin de manche, exclusion.
- **Question :** ton code lit-il une valeur, appelle-t-il autre chose, puis réécrit-il la valeur lue ? Appliquer une action d'un bloc donne-t-il le même état que passer par ses événements intermédiaires ?
- **Pression :** actions qui éliminent ou finissent la manche (dernière carte, dernière vie) ; CAT-58 vérifiée après chaque pas.
- **Sous la nouvelle forme :** question de mécanique. La moitié « autre onglet » disparaît : une seule partie fait foi (`CONTRAT.md:84`) et le commun conserve l'état. La réécriture d'une copie dans le code du jeu reste possible.
- **Étage :** mécanique.

### CAT-64 · Faire puis défaire ne rapporte jamais rien (ECO-06)
- **Ce qui arrive :** dans Mindustry, la construction appliquait le multiplicateur de coût, mais pas le remboursement à la démolition : avec un multiplicateur de 0,1, chaque cycle créait des ressources.
- **D'où on le sait :** `soiree/certifications/economie.md:42`. Mindustry, issue #6518 (https://github.com/Anuken/Mindustry/issues/6518).
- **Mécaniques :** acheter puis revendre, poser puis reprendre, construire puis démolir, miser puis annuler.
- **Question :** quelles sont tes paires faire/défaire, et quels réglages changent un prix ? Un remboursement est-il toujours au plus le coût payé ?
- **Pression :** réglages tirés, paires « faire, défaire » intercalées dans des parties ; ressources comparées avant et après chaque paire. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-65 · Une même chose ne rapporte qu'une fois (ECO-07)
- **Ce qui arrive :** dans Mindustry, désigner un bloc à ramasser puis le supprimer avant l'arrivée de l'unité remboursait le bloc, et l'unité le ramassait quand même. Dans OpenTTD, décharger une cargaison, la recharger ailleurs puis revenir la faisait payer une seconde fois.
- **D'où on le sait :** `soiree/certifications/economie.md:49`. Mindustry, issue #6565 (https://github.com/Anuken/Mindustry/issues/6565). OpenTTD, issue #2995 (https://github.com/OpenTTD/OpenTTD/issues/2995).
- **Mécaniques :** points d'une bonne réponse, prime, remboursement, récompense de fin de manche, question rejouée.
- **Question :** quelles sont tes unités de gain ? Chacune, par identifiant, apparaît-elle au plus une fois au journal des gains, même quand deux actions concurrentes la visent dans le même pas, dans les deux ordres ?
- **Pression :** registre des gains par identifiant tenu dans le test ; paires concurrentes (« supprimer » et « réclamer », « répondre » et « fin du temps »). Punchline J28 (`a_duel_pays_once_whatever_ends_it`). Sidequest J27 (`a_question_played_again_pays_once`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-66 · Aucune boucle de transferts entre joueurs ne crée de valeur (ECO-08)
- **Ce qui arrive :** dans OpenTTD, avec les actions activées, acheter 75 % d'une société fantôme, lui envoyer son argent, revendre ses parts au prix gonflé puis récupérer l'argent faisait passer de 5 à 8,75 millions par tour de boucle. Fermé sans correctif, l'option étant désactivée par défaut.
- **D'où on le sait :** `soiree/certifications/economie.md:56`. OpenTTD, issue #7691 (https://github.com/OpenTTD/OpenTTD/issues/7691). Chez nous : `players_keep_their_seat`, `games/punchline/crates/core/src/tests/properties.rs:449` (la seule source est le score des duels, aucun score ne baisse).
- **Mécaniques :** dons, échanges, paris, prêts, parts, banque.
- **Question :** quelle est la liste écrite de tes sources et de tes puits ? La somme de tous les comptes ne varie-t-elle que par eux ?
- **Pression :** journal des sources et des puits dans le test ; après chaque pas, masse totale égale à la masse initiale plus le journal. Punchline J28.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-67 · Aucun compteur ne déborde, ne revient à zéro ni ne change de signe (ECO-09)
- **Ce qui arrive :** dans OpenTTD, un compteur stocké en non signé mais calculé en signé faisait passer l'entretien de +32 654 à −32 654 par an : le jeu créait de l'argent. Un don énorme débordait à la conversion de devise et envoyait 0 £. Une sauvegarde de la tier-list au bout de la plage d'identifiants faisait déborder l'identifiant suivant, ce qui panique avec `overflow-checks`.
- **D'où on le sait :** `soiree/certifications/economie.md:63`. OpenTTD, issues #7665 (https://github.com/OpenTTD/OpenTTD/issues/7665) et #7750 (https://github.com/OpenTTD/OpenTTD/issues/7750). Chez nous : `a_save_at_the_end_of_the_id_range_refuses_new_ids_without_panicking`, `games/tier-list/crates/core/src/tests/properties.rs:703`.
- **Mécaniques :** scores, argent, multiplicateurs, identifiants, compteurs de manches.
- **Question :** le maximum théorique (joueurs × manches × gain maximal × bonus) tient-il dans le type ? À la limite, ton jeu refuse-t-il avec un refus explicite (`checked_`), ou sature-t-il en silence (`saturating_`), ce qui donne un score faux ?
- **Pression :** maximum théorique calculé dans un test ; montants aux extrêmes de leur type ; aucune opération ne panique, les refus ne changent rien. Punchline J32, Sidequest J49 (`a_counter_at_its_limit_refuses_and_changes_nothing`), tier-list J1.
- **Sous la nouvelle forme :** impossible par la forme, pour le débordement silencieux. Les règles d'écriture interdisent l'arithmétique entière qui peut déborder et la conversion qui perd des chiffres sans le dire, et le dépassement est vérifié même en release (`CONTRAT.md:153-154`). Un compteur ne peut plus revenir à zéro ni changer de signe sans qu'on l'ait écrit. Le dimensionnement du type et le choix entre refus et saturation restent dans la question.
- **Étage :** commun.

### CAT-68 · Partager ou arrondir ne crée ni ne détruit rien (ECO-10)
- **Ce qui arrive :** dans OpenTTD, un prêt de 100 000 à 2 % rapportait 1 968 par an au lieu de 2 000, par arrondis répétés. Les correctifs proposés gardaient un écart de −8 ou +4 par an.
- **D'où on le sait :** `soiree/certifications/economie.md:70`. OpenTTD, issue #2410 (https://github.com/OpenTTD/OpenTTD/issues/2410).
- **Mécaniques :** points partagés, part des votes, intérêts, pourcentages, pot divisé.
- **Question :** la somme des parts vaut-elle exactement le total ? Deux parts qui devraient être égales diffèrent-elles d'au plus 1, avec un reste attribué par une règle qui ne dépend pas du numéro des joueurs ? Si le reste est perdu, l'est-il d'au plus N−1, et est-ce écrit ?
- **Pression :** total, N et poids tirés ; somme exacte et écart vérifiés. Punchline J28 (`sharing_the_votes_loses_at_most_a_point`, formule entière `1000 × multiplicateur × part des votes`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-69 · Une commande répétée par un double appui n'est payée qu'une fois (ECO-11)
- **Ce qui arrive :** dans OpenTTD, recliquer sur une gare déjà construite refacturait tout son coût : un rebond de souris suffisait à vider la caisse. Fermé sans correction, car reconstruire est voulu pour certaines gares. Sur un téléphone, le double tap est le cas courant.
- **D'où on le sait :** `soiree/certifications/economie.md:77`. OpenTTD, issue #5544 (https://github.com/OpenTTD/OpenTTD/issues/5544).
- **Mécaniques :** achats, mises, pioche, toute action qui coûte ou fait avancer la partie.
- **Question :** quelles actions sont cumulables par règle, et lesquelles fixent une valeur ? Une commande doublée (même contenu, même phase) change-t-elle l'état final ?
- **Pression :** la même suite avec des doublons insérés juste après les originaux ; états finaux comparés. Punchline J27, Sidequest J15, tier-list J14 (`a_double_click_counts_once`).
- **Sous la nouvelle forme :** question de mécanique. Le commun compte un événement exactement une fois sur le réseau ; un double appui du joueur fait deux gestes, que seul le jeu sait juger.
- **Étage :** mécanique.

---

## Progression (`progression.md`)

### CAT-70 · Aucun numéro de manche ou de niveau ne produit un état impossible (PROG-01)
- **Ce qui arrive :** dans Pac-Man, un débordement d'entier charge mal le 256ᵉ niveau, impossible à finir. Dans Donkey Kong, le temps du niveau 22 ne tient plus sur un octet et retombe à presque rien : Jumpman meurt en quelques secondes (le « kill screen »).
- **D'où on le sait :** `soiree/certifications/progression.md:5`. Wikipédia, « Pac-Man » (https://en.wikipedia.org/wiki/Pac-Man). heise sur Donkey Kong (https://heise.de/-10275685).
- **Mécaniques :** manches qui s'enchaînent, difficulté qui monte, minuteurs selon le niveau, parties relancées toute la soirée.
- **Question :** qu'est-ce qui dépend du numéro de manche (multiplicateur, minuteur, taille de grille) ? De 0 à la plus haute borne atteignable, et au-delà si « rejouer » enchaîne les compteurs, ces grandeurs restent-elles bornées, et la manche gagnable ?
- **Pression :** pour chaque numéro de manche, construire l'état de départ et vérifier les bornes ; jouer la manche avec des bots jusqu'à sa fin. Punchline J29 (`every_round_number_stays_in_bounds`).
- **Sous la nouvelle forme :** question de mécanique. Le débordement silencieux est déjà supprimé (CAT-67) ; une durée qui tombe à presque rien sans déborder reste possible.
- **Étage :** mécanique.

### CAT-71 · Une manche ne lit jamais ce qu'une autre a laissé (PROG-02, l'état)
- **Ce qui arrive :** dans Pokémon Rouge et Bleu, un tutoriel copie le nom du joueur dans le tampon des rencontres sauvages, et un lieu qui n'écrit pas ses propres rencontres lit ces restes comme des Pokémon : MissingNo.
- **D'où on le sait :** `soiree/certifications/progression.md:12`. Wikipédia, « MissingNo. » (https://en.wikipedia.org/wiki/MissingNo.).
- **Mécaniques :** tampons réutilisés d'une phase à l'autre, sélections, numéros qui repartent de zéro, entrées en attente.
- **Question :** ce qu'une phase montre et accepte dépend-il seulement de ce qu'elle a construit ? Chaque phase porte-t-elle ses données dans sa variante d'`enum`, qui disparaît avec elle ?
- **Pression :** champs hors phase remplis de valeurs poison au changement de manche ; la vue et le résultat de la manche suivante sont ceux obtenus avec des champs vides. Punchline J8 (`a_round_reads_nothing_the_last_one_left`, `games/punchline/crates/core/src/tests/promises.rs`). Sidequest J24.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-72 · Un brouillon appartient à ce qu'il édite, pas à un numéro qui revient (PROG-02, le téléphone)
- **Ce qui arrive :** chez Punchline, les brouillons d'une manche réapparaissaient à la suivante, parce que les numéros de consigne repartent de 0 à chaque manche. Ils sont maintenant effacés dès qu'on quitte l'écran d'écriture.
- **D'où on le sait :** `soiree/certifications/progression.md:12`. `games/punchline/LEARNINGS.md:197` (« Bugs trouvés en jouant »). Tests `coalesced_rounds_cannot_reuse_an_identical_prompts_draft` et `a_draft_of_another_moment_reaches_nothing` (Punchline J8).
- **Mécaniques :** champs de texte, brouillons, toute saisie en cours sur un téléphone.
- **Question :** un champ de texte est-il rattaché à l'identité de l'objet qu'il édite ? Son brouillon disparaît-il avec cet objet, même si un nouvel objet reçoit le même numéro local ?
- **Pression :** Punchline J8 (champs empoisonnés, comparaison de l'état à l'octet).
- **Sous la nouvelle forme :** pression du commun. Les champs de texte sont des composants du commun (`CONTRAT.md:45`), et un élément en cours d'édition garde son identité, sa sélection et sa composition tant que son objet existe (`CONTRAT.md:118`). Le lien entre un brouillon et son objet se garantit une fois, pour toutes les pages.
- **Étage :** commun.

### CAT-73 · Après « rejouer », l'état est celui d'une partie neuve avec les mêmes joueurs (PROG-03)
- **Ce qui arrive :** dans SuperTuxKart, les temps du mode « suivez le meneur » n'étaient pas remis à zéro au redémarrage, et le dernier cumulait le temps du premier. Un kart qui portait une bombe au redémarrage explosait au départ suivant.
- **D'où on le sait :** `soiree/certifications/progression.md:19`. SuperTuxKart, issues #3019 (https://github.com/supertuxkart/stk-code/issues/3019) et #3820 (https://github.com/supertuxkart/stk-code/issues/3820). Chez nous : `new_game_keeps_the_table`, `games/punchline/crates/core/src/tests/properties.rs:453`.
- **Mécaniques :** relance de partie, retour au salon, nouvelle manche.
- **Question :** l'état après « rejouer » est-il égal à celui d'une instance neuve avec les mêmes joueurs, les mêmes réglages et une graine, sans liste de champs ? Ce qui continue exprès (paquet et générateur chez Punchline, numéro de manche chez Sidequest) est-il déclaré ?
- **Pression :** suite d'actions au hasard, « rejouer », comparaison complète avec un état neuf. Punchline J29, Sidequest J29 (`whatever_ended_the_party_the_next_one_plays_the_same`).
- **Sous la nouvelle forme :** pression du commun. L'état est la mémoire entière du module, que le commun peut copier et comparer (`CONTRAT.md:23`), et on relance une partie sans rien redémarrer (`CONTRAT.md:274`). Le commun peut comparer, pour tous les jeux, l'état après « rejouer » à une instance neuve, aux reports déclarés près.
- **Étage :** commun.

### CAT-74 · Quelle que soit la façon dont une partie finit, la suivante se joue avec les mêmes réglages (PROG-04)
- **Ce qui arrive :** dans un jeu de mots chronométré, le score et les mots trouvés passaient à la partie suivante, et après l'expiration du chrono on ne pouvait plus envoyer de mot sans recharger la page. Dans un clone de Codenames, le minuteur choisi ne s'appliquait qu'à la première partie (corrigé par le commit 858c212).
- **D'où on le sait :** `soiree/certifications/progression.md:39`. smstamm/maple, issue #16 (https://github.com/smstamm/maple/issues/16). jbowens/codenames, issue #113 (https://github.com/jbowens/codenames/issues/113).
- **Mécaniques :** fin au temps, fin anticipée, arrêt par l'organisateur, plus personne, victoire, puis relance.
- **Question :** quelles sont toutes les sorties de ta phase de fin ? Après chacune, au moins une action de jeu est-elle acceptée, et la partie k se comporte-t-elle comme la partie 1 avec les mêmes réglages et les mêmes entrées ?
- **Pression :** façon de finir tirée, « rejouer », puis une suite d'entrées fixe ; trace comparée à celle d'un jeu neuf. Punchline J29, Sidequest J29 (cinq façons de finir).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-75 · Aucune échéance ne se déclenche après la fin de sa partie ou de sa phase (PROG-05)
- **Ce qui arrive :** quand tous les joueurs partent, la partie est fermée, mais son minuteur continue, et la fin de partie est appelée deux fois.
- **D'où on le sait :** `soiree/certifications/progression.md:46`. words-treasure, issue #22 (https://github.com/JumpyLionnn/words-treasure/issues/22). Chez nous : `next_deadline`, dérivée de `phase_started` (`games/punchline/crates/core/src/lib.rs:599`), et Sidequest J28 (`a_deadline_after_the_end_is_nothing`).
- **Mécaniques :** minuteurs, comptes à rebours, échéances, robots qui agissent « dans 2 s ».
- **Question :** une échéance de ton état peut-elle survivre à la phase qui l'a créée ?
- **Pression :** fins et relances juste avant des échéances, temps qui passe, nombre de fins compté par partie. Punchline J29, Sidequest J28.
- **Sous la nouvelle forme :** impossible par la forme. Le jeu ne programme rien : il reçoit le temps en pas (`CONTRAT.md:27`). Une échéance est une donnée de son état, comparée au pas courant ; il n'y a pas de minuteur à annuler. Sur la page, minuteurs et écouteurs appartiennent à une vue et s'arrêtent avec elle (`CONTRAT.md:118`). Le seul piège restant, une échéance rangée hors de sa phase, est CAT-71.
- **Étage :** commun.

### CAT-76 · Les effets de la fin s'appliquent une seule fois (PROG-06)
- **Ce qui arrive :** dans un clone de Timberman, une fin de partie en mode normal enregistrait la session deux fois. Les totaux doublaient, mais la moyenne restait à peu près juste, ce qui masquait l'erreur. Le bug ne touchait qu'un mode.
- **D'où on le sait :** `soiree/certifications/progression.md:53`. majiayu000/timberman-game, issue #8 (https://github.com/majiayu000/timberman-game/issues/8).
- **Mécaniques :** résultats enregistrés, statistiques, couronne, podium, gain de fin, sons et confettis de fin.
- **Question :** dans tous tes modes, la transition vers la fin a-t-elle lieu une seule fois, même si plusieurs causes coïncident ou si le temps continue après ? Les effets de fin y sont-ils attachés ?
- **Pression :** journal des applications de l'effet de fin ; fins par chaque chemin, longues attentes après, modes croisés. Punchline J29 ; Sidequest J28 (le tableau final est calculé une fois, dans la transition vers la fin).
- **Sous la nouvelle forme :** question de mécanique. Un effet ponctuel porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas (`CONTRAT.md:24`) ; deux transitions vers la fin produiraient deux effets distincts, et cela reste au jeu.
- **Étage :** mécanique.

### CAT-77 · Quand le dernier joueur attendu a joué, la phase passe tout de suite (PROG-07)
- **Ce qui arrive :** dans boardgame.io, la condition de fin de tour était évaluée avant la mise à jour des joueurs actifs : le tour ne finissait pas quand le dernier avait joué. Corrigé en évaluant après.
- **D'où on le sait :** `soiree/certifications/progression.md:60`. boardgame.io, issue #769 (https://github.com/boardgameio/boardgame.io/issues/769). Chez nous, la règle de Punchline : une phase finit plus tôt quand tout le monde a répondu (`games/punchline/LEARNINGS.md:78`).
- **Mécaniques :** fin anticipée d'une phase d'écriture, de vote ou de tour simultané.
- **Question :** qui est « attendu » (les présents, les absents depuis moins de 90 s) ? La phase passe-t-elle au plus tard à l'action, ou au départ, du dernier attendu, sans attendre l'échéance ?
- **Pression :** réponses dans un ordre au hasard et départs intercalés ; la phase a changé dans le même pas. Punchline J14 (`a_vote_nobody_can_cast_ends_at_once`) ; quand le dernier attendu part, Punchline attend que la salle soit stable depuis 20 s, pas son départ.
- **Sous la nouvelle forme :** question de mécanique. Qui est absent ou parti est un fait du commun ; qui la phase attend est une règle du jeu.
- **Étage :** mécanique.

### CAT-78 · Le jeu n'attend jamais un joueur qui ne peut pas agir (PROG-08)
- **Ce qui arrive :** dans boardgame.io, au début du tour du joueur 1, les joueurs actifs contenaient encore le joueur 0, et le joueur courant n'avait plus aucun coup valide. Dans SuperTuxKart, un joueur qui passait la ligne à grande vitesse n'avait pas son tour compté : il restait au tour 1, et la course ne finissait jamais.
- **D'où on le sait :** `soiree/certifications/progression.md:67`. boardgame.io, issue #964 (https://github.com/boardgameio/boardgame.io/issues/964). SuperTuxKart, issue #4760 (https://github.com/supertuxkart/stk-code/issues/4760). Chez nous, le test de Punchline joue « ce que l'écran demande » (`Op::Answers`, `Op::Votes`).
- **Mécaniques :** joueurs actifs, tours, conditions de progression, écrans qui demandent une action.
- **Question :** le commun sait-il qui le jeu attend ? Pour chaque joueur attendu, au moins une action est-elle acceptée, sinon une échéance qui ne dépend pas du progrès des joueurs existe-t-elle ?
- **Pression :** après chaque pas, essayer pour chaque joueur attendu les actions que son écran propose. Punchline J26, Sidequest J39 (`time_alone_ends_every_round`).
- **Sous la nouvelle forme :** pression du commun. Ne jamais figer la partie est la première garantie (`MANIFESTE.md:15`). Le jeu fournit des bots qui avancent dans une vraie partie (`CONTRAT.md:29`), et l'épreuve les fait parcourir toutes les phases (`CONTRAT.md:249`). Le commun peut vérifier pour tous qu'un joueur attendu a une action acceptée, ou qu'une échéance existe, si le jeu déclare qui il attend.
- **Étage :** commun.

---

## Texte (`texte.md`)

### CAT-79 · Deux réponses identiques à l'écran ne sont jamais jugées différentes à cause de leur encodage Unicode (TXT-01)
- **Ce qui arrive :** sous macOS, le système de fichiers stockait les noms en forme décomposée (NFD) alors que l'utilisateur tapait en forme composée (NFC) : un même nom était « manquant » d'un côté et « inconnu » de l'autre. Un clavier iOS et un clavier Android ne produisent pas toujours la même forme. Aucune forme n'est stable par concaténation.
- **D'où on le sait :** `soiree/certifications/texte.md:7`. Bazaar 0.92, bug 172383 (https://bugs.launchpad.net/bugs/172383). Unicode, UAX #15 (https://unicode.org/reports/tr15/). Chez nous : `two_unicode_forms_are_one_text`, `games/punchline/crates/core/src/tests/text.rs`.
- **Mécaniques :** bonne réponse d'un quiz, réponses identiques (« jinx »), mots à deviner.
- **Question :** ta comparaison normalise-t-elle les deux chaînes vers la même forme avant de comparer, y compris une réponse assemblée en plusieurs morceaux ?
- **Pression :** chaînes accentuées générées, formes NFC et NFD produites par `unicode-normalization`, comparées par la fonction du jeu. Punchline J22.
- **Sous la nouvelle forme :** question de mécanique. Pour les prénoms, la comparaison est au commun (CAT-88).
- **Étage :** mécanique.

### CAT-80 · La clé de comparaison appliquée deux fois donne le même résultat qu'une fois (TXT-02)
- **Ce qui arrive :** chez Spotify en 2013, la normalisation des noms d'utilisateur n'était pas idempotente : « ᴮᴵᴳᴮᴵᴿᴰ » donnait `BIGBIRD` au premier passage et `bigbird` au second. Un attaquant a pu réinitialiser le mot de passe d'un autre compte.
- **D'où on le sait :** `soiree/certifications/texte.md:14`. Spotify Engineering, « Creative usernames » (https://engineering.atspotify.com/2013/06/creative-usernames). Chez nous : `a_key_applied_twice_is_the_key` (Punchline J22).
- **Mécaniques :** toute normalisation de texte (casse, accents, espaces, ponctuation) utilisée pour comparer.
- **Question :** `clé(clé(x))` vaut-il `clé(x)` ? Ta comparaison est-elle une égalité de clés, donc transitive ? Si tu tolères les fautes de frappe, ne t'en sers-tu jamais pour des doublons ?
- **Pression :** chaînes quelconques enrichies de petites capitales, exposants, ligatures (« ﬃ »), pleine chasse. Punchline J22.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-81 · « Ces deux réponses sont la même » ne dépend jamais de la langue d'un appareil (TXT-03)
- **Ce qui arrive :** en turc, la minuscule de « I » est « ı » sans point : `"FILE".ToLower()` ne correspond plus à « file » (le « Turkey test »). Une recherche ne trouvait pas les titres tchèques accentués, jusqu'à une option ajoutée en 2025.
- **D'où on le sait :** `soiree/certifications/texte.md:21`. Moserware, « Does your code pass the Turkey test? » (https://www.moserware.com/2008/02/does-your-code-pass-turkey-test.html). Fuse.js, issue #723 (https://github.com/krisk/Fuse/issues/723). Chez nous : `the_key_depends_on_no_language` (Punchline J22).
- **Mécaniques :** comparaison insensible à la casse ou aux accents.
- **Question :** ton jeu ignore-t-il les accents (« éléphant » = « elephant ») ? C'est écrit ? Sur la table fixe {I, ı, İ, i, ß, ẞ, Σ final}, la clé donne-t-elle une valeur unique, écrite dans le test ?
- **Pression :** test de table pour les cas fixes, propriété sur des chaînes accentuées. Punchline J22 (les accents comptent).
- **Sous la nouvelle forme :** impossible par la forme. Le jeu est enfermé et n'atteint le monde que par le commun (`CONTRAT.md:10`, loi `CONTRAT.md:126`) : il ne connaît pas la langue de l'appareil. La même étape tourne partout, prédiction comprise (`CONTRAT.md:87`), et les conversions de casse de Rust ne dépendent d'aucune locale. Le choix des accents reste une décision du jeu, dans la question.
- **Étage :** commun.

### CAT-82 · Mettre un texte en majuscules ne lui fait pas dépasser son budget (TXT-04)
- **Ce qui arrive :** dans MediaWiki, mettre la première lettre d'un titre en majuscule changeait « ß » en « SS » sous Chromium 54, alors que le serveur la laissait intacte : l'article « ß » devenait introuvable pour certains outils. En Rust aussi, `to_uppercase` peut produire plusieurs caractères pour un seul (« tschüß » donne « TSCHÜSS »).
- **D'où on le sait :** `soiree/certifications/texte.md:28`. MediaWiki, T147646 (https://phabricator.wikimedia.org/T147646). Documentation de Rust, `str` (https://doc.rust-lang.org/std/primitive.str.html). Chez nous : `capitals_stay_within_the_budget` (Punchline J21) ; Sidequest J37, pour le contenu livré seulement (une question libre de l'organisateur n'est pas couverte).
- **Mécaniques :** prénoms ou réponses affichés en capitales, titres, comparaisons après `to_uppercase`.
- **Question :** la forme en majuscules de tout texte accepté respecte-t-elle le budget d'affichage ? Un indice ou une coupe calculés sur la forme transformée sont-ils jamais appliqués à l'original ?
- **Pression :** textes riches en « ß », « ŉ », « ǰ », ligatures « ﬁ » et « ﬀ », « İ », à la limite de longueur. Punchline J21.
- **Sous la nouvelle forme :** question de mécanique. La divergence entre deux implémentations (navigateur et serveur) disparaît : le même module calcule partout. L'allongement reste.
- **Étage :** mécanique.

### CAT-83 · Couper un texte ne coupe jamais au milieu d'un caractère ni d'un graphème (TXT-05)
- **Ce qui arrive :** pour Yjs, un `.slice()` a séparé les deux moitiés d'un caractère hors du plan de base ; l'encodage a levé une erreur, et la synchronisation s'est arrêtée sans bruit en perdant des modifications. Couper à une frontière de caractère peut encore couper un graphème, comme un emoji composé. Chez Sidequest, le préremplissage du prénom coupe par point de code et peut amputer une famille d'emojis sur le téléphone.
- **D'où on le sait :** `soiree/certifications/texte.md:35`. George Mandis, « Invalid surrogate pairs » (https://george.mand.is/2026/05/my-favorite-bugs-invalid-surrogate-pairs/). Documentation de Rust, `str`. Sidequest a laissé l'item « unreviewed » (`games/sidequest/verification.toml`, décision TXT-05). Chez nous : `a_text_is_cut_between_graphemes` (Punchline J21) ; `a_cut_never_splits_an_emoji_family_or_an_accent` (tier-list J6).
- **Mécaniques :** troncature d'un prénom ou d'une réponse, aperçu, « … », limite d'un champ de saisie.
- **Question :** le commun coupe-t-il et compte-t-il une seule fois, sur des frontières de graphèmes, de la même façon dans le champ du téléphone et dans la partie qui fait foi ? `couper(couper(s, n), n)` vaut-il `couper(s, n)` ?
- **Pression :** suites d'emoji à ZWJ, drapeaux, lettres suivies d'accents combinants ; n tiré sur toute la longueur ; vérification avec `unicode-segmentation`.
- **Sous la nouvelle forme :** pression du commun. Les prénoms et les champs de texte sont au commun (`CONTRAT.md:42`, `CONTRAT.md:45`). La moitié « au milieu d'un caractère » est déjà impossible en Rust : on ne coupe pas une `str` hors d'une frontière sans paniquer, et l'index hors bornes est interdit (`CONTRAT.md:150`). Une coupe d'affichage que le jeu ferait lui-même reste à lui.
- **Étage :** commun.

### CAT-84 · Deux prénoms qui se ressemblent à l'œil ne coexistent pas (TXT-06)
- **Ce qui arrive :** constat de spécification, pas bug de jeu. Un « paypal » écrit avec un « а » cyrillique ressemble au vrai. Sur la télé, deux joueurs aux prénoms identiques à l'œil ne se distinguent plus.
- **D'où on le sait :** `soiree/certifications/texte.md:42`. Unicode, UTS #39, « squelette » des confusables (https://www.unicode.org/reports/tr39/).
- **Mécaniques :** prénoms, noms d'équipe, tout ce qui sert à reconnaître quelqu'un sur la télé.
- **Question :** le commun refuse-t-il un prénom qui ne diffère d'un autre que par une lettre confusable (a/а, e/е, o/о, p/р, c/с, l/I/1, O/0), ou garantit-il une autre marque (couleur, personnage) ?
- **Pression :** prénom généré, puis variante par substitution ; refus vérifié. Aucun de nos jeux : tous trois s'appuient sur le personnage et la couleur.
- **Sous la nouvelle forme :** pression du commun. Les prénoms validés appartiennent au commun (`CONTRAT.md:42`).
- **Étage :** commun.

### CAT-85 · Un caractère invisible ne permet jamais de prendre le nom d'un autre (TXT-07)
- **Ce qui arrive :** dans OnionShare 2.3 à 2.3.3, on pouvait se renommer « Alice » suivi d'une espace invisible et écrire dans le chat sous l'apparence d'Alice ; corrigé en 2.5 en retirant les caractères non visibles. On ne peut pas tout bannir : certaines écritures et les emoji composés ont besoin du ZWJ. U+200B et U+202E sont de catégorie Cf, pas Cc : un nettoyage des caractères de contrôle ne les voit pas.
- **D'où on le sait :** `soiree/certifications/texte.md:49`. Avis GHSA-68vr-8f46-vc9f (https://osv.dev/vulnerability/GHSA-68vr-8f46-vc9f). Liste rails-dev d'OpenStreetMap, février 2024 (https://lists.openstreetmap.org/pipermail/rails-dev/2024-February/029813.html). Chez nous : `an_invisible_character_changes_no_name`, `an_emoji_keeps_its_joiners_and_a_letter_does_not` (Punchline J21).
- **Mécaniques :** prénoms, auteur d'un message ou d'une réponse.
- **Question :** insérer un caractère ignorable par défaut ou de contrôle bidirectionnel (U+200B, U+2060, U+202E, U+2066…) à n'importe quelle place donne-t-il un prénom refusé ou ramené à l'original ? Un emoji à ZWJ légitime reste-t-il accepté ?
- **Pression :** prénom, caractère et position tirés ; refus ou égalité de clé ; emoji à ZWJ testés à part. Punchline J21.
- **Sous la nouvelle forme :** pression du commun. L'identité est un identifiant que seul le commun crée (`CONTRAT.md:42`) : un prénom ne donne plus aucun droit, et l'usurpation d'identité disparaît. Reste l'apparence sur la télé, que le commun garantit en validant les prénoms.
- **Étage :** commun.

### CAT-86 · Le filtre de mots ne censure jamais un mot innocent qui contient un mot interdit (TXT-08)
- **Ce qui arrive :** le « problème de Scunthorpe » : AOL a bloqué les habitants de Scunthorpe en 1996 ; Xbox Live a suspendu en 2010 un joueur qui habitait Fort Gay ; des filtres de remplacement ont produit « clbuttic » pour « classic ».
- **D'où on le sait :** `soiree/certifications/texte.md:56`. Wikipédia, « Scunthorpe problem » (https://en.wikipedia.org/wiki/Scunthorpe_problem). Punchline n'a pas de filtre : on joue entre amis (`games/punchline/LEARNINGS.md:43`, « Leçons à trier »).
- **Mécaniques :** filtre « mode famille », censure, mots interdits d'un jeu de mots.
- **Question :** un corpus innocent embarqué (Scunthorpe, analyse, Essex, classique, assassin, « conférence », « Bitche ») passe-t-il le filtre ? Seule une correspondance sur un mot entier déclenche-t-elle ? Filtrer deux fois revient-il à filtrer une fois ?
- **Pression :** mot interdit entouré de préfixes et de suffixes tirés d'un dictionnaire. Aucun de nos jeux n'a de filtre.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-87 · Un texte de joueur inséré dans une phrase n'est jamais réinterprété comme un gabarit (TXT-09)
- **Ce qui arrive :** dans Army Men RTS 1.0, rejoindre un serveur avec le pseudo `%n%n%n` le faisait planter immédiatement. Dans Spigot, un greffon passait le message brut comme format, et « I am 100% sure » levait une exception. Un prénom qui contient `{réponse}` peut être remplacé par la réponse de quelqu'un si la substitution se fait en plusieurs passages.
- **D'où on le sait :** `soiree/certifications/texte.md:63`. Bugtraq, 2004 (https://lists.openwall.net/bugtraq/2004/11/14/2). SPIGOT-839 (https://hub.spigotmc.org/jira/browse/SPIGOT-839). Chez nous : `a_name_is_not_read_as_a_template` (Punchline J22) ; `a_name_that_looks_like_a_template_stays_a_name` (Sidequest J44).
- **Mécaniques :** consignes qui citent un prénom, messages qui citent une réponse, annonces de l'animateur.
- **Question :** ta substitution se fait-elle en un seul passage ? Chaque texte inséré se retrouve-t-il tel quel à sa place, sans que rien d'autre soit remplacé ?
- **Pression :** textes sur l'alphabet `{ } % s n d $ \` plus `{joueur}` et `{réponse}` ; rendre, retrouver chaque texte à sa place. Punchline J22, Sidequest J44.
- **Sous la nouvelle forme :** question de mécanique. La famille `%n` disparaît en Rust, où le format est une chaîne littérale vérifiée à la compilation. Un gabarit écrit par le jeu reste exposé.
- **Étage :** mécanique.

### CAT-88 · Le contrôle des doublons porte sur le nom final, noms réservés compris (TXT-10)
- **Ce qui arrive :** les tests de propriétés de Punchline ont trouvé un joueur qui pouvait s'appeler comme l'animateur, des prénoms en double une fois coupés à la limite, et des doubles espaces invisibles qui distinguaient deux prénoms identiques à l'écran.
- **D'où on le sait :** `soiree/certifications/texte.md:70`. `games/punchline/LEARNINGS.md:199` (« Bugs trouvés par la certification »). Invariant `names_fit_and_differ`, `games/punchline/crates/core/src/tests/properties.rs:419` ; `names_are_compared_once_cleaned` (Punchline J22).
- **Mécaniques :** prénoms des joueurs, des bots et de l'animateur.
- **Question :** le commun nettoie-t-il, coupe-t-il, puis compare-t-il, dans cet ordre ? Un jeu peut-il lui déclarer ses noms réservés (animateur, bots, « TV ») ?
- **Pression :** générateur de prénoms : textes vicieux, nom de l'animateur et d'un bot, variantes de casse, espaces autour, prénoms déjà à la limite (« Maximilienne » et « MAXIMILIENNE »). Punchline J22.
- **Sous la nouvelle forme :** pression du commun. Les prénoms sont validés par le commun (`CONTRAT.md:42`). Seul l'ordre nettoyer, couper, puis comparer donne des noms distincts à l'écran.
- **Étage :** commun.

---

## Hasard (`hasard.md`)

### CAT-89 · Un mélange donne chaque ordre avec la même probabilité (HAS-01)
- **Ce qui arrive :** échanger chaque carte avec une position prise dans tout le paquet donne 27 chemins pour 6 ordres de trois cartes : certains ordres sortent bien plus souvent. En 2010, l'écran de choix du navigateur de Microsoft mélangeait par un tri à comparateur aléatoire ; Internet Explorer finissait bien plus souvent à certaines places.
- **D'où on le sait :** `soiree/certifications/hasard.md:7`. Jeff Atwood, « The danger of naïveté » (https://blog.codinghorror.com/the-danger-of-naivete/). Rob Weir, sur le bulletin de Microsoft (https://www.robweir.com/blog/2010/02/microsoft-random-browser-ballot.html). Chez nous : `every_order_is_equally_likely`, `games/punchline/crates/core/src/rng.rs`.
- **Mécaniques :** mélanger un paquet, l'ordre de passage, la place des réponses à l'écran.
- **Question :** ton mélange passe-t-il jamais par un tri à comparateur aléatoire ? Pour n = 3 et 4, chacun des n! ordres sort-il avec la même fréquence ?
- **Pression :** K mélanges à graines fixées, test du χ² à seuil strict. Punchline J31.
- **Sous la nouvelle forme :** question de mécanique. Le commun fournit la graine (`CONTRAT.md:38`) ; le mélange est écrit par le jeu. Si le commun fournit un jour les tirages eux-mêmes, la leçon passe au commun.
- **Étage :** mécanique.

### CAT-90 · Chaque élément peut finir à chaque place, y compris la dernière et la sienne (HAS-02)
- **Ce qui arrive :** dans le poker en ligne d'ASF Software (1999), le générateur renvoyait 0 à n−1 mais le code supposait 1 à n : la dernière carte ne revenait jamais à la dernière place. La variante de Sattolo, elle, ne laisse jamais un élément à sa place.
- **D'où on le sait :** `soiree/certifications/hasard.md:29`. Datamation, « How to cheat at online poker » (https://www.datamation.com/applications/how-to-cheat-at-online-poker-a-study-in-software-security/). Chez nous : `every_item_reaches_every_place` (Punchline J31).
- **Mécaniques :** tout mélange de Fisher-Yates écrit à la main.
- **Question :** pour n de 2 à 8, la matrice « élément j à la place i » a-t-elle une case vide ?
- **Pression :** positions comptées sur K mélanges, chaque case proche de K/n (χ²). Punchline J31.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-91 · Un tirage borné ne favorise jamais les petites valeurs (HAS-03)
- **Ce qui arrive :** avec `rand() % 52` sur 32 bits, les 48 premières cartes ont une chance de plus que les 4 dernières ; au-delà de 2³¹, certaines issues deviennent deux fois plus fréquentes. Les remèdes : le rejet, ou la méthode de Lemire.
- **D'où on le sait :** `soiree/certifications/hasard.md:36`. Melissa O'Neill, PCG, « Bounded rands » (https://www.pcg-random.org/posts/bounded-rands.html). Chez nous : `a_bounded_draw_favours_no_value` (Punchline J31).
- **Mécaniques :** « une question parmi 37 », « un joueur parmi 7 », tout `% n`.
- **Question :** avec une source qui énumère tout un domaine de 8 bits, ton tirage borné donne-t-il des comptes strictement égaux pour chaque n de 2 à 255 ?
- **Pression :** source de test injectée qui renvoie 0 à 255 ; un `% n` échoue dès n = 3. Sans source injectable, χ² sur n = 3, 5, 7. Punchline J31.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-92 · Deux parties de la soirée ne reçoivent jamais la même graine par défaut (HAS-04)
- **Ce qui arrive :** sur Pavlovia, plus de 300 participants d'une expérience ont reçu la même liste dans le même ordre : un composant avait une graine fixe à 1, et tout le hasard suivait la même suite.
- **D'où on le sait :** `soiree/certifications/hasard.md:43`. Forum de PsychoPy (https://discourse.psychopy.org/t/math-random-using-the-same-random-seed-and-so-is-shuffle/16956). Chez nous : `two_games_of_an_evening_differ` (`games/punchline/crates/core/src/tests/promises.rs`) et `two_launches_draw_two_seeds` (`games/punchline/crates/host/src/main.rs`).
- **Mécaniques :** création de la graine au lancement ou à chaque nouvelle partie.
- **Question :** le commun tire-t-il une graine neuve pour chaque partie, sauf rejeu voulu, et la note-t-il pour rejouer ? Le premier joueur désigné et la première question sont-ils uniformes sur K graines ?
- **Pression :** propriété sur la fonction « entropie vers graine » ; test statistique sur le premier tirage ; deux nouvelles parties de suite diffèrent. Punchline J31.
- **Sous la nouvelle forme :** pression du commun. La graine appartient au commun (`CONTRAT.md:38`), et le hasard arrive au jeu comme une entrée (`MANIFESTE.md:63`).
- **Étage :** commun.

### CAT-93 · L'espace des graines n'est jamais réduit au point de rendre la partie devinable (HAS-05)
- **Ce qui arrive :** au poker d'ASF Software, la graine valait les millisecondes depuis minuit : environ 86 millions de paquets possibles au lieu de 52!, et des chercheurs prédisaient tout le paquet en direct. Entre amis, la menace sérieuse est autre : une graine tronquée fait revenir les mêmes parties.
- **D'où on le sait :** `soiree/certifications/hasard.md:50`. Datamation, même article. Chez nous : `the_whole_seed_counts`, `games/punchline/crates/core/src/rng.rs`.
- **Mécaniques :** graine tirée d'une horloge, tronquée en 32 bits, dérivée d'un code de salle court.
- **Question :** la graine que le commun livre est-elle entière (au moins 64 bits d'entropie), jamais dérivée d'une horloge ni d'un code de salle ? Deux graines qui ne diffèrent que par leurs bits de poids fort donnent-elles deux parties différentes ?
- **Pression :** paires de graines `a` et `a ^ (b << 32)` ; premiers tirages comparés. Punchline J31.
- **Sous la nouvelle forme :** pression du commun. La graine est au commun (`CONTRAT.md:38`). La moitié « la simulation lit une horloge » est impossible par la forme : `Instant::now` et `SystemTime::now` sont interdits au jeu (`CONTRAT.md:157`).
- **Étage :** commun.

### CAT-94 · Seules les actions de jeu acceptées consomment du hasard (HAS-06)
- **Ce qui arrive :** dans le mode multijoueur de RimWorld (2026), construire une boîte de confirmation tirait un nombre sur la machine du seul joueur qui cliquait ; la suite des tirages divergeait d'une machine à l'autre. Corrigé en isolant ce générateur (PR #968). Chez Punchline, un « Je sèche » refusé changeait le tirage.
- **D'où on le sait :** `soiree/certifications/hasard.md:57`. rwmt/Multiplayer, issue #967 (https://github.com/rwmt/Multiplayer/issues/967). `games/punchline/LEARNINGS.md:199` ; `refused_input_changes_nothing` (`games/punchline/crates/core/src/tests/properties.rs:446`) et `a_refused_stock_answer_draws_nothing` (`games/punchline/crates/core/src/tests.rs`).
- **Mécaniques :** actions refusées, consultations, reconnexions, bots, animations qui tirent au sort.
- **Question :** l'affichage tire-t-il son hasard de décor (confettis, variations) d'un générateur à lui, hors de l'état de jeu ?
- **Pression :** deux flux entrelacés, actions de jeu et messages refusés ou consultatifs ; état final et générateur comparés à ceux du seul flux de jeu. Punchline J31.
- **Sous la nouvelle forme :** impossible par la forme. Un refus ne change rien : le jeu ne produit un nouvel état que s'il accepte l'entrée (`CONTRAT.md:26`), et le générateur fait partie de l'état. Une vue se calcule depuis l'état sans l'écrire (`CONTRAT.md:24`). Une reconnexion est au commun et n'arrive pas au jeu. Le commun compare la mémoire entière, générateur compris (`CONTRAT.md:23`).
- **Étage :** commun.

### CAT-95 · Une question ne revient jamais avant que le paquet soit épuisé, ni deux fois de suite entre deux paquets (HAS-07)
- **Ce qui arrive :** dans Drawphone, deux des six mots proposés étaient identiques. Dans un clone de Codenames, des mots revenaient d'une partie à l'autre, au remélange du paquet selon le mainteneur. Tetris règle la répétition par un « sac » de sept pièces, avec un défaut connu : deux sacs qui se touchent peuvent répéter une pièce. Un doublon dans le fichier de contenu casse tout cela.
- **D'où on le sait :** `soiree/certifications/hasard.md:64`. Drawphone, issue #142 (https://github.com/tannerkrewson/drawphone/issues/142). jbowens/codenames, issue #112 (https://github.com/jbowens/codenames/issues/112). Tetris Wiki, « Random Generator » (https://tetris.wiki/Random_Generator). Chez nous : `a_prompt_waits_for_the_deck_to_be_spent` et `the_content_holds_nothing_twice` (`games/punchline/crates/core/src/tests/promises.rs`).
- **Mécaniques :** paquets de consignes, de questions, de mots, de pièces ; parties enchaînées.
- **Question :** sur N tirages dans un paquet de taille P, chaque élément sort-il au plus ⌈N/P⌉ fois ? Le dernier d'un paquet peut-il être le premier du suivant ? Les parties enchaînées continuent-elles le même paquet ? Ton contenu est-il sans doublon ?
- **Pression :** P, N et graine tirés, tirages à travers plusieurs « rejouer » ; comptes et frontières. Punchline J25. Les fichiers de contenu sont validés à leur chargement et vérifiés par un test (`CONTRAT.md:173`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-96 · Mêler des éléments imposés et des éléments tirés ne crée jamais de doublon (HAS-08)
- **Ce qui arrive :** dans le module Quiz de Drupal (2008), avec deux questions toujours posées et trois tirées parmi six, un joueur a reçu une question imposée en double. La cause n'a pas été établie.
- **D'où on le sait :** `soiree/certifications/hasard.md:71`. Drupal, issue #236663 (https://www.drupal.org/node/236663).
- **Mécaniques :** question finale imposée plus questions tirées, tutoriel plus tirage, consigne où l'on insère un prénom.
- **Question :** pour des ensembles imposé et aléatoire qui se recouvrent, la liste d'une manche est-elle sans doublon d'identifiant et de la taille annoncée, sinon une erreur explicite dit-elle que le paquet est trop petit ?
- **Pression :** deux ensembles générés avec des identifiants communs ; manche construite et vérifiée. Punchline J25.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-97 · Sur une partie, chacun est désigné à peu près autant que les autres (HAS-09)
- **Ce qui arrive :** principe, pas bug vérifié. Un pur hasard produit des séries et de longues absences, que les joueurs remarquent. Dota 2 utilise une distribution pseudo-aléatoire dont la probabilité grandit à chaque échec.
- **D'où on le sait :** `soiree/certifications/hasard.md:78`. Diplograph, « Dota 2 PRD » (https://diplograph.net/notes/games/randomness/dota-2-prd). Punchline a choisi exprès le pur hasard pour les duels (`games/punchline/verification.toml`, décision HAS-09).
- **Mécaniques :** premier joueur, juge, dessinateur, rôle secret, adversaire de duel.
- **Question :** sur R manches et N joueurs, chacun est-il désigné ⌊R/N⌋ ou ⌈R/N⌉ fois ? L'écart entre deux désignations d'un même joueur est-il au plus 2N−1 ? Si ton jeu veut un pur hasard, l'as-tu écrit ?
- **Pression :** N, R et graine tirés ; attributions jouées sans jouer les manches ; comptes. Un sac de joueurs donne ces garanties. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

---

## Affichage (`affichage.md`)

### CAT-98 · Aucun nombre affiché n'a plus de chiffres que l'écran n'en prévoit (AFF-01)
- **Ce qui arrive :** dans Touhou 4 (PC-98), le score est stocké sur 8 chiffres décimaux ; à 100 millions, le chiffre de tête dépasse 9 et s'affiche comme une lettre. Dans Donkey Kong, le temps du niveau 22 ne tient plus sur un octet.
- **D'où on le sait :** `soiree/certifications/affichage.md:7`. ReC98, décembre 2024 (https://rec98.nmlgc.net/blog/2024-12-04). heise sur Donkey Kong (https://heise.de/-10275685). Chez nous : `the_highest_score_fits_the_screen_and_its_type` (`games/punchline/crates/core/src/tests/promises.rs`) ; Sidequest J34 (`the_final_board_holds_at_its_worst_case`).
- **Mécaniques :** scores, argent, compteurs, multiplicateurs, temps.
- **Question :** pour chaque nombre de tes vues, quelle est sa largeur maximale, écrite à côté de la constante qui le borne ? Le maximum atteignable, calculé depuis les règles, tient-il dans cette largeur ?
- **Pression :** maximum théorique calculé et formaté ; chaque nombre de la vue télé vérifié après chaque pas des sessions. Punchline J32, Sidequest J34.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-99 · Le temps restant affiché ne devient jamais négatif, ne remonte jamais, et vaut 0 au changement de phase (AFF-02)
- **Ce qui arrive :** dans KDE Knights, à l'expiration, l'horloge affichait 00:00 puis 59:59 : des ticks arrivaient sous zéro pendant l'arrêt ; corrigé en 2016 en figeant l'affichage. Chez Punchline, une vue renvoyée après une reconnexion allongeait le compte à rebours ; une vue porte maintenant le temps restant et l'heure de l'hôte où elle a été faite, et l'écran retire le retard qu'il mesure.
- **D'où on le sait :** `soiree/certifications/affichage.md:14`. KDE, bug 330398 (https://bugs.kde.org/show_bug.cgi?id=330398). `games/punchline/LEARNINGS.md:139` (« Les comptes à rebours »). Sidequest J48 (`the_time_left_counts_down_stops_with_the_pause_and_goes_with_the_round`).
- **Mécaniques :** comptes à rebours sur la télé et sur les téléphones.
- **Question :** le pas courant qu'un appareil estime peut-il reculer, même après une reconnexion ? Un compte à rebours se calcule-t-il sur l'appareil, depuis une échéance de l'état et ce pas, plutôt que depuis un temps restant figé dans une vue ?
- **Pression :** temps restant de chaque vue relevé après chaque pas, avec des instants qui reculent ou se répètent ; bornes [0, durée] et monotonie par phase. Punchline J32, Sidequest J48.
- **Sous la nouvelle forme :** pression du commun. Les appareils partagent des numéros de pas, jamais une heure, et le commun estime l'écart des horloges en continu (`CONTRAT.md:98`). Il compense aussi le retard de chaque appareil (`CONTRAT.md:24`). Un réaffichage ne peut plus allonger le compte ; la monotonie du pas estimé est au commun. Le jeu borne sa soustraction (CAT-67).
- **Étage :** commun.

### CAT-100 · Le rang est calculé sur la même valeur que celle qui est affichée (AFF-03)
- **Ce qui arrive :** sur CodinGame, deux joueurs aux scores identiques à 20 décimales échangeaient leurs places : le classement tenait compte des erreurs de précision.
- **D'où on le sait :** `soiree/certifications/affichage.md:21`. Forum de CodinGame, « The Great Dispatch » (https://forum.codingame.com/t/the-great-dispatch-leaderboard/16767).
- **Mécaniques :** classement, podium, scores en flottants ou à décimales cachées.
- **Question :** deux scores qui s'affichent pareil ont-ils le même rang ? Recalculer le classement sans changement de score donne-t-il le même résultat ? Tes scores sont-ils entiers ?
- **Pression :** scores qui diffèrent sous la précision affichée ; rangs et valeurs comparés dans la vue. Punchline J20, Sidequest J30 (scores entiers).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-101 · Les pourcentages de votes affichés font 100 et ne trompent pas (AFF-04)
- **Ce qui arrive :** dans les sondages de Discourse, l'arrondi arrondissait tout vers le bas, puis ajoutait 1 aux premiers éléments sans regarder les décimales : 69 votes sur 106 (65,09 %) s'affichaient 66 %.
- **D'où on le sait :** `soiree/certifications/affichage.md:28`. Discourse Meta (https://meta.discourse.org/t/rounding-of-poll-percentages-is-imprecise/52319).
- **Mécaniques :** répartition des votes, barres de sondage, « 60 % ont choisi ».
- **Question :** la somme des pourcentages affichés vaut-elle 100 (ou tous 0 sans vote) ? Chacun est-il à moins d'un point de la valeur exacte ? Deux options à égalité ont-elles le même pourcentage, sauf l'écart inévitable attribué par une règle écrite ? Le résultat dépend-il de l'ordre des options ?
- **Pression :** vecteurs de 2 à 8 options avec beaucoup d'égalités ; la méthode du plus fort reste tient les quatre conditions. Aucun de nos jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-102 · Un message qui contient un nombre est toujours accordé (AFF-05)
- **Ce qui arrive :** dans gtali (GNOME), « %s wins the game with %d points » était un format fixe ; le correctif de 2003 passe par `ngettext`. Les traducteurs rappelaient que le polonais choisit la forme selon le dernier chiffre. En français, 0 et 1 sont au singulier.
- **D'où on le sait :** `soiree/certifications/affichage.md:35`. GNOME, bug 106697 (https://bugzilla.gnome.org/show_bug.cgi?id=106697). Chez nous : `a_counted_message_agrees_with_its_number` (Punchline J32), `counts_agree_with_their_noun` (Sidequest J35), `notices_agree_with_their_number` (tier-list J9).
- **Mécaniques :** « 1 point », « 0 joueur a voté », « plus que 2 secondes », annonces.
- **Question :** chaque message qui contient un nombre passe-t-il par une seule fonction d'accord, testée pour n dans {0, 1, 2, 11, 21, 100, maximum} ?
- **Pression :** test de table sur la fonction ; propriété qui cherche « 1 points » ou « 0 joueurs » dans les vues sérialisées. Les trois jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### CAT-103 · Chaque écran de la télé a son pire cas, construit à partir des limites (AFF-06)
- **Ce qui arrive :** chez Punchline, dans une fenêtre plus étroite que 16:9, le texte des bulles sortait de son cadre. Rien ne garantissait qu'une réponse de 80 caractères, ou trois en finale, tienne à l'écran : la mise en page n'avait été vérifiée qu'en 1600 × 900. Un harnais qui regarde la télé de l'extérieur ne voit que les états qu'il atteint par hasard.
- **D'où on le sait :** `soiree/certifications/affichage.md:42` et l'introduction du fichier (`affichage.md:3`). `games/punchline/LEARNINGS.md:170` (« Mise en page déterministe de la TV »). La galerie `pub mod gallery` (`games/punchline/crates/core/src/lib.rs:20`) et `gallery_has_a_worst_case_for_every_tv_screen` (`games/punchline/crates/core/src/tests.rs`).
- **Mécaniques :** tout écran de télé au contenu variable.
- **Question :** pour chaque variante de vue (chaque phase, chaque sorte d'écran), le jeu fournit-il un état où chaque champ variable est à sa limite (textes les plus longs en lettres larges, table pleine, plus grands nombres, égalité générale), construit depuis les constantes et non à la main ?
- **Pression :** un test qui énumère les variantes de vue par un `match` exhaustif et vérifie que la galerie couvre chacune ; la galerie ouverte à tous les formats de l'ancien T3. Punchline J32, Sidequest J34.
- **Sous la nouvelle forme :** pression du commun. L'affichage est au commun, dans un langage de dessin portable (`CONTRAT.md:44`) : il peut mesurer un débordement sans navigateur, à tous les formats. Il peut demander à chaque jeu une galerie de pires cas, comme il lui demande ses bots (`CONTRAT.md:136`).
- **Étage :** commun.

### CAT-104 · Le contenu du jeu respecte les limites de la télé, une fois les prénoms insérés (AFF-07)
- **Ce qui arrive :** chez Punchline, une consigne doit faire au plus 100 caractères une fois le prénom inséré. Un contenu ajouté plus tard, par les amis ou par une IA, peut dépasser sans que personne le voie.
- **D'où on le sait :** `soiree/certifications/affichage.md:49`. `games/punchline/LEARNINGS.md:170`, point 4. `MAX_PROMPT_CHARS` (importé dans `games/punchline/crates/core/src/lib.rs:27`) et l'invariant `prompts_fit_the_tv` (`games/punchline/crates/core/src/tests/properties.rs:424`).
- **Mécaniques :** fichiers de consignes, de questions, de mots ; gabarits qui insèrent un prénom ou une réponse.
- **Question :** chaque élément du contenu, avec le prénom le plus long en caractères larges, respecte-t-il la limite de son emplacement ? Le contenu ajouté plus tard passe-t-il par le même test ?
- **Pression :** test sur tout le contenu avec le prénom le plus long, et invariant dans les sessions. Punchline J32, Sidequest J36 et J37. Les fichiers de contenu sont validés à leur chargement (`CONTRAT.md:173`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

---

## Leçons transversales (README, introductions, décisions des jeux)

### CAT-105 · Un contrôle doit échouer quand on casse la règle exprès (README)
- **Ce qui arrive :** un test qui passe aussi quand la règle est cassée ne teste rien. Il donne une fausse garantie, la pire de toutes.
- **D'où on le sait :** `soiree/certifications/README.md:23` (« vérifie qu'il échoue quand tu casses exprès la règle »).
- **Mécaniques :** toutes.
- **Question :** pour chaque invariant d'un jeu, une faute injectée dans son code le fait-elle échouer ?
- **Pression :** l'injection de bugs de l'épreuve (`CONTRAT.md:250`) ; « un contrôle doit savoir échouer » (`CONTRAT.md:254`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### CAT-106 · Chaque leçon reçoit une décision écrite, et dans le doute on l'adopte (README)
- **Ce qui arrive :** un item que personne n'a tranché est un item oublié. Écarter sans raison cache un oubli. Les trois jeux ont décidé des 97 items ; Sidequest en a laissé deux « unreviewed » (TR-03, TXT-05), chacun avec ce qui manque pour trancher.
- **D'où on le sait :** `soiree/certifications/README.md:22` (« Dans le doute, prends l'item ») et `:41` (« Sans décision explicite, un item reste non revu »). Les `catalogue_decisions` des trois `verification.toml`.
- **Mécaniques :** toutes.
- **Question :** chaque question de la base de leçons qui vise les mécaniques déclarées a-t-elle une réponse, adoptée ou sans objet, avec sa raison ?
- **Pression :** l'examen de l'épreuve : une question sans réponse bloque, et une applicabilité inconnue n'est jamais une exclusion (`CONTRAT.md:238`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### CAT-107 · Une leçon s'applique souvent sous un autre nom (README)
- **Ce qui arrive :** un agent qui ne lit que les titres manque les leçons de son jeu : la « consigne » du catalogue s'appelle chez lui « mot » ou « défi ». Les jeux l'ont vécu : Sidequest a lu VOTE-01 comme « chaque joueur compte une fois dans sa zone », la tier-list a lu ECO-05 comme la copie périmée d'un autre onglet.
- **D'où on le sait :** `soiree/certifications/README.md:21`. Décisions VOTE-01 de Sidequest et ECO-05 de la tier-list.
- **Mécaniques :** toutes.
- **Question :** l'agent a-t-il lu chaque leçon de ses mécaniques en entier, et cherché sous quel nom elle vit dans son jeu ?
- **Pression :** aucune ; l'examen (CAT-106) en est la trace.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### CAT-108 · Un identifiant publié ne change jamais et n'est jamais réutilisé (README)
- **Ce qui arrive :** des jeux citent une leçon par son identifiant. Le renuméroter ou le réutiliser fait mentir leurs décisions, sans que rien ne casse.
- **D'où on le sait :** `soiree/certifications/README.md:67`.
- **Mécaniques :** toutes.
- **Question :** chaque identifiant d'une version du commun existe-t-il encore dans la suivante, retiré avec sa raison s'il le faut ?
- **Pression :** aucune encore ; un test du commun peut comparer les identifiants de deux versions.
- **Sous la nouvelle forme :** pression du commun. Les leçons sont versionnées avec le commun, et un jeu est jugé sur celles de sa version (`CONTRAT.md:226`).
- **Étage :** commun.

### CAT-109 · Une leçon née dans un jeu monte avec le test qui l'a trouvée, et un humain décide (README)
- **Ce qui arrive :** une faille trouvée dans un jeu peut menacer les autres. Sans procédure, elle reste dans un `LEARNINGS.md` que personne d'autre ne lit.
- **D'où on le sait :** `soiree/certifications/README.md:58-65` : l'agent la propose de lui-même par une pull request sur le commun, avec un lien vers le test du jeu ; l'hôte décide ; en attendant, la garantie reste dans le jeu.
- **Mécaniques :** toutes.
- **Question :** la proposition porte-t-elle sa source, le test qui a trouvé la faille, et la pression qui la vérifie ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance. Le contrat reprend la procédure (`CONTRAT.md:224`, `MANIFESTE.md:107`).
- **Étage :** connaissance.

### CAT-110 · Chaque origine est une source ouverte et lue, et sa faiblesse se dit (README)
- **Ce qui arrive :** une leçon tirée d'une source qu'on n'a pas lue est une rumeur. Une issue réduite à son titre ne prouve presque rien. Un principe présenté comme un bug trompe sur la force de la leçon.
- **D'où on le sait :** `soiree/certifications/README.md:69-73`. Le catalogue l'applique : EGA-07 et VOTE-07 se disent faibles, HAS-09 et TXT-06 se disent des principes.
- **Mécaniques :** toutes.
- **Question :** la source de chaque leçon a-t-elle été ouverte et lue ? Est-elle résumée, jamais recopiée ? Sa faiblesse est-elle écrite ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### CAT-111 · Un jeu ne réécrit jamais le réseau (README)
- **Ce qui arrive :** les coupures, les jetons, les paquets perdus ou hostiles sont des pièges du réel. Chaque jeu qui les traite à sa façon les redécouvre, souvent en pleine soirée.
- **D'où on le sait :** `soiree/certifications/README.md:16` ; `temps-reel.md:3`.
- **Mécaniques :** toutes.
- **Question :** aucune pour le jeu.
- **Pression :** le constat de ce à quoi le module a accès (`CONTRAT.md:247`).
- **Sous la nouvelle forme :** impossible par la forme. Le jeu est enfermé et n'atteint le monde que par le commun (`CONTRAT.md:10`, loi `CONTRAT.md:126`) ; le réseau appartient au commun (`CONTRAT.md:39`).
- **Étage :** commun.

### CAT-112 · Les propriétés d'un jeu se vérifient sur sa simulation seule, sans navigateur ni réseau (README)
- **Ce qui arrive :** une propriété qui demande un navigateur tourne lentement, rarement, et dépend du hasard des états atteints. Les items du catalogue étaient tous écrits sur `crates/core`, et « les vues sont des données » qu'on teste comme le reste.
- **D'où on le sait :** `soiree/certifications/README.md:12` ; `physique.md:5` ; `affichage.md:5`.
- **Mécaniques :** toutes.
- **Question :** aucune pour le jeu.
- **Pression :** les bots et les invariants que le jeu fournit (`CONTRAT.md:29`, `CONTRAT.md:136`).
- **Sous la nouvelle forme :** impossible par la forme. Un jeu est une fonction pure (`MANIFESTE.md:63`), et le même module tourne sans écran pour les tests, à vitesse machine (`CONTRAT.md:12`, `MANIFESTE.md:27`). Une vue est calculée depuis l'état (`CONTRAT.md:24`).
- **Étage :** commun.

### CAT-113 · Presque tous les jeux de soirée ont des manches, du texte, du hasard et des nombres à l'écran (README)
- **Ce qui arrive :** quatre familles reviennent dans presque chaque jeu : progression, texte, hasard, affichage. Ce qui revient partout est un candidat pour monter au commun, mais seul un humain le décide.
- **D'où on le sait :** `soiree/certifications/README.md:20` et `:56`. Les décisions des jeux le confirment en partie : Punchline et Sidequest adoptent des items des quatre familles ; la tier-list, éditeur à une seule personne, en adopte peu.
- **Mécaniques :** progression, texte, hasard, affichage.
- **Question :** pour chacune de ces familles, quelles leçons reviennent dans tous les jeux, et doivent-elles devenir des pressions du commun ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### CAT-114 · Le format des décisions dans `verification.toml` et `CERTIFICATION.md` (README)
- **Ce qui arrive :** le catalogue prescrivait un format : une garantie `J4` avec `claim`, `checks`, `origins`, `assumptions`, et une `catalogue_decisions` par item. Ce format a changé deux fois en une semaine (d'un `CERTIFICATION.md` écrit à la main vers un `verification.toml` dont il est une vue).
- **D'où on le sait :** `soiree/certifications/README.md:23-41`. Commits 6764852 et c9ea75c.
- **Mécaniques :** toutes.
- **Question :** aucune.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète. C'est un accident de l'ancienne architecture : le contrat le remplace par le document de chaque jeu, en trois parties (`CONTRAT.md:232-240`), dont la forme et le nom restent à décider. Le fond survit dans CAT-106 : chaque leçon a sa décision, chaque garantie ses contrôles et ses origines.
- **Étage :** commun.

### CAT-115 · Un échec se réduit à la plus petite scène qui casse encore (introduction de `physique.md`)
- **Ce qui arrive :** un échec trouvé sur une session de mille pas et huit corps ne dit pas sa cause. Réduit à deux corps et trois pas, il la montre.
- **D'où on le sait :** `soiree/certifications/physique.md:5`.
- **Mécaniques :** toutes, et d'abord la physique.
- **Question :** quand l'épreuve trouve un échec, le réduit-elle au plus petit cas qui casse encore, et l'enregistre-t-elle pour en faire une pression ?
- **Pression :** la réduction des tests de propriétés que les jeux utilisent déjà.
- **Sous la nouvelle forme :** pression du commun. Ce qui casse s'enregistre, et le commun rend facile d'en faire une pression (`CONTRAT.md:228`).
- **Étage :** commun.

### CAT-116 · À huit joueurs, les égalités arrivent bien plus souvent qu'à deux (introduction de `egalites.md`)
- **Ce qui arrive :** le soir même, huit amis jouent ensemble. Une égalité qu'un test à deux joueurs ne rencontre jamais arrive à chaque partie. Le hasard des tests doit ressembler à la soirée.
- **D'où on le sait :** `soiree/certifications/egalites.md:3`. EGA-06 tire ses scores dans un petit intervalle pour forcer les égalités (`egalites.md:57`).
- **Mécaniques :** toutes celles qui comptent ou classent.
- **Question :** les bots de l'épreuve jouent-ils au nombre maximal de joueurs, avec des scores serrés ?
- **Pression :** aucune générale ; seuls des items isolés forcent les égalités.
- **Sous la nouvelle forme :** pression du commun. Les pressions doivent ressembler au monde réel (`MANIFESTE.md:117`) ; le commun fait jouer les bots de chaque jeu.
- **Étage :** commun.

### CAT-117 · Une seule fonction de clé sert à toutes les comparaisons de texte (introduction de `texte.md`)
- **Ce qui arrive :** quand les doublons de prénoms, les réponses identiques et la bonne réponse d'un quiz ont chacun leur comparaison, deux textes peuvent être « le même » ici et « différents » là. La clé doit être unique, calculée par la partie qui fait foi, sans locale, idempotente.
- **D'où on le sait :** `soiree/certifications/texte.md:5`. Chez nous : `names_are_compared_once_cleaned` et `the_key_follows_the_rule` (Punchline J22).
- **Mécaniques :** texte.
- **Question :** ton jeu a-t-il une seule fonction de clé, utilisée par toutes ses comparaisons ? S'accorde-t-elle avec celle du commun pour les prénoms ?
- **Pression :** Punchline J22.
- **Sous la nouvelle forme :** question de mécanique. Deux morceaux qui doivent s'accorder n'ont qu'une seule définition (`MANIFESTE.md:82`).
- **Étage :** mécanique.

### CAT-118 · Un test statistique à graines fixées ne clignote pas (introduction de `hasard.md`)
- **Ce qui arrive :** un test du χ² sur un hasard libre échoue de temps en temps, et on finit par l'ignorer. Avec des graines fixées, il est déterministe : il passe ou échoue toujours.
- **D'où on le sait :** `soiree/certifications/hasard.md:5`.
- **Mécaniques :** hasard.
- **Question :** tes tests statistiques ont-ils des graines fixées, un nombre de tirages et un seuil choisis une fois, et gardés ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### CAT-119 · boardgame.io est la référence la plus proche pour le tour par tour (introduction de `tour-par-tour.md`)
- **Ce qui arrive :** boardgame.io a la même forme que nos jeux de plateau : un état qui fait autorité, des phases, des tours, des étapes et des joueurs actifs. Ses mainteneurs expliquent souvent la cause d'un bug. Le catalogue lui doit neuf items (TOUR-01, 03, 05, 06, 07, 08, PROG-07, PROG-08, VOTE-10).
- **D'où on le sait :** `soiree/certifications/tour-par-tour.md:5`.
- **Mécaniques :** tour par tour, phases.
- **Question :** pour un sous-système à tours et phases, a-t-on relu les issues fermées et les PR les plus discutées de boardgame.io ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance. Le contrat demande deux à quatre références par sous-système (`CONTRAT.md:189`) : celle-ci en est une.
- **Étage :** connaissance.

### CAT-120 · Une leçon n'énonce qu'une affirmation, et son titre dit la même chose que sa propriété (décisions des jeux)
- **Ce qui arrive :** VOTE-04 annonçait dans son titre que le bulletin d'un joueur parti ne compte plus, alors que sa propriété demandait seulement de suivre une règle écrite : Punchline l'a adopté « à l'inverse de son titre ». Sept items mêlaient deux leçons d'étages différents, et TOUR-07 recouvrait en partie PROG-07. Une leçon double se décide mal : on l'adopte pour une moitié et on écarte l'autre sans le dire.
- **D'où on le sait :** `games/punchline/verification.toml`, décision VOTE-04. Les coupures de ce fichier : PHY-04, TR-09, VOTE-06, VOTE-09, TOUR-07, ECO-04, PROG-02.
- **Mécaniques :** toutes.
- **Question :** chaque leçon de la base porte-t-elle une seule question, et son titre, sa question et sa pression disent-ils la même chose ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### CAT-121 · Le test réécrit la règle, sans réutiliser le code du jeu (`vote-quiz.md`, `tour-par-tour.md`, `egalites.md`)
- **Ce qui arrive :** un test qui calcule le résultat attendu avec le code du jeu reproduit ses erreurs et passe toujours. Le catalogue demande un modèle indépendant de la règle : le score de Punchline réécrit dans le test, un compteur de seuil minimal, un modèle de fin au temps.
- **D'où on le sait :** `soiree/certifications/vote-quiz.md:26` (« réécris ta règle de score dans le test […] sans réutiliser le code du jeu ») ; `tour-par-tour.md:67` ; `egalites.md:71`. Chez nous : `expected_outcome` (`games/punchline/crates/core/src/tests/properties.rs:751`) ; tier-list J12 (« chaque étape est comparée à un modèle indépendant »).
- **Mécaniques :** toutes celles qui comptent.
- **Question :** l'oracle de chaque invariant est-il écrit à partir des intentions du jeu, sans appeler son code ?
- **Pression :** l'injection de bugs (CAT-105) voit en partie un oracle qui réutilise le code fautif : la faute injectée passe alors des deux côtés.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.
