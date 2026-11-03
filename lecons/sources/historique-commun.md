# Leçons de l'historique du commun

## La source

L'historique git complet de `soiree`, jusqu'à l'étiquette locale `avant-refonte` (503b386) : 258 commits, du 26 octobre au 2 novembre 2026. Étiquettes publiées à ce moment : `v0.1.0` (d868b76), `v0.2.0` (b226c97), `v0.3.0` (0acc4e3). La `v0.4.0` est décrite dans `CHANGELOG.md` mais n'est pas étiquetée.

Cette fiche garde les défauts du modèle, du catalogue, de la documentation, du processus de version et de l'épreuve elle-même. Les défauts du code de salon, de la télécommande et de l'outil ont leurs propres sources : ils ne sont cités ici que lorsqu'un même défaut touche aussi le reste.

## Ce que j'ai lu

- Le message complet des 258 commits.
- Le diff de chaque commit qui corrige un défaut hors de `salon/`, `remote/`, `outil/` : documents (`AGENTS.md`, `CERTIFICATION.md`, `VERIFICATION.md`, `CHANGELOG.md`, `README.md`, `REJOINDRE.md`, `REPETITION.md`, `MANIFESTE.md`), `modele/`, `certifications/`, `catalogue.toml`, `tools/` (harnais et répétition de montée), `.github/`.
- Pour les commits mixtes, la part hors de salon, remote et outil, et les parties d'outil qui touchent la publication (`outil/src/upgrade.rs`, `outil/src/docs.rs`, `outil/dossier/AGENTS.md`).
- L'état d'`avant-refonte` des fichiers cités, pour les numéros de ligne.

## Ce que je n'ai pas lu

- Le diff du code de `salon/`, `remote/` et `outil/` au-delà de ce qui précède : couverts par d'autres sources.
- Les fiches de `recherche/*.toml`, sauf les lignes qu'un commit corrige.
- Le contenu des 97 items de `certifications/*.md` : seules leurs corrections de forme sont ici.
- Les campagnes de `certification/` (e73554f, fff8061, b22d204…), ni les diffs des enquêtes sur les minuteries macOS (`[skip ci]`), au-delà de leurs messages.
- Les gros commits de fondation (d868b76, 7ff2bd3, b226c97, 0acc4e3, 73891de) : seulement leur message, leur liste de fichiers et les parties citées.
- Les dépôts des jeux : les bugs de jeu cités ici viennent des messages de commit du commun.

---

## L'épreuve elle-même

### HIS-01 · Rien qui n'a pas tourné ne compte comme réussi
- **Ce qui arrive :** un contrôle sans exécution sort en succès. Une crate dont tous les tests sont `#[ignore]` donnait `cargo:suite = passed` avec zéro test exécuté. `recherche verifie` sortait 0 avec « 0 contrôle(s) » sur un dossier vide ou inexistant. Un test `should_panic` mal lu faisait diverger le compte. Le code Swift de la télécommande n'était exécuté par aucune suite, donc chaque trou passait toutes les suites. Un test de salon s'arrêtait à son propre envoi, refusé par une borne, et ne prouvait rien.
- **D'où on le sait :** 632eff6, 35015c9, be86064, 32c1964 (« It fails, rather than skips, where it cannot run »), 5807e66 ; `CHANGELOG.md:87`.
- **Mécaniques :** toutes.
- **Question :** pour chaque contrôle annoncé, combien de cas ont réellement tourné, et le rapport le dit-il ?
- **Pression :** `soiree check` refuse une crate à tests dont aucun ne tourne ; un banc qui ne peut pas tourner échoue au lieu de sauter.
- **Sous la nouvelle forme :** pression du commun. Le contrat le dit (« Un contrôle qui n'a pas tourné ne compte pas comme réussi ») ; le commun l'applique à chaque rapport.
- **Étage :** commun.

### HIS-02 · Une déclaration qui n'observe rien ne prouve rien
- **Ce qui arrive :** le jeu déclare un contrôle qui ne regarde pas le jeu, et l'épreuve le croit. `["true"]` validait U1, T2 et D2, car seul le code de sortie était lu. Une clé `storage` ne changeait rien sur un jeu à téléphones, et C9 passait sans toucher la sauvegarde. Un trou de recherche se fermait sur n'importe quel fichier, pas sur un test.
- **D'où on le sait :** 1289ad1, a26c55c, 011c8c5 ; `CHANGELOG.md:61`, `CHANGELOG.md:191`.
- **Mécaniques :** toutes.
- **Question :** ce contrôle échoue-t-il quand le jeu est faux ? Que lit-il du jeu ?
- **Pression :** `check` et le harnais refusent une commande sans `{url}` ni `success` ; le harnais refuse `storage` sur un jeu à téléphones.
- **Sous la nouvelle forme :** pression du commun. Le jeu ne déclare plus de commandes : il fournit invariants et bots en code. Mais un invariant toujours vrai reste possible : l'injection de bugs (étape 5 de l'épreuve) est la pression qui le voit.
- **Étage :** commun.

### HIS-03 · Un test échoue quand on retire ce qu'il garde
- **Ce qui arrive :** une protection disparaît, tout reste vert. `NOTE_CRITICAL` pouvait être retiré de la minuterie. `PAST_SESSIONS` pouvait passer de 8 à 1, 7 ou 9. Une campagne de mutants a laissé 141 mutants sur 433 vivants dans salon-core. Le chaos restait vert sans débit ni tampon, alors qu'un gros message n'arrivait jamais. Une mesure « retirer ce correctif fait rougir le chaos » datait d'avant un changement et ne valait plus.
- **D'où on le sait :** 7bb4466, 89361d7, 085d61e, 283a015, 445f562, 01e148f, dcd78b4, 1863d74, 0dfc504, 46e0ef7, e4b275f, 08d278e, 42dd17d.
- **Mécaniques :** toutes.
- **Question :** pour chaque garantie, quelle faute injectée fait échouer son test, et quand l'a-t-on mesuré pour la dernière fois ?
- **Pression :** mutations nocturnes sans seuil (42dd17d) ; tests ajoutés mutant par mutant.
- **Sous la nouvelle forme :** pression du commun. Étape 5 de l'épreuve, et « une faute injectée doit le faire échouer ».
- **Étage :** commun.

