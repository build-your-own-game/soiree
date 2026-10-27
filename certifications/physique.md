# Physique et collisions (`PHY`)

> Pour un jeu où des corps bougent et se touchent : plateformes, bagarre, course, auto-tamponneuses, billard, tout ce qui a une position et une vitesse. Lis aussi `temps-reel.md` (pas de temps, commandes) et `egalites.md` (deux joueurs qui touchent la même chose au même tick).

Toutes ces propriétés se vérifient sur la simulation à pas fixe de `crates/core`, sans écran : on génère des positions, des vitesses et des suites d'entrées, on fait avancer les ticks, et on vérifie après chaque pas. Un échec se réduit à la plus petite scène qui casse encore.

### PHY-01 · Un corps rapide ne traverse jamais un mur fin sans le toucher
- **Mécaniques :** projectiles, chutes, dash, tout corps dont la vitesse par tick approche l'épaisseur d'un obstacle.
- **Le bug d'origine :** Glenn Fiedler explique qu'un pas de temps trop grand fait traverser les murs aux objets rapides ([source](https://gafferongames.com/post/fix_your_timestep/)). Dans Rapier (issue #524, 2023), une dalle en rotation traversait un sol en triangles sans aller vite : un coin tournait plus vite que la marge de prédiction ([source](https://github.com/dimforge/rapier/issues/524)). Le wiki de DDNet documente qu'avec assez de vitesse on saute au-delà d'un bloc « stopper » ([source](https://wiki.ddnet.org/wiki/Collision)).
- **Propriété :** pour toute position et vitesse initiales jusqu'au plafond de vitesse, tout mur d'épaisseur au moins la plus fine du jeu, et toute suite d'entrées, le segment parcouru pendant chaque tick ne passe jamais à travers l'intérieur d'un solide.
- **Test :** générer un mur d'un pixel (ou de l'épaisseur minimale du jeu) à une position au hasard, un corps lancé vers lui à une vitesse tirée entre 0 et `SPEED_CAP`, sous un angle quelconque. Après chaque tick, vérifier que le corps est du même côté du mur qu'au départ, ou en contact.
  ```rust
  proptest! {
      #![proptest_config(config())]
      #[test]
      fn nothing_tunnels_through_a_thin_wall(x in 0.0..WORLD_W, speed in 0.0..=SPEED_CAP, angle in 0.0..TAU) {
          let mut game = Game::with_wall_at(x, THINNEST_WALL);
          let side = game.side_of_wall(PLAYER);
          game.launch(PLAYER, speed, angle);
          for _ in 0..240 {
              game.tick();
              prop_assert!(game.side_of_wall(PLAYER) == side || game.touches_wall(PLAYER));
          }
      }
  }
  ```
- **Adapter :** remplace le mur par tes obstacles les plus fins et le corps par ton objet le plus rapide (balle, joueur en dash). Si tu fais de la détection continue (balayage), la propriété la vérifie ; sinon, borne la vitesse par tick sous l'épaisseur minimale et écris-le. Voisins : PHY-03 (plafond de vitesse), TR-01 (pas de temps).

### PHY-02 · Marcher sur un sol plat fait de tuiles ne bloque jamais à une jointure
- **Mécaniques :** sols et murs en tuiles, niveaux faits de blocs alignés, boîtes de collision rectangulaires.
- **Le bug d'origine :** Erin Catto (Box2D, 2020) décrit une boîte qui glisse sur deux arêtes alignées et se fait arrêter à leur jonction : en détection discrète, la plus courte sortie du chevauchement pointe vers l'arrière ([source](https://box2d.org/posts/2020/06/ghost-collisions/)).
- **Propriété :** pour tout découpage d'un sol plat en N tuiles de largeurs quelconques et toute entrée « avancer » tenue, la trajectoire est la même qu'avec un sol d'un seul tenant, et la vitesse horizontale ne tombe jamais à 0 avant le bout.
- **Test :** générer N entre 1 et 50 et des largeurs au hasard dont la somme est fixe. Jouer la même entrée sur le sol découpé et sur le sol entier ; comparer les positions tick par tick (à ε près si le jeu est en flottants).
- **Adapter :** remplace « sol » par tes surfaces composées (murs, plafonds, pentes raccordées). Voisins : PHY-11 (plateformes mobiles).

### PHY-03 · Aucune combinaison d'entrées ne dépasse le plafond de vitesse, dans un sens comme dans l'autre
- **Mécaniques :** accélération, sauts enchaînés, rebonds, poussées entre joueurs, boosts.
- **Le bug d'origine :** dans Super Mario 64 (1996), le saut en longueur multiplie la vitesse par 1,5 et ne la plafonne qu'en positif : à reculons, elle grandit sans fin (le « BLJ »). Le code décompilé le dit en commentaire au cas `ACT_LONG_JUMP` ([source](https://github.com/n64decomp/sm64/blob/master/src/game/mario.c)). Le bunny hopping de Quake vient d'une limite appliquée à l'accélération et non à la vitesse totale ([source](https://en.wikipedia.org/wiki/Bunny_hopping)). Chez nous, Sidequest vérifie déjà que les poussées ne dépassent jamais `SPEED_CAP` ([`sidequest/crates/core/tests/certification/invariants.rs`](https://github.com/build-your-own-game/sidequest/blob/main/crates/core/tests/certification/invariants.rs), `nobody_outruns_the_speed_cap`).
- **Propriété :** pour toute suite d'entrées, à chaque tick, `|vx|`, `|vy|` et `|v|` restent sous le plafond, et toutes sont finies.
- **Test :** un générateur qui vise les cas limites : saut à chaque tick, demi-tours à chaque tick, appui contre un mur ou une pente, foule qui pousse le même joueur, et surtout les directions négatives. Vérifier l'invariant après chaque tick, dans le test de sessions qui vérifie déjà les autres invariants.
- **Adapter :** remplace le plafond par celui de chaque objet (joueur, balle, kart). Si le jeu autorise un dépassement (boost), le plafond devient celui du boost, et la propriété dit combien de ticks il dure. Voisins : PHY-01, PHY-07 (énergie).

### PHY-04 · Aucune position, vitesse ou force ne devient NaN ou infinie
- **Mécaniques :** toute simulation en flottants.
- **Le bug d'origine :** dans Godot 4.4 (issue #104359), une force NaN passe dans la vitesse, la position, puis la détection de collisions, et contamine les corps en contact ([source](https://github.com/godotengine/godot/issues/104359)). Dans Box2D (issue #434), une friction infinie multipliée par zéro donnait NaN ([source](https://github.com/erincatto/box2d/issues/434)). Chez nous, Sidequest acceptait un bail `NaN`, car `lease_ms <= 0.0` est faux pour `NaN`, puis un « stop » le mémorisait et le replay ne se rejouait plus au bit près ([`LEARNINGS.md` de Sidequest](https://github.com/build-your-own-game/sidequest/blob/main/LEARNINGS.md), « Bail NaN » ; [`sidequest/crates/core/tests/replays.rs`](https://github.com/build-your-own-game/sidequest/blob/main/crates/core/tests/replays.rs)).
- **Propriété :** pour toute suite d'entrées, y compris des valeurs non finies là où une entrée porte un nombre, et pour tout réglage extrême mais fini, chaque champ flottant de l'état reste fini après chaque tick ; une entrée non finie est refusée ou ramenée à une valeur neutre dès l'entrée dans la simulation.
- **Test :** dans le générateur d'entrées, ajouter `f64::NAN`, `-f64::NAN`, `±f64::INFINITY`, `±1e300`, les sous-normaux et `-0.0` à chaque champ numérique. Après chaque pas, parcourir l'état et vérifier `is_finite()`. Attention aux comparaisons : écris `!(x > 0.0)` plutôt que `x <= 0.0` quand tu refuses.
- **Adapter :** liste tous les champs flottants de ton état (positions, vitesses, angles, minuteurs) et toutes les entrées qui portent un nombre. Voisins : PHY-05, TR-09 (axes de manette hostiles).

### PHY-05 · Un pas de durée nulle ou une direction de longueur nulle ne corrompt rien
- **Mécaniques :** normalisation de vecteurs (direction de déplacement, normale de contact), pas de temps variables, deux corps exactement superposés.
- **Le bug d'origine :** dans Rapier (issue #659, 2024), un pas avec `dt = 0` donnait des positions NaN aux corps reliés par une articulation ([source](https://github.com/dimforge/rapier/issues/659)). L'issue #77 du même projet cite la normalisation d'une vitesse nulle parmi les sources de NaN ([source](https://github.com/dimforge/rapier/issues/77)).
- **Propriété :** pour tout état, un pas de durée nulle laisse l'état inchangé ; pour toute entrée de direction, y compris `(0, 0)` et `(-0.0, 0.0)`, la direction calculée est finie, et une entrée nulle ne fait pas bouger ; deux corps de même centre se séparent selon une règle définie, sans NaN.
- **Test :** générer des états au hasard (par une suite d'entrées), puis appliquer un pas de 0 et comparer l'état avant et après. Générer des paires de corps au même centre et vérifier après un tick que leurs positions sont finies et distinctes.
- **Adapter :** si ta simulation n'a qu'un pas fixe, le cas `dt = 0` devient « un tick sans entrée ne crée pas de mouvement chez un corps au repos ». Voisins : PHY-04, PHY-09.

### PHY-06 · Aucun corps ne sort du monde ; s'il tombe dans un trou, il revient en jeu en un temps borné
- **Mécaniques :** arènes fermées, plateformes avec vide, pistes de course, objets éjectés.
- **Le bug d'origine :** dans SuperTuxKart (issue #1817), on pouvait passer sous la piste Cocoa Temple ; des plans de remise en piste ont été ajoutés, puis le bug est revenu au même endroit ([source](https://github.com/supertuxkart/stk-code/issues/1817)). Dans Rapier (issue #77), un objet en chute libre sans sol finissait par faire déborder un calcul de grille en `i32` et paniquer ([source](https://github.com/dimforge/rapier/issues/77)). Chez nous, Sidequest garde chaque joueur un rayon à l'intérieur du sol (`players_stay_on_the_floor`).
- **Propriété :** pour toute suite d'entrées sur N ticks, y compris « foncer vers le bord » et « se laisser tomber », chaque corps est dans la boîte du monde à chaque tick, ou il a été replacé en jeu en au plus K ticks ; aucune coordonnée ne dépasse la borne du monde.
- **Test :** générateur qui pousse vers les bords et les trous, plus des poussées de foule. Après chaque tick : position dans la boîte, ou compteur « hors jeu depuis » ≤ K.
- **Adapter :** remplace la boîte par la forme de ton monde, et K par ton délai de réapparition. Si tomber est une mécanique (éliminé), la propriété devient « tout corps hors du monde est éliminé dans le tick ». Voisins : PHY-01, PHY-12.

### PHY-07 · Sans entrée, l'énergie totale n'augmente jamais
- **Mécaniques :** rebonds, chocs entre joueurs, balles, billard, pendules, tout ce qui a un coefficient de restitution.
- **Le bug d'origine :** dans Godot 3.1 à 4.2 (issue #29639), des balles aux chocs élastiques accéléraient peu à peu jusqu'à disparaître ; la séparation de corps qui se chevauchent ne conservait pas l'énergie ([source](https://github.com/godotengine/godot/issues/29639)). Dans Rapier (issue #630, 2024), un pendule de Newton parfaitement élastique gagnait de l'énergie, à cause du réglage de séparation des corps ([source](https://github.com/dimforge/rapier/issues/630)).
- **Propriété :** pour tout ensemble de 1 à 20 corps (positions et vitesses au hasard, restitution ≤ 1), sans aucune entrée, l'énergie cinétique plus potentielle au tick t+1 est au plus celle du tick t, à ε près ; une balle lâchée ne rebondit jamais plus haut que son point de départ.
- **Test :** générer les corps, couper toutes les entrées, faire 600 ticks et vérifier la décroissance tick par tick.
- **Adapter :** remplace l'énergie par ce qui se conserve ou décroît dans ton jeu (vitesse totale d'auto-tamponneuses sans moteur, hauteur des rebonds). Voisins : PHY-03, PHY-08.

### PHY-08 · Un tas d'objets laissé seul finit par se poser, sans trembler ni s'enfoncer
- **Mécaniques :** empilements, caisses, joueurs entassés dans un coin, objets dans une fosse.
- **Le bug d'origine :** dans Godot (issue #2092, de 2015 à 2021), des corps empilés dans une fosse se chevauchaient nettement et tremblaient sans fin ; monter la physique à 180 Hz ne faisait qu'atténuer ([source](https://github.com/godotengine/godot/issues/2092)).
- **Propriété :** pour toute pile de N corps (N ≤ 30) lâchés au hasard, en au plus T ticks toutes les vitesses passent sous ε et y restent ; à chaque tick, la profondeur de chevauchement de chaque paire reste sous une tolérance.
- **Test :** générer N et les positions de départ au-dessus d'un sol, sans entrée ; mesurer la pénétration maximale à chaque tick et les vitesses après T ticks.
- **Adapter :** remplace les caisses par ce qui s'entasse dans ton jeu. Si les joueurs eux-mêmes s'entassent (foule dans une zone de vote), c'est le même test avec des joueurs immobiles. Voisins : PHY-09, PHY-07.

### PHY-09 · Deux joueurs ne restent jamais coincés l'un dans l'autre
- **Mécaniques :** collisions entre joueurs, apparition, téléportation, poussées.
- **Le bug d'origine :** dans Teeworlds (issue #1261), le code de collision coinçait des personnages l'un dans l'autre pendant plusieurs ticks ; le rapporteur soupçonnait le correctif qui empêchait de se traverser ([source](https://github.com/teeworlds/teeworlds/issues/1261)).
- **Propriété :** pour toute position de départ (y compris superposée) et toute suite d'entrées de 2 à 8 joueurs, le chevauchement de chaque paire devient nul en au plus K ticks et ne réapparaît que par une poussée ; deux points d'apparition ne se chevauchent jamais.
- **Test :** générer des positions de départ dont certaines se recouvrent exactement ou partiellement, puis des entrées au hasard ; compter pour chaque paire les ticks consécutifs de chevauchement et vérifier ≤ K. Vérifier à part que chaque apparition donne une place libre (Sidequest : premier point libre à 28 px de tout le monde).
- **Adapter :** remplace « joueurs » par tous les corps solides entre eux. Si les joueurs se traversent par règle (fantômes), écris-le et écarte l'item. Voisins : PHY-05 (centres confondus), PHY-10, EGA-01.

### PHY-10 · Un corps poussé contre un mur n'entre jamais dedans ni ne passe de l'autre côté
- **Mécaniques :** pousser un objet ou un joueur, être écrasé entre deux joueurs ou entre un joueur et un mur, plateformes qui écrasent.
- **Le bug d'origine :** dans Godot 3.1 à 3.5 (issue #31981), un corps cinématique ne gardait pas sa marge et finissait superposé au corps qu'il poussait ([source](https://github.com/godotengine/godot/issues/31981)). La recette de KidsCanCode explique qu'un personnage à inertie infinie fait traverser les murs aux objets qu'il pousse ([source](https://kidscancode.org/godot_recipes/4.x/physics/character_vs_rigid/)).
- **Propriété :** pour toute scène « mur + corps + 1 à 3 pousseurs » et toute suite de poussées, aucun centre de corps n'est dans un solide à aucun tick ; l'écrasement se résout par une règle déterministe écrite (le pousseur s'arrête, ou le corps écrasé est éliminé ou replacé), jamais par une expulsion à travers le mur.
- **Test :** générer la scène avec les pousseurs du même côté, tous poussant vers le mur ; vérifier après chaque tick que le corps poussé est du même côté du mur et hors de lui.
- **Adapter :** liste tes pousseurs (joueurs, plateformes, objets lourds). Voisins : PHY-01, PHY-09, PHY-06.

### PHY-11 · Un joueur posé sur une plateforme mobile ne passe pas au travers et ne flotte pas au-dessus
- **Mécaniques :** plateformes qui montent, descendent ou glissent, ascenseurs, tapis roulants, plateformes à sens unique.
- **Le bug d'origine :** dans Godot 3.0, sur une plateforme verticale à collision à sens unique, le joueur s'enfonçait d'un pixel en montée puis tombait au travers, et flottait d'un pixel en descente ; l'option « Sync to Physics » est arrivée en 3.1 ([source](https://forum.godotengine.org/t/issue-with-moving-platforms-and-one-way-collisions/25265)).
- **Propriété :** pour toute vitesse et direction de plateforme jusqu'à son maximum, et tout instant d'atterrissage, sans entrée de saut, le bas du joueur est égal au haut de la plateforme à chaque tick, à ε près.
- **Test :** générer la vitesse de la plateforme, son sens, et l'instant où le joueur s'y pose ; après l'atterrissage, comparer les deux hauteurs à chaque tick pendant une période complète de la plateforme.
- **Adapter :** remplace la plateforme par tout support qui bouge (dos d'un autre joueur, véhicule). Voisins : PHY-02, PHY-10.

### PHY-12 · Une longue partie ou un réglage extrême ne fait jamais diverger la simulation
- **Mécaniques :** toute simulation qui tourne toute une soirée, mondes très grands, masses très différentes.
- **Le bug d'origine :** dans Box2D (issue #508), un serveur multijoueur voyait après plusieurs jours ses rotations et translations passer à des millions, puis à NaN, puis planter ([source](https://github.com/erincatto/box2d/issues/508)). Dans Rapier (issue #629), un sol 2D très large (demi-largeur de 1e6 à 1e8) faisait gagner de l'énergie aux objets, et certains le traversaient ([source](https://github.com/dimforge/rapier/issues/629)).
- **Propriété :** pour toute graine, sur 10⁵ à 10⁶ ticks d'entrées au hasard, le maximum des positions et des vitesses reste sous ses bornes ; aux réglages extrêmes autorisés (plus grand monde, masses extrêmes), PHY-04, PHY-06 et PHY-07 tiennent encore.
- **Test :** un test long, marqué `#[ignore]` pour ne pas ralentir `cargo test`, lancé par `cargo test -- --ignored` avant une étape finie. Aux réglages extrêmes, rejouer les tests de PHY-04, PHY-06 et PHY-07.
- **Adapter :** la durée vise une soirée de 6 h : 6 × 3 600 × 60 ≈ 1,3 million de ticks à 60 Hz. Voisins : R2 du commun (mémoire), PHY-04.
