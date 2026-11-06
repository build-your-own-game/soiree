# Le journal des lots de la phase 3

> Ce que chaque lot de [`PHASE-3.md`](PHASE-3.md) (§ 7) a laissé : ce qui est fait, les tests qui le tiennent, les mesures, ce qui ne l'est pas et pourquoi, ce que le lot suivant doit savoir. Un lot s'ajoute à la fin ; on ne réécrit pas celui d'un autre. Les mesures y portent leur date, leur machine, la charge et la commande qui les refait ; les plus durables vont aussi dans [`MESURES-PHASE-3.md`](MESURES-PHASE-3.md).

## Lot 1 — Le cœur prêt pour le monde

Fait le 2026-11-05, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Rien n'est commité : un agent à part le fait après la vérification.

### Ce qui est fait

**Le journal à base d'indices absolus (P10).** `Journal` gagne une base : `base()`, `fin()`, `ligne(indice)`, `tronquer_avant(indice)`. Tout ce qui désigne une ligne (l'indice d'un point, `Ligne::Reprise::depuis`, `RapportFaute::point` et `jusqu_a`, le curseur du rejeu) est absolu et ne bouge pas quand le journal est tronqué ; `lignes()` ne rend que les lignes gardées. `Partie::reprendre` accepte un journal tronqué et refuse un point avant sa base (`PointHorsJournal`). `Partie::tronquer_journal_avant` tronque le journal vivant, jamais au-delà du dernier point (d'où une faute se reconstruit).