### HIS-04 · Rien ne devient sans objet en silence
- **Ce qui arrive :** un oubli retire des exigences. Enlever `socket` de `cert.json` rendait E1 à E4 et C7 non applicables ; `"phone": null` en retirait dix autres. Le résumé additionnait les contrôles sans objet et les vérifiés. Sept exigences restaient sans liaison pour toujours, faute de moyen de les relier. Un niveau inconnu de fiche sautait quotas et saturation. À l'inverse, `layout: false` se croyait une exclusion et ne retirait rien.
- **D'où on le sait :** e24004f, 92516af, 5d7a6ab, 5d8d40e, 6764852 ; `CHANGELOG.md:128`, `CERTIFICATION.md:102`, `certifications/README.md:41`.
- **Mécaniques :** toutes.
- **Question :** chaque exigence écartée l'est-elle par une déclaration explicite, avec sa raison ?
- **Pression :** `phone` exige `socket` ; un jeu sans téléphone exige `display.role` avec sa raison ; un item du catalogue sans décision reste « non revu ».
- **Sous la nouvelle forme :** pression du commun. « Une applicabilité inconnue n'est jamais une exclusion » ; une question sans réponse bloque l'épreuve.
- **Étage :** commun.

### HIS-05 · Le résumé ne promet que ce qui est vrai, et distingue chaque situation
- **Ce qui arrive :** les mots du résumé disent plus que le passage. Un passage avec des jugements humains affichait « ✓ Aucun problème trouvé », les mots mêmes que `README.md` et `REJOINDRE.md` donnent comme fin du travail. Un passage échoué affichait aussi la phrase d'un passage réussi. Un point à juger et un point non résolu se confondaient. Le rapport disait « satisfait » pour une liaison dont l'hypothèse ne vivait que dans le JSON.
- **D'où on le sait :** 23b6cf6, 6d74558, 5e35d59, c741483, 6da0b44 ; `CHANGELOG.md:127`.
- **Mécaniques :** toutes.
- **Question :** un lecteur pressé, humain ou agent, peut-il prendre ce résumé pour une réussite complète alors qu'il ne l'est pas ?
- **Pression :** décision de passage dans un noyau testé sur ses 486 cas (dca71b3), puis prouvé (6c4e2eb) ; contrôles négatifs sur les deux issues.
- **Sous la nouvelle forme :** pression du commun. Le contrat distingue sans objet, pas exécuté, échec du jeu et panne de l'outil ; le vocabulaire du résumé doit avoir une seule définition.
- **Étage :** commun.

### HIS-06 · Une panne de l'outil ou de la machine n'est jamais un échec du jeu
- **Ce qui arrive :** l'outil échoue et accuse le jeu. Sous Windows, le harnais échouait chaque scénario et les rapportait comme 23 points à corriger dans le jeu. Sans Node, l'erreur était un « No such file » nu sous `cert.json`. Un rejet non traité dans une route du relais terminait tout le passage, 22 points jamais atteints. Un portail captif répondait 2xx et passait pour GitHub ; une réponse interceptée faisait lire chaque document comme changé. Le pare-feu muet ressemblait à un jeu qui ne répond pas.
- **D'où on le sait :** 5e9bf2b, a4c97dc, e5ff52e, b342690, 35015c9, 23b6cf6.
- **Mécaniques :** toutes.
- **Question :** devant chaque échec, l'outil sait-il dire si c'est le jeu, la machine, le réseau ou lui-même ?
- **Pression :** garde de plateforme avant `npm ci` ; code de sortie propre à une panne réseau ; tests des deux bouchons de `recherche verifie`.
- **Sous la nouvelle forme :** pression du commun. Le rapport distingue « une panne de l'outil » d'« un échec du jeu ».
- **Étage :** commun.

### HIS-07 · Un échec tombe sur le bon item, avec ce qu'il a vu
- **Ce qui arrive :** l'échec est attribué ailleurs, ou sans indice. `jitter.ready` était lue puis ignorée : R1 n'était jamais observé et l'échec tombait sur E1. Une navigation abandonnée à son échéance continuait, et son 500 tardif était imputé à la navigation suivante. Un échec de C1 n'apparaissait qu'avec le délai nu de Playwright, et ne revenait pas en neuf rejeux. Des notes écrites hors de leur scénario disparaissaient du verdict.
- **D'où on le sait :** c10ea3d, 79a6cc0, 1f30437, 457c45c.
- **Mécaniques :** toutes.
- **Question :** l'échec dit-il quel contrôle, quelle valeur observée, depuis combien de temps ?
- **Pression :** une navigation ne possède que les requêtes qu'elle a vues partir ; un délai porte la dernière valeur lue et le nombre d'images.
- **Sous la nouvelle forme :** pression du commun. Avec un jeu déterministe, un échec rare se rejoue : il doit au moins laisser sa graine et son histoire.
- **Étage :** commun.

### HIS-08 · Un test de temps réel juge le code, pas la machine
- **Ce qui arrive :** une machine chargée fait échouer des tests justes. Une compilation à côté du harnais produisait de faux échecs, sans un mot. Un test de navigation donnait 250 ms au navigateur, qu'un runner lent dépensait avant d'analyser le document. Un test d'images affirmait 100 ms entre deux images ; Chrome en arrière-plan en prend 200. Des fixtures natives se disputaient le runner.
- **D'où on le sait :** 79a6cc0, f9049f2, 671ee7f, 8a20dbf ; `tools/cert/load.mjs:14`.
- **Mécaniques :** temps réel, toutes celles qui mesurent.
- **Question :** ce test affirme-t-il une causalité du code, ou une vitesse de la machine ?
- **Pression :** charge du processeur mesurée avant et après, avertissement à la moitié des cœurs ; un essai dont l'échéance précède sa cause ne conclut rien et double l'échéance.
- **Sous la nouvelle forme :** pression du commun. Le temps devient une entrée : la logique se teste à vitesse machine, et seuls les budgets mesurent le temps réel.
- **Étage :** commun.

