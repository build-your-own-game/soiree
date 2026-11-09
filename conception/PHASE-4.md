# Phase 4 : la prédiction sur le téléphone

> **Statut.** Conception retenue pour la phase 4, écrite le 2026-11-09 sur la branche `refonte`, puis reprise le même jour d'après une relecture qui a vérifié ses renvois dans le code (§ 1, « Ce que la relecture a corrigé »). Rien n'est construit ni commité. Elle part de trois propositions indépendantes (« court », « correct », « reel ») et du jugement de deux juges (§ 1). [`MANIFESTE.md`](../MANIFESTE.md) et [`CONTRAT.md`](../CONTRAT.md) font foi, puis [`INTERFACE.md`](INTERFACE.md) et [`REPONSES-PROVISOIRES.md`](REPONSES-PROVISOIRES.md), puis [`PHASE-3.md`](PHASE-3.md) et [`RESTE-PHASE-3.md`](RESTE-PHASE-3.md). Ce document se corrige s'il les contredit. Le mainteneur seul modifie INTERFACE.md et CONTRAT.md : les écarts qu'on y lit sont des questions (§ 9), réunis au § 9.3, pas des changements faits.
>
> **Mesures.** Aucune mesure n'est nouvelle ici, et rien n'a été compilé ni lancé pour l'écrire. Les chiffres mesurés viennent de [`MESURES-PHASE-3.md`](MESURES-PHASE-3.md) (MacBook Apple M5, 16 Go ; Chrome 156, Firefox 158 ; Safari non mesuré), cités par leur section, ou des relevés des jeux (`BESOINS-COMMUN.md`), cités comme relevés. Tout autre chiffre est **choisi**, **estimé** ou **calculé**, et le dit.
>
> **Numéros.** Les décisions de cette phase sont **Q1 à Q28**, pour ne pas se confondre avec les P de la phase 3. Les questions au mainteneur continuent la numérotation de PHASE-3 et de RESTE-PHASE-3 : **questions 34 à 45**. Les mesures nouvelles sont **M16 à M19**. Les contrôles nouveaux de l'épreuve sont **E22** et **E23**.
>
> **Renvois.** La commande de ce document citait « PHASE-3, § 10 » (ce que la phase 3 laisse aux phases 4 et 5) et un « point 8 » différé. PHASE-3 n'a pas de § 10 : ce qui est réservé aux phases 4 et 5 est au **§ 5** ; le point 8 différé est au **§ 9.3** (« l'étape 4 attend la prédiction (phase 4) »). C'est à ces deux endroits que ce texte renvoie.

## 0. En une page

**Ce que la phase 4 livre.** Un jeu qui le déclare voit l'effet d'un geste **sur le téléphone qui l'a fait**, avant la confirmation de l'autorité. Le jeu n'écrit que sa déclaration et `previsible`. Au bout de la phase, le socle des fautifs prédit (`predit`), puis **Punchline prédit un geste, `Envoyer`** : la consigne suivante apparaît sous le doigt. Tout est vérifié par l'épreuve (E22, E23) et par le banc. La télé ne change pas.

**Ce qui simplifie tout.**

1. **Seule la page du téléphone se prédit.** La télé tourne sur la machine qui fait foi : un geste de téléphone ne peut pas y paraître avant d'arriver (Q1).
2. **Le module et les appels sont déjà sur le téléphone.** `Appel::Projeter` et `Appel::Charger` existent dans l'ABI 4. La prédiction tourne dans l'instance de la page, qui existe déjà (Q4).
3. **Aucune montée d'ABI.** La colle de `jeu` apprend ce qu'est une enveloppe chargée par `Charger`. Sur elle, un geste non prévisible attend ; une lecture d'un secret absent, un tirage, ou un secret révélé vide font attendre, **même si le jeu avale l'`Absent`** (Q5). La famille « absent avalé » devient impossible par la forme.
4. **Une seule définition du rejeu**, `partie::prediction`. Le téléphone du navigateur, le téléphone natif du banc et l'épreuve l'appellent tous les trois.
5. **Un seul message neuf**, `VersAppareil::Projection`. Il porte la vue confirmée **et** la projection du même pas, avec ce que le cœur sait et que la projection ne dit pas : le réveil, le temps sauté en attente, et si le temps court (Q27). Pour un téléphone qui prédit, il **remplace** `Vue` : un seul flux, rien à départager (Q8). Rien ne remonte : `VersAutorite` ne change pas.
6. **On rejoue toujours depuis la base.** Charger la projection, rejouer les gestes en vol, regarder, poser la page. Pas d'état incrémental à garder juste. Une session neuve jette la base et refait les gestes en vol depuis la file (Q28).
7. **Une prédiction ne décide jamais, même sur le téléphone.** Le tenu, le retenu et les relâchements sortent de la page confirmée seule (Q14).
8. **Une prédiction ne trompe pas durablement.** Sans geste en vol, la page montrée est celle de la vue confirmée. Un geste non confirmé en 1,5 s cesse d'être montré (Q16). Ce qui est montré ne recule jamais derrière la vue confirmée (Q13).
9. **WebSocket suffit.** La prédiction cache l'aller-retour sous le doigt, quel que soit le canal. WebRTC reste suspendu à M5, mesure humaine (Q21).

