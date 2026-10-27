# Texte saisi et contenu des joueurs (`TXT`)

> Pour un jeu où les joueurs tapent quelque chose que le jeu **compare, transforme ou insère** : prénoms, réponses, mots à deviner, messages. Le commun couvre déjà l'affichage et la robustesse : limites de longueur appliquées par l'hôte et comptées de la même façon (T1), textes vicieux qui ne cassent rien et n'injectent rien (T2). Ce fichier porte sur la **logique** : quand deux textes sont-ils « le même » ? que devient un texte qu'on coupe, qu'on met en majuscules, qu'on filtre, qu'on insère dans une phrase ? Lis aussi `vote-quiz.md` et `affichage.md`.

L'idée commune : **une seule fonction de clé**, sur l'hôte, sans locale, idempotente, qui sert à toutes les comparaisons (doublons de prénoms, réponses identiques, bonne réponse d'un quiz). Ces items la testent.

### TXT-01 · Deux réponses identiques à l'écran ne sont jamais jugées différentes à cause de leur encodage Unicode
- **Mécaniques :** bonne réponse d'un quiz, réponses identiques (« jinx »), doublons de prénoms, mots à deviner.
- **Le bug d'origine :** dans Bazaar (bzr 0.92 sous macOS), le système de fichiers stockait les noms en forme décomposée (NFD) alors que l'utilisateur tapait en forme composée (NFC) : un même nom était « manquant » d'un côté et « inconnu » de l'autre ([source](https://bugs.launchpad.net/bugs/172383)). L'annexe Unicode UAX #15 fixe la règle : normaliser les deux chaînes vers la même forme avant de les comparer, et rappelle qu'aucune forme n'est stable par concaténation ([source](https://unicode.org/reports/tr15/)). Un clavier iOS et un clavier Android ne produisent pas toujours la même forme.
- **Propriété :** pour toute chaîne s, `même(s, nfd(s))` et `même(nfc(s), nfd(s))` ; pour toutes chaînes a et b, `même(a + b, nfc(a + b))`, ce qui couvre une réponse assemblée en plusieurs morceaux.
- **Test :** générer des chaînes sur `[a-zçéèêëïîôûü ]{0,20}`, produire leurs formes NFC et NFD avec la crate `unicode-normalization`, et comparer avec la fonction du jeu.
- **Adapter :** remplace `même` par la fonction de ton jeu (Punchline : la comparaison du jinx et des prénoms). Voisins : TXT-02, TXT-10.

### TXT-02 · La clé de comparaison appliquée deux fois donne le même résultat qu'une fois
- **Mécaniques :** toute normalisation de texte (casse, accents, espaces, ponctuation) utilisée pour comparer.
- **Le bug d'origine :** chez Spotify (2013), la fonction qui normalisait les noms d'utilisateur n'était pas idempotente : « ᴮᴵᴳᴮᴵᴿᴰ » donnait `BIGBIRD` au premier passage et `bigbird` au second, et un attaquant a pu réinitialiser le mot de passe d'un autre compte ([source](https://engineering.atspotify.com/2013/06/creative-usernames)).
- **Propriété :** pour toute chaîne x, `clé(clé(x)) == clé(x)` ; pour tous x, y, z, si `clé(x) == clé(y)` et `clé(y) == clé(z)`, alors `clé(x) == clé(z)` (c'est automatique si `même` compare des clés, et faux dès qu'on compare « à peu près »).
- **Test :** générer `any::<String>()` enrichi de caractères de compatibilité : petites capitales, exposants, ligatures (« ﬃ »), lettres pleine chasse.
- **Adapter :** si ton jeu tolère les fautes de frappe (distance d'édition), ce n'est plus une relation d'équivalence : ne t'en sers pas pour les doublons de prénoms. Voisins : TXT-01, TXT-03.

### TXT-03 · « Ces deux réponses sont la même » ne dépend jamais de la langue d'un appareil
- **Mécaniques :** comparaison insensible à la casse ou aux accents.
- **Le bug d'origine :** le « Turkey test » : en turc, la minuscule de « I » est « ı » sans point, donc `"FILE".ToLower()` ne correspond plus à « file » ; la correction recommandée est une comparaison indépendante de la culture ([source](https://www.moserware.com/2008/02/does-your-code-pass-turkey-test.html)). Dans Fuse.js (issue #723), une recherche ne trouvait pas les titres tchèques accentués ; une option pour ignorer les accents a été ajoutée en 2025 ([source](https://github.com/krisk/Fuse/issues/723)).
- **Propriété :** la clé est calculée sur l'hôte, sans paramètre de langue ; sur un jeu fixe de cas {I, ı, İ, i, ß, ẞ, Σ final}, elle donne une valeur unique et écrite dans le test ; si le jeu ignore les accents, `clé(sans_accents(x)) == clé(x)`, et la réponse affichée reste celle qui a été tapée.
- **Test :** un test de table pour les cas fixes, et une propriété sur des chaînes accentuées générées.
- **Adapter :** décide une fois si ton jeu ignore les accents (« éléphant » = « elephant ») et écris-le dans le `CERTIFICATION.md` du jeu. Voisins : TXT-02, TXT-04.

### TXT-04 · Mettre un texte en majuscules ne lui fait pas dépasser son budget
- **Mécaniques :** prénoms ou réponses affichés en capitales sur la TV, titres, comparaisons après `to_uppercase`.
- **Le bug d'origine :** dans MediaWiki (T147646), mettre la première lettre d'un titre en majuscule avec `toUpperCase` changeait « ß » en « SS » sous Chromium 54, alors que le serveur la laissait intacte : l'article « ß » devenait introuvable pour certains outils ([source](https://phabricator.wikimedia.org/T147646)). La documentation de Rust rappelle que `to_uppercase` peut produire plusieurs caractères pour un seul (« tschüß » donne « TSCHÜSS ») ([source](https://doc.rust-lang.org/std/primitive.str.html)).
- **Propriété :** pour tout texte accepté, la longueur de sa forme en majuscules respecte le budget d'affichage ; aucun indice ni aucune coupe calculés sur la forme transformée n'est appliqué à l'original.
- **Test :** générer des textes riches en « ß », « ŉ », « ǰ », ligatures « ﬁ » et « ﬀ », « İ », à la limite de longueur ; vérifier la longueur après transformation.
- **Adapter :** mesure le budget sur ce qui est vraiment affiché (Punchline borne les consignes une fois le prénom inséré : AFF-07). Voisins : TXT-05, T1 du commun.

### TXT-05 · Couper un texte ne coupe jamais au milieu d'un caractère ni d'un graphème
- **Mécaniques :** troncature d'un prénom ou d'une réponse trop longue, aperçu, « … ».
- **Le bug d'origine :** pour Yjs, un `.slice()` a séparé les deux moitiés d'un caractère hors du plan de base ; l'encodage a ensuite levé une erreur, et la synchronisation s'est arrêtée sans bruit en perdant des modifications ([source](https://george.mand.is/2026/05/my-favorite-bugs-invalid-surrogate-pairs/)). La documentation de Rust précise que couper à une frontière de caractère peut encore couper visuellement un graphème, comme un emoji composé ([source](https://doc.rust-lang.org/std/primitive.str.html)).
- **Propriété :** pour tout texte s et toute limite n, `t = couper(s, n)` est un préfixe de s, se termine sur une frontière de graphème, respecte la limite, et `couper(t, n) == t`.
- **Test :** générer des suites d'emoji à ZWJ, de drapeaux, de lettres suivies d'accents combinants ; tirer n sur toute la longueur ; vérifier avec `unicode-segmentation`.
- **Adapter :** T1 du commun dit **combien** on garde ; cet item dit **où** on coupe. Voisins : TXT-04, TXT-10.

### TXT-06 · Deux prénoms qui se ressemblent à l'œil ne coexistent pas
- **Mécaniques :** prénoms, noms d'équipe, tout ce qui sert à reconnaître quelqu'un sur la TV.
- **Le bug d'origine :** constat de spécification plutôt que bug de jeu : le rapport Unicode UTS #39 définit le « squelette » d'une chaîne pour détecter les confusables, avec l'exemple d'un « paypal » écrit avec un « а » cyrillique ([source](https://www.unicode.org/reports/tr39/)).
- **Propriété :** pour tout prénom accepté n et toute substitution d'une lettre par une lettre confusable (table embarquée : a/а, e/е, o/о, p/р, c/с, l/I/1, O/0), le second prénom est refusé comme doublon, ou la TV les distingue autrement (couleur, personnage).
- **Test :** générer un prénom, puis une variante par substitution ; vérifier le refus.
- **Adapter :** entre amis, on peut juger cet item excessif ; si tu l'écartes, dis pourquoi en une ligne (par exemple : chaque joueur a son personnage et sa couleur, qui le distinguent mieux que son prénom). Voisins : TXT-07, TXT-10.

### TXT-07 · Un caractère invisible ne permet jamais de prendre le nom d'un autre
- **Mécaniques :** prénoms, auteur d'un message ou d'une réponse.
- **Le bug d'origine :** dans OnionShare 2.3 à 2.3.3 (avis GHSA-68vr-8f46-vc9f), on pouvait se renommer « Alice » suivi d'une espace invisible et écrire dans le chat sous l'apparence d'Alice ; corrigé en 2.5 en retirant les caractères non visibles ([source](https://osv.dev/vulnerability/GHSA-68vr-8f46-vc9f)). La liste de développement d'OpenStreetMap rappelle qu'on ne peut pas bannir tous les caractères de largeur nulle : certaines écritures et les emoji composés ont besoin du ZWJ ([source](https://lists.openstreetmap.org/pipermail/rails-dev/2024-February/029813.html)).
- **Propriété :** pour tout prénom n et tout caractère c des catégories « ignorable par défaut » ou de contrôle bidirectionnel (U+200B, U+2060, U+202E, U+2066…), insérer c à n'importe quelle position donne un prénom refusé ou ramené à n ; un emoji à ZWJ légitime (👩‍👩‍👧) reste accepté tel quel.
- **Test :** générer n, c et la position ; vérifier le refus ou l'égalité de clé ; tester à part les emoji à ZWJ.
- **Adapter :** Punchline retire déjà les caractères de contrôle et les espaces doublés (`names_fit_and_differ`), mais U+200B et U+202E sont de catégorie Cf, pas Cc : vérifie que ton nettoyage les voit. Voisins : TXT-06, TXT-10.

### TXT-08 · Le filtre de mots ne censure jamais un mot innocent qui contient un mot interdit
- **Mécaniques :** filtre « mode famille », censure, mots interdits dans un jeu de mots.
- **Le bug d'origine :** le « problème de Scunthorpe » : AOL a bloqué les habitants de Scunthorpe en 1996, Xbox Live a suspendu en 2010 un joueur qui habitait Fort Gay (Virginie-Occidentale), et des filtres de remplacement ont produit « clbuttic » pour « classic » ([source](https://en.wikipedia.org/wiki/Scunthorpe_problem)).
- **Propriété :** aucun mot d'un corpus innocent embarqué n'est filtré (Scunthorpe, analyse, Essex, classique, assassin, et des équivalents français à choisir selon ta liste : « conférence », « Bitche » en Moselle…) ; un filtre de remplacement ne change jamais un mot absent de la liste ; filtrer deux fois revient à filtrer une fois.
- **Test :** générer un mot interdit entouré de préfixes et de suffixes tirés d'un dictionnaire ; vérifier que seule la correspondance sur un mot entier déclenche le filtre.
- **Adapter :** Jackbox a un filtre parce qu'il joue devant des streams ; entre amis, Punchline n'en a pas ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Leçons à trier »). Si ton jeu n'a pas de filtre, écarte l'item. Voisins : TXT-02.

### TXT-09 · Un texte de joueur inséré dans une phrase n'est jamais réinterprété comme un gabarit
- **Mécaniques :** consignes qui citent un prénom (« Qu'est-ce que {joueur} cache dans son frigo ? »), messages qui citent une réponse, annonces de l'animateur.
- **Le bug d'origine :** dans Army Men RTS 1.0 (Bugtraq, 2004), rejoindre un serveur avec le pseudo `%n%n%n` le faisait planter immédiatement ([source](https://lists.openwall.net/bugtraq/2004/11/14/2)). Dans Spigot (SPIGOT-839, Minecraft, 2015), un greffon passait le message brut comme format, et « I am 100% sure » levait une exception ([source](https://hub.spigotmc.org/jira/browse/SPIGOT-839)).
- **Propriété :** pour tout gabarit t et tous textes v₁…vₖ, `rendu(t, v)` contient chaque vᵢ tel quel à son emplacement, et rien d'autre n'est remplacé : la substitution se fait en un seul passage, donc un prénom qui contient `{réponse}` n'est pas remplacé par la réponse de quelqu'un.
- **Test :** générer des textes sur l'alphabet `{ } % s n d $ \` plus `{joueur}` et `{réponse}` ; rendre, puis retrouver chaque vᵢ dans le résultat à sa place.
- **Adapter :** T2 du commun vérifie que `{joueur}` ne casse pas l'affichage ; cet item vérifie la substitution dans le cœur. Voisins : AFF-07.

### TXT-10 · Le contrôle des doublons porte sur le nom final, noms réservés compris
- **Mécaniques :** prénoms des joueurs, des robots et de l'animateur.
- **Le bug d'origine :** chez nous, Punchline : ses tests de propriétés ont trouvé un joueur qui pouvait s'appeler comme l'animateur, des prénoms en double une fois coupés à la limite, et des doubles espaces invisibles qui distinguaient deux prénoms identiques à l'écran ; chacun a son test de régression ([`LEARNINGS.md` de Punchline](https://github.com/build-your-own-game/punchline/blob/main/LEARNINGS.md), « Bugs trouvés par la certification » ; invariant `names_fit_and_differ`).
- **Propriété :** pour toute suite d'arrivées avec des prénoms quelconques, les prénoms gardés sont nettoyés, coupés, puis comparés : deux joueurs n'ont jamais la même clé de prénom, et aucun joueur n'a la clé d'un nom réservé (animateur, robots, « TV »).
- **Test :** le générateur de prénoms de Punchline : textes vicieux, nom de l'animateur et d'un robot, variantes de casse, espaces autour, prénoms déjà à la limite (« Maximilienne » et « MAXIMILIENNE »).
- **Adapter :** l'ordre est la clé de l'item : nettoyer, couper, **puis** comparer. Voisins : TXT-01, TXT-05, TXT-07.