### HIS-09 · Un budget se juge à la résolution de sa mesure
- **Ce qui arrive :** le budget de 33 ms échouait sur l'image même qu'il nomme. Une vsync sautée à 60 Hz dure 33,33 ms, et Chrome la rapporte à 33,3 ou 33,4 ms. La cadence sur un échantillon court a aussi sa résolution, d'une demi-image par seconde.
- **D'où on le sait :** 96173cf, 5127d14, d35786f ; `CERTIFICATION.md:118`, `tools/cert/performance.test.mjs:18`.
- **Mécaniques :** affichage, temps réel.
- **Question :** quelle est la résolution de cette mesure, et le seuil en tient-il compte sans devenir une marge ?
- **Pression :** test qui fait passer une vsync sautée et échouer une vraie image lente.
- **Sous la nouvelle forme :** pression du commun. Les budgets d'image restent mesurés par le commun.
- **Étage :** commun.

### HIS-10 · La mémoire d'une soirée se juge à sa pente
- **Ce qui arrive :** R2 comparait la mémoire à la minute 5 et à la fin, en rapport. Punchline échouait à 3 → 5 Mio : la moitié de quelques Mio est du bruit d'allocateur.
- **D'où on le sait :** 6197cc8 ; `tools/cert/endurance.mjs:20`.
- **Mécaniques :** toutes.
- **Question :** que projette la croissance sur six heures, et le nombre de threads croît-il ?
- **Pression :** échec au-delà de 256 Mio projetés sur six heures, ou quand les threads croissent ; la série entière est imprimée.
- **Sous la nouvelle forme :** pression du commun. « Mémoire bornée : mesure ».
- **Étage :** commun.

### HIS-11 · Une endurance se mesure par ce qui arrive, pas par sa durée
- **Ce qui arrive :** ce qu'une salle garde dépend des événements, pas du temps passé. Des fuites mises exprès dans salon n'ont été vues qu'une fois la soirée simulée faite de rechargements, de gros messages et d'un lien gardé vivant tout du long.
- **D'où on le sait :** 503b386.
- **Mécaniques :** toutes.
- **Question :** la soirée simulée contient-elle chaque événement qui peut retenir de la mémoire ?
- **Pression :** tours de soirée sur le réseau simulé, octets vivants comptés exactement ; dix minutes à chaque `cargo test`, une heure avec `SOIREE_EVENING_MINUTES=60`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-12 · Chaque cas d'épreuve part d'une mémoire neuve
- **Ce qui arrive :** deux négociations refusées laissaient leurs échecs dans `sessionStorage`. Le cas suivant partait directement sur WebSocket et ne trouvait aucun pair à fermer. La page avait raison ; le test avait hérité d'un état.
- **D'où on le sait :** af3199a ; `tools/cert/rtc-negotiation.mjs:65`.
- **Mécaniques :** toutes.
- **Question :** ce cas dépend-il de ce qu'un cas précédent a laissé dans le navigateur ou le disque ?
- **Pression :** chaque cas oublie l'onglet ; un cas à part vérifie ce que fait un échec mémorisé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-13 · L'épreuve ne suppose pas la forme d'un jeu
- **Ce qui arrive :** le harnais présumait ce qu'est un jeu. Il ouvrait 8 téléphones quelle que soit la salle, et `silent` en voulait 5 : une salle plus petite ne se certifiait pas. Le scénario `rtc` attendait que la valeur affichée revienne à sa base, ce que fait un saut et jamais un score : le jeu de départ échouait C1 à chaque fois. `start` était pressé même quand `join` n'était pas du passage.
- **D'où on le sait :** a26c55c, 0849044, d35786f ; `tools/cert/capacity.mjs:2`.
- **Mécaniques :** toutes.
- **Question :** quelle hypothèse sur le jeu ce scénario fait-il, et le jeu l'a-t-il déclarée ?
- **Pression :** `ordinaryPhones` borné par `players.max`, avec son test ; un effet finit en revenant ou en se posant.
- **Sous la nouvelle forme :** pression du commun. Le commun lit désormais l'état et fait jouer les bots du jeu, au lieu de deviner par le DOM ; reste à typer ce que le jeu déclare (taille de salle, fin ou non).
- **Étage :** commun.

### HIS-14 · Une panne simulée a les symptômes de la vraie
- **Ce qui arrive :** la panne jouée en test était plus gentille que le réel. Un Wi-Fi mort ne livre jamais la fermeture du socket, alors qu'une page qui ne réagissait qu'à cette fermeture passait. Le réseau simulé connaissait pertes et gigue, pas le débit ni le tampon. Un flux simulé avait le tampon du tuyau, ce qu'aucun WebSocket n'a. Le faux réseau ne gardait pas une fermeture derrière les données envoyées avant elle, comme le fait TCP.
- **D'où on le sait :** 5392a03, 0dfc504, 1e39346, 658b388, 1becb46.
- **Mécaniques :** toutes.
- **Question :** cette panne simulée se comporte-t-elle comme celle d'un vrai Wi-Fi, d'un vrai TCP, d'un vrai téléphone ?
- **Pression :** scénario `silent` sans fermeture ; réseau simulé avec débit et tampon ; relais TCP réel qui joue `slow_close` et `reset_peer`.
- **Sous la nouvelle forme :** pression du commun. « Nos pressions doivent ressembler à ce monde-là. »
- **Étage :** commun.

### HIS-15 · Un téléphone verrouillé ne fait plus tourner aucun code, minuteurs compris
- **Ce qui arrive :** pour simuler un iPhone verrouillé, `Page.setWebLifecycleState` laissait tourner les minuteurs d'une page visible. Le gel n'était pas un gel. Seule la pause du débogueur arrête tout le JavaScript.
- **D'où on le sait :** 5392a03 ; `tools/cert/browser.mjs:437`.
- **Mécaniques :** toutes celles qui ont des téléphones.
- **Question :** le gel simulé arrête-t-il aussi les minuteurs et les boucles d'animation ?
- **Pression :** scénario `freeze`, par `Debugger.pause`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-16 · Une vue tout juste dessinée peut encore être invisible
- **Ce qui arrive :** une vue qui vient d'apparaître peut être dans la première image de son animation, à l'opacité 0. WebKit tombait sur cette image à chaque fois, et le contrôle T2 de Punchline s'arrêtait à son premier cas.
- **D'où on le sait :** 280ac99 ; `tools/cert/text.mjs:129`.
- **Mécaniques :** texte, affichage.
- **Question :** l'oracle lit-il un élément au repos, après ses animations finies ?
- **Pression :** l'oracle attend les animations finies du texte et de ses ancêtres ; ce qui finit caché échoue toujours.
- **Sous la nouvelle forme :** connaissance. Le commun lira l'état plutôt que les pixels, mais un contrôle visuel garde ce piège.
- **Étage :** connaissance.

