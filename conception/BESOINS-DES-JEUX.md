# Les besoins des jeux, et la réponse du commun

> **Statut.** Réponse du commun (`soiree/crates`) aux besoins que les trois jeux reconstruits pour la phase 1 ont écrits dans `games/<jeu>/BESOINS-COMMUN.md`, le 2026-11-04. Chaque besoin est **accordé** (implémenté, avec ses tests, et son contrôle dans l'épreuve quand c'est une loi), **refusé** (avec sa raison tirée du contrat), ou **différé** (refusé pour la phase 1, avec ce qui le rouvrira). Rien n'est committé : le mainteneur relit, puis committe dans `soiree/`.
>
> **Ce qui fait foi.** [`MANIFESTE.md`](../MANIFESTE.md) et [`CONTRAT.md`](../CONTRAT.md), puis [`INTERFACE.md`](INTERFACE.md). Un besoin qui revient dans deux jeux pèse plus lourd (MANIFESTE, « Une leçon monte d'un étage quand elle revient ») : le besoin 1 revient dans les trois.
>
> **Mesures.** Chaque chiffre de carburant ci-dessous est mesuré le 2026-11-04 sur un Mac Apple Silicon (aarch64-apple-darwin), rustc 1.99.1, par `soiree-epreuve` (le carburant est compté par la réécriture : il est déterministe). Les modules des jeux mesurés sont ceux que leurs agents ont livrés (punchline `7ecb10a4…`, sidequest `a004867e…`, tier-list `a124bfd5…`), compilés contre le commun d'avant ces changements : ils gardent leurs contournements. Les budgets que juge E19 restent des points de départ, non mesurés.

## En une page

| # | Besoin | Jeux | Réponse | Où |
|---|---|---|---|---|
| 1 | La pile laissée par une lecture entre dans l'empreinte de l'état | Punchline 2, Sidequest B1, Tier list 1 | **accordé** | `reecriture` : passe `pile` |
| 2 | Les pannes de téléphone d'un jeu sans téléphone | Tier list 2 | **accordé** | `epreuve` |
| 3 | `vue_telephone` appelée pour un jeu sans téléphone | Tier list 3 | **accordé** | `jeu::colle` |
| 4 | `Texte::avec_suffixe` perd l'espace du suffixe | Punchline 1 | **accordé** (correctif) | `jeu` |
| 5 | Une clé constante | Punchline 5 | **accordé** | `jeu-abi` : `Cle::nommee`, `Cle::numero`, `IdEffet::nomme` |
| 6 | Une identité de demande pour un pire cas | Tier list 10 | **accordé** | `jeu` : `Construction::demande` |
| 7 | `Attente` sans autorité | Punchline 4 | **accordé** | `jeu` : `Attente::autorite` |
| 8 | E11 : l'hypothèse lue au début seulement ; un script fixe | Sidequest B3, Punchline 3 | **accordé** pour Sidequest, **refusé** pour Punchline | `epreuve`, `jeu` (doc) |
| 9 | Une direction en retard tient jusqu'au silence | Sidequest B4 | **accordé** | `partie::regles` |
| 10 | 450 ms de silence : comprises ou non | Sidequest B6 | **accordé** : comprises | `partie::regles` |
| 11 | Les demandes en vol à une nouvelle partie | Sidequest B7 | **déjà la règle** : annulées | `partie` (test cité) |
| 12 | Un appareil oublié qui revient | Sidequest B8 | **confirmé** : même `Appareil` | `partie` (test ajouté) |
| 13 | Le carburant d'une scène, le budget de lecture | Tier list 4 | **accordé** pour le compte du carburant ; le budget reste au mainteneur | `reecriture` : carburant par segment |
| 14 | Le brouillon, les sauvegardes de 64 Mio | Tier list 5 | **différé** (question 1 de § 13) | — |
| 15 | Des octets copiés d'un bloc dans la forme canonique | Tier list 10 (TL-22) | **accordé** | `jeu` : `Vec<u8>` |
| 16 | Comparer deux `Partage` par leur allocation | Tier list 10 | **accordé** | `jeu` : `Partage::meme` |
| 17 | Un texte qui garde ses retours à la ligne | Tier list 9 | **différé** à la phase 2 | — |
| 18 | Des commandes d'organisateur hors de la scène | Sidequest B2 | **refusé** | — |
| 19 | Le jeu relâche un état tenu | Sidequest B5 | **refusé** | — |
| 20 | La scène de la phase 2 : mesure de texte, boîte d'un composant, info-bulle, nom accessible, texte de remplacement, sélecteur nommé, nom de fichier d'un lien, images non dessinées, police nommée, glisser annulé par l'état | Tier list 6, 7, 8 ; Punchline 6 ; Sidequest B9 | **différé** à la phase 2 ; la police nommée **refusée** | — |

L'ABI ne change pas : `jeu_abi::ABI` vaut toujours 2, et un module compilé avant ces changements se charge et s'éprouve tel quel. `reecriture::VERSION` passe à 2.

## Ce que chaque jeu doit faire

Les agents des jeux s'adaptent d'après ceci. Tout le reste de ce document en donne les raisons.

**Les trois jeux**

1. Retirer le lavage de pile : `laver_la_pile` (Punchline), `nettoyer_la_pile` (Sidequest), `effacer_la_pile` (Tier list), et leurs appels dans `creer`, `appliquer` et `pire_cas`. La réécriture le fait pour tout module (besoin 1). Le test de chaque jeu qui compare deux exécutions avec et sans lectures (`deux_instances_meme_soiree_meme_empreinte`, `deux_executions_du_meme_journal_ont_la_meme_empreinte`, et celui de Punchline) doit rester vert sans le contournement : c'est le contrôle.
2. Les budgets mesurés changent : le carburant se compte maintenant par segment (besoin 13). Les chiffres de E1 et E19 des rapports de chaque jeu sont à remesurer ; ils baissent d'un facteur deux à neuf (tableau du besoin 13).
3. `Coupure::hypothese` est maintenant lue pendant toute la coupure, dans la soirée sans coupure (besoin 8). Une hypothèse qui ne tenait qu'au début d'une coupure ne laisse plus couper.

**Punchline**

- `textes::avec_suffixe` peut redevenir `Texte::avec_suffixe(" 2")` (besoin 4).
- Les clés des polices : `Cle::nommee("…")` au lieu de `Cle::du_fil(1..3)` (besoin 5). Attention : la clé change de valeur.
- `Jeu::attente` peut nommer les votants attendus avec `a.autorite()` (besoin 7). Ce qu'elle y écrit ne va qu'au commun.
- Les `Delai` qui remplaçaient `Option<Echeance>` peuvent redevenir des `Option<Echeance>` (besoin 1) ; ce n'est pas obligé.
- E11 ne reconsulte pas les bots (besoin 8, refusé) : `une_coupure_courte_ne_change_rien` reste le test de PUN-73.

**Sidequest**

- `proprietes().coupure_transparente` peut devenir `Applicable::Oui` : une hypothèse « il ne tient aucune direction, et personne n'est à moins de 28 px de lui » est lue à chaque lecture de la coupure (besoin 8).
- `age_max` borne maintenant la durée d'une valeur jeune mais en retard (besoin 9), et la fraîcheur est comprise : à 450 ms pile, la direction tient, et se relâche au 28ᵉ pas (besoin 10). Les trajectoires de référence qui jouaient la règle de l'original à la main n'ont plus d'écart avec le commun.
- B2 (chat, question libre) : un `raccourci` de la scène pour le chat ; un `ComposantVu::Champ` pour une question libre (besoin 18).
- B5 : garder le gel (besoin 19, refusé).
- B7 : garder « REJOUER attend la réponse de la ligne » (besoin 11).
- `Look` fait d'énumérations d'un octet n'est plus nécessaire (besoin 1) ; ce n'est pas obligé.

**Tier list**

- `vue_telephone` ne sera jamais appelée (besoin 3) ; elle peut garder son `panic!` sous `#[expect]`, qui ne s'atteint plus.
- L'option `--sans-pannes-de-telephone` de `examples/epreuve.rs` n'a plus d'objet : `soiree-epreuve` rend 0 sur le module livré (besoin 2).
- `IdDemande::du_fil(u64::MAX)` dans le pire cas devient `c.demande(0)` (besoin 6).
- `Contenu::meme_que` peut comparer deux images par `Partage::meme` (besoin 16).
- Les images en `Vec<u8>` s'écrivent d'un bloc dans la forme canonique, sans rien changer au jeu (besoin 15) ; recompiler suffit.
- Le réglage du bot fait pour E19 (20 tuiles, 7 rangs) peut se desserrer : `Regarder` coûte 2,5 fois moins (besoin 13).

---

## 1. La pile laissée par une lecture entre dans l'empreinte — accordé

**Le besoin** (Punchline 2, Sidequest B1, Tier list 1). Une valeur bâtie sur la pile puis copiée dans le tas emporte les octets que ses champs ne couvrent pas : bourrage, valeur d'un variant vide, mot jamais écrit (le `height` de la racine vide d'une `BTreeMap`). Ces octets sont ce que l'appel d'avant a laissé sur la pile. Une lecture (une vue, un bot, l'état) écrit la pile ; une reprise ou une instance jumelle rejoue le journal sans elle. La mémoire hors des zones exclues diffère alors, et l'empreinte aussi : E9 et E14 échouaient dans les trois jeux.

**Pourquoi c'est au commun.** Le contrat : « La mémoire entière de son module compte comme son état », et « Deux exécutions du même module sur le même journal ont la même empreinte » (§ 3.2). Une empreinte qui dépend des lectures casse la reprise, la seule façon de repartir (MANIFESTE). Le défaut est dans la forme du commun, pas dans un jeu : tout état qui contient une `Option`, une enum à données, une `BTreeMap` ou un struct à bourrage y est exposé. Il revient dans les trois jeux.

**La réponse.** Une passe de plus dans la réécriture, `reecriture::passes::pile` : au début de `soiree_boite(taille, sorte)`, quand `sorte` vaut 1 (une écriture, § 3.3), elle remet à zéro toute la pile `[0, __stack_pointer)` par un `memory.fill`. Chaque écriture part ainsi d'une pile de zéros : la mémoire hors des zones exclues ne dépend que des écritures.

- **Dans la réécriture, pas dans `jeu` ni dans le moteur.** Le module réécrit porte lui-même ses garanties (D32) : la passe vaut pour tout module, écrit ou non avec `jeu`, et le moteur du navigateur (phase 2) n'aura rien à ajouter. La colle de `jeu` ne connaît pas les bornes de la pile ; la réécriture les constate déjà (`Refus::PileNonEnBas`).
- **Sans carburant.** La passe vient après celle du carburant : la remise à zéro est le fait du commun, elle ne coûte rien au jeu. Son prix est un `memory.fill` de la pile (1 Mio par défaut) avant chaque écriture, en temps de machine seulement. Une lecture ne remet pas la pile à zéro : elle n'en a pas besoin.
- **`reecriture::VERSION` passe à 2.** Les octets réécrits changent.

**Les tests.**

- `reecriture` : `sans_pile_une_lecture_change_l_etat_par_la_pile_laissee` (la passe rougit quand on la retire), `avec_pile_une_ecriture_part_d_une_pile_de_zeros`, `pile_remise_a_zero_aux_seules_ecritures_et_sans_carburant`, sur un module `wat` qui imite le défaut.
- `epreuve/tests/epreuve/pile.rs` : `une_lecture_entre_deux_ecritures_ne_change_pas_l_empreinte`, sur le module d'**`ardoise`**, un jeu d'exemple ajouté au commun (`crates/exemples/ardoise`) : son état garde des `Option<u64>` dans un `Vec` et l'historique d'une `BTreeMap` vidée, et sa scène descend plus bas sur la pile que l'écriture qui la suit.
- **Contrôle négatif, fait à la main le 2026-11-04** : avec la passe retirée de `reecrire`, ce test échoue (« après l'entrée 0 (Ecrire(Borne(0))), une lecture a changé l'état »), et `soiree-epreuve --court` sur `ardoise` rend le code 1 : E9 « partie 0, entrée d'ordre 0 au pas 33 : deux exécutions du module n'ont pas la même empreinte », E14 « la reprise depuis le pas 0 diverge à l'entrée d'ordre 54 de la partie 0 ». Ce sont les messages que les jeux ont rapportés. La passe remise, le code est 0.
- Une loi déjà contrôlée : E9 et E14 sont ses contrôles, et le fautif est `ardoise` sans la passe. Il n'y a pas de fautif permanent : le défaut est dans l'outil, pas dans un jeu.

**Constaté** : `devine` passait par chance, son état n'a presque pas de bourrage. `ardoise` n'échouait pas non plus tant que sa valeur se bâtissait dans un cadre de pile que l'écriture elle-même avait déjà écrit : il a fallu le cadre profond de `sous_la_pile` pour reproduire le défaut. Dans un vrai jeu, une étape longue fait la même chose.

## 2. Les pannes de téléphone d'un jeu sans téléphone — accordé

**Le besoin** (Tier list 2). Le plan standard programme des coupures, silences, trous noirs, paquets retenus, rafales, rechargements et onglets jumeaux. Un jeu sans téléphone ne peut pas les vivre ; l'épreuve les disait « n'ont pas eu lieu », et chaque contrôle des soirées finissait « pas exécuté » : code 3, quoi que fasse le jeu.

**Pourquoi.** Le contrat (« L'épreuve ») distingue « une exigence sans objet » d'« un contrôle pas encore exécuté ». Une panne de téléphone pour un jeu qui a déclaré `telephones: SansObjet` est sans objet, par sa déclaration ; ce n'est pas une panne qui n'a pas eu lieu. OUT-26 reste vrai pour toute panne qui a un objet.

**La réponse.** `Panne::du_telephone()` dit si une panne est celle d'un téléphone. Une soirée sans téléphone ne les programme pas : elles sont « sans objet » et ne rendent rien « pas exécuté ». Le rapport le dit, dans « Joué » : « pannes sans objet, le jeu n'a pas de téléphone : coupure de 1500 ms, … ». Le redémarrage de l'autorité et les gels restent joués et constatés.

**Les tests.** `epreuve/tests/epreuve` : `un_jeu_sans_telephone_passe_tout_sans_pannes_de_telephone` (`ardoise`, plan court, copie native comparée : tout réussi ou sans objet, code 0, le rapport nomme les pannes sans objet). `panne_injectee_constatee` et les autres tests d'OUT-26 restent verts.

**Mesuré** : `soiree-epreuve` sur le module livré de Tier list, plan standard, sans copie native : **code 0** (il rendait 3), 39 s. E1, E3 à E9, E13 à E19 réussis ; E2, E10, E11, E12 sans objet.

## 3. `vue_telephone` pour un jeu sans téléphone — accordé

**Le besoin** (Tier list 3). Avec `telephones: SansObjet`, `VueTelephone` vaut `Aucun`, sans valeur : `vue_telephone` ne peut rien rendre. La colle l'appelait quand même pour un `Regarder` ou un `Bot` au regard d'appareil, qu'un appel absurde de E13 peut former : le jeu paniquait, une faute pendant une lecture.

**Pourquoi.** « Ce qui vient de l'extérieur est validé dès son entrée » (MANIFESTE) : un regard d'appareil n'a pas de sens pour un jeu qui n'a pas de téléphone. E13 attend `Malforme` ou `Illisible`, jamais une faute.

**La réponse.** `jeu::colle` refuse un regard d'appareil quand la déclaration n'a pas de téléphone : `Regarder` et `Bot` rendent `Retour::Illisible` (`Illisible::Valeur`), sans appeler le jeu. `Projeter` reste permis (une projection est un état).

**Les tests.** `ardoise/tests/colle.rs` : `regarder_un_appareil_sans_telephone_est_illisible`, `un_bot_de_telephone_sans_telephone_est_illisible`, `un_geste_de_telephone_sans_telephone_ne_se_decode_pas`. Sans la règle, `vue_telephone` d'`ardoise` panique et les deux premiers tests échouent.

## 4. `Texte::avec_suffixe` perd l'espace du suffixe — accordé

**Le besoin** (Punchline 1, PUN-25). `avec_suffixe(" 2")` nettoyait le suffixe seul, donc retirait son espace : `MMMMMMMMMMMM` devenait `MMMMMMMMMMM2` au lieu de `MMMMMMMMMM 2`.

**La réponse.** Un correctif, sans changement d'API : un suffixe qui commence par un blanc garde une espace devant lui ; le texte raccourcit entre graphèmes pour lui faire place, et une coupe qui tombe sur une espace ne la double pas.

**Les tests.** `jeu/tests/texte.rs` : `texte_suffixe_garde_son_espace` (les cas du cahier) ; la propriété `texte_suffixe_tient` vérifie de plus l'espace et que le résultat est déjà propre.

## 5. Une clé constante — accordé

**Le besoin** (Punchline 5). `Declaration::assets` est une constante, mais `Cle::de` ne l'est pas ; Punchline employait `Cle::du_fil`, réservé au commun.

**La réponse.** Trois fonctions `const` dans `jeu-abi`, réexportées par `jeu::prelude` :

```rust
impl Cle {
    pub const fn nommee(nom: &str) -> Cle;   // == Cle::de(nom)
    pub const fn numero(n: u64) -> Cle;      // == Cle::de(&n)
}
impl IdEffet {
    pub const fn nomme(nom: &str) -> IdEffet; // == IdEffet::de(nom)
}
```

Elles calculent le FNV-1a 64 de l'encodage postcard (le varint de la longueur, puis les octets) : la même clé que `Cle::de`, une seule définition (« Deux morceaux qui s'accordent n'ont qu'une définition »). `Cle::du_fil` reste réservé.

**Le test.** `jeu-abi/tests/abi/valeurs.rs` : `cle_nommee_et_numero_valent_cle_de`, dont une constante, un nom de 400 octets (varint sur deux octets) et des nombres de part et d'autre de chaque longueur de varint.

## 6. Une identité de demande pour un pire cas — accordé

**Le besoin** (Tier list 10). Un pire cas qui montre une demande (un export en cours) a besoin d'un `IdDemande`, que seul `Sorties::id` crée ; Tier list employait `IdDemande::du_fil(u64::MAX)`, réservé.

**La réponse.** `Construction::demande(n: u16) -> IdDemande` : des identités distinctes, prises tout en haut de l'espace (`u64::MAX − n`), que le compteur d'une soirée n'atteint pas. Elles ne valent que pour ce pire cas, comme `Construction::appareils`.

**Le test.** Couvert par la compilation et E16 sur les jeux ; pas de test propre (une ligne sans logique).

## 7. `Attente` sans autorité — accordé

**Le besoin** (Punchline 4). Pendant un vote, les votants attendus dépendent des auteurs du duel, qui sont secrets ; `Jeu::attente(&self, …)` n'avait pas l'autorité, et Punchline ne pouvait donner que l'échéance.

**Pourquoi.** E5 (« jamais figée ») vérifie mieux quand le jeu nomme qui il attend : « par un essai sur une copie, pas par une supposition » (D31). L'attente est lue sur la partie qui fait foi et ne sort pas du commun : `AttenteFil` ne va à aucun appareil. L'ouvrir avec `Autorite`, comme `resume` et `invariants`, ne fait fuir aucun secret.

**La réponse.** `Attente::autorite(&self) -> &Autorite`. La signature de `Jeu::attente` ne change pas.

**Ce qu'il ne faut pas faire** : montrer `AttenteFil` à un appareil. Nommer les votants désigne les auteurs par élimination. Si la phase 2 veut afficher « on attend : … » sur la télé, elle le fera depuis la vue du jeu, à travers son regard, pas depuis l'attente.

**Le test.** `devine` nomme maintenant, pendant les essais, les joueurs qui n'ont pas proposé (leur essai est un secret) : `devine/tests/regles.rs`, `l_attente_nomme_ceux_qui_n_ont_pas_propose`. E5 l'éprouve dans chaque soirée de `devine` : chaque joueur nommé doit voir son « Proposer » accepté sur une copie.

## 8. E11, la coupure transparente — accordé pour Sidequest, refusé pour Punchline

**Les besoins.** Sidequest B3 : l'hypothèse de `Coupure` n'était lue qu'au début de la coupure ; or « personne n'a poussé ce joueur pendant la coupure » ne se sait qu'à la fin. Punchline 3 : E11 rejoue un script fixe ; une phase qui se ferme « quand plus personne n'est attendu » se ferme plus tard quand un vote est retenu, et toute la suite se décale. Punchline demande que les bots soient reconsultés après la coupure.

**La réponse pour Sidequest (accordé).** E11 lit maintenant l'hypothèse **pendant toute la coupure**, dans la soirée **sans** coupure : tous les 10 pas (`PERIODE_HYPOTHESE`), du début de la coupure à la première lecture après sa fin. Une coupure n'est essayée que si l'hypothèse a tenu à chaque lecture. Une condition « de fin » s'écrit comme une condition de chaque pas, lue dans la soirée de référence : « personne n'est à moins de 28 px de lui, et il ne tient aucune direction ». Si elle tient à chaque pas sans la coupure, la coupure ne change pas les trajectoires des autres.

Ce changement de sens est une rupture sans changement de type (CONTRAT, « Les versions ») : `Coupure` garde ses deux champs, mais une hypothèse qui ne tenait qu'au début ne laisse plus couper. Le socle des fautifs en dépendait : son hypothèse (« au salon ») ne tenait pas deux secondes, car son organisateur lançait la manche après une seconde. Il attend maintenant quatre secondes au salon, et son hypothèse dit ce qu'elle promet vraiment (une coupure tout entière au salon). `devine` et Punchline n'ont rien à changer : leurs hypothèses durent.

**Les tests.** `epreuve` (unité) : `l_hypothese_doit_tenir_pendant_toute_la_coupure` (une hypothèse vraie au début mais fausse avant la fin ne laisse pas couper ; un trou d'une seule lecture suffit à écarter une coupure). `epreuve/tests/epreuve` : `devine`, `ardoise` et le socle sain passent E11 ou le déclarent sans objet ; `coupure_qui_compte_echoue_e11` échoue toujours sur E11, et seulement sur lui.

**La réponse pour Punchline (refusé).** Reconsulter les bots après une coupure, c'est comparer deux soirées dont les bots ont décidé à d'autres moments, depuis d'autres vues : leurs choix diffèrent pour des raisons qui ne sont pas la coupure (le hasard d'un bot est semé par son tirage, et un vote tiré au pas 1200 n'est pas celui tiré au pas 1260). Un résumé différent ne prouverait alors rien contre le jeu, et un résumé égal ne prouverait rien pour lui. Le contrat demande qu'« un contrôle doit savoir échouer » pour la bonne raison. PUN-73 reste une question de jeu, vérifiée par `absences::une_coupure_courte_ne_change_rien`, où les gestes suivent l'état ; la lecture pendant toute la coupure lui permet aussi de déclarer à E11 des hypothèses plus larges qu'aujourd'hui (un joueur qui n'est attendu à aucun pas de la coupure).

## 9. Une direction en retard ne tient que jusqu'à son âge — accordé

**Le besoin** (Sidequest B4, SQ-02). Une direction faite il y a plus d'1 s ne fait bouger personne, **et une plus jeune ne tient que jusqu'à cette seconde**. Le cœur écartait à l'arrivée un état plus vieux que `age_max`, mais une valeur jeune et en retard tenait ensuite jusqu'au silence de `fraicheur`. Le jeu ne peut pas le faire : il ne reçoit que les changements de la valeur effective.

**Pourquoi c'est au commun.** SQ-02 est une leçon de l'étage commun (« Sous la nouvelle forme : pression du commun ») ; D17 : « fraîcheur, âge et débit sont du monde, pas du jeu ».

**La réponse.** `EtatTenu` retient `fait` : l'instant le plus récent d'une arrivée acceptée de ce tenu, même inchangée. `regles::a_ce_pas` relâche la valeur effective, cause `Silence`, quand `pas − fait > age_max`. Une redite de la même valeur repousse la borne. Cela suppose que l'appareil qui tient une valeur la redise : la page de la phase 2 le fera, comme celle de l'ancien jeu toutes les 150 ms (SQ-01) ; c'est une exigence pour `jeu-page`. Le téléphone simulé de l'épreuve ne redit pas encore son état tenu (voir « Ce qui n'est pas fait »).

**Les tests.** `partie/tests/coeur/regles.rs` : `une_valeur_tardive_ne_tient_que_jusqu_a_son_age` ; la propriété `un_tenu_tient_ses_promesses` vérifie qu'un relâchement par silence n'arrive que si la fraîcheur ou l'âge le permet.

## 10. La fraîcheur : 450 ms comprises — accordé

**Le besoin** (Sidequest B6). Le cœur relâchait au 27ᵉ pas de silence (`≥`) ; l'original tenait encore à 450 ms pile, et lâchait au 28ᵉ.

**La réponse.** Les deux bornes sont strictes, comme la source de la leçon (SQ-01) : une direction se relâche quand le silence **dépasse** `fraicheur`, et quand sa dernière redite **dépasse** `age_max` ; un état de `age_max` pile compte encore, comme avant.

**Les tests.** `partie` : `la_fraicheur_est_comprise` (à 27 pas, la valeur tient ; au 28ᵉ, elle se relâche), `tenu_silence_relache` mis à jour.

## 11. Les demandes en vol à une nouvelle partie — déjà la règle

**Le besoin** (Sidequest B7). Que devient une demande listée par l'ancienne partie quand `nouvelle_partie` remplace l'instance ?

**La réponse.** Elle est **annulée** : la nouvelle instance ne la liste pas, et « une demande qui disparaît de l'état est annulée ; sa réponse ne revient jamais » (§ 0, D9). `partie` la met dans `Avancee::annuler`, et sa réponse tardive est `Perimee`. C'était déjà le code ; le test le dit maintenant : `partie/tests/coeur/partie.rs`, `demande_d_une_nouvelle_partie_part_et_l_ancienne_reponse_est_perimee`, et `nouvelle_partie_instance_neuve_memoire_rendue`.

Pour une ligne de résultats : annuler une ligne déjà ajoutée au flux ne l'efface pas, et la reporter dans la garde risquerait de l'écrire deux fois (CAT-76). Ce que fait Sidequest est juste : REJOUER attend la réponse de la ligne, et l'attente le dit par `libre`.

## 12. Un appareil oublié qui revient — confirmé

**Le besoin** (Sidequest B8). Un appareil oublié revient-il sous le même `Appareil` ?

**La réponse.** Oui. Le contrat donne « une identité stable par appareil » ; `Sorties::oublier` fait revenir l'appareil « comme un neuf », c'est-à-dire annoncé par `Presence::Arrivee` et non par `Retour`, sous la même identité. Le cœur ne renomme jamais un appareil : il joue celui que le transport lui donne. Le téléphone simulé de l'épreuve garde son identité ; le relais de la phase 2 devra la garder aussi.

**Le test.** `partie` : `appareil_oublie_revient_sous_le_meme_appareil`.

## 13. Le carburant d'une scène — accordé pour le compte, le budget reste au mainteneur

**Le besoin** (Tier list 4). Un nœud de scène coûtait 7 000 à 9 000 unités de carburant ; Tier list dépassait le budget de lecture vers 55 tuiles, et ne passait E19 qu'en bridant son bot.

**Ce qui se passait.** La réécriture comptait le carburant par fonction : à chaque entrée, le nombre d'instructions **écrites** de tout le corps hors des boucles. Une grande fonction (le `match` de l'enveloppe dans la colle, un sérialiseur de `serde` pour une enum à quinze variants) payait son corps entier pour le chemin court qu'elle prenait. Un `Bonjour` coûtait 28 302 unités pour quelques dizaines d'instructions exécutées. Le compte était un majorant juste, mais grossier d'un facteur cinq à dix ; il ne mesurait pas le travail que le budget veut borner.

**La réponse.** Le carburant se compte maintenant **par segment** : une suite d'instructions où le contrôle n'entre que par la première. Un segment commence à l'entrée d'une fonction et juste après chaque `loop`, `if`, `else`, `end`, `br`, `br_table`, `return` et `unreachable` ; aucune autre instruction n'est la cible d'un saut. Chaque segment paie son nombre d'instructions écrites quand on y entre. Le compte reste un majorant fixe et déterministe (un `br_if` pris laisse payées les instructions qui le suivent), et une tête de boucle coûte au moins 1 : la boucle infinie s'arrête toujours au même endroit. `memory.copy` et `memory.fill` coûtent toujours leur longueur divisée par 64. § 3.6 d'INTERFACE.md disait déjà « le nombre d'instructions du bloc ».

**Les tests.** `reecriture` : `carburant_ne_paie_que_le_chemin_pris` (le chemin court d'une fonction de 800 instructions coûte moins de 20) ; `carburant_majore_le_carburant_de_wasmtime`, une propriété où `wasmtime` sert de juge mûr (CONTRAT, « Le code tiers ») : sur le corpus, notre compte n'est jamais sous le sien, appel par appel, piège compris (les modules à `memory.copy`, `memory.fill` et `memory.grow` en sont exclus : `wasmtime` compte un par octet et charge la croissance). Les tests d'avant (boucle infinie, compte égal d'une exécution à l'autre, `a + b × n`, longueur des copies) restent verts, et `boucle_au_vote_echoue_e19` échoue toujours sur E19.

**Mesuré** (2026-11-04, aarch64-apple-darwin ; « avant » est le compte par fonction, « après » le compte par segment, sur les mêmes octets de module et le même plan) :

| Module, plan | Appel | Avant | Après |
|---|---|---|---|
| `devine`, `--court` | `Bonjour`, instance neuve | 28 302 | 3 131 |
| `devine`, `--court` | `Regarder`, 99ᵉ centile | 116 280 | 40 384 |
| `devine`, `--court` | `Entree`, 99ᵉ centile | 97 839 | 24 975 |
| `ardoise`, `--court` | `Regarder`, 99ᵉ centile | 498 681 | 164 463 |
| Tier list, standard | `Regarder`, 99ᵉ centile | 1 963 893 | 790 642 |
| Punchline, standard | `Regarder`, 99ᵉ centile | 609 640 | 260 058 |
| Punchline, standard | `Entree`, 99ᵉ centile | 1 223 441 | 382 850 |
| Punchline, standard | `Creer`, 99ᵉ centile | 949 643 | 302 475 |
| Sidequest, standard | `Regarder`, 99ᵉ centile | 1 365 893 | 520 121 |
| Sidequest, standard | `Entree`, 99ᵉ centile | 181 409 | 119 728 |

Les colonnes « avant » des trois jeux sont celles de leurs comptes rendus (même date, même machine, même module) ; celles de `devine` et d'`ardoise` sont mesurées ici. Un nœud de la scène d'`ardoise` coûte encore environ 4 800 unités : c'est maintenant du travail exécuté (la clé, la copie du nœud, son encodage, puis celui de la réponse), qu'une optimisation de `jeu::Scene` pourrait réduire ; elle n'est pas faite.

**Ce qui reste au mainteneur.** Le budget lui-même (`BUDGET_LECTURE`, 2²¹) reste un point de départ non mesuré : la question 12 de § 13 est la sienne. Avec ce compte, le 99ᵉ centile de `Regarder` de Tier list, bot bridé, est à 38 % du budget au lieu de 94 %.

**Vérifié avec ce commun** (le 2026-11-04, aarch64-apple-darwin, `soiree-epreuve` plan standard, sans copie native, sur les modules livrés) : Punchline code 0 en 37 s, Tier list code 0 en 39 s (il rendait 3, besoin 2), Sidequest code 0 en 5 min 08 s. Le temps de Sidequest est celui qu'il rapportait avec l'ancien compte (5 min 01 s) : compter par segment ne ralentit pas l'épreuve de façon visible.

## 14. Le brouillon et les sauvegardes de 64 Mio — différé

**Le besoin** (Tier list 5). Une sortie passe par le brouillon : une sauvegarde de 64 Mio demanderait un brouillon de plus de 64 Mio par instance, recopié par chaque point de reprise.

**Pourquoi différé.** C'est la question 1 de § 13 (les gros états), qui est au mainteneur : la réponse provisoire garde « la mémoire entière est l'état » (D23) et dit de mesurer. Rendre des octets du tas sans les copier au brouillon, ou des blocs hors de la mémoire nommés par empreinte, changerait l'enveloppe (§ 3.4, étape 6 : la sortie doit être dans le brouillon) et le sens de l'état. Ce n'est pas une décision d'agent. En attendant, les bornes de Tier list (16 Mio par sauvegarde, 10 Mio d'images, brouillon de 24 Mio) sont la bonne façon de tenir : un plateau valide tient toujours dans une sauvegarde, et le message cite la borne réelle.

## 15. Des octets copiés d'un bloc — accordé

**Le besoin** (Tier list 10, TL-22). Les octets d'une image sont un `Vec<u8>` ; la forme canonique les écrivait un par un.

**La réponse.** Sans type nouveau : `Etat` a deux méthodes cachées de plus, `ecrire_suite` et `lire_suite`, qui écrivent une suite élément par élément par défaut ; `u8` les remplace par une copie d'un bloc, et `Vec<T>` les appelle. Les octets écrits sont **les mêmes** : la forme canonique ne change pas, et une sauvegarde ou une projection d'avant se relit. Une longueur qui annonce plus qu'il ne reste échoue toujours avant toute réservation (REM-58).

**Les tests.** `jeu` (unité) : `octets_en_bloc_meme_forme` ; les propriétés `forme_canonique_aller_retour` et `octets_abimes_sans_panique`, sur un état qui contient un `Partage<Vec<u8>>`, restent vertes.

**Non mesuré** sur Tier list : son module livré est compilé contre l'ancien `jeu`. Le 99ᵉ centile de sa `Canonique` y vaut 9 143 550 unités (plan standard, compte par segment) ; il faudra le remesurer après recompilation.

## 16. Comparer deux `Partage` — accordé

**Le besoin** (Tier list 10). Dire qu'une action n'a rien changé (TL-18) sans comparer les octets de chaque image.

**La réponse.** `Partage::meme(a, b) -> Oui` : les deux partagent-ils la même allocation ? La forme canonique écrit un partage une fois par allocation et le rend partagé à la relecture : la réponse survit à une reprise et à un rechargement.

**Le test.** `jeu` (unité) : `partage_meme_survit_a_la_forme_canonique`.

## 17. Un texte qui garde ses retours à la ligne — différé à la phase 2

**Le besoin** (Tier list 9). `Texte<L>::nettoyer` réduit tout blanc à une espace ; Maj+Entrée ne fait pas de ligne dans une étiquette.

**Pourquoi différé.** Le nettoyage est une seule fonction, pour la page et les règles (D22, TL-27), et ses leçons (PUN-22 à PUN-26, CAT-79 à CAT-85) supposent le blanc réduit. Un mode qui garde les retours à la ligne doit dire combien de lignes, ce qu'est une ligne vide, comment se compte un retour dans chaque `Unite`, et ce que fait la page sur Entrée et Maj+Entrée : c'est le contrat du composant `Champ` multiligne, qui vient avec la page (phase 2). Les cahiers ne s'accordent d'ailleurs pas : celui de Sidequest (§ 4.6) veut qu'un retour disparaisse, celui de Tier list qu'il reste. Une incohérence est notée pour ce moment : `ComposantVu::Champ` a déjà un champ `lignes`, qu'aucun `Texte<L>` ne peut tenir au-delà d'une.

## 18. Des commandes d'organisateur hors de la scène — refusé

**Le besoin** (Sidequest B2). Le chat oui ou non, et une question libre, « sans interface ».

**Pourquoi refusé.** « Une action que l'écran n'affichait pas » ne s'envoie pas : c'est une forme du commun (SAL-152, D26, § 2.6). Un canal hors de la scène la contournerait, et c'est exactement ce que la forme rend impossible.

**Ce qui existe déjà.** Le chat se bascule par un **raccourci** : `Scene::raccourci(touche, mods, repetition, action)` porte l'action sans rien dessiner. La question libre se saisit dans un **composant** `ComposantVu::Champ` (ou derrière une `Modale` ouverte par un raccourci) : le jeu reçoit `Composant::Saisie` et lit le texte. Les deux sont « depuis l'ordinateur de la soirée », sans bouton visible.

## 19. Le jeu relâche un état tenu — refusé

**Le besoin** (Sidequest B5). Une sortie « relâcher ce tenu » qui remette aussi à zéro la valeur effective du cœur, ou une relivraison après une pause du jeu.

**Pourquoi refusé.** L'état tenu est au commun : « un état périmé est remplacé par le plus récent » (CONTRAT, « Réseau »), et ses relâchements sont ceux du monde (silence, départ, arrière-plan, pause du commun, âge). Un relâchement par une règle du jeu n'est pas un fait du monde : c'est la décision de ne pas en tenir compte, et elle s'écrit dans l'étape. Le gel que Sidequest a choisi (personne ne bouge en pause, aux Résultats et à la Fin ; la direction tenue repart ensuite) reproduit ce qu'on voyait dans l'ancien jeu : sa page redisait la direction toutes les 150 ms, et un pouce resté appuyé repartait aussitôt après le relâchement forcé. Si le cahier veut qu'il faille relever le doigt, le jeu ignore la valeur tenue jusqu'au prochain `Changement` qu'il reçoit, sans toucher au commun. Après une nouvelle partie, le cœur oublie déjà les valeurs effectives : la redite de la page les relivre.

## 20. La scène de la phase 2 — différé, sauf la police nommée, refusée

Rien de ce qui suit n'a de sens sans rendu ni page, qui sont la phase 2 (§ 8, « Le but de la phase 1 »). Chacun se décidera avec eux, sur un vrai dessin :

- **Mesurer un texte** (Tier list 6) : la promesse de `Taille::Budget` (rétrécir pour tenir) est celle du commun, et la galerie des pires cas la mesurera avec le rendu (§ 7.2, E16 : « la mesure des débordements vient avec le rendu »).
- **Une boîte pour un composant**, **une info-bulle, un nom accessible, un texte de remplacement** (Tier list 7, TL-29) : des ajouts au langage de scène, à dessiner.
- **Le sélecteur d'où viennent des fichiers** (Tier list 8) : un dépôt « partout » ou un collage ne vient d'aucun sélecteur ; la question est celle du composant `Selecteur`, pas de l'étape. La règle de Tier list (un seul JSON est une restauration) tient d'ici là.
- **Un nom de fichier pour `Lien`**, **le nombre d'images que `Rendu` n'a pas dessinées** (Tier list 7, 8, TL-56) : des formes de `ComposantVu` et de `Reponse` que le rendu fixera ; les changer maintenant serait une rupture d'ABI sans producteur pour la remplir.
- **Annuler un glisser depuis l'état** (Tier list 7, TL-44) : changer la clé des objets, comme le fait Tier list, est la forme voulue (§ 2.9 : une clé stable, calculée depuis une valeur ; un objet qui n'existe plus ne se dépose pas).
- **Les personnages, la pâte, les états de connexion** (Punchline 6), **les polices pixel, les sprites, le pad, le son** (Sidequest B9) : phase 2.
- **Une police nommée** (Tier list 7, Arial) — **refusé.** Une police du système diffère d'une machine à l'autre, et le dessin avec elle ; le contrat dit « nous créons nos propres assets ». Un jeu déclare sa police dans `Declaration::assets`, comme Punchline le fait.

---

## Les changements d'API, précisément

Aucun changement n'est une rupture de type pour un jeu qui compile aujourd'hui : chaque ajout est une fonction ou une méthode nouvelle, ou une méthode cachée avec un corps par défaut. Les ruptures de sens sont marquées.

| Crate | Changement | Sorte |
|---|---|---|
| `reecriture` | `VERSION` = 2 ; `passes::pile(m, pile: Option<u32>)` ; `reecrire` l'enchaîne après `croissance` | ajout ; octets réécrits différents |
| `reecriture` | le carburant se compte par segment | **sens** : les budgets mesurés baissent |
| `jeu-abi` | `Cle::nommee(&str)`, `Cle::numero(u64)`, `IdEffet::nomme(&str)`, `const` | ajout |
| `jeu` | `Attente::autorite(&self) -> &Autorite` | ajout |
| `jeu` | `Construction::demande(&self, n: u16) -> IdDemande` | ajout |
| `jeu` | `Partage::meme(&Partage<T>, &Partage<T>) -> Oui` | ajout |
| `jeu` | `Etat::ecrire_suite`, `Etat::lire_suite`, cachées, avec un corps par défaut ; `u8` et `Vec<T>` les emploient | ajout ; forme canonique inchangée |
| `jeu` | `Texte::avec_suffixe` garde l'espace de tête du suffixe | **sens** (correctif) |
| `jeu` | la colle rend `Illisible::Valeur` pour `Regarder` ou `Bot` au regard d'appareil quand `telephones: SansObjet` | **sens** |
| `jeu` | `Coupure::hypothese` : lue pendant toute la coupure | **sens** (doc ; la lecture est dans `epreuve`) |
| `partie` | `EtatTenu::fait` ; `a_ce_pas` relâche par âge ; fraîcheur et âge strictement dépassés | champ public ajouté ; **sens** |
| `epreuve` | `Panne::du_telephone() -> Oui` ; les pannes de téléphone sans objet pour un jeu sans téléphone | ajout ; **sens** (rapport) |
| `epreuve` | E11 lit l'hypothèse tous les 10 pas, pendant toute la coupure | **sens** |
| `exemples` | `ardoise`, nouveau ; `devine::attente` nomme les joueurs attendus ; le socle des fautifs attend quatre secondes au salon | exemples |

`INTERFACE.md` n'est pas modifié par ce chantier : § 2.13 (`Coupure`, « à son début »), § 3.6 (les passes, et la version), § 4.3 (les tenus) et § 8.5 (l'API de `reecriture`) sont à mettre à jour par le mainteneur d'après ce tableau.

## Ce qui n'est pas fait

- **Le téléphone simulé ne redit pas son état tenu.** La page de la phase 2 devra le redire (SQ-01) pour que la fraîcheur et l'âge (besoin 9) ne relâchent pas un pouce resté appuyé ; l'épreuve ne le simule pas encore. Avec un jeu qui n'envoie un tenu qu'au changement, l'âge peut relâcher dans l'épreuve une valeur qu'une vraie page aurait redite.
- **Le coût d'un nœud de scène** reste d'environ 4 800 unités de carburant (mesuré sur `ardoise`) ; `jeu::Scene` n'est pas optimisée.
- **Les budgets de E19** restent des points de départ (question 12).
- **Les modules des jeux n'ont pas été recompilés** contre ce commun : les mesures de ce document portent sur les modules livrés, avec leurs contournements.
