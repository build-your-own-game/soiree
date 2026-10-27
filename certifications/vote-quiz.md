# Vote et quiz (`VOTE`)

> Pour un jeu où les joueurs répondent, votent, buzzent ou désignent quelqu'un : Quiplash, Fibbage, quiz, loup-garou, Codenames, « qui est le plus… ». Lis aussi `egalites.md` (égalité au vote), `tour-par-tour.md`, `texte.md` (comparer des réponses) et `hasard.md` (tirer les questions).

Le commun couvre déjà le refus d'une action hors de sa phase ou d'un joueur non concerné (E5) et la fin de chaque phase à son échéance (C6, D2). Ces items portent sur ce qui se compte : qui a voté, combien de fois, pour quoi, et ce que cela rapporte.

### VOTE-01 · Un joueur a au plus une réponse et un vote par question, et « tout le monde a répondu » compte des joueurs distincts
- **Mécaniques :** réponses, votes, choix multiples, fin anticipée d'une phase quand tout le monde a joué.
- **Le bug d'origine :** dans ClassQuiz (issue #447, doublon de #441), le bouton d'envoi ne se désactivait pas et chaque envoi comptait ; comme la question se fermait quand le nombre de réponses atteignait le nombre de joueurs, un seul joueur pouvait la fermer pour tous. Une vérification côté serveur a été ajoutée (PR #455) ([source](https://github.com/mawoka-myblock/ClassQuiz/issues/447), [source](https://github.com/mawoka-myblock/ClassQuiz/issues/441)).
- **Propriété :** pour toute suite d'actions où des envois sont répétés ou rejoués, chaque question a au plus une réponse par joueur, et la phase ne se ferme en avance que lorsque chaque joueur attendu a répondu.
- **Test :** générer une suite d'actions, puis y insérer des copies d'actions déjà envoyées à des positions au hasard ; après chaque pas, compter les réponses par (joueur, question) et vérifier la condition de fin anticipée.
- **Adapter :** remplace « réponse » par chaque chose que ton jeu compte une fois par joueur (vote, mise, carte jouée). Voisins : VOTE-02, PROG-07.

### VOTE-02 · Renvoyer le même vote ne le change pas
- **Mécaniques :** votes qu'on peut changer, boutons « oui / non », sélection qu'on peut annuler.
- **Le bug d'origine :** dans Secret Hitler (issue #1596), un clic sur Ja ou Nein inversait le vote : sur une connexion lente, les joueurs recliquaient et leur vote se désélectionnait 10 à 15 s plus tard, au point de relancer des parties ([source](https://github.com/cozuya/secret-hitler/issues/1596)).
- **Propriété :** pour tout état et toute action de vote m, appliquer m deux fois donne le même état que l'appliquer une fois ; changer d'avis passe par une action explicite (`Vote(x)` remplace, `Retirer` retire), jamais par une bascule.
- **Test :** générer des suites où chaque action de vote est répétée de 1 à 3 fois ; comparer à la même suite sans répétitions.
- **Adapter :** toute action « bascule » (prêt / pas prêt, coché / décoché) a le même piège ; envoie l'état voulu, pas « inverse ». Voisins : VOTE-01, TR-04, ECO-11.

### VOTE-03 · Les points d'une manche ne sont comptés qu'une fois, et les totaux sont la somme des parts
- **Mécaniques :** points par vote, bonus, totaux d'équipe, cumul des manches.
- **Le bug d'origine :** dans OWASP CTF-in-a-box (issue #520), les totaux d'équipe comptaient deux fois les points de quiz (4 158 au lieu de 2 108) alors que les lignes individuelles étaient justes : deux passes d'agrégation attribuaient les mêmes points ([source](https://github.com/OWASP/owasp-ctf-in-a-box/issues/520)). Chez nous, le test de Punchline vérifie que le gain d'une manche est celui de ses duels et que le score monte exactement de ce gain (`gains_add_up`, `scores_follow_gains`).
- **Propriété :** pour toute partie, à chaque pas, le score de chaque joueur (ou équipe) est la somme des points de son journal de gains ; les points distribués par une manche sont ceux de la règle, recalculés dans le test à partir des votes.
- **Test :** tenir dans le test un journal indépendant des gains (qui, combien, pour quel duel) et un modèle de la règle de score ; vérifier après chaque pas que les scores du jeu en sont la somme, et qu'aucun duel n'a été payé deux fois.
- **Adapter :** réécris ta règle de score dans le test, à partir du `LEARNINGS.md`, sans réutiliser le code du jeu (Punchline : `expected_outcome`). Voisins : ECO-07, EGA-07.

### VOTE-04 · Le bulletin d'un joueur parti ne compte plus, et un revote repart d'une urne vide
- **Mécaniques :** votes avec départs en cours de manche, revote, vote à la majorité des présents.
- **Le bug d'origine :** dans « wrongest » (issue #23, 2015), quand un joueur partait pendant le vote, tout le monde revotait, mais les votes déjà émis restaient comptés ; corrigé par le commit ddff0dd ([source](https://github.com/AnotherDole/wrongest/issues/23)).
- **Propriété :** pour toute suite de votes, départs et retours pendant une phase de vote, le résultat ne compte que les bulletins que la règle accepte (par exemple, ceux des joueurs encore là), chaque votant a au plus un bulletin, et un revote commence sans aucun bulletin.
- **Test :** générer `Vote | Départ(p) | Retour(p) | Tick` pendant la phase de vote ; recalculer le résultat avec le modèle du test.
- **Adapter :** décide ce que vaut le vote d'un joueur parti puis revenu (salon le ramène à sa place, C2) : gardé ou annulé, mais écrit. Voisins : VOTE-05, C2 et C6 du commun.

### VOTE-05 · Si le joueur au cœur du vote part, la manche se termine quand même avec un résultat défini
- **Mécaniques :** accusé, imposteur, caméléon, juge, auteur d'une réponse, tout rôle dont la manche dépend.
- **Le bug d'origine :** dans un clone du Caméléon (jwayn/chameleon, issue #4, ouverte), si le caméléon quitte la partie pendant le vote, elle ne se termine jamais ([source](https://github.com/jwayn/chameleon/issues/4)).
- **Propriété :** pour tout joueur de rôle clé et tout instant de son départ pendant la manche, la manche se termine en un temps borné, avec le résultat que la règle prévoit pour ce cas.
- **Test :** générer une partie, puis un `Départ(rôle_clé)` à un indice au hasard ; faire passer le temps ; vérifier la fin de manche et le résultat.
- **Adapter :** liste tes rôles clés (celui qui dessine, celui qui fait deviner, le juge). Ce cas va plus loin que C6 (« un joueur parti ne bloque pas ») : ici, c'est le sujet même du vote qui disparaît. Voisins : TOUR-04, EGA-07.

### VOTE-06 · Un buzz simultané donne la main à exactement un joueur, qui ne se la fait pas reprendre
- **Mécaniques :** buzzer, « premier qui répond », « premier qui touche ».
- **Le bug d'origine :** sur qbreader (issue #153), lors d'une course au buzzer, personne n'obtenait la main et la question ne pouvait plus être passée ([source](https://github.com/qbreader/website/issues/153)). Dans l'issue #180, en buzz quasi simultané, l'écran montrait d'abord le buzz du rapporteur, puis le remplaçait par celui de l'autre ([source](https://github.com/qbreader/website/issues/180)).
- **Propriété :** pour tout lot de buzz arrivés dans le même tick, dans n'importe quel ordre, exactement un joueur obtient la main, selon une règle écrite qui respecte EGA-01 ; une fois donnée, la main ne change que sur une mauvaise réponse, une fin de temps ou un départ.
- **Test :** générer plusieurs buzz par tick, mélangés, et des départs du joueur qui a la main ; vérifier qu'il y a toujours zéro ou un détenteur, jamais deux, et qu'un buzz arrivé après n'écrase rien.
- **Adapter :** l'ordre d'arrivée chez l'hôte dépend du Wi-Fi ; si ton jeu veut départager à la milliseconde, utilise l'heure de l'hôte estimée par le téléphone (salon), et un tirage à la graine pour les égalités. Voisins : EGA-01, EGA-03.

### VOTE-07 · Chaque consigne va au bon nombre de joueurs, et chacun en reçoit autant que les autres
- **Mécaniques :** distribution de consignes, de duels, de mots, de dessins à deviner.
- **Le bug d'origine :** dans un clone de Quiplash (yodigi7/quiplash, issue #18), dans une partie à quatre, les joueurs n'avaient pas le même nombre de questions ; corrigé par la PR #28 ; l'issue n'a que son titre ([source](https://github.com/yodigi7/quiplash/issues/18)). Chez nous, Punchline vérifie après chaque pas que chaque consigne va à deux joueurs différents, et que chacun affronte deux rivaux différents (`each_prompt_goes_to_two_players`).
- **Propriété :** pour tout nombre de joueurs autorisé et toute graine, chaque consigne va à exactement le nombre prévu de joueurs distincts, chacun reçoit le même nombre de consignes (à la règle écrite près pour un nombre impair), personne ne reçoit deux fois la même, et personne ne s'affronte lui-même.
- **Test :** générer n dans `MIN..=MAX` et la graine ; lancer la manche ; vérifier les comptes. Faire aussi partir et arriver des joueurs avant le lancement.
- **Adapter :** remplace « consigne » par ton unité (mot à faire deviner, paire de duel). Voisins : HAS-09 (équité sur plusieurs manches), VOTE-08.

### VOTE-08 · Un contenu écrit par les joueurs est joué exactement une fois
- **Mécaniques :** mots proposés par les joueurs, dessins à faire circuler, réponses qui deviennent des consignes (téléphone arabe, Drawful).
- **Le bug d'origine :** dans Drawphone (issue #30), en mode « choisissez vos mots », le mot d'un joueur n'a jamais été joué et celui d'un autre est apparu deux fois ; le mainteneur n'a pas pu le reproduire ([source](https://github.com/tannerkrewson/drawphone/issues/30)).
- **Propriété :** pour tout ensemble de contributions (y compris des contributions identiques entre joueurs, vides, ou d'un joueur parti), l'attribution est une permutation : chaque contribution retenue est jouée exactement une fois, et la règle dit ce qui remplace une contribution manquante.
- **Test :** générer les contributions avec des doublons volontaires et des absents ; vérifier que le multiensemble des contributions jouées est celui des contributions retenues.
- **Adapter :** deux joueurs qui écrivent le même mot sont deux contributions, pas une : compare par identifiant, pas par texte. Voisins : ECO-01 (conservation), HAS-07.

### VOTE-09 · Un vote arrivé en retard ne tombe jamais sur la question suivante, et personne ne vote pour sa propre réponse
- **Mécaniques :** enchaînements rapides de votes, duels successifs, toute action qui ne nomme pas sa question.
- **Le bug d'origine :** chez nous, Punchline : `Vote { choice }` ne nomme pas son duel ; un vote parti pendant un duel et arrivé après le passage au suivant aurait compté pour le suivant. Chaque phase a maintenant son numéro, salon estampille chaque action de la phase que l'écran voyait, et l'hôte refuse les actions d'une phase passée ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Les actions périmées » ; tests `a_late_vote_does_not_land_on_the_next_duel` et `votes_come_from_voters`).
- **Propriété :** pour toute suite d'actions estampillées d'une phase courante ou ancienne, une action d'une phase ancienne ne change rien ; le numéro de phase change exactement quand la phase change (chaque duel a le sien) ; aucun vote ne vient d'un auteur de l'une des réponses.
- **Test :** générer des actions avec un tampon tiré entre « phase courante » et « une phase plus tôt » ; vérifier que les secondes ne changent rien, et que `phase_number` suit la phase (`phase_number_follows_the_phase`).
- **Adapter :** passe ton numéro de phase à salon (`Host::set_phase`) et donne un numéro distinct à chaque sous-étape qui accepte des actions. Voisins : E5 du commun, TOUR-01.

### VOTE-10 · Aucun secret ne fuit dans la vue d'un joueur avant sa révélation
- **Mécaniques :** votes cachés jusqu'à la révélation, auteur anonyme d'une réponse, rôles secrets, mot à faire deviner, main de cartes.
- **Le bug d'origine :** dans boardgame.io (issue #399), le filtre des secrets nettoyait l'état du jeu, mais les secrets passaient encore par l'historique d'annulation (`ctx._undo`) et l'état initial ; corrigé par la PR #400 ([source](https://github.com/boardgameio/boardgame.io/issues/399)).
- **Propriété :** pour tout état et tout joueur p, la vue envoyée à p (sérialisée, comme salon l'envoie) ne contient aucune valeur secrète pour p avant la phase qui la révèle ; la vue de la TV non plus, si le secret ne doit pas être à l'écran.
- **Test :** donner aux secrets des valeurs sentinelles reconnaissables (un auteur nommé « SECRET-7f3a », un mot « ZORGLUB ») ; après chaque pas, sérialiser chaque vue et y chercher les sentinelles qui ne devraient pas y être.
- **Adapter :** liste tes secrets et la phase où chacun devient public. Une vue construite champ par champ pour chaque écran (comme Punchline : `phone_view`, `tv_view`) protège mieux qu'un état complet qu'on filtre. Voisins : VOTE-09.

### VOTE-11 · Une question sans aucune réponse ni aucun vote se compte sans planter
- **Mécaniques :** quiz, votes, fin de phase par le temps alors que personne n'a joué.
- **Le bug d'origine :** dans ClassQuiz (issue #82), le jeu plantait quand personne ne répondait à une question ; corrigé par le commit f1c463c ([source](https://github.com/mawoka-myblock/ClassQuiz/issues/82)). Chez nous, la règle de Punchline : sans aucun vote, personne ne marque ; avec une seule réponse, l'autre gagne par forfait ; avec deux réponses identiques, personne ne marque ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Nos choix »).
- **Propriété :** pour toute combinaison « aucune réponse / une seule / toutes ; aucun vote / quelques-uns / tous », la manche se termine, les points sont ceux de la règle (sans division par zéro ni NaN) et l'écran de révélation a quelque chose de défini à montrer.
- **Test :** dans le générateur de sessions, donner un poids fort au silence total d'une phase (personne ne répond, puis on attend l'échéance) ; vérifier les points avec le modèle de règle de VOTE-03.
- **Adapter :** énumère les cas vides de ton jeu (personne n'a dessiné, personne n'a deviné, une seule équipe a joué). Voisins : EGA-07, VOTE-03.
