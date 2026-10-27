# Égalités et simultanéité (`EGA`)

> Pour tout jeu où deux joueurs peuvent faire la même chose au même moment, ou finir avec le même score : c'est-à-dire presque tous. Le soir même, huit amis jouent ensemble ; les égalités arrivent bien plus souvent qu'en test à deux. Lis aussi `vote-quiz.md` (égalité au vote, buzzer), `temps-reel.md` et `affichage.md`.

L'idée commune : **le numéro d'un joueur (ordre d'arrivée, port, siège) ne départage jamais rien à lui seul**. La forme la plus forte de cette règle est une propriété d'équivariance : renuméroter les joueurs renumérote le résultat, sans le changer.

### EGA-01 · Le numéro d'un joueur ne décide jamais seul qui gagne une simultanéité
- **Mécaniques :** deux joueurs qui ramassent le même objet, frappent, buzzent ou franchissent la ligne dans le même tick ; toute boucle `for player in players` qui applique les actions une par une.
- **Le bug d'origine :** dans les premiers Super Smash Bros., la « priorité de port » traite les joueurs dans l'ordre des manettes : le port le plus bas gagne les prises simultanées et les objets ramassés au même instant ; Brawl est passé au hasard ([source](https://www.ssbwiki.com/Port_priority)). Dans RuneScape Classic, le numéro attribué à la connexion fixait la priorité : le plus petit ramassait l'objet disputé et frappait le premier ([source](https://classic.runescape.wiki/w/Player_ID)).
- **Propriété :** pour tout état, toutes entrées et toute permutation σ des joueurs, `simuler(σ(état), σ(entrées)) == σ(simuler(état, entrées))`.
- **Test :** générer une scène qui force la simultanéité (deux joueurs à égale distance d'un objet, deux coups dans le même tick, deux buzz) ; générer σ avec `Just(players).prop_shuffle()` ; comparer les deux côtés après N ticks.
  ```rust
  proptest! {
      #![proptest_config(config())]
      #[test]
      fn renumbering_players_renumbers_the_outcome(seed in any::<u64>(), ops in ops(), order in Just((0..8).collect::<Vec<usize>>()).prop_shuffle()) {
          let plain = play(seed, &ops);
          let permuted = play(seed, &permute(&ops, &order));
          prop_assert_eq!(permute_state(&plain, &order), permuted);
      }
  }
  ```
- **Adapter :** si ton jeu a une règle de départage qui dépend d'un joueur (le meneur, le plus ancien), elle passe dans l'état et se permute avec lui. Si le départage est un tirage, il se fait avec la graine de la partie (HAS). Voisins : EGA-02, VOTE-06, PHY-09.

### EGA-02 · Des actions simultanées sont toutes évaluées sur l'état d'avant, puis appliquées ensemble
- **Mécaniques :** deux coups mortels échangés, deux joueurs qui prennent la dernière carte, deux tirs croisés, tour simultané (chacun choisit, puis tout se résout).
- **Le bug d'origine :** dans le tournoi de bots d'Entelect (bataille navale), quand le dernier navire d'un joueur coulait pendant un tour, le dernier tir de l'adversaire était rejeté : le joueur traité en premier gagnait. Corrigé en 1.0.2 pour évaluer tous les ordres avant toute mort ([source](https://forum.entelect.co.za/t/same-turn-kill-shot-loss-or-draw/78)). Dans Smash, deux coups qui se rencontrent avec des dégâts proches s'annulent tous les deux, symétriquement ([source](https://www.ssbwiki.com/Clang)).
- **Propriété :** pour tout tick où plusieurs actions s'excluent ou s'annulent, le résultat ne change pas si l'on inverse l'ordre de traitement des joueurs ; deux coups mortels échangés au même tick donnent un double K.O. ou une règle symétrique écrite.
- **Test :** construire des scènes où deux actions se contrarient dans le même tick ; faire tourner la résolution avec la boucle dans l'ordre et à l'envers (un drapeau de test), et comparer.
- **Adapter :** la façon simple de tenir l'item : lire un instantané immuable de l'état, calculer toutes les conséquences, puis tout appliquer. Voisins : EGA-01, TOUR-06.

### EGA-03 · Le classement d'une course suit les temps, et deux arrivées identiques reçoivent la même chose
- **Mécaniques :** courses, chronos, « premier qui trouve », buzzer, tout classement par instant d'arrivée.
- **Le bug d'origine :** dans SuperTuxKart (issue #5107), sur des arrivées serrées, le 2ᵉ affichait un meilleur temps que le 1er ([source](https://github.com/supertuxkart/stk-code/issues/5107)). Dans l'issue #4571, deux karts arrivés en même temps n'avaient pas les mêmes points : le jeu n'a pas de notion de même position et départage toujours par une interpolation sous l'image ([source](https://github.com/supertuxkart/stk-code/issues/4571)).
- **Propriété :** pour toute course, le rang i a toujours un temps au plus égal à celui du rang i+1 ; si deux temps sont égaux à la résolution du jeu, les rangs et les points sont égaux, ou le départage suit une règle écrite qui respecte EGA-01.
- **Test :** générer deux à huit coureurs qui franchissent la ligne dans le même tick à des fractions variées, dont des fractions identiques ; vérifier la monotonie et le traitement des égalités.
- **Adapter :** remplace la ligne par ton événement d'arrivée (bonne réponse, objectif atteint). Choisis la résolution (le tick, ou l'instant interpolé) et tiens-t'y partout, affichage compris. Voisins : EGA-06, TR-12.

### EGA-04 · Des scores égaux ne sont jamais présentés comme la victoire de l'un
- **Mécaniques :** fin de partie, fin de manche, duels, toute annonce d'un gagnant.
- **Le bug d'origine :** dans Battle for Wesnoth (issue #1341), une partie finie par la limite de tours avec un total d'or égal annonçait l'avantage au joueur 1 et la défaite au second : le calcul n'avait pas de cas d'égalité. Corrigé en 2018 (PR #3447) ([source](https://github.com/wesnoth/wesnoth/issues/1341)).
- **Propriété :** pour tout état final où les meilleurs scores sont égaux, le résultat est « égalité » (plusieurs gagnants), ou un départage dont la règle ne dépend pas du numéro des joueurs ; le texte annoncé est tiré du même résultat.
- **Test :** générer des parties puis forcer des scores égaux en tête (deux, trois, tous) ; vérifier le résultat et sa permutation (EGA-01).
- **Adapter :** décide une fois ce que ton jeu fait d'une égalité (couronne partagée, manche de départage, critère secondaire) et écris-le dans le `CERTIFICATION.md` du jeu. Voisins : EGA-05, AFF-03.

### EGA-05 · Le classement ne dépend que des scores et de la règle de départage
- **Mécaniques :** tableau des scores, podium, classement en direct.
- **Le bug d'origine :** dans ClassQuiz, un clone de Kahoot (issue #487), un joueur nommé « 4242 » restait en tête du classement avec 0 point, parce que les clés numériques d'un objet JavaScript passent en premier ; corrigé en triant un tableau par score (PR #516) ([source](https://github.com/mawoka-myblock/ClassQuiz/issues/487)). Dans l'issue #107 du même projet, l'écran de fin classait premier un joueur à 2/10 devant un joueur à 5/10 ([source](https://github.com/mawoka-myblock/ClassQuiz/issues/107)).
- **Propriété :** pour tout ensemble de joueurs, le classement est trié par score décroissant ; renommer les joueurs (noms numériques, vides, Unicode) ou les faire arriver dans un autre ordre ne change le classement qu'entre ex æquo, selon la règle écrite.
- **Test :** générer des noms dont `"[0-9]{1,5}"` et des textes vicieux, des scores avec collisions ; calculer le classement, renommer, réordonner, recalculer.
- **Adapter :** en Rust, une `BTreeMap` indexée par nom trie par nom, pas par score : le même piège sous une autre forme. Voisins : EGA-06, EGA-04.

### EGA-06 · Deux scores égaux ont toujours le même rang affiché
- **Mécaniques :** classement, podium, « 3ᵉ sur 8 » sur le téléphone.
- **Le bug d'origine :** dans le classement du greffon Gamification de Discourse, les scores égaux étaient numérotés à la suite : un utilisateur apparaissait 30ᵉ au lieu de 32ᵉ ; un réglage de méthode de rang a été ajouté en 2025 ([source](https://meta.discourse.org/t/display-same-rank-when-score-is-the-same/267579)). Le classement « 1224 » des compétitions sportives est décrit sur Wikipédia ([source](https://en.wikipedia.org/wiki/Ranking)).
- **Propriété :** pour tout vecteur de scores, `rang(i) == 1 + #{j : score(j) > score(i)}` ; deux scores égaux ont le même rang ; l'ordre d'affichage des ex æquo est stable (par exemple l'ordre d'arrivée), et le même sur la TV et sur les téléphones.
- **Test :** générer des scores dans un petit intervalle pour forcer les égalités ; comparer le rang de la vue TV et de chaque vue téléphone à la formule.
- **Adapter :** si ton jeu préfère le rang dense (1, 2, 2, 3), écris-le et teste cette formule-là. Voisins : AFF-03 (précision cachée), EGA-05.

### EGA-07 · Une égalité ne bloque jamais la phase qui doit la trancher
- **Mécaniques :** vote d'élimination, vote du meilleur, élection d'un rôle, départage.
- **Le bug d'origine :** dans le module de carte de missions d'EvokeNet (issue #41, ouverte), le système de vote se bloque en cas d'égalité ; l'issue n'a qu'un titre, sans description : source faible ([source](https://github.com/EvokeNet/moodle-block_missionmap/issues/41)). Chez nous, le test de Punchline réécrit la règle de score et y traite le duel sans aucun vote : personne ne marque, sans division par zéro (`expected_outcome`, [`punchline/crates/core/src/tests/properties.rs`](https://github.com/build-your-own-game/punchline/blob/main/crates/core/src/tests/properties.rs)).
- **Propriété :** pour toute répartition des votes, y compris les égalités à deux ou à n et l'absence totale de vote, la phase produit un résultat défini par la règle et passe à la suivante au plus tard à son échéance.
- **Test :** générer les urnes avec des comptes tirés dans {0, 1, 2} pour forcer les égalités ; vérifier qu'après l'échéance la phase a changé et que le résultat suit la règle.
- **Adapter :** si l'égalité déclenche un revote, borne le nombre de revotes (sinon deux joueurs têtus bloquent la soirée) et dis ce qui arrive au dernier. Voisins : VOTE-05, C6 et D2 du commun.

### EGA-08 · Quand un minuteur expire, le résultat tient compte de ce que l'autre pouvait encore faire, et le texte dit la même chose
- **Mécaniques :** pendule par joueur, temps de réflexion, défaite au temps, abandon.
- **Le bug d'origine :** sur Lichess (issue #3617), des parties perdues au temps contre un adversaire qui n'avait plus de quoi mater étaient comptées comme défaites au lieu de nulles ([source](https://github.com/lichess-org/lila/issues/3617)). Dans l'issue #10339, ces mêmes parties étaient nulles, mais le PGN écrivait que l'un gagnait au temps ([source](https://github.com/lichess-org/lila/issues/10339)). La règle FIDE 6.9 dit qu'un joueur dont le temps est écoulé perd, sauf si l'adversaire ne peut plus gagner ([source](https://handbook.fide.com/chapter/E012023)).
- **Propriété :** pour tout état et tout joueur dont le minuteur expire, le résultat est une fonction pure de (état, qui a expiré, ce que les autres peuvent encore obtenir) ; le libellé affiché est calculé à partir du même résultat, jamais à part.
- **Test :** générer des états finaux et des expirations au hasard ; comparer le résultat à un modèle écrit dans le test, et le libellé au résultat.
- **Adapter :** dans un jeu de soirée, « ne peut plus gagner » devient par exemple « ne peut plus rattraper le premier avec les points qui restent en jeu ». Voisins : EGA-04, PROG-07.