### HIS-17 · Le navigateur écrit ses propres erreurs quand le réseau tombe
- **Ce qui arrive :** quand le harnais coupe le réseau, le navigateur écrit lui-même deux `console.error` : `Failed to load resource…` et `WebSocket connection to … failed`. Compter toute erreur de console comme un défaut du jeu fait échouer chaque coupure ; n'en compter aucune laisse passer les vraies.
- **D'où on le sait :** 5a39e48 ; `tools/cert/browser.mjs:228`.
- **Mécaniques :** toutes.
- **Question :** cette erreur vient-elle du jeu, ou du navigateur qui constate la panne qu'on lui inflige ?
- **Pression :** E6 échoue sur tout `console.error` ou `console.assert`, hors ces deux messages.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-18 · Ce qu'aucun banc ne reproduit a son geste sur un vrai appareil
- **Ce qui arrive :** le simulateur iOS n'applique pas le refus du réseau local ; un vrai dégel, un Wi-Fi qui s'endort, un écran à 120 Hz, une page tuée faute de mémoire non plus. Le changelog renvoyait T9 à `REPETITION.md`, qui ne le mentionnait pas. La fiche citait le port d'un seul jeu.
- **D'où on le sait :** 1604db1, 74693d3, 073a7b0, d9b7917, 0e7054d.
- **Mécaniques :** toutes celles qui ont des téléphones.
- **Question :** pour chaque trou que le banc ne peut pas jouer, quel geste sur quel appareil le tranche, et qu'est-ce qu'on note ?
- **Pression :** `REPETITION.md`, lu par `soiree check .` contre le code ; jugement humain.
- **Sous la nouvelle forme :** pression du commun. Le contrat garde les gestes sur de vrais appareils comme jugements humains, avec leur fiche.
- **Étage :** commun.

---

## La documentation

### HIS-19 · Un document ne cite que ce qui existe, à cette version
- **Ce qui arrive :** un agent suit un document et tombe sur du vide. Le catalogue renvoyait au `CERTIFICATION.md` d'un jeu, devenu une vue générée de `verification.toml`. `catalogue.toml` disait que `soiree play` le lit ; cette commande n'existait pas. D5 nommait un contrôle inexistant. Les notes de montée renvoyaient à `modele/tools/*.mjs`, qu'un jeu n'a pas. `remote/AGENTS.md` citait `ios/Generated`, absent d'un clone neuf. La fiche des liens de recherche pointait vers des commits locaux. Un exemple portait la date d'une version future ; des exemples figeaient `v0.1.0`.
- **D'où on le sait :** c9ea75c, 6764852, c7b507f, 7c0ce7a, 09ca63c, 3913388, 0f7e155, 965b3bb, d9b7917 ; `outil/src/docs.rs:41`, `AGENTS.md:86`.
- **Mécaniques :** aucune (documentation).
- **Question :** chaque chemin, commande, option, version, scénario, clé et item cité existe-t-il dans cette version ?
- **Pression :** `soiree check .` (d44d0e7) refuse une version qui n'est pas la sienne, une commande ou option absente, un chemin inexistant, une liste de scénarios différente du harnais, une clé de `cert.json` non documentée, un item qui cite un scénario qui ne le rapporte pas. Les exemples écrivent `vX.Y.Z`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-20 · Une promesse écrite est vraie du code
- **Ce qui arrive :** un document promet ce que le code ne fait pas, et chaque agent qui le lit écrit du code faux. `remote/CONTRAT.md` promettait des télécommandes natives que salon ne relayait pas, et de l'UDP que salon ne livre pas. Il offrait micro et caméra sur une page en `http` locale, où `navigator.mediaDevices` n'existe pas. Un commit affirmait que `soiree check` refuse tout autre workflow : il ne comparait que `soiree.yml`. Les notes disaient que `upgrade` s'arrête sur un `soiree.yml` modifié ; il ne lisait que la première ligne. `LEARNINGS.md` marquait une leçon tenue alors qu'elle ne l'était pas. `Host::forget` disait que le jeton cesse de marcher, et la documentation promettait que rien ne croît sans borne : faux tous les deux.
- **D'où on le sait :** 6764852, e6d3a0a, 367201d, f166420 corrigé par c7b507f, 60d2739, b7bae9f, 0eae554, 7b8ea01, cc5c0a7, d35786f ; `AGENTS.md:72` (« Une phrase fausse produit du mauvais code chez tous les agents »), `remote/CONTRAT.md:92`.
- **Mécaniques :** aucune (documentation).
- **Question :** chaque phrase qui décrit un comportement a-t-elle un test qui échouerait si elle devenait fausse ?
- **Pression :** `soiree check .` pour les faits ; pour le sens, un agent neuf relit la documentation contre le code avant chaque étiquette (`AGENTS.md:139`). Aucune machine ne vérifie le sens.
- **Sous la nouvelle forme :** pression du commun. Une intention s'écrit pour être relue ; une garantie annoncée a son test.
- **Étage :** commun.

### HIS-21 · Un chiffre écrit vient d'une mesure datée
- **Ce qui arrive :** les chiffres des documents vieillissent ou sont faux dès le départ. Le changelog disait 1 636 parts par paquet au lieu de 1 635. Une fiche disait 4 500 cas de chaos sans déclenchement, la garde en avait vu 9 000 depuis. « Retirer ce correctif rougit en moins de 3 000 cas » datait d'avant un changement qui l'a rendu faux. Les durées d'installation n'avaient ni machine ni date.
- **D'où on le sait :** e6d3a0a, f2100bd, 08d278e, 9741625, 589bc9d, 0519842, 8f4c16e ; `README.md:17`.
- **Mécaniques :** aucune (documentation).
- **Question :** d'où vient ce chiffre, sur quelle machine, à quelle date, et quel test le tient ?
- **Pression :** tests qui figent les chiffres de salon à leur valeur (0519842) ; durées données avec machine et date.
- **Sous la nouvelle forme :** pression du commun. Le contrat dit que ses ordres de grandeur « restent à mesurer et à sourcer ».
- **Étage :** commun.

