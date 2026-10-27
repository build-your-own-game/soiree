# Tour par tour (`TOUR`)

> Pour un jeu où les joueurs jouent chacun leur tour, ou par étapes : jeux de plateau, de cartes, de dessin à tour de rôle, de stratégie, avec ou sans annulation. Lis aussi `progression.md` (blocages, fin de partie), `vote-quiz.md` et `egalites.md`.

Les bugs de ce fichier viennent surtout de boardgame.io, le cadre open source le plus proche de nos jeux : un état qui fait autorité, des phases, des tours, des étapes et des joueurs actifs. Ses mainteneurs expliquent souvent la cause, ce qui en fait une bonne mine pour enrichir ce fichier.

### TOUR-01 · Seul le joueur dont c'est le tour agit, annulation comprise
- **Mécaniques :** coups, pioche, fin de tour, annulation, toute action réservée au joueur courant.
- **Le bug d'origine :** dans boardgame.io (issue #345), le joueur 1 pouvait annuler pendant le tour du joueur 2 et défaire ses coups : rien ne vérifiait qui demandait l'annulation ; corrigé par la PR #408 ([source](https://github.com/boardgameio/boardgame.io/issues/345)). Le correctif, devenu trop strict avec les étapes, a ensuite été assoupli (issue #565) ([source](https://github.com/boardgameio/boardgame.io/issues/565)).
- **Propriété :** pour toute suite d'actions envoyées par n'importe quel joueur, une action de tour (annulation comprise) n'est acceptée que de la part d'un joueur actif, n'annule que ses propres coups du tour en cours, et une action refusée ne change rien.
- **Test :** générer `Coup(p) | Annuler(p) | FinDeTour(p)` avec p tiré parmi tous les joueurs, plus un inconnu ; vérifier l'auteur de chaque action acceptée, et que les refusées laissent l'état identique (y compris la graine).
- **Adapter :** si plusieurs joueurs jouent en même temps (étapes simultanées), « joueur actif » est l'ensemble de ceux qui ont encore à jouer. Voisins : E5 du commun, TOUR-02.

