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

## Lot 4a — La liaison et le chiffrement

Fait le 2026-11-06, de 18 h 45 à 19 h 25 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Chaque commande cargo lourde est passée par le portillon (`target/hote/lourd`). Aucun navigateur, aucun serveur. Rien n'est commité.

### Ce qui est fait

**`crates/liaison`** (nouvelle, `no_std`, sans `unsafe`) : les trames en clair du § 2.7, `VersRelais` et `DuRelais`, avec `CodeSalle` (8 chiffres hexadécimaux majuscules, `lire` strict), `Lien`, `Role`, `Classe`, `Case`, `AdresseIp`, `AdresseAnnoncee`, `RefusRelais`, `RaisonCoupure`, `CauseFermeture` ; `PROTOCOLE_RELAIS` (1), `TRAME_MAX` (256 Kio), `BATTEMENT_MS` (250), `SILENCE_MAX_MS` (3 000) et `SILENCE_MAX` (180 pas), `LIENS_MAX_SANS_AUTORITE` (16), `REDITE_MIN` (100 ms), `REDITE_MAX` (250 ms), `periode_de_redite` (P27 ; 150 ms chez Sidequest, rien chez Punchline) ; `protocole_d_un_bonjour`, qui lit le protocole d'un bonjour illisible (SAL-40). `Bonjour` reste le premier variant, `protocole` son premier champ.

**`crates/salle`** (nouvelle, sans `unsafe`, sans entrées-sorties), modules `chiffre` et `fil` :
- `chiffre` : la poignée de main `Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s` par `snow`, prologue `soiree fil 1`, deux messages de 48 octets exactement (sans charge : la poignée de main ne porte rien, SAL-17) ; `Initiateur::commencer` / `finir` pour l'appareil, `repondre` pour l'autorité, dont la session n'est confirmée (`est_confirmee`) qu'à la première trame ouverte. Les trames : ChaCha20-Poly1305 sous les deux clés du `Split` (`risky-raw-split`), nonce de Noise, `nonce (8, petit-boutiste) ‖ chiffré ‖ étiquette`, la classe en données associées, fenêtre de rejeu de 1 024 qui n'avance qu'après authentification, jusqu'à `TRAME_MAX`. Les cases de la garde : XChaCha20-Poly1305, clé `derive_key("soiree cases de garde 1", G)`, en-tête de chaîne en données associées, `empreinte_case`. Les clés : `SecretDEntree` et sa psk (`derive_key("soiree secret d'entree 1", …)`), `PaireAutorite`, `ClePublique`, `CleDeGarde` et sa preuve (`derive_key("soiree preuve d'autorite 1", G)`) ; leurs `Debug` ne montrent aucun secret. `Fragment`, le fragment du QR en base64url strict (SAL-119). Le hasard entre par le trait `Hasard` ; `snow` reçoit un résolveur qui sert une seule fois les 32 octets d'une clé tirés de ce hasard, et n'a ni `std` ni `getrandom`.
- `fil` : `VersAutorite`, `VersAppareil`, `TraceTelephone`, `LimitesSalle`, `RefusSalle`, `AttenteCommun`, `CoupBanc`, `PROTOCOLE` (1), `protocole_d_un_bonjour` (SAL-40).

**`epreuve/src/telephone.rs`** relit `liaison::SILENCE_MAX` (`SILENCE_MAX`, 180 pas) et `liaison::REDITE_MAX` (`RAPPEL_TENU`, 15 pas), aux mêmes valeurs. `epreuve` dépend de `liaison`.

**Les dépendances** : `snow` 0.10.0 (la dernière, 2025-07-19 ; sans fonctionnalités par défaut ; `use-curve25519`, `use-chacha20poly1305`, `use-blake2`, `risky-raw-split`) et `chacha20poly1305` 0.10.1 (celle que `snow` tire ; sans fonctionnalités par défaut), déclarées dans `[workspace.dependencies]` avec leur raison, versions vérifiées sur crates.io (`cargo search`, `cargo info`, l'API des versions). Elles ajoutent au `Cargo.lock` `aead`, `blake2`, `chacha20` 0.9.1, `cipher`, `curve25519-dalek` 4.1.3, `fiat-crypto`, `inout`, `opaque-debug`, `poly1305`, `rustc_version`, `subtle`, `universal-hash`, `zeroize`, et `curve25519-dalek-derive` avec `syn` 2, qui ne se compilent que sur `x86_64` ; aucune version existante ne change.

Documents : PHASE-3 (§ 2.2, § 2.6, § 2.7, ligne du lot 4a au § 7, M7, annexe des dépendances), CHANGELOG, `crates/README.md`, `AGENTS.md`, le commentaire de tête de `Cargo.toml`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **`periode_de_redite` n'est pas relue par l'épreuve.** La ligne du lot disait que `telephone.rs` relit `SILENCE_MAX_MS` et `periode_de_redite`, et la règle « Le plan court ou le plan standard » que 4a ne change aucune valeur. Les deux ne tiennent pas ensemble : le téléphone simulé redit à 250 ms pour tous les jeux, et `periode_de_redite` vaut 150 ms chez Sidequest et rien chez Punchline. La relire changerait ce que l'épreuve joue. L'épreuve relit donc `REDITE_MAX` (250 ms, la plus longue redite de P27), et passe à la période propre à chaque jeu avec M7, au lot 12, qui jouera le plan standard des trois jeux.
2. **Le transport n'est pas celui de `snow`.** Il ne prend pas de données associées, et un message de transport de Noise ne dépasse pas 65 535 octets, quand `TRAME_MAX` en permet 256 Kio (et que la vue d'un téléphone de Sidequest peut faire 128 Kio, `limites.vue`). Les trames sont donc chiffrées directement sous les clés du `Split` (§ 2.6). Le vecteur public tient la poignée de main, le `Split` et ce chiffrement.
3. **`salle` dépend de `jeu-abi` et de `blake3`**, et pas encore de `partie`, qui viendra au lot 4b (§ 2.2).
4. **`adresse_jointe` est une `AdresseIp`** (v4 ou v6), et non `[u8; 4]` : le relais écoute aussi sur `[::1]`, et `--adresse` peut en imposer une en IPv6. `CapacitesFil` est `jeu_abi::Capacites`. Les types que le § 2.7 nommait sans les définir sont écrits (§ 2.7).
5. **`cargo tree -i getrandom` dans `web/`** ne trouve aucun paquet : `salle` n'y entre pas avant le lot 10a. Il est lancé aussi sur `salle` et `liaison` dans le commun (ci-dessous).
6. `chacha20poly1305` est en 0.10.1, et non en 0.11.0, la plus récente publiée depuis plus de deux semaines (règle de l'annexe) : c'est la version que `snow` 0.10 tire, et deux copies grossiraient le paquet du téléphone.

### Les tests qui le tiennent

Dans `crates/liaison` :
- `tests/forme.rs` : `les_octets_des_trames_vers_le_relais_sont_figes`, `les_octets_des_trames_du_relais_sont_figes` (chaque variant de chaque enum, 18 et 26 trames) ; `une_seconde_lecture_relit_chaque_trame_sal90` (un lecteur écrit à la main, qui rebâtit chaque valeur) ; `les_octets_figes_couvrent_chaque_variant` (les `nom_*` sans `_`) ; `une_trame_figee_allongee_ou_raccourcie_est_refusee_sal118` ; `les_constantes_du_lien_sont_figees` ; `la_periode_de_redite_est_le_tiers_de_la_plus_courte_borne` ; `un_bonjour_d_un_autre_protocole_se_lit_meme_casse_sal40` ; sous proptest (4 096 cas), `des_octets_au_hasard_ne_font_jamais_paniquer_les_decodeurs_rem64` et `une_trame_abimee_ne_fait_jamais_paniquer_rem64` (ce qui se décode se réencode en un message égal).
- `src/lib.rs` : `un_code_de_salle_s_ecrit_et_se_relit`, `un_code_de_salle_d_une_autre_forme_est_refuse_sal119`, `le_varint_d_un_u16_se_lit`.

Dans `crates/salle` :
- `src/chiffre.rs` : **`les_vecteurs_publics_de_noise_nkpsk0_se_retrouvent`** (le vecteur de cacophony que livre `snow` 0.10.0, `tests/vectors/cacophony.txt`, l. 10137 à 10173 : les deux messages de la poignée de main, avec charges, l'empreinte de la poignée de main, et quatre messages de transport aux nonces 0 et 1, retrouvés octet pour octet) ; `la_fenetre_de_rejeu_suit_son_modele_sur_une_suite_fixe`, et `la_fenetre_de_rejeu_suit_son_modele` sous proptest (512 cas).
- `tests/chiffre.rs` : **`la_psk_derive_du_secret_d_entree_octets_figes`** ; `la_preuve_d_autorite_derive_de_g_octets_figes` ; `la_cle_publique_de_l_autorite_suit_x25519_rfc7748` (RFC 7748, § 6.1) ; `le_prologue_est_fige` ; `une_poignee_de_main_ouvre_une_session_dans_les_deux_sens` ; `les_octets_d_une_poignee_de_main_et_de_ses_trames_sont_figes` (à graine fixe) ; `un_secret_d_une_autre_soiree_refuse_l_entree` ; `un_invite_ne_se_fait_pas_passer_pour_l_autorite` ; `une_poignee_de_main_rejouee_ne_donne_aucune_session_confirmee_sal17` ; `une_poignee_de_main_qui_porte_le_jeton_est_refusee_sal17` (un premier message écrit avec `snow` directement) ; **`une_trame_rejouee_par_le_relais_est_refusee`** ; `une_classe_changee_par_le_relais_est_refusee` ; `une_trame_abimee_ou_d_une_autre_session_est_refusee_sans_rien_changer` (SAL-117) ; `une_trame_va_jusqu_a_trame_max` ; `une_case_se_scelle_et_se_rouvre_octets_figes` ; `une_case_de_garde_intervertie_est_refusee` ; `le_fragment_s_ecrit_et_se_relit` ; `un_fragment_d_une_autre_forme_est_refuse_sal119` ; sous proptest (1 024 cas), `des_octets_au_hasard_ne_font_jamais_paniquer_le_dechiffrement_rem64` et `une_trame_ou_une_case_abimee_est_refusee_rem64`.
- `tests/fil.rs` : `les_octets_des_messages_vers_l_autorite_sont_figes`, `les_octets_des_messages_vers_un_appareil_sont_figes` (6 et 18 messages, chaque variant) ; `une_seconde_lecture_relit_chaque_message_sal90` ; `les_octets_figes_couvrent_chaque_variant` ; `le_protocole_du_fil_est_fige` ; `un_message_fige_allonge_ou_raccourci_est_refuse_sal118` ; `le_bonjour_d_une_page_d_un_autre_protocole_se_lit_meme_casse_sal40` ; sous proptest (4 096 cas), `des_octets_au_hasard_ne_font_jamais_paniquer_les_decodeurs_rem64` et `un_message_abime_ne_fait_jamais_paniquer_rem64`.

Dans `crates/epreuve`, `telephone::tests::les_constantes_relues_de_liaison_sont_celles_d_avant` : l'égalité des constantes relues avec celles d'avant le lot (180 et 15 pas), et `periode_de_redite` de Sidequest (9 pas) et de Punchline (rien). C'est la preuve que demande la règle du § 7 pour ce lot, à la place d'une épreuve : aucune épreuve n'a été lancée.

**Les contrôles négatifs**, faits par `target/travail/lot-4a/controles.py` (un changement d'une ligne, le test qui doit rougir, le fichier remis tel qu'il était) ; chacun rouge, code 101 :

| Changement | Rougit |
| --- | --- |
| le nonce de Noise en gros-boutiste | `les_vecteurs_publics_de_noise_nkpsk0_se_retrouvent` |
| sans la vérification de la fenêtre de rejeu | `une_trame_rejouee_par_le_relais_est_refusee` |
| la classe hors des données associées | `une_classe_changee_par_le_relais_est_refusee` |
| un autre contexte de dérivation de la psk | `la_psk_derive_du_secret_d_entree_octets_figes` |
| l'empreinte de la case précédente hors des données associées | `une_case_de_garde_intervertie_est_refusee` |
| sans la longueur exacte ni le refus d'une charge dans la poignée de main | `une_poignee_de_main_qui_porte_le_jeton_est_refusee_sal17` (une première version ne retirait que la longueur, et le test restait vert : le refus d'une charge le tenait encore) |
| deux champs de `VersRelais::Vers` intervertis | `les_octets_des_trames_vers_le_relais_sont_figes` |
| deux champs de `VersAutorite::Geste` intervertis | `les_octets_des_messages_vers_l_autorite_sont_figes` |
| `SILENCE_MAX_MS` à 5 000 ; `REDITE_MAX` à 200 ms | `les_constantes_relues_de_liaison_sont_celles_d_avant` |

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-4a/` (hors dépôt) ; chaque dossier a `commande.txt`, `sortie.txt` (complète, avec la charge et la mémoire que le portillon écrit au départ et à la fin), `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (pour le commun et chaque jeu : `HEAD`, `sha256` de `git diff HEAD`, des fichiers non suivis, et de `git diff HEAD` hors des `.md`). Le commun est à `bc4e885` ; les jeux sont à leur `HEAD`, sans aucun changement (Punchline `dd6d4f0`, Sidequest `6e7219c`, Tier list `eebc5c1`). Le code n'a pas changé du premier au dernier de ces passages : `diff HEAD` hors `.md` vaut `4ced2f42…` et les non suivis `7a1bcee0…` dans chaque empreinte ; seuls des `.md` ont changé entre eux, et après eux (PHASE-3, ce journal).

| Preuve | Commande | Résultat |
| --- | --- | --- |
| `fmt` | `cargo fmt --all --check` (et dans `web/`, inchangé) | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0, aucun avertissement ; 13 s ; charge 4,06 au départ |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 548 réussis** (les 502 du lot 3c et 46 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 318 s ; 1,64 Go de mémoire résidente au plus (le plus gros processus) ; charge 3,74 au départ, 4,10 à la fin |
| `wasm32` | `lourd /usr/bin/time -l cargo build -p liaison -p salle --target wasm32-unknown-unknown --release` | code 0 ; 0,19 Go au plus (déjà compilé ; la première compilation, plus tôt dans le lot, prenait 2,9 s et 0,31 Go) |
| `getrandom` | `target/travail/lot-4a/getrandom.sh` | `cargo tree -p salle -i getrandom --target wasm32-unknown-unknown -e normal,build` : rien ; de même pour `liaison` ; dans tout le commun, `getrandom` 0.4.3 seul, par `rand` (sous `proptest`, en test, et `tungstenite`, le banc) ; dans `web/`, aucun paquet `getrandom` |
| `controles-negatifs` | `python3 -I target/travail/lot-4a/controles.py` | les dix contrôles du tableau, rouges ; un sous-dossier par contrôle |

Non lancés, parce que ce lot ne change ni ce que l'épreuve joue ni ce qu'elle juge (§ 7, « Le plan court ou le plan standard », qui demande ici un test d'égalité et non une épreuve) : le plan court et le plan standard des trois jeux, E20, `soiree-banc`. `web/` n'a pas changé : ses tests, son clippy et la règle « mêmes lints » non plus.

### Les mesures

Aucune mesure nommée (M1 à M15) n'est de ce lot. La mémoire de ses commandes : `cargo test --workspace` 1,64 Go au plus, sous la charge notée ; la compilation de `liaison` et `salle` pour `wasm32`, 0,31 Go la première fois. Estimé au § 7 : « 1 à 2 Go ». Les durées ci-dessus sont celles de commandes sous une charge de 1,8 à 4,1, pas des mesures.

### Ce qui n'est pas fait, et pourquoi

- `une_poignee_de_main_rejouee_ne_remplace_personne_sal17`, le test du § 8.1, demande `salle::Autorite` (lot 4b). Ce lot en tient la part du chiffrement : une poignée de main rejouée donne une session non confirmée, où rien ne s'ouvre (`une_poignee_de_main_rejouee_ne_donne_aucune_session_confirmee_sal17`).
- La chaîne entière de la garde (refuser une garde plus vieille que le dernier `n`, `Effacer`) est au lot 4b ; ici, seule une case se scelle, s'ouvre sous son en-tête, et refuse un autre en-tête.
- Le monde simulé ne passe pas à `periode_de_redite` (écart 1).
- Les sessions ne se gardent pas : une autorité qui redémarre refait la poignée de main avec chaque téléphone (lot 4b).
- Aucun test dans un navigateur : rien de `salle` ne tourne encore dans `web/`. La compilation pour `wasm32` et clippy pour cette cible sont faits.

### Ce que le lot suivant doit savoir

- Le hasard entre par `salle::chiffre::Hasard` ; les tests en tirent un à graine fixe par BLAKE3 en mode extensible (`tests/chiffre.rs`, `Graine`). Chaque clé tirée (paire, éphémère, nonce de case) consomme le générateur : changer l'ordre des tirages change les octets figés de `les_octets_d_une_poignee_de_main_et_de_ses_trames_sont_figes`.
- L'autorité ne doit rien annoncer (présence, `Remplace`, accueil, place) avant `Session::est_confirmee` ; le premier `ouvrir` réussi la confirme, et le premier message de transport est `VersAutorite::Bonjour`.
- `Session::ouvrir` prend la classe que dit le relais (`DuRelais::De`, `classe`) : une trame dont la classe a été changée est refusée sans consommer son nonce. Une trame refusée doit couper son seul lien (`RaisonCoupure::Illisible`, SAL-117).
- `TRAME_MAX` borne la trame chiffrée ; le clair va jusqu'à `CLAIR_MAX` (262 120 octets). `VersRelais::Ecrire` d'un point de Tier list (16,8 Mio au pire, M3) dépasse de loin cette borne : la garde (lots 4b et 5) devra découper un point en cases, ou donner au lien de garde sa propre borne.
- La forme des cases et de leurs données associées est figée ici ; le registre et le dernier `n` viennent au lot 4b.
- Tier list verra son `Cargo.lock` gagner `liaison` à sa prochaine compilation (CHANGELOG).

## Lot 4b — La salle, sans entrées-sorties

Fait le 2026-11-06, de 19 h 30 à 20 h 30 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 26.6.2, rustc 1.99.0). Chaque commande cargo lourde est passée par le portillon (`target/hote/lourd`). Aucun navigateur, aucun serveur. Rien n'est commité. Le code des jeux n'est pas touché ; le `Cargo.lock` de Tier list, lui, a gagné `liaison` pendant le lot, en compilant l'épreuve (voir « Ce que le lot suivant doit savoir ») : un changement non commité dans le dépôt de Tier list.

### Ce qui est fait

**Dans `crates/salle`** (sans `unsafe`, sans entrées-sorties ; le temps entre en microsecondes et le hasard par `chiffre::Hasard`) :

- **`autorite`** : `Autorite<F: Fabrique, H: Hasard>`, la partie qui fait foi derrière ses deux liens au relais. Le démarrage : bonjour au relais (`Role::Autorite`), puis la garde (`Role::Garde`), `Lister`, `Lire` chaque case, et la partie ouverte (garde vide : partie neuve, son premier point écrit aussitôt) ou reprise (`garde::relire`, `Partie::reprendre`, le registre restauré, la présence de chaque appareil relue dans la partie reprise) ; ce qui arrive des appareils entre-temps attend. Une garde illisible, plus vieille que le dernier numéro que la coque sait écrit, ou vide alors qu'elle en sait un, arrête l'autorité (`Arret`), sans partie neuve en silence. Les appareils : poignée de main sur un lien ouvert, session non confirmée, et **rien avant le bonjour chiffré** (SAL-17) ; une trame qui ne s'ouvre pas, ou un message avant le bonjour, coupe son seul lien (SAL-117) ; une poignée de main sans bonjour en 10 s est coupée ; **le lien le plus récent gagne** dans l'ordre d'ouverture du relais, l'ancien reçoit `Refus(Remplace)` et n'est plus écouté (SAL-18, SAL-19, SAL-21) ; un bonjour répété sur le même lien redonne le même accueil, même session, sans présence (SAL-29) ; l'accueil dit le dernier geste placé de l'onglet et le dernier tenu reçu (`Partie::numero_tenu`, SAL-38), puis accuse ce qui est déjà écrit ; un geste se place une fois par (appareil, onglet), à la suite (SAL-57), avec son identité `id_geste` = `blake3(appareil ‖ onglet ‖ numéro)` tronqué ; il n'est accusé qu'une fois écrite la case de journal qui le contient (SAL-56) ; huit onglets retenus par appareil, ceux qui ont livré (SAL-37) ; la demi-latence mesurée par l'autorité (`AllerRetour`, SAL-129) ; un appareil silencieux plus de `SILENCE_MAX_MS` est dit parti, et revient au premier message ; la salle pleine se refuse (`RefusSalle::Pleine`). Le tour (`reveiller`, sur le signal d'image de la page ou sa minuterie) : `avancer_borne(pas, ARRIERES_PAR_TOUR)`, les avis, les demandes à la page, les vues (une seule lecture `regarder` pour l'écran et chaque appareil accueilli, une vue envoyée seulement si elle change, en trame remplaçable), les échos, les silences, la garde (une case de journal toutes les 100 ms qui ont écrit, un point tous les 600 pas, `Effacer` après chaque point écrit, le journal de la partie tronqué), l'état de la salle et le battement à la page. **Le mode banc** (`ReglagesSalle::banc`) : les coups du bot de chaque appareil accueilli (`Partie::bot`), envoyés par `VersAppareil::Banc`, et ceux du bot de l'écran, entrés comme des entrées de l'écran.
- **`appareil`** : `Appareil<H>`, le cœur d'un téléphone sans sa page : bonjour au relais, poignée de main sur `AutoritePresente`, bonjour chiffré redit s'il reste sans accueil 2 s, gestes numérotés en file jusqu'à leur accusé (`Envoi::FilePleine`, `Envoi::TropLong`), renvoyés après chaque accueil au-delà du dernier placé, tenu numéroté au-dessus de ce que l'accueil dit et redit à sa période, horloge de l'autorité par écho, réponse aux échos de l'autorité, vue, avis, refus final ; `recharger` (jeton, onglet et file gardés, P26), `reprendre_ici`, `taire_le_banc` (la fin d'une soirée du banc) et `renvoyer_la_file` (un appareil qui renvoie ce qui est placé : le banc s'en sert pour le dédoublonnage).
- **`garde`** : `Registre` (empreinte du jeton → appareil, onglets et dernier geste écrit), `ContenuPoint`, `ContenuJournal`, `VERSION_CASES` = 1 et leur forme ; `Ecrivain` (une seule suite de numéros, chaque case scellée à la suite de la précédente, le dernier numéro écrit, le numéro avant lequel effacer) ; `relire` (`RefusDeGarde` : `Vide`, `PlusVieille`, `Doublon`, `Trouee`, `SansPoint`, `Scellee`, `Contenu`, `Partie`).
- **`horloge`** : `Horloge` (le temps de la partie depuis l'ouverture), `instant_de_la_partie`, `debut_du_pas`, `duree_de_us` (au pas supérieur), `Estimation` (SAL-127, SAL-128), `AllerRetour` (SAL-129).
- **`ecran`** : `VersPage`, `DeLaPage`, `CommandeEcran`, `EtatSalle`, `AppareilVu`, `PanneSalle`, `RapportPanne`, `Trace`, `IdTrace`, `PROTOCOLE` = 1.
- **`simulation`** : `RelaisEnMemoire` (le modèle du relais : rôles, autorité plus récente qui remplace l'ancienne, garde en mémoire et son historique), `Soiree` (le réseau à latence par sens et ordre gardé par lien, la page réduite à son image et à des refus aux demandes, les pannes tirées de la graine : coupure, rechargement, onglet jumeau, poignée de main rejouée, autorité tuée et relancée depuis la garde ; puis une fin calme), `Graine`, `verifier_l_historique` (la chaîne entière des cases depuis la première, le journal d'un seul tenant, chaque geste au plus une fois) et `Soiree::comparer_a_la_garde` (la partie reprise de la garde finale a la forme canonique et l'empreinte de l'autorité vivante).
- L'exemple **`soiree_en_memoire`** : `cargo run --release -p salle --example soiree_en_memoire -- <module.wasm> [--telephones N] [--duree S] [--graines 1,2,3]` ; code 0 si tout tient, 1 sinon, 2 si l'outil n'a pas pu jouer.

**Dans `crates/partie`**, deux lectures, sans rien changer à ce qu'elle joue : `Partie::bot` (`Appel::Bot` sur la définitive, rien pendant un pas inachevé, une faute traitée comme celle d'une lecture) et `Partie::presence`.

Documents : PHASE-3 (§ 2.2, § 2.4, § 2.5, § 2.7, ligne du lot 4b au § 7, annexe des valeurs choisies), CHANGELOG, `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **`salle::simulation` est dans la crate**, et non dans ses seuls tests : l'exemple qui joue les trois jeux et les tests partagent le même code (§ 2.2).
2. **`partie` gagne `Partie::bot` et `Partie::presence`** (§ 2.2) : l'autorité du banc lit le bot sur la définitive, et une autorité qui reprend relit la présence dans la partie reprise, pas dans ce que l'autorité morte avait placé.
3. **La garde** (§ 2.4) : une seule suite de numéros ; un point ne s'écrit qu'entre deux pas, au bord du journal écrit ; chaque case porte le registre entier, et non « ce que la salle a appris pendant ce temps » ; le registre ne garde pas le dernier tenu (la partie reprise le dit) ; la case qui précède le dernier point n'est que hachée ; une garde vide alors qu'un numéro est connu est refusée. Aucune borne de taille : la case de point de Tier list au pire (16,8 Mio, M3) dépasse `TRAME_MAX`, et le lot 5 doit en décider.
4. **Ce que la salle attend du relais** (§ 2.5) : `AutoritePresente` ou `AutoriteAbsente` après l'accueil d'un appareil ; `Ouvert` pour chaque appareil déjà là et `AutoritePresente` à chacun quand une autorité se présente ; `AutoriteAbsente` quand elle part ; `Ferme` après un `Couper`. Le modèle en mémoire le fait ; le relais du lot 5 doit le faire aussi.
5. **`salle::ecran`** (§ 2.7) emprunte ses octets et porte `DemandeFil`, `EcranFil`, `ReponseFil` de `jeu-abi`, et non les types possédés de `partie` ; `ChangerDeJeu` porte l'empreinte du module. Le temps donné aux appareils (`Echo`, `Accueil.heure`) est le temps de la partie depuis le pas 0 (le commentaire de `fil::VersAppareil::Accueil` le dit ; la forme ne change pas).
6. **Le mode banc fait entrer les coups du bot de l'écran sans page** : Tier list, sans téléphone, ne jouerait rien sinon. La page du banc répond à chaque demande par un refus (un téléchargement et une ligne : fait).
7. Valeurs choisies nouvelles, à l'annexe : le bonjour redit en 2 s, 16 échos retenus, 32 gestes en file avant l'accueil, le bot consulté tous les 15 pas en mode banc, la case écrite en 1 à 20 ms par le relais en mémoire.

### Les tests qui le tiennent

Dans `crates/salle` (64 tests, dont 32 nouveaux) :
- `tests/salle.rs`, les tests du § 8.1 qui touchent la salle, chacun avec sa leçon : **`rejoindre_est_idempotent_sal29`**, **`le_lien_le_plus_recent_gagne_sal19`** (un bonjour coincé sur une tentative abandonnée, livré après celui du lien suivant, ne déloge personne ; puis un onglet plus récent remplace le courant, qui ne revient pas seul), **`une_poignee_de_main_rejouee_ne_remplace_personne_sal17`** (ni remplacement, ni présence, ni retour d'un téléphone parti ; la poignée de l'invité coupée après 10 s), **`l_accueil_realigne_les_tenus_sal38`**, **`un_geste_compte_une_fois_a_travers_le_redemarrage_de_l_autorite_sal57`** (placé mais pas écrit, puis renvoyé à la suivante ; écrit, l'autorité morte juste après la case qui le contient et son accusé perdu ; renvoyé déjà placé), `un_geste_n_est_accuse_qu_une_fois_ecrit_sal56` (le relais répondait `Ecrit` de 5 à 30 ms après l'`Ecrire`, mais tenait la case pour écrite dès l'`Ecrire` : le test ne voyait pas un accusé envoyé dès l'`Ecrire` ; corrigé après la vérification, voir « Lot 4b, correction après la vérification »), et `des_octets_au_hasard_ne_font_paniquer_ni_l_autorite_ni_un_appareil_rem64` (proptest, 256 cas).
- `tests/garde.rs` : `la_garde_tient_au_plus_deux_points_et_se_relit`, `une_garde_plus_vieille_que_le_dernier_n_est_refusee`, `une_case_de_garde_intervertie_dans_la_chaine_est_refusee` (interverties, renommée, retirée du milieu, abîmée, la case avant le point changée, une autre salle, une autre clé, un doublon) ; `la_version_des_cases_est_figee`, `les_octets_des_contenus_de_case_sont_figes`, `une_seconde_lecture_relit_chaque_contenu_sal90`, `un_contenu_fige_allonge_ou_raccourci_est_refuse_sal118`, et sous proptest (4 096 cas) `des_octets_au_hasard_ne_font_jamais_paniquer_les_contenus_rem64`.
- `tests/ecran.rs` : les octets de chaque variant des deux enums figés (13 et 10 messages), `les_octets_figes_couvrent_chaque_variant` (22 variants), `une_seconde_lecture_relit_chaque_message_sal90`, `le_protocole_de_l_ecran_est_fige`, `un_message_fige_allonge_ou_raccourci_est_refuse_sal118`, et sous proptest (4 096 cas) les décodeurs sans panique, ce qui se décode se réencodant égal (REM-64).
- `tests/horloge.rs` : `l_instant_de_la_partie_se_compte_en_pas`, `une_demi_latence_ne_se_reduit_jamais_en_se_convertissant`, `l_horloge_ne_depend_que_de_l_ouverture`, `l_heure_de_l_autorite_se_garde_au_plus_petit_aller_retour_sal128`, `un_appareil_peut_grossir_sa_latence_pas_la_reduire_sal129`.
- `tests/soiree.rs`, sur le modèle du cœur, dont l'état garde l'identité de chaque geste reçu : `une_soiree_sans_panne_compte_chaque_geste_une_fois` et `une_soiree_avec_ses_pannes_compte_chaque_geste_une_fois_sal57` (quatre graines, 60 s, six téléphones : la chaîne entière vérifiée, aucun geste en double, chaque geste accusé écrit, tout accusé à la fin sauf ce qu'un onglet jumeau fermé emporte, la garde finale qui reprend la même partie).
- `tests/devine.rs`, sur le module de `devine` réécrit et chargé dans wasmtime (`PHASE_3`) : `une_soiree_de_devine_en_memoire_survit_a_ses_pannes` et `une_soiree_en_memoire_se_refait_a_l_identique_sal09` (la même graine, la même chaîne, case pour case).

Les tests du § 8.1 que le lot 4a tenait (`une_trame_rejouee_par_le_relais_est_refusee`, `un_secret_d_une_autre_soiree_refuse_l_entree`, `un_invite_ne_se_fait_pas_passer_pour_l_autorite`, `une_case_de_garde_intervertie_est_refusee`, `la_psk_derive_du_secret_d_entree_octets_figes`) restent dans `tests/chiffre.rs` ; ceux de `relais/tests/` sont au lot 5 ; `un_maintenu_dont_la_cle_disparait_est_relache_rem23` vient avec la page (lot 12).

Dans `crates/partie`, `tests/coeur/partie.rs` : `le_bot_se_lit_sur_la_definitive_sans_rien_changer`, `la_presence_est_celle_que_la_definitive_sait`.

**Les contrôles négatifs**, faits par `target/travail/lot-4b/controles.py` (le changement, le test qui doit rougir, le fichier remis) ; chacun rouge, code 101 :

| Changement | Rougit |
| --- | --- |
| un geste déjà placé replacé (sans dédoublonnage ni suite) | `un_geste_compte_une_fois_a_travers_le_redemarrage_de_l_autorite_sal57` |
| le registre d'une case de journal sans les gestes de cette case | idem (le geste écrit compté deux fois après le redémarrage) |
| un geste en vol qui n'est dans aucune case compté écrit (l'accusé avant l'écriture) | `un_geste_n_est_accuse_qu_une_fois_ecrit_sal56` (une première version, sans latence d'écriture au relais, restait verte : elle a fait ajouter `Pannes::garde_us`) |
| le lien le plus ancien gagne | `le_lien_le_plus_recent_gagne_sal19` |
| une session neuve à chaque bonjour | `rejoindre_est_idempotent_sal29` |
| l'accueil sans le dernier tenu | `l_accueil_realigne_les_tenus_sal38` |
| une poignée de main jamais coupée | `une_poignee_de_main_rejouee_ne_remplace_personne_sal17` |
| aucun `Effacer` | `la_garde_tient_au_plus_deux_points_et_se_relit` |
| une garde d'un numéro de moins acceptée | `une_garde_plus_vieille_que_le_dernier_n_est_refusee` |
| une garde trouée acceptée | `une_case_de_garde_intervertie_dans_la_chaine_est_refusee` |
| l'heure de l'autorité au dernier écho, et non au meilleur | `l_heure_de_l_autorite_se_garde_au_plus_petit_aller_retour_sal128` |

Le premier essai de ces contrôles en a trouvé deux verts (le registre sans sa case, l'accusé avant l'écriture) : les tests ne les voyaient pas, parce qu'une case suivante corrigeait le registre, et qu'un relais sans latence répondait `Ecrit` avant tout geste placé que sa case ne contenait pas. Le test du SAL-57 tue désormais l'autorité juste après la case du geste, et le test du SAL-56 a été écrit avec un relais qui met de 5 à 30 ms à écrire.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-4b/` (hors dépôt) ; chaque dossier a `commande.txt`, `sortie.txt` (complète, avec la charge et la mémoire que le portillon écrit au départ et à la fin), `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (pour le commun et chaque jeu : `HEAD`, `sha256` de `git diff HEAD`, des fichiers non suivis, et de `git diff HEAD` hors des `.md`). Le commun est à `dceea7a` ; les jeux à leur `HEAD` (Punchline `dd6d4f0`, Sidequest `6e7219c`, Tier list `eebc5c1`). Le code suivi n'a pas changé d'une preuve à l'autre (`diff HEAD` hors `.md` : `31e783cf…` partout) ; les fichiers non suivis valent `01f1ec95…` partout, sauf pendant `build-epreuve`, `court-punchline` et `court-sidequest` (`e4e41d5b…`) : l'exemple `soiree_en_memoire` a reçu la période de redite du jeu pendant ces passages, et `epreuve` ne dépend pas de `salle` (`cargo tree -p epreuve` n'y trouve pas `salle`). Seuls des `.md` ont changé après.

| Preuve | Commande | Résultat |
| --- | --- | --- |
| `fmt` | `cargo fmt --all --check` (et dans `web/`, inchangé) | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0, aucun avertissement |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `wasm32` | `lourd /usr/bin/time -l cargo build -p salle --target wasm32-unknown-unknown --release` | code 0 ; 0,31 Go au plus |
| `getrandom` | `cargo tree -p salle -i getrandom --target wasm32-unknown-unknown -e normal,build`, puis le commun entier | rien pour `salle` (avec `partie` désormais) ; dans le commun, `getrandom` 0.4.3 seul, par `tungstenite` (le banc) |
| `test-salle` | `lourd /usr/bin/time -l cargo test -p salle` | code 0 : 64 réussis ; 0,28 Go au plus ; 14 s |
| `test-partie` | `lourd cargo test -p partie` | code 0 |
| `controles-negatifs` | `python3 -I target/travail/lot-4b/controles.py` | les onze contrôles du tableau, rouges (code 101 chacun) ; un sous-dossier par contrôle |
| `build-epreuve` | `lourd /usr/bin/time -l cargo build --release -p epreuve --bin soiree-epreuve` | code 0 (`partie` a changé) |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/<jeu> --court` | **la référence du § 7** : Punchline 1, E2 seul (« une fin, mais aucune autre après la nouvelle partie ») ; Sidequest 0 ; Tier list 1, E15 seul (octets vivants de 201 670 à 213 975) ; 30 s, 123 s, 41 s. Modules livrés (blake3 du rapport) `ad1b7b04…`, `8112385d…`, `73552646…` |
| `exemple-release` | `lourd cargo build --release -p salle --example soiree_en_memoire` | code 0 |
| `memoire-devine_module`, `-punchline_module`, `-sidequest_module`, `-tier_list_module` | `shasum -a 256 <module>`, puis `lourd /usr/bin/time -l target/release/examples/soiree_en_memoire <module> --duree 120 --graines 1,2,3` | **code 0 pour chacun, trois graines réussies chacun** (voir ci-dessous) |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 582 réussis** (les 548 du lot 4a et 34 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 305 s ; 1,65 Go au plus ; charge 3,75 au départ, 2,63 à la fin |

**Les soirées en mémoire des trois jeux et de `devine`**, 120 s simulées par graine, graines 1, 2 et 3, huit téléphones (aucun pour Tier list, qui n'en déclare pas : l'écran seul joue, par son bot), pannes de `Pannes::SOIREE`, une fin calme de 7 s : à chaque graine, l'autorité ne s'est jamais arrêtée ; la chaîne entière des cases de la soirée (de 1 168 à 1 239 cases, 13 points) s'ouvre case après case et porte un journal d'un seul tenant ; aucun geste n'y est deux fois ; chaque geste accusé y est ; plus rien n'attend d'accusé à la fin ; la garde finale reprend une partie de même forme canonique et de même empreinte que l'autorité vivante.

| Module (sha256) | Redémarrages de l'autorité | Coupures, rechargements, jumeaux, poignées rejouées (sur les trois graines) | Gestes émis = accusés = écrits | Mémoire |
| --- | --- | --- | --- | --- |
| `devine` (`c6cbe84d…`) | 10, 6, 8 | 19, 16, 9, 8 | 55, 56, 55 | 0,07 Go |
| Punchline (`d3f4ba08…`) | 7, 8, 6 | 13, 15, 7, 8 | 422, 516, 576 | 0,09 Go |
| Sidequest (`e2ebb6a7…`) | 5, 6, 7 | 19, 19, 9, 9 | 748, 710, 735 | 0,08 Go |
| Tier list (`1efe48d6…`) | 4, 8, 5 | aucune (pas de téléphone) | aucun ; de 7 887 à 8 303 lignes de journal, de 19 à 22 demandes refusées par la page du banc | 0,13 Go |

Les gestes placés par les autorités successives dépassent parfois les gestes écrits (Punchline, graine 3 : 578 placés, 576 écrits ; Sidequest, graines 2 et 3) : un geste placé par une autorité morte avant de l'écrire est placé une seconde fois par la suivante, et compte une fois. C'est ce que le SAL-57 demande.

### Les mesures

Aucune mesure nommée (M1 à M15) n'est de ce lot. La mémoire de ses commandes, prise comme le § 7 le demande : `cargo test -p salle` 0,28 Go au plus ; la compilation de `salle` pour `wasm32`, 0,31 Go ; une soirée en mémoire de Tier list, 0,13 Go ; `cargo test --workspace` 1,65 Go. Estimé au § 7 : « 2 à 3 Go ». Les durées ci-dessus sont celles de commandes sous une charge de 1,4 à 4, pas des mesures ; celle d'une soirée en mémoire (quelques secondes pour trois soirées de 120 s) n'en est pas une non plus.

### Ce qui n'est pas fait, et pourquoi

- **L'envoi du module aux téléphones** (P18 : morceaux compressés, fenêtre de deux, `ModuleRecu`, `Encombre`) et **les ressources de la page** (`VersAppareil::Ressource`) : les téléphones n'en ont l'usage qu'avec la page (lots 10a et 12). L'autorité ignore `ModuleRecu`, l'appareil ignore `Module` et `Ressource`.
- Ni `VersPage::Traces` (lot 13), ni `VersPage::EnCours` et l'entrée écartée après deux arrêts muraux (P39, lot 10a), ni `VersAppareil::Ecarte` envoyé par l'autorité, ni les commandes `ChangerDeJeu`, `NouvelleSoiree`, `Marque`, ni `DeLaPage::Fichiers` (lots 10a, 10b, 11). `Reprendre` appelle `Partie::repartir`.
- **La relâche des tenus côté appareil** (P28 : coupure, arrière-plan) et le geste tenu quand la page change (REM-23) : ils demandent la page et `Tenu::relache` (lot 12). Le tenu d'un appareil natif se redit à sa période sans jamais se relâcher de lui-même.
- **Une borne de taille pour les cases de la garde** : le relais en mémoire n'en a pas, et une case de point de Tier list au pire dépasse `TRAME_MAX` (écart 3) ; au lot 5.
- `Retirer` : l'autorité tient l'appareil pour à annoncer de nouveau, sans le couper ; ce que le commun doit dire à un téléphone retiré n'est écrit nulle part (à trancher avec la page, lot 12). Un appareil oublié revient sous le même `Appareil`, par une arrivée.
- Aucune soirée sur le vrai relais ni sur de vraies WebSocket : lot 5 (`soiree-banc soiree-native`).
- Le plan standard n'est pas joué : ce lot ne change ni ce que l'épreuve joue ni ce qu'elle juge (§ 7, « Le plan court ou le plan standard »), et le plan court des trois jeux rend la référence. E20 n'est pas relancé : aucun module ni aucune réécriture ne change.

### Ce que le lot suivant doit savoir

- Le relais réel doit faire ce que fait `RelaisEnMemoire` (PHASE-3, § 2.5, « Ce que l'autorité et les appareils attendent du relais ») : sinon un appareil ne commence pas sa poignée de main, et une autorité relancée ne retrouve pas les téléphones. Les tests de la salle tournent sur ce modèle ; la soirée native du lot 5 doit passer par le vrai relais avec les mêmes `Autorite` et `Appareil`, et les mêmes jugements (`verifier_l_historique`, `comparer_a_la_garde`).
- La coque garde, à côté de G, la clé privée et le secret, le dernier numéro de case écrit (`Autorite::dernier_ecrit`) et le passe à `Autorite::demarrer` : une garde plus vieille est refusée, une garde vide alors qu'il est connu aussi.
- La garde n'a pas de borne de taille (écart 3) : `VersRelais::Ecrire` d'un point de Tier list peut faire 16,8 Mio.
- `Pannes::garde_us` (1 à 20 ms) tient lieu de M11 dans la simulation ; à remplacer par la mesure du lot 5.
- Tier list : son `Cargo.lock` a gagné `liaison` à la compilation de l'épreuve (annoncé au lot 4a, CHANGELOG) ; le changement est dans son dépôt, à committer avec ce lot ou à remettre.

## Lot 4b, correction après la vérification

Fait le 2026-11-06, de 20 h 15 à 21 h environ (GMT), sur la même machine. Chaque commande cargo lourde est passée par le portillon. Aucun navigateur, aucun serveur. Rien n'est commité. Le code des jeux n'est pas touché ; le `Cargo.lock` de Tier list reste modifié comme au lot 4b (`liaison`, diff `53a1f747…`), non commité.

### Les constats, et ce qui en est fait

1. **[bloquant] `un_geste_n_est_accuse_qu_une_fois_ecrit_sal56` ne voyait pas un accusé envoyé dès l'`Ecrire`** : vérifié, juste. `RelaisEnMemoire::recevoir` rangeait la case dans la garde et l'historique dès l'`Ecrire`, et seule la réponse `Ecrit` attendait `garde_us` ; le test jugeait donc « accusé ⇒ case envoyée ». **Corrigé dans `salle::simulation`** : la latence de la garde porte désormais sur ce que la garde envoie au relais (`Soiree::latence`, `(Bout::Garde, Sens::VersRelais)`) ; le relais traite chaque demande de la garde, dans l'ordre, au bout du temps d'écrire, écrit la case et répond `Ecrit` au même instant. Une case n'est dans la garde et l'historique qu'à l'instant de son `Ecrit`. Pour ne pas perdre le cas « case écrite, `Ecrit` jamais reçu » que l'ancien modèle donnait quand l'autorité mourait pendant la réponse : ce qu'une autorité tuée avait déjà donné à sa garde s'écrit jusqu'au bout, et la garde ne se ferme qu'après (une trame vide en fin de file, `Soiree::tuer_l_autorite`) ; une autorité plus récente qui se présente avant refuse toujours l'ancienne garde (SAL-20). Le test compte en plus les instants où une case s'écrit pendant que le téléphone attend un accusé (`Soiree::cases_en_ecriture`), et exige d'en voir plus de 20 : la fenêtre où un accusé trop tôt se verrait est bien parcourue. Le contrôle « accuser dès l'`Ecrire` » (celui du vérificateur, `self.case_ecrite(case)` juste après le `vers_garde(&VersRelais::Ecrire {..})`) **rougit** maintenant : « le geste 1 est accusé avant d'être écrit ». Le comportement de l'autorité n'a pas changé.
2. **[mineur] « un message avant le bonjour coupe son seul lien » n'avait pas de test** : vérifié, juste. Nouveau test **`un_message_avant_le_bonjour_coupe_son_seul_lien_sal117`** (`tests/salle.rs`) : sur quatre liens dont la poignée de main est faite (par `chiffre::Initiateur`, avec le fragment du QR), un premier message qui n'est pas le bonjour (geste, tenu, écho, morceau reçu) fait envoyer `Couper` pour ce lien et lui seul ; un bonjour scellé ensuite sur le même lien ne fait plus rien sortir ; aucun geste placé, aucun appareil de plus au registre, aucun remplacement, aucun tenu de plus ; le téléphone honnête reste accueilli, ses présences inchangées. Le contrôle du vérificateur (« rien avant le bonjour » qui ne coupe plus) le fait rougir.
3. **[mineur] Le contrôle négatif du § 8.1 « le jeton dans la poignée de main » manquait** : vérifié, juste. Ajouté (`jeton-dans-la-poignee`) : l'autorité retient chaque premier message de poignée de main qu'elle a vu ; une poignée déjà vue désigne alors son téléphone (comme si le jeton y était), et le lien neuf le remplace aussitôt (`Refus(Remplace)` à l'ancien), avant tout bonjour chiffré. `une_poignee_de_main_rejouee_ne_remplace_personne_sal17` rougit (le vrai téléphone n'est plus accueilli sans refus).
4. **[mineur] `un_maintenu_dont_la_cle_disparait_est_relache_rem23` reporté sans écart ailleurs** : vérifié. Le report se défend (il demande les clés d'une `PageFil`, § 2.10), mais la ligne du lot 12 au § 7 de PHASE-3 ne le portait pas : elle le porte désormais dans sa colonne « Ce qui le prouve fini », et la ligne du lot 4b dit qu'il y est reporté.
5. **[mineur] L'exemple sautait en silence un appareil introuvable ; `devine.rs` ne vérifiait pas « tout geste accusé est écrit »** : vérifié, juste. Le jugement est désormais une seule fonction, `Soiree::accuses_non_ecrits` : tout geste accusé absent de la chaîne écrite est une faute, et un téléphone à qui un geste est accusé alors que le registre ne connaît pas son appareil en est une aussi (un téléphone à qui rien n'est accusé n'a rien à vérifier). L'exemple et `tests/devine.rs` s'en servent ; `tests/soiree.rs` garde son `expect` sur chaque téléphone, plus strict.
6. **[mineur] Tier list : rien ne comptait les coups du bot de l'écran** : vérifié, juste. `Comptes::coups_ecran` (et `ComptesSomme::coups_ecran`) compte les coups du bot de l'écran entrés dans la partie en mode banc ; l'exemple l'imprime, et échoue si personne n'a joué (ni geste d'un téléphone, ni coup de l'écran). Tier list : de 535 à 561 coups de l'écran par soirée. Sans téléphone, la soirée de Tier list n'a toujours pas d'autre panne que l'autorité tuée et relancée : le commentaire de l'exemple le dit.
7. **[mineur] « Les jeux ne sont pas touchés » alors que le `Cargo.lock` de Tier list a changé** : vérifié, juste. La phrase de la section du lot 4b est corrigée ; le changement (gagné en compilant l'épreuve, annoncé au lot 4a) reste non commité dans le dépôt de Tier list, à committer avec ce lot ou à remettre. Je ne l'ai pas remis : la prochaine compilation de l'épreuve sur Tier list le referait.

La phrase de la section du lot 4b sur le test du SAL-56 (« le relais met de 5 à 30 ms à écrire une case ») est corrigée aussi.

### Les tests qui le tiennent

`crates/salle` : 65 tests (64 + `un_message_avant_le_bonjour_coupe_son_seul_lien_sal117`), dont `un_geste_n_est_accuse_qu_une_fois_ecrit_sal56` renforcé, `tests/devine.rs` qui vérifie désormais chaque accusé.

**Les contrôles négatifs**, refaits tous sur le code final par `target/travail/lot-4b/correction/controles.py` (copié dans la preuve) : les onze du lot 4b et trois nouveaux, **quatorze rouges**, chacun code 101 avec `FAILED` et `panicked` sur son test (et non une erreur de compilation), chaque fichier remis à l'identique (sha256 vérifié par le script, et les sources de `salle` identiques avant et après : `avant-controles.txt`, `apres-controles.txt`).

| Changement nouveau | Rougit |
| --- | --- |
| l'accusé dès l'envoi de l'`Ecrire`, avant l'`Ecrit` (§ 8.1) | `un_geste_n_est_accuse_qu_une_fois_ecrit_sal56` |
| un message avant le bonjour qui ne coupe pas son lien | `un_message_avant_le_bonjour_coupe_son_seul_lien_sal117` |
| le jeton dans la poignée de main : une poignée déjà vue remplace le lien de son téléphone (§ 8.1) | `une_poignee_de_main_rejouee_ne_remplace_personne_sal17` |

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-4b/correction/`, chaque dossier comme au lot 4b (`commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt`). Le commun est à `dceea7a` ; tout le code changé est dans des fichiers non suivis, qui valent `8c48e83e…` dans chaque preuve (`git diff HEAD` hors `.md` : `31e783cf…`, inchangé) ; Punchline et Sidequest propres ; Tier list `53a1f747…` (son `Cargo.lock`). Seuls des `.md` ont changé après les preuves.

| Preuve | Commande | Résultat |
| --- | --- | --- |
| `fmt` | `cargo fmt --all --check` | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `controles-negatifs` | `python3 -I target/travail/lot-4b/correction/controles.py` | les quatorze rouges ; un sous-dossier par contrôle |
| `test-salle` | `lourd /usr/bin/time -l cargo test -p salle` | code 0 : 65 réussis ; 0,27 Go au plus ; 15 s |
| `exemple-release` | `lourd cargo build --release -p salle --example soiree_en_memoire` | code 0 |
| `memoire-devine_module`, `-punchline_module`, `-sidequest_module`, `-tier_list_module` | `shasum -a 256 <module>`, puis `lourd /usr/bin/time -l target/release/examples/soiree_en_memoire <module> --duree 120 --graines 1,2,3` | **code 0 pour chacun, trois graines réussies chacun** ; mêmes modules qu'au lot 4b (sha256 `c6cbe84d…`, `d3f4ba08…`, `e2ebb6a7…`, `1efe48d6…`) |
| `getrandom` | `cargo tree -p salle -i getrandom --target wasm32-unknown-unknown -e normal,build`, puis `cargo tree -p epreuve -e normal,build`, compté sur ` salle ` | rien pour `salle` ; `epreuve` ne dépend pas de `salle` (0 ligne) |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 583 réussis** (les 582 du lot 4b et le nouveau test du SAL-117), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 276 s ; 1,92 Go au plus ; charge 2,72 au départ, 3,53 à la fin |

**Les soirées en mémoire**, sur le relais qui n'écrit qu'au bout du temps d'écrire (120 s, graines 1, 2, 3, pannes de `Pannes::SOIREE`) :

| Module | Redémarrages | Coupures, rechargements, jumeaux, poignées rejouées (sur les trois graines) | Gestes émis ; écrits | Coups du bot de l'écran | Mémoire (résidente, au plus) |
| --- | --- | --- | --- | --- | --- |
| `devine` | 6, 6, 12 | 16, 13, 10, 7 | 56, 56, 55 ; les mêmes | 6, 6, 6 | 0,07 Go |
| Punchline | 7, 5, 7 | 21, 20, 8, 12 | 472, 516, 556 ; 472, 516, 556 (graine 2 : 515 accusés) | 109, 109, 112 | 0,09 Go |
| Sidequest | 5, 7, 10 | 27, 12, 10, 6 | 759, 702, 711 ; 757, 702, 711 (graine 2 : 701 accusés) | 36, 37, 34 | 0,08 Go |
| Tier list | 3, 8, 7 | aucune (pas de téléphone) | aucun | 561, 535, 557 | 0,14 Go |

Un geste émis et jamais écrit (Sidequest, graine 1 : deux) ou écrit et jamais accusé (Punchline et Sidequest, graine 2 : un) est celui d'un onglet jumeau fermé, qui emporte sa file et ne reçoit plus d'accusé ; l'exemple ne demande « plus rien en file » qu'aux téléphones qui ne sont pas des jumeaux, comme `tests/soiree.rs`. Tout geste accusé est écrit, dans chaque soirée.

**Le plan court n'est pas relancé** : ni les modules, ni ce dont `soiree-epreuve` dépend n'ont changé depuis les preuves `court-*` du lot 4b (`epreuve` ne dépend pas de `salle`, et `partie` n'a pas changé) ; on relit ces rapports (la référence du § 7).

### Les mesures

Aucune. La mémoire des commandes est prise par `/usr/bin/time -l` ; les durées sont celles de commandes sous une charge de 2 à 4, pas des mesures.

### Ce qui n'est pas fait, et pourquoi

- Le reste de « Ce qui n'est pas fait » du lot 4b tient tel quel.
- Une soirée de Tier list n'a pas de panne de téléphone : le jeu n'en a pas. Une soirée avec des téléphones ajoutés de force n'aurait pas de sens pour ce jeu.

### Ce que le lot suivant doit savoir

- Le relais réel doit répondre `Ecrit` seulement une fois la case écrite (§ 2.5, déjà dit) ; la simulation le fait maintenant à la lettre, et le test du SAL-56 le voit.
- Une écriture que la garde d'une autorité tuée avait déjà donnée au relais s'y achève dans la simulation : la nouvelle autorité peut trouver une garde plus récente que le dernier numéro que la coque sait écrit (accepté, seule une garde plus vieille est refusée). Le relais du lot 5 aura le même cas.
- `Soiree::accuses_non_ecrits` est le jugement « tout geste accusé est écrit » : la soirée native du lot 5 doit s'en servir.
- `un_maintenu_dont_la_cle_disparait_est_relache_rem23` est dans la colonne du lot 12.

## Lot 5 — Le relais et la soirée native

Fait le 2026-11-06, de 20 h 50 à 22 h 05 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo lourde, chaque soirée native et chaque mesure sont passées par le portillon (`target/hote/lourd`, `--calme` pour M11). Aucun navigateur. Chaque relais, mandataire et banc lancé est arrêté (aucun processus `soiree-relais` ni `soiree-banc` ne reste). Rien n'est commité. Le code des jeux n'est pas touché ; le `Cargo.lock` de Tier list reste modifié comme au lot 4b (`53a1f747…`), non commité.

### Ce qui est fait

**`crates/relais`** (nouvelle, sans `unsafe`), et le binaire **`soiree-relais`** :
- **`coeur`**, sans entrées-sorties, le temps en paramètre (µs depuis le lancement), les preuves par un trait (`Preuves` ; `PreuvesEnMemoire` pour les tests, `garde::PreuvesSurDisque` pour la coque, lues à chaque bonjour d'autorité, pour que `oublier` se voie sur un relais qui tourne). Les rôles (`Autorite`, `Garde`, `Appareil`) ; **une seule autorité par salle**, la plus récente dans l'ordre d'ouverture des liens : l'ancienne reçoit `Refus(Remplacee)` sur ses deux liens, un bonjour d'autorité d'un lien plus ancien est refusé de même, une garde qui nomme une autorité passée aussi, et rien d'une garde passée ne s'écrit (SAL-18, SAL-19, SAL-20) ; ce que fait `RelaisEnMemoire` (PHASE-3, § 2.5) : `AutoritePresente`/`AutoriteAbsente` après l'accueil d'un appareil, `Ouvert` et `AutoritePresente` pour chaque appareil à l'autorité qui se présente, `AutoriteAbsente` à chacun quand elle part (SAL-23), `Ferme` à l'autorité, `Ecrit` une fois la case écrite ; une salle dont la preuve est sur disque est connue (un relais relancé reprend les téléphones avant l'autorité) ; les refus typés, chacun suivi de la fermeture du lien (SAL-42, SAL-148), `Protocole` compris pour un bonjour cassé d'un autre protocole (SAL-40) ; `Limites` ; un lien sans bonjour en 10 s ou muet depuis `SILENCE_MAX_MS` fermé, un battement par lien toutes les 250 ms (SAL-43, SAL-49) ; un message illisible ou trop gros ne coûte que son lien ; `Echo` à l'heure du relais.
- **`file`** : la file d'un lien, fiable bornée en octets (pleine, elle le dit : SAL-06, SAL-07), une place par `(lien d'origine, canal)` remplaçable (SAL-70), le battement, la fin.
- **`http`** : la requête par `httparse` ; qui parle (SAL-33 : boucle locale, `Host` local, `Origin` de l'écran, ou absente avec `--banc` ; une page du relais n'est qu'un appareil ; le réseau, par une adresse annoncée ; aucun en-tête de mandataire ; deux `Origin` ou deux `Host` étrangers) ; les routes, comparées à des noms, jamais au disque (SAL-150) ; les en-têtes (CSP, COOP et COEP sur l'écran, `no-store`) ; la réponse au WebSocket.
- **`admission`** : décidée sur le fil d'écoute, avant tout fil (SAL-143), 256 connexions dont 16 réservées à la boucle, le réseau limité à 64 par seconde, rafale de 128 (SAL-78).
- **`adresses`** : les interfaces (`if-addrs`), chaque adresse joignable annoncée, IPv4 d'abord (SAL-76) ; `--adresse` portée par une interface, sinon un refus qui dit pourquoi (SAL-77) ; l'adresse jointe lue sur chaque connexion (SAL-75).
- **`garde`** : la garde sur disque (`<dossier>/<SALLE>/cases/<n>-p|j`, `preuve`, `salle`), écrite par `ecrire_durable` (fichier temporaire, `sync_all`, renommage, dossier synchronisé : SAL-26, SAL-27), un temporaire resté jeté, `GARDE_MAX` (256 Mio, `GardePleine`), `oublier`.
- **`enregistrement`** (`--enregistrer`) : chaque trame reçue et envoyée, d'un seul `write` (un `kill -9` n'en perd aucune écrite).
- **`coque`** : la boucle locale dans les deux familles au même port (un port pris dans l'une empêche le démarrage), chaque adresse annoncée écoutée ; deux fils par lien (la lecture par `tungstenite`, ses réponses automatiques sous le verrou de l'écrivain ; l'écrivain forme ses trames entières) ; un fil pour les salles, un pour la garde, dans l'ordre ; `catch_unwind` par lien (SAL-79) ; la garde a sa propre borne de lecture (`coeur::CASE_MAX`, 64 Mio) une fois le lien admis ; un lien fermé qui ne part pas est coupé en 2 s.
- Le binaire : `soiree-relais [--port 7878] [--adresse <ip> | --sans-reseau] [--garde <dossier>] [--banc] [--enregistrer <fichier>]`, `soiree-relais oublier <SALLE>`. L'exemple `mesure_garde` (M11).

**Dans `crates/banc`** : `soiree_native` (le scénario `soiree-banc soiree-native`), `mandataire` (le mandataire TCP de pannes : latence par sens, rafales, retenue, coupure, fermeture lente, fin derrière les données), `ws` (un client WebSocket non bloquant). La soirée : le relais (le binaire en sous-processus, `--sans-reseau --banc --enregistrer`, ou `relais::coque` dans le processus pour les tests), l'autorité native (`salle::Autorite` sur wasmtime, page du banc qui refuse les demandes), des téléphones natifs (`salle::Appareil`, les coups du banc) derrière le mandataire ; les pannes de la graine (téléphone coupé, retenu, rechargé, en rafales, fermé lentement ; onglet jumeau ; poignée de main rejouée par un invité ; « Reprendre ici » ; autorité tuée et relancée depuis la garde) et `--tuer-le-relais N` (`kill -9`, puis relance sur le même port et la même garde) ; la fin calme ; les jugements : aucune autorité arrêtée, la chaîne entière relue dans les enregistrements successifs du relais (une case est écrite quand le relais l'accuse ou la lit sur son disque, la garde finale a le dernier mot) et vérifiée par `verifier_l_historique`, aucun geste deux fois, aucune entrée écartée, tout geste accusé écrit (`accuses_hors_de_l_historique`), plus rien en file, la garde finale relue sur le disque du relais qui reprend la même forme canonique et la même empreinte (`reprendre_de_la_garde`), une phase terminale vue si `--partie-entiere`, quelqu'un a joué, et **aucun secret** dans ce que le relais a vu ni dans sa garde (le secret d'entrée, la psk, G, la clé privée, chaque jeton, chaque geste d'au moins 8 octets, jusqu'à 400 vues d'au moins 16 octets échantillonnées sur les téléphones). M15 lu sur le relais toutes les 5 s.

**Dans `crates/salle`**, `simulation` seulement, sans rien changer de ce que la salle fait : `accuses_hors_de_l_historique`, `TelephoneAccuse`, `reprendre_de_la_garde`, `forme_et_empreinte` et `reponse_par_defaut` publics (`Soiree::accuses_non_ecrits` et `comparer_a_la_garde` s'en servent) ; `Historique` compte `ecartees`, `tenus`, `sauts` et garde `octets_des_gestes`.

Documents : PHASE-3 (§ 2.2, § 2.4, § 2.5, ligne du lot 5 au § 7, M11 et M15 au § 8.6, annexes), MESURES-PHASE-3 (« Lot 5 »), CHANGELOG, `crates/README.md`, `AGENTS.md`, le commentaire de tête de `Cargo.toml`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **Le test du graphe permet `chacha20` par `rand`.** `tungstenite` 0.30 tire `rand` sans option, et `rand` 0.10 tire `chacha20` 0.10 comme générateur. Le test refuse toute suite de chiffrement des trames (`snow`, `chacha20poly1305`, `poly1305`, `aead`, `curve25519-dalek`…), `salle`, et toute crate du commun autre que `liaison` et `jeu-abi`, et n'admet `chacha20` que par `rand` (§ 2.5). Écrire à la main le WebSocket du relais l'aurait évité, au prix d'un code tiers recopié ; ce n'est pas fait.
2. **La garde a sa propre borne, et ne découpe pas un point** (§ 2.4, la question laissée par le lot 4b) : 64 Mio par case sur le seul lien admis comme garde, 256 Mio par salle.
3. **Le battement du relais est le `Ping` de WebSocket** (§ 2.5) : aucun message de `liaison` ne s'ajoute ; toute trame reçue, `Pong` compris, dit le lien vivant.
4. **`--sans-reseau`**, une option du binaire que PHASE-3 ne nommait pas : la boucle seulement ; le banc s'en sert pour ne rien ouvrir sur le Wi-Fi (et ne pas faire demander le pare-feu, SAL-95).
5. **Les contrôles négatifs du § 8.1** (« une option de test qui désactive le chiffrement », « une option qui ne vérifie pas l'`Origin` ») sont des changements du code faits par `target/travail/lot-5/controles.py`, comme aux lots 4a et 4b, et non des options dans le code livré.
6. **`le_relais_ne_voit_aucun_secret` est dans `banc/tests/`**, pas dans `relais/tests/` : il demande la salle et un module, que le relais ne doit pas tirer, même en développement (le test du graphe ne regarde que les dépendances livrées, mais une dépendance de développement à `salle` brouillerait la forme).
7. Les routes `GET /web/…` (la table du paquet, lot 10a), `GET /jeux/…` (lot 10a) et `GET /banc`, `POST /banc/rapport` répondent `404` : le lot 5 n'en a pas l'usage ; `--banc` n'admet aujourd'hui que les outils natifs sans `Origin`. La page servie est minimale (lot 10b). `AjouterLigne` est lu et ignoré (lot 11).

### Les tests qui le tiennent

Dans `crates/relais` (41 tests) : `src/file.rs` (3, dont `une_file_fiable_pleine_refuse_sans_rien_jeter_sal06_sal07`, `la_derniere_trame_d_une_place_remplace_celle_qui_attend_sal70`) ; `tests/relais/`, un seul binaire :
- `coeur` (13, horloge simulée) : `l_autorite_et_les_appareils_se_trouvent_comme_dans_le_relais_en_memoire`, **`une_seconde_autorite_remplace_la_premiere_sal18`**, `un_bonjour_d_autorite_coince_sur_un_lien_plus_ancien_ne_remplace_personne_sal19`, **`un_ecrire_d_une_autorite_remplacee_est_refuse_sal20`**, `quand_l_autorite_part_chaque_appareil_l_apprend_sal23`, `chaque_refus_est_type_et_ferme_son_lien_sal42_sal148`, `un_message_illisible_ou_trop_gros_ne_coute_que_son_lien`, `le_silence_se_voit_et_chaque_lien_bat_sal43`, `un_lien_sans_bonjour_se_ferme_en_dix_secondes_sal49`, `l_adresse_jointe_par_chaque_appareil_va_a_l_autorite_sal75`, `l_echo_rend_l_instant_et_l_heure_du_relais`, `un_relais_relance_reprend_les_telephones_avant_l_autorite`, et sous proptest (2 048 cas) `des_octets_au_hasard_ne_font_jamais_paniquer_le_coeur_rem64` ;
- `coque` (8, de vraies connexions) : `un_port_pris_dans_une_famille_empeche_le_demarrage`, `sans_reseau_le_relais_demarre_en_boucle_locale_sal76`, `une_adresse_qu_on_n_ecoute_pas_se_refuse_a_voix_haute_sal77`, `l_adresse_jointe_est_celle_de_la_connexion_sal75` (IPv4 et IPv6), `un_envoi_n_attend_pas_une_lecture_sal146`, `une_file_pleine_ferme_son_lien_sal06_sal07` (et `Encombre` avant), `un_refus_ferme_sa_connexion_sal148`, `le_silence_ferme_un_lien_qui_ne_rend_pas_ses_battements_sal43` ;
- `http` (7) : `qui_parle_au_relais_se_juge_sur_l_adresse_le_host_et_l_origin_sal33`, **`un_site_etranger_ne_prend_pas_la_salle_sal33`**, **`un_rebond_dns_est_refuse_sal33`**, `aucun_fichier_hors_de_la_table_n_est_servi_sal150`, `la_boucle_locale_garde_ses_places_sal143`, `les_adresses_annoncees_sont_celles_des_interfaces_sal76`, `une_adresse_imposee_doit_etre_celle_d_une_interface_sal77` ;
- `garde` (8) : `le_nom_d_une_case_se_lit_et_garde_l_ordre_des_numeros`, `une_case_s_ecrit_durablement_se_relit_et_s_efface`, `une_garde_pleine_refuse_l_ecriture_sans_rien_jeter_sal07`, `ecrit_ne_part_qu_une_fois_la_case_sur_disque_sal26`, `une_case_plus_grande_qu_une_trame_passe_par_la_garde`, `un_relais_relance_garde_sa_salle_sa_preuve_et_sa_garde`, `oublier_efface_la_preuve_et_la_garde_et_garde_le_code_p41`, `l_enregistrement_note_tout_ce_que_le_relais_voit` ;
- `graphe` (2) : `le_relais_ne_tire_ni_la_salle_ni_le_chiffrement`, et son témoin, `le_temoin_qui_tire_snow_fait_echouer_le_test_du_graphe` (`tests/temoins/tire-snow/`, lu dans une copie sous `target/`, hors ligne).

Dans `crates/banc` : `tests/soiree_native.rs`, **`le_relais_ne_voit_aucun_secret`** (une soirée de `devine` sous pannes, 3 téléphones, 15 s, par le relais dans le processus ; tout le reste de la soirée doit tenir aussi).

Les premiers passages des tests du relais en ont trouvé deux instables (`un_relais_relance…` et `oublier…`, rouges un passage sur deux ou trois) : le fil de lecture, quand le fil des salles refusait le bonjour, coupait aussitôt la connexion et jetait le refus encore en file. Corrigé (la lecture finit d'elle-même quand l'écrivain ferme) ; cinq passages de suite verts ensuite.

**Les contrôles négatifs**, par `target/travail/lot-5/controles.py` (le changement, le test qui doit rougir, rouge seulement sur `FAILED` et `panicked` avec le code 101, chaque fichier remis à l'identique, sha256 vérifié) ; **dix-sept rouges** :

| Changement | Rougit |
|---|---|
| le lien d'autorité le plus ancien gagne | `un_bonjour_d_autorite_coince…_sal19` |
| l'ancienne autorité n'est pas refusée | `une_seconde_autorite_remplace_la_premiere_sal18` |
| une garde qui nomme une autorité passée est admise | `un_ecrire_d_une_autorite_remplacee_est_refuse_sal20` |
| aucun `AutoriteAbsente` | `quand_l_autorite_part…_sal23` |
| un refus qui ne ferme pas son lien | `un_refus_ferme_sa_connexion_sal148` (le premier essai restait vert : le test attendait 5 s, et le silence, 3 s, fermait le lien ; il demande désormais la fermeture en 1,5 s) |
| le silence mille fois plus long | `le_silence_ferme_un_lien…_sal43` |
| toute `Origin` admise | `un_site_etranger_ne_prend_pas_la_salle_sal33` |
| le `Host` non vérifié | `un_rebond_dns_est_refuse_sal33` |
| l'adresse jointe fixée à la boucle | `l_adresse_jointe_est_celle_de_la_connexion_sal75` |
| les adresses de lien local annoncées | `les_adresses_annoncees_sont_celles_des_interfaces_sal76` |
| une `--adresse` d'aucune interface acceptée | `une_adresse_imposee_doit_etre_celle_d_une_interface_sal77` |
| aucune réserve pour la boucle | `la_boucle_locale_garde_ses_places_sal143` |
| une file fiable sans borne | `une_file_pleine_ferme_son_lien_sal06_sal07` |
| l'écrivain qui attend 2,5 s avant chaque trame | `un_envoi_n_attend_pas_une_lecture_sal146` |
| `/web/<chemin>` servi du disque | `aucun_fichier_hors_de_la_table_n_est_servi_sal150` |
| `Ecrit` sans écrire | `ecrit_ne_part_qu_une_fois_la_case_sur_disque_sal26` |
| le chiffrement des trames désactivé (le clair à la place du chiffré, des deux côtés) | `le_relais_ne_voit_aucun_secret` (les jetons vus en clair) |

Le témoin du graphe est le contrôle négatif de `le_relais_ne_tire_ni_la_salle_ni_le_chiffrement`.

### Les soirées natives

| Soirée | Module (sha256) | Pannes (graine) | Résultat |
|---|---|---|---|
| Punchline, 8 téléphones, 180 s, `--tuer-le-relais 2 --partie-entiere`, graine 1 | `d3f4ba08…` (blake3 `ad1b7b04…`) | 8 autorités tuées, 2 relais tués (`kill -9`), 10 redémarrages de l'autorité ; 5 coupures, 5 retenues, 8 rechargements, 3 rafales, 2 fermetures lentes, 3 jumeaux, 1 poignée rejouée, 3 « Reprendre ici » | **code 0** : 851 gestes émis, 851 écrits, aucun deux fois, 850 accusés (le dernier, d'un jumeau parti) ; 472 tenus ; **4 phases terminales** ; 1 554 cases, 18 points, 88 082 lignes, aucune entrée écartée, 105 sauts (les reprises) ; la garde finale reprend la même partie ; aucun secret parmi 575 aiguilles dans 39,4 Mo vus ; 0,08 Go résidents (le banc) |
| Sidequest, 71 téléphones, 600 s, `--tuer-le-relais 2`, graine 2 | `e2ebb6a7…` (blake3 `8112385d…`) | 25 autorités tuées, 2 relais tués, 31 redémarrages ; 8 coupures, 23 retenues, 31 rechargements, 12 rafales, 14 fermetures lentes, 16 jumeaux, 11 poignées rejouées, 11 « Reprendre ici » | **code 0** : 30 674 gestes émis, accusés et écrits, aucun deux fois ; **296 236 tenus** écrits ; 23 phases terminales ; 5 014 cases, 60 points, 2 415 532 lignes, aucune entrée écartée, 133 sauts ; la garde finale reprend la même partie ; aucun secret parmi 562 aiguilles dans 306 Mo vus ; M15 : 153 fils, 14,4 Mio ; 0,20 Go résidents (le banc) |

L'autorité native a joué 32 110 images en 600 s à 71 téléphones (53 par seconde) : la boucle du banc, ses 71 téléphones et son mandataire sur la même machine ne tiennent pas 60 images par seconde ; la partie suit pourtant l'horloge (elle joue plusieurs pas par réveil) ; ce n'est pas M13. Les 133 sauts viennent des reprises (le temps d'un redémarrage devient un saut, D14).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-5/` (hors dépôt) ; chaque dossier a `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (pour le commun et chaque jeu : `HEAD`, `sha256` de `git diff HEAD`, des fichiers non suivis, et de `git diff HEAD` hors des `.md`). Le commun est à `11c65c8` ; les jeux à leur `HEAD` (Punchline `dd6d4f0`, Sidequest `6e7219c`, Tier list `eebc5c1`, son `Cargo.lock` modifié, `53a1f747…`). Les preuves finales (`fmt` à `test-workspace`) ont toutes le même code : non suivis `d56d98c1…`, `diff HEAD` hors `.md` `09f54b26…`. Les soirées natives et M11 ont été prises avant deux changements qui ne touchent aucun binaire : un commentaire du `Cargo.toml` du commun (non suivis `d19b1a01…`, `diff HEAD` hors `.md` `5f9a3ddc…`, devenu `09f54b26…` pendant la soirée de Sidequest), puis `cargo fmt` de l'exemple `mesure_garde` (devenu `d56d98c1…`) ; aucune crate que les soirées lancent n'a changé.

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt` | `cargo fmt --all --check` | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `graphe` | `cargo tree -p relais -e normal,build` ; `cargo tree -p epreuve -e normal,build`, compté sur `salle`, `relais`, `banc` ; `cargo tree -p salle -i getrandom --target wasm32-unknown-unknown -e normal,build` | le relais : `liaison`, `jeu-abi`, `tungstenite`, `httparse`, `if-addrs` et leurs dépendances ; `epreuve` ne tire ni `salle`, ni `relais`, ni `banc` (0 ligne) ; `salle` ne tire pas `getrandom` pour `wasm32` |
| `controles-negatifs` | `python3 -I target/travail/lot-5/controles.py` | les dix-sept rouges ; un sous-dossier par contrôle |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 625 réussis** (les 583 du lot 4b, 41 du relais, `le_relais_ne_voit_aucun_secret`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 329 s ; 1,77 Go résidents au plus ; charge 2,40 au départ, 3,64 à la fin |
| `soiree-native-punchline` | `lourd /usr/bin/time -l target/release/soiree-banc soiree-native …/punchline_module.wasm --telephones 8 --duree 180 --graine 1 --tuer-le-relais 2 --partie-entiere` | code 0 (ci-dessus) ; 185 s |
| `soiree-native-sidequest-71` | `lourd /usr/bin/time -l target/release/soiree-banc soiree-native …/sidequest_module.wasm --telephones 71 --duree 600 --graine 2 --tuer-le-relais 2 --calme 120` | code 0 (ci-dessus) ; 606 s |
| `m11` | `lourd --calme target/release/examples/mesure_garde …`, puis `lourd --calme python3 -I target/travail/lot-5/fsync.py …` | MESURES-PHASE-3, « Lot 5 » |

Non lancés, parce que ce lot ne change ni ce que l'épreuve joue ni ce qu'elle juge (§ 7, « Le plan court ou le plan standard ») : le plan court et le plan standard des trois jeux, E20. `epreuve` ne dépend ni de `salle` ni de `relais` (preuve `graphe`) ; `partie` n'a pas changé. `web/` n'a pas changé.

### Les mesures

- **M11** (calme, 2026-11-06 21 h 39) : une case de journal écrite et synchronisée en 8,0 à 8,9 ms en médiane, au plus 13,2 ms ; le point de Tier list au pire en 18 ms en médiane, au plus 26 ms ; `sync_all` fait `F_FULLFSYNC` (constaté par son coût : 3,8 ms, contre 22 à 37 µs pour `fsync`). MESURES-PHASE-3, « Lot 5 ».
- **M15** : 153 fils et 14,4 Mio résidents au plus, à 71 téléphones (Sidequest, 10 min, charge de 2 à 4) ; 27 fils et 10 Mio à 8 (Punchline).
- La mémoire des commandes : `cargo test --workspace` 1,77 Go ; une soirée native de Sidequest à 71 téléphones, 0,20 Go pour le banc (l'autorité et son wasmtime, 71 téléphones, le mandataire), 14 Mio pour le relais. Estimé au § 7 : « de 0,3 à 1,1 Go » pour 71 instances de wasmtime : les téléphones natifs du lot 5 n'en ont pas (voir plus bas). Les durées ci-dessus, hors M11, sont celles de commandes sous charge, pas des mesures.

### Ce qui n'est pas fait, et pourquoi

- **Les téléphones natifs ne jouent pas le module** : ils jouent les coups que l'autorité leur envoie (`VersAppareil::Banc`), comme le veut le lot 5 ; « les téléphones natifs partagent un seul `Engine` et un seul `Module` » (§ 8.1) vaudra au lot 12, quand ils passeront par la page.
- `GET /web/…`, `GET /jeux/…`, `GET /banc`, `POST /banc/rapport`, `AjouterLigne` : lots 9 à 11 (écart 7). La page de démarrage est minimale.
- Aucun test de coupure de courant (le disque synchronisé est constaté par son coût, pas éprouvé contre une coupure).
- Le mandataire recopie les valeurs du monde simulé de l'épreuve (16 à 66 ms, 100 ms, 0,5 s) au lieu de les relire : elles sont `pub(crate)` dans `epreuve`, et le banc ne dépend pas d'elle ; M5 les remplacera aux deux endroits (§ 8.1).
- Le relais ne garde pas de trace des refus d'admission (une connexion refusée est fermée sans un mot, avant tout HTTP : il n'y a personne à qui le dire).

### Ce que le lot suivant doit savoir

- `relais::coque::lancer(&Reglages)` lance le relais dans un processus (les tests) ; `Reglages::page` accueille la page de démarrage (lot 10b) ; la table du paquet `web/` reste à brancher (lot 10a), comme les routes `/jeux/…` et le canal du banc.
- Le battement est le `Ping` de WebSocket : une page ne le voit pas ; un téléphone juge l'autorité par ses échos, et le Worker doit juger le relais par ce qu'il en reçoit (le relais lui envoie au moins ses `Ping`, invisibles en JavaScript : prévoir un `VersRelais::Echo` périodique si le Worker doit détecter un relais muet).
- La coque refuse un message de plus de `TRAME_MAX` + 4 Kio sur tout lien qui n'est pas une garde admise ; le Worker de l'écran ne doit pas envoyer un `Ecrire` sur son lien temps réel.
- `soiree-banc soiree-native` sert de banc à tout changement de `salle` ou du relais : son enregistrement grossit d'environ 0,5 Mo/s à 71 téléphones (306 Mo pour 10 min), lu en flux. Ses jugements sont ceux de `salle::simulation`.
- M11 ne demande pas de changer la cadence de 100 ms. `Pannes::SOIREE` (1 à 20 ms) peut rester.
- Le relais lancé sans `--sans-reseau` écoute sur les adresses du réseau local : sous macOS, le pare-feu peut le demander (SAL-95, fiche de la veille).

## Lot 5, correction après la vérification

Fait le 2026-11-06, de 22 h 15 à 23 h 20 environ (GMT), sur la même machine. Chaque commande cargo lourde, chaque contrôle, chaque soirée native et M11 sont passés par le portillon (`--calme` pour M11). Aucun navigateur. Aucun processus `soiree-relais`, `soiree-banc` ni mandataire ne reste. Rien n'est commité. Les jeux ne sont pas touchés ; le `Cargo.lock` de Tier list reste modifié comme au lot 4b (`53a1f747…`).

### Les constats, et ce qui en est fait

1. **[important] Moins de contrôles négatifs qu'annoncé** : vérifié, juste (dix-sept contrôles pour quarante et un tests du relais ; SAL-42, SAL-49, SAL-43 du cœur, SAL-75 du cœur, SAL-76 et SAL-77 de la coque, `qui_parle…_sal33`, SAL-07 de la garde, P41, SAL-70, REM-64 sans le leur). **Corrigé** : chaque test du relais et `le_relais_ne_voit_aucun_secret` a désormais au moins un contrôle négatif qui le fait rougir, par `target/travail/lot-5/correction/controles.py` (quarante-quatre contrôles de tests, plus deux soirées, ci-dessous) ; un contrôle peut nommer plusieurs tests, et n'est rouge que si **chacun** a sa ligne `FAILED` (avec `panicked` et le code 101 ; `--no-fail-fast`, pour qu'un test de la bibliothèque qui rougit n'empêche pas ceux du binaire de test de tourner). Seul `le_relais_ne_tire_ni_la_salle_ni_le_chiffrement` garde pour contrôle son témoin, comme le veut le § 2.5. Le test d'`oublier` est renforcé : il lit `GET /salle` juste après l'oubli, avant que l'autorité suivante ne réécrive le code (sinon un `oublier` qui effaçait aussi le code passait). Les documents (ligne du lot 5, `crates/README.md`) disent maintenant ce qui est.
2. **[important] La fouille des secrets ne cherchait sur `devine` que les clés et les jetons** : vérifié, juste (les gestes de `devine` font moins de 8 octets, ses vues moins de 16 ; « 7 aiguilles »). **Corrigé** : le banc cherche aussi, dans tout ce que le relais a vu, chaque **message de geste en clair** (`VersAutorite::Geste` jusqu'aux octets du geste : l'onglet de 64 bits, le numéro, la longueur, le geste, le prénom tapé de `devine` compris ; le préfixe commun de deux encodages du message à deux `fait` différents) et chaque **message de vue en clair** (`VersAppareil::Vue`, entier, d'au moins 8 octets), échantillonnés sur les téléphones (400 environ par sorte : la borne se lit avant chaque téléphone, et un téléphone peut en ajouter un de plus, 401 vus dans la soirée de Sidequest). Pour les gestes, le banc lit la file de chaque téléphone : `salle::appareil::Appareil::gestes_en_file`, un accesseur nouveau, sans rien changer de ce que l'appareil fait. `Bilan::aiguilles` compte les aiguilles par sorte, et le test exige 4 clés, au moins 3 jetons, au moins un message de geste et un message de vue (sur `devine` : 6 messages de geste, 12 de vue). Le contrôle négatif (chiffrement désactivé des deux côtés) doit désormais voir des jetons, des messages de geste **et** des messages de vue ; et deux soirées natives sans chiffrement, Punchline et Sidequest (4 téléphones, 40 s, sans pannes, le binaire `soiree-banc` reconstruit avec le changement), doivent rendre 1 en les voyant : elles voient aussi les octets seuls des gestes (les réponses de Punchline) et des vues.
3. **[mineur] Des preuves prises sur un arbre antérieur** : vérifié. Toutes les preuves de la correction sont prises sur l'arbre final (code hors `.md` `b8a5d3c1…`, non suivis `b82ad863…`, les mêmes au départ et à la fin de chacune) ; les soirées sont refaites avec les binaires reconstruits juste avant (`soiree-banc` `7a66efb8…`, `soiree-relais` `eaaa9077…`), et M11 avec le `mesure_garde` reconstruit (`298e1640…`).
4. **[mineur] Le plan court n'a pas été joué** : vérifié, et l'omission tient encore : la correction ne touche ni `epreuve`, ni `partie`, ni `liaison`, ni `jeu-abi`, ni les moteurs, ni la réécriture, ni `jeu`, ni `web/` (`salle` gagne un accesseur, et `epreuve` ne dépend pas de `salle` : preuve `graphe`, 0 ligne). Le § 7 dit « les autres lots se contentent du plan court » sous la règle « Le plan court ou le plan standard », qui le demande quand le lot change ce que l'épreuve joue ou juge ; ce lot ne le change pas. Non joué, et dit.
5. **[mineur] Le test du graphe desserré** : vérifié. L'exception de `chacha20` par `rand` est désormais tenue par un contrôle : le témoin `tire-snow` tire `chacha20` 0.9.1 par `chacha20poly1305`, et son test exige le refus « relais-temoin tire chacha20 autrement que par rand » ; le contrôle `chacha20-permis-partout` (l'exception étendue à tout chemin) le fait rougir. L'écart au texte d'origine (« aucune crate de chiffrement ») reste celui que dit le § 2.5.
6. **[mineur] Ce que juge `soiree-native`** : vérifié, juste. Rien n'est changé dans le code ; la ligne du lot 5 dit maintenant que « même forme canonique » se juge une fois, à la fin (la garde finale reprise et comparée à l'autorité vivante), chaque reprise en route n'étant jugée que par la chaîne écrite et les accusés, et que « le journal final rejoué retrouve chaque empreinte » (§ 8.1) n'est pas jugé par la soirée native : il est dans la colonne du lot 10a.
7. **[mineur] La garde sur disque** : vérifié, juste. **Corrigé** : un numéro de case est un seul fichier (`cases/<n sur 20 chiffres>`, la sorte de la case dans son premier octet, `p` ou `j`), écrit par `ecrire_durable_en_morceaux` (l'octet de sorte puis les octets, sans recopier une case de 16,8 Mio) : une case qui en remplace une autre de l'autre sorte prend sa place par le seul renommage, sans instant où le numéro n'a plus de case (l'ancienne, déjà dite `Ecrit`, perdue avant que la nouvelle soit sur disque), ni où il en a deux (que la salle refuserait, `RefusDeGarde::Doublon`). La borne de la garde lit la taille de la case remplacée dans les métadonnées de son fichier, sans le lire, et quelle que soit sa sorte (l'ancien code ne retranchait pas une case de l'autre sorte qu'il allait effacer). Tests : `une_case_remplacee_interrompue_garde_l_ancienne_entiere_sal27` (l'écriture du remplaçant interrompue, son fichier temporaire ne pouvant naître : l'ancienne reste entière et listée ; achevée, la nouvelle seule tient le numéro, un seul fichier), et `une_garde_pleine…_sal07` qui vérifie qu'un remplaçant de l'autre sorte ne compte qu'une fois. Que la taille se lise sans lire la case n'a pas de test (c'est un coût) ; le code se lit.
8. **[mineur] `Cargo.toml` nommait `tests/graphe.rs`** : corrigé (`tests/relais/graphe.rs`, là et dans `src/lib.rs`). Le mandataire qui recopie les valeurs du monde simulé de l'épreuve reste tel quel, déjà dit (M5 les remplacera aux deux endroits).

### Les tests qui le tiennent

`crates/relais` : 42 tests (les 41 du lot 5 et `une_case_remplacee_interrompue_garde_l_ancienne_entiere_sal27`), dont le test d'`oublier` et le témoin du graphe renforcés. `crates/banc` : `le_relais_ne_voit_aucun_secret`, renforcé.

**Les contrôles négatifs**, tous refaits sur le code final par `target/travail/lot-5/correction/controles.py` (copie de son exécution dans la preuve) : **quarante-six rouges sur quarante-six**, chaque fichier remis à l'identique (sha256 vérifié par le script ; `avant-controles.txt`, `apres-controles.txt`). Les dix-sept du lot 5 restent, quatre d'entre eux nommant désormais plus d'un test (`un-refus-ne-ferme-pas` : SAL-148 de la coque et SAL-42/148 du cœur ; `le-silence-ignore` : SAL-43 des deux ; `l-origin-non-verifiee` et `le-host-non-verifie` : aussi `qui_parle…_sal33` ; `une-file-sans-borne` : aussi le test de la file) ; `une-adresse-absente-acceptee` ne nomme plus que le test d'`adresses` : la coque a une seconde défense (une adresse annoncée qu'elle ne peut pas écouter l'arrête, SAL-77), et son test a son propre contrôle. Les nouveaux :

| Changement | Rougit |
|---|---|
| `Pleine` refusée comme `SalleInconnue` | `chaque_refus_est_type_et_ferme_son_lien_sal42_sal148` |
| un lien sans bonjour gardé cent fois plus longtemps | `un_lien_sans_bonjour_se_ferme_en_dix_secondes_sal49` |
| un battement toutes les deux périodes | `le_silence_se_voit_et_chaque_lien_bat_sal43`, `le_silence_ferme…_sal43` |
| l'adresse jointe perdue dans le cœur | `l_adresse_jointe_par_chaque_appareil_va_a_l_autorite_sal75`, `l_adresse_jointe_est_celle_de_la_connexion_sal75` |
| un index sur le quatrième octet de tout message | `des_octets_au_hasard_ne_font_jamais_paniquer_le_coeur_rem64` |
| aucun `AutoritePresente` à l'appareil accueilli | `l_autorite_et_les_appareils_se_trouvent_comme_dans_le_relais_en_memoire` |
| une trame trop grosse d'un appareil transmise | `un_message_illisible_ou_trop_gros_ne_coute_que_son_lien` |
| l'écho à l'heure fausse d'une µs | `l_echo_rend_l_instant_et_l_heure_du_relais` |
| la preuve sur disque ignorée à l'accueil d'un appareil | `un_relais_relance_reprend_les_telephones_avant_l_autorite` |
| le port pris en IPv6 contourné | `un_port_pris_dans_une_famille_empeche_le_demarrage` |
| sans réseau, le relais refuse de démarrer | `sans_reseau_le_relais_demarre_en_boucle_locale_sal76` |
| une `--adresse` d'aucune interface acceptée, et l'écoute qui échoue sautée (deux fichiers) | `une_adresse_qu_on_n_ecoute_pas_se_refuse_a_voix_haute_sal77` |
| un en-tête de mandataire admis | `qui_parle_au_relais_se_juge_sur_l_adresse_le_host_et_l_origin_sal33` |
| un téléphone qui se dit `localhost` jugé comme la boucle | `qui_parle…_sal33` |
| la borne de la garde doublée | `une_garde_pleine_refuse_l_ecriture_sans_rien_jeter_sal07` |
| un fichier temporaire resté gardé | `une_case_s_ecrit_durablement_se_relit_et_s_efface` |
| l'ancienne case effacée avant d'écrire la nouvelle | `une_case_remplacee_interrompue_garde_l_ancienne_entiere_sal27` |
| le numéro sans ses zéros | `le_nom_d_une_case_se_lit_et_garde_l_ordre_des_numeros` |
| la preuve jamais écrite sur disque | `un_relais_relance_garde_sa_salle_sa_preuve_et_sa_garde` |
| `oublier` qui garde la preuve | `oublier_efface_la_preuve_et_la_garde_et_garde_le_code_p41` |
| `oublier` qui efface aussi le code | `oublier…_p41` |
| la garde lue à la borne d'une trame | `une_case_plus_grande_qu_une_trame_passe_par_la_garde` |
| l'enregistrement sans ce qui arrive | `l_enregistrement_note_tout_ce_que_le_relais_voit` |
| la lecture du graphe qui n'interdit rien | `le_temoin_qui_tire_snow_fait_echouer_le_test_du_graphe` |
| `chacha20` permis par tout chemin | `le_temoin_qui_tire_snow…` |
| une place remplaçable qui ne remplace pas | `la_derniere_trame_d_une_place_remplace_celle_qui_attend_sal70` |
| la fin avant ce qui attend | `la_fin_passe_apres_ce_qui_attend_et_ferme_la_file` |
| (le lot 5) le chiffrement désactivé | `le_relais_ne_voit_aucun_secret`, la sortie devant dire un jeton, un message de geste et un message de vue vus |
| le chiffrement désactivé, `soiree-banc soiree-native` de Punchline (4 téléphones, 40 s, sans pannes, graine 3) | code 1 : jetons, messages de geste, messages de vue, gestes et vues vus |
| de même, Sidequest | code 1 : de même |

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-5/correction/` (hors dépôt), chaque dossier comme au lot 5 (`commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt`). Le commun est à `11c65c8` ; chaque preuve, au départ comme à la fin : non suivis `b82ad863…`, `git diff HEAD` hors `.md` `b8a5d3c1…` ; Punchline (`dd6d4f0`) et Sidequest (`6e7219c`) propres ; Tier list (`eebc5c1`) `53a1f747…`. Seuls des `.md` ont changé après (LOTS, MESURES, PHASE-3). Lancées à la suite par `target/travail/lot-5/correction/final.sh`.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles-negatifs` | `python3 -I target/travail/lot-5/correction/controles.py` | **46 rouges sur 46** ; 294 s ; un sous-dossier par contrôle |
| `construire-release` | `lourd cargo build --release -p relais -p banc --bins --examples` | code 0 (après les soirées sans chiffrement, `soiree-banc` refait depuis les sources remises) |
| `fmt` | `cargo fmt --all --check` | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `graphe` | les trois `cargo tree` du lot 5 | le relais : `liaison`, `jeu-abi`, `tungstenite`, `httparse`, `if-addrs` et leurs dépendances (`chacha20` par `rand`) ; `epreuve` ne tire ni `salle`, ni `relais`, ni `banc` (0 ligne) ; `salle` ne tire pas `getrandom` pour `wasm32` |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 626 réussis** (les 625 du lot 5 et le test du SAL-27), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 326 s ; 1,99 Go résidents au plus ; charge 2,12 au départ, 3,68 à la fin |
| `soiree-native-punchline` | `lourd /usr/bin/time -l target/release/soiree-banc soiree-native …/punchline_module.wasm --telephones 8 --duree 180 --graine 1 --tuer-le-relais 2 --partie-entiere --travail …` | **code 0** ; 186 s |
| `soiree-native-sidequest-71` | même commande sur `sidequest_module.wasm`, `--telephones 71 --duree 600 --graine 2 --tuer-le-relais 2 --calme 120` | **code 0** ; 606 s |
| `m11` | `lourd --calme target/release/examples/mesure_garde … --fois 64`, puis `lourd --calme python3 -I target/travail/lot-5/fsync.py …` | code 0 ; MESURES-PHASE-3, « M11, refaite après la vérification du lot 5 » |

**Les soirées natives**, refaites :

| Soirée | Module (sha256) | Pannes (graine) | Résultat |
|---|---|---|---|
| Punchline, 8 téléphones, 180 s, `--tuer-le-relais 2 --partie-entiere`, graine 1 | `d3f4ba08…` | 8 autorités tuées, 2 relais tués, 10 redémarrages ; 6 coupures, 5 retenues, 8 rechargements, 3 rafales, 2 fermetures lentes, 3 jumeaux, 1 poignée rejouée, 3 « Reprendre ici » | **code 0** : 848 gestes émis, accusés et écrits, aucun deux fois ; 409 tenus ; **3 phases terminales** ; 1 556 cases, 18 points, 87 167 lignes, aucune entrée écartée ; la garde finale reprend la même partie ; aucun secret parmi **1 384 aiguilles** (4 clés, 11 jetons, 169 gestes, 400 messages de geste, 400 vues, 400 messages de vue) dans 40,9 Mo vus ; 0,08 Go résidents (le banc) |
| Sidequest, 71 téléphones, 600 s, `--tuer-le-relais 2`, graine 2 | `e2ebb6a7…` | 23 autorités tuées, 2 relais tués, 29 redémarrages ; 8 coupures, 15 retenues, 33 rechargements, 9 rafales, 14 fermetures lentes, 17 jumeaux, 11 poignées rejouées, 16 « Reprendre ici » | **code 0** : 30 929 gestes émis et écrits, 30 928 accusés, aucun deux fois (le geste sans accusé n'est en file chez aucun téléphone qui ne soit pas un jumeau à la fin : sinon la soirée le dirait en faute) ; **297 515 tenus** écrits ; 24 phases terminales ; 5 040 cases, 60 points, 2 418 334 lignes, aucune entrée écartée ; la garde finale reprend la même partie ; aucun secret parmi **1 362 aiguilles** (4 clés, 88 jetons, 69 gestes, 401 messages de geste, 400 vues, 400 messages de vue) dans 300 Mo vus ; M15 : 153 fils, 14 736 Kio ; 0,19 Go résidents (le banc) |

### Les mesures

- **M11**, refaite sur machine calme (2026-11-06, 23 h 10, charge 2,71 au départ et à la fin) par `Dossier::ecrire` : 8,0 à 9,2 ms en médiane pour une case de journal, au plus 12,1 ms ; 19,2 ms pour le pire point, au plus 20,2 ms ; `F_FULLFSYNC` 3,8 ms, `fsync` 19 à 28 µs. MESURES-PHASE-3, « M11, refaite après la vérification du lot 5 ».
- **M15**, relue : 153 fils et 14 736 Kio au plus à 71 téléphones ; 27 fils et 15 696 Kio à 8. Pas une mesure de temps.
- Les durées des commandes, hors M11, sont celles de commandes sous une charge de 1,7 à 3,7 : pas des mesures.

### Ce qui n'est pas fait, et pourquoi

- Le reste de « Ce qui n'est pas fait » du lot 5 tient tel quel (routes `/web/…`, `/jeux/…`, `/banc` ; téléphones natifs sans module ; aucun test de coupure de courant ; le mandataire qui recopie les valeurs de l'épreuve).
- Le plan court des trois jeux n'est pas joué (constat 4) : ni l'épreuve ni ce dont elle dépend n'ont changé.
- La soirée native ne juge toujours pas « le journal final rejoué retrouve chaque empreinte » (lot 10a), ni la forme canonique à chaque reprise (constat 6).
- Que la borne de la garde lise la taille sans lire la case n'est pas testé (un coût, pas un comportement).
- Le commentaire de `ECHANTILLON_MAX` (`banc::soiree_native`) dit « gardés au plus » : la borne se lit avant chaque téléphone, et un échantillon peut la dépasser d'un (401 vus). Sans effet sur ce qui est jugé ; laissé pour ne pas refaire les preuves après elles.

### Ce que le lot suivant doit savoir

- La garde sur disque : `cases/<n sur 20 chiffres>`, l'octet de sorte en tête (`relais::garde::octet_de_sorte`, `case_de_sorte`). Toute lecture de la garde passe par `Dossier::lister` et `Dossier::lire`, qui retirent l'octet ; un outil qui lirait les fichiers à la main doit le savoir.
- `le_relais_ne_voit_aucun_secret` et la soirée native cherchent les messages en clair, pas seulement les octets des jeux : un changement de la forme de `VersAutorite::Geste` (l'ordre de ses champs) ou de `VersAppareil::Vue` doit garder `message_du_geste` juste ; il rend `None` si le préfixe ne finit pas par les octets du geste, et le test échoue alors faute de messages de geste cherchés.
- Les contrôles négatifs du relais sont dans `target/travail/lot-5/correction/controles.py` ; un test nouveau du relais y gagne le sien (la règle d'`AGENTS.md`).

## Lot 6 — L'ABI 4

Fait du 2026-11-06, 23 h 10, au 2026-11-07, 00 h 40 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo lourde, chaque épreuve et chaque mesure sont passées par le portillon (`target/hote/lourd`). Aucun navigateur, aucun serveur. Rien n'est commité. Le commun et Tier list sont modifiés ; Punchline et Sidequest ne le sont pas.

### Ce qui est fait

**`jeu-abi`, l'ABI 4** (`jeu_abi::ABI` = 4) :
- `Appel::Page { montre, pas, locaux: LocauxFil }` et `Retour::Page(PageFil)`, à la fin de leurs enums ; `Appel::Page` est une lecture.
- Le module `page` : `LocauxFil` (l'écran et ses marges sûres, les champs, les choix, les maintenus appuyés, les croix, le retenu, les capacités), `PageFil` (un arbre en flux, chaque nœud après son parent, relu strictement : un parent qui ne précède pas son enfant ou n'est pas un conteneur ne se décode pas), `NoeudPage`, `SorteNoeudPage` (`Colonne`, `Rangee`, `Grille`, **`Pile`**, `Texte`, `Image` avec sa **`teinte`**, `CompteARebours`, `Bouton` avec son `Declencheur`, `Maintenu`, `Croix` en quatre ou huit `Directions`, `Champ` avec son `Clavier`, `Choix` balayable ou non), `TenuPage`, `StylePage`, `TaillePage`, `Direction8`, `Envoi`, `EcranTelephone`, `Marges`, et **`valider_page`** (256 nœuds, 16 étages, clés uniques parmi les frères, textes de 1 Kio, 64 options, 12 colonnes, de 8 à 512 px, images et polices déclarées, gestes et tenu sous `limites.geste`, retenu sous 1 Kio ; toutes ces bornes sont choisies).
- La scène : `SorteNoeud::Composant { boite: Option<Rect>, composant }` ; `ComposantVuFil::Modale { fermer, panneau }` ; `ComposantVuFil::Lien { nom, ressource, fichier }`.
- Les entrées et les réponses : `Composant::Fichiers { geste, selecteur: Option<Cle>, fichiers }` ; `Reponse::Rendu(Result<ImageRendue, EchecDemande>)`, `ImageRendue { octets, non_dessinees }`. `Telecharger` ne change pas (un `&[u8]` s'écrit déjà d'un bloc, les mêmes octets qu'un `Vec<u8>`).

**`jeu`** : `Jeu::page(vue: &Recue<VueTelephone>, l: &Locaux<'_>, p: &mut Page<Self>)`, une page vide par défaut ; `Recue` (`montre`, `pas`, `confirmee`, toujours `Oui`) ; `Locaux` (`ecran`, `texte::<L>` nettoyé par `Texte::nettoyer`, `choix`, `appuye`, `direction`, `retenu`, `capacites`) ; `Page` (`colonne`, `rangee`, `grille`, `pile`, `texte`, `image`, `image_teintee`, `compte_a_rebours`, `bouton`, `maintenu`, `croix`, `champ`, `choix`, `tenir` — qui calcule la valeur relâchée par `Tenu::relache` —, `retenir`) ; `ChampPage` ; `Scene::composant_dans(cle, boite, c)`, `Scene::composant` restant sans boîte ; `ComposantVu::Modale::panneau`, `ComposantVu::Lien::fichier` ; le prélude. **`jeu::colle`** traite `Page` comme `Valider`, sans enveloppe : il décode la `MontreFil`, puis la vue du jeu, appelle `Jeu::page`, rend la `PageFil` ; un jeu sans téléphone rend `Illisible` sans être appelé.

**`partie`** : `valider_scene` refuse un champ de plus d'une ligne (`DefautScene::ChampMultiligne`) et vérifie dans le repère la boîte d'un composant et le panneau d'une modale (`dans_le_repere`) ; des fichiers qui nomment leur sélecteur ne passent que par lui (`None` : par tout sélecteur, comme avant) ; `ComposantArrivee::Fichiers::selecteur`, `ReponseArrivee::Rendu(Result<(Vec<u8>, u32), _>)` ; `VERSION_GARDE` = 3 (la garde garde ces deux types). `moteur-commun` : `TAILLE_MODULE_MAX` (4 Mio, choisi), `trace::NOMS` et `releve` nomment `Page`. `reecriture::Reecrit::taille_originale`, `moteur_wasmtime::ModuleCharge::taille_livree`. `salle` : la conversion des fichiers et du rendu suit, sans rien changer d'autre.

**L'épreuve** :
- **E21, « Pages »** (`epreuve/src/pages.rs`), dans `IdControle::DU_PLAN` et l'étape 4 du CONTRAT. La soirée note chaque vue qui part vers un téléphone (`VuesVues`, sans rien tirer de son hasard) ; après les soirées, au plus 256 vues distinctes (les plus petites empreintes FNV-1a) sont appelées en `Page` sur une instance nue (`Bonjour`, `Decrire`), avec des locaux vides puis trois tirés au hasard (écran, textes hostiles, choix hors des options, maintenus, croix, octets retenus, capacités, clés étrangères) ; chaque page doit passer `valider_page`, chaque geste et le tenu (et sa valeur relâchée) `Valider`. Sans objet pour un jeu sans téléphone. Le refus de la page vide d'un jeu à téléphones attend le lot 7.
- **E19** juge chaque `Page` de E21 sur `BudgetsE19::page` = une lecture courante / `MARGE_TELEPHONE` (4, choisi) = 1 302 083 unités, coût fixe compris (`juger_les_pages`).
- **E1** juge la taille du module livré contre `TAILLE_MODULE_MAX`.
- Trois fautifs : **`module-trop-gros`** (E1 : une table de 4,15 Mio lue à chaque entrée, `build.rs` à 128 pages ; module livré de 4 813 717 octets), **`page-qui-panique`** (E21 : la page indexe un tableau de dix par l'option choisie, sans la borner ; seuls des locaux tirés au hasard la font paniquer), **`page-qui-envoie-un-geste-invalide`** (E21 : un bouton dont le geste, 300 octets bruts, dépasse `limites.geste`). Le socle des fautifs a une page (prénom, « Prêt », taper et choisir un chiffre, résultat) et un variant de geste de plus, `GesteTel::Octets`, qu'il ignore.

**Les pages** : `devine` (le prénom, l'attente, le carrousel des chiffres dont le choix est le curseur tenu, le résultat) ; le socle des fautifs ; `ardoise` n'a pas de téléphone (`Page` y est illisible, testé).

**Le carrousel de Sidequest** : `pile` et `teinte` retenues, tranchées sur son cahier (§ 6.1) ; l'agent de Sidequest n'a pas pu être joint (voir les écarts).

**Tier list**, adapté dans son dépôt, sans changer son comportement : `panneau` = le voile entier (`vue.rs`), son champ d'étiquette en `Lignes::Une`, `Composant::Fichiers { geste, fichiers: f, .. }` et `o.octets` du rendu (`lib.rs`), ses tests (motifs à champs de `SorteNoeud::Composant`, `selecteur: None`, `ImageRendue`), `BESOINS-COMMUN.md` (§ 7, § 8). Son `Cargo.lock` ne change pas.

**L'exemple `epreuve/examples/cout_d_un_noeud.rs`** : le carburant d'un nœud de scène sur `ardoise`.

Documents : PHASE-3 (§ 2.10, § 6, § 8.3, ligne du lot 6), MESURES-PHASE-3 (« Lot 6 »), BESOINS-DES-JEUX (besoin 24, changements d'API, « Ce qui n'est pas fait »), CHANGELOG (« Phase 3, lot 6 »), `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **`SorteNoeud::Composant { boite: Option<Rect>, … }`**, pas `boite: Rect` : `Scene::composant`, qui doit rester ce qu'il est (§ 6), ne connaît pas de boîte.
2. **Tier list cassait en deux points de plus** que `panneau` et `..` : un `Champ` en `Lignes::Libre` (refusé désormais) et la lecture de `Reponse::Rendu(Ok(_))` ; ses tests filtraient `SorteNoeud::Composant(_)`. Adaptés de même.
3. **`partie::VERSION_GARDE` monte à 3** : la garde du cœur garde `ComposantArrivee::Fichiers` et `ReponseArrivee::Rendu` ; leurs octets figés sont réécrits à la main (`tests/coeur/forme_garde.rs`), avec la seconde lecture.
4. **Le vocabulaire du carrousel n'est pas « tranché avec l'agent de Sidequest »** : l'agent du lot 6 n'a aucun canal vers lui. Tranché sur le cahier de Sidequest, à confirmer au lot 7 (§ 2.10).
5. **`page-qui-envoie-un-geste-invalide`** fait échouer E21 par la taille du geste (`valider_page`), pas par `Valider` : l'API typée de `Page` ne sait pas produire un geste que `Valider` refuse. Le contrôle par `Valider` reste, pour un jeu dont la forme serait plus stricte au décodage qu'à l'encodage.
6. **Des fichiers sans sélecteur (`None`) passent par tout sélecteur**, comme avant l'ABI 4 ; le contrôle de `depot_partout` et `collage` vient avec la page de l'écran (lot 11). Le bot de l'épreuve envoie `None`.
7. **Le code du § 2.10** : `Page<J>` sans durée de vie ; `compte_a_rebours` prend un `CompteARebours` ; `Page::image_teintee` ; `ChampPage<'_, G>`. `VersAppareil::Ressource` n'est pas écrit (lots 10a et 12).

### Les tests qui le tiennent

- `jeu-abi`, `tests/abi` : `forme_figee` réécrit à la main pour l'ABI 4 (cinq cas nouveaux : `Appel::Page`, `Retour::Page`, `SceneFil::Composant`, `EntreeFil::Ecran::Composant::Fichiers`, `EntreeFil::Reponse::Rendu`), passé du premier coup ; `le_corpus_couvre_chaque_variant` (22 appels, 18 retours) ; `chaque_appel_a_sa_sorte` ; `abi_est_celle_des_octets_figes` ; `strict` (le variant 0x16 inconnu) ; `seconde_lecture_de_la_page` (proptest : `Appel::Page` relu sans `jeu_abi`, sans postcard) ; `page.rs` : `une_page_vide_et_une_page_complete_sont_valides`, `le_decodage_refuse_un_parent_qui_ne_contient_rien_ou_vient_apres`, `les_bornes_de_la_page_se_nomment_rem57`, `chaque_noeud_porte_ses_bornes_rem57`, `des_octets_au_hasard_ne_font_jamais_paniquer_la_page_rem64`, `retirer_la_derniere_feuille_garde_la_page_valide`.
- `partie`, `tests/coeur` : `la_boite_d_un_composant_et_le_panneau_d_une_modale_sont_dans_le_repere`, `un_champ_de_plus_d_une_ligne_est_refuse`, `des_fichiers_qui_nomment_leur_selecteur_ne_passent_que_par_lui`, `les_octets_de_chaque_variant_sont_figes` (version 3), `les_octets_de_la_garde_sont_figes`.
- `devine`, `tests/regles.rs` : `la_page_se_lit_depuis_la_vue_et_les_locaux` (par la colle, sans état créé : le prénom sale part nettoyé, le choix du carrousel devient le curseur tenu, une vue illisible rend `Illisible`) ; `ardoise`, `tests/colle.rs` : `la_page_d_un_jeu_sans_telephone_est_illisible`.
- `epreuve` : `budgets::tests::une_page_trop_lourde_pour_un_telephone_echoue`, `les_budgets_se_derivent_du_temps_d_une_image` (`page` = 1 302 083) ; `pages::tests::les_vues_gardees_sont_les_plus_petites_empreintes` ; `tests/epreuve` : `module_trop_gros_echoue_e1`, `page_qui_panique_echoue_e21`, `page_qui_envoie_un_geste_invalide_echoue_e21`, et chaque fautif d'avant toujours en échec de son seul contrôle avec E21 au plan (plan court) ; `devine_passe_tout`, `le_socle_sain_passe_tout` (E21 réussi).
- `moteur-commun`, `instantane` : octets inchangés sous la version 3.
- Tier list : ses 90 tests, adaptés (`partie.rs`, `soiree_fichiers.rs`, `commun/mod.rs`).

**Les contrôles négatifs** de ce lot sont les trois fautifs, chacun rouge sur son seul contrôle (`module-trop-gros` : E1 ; `page-qui-panique` et `page-qui-envoie-un-geste-invalide` : E21), sur le plan court, par leur module et leur copie native. Les tests nouveaux de `jeu-abi` et de `partie` portent leurs cas refusés en eux-mêmes ; aucune mutation du code n'a été faite pour les faire rougir (pas de `controles.py` à ce lot).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-6/` (hors dépôt), chaque dossier avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (pour le commun et chaque jeu : `HEAD`, sha256 de `git diff HEAD`, de `git diff HEAD` hors des `.md`, et des fichiers non suivis). Toutes les preuves finales, lancées à la suite par `target/travail/lot-6/final.sh`, ont la même empreinte du code au départ et à la fin : commun à `320b9b1`, `diff HEAD` hors `.md` `f994361797d8d5bb`, non suivis (12) `ef13dc5b315a9914` ; Tier list à `5ec845c`, hors `.md` `355061048ec3b646` ; Punchline (`dd6d4f0`) et Sidequest (`6e7219c`) propres. Seuls des `.md` ont changé après (LOTS, PHASE-3, MESURES). Les empreintes des modules éprouvés sont dans `target/preuves/lot-6/modules.txt` (sha256 du `.wasm`, et le BLAKE3 de chaque rapport).

| Preuve | Commande | Résultat |
|---|---|---|
| `cout-noeud-avant` | `lourd bash -c 'cargo build -p ardoise-module --target wasm32-unknown-unknown --profile module && … cargo run --release -p epreuve --example cout_d_un_noeud -- ardoise-avant.wasm'`, avant tout changement (l'exemple seul, non suivi, puis reformaté par `cargo fmt` sans changer ce qu'il fait) | code 0 ; module `a77e81f5…` ; un `Rect` 2 395 en médiane |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve --example cout_d_un_noeud` | code 0 |
| `cout-noeud-apres` | `lourd target/release/examples/cout_d_un_noeud ardoise-apres.wasm` | code 0 ; module `036799a4…` ; un `Rect` 2 395 en médiane |
| `court-sidequest` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/sidequest --court --json …` | **code 0**, comme la référence ; E21 réussi (1 024 évaluations : 256 vues sur 8 019 distinctes, page vide) ; 136 s ; module `0f4df7a2…` |
| `court-punchline` | même commande, `../games/punchline` | **code 1, E2 seul** (« une fin, mais aucune autre après la nouvelle partie »), comme la référence ; E21 réussi (1 024 ; 256 vues sur 15 896) ; 44 s ; module `f7ab5c4a…` |
| `court-tier-list` | même commande, `../games/tier-list` | **code 1, E15 seul** (octets vivants de 201 670 à 213 975, les nombres mêmes de la référence) ; E21 sans objet (« le jeu n'a pas de téléphone ») ; 66 s ; module `5e9cd937…` |
| `devine-standard` | `lourd /usr/bin/time -l target/release/soiree-epreuve crates/exemples/devine --json …` (plan standard, depuis ses sources) | **code 0** ; E21 réussi (216 évaluations : 32 vues distinctes, pages de 4 nœuds au plus) ; 64 s ; module `2c2b3ca6…` |
| `fmt` | `cargo fmt --all --check` | rien à reformater |
| `clippy-commun` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie -p jeu -p jeu-abi --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `clippy-web` | dans `web/` : `cargo clippy --workspace --all-targets -- -D warnings`, puis pour `wasm32-unknown-unknown` | code 0 |
| `jeux-clippy` | dans chaque jeu : `cargo clippy --workspace --all-targets -- -D warnings` et `cargo fmt --all --check` | code 0 : **les trois jeux compilent**, Tier list compris |
| `tier-list-tests` | dans Tier list : `cargo test --workspace` | code 0 |
| `test-workspace` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 643 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 598 s ; 1,96 Go résidents au plus ; charge 3,53 au départ, 2,86 à la fin |

Le plan court des trois jeux rend la référence du § 7 de PHASE-3 (« Le plan court ou le plan standard ») : Sidequest 0, Punchline 1 par E2 seul, Tier list 1 par E15 seul, avec les mêmes constats ; E21 s'y ajoute, réussi ou sans objet. La référence n'est donc pas relevée à nouveau. E20 n'est pas relancé : le lot ne touche ni le relevé ni le banc (§ 7, ligne du lot 6 : le plan court et le plan standard de `devine` suffisent).

### Les mesures

- **Le coût d'un nœud de scène** (carburant, pas une mesure de temps) : un `Rect` d'`ardoise` 2 395 unités en médiane avant (ABI 3) comme après (ABI 4) ; la vue entière d'`ardoise` 61 102 → 62 535 (+2,3 %, une fois par scène). MESURES-PHASE-3, « Lot 6 ». Le chiffre de RESTE-PHASE-2 (4 500 à 4 800) est celui de la phase 1.
- **Le coût d'une page** (carburant, E19) : `devine` 12 156 en médiane, au plus 29 823 ; la page vide de Sidequest au plus 26 749 ; celle de Punchline jusqu'à **722 971** (99ᵉ centile 707 303) — c'est le décodage de la vue de Punchline dans la colle, la page étant vide —, soit, avec son coût fixe de 155 800, 67 % du budget d'une page (1 302 083) avant que Punchline n'écrive la sienne au lot 7.
- Les durées des commandes ci-dessus sont celles de commandes sous une charge de 1,8 à 3,5 : pas des mesures. Mémoire attendue au § 7 : « 3 à 4 Go » (estimé) ; mesurée : 1,96 Go résidents au plus pour `cargo test --workspace`, de 0,78 à 0,93 Go pour une épreuve.

### Ce qui n'est pas fait, et pourquoi

- **Le refus de la page vide** (E21) et la règle F8 de E1 : au lot 7, avec les pages des jeux.
- **`VersAppareil::Ressource`**, l'envoi des polices et des images aux téléphones : lots 10a et 12.
- **L'agent de Sidequest n'a pas été consulté** sur `pile` et `teinte` (écart 4).
- **Aucune mutation pour les tests nouveaux** de `jeu-abi` et de `partie` (voir plus haut) ; les fautifs sont les contrôles négatifs des règles de l'épreuve.
- **Le plan standard des 38 fautifs** (`--ignored plan_standard`) n'est pas relancé : le § 7 ne le demande pas à ce lot ; le plan court les tient tous.
- **Le sélecteur nommé n'est pas encore éprouvé dans les soirées** : le bot de l'écran ne dit pas d'où viennent ses fichiers (`ComposantBot::Fichiers`), l'épreuve envoie `None`. Si Tier list veut que ses bots disent leur sélecteur (lot 7), `ComposantBotFil::Fichiers` devra le porter (une ABI de plus, ou un ajout à la fin).

### Ce que le lot suivant doit savoir

- Un jeu se recompile : un module de l'ABI 3 est refusé (« le moteur de 4 à 4 »). Tout `.wasm` gardé d'un lot précédent (captures, `dist/`) est périmé.
- La page d'un jeu se teste par la colle sans état créé (`devine/tests/regles.rs`, `la_page_se_lit_depuis_la_vue_et_les_locaux`) ; E21 la juge dans wasmtime sur les vues réelles des soirées. Le décodage de la vue compte dans le carburant de `Page` : Punchline en paie déjà jusqu'à 723 000 unités pour une page vide.
- E21 n'est pas dans `DES_SOIREES` : une panne déclarée non constatée ne le rend pas « pas exécuté ». Pour armer le refus de la page vide (lot 7), il suffit de regarder, dans `pages::eprouver`, si chaque page valide est vide alors que le jeu a des téléphones.
- `partie::VERSION_GARDE` vaut 3 ; une garde de la version 2 ne se relit plus.
- `moteur_commun::TAILLE_MODULE_MAX` est la borne que le Worker de l'écran appliquera au chargement (lot 10a) : la relire, ne pas la recopier.

## Lot 7 — Les jeux passent à l'ABI 4

Les jeux passent l'un après l'autre, chacun par son agent et dans son dépôt ; la règle F8 de E1 et le refus de la page vide (E21), avec leurs fautifs, viennent après les trois jeux, dans le commun. Chaque part s'ajoute ci-dessous.

### Punchline

Fait le 2026-11-07, de 00 h 15 à 01 h 00 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo lourde et l'épreuve sont passées par le portillon (`target/hote/lourd`). Aucun navigateur, aucun serveur. Rien n'est commité. **Seul le dépôt de Punchline est modifié** (et ce journal) : le commun ne l'est pas, ni Sidequest, ni Tier list.

#### Ce qui est fait

**La page du téléphone** (`games/punchline/crates/punchline/src/page.rs`, `Jeu::page`) : la barre (le personnage et le prénom, ou le logo ; le compte à rebours), puis une colonne par écran du cahier (§ 6) — le prénom (champ au clavier `Prenom`, « C'est moi ! » et l'Entrée portent le prénom tapé, nettoyé ; « Ta place a été libérée… »), le salon (une grille de deux colonnes : un bouton par personnage libre, le sien en évidence, les pris grisés avec le prénom de leur joueur ; « Tout le monde est là ! » chez l'organisateur, « Il manque encore n joueurs. » ou « Le premier arrivé lance la partie. »), le départ (l'énorme compte à rebours, « Attends, on annule ! » chez l'organisateur), l'annonce, l'écriture (« Consigne 1 sur 2 », la consigne, un champ par case, « Réponse 1 » en finale, « n/80 », « Envoyer », « Je sèche (½ points) » ; le brouillon en tenu), « C'est envoyé ! », le vote (deux boutons, chacun avec sa réponse, ses cases une par ligne, « … » pour une case vide de la finale), les écrans d'attente, le podium (« 1er sur 6 », le personnage, « N points », « On rejoue ! » chez l'organisateur). Les polices du jeu (Rubik, Rokkitt, Schoolbell), les couleurs de la pâte du cahier (§ 8).

Trois choix, écrits en tête de `page.rs` :
- **Les champs d'une réponse ont une clé qui porte la consigne** (`page::cle_case`, PUN-04) : le commun garde un champ par sa clé, et une clé constante ferait passer ce qui est tapé (et le brouillon) d'une consigne à la suivante.
- **Le brouillon ne part que d'un champ touché** : la page ne déclare de tenu que si l'un des champs de la consigne est dans les locaux. Sinon une page rechargée, champ vide, effacerait le brouillon que la partie garde (CAHIER, § 6) ; un champ vidé, lui, est dans les locaux, et son tenu vide retire le brouillon (§ 4.4). Cela suppose une sémantique des locaux que le commun n'a pas encore écrite (demande 2 ci-dessous).
- **Ce qui est trop lourd est recoupé, jamais refusé** : une case de 80 capitales peut peser des milliers d'octets de marques combinantes. Chaque texte montré est coupé sous `jeu_abi::TEXTE_PAGE_MAX` (1 024 octets), chaque case tapée sous `(limites.geste − 32) / 3` octets puis renettoyée, pour que « Envoyer » et le tenu tiennent sous `limites.geste` (4 096) ; la page reste valide quoi que porte la vue ou que tape le joueur (REM-57).

**Les polices en TTF** (P22) : `ressources/polices/{rubik,rokkitt,schoolbell}.ttf`, déclarées comme assets à la place des `.woff2` (gardés à côté, non compilés, comme source de la conversion). Faute d'outil sur la machine (ni `woff2_decompress`, ni fontTools), un décodeur WOFF2 est écrit pour l'occasion dans le dépôt du jeu, **hors de son workspace** : `outils/woff2-vers-ttf/` (son propre `[workspace]`, la recommandation du W3C « WOFF File Format 2.0 » : répertoire, flux Brotli, reconstruction de `glyf`, `loca` et `hmtx`, sommes de contrôle). Sa seule dépendance tierce, `brotli-decompressor` 5.0.3, est la version du registre déjà en cache (la dernière publiée, 6.3.0, vue par `cargo search`, n'y est pas ; l'outil se construit `--offline`). Les TTF ont exactement la taille que l'en-tête de chaque `.woff2` annonce (`totalSfntSize` : 81 216, 66 316 et 45 060 octets). **Vérifiés par un décodeur indépendant** : Core Text lit le `.woff2` lui-même, et `outils/woff2-vers-ttf/verifier.swift` compare, glyphe par glyphe, chaque élément de contour, chaque chasse et chaque boîte, et les octets de chaque table hors `glyf` et `loca` (`head` sans `checkSumAdjustment`) : aucun écart sur 293, 245 et 215 glyphes (10 415, 6 510 et 11 600 éléments de contour). Son contrôle négatif : un octet de `glyf` changé dans le TTF de Rubik, « glyphe 144 : contours différents ». Rubik et Rokkitt sont des polices variables (`fvar`, `gvar`, `avar`).

Le reste du dépôt : `ecran.rs` partage ses couleurs de pâte avec la page (`pub(crate)`) ; `.gitignore` ignore `outils/*/target` ; `AGENTS.md`, `README.md`, `BESOINS-COMMUN.md` (§ 11 nouveau, et « Ce que dit l'épreuve » refait).

#### Les écarts

1. **Le champ de trois lignes** du cahier (§ 6, écriture des manches 1 et 2) est un champ d'une ligne : l'ABI 4 n'en a pas d'autre (PHASE-3, § 6, le multiligne différé). La réponse ne change pas (80 capitales). Demande 3 de `BESOINS-COMMUN.md`, § 11.
2. **Le motif du dernier refus** sous le prénom, et le bandeau d'un refus, ne sont pas sur la page : l'avis part vers le téléphone en octets du jeu, que ni `Recue` ni `Locaux` ne donnent à la page, et le commun ne sait pas l'écrire. Demande 1 : un changement d'ABI (le dernier avis dans `Recue`) ou une façon pour le commun de demander au module le texte d'un avis.
3. **Les personnages** sont des plaques de leur couleur à leur nom (aucune image déclarée) ; la grille du départ, « masquée et inactive » dans le cahier, est retirée (aucun nœud masqué) ; le compte à rebours « pressé » n'a pas de vocabulaire. Le focus, la touche « suivant », Entrée qui passe à la case suivante et les retours à la ligne d'un bouton (`\n`, qu'il faudra rendre en `pre-line`) attendent le commun (demandes 4 à 6).
4. PHASE-3 n'a pas été trouvé faux sur la part de Punchline ; il n'est pas modifié.

#### Les tests qui le tiennent

- `tests/page.rs` (nouveau ; `Page` sur une instance de téléphone qui n'a rien créé, chaque page passée par `valider_page`, chaque geste et le tenu par `Valider`, `Pilote::page`) : `une_partie_entiere_se_joue_par_les_pages` (cinq téléphones, de l'arrivée au podium puis « On rejoue ! », tout geste venu d'un bouton ou d'un tenu de la page ; l'organisateur seul a ses boutons ; PUN-76, PUN-85) ; `le_brouillon_tape_sur_la_page_compte_a_l_echeance` (PUN-01, PUN-03) ; `un_champ_jamais_touche_ne_declare_aucun_brouillon` (CAHIER, § 6 et § 4.4) ; `les_champs_d_une_consigne_portent_sa_cle` (PUN-04) ; `chaque_ecran_reste_une_page_valide_dans_ses_pires_cas` (REM-57 : un prénom et des cases de 4 001 octets pour une capitale, trois cases tapées trop lourdes) ; `la_borne_d_un_texte_est_celle_du_commun` ; `une_vue_illisible_ne_donne_pas_de_page`.
- `tests/contenu.rs`, `les_polices_sont_des_fichiers_sfnt` (P22 : saveur TrueType, répertoire trié, tables alignées et dans le fichier, somme de chaque table et du fichier, tables d'une police TrueType, `head` et `loca` d'accord avec `maxp`) ; ses contrôles négatifs sont dans le test : le `.woff2` de Rubik et le TTF à un octet changé y échouent.
- `tests/commun/mod.rs` gagne `Pilote::page`, `page_de`, `valider_page`, `appuyer`, `tenir_la_page`.

**Les contrôles négatifs** (`target/travail/lot-7/controles.py`, sept mutations de `page.rs`, chacune rouge sur son test et pour la raison attendue, le fichier remis tel quel ensuite) : la clé de case sans la consigne (`les_champs_d_une_consigne_portent_sa_cle` : deux clés égales) ; un tenu même sans champ touché (`un_champ_jamais_touche…`) ; aucun tenu (`le_brouillon_tape…` : `None`) ; les textes montrés non bornés (`chaque_ecran…` : `TexteTropLong`) ; les cases tapées non recoupées (`chaque_ecran…` : `GesteHorsBornes`) ; « Lancer » chez tout le monde et « Envoyer » sans les cases (`une_partie_entiere…`).

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7/` (hors dépôt), chaque dossier avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée) et l'empreinte au départ et à la fin (`empreinte.txt`, `empreinte-fin.txt` : pour le commun et chaque jeu, `HEAD`, sha256 de `git diff HEAD`, de `git diff HEAD` hors des `.md`, et des fichiers non suivis). Toutes les preuves finales, lancées à la suite par `target/travail/lot-7/final.sh` de 00 h 49 à 00 h 52, ont la même empreinte du code au départ et à la fin : Punchline à `dd6d4f0`, `diff HEAD` hors `.md` `c7588ab241388f21`, non suivis (9) `b10f0d5c3d93dfc4` ; le commun (`c371707`), Sidequest (`6e7219c`) et Tier list (`32f3759`) propres. Seuls des `.md` ont changé après (`BESOINS-COMMUN.md`, `AGENTS.md` du jeu, ce journal). Le module éprouvé : `target/preuves/lot-7/modules.txt` (sha256 `82669f7744d63c7b…`, 941 989 octets).

| Preuve | Commande | Résultat |
|---|---|---|
| `outil-clippy` | `lourd bash -c "cd …/outils/woff2-vers-ttf && cargo fmt --check && cargo clippy --offline --release … -- -D warnings"` | code 0 |
| `outil-conversion` | l'outil refait les trois TTF depuis les `.woff2`, `cmp` avec ceux du dépôt | code 0 : identiques |
| `outil-coretext` | `swiftc -O verifier.swift`, puis le vérificateur sur chaque police | code 0 : aucun écart (macOS 27.2, 26B123) |
| `jeu-fmt` | dans Punchline : `cargo fmt --all --check` | code 0 |
| `jeu-clippy` | `lourd … cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `jeu-clippy-wasm32` | `lourd … cargo clippy -p punchline -p punchline-module --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `jeu-tests` | `lourd /usr/bin/time -l … cargo test --workspace --no-fail-fast` | **code 0 : 85 réussis**, 0 échec, 0 ignoré ; 13 s ; 0,30 Go résidents |
| `jeu-module` | `lourd … cargo build -p punchline-module --target wasm32-unknown-unknown --profile module` | code 0 ; 941 989 octets, sha256 `82669f77…` |
| `controles` | `python3 -I target/travail/lot-7/controles.py` | code 0 : les sept mutations rouges |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 (rien à refaire) |
| `standard-punchline` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/punchline --json …/standard-punchline.json` (plan standard, depuis les sources) | **code 0** ; E21 réussi (2 446 évaluations : 256 vues sur 74 158 distinctes, pages jusqu'à 17 nœuds, gestes et tenus passés par `Valider`) ; 109 s ; 1,60 Go résidents ; charge 2,39 au départ, 4,08 à la fin |
| `court-punchline` (avant les preuves finales, même module) | `lourd /usr/bin/time -l cargo run --release -p epreuve --bin soiree-epreuve -- ../games/punchline --court --json …` | code 1, **E2 seul** (« une fin, mais aucune autre après la nouvelle partie »), la référence du § 7 ; E21 réussi (2 198) ; 31 s |

Le plan standard est exigé par la ligne du lot 7 (« chaque jeu au code 0 depuis ses sources, plan standard ») : il est vert pour Punchline. Le `cargo test --workspace` du commun n'est pas relancé : le commun n'a pas changé.

#### Les mesures

- **Le coût d'une page** (carburant, E19, déterministe ; pas une mesure de temps) : plan standard, 1 024 pages, médiane 70 317 unités, 99ᵉ centile 704 676, maximum 846 390 (plan court : médiane 45 748, maximum 740 229) ; avec le coût fixe de 155 800, 77 % du budget provisoire d'une page (1 302 083). La page vide du lot 6 coûtait déjà jusqu'à 722 971 : presque tout est le décodage de la vue (chaque `Texte` renettoyé). Le jeu garde la forme de sa vue tant que le budget tient (`BESOINS-COMMUN.md`, § 11).
- Le module passe de 810 384 octets (lot 6, `.woff2`) à 941 989 (TTF) ; mémoire de 39 à 128 pages.
- Les durées ci-dessus sont celles de commandes sous une charge de 1,5 à 4,1 : pas des mesures.

#### Ce qui n'est pas fait, et pourquoi

- **La page n'a été vue dans aucun navigateur** : le commun n'a pas encore de page de téléphone qui dessine une `PageFil` (lot 12) ; le lot 7b la fera voir à l'agent du jeu.
- **Les six demandes** de `BESOINS-COMMUN.md`, § 11 (le motif d'un refus, la sémantique des champs dans les locaux, le champ de trois lignes, les retours à la ligne d'un bouton, le clavier et le focus, les personnages et la pâte) : au commun.
- La règle F8 de E1 et le refus de la page vide (E21), avec `diagnostic-f8` et `page-vide` : au commun, après les trois jeux. Punchline a une page non vide pour chaque vue (au moins la barre), et sa touche de diagnostic est `KeyD`.

#### Ce que le jeu suivant et le commun doivent savoir

- Le pilote des tests de Punchline (`tests/commun/mod.rs`, `Pilote::page`, `appuyer`, `tenir_la_page`) joue une partie entière par les pages ; il peut servir de modèle aux deux autres jeux.
- `outils/woff2-vers-ttf` est réutilisable par Tier list s'il part d'un `.woff2` (« une police libre en TTF ») ; il refuse une collection (`ttcf`).
- Pour armer le refus de la page vide : la page de Punchline n'est jamais vide (barre et colonne d'écran). Pour le contrôle par `Valider` de E21, le pilote fait déjà la même chose dans les tests du jeu.
- Le décodage de la vue domine le coût de `Page` chez Punchline : si le budget d'une page baisse après la mesure sur de vrais téléphones (§ 8.5), Punchline devra porter ses textes de vue autrement que par `Texte`, ou le commun ne plus renettoyer un `Texte` à son décodage dans une lecture.

### Sidequest

Fait le 2026-11-07, de 01 h 00 à 01 h 40 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo lourde, chaque épreuve et les contrôles négatifs sont passés par le portillon (`target/hote/lourd`) ; seul un `cargo check -p sidequest` de 8 s s'en est passé. Aucun navigateur, aucun serveur. Rien n'est commité. **Seul le dépôt de Sidequest est modifié** (et ce journal) : le commun ne l'est pas, ni Punchline (dont l'arbre porte encore la part non commitée de son agent), ni Tier list.

#### Ce qui est fait

**La page du téléphone** (`games/sidequest/crates/sidequest/src/page.rs`, `Jeu::page`) :
- **le sélecteur** (CAHIER, § 6.1) : « 1P », « TON PERSONNAGE » ; le héros entre ses deux voisins, chacun dans sa couleur par défaut ; le carrousel des cinq personnages, un `choix` balayable ; la vue du héros (face, droite, dos, gauche), un `choix` ; les six couleurs, un `choix` **dont la clé porte le personnage** (changer de personnage montre un choix neuf, et le héros prend sa couleur par défaut, 0, 2, 4, 1, 3) ; deux accessoires, deux `choix` ; le résumé (« Personnage 3 sur 5, Violet, Casquette, Lunettes. ») ; le champ du prénom ; « C'EST MOI ! », ou, si le prénom serait refusé, le même texte grisé et le refus du jeu dessous (« Ces caractères ne s'affichent pas sur la TV… »).
- **la manette** (§ 6.2) : le prénom et « En ligne · SALLE » ; ce que fait la partie quand les commandes sont désactivées (« Pause », « Résultats : regarde la TV », « Partie finie ») ; **la croix en huit directions**, toujours là (SQ-95), dont la direction est le tenu de la page (droite − gauche, bas − haut, § 7.1), le stop quand les commandes sont désactivées ; le bouton de saut **au contact** (SQ-07), posé dans une `pile` sur l'avatar du joueur (SQ-102) ; le message : un champ dont la clé porte le pas du dernier message accepté (il se vide à l'acceptation), le compteur « n/80 » ou l'aperçu exact de ce que la télé en dessinera, ou le refus ; « Envoyer » ou « Message envoyé ✓ » pendant 2,5 s.
- **Une seule règle pour la page et l'étape** (invariant 11 du cahier, SQ-40) : `regles::juger_prenom` et `regles::juger_texte_du_message`, écrites pour la page et lues aussi par `juger_rejoindre` et `juger_message` ; le saut et la croix suivent `Pad::commandes`, que l'étape tient déjà. La vue du téléphone gagne `Pad::message_libre`, tiré de l'état, pour que la vue change quand les 2,5 s finissent.
- **Le personnage retenu** (§ 6.4) : la manette retient `page::Retenu` (une version, les rangs du look, le prénom ; 77 octets au plus) ; le sélecteur part du prénom de l'app, sinon de celui retenu, et du look retenu ; des octets qui ne sont pas exactement un retenu sont ignorés. **Ce que l'ITP de Safari en fait** : sans rien de retenu, le sélecteur part de zéro, comme un téléphone neuf, et la perte est acceptée, comme le propose la question 23 (`BESOINS-COMMUN.md` du jeu, « Ce que l'ITP de Safari en fait »).

**Les avatars** (`src/avatar.rs`, `build/avatars.rs`, `ressources/art/accessoires.json`) : 55 calques SVG de 32 × 32 cases (`shape-rendering="crispEdges"`), déclarés comme images, écrits par `build.rs` depuis les grilles de l'original (`avatar-pixels.json`, tel quel) et `accessoires.json`, extrait de l'ancien `avatar.rs` par un script (`target/travail/lot-7/sidequest/extraire_accessoires.py`) : les trois tons des vêtements de chaque couleur, les couleurs et les grilles des accessoires, leurs places par vue. La règle des têtes de dos et de profil (l'ancien `head`) est réécrite dans `build/avatars.rs` ; la vue de gauche est le miroir de la droite. Un personnage est une pile : sa base (5 personnages × 4 vues), **trois masques blancs des vêtements teints** des trois tons de sa couleur (`image_teintee`), ses accessoires. Aucune dépendance nouvelle (`serde_json` était déjà la dépendance de construction du jeu). Le module passe de 639 798 à 738 744 octets.

**`pile` et `teinte` sont confirmés** (PHASE-3, § 2.10, écart 4 du lot 6), avec ce que le jeu suppose du rendu : la teinte multiplie chaque canal (blanc × teinte = teinte, alpha gardé) ; les enfants d'une pile partagent sa boîte, le premier dessous ; un bouton dans une pile prend la boîte de la pile, transparent sans `fond` ; les cases restent carrées. Les trois tons d'une couleur ne sont pas multiples l'un de l'autre (le reflet est plus clair que le ton franc) : d'où trois masques, pas un.

Documents du jeu : `BESOINS-COMMUN.md` (le tableau, « Adapté à l'ABI 4 », B11 à B18, « Ce que l'ITP de Safari en fait », « Vérifié le 2026-11-07 »), `README.md` (où en est le jeu, trois écarts au cahier), `AGENTS.md` (la page, les avatars), `ressources/README.md` (ce qui est compilé).

#### Les écarts

1. **Le vocabulaire de l'ABI 4 ne suffit pas à tout le § 6 du cahier** ; chaque manque est demandé au commun dans `BESOINS-COMMUN.md` du jeu, avec ce que la page fait en attendant : l'option d'un `choix` que le joueur n'a pas touché (B12 : le carrousel montre le personnage retenu, mais le `choix` ne peut pas dire qu'il est choisi) ; un choix de deux accessoires où le troisième chasse le plus ancien (B13 : deux `choix`) ; la valeur de départ d'un champ (B14 : le prénom proposé est l'exemple du champ et le geste le porte) ; rejoindre seul avec le personnage retenu (B15 : un appui) ; l'avis d'un refus venu du jeu (B16, comme la demande 1 de Punchline : la page refuse d'avance ce qu'elle peut) ; la croix au clavier et le saut au centre de la croix (B17) ; les animations (B18). Le héros tourne par un `choix` de quatre vues, pas par un bouton.
2. **PHASE-3, § 2.10, dit qu'« aucun jeu n'a besoin d'un état de page à lui »** : c'est vrai du carrousel, pas tout à fait du sélecteur (B12, B13, B14). PHASE-3 n'est pas corrigé ici (la mission de ce lot ne touche pas `soiree/`) : le commun le tranchera avec ces demandes.
3. **E21 n'éprouve pas forcément le sélecteur** : ses 256 vues sont les plus petites empreintes ; le plan standard en voit 43 793 distinctes, et ses pages vont « jusqu'à 17 nœuds » avec des locaux vides, quand le sélecteur en a davantage. Le sélecteur est tenu par les tests du jeu (`page::aucune_page_n_est_vide`, `chaque_page_reste_valide_quels_que_soient_les_locaux`) ; c'est au commun de dire si l'échantillon de E21 doit garder chaque forme de vue.

#### Les tests qui le tiennent

- `tests/page.rs` (nouveau ; `Page` sur une instance qui n'a rien créé, `commun::Telephone`, chaque page passée par `valider_page` et chaque geste, le tenu et sa valeur relâchée par `Valider`) : `une_partie_se_joue_par_les_pages` (le sélecteur, « C'EST MOI ! », la croix qui fait marcher, le saut au contact SQ-07, la pause) ; `un_message_vide_son_champ_et_attend_2_5_s` ; `changer_de_personnage_prend_sa_couleur_par_defaut` ; `le_heros_est_une_pile_de_calques_teints` ; `le_personnage_retenu_revient_au_selecteur` (§ 6.4, le prénom de l'app d'abord) ; `sans_rien_de_retenu_le_selecteur_part_de_zero` (l'ITP, et des octets abîmés) ; `le_retenu_se_relit` ; `la_croix_tient_la_direction_du_cahier` ; `les_commandes_se_desactivent_aux_resultats_et_a_la_fin` ; `le_message_dit_ce_que_la_tele_en_dessinera` (SQ-38, SQ-39) ; `ce_que_la_page_propose_le_jeu_l_accepte` (propriété, 64 cas : invariant 11, SQ-40) ; `chaque_page_reste_valide_quels_que_soient_les_locaux` (propriété, 48 cas, REM-57 : choix hors bornes, clés étrangères, textes de plusieurs kilo-octets, octets retenus quelconques, sur huit vues d'une soirée) ; `une_vue_illisible_ne_donne_pas_de_page` ; `aucune_page_n_est_vide`.
- `tests/avatars.rs` (nouveau) : `les_calques_refont_les_avatars_de_l_original_sq52` (les 440 avatars immobiles de `ressources/art/reference.json` refaits au pixel près par les calques que la page cite, relus depuis les SVG déclarés, teints par multiplication ; et les 44 échantillons RGBA octet par octet ; sans la teinte, aucune empreinte ne tombe juste, contrôle négatif dans le test) ; `les_tons_francs_sont_les_couleurs_de_la_tele` ; `les_tons_des_vetements_sont_ceux_de_l_original` (relus dans l'ancien `avatar.rs`) ; `chaque_calque_declare_sert`.
- `tests/commun/mod.rs` gagne `Telephone`, `Pilote::montre`, `appuyer`, `geste_brut`, `tenir_la_page` et des lecteurs de page ; `tests/regles.rs` écrit `#[expect(dead_code)]` sur le pilote, qu'il n'emploie plus entier.

**Les contrôles négatifs** (`target/travail/lot-7/sidequest/controles.py`, seize mutations de `page.rs`, `avatar.rs` et `build/avatars.rs`, chacune rouge sur le test visé, la source remise telle quelle ensuite) : la clé de la couleur sans le personnage ; la direction tenue et le saut proposé commandes désactivées ; « C'EST MOI ! » sans la règle du prénom ; « Envoyer » pendant les 2,5 s ; une clé de message qui ne change pas d'un message à l'autre ; un retenu d'une autre version relu ; la manette qui ne retient rien ; le prénom de l'app oublié ; les vêtements sans teinte ; le saut au relâchement ; une diagonale de travers ; une couleur par défaut fausse ; les accessoires dans l'autre ordre ; la gauche sans miroir ; une joue de dos oubliée. Le premier passage en a laissé deux vertes (« Envoyer » pendant les 2,5 s, la clé du message figée) : `un_message_vide_son_champ_et_attend_2_5_s` vérifie depuis l'Entrée du champ pendant le délai et le changement de clé au second message, et les seize sont rouges.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7/sidequest-*/` (hors dépôt), avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (le `empreinte.sh` du lot 7). Les preuves finales, lancées à la suite par `target/travail/lot-7/sidequest/final.sh` de 01 h 19 à 01 h 29, ont la même empreinte au départ et à la fin : Sidequest à `6e7219c`, `diff HEAD` hors `.md` `2f424998484a2a11`, non suivis (6) `b6f52aed45f257ae` ; le commun à `c371707` sans changement de code ; Punchline à `dd6d4f0` avec la part non commitée de son agent (hors `.md` `c7588ab241388f21`, non suivis `b10f0d5c3d93dfc4`) ; Tier list à `32f3759`, propre. Seuls des `.md` ont changé après (`BESOINS-COMMUN.md` du jeu, ce journal). Les modules : `target/preuves/lot-7/sidequest-modules.txt`.

| Preuve | Commande | Résultat |
|---|---|---|
| `sidequest-fmt` | dans Sidequest : `cargo fmt --all --check` | code 0 |
| `sidequest-clippy` | `lourd … cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `sidequest-clippy-wasm32` | `lourd … cargo clippy -p sidequest -p sidequest-module --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `sidequest-tests` | `lourd /usr/bin/time -l … cargo test --workspace --no-fail-fast` | **code 0 : 84 réussis**, 0 échec, 0 ignoré ; 11 s ; 0,28 Go résidents |
| `sidequest-module` | `lourd … cargo build -p sidequest-module --target wasm32-unknown-unknown --profile module` | code 0 ; 738 744 octets, sha256 `620435263ed92a99…` |
| `sidequest-controles` | `lourd python3 -I target/travail/lot-7/sidequest/controles.py` | code 0 : les seize mutations rouges |
| `sidequest-construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 (rien à refaire) |
| `sidequest-standard` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/sidequest --json …/sidequest-standard.json` (plan standard, depuis les sources) | **code 0** ; E1 à E19 et E21 réussis, E20 pas exécuté (hors du plan par défaut) ; E21 : 4 394 évaluations, 256 vues sur 43 793 distinctes ; 557 s ; 2,09 Go résidents ; charge 4,15 au départ, 2,96 à la fin |
| `sidequest-court` (avant les preuves finales, même module : même empreinte de module dans les deux rapports) | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/sidequest --court --json …` | **code 0**, la référence du § 7 ; E21 réussi (4 450 évaluations, 256 vues sur 9 530) ; 138 s ; 0,72 Go |

Le plan standard est celui qu'exige la ligne du lot 7 : il est vert pour Sidequest. Le `cargo test --workspace` du commun n'est pas relancé : le commun n'a pas changé.

#### Les mesures

- **Le coût d'une page** (carburant, E19, déterministe ; pas une mesure de temps) : plan standard, 1 024 pages, médiane 72 102 unités, 99ᵉ centile 418 716, maximum 420 164 ; avec le coût fixe de 207 000, 48 % du budget provisoire d'une page (1 302 083). La page vide du lot 6 coûtait au plus 26 749.
- Le module : 738 744 octets (639 798 avant) ; mémoire de 53 à 256 pages.
- Les durées ci-dessus sont celles de commandes sous une charge de 3 à 4,5 : pas des mesures.

#### Ce qui n'est pas fait, et pourquoi

- **La page n'a été vue dans aucun navigateur** : le commun ne dessine pas encore une `PageFil` (lot 12). Que la teinte multiplie, qu'une pile superpose et qu'un bouton en pile prenne sa boîte est ce que le jeu suppose (B11), pas ce qui est constaté.
- **Les demandes B12 à B18** : au commun. La police pixel des titres reste avec celles de la télé (B9).
- La règle F8 de E1 et le refus de la page vide (E21), avec leurs fautifs : au commun, après Tier list. Aucune page de Sidequest n'est vide, et sa touche de diagnostic est `KeyD`.

#### Ce que le jeu suivant et le commun doivent savoir

- `games/sidequest/crates/sidequest/tests/commun/mod.rs`, `Telephone`, appelle `Page` comme le téléphone et valide chaque geste et chaque tenu : il se recopie dans un jeu sans instance de téléphone.
- Une clé qui porte ce qui doit vider un champ ou un choix (le personnage pour la couleur, le dernier message pour le champ du message) remplace, chez Sidequest comme chez Punchline (PUN-04), un état de page : le commun doit dire ce que devient la valeur d'une clé qui disparaît puis revient (Punchline, demande 2).
- Pour armer le refus de la page vide : la page de Sidequest n'est jamais vide, sélecteur compris. L'échantillon de E21 (les 256 plus petites empreintes) peut ne prendre aucune vue de sélecteur (écart 3).
- Le rendu de la page (lot 12) devra tenir B11 : teinte par multiplication, pile qui superpose, cases carrées. Les 55 calques sont de petits SVG ; `Ressources` les rend par clé.

### Tier list

Fait le 2026-11-07, de 01 h 05 à 02 h 10 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo, chaque épreuve et les contrôles négatifs sont passés par le portillon (`target/hote/lourd`) ; seuls un `cargo check -p tier-list` de quelques secondes et des `cargo fmt` s'en sont passés. Aucun navigateur, aucun serveur. Rien n'est commité. **Seul le dépôt de Tier list est modifié** (et ce journal) : le commun ne l'est pas, ni Punchline ni Sidequest (dont les arbres portent encore la part non commitée de leurs agents).

#### Ce qui est fait

Les quatre points de la ligne du lot, et les deux que le lot 6 avait laissés au jeu (le lien nommé, les images non dessinées) :
- **Les boîtes de composants** (`vue.rs`) : chaque composant qui se dessine a sa boîte (`Scene::composant_dans`) — le titre et chaque étiquette à la hauteur de leurs lignes estimées, les deux sélecteurs sur leur bouton, le champ de nom et la zone des réglages sur leur fond, le lien sur son fond, la modale sur le voile. Les six groupes décalés qui les plaçaient sont retirés. Les objets déplaçables restent sans boîte : leurs zones et leurs objets sont des nœuds, chacun avec la sienne. **Le panneau de la modale est la fenêtre** (768 × 330 ; 1 000 px de large pour l'export), plus le voile entier comme au lot 6 : un clic sur le voile la ferme (CAHIER, § 7).
- **Le sélecteur nommé** (`lib.rs`, `sorte`) : de « Restore Backup », un fichier est une restauration quel que soit son type ; de « Add Images », seules les images comptent, un JSON compris ; d'un sélecteur inconnu, rien ; **d'un dépôt (`None`), un seul JSON reste une restauration** (voir les écarts).
- **Une police libre en TTF** : Liberation Sans 2.1.5, romain et gras, sous SIL OFL 1.1 (`contenu/polices/`, avec la licence), déclarée sous `tier-list/liberation-sans` et `tier-list/liberation-sans-grasse` et employée par la page et l'export, en gras là où la feuille de style de l'ancien jeu le met. Choisie parce que ses chasses sont celles d'Arial (le cahier) : `mesure.rs`, qui estime les largeurs avec les chasses d'Helvetica, tombe juste au millième de cadratin sur chaque caractère ASCII, dans les deux graisses. `mesure::LARGE` (ce qui se compte pour tout autre graphème, « pour ne jamais sous-estimer ») monte de 1 250 à 1 338 millièmes, le glyphe le plus large de la police (`hhea.advanceWidthMax`, 2 740 sur 2 048) : « DŽ » (U+01C4), qui se coupe par caractère, en fait 1 333. **Provenance** (sans réseau) : la copie du LibreOfficeDev installé sur la machine (signature de code valide, `codesign -v --deep`) ; texte de la licence repris de pdf.js. La copie de Liberation Sans de pdf.js elle-même est la 1.07.4, sous une autre licence (GPL avec exception) : elle n'est pas prise, et un test refuse toute autre version que la 2.1.5 sous OFL.
- **Une autre touche de diagnostic** : `TOUCHE_DE_DIAGNOSTIC` = **F2** (`lib.rs`). Pas `KeyD` comme les deux autres jeux : Tier list a quatre champs de texte, et le commun ne dit pas encore s'il cède sa touche de diagnostic au champ qui a le focus.
- **« Download Image (PNG) » est un lien** (`ComposantVu::Lien`, `fichier: "tierlist.png"`, la ressource de l'image exportée) ; l'action `TelechargerImage` et le téléchargement de l'image par `Demande::Telecharger` sont retirés, le bot ne les joue plus.
- **Les images non dessinées** (TL-56) : un rendu dont `non_dessinees` n'est pas nul montre l'image et l'annonce (`AvisJeu::ImagesNonDessinees`).

Documents du jeu : `BESOINS-COMMUN.md` (le tableau, § 7, § 8, « Mesures », § 17 et § 18 nouveaux), `AGENTS.md` (le dépôt, les règles, les commandes), `README.md` (le contenu gardé). Le `Cargo.lock` du jeu ne change pas ; aucune dépendance n'est ajoutée.

#### Les écarts

1. **Un JSON lâché sur la page restaure encore** : le cahier (4.5) ignore un fichier déposé qui n'est pas une image, mais le bot de l'épreuve ne sait pas nommer de sélecteur (`ComposantBot::Fichiers`, `ComposantBotFil::Fichiers`) : sans cette règle, aucune restauration ne passerait plus par les soirées de l'épreuve. Demandé au commun (`BESOINS-COMMUN.md` du jeu, § 8) ; le jeu retirera la règle quand le bot dira son sélecteur.
2. **La police n'est pas Arial** (le cahier, § 5 et § 8 : « Le plateau ne charge aucune police ») : aucune licence ne laisse livrer Arial, et le commun refuse une police du système. Liberation Sans en a les chasses ; la forme des glyphes diffère. Pas de CJK dans la police.
3. **Chaque point de reprise grossit de 786 432 octets** (les 825 Kio de polices du segment de données) : plateau neuf 323 584 → 1 110 016 octets hors des zones, pire cas (M3) 17 625 088 → 18 411 520. Demandé au commun : un instantané qui ne garde pas ce que le module n'a jamais écrit (§ 17 du jeu).
4. **La référence du plan court change pour Tier list : code 0, et non plus 1 par E15 seul.** E15 voit 284 244 octets vivants au plus au début de la soirée longue et 273 091 à la fin (référence : de 201 670 à 213 975, en montée). La cause n'est pas isolée : le bot ne joue plus « Download Image » (un tirage de moins, un téléchargement de moins à chaque export prêt), et toute la trajectoire des soirées en change ; E15 au plan court juge une seule soirée longue sans allées et venues, sensible à cette trajectoire (PHASE-3, § 7). Ce code 0 ne prouve donc pas que la mémoire du jeu va mieux. Le plan standard, celui qu'exige la ligne du lot, rend 0 comme avant. **PHASE-3, § 7 (« Le plan court ou le plan standard ») n'est pas corrigé ici** (la mission ne touche pas `soiree/` hors de ce journal) : à relever par le commun, avec Tier list à 0.
5. PHASE-3 n'a pas été trouvé faux sur la part de Tier list ; il n'est pas modifié.

#### Les tests qui le tiennent

98 tests (90 au lot 6), tous verts.
- `tests/partie.rs` : `chaque_composant_qui_se_dessine_a_sa_boite` (les boîtes, et qu'il ne reste de groupes que les rangs, la réserve et la fenêtre) ; `la_modale_a_pour_panneau_la_fenetre` ; `les_fichiers_suivent_leur_selecteur` ; `un_rendu_annonce_les_images_qu_il_n_a_pas_dessinees_tl56` ; `la_touche_de_diagnostic_n_est_pas_f8` (P33) ; `l_export_se_demande_et_s_annule` (le lien, sa boîte, son fichier, sa ressource servie par `Appel::Ressources`) ; `la_scene_est_valide` (avec les polices déclarées, et invalide sans elles).
- `tests/polices.rs` (nouveau) : `les_polices_sont_liberation_sans_sous_ofl` (P22 : sfnt TrueType, répertoire trié, sommes, tables, `loca` et `maxp` ; famille, version 2.1.5, licence OFL 1.1 dans la table `name` ; une police à un octet changé refusée) ; `la_police_a_les_chasses_que_mesure_estime` (chaque caractère ASCII, deux graisses, au millième ; `LARGE` au-dessus de chaque chasse de `hmtx` ; contrôles négatifs dans le test : les chasses de l'autre graisse ne collent pas, 1 250 ne suffisait pas) ; `la_page_et_l_export_ecrivent_dans_leurs_polices`.
- `tests/soiree_fichiers.rs` : son monde nomme le sélecteur comme la page (« Restore Backup » pour une sauvegarde, « Add Images » pour des images, parfois un JSON dans « Add Images », un dépôt une fois sur trois) et un rendu sur quatre manque une à trois images ; quatre chemins de plus doivent être atteints (couverture des six graines : 25 restaurations par « Restore Backup », 22 imports par « Add Images », 13 JSON ignorés dans « Add Images », 7 annonces d'images non dessinées, chacune égale au nombre dit par le rendu).

**Les contrôles négatifs** (`target/travail/lot-7/tier-list/controles.py`, dix-sept mutations de `vue.rs`, `lib.rs`, `mesure.rs` et `export.rs`, chacune rouge sur son test et pour la raison attendue, la source remise telle quelle ensuite) : le panneau de la modale égal au voile ; le titre sans boîte ; une étiquette vide sans hauteur ; « Add Images » lu comme un dépôt ; « Restore Backup » qui ne restaure qu'un JSON ; un sélecteur inconnu lu comme un dépôt ; le sélecteur d'images sous une autre clé dans la scène ; les images non dessinées passées sous silence (dans `partie.rs` et dans les soirées de fichiers) ; la touche de diagnostic F8, puis `KeyD` ; `LARGE` à 1 250 ; une chasse fausse dans la table romaine ; le gras déclaré avec le fichier romain ; la marque en romain ; le titre de l'export en romain ; le lien qui nomme un autre fichier. Au premier passage, deux mutations n'étaient pas justes : « un titre vide sans zone cliquable » restait verte parce que `mesure::nombre_de_lignes` compte déjà au moins une ligne (le `.max(1)` de la vue était redondant ; il est retiré, et la mutation portée sur l'étiquette) ; deux autres rougissaient sur une autre assertion que celle prévue, dont le motif attendu est corrigé.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7/tier-list-*/` (hors dépôt), avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (le `empreinte.sh` du lot 7). Les preuves finales, lancées à la suite par `target/travail/lot-7/tier-list/final.sh` de 01 h 54 à 01 h 59 (les contrôles relancés seuls à 02 h 00, leur motif attendu corrigé), ont toutes la même empreinte au départ et à la fin : Tier list à `32f3759`, `diff HEAD` hors `.md` `65e1ef73b2881392`, non suivis (4 : les deux polices, la licence, `tests/polices.rs`) `ea0eb7ccd3027e53` ; le commun à `c371707` sans changement de code ; Punchline (`dd6d4f0`, hors `.md` `c7588ab241388f21`) et Sidequest (`6e7219c`, hors `.md` `2f424998484a2a11`) avec la part non commitée de leurs agents. Seuls des `.md` ont changé après (`BESOINS-COMMUN.md` et `AGENTS.md` du jeu, ce journal). Les modules et les rapports : `target/preuves/lot-7/tier-list-modules.txt` (module livré 1 615 033 octets, sha256 `84315a1ba93d67fe…` ; empreinte que donnent les deux rapports d'épreuve `44594a80bef6fa17…`).

| Preuve | Commande | Résultat |
|---|---|---|
| `tier-list-fmt` | `lourd bash -c "cd …/tier-list && cargo fmt --all --check"` | code 0 |
| `tier-list-clippy` | `lourd … cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `tier-list-clippy-wasm32` | `lourd … cargo clippy -p tier-list -p tier-list-module --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `tier-list-tests` | `lourd /usr/bin/time -l … cargo test --workspace --no-fail-fast -- --nocapture` | **code 0 : 98 réussis**, 0 échec, 0 ignoré ; 18 s ; 0,42 Go résidents |
| `tier-list-module` | `lourd … cargo build -p tier-list-module --target wasm32-unknown-unknown --profile module` | code 0 ; 1 615 033 octets, sha256 `84315a1b…` |
| `tier-list-controles` | `lourd python3 -I target/travail/lot-7/tier-list/controles.py` | code 0 : les dix-sept mutations rouges (le passage de `final.sh`, code 1, avait une raison attendue mal écrite, corrigée ; voir plus haut) |
| `tier-list-construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 (rien à refaire) |
| `tier-list-standard` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/tier-list --json …/tier-list-standard.json` (plan standard, depuis les sources) | **code 0** ; E1 et E3 à E19 réussis, E2, E10, E11 sans objet, E20 pas exécuté (hors du plan par défaut), E21 sans objet (pas de téléphone) ; 200 s ; 1,07 Go résidents ; charge 3,37 au départ, 1,85 à la fin |
| `tier-list-court` | même commande, `--court` | **code 0** (référence : 1 par E15 seul ; écart 4) ; 41 s ; charge 1,85 au départ, 1,74 à la fin |

Le plan standard est celui qu'exige la ligne du lot 7 : il est vert pour Tier list. Le `cargo test --workspace` du commun n'est pas relancé : le commun n'a pas changé.

#### Les mesures

Des octets et du carburant, déterministes ; pas des mesures de temps.
- **Le module** : 791 068 → 1 615 033 octets (`TAILLE_MODULE_MAX` : 4 Mio).
- **Le point de reprise** (`cargo test --test sauvegarde_lourde le_point -- --nocapture`, depuis `games/tier-list/`) : plateau neuf 323 584 → 1 110 016 octets hors des zones (mémoire 406 → 418 pages) ; pire cas 17 625 088 → 18 411 520 (670 → 682 pages).
- **La vue de l'écran des pires cas** (`cargo test --test budgets -- --nocapture`) : de 8 427 185 à 8 633 962 avant, de 8 553 988 à 8 759 698 après, fixe compris, pour 10 312 500 (+1,5 %). Cause probable, non mesurée à part : chaque texte porte une clé de police de 64 bits au lieu de celle du commun ; `LARGE` n'y est pour rien (mesuré : la même vue avec 1 250).
- **Dans les soirées du plan standard** : une entrée 1 637 437 au plus, une image 4 212 839 fixes compris, `Regarder` 825 149, `Decrire` 10 284, la vue la plus lourde 818 783 ; E15 336 222 → 190 900 octets vivants.
- Les durées du tableau sont celles de commandes sous une charge de 1,7 à 3,8 : pas des mesures.

#### Ce qui n'est pas fait, et pourquoi

- **La page n'a été vue dans aucun navigateur** : l'écran du commun ne dessine pas encore la scène (lots 9 et 11). Les boîtes, le panneau de la modale et le lien sont ce que le jeu demande, pas ce qui est constaté.
- **Les polices ne sont pas comparées à la publication de Liberation** (pas de réseau) ; leur intégrité tient à la signature du LibreOfficeDev d'où elles viennent, et leurs tables à `tests/polices.rs`.
- **La règle du JSON lâché** reste, faute d'un sélecteur dans le coup du bot (écart 1) ; **le monde de l'épreuve ne dit jamais d'image non dessinée** (`epreuve/src/fichiers.rs` affirme `non_dessinees == 0`) : l'avis n'est éprouvé que par les tests du jeu.
- **La référence du plan court** dans PHASE-3, § 7, n'est pas relevée (écart 4).
- La règle F8 de E1 et le refus de la page vide (E21), avec `diagnostic-f8` et `page-vide` : au commun, maintenant que les trois jeux sont passés.

#### Ce que le commun doit savoir

- **Pour armer la règle F8 de E1** : Tier list déclare `F2`, Punchline et Sidequest `KeyD` ; aucun jeu ne déclare plus F8. Tier list n'a pas de téléphone : E21 y reste sans objet, et la page par défaut (vide) n'y est pas une faute.
- **Pour le lot 11** : `partie::regles::images_citees` ne compte pas la ressource d'un `Lien` ; l'écran devra la demander lui-même. Les fenêtres de Tier list sont posées en haut de la page (y = 50), et la scène ne sait pas dire « fixe » : dans une page qui défile, une fenêtre ouverte en bas de page serait hors de vue (`BESOINS-COMMUN.md` du jeu, § 7).
- **Pour le bot de l'écran** : un sélecteur dans `ComposantBot::Fichiers`, et un rendu simulé qui manque parfois des images, donneraient à l'épreuve les chemins que seuls les tests du jeu parcourent (§ 8 du jeu).
- **Pour le point de reprise** (R2, M3 au lot 11) : les assets des trois jeux sont dans le segment de données et dans chaque instantané ; chez Tier list, 768 Kio de plus par point (§ 17 du jeu).
- **Pour la mesure des textes** (lot 8) : la police de Tier list a les chasses d'Arial ; `mise-en-page` mesurera la vraie, et les estimations de `mesure.rs` devraient tomber juste en ASCII.

### Le commun

Fait le 2026-11-07, de 02 h 00 à 02 h 45 environ (GMT), après les trois jeux, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo, chaque épreuve et les contrôles négatifs sont passés par le portillon (`target/hote/lourd`) ; seuls un `cargo check -p epreuve -p fautifs -p moteur-commun` de 11 s et des `cargo fmt` s'en sont passés. Aucun navigateur, aucun serveur. Rien n'est commité. **Seul le commun est modifié** ; les trois jeux portent la part non commitée de leurs agents, que cette part ne touche pas.

#### Ce qui est fait

- **F8 est au commun** (P33 ; REPONSES-PROVISOIRES 14 ; SAL-162) : `moteur_commun::chargement::accord` refuse une déclaration dont `diagnostic` est F8 (« déclaration refusée : diagnostic : F8 est la touche du commun, qui pose la marque au journal ; le jeu en déclare une autre »). Le chargement est partagé par wasmtime et le navigateur (`verifier_chargement`) : les deux refusent pareil, et l'épreuve rend E1 en échec, rien d'autre ne tournant (`eprouver_octets`, comme `declaration-incoherente`). La comparaison se fait sur le texte (`CodeTouche::as_str`) : un `CodeTouche` relu du fil est toujours reconstruit par `essayer`.
- **E21 refuse la page vide** (`epreuve/src/pages.rs`) : toute page valide sans aucun nœud, sous des locaux vides comme tirés au hasard, chez un jeu qui a des téléphones. Une seule évaluation pour la règle, sur toutes les pages valides (`juger_les_vides`) : si chacune est vide, « la page est celle par défaut de Jeu::page, vide : aucun nœud sur chacune des N pages… » ; sinon, le nombre de pages vides et la vue de la première. La note de E21 dit « n page(s) vide(s) sur m valide(s) ».
- **Deux fautifs** : `diagnostic-f8` (`Defaut::DECLARATION` avec `diagnostic: CodeTouche::F8`) et `page-vide` (`Defaut::page_par_defaut`, une méthode de plus du socle : la page du socle ne fait alors rien, comme la méthode par défaut de `Jeu::page`). Chacun a sa crate de module (`exemples/fautifs/modules/<nom>/`), son entrée dans l'exemple `natif`, dans `tests/epreuve/modules.rs` (40 jeux) et dans `plan_standard.rs`, et son test.
- Documents : PHASE-3 (ligne du lot 7, § 7 « Le plan court ou le plan standard » : la référence relevée à nouveau, § 8.3 : E21 et le tableau des contrôles négatifs), CHANGELOG (« Phase 3, lot 7 »), `crates/README.md`, `AGENTS.md` (40 jeux au plan standard des fautifs).

#### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **E21 refuse toute page sans nœud**, pas seulement la page par défaut : une vue oubliée laisse le téléphone vide comme la page par défaut (REM-38). Les pages de Punchline et de Sidequest ne sont jamais vides (leurs tests `aucune_page_n_est_vide` et les rapports : 0 page vide sur 1 024).
2. **La règle F8 est au chargement, pas dans `jeu::module!`** : une vérification en `const` empêcherait le fautif de compiler, et un module écrit sans `jeu` doit être refusé de même.
3. **La référence du plan court change pour Tier list : 0, et non plus 1 par E15 seul** (écart 4 de la part de Tier list), relevée dans PHASE-3, § 7, sur le commun final (`commun-court-tier-list`). Sidequest 0 et Punchline 1 par E2 seul ne changent pas.

#### Les tests qui le tiennent

- `epreuve`, `tests/epreuve` : `diagnostic_f8_echoue_e1` (E1 seul, le détail nomme « diagnostic » et « F8 ») ; `page_vide_echoue_e21` (E21 seul, le détail nomme la page par défaut) ; chaque fautif d'avant, `devine_passe_tout` et `le_socle_sain_passe_tout` restent verts.
- `epreuve`, `pages::tests::la_page_vide_se_dit_par_defaut_ou_par_vue` : aucune page vide, toutes, une parmi douze.
- **Les contrôles négatifs** (`target/travail/lot-7/commun/controles.py`, trois mutations, chacune rouge pour la raison attendue, la source remise telle quelle ensuite) : F8 plus refusé au chargement (`diagnostic_f8_echoue_e1` : aucun échec au lieu de E1) ; la page par défaut plus refusée (`page_vide_echoue_e21` : aucun échec au lieu de E21) ; une page vide parmi d'autres acceptée (le test de `pages`). Les deux fautifs sont eux-mêmes les contrôles négatifs des deux règles.
- Au premier passage, `page_vide_echoue_e21` cherchait « page par défaut » dans un détail qui dit « celle par défaut de Jeu::page » : E21 échouait bien, et lui seul ; le texte attendu est corrigé (« la page est celle par défaut de Jeu::page »), rien n'est affaibli.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7/commun-*/` (hors dépôt), avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (le `empreinte.sh` du lot 7). Les preuves finales, lancées à la suite par `target/travail/lot-7/commun/final.sh` de 02 h 13 à 02 h 42 (les contrôles juste avant, 02 h 12), ont la même empreinte du code au départ et à la fin : le commun à `c371707`, `diff HEAD` hors `.md` `172d3fe24ad94d51`, non suivis (4 : les deux crates de module) `5f2ddfdcb35a3dbe` ; Punchline (`dd6d4f0`, hors `.md` `c7588ab241388f21`, non suivis `b10f0d5c3d93dfc4`), Sidequest (`6e7219c`, `2f424998484a2a11`, `b6f52aed45f257ae`) et Tier list (`32f3759`, `65e1ef73b2881392`, `ea0eb7ccd3027e53`), les empreintes mêmes que leurs agents ont rendues. Seuls des `.md` du commun ont changé pendant et après (PHASE-3, ce journal). Les modules et les rapports : `target/preuves/lot-7/commun-modules.txt`.

| Preuve | Commande | Résultat |
|---|---|---|
| `commun-controles` | `lourd python3 -I target/travail/lot-7/commun/controles.py` | code 0 : les trois mutations rouges pour la raison attendue |
| `commun-fmt` | `cargo fmt --all --check` | code 0 |
| `commun-clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `commun-clippy-wasm32` | `lourd cargo clippy -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `commun-clippy-web` | dans `web/` : `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, puis pour `wasm32-unknown-unknown` | code 0 (`moteur-navigateur` recompilé sur le nouveau `moteur-commun`) |
| `commun-construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 |
| `commun-standard-diagnostic-f8` | `lourd /usr/bin/time -l target/release/soiree-epreuve crates/exemples/fautifs/modules/diagnostic-f8 --json …` (plan standard, depuis ses sources) | **code 1, E1 seul** (« le moteur refuse le module : déclaration refusée : diagnostic : F8… ») ; étape 1 faite ; 20 s ; module `bc47ad7e…` |
| `commun-standard-page-vide` | même commande, `…/page-vide` | **code 1, E21 seul** (« la page est celle par défaut de Jeu::page, vide : aucun nœud sur chacune des 1024 pages… » ; 2 799 vues distinctes) ; étape 1 faite ; 58 s ; module `3f35f2ac…` |
| `commun-standard-punchline` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/punchline --json …` (plan standard, depuis les sources) | **code 0** ; E21 réussi (2 447 évaluations, 0 page vide sur 1 024) ; 105 s ; 1,62 Go résidents ; module `82669f77…` (rapport `46ebf4b5…`, celui de son agent) |
| `commun-standard-sidequest` | même commande, `../games/sidequest` | **code 0** ; E21 réussi (4 395 évaluations, 0 page vide sur 1 024) ; 550 s ; 2,14 Go ; module `62043526…` (rapport `5d86f589…`) |
| `commun-standard-tier-list` | même commande, `../games/tier-list` | **code 0** ; E21 sans objet ; 205 s ; 1,13 Go ; module `84315a1b…` (rapport `44594a80…`) ; le rapport JSON a le sha256 même de celui de son agent (`1fc69160…`) |
| `commun-court-punchline` | même commande, `--court` | code 1, **E2 seul** (« une fin, mais aucune autre après la nouvelle partie »), la référence ; 24 s |
| `commun-court-sidequest` | même commande, `--court` | **code 0**, la référence ; 129 s |
| `commun-court-tier-list` | même commande, `--court` | **code 0**, la référence relevée au lot 7 (E15 : 284 244 au début, 273 091 à la fin) ; 41 s ; rapport JSON au sha256 de celui de son agent (`cb84457e…`) |
| `commun-tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 646 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 541 s ; 1,63 Go résidents ; charge 1,66 au départ, 2,32 à la fin |

Charge au départ et à la fin de chaque épreuve, écrite par le portillon : de 1,36 à 2,72 sur 10 cœurs, mémoire libre de 40 à 48 %. Le plan standard des 40 fautifs (`--ignored plan_standard`) n'est pas relancé : la ligne du lot ne le demande pas, et les deux fautifs nouveaux sont joués au plan standard depuis leurs sources ci-dessus. E20 n'est pas relancé : ni le relevé ni le banc ne changent, et un module refusé au chargement l'est par le même code dans le navigateur.

#### Les mesures

- Aucune mesure de temps : les durées ci-dessus sont celles de commandes sous une charge de 1,4 à 2,7, pas des mesures.
- Le carburant d'une page (E19, déterministe) ne change pas : Punchline médiane 70 317, maximum 846 390 ; Sidequest médiane 72 102, maximum 420 164 — les nombres mêmes que leurs agents ont relevés, le même module et les mêmes soirées.
- Mémoire : de 0,31 Go (Tier list, plan court) à 2,14 Go résidents (Sidequest, plan standard) par épreuve ; 1,63 Go pour `cargo test --workspace`. Le § 7 attendait « 2 à 3 Go » (estimé).

#### Ce qui n'est pas fait, et pourquoi

- **Les demandes des jeux au commun** (Punchline, `BESOINS-COMMUN.md` § 11 ; Sidequest, B12 à B18 ; Tier list, § 7, § 8, § 17, § 18) ne sont pas reportées dans `BESOINS-DES-JEUX.md` ni tranchées : ce n'est pas la ligne de ce lot. Elles sont écrites dans les dépôts des jeux.
- **PHASE-3, § 2.10, « aucun jeu n'a besoin d'un état de page à lui »** (écart 2 de Sidequest) et la sémantique des champs dans les locaux (demande 2 de Punchline) ne sont pas tranchés ici.
- **L'échantillon de E21** (les 256 plus petites empreintes) peut ne prendre aucune vue de sélecteur de Sidequest (écart 3 de Sidequest) : non changé, pour ne pas changer ce que E21 éprouve dans ce lot ; ses pages vont jusqu'à 17 nœuds dans les trois rapports.
- **Le bot de l'écran ne nomme pas de sélecteur** de fichiers, et le monde ne simule pas d'image non dessinée (Tier list, écart 1 et TL-56) : non fait.
- Aucun navigateur : la page d'aucun jeu n'est encore dessinée (lot 12).

#### Ce que le lot suivant doit savoir

- Un jeu qui déclare F8 est refusé au chargement, dans wasmtime comme dans le navigateur ; un jeu à téléphones dont une page est vide échoue E21. Les trois jeux passent les deux règles (Punchline et Sidequest `KeyD`, Tier list `F2`).
- Le socle des fautifs a `Defaut::page_par_defaut` ; les fautifs sont 40 dans `tests/epreuve/modules.rs`.
- La référence du plan court est maintenant : Sidequest 0, Punchline 1 par E2 seul, Tier list 0 (PHASE-3, § 7). Le lot 7b et les suivants la comparent à celle-ci.
- Les rapports d'épreuve de Tier list ont le même sha256 d'une exécution à l'autre sur le même module et le même commun : une épreuve refaite se compare octet pour octet.

## Lot 8 — La mesure des textes

Fait le 2026-11-07, de 02 h 45 à 04 h 05 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1). Chaque commande cargo, chaque épreuve, les contrôles négatifs et la mesure sont passés par le portillon (`target/hote/lourd`) ; seuls des `cargo check -p …` de quelques secondes, des `cargo fmt` et trois essais de E1 et E16 seuls (moins d'une seconde chacun, sur les modules du lot 7, `target/travail/lot-8/explo-*`) s'en sont passés. Aucun navigateur, aucun serveur. Rien n'est commité. **Seul le commun est modifié** ; voir plus bas le `Cargo.lock` de Tier list.

### Ce qui est fait

**`crates/mise-en-page`** (nouvelle, la sixième crate de la phase 3 ; `jeu-abi` seulement dans le commun) :
- `Polices` : la police du commun d'abord, puis celles du jeu, dans l'ordre où elles s'ajoutent (celui de la déclaration, que la télé suivra). **La police du commun est Liberation Sans 2.1.5**, romain, sous SIL OFL 1.1 (`polices/LiberationSans-Regular.ttf` et `polices/OFL-Liberation.txt`), la copie que Tier list a vérifiée au lot 7 (son `BESOINS-COMMUN.md`, § 17 : LibreOfficeDev signé, table `name` « Version 2.1.5 » et OFL), au même sha256 (`76d04c18…`, et `93fed460…` pour la licence). `valider_police` : un fichier sfnt (`00 01 00 00`, `true`, `OTTO`), exactement une face que `fontdb` lit ; refus nommés pour `wOFF`, `wOF2`, `ttcf`, une autre signature, des tables illisibles, et une police homonyme (même famille, graisse, style et chasse) aux octets différents.
- `MiseEnPage::agencer(texte, boite, style) -> Agencement` : le texte coupé (une ligne par `\n`, ce que la télé donnera à `glyphon`), la taille retenue, l'interligne, les lignes (bord gauche, largeur, haut) et leurs glyphes (police, numéro, x, chasse), `Tient::Oui` ou `Tient::Non(Debord)` (`TropLarge`, `TropHaut`, `TropDeLignes`, `StyleInvalide`), les glyphes absents, la police inconnue. Gardé par (texte, police, taille, alignement, lignes, coupe, taille de la boîte), jusqu'à 4 096 agencements.
- **La règle** (en tête de `agencement.rs`, et PHASE-3, § 2.9, *écrit au lot 8*) : la taille est celle du cadratin ; une ligne fait 1,2 fois la taille (`INTERLIGNE`, choisi) ; fins de ligne forcées (`\n`, `\r`, U+2028, U+2029…) ; `Coupe::Mots` aux occasions de UAX #14 (`unicode-linebreak`), un mot trop large restant entier et débordant ; `Coupe::Graphemes` entre deux graphèmes (`unicode-segmentation`) ; au plus tard possible ; espaces de fin de ligne retirées et non comptées ; SQ-46 (U+0020 ou U+00A0 devant `?!:;` devient U+202F, en `Coupe::Mots`) ; `Taille::Budget` par dichotomie au 1/256 ; alignement dans la largeur, bloc centré dans la hauteur ; polices de repli indépendantes de la plateforme (celle du commun, puis toute police connue), langue fixée (`fr-FR`), aucune police du système.
- **Pourquoi la coupe est au commun** : essayé d'abord sur `cosmic-text` 0.19, `Wrap::Glyph` met sur deux lignes les glyphes d'un même graphème (un emoji et sa couleur de peau, absents de la police), et `LayoutRun::line_w` compte les espaces de fin de ligne. Le commun coupe donc, puis façonne sans coupe automatique.

**L'épreuve** :
- `epreuve/src/polices.rs` : chaque police déclarée lue par `Ressources` sur une instance créée (la colle refuse `Ressources` avant `Creer`), comparée à sa taille déclarée, puis validée. Lue une fois par épreuve (`Epreuve::polices`).
- **E1** juge chaque police (une évaluation chacune) et note les polices admises, leur famille et leur taille ; une police refusée est un échec qui dit pourquoi et quoi faire.
- **E16** mesure chaque nœud `Texte` de la scène de chaque pire cas valide (`pires_cas::mesurer`), sous `catch_unwind` (une panique de la mise en page serait une panne de l'outil). Un débordement nomme le pire cas, le texte (60 caractères), la clé, la boîte, la taille où il a été mesuré et la raison. La note dit les textes mesurés, les débordements, les glyphes absents, et les textes dans une police inconnue. Ni les noms des boutons, ni les comptes à rebours, ni la valeur d'un `EnPlace` ne sont mesurés (PHASE-3, § 2.9, *écrit au lot 8*).

**Deux fautifs** (40 → 42 jeux dans `tests/epreuve/modules.rs`, l'exemple `natif`, `plan_standard.rs`) : `police-woff2` (`Defaut::DECLARATION` avec un asset police de 48 octets, un en-tête WOFF2 ; aucun texte ne s'en sert) et `texte-qui-deborde` (`Defaut::texte_du_record`, une méthode de plus du socle : à 9 999 points, donc dans les pires cas seuls, une phrase de 127 caractères sur une ligne de 16 × 1).

**Les exemples du commun** : le style du socle des fautifs et celui d'`ardoise` prennent une plus petite taille d'une demi-unité (le prénom de douze « W » du socle tient dans ses six unités, le chiffre d'`ardoise` dans sa case d'une unité, une ligne faisant 1,2 fois la taille). `devine` tient sans changer. Leurs modules changent d'empreinte.

**Dépendances** (`Cargo.toml`, chacune avec sa raison) : `cosmic-text` 0.19.0 (la dernière, 2026-04-22 ; `glyphon` 0.12 la demande) sans `swash` ni `fontconfig` ; `unicode-linebreak` 0.1.5 (celle qu'elle tire) ; `unicode-script` 0.5.8, tenue à 0.5.8 dans le `Cargo.lock` parce que 0.5.9 date du 2026-11-02 (moins de deux semaines), comme `yazi` tenue à 0.2.1 (0.2.2 du 2026-11-01, que `swash` tirerait). Les autres crates entrées au `Cargo.lock` ont plus de deux semaines (vérifié sur crates.io : la plus jeune, `memmap2` 0.9.12, du 2026-10-21). `mise-en-page` se compile aussi pour `wasm32-unknown-unknown`, clippy propre.

Documents : PHASE-3 (§ 2.9 *écrit au lot 8* et *corrigé au lot 8*, § 7 ligne du lot et référence du plan court, § 8.3, question 24, les deux annexes), MESURES-PHASE-3 (« Lot 8 »), CHANGELOG (« Phase 3, lot 8 »), `crates/README.md` (la carte, le graphe, « Constaté pour les crates à venir »), `AGENTS.md` (la mesure d'un texte a une seule définition ; 42 jeux).

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **La police d'emoji n'est pas livrée** (§ 2.9) : aucune police d'emoji libre n'est sur la machine, et le lot n'a rien téléchargé (un téléchargement demande l'accord de l'hôte). Un emoji se mesure comme une boîte, compté. *Corrigé après la vérification : ce n'est pas un écart, PHASE-3 avait raison ; c'est un travail non fait, rendu à PHASE-3 comme tel (§ 2.9, question 28). Voir « Lot 8, correction après la vérification ».*
2. **La coupe est celle du commun, pas celle de `cosmic-text`** (§ 2.9) : voir plus haut. La télé donnera à `glyphon` le texte agencé.
3. **Le cache ne prend ni la clé du nœud ni la couleur** (§ 2.9 disait « par (clé, texte, style, boîte) ») : l'agencement n'en dépend pas.
4. **E1 lit les polices dans l'épreuve, pas au chargement du moteur** : leurs octets ne sont pas dans la déclaration, et `Ressources` demande une partie créée. La télé appellera `valider_police` en chargeant les polices.

### Les tests qui le tiennent

- `mise-en-page`, `tests/mesure.rs` (13 tests) : `le_signe_isole_reste_avec_son_mot_sq46` et, sous proptest, `aucune_ligne_ne_commence_par_un_signe_isole_sq46` (SQ-46) ; `un_texte_trop_long_retrecit_et_rien_n_est_perdu_tl55` (la police rétrécit, la ligne grandit, chaque lettre est là, et 1/256 de plus ne tient pas) et `une_ligne_ne_coupe_jamais_un_grapheme_tl55` (TL-55) ; sous proptest, `l_agencement_tient_dans_sa_boite_ou_le_dit_pun53` (128 cas : un texte qui tient a chaque glyphe et chaque ligne dans sa boîte, un débordement dit sa vraie raison, la taille reste entre ses bornes, et une mise en page neuve rend le même agencement) (PUN-53) ; `la_police_du_commun_se_lit`, `seules_les_polices_sfnt_sont_admises_p22`, `deux_polices_homonymes_aux_octets_differents_sont_refusees`, `un_texte_qui_ne_tient_pas_meme_au_min_deborde`, `les_fins_de_ligne_et_les_lignes_permises`, `un_style_qui_ne_se_mesure_pas_le_dit`, `un_glyphe_absent_se_compte_et_aucune_police_du_systeme_ne_sert` (la machine a des polices CJK, la mesure ne les voit pas), `l_alignement_place_les_lignes_dans_la_boite`. `tests/mesure.proptest-regressions` garde le cas que la mutation PUN-53 a trouvé (un « ! » au bord droit d'une boîte d'une unité), rejoué à chaque fois.
- `epreuve`, `tests/epreuve` : `police_woff2_echoue_e1` (E1 seul, le détail nomme WOFF2 et « TTF ou OTF ») ; `texte_qui_deborde_echoue_e16` (E16 seul, le détail nomme le texte, la boîte de 16 × 1 et la plus petite taille) ; `le_socle_sain_passe_tout` vérifie de plus que E16 a mesuré des textes et qu'aucun ne déborde ; chaque fautif d'avant (40), `devine_passe_tout` et `un_jeu_sans_telephone_passe_tout_sans_pannes_de_telephone` restent verts.
- **Les contrôles négatifs** (`target/travail/lot-8/controles.py`, huit mutations, chacune rouge pour la raison attendue, la source remise telle quelle ensuite) : E1 ne juge plus les polices refusées ; la signature WOFF2 n'est plus reconnue ; E16 ne mesure plus ; une ligne trop large n'est plus un débordement ; SQ-46 retiré ; `Coupe::Graphemes` entre deux points de code ; `Taille::Budget` arrêtée au `min` ; les glyphes déplacés hors de la boîte. Au premier passage (`target/travail/lot-8/controles-premier-passage/`), la quatrième était rouge, mais le script attendait le mot « TropLarge » là où le test, qui n'en voit plus, montre `Non(TropHaut…)` : seul le texte attendu par le script est corrigé, rien n'est affaibli.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-8/*/` (hors dépôt), avec `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (`target/travail/lot-8/empreinte.sh`). Les preuves finales, lancées à la suite par `target/travail/lot-8/final.sh` de 03 h 17 à 03 h 48, puis les contrôles refaits (de 03 h 48 min 54 s à 03 h 50 min 00 s) et la mesure (de 03 h 50 min 43 s à 03 h 50 min 50 s ; heures corrigées après la vérification, d'après les `code.txt` des preuves), ont toutes la même empreinte du commun : `b146dc5`, `diff HEAD` hors `.md` `0d88adbdd6538e60`, non suivis (13 : la crate `mise-en-page`, `epreuve/src/polices.rs`, les deux crates de module, le fichier de régressions) `1d9dd7808dac254c` ; Punchline (`f84b86f`) et Sidequest (`229199b`) sans aucune modification. **Le `Cargo.lock` de Tier list** : l'épreuve depuis ses sources (`standard-tier-list`) l'a réécrit, Tier list dépendant d'`epreuve`, qui tire maintenant `cosmic-text` (`diff HEAD` `b679c8f73cb243fe` dans les onze empreintes prises depuis) ; il est remis à son état commité après les preuves (`git checkout -- Cargo.lock`, la différence gardée dans `target/travail/lot-8/tier-list-cargo-lock.diff`) : il se réécrira à la prochaine compilation de Tier list, et son agent le commitera. Seuls des `.md` du commun ont changé pendant et après. Les rapports JSON sont à côté (`standard-*.json`, `court-*.json`).

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-8/controles.py` | code 0 : les huit mutations rouges pour la raison attendue ; 65 s |
| `fmt` | `cargo fmt --all --check` | code 0 |
| `clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p mise-en-page -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 |
| `standard-police-woff2` | `lourd /usr/bin/time -l target/release/soiree-epreuve crates/exemples/fautifs/modules/police-woff2 --json …` (plan standard, depuis ses sources) | **code 1, E1 seul** (« la police 1f12703cef8bbd15 (48 octets) est refusée : une police WOFF2… convertissez-la ») ; 86 s ; 0,87 Go |
| `standard-texte-qui-deborde` | même commande, `…/texte-qui-deborde` | **code 1, E16 seul** (« le texte « Record absolu… » ne tient pas dans sa boîte de 16.00 × 1.00, même à sa plus petite taille, 0.50 : une ligne de 27.14 unités de large ») ; 60 s |
| `standard-punchline` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/punchline --json …` | **code 1, E16 seul** : 142 textes mesurés, 9 débordent, 12 glyphes absents ; E1 admet ses trois polices TTF ; 130 s ; 1,89 Go ; module `82669f77…`, celui du lot 7 |
| `standard-sidequest` | même commande, `../games/sidequest` | **code 1, E16 seul** : 426 textes, 4 débordent, 1 glyphe absent ; aucune police déclarée ; 575 s ; 2,32 Go ; module `62043526…`, celui du lot 7 |
| `standard-tier-list` | même commande, `../games/tier-list` | **code 1, E16 seul** : 1 151 textes, 4 débordent, 10 080 glyphes absents (les idéogrammes de ses pires cas) ; E1 admet ses deux polices ; 231 s ; 1,08 Go ; module `84315a1b…`, celui du lot 7 |
| `court-punchline` | même commande, `--court` | code 1, E2 (même constat qu'avant) et E16 ; 24 s |
| `court-sidequest` | même commande, `--court` | code 1, E16 seul ; 127 s |
| `court-tier-list` | même commande, `--court` | code 1, E16 seul ; 42 s |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 661 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 551 s ; 1,75 Go |
| `mesure-e16` | `lourd --calme target/travail/lot-8/mesure-e16.sh` | code 0 ; voir les mesures |

Comparés aux rapports du lot 7 (mêmes modules, même commun hors lot 8), les trois rapports du plan standard ne diffèrent qu'en E1 (la note des polices), E16, les étapes et le code de sortie (et, pour Sidequest, la compilation). Charge au départ et à la fin de chaque épreuve, écrite par le portillon : de 1,56 à 4,66 sur 10 cœurs, mémoire libre de 41 à 52 %. Le plan standard des 42 fautifs (`--ignored plan_standard`) n'est pas relancé : la ligne du lot ne le demande pas, les deux fautifs nouveaux sont joués au plan standard depuis leurs sources, et les pires cas, que E16 mesure, ne dépendent pas du plan (le plan court de chaque fautif, dans `cargo test --workspace`, les mesure tous). E20 n'est pas relancé : ni le relevé, ni le banc, ni les modules des jeux ne changent.

### Les débordements des trois jeux, pour leurs agents (lot 7b)

La liste entière (le rapport n'en détaille que huit) vient de `target/travail/lot-8/liste-debords` (un outil de travail hors dépôt, qui rejoue les pires cas et mesure comme E16), sortie dans `target/travail/lot-8/debords-des-jeux.txt`. Une ligne fait 1,2 fois la taille ; un mot plus large que sa boîte ne se coupe qu'en `Coupe::Graphemes`.
- **Punchline** (9) : le mot de 80 « W » (clés `fe7c3a9b…`, `fe7c399b…`, `fe7c389b…`, Schoolbell, `Budget` de 56 à 18, `AuPlus(3)`, `Coupe::Mots`), pires cas 4 à 7, dans des boîtes de 600 × 280 et 600 × 93 : à 18, une ligne de 1 192,5 unités ; et, au pire cas 9, « Égalité : WWWWWWWWWWW0, … » (122 caractères, clé `e4f9d304…`, Rokkitt SemiBold, `Budget` de 80 à 26, `Lignes::Une`, `Coupe::Mots`) dans 1 200 × 90 : 2 509,5 unités.
- **Sidequest** (4) : 180 « W » (clé `d505e5cf…`, police du commun, `Budget` de 63 à 14, `Libre`, `Coupe::Mots`), pires cas 2 à 4, boîte 704 × 126 : 2 378,5 unités ; et « « WWW… » (246 caractères, clé `8effa846…`, `Budget` de 20 à 8, `AuPlus(3)`, `Coupe::Mots`), pire cas 5, boîte 840 × 90 : 1 359,1 unités.
- **Tier list** (4) : le titre « TIER LIST » (clé `3341e08e…`, `Taille::Fixe(28)`, `Lignes::Une`) dans une boîte de 1 100 × 28, pires cas 0 à 3 : une ligne de 33,6 unités de haut. Une conséquence de l'interligne de 1,2 choisi (PHASE-3, annexe « Les valeurs choisies ») : la boîte doit faire 34, ou la taille passer en `Budget`.

Aucun de ces jeux n'est modifié par ce lot : les débordements se corrigent au lot 7b, par leurs agents. Ils ne sont pas reportés dans les `BESOINS-COMMUN.md` des jeux (le dépôt d'un jeu est à son agent) ; ils sont ici et au CHANGELOG.

### Les mesures

- **La durée de E16** (question 24), machine calme (charge 1,91 au départ et à la fin), cinq essais par jeu, le processus entier : Punchline 0,44 à 0,48 s, Sidequest 0,38 à 0,39 s, Tier list 0,46 à 0,47 s (`MESURES-PHASE-3.md`, « Lot 8 ») : E16 reste un contrôle rapide.
- Mémoire : 0,71 Go au plus pour un processus pour compiler et tester `mise-en-page` à froid ; 1,75 Go pour `cargo test --workspace` ; de 0,33 Go (Tier list, plan court) à 2,32 Go (Sidequest, plan standard) par épreuve. Le § 7 attendait « 2 à 3 Go » (estimé).
- Les autres durées ci-dessus sont celles de commandes sous une charge de 1,6 à 4,7 : pas des mesures.

### Ce qui n'est pas fait, et pourquoi

- **La police d'emoji monochrome** : non livrée (écart 1). Elle s'ajoutera à `polices/` et à la liste des polices de repli (`agencement::Repli`) quand l'hôte en fournira une, libre, avec sa licence.
- **Les textes des boutons, des comptes à rebours et des `EnPlace`** ne sont pas mesurés : leur style ou leur texte ne se connaît qu'au lot 9 (le rendu) ou en jeu.
- **Le point 7 de RESTE-PHASE-2** (E16 et les limites propres au jeu) reste non réglé : une décision d'API.
- **Les débordements des trois jeux** ne sont pas corrigés : c'est le lot 7b, par leurs agents.
- Aucun navigateur : la mesure n'est pas encore comparée au dessin (lot 9, « agencement identique au natif »).

### Ce que le lot suivant doit savoir

- **Le rendu (lot 9)** dessine `Agencement::texte` (le texte coupé, une ligne par `\n`) avec `Wrap::None`, la taille et l'interligne de l'agencement, la police du style et les mêmes polices de repli (`mise_en_page` les ajoute dans l'ordre de la déclaration, celle du commun d'abord) : il ne coupe jamais lui-même, et ne donne jamais à `glyphon` le texte de la scène. Les positions des glyphes sont dans `LigneAgencee` pour la comparaison glyphe par glyphe. Le bloc des lignes se centre dans la hauteur de la boîte ; le placement de la ligne de base dans sa ligne est celui de `cosmic-text`.
- La police du commun est `mise_en_page::OCTETS_DU_COMMUN` ; `valider_police` est la règle de E1, que la télé appelle en chargeant les polices d'un jeu. Les flottants de `mise-en-page` évitent `f32::min` et `f32::max` (le zéro signé, `clippy.toml`).
- Lire une police de 410 Kio par `Ressources` coûterait de l'ordre de 17 millions d'unités (estimé, non mesuré : les 42 unités par octet que Tier list a mesurées pour un `&[u8]` encodé octet par octet) : sous le plafond d'un appel (2²⁸), au-dessus du budget d'une lecture courante ; la télé les lit au chargement, pas dans une image.
- La référence du plan court est maintenant : Sidequest 1 par E16 seul, Punchline 1 par E2 et E16, Tier list 1 par E16 seul (PHASE-3, § 7). Le lot 7b la ramène à celle du lot 7 (0, 1 par E2, 0).
- Le socle des fautifs a `Defaut::texte_du_record` ; les fautifs sont 42.
- Une épreuve depuis les sources de Tier list réécrit son `Cargo.lock` (il prend `cosmic-text` par `epreuve`) ; son agent le commitera, en tenant `unicode-script` à 0.5.8 et `yazi` à 0.2.1 si elles ont toujours moins de deux semaines.

## Lot 8, correction après la vérification

Fait le 2026-11-07, de 04 h 00 à 04 h 45 environ (GMT), sur la même machine. Chaque commande cargo, les contrôles négatifs et chaque épreuve sont passés par le portillon ; aucun navigateur, aucun serveur. Rien n'est commité. Seul le commun est modifié ; Punchline, Sidequest et Tier list sont restés propres (aucune compilation d'un jeu depuis ses sources : le `Cargo.lock` de Tier list n'a pas bougé).

### Les constats, et ce qui en est fait

1. **[important] La police d'emoji n'était pas un écart, mais un travail non fait.** Vérifié, exact : PHASE-3 avait raison, et le lot avait réécrit le § 2.9 comme si le plan était faux. **Corrigé** : le § 2.9 redit que la police d'emoji monochrome est demandée, la dit *pas faite au lot 8* (aucune police libre sur la machine, aucun téléchargement sans l'accord de l'hôte), et la donne à la **question 28** du § 9.1, nouvelle : l'hôte fournit-il une police libre, ou en autorise-t-il le téléchargement ? Réponse provisoire : non tant qu'il ne l'a pas fait ; un emoji se mesure et se dessine comme une boîte, compté ; dès sa décision, la police entre dans `crates/mise-en-page/polices/` et dans les polices de repli, au plus tard au lot 9. L'écart 1 du lot 8 est annoté, le CHANGELOG renvoie à la question 28. Elle reste dans « non fait ».
2. **[important] Le `Cargo.lock` commité de Tier list est périmé.** Vérifié, exact, et **le commun ne peut pas le corriger** : le dépôt est à l'agent du jeu. Une précision de plus : la différence gardée (`target/travail/lot-8/tier-list-cargo-lock.diff`, 27 paquets nouveaux) prend `unicode-script` **0.5.9** et `yazi` **0.2.2**, les deux versions de moins de deux semaines que le commun tient à l'écart ; elle ne se committe donc pas telle quelle. Le CHANGELOG (« Phase 3, lot 8 », « Passer à cette version », point 3, nouveau) le dit à l'agent du jeu : committer le nouveau lock avant toute compilation `--locked`, en tenant `unicode-script` à 0.5.8 et `yazi` à 0.2.1 (par exemple `cargo update -p unicode-script --precise 0.5.8` et `cargo update -p yazi --precise 0.2.1`).
3. **[mineur] La garde de `le_socle_sain_passe_tout` ne voyait pas un compte nul.** Vérifié, exact : les notes sont jointes par `\n`, et `" 0 texte(s) mesuré(s)"` ne pouvait pas apparaître. **Corrigé** (`crates/epreuve/tests/epreuve/main.rs`) : le test lit le compte au début de sa ligne, le convertit en nombre et exige qu'il soit positif (« E16 n'a mesuré aucun texte »). **Contrôle négatif** ajouté (`target/travail/lot-8-correction/controles.py`, la neuvième mutation) : E16 ne mesure plus aucun texte, et `le_socle_sain_passe_tout` rougit sur ce message. La première assertion du test (le libellé est là, « 0 débordent ») passe toujours sous la mutation : seule la nouvelle garde la voit.
4. **[mineur] « testé » sans test du sha256.** Vérifié, exact. **Corrigé** : `la_police_du_commun_se_lit` (`crates/mise-en-page/tests/mesure.rs`) compare le sha256 de `OCTETS_DU_COMMUN` à `76d04c18…` et celui de `polices/OFL-Liberation.txt` à `93fed460…`, ceux de la copie de Tier list. Il faut pour cela `sha2`, en dépendance de test de `mise-en-page` seulement : 0.10.9, sans ses fonctionnalités par défaut, la version que wasmtime tire déjà par `cranelift-codegen` (aucun paquet nouveau au `Cargo.lock`, seule l'arête `mise-en-page → sha2` ; rien à télécharger, la source est dans le registre en cache). Sa raison est à côté d'elle dans `Cargo.toml` et dans l'annexe des dépendances de PHASE-3. Le commentaire de `polices.rs` (« testé ») devient vrai sans changer.
5. **[mineur] Les heures de la mesure et des contrôles.** Vérifié, exact. **Corrigé** d'après les `code.txt` des preuves : contrôles de 03 h 48 min 54 s à 03 h 50 min 00 s, mesure de E16 de 03 h 50 min 43 s à 03 h 50 min 50 s (MESURES-PHASE-3, « Lot 8 », et la section « Lot 8 » ci-dessus). La charge (1,91) et la commande ne changent pas.
6. **[mineur] Les modules des fautifs et d'`ardoise` changent d'empreinte, sans le plan standard des 42 jeux ni E20.** Vérifié, exact pour le plan standard : **relancé** (`cargo test -p epreuve --test epreuve -- --ignored plan_standard`, preuve `plan-standard`, résultat plus bas). Pour E20, le constat est juste, mais il n'est pas propre au lot 8 : les résultats E20 du lot 3b sur les fautifs et `ardoise` ne valaient déjà plus depuis le lot 6, où l'ABI 4 a changé l'empreinte de tous les modules du commun sans que E20 y soit relancé (« Lot 6 » : « E20 n'est pas relancé : le lot ne touche ni le relevé ni le banc »). **Pas relancé ici** : E20 demande les navigateurs et le banc, ce qu'aucune ligne de lot depuis le 3b n'exige pour un changement de module, et le relevé comme le banc sont inchangés. C'est dit dans « ce qui n'est pas fait ».
7. **[mineur] La règle « Le plan court ou le plan standard » se contredisait.** Vérifié, exact. **Corrigé** (PHASE-3, § 7) : au lot 8, le plan standard des trois jeux est exigé, mais pas leur code 0 ; ce que le lot doit donner, c'est chaque jeu en échec par E16 seul, tout autre contrôle à l'issue du lot 7 ; le code 0 revient au lot 7b.

### Les preuves de la correction

Dans `target/preuves/lot-8-correction/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt`), lancées à la suite par `target/travail/lot-8-correction/final.sh` à partir de 04 h 08 min 31 s. Empreinte du commun, la même au départ et à la fin de chacune : `b146dc5`, `diff HEAD` hors `.md` `cfcab7acae87687e`, non suivis (13) `48bb1db0be733751` (seuls des `.md` ont changé pendant) ; Punchline `f84b86f`, Sidequest `229199b`, Tier list `9827f39`, tous trois sans aucune modification. Un premier passage (04 h 04) s'est arrêté sur clippy (`format!` dans un `map`, dans le test du sha256) ; il a été interrompu, le test corrigé, et tout relancé.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-8-correction/controles.py` | code 0 : les neuf mutations (les huit du lot et celle du constat 3) rouges pour la raison attendue ; source remise (empreinte hors `.md` identique avant et après) ; 90 s |
| `fmt` | `cargo fmt --all --check` | code 0 |
| `clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p mise-en-page -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 ; le binaire (`5925060…`) n'est pas octet pour octet celui des preuves du lot 8 (`dfe87b7…`), bien que ses sources (hors tests et dépendances de test) soient les mêmes |
| `reconstruire-release` | la même, après `touch` de deux `lib.rs` | code 0 ; le binaire reconstruit est identique au précédent (0 octet ne diffère) : la construction se refait à l'identique, et la différence avec le lot 8 vient du changement de `Cargo.toml` et du `Cargo.lock` (la cause exacte n'est pas cherchée) |
| `e1-e16-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module du lot 8> --controles E1,E16 --json …` | code 1 chacun (E16) ; comparés aux rapports standard du lot 8 par `target/travail/lot-8-correction/compare.py` (`comparaison.txt`) : même module, même issue de E1 et mêmes lignes de polices, E16 de même issue, au même détail ligne pour ligne (142, 426 et 1 151 textes ; 9, 4 et 4 débordements ; 12, 1 et 10 080 glyphes absents), hors la seule ligne qui compare la vue la plus lourde des soirées, absente sans soirées (une évaluation de moins). Le journal (`final.log`) écrit « comparaison … 1 » : c'était la première version du comparateur, qui n'excluait pas cette ligne ; corrigé, relancé à la main, code 0 pour les trois |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 661 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 390 s ; 1,83 Go résidents au plus |
| `plan-standard` | `lourd /usr/bin/time -l cargo test -p epreuve --test epreuve -- --ignored plan_standard` | **code 0** : chacun des 42 jeux du commun (`devine`, `ardoise`, `sain`, les 37 fautifs d'avant, sur le socle à une demi-unité, et les deux nouveaux, tels que `modules::JEUX` les liste) rend sur le plan standard ce qu'il doit, chaque fautif son seul contrôle, chaque jeu sain tout ; 1 305 s, quatre fils ; 1,66 Go résidents au plus. C'est la première fois depuis la phase 2 que ce plan est joué (35 jeux alors) : il tient aussi l'ABI 4 et les lots 6 et 7 sur les modules du commun |

Les épreuves du lot 8 au plan standard (les trois jeux, les deux fautifs) ne sont pas relancées : les modules sont ceux du lot 8 (même sha256), et les sources du commun que le binaire compile sont les mêmes ; ce que le binaire nouveau juge de E1 et E16, il le juge pareil (les trois comparaisons ci-dessus). Aucune durée de cette section n'est une mesure (charge de 2,2 à 5,2 au départ et à la fin des commandes, écrite par le portillon).

### Ce qui n'est pas fait, et pourquoi

- **La police d'emoji** : attend la décision de l'hôte (PHASE-3, question 28).
- **Le `Cargo.lock` de Tier list** : à committer par l'agent du jeu (constat 2).
- **E20 sur les fautifs et `ardoise`** : pas relancé depuis le lot 3b (voir constat 6). Il faudrait `soiree-epreuve --wasm <module> --court --navigateurs chrome,firefox` sur chacun, par le portillon, un navigateur à la fois.
- Le reste de la section « Lot 8 » tient toujours (boutons, comptes à rebours, `EnPlace` non mesurés ; point 7 de RESTE-PHASE-2 ; débordements des jeux au lot 7b).

### Ce que le lot suivant doit savoir

- `sha2` 0.10.9 est une dépendance de test du commun (`mise-en-page`) ; une épreuve depuis les sources de Tier list ne la voit pas (dépendance de test seulement).
- Les contrôles négatifs du lot 8 sont maintenant `target/travail/lot-8-correction/controles.py` (neuf mutations ; il reprend les huit de `target/travail/lot-8/controles.py`).
- PHASE-3 a une question 28 (la police d'emoji).

## Lot 9 — Le rendu

Fait le 2026-11-07, de 04 h 50 à 06 h 30 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1). Chaque commande cargo, chaque scénario du banc, les contrôles négatifs et la mesure sont passés par le portillon (`target/hote/lourd`) ; seuls des `cargo check`, `cargo clippy` de quelques secondes sur une crate et des `cargo fmt` s'en sont passés. Un scénario du banc à la fois, jamais à côté d'une compilation ; chaque navigateur lancé a été fermé par le banc (aucun ne reste, vérifié). Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres (aucune compilation d'un jeu depuis ses sources : le plan court s'est joué sur leurs modules, `--wasm`).

### Ce qui est fait

**`web/rendu`** (nouvelle) : le dessin de la scène sur `wgpu` 30.0.1, Metal en natif, WebGPU ou WebGL2 dans le navigateur.
- `cadre` : la zone sûre et l'échelle entière de `FormeEcran::Scene` (HIS-46, PUN-51, SQ-52), la page de `FormeEcran::Page` (une unité par pixel de l'appareil, le contenu centré, un défilement).
- `liste` : la scène mise à plat en pixels, sans GPU : les groupes décalent, l'opacité se multiplie, `pixel_entier` pose l'origine sur un pixel, les bords d'un rectangle tombent sur un pixel entier ; un `Bouton` est un fond et un nom dans le style du commun, un `CompteARebours` un texte, un `Qr` des modules ; les composants, les avertissements, les surfaces et les sous-arbres définitifs ne dessinent rien ici. Les animations se lisent à un instant ou au repos (le sens des courbes, posé au mainteneur : question 29). Les éléments se rangent en **couches** (quads puis textes dans une couche ; un quad monte au-dessus d'un texte antérieur qu'il recouvre), pour garder l'ordre du peintre sans un tirage par nœud. L'option de test `Fautif::OublierUnNoeud` oublie le dernier `Rect`.
- `gpu` : `Quad` et `Cible`, `#[repr(C)]`, leurs octets écrits champ par champ aux décalages d'`offset_of!` (ni `unsafe` ni `bytemuck`), et leur programme `quads.wgsl`.
- `qr` : `qrcodegen` 1.8.0, un fond blanc, des modules noirs (une suite de modules d'une ligne en un quad), la zone calme de quatre modules.
- `rendu` : `Rendu` dessine dans sa propre texture (`Rgba8Unorm`), les quads d'un seul programme (une couleur prémultipliée, une image d'un pixel blanc pour un rectangle) et les textes de `glyphon` 0.12.0, chacun d'après l'agencement de `mise-en-page` et le tampon qu'elle en donne ; la relecture des pixels (`Relecture`) ; `recreer` après la perte du GPU (polices et images gardées en mémoire) ; `quand_fini` (`onSubmittedWorkDone`).

**`web/ecran`** (nouvelle) : `messages` (un `salle::ecran::VersPage` : la scène, une police validée comme E1, ou une image ; natif) et, dans le navigateur, `Ecran` : l'instance (WebGPU détecté, sinon WebGL2 ; WebGL2 forcé sur demande), la surface dans un format sans sRGB, la texture du rendu copiée sur la toile (`TextureBlitter`), la présentation, le travail du GPU fini daté par `performance.now()`, les erreurs du GPU gardées (`on_uncaptured_error`), la perte (`device.lost`) et la reprise, une image de démarrage qui compile les programmes, les images par `createImageBitmap` ou, pour un SVG, un `<img>`, lues par une toile hors écran (une image illisible se dessine comme absente). L'exemple `dessin-banc` est la page du scénario.

**`mise-en-page`** (commun) : `MiseEnPage::tampon`, `verifier_tampon`, `systeme`, `Agencement::octets_canoniques` ; les modules `bouton` (le style du nom d'un bouton, que le commun dessine) et `compte` (le texte d'un compte à rebours). Rien de ce que l'épreuve mesure ne change.

**`banc`** (commun) : le scénario `soiree-banc dessin` (`dessin.rs`, `pages/dessin.html`), décrit au § 8.2 de PHASE-3 (*écrit au lot 9*) ; la trace de CDP de Chrome (`Chrome::tracer`, `fin_de_trace`) ; `Lance::chrome` ; `machine::construire_de` (la construction d'un exemple d'une autre crate de `web/`) ; la règle des mêmes versions de la mesure (`web::dependances_tierces`, `web::ecarts_de_versions`) ; le banc dépend en plus de `partie` et `mise-en-page`.

**Dépendances** (`web/Cargo.toml`, chacune avec sa raison) : `wgpu` 30.0.1 (`std`, `wgsl`, `metal`, `webgpu`, `webgl`), `glyphon` 0.12.0, `qrcodegen` 1.8.0, `web-sys` 0.3.106 (des fonctionnalités de plus), `wasm-bindgen-futures` 0.4.79 ; pour les essais, `naga` 30.0.1 (`wgsl-in`), `rqrr` 0.11.0, `pollster` 1.0.1, `blake3` 1.8.7. Chacune des 140 crates entrées au `Cargo.lock` de `web/` a sa date vérifiée sur crates.io (`target/travail/lot-9/dates-web.txt`) : toutes publiées avant le 2026-10-24, sauf `log` 0.4.35 (2026-10-30), gardée parce que c'est celle du `Cargo.lock` du commun (même code que la mesure) ; `cc`, `smallvec`, `unicode-script` et `yazi` ramenées aux versions du commun (`cargo update --precise`). Aucune dépendance nouvelle au `Cargo.lock` du commun (le banc gagne deux arêtes vers des crates du commun).

Documents : PHASE-3 (§ 2.2 lignes `rendu` et `ecran`, § 2.9 *écrit au lot 9*, § 7 ligne du lot, § 8.2 ligne `dessin`, question 29 nouvelle, § 9.3 point 12, les deux annexes), MESURES-PHASE-3 (« Lot 9 »), CHANGELOG (« Phase 3, lot 9 »), `crates/README.md` (`mise-en-page`, `banc`, le graphe, les deux crates de `web/`, un constat de plus), `AGENTS.md` (`web/` et ses crates, le rendu sur Metal, `soiree-banc dessin`).

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **Le sens des courbes d'animation n'est écrit nulle part** (INTERFACE, § 2.9, ne nomme que les variants) ; le rendu doit pourtant dessiner « au repos ». Décidé provisoirement d'après le seul usage (le saut de Sidequest) : `Rebond` revient à son départ (§ 2.9 *écrit au lot 9* ; question 29).
2. **Les images déclarées de Sidequest et de Tier list sont des SVG**, que `createImageBitmap` ne lit pas : P23 le prévoyait (« un SVG par un élément `<img>` »), c'est fait ; les pixels passent par une toile hors écran (une copie par le processeur, que P23 ne prévoyait pas).
3. **Une image de démarrage** : sans elle, la première image de Punchline prenait 85 ms dans Chrome en WebGL2 (la compilation paresseuse des programmes), au-delà du budget. Elle est hors jeu ; la reprise après la perte du GPU, elle, ne chauffe rien : son coût compte dans « l'image revenue ».
4. **Les styles du nom d'un bouton et du texte d'un compte à rebours** vivent dans `mise-en-page` (et non dans `rendu`) pour que E16 puisse les mesurer un jour avec la même définition ; E16 ne les mesure toujours pas (le faire change ce que l'épreuve juge).
5. **Le banc ne recopie pas le cadre** : les points à relire sont dans le repère de la scène, et c'est la page qui les porte en pixels par le cadre du rendu (que les tests natifs vérifient au pixel près).

### Les tests qui le tiennent

- `web/rendu/tests/pixels.rs` (12, sur Metal) : `les_rectangles_et_les_groupes_tombent_au_pixel_pres` et `la_zone_sure_et_l_echelle_entiere_placent_la_scene` (chaque pixel exact), `l_opacite_se_melange_a_deux_pres`, `une_page_se_centre_et_defile`, `une_image_se_dessine_au_plus_proche` (et une image absente en gris), `un_texte_se_dessine_dans_sa_boite_et_porte_son_agencement`, `un_glyphe_absent_se_dessine_comme_une_boite_et_se_compte`, `un_bouton_a_le_fond_et_le_nom_du_commun`, `au_repos_une_animation_est_a_son_arrivee_sauf_le_rebond`, `l_oubli_d_un_noeud_se_voit_aux_pixels` (le contrôle négatif, en natif), `apres_la_perte_du_gpu_l_image_revient`, `un_compte_a_rebours_se_forme_depuis_le_pas`.
- `web/rendu/tests/wgsl.rs` (3) : le WGSL lu par `naga`, comparé à `Cible` (taille, décalages) et à `Quad` (emplacement, type et décalage de chaque attribut, pas des instances) ; `tests/qr.rs` (3) : `le_qr_dessine_se_relit_et_rend_l_adresse_sal191` (relu par `rqrr` à 1 280 × 720 et à 320 × 180) ; `tests/liste.rs` (4) : le cadre, `pixel_entier_pose_l_origine_sur_un_pixel_sq52`, l'opacité, les couches.
- `web/ecran/tests/messages.rs` (2).
- `crates/mise-en-page/tests/mesure.rs` (4 de plus, 17 en tout) : `le_tampon_du_dessin_porte_l_agencement` (proptest, 128 cas ; son cas minimal, un texte vide, gardé dans `mesure.proptest-regressions`), `un_tampon_d_un_autre_texte_est_refuse`, `les_octets_canoniques_distinguent_au_bit_pres`, `le_nom_d_un_bouton_tient_dans_sa_boite`.
- `crates/banc/tests/dessin.rs` (7) : `les_points_relus_evitent_ce_qui_recouvre_et_ce_qui_s_anime_out30_out32`, `un_texte_qui_deborde_couvre_tout`, `les_agencements_attendus_comprennent_boutons_et_comptes`, `un_rapport_fidele_reussit`, `chaque_ecart_fait_echouer_le_rapport` (un agencement, un pixel à 3 près, une image de 33,5 ms, une erreur du GPU, une perte non reprise, un tampon ; 2 près encore juste ; WebGL2 forcé mais WebGPU rapporté), `le_controle_negatif_doit_voir_l_oubli_et_rien_d_autre`, `la_trace_compte_les_images_perdues` ; `crates/banc/tests/web.rs` (2 de plus) : `la_mesure_des_textes_a_les_memes_versions_des_deux_cotes` et son témoin `une_autre_version_de_cosmic_text_se_voit`.
- **Les contrôles négatifs** (`target/travail/lot-9/controles.py`, huit mutations, chacune rouge pour la raison attendue, la source remise ensuite) : deux emplacements du WGSL intervertis ; le décalage d'un groupe ignoré ; l'option qui oublie un nœud désarmée ; le QR d'une autre adresse ; un tampon façonné plus large que l'agencement ; la tolérance relevée à 3 ; le budget ignoré jusqu'à 66 ms ; les versions non comparées. Au premier passage, la tolérance relevée n'était pas vue (le test changeait un canal de 203 où l'on attendait 0) : le test a été rendu exact (trois de plus sur un canal), rien n'est affaibli.
- **Le scénario et son contrôle négatif** : `soiree-banc dessin` (plus bas).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-9/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-9/preuve.sh` et `empreinte.sh`), lancées à la suite par `target/travail/lot-9/final.sh` de 06 h 05 min 02 s à 06 h 19 min 54 s (`final.log`). Les quinze preuves ont la même empreinte du commun au départ et à la fin : `cc207c1`, `diff HEAD` hors `.md` `cb658af3bf88bbcc`, non suivis (24) `13401830208cd254` ; seuls des `.md` ont changé pendant. Punchline, Sidequest et Tier list sans aucune modification.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-9/controles.py` | code 0 : les huit mutations rouges ; 19 s |
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0 |
| `clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p mise-en-page -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `clippy-web` | `lourd sh -c "cd web && cargo clippy --workspace --all-targets -- -D warnings"` | code 0 |
| `clippy-web-wasm32` | `lourd sh -c "cd web && cargo clippy --workspace --target wasm32-unknown-unknown --lib --examples -- -D warnings"` | code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | **code 0 : 33 réussis**, 0 échec (dont les 22 du rendu sur Metal) ; 49 s ; 0,83 Go |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0 |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module du lot 7> --court --json …` | code 1 chacun, **la référence du lot 8** : Punchline E2 (même constat) et E16, Sidequest E16 seul, Tier list E16 seul (E20 pas exécuté, comme au lot 8) ; comparés aux rapports `court-*` du lot 8 par `target/travail/lot-9/compare.py` (`comparaison.txt`) : chaque contrôle de même issue, et de même détail sauf E9 et E14, où le lot 9, sans les sources, n'a pas de copie native ; modules au même sha256 (`module.txt`) |
| `construire-banc` | `lourd sh -c "cargo build -p banc --bin soiree-banc && cd web && cargo build -p ecran --example dessin-banc --target wasm32-unknown-unknown --release && wasm-bindgen …"` | code 0 ; paquet `1201a6f5…` (`paquet.txt`) |
| `dessin` | `lourd --calme target/debug/soiree-banc dessin <les trois modules> --navigateurs chrome,firefox,safari --fois 5 --sans-construire --sortie target/preuves/lot-9/dessin.json` | **code 0** : Chrome WebGPU, Chrome WebGL2 forcé, Firefox (WebGL2) par défaut et forcé, tous réussis ; les deux contrôles négatifs réussis (l'oubli se voit à 2 pixels de Punchline, rien d'autre ne change) ; Safari pas exécuté (« You must enable 'Allow remote automation'… ») ; 25 s ; M8 dans `dessin/m8.txt` |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 674 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 559 s ; 1,87 Go |

Dans chaque passage réussi du scénario : 2 104 textes des pires cas (158, 445, 1 501) et 1 000 de la charge agencés au bit près comme en natif, chaque tampon conforme ; 104 points relus à 2 près (51 chez Punchline, 51 chez Sidequest, 2 chez Tier list, plus 8 de la charge), aucun hors de la toile ; 81 images de Tier list que le navigateur ne décode pas, dessinées comme absentes (constat, pas un écart : un en-tête PNG et des zéros) ; la perte du GPU reprise après chaque jeu, les mêmes pixels relus après. Le plan standard n'est pas exigé à ce lot (§ 7 : le lot ne change rien de ce que l'épreuve joue ou juge, ni `jeu` ni `jeu-abi`) ; E20 n'est pas relancé (ni le relevé, ni le rejeu, ni les modules ne changent).

### Les mesures

M8 (`MESURES-PHASE-3.md`, « Lot 9 ») : aucune image de pire cas au-delà de 33 ms (au plus 15,63 ms dans Chrome, 23,96 ms dans Firefox) ; le coût d'un nœud dessiné, 2 µs dans Chrome, 3,3 µs dans Firefox ; les images perdues de la trace de Chrome (0 à 1 en WebGPU, 64 en WebGL2 forcé) ; la perte du GPU reprise en 3 à 23 ms. Charge de 1,48 au départ, 4,14 à la fin (les navigateurs). Compilations (pas des mesures) : `rendu` et `ecran` natifs 49 s et 1,00 Go, `wasm32` 28 s et 0,70 Go ; `cargo test --workspace` 1,87 Go au plus, `web/` 0,83 Go.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté, `safaridriver` refuse la session tant que l'hôte n'autorise pas l'automatisation à distance (on ne touche pas à ce réglage).
- **La police d'emoji monochrome** : toujours pas livrée (question 28, aucune décision de l'hôte) ; PHASE-3 la voulait au plus tard ce lot. Un emoji se dessine comme une boîte, compté.
- **E16 ne mesure ni les noms des boutons ni les comptes à rebours**, bien que leur style soit fixé : le faire change ce que l'épreuve juge (plan standard des trois jeux, et sans doute de nouveaux débordements à corriger au lot 7b). À décider avec le lot 7b.
- **Le contrôle négatif ne se voit que chez Punchline** : chez Sidequest et Tier list, aucun point du dernier `Rect` n'a sous lui une couleur connue et différente. Tier list n'a que deux points relus (sa page est couverte d'un `EnPlace` de 1 100 × 1 125 et de textes) ; il en aura de plus utiles quand ses composants seront dessinés (lot 11).
- **Les 64 images perdues de Chrome en WebGL2** ne sont pas expliquées (§ Mesures) ; le lot 13 dessinera au rythme de l'écran.
- **Le rendu des composants** (`Champ`, `EnPlace`, `Modale`, `Selecteur`, `Lien`, `Deplacables`), les avertissements du commun, le glisser entre deux pas, le défilement réel de `FormeEcran::Page`, les demandes `Rendu`/`Image` : aux lots 10b à 13, comme prévu.
- **WebGPU dans Firefox** : non joué, Firefox 158 sans interface ne l'expose pas ici.

### Ce que le lot suivant doit savoir

- La page de la télé est `web/ecran::Ecran` : `creer`, `chauffer` (avant le premier jeu), `recevoir` (un `VersPage` encodé), `regler_forme`, `dessiner`, `quand_fini`, `relire`, `perdu`, `reprendre`. Elle ne fait encore rien des autres messages du Worker (`Recu::Laisse`). Le lot 10a la relie au Worker.
- Le rendu prend un `Contexte` : le pas (comptes à rebours), l'instant (animations ; `None` au repos), l'adresse du QR, le fond par défaut. La télé doit passer l'adresse réellement jointe.
- `soiree-banc dessin` refuse de jouer si `mise-en-page` ou ce qu'elle tire n'ont pas les mêmes versions dans les deux `Cargo.lock` : toute mise à jour d'une dépendance de `web/` doit s'y plier (`cargo update -p <crate> --precise <version du commun>`).
- `log` 0.4.35, au `Cargo.lock` du commun et de `web/`, a été publiée moins de deux semaines avant le lot ; elle était déjà au commun, et n'est pas changée ici.
- Les erreurs du GPU se lisent par `Ecran::erreurs_gpu` ; une panique dans la page se dit sur la console (le crochet de l'exemple `dessin-banc`), que le banc relaie.
- Les outils de travail hors dépôt : `target/travail/lot-9/stats` (ce que contiennent les pires cas), `sondes` (la place des nœuds d'une scène préparée), `m8.py`, `compare.py`, `controles.py`.

## Lot 9, correction après la vérification

Fait le 2026-11-07, de 06 h 25 à 06 h 55 environ (GMT), sur la même machine (Chrome 156.0.8078.80, Firefox 158.0.1). Chaque commande cargo, chaque scénario du banc et les contrôles négatifs sont passés par le portillon ; seuls des `cargo check`, `cargo clippy -p banc` de quelques secondes et des `cargo fmt` s'en sont passés. Un scénario à la fois, jamais à côté d'une compilation ; chaque navigateur a été fermé par le banc (aucun ne reste, vérifié par `pgrep`). Rien n'est commité. Seuls le commun (`crates/banc`, `web/rendu`, `web/ecran`, des `.md`) est modifié ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres.

### Les constats, et ce qui en est fait

1. **[important] La police d'emoji monochrome n'est pas livrée.** Vérifié, exact. **Pas corrigé** : elle attend toujours la décision de l'hôte (question 28 : fournir une police libre, ou autoriser son téléchargement), et aucun téléchargement ne se fait sans son accord. Un emoji se mesure et se dessine comme une boîte, compté (`un_glyphe_absent_se_dessine_comme_une_boite_et_se_compte`). Reste dans « non fait ».
2. **[important] Le contrôle négatif et les pixels relus sont maigres.** Vérifié, exact. **Corrigé en partie.** L'option de test oubliait toujours le dernier `Rect` de la scène, que seul Punchline relisait sur une couleur connue. Désormais :
   - `rendu::Fautif::OublierLeNoeud(n)` remplace `Fautif::OublierUnNoeud` : le rendu oublie le nœud `n`, sans rien changer de ce qu'il transmet à ses enfants (testé : `l_oubli_d_un_noeud_se_voit_aux_pixels` oublie aussi un enfant d'un groupe, et le reste du groupe reste à sa place).
   - Le banc choisit ce nœud dans chaque scène (`banc::dessin::noeud_a_oublier`). C'est le dernier `Rect` relu dont le point a dessous une couleur connue et différente ; s'il n'y en a pas, le dernier `Rect`. Pour cela, `sondes` cherche dans chaque rectangle un point libre qui voit l'oubli avant de prendre le premier point libre. Le manifeste porte ce nœud (`oublie`), et la page `dessin-banc` le passe au rendu scène par scène.
   - Résultat : l'oubli se voit chez **Punchline (3 points) et Sidequest (1 point)**, dans Chrome comme dans Firefox ; rien d'autre ne change.
   - **Tier list n'a toujours que 2 points relus, et aucun qui voie l'oubli.** Dans ses quatre pires cas, chaque `Rect` est recouvert par un composant (26 `EnPlace`, dont un de 1 100 × 1 125, des `Modale`, `Deplacables`, `Selecteur`), un texte, une image ou un bouton venu après lui. Le banc tient un composant pour couvrant, parce qu'on ne sait pas encore ce qu'il dessinera (lot 11). Sous le contrôle négatif, l'un de ses deux points change (1 exact sur 2) : c'est celui du dernier `Rect`, qui a dessous une couleur inconnue, et ce n'est pas compté comme une preuve. « Quelques pixels relus aux endroits prévus » ne tient donc, pour Tier list, que sur ces deux points.
   - Le scénario échoue toujours si aucun jeu ne voit l'oubli, ou si un jeu qui a un point sensible ne le voit pas.
3. **[mineur] Le juge laissait passer un rapport incomplet.** Vérifié, exact. **Corrigé** (`juger_sondes`, `juger_durees`). Ces cas sont maintenant des échecs : moins de points rapportés que demandés, un point rapporté hors de la toile (`null` : le banc ne choisit que des points visibles), moins de durées d'image que `--fois`. Pour juger ce dernier cas, `juger` prend désormais le nombre d'images demandées. Chacun de ces cas est tenu par un cas de plus de `chaque_ecart_fait_echouer_le_rapport` et par une mutation.
4. **[mineur] Une assertion tautologique dans `tests/qr.rs`.** Vérifié, exact. **Corrigé.** Le test s'appelle désormais `les_modules_ont_la_forme_d_un_qr_code` et vérifie la structure d'un QR code (ISO/CEI 18004) :
   - un côté de 17 + 4 v, les trois motifs de position exacts (7 × 7) et aucun dans le coin bas droit ;
   - les séparateurs blancs, les deux lignes de synchronisation, le module toujours noir ;
   - une autre adresse donne d'autres modules.
   Le contenu reste tenu par la relecture de `rqrr`.
5. **[mineur] `?rendu=webgl2` dans PHASE-3.** Vérifié, exact (la page et le banc lisent `?moteur=`). **Corrigé** dans PHASE-3 (§ 7, ligne du lot 9 ; § 8.2, ligne `dessin`) et dans la documentation de `ecran::ChoixMoteur::WebGl2`.
6. **[mineur] « 0 d'écart dans Chrome et Firefox » n'était pas prouvé.** Vérifié, exact : le banc jugeait à 2 près sans compter les points exacts. **Corrigé** : le banc compte les points relus exactement (`Lecture::exactes`, dans la ligne de chaque jeu). Dans la preuve `dessin`, chaque passage sans défaut a ses 112 points exacts, à 0 près (Chrome WebGPU, Chrome WebGL2, Firefox WebGL2 par défaut et forcé). L'annexe de PHASE-3 le dit maintenant, avec la preuve, et rappelle que la tolérance de 2 est un juge, pas une mesure.
7. **[mineur] « image illisible : {} ».** Vérifié, exact. **Corrigé** : `ecran::texte_d_erreur` donne le nom et le message d'une erreur, ce que `JSON.stringify` perd pour une `DOMException`. La page `dessin-banc` s'en sert aussi. On lit désormais : « image illisible : InvalidStateError : The source image could not be decoded. » (Chrome), « … The image could not be decoded » (Firefox).
8. **[mineur] Le plan court sur les modules et non depuis les sources.** Vérifié, et accepté tel quel, comme le vérificateur le juge : les modules ont le même sha256 qu'aux lots 7 et 8 (relu, `construire-banc/modules.txt`), et `mise-en-page` n'est dans le `Cargo.lock` d'aucun jeu. **Pas relancé** : la correction ne touche ni l'épreuve ni ce qu'elle compile. `epreuve` ne dépend pas de `banc`, et ni `mise-en-page` ni rien de ce qu'elle tire ne change. Les rapports `court-*` du lot 9 valent toujours.
9. **[mineur] `log` 0.4.35.** Vérifié, accepté tel quel : la règle des mêmes versions l'exige, et elle était déjà au `Cargo.lock` du commun. Aucun `Cargo.lock` ne change dans cette correction.

### Les tests qui la tiennent

- `crates/banc/tests/dessin.rs` (8, un de plus) : `le_noeud_oublie_est_le_dernier_dont_l_oubli_se_voit` (nouveau : un dernier `Rect` posé sur un texte ne voit pas l'oubli, le contrôle négatif oublie le précédent) ; `chaque_ecart_fait_echouer_le_rapport` (trois cas de plus : un point non rapporté, un point hors de la toile, une durée manquante).
- `web/rendu/tests/pixels.rs` : `l_oubli_d_un_noeud_se_voit_aux_pixels` (le nœud nommé ; l'oubli d'un enfant de groupe) ; `web/rendu/tests/qr.rs` : `les_modules_ont_la_forme_d_un_qr_code` (remplace `les_modules_sont_ceux_du_code`).
- **Les contrôles négatifs** (`target/travail/lot-9-correction/controles.py`) : les huit du lot, dont `oubli-desarme` réécrit pour `OublierLeNoeud`, et cinq de plus :
  - `points-manquants-acceptes`, `hors-de-la-toile-accepte`, `durees-manquantes-acceptees` : `chaque_ecart_fait_echouer_le_rapport` rougit ;
  - `oubli-du-dernier-rect-seulement` (le choix du nœud ramené au dernier `Rect`) : `le_noeud_oublie_est_le_dernier_dont_l_oubli_se_voit` rougit ;
  - `qr-inverse` : `les_modules_ont_la_forme_d_un_qr_code` rougit.
  Les treize rougissent pour la raison attendue, et chaque source est remise.

### Les preuves de la correction

Dans `target/preuves/lot-9-correction/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt`). Elles ont été lancées à la suite par `target/travail/lot-9-correction/final.sh` à partir de 06 h 37 min 00 s (`final.log`). Chaque preuve a la même empreinte du commun au départ et à la fin : `cc207c1`, `diff HEAD` hors `.md` `cb658af3bf88bbcc` (la même qu'au lot 9 : tout ce que la correction change hors des `.md` est encore non suivi), non suivis (24) `2adbe9fc57f87bc7`. Seuls des `.md` ont changé pendant. Punchline, Sidequest et Tier list n'ont aucune modification.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-9-correction/controles.py` | code 0 : 13 mutations sur 13 rouges ; 37 s |
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0 |
| `clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p mise-en-page -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `clippy-web`, `clippy-web-wasm32` | `lourd sh -c "cd web && cargo clippy --workspace --all-targets -- -D warnings"`, puis `--target wasm32-unknown-unknown --lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | **code 0 : 33 réussis**, 0 échec (12 pixels, 3 qr, 3 wgsl, 4 liste, 2 messages, 9 moteur-navigateur) |
| `construire-banc` | comme au lot 9 | code 0 ; paquet `1bbfed48…` (`paquet.txt`) ; modules `82669f77…`, `62043526…`, `84315a1b…` (`modules.txt`), ceux des lots 7 à 9 |
| `dessin` | `lourd --calme target/debug/soiree-banc dessin <les trois modules> --navigateurs chrome,firefox,safari --fois 5 --sans-construire --sortie target/preuves/lot-9-correction/dessin.json` | **code 0** : six passages réussis. Chrome WebGPU (pire image de pire cas 13,0 ms), Chrome WebGL2 forcé (15,5 ms), Firefox WebGL2 par défaut (20,0 ms) et forcé (22,3 ms). Chaque passage sans défaut a ses 112 points exacts, à 0 près, et 0 hors de la toile. Les deux contrôles négatifs voient l'oubli chez Punchline (3 pixels) et Sidequest (1), rien d'autre ne change. Safari pas exécuté (« You must enable 'Allow remote automation'… »). 18 s ; charge 2,99 au départ, 2,79 à la fin |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 675 réussis** (674 au lot 9, plus `le_noeud_oublie_est_le_dernier_dont_l_oubli_se_voit`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 372 s ; 1,00 Go résidents au plus ; charge 2,79 puis 3,05 |

Les durées du scénario ne remplacent pas M8 : la mesure du lot 9 (`MESURES-PHASE-3.md`, « Lot 9 ») tient, et la correction ne change ni le rendu ni ce que la page mesure. L'essai fait avant les preuves (`target/travail/lot-9-correction/essai.txt`, sous une charge de 3,47) n'est pas une mesure.

### Ce qui n'est pas fait, et pourquoi

- **La police d'emoji monochrome** : attend la décision de l'hôte (question 28).
- **Tier list : 2 points relus, aucun ne voit l'oubli** (constat 2). Il faut d'abord savoir ce que ses composants dessinent (lot 11). Le banc pourra alors cesser de les tenir pour couvrants là où ils ne dessinent rien sur la toile.
- **Safari** : pas exécuté (réglage de l'hôte, non touché).
- Le reste de la section « Lot 9 » tient toujours : E16 ne mesure ni les noms des boutons ni les comptes à rebours ; les images perdues de Chrome en WebGL2 ne sont pas expliquées ; WebGPU dans Firefox n'est pas joué ; le plan standard et E20 ne sont pas relancés.

### Ce que le lot suivant doit savoir

- L'option de test du rendu est `Fautif::OublierLeNoeud(indice)` ; `banc::dessin::juger` prend le nombre d'images par scène (`fois`).
- `ecran::texte_d_erreur` met en texte une valeur levée par la page ; à employer pour toute erreur JavaScript qu'on rapporte.
- Les contrôles négatifs du lot 9 sont maintenant `target/travail/lot-9-correction/controles.py` (treize mutations).

## Lot 10a — L'autorité dans le navigateur, et la reprise

Fait le 2026-11-07, de 06 h 55 à 08 h 30 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari 27.2 « pas exécuté »). Chaque commande cargo, chaque scénario du banc, les contrôles négatifs et les mesures sont passés par le portillon (`target/hote/lourd`) ; seuls des `cargo check`, `cargo fmt` et quelques `cargo clippy` d'une crate de quelques secondes s'en sont passés au début du lot (avant d'y revenir : tout le reste est passé par lui). Un scénario à la fois, jamais à côté d'une compilation (le paquet `web/` se construit avant le navigateur, dans le même scénario) ; chaque navigateur a été fermé par le banc (aucun ne reste, vérifié par `pgrep`). Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres.

### Ce qui est fait

**`web/autorite`** (nouvelle) : le Worker de l'autorité, coque de `salle::autorite::Autorite`. `enregistrement` (ce qu'IndexedDB garde : le code de salle, la clé privée de l'autorité, le secret d'entrée, G, le module joué et le dernier numéro de case écrit, une forme fixe à `VERSION_ENREGISTREMENT` 1, relue strictement ; son `Debug` ne montre aucun secret) et, dans le navigateur, `demarrer_le_worker` : IndexedDB (une salle neuve prend le code que le relais garde, `GET /salle`, sinon en tire un, et des clés neuves par `crypto.getRandomValues`) ; le module par `GET /jeux/<empreinte>.wasm`, son BLAKE3 vérifié, `TAILLE_MODULE_MAX`, puis `moteur_navigateur::charger` sous `PHASE_3` encadré par `VersPage::Chargement` ; `VersPage::Jeu` (la forme de l'écran) ; deux WebSocket `ws://localhost:<port>/ws` ; `Autorite::demarrer` sur `FabriqueQuiDit` (le geste en cours posté avant chaque `Entree`) ; le pas joué au signal de chaque image (`DeLaPage::Image`), et une minuterie de secours toutes les 8 ms quand la page n'a pas donné d'image depuis 20 ms ; le retard de chaque réveil noté au battement ; le dernier numéro écrit gardé dans IndexedDB à chaque `Ecrit` ; les polices et les images de l'écran envoyées à l'entrée en jeu ; un lien perdu le dit et le Worker se tait (la page le relancera). En mode banc : un rapport par seconde, la forme canonique à l'entrée en jeu et à la partie figée (`CommandeEcran::Figer`), le retard injecté (`Retarder`).

**`web/ecran`** : `page` (`demarrer_la_page`) : la toile et un bandeau du commun ; le Worker lancé sur `demarrage.js` lui-même, `Demarrer` une fois qu'il a dit `EnCours(None)` (il écoute), puis `Reprendre { ecarter }` s'il le faut ; la scène, les ressources et le jeu passés à l'écran dans l'ordre d'une file ; les demandes du jeu répondues comme la soirée en mémoire (lot 11) ; le chien de garde (`salle::chien`) toutes les 100 ms ; « Remplacée — Reprendre ici » et « Le commun est en panne — Reprendre » (un clic sur le bandeau) ; F8 (`keydown`) ; le Worker tué par la page qui s'en va (`beforeunload`, `pagehide`) ; `document.wasDiscarded` dit au banc ; en mode banc, les lignes `banc <onglet> <json>` sur la console et `window.__banc` (`tuer`, `figer`, `retarder`, `exporter_la_cle`, `reprendre_ici`, `pas`). La crate devient `cdylib` et `rlib`.

**`web/demarrage.js`** : le seul JavaScript écrit à la main, 28 lignes ; le même fichier pour la page et pour le Worker.

**`salle`** : `chien` (le chien de garde de la page, sans entrées-sorties : `GARDE_FOU_PARTIE_MS` 2 000, `GARDE_FOU_CHARGEMENT_MS` 1 000, REM-02 par l'écart des tics, `ARRETS_AVANT_D_ECARTER` 2, `RELANCES_MAX` 3 par minute, le retard signalé après trois réveils de plus de 100 ms) ; `en_cours` (`FabriqueQuiDit`, `geste_de_l_appel`) ; dans `autorite` : `Reprendre { ecarter }` gardé dès le démarrage, le geste écarté (`Ecarte` à l'appareil, compté comme placé, gardé dans le registre, dit à la page en panne de l'outil, une case écrite même vide), F8 (`marquer`), `ExporterLaCle`, `VersPage::Accueil` (le fragment et les adresses, pour le QR), `noter_retard`, `venue`, `depuis_le_point`, `garde_a_jour`, `Comptes::gestes_ecartes` et `marques` ; dans `garde` : le registre porte `marques` (`Marque`, au plus `MARQUES_MAX` 64) et `ecartes` (`VERSION_CASES` 2), `relire_l_historique` (la chaîne entière depuis sa première case), `journal_jusqu_a` ; dans `ecran` : les variants du lot 10a (`PROTOCOLE` 2) ; dans `simulation` : `observer_la_page`, `depuis_la_page`, `a_la_relance`, `reprendre_de_l_historique`, et `accuses_hors_de_l_historique` qui ne compte pas un geste écarté.

**`relais`** : `paquet` (`Paquet`, `Jeux`, `page_de_demarrage`) ; `--paquet` et `--jeux` ; `GET /web/<empreinte>/<fichier>` (à tous, `immutable`, CORP, COOP, COEP), `GET /jeux/catalogue` et `GET /jeux/<empreinte>.wasm` (boucle locale) ; la politique de contenu gagne `img-src 'self' blob:`.

**`banc`** : `soiree` (`soiree-banc soiree`, `--m12`, `soiree-banc arriere-plan`, `construire_le_paquet`), `rejouer_garde` (`soiree-banc rejouer-garde`) ; les pilotes de Chrome et de Firefox savent évaluer, recharger, ouvrir et fermer un onglet, cacher et montrer, appuyer sur F8.

**Dépendances** : aucune crate tierce nouvelle (le `Cargo.lock` de `web/` ne gagne que `autorite` ; celui du commun ne change pas). `web-sys` prend des fonctionnalités de plus dans `autorite` et `ecran` (WebSocket, IndexedDB, `Crypto`, Worker, clavier). `getrandom` n'entre pas : `crypto.getRandomValues` s'appelle par `web-sys`.

Documents : PHASE-3 (§ 2.2, § 2.3, § 2.4, § 2.5, § 2.6, § 2.7, § 7 ligne du lot, § 8.2 ligne `soiree <jeu>`, § 8.3 `arriere-plan`, § 8.6 M12, question 30), MESURES-PHASE-3 (« Lot 10a »), CHANGELOG (« Phase 3, lot 10a »), `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **La politique de contenu gagne `img-src 'self' blob:`** : la télé décode un SVG par un `<img>` sur un `blob:` (P23) ; `default-src 'self'` le refusait, et le banc l'a vu (une violation dans Chrome et Firefox, sur les SVG de Tier list, pendant M12).
2. **`GET /banc` et `POST /banc/rapport` ne sont pas écrits** : la page écrit sur sa console, le banc la lit et la pilote par CDP et BiDi.
3. **`soiree-relais preparer` reste au lot 10b** : le paquet se construit par le banc, dans `target/soiree-web/<empreinte>/`.
4. **Sans `getrandom`** : `crypto.getRandomValues` par `web-sys` ; un échec arrête l'autorité.
5. **La minuterie de secours du Worker bat toutes les 8 ms** (et non 16), et joue un pas sans image depuis 20 ms : à 16 ms, un réveil d'environ 22 ms manquait deux pas assez souvent pour qu'un jeu qui ne rattrape qu'un pas saute (constaté : 159 sauts dans Chrome, 507 dans Firefox, en 40 s cachés ; à 8 ms, 1 et 5).
6. **Le retard de `arriere-plan` s'injecte une fois l'onglet revenu** : il fait sauter des pas à dessein.
7. **Le fichier d'une marque ne se rejoue pas encore par l'épreuve** (aucune option ne le lit) ; `rejouer-garde` le rejoue dans wasmtime et l'écrit.
8. **Les messages entre la page et le Worker** gagnent des variants (`PROTOCOLE` 2), et la garde porte les marques et les écartés (`VERSION_CASES` 2) : PHASE-3 n'en nommait pas la forme.

### Les tests qui le tiennent

- `crates/salle/tests/devine.rs` (2 de plus) : `une_entree_qui_bloque_deux_fois_est_ecartee` (P39 ; HIS-06, OUT-11 : deux arrêts muraux pendant le même geste, sur `devine` dans wasmtime, avec le chien de garde de la page ; le geste écarté n'est pas écrit, rien d'autre n'est perdu, la file du téléphone se vide, la garde reprend la même partie) et `une_marque_f8_va_a_la_garde_et_survit_a_la_reprise_sal162`.
- `crates/salle/src/chien.rs` (8 tests) : un Worker qui bat n'est jamais tué ; un Worker muet l'est à son garde-fou, celui du chargement compris ; `une_page_gelee_n_accuse_pas_le_worker_rem02` ; `le_second_arret_sur_le_meme_geste_l_ecarte_p39` ; un battement finit le geste en cours ; au-delà de trois relances en une minute, la panne ; trois retards de suite signalés une fois.
- `crates/salle/tests/ecran.rs` et `garde.rs` : les octets figés et la seconde lecture des variants nouveaux, du registre aux marques et aux écartés, `PROTOCOLE` 2, `VERSION_CASES` 2.
- `crates/relais/tests/relais/http.rs` (2 de plus) : `le_paquet_et_les_modules_se_servent_de_la_table_sal150`, `un_paquet_ou_un_catalogue_mal_forme_se_refuse` ; `coque.rs` : la politique de contenu.
- `web/autorite/tests/enregistrement.rs` (4) : la forme figée, la seconde lecture (SAL-90), l'enregistrement abîmé refusé sans panique (REM-64), le `Debug` sans secret.
- **Les contrôles négatifs** (`target/travail/lot-10a/controles.py`), dix mutations, chacune rouge pour la raison attendue, la source remise : l'écart désarmé ; trois arrêts avant d'écarter (deux tests) ; REM-02 retiré ; la marque oubliée ; les marques perdues à la reprise ; `/jeux` hors de la boucle ; le paquet sans empreinte ; un drapeau faux accepté dans l'enregistrement ; et, avec `--scenario`, **le contrôle négatif du scénario du navigateur** : une autorité qui reprend du point sans son journal ; `soiree-banc soiree` voit chaque reprise différer de wasmtime, et la soirée entière ne pas se rejouer (`RejeuDivergent`).
- **Les scénarios** : `soiree-banc soiree`, `arriere-plan`, `--m12`, `rejouer-garde` (plus bas).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-10a/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-10a/preuve.sh` et `empreinte.sh`), lancées à la suite par `target/travail/lot-10a/final.sh` de 07 h 54 min 09 s à 08 h 24 min 58 s (`final.log` ; un premier passage, interrompu par un défaut de clippy dans un test, est dans `final-interrompu.log`, et ses preuves effacées). Toutes ont la même empreinte du commun au départ et à la fin : `79a899b`, `diff HEAD` hors `.md` `6e4c8b5ff7869349`, non suivis (12) `e7cf9f7de107120a` (le `diff` entier change pendant la première, `controles`, où des `.md` s'écrivaient encore). Punchline, Sidequest et Tier list sans aucune modification.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-10a/controles.py` | code 0 : 9 mutations sur 9 rouges ; 34 s |
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0 |
| `clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-wasm32` | `lourd cargo clippy -p mise-en-page -p liaison -p salle -p partie -p jeu -p jeu-abi -p moteur-commun --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `clippy-web`, `clippy-web-wasm32` | `lourd sh -c "cd web && cargo clippy --workspace --all-targets -- -D warnings"`, puis `--target wasm32-unknown-unknown --lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | **code 0 : 37 réussis** (33 au lot 9, plus les 4 de `autorite`), 0 échec |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 ; modules `82669f77…`, `62043526…`, `84315a1b…` (`modules.txt`), ceux des lots 7 à 9 |
| `soiree` | `lourd /usr/bin/time -l target/debug/soiree-banc soiree <Punchline> --navigateurs chrome,firefox,safari --travail target/preuves/lot-10a/soiree-travail` | **code 0** : Chrome (WebGPU) et Firefox (WebGL2) réussis ; Safari pas exécuté (« The session timed out while connecting to a Safari instance… »). Dans chacun : quatre reprises (écran rechargé, Worker tué, second onglet, « Reprendre ici ») de même forme canonique et même empreinte que wasmtime sur les cases 0 à 195, 203, 296, 304 (Chrome) et 0 à 203, 209, 305, 313 (Firefox) ; `Remplacee` au premier onglet ; une phase terminale ; la chaîne entière (394 et 405 cases, 5 points, aucun geste deux fois, aucune entrée écartée, 7 sauts : les reprises) rejouée dans wasmtime depuis sa première case, chaque empreinte retrouvée, de même forme que le Worker figé, comme la garde finale ; tout geste accusé écrit ; trois marques F8 posées, trouvées et rejouées ; `connect-src 'self'` sur la page, le Worker relié à `ws://localhost:7878/ws`, aucune violation dite ; aucune relance du chien, aucune panne de l'outil. Paquet `9b384233…` (`paquet.txt` : `autorite_bg.wasm` `fe6f31b7…`, `ecran_bg.wasm` `046d83a1…`) ; 130 s ; 0,79 Go résidents (le banc) |
| `rejouer-garde` | `lourd target/debug/soiree-banc rejouer-garde <la garde de Chrome> --cle <cle.txt> --module <Punchline> --sortie …/marques` | code 0 : 118 cases, dernier point à l'indice 16 353 (pas 2 400) ; les trois marques (pas 2 412, 2 473, 2 533) rejouées, `marque-0.soiree` à `marque-2.soiree` écrits |
| `arriere-plan` | `lourd target/debug/soiree-banc arriere-plan <Punchline> --navigateurs chrome,firefox,safari --cache 360 --sans-construire …` | **code 1** : **Chrome réussi** (360 s caché, 21 587 pas joués, aucun saut dans la fenêtre cachée, le retard injecté signalé, la garde finale et la soirée entière de même forme que le Worker figé) ; **Firefox en échec** : 21 598 pas joués, le retard signalé, mais **24 sauts d'un pas** dans la fenêtre cachée, dont 15 au pas qui suit la prise d'un point (question 30) ; Safari pas exécuté ; 806 s |
| `m12-punchline`, `-sidequest`, `-tier-list` | `lourd --calme target/debug/soiree-banc soiree <module> --navigateurs chrome,firefox --m12 3 --sans-construire …` | code 0 chacun : trois reprises au pire moment par navigateur, chacune de même forme que wasmtime ; M12 dans `MESURES-PHASE-3.md`, « Lot 10a » |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 687 réussis** (675 au lot 9 ; 12 de plus : 8 du chien, 2 de `devine`, 2 du relais), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 410 s ; 1,03 Go résidents au plus |
| `controle-scenario` | `lourd python3 -I target/travail/lot-10a/controles.py --scenario` | code 0 : les neuf mutations rouges, et le scénario muté en échec pour la raison attendue (chaque reprise diffère de wasmtime ; la soirée entière ne se rejoue pas, `RejeuDivergent`) ; le paquet muté a été effacé ensuite |

**Le plan court des trois jeux n'est pas relancé** : `epreuve` ne dépend d'aucune crate que le lot change (`cargo tree -p epreuve -e normal` n'y trouve, parmi les crates touchées, que `liaison`, qui ne change pas) ; les modules ont le même sha256 qu'aux lots 7 à 9 ; les rapports `court-*` du lot 9 valent toujours. Le plan standard n'est pas exigé (§ 7). E20 n'est pas relancé (ni le relevé, ni le rejeu, ni les modules ne changent). `soiree-banc dessin` n'est pas relancé : `ecran` ne change pour lui que par sa sorte de bibliothèque et trois variants laissés (`tests-web` tient `messages`).

### Les mesures

M12 (`MESURES-PHASE-3.md`, « Lot 10a ») : de la création du Worker à la partie reprise, au pire moment (575 à 590 pas depuis le point), 213 à 318 ms dans Chrome, 405 à 657 ms dans Firefox, chez les trois jeux, sur machine calme (charge au plus 1,99) ; `POINT_TOUS_LES` reste à 600. Le constat des sauts de Firefox onglet caché y est écrit, sans durée. Compilations (pas des mesures) : le paquet `web/` (`ecran` et `autorite` pour `wasm32` en release, et leur colle) se construit dans le scénario `soiree` ; le banc y tient 0,79 Go ; `cargo test --workspace` 1,03 Go au plus, `web/` 0,20 Go.

### Ce qui n'est pas fait, et pourquoi

- **`arriere-plan` échoue dans Firefox** : 24 sauts d'un pas en 360 s cachés, au tour d'un point pour la plupart. Ni le contrôle ni `rattrapage` ne sont changés ; le coût d'un point dans le Worker se mesure au lot 11 (M3), R2 (le chiffrement hors du tour) se juge alors ; question 30 au mainteneur. Le lot n'est donc pas entièrement vert : c'est la seule colonne qui ne l'est pas.
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Le fichier d'une marque n'est pas rejoué par l'épreuve** comme un test (§ 2.4 le voulait) : `rejouer-garde` l'écrit et le rejoue dans wasmtime ; l'option de l'épreuve qui le lirait reste à écrire.
- **L'empreinte réécrite n'est pas vérifiée contre le rapport d'épreuve** au chargement (§ 2.8, point 2) : les rapports du catalogue viennent au lot 10b (`soiree-epreuve --catalogue`) ; le Worker vérifie le BLAKE3 du module livré.
- **« Le commun est en panne »** ne propose qu'une reprise (un Worker neuf) ; la nouvelle partie demande `ChangerDeJeu`, que l'autorité ne fait pas encore (lot 10b). Le bandeau du commun reste un texte : l'écran du commun, le QR, le plein écran, « Cliquer pour reprendre », le son, les entrées du jeu (clavier hors F8, pointeur, composants) viennent aux lots 10b à 13. Le bandeau des téléphones pour un retard signalé n'est pas fait (la page des téléphones vient au lot 12).
- **Un relais relancé** sous un écran du navigateur n'est pas joué (§ 8.2 le nomme ; la ligne du lot ne le demande pas) : le Worker dit son lien perdu et se tait, la page le relance au bout de 2 s ; non éprouvé.
- **Le chien de garde n'a jamais tué de Worker dans un navigateur** : ses décisions sont éprouvées en natif (`chien`, `une_entree_qui_bloque_deux_fois_est_ecartee`) ; le Worker tué du scénario l'est par le crochet de test, comme le chien le tuerait. Aucun module ne sait bloquer un Worker sous le carburant.
- `/favicon.ico` répond `404` : Chrome le dit dans son journal (une ligne « journal error » que le banc ne compte pas comme une faute).

### Ce que le lot suivant doit savoir

- La page de la télé démarre par `web/demarrage.js` (une page du relais qui le charge, sans script en ligne) ; `?jeu=<empreinte>` nomme le module d'une salle neuve, sinon le premier du catalogue ; une salle déjà ouverte reprend le module gardé avec G. Le lot 10b remplace le bandeau par l'écran de démarrage, et écrit `ChangerDeJeu` (une nouvelle `Autorite` sur une nouvelle fabrique, même registre).
- Le relais se lance avec `--paquet <target/soiree-web/<empreinte>>` et `--jeux <dossier de <empreinte>.wasm>` ; `soiree-relais preparer` et `jouer` (lot 10b) doivent construire le paquet comme `banc::soiree::construire_le_paquet`.
- Le Worker écrit `VersPage::Banc` (JSON) seulement quand la page de démarrage porte `<meta name="soiree-banc">`, que le relais n'écrit qu'avec `--banc`.
- Une garde d'avant ce lot ne se relit plus (`VERSION_CASES` 2).
- La forme d'un nouveau message entre la page et le Worker se fige dans `salle/tests/ecran.rs`, avec sa seconde lecture, `PROTOCOLE` monté.
- `soiree-banc soiree --sans-construire` prend le dernier paquet construit (`target/soiree-web/`, par date) : le contrôle négatif du scénario en construit un muté, que `controles.py --scenario` laisse ; il a été effacé à la main ici. À effacer de même après chaque `--scenario`.
- Les outils hors dépôt : `target/travail/lot-10a/controles.py` (dix mutations), `final.sh`, `preuve.sh`, `empreinte.sh` ; les essais du lot dans `target/travail/lot-10a/essai*`.

## Lot 10a, correction après la vérification

Faite le 2026-11-07, de 08 h 40 à 12 h 10 environ (GMT), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Chaque cargo, chaque scénario du banc et les contrôles négatifs sont passés par le portillon (sauf quelques `cargo check -p` de quelques secondes) ; aucun navigateur n'est resté ouvert (`pgrep`). Rien n'est commité. Seul le commun est modifié ; les trois jeux sont restés propres.

### Les constats, et ce qui en est fait

1. **[bloquant] `arriere-plan` échouait dans Firefox (24 sauts).** Vérifié, puis expliqué par le banc lui-même : le Worker rapporte maintenant chaque tour de plus de 10 ms et chaque trou de plus de 30 ms entre deux pas, avec ses durées (`Autorite::chronometrer`, `DureesDuTour`). Trois causes, constatées le 2026-11-07 dans Firefox onglet caché (`target/travail/lot-10a-corr/essai1/`, 60 s) :
   - **le tour qui prend un point** durait de 13 à 33 ms (le scellement, l'encodage et l'envoi d'un point de 0,9 à 1,4 Mo) : 15 des 24 sauts. **Corrigé** : le point se scelle par tranches de 128 Kio, une par réveil, à partir du réveil suivant (`salle::chiffre::ScellementDeCase`, `salle::garde::Ecrivain::commencer_point` et `finir_point`, `salle::autorite::TRANCHE_DU_POINT`), les octets étant ceux d'un scellement d'un coup ; aucune case de journal ne se scelle avant lui, une écriture d'urgence de la garde le finit d'abord. Le tour qui prend le point garde l'instantané et l'encodage : de 10 à 21 ms dans Firefox (l'encodage d'environ un Mo par `jeu_abi::encoder`, octet par octet, en est l'essentiel ; non changé, voir plus bas) ;
   - **le moment où l'onglet se cache** (le saut des pas 650 à 653, dans Firefox et, une fois, dans Chrome) : à la dernière image, le secours ne jouait qu'après 20 à 28 ms sans image, et si l'image tombait tard dans le pas, deux pas étaient manqués. **Corrigé** : la minuterie joue le pas, images ou pas, dès qu'attendre son réveil suivant ferait manquer deux pas (`Autorite::secours_presse`, `MARGE_SECOURS_MS` 4) ;
   - **des réveils en retard d'environ 30 ms**, tours courts, de loin en loin (Firefox onglet caché : la minuterie de 8 ms ne revenait parfois qu'après 30 à 32 ms, même sur une machine calme ; et, sous la charge de la machine, jusqu'à 96 ms). **Atténué** : la minuterie bat toutes les 4 ms (le plancher des minuteries répétées) ; le pas se joue au plus 4 ms après son début, et un trou de moins de 29 ms entre deux pas ne fait plus manquer deux pas (il en fallait 25 à 8 ms). Un trou de plus de 33 ms en fait toujours manquer deux : le fil du Worker n'a pas tourné, et cela ne se corrige pas dans le commun (voir « Ce qui n'est pas fait »).
   Ni le contrôle (aucun saut dans la fenêtre cachée, 90 % des pas) ni le jeu (`rattrapage: 1`) ne sont changés.
2. **[important] Le « réussi » de Chrome tenait à un seul passage.** Vérifié : l'essai 6 du lot notait un saut au pas 653, au moment où l'onglet se cache (la cause corrigée au point 1). `soiree-banc arriere-plan --fois <n>` joue maintenant `n` passages par navigateur ; la preuve en joue trois dans chacun.
3. **[mineur] `arriere-plan` ne vérifiait pas que l'onglet était caché.** Vérifié et corrigé : le banc lit `document.visibilityState` au début et à la fin de la fenêtre (`hidden` exigé), et le Worker compte les pas joués au signal d'une image et par la minuterie (`par_image`, `par_minuterie` du rapport d'état) ; au plus 1 % des pas de la fenêtre au signal d'une image. **Un seuil choisi après l'avoir vu** : posé d'abord à zéro, il a fait échouer deux passages de Firefox où l'onglet était bien caché (`hidden` au début et à la fin) et où une seule image était venue en 360 s (constaté : Firefox donne encore, de loin en loin, une image à un onglet caché ; de 0 à 3 en 360 s dans les passages gardés, `target/preuves/lot-10a-corr-passage1/`) ; un onglet visible en donne une par pas. Son contrôle négatif : un banc qui ne cache plus l'onglet (`onglet-pas-cache`).
4. **[mineur] `soiree` ne jugeait pas les sauts onglet visible.** Vérifié : le lot affirmait « 7 sauts : les reprises » sans le rattacher, et il y en avait trois de trop (une image tardive). Corrigé : chaque saut du journal est rattaché à ce qui l'explique (le premier pas joué après une reprise, d'après la ligne « en-jeu » relue d'une case ; un retard injecté, d'après la ligne « retarde » du Worker), ou il est une faute ; de même dans `arriere-plan`, fenêtre cachée ou non. Constaté depuis : quatre sauts, les quatre reprises. Son contrôle négatif : un tour de 60 ms tous les 300 pas onglet visible (`saut-visible`).
5. **[mineur] Le retard s'injectait onglet revenu.** Corrigé : il s'injecte pendant que l'onglet est caché, vingt secondes avant la fin ; le Worker dit le pas où il gèle, et seuls les sauts qui le suivent (de `p` à `p + 2`) lui sont rattachés. Le signalement est constaté onglet caché, dans Chrome et Firefox.
6. **Vu en chemin : M12 n'attrapait plus le pire moment dans Firefox.** Avec la minuterie à 4 ms, le rapport d'état d'une fois par seconde tombait, chez Firefox, toujours au même endroit du cycle des points (566 à 570 pas depuis le point, jamais 575), et `--m12` attendait 60 s en vain chez les trois jeux (`target/preuves/lot-10a-corr-passage2/m12-*`). Corrigé : en mode banc, le Worker envoie un rapport de plus, une fois par point, à 580 pas (`PIRE_MOMENT`) ; le pire moment est alors pris à 580 pas, exactement.
7. **[mineur] Écarts dits.** Rien n'y est changé : le fichier d'une marque n'est toujours pas rejoué par l'épreuve ; `GET /banc` et `POST /banc/rapport` ne sont pas écrits ; `soiree-relais preparer` reste au lot 10b ; le chien de garde n'a toujours tué aucun Worker dans un vrai navigateur ; Safari « pas exécuté », le réglage de l'hôte non touché.

### Les écarts à PHASE-3, corrigés dans PHASE-3

- § 2.3 : la minuterie de secours bat toutes les 4 ms, et joue le pas dès qu'attendre ferait manquer deux pas.
- § 2.4 : le point se scelle par tranches.
- § 8.2 (`soiree`) : chaque saut doit venir d'une reprise.
- § 8.3 (`arriere-plan`) : le retard s'injecte onglet caché ; l'onglet est constaté caché ; chaque saut est rattaché ; `--fois`.
- Question 30 : réponse provisoire mise à jour.
- Annexe : `chacha20` 0.9.1 et `poly1305` 0.8.0 dans `salle`, les versions que `chacha20poly1305` 0.10.1 tirait déjà (aucune crate de plus dans les deux `Cargo.lock`).

### Les tests qui la tiennent

- `crates/salle/tests/chiffre.rs` : `une_case_scellee_par_tranches_a_les_octets_figes` (les octets figés du scellement d'un coup) ; `sceller_par_tranches_rend_les_octets_d_un_coup` (proptest : toute longueur jusqu'à 5 000 octets, toutes tranches, les mêmes octets, la même empreinte, la case se rouvre).
- `crates/salle/tests/devine.rs` : `un_point_se_scelle_par_tranches_sur_plusieurs_reveils` (tranches de 1 Kio : le tour qui prend le point ne l'écrit pas ; il part plus de trois réveils plus tard ; aucune case de journal ne se scelle devant lui ; une autorité tuée pendant le scellement reprend du point d'avant, la même partie ; la chaîne entière se vérifie ; et `secours_presse`).
- Les contrôles négatifs (`target/travail/lot-10a-corr/controles.py`) : les neuf du lot, et quatre de plus, chacun rouge pour la raison attendue : le point scellé d'un coup ; une tranche sans Poly1305 ; une tranche sans l'empreinte ; le secours jamais pressé. Avec `--seulement-scenarios`, trois scénarios mutés : la reprise sans journal (du lot), un tour de 60 ms onglet visible (`saut-visible`), un onglet que le banc ne cache plus (`onglet-pas-cache`).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-10a-corr/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-10a-corr/preuve.sh`, `empreinte.sh`, `final.sh`, `controles.py`), lancées à la suite par `final.sh` de 11 h 03 min 34 s à 12 h 01 min 50 s (`final.log`), puis deux M12 refaits. **Toutes ont la même empreinte du commun au départ et à la fin** : `79a899b`, `diff HEAD` hors `.md` `e9bb202fdee87f2d`, non suivis (13) `084465bc09fd9a81` (le `diff` entier change pendant `controles` et `m12-punchline`, où des `.md` s'écrivaient). Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sans aucune modification. Deux passages antérieurs de `final.sh`, sur un code d'avant (la minuterie à 8 ms et le critère « aucune image » ; puis sans le rapport au pire moment), sont gardés pour ce qu'ils montrent : `target/preuves/lot-10a-corr-passage1/` et `-passage2/` (`final-passage1.log`, `final-passage2.log`).

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-10a-corr/controles.py` | code 0 : 13 mutations sur 13 rouges, chacune pour la raison attendue ; 54 s |
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 37 réussis, 0 échec |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 ; modules `82669f77…`, `62043526…`, `84315a1b…` (ceux des lots 7 à 10a) |
| `soiree` | `lourd /usr/bin/time -l target/debug/soiree-banc soiree <Punchline> --navigateurs chrome,firefox,safari …` | **code 0** : Chrome et Firefox réussis, Safari pas exécuté (`safaridriver` : « The session timed out while connecting to a Safari instance… »). Dans chacun : les quatre reprises de même forme que wasmtime, `Remplacee`, une phase terminale, trois marques F8 rejouées, la chaîne entière rejouée ; **quatre sauts, chacun rattaché à sa reprise** (Chrome : pas 1 250, 1 312, 1 928, 1 981 ; Firefox : 1 244, 1 311, 1 932, 1 981). Paquet `6b6be334…` (`paquet.txt`) ; 162 s ; 0,77 Go résidents |
| `rejouer-garde` | `lourd target/debug/soiree-banc rejouer-garde <garde de Chrome> --cle … --module <Punchline> --sortie …` | code 0 : trois marques sur trois rejouées |
| `arriere-plan` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc arriere-plan <Punchline> --navigateurs chrome,firefox,safari --cache 360 --fois 3 --sans-construire …` | **code 1** : Chrome réussi 2 fois sur 3 ; le troisième passage, 7 sauts en 2,7 s (pas 5 297 à 5 462), après des trous de 30 à 89 ms et un tour de 23 ms ; **Firefox en échec 3 fois** : 5, 2 et 1 sauts, chacun après un trou de 30 à 73 ms, le tour court. Dans chaque passage : l'onglet `hidden` au début et à la fin, au plus 1 image (Firefox) dans la fenêtre, plus de 21 300 pas joués par la minuterie, le retard injecté onglet caché, signalé, et ses trois sauts rattachés. Safari pas exécuté (×3). Charge 1,91 au départ, **5,48 au plus pendant** ; 2 404 s ; 0,76 Go |
| `m12-tier-list`, `m12-punchline-refait`, `m12-sidequest-refait` | `lourd --calme target/debug/soiree-banc soiree <module> --navigateurs chrome,firefox --m12 3 --sans-construire …` | code 0 chacun, charge au plus 2,48, 2,38 et 2,90 : M12 (`MESURES-PHASE-3.md`, « Lot 10a, correction ») |
| `m12-punchline`, `m12-sidequest` | les mêmes, dans `final.sh` | code 0, mais la charge a atteint 3,96 et 3,16 : **écartés** comme mesures, refaits plus haut |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 690 réussis** (687 au lot, et 3 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 409 s ; 0,99 Go |
| `controle-scenario` | `lourd python3 -I target/travail/lot-10a-corr/controles.py --seulement-scenarios` | code 0 : les trois scénarios mutés en échec pour la raison attendue : `reprise-sans-journal` (« la forme canonique diffère »), `saut-visible` (« 8 saut(s) qu'aucune reprise n'explique »), `onglet-pas-cache` (« document.visibilityState vaut "visible" ») ; chaque paquet muté effacé, le banc reconstruit sur la source remise ; 175 s |

Le plan court n'est pas relancé : `epreuve` ne dépend d'aucune crate que la correction change (`salle`, `banc`, `web/autorite` ; `liaison` et `partie` ne changent pas) ; les modules ont le même sha256.

### Les mesures

M12 remesuré (`MESURES-PHASE-3.md`, « Lot 10a, correction ») : à 580 pas depuis le point, 217 à 285 ms dans Chrome, 437 à 660 ms dans Firefox, sur machine calme. Les durées des tours longs du Worker et les sauts d'`arriere-plan` y sont écrits comme constats, avec la charge.

### Ce qui n'est pas fait, et pourquoi

- **`arriere-plan` n'est pas vert.** Les causes que le commun portait sont corrigées (le point dans un seul tour, le passage à l'arrière-plan, la minuterie de 8 ms), mais il reste des sauts quand le fil du Worker ne tourne pas pendant plus de deux pas : un trou de plus de 33 ms entre deux pas joués, le tour lui-même court. Ils suivent la charge de la machine (que d'autres travaux de l'hôte tiennent, et que `lourd --calme` n'attend qu'au départ) : charge au plus 3,97, Chrome 3 sur 3 et Firefox 2 sur 3 ; charge au plus 5,48, Chrome 2 sur 3 et Firefox 0 sur 3. Le contrôle n'est pas changé (aucun saut dans la fenêtre cachée), ni le jeu (`rattrapage: 1`). C'est la question 30, mise à jour : accepter un saut qui suit un réveil en retard de plus de deux pas (le Worker le rapporte, le banc pourrait le rattacher comme un retard injecté), demander aux jeux un rattrapage d'au moins 2, ou garder le contrôle et son échec. Le lot n'est donc pas fini.
- **Le tour qui prend un point garde l'encodage** du point (10 à 18 ms dans Firefox) : `jeu_abi::encoder` écrit les octets de l'instantané un à un. L'écrire d'un bloc (`serialize_bytes`, les mêmes octets sur le fil) toucherait `partie` et `liaison`, donc l'épreuve et son plan court ; non fait ici.
- **Le seuil d'images de la fenêtre cachée (1 %) est choisi après avoir vu** Firefox donner une image à un onglet caché ; il est dit, avec son contrôle négatif.
- Les écarts du lot restent : Safari pas exécuté ; le fichier d'une marque non rejoué par l'épreuve ; `GET /banc` et `POST /banc/rapport` non écrits ; `soiree-relais preparer` au lot 10b ; le chien de garde n'a tué aucun Worker dans un navigateur.

### Ce que le lot suivant doit savoir

- `ReglagesSalle` gagne `tranche_du_point` (`TRANCHE_DU_POINT`, 128 Kio) ; `Soiree::regler_la_tranche_du_point` le baisse pour un test. Une case de point se commence (`Ecrivain::commencer_point`) et se finit (`finir_point`) ; **aucune autre case ne se scelle entre les deux** (`finir_point` le refuse, `Interne`) ; `ecrire_le_journal` finit d'abord un point en cours.
- `Autorite::secours_presse(t_us, avance_us)` dit à la coque de jouer le pas sans attendre l'image ; `Autorite::chronometrer(fn() -> f64)` et `durees_du_tour()` donnent les durées d'un tour (le banc seulement ; M3 au lot 11 peut s'en servir).
- En mode banc, le Worker écrit des lignes `tour` (un tour de plus de 10 ms ou un trou de plus de 30 ms), `retarde` (le pas où il gèle), et `par_image`, `par_minuterie` dans `etat` ; un rapport `etat` de plus à 580 pas depuis le point.
- Le banc rattache chaque saut à une reprise ou à un retard injecté (`rattacher_les_sauts`) : un mécanisme qui fait sauter la partie à dessein doit dire au banc le pas où il le fait.
- `soiree-banc arriere-plan --fois <n>` ; le lancer par `lourd --calme`, et dire la charge la plus haute : sous charge, le Worker d'un onglet caché manque des réveils.
- `controles.py --seulement-scenarios` joue les trois scénarios mutés ; il efface chaque paquet muté et reconstruit le banc.


## Lot 10b — Le démarrage et le jour J

Fait le 2026-11-07, de 12 h 20 à 14 h 10 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Chaque commande cargo, chaque scénario du banc et les contrôles négatifs sont passés par le portillon (`target/hote/lourd`), sauf des `cargo check`, `cargo clippy` et `cargo fmt` de quelques secondes sur une crate. Un scénario à la fois, jamais à côté d'une compilation ; chaque navigateur a été fermé par le banc, et le relais lancé par un test muté (voir plus bas) tué à la main. Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres (leurs modules au même sha256 qu'aux lots 7 à 10a).

### Ce qui est fait

**`relais`** : `aide` (sans entrées-sorties : les pages de téléphone venues du réseau sans WebSocket de leur adresse dans `PAGE_SANS_LIEN_MS`, 15 s, dites pendant `SOUVENIR_MS`, 10 min, au plus 128 adresses ; le jugement de ce que `socketfilterfw --getglobalstate`, `--getblockall` et `--listapps` écrivent pour le binaire qui tourne) et `GET /aide` (boucle locale ; le pare-feu relu au plus toutes les 10 s, par la coque) ; `preparer` (`soiree-relais preparer`, le paquet de la page depuis le commun dont le relais est compilé, la CLI `wasm-bindgen` vérifiée à `WASM_BINDGEN`, l'empreinte FNV-1a 128 ; la seule définition, que le banc appelle) ; `jouer` et `soiree-relais jouer` (`soiree-epreuve --construire`, le rapport cherché par empreinte, le refus qui dit comment éprouver, `--non-eprouve`, le paquet, le relais, `open`, `--sans-ouvrir`) ; `Jeux` lit et sert les rapports (`<empreinte>.json`, plausible : un objet JSON qui nomme son module), le catalogue dit `"eprouve"` ; `Reglages::annonce_du_banc` (une adresse annoncée sans écoute, au banc seulement). Le relais ne tire toujours que `liaison` du commun, et aucune crate de plus.

**`epreuve`** : `catalogue` (`--catalogue <dossier>` : sur un code 0 seulement, `<empreinte>.wasm` puis `<empreinte>.json` — empreinte, `reecrite` sous `PHASE_3`, `reecriture`, date UTC, plan, nom, places, code, verdict), passé à la copie native ; `sources::construire` et `--construire <jeu>` (la compilation du module de l'étape 1, la même fonction).

**`salle`** : `Comptes::entrees_ecran` (les entrées de l'écran que la partie a placées).

**`web/ecran`** : sans navigateur, testés en natif : `demarrage` (l'écran de démarrage, une `SceneFil` 1920 × 1080 : la liste du catalogue, le jeu de la salle marqué, « Jouer », le QR, l'adresse sans fragment, la salle, les téléphones, l'aide), `aide` (les quatre messages et quand les dire), `clavier` (raccourcis de la dernière scène, clavier brut, F8 au commun, Échap en plein écran selon P38), `son` (perdu avant le geste, périmé, une fois par identité) ; `messages` reconnaît un son déclaré. Dans le navigateur : `page/commun.rs` (le geste : plein écran, puis `navigator.keyboard.lock(["Escape"])`, contexte audio, `navigator.wakeLock`, par `Reflect` ; « Cliquer pour reprendre » après un rechargement, par `sessionStorage` ; « Plein écran quitté — cliquer pour revenir » ; l'insigne « NON ÉPROUVÉ » ; le catalogue et les rapports ; `GET /aide` toutes les 2 s ; la ligne `ecran-du-commun` au banc ; `window.__banc.centre()` et `rapporter()`), `page/audio.rs` (les quatre signaux synthétisés, les sons déclarés décodés après le geste, la latence de sortie) ; `page.rs` envoie au jeu ses touches (`DeLaPage::Ecran`), pose l'adresse du QR dans le contexte du rendu (les QR des jeux portent enfin l'adresse de la salle), dessine l'écran de démarrage par `Ecran::dessiner_le_commun`. Pendant l'écran de démarrage et « Cliquer pour reprendre », aucune touche ne va au jeu (F8 si) ; sous « Plein écran quitté », si.

**`web/autorite`** : l'empreinte réécrite vérifiée contre le rapport (§ 2.8, point 2) ; `ecran` au rapport d'état du banc.

**`banc`** : `aide` (`soiree-banc aide <module>…`), les pilotes cliquent (`Input.dispatchMouseEvent`, `input.performActions`) et appuient sur Échap ; le paquet par `relais::preparer` ; `WASM_BINDGEN` relu du relais ; dépend aussi de `reecriture` (la version que nomme un rapport).

**Dépendances** : aucune crate tierce nouvelle ; `web-sys` prend des fonctionnalités de plus dans `ecran` (le son, `MouseEvent`, `Storage`) ; le `Cargo.lock` du commun ne gagne que l'arête `banc → reecriture`, celui de `web/` ne change pas.

Documents : PHASE-3 (§ 2.2, § 2.5 et sa table des routes, § 2.8, § 2.9 Échap et le son, § 7 ligne du lot, § 8.2 `aide`, question 21, annexe des valeurs choisies), BESOINS-DES-JEUX (besoin 25, Échap en plein écran, dit aux agents de Punchline et de Tier list), CHANGELOG (« Phase 3, lot 10b »), `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **`--construire` n'est pas l'étape 1 entière** : la compilation du module de l'étape 1, par la même fonction (la même invocation de cargo), sans règles, clippy ni copie native ; le rapport cherché par empreinte garantit le reste.
2. **L'empreinte du paquet est un FNV-1a 128**, et non un BLAKE3 : `soiree-relais preparer` la calcule, et le relais ne tire aucune crate de hachage ; elle ne protège rien.
3. **`GET /aide` et `GET /jeux/<empreinte>.json`**, routes de boucle locale que PHASE-3 ne nommait pas ; **`Reglages::annonce_du_banc`**, au banc seulement.
4. **Changer de jeu depuis l'écran de démarrage n'est pas écrit** (`ChangerDeJeu`, lot 12, que sa ligne exige) ; une salle déjà ouverte reprend son module même si `?jeu=` (et donc `jouer`) en nomme un autre.
5. **Échap n'est constaté que dans Chrome** (Firefox refuse le plein écran au banc) ; sous « Plein écran quitté », le jeu reçoit ses touches : hors de Chrome, le second Échap lui arrive.

### Les tests qui le tiennent

- `crates/relais/src/aide.rs` (4) : `une_page_sans_lien_se_dit_apres_le_delai_sal99`, `les_adresses_suivies_sont_bornees`, `le_json_de_l_aide`, `le_pare_feu_se_lit_sans_droits_sal95_rem150` (les textes de `socketfilterfw` de la machine) ; `preparer.rs` (2) : FNV-1a 128 contre ses valeurs publiées, l'empreinte suit chaque fichier.
- `crates/relais/tests/relais/jour_j.rs` (4) : `le_catalogue_dit_quel_module_est_eprouve_p17`, `jouer_refuse_un_module_sans_rapport_p17`, `l_aide_se_lit_a_la_boucle_locale_sal99` (SAL-33 compris, l'annonce refusée hors du banc), `soiree_relais_jouer_refuse_puis_joue_non_eprouve_p17` (le binaire, un faux `soiree-epreuve` : code 1 et la manière d'éprouver ; puis « NON ÉPROUVÉ » et `"eprouve":false` servi) ; `http.rs` : le catalogue dit `eprouve`.
- `crates/epreuve/tests/sources.rs` : `sain_et_un_fautif_sans_static_depuis_leurs_sources` dépose `sain` (code 0) au catalogue, son module sous son empreinte et son rapport, `--construire` rend la même empreinte, et `invariant-viole` (code 1) n'y va pas ; `src/catalogue.rs` (2) : la date civile, le nom du plan ; `src/ligne.rs` : `le_catalogue_se_lit`.
- `web/ecran` (13 de plus) : `aide` (3 : SAL-99, SAL-95, sans réseau), `clavier` (3 : `echap_en_plein_ecran_p38`, `un_raccourci_envoie_son_action_sal152`, le clavier brut), `demarrage` (3 : `chaque_texte_tient_au_pire_cas`, mesuré comme E16 ; la scène dit la salle et l'aide ; le bouton sous le clic), `son` (4 : `un_effet_tombe_avant_le_geste_est_compte_perdu_sal174`, `une_identite_se_joue_une_fois_sal167_pun66`, `un_effet_perime_ne_se_joue_pas_sal176`, la borne) ; `tests/messages.rs` : `un_son_se_garde_pour_la_page`.
- `crates/banc/src/aide.rs` : `la_minute_de_l_aide_est_celle_de_l_ecran` (le banc relit la source de `web/ecran/src/aide.rs`).
- **Les contrôles négatifs** (`target/travail/lot-10b/controles.py`), douze mutations, chacune rouge pour la raison attendue, la source remise : la page sans lien sans délai ; un binaire inconnu du pare-feu pris pour autorisé ; `jouer` sans refus ; l'annonce hors du banc ; un rapport d'un autre module accepté ; le catalogue sur un code 1 ; un effet joué sans contexte ; un effet rejoué ; Échap sans verrou donné au jeu ; « Wi-Fi isolé » tout de suite ; la minute de l'aide changée d'un seul côté ; l'aide trop étroite pour sa boîte. Avec `--seulement-scenarios`, **le scénario muté** : le banc écrit au rapport une empreinte réécrite fausse, et le Worker la refuse (« n'est pas celle du rapport d'épreuve »), le scénario échoue. **Le contrôle négatif du scénario `aide`** (l'aide tue, `?fautif=taire-l-aide`) se joue à chaque passage, dans chaque navigateur.
- **Le scénario `aide`** (plus bas).

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-10b/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-10b/preuve.sh`, `empreinte.sh`, `final.sh`, `controles.py`), lancées à la suite par `final.sh` de 13 h 41 min 23 s à 14 h 05 min 43 s (`final.log` ; sa première ligne « 13:40:14 début » est celle d'un passage arrêté par moi après une faute de `clippy` dans le banc, corrigée, et d'un second arrêté avant d'avoir rien prouvé, pour une ligne fausse du script ; leurs preuves sont mises de côté dans `target/travail/lot-10b/preuves-interrompues/`). **Les 17 preuves ont la même empreinte au départ et à la fin** : le commun `83617b4`, `diff HEAD` `f26357c4448332f8`, hors `.md` `98e9d4c27e6c58ca`, non suivis (12) `4431bd93ad515e29` ; Punchline, Sidequest et Tier list sans aucune modification.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | `lourd python3 -I target/travail/lot-10b/controles.py` | code 0 : 12 mutations sur 12 rouges ; 96 s |
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | **code 0 : 51 réussis** (37 au lot 10a, 14 de plus), 0 échec ; 0,29 Go |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc -p relais --bin soiree-relais` | code 0 ; modules `82669f77…` (Punchline), `62043526…` (Sidequest), `84315a1b…` (Tier list) (`modules.txt`) |
| `aide` | `lourd /usr/bin/time -l target/debug/soiree-banc aide <Punchline> <Tier list> --navigateurs chrome,firefox,safari --travail …` | **code 0** : Chrome et Firefox réussis, Safari pas exécuté (« The session timed out while connecting to a Safari instance… »). Dans chacun : l'écran de démarrage avec le QR et « NON ÉPROUVÉ » ; un `GET /s/…` sans WebSocket, puis « Un téléphone a ouvert la page mais n'a pas pu se relier » à l'écran (15,8 s après) ; « Certains Wi-Fi isolent les appareils » à 61,8 s du QR ; 55 effets de Punchline comptés perdus avant le geste, aucun joué, aucun contexte audio ; le clic sur « Jouer » est un geste ; le rechargement montre « Cliquer pour reprendre » sur la partie reprise de la garde ; sans réseau, le relais démarre, sans QR, « Pas de réseau », le module avec son rapport accepté par le Worker (l'empreinte réécrite du navigateur égale à celle de wasmtime) et pas « NON ÉPROUVÉ » ; le contrôle négatif rouge sur l'aide seule (2 fautes). **Chrome** : plein écran, Keyboard Lock, son et verrou d'écran au clic ; Échap (Tier list) arrive au jeu (entrées de l'écran 0 → 1), le plein écran reste ; le plein écran quitté montre le voile, dont le clic le rend ; « Cliquer pour reprendre » rend son, plein écran et verrou d'écran. **Firefox** : le plein écran refusé au clic (« Fullscreen request denied », la page sans focus) : **Échap « pas exécuté »**, dit par le banc ; son et verrou d'écran rendus. Paquet `9f598e57…` (`paquet.txt` : `autorite_bg.wasm` `59733697…`, `ecran_bg.wasm` `5099807f…`) ; 328 s ; 0,78 Go résidents (le banc) ; charge 2,90 au départ, 3,53 au plus |
| `soiree` | `lourd /usr/bin/time -l target/debug/soiree-banc soiree <Punchline> --navigateurs chrome,firefox --sans-construire …` | **code 0** (non-régression du lot 10a sur la page changée) : Chrome et Firefox réussis, quatre sauts chacun, chacun rattaché à sa reprise ; 96 s ; 0,74 Go |
| `construire-epreuve`, `court-punchline`, `-sidequest`, `-tier-list` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` ; `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 0 ; puis code 1, 1, 1 : **la référence du lot 9, à l'identique** (chaque contrôle, son issue et son nombre d'évaluations : Punchline E2 et E16, Sidequest E16, Tier list E16) |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 704 réussis** (690 au lot 10a ; 14 de plus), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 585 s ; 0,82 Go résidents au plus. La charge a atteint 52,9 pendant (d'autres travaux de l'hôte) : une durée relevée, pas une mesure |
| `controle-scenario` | `lourd python3 -I target/travail/lot-10b/controles.py --seulement-scenarios` | code 0 : `reecrite-fausse` rouge (« n'est pas celle du rapport d'épreuve »), le banc reconstruit sur la source remise ; 141 s |

Le plan standard n'est pas exigé (§ 7 : le lot ne change ni ce que l'épreuve joue ni ce qu'elle juge ; le plan court le prouve, à l'identique). E20 n'est pas relancé (ni le moteur, ni la réécriture, ni les modules ne changent). `soiree-banc dessin` n'est pas relancé : `toile.rs` ne gagne qu'une méthode et un bras sans effet, l'exemple `dessin-banc` ne passe pas par `page.rs`. `arriere-plan` n'est pas relancé : le Worker ne change que par la vérification au chargement et un champ du rapport d'état.

### Les mesures

Aucune : ce lot ne mesure rien. Les délais du scénario (15,8 s, 61,8 s) sont des constats de l'aide, pas des mesures. Compilations : le banc tient 0,78 Go dans `aide`, `cargo test --workspace` 0,82 Go, `web/` 0,29 Go.

### Ce qui n'est pas fait, et pourquoi

- **Échap en plein écran n'est pas constaté dans Firefox** : Firefox 158 sans interface refuse le plein écran à un vrai clic du banc (`input.performActions`), la page n'ayant pas le focus. Essayé, sans effet, puis retiré : `full-screen-api.ignore-widgets`, `full-screen-api.allow-trusted-requests-only` à faux, `focusmanager.testmode`, et l'onglet activé (`browsingContext.activate`) avant le clic. La raison exacte du refus n'est pas lisible par BiDi (sa console ne la donne pas). La règle de la page (sans verrou, le premier Échap ne va pas au jeu) est tenue en natif, et le voile constaté dans Chrome (sur `document.exitFullscreen()`) ; le chemin entier « Échap → sortie → voile » reste à constater, avec interface ou dans Safari. **Safari** : pas exécuté (`safaridriver`, le réglage de l'hôte non touché).
- **Changer de jeu** depuis l'écran de démarrage (`ChangerDeJeu`) : lot 12. `soiree-relais jouer` sur une salle déjà ouverte avec un autre module reprend ce module.
- **Le README de chaque jeu** ne dit pas encore `soiree-relais jouer .` (P35) : les jeux n'ont pas été touchés ; la commande unique suppose aussi l'étiquette du commun (question 19, lot 14).
- **L'insigne « NON ÉPROUVÉ »** est constaté par la ligne que la page écrit et le texte de la scène de démarrage, pas par les pixels.
- Le pare-feu de macOS n'est éprouvé que par le jugement de ses textes (le banc ne change pas un réglage de la machine) ; sur la machine de l'hôte, il est allumé.
- La police d'emoji (question 28) et le reste des lots 11 à 13 (composants, pointeur, diagnostic) restent.

### Ce que le lot suivant doit savoir

- La page montre l'écran de démarrage à la première ouverture d'un onglet (`sessionStorage` `soiree-geste`), « Cliquer pour reprendre » ensuite ; le banc clique où `window.__banc.centre()` le dit (en JSON par `JSON.stringify`, pour BiDi), et lit la ligne `ecran-du-commun`. Un scénario qui veut le jeu au clavier doit d'abord cliquer « Jouer » (les touches ne vont pas au jeu sous l'écran du commun).
- Les touches vont au jeu par `ecran::clavier::pour_le_jeu` ; les composants (focus d'un champ, composition, TL-32, TL-34) devront s'y insérer au lot 11.
- Les effets d'une scène passent par `ecran::son::Effets` ; un rechargement repart en silence.
- `soiree-relais jouer` cherche `soiree-epreuve` dans `SOIREE_EPREUVE` ; les tests ne l'appellent qu'avec `--sans-ouvrir`. Le catalogue par défaut est `$HOME/.soiree/catalogue`.
- Le paquet se construit par `relais::preparer::construire_le_paquet` ; `banc::soiree::dernier_paquet` lit les dossiers de 32 chiffres sous `target/soiree-web/`.
- `soiree-banc aide` veut Punchline (l'effet perdu) et Tier list (Échap) ; il dure environ six minutes pour deux navigateurs, dont une minute d'attente par passage. Ne pas lancer un `controles.py` pour l'arrêter au milieu : une mutation interrompue reste dans la source (c'est arrivé une fois ici, à `catalogue.rs`, remise à la main et vérifiée).
- Les outils hors dépôt : `target/travail/lot-10b/controles.py`, `final.sh`, `preuve.sh`, `empreinte.sh` ; les essais du scénario dans `target/travail/lot-10b/essai*`.

## Lot 10b, correction après la vérification

Faite le 2026-11-07, de 14 h 15 à 16 h 15 environ (GMT), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »), en deux temps : un premier travail interrompu (les essais de Firefox avec sa fenêtre, `target/travail/lot-10b-corr/essai-ff*`, `ff/essai.py`, le catalogue au plan standard, les documents), repris ensuite. Chaque cargo, chaque scénario du banc, `jouer-reel` et les contrôles négatifs sont passés par le portillon (sauf quelques `cargo check -p` et `cargo fmt` de quelques secondes) ; aucun navigateur ni relais du banc n'est resté ouvert (`pgrep`). Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres, leurs modules au même sha256 qu'au lot 10b (`construire/modules.txt`).

### Les constats, et ce qui en est fait

1. **[bloquant] Échap sans verrou n'était constaté dans aucun navigateur.** Vérifié : seul `--headless` empêchait Firefox, et aucun essai avec sa fenêtre n'avait été fait. **Fait** : `soiree-banc aide --avec-interface` lance Firefox avec sa fenêtre (`banc::navigateur::Interface`, `banc::firefox`), et deux réglages de son seul profil (aucun réglage de la machine) : `dom.disable_window_flip` à faux, pour que le `window.focus()` que le banc fait avant de cliquer (`Firefox::montrer`) donne le focus à la page (il restait à la barre d'adresse, et le plein écran était refusé) ; `full-screen-api.macos-native-full-screen` à faux, parce que le plein écran natif de macOS y était quitté aussitôt entré, sans aucune touche (cause non trouvée). **Constaté dans Firefox 158.0.1** (preuve `aide`, Tier list, au premier passage et au passage court) : Échap sort du plein écran, rien n'arrive au jeu (aucune touche à la page, entrées de l'écran inchangées), l'écran du commun montre « Plein écran quitté — cliquer pour revenir », et le clic sur le voile rend le plein écran. La branche « Échap sans verrou » de `banc::aide::echap` a donc tourné. Pour qu'une sortie se rattache bien à Échap, le banc exige maintenant que le plein écran, donné au clic, tienne encore 1,5 s sans touche (`TENU_SANS_ECHAP_MS`, choisi) ; un plein écran donné puis quitté avant l'appui est une faute (« quitté sans Échap »), et non un refus ; un refus n'est « pas exécuté » que dans Firefox sans interface, une faute partout ailleurs, Firefox avec sa fenêtre compris. **Ce qui reste** : Échap dans le plein écran **natif** de macOS n'est pas constaté (la page ne distingue pas les deux : `fullscreenchange`, `document.fullscreenElement`) ; Safari pas exécuté (`safaridriver`, le réglage de l'hôte non touché). Dans Chrome, le voile reste constaté après `document.exitFullscreen()` (Échap y va au jeu sous Keyboard Lock, ce qui est la règle).
2. **[important] Un code 0 du plan court allait au catalogue, et se jouait sans « NON ÉPROUVÉ ».** Vérifié (code et test lus). **Corrigé** : seul un code 0 **du plan standard** va au catalogue (`epreuve::catalogue::refus_du_catalogue` ; E20 non exigé, `--navigateurs` laisse le plan standard ; un plan réglé est refusé) ; le relais refuse au lancement un rapport qui ne dit pas `"code": 0` et `"plan": {"nom": "standard", …}` (`rapport_plausible`). Le Worker et l'écran n'ont pas à le relire : ce qu'ils lisent est passé par le relais. Dit dans PHASE-3 (§ 2.8), CHANGELOG, AGENTS.md, `crates/README.md`. Le trou ne va pas plus loin (le vérificateur l'avait vu : un sous-ensemble de contrôles rend le code 3).
3. **[mineur] Le CHANGELOG affirmait Échap ailleurs sans réserve.** Corrigé : il dit maintenant où c'est constaté (Chrome 156, Firefox 158 avec sa fenêtre, dans le plein écran de Firefox et non celui de macOS) et que Safari ne l'est pas.
4. **[mineur] Le chemin réel de `jouer` n'était tenu par rien.** Vérifié. **Fait** : le faux `soiree-epreuve` du test du binaire annonce le vrai BLAKE3 de ses octets (calculé hors du relais, `target/travail/lot-10b-corr/blake3_court.py`, vérifié contre les vecteurs publiés) ; le test passe aussi par un faux `open`, premier dans le chemin, et vérifie l'adresse ouverte ; `jouer` compare les octets construits à ceux que le catalogue garde sous leur empreinte, et refuse s'ils diffèrent, même avec `--non-eprouve` (le relais ne hache toujours rien). **Le chemin réel** (le vrai `soiree-epreuve --construire`, sans aucun faux, sur `sain`) est constaté par la preuve `jouer-reel` : refusé sur un catalogue vide (code 1, la manière d'éprouver dite), puis, après le plan standard de `sain` déposé au catalogue, servi éprouvé (`"eprouve":true`, le relais tué ensuite). Ce n'est pas un test : il demande une épreuve au plan standard et la compilation d'un module (55 s), et `open` y est évité (`--sans-ouvrir`) pour ne pas ouvrir le navigateur de l'hôte.
5. **[mineur] « mesuré : moins de 1 Go » était ambigu.** Corrigé dans la ligne du lot (§ 7) : c'est le maximum résident de `/usr/bin/time -l` sur le seul processus du banc (0,78 Go au lot 10b, 0,74 Go ici) ; ses navigateurs, des processus à part, n'y sont pas, et l'empreinte d'un scénario avec eux n'est pas mesurée.

### Ce que les contrôles négatifs ont appris en chemin

- **Premier passage des preuves** (`target/preuves/lot-10b-corr/passage-1/`, `final-passage1.log`) : le contrôle `catalogue-sur-un-code-1` (le catalogue qui prend un code 1) **n'a pas rougi** `sain_et_un_fautif_sans_static_depuis_leurs_sources` : `invariant-viole` y est joué au plan court, refusé pour le plan avant de l'être pour le code. Le test exige maintenant que le refus dise « code 1 : rien ne va au catalogue » ; le test unitaire `seul_le_code_0_du_plan_standard_va_au_catalogue_p17` le tenait déjà.
- **Second passage** (`passage-2/`, `final-passage2.log`) : le contrôle `firefox-sans-fenetre` (Firefox lancé `--headless` malgré `--avec-interface`) **n'a pas rougi** : avec les deux réglages et `window.focus()`, Firefox **sans interface** donne lui aussi le plein écran, et Échap en sort, le voile le disant. C'est le focus de la page qui manquait au lot 10b, non la fenêtre. Le contrôle est remplacé par `firefox-sans-focus` (la page sans `window.focus()`), rouge. Et `plein-ecran-natif` rougissait, mais pour une raison mal dite (« refuse le plein écran (true) ») : `echap` distingue maintenant un plein écran donné puis quitté d'un refus.
- Les preuves des deux premiers passages sont gardées pour ce qu'elles montrent ; celles qui font foi sont le troisième passage, à la racine de `target/preuves/lot-10b-corr/`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

- § 2.8 : seul un code 0 du plan standard va au catalogue, et le relais refuse un autre rapport ; `jouer` compare les octets au catalogue ; le chemin réel de `jouer` est constaté sans test.
- § 2.9 (Échap, P38) et question 21 : constaté dans Firefox avec sa fenêtre ; les deux réglages du profil ; le plein écran natif non constaté ; la règle du banc (1,5 s sans touche, refus et sortie avant l'appui jugés) ; Firefox sans interface le donne aussi avec ces réglages, ce que le banc n'emploie pas encore (lot 12).
- § 7 (ligne du lot) et § 8.2 (`aide`) : `--avec-interface`, le plan standard, `jouer`, la mémoire relevée.

### Les tests qui la tiennent

- `crates/epreuve/src/catalogue.rs` : `seul_le_code_0_du_plan_standard_va_au_catalogue_p17` (P17 : standard, standard avec E20, court, réglé, codes 1 à 3) ; `crates/epreuve/tests/sources.rs` : `sain_et_un_fautif_sans_static_depuis_leurs_sources` (`sain` au plan court ne va pas au catalogue, au plan standard il y va ; « code 1 : rien ne va au catalogue » ; la vérification du catalogue dans `le_catalogue_tient_sain`).
- `crates/relais/tests/relais/jour_j.rs` : `le_catalogue_dit_quel_module_est_eprouve_p17` (un rapport indenté du plan standard accepté ; plan court, réglé, code 1 et `03` refusés, même d'un jeu nommé « standard ») ; `jouer_refuse_un_module_sans_rapport_p17` (des octets qui ne sont pas ceux du catalogue refusés, avec ou sans `--non-eprouve`) ; `soiree_relais_jouer_refuse_puis_joue_non_eprouve_p17` (le vrai BLAKE3, le faux `open`, l'adresse ouverte).
- **Les contrôles négatifs** (`target/travail/lot-10b-corr/controles.py`) : les douze du lot 10b (dont `catalogue-sur-un-code-1` remis au texte corrigé) et cinq de plus, chacun rouge pour la raison attendue : le catalogue sur le plan court (test unitaire, puis test depuis les sources) ; le relais qui prend un plan court pour une épreuve ; `jouer` qui ne compare pas les octets ; `jouer` qui n'ouvre pas l'écran. Avec `--seulement-scenarios`, trois scénarios mutés : `reecrite-fausse` (du lot), `firefox-sans-focus` (« refuse le plein écran à un vrai clic du banc (…), avec sa fenêtre »), `plein-ecran-natif` (« le plein écran, donné au clic, est quitté sans Échap avant l'appui »).
- **Le scénario `aide`** avec `--avec-interface`.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-10b-corr/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-10b-corr/preuve.sh`, `final.sh`, `final2.sh`, `controles.py`, `jouer-reel.sh` ; l'empreinte par `target/travail/lot-10b/empreinte.sh`), troisième passage lancé par `final.sh` de 15 h 36 min 10 s à 16 h 10 min 14 s (`final.log`). **Les 16 preuves ont la même empreinte au départ et à la fin** : le commun `83617b4`, `diff HEAD` `0960b9755d9847f7`, hors `.md` `f54060c15d6a4242`, non suivis (12) `687b31a3e59fe04e` ; Punchline, Sidequest et Tier list sans aucune modification. (Ce journal, PHASE-3 et AGENTS.md ont été écrits après : le `diff` entier change, pas celui hors `.md`.)

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur les crates `no_std` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 (`web/` n'a pas changé depuis le lot 10b) |
| `construire` | `lourd sh -c "cargo build -p banc --bin soiree-banc -p relais --bin soiree-relais && cargo build --release -p epreuve --bin soiree-epreuve"` | code 0 ; modules `82669f77…`, `62043526…`, `84315a1b…` (`modules.txt`, identiques au lot 10b) |
| `aide` | `lourd /usr/bin/time -l target/debug/soiree-banc aide <Punchline> <Tier list> --navigateurs chrome,firefox,safari --avec-interface --travail …` | **code 0** : Chrome 156 réussi (Échap sous Keyboard Lock arrive au jeu, entrées de l'écran 0 → 1, le plein écran reste ; le voile après `document.exitFullscreen()`) ; **Firefox 158.0.1 réussi, avec sa fenêtre, aucun constat « pas exécuté »** (Échap sans verrou sort du plein écran sans arriver au jeu, le voile le dit, son clic le rend, deux fois) ; Safari pas exécuté (« The session timed out while connecting to a Safari instance… ») ; dans chacun, l'aide (15,8 s ; 61,8 et 61,9 s), 55 effets perdus avant le geste, « NON ÉPROUVÉ », « Cliquer pour reprendre », sans réseau le rapport accepté par le Worker, le contrôle négatif rouge sur l'aide seule (2 fautes). Paquet `9f598e57…` (`paquet.txt`, le même qu'au lot 10b) ; 351 s ; 0,74 Go résidents (le banc seul) ; charge 3,57 au départ, 1,82 à la fin, 3,57 au plus |
| `jouer-reel` | `lourd target/travail/lot-10b-corr/jouer-reel.sh` | **code 0** : le vrai `soiree-epreuve --construire` sur `sain` ; refus sur un catalogue vide (code 1) ; le plan standard de `sain` au catalogue (code 0) ; `jouer` le sert éprouvé (`"eprouve":true`, module `df1f808b…`, 460 436 octets) ; 55 s |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 1, 1, 1 : chaque ligne de contrôle identique au lot 10b (et donc au lot 9 : E16 chez les trois, E2 chez Punchline) |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 51 réussis, 0 échec |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 705 réussis** (704 au lot 10b, plus `seul_le_code_0_du_plan_standard_va_au_catalogue_p17`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 683 s ; 1,59 Go résidents au plus ; la charge a atteint 16,7 (d'autres travaux de l'hôte) : une durée relevée, pas une mesure |
| `controles` | `lourd python3 -I target/travail/lot-10b-corr/controles.py` | code 0 : 17 mutations sur 17 rouges ; 216 s |
| `controle-scenario` | `lourd python3 -I target/travail/lot-10b-corr/controles.py --seulement-scenarios` | code 0 : 3 scénarios mutés sur 3 rouges, chacun pour sa raison, le banc reconstruit sur la source remise ; 307 s |

Le plan standard des trois jeux n'est pas exigé (le lot ne change ni ce que l'épreuve joue ni ce qu'elle juge : seul le dépôt au catalogue change ; le plan court le prouve, à l'identique). `soiree-banc soiree` n'est pas relancé : ni la page ni le Worker ne changent, et Firefox sans `--avec-interface` part comme avant (`--headless`, le même profil). E20, `dessin` et `arriere-plan` non plus.

### Les mesures

Aucune. Les délais de l'aide sont des constats ; les durées et les mémoires des preuves sont relevées, sous une charge qui a dépassé 3 pendant `tests` et `court-*`.

### Ce qui n'est pas fait, et pourquoi

- **Échap dans le plein écran natif de macOS** : non constaté ; le banc garde Firefox dans son plein écran à lui (le natif y est quitté aussitôt entré, sans touche, cause non trouvée ; peut-être une fenêtre qui n'est pas celle de l'application au premier plan, quand Firefox est lancé par un processus sans interface). À constater à la main le jour J, ou au lot 12.
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage « Autoriser l'automatisation à distance » de l'hôte n'est pas touché).
- **Firefox sans interface** juge Échap avec les deux réglages et `window.focus()`, mais le banc ne s'en sert pas sans `--avec-interface` (non éprouvé sur les autres scénarios) : `--avec-interface` ouvre une fenêtre sur l'écran de l'hôte, qu'elle couvre en plein écran par moments.
- Le chemin réel de `jouer` n'est tenu par aucun test automatique (une preuve, `jouer-reel`) ; `open` réel n'est lancé par rien.
- Le reste du lot 10b ne change pas : `ChangerDeJeu` (lot 12), le README des jeux (P35, question 19), l'insigne lu par ce que la page dit afficher, le pare-feu jugé par ses textes.

### Ce que le lot suivant doit savoir

- La preuve d'Échap hors de Chrome est `soiree-banc aide … --avec-interface` : Firefox ouvre sa fenêtre sur l'écran de l'hôte. Sans l'option, Firefox dit Échap « pas exécuté ». Le lot 12 peut essayer les deux réglages et `window.focus()` sans interface, en vérifiant `arriere-plan` (`window.focus()` peut rendre visible un onglet que le banc veut caché).
- `soiree-epreuve --catalogue` ne dépose qu'un code 0 du plan standard : un test qui veut un module au catalogue joue le plan standard (`sain` : `jouer-reel`, qui le construit deux fois et joue son plan standard, a duré 55 s, compilation déjà faite ; une durée relevée) ; le relais refuse au lancement tout autre rapport, et `jouer` refuse des octets qui ne sont pas ceux du catalogue.
- `target/travail/lot-10b-corr/jouer-reel.sh` rejoue le chemin réel de `jouer` sans rien ouvrir.

## Lot 11 — Tier list jouable

Fait le 2026-11-07, de 14 h 05 à 16 h 15 environ (GMT), en deux sessions (la première, interrompue, a écrit l'essentiel de `web/ecran` et de `salle` ; la seconde l'a fait compiler, a écrit le banc, joué et mesuré), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari 27.2 « pas exécuté »), dans le worktree `soiree-lot-11` (branche `refonte-lot-11`), pendant que `soiree/` construisait le lot 10b. Chaque cargo, chaque scénario et chaque mesure sont passés par le portillon (`target/hote/lourd`), sauf quelques `cargo check -p` et `cargo fmt` de quelques secondes ; un scénario à la fois ; chaque navigateur fermé par le banc. Rien n'est commité. **Seul le commun est modifié** ; aucun jeu n'est construit : le module de Tier list est celui de `target/modules-figes/` (`84315a1b…`, commit `9827f39`) ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres.

### Ce qui est fait

**`web/ecran`** (la colonne « contenu » du lot 11) :

- sans navigateur, testés en natif : `deplacables` (le glisser : seuil euclidien de TL-47, place visée dans l'ordre de l'état sans l'objet tenu de TL-39, dernière place valide hors des zones de TL-43, poubelle, annulation par Échap et `pointercancel`, défilement au bord selon le temps écoulé et plafonné au réveil de TL-40 à TL-42 ; le plateau lu dans la scène aux boîtes dessinées) ; `saisie` (la saisie coupée à la limite dans l'unité du champ, entre deux graphèmes, TL-27 et TL-28 ; un champ qui a le focus n'est réécrit que si le jeu a changé sa valeur, TL-30, TL-32, TL-33 ; **les échos en retard** : la page garde chaque texte envoyé dont l'écho n'est pas revenu, `ECHOS_ATTENDUS_MAX` 64) ; `cibles` (le bouton du dessus sous un clic, la modale qui ne laisse viser que son panneau et se ferme au dehors, les raccourcis `CMD_OU_CTRL`) ;
- dans le navigateur : `composants` (`Champ`, un `<input>` ; `EnPlace`, un `contenteditable="plaintext-only"`, du texte jamais du HTML ; `Selecteur`, un `<input type=file>` invisible sur la boîte du bouton que le jeu dessine ; `Lien`, un `<a download>` vers les octets de la ressource ; les polices du jeu enregistrées pour le DOM ; les éléments sous une modale ouverte cachés) ; `interaction` (la file des événements du DOM ; clic, pointeur, clavier, raccourcis qui cèdent au champ et à la composition, dépôt et collage de fichiers n'importe où, défilement, taille de la fenêtre ; la page qui défile, `FormeEcran::Page` ; la géométrie de la scène) ; `demandes` (`Image` : décodée par le navigateur, recadrée en carré, réduite par une toile hors écran, `convertToBlob` en WebP ou PNG ; `Lire`, la taille vérifiée avant ; `Telecharger`, dans l'activation du geste, sinon un bouton du commun la redemande, P24 ; `Rendu`, par bandes de 1 024 pixels, une image présentée entre deux, TL-57, et le compte de TL-56) ; dans `toile`, ce qu'il faut à la page qui défile (`echelle_page`, `defilement`, `cadre_fixe`), au rendu hors écran (`dessiner_hors_ecran`) et à une image décodée sans tenir l'écran (`recevoir_sans_attendre`, `poser_image`) ; en mode banc, `window.__banc.composants()` (les boutons, les composants, les zones et les objets du glisser, la poubelle, la modale, en pixels du document) et les options de test `?bots=non` et `?fautif=activation-perdue`.

**`web/rendu`** : `boites` (la boîte de chaque nœud en pixels), le glisser dessiné (`GlisserVu` : l'objet à 20 %, sa copie à 80 % qui suit le pointeur, la marque de la place visée).

**`web/autorite`** : la ligne `point` du banc (M3 : la taille de la case, l'instantané, l'encodage, le scellement par tranches, la plus longue tranche, la mise en file de l'envoi).

**`salle`** : `Ligne` donnée par l'autorité à sa garde (`VersRelais::AjouterLigne`), `Fait` rendu au jeu ; les images qu'une scène ou un rendu cite envoyées à la page avant eux, une fois chacune (`ecran::images_citees`, `EnJeu::ressources`) ; `CommandeEcran::Bots` (le banc seulement ; `PROTOCOLE` 3) ; `MesurePoint` et `Autorite::mesure_du_point` (le banc seulement) ; `RelaisEnMemoire::flux`. **`relais`** : `OpGarde::AjouterLigne`, `Dossier::ajouter_ligne` (`<garde>/<SALLE>/flux/<nom>`, synchronisé), `FLUX_MAX` 16 Mio. **`liaison`** : `LIGNE_MAX` 64 Kio, `flux_valide`.

**`banc`** : `ecran` (`soiree-banc ecran tier-list`, ci-dessous) ; les pilotes de Chrome et de Firefox savent donner des fichiers à un sélecteur (`DOM.setFileInputFiles`, `input.setFiles`), jouer des gestes de souris (`Input.dispatchMouseEvent`, `input.performActions`), taper un texte et une touche, régler la taille de la fenêtre (`Emulation.setDeviceMetricsOverride`, `browsingContext.setViewport`), et envoient les téléchargements dans `<profil>/telechargements` (`Browser.setDownloadBehavior` ; les préférences `browser.download.*` du profil de Firefox) ; `soiree::ouvrir` prend une requête de plus pour l'adresse de la page.

**Le scénario `soiree-banc ecran tier-list`**, dans Chrome (WebGPU), Chrome (WebGL2 forcé) et Firefox (WebGL2) : la vraie autorité dans le Worker, ses bots arrêtés, une fenêtre de 1 280 × 1 000 ; six images données au sélecteur « Add Images », dans la réserve, dessinées ; deux tuiles glissées vers les rangs S et A, une à la poubelle ; le titre tapé en place ; la page **rechargée**, le plateau retrouvé tel quel, images comprises, la partie reprise de la garde ; « Export Image (PNG) », le rendu, puis le lien du commun cliqué : `tierlist.png` **capté**, un PNG de 2 400 pixels de large ; « Download Backup » **capté** (par le geste) ; Reset, puis la sauvegarde redonnée à « Restore Backup » : le plateau d'avant revient ; puis le **plateau plein** (M3, M8). Le banc juge ce que la page montre et les fichiers reçus ; une panne, une relance du chien de garde, une page qui ne répond pas sont des fautes. Son contrôle négatif : `--fautif activation-perdue`.

**Dépendances** : aucune crate tierce nouvelle. `ecran` prend `partie` (du commun) et `unicode-segmentation` 1.13.3, la version du commun, déjà dans le `Cargo.lock` de `web/` par `partie` (le `Cargo.lock` de `web/` ne gagne que ces deux arêtes) ; `web-sys` prend des fonctionnalités de plus dans `ecran`. Le banc écrit ses PNG d'essai sans compression (des blocs « stored » de zlib), sans crate.

Documents : PHASE-3 (§ 7, la note « Écrit au lot 11 » sous le tableau ; § 8.2, la ligne `ecran tier-list` ; § 8.6, M3 ; question 31), MESURES-PHASE-3 (« Lot 11 »), CHANGELOG (« Phase 3, lot 11 »), `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **Le contrôle négatif de `ecran tier-list`** : PHASE-3 disait « l'export capté manque ». Chez Tier list, l'export se télécharge par le lien du commun (`ComposantVu::Lien`), sans `Telecharger` ; `Telecharger` sert à la sauvegarde : c'est **la sauvegarde captée qui manque** (§ 8.2 corrigé).
2. **`Ligne` ne passe pas par la page** : l'autorité la donne à sa garde, comme le § 2.7 le dessinait (`AjouterLigne`) ; PHASE-3 ne disait pas où le relais l'écrit, ni ses bornes, ni qu'une ligne peut s'écrire deux fois (le relais n'accuse pas une ligne) : écrit dans la note du § 7.
3. **Le glisser se dessine par une marque**, la tuile ne se déplace pas en direct : Tier list n'utilise pas `Deplace::Survol` (un choix du jeu, CAHIER 4.7) ; écrit dans la note du § 7.
4. **Les bots du banc s'arrêtent** par une commande de la page (`CommandeEcran::Bots`, `PROTOCOLE` 3) : un coup de bot défairait ce que le scénario vérifie.

### Les tests qui le tiennent

- `web/ecran/tests/composants.rs` (11) : TL-39, TL-47, TL-36/TL-43, la poubelle et l'annulation, TL-40 à TL-42, le plateau lu dans la scène, TL-27/TL-28, TL-30/TL-32/TL-33, **`un_echo_en_retard_ne_reecrit_pas_le_champ_tl33`** (né du scénario : dans Firefox, qui tape lettre à lettre, l'écho de « B » réécrivait le champ quand « Ban » était tapé, et renvoyait le curseur au début ; rouge sur l'ancien code), SAL-152 et la modale, les raccourcis.
- `web/rendu/tests/liste.rs` : `un_glisser_dessine_l_objet_attenue_sa_copie_et_la_marque_tl47`.
- `crates/salle/tests/devine.rs` : `une_ligne_de_resultats_va_au_flux_de_la_garde_et_non_a_la_page` (rouge si la ligne va à la page : aucune ligne dans le flux) ; `la_commande_bots_arrete_les_coups_du_banc`. `crates/salle/tests/ecran.rs` : les octets figés et la seconde lecture de `CommandeEcran::Bots`, `PROTOCOLE` 3 ; `les_images_citees_par_une_scene`.
- `crates/relais/tests/relais/garde.rs` : `une_ligne_d_un_flux_s_ajoute_sur_disque_par_la_garde` (l'ordre, un nom hors du dossier, une capitale, une ligne trop longue ignorés sans couper la garde).
- `crates/banc/src/ecran.rs` : `un_png_brut_dit_sa_taille`.
- **Le scénario et son contrôle négatif** (ci-dessous).

### Les vérifications, et l'index des preuves

Dans `target/preuves/worktree-lot-11/lot-11/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `duree.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-11/final.sh`, `preuve.sh`, `empreinte.sh`), lancées à la suite par `final.sh` de 15 h 24 min 46 s à 16 h 01 min 06 s (`final.log` ; un premier passage, arrêté au défaut de clippy d'une addition dans `banc::ecran`, est effacé). **Toutes ont la même empreinte du commun au départ et à la fin** : `83617b4`, `diff HEAD` hors `.md` `1cb9d6d326db7651`, non suivis (8) `ecc30f9c48da21de` (le `diff` entier change pendant les preuves : les `.md` s'écrivaient). Punchline, Sidequest et Tier list sans aucune modification. Le paquet `web/` construit par `ecran` : `ab78d3983c1039cd…` (`autorite_bg.wasm` `4f92436c…`, `ecran_bg.wasm` `c1aeddd5…`, sha256).

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | **code 0 : 49 réussis** (37 au lot 10a ; 12 de plus : les 11 de `composants`, dont 10 de la première session, et celui de `liste`), 0 échec ; 0,35 Go |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 |
| `ecran` | `lourd /usr/bin/time -l target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --travail …` | **code 0 : Chrome WebGPU, Chrome WebGL2 forcé et Firefox réussis**, chaque étape faite (import, trois glissers, titre en place, rechargement, export et sauvegarde captés, restauration, plateau plein, M3, M8) ; Safari pas exécuté (« The session timed out while connecting to a Safari instance… ») ; 136 s ; 0,87 Go (le banc) |
| `ecran-fautif` | `lourd … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --sans-webgl2 --plein 0 --fautif activation-perdue --sans-construire …` | **code 1, pour la raison attendue** dans les deux navigateurs : « sauvegarde : la sauvegarde n'est pas captée (1 téléchargement(s) laissé(s) au bouton du commun) » ; tout ce qui la précède réussi |
| `ecran-mesure` | `lourd --calme … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --plein 64 --sans-construire …` | code 0, les trois passages réussis ; charge 2,12 au départ, 2,32 à la fin, 2,92 au plus : **M3 et M8** (`MESURES-PHASE-3.md`, « Lot 11 ») |
| `dessin-construire`, `dessin` | `lourd … soiree-banc dessin <les trois modules> --navigateurs chrome --fois 1` (pour construire l'exemple), puis `lourd --calme … --navigateurs chrome,firefox,safari --fois 5 --sans-construire` | code 0, code 0 : six passages réussis (pire image de pire cas : Chrome 13,0 ms, Chrome WebGL2 15,6 ms, Firefox 20,9 et 18,8 ms), les deux contrôles négatifs voient l'oubli (Punchline 3 pixels, Sidequest 1), Safari pas exécuté ; charge au plus 2,09. Relancé parce que `toile` change (le cadre de la page) |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 695 réussis** (690 au lot 10a, correction ; 5 de plus : 2 de `devine`, 1 de `salle/tests/ecran.rs`, 1 du relais, 1 de `banc::ecran`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 977 s, sous la charge d'autres travaux de l'hôte (17,5 au plus) ; 2,08 Go résidents au plus |
| `soiree-punchline` | `lourd --calme target/debug/soiree-banc soiree <punchline.wasm> --navigateurs chrome,firefox --sans-construire …` (après `final.sh` : l'autorité change pour tous les jeux) | **code 1** : Chrome réussi ; **Firefox en échec, un saut (48, 49) qu'aucune reprise n'explique**, au début de la partie, sans ligne `tour` du Worker qui le dise (aucun tour de plus de 10 ms, aucun trou de plus de 30 ms rapporté) ; **la charge a atteint 3,35 pendant le tour** (d'autres travaux de l'hôte). Les quatre reprises de même forme que wasmtime, la chaîne entière rejouée |
| `soiree-punchline-refait` | la même commande, aussitôt | **code 0** : Chrome et Firefox réussis, quatre sauts dans chacun, chacun rattaché à sa reprise ; charge au plus 2,57 |

**Le plan court des trois jeux n'est pas relancé** : `epreuve` ne dépend, parmi les crates que le lot change, que de `liaison` (`cargo tree -p epreuve -e normal`), où le lot n'ajoute qu'une constante et une fonction qu'elle n'appelle pas (`grep` : aucune occurrence dans `crates/epreuve`) ; les modules ont le même sha256 qu'aux lots 7 à 10a. Le plan standard n'est pas exigé (§ 7). E20 n'est pas relancé (ni le relevé, ni le rejeu, ni les modules ne changent). `soiree-banc soiree` est relancé sur Punchline après `final.sh` (l'autorité change pour tous les jeux : chaque scène envoyée est décodée pour les images qu'elle cite) ; `arriere-plan` ne l'est pas : l'autorité n'y change que pour une demande `Ligne` (qu'aucun des trois jeux ne fait), les ressources citées (une scène de Punchline ou de Sidequest ne cite que des assets, déjà envoyés) et des mesures du banc.

### Les mesures

M3 et M8 (`MESURES-PHASE-3.md`, « Lot 11 ») : sur le plateau plein de Tier list, un point de **18 à 19,2 Mio** ; son scellement par tranches au plus 2,3 ms chacune ; **l'encodage de l'instantané (41 à 112 ms) et la fin du point (38 à 86 ms) restent chacun dans un tour**, comme la case de journal d'une grande entrée (73 à 134 ms) : au-delà du seuil de R2 (question 31). L'export du plateau plein en 111 à 123 ms, aucune image de la page au-delà de 33 ms, 0 et 1 image perdue dans Chrome. Les compilations (pas des mesures) : le banc tient 0,87 Go pendant `ecran`.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **R2 n'est pas tranché** : le chiffrement est déjà hors du tour, mais l'encodage de l'instantané, la fin du point et une grande case de journal tiennent le Worker de 38 à 134 ms. Les sortir du tour touche `jeu_abi::encoder` (donc l'épreuve et son plan court) ou la garde ; c'est la question 31 au mainteneur. Rien n'est changé ici.
- **Le glisser ne déplace pas la tuile en direct** : le commun dessine une marque ; Tier list ne s'abonne pas à `Survol` (à son agent de le décider ; le commun l'offre).
- **Le défilement au bord (TL-40 à TL-42) n'est éprouvé qu'en natif** (`le_defilement_depend_du_temps_et_du_plafond_tl41_tl42`) : le scénario évite la bande des 70 pixels (constaté au premier essai : une tuile lâchée près du haut d'une fenêtre de 600 pixels tombait au rang suivant, la page ayant défilé pendant la pause, ce qui est le comportement voulu). Un geste du banc qui le juge resterait à écrire.
- **Ce que le scénario ne joue pas** : le collage et le dépôt de fichiers (le même chemin que le sélecteur, `fichiers_recus`, non éprouvé dans un navigateur), une composition IME, les réglages d'un rang (sa modale, sa zone de texte), Undo et Redo, un raccourci clavier (Échap pendant un glisser), le champ « Or type a name… » ; un caractère hors du plan de base tapé dans Firefox (`input.performActions` ne le tape pas ; le titre n'en a donc pas). Le son, le plein écran et le diagnostic viennent aux lots 10b à 13.
- **Les octets des fichiers reçus sont perdus au rechargement** (§ 2.9) : une demande `Image` ou `Lire` relancée après un rechargement rend `Interne` ; le scénario ne recharge pas pendant un import.
- **Un saut de Firefox au début d'une soirée de Punchline, sous charge** (`soiree-punchline` : charge 3,35 au plus) : non reproduit au calme (`soiree-punchline-refait`), et le Worker n'a rien rapporté qui l'explique ; le banc ne rapporte pas le trou qui précède le premier pas qu'il compte, ni le moment où l'onglet devient visible. Non expliqué ; à surveiller dans les soirées suivantes.
- `/favicon.ico` répond toujours `404` (une ligne « journal error » de Chrome, que le banc ne compte pas).

### Ce que le lot suivant doit savoir

- `soiree-banc ecran tier-list <module> [--navigateurs …] [--plein N] [--sans-webgl2] [--fautif activation-perdue] [--sans-construire] [--travail …]` ; les fichiers captés sont gardés dans `<travail>/profils/captes/` (sortis du dossier des téléchargements du profil, effacé à la fermeture) ; les lignes de chaque passage dans `<travail>/<navigateur>/lignes.txt`.
- Les pilotes savent maintenant `fichiers`, `souris` (`chrome::GesteSouris`), `texte`, `touche`, `fenetre` et `telechargements` : la page des téléphones (lot 12) peut s'en servir. **Un fichier donné à un sélecteur se nomme par un chemin absolu** (le navigateur le lit depuis son propre dossier : un chemin relatif donne un fichier vide, « The source image could not be decoded »). **`BiDi` ne rend telle quelle qu'une valeur primitive** : un tableau évalué dans Firefox revient sérialisé ; passer par `JSON.stringify`.
- Une demande `Ligne` ne va jamais à la page ; un flux de résultats se lit dans `<garde>/<SALLE>/flux/<nom>`. `soiree-relais preparer` et l'écran de démarrage (lot 10b) n'ont rien à en faire.
- La fusion avec le lot 10b (dans `soiree/`) touchera `web/ecran/src/page.rs` (le bandeau, l'écran du commun), `salle::ecran` (`PROTOCOLE` : 3 ici ; le lot 10b peut l'avoir monté de son côté), `salle::autorite` (`traiter`, `vues`) et `banc::soiree` (`ouvrir` prend ici une requête).
- Les outils hors dépôt : `target/travail/lot-11/final.sh`, `preuve.sh`, `empreinte.sh` ; les essais du lot dans `target/travail/lot-11/essai1` à `essai12`.

## Lot 11, correction après la vérification

Faite le 2026-11-07, de 16 h 40 à 17 h 10 environ (GMT), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »), dans le worktree `soiree-lot-11`. Chaque cargo, chaque scénario est passé par le portillon (sauf `cargo fmt`) ; un scénario à la fois ; aucun navigateur, relais ni banc ne reste lancé (`pgrep`). Rien n'est commité. Seul le commun est modifié ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres ; les modules sont ceux de `target/modules-figes/`.

### Les constats, et ce qui en est fait

1. **[important] `soiree-banc soiree` rouge sur Sidequest.** Vérifié dans le code : la partie reprise joue jusqu'à `rattrapage` pas avant de sauter (`partie::regles::cible` : `cible = tete + rattrapage`, puis le `Saut`), et Sidequest déclare `rattrapage: 3` ; ses sauts commencent donc trois pas après la reprise. La règle écrite à la correction du lot 10a (`rattacher_les_sauts`, `de` valant `p` ou `p + 1`) ne valait que pour un jeu qui rattrape un pas. **Corrigé** : `de` va de `p` à `p + rattrapage` pour une reprise, de `p` à `p + rattrapage + 1` pour un retard injecté (le rattrapage déclaré par le module, au moins 1 ; inchangé pour Punchline et Tier list). Rejoué au calme : Sidequest **réussi** dans Chrome et Firefox, quatre sauts dans chacun, chacun exactement trois pas après sa reprise ; Punchline réussi, chaque saut un pas après la sienne. Le contrôle négatif du lot 10a (`saut-visible`, chez Punchline) juge la même règle à `rattrapage: 1`, inchangée : non rejoué ; le test unitaire le tient (un saut au-delà de `p + rattrapage` reste une faute).
   **Vu en chemin : `soiree` ne s'applique pas à Tier list.** Rejoué aussi sur Tier list (l'autorité change pour tous les jeux), il échoue dans les deux navigateurs, « aucune phase terminale » après 400 s : Tier list n'en a pas (un éditeur, une seule phase, `Etape::terminale` rend `Non`). Sans phase terminale, la partie n'est pas figée, G n'est pas exportée, et les reprises ne sont pas comparées à wasmtime (la sortie les marque « ÉCART » sans les avoir jugées). Ce n'est pas une régression (le scénario n'a jamais été joué sur Tier list ; le lot 10a l'y jouait en `--m12`) ; dit dans PHASE-3, § 8.2. Les reprises de Tier list sont jugées par `--m12 3` : **trois reprises par navigateur, chacune de même forme que wasmtime**, code 0.
2. **[mineur] `Ligne`.** Vérifié, et corrigé :
   - l'autorité ne dit plus `Fait(Ok)` qu'à une ligne que le relais écrira (`salle::autorite::reponse_a_la_ligne`) : au-delà de `LIGNE_MAX`, `Fait(Err(TropGros))` ; un nom de flux invalide ou une ligne qui contient `\n`, `Fait(Err(Refuse))` ; aucune ne part au relais ;
   - `liaison::ligne_valide` (au plus `LIGNE_MAX`, sans `\n`) : le relais et le relais en mémoire ignorent une ligne coupée d'un saut de ligne ;
   - **un flux plein ne coupe plus la garde** : une ligne qui ferait passer son flux au-delà de `FLUX_MAX`, la ligne d'un flux de plus que `FLUX_PAR_SALLE` (16, choisi), ou une ligne que le disque n'écrit pas, rend `Fait::LigneIgnoree` : rien ne part, le lien reste. Avant, `GardePleine` coupait le lien et l'autorité s'arrêtait (`Arret::Relais`). Le jeu, à qui l'autorité a déjà dit la ligne faite, ne le sait pas : l'autorité ne connaît pas la taille des flux sur le disque du relais (dit dans la note du § 7 de PHASE-3, et dans le CHANGELOG) ;
   - `FLUX_MAX` et `FLUX_PAR_SALLE` ont maintenant un test.
3. **[mineur] La phrase de MESURES-PHASE-3, « Lot 11 ».** Vérifiée : Chrome n'a refusé aucun lot (`--plein 64` arrête l'import ; la sortie ne porte pas « le dernier lot refusé ») ; Firefox a refusé le huitième. Recompté dans les sauvegardes captées : 939 282 octets d'images au plateau plein de Chrome (69 images, 0,896 Mio), 919 586 chez Firefox (61 images, 0,877 Mio ; 0,874 Mio sans les cinq images du début, le chiffre de la vérification). La phrase est corrigée.
4. **[mineur] Le plan court des trois jeux n'est pas relancé**, avec la même raison, toujours vraie : `epreuve` ne dépend, parmi les crates changées, que de `liaison` (`cargo tree -p epreuve -e normal`), qui gagne `LIGNE_MAX`, `flux_valide` et maintenant `ligne_valide`, qu'`epreuve` n'appelle pas (`grep` : aucune occurrence dans `crates/epreuve`) ; les modules ont le même sha256. Les jeux ne se construisent pas contre ce worktree (le `[patch]` vise `soiree/`).
5. **[mineur] L'étape « plateau plein : sa sauvegarde redonnée à Restore Backup » ne jugeait rien.** Corrigé : la ligne `Lire` doit valoir `ok` avec autant d'octets que le fichier, et le jeu doit dire « This backup is the tier list already open. » (`DEJA_OUVERTE` : il l'a lue, décodée et comparée au plateau ouvert ; une sauvegarde illisible ou refusée dirait autre chose). Constaté dans les trois passages. Le coût des vues qui redécodent chaque scène nouvelle (`images_citees`) est écrit à la question 31 (jusqu'à 56 ms de vues dans Firefox, au tour de la restauration pleine), non isolé dans M8.
6. **[mineur] Les tranches de 2,08 à 2,34 ms**, juste au-delà des 2 ms de R2 : écrit à la question 31. La coquille « larg'activation » est corrigée.

### Les écarts à PHASE-3, corrigés dans PHASE-3

- § 7, note « Écrit au lot 11 » : `reponse_a_la_ligne`, `ligne_valide`, `FLUX_PAR_SALLE`, `Fait::LigneIgnoree`, et ce que le jeu ne sait pas.
- § 8.2, `soiree` : un saut se rattache à sa reprise de `p` à `p + rattrapage` ; le scénario ne s'applique pas à un jeu sans phase terminale (Tier list). § 8.3, `arriere-plan` : le retard injecté de `p` à `p + rattrapage + 1`.
- Question 31 : la plus longue tranche, et le coût des vues.

### Les tests qui la tiennent

- `crates/banc/src/soiree.rs` : `un_saut_se_rattache_a_sa_reprise_selon_le_rattrapage_du_jeu` (le cas de Sidequest, pas 1 253 et saut de 1 256, rattaché à `rattrapage: 3` et non à 1 ; un saut au-delà de `p + rattrapage` reste seul ; une reprise n'explique qu'un saut ; un rattrapage de 0 vaut 1 ; SQ-62).
- `crates/salle/tests/devine.rs` : `l_autorite_refuse_au_jeu_une_ligne_que_le_relais_ignorerait`.
- `crates/relais/tests/relais/garde.rs` : `un_flux_plein_ou_de_trop_ignore_la_ligne_sans_couper_la_garde` (rouge sur l'ancien code : `GardePleine` coupait le lien, la liste ne revenait pas) ; `une_ligne_d_un_flux_s_ajoute_sur_disque_par_la_garde` gagne une ligne coupée d'un saut de ligne.
- Le scénario `ecran` (l'étape de la sauvegarde pleine jugée) et son contrôle négatif ; `soiree` sur Sidequest et Punchline ; `--m12` sur Tier list.

### Les vérifications, et l'index des preuves

Dans `target/preuves/worktree-lot-11/lot-11-corr/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `duree.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-11/correction/final.sh`, `preuve.sh`, et `target/travail/lot-11/empreinte.sh`), lancées à la suite par `final.sh` de 16 h 35 min 14 s à 17 h 04 min 09 s (`final.log` ; un premier passage, arrêté au `fmt` rouge d'une ligne de `banc::ecran`, est effacé), puis `m12-tier-list`. **Toutes ont la même empreinte du commun au départ et à la fin** : `83617b4`, `diff HEAD` hors `.md` `4592f6aa0d1d7583`, non suivis (8) `b8a0e12dd4a2c186` ; les trois jeux sans aucune modification. Le paquet `web/` : `b710551de21d4c73…` (`autorite_bg.wasm` `ac70fe8945c1b607…`, `ecran_bg.wasm` `28c7666ec7b05ca5…`, sha256). Les modules figés : ceux de `MODULES.txt`.

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 49 réussis, 0 échec |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 |
| `ecran` | `lourd /usr/bin/time -l target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --travail …` | **code 0 : Chrome WebGPU, Chrome WebGL2 forcé et Firefox réussis**, chaque étape faite ; la sauvegarde pleine (1 256 424 et 1 229 728 octets) lue entière et dite déjà ouverte par le jeu (69 et 61 tuiles) ; Safari pas exécuté (« The session timed out while connecting to a Safari instance… ») ; 134 s ; 0,83 Go (le banc) |
| `ecran-fautif` | `lourd … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --sans-webgl2 --plein 0 --fautif activation-perdue --sans-construire …` | **code 1, pour la raison attendue** dans les deux : « la sauvegarde n'est pas captée (1 téléchargement(s) laissé(s) au bouton du commun) » ; 75 s |
| `soiree-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <sidequest.wasm> --navigateurs chrome,firefox --sans-construire …` | **code 0** : Chrome et Firefox réussis ; les quatre reprises de même forme que wasmtime ; quatre sauts chacun, rattachés : Chrome (1 250, 1 267) à la reprise du pas 1 247, (1 317, 1 329) à 1 314, (1 930, 1 936) à 1 927, (1 984, 2 059) à 1 981 ; Firefox 1 249 → 1 252, 1 311 → 1 314, 1 932 → 1 935, 1 981 → 1 984 ; charge au plus 2,28 ; 94 s |
| `soiree-punchline` | la même, `<punchline.wasm>` | **code 0** : Chrome et Firefox réussis, quatre sauts chacun, chacun un pas après sa reprise ; charge au plus 2,96 ; 96 s |
| `soiree-tier-list` | la même, `<tier-list.wasm>` | **code 1, attendu** : « aucune phase terminale » dans les deux (Tier list n'en a pas) ; reprises non jugées ; 872 s |
| `m12-tier-list` | `lourd --calme target/debug/soiree-banc soiree <tier-list.wasm> --navigateurs chrome,firefox --m12 3 --sans-construire …` | **code 0** : trois reprises par navigateur au pire moment, chacune de même forme que wasmtime ; la chaîne entière vérifiée ; charge au plus 2,76 ; 68 s |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 698 réussis** (695 au lot, et les 3 nouveaux), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 446 s ; 1,78 Go ; charge jusqu'à 12,2 pendant (d'autres travaux de l'hôte ; ce n'est pas une mesure) |

**Non relancés, et pourquoi** : `ecran-mesure` (M3, M8) : le code mesuré (la prise, l'encodage, le scellement et l'envoi du point, le rendu de l'export) ne change pas ; l'autorité ne change que pour `Ligne` (Tier list n'en demande pas) et le banc que pour juger l'étape de la sauvegarde pleine ; les chiffres du lot tiennent. `dessin` : ni `web/rendu` ni `web/ecran` ne changent (une coquille dans un commentaire). `arriere-plan` : la règle des sauts n'y change pas pour Punchline (`rattrapage: 1`), le seul jeu sur lequel il se joue. Le plan court : constat 4. Les chiffres M12 de `m12-tier-list` (Chrome 251 à 305 ms, Firefox 484 à 679 ms) sont un sous-produit, pris au calme ; ils ne remplacent pas ceux de MESURES-PHASE-3, « Lot 10a, correction ».

### Ce qui n'est pas fait, et pourquoi

- Safari : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- `soiree` ne dit pas « sans objet » pour un jeu sans phase terminale : il attend 400 s et échoue. À écrire (le jeu le dit par sa déclaration de phases) ; non fait ici, hors des constats.
- Une ligne au-delà de `FLUX_MAX` ou d'un flux de trop est perdue sans que le jeu le sache (l'autorité ne connaît pas les flux sur disque) ; le relais en mémoire ne simule pas ces bornes.
- Le reste du lot 11 tient tel quel (R2 et la question 31, le défilement au bord en natif seulement, ce que le scénario ne joue pas, le saut de Firefox sous charge 3,35 non reproduit, `/favicon.ico`).

### Ce que le lot suivant doit savoir

- `rattacher_les_sauts` prend le rattrapage du jeu (`charge.declaration().rattrapage`) ; un mécanisme qui fait sauter la partie à dessein dit toujours au banc le pas où il le fait.
- `salle::autorite::reponse_a_la_ligne`, `liaison::ligne_valide`, `relais::garde::FLUX_PAR_SALLE`, `relais::coeur::Fait::LigneIgnoree` ; une ligne refusée au jeu ne part pas au relais.
- `soiree` ne se joue pas sur Tier list ; ses reprises se jugent par `--m12` et par `ecran tier-list`.

## Lot 11 : remontée du lot 10b, commit 010c5d5

Fait le 2026-11-07, de 17 h à 18 h 20 environ (heure de la machine, UTC), dans le worktree `soiree-lot-11` (branche `refonte-lot-11`) : `git merge --no-ff --no-commit refonte` (le lot 10b, `010c5d5`, sur la base commune `83617b4`, le lot 10a). L'arbre était propre avant la fusion. Rien n'est commité : la fusion reste en cours (`MERGE_HEAD` = `010c5d5`), les conflits résolus et mis à l'index ; un agent à part fait le commit de fusion.

### Les conflits, et comment ils sont résolus

Treize fichiers en conflit ; le reste s'est fusionné seul (dont `crates/banc/src/chrome.rs`, `lib.rs`, `crates/salle/src/autorite.rs`, `web/autorite/src/worker.rs`, `web/ecran/src/messages.rs`, `Cargo.lock`, `web/Cargo.lock`). Les deux côtés gardés partout :

- **`conception/LOTS-PHASE-3.md`** : les sections « Lot 10b » et « Lot 10b, correction » avant « Lot 11 » et « Lot 11, correction » (l'ordre des lots), puis celle-ci.
- **`CHANGELOG.md`** : la section du lot 11 au-dessus de celle du lot 10b (la plus récente d'abord), et une puce de plus au lot 11 pour ce que la remontée change.
- **`AGENTS.md`** : la ligne « Le jour J » (10b) puis « Tier list sur la télé » (11), cette dernière complétée (le scénario clique « Jouer »).
- **`crates/README.md`** : chaque ligne du tableau prend l'ajout des deux lots, 10b puis 11 (`epreuve`, `banc`, `relais`, `ecran`, la ligne des dépendances de `banc`) ; une phrase de plus pour `ecran` (la remontée).
- **`conception/PHASE-3.md`** : la ligne `soiree <jeu>` du § 8.2 prend la correction du lot 11, la ligne `aide` celle du lot 10b ; le reste s'est fusionné seul ; une phrase de plus au § 2.9 (les composants) dit les règles de la remontée (ci-dessous).
- **`crates/banc/src/bin/soiree-banc.rs`** : les deux sous-commandes (`aide`, `ecran`) dans l'aide du module, l'usage et la répartition.
- **`crates/banc/src/navigateur.rs`** : `touche_echap` et `cliquer` (10b), `fenetre`, `telechargements`, `fichiers`, `souris`, `texte`, `touche` (11). Rien n'est fondu : `touche(…, "Escape")` et `touche_echap` font la même chose par deux chemins, que les deux scénarios gardent.
- **`crates/banc/src/firefox.rs`** : les réglages du profil des deux lots (les téléchargements, et sous `Interface::Avec` le focus et le plein écran natif) ; `GesteSouris` et `Interface` importés.
- **`crates/banc/src/soiree.rs`** : `Session`, aux champs `pub(crate)` du lot 10b (les mêmes champs que le lot 11).
- **`web/ecran/Cargo.toml`** : les traits de `web-sys` des deux lots (`MouseEvent` une seule fois).
- **`web/ecran/src/lib.rs`** : les modules des deux lots, et leur description.
- **`web/ecran/src/toile.rs`** : `recevoir_sans_attendre` (11) laisse l'image à la page et le son à la page (10b) : `Recu::Image { .. } | Recu::Son { .. } | Recu::Laisse(_) => {}`.
- **`web/ecran/src/page.rs`** (huit conflits) : la `Page` a `toile` (non optionnelle, `pub(crate)`), `jeu` (11), `commun`, `son`, `systeme` (10b) ; `demarrer` lit l'adresse par `lire_l_adresse` (10b) et un `parametre` commun aux deux (`bots`, `fautif=activation-perdue` du lot 11) ; `commun::creer` vient après `Interaction::new` ; la file des messages du Worker garde le décodage d'image sans tenir l'écran (11) et y ajoute le son (10b : `effets_de_la_scene`, `Recu::Son`) ; `image` fait `avant_de_dessiner` (11) puis la scène du commun (10b) ; les crochets du banc ont `composants` (11), `centre` et `rapporter` (10b).

Les versions : `salle::ecran::PROTOCOLE` n'a été montée que par le lot 11 (3, `CommandeEcran::Bots`) ; `VERSION_CASES` n'a changé d'aucun côté : aucune montée de plus, aucun octet figé à réécrire. Les crates `jeu`, `jeu-abi` et `jeu-derive` n'ont changé d'aucun côté (`git diff 83617b4 refonte` et `83617b4 HEAD` vides sur elles) : les modules figés de `target/modules-figes/` (bâtis sur `83617b4`) restent ceux des deux lots, sha256 vérifiés.

### Ce que le lot 11 a dû changer pour ce qui est entré

Le lot 10b et le lot 11 avaient chacun leur écouteur du clavier, et le lot 10b met l'écran du commun par-dessus le jeu. Fusionnés tels quels, chaque raccourci serait parti deux fois, « Jouer » n'aurait plus reçu de clic (l'espaceur du lot 11, `z-index` 1, couvre la toile où le lot 10b l'écoutait), le voile serait passé sous l'espaceur, et le clic du geste serait allé aussi au jeu. Corrigé :

- **Une seule chaîne du clavier.** L'écouteur du lot 10b (`ecouter_le_clavier`) est retiré ; la file d'`interaction` (11) laisse passer F8, écarte une composition et un champ qui a le focus (TL-32, TL-34) sauf pour F8 (SAL-162), annule un glisser par Échap, puis appelle `page::touche` (10b, qui prend maintenant `&Page`, le code, les modificateurs, la phase et la répétition de l'événement de la file) : F8, P38, l'écran du commun qui couvre le jeu, les lignes `touche` du banc.
- **Le raccourci** : `clavier::pour_le_jeu` cherche par `cibles::raccourci` (11) ; l'ancien `memes_mods` du lot 10b ne reconnaissait pas `CMD_OU_CTRL` avec Maj (Cmd+Maj+Z, « Rétablir » de Tier list). Un raccourci prime sur le clavier brut (règle du lot 10b, écrite dans `clavier`) : le lot 11 envoyait les deux ; aucun des trois jeux ne demande le clavier brut (`git grep ClavierBrut` vide dans chacun). Test : `clavier::tests::cmd_maj_et_le_raccourci_avant_le_clavier_brut` ; les tests des deux lots (`echap_en_plein_ecran_p38`, `un_raccourci_envoie_son_action_sal152`, `le_clavier_brut`, `un_raccourci_suit_cmd_sur_mac_et_ctrl_ailleurs`) passent inchangés.
- **L'écran du commun couvre le jeu** (`page::le_commun_couvre` : tout sauf `Montre::Rien`) : aucun clic au jeu (`Evenement::Clic`), aucun glisser qui commence (un glisser commencé finit), aucun fichier donné ; sous l'écran de démarrage, l'espaceur et les composants du jeu sont cachés (`interaction::couvrir`, `display: none`). Les touches suivent la règle du lot 10b (rien au jeu sous l'écran de démarrage et « Cliquer pour reprendre » ; sous « Plein écran quitté », elles lui vont, P38).
- **Le clic du geste** : « Jouer » s'écoute sur le document, à la capture, en pixels CSS (la toile est en pixels de l'appareil depuis le lot 11 ; `centre_pour_le_banc` rend aussi des pixels CSS) ; un clic que la page provoque (`is_trusted` faux) n'est pas un geste, et l'écouteur ne panique pas si la page tient son état (`try_borrow`). « Jouer » et le voile arrêtent le clic (`stopPropagation`). Le voile et l'insigne passent à `z-index` 4.
- **`soiree-banc ecran tier-list`** clique « Jouer » après la scène de Tier list, et « Cliquer pour reprendre » après le rechargement (`banc::ecran::le_geste`, par `window.__banc.centre()` et la vraie chaîne d'entrées) : sans cela, rien ne va au jeu. Ce n'est pas affaiblir le scénario : c'est le geste qu'un joueur fait, et le scénario juge toujours tout le reste.

### Les vérifications, et l'index des preuves

Dans `target/preuves/worktree-lot-11/remonter/<nom>/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `duree.txt` (de bout en bout, attente du portillon comprise), `empreinte.txt`, `empreinte-fin.txt` ; script `target/travail/remonter/preuve.sh`). Toutes sur le même arbre : `HEAD` `c2588bd`, `MERGE_HEAD` `010c5d5`, `git diff HEAD` sha256 `4211abcd…`, hors `.md` `0f5696ee…`, aucun fichier non suivi ; modules figés `82669f77…` (Punchline), `62043526…` (Sidequest), `84315a1b…` (Tier list). Les empreintes de départ de `tests` et `ecran` sont vides, et leurs empreintes de fin n'ont pas la ligne « hors `.md` » : le script a été modifié pendant qu'ils démarraient. *Corrigé à la vérification* : `empreinte-code-pendant-tests-et-ecran.txt` est daté de 17:35:11, une dizaine de minutes avant le départ d'`ecran` (17:45:18) et de `tests` (17:46:21) ; il n'a donc été relevé ni « juste avant » ni « pendant », comme cette phrase le disait d'abord. La preuve tient quand même : le dernier fichier de code a été modifié à 17:34:19, et le diff complet de leurs empreintes de fin (`4211abcd…`) est celui de toutes les autres preuves. Le paquet `web/` joué (ajouté à la vérification) : `target/soiree-web/4fdd0221e12b12b58a1588e2c465a830`, construit à 17:45 par la preuve `ecran` à partir des sources fusionnées ; `aide`, `soiree-punchline` et `soiree-sidequest` l'ont repris par `--sans-construire` (aucun paquet plus récent alors).

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 713 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 415 s ; charge au plus 11,87 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 64 réussis (49 au lot 11, 51 au lot 10b ; plus `cmd_maj_et_le_raccourci_avant_le_clavier_brut`), 0 échec |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 (déjà à jour) |
| `ecran` | `lourd /usr/bin/time -l target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --travail …` (le paquet reconstruit) | **code 0** : Chrome WebGPU, Chrome WebGL2 forcé, Firefox WebGL2 réussis (« Jouer », import, glisser, poubelle, édition en place, « Cliquer pour reprendre » et le plateau retrouvé, export et sauvegarde captés, restauration, plateau plein de 64 images) ; Safari pas exécuté (`safaridriver`) ; 134 s ; 0,85 Go résidents pour le banc ; charge au plus 7,76 |
| `ecran-fautif` | `lourd … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --sans-webgl2 --plein 0 --fautif activation-perdue --sans-construire …` | **code 1, pour la raison attendue** dans les deux : « la sauvegarde n'est pas captée (1 téléchargement laissé au bouton du commun) » |
| `aide` | `lourd /usr/bin/time -l target/debug/soiree-banc aide <punchline.wasm> <tier-list.wasm> --navigateurs chrome,firefox,safari --avec-interface --sans-construire …` | **code 0** : Chrome 156 et Firefox 158.0.1 réussis (Échap sous Keyboard Lock arrive au jeu dans Chrome ; sans verrou dans Firefox avec sa fenêtre, il sort du plein écran, le voile le dit, son clic le rend) ; le contrôle négatif (l'aide tue) rouge comme attendu dans les deux ; Safari pas exécuté ; 346 s |
| `soiree-punchline`, `soiree-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <module> --navigateurs chrome,firefox --sans-construire …` | **code 0, code 0** : Chrome et Firefox réussis, quatre sauts chacun, chacun rattaché à sa reprise, trois marques F8 posées et rejouées (F8 par la file d'`interaction`) ; 107 s, 95 s |

Ces scénarios ne mesurent rien ici : les points (M3) et le travail long (M8) que `ecran` rapporte ont été pris sous une charge de 4 à 8 et ne valent pas mesure ; `soiree-punchline` a vu la charge monter à 3,17 (le portillon le dit), sans effet sur ce qu'il juge, qui n'est pas une durée.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Les contrôles négatifs scriptés des deux lots** (`target/travail/lot-10b-corr/controles.py`, `jouer-reel.sh`, `target/travail/lot-10a-corr/controles.py`) ne sont pas relancés : ils sont dans le `target/` de l'autre worktree, où ce lot ne lit ni ne lance rien. Le chemin réel de `jouer` reste tenu par `tests/relais/jour_j.rs` (dans `tests`), les scénarios ont leurs propres contrôles négatifs (`ecran-fautif`, l'aide tue).
- ~~**Aucun test automatique** ne vérifie qu'un clic sous l'écran du commun ne va pas au jeu~~ *fait à la correction de la remontée (ci-dessous)* : `soiree-banc ecran tier-list` le constate, avec deux contrôles négatifs, et à un rapport de pixels de 2.
- **Le plan court des trois jeux n'est pas relancé** (§ 7 : « Le plan court d'un lot prouve donc qu'il ne change rien ») : `cargo tree -p epreuve` ne tire, parmi les crates locales, que `jeu-abi`, `liaison`, `mise-en-page`, `moteur-commun`, `moteur-wasmtime`, `partie` et `reecriture` ; entre `010c5d5` et l'arbre fusionné, seule `liaison` change (`LIGNE_MAX`, `ligne_valide`, `flux_valide`), qu'`epreuve` n'appelle pas. L'épreuve est donc celle du lot 10b, dont le plan court a donné la référence à l'identique (raison ajoutée à la vérification).
- `dessin`, `arriere-plan`, `--m12` et `soiree` sur Tier list ne sont pas rejoués (ni le dessin ni la reprise ne changent ; `soiree` sur Tier list échoue toujours faute de phase terminale, lot 11, correction).

### Ce que le lot suivant doit savoir

- Une touche passe par la file d'`interaction`, puis par `page::touche` : un nouveau raccourci du commun se décide dans `clavier::pour_le_jeu`, un nouveau composant qui garde les touches dans `interaction::traiter_un`.
- Tout scénario qui joue sur la télé doit cliquer « Jouer », et « Cliquer pour reprendre » après un rechargement (`banc::ecran::le_geste`, ou `cliquer_le_commun` de `banc::aide`) ; F8 passe toujours.
- La toile est en pixels de l'appareil, les clics et `__banc.centre()` en pixels CSS.

## Lot 11 : remontée du lot 10b, correction

Fait le 2026-11-07, de 18 h 20 à 19 h environ (heure de la machine, UTC), dans le worktree `soiree-lot-11`, d'après la vérification de la remontée. La fusion reste en cours (`MERGE_HEAD` = `010c5d5`) ; rien n'est commité ; les changements sont mis à l'index avec ceux de la remontée.

### Les constats, et ce qui en est fait

1. **[important] Rien ne vérifiait la règle centrale de la remontée**, ni la conversion en pixels CSS × dpr. Vérifié : les scénarios ne jouaient la règle que dans le bon sens, et `fenetre` du banc fixait le rapport de pixels à 1 (`deviceScaleFactor: 1`, `devicePixelRatio: 1`). **Corrigé** :
   - **La page dit au banc chaque entrée qu'elle donne au jeu** : une ligne `au-jeu`, `par` `clic` (un clic passé le contrôle de `page::le_commun_couvre`), `appui` (un glisser qui commence), `fichiers` (des fichiers donnés au jeu) ; dans `interaction::traiter_un` et `pointeur`, au point même où la règle décide (mode banc seulement).
   - **`soiree-banc ecran tier-list` joue la règle dans les deux sens** (`banc::ecran`) : sous l'écran de démarrage, un clic sur la page du jeu loin de « Jouer » (le milieu du premier rang ; sous l'écran de démarrage la page ne défile pas, et les boutons de Tier list sont sous le bas de la fenêtre) et une image donnée au sélecteur « Add Images » : aucune ligne `au-jeu`, pas de geste, ce que la page montre du jeu (le plateau, la modale) inchangé (`sous_l_ecran_de_demarrage`) ; le clic sur « Jouer » : aucune ligne `au-jeu` (`rien_au_jeu`) ; après le rechargement, le clic sur « Cliquer pour reprendre » posé là où le jeu a son bouton Reset : aucune ligne `au-jeu`, et le plateau retrouvé tel quel. Pour que l'absence de ligne prouve quelque chose, l'import, le premier glisser et Reset doivent chacun en donner une (`vu_au_jeu`), sinon c'est une faute.
   - **Deux contrôles négatifs**, options de test de la page au banc seulement, comme `taire-l-aide` : `?fautif=geste-au-jeu` (`commun::Fautif::GesteAuJeu` : ni « Jouer » ni le voile n'arrêtent le clic du geste, `stopPropagation` retiré) et `?fautif=ne-couvre-pas` (`Fautif::NeCouvrePas` : `page::le_commun_couvre` toujours faux) ; `soiree-banc ecran … --fautif geste-au-jeu|ne-couvre-pas`. Le premier échoue au clic sur « Jouer » et au clic sur « Cliquer pour reprendre » (la ligne `au-jeu`, puis Reset joué : le plateau n'est pas retrouvé) ; le second sous l'écran de démarrage (le clic et l'image vont au jeu : la ligne `au-jeu`, et l'image arrive dans la réserve).
   - **`--dpr R`** : `Lance::fenetre` prend le rapport de pixels (`Emulation.setDeviceMetricsOverride`, `browsingContext.setViewport`). Le scénario constate `window.devicePixelRatio`, et que la toile fait la largeur de la fenêtre × R (`rapport_de_pixels`) ; `le_geste` exige que le point que la page donne pour « Jouer » tombe dans la fenêtre : une conversion oubliée des deux côtés (`centre_pour_le_banc` et l'écouteur), qui se compenserait, le sort de la fenêtre. Joué à 2 dans Chrome (WebGPU, WebGL2 forcé) et Firefox. Deux contrôles par mutation, scriptés (`mutation.sh`, le fichier rendu tel quel, sha256 vérifié) : la conversion oubliée partout (« Jouer » placé en (500, 1438), hors de la fenêtre de 1 280 × 1 000) et à l'écoute seulement (le clic ne tombe plus sur « Jouer » : pas de geste en 20 s), tous deux rouges à `--dpr 2` ; à `--dpr 1`, multiplier ou diviser par 1 ne change rien, d'où le constat.
   - `commun::ecouter` dépassait 100 lignes : l'écouteur de « Jouer » passe dans `ecouter_jouer`, inchangé.
2. **[mineur] Pourquoi le plan court n'est pas relancé** : vérifié (`cargo tree -p epreuve`, `git diff 010c5d5` sur ces crates : seule `liaison` change, par trois ajouts qu'`epreuve` n'appelle pas) et écrit dans « Ce qui n'est pas fait » de la remontée. Cette correction ne touche que `banc` et `web/ecran`, hors de l'arbre d'`epreuve` : la raison tient pour elle aussi.
3. **[mineur] L'empreinte du paquet `web/` joué** : vérifiée (`target/soiree-web/4fdd0221…`, 17:45) et ajoutée à l'index de la remontée. Ici, chaque empreinte de preuve donne le dernier paquet construit (le script de preuve la relève).
4. **[mineur] Les empreintes de départ de `tests` et `ecran`** : vérifié ; la phrase de la remontée qui disait l'empreinte du code relevée « juste avant » est corrigée (relevée à 17:35:11, dix minutes avant ; la preuve tient par le diff complet de leurs empreintes de fin et l'heure du dernier fichier de code).
5. **[mineur] Une touche traitée au prochain appel de `traiter` (F8 compris), où `prevent_default` n'agirait plus** : **écarté**, sans changement de code. Une touche du clavier (`is_trusted`) n'arrive que par la boucle d'événements, comme une tâche à elle ; la page ne tient son état que dans du code synchrone : aucun emprunt du `RefCell` n'est tenu à travers un `await` (`clippy::await_holding_refcell_ref`, actif par défaut, constaté le 2026-11-07 avec la chaîne d'outils du dépôt, et le clippy de `web/` en `wasm32` passe sous `-D warnings`), et ce que la page déclenche elle-même pendant qu'elle le tient est soit un clic (`a.click()`, `input.click()` : `is_trusted` faux, écarté par les écouteurs), soit un changement de focus (`blur()`), jamais une touche. `try_borrow_mut` ne peut donc pas échouer à l'arrivée d'une touche ; la file ne la garde que si un événement est arrivé pendant qu'un autre était traité, ce qui ne se peut pas pour une touche.
6. **[mineur] Ce qui n'est pas rejoué sur le code fusionné** (`arriere-plan`, `--m12` et `soiree` sur Tier list, les contrôles par mutation scriptés des lots 10a et 10b) : laissé tel que la remontée le déclare. Cette correction ne touche ni `worker.rs`, ni la reprise, ni l'autorité ; `aide`, `soiree-punchline` et `soiree-sidequest`, qui passent par le clic du geste et la file d'`interaction`, sont rejoués sur le paquet corrigé.

### Ce que les jeux voient

Rien : leur API ne change pas. `soiree-banc ecran` gagne `--dpr` et deux valeurs de `--fautif` ; la page, en mode banc seulement, les lignes `au-jeu` et deux options de test.

### Les vérifications, et l'index des preuves

Dans `target/preuves/worktree-lot-11/remonter-corr/<nom>/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `duree.txt`, `empreinte.txt`, `empreinte-fin.txt` ; script `target/travail/remonter-corr/preuve.sh`, qui relève aussi le dernier paquet `web/`). Toutes sur le même code : `HEAD` `c2588bd`, `MERGE_HEAD` `010c5d5`, `git diff HEAD` hors `.md` sha256 `374afd26…` (le diff complet change avec les documents de ce lot), aucun fichier non suivi ; modules figés `82669f77…` (Punchline), `62043526…` (Sidequest), `84315a1b…` (Tier list), inchangés. Le paquet `web/` joué : `target/soiree-web/bd4a20b0667a0a781c7ab428b6d86d05`, construit à 18:40:08 par la preuve `ecran` ; toutes les preuves suivantes le reprennent (`--sans-construire`, aucun paquet plus récent).

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 713 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 489 s, charge au plus 13,53 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 64 réussis, 0 échec |
| `construire-banc` | `lourd cargo build -p banc --bin soiree-banc` | code 0 |
| `mutations-dpr` | `target/travail/remonter-corr/mutation.sh dpr-oublie-partout`, puis `dpr-oublie-a-l-ecoute` (Chrome, `--dpr 2`, le paquet reconstruit sur le fichier muté) | **code 1 et code 1, pour la raison attendue** : « la page le place en (500, 1438), hors de la fenêtre » ; « « Jouer » : rien en 20s » ; `commun.rs` rendu (sha256 `83978ae4…` avant et après) ; joués avant les autres preuves, qui reconstruisent le paquet |
| `ecran` | `lourd /usr/bin/time -l target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --travail …` (le paquet reconstruit) | **code 0** : Chrome WebGPU, Chrome WebGL2 forcé, Firefox WebGL2 réussis, dont les trois « rien au jeu » et les trois lignes `au-jeu` attendues ; Safari pas exécuté (`safaridriver`) ; 141 s ; charge au plus 11,06 |
| `ecran-dpr2` | `lourd … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --plein 0 --dpr 2 --sans-construire …` | **code 0** : les trois passages réussis, au rapport de pixels 2 (la toile 2 560 pixels de l'appareil pour 1 280 CSS), « Jouer » et « Cliquer pour reprendre » cliqués en pixels CSS, tout le reste du scénario joué ; 39 s |
| `ecran-fautif-geste-au-jeu` | `… --navigateurs chrome,firefox --sans-webgl2 --plein 0 --fautif geste-au-jeu --sans-construire …` | **code 1, pour la raison attendue** dans les deux : « le clic sur « Jouer » est allé au jeu », de même pour « Cliquer pour reprendre », puis le plateau non retrouvé (Reset joué) |
| `ecran-fautif-ne-couvre-pas` | `… --fautif ne-couvre-pas …` | **code 1, pour la raison attendue** dans les deux : le clic et l'image sous l'écran de démarrage vont au jeu (`au-jeu` `clic` et `fichiers`), et l'image arrive dans la réserve |
| `ecran-fautif-activation-perdue` | `… --fautif activation-perdue …` | **code 1, pour la raison attendue** dans les deux : « la sauvegarde n'est pas captée » (les nouveaux contrôles y passent) |
| `aide` | `lourd /usr/bin/time -l target/debug/soiree-banc aide <punchline.wasm> <tier-list.wasm> --navigateurs chrome,firefox,safari --avec-interface --sans-construire …` | **code 0** : Chrome 156 et Firefox 158.0.1 réussis (Échap sous Keyboard Lock arrive au jeu dans Chrome, 0 → 1 ; dans Firefox avec sa fenêtre, Échap quitte le plein écran, le voile le dit, son clic est le geste et rend le plein écran) ; le contrôle négatif (l'aide tue) rouge comme attendu dans les deux ; Safari pas exécuté ; 346 s |
| `soiree-punchline`, `soiree-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <module> --navigateurs chrome,firefox --sans-construire …` | **code 0, code 0** : Chrome et Firefox réussis, quatre sauts chacun, chacun rattaché à sa reprise, trois marques F8 posées et rejouées ; 96 s et 95 s de scénario (244 s de bout en bout pour le premier, attente du portillon comprise) ; charge au plus 2,59 et 2,67 |

Aucune mesure n'est revendiquée : les points (M3) et le travail long (M8) que `ecran` rapporte ont été pris pendant `tests` (charge jusqu'à 11), et `soiree` ne juge pas de durée.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Les lignes `au-jeu` sont ce que la page dit d'elle** (le § 8.2 veut que `ecran` juge ce que la page montre et les fichiers reçus) : elles sont posées au point où la règle décide, et le banc juge aussi ce qu'il voit quand une entrée qui passerait a un effet (l'image dans la réserve sous l'écran de démarrage, Reset sous le voile : les contrôles négatifs le montrent). Le clic sur « Jouer » n'a pas d'effet visible s'il passe (rien du jeu sous le bouton) : là, seule la ligne le dit, et `geste-au-jeu` montre qu'elle vient.
- **Les touches sous l'écran du commun** ne sont pas rejouées par `ecran` : la règle des touches est celle du lot 10b (`page::touche`), jouée par `aide` (Échap) ; cette correction n'y touche pas.
- **Le plan court** : non relancé, pour la raison écrite dans la remontée (l'arbre d'`epreuve` ne contient ni `banc` ni `web/ecran`).
- `arriere-plan`, `--m12` et `soiree` sur Tier list, les contrôles scriptés des lots 10a et 10b : comme la remontée (constat 6).
- Les deux mutations du dpr ont été jouées dans Chrome seulement (le même code de la page dans Firefox ; `ecran-dpr2` joue Firefox à 2).

### Ce que le lot suivant doit savoir

- Toute entrée nouvelle que la page donne au jeu (un composant, un geste) doit passer par `page::le_commun_couvre` et dire sa ligne `au-jeu` : `ecran tier-list` le juge.
- `--dpr 2` se joue en quelques dizaines de secondes avec `--plein 0` : à relancer dès qu'on touche aux pixels (la toile, `centre_pour_le_banc`, les clics).
- Les options de test de la page au banc sont maintenant `activation-perdue`, `taire-l-aide`, `geste-au-jeu`, `ne-couvre-pas` ; la page les ignore hors du banc (`meta[name=soiree-banc]`).

## Lot 12 — Les téléphones

Fait le 2026-11-07, de 14 h 30 à 18 h 05 environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »), à côté du lot 11 (le worktree `soiree-lot-11`, non touché ; le portillon partagé). Chaque cargo, chaque scénario et les contrôles négatifs sont passés par le portillon (`target/hote/lourd`), sauf des `cargo check` et `cargo clippy -p` de quelques secondes ; un scénario à la fois, jamais à côté d'une compilation ; chaque navigateur a été fermé par le banc. Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) sont restés propres, leurs modules au même sha256 qu'aux lots 7 à 10b (`target/preuves/lot-12/modules.txt`).

### Ce qui est fait

**`salle::livraison`** (nouveau) : le module **livré** et les ressources de la page, de l'autorité aux téléphones, par la connexion chiffrée : morceaux de 64 Kio (`MORCEAU_MODULE`, choisi), au plus deux non accusés par téléphone (`FENETRE_MODULE`, § 2.5), aucun sur un lien encombré tant qu'un morceau attend son accusé (un à la fois s'il n'en attend aucun : sinon rien ne repartirait) ; les ressources suivent, la `i`-ième accusée par `ModuleRecu { morceau: total + i }` ; le téléphone (`Reception`) rassemble, vérifie le BLAKE3 contre l'empreinte que l'accueil nomme, et dit le module connu à son bonjour suivant. `Autorite::livrer` (le Worker le livre après le chargement), l'accueil nomme le module, `Encombre` suspend, `ModuleRecu` accuse ; `Comptes::morceaux_livres`.

**`salle::page`** (nouveau) : la page du téléphone sans DOM, une seule définition pour le navigateur et les téléphones natifs. `InstanceDePage` (n'importe quelle `Fabrique` : `Bonjour`, `Decrire`, puis `Page` seulement ; une page en faute ou invalide garde la précédente et se compte ; l'instance qui a fauté se refait) ; `PageDuTelephone` (les locaux du commun ; `Page` à chaque vue, saisie, taille ; le tenu, le dernier gagne, au débit du jeu, un relâchement passe toujours, et un tenu relâché ne se redit pas ; REM-23, le doigt lâché quand la clé disparaît ; l'arrière-plan, REM-11 ; la coupure pour un tenu à fraîcheur, P28 ; pour le banc, `chercher_le_geste` et `chercher_le_tenu`, le chemin d'un coup de bot sur la page — le bouton ou l'Entrée qui le porte, au besoin après avoir tapé dans les champs les textes qu'il porte ; la croix, les maintenus, les choix, les champs qui donnent ce tenu —, et `singe`) ; `JoueurDePage`, un téléphone natif qui passe par la page (un coup sans chemin n'est pas envoyé, et se compte).

**`salle::appareil`** : le module et les ressources reçus, `garder_la_file` et `reprendre_la_file` (`sessionStorage`, P26), `regler_la_redite`, `taire_la_redite`, `coupe` (l'écho sans réponse 1,5 s), `coups_par_la_page` et `prendre_les_coups`, `lien_relais`. **`salle::simulation`** : `livrer_le_module`, `par_la_page`, les joueurs par téléphone, rechargés avec la page.

**`web/telephone`** (nouvelle crate de `web/`, dans le paquet de la page ; `web/demarrage.js` la charge sur `/s/…`) : `adresse`, `ecrans` (les six écrans du commun, un seul, REM-69), `style` (un champ jamais sous 16 px), et la page : l'appareil, le module reçu chargé par `moteur-navigateur` (réécrit sous `PHASE_3`), le rendu DOM par clé (événements délégués à la racine ; les champs jamais réécrits ; la croix et les maintenus à capture du pointeur ; le carrousel balayable ; `FontFace`, `Blob` ; les images teintées en masque), `localStorage` (le jeton) et `sessionStorage` (l'onglet, la file, les effets), la reconnexion (intervalle croissant avec du hasard, sans réponse à l'écho, reliée sans accueil 5 s, jamais pendant un gel du banc), la vibration, et, avec `--banc`, `window.__banc` (`etat`, `noeuds`, `centre`, `debordements`, `geler`, `deborder`). Aucune crate tierce nouvelle ; le `Cargo.lock` de `web/` ne gagne que `telephone`.

**`banc`** : `telephones` (`soiree --pages …`, les pages dans de vrais navigateurs, voir plus bas), les pilotes touchent (`Input.dispatchTouchEvent`, pointeurs `touch` de BiDi), tapent (`Input.insertText`, touches de BiDi) et émulent un téléphone (`Emulation.setDeviceMetricsOverride`, `setTouchEmulationEnabled` ; `browsingContext.setViewport`) ; les téléphones natifs de `soiree` passent par la page (le premier est un singe) ; les sauts d'une reprise se rattachent au rattrapage que le jeu déclare. **`relais::preparer`** : le paquet gagne `telephone`.

Documents : PHASE-3 (§ 2.2, § 2.8 point 4, § 2.10, § 7 ligne du lot, § 8.2 `pages`), MESURES-PHASE-3 (« Lot 12 » : M4, M7), CHANGELOG, `crates/README.md`, `AGENTS.md`.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **Le module livré part aux téléphones, et non le réécrit** (§ 2.8, point 4) : le téléphone le réécrit lui-même (une seule définition du chargement dans le navigateur) et vérifie l'empreinte du catalogue.
2. **Le scénario `pages` n'est pas à part** (§ 8.2) : il juge, dans `soiree --pages`, chaque page vue en soirée à chaque taille où elle se montre, et non chaque page de pire cas ; son contrôle négatif est un texte qui déborde ajouté par la page (`__banc.deborder`), non un fautif de module.
3. **Les sauts d'une reprise se rattachent au rattrapage du jeu** (`banc::soiree::rattacher_les_sauts`) : la règle du lot 10a, « le premier pas joué après la partie reprise », supposait le rattrapage de Punchline (1) ; Sidequest en déclare 3, et chacun de ses sauts tombait trois pas après sa reprise (constaté sur ses quatre reprises, essais 2 et 3). `arriere-plan` garde la règle du lot 10a.
4. Un tenu relâché ne se redit pas ; la page reconnecte aussi cachée, et reliée sans accueil 5 s (§ 2.10, « Écrit au lot 12 »).

### Les tests qui le tiennent

- `crates/salle/src/page.rs` (5) : `un_maintenu_dont_la_cle_disparait_est_relache_rem23` (une page écrite à la main, sur `Pager`), `un_maintenu_dont_la_cle_reste_continue`, `le_debit_retient_les_tenus_mais_pas_le_relachement_rem15`, `l_arriere_plan_et_la_coupure_relachent_rem11_p28`, `les_chaines_d_un_geste_se_lisent`.
- `crates/salle/src/livraison.rs` (3) : la fenêtre (et l'encombrement sans rien en vol), le module rassemblé et vérifié, le module qui n'est pas celui annoncé refusé ; `crates/salle/src/appareil.rs` : `la_file_survit_au_rechargement_sal56`.
- `crates/salle/tests/devine.rs` (2) : `une_soiree_de_devine_par_la_page_survit_a_ses_pannes` (le module livré à chaque arrivée et rechargement, les coups du bot par la page, un singe, la chaîne écrite qui tient sous les pannes), `le_module_ne_repart_qu_a_une_page_rechargee_p18`.
- `web/telephone` (3) : `la_salle_et_le_fragment_se_lisent_sal119`, `un_seul_message_qui_nomme_la_vraie_cause_rem69`, `un_champ_n_est_jamais_sous_16_px`.
- **Les contrôles négatifs** (`target/travail/lot-12/controles.py`), dix mutations, chacune rouge pour sa raison, la source remise : le doigt jamais lâché (REM-23), la fenêtre ouverte, le module connu oublié, la file non reprise, le débit ignoré, la coupure sans fraîcheur, le module non vérifié, « Remplacé » tu, un champ sous 16 px, les coups du bot sans la page. Avec `--seulement-scenarios`, **le scénario muté** : la page ne reprend pas sa file au rechargement, et `soiree --pages chrome` voit le geste perdu (« n'est pas accusé en 60 s »). Le premier passage des contrôles (`target/travail/lot-12/controles-premier-passage/`) avait une mutation dont la ligne n'existait plus (la fenêtre, réécrite pour l'encombrement) : corrigée, le second passage fait foi.
- **Le scénario** `soiree --pages`, et ses contrôles intégrés : le contrôle négatif des débordements vu à chaque passage ; les secrets cherchés dans l'enregistrement du relais.

### Le scénario des pages (`soiree-banc soiree <jeu> --pages chrome,firefox,safari`)

Pour chaque passage (l'écran dans un navigateur), une page du téléphone dans chaque navigateur de `--pages`, chacun lancé à part sur un profil neuf (son jeton est à lui), ouverte à `http://127.0.0.1:<port>/s/<SALLE>#<fragment>` (une autre origine que l'écran) : Chrome en émulation tactile (390 × 844), Firefox à 360 × 640, Safari « pas exécuté » (`safaridriver` refuse la session). Les pages attendent l'accueil, le module (M4 dit par la page) et une page posée ; un pilote, sans rien savoir du jeu, tape dans un champ vide (après l'avoir touché), touche le bouton le moins récemment touché, et tient une croix 8 s sur 12 ; à chaque page vue et chaque taille, il juge les débordements. Puis, après les reprises de l'écran : **la page rechargée en pleine écriture** (réseau gelé, une réponse tapée, un bouton touché : le geste ne part pas ; la page rechargée doit le voir accusé) ; **l'onglet jumeau** (le jumeau prend la place, l'ancien montre « Remplacé — Reprendre ici », toucher le bouton la lui rend, le jumeau est remplacé à son tour) ; **la coupure** d'une page qui tient la croix (réseau gelé 2,5 s : relâchement côté page, et retour) ; avec des pages, **« On rejoue »** (une partie non terminale après le podium). À la fin : chaque page a émis des gestes et dit M4, sans erreur de console ; aucun débordement ; le contrôle négatif vu ; M7 lu dans l'enregistrement du relais pendant les croix tenues ; le secret d'entrée, G, les jetons des natifs et les textes tapés cherchés dans cet enregistrement.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-12/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-12/preuve.sh`, `final.sh`, `controles.py` ; l'empreinte par `target/travail/lot-10b/empreinte.sh`), lancées à la suite par `final.sh` de 17 h 18 min 10 s à 17 h 51 min 11 s (`final.log`), puis `controles` refait (17 h 51 min 43 s) et `soiree-punchline-2` (17 h 52 min 39 s à 18 h 00 min 51 s). **Les 18 preuves ont la même empreinte hors `.md` au départ et à la fin** : le commun `010c5d5`, `diff HEAD` hors `.md` `29fd18d8aada3192`, non suivis (9) `fc582afd261f20e2` ; Punchline, Sidequest et Tier list sans aucune modification. (Ce journal, PHASE-3, MESURES-PHASE-3, CHANGELOG, `crates/README.md` et `AGENTS.md` ont été écrits pendant et après : le `diff` entier change, pas celui hors `.md`.)

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : **54 réussis** (51 au lot 10b, 3 de plus), 0 échec ; 0,47 Go |
| `construire` | `lourd sh -c "cargo build -p banc … -p relais … && cargo build --release -p epreuve … && target/debug/soiree-relais preparer && shasum …"` | code 0 ; modules `82669f77…`, `62043526…`, `84315a1b…` ; paquet `540f6713…` |
| `soiree-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <Sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --sans-construire …` | **code 0** : les deux passages réussis (l'écran dans Chrome, puis dans Firefox), chacun avec ses quatre reprises de même forme que wasmtime, la chaîne rejouée, trois marques F8 rejouées, quatre sauts rattachés à leur reprise ; les pages Chrome et Firefox accueillies (M4), rechargée en pleine écriture (geste 3 accusé après), jumeau puis « Reprendre ici », coupure relâchée côté page (P28 : coupures 0 → 1) et revenue, « On rejoue », aucun secret parmi 22 cherchés, aucun débordement (18 et 16 pages jugées au premier passage), le contrôle négatif vu, M7 ; Safari pas exécuté ; 191 s ; 0,79 Go (le banc) ; charge 2,63 au départ, 1,87 à la fin, **2,84 au plus** : une mesure sur machine calme |
| `soiree-punchline` | `lourd --calme … soiree <Punchline> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 5 --sans-construire …` | **code 1** : **Chrome en échec sur un seul constat**, un saut d'un pas (3001 → 3002) qu'aucune reprise n'explique, au tour d'un point (le tour 22,6 ms, dont 21,7 ms à sceller le point, après un trou de 26 ms) ; tout le reste du passage tient (podium puis « On rejoue », rechargement en pleine écriture, geste 10 accusé après ; jumeau ; secrets ; débordements) ; **Firefox réussi** ; 328 s ; charge 2,95 au départ, **5,57 au plus** (le portillon : « ne vaut pas mesure ») |
| `soiree-punchline-2` | la même commande, relancée | **code 1** : Chrome réussi ; **Firefox en échec sur un seul constat**, un saut d'un pas (1049 → 1050) qu'aucune reprise n'explique, tôt dans la partie, sans tour long rapporté ; tout le reste tient ; 491 s ; **4,66 au plus** |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 1, 1, 1 : chaque ligne de contrôle identique au lot 10b (E16 chez les trois, E2 chez Punchline) |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 716 réussis** (705 au lot 10b ; 11 de plus), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 675 s ; 1,70 Go ; la charge a atteint 16,2 (d'autres travaux de l'hôte) : une durée relevée |
| `controles` | `lourd python3 -I target/travail/lot-12/controles.py` | code 0 : 10 mutations sur 10 rouges ; 43 s |
| `controle-scenario` | `lourd python3 -I … controles.py --seulement-scenarios` | code 0 : `rechargement-sans-file` rouge (« n'est pas accusé en 60 s ») ; le paquet reconstruit ensuite sur la source remise (`preparer-de-nouveau`, le même `540f6713…`) |

Le plan standard n'est pas exigé (le lot ne change ni ce que l'épreuve joue ni ce qu'elle juge ; le plan court le prouve, à l'identique). E20, `dessin`, `aide` et `arriere-plan` ne sont pas relancés : ni le moteur, ni la réécriture, ni la télé ne changent ; le Worker ne gagne que `livrer`.

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 12 » : **M4** sur la machine de l'hôte, Sidequest (738 744 octets) : 127,6 et 129,2 ms dans Chrome, 172,0 et 185,0 ms dans Firefox, du module reçu à l'instance prête ; **M7** : médiane 153,5 ms (Chrome) et 167,2 ms (Firefox) entre deux tenus pendant une croix tenue, pour 150 ms de P27. Les relevés de Punchline ne valent pas mesure (charge au-dessus de 3).

### Ce qui n'est pas fait, et pourquoi

- **Le scénario de Punchline n'est pas vert** : deux lancements, chacun un passage sur deux en échec sur un seul saut d'un pas inexpliqué, sous la charge que le scénario fait lui-même (l'écran, deux navigateurs de pages, cinq natifs : 4,66 à 5,57). C'est la famille de la question 30 (les sauts sous charge, le point scellé dans le tour) ; ni le contrôle ni le Worker ne sont changés pour le faire passer. Sidequest, à moins de 3, passe.
- **Changer de jeu sans rien relancer** (`CommandeEcran::ChangerDeJeu`) : non écrit (une nouvelle partie, une nouvelle chaîne de garde, la livraison d'un autre module : la livraison est prête à suivre une autre empreinte, l'autorité ne sait pas encore changer de fabrique).
- **Un téléphone qui rejoint Sidequest à 71 appareils ne retarde personne** : non éprouvé (la fenêtre de livraison le permet ; aucun scénario ne mesure le retard des vues des autres).
- **`FilePleine`** : compté (la page le dit à l'écran et au banc, les natifs le comptent), jamais provoqué dans un scénario (0 dans chaque passage).
- **Sidequest cinq minutes** : le scénario dure ce que dure la soirée du banc (environ 1 min 30 par passage), non cinq minutes.
- **M13** (la part du commun dans le Worker à chaque tour) : non mesurée.
- **Le scénario `pages` sur les pires cas** : jugé sur les pages vues en soirée (écart 2).
- **Les coups des bots par la page** : beaucoup restent sans chemin chez Sidequest (un `Rejoindre` au look tiré au hasard, qu'aucun choix de la page ne donne d'un coup ; une direction continue que la croix en huit ne donne pas) : ils ne sont pas envoyés, et se comptent (`ComptesJoueur`) ; la partie avance par ceux qui en ont un et par le singe.
- **Ce que la page retient** (`Locaux::retenu`) reste en mémoire, pas dans `localStorage` : le personnage retenu de Sidequest ne survit pas encore à un rechargement.
- **Safari** : pas exécuté (`safaridriver`, le réglage de l'hôte non touché) ; `connect-src 'self'` n'y est donc pas constaté.
- Le son sur le téléphone, les motifs de vibration, le verrou d'écran (hors contexte sécurisé) : non faits.

### Ce que le lot suivant doit savoir

- `soiree-banc soiree <jeu> --pages chrome,firefox[,safari]` ; une page par navigateur, lancé à part ; le pilote est générique. Les pages offrent `window.__banc` en mode banc ; `geler(ms)` coupe le réseau de la page sans fermer sa WebSocket, `deborder()` ajoute un texte qui déborde (contrôle négatif).
- Les téléphones natifs du scénario `soiree` passent par la page ; `soiree-native` et `salle::simulation` gardent les gestes directs par défaut (`Soiree::par_la_page` les fait passer par elle).
- Le module part aux téléphones depuis `Autorite::livrer` ; un changement de jeu devra appeler `livrer` avec l'autre module (les téléphones suivent l'empreinte des morceaux).
- La charge d'un scénario à trois navigateurs dépasse 3 : une mesure de temps sur la page (M4) se prend au mieux sur le scénario de Sidequest, ou sur un scénario plus léger.
- Le paquet de la page se reconstruit après le contrôle négatif du scénario (`soiree-relais preparer`), sans quoi `--sans-construire` prendrait le paquet muté.


## Lot 12, correction après la vérification

Faite le 2026-11-07, de 18 h 15 à 21 h environ (GMT), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »), à côté du lot 11 (le worktree `soiree-lot-11`, non touché ; le portillon partagé). Chaque cargo, chaque scénario et chaque contrôle négatif est passé par le portillon, sauf des `cargo check -p` de quelques secondes ; un scénario à la fois, jamais à côté d'une compilation ; chaque navigateur a été fermé par le banc. Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) restent propres, leurs modules au même sha256 (`target/preuves/lot-12-corr/modules.txt`).

### Les constats, et ce qui en est fait

1. **[bloquant] Le scénario `soiree punchline` avec pages n'était pas vert** (question 30). Une des deux causes est dans le commun, et corrigée : le tour qui prend un point l'encodait octet par octet, deux fois (l'instantané dans `PointFil`, puis le point dans `ContenuPoint`), 10 à 22 ms dans le tour même. Le point s'écrit désormais **d'un bloc** (`partie::persistance::octets`, `serialize_bytes`) : les **mêmes octets** (les tests de forme figée de `partie` et de `salle` passent sans changement, et le plan court rend, contrôle pour contrôle, ce qu'il rendait), et aucun tour de point ne dépasse plus 10 ms dans Chrome (aucune ligne `tour` de point dans les essais ni dans les preuves). L'autre cause, les réveils de la minuterie en retard de plus de deux pas sous charge, tour court, reste la question 30 ; le contrôle n'est pas changé. Résultat : voir « Les vérifications ».
2. **[bloquant] Les exigences non faites.**
   - **`FilePleine` provoqué** : fait. `remplir_la_file_d_une_page` gèle le réseau de la première page, touche un bouton autant de fois que la file tient de gestes plus huit ; le compte doit être exact (placés + refusés = touchers, la file à sa limite, le bandeau dit), puis, au dégel, chaque geste placé accusé. Côté natif, `une_file_pleine_se_dit_et_se_compte_sal07`.
   - **Sidequest cinq minutes** : fait (`--minutes 5` : la partie prolongée jusqu'à cinq minutes depuis son ouverture, croix tenue, redite, coupure, file pleine, « On rejoue » compris).
   - **Un téléphone qui rejoint Sidequest à 71 appareils** : fait (`--retardataires`) : 69 natifs accueillis avec leur module, puis les deux pages arrivent une à une (71 téléphones) ; pendant que chacune reçoit le module, la médiane et le 99ᵉ centile des allers-retours d'écho des natifs ne doivent pas croître de plus d'un pas (16,7 ms, choisi) par rapport aux 20 s d'avant.
   - **M13** : écrit (`RepartitionDuTour` dans le Worker, rapportée dans les lignes `etat` ; le banc la lit et la juge à 71 appareils contre 4,17 ms au 99ᵉ centile, la moitié de la part d'image que E19 laisse au commun) ; relevée, **pas mesurée sur machine calme** (la charge du scénario a dépassé 3).
   - **Changer de jeu sans rien relancer** : **non fait** (voir « Ce qui n'est pas fait »).
3. **[important] La couverture de Sidequest** : rétablie. Un coup de bot cherche maintenant son chemin aussi **par les choix** de la page (`tourner_les_choix` : une montée, choix par choix, vers un bouton dont le geste est le plus proche de celui du bot, les choix relus après chaque option gardée, la couleur de Sidequest dépendant du personnage), après les textes ; une direction continue que la croix en huit ne donne pas prend **la plus proche** (`approcher_le_tenu`, sur des tenus de flottants seulement, comptée à part : `tenus_approches`). Un natif qui n'envoie aucun geste par la page est désormais **une faute** du scénario. Sidequest passe de 50 gestes (lot 12, passage 1) à plus de 1 100 par passage de cinq minutes ; en mémoire (`soiree_en_memoire --par-la-page`, 60 s, huit téléphones) : 299 gestes, contre 407 sans la page.
4. **[important] § 8.2 `pages` réécrit** : le texte d'origine est remis, et ce qui est fait s'y dit comme **un écart**, non comme une correction : les pages vues en soirée (Chrome et, désormais, Firefox aux trois tailles), pas les pages de pire cas ; le contrôle négatif par un texte ajouté par la page, plus faible qu'un fautif de module. Les pages de pire cas et le fautif : non faits.
5. **[mineur] `controles.py`** : remplacé par `target/travail/lot-12-corr/controles.py`, au critère strict (le test nommé doit être `FAILED`, sans erreur de compilation) ; 15 mutations (les dix du lot 12 et cinq nouvelles), et deux programmes mutés.
6. **[mineur] M7** : le second passage n'était pas lu à cause de l'horloge du relais, qui part du premier relais du processus et non de chacun (`relais::horloge`) : le banc la relit à chaque lancement (`relais_us0`). M7 se dit aussi sur les seuls coups du pilote où le tenu n'est pas la valeur relâchée (`tenu_relache` dans l'état de la page) : les écarts de 1,75 et 3,5 s sont bien des moments où le tenu était relâché (commandes désactivées), mais cette seconde lecture a un défaut, vu après les preuves : ses fenêtres vont d'un coup du pilote au suivant, et deux coups pris dans deux croix tenues de suite en font une fenêtre qui couvre le lâcher de 4 s (d'où un 99ᵉ centile de 4 s) ; elle n'est pas une mesure. Le 99ᵉ centile de Chrome au lot 12 (1 248,5 ms) et le maximum de 3,5 s viennent de ces moments relâchés.
7. **[mineur] Les secrets** : les jetons des pages (lus dans leur `localStorage`, en octets et en hexadécimal) sont cherchés, et **un témoin positif** (24 octets d'une trame qu'une page a envoyée) doit être vu par la même recherche. Un texte tapé de moins de 8 octets se cherche précédé de sa longueur postcard : au premier passage des preuves, « Hugo » (4 octets) a été trouvé **une fois, au milieu d'octets chiffrés**, dans un enregistrement de 62 Mo, ce que le hasard donne une fois sur 70 par texte de quatre octets (62·10⁶ / 2³²) ; il est désormais noté comme constat, tel quel, et jugé avec sa longueur (5 octets : une fois sur 18 000).
8. **[mineur] `rattacher_les_sauts`** : `un_saut_se_rattache_au_rattrapage_du_jeu_une_fois` (la fenêtre de `p` à `p + rattrapage`, un rattrapage nul lu comme 1, une reprise par saut, la ligne `en-jeu` sans case relue, le retard injecté).
9. **[mineur] `soiree punchline` sans pages** : relancé (voir « Les vérifications »).
10. **[mineur] Safari** : toujours « pas exécuté » (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).

### Ce que les essais et les contrôles ont appris en chemin

- **Le bouton « Sauter » de Sidequest ne répondait souvent pas au doigt dans Chrome** : le banc note maintenant ce que le doigt touche quand un bouton ne répond pas, et c'était l'image de l'avatar, sous la même pile. Les enfants d'une pile ne prenaient leur case et leur rang qu'à la mise à jour de la pile, qui précède leur création : une image refaite après elle n'avait ni l'une ni l'autre. Corrigé dans la page (`ranger_dans_une_pile` : `grid-area` et `z-index` dans l'ordre de la page, à la pose) ; aux preuves, plus aucun bouton sans réponse.
- **Le gel du banc perdait ce que la page envoyait** (`vider` jetait les sorties pendant un gel, la WebSocket restant ouverte) : une file pleine provoquée sous un gel plus court que la coupure (P28) ne repartait jamais (32 gestes en file, jamais accusés). Une WebSocket ouverte ne perd rien : le gel retient maintenant les sorties et les laisse partir au dégel sur la même connexion (une connexion neuve les oublie). Le rechargement en pleine écriture et la coupure tiennent comme avant, et le contrôle négatif du rechargement reste rouge pour sa raison.
- **Le pilote touchait un bouton toutes les 400 ms** : dans le salon de Punchline, chaque personnage touché change la télé, et l'organisateur simulé, qui attend 2 s d'une télé calme pour lancer, n'a jamais lancé (le contrôle négatif du scénario, au premier passage des preuves : la partie restée 400 s au salon, rouge mais pas pour sa raison). Le pilote attend désormais 3 s entre deux boutons (choisi).
- La file pleine se provoque **réseau gelé dès le premier toucher** : un geste change souvent la page qui le porte (le personnage choisi devient un texte sous la même clé, le vote envoyé disparaît), et un toucher d'essai réseau ouvert laissait la page changer.

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. § 8.2 `pages` : le texte d'origine remis ; l'écart du lot 12 dit comme tel (constat 4).
2. § 9.1, question 30 : le point s'écrit d'un bloc ; restent les réveils en retard.
3. § 2.10 : les enfants d'une pile, le gel du banc qui retient.
4. § 7, ligne du lot 12 : ce que la correction a fait et ce qui reste.

### Les tests qui la tiennent

- `crates/salle/src/page.rs` : `un_geste_porte_par_des_choix_trouve_son_chemin` (un sélecteur écrit à la main, dont la clé de couleur dépend du personnage), `un_tenu_continu_prend_la_direction_la_plus_proche`, `une_file_pleine_se_dit_et_se_compte_sal07`.
- `crates/salle/src/autorite.rs` : `la_repartition_du_tour_rend_ses_centiles_m13`.
- `crates/banc/src/soiree.rs` : `un_saut_se_rattache_au_rattrapage_du_jeu_une_fois`.
- Les tests de forme figée de `partie` (`tests/coeur/garde.rs`, `forme_garde.rs`) et de `salle` (`tests/garde.rs`, `tests/chiffre.rs`), inchangés, tiennent que le point écrit d'un bloc a les mêmes octets.
- **Les contrôles négatifs** (`target/travail/lot-12-corr/controles.py`, critère strict) : 15 mutations, chacune rouge par son test nommé, sans erreur de compilation : les dix du lot 12, et les choix jamais tournés, le tenu jamais approché, la file pleine tue, la fenêtre des sauts élargie d'un pas, la répartition au bas de sa case. Deux programmes mutés (`--seulement-scenarios`) : sans les choix, `soiree_en_memoire --par-la-page` sur Sidequest dit « aucun geste par la page » ; sans reprise de la file au rechargement, `soiree --pages chrome` sur Punchline voit le geste perdu.
- **Le scénario** `soiree --pages` et ses contrôles intégrés : le natif sans geste, la file pleine, le témoin positif des secrets, les retardataires, M13 à 71 appareils.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-12-corr/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-12-corr/preuve.sh`, `final.sh`, `controles.py` ; l'empreinte par `target/travail/lot-10b/empreinte.sh`), lancées à la suite par `final.sh` de 19 h 51 min 07 s à 20 h 36 min 30 s (`target/travail/lot-12-corr/final.log`), puis `soiree-punchline-2` (20 h 37 min à 20 h 43 min). **Les 19 preuves ont la même empreinte hors `.md` au départ et à la fin** : le commun `010c5d5`, `diff HEAD` hors `.md` `de1bf6d6c3c716f9`, non suivis (9) `92fa96c909489b34` ; Punchline, Sidequest et Tier list sans aucune modification ; les modules aux mêmes sha256 qu'aux lots 7 à 12 (`82669f77…`, `62043526…`, `84315a1b…`). (Ce journal, PHASE-3, MESURES-PHASE-3, CHANGELOG, `crates/README.md` et `AGENTS.md` ont été écrits pendant et après : le `diff` entier change, pas celui hors `.md`.) Un **premier passage** des preuves (`target/preuves/lot-12-corr/premier-passage/`, `final-premier-passage.log`) a été arrêté après le scénario de Sidequest : il a montré la partie restée au salon, le bouton « Sauter » sans réponse, « Hugo » trouvé au hasard et M7 vide au second passage ; tout ce qui suit a été refait après leur correction.

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : 54 réussis, 0 échec |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 721 réussis** (716 au lot 12, 5 de plus), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 469 s ; 1,73 Go ; la charge a atteint 11,2 (d'autres travaux de l'hôte) : une durée relevée |
| `controles` | `lourd python3 -I target/travail/lot-12-corr/controles.py` | code 0 : **15 mutations sur 15 rouges par le test nommé** (`FAILED`, sans erreur de compilation) |
| `controle-scenario` | `… controles.py --seulement-scenarios` | code 0 : `sidequest-sans-choix` (« aucun geste par la page ») et `rechargement-sans-file` (« n'est pas accusé en 60 s ») rouges pour leur raison ; le paquet reconstruit ensuite sur la source remise (`construire`) |
| `construire` | `lourd sh -c "cargo build -p banc … -p relais … && cargo build --release -p epreuve … && target/debug/soiree-relais preparer && shasum …"` | code 0 ; paquet `94def9a6…` ; modules inchangés |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 1, 1, 1 : **chaque contrôle identique au lot 12** (identifiant, issue, nombre d'évaluations, détail : E16 chez les trois, E2 chez Punchline), comparé sur les rapports JSON : le point écrit d'un bloc ne change rien de ce que l'épreuve joue ou juge |
| `soiree-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <Sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --minutes 5 --sans-construire …` | **code 0** : les deux passages réussis, chacun **300 s de partie** (2 882 et 2 932 cases, 1 151 et 1 221 gestes, 23 et 24 phases terminales, quatre reprises de même forme, quatre sauts rattachés, trois marques rejouées) ; pages accueillies (M4), rechargement en pleine écriture, jumeau, coupure P28, **file pleine provoquée** (40 touchers : 32 placés, 8 refusés, le bandeau ; tout accusé au dégel), « On rejoue », 29 secrets absents et le témoin positif vu, aucun débordement (77 et 83 pages jugées), le contrôle négatif vu, **aucun bouton sans réponse au doigt**, chaque natif a envoyé des gestes par la page ; Safari pas exécuté ; 672 s ; 0,79 Go ; charge 1,38 au départ, **3,02 au plus** (le portillon : « ne vaut pas mesure ») |
| `soiree-punchline` | la même commande sur Punchline, `--telephones 5`, sans `--minutes` | **code 1** : **Chrome réussi** (podium, « On rejoue », rechargement, jumeau, file pleine, secrets) ; **Firefox en échec sur un seul saut d'un pas** (2670 → 2671) qu'aucune reprise n'explique : un réveil de la minuterie après un trou de 34,4 ms, tour de 0,6 ms, dans le second onglet de l'écran juste avant qu'il ne soit fermé (la question 30, ce qui en reste) ; tout le reste tient ; aucune ligne `tour` de point ; 304 s ; **3,13 au plus** |
| `soiree-punchline-2` | la même, relancée | **code 0** : les deux passages réussis ; 373 s ; 3,73 au plus |
| `soiree-punchline-sans-pages` | `lourd --calme … soiree <Punchline> --navigateurs chrome,firefox --telephones 5 --sans-construire …` | **code 0** : les deux passages réussis (constat 9) ; 155 s ; 3,24 au plus |
| `soiree-71` | `lourd --calme … soiree <Sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 69 --retardataires --sans-construire …` | **code 1** : **Chrome réussi** (69 natifs puis les deux pages, 71 téléphones ; 4 617 gestes ; les deux retardataires sans retard : allers-retours des natifs, médiane 11,6 → 10,7 ms et 10,7 → 12,6 ms ; M13 relevée, avec la réserve dite plus bas) ; **Firefox en échec sur un seul constat** : quand la page Firefox arrive (la seconde), les 67 allers-retours pris pendant sa livraison (268 ms) valent tous 68,3 ms, contre une médiane de 10,6 ms avant : un tour d'écho entier retardé de 58 ms pour tous ; la page Chrome, arrivée avant dans le même passage, n'a rien retardé (8,9 ms) ; tout le reste tient (rechargement, jumeau, coupure, file pleine, secrets, natifs) ; 644 s ; 1,0 Go (le banc) ; **4,47 au plus** |

Le plan standard n'est pas exigé : le lot ne change ni ce que l'épreuve joue ni ce qu'elle juge, et le plan court le prouve, contrôle pour contrôle. E20, `dessin`, `aide` et `arriere-plan` ne sont pas relancés : ni le moteur, ni la réécriture, ni la télé ne changent ; le Worker ne gagne que la répartition de M13, et le point s'écrit avec les mêmes octets.

### Les mesures

Aucune mesure nouvelle **sur machine calme** : chaque scénario de la correction a dépassé 3 (3,02 à 4,47), et le portillon l'a dit ; ce qui suit est relevé, écrit dans `MESURES-PHASE-3.md` (« Lot 12, correction ») comme tel. M4 et M7 du lot 12 (charge 2,84) restent les mesures.
- **M13** (la part du commun dans le Worker à chaque tour : vues, chiffrement, garde), relevée dans `soiree-71` sur le Worker qui a joué le plus de tours (le premier de l'écran, 6 121 tours) : dans Chrome, médiane 0,4 ms, 99ᵉ centile 1,7 ms, maximum 4,19 ms (le tour entier : 0,8, 5,2 et 30,6 ms) ; dans Firefox 0,5, 2,2 et 6,94 ms. Sous les 4,17 ms au 99ᵉ centile, mais pas mesurée au calme. **Une réserve** : ce Worker compte 69 à 71 appareils accueillis jusqu'au pas 5 400 environ, puis **2** jusqu'à la fin (ses 720 derniers tours, 12 %), alors que les lignes `salle` de l'écran disent 71 téléphones reliés tout du long ; ce que compte `Autorite::appareils_accueillis` (les liens chiffrés dont le bonjour est accueilli) a donc perdu les natifs à ce moment, sans que le banc l'explique : la répartition couvre environ 88 % de tours à 69-71 appareils.
- **Le point écrit d'un bloc** : aucun tour de point au-delà de 10 ms dans aucun passage (au lot 12 : 10 à 22 ms à sceller).

### Ce qui n'est pas fait, et pourquoi

- **Changer de jeu sans rien relancer** (`CommandeEcran::ChangerDeJeu`) : non écrit. Il demande que l'autorité change de fabrique et de chaîne de garde (une partie neuve, une ouverture neuve), que le Worker charge un autre module du catalogue (le rapport vérifié, la réécriture), que la télé change de scène et de ressources, que les téléphones rechargent leur instance sur l'empreinte nouvelle (la livraison sait déjà suivre une autre empreinte), et un scénario du banc ; trop pour cette correction, à côté des autres constats. **Le lot reste donc inachevé.**
- **Le scénario de Punchline n'est vert qu'une fois sur deux** : `soiree-punchline` échoue dans Firefox sur un saut d'un pas après un réveil de la minuterie en retard (34,4 ms, tour de 0,6 ms) ; `soiree-punchline-2` passe. La part du commun (le point) est corrigée ; le reste est la question 30, et le contrôle n'est pas changé.
- **Les retardataires dans Firefox** : un échec sur quatre arrivées (constat ci-dessus). Le banc tourne sur un seul fil, qui interroge la page nouvelle toutes les 50 ms pendant la livraison : un arrêt du banc (une évaluation BiDi sur une page Firefox fraîche) retarde la lecture de tous les échos à la fois, comme un retard de l'autorité ; la mesure ne sait pas les séparer, et la livraison n'est pas mise hors de cause. Non corrigé, non affaibli.
- **M13 et la seconde lecture de M7** : relevées, pas mesurées au calme ; M13 couvre 88 % de tours à 69-71 appareils, le compte des appareils accueillis tombant à 2 sur la fin sans explication (une question ouverte : des liens des natifs qui changent d'état pour l'autorité, ou un compte faux) ; la seconde lecture de M7 a des fenêtres qui couvrent le lâcher entre deux croix (son 99ᵉ centile de 3 à 4 s n'est pas une redite).
- **Les pages de pire cas** du scénario `pages` et leur contrôle négatif par un fautif de module : non faits (§ 8.2, écart dit).
- **Safari** : pas exécuté ; `connect-src 'self'` n'y est pas constaté.
- Restent du lot 12 : `Locaux::retenu` en mémoire seulement, ni son, ni motifs de vibration, ni verrou d'écran. Des coups de bot restent sans chemin (Sidequest : de 10 à 40 gestes par natif et par passage, des messages hostiles ou des sauts commandes désactivées), comptés.

### Ce que le lot suivant doit savoir

- Un coup de bot cherche son chemin par les champs puis par les choix (`ESSAIS_DES_CHOIX`, 192 appels `Page` au plus) ; un tenu de flottants sans chemin prend le plus proche (`tenus_approches`). Un natif sans geste est une faute : un jeu dont la page ne donne aucun chemin aux coups de son bot fera échouer `soiree`.
- `soiree-banc soiree <jeu> --pages … [--minutes M] [--retardataires]` ; le scénario des 71 appareils est `--telephones 69 --pages chrome,firefox --retardataires` sur Sidequest (1 Go pour le banc, une charge de 4 à 5).
- Le pilote des pages attend 3 s entre deux boutons. La file pleine se provoque réseau gelé dès le premier toucher. `__banc.geler` retient les sorties.
- L'horloge du relais part du premier relais du processus (`relais::horloge`) : tout ce qui compare un instant du banc à l'enregistrement du relais passe par `Session::relais_us0`.
- `partie::persistance::octets` écrit un `Vec<u8>` d'un bloc avec les mêmes octets : à mettre sur tout gros champ d'octets gardé ou envoyé (le journal ne l'a pas encore, ni les messages de `salle::fil`).
- Le changement de jeu reste à écrire (voir plus haut), comme les pages de pire cas.

## Fusion du lot 11 (Tier list jouable, avec le lot 10b déjà remonté dans sa branche)

Faite le 2026-11-07, à partir de 20 h 45 environ (heure de la machine, UTC), dans `soiree/` (branche `refonte`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Les deux arbres étaient propres avant (`git status` vide dans `soiree/` et dans le worktree `soiree-lot-11`). `git merge --no-ff --no-commit refonte-lot-11` : `HEAD` `e719045` (le lot 12, inachevé), `MERGE_HEAD` `2d95152` (le lot 11, `c2588bd`, et la remontée du lot 10b), base commune `010c5d5` (le lot 10b). **Rien n'est commité** : la fusion reste en cours, les conflits résolus et mis à l'index ; un agent à part fait le commit de fusion. Seul le commun est modifié ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) restent propres. Le lot 12 reste inachevé (`CommandeEcran::ChangerDeJeu` non écrit, et le reste de « Lot 12, correction », « Ce qui n'est pas fait ») : cette fusion ne le finit pas.

### Les conflits, et comment ils sont résolus

Dix fichiers en conflit ; le reste s'est fusionné seul (dont `crates/banc/src/chrome.rs`, `firefox.rs`, `lib.rs`, `crates/salle/src/simulation.rs`, `crates/salle/tests/devine.rs`, `AGENTS.md`, `web/Cargo.toml`, `web/Cargo.lock`). Les deux côtés gardés partout ; les documents fusionnés mot à mot sur la base (`target/travail/fusion-11/fusion3.py`), puis vérifiés : chaque texte qu'un côté ajoute depuis `010c5d5` se retrouve dans le fichier fusionné (`target/travail/fusion-11/verifier.py`, 0 manquant dans `PHASE-3.md`, `crates/README.md`, `CHANGELOG.md`, `AGENTS.md`, `MESURES-PHASE-3.md`, ce journal ; dans le code, 0 manquant dans `salle/src/autorite.rs`, `banc/src/navigateur.rs`, `web/autorite/src/worker.rs`, `soiree-banc.rs`, et dans `banc/src/soiree.rs` seulement les lignes de la règle des sauts remplacées, ci-dessous).

- **`conception/LOTS-PHASE-3.md`** : refait dans l'ordre des lots, les sections antérieures identiques des deux côtés à la base : « Lot 11 », « Lot 11, correction », « Lot 11 : remontée du lot 10b » et sa correction, puis « Lot 12 » et « Lot 12, correction », puis celle-ci.
- **`CHANGELOG.md`** : la section du lot 12 au-dessus de celle du lot 11 (la plus récente d'abord), et une puce de plus au lot 11 pour la règle des sauts fusionnée.
- **`conception/MESURES-PHASE-3.md`** : « Lot 11 » avant « Lot 12 » et « Lot 12, correction ».
- **`conception/PHASE-3.md`** (deux conflits) : la ligne `pages` du § 8.2 (l'écart du lot 12) et la ligne `soiree <jeu>` (la correction du lot 11), chacune changée d'un seul côté, prises telles quelles ; au § 9.1, la question 30 (inchangée des deux côtés) et la question 31 (lot 11), gardées.
- **`crates/README.md`** (deux conflits) : chaque ligne du tableau prend les ajouts des deux lots, le lot 11 puis le lot 12 quand ils s'ajoutent au même endroit (`liaison`, `salle`, `banc`, `relais`, `rendu`, `ecran`, `telephone`, `autorite`, et la phrase des dépendances de `web/`).
- **`crates/banc/src/bin/soiree-banc.rs`** : l'usage de `soiree` (les options du lot 12 : `--pages`, `--retardataires`, `--minutes`) et la ligne `ecran tier-list` (lot 11, avec `--dpr` et les trois `--fautif`).
- **`crates/banc/src/navigateur.rs`** : `telephone`, `doigt`, `toucher`, `taper` (lot 12) et `fenetre`, `telechargements`, `fichiers`, `souris`, `texte`, `touche` (lot 11). Rien n'est fondu : `taper` (`Input.insertText`) et `texte` tapent par deux chemins, `telephone` (l'émulation tactile) et `fenetre` (la taille et le rapport de pixels) règlent l'onglet par deux chemins, que chaque scénario garde.
- **`crates/salle/src/autorite.rs`** (quatre conflits) : l'`Autorite` a `livraison` et `ressources_lues` (lot 12), `bots`, `mesure_du_point` et `point_en_mesure` (lot 11) ; `RepartitionDuTour` (M13, lot 12) et `MesurePoint` (M3, lot 11) ; `livrer_aux_appareils` (lot 12) avant `traiter`, qui prend l'heure (lot 11 : `traiter(t_us, …)`) ; `reponse_a_la_ligne` (lot 11) puis le module de tests du lot 12.
- **`web/autorite/src/worker.rs`** : la coque garde `m13` (lot 12) et `point_rapporte` (lot 11).
- **`crates/banc/src/soiree.rs`** (cinq conflits) : **la règle des sauts**, la seule résolution qui ne garde pas les deux côtés mot pour mot. Le lot 11 (correction) et le lot 12 (correction, constat 8) avaient chacun rattaché le saut d'une reprise à `de` de `p` à `p + rattrapage` (la même règle, deux écritures) ; ils différaient pour **un retard injecté** : le lot 11 le rattache de `p` à `p + rattrapage + 1`, et `arriere-plan` lui passe le rattrapage du jeu (§ 8.3 de PHASE-3, écrit par le lot 11) ; le lot 12 gardait `p + 2` « quel que soit le rattrapage », et `arriere-plan` passait 1 (« le lot 12 ne rejoue pas `arriere-plan` »). **La règle du lot 11 est gardée** : c'est celle que PHASE-3 écrit, elle suit `partie::regles::cible` (la partie retardée joue `rattrapage` pas avant de sauter, comme la partie reprise), et les deux règles ne diffèrent qu'à un rattrapage au-delà de 1 (Sidequest) ; aucun scénario du lot 12 n'avait éprouvé `p + 2` sur Sidequest. Les deux tests sont gardés ; dans celui du lot 12 (`un_saut_se_rattache_au_rattrapage_du_jeu_une_fois`), les deux assertions du retard injecté à `rattrapage: 3` sont réécrites sur la règle gardée, **avec la même rigueur** (la borne vue des deux côtés : `904` rattaché, `905` seul) et deux de plus à `rattrapage: 1` (`902` rattaché, `903` seul), et son commentaire le dit ; celui du lot 11 est inchangé. `juger_les_sauts_caches` passe le rattrapage du jeu (lot 11). Les deux aides `ligne` des tests sont fondues en celle du lot 12 (`use super::*`). Trois contrôles par mutation, rouges par les tests nommés (`target/travail/fusion-11/controles.py`) : le retard un pas plus loin, la règle du lot 12 remise, la reprise un pas plus loin.

Les versions : `salle::ecran::PROTOCOLE` n'a été montée que par le lot 11 (3, `CommandeEcran::Bots`) ; le lot 12 n'a touché ni `salle::ecran` ni `salle::fil` (`PROTOCOLE` reste 1). `VERSION_CASES` n'a changé d'aucun côté (le lot 12 écrit le point d'un bloc avec les mêmes octets) : aucune montée de plus, aucun octet figé à réécrire. `jeu`, `jeu-abi` et `jeu-derive` n'ont changé d'aucun côté : les modules des jeux gardent leur empreinte.

Les `Cargo.lock` (le commun et `web/`) se sont fusionnés seuls et ne sont pas touchés à la main : `cargo metadata --locked` les accepte dans les deux workspaces (aucune réécriture nécessaire), `web/Cargo.lock` gagne `telephone` (lot 12) et les arêtes `partie` et `unicode-segmentation` d'`ecran` (lot 11).

### Ce que le lot 11 a dû changer pour ce qui est entré

- **`banc::ecran`** construit un `soiree::Options` pour ouvrir l'écran ; le lot 12 y a ajouté `pages`, `duree_min_s` et `retardataires` : l'écran de Tier list les prend vides (aucune page de téléphone, aucune durée, pas de retardataires). Sans cela, rien ne compile.
- **La soirée de devine par la page** (`une_soiree_de_devine_par_la_page_survit_a_ses_pannes`, lot 12) était rouge au premier passage de `tests` sur l'arbre fusionné, sur une seule assertion : le singe n'avait joué aucun geste (`gestes_du_singe` 0) ; tout le reste tenait. Cherché : sur l'arbre du lot 12 seul (`git archive e719045`, `target/travail/fusion-11/refonte-src/`, sortie `refonte-devine.txt`), le test passe, **avec un seul geste du singe** dans toute la soirée. Ce n'est donc pas une régression du lot 11 mais une assertion tenue par le hasard : `Ligne` donnée à la garde par l'autorité (lot 11) au lieu d'aller à la page change la trajectoire de devine (qui demande ses lignes de résultats), donc les tirages de la graine 21 ; sur l'arbre fusionné, dix graines (21 à 30, `target/travail/fusion-11/graines.sh`) donnent cinq rouges, toujours sur cette seule assertion (le singe à 0 ; 1 à 8 ailleurs). Deux causes, vues en traçant la graine 28 : (1) la page de devine n'offre un champ et un bouton que quelques réveils par manche (4 à 46 dans la soirée), et le singe tirait sa sorte parmi les cinq (bouton, champ, choix, croix, maintenu) : trois tirages sur cinq tombaient sur une sorte absente et perdaient leur tour ; (2) une page rechargée (`Soiree::recharger`) retirait son `JoueurDePage`, et avec lui ses comptes : à la graine 28, le singe, rechargé six fois, finissait à zéro quoi qu'il eût joué. **Corrigé sans toucher l'assertion ni la graine** : `salle::page::sorte_du_singe` tire la sorte parmi celles que la page offre (`PageDuTelephone::singe`, dont les croix passent par `les_croix` pour tenir sous les 100 lignes) ; `Soiree::recharger` remet un joueur neuf qui garde les comptes de l'ancien. Après : vingt graines (21 à 40) vertes, le singe de 2 à 20 gestes. Deux tests de plus : `le_singe_tire_parmi_ce_que_la_page_offre` (`salle::page`), et dans la soirée de devine, le singe (téléphone 0) a joué, et une page rechargée garde ses comptes. Contrôles par mutation (`controles.py`) : le tirage parmi les cinq remis, et les comptes perdus au rechargement, chacun rouge par son test. Un essai intermédiaire (le singe qui attend une page qui offre de quoi jouer) n'est pas gardé : sa mutation ne rendait rien rouge, et les vingt graines tiennent sans lui. Ce que cela change hors des tests : le singe natif du scénario `soiree` (le premier téléphone natif) joue plus souvent.
- Rien d'autre : le lot 12 ne touche pas `web/ecran` (ni la page, ni le geste, ni le clavier de la remontée), ni `salle::ecran` ; ses scénarios ouvrent l'écran par le même `soiree::ouvrir` (avec la requête du lot 11) et ne donnent rien au jeu par les entrées de la télé : les gestes viennent des téléphones, natifs ou pages. Les clics du banc sur la page du téléphone (`window.__banc.centre(…)` de `web/telephone`) ne passent pas par le code de la remontée.

### Ce que les jeux voient

Rien : leur API ne change pas, ni les modules (mêmes sha256 qu'aux lots 7 à 12), ni ce que l'épreuve joue ou juge (le plan court rend la référence, contrôle pour contrôle). Construire Tier list depuis ses sources réécrit son `Cargo.lock` (les dépendances de `mise-en-page`, que l'épreuve tire depuis le lot 8 par le `[patch]`, n'y étaient pas) : constaté ici (le premier passage de `construire`, 21 h 14), le fichier réécrit gardé dans `target/travail/fusion-11/tier-list-Cargo.lock.reecrit`, puis **remis** (`git -C games/tier-list checkout -- Cargo.lock`) après les preuves : le dépôt du jeu est propre à la fin, mais **aucune des 24 preuves n'a tourné sur un dépôt Tier list propre** (chacune relève au départ et à la fin le `Cargo.lock` réécrit, `b679c8f7…` ; *précisé à la correction*, plus bas) ; à son agent de le mettre à jour. Le module construit avec le fichier réécrit a le même sha256 (`84315a1b…`).

### Les vérifications, et l'index des preuves

Dans `target/preuves/fusion-11/<nom>/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée de bout en bout, attente du portillon comprise), `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/fusion-11/preuve.sh`, `final.sh`, `final-2.sh`, `empreinte.sh` (l'empreinte relève aussi `MERGE_HEAD`, le sha256 des modules et le dernier paquet `web/`), `controles.py`, `comparer.py`), lancées à la suite par `final.sh` de 21 h 34 min 52 s à 22 h 23 min 08 s (UTC, `target/travail/fusion-11/final.log`), puis `final-2.sh` (les deux scénarios du lot 12 relancés, jusqu'à 22 h 40 min 32 s). **Les 24 preuves ont la même empreinte du code au départ et à la fin** : le commun `HEAD` `e719045`, `MERGE_HEAD` `2d95152`, `git diff HEAD` hors `.md` `50fae729fc909c24`, aucun fichier non suivi ; Punchline et Sidequest sans aucune modification ; Tier list avec son seul `Cargo.lock` réécrit (`b679c8f73cb243fe`, voir plus haut, remis après les preuves) ; les modules `82669f77…` (Punchline), `62043526…` (Sidequest), `84315a1b…` (Tier list), construits depuis leurs sources par la preuve `construire` ; le paquet `web/` joué, `target/soiree-web/bd7e65992c2dc04b3c019c73e61de6b3`, construit par `construire` sur l'arbre des preuves (les preuves d'avant `construire` relèvent un paquet plus ancien, qu'elles n'utilisent pas). Ce journal, PHASE-3, MESURES-PHASE-3, CHANGELOG et `crates/README.md` ont été écrits pendant les preuves : le `diff` entier change, pas celui hors `.md`.

Un **premier passage** (`target/preuves/fusion-11/premier-passage/`, `target/travail/fusion-11/final-premier-passage.log`, de 21 h 04 à 21 h 14) a été arrêté au rouge de `tests` (la soirée de devine par la page, ci-dessus) ; tout ce qui suit a été refait après la correction.

| Preuve | Commande | Résultat |
|---|---|---|
| `fmt`, `fmt-web` | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `controles` | `lourd python3 -I target/travail/fusion-11/controles.py` | code 0 : **5 mutations sur 5 rouges par les tests nommés** (`FAILED`, sans erreur de compilation), chaque source remise (sha256 vérifié) : le retard injecté un pas plus loin, la règle du retard du lot 12 remise, la reprise un pas plus loin (les deux tests des sauts), le singe qui tire parmi les cinq (`le_singe_tire_parmi_ce_que_la_page_offre`), les comptes perdus au rechargement (la soirée de devine par la page) |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; le même pour `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` | code 0, code 0 |
| `clippy-web`, `clippy-web-wasm32` | dans `web/`, natif `--all-targets`, puis `wasm32` `--lib --examples` | code 0, code 0 |
| `tests-web` | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : **67 réussis** (54 au lot 12, 64 à la remontée du lot 10b dans le lot 11, 51 en commun au lot 10b : 54 + 64 − 51), 0 échec |
| `tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 730 réussis** (721 à la correction du lot 12, 713 à la remontée, 705 en commun au lot 10b : 729 ; et `le_singe_tire_parmi_ce_que_la_page_offre`), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 498 s ; 1,82 Go ; la charge a atteint 11,15 (d'autres travaux de l'hôte) : une durée relevée |
| `construire` | `lourd sh -c "cargo build -p banc … -p relais … && cargo build --release -p epreuve … && soiree-epreuve --construire <chaque jeu> && soiree-relais preparer && shasum …"` | code 0 ; les trois modules construits depuis leurs sources (le `[patch]` vers ce commun), aux mêmes sha256 qu'aux lots 7 à 12 (`modules.txt`) ; paquet `bd7e6599…` |
| `court-punchline`, `-sidequest`, `-tier-list` | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 1, 1, 1 : **chaque contrôle identique à la correction du lot 12** (identifiant, issue, nombre d'évaluations, détail ; `comparer.py` sur les rapports JSON : 0 écart sur 21 contrôles chez chacun ; E16 chez les trois, E2 chez Punchline) |
| `ecran` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox,safari --plein 64 --sans-construire …` | **code 0 : Chrome WebGPU, Chrome WebGL2 forcé et Firefox réussis**, chaque étape faite : rien au jeu sous l'écran de démarrage, « Jouer », import (6 images), glissers, poubelle, titre en place, rechargement et « Cliquer pour reprendre » (le plateau retrouvé, images comprises), export capté (PNG 2 400 × 1 294) et sauvegarde captée (par le geste), restauration, plateau plein (64 et 56 images), sa sauvegarde lue entière et dite déjà ouverte, M3, M8 ; Safari pas exécuté (`safaridriver` refuse la session) ; 135 s ; 0,81 Go ; charge 1,58 au départ, 2,04 à la fin, **2,26 au plus** : M3 et M8 mesurés au calme (`MESURES-PHASE-3.md`, « Fusion du lot 11 ») |
| `ecran-dpr2` | `lourd … ecran tier-list <tier-list.wasm> --navigateurs chrome,firefox --plein 0 --dpr 2 --sans-construire …` | **code 0** : les trois passages réussis au rapport de pixels 2 |
| `ecran-fautif-activation-perdue` | `… --navigateurs chrome,firefox --sans-webgl2 --plein 0 --fautif activation-perdue …` | **code 1, pour la raison attendue** dans les deux : « la sauvegarde n'est pas captée (1 téléchargement(s) laissé(s) au bouton du commun) » |
| `ecran-fautif-geste-au-jeu` | `… --fautif geste-au-jeu …` | **code 1, pour la raison attendue** dans les deux : le clic sur « Jouer » et celui sur « Cliquer pour reprendre » vont au jeu (lignes `au-jeu`), puis le plateau n'est pas retrouvé (Reset joué) |
| `ecran-fautif-ne-couvre-pas` | `… --fautif ne-couvre-pas …` | **code 1, pour la raison attendue** dans les deux : le clic et l'image sous l'écran de démarrage vont au jeu, l'image arrive dans la réserve |
| `m12-tier-list` | `lourd --calme … soiree <tier-list.wasm> --navigateurs chrome,firefox --m12 3 --sans-construire …` | **code 0** : trois reprises par navigateur au pire moment, chacune de même forme que wasmtime, la chaîne entière vérifiée ; 96 s ; 3,62 au plus (aucune durée revendiquée) |
| `soiree-punchline` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <punchline.wasm> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 5 --sans-construire …` | **code 1** : **Chrome réussi** (pages accueillies, M4 ; rechargement en pleine écriture, le geste accusé après ; jumeau ; file pleine provoquée, 32 placés et 8 refusés, tout accusé au dégel ; « On rejoue » ; aucun secret et le témoin positif vu ; quatre sauts rattachés) ; **Firefox en échec sur un seul constat** : « téléphone natif 2 : aucun geste envoyé par la page », ses comptes tous à zéro, y compris les coups sans chemin : le bot de Punchline ne lui a jamais donné un coup (voir « Ce qui n'est pas fait ») ; tout le reste tient ; 331 s ; 3,42 au plus |
| `soiree-punchline-2` | la même, relancée (`final-2.sh`) | **code 0** : Chrome et Firefox réussis (chaque natif a envoyé des gestes par la page, de 3 à 40) ; 337 s ; 2,83 au plus |
| `soiree-sidequest` | `lourd --calme … soiree <sidequest.wasm> --navigateurs chrome,firefox --pages chrome,firefox,safari --minutes 5 --sans-construire …` | **code 1** : **Chrome réussi** (cinq minutes ; quatre reprises de même forme ; quatre sauts rattachés, chacun trois pas après sa reprise, `rattrapage: 3` ; pages, rechargement, jumeau, coupure P28 relâchée côté page, file pleine, « On rejoue », secrets) ; **Firefox en échec sur un seul saut d'un pas** (5 248 → 5 249) qu'aucune reprise n'explique : un réveil de la minuterie après un trou de 76,1 ms, tour de 1,0 ms (la question 30, ce qui en reste, comme au lot 12) ; tout le reste tient ; 670 s ; 3,01 au plus |
| `soiree-sidequest-2` | la même, relancée | **code 0** : Chrome et Firefox réussis ; 674 s ; 3,55 au plus |
| `soiree-71` | `lourd --calme … soiree <sidequest.wasm> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 69 --retardataires --sans-construire …` | **code 0 : Chrome et Firefox réussis** (au lot 12, Firefox échouait sur un tour d'écho retardé de 58 ms) : 69 natifs puis les deux pages, une à une ; pendant chaque livraison, les allers-retours d'écho des natifs restent dans la tolérance (médianes de 11,1 à 21,3 ms pendant, de 11,1 à 11,6 ms avant) ; M13 relevée (Chrome : part du commun, 99ᵉ centile 1,7 ms ; Firefox 2,2 ms) ; 89 secrets absents, le témoin vu ; 638 s ; 0,89 Go (le banc) ; **5,33 au plus** : rien de mesuré |

**Non relancés, et pourquoi** : le plan standard (le lot 11 ne change ni ce que l'épreuve joue ni ce qu'elle juge, et le plan court le prouve, contrôle pour contrôle, sur les modules construits) ; `dessin` (ni `web/rendu` ni la toile de `web/ecran` ne changent à la fusion : le lot 12 ne touche pas `web/ecran`, et le dessin a été rejoué au lot 11) ; `aide` (le lot 10b, rejoué à la remontée ; le lot 12 ne touche ni la page de l'écran ni ses entrées) ; `arriere-plan` (sa règle du retard injecté est celle que le lot 11 a écrite, jugée ici par les tests et trois mutations ; le scénario lui-même ne s'est pas rejoué depuis le lot 10a, et ne l'est pas ici) ; `soiree-punchline-sans-pages` (le même scénario que `soiree-punchline` sans les pages, couvert par lui) ; E20 (ni le moteur, ni la réécriture, ni les modules ne changent).

### Les mesures

M3 et M8 refaits au calme sur le code fusionné (`MESURES-PHASE-3.md`, « Fusion du lot 11 ») : le point de Tier list au pire, 18 à 19,2 Mio comme au lot 11, mais **son encodage dans le tour du point passe de 41-112 ms à 19-31 ms** (le point écrit d'un bloc de la correction du lot 12, entré par la fusion) ; les tranches au plus 2,19 ms (Chrome) et 2,76 ms (Firefox) ; la fin du point (38 à 84 ms) et la case de journal de la restauration (82 à 127 ms) restent chacune dans un tour (question 31) ; l'export du plateau plein en 111 à 132 ms, aucune image de la page au-delà de 33 ms, une image perdue par passage de Chrome. Les scénarios `soiree` ont dépassé une charge de 3 (de 3,01 à 5,33), sauf `soiree-punchline-2` (2,83) : M4, M7 et M13 qu'ils rapportent sont relevés, pas revendiqués comme mesures.

### Ce qui n'est pas fait, et pourquoi

- **Le lot 12 reste inachevé** : `CommandeEcran::ChangerDeJeu`, les pages de pire cas et leur fautif de module, Safari, et le reste de « Lot 12, correction », « Ce qui n'est pas fait ». La fusion ne les fait pas.
- **Les scénarios du lot 12 ne sont verts qu'une fois sur deux** sur le code fusionné, chacun sur un seul constat, et verts à la relance : Punchline (le natif 2 sans geste, dans Firefox) et Sidequest (un saut d'un pas après un réveil en retard de 76 ms, dans Firefox, la question 30). *Cause établie à la correction (plus bas, « Fusion du lot 11, correction ») : une régression de la fusion, le singe du natif 0 qui lance chaque partie de son bouton « Tout le monde est là » ; ce qui suit est le constat d'avant.* Le natif sans geste n'est pas expliqué : ses comptes sont tous à zéro, coups sans chemin compris, donc le bot de Punchline ne lui a donné aucun coup en cinq minutes (il rejoint d'un « Rejoindre » sur l'écran du prénom, et ne fait rien sur `PasDeLaPartie`) ; l'hypothèse d'un natif arrivé après le lancement de la partie n'est pas vérifiée (les lignes du banc ne disent ni l'écran de chaque natif ni le moment de son accueil). Au lot 12 déjà, des natifs de Punchline n'envoyaient que 6 gestes ; à la relance ici, 3. Le singe change de comportement avec la fusion (il tire parmi ce que la page offre, et joue donc plus : 17 et 18 gestes au premier lancement, 5 et 18 à la relance, contre 1 à 11 au lot 12) ; aucun lien entre ce singe et le natif 2 n'est établi, ni écarté. Le contrôle n'est pas changé.
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Les contrôles négatifs scriptés des lots 10a, 10b, 11 et 12** (`controles.py` de chacun, les mutations du dpr de la remontée) ne sont pas rejoués en entier : ceux que la fusion touche le sont par `target/travail/fusion-11/controles.py` (la règle des sauts, le singe, les comptes) ; les scénarios gardent leurs contrôles négatifs intégrés (les trois fautifs d'`ecran`, le débordement ajouté par la page, le témoin positif des secrets), rejoués ici.
- `arriere-plan`, `aide`, `dessin`, le plan standard : non relancés (plus haut).

### Ce que le lot suivant doit savoir

- La règle des sauts est une : une reprise de `p` à `p + rattrapage`, un retard injecté de `p` à `p + rattrapage + 1` (`banc::soiree::rattacher_les_sauts`), et `arriere-plan` passe le rattrapage du jeu. Elle n'a été jouée par `arriere-plan` sur aucun jeu à `rattrapage: 3` (*jouée depuis, à la correction : Sidequest, douze sauts du retard, chacun à `p + 3`*).
- Le singe d'une page tire parmi ce que la page offre (`salle::page::sorte_du_singe`) ; une page rechargée de `salle::simulation` garde les comptes de son téléphone (les natifs du banc ne rechargent pas leur joueur).
- Le `Cargo.lock` de Tier list n'a pas les dépendances de `mise-en-page` : le construire depuis ses sources le réécrit ; à son agent de le mettre à jour (ou à un contrôle de passer `--locked`).
- Le natif de Punchline sans aucun coup du bot reste à comprendre (une ligne du banc qui dirait l'écran de chaque natif et le pas de son accueil le trancherait). *Compris à la correction, par le journal relu de la garde : voir « Fusion du lot 11, correction ».*
- Les outils de la fusion : `target/travail/fusion-11/` (`hunk.py`, `fusion3.py`, `verifier.py` pour résoudre et vérifier une fusion de documents ; `graines.sh` ; `refonte-src/`, les sources du lot 12 seul, sans leur `target`).

## Fusion du lot 11, correction après la vérification

Faite le 2026-11-07, de 22 h 50 à 23 h 30 environ (UTC), dans `soiree/` (branche `refonte`, la fusion toujours en cours : `HEAD` `e719045`, `MERGE_HEAD` `2d95152`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). **Aucun fichier de code n'est changé** : seuls ce journal, `MESURES-PHASE-3.md` et `CHANGELOG.md` le sont ; le `git diff HEAD` hors `.md` reste `50fae729fc909c24`, les trois jeux sont propres (Tier list compris, son `Cargo.lock` remis), les modules aux mêmes sha256. Rien n'est commité. Les outils jetables sont dans `target/travail/fusion-11-corr/` (hors dépôt, aucune dépendance ajoutée à un dépôt).

### Les constats, et ce qui en est fait

1. **[important] Le natif 2 de Punchline sans aucun coup, dans Firefox : une régression de la fusion, cause établie.** Vérifié, puis cherché jusqu'à la cause. Un outil jetable (`target/travail/fusion-11-corr/journal/`, `journal-du-banc`) relit la chaîne entière d'un passage (l'enregistrement du relais et la garde sur disque, déchiffrés par G, comme `banc::soiree::sauts`) et écrit chaque arrivée placée ; `chronologie.py` et `lancements.py` nomment les gestes d'après `GesteTel` et `ActionEcran` de Punchline (la variante au premier octet). Relu sur les quatre passages de la fusion et sur les quatre de « Lot 12, correction » (`final-punchline`, `final-punchline-2`) :
   - L'appareil 2 est le natif 0, le singe (ses gestes au journal, 48, font ses 31 coups du bot et ses 17 du singe), l'appareil 4 le natif 2.
   - **Avant la fusion, chaque partie est lancée par l'écran** (le bot de l'écran, « Lancer » deux secondes après le dernier changement du salon), **135 à 180 pas après l'ouverture du salon**, les natifs arrivés ayant rejoint ; le singe n'appuie jamais sur « Tout le monde est là » ni sur « On rejoue » (huit parties, quatre passages).
   - **Après la fusion, dans trois passages sur quatre, chaque partie est lancée par le singe** : le natif 0 rejoint le premier, devient le VIP (le premier humain présent), et appuie sur « Tout le monde est là » **5 à 15 pas** (80 à 250 ms) après l'ouverture du salon, puis, à chaque fin, sur « On rejoue » et de nouveau sur « Tout le monde est là » 42 à 45 pas plus tard (une période du singe, 700 ms). Un natif qui n'a pas rejoint dans cette fenêtre reste sur `PasDeLaPartie` toute la partie : le bot de Punchline ne lui donne alors aucun coup. Dans le passage Firefox en échec, l'appareil 4 (le natif 2) n'a rejoint aucune des trois parties de la soirée ; dans les deux autres passages lancés par le singe, les natifs à 2 et 3 gestes ne sont entrés que dans la dernière partie. Le quatrième passage (Chrome, relance), où un autre natif a rejoint le premier, est lancé par l'écran comme avant la fusion.
   - **La cause** : le singe tire maintenant sa sorte parmi celles que la page offre (`salle::page::sorte_du_singe`, écrit à la fusion pour la soirée de devine) ; la page du VIP au salon n'offre que des boutons (les personnages libres et « Tout le monde est là »), et le singe y appuie à chacun de ses tours, au lieu d'un tour sur cinq. Le `bots` du lot 11 dans `Autorite::banc` n'y est pour rien : il reste `Oui` (le scénario `soiree` ne passe pas `?bots=non`, seul `banc::ecran` le fait), et il est commun à tous les téléphones, alors que les natifs 1, 3 et 4 recevaient leurs coups au même moment. `traiter(t_us, …)` non plus : Punchline ne fait aucune demande (`fn demandes` vide), et ses scènes ne citent aucune image.
   - **Ce que ce n'est pas** : une faute du produit. Un VIP qui lance vite est une soirée possible, et Punchline dit `PasDeLaPartie` comme il le doit. C'est le **contrôle du scénario** (« chaque natif a envoyé un geste par la page », correction du lot 12) qui devient tributaire du singe : un passage sur quatre l'a vu rouge ici, et la couverture baisse (moins de natifs passent par la page dans chaque partie). Ce n'est donc pas le même bruit que les sauts d'un pas du lot 12 : c'est un **risque ouvert, causé par la fusion**, et dit tel.
   - **Ce qui n'est pas fait** : aucune correction de code. Le contrôle n'est pas affaibli (un natif sans aucun coup reste une faute), le tirage du singe n'est pas défait (la soirée de devine retomberait), et choisir comment le singe et les natifs se partagent le salon (par exemple armer le singe après le premier lancement, ou le laisser hors du premier salon) change le scénario de § 8.1 : c'est au lot qui reprend le lot 12 d'en décider. Preuve : `target/preuves/fusion-11-corr/natifs-de-punchline/` (`lancements.txt`, `chronologie.txt`, les cases relues, l'outil et sa commande).
2. **[mineur] La règle des sauts à `rattrapage: 3` : jouée par `arriere-plan` sur Sidequest.** Vérifié : le constat est juste, aucun scénario ne l'avait jouée. `lourd --calme … soiree-banc arriere-plan <sidequest.wasm> --navigateurs chrome,firefox --cache 90 --fois 2` : **code 0, quatre passages réussis** (deux par navigateur), chacun onglet caché 90 s (5 354 à 5 357 pas joués, 5 149 à 5 210 par la minuterie, aucun au signal d'une image), le retard injecté signalé, **ses trois sauts rattachés dans chaque passage, et chacun tombe à `p + 3`** (`p + rattrapage`) : (4 856, 4 862) au retard du pas 4 853, (4 865, 4 871) à 4 862, (4 874, 4 880) à 4 871 dans Chrome, de même dans les trois autres passages ; aucun saut seul, caché ou visible. La règle du lot 12 (`p + 2`) aurait vu ces douze sauts comme des fautes ; la règle gardée les rattache tous, et **sa marge d'un pas (`p + 4`) n'a servi à aucun**. La charge a atteint **7,75** pendant (d'autres travaux de l'hôte ; 2,96 au départ, 2,68 à la fin) : ce n'est pas une mesure de temps et aucune durée n'en est tirée ; les trous de plus de 30 ms hors du retard (six dans Firefox, cinq puis un, de 30,3 à 47,9 ms) n'ont fait aucun saut. Preuve : `target/preuves/fusion-11-corr/arriere-plan-sidequest/`.
3. **[mineur] Les comptes gardés au rechargement : laquelle des assertions tombe.** Vérifié par `target/travail/fusion-11-corr/quelle_assertion.py` (par le portillon, chaque source remise, sha256 vérifié) : (a) sous la mutation « comptes perdus au rechargement », **seule l'assertion ajoutée par la fusion tombe** (`crates/salle/tests/devine.rs:152`, la page rechargée garde ses comptes) ; (b) la même mutation, ces deux lignes retirées : **le test passe** : les assertions d'origine du lot 12 (`gestes_par_la_page`, `tenus_par_la_page`, `gestes_du_singe` `> 0`) tiennent sur les seules pages vivantes à la fin (le singe : 3 gestes depuis son dernier rechargement ; la somme : 10 gestes et 7 tenus par la page) ; (c) sans mutation, vert (le singe 4 en tout). À la graine 21, l'assertion d'origine ne doit donc rien aux comptes gardés ; elle reste plus facile à tenir pour une autre graine, et ce journal le dit. Le test n'est pas changé. Preuve : `target/preuves/fusion-11-corr/quelle-assertion/`.
4. **[mineur] Le `Cargo.lock` de Tier list.** Vérifié, et plus large que dit : les 24 preuves de la fusion relèvent toutes, au départ et à la fin, le `Cargo.lock` réécrit (`b679c8f73cb243fe`, de 21 h 34 à 22 h 40) ; il l'était depuis le premier passage de `construire` (21 h 14). Aucune preuve n'a donc tourné sur un dépôt Tier list propre. Le fichier ne gagne que des paquets (les dépendances de `mise-en-page`, 252 lignes), aucun code du jeu ni du commun ne change, et le module construit a le même sha256 (`84315a1b…`) : les preuves restent celles du même module. La phrase « le dépôt du jeu reste propre » de la section de la fusion est corrigée. Les preuves de cette correction tournent, elles, sur un Tier list propre. Le défaut date du lot 8 ; à l'agent de Tier list.
5. **[mineur] M12 de Tier list sur le code fusionné.** Vérifié : la preuve de la fusion avait atteint 3,62 et ne revendiquait aucune durée. Refait au calme : voir « Les mesures ».

### Ce que les jeux voient

Rien : aucun code ne change dans cette correction.

### Les vérifications, et l'index des preuves

Dans `target/preuves/fusion-11-corr/<nom>/` (`commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` par `target/travail/fusion-11-corr/preuve.sh` et `target/travail/fusion-11/empreinte.sh`) ; empreinte de chacune : le commun `HEAD` `e719045`, `MERGE_HEAD` `2d95152`, `diff-hors-md` `50fae729fc909c24`, aucun fichier non suivi ; Punchline `f84b86f`, Sidequest `229199b`, Tier list `9827f39`, **tous trois sans modification** ; modules `82669f77…`, `62043526…`, `84315a1b…` ; paquet `web/` `bd7e6599…` (celui des preuves de la fusion). `soiree-banc` reconstruit d'abord (`lourd cargo build -p banc --bin soiree-banc`, 3 s : les mutations de la vérification avaient laissé `salle` et `banc` à recompiler ; binaire `441b64b0…`).

| Preuve | Commande | Résultat |
|---|---|---|
| `natifs-de-punchline` | `journal-du-banc` sur les huit passages, puis `lancements.py`, `chronologie.py` (lecture seule) | avant la fusion, 8 parties sur 8 lancées par l'écran à +135 à +180 pas ; après, 9 parties sur 11 lancées par le singe (natif 0, VIP) à +5 à +45 pas ; le natif 2 du passage Firefox en échec n'a rejoint aucune des trois parties |
| `quelle-assertion` | `lourd python3 -I target/travail/fusion-11-corr/quelle_assertion.py` | code 0 ; (a) rouge à `devine.rs:152` seulement ; (b) vert ; (c) vert ; sources remises (sha256 vérifiés) ; 21 s |
| `arriere-plan-sidequest` | `lourd --calme /usr/bin/time -l target/debug/soiree-banc arriere-plan <sidequest.wasm> --navigateurs chrome,firefox --cache 90 --fois 2 --sans-construire …` | **code 0**, 4 passages réussis, 12 sauts du retard rattachés, chacun à `p + 3` ; 430 s ; charge 2,96 au départ, 2,68 à la fin, 7,75 au plus (pas une mesure) |
| `m12-tier-list-ecarte-1`, `-2` | `lourd --calme … soiree <tier-list.wasm> --navigateurs chrome,firefox --m12 3 --sans-construire …` | code 0, réussis, mais charge 3,75 puis 3,77 au plus : **écartés** (règle posée avant) |
| `m12-tier-list` | la même, lancée par `target/travail/fusion-11-corr/m12.sh` quand la charge est sous 1,6 | **code 0** : trois reprises par navigateur au pire moment, chacune de même forme que wasmtime ; charge 1,52 au départ, 2,13 à la fin, 2,07 au plus : **M12 mesuré** (plus bas) ; 56 s |

Non relancés : `tests`, `clippy`, `fmt`, le plan court, `ecran` et les scénarios `soiree` : aucun fichier de code n'a changé depuis les preuves de la fusion (même `diff-hors-md`, mêmes modules, même paquet), leurs rapports dans `target/preuves/fusion-11/` valent toujours ; `soiree-punchline` n'est pas rejoué : son échec est maintenant expliqué, et un passage de plus ne trancherait rien (le singe ne lance la partie que s'il rejoint le premier).

### Les mesures

**M12 de Tier list, sur le code fusionné, au calme** (`MESURES-PHASE-3.md`, « Fusion du lot 11, correction ») : la partie reprise au pire moment en **226 à 289 ms dans Chrome**, **397 à 701 ms dans Firefox**, toujours sous la seconde ; 701 ms est le plus long relevé pour Tier list (536 au lot 10a corrigé), sur un tour, non expliqué. Deux passages écartés d'abord (charge 3,75 et 3,77).

### Ce qui n'est pas fait, et pourquoi

- **Le natif de Punchline sans aucun coup** : la cause est établie (le singe du VIP qui lance la partie, constat 1), mais rien n'est corrigé : le choix (armer le singe plus tard, le tenir hors du premier salon, ou autre chose) change le scénario de § 8.1 et revient au lot qui reprend le lot 12. `soiree-punchline` reste donc rouge un passage sur quatre environ (un seul rouge sur les quatre passages de la fusion), sur ce contrôle.
- Le saut d'un pas de Sidequest dans Firefox (`soiree-sidequest`, la question 30) n'est pas repris : il n'était pas dans la vérification.
- L'assertion d'origine de la soirée de devine par la page n'est pas resserrée : elle tient sans les comptes gardés à la graine 21 (constat 3), mais rien ne l'oblige pour une autre graine.
- Le `Cargo.lock` de Tier list reste à l'agent de Tier list.
- Safari : pas exécuté.

### Ce que le lot suivant doit savoir

- Au salon de Punchline, le singe du natif 0 (`salle::page::sorte_du_singe`, depuis la fusion) appuie à chacun de ses tours sur les boutons du VIP ; s'il a rejoint le premier, il lance chaque partie en quelques pas, et un natif en retard ne joue plus. Avant de toucher au contrôle « aucun geste envoyé par la page », décider comment le singe partage le salon.
- Pour savoir qui a fait quoi dans une soirée du banc, relire la chaîne : `target/travail/fusion-11-corr/journal/` (`journal-du-banc <dossier d'un passage>`, qui lit `enregistrement.bin`, `garde/` et `cle.txt`), puis `lancements.py` ou `chronologie.py` (les noms des gestes de Punchline). Un outil de ce genre dans `banc` (les arrivées par appareil et par partie) aurait trouvé la cause du premier coup.
- La règle du retard injecté (`p` à `p + rattrapage + 1`) est jouée à `rattrapage: 3` : sur Sidequest, chaque saut du retard tombe à `p + 3` ; la marge d'un pas n'a servi à aucun des douze.

## Lot 12, fin

Faite le 2026-11-07 et le 2026-11-08, de 23 h 05 à 02 h 30 environ (heure de la machine, UTC), dans `soiree/` (branche `refonte`, `HEAD` `cda28f3`, la fusion du lot 11 commitée), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Chaque cargo, chaque scénario et chaque contrôle par mutation est passé par le portillon (`target/hote/lourd`), sauf des `cargo check -p` de quelques secondes ; un scénario à la fois ; chaque navigateur a été fermé par le banc (ceux d'un passage interrompu à la main, tués par leur profil). Rien n'est commité. **Seul le commun est modifié** ; Punchline (`f84b86f`), Sidequest (`229199b`) et Tier list (`9827f39`) restent propres, leurs modules aux mêmes sha256 qu'aux lots 7 à 12 (`target/preuves/lot-12-fin/modules.txt`). Les fichiers de travail sont dans `target/travail/lot-12-fin/` (hors dépôt).

**Les chemins du lot 11** : les preuves du lot 11 (« Lot 11 », sa correction, la remontée du lot 10b et sa correction) étaient dans le worktree `soiree-lot-11`, retiré depuis la fusion ; elles sont désormais sous `target/preuves/worktree-lot-11/` (`lot-11`, `lot-11-corr`, `remonter`, `remonter-corr`, `modules-figes`, `modules-figes-journal`), et les chemins de ces sections sont corrigés en conséquence. Leurs fichiers de travail (`target/travail/lot-11*`, `target/travail/remonter*` du worktree) n'ont pas été gardés : ces chemins-là ne mènent plus nulle part, et le journal le dit ici plutôt que de les réécrire vers un dossier qui n'existe pas.

### Ce qui est fait, dans l'ordre de la mission

**(0) La régression de la fusion du lot 11 : le singe partage le salon.** Deux règles, écrites au § 8.1 de PHASE-3 :
- **Le singe attend le salon au complet** : il ne joue aucun tour tant que chaque autre téléphone natif de la soirée n'a pas vu accuser un premier geste (`salle::page::salon_au_complet`, `JoueurDePage::retenir_le_singe`) ; le banc le calcule à chaque tour de ses natifs (le dernier accusé d'un natif ne redescend pas : le salon au complet le reste) ; `salle::simulation` le fait sur demande (`Soiree::singe_attend_le_salon`), la soirée de devine par la page gardant son singe libre. Le banc ne connaît pas le jeu : un geste accusé est ce qu'il sait de « ce téléphone a rejoint ».
- **Le singe n'appuie sur un bouton tiré qu'une fois sur cinq** (`page::SINGE_BOUTON_UN_SUR`, le rythme d'avant la fusion). Constaté au premier passage des preuves (`target/preuves/lot-12-fin/premier-passage/soiree-punchline/`) : avec la seule règle du salon, **aucune partie de Punchline ne commençait** (400 s sans phase terminale, dans Chrome et dans Firefox ; chaque natif à un seul geste, son `Rejoindre`, et le singe à 904) : le singe devenu VIP, seul bouton de sa page à chaque fois, lançait la partie (« Tout le monde est là ») puis l'annulait (« Attends, on annule ! ») à chacun de ses tours de 700 ms. Reproduit en mémoire par `soiree_en_memoire --par-la-page --singe-attend` (option ajoutée à l'exemple) : avant le rythme, chaque natif à 1 geste sur trois graines ; après, de 31 à 62.
Ni le contrôle « chaque natif a envoyé un geste par la page », ni le tirage du singe parmi ce que la page offre, ni la soirée de devine ne changent.

**(1) L'anomalie de M13 : le compte est juste, l'autorité n'oublie rien ; c'est le banc qui taisait ses natifs.** Relu dans les lignes du banc de `soiree-71` (fusion du lot 11, `target/travail/fusion-11/soiree-71/{chrome,firefox}/lignes.txt`) : dans les deux passages, **aucune ligne n'est lue de 73,0 s à 103,3 s** (30,29 s et 30,26 s), puis toutes celles du Worker d'un coup ; ce trou est l'essai de la page Safari des retardataires (`Lance::lancer(Safari)` : `safaridriver` met 30 s à refuser la session), fait sur le seul fil du banc, qui faisait aussi tourner les téléphones natifs. Pendant ces 30 s, les 69 natifs n'ont lu ni répondu à rien ; l'autorité, sans leurs échos, a perdu leurs liens (les lignes `etat` lues en retard montrent 71 appareils jusqu'au pas 4 497, puis 2 : les deux pages, dans leurs navigateurs) ; l'écran rechargé tout de suite après (la première reprise) a fait du Worker qui comptait 2 celui qui « a joué le plus de tours ». Les lignes `salle` disaient 71 parce qu'elles comptent les appareils de la salle, pas les liens accueillis. **Corrigé dans le banc** : les téléphones natifs tournent **sur leur propre fil** (`banc::soiree::Natifs`, `Troupe` ; le fil du banc n'y touche que sous un verrou, le temps d'y lire ou d'ajouter), et chaque silence de ce fil de plus de 250 ms est noté, **une faute au-delà de 1 s** (sous la coupure de 1,5 s d'un téléphone). Tenu par `les_natifs_tournent_pendant_que_le_banc_attend` (le fil du banc dort 300 ms, les natifs jouent au moins 30 tours ; il échoue sans le fil, mutation `natifs-sans-fil`) et `un_long_silence_des_natifs_est_une_faute` (le trou de 30 287 ms de `soiree-71` est une faute qui dit quand et combien).

**(2) Changer de jeu sans rien relancer.** `Autorite::changer_de_jeu` (§ 2.8 de PHASE-3, écrit) ; le Worker sur `CommandeEcran::ChangerDeJeu` ; l'écran de démarrage, dont chaque ligne choisit son jeu, « Jouer » qui demande le jeu choisi, F9 qui le remontre par-dessus la partie ; la page d'un téléphone (natif et navigateur) qui se refait sur le nouveau module ; un jeu sans téléphone protégé des téléphones encore reliés (ni présence, ni regard, ni bot) ; `verifier_l_historique` qui accepte la chaîne neuve d'un autre jeu. Le scénario `soiree-banc changer` et son contrôle négatif (§ 8.2, ligne `changer`).

**(3) Le scénario `pages` sur les pages de pire cas, avec un fautif de module : non fait** (plus bas).

**(4) Le retard d'écho des natifs dans Firefox à 71 appareils** : la cause se sépare désormais par construction (plus bas, « Les mesures » et `soiree-71`).

**(5) `aide` rejoué** sur l'arbre actuel (preuve `aide`).

**(6)** `Locaux::retenu` **survit au rechargement** de la page d'un téléphone (`localStorage`, une clé par salle et par jeu ; `PageDuTelephone::retenu`, `reprendre_le_retenu`) ; le scénario `soiree --pages` le vérifie à la page rechargée en pleine écriture (ce qu'elle avait retenu doit être retrouvé). Sidequest cinq minutes et `FilePleine` provoqué, faits à la correction du lot 12, sont rejoués (`soiree-sidequest`).

### Les écarts à PHASE-3, corrigés dans PHASE-3

1. **§ 8.1** : le singe partage le salon (les deux règles, plus haut) ; le texte de la fusion du lot 11 (« le singe tire sa sorte parmi celles que la page offre ») reste vrai.
2. **§ 2.8, « Changer de jeu »** : « nouvelle chaîne de garde » ne veut pas dire une autre suite de numéros (la garde n'en a qu'une par salle et refuse une garde plus vieille) : la chaîne du nouveau jeu part d'un point neuf au numéro suivant, et une reprise ne relit que lui et ce qui le suit. Écrit, avec la fenêtre qui reste (un Worker tué entre l'envoi du point neuf et l'écriture de l'enregistrement reprendrait ce point avec l'ancien module), et F9, la touche du commun qui remontre l'écran de démarrage (choisie). L'écart « changer de jeu n'est pas écrit » de l'écran de démarrage est barré.
3. **§ 8.2** : la ligne `changer` (le scénario, son contrôle négatif).
4. **§ 7, ligne du lot 12** et **§ 9.1, question 30** : ce que la fin du lot a fait, et les chiffres des sauts.
5. Les jeux sans téléphone (§ 2.8, écrit) : le commun ne leur donne, des téléphones encore reliés après un changement de jeu, ni présence, ni regard, ni bot ; aucun texte de PHASE-3 ne disait ce qu'un téléphone relié à Tier list devenait.

### Les tests qui la tiennent

- `crates/salle/tests/devine.rs` : `le_singe_attend_que_le_salon_soit_au_complet` (trois téléphones, deux qui ne se relient que 20 s après, les bots arrêtés tant que le singe est seul : sans la règle le singe seul joue, avec elle il ne joue aucun tour (`ComptesJoueur::tours_du_singe`) avant que les deux autres aient un geste accusé, puis au moins 30 tours en 30 s) ; `une_salle_change_de_jeu_sans_rien_relancer` (devine, puis ardoise, un jeu sans téléphone, puis devine : la partie neuve et son pas qui suit, le nom du jeu, mêmes liens, chaque téléphone sur le nouveau module, la garde qui reprend le nouveau jeu, l'autorité tuée qui reprend ardoise, chaque page refaite deux fois et qui rejoue devine, la chaîne entière vérifiée avec ses deux changements, tout accusé écrit, aucun geste deux fois).
- `crates/salle/src/page.rs` : `le_singe_n_appuie_qu_un_tour_sur_cinq_sur_un_bouton` (sur une page d'un seul bouton, 300 à 500 appuis sur 2 000 tours).
- `crates/banc/src/soiree.rs` : `les_natifs_tournent_pendant_que_le_banc_attend`, `un_long_silence_des_natifs_est_une_faute`.
- `web/ecran/src/demarrage.rs` : `chaque_ligne_choisit_son_jeu`.
- **Les contrôles par mutation** (`target/travail/lot-12-fin/controles.py`, critère strict : chaque test nommé `FAILED`, sans erreur de compilation, la source remise et son sha256 vérifié) : **~~11 sur 11~~ 10 sur 10 rouges** (*corrigé après la vérification* : le script n'en a que dix, et ses sorties dix lignes « ROUGE comme attendu ») : le singe jamais retenu, le singe à chaque tour sur un bouton, les natifs sans fil, un silence jamais une faute, la partie neuve au pas 0, la page jamais refaite, les regards d'un jeu sans téléphone, le module jamais relivré, l'historique sans chaîne neuve, les lignes de l'écran de démarrage sans bouton. *Une mutation essayée d'abord n'est pas gardée* : retirer la garde des présences d'un jeu sans téléphone (`arriver`) ne rend rien rouge (ardoise accepte une présence ; la garde reste, comme précaution, et ce journal le dit : rien ne prouve qu'elle soit nécessaire) ; la mutation des regards la remplace, et elle est rouge.
- **Les scénarios** et leurs contrôles intégrés : `changer` et son contrôle négatif ; dans `soiree`, les silences des natifs, et le retenu retrouvé au rechargement d'une page.

### Le scénario `changer`

`soiree-banc changer <punchline> <sidequest> <tier-list> --navigateurs chrome,firefox --pages chrome,firefox` : par passage, le relais lancé une fois avec les trois modules au catalogue, l'écran ouvert sur Punchline, six téléphones natifs, une page dans Chrome et une dans Firefox ; « Jouer », 20 s de Punchline ; puis, pour Sidequest puis Tier list : F9 (`Input.dispatchKeyEvent`, `input.performActions`), un clic sur la ligne du jeu, un clic sur « Jouer » (les centres donnés par `__banc.centre(<empreinte>)`), et le jugement (plus haut, § 8.2). Constaté dans chaque passage réussi : la partie neuve 50 à 80 pas après le dernier pas de l'ancienne ; les six natifs sur le nouveau module en moins de 30 s, leur page refaite (deux fois sur la soirée), et, chez Sidequest, les six qui rejouent par elle ; les deux pages sur le nouveau module (une page refaite, puis deux) ; les tours du Worker qui continuent (2 685 puis 3 947, par exemple) ; aucune ligne `page` ni `en-jeu` de plus ; la garde finale reprise par wasmtime avec le module de Tier list, à la forme et à l'empreinte de la partie figée ; aucun silence des natifs de plus de 250 ms. Le contrôle négatif : les natifs restent sur le module de Punchline (`46ebf4b5…`, celui du catalogue), et le scénario échoue, dans les deux navigateurs, sur « des téléphones natifs n'ont pas reçu son module en 30 s », pour Sidequest comme pour Tier list.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-12-fin/<nom>/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-12-fin/preuve.sh`, `empreinte.sh`, `final.sh`, `final-2.sh`, `final-3.sh`, `controles.py` ; journaux `final.log`, `final-premier-passage.log`). Quatre arbres de code se suivent, chacun dit par le sha256 de `git diff HEAD` hors `.md` du commun (`HEAD` `cda28f3`, aucun fichier non suivi hors `crates/banc/src/changer.rs`) ; les trois jeux sans aucune modification à chaque preuve ; les modules `82669f77…`, `62043526…`, `84315a1b…` (`modules.txt`), aux mêmes sha256 qu'aux lots 7 à 12 :
- **`15a8b675…`** (`final.sh`, de 00 h 45 à 01 h 43) : tout le code de la fin du lot ;
- **`bf74bb4d…`** : plus `RELIER_PAR_TOUR` (le fil des natifs relie au plus quatre téléphones par tour ; la première preuve `soiree-71` avait vu le tour qui reliait les 69 natifs taire le fil 1,6 à 1,7 s, au-delà de la borne de 1 s) ;
- **`74de73c4…`** : plus `Troupe::noter_le_tour` (le même code sorti de `tourner`, que clippy trouvait trop long ; rien de ce qu'il fait ne change) ;
- **`c97542cc…`** : plus `overflow-wrap: anywhere` sur les boutons et les maintenus de la page du téléphone (`soiree-punchline-3`, dans Firefox, a vu un bouton de vote de Punchline déborder à 768 × 1024 : un mot tapé par le singe, plus large que la page). C'est l'arbre final.
Ce journal, PHASE-3, MESURES-PHASE-3, CHANGELOG, `crates/README.md` et `AGENTS.md` ont été écrits pendant les preuves : le `diff` entier change, pas celui hors `.md`. Un **premier passage** (`premier-passage/`, `final-premier-passage.log`, de 23 h 57 à 00 h 40) a été arrêté à la main pendant `soiree-punchline-2` : `soiree-punchline` y avait montré Punchline sans aucune partie commencée (la seconde règle du singe en vient), `controles` une mutation pas rouge (la garde des présences, remplacée), `clippy` un test trop long.

| Preuve | Arbre | Commande | Résultat |
|---|---|---|---|
| `fmt`, `fmt-web` | 15a8 | `cargo fmt --all --check` (le commun, puis `web/`) | code 0, code 0 |
| `controles` | 15a8 | `lourd python3 -I target/travail/lot-12-fin/controles.py` | code 0 : **~~11~~ 10 mutations sur ~~11~~ 10 rouges** par leur test nommé (*corrigé après la vérification*) |
| `clippy`, `clippy-wasm32`, `clippy-web`, `clippy-web-wasm32` | 15a8 | `lourd cargo clippy --workspace --all-targets -- -D warnings` ; `wasm32` sur `mise-en-page`, `liaison`, `salle`, `partie`, `jeu`, `jeu-abi`, `moteur-commun` ; dans `web/`, natif puis `wasm32` | code 0, 0, 0, 0 |
| `tests-web` | 15a8 | `lourd sh -c "cd web && /usr/bin/time -l cargo test --workspace --no-fail-fast"` | code 0 : **68 réussis** (67 à la fusion du lot 11 ; `chaque_ligne_choisit_son_jeu`), 0 échec |
| `tests` | 15a8 | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 735 réussis** (730 à la fusion du lot 11 ; les cinq tests plus haut), 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 476 s ; 1,82 Go ; la charge a atteint 10,84 (d'autres travaux de l'hôte) : une durée relevée |
| `construire` | 15a8 | `lourd sh -c "cargo build -p banc … -p relais … && cargo build --release -p epreuve … && target/debug/soiree-relais preparer && shasum …"` | code 0 ; paquet `5eeaff85…` |
| `court-punchline`, `-sidequest`, `-tier-list` | 15a8 | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 1, 1, 1 : **0 écart sur 21 contrôles chez chacun** avec la fusion du lot 11 (`target/travail/fusion-11/comparer.py` sur les rapports JSON ; E16 chez les trois, E2 chez Punchline) : rien de ce que l'épreuve joue ou juge ne change |
| `changer` | 15a8 | `lourd /usr/bin/time -l target/debug/soiree-banc changer <punchline> <sidequest> <tier-list> --navigateurs chrome,firefox --pages chrome,firefox --sans-construire …` | **code 0 : Chrome et Firefox réussis** (plus haut) ; 143 s ; 0,79 Go ; 3,16 au plus |
| `changer-negatif` | 15a8 | la même, `--fautif changer-sans-livrer`, sans pages | **code 1, pour sa raison**, dans les deux navigateurs |
| `soiree-punchline` | 15a8 | `lourd --calme /usr/bin/time -l target/debug/soiree-banc soiree <punchline> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 5 --sans-construire …` | **code 0 : Chrome et Firefox réussis** (podium puis « On rejoue », rechargement en pleine écriture, jumeau, file pleine, secrets, débordements ; quatre sauts rattachés chacun ; chaque natif de 44 à 57 gestes par la page) ; 287 s ; 3,06 au plus |
| `soiree-punchline-2` | 15a8 | la même, relancée | **code 0** : les deux réussis ; 299 s ; **2,77 au plus** (M4, plus bas) |
| `soiree-71` | 15a8 | `lourd --calme … soiree <sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --telephones 69 --retardataires --sans-construire …` | **code 1, sur le seul contrôle nouveau** : « les téléphones natifs sont restés 1 597 ms (Chrome), 1 719 ms (Firefox) sans être servis », à 2,8 s du début du passage (le tour qui reliait les 69 natifs) ; tout le reste tient, et **l'anomalie de M13 a disparu** : 71 appareils accueillis au dernier rapport, dans les deux passages ; les retardataires sans retard (plus bas) ; 686 s ; 3,87 au plus |
| `soiree-sidequest` | 15a8 (le fil des natifs ; `RELIER_PAR_TOUR` écrit pendant, non compilé) | `lourd --calme … soiree <sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --minutes 5 --sans-construire …` | **code 0** : les deux réussis, 300 s de partie chacun, file pleine provoquée (32 placés, 8 refusés, tout accusé au dégel), page rechargée en pleine écriture **et ce qu'elle avait retenu retrouvé**, aucun silence des natifs ; 672 s ; 3,34 au plus |
| `aide` | ~~15a8 (binaire de `construire`)~~ bf74 (*corrigé après la vérification* : son empreinte dit `diff-hors-md` `bf74bb4d`) | `lourd /usr/bin/time -l target/debug/soiree-banc aide <punchline> <tier-list> --navigateurs chrome,firefox,safari --avec-interface --sans-construire …` | **code 0** : Chrome et Firefox réussis, le contrôle négatif (l'aide tue) rouge comme attendu dans chacun ; Safari pas exécuté (`safaridriver` refuse la session) ; 346 s |
| `fmt-2`, `controles-2`, `tests-banc-2`, `construire-2` | bf74 | `final-2.sh` | code 0 (~~11 sur 11~~ 10 sur 10 rouges ; 53 tests de `banc`), 0, 0 ; `clippy-2` code 101 (`Troupe::tourner` trop long, d'où `noter_le_tour`) |
| `changer-2` | bf74 | comme `changer` | **code 0**, les deux réussis |
| `soiree-punchline-3` | bf74 | comme `soiree-punchline` | **code 1** : Chrome réussi ; **Firefox en échec sur les débordements** (un bouton de vote et ses deux colonnes à 768 × 1024, vus par les pages Chrome et Firefox) ; tout le reste tient, sauts compris ; corrigé (`c975`) |
| `soiree-71-2` | ~~bf74~~ 74de (*corrigé après la vérification* : son empreinte dit `74de73c4`, avec `noter_le_tour`) | comme `soiree-71` | **code 0 : Chrome et Firefox réussis** ; le plus long silence des natifs 415 et 368 ms ; M13 relevée à 71 appareils ; retardataires sans retard ; 661 s ; 3,34 au plus |
| `fmt-3`, `fmt-web-3`, `clippy-3`, `clippy-web-3` | c975 | `final-3.sh` : le commun, puis `web/` natif et `wasm32` | code 0, 0, 0, 0 |
| `tests-banc-3`, `tests-web-3` | c975 | `lourd cargo test -p banc --no-fail-fast` ; dans `web/`, `cargo test --workspace --no-fail-fast` | code 0 : 53 et 68 réussis, 0 échec (`salle`, inchangée depuis `tests`, n'est pas relancée) |
| `construire-3` | c975 | `cargo build -p banc … -p relais … && target/debug/soiree-relais preparer` | code 0 ; `soiree-banc` `65186a0c…`, **paquet `4e85aa48…`** (la coupe des mots dans les boutons) |
| `changer-3` | c975 | comme `changer` | **code 0 : Chrome et Firefox réussis** ; 143 s ; 2,84 au plus |
| `soiree-punchline-4` | c975 | comme `soiree-punchline` | **code 1** : Firefox réussi (aucun débordement) ; **Chrome en échec sur la recherche des secrets** : « Zoé », un texte tapé de 4 octets, trouvé **une fois précédé de sa longueur** (5 octets, `04 5a 6f c3 a9`) dans un enregistrement de 46,3 Mo. Cherché : il tombe à l'octet 191 861 d'une note de 1 374 455 octets du lien 17, la garde, une case de point scellée par G (`Ecrire`), au milieu d'octets chiffrés ; le hasard y met une suite donnée de 5 octets une fois sur 24 000 environ (46,3·10⁶ / 2⁴⁰), et le banc en cherche, par passage, une quarantaine sous cette forme courte (de l'ordre d'une fois sur 600 passages). Noté tel quel, jugé comme un hasard des octets chiffrés, et **le contrôle n'est pas changé** ; tout le reste du passage tient (sauts, rechargement, jumeau, file pleine, débordements) |
| `soiree-punchline-5` | c975 | la même, relancée | **code 0 : Chrome et Firefox réussis** ; 28 secrets absents et le témoin vu dans chacun ; 298 s ; 3,84 au plus |

Non relancés, et pourquoi : le plan standard (rien de ce que l'épreuve joue ou juge ne change : le plan court le prouve, contrôle pour contrôle) ; `dessin` et E20 (ni `web/rendu`, ni la toile, ni le moteur, ni la réécriture ne changent ; l'écran de démarrage gagne des boutons sans nom, que `chaque_texte_tient_au_pire_cas` et `aide` couvrent) ; `arriere-plan` (le Worker ne change que par `changer_de_jeu`, hors de son chemin ; rejoué à la correction de la fusion) ; `ecran tier-list` (`web/ecran` change par l'écran de démarrage et F9, joués par `changer` et `aide` ; le clic du geste, la page du jeu et le clavier de Tier list ne changent pas) ; les contrôles par mutation des lots précédents (ceux que ce lot touche sont dans les siens) ; `soiree-sidequest` sur l'arbre final (le fil des natifs, joué par lui, n'a changé depuis que par le rythme des liaisons, et la page du téléphone que par la coupe d'un mot dans un bouton, jouée par `changer-3` et `soiree-punchline-4`).

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 12, fin » : **M4 de Punchline** au calme (`soiree-punchline-2`, 2,77 au plus) : 141,4 et 144,0 ms dans Chrome, 210,0 et 199,0 ms dans Firefox, pour 941 989 octets. Le reste est **relevé, pas mesuré au calme** (chaque preuve a dépassé une charge de 3) :
- **M13 à 71 appareils, sur tout le Worker** (`soiree-71-2`) : la part du commun, médiane 0,4 et 0,6 ms, 99ᵉ centile 2,0 et 2,4 ms, maximum 2,57 et 6,14 ms (Chrome, Firefox) ; le tour entier 1,1 et 1,2 ms, 3,5 et 4,0 ms, 9,63 et 12,56 ms ; **71 appareils accueillis au dernier rapport**, et non plus 2 (la réserve de la correction du lot 12 est levée : le Worker qui fait foi a compté ses 71 appareils jusqu'au bout). Sous les 4,17 ms au 99ᵉ centile.
- **(4) Le retard d'écho des natifs dans Firefox : c'était le banc.** Avec les natifs sur leur propre fil, leurs allers-retours d'écho ne passent plus par le fil du banc ; sur les quatre arrivées de `soiree-71-2`, la médiane pendant la livraison du module reste celle d'avant (3,9 → 3,6 ms, 3,5 → 3,4 ms, 3,8 → 4,1 ms, 3,6 → 3,7 ms ; 99ᵉ centile pendant au plus 32 ms, sous celui des 20 s d'avant plus un pas), et de même dans `soiree-71` (la page Firefox : 3,8 → 3,2 ms et 3,5 → 3,3 ms). Au lot 12, la même arrivée valait 68,3 ms pour les 67 échos à la fois ; et la médiane d'avant, 10,6 à 11,6 ms, tombe à 3,5 à 3,9 ms : les 7 à 8 ms de différence étaient l'attente des natifs entre deux tours du fil du banc (8 ms d'écoute de la console par tour). Le relais et l'autorité n'y étaient pour rien qu'on puisse voir : la livraison d'un module de 738 744 octets à une page ne retarde plus les échos des 69 autres. Ce n'est pas mesuré au calme (3,34 et 3,87 au plus).
- **Les sauts** (question 30) : aucun saut d'un pas inexpliqué dans les seize passages `soiree` des preuves (Punchline avec pages, cinq lancements ; Sidequest cinq minutes ; les 71 appareils, deux lancements) : chaque passage a ses quatre sauts, tous rattachés à ses reprises, sous des charges de 2,77 à 3,87 au plus. Au lot 12 et à la fusion, un saut inexpliqué venait un passage sur deux à quatre : seize passages sans aucun ne tranchent pas la question 30, et le contrôle comme le rattrapage des jeux restent tels quels ; les chiffres sont ajoutés à la question.

### Ce qui n'est pas fait, et pourquoi

- **(3) Le scénario `pages` sur les pages de pire cas, avec un fautif de module** : non fait. Il faudrait construire, pour chaque jeu, les pages de téléphone de pire cas depuis les limites déclarées (aucune n'existe : E16 ne construit que la scène de l'écran, E21 appelle `Page` sur les vues des soirées), les poser dans la page d'un navigateur à trois tailles, et un module fautif dont la page déborde. Les débordements restent jugés sur les pages vues en soirée (l'écart du lot 12, au § 8.2) ; ce jugement-là a trouvé, à cette fin de lot, un vrai débordement du commun (un bouton d'un seul long mot), corrigé.
- **La fenêtre du changement de jeu** : un Worker tué entre l'envoi du premier point du nouveau jeu et l'écriture de l'enregistrement reprendrait ce point avec l'ancien module et s'arrêterait (`Arret::Reprise`). La fermer demande que la garde nomme son module (le registre ou le point, `VERSION_CASES` à monter, octets figés à réécrire) : non fait, dit au § 2.8. Aucun scénario ne tue le Worker pendant un changement.
- **Un accusé de morceau d'avant le changement** (`ModuleRecu` ne nomme pas son module) arrivé juste après pourrait être pris pour un morceau du nouveau module ; la fenêtre est celle des deux morceaux en vol au moment du changement. Ni testé ni corrigé ; aucun scénario ne change de jeu pendant une livraison.
- **Changer de jeu pendant un changement**, ou vers un module que le catalogue n'a pas : le Worker refuse le second (un seul à la fois) et rapporte le refus d'un module qui ne se charge pas (`change-refuse`), la partie d'avant continuant ; ni l'un ni l'autre n'est joué par un scénario.
- **La garde des présences d'un jeu sans téléphone** (`arriver`) : gardée par précaution, sans preuve qu'elle soit nécessaire (sa mutation ne rend rien rouge : ardoise accepte une présence) ; celle des regards et des bots est tenue.
- **`Locaux::retenu`** : gardé dans `localStorage`, vu retrouvé par le scénario `soiree` sur Sidequest ; aucun test natif (le stockage est celui du navigateur).
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché) ; l'essai de sa session ne tait plus les natifs.
- Restent du lot 12 : ni son, ni motifs de vibration, ni verrou d'écran sur le téléphone. Le lot 12 n'a plus, à cette fin, que ces trois points, les pages de pire cas et Safari hors de sa colonne « ce qui le prouve fini ».

### Ce que le lot suivant doit savoir

- `soiree-banc changer <module>…` joue un changement de jeu par l'écran de démarrage (F9, la ligne, « Jouer ») ; `__banc.centre(<empreinte>)` donne la ligne d'un jeu. Le Worker rapporte `change` (la forme de la partie neuve) et `change-refuse` ; la page, `choisi` et `changer`.
- `Autorite::changer_de_jeu` ouvre la partie neuve au pas qui suit, écrit son point aussitôt, garde liens et registre ; `simulation::Soiree::changer_de_jeu` la joue en mémoire (`une_salle_change_de_jeu_sans_rien_relancer`). `verifier_l_historique` accepte un point d'une autre ouverture à l'indice 0 ; **`reprendre_de_l_historique` (la soirée entière rejouée depuis sa première case) ne sait pas traverser un changement de jeu** : le scénario `soiree` ne change pas de jeu, et `changer` ne juge que la garde finale.
- Les téléphones natifs du banc tournent sur leur propre fil (`Session::natifs`, `troupe()` le temps d'y lire) ; un silence de plus de 1 s est une faute ; le fil relie au plus quatre téléphones par tour. Leurs allers-retours d'écho mesurent désormais la salle, sans le banc (3,5 à 4 ms de médiane à 71 appareils).
- Le singe attend le salon au complet et n'appuie sur un bouton qu'une fois sur cinq ; ses tours se comptent (`tours_du_singe`). Un jeu dont une page n'offre qu'un bouton d'annulation tient désormais.
- `soiree_en_memoire --par-la-page --singe-attend` reproduit en quelques secondes ce que le singe fait d'un jeu, avant un scénario de navigateur.

## Lot 12, fin, correction après la vérification

Faite le 2026-11-08, de 02 h 40 à 04 h 10 environ (UTC), dans `soiree/` (branche `refonte`, `HEAD` `cda28f3`), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Tout cargo, scénario et contrôle par mutation est passé par le portillon (`target/hote/lourd`), sauf des `cargo check -p` de quelques secondes ; un scénario à la fois ; aucun navigateur ni serveur ne reste ouvert (chacun fermé par le banc). Rien n'est commité. **Seul le commun change** ; les trois jeux restent propres, leurs modules aux mêmes sha256 (`82669f77…`, `62043526…`, `84315a1b…`). Fichiers de travail : `target/travail/lot-12-fin-corr/` ; preuves : `target/preuves/lot-12-fin-corr/<nom>/`.

### Les constats, et ce qui en est fait

1. **[bloquant] Le scénario `pages` sur les pages de pire cas, avec un fautif de module : fait.** `banc::pages`, `soiree-banc pages <module>… --negatif <module>`. En natif, chaque module dans wasmtime (`PHASE_3`) : chaque pire cas construit sur une instance nue depuis les limites (`PireCas`, comme E16 et `dessin`), la vue de téléphone de chaque appareil qu'il connaît (`Regarder` de `n × 0x9e3779b97f4a7c15`, `n` de 1 au plus d'appareils, comme `Construction::appareils`), les vues identiques posées une fois, les ressources de la page (`Ressources` de chaque asset déclaré, comme la livraison). Dans un vrai navigateur : la page du téléphone servie par le relais en mode banc, sans salle ni fragment ; le banc lui passe module, ressources et vues par deux crochets nouveaux (`window.__banc.morceau` par morceaux de 128 Kio, `window.__banc.pire_cas`, mode banc seulement), et la page en fait ce qu'elle fait d'un module reçu par la connexion (`moteur_navigateur::charger`, `InstanceDePage`, `PageDuTelephone::vue_recue`, le DOM par clé, `FontFace`) ; l'écran du commun ne la couvre pas. À 360 × 640, 390 × 844 et 768 × 1024 : chaque élément qui déborde (`scrollWidth` contre `clientWidth`, ou hors de la page), et la page elle-même, est une faute qui nomme jeu, pire cas, appareil, taille, sorte et clé ; une page en faute aussi ; un jeu dont aucune page posée n'a de nœud aussi (rien n'aurait été jugé). Le contrôle négatif : **un fautif de module**, `fautif-page-qui-deborde-module` (`fautifs::jeux::PageQuiDeborde`, nouveau : à 9 999 points, donc seulement en pire cas, la page écrit le prénom du record, douze « W », à 512 px, la plus grande taille que `PX_PAGE` permet ; une lettre est plus large qu'un téléphone de 360 ou 390 px, et `overflow-wrap: anywhere` n'y peut rien) : joué dans le même passage, par le même chemin, il doit déborder, sinon « CONTRÔLE NÉGATIF RATÉ » ; seul, il fait échouer le scénario (code 1). L'épreuve ne le voit pas (E16 ne mesure que la scène de l'écran ; E21 n'appelle `Page` que sur les vues des soirées, jamais à 9 999 points) ; il n'est pas dans ses listes (`epreuve/tests/epreuve/modules.rs`). Constaté (`pages-2`) : Punchline, 10 pires cas, 61 vues distinctes, 183 pages posées et jugées (de 5 à 17 nœuds) ; Sidequest, 6 pires cas, 355 vues distinctes (71 appareils), 1 065 pages (17 nœuds), 55 ressources ; Tier list sans téléphone, « sans objet » ; **aucun débordement**, dans Chrome comme dans Firefox ; le fautif déborde à 360 et 390 px (texte `1f88300eb24c0d48`, la clé `record` ; sa colonne ; la page : 481 px contre 360), pas à 768 px. Ce qui n'y est pas joué, et le reste : le transport, et les locaux d'un joueur (un champ rempli, un choix fait) ; le jugement des pages vues en soirée (`soiree --pages`) les couvre.
2. **[important] M13 : mesurée au calme.** La cause des relevés trop chargés : trois navigateurs à côté de 71 téléphones. Lancée avec 71 téléphones natifs et aucune page de navigateur (`soiree <sidequest> --telephones 71`), sous `lourd --calme`, la règle de validité écrite avant les tours (`final-3.sh` : un tour à 3 ou plus est écarté et refait, au plus trois) : `m13-1` (2,64 au plus) et `m13-long` (`--minutes 3`, 2,61 au plus), aucun écarté. **Au 99ᵉ centile, 2,0 et 2,9 ms (`m13-1`), 2,0 et 2,4 ms (`m13-long`), Chrome et Firefox, sous les 4,17 ms** ; 71 appareils accueillis au dernier rapport (`MESURES-PHASE-3.md`, « Lot 12, fin, correction »).
3. **[important] `soiree-sidequest --minutes 5` sur l'arbre final : réussi** dans les deux navigateurs (`soiree-sidequest`, marques posées 3 et rejouées 3 chacun). La cause de l'échec que la vérification a vu sur `c975`, relue dans ses lignes (`target/travail/verif-lot-12/soiree-sidequest/chrome/lignes.txt`) : à partir de 297 s, chaque tour du banc a lu ses lignes avec 20 s de retard (huit trous de 20,0 à 20,1 s, autant que de « CDP Input.dispatchTouchEvent : pas de réponse en 20s » du pilote de la page Chrome) ; les trois marques F8, attendues à 1 s l'une de l'autre, se sont posées à 20 s l'une de l'autre (pas 20 240, 21 445, 22 650), la partie figée encore 32 s plus tard (dernier point au pas 24 600) : la garde, qui ne tient que deux points, n'avait plus celui d'avant la première marque. **Corrigé dans le banc** : les pilotes des pages s'arrêtent avant les marques (`soiree.rs`, « Les marques F8 ») ; rien de ce que `rejouer-garde` juge ne change. *Ce qui n'est pas séparé* : pourquoi Chrome sans interface n'accusait plus les touchers de cette page-là (`arriere_plans` à 1 : un `blur` ou un `visibilitychange` l'avait mise en arrière-plan, sans cause trouvée) ; dans les passages de cette correction, le pilote n'a buté sur aucun toucher (aucune ligne « le pilote a buté »). La règle « on ne rejoue pas, rien n'a changé que le rythme des liaisons » était fausse en pratique : le rythme du fil du banc compte pour les marques ; elle n'est plus suivie ici.
4. **[important] Le retenu passe par `localStorage`, et c'est prouvé.** La page dit ce que `localStorage` lui a rendu quand elle s'est faite sur son module, avant toute vue (`retenu_repris`, et la clé `retenu_cle`, dans l'état du banc). Le scénario, avant le rechargement en pleine écriture, lit la clé dans `localStorage` (elle doit valoir le retenu de la page) ; après, il exige `retenu_repris` égal au retenu d'avant (le seul retenu retrouvé ne suffisait pas : la manette de Sidequest le recalcule depuis la vue). **Contrôle négatif**, dans le même passage : la clé ôtée, la page rechargée une seconde fois ne doit rien reprendre (sinon « CONTRÔLE NÉGATIF RATÉ ») ; constaté dans `soiree-sidequest` et `soiree-71`, avec la note que la vue redonne pourtant le même retenu. Chez Punchline, rien n'est retenu : le constat est dit « sans objet pour ce jeu ». **Tests natifs** : `web/telephone/src/retenu.rs` (la clé par salle et par jeu, quand écrire : un retenu absent n'efface rien), `la_cle_nomme_la_salle_et_le_jeu`, `un_retenu_s_ecrit_quand_il_change_et_jamais_ne_s_efface`, chacun rouge par mutation.
5. **[mineur] « 11 mutations sur 11 »** : faux, c'étaient 10 sur 10 ; corrigé dans la section « Lot 12, fin » (barré, le bon nombre à côté).
6. **[mineur] L'index des preuves** : `aide` était sur l'arbre `bf74`, `soiree-71-2` sur `74de` ; corrigé dans la section « Lot 12, fin ».
7. **[mineur] Le jugement de `changer`** : un natif relié de nouveau au relais est une faute (c'était une note) ; les gestes d'un natif ne comptent pour le nouveau jeu que depuis qu'il a refait sa page sur le nouveau module (`noter_les_pages_neuves`), et **chaque** natif doit rejouer par elle (§ 8.2). Le contrôle négatif rejoué dit maintenant « 0 téléphones natifs sur 6 » (il disait 4), et échoue toujours sur le module non reçu.
8. **[mineur] Les mutations des natifs rejouées sur le `soiree.rs` final** (`49c70069…`, après `noter_le_tour` et cette correction) : rouges (`controles`).
9. **[mineur] Le commentaire de doc d'`appliquer`** est revenu au-dessus d'`appliquer` ; `garder_le_retenu` n'a plus que le sien.
10. **[mineur] Les fenêtres du changement de jeu** : restent ouvertes, et dites (plus bas). Écarté de cette correction : les fermer demande de monter `VERSION_CASES` ou `salle::fil::PROTOCOLE`, ce que ni la ligne du lot ni la vérification ne demandent.

### Les écarts à PHASE-3, corrigés dans PHASE-3

- **§ 8.2, ligne `pages`** : récrite, ce qui est fait (le scénario, ses pages de pire cas, son fautif de module), ce qui n'y est pas joué.
- **§ 8.2, ligne `soiree`** : les pilotes arrêtés avant les marques, le retenu repris du stockage et son contrôle négatif.
- **§ 8.2, ligne `changer`** : le jugement plus strict.
- **§ 7, ligne du lot 12**, et **§ 9.1, question 30** (un passage sur douze avec deux sauts inexpliqués, au tour qui suit un réveil de la minuterie en retard ; le passage relancé réussi ; rien de changé).

### Ce que les jeux voient

Rien : ni l'API, ni l'ABI, ni `salle::fil::PROTOCOLE`, ni `salle::ecran::PROTOCOLE`, ni `VERSION_CASES`, ni ce que l'épreuve joue ou juge (le binaire `soiree-epreuve` reconstruit est le même octet pour octet, `1edcdde5…`, `construire`). Le scénario `pages` est vert pour les trois jeux tels qu'ils sont : le lot 7b n'a pas de débordement de page de pire cas à corriger.

### Les tests qui la tiennent

- `web/telephone/src/retenu.rs` : `la_cle_nomme_la_salle_et_le_jeu`, `un_retenu_s_ecrit_quand_il_change_et_jamais_ne_s_efface`.
- **Les contrôles par mutation** (`target/travail/lot-12-fin-corr/controles.py`, même critère strict qu'à la fin du lot 12) : **13 sur 13 rouges**, les dix de la fin du lot 12 rejoués sur l'arbre final (dont `natifs-sans-fil` et `silence-jamais-une-faute` sur le `soiree.rs` final) et trois nouveaux (`retenu-qui-s-efface`, `retenu-recrit-a-chaque-fois`, `cle-sans-jeu`).
- **Les scénarios et leurs contrôles négatifs** : `pages` et son fautif de module ; le retenu repris et sa clé ôtée (dans `soiree`) ; `changer` et `changer-sans-livrer`.

### Les vérifications, et l'index des preuves

Un seul arbre de code : `git diff HEAD` hors `.md` du commun **`54fc4b33…`** (`HEAD` `cda28f3` ; non suivis : `crates/banc/src/changer.rs`, `crates/banc/src/pages.rs`, `crates/exemples/fautifs/modules/page-qui-deborde/`, `web/telephone/src/retenu.rs`) ; le paquet `c8c83348…` ; le fautif `7b520932…` ; `soiree-banc` `d16ba028…` à `construire`, puis `ff48ae01…` reconstruit par `cargo test --workspace` (mêmes sources : la compilation des tests unifie d'autres fonctionnalités) et utilisé par toutes les preuves de scénario à partir de `pages-2`. Ce journal, PHASE-3, MESURES, CHANGELOG, `crates/README.md` et `AGENTS.md` ont été écrits pendant les preuves (le `diff` entier change, pas celui hors `.md`). Chaque dossier : `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `preuve.sh`, `empreinte.sh`, `final.sh`, `final-2.sh`, `final-3.sh`, `controles.py`.

| Preuve | Commande | Résultat |
|---|---|---|
| `construire` | `lourd sh -c "shasum soiree-epreuve && cargo build --release -p epreuve && shasum … && cargo build -p banc -p relais && soiree-relais preparer && cargo build -p fautif-page-qui-deborde-module --target wasm32-unknown-unknown --profile module && shasum …"` | code 0 ; `soiree-epreuve` `1edcdde5…` avant et après (inchangé) |
| `controles` | `lourd python3 -I target/travail/lot-12-fin-corr/controles.py` | code 0 : **13 sur 13 rouges**, sources remises |
| `fmt`, `fmt-web` | `cargo fmt --all --check` | code 0, 0 |
| `clippy`, `clippy-wasm32`, `clippy-web`, `clippy-web-wasm32` | comme à la fin du lot 12 | code 0, 0, 0, 0 |
| `tests-web` | `lourd sh -c "cd web && cargo test --workspace --no-fail-fast"` | code 0 : **70 réussis** (68 + les deux de `retenu`), 0 échec |
| `tests` | `lourd cargo test --workspace --no-fail-fast` | code 0 : **735 réussis**, 0 échec, 1 ignoré (`plan_standard`, déjà ignoré) ; 757 s ; la charge a atteint 11,63 (d'autres travaux) |
| `pages`, `pages-negatif` | les mêmes que `pages-2`, `pages-negatif-2` | code 0, code 1 ; **leur empreinte de fin n'est pas valable** : ils ont tourné pendant `controles`, qui mutait les sources (le binaire, construit avant, n'en dépend pas) ; refaits |
| `pages-2` | `lourd soiree-banc pages <punchline> <sidequest> <tier-list> --negatif <fautif> --navigateurs chrome,firefox,safari --sans-construire` | **code 0** : Chrome et Firefox réussis, aucun débordement, le contrôle négatif vu (plus haut) ; Safari pas exécuté ; 14 s |
| `pages-negatif-2` | `lourd soiree-banc pages <fautif> --navigateurs chrome,firefox` | **code 1** pour sa raison : six débordements dans chacun, à 360 et 390 px |
| `changer` | comme à la fin du lot 12 | **code 0** : Chrome et Firefox réussis ; 6 natifs sur 6 rejouent par la page neuve ; 142 s ; 2,78 au plus |
| `changer-negatif` | la même, `--fautif changer-sans-livrer` | **code 1** pour sa raison (natifs restés sur `46ebf4b5…`), « 0 sur 6 » |
| `soiree-sidequest` | `lourd --calme … soiree <sidequest> --navigateurs chrome,firefox --pages chrome,firefox,safari --minutes 5` | **code 0** : les deux réussis ; 300 s de partie ; marques 3/3 rejouées ; retenu repris et contrôle négatif vus ; file pleine, jumeau, coupure, secrets, débordements ; quatre sauts rattachés ; aucun silence des natifs ; 672 s ; 3,33 au plus |
| `soiree-punchline` | `lourd --calme … soiree <punchline> … --pages chrome,firefox,safari --telephones 5` | **code 1** : Chrome réussi ; **Firefox en échec sur deux sauts d'un pas inexpliqués**, (4817, 4818) et (6019, 6020), chacun au tour qui suit un réveil de la minuterie en retard (32,6 et 34,0 ms), bien avant les marques (question 30) ; tout le reste tient ; 308 s ; 3,26 au plus |
| `soiree-punchline-2` | la même, relancée | **code 0** : les deux réussis (podium puis « On rejoue », rechargement, jumeau, file pleine, 28 secrets absents, quatre sauts rattachés) ; 298 s ; 4,69 au plus |
| `soiree-71` | `lourd --calme … soiree <sidequest> … --telephones 69 --retardataires` | **code 0** : les deux réussis ; 71 appareils accueillis ; retardataires sans retard (médiane d'écho pendant la livraison 3,2 à 3,8 ms, sous celle d'avant) ; retenu repris ; le plus long silence des natifs 454 et 843 ms ; 663 s ; 3,18 au plus |
| `m13-1`, `m13-long` | `lourd --calme … soiree <sidequest> --telephones 71 [--minutes 3]` | **code 0** et code 0 ; **M13 au calme** (2,64 et 2,61 au plus) |

Non relancés, et pourquoi : le plan court et le plan standard (`soiree-epreuve` est le même binaire, les modules les mêmes : rien de ce que l'épreuve joue ne change) ; `dessin`, E20, `arriere-plan`, `ecran tier-list`, `aide` (ni `web/ecran`, ni `web/autorite`, ni le rendu ne changent ; la page du téléphone ne change que par ses crochets du banc et le retenu, joués par `pages` et `soiree`).

### Les mesures

M13, au calme : `MESURES-PHASE-3.md`, « Lot 12, fin, correction ». Le reste (M4, M7, les échos) est relevé dans les preuves au-dessus d'une charge de 3, pas revendiqué.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché ; `pages` n'évalue rien dans Safari).
- **Les fenêtres du changement de jeu**, ouvertes : un Worker tué entre l'envoi du point neuf et l'écriture d'IndexedDB ; un `ModuleRecu` d'avant compté pour le nouveau module ; changer pendant un changement. Aucun scénario ne les joue ; les fermer touche `VERSION_CASES` ou `salle::fil::PROTOCOLE`.
- **`reprendre_de_l_historique`** ne traverse pas un changement de jeu : `changer` ne juge que la garde finale.
- **La garde des présences d'un jeu sans téléphone** : gardée par précaution, sans preuve qu'elle serve.
- **Pourquoi Chrome sans interface cessait d'accuser les touchers d'une page** (constat 3) : non séparé.
- **Le saut d'un pas après un réveil en retard** (question 30) : revu une fois sur douze passages, ouvert.
- `pages` ne joue ni les locaux d'un joueur ni le transport ; ses pages viennent de wasmtime, pas d'une autorité dans le Worker.
- Restent du lot 12 : ni son, ni motifs de vibration, ni verrou d'écran sur le téléphone.

### Ce que le lot suivant doit savoir

- `soiree-banc pages <module>… --negatif target/wasm32-unknown-unknown/module/fautif_page_qui_deborde_module.wasm` dure quelques secondes ; c'est la preuve `pages` du lot 7b pour chaque jeu. Le fautif se construit par `cargo build -p fautif-page-qui-deborde-module --target wasm32-unknown-unknown --profile module`.
- La page du téléphone a deux crochets de plus en mode banc (`morceau`, `pire_cas`) et deux champs de plus dans son état (`retenu_repris`, `retenu_cle`).
- Une mesure à 71 appareils se prend sans page de navigateur (`--telephones 71`), sinon la charge dépasse 3.
- `cargo test --workspace` reconstruit `target/debug/soiree-banc` (autres fonctionnalités) : relever son sha256 après les tests, pas seulement à `construire`. Les contrôles par mutation changent les sources : rien d'autre ne tourne à côté d'eux.

## Lot 7b — Les jeux corrigent leurs pages et leurs textes

Chaque jeu, par son agent, dans son dépôt, en même temps (PHASE-3, § 7, ligne du lot 7b) ; chacun écrit sa part ci-dessous.

### Tier list

Fait le 2026-11-08, de 04 h 05 à 04 h 50 environ (UTC), par l'agent du jeu, dans `games/tier-list` seulement (branche `refonte`, `HEAD` `9827f39`), à côté des agents de Punchline et de Sidequest (le portillon partagé), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Le commun n'est pas modifié (`bc56fec`, arbre propre à chaque preuve), sauf ce journal. Chaque cargo, chaque épreuve, chaque navigateur est passé par le portillon (`target/hote/lourd`), sauf des `cargo fmt` et `cargo update --offline`. Chaque navigateur et chaque relais lancés sont fermés. Rien n'est commité. Fichiers de travail : `target/travail/lot-7b/tier-list/` ; preuves : `target/preuves/lot-7b/tier-list/`.

#### Ce qui est fait

- **Le débordement que E16 a trouvé au lot 8** : la marque « TIER LIST » (28 px, `Lignes::Une`) dans une boîte de 28 px, une ligne de 33,6. Sa boîte fait maintenant 34 px, centrée dans la barre de 64 (`vue::ligne_du_commun(28)`, `vue::INTERLIGNE_DU_COMMUN` : 12 dixièmes, l'interligne de `mise-en-page`). E16 réussi : 1 151 textes mesurés, aucun débordement.
- **Ce que la page montrait dans un navigateur, et que E16 ne voit pas** (la page servie par le relais du banc, vue dans Chrome et dans le navigateur de l'application) :
  1. **Le titre rogné.** Un `EnPlace` de 40 px, que la page pose en `line-height: 1.2` sous `overflow: hidden`, dans 45 px par ligne (le 40/45 du cahier) : chaque ligne perdait 3 px. Le titre a maintenant 48 px par ligne (`vue::LIGNE_DU_TITRE`). Constaté : un titre de 500 « W » tient en 18 lignes de 48 px, la boîte exacte que le jeu lui donne (`scrollHeight` 864 = `clientHeight` 864).
  2. **« Drop here to remove » invisible** : un nom rouge sur la poubelle rouge pleine. La poubelle est un bord rouge de 2 px sur le fond de la page, comme `#trash` dans `contenu/reference/style.css` (un nœud de plus).
  3. **L'étiquette invisible dans les réglages** : le commun écrit un `Champ` en blanc, en `mix-blend-mode: difference`, et ce mélange ne se fait pas avec la toile (Chrome, WebGPU comme WebGL2 forcé ; de même dans le navigateur de l'application) ; sur le fond blanc du cahier, rien ne se lisait. Le fond de la zone de texte est sombre (`#111`, celui du champ de nom) : l'étiquette se lit. Écart au cahier, dit dans `BESOINS-COMMUN.md`, § 20.
- **Ce qui retourne au commun, sans contournement** (`games/tier-list/BESOINS-COMMUN.md`) : § 19, l'interligne (un `StyleTexte` sans interligne : le 40/45 et le 28/1 du cahier restent hors d'atteinte) ; § 20, **l'allure d'un bouton** : le fond opaque et le nom que le commun dessine (`mise_en_page::bouton`) cachent la roue et les flèches de chaque rang, la croix, **les quinze pastilles des réglages** (on ne voit plus les couleurs qu'on choisit) et les boutons bleus du cahier ; le jeu pourrait dessiner son allure par-dessus (un clic vise le dernier bouton qui le contient, quoi qu'on dessine au-dessus), il ne le fait pas : ce serait contourner l'allure du commun, « à revoir à la répétition » (PHASE-3, annexe) ; et la couleur du texte d'un champ ; § 21, une coupe « par mots, puis par graphèmes dans un mot trop large » (l'aperçu d'une tuile coupe « d'u / ne »). Vu aussi : dans une fenêtre de moins de 1 120 px, la colonne des réglages des rangs sort à droite ; la barre du haut n'a pas toute la largeur de la fenêtre.
- **Le scénario `pages`** : Tier list n'a pas de téléphone ; `pages` le dit « sans objet », et vert avec son contrôle négatif (rien à corriger, comme l'avait dit la fin du lot 12).
- **`Cargo.lock`** mis à jour : les 27 paquets de `mise-en-page` (`cosmic-text` 0.19.0 et ce qu'elle tire), que l'épreuve depuis les sources y écrivait depuis le lot 8. Chaque version est celle du `Cargo.lock` du commun (comparé paquet par paquet : 0 écart) : `unicode-script` tenue à 0.5.8 et `yazi` à 0.2.1 (`cargo update --offline --precise`), leurs versions suivantes ayant moins de deux semaines (0.5.9 publiée le 2026-11-02, 0.2.2 le 2026-11-01, lu dans le registre en cache). L'épreuve depuis les sources ne réécrit plus le fichier (empreintes de début et de fin de `standard` identiques).

#### Les tests qui le tiennent

- `tests/partie.rs`, `les_lignes_tiennent_a_l_interligne_du_commun` (TL-55, à l'écran) : la part « hauteur » de E16 refaite en natif, et étendue aux textes en place (titre, étiquettes), sur les quatre pires cas et cinq écrans ordinaires.
- `tests/partie.rs`, `aucun_texte_n_a_la_couleur_de_son_fond` : aucun texte de la couleur du rectangle sur lequel il se pose, sur chaque pire cas et le plateau ordinaire.
- `tests/partie.rs`, `chaque_composant_qui_se_dessine_a_sa_boite` : 48 px pour une ligne de titre (au lieu de 45).
- **Contrôles par mutation** (`controles.py`, chaque correction défaite le temps d'un essai) : **3 sur 3 rouges** (la marque à 28, le titre à 45, la poubelle pleine), sources remises.
- Le fond sombre du champ des réglages n'a pas de test natif (ce serait tester une constante) : sa preuve est la capture `3-reglages.png`.

#### Les vérifications, et l'index des preuves

Un seul arbre de code pour toutes : Tier list `HEAD` `9827f39`, `git diff HEAD` hors `.md` **`49c033aa…`**, aucun fichier non suivi ; le commun `bc56fec`, aucun changement ; module `target/wasm32-unknown-unknown/module/tier_list_module.wasm` sha256 **`ae27434d…`**, BLAKE3 `1294a3e9…` (le même octet pour octet que celui que l'épreuve compile depuis les sources : même BLAKE3) ; `soiree-epreuve` `1edcdde5…`. Chaque dossier : `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt`, `empreinte-fin.txt` (par `empreinte.sh`, copié avec `preuve.sh`, `final-a.sh`, `final-b.sh`, `capture.sh`, `capture.mjs` et `controles.py`).

| Preuve | Commande | Résultat |
|---|---|---|
| `standard` | `lourd target/release/soiree-epreuve ../games/tier-list --json …/standard.json` | **code 0** ; E1 à E19 réussis ou sans objet (E2, E10, E11 sans objet), E20 pas exécuté (hors du plan), E21 sans objet (pas de téléphone) ; E16 : 1 151 textes, 0 débordement ; 206 s, charge 4,82 au plus |
| `court` | `lourd target/release/soiree-epreuve --wasm <module> --court --json …` | code 3 (module seul : l'étape 1 n'est pas faite) ; **chaque contrôle réussi ou sans objet** : E16 réussi, comme la référence du lot 7 (Tier list 0 depuis ses sources) ; plus de E16 en échec (référence du lot 8) |
| `e16` | `lourd target/release/soiree-epreuve --wasm <module> --controles E16 --json …` | code 3 (plan réduit, module seul) ; E16 réussi, 1 163 évaluations |
| `pages` | `lourd target/debug/soiree-banc pages <module> --negatif target/wasm32-unknown-unknown/module/fautif_page_qui_deborde_module.wasm --navigateurs chrome,firefox --sans-construire` | **code 0** : Chrome et Firefox réussis ; Tier list « sans téléphone, sans objet » ; le contrôle négatif voit ses six débordements |
| `capture` | `lourd capture.sh <module> …/captures` | **code 0** : la page vue dans Chrome 156 (`soiree-relais --banc --sans-reseau`, le paquet `c8c83348…`, `?bots=non`, « Jouer » cliqué où l'écran du commun le propose) : `captures/1-plateau-neuf.png`, `2-titre-a-sa-limite.png` et `2b-…-entier.png` (500 « W », aucun texte en place rogné), `3-reglages.png`, `4-tuiles-et-actions.png` (et `4b-ajout-actions-poubelle.png`, recadrée), `5-export.png` ; aucune erreur de la page ; `journal.txt` |
| `tests` | `lourd cargo test --no-fail-fast` | code 0 : 100 réussis, 0 échec |
| `clippy`, `clippy-wasm32` | `lourd cargo clippy --all-targets -- -D warnings` ; `… -p tier-list -p tier-list-module --target wasm32-unknown-unknown -- -D warnings` | code 0, 0 |
| `fmt` | `cargo fmt --all -- --check` | code 0 |
| `module` | `lourd sh -c "cargo build -p tier-list-module --target wasm32-unknown-unknown --profile module && shasum -a 256 …"` | code 0, `ae27434d…` |
| `controles` | `python3 -I controles.py` | code 0 : 3 sur 3 rouges, sources remises (empreintes de début et de fin identiques) |

Un premier passage de `standard` et des tests a été arrêté à la main (04 h 35) : clippy refusait l'arithmétique des deux nouveaux tests (`arithmetic_side_effects`, `map_unwrap_or`, `panic`) ; corrigés, tout a été relancé sur l'arbre final.

**Le banc ne fait pas de capture d'écran** (aucun scénario n'écrit d'image de la page) : la capture vient de `capture.mjs`, qui prend la page que le relais du commun sert en mode banc, la pilote dans Chrome sans interface par CDP et l'écrit par `Page.captureScreenshot`. La page a aussi été regardée à la main dans le navigateur de l'application (1 280 × 900), avant la correction du champ des réglages. Firefox n'a pas été regardé (seulement joué par `pages`) ; Safari pas exécuté.

#### Les mesures

Aucune durée revendiquée (les durées du tableau sont celles de commandes sous une charge de 2,2 à 5,2). Du carburant, compté par la réécriture : la vue du pire cas le plus lourd 7 428 969, 8 762 369 avec le fixe, pour un budget de 10 312 500 (lot 7 : 7 426 116 et 8 759 516) ; une image, fixes compris, 4 212 839 au plus ; E15 : 336 222 octets vivants au plus au début de la soirée longue, 190 900 à la fin (comme au lot 7).

#### Ce qui n'est pas fait, et pourquoi

- **L'allure des boutons, la couleur d'un champ, l'interligne, la coupe par mots puis graphèmes** : au commun (`BESOINS-COMMUN.md`, § 19 à § 21). Tant que le bouton du commun est opaque, la fenêtre des réglages ne montre pas ses couleurs : le défaut le plus visible de Tier list sur la télé.
- **Le 40/45 du titre et le 28/1 de la marque du cahier** : demandent un interligne dans le style ; 48 et 34 en attendant (écart au cahier, dit).
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Firefox** n'a pas été regardé à l'œil : `capture.mjs` ne parle que CDP.
- `CAHIER.md` n'est pas modifié : les écarts (48 px, le fond sombre du champ) sont dits dans `BESOINS-COMMUN.md`.

#### Ce que le lot suivant doit savoir

- La référence du plan court de Tier list revient à celle du lot 7 : chaque contrôle réussi ou sans objet (code 0 depuis les sources ; code 3 par `--wasm`, faute d'étape 1).
- Le module de Tier list change : sha256 `ae27434d…`, BLAKE3 `1294a3e9…` (`84315a1b…` aux lots 7 à 12). Une épreuve ou un scénario qui le prend dans `target/modules-figes/` doit le reprendre.
- `Cargo.lock` de Tier list porte maintenant `mise-en-page` et ses dépendances, aux versions du commun ; une épreuve depuis ses sources ne le réécrit plus.
- Pour regarder la page : `target/travail/lot-7b/tier-list/capture.sh <module> <dossier>` (le relais du banc sur le port 7941, Chrome sans interface, six captures). Un outil de travail hors dépôt ; un scénario de capture dans le banc serait sa place.

### Sidequest

Fait le 2026-11-08, de 04 h 10 à 05 h 00 environ (UTC), par l'agent du jeu, dans `games/sidequest` seulement (branche `refonte`, `HEAD` `229199b`), à côté des agents de Punchline et de Tier list (le portillon partagé), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Le commun n'est pas modifié (`bc56fec`, `diff HEAD` hors `.md` vide à chaque preuve), sauf ce journal. Chaque cargo, chaque épreuve, chaque navigateur est passé par le portillon (`target/hote/lourd`), sauf des `cargo fmt`, `cargo check -p sidequest` et deux `cargo clippy -p sidequest` de quelques secondes pendant le travail. Chaque navigateur et chaque relais lancés sont fermés (par le banc et par l'outil de capture). Rien n'est commité. Fichiers de travail : `target/travail/lot-7b-sidequest/` ; preuves : `target/preuves/lot-7b/sidequest-*`.

#### Ce qui est fait

- **Les quatre débordements que E16 a trouvés au lot 8** (la question de 180 « W » d'un seul mot au tableau, pires cas 2 à 4 ; le moment de la partie qui la cite, pire cas 5) : `Coupe::Mots` laisse entier un mot plus large que sa boîte. **Chaque texte variable de la télé choisit sa coupe** (`vues::coupe`) : la question au tableau, les réponses au sol, la bulle et le moment de la partie se coupent entre les mots si, coupés ainsi, ils tiennent sûrement à la plus petite taille de leur `Taille::Budget`, sinon entre deux graphèmes. Le jeu ne mesure pas : chaque caractère compte pour la plus large chasse de la police du commun (« Ѡ », 2 740 sur 2 048, `vues::CHASSE_MAX`), une espace ne coupe qu'entre deux lettres ou chiffres (là, UAX #14 permet toujours une coupe), les lignes se remplissent au plus tard comme dans `mise-en-page`. Tout texte des deux paquets garde ses mots entiers (CAHIER § 8.1, SQ-46) ; seul un texte qu'un mot trop long ferait déborder est coupé entre graphèmes (SQ-44). La bulle, toujours coupée entre graphèmes jusqu'ici, garde ses mots quand elle le peut.
- **Les pires cas étaient trop doux, et le sont moins** (`epreuve.rs`) : ils écrivaient « W » (1 933) et « M » (1 706), quand la police a plus large. Le prénom et la bulle (encrés, tels quels) sont en « @ » (2 079, le plus large caractère encré) ; la question au tableau et les réponses au sol (en capitales) en « ﬄ », dont la capitale « FFL » fait 3 641 (un point de code y devient trois) ; le moment de la partie (tel quel) en « Ѡ » (2 740). Le tableau s'éprouve sous trois formes, une par écran qui le montre : un seul mot de 180 points de code, coupé entre graphèmes (`Question`) ; la plus longue question que la coupe laisse entière, des mots de 37 et 1 « Ѡ » sur sept lignes à la taille 14 (`Pause`) ; une échelle aux bornes les plus larges, comptes compris, dans ses bandes de 200 px (`Resultats`). La Fin a trois premiers à égalité (la plus longue annonce). Ces pires cas plus durs ont montré **deux débordements que ceux du lot 8 cachaient** : 80 « @ » ne tiennent pas en trois lignes dans une bulle, ni la plus longue borne d'une échelle en deux lignes dans sa bande. La bulle et les réponses prennent autant de lignes que leur boîte en tient (`Lignes::Libre` : 6 et 4 à la taille minimale). Le moment de la partie reste à `AuPlus(3)` : il tient (vu par E16).
- **Le signe de la pause** « Ⅱ » (U+2161, absent de la police du commun : la télé en dessinait une boîte, le seul glyphe absent que comptait E16) est dessiné en deux barres.
- **Le scénario `pages`** : rien à corriger (la fin du lot 12 l'avait dit vert) ; rejoué sur le module nouveau (le prénom des pires cas a changé) : vert.
- **La page vue dans un navigateur** : par les pièces du banc (le relais en mode banc, la page du téléphone, ses crochets `__banc.morceau` et `__banc.pire_cas`), avec l'outil de travail de Punchline copié et réduit à une capture par pire cas (`target/travail/lot-7b-sidequest/capture`) : Chrome sans interface, 390 × 844, rapport 2 ; 355 vues jugées, 0 débordement ; cinq captures (les pires cas 0, 2, 3, 4, 5 ; le 1 n'a que des vues déjà vues), dans `target/preuves/lot-7b/sidequest-capture-390x844/png/`, par exemple `vue-213-pire-cas-4-appareil-1-390x844.png` (la manette aux résultats : le prénom de 18 « @ » tient sur une ligne, « En ligne · A1B2C3D4 », « Résultats : regarde la TV », la croix, l'avatar du bouton de saut, le message). Seule la manette s'y voit : chaque appareil d'un pire cas a un personnage, le sélecteur n'est pas un pire cas (il est vu dans `soiree --pages`, lot 12).
- **Ce qui retourne au commun, sans contournement** (`games/sidequest/BESOINS-COMMUN.md`) : **B19**, une coupe « entre les mots, et un mot trop long entre graphèmes » (`overflow-wrap: anywhere`) ; en attendant, `vues::coupe` choisit l'une des deux coupes qui existent, d'après sa borne (prix : un texte coupé entre graphèmes l'est partout, ses mots ordinaires compris). Tier list demande la même (§ 21) : un besoin de deux jeux. **B20**, les 13 emoji que la police de l'original encre pour les bulles, dessinés comme des boîtes tant que la question 28 (police d'emoji) n'est pas réglée.

#### Les tests qui le tiennent

Dans `crates/sidequest/src/tests.rs` :
- `la_plus_large_chasse_est_celle_de_la_police_du_commun` (SQ-44) : `vues::CHASSE_MAX` relue dans `LiberationSans-Regular.ttf` du commun (par son chemin ; tables `head`, `hhea`, `hmtx`, `cmap` Unicode au format 4, la seule), pour tout point de code, glyphe absent compris ;
- `les_pires_cas_prennent_les_caracteres_les_plus_larges` (SQ-44) : « @ », « ﬄ » (en capitale, avec `char::to_uppercase` de Rust) et « Ѡ » sont les plus larges, chacun pour ce qu'en fait la télé, pour tout point de code ;
- `les_paquets_gardent_leurs_mots_entiers` (SQ-46, CAHIER § 8.1) : chaque question des deux paquets au tableau, chaque réponse dans sa zone avec « · 71 · +1 », coupées entre les mots ;
- `la_coupe_entre_les_mots_seulement_quand_elle_tient` (SQ-44, SQ-46) : 37 « Ѡ » oui, 38 non ; le signe isolé reste avec son mot ; sept lignes oui, huit non ; `Une`, `AuPlus`, la hauteur.

Et **E16** sur les pires cas nouveaux. Les contrôles négatifs (`target/travail/lot-7b-sidequest/mutations.py`, preuve `sidequest-controles`) : 8 mutations, chacune rouge, la source remise et vérifiée par son sha256. E16 en échec sur la coupe forcée entre les mots au tableau (pires cas 2 et 4, une ligne de 4 480 unités), dans la bulle (pire cas 1), le moment (pire cas 5) et les réponses (pire cas 4), et sur la bulle à trois lignes (« 4 lignes, pour 3 permises ») et les réponses à deux (« 3 lignes, pour 2 permises ») ; `la_coupe_entre_les_mots_seulement_quand_elle_tient` rouge sur la chasse de « W » au lieu de « Ѡ » et sur une espace devant « ? » comptée pour une coupe.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7b/sidequest-*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt`, `empreinte-fin.txt` ; scripts `target/travail/lot-7b-sidequest/preuve.sh` et `empreinte.sh`). **Toutes les preuves ont la même empreinte de code** : le commun `bc56fec`, `diff HEAD` hors `.md` vide ; Sidequest `229199b`, `diff HEAD` hors `.md` `2d05eb25159bb6aa`, aucun fichier non suivi ; le module `e80b86a9cf017d650865f67e8de1faef601850b570894b627d26b413e8b75430`, 749 827 octets (738 744 octets, `62043526…`, aux lots 7 à 12). (`BESOINS-COMMUN.md` et ce journal ont été écrits pendant et après : le `diff` entier change, pas celui hors `.md`.)

| Preuve | Commande | Résultat |
|---|---|---|
| `sidequest-fmt` | `cargo fmt --all -- --check` | code 0 |
| `sidequest-clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `sidequest-clippy-wasm32` | `lourd cargo clippy -p sidequest -p sidequest-module --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `sidequest-module` | `lourd cargo build -p sidequest-module --target wasm32-unknown-unknown --profile module` | code 0, le module ci-dessus |
| `sidequest-test-workspace` | `lourd cargo test --workspace --no-fail-fast` | code 0 : 88 tests (13 unitaires, 4 `avatars`, 3 `module`, 14 `page`, 7 `physique`, 47 `regles`) |
| `sidequest-controles` | `python3 -I mutations.py` | 8 mutations, 8 rouges |
| `sidequest-capture-390x844` | `lourd capture-pages-sidequest <module> <png> 390 844` | code 0 : 355 vues jugées dans Chrome, 0 débordement, 5 captures |
| `sidequest-pages` | `lourd /usr/bin/time -l target/debug/soiree-banc pages <module> --negatif target/wasm32-unknown-unknown/module/fautif_page_qui_deborde_module.wasm --navigateurs chrome,firefox --sans-construire` | **code 0** : Chrome et Firefox, 6 pires cas, 355 vues distinctes, 1 065 pages jugées aux trois tailles, aucun débordement ; le contrôle négatif vu (6 débordements) ; 9 s ; 0,77 Go |
| `sidequest-standard` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/sidequest --json …/sidequest-standard.json` | **code 0**, plan standard depuis les sources : E1 à E19 et E21 réussis, E20 pas exécuté (hors du plan) ; **E16 : 426 textes, 0 débordent, 0 glyphe absent** ; 551 s ; 2,33 Go ; charge 5,00 au départ, 3,33 à la fin |
| `sidequest-court` | `lourd target/release/soiree-epreuve ../games/sidequest --court --json …/sidequest-court.json` | code 0, plan court depuis les sources : chaque contrôle réussi, E20 pas exécuté ; 150 s |

`soiree-epreuve` est le binaire de la fin du lot 12 (`1edcdde5…`), non reconstruit.

#### Les mesures

Aucune. Les durées ci-dessus sont celles de commandes sous une charge de 2,3 à 5,4 (les autres agents) : pas des mesures. La mémoire résidente (`/usr/bin/time -l`) : 2,33 Go pour le plan standard (2,32 au lot 8), 0,77 Go pour `pages`.

#### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **La coupe voulue** (B19) n'existe pas dans le commun : la borne de `vues::coupe` la remplace, avec son prix (dit plus haut). Elle suppose la police du commun seule : une police de repli plus large (celle d'emoji, question 28) la rendrait fausse sans que le test de la chasse le voie.
- **Les pires cas ne couvrent que ce que E16 voit** : un pire cas par écran. La coupe entre les mots d'un texte que la borne accepte n'est éprouvée par E16 qu'à travers la forme `Pause` (sept lignes de « Ѡ ») ; le reste l'est par la borne elle-même (les tests). Les graphèmes faits de plusieurs glyphes larges (une suite de jamos, un emoji fait de liants), qu'aucune coupe ne coupe, ne sont pas dans les pires cas : un tel graphème de 180 points de code déborderait à toute coupe ; ni le jeu ni le commun ne le bornent aujourd'hui.
- **Les 13 emoji des bulles** se dessinent comme des boîtes (B20, question 28).
- Le `CAHIER.md` n'est pas changé : la pause montre toujours le signe « Ⅱ », en deux barres.

#### Ce que le lot suivant doit savoir

- La référence du plan court de Sidequest revient à celle du lot 7 : code 0 depuis les sources, chaque contrôle réussi (E20 hors du plan) ; par `--wasm`, code 3 (faute d'étape 1), E16 réussi.
- Le module de Sidequest change : sha256 `e80b86a9…`, 749 827 octets (`62043526…` aux lots 7 à 12). Une épreuve ou un scénario qui le prend dans `target/modules-figes/` doit le reprendre.
- Les pires cas de Sidequest sont nouveaux (prénoms et bulles en « @ », questions en « ﬄ », moment en « Ѡ », trois premiers à égalité) : ce que E16, E21, `pages` et `dessin` en voient change. La scène de la télé change aussi en soirée : le `StyleTexte` de la question, des réponses, de la bulle et du moment porte la coupe que `vues::coupe` choisit (entre les mots pour tout texte des paquets, quand la bulle était toujours entre graphèmes), et les réponses et la bulle ont `Lignes::Libre` ; la pause dessine deux rectangles au lieu d'un texte.
- Pour regarder la page : `target/travail/lot-7b-sidequest/capture/target/debug/capture-pages-sidequest <module> <dossier> [largeur hauteur]` (Chrome sans interface, une capture par pire cas, chaque vue jugée). Un outil de travail hors dépôt.

### Punchline

Fait le 2026-11-08, de 04 h 00 à 05 h 05 environ (UTC), par l'agent du jeu, dans `games/punchline` seulement (branche `refonte`, `HEAD` `f84b86f`), à côté des agents de Sidequest et de Tier list (le portillon partagé), sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). Chaque cargo, chaque épreuve, `soiree-banc pages`, l'outil de capture et les contrôles négatifs sont passés par le portillon, sauf des `cargo check -p punchline` et `cargo fmt` de quelques secondes et des E1+E16 sur un module seul (moins d'une seconde). Le commun n'est pas modifié (`bc56fec`, propre). Rien n'est commité. Chaque navigateur lancé a été fermé (par le banc, ou par l'outil de capture, qui tue Chrome et efface son profil).

#### Ce qui est fait

- **Les neuf débordements que E16 a trouvés au lot 8** (« Lot 8 », « Les débordements des trois jeux ») :
  - **Le mot de 80 « W » sans espace dans les cases des bulles** (pires cas 4 à 7) : en `Coupe::Mots`, un mot plus large que sa boîte reste entier (PHASE-3, § 2.9). Les cases se coupent maintenant entre les mots, **sauf si un groupe insécable dépasse 16 caractères** (`ecran::MOT_LONG_MAX`, `ecran::coupe_d_une_case`) : la case entière se coupe alors entre graphèmes. Un mot sans lettre ni chiffre (« ! », « : ») colle à ses voisins, comme la coupe du commun colle le signe isolé (SQ-46). La borne est calculée, écrite en tête de `MOT_LONG_MAX` : à la plus petite taille d'une case (18), une ligne de 600 fait 33,3 cadratins, « Æ », la lettre la plus large de Schoolbell (lue dans `hmtx`), 0,882, l'espace 0,310 ; coupées entre les mots, trois lignes tiennent 80 lettres tant que 1,764 × L ≤ 29,4, soit L ≤ 16. Une recherche au hasard sur ce même modèle ne trouve un débordement qu'à partir de L = 19 (trois témoins dans le test). Le mot de 80 « W » tient maintenant à 26,8 (bulle de 600 × 280) et 25,8 (case de finale, 600 × 93), sur trois lignes.
  - **Le titre de l'égalité à huit** (pire cas 9, 122 caractères sur une ligne de 1 200) : trois lignes au plus (`Lignes::AuPlus(3)`), boîte de 1 200 × 125 (au lieu de 1 200 × 90), qui descend jusqu'à la couronne de la plus haute marche ; il tient à 34,1 sur trois lignes ; « Léa gagne ! » reste à 80.
  - **Le pire cas « des mots larges »** est passé de dix mots de 7 « W » à des mots de 16 « Æ » (la borne même) : 21,0 sur trois lignes, coupé entre les mots. Il est plus dur que l'ancien, pas plus facile.
- **Un glyphe absent** : la pastille de l'écriture « ✓ », qu'aucune police connue ne dessine (4 des 12 glyphes absents de E16), devient « ● » (dans la police du commun ; le cahier dit « pastille jaune »). Restent 8 glyphes absents, les couronnes « 👑 » : la police d'emoji du commun (PHASE-3, question 28).
- **La page vue dans un navigateur.** `soiree-banc pages` est vert (61 vues de pire cas, 183 pages aux trois tailles, Chrome et Firefox ; le scénario du lot 12 ne demandait rien à corriger chez Punchline). Pour la regarder, un outil de travail hors dépôt (`target/travail/lot-7b/punchline/capture/`, `capture-pages-punchline <module> <sortie> [largeur hauteur]`), fait des pièces du banc : `banc::pages::preparer`, le relais en mode banc, la page du téléphone du dernier paquet `web/`, Chrome sans interface en émulation de téléphone, les crochets `__banc.morceau` et `__banc.pire_cas`, puis `Page.captureScreenshot` pour chaque vue distincte (le banc ne garde aucune image). Vu : aucune page ne déborde, mais **les textes à l'encre sombre du jeu (titres, consignes, libellés) se lisent à peine sur le fond #111 de la page du commun**. Cause : la page du commun (`web/telephone`) n'applique pas `Reglages::fond`, que le jeu donne à chaque vue (la couleur du personnage). Rien n'est contourné dans le jeu (pas de plaque sous chaque texte) ; c'est la demande du § 13 de `BESOINS-COMMUN.md` du jeu. Le jeu corrige sa part : avant d'avoir un personnage, sa vue demandait aucun fond (`None`), donc le #111 du commun, sous une encre sombre ; elle demande maintenant la pâte grise du cahier (§ 6).
- **`BESOINS-COMMUN.md`** du jeu : § 12 (les textes mesurés ; la demande d'une coupe qui ne coupe un mot que s'il ne tient pas seul, `overflow-wrap: anywhere`, qui rendrait `MOT_LONG_MAX` inutile : un changement de la forme de la scène, donc de l'ABI ; les glyphes des polices de repli, jusqu'à 1,57 cadratin, qu'aucune coupe ne borne ; la couronne), § 13 (la page n'applique pas `Reglages::fond`, ni `ecran_allume`), et « Ce que dit l'épreuve » refait. `AGENTS.md` du jeu : la table des tests, le choix des textes de la télé, la page vue, les chiffres du plan standard.
- **Une dépendance de test** : `mise-en-page` du commun, par chemin (la règle « même commun » l'admet), pour mesurer dans les tests du jeu comme E16. Le `Cargo.lock` du jeu gagne ses 24 crates (27 paquets avec leurs versions), **aux versions du `Cargo.lock` du commun** (comparé paquet par paquet), dont `unicode-script` 0.5.8 et `yazi` 0.2.1 tenues par `cargo update --precise` (Cargo prenait 0.5.9 et 0.2.2, de moins de deux semaines) ; rien n'est téléchargé (`--offline`).

#### Les écarts

1. **La règle de coupe des bulles est une approximation**, avec les deux coupes qui existent, de la coupe de l'ancien jeu (`overflow-wrap: anywhere`) : une case qui a un seul groupe de plus de 16 caractères se coupe tout entière entre graphèmes, ses autres mots compris. Ce n'est pas un vocabulaire inventé ni une mesure lue par le jeu (le jeu compte des caractères) ; la borne vient de la chasse des polices, calculée hors du jeu et tenue par les tests. Un relecteur peut la voir comme un contournement : la coupe juste est demandée au commun (§ 12 du `BESOINS-COMMUN.md` du jeu).
2. **La règle ne borne pas les glyphes des polices de repli** : une lettre absente de Schoolbell se mesure dans une police plus large (jusqu'à 1,57 cadratin, Rubik) ; 80 de celles-là ne tiennent dans aucune coupe. Aucun pire cas ne le dit ; c'est écrit au § 12.
3. **Les pires cas de la page n'ont pas d'écran de vote à boutons** (dans le pire cas du vote, chacun a déjà voté) : les boutons de vote se voient dans `soiree --pages` (lot 12), pas dans les captures de ce lot.
4. PHASE-3 n'a pas été trouvé faux sur la part de Punchline ; il n'est pas modifié.

#### Les tests qui le tiennent

- `tests/mesure.rs` (nouveau ; les textes de la scène mesurés par `mise-en-page` avec les polices du jeu dans l'ordre de la déclaration, comme E16) : `une_reponse_tapee_tient_dans_sa_bulle_pun54` (proptest, 64 cas : trois réponses de mots de 1 à 24 lettres parmi les plus larges de Schoolbell, avec des signes isolés, jouées jusqu'au premier duel, chaque texte de la scène mesuré ; PUN-54) ; `des_mots_de_seize_lettres_au_plus_se_coupent_entre_les_mots` (proptest, 64 cas) ; `un_mot_trop_long_se_coupe_entre_deux_graphemes` (le mot de 80 « W », un mot de 17) ; `un_signe_isole_compte_avec_son_mot_sq46` ; `chaque_texte_des_pires_cas_tient_pun54` (E16 avant l'épreuve) ; `aucun_glyphe_absent_hors_la_couronne` ; `sans_borne_la_coupe_entre_les_mots_deborderait` (trois témoins de 80 caractères au plus, en mots de 19 et 20 « Æ », qui débordent coupés entre les mots et tiennent par la règle). Une fois, à la main, les deux proptests à 4 096 cas chacun : verts (5,4 s ; le fichier remis à 64 cas ensuite).
- **Les contrôles négatifs** (`target/travail/lot-7b/punchline/controles.py`, six mutations de `ecran.rs`, chacune rouge par son test nommé et pour sa raison, la source remise et vérifiée par son sha256, la graine qu'un proptest rougi garde effacée) : la borne relâchée à 20 (`sans_borne…` : « (Mots) : »), toujours entre les mots (`un_mot_trop_long…`), toujours entre graphèmes (`des_mots_de_seize…`), le titre remis sur une ligne de 90 (`chaque_texte_des_pires_cas…` : « Égalité »), « ✓ » remis (`aucun_glyphe_absent…`), le signe isolé non collé (`un_signe_isole…`). Aux essais, une borne relâchée à 24, puis à 20, ne rougissait pas le pire cas (« 24 24 24 5 » et « 20 20 20 17 » tiennent : la dernière ligne prend deux mots) : les témoins de `sans_borne…` viennent de là, et c'est lui que la mutation vise.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7b/punchline/*/` (hors dépôt ; `commande.txt`, `sortie.txt`, `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` par `target/travail/lot-7b/punchline/empreinte.sh` : pour le commun et Punchline, `HEAD`, sha256 de `git diff HEAD`, de `git diff HEAD` hors des `.md`, des fichiers non suivis, et le sha256 du module), lancées à la suite par `target/travail/lot-7b/punchline/final.sh` de 04 h 42 min 34 s à 04 h 55 min 11 s (`final.log`). **Les 12 preuves ont la même empreinte du code au départ et à la fin** : le commun `bc56fec`, aucune modification ; Punchline `f84b86f`, `diff HEAD` hors `.md` `c8ba92030d004436`, non suivi `crates/punchline/tests/mesure.rs` (`53f4a9a83f71255a`) ; module sha256 `4c3ebf10…` (943 772 octets, BLAKE3 `51bf9b6b…`). Seuls des `.md` ont changé après (`BESOINS-COMMUN.md` et `AGENTS.md` du jeu, ce journal). `modules.txt` : les sha256 du module, de `soiree-epreuve` (`1edcdde5…`, celui du lot 12), de `soiree-banc` (`ff48ae01…`) et de l'outil de capture.

| Preuve | Commande | Résultat |
|---|---|---|
| `jeu-fmt` | dans Punchline : `cargo fmt --all -- --check` | code 0 |
| `jeu-clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `jeu-clippy-wasm32` | `lourd cargo clippy -p punchline -p punchline-module --target wasm32-unknown-unknown -- -D warnings` | code 0 |
| `jeu-tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0 : 92 réussis** (85 au lot 7, et les 7 de `mesure.rs`), 0 échec, 0 ignoré ; 15 s ; 0,30 Go |
| `jeu-module` | `lourd cargo build -p punchline-module --target wasm32-unknown-unknown --profile module` | code 0 ; 943 772 octets, sha256 `4c3ebf10…` |
| `controles` | `lourd python3 -I target/travail/lot-7b/punchline/controles.py …/controles/mutations` | code 0 : les six mutations rouges pour leur raison (556 s, dont 505 pour la première, surtout à attendre une place du portillon : le script tient une place, chaque test en prend une autre) |
| `construire-release` | `lourd cargo build --release -p epreuve --bin soiree-epreuve` | code 0, rien à refaire |
| `e16-module` | `soiree-epreuve --wasm <module> --controles E1,E16` | code 3 (module seul, étape 1 pas faite : jamais 0) ; **E16 réussi** : 142 textes, 0 débordement, 8 glyphes absents ; E1 réussi |
| `court-punchline` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/punchline --court --json …` | code 1, **E2 seul** (« une fin, mais aucune autre après la nouvelle partie ») : la référence du lot 7, rendue (E16 réussi) ; 40 s ; 1,25 Go |
| `standard-punchline` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/punchline --json …` | **code 0** ; E16 réussi (142 textes, 0 débordement) ; E21 réussi (2 500 évaluations, 74 158 vues distinctes) ; E19 : une page au plus 728 878 unités ; 112 s ; 1,73 Go ; charge 2,69 au départ, 2,31 à la fin, 2,89 au plus |
| `pages` | `lourd /usr/bin/time -l target/debug/soiree-banc pages <module> --negatif <fautif_page_qui_deborde> --navigateurs chrome,firefox,safari --sans-construire …` | **code 0** : Chrome et Firefox réussis (10 pires cas, 61 vues distinctes, 183 pages aux trois tailles, de 5 à 17 nœuds, aucun débordement ; le contrôle négatif vu, 6 débordements) ; Safari pas exécuté ; 9 s |
| `capture` | `lourd target/travail/lot-7b/punchline/capture/target/release/capture-pages-punchline <module> …/captures 390 844` | code 0 : 61 captures PNG (`captures/vue-NN-pire-cas-I-appareil-N-390x844.png`), chacune sans débordement selon la page ; polices chargées |

**Les captures jointes** (dans `target/preuves/lot-7b/punchline/captures/`) : `vue-08-pire-cas-0-appareil-9-390x844.png` (le prénom, avant d'avoir un personnage), `vue-00-pire-cas-0-appareil-1-390x844.png` (le salon de l'organisateur, huit prénoms de 12 « W »), `vue-27-pire-cas-3-appareil-1-390x844.png` (l'écriture de la finale, trois cases), `vue-38-pire-cas-4-appareil-3-390x844.png` (vote enregistré), `vue-53-pire-cas-9-appareil-1-390x844.png` (le podium, « On rejoue ! »). On y voit le défaut du § 13 : l'encre sombre sur #111.

#### Les mesures

Aucune mesure de temps dans ce lot : les durées ci-dessus sont celles de commandes sous une charge de 2,2 à 3,9 (le portillon). Le carburant (E19) est déterministe : une page du téléphone au plus 728 878 unités (846 390 au lot 7), médiane 72 323.

#### Ce qui n'est pas fait, et pourquoi

- **La coupe juste des bulles** (`overflow-wrap: anywhere`) : demandée au commun (vocabulaire de la scène, ABI) ; d'ici là, la règle de `MOT_LONG_MAX`.
- **La lisibilité de la page** : au commun (`Reglages::fond` non appliqué) ; le jeu ne la contourne pas.
- **La couronne** reste une boîte : la police d'emoji du commun (question 28).
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- E20 n'est pas relancé : ni le relevé, ni le banc, ni le moteur ne changent, et la ligne du lot ne le demande pas.

#### Ce que le lot suivant doit savoir

- Le module de Punchline change : sha256 `4c3ebf10…`, BLAKE3 `51bf9b6b…` (`82669f77…` aux lots 7 à 12). Une épreuve ou un scénario qui le prend ailleurs doit le reprendre.
- La référence du plan court de Punchline revient à celle du lot 7 : code 1, E2 seul.
- `tests/mesure.rs` du jeu mesure comme E16 : le commun qui change la règle de `mise-en-page` (PHASE-3, § 2.9) le verra rougir dans le jeu, et `ecran::MOT_LONG_MAX` est à recalculer si l'interligne, la police de Schoolbell ou la taille d'une case changent.
- `Cargo.lock` de Punchline porte `mise-en-page` et ses dépendances aux versions du commun ; à tenir quand le commun met à jour `cosmic-text` ou ce qu'elle tire.
- Pour regarder la page d'un jeu : `target/travail/lot-7b/punchline/capture/` (un outil de travail hors dépôt, qui dépend de `banc` et `relais` par chemin ; `cargo build --offline --release` dans son dossier, par le portillon). Un crochet de capture dans `soiree-banc pages` le rendrait inutile.

### Le commun, après les trois jeux

Fait le 2026-11-08, de 04 h 57 à 05 h 15 environ (UTC), par l'agent du commun, après les parts des trois jeux ci-dessus, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1, Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté »). **Le code du commun ne change pas** (`bc56fec`, `git diff HEAD` hors `.md` vide à chaque preuve) ; seuls changent des documents. Chaque commande lourde est passée par le portillon. Rien n'est commité. Fichiers de travail : `target/travail/lot-7b/commun/` ; preuves : `target/preuves/lot-7b/commun/`.

#### Ce qui est fait

- **La preuve de la ligne du lot, refaite sur l'arbre final, les trois jeux ensemble.** Les arbres des jeux sont, hors `.md`, ceux que leurs agents ont éprouvés (mêmes `HEAD`, mêmes empreintes de `git diff HEAD` hors `.md` : Punchline `c8ba92030d004436` avec `tests/mesure.rs` non suivi, Sidequest `2d05eb25159bb6aa`, Tier list `49c033aab75e20e6` ; mêmes modules). La ligne du lot demande la preuve « après les trois jeux » : elle est refaite d'une traite plutôt que relue, puisque les trois jeux avaient travaillé en même temps.
  - **Plan standard, depuis les sources, les trois au code 0.** Punchline : E1 à E19 et E21 réussis, E20 pas exécuté (hors du plan) ; E16, 142 textes, 0 débordement, 8 glyphes absents (les couronnes « 👑 », question 28). Sidequest : E1 à E19 et E21 réussis, E20 pas exécuté ; E16, 426 textes, 0 débordement, 0 glyphe absent. Tier list : E1 à E19 réussis ou sans objet (E2, E10, E11), E20 pas exécuté, E21 sans objet (pas de téléphone) ; E16, 1 151 textes, 0 débordement, 10 080 glyphes absents (les idéogrammes de ses pires cas, que Liberation Sans n'a pas : PHASE-3, § 2.9, « Pas de CJK »).
  - **`pages` sur les trois modules en une seule fois**, le paquet de la page reconstruit depuis les sources du commun (sans `--sans-construire`) : Chrome et Firefox réussis ; Punchline 10 pires cas, 61 vues, 183 pages jugées aux trois tailles ; Sidequest 6 pires cas, 355 vues, 1 065 pages ; Tier list sans téléphone, sans objet ; le socle 4 vues ; aucun débordement ; le contrôle négatif voit ses 6 débordements dans chaque navigateur. Safari pas exécuté (`safaridriver`).
  - **Le plan court de Tier list depuis ses sources** (l'agent du jeu l'avait joué par `--wasm`, qui rend 3 faute d'étape 1) : code 0, la référence du lot 7. Ceux de Punchline (code 1, E2 seul) et de Sidequest (code 0) l'ont été depuis leurs sources par leurs agents, sur les mêmes arbres. **La référence du plan court revient à celle du lot 7** ; c'est écrit dans PHASE-3, § 7.
  - **Chaque agent a vu sa page** : les captures sont dans `target/preuves/lot-7b/punchline/captures/` (61), `target/preuves/lot-7b/sidequest-capture-390x844/png/` (5) et `target/preuves/lot-7b/tier-list/captures/` (7) ; relu ici `vue-27-pire-cas-3-appareil-1-390x844.png` de Punchline : l'encre sombre du jeu sur le #111 de la page du commun (la demande D2).
- **`conception/BESOINS-DES-JEUX.md`, « Les demandes ouvertes après le lot 7b »** : les 22 demandes des jeux restées ouvertes (Punchline § 11, § 12, § 13 ; Sidequest B12 à B20 ; Tier list § 7, § 8, § 17 à § 21), D1 à D22, chacune avec ce que le commun fait aujourd'hui (lu dans son code), ce qu'elle coûterait (estimé, non mesuré) et si elle demande une ABI nouvelle ; **aucune n'est tranchée**. En tête, ce que coûte une montée d'ABI en soi, et que plusieurs demandes peuvent la partager. Deux viennent de plusieurs jeux : D1 (la coupe « mots puis graphèmes », des trois) et D3 (l'avis d'un refus, de deux). Lu en chemin et dit : la ressource d'un `Lien` (Tier list § 7, « pour le lot 11 ») est envoyée à la page depuis le lot 11 (`salle::ecran::images_citees`) ; la majuscule de la page suit déjà le `Clavier` (Punchline § 11.5) ; D1 peut se faire sans ABI si `Coupe::Mots` change de sens, ce que le mainteneur peut préférer à une troisième coupe.
- **PHASE-3.md**, § 7 : la ligne du lot 7b porte « écrit après le lot », et la référence du plan court « ramenée au lot 7b ». **CHANGELOG.md** : une section « Phase 3, lot 7b » (rien à changer pour un jeu ; les demandes ouvertes). `crates/README.md` et `AGENTS.md` du commun ne changent pas : rien de ce qu'ils disent ne change.

#### Les écarts

1. **« Capture du banc »** (ligne du lot) : le banc ne fait pas de capture d'écran ; les captures viennent d'outils de travail hors dépôt faits de ses pièces, dans Chrome seulement (Firefox n'est vu que par `pages`). Écrit dans la ligne du lot, PHASE-3, § 7.
2. **La part du commun ne relance pas `cargo test --workspace` ni clippy du commun** : son code est celui de la fin du lot 12, vérifié par la correction de la fin du lot 12, et rien n'y a changé (`diff HEAD` hors `.md` vide) ; seul `cargo fmt --all -- --check` a été relancé, dans les deux workspaces (code 0). Les tests, clippy et fmt des jeux sont ceux de leurs agents (leurs sections ci-dessus), sur les mêmes arbres.

#### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7b/commun/<nom>/` : `commande.txt`, `sortie.txt` (complète), `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (par `empreinte.sh` : pour les quatre dépôts `HEAD`, sha256 de `git diff HEAD`, du même hors `.md`, des fichiers non suivis ; sha256 des trois modules, de `soiree-epreuve`, de `soiree-banc` et du fautif `page-qui-deborde`) ; les scripts `preuve.sh`, `empreinte.sh`, `final-a.sh`, `final-b.sh` y sont copiés ; `empreinte-finale.txt`, après les documents. **Empreinte du code, identique au début et à la fin de chaque preuve** : le commun `bc56fec`, hors `.md` vide (le `diff` entier change pendant `standard-sidequest` et `standard-tier-list` : ce sont les `.md` de ce lot) ; Punchline `f84b86f` / `c8ba92030d004436` ; Sidequest `229199b` / `2d05eb25159bb6aa` ; Tier list `9827f39` / `49c033aab75e20e6`. Modules (sha256) : Punchline `4c3ebf103714ac09…` (943 772 octets ; BLAKE3 `51bf9b6b…` dans le rapport), Sidequest `e80b86a9cf017d65…` (749 827 ; BLAKE3 `3ac3d7f1…`), Tier list `ae27434d35d9ce3d…` (1 615 421 ; BLAKE3 `1294a3e9…`) : les mêmes que ceux des agents, et ceux que l'épreuve a compilés depuis les sources. `soiree-epreuve` `1edcdde5…` (celui du lot 12, rien à refaire) ; `soiree-banc` reconstruit par `construire` (`05e7890d…`, les sources du commun inchangées).

| Preuve | Commande (depuis `soiree/`, chacune par `target/hote/lourd`) | Résultat |
|---|---|---|
| `construire` | `sh -c "cargo build --release -p epreuve --bin soiree-epreuve && cargo build -p banc --bin soiree-banc"` | code 0 ; 8 s |
| `standard-punchline` | `/usr/bin/time -l target/release/soiree-epreuve ../games/punchline --json …/standard-punchline.json` | **code 0** ; E16 142 textes, 0 débordement ; E21 2 500 évaluations ; 110 s, 2,05 Go |
| `standard-tier-list` | `… ../games/tier-list --json …/standard-tier-list.json` | **code 0** ; E16 1 151 textes, 0 débordement ; 232 s, 1,29 Go |
| `standard-sidequest` | `… ../games/sidequest --json …/standard-sidequest.json` | **code 0** ; E16 426 textes, 0 débordement ; E21 4 395 évaluations ; 589 s, 2,75 Go |
| `pages` | `/usr/bin/time -l target/debug/soiree-banc pages <les trois modules> --negatif target/wasm32-unknown-unknown/module/fautif_page_qui_deborde_module.wasm --navigateurs chrome,firefox,safari --travail target/travail/lot-7b/commun/pages-travail` | **code 0** : Chrome et Firefox réussis, Safari pas exécuté ; 20 s, 0,77 Go |
| `court-tier-list` | `/usr/bin/time -l target/release/soiree-epreuve ../games/tier-list --court --json …` | code 0 (la référence du lot 7) ; 41 s, 0,32 Go |

`standard-sidequest` a tourné en même temps que `standard-punchline`, `standard-tier-list` et `pages`, l'un après l'autre (deux places du portillon).

#### Les mesures

Aucune mesure de temps : les durées ci-dessus sont celles de commandes lancées deux à la fois, sous une charge de 1,5 à 2,9 (relevée par le portillon : au plus 2,13 pour Punchline, 2,85 pour Tier list, 2,81 pour Sidequest, 2,70 pour `pages`), pas au calme. La mémoire résidente est celle de `/usr/bin/time -l` (Sidequest 2,75 Go, contre 2,33 Go chez son agent et 2,32 au lot 8 : non expliqué, non mesuré à part ; ce n'est pas une borne).

#### Ce qui n'est pas fait, et pourquoi

- **Aucune demande des jeux n'est tranchée** : c'est au mainteneur (la mission). D2, D4, D6, D7 et D22 sont petites et sans ABI ; elles attendent quand même sa décision.
- **Safari** : pas exécuté (`safaridriver` refuse la session ; le réglage de l'hôte n'est pas touché).
- **Un scénario de capture dans le banc** : non écrit (hors de la ligne du lot) ; deux outils de travail le font aujourd'hui, hors dépôt (`target/travail/lot-7b/punchline/capture/`, `target/travail/lot-7b/tier-list/capture.mjs`).
- E20 n'est pas relancé : ni le relevé, ni le banc, ni le moteur ne changent, et la ligne du lot ne le demande pas.

#### Ce que le lot suivant doit savoir

- Les modules des trois jeux changent (sha256 ci-dessus ; aux lots 7 à 12 : Punchline `82669f77…`, Sidequest `62043526…`, Tier list `84315a1b…`) : une épreuve ou un scénario qui les prend dans `target/modules-figes/` doit les reprendre.
- La référence du plan court est de nouveau celle du lot 7 : Sidequest 0 ; Punchline 1, E2 seul ; Tier list 0.
- Les demandes ouvertes sont D1 à D22 dans `BESOINS-DES-JEUX.md` ; celles qui demandent une ABI (D1 par une troisième coupe, D3, D5, D8 à D12, D13 pour le centre, D14, D17, D18 pour l'allure, D19, D20 pour le sélecteur) peuvent partager une seule ABI 5. Punchline (`tests/mesure.rs`, `MOT_LONG_MAX`) et Sidequest (`vues::CHASSE_MAX`, et un test qui lit `crates/mise-en-page/polices/LiberationSans-Regular.ttf` par son chemin) dépendent désormais de la règle et de la police de `mise-en-page` : un changement de l'une ou de l'autre (D1, D15) les fait rougir, et c'est voulu.

## Lot 7b, correction après la vérification

Fait le 2026-11-08, de 05 h 20 à 05 h 45 environ (UTC), par un agent seul, sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2, rustc 1.99.1), d'après la vérification adverse du lot 7b (six constats : un important, cinq mineurs). Chaque cargo, chaque épreuve est passé par le portillon (sauf `cargo fmt`) ; aucun navigateur n'a été lancé. Rien n'est commité. Fichiers de travail : `target/travail/lot-7b-correction/` ; preuves : `target/preuves/lot-7b-correction/`.

### Les constats, et ce qui en est fait

1. **[important] La page de Punchline se lit à peine, parce que `web/telephone` n'applique pas `Reglages::fond`.** Vérifié : `page.rs` pose `background: #111` au corps et ne lit de `MontreFil` que ses effets ; INTERFACE (§ 2.9, `Montre { vue, effets, reglages }`) et `jeu_abi` (« La couleur de fond de la page ») donnent ce fond au jeu. **Tranché comme un écart de la page à INTERFACE, né au lot 12**, et non comme une demande nouvelle d'un jeu : c'est écrit dans la ligne du lot 12 (PHASE-3, § 7, « écrit après la vérification du lot 7b »), dans D2 (`BESOINS-DES-JEUX.md`, « Rangée après la vérification du lot 7b »), au CHANGELOG (section du lot 7b) et au § 13 du `BESOINS-COMMUN.md` de Punchline. Il se corrige dans le commun, sans ABI (`web/telephone`, et un constat du fond au scénario `pages`), au premier lot qui touche la page et au plus tard avant la fiche de répétition du lot 14 ; le mainteneur n'a à choisir que ce que gardent les écrans du commun (le voile, l'erreur). **Le code du commun n'est pas changé ici** : la ligne du lot 7b ne touche que les jeux (« dans chaque dépôt, par l'agent du jeu »), et la vérification range l'écart au lot 12. Dit aussi, au § 13 de Punchline : d'ici là, le passage de la vue sans personnage à `Some(PATE_GRISE)` n'a aucun effet visible.
2. **[mineur] « Capture du banc »** : le banc ne fait pas de capture ; c'était déjà écrit dans la ligne du lot 7b et dans ce journal. Rien de simple à ajouter (un scénario de capture dans le banc est un travail du commun, hors de la ligne du lot) ; rien n'est changé.
3. **[mineur] La preuve `sidequest-controles` était reconstituée à la main, et son script laissait dans `target/` le module du dernier mutant E16.** Corrigé : `target/travail/lot-7b-correction/controles.py` reprend les huit mutations de Sidequest à l'identique, note le sha256 du module au départ et, à la fin, sources remises, **reconstruit le module et exige le même sha256** (code 2 sinon) ; la preuve est refaite par `preuve.sh` (commande, sortie complète, début, fin, durée, empreinte des quatre dépôts et des trois modules au départ et à la fin). Le module est revenu à `e80b86a9…`. Le premier script (`target/travail/lot-7b-sidequest/mutations.py`) n'est pas à reprendre.
4. **[mineur] Les règles de coupe des jeux ne bornent que les polices déclarées** (repli plus large, graphème de plusieurs glyphes larges) : déjà dit dans les deux `BESOINS-COMMUN.md` et ici ; la coupe juste est D1. Aucun pire cas simple ne le couvrirait sans connaître la police de repli de chaque navigateur ; rien n'est changé.
5. **[mineur] Deux corrections sans test ni contrôle négatif.** Corrigé, par deux tests et trois mutations :
   - Sidequest, `tests::aucun_texte_de_la_tele_n_a_de_glyphe_absent` (`src/tests.rs`) : chaque texte et chaque nom de bouton de la télé, à l'accueil, au salon avec un personnage et en pause, lu dans la `cmap` de la police du commun (la lecture de `Chasses::du_commun`, déjà là) ; E16 compte un glyphe absent sans échouer, ce test le tient. La mutation `sq-pause-en-texte` (la pause remise en texte « Ⅱ », le code d'avant le lot) le rougit : « pause : « Ⅱ », absents « Ⅱ » ».
   - Tier list, `chaque_champ_se_pose_sur_un_fond_sombre` (`tests/partie.rs`) : chaque `Champ` de la télé (le nom d'une tuile, au plateau neuf ; celui des réglages) sur un rectangle sans composante au-dessus de 0x40 (le commun l'écrit en blanc) ; la recherche du rectangle dessous est mise en commun avec `aucun_texte_n_a_la_couleur_de_son_fond` (`fond_sous`, même règle qu'avant). Les mutations `tl-zone-de-texte-blanche` et `tl-champ-du-nom-blanc` le rougissent (« réglages, … : sur Some(Rgba(255, 255, 255, 255)) »).
6. **[mineur] Le commun n'a relancé ni ses tests ni clippy** : justifié par la vérification elle-même ; son code est toujours à `bc56fec`, `diff HEAD` hors `.md` vide (empreinte ci-dessous). Rien n'est changé.

### Ce que les jeux voient

Rien ne change dans leurs modules : seuls des tests (`src/tests.rs` de Sidequest, `tests/partie.rs` de Tier list) et des `.md` changent. **Les trois modules gardent leur sha256 du lot 7b** (Punchline `4c3ebf10…`, Sidequest `e80b86a9…`, Tier list `ae27434d…`), et le commun ses sources : par la règle « une épreuve déjà faite sur le même module, avec les mêmes sources du commun, ne se relance pas », le plan standard des trois jeux et `pages` restent ceux du lot 7b (`target/preuves/lot-7b/commun/`). Le plan court de Sidequest et de Tier list est relancé depuis leurs sources, parce que l'étape 1 de l'épreuve passe clippy sur les tests du jeu : il l'a montré (ci-dessous).

### Les tests qui la tiennent

`sidequest` : `tests::aucun_texte_de_la_tele_n_a_de_glyphe_absent` (nouveau), et les quatre du lot 7b. `tier-list` : `partie::chaque_champ_se_pose_sur_un_fond_sombre` (nouveau), `aucun_texte_n_a_la_couleur_de_son_fond` (son aide réécrite, même règle). Les onze contrôles négatifs de `controles.py`, tous rouges pour leur raison.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-7b-correction/<nom>/` : `commande.txt`, `sortie.txt` (complète), `code.txt` (début, fin, code, durée), `empreinte.txt` et `empreinte-fin.txt` (par `target/travail/lot-7b-correction/empreinte.sh` : pour les quatre dépôts `HEAD`, sha256 de `git diff HEAD`, du même hors `.md`, des fichiers non suivis ; sha256, taille et date des trois modules). **Empreinte du code à la fin de chaque preuve gardée** : le commun `bc56fec`, hors `.md` vide (`e3b0c442…`) ; Punchline `f84b86f`, hors `.md` `c8ba92030d004436`, non suivi `tests/mesure.rs` (inchangé depuis le lot 7b ; ce script hache les non suivis autrement que celui de Punchline, d'où `bd569872…` ici pour `53f4a9a8…` là-bas) ; Sidequest `229199b`, hors `.md` `c8476b63f668c7d1` ; Tier list `9827f39`, hors `.md` `8ceb7cc17a911f47` ; modules `4c3ebf10…`, `e80b86a9…`, `ae27434d…`.

| Preuve | Commande | Résultat |
|---|---|---|
| `controles` | depuis `soiree/` : `python3 -I target/travail/lot-7b-correction/controles.py` (chaque cargo et chaque épreuve par `lourd`) | **code 0** : 11 mutations, 11 rouges ; six par E16 (plan court, module seul : le tableau, la bulle, le moment et les réponses toujours entre les mots, la bulle à 3 lignes « 4 lignes, pour 3 permise(s) », les réponses à 2 « 3 lignes, pour 2 permise(s) ») ; deux par `tests::la_coupe_entre_les_mots_seulement_quand_elle_tient` ; une par `tests::aucun_texte_de_la_tele_n_a_de_glyphe_absent` ; deux par `chaque_champ_se_pose_sur_un_fond_sombre` ; chaque source remise et vérifiée par son sha256 ; module de Sidequest reconstruit, revenu à `e80b86a9…` ; 182 s |
| `sq-fmt` | dans Sidequest : `cargo fmt --all -- --check` | code 0 |
| `sq-clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `sq-tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | code 0 : **89 réussis** (88 au lot 7b, et le nouveau), 0 échec, 0 ignoré ; 12 s |
| `sq-court` | depuis `soiree/` : `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/sidequest --court --json …/sq-court.json` | **code 0**, la référence du lot 7 ; étape 1 faite (clippy et clippy wasm32 compris) ; module BLAKE3 `3ac3d7f1…`, celui du lot 7b ; E16 réussi ; 128 s |
| `tl-fmt` | dans Tier list : `cargo fmt --all -- --check` | code 0 |
| `tl-clippy` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `tl-tests` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | code 0 : **101 réussis** (100 au lot 7b, et le nouveau), 0 échec, 0 ignoré ; 31 s ; 0,41 Go |
| `tl-court` | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/tier-list --court --json …/tl-court.json` | **code 0**, la référence du lot 7 ; étape 1 faite ; module BLAKE3 `1294a3e9…`, celui du lot 7b ; E16 réussi ; 59 s |

`premier-essai/` garde le premier passage, sur un arbre de Sidequest abandonné : `sq-clippy` y rend 101 et `sq-court` 1 (étape 1, « clippy : échouée »), le nouveau test ayant un `_ => continue` que `clippy::wildcard_enum_match_arm` refuse ; corrigé (deux `if let`), puis `controles`, `sq-*` relancés sur l'arbre final. Les preuves `tl-*` de ce premier passage sont gardées : l'empreinte de Tier list n'a pas changé depuis. `empreinte-finale.txt`, prise après ce journal : le code des quatre dépôts et les trois modules identiques à ceux des preuves ; seuls des `.md` ont changé depuis. `soiree status` : rien n'est commité ni poussé, comme voulu (un agent à part committe).

### Les mesures

Aucune mesure de temps : les durées ci-dessus sont celles de commandes sous une charge de 1,7 à 3,6 (le portillon), deux à la fois.

### Ce qui n'est pas fait, et pourquoi

- **La page qui applique `Reglages::fond`** : un écart du commun au lot 12, désormais écrit comme tel ; non corrigé ici (hors de la ligne du lot 7b).
- Le plan standard des trois jeux et `pages` ne sont pas relancés : modules et sources du commun identiques à ceux du lot 7b (sha256 ci-dessus), dont les preuves restent valides ; clippy wasm32 n'est pas relancé à part (les changements sont des tests, que la cible wasm32 ne compile pas ; l'étape 1 des plans courts l'a passé).
- Rien de Punchline n'est relancé : seul son `BESOINS-COMMUN.md` change.
- Safari : pas concerné (aucun navigateur lancé).

### Ce que le lot suivant doit savoir

- Le lot qui touche `web/telephone` corrige D2 (le fond de la page), avec un constat au scénario `pages` ; Punchline n'a rien à changer alors (sa vue donne déjà le fond).
- Les contrôles négatifs de Sidequest et Tier list pour le lot 7b sont dans `target/travail/lot-7b-correction/controles.py` (onze mutations), qui remet le module de Sidequest à son empreinte de départ.
- `aucun_texte_de_la_tele_n_a_de_glyphe_absent` (Sidequest) lit la `cmap` de `crates/mise-en-page/polices/LiberationSans-Regular.ttf` : une police du commun changée, ou des emoji dans un pire cas de la télé (B20, question 28), le feraient rougir, et c'est voulu.

## Lot 13 — Latence et diagnostic

Fait le 2026-11-08 sur la machine de l'hôte (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2 ; Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari « pas exécuté », l'automatisation à distance n'étant pas permise), dans `soiree/` seulement (branche `refonte`, `HEAD` `cf61d0f`), par deux agents : **le premier**, de 05 h 50 à 07 h 56 (UTC), a écrit tout le code, les tests, la plupart des documents et pris la plupart des mesures, puis a été arrêté par une limite d'usage sans rendre compte ; **le second** a repris à 18 h 54, relu chaque changement, gardé le code tel quel (aucune ligne de code changée à la reprise), corrigé ses scripts de preuve et ses documents, refait ce qui devait l'être, fini les mesures et écrit cette section. Les jeux ne changent pas (modules au sha256 du lot 7b : Punchline `4c3ebf10…`, Sidequest `e80b86a9…`, Tier list `ae27434d…` ; `ardoise` `400cc5ae…`).

### Ce qui est fait

- **La trace de bout en bout** (§ 8.4 ; `salle::trace`, `salle::ecran`, `salle::autorite`, `web/ecran/src/page/mesure.rs`, `web/autorite`) : chaque entrée de l'écran part au Worker avec son numéro, l'instant de son événement (`event.timeStamp` ramené à `timeOrigin`) et celui de son envoi (`DeLaPage::Ecran { numero, envoi_us }`) ; chaque geste placé d'un téléphone porte sa `TraceTelephone`, que l'autorité ramène du temps de la partie à son heure (`Horloge::heure_us`). L'autorité suit chaque entrée placée jusqu'au tour qui joue son pas (au plus `TRACES_EN_COURS_MAX`, 256, la plus ancienne s'oublie) et rend ses traces (`VersPage::Traces`) juste après la scène de ce tour, avec le début du tour et les fins du pas et des vues (le chronomètre du Worker tourne désormais hors du banc : quelques `performance.now()` par tour, rien qui décide). La page ajoute la scène reçue, le début de l'image qui la dessine en entier (**`Trace::image_us`, un champ nouveau**, qui sépare l'attente de l'image du dessin : sans lui, le premier essai dans Chrome ne disait pas ce qui pesait), le dessin soumis et le travail du GPU fini. Les segments : événement, transport, attente, pas, vues, vers la page, attente de l'image, dessin, GPU, total ; les distributions sans moyenne, au rang le plus proche. `relais` reste `None` (horodater en clair au relais changerait `liaison`) : le transport d'un téléphone comprend le chiffrement, l'envoi, le relais et la boucle locale. Une entrée dont la scène ne change pas n'a pas de pixel, et se compte. `ecran::PROTOCOLE` passe de 3 à 4 ; ni l'ABI, ni la garde, ni `salle::fil`, ni ce que joue l'épreuve ne changent.
- **Le panneau de diagnostic** (§ 2.9) : la touche que déclare le jeu (`VersPage::Jeu { diagnostic }`) l'ouvre et le ferme, et ne va plus au jeu ; un `<pre>` par-dessus la toile (du texte, pas encore des barres) : par source (clavier, souris, chaque téléphone), les 50ᵉ, 95ᵉ et 99ᵉ centiles de chaque segment sur les 200 derniers gestes, la ligne du budget du CONTRAT (« tenu » ou « DÉPASSÉ »), l'histogramme des pas par image, les entrées sans pixel, le moteur de dessin, les glyphes absents, la période des images, glisser, et la ligne du matériel à part (« non mesuré ici ») ; un carré blanc de 48 px en bas à droite, au moins 100 ms à chaque entrée, panneau ouvert (REM-117, pour la caméra de la répétition). Les débordements n'y sont pas (la télé ne les mesure pas ; E16 les juge avant la soirée).
- **Le calage réel, mesurable** : la commande du banc `CommandeEcran::Calage(Non)` fait jouer le pas par la seule minuterie du Worker ; `CommandeEcran::Echo` répond aussitôt (M14). Ignorées hors du banc.
- **Glisser** (`rendu::glisser`, `Ecran::dessiner_entre`) : la télé glisse quand la médiane des 30 dernières périodes d'image est sous 10 ms (100 Hz et plus, choisi : la ligne du lot dit 120 Hz, le seuil laisse passer 100), ou quand le banc le force ; un nœud glisse s'il porte `glisse` sans `rupture` (TL-23), avec la même clé et la même sorte dans la scène d'avant ; seule sa position va de l'une à l'autre, en un pas (16,667 ms) depuis l'arrivée de la scène, en millièmes ; tailles, couleurs, textes et composants sont ceux de la dernière scène. Le pixel d'une entrée attend la fin du chemin (le pas de retard que coûte glisser, mesuré). *Faux tel quel, corrigé après la vérification* : la page glissait même sans nœud qui glisse ; voir plus bas.
- **`soiree-banc mesures <module>`** (`banc::mesures`) : M2 par de vraies touches (`Input.dispatchKeyEvent`, `input.performActions` ; le banc tape désormais chiffres et lettres, `navigateur::caractere_de`), de vrais clics sur « Effacer » (la souris) et les gestes des téléphones natifs, en quatre conditions (avec et sans calage, avec et sans glisser forcé), par source et par segment, le segment qui pèse le plus nommé quand le budget n'est pas tenu ; M14 par écho entre la page et son Worker, un autre onglet (`BroadcastChannel`) et le processus du banc ; le contrôle négatif (chaque dessin ralenti de 33 ms, `window.__banc.dessin_lent` : le segment du dessin doit monter d'au moins 25 ms en médiane, sinon « CONTRÔLE NÉGATIF RATÉ ») ; `--avec-interface`. Un rapport, pas une porte : code 1 seulement si un navigateur n'a pas pu mesurer ou si le contrôle négatif rate.
- **M6** : `soiree-banc arriere-plan --avec-interface --mode recouverte|reduite` (`soiree::Cacher`) ; Chrome se lance aussi avec sa fenêtre (`Chrome::lancer_avec`) ; `aide --avec-interface` reste Firefox seul.
- `conception/RESTE-PHASE-3.md` commencé pour ce que la ligne du lot y envoie (la latence ; le lot 14 l'écrit en entier) ; `CHANGELOG.md`, `crates/README.md`, `AGENTS.md`, PHASE-3 (§ 2.9, § 2.10, § 8.4, § 8.6, questions 27 et 30, ligne du lot 10a) et `MESURES-PHASE-3.md` mis à jour.

### Ce que la reprise a trouvé et corrigé

1. **L'écran de la machine était éteint pendant les mesures de Chrome et les deux M6 du premier agent** (`pmset -g log` : éteint de 02 h 33 à 06 h 54 min 33 s, et de 07 h 01 min 51 s à 18 h 52). **Chrome sans interface cale ses images sur l'écran** : 30 par seconde écran éteint, 60 écran allumé (Firefox sans interface : 120 dans les deux cas). Le premier agent en avait conclu que **le budget du clavier n'était pas tenu dans Chrome** (69 à 73 ms au 99ᵉ centile), et avait nommé l'attente de l'image d'une page « à 30 Hz » comme le segment fautif, dans `MESURES-PHASE-3.md`, `RESTE-PHASE-3.md`, PHASE-3 (M2) et `AGENTS.md`. Refait écran allumé (sous `caffeinate -d`, une assertion le temps de la commande, aucun réglage changé) : **34,5 et 35,5 ms au 99ᵉ centile, le budget tenu** sans glisser. Les quatre documents sont corrigés ; les mesures écran éteint restent écrites comme telles. Les deux M6 « recouverte » du premier agent sont renommées `m6-recouverte-<n>-ecran-eteint`, gardées comme constat, et M6 est refaite écran allumé.
2. ~~**`preuve.sh` hachait du vide pour les fichiers non suivis** (il les cherchait depuis `soiree/` au lieu de leur dépôt : `e3b0c442…` dans chaque `empreinte.txt` du premier agent, alors que sept fichiers nouveaux n'étaient pas suivis).~~ *Corrigé après la vérification* : c'était inexact. Le premier `preuve.sh` hachait les chemins que `git ls-files` donne relatifs à leur dépôt depuis `soiree/` : juste pour `soiree` (ses empreintes portent `2985e2dd…` aux plans courts et `bb9dab47…`, l'empreinte finale hors `.md`, à `ecran-tier-list` et aux `m2-*` de 06 h 50 et après), faux seulement pour un jeu qui aurait eu des fichiers non suivis ; les trois jeux n'en ont aucun, et leur `e3b0c442…` (le vide) est donc juste. Ce qui renforce la provenance des mesures du premier agent. Le script est quand même rendu sûr ; l'empreinte dit aussi le diff hors `.md`, les sha256 des modules, le paquet de la page, celui de `soiree-banc`, l'état de l'écran, et une seconde empreinte est prise à la fin. Les fichiers non suivis du premier agent n'ont plus changé après 06 h 49 (dates), avant chacune de ses mesures ; ce n'est pas une empreinte, et c'est dit.
3. Question 30 : la charge des passages d'`arriere-plan` était dite « au plus 2,24 à 2,98 » ; c'est 2,14 à 2,98.
4. Le seul `m6-reduite-chrome` du premier agent, interrompu sans code par l'arrêt, est gardé sous `m6-reduite-chrome-interrompu`.

### Les tests qui le tiennent

- `crates/salle/tests/trace.rs` (7) : `chaque_segment_va_d_une_borne_a_la_suivante_et_le_total_les_somme`, `un_segment_entre_deux_horloges_peut_etre_negatif` (M14), `une_trace_sans_pixel_n_a_ni_dessin_ni_total`, `les_centiles_sont_au_rang_le_plus_proche`, `le_panneau_garde_200_gestes_par_source_et_dit_le_budget` (OUT-88 : le matériel sur sa ligne), et par la vraie autorité dans `salle::simulation` : `chaque_geste_place_revient_en_trace_apres_son_pas`, `une_entree_de_l_ecran_revient_en_trace_juste_apres_sa_scene` (OUT-93).
- `crates/salle/tests/ecran.rs` : `PROTOCOLE` à 4, les octets figés et la seconde lecture de `DeLaPage::Ecran { numero, envoi_us }`, `VersPage::Jeu { diagnostic }`, `Trace::image_us`, `CommandeEcran::Calage` et `Echo`, chaque variant couvert.
- `web/rendu/tests/glisser.rs` (3) : `un_noeud_qui_glisse_va_a_mi_chemin_et_garde_le_reste_de_la_derniere`, `rupture_cle_absente_ou_sorte_changee_ne_glissent_pas` (TL-23), `le_decalage_d_un_groupe_glisse` (SQ-56, SQ-58).
- `banc::mesures::tests` (3) : `l_ecart_d_un_echo_se_retrouve_par_les_echos_les_plus_courts`, `une_ligne_de_trace_se_relit`, `le_segment_fautif_se_nomme_au_dela_du_budget`.
- **Le contrôle négatif de `mesures`**, joué à chaque passage avec des entrées de l'écran : le dessin ralenti de 33 ms se voit (33,17 ms en médiane au lieu de 0,18 dans Chrome écran allumé ; 33,15 au lieu de 0,82 écran éteint ; 33,28 au lieu de 1,56 dans Firefox).

### Les mesures

Dans `MESURES-PHASE-3.md`, « Lot 13 ». Toutes par `target/hote/lourd --calme`, une à la fois ; la règle de validité posée avant chaque campagne (`target/travail/lot-13/campagne.sh`, son journal `campagne-journal.txt`) : un passage ne vaut mesure que si la charge la plus haute pendant, écrite par le portillon, reste sous 3 ; sinon il est écarté (`<nom>-ecarte-<k>`) et refait, quatre essais au plus. À la reprise, `mediaanalysisd` de macOS a tenu la machine entre 4 et 66 de charge de 19 h 00 à 20 h 00 environ : la campagne a attendu, puis a été arrêtée pour passer les vérifications finales (qui n'ont pas besoin du calme), puis relancée.

- **M2**, sans interface, écran allumé : au clavier, 34,5 et 35,5 ms au 99ᵉ centile dans Chrome (60 Hz), 32,9 et 35,3 ms dans Firefox (120 Hz), sans glisser ; ~~49,7 à 51,1 ms avec glisser forcé (l'attente de l'image pesant le plus) ; au téléphone natif, 30,5 à 42,0 ms sans glisser, 63 à 70 avec~~ (*faux, corrigé après la vérification* : aucun nœud ne glissait ; voir « Lot 13, correction après la vérification »). **Avec la fenêtre** (120 Hz dans les deux) : Chrome 28,5 et 32,5 ms sans glisser, ~~45,5 et 46,7 avec~~ ; **Firefox 124 à 137 ms, le segment du GPU pesant le plus** (de la soumission à `onSubmittedWorkDone`, 84 à 110 ms au 99ᵉ centile), sans qu'on sache si l'image est à l'écran si tard : nommé dans `RESTE-PHASE-3.md` et au mainteneur. Écran éteint (le premier agent), Chrome à 30 Hz : 69 à 73 ms. Sans calage, rien de plus lent qu'avec (la minuterie de secours du lot 10a joue déjà le pas avant une image tardive) : une donnée pour P6. Le contrôle négatif se voit à chaque passage.
- **M14** (avant tout découpage, dans chaque passage de M2) : la page et son Worker à 0,00–0,02 ms dans les deux navigateurs ; les processus de Chrome à 0,4 ms au plus ; ceux de Firefox s'écartent de 0,3 à 2,7 ms, autrement à chaque lancement. Aucun segment de M2 ne franchit deux processus de navigateur ; la trace d'un natif est à son demi-aller-retour près (1,5 ms au plus).
- **M6** (avec interface, écran allumé, 480 s chacun, au calme) : **recouverte**, la fenêtre reste `visible` et reçoit ses images dans Chrome et Firefox, réussi dans les deux ; **réduite**, Chrome reste `visible` et reçoit ses images (la réduction par CDP n'est pas constatée), réussi ; Firefox passe à `hidden`, la minuterie seule joue, et **un saut d'un pas** juste après la réduction, après un réveil revenu au bout de 33,0 ms (le cas de la question 30 ; une seconde série, sous la même règle, après quatre passages écartés pour la charge). Aucun signe d'App Nap ; le seuil des « 100 ms trois fois de suite » n'est franchi que par le retard injecté. Écran éteint (le premier agent), Chrome recouverte ne recevait plus d'images et Firefox a sauté un pas sans cause relevée.
- **M1** (huit tours du harnais, sept du banc valides ; *corrigé après la vérification* : sept du harnais, le tour 1 écarté, sa charge de fin à 3,08, la médiane du harnais dans Chrome à 36 à 39 ; le tour 1 du banc, sept essais tous écartés), en ps : Chrome, harnais 35 à 39 ; 56 à 78 ; 67 à 136, banc 28 à 37 ; 47 à 65 ; 57 à 493 ; Firefox, harnais 64 à 70 ; 106 à 135 ; 120 à 318, banc 62 à 66 ; 98 à 123 ; 217 à 592. Aucun écart. Médianes et 99ᵉ centiles du banc dans l'étendue du harnais ou au-dessous ; les maxima ne se retrouvent pas (Tier list dans Chrome, Punchline dans Firefox, comme au lot 3c). Ajouté à la question 27, non tranchée.
- **`arriere-plan` du lot 10a** (headless, écran éteint ; le premier agent, 07 h 02 à 07 h 39, chaque passage seul sous `lourd --calme`, la règle posée avant) : aucun passage écarté (charge la plus haute pendant de 2,14 à 2,98) ; Chrome trois réussis sur trois ; Firefox deux sur trois, le troisième avec un saut d'un pas (4482, 4483) au tour qui suit un réveil de la minuterie revenu après 33,2 ms, le tour lui-même de 0,5 ms : **le point du lot 10a n'est pas tenu dans Firefox** ; les chiffres sont à la question 30, non tranchée. Les passages ont été lancés un par un (`--fois` 1, trois fois par navigateur) et non par `--fois 3` : chacun sous sa propre règle de charge, ce qui est plus strict.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-13/<nom>/` : `commande.txt`, `sortie.txt` (complète), `code.txt` (code, durée, fin), `empreinte.txt` (et, à la reprise, `empreinte-fin.txt`). **L'arbre de code des preuves finales** : `soiree` `HEAD` `cf61d0f`, sha256 de `git diff HEAD` hors `.md` **`b8d4ea0c…`** (le même que le `diff HEAD` entier des plans courts du premier agent à 06 h 33 : aucune ligne de code suivie n'a changé depuis), des fichiers non suivis hors `.md` **`bb9dab47…`** ; les trois jeux sans changement (`HEAD` `72d3710`, `1dc4a25`, `202f49a` ; *corrigé après la vérification* : `9827f39` était écrit pour Tier list, son `HEAD` est `202f49a` dans toutes les empreintes) ; paquet de la page `755ccba5…` (construit à 06 h 49, après la dernière modification de `web/`, à 06 h 49 min 04 s) ; `soiree-banc` (`target/release`, 06 h 31) sha256 `326d2e75…`.

| Preuve | Commande | Résultat |
|---|---|---|
| `test-workspace-final` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0** : 745 réussis, 0 échec, 1 ignoré (`plan_standard::chaque_fautif_sur_le_plan_standard`, comme avant) ; 574 s (sous charge, pas une mesure) |
| `clippy-commun-final` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 |
| `clippy-web-natif-final`, `clippy-web-wasm-final` | `lourd zsh -c "cd web && cargo clippy --all-targets [--target wasm32-unknown-unknown] -- -D warnings"` | code 0, 0 |
| `test-web-final` | `lourd zsh -c "cd web && cargo test"` | code 0 (dont `glisser` 3, `composants` 11, `pixels` 12) |
| `fmt-final` | `cargo fmt --all --check`, puis dans `web/` | code 0 |
| `regles-web-final` | `lourd target/release/soiree-banc regles-web` | code 0 : mêmes lints, même `clippy.toml`, même toolchain |
| `court-sidequest`, `court-punchline`, `court-tier-list` (premier agent, 06 h 33 à 06 h 39, arbre de code suivi identique au final) | `lourd /usr/bin/time -l target/release/soiree-epreuve ../games/<jeu> --court --json …` | **la référence du lot 7b** : Sidequest 0 ; Punchline 1, E2 seul (« une fin, mais aucune autre après la nouvelle partie ») ; Tier list 0 ; E16 et E21 réussis (Tier list : E21 sans objet) |
| `ecran-tier-list` (premier agent, 06 h 50, le paquet final) | `lourd target/release/soiree-banc ecran tier-list <module> --navigateurs chrome,firefox --sans-webgl2 --plein 0 --sans-construire` | code 0 : Chrome et Firefox réussis (le scénario du lot 11 sur la page changée) |
| `m2-ardoise-chrome-ecran-allume`, `m2-punchline-chrome-ecran-allume` | `lourd --calme caffeinate -d target/release/soiree-banc mesures <module> --navigateurs chrome --entrees 150 --duree 20 --echos 200 --sans-construire` (`--entrees 0 --duree 30` pour Punchline) | code 0 ; charge au plus 2,67 et 2,56 (trois passages d'`ardoise` écartés avant : 3,27, 4,16, 3,99) |
| `m2-ardoise-chrome-interface`, `m2-ardoise-firefox-interface` | la même, `--avec-interface` | code 0, 0 ; charge au plus 2,82 et 2,23 |
| `m2-ardoise-firefox`, `m2-punchline-firefox` (premier agent, écran allumé) ; `m2-ardoise-chrome`, `m2-punchline-chrome` (premier agent, écran éteint) | la même, sans `caffeinate` | code 0 ; charge au plus 2,50, 2,14, 2,34, 2,60 (un `m2-punchline-firefox` écarté : 3,30) |
| `m6-recouverte-chrome`, `m6-recouverte-firefox`, `m6-reduite-chrome`, `m6-reduite-firefox` | `lourd --calme caffeinate -d target/release/soiree-banc arriere-plan <punchline> --navigateurs <n> --cache 480 --avec-interface --mode <mode> --sans-construire` | 0, 0, 0, **1** (le saut de Firefox réduite) ; charge au plus 2,64, 2,43, 2,92, 2,15 ; écartés : 1, 2 et 4 passages |
| `m6-recouverte-<n>-ecran-eteint` (premier agent) | la même, sans `caffeinate`, écran éteint | 0 et 1 ; un constat, pas M6 |
| `m1-harnais-<1…8>`, `m1-banc-<2…8>` | `lourd --calme zsh -c "cd target/travail/lot-3c/harnais/mesures/navigateur && node mesurer.mjs --navigateurs chrome,firefox --tour <10+n>"` ; `lourd --calme target/release/soiree-banc etalonner --mesures m1 --navigateurs chrome,firefox --sans-construire` | code 0 chacun ; charge au plus 2,30 à 2,93 ; 26 essais écartés pour la charge et un interrompu par l'arrêt du premier agent (`-ecarte-<k>`) |
| `ap-chrome-<1…3>`, `ap-firefox-<1…3>` (premier agent, écran éteint) | `lourd --calme target/release/soiree-banc arriere-plan <punchline> --navigateurs <n> --cache 360 --sans-construire` | Chrome 0, 0, 0 ; Firefox 0, 0, **1** ; charge au plus 2,14 à 2,98, aucun écarté |

Les preuves du premier agent qui précèdent ses corrections (`*-avant-correction`), ses essais (`target/travail/lot-13/essai*`), et ses premières vérifications finales (`test-workspace`, `clippy-*`, `fmt`, `test-web`, `regles-web`, `build-outils`, toutes à code 0) sont gardées ; les vérifications de la reprise les remplacent. Les tableaux de M2, segment par segment : `target/travail/lot-13/tableau-m2-*.md`.

### Ce qui n'est pas fait, et pourquoi

- **Safari** : pas exécuté (automatisation à distance non permise ; réglage non touché).
- **Le tour 1 du banc de M1** : aucun essai valide sur sept (la règle de charge) ; M1 a donc sept tours du banc, et non huit.
- **La réduction d'une fenêtre de Chrome** n'est pas constatée par la page (elle reste `visible`, reçoit ses images) : le banc ne relit pas l'état de la fenêtre ; ce que Chrome fait d'une fenêtre vraiment réduite reste à voir, à l'œil, à la répétition.
- **M2 avec interface** n'a été joué que sur `ardoise` (le clavier et la souris), pas avec les téléphones de Punchline.
- **Le point du lot 10a (`arriere-plan` vert dans Firefox)** n'est pas tenu : question 30, non tranchée ; ni le contrôle ni le rattrapage des jeux ne changent.
- **Le panneau en barres** : du texte ; les barres ne sont pas faites (le texte dit les mêmes centiles).
- **Le relais n'horodate rien** (`Trace::relais` reste `None`) : le transport d'un téléphone n'est pas découpé ; le faire changerait `liaison`.
- **La souris et les téléphones natifs n'ont pas de segment « événement »** (l'événement de la souris passe par la file de la page ; un natif n'a pas de DOM).

### Ce que le lot suivant doit savoir

- **Une mesure de Chrome se prend écran allumé** (`caffeinate -d`), et dit l'état de l'écran (`pmset -g log`) : écran éteint, Chrome sans interface tombe à 30 images par seconde, et toute latence de la télé double presque.
- Le lot 14 finit `RESTE-PHASE-3.md` : le point de la latence y est écrit (le budget tenu dans Chrome, glisser compris, et dans Firefox sans interface ; Firefox avec sa fenêtre au-dessus, le segment du GPU nommé ; deux questions au mainteneur).
- **Le pixel de Firefox avec sa fenêtre** (WebGPU) se dit par `onSubmittedWorkDone`, qui arrive 84 à 110 ms après la soumission au 99ᵉ centile : avant d'en conclure quoi que ce soit, regarder à la caméra (la fiche de répétition du lot 14) ; question au mainteneur dans `RESTE-PHASE-3.md`.
- **La campagne** : chaque passage par `lourd --calme`, un à la fois ; une machine que l'hôte occupe (ici `mediaanalysisd` de macOS, puis d'autres travaux) peut tenir la charge au-dessus de 3 pendant une heure, et un passage de M1 (cinq modules dans deux navigateurs) monte souvent la charge de lui-même au-dessus de 3 : compter plusieurs essais par tour.
- Les scripts : `target/travail/lot-13/campagne.sh` (étapes `m2`, `m2c`, `m2i`, `ap`, `m6`, `m1`), `preuve.sh`, `tableau.py`, `comparer2.py`.

## Lot 13, correction après la vérification

Fait le 2026-11-08, de 22 h 20 à 23 h 10 environ (UTC), sur la même machine (MacBook Apple M5, 10 cœurs, 16 Go, macOS 27.2 ; Chrome 156.0.8078.80, Firefox 158.0.1 ; Safari pas exécuté), dans `soiree/` seulement (branche `refonte`, `HEAD` `cf61d0f`, rien de commité) ; les jeux ne changent pas (`HEAD` `72d3710`, `1dc4a25`, `202f49a`, propres ; modules au sha256 du lot 7b). Chaque constat de la vérification a été vérifié avant d'être suivi.

### Ce qui est corrigé

1. **[important] Glisser retardait le pixel de toute scène** (vérifié : `chemin()` rendait `Some` après chaque scène neuve dès que `glisse()` était vrai, `a_des_noeuds_qui_glissent` n'était appelé par rien, et `ardoise` et Punchline ne marquent aucun nœud `glisse` ; seul Sidequest en marque). Corrigé :
   - `rendu::glisser::deplace_des_noeuds(precedente, derniere)` : un nœud qui glisse dans la dernière, de même clé et de même sorte dans la précédente, à une autre position ; la page ne garde la scène d'avant (et ne glisse) que si c'est vrai, sinon elle dessine la dernière en entier tout de suite (`Mesure::scene_a_l_ecran`).
   - **La définition du pixel avec glisser** (§ 8.4, corrigé) : la première image où chaque nœud est au moins à sa place du pas de l'entrée, c'est-à-dire la scène dessinée en entier, ou une image d'un chemin qui part de sa scène ou d'une plus récente (`Mesure::dessine` reçoit le pas d'où part le chemin). Sans cela, sur Sidequest, où les scènes changent à chaque pas, le chemin d'un pas n'est presque jamais fini avant que le suivant commence, et les traces auraient attendu sans fin (vu en lisant le code, avant de mesurer).
   - La page compte les scènes où un nœud a glissé et les images à mi-chemin (`window.__banc.glisses()`, et la ligne « glisser » du panneau) ; `soiree-banc mesures` les lit avant et après chaque condition et le dit au rapport (`mesures::note_de_glisser` : « glisser n'a rien déplacé, la condition vaut « sans glisser » »). Une condition « avec glisser » qui ne glisse pas ne peut plus passer pour un coût de glisser.
   - **Mesuré de nouveau** (`MESURES-PHASE-3.md`, « Lot 13 », « M2 corrigée : glisser ») : un vrai glisser sur Sidequest (les gestes de ses six téléphones natifs), dans Chrome et Firefox, sans interface et avec ; et `ardoise` sur la page corrigée, où « avec glisser » vaut désormais « sans glisser ». Les écrits faux sont barrés et corrigés : `MESURES-PHASE-3.md` (l'avertissement en tête de M2, la puce « Glisser coûte un pas… », les chiffres « avec glisser » des conclusions), `RESTE-PHASE-3.md` (la question sur glisser retirée, le point de glisser réécrit), PHASE-3 (§ 2.9 « Tier list ne glisse pas » : vrai désormais pour la trace aussi ; § 8.4 ; § 8.6 M2 ; la ligne du lot), `CHANGELOG.md`, `AGENTS.md`, `crates/README.md`, et la section « Lot 13 » plus haut.
2. **[mineur] M1 : le tour 1 du harnais est écarté** (vérifié : départ 2,87, fin 3,08, « la plus haute pendant : 2.87 » ; le portillon relève toutes les 10 s et ne compte pas la valeur de la fin). Selon l'esprit de la règle, la moyenne d'une minute a atteint 3 pendant le tour. Aucun autre tour gardé (M1, M2, M6, `arriere-plan`) n'a une charge de fin de 3 ou plus (relevé dans chaque `sortie.txt`). Sept tours du harnais au lieu de huit ; seule la médiane de Chrome change, 36 à 39 au lieu de 35 à 39 (`target/travail/lot-13-corr/comparer3.py`, sa sortie `comparer3-sortie.txt`). La campagne de la correction compte la charge de fin dans sa règle, et `AGENTS.md` le dit. Le portillon (`target/hote/lourd`, hors de tout dépôt) n'est pas changé.
3. **[mineur] `HEAD` de Tier list** : `202f49a`, et non `9827f39` ; corrigé dans la section « Lot 13 ».
4. **[mineur] L'empreinte des fichiers non suivis du premier agent** : vérifié, la reprise s'était trompée ; le premier `preuve.sh` hachait juste pour `soiree` (`2985e2dd…` aux plans courts, `bb9dab47…` ensuite) et le vide des jeux est juste (ils n'ont rien de non suivi). Corrigé dans la section « Lot 13 ».
5. **[mineur] Le contrôle négatif** : son jugement est sorti dans `mesures::juger_le_controle`, et le chemin « CONTRÔLE NÉGATIF RATÉ » est tenu par un test (`le_controle_negatif_rate_quand_le_ralenti_ne_se_voit_pas` : retard trop petit, nul, inversé, une médiane qui manque). Ce que le contrôle ne prouve pas reste dit : il ne regarde que le segment du dessin, encadré directement par l'attente active, et ne prouve pas que la trace se rattache à la bonne image ; ce que glisser ajoute (un pas d'attente de l'image sur Sidequest, rien sur `ardoise`) en est un indice, pas une preuve. Le panneau reste du texte (non fait, dit au lot 13).
6. **[mineur] M14 avant tout découpage** : les échos se mesurent désormais avant les quatre conditions, dans `passage()`. `Trace::incertitude_us` ne porte que le demi-aller-retour d'un téléphone : le commentaire de `salle::ecran` et § 8.4 sont corrigés, et le rapport donne l'écart de M14 à côté des segments qu'il touche (une note : « les segments entre la page et son Worker … portent en plus l'écart de M14 »). Mesuré : 0,00 à 0,01 ms.
7. **[mineur] Les gestes qui attendent 1,3 à 1,9 s** au début de la partie (vérifié : pas 17 à 47 de `m2-punchline-chrome-ecran-allume`, `lignes.txt` lignes 47 à 52, tous rattachés à la même image) : écrit dans `MESURES-PHASE-3.md` et `RESTE-PHASE-3.md`, avec la cause probable (la transition depuis l'écran du commun), non établie ; vu aussi sur Sidequest à l'essai (717 à 985 ms au 99ᵉ centile du panneau).
8. **[mineur] Les plans courts** : vérifié, `soiree-epreuve` ne dépend ni de `salle`, ni de `banc`, ni de `web/` (son `Cargo.toml`) ; la correction ne touche que `banc`, `web/` et un commentaire de `salle::ecran`. La référence des plans courts (Sidequest 0, Punchline 1 E2 seul, Tier list 0) tient, et ils ne sont pas relancés.

**Écarté** : aucun constat.

**Vu en chemin, non corrigé** : avec la fenêtre de Chrome, le plein écran est quitté aussitôt entré dans les trois passages de la correction (22 h 37 à 22 h 58), alors qu'il tenait à la reprise (22 h 03) ; le voile du commun couvre alors le jeu, et les clics sur « Effacer » d'`ardoise` n'y arrivent pas (aucune trace de la souris, l'ardoise se remplit, des touches sans pixel). Cause non trouvée ; la page n'a pas changé sur ce point. Dit dans `MESURES-PHASE-3.md`.

### Les tests qui le tiennent

- `web/rendu/tests/glisser.rs` : `glisser_ne_retarde_que_les_scenes_ou_un_noeud_se_deplace` (nouveau ; TL-23 pour la rupture), et les trois d'avant.
- `banc::mesures::tests` : `le_controle_negatif_rate_quand_le_ralenti_ne_se_voit_pas` et `une_condition_avec_glisser_sans_rien_qui_glisse_se_dit` (nouveaux), et les trois d'avant.
- Le rapport de `mesures` lui-même : dans chaque passage refait, « aucune scène où un nœud glisse » pour `ardoise`, de 1 233 à 1 642 scènes glissées par condition « avec glisser » pour Sidequest, aucune sans glisser.

### Les mesures

`MESURES-PHASE-3.md`, « Lot 13 », « M2 corrigée : glisser » ; chaque passage par `lourd --calme caffeinate -d`, la règle posée avant les tours (la charge la plus haute pendant et la charge de fin sous 3 ; `target/travail/lot-13-corr/campagne.sh`, son journal `campagne-journal.txt`) ; aucun passage écarté pour la charge. **Sidequest, au téléphone, au 99ᵉ centile** : Chrome sans interface (60 images/s) 40 à 42 ms sans glisser, 50 et 55 avec ; Firefox sans interface 43 et 44, 58 et 60 ; Chrome avec sa fenêtre 34 et 36, 49 et 51 ; Firefox avec sa fenêtre 72 et 77, 70 et **80,2** (le budget du téléphone dépassé d'un rien dans une condition, le GPU nommé, 65 ms). Glisser coûte un pas d'attente de l'image (14 à 17 ms en médiane à 120 Hz). **`ardoise`, au clavier** : « avec glisser » dans l'étendue de « sans glisser » (Chrome 36,8 à 38,2 ms ; Firefox 37,9 à 42,4 ; Chrome avec sa fenêtre 29,1 à 33,6). **M1** : sept tours de chaque outil ; Chrome, harnais 36 à 39 en médiane.

### Les vérifications, et l'index des preuves

Dans `target/preuves/lot-13-corr/<nom>/` : `commande.txt`, `sortie.txt`, `code.txt`, `empreinte.txt` et `empreinte-fin.txt` (`target/travail/lot-13-corr/preuve.sh`).

**L'arbre de code des preuves finales** : `soiree` `HEAD` `cf61d0f`, sha256 de `git diff HEAD` hors `.md` **`61ed9bec…`**, des fichiers non suivis hors `.md` **`0738e1da…`** ; les jeux propres. Les mesures ont été prises sur l'arbre d'avant deux retouches demandées par clippy (un alias de type dans `banc::mesures` et dans `page/mesure.rs`, une variable renommée dans `page.rs` ; `cdbd78b2…` et `bdad099d…`), **avec les mêmes binaires** : le paquet de la page reconstruit après ces retouches a la même empreinte (`target/soiree-web/5d4fb859…`, reconstruit à 23 h 00), et `soiree-banc` le même sha256 (`4955d793…`, reconstruit par `build-banc-final`).

| Preuve | Commande | Résultat |
|---|---|---|
| `test-workspace-final` | `lourd /usr/bin/time -l cargo test --workspace --no-fail-fast` | **code 0** : 118 binaires, 747 réussis (745 au lot 13, plus les deux de `mesures`), 0 échec, 1 ignoré (`plan_standard`, comme avant) ; 437 s (pas une mesure) |
| `clippy-commun-final` | `lourd cargo clippy --workspace --all-targets -- -D warnings` | code 0 (un premier passage rouge, `type_complexity`, gardé sous `-rouge-1`, corrigé par un alias) |
| `clippy-web-natif-final`, `clippy-web-wasm-final` | `lourd zsh -c "cd web && cargo clippy --all-targets [--target wasm32-unknown-unknown] -- -D warnings"` | code 0, 0 (le wasm rouge une première fois, `similar_names` et `type_complexity`, gardé sous `-rouge-1`) |
| `test-web-final` | `lourd zsh -c "cd web && cargo test"` | code 0 (`glisser` 4) |
| `fmt-final` | `cargo fmt --all --check`, puis dans `web/` | code 0 |
| `build-banc-final`, `regles-web-final` | `lourd cargo build --release -p banc --bin soiree-banc` ; `lourd target/release/soiree-banc regles-web` | code 0, 0 |
| `ecran-tier-list` | `lourd target/release/soiree-banc ecran tier-list <tier_list_module.wasm> --navigateurs chrome,firefox --sans-webgl2 --plein 0` (le paquet reconstruit) | code 0 : Chrome et Firefox réussis (pas une mesure ; charge 4,40 pendant) |
| `m2-sidequest-chrome`, `m2-sidequest-firefox`, `m2-sidequest-chrome-interface`, `m2-sidequest-firefox-interface` | `lourd --calme caffeinate -d target/release/soiree-banc mesures <sidequest_module.wasm> --navigateurs <n> --entrees 0 --duree 30 --echos 200 --sans-construire [--avec-interface]` | code 0 chacun ; charge (départ ; fin ; la plus haute pendant) 1,78 ; 1,65 ; 2,36 — 1,65 ; 2,20 ; 2,69 — 2,18 ; 2,09 ; 2,18 — 2,09 ; 2,63 ; 2,91 |
| `m2-ardoise-chrome`, `m2-ardoise-firefox`, `m2-ardoise-chrome-interface` | la même sur `ardoise_module.wasm`, `--entrees 150 --duree 20` | code 0 chacun ; 2,63 ; 1,09 ; 2,63 — 1,09 ; 1,44 ; 1,55 — 2,58 ; 1,69 ; 2,58 |
| `m2-ardoise-chrome-interface-plein-ecran-quitte` | la même, le premier passage | code 0 ; 1,44 ; 1,86 ; 1,97 ; gardé comme constat (le plein écran quitté, la souris sans trace), refait une fois |
| `build-banc` | `lourd cargo build --release -p banc --bin soiree-banc` (avant les mesures) | code 0 |

L'essai sur Sidequest, avant la campagne, hors règle de calme (`lourd caffeinate -d … --duree 15 --echos 50`, sans `--sans-construire`, qui a construit le paquet) : `target/travail/lot-13-corr/essai-sidequest.txt`, pas une mesure. M1 recalculée : `python3 -I target/travail/lot-13-corr/comparer3.py`. Les tableaux : `target/travail/lot-13-corr/tableau-m2-*.md` (`tableau.py`).

### Ce qui n'est pas fait, et pourquoi

- Safari : pas exécuté (automatisation à distance non permise ; réglage non touché).
- **Le coût de glisser au clavier** : aucun module ne glisse sous une touche (`ardoise` n'a aucun nœud `glisse`, Sidequest se joue aux téléphones) ; il n'est pas mesuré. Le changer demanderait un module de test qui glisse au clavier, hors de cette correction.
- **La souris avec la fenêtre de Chrome** n'a pas de chiffres sur la page corrigée : le plein écran y a été quitté aussitôt entré (cause non trouvée), et les clics sont allés au voile du commun. Le clavier y est mesuré.
- Le panneau en barres, `Trace::relais`, le segment « événement » de la souris et des natifs : comme au lot 13.
- La cause des gestes qui attendent 1,3 à 1,9 s après « Jouer » : probable, non établie (lot 14).
- `soiree status` n'est pas lancé (`soiree/AGENTS.md` interdit la commande `soiree` à l'agent) ; `git status` des quatre dépôts relevé à la place.

### Ce que le lot suivant doit savoir

- **Glisser ne joue que si un nœud se déplace** : sur `ardoise`, Punchline et Tier list, « avec glisser » vaut « sans glisser », et le rapport de `mesures` le dit. Un vrai glisser se mesure sur Sidequest (`--entrees 0`).
- **La règle de validité d'une mesure compte la charge de fin** : le portillon écrit « la plus haute pendant » sans la valeur de la fin (`target/travail/lot-13-corr/campagne.sh` l'applique ; `AGENTS.md` le dit).
- Avec la fenêtre de Chrome, regarder si le plein écran tient (les lignes `plein-ecran-quitte` du `lignes.txt`) avant de lire les chiffres de la souris.
- Le lot 14 (`RESTE-PHASE-3.md`) : la question sur glisser est retirée ; restent `onSubmittedWorkDone` dans Firefox avec sa fenêtre (le GPU, au clavier et au téléphone) et les premiers gestes après « Jouer ».