### HIS-22 · Un fait n'a qu'une source
- **Ce qui arrive :** le même fait écrit à plusieurs endroits diverge. Les règles du manifeste étaient des blocs de prose que `check.sh` analysait, et leurs copies divergeaient du code. Les chiffres de la CI et la porte de publication vivaient dans le manifeste et dans le README du modèle. Les mots du résumé (« Rien à corriger », « Aucun problème trouvé », « Aucun échec, N point(s) à juger ») ont été réécrits dans `modele/AGENTS.md`, `README.md`, `REJOINDRE.md` et `VERIFICATION.md` à chaque changement.
- **D'où on le sait :** 75f0e2e, a6da2a7, ff045b4, 6764852 (« Each rule now has one source »), 2601e06, 23b6cf6, 92516af, 6d74558, c741483 ; `AGENTS.md:72`.
- **Mécaniques :** aucune (documentation).
- **Question :** ce fait est-il écrit ailleurs ? Si oui, lequel fait foi, et les autres y renvoient-ils ?
- **Pression :** table des rôles des documents dans `AGENTS.md` ; règles déplacées dans `outil/regles/`, embarquées dans l'outil.
- **Sous la nouvelle forme :** pression du commun. Entre deux fichiers de code, la forme l'impose (« une seule définition, que le compilateur fait respecter ») ; entre documents, seul `soiree check .` et la relecture le tiennent.
- **Étage :** commun.

### HIS-23 · Un exemple de la documentation passe les contrôles
- **Ce qui arrive :** l'exemple de `cert.json` de `CERTIFICATION.md` ne passait ni `check` ni `certify`. Un jeu qui le copiait partait de travers. Le modèle n'avait aucun exemple de progression finie, alors que les notes de montée en demandaient une.
- **D'où on le sait :** 5a39e48, c7b507f, 1289ad1.
- **Mécaniques :** progression, toutes.
- **Question :** chaque exemple qu'un agent copiera tourne-t-il dans un test, contre un jeu réel ?
- **Pression :** `tools/cert/progression-example.test.mjs` fait tourner l'exemple contre un jeu qui finit et contre un jeu qui ne fait que changer d'écran.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-24 · Ce qu'un outil tiers fait se constate avant d'être contourné
- **Ce qui arrive :** un commit a ajouté une aide pour que `PROPTEST_CASES` élargisse le chaos, croyant qu'un champ `cases` littéral masquait la variable. C'était faux : la macro `proptest!` passe sa configuration par `contextualize_config`. Le commit suivant l'a retiré, et son message dit que le précédent avait tort. Même famille ailleurs : libtest écrit `test name - should panic ... ok`, que le compteur ne reconnaissait pas ; str0m retire 88 octets de son MTU ; `URLSession.shared` attend une minute.
- **D'où on le sait :** 8dda2ee, e717966, be86064, f8230d6, 97cc51f.
- **Mécaniques :** aucune (outillage).
- **Question :** a-t-on compté ce que fait vraiment la dépendance, sur un cas réel, avant d'écrire autour ?
- **Pression :** test qui compte les cas que la vraie propriété joue (e717966).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### HIS-25 · Les notes de version se suivent pas à pas par un agent qui ne connaît pas le dépôt
- **Ce qui arrive :** les étapes de « Passer à cette version » n'étaient pas exécutables. Elles renvoyaient à des fichiers absents d'un jeu, taisaient `players.max` devenu obligatoire, les commandes qui n'observent rien désormais refusées, le scénario `layout` à éprouver. Deux sections de versions non publiées coexistaient.
- **D'où on le sait :** c7b507f, 60d2739, 5ceb375 (« The steps to upgrade assume an agent that does not know this repository »), ff593aa, 048b16a.
- **Mécaniques :** aucune (processus).
- **Question :** un agent qui n'a jamais vu ce dépôt arrive-t-il au vert en suivant seulement ces étapes ?
- **Pression :** `tools/rehearse-upgrade.py` joue `soiree upgrade` puis `soiree check --locked --published` sur un clone de chaque jeu ; un agent neuf relit.
- **Sous la nouvelle forme :** pression du commun. Le contrat demande « les étapes exactes pour s'adapter ».
- **Étage :** commun.

### HIS-26 · Ce qui fait trébucher un ami qui commence à froid
- **Ce qui arrive :** un essai à froid sur un Mac neuf a buté sur ce que la documentation taisait. rustup n'installe pas d'éditeur de liens. Le premier `soiree check` compile tout pendant plusieurs minutes presque sans rien afficher. `gh auth login` ouvre le navigateur et doit être lancé par la personne. Git refuse de committer sans identité. Créer le dépôt demande l'invitation de l'hôte dans l'organisation. macOS demande d'accepter les connexions entrantes, sinon les téléphones n'atteignent pas le jeu. Un agent seulement dans le cloud ne peut pas montrer le jeu. Windows n'était pas essayé. Un mot de passe ne se colle jamais dans une conversation.
- **D'où on le sait :** 8f4c16e, 2601e06, efa6b15 ; `README.md:17`, `README.md:19`.
- **Mécaniques :** aucune (démarrage).
- **Question :** le parcours a-t-il été refait à froid, sur une machine neuve, et ce qui a bloqué est-il écrit ?
- **Pression :** `soiree doctor` vérifie l'éditeur de liens ; mesure datée d'un essai à froid.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

---

## Le processus de version

### HIS-27 · Les étapes de publication sont dans l'ordre où elles peuvent tourner
- **Ce qui arrive :** la liste d'avant l'étiquette demandait `soiree check` et `soiree certify` sur `modele/`, qui ne compile pas brut (`{{version}}`). Elle installait le binaire avec `--locked` avant de mettre `outil/Cargo.toml` et son lock à la version, ce que `--locked` refuse. Elle répétait la montée sur un commit qui n'existait pas encore.
- **D'où on le sait :** d21f8e4, 23075f0 ; `AGENTS.md:113-141`.
- **Mécaniques :** aucune (processus).
- **Question :** chaque étape peut-elle tourner avec ce que les précédentes ont produit, et seulement cela ?
- **Pression :** aucune machine ; l'ordre a été corrigé après coup.
- **Sous la nouvelle forme :** pression du commun. Une suite d'étapes vérifiables, que les agents suivent bien.
- **Étage :** commun.

