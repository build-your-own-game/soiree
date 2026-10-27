# Affichage de contenu variable sur la TV (`AFF`)

> Pour tout jeu qui affiche des nombres, des classements, des minuteurs ou des textes qui changent. Le harnais commun regarde déjà la TV de l'extérieur : texte dans la zone sûre, à plusieurs formats, sans débordement visible (T3), et textes vicieux (T2). Mais il ne voit que les états qu'il atteint par hasard. Ce fichier vérifie, **dans le cœur**, ce qu'il ne peut pas voir : la pire valeur que le jeu peut produire, et la cohérence entre ce qui est calculé et ce qui est montré. Lis aussi `egalites.md` (rangs ex æquo) et `texte.md`.

L'idée commune : **les vues sont des données**. Le cœur produit `tv_view()` et `phone_view()` ; ce qu'elles contiennent se teste comme le reste, sans navigateur.

### AFF-01 · Aucun nombre affiché n'a plus de chiffres que l'écran n'en prévoit
- **Mécaniques :** scores, argent, compteurs, multiplicateurs, temps.
- **Le bug d'origine :** dans Touhou 4 (PC-98), le score est stocké sur 8 chiffres décimaux ; à 100 millions, le chiffre de tête dépasse 9 et s'affiche comme une lettre ([source](https://rec98.nmlgc.net/blog/2024-12-04)). Dans Donkey Kong, le temps du niveau 22 ne tient plus sur un octet et retombe à presque rien ([source](https://heise.de/-10275685)).
- **Propriété :** le score maximal atteignable, calculé à partir des règles (joueurs × manches × gain maximal × bonus), s'écrit avec au plus le nombre de chiffres que la vue prévoit ; dans toute partie générée, chaque nombre de la vue TV respecte sa largeur.
- **Test :** un test qui calcule le maximum théorique et le formate ; une propriété dans le test des sessions qui vérifie chaque nombre de `tv_view()` après chaque pas.
- **Adapter :** pour chaque nombre de tes vues, écris sa largeur maximale à côté de la constante qui le borne. Voisins : ECO-09, AFF-06.

### AFF-02 · Le temps restant affiché ne devient jamais négatif, ne remonte jamais, et vaut 0 au changement de phase
- **Mécaniques :** comptes à rebours sur la TV et sur les téléphones.
- **Le bug d'origine :** dans KDE Knights (bug 330398), à l'expiration, l'horloge affichait 00:00 puis 59:59 : des ticks arrivaient sous zéro pendant l'arrêt ; corrigé en 2016 en figeant l'affichage à 00:00 en fin de partie ([source](https://bugs.kde.org/show_bug.cgi?id=330398)). Chez nous, Punchline : une vue renvoyée après une reconnexion allongeait le compte à rebours ; une vue porte maintenant le temps restant et l'heure de l'hôte à laquelle elle a été faite, et l'écran retire le retard qu'il mesure ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Les comptes à rebours »).
- **Propriété :** pour toute suite d'événements (ticks, ticks en retard ou en double, pause, reprise, tick après la fin de phase), le temps restant de chaque vue est dans [0, durée de la phase], ne croît pas au sein d'une phase (sauf pause et reprise), et vaut 0 quand la phase change.
- **Test :** dans le test des sessions, relever le temps restant de `tv_view(now)` et de chaque `phone_view(now)` après chaque pas, avec des `now` qui reculent ou se répètent ; vérifier les bornes et la monotonie par phase.
- **Adapter :** si tes vues portent une échéance plutôt qu'un temps restant, la propriété porte sur l'échéance : constante au sein d'une phase, sauf pause. Voisins : TR-10, PROG-05.

### AFF-03 · Le rang est calculé sur la même valeur que celle qui est affichée
- **Mécaniques :** classement, podium, scores en flottants ou à décimales cachées.
- **Le bug d'origine :** sur CodinGame (The Great Dispatch), deux joueurs aux scores identiques à 20 décimales échangeaient leurs places ; l'équipe a reconnu que le classement tenait compte des erreurs de précision ([source](https://forum.codingame.com/t/the-great-dispatch-leaderboard/16767)).
- **Propriété :** pour tout vecteur de scores, si deux scores s'affichent pareil, ils ont le même rang (EGA-06) ; recalculer le classement sans changement de score donne le même résultat.
- **Test :** générer des scores qui diffèrent sous la précision affichée ; comparer rangs et valeurs affichées dans `tv_view()`.
- **Adapter :** le plus simple : des scores entiers, comme dans Punchline et Sidequest. Voisins : EGA-05, EGA-06.

### AFF-04 · Les pourcentages de votes affichés font 100 et ne trompent pas
- **Mécaniques :** répartition des votes, barres de sondage, « 60 % ont choisi… ».
- **Le bug d'origine :** dans les sondages de Discourse, l'arrondi arrondissait tout vers le bas, puis ajoutait 1 aux premiers éléments sans regarder les décimales : 69 votes sur 106 (65,09 %) s'affichaient 66 % ([source](https://meta.discourse.org/t/rounding-of-poll-percentages-is-imprecise/52319)).
- **Propriété :** pour tout vecteur de votes, la somme des pourcentages affichés vaut 100 s'il y a au moins un vote (sinon tous valent 0) ; chaque pourcentage est à moins d'un point de la valeur exacte ; deux options aux votes égaux ont le même pourcentage, sauf l'écart d'un point inévitable, attribué par une règle écrite ; le résultat ne dépend pas de l'ordre des options.
- **Test :** générer des vecteurs de 2 à 8 options avec beaucoup d'égalités ; vérifier les quatre conditions. La méthode du plus fort reste les tient.
- **Adapter :** si ton jeu affiche des nombres de votes plutôt que des pourcentages, écarte l'item. Voisins : ECO-10.

### AFF-05 · Un message qui contient un nombre est toujours accordé
- **Mécaniques :** « 1 point », « 0 joueur a voté », « plus que 2 secondes », annonces de l'animateur.
- **Le bug d'origine :** dans gtali (GNOME, bug 106697), « %s wins the game with %d points » était un format fixe ; le correctif de 2003 passe par `ngettext`, et les traducteurs rappelaient que le polonais choisit la forme selon le dernier chiffre ([source](https://bugzilla.gnome.org/show_bug.cgi?id=106697)).
- **Propriété :** pour chaque message qui contient un nombre, et pour n ∈ {0, 1, 2, 11, 21, 100, maximum}, le texte produit suit la règle de pluriel de la langue du jeu (en français : singulier pour 0 et 1).
- **Test :** un test de table sur la fonction de pluriel ; une propriété sur les vues qui cherche les motifs « 1 points », « 0 joueurs »… dans le texte sérialisé.
- **Adapter :** si les textes sont construits dans le crate web, sors la fonction de pluriel dans `core` pour la tester. Voisins : AFF-06.

### AFF-06 · Chaque écran de la TV a son pire cas, construit à partir des limites
- **Mécaniques :** tout écran TV qui affiche du contenu variable.
- **Le bug d'origine :** chez nous, Punchline : dans une fenêtre plus étroite que 16:9, le texte des bulles sortait de son cadre, et rien ne garantissait qu'une réponse de 80 caractères, ou trois en finale, tienne à l'écran : la mise en page n'avait été vérifiée qu'en 1600 × 900. Le jeu a maintenant une galerie des pires cas (`punch_core::gallery` : textes les plus longs en lettres larges, mot de 80 lettres, table pleine, égalité à huit), et un test Rust oblige à ajouter un pire cas pour tout nouvel écran ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Mise en page déterministe de la TV »).
- **Propriété :** pour chaque variante de la vue TV (chaque phase, chaque sorte d'écran), la galerie contient au moins un état où chaque champ variable est à sa limite : textes les plus longs en caractères larges, nombre maximal de joueurs, plus grands nombres (AFF-01), égalité générale.
- **Test :** un test qui énumère les variantes de `TvView` (un `match` exhaustif, pour que le compilateur force l'ajout d'un cas) et vérifie que la galerie couvre chacune. Le harnais commun (`layout`) ou un script comme `tools/fit-check.mjs` ouvre ensuite la galerie dans Chrome.
- **Adapter :** construis tes pires cas à partir des constantes du protocole, pas à la main : si une limite change, le pire cas suit. Voisins : AFF-07, T3 du commun.

### AFF-07 · Le contenu du jeu respecte les limites de la TV, une fois les prénoms insérés
- **Mécaniques :** fichiers de consignes, de questions, de mots ; gabarits qui insèrent un prénom ou une réponse.
- **Le bug d'origine :** chez nous, Punchline : les consignes doivent faire au plus 100 caractères une fois le prénom inséré (`MAX_PROMPT_CHARS`), et un test vérifie que les fichiers de `content/` respectent ces limites ; l'invariant `prompts_fit_the_tv` le vérifie aussi après chaque pas d'une partie ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Mise en page déterministe de la TV », point 4).
- **Propriété :** pour chaque élément du contenu et chaque prénom de longueur maximale (en caractères larges), le texte rendu respecte la limite de son emplacement ; pour toute partie générée, chaque texte de la vue TV la respecte aussi.
- **Test :** un test sur tout le contenu avec le prénom le plus long ; l'invariant dans le test des sessions.
- **Adapter :** le contenu généré ou ajouté plus tard (par les amis, par une IA) passe par le même test. Voisins : TXT-09, TXT-04, AFF-06.
