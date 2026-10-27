# Progression, blocages, fin et relance de partie (`PROG`)

> Pour tout jeu qui a des manches, des niveaux, une fin et un « on rejoue ». Le commun exige déjà qu'une partie se termine toujours (D2), qu'on la relance sans rien redémarrer (D5) et qu'aucune action ne soit acceptée dans une partie finie (E5). Ce fichier rend ces exigences plus précises : ce que « neuf » veut dire après une relance, ce qui ne doit jamais survivre d'une manche à l'autre, et ce qui ne doit arriver qu'une fois. Lis aussi `tour-par-tour.md` et `economie.md`.

### PROG-01 · Aucun numéro de manche ou de niveau ne produit un état impossible
- **Mécaniques :** manches qui s'enchaînent, difficulté qui monte, minuteurs qui dépendent du niveau, parties relancées sans fin dans la soirée.
- **Le bug d'origine :** dans Pac-Man, un débordement d'entier charge mal le 256ᵉ niveau, qui devient impossible à finir ([source](https://en.wikipedia.org/wiki/Pac-Man)). Dans Donkey Kong, le temps accordé au niveau 22 ne tient plus sur un octet et retombe à presque rien : Jumpman meurt au bout de quelques secondes (le « kill screen ») ([source](https://heise.de/-10275685)).
- **Propriété :** pour tout numéro de manche de 0 à la borne la plus haute que le jeu peut atteindre (et au-delà si « rejouer » enchaîne les compteurs), toutes les grandeurs dérivées (durées, nombre de consignes, points en jeu, vitesse) restent dans leurs bornes, et la manche reste gagnable.
- **Test :** pour chaque numéro de manche, construire l'état de départ de la manche (sans la jouer) et vérifier les bornes ; jouer la manche avec des robots et vérifier qu'elle se termine.
- **Adapter :** liste ce qui dépend du numéro de manche (multiplicateur, minuteur, taille de la grille). Voisins : ECO-09, TOUR-09.

### PROG-02 · Une manche ne lit jamais ce qu'une autre a laissé
- **Mécaniques :** tampons réutilisés d'une phase à l'autre, brouillons, numéros qui repartent de zéro, sélection courante, entrées en attente.
- **Le bug d'origine :** dans Pokémon Rouge et Bleu, MissingNo. : un tutoriel copie le nom du joueur dans le tampon des rencontres sauvages, et un lieu qui n'écrit pas ses propres rencontres lit ces restes comme des Pokémon ([source](https://en.wikipedia.org/wiki/MissingNo.)). Chez nous, Punchline : les brouillons d'une manche réapparaissaient à la suivante, parce que les numéros de consigne repartent de 0 à chaque manche ; ils sont maintenant effacés dès qu'on quitte l'écran d'écriture ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Bugs trouvés en jouant »).
- **Propriété :** pour toute partie, ce qu'une phase montre et accepte ne dépend que de ce qu'elle a elle-même construit ; une action, un brouillon ou une sélection désignée par un numéro local à la manche n'est jamais lue dans une autre manche.
- **Test :** remplir les champs hors phase avec des valeurs poison générées (brouillons, sélections, compteurs) au moment du changement de manche ; vérifier que la vue et le résultat de la manche suivante sont les mêmes qu'avec des champs vides.
- **Adapter :** la parade structurelle : chaque phase porte ses données dans sa variante d'`enum` (`Phase::Write { drafts, … }`), qui disparaît avec elle. Côté téléphone, l'état local se vide aussi au changement de phase. Voisins : PROG-03, VOTE-09.

### PROG-03 · Après « rejouer », l'état est celui d'une partie neuve avec les mêmes joueurs
- **Mécaniques :** relance de partie, retour au salon, nouvelle manche.
- **Le bug d'origine :** dans SuperTuxKart (issue #3019), les temps du mode « suivez le meneur » n'étaient pas remis à zéro au redémarrage, et le dernier cumulait le temps du premier ([source](https://github.com/supertuxkart/stk-code/issues/3019)). Dans l'issue #3820, un kart qui portait une bombe au moment du redémarrage explosait au départ suivant ([source](https://github.com/supertuxkart/stk-code/issues/3820)). Chez nous, Punchline vérifie qu'une nouvelle partie garde les joueurs présents, avec zéro point, et repart (`new_game_keeps_the_table`).
- **Propriété :** pour toute partie suivie de « rejouer », l'état obtenu est égal, champ par champ, à `Game::new(mêmes joueurs, mêmes réglages, graine)`, sans liste de champs à comparer : minuteurs, effets en cours, compteurs annexes et objets attachés compris.
- **Test :** jouer une suite d'actions au hasard, rejouer, puis comparer avec `==` à un état neuf construit avec les mêmes joueurs. Dériver `PartialEq` sur tout l'état : un champ ajouté plus tard est comparé d'office.
  ```rust
  proptest! {
      #![proptest_config(config())]
      #[test]
      fn a_new_game_is_a_fresh_game(seed in any::<u64>(), ops in ops()) {
          let mut run = Run::new(seed);
          for op in &ops { run.step(op); }
          let players = run.game.players_still_here();
          run.apply(Input::NewGame).unwrap();
          prop_assert_eq!(&run.game.without_seed(), &Game::lobby_with(&players, run.game.settings()).without_seed());
      }
  }
  ```
- **Adapter :** décide si la graine est neuve à chaque partie (sinon, on rejoue les mêmes questions : HAS-04) et sors-la de la comparaison. Voisins : PROG-04, D5 du commun, TR-10.

### PROG-04 · Quelle que soit la façon dont une partie finit, la suivante se joue avec les mêmes réglages
- **Mécaniques :** fin au temps, fin anticipée, fin parce que tout le monde est parti, puis relance.
- **Le bug d'origine :** dans un jeu de mots chronométré (smstamm/maple, issue #16), le score et les mots trouvés passaient à la partie suivante, et après l'expiration du chrono on ne pouvait plus envoyer de mot sans recharger la page ([source](https://github.com/smstamm/maple/issues/16)). Dans un clone de Codenames (jbowens/codenames, issue #113), le minuteur choisi ne s'appliquait qu'à la première partie ; corrigé par le commit 858c212 ([source](https://github.com/jbowens/codenames/issues/113)).
- **Propriété :** pour chaque façon de finir (chrono, arrêt par l'organisateur, plus personne, condition de victoire) suivie de « rejouer », au moins une action de jeu est acceptée ensuite, et la partie k se comporte comme la partie 1 avec les mêmes réglages, la même graine et les mêmes entrées.
- **Test :** générer la façon de finir, rejouer, puis jouer une suite d'entrées fixe ; comparer la trace à celle de la même suite sur un jeu neuf.
- **Adapter :** énumère toutes les sorties de ton `Phase::End` (ou équivalent). Voisins : PROG-03.

### PROG-05 · Aucune échéance ne se déclenche après la fin de sa partie ou de sa phase
- **Mécaniques :** minuteurs, comptes à rebours, échéances programmées, robots qui agissent « dans 2 s ».
- **Le bug d'origine :** dans words-treasure (issue #22), quand tous les joueurs partent, la partie est fermée mais son minuteur continue, et la fin de partie est appelée deux fois ([source](https://github.com/JumpyLionnn/words-treasure/issues/22)).
- **Propriété :** pour toute suite où une échéance arrive après un départ général, un retour au salon ou une relance, l'échéance périmée est sans effet ; chaque partie a exactement un événement « fin ».
- **Test :** chaque échéance porte le numéro de la partie et de la phase qui l'ont créée ; générer des fins et des relances juste avant des échéances, faire passer le temps, compter les fins par numéro de partie.
- **Adapter :** si l'échéance est calculée à partir de la phase courante (Punchline : `next_deadline()` dérivée de `phase_started`), il n'y a pas de minuteur à annuler : c'est la forme qui tient l'item par construction. Voisins : PROG-06, TR-10.

### PROG-06 · Les effets de la fin s'appliquent une seule fois
- **Mécaniques :** résultats enregistrés, statistiques, couronne du gagnant, podium, sons et confettis déclenchés par l'état.
- **Le bug d'origine :** dans un clone de Timberman (majiayu000/timberman-game, issue #8), une fin de partie en mode normal enregistrait la session deux fois : les totaux doublaient, mais la moyenne restait à peu près juste, ce qui masquait l'erreur ([source](https://github.com/majiayu000/timberman-game/issues/8)).
- **Propriété :** pour toute partie, dans tous les modes, l'effet « fin » (résultat écrit, ligne de statistiques, gain de fin) est appliqué exactement une fois, même si plusieurs causes de fin coïncident ou si le temps continue de passer après.
- **Test :** compter les applications de l'effet de fin dans un journal ; générer des fins par chaque chemin et de longues attentes après la fin ; croiser les modes de jeu, car le bug ne touchait qu'un mode.
- **Adapter :** Sidequest écrit un résultat par manche (`rounds.jsonl`) : c'est ce genre d'écriture qu'on compte. Voisins : PROG-05, TOUR-08.

### PROG-07 · Quand le dernier joueur attendu a joué, la phase passe tout de suite
- **Mécaniques :** fin anticipée d'une phase d'écriture, de vote ou de tour simultané.
- **Le bug d'origine :** dans boardgame.io (issue #769), la condition de fin de tour était évaluée avant la mise à jour des joueurs actifs ; le tour ne finissait donc pas quand le dernier avait joué ; corrigé en évaluant la condition après ([source](https://github.com/boardgameio/boardgame.io/issues/769)). Chez nous, la règle de Punchline : une phase se termine plus tôt quand tout le monde a répondu ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Nos choix »).
- **Propriété :** pour tout ordre de réponse, avec des départs intercalés, la phase passe au plus tard à l'action du dernier joueur encore attendu ; si le dernier attendu part, elle passe à son départ ; elle n'attend jamais l'échéance quand personne n'est plus attendu.
- **Test :** générer les réponses dans un ordre au hasard et des départs ; vérifier qu'après l'action ou le départ du dernier attendu, la phase a changé dans le même pas.
- **Adapter :** décide qui est « attendu » (les présents ? les absents depuis moins de 90 s ?) et écris-le. Voisins : VOTE-01, TOUR-07, PROG-08.

### PROG-08 · Le jeu n'attend jamais un joueur qui ne peut pas agir
- **Mécaniques :** joueurs actifs, tours, conditions de progression (tour de piste, objectif), écrans qui demandent une action.
- **Le bug d'origine :** dans boardgame.io (issue #964), au début du tour du joueur 1, la liste des joueurs actifs contenait encore le joueur 0, et le joueur courant n'avait plus aucun coup valide ([source](https://github.com/boardgameio/boardgame.io/issues/964)). Dans SuperTuxKart (issue #4760), un joueur qui passait la ligne à grande vitesse sur une piste n'avait pas son tour compté : il restait au tour 1, et la course ne finissait jamais ([source](https://github.com/supertuxkart/stk-code/issues/4760)).
- **Propriété :** pour tout état atteignable, chaque joueur que la phase attend a au moins une action valide sur son écran, ou bien l'avance du temps jusqu'à la prochaine échéance fait changer la phase ; la fin de partie est garantie par une échéance qui ne dépend pas du progrès des joueurs.
- **Test :** après chaque pas, pour chaque joueur attendu, demander au jeu ses actions valides (ou essayer celles que son écran propose) ; vérifier qu'au moins une est acceptée, sinon qu'une échéance existe.
- **Adapter :** les vues des téléphones disent ce qu'un joueur peut faire ; c'est le bon point d'appui pour ce test (le test de Punchline joue « ce que l'écran demande », `Op::Answers` et `Op::Votes`). Voisins : C6 et D2 du commun, TOUR-04.