### HIS-28 · Sous un `[patch]` local, Cargo écrit un lock qui échoue partout ailleurs
- **Ce qui arrive :** sous le `[patch]` du dossier de l'hôte, Cargo retire les `source =` des crates communes dans `Cargo.lock`. Ce lock, committé, échoue chez un ami. Puis `cargo update --package salon-core` refusait ce lock, une fois les manifestes déjà réécrits.
- **D'où on le sait :** dc1e6f5, 673b36d ; `outil/dossier/AGENTS.md:55`, `outil/src/upgrade.rs:61`, `CHANGELOG.md:130`.
- **Mécaniques :** aucune (processus).
- **Question :** ce lock a-t-il été écrit sous un `[patch]` ? La montée tourne-t-elle hors du dossier patché ?
- **Pression :** `public_upgrade_gives_back_the_sources_a_patched_lock_lost`, dans un dossier qu'aucun `[patch]` ne couvre ; `upgrade` lance `cargo update --workspace`.
- **Sous la nouvelle forme :** connaissance. Tant que l'hôte garde un `[patch]` local, ce comportement de Cargo reste.
- **Étage :** connaissance.

### HIS-29 · Seules les étiquettes annotées partent avec `--follow-tags`
- **Ce qui arrive :** `git push --follow-tags` ne pousse que les étiquettes annotées. Une étiquette légère reste locale, et chaque jeu qui la nomme ne compile plus chez un ami.
- **D'où on le sait :** 3913388 ; `AGENTS.md:141`.
- **Mécaniques :** aucune (processus).
- **Question :** l'étiquette est-elle annotée, et visible sur GitHub avant qu'un jeu la nomme ?
- **Pression :** aucune ; la consigne dit `git tag -a`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### HIS-30 · La montée de version s'éprouve avant l'étiquette, contre un faux amont
- **Ce qui arrive :** avant l'étiquette, rien ne montrait ce qu'un jeu d'ami vivrait après : chaque jeu reste sur l'étiquette publiée, `soiree upgrade` refuse et `soiree check` s'arrête à la ligne de version. Sous le `[patch]`, tout compilait contre le salon local, sans rien prouver du publié.
- **D'où on le sait :** 31d57fb, d21f8e4, 23075f0 ; `tools/rehearse-upgrade.py:15` (après l'étiquette, le chemin rapide de Cargo pour GitHub demande le commit publié).
- **Mécaniques :** aucune (processus).
- **Question :** la montée a-t-elle tourné sur un clone de chaque jeu, hors du dossier, contre l'étiquette au commit courant ?
- **Pression :** `tools/rehearse-upgrade.py` et son test `rehearse-upgrade.test.py` ; verdict `soiree check --locked --published`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-31 · La montée couvre tout ce qu'un jeu tient du commun, ou dit ce qu'elle laisse
- **Ce qui arrive :** `soiree upgrade` ne touchait pas `fuzz/`, espace de travail à part, qui restait sur l'ancienne étiquette. Il ne lisait que la première ligne de `soiree.yml`, et écrasait une modification faite sous l'en-tête. `soiree check` ne lisait pas les autres workflows.
- **D'où on le sait :** 673b36d, 60d2739 ; `CHANGELOG.md:19`.
- **Mécaniques :** aucune (processus).
- **Question :** quels fichiers d'un jeu nomment le commun, et la montée les atteint-elle tous ?
- **Pression :** aucune pour `fuzz/` ; la note le dit.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-32 · Une version non publiée n'a qu'une section de notes
- **Ce qui arrive :** v0.3.1 et v0.4.0 ont été décrites dans `CHANGELOG.md`, sans qu'aucune soit publiée. Un jeu sur v0.3.0 aurait dû suivre deux sections, dont une n'existe pour personne. Les notes ont été fondues en une seule section. Inversement, tant qu'aucune étiquette ne l'a publié, un format peut encore changer sans rupture (protocole 2).
- **D'où on le sait :** 73891de, 5ceb375, cc9ec13.
- **Mécaniques :** aucune (processus).
- **Question :** les notes partent-elles de la dernière étiquette publiée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### HIS-33 · L'outil embarque ses modèles : changer un modèle demande de réinstaller
- **Ce qui arrive :** les modèles du dossier de l'hôte sont compilés dans l'outil. Changer `outil/dossier/` puis lancer `soiree host-setup --force` réécrivait l'ancienne version tant que l'outil n'était pas réinstallé.
- **D'où on le sait :** 3913388.
- **Mécaniques :** aucune (processus).
- **Question :** ce que je change est-il lu au lancement, ou figé à la compilation de l'outil ?
- **Pression :** aucune ; la consigne le dit.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### HIS-34 · La CI d'un jeu ne part pas à chaque commit
- **Ce qui arrive :** le workflow du modèle tournait sur macOS à chaque push. Les dépôts privés de l'organisation partagent environ 190 minutes macOS par mois, et un passage en prenait 6 à 13. Le modèle disait aussi de pousser souvent, sans dire qu'un push ne lançait plus la CI.
- **D'où on le sait :** f166420, 744fc4a, 2bf7a6b, ff045b4 ; `CHANGELOG.md:129`, `modele/AGENTS.md:73`.
- **Mécaniques :** aucune (processus).
- **Question :** ce contrôle doit-il accompagner chaque changement, ou tourner à part, et qui paie ses minutes ?
- **Pression :** `soiree check` compare `soiree.yml` au fichier généré ; mutations et fuzz la nuit, jamais sur un push (42dd17d).
- **Sous la nouvelle forme :** connaissance. Le contrat garde le principe : rapide à chaque changement, long en continu.
- **Étage :** connaissance.