### TOUR-02 · Annuler un coup rend exactement l'état d'avant, jusqu'aux détails
- **Mécaniques :** annulation, retour arrière, « reprendre un coup ».
- **Le bug d'origine :** sur Lichess (issue #17937), après un retour arrière, une proposition de nulle restait attachée au numéro de coup et réapparaissait au coup suivant ; corrigé par la PR #18016 ([source](https://github.com/lichess-org/lila/issues/17937)). Dans Battle for Wesnoth (issue #11174), rappeler une unité puis annuler lui laissait 0 point de mouvement, car la remise à zéro du rappel n'était pas défaite ([source](https://github.com/wesnoth/wesnoth/issues/11174)). Chez nous, la tier-list vérifie qu'annuler une modification rend le plateau d'avant, et qu'annuler puis rétablir revient au même (`random_sessions_obey_the_undo_laws`, [`tier-list/crates/core/src/tests/properties.rs`](https://github.com/build-your-own-game/tier-list/blob/main/crates/core/src/tests/properties.rs)).
- **Propriété :** pour tout état atteignable s et tout coup annulable m, `annuler(appliquer(s, m)) == s`, par égalité structurelle complète : propositions en cours, compteurs, minuteurs, graine ; annuler puis rétablir revient à `appliquer(s, m)`.
- **Test :** générer un préfixe d'actions au hasard, puis m, puis l'annulation ; comparer avec `==` sur tout l'état (dériver `PartialEq` sur l'état, sans liste de champs à comparer).
- **Adapter :** si ton annulation stocke des différences plutôt que des copies, cette propriété est ce qui les garde honnêtes. Voisins : TOUR-03, PROG-03.

### TOUR-03 · Une annulation ne fait jamais revenir un joueur parti
- **Mécaniques :** annulation, joueurs qui quittent en cours de partie.
- **Le bug d'origine :** dans boardgame.io, un joueur parti restait dans l'ordre des tours et bloquait les autres (issue #1088) ; la PR #1258 le retire de l'ordre de jeu et des joueurs actifs, et recale l'historique d'annulation pour qu'on ne puisse pas revenir avant son départ ([source](https://github.com/boardgameio/boardgame.io/pull/1258), [source](https://github.com/boardgameio/boardgame.io/issues/1088)).
- **Propriété :** pour toute suite où un départ définitif est suivi d'annulations et de rétablissements, le joueur parti ne réapparaît jamais dans l'ordre de jeu ni parmi les joueurs actifs.
- **Test :** générer des `Départ(p)` intercalés entre des coups et des rafales d'annulations ; vérifier l'ordre de jeu après chaque pas.
- **Adapter :** un joueur absent que salon peut ramener (C2) n'est pas « parti » ; décide à partir de quand il l'est (Punchline : 90 s dans le salon). Voisins : TOUR-04.

### TOUR-04 · Le tour passe toujours à un joueur présent, sinon la manche se termine
- **Mécaniques :** ordre des tours, sièges vides, joueurs partis ou absents, robots remplaçants.
- **Le bug d'origine :** dans Battle for Wesnoth (issue #3899), une partie multijoueur dont le premier camp était vide produisait des commandes de tour illégales, puis une désynchronisation ; corrigé en 2024 ([source](https://github.com/wesnoth/wesnoth/issues/3899)). Dans Drawphone (issue #85), un joueur remplacé par le robot « fantôme » ne jouait jamais, et la partie restait bloquée ([source](https://github.com/tannerkrewson/drawphone/issues/85)).
- **Propriété :** pour tout ensemble de sièges vides ou de joueurs partis (y compris le premier siège et le joueur courant), après chaque transition le joueur courant est présent, ou la manche est finie ; la partie avance en un temps borné sans aucune action humaine.
- **Test :** générer un sous-ensemble de sièges vides au départ, puis des départs en cours de partie, dont celui du joueur courant ; vérifier le joueur courant après chaque pas, puis faire passer le temps et vérifier que la partie avance.
- **Adapter :** si un robot remplace l'absent, la propriété devient « le robot joue avant l'échéance ». Voisins : C6 et D2 du commun, VOTE-05, PROG-08.

### TOUR-05 · Une fin de phase n'avance jamais le tour deux fois, et chaque joueur joue une fois par tour de table
- **Mécaniques :** phases qui contiennent des tours, fin de phase qui finit aussi le tour.
- **Le bug d'origine :** dans boardgame.io (issue #193), le compteur de tours sautait de 1 à 3 à la fin d'une phase, car la fin de phase déclenchait la fin de tour et inversement ([source](https://github.com/boardgameio/boardgame.io/issues/193)). Dans l'issue #517, finir une phase finissait le tour sans avancer le joueur : le même joueur jouait deux tours de suite ([source](https://github.com/boardgameio/boardgame.io/issues/517)).
- **Propriété :** chaque transition fait avancer le compteur de tours d'au plus un ; sur n tours consécutifs sans départ, chaque joueur présent joue exactement une fois.
- **Test :** générer des configurations de phases (longueur, condition de fin) et des suites d'actions ; relever la suite des joueurs courants et vérifier qu'elle est une rotation.
- **Adapter :** si ton ordre change (sens inversé, joueur qui passe son tour), écris la règle et teste-la à la place de la rotation simple. Voisins : TOUR-06.

### TOUR-06 · Deux causes de fin de tour dans le même pas finissent un seul tour, sans perdre la demande explicite
- **Mécaniques :** limite de coups, fin de tour demandée, échéance du minuteur, départ du joueur : plusieurs déclencheurs qui peuvent coïncider.
- **Le bug d'origine :** dans boardgame.io (issue #924, ouverte), avec une limite d'un coup, une fin de tour qui désignait le joueur suivant était ignorée : la limite avait déjà clos le tour, et la demande était jetée comme un doublon ([source](https://github.com/boardgameio/boardgame.io/issues/924)). Dans l'issue #1237, une fin de tour demandée au début d'une phase était jetée sans erreur ; la PR #1266 la transforme en erreur explicite ([source](https://github.com/boardgameio/boardgame.io/issues/1237)).
- **Propriété :** pour tout pas où plusieurs causes de fin de tour arrivent ensemble, exactement un tour se termine ; le joueur suivant suit une priorité écrite (la demande explicite avant le défaut) ; une demande ignorée donne un refus observable, jamais un silence.
- **Test :** générer des pas qui combinent limite atteinte, fin demandée avec un joueur suivant, échéance et départ du joueur courant ; vérifier le nombre de tours finis et le joueur suivant.
- **Adapter :** énumère tes déclencheurs de fin de tour et leur priorité. Voisins : EGA-02, TOUR-05.

### TOUR-07 · Les conditions de fin se lisent sur l'état à jour, et une entrée ne déclenche qu'un nombre borné de transitions
- **Mécaniques :** conditions « tout le monde a joué », fins de tour en cascade, phases qui s'enchaînent.
- **Le bug d'origine :** dans boardgame.io (issue #899), une fin d'étape était traitée après le début du tour suivant et retirait le dernier joueur actif du nouveau tour ([source](https://github.com/boardgameio/boardgame.io/issues/899)). Le correctif a créé une boucle infinie : une condition de fin qui modifie les joueurs actifs se redéclenchait sans fin, et le détecteur de boucles ne la voyait pas (issue #979, corrigée par la PR #980) ([source](https://github.com/boardgameio/boardgame.io/issues/979)).
- **Propriété :** pour toute entrée, le nombre de transitions internes qu'elle déclenche est au plus une constante K ; quand le dernier joueur attendu agit, la fin se produit dans le même pas, jamais au pas suivant ni dans le tour d'après.
- **Test :** compter les transitions internes déclenchées par chaque entrée (un compteur dans l'état, ou un journal) ; vérifier ≤ K. Générer des actions simultanées où le dernier joueur termine.
- **Adapter :** si ton jeu enchaîne des phases instantanées (titre, puis consigne), compte-les aussi. Voisins : PROG-07, TOUR-06.

### TOUR-08 · La partie peut finir à tout moment, même avant le premier tour, et le coup final compte
- **Mécaniques :** conditions de victoire, paquet vide au départ, partie à zéro ou un joueur, fin immédiate.
- **Le bug d'origine :** dans boardgame.io (issue #788), une condition de fin déjà vraie à l'initialisation faisait planter le moteur ; corrigé par la PR #888 ([source](https://github.com/boardgameio/boardgame.io/issues/788)). Dans l'issue #80, le dernier coup de la partie manquait au journal ; corrigé par la PR #88 ([source](https://github.com/boardgameio/boardgame.io/issues/80)).
- **Propriété :** pour toute condition de fin, même satisfaite à l'étape 0 (paquet vide, zéro ou un joueur), l'hôte atteint un état « fini » sans panique ; le score et le journal comptent le coup qui a déclenché la fin ; après la fin, toute action est refusée sans rien changer (E5).
- **Test :** générer des configurations de départ dégénérées (zéro, un ou deux joueurs ; paquet vide ; objectif à 0) et des actions après la fin.
- **Adapter :** liste tes conditions de fin et essaie chacune au tout premier pas. Voisins : PROG-06, PROG-07.

### TOUR-09 · Un compteur à seuil déclenche son effet exactement au seuil, ni avant, ni après
- **Mécaniques :** limite de tours, échecs consécutifs, nombre de vies, « au bout de 3 refus… ».
- **Le bug d'origine :** dans Secret Hitler (issue #68), quand un veto portait le compteur d'élections ratées à 3, le jeu organisait une nouvelle élection au lieu d'appliquer l'effet prévu ; corrigé en 0.5.0 ([source](https://github.com/cozuya/secret-hitler/issues/68)). Dans Battle for Wesnoth (issue #11326), un succès prévu pour le tour 40 se déclenchait au tour 39, à cause d'un `-1` en trop ([source](https://github.com/wesnoth/wesnoth/issues/11326)).
- **Propriété :** pour toute suite d'événements, l'effet d'un seuil N se produit dans la transition où le compteur atteint N, par n'importe quel chemin (veto, échec, expiration), et le compteur repart comme la règle le dit ; une limite de L tours autorise exactement L tours.
- **Test :** un modèle de référence minimal dans le test (le compteur et son seuil) ; générer des suites d'échecs et de réussites par tous les chemins ; comparer.
- **Adapter :** remplace par tes seuils (manches, vies, cartes). Les bornes se testent toujours en N−1, N, N+1. Voisins : PROG-01.
