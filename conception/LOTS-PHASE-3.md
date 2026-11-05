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