### HIS-35 · Rien de propre à une machine n'entre dans un dépôt
- **Ce qui arrive :** `check.json` et `check-fast.json` changeaient à chaque passage, entraient dans le premier commit d'un ami et portaient des chemins locaux, sans que rien ne relise les anciens. 250 fichiers de campagnes, lus par aucun outil, portaient des chemins de la machine dans un dépôt public.
- **D'où on le sait :** 09738ee, 2f01c3c, e73554f ; `modele/.gitignore:6-11`.
- **Mécaniques :** aucune (processus).
- **Question :** ce fichier est-il relu par un outil, et contient-il un chemin, un nom ou une donnée de cette machine ?
- **Pression :** `.gitignore` du modèle ; résultats générés ignorés.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-36 · Une enquête close laisse sa régression, pas ses sondes
- **Ce qui arrive :** l'enquête sur les minuteries macOS a laissé des sondes et deux workflows qui tournaient à chaque changement de l'hôte sans rien garder. Elle a fini par une minuterie kqueue critique ; seul le test de régression avait de la valeur.
- **D'où on le sait :** fb4a8d1, a002ab2, b22d204, 37d23b4, e7fb615, b4e09b4.
- **Mécaniques :** aucune (processus).
- **Question :** une fois l'enquête close, quel test garde sa conclusion, et que peut-on retirer ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### HIS-37 · Retirer une preuve est une décision humaine
- **Ce qui arrive :** le noyau Verus de la décision de passage a été remplacé par un test exhaustif de ses 486 cas, « simplification » qui retirait vingt paquets à installer. Il a été restauré le jour même : la preuve formelle fait partie de ce que le projet éprouve.
- **D'où on le sait :** dca71b3, 6c4e2eb (« Removing it was a mistake, not a simplification »).
- **Mécaniques :** aucune (processus).
- **Question :** ce que je retire fait-il partie de l'expérience elle-même ? Qui l'a décidé ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance. Le contrat le confirme : le commun permet dès maintenant d'exiger une preuve.
- **Étage :** connaissance.

### HIS-38 · La dette de recherche bloque à l'étiquette, ligne par ligne
- **Ce qui arrive :** la dette pouvait rester sans raison, ou être effacée par un niveau écrit à la main (`niveau = 2`, `raison_niveau = "x"`). Les sections de version ne disaient pas ce qui restait ouvert, ni pourquoi.
- **D'où on le sait :** 6d2ca24, 589bc9d, 2b9e0aa ; `AGENTS.md:138`.
- **Mécaniques :** aucune (processus).
- **Question :** chaque trou ouvert de niveau 3 a-t-il sa raison dans la section de cette version ?
- **Pression :** `soiree recherche dette`, à la seule étape de l'étiquette ; les trous comptent au plancher du registre, pas au niveau déclaré.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-39 · L'environnement se découvre, il ne se suppose pas
- **Ce qui arrive :** chaque machine différait de celle de l'auteur. Le build iPhone cherchait cargo seulement dans `~/.cargo/bin`, et les runners macOS de GitHub installent rustup par Homebrew. `ps -M` liste les threads sur macOS, et veut dire autre chose sur Linux. Ouvrir `/dev/urandom` échouait au démarrage sous Windows. Un variant mort sur Windows cassait le build avec `-D warnings`. Le harnais voulait Node 20 alors que `hostile` a besoin du `WebSocket` global de Node 22. `check` exigeait Node alors que seul le harnais en a besoin.
- **D'où on le sait :** 3f891a6, a26c55c, 1b9fdda, a71be6d, 82de704, a8a1e6a, efa6b15, debb481.
- **Mécaniques :** aucune (outillage).
- **Question :** cette commande, ce chemin, cette option existent-ils sur chaque système visé ?
- **Pression :** `soiree doctor` ; garde de plateforme du harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

---

## Le modèle et le catalogue