**Ce que l'épreuve prouve en plus.** E22, dans l'épreuve : l'étape jouée sur la projection donne la projection de l'étape jouée sur l'état, ou attend ; la machine du rejeu rend la projection de l'autorité quand ses suppositions sont justes ; aucun secret n'est copié dans l'état public. E23, par la vraie `salle` : la même histoire définitive avec et sans prédiction (l'étape 4 du CONTRAT), la convergence, les bornes, et des gestes faits sur la page prédite. E22 est exigé pour déclarer la prédiction ; E23 aussi, tant que sa durée le permet (question 38, § 4.4).

**Les pièces.** Aucune crate nouvelle. Deux modules nouveaux, `partie::prediction` (le rejeu) et `salle::prediction` (l'état du téléphone qui prédit). Un message, `Projection` (`salle::fil::PROTOCOLE` 2). Deux contrôles, E22 et E23. Quatre mesures, M16 à M19. Les lots 0 à 11, dont trois découpés (2a-2b, 5a-5c, 7a-7c), trois en parallèle au départ et deux branches ensuite, puis trois optionnels (§ 7).

**Ce qui n'est pas promis.** Un gain perceptible sur le Wi-Fi d'une vraie soirée n'est pas mesuré. Le lot 0a mesure d'abord le pixel du téléphone **sans** prédiction (M16) : c'est le gain à gagner. Les trois cahiers disent aujourd'hui « aucune prédiction ». Faire prédire Punchline change son cahier : c'est une décision humaine (question 34). **Sans elle, la phase finit sur le socle `predit`** : rien ne prouve alors qu'un vrai jeu prédit. Le coût d'un recalcul de `Envoyer` est près du budget provisoire, et le dépasse très probablement si l'on n'en retire ni `Etat` ni le constat (§ 3.9, calculé) : le lot 0b le relève avant toute construction.

---

## 1. D'où vient cette conception

**Le classement.** Les deux juges se contredisent : « reel > court > correct » pour l'un, « correct > court > reel » pour l'autre. Au total, les trois sont à égalité. « court » est second chez les deux, et les deux y prennent le plus de greffes (la règle 2 de la colle, le lot qui mesure d'abord, la définition d'`inclus`, le tirage paresseux de Punchline). **La base est « court »** : le chemin le plus court vers un jeu qui prédit vraiment, sans montée d'ABI.

**Ce qui est greffé**, comme les juges l'ont désigné :

- de « reel » : `Envoyer` seul pour Punchline ; la règle « un geste disputé reste `Prevision::Non` » (CAT-41, REM-126) ; aucune entrée décidée depuis une page prédite (son D8) ; le geste part avant le calcul (son D17) ; une borne de temps égale à `COUPURE_US` ; la base jamais gardée dans `sessionStorage` ; les refus à l'arrivée comptés dans `inclus` ; les secrets cherchés dans les projections (CAT-46) ; le constat que tout l'état public part sur le téléphone qui prédit (question 18 aggravée) ; le test de la mémoire sur mille prédictions (SAL-186) ; la règle « rien ne recule » ;
- de « correct » : `Secret::montrer` sur une valeur absente fait attendre ; le constat en soirée (la vue recalculée depuis la projection doit égaler celle de l'autorité) ; le champ `partie` dans `Projection` ; l'heure par l'ouverture (`heure_du_pas`) ; l'oracle de la machine du rejeu (son E23c, ici E22b) ; la même histoire avec et sans prédiction, par la vraie salle, lancée en sous-processus comme E20 (ici E23) ; le mode datagramme de `salle::simulation` ; les statistiques sans seuil, séparées de l'exactitude ; le tableau des écarts à INTERFACE ; le recalcul depuis une instance neuve restaurée, gardé en repli ;
- de « court », gardé : les quatre règles de la colle, la garde par l'épreuve, `Projection` qui porte la vue, M16 sans prédiction avant tout, le hasard paresseux de Punchline dès le début, l'instance longue de E22 (qui a servi des pages), ses contrôles négatifs, son annexe des valeurs choisies.

**Ce qui est corrigé**, parce que les juges ou le code le montrent :

- « court » mettait une vue (jusqu'à `limites.vue`, 65 536 octets chez Punchline, `lib.rs`, l. 137) et une projection de 192 Kio dans une trame de 256 Kio (`liaison::TRAME_MAX`, `liaison/src/lib.rs`, l. 57). La borne retranche désormais `limites.vue` et une marge (Q9).
- « court » laissait partir un relâchement REM-23 depuis une page prédite (`salle/src/page.rs`, l. 419 à 441, lâche toute clé disparue) : une entrée décidée par une prédiction. Q14 l'interdit.
- « court » envoyait `Vue` et `Projection` sur le même canal remplaçable, alors que le relais garde une place par origine et par canal (`relais/src/file.rs`, l. 15 à 20) : une `Vue` aurait remplacé dans la file la `Projection` qui confirmait un geste. Ici, un lien qui prédit ne reçoit plus que des `Projection`, qui portent la vue (Q8).
- « court » posait la marque d'absence dans `Autorite`. Or `Sorties::hasard` ouvre le générateur avec un `Autorite::new()` à lui (`jeu/src/etape.rs`, l. 187) : la marque du tirage vit dans `Sorties`. Et `Secret::montrer` (`secret.rs`, l. 255) ne lit rien : une révélation d'un absent passait. La règle 3 de Q5 la ferme.
- « court » déclarait `Personnage` et `Rejoindre` prévisibles chez Punchline : ce sont des gestes disputés, donc souvent démentis. « correct » déclarait `JeSeche`, qui tire toujours le hasard après avoir accepté (`regles.rs`, l. 446 ; PUN-27), et `Voter`, qui lit les auteurs, secrets, et tire un sel (l. 536). Seul `Envoyer` reste (Q24).
- « court » se contredisait sur E19 : environ 3 millions d'unités par prédiction, sous un budget d'une page (`BudgetsE19::page`, 1 302 083, `epreuve/src/budgets.rs`, l. 243). Le budget de la prédiction est posé à part (Q23), et le lot 0b mesure ce qu'il coûte.
- « court » faisait l'étape 4 du CONTRAT par E9 rejoué avec des lectures `Projeter` : cela ne fait jamais tourner la prédiction du téléphone. Ce contrôle reste (E22f, une lecture ne change pas l'histoire), mais l'étape 4 se fait par E23.
- « court » citait SAL-194 comme un doute. La leçon dit l'inverse : l'ancien salon refusait l'exécution optimiste, le CONTRAT la reprend.
- « court » et « correct » disaient `miniz_oxide` « déjà au commun » : seul `crates/epreuve/Cargo.toml` l'utilise. La projection ne se compresse pas (Q9).
- « court » plaçait `EnFile` à la ligne 792 de `salle/src/appareil.rs` : il est à la ligne 103.
- Les trois disaient « la télé tient ses budgets ». C'est vrai dans Chrome, pas dans Firefox avec sa fenêtre : 124 à 137 ms au 99ᵉ centile au clavier, 70 à 80,2 ms au téléphone, le GPU nommé (`MESURES-PHASE-3.md`, « Lot 13 » et « M2 corrigée » ; RESTE-PHASE-3, § 1). La conclusion tient : rien à prédire sur la télé, puisque le retard y est dans le dessin, pas dans un aller-retour.
- « correct » disait que Sidequest et Tier list ne se recompilent pas. Pendant la refonte, chaque jeu prend le commun par chemin (`../../soiree/crates/jeu` ; `soiree/AGENTS.md`, « Chaque changement touche tous les jeux ») : chaque jeu se recompile contre la nouvelle crate `jeu`, nouvelle empreinte, épreuve à repasser. Et `PROTOCOLE` 2 fait recharger les pages restées en cache (§ 6).
- « correct » disait la question 18 non aggravée. Une page n'affiche que ce qu'elle reçoit ; la projection porte tout l'état non secret, plus que la vue. C'est une aggravation (question 42).
- « reel » supprimait le `Temps` du rejeu en disant que « le CONTRAT l'interdit à la lettre ». Le CONTRAT dit « Prédire, c'est exécuter la même étape » ; INTERFACE § 5.3 rejoue `Temps(s)`. Le `Temps` est rejoué où le réveil le demande (Q3). Son exemple tombe aussi : chez Punchline, `hors_phase` lit `j.phase` et `quand.vu`, jamais `maintenant` (`regles.rs`, l. 60) ; c'est le `Temps` qui consomme l'échéance.
- « reel » projetait la définitive « juste avant » chaque geste. `partie` ne projette qu'aux pas finis. E22 travaille pas par pas (§ 4.1).
- « reel » écrivait « l'accusé durable, 100 ms plus M11, soit 8,0 à 9,2 ms en médiane ». M11 vaut 8 à 9 ms en médiane (`MESURES-PHASE-3.md`, « M11, refaite après la vérification du lot 5 ») : l'accusé tombe vers 108 ms après l'écriture, pas à 9 ms.

**Ce que j'ai trouvé en vérifiant**, qu'aucun juge ne relevait :

- « court » bornait le rejeu à 60 pas entre la base et le dernier geste. Or chez Punchline la projection reste la même pendant des secondes (un événement toutes les quelques secondes, son cahier, l. 393). Une base vieille de 300 pas est encore exacte, et la borne aurait coupé presque toutes les prédictions. Une base dont l'état n'a pas changé reste valable : l'autorité n'a rien accepté depuis, donc ni geste ni `Temps` n'a changé l'état (`colle.rs`, l. 658 : le pas de l'enveloppe est celui de la dernière entrée acceptée). La borne porte donc sur le nombre d'entrées rejouées (`REJEU_MAX`) et sur l'âge du geste (`PREDICTION_MAX_US`), jamais sur l'âge de la base (Q16).
- « court » donnait l'heure du `Temps` en pas de 16 667 µs. `Ouverture::heure` est en millisecondes, et l'heure d'un `Temps` s'en déduit par `heure_du_pas` (`partie/src/partie.rs`, l. 41 à 43 et l. 1254). Le message porte l'ouverture, et le téléphone appelle la même fonction (Q8).
- « court » vidait les `Definitif` par un drapeau que `Definitif::de` lirait, sans dire par où il passe : `Definitif::de(cle, valeur)` ne voit ni le regard ni l'enveloppe (`scene.rs`, l. 501). Le regard porte la marque (Q6).
- `version_jeu` (`Bonjour`, `enveloppe.rs`, l. 351) vaut la version de la crate `jeu`, 0.1.0 partout sur `refonte` tant qu'aucune étiquette n'est posée. Une garde par version ne protège que des modules compilés contre une étiquette plus ancienne ; aujourd'hui, il n'en existe aucun du nouveau commun. La vraie garde est l'épreuve (Q7).
- Un téléphone qui prédit doit savoir si le temps court sur l'autorité : la pause et la panne du cœur arrêtent le `Temps` (`partie/src/pas.rs`, l. 267 à 271), et la projection ne les porte pas. *Corrigé à la relecture* : la première version disait que la pause se montre par un écran du commun. C'est faux. La `Projection` porte `temps_libre` (Q27).

**Ce que j'ai vérifié moi-même**, en lecture seule, sans compiler :

- `jeu` : `executer` crée un seul `Autorite` par appel (`colle.rs`, l. 554) ; `Entree` ne demande qu'une enveloppe, donc passe après `Charger`, et la règle du tirage non déclaré rend déjà `Attente` après `appliquer` (l. 636 à 656) ; `Projeter` projette l'état et le hasard, `Charger` refait l'enveloppe (l. 714 à 727) ; `page` pose `confirmee: Oui::Oui` (l. 875) ; l'accord `prediction` ⇒ `telephones` est vérifié en `const` (l. 275) ; `Jeu::previsible` (`traits.rs`, l. 90) n'est appelé nulle part ; `Autorite` n'a qu'un constructeur `pub(crate)` (`secret.rs`, l. 24 à 28) ; `Secret::lire` rend `Err(Absent)` sur une valeur vide (l. 237 à 240) ; `montrer` change `qui` sans rien vérifier (l. 255) ; aucun jeu n'utilise `Definitif` ; un `impl Etat` écrit à la main est refusé par l'étape 1 (`forme.rs`, l. 38 à 41).
- `jeu-abi` : `Projeter` et `Charger` (`enveloppe.rs`, l. 101 à 111) ; `Page { montre, pas, locaux }` (l. 167) ; `version_jeu` (l. 351) ; `QuandFil { fait, fraction, recu, vu, id }` (l. 631). `moteur-commun` : `verifier_chargement` exige l'égalité de l'ABI (`chargement.rs`, l. 154). `partie::Instance` a `instantane` et `restaurer` (`instance.rs`, l. 57 à 60).
- `salle` : `fil::PROTOCOLE` vaut 1 (`fil.rs`, l. 20) ; `Accueil` ne porte pas l'`Appareil` (l. 87 à 105) ; `Vue { pas, montre }` (l. 129) ; `CANAL_VUE` vaut 0 (`autorite.rs`, l. 99) ; `EnVol` (l. 340) ; `id_geste(appareil, onglet, numero)` (l. 546) ; un geste placé entre dans `en_vol`, un refusé à l'arrivée n'est que compté (l. 1866 à 1898) ; la vue part en `Classe::Remplacable(CANAL_VUE)` quand son empreinte change (l. 2297 à 2320) ; `COUPURE_US` vaut 1,5 s (`page.rs`, l. 47) ; `InstanceDePage` (l. 81) ; le relâchement REM-23 (l. 419 à 441).
- `partie` : `regles::placer` (l. 64), `suivre` (l. 102), `reveil_demande` (l. 549) ; dans un pas, les arrivées puis le `Temps` si le réveil le demande et que le cœur n'est ni en pause ni en panne (`pas.rs`, l. 248 à 271) ; `heure_du_pas` est `pub(crate)` (`partie.rs`, l. 1254) ; le temps jeté se garde dans `coeur.saut` jusqu'au prochain `Temps` (`pas.rs`, l. 630 et 1102), et `Ligne::Saut { de, a }` saute des pas sans les jouer (l. 377 à 381).
- `epreuve` : E8 repart d'une instance neuve restaurée et charge la projection (`soiree.rs`, l. 2382 à 2460), tous les 30 pas ou à un changement de phase (`PERIODE_SECRETS`, l. 77 et 2166), et ne cherche les secrets que dans la vue (l. 2479 à 2492), après avoir aligné la forme canonique et la projection octet par octet (`controles::places_cachees`, l. 92 à 130, qui rend `None` sans rien dire si elles ne s'alignent pas) ; le rapport dit l'étape 4 incomplète (`rapport.rs`, l. 238 à 240) ; E20 trouve `soiree-banc` par `SOIREE_BANC` (`deux_moteurs.rs`, l. 123 à 139) ; `MARGE_TELEPHONE` vaut 4 (`budgets.rs`, l. 186).
- Punchline : `envoyer` prend toujours le générateur (`regles.rs`, l. 411), `je_seche` aussi, après avoir accepté (l. 446), `voter` aussi (l. 536) ; `rendre` avale un `Absent` (`if let Ok(prive)`, l. 682 à 683) ; `quelqu_un_attendu` ne lit à l'écriture que des champs publics, et avale les auteurs absents au vote (l. 827 à 840) ; `montrer` sert aux révélations (`manche.rs`, l. 649 à 675) ; `previsible` rend `Non` (`lib.rs`, l. 250) ; le brouillon est un tenu (`regles.rs`, l. 459), accepté jusqu'à 10 fois par seconde et par joueur (`debit_max: 10`, `lib.rs`, l. 114), qui lit le `Prive` du joueur et s'ignore s'il manque (l. 476) ; le bot de l'épreuve tient un brouillon puis envoie, dans le même coup (`epreuve.rs`, l. 397 à 410).

### Ce que la relecture a corrigé

Une relecture a vérifié les renvois de la première version dans le code de `refonte`, sans rien compiler. Chaque constat ci-dessous a été revérifié avant d'être suivi.

- **La pause n'arrive pas au téléphone.** `AttenteCommun` n'a que `Chargement`, `Panne` et `Reprise` (`salle/src/fil.rs`, l. 206 à 213). L'autorité n'envoie aucune `VersAppareil::Attente` (aucun envoi dans `salle/src/autorite.rs`) ; seul l'appareil se pose `Reprise` quand le relais dit l'autorité absente (`appareil.rs`, l. 553). Aucun appel à `Partie::pause` n'existe dans `salle` ni dans `web/`. Le téléphone ne voit donc ni la pause ni la panne du cœur. La `Projection` porte `temps_libre` (Q27).
- **Le saut change le rejeu.** Après un saut, l'autorité joue un seul `Temps`, au `saut` non nul, à l'heure d'un pas qui suit le saut. La `Projection` porte le saut en attente (Q27), et l'oracle exclut toute fenêtre qui contient une ligne `Saut`, `Pause`, `Reprise` ou `Partie`. La question 30 touche donc la prédiction (§ 9.2).
- **La reprise fait reculer le pas.** Les vues partent avant que leur case soit écrite (`autorite.rs`, l. 2050 puis 2060 ; une case au plus tous les `journal_tous_les_us`). Une autorité reprise repart souvent d'un pas plus bas que la dernière base reçue. Un geste inclus mais pas encore écrit disparaît de la définitive reprise : l'appareil le renvoie depuis sa file (`appareil.rs`, l. 717 à 726, SAL-56), mais le prédicteur l'avait retiré de `en_vol`. À chaque accueil, la base tombe et `en_vol` se refait depuis la file (Q28).
- **E8 ne projette pas à chaque pas**, mais tous les 30 pas ou à un changement de phase. E22 (a) appelle `Projeter` lui-même (§ 4.1).
- **Le regard en tête de `Projeter` aurait aveuglé E8 sans un mot.** `places_cachees` n'aurait plus rien aligné : aucun secret cherché, E8 vert. Le lot 2b retire le préfixe dans l'épreuve, et le prouve (§ 2.4, § 7).
- **La règle 3 change un test de `devine`.** `projection_cache_le_nombre_et_le_hasard` (`devine/tests/regles.rs`, l. 315 à 333) exige `Acceptee` d'un `Temps` qui révèle un nombre absent (`nombre.montrer(Qui::Tous)`, `devine/src/lib.rs`, l. 254 et 809). Avec la règle 3, il attend. Le lot 2b change ce test exprès.
- **E22 (b) comparait des vues que la règle 4 rend différentes.** Il compare désormais les projections (§ 4.1). `QuandFil` ne porte pas la place (`enveloppe.rs`, l. 631) : elle est dans `ArriveePlacee.place` (`journal.rs`, l. 230 à 245).
- **Chez Punchline, E22 (b) risquait de n'avoir aucune fenêtre** : un brouillon, tenu, tombe dans presque toute fenêtre pendant l'écriture. L'oracle rejoue aussi les entrées du journal qui ne sont pas des gestes en vol (§ 4.1), et la porte du lot 0b compte les fenêtres jugeables.
- **L'état de Punchline change souvent** : chaque brouillon accepté le change. La projection d'un regard ne se calcule plus qu'au moment où un envoi est dû (Q10).
- **Le catalogue ne dit ni E22 ni E23** (`relais/src/paquet.rs`, l. 296 à 306 : `empreinte`, `taille`, `eprouve`), et `ReglagesSalle` n'a pas de champ pour prédire (`autorite.rs`, l. 119 à 132). Q7 et le lot 6 le prennent en compte, et le banc reçoit une permission à lui.
- **Le calcul du coût était faux** : cinq coûts fixes pour six appels, et la page la plus chère relevée n'était pas la plus grande des deux relevées (§ 3.9). `Etat` est retiré du rejeu : le réveil vient de l'autorité (Q27).
- **Une erreur d'un pas** dans la place estimée (§ 3.3, point 2).
- **Les lots** étaient mal ordonnés (le lot 6 jouait `predit`, né au lot 4) ou trop gros pour un agent (2, 5, 7) : ils sont découpés (§ 7).
- **Des preuves sans témoin positif** : un passage du banc vert sans aucune page prédite posée ne prouve rien (§ 7, lots 9 et 10).
- **Des écarts au CONTRAT et à PHASE-3 manquaient au § 9.3**, et des décisions qui reviennent au mainteneur au § 9.2.

---

## 2. Le modèle

### 2.1 Ce qui se prédit

- **La page du téléphone qui a fait le geste**, et rien d'autre : ni la scène de la télé, ni la page d'un autre téléphone (Q1). Le CONTRAT : « Seules les conséquences du geste de l'appareil s'affichent avant confirmation ».
- **Ses propres gestes, seulement ceux que `previsible` accepte** (Q2). Pas ses tenus en phase 4 : leurs règles (débit, fraîcheur, relâchement) sont celles du cœur, et les rejouer ailleurs serait une seconde définition. Pas les gestes des autres, jamais inventés.
- **Le `Temps`, là où le réveil le demande, jusqu'à la place du dernier geste en vol** (Q3). L'autorité le jouera avant ce geste ; ne pas le rejouer, c'est prédire faux par construction. Le premier `Temps` rejoué porte le temps sauté que la base dit en attente ; aucun `Temps` n'est rejoué si la base dit que le temps ne court pas (pause ou panne du cœur, Q27). Un saut à venir ne se prédit pas : il fait une prédiction fausse, corrigée par la base suivante. Au-delà du dernier geste en vol, rien : le téléphone n'extrapole jamais le monde. Sans geste en vol, il montre la vue confirmée.
- **Une issue qui porte une sortie** (`Oublier`, `Retirer`, `NouvellePartie`) arrête le rejeu comme une attente : une sortie est une décision que l'autorité exécute au pas définitif (INTERFACE, § 4.4).

### 2.2 Où elle tourne

- **Sur le téléphone, dans l'instance de la page** (Q4). `InstanceDePage` tient déjà le module réécrit sous `PHASE_3`, après `Bonjour` et `Decrire`. `Page` ne lit que ses arguments : la même instance peut `Charger`, jouer, `Regarder`, puis servir `Page`. Une faute pendant la prédiction refait l'instance, comme une faute de `Page` aujourd'hui. P19 l'annonçait : « la phase 4 trouvera le module chargé ».
- **Le rejeu est une fonction pure**, `partie::prediction`, sur le trait `partie::Instance`. Elle sert le téléphone du navigateur (`web/telephone`), le téléphone natif (`salle::page::JoueurDePage`) et l'épreuve (E22). `epreuve` dépend déjà de `partie`, pas de `salle`.
- **L'état du téléphone qui prédit** (la base, les gestes en vol, les coupures, ce qui se montre) est dans `salle::prediction`, sans entrées-sorties, comme `salle::page`.
- **Pas dans le Worker de l'écran.** C'est l'autorité : elle n'a rien à deviner. Elle calcule seulement les projections.
- **Pas dans le JavaScript de la page.** Le CONTRAT : la page « ne prédit rien elle-même ».

### 2.3 Ce que le jeu déclare

```rust
// Declaration (INTERFACE § 2.12) : le champ existe, aujourd'hui SansObjet partout.
prediction: Applicable::Oui(()),
// Le trait Jeu (INTERFACE § 2.13) : la méthode existe, aujourd'hui `Prevision::Non` partout.
fn previsible(g: &GesteTel) -> Prevision {
    match g {
        GesteTel::Envoyer { .. } => Prevision::Oui,
        _ => Prevision::Non,
    }
}
```

Et, s'il le veut :

- **`Definitif<T>`** dans sa vue de téléphone, pour une valeur qui ne s'annonce qu'une fois sûre (un score, une élimination). Une vue prédite la rend vide (Q6, CAT-41).
- **`Tenue::Provisoire`** sur un effet qu'on peut jouer avant confirmation (une vibration courte). Un effet `Definitive` attend la vue confirmée (Q15).
- **Nommer ce qu'un geste crée depuis `quand.id`** (INTERFACE, § 5.4), jamais depuis un compteur de l'état.

Ce que le jeu doit savoir (question 42) : **tout ce qui n'est pas `Secret` part sur chaque téléphone qui prédit**, puisque la projection porte l'état public entier, pas seulement ce que la vue montre. Le rapport de E22 le dit, avec la taille.

La règle pour choisir ses gestes : **un geste disputé reste `Prevision::Non`** (CAT-41, REM-126). Deux joueurs qui prennent le même personnage, un prénom contre ceux des autres, « le premier qui… » : chacun verrait sa victoire, et l'un serait démenti.

`fenetre` reste `SansObjet` dans les trois jeux (Q25). La prédiction sur le téléphone n'en dépend pas : la fenêtre sert la tête de l'autorité.

### 2.4 Ce que la colle garantit par la forme (crate `jeu`, sans montée d'ABI)

L'enveloppe gagne deux marques internes, hors de la forme canonique : `projetee` (posée par `Charger`, ôtée par `Creer` et `DepuisCanonique`) et `predite` (posée par la première `Entree` acceptée sur une enveloppe projetée). La sortie de `Projeter` gagne le regard, en tête : `Charger` le relit. Ce n'est pas la forme canonique de la garde (`DepuisCanonique` ne change pas) : seuls `Projeter` et `Charger`, toujours dans le même module, la lisent et l'écrivent.

**L'épreuve doit retirer ce préfixe.** E8 aligne la forme canonique et la projection octet par octet pour trouver les secrets vidés (`controles::places_cachees`, l. 92 à 130). Un préfixe qui ne s'aligne pas lui fait rendre `None`, et la recherche ne cherche rien, sans échec ni compte (`soiree.rs`, l. 2479 : `secrets_non_cherches` ne bouge pas). E8 resterait vert chez tous les jeux, E22 (g) aussi. Le lot 2b décode donc le `RegardFil` en tête (postcard) et aligne le reste, dans `controles.rs` et `soiree.rs`. Sa preuve : `vue-qui-fuit` fait toujours rougir E8, et `secrets_non_cherches` ne change pas chez `devine` ni chez les trois jeux.

Quatre règles, qui ne valent que sur une enveloppe projetée :

1. **Un geste non prévisible attend.** Une `EntreeFil::Geste` que `J::previsible` dit `Non`, ou d'un jeu à `prediction: SansObjet`, rend `IssueFil::Attente` sans appeler l'étape.
2. **Un secret absent lu, ou un tirage, font attendre, même avalés.** Le jeton `Autorite` prêté à l'étape gagne une `Cell<Oui>` privée, que `Secret::lire` et `lire_mut` posent quand la valeur manque. `Sorties` gagne une marque, que `Sorties::hasard` pose quand le générateur manque. Après `J::appliquer`, si l'une est posée, la colle rend `Attente`, quoi que le jeu ait décidé. C'est la règle qui existe déjà pour un tirage non déclaré (`hasard_non_declare`, `colle.rs`, l. 652). `Sorties::nouvelle_partie` lit aussi le générateur avec son propre `Autorite` et avale l'absence (`etape.rs`, l. 214 à 224) : sur une projection, la graine serait nulle. La sortie arrête déjà le rejeu (§ 2.1) ; la marque s'y pose quand même, pour que la règle ne dépende pas de cet ordre.
3. **Un secret révélé vide fait attendre.** Une méthode cachée de `Etat`, générée par `jeu-derive` comme `projeter`, dit s'il reste un secret vide que le regard de la projection voit. Après une entrée acceptée, s'il y en a un, la colle rend `Attente`. C'est `montrer` sur une valeur absente (`secret.rs`, l. 255), que la règle 2 ne voit pas. La méthode s'écrit dans chaque `impl Etat` de `jeu` qui contient un autre `Etat` ou un secret (`forme.rs`, `collections.rs`, `secret.rs`) ; une valeur par défaut, « aucun », sert aux feuilles (`texte.rs`, `temps.rs`, `hasard.rs`). Ce parcours a un coût, à relever (§ 3.9).
4. **Une vue prédite cache ses `Definitif`.** `Regarder` sur une enveloppe `predite` construit un `Regard` marqué `predit` (privé à `jeu`). `Definitif::de(r, cle, valeur)` lit la marque et pose `valeur: None` (Q6).

**Sur l'autorité, rien ne change, par construction** : `Charger` n'y est jamais appelé, donc aucune règle ne s'y applique. E8 appelle `Charger` mais ne joue aucune entrée ensuite.

La règle 2 est la pièce maîtresse. Punchline écrit `if let Ok(prive) = moi.prive.lire_mut(a)` (`regles.rs`, l. 683). Sur une projection où ce secret manquerait, l'étape continuerait sans rien dire, et la vue prédite serait fausse sans qu'aucune attente ne le signale. Avec la règle 2, ce geste attend. Le CONTRAT le demande : « Quand l'étape a besoin d'un secret absent, elle le dit ». Le MANIFESTE aussi : « Face à une erreur connue, nous cherchons d'abord une forme qui la rend impossible. »

`Recue::confirmee` reste `Oui` : la passer à `Non` demande un champ dans `Appel::Page`, donc une ABI 5 (question 43). Ce qui ne doit pas s'afficher avant d'être sûr passe par `Definitif`. C'est un écart : le CONTRAT dit que la page « reçoit ce qu'elle doit montrer, prédit ou confirmé », et PHASE-3 (§ 5) que « `Recue::confirmee` existe : la page ne changera pas ». Ici, une page ne peut jamais savoir qu'elle est prédite (§ 9.3).

### 2.5 Ce que le commun garantit, sans code de réseau dans le jeu

- la projection, calculée et non écrite ; chaque téléphone ne reçoit que ce que son regard voit ;
- son envoi avec la vue confirmée, d'un seul message ;
- le rejeu des gestes en vol depuis la base confirmée, par la même étape et le même module ;
- l'arrêt du rejeu sur un secret absent, un tirage, un geste non prévisible, une sortie ;
- l'affichage sans clignotement, la correction en une fois, la vue confirmée au repos ;
- aucune entrée décidée depuis une page prédite ;
- les effets sous identité, `Provisoire` seuls avant confirmation ;
- la coupure : faute, lenteur, rejeu trop long, geste trop vieux, constat raté, arrière-plan ;
- E22 et E23.

---

## 3. La réconciliation

### 3.1 Le message (`salle::fil`, `PROTOCOLE` 1 → 2)

PHASE-3 (§ 2.7) réservait `Projection { pas, morceaux, inclus_geste, inclus_tenu }`. La forme retenue :

```rust
// Ajouté à la fin de VersAppareil ; octets figés et seconde lecture dans salle/tests/fil.rs.
/// La vue confirmée et la projection du même pas, pour un téléphone qui prédit (phase 4).
/// Remplaçable sur CANAL_VUE : pour ce lien, elle remplace `Vue`.
Projection {
    /// Le pas définitif de la vue et de la projection.
    pas: Pas,
    /// La partie, dans la soirée : une base d'une autre partie est jetée.
    partie: u32,
    /// L'`Appareil` du téléphone : le `de` des gestes rejoués, et `autorite::id_geste`.
    appareil: u64,
    /// Le pas et l'heure (ms) de l'ouverture : l'heure d'un `Temps` par `regles::heure_du_pas`.
    ouverture_pas: u64,
    ouverture_heure: u64,
    /// Chaque geste de cet onglet jusqu'à ce numéro est joué par la définitive à un pas
    /// au plus `pas`, ou refusé à l'arrivée.
    inclus: u64,
    /// Ce que le cœur sait et que la projection ne dit pas (Q27) : le réveil de la
    /// définitive (`EtatCoeur::reveil`), le temps jeté en attente du prochain `Temps`
    /// (`coeur.saut`, en pas), et si le temps court (ni pause ni panne du cœur).
    reveil: ReveilFil,
    saut: u64,
    temps_libre: Oui,
    /// La vue confirmée, comme `Vue` : ce qu'un téléphone qui ne prédit pas montre.
    #[serde(borrow)]
    montre: &'a [u8],
    /// `Appel::Projeter { regard }` recollé : le regard, la forme canonique, secrets d'autrui vidés.
    #[serde(borrow)]
    projection: &'a [u8],
},
```

Ce qui change par rapport à la réserve, et pourquoi :

- **La vue est dans le message.** Vue et projection d'un même pas arrivent ensemble, ou pas du tout : la plus récente remplace l'autre (SAL-28, SAL-112, SAL-113). Un téléphone qui ne prédit pas, ou plus, montre `montre` et ignore `projection`. Un seul flux : aucun ordre à tenir entre deux canaux.
- **Une trame, pas de morceaux.** Un état en morceaux ne peut pas être remplaçable : chaque morceau écraserait le précédent (Q9).
- **Pas d'`inclus_tenu`.** Les tenus ne se prédisent pas (Q2).
- **`appareil`, `partie`, l'ouverture en plus.** Sans l'`Appareil`, le téléphone ne nomme pas ses gestes comme l'autorité (INTERFACE, § 5.4) : `Accueil` ne le porte pas (`fil.rs`, l. 87 à 105). Sans l'ouverture, son `Temps` aurait une heure inventée.
- **Le réveil, le saut et le temps libre en plus** (Q27). Le rejeu en a besoin, et la projection ne les porte pas. Les demander au module (`Appel::Etat`) ferait tourner sur le téléphone `attente()` et `demandes()` du jeu avec l'`Autorite` (`colle.rs`, l. 1007 à 1047) : un coût, et une faute possible que E22 (a) ne voit pas (chez Punchline, `quelqu_un_attendu` y avale des auteurs absents). L'autorité les connaît déjà.

Rien ne remonte : `VersAutorite` ne change pas. Un téléphone qui coupe sa prédiction garde la même réception et montre `montre`.

### 3.2 L'autorité (`salle::autorite`, `partie`)

- **Quand.** Un jeu `prediction: Oui`, un module éprouvé (Q7), un téléphone accueilli. Jamais sous `--non-eprouve`. Aujourd'hui, le catalogue ne dit que `empreinte`, `taille` et `eprouve` (`relais/src/paquet.rs`, l. 296 à 306), et `ReglagesSalle` n'a aucun champ pour prédire (`autorite.rs`, l. 119 à 132). Le plus simple : sous le § 4.4, un code 0 du plan standard d'un module `prediction: Oui` implique E22 et E23 réussis, et le relais refuse déjà un rapport qui ne dit pas ce code 0. `eprouve` et la déclaration du module suffisent donc. Le lot 6 ajoute `ReglagesSalle::predire`, que la coque du Worker (`web/autorite`) pose d'après `eprouve`, lu au démarrage de la page (`web/ecran`, `web/demarrage.js`). Si la question 38 réserve E23 à la publication, ce raccourci tombe : le catalogue doit porter E22 et E23, et le lot 6 touche aussi `relais/src/paquet.rs` et `jouer.rs`.
- **Au banc.** Les modules que le banc sert n'ont pas de rapport dans `soiree` : rien n'y prédirait. Une permission du banc, refusée hors de `--banc` comme `Reglages::annonce_du_banc` (`relais/src/coque.rs`, l. 523), allume la prédiction pour un module sans rapport. Elle ne s'ajoute jamais au binaire de la soirée. (`soiree-banc aide` fabrique déjà un rapport, `rapport_du_banc`, `banc/src/aide.rs`, l. 171 ; une permission dit mieux ce qu'elle fait.)
- **Quoi.** `Partie::projeter(&[RegardFil])`, une lecture nouvelle de `partie/src/lecture.rs`, faite comme `canonique` (par morceaux, sous `BUDGETS_DE_SOIREE.canonique`), sans ligne au journal. Un tour où la définitive a **accepté** une entrée marque chaque regard « à refaire » : sinon l'état, donc la projection, n'a pas bougé (`colle.rs`, l. 658). **La projection d'un regard ne se calcule qu'au moment où un envoi est dû**, jamais à chaque acceptation. Chez Punchline, chaque brouillon accepté change l'état, jusqu'à 10 fois par seconde et par joueur (`debit_max: 10`) : calculer à chaque acceptation ferait huit projections par brouillon. La dernière projection de chaque regard est gardée en mémoire, avec son empreinte.
- **La cadence** (Q10). Une `Projection` part à l'accueil, l'état entier d'abord (SAL-28), puis tout de suite quand `montre`, `inclus`, le saut en attente ou le temps libre change : la projection se calcule alors (le saut et le temps libre changent sans qu'aucune entrée soit acceptée). Quand seul l'état a changé (« à refaire », la vue identique), la projection se calcule au plus une fois toutes les 100 ms par lien (choisi), et part si son empreinte a changé. Rien ne se calcule ni ne part sinon.
- **Le réveil, le saut, le temps libre** (Q27) se lisent dans `EtatCoeur` au moment de l'envoi, sans appel au module, par une lecture que le lot 3 ajoute à `partie` (les champs sont `pub(crate)`, `coeur.rs`, l. 37 à 47).
- **`regarder` continue** pour ce lien : la vue de `montre`, et `EtatCoeur::vues`, la borne de `vu` (`regles::borner_vu`).
- **`inclus`** (Q11). Chaque onglet note `(numéro, place)` à chaque `Placement::Place { pas }`, en ajoutant `place` à `EnVol` (`autorite.rs`, l. 340), et `(numéro, traité)` pour `Malforme`, `HorsScene`, `Perimee`, `FilePleine` et `Ecartee` (l. 1891 à 1895, qui aujourd'hui ne font que compter). `inclus(d)` est le plus grand numéro dont tous les précédents sont traités, ou placés à un pas au plus `d`. Les numéros d'un onglet arrivent dans l'ordre et se dédoublonnent : `inclus` ne fait que monter. La note est bornée par `limites.file`. Après une reprise, elle repart du registre restauré. Chaque geste finit ainsi inclus, sans message de plus.
- **La coupure côté autorité.** Une faute de `Projeter` (une lecture : la définitive se reconstruit déjà), une trame trop grosse : ce lien reçoit des `Vue` jusqu'au module suivant, et l'autorité le compte (`ComptesSalle::projections_coupees`).
- **La garde et la reprise ne changent pas.** Une projection se calcule, elle ne se garde pas. Après une reprise : l'accueil, puis une `Projection` neuve (SAL-28). Son pas peut être plus bas que celui de la dernière base envoyée : les vues partent avant que leur case soit écrite (`autorite.rs`, l. 2050 puis 2060), et la reprise repart de la dernière case écrite. Le téléphone le sait par l'accueil (Q28).
- **Le coût** s'ajoute à M13, la part du commun dans le Worker : M13 bis (§ 5.3).

### 3.3 Le téléphone : rejouer depuis la base

`salle::prediction` tient, en plus de ce que tient `salle::appareil` :

```rust
pub struct Predicteur {
    /// La dernière base reçue dans cette session : plus récente, en (partie, pas, inclus),
    /// que toute autre reçue depuis l'accueil (Q28).
    base: Option<partie::prediction::Base>,
    /// Les gestes envoyés que la base n'inclut pas encore, prévisibles ou non.
    en_vol: Vec<partie::prediction::GestePredit>,
    /// Coupée, et pourquoi ; trois lenteurs de suite coupent.
    coupee: Option<Coupure>,
    lentes: u8,
    /// La base de la dernière constatation réussie (Q17).
    constatee: Option<(u32, Pas)>,
    pub comptes: ComptesPrediction,
    /// Les contrôles négatifs du banc, compilés partout, posés par le banc seul.
    fautif: Option<FautifPrediction>,
}
pub enum Coupure { Faute, Lente, RejeuTropLong, GesteTropVieux, Constat, ArrierePlan, EcranDuCommun }
```

`en_vol` n'est pas la file des gestes non accusés (`appareil.rs`, l. 103) : celle-ci se vide à l'accusé durable, une case de garde plus tard (environ 100 ms plus M11), qui peut précéder ou suivre l'inclusion. Deux comptes distincts : l'inclusion pour la prédiction, l'écriture sur disque pour l'accusé.

Le rejeu, dans `partie::prediction` (forme indicative ; le lot 3 la fixe) :

```rust
pub struct Base { pub pas: Pas, pub partie: u32, pub appareil: Appareil,
                  pub ouverture: (u64, u64), pub inclus: u64,
                  pub reveil: ReveilFil, pub saut: u64, pub temps_libre: Oui,
                  pub projection: Vec<u8> }
pub struct GestePredit { pub numero: u64, pub octets: Vec<u8>, pub fait: Instant,
                         pub vu: Pas, pub id: IdGeste }
/// Les places : estimées sur le téléphone ; vraies dans l'oracle de E22, avec les
/// autres entrées du journal de la fenêtre (tenus, gestes d'autrui, entrées
/// synthétisées), chacune à sa vraie place.
pub enum Places<'a> {
    Estimees { demi_aller_retour: Duree },
    Vraies { quand: &'a [(QuandFil, Pas)], autres: &'a [(Pas, Vec<u8>)] },
}
pub enum Arret { Attente, Sortie, RejeuTropLong }
pub struct Rejoue { pub vue: Option<(Pas, Vec<u8>)>, pub joues: usize,
                    pub arret: Option<(u64, Arret)>, pub carburant: u64 }
pub fn predire<I: Instance>(i: &mut I, base: &Base, gestes: &[GestePredit],
                            places: Places<'_>, regard: RegardFil) -> Result<Rejoue, Echec>;
```

`predire` fait, dans l'ordre :

1. `Charger { projection }`, une écriture ;
2. pour chaque geste en vol, sa place (Q12) : `regles::placer` avec `recu` estimé à `fait + demi-aller-retour` et pour définitif estimé le pas qui précède l'arrivée estimée, au moins `base.pas`, puis `regles::suivre` sur la place du geste d'avant. `placer` rend déjà `place ≥ definitif + 1` (`regles.rs`, l. 60 et 73) : un définitif estimé d'au moins `base.pas + 1`, comme l'écrivait la première version, forçait `place ≥ base.pas + 2`, un pas trop tard. La propriété du lot 3 (« la place estimée égale celle de `regles::placer` ») doit l'attraper ;
3. de `base.pas + 1` à la dernière place, pas à pas, dans l'ordre de `commencer_pas` (`partie/src/pas.rs`) : les gestes placés à ce pas, par `fait` puis par numéro (`EntreeFil::Geste { de: appareil, quand: QuandFil { fait, fraction, recu, vu, id } }`), puis, si `base.temps_libre` et `regles::reveil_demande(reveil, s)`, `Temps { saut, heure: heure_du_pas(ouverture, s) }` : le premier `Temps` rejoué porte `base.saut`, les suivants 0, comme `coeur.saut` sur l'autorité (`pas.rs`, l. 630). Le réveil vient de `base.reveil`, puis de chaque `IssueFil::Acceptee`. Aucun `Appel::Etat` (Q27) ;
4. `Attente`, ou une `Acceptee` qui porte une sortie, arrête ce geste et les suivants (INTERFACE, § 5.3) ; `Refusee` et `Ignoree` laissent l'état, et l'on continue ; aucun avis prédit ne s'affiche, seul l'`Avis` de l'autorité parle ;
5. au-delà de `REJEU_MAX` entrées (gestes et `Temps`, 60, choisi), on s'arrête (`RejeuTropLong`) ;
6. `Regarder { regards: [regard] }` : la vue prédite, au pas du dernier geste joué.

Dans l'oracle (`Places::Vraies`), le point 2 prend les vraies places, et le point 3 joue aussi, à leur vraie place et dans l'ordre du journal, les entrées `autres`. Les `Temps` n'en sont pas : le rejeu les synthétise comme sur le téléphone, et c'est ce que l'oracle éprouve.

`vu` reste le pas de la dernière vue **confirmée** : un geste fait sur une page prédite dit ce que le joueur a vu de confirmé. L'autorité borne `vu` par la dernière vue qu'elle a produite (`regles::borner_vu`) ; une vue prédite n'en est pas une.

On rejoue **toujours depuis la base**, à chaque geste et à chaque base neuve. C'est plus simple que tenir une instance à jour, exact par construction, et borné par `REJEU_MAX`. Une base dont l'état n'a pas bougé reste valable, même vieille (§ 1, « Ce que j'ai trouvé »).

À chaque événement :

- **Un `Accueil` arrive** (une session neuve : `session` est « neuf à chaque accueil », `fil.rs`, l. 88) (Q28). La base tombe. `en_vol` se refait depuis la file entière des gestes non accusés, dans l'ordre (`appareil.rs`, l. 103) : un geste inclus par l'ancienne base mais pas encore écrit a pu disparaître de la définitive reprise, et l'appareil le renvoie (l. 717 à 726, SAL-56). La première `Projection` de la session est prise quel que soit son pas, puis élague `en_vol` par son `inclus`. Sans cela, une autorité reprise d'un pas plus bas verrait toutes ses `Projection` jetées, et la page reculerait sur le geste perdu (CAT-41). Aujourd'hui l'appareil prend toute `Vue` sans comparer de pas (`appareil.rs`, l. 630 à 634) : il n'a pas ce problème.
- **Une `Projection` arrive.** Dans une session, elle est jetée si elle est plus ancienne que la base en `(partie, pas)` (SAL-111) ; à `(partie, pas)` égal, celle qui a le plus grand `inclus` gagne : un geste refusé à l'arrivée fait monter `inclus` sans que le pas définitif ait bougé. Sinon elle devient la base ; les gestes de numéro au plus `inclus` quittent `en_vol` ; la page se recalcule.
- **Un geste part** (Q18) : il est envoyé d'abord. Puis il entre dans `en_vol`, prévisible ou non, et la page se recalcule. Le téléphone ne lit pas `previsible` lui-même : un geste non prévisible rend `Attente` au rejeu (règle 1 de la colle), et arrête la prédiction à sa place ; les gestes qui le suivent attendent avec lui (INTERFACE, § 5.3).
- **Le tic** : un geste en vol depuis plus de `PREDICTION_MAX_US` (1,5 s, `COUPURE_US`) ne se montre plus (Q16) ; tant qu'il est dans `en_vol`, aucune prédiction ne se montre, puisque montrer les gestes suivants sans lui serait faux. `salle::prediction` n'a pas d'horloge : l'heure lui est donnée. Dans `web/telephone`, c'est son tic de 50 ms (`TIC_MS`, `web/telephone/src/page.rs`, l. 44 ; à ne pas confondre avec `salle::chien::TIC_MS`, 100 ms, le chien de l'écran) ; pour le téléphone natif (`JoueurDePage`), l'horloge de `salle::simulation` ou du banc, à chaque tour.
- **Une `Vue`** (l'autorité a coupé ce lien) : la page montre `montre`, `en_vol` reste, rien ne se prédit avant la prochaine `Projection`.

Au plus un recalcul par image, comme `Page` aujourd'hui (PHASE-3, § 2.10).

### 3.4 Afficher, et corriger sans saut choquant

La règle, dans `salle::page` (Q13) :

- **La page montre la vue prédite si et seulement si** la prédiction n'est pas coupée ; `en_vol` n'est pas vide ; le rejeu a joué au moins le premier geste en vol ; la base est au moins aussi récente que la dernière vue confirmée reçue ; aucun écran du commun ne couvre la page (« Un instant… », « Remplacé », le chargement). La pause n'en est pas un : elle n'arrive pas au téléphone (§ 1, « Ce que la relecture a corrigé ») ; la base dit seulement que le temps ne court pas (Q27).
- **Sinon**, la page montre la vue confirmée, `montre`.
- **Jamais la vue confirmée d'une base qui n'inclut pas encore un geste prédit.** À chaque base neuve, le rejeu se refait sur elle. C'est ce qui évite le clignotement classique : prédite A, puis l'ancienne vue, puis A encore.
- **Une prédiction fausse se corrige d'un coup**, à la base qui inclut le geste : la page passe à la vue confirmée, à l'image suivante, sans fondu (REM-116, REM-119). La page est un arbre en flux dans le DOM : il n'y a pas de position à faire glisser, et un écran discret ne se lisse pas. `Noeud::lisse` (dans l'ABI, non dessiné) reste hors du chemin du téléphone. C'est un écart au CONTRAT (« le lissage des corrections ») et à INTERFACE § 5.3 (§ 9.3).
- **Ce qui ne bouge jamais.** Un champ qui a le focus ou qui compose garde sa valeur (PHASE-3, § 2.10 ; PUN-40). Les locaux appartiennent au commun et survivent à une page qui retire leur clé (SAL-165) : si `Envoyer` est démenti, la consigne revient avec son texte.
- **Mesurer avant de retarder.** REM-117 : un affichage de moins d'environ 100 ms se lit comme un scintillement. On ne retarde pas une correction pour autant. M19 compte la durée de chaque page fausse montrée. Si elle est souvent sous 100 ms, on décidera.
- **Une reprise peut faire reculer ce que montre la page.** L'autorité reprise repart d'un pas plus bas (§ 3.2) : la télé recule aussi. Les gestes du joueur, eux, ne disparaissent pas de la page prédite, puisque `en_vol` se refait depuis la file (Q28). « Rien ne recule » vaut dans une session.