**Le tour borné (P7).** `pas.rs` coupe le jeu d'un pas en trois : `commencer_pas` (les arrivées du pas, figées maintenant, rangées et traduites en entrées), `continuer_pas` (les entrées, tant que le tour le permet), puis la fin du pas (`Temps`, sorties, état, point). Une avancée (`AvanceeEnCours` : la borne, la cible, le saut) peut rester en cours d'un tour à l'autre.
- `Partie::avancer_borne(cible, arrieres_par_tour)` : au plus `arrieres_par_tour` entrées d'un arriéré par tour ; `Partie::avancer_decoupe(cible, n)` : au plus `n` entrées venues d'une arrivée, pour éprouver le découpage ; `Partie::avancer` : sans borne, comme avant.
- `partie::ARRIERE_PAS` (60, déplacé de `epreuve::budgets`, que l'épreuve relit) et `partie::ARRIERES_PAR_TOUR` (8, choisi).
- Un arriéré est **un geste ou une entrée de l'écran** faits sur une vue de plus de `ARRIERE_PAS` pas avant leur réception (`pas.rs`, `est_un_arriere`). Un tenu, une manette, une présence, une réponse n'en sont jamais : voir les écarts.
- Une arrivée reçue pendant un pas inachevé est placée au-delà de ce pas (`Partie::plancher`) : `max(fait.pas, pas en cours + 1)`. *Faux dans une avancée de plusieurs pas, corrigé : voir « Lot 1, correction après la vérification », constat 1.*
- Pendant un pas inachevé : `regarder` rend les dernières vues valides sans rien lire ni noter ; `point` rend le dernier point pris ; `pause`, `repartir` et `finir_le_pas` finissent d'abord l'avancée laissée. `Partie::pas_en_cours`, `Partie::avancee_en_cours` (le dernier pas et le saut qui restent), `Partie::fin_du_dernier_pas` (jusqu'où une garde peut écrire le journal), `Avancee::en_cours`.
- Avec une fenêtre, la tête n'avance qu'à la fin d'une avancée, et une arrivée reçue pendant qu'elle restait en cours ne compte pour la tête qu'après elle (`poursuivre`) : sans cela, la vue de l'écran différait d'un découpage à l'autre (trouvé par la propriété ci-dessous). *Ne tenait pas pour une arrivée que la définitive jouait dans la même avancée : voir la correction, constat 1.*
- `Partie::etaler_mal` (caché) : le contrôle négatif `etale-mal`, une arrivée reçue pendant un pas inachevé qui y entre.

**La forme de la garde** (`partie::persistance`, `VERSION_GARDE` = 1). Un en-tête de cinq octets (la version en `u32` petit-boutiste, la sorte), puis postcard, relu strictement. Un point (sa chaîne `sain` et `periodique` aplatie dans l'ordre postfixe retourné, dédoublonnée, points et instantanés), un journal ou un morceau de journal (`encoder_lignes`, `raccorder`), l'ouverture, l'état du cœur. Les types gardés dérivent `Serialize`/`Deserialize`. L'instantané s'écrit par le trait `InstantaneGarde`, implémenté pour `moteur_wasmtime::InstantaneWasm` (empreinte, pages, mémoire entière, globales étiquetées ; relecture stricte) et pour le modèle des tests. `PointDeReprise::sain` et `periodique` se lisent.

**Une seule définition des budgets de la soirée** : `partie::BUDGETS_DE_SOIREE` ; `epreuve::BUDGETS` en est un alias, `epreuve::ARRIERE_PAS` relit `partie::ARRIERE_PAS`.

**L'épreuve.**
- La soirée joue un tour par pas simulé (`avancer_borne` à `ARRIERES_PAR_TOUR`), le script le rejoue (`script.rs`) ; elle ne lit ni l'état ni les vues pendant un pas inachevé (`apres_le_pas`), et finit le pas avant la forme canonique et un redémarrage de l'autorité.
- **E19 juge chaque tour** (`budgets.rs`) : l'image d'un tour est son coût et celui du tour suivant, plus, s'il a fini son avancée et que le jeu a une fenêtre, les entrées des pas `s + 2` à `s + fenêtre + 1` (le rejeu de la tête). Sans découpage, c'est exactement la somme de `(fenêtre + 2)` pas de la phase 2. La règle `d_un_arriere` (coût fixe des arriérés non compté), `RATTRAPAGE_PAS` et les retours de la soirée sont retirés.
- **E12 juge aussi le découpage d'un pas**, pour tout jeu (`proprietes.rs`, `regrouper` et `rejouer_decoupe` ; `epreuve.rs`, `decoupage`) : le script des propriétés, ses arrivées remontées juste après leur `Avancer`, rejoué en tours d'une entrée, doit donner le journal d'un seul tour, ligne pour ligne. Sans fenêtre et sans pas à découper, E12 reste sans objet (`ardoise`).
- **E14 reprend par la garde** (`soiree.rs`, `par_la_garde`) : chaque redémarrage de l'autorité, et chaque reprise de la fin, part d'un point et d'un journal écrits puis relus par `persistance` ; les reprises de la fin, d'un journal tronqué avant le point.
- La soirée longue dit la longueur de son journal, en lignes et en octets de la garde.
- `Plan::commun_fautif` et `CommunFautif::EtaleMal` : le contrôle négatif de E12.

### Les tests qui le tiennent

Dans `crates/partie/tests/coeur/` :
- `garde.rs` : `point_et_journal_aller_retour` ; `reprendre_depuis_un_point_decode_meme_empreinte_sur_le_modele` ; `reprendre_sur_un_journal_tronque` ; `une_partie_tronquee_continue_et_se_reprend` ; `les_morceaux_de_journal_se_raccordent` ; `une_garde_illisible_est_refusee` ; `un_point_incoherent_est_refuse` ; `les_octets_de_la_garde_sont_figes` (ouverture, et un journal qui a chaque sorte de ligne et chaque issue) ; la seconde lecture, sans serde ni postcard (SAL-90) : `une_seconde_lecture_relit_l_ouverture`, `…_le_journal`, `…_le_point` (et l'état du cœur) ; sous proptest (REM-64), `des_octets_au_hasard_ne_font_jamais_paniquer_la_garde` et `une_garde_abimee_ne_fait_jamais_paniquer`.
- `decoupage.rs` : `meme_journal_quel_que_soit_le_budget_de_tour` (proptest, CAT-14 : 128 cas, scripts de 1 à 80 opérations, budget de 1 à 3 entrées par tour, sans fenêtre ou avec une fenêtre de 1 à 4 pas que l'écran regarde, gestes reçus pendant le pas découpé, arriérés, refus, paniques ; même journal, même cœur, même état du jeu) ; `etale_mal_change_le_journal` (son contrôle négatif) ; `une_entree_fraiche_n_est_jamais_retenue` ; `les_arrieres_d_un_pas_se_jouent_sur_plusieurs_tours` ; `une_arrivee_pendant_un_pas_inacheve_va_au_pas_suivant` ; `un_pas_inacheve_ne_se_regarde_ni_ne_se_garde` ; `une_pause_finit_le_pas_inacheve`.

Dans `crates/epreuve/` :
- `tests/deux_instances`, `reprendre_depuis_un_point_decode_meme_empreinte` : chaque point de la soirée du module de `devine`, et son journal, relus de la garde (l'instantané de wasmtime compris), puis repris depuis le journal entier et depuis le journal tronqué avant le point : même cœur, même empreinte, même forme canonique.
- `tests/epreuve`, `etale_mal_fait_echouer_e12_et_lui_seul` (le socle sain, `CommunFautif::EtaleMal`) ; `le_socle_sain_passe_tout` vérifie que E12 y est réussi, avec des évaluations, par le découpage.
- `budgets.rs`, `l_arriere_de_l_epreuve_est_celui_du_coeur` ; `une_image_rare_et_lourde_echoue` couvre l'image d'un tour inachevé et d'un tour fini.
- Sur les trois jeux, `reprendre_depuis_un_point_decode_meme_empreinte` n'est pas un test du commun (leurs modules sont dans leurs dépôts) : c'est E14, qui reprend désormais par la garde, et qui doit être réussi au plan standard (ci-dessous).

### Les vérifications

Toutes le 2026-11-05, sur la machine de l'hôte, `CARGO_BUILD_JOBS=4`, une commande lourde à la fois. La charge était haute, de 5,7 à 18 (`vm.loadavg`), par d'autres programmes que ce lot (dont `coqchk`) ; la mémoire libre, de 31 à 42 % (`memory_pressure`). Les durées ci-dessous en dépendent ; les verdicts, non (le carburant ne dépend pas de la machine).

| Commande | Résultat |
| --- | --- |
| `cargo test --workspace --no-fail-fast` (09 h 34 à 09 h 46, en arrière-plan, `/usr/bin/time -l`) | code 0 ; 424 tests réussis, 0 échec, 1 ignoré (le `plan_standard` de la phase 2, inchangé) ; 749 s, compilation comprise ; 1,02 Go de mémoire résidente au plus |
| `cargo clippy --workspace --all-targets -- -D warnings` | aucun avertissement |
| `cargo fmt --all --check` | rien à reformater |
| `cargo test -p partie` | 115 + 8 tests réussis, dont la propriété du découpage (128 cas) |
| `soiree-epreuve ../games/punchline` (plan standard, `--release`, 09 h 09) | **code 0** ; 146 s ; 1,70 Go au plus |
| `soiree-epreuve ../games/sidequest` (plan standard, 09 h 12) | **code 0** ; 689 s ; 1,89 Go au plus |
| `soiree-epreuve ../games/tier-list` (plan standard, 09 h 30) | **code 0** ; 256 s ; 0,80 Go au plus. Une première passe (09 h 23) avait rendu 3 : E12 « pas exécuté », des pas découpés mais aucune arrivée juste après eux ; E12 compare désormais aussi les pas joués en plusieurs tours sans arrivée pendant eux, et le dit |

Ce que les rapports disent du découpage (plan standard) :

| Jeu | Tours de la boucle (soirées principales) | Tours qui ont laissé un pas inachevé | E12, découpage : tours d'une entrée inachevés, arrivées reçues pendant un pas découpé | E19 |
| --- | --- | --- | --- | --- |
| Punchline | 16 440 | 9 | 207, 150 | réussi |
| Sidequest | 16 440 | 50 | 458, 498 | réussi |
| Tier list | (jeu sans téléphone) | — | 132, 0 | réussi |

E14 reprend par la garde sur les trois jeux (20 évaluations chacun, réussi). La longueur du journal de la soirée longue est dans `MESURES-PHASE-3.md`, « Lot 1 » : 13 120 681 lignes, 253 Mo de garde chez Sidequest en six heures simulées, sans troncature.

Mémoire attendue au § 7 : « 2 à 3 Go pour `cargo test -p partie -p epreuve` » (estimé) ; mesurée : 1,02 Go au plus pour `cargo test --workspace` entier, et 1,89 Go au plus pour l'épreuve de Sidequest.

### Ce qui n'est pas fait, et pourquoi

- **`etale-mal` n'est pas une crate de module** : le découpage est le fait du cœur seul, aucun jeu ne sait le faire échouer. C'est un défaut injecté dans le commun, comme le moteur fautif de E20 ; PHASE-3.md, § 8.3, est corrigé.
- **Le redémarrage de l'autorité de l'épreuve finit d'abord le pas en cours.** Une vraie autorité qui meurt au milieu d'un pas perd ses arrivées figées que la garde n'a pas écrites (au-delà de `fin_du_dernier_pas`) ; elles reviennent par les renvois des appareils non accusés. Cela suppose l'accusé durable de la salle (lot 4b, lot 5) : l'épreuve ne l'a pas, et ne simule donc pas cette perte.
- **L'instantané de wasmtime se garde entier** (25,4 Mio chez Tier list) : l'instantané hors des quatre zones est au lot 2 (`moteur-commun`). Changer sa forme montera `VERSION_GARDE`.
- **Le plan standard des 35 jeux du commun** (`cargo test -p epreuve --test epreuve -- --ignored plan_standard`, 3 125 s en phase 2) n'a pas été relancé : le lot ne l'exige pas ; le plan court des fautifs (`tests/epreuve`) l'est.
- **L'arriéré se lit à l'âge de la vue seul.** Un geste ordinaire fait sur une vue restée la même plus d'une seconde compte comme un arriéré (revue de la phase 2, constat 11). Ici, cela ne fait que découper un pas qui en a plus de huit, sans changer le journal ; la règle de la phase 2, qui excluait leur coût fixe de E19, en souffrait davantage.

### Ce que le lot suivant doit savoir

- `Partie::avancer` finit toujours tout ; seul `avancer_borne` laisse un pas en cours. Code qui lit la définitive (`etat`, `canonique`, `definitive()`) pendant un pas inachevé lit le milieu d'un pas : vérifier `pas_en_cours()` d'abord, comme `soiree.rs`.
- Le moteur commun (lot 2) devra implémenter `InstantaneGarde` pour son `Instantane` hors zones, et monter `VERSION_GARDE` ; `tests/coeur/garde.rs` ne fige que les types de `partie`, l'instantané du modèle y passe pour opaque.
- `Avancee` a un champ de plus (`en_cours`) ; `Plan` aussi (`commun_fautif`, que le lot 3b peut étendre à `fausser_apres(n)`).
- La garde (lot 4b, 10a) écrit le journal jusqu'à `fin_du_dernier_pas()`, par morceaux (`encoder_lignes(base, pas, lignes)`), et les recolle par `raccorder`.

## Lot 1, correction après la vérification

Fait le 2026-11-05, de 10 h 54 à 12 h environ, sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Rien n'est commité. La vérification a relevé un constat bloquant, deux importants et quatre mineurs ; chacun a été vérifié avant d'être suivi.

### Ce qui est corrigé

**Constat 1 (bloquant) : la tête restait fausse pour de bon.** Reproduit (le cas de la vérification, écrit en test). Pendant une avancée inachevée, une arrivée était placée à `max(fait.pas, pas en cours + 1)`. Dans une avancée de plusieurs pas, elle entrait dans l'avancée dès le pas suivant, s'il était avant sa borne. Elle échappait alors au tri de la fin d'avancée (`poursuivre`), et la tête ne se rejouait jamais : à fenêtre 2 et rattrapage 3, l'écran montrait un total de 2 pour la tête contre 7 pour la définitive, au pas 9 puis encore au pas 14. Désormais, une arrivée reçue pendant une avancée inachevée va **après la fin de l'avancée**, `max(fait.pas, fin + 1)`, où `fin` est son dernier pas plus son saut (`Partie::plancher`, `AvanceeEnCours::fin`) : la même place que si l'avancée avait fini d'un seul tour. Elle attend donc toute l'avancée dans `en_attente`, et la fin de l'avancée la juge pour la tête comme avant. PHASE-3.md, § 2.11, est corrigé, ainsi que le pseudo-code du § 2.9 (l. 150), `CHANGELOG.md` et la documentation de `pas_en_cours` et de `PasEnCours`.

**Constat 2 (important) : la propriété du découpage avait été restreinte en silence.** Exact : elle n'injectait d'arrivées que pendant le dernier pas d'une avancée sans saut, et ne comparait pas les vues de la tête. C'est la règle même du § 2.11 qui contredisait sa phrase « le même journal quel que soit le découpage ». Elle injecte maintenant ses gestes et ses tenus pendant l'avancée laissée, à n'importe lequel de ses pas et avant son saut, et compare aussi les vues de l'écran (`meme_journal_quel_que_soit_le_budget_de_tour`, 128 cas). Le rejeu découpé de E12 (`rejouer_decoupe`) n'a plus de restriction non plus, et compare aussi les vues ; son rapport compte les arrivées reçues avant le dernier pas de l'avancée ou son saut. Deux contrôles négatifs faits à la main, puis retirés : remettre l'ancienne règle (`p.pas` dans `plancher`) fait rougir la propriété et `une_arrivee_au_milieu_d_une_avancee_va_apres_elle` ; retirer le rejeu de la tête en fin d'avancée fait rougir les mêmes. Le premier a laissé une graine de régression dans `tests/proptest-regressions/decoupage.txt`, gardée.

**Constat 3 (important) : les octets figés ne couvraient qu'une partie de la forme.** Exact. Nouveau `tests/coeur/forme_garde.rs`, qui fige :
- chaque variant de `QuoiPlace`, `EcranArrivee`, `ComposantArrivee`, `ReponseArrivee` (avec `Ok` et `Err`), `Origine` et `Faute` ;
- un `EtatCoeur` qui a deux fiches annoncées, un tenu, une manette, une demande lancée et une vue ;
- un point dont la chaîne a trois points et deux instantanés (point sain et point à intervalle distincts).

Chaque cas a ses octets écrits et sa seconde lecture sans serde ni postcard (SAL-90). `les_octets_figes_couvrent_chaque_variant` compare la liste des variants à ce que les octets figés tiennent, et les `nom_*` sont des `match` sans `_` : un variant ajouté ne compile plus tant qu'il n'a pas ses octets. Pour l'instantané de wasmtime, trois tests dans `moteur-wasmtime/src/interieur.rs` : `les_octets_de_l_instantane_sont_figes`, `une_seconde_lecture_relit_l_instantane` et `un_instantane_abime_est_refuse`. Contrôle négatif fait à la main, puis retiré : un champ ajouté à `Fiche` fait rougir `les_octets_du_coeur_et_du_point_sont_figes` et `une_seconde_lecture_relit_le_coeur_et_le_point_pleins`.

**Constat 5 (mineur) : Tier list n'éprouvait pas les arrivées pendant un pas découpé.** E12 coupe désormais aussi les rafales (`proprietes::couper_les_rafales`). Une rafale d'au moins trois arrivées jouée par un `Avancer`, qu'aucune arrivée ne suit avant l'`Avancer` d'après, voit sa seconde moitié passer juste après son `Avancer`. Le script coupé est rejoué des deux façons. Chez Tier list, cela donne 2 arrivées pendant un pas découpé, au lieu de 0. C'est peu : ses pas découpés viennent surtout de pas à deux arrivées. Le test est `une_rafale_que_rien_ne_suit_est_coupee`.

**Constat 6 (mineur) : E15 lisait le milieu d'un pas.** Exact. `mesurer_la_memoire` (`soiree.rs`) regarde maintenant `pas_en_cours()` : pendant un pas inachevé, la mesure est remise au tour suivant (`memoire_due`).

**Un test qui ne finissait pas.** La première écriture de `une_arrivee_au_milieu_d_une_avancee_va_apres_elle` avançait par l'aide `commun::avancer`, qui suit le pas définitif. Avec une fenêtre, ce pas reste à deux pas derrière l'horloge, et la boucle ne finissait jamais (la mémoire montait, le processus a été tué). Le test avance maintenant l'horloge pas à pas.

### Ce qui n'est pas suivi, et pourquoi

- **Constat 4** : le test nommé `reprendre_depuis_un_point_decode_meme_empreinte` ne joue que `devine`. Les modules des trois jeux sont dans leurs dépôts, et un test du commun qui en dépendrait lierait le commun aux jeux. Pour eux, c'est E14 qui tient la preuve : il reprend par la garde, instantané de wasmtime compris, avec le journal tronqué avant le point (20 évaluations réussies chez chacun, ci-dessous). C'est une substitution, dite.
- **Constat 7** : `etale-mal` reste « une arrivée reçue pendant un pas inachevé entre dans ce pas ». Le défaut qu'écrivait la colonne (« un arriéré placé avant une entrée plus ancienne du même appareil ») ne peut pas venir du découpage. L'ordre d'un appareil se garde au placement, par `regles::suivre`, avant tout découpage, et un pas découpé garde l'ordre de ses entrées. Le défaut que le découpage peut vraiment produire, c'est une arrivée qui entre dans le pas en cours. L'ordre d'un même appareil à travers un pas découpé est, lui, comparé par la propriété : jusqu'à trois appareils y font plusieurs gestes. Il ne l'est pas par un contrôle négatif propre.
- E12 sur les trois jeux ne voit jamais d'arrivée avant le dernier pas d'une avancée : la soirée des propriétés avance d'un pas par `Avancer`. Ce cas n'est tenu que par la propriété du cœur (avec et sans fenêtre, sauts compris) et par `une_arrivee_au_milieu_d_une_avancee_va_apres_elle`.
- Les types de l'ABI contenus dans la garde (capacités, manette, touche, pointeur, présence, échec d'une demande) sont écrits octet par octet dans les octets figés. Mais la seconde lecture les compare à leurs octets d'ABI (`jeu_abi::encoder`), sans les relire champ à champ : leur forme est figée par `jeu-abi` (`forme_figee`).

### Les vérifications

Toutes le 2026-11-05, `CARGO_BUILD_JOBS=4`, une commande lourde à la fois, mémoire libre de 41 à 69 % (`memory_pressure`) et charge de 3 à 21 (`vm.loadavg`), à cause d'autres programmes.

| Commande | Résultat |
| --- | --- |
| `cargo test --workspace --no-fail-fast` (11 h 12 à 11 h 19, en arrière-plan, `/usr/bin/time -l`) | code 0 ; 435 réussis, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 412 s ; 2,06 Go de mémoire résidente au plus |
| `cargo test -p epreuve --no-fail-fast`, après le seuil des rafales abaissé à 3 (11 h 36 à 11 h 42) | code 0 ; 110 réussis, 1 ignoré ; 366 s ; 1,50 Go au plus |
| `cargo clippy --workspace --all-targets -- -D warnings`, puis `-p epreuve` après le dernier changement | aucun avertissement |
| `cargo fmt --all --check` | rien à reformater |

`soiree-epreuve`, plan standard, `--release`, en arrière-plan, l'un après l'autre, avec le code final (seuil des rafales à 3) :

| Jeu | Heure | Code | Durée | Mémoire max | E12, découpage : tours d'une entrée inachevés ; arrivées pendant un pas découpé ; dont avant la fin de l'avancée ; passées après le pas de leur rafale | E14 | E19 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Punchline | 11 h 42 | **0** | 113 s | 1,96 Go | 186 ; 188 ; 0 ; 180 | 20 évaluations, réussi | réussi (9 tours inachevés sur 16 440) |
| Sidequest | 11 h 44 | **0** | 492 s | 2,61 Go | 424 ; 593 ; 0 ; 466 | 20 évaluations, réussi | réussi (50 tours inachevés sur 16 440) |
| Tier list | 11 h 33 | **0** | 174 s | 1,69 Go | 132 ; 2 ; 0 ; 2 | 20 évaluations, réussi | réussi |

Une première passe des trois (11 h 19 à 11 h 32), avec un seuil de rafale à 4, rendait aussi 0 partout. Tier list y avait toujours 0 arrivée pendant un pas découpé, d'où le seuil abaissé.

La soirée longue de Sidequest donne maintenant 13 120 680 lignes et 253 116 411 octets, au lieu de 13 120 681 et 253 116 416 au lot 1 : la nouvelle place des arrivées reçues pendant une avancée inachevée change à peine le journal. Punchline ne bouge pas (5 278 242 lignes, 151 838 722 octets). `MESURES-PHASE-3.md`, « Lot 1 », le dit. La mémoire maximale de l'épreuve de Sidequest (2,61 à 2,71 Go, contre 1,57 à 1,89 Go au lot 1) n'a pas été expliquée. La seule différence connue est le second rejeu de E12 sur le script coupé, qui reste de la même taille.

Aucun navigateur ni serveur n'a été lancé. Toutes les tâches en arrière-plan sont finies.

### Ce que le lot suivant doit savoir

- Une arrivée reçue pendant une avancée inachevée est placée après sa fin (`Partie::avancee_en_cours` donne la borne et le saut). La garde (lots 4b et 10a) doit garder cette règle : les arrivées d'une avancée laissée en cours s'écrivent au journal avec le pas où elles seront jouées, qui est au-delà de cette fin.
- Un type gardé qui change de forme fait rougir `forme_garde.rs` (ou, pour l'instantané de wasmtime, les tests de `interieur.rs`). Un variant ajouté à une enum gardée ne compile plus dans `forme_garde.rs`. Le moteur commun du lot 2 aura besoin, pour son instantané hors zones, des mêmes trois tests que ceux de `interieur.rs`.
- E12 juge le découpage sur un script dont les rafales sont coupées (`couper_les_rafales`) : ce n'est plus tout à fait le script de la soirée des propriétés. Les deux rejeux, d'un seul tour et découpé, partent du même script coupé.

## Lot 2 — Le moteur en deux

Fait le 2026-11-05, de 12 h 15 à 13 h 10 environ, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Rien n'est commité.

### Ce qui est fait

**`crates/moteur-commun`, nouveau** (P2), `no_std`, sans `unsafe`, sans entrées-sorties. Il prend de l'ancien `moteur-wasmtime/src/interieur.rs` et de `lib.rs` tout ce qui ne dépend pas de wasmtime :
- le trait `Machine` (`neuve`, `pages`, `lire`, `ecrire`, `remplir_de_zeros`, `grandir_jusqu_a`, `globale`, `poser_globale` par nom, `boite`, `appel`), `Bits` et le piège brut `Piege` (`HorsMemoire`, `PileNative`, `Interrompu`, `Autre`, `Moteur`) ;
- `Protocole<M: Machine>`, une `partie::Instance` : le déroulé d'un appel (§ 3.4), le classement d'un piège dans l'ordre d'avant, la pile vérifiée, l'empreinte (mêmes octets hachés qu'avant), `instances_creees`, `memoire_max` ;
- `Plan` (ce qu'on sait d'un module réécrit avant `Bonjour`) et `Carte` (ses zones, `soiree_memoire_max`, `remises_a_zero`, `octets_remis_a_zero`, `plages`) ;
- **l'instantané hors des quatre zones** (`Instantane`, P3) : la mémoire que la pile, la boîte, le brouillon et la zone de panique laissent, en plages ; la restauration vérifie que les plages sont exactement le reste des zones du module, remet les zones à zéro, et demande une machine neuve si la mémoire est plus grande. Sa forme gardée (`InstantaneGarde`) : empreinte, pages, plages (adresse, longueur, octets), globales. **`VERSION_GARDE` passe à 2** ; les octets figés de `partie` n'ont pas changé (seul leur en-tête) ;
- `verifier_chargement` (`Bonjour` deux fois, zones, `Decrire`, accord de la déclaration), `Charge` (avec le carburant de `Bonjour` et `Decrire`), `RefusModule` (et `From<reecriture::Refus>`), `globales_d_origine`, `BUDGET_DE_CHARGEMENT` ;
- `essais::BONJOUR_QUI_BOUCLE` : le module de `Bonjour` qui boucle, en `wat`, dans `crates/moteur-commun/essais/bonjour-qui-boucle.wat`, là où le banc le lira (lot 3c). Le test de `charger.rs`, qui a sa propre colle, reste tel quel (les tests de `moteur-wasmtime` sont inchangés).

**`moteur-wasmtime` réduit** à `src/machine.rs` (`impl Machine` sur wasmtime : magasin, exports, échéance de l'époque posée avant `soiree_boite` ; *corrigé ensuite*, voir « Lot 2, correction »), `src/instance.rs` (le fil de chaque instance, qui porte un `Protocole<MachineWasm>` ; la sonde du chargement), `src/horloge.rs` (inchangé) et `lib.rs` (`Moteur`, `ReglagesMoteur`, `ModuleCharge`). `interieur.rs` est retiré. Son API publique ne change pas ; `InstantaneWasm` est un alias de `moteur_commun::Instantane`, `RefusModule` et `BUDGET_DE_CHARGEMENT` sont réexportés. Nouveaux : `ModuleCharge::carte`, `ModuleCharge::carburant_du_chargement`, l'exemple `boucle_vide` (M10).

**`BUDGET_DE_CHARGEMENT` baissé de 2³² à 2²⁶** (P5, la voie préférée ; RESTE-PHASE-2, point 16 coché). `ReglagesMoteur` ne change pas : le garde-fou de chargement reste celui de `PHASE_1`, 1 000 ms. Raisons mesurées ci-dessous : `Bonjour` et `Decrire` brûlent au plus 9 322 unités ; une unité de boucle vide coûte jusqu'à 1 492 ps sous quatorze `yes`, donc 2²⁶ unités au plus 100 ms environ.

**Ailleurs** : `epreuve/src/sources.rs` compte `moteur-commun` parmi les crates du commun ; `epreuve/examples/etalonner.rs` dit le carburant du chargement ; `Cargo.toml` du commun déclare le membre et la dépendance ; dans **Tier list** (`games/tier-list/crates/tier-list/tests/sauvegarde_lourde.rs`), le test de mesure `le_point_du_plus_lourd_plateau_se_mesure` (M3), qui reprend le plus lourd plateau de ce fichier (`mesurer` rend désormais aussi l'instance de la fin et son module). Les `Cargo.lock` des trois jeux gagnent `moteur-commun` (Cargo l'a écrit : `moteur-wasmtime`, pris par chemin, le tire) ; à committer dans chaque dépôt.

Documents : `PHASE-3.md` (§ 2.2, § 2.3, § 2.4, P3, P5, l'annexe des valeurs), `RESTE-PHASE-2.md` (point 16), `MESURES-PHASE-3.md` (« Lot 2 »), `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`.

### Les tests qui le tiennent

- `moteur-commun`, sur une machine fausse en pur Rust (`tests/commun/fausse.rs`, qui répond à `Bonjour` et `Decrire` comme la colle) : `classer_garde_son_ordre` (les 35 paires de constats qui peuvent aller ensemble, dans les deux sens : le premier dans l'ordre l'emporte) et `chaque_constat_seul_donne_son_issue` ; `instantane_sans_zones_meme_empreinte` (exactement la mémoire hors des zones, zones salies puis remises à zéro à la restauration, même empreinte, même avenir) ; `restaurer_dans_plus_grand_cree_une_machine_neuve` ; `un_instantane_d_une_autre_forme_est_refuse` ; `le_module_de_bonjour_qui_boucle_se_reecrit`. Dans `src/` : les octets figés de l'instantané (`les_octets_de_l_instantane_sont_figes`), sa seconde lecture sans `lire` (SAL-90, `une_seconde_lecture_relit_l_instantane`), `un_instantane_abime_est_refuse`, et sous proptest `une_relecture_ne_panique_jamais` (REM-64) ; `les_plages_sont_le_reste_des_zones`.
- `moteur-wasmtime/tests/moteur` : les 31 tests d'avant de ce binaire, **sans une ligne changée** (`git diff crates/moteur-wasmtime/tests` ne montre que `main.rs`, qui déclare deux fichiers), et verts ; plus `sans_zones::instantane_sans_zones_meme_empreinte` (wasmtime, une colle qui salit les quatre zones : aucune plage ne touche une zone, la taille est la mémoire moins les zones, relu de la garde puis restauré, même empreinte et même avenir) et `chargement_borne::bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement` (le module d'essai sous `PHASE_1` : faute `Budget`).
- `epreuve/tests/deux_instances` : les quatre tests d'avant, verts, et `le_chargement_tient_largement_son_budget` (le chargement de `devine` sous un millième du budget, une marge choisie).
- **Les empreintes de la soirée de `devine` sont identiques avant et après.** Comparé une fois, par un test temporaire retiré ensuite (il écrivait dans un fichier, sous `SOIREE_EMPREINTES`) : le journal entier de la partie du module (lignes, empreintes notées), les indices des points, l'empreinte finale, puis la soirée rejouée entrée par entrée sur une instance nue, avec l'empreinte et le carburant après chacune des 340 entrées, et l'empreinte d'une instance neuve restaurée de l'instantané pris à l'entrée 100. Avant le changement (12 h 20) et après (12 h 32) : les deux fichiers (504 030 octets) sont identiques octet pour octet (`cmp`). L'instantané a changé de forme sans changer d'empreinte.
- **Contrôles négatifs**, faits à la main : remettre `BUDGET_DE_CHARGEMENT` à 2³² fait rougir `bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement` sans aucune charge (`Panne(PanneOutil { cause: GardeFouMural, … })`), puis remis à 2²⁶. `classer_garde_son_ordre` est son propre contrôle : chaque paire est jouée dans les deux sens.

### Les vérifications

Le 2026-11-05, `CARGO_BUILD_JOBS=4`, une commande lourde à la fois, mémoire libre de 53 à 57 % (`memory_pressure`), charge de 1,6 à 7,8 hors des mesures sous `yes` (d'autres programmes tournaient).

| Commande | Résultat |
| --- | --- |
| `cargo test --workspace --no-fail-fast` (12 h 41 à 12 h 46, en arrière-plan, `/usr/bin/time -l`) | code 0 ; 446 réussis, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 290 s ; 1,25 Go de mémoire résidente au plus |
| `cargo clippy --workspace --all-targets -- -D warnings` | aucun avertissement |
| `cargo fmt --all --check` (commun et Tier list) | rien à reformater |
| `cargo test -p moteur-wasmtime -p moteur-commun` | 33 + 11 réussis |
| `cargo test -p epreuve --test deux_instances` (dans le précédent) | 5 réussis |
| `chargement_borne` sous quatorze `yes` (le binaire de test seul) | 10 sur 10 à 5 s de charge, 20 sur 20 à 45 s (charge 21) |

`soiree-epreuve`, plan standard, `--release`, depuis les sources, l'un après l'autre en arrière-plan :

| Jeu | Heure | Code | Durée | Mémoire max | E12 | E14 | E19 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Punchline | 12 h 46 | **0** | 106 s | 1,79 Go | réussi | 20 évaluations, réussi | réussi |
| Sidequest | 12 h 48 | **0** | 504 s | 2,91 Go | réussi | 20 évaluations, réussi | réussi |
| Tier list | 12 h 56 | **0** | 168 s | 1,24 Go | réussi | 20 évaluations, réussi | réussi |

La soirée longue donne les mêmes journaux qu'après la correction du lot 1 : Punchline 5 278 242 lignes et 151 838 722 octets, Sidequest 13 120 680 lignes et 253 116 411 octets. Les empreintes n'entrent pas dans ce compte ; leur égalité est vérifiée sur `devine` (ci-dessus).

Les tests propres des trois jeux, qui chargent leur module par `moteur-wasmtime` (`cargo test` dans chaque dépôt, 13 h 00 à 13 h 01) : tous verts, Punchline 77, Sidequest 66, Tier list 90 réussis, aucun échec. `cargo clippy --all-targets -- -D warnings` de Tier list, après l'ajout de son test de mesure : aucun avertissement.

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 2 », avec leurs commandes :
- **Le carburant du chargement** des cinq modules : `Bonjour` de 3 104 à 3 194 unités, `Decrire` de 7 304 à 9 322, `Creer` au plus 343 002 (Sidequest, plan court).
- **M10 dans wasmtime** : une unité de boucle vide coûte de 481 à 539 ps peu chargée (2³² unités, 7 chargements), jusqu'à 1 492 ps sous quatorze `yes` (médiane 1 249) ; à 2²⁶, de 233 à 1 033 ps selon la charge, la boucle prenant de 16 à 70 ms.
- **M3 dans wasmtime, Tier list** : plateau neuf, 323 584 octets hors des zones (le calcul de P3, 0,31 Mio, tient) ; **pire cas (plateau plein, quatre images de 256 Kio, sauvegarde écrite puis relue), 17 625 088 octets (16,8 Mio)**, gardés en 17 625 193, pour une mémoire de 670 pages. Avant ce lot, l'instantané aurait copié 43 909 120 octets.
- Mémoire : estimée à 3 à 4 Go au § 7 ; mesurée 1,25 Go au plus pour `cargo test --workspace`, 2,91 Go pour l'épreuve de Sidequest, 0,73 Go pour `sauvegarde_lourde` de Tier list.

### Ce qui n'est pas fait, et pourquoi

- **`Creer` reste sous son propre budget**, `BUDGETS_DE_SOIREE.creation` (2²⁸), que le lot ne change pas : `Creer` est un appel de la partie, pas du chargement, contrairement à ce que P5 groupait. Sous charge, une boucle dans `Creer` y prend au plus 400 ms à 1 492 ps par unité, sous le garde-fou d'une seconde, avec un facteur 2,5 seulement (calculé, non éprouvé par un test). Dit dans PHASE-3, § 2.3.
- **Le garde-fou de chargement n'est pas dérivé** : la voie préférée (le budget baissé) suffit dans wasmtime. Les navigateurs (M10 au lot 3c) peuvent demander l'autre voie, ou un budget plus bas encore.
- ~~**Le comportement du garde-fou change un peu** : son échéance se pose avant `soiree_boite`, puis de nouveau avant `soiree_appel` ; un appel peut donc durer jusqu'à deux fois le garde-fou si sa boîte en prend presque tout (une boîte ne fait qu'allouer). Aucun test n'en dépend.~~ *Corrigé après la vérification* (« Lot 2, correction », constat 3) : l'échéance se pose une fois, dans `boite`, comme avant.
- **M3 est cinquante-cinq fois le calcul.** Ce lot mesure, il ne décide pas : R2 (un second Worker si le chiffrement dépasse 2 ms), P8 et P9 (la garde au relais, des blocs immuables) se jugent sur 16,8 Mio au lot 11. La cause probable, lue et non prouvée : le tas de `dlmalloc` garde la place des grands tampons de la sauvegarde, que l'instantané garde avec lui.
- **Le test des empreintes avant et après n'est pas resté** : il compare deux versions du moteur, et la version d'avant n'existe plus. Un test d'empreintes figées dépendrait des octets que rustc produit pour `devine`.
- Le plan standard des 35 jeux du commun (`plan_standard`) n'a pas été relancé : le lot ne l'exige pas.

### Ce que le lot suivant doit savoir

- `moteur-navigateur` (lot 3a) n'écrit que `impl moteur_commun::Machine` : `Plan::new(&reecrit, &reglages, &globales_d_origine(noms des exports))`, puis `verifier_chargement(&mut machine_sonde, plan)` rend la `Charge` (et sa `Carte`), puis `Protocole::new(machine, Arc::new(carte))` est une `partie::Instance`. Un `RangeError` ou un `InternalError` devient `Piege::PileNative` ; une erreur qui n'est pas un piège du module, `Piege::Moteur`. Les globales se lisent et se posent par leur nom (celles du moteur, de `reecriture`, et `soiree_globale_<n>`).
- `Machine::neuve` est obligatoire : restaurer un instantané plus petit que la mémoire actuelle la demande.
- Le banc lit `moteur_commun::essais::BONJOUR_QUI_BOUCLE` (texte `wat`, à lire par `wat::parse_str`) pour M10 dans les navigateurs ; `cargo run --release -p moteur-wasmtime --example boucle_vide` est la mesure de référence dans wasmtime.
- Un instantané relu de la garde ne connaît pas les zones : c'est `Protocole::restaurer` qui refuse des plages qui ne sont pas le reste des zones du module.
- `VERSION_GARDE` vaut 2 : les octets figés de l'instantané sont dans `crates/moteur-commun/src/instantane.rs`.

## Lot 2, correction après la vérification

Fait le 2026-11-05, de 13 h 23 à 14 h environ (heure de la machine, GMT), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Rien n'est commité. La vérification a relevé un constat important et cinq mineurs ; chacun a été vérifié avant d'être suivi.

### Ce qui est corrigé

**Constat 1 (important) : la comparaison des empreintes ne portait pas sur le code livré.** Exact. `empreintes-apres.txt` date de 12 h 32 ; `chargement.rs` (le budget à 2²⁶) a changé à 12 h 37, puis cinq fichiers à 12 h 40 (un `cargo fmt`). La comparaison est **refaite sur le code final**, avec la correction du constat 3 en plus, à **13 h 27 GMT**. Le test temporaire avait été retiré ; il est réécrit d'après le fichier d'avant, sans en connaître le texte, et ajouté un moment à la fin de `epreuve/tests/deux_instances/main.rs` (`empreintes_temporaires`, sous `SOIREE_EMPREINTES`). Il écrit :
- les lignes d'empreinte du journal du module, l'indice de chaque point, l'empreinte finale, le journal entier ;
- puis, sur une instance nue (`Bonjour`, `Decrire`, `Creer`), l'empreinte après `Creer`, et après chacune des 340 entrées non écartées : l'empreinte, le carburant, la longueur du retour ;
- enfin l'empreinte d'une instance neuve (`Bonjour`, `Decrire`) restaurée de l'instantané pris après l'entrée d'ordre 100.

Résultat : 504 030 octets, **identiques octet pour octet** (`cmp`) à `empreintes-avant.txt`, écrit à 12 h 20 par l'ancien moteur (avant la naissance de `moteur-commun`, 12 h 23). Un test écrit à nouveau qui retrouve le même fichier montre aussi que la forme et le sens de la comparaison sont ceux d'avant. Le test est retiré ensuite : `main.rs` est remis à l'octet près depuis sa copie (`git diff` n'y montre que les 17 lignes de `le_chargement_tient_largement_son_budget`). Il n'est pas gardé, pour la raison déjà dite : l'ancien moteur n'existe plus.

**Constat 3 (mineur) : le garde-fou mural bornait la boîte et l'appel chacun.** Exact, et évitable. `MachineWasm::echeance` est maintenant appelée une seule fois, dans `boite` ; `appel` ne la repose pas. Un appel du protocole, `soiree_boite` et `soiree_appel` ensemble, est de nouveau borné par `garde_fou_murale_ms`, comme avant le lot et comme le dit INTERFACE, § 4.6. Le contrat est écrit dans la documentation du trait `moteur_commun::Machine`, pour que `moteur-navigateur` le suive : l'échéance se pose dans `boite`, et le protocole appelle toujours `boite` avant `appel`. Mis à jour : PHASE-3, § 2.3 ; `CHANGELOG.md` ; `crates/README.md` ; la documentation de `moteur-wasmtime/src/lib.rs`. **Aucun test ne tient cette différence.** Il faudrait une boîte et un appel qui prennent chacun un peu plus de la moitié du garde-fou, donc un test qui dépend de l'horloge ; il n'est pas écrit.

**Constat 4 (mineur) : cinq mutants vivants dans `protocole.rs`.** Reproduit en lisant le code, puis tenu par deux tests nouveaux sur la machine fausse :
- `ordre::les_bornes_exactes_ne_sont_pas_des_fautes` : un piège qui laisse le carburant à 0 tout juste, ou la profondeur à `profondeur_max` tout juste, n'est ni `Budget` ni `Profondeur`. C'est la règle de la réécriture : `i64.lt_s` contre 0 pour le carburant, `i32.gt_u` pour la profondeur (`reecriture/src/passes.rs`). Une unité plus loin, chacune redevient sa faute.
- `instantane::restaurer_dans_plus_petit_fait_grandir` : un instantané de 5 pages restauré dans une instance de 3 pages. La mémoire grandit, sans machine neuve ; on retrouve la même taille, la même empreinte, le même instantané et le même avenir. À taille égale, la restauration ne demande aucune croissance : dans un navigateur, `memory.grow`, même de zéro page, détache le tampon de la mémoire. La machine fausse compte donc ses croissances (`Fausse::croissances`).

cargo-mutants, `--jobs 1`, `-p moteur-commun --file crates/moteur-commun/src/protocole.rs --re 'classer_ou_panne|plages|restaurer'` (un filtre un peu plus large que celui de la vérification) : **34 mutants, 32 attrapés, 2 non viables, aucun manqué**, en 81 s. Les cinq de la vérification (373:30 `<` en `<=`, 376:31 `>` en `>=`, 590:18 `<` en `==`, `>` et `<=`) sont attrapés.

**Constat 2 (mineur) : « les 31 tests d'avant » taisait trois tests retirés.** Exact. `moteur-wasmtime` avait 34 tests : les 31 de `tests/moteur/`, inchangés, et 3 tests unitaires dans `src/interieur.rs` (`les_octets_de_l_instantane_sont_figes`, `une_seconde_lecture_relit_l_instantane`, `un_instantane_abime_est_refuse`). Ces trois-là sont partis avec ce fichier. Ils sont réécrits sous les mêmes noms dans `moteur-commun/src/instantane.rs`, pour la forme gardée de la version 2, avec en plus le proptest `une_relecture_ne_panique_jamais`. C'était inévitable : la forme gardée change. La section « Lot 2 » dit maintenant « les 31 tests d'avant de ce binaire ». La ligne du lot 2 au § 7 de PHASE-3 le note aussi (« écrit après le lot »).

### Ce qui n'est pas suivi, et pourquoi

- **Constat 5** : le contrôle négatif à 2³² n'est pas refait. Il demande de modifier le code, et rien de ce que la correction touche ne le change : le budget du chargement est le même, et le garde-fou ne peut que couper plus tôt qu'avant la correction. Le module d'essai de `moteur-commun/essais/` est une **copie** autonome, pas un déplacement : `charger::bonjour_qui_boucle_refuse_avec_sa_faute` garde sa propre colle (déjà dit dans la section « Lot 2 »).
- **Constat 6** : rien à corriger. `Creer` garde son budget de 2²⁸, avec une marge de 2,5 sous le garde-fou d'une seconde. C'est calculé, non éprouvé, et déjà dit. La condition de P5 tient : `Creer` brûle au plus 343 002 unités, loin sous 2²⁶.

### Les vérifications

Le 2026-11-05, `CARGO_BUILD_JOBS=4`, une commande lourde à la fois, mémoire libre de 43 à 57 % avant chacune (`memory_pressure`). Le test temporaire est écrit dans le scratchpad, sous `empreintes-correction.txt` ; la copie de `main.rs` y est aussi.

| Commande | Résultat |
| --- | --- |
| `cargo test -p moteur-commun` | 5 + 8 réussis, dont les deux nouveaux |
| `cargo mutants --jobs 1 …` (ci-dessus) | 34 mutants : 32 attrapés, 2 non viables, 0 manqué |
| test temporaire des empreintes (13 h 27) puis `cmp` | identiques, 504 030 octets |
| `cargo fmt --all --check` (commun et Tier list) | rien à reformater |
| `cargo clippy --workspace --all-targets -- -D warnings` ; Tier list `cargo clippy --all-targets -- -D warnings` | aucun avertissement |
| `cargo check -p moteur-commun --target wasm32-unknown-unknown` | compile |
| `cargo test --workspace --no-fail-fast` (13 h 28 à 13 h 33, en arrière-plan, `/usr/bin/time -l`) | code 0 ; **448 réussis** (446 et les deux nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 278 s ; 1,14 Go de mémoire résidente au plus |
| `chargement_borne`, le binaire de test seul, 20 fois, sous quatorze `yes` lancés 30 s avant (charge de 3,6 à 15,8) | 20 réussites sur 20 |
| `cargo test` dans chaque jeu | Punchline 77, Sidequest 66, Tier list 90 réussis, aucun échec |
| Tier list, `cargo test --test sauvegarde_lourde le_point -- --nocapture` | réussi ; mêmes chiffres : 670 pages, 17 625 088 octets en 3 plages, gardés en 17 625 193 ; plateau neuf 406 pages, 323 584 octets |

`soiree-epreuve`, plan standard, `--release`, depuis les sources, l'un après l'autre en arrière-plan :

| Jeu | Heure | Code | Durée | Mémoire max | E12 | E14 | E19 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Punchline | 13 h 38 | **0** | 104 s | 2,06 Go | réussi | 20 évaluations, réussi | réussi |
| Sidequest | 13 h 40 | **0** | 558 s | 1,64 Go | réussi | 20 évaluations, réussi | réussi |
| Tier list | 13 h 49 | **0** | 258 s | 0,94 Go | réussi | 20 évaluations, réussi | réussi |

Les rapports sont identiques, ligne à ligne, à ceux de la vérification (13 h 08 à 13 h 22), compteurs du monde simulé et coût fixe compris. Seule la ligne de `/usr/bin/time` diffère. Les durées sont plus longues que celles de la vérification, sous une charge plus forte : d'autres programmes tournaient, et la mémoire libre est descendue à 43 %.

Aucun navigateur ni serveur lancé. Plus aucun `yes` ni aucune tâche de fond ne tourne.

### Ce que le lot suivant doit savoir

- `moteur-navigateur` pose l'échéance de son garde-fou dans `Machine::boite`, et non dans `appel` : un appel du protocole est borné en entier.
- `Fausse::croissances` compte les `grandir_jusqu_a` de la machine fausse. Une restauration à taille égale n'en fait aucune, et `moteur-navigateur` doit faire de même : un `grow(0)` détache le tampon.

## Lot 3a — Le moteur du navigateur

Fait le 2026-11-05, de 14 h 40 à 16 h 10 environ (heure de la machine, GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), avec Chrome 156.0.8078.80. Rien n'est commité.

### Ce qui est fait

**Le second workspace, `soiree/web/`** (P31) : `Cargo.toml` (membres nommés un à un, `exclude = ["temoins"]`, les crates du commun par chemin, les dépendances tierces avec leur raison, les profils `dev.package."*"` et `release` du commun), `Cargo.lock` (amorcé depuis celui du commun : les crates partagées y ont les mêmes versions), `rust-toolchain.toml` et `clippy.toml` copiés du commun, et `[workspace.lints]` recopié à l'identique.

**`web/moteur-navigateur`** (`#![forbid(unsafe_code)]`) :
- `MachineNavigateur`, l'`impl moteur_commun::Machine` sur `WebAssembly.Module`, `Instance`, `Memory` et `Global` (`src/machine.rs`). La mémoire passe par un `Uint8Array` posé sur `memory.buffer`, relu à chaque opération ; `remplir_de_zeros` par `Uint8Array::fill`. `grandir_jusqu_a` ne demande rien à taille égale ou moindre (`grow(0)` détache le tampon), et appelle `Memory.prototype.grow` par `Reflect` : un refus revient en `CroissanceRefusee` au lieu de traverser le module. Une globale se lit par `value`, se pose par `Reflect.set` (une globale immuable, ou d'un autre type, est une panne) ; les `i64` par `BigInt`, exacts. `boite` et `appel` passent par `Function::call*`, qui rendent l'exception levée ; `appel` relit son `i64` par `BigInt`. Pas de garde-fou mural dans la machine : elle ne rend jamais `Piege::Interrompu`.
- `ModuleNavigateur` : le module compilé (`new WebAssembly.Module`, synchrone) et **le type de chaque globale exportée, lu par `wasmparser`** (`src/types.rs`) : le navigateur ne le dit pas, et un `i32`, un `f32` et un `f64` y sont tous un nombre.
- `classer` (`src/pieges.rs`) : une erreur levée pendant un appel, ramenée à un piège brut. `RuntimeError` par son message : la table `MESSAGES` (dix messages de V8, constatés) ramène chacun au nom que wasmtime donne au même piège (`codes`) ou à `HorsMemoire` ; `AMBIGUS` (`float unrepresentable in integer range`, que V8 donne à deux pièges que wasmtime distingue) garde le texte du navigateur, précédé de `navigateur : ` ; un message inconnu aussi. `RangeError`, et `InternalError: too much recursion` (Firefox, lu, non constaté), donnent `PileNative` ; toute autre erreur, `Piege::Moteur`.
- `charger` et `FabriqueNavigateur` (`src/chargement.rs`), comme `moteur-wasmtime` : réécrire, compiler, une sonde vérifiée par `verifier_chargement`, puis des `Protocole<MachineNavigateur>` (`InstanceNavigateur`). Rien du protocole n'y est recopié.
- L'exemple `essais-navigateur` (`crate-type = ["cdylib"]`), la bibliothèque WebAssembly de la page de test : `essayer(fautif)` assemble `essais/machine.wat` dans la page, joue 33 essais et rend leur rapport. `essais/pieges.txt` dit, pour chacun des onze scénarios de piège, ce que wasmtime rend.

**`crates/banc`, une crate de plus dans le commun**, et le binaire `soiree-banc` (`#![forbid(unsafe_code)]`) :
- `serveur` : un serveur HTTP/1.1 sur `127.0.0.1`, un port libre, une requête par connexion, COOP, COEP et CORP posés ; il sert des fichiers sous des préfixes (sans `..`, sans `%`, sans chemin absolu) et fait passer chaque `POST /banc/rapport` sur un canal.
- `cdp` et `chrome` : le pilote minimal. Chrome lancé `--headless=new --remote-debugging-port=0` sur un profil neuf sous `target/banc/profils/` ; l'adresse lue sur sa sortie d'erreur ; `Browser.getVersion`, `Target.createTarget`, `Target.attachToTarget` à plat, `Runtime.enable`, `Log.enable`, `Page.enable`, `Page.navigate` ; les exceptions, la console et le journal de la page gardés ; `Browser.close`, puis au plus 10 s avant `kill`, et le profil effacé.
- `rapport` : la lecture et le jugement d'un rapport. Une page qui rapporte `{ erreur }`, ou rien, n'a pas réussi ; une famille que le lot demande (`lire`, `ecrire`, `remplir`, `grandir`, `globales_i64`, `pieges`) sans essai fait échouer (OUT-01).
- `web` : **la règle « mêmes lints »**. Les `[workspace.lints]` de `web/` égaux à ceux du commun, comparés comme TOML ; `clippy.toml` et `rust-toolchain.toml` égaux ; chaque membre nommé (pas de motif) écrit `[lints] workspace = true` et rien d'autre. Égaux, et non « au moins aussi stricts » comme pour un jeu : `web/` est le commun. Le témoin `web/temoins/lints-differents/` (un workspace qui permet `unwrap_used`, et un membre aux lints propres) doit la faire échouer.
- `machine` : le scénario `soiree-banc machine`. La règle « mêmes lints », puis la construction (l'exemple pour `wasm32-unknown-unknown` en `--release`, `CARGO_BUILD_JOBS=4` par défaut ; `wasm-bindgen --version` vérifiée égale à 0.2.129, REM-156 ; `wasm-bindgen --target web`), finie avant que Chrome ne parte ; puis deux passages, chacun dans un Chrome neuf : la vraie machine (tout réussit), et la machine fautive `i64-par-nombre` (les essais `globales_i64` échouent, et eux seuls). Code 0 réussi, 1 échec, 2 pas exécuté. `soiree-banc regles-web` ne joue que la règle.
- La page `crates/banc/pages/machine.html` : une vingtaine de lignes de JavaScript, qui chargent la colle, appellent `essayer` et envoient le rapport.

**`#[wasm_bindgen]` n'exige pas d'`unsafe`** (question 5, P32) : l'export compile sous `#![forbid(unsafe_code)]`, clippy pédant compris (wasm-bindgen 0.2.129, édition 2024, rustc 1.99.0). Aucune exception n'est écrite.

Documents : `PHASE-3.md` (§ 2.2, les dépendances de `moteur-navigateur` et de `banc` ; § 2.3, ce qui est constaté dans le navigateur ; P32 et la question 5 ; la ligne du lot 3a au § 7 ; l'annexe des dépendances), `MESURES-PHASE-3.md` (« Lot 3a »), `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`.

### Les tests qui le tiennent

Dans Chrome, par `soiree-banc machine` (33 essais, chacun sur une instance neuve) :
- `lire`, `ecrire` : `ecrire_puis_le_module_lit`, `lire_ce_que_le_module_ecrit`, `lire_ecrire_aller_retour_a_travers_une_page` (70 000 octets de part et d'autre d'une page), `lire_hors_de_la_memoire_est_une_panne`, `ecrire_hors_de_la_memoire_est_une_panne` (aux bornes exactes, et à `u32::MAX`) ;
- `remplir` : `remplir_de_zeros_aux_bornes`, `remplir_hors_de_la_memoire_est_une_panne` ;
- `grandir` : `grandir_garde_la_memoire`, `grandir_a_la_meme_taille_ne_detache_pas` (qui voit aussi le détachement d'une vraie croissance : l'essai sait rougir), `grandir_au_dela_du_maximum_est_refuse`, `grandir_par_le_module_se_voit` ;
- `globales_i64` : `globales_i64_lues_exactement`, `globales_i64_posees_exactement` (`i64::MIN`, `i64::MAX`, ±(2⁵³ + 1)…, relus et lus par le module), `globales_i64_ecrites_par_le_module` ; et `globales_des_autres_types` (NaN canoniques, -0), `globales_inconnue_immuable_ou_mal_typee_sont_des_pannes` ;
- `appel_boite_passe_ses_arguments`, `appel_rend_un_i64_exact`, `neuve_repart_de_l_origine`, `neuve_apres_un_piege` ;
- `pieges` : un essai par scénario de `pieges.txt` (`pieges_unreachable`, `pieges_hors_memoire`, `pieges_pile_native`, `pieges_division_par_zero`, `pieges_reste_par_zero`, `pieges_debordement_de_division`, `pieges_conversion_d_un_nan`, `pieges_conversion_hors_bornes`, `pieges_table_hors_bornes`, `pieges_element_nul`, `pieges_signature`) : le piège rendu est celui de wasmtime, ou, pour un message d'`AMBIGUS` seulement, le texte du navigateur ; jamais un autre nom ; l'instance répond encore après. `pieges_la_table_est_constatee` : chaque message des deux tables a été levé pendant ce passage ;
- `chargement_d_une_boucle_est_la_faute_budget` : le module de `Bonjour` qui boucle, réécrit (`PHASE_1`) et chargé dans le navigateur par `charger` : `RefusModule::Faute { appel: "Bonjour", faute: Budget }`. La réécriture, le carburant `i64` et le protocole du commun tournent dans la page.

En natif :
- `crates/banc/tests/web.rs` : `web_a_les_lints_du_commun`, et son contrôle négatif `le_temoin_aux_lints_differents_fait_echouer_la_regle` (la règle nomme `workspace.lints.clippy.unwrap_used` et le membre `temoin`).
- `crates/banc/tests/pieges.rs` : `chaque_piege_des_essais_est_celui_de_wasmtime` (les onze lignes de `pieges.txt` sont ce que wasmtime 50.0.1 rend pour `machine.wat`, et l'instance sert encore après).
- `banc`, en unités : la lecture d'une requête et d'un corps, une requête coupée ou trop grosse refusée, un chemin qui ne sort pas de sa route ; les messages de CDP ; l'adresse annoncée par Chrome ; le jugement d'un rapport (`une_famille_absente_echoue_out01`, `le_controle_negatif_n_echoue_que_dans_sa_famille`, `un_rapport_d_erreur_n_est_pas_un_succes`) ; les différences de deux tables TOML.
- `moteur-navigateur` (`cargo test` dans `web/`) : la table des messages (`chaque_message_une_fois`, `un_message_inconnu_garde_le_texte_du_navigateur`, `la_pile_native_de_chaque_moteur`…) et le type des globales (`chaque_globale_exportee_a_son_type`).

**Contrôles négatifs.** Le témoin de `web/` (tenu par un test) ; la machine fautive `i64-par-nombre`, jouée à chaque `soiree-banc machine` : `globales_i64_lues_exactement` (lu -2⁵³ au lieu de -(2⁵³ + 1)), `globales_i64_posees_exactement` et `globales_i64_ecrites_par_le_module` échouent, et aucun autre. Faits à la main puis retirés, chacun après une copie du fichier remise à l'octet près : retirer `unreachable` de `MESSAGES` fait échouer `pieges_unreachable` (le texte du navigateur n'est accepté que pour un message d'`AMBIGUS`), code 1 ; demander `grow` à taille égale (`<` au lieu de `<=`) fait échouer `grandir_a_la_meme_taille_ne_detache_pas` (« tampon détaché (0 octets) »), code 1.

### Les vérifications

Toutes le 2026-11-05, `CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation. D'autres programmes chargeaient la machine : `vm.loadavg` de 9 à 29, de 32 à 60 % de mémoire libre (`memory_pressure`) avant chaque commande lourde.

| Commande | Résultat |
| --- | --- |
| `cargo test --workspace --no-fail-fast` du commun (15 h 42 à 15 h 59, en arrière-plan, `/usr/bin/time -l`) | code 0 ; **464 réussis** (448 et les 16 de `banc`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 1 010 s, wasmtime recompilé compris ; 0,64 Go de mémoire résidente au plus |
| `cargo clippy --workspace --all-targets -- -D warnings` (commun) | aucun avertissement |
| `cargo fmt --all --check` (commun, puis `web/`) | rien à reformater |
| `cargo test -p banc` | 13 + 1 + 2 réussis ; 79 s compilation comprise, 1,16 Go au plus |
| dans `web/` : `cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings`, puis `cargo clippy --all-targets -- -D warnings` | aucun avertissement |
| dans `web/` : `cargo test` | 8 réussis |
| `soiree-banc regles-web` | code 0 |
| `soiree-banc machine` (15 h 39, 15 h 39, 15 h 41, 16 h 03, construction comprise sauf la première) | **code 0** chaque fois : 33 essais réussis dans Chrome 156.0.8078.80 ; le contrôle négatif n'a fait échouer que les trois essais `globales_i64`. Le premier passage, `--sans-construire`, jouait une colle périmée (encore sans les messages `null function` et `function signature mismatch` dans la table) : il est écarté, et a montré que `--sans-construire` ne vérifie pas la fraîcheur |
| les deux contrôles négatifs faits à la main (15 h 40 et 15 h 41) | code 1, chacun sur le seul essai visé (plus les `globales_i64` du passage fautif) |

Après chaque exécution, aucun processus de Chrome ne restait sur un profil du banc (`pgrep -f target/banc/profils`), ni aucun profil sous `target/banc/profils/`. Le serveur du banc s'arrête avec lui.

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 3a » : les messages de V8 pour chaque piège, face à ceux de wasmtime ; le coût du scénario (de 3,8 à 10,4 s, construction incrémentale comprise ; Chrome de 0,46 à 1,22 Go de mémoire résidente, somme de son arbre de processus, un majorant ; `soiree-banc` 0,21 Go). La compilation de `web/` depuis un dossier neuf (`cargo clean`, 15 h 59, charge 13 à 18) : l'exemple pour `wasm32-unknown-unknown` en `--release`, toutes ses dépendances comprises, 32 s et 0,53 Go de mémoire résidente au plus ; `cargo test --no-run` en natif ensuite, 72 s et 0,66 Go. Estimé au § 7 : « léger », Chrome « environ 1 Go ». La mémoire n'a pas été mesurée avant le premier travail du lot, comme le § 7 le demande, mais pendant : la première compilation (26,9 s, 14 h 5x) ne l'a pas relevée.

### Ce qui n'est pas fait, et pourquoi

- **Firefox et Safari ne sont pas joués** : le lot ne demande que Chrome par CDP. `InternalError: too much recursion` (SpiderMonkey) est classé d'après PHASE-3, § 2.3, non constaté ; aucun message de SpiderMonkey ni de JavaScriptCore n'est dans `MESSAGES`. Un message inconnu reste sous le texte du navigateur, et les essais du banc le refuseront tant qu'il n'est pas rangé (lots 3b et 3c).
- **Les essais tournent sur le fil principal de la page**, pas dans un Worker : la pile native qu'ils épuisent est celle de la page (le Worker vient au lot 10a, la profondeur qu'il tient au lot 3c, M9). `new WebAssembly.Module` synchrone est permis ici parce que le module d'essai est petit.
- **Le protocole n'est essayé dans le navigateur que par le chargement** (`Bonjour` qui boucle, faute `Budget`) : un appel `Entree`, l'instantané, la restauration et l'empreinte sur `MachineNavigateur` ne sont joués qu'avec E20 (lot 3b), sur de vrais journaux.
- **La faute d'un piège ambigu s'écrira autrement au journal dans Chrome et dans wasmtime** : `Faute::Piege { code: "navigateur : float unrepresentable in integer range" }` contre `invalid conversion to integer` ou `integer overflow`. Rust n'émet pas de conversion qui piège (`as` est saturée) ; un module écrit à la main le pourrait. Ce n'est pas tranché ici : E20 le verra (lot 3b).
- **Aucun instantané ne passe d'un moteur à l'autre** : `InstanceNavigateur` garde l'instantané de `moteur-commun`, dont la forme est déjà figée, mais aucun essai ne relit dans le navigateur un instantané pris dans wasmtime (lot 3b, puis 10a).
- **La règle « mêmes lints » ne compare pas les profils** : `web/` les a recopiés (`dev.package."*"`, `release`), sans `module`, qui ne sert qu'aux modules des jeux ; un écart de profil n'y rougirait pas. *Corrigé après la vérification (ci-dessous).*
- **Le plan court ni le plan standard des trois jeux ne sont relancés** : le lot ne change rien de ce que l'épreuve joue ou juge (PHASE-3, § 7, « Le plan court ou le plan standard »), ni aucun fichier des jeux. *Faux d'après le § 7, qui demande le plan court à ce lot : joué après la vérification (ci-dessous).*

### Ce que le lot suivant doit savoir

- `soiree-banc machine` est le modèle d'un scénario : la règle « mêmes lints », la construction finie avant le navigateur, un navigateur neuf par passage, une page qui rapporte à `POST /banc/rapport`, un jugement pur (`banc::rapport::juger`) et un contrôle négatif joué à chaque fois. Le pilote CDP (`banc::cdp::Cdp`) suit les événements entre les réponses ; BiDi (lot 3b) peut reprendre le même `tungstenite` sur `TcpStream`.
- Pour E20, `moteur_navigateur::charger` et `FabriqueNavigateur` donnent une `partie::Instance` dans la page ; le module d'un jeu (jusqu'à 4 Mio) se compile par `ModuleNavigateur::compiler`, synchrone : dans un Worker, ou sur la page tant que Chrome l'accepte. Le relevé du carburant de chaque appel se lit par `Instance::carburant`, comme dans wasmtime.
- **Ajouter un navigateur, c'est ranger ses messages** : chaque message de `RuntimeError` que Firefox ou Safari lève pour les scénarios de `pieges.txt` va dans `MESSAGES` (s'il ne désigne qu'un piège de wasmtime) ou dans `AMBIGUS`. `pieges_la_table_est_constatee` vérifie que chaque message des tables est levé par le navigateur joué : la table devra alors se lire par navigateur.
- Changer une règle d'écriture du commun se refait dans `web/` (`crates/banc/tests/web.rs` rougit sinon).
- `banc` ajoute au `Cargo.lock` du commun `tungstenite` 0.30.0 et ce qu'il tire (`http`, `httparse`, `sha1`, `digest`, `data-encoding`…). Après son ajout, `cargo test --workspace` a recompilé wasmtime une fois (constaté ; la cause, des fonctionnalités d'une dépendance partagée unifiées autrement, est supposée). Les jeux n'en voient rien : leurs `Cargo.lock` ne changent pas.
- La page de test sert `/paquet/` depuis `target/banc/paquet/` ; `--sans-construire` y reprend la dernière colle construite, même périmée.

## Lot 3a, correction après la vérification

Fait le 2026-11-05, de 16 h 25 à 17 h 25 environ (GMT), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), Chrome 156.0.8078.80. Rien n'est commité.

### Ce qui est corrigé

**Constat 1 (important) : le plan court n'avait pas été joué.** Exact : PHASE-3, § 7, le demande aux lots qui ne sont pas dans la liste du plan standard, et la phrase « le lot ne l'exige pas » était fausse. Joué depuis les sources (`target/release/soiree-epreuve ../games/<jeu> --court`, le binaire à jour : `cargo build --release -p epreuve --bin soiree-epreuve` n'avait rien à refaire) :

| Jeu | Heure | Code | Contrôles en échec | Durée | Mémoire max |
| --- | --- | --- | --- | --- | --- |
| Punchline | 16 h 30 | 1 | E2 seul : « une fin, mais aucune autre après la nouvelle partie » (rejeu : graine `b4e6…61c5`, 13 783 lignes) | 35 s | 0,34 Go |
| Sidequest | 16 h 31 | **0** | aucun | 349 s | 0,48 Go |
| Tier list | 16 h 37 | 1 | E15 seul : octets vivants « au plus 201670 au début, 213975 en moyenne à la fin », douze mesures, aucune allée et venue | 94 s | 0,26 Go |

Mêmes codes, mêmes contrôles et mêmes chiffres que la vérification à 16 h 19 : le plan court est déterministe. **Ces échecs ne viennent pas du lot** : il ne touche rien de ce que l'épreuve compile ou joue (ni `epreuve`, `partie`, `jeu`, `jeu-abi`, `moteur-commun`, `moteur-wasmtime`, `reecriture`, ni les jeux), et son `Cargo.lock` n'ajoute que des paquets (`tungstenite` et ce qu'il tire) et des qualificatifs de version (`digest 0.10.7`…) : aucune version déjà là ne change. Ils sont connus en partie : le CHANGELOG dit que le plan court « ne garantit pas le résultat d'un jeu » (Punchline y échouait E2 et E15 en phase 1, faute de temps pour deux parties ; RESTE-PHASE-1). Les trois jeux passent le plan standard (lot 2, correction, 13 h 38 à 13 h 53).

Ce qui était faux, c'est le § 7 : il demande le plan court sans dire ce qu'il prouve, alors qu'il ne donne pas le code 0. **PHASE-3, § 7, est corrigé** : le plan court d'un lot doit rendre, pour chaque jeu, le même code, les mêmes contrôles en échec et les mêmes constats qu'avant le lot ; la référence est le tableau ci-dessus ; tout écart est un échec du lot, et un lot qui change la référence la relève et dit pourquoi. La ligne du lot 3a le dit aussi.

**Constat 2 (mineur) : deux codes de sortie.** Exact, et corrigé. `machine::jouer` rend `Arret::Echec` quand la règle « mêmes lints » refuse `web/` (code 1, « machine : échec »), `Arret::PasExecute` quand la construction, la colle ou le serveur manquent (code 2). Une page qui rapporte `{ erreur }`, ou rien, reste un échec (code 1) : c'est le code qu'on joue qui n'a pas tourné, dans un Chrome qui, lui, est parti ; c'est la documentation de `banc::rapport`, qui disait « pas exécuté », qui est corrigée, comme celle de `soiree-banc`, `AGENTS.md` et `CHANGELOG.md`.

**Constat 3 (mineur) : un message ambigu accepté pour n'importe quel piège.** Exact, et corrigé. `AMBIGUS` porte, pour chaque message, les pièges de wasmtime qu'il peut désigner (`float unrepresentable in integer range` : `CONVERSION_INVALIDE` et `DEBORDEMENT_ENTIER`) ; `juger_piege` n'accepte le texte du navigateur que si le piège que wasmtime rend pour ce scénario en est un. `chaque_message_une_fois` vérifie aussi qu'un message ambigu en désigne au moins deux, et n'est qu'une fois dans la table. Ce qui reste : `debordement_de_division` attend aussi `DEBORDEMENT_ENTIER` ; si V8 y levait le message ambigu, l'essai passerait, mais `divide result unrepresentable` ne serait plus levé nulle part et `pieges_la_table_est_constatee` échouerait.

**Constat 4 (mineur) : le jugement ne comptait pas les essais ni le passage.** Exact, et corrigé. Le rapport se lit avec son champ `fautif` (absent, il ne se lit pas) ; `juger` prend la suite attendue (`SUITE_MACHINE` : les familles, et `ESSAIS_MACHINE` = 33 essais exactement) et le passage joué : un rapport d'un autre passage, un essai de moins, de plus ou en double font échouer. Tests : `un_essai_de_moins_ou_en_double_echoue_out01`, `le_rapport_d_un_autre_passage_echoue`, et `un_rapport_d_erreur_n_est_pas_un_succes` complété.

**Constat 5 (mineur) : les mesures.** (1) Exact : la mémoire de Chrome n'avait pas de commande. Le script est maintenant dans `MESURES-PHASE-3.md`, « Lot 3a », et la mesure est refaite avec lui (1 206 272 et 1 208 624 Kio au plus, soit 1,24 Go, dans l'écart déjà relevé). (2) Exact : la charge de l'en-tête est corrigée en « 18 à 29 ». (3) Je ne peux pas départager 3 710 370 et 3 710 374 : le premier chiffre est celui du lot, le second celui du vérificateur ; les deux sont écrits, avec la taille d'après la correction (3 710 900 octets ; 4 230 413 compilé), qui change avec `AMBIGUS`.

**Constat 6 (mineur) : la règle ne comparait pas les profils.** Corrigé : les profils de `web/` doivent être ceux du commun, sauf `banc::web::PROFILS_DES_JEUX` (`module`). Le témoin `web/temoins/lints-differents/` a un troisième écart (`[profile.release] overflow-checks = false`), que `le_temoin_aux_lints_differents_fait_echouer_la_regle` exige de voir nommé.

### Ce qui n'est pas suivi, et pourquoi

- **Constat 7** : la mémoire du lot n'a pas été mesurée avant son premier travail ; on ne peut pas le refaire après coup. Les mesures faites ensuite (compilation `wasm32` à froid 0,53 Go et 32 s ; Chrome au plus 1,24 Go) restent dans l'estimation du § 7 (« léger », Chrome « environ 1 Go », un majorant).
- **Les échecs du plan court de Punchline (E2) et de Tier list (E15)** ne sont pas corrigés : ils viennent de la longueur du plan court, pas du lot, et les corriger toucherait l'épreuve ou les jeux, hors du lot. Celui de Tier list est nouveau au journal : une heure de soirée sans allées et venues fait monter les octets vivants de 6 % ; le plan standard (six heures, avec allées et venues) passe.

### Les tests qui le tiennent

En plus de ceux du lot 3a : `rapport::tests::un_essai_de_moins_ou_en_double_echoue_out01`, `rapport::tests::le_rapport_d_un_autre_passage_echoue` (OUT-01), le troisième écart du témoin, `chaque_message_une_fois` étendu.

**Contrôles négatifs faits à la main**, chacun sur une copie du fichier remise à l'octet près (`cmp`) :
- `pieges.txt` dit que wasmtime rend `integer divide by zero` pour `conversion_d_un_nan` : `soiree-banc machine` rend 1, et seul `pieges_conversion_d_un_nan` échoue (« Autre("navigateur : float unrepresentable in integer range"), wasmtime rend Autre("wasm trap: integer divide by zero") ») ; avant la correction, l'essai aurait réussi.
- une clé de plus dans `web/clippy.toml` : `soiree-banc machine` rend **1** (« machine : échec », la clé nommée), et non plus 2, sans rien construire ni lancer.
- `lto = true` ajouté à `[profile.release]` de `web/Cargo.toml` : `soiree-banc regles-web` rend 1 et nomme `profile.release.lto`.

### Les vérifications

`CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation ; 24 à 40 % de mémoire libre (`memory_pressure`) avant chacune, `vm.loadavg` de 11 à 27 (d'autres programmes tournaient, dont un Chrome de l'hôte, sans rapport avec le banc).

| Commande | Résultat |
| --- | --- |
| dans `web/` : `cargo fmt --all --check` ; `cargo test` ; `cargo clippy --all-targets --target wasm32-unknown-unknown -- -D warnings` puis en natif | ok ; 8 réussis ; aucun avertissement |
| `cargo fmt --all --check` ; `cargo clippy --workspace --all-targets -- -D warnings` (commun) | ok ; aucun avertissement |
| `cargo test -p banc` | 15 + 1 + 2 réussis |
| le plan court des trois jeux (ci-dessus) | Sidequest 0 ; Punchline 1 (E2) ; Tier list 1 (E15) : la référence |
| `soiree-banc machine` (16 h 39 et 16 h 40, sous le script de mesure) | **code 0** les deux fois : 33 essais réussis ; le contrôle négatif n'a fait échouer que les trois `globales_i64` ; `pieges_conversion_*` acceptés comme ambigus, pour un piège que le message désigne |
| `soiree-banc regles-web` | code 0 |
| `cargo test --workspace --no-fail-fast` (16 h 41 à 16 h 56, en arrière-plan, `/usr/bin/time -l`) | **code 101** : 465 réussis, **1 échec**, 1 ignoré (`plan_standard`) ; 862 s ; 0,38 Go au plus. L'échec : `boucle_au_vote_echoue_e19` (`epreuve/tests/epreuve`), où E15 est en « panne de l'outil » : « avancer : panne de la machine (Instanciation) : wasm trap: interrupt ». Le garde-fou mural d'une seconde de wasmtime a coupé l'instanciation d'une instance neuve ; la mémoire libre était descendue à 24 % et la charge dépassait 20 |
| `cargo test -p epreuve --test epreuve boucle_au_vote_echoue_e19`, seul (16 h 56, charge 12) | réussi, 39 s |
| `cargo test -p epreuve --test epreuve --no-fail-fast` (16 h 58 à 17 h 08, en arrière-plan, charge de 12 à 24) | **code 0** : 42 réussis, 0 échec, 1 ignoré (`plan_standard`), `boucle_au_vote_echoue_e19` compris. |
| `cargo test --workspace --no-fail-fast`, relancé (17 h 08 à 17 h 22, en arrière-plan, charge de 16 à 24, 30 % de mémoire libre au départ) | **code 0** : **466 réussis** (464 et les deux nouveaux de `rapport`), 0 échec, 1 ignoré (`plan_standard`) ; 803 s ; 0,60 Go au plus |

Après chaque exécution : aucun processus sur `target/banc/profils`, aucun profil restant, aucun serveur ni tâche de fond de moi. Les jeux : `git status` propre.

### Ce que le lot suivant doit savoir

- **Le plan court a une référence** (PHASE-3, § 7) : Sidequest 0, Punchline 1 (E2), Tier list 1 (E15). Un lot la compare ; il ne la prend pas pour un code 0.
- **Le garde-fou mural de wasmtime peut couper une instanciation sous forte charge** : une panne de l'outil, jamais un succès, mais `cargo test --workspace` peut rougir sans que rien n'ait changé (OUT-89). `moteur-wasmtime` pose l'échéance à la création du `Store`, avant `Instance::new`. Ce n'est pas tranché ici (le lot ne touche pas `moteur-wasmtime`) ; le lot 3c, qui revoit le garde-fou d'après M10, devrait le regarder.
- `banc::rapport::juger` prend la suite attendue et le passage : un scénario nouveau déclare sa `Suite` (familles et nombre exact d'essais), et sa page renvoie `fautif`.
- Un message du navigateur qui désigne plusieurs pièges de wasmtime va dans `AMBIGUS` avec la liste de ceux qu'il désigne.

## Lot 3b — Le second moteur de l'épreuve

Fait le 2026-11-05, de 17 h 30 à 19 h 50 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), avec Chrome 156.0.8078.80 et Firefox 158.0.1, sans interface. Rien n'est commité.

### Ce qui est fait

**E20, « Deux moteurs »** (PHASE-3, § 8.2 et § 8.3), dans `epreuve` (`src/deux_moteurs.rs`) :
- `IdControle::E20` ; `IdControle::TOUS` a vingt contrôles, et `IdControle::DU_PLAN` (E1 à E19) est le plan par défaut, standard comme court : E20 est un contrôle long, qu'on demande (`--navigateurs chrome,firefox[,safari]`, qui l'ajoute au plan). Il n'est dans aucune étape du CONTRAT (`Etape::controles`), pour ne pas rendre « pas faite » une étape qu'un code 0 sans lui fait (P17).
- Quand E20 est demandé, l'épreuve garde chaque soirée qu'elle joue (ouverture et journal ; une soirée rejouée à l'identique, comme la seconde exécution de E9, une seule fois) ; `eprouver_octets` fait le relevé de wasmtime (chaque soirée reprise depuis sa création, sur `moteur_commun::releve::Enregistreuse`), écrit un dossier (module livré, `soirees.bin`, `releve.bin`) dans le dossier temporaire, lance `soiree-banc rejouer` (trouvé par `SOIREE_BANC`, sinon à côté de l'exécutable ; `soiree-epreuve <dossier>` le pose pour la copie native), relit son JSON. Un résultat par navigateur au rapport (texte ; JSON, champ `e20`) ; E20 prend le pire (échec, puis panne, puis pas exécuté). Le dossier est effacé sur un succès, gardé sinon, avec `banc.txt`. Un module refusé au chargement joue E20 aussi : chaque navigateur doit le refuser pareil. `eprouver` (sans les octets) laisse E20 « pas exécuté », dit.
- Le contrôle négatif dans l'épreuve : `CommunFautif::FausserApres(n)`, `--moteur-fautif <n>` (transmis à la copie native, écrit au rapport : « Défaut injecté dans le commun »).

**`moteur_commun::releve`**, nouveau : le relevé (`Evenement` : début et fin de soirée, instance neuve, appel — nom, budget, empreinte courte de l'appel, ordre et place d'une entrée, carburant, issue : retour (longueur, empreinte courte), faute classée, ou cause de panne —, instantané, restauration, empreinte), `Chargement` (empreinte réécrite et carburant de `Bonjour` et `Decrire`, ou le refus classé par `classer_refus`), sa forme binaire relue strictement, `comparer`, `Enregistreuse` (qui peut comparer en chemin et arrêter le rejeu au premier écart), et `rejouer` : la même procédure dans wasmtime et dans le navigateur. Il reprend `Enregistree` et `Enregistreuse` de `mesures/navigateur/capture`, sur le protocole du commun ; la capture du harnais n'est pas touchée.

**`partie::Partie::reprendre_en_refaisant_les_fautes`** : la reprise ordinaire saute les entrées écartées ; celle-ci rejoue chacune, vérifie que sa faute est celle du journal (sinon `RejeuDivergent`), puis reconstruit la définitive, comme la partie l'a fait. Sans elle, aucune faute ne passerait par E20, et « chaque fautif classé pareil » ne voudrait rien dire. `reprendre` ne change pas (`refaire_les_fautes` vaut `Non` hors de cette méthode).

**`banc`** :
- `bidi` et `firefox` : Firefox sans interface sur un profil neuf (`user.js` du harnais), `--remote-debugging-port 0`, l'adresse lue sur sa sortie, `session.new`, `session.subscribe` (`log.entryAdded`), `browsingContext.getTree`, `browsingContext.navigate`, `browser.close` puis au plus 10 s avant de le tuer, profil effacé. Sans `geckodriver`.
- `navigateur` : `Navigateur` (chrome, firefox, safari) et `Lance`, un même pilote pour les deux ; Safari y est « pas exécuté » (lot 3c). Les aides de profil, de fermeture et de lecture de l'adresse sont partagées avec `chrome`.
- `machine` prend `--navigateur firefox` (et `--executable`) ; `construire` construit n'importe quels exemples de `web/moteur-navigateur`.
- `rejouer` : le scénario de E20. La règle « mêmes lints », la construction de l'exemple `rejeu-navigateur` avant tout navigateur, puis, par navigateur, la page `pages/rejouer.html`, dont le Worker (`pages/rejouer-worker.js`) joue deux passages : le moteur demandé, puis le moteur fautif `fausser-apres:<n>` (n = la moitié des appels, au plus 100). Le jugement (`rejouer::juger`) : le vrai passage sans écart **et** avec l'empreinte BLAKE3 et le nombre d'événements du relevé servi (la page n'est pas crue sur parole) ; le contrôle négatif doit diverger à l'appel `n`, sur le carburant, sinon E20 est une panne de l'outil. `soiree-banc rejouer <dossier> [--navigateurs] [--fautif] [--sortie] [--delai] [--sans-construire]`.

**`moteur-navigateur`** :
- l'exemple `rejeu-navigateur` (`rejouer(module, soirees, releve, fautif)`) et son moteur fautif (`Faussee` : passé `n` appels de `soiree_appel`, le carburant restant lu d'une unité de trop) ; `ModuleCharge::instance_sur`, une instance du protocole sur une machine qui enveloppe la vraie.
- **Les messages de SpiderMonkey** : `MESSAGES` et `AMBIGUS` portent leur moteur (`MoteurJs::V8`, `MoteurJs::SpiderMonkey`, tiré de `navigator.userAgent`) ; six messages de Firefox ramenés au nom de wasmtime ; `index out of bounds` (la mémoire comme la table) ambigu ; `AMBIGUS` désigne désormais des `Ramene` (une faute `HorsMemoire`, ou un code), et l'essai `pieges_la_table_est_constatee` ne vérifie que les messages du moteur joué.

Documents : `PHASE-3.md` (§ 2.2, `moteur-commun`, `banc`, `epreuve` ; la ligne du lot 3b au § 7 ; le scénario `rejouer` au § 8.2 ; E20 au § 8.3 ; la question 24), `MESURES-PHASE-3.md` (« Lot 3b »), `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`.

### Les tests qui le tiennent

En natif :
- `moteur-commun/tests/commun/releve.rs` (sur le modèle de `partie`) : `la_reprise_depuis_la_creation_se_releve_a_l_identique` (deux relevés égaux, chaque soirée reprise jusqu'au bout, des `Panique` refaites, entrées, restaurations, empreintes et `Canonique` relevées), `un_moteur_qui_differe_se_voit_a_son_premier_appel` (le modèle divergent : l'écart tombe sur la forme canonique de la fin, et le rejeu s'arrête là), `un_carburant_fausse_d_une_unite_se_voit`, `un_releve_plus_court_ou_plus_long_est_un_ecart`, `les_formes_se_relisent_strictement`.
- `partie/tests/coeur/garde.rs` : `la_reprise_qui_refait_les_fautes_refait_chacune` (même partie, une restauration de plus par faute ; un journal qui dit une autre faute est une divergence, que la reprise ordinaire ne voit pas).
- `banc` : `rejouer::tests` (`le_vrai_moteur_egal_et_le_negatif_vu_reussit`, `un_ecart_est_un_echec_qui_le_nomme`, `la_page_n_est_pas_crue_sur_parole` — OUT-01 —, `le_moteur_fautif_de_l_epreuve_fait_echouer`, `le_negatif_suit_le_nombre_d_appels`, `un_rapport_d_erreur_n_est_pas_un_succes`), `bidi::tests`, `firefox::tests`, `navigateur::tests`.
- `epreuve` : `deux_moteurs::tests` (`le_resultat_du_banc_se_lit`, `e20_prend_le_pire`), `ligne::tests::le_plan_se_transmet` (E20, navigateurs et moteur fautif compris), `les_navigateurs_demandent_e20` ; `tests/epreuve` : `passe_tout` vérifie que E20 est le seul contrôle hors du plan, « pas exécuté » ; `zero_evaluation_n_est_pas_un_succes` compte vingt contrôles.
- `moteur-navigateur` (`cargo test` dans `web/`) : `chaque_message_une_fois` (les deux moteurs, et chacun a son accès hors de la mémoire), `le_moteur_d_un_navigateur`.

Dans les navigateurs (chaque fois Chrome puis Firefox, un à la fois) :
- `soiree-banc machine --navigateur firefox` et `--navigateur chrome` : code 0, 33 essais réussis, le contrôle négatif n'a fait échouer que les trois `globales_i64`.
- E20 réussi, plan court, pour `devine`, `ardoise`, Punchline, Sidequest et Tier list (ces trois depuis leurs sources), et pour chacun des 33 modules de `exemples/fautifs` ; à chaque fois, le contrôle négatif du banc (`fausser-apres:100`, ou la moitié des appels) a divergé à l'appel attendu, sur le carburant.
- **Le moteur fautif fait échouer E20 seul** : `soiree-epreuve --wasm devine_module.wasm --court --navigateurs chrome,firefox --moteur-fautif 50` rend 1, E20 « échec du jeu » dans les deux navigateurs (« événement 57, soirée 0, après l'entrée d'ordre 28 (place 202) : un carburant de 8400 au lieu de 8401 ; … Etat sur l'instance 1 »), et chaque autre contrôle a l'issue et le nombre d'évaluations du même plan sans le défaut.

### Les vérifications

`CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation ; 20 à 53 % de mémoire libre avant chaque commande lourde (une fois 20 %, pendant les fautifs, à la limite : rien n'a été lancé à côté), `vm.loadavg` de 10 à 27 (d'autres programmes de l'hôte tournaient, dont son propre Chrome).

| Commande | Résultat |
| --- | --- |
| `cargo clippy` de `moteur-commun`, `partie`, `banc`, `epreuve` (`--all-targets -- -D warnings`) ; dans `web/`, en `wasm32` et en natif | aucun avertissement |
| `cargo fmt --all --check` (commun, `web/`) | rien à reformater |
| `cargo test -p moteur-commun --test commun releve`, `-p partie --test coeur la_reprise_qui_refait`, `-p banc`, `-p epreuve --lib` ; `cargo test` dans `web/` | réussis (5, 1, 25 + 1 + 2, 63, 9) |
| `soiree-banc machine --navigateur firefox`, puis `chrome` (18 h 05) | code 0 chacun |
| le plan court des trois jeux, depuis les sources, avec E20 (18 h 23 à 18 h 42) | Sidequest **0** ; Punchline 1, E2 seul (« une fin, mais aucune autre après la nouvelle partie », graine `b4e6…61c5`, 13 783 lignes) ; Tier list 1, E15 seul (201 670 au début, 213 975 à la fin) : **la référence du § 7**, inchangée ; E20 réussi dans les deux navigateurs pour chacun |
| `devine`, `ardoise` (`--wasm`, plan court, E20) | code 3 (module seul), chaque contrôle réussi ou sans objet, E20 réussi |
| les 33 fautifs (`--wasm`, plan court, E20 ; 18 h 43 à 19 h 19) | E20 réussi pour chacun, dans les deux navigateurs |
| `devine --moteur-fautif 50` | code 1, E20 seul en échec |
| `cargo test --workspace --no-fail-fast` (19 h 22 à 19 h 36, arrière-plan) | **code 101** : 480 réussis, **5 échecs**, 1 ignoré ; 868 s ; 0,44 Go au plus. Les cinq, dans `tests/epreuve` : `devine_passe_tout`, `avec_fenetre_passe_tout`, `le_socle_sain_passe_tout`, `un_jeu_sans_telephone_passe_tout_sans_pannes_de_telephone` (`passe_tout` exigeait chaque contrôle, demandé ou non, réussi ou sans objet ; E20, hors du plan, y est « pas exécuté ») et `zero_evaluation_n_est_pas_un_succes` (il comptait 19 contrôles). Corrigés sans rien affaiblir : `passe_tout` exige que E20 soit le seul contrôle hors du plan, et « pas exécuté » ; l'autre compte `IdControle::TOUS.len()`, soit 20 |
| `cargo test -p epreuve --test epreuve --no-fail-fast`, après la correction (19 h 37 à 19 h 48, arrière-plan) | **code 0** : 42 réussis, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 665 s ; 0,55 Go au plus. Avec les 480 autres réussis de la commande précédente (dont aucun binaire de test n'a changé depuis), **485 tests**, tous réussis |
| `cargo clippy --workspace --all-targets -- -D warnings` (commun), après la correction ; dans `web/`, `wasm32` et natif ; `cargo test` dans `web/` ; `cargo fmt --all --check` des deux ; `soiree-banc regles-web` | aucun avertissement ; 9 réussis ; rien à reformater ; code 0 |

Après chaque exécution : aucun processus sur `target/banc/profils`, aucun profil restant, aucun dossier `soiree-e20-*` laissé (ceux des contrôles négatifs, gardés exprès, effacés à la main), aucun serveur ni tâche de fond de moi. Les jeux : Punchline et Sidequest, `git status` propre ; **Tier list : son `Cargo.lock` a une ligne de plus**, écrite par Cargo pendant l'étape 1 de son épreuve (son workspace dépend de `epreuve`, qui dépend maintenant de `moteur-commun`). Elle n'est pas défaite : un `--locked` refuserait l'ancien fichier. Elle est à committer dans le dépôt de Tier list, à part du commun.

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 3b » : la durée de E20 par jeu et par navigateur, plan court (de 20 s à 2 min 40 par jeu pour les deux navigateurs ; Sidequest, 497 088 événements : 49 s de relevé dans wasmtime, 47 s de reprise dans Chrome, 37 s dans Firefox), une seule exécution chacune, sous une charge de 15 à 24 ; les messages de SpiderMonkey. La mémoire du lot, mesurée en premier comme le § 7 le demande : `cargo build --release -p banc` à 0,58 Go (14 s) ; `soiree-banc machine --navigateur firefox`, 0,64 Go (navigateur non compté) ; la reconstruction de `soiree-epreuve` et `soiree-banc` en `--release`, 1,05 Go (161 s). Estimé au § 7 : « `banc` natif léger ; un navigateur à la fois, environ 1 Go » ; la mémoire des navigateurs n'est pas mesurée à ce lot.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : « pas exécuté », dit par le banc et par E20 (« Safari se pilote par safaridriver, que le banc n'a qu'au lot 3c »). Le lot ne le demande pas ; aucun réglage de la machine n'a été touché.
- **Les entrées des pilotes** (`Input.dispatchKeyEvent`, `Input.dispatchMouseEvent`, `Input.dispatchTouchEvent`, `input.performActions`) ne sont pas écrites : aucun scénario de ce lot ne s'en sert, et un code non joué ne serait pas éprouvé. « Pilotes complets » veut dire ici : le même pilote pour Chrome et Firefox (lancement, version, page, console et exceptions, fermeture), joué par deux scénarios. Elles viennent avec `dessin`, `pages` et `soiree` (lots 9 à 12).
- **Le plan standard avec E20** n'est pas joué : le lot ne change rien de ce que le plan par défaut joue ou juge (E20 n'y est pas, et la reprise qui refait les fautes n'est appelée que par E20), et le § 7 ne le demande pas au lot 3b ; le plan court des trois jeux rend la référence. La durée de E20 sur le plan standard n'est donc pas mesurée (sa soirée longue de six heures serait six fois plus longue à reprendre : estimé).
- **Les fautes que E20 compare** : les journaux des 33 fautifs ne font refaire que des `Panique`. `Budget`, `Profondeur`, `HorsMemoire`, `Memoire` et `Piege` ne passent par E20 sur aucun module d'aujourd'hui (les fautifs qui les visent fautent dans des appels qui ne sont pas des entrées, ou au chargement) ; leur classement dans le navigateur reste tenu par les essais de `Machine` (onze pièges, dans les deux navigateurs). Un message ambigu (`float unrepresentable in integer range` dans V8, `index out of bounds` dans SpiderMonkey) donnerait une faute écrite autrement que dans wasmtime, et E20 échouerait : c'est voulu (la différence se voit), et un module compilé par Rust sûr n'y tombe pas.
- **Le relevé de wasmtime n'est pas celui du jeu en direct** : c'est celui de la reprise de chaque soirée par la procédure commune (le jeu en direct mêle la tête, les sondes et les copies). PHASE-3, § 8.3, le dit maintenant. Si wasmtime ne reprend pas son propre journal, E20 est une panne de l'outil sans lancer de navigateur ; ce n'est arrivé sur aucun module.
- **La pile du Worker** : aucun des modules joués n'a épuisé la pile d'un Worker (aucune `PileNative`), mais la profondeur qu'il tient n'est pas mesurée (M9, lot 3c).
- **Le lancement et la fermeture des navigateurs** coûtent de 4 à 28 s par navigateur sous charge, plus que la reprise des petits modules ; ce n'est pas optimisé (un navigateur par module et par E20).
- `mesures/navigateur/capture` n'est pas retirée : elle reste l'outil de la mesure 1 (`banc-coeur.js` part au lot 3c).

### Ce que le lot suivant doit savoir

- **Lancer E20** : `cargo build --release -p banc` **et** `cargo build --release -p epreuve --bin soiree-epreuve`, en deux commandes : `-p banc -p epreuve --bin soiree-epreuve` ne construit que `soiree-epreuve` (`--bin` vaut pour les deux paquets ; constaté à ce lot, un `soiree-banc` périmé a tourné une fois). Puis `target/release/soiree-epreuve <jeu> --court --navigateurs chrome,firefox`.
- **Le lot 3c change toutes les empreintes réécrites** (`PHASE_3`) : le relevé suit (le `Chargement` porte l'empreinte réécrite ; la page réécrit avec `reecriture::Reglages::PHASE_1`, en dur dans `rejeu_navigateur.rs` : à changer avec `ReglagesMoteur::PHASE_3`, sinon chaque E20 échouera au chargement, ce qui se verra).
- **Ajouter Safari**, c'est : son pilote derrière `navigateur::Lance`, ses messages dans `MESSAGES`/`AMBIGUS` avec un `MoteurJs::JavaScriptCore` (et `depuis_agent`, qui prend aujourd'hui tout agent `Chrome/` pour V8), puis `soiree-banc machine --navigateur safari` et E20.
- `Partie::reprendre_en_refaisant_les_fautes` est la reprise qui fait les mêmes appels que la partie qui a écrit le journal ; la reprise d'une autorité (lot 10a) garde `reprendre`, qui saute les fautes.
- Le relevé d'une page passe par le Worker ; `moteur_commun::releve::Enregistreuse::comparant` arrête le rejeu au premier écart : un rejeu fautif coûte ce qui précède l'écart.
- Le dossier d'un E20 qui n'a pas réussi reste dans le dossier temporaire de la machine (`soiree-e20-<empreinte>-<pid>`) : `soiree-banc rejouer <dossier> --sans-construire` le rejoue.

## Lot 3b, correction après la vérification

Fait le 2026-11-05, de 20 h 15 à 21 h 50 environ (GMT), même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface. Rien n'est commité.

### Les constats, et ce qui en est fait

**Constat 1 (important) : « chaque fautif classé pareil » ne comparait que des `Panique`.** Vérifié, exact : sur les 33 fautifs, les journaux ne font refaire que des `Panique` (3 et 6), et aucune autre sorte de faute ne passait d'un moteur à l'autre par E20. (Précision : `boucle-au-vote` et `jamais-fini` ne fautent pas du tout dans l'épreuve — le premier reste sous le budget d'arrêt, le second ne finit plus ses manches — ; rien n'était donc à rejouer chez eux.) **Corrigé** : la procédure commune (`moteur_commun::releve::rejouer`, jouée telle quelle dans wasmtime et dans le Worker) provoque, avant les soirées, deux fautes `Budget` sur chaque module accepté : une instance neuve, `Bonjour` et `Decrire` au budget de lecture de la première soirée, dont le carburant `c` est relevé ; puis, chaque fois sur une instance neuve, `Decrire` à `c / 2`, puis à `c - 1` (le carburant s'épuise au milieu de l'appel, puis à une unité de sa fin). Chaque navigateur doit rendre la même faute, au même carburant, que wasmtime ; et l'épreuve exige que wasmtime en fasse des `Budget` (`releve::budgets_provoques`), sinon E20 est une panne de l'outil. Le rapport le dit : « fautes : Budget × 2, Panique × 3 (dont 2 Budget provoquées avant les soirées) », et pour chaque navigateur « fautes classées pareil : Budget × 2, Panique × 3 ». Ce que cela tient : le piège du carburant épuisé, levé par le navigateur, puis la lecture des globales après le piège, qui est le chemin même de `Profondeur` (`soiree_profondeur`) et de `Memoire` (`soiree_croissance_bornee`) dans `moteur_commun::protocole`. Ce que cela ne tient pas : `HorsMemoire` et `Piege` ne se provoquent pas sur un module compilé par Rust sûr ; ils restent tenus par les essais de `Machine` (onze pièges, dans chaque navigateur), et c'est dit au § 7 et au § 8.2 de PHASE-3.

**Constat 2 (mineur) : `index out of bounds` est ambigu dans Firefox.** Vérifié, exact (MESURES-PHASE-3, « Les messages de SpiderMonkey » : SpiderMonkey 158 lève ce même message pour la lecture hors de la mémoire et pour le `call_indirect` au-delà de la table). **Écarté**, sans changement : le message seul ne dit pas lequel des deux pièges est tombé, et le ramener à l'un serait un classement deviné ; la faute garde alors le texte du navigateur, et un E20 qui y tomberait échouerait — la différence se voit, elle ne fait pas un faux succès. Un module de Rust sûr n'y tombe pas (ses accès sont bornés avant, un dépassement y est une panique). Le lever demanderait de lire l'instruction fautive (le décalage de la trame wasm dans `error.stack`, puis l'opérateur à ce décalage par `wasmparser`) : ce n'est pas simple, et cela se joue avec les autres moteurs au lot 3c (ci-dessous).

**Constat 3 (mineur) : `deux_moteurs.rs` ignorait le code de sortie de `soiree-banc`.** Vérifié, exact : un banc arrêté sur la règle « mêmes lints » (`"arret": "echec"`) ou qui ne rendait rien donnait « pas exécuté ». **Corrigé** : `lire_le_banc` rend un `ArretDuBanc` qui garde la nature de l'arrêt (`pas_execute` reste « pas exécuté » : un navigateur ou un fichier absent ; `echec`, la règle « mêmes lints » qui refuse `web/`, est une **panne de l'outil**, puisque c'est le commun qui est en défaut, pas le jeu ; un JSON illisible aussi) ; un banc qui n'écrit pas `--sortie`, ou que l'on n'a pas pu lancer, est une panne de l'outil ; et `code_coherent` exige que le code de sortie dise ce que dit le JSON (0 si chaque navigateur réussit, 1 si l'un échoue, 2 sinon ; un signal est une panne).

**Constat 4 (mineur) : `partie` change, et seul le plan court avait été joué.** Vérifié : la branche nouvelle de `jouer_entree` n'est prise que sous `reprendre_en_refaisant_les_fautes`, que seul E20 appelle ; mais le § 7 ne tranchait pas ce cas. **Tranché en jouant** le plan standard des trois jeux depuis leurs sources (sans E20) : code 0 pour chacun (ci-dessous). Le § 7 (ligne du lot 3b) le dit.

**Constat 5 (mineur) : les durées de Sidequest manquaient, et dépendent de la charge.** Vérifié, exact. **Corrigé** : E20 remesuré sur les cinq modules avec le banc à jour (MESURES-PHASE-3, « Lot 3b », « Après la vérification ») ; les chiffres de Sidequest qui manquaient y sont, et les trois séries (lot, vérificateur, correction) y sont mises côte à côte avec leur charge.

**Constat 6 (mineur) : le `Cargo.lock` de Tier list.** Vérifié : toujours une ligne (`moteur-commun` parmi les dépendances de `epreuve`), écrite par Cargo ; rien d'autre n'a changé dans les jeux. Rien à faire ici : elle est à committer dans le dépôt de Tier list, à part du commun (CHANGELOG, « Passer à cette version »).

### Les tests qui le tiennent

- `moteur-commun/tests/commun/releve.rs`, `les_fautes_budget_se_provoquent_avant_les_soirees` : sur le modèle, la faute `Budget` provoquée est relevée avant la soirée 0 (`budgets_provoques`) ; sans soirée, rien ; un moteur qui classe le carburant épuisé en piège (`BudgetEnPiege`) diverge là, avant toute soirée. Son contrôle négatif, joué à la main : sans l'appel à `provoquer_le_budget`, ce test échoue, et lui seul des six.
- `epreuve`, `deux_moteurs::tests` : `le_resultat_du_banc_se_lit` (chaque arrêt garde sa nature), `le_code_du_banc_doit_s_accorder` (nouveau).
- Dans les navigateurs, E20 : `Budget × 2` classées pareil dans Chrome et Firefox pour `devine`, `ardoise`, les trois jeux et 32 des 33 fautifs (`declaration-incoherente` est refusé au chargement, pareil, et ne joue rien).

### Les vérifications

`CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation ; 31 à 51 % de mémoire libre avant chaque commande lourde, `vm.loadavg` de 5 à 13.

| Commande | Résultat |
| --- | --- |
| `cargo test -p moteur-commun --test commun releve` ; `-p epreuve --lib deux_moteurs` ; `-p banc` | 6, 3, 28 réussis |
| `cargo clippy -p moteur-commun -p epreuve -p banc --all-targets -- -D warnings` ; dans `web/`, `wasm32` et natif ; `cargo test` dans `web/` ; `cargo fmt --all --check` (commun, `web/`) | aucun avertissement ; 9 réussis ; rien à reformater |
| `cargo build --release -p banc`, puis `-p epreuve --bin soiree-epreuve` | 4,9 s (0,23 Go), 12,7 s (0,40 Go) |
| `soiree-banc machine --navigateur firefox`, puis `chrome` (20 h 42) | code 0 chacun ; 33 essais, le contrôle négatif n'a fait échouer que les trois `globales_i64` |
| `devine`, `ardoise` (`--wasm`, plan court, E20) | code 3 (module seul), chaque contrôle réussi ou sans objet ; E20 réussi, `Budget × 2` classées pareil |
| `devine --moteur-fautif 50` | code 1, **E20 seul en échec** dans les deux navigateurs (« événement 60, soirée 0, après l'entrée d'ordre 24 (place 198) : un carburant de 5530 au lieu de 5531 ») ; le rapport entier, comparé ligne à ligne avec celui sans défaut : seuls E20, le verdict et la ligne « Défaut injecté dans le commun : FausserApres(50) » diffèrent |
| les 33 fautifs (`--wasm`, plan court, E20 ; 20 h 43 à 21 h 02) | E20 réussi pour chacun ; `Budget × 2` partout sauf `declaration-incoherente` (refusé pareil) ; `Panique × 3` (`fichier-qui-panique`) et `× 6` (`panique-au-temps`) en plus ; mêmes codes de sortie, et E1 à E19 aux mêmes issues et aux mêmes nombres d'évaluations, que le passage du vérificateur |
| le plan court des trois jeux, depuis les sources, avec E20 (21 h 02 à 21 h 08) | Sidequest **0** ; Punchline 1, E2 seul (« une fin, mais aucune autre après la nouvelle partie », graine `b4e6…61c5`, 13 783 lignes) ; Tier list 1, E15 seul (201 670 au début, 213 975 à la fin) : **la référence du § 7** ; E20 réussi dans les deux navigateurs pour chacun |
| le plan standard des trois jeux, depuis les sources, sans E20 (21 h 08 à 21 h 24) | Punchline **0** (140 s, 1,41 Go) ; Sidequest **0** (565 s, 2,35 Go) ; Tier list **0** (248 s, 0,85 Go) |
| le code du banc, joué à la main avec trois faux `soiree-banc` (`SOIREE_BANC`) sur `ardoise` : `/usr/bin/false`, un banc qui écrit `"arret": "echec"` (la règle « mêmes lints »), un banc qui écrit « réussi » et rend 1 | code 2 chaque fois, E20 **panne de l'outil** : « le banc rend le code 1 sans écrire … resultat.json », « le banc s'arrête (echec) : la règle « mêmes lints » refuse web/ », « le banc rend le code 1, et son résultat en demande 0 » ; avant la correction, d'après le code d'alors (non rejoué), les deux premiers auraient donné « pas exécuté », le troisième un succès |
| `cargo test --workspace --no-fail-fast` (21 h 24 à 21 h 33, arrière-plan) | **code 0** : 487 réussis (485 et les deux tests nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 505 s ; 0,78 Go au plus |

Les mesures : MESURES-PHASE-3, « Lot 3b », « Après la vérification » (la durée de E20 par module et par navigateur, Sidequest compris, sous une charge de 8 à 13).

État laissé : aucun navigateur, serveur ni tâche de fond de moi ; aucun profil sous `target/banc/profils` ; aucun dossier `soiree-e20-*` (ceux du moteur fautif et des faux bancs, gardés exprès par l'épreuve, effacés à la main). Punchline et Sidequest propres ; Tier list n'a que la ligne de son `Cargo.lock`. Fichiers de code changés par la correction : `crates/moteur-commun/src/releve.rs`, `crates/moteur-commun/tests/commun/releve.rs`, `crates/epreuve/src/deux_moteurs.rs` ; documents : `PHASE-3.md` (§ 7, ligne du lot 3b ; § 8.2, `rejouer`), `MESURES-PHASE-3.md`, `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`, ce journal.

### Ce qui n'est pas fait, et pourquoi

- **`HorsMemoire` et `Piege` ne passent toujours pas par E20** : aucun module de Rust sûr ne les lève, et aucune entrée de l'ABI ne permet de les provoquer de dehors. Ils restent tenus par les essais de `Machine` dans chaque navigateur. `Profondeur` et `Memoire` ne sont pas provoquées non plus ; leur classement passe par la même lecture des globales après un piège que `Budget`, que E20 compare désormais.
- **`index out of bounds` dans Firefox** reste ambigu (constat 2, écarté ci-dessus).
- Le reste de « Ce qui n'est pas fait » du lot 3b tient : Safari, les entrées des pilotes, E20 sur le plan standard (sa durée y reste non mesurée), la mémoire des navigateurs.

### Ce que le lot suivant doit savoir

- Le relevé de chaque module commence désormais par `Neuve`, `Bonjour`, `Decrire` (au budget de lecture), puis deux fois `Neuve`, `Bonjour`, `Decrire` en faute `Budget` : 3 instances et 6 appels avant `Soiree(0)`. Un écart à cet endroit n'a pas de soirée (« événement 5 : … »), et c'est le classement d'un carburant épuisé qui diffère.
- Le lot 3c change toutes les empreintes réécrites, et peut changer le coût de `Decrire` : les budgets provoqués suivent d'eux-mêmes (`c / 2`, `c - 1`, relevés à chaque fois).
- Pour lever l'ambiguïté de `index out of bounds` (et faire de même pour JavaScriptCore) : lire dans `error.stack` le décalage de la trame wasm, puis l'opérateur du module réécrit à ce décalage (`wasmparser`, déjà dans `moteur-navigateur`) — un accès mémoire donne `HorsMemoire`, un `call_indirect` le code de la table —, avec un essai de `Machine` qui le vérifie dans Firefox.
- Le code de sortie de `soiree-banc rejouer` fait foi avec son JSON : un scénario qui change ses codes (0, 1, 2) doit changer `deux_moteurs::code_coherent` avec lui.

## Lot 3c — Étalonner, la profondeur, Safari

Fait du 2026-11-05 au 2026-11-06, de 22 h 30 à 00 h 40 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), avec Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface ; Safari 27.2 et `safaridriver` (« Included with Safari 27.2 (21625.2.8.18.4) ») présents, mais la session refusée. Rien n'est commité.

### Ce qui est fait

**M9, la profondeur, et `PHASE_3`** (PHASE-3, § 2.3, P4) :
- `reecriture::passes::profondeur_mesuree` et `reecriture::reecrire_en_mesurant_la_profondeur` : la variante de mesure de la passe de profondeur, qui garde le plus grand `soiree_profondeur` vu dans une seconde globale exportée, `soiree_profondeur_atteinte` (`reecriture::PROFONDEUR_ATTEINTE`). Le carburant est compté avant : la variante brûle le même carburant et rend les mêmes octets ; seule son empreinte diffère.
- `moteur_commun::Plan::en_mesurant_la_profondeur`, et dans le protocole, après chaque appel d'un tel module, la lecture de la globale ; `moteur_commun::profondeur_atteinte()` en rend le maximum du processus (`oublier_la_profondeur_atteinte()` le remet à zéro). Un module de partie n'est jamais lu ainsi.
- `moteur_wasmtime::Moteur::charger_en_mesurant_la_profondeur`.
- `epreuve/examples/profondeur.rs` : l'épreuve entière (plan standard, ou `--court`) sur un module ainsi réécrit, sans copie native, sous `PHASE_1` : la profondeur atteinte. Au plus **18 étages** (Tier list).
- Dans `web/moteur-navigateur`, l'exemple `etalonnage-navigateur`, `profondeur_tenue` : huit récursions sans fin écrites en `wat`, de 0 à 4 000 valeurs `i64` vivantes par étage, réécrites sans plafond utile ; dans le Worker puis sur le fil de la page. Au plus juste, **247 étages** (Worker de Chrome, 250 valeurs par étage) ; 492 dans un Worker de Firefox.
- `reecriture::Reglages::PHASE_3` et `moteur_wasmtime::ReglagesMoteur::PHASE_3` : **64 étages** de profondeur, le reste comme `PHASE_1`. L'épreuve (`ligne.rs`), ses tests (`deux_instances`, `tests/epreuve/modules.rs`), l'exemple `etalonner`, le Worker de E20 (`rejeu_navigateur.rs`), les essais de `Machine` et la capture du harnais réécrivent sous `PHASE_3`. Toutes les empreintes réécrites changent (tableau plus bas).

**M1 et M10, `soiree-banc etalonner`** (`crates/banc/src/etalonner.rs`, `pages/etalonner.html` et `etalonner-worker.js`, l'exemple `etalonnage-navigateur`) :
- M1 : pour chaque module de la capture, un navigateur neuf ; dans un Worker, le module livré chargé par `moteur_navigateur::charger` sous `PHASE_3` (réécriture, compilation et mise en route chronométrées), la remise à zéro des zones par la machine, deux mille `Bonjour` d'un bloc, puis la trace rejouée deux fois sur le protocole du commun, chaque appel, instance neuve, instantané et restauration chronométrés, le carburant, l'issue et le retour de chaque appel comparés à wasmtime. Le banc en tire les tableaux de la mesure 1 (`etalonner::analyser`, au rang le plus proche, comme l'analyse du harnais).
- M10 : le module de `Bonjour` qui boucle chargé quinze fois jusqu'à sa faute `Budget`, moins un témoin ; la même boucle par la machine seule sous un plus grand budget (`--budget`).
- `moteur_commun::trace` : le format de la trace d'étalonnage, une seule définition (la capture l'écrit, le navigateur la relit). La capture (`mesures/navigateur/capture`) s'en sert, réécrit sous `PHASE_3` et écrit aussi le module livré (`livre.wasm`).
- **`page/banc-coeur.js` retiré**, avec ce qui en dépendait : `page/banc.js`, `page/banc.html`, `page/trace.js`, `mesurer.mjs`, `analyser.mjs`, `hors-navigateur.mjs`. La mesure 6 (`cadence.mjs`) reste. `mesures/navigateur/README.md` réécrit.

**Le garde-fou de chargement revu d'après M10** : `moteur_commun::BUDGET_DE_CHARGEMENT` baisse de 2²⁶ à **2²⁴** : Firefox chargé a mis jusqu'à 6 092 ps par unité de boucle vide au chargement (410 ms pour 2²⁶ unités : deux fois et demie sous la seconde, au lieu de dix). `Creer` (2²⁸) n'est pas changé : jugé et dit (PHASE-3, § 2.3), à reprendre au lot 10a avec le garde-fou de partie. Le constat du lot 3a sur l'instanciation coupée par l'époque a été regardé : rien dans le code ne l'explique (aucun code du module ne tourne dans `Instance::new`), il n'a pas été reproduit, et rien n'est changé.

**Safari par `safaridriver`** (`crates/banc/src/safari.rs`) : WebDriver classique en HTTP sur la boucle locale, sans dépendance nouvelle (une requête par connexion, `Content-Length`), derrière `navigateur::Lance` ; `machine`, `rejouer` et `etalonner` le prennent (`--navigateur safari`, `--navigateurs …,safari`). `moteur_navigateur::MoteurJs::JavaScriptCore` reconnaît l'agent de Safari ; aucun de ses messages n'est rangé, et l'essai `pieges_la_table_est_constatee` échoue pour un moteur sans message (il réussissait à vide).

Documents : `PHASE-3.md` (§ 2.2, `moteur-commun` et `banc` ; § 2.3, la pile d'un Worker, le garde-fou de chargement, l'instanciation ; la ligne du lot 3c au § 7 ; Safari au § 8.2 ; M1, M9, M10 au § 8.6 ; la question 8 ; l'annexe des valeurs choisies), `MESURES-PHASE-3.md` (« Lot 3c »), `mesures/navigateur/README.md`, `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`.

### Les tests qui le tiennent

- `reecriture/tests/reecriture/passes.rs` : `la_variante_de_mesure_garde_la_profondeur_atteinte` (le maximum d'un appel à l'autre, `soiree_profondeur` rendu à zéro, le même étage au piège) ; `la_variante_de_mesure_ne_change_ni_le_carburant_ni_l_issue` (mêmes issues, même carburant, même profondeur que la passe de la partie ; seule la variante exporte la globale).
- `moteur-wasmtime/tests/moteur/fautes.rs` : `la_variante_de_mesure_dit_la_profondeur_atteinte` (1 étage au chargement, 32 après l'appel, le même retour et le même carburant que le module de la partie, et le module de la partie ne la change pas) ; `cadres_epais_jamais_la_pile_native` joue désormais sous `PHASE_1` **et** `PHASE_3`.
- `moteur-wasmtime/tests/moteur/chargement_borne.rs` : `bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement`, la valeur figée passée à 2²⁴ ; `epreuve/tests/deux_instances`, `le_chargement_tient_largement_son_budget`, tient toujours (`devine` brûle mille fois moins).
- `moteur-commun/src/trace.rs` : `la_trace_se_relit_a_l_identique` (octets figés de l'en-tête), `une_trace_abimee_ne_se_lit_pas` (chaque troncature, un octet en trop, un événement inconnu), `fnv_est_celle_du_harnais`.
- `banc` : `safari::tests::une_reponse_de_safaridriver_se_lit` ; `etalonner::tests` (`les_statistiques_au_rang_le_plus_proche`, `les_noms_des_mesures`, `la_profondeur_tenue_est_le_plus_petit_essai`) ; `navigateur::tests` inchangé.
- `moteur-navigateur` : `le_moteur_d_un_navigateur` reconnaît Safari (`JavaScriptCore`) et ne reconnaît pas un agent quelconque.
- Dans les navigateurs : `soiree-banc machine` dans Chrome et Firefox, code 0 (33 essais ; le contrôle négatif n'a fait échouer que les trois `globales_i64`), dans Safari code 2, « pas exécuté » ; E20 réussi partout où il a tourné.

Aucune leçon de `lecons/sources/` n'a fait naître un test de ce lot ; OUT-01 (« pas exécuté », jamais réussi) est cité dans `safari.rs` et `navigateur.rs`.

### Les vérifications

`CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation ; de 23 à 70 % de mémoire libre avant chaque commande lourde ; `vm.loadavg` de 5 à 21, sauf pendant la mesure de M10 « sous charge » (jusqu'à 161, des services de macOS en plus des quatorze `yes`) et juste après (35 à 38).

| Commande | Résultat |
| --- | --- |
| `cargo clippy -p reecriture -p moteur-commun -p moteur-wasmtime -p banc -p epreuve --all-targets -- -D warnings` ; dans `web/`, `--target wasm32-unknown-unknown` et en natif ; la capture (`--release`) | aucun avertissement |
| `cargo fmt --all --check` (commun, `web/`) | rien à reformater |
| `cargo test -p reecriture -p moteur-commun -p moteur-wasmtime` ; `-p banc` ; `cargo test` dans `web/` | réussis (53 ; 8 et 14 ; 34 ; 29 et 3 ; 9) |
| `target/release/examples/profondeur` sur les cinq modules, plan standard (22 h 58 à 23 h 17) | 10, 9, 15, 13, 18 étages ; code des contrôles 0 pour chacun (sans copie native) ; 1 149 s, 2,60 Go au plus |
| `soiree-banc etalonner --mesures m9` dans Chrome, puis Firefox et Safari (vers 23 h 20) | Worker 247 et 492 étages tenus au plus juste ; page 480 et 893 ; Safari pas exécuté (code 2) |
| `capture-navigateur` sous `PHASE_3` (23 h 24) | 116 807 appels, 0 écart dans les deux rejeux de wasmtime ; 49 s |
| `soiree-banc etalonner --mesures m1,m10 --navigateurs chrome,firefox,safari` (23 h 25 à 23 h 29) | 0 écart de carburant, d'issue ou de retour sur les 116 807 appels, deux rejeux, dans chaque navigateur ; empreintes réécrites égales à celles de la capture ; Safari pas exécuté (code 2 : tout n'a pas été mesuré) ; 245 s |
| M10 sous charge (23 h 31), puis après la baisse de `BUDGET_DE_CHARGEMENT` (23 h 36) | `MESURES-PHASE-3.md`, « Lot 3c » |
| `soiree-banc machine --navigateur chrome`, `firefox`, `safari` (23 h 37) | 0, 0, 2 (Safari pas exécuté) |
| E20, plan court, `--navigateurs chrome,firefox,safari` : `devine`, `ardoise` (`--wasm`), Sidequest (23 h 35 à 23 h 41) | E20 réussi dans Chrome et Firefox, Safari pas exécuté, donc E20 « pas exécuté » (il prend le pire) et le code 3 ; E1 à E19 de Sidequest réussis ou sans objet |
| E20, plan court, `--navigateurs chrome,firefox`, les trois jeux depuis leurs sources (23 h 41 à 23 h 49) | Sidequest **0** ; Punchline **1**, E2 seul (« une fin, mais aucune autre après la nouvelle partie », graine `b4e6…61c5`, 13 783 lignes) ; Tier list **1**, E15 seul (201 670 au début, 213 975 à la fin) : **la référence du § 7, inchangée** ; E20 réussi dans les deux navigateurs pour chacun (`Budget × 2` classées pareil, le contrôle négatif vu à l'appel 100) |
| le plan standard des trois jeux, depuis leurs sources, sous `PHASE_3`, sans E20 (23 h 50 à 00 h 21) | Punchline **0** (387 s, 0,38 Go) ; Sidequest **0** (1 145 s, 2,29 Go) ; Tier list **0** (318 s, 0,62 Go) |
| `cargo test --workspace --no-fail-fast` (00 h 21 à 00 h 30, arrière-plan) | **code 0** : 497 réussis (les 487 du lot 3b et les 10 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 496 s ; 0,62 Go au plus |
| `cargo clippy --workspace --all-targets -- -D warnings` ; dans `web/`, `wasm32` et natif ; `cargo fmt --all --check` des deux ; `soiree-banc regles-web` | aucun avertissement ; rien à reformater ; code 0 |

État laissé : aucun navigateur, `safaridriver`, serveur, `yes` ni tâche de fond de moi ; aucun profil sous `target/banc/profils` ; les trois dossiers `soiree-e20-*` laissés par les E20 où Safari était demandé (« pas exécuté », donc gardés par l'épreuve) effacés à la main. Les trois jeux : `git status` propre. La capture a son `Cargo.lock` à jour (`moteur-commun` en dépendance directe ; il manquait aussi, depuis le lot 3b, parmi celles de `epreuve`). Les résultats bruts sont dans `target/mesures-navigateur/` (hors dépôt) : la capture, `etalonnage/<navigateur>/` et `analyse.json` ; la mesure M10 à 2²⁶ dans Chrome et Firefox a été écrasée dans `etalonnage/chrome/m10.json` et `firefox/m10.json` par celle à 2²⁴ ; ses chiffres sont dans `MESURES-PHASE-3.md`.

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 3c » : M9 (les deux moitiés), la mesure 1 refaite par le banc et sa comparaison au harnais, M10 dans Chrome et Firefox, au repos et sous charge, avant et après la baisse de `BUDGET_DE_CHARGEMENT`. La mémoire, mesurée en premier comme le § 7 le demande : l'exemple `profondeur` 2,60 Go (plan standard des cinq modules) ; `soiree-banc etalonner` (M1 et M10, trois navigateurs, l'analyse comprise) 0,76 Go ; les épreuves du plan court avec E20, de 0,61 à 0,94 Go, du plan standard de 0,38 à 2,29 Go (`/usr/bin/time -l`, navigateurs non comptés). Estimé au § 7 : « léger ; un navigateur à la fois ».

La durée de E20 sur le plan court, sous une charge de 9 à 21 : Sidequest, reprise de 16,5 s dans Chrome et 20,5 s dans Firefox (19,7 s et 25,3 s lancement et fermeture compris) ; Punchline 2,6 s et 2,9 s ; Tier list 8,0 s et 9,6 s ; du même ordre qu'au lot 3b après la vérification.

Les nouvelles empreintes réécrites, sous `PHASE_3` (`target/mesures-navigateur/<module>/meta.json`), et celles d'avant (`PHASE_1`), pour les mêmes modules livrés :

| Module (livré) | Réécrit sous `PHASE_1` | Réécrit sous `PHASE_3` |
| --- | --- | --- |
| `devine` (`ac0c95aa…`) | `367cd2b34e653b83…` | `365e513999d6e3e2…` |
| `ardoise` (`2964ab9c…`) | `aac085379031b13e…` | `589d154735c3a5c6…` |
| Punchline (`ad1b7b04…`) | `d37ddb637d39e50f…` | `a63d05e9521c682b…` |
| Sidequest (`8112385d…`) | `f2a6310f3939308f…` | `58995a7ee5925921…` |
| Tier list (`73552646…`) | `dcd70ccb0b2ddf07…` | `ec7da1d169b7b6ba…` |

### Ce qui n'est pas fait, et pourquoi

- **Safari n'est pas exécuté**, nulle part : `safaridriver` refuse la session (« Could not create a session: The session timed out while connecting to a Safari instance… », au bout de 30 s), l'automatisation à distance n'étant pas autorisée sur la machine. Le lot demandait « réussi, échec, ou pas exécuté avec le message de `safaridriver` » : c'est le troisième, dit par chaque scénario. Aucun réglage de la machine n'a été touché. Les messages de pièges de `JavaScriptCore`, la profondeur que tient Safari, M1 et M10 dans Safari restent à faire dès que l'hôte aura coché le réglage (question 8).
- **Le plan standard avec E20** n'est pas joué : E20 est relancé sur le plan court, comme au lot 3b ; le plan standard des trois jeux, sans E20, rend le code 0 sous `PHASE_3`.
- **La mesure 1 n'est refaite qu'en un tour**, et ses queues sont plus hautes que celles des quatre tours du harnais (Chrome : 99ᵉ centile de 138 ps au lieu de 52 à 74 ; Firefox : maximum de 3 600 ps au lieu de 336 à 678), sous une charge plus haute ; les médianes se retrouvent. La proposition de budgets de E19 n'est pas refaite : elle reste sur les quatre tours du harnais. Node, deno et bun ne se mesurent plus (le protocole JavaScript est retiré).
- **Le bloc « sans remise à zéro »** du harnais n'existe plus : le banc ne recopie pas le protocole pour sauter la remise à zéro ; il la chronomètre par `Machine::remplir_de_zeros`, dans la mémoire d'une instance.
- **`Creer` sous 2²⁸** : jugé, pas changé (au pire mesuré dans Firefox, 1,64 s calculées pour une boucle, sous le garde-fou de partie de 2 s du Worker, avec un facteur 1,2) : à reprendre avec le garde-fou de partie au lot 10a, ou par le mainteneur (question 4).
- **L'instanciation coupée par l'époque** (lot 3a) : regardée, non expliquée, non reproduite, rien de changé (PHASE-3, § 2.3).
- **Les tests des jeux** chargent encore leur module sous `ReglagesMoteur::PHASE_1` (dans leurs dépôts) : ils passent, mais n'éprouvent pas la profondeur de l'autorité. À leurs agents d'y passer (CHANGELOG, « Passer à cette version ») ; ce lot ne touche pas les dépôts des jeux.

### Ce que le lot suivant doit savoir

- **`ReglagesMoteur::PHASE_3` est le réglage de la partie** : l'autorité du lot 10a charge sous `reecriture::Reglages::PHASE_3`, comme le Worker de E20. Changer sa profondeur change toutes les empreintes réécrites : relancer la capture, `etalonner`, le plan standard des trois jeux et E20, et refaire M9 (les deux commandes sont dans `MESURES-PHASE-3.md`). Un jeu qui dépasserait 64 étages se verrait en faute `Profondeur` ; l'exemple `profondeur` dit où en est chacun.
- La pile d'un Worker de Chrome tient la moitié de celle de sa page, et la moitié de celle d'un Worker de Firefox : c'est elle qui borne. Le fil d'un téléphone (page, Safari iOS) n'est pas mesuré.
- `BUDGET_DE_CHARGEMENT` vaut 2²⁴. Le garde-fou de chargement du Worker (lot 10a) peut s'appuyer sur M10 : au plus 6 092 ps par unité dans Firefox chargé, au chargement, soit 102 ms (calculé) ; `Creer` (2²⁸) reste à juger avec le garde-fou de partie.
- **E20 avec Safari** : tant que `safaridriver` refuse, demander Safari rend E20 « pas exécuté » (il prend le pire) et le code 3, et l'épreuve garde son dossier `soiree-e20-*` dans le dossier temporaire ; pour la référence du § 7, `--navigateurs chrome,firefox`.
- `soiree-banc etalonner` relit la capture de `target/mesures-navigateur` ; une capture d'un autre réglage se voit (« AUTRE que celle de la capture : rien n'est comparable »). Le code de sortie d'`etalonner` dit seulement si tout a été mesuré (2 dès qu'un navigateur est « pas exécuté ») : une mesure n'est pas un jugement. *Corrigé après la vérification : 1 dès un défaut (écart avec wasmtime, récursion de M9 sous `PHASE_3` qui n'est pas la faute `Profondeur`), qui l'emporte.*
- Les entrées des pilotes (`Input.dispatch*`, `input.performActions`) ne sont toujours pas écrites (lots 9 à 12) ; Safari, en WebDriver classique, les aura par `/actions`.

## Lot 3c, correction après la vérification

Le 2026-11-06, de 01 h 20 à 02 h 10 environ (GMT), sur la machine de l'hôte (la même ; Chrome 156.0.8078.80, Firefox 158.0.1). Rien n'est commité.

### Les constats, et ce qui en est fait

1. **[important] « `etalonner` retrouve la mesure 1 du harnais » avait été relu après coup** (la colonne du § 7 ne gardait que ce qui s'était retrouvé en un tour). Constat vérifié : c'était écrit tel quel au § 7 et dans `MESURES-PHASE-3.md`. Fait : `soiree-banc etalonner --tours <n>` (M1 rejouée `n` fois, un navigateur neuf par module et par tour, les tours s'enchaînant d'un navigateur à l'autre ; l'étendue de la médiane, du 99ᵉ centile et du maximum sur les tours) ; la capture refaite ; la mesure 1 jouée en **quatre tours** dans Chrome et Firefox ; et, pour séparer l'outil de l'état de la machine, **le harnais retiré rejoué juste après, sur la même capture** (ses six fichiers repris du commit `44160f9`, hors du dépôt). Résultat (`MESURES-PHASE-3.md`, « La mesure 1 en quatre tours, et le harnais rejoué à côté ») : le carburant, l'issue et le retour égaux appel par appel (aucun écart, quarante pages de chaque outil) ; le coût fixe et la remise à zéro retrouvés ; la médiane et le 99ᵉ centile d'une unité au niveau du harnais rejoué, ou en dessous (Chrome 39 à 44 et 78 à 129 ps contre 37 à 48 et 113 à 142 ; Firefox 68 à 100 et 135 à 299 contre 94 à 143 et 299 à 402) ; **les maxima ne se retrouvent pas** : ni ceux du matin, que le harnais rejoué ne retrouve pas non plus (pas plus que ses médianes de Firefox), ni, dans Chrome, ceux du harnais rejoué à côté (389 à 1 181 ps contre 123 à 527, à chaque tour plus haut). Ils tombent sur les appels de la soirée de Tier list, où une milliseconde de pause ou de remise à zéro lente des 25 Mio pèse 400 à 800 ps par unité ; que le Worker du banc en subisse plus que la page du harnais n'est ni expliqué ni exclu. La colonne du § 7 garde le critère tel qu'il était, et dit ce qui en est tenu et ce qui ne l'est pas ; elle ne le relit plus.
2. **[mineur] La proposition de budgets de E19 disait que 800 ps « couvrent le maximum mesuré dans Firefox (678 ps) »**, contredit par le lot 3c. Vérifié. Fait : la phrase dit maintenant que ce maximum est celui du matin, que le même soir le maximum de Firefox est monté à 900 ps (harnais) et jusqu'à 3 096 ps (banc), et qu'aucun maximum d'une seule soirée ne peut servir d'unité ; la proposition n'est pas refaite, la question 4 reste au mainteneur.
3. **[mineur] Les résultats bruts de M9 dans les navigateurs n'existaient nulle part.** Vérifié : aucun `m9.json` sous `target/mesures-navigateur/etalonnage/`, et rien dans le code ne l'efface ; la cause n'est pas retrouvée (sans doute un `--donnees` ou un dossier de travail autre au moment du lot). Fait : M9 refait, ses `m9.json` sont dans `etalonnage/chrome/` et `etalonnage/firefox/` ; les chiffres sont ceux du lot (247 et 480 ; 492 et 893).
4. **[mineur] Rien ne figeait `PHASE_3.profondeur_max = 64`, et rien ne vérifiait dans le navigateur qu'une récursion à cadres épais tombe en faute `Profondeur` sous `PHASE_3`.** Vérifié. Fait : le test `phase_3_se_tient_entre_les_jeux_et_la_pile_du_worker` (`reecriture/tests/reecriture/passes.rs`) fige 64, et que 64 laisse un facteur 3 au-dessus du jeu le plus profond (18) et sous le Worker le plus court (247) ; et M9 charge maintenant chacune de ses huit récursions (0 à 4 000 valeurs par étage) sous `PHASE_3` par `moteur_navigateur::charger`, dans le Worker puis sur la page : chacune doit être la faute `Profondeur`, sinon un défaut et le code 1. Dans Chrome et Firefox, toutes le sont. Contrôle négatif : chargées sous `PHASE_1`, treize défauts (Worker de Chrome dès 16 valeurs, page dès 64, « pile native épuisée avant soiree_profondeur »), code 1. Un retour de `PHASE_3` à 4 096 fait donc échouer un test du commun, et `etalonner --mesures m9`.
5. **[mineur] `etalonner` rendait 0 même avec des écarts de carburant.** Vérifié (c'était voulu et écrit). Fait : un défaut fait le code 1, qui l'emporte sur « pas exécuté », comme un échec de `rejouer` : un écart de carburant, d'issue ou de retour, une panne du rejeu, un nombre d'appels différent, un module réécrit autrement que dans la capture, une capture illisible, une récursion de M9 sous `PHASE_3` qui n'est pas la faute `Profondeur` ou un M9 qui n'a pas tourné. Les défauts sont listés à la fin de la sortie (« DÉFAUTS ») et dans `analyse.json` (`defauts`). Le code 2 reste « tout n'a pas été mesuré ».
6. **[mineur] Le message de `safaridriver` cité dépend du moment.** Vérifié : à 01 h 44, `soiree-banc machine --navigateur safari` rend le code 2 avec « You must enable 'Allow remote automation' in the Developer section of Safari Settings to control Safari via WebDriver. », tout de suite. Fait : `MESURES-PHASE-3.md` (en-tête du lot 3c) et PHASE-3 § 8.2 citent les deux messages et disent que le banc relaie celui du moment ; l'issue ne change pas. Aucun réglage de la machine touché.

Aucun constat n'est écarté.

### Les tests qui le tiennent

- `reecriture/tests/reecriture/passes.rs` : `phase_3_se_tient_entre_les_jeux_et_la_pile_du_worker` (né du constat 4 de cette vérification).
- `banc`, `etalonner::tests` : `sous_phase_3_tout_ce_qui_n_est_pas_profondeur_est_un_defaut` (une issue autre que `Profondeur`, des récursions absentes ou vides sont des défauts) ; `l_etendue_sur_les_tours`.
- Dans le navigateur : `soiree-banc etalonner --mesures m9`, code 0 dans Chrome et Firefox, code 1 sur son contrôle négatif (`PHASE_1` à la place de `PHASE_3`, le temps d'un essai, puis rendu).

### Les vérifications

`CARGO_BUILD_JOBS=4`, une commande lourde à la fois, jamais de navigateur pendant une compilation ; de 31 à 68 % de mémoire libre avant chaque commande lourde ; `vm.loadavg` de 4 à 12, et jusqu'à 28 pendant les deux derniers tours du harnais.

| Commande | Résultat |
| --- | --- |
| `cargo clippy -p banc -p reecriture --all-targets -- -D warnings` ; dans `web/`, `--target wasm32-unknown-unknown` et en natif | aucun avertissement |
| `cargo fmt --all --check` (commun, `web/`) | rien à reformater |
| `cargo test -p banc -p reecriture` ; `cargo test` dans `web/` | réussis (31, 1, 2 et 54 ; 9) |
| `capture-navigateur` (01 h 32) | 116 807 appels, 0 écart dans les deux rejeux de wasmtime ; 55 s ; empreintes réécrites celles du lot |
| `soiree-banc etalonner --mesures m9,m1 --tours 4 --navigateurs chrome,firefox` (01 h 33 à 01 h 37) | code 0 ; 0 écart sur les 116 807 appels à chaque tour et chaque rejeu ; M9 : 247, 480, 492, 893, chaque récursion sous `PHASE_3` en faute `Profondeur` ; 197 s, 0,74 Go |
| le harnais retiré, quatre tours, `node mesurer.mjs`, puis `node analyser.mjs` (01 h 37 à 01 h 41) | 0 écart partout ; les chiffres dans `MESURES-PHASE-3.md` |
| `etalonner --mesures m9 --navigateurs chrome` sous `PHASE_1` (contrôle négatif), puis `--navigateurs chrome,firefox` rendu | code 1 (treize défauts), puis code 0 |
| `soiree-banc machine --navigateur safari` (01 h 44) | code 2, pas exécuté, avec le message de `safaridriver` du moment |
| `cargo test --workspace --no-fail-fast` (01 h 46 à 01 h 54, arrière-plan) ; `cargo clippy --workspace --all-targets -- -D warnings` ; `soiree-banc regles-web` | **code 0** : 500 réussis (les 497 du lot et les 3 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 466 s ; 0,89 Go au plus ; aucun avertissement ; code 0 |

Non relancés, parce que rien de ce que cette correction change ne les touche (ni l'épreuve, ni `rejouer`, ni `PHASE_3`, ni les essais de `Machine`) : le plan standard des trois jeux sous `PHASE_3`, E20 sur le plan court, M10. La vérification les a relancés le 2026-11-06 vers 01 h 0x, avec les mêmes résultats que le lot.

### Les mesures

`MESURES-PHASE-3.md`, « La mesure 1 en quatre tours, et le harnais rejoué à côté » et M9 refait. La mémoire : la capture 0,15 Go, `etalonner` (M9 et quatre tours de M1, l'analyse comprise) 0,74 Go, navigateurs non comptés.

### Ce qui n'est pas fait, et pourquoi

- **Les maxima de la mesure 1 ne se retrouvent pas** (constat 1) : ce point du critère du § 7 n'est pas tenu. Ni le banc ni le harnais ne retrouvent les maxima du matin, et dans Chrome les maxima du banc dépassent, à chaque tour, ceux du harnais rejoué à côté. Ce qu'il faudrait pour trancher : séparer la remise à zéro et les pauses de la part du carburant dans les appels de la soirée de Tier list (par exemple chronométrer la remise à zéro de chaque appel à part, dans le banc comme dans le harnais), ou rejouer les deux outils en alternance sur plusieurs soirées. Ou décider que le maximum d'une soirée n'est pas une mesure de l'outil, ce qui est une décision sur le critère, pas une mesure : elle revient au mainteneur, ou à la prochaine vérification.
- **La proposition de budgets de E19** n'est pas refaite (constat 2) : elle reposait sur un maximum, et aucun maximum d'une soirée ne tient.
- **Safari** reste « pas exécuté », partout.
- Le plan standard, E20 et M10 ne sont pas relancés par cette correction (voir plus haut).
- Un Safari tournait sur la machine depuis 01 h 03 (avant cette correction, sans doute la vérification ou l'hôte) ; il n'est pas de moi et je ne l'ai pas fermé.

### Ce que le lot suivant doit savoir

- `soiree-banc etalonner --tours 4` est la façon de refaire la mesure 1 ; un seul tour ne dit rien des queues. Les maxima de la durée d'une unité tombent sur la soirée de Tier list : à lire comme des pauses, pas comme du calcul.
- `etalonner` rend 1 dès un défaut ; son résumé (`target/mesures-navigateur/etalonnage/analyse.json`) a une liste `defauts`, et chaque navigateur ses `tours`. `analyse.json` est réécrit à chaque lancement : il ne garde que le dernier.
- `PHASE_3` est figé à 64 par un test ; le changer demande de refaire M9 (les deux moitiés), qui vérifie maintenant aussi, dans le navigateur, que la profondeur pique avant la pile native.
- Le harnais retiré se reprend au commit `44160f9` (`mesures/navigateur/mesurer.mjs`, `analyser.mjs`, `page/banc.html`, `banc.js`, `banc-coeur.js`, `trace.js`) ; il lit encore la capture d'aujourd'hui.


## Lot 3c, fin

Le 2026-11-06, en deux temps : de 02 h 50 à 04 h 10 environ (GMT) par un premier agent, interrompu par un redémarrage de la machine ; puis de 17 h 35 à 18 h 10 par un second, qui a relu et fini ses changements. Même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0), Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface. Chaque commande lourde est passée par le portillon de l'hôte (`target/hote/lourd`), chaque mesure par `lourd --calme`. Rien n'est commité.

### Ce qui est fait

1. **La mesure 1 sur machine calme, les deux outils en alternance.** Le harnais retiré repris de `44160f9` sous `target/travail/lot-3c/harnais/` (le dossier temporaire du premier agent a disparu au redémarrage), puis un tour du harnais, un tour du banc (`soiree-banc etalonner --mesures m1 --navigateurs chrome,firefox`), et ainsi de suite : **huit tours valides chacun**, Chrome et Firefox, tous pris avec une charge sous 3 au départ (de 1,74 à 2,97 ; de 2,52 à 5,54 à la fin, le navigateur compris). Les tours 1 et 2 sont ceux du premier agent (04 h 00 à 04 h 08), gardés : pris sous 3, sur le même code de M1 (l'exemple construit à 03 h 02, l'arbre de travail de ses changements de code inchangé depuis) ; son troisième tour du harnais, interrompu avant de partir, n'a rien mesuré (`calme-harnais-3-interrompu`). Quatre tours étaient demandés au moins ; après les quatre premiers, ni les queues ni les médianes de Chrome ne tombaient dans l'étendue du harnais, et quatre autres ont été pris, décidés avant de les voir et tous gardés, pour que l'étendue de chaque outil ne repose pas sur quatre valeurs. Résultat (`MESURES-PHASE-3.md`, « La mesure 1 sur machine calme ») :
   - aucun écart de carburant, d'issue ou de retour, aucune panne, sur les 116 807 appels, dans chaque rejeu de chaque tour, par les deux outils ;
   - médianes : Firefox, banc 65 à 75 ps dans l'étendue du harnais (63 à 83) ; Chrome, banc 36 à 49 contre 36 à 43, **deux tours sur huit au-dessus** (46 et 49) ;
   - 99ᵉ centile : Chrome dedans (64 à 100 contre 60 à 105) ; Firefox non (120 à 207 contre 107 à 144) ;
   - maxima : non (Chrome 84 à 501 contre 108 à 175, six tours sur huit au-dessus, sur la soirée de Tier list ; Firefox 152 à 3 814 contre 172 à 300, un appel lourd de Punchline à 247 à 432 ps dans six tours sur huit, au plus 87 au harnais).

   Les queues du banc ne tombent donc pas dans l'étendue du harnais sur machine calme : **la décision de l'agent principal pour l'hôte s'applique.** Le critère du § 7 pour ce lot devient « zéro écart de carburant, d'issue et de retour, et médianes dans l'étendue du harnais d'un tour à l'autre » ; les queues se remesurent au lot 13 sur machine calme ; la question 27, nouvelle, porte les chiffres au mainteneur. Écrit dans PHASE-3 (la ligne du lot 3c au § 7, M1 au § 8.6, la question 27 au § 9.1). **Ce critère lui-même n'est tenu que dans Firefox** : dans Chrome, deux médianes sur huit le dépassent, de 3 et 6 ps. C'est dit tel quel, ni arrondi ni relu.
2. **Le contrôle négatif de M1** : `soiree-banc etalonner --fautif fausser-apres:<n>` rejoue la trace sur un moteur fautif (`Faussee` dans l'exemple `etalonnage-navigateur`, la machine fautive de E20 recopiée : passé `n` appels, le carburant restant lu d'une unité de trop) ; ses résultats s'écrivent sous `etalonnage/<navigateur>-fautif/`. L'analyse vérifie que chaque passe compte un écart, le premier au `n + 1`-ième appel (`premier_ecart_appel`, rapporté par la page), et le dit sinon. Dans Chrome et Firefox, sur `devine`, `fausser-apres:100` : 7 840 écarts par passe, le premier à l'appel 101, **code 1**. Le code de sortie est tiré dans `etalonner::code_de_sortie`, que le binaire appelle et que le test voit.
3. **M9** : un essai sans plafond qui ne s'arrête pas sur la pile native (un retour, un autre piège, aucun essai, aucun cadre) est un **défaut** (code 1), et non plus un « ATTENTION » (`defauts_des_cadres`). Refait dans Chrome et Firefox : code 0, les mêmes étages tenus (247 et 480 ; 492 et 893), chaque récursion sous `PHASE_3` en faute `Profondeur`.
4. **`rejouer_la_trace`** (l'exemple `etalonnage-navigateur`) : un `Instantane` ou un `Restaurer` qui nomme une instance ou un instantané absent est une **panne du rejeu**, qui fait un défaut et le code 1, et non plus un événement sauté. La fonction est devenue générique sur la fabrique (le vrai moteur, ou le moteur fautif) ; la comparaison d'un appel est `Rejeu::comparer`. Contrôle dans les navigateurs : la trace de `devine` à laquelle sont ajoutés, avant la fin, un instantané de l'instance 999 et une restauration vers l'instantané 999 (`target/travail/lot-3c/abimer.py`, `--donnees target/travail/lot-3c/donnees-abimee`) : 0 écart, 2 pannes par passe, **code 1**, dans Chrome et Firefox.
5. **Les en-têtes et la largeur** : `epreuve/examples/profondeur.rs` dit « M9, la première moitié » (la profondeur qu'atteignent les jeux), l'exemple `etalonnage-navigateur` « la seconde moitié » (celle que tient chaque navigateur), comme `MESURES-PHASE-3.md` ; les commentaires ajoutés ou repris (`soiree-banc.rs`, `rejouer.rs`, `moteur-wasmtime/src/lib.rs`, `etalonner.rs`, l'exemple) tiennent dans 88 colonnes (le code qui les entoure monte par endroits à 90) ; seules restent plus larges les lignes d'usage de `soiree-banc.rs`, qui sont du texte de commande.

Documents : `PHASE-3.md` (§ 7, § 8.6, § 9.1), `MESURES-PHASE-3.md`, `CHANGELOG.md`, `AGENTS.md`, `crates/README.md`, `mesures/navigateur/README.md`, la documentation de `soiree-banc` et de `etalonner`, `etalonner.html`.

### Les tests qui le tiennent

- `banc`, `etalonner::tests::un_ecart_avec_wasmtime_fait_le_code_1` : d'une capture d'essai et d'un rapport de page jusqu'au code de sortie ; sans écart 0, un écart 1, le contrôle négatif qui voit son écart au bon appel, qui ne voit rien, qui le voit trop tôt, une trace trop courte, un passage non mesuré 2 (né du constat 2 de la vérification : rien ne prouvait qu'un écart de M1 compte).
- `banc`, `etalonner::tests::m9_un_essai_hors_de_la_pile_native_est_un_defaut`.
- Dans les navigateurs : `etalonner --fautif fausser-apres:100` (code 1, écart à l'appel 101), la trace abîmée (code 1, deux pannes par passe), `etalonner --mesures m9` (code 0).
- **Aucun test natif ne tient le point 4** : l'exemple ne tourne que dans un navigateur (son horloge est `performance.now()`) ; il est tenu par le contrôle de la trace abîmée, joué à la main et gardé dans les preuves.

Aucune leçon de `lecons/sources/` n'a fait naître ces tests.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-3c/` (hors dépôt) ; chaque dossier a `commande.txt`, `sortie.txt` (complète, avec la charge et la mémoire que le portillon écrit au départ et à la fin), `code.txt` (code, durée, fin) et `empreinte.txt` (pour le commun et chaque jeu : `HEAD`, `sha256` de `git diff HEAD` et des fichiers non suivis). Le commun est à `f4fba17` pour toutes ; `diff HEAD` vaut `968018f9…` pour toutes, sauf `calme-harnais-1` (`622405fb…`, lancé à 03 h 02, avant que le premier agent ne finisse quatre documents `.md` à 03 h 03 ; le code était déjà celui-ci). Après les preuves, seuls des documents ont changé (PHASE-3, MESURES-PHASE-3, ce journal, CHANGELOG, AGENTS) : l'empreinte finale de l'arbre diffère donc, ses sources de code non. Le module éprouvé est l'exemple `etalonnage-navigateur` construit à 03 h 02 : `sha256` `7fc15e87314d4f4b092c568f31e52cf9f08ba91167c127b773ae0d2910e1f283` (`web/target/wasm32-unknown-unknown/release/examples/etalonnage_navigateur.wasm`) ; le paquet `wasm-bindgen` servi, `target/banc/paquet/etalonnage_navigateur_bg.wasm`, `c6ab00d3e5702323…` (réécrit à 17 h 59 depuis le même module). Les jeux n'ont pas changé.

| Preuve | Commande | Résultat |
| --- | --- | --- |
| `calme-harnais-1` à `-8`, `calme-banc-1` à `-8` | `lourd --calme node mesurer.mjs --navigateurs chrome,firefox --tour <n>` ; `lourd --calme target/release/soiree-banc etalonner --mesures m1 --navigateurs chrome,firefox --sans-construire` | code 0 chacun ; de 28 à 51 s de mesure par tour (le portillon l'écrit), l'attente du calme en plus (57 min pour le premier tour du harnais, au plus 4 min ensuite) ; `analyse.json` de chaque tour du banc dans son dossier |
| `harnais-analyse/` | `node analyser.mjs --tour <n>`, n de 1 à 8 | les analyses (Markdown et `analyse-harnais-tour-<n>.json`) |
| `comparaison.txt`, `comparaison-8-tours.txt` | `python3 -I target/travail/lot-3c/comparer.py 1 2 3 4` (puis `… 5 6 7 8`) | les étendues ci-dessus |
| `m1-fautif` | `lourd target/release/soiree-banc etalonner --mesures m1 --modules devine --navigateurs chrome,firefox --fautif fausser-apres:100` | **code 1**, écart à l'appel 101 dans les deux, 5 s |
| `m1-trace-abimee` | `lourd target/release/soiree-banc etalonner --mesures m1 --modules devine --navigateurs chrome,firefox --donnees target/travail/lot-3c/donnees-abimee --sans-construire` | **code 1**, 2 pannes par passe dans les deux, 4 s |
| `m9` | `lourd target/release/soiree-banc etalonner --mesures m9 --navigateurs chrome,firefox --sans-construire` | code 0, 3 s |
| `test-banc` | `lourd cargo test -p banc` | code 0, 36 réussis |
| `clippy-commun`, `clippy-web-wasm`, `clippy-web-natif` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; dans `web/`, `--target wasm32-unknown-unknown` et en natif | aucun avertissement |
| `test-web` | dans `web/`, `lourd cargo test` | code 0, 9 réussis |
| `fmt` | `cargo fmt --all --check`, commun et `web/` | rien à reformater |
| `regles-web` | `lourd target/release/soiree-banc regles-web` | code 0 |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0** : 502 réussis (les 500 de la correction et les 2 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 287 s ; 1,49 Go au plus (le plus gros processus) |
| `m1-fautif-avant-redemarrage`, `m9-avant-redemarrage`, `avant-harnais-0137` | du premier agent, sur un état antérieur du code | gardés pour l'histoire, non comptés |

Non relancés, parce que rien de ce que cette fin change ne les touche (ni l'épreuve, ni `rejouer`, hors d'un commentaire, ni `PHASE_3`, ni `partie`, ni les essais de `Machine`) : le plan standard des trois jeux, E20, M10, `soiree-banc machine`.

État laissé : aucun navigateur, serveur ni tâche de fond de moi ; le harnais et les données abîmées restent sous `target/travail/lot-3c/` (hors dépôt).

### Ce qui n'est pas fait, et pourquoi

- **Les queues de la mesure 1 ne se retrouvent pas, même au calme** ; le critère est changé par décision (plus haut), et **même le critère changé n'est pas tenu dans Chrome** (deux médianes sur huit au-dessus de l'étendue du harnais, 46 et 49 ps contre 43). Ce n'est pas corrigé : ni la décision ni ce document n'élargissent l'étendue. À trancher par le mainteneur (question 27), ou à remesurer au lot 13.
- La cause des queues du banc n'est pas cherchée : dans Chrome, la soirée de Tier list ; dans Firefox, un appel lourd de Punchline qui revient six tours sur huit. Le lot 13 peut chronométrer la remise à zéro à part et comparer appel par appel.
- Safari reste « pas exécuté » (question 8) ; rien n'a été touché aux réglages de la machine.
- Le point 4 n'a pas de test natif (plus haut).

### Ce que le lot suivant doit savoir

- Une mesure de M1 se prend par `target/hote/lourd --calme`, et sur plusieurs tours ; sous charge, les médianes de Firefox doublent. Les queues ne se jugent pas sur une soirée (question 27), et aucun budget ne se règle sur un maximum (question 4).
- `etalonner --fautif fausser-apres:<n>` est le contrôle négatif de M1 ; il doit rendre 1.
- Le harnais retiré se reprend par `git archive 44160f9 mesures/navigateur`, avec un lien `target` vers `soiree/target` et un lien `../games` à côté : ses résultats vont dans `target/mesures-navigateur/resultats/<navigateur>/<module>-<tour>.json`, où les huit tours de cette fin sont gardés.

## Lot 3c, seconde correction après la vérification de la fin

Le 2026-11-06, de 18 h 13 à 18 h 42 environ (GMT), même machine, Chrome 156.0.8078.80 et Firefox 158.0.1 sans interface. Chaque commande lourde par `target/hote/lourd`. Rien n'est commité.

### Les constats, et ce qui en est fait

1. **[bloquant] Même le critère changé n'est pas tenu dans Chrome.** Vérifié : les médianes de Chrome du banc vont de 36 à 49 ps, celles du harnais de 36 à 43 ; les tours 2 et 3 du banc (49 et 46) sont au-dessus (`comparaison-8-tours.txt`, recalculé par la vérification depuis les données brutes). Le critère est appliqué tel quel, comme l'hôte l'a décidé : rien n'est élargi, arrondi, retiré ni rejoué pour le faire passer. **Non corrigé, et le lot n'est pas fini selon sa colonne** ; c'est écrit dans la ligne du lot 3c au § 7 et à la question 27. Ce qui est ajouté pour le mainteneur, sans changer le verdict : la charge pendant la partie Chrome de chaque tour (constat 4), et l'étendue sans les tours 1 et 2 (constat 3), qui reste au-dessus (banc 36 à 46, harnais 36 à 42).
2. **[mineur] Sous `--fautif`, le code ne distinguait pas un contrôle négatif réussi d'un contrôle raté.** Vérifié : `controle_negatif` ajoutait ses lignes aux défauts, et un contrôle qui ne voyait rien rendait 1 comme un contrôle qui voit son écart. Fait : ses lignes vont dans une liste à part (`controle_rate`, dans `analyse.json` et dans la sortie, « CONTRÔLE NÉGATIF RATÉ ») ; `etalonner::code_de_sortie` rend **2** dès qu'elle n'est pas vide, avant tout défaut : une panne de l'outil, comme `rejouer --fautif` classe « le contrôle négatif n'a rien vu ». Un contrôle qui ne voit pas son écart, qui le voit ailleurs qu'au `n + 1`-ième appel, sans objet (trace trop courte), sans passe, non jugé (nombre d'appels différent, capture illisible) ou sans aucun module de M1 rejoué fait le code 2. Un contrôle réussi rend toujours 1. Le binaire le dit en une ligne. Documents : PHASE-3 § 7, `AGENTS.md`, `crates/README.md`, `mesures/navigateur/README.md`, `CHANGELOG.md`, la documentation de `soiree-banc` et de `etalonner`.
3. **[mineur] Les tours 1 et 2 ne sont pas passés par le portillon de l'hôte.** Vérifié : leurs `commande.txt` nomment une copie de `lourd` dans le dossier temporaire de la session, qui n'existe plus (le dossier ne contient plus qu'un script de la vérification) ; elle ne peut pas être comparée au portillon. Ses lignes de sortie ont la forme exacte du portillon, et ses charges au départ sont confirmées par les relevés indépendants du banc et du harnais, sauf le 3,07 que lit le harnais au premier module de son tour 1. Ces tours ne sont pas écartés (la règle de validité n'était pas posée avant), ni rejoués (ce serait rejouer jusqu'à passer) ; c'est écrit dans `MESURES-PHASE-3.md` et à la question 27, avec l'étendue sans eux, qui ne change pas le verdict.
4. **[mineur] « Sous 3 » ne vaut que pour la charge au départ.** Vérifié. Fait : `MESURES-PHASE-3.md` donne, tour par tour, la plus haute charge sur une minute pendant la partie Chrome (banc, avant et après chaque module : 3,95 ; 5,26 ; 5,22 ; 2,50 ; 3,92 ; 3,12 ; 2,99 ; 3,14 ; harnais, au départ de chaque module seulement : 4,34 ; 2,75 ; 2,24 ; 2,97 ; 4,33 ; 3,06 ; 2,66 ; 3,00). Les deux tours du banc au-dessus de 5 sont ceux des deux médianes au-dessus ; au harnais, le tour 5, à 4,33, reste à 37 ps : un lien n'est ni établi ni exclu. La question 27 demande s'il faut à M1 une règle de validité posée avant les tours, sur la charge pendant la mesure.
5. **[mineur] La ligne du lot 13 ne disait pas que les queues de M1 s'y remesurent.** Vérifié. Fait : la colonne « Ce qui le prouve fini » du lot 13 dit maintenant, « reporté du lot 3c », les queues de M1 remesurées dans Chrome et Firefox sur machine calme, banc et harnais retiré en alternance, plusieurs tours, la charge relevée pendant chaque tour, et la médiane de Chrome avec elles ; ce qui ne se retrouve pas va à la question 27.
6. **[mineur] Le plan standard des trois jeux sous `PHASE_3` et E20 n'avaient aucune preuve gardée.** Vérifié. Fait : relancés, preuves gardées (tableau plus bas). Le plan standard rend **0** pour Punchline, Sidequest et Tier list ; E20 sur le plan court, dans Chrome et Firefox, est réussi pour chacun, son contrôle négatif vu à l'appel 100, et les codes rendent **la référence du § 7** : Sidequest 0 ; Punchline 1, E2 seul (« une fin, mais aucune autre après la nouvelle partie », 13 783 lignes) ; Tier list 1, E15 seul (201 670 au début, 213 975 à la fin). Les modules éprouvés ont les empreintes du lot (Punchline `ad1b7b04…`, Sidequest `8112385d…`, Tier list `73552646…`, telles que l'épreuve les écrit), et leur `sha256` est gardé dans chaque preuve (`module.sha256`).

Aucun constat n'est écarté.

### Les tests qui le tiennent

- `banc`, `etalonner::tests::un_ecart_avec_wasmtime_fait_le_code_1`, étendu : sans écart 0 ; un écart 1 ; le contrôle qui voit son écart au bon appel 1, sans contrôle raté ; qui ne voit rien 2 (une ligne par passe, aucun défaut) ; trop tôt 2 (l'écart en défaut, le contrôle raté l'emporte) ; trace trop courte 2 ; nombre d'appels différent 2 ; `--fautif` sans module de M1 2 ; un passage non mesuré 2 (né du constat 2 de cette vérification).
- Dans les navigateurs, sur `devine` : `--fautif fausser-apres:100`, **code 1**, 7 840 écarts par passe, le premier à l'appel 101, aucun contrôle raté ; `--fautif fausser-apres:8000` (la trace n'a que 7 940 appels), **code 2**, « CONTRÔLE NÉGATIF RATÉ », un par navigateur : le code distingue maintenant les deux.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-3c/seconde-correction/` (hors dépôt), chaque dossier comme ceux de la fin du lot (`commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, et `module.sha256` pour une épreuve). Le commun est à `f4fba17` ; le code n'a pas changé depuis 18 h 17 : `git diff HEAD -- . ':(exclude)*.md'` vaut `5930472b4ee92cce…` du premier au dernier de ces passages (`target/travail/lot-3c/empreinte-code-1817.txt`, puis `empreinte-code.txt` à la fin) ; seuls des `.md` ont changé entre les preuves, d'où des `diff HEAD` différents dans leurs empreintes. Les jeux n'ont aucun diff. Le module de M1 est toujours l'exemple `etalonnage-navigateur` de 03 h 02 (`7fc15e87…`) : `web/` n'a pas changé.

| Preuve | Commande | Résultat |
| --- | --- | --- |
| `build-release` | `lourd cargo build --release -p banc -p epreuve` | code 0, 7 s |
| `m1-fautif` | `lourd target/release/soiree-banc etalonner --mesures m1 --modules devine --navigateurs chrome,firefox --fautif fausser-apres:100 --sans-construire` | **code 1**, écart à l'appel 101 dans les deux, aucun contrôle raté, 5 s |
| `m1-fautif-sans-objet` | la même, `--fautif fausser-apres:8000` | **code 2**, contrôle négatif raté (sans objet) dans les deux, 4 s |
| `m1-trace-abimee` | la même sans `--fautif`, `--donnees target/travail/lot-3c/donnees-abimee` | **code 1**, 0 écart et 2 pannes par passe dans les deux, 4 s |
| `m9` | `lourd target/release/soiree-banc etalonner --mesures m9 --navigateurs chrome,firefox --sans-construire` | code 0 ; 247 et 480 (Chrome), 492 et 893 (Firefox) ; chaque récursion sous `PHASE_3` en faute `Profondeur` ; 3 s |
| `plan-standard-punchline`, `-sidequest`, `-tier-list` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/<jeu>` | **code 0** chacun ; 99 s, 502 s, 164 s ; 1,61, 2,49 et 1,02 Go au plus |
| `e20-court-punchline`, `-sidequest`, `-tier-list` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/<jeu> --court --navigateurs chrome,firefox` | 1, 0, 1 : la référence du § 7 ; E20 réussi dans les deux navigateurs pour chacun ; 31 s, 163 s, 57 s |
| `test-banc` | `lourd cargo test -p banc` | code 0, 36 réussis |
| `clippy-commun`, `clippy-web-wasm`, `clippy-web-natif` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; dans `web/`, `--target wasm32-unknown-unknown` et en natif | aucun avertissement |
| `test-web` | dans `web/`, `lourd cargo test` | code 0, 9 réussis |
| `fmt` | `cargo fmt --all --check`, commun et `web/` | rien à reformater |
| `regles-web` | `lourd target/release/soiree-banc regles-web` | code 0 |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0** : 502 réussis, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 263 s ; 1,62 Go au plus |

Non relancés : les seize tours de M1 (le code de M1 n'a pas changé : l'exemple et sa page sont les mêmes ; la vérification les a recalculés depuis les données brutes), M10, `soiree-banc machine` ; Safari reste « pas exécuté », aucun réglage de la machine touché.

État laissé : aucun navigateur sans interface, serveur ni tâche de fond de moi. Les scripts de cette correction sont dans `target/travail/lot-3c/` (`preuve2.sh`, `epreuves.sh`, `finales.sh`).

### Les mesures

Aucune mesure de temps nouvelle : les durées ci-dessus sont celles des commandes, sous la charge que le portillon a notée, pas des mesures au sens du § 8.6. La mémoire : le plan standard de Sidequest 2,49 Go au plus, `cargo test --workspace` 1,62 Go (`/usr/bin/time -l`, le plus gros processus).

### Ce qui n'est pas fait, et pourquoi

- **Le critère changé du lot 3c n'est pas tenu dans Chrome** (constat 1) : deux médianes du banc sur huit, 46 et 49 ps, au-dessus de l'étendue du harnais (36 à 43). Le lot n'est donc pas fini selon sa colonne. Ce n'est pas corrigeable ici sans affaiblir le critère : à trancher par le mainteneur (question 27), ou à remesurer au lot 13 (sa ligne le dit maintenant).
- La cause des queues, et des deux médianes de Chrome, n'est pas cherchée ; la charge pendant la partie Chrome est notée, sans conclusion.

### Ce que le lot suivant doit savoir

- `etalonner --fautif fausser-apres:<n>` rend 1 quand le contrôle négatif voit son écart au `n + 1`-ième appel, et **2** quand il le rate (`controle_rate`) : un contrôle automatique qui lit le code doit attendre 1, et seulement 1.
- Le lot 13 reprend les queues de M1 (sa ligne au § 7) ; poser avant les tours la règle de validité sur la charge pendant la mesure, si le mainteneur la veut (question 27).