### HIS-40 · Le modèle s'éprouve tel qu'un ami le reçoit
- **Ce qui arrive :** `modele/` brut ne compile pas (`{{version}}`), et la consigne demandait pourtant de le vérifier tel quel. Ses manifestes avaient `{{nom}}` dans le nom des paquets, et Cargo imprimait de fausses erreurs dans chaque jeu. `VERIFICATION.md` décrivait un mode du modèle qui n'avait jamais servi.
- **D'où on le sait :** d21f8e4, 23b6cf6, 8c8172f ; `AGENTS.md:134`.
- **Mécaniques :** toutes.
- **Question :** le modèle passe-t-il l'épreuve une fois créé par `soiree new`, exactement comme chez un ami ?
- **Pression :** `.github/scripts/model-ci.py` fait un jeu par `soiree new`, puis `soiree check`, le refus de `--published` et `soiree certify -- --only ui,panic`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-41 · Le modèle montre chaque obligation qu'un jeu aura
- **Ce qui arrive :** le modèle est un compteur continu, sans fin : rien ne montrait une progression finie, et les notes renvoyaient à un exemple absent. Il ne déclarait pas `success` pour son propre contrôle de texte. Il ne reliait pas E5 et D1, qu'il tenait pourtant déjà.
- **D'où on le sait :** c7b507f, 1289ad1, dea171f, 73891de.
- **Mécaniques :** progression, texte, toutes.
- **Question :** pour chaque obligation qu'un jeu rencontrera, le modèle en donne-t-il un exemple qui tourne ?
- **Pression :** exemple de progression finie testé contre un jeu qui finit (`tools/cert/progression-example.test.mjs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### HIS-42 · Le catalogue suit le format que les jeux écrivent
- **Ce qui arrive :** le catalogue disait d'écrire l'origine d'un item dans le `CERTIFICATION.md` du jeu, devenu une vue générée, sous un champ `origine` ; les jeux écrivent `origins` dans `verification.toml`. Son exemple était en Markdown, alors que le format est en TOML. Un item écarté se justifiait dans une section de ce même fichier généré.
- **D'où on le sait :** c9ea75c, 6764852 ; `certifications/README.md:41`, `certifications/README.md:67`.
- **Mécaniques :** toutes celles du catalogue.
- **Question :** l'exemple du catalogue se lit-il avec le format qu'un jeu écrit aujourd'hui ?
- **Pression :** aucune sur le catalogue lui-même.
- **Sous la nouvelle forme :** pression du commun. Le contrat donne à une leçon « la même forme à tous les étages » : un seul type, lu par l'épreuve, rendrait ce décalage impossible ; tant que le catalogue reste du texte, seule la relecture le tient.
- **Étage :** commun.

### HIS-43 · Une raison dit quelque chose
- **Ce qui arrive :** une raison vide de sens passait. `"x"` justifiait un item écarté ou un niveau de recherche. Douze références lues valaient `["x"]`, sans aucune leçon.
- **D'où on le sait :** 1289ad1, 2b9e0aa, 011c8c5.
- **Mécaniques :** toutes.
- **Question :** un autre lecteur comprend-il pourquoi, et peut-il retrouver ce qui est cité ?
- **Pression :** au moins deux mots et 12 caractères ; une référence doit être retrouvable. Les plus courtes raisons réelles des trois jeux passent (« Pas de saut. »).
- **Sous la nouvelle forme :** pression du commun. La garantie d'examen exige une réponse avec sa raison ; la machine ne juge que sa forme, un humain son sens.
- **Étage :** commun.

---

## Ce que l'épreuve a trouvé dans les jeux

### HIS-44 · Un refus ne touche pas au hasard
- **Ce qui arrive :** dans Punchline, une réponse de réserve refusée faisait quand même avancer le générateur aléatoire. La partie suivante divergeait.
- **D'où on le sait :** d0906bd.
- **Mécaniques :** hasard, vote et quiz.
- **Question :** une entrée refusée laisse-t-elle tout l'état identique, graine comprise ?
- **Pression :** propriété « un événement refusé ne change rien » (7a6857e, Sidequest).
- **Sous la nouvelle forme :** impossible par la forme. « Un refus ne change rien : le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte l'entrée. »
- **Étage :** commun.

### HIS-45 · Une valeur venue de l'extérieur n'est jamais NaN ni infinie
- **Ce qui arrive :** Sidequest acceptait un bail d'entrée NaN, et affichait l'infini pour un énorme aller-retour. Ailleurs, une inclinaison de -128 se lisait -1,008, et une disposition hostile mettait le bouton d'un stick à NaN.
- **D'où on le sait :** 7a6857e, dffe8af, df675cb.
- **Mécaniques :** physique, temps réel, affichage.
- **Question :** chaque nombre qui entre est-il borné et fini dès son entrée ?
- **Pression :** tests ignorés qui épinglaient ces deux bugs (7a6857e) ; fuzz.
- **Sous la nouvelle forme :** impossible par la forme, à condition que le type d'entrée exclue NaN et l'infini. « Ce qui vient de l'extérieur est validé à l'entrée : des types. »
- **Étage :** commun.

### HIS-46 · La TV rogne ses bords
- **Ce qui arrive :** l'horloge, la plaque de manche et les boutons de l'organisateur de Punchline étaient dans la bande que beaucoup de télés coupent.
- **D'où on le sait :** 5392a03 ; `modele/web/style.css:2` (90 % intérieurs de l'écran).
- **Mécaniques :** affichage.
- **Question :** tout ce qui compte tient-il dans la zone sûre, à chaque format d'écran ?
- **Pression :** scénario `layout` (T3), sur six formats de 1280×720 à 3840×2160.
- **Sous la nouvelle forme :** pression du commun. Le commun possède l'affichage et peut vérifier la zone sûre de toute vue.
- **Étage :** commun.

### HIS-47 · Un réaffichage n'efface pas ce qu'on est en train de taper
- **Ce qui arrive :** dans Punchline, chaque réaffichage effaçait le prénom en cours de saisie.
- **D'où on le sait :** 5392a03.
- **Mécaniques :** texte.
- **Question :** un champ en cours d'édition garde-t-il sa valeur, sa sélection et sa composition à travers un réaffichage ?
- **Pression :** aucune nommée dans l'historique.
- **Sous la nouvelle forme :** pression du commun. Le contrat de la page téléphone : « Un élément en cours d'édition garde son identité, sa sélection et sa composition tant que son objet existe. »
- **Étage :** commun.

### HIS-48 · Deux noms se comparent comme un humain les voit
- **Ce qui arrive :** Punchline laissait un joueur prendre le nom du présentateur, deux noms trop longs identiques une fois coupés, et des doubles espaces invisibles.
- **D'où on le sait :** d0906bd.
- **Mécaniques :** texte.
- **Question :** deux noms qu'un joueur ne distingue pas sur la TV sont-ils refusés comme doublons ? Un nom réservé est-il protégé ?
- **Pression :** propriétés de Punchline (étage 1).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### HIS-49 · Un index ou un arrêt ne tue pas la partie
- **Ce qui arrive :** un numéro de duel périmé, utilisé comme index, pouvait tuer le thread de la salle de Punchline. Le manifeste interdisait `unwrap` mais laissait `expect`, `panic!` et l'index hors bornes arrêter la partie de la même façon.
- **D'où on le sait :** 75f0e2e.
- **Mécaniques :** toutes.
- **Question :** une valeur périmée peut-elle servir d'index sans vérification ?
- **Pression :** lints `expect_used`, `panic`, `indexing_slicing` en erreur.
- **Sous la nouvelle forme :** impossible par la forme. Le module est enfermé, un pas qui échoue est interrompu et la partie reprend ; l'entrée fautive est isolée et écartée. Les lints signalent l'erreur dès l'écriture.
- **Étage :** commun.

### HIS-50 · Les fonctions flottantes qui diffèrent entre WebAssembly et natif sont plus nombreuses qu'on croit
- **Ce qui arrive :** la liste du manifeste oubliait `asin`, `atan`, `tanh`, `log2`, `hypot`, `powi` et d'autres. La copie native pouvait diverger du module sans que rien le dise.
- **D'où on le sait :** 75f0e2e.
- **Mécaniques :** physique, temps réel, toutes celles qui calculent en flottants.
- **Question :** chaque méthode flottante appelée a-t-elle un résultat identique en WebAssembly et en natif ?
- **Pression :** `clippy.toml` interdit chaque méthode sensible ; `libm` à la place.
- **Sous la nouvelle forme :** pression du commun. Dans le module, l'enfermement garantit le déterminisme ; la règle protège l'accord entre copie native et module, que l'épreuve compare.
- **Étage :** commun.

### HIS-51 · Une sauvegarde abîmée se charge sans planter
- **Ce qui arrive :** le chargement des sauvegardes de la tier list avait trois bugs. Une sauvegarde corrompue, avec une couleur hors palette ou une tuile manquante, faisait paniquer au lieu de s'afficher.
- **D'où on le sait :** d0906bd, 75f0e2e.
- **Mécaniques :** progression, économie (objets, tuiles).
- **Question :** toute sauvegarde lue, même tronquée ou d'une ancienne version, donne-t-elle un état valide ou un refus net ?
- **Pression :** fuzz `board` de la tier list (2f770aa) ; scénario `storage` (C9), qui doit vraiment toucher la sauvegarde (voir HIS-02).
- **Sous la nouvelle forme :** pression du commun. Le commun conserve l'état et le valide à l'entrée ; une sauvegarde d'une version antérieure passe par une conversion explicite.
- **Étage :** commun.
