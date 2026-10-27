# Hasard et équité (`HAS`)

> Pour tout jeu qui tire quelque chose au sort : ordre des questions, distribution des cartes, rôles secrets, premier joueur, duels, objets qui apparaissent. Le commun exige déjà la même partie pour la même graine et les mêmes entrées (D1). Ce fichier vérifie que le hasard est **juste** (chaque issue a sa chance), **frais** (pas la même partie à chaque soirée), **rangé** (seules les actions de jeu le consomment) et **équitable** sur une soirée. Lis aussi `vote-quiz.md` (distribution des consignes).

Un test statistique est déterministe ici : la graine est fixée, donc un test du χ² ne clignote pas. Choisis le nombre de tirages et le seuil une fois, et garde-les.

### HAS-01 · Un mélange donne chaque ordre avec la même probabilité
- **Mécaniques :** mélanger un paquet de questions, de cartes, l'ordre de passage, la place des réponses à l'écran.
- **Le bug d'origine :** Jeff Atwood montre qu'échanger chaque carte avec une position prise dans tout le paquet donne 27 chemins pour 6 ordres de trois cartes, et que certains ordres sortent bien plus souvent ([source](https://blog.codinghorror.com/the-danger-of-naivete/)). En 2010, l'écran de choix du navigateur de Microsoft mélangeait avec un tri à comparateur aléatoire ; selon le navigateur, Internet Explorer finissait bien plus souvent à certaines places ([source](https://www.robweir.com/blog/2010/02/microsoft-random-browser-ballot.html)).
- **Propriété :** pour n = 3 et n = 4, sur K mélanges avec K graines fixées, la fréquence de chacun des n! ordres passe un test du χ² à un seuil strict.
- **Test :**
  ```rust
  #[test]
  fn every_order_is_equally_likely() {
      for n in 3..=4_usize {
          let mut counts = BTreeMap::new();
          for seed in 0..24_000_u64 {
              let mut deck: Vec<usize> = (0..n).collect();
              Rng::new(seed).shuffle(&mut deck);
              *counts.entry(deck).or_insert(0_u32) += 1;
          }
          assert_eq!(counts.len(), (1..=n).product::<usize>());
          assert!(chi_square(&counts) < CRITICAL[n], "{counts:?}");
      }
  }
  ```
- **Adapter :** remplace `Rng::shuffle` par ton mélange. Un mélange ne passe jamais par un tri avec un comparateur aléatoire (`sort_by(|_, _| random)`). Voisins : HAS-02, HAS-03.

### HAS-02 · Chaque élément peut finir à chaque place, y compris la dernière et la sienne
- **Mécaniques :** tout mélange de Fisher-Yates écrit à la main.
- **Le bug d'origine :** dans le poker en ligne d'ASF Software (1999), le générateur renvoyait 0 à n−1 mais le code supposait 1 à n : la dernière carte ne revenait jamais à la dernière place ; le même code échangeait aussi avec tout le paquet, le biais de HAS-01 ([source](https://www.datamation.com/applications/how-to-cheat-at-online-poker-a-study-in-software-security/)).
- **Propriété :** pour n de 2 à 8, sur K graines, la matrice « élément j à la place i » n'a aucune case vide, et chaque case vaut environ K/n (χ²).
- **Test :** compter les positions de chaque élément sur K mélanges ; vérifier la matrice. Ce test voit aussi la variante de Sattolo, qui ne laisse jamais un élément à sa place.
- **Adapter :** utilise le mélange de ta crate de hasard plutôt que d'en écrire un, et garde le test quand même. Voisins : HAS-01.

### HAS-03 · Un tirage borné ne favorise jamais les petites valeurs
- **Mécaniques :** « une question parmi 37 », « un joueur parmi 7 », tout `rng() % n`.
- **Le bug d'origine :** Melissa O'Neill (PCG) montre qu'avec `rand() % 52` sur 32 bits, les 48 premières cartes ont une chance de plus que les 4 dernières, et qu'au-delà de 2³¹ certaines issues deviennent deux fois plus fréquentes ; elle recommande le rejet ou la méthode de Lemire ([source](https://www.pcg-random.org/posts/bounded-rands.html)).
- **Propriété :** avec une source qui énumère tout un domaine de 8 bits (0 à 255), `tirer_borné(n)` donne des comptes strictement égaux pour chaque n de 2 à 255.
- **Test :** injecter une source de test qui renvoie 0, 1, 2… 255 ; appeler le tirage borné pour chaque valeur et compter. Un `% n` échoue tout de suite pour n = 3.
- **Adapter :** si ta fonction de tirage n'accepte pas de source injectée, teste-la statistiquement (χ²) sur n = 3, 5, 7. Voisins : HAS-01.

### HAS-04 · Deux parties de la soirée ne reçoivent jamais la même graine par défaut
- **Mécaniques :** création de la graine au lancement de l'hôte ou à chaque nouvelle partie.
- **Le bug d'origine :** sur Pavlovia, plus de 300 participants d'une expérience ont tous reçu la même liste et le même ordre : un composant avait une graine fixe à 1, et tout le hasard tiré ensuite suivait la même suite ([source](https://discourse.psychopy.org/t/math-random-using-the-same-random-seed-and-so-is-shuffle/16956)).
- **Propriété :** sur un échantillon d'entropies distinctes, la fonction « entropie → graine » ne donne jamais deux fois la même graine ; sur K graines, le premier joueur désigné et la première question sont uniformes (χ²) ; aucune graine littérale n'est dans le chemin de création d'une partie, et une nouvelle partie tire une graine neuve (sauf rejeu volontaire).
- **Test :** une propriété sur la fonction de graine ; un test statistique sur le premier tirage ; un test qui lance deux nouvelles parties de suite et vérifie que leurs premières questions diffèrent pour au moins une graine sur deux, ou que la graine a changé.
- **Adapter :** la graine vient de l'hôte (Punchline la tire de `/dev/urandom`) ; le cœur la reçoit en paramètre, comme le veut D1. Note la graine dans le journal pour rejouer une partie. Voisins : HAS-05, PROG-03.

### HAS-05 · L'espace des graines n'est jamais réduit au point de rendre la partie devinable
- **Mécaniques :** graine tirée d'une horloge, tronquée en 32 bits, dérivée d'un code de salle court.
- **Le bug d'origine :** dans le poker d'ASF Software, la graine valait les millisecondes depuis minuit : environ 86 millions de paquets possibles au lieu de 52!, et des chercheurs prédisaient tout le paquet en direct ([source](https://www.datamation.com/applications/how-to-cheat-at-online-poker-a-study-in-software-security/)).
- **Propriété :** pour toute paire de graines u64 qui ne diffèrent que par leurs 32 bits de poids fort, les parties générées diffèrent ; la simulation ne lit aucune horloge pour son hasard.
- **Test :** générer des paires de graines (a, a ^ (b << 32)) ; comparer le premier tirage de chaque partie (paquet mélangé, rôles).
- **Adapter :** entre amis, deviner le paquet n'est pas une menace sérieuse ; une graine tronquée l'est davantage, car elle fait revenir les mêmes parties. Voisins : HAS-04.

### HAS-06 · Seules les actions de jeu acceptées consomment du hasard
- **Mécaniques :** actions refusées, consultations (aide, aperçu, demande de vue), reconnexions, robots, animations qui tirent au sort.
- **Le bug d'origine :** dans le mode multijoueur de RimWorld (issue #967, 2026), construire une boîte de confirmation tirait un nombre au sort sur la machine du seul joueur qui cliquait ; la suite des tirages divergeait d'une machine à l'autre ; corrigé en isolant ce générateur (PR #968) ([source](https://github.com/rwmt/Multiplayer/issues/967)). Chez nous, un « Je sèche » refusé changeait le tirage de Punchline ; la propriété `refused_input_changes_nothing` compare maintenant tout l'état, générateur compris, à celui d'un simple passage du temps ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Bugs trouvés par la certification »).
- **Propriété :** pour toute partie et toute insertion de messages refusés ou consultatifs (vue demandée, reconnexion, appui hors phase), l'état final et l'état du générateur sont identiques à ceux de la partie sans ces messages.
- **Test :** entrelacer deux flux générés : les actions de jeu, et des messages refusés ou consultatifs ; comparer l'état final (générateur compris) avec celui du seul flux de jeu.
- **Adapter :** si l'affichage veut du hasard (confettis, petites variations), il prend un générateur à lui, hors de l'état de jeu. Voisins : E5 du commun, D1 du commun.

### HAS-07 · Une question ne revient jamais avant que le paquet soit épuisé, ni deux fois de suite entre deux paquets
- **Mécaniques :** paquets de consignes, de questions, de mots, de pièces ; parties enchaînées dans la soirée.
- **Le bug d'origine :** dans Drawphone (issue #142, 2023), deux des six mots proposés étaient identiques ([source](https://github.com/tannerkrewson/drawphone/issues/142)). Dans un clone de Codenames (jbowens/codenames, issue #112), des mots revenaient d'une partie à l'autre ; d'après le mainteneur, seulement au remélange du paquet ([source](https://github.com/jbowens/codenames/issues/112)). Tetris le règle par un « sac » de sept pièces, avec un défaut connu : deux sacs qui se touchent peuvent répéter une pièce ([source](https://tetris.wiki/Random_Generator)).
- **Propriété :** sur N tirages dans un paquet de taille P, chaque élément sort au plus ⌈N/P⌉ fois et les comptes diffèrent d'au plus 1 ; le dernier élément d'un paquet n'est jamais le premier du suivant (si P ≥ 2) ; les parties enchaînées sans redémarrer continuent le même paquet au lieu d'en remélanger un neuf.
- **Test :** générer P, N et la graine ; tirer N éléments, à travers plusieurs « rejouer » ; vérifier les comptes et les frontières.
- **Adapter :** un paquet qui contient deux fois la même question (doublon dans le fichier de contenu) casse l'item : ajoute un test qui vérifie que ton contenu n'a pas de doublons. Voisins : HAS-08, VOTE-07.

### HAS-08 · Mêler des éléments imposés et des éléments tirés ne crée jamais de doublon
- **Mécaniques :** question finale imposée plus questions tirées, tutoriel plus tirage, consigne personnalisée plus paquet.
- **Le bug d'origine :** dans le module Quiz de Drupal (issue #236663, 2008), avec deux questions toujours posées et trois tirées parmi six, un joueur a reçu une question imposée en double ; la cause n'a pas été établie ([source](https://www.drupal.org/node/236663)).
- **Propriété :** pour tous ensembles imposé et aléatoire qui se recouvrent de façon quelconque, la liste de la manche n'a aucun doublon d'identifiant et a la taille annoncée, ou une erreur explicite dit que le paquet est trop petit.
- **Test :** générer les deux ensembles avec des identifiants communs ; construire la manche ; vérifier.
- **Adapter :** chez Punchline, les consignes de finale et de manche viennent de deux fichiers ; les consignes où l'on insère un prénom sont un autre cas de recouvrement (la même consigne avec deux prénoms). Voisins : HAS-07.

### HAS-09 · Sur une partie, chacun est désigné à peu près autant que les autres
- **Mécaniques :** premier joueur, juge, dessinateur, rôle secret, adversaire de duel.
- **Le bug d'origine :** pas de bug de jeu vérifié pour cet item : c'est un principe. Dota 2 utilise une distribution pseudo-aléatoire dont la probabilité grandit à chaque échec, pour réduire les séries et borner les longues absences ([source](https://diplograph.net/notes/games/randomness/dota-2-prd)). Le clone de Quiplash de VOTE-07 montre l'effet d'une distribution inégale à l'échelle d'une manche.
- **Propriété :** sur R manches avec N joueurs, chacun est désigné ⌊R/N⌋ ou ⌈R/N⌉ fois ; pour les duels, le nombre de rencontres de chaque paire diffère d'au plus 1 ; l'écart entre deux désignations d'un même joueur est au plus 2N−1.
- **Test :** générer N, R et la graine ; jouer les attributions (sans jouer les manches) ; compter.
- **Adapter :** un sac de joueurs (on tire sans remise, puis on remplit le sac) donne ces garanties. Si ton jeu veut un pur hasard, écris-le et écarte l'item. Voisins : VOTE-07, EGA-01.
