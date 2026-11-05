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
