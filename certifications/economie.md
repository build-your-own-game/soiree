# Score, économie et conservation des objets (`ECO`)

> Pour un jeu qui compte quelque chose qui ne doit ni apparaître ni disparaître : points, pièces, jetons, cartes, objets à ramasser, tuiles à placer, ressources. Lis aussi `vote-quiz.md` (points des votes) et `progression.md` (relance).

L'idée commune : **tout ce qui se compte a un bilan**. La somme de ce qui existe ne change que par des sources et des puits nommés, et le test le vérifie après chaque pas. Les bugs de duplication les plus célèbres sont des oublis de ce bilan à un moment de transition : échange, sauvegarde, mort, fin de manche.

### ECO-01 · Un objet n'est jamais perdu ni dupliqué : chacun est à exactement un endroit
- **Mécaniques :** cartes en main, sur la table ou dans la pioche ; tuiles placées ou en réserve ; objets portés ou au sol.
- **Le bug d'origine :** chez nous, la tier-list : chaque tuile doit être placée exactement une fois (dans une ligne ou dans la réserve) ; une sauvegarde qui plaçait une tuile deux fois a été trouvée par le test des sauvegardes abîmées, et est maintenant refusée ([`tier-list/LEARNINGS.md`](https://github.com/build-your-own-game/tier-list/blob/main/LEARNINGS.md), « Écarts au manifeste », point 1 ; tests `random_actions_keep_the_board_consistent` et `a_save_placing_an_item_twice_is_refused`). Dans Luanti (issue #8229, ouverte), poser un objet incompatible sur une pile laissait au curseur une copie de la pile remplacée ([source](https://github.com/luanti-org/luanti/issues/8229)).
- **Propriété :** pour toute suite d'actions, chaque objet (par identifiant) est dans exactement un conteneur, et le nombre d'objets de chaque sorte est celui du départ, plus les créations et moins les destructions prévues par la règle.
- **Test :** après chaque pas, parcourir tous les conteneurs de l'état, compter les identifiants et vérifier qu'aucun n'apparaît deux fois ni ne manque. Générer les actions qui déplacent (prendre, poser, échanger, scinder, fusionner) entre conteneurs pleins, vides et inexistants.
- **Adapter :** liste tes conteneurs (mains, pioche, défausse, table, inventaires). Mieux : rends le doublon impossible à représenter (un seul `BTreeMap<ObjetId, Lieu>` plutôt qu'une liste par conteneur). Voisins : ECO-02, ECO-04, VOTE-08.

### ECO-02 · Un échange est appliqué des deux côtés ou pas du tout
- **Mécaniques :** échanges entre joueurs, achats à un marchand, dons, vols, toute action à deux parties.
- **Le bug d'origine :** dans Minecraft Java (1.14 et suivantes), le « void trading » : on décharge le villageois avant de fermer la fenêtre d'échange ; le joueur garde ce qu'il a reçu, mais le villageois n'enregistre jamais l'échange ([source](https://minecraft.wiki/w/Tutorials/Block_and_item_duplication)).
- **Propriété :** pour toute suite d'actions où un départ, une fin de manche ou un changement de phase s'intercale entre « proposer », « accepter » et « conclure », l'échange est appliqué en entier ou pas du tout, et le total des objets de tous les joueurs ne change pas.
- **Test :** générer des échanges en plusieurs étapes avec des interruptions au hasard entre elles ; vérifier ECO-01 après chaque pas.
- **Adapter :** dans un jeu à hôte unique, un échange peut être une seule entrée appliquée d'un coup : c'est la façon la plus simple de tenir l'item. Voisins : ECO-03, ECO-05.

### ECO-03 · Un échange impossible est refusé sans rien faire disparaître, et livre exactement ce qui était offert
- **Mécaniques :** inventaires bornés, mains limitées, échanges proposés puis conclus.
- **Le bug d'origine :** dans Veloren (issue #2172), un échange avec un marchand, inventaire plein, aboutissait sans erreur, et les objets s'évaporaient ([source](https://gitlab.com/api/v4/projects/veloren%2Fveloren/issues/2172)). Dans l'issue #1631, l'offre retenait des positions dans l'inventaire et non des objets : trier l'inventaire pendant l'échange changeait ce qui était offert ([source](https://gitlab.com/api/v4/projects/veloren%2Fveloren/issues/1631)).
- **Propriété :** pour toute suite d'échanges et de réordonnancements entrelacés, avec des inventaires proches de leur capacité, un échange qui ne tient pas est refusé et laisse l'état identique ; un échange accepté livre les objets offerts (par identifiant), quel que soit le réordonnancement entre l'offre et la conclusion.
- **Test :** générer des inventaires presque pleins, des offres, des tris et des déplacements, puis des conclusions ; vérifier les identifiants livrés.
- **Adapter :** une offre désigne des objets par identifiant, jamais par position. Voisins : ECO-02, E5 du commun.

### ECO-04 · Une transition (fin de manche, sauvegarde, rechargement) ne crée ni ne perd rien
- **Mécaniques :** fin de manche pendant qu'un objet est en main ou en vol ; sauvegarde et rechargement ; retour au salon.
- **Le bug d'origine :** dans Luanti (issue #4661), des objets rangés dans un coffre juste avant un redémarrage du serveur se retrouvaient à la fois dans le coffre et dans l'inventaire, sans doute par une sauvegarde du joueur et une de la carte désynchronisées ([source](https://github.com/luanti-org/luanti/issues/4661)). Chez nous, une sauvegarde de la tier-list dont le compteur d'identifiants était en retard sur ses tuiles aurait donné un identifiant déjà pris à la tuile suivante (`a_save_with_a_stale_counter_never_reuses_an_id`).
- **Propriété :** pour tout instant de la partie, sérialiser puis relire l'état donne un état égal ; pour toute transition qui survient pendant qu'un objet est en transit (tenu, lancé, en cours de pose), l'objet finit dans un seul conteneur ; tout compteur d'identifiants relu reste au-delà du plus grand identifiant utilisé.
- **Test :** à un pas au hasard, faire l'aller-retour de sérialisation et comparer ; forcer des fins de manche pendant les actions de déplacement ; vérifier ECO-01.
- **Adapter :** si ton jeu ne sauvegarde rien, garde la partie « transition ». Si l'hôte garde l'état sur disque pour survivre à un redémarrage (C7), c'est cette sauvegarde-là qu'on teste. Voisins : ECO-01, PROG-03, C9 du commun.

### ECO-05 · L'effet d'une action n'est jamais écrasé par une copie périmée de l'état
- **Mécaniques :** actions qui déclenchent une cascade (élimination qui vide la main, fin de manche, joueur exclu) ; code qui lit une valeur, appelle autre chose, puis réécrit la valeur lue.
- **Le bug d'origine :** dans Luanti (issue #6546), un objet qui tuait son utilisateur restait dans son emplacement : la mort vidait l'inventaire, puis le moteur réécrivait l'ancienne pile par-dessus ; on dupliquait ainsi l'objet ([source](https://github.com/luanti-org/luanti/issues/6546)).
- **Propriété :** pour toute action dont l'effet déclenche une cascade, le total des objets est conservé (ECO-01), et appliquer l'action d'un bloc donne le même état que passer par ses événements intermédiaires.
- **Test :** générer des actions qui éliminent ou font finir la manche (jouer la dernière carte, perdre la dernière vie) ; vérifier ECO-01 après chaque pas.
- **Adapter :** en Rust, le piège prend la forme d'un `let pile = state.pile.clone(); …; state.pile = pile;`. Voisins : ECO-02, EGA-02.

### ECO-06 · Faire puis défaire ne rapporte jamais rien
- **Mécaniques :** acheter puis revendre, poser puis reprendre, construire puis démolir, miser puis annuler.
- **Le bug d'origine :** dans Mindustry (issue #6518), la construction appliquait le multiplicateur de coût, mais pas le remboursement à la démolition : avec un multiplicateur de 0,1, chaque cycle créait des ressources ([source](https://github.com/Anuken/Mindustry/issues/6518)).
- **Propriété :** pour tout réglage du jeu (multiplicateurs, bonus) et toute suite d'actions suivie de leur défaite, les ressources de chaque joueur ne dépassent jamais celles d'avant ; un remboursement est toujours au plus le coût payé.
- **Test :** générer les réglages et des paires « faire, défaire » intercalées dans des parties ; comparer les ressources avant et après chaque paire.
- **Adapter :** liste tes paires faire/défaire, et tous les réglages qui changent un prix. Voisins : ECO-08, TOUR-02.

### ECO-07 · Une même chose ne rapporte qu'une fois
- **Mécaniques :** points pour une bonne réponse, prime pour un objectif, remboursement, récompense de fin de manche.
- **Le bug d'origine :** dans Mindustry (issue #6565), désigner un bloc à ramasser puis le supprimer avant l'arrivée de l'unité remboursait le bloc, et l'unité le ramassait quand même ([source](https://github.com/Anuken/Mindustry/issues/6565)). Dans OpenTTD (issue #2995), décharger une cargaison, la recharger ailleurs puis revenir la faisait payer une seconde fois ([source](https://github.com/OpenTTD/OpenTTD/issues/2995)).
- **Propriété :** pour toute suite d'actions, y compris deux actions concurrentes sur la même chose dans le même tick, dans les deux ordres, chaque chose payante (par identifiant) apparaît au plus une fois dans le journal des gains.
- **Test :** tenir dans le test un registre des gains par identifiant ; générer des paires d'actions concurrentes (« supprimer » et « réclamer », « répondre » et « fin du temps ») ; vérifier l'unicité.
- **Adapter :** remplace « chose payante » par tes unités de gain (réponse, duel, objectif, objet livré). Voisins : VOTE-03, EGA-02.

### ECO-08 · Aucune boucle de transferts entre joueurs ne crée de valeur
- **Mécaniques :** dons, échanges, paris, prêts, parts, tout transfert entre joueurs ou vers la banque.
- **Le bug d'origine :** dans OpenTTD (issue #7691), en multijoueur avec les actions activées, acheter 75 % d'une société fantôme, lui envoyer son argent, revendre ses parts au prix gonflé puis récupérer l'argent faisait passer de 5 à 8,75 millions par tour de boucle ; fermé sans correctif, l'option étant désactivée par défaut ([source](https://github.com/OpenTTD/OpenTTD/issues/7691)).
- **Propriété :** pour toute suite de transferts, achats et ventes, la somme de tous les comptes ne varie que par les sources et puits nommés de la règle, et la variation est exactement la somme de ces événements dans le journal.
- **Test :** tenir le journal des sources et des puits dans le test ; après chaque pas, comparer la masse totale à la masse initiale plus le journal.
- **Adapter :** chez Punchline, la seule source est le score des duels, et aucun score ne baisse (`players_keep_their_seat`). Écris la liste de tes sources et puits dans le `CERTIFICATION.md` du jeu. Voisins : ECO-06, ECO-07.

### ECO-09 · Aucun compteur ne déborde, ne revient à zéro ni ne change de signe
- **Mécaniques :** scores, argent, multiplicateurs, identifiants, compteurs de manches.
- **Le bug d'origine :** dans OpenTTD (issue #7665), un compteur d'infrastructure stocké en non signé mais calculé en signé faisait passer l'entretien de +32 654 à −32 654 par an : le jeu se mettait à créer de l'argent ([source](https://github.com/OpenTTD/OpenTTD/issues/7665)). Dans l'issue #7750, un don énorme débordait à la conversion de devise et envoyait 0 £ ([source](https://github.com/OpenTTD/OpenTTD/issues/7750)). Chez nous, une sauvegarde de la tier-list au bout de la plage d'identifiants faisait déborder l'identifiant suivant, ce qui panique avec `overflow-checks` (`a_save_at_the_end_of_the_id_range_refuses_new_ids_without_panicking`).
- **Propriété :** pour toutes entrées, y compris des montants absurdes envoyés par un téléphone, chaque compteur reste dans ses bornes ; le maximum théorique (joueurs × manches × gain maximal × bonus) tient dans le type ; une entrée hors bornes est refusée, jamais réduite en silence ; un coût reste positif.
- **Test :** calculer le maximum théorique dans un test et vérifier qu'il tient ; générer des montants aux extrêmes de leur type ; vérifier qu'aucune opération ne panique (le manifeste active `overflow-checks` même en release) et que les refus ne changent rien.
- **Adapter :** préfère `checked_add` avec un refus explicite à `saturating_add`, qui cache le problème. Voisins : PROG-01, AFF-01 (le nombre de chiffres affiché).

### ECO-10 · Partager ou arrondir ne crée ni ne détruit rien
- **Mécaniques :** points partagés entre gagnants, part des votes, intérêts, pourcentages, pot divisé.
- **Le bug d'origine :** dans OpenTTD (issue #2410), un prêt de 100 000 à 2 % rapportait 1 968 par an au lieu de 2 000, par arrondis répétés ; les correctifs proposés gardaient un écart de −8 ou +4 par an ([source](https://github.com/OpenTTD/OpenTTD/issues/2410)).
- **Propriété :** pour tout montant et toute répartition entre N parts, la somme des parts vaut exactement le total, et deux parts qui devraient être égales diffèrent d'au plus 1 (le reste va selon une règle écrite qui respecte EGA-01).
- **Test :** générer le total et N, et des poids (votes) ; vérifier la somme exacte et l'écart. Punchline calcule `1000 × multiplicateur × part des votes` en entiers : c'est ce genre de formule qu'on vérifie.
- **Adapter :** si ta règle accepte de perdre le reste (arrondi vers le bas pour chacun), écris-le, et la propriété devient « la perte est au plus N−1 ». Voisins : AFF-04 (pourcentages affichés), ECO-08.

### ECO-11 · Une commande répétée par un double appui n'est payée qu'une fois
- **Mécaniques :** achats, mises, pioche, toute action qui coûte quelque chose.
- **Le bug d'origine :** dans OpenTTD (issue #5544), recliquer sur une gare déjà construite refacturait tout son coût : un rebond de souris suffisait à vider la caisse ; fermé sans correction, car reconstruire est voulu pour certaines gares ([source](https://github.com/OpenTTD/OpenTTD/issues/5544)).
- **Propriété :** pour toute suite d'actions dans laquelle on duplique au hasard des commandes (même contenu, même phase), l'état final est identique à celui de la suite sans doublons, sauf pour les actions que la règle déclare cumulables.
- **Test :** générer une suite, puis la même avec des doublons insérés juste après les originaux ; comparer les états finaux.
- **Adapter :** un double tap sur un téléphone est le cas courant ; salon ne déduplique pas ce que le joueur envoie deux fois. Voisins : VOTE-02, TR-04.