### 3.5 Une page prédite ne décide rien

`salle::page` calcule deux pages quand les vues diffèrent (Q14) :

- **la page montrée** sort de la vue prédite. Ses boutons envoient de vrais gestes : le joueur agit sur ce qu'il voit, l'autorité revalide ;
- **la page confirmée** sort de `montre`. Elle seule donne le tenu, le retenu et les relâchements de REM-23.

Si la page prédite retire une clé que le doigt tient (`Maintenu`, `Croix`), elle ne se montre pas : la page confirmée reste. Sans cette règle, une prédiction enverrait un relâchement : une entrée décidée par une prédiction. La page confirmée est celle que le téléphone calcule déjà aujourd'hui à chaque `Vue` : elle ne coûte rien de plus par geste.

Un geste fait sur la page prédite dit `vu` égal au dernier pas confirmé (§ 3.3). Un champ qui n'existe que sur la page prédite peut porter un geste. Ce chemin s'éprouve dans la troisième soirée de E23 (§ 4.2).

**Ce que le joueur peut perdre** (question 34). Chez Punchline, après `Envoyer`, le champ de la consigne suivante n'existe que sur la page prédite. Le texte qu'on y tape reste dans les locaux, mais le brouillon, un tenu, ne part que de la page confirmée : il ne part qu'à la confirmation. Si l'échéance de l'écriture tombe entre les deux, ce texte ne compte pas (PUN-01). La fenêtre est d'un aller-retour, non mesurée sur un vrai Wi-Fi.

### 3.6 Les effets

- D'une vue prédite, seuls les effets `Provisoire` se jouent, sous leur identité (SAL-167, INTERFACE § 5.4).
- Un effet `Definitive` attend une vue confirmée.
- Un `Provisoire` absent de la vue confirmée n'est pas joué s'il ne l'a pas encore été ; s'il est parti, il ne se coupe pas (Q15). Sur le téléphone, il n'y a aujourd'hui que la vibration (lot 12). C'est un écart au CONTRAT, « un effet prédit à tort s'annule » (§ 9.3, question 37).
- Les effets joués restent gardés dans `sessionStorage` (P26) : une page rechargée ne rejoue pas un effet prédit puis confirmé.

### 3.7 La coupure (CONTRAT : « La prédiction peut toujours être coupée »)

Sur le téléphone (Q16) :

- **une faute** pendant `predire` (panique, budget, sortie illisible) coupe jusqu'au module suivant ; l'instance se refait, la page montre la confirmée ;
- **trois recalculs de suite au-delà de 8 ms murales** (choisi : la moitié d'une image à 60 Hz) : c'est le téléphone lent ; coupé jusqu'au rechargement. `salle::prediction` ne mesure rien : la durée lui est donnée. La page la mesure par `performance.now()`. Dans `salle::simulation` et dans E23, une durée murale dépendrait de la charge d'une machine partagée, et rendrait E23 non déterministe : la durée y est déduite du carburant du recalcul (unités × une durée d'unité choisie), ou la coupure lente y est éteinte. Elle s'éprouve à part, dans un test de `salle::prediction`, avec des durées données ;
- **un rejeu de plus de `REJEU_MAX` entrées** : ce recalcul montre la confirmée ;
- **un geste en vol depuis plus de 1,5 s** : il ne se montre plus ; il reste dans la file, renvoyé comme aujourd'hui. *Corrigé à la relecture* : la première version disait qu'au-delà, l'écran du commun dit déjà la coupure (P28). Non : P28 juge le silence de l'écho, pas l'âge d'un geste. Un geste peut rester sans confirmation plus de 1,5 s pendant que les échos répondent (file pleine, module en cours de livraison, Worker lent). La page montre alors la confirmée sans rien dire ; le diagnostic compte ces gestes ;
- **un constat raté** (Q17) : coupé jusqu'au module suivant, compté, et dit au diagnostic ;
- **l'arrière-plan** (REM-11) : suspendu ; au retour, on attend la première `Projection` ;
- **le rechargement** (P26, SAL-56) : les gestes non accusés reviennent de `sessionStorage` et rentrent dans `en_vol` ; la base, elle, n'est jamais gardée (Q26). Un rechargement de plus de 1,5 s les rend trop vieux pour être montrés : la page montre la confirmée jusqu'à ce qu'ils soient inclus. Ils ne « se reprédisent » donc que sur un rechargement court ;
- **l'onglet jumeau** (SAL-18, SAL-36) : l'ancien reçoit `Remplace` et cesse ; `inclus` vaut par onglet ;
- **l'autorité qui reprend** : « Un instant… » couvre la page ; à l'accueil de la session neuve, la base tombe et `en_vol` se refait depuis la file (Q28) ; la première `Projection` de la session est prise, même d'un pas plus bas ;
- **le changement de jeu** : l'instance se refait sur le nouveau module, `en_vol` se vide des gestes de l'ancien.

La reprise est la même que sans prédiction : la vue confirmée, puis la base suivante.

### 3.8 Le constat en soirée (Q17)

Sur une base neuve, le téléphone appelle `Charger`, puis `Regarder`, et compare la vue obtenue à `montre`, octet pour octet. C'est E8 constaté en soirée (`vue(état, r) = vue(projection(état, r), r)`, INTERFACE § 5.2). Le constat se fait **à part du recalcul**, à l'arrivée de la base, sur son propre budget, et non dans l'image où un geste se prédit : avec lui, le recalcul de `Envoyer` dépasserait très probablement son budget (§ 3.9). S'il coûte encore trop, il se fait sur une base sur deux, ou sur la première base de chaque module (M17 le dira). Un écart est une panne de l'outil (un moteur qui diverge, JavaScriptCore compris, question 44) : la prédiction se coupe, au lieu de démentis que personne n'explique. MANIFESTE : « il ne croit rien sur parole : il constate ».

### 3.9 Le coût sur un téléphone

**Mémoire.** Pas de seconde instance. En plus : une base, de l'ordre de la forme canonique du jeu, et `en_vol`, au plus `limites.file` gestes de `limites.geste` octets. Chez Punchline, un diagnostic de E8 a relevé des formes canoniques de 1 104 et 1 106 octets (`games/punchline/BESOINS-COMMUN.md`, § 9 ; un relevé, pas une mesure de la projection, que M17 fera). Le tas de l'instance ne doit pas grandir d'une prédiction à l'autre : `mille_predictions_ne_font_pas_grandir_la_memoire_sal186`. S'il grandit, le repli est le recalcul depuis un instantané neuf restauré, puis `Charger` : la configuration de E8, et `restaurer` existe dans le navigateur (`partie::Instance`).

**Carburant**, calculé à partir des relevés de E19 au plan standard de Punchline (`games/punchline/BESOINS-COMMUN.md` ; relevés, non mesurés sur un téléphone) : une `Entree` au plus 701 661 unités (relevé du 2026-11-08) ; une page au plus 846 390 (plan standard du 2026-11-07, 1 024 pages, l. 178) ou 728 878 (relevé du 2026-11-08) : E21 n'éprouve qu'un échantillon des vues, on prend la plus grande des deux ; plus 155 800 de coût fixe par appel.

- *La première version* comptait `Charger`, `Etat`, `Regarder` (le constat), une `Entree`, `Regarder` et `Page`, soit six appels, avec cinq coûts fixes et la plus petite des deux pages : « environ 2,2 millions ». Avec six coûts fixes : 701 661 + 846 390 + 6 × 155 800 = 2 482 851 unités (calculé), **sans** le travail de `Charger`, d'`Etat`, des deux `Regarder` ni le parcours de la règle 3, non relevés. Le budget de Q23 (2 604 166) laissait 121 315 unités pour tout cela : **très probablement dépassé**, pas « à la limite ».
- *Avec les réductions retenues* : `Etat` retiré (Q27), le constat à part (§ 3.8). Un recalcul fait `Charger`, une `Entree`, `Regarder` et `Page` : 701 661 + 846 390 + 4 × 155 800 = 2 171 251 unités (calculé), plus le travail de `Charger`, d'un `Regarder` et de la règle 3, non relevés. Il reste 432 915 unités pour eux. Chez Punchline, presque tout le coût d'une page est le décodage de la vue (son `BESOINS-COMMUN.md`, l. 178) ; l'encodage par `Regarder` n'est pas relevé.

Le lot 0b relève ce qui manque (M17) avant le lot 2a. Si le recalcul dépasse encore, trois sorties : la question 22 (la marge du téléphone, donc le budget de Q23, question 39), une vue de téléphone plus légère chez Punchline (ses textes de vue en `String`, que son `BESOINS-COMMUN.md` envisage), ou retour à Q23.

**Temps.** À 47 à 65 ps l'unité au 99ᵉ centile dans Chrome (`MESURES-PHASE-3.md`, « M1 remesurée »), 2,6 millions d'unités font environ 0,17 ms sur un M5 (calculé), environ 0,7 ms avec la marge du téléphone de 4 (choisie, question 22). **Rien n'est mesuré sur un vrai téléphone** : M18 le mesure dans les pages émulées du banc ; la fiche de répétition, sur de vrais téléphones.

### 3.10 Les gros états : une projection, jamais une mémoire

- On transfère **une projection entière**, jamais la mémoire du module ni un point de reprise. La mémoire porte tous les secrets ; la projection les vide par les types (CONTRAT : « Chaque appareil ne reçoit que ce qu'il a le droit de savoir »). Pas de delta (SAL-113).
- La seule borne est celle de Q9. Tier list, le seul gros état (16,8 Mio au pire, `MESURES-PHASE-3.md`, « Lot 2 », M3), n'a pas de téléphone : il ne prédit jamais.
- Un jeu à téléphones dont la projection dépasserait la borne ne peut pas déclarer la prédiction, et E22 le dit. Les morceaux, la compression ou les blocs immuables (question 13) viendront s'il en apparaît un.

---

## 4. Ce que l'épreuve prouve en plus

**Pourquoi E8 et E9 ne suffisent pas.** E9 compare deux instances de même histoire : un état caché hors de la forme canonique (la capacité d'un `Vec`, une adresse) y est le même deux fois. E8 recharge la projection mais n'y joue aucune entrée : une étape qui avale un absent n'est jamais exécutée sur une projection. Sur le téléphone, l'instance a une autre histoire (`Bonjour`, `Decrire`, des `Page`, des `Charger`, jamais `Creer`), et l'étape tourne sur une projection.

### 4.1 E22, « Prédiction » (dans l'épreuve, rapide)

Applicable si `prediction: Oui` ; sinon « sans objet : le jeu ne prédit pas ». Il tourne pendant les soirées que l'épreuve joue déjà. Il ne reprend pas les projections de E8 : E8 ne projette que tous les 30 pas ou à un changement de phase (`PERIODE_SECRETS`, `soiree.rs`, l. 77 et 2166). E22 appelle `Projeter` lui-même, aux seuls pas où il juge.

| Partie | Ce qu'elle fait | Échoue si |
|---|---|---|
| **(a) La projection suit l'étape** | Aux seuls pas finis `d` où le pas `d + 1` joue un geste de téléphone, pour chaque regard de téléphone `r` : appeler `Projeter(r)` sur l'autorité à `d`. La charger dans **l'instance de prédiction de ce regard**, une instance longue qui a déjà servi, elle-même, `Page` sur chaque vue que ce regard a reçue, et les pas jugés précédents (E21 tourne après les soirées, sur une seule instance : il ne sert pas ici). Y rejouer toutes les lignes du journal du pas `d + 1`, octet pour octet (`Appel::Entree` avec ordre et place, entrées synthétisées comprises). Comparer les issues, puis `Projeter(r)`, avec celles de l'autorité à `d + 1`. Un pas où une entrée rend `Attente` est compté, pas comparé. | une faute sur la projection (un jeu qui panique sur un appareil) ; une issue différente sans `Attente` ; une projection différente. Le rapport nomme la première entrée qui diverge, puis rejoue le même pas sur une instance neuve : il dit si l'écart dépend de l'histoire de la mémoire |
| **(b) L'oracle du rejeu** | Sur chaque fenêtre `(d, s]` qui contient au moins un geste de cet appareil et aucune ligne `Saut`, `Pause`, `Reprise` ni `Partie` : appeler `partie::prediction::predire` avec la base `d` (sa projection, son réveil, son saut en attente, son temps libre, lus dans le cœur), les gestes de l'appareil à leur vraie place, et les autres entrées du journal de la fenêtre à la leur (`Places::Vraies { quand, autres }`). Puis `Projeter(r)` sur la même instance. Une fenêtre où le rejeu attend ou sort est comptée, pas jugée. | la projection rendue diffère de `Projeter(r)` de l'autorité à `s`. On compare les projections, pas les vues : la règle 4 vide les `Definitif` d'une vue prédite, que la vue de l'autorité montre remplis, et l'état ne contient aucun `Definitif`. **Zéro fenêtre jugée donne « pas exécuté », jamais « réussi »** (CONTRAT, « L'épreuve ») |
| **(c) Couverture** | Compter, par variante de geste (le premier entier de l'encodage postcard, lisible sans le type du jeu), les gestes rejoués sans attente en (a). | aucun geste de téléphone joué sans attente sur toute la soirée principale : la déclaration ne prédit rien. Une variante qui attend toujours est une note, pas un échec |
| **(d) Taille** | La plus grosse projection des soirées et des pires cas. | au-delà de `PROJECTION_MAX` (Q9) |
| **(e) Coût** | Pour chaque fenêtre de (b), la somme du carburant de ce que le téléphone ferait : `Charger`, les entrées des gestes de l'appareil et des `Temps`, `Regarder`, `Page` de la vue prédite, coûts fixes compris ; les entrées d'autrui, que seul l'oracle rejoue, n'y comptent pas. Le constat (`Charger`, `Regarder`), à part, sur le même budget. Jugés par E19 (sorte « prédiction », Q23). | au-delà du budget |
| **(f) Une lecture ne change pas l'histoire** | La seconde exécution de E9 se joue avec une lecture `Projeter` par téléphone et par tour, comme l'autorité qui prédit. | journal ou résumé différents |
| **(g) Aucun secret dans l'état public** | Les valeurs secrètes que E8 sait cacher à `r` (`controles::secrets_caches`) se cherchent aussi dans les octets de la projection de `r`, pas seulement dans sa vue (CAT-46). | une valeur trouvée : un secret copié dans un champ public partirait sur le téléphone |

Le rapport dit aussi, sans seuil : la taille de l'état public que la vue ne montre pas (question 42), la part des pas comparés en (a), le nombre de fenêtres jugées en (b), et la durée de E22. Cette durée n'est pas estimée : (a) appelle `Projeter` deux fois et `Page` sur chaque vue, pour chaque regard ; elle se relève sur `predit` (lot 5a) **et sur Punchline** (lot 10).

**Pourquoi l'oracle rejoue les autres entrées.** Chez Punchline, le brouillon est un tenu, accepté jusqu'à 10 fois par seconde et par joueur, et le bot de l'épreuve tient un brouillon puis envoie dans le même coup (`epreuve.rs` du jeu, l. 397 à 410). Une fenêtre « seulement les gestes de cet appareil et des `Temps` », comme l'écrivait la première version, ne contiendrait presque jamais un `Envoyer` : (b) serait « pas exécuté », Punchline n'aurait pas de code 0, et le lot 10 ne finirait pas. L'oracle rejoue donc aussi les tenus et les gestes d'autrui. Il éprouve ainsi la machine du rejeu (l'ordre, le `Temps`, le réveil, le saut), ce qui est son but ; les places supposées, elles, se jugent par les statistiques de E23. **Ce que cela ne règle pas** : le brouillon d'un autre joueur lit son `Prive`, absent de la projection de `r` ; la règle 2 le fait attendre, et la fenêtre n'est pas jugée. Restent les fenêtres sans brouillon d'autrui, les plus courtes d'abord (un seul pas). Combien en reste-t-il chez Punchline : la porte du lot 0b les compte, et compte les pas comparés en (a), où le même brouillon d'autrui fait attendre.

**Les deux instances se complètent.** E8 repart exprès d'une instance neuve, pour qu'un état caché ne recopie pas un secret (revue de la phase 2, constat 1). E22 (a) prend l'autre risque : une étape qui dépend de l'histoire de la mémoire. Deux exécutions de même histoire (E9) ne la voient pas ; une instance qui a déjà servi des pages, si.

**E22 cherche les secrets avec la même règle que E8.** La projection de E22 porte le regard en tête (§ 2.4) : (g) le retire avant `controles::secrets_caches`, comme E8 depuis le lot 2b. Sans cela, (g) ne chercherait rien et resterait vert.

**E22 ne charge pas l'auteur des gestes qu'il ne prédit pas.** Par la règle 1, un geste non prévisible attend sur une projection : (a) ne le compare jamais. Une transition non prévisible qui avale un absent n'a rien à corriger.

**Ce qu'E22 ne voit pas, et c'est dit.** Un effet nommé par un compteur de l'état (au lieu de `quand.id`) ne diverge que si un autre joueur agit entre la base et le geste : ni (a) ni (b) ne le voient. E23 compte les effets prédits démentis ; la règle reste une règle d'écriture (INTERFACE, § 5.4).

### 4.2 E23, « Avec et sans prédiction » (par la vraie salle)

C'est l'étape 4 du CONTRAT : « Jouer chaque partie avec et sans prédiction, et comparer leur histoire définitive ». Applicable si `prediction: Oui`.

- **Il passe par la vraie salle** (SAL-10) : `salle::simulation`, la vraie `Autorite`, de vrais `Appareil`, la vraie `salle::page`, le vrai `salle::prediction`, sur wasmtime.
- **Il est lancé en sous-processus**, comme E20 : `soiree-banc predire <module> --json`, trouvé par `SOIREE_BANC` ou à côté de l'exécutable (`deux_moteurs.rs`, l. 123 à 139). `epreuve` ne tire ni `salle` ni `snow`. Sans le binaire, E23 est « pas exécuté », et un jeu qui prédit n'a pas de code 0. « À côté de l'exécutable », pour un test de `crates/epreuve/tests/epreuve`, c'est `target/…/deps`, où `soiree-banc` n'est pas : un test ne peut pas prendre `CARGO_BIN_EXE` d'un autre paquet. Le lot 8 construit donc `soiree-banc` (`cargo build -p banc`) et pose `SOIREE_BANC` pour ces tests. `soiree-epreuve <jeu> --catalogue` ne déposera plus un jeu qui prédit sans `soiree-banc` : `soiree/AGENTS.md` le dit au lot 8.
- **Deux soirées comparées, à la même graine.** Dans les deux, l'autorité envoie les projections : seul le prédicteur des téléphones est allumé ou éteint. Ainsi le réseau à graine tire les mêmes nombres, et les gestes arrivent aux mêmes instants. Couper aussi les projections changerait les messages, donc les tirages, donc les places : deux histoires différentes, légitimement. **La coupure lente n'y dépend pas d'une horloge murale** (§ 3.7) : sinon la charge d'une machine partagée ferait passer une page à la confirmée au hasard, et (b) échouerait à tort.
- **Les bots y jouent depuis la page confirmée** : les gestes sont les mêmes par construction.
- **Une troisième soirée, non comparée en (a)**, où les bots jouent depuis la page **montrée**, prédite quand elle l'est. Sans elle, aucun geste fait sur une page prédite n'est éprouvé : `vu` égal au pas confirmé, un champ qui n'existe que sur la page prédite, G2 sous un vrai doigt. On y juge qu'aucun tenu ni relâchement ne sort d'une page prédite, que chaque geste passe `Valider`, et que `vu` vaut le dernier pas confirmé reçu.

| Partie | Ce qu'elle vérifie | Échoue si |
|---|---|---|
| **(a) Même histoire** | Le journal définitif, la garde finale, et les émissions en clair de chaque appareil (gestes, tenus, retenus, numéros), comparés entre les deux soirées | une différence, même d'un tenu : une fuite de la prédiction vers le fil s'y voit |
| **(b) Convergence** | Après chaque `Projection` reçue, la page montrée est celle d'un `Predicteur` de référence neuf sur la même base et les mêmes gestes en vol. Sans geste en vol, c'est la page de `montre`, octet pour octet | un écart |
| **(c) Bornes** | Dans les trois soirées : aucun rejeu au-delà de `REJEU_MAX` ; aucun geste montré au-delà de `PREDICTION_MAX_US` ; aucun tenu ni relâchement sorti d'une page prédite ; aucun effet `Definitive` sorti d'une vue prédite. Dans la troisième : chaque geste passe `Valider`, et son `vu` vaut le dernier pas confirmé reçu | une borne franchie |
| **Statistiques**, sans seuil | Par geste prédit : « juste » si la projection après le rejeu (`Projeter(r)` sur l'instance de la page) égale la projection de la première base qui l'inclut, « corrigée » sinon (les changements d'autrui comptés à part, et dits). On compare les projections, pas les vues, pour la même raison qu'en E22 (b) : la règle 4. Puis les retours à la vue confirmée ; la durée de chaque page fausse ; les effets prédits démentis | jamais : « Les machines mesurent leur fréquence ; les joueurs jugent » (CONTRAT) |

Zéro geste prédit sur la soirée donne « pas exécuté ». Le rapport dit, par soirée, le nombre de pages prédites posées, de démentis, et les coupures avec leur cause.

Les pannes de E23 : celles de `salle::simulation` aujourd'hui (coupure, rechargement, onglet jumeau, autorité tuée et reprise, changement de jeu), plus le mode datagramme des trames remplaçables (perte, double, réordre), les rafales et les retenues (§ 8.3).

**Séparer l'oracle des statistiques est le point de méthode.** Une prédiction fausse parce que la place supposée diffère, ou qu'un autre a joué, n'est pas un bug : c'est le fonctionnement normal (CONTRAT). Un écart quand les suppositions sont justes en est toujours un, du jeu ou du commun : E22 (b) l'attrape.

### 4.3 E19 et E20

- **E19** gagne la sorte « prédiction » (Q23), lue dans E22 (e).
- **E20** pourra rejouer les appels de E22 dans Chrome et Firefox (lot optionnel 13). Safari reste « pas exécuté » (question 8). La correction n'en dépend pas : le constat de Q17 coupe la prédiction d'un moteur qui diverge.

### 4.4 Le droit de déclarer la prédiction

Un module qui déclare `prediction: Oui` n'a de code 0 au plan standard que si E22 réussit, en plus de tout le reste. E23 aussi, **tant que sa durée relevée au lot 8 le permet** : c'est la réponse provisoire de la question 38. Si elle ne le permet pas, E23 passe à la publication, comme E20 ; un code 0 ne dit plus alors E23, et le catalogue doit porter E22 et E23 à part (§ 3.2, Q7). Sinon, `prediction: Oui` fait échouer l'épreuve, comme toute déclaration non tenue. Le rapport retire alors la note de l'étape 4 (`rapport.rs`, l. 238 à 240) ; pour un jeu qui ne prédit pas, il dit « sans objet : le jeu ne prédit pas ». Les étapes 3, 5 et 6 restent différées : elles attendent le mainteneur (PHASE-3, § 9.3).

### 4.5 Les fautifs que l'épreuve doit refuser

Dans `crates/exemples/fautifs`, sur le socle, à un défaut près. Chacun doit faire échouer **son** contrôle, et lui seul (OUT-27) : c'est à constater aux lots 5a et 5b.

| Fautif | Défaut | Doit rougir |
|---|---|---|
| `predit` (sain) | le socle, `prediction: Oui`, `previsible(Taper) = Oui`, une vue de téléphone qui montre ses points, un `Definitif` | rien : il passe E22 et E23 |
| `capacite-cachee` | décide sur `Vec::capacity()` d'un champ de l'état : déterministe en mémoire, pas fonction de la forme canonique | E22 (a) ; **E9 doit réussir**, c'est ce que le fautif prouve |
| `adresse-du-tas` | décide sur la parité de l'adresse d'une `Box` | E22 (a). **Risque dit** : la copie native a d'autres adresses et peut le voir aussi ; s'il rougit deux contrôles, il est écrit comme tel |
| `predit-sans-effet` | chaque `Taper` lit le mot secret avant d'accepter : tout attend | E22 (c) |
| `projection-trop-grosse` | un champ public de 256 Kio | E22 (d) |
| `projection-qui-panique` | l'étape fait `unwrap` sur le mot secret au lieu de rendre `Absent` ; sur l'autorité il ne manque jamais | E22 (a), une faute sur la projection |
| `secret-dans-le-public` | copie le mot secret dans un champ public que la vue ne montre pas | E22 (g) ; E8 doit réussir |

Un absent avalé n'a pas de fautif : la colle le fait attendre (règle 2). Un test de `jeu` le tient (`un_absent_avale_attend`), avec son témoin : la même étape, la marque désactivée dans une copie de test de la colle, diverge. De même pour la règle 3 (`montrer_un_secret_absent_attend`).

Dans les tests de l'épreuve : un crochet qui décale d'un pas une entrée rejouée fait rougir (a) ; un autre qui donne une entrée de plus à la seconde exécution de E9 fait rougir (f) ; un autre qui fausse une heure de `Temps` dans l'oracle fait rougir (b). Chaque partie sait échouer seule.

### 4.6 Le monde simulé et la vraie salle (question 12)

La réponse provisoire de la question 12 était : « En phase 4, avec la prédiction ». Ce que fait la phase 4 (Q20) :

- **Le chemin de la prédiction passe par la vraie salle** : E23, par `salle::simulation` et `soiree-banc`.
- **Le code qui rejoue est celui de la soirée** : E22 appelle `partie::prediction`, la fonction même du téléphone.
- **E2 à E21 restent sur `epreuve::telephone`.** Les déplacer change ce que l'épreuve joue pour les trois jeux et demande leur plan standard ; et les constantes du monde simulé restent choisies tant que M5 n'est pas faite (RESTE-PHASE-3, § 3).

Si le mainteneur veut tout par la vraie salle : un lot de plus, après le lot 8, qui remplace `epreuve/src/telephone.rs` par `salle::simulation` (`RAPPEL_TENU` remplacé par la redite de `liaison`), et le plan standard des trois jeux (environ 25 minutes, RESTE-PHASE-3, § 5).

---

## 5. Le transport et la latence

### 5.1 Le transport (Q21)

**WebSocket seul pendant la phase 4.** C'est un écart au défaut de P11 (« WebRTC tranché par la mesure du Wi-Fi, en phase 4 par défaut ») : question 40.

- **La prédiction cache l'aller-retour sous le doigt, quel que soit le canal.** Le canal ne change que le temps pendant lequel une prédiction reste en vol, et la latence jusqu'à la télé.
- **Une perte sur TCP retient tout ce qui suit** (CONTRAT, « Le canal idéal n'existe pas partout »). Sur la page d'un jeu qui prédit, ce retard est justement caché. La prédiction rend WebRTC moins nécessaire aux jeux à page.
- **Les gestes sont des événements**, comptés une fois : un canal non fiable devrait les renvoyer, la même attente.
- **Rien de mesuré ne demande mieux.** Au téléphone natif sur la boucle locale, le transport fait 1,2 à 1,5 ms en médiane (`MESURES-PHASE-3.md`, « Lot 13 », M2). Le vrai Wi-Fi n'est pas mesuré (M5, humaine). SAL-65 donne 67 ms d'aller-retour au 99ᵉ centile sur un DataChannel, sur un partage de connexion, pas sur un Wi-Fi de salon.
- **WebRTC coûte** `str0m` en ICE-lite, un certificat qui signe, l'UDP au pare-feu (SAL-87, SAL-91, SAL-95), et un second chemin à éprouver.

**Ce qui est prêt pour plus tard.** `Projection` est un état entier, monotone, d'une trame : elle irait d'elle-même sur un DataChannel non ordonné. Les trames portent leur nonce (P11). `salle::simulation` éprouve dès la phase 4 la perte, le double et le réordre des trames remplaçables (§ 8.3).

**Ce qui changerait.** Si M5 montre la part du Wi-Fi au 99ᵉ centile au-delà des 30 ms que PHASE-3 (§ 3) laisse au téléphone, ou des retenues de TCP fréquentes, un lot conditionnel (14) ajoute le DataChannel non ordonné entre téléphone et relais pour les trames `Classe::Remplacable` : d'abord les tenus de Sidequest vers la télé, le vrai chemin critique, puis `Vue` et `Projection`. `salle` ne change pas.

### 5.2 La latence au pixel du téléphone (M16, Q22)

La `Trace` de la phase 3 mesure le geste jusqu'au pixel **de la télé** (PHASE-3, § 8.4). La prédiction ne la change pas : la télé attend toujours l'autorité. Il faut une mesure de plus, sur la seule horloge du téléphone : aucun écart d'horloge entre processus n'y entre (M14 sans objet).

```rust
// salle::trace, écrite par web/telephone.
pub struct TracePage {
    pub numero: u64,
    pub evenement_us: u64,              // `pointerdown`, event.timeStamp
    pub envoye_us: u64,                 // le geste parti (Q18)
    pub predite_image_us: Option<u64>,  // le rAF qui suit la page prédite posée
    pub confirmee_image_us: u64,        // le rAF qui suit la première page confirmée qui l'inclut
    pub dementie: Oui,                  // la page confirmée diffère de la prédite
    pub coupure: Option<Coupure>,
}
```

- **Le pixel est la page posée**, au `requestAnimationFrame` qui suit l'écriture du DOM, pas le photon. Comme pour la télé, la dalle et l'écran restent au matériel, à la caméra de la répétition.
- **Sans prédiction** : `confirmee_image_us − evenement_us`. Avant le lot 6, le banc joint chaque geste à sa `Trace.place`, lue sur la télé, et retient la première vue de pas au moins égal (lot 0a). Après, `inclus` le dit directement.
- **Avec** : `predite_image_us − evenement_us`. **Le gain** est la différence des deux distributions, dans le même passage. Médiane, 99ᵉ centile et maximum, jamais une moyenne (CONTRAT, « Les budgets »).
- **Où** : sur la boucle locale, puis à travers le mandataire de pannes du banc (16 à 66 ms par sens, rafales de 100 ms, retransmission de 200 ms : des valeurs choisies au lot 5 de la phase 3, en attendant M5). Dans Chrome, le banc peut brider le processeur de la page (`Emulation.setCPUThrottlingRate`, 4, choisi comme la marge de la question 22) ; Firefox n'a pas cette commande, et le rapport le dit.
- **Contrôle négatif** : `window.__banc.page_lente(33)` retarde la pose de 33 ms, ce que M16 doit voir.
- **Ce n'est pas M5.** REM-89 (« Un appui en retard de moins d'environ 60 ms passe inaperçu ») se compare à ces chiffres sans les valider.

### 5.3 La télé ne doit pas perdre

- **M2 bis** : M2 refaite au téléphone, prédiction allumée. Les segments `evenement` et `transport` ne doivent pas bouger : le geste part avant le calcul (Q18).
- **M13 bis** : la part du commun dans le Worker avec les projections, sous les 4,17 ms ; mesurée sans projection à 2,0 à 2,9 ms au 99ᵉ centile pour 71 appareils (`MESURES-PHASE-3.md`, « Lot 12, fin, correction »). Elle se mesure sur Sidequest à 71 téléphones, **et sur Punchline avec huit joueurs qui tapent** : chaque brouillon accepté rend les projections « à refaire » (§ 3.2), et c'est là que le calcul des projections pèse le plus souvent.

### 5.4 Les mesures nommées de la phase 4

| # | Ce qu'elle mesure | Où, quand |
|---|---|---|
| M16 | Le pixel du téléphone, sans puis avec prédiction | banc, Chrome et Firefox ; lot 0a (sans), lot 9 (avec) |
| M17 | La taille et le taux de changement des projections par regard ; le carburant de `Projeter`, `Charger`, `Entree` sur la projection, `Regarder`, du parcours de la règle 3 ; chez Punchline, les pas comparables en E22 (a) et les fenêtres jugeables en (b) | épreuve, lot 0b (Punchline après le lot 1) ; Punchline au lot 10 |
| M18 | Le coût d'un recalcul sur la page : carburant, ms, mémoire | banc, processeur bridé dans Chrome ; lot 9 |
| M19 | Les prédictions justes, corrigées, les retours, la durée de chaque page fausse | E23 et le banc ; lots 8 et 9 |

M2 bis et M13 bis se construisent au lot 9 et se mesurent au calme au lot 11 (`target/hote/lourd --calme`). Les vrais téléphones vont sur la fiche de répétition.

**Le budget.** Le CONTRAT ne donne de budget que jusqu'à la télé (80 ms au téléphone). Rien ne fixe le pixel du téléphone : question 45, avec pour réponse provisoire une mesure sans seuil.

---

## 6. Les jeux

| Jeu | Gagnerait-il à prédire ? | Ce qu'il changerait | Recommandation |
|---|---|---|---|
| **Punchline** | **Oui.** Sa page est toute l'interface du joueur. Après `Envoyer`, la consigne suivante (« Une consigne à la fois », son CAHIER) apparaît sous le doigt, et le joueur tape aussitôt ; elle est dans le `prive` du joueur, qu'il voit : la projection la porte. Son réveil est une échéance (`Reveil::A`, `lib.rs`, l. 201 à 206) : presque jamais de `Temps` rejoué. Mais son état change souvent pendant l'écriture : chaque brouillon accepté le change, jusqu'à 10 fois par seconde et par joueur. Les projections ne se calculent donc qu'au moment d'un envoi (Q10), et M13 bis se mesure sur lui (§ 5.3). Le gain porte sur la queue de la distribution : les rafales du Wi-Fi d'un téléphone vont jusqu'à une centaine de millisecondes (CONTRAT, « Ce que le réel impose ») | `prediction: Oui` ; `previsible` : `Envoyer` → `Oui`, tout le reste → `Non`. **`envoyer` ne prend le générateur que si la phase se termine** : `quelqu_un_attendu` d'abord (il ne lit à l'écriture que des champs publics), `hasard()` ensuite. Prendre le générateur sans tirer ne l'avance pas (PUN-27) : l'histoire de l'autorité reste la même, ce que le jeu vérifie (même résumé, même graine, avant et après). Le dernier `Envoyer` de la salle attend (`Attente`) : c'est juste, il change la phase de tous. Aujourd'hui, `envoyer` prend le générateur puis attend explicitement s'il manque (`Err(a) => return e.attendre(a)`, `regles.rs`, l. 411 à 414) : sur une projection, chaque `Envoyer` attend. `CAHIER.md` (l. 393) se corrige, avec sa raison. Le brouillon tapé dans le champ de la consigne suivante ne part qu'à la confirmation (§ 3.5) : le cahier le dit | **Le jeu de la phase 4**, si son agent et le mainteneur l'acceptent (question 34). `Voter` (il lit les auteurs, secrets, et tire un sel), `JeSeche` (il tire), `Personnage` et `Rejoindre` (disputés), les gestes du VIP (ils changent la phase de tous) restent `Non` |
| **Sidequest** | Peu. Son cahier : « les téléphones ne montrent pas le monde » (l. 410), et le téléphone ne reçoit « que son statut » (l. 409). Visible : le champ du message se vide sous le doigt. Mais la projection porterait tout le monde public (les positions des 71), changerait à chaque pas, et le téléphone rejouerait la physique de chaque `Temps` jusqu'à son geste. Un geste qui dépend du monde serait souvent démenti | Rien en phase 4. S'il le veut : `prediction: Oui`, `previsible` (`Message` → `Oui`), après M17 et M13 bis à 71 téléphones | **Optionnel** (lot 12) |
| **Tier list** | Sans objet. Pas de téléphone, et `prediction: Oui` demande `telephones: Oui`, accord vérifié en `const` (`colle.rs`, l. 275). Son glisser-déposer est déjà local à la page (`Deplacables`) ; ses entrées de l'écran tiennent leur budget dans Chrome (28,5 à 33,6 ms au 99ᵉ centile au clavier, RESTE-PHASE-3, § 4.2), pas dans Firefox avec sa fenêtre, où le GPU pèse (question 32) | Rien | — |

**Un jeu qui ne déclare rien ne voit rien changer à son comportement.** Pas de `Projection`, la `Vue` comme avant, E22 et E23 « sans objet », E8 et E19 inchangés. Ce qui change malgré tout :

- il prend le commun par chemin (`../../soiree/crates/jeu`) : chaque jeu se recompile contre la nouvelle `jeu`, son module change d'octets, donc d'empreinte. Il repasse l'épreuve pour rentrer au catalogue (P17), avec les mêmes résultats ;
- `salle::fil::PROTOCOLE` passe à 2 : une page de téléphone restée en cache dit « Version différente, recharge » (SAL-41) ;
- `Definitif::de` prend le regard : aucun des trois ne s'en sert ;
- la règle 3 vaut sur toute projection rechargée, même d'un jeu qui ne prédit pas : un `Temps` qui révèle un secret absent y attend. C'est ce qui change le test `projection_cache_le_nombre_et_le_hasard` de `devine` (lot 2b) ;
- le rapport dit « sans objet » à l'étape 4.

`CHANGELOG.md` le dit aux lots 2a et 2b.

**D'abord le socle.** Avant Punchline, le fautif sain `predit` prouve chaque pièce sur un module qui se compile en secondes.

---

## 7. Les lots de construction

**Règles de chaque lot.** Celles de PHASE-3, § 7. Chaque commande lourde passe par le portillon (`soiree/target/hote/lourd`), qui a deux places, sur une machine de 16 Go partagée. Jamais de `cargo test --workspace`. `cargo … -p <crate>`. Jamais de navigateur pendant une compilation ; un scénario du banc à la fois. Un commit par dépôt ; un jeu ne se pousse jamais sur un commun non étiqueté (`AGENTS.md` de l'hôte).

**Les branches.** Deux branches peuvent avancer ensemble quand leurs fichiers sont disjoints :

- **A**, la colle et l'épreuve : `crates/jeu`, `crates/jeu-derive`, `crates/exemples/fautifs`, `crates/epreuve` ;
- **B**, le rejeu, la salle et la page : `crates/partie`, `crates/salle`, `web/telephone`, `web/autorite`, `web/ecran`, `web/demarrage.js`, `crates/banc` (et `crates/relais` si la question 38 l'exige, lot 6).

**L'ordre.** Au départ, trois lots indépendants : **0a** (B), **0b** (A, sans Punchline) et **1** (dépôt de Punchline). La partie Punchline du lot 0b vient après le lot 1. Puis **A** : 2a → 2b → 4 → 5a → 5b → 5c ; **B** : 3 → 6 → 7a → 7b → 7c. Le lot 6 se prouve sur `partie::modele` : il n'attend pas `predit` ; le lot 5b attend le lot 3 ; le lot 7b attend les lots 2b et 4 (`predit`). Puis **8** (après 5b et 7c) et **9** (après 7c) côte à côte (fichiers disjoints du banc, mais pas deux compilations de `banc` en même temps). Puis 10 et 11. Les lots 12 à 14 sont optionnels ou conditionnels.

**Le lot 1 et la question 34.** Le lot 1 change les règles de Punchline sans changer son histoire (même résumé, même graine). Il ne sert qu'à la prédiction. Il se fait sous la question 34, ou comme un changement neutre que l'agent du jeu accepte d'après l'`AGENTS.md` du jeu. Sans l'un ni l'autre, la partie Punchline du lot 0b se mesure sur une copie de travail du jeu avec le lot 1, non commitée : c'est une mesure, pas un changement du jeu.

**Le plan court ou le plan standard.** Les lots 2a et 2b changent les octets de chaque module, pas ce qu'ils font sur l'autorité : le plan court des trois jeux suffit, au même verdict qu'avant. Le plan standard des trois jeux est exigé au lot 11, avant toute étiquette, puisque leurs empreintes ont changé ; celui des fautifs, au lot 5c.

**Les coûts.** Mesurés en phase 3 : `cargo test -p epreuve` 366 s et 1,50 Go (`LOTS-PHASE-3.md`) ; `tests/epreuve` 289 s (`AGENTS.md`) ; `cargo test -p banc` 79 s et 1,16 Go (`LOTS-PHASE-3.md`) ; `cargo test -p salle` 14 à 15 s et 0,28 Go au lot 4b, avant que `salle` grandisse (`LOTS-PHASE-3.md`). Relevés, non mesurés au calme : le plan standard des fautifs, 1 305 s pour 42 jeux le 2026-11-07, quatre fils, sous une charge de 2,2 à 2,4 (`soiree/AGENTS.md`) ; le lot 4 y ajoute sept jeux ; Punchline au plan standard 112 s et 1,73 Go, Sidequest 557 à 577 s (leurs `BESOINS-COMMUN.md`). Le reste est **estimé**, et chaque lot mesure le sien en premier.

| Lot | Contenu et fichiers | Ce qui le prouve fini | Compilation et mémoire (estimées sauf mention) |
|---|---|---|---|
| **0a. M16 sans prédiction** (B, au départ) | `web/telephone/src/page.rs` (journal `pointerdown` par numéro, page posée par pas de vue, `window.__banc.traces_page()`, `__banc.page_lente`) ; `crates/banc/src/mesures.rs`, `telephones.rs` (`mesures --pages`, la jointure par `Trace.place`) ; une section « Lot 0a » dans `conception/MESURES-PHASE-4.md` (nouveau) | M16 sans prédiction sur Punchline, Chrome et Firefox, sur la boucle locale et à travers le mandataire, sous `lourd --calme`, la règle de validité posée avant les tours ; le contrôle négatif vu | `web/telephone` pour `wasm32`, puis le banc (79 s, mesuré) ; passages au calme, les deux places |
| **0b. M17 avant de construire** (A ; `devine` et Sidequest au départ, Punchline après le lot 1) | `crates/epreuve/src/prediction.rs` (mesure seulement, derrière `--mesurer-prediction`), le crochet dans `soiree.rs` ; `conception/MESURES-PHASE-4.md` | Avec l'ABI 4 telle qu'elle est (`Entree` passe après `Charger`, `colle.rs`, l. 636 : à constater), pour chaque geste de téléphone du journal : l'issue sur la projection, l'égalité des projections après, la taille, le carburant de `Projeter`, `Charger`, `Entree`, `Regarder`, et les pages du tas après `Charger`, par variante de geste, au plan court, daté, avec sa commande. Chez Punchline en plus : les pas où (a) comparerait (sans `Attente` d'un brouillon d'autrui) et les fenêtres que (b) jugerait. **Porte** : la projection de Punchline au pire sous `PROJECTION_MAX` ; un recalcul de `Envoyer` (`Charger`, `Entree`, `Regarder`, `Page`) estimé sous le budget de Q23, le constat à part ; au moins une fenêtre jugeable par soirée chez Punchline. Sinon retour à Q9, Q23 ou au § 4.1 avant le lot 2a. Avant le lot 1, chaque `Envoyer` attend sur une projection (`regles.rs`, l. 411 à 414) : son coût n'y est pas mesurable, d'où l'ordre | `epreuve` en test ciblé ; trois plans courts ; une place |
| **1. Punchline : le hasard paresseux** (dépôt du jeu, au départ ; sous la question 34 ou accepté comme neutre) | `games/punchline/crates/punchline/src/regles.rs` (`envoyer` : `quelqu_un_attendu` d'abord) ; un test du jeu | Même résumé qu'avant sur la même graine ; plan standard au code 0 | plan standard de Punchline, 112 s et 1,73 Go (relevé du jeu) |
| **2a. La colle attend** (A) | `crates/jeu/src/colle.rs` (marques `projetee` et `predite`, règles 1 et 2) ; `secret.rs` (la `Cell` d'`Autorite`) ; `etape.rs` (la marque du tirage dans `Sorties`, `nouvelle_partie` comprise) ; tests dans `crates/jeu` et `crates/jeu-natif/tests` ; `CHANGELOG.md` (le changement de sens de `Charger` puis `Entree`, CONTRAT « Les versions »). La sortie de `Projeter` ne change pas | Sur l'autorité, les octets figés de `jeu-abi` inchangés, et le plan court de `devine` et des trois jeux au même résumé et au même verdict qu'avant ; sur une projection : un geste `Non` attend, un absent avalé attend, un tirage avalé attend ; chaque témoin sans sa marque diverge | `cargo test -p jeu -p jeu-natif`, petit ; **chaque module change d'empreinte** ; trois plans courts |
| **2b. Le regard dans la projection** (A, après 2a) | `crates/jeu/src/colle.rs` (le regard en tête de `Projeter`, relu par `Charger` ; règles 3 et 4) ; `secret.rs` (`Regard::predit`) ; `forme.rs`, `collections.rs`, `secret.rs` et `crates/jeu-derive` (la méthode cachée de la règle 3, « aucun » par défaut pour les feuilles) ; `scene.rs` (`Definitif::de(r, cle, valeur)`) ; `page.rs` (doc de `confirmee`) ; `crates/epreuve/src/controles.rs` et `soiree.rs` (le préfixe retiré avant l'alignement de E8) ; `crates/exemples/devine/tests/regles.rs` (`projection_cache_le_nombre_et_le_hasard` : le `Temps` qui révèle un nombre absent attend, exprès) ; `CHANGELOG.md` | Sur une projection : un secret révélé vide attend, une vue prédite vide ses `Definitif` ; chaque témoin sans sa marque diverge ; la sortie de `Projeter` relue par `Charger` (E8 vert) ; **`vue-qui-fuit` fait toujours rougir E8**, et `secrets_non_cherches` ne change pas chez `devine` ni chez les trois jeux ; le plan court des trois jeux au même verdict | `cargo test -p jeu -p jeu-derive -p jeu-natif -p devine`, petit ; `epreuve` en test ciblé ; trois plans courts |
| **3. Le rejeu** (B) | `crates/partie/src/prediction.rs` (nouveau : `Base`, `GestePredit`, `Places`, `predire`) ; `lecture.rs` (`Partie::projeter`, et une lecture du réveil, du saut en attente et du temps libre, aujourd'hui `pub(crate)` dans `coeur.rs`, l. 37 à 47) ; `partie.rs` (`heure_du_pas` public) ; `lib.rs` ; `modele.rs` (un réglage qui déclare la prédiction, une projection qui cache un champ à un regard, un geste qui le lit et attend sur une projection) ; `crates/partie/tests/prediction.rs` sur `partie::modele` | La propriété `la_prediction_exacte_rejoue_l_autorite` : sur des journaux tirés, `predire(Projeter(d), gestes, Places::Vraies)` égale `Projeter` de l'autorité à `s` ; l'ordre dans un pas ; `Temps` joué où `reveil_demande` le dit, pas quand le temps ne court pas ; le premier `Temps` porte le saut en attente ; une attente ou une sortie qui arrête la suite ; `REJEU_MAX` ; la place estimée égale celle de `regles::placer` quand l'estimation est exacte (l'erreur d'un pas de la première version y rougit). Contrôles négatifs : `TempsAvantLesGestes`, `HeureFausse`, `OublierLeTemps`, `OublierLeSaut` font rougir la propriété. Le modèle sert aux tests de `salle` : `cargo test -p salle` relancé, vert | `cargo test -p partie -p salle`, moyen |
| **4. Le socle qui prédit, et ses fautifs** (A, après 2b) | `crates/exemples/fautifs/src/defauts.rs`, `socle.rs`, `lib.rs` ; `modules/predit`, `capacite-cachee`, `adresse-du-tas`, `predit-sans-effet`, `projection-trop-grosse`, `projection-qui-panique`, `secret-dans-le-public` | Les modules se compilent et passent l'étape 1 ; `predit` passe les contrôles d'aujourd'hui ; `capacite-cachee` passe E9 | sept petits modules `wasm32` |
| **5a. E22 : l'étape et les secrets** (A, après 4) | `crates/epreuve/src/plan.rs` (`IdControle::E22`), `soiree.rs` (instance longue par regard, qui sert `Page` sur chaque vue reçue ; `Projeter` aux pas jugés ; (a), (c), (d), (g)), `controles.rs`, `rapport.rs` ; `prediction.rs` devient le contrôle ; `crates/epreuve/tests/epreuve/` (les fautifs de (a), (c), (d), (g), le crochet qui décale une entrée) | `cargo test -p epreuve --test epreuve` au code 0 : `predit` réussit (a), (c), (d), (g) ; `capacite-cachee`, `adresse-du-tas` (le risque constaté et écrit), `projection-qui-panique`, `predit-sans-effet`, `projection-trop-grosse`, `secret-dans-le-public` échouent leur partie seule ; le crochet rougit (a) ; les trois jeux au code d'avant, E22 « sans objet » ; la durée de E22 relevée sur `predit` | lourd : 289 à 366 s et jusqu'à 1,5 Go pour `epreuve` (mesuré en phase 3) ; une place |
| **5b. E22 : l'oracle, le coût, la lecture** (A, après 3 et 5a) | `soiree.rs` ((b), (e)), `determinisme.rs` (E9 avec `Projeter`, (f)), `budgets.rs` (`BudgetsE19::prediction`) ; `crates/epreuve/tests/epreuve/` (les crochets de (b) et (f)) | `predit` réussit E22 entier, avec au moins une fenêtre jugée en (b) ; les crochets rougissent (b) et (f) ; les fautifs de 5a toujours seuls en échec | comme 5a ; une place |
| **5c. Le plan standard des fautifs** (A, après 5b) | rien de neuf : `cargo test -p epreuve --test epreuve -- --ignored plan_standard` | le plan standard des fautifs repasse, 49 jeux, chacun sur son seul contrôle | 1 305 s pour 42 jeux (relevé sous charge, non mesuré au calme), plus sept ; une place |
| **6. Le fil et l'autorité** (B, après 3) | `crates/salle/src/fil.rs` (`PROTOCOLE` 2, `Projection` avec `reveil`, `saut`, `temps_libre`), `tests/fil.rs` (octets figés, seconde lecture, SAL-90) ; `autorite.rs` (`EnVol::place`, `inclus`, les regards « à refaire », la projection calculée à l'envoi, cadence, borne de trame, coupure, `ReglagesSalle::predire`, la permission du banc) ; `appareil.rs` (la réception de `Projection`, `montre` montrée) ; `web/autorite` (`predire` d'après `eprouve`), `web/ecran`, `web/demarrage.js` ; si la question 38 réserve E23 à la publication : `crates/relais/src/paquet.rs`, `jouer.rs` | `salle::simulation` sur `partie::modele` (lot 3) : projections reçues, `inclus` monotone, chaque geste inclus un jour, refusés à l'arrivée compris ; aucune projection calculée hors d'un envoi ; après une reprise, une `Projection` neuve même d'un pas plus bas ; `un_jeu_sans_prediction_ne_recoit_aucune_projection` (sur `devine` et `ardoise`, `tests/devine.rs`) et son négatif (forcer l'envoi : rouge) ; `la_projection_trop_grosse_repasse_aux_vues`. Ce qui demande `predit` passe au lot 7b | `cargo test -p salle`, moyen ; `web/` pour `wasm32` |
| **7a. Le prédicteur, sans page** (B, après 6) | `crates/salle/src/prediction.rs` (nouveau : `Predicteur`, `Coupure`, `ComptesPrediction`, `FautifPrediction`, `REJEU_MAX`, `PREDICTION_MAX_US`, la durée donnée de l'extérieur, l'accueil qui refait `en_vol`) ; tests sur `partie::modele` | G4 côté téléphone, G7 (sans page), G8 (dans une session ; à `(partie, pas)` égal, le plus grand `inclus`), G9 pour la reprise (`en_vol` refait depuis la file ; un geste inclus non écrit reprédit), la coupure lente sur des durées données ; chaque `FautifPrediction` de ces garanties rouge | `cargo test -p salle`, moyen |
| **7b. Les deux pages et les effets** (B, après 7a, 2b et 4) | `crates/salle/src/page.rs` (les deux pages, Q13 à Q15, les locaux gardés, le constat à part) ; `crates/salle/tests/salle.rs` ; ce que le lot 6 a laissé sur `predit` | G2, G6, G10, G11, G12 en natif sur `predit` sous wasmtime ; `predit` reçoit ses projections ; chaque `FautifPrediction` rouge pour sa raison ; `mille_predictions_ne_font_pas_grandir_la_memoire_sal186` | `cargo test -p salle` et le module `predit` ; moyen |
| **7c. Les pannes** (B, après 7b) | `crates/salle/src/simulation.rs` (mode datagramme des remplaçables, rafales, retenues ; `JoueurDePage` qui prédit, qui joue depuis la page confirmée ou depuis la page montrée) | G8 et G9 sous pannes ; `cargo run --release -p salle --example soiree_en_memoire -- predit --prediction` sous pannes donne ses démentis, et au moins une page prédite posée | `cargo test -p salle` ; moyen |
| **8. E23** (après 5b et 7c) | `crates/banc/src/predire.rs` (nouveau scénario, deux soirées comparées à graine égale et une troisième où les bots jouent depuis la page montrée, émissions comparées, coupure lente sans horloge murale, `--json`) ; `crates/epreuve/src/` (`IdControle::E23`, le sous-processus comme `deux_moteurs.rs`, `rapport.rs` : la note de l'étape 4 retirée) ; `crates/epreuve/tests/epreuve` (`SOIREE_BANC` posé, après `cargo build -p banc`) ; `soiree/AGENTS.md` (`soiree-banc` nécessaire à `--catalogue` pour un jeu qui prédit) ; le fautif du commun `--fautif-commun tenu-de-la-page-predite` | `predit` passe E23, avec des pages prédites posées dans chaque soirée ; chaque contrôle négatif du commun fait rougir sa partie ; les trois jeux gardent leur verdict (E23 « sans objet ») ; la durée de E23 relevée (question 38) | le banc (79 s, mesuré) ; `epreuve` (lourd) ; une place |
| **9. Le navigateur** (après 7c ; côte à côte avec 8) | `web/telephone/src/page.rs` (le prédicteur sur `FabriqueNavigateur`, `TracePage`, la coupure lente par `performance.now()`, `?predire=non`, `__banc.prediction()`, `__banc.fautif_prediction`) ; `crates/salle/src/trace.rs` ; `crates/banc/src/pages.rs`, `telephones.rs`, `mesures.rs`, `soiree.rs` (`--prediction avec,sans`, `--lent 4`, la permission du banc) | `soiree-banc soiree predit --pages chrome,firefox --prediction` vert, mandataire compris, **avec, par navigateur, au moins une page prédite posée (`predite_image_us`), le compte des démentis, et la liste des coupures avec leur cause** ; chaque `FautifPrediction` rouge dans le navigateur ; `soiree-banc soiree <jeu>` sur les trois jeux : zéro `Projection` reçue (G13) ; M16 avec et sans, M18 ; M2 bis et M13 bis construits | lourd : `web/` pour `wasm32` et `wasm-bindgen`, deux navigateurs ; passages au calme, les deux places |
| **10. Punchline prédit `Envoyer`** (dépôt du jeu, après 1, 8 et 9, et l'accord de la question 34) | `crates/punchline/src/lib.rs` (`prediction`, `previsible`), `CAHIER.md` (§ 10 et l. 393 amendés, le brouillon perdu entre la page prédite et la confirmation dit), `BESOINS-COMMUN.md`, `LEARNINGS.md` | Plan standard au code 0, **E22 et E23 réussis**, `Envoyer` couvert, des fenêtres jugées en (b), la durée de E22 relevée ; `soiree-banc soiree punchline --pages chrome,firefox --prediction` vert, avec le même témoin positif qu'au lot 9 ; M16 et M17 de Punchline | plan standard de Punchline (112 s, relevé), plus le banc |
| **11. Les mesures et ce qui reste** | `conception/MESURES-PHASE-4.md` (M16 à M19 ; M2 bis ; M13 bis sur Sidequest à 71 téléphones et sur Punchline avec huit joueurs qui tapent ; au calme) ; `conception/RESTE-PHASE-4.md` ; `conception/REPETITION-PHASE-3.md` (une ligne pour la prédiction sur de vrais téléphones) ; `README.md`, `AGENTS.md`, `CHANGELOG.md` du commun | Les mesures ont date, machine et charge ; le plan standard des trois jeux au code 0 depuis leurs sources ; ce qui reste est dit | environ 25 minutes pour les trois jeux (RESTE-PHASE-3, § 5) ; `--calme`, les deux places |
| 12. Sidequest prédit `Message` (optionnel) | son dépôt | M17 et M13 bis à 71 téléphones ; décision écrite | plan standard de Sidequest (557 à 577 s, relevé) |
| 13. E20 reprend la prédiction (optionnel) | `crates/moteur-commun/src/releve.rs`, `crates/epreuve/src/deux_moteurs.rs` | les appels de E22 rejoués dans Chrome et Firefox, identiques à wasmtime | E20, de 20 s à 2 min 40 par jeu au plan court (`MESURES-PHASE-3.md`, « Lot 3b ») |
| 14. WebRTC (conditionnel, après M5) | `crates/relais`, `web/telephone` | § 5.1 | lourd |

**Si la question 34 reste sans réponse**, la phase finit au lot 9 et au lot 11 sur le socle `predit`. Le lot 12 (Sidequest) n'a alors pas de preuve écrite : il demanderait d'abord M17 à 71 téléphones, et l'accord de son agent.

---

## 8. Vérifier sans humain

### 8.1 Où l'on vérifie

- **En natif, à graine fixe** : `jeu` (la colle), `partie` (le rejeu sur son modèle), `salle::simulation` (la vraie salle en mémoire), l'épreuve (E22, et E23 par `soiree-banc`).
- **Le banc, vrai relais** : les téléphones natifs (`JoueurDePage`, qui prédisent par la même page) traversent le mandataire de pannes : retard, rafales, retenue, coupure, fermeture lente. Le mandataire travaille sous TCP et ne perd pas une trame seule : la perte et le réordre des projections se jugent dans `salle::simulation`.
- **Le banc, vrais navigateurs** : les pages du téléphone dans Chrome et Firefox, en émulation tactile, M16 avec et sans. Safari : pas exécuté tant que la question 8 n'est pas réglée.

### 8.2 Chaque garantie, sa pression et son contrôle négatif

| Garantie | Pression | Contrôle négatif |
|---|---|---|
| G1. La prédiction ne décide jamais (l'autorité ne reçoit que des gestes) | Forme : `VersAutorite` ne change pas ; E23 (a) | `--fautif-commun tenu-de-la-page-predite` : E23 (a) rouge |
| G2. Une page prédite n'envoie ni tenu, ni retenu, ni relâchement ; un geste qui en part dit le `vu` confirmé et passe `Valider` | `une_page_predite_ne_relache_pas_rem23` ; E23 (c), dont la troisième soirée, où les bots jouent depuis la page montrée | `FautifPrediction::LacherSurLaPagePredite`, `TenuDeLaPagePredite` |
| G3. La projection suit l'étape | E22 (a) | `capacite-cachee`, `adresse-du-tas`, `projection-qui-panique` ; le crochet qui décale une entrée |
| G4. Le rejeu du commun est exact | E22 (b) ; `la_prediction_exacte_rejoue_l_autorite` | `TempsAvantLesGestes`, `HeureFausse`, `OublierLeTemps`, `OublierLeSaut` |
| G5. Un absent lu, un tirage, un secret révélé vide, un geste non prévisible arrêtent la prédiction | Forme (colle) ; tests de `jeu` et de `partie::prediction` | la marque désactivée dans une copie de test de la colle : diverge |
| G6. Pas de clignotement | `salle::simulation`, banc : la page montrée ne revient jamais à une vue qui n'inclut pas un geste en vol | `FautifPrediction::MontrerLaConfirmee` |
| G7. Ne pas tromper durablement : au repos, la page de `montre`, au plus une base après ; aucun geste montré plus de 1,5 s | E23 (b) et (c) ; `une_prediction_sans_confirmation_tombe_apres_la_coupure_p28` | `GarderLaPrediction`, `IgnorerInclus`, `SansLimite` |
| G8. Rien ne recule dans une session : une base périmée ne remplace pas une plus récente ; à `(partie, pas)` égal, le plus grand `inclus` gagne ; une session neuve repart de sa première base (Q28) | `salle::simulation`, projections réordonnées et doublées ; autorité reprise d'un pas plus bas | `AccepterUneBasePerimee`, `GarderLaBaseALAccueil` (toutes les projections de la reprise jetées : rouge) |
| G9. La prédiction se coupe, et la page reste juste | faute injectée, autorité qui cesse de projeter, page gelée puis reprise, rechargement avec des gestes en vol, onglet jumeau, autorité tuée et reprise avec un geste inclus mais pas encore écrit, changement de jeu | `projection-qui-panique` sur le téléphone : jamais de page blanche ni figée ; `OublierLaFileALAccueil` (le geste perdu par la reprise disparaît de la page) : rouge |
| G10. Le constat coupe un moteur qui diverge | `salle::simulation` avec une instance de page dont `Regarder` est faussé | `NePasConstater` : la page garde une vue fausse, E23 (b) rougit |
| G11. Pas de résultat prédit : `Definitif` vide, `Definitive` seulement confirmé | test de la colle ; test de `salle::page` (effets) ; E23 (c) | `JouerLesDefinitifsPredits` |
| G12. Une saisie survit à une prédiction démentie | `une_saisie_survit_a_une_prediction_dementie_sal165` | les locaux oubliés : rouge |
| G13. Un jeu qui ne déclare rien ne voit rien | `salle::simulation` sur `devine` et `ardoise` (les seuls modules que les tests de `salle` compilent, `tests/devine.rs`, l. 27 à 40) ; `soiree-banc soiree <jeu>` sur les trois jeux : zéro `Projection` reçue ; leur plan court identique, E22 et E23 « sans objet » | forcer l'envoi pour un jeu qui ne déclare rien : rouge |
| G14. Coût, taille et mémoire bornés | E22 (d), (e) ; `mille_predictions_ne_font_pas_grandir_la_memoire_sal186` | `projection-trop-grosse` ; un recalcul au carburant gonflé dans un test |
| G15. Aucun secret dans l'état public | E22 (g) | `secret-dans-le-public` |
| G16. La mesure mesure | M16 | `__banc.page_lente(33)` |
| G17. La télé ne perd rien | M2 bis, M13 bis | — (un rapport, pas une porte, comme M2) |

Les contrôles négatifs du téléphone sont une enum `FautifPrediction` de `salle::prediction`, comme `Fautif` dans `rendu` : compilée partout, posée seulement par le banc. Chacun doit faire rougir sa seule garantie. Un test né d'une leçon la cite dans son nom.

### 8.3 Les pannes provoquées

- **En mémoire** (`salle::simulation`) : `Pannes` gagne un mode datagramme des trames remplaçables (perte, double, réordre), `rafale_us` (un lien retenu puis livré d'un coup) et `retenue_us` (une perte TCP : tout ce qui suit attend la retransmission), à graine fixe (SAL-09). Le mode datagramme n'est pas le WebSocket d'aujourd'hui : il éprouve le remplacement en file du relais, qui perd déjà des projections, et le canal de demain.
- **Les pannes de la phase 3, rejouées avec la prédiction** : coupure, rechargement (les gestes en vol reviennent de `sessionStorage` ; ils ne se montrent que si le rechargement a duré moins de 1,5 s), onglet jumeau, autorité tuée puis reprise (`inclus` repart du registre ; la reprise repart d'un pas plus bas que la dernière base ; un geste inclus mais pas encore écrit revient par la file), changement de jeu.
- **Dans les navigateurs** : `soiree-banc soiree <module> --pages chrome,firefox --prediction`, à travers le mandataire. Un passage vert n'y vaut que s'il a posé au moins une page prédite par navigateur, et dit ses démentis et ses coupures avec leur cause : sinon un module sans prédiction, ou une coupure lente sous le bridage du processeur, le rendrait vert sans rien prouver.
- **Gestes concurrents** : `deux_telephones_predisent_le_meme_objet_le_second_est_dementi_cat41`, sur un geste que le socle déclare prévisible exprès pour ce test.

---

## 9. Les décisions et les questions

### 9.1 Les décisions provisoires

| # | Décision | Raison | Ce qui changerait sinon |
|---|---|---|---|
| Q1 | Seule la page du téléphone qui a fait le geste se prédit ; rien sur la télé | La télé est la page de l'autorité : un geste ne s'y montre pas avant d'arriver. Son retard est dans le dessin (le GPU de Firefox avec sa fenêtre, question 32), pas dans un aller-retour. Une instance de plus sur le fil de la page rouvrirait P1 | Un jeu à fenêtre, comme un FPS : la tête de l'autorité, et `lisse` dans `rendu` |
| Q2 | Ses gestes `previsible` seulement ; ni tenus, ni gestes des autres ; un geste disputé reste `Non` | CONTRAT : « Seules les conséquences du geste de l'appareil » ; les règles des tenus sont celles du cœur ; CAT-41, REM-126 | Prédire les tenus (un stick sur la page) : partager les règles de `partie::coeur` avec `prediction`, `inclus_tenu` dans `Projection`, et E22 (b) étendu aux tenus |
| Q3 | Le `Temps` rejoué où `reveil_demande` le demande, jusqu'à la place du dernier geste en vol ; jamais au-delà ; une sortie arrête | « Prédire, c'est exécuter la même étape » ; l'autorité jouera ce `Temps` avant le geste ; la seule extrapolation des autres est celle qui précède le geste (INTERFACE, question 5) | Aucun `Temps` : des prédictions fausses par construction dès qu'un réveil tombe ; ou jusqu'au pas estimé (INTERFACE § 5.3) : plus d'extrapolation des autres, plus de coût |
| Q4 | La prédiction tourne dans l'instance de la page : `partie::prediction` (le rejeu), `salle::prediction` (l'état) | Une mémoire de module au lieu de deux ; `Page` ne lit que ses arguments ; une seule définition pour le téléphone et l'épreuve | Le recalcul depuis un instantané neuf restauré, si le tas grandit (SAL-186) ; une seconde instance, si une faute de la prédiction gênait la page |
| Q5 | Pas d'ABI 5 : la colle connaît l'enveloppe projetée, avec ses quatre règles (§ 2.4) ; `Recue::confirmee` reste `Oui`, même sur une vue prédite : un écart au CONTRAT et à PHASE-3 (§ 9.3, question 43) | Une ABI neuve fait repasser les trois jeux au plan standard pour rien ; aucune forme d'appel ne change ; la colle décode déjà le geste et peut lire `previsible` elle-même | `Appel::Prevoir` et `Appel::Page { confirmee }` dans une ABI 5 (« reel »), ou par la fin avec `ABI_MIN` (« correct »), réunie avec D1, D3, D9 et D11 des jeux (RESTE-PHASE-3, § 6.2) |
| Q6 | `Definitif::de(r, cle, valeur)` ; le regard porte la marque `predit` | Aucun autre chemin ne mène de l'enveloppe à `Definitif::de` ; aucun jeu ne s'en sert | Une marque globale dans la colle, lue par `Definitif::de` : un état hors des arguments, contre l'enfermement |
| Q7 | Projection seulement pour un module éprouvé qui déclare `prediction: Oui` : un code 0 du plan standard dit alors E22 et E23 (§ 4.4), et `eprouve` du catalogue suffit ; au banc, une permission refusée hors de `--banc` ; jamais sous `--non-eprouve` ; `version_jeu` gardée à partir de la première étiquette qui porte cette colle | L'épreuve constate ; le catalogue ne dit aujourd'hui que `eprouve` (`relais/src/paquet.rs`, l. 296 à 306) ; la version ne protège que d'un module d'une étiquette plus ancienne, qui n'existe pas encore (question 19) | Si E23 passe à la publication (question 38) : le catalogue porte E22 et E23 à part. La prédiction sous `--non-eprouve`, affichée ; ou une marque de colle dans une ABI 5 |
| Q8 | `Projection { pas, partie, appareil, ouverture_pas, ouverture_heure, inclus, reveil, saut, temps_libre, montre, projection }`, une trame, `CANAL_VUE`, qui **remplace** `Vue` pour un lien qui prédit ; `PROTOCOLE` 2 ; rien ne remonte | Vue et projection atomiques, un seul flux, aucune inversion possible dans la file du relais (`relais/src/file.rs`) ; l'heure par la même fonction que l'autorité | Deux flux (« reel ») : un canal à part et une règle pour leur inversion ; un `VersAutorite::Prediction(Oui)` si les octets reçus par un téléphone qui a coupé gênent la batterie |
| Q9 | `montre`, `projection` et l'entête tiennent une trame : `PROJECTION_MAX = TRAME_MAX − limites.vue − MARGE_TRAME` (4 Kio, choisi) ; sans morceaux ni compression | Une trame de 256 Kio (`liaison`) ; rien ne compresse dans `salle` ; un état en morceaux n'est pas remplaçable ; Punchline relève des formes canoniques d'environ 1 Kio | Des morceaux comme `Module`, ou `miniz_oxide` dans `salle`, si M17 trouve une projection trop grosse |
| Q10 | Une `Projection` part à l'accueil, puis tout de suite si `montre` ou `inclus` change ; si seul l'état a changé, au plus toutes les 100 ms par lien (choisi) ; la projection d'un regard ne se calcule qu'au moment où un envoi est dû, et seulement si un tour a accepté une entrée depuis la dernière | Borne l'envoi **et le calcul** d'un monde qui change à chaque pas (Sidequest) ou d'un brouillon accepté jusqu'à 10 fois par seconde et par joueur (Punchline), sans retarder une confirmation ; un tour sans acceptation ne change pas l'état | À chaque changement, si M17 et M13 bis le permettent |
| Q11 | `inclus` par onglet : le plus grand numéro dont tous les précédents sont joués à un pas au plus `pas`, ou refusés à l'arrivée | L'accusé est durable, donc tardif (environ 100 ms plus M11) ; un refus à l'arrivée ne produit aujourd'hui aucun message | Réconcilier sur l'accusé : des prédictions montrées une centaine de ms de plus |
| Q12 | La place estimée par `regles::placer`, `recu` estimé à `fait + demi-aller-retour`, le définitif estimé au moins `base.pas` ; `vu` reste le pas de la vue confirmée | Une seule règle de placement ; l'autorité borne `vu` par ses vues produites | Une estimation apprise des écarts entre places prédites et réelles (M19) |
| Q13 | Vue prédite tant qu'un geste est en vol et que le rejeu l'inclut ; jamais la confirmée d'une base qui ne l'inclut pas ; rien ne recule derrière la confirmée dans une session ; correction d'un coup, sans lissage ; rien sous un écran du commun (« Un instant… », « Remplacé », le chargement) | Pas de clignotement par construction ; un écran discret ne se lisse pas. La pause n'est pas branchée sur la salle aujourd'hui : aucun appel à `Partie::pause` dans `salle` ni dans `web/`, et l'autorité n'envoie aucune `Attente` ; elle ne se montre donc pas par un écran du commun. Le temps arrêté (pause ou panne du cœur) se lit dans `temps_libre` (Q27) | Une durée minimale avant correction (REM-117), si M19 mesure des pages fausses souvent montrées moins de 100 ms |
| Q14 | Tenu, retenu et relâchements sortent de la page confirmée seule ; une page prédite qui retire une clé tenue ne se montre pas | « La prédiction ne décide jamais » vaut aussi sur l'appareil (REM-23, `page.rs`, l. 419) | Une page prédite qui envoie un relâchement |
| Q15 | `Provisoire` seuls d'une vue prédite ; `Definitive` seulement confirmés ; une annulation empêche de jouer, ne coupe pas : un écart au CONTRAT, « un effet prédit à tort s'annule » (§ 9.3) | REPONSES-PROVISOIRES 17 et question 25 ; le téléphone ne joue que la vibration | Couper un effet parti (`navigator.vibrate(0)`), et en phase 5 un motif de vibration |
| Q16 | Coupure : une faute, trois recalculs de plus de 8 ms (choisi ; la durée donnée de l'extérieur, déduite du carburant dans `salle::simulation` et E23), `REJEU_MAX` 60 entrées (choisi), un geste de plus de `PREDICTION_MAX_US` = `COUPURE_US` (1,5 s), un constat raté, l'arrière-plan ; jamais l'âge de la base | La voie de secours du CONTRAT, sans réglage par le jeu ; une base dont l'état n'a pas bougé reste exacte | Des seuils mesurés sur de vrais téléphones (fiche de répétition) ; une valeur par jeu |
| Q17 | Le constat : à l'arrivée d'une base, à part du recalcul, `Regarder` après `Charger` doit égaler `montre` ; sinon coupure jusqu'au module suivant, comptée ; une base sur deux, ou la première de chaque module, si M17 le demande | E8 constaté en soirée ; un moteur qui diverge (JavaScriptCore, jamais éprouvé) devient une coupure, pas des démentis inexpliqués | Pas de constat : un `Regarder` de moins par base, et des démentis que rien n'explique sur Safari |
| Q18 | Le geste part avant le calcul de la prédiction | Ne pas retarder la télé sur un téléphone lent | Une latence vers la télé qui grandit avec la prédiction |
| Q19 | E22 dans l'épreuve, E23 par `soiree-banc` en sous-processus ; E22 exigé pour déclarer la prédiction, au plan standard ; E23 aussi, tant que sa durée relevée au lot 8 le permet (question 38) ; sans objet sinon | La garantie de résultat (MANIFESTE) ; `epreuve` ne tire pas `salle` ; E23 est natif, et sa durée n'est pas encore relevée | E23 pour publier seulement, comme E20 (question 38) |
| Q20 | Le chemin de la prédiction passe par la vraie salle (E23) ; E2 à E21 restent sur `epreuve::telephone` | SAL-10 et la question 12, sans repasser les trois jeux ; M5 d'abord | Tout migrer maintenant (§ 4.6) |
| Q21 | WebSocket seul en phase 4 | § 5.1 ; M5 manque | Le lot 14 ; P11 tel quel (question 40) |
| Q22 | Le pixel du téléphone se mesure sur sa seule horloge (`TracePage`), sans prédiction d'abord | Aucune incertitude de M14 ; le gain se lit dans le même passage | Une trace qui traverse deux horloges |
| Q23 | E19 juge la sorte « prédiction » : la somme des appels d'un recalcul (`Charger`, les entrées, `Regarder`, `Page` de la vue prédite), coûts fixes compris, sous `2 × BudgetsE19::page` (2 604 166 unités) ; le constat à part, sous le même budget | La moitié d'une image à 60 Hz sur un téléphone quatre fois plus lent, comme la coupure de 8 ms (calculé ; marge choisie, question 22). Chez Punchline, le recalcul de `Envoyer` n'y tiendrait, au mieux, que sans `Etat` ni constat dans la même image (§ 3.9, calculé ; à relever au lot 0b) | Chaque appel sous `BudgetsE19::page` seul ; ou un budget tiré des vrais téléphones (question 39) |
| Q24 | Punchline prédit `Envoyer` seul, après le socle `predit` ; Sidequest optionnel (`Message`) ; Tier list sans objet | § 6 | Q24 tombe si l'agent de Punchline refuse : la phase finit sur le socle, Sidequest devient le candidat |
| Q25 | Pas de fenêtre : `fenetre` reste `SansObjet` | Aucun jeu ne la déclare ; la prédiction sur le téléphone n'en dépend pas | Une fenêtre chez Punchline : la tête rejouée dans le Worker, E12 armé, `(fenêtre + 2) × pas` sous une image (E19) |
| Q26 | La base n'est jamais gardée dans `sessionStorage` ; les gestes non accusés, si (P26), et rentrent dans `en_vol` ; ils ne se montrent que si le rechargement a duré moins de 1,5 s | La base porte les secrets du joueur, et serait périmée au retour | Une base gardée : une prédiction plus tôt après un rechargement, des secrets sur le disque du téléphone |
| Q27 | La `Projection` porte ce que le cœur sait et que la projection ne dit pas : `reveil` (`EtatCoeur::reveil`), `saut` (le temps jeté en attente, `coeur.saut`) et `temps_libre` (ni pause ni panne du cœur). Le rejeu n'appelle pas `Appel::Etat` | Sans le saut, le téléphone rejoue des `Temps` à saut 0 sur des pas que l'autorité n'a pas joués (`pas.rs`, l. 377 à 381, 630 et 1102) ; sans le temps libre, il rejoue un `Temps` que le cœur arrêté ne joue pas (l. 267) ; `Etat` ferait tourner `attente()` et `demandes()` du jeu sur le téléphone (`colle.rs`, l. 1007 à 1047), et coûterait un appel de plus par recalcul | `Appel::Etat` dans chaque recalcul : un appel et du code de jeu de plus sur le téléphone ; ou une pause branchée sur la salle avec un écran du commun, si le mainteneur veut une pause visible |
| Q28 | À chaque `Accueil` (une session neuve), la base tombe et `en_vol` se refait depuis la file des gestes non accusés ; la première `Projection` de la session est prise quel que soit son pas ; ensuite, la fraîcheur se compare en `(partie, pas)`, puis `inclus` à pas égal | Les vues partent avant que leur case soit écrite (`autorite.rs`, l. 2050 puis 2060) : une autorité reprise repart d'un pas plus bas, et un geste inclus mais pas encore écrit disparaît de sa définitive ; l'appareil le renvoie depuis sa file (SAL-56), le prédicteur doit le reprédire (CAT-41) ; un refus à l'arrivée fait monter `inclus` sans changer le pas | Comparer `(partie, pas)` à travers les sessions : après une reprise, toutes les projections jetées et la page figée sur la confirmée ; ne pas refaire `en_vol` : le geste perdu disparaît de la page jusqu'à son renvoi |

### 9.2 Les questions au mainteneur

Chacune a sa réponse provisoire, retenue pour avancer. Elles continuent la numérotation de RESTE-PHASE-3, § 6.1.

| # | Question | Réponse provisoire |
|---|---|---|
| 34 | Punchline prédit-il `Envoyer` ? Son cahier (l. 393) dit « ni prédiction » ; c'est la décision de son humain. Deux choses viennent avec : (a) le lot 1 change ses règles (le hasard paresseux) sans changer son histoire, et ne sert qu'à la prédiction ; (b) le texte tapé dans le champ de la consigne suivante, qui n'existe que sur la page prédite, ne part en brouillon qu'à la confirmation : si l'échéance tombe entre les deux, il ne compte pas (§ 3.5) | Oui, `Envoyer` seul, par son agent, cahier amendé, la perte (b) dite dans le cahier ; le lot 1 sous cette question, ou accepté comme neutre par l'agent du jeu. Sans réponse, la phase finit sur le socle `predit` : la fin « un jeu prédit vraiment » n'est pas atteinte, et Sidequest, candidat de rechange, n'a pas de preuve écrite (lot 12) |
| 35 | INTERFACE, question 5 : le téléphone joue-t-il le temps des autres ? | Le `Temps` où le réveil le demande, jusqu'au dernier geste en vol, pas au-delà (Q3) |
| 36 | Les écarts à INTERFACE § 5.3, au CONTRAT et à PHASE-3 (§ 9.3) : le tenu non prédit (même une carte qu'on glisse), pas de lissage, le rejeu jusqu'au dernier geste, `montre` dans la projection, une page qui ne sait pas qu'elle est prédite, un effet parti qui ne s'annule pas | Les accepter pour la phase 4 |
| 37 | REPONSES-PROVISOIRES 17, question 25, et le CONTRAT (« un effet prédit à tort s'annule ») : une annulation coupe-t-elle un effet parti ? | Non, elle empêche seulement de le jouer (Q15) : un écart au CONTRAT, à accepter ou à refuser |
| 38 | E22 et E23 exigés pour jouer, ou seulement pour publier ? | Pour jouer : E22 ; E23 aussi, si sa durée relevée au lot 8 le permet, sinon pour publier comme E20. La réponse décide si `eprouve` suffit au Worker (§ 3.2, Q7) ou si le catalogue doit porter E22 et E23 |
| 39 | Le budget de E19 pour la prédiction sur un téléphone, prolongement de la question 22 | Deux pages : la moitié d'une image d'un téléphone quatre fois plus lent (Q23) |
| 40 | Écart à P11 : WebRTC n'entre pas en phase 4 par défaut, il attend M5 | Oui, il attend M5 |
| 41 | Les valeurs choisies (annexe) : `MARGE_TRAME`, la cadence de 100 ms, `REJEU_MAX`, 8 ms, 1,5 s | Choisies ; M17 à M19 et la fiche de répétition les remplacent |
| 42 | Tout l'état public part sur le téléphone qui prédit, pas seulement sa vue : la question 18 s'aggrave (un invité actif qui sert une page modifiée lit plus) | L'accepter, le dire au jeu (rapport de E22, `CHANGELOG.md`), et en faire une question de l'examen (étape 3) quand elle aura sa forme |
| 43 | L'avis d'un refus donné à la page (D3 des jeux), et `Recue::confirmee` à `Non` : avec la prédiction ? Sans lui, la page reçoit une vue prédite marquée confirmée (`colle.rs`, l. 875), contre le CONTRAT et PHASE-3, § 5 | Dans l'ABI 5 qui réunira les demandes des jeux, après la répétition ; d'ici là, un démenti ne dit pas sa cause, et ce qui doit attendre la confirmation passe par `Definitif` |
| 44 | La prédiction sur Safari iOS sans E20 dans Safari (question 8) | Permise : le constat (Q17) coupe un moteur qui diverge, et la vue confirmée corrige toujours ; à fermer avec la question 8 |
| 45 | Un budget pour le pixel du téléphone ? Le CONTRAT n'en a que jusqu'à la télé | Aucun : une mesure (M16), et une proposition après la répétition |

**Les questions de RESTE-PHASE-3 dont la phase 4 dépend**, et ce qui changerait :

- **12 (le monde simulé)** : la phase en dépend vraiment, et la suit en partie (Q20). « Tout par la vraie salle » ajoute la migration de E2 à E21 et un plan standard des trois jeux. « Jamais par la vraie salle » ferait juger E23 sur une copie : la prédiction s'écrirait une seconde fois dans `epreuve/src/telephone.rs` (SAL-10).
- **22 (le budget de `Page`)** : il fixe celui de la prédiction (Q23). Avec les relevés de Punchline, un recalcul de `Envoyer` dépasse très probablement `2 × page` tant que `Etat` et le constat sont dans la même image (§ 3.9). Une autre réponse change la porte du lot 0b, pas la conception.
- **30 (le saut d'un pas)** : elle touche aussi la prédiction. Un saut change le rejeu : le `Temps` qui suit porte le temps jeté (Q27), et les pas sautés ne sont pas joués. Plus il y a de sauts, plus il y a de prédictions fausses, corrigées par la base suivante ; l'oracle de E22 exclut leurs fenêtres. Demander aux jeux un `rattrapage` d'au moins 2 réduirait ces sauts, sans rien changer à la conception.
- **38 (E23 pour jouer ou pour publier)** : elle décide si `eprouve` suffit au Worker ou si le catalogue doit porter E22 et E23 (§ 3.2, lot 6).
- **25 (les effets provisoires)** : question 37.
- **8 (Safari)** : les iPhone d'une soirée prédiront dans JavaScriptCore, où E20 n'a jamais tourné. La correction n'en dépend pas (Q17) ; le taux de prédictions justes, si.
- **19 (l'étiquette)** : Punchline ne se pousse qu'une fois le commun de la phase 4 étiqueté ; la garde de version (Q7) part de cette étiquette.
- **1 (la page dans le module, P19)** : la prédiction suppose le module sur le téléphone. Sans P19, il faudrait l'y envoyer pour la seule prédiction.
- **7 (la coupure)** : `PREDICTION_MAX_US` reprend `COUPURE_US` ; une autre réponse le ferait bouger avec elle.
- **18 (la page en `http`)** : aggravée (question 42).
- **6 (les navigateurs)** : le banc juge Chrome et Firefox.
- **24 (la durée des vérifications)** : la durée de E22 n'est pas estimée (`Projeter` deux fois et `Page` par regard aux pas jugés) : relevée sur `predit` au lot 5a et sur Punchline au lot 10 ; E23, trois soirées par la salle, à relever au lot 8.
- **M5** (humaine) : suspend WebRTC (§ 5.1), pas la prédiction.

Les questions 31 à 33 (point de Tier list, pixel de Firefox, gestes sous l'écran du commun) ne concernent que la télé. *Corrigé à la relecture* : la première version y mettait la question 30, en disant qu'un pas sauté fait au plus une place estimée fausse d'un pas. C'est faux : le saut change le `Temps` rejoué (voir plus haut).

**Les questions qui restent hors de la phase 4.** La question 26 (le bac à sable des `build.rs`, « en phase 4 au plus tôt ») : la prédiction n'en dépend pas, et la phase 4 ne la traite pas ; elle reste ouverte. La question 2 (le service en ligne, « sans objet jusqu'à la phase 4 ») : le mainteneur a écarté le relais en ligne aujourd'hui ; la prédiction n'en dépend pas, elle reste sans objet.

### 9.3 Les écarts à INTERFACE et au CONTRAT, à relire

| Où | Ce qu'il dit | Ce que fait la phase 4 |
|---|---|---|
| INTERFACE, § 5.3 | « son propre tenu à s » | Le tenu n'est pas prédit (Q2) |
| INTERFACE, § 5.3 | le rejeu « au pas estimé » | Jusqu'à la place du dernier geste en vol (Q3) |
| INTERFACE, § 5.3 ; CONTRAT, « Réagir sans attendre » | « Une correction se lisse, nœud par nœud » ; « le lissage des corrections » | La page en flux se remplace d'un coup ; `lisse` reste non dessiné (Q13) |
| INTERFACE, § 5.2 | « prédit : sa projection de la définitive, et le module » | La projection et la vue confirmée, d'un seul message (Q8) |
| INTERFACE, § 5.3 | « Un `Definitif<T>` ne montre sa valeur que si elle vient d'une projection confirmée » | Tenu ; `Definitif::de` prend le regard (Q6) |
| CONTRAT, « Réagir sans attendre » | « Seules les conséquences du geste de l'appareil s'affichent avant confirmation » | Tenu pour les gestes ; le `Temps` rejoué jusqu'au geste extrapole les autres sur ces pas (question 35) |
| PHASE-3, P11 | WebRTC « en phase 4 par défaut » | WebSocket seul jusqu'à M5 (question 40) |
| CONTRAT, « La page téléphone » ; PHASE-3, § 5 | « Elle reçoit ce qu'elle doit montrer, prédit ou confirmé » ; « `Recue::confirmee` existe : la page ne changera pas » | Une vue prédite passe à la page avec `confirmee: Oui` (`colle.rs`, l. 875) : la page ne peut jamais savoir qu'elle est prédite (Q5, question 43) |
| CONTRAT, « Réagir sans attendre » | « un effet prédit à tort s'annule » | Un effet `Provisoire` parti ne se coupe pas ; l'annulation empêche seulement de le jouer (Q15, question 37) |
| CONTRAT, « La forme d'un jeu » | « une carte qu'on glisse sur un téléphone, est prédit par le commun » | Aucun tenu ne se prédit (Q2) : une carte glissée sur un téléphone n'est pas prédite par le commun en phase 4 (question 36) |

Un écart interne est corrigé : la première version exigeait E23 sans condition au § 4.4, alors que la réponse provisoire de la question 38 le rend conditionnel à sa durée. Le § 4.4 et Q19 suivent désormais la question 38.

---

## 10. Ce que la phase 4 laisse à la phase 5, l'app iPhone

- **Le message est en Rust**, dans `salle::fil` : ses liaisons Swift se généreront (CONTRAT, « Les types de bout en bout »). `Projection` porte `montre` : une app qui ne prédit pas s'en sert telle quelle.
- **Le rejeu et l'état du téléphone sont en Rust**, dans `partie::prediction` et `salle::prediction`, sans DOM ni JavaScript : l'app les appellera tels quels.
- **Le moteur.** L'app ne compile pas à la volée (REM-81). Pour prédire, elle fera tourner le module entier, pas seulement `Page`, dans une vue web, ou par un interpréteur qui implémente `moteur_commun::Machine`. Cet interpréteur devra entrer dans E20 avec les appels de la prédiction (lot 13). Un interpréteur est lent : la coupure lente (Q16) le couvre sans rien changer d'autre.
- **JavaScriptCore** entre dans E20 avec la question 8 : la prédiction de l'iPhone y tourne.
- **Les vibrations** sont la raison de l'app (REM-79). Un effet `Provisoire` prédit fait vibrer ; l'annulation (question 37) devra peut-être couper un motif (REM-129).
- **Une page prédite reste un `PageFil`** : des intentions, que l'app rend en natif comme la page confirmée.
- **La question 18** (la page en `http` sur un Wi-Fi partagé, aggravée par la question 42) se ferme avec l'app, pas avec la prédiction.

---

## Annexe. Les valeurs choisies

| Valeur | Où | Choisie | Ce qui la remplacera |
|---|---|---|---|
| `MARGE_TRAME` | `salle::autorite`, E22 | 4 Kio, sous `TRAME_MAX` | la taille réelle de l'entête et du chiffrement, puis M17 |
| `PROJECTION_MAX` | `salle::autorite`, E22 | `TRAME_MAX − limites.vue − MARGE_TRAME` (188 Kio chez Punchline, calculé) | M17, ou des morceaux |
| Cadence des projections | `salle::autorite` | 100 ms par lien au plus, sauf `montre` ou `inclus` qui changent ; la projection ne se calcule qu'à l'envoi | M17, M13 bis |
| `REJEU_MAX` | `partie::prediction` | 60 entrées par recalcul | M18 sur de vrais téléphones |
| `PREDICTION_MAX_US` | `salle::prediction` | 1,5 s (`COUPURE_US`) | M5, et la répétition |
| Recalcul trop lent | `salle::prediction` | 8 ms murales, trois fois de suite ; la durée donnée par la page, déduite du carburant dans `salle::simulation` et E23 | M18 sur de vrais téléphones |
| `BudgetsE19::prediction` | `epreuve::budgets` | `2 × BudgetsE19::page` | M17, la question 22, la fiche de répétition |
| Bridage du processeur de la page | banc, Chrome | 4, comme la marge de la question 22 | de vrais téléphones |
| Pannes du banc et de la simulation | mandataire, `salle::simulation` | 16 à 66 ms par sens, rafales de 100 ms, retransmission de 200 ms, perte d'une trame fiable sur 40 (déjà choisies en phase 3) | M5 |
