# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## Non publié — la refonte, phases 1 et 2

Le commun repart de zéro, d'après [`MANIFESTE.md`](MANIFESTE.md), [`CONTRAT.md`](CONTRAT.md) et [`conception/INTERFACE.md`](conception/INTERFACE.md). L'ancien commun est retiré : salon, la télécommande iPhone (`remote/`), le modèle de jeu (`modele/`) et le harnais de certification (`tools/cert/`). Il reste lisible à l'étiquette `avant-refonte`. À sa place, un workspace de crates dans `crates/` : un jeu s'écrit avec `jeu`, se compile en un module WebAssembly qui n'importe rien, est réécrit (`reecriture`), chargé dans wasmtime (`moteur-wasmtime`), joué par le cœur de la partie qui fait foi (`partie`) sur une horloge simulée, et éprouvé (`epreuve`, contrôles E1 à E19, chacun avec son jeu fautif). Il n'y a encore ni navigateur, ni relais, ni rendu, ni prédiction sur appareil, ni outil `soiree` réécrit : un jeu ne se joue pas encore, il s'éprouve. Aucune étiquette n'est posée ; les étiquettes `v0.x` restent celles de l'ancien commun.

Les trois jeux de la soirée sont reconstruits sur ce commun, dans leur branche `refonte`, et en dépendent par chemin. Ce que chacun a demandé au commun, et la réponse, est dans [`conception/BESOINS-DES-JEUX.md`](conception/BESOINS-DES-JEUX.md) ; ce qui a été mesuré sur eux, dans [`conception/MESURES-PHASE-1.md`](conception/MESURES-PHASE-1.md).

### Passer à cette version

Aucune API de l'ancien commun ne survit : salon, `DeviceEvent`, `soiree.toml`, `cert.json` et `verification.toml` n'existent plus. Un jeu se réécrit sur la crate `jeu`.

1. Lis `conception/INTERFACE.md`, § 2 (l'API d'un jeu) et § 9 (`devine`, un jeu complet), puis `crates/exemples/devine/`.
2. Fais deux crates : les règles (`#![no_std]`, `#![forbid(unsafe_code)]`, `impl jeu::Jeu`) et la crate de module (`jeu::module!(<ton jeu>);` et rien d'autre qu'un attribut `no_std`, `crate-type = ["cdylib"]`, qui ne dépend que de `jeu` et des règles). Reprends les `[workspace.lints]`, `clippy.toml`, `[profile.release]`, `[profile.module]` et `[profile.dev.package."*"]` de ce dépôt, sans rien assouplir : l'épreuve les compare. Une crate d'épreuve écrite à la main n'est plus nécessaire.
3. Pendant la refonte, dépends du commun par chemin, depuis le dossier de l'hôte : `jeu = { path = "../../soiree/crates/jeu" }`, et de même pour `jeu-abi`, `jeu-natif`, `partie`, `epreuve`, `moteur-wasmtime`. Il n'y a pas encore d'étiquette à nommer, ni de `soiree upgrade`.
4. Éprouve le jeu depuis ses sources, depuis `soiree/` :
   ```sh
   cargo run --release -p epreuve --bin soiree-epreuve -- ../games/<jeu>
   ```
   L'épreuve vérifie les règles d'écriture, lance clippy, compile le module et la copie native (écrite hors du dépôt du jeu, dans son dossier de compilation), puis joue. Le jeu passe quand le code de sortie vaut 0, sur le plan standard : chaque étape exigée du CONTRAT faite, chaque contrôle réussi ou sans objet. `soiree-epreuve --wasm <module.wasm>` éprouve un module seul, mais ne rend jamais 0 : l'étape 1 n'y est pas faite. Ne lance pas `soiree check` ni `soiree certify` : l'outil n'est pas réécrit.

### Phase 3, lot 2 : le moteur en deux

Rien ne change pour l'API d'un jeu ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes : le même module réécrit donne, sur la soirée de `devine`, les mêmes empreintes avant et après (`conception/PHASE-3.md`, § 2.3, lot 2 ; `conception/LOTS-PHASE-3.md`, « Lot 2 »).

- **Une crate de plus, `moteur-commun`** (P2) : le protocole d'un appel, une seule fois pour tous les moteurs. Le trait `Machine` (les primitives d'un moteur : lire, écrire et remettre à zéro la mémoire, la faire grandir, lire et poser une globale par son nom, appeler `soiree_boite` et `soiree_appel`, une instance neuve) et le piège brut `Piege` ; `Protocole<M: Machine>`, une `partie::Instance` ; le classement d'un piège, dans l'ordre d'avant (tenu par `classer_garde_son_ordre`) ; l'empreinte ; `verifier_chargement` (`Bonjour`, zones, `Decrire`, déclaration) ; `RefusModule` ; le module d'essai `essais::BONJOUR_QUI_BOUCLE`. `no_std`, sans entrées-sorties.
- **`moteur-wasmtime` est réduit à `impl Machine` sur wasmtime**, l'époque du garde-fou et le fil de chaque instance. Son API ne change pas : `Moteur`, `ReglagesMoteur`, `ModuleCharge`, `InstanceWasm`, `FabriqueWasm`, `InstantaneWasm` (désormais un alias de `moteur_commun::Instantane`), `RefusModule` et `BUDGET_DE_CHARGEMENT` (réexportés de `moteur-commun`). Nouveaux : `ModuleCharge::carte` et `ModuleCharge::carburant_du_chargement`, l'exemple `boucle_vide` (M10). Le garde-fou mural borne toujours un appel en entier, `soiree_boite` et `soiree_appel` ensemble : son échéance se pose une fois, dans `Machine::boite`.
- **L'instantané ne garde plus que la mémoire hors des quatre zones** (P3) : la pile, la boîte, le brouillon et la zone de panique, remises à zéro avant chaque appel et exclues de l'empreinte, ne sont plus copiées, et la restauration les remet à zéro. Chez Tier list, un plateau neuf garde 323 584 octets au lieu de 25,4 Mio ; le pire cas, 16,8 Mio (M3). Sa forme gardée change : **`partie::persistance::VERSION_GARDE` vaut 2** ; une garde de la version 1 est refusée (`RefusGarde::Version`). Les octets figés de l'instantané sont dans `crates/moteur-commun/src/instantane.rs` ; ceux des types de `partie` n'ont pas changé.
- **`BUDGET_DE_CHARGEMENT` passe de 2³² à 2²⁶** (P5 ; RESTE-PHASE-2, point 16) : une boucle dans `Bonjour` ou `Decrire` est la faute `Budget` du jeu sous le garde-fou d'une seconde de `ReglagesMoteur::PHASE_1`, même sur une machine chargée, au lieu d'une panne de l'outil. Mesuré : `Bonjour` et `Decrire` des cinq modules brûlent au plus 9 322 unités.
- `epreuve` compte `moteur-commun` parmi les crates du commun (étape 1, `sources.rs`).

#### Passer à cette version

1. Un jeu n'a rien à faire dans son code. Son `Cargo.lock` gagne `moteur-commun` (que `moteur-wasmtime`, pris par chemin, tire désormais) : Cargo l'écrit à la première compilation ; c'est un changement à committer dans le dépôt du jeu.
2. Un `Bonjour` ou un `Decrire` qui brûlerait plus de 2²⁶ unités est refusé (faute `Budget`). Aucun des cinq modules mesurés n'en approche (au plus 9 322).
3. Du code qui lisait les champs de `InstantaneWasm` (aucun hors du moteur) lit `pages()`, `plages()`, `octets()` et `globales()` de `moteur_commun::Instantane`. Une garde écrite avant ce lot ne se relit plus.
4. Changer la forme de l'instantané monte `VERSION_GARDE` et réécrit à la main les octets figés de `crates/moteur-commun/src/instantane.rs` (et non plus de `moteur-wasmtime/src/interieur.rs`, retiré).

### Phase 3, lot 1 : le cœur prêt pour le monde

Rien ne change pour l'API d'un jeu ni pour l'ABI (`jeu_abi::ABI` vaut 3). Ce qui change dans `partie` et dans ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.4, § 2.11, lot 1 ; `conception/LOTS-PHASE-3.md`, « Lot 1 ») :

- **Un pas trop chargé d'arriérés se finit sur plusieurs tours** (P7). `Partie::avancer_borne(cible, arrieres_par_tour)` joue au plus `arrieres_par_tour` entrées d'un arriéré (un geste ou une entrée de l'écran faits sur une vue de plus de `partie::ARRIERE_PAS` pas avant leur réception) ; au-delà, le pas reste en cours (`Avancee::en_cours`, `Partie::pas_en_cours`) et se finit au tour suivant. Les arrivées d'un pas se figent au tour où il commence, et celles d'une avancée au tour où elle commence : une arrivée reçue pendant une avancée inachevée va après sa fin (son dernier pas, puis son saut), comme si l'avancée avait fini d'un seul tour. Le journal ne dépend pas du découpage, ni les vues de l'écran : avec une fenêtre, la tête juge à la fin de l'avancée les arrivées reçues pendant elle (CAT-14). `Partie::avancer` finit toujours d'un seul tour, comme avant. Nouveaux : `partie::ARRIERE_PAS` (60), `partie::ARRIERES_PAR_TOUR` (8), `Partie::avancer_decoupe` (pour éprouver le découpage), `Partie::finir_le_pas`, `Partie::fin_du_dernier_pas`, et le champ `Avancee::en_cours`.
- **Le journal a une base d'indices absolus** (P10) : `Journal::base`, `Journal::fin`, `Journal::ligne`, `Journal::tronquer_avant`, `Partie::tronquer_journal_avant` (jamais au-delà du dernier point). `Journal::lignes` ne rend que les lignes gardées, la première d'indice `base`. `Partie::reprendre` accepte un journal tronqué ; un point avant sa base est refusé (`RefusReprise::PointHorsJournal`, dont le sens s'élargit).
- **La forme de la garde** : `partie::persistance` encode et relit un point de reprise (sa chaîne `sain` et `periodique` aplatie et dédoublonnée), le journal ou un morceau de journal, l'ouverture et l'état du cœur, derrière un en-tête de cinq octets (`VERSION_GARDE` en `u32` petit-boutiste, puis la sorte). L'instantané s'écrit par le trait `persistance::InstantaneGarde`, implémenté pour `moteur_wasmtime::InstantaneWasm` et le modèle des tests. Les types gardés de `partie` (lignes du journal, `EtatCoeur`, `Ouverture`, `Budgets`, `Faute`) dérivent `Serialize` et `Deserialize`. `PointDeReprise::sain` et `periodique` se lisent.
- **Une seule définition des budgets de la soirée** : `partie::BUDGETS_DE_SOIREE` ; `epreuve::BUDGETS` en est désormais un alias (même valeur, 2²⁸ par sorte d'appel), et `epreuve::ARRIERE_PAS` relit `partie::ARRIERE_PAS`.
- **L'épreuve joue un tour par pas simulé**, comme le Worker un par image (`avancer_borne` à `ARRIERES_PAR_TOUR`), et **E19 juge chaque tour** : l'image d'un tour est son coût, plus, s'il a fini son avancée, celui des entrées placées aux `fenêtre + 1` pas qui suivent (le rejeu de la tête) ; sans découpage, c'est la somme de `(fenêtre + 2)` pas, comme avant. Le coût fixe de chaque entrée compte. La règle de la phase 2 qui ne comptait pas le coût fixe des arriérés est retirée. Après un gel, un jeu peut donc voir un verdict de E19 changer : ses arriérés se jouent huit par tour, et le rattrapage qui suit joue plusieurs pas dans un même tour.
- **E12 juge aussi le découpage d'un pas, pour tout jeu** : le script de la soirée des propriétés, rejoué en tours d'une entrée, avec les arrivées reçues pendant le pas découpé (la seconde moitié d'une rafale d'au moins trois arrivées passe juste après le pas qui la jouait), doit donner le même journal, ligne pour ligne, et les mêmes vues ; une soirée où aucune arrivée ne suit un pas découpé compare encore les pas joués en plusieurs tours, et le rapport le dit. E12 n'est donc plus sans objet pour un jeu sans fenêtre, sauf si aucun pas de cette soirée n'a deux entrées venues d'arrivées (un jeu où l'écran seul joue, comme `ardoise`). Son contrôle négatif est un défaut du commun, non un jeu : `Plan::commun_fautif = Some(CommunFautif::EtaleMal)`.
- **E14 reprend par la garde** : chaque reprise de l'épreuve part d'un point et d'un journal écrits puis relus par `partie::persistance`, le journal tronqué avant le point. La soirée longue dit la longueur de son journal, en lignes et en octets.

#### Passer à cette version

1. Un jeu n'a rien à faire : relancer `soiree-epreuve` depuis le dossier du jeu, et lire E12 et E19, dont ce qui est jugé a changé.
2. Du code qui construit un `Avancee` littéral ajoute `en_cours: None`. Du code qui lit `Journal::lignes()` par un indice venu d'un point ou d'un rapport soustrait `Journal::base()` (ou passe par `Journal::ligne(indice)`) ; tant que rien n'est tronqué, la base vaut 0.
3. Du code qui construit un `epreuve::Plan` littéral sans `..Plan::court(…)` ajoute `commun_fautif: None`.
4. Changer la forme d'un type gardé (un champ, un variant, leur ordre) monte `partie::persistance::VERSION_GARDE` et réécrit à la main les octets figés de `crates/partie/tests/coeur/garde.rs`, de `forme_garde.rs` et de l'instantané de `moteur-wasmtime` (`src/interieur.rs`).

### Après la phase 2 : les fautifs à `static` à part, et l'épreuve nomme un `static`

Rien ne change pour un jeu : ni l'ABI, ni l'API de `jeu`, ni ce que l'épreuve juge. Deux points de `conception/RESTE-PHASE-2.md` sont faits (1 et 2) :

- **Les fautifs repassent l'étape 1 depuis leurs sources.** Les cinq défauts dont la faute est un `static` (E6 `refuse-apres-avoir-ecrit` et `retient-un-refus`, E7 `vue-qui-ecrit`, E8 `secret-dans-un-static`, E14 `etat-hors-enveloppe`) quittent `fautifs` pour une crate à eux, `crates/exemples/fautifs-statiques` ; leurs types se nomment `fautifs_statiques::jeux::<Nom>` au lieu de `fautifs::jeux::<Nom>`, et leurs crates de module, qui restent dans `crates/exemples/fautifs/modules/<nom>`, dépendent de `fautifs-statiques`. Depuis leurs sources, `sain` et `avec-fenetre` rendent 0, un fautif sans `static` ne fait échouer que son contrôle ; un fautif à `static` échoue l'étape 1 et son contrôle (tests `sain_et_un_fautif_sans_static_depuis_leurs_sources` et `un_fautif_a_static_echoue_l_etape_1_en_nommant_le_static`, `crates/epreuve/tests/sources.rs`).
- **La règle « rien hors de l'état » nomme le `static`** qu'elle refuse : ``un `static` `NOM` : une valeur gardée hors de l'état (écrire un `const`)``, au lieu de ``un `static` : …``. Un outil qui lit ce texte dans le rapport doit s'y attendre.
- **`moteur-wasmtime`, `charger::bonjour_qui_boucle_refuse_avec_sa_faute` ne dépend plus de la charge** : son garde-fou mural passe de 2 s à une minute, et seul le carburant de chargement réel (`BUDGET_DE_CHARGEMENT`) arrête la boucle (OUT-11). Le test prend environ 2 s, plus sous la charge.

#### Passer à cette version

1. Un jeu n'a rien à faire.
2. Du code qui nomme l'un des cinq fautifs à `static` (`fautifs::jeux::RefuseApresAvoirEcrit`, `RetientUnRefus`, `VueQuiEcrit`, `SecretDansUnStatic`, `EtatHorsEnveloppe`) dépend de `fautifs-statiques` (`fautifs-statiques.workspace = true`) et écrit `fautifs_statiques::jeux::<Nom>`. Le trait `fautifs::defauts::Defaut` et le socle ne changent pas.
3. Un défaut de plus qui tient un `static` va dans `fautifs-statiques`, jamais dans `fautifs`.

### Le contrôle final de la phase 2

Rien ne change pour un jeu : ni l'ABI (`jeu_abi::ABI` vaut 3), ni l'API de `jeu`, ni l'épreuve. Le contrôle final (2026-11-05, `conception/RESTE-PHASE-1.md`, « Contrôle final de la phase 2 ») a tout relancé depuis zéro ; ce qui reste après la phase 2, et ce que la phase 3 devra mesurer ou trancher, est dans `conception/RESTE-PHASE-2.md`. Deux choses à savoir en éprouvant :

- **Les fautifs du commun ne passent pas l'étape 1 depuis leurs sources** (corrigé depuis : voir « Après la phase 2 » ci-dessus) : la crate `fautifs`, que toutes leurs crates de module partagent, tient cinq `static` (ceux des défauts E6, E7, E8 et E14), que la règle « rien hors de l'état » refuse. `soiree-epreuve crates/exemples/fautifs/modules/<nom>` rend donc 1 même pour `sain` et `avec-fenetre`. Pour voir un fautif échouer exactement son contrôle, l'éprouver par son module et sa copie native (`cargo run --release -p epreuve --example natif -- <nom> <module>`, ou le test `plan_standard`). Un jeu n'est pas touché.
- **Le plan standard est ce qui fait foi** ; le plan court ne garantit pas le résultat d'un jeu (Punchline y échouait E2 et E15 en phase 1, faute de temps pour deux parties).

### La forme, les sources et l'ABI 3 (phase 2)

Ce qui change pour un jeu écrit sur le commun de la phase 1 :

- **ABI 3** (`jeu_abi::ABI`) : `Retour::Proprietes` porte le nombre de variantes de `Phase` (E3 juge chaque phase, qu'un pire cas y mène ou non) et les noms des invariants (E4). Un module compilé contre l'ABI 2 est refusé au chargement : recompiler.
- **`Epreuve::INVARIANTS`**, sans valeur par défaut : `const INVARIANTS: &'static [&'static str] = &["…", …];`, la liste exacte des noms passés à `jeu::exiger!` dans `invariants` et `transition`. Un nom vide ou déclaré deux fois ne compile pas ; un nom déclaré jamais évalué dans une soirée, ou évalué sans être déclaré, fait échouer E4.
- **`Regard` n'est plus `Clone` ni `Copy`**, et ne se construit que dans `jeu` (`jeu::essai::regard` pour les tests natifs, absent sous `wasm32`). `jeu::colle::appeler`, `canonique`, `wasm::boite` et `wasm::appel` demandent un `&colle::Hote`, dont le constructeur est `unsafe` ; `colle::enveloppe_de_test` est retirée. `jeu-natif` est vide sous `wasm32`.
- **Les règles d'écriture** : `usize` n'est ni `Etat` ni `Message` (écrire `u32` ou `u64`) ; `f32::min`, `f32::max`, `f64::min` et `f64::max` sont interdits par `clippy.toml` (écrire `jeu::math::fminf`, `fmaxf`, `fmin`, `fmax`, ou une comparaison) ; la police du commun se nomme `Cle::POLICE_DU_COMMUN`.
- **`reecriture::VERSION` vaut 3** : `memory.init` paie sa longueur divisée par 64 ; une fonction a au plus `LOCAUX_MAX` (49 984) paramètres et locaux et `HAUTEUR_MAX` (1 024) valeurs sur la pile d'opérandes ; `soiree_profondeur` compte en étages de 4 096 octets de pile native. Le moteur remet à zéro la pile, la boîte, le brouillon et la zone de panique avant chaque appel.
- **L'épreuve part des sources** : `soiree-epreuve <dossier du jeu>` (étape 1 : règles d'écriture, clippy natif et `wasm32`, module et copie native compilés par l'épreuve). La crate de module se réduit à `jeu::module!(<chemin du type du jeu>);` ; la crate `<jeu>-epreuve` écrite à la main n'est plus nécessaire. Le rapport nomme les sept étapes du CONTRAT ; un module seul (`--wasm`) rend 3 au mieux.
- **Les contrôles** : la soirée appelle `Partie::repartir` quand la partie tombe en panne (fautif `panique-au-temps`, E2) ; E6 rejoue l'entrée acceptée qui suit des refus sur l'instantané d'avant eux (« même avenir », fautif `retient-un-refus`) ; E8 rejoue un geste ou un tenu avec un autre dernier octet (fautif `choix-qui-fuit`) ; E3 et E4 comme ci-dessus (fautifs `phase-sans-cas`, `invariant-jamais-evalue`). `etat-hors-enveloppe` échoue désormais E14 sur le plan standard.

Ajouts sans rupture dans `partie` : `Partie::repartir`, `Ligne::Reprise` (un `match` exhaustif sur `Ligne` doit le nommer), `EtatCoeur::reveil`, `Partie::numero_tenu`.

Pour s'adapter : déclarer `INVARIANTS` ; remplacer tout `usize` de l'état ou d'un message, tout `f32::min`/`max`, tout `Cle::du_fil(0)` ; réduire la crate de module à `jeu::module!`, retirer la crate d'épreuve écrite à la main ; reprendre `clippy.toml` et les profils du commun ; relancer `soiree-epreuve` depuis le dossier du jeu.

### L'épreuve, après la revue de la phase 2

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu. Les constats sont numérotés dans `conception/RESTE-PHASE-1.md`, « Revue de la phase 2 ».

- **L'étape 1 refuse de nouvelles choses** (règles nouvelles au rapport) :
  - « rien hors de l'état » : aucun `static` (atomique, `Cell`, `OnceLock`, `static mut`, ou même immuable) ni `thread_local!` dans le code des crates du jeu, hors de `tests/`. Écrire un `const` à la place d'un `static` immuable. Une valeur qui doit durer va dans l'état.
  - « forme calculée » : aucun `impl Forme`, `impl Etat` ni `impl Message` écrit à la main, aucun `Etat as …` (ou `Forme as`, `Message as` ; `as _` reste permis), aucun usage de `jeu::cache`, `Ecrivain` ou `Liseur`. Seules `#[jeu::etat]` et `#[jeu::message]` écrivent ces traits.
  - « sans unsafe » : une crate par chemin qui entre dans le module sans être membre du workspace du jeu, ni du commun, échoue ; en faire un membre. Le code tiers (registre, git) reste permis, et le rapport le nomme, avec la présence ou non d'`unsafe` dans ses sources.
  - « build.rs » : ce que le `build.rs` d'une crate du jeu dit à Cargo (le fichier `output` que Cargo garde) ne peut être que `rerun-if-changed`, `rerun-if-env-changed`, `warning`, `error`, `rustc-check-cfg`, et, pour la crate de module seule, `rustc-link-arg-cdylib=--max-memory=<octets>`. Le rapport nomme les `build.rs` lancés sur la machine.
- **E8 recharge la projection dans une instance neuve**, et non plus dans la mémoire de la définitive : un `static` où une entrée recopie un secret ne passe plus dans la vue rechargée. **E8 varie aussi un geste ou un tenu d'un seul octet** (un vote en enum plate) ; le rapport dit quand la variation n'a rien jugé. **E8 compare les valeurs secrètes une à une** quand une longueur change : Punchline n'échoue plus à tort au dernier bulletin.
- **E16 compare chaque pire cas aux soirées** : si une vue de l'écran des soirées coûte plus de carburant que la vue du plus lourd des pires cas, E16 échoue. Un pire cas se construit depuis les limites.
- **Deux pannes de plus au plan standard** : `Panne::Depart` (un téléphone part pour de bon, assez tôt pour que la soirée dure encore 90 s au moins) et `Panne::Doublon` (le réseau livre deux fois le même geste). Un jeu dont une phase attend pour toujours un joueur parti peut maintenant échouer E5 dans une soirée principale.
- **Les arriérés de E19** ne sont plus que ce qui arrive dans les deux secondes après le retour d'un appareil ou la fin d'un gel, fait sur une vue d'avant ce retour. Un geste ordinaire sur un écran resté le même paie son coût fixe : un module aux grandes zones peut maintenant échouer E19 quand plusieurs téléphones répondent dans la même image.
- **La soirée longue (E15)** choisit la cible de chaque allée et venue parmi les téléphones présents quand elle commence ; une allée et venue manquée rend E15 « pas exécuté ».
- **Le rapport** : l'étape 4 est « faite en partie » (`IssueEtape::FaiteEnPartie`, `"faite_en_partie"` en JSON) tant que la comparaison avec et sans prédiction manque ; le verdict d'un code 0 nomme les étapes du CONTRAT que cette version ne fait pas. Les budgets de E19 sont dits « choisis », non « mesurés ». Les secrets non cherchés par E8 sont décrits avec leur borne exacte (moins de 4 octets, ou de 3 avec leur longueur).

Ruptures dans la crate `epreuve` : `Panne` a deux variants de plus (`Depart`, `Doublon`) ; `IssueEtape` en a un de plus (`FaiteEnPartie`) ; `Rapport::verdict_du_code_0` est nouvelle ; `RATTRAPAGE_PAS` est nouvelle. Deux fautifs nouveaux : `secret-dans-un-static` (E8) et `pire-cas-trop-leger` (E16). `devine` garde son curseur secret pour son joueur.

Pour s'adapter : relancer l'épreuve depuis les sources. Remplacer chaque `static` du code par un `const` ou par un champ de l'état ; supprimer tout `impl Etat`/`Forme`/`Message` écrit à la main au profit de `#[jeu::etat]`/`#[jeu::message]` ; rendre membre du workspace toute crate par chemin dont dépend la crate de règles ; ne garder dans un `build.rs` que ce qui est permis ci-dessus. Lire E5 (le départ pour de bon), E16 (un pire cas plus léger que l'écran d'une soirée) et E19 (le coût fixe des gestes ordinaires).

### Les budgets de E19, dérivés du temps d'une image

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu :

- **E19 juge un coût, pas seulement un carburant.** Le coût d'un appel est son carburant plus le coût fixe d'un appel du module : le passage de l'appel et la remise à zéro des zones que le moteur fait avant chaque appel (la pile, la boîte, le brouillon, la zone de panique), que le carburant ne compte pas. Pour un module aux zones de 2 Mio, de l'ordre de 156 000 unités par appel ; pour un brouillon de 24 Mio, de l'ordre de 1,3 million. Un jeu dont le brouillon est grand paie donc chaque appel, même vide.
- **Ce qui est jugé, et comment** : chaque image des soirées (la somme du coût des entrées de `(fenêtre + 2)` pas consécutifs) à son maximum, et leur 99ᵉ centile ; chaque lecture courante du cœur (`Regarder`, `Etat`, `Valider`, `Ressources`) à son maximum ; la vue de l'écran de chaque pire cas à son maximum, contre son propre budget, plus large que celui d'une lecture courante. Le 99ᵉ centile d'une entrée ou d'une lecture n'est plus jugé seul.
- **Les budgets sont provisoires, et le rapport le dit** : `epreuve::BUDGETS_E19` les dérive, à un seul endroit (`crates/epreuve/src/budgets.rs`), du temps d'une image du CONTRAT (33 ms au plus, 60 images par seconde), de mesures de wasmtime sur la machine de l'hôte (`examples/etalonner.rs`), d'une marge choisie (4) pour le navigateur et l'ordinateur de la soirée, et de la part de l'image laissée au jeu (la moitié). Aujourd'hui : entrées d'une image 10 312 500 unités au plus et 5 208 333 au 99ᵉ centile, lecture courante 5 208 333 au plus, vue d'un pire cas 10 312 500 au plus. Le mainteneur les fixera dans `CONTRAT.md`.
- **Une entrée d'un arriéré** (faite sur une vue de plus d'une seconde : un téléphone coupé ou en arrière-plan, un ordinateur de la soirée gelé) ne compte que son carburant dans les images, pas son coût fixe : leur nombre dans un pas est le fait du commun. Le rapport les compte à part.

Ruptures dans la crate `epreuve` : `BUDGET_ENTREE`, `BUDGET_LECTURE` et `BUDGET_IMAGE` sont retirés, remplacés par `BUDGETS_E19` (un `BudgetsE19`), ses constantes d'origine (`PS_PAR_UNITE`, `FIXE_NS`, `PS_PAR_KIO_REMIS_A_ZERO`, `MARGE_NAVIGATEUR`, `MARGE_MACHINE`, `PART_DU_JEU_POUR_CENT`, `PART_DES_ENTREES_POUR_CENT`, `IMAGE_MAX_NS`, `IMAGE_COURANTE_NS`) et `ARRIERE_PAS` ; `Rapport` a deux champs de plus, `budgets: BudgetsE19` et `octets_remis_a_zero: Option<u64>` ; le texte du rapport a une ligne « Budgets de E19 provisoires… », et le JSON un objet `budgets` (`"provisoires": true`). Ajout dans `moteur-wasmtime` : `ModuleCharge::octets_remis_a_zero`. Trois fautifs nouveaux, chacun pour E19 seul : `pire-cas-trop-lourd`, `fin-trop-lourde` et `zones-lourdes`.

Pour s'adapter : relancer l'épreuve et lire E19. Un échec par le coût fixe se corrige en réduisant les zones que le jeu déclare (`limites.brouillon`), un échec d'un pire cas en allégeant la vue de l'écran à ses limites déclarées, un échec d'une lecture courante en allégeant la vue la plus lourde que les soirées atteignent.

### Le monde simulé de l'épreuve ressemble au réel

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu :

- **Les bots d'un téléphone décident sur la vue que ce téléphone a reçue**, en retard d'un trajet, et sur l'état de la définitive d'où elle vient (`bot_telephone` reçoit cet état-là). Un téléphone sans vue (à la connexion, après un rechargement) ne fait rien. Un bot qui comptait sur l'état courant pour ne jouer que des gestes acceptables en verra refuser : c'est voulu. Un geste porte, dans `quand.vu`, le pas de la vue que la page montrait quand il a été fait, même renvoyé après une coupure.
- **`quand.fait` porte l'erreur de l'horloge du téléphone**, jusqu'à un demi-aller-retour, dans les deux sens ; la demi-latence que le cœur reçoit est mesurée, gigue comprise. Un jeu qui juge sur `fait` ce qu'un joueur a vu se trompe : il faut lire `quand.vu` (fautif `juge-sur-le-fait`).
- **Le réseau** : un téléphone sur trois n'a que WebSocket (une perte retient la file) ; les tenus se perdent et se réordonnent sur WebRTC ; la page redit son tenu toutes les 250 ms ; les poignées de main ratent ; un onglet rechargé renumérote ses tenus, et l'accueil le réaligne ; une panne nouvelle, `Panne::ArrierePlan { duree }`, relâche le tenu, fige la page, et ferme sa connexion au-delà de 5 s. La soirée longue (E15) a ses coupures, rechargements, arrière-plans et sommeils.
- **Les fichiers** : les fichiers qu'apporte un bot de l'écran arrivent au jeu (`Composant::Fichiers`), et `Image`, `Lire`, `Rendu`, `Telecharger` sont servis avec de vrais contenus. Une image factice revient `Illisible` ; `jeu::fichiers::png_uni` écrit un vrai PNG pour un bot.
- **E13** tire ses gestes abîmés sorte par sorte (par le premier octet, le variant), et non plus parmi tous les gestes vus : un geste rare parmi ce qu'envoient les bots y a sa chance.

Ajouts sans rupture : `jeu::PERIODE_HYPOTHESE` (et `jeu_abi::PERIODE_HYPOTHESE`, dans le prélude), la période où E11 lit l'hypothèse ; `jeu::fichiers::png_uni` ; `partie::Partie::numero_tenu`. Ruptures dans la crate `epreuve` : `Plan` a un champ `monde: Monde` (`Monde::REEL` dans `Plan::standard` et `Plan::court`), `Rapport` un champ `monde: MondeJoue`, `Panne` une variante `ArrierePlan { duree }` (un `match` exhaustif doit la nommer), et le JSON du rapport un objet `monde`.

Pour s'adapter : relancer l'épreuve ; lire les refus nouveaux de ses bots ; remplacer tout jugement de ce qu'un joueur a vu par `quand.vu` ; Tier list, faire apporter de vrais PNG à son bot ; Sidequest, vérifier `jeu::PERIODE_HYPOTHESE` à la compilation.

## v0.4.0 — 2026-11-02

Un téléphone revient mieux : recharger, rejouer dans un onglet, dormir ou traverser un redémarrage de l'hôte ne perd plus ce qu'il a envoyé, et le protocole passe à 2. Un jeu peut choisir le stick natif de l'app iPhone et relier ses tests aux exigences communes, et sa CI ne part plus à chaque commit. `soiree certify` devient plus strict : une page qui écrit `console.error`, ou un `advance` sans `advance.progress`, le font échouer. `soiree check` aussi : une crate dont aucun test ne tourne, un `cert.json` qui retire `socket` ou `phone` sans le dire, une raison vide de sens ou une commande qui n'observe rien ne passent plus.

### Passer à cette version

Le code d'un jeu de la v0.3.0 compile sans changement : aucune API publique n'a disparu. Ce qui suit concerne son comportement et sa certification.

1. Installe l'outil de cette version, puis, à la racine du jeu, regarde ce qui va changer et applique-le :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.0 soiree
   soiree upgrade --dry-run
   soiree upgrade
   ```
   `soiree upgrade` change l'étiquette dans `Cargo.toml` et les crates du jeu, la version dans `soiree.toml`, les règles, `.github/workflows/soiree.yml` et `.soiree/`, met à jour les dépendances communes, puis lance `soiree check`. Ce `soiree check` peut encore échouer sur ce que les étapes suivantes corrigent : continue, et l'étape 13 le relance. `soiree upgrade` regarde seulement la première ligne de `soiree.yml` : il s'arrête avant toute écriture si l'en-tête `# @generated by soiree:` a disparu, et sinon il réécrit le fichier. Une modification faite à la main sous cet en-tête disparaît donc : renomme d'abord ta version. `soiree check` compare tout le fichier et refuse un `soiree.yml` modifié. Rien d'autre à faire pour cette CI : elle ne part plus qu'à la main (`gh workflow run soiree.yml`) ou sur une pull request. `soiree check` ne lit pas les autres fichiers de `.github/workflows/` : si l'un d'eux part à chaque commit, retire ce déclencheur toi-même. `soiree upgrade` ne touche pas à `fuzz/`, qui est un espace de travail à part : si `fuzz/Cargo.lock` existe, lance ensuite `(cd fuzz && cargo update --workspace)`, sinon il reste sur l'ancienne étiquette.
2. Si `cert.json` déclare `advance`, ajoute `advance.progress`, une expression JavaScript dont la valeur change quand la partie avance. Sans elle, `certify` échoue, alors qu'il ne faisait que le noter :
   ```json
   "advance": { "key": " " }
   "advance": { "key": " ", "progress": "document.querySelector('#manche')?.textContent" }
   ```
   Remplace `#manche` par ce qui, sur ta TV, change à chaque étape.
3. Cherche `console.error` et `console.assert` dans les pages du jeu. Pendant un scénario, chacun fait désormais échouer `certify`. Corrige ce qu'ils signalent, ou écris en `console.warn` ce qui n'est pas une erreur.
4. Dans `cert.json`, déclare ce que la certification exerce maintenant. `soiree upgrade` a écrit les exemples du modèle dans `.soiree/modele/tools/`, et `.soiree/CERTIFICATION.md` décrit chaque clé. Une commande reçoit un hôte frais par `{url}` et le navigateur du harnais par `SOIREE_CERT_BROWSER_MODULE`. Éprouve chacune avec `soiree certify -- --only text,progression,layout`.
   - `socket` : obligatoire dès que `phone` est déclaré, en général `"socket": "/salon/ws"`. Un jeu sans téléphone écrit `"phone": null`, `"socket": null` et `display` (voir plus bas) : retirer ces clés n'exclut plus aucun item.
   - `players.max` : obligatoire dès que `phone` est déclaré, par exemple `"players": {"min": 2, "max": 8}`. `soiree check` ne le demande pas, mais `capacity` échoue sans lui, alors qu'il sautait R3.
   - `refused` : le sélecteur et le texte exacts de ce que voit le joueur en trop, par exemple `"refused": {"selector": "#status", "text": "La partie est pleine."}`.
   - `text.checks` : copie l'exemple, puis remplace ses sélecteurs et ses textes par ceux de ton jeu.
     ```sh
     mkdir -p tools
     cp .soiree/modele/tools/text-check.mjs tools/text-check.mjs
     ```
     ```json
     "text": {"checks": [{
       "name": "chaque source de texte du jeu, cadres réels et négatifs",
       "command": ["node", "tools/text-check.mjs", "{url}"],
       "timeout_ms": 120000,
       "success": "no free-text entry"
     }]}
     ```
     `success` est une expression régulière que la sortie doit contenir : remplace-la par ce que ton script imprime quand il a fini.
   - Un jeu qui a une fin : copie l'exemple, puis change dans ce fichier `FINISHED`, vraie seulement à l'écran de fin, et `RUNNING_AGAIN`, vraie quand la partie d'après tourne. Si ton jeu ne repart pas par `advance` puis `start`, change aussi `restart`. Le script exige `advance.progress` (étape 2) et lit `join`, `joined`, `start` et `players` dans `cert.json`.
     ```sh
     cp .soiree/modele/tools/progression-exemple.mjs tools/progression-check.mjs
     ```
     ```json
     "progression": {"kind": "finite", "checks": [{
       "name": "une vraie partie va jusqu'à sa fin, puis une nouvelle reprend",
       "command": ["node", "tools/progression-check.mjs", "{url}"],
       "timeout_ms": 90000,
       "success": "D2 : fin réelle"
     }]}
     ```
   - Un jeu sans fin, sans `start` ni `advance`, comme un compteur ou un éditeur, écrit `"progression": {"kind": "continuous", "reason": "Compteur partagé : ni manche, ni départ, ni fin."}`. Un éditeur desktop solo écrit aussi `"display": {"role": "desktop", "reason": "Éditeur solo au clavier et à la souris, sans téléphone."}` ; ses cadres T2 restent requis.
   - Une TV dessinée sur un canvas déclare `"layout": {"name": "…", "command": ["node", "tools/layout-check.mjs", "{url}"], "timeout_ms": 120000}`, un script à toi qui lit les cadres vraiment dessinés et échoue quand l'un sort de la zone sûre. Une TV en DOM n'a rien à déclarer.
   - Pour une sauvegarde IndexedDB d'un jeu sans téléphone, `"storage": {"kind": "indexeddb", "database": "…", "store": "…", "key": "…"}`. Le harnais refuse cette clé dès que `phone` est déclaré.
   - Une raison tient maintenant en au moins deux mots et 12 caractères : `reason` de `catalogue_decisions`, `ui.not_applicable`, `progression.reason` et `display.reason`.
   - Une commande de `ui.checks`, `text.checks`, `progression.checks` ou `layout` contient `{url}` ou déclare `success`. Sans l'un ni l'autre, elle n'observe rien du jeu : `soiree check` et le harnais la refusent, y compris une commande de `ui.checks` qui passait en v0.3.0. Avec `{url}` seul, le code de sortie est toute la preuve : déclare aussi `success`.
5. Relis comment le jeu reçoit un événement : ce qu'une page a envoyé pendant un redémarrage de l'hôte arrive maintenant à la nouvelle partie, après `DeviceEvent::Arrived` et avec la phase de son envoi. Refuse-le comme tout événement périmé :
   ```rust
   DeviceEvent::Event { device, event, phase, .. } => {
       if phase != host.phase() {
           return; // périmé : envoyé pour une autre manche
       }
       self.jouer(device, event);
   }
   ```
   Des phases qui repartent de zéro à chaque lancement peuvent tomber juste : un jeu qui garde son état d'un lancement à l'autre doit reconnaître lui-même une action déjà comptée, car un événement en vol quand l'hôte est mort peut atteindre les deux lancements.
6. Sur la page, ne renvoie rien à la main après `Happening::Reset` ni après `connection.resume()` : salon envoie déjà ce qui attendait, une fois. `Happening::Reset` reste l'endroit où oublier l'écran et attendre le nouveau.
7. Si le jeu comptait sur `host.forget(device)` pour tenir un téléphone dehors, ajoute `host.close()` : un jeton oublié ne nomme plus personne, et le téléphone revient en appareil neuf dans une salle ouverte.
   ```rust
   host.forget(device);
   host.close(); // personne de nouveau n'entre, ce téléphone non plus
   ```
8. Ajoute à `.gitignore` ces quatre lignes, réécrites à chaque `soiree check`, puis retire-les du dépôt avec `git rm --cached --ignore-unmatch certification/check.json certification/check.md certification/check-fast.json certification/check-fast.md` :
   ```text
   /certification/check.json
   /certification/check.md
   /certification/check-fast.json
   /certification/check-fast.md
   ```
9. Facultatif : relie à tes tests celles des exigences C6, E5, T1, D1, D3, D4 et D5 que le jeu tient, avec `[[bindings]]` dans `verification.toml` (`VERIFICATION.md`, « Relier une exigence commune »), puis `soiree contracts --write-view`. Le stick natif de l'app passe par `Game::remote`, `Game::on_app` et `Game::feedback`, qui ont tous une valeur par défaut (`.soiree/salon/README.md`).
10. Dans les passages ciblés, prévois `resources` (R4) : son succès ne remplace pas les trente minutes d'endurance. `rtc` exige un vrai trajet UDP et éprouve les moteurs disponibles, les négociations retardées ou défaillantes et le repli dans son domaine prévu.
11. Si `soiree check` écrit `rien n'a tourné dans …`, cette crate a des tests et tous sont `#[ignore]` : retire l'attribut d'au moins l'un d'eux. Si le workspace n'a aucun test, écris-en un. Un test ignoré parmi d'autres ne fait rien échouer : le résumé devient `✓ Aucun échec, N point(s) à juger` et le nomme avec sa raison. Lis chaque point, corrige ce qui est un défaut et montre le reste à l'auteur avant de dire que c'est fini.
12. Si ta page s'affiche dans l'app iPhone, vérifie trois choses (`remote/CONTRAT.md`, « Qui atteint l'app ») ; aucun jeu du dossier n'est concerné au moment d'écrire :
    - un iframe n'atteint plus `webkit.messageHandlers.telecommande`, même de ton origine : passe ces messages par le cadre principal ;
    - un lien, `location` ou une redirection vers une autre origine sont annulés, schéma et port compris : envoie dans `Remote::Web` l'adresse finale, par exemple celle qui suit une redirection vers `https` ;
    - `navigator.mediaDevices` n'existe pas sur une adresse locale en `http`, ce qui était déjà vrai, et ailleurs WebKit pose maintenant sa question : teste `navigator.mediaDevices` avant de t'en servir.
    Hors de salon, un serveur `/remote` doit de plus continuer à lire son socket : l'app le lâche quand un Ping reste sans Pong.
13. `soiree check`, puis `soiree certify`. Pousse le jeu seulement après l'étiquette du commun : un jeu qui dépend d'un salon non publié ne compile pas chez un ami.

Une page restée ouverte pendant la mise à jour est refusée (`Refusal::Version`) : les joueurs rechargent, il n'y a rien à coder.

Le flux fiable ne demande aucun changement de code non plus, mais quatre choses se voient dans un jeu qui envoie de gros messages :
- `Host::show` ne remplace plus un état déjà parti en plusieurs paquets : il finit, puis le dernier montré le remplace. Une page peut donc afficher un état de plus avant le dernier. Avec un état qui tient dans un paquet, rien ne change.
- `Host::send` et `Client::send` rendent `Full` dès que 1 Mio d'événements attend, et plus seulement à `max_pending` messages. Traite `Full` comme avant. Un seul événement passe toujours, quelle que soit sa taille permise.
- `Limits::max_up` et `max_down` sont ramenés à 962 560 octets quand salon les reçoit : au-delà, `send` rend `TooBig` au lieu de rendre `Ok` et de bloquer la file.
- Après son `Welcome`, un `Client` prend le `max_up` et le `max_down` de l'hôte, quels que soient ceux de son `ClientConfig` : `Client::send` rend `TooBig` au-delà du `max_up` de l'hôte.

### Change de sens

- Le lien le plus récent gagne : un hello resté coincé sur une tentative abandonnée, ou celui de la page d'avant un rechargement, remplaçait le lien vivant. Il perd désormais, quel que soit l'ordre d'arrivée.
- L'époque est tirée au sort à chaque départ. Comptée depuis `salon.json`, elle retombait à 1 quand le fichier manquait, et la page perdait ses envois sans erreur.
- Une connexion `/remote` est un seul joueur, avec son budget. Une deuxième identité sur un lien n'est pas lue.
- Une absente évincée revient en `AppEvent::Back`, tant qu'elle est parmi les 256 dernières à avoir perdu sa place.
- `AppEvent::Input` et `AppEvent::Draw` n'arrivent que pendant un `Remote::Pad`. Ceux qui croisent un changement de télécommande sont jetés, et -128 devient -127.
- Une app refusée voit sa connexion fermée.
- Un paquet coûte ce qu'il transporte : au plus 1 635 parts par paquet.
- Les identités n'ont pas de plafond : voir `salon/README.md`, « Les limites ». « Nothing grows without bound » était faux.

- **`Happening::Reset` n'annule plus que ce que l'hôte a dit.** Avant, la file de la page était vidée : une réponse envoyée pendant un redémarrage rendait `Ok`, puis disparaissait sans un mot. Elle part maintenant vers la nouvelle partie, après l'arrivée de l'appareil et avec la phase de son envoi. Un événement en vol quand l'hôte est mort peut atteindre les deux lancements, une fois chacun.
- **« Jouer ici » (`connection.resume()`) rend ce qui attendait dans l'onglet.** Deux onglets montrent un seul jeton, et chacun gardait ses compteurs : après la reprise, l'onglet numérotait sous ce que l'autre avait envoyé, et l'hôte ignorait ses événements en les accusant. L'hôte numérote maintenant chaque client qui prend l'appareil (`Welcome.session`). Ce qui attendait part une fois, après ce que l'autre a envoyé, et ce qui était arrivé ne repart pas. L'hôte s'en souvient pour les huit derniers onglets qui ont envoyé quelque chose : un onglet absent plus longtemps peut répéter ce qu'il avait en vol, sans rien perdre.
- **Une page rechargée est entendue tout de suite.** Elle numérotait ses états depuis 1 alors que l'hôte gardait les numéros de l'ancienne page : chaque état était ignoré et accusé, et `set_state` rendait `Ok`. Le `Welcome` dit où en sont les numéros, et le premier message de la page ne complète plus celui que l'ancienne avait laissé à moitié envoyé, ce qui en faisait un seul de deux. Les paquets changent de forme : `Hello.resume` devient `Option<Resume { epoch, device, session }>` et `Welcome` devient `{ device, epoch, session, up_next, up_seen, state, down_first, max_up, max_down }`. `PROTOCOL` vaut 2. Une page d'un autre protocole est refusée même si son hello ne se lit plus, au lieu de se reconnecter sans fin.
- **`Host::forget` n'a jamais tué un jeton.** Sa documentation disait que le jeton cessait de marcher : c'était faux, et Punchline comme le modèle comptent sur le contraire. Le téléphone qui le remontre arrive en appareil neuf, sauf si la salle est fermée (`Host::close`). Son numéro n'est pas redonné pendant ce lancement ; il peut l'être après un redémarrage, puisque `Host::known` ne l'a plus.
- **Une identité est écrite avant que son `Welcome` parte.** La salle les enregistrait une fois par seconde : un appareil accueilli juste avant un plantage était inconnu du lancement suivant, et son numéro partait à un autre. Cela couvre aussi un processus tué à l'instant d'après, pas une coupure de courant : le fichier n'est pas synchronisé sur le disque.
- **Un lien qui a reçu son dernier mot n'écoute plus rien.** Un hello sur un lien refusé ou remplacé refaisait toute l'entrée : il reprenait l'appareil au lien courant, ou créait un fantôme qui gardait une place sans jamais de `Left`. Aucun appareil ne quitte plus par un lien qui se ferme tout seul : le jeu n'entend rien.
- **La mort de la salle arrête le processus.** Après une panique du jeu ou une attente système en échec, HTTP répondait encore et les téléphones réessayaient sans fin. `salon_host::run` écrit la cause et sort avec le code 101 (`ROOM_DIED`), ce qui ferme toutes les connexions.
- **Se réveiller abandonne une tentative trop vieille.** `Client::wake` laissait courir son budget de 20 s, au-delà des 10 s de C3. Il abandonne maintenant une tentative plus vieille que `Timing::silent`, et le retour du réseau abandonne toute tentative. Une page lit son temps par `SleepClock`, pour que le sommeil d'un iPhone ne cache pas le silence.
- **`certify` juge les notes écrites hors de leur scénario.** Elles disparaissaient quand le couple scénario et item n'était pas un contrôle. Elles rejoignent maintenant leur item, ou la santé du contrôleur si aucun contrôle de cet item n'a été choisi. C'est pourquoi `console.error` et `advance` sans `advance.progress` font échouer le passage : une vérification qui n'a pas pu finir ne vaut pas une réussite.
- **`soiree check` et `soiree certify` finissent par un résumé en clair.** Ce qu'un humain doit juger (`?` dans le rapport du harnais) ne fait plus échouer la commande : T2, C7 et C9 quand la page dit quelque chose, C4 et C5 quand le jeton et l'appareil ont été retrouvés. Cette décision vit dans le noyau prouvé (`passes_run`). Une vérification qui n'a pas pu finir (`~`) fait toujours échouer, avec le statut « pas pu vérifier ». Le harnais échoue maintenant si un retour donne une autre place, si un téléphone reste sur un écran vide après un redémarrage de l'hôte, ou si le stockage rétabli ne rend pas sa place.
- **Une tentative WebRTC a la même fenêtre absolue de 15 s côté page et côté hôte.** La page la détruisait après 3 s, pendant une négociation encore viable. Les erreurs natives déclenchent le repli WebSocket immédiatement. Un résultat SDP tardif ne remplace jamais ce repli, et la progression ne prolonge pas la fenêtre. Le délai de connexion C1 de 30 s et les budgets de rendu et de latence P1 et P2 ne changent pas.
- **Le budget d'image se juge à 1 ms près.** Une image sautée à 60 Hz dure 33,33 ms et Chrome la rapporte à 33,3 ou 33,4 ms : le budget de 33 ms échouait sur l'image qu'il nomme. Ce 1 ms est la résolution de la mesure, pas une marge, et une vraie image lente échoue toujours.
- **`cargo:suite` ne réussit plus quand rien n'a tourné.** Une crate dont tous les tests étaient `#[ignore]` donnait un vert à zéro test exécuté. Si rien ne tourne dans le workspace, ou si une crate a des tests et n'en exécute aucun, le contrôle reste `not-run` et le passage échoue. Un test ignoré parmi d'autres est compté et nommé avec sa raison, sans faire échouer. Une crate sans aucun test, à côté de crates qui en exécutent, ne compte pas comme un trou.
- **`✓ Aucun problème trouvé` veut dire ce qu'il dit.** Une revue donnait ce titre et le code de sortie 0, alors que `README.md` et `REJOINDRE.md` en font la fin du travail. Le titre devient `✓ Aucun échec, N point(s) à juger`, le code de sortie reste 0, et le résumé dit quoi faire de ces points.
- **`cert.json` ne retire plus d'item en silence.** Enlever `socket` rendait E1 à E4 et C7 non applicables, et `"phone": null` dix autres items. `phone` exige maintenant `socket`, et un jeu sans téléphone exige `display.role = "desktop"` avec sa raison. Sinon, ces items restent applicables et le passage échoue.
- **Un jeu n'a plus de CI à chaque commit.** Le workflow du modèle tournait sur macOS à chaque push, sur des dépôts privés qui partagent environ 190 minutes macOS par mois, à 6 à 13 minutes le passage.
- **`soiree upgrade` ne s'arrête plus sur un `Cargo.lock` écrit sous un `[patch]`.** Ce fichier nomme salon sans sa source : `cargo update --package salon-core` refusait, une fois les manifestes déjà réécrits. `upgrade` lance maintenant `cargo update --workspace`, qui rend leur source aux crates communes et garde tout le reste verrouillé.

### Réseau et salon

- `salon_web::audio` ajoute des effets TV bornés, identifiés et périssables, débloqués par un geste réel, avec coupure, volume et libération du graphe. Ses régressions rendent le vrai signal et éprouvent les navigateurs. L'activation et la qualité de la sortie physique restent à contrôler sur les appareils.
- `Game::remote`, `Game::on_app` et `Game::feedback` relaient les télécommandes natives : la porte ne répondait que par la page du téléphone et jetait les entrées, donc aucun jeu ne pouvait choisir `Remote::Pad`. Le pilote tient les apps par joueur et applique le contrat au même endroit : `seq`, silence, débit, table bornée.
- `SALON_PUBLIC_URL` dit où les téléphones joignent l'hôte, sans réseau ou derrière un VPN complet. `join_url` y prend sa base, validée à la frontière, et WebRTC annonce son hôte quand c'est une adresse IP.
- La page retient un échec de WebRTC dans son stockage de session : là où l'UDP est bloqué, chaque chargement ne repaie plus les 15 s. WebRTC est retenté après dix minutes, quand le navigateur annonce `online`, ou dès qu'un lien s'ouvre. Une offre restée sans réponse n'est pas un échec, et là où WebRTC a déjà marché un seul échec ne survit pas au rechargement. La page écoute aussi `resume` : `visibilitychange` ne suit plus toujours un dégel.
- Un nom trop long pour un paquet laissait le téléphone sur `Connecting` sans erreur, aucun hello ne partant. Le client le coupe comme l'hôte, et chaque message de contrôle, au plus grand, tient dans le plus petit paquet.
- Le serveur borne les connexions avant de créer leurs threads : deux connexions par appareil configuré, 32 places HTTP et 32 réservées au loopback, au moins 128 au total. Il applique une échéance absolue aux lectures HTTP et WebSocket et borne les écritures. La fermeture d'un lecteur arrête aussi son writer, même si un destinataire conserve sa file. Les événements et états de jeu ne changent pas de format.
- Sous macOS, la salle attend ses échéances avec une minuterie kqueue critique, et les messages réseau ou la fin du dernier émetteur la réveillent immédiatement. La file reste bornée et FIFO. Aucun réglage global, thread de minuterie ou spin permanent : les erreurs natives restent explicites et les budgets des jeux ne changent pas. Les autres systèmes utilisent leur attente de canal native.
- Les secrets de l'hôte ne passent plus par `/dev/urandom`, dont l'ouverture faisait échouer le démarrage sous Windows : ils viennent de `rand_core`, déjà dans le graphe. Rien de cela n'a été exécuté sous Windows.
- **Une panique d'une dépendance de WebRTC ne coûte plus qu'un lien.** dimpl 0.7.6 panique sur deux enregistrements identiques d'une page sans jeton, et un seul fil servait toutes les pages : la salle entière passait en WebSocket jusqu'au redémarrage de l'hôte, et chaque offre recevait 503. Chaque appel à str0m est gardé par lien, et le fil repart si une panique lui échappe. Il faut le déroulement de pile : un jeu ne doit pas écrire `panic = "abort"` dans ses profils, et l'hôte le dit au lancement s'il le fait.
- **Le repli sur WebSocket part 3 s après la réponse de l'hôte si ICE n'a rien entendu.** Là où l'UDP n'arrive pas, un téléphone neuf et chaque reprise attendaient 15 s. C'est un changement visible : un téléphone sur un réseau où ICE met plus de 3 s à répondre passe en WebSocket, ce qu'il ne faisait pas. Les 15 s absolues restent pour une réponse qui tarde et pour DTLS ou SCTP bloqués après ICE. `RTC_PATIENCE_MS` garde sa valeur. Rien à changer dans un jeu.
- WebRTC annonce d'abord l'adresse à laquelle le téléphone a envoyé son offre, puis celle d'avant, puis la boucle locale, et chaque lien écoute sur la sienne : un Mac à deux interfaces, ou dont le DHCP a changé l'adresse, ne fait plus payer le repli.
- Un paquet de `Caps::DATAGRAM` tient en un datagramme : str0m coupait en deux tout paquet de plus de 1 062 octets, sur un canal où un fragment perdu perd le message.
- Le tampon du canal ne fait plus attendre l'état frais derrière l'état périmé : un lien au-dessus de 4 Kio pendant 100 ms saute ses écritures, sauf une toutes les 100 ms, sur l'hôte comme sur la page. Le type est `salon_core::transport::Backlog` ; un jeu n'a pas à s'en servir.
- Une offre refusée (503, 400) n'est plus comptée comme un échec de WebRTC : la page rechargée essaie de nouveau WebRTC.
- Au plus 64 liens négocient à la fois, le plus ancien part. Une boucle d'offres ne ralentit plus les liens ouverts.
- `SALON_PUBLIC_URL` refuse une adresse IPv6 et le dit, au lieu d'un QR code qui ne mène nulle part : salon n'écoute qu'en IPv4. Passe par une adresse IPv4 ou un nom.
- **Un message plus gros que ce que le tuyau passe arrive.** Chaque renvoi repartait du fragment 0 et l'accusé ne disait rien d'un message incomplet : un tuyau qui garde le début d'une rafale ne passait jamais la fin, et tout ce qui suivait dans la file attendait pour toujours, sans erreur. L'accusé nomme maintenant les fragments arrivés, et un renvoi ne porte que les autres. Sur le banc, 60 000 octets à 1 Mb/s et 32 Kio de tampon : 0 message en 120 s avant, 941 kb/s utiles après. Cinq messages à 100 kb/s et 128 Kio : 2 sur 5 en 120 s avant, 5 sur 5 en 31,5 s après.
- **Ce qui part suit ce qui revient.** Une fenêtre part de dix paquets, double à chaque aller-retour et se coupe en deux à chaque perte, et chaque minuterie expirée double l'attente suivante. Sur un lien muet, un état de 60 000 octets coûtait 60 fois sa taille avant `Timing::absent`. Le prix : un gros message sur un lien rapide prend quelques allers-retours de plus. Ces réglages n'ont été éprouvés que sur un tuyau simulé.
- **Ce qui arrive après un trou attend.** Le récepteur garde 64 numéros, et trois accusés qui passent par-dessus un fragment le renvoient sans minuterie. 200 événements à 5 ms de gigue prenaient 10 s sans aucune perte.
- **Aucune minuterie de renvoi sur un WebSocket.** Les copies passaient devant ce qui est frais. Ce que l'hôte jette avant le socket est prouvé perdu par l'ordre du flux.
- **Un WebSocket ne reste plus muet sans erreur.** Si ce qui est parti reste sans nouvelle pendant trois fois `Timing::absent`, soit 18 s, alors que l'autre côté parle encore, le lien est déclaré perdu des deux côtés et la reconnexion reprend. Un jeu le voit comme une coupure, sans rien à changer : `DeviceEvent::Left`, avec `Why::Silent` si l'hôte renonce le premier et `Why::Closed` si c'est la page, puis `Back`. Aucune copie, et aucune reconnexion sur le banc jusqu'à 32 kb/s.
- **Le Welcome dit les limites de la salle** (`max_up`, `max_down`), et un état trop gros est sauté comme un événement au lieu de repartir sans fin.
- Un paquet en retard ne ramène plus la phase en arrière, ce qui faisait refuser une réponse (E5). Un événement envoyé juste avant `quit` arrive. Un `Timing` à l'envers ne fait plus paniquer la salle, et un `heartbeat` ou un `resend_min` à 0 valent 1 ms : `poll` ne répondait plus jamais `None`.
- Les paquets changent de forme : `Ack` porte `ahead`, `event_parts` et `state_parts`, et `Welcome` porte `max_up` et `max_down`. `PROTOCOL` reste à 2 : aucune étiquette n'a publié le protocole 2. Un fragment porte 940 octets sur un lien `Caps::DATAGRAM`, contre 996.

### Ce qu'une soirée laisse en mémoire

`salon_host::simulation` compte désormais ce que garde une salle, sans attendre. `evening` joue six tours sur le vrai hôte et les vrais clients, en temps simulé, et rend les octets vivants après chaque tour ; `flat` dit s'ils se stabilisent. Le compteur est `Counting`, l'allocateur à déclarer dans les tests (`#[global_allocator]`). `Network` gagne `replace`, une nouvelle page, et `disturb`, une coupure ou un silence tirés au dé. Un tour dure 10 min simulées dans `cargo test` et 60 avec `SOIREE_EVENING_MINUTES=60` : une fuite dépend de ce qui arrive, pas de la durée.

- Punchline et Sidequest en ont une, et elle y a trouvé le même défaut : un téléphone parti pour de bon restait en mémoire, dans la salle et dans salon, jusqu'à la fin de la soirée. Corrigé dans les deux. Le jeu ne perd rien : ce téléphone, s'il revient, est un nouveau venu, comme après l'oubli d'un inconnu.
- Sept retraits de code dans salon ont mesuré le test. Les quatre qui faisaient vraiment grandir la mémoire sont vus sur les six heures, grâce à des pages rechargées, de gros messages et un téléphone qui ne perd jamais son lien : une soirée ne voit que ce qu'elle emprunte. Les trois autres ne changent pas la soirée d'un octet : ils gardent des octets plus longtemps sans les accumuler. La version courte de `cargo test` manque les plus petites fuites, d'où les six heures dans la CI de salon et avant une étiquette.
- `--soak 30` reste en option : c'est la seule endurance avec un vrai navigateur et de vrais sockets. La mémoire du serveur s'y lit moins bien : une mesure en processus bouge de quelques Mio toute seule, là où la soirée simulée compte l'octet.

### Certification

- Une salle de moins de 8 joueurs se certifie : les parcours ordinaires ouvrent `players.max` téléphones au plus, et `silent` ne demande plus cinq téléphones.
- `cert.json.storage` est refusé sur un jeu à téléphones, où il ne changeait rien : C9 pouvait passer sans avoir touché la sauvegarde.
- `rtc` perturbe le vrai transport UDP puis renégocie son DataChannel avec la même identité. Il accepte qu'un effet se termine en revenant à sa valeur ou en cessant de bouger : il supposait un saut qui retombe, et échouait toujours sur un compteur comme celui du modèle.
- Le nouveau contrôle R4 observe la rétention DOM, JS et WASM et le RSS de Chromium et du GPU ; l'endurance R2 conserve aussi ces mesures. Les contrôles négatifs éprouvent les détecteurs. Le refus de stockage inclut IndexedDB, et `cert.json.storage` peut identifier la sauvegarde à corrompre.
- La certification complète exerce `capacity` au maximum déclaré, `text` sur un corpus déterministe et `progression` jusqu'à une fin réelle puis une reprise. Les commandes UI, texte et progression ont chacune leur salle. Les champs de 16 px sont contrôlés même avec une TV canvas, dont la géométrie exige un oracle réel.
- La cadence garde le reçu d'une mesure déjà terminée si Node est retardé, sans nouvel échantillon ni acceptation d'un arrêt réel du rendu. L'endurance garde ses séries brutes, un temps monotone, cinq minutes de chauffe effective et aucun thread supplémentaire conservé.
- Le harnais mesure la part occupée du processeur avant de lancer quoi que ce soit et après avoir tout arrêté, garde les deux dans le rapport et prévient dans le résumé à partir de la moitié des cœurs : une compilation à côté faisait de faux échecs sans un mot. Le verdict ne change pas, et une mesure illisible reste inconnue, jamais une machine au repos.
- L'identité d'un passage ne s'invalide plus pour ce qui n'est pas une source : bruit de la machine, notes que Git ne suit pas, dossier de données ignoré. Ils sont nommés dans la provenance, tout fichier suivi par Git reste une entrée, et rien n'est présumé hors de Git. Une vraie source changée pendant un passage le fait toujours échouer, et l'échec nomme les fichiers. Un `Cargo.lock` périmé faisait échouer le premier passage : un passage ordinaire le rafraîchit d'abord, comme Cargo ; un passage verrouillé le refuse toujours et ne réécrit rien.
- `[[bindings]]` relie à ses tests C6, E5, T1, D1, D3, D4 ou D5, qui dépendent des règles de chaque jeu. La validation est stricte : contrôle inconnu ou répété, liste vide, exigence reliée deux fois ou hors des sept sont refusés, et un minimum commun garde ses contrôles. Relier une exigence conditionnelle la rend applicable ; rien ne la rend non applicable. Une liaison ne compte que par l'observation de son test. Le modèle relie E5 et D1, les deux que ses tests portent vraiment.
- Deux tests du harnais supposaient un runner rapide : 250 ms pour atteindre une ressource retenue, 100 ms entre deux images. Une tentative dont le délai a expiré avant sa cause ne conclut plus rien et double ce délai, et le test de cadence vérifie que chaque intervalle vient du reçu du navigateur. Une navigation abandonnée à son délai continue de courir : sa réponse 500 tardive ne se lit plus comme l'échec de la suivante.
- Le contrôleur conserve les pannes TCP de Toxiproxy, mais borne sa connexion amont par un connecteur possédé : deux secondes pour joindre la vraie adresse LAN, sans nouvelle tentative ni repli. Un SYN bloqué ne retient plus la commande de coupure jusqu'au délai du système. Le connecteur borne ses ressources et les libère à l'arrêt ; les erreurs d'administration restent des pannes d'infrastructure.
- `rtc` juge quatre cas de plus : la fermeture 3 s après la réponse quand ICE reste muet, la reprise d'un WebSocket sain sous UDP muet en moins de 5 s, l'échéance absolue de 15 s une fois ICE passé, DTLS seul étant coupé, et une offre refusée qui n'empêche pas WebRTC au rechargement. L'ancien cas de l'échéance absolue après la réponse SDP ne pouvait plus l'atteindre : ICE décide avant. Le fichier `salon/tests/rtc-browser.mjs` joue ces cas sur l'hôte réel de la démo, dans les deux moteurs, sans jeu.
- `rtc` commence chaque cas de négociation avec la mémoire d'un onglet neuf : deux refus laissaient leurs échecs dans `sessionStorage`, et le cas de l'échéance absolue partait sans WebRTC. Un cas à part vérifie qu'un rechargement après un échec ne repaie pas la fenêtre.
- T2 juge un texte au repos : `assertLiteral` attend les animations finies du texte et de ses ancêtres, puisqu'une vue tout juste dessinée peut être à l'opacité 0 de sa première image. Ce qui finit caché échoue toujours.
- `jitter.ready` ne se contente plus d'être lue : si elle reste fausse, le harnais fait avancer la partie par `advance`, et R1 échoue s'il n'y arrive pas. Une panne de `jitter` compte pour R1, plus pour E1.
- `certify` ne dit plus « Ce passage réussit » à côté de `✗ À corriger` quand il reste aussi des points à juger.
- `certify` signale le pare-feu quand l'adresse du réseau ne répond pas. Les tests du harnais navigateur tournent dans un job indépendant des compilations Rust et iPhone, leurs fichiers l'un après l'autre. Le contrôle de navigation conserve l'historique des requêtes : la fermeture d'une connexion au délai ne signifie pas que sa ressource n'a pas été demandée. Ses délais et ses assertions ne changent pas.

### Outil

- `soiree doctor` demande Node 22, et non plus 20 : `hostile` a besoin du `WebSocket` global de Node.
- `soiree contracts --migrate` disparaît : tous les jeux ont `verification.toml`.
- Une raison tient en au moins deux mots et 12 caractères (`catalogue_decisions`, `ui.not_applicable`, `progression.reason`, `display.reason`, `raison_niveau`) : `x` n'excuse plus rien. Une commande de `ui`, `text`, `progression` ou `layout` sans `{url}` ni `success` est refusée : `["true"]` validait U1, T2 et D2. Avec `{url}` seul, le code de sortie reste toute la preuve, et le rapport du passage note cette limite. Le Markdown des rapports écrit les hypothèses à côté du résultat d'une liaison ou d'une garantie du jeu.
- Tout fichier suivi par Git reste une entrée de l'identité, même nommé `._x` ou `x~`. Dans un dépôt Git, seul un `.md` ou un `.txt` non suivi est présumé personnel, s'il est dans un dossier `notes/`, si son nom contient le mot `note` ou `notes`, ou s'il commence par `todo`, `scratch` ou `brouillon` : `footnotes.txt` restait dehors.
- `.soiree/modele/tools/` donne à un jeu les exemples que la mise à jour lui demande de copier, dont `progression-exemple.mjs` pour un jeu fini.
- `soiree check .` lit les documents du commun contre le code : versions, commandes et options, chemins, scénarios, clés de `cert.json` et items. Le catalogue renvoie à `verification.toml`, `CONTRAT.md` dit ce que salon permet vraiment, et seuls les derniers résultats restent dans `certification/` ; les campagnes passées sont dans l'historique git.
- `soiree check` marche sans Node, sauf si `cert.json` déclare `ui.checks[].success`, et il le dit alors clairement. Le harnais l'exige toujours.
- `soiree doctor` vérifie l'éditeur de liens, que rustup n'installe pas, et GitHub CLI, qui reste facultatif.
- Le `.gitignore` du modèle ignore les rapports que `soiree check` réécrit : ils entraient dans le premier commit d'un ami avec des chemins locaux. Les rapports de `soiree certify` restent suivis, avec le code. Le modèle ne provoque plus de fausses erreurs `{{nom}}` dans la sortie de Cargo.
- Sous Windows, `soiree certify` s'arrête avant `npm ci` et dit qu'il n'y est pas encore disponible, sans le faux titre `✗ À corriger : 23 point(s)` : le harnais dépend de `lsof`, de `ps` et des groupes de processus POSIX. L'identité d'un passage trouve `node.exe`, `Path` et `USERPROFILE`, et Toxiproxy n'appelle plus `which`. Rien de cela n'a été exécuté sous Windows.
- Un test `should_panic` se lit : libtest écrit `test nom - should panic ... ok`, un nom que l'analyseur ne reconnaissait pas, ce qui transformait toute la cible en erreur d'infrastructure.

### Recherche mesurable

`recherche/` rend mesurable la méthode du manifeste : une fiche par sous-système, des citations vérifiables, chaque leçon reliée à un test ou à un trou. La fiche pilote, `recherche/reconnexion.toml`, a lu 28 sources et relevé 13 trous dans salon, dont trois graves. `fiabilite`, `transport-webrtc` et `telecommande` en ont relevé 41 de plus, dont sept graves, que cette version ferme presque tous.

- `soiree recherche plan` dit ce que le diff ajoute, le sous-système, le niveau, la fiche, ce qui manque et les étapes à suivre. Il n'empêche rien.
- `soiree recherche verifie` confronte à l'amont chaque commit et chaque citation des fiches. Un lien mort fait échouer la fiche ; une panne de réseau a son propre code et ne conclut rien.
- `soiree recherche dette` liste les sous-systèmes de niveau 3 sans fiche, les fiches sous leur plancher et les trous ouverts. Elle part du plancher de `sous-systemes.toml`, pas du niveau écrit dans la fiche. `soiree check .` juge les fiches hors ligne, et la dette ne bloque qu'à l'étiquette.
- `soiree recherche verifie` échoue s'il n'a aucune fiche à vérifier ou si le dossier n'existe pas. Sans `gh`, le 422 de GitHub pour un commit inconnu est un échec, pas une panne, et une réponse 2xx qui n'est pas ce commit, comme un portail Wi-Fi, ne conclut rien.
- Une fiche doit tenir ses leçons (`lecons` dans `[[niveau]]`), dire dans `consulte` de quoi retrouver sa lecture, et une raison de baisse doit en être une. Un trou ne se ferme que sur un test non ignoré ou un item, jamais sur un fichier ou une ligne de code : `#[cfg(test)]` n'est plus pris pour un test, et `#[test] #[ignore]` sur une ligne est vu.

La dette de niveau 3 restante, ligne par ligne :

- `telecommande` T17, faible : l'app ne déclare ni `UIRequiresPersistentWiFi` ni de type de service réseau. L'effet ne se voit qu'avec de vrais iPhones et une partie de plus de 30 min, et rien ne dit qu'un réglage aveugle ne ferait pas pire : à mesurer d'abord (`REPETITION.md`).
- `transport-webrtc` T9, moyen : Safari iOS n'a jamais été mesuré sur un vrai Wi-Fi de soirée. Il faut un vrai iPhone : le cas est dans `REPETITION.md`.
- `transport-webrtc` T11, moyen : dimpl 0.7.6 garde ses défauts, contenus à un lien par T1. Aucune version publiée de str0m n'embarque dimpl 0.9, et une dépendance `git` sur 46 commits non publiés, avec 32 paquets de plus chez chaque ami, coûterait plus que ce défaut confiné : on attend la publication.
- `reconnexion` T9, faible : `wake` coupe un lien encore valide après 1,5 s de silence. Rien n'est perdu, et le coût visible pour un joueur reste à juger sur de vrais téléphones (`REPETITION.md`).
- `reconnexion` T11, faible : le rattrapage ne se distingue pas du direct. Rien ne montre que cela gêne un jeu, et les jeux peuvent horodater leurs événements.

### Nuit, mutations, tests

- `nuit.yml` lance chaque nuit, sur des machines Linux gratuites, ou à la main, jamais sur un push : les mutations de `salon-core`, dont le rapport n'a pas de seuil, et un fuzzing borné dont le corpus se garde d'une nuit à l'autre. `salon/fuzz/` attaque les paquets et une session hostile : un lien hostile doit laisser connecté un téléphone honnête, et lui devoir exactement ce qui lui était dû.
- Une campagne réelle sur `salon-core` laissait 141 mutants vivants sur 433 : accusés, renvois, renumérotation et échéances n'avaient aucun test pour les voir manquer. La campagne suivante en laisse 11 sur 453. Les nouveaux tests (`client_edges.rs`, `host_edges.rs`, `stream_edges.rs`, `types_edges.rs`) forgent des paquets avec une copie du format, et `found_bugs.rs` garde chaque bug reproduit en les lisant.
- Le chaos joue rechargements, onglets jumeaux, « Jouer ici », pages mortes en silence, redémarrages de l'hôte et paquets retenus : retirer seul un correctif de ces trous le fait rougir, parfois après plusieurs milliers de cas (mesures dans `recherche/reconnexion.toml`). `NOTE_CRITICAL` a son test : le retirer de la minuterie le fait échouer. Un serveur réel dans un processus fils prouve la mort de la salle.

### L'app iPhone

Le projet Xcode déclare les fichiers produits par sa phase de compilation Rust : les liaisons Swift, leur en-tête, leur module et l'archive native. Xcode attend ainsi leur génération avant de compiler Swift, y compris sur un checkout neuf, et la CI le vérifie sans liaisons prégénérées. Aucun `cargo xtask ios` préalable n'est nécessaire.

La coquille Swift a enfin un banc : `remote/bench` compile ses vraies sources, les lance sur un simulateur et les joue contre un faux jeu qui coupe, gèle ou se tait, et contre un vrai salon. `recherche/telecommande.toml` y a mené 17 de ses 18 trous à un test ; T17 reste à mesurer.

- **Une coupure ne perd plus un trait.** Ce qui est tracé pendant une coupure attend le socket suivant, derrière `Hello` et le `Show` du jeu, à 180 trames par seconde au plus pour rester sous le budget de salon. Seule reste perdue une trame déjà remise au noyau quand le lien meurt : la voir demanderait un accusé.
- **Un hôte muet se voit.** L'app envoie un Ping toutes les 2 s et lâche le socket sans Pong, en 4 s au plus. Une tentative vers une adresse muette dure 5 s, plus 60.
- **Une page ne laisse plus un écran vide.** Un chargement raté et un processus de page mort se rechargent, avec le même repli de 1 à 5 s et une notice, et les capteurs et sockets d'une page morte sont rendus.
- **Une seule notice à la fois.** Réseau local refusé, elle nomme le réglage. Un jeu jamais joint n'est plus une « connexion perdue ».
- **Le pouce survit à une coupure.** Une coupure ne lâche plus les doigts, et le même `Show` reçu à nouveau garde la télécommande, son dessin et ses numéros de trait.
- **Les états partent à 60 par seconde.** Sur un écran à 60 Hz, le frein en sautait un sur deux. Un appui plus court qu'un envoi atteint maintenant le jeu.
- **Un retournement de 180° se dit.** La page reçoit `telecommande:orientation`, et le volant ne s'inverse plus.
- **La page, et elle seule.** Le pont et `window.telecommande` restent au cadre principal de l'origine de la page (`remote_core::trust`), et WebKit demande au joueur avant de donner le micro ou la caméra, qu'une page obtenait sans un mot.
- **Le reste.** L'écran se remet en veille en quittant un jeu, et une page relative garde la requête du QR.

## v0.3.0 — 2026-10-28

Les engagements propres au jeu passent de tableaux Markdown à
`verification.toml`. L’outil résout leurs obligations et les contrôles communs,
exécute les contrôleurs, puis évalue des observations rattachées à leurs entrées.
`CERTIFICATION.md` devient une vue des engagements ; les rapports datés restent
séparés. Cette rupture demande une migration explicite des identités de tests :
un nom de fonction trouvé dans le texte ou un ancien rapport ne suffit plus.

Cargo fournit les cibles et libtest les tests réellement découverts et exécutés.
Paquet, genre de cible, cible, nom complet, features, features par défaut et
profil distinguent les identités. Tests ignorés, zéro sélection et compilation
échouée ne satisfont aucun engagement. Le noyau de décision de production est
vérifié avec une version figée de Verus ; les adaptateurs, oracles et décisions
d’applicabilité restent hors de cette preuve.

`check --fast` et `certify -- --only …` rendent leur périmètre explicite.
`certify` échoue si un contrôle demandé est absent, sauté, partiel ou si le
contrôleur tombe en panne ; un simple code de sortie Node réussi ne suffit plus.
Un passage ciblé réussi laisse les autres obligations non attestées. Les items
du catalogue sans décision restent non revus et les gestes sur appareils réels
restent des jugements humains. Les coûts d’itération et de CI restent à mesurer.

La CI conserve les téléchargements Cargo, les compilations et les outils figés,
avec contrôle exact de leurs versions. Le binaire du commun est toujours
reconstruit depuis son checkout. Aucun rapport n’est restauré comme résultat
valide ; le gain de temps de ces caches reste à mesurer.

Les attentes UI attendent désormais la valeur résolue de leur expression : une
promesse qui résout `false` ne franchit pas l’attente. Les expressions régulières
et les délais sont validés avant l’exécution des contrôles UI. Le transport de
test est inscrit dans le périmètre ; un passage `phone_upstream: "loopback"`
laisse les garanties d’accès au réseau local non attestées. Les fichiers servis
sont préparés avant leur empreinte.

`soiree upgrade` met aussi à niveau les dépendances communes déclarées
directement dans les manifestes des membres du workspace. En 0.2.0, seules
celles du manifeste racine étaient alignées ; Sidequest conservait ainsi
`remote-web` et `remote-protocol` à l’ancienne étiquette dans ses membres.
Les commentaires et les dépendances de développement, de construction et
propres à une cible restent préservés. La découverte Cargo fonctionne
avant le premier lockfile et ne le crée ni ne le modifie ; un membre hors
du dossier du jeu ou une étiquette absente arrête la commande avant toute
écriture.

Le lock du projet fuzz de la télécommande et sa cible `phone_session` sont remis en état :
le booléen du corpus est converti vers `Orientation` au bord de l’entrée,
sans changer les octets des corpus.

### Passer à cette version

1. Installer l’outil avec
   `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.3.0 soiree`.
2. À la racine du jeu, lancer `soiree upgrade --dry-run`, puis
   `soiree upgrade`. La commande aligne le manifeste racine, les membres
   du workspace, les règles et la CI sur cette étiquette publiée. Un
   sous-projet indépendant, tel qu’un workspace `fuzz/`, garde son propre
   lockfile : le résoudre aussi après la migration s’il dépend du jeu.
3. Préparer `references.json` : pour chaque référence historique active, donner
   son paquet Cargo, son genre de cible (`lib`, `bin` ou `test`), sa cible,
   son nom complet libtest et sa configuration. Une redirection de helper ou
   de nom doit être expliquée. Le format exact est dans `VERIFICATION.md`.
4. Lancer `soiree contracts --migrate references.json`, relire les engagements,
   hypothèses et décisions importés, puis `soiree contracts --write-view`.
   L’original est conservé dans `CERTIFICATION.legacy.md` ; aucun résultat
   historique n’est importé comme réussi. Un jeu qui possède déjà un contrat
   structuré l’édite directement, sans relancer cette migration.
5. Lancer `soiree check`, les scénarios nécessaires avec `soiree certify`, puis
   le passage `soiree check --locked --published` depuis un clone sans patch
   local. Examiner les contrôles en attente et les limites du rapport avant
   de déclarer le périmètre validé. Les crates communes du SDK gardent leur
   version de paquet 0.2.0 ; l’étiquette Git v0.3.0 fige l’ensemble distribué.

## v0.2.0 — 2026-10-28

Le contrat entre une page libre et l’app iPhone est défini dans
`remote-protocol::web`, réutilisé par le SDK WASM et par les liaisons Swift
UniFFI. Le format JSON public reste identique. Les propriétés CSS écrites par
Rust demandent maintenant une catégorie de valeur typée.

Les pages possèdent leurs ressources navigateur via `salon_web::ui` :
listeners, timers, animations, tickets d’opérations asynchrones, composition
et effets ponctuels. Les formulaires gardent leur identité pendant les
mises à jour qui ne changent pas leur tâche. Les tests exercent les
destructions, les résultats tardifs, les compositions et les décodeurs
ambigus, ainsi que la propagation des échecs des contrôles eux-mêmes.
`soiree new` et `soiree upgrade` installent une CI versionnée ; le mode
`check --published` refuse les dépendances communes non publiées ou locales.
Ce mode exige un jeu versionné (`Cargo.toml` et `soiree.toml`) ; retirer la
version ne permet pas de sauter les règles de CI et les déclarations U1.
Les noms de jeu `salon` et `remote` sont réservés aux crates du commun ;
`--org` choisit le dépôt du jeu sans rediriger ses dépendances communes.

### Passer à cette version

1. Après publication de l’étiquette `v0.2.0`, installer l’outil de cette version
   et aligner les dépendances salon/remote ainsi que `soiree.toml`. Pendant le
   développement chez l’hôte, les jeux gardent leur ancienne étiquette Cargo
   et utilisent le `[patch]` local ; ils ne doivent pas être poussés ainsi.
2. Régénérer chaque module avec `soiree dom <page.rs>`. Pour une propriété
   personnalisée écrite par Rust, déclarer sa catégorie dans sa feuille CSS,
   par exemple `/* soiree-type: --accent color */`. Catégories : `color`,
   `length`, `number`, `time`, `image`. Une propriété sans catégorie n’a pas
   de setter ; les déclarations absentes ou contradictoires échouent.
3. Remplacer les valeurs libres de `prop::X.set` par une référence à
   `salon_web::style::{Color, Length, Number, Time, Image}` :
   `Color::rgb(255, 0, 0)`, `Length::px(x)?`, `Number::new(n)?`,
   `Time::milliseconds(ms)` ou `Image::url(src)?`. Les couleurs hexadécimales
   existantes se valident avec `parse::<Color>()`. Les coordonnées et nombres
   non finis sont refusés. Les noms de propriété viennent du générateur.
4. `remote_web::Info::player` est un `PlayerId` validé : utiliser `.bytes()`
   pour le jeton ou `.to_string()` pour l’affichage. Retirer les décodeurs
   hexadécimaux qui validaient à nouveau cette même identité.
5. Reconstruire l’app avec `cargo xtask ios`. `WebRemote.swift` utilise les
   commandes générées de `remote-ffi`, et les scripts de sortie sont fournis
   par le même contrat Rust. `cargo xtask web-contract` compile et exécute les
   liaisons Swift réelles et vérifie les scripts dans JavaScriptCore ; ce
   contrôle rejoint `soiree check remote` sur macOS. Ailleurs, il est signalé
   comme non exécuté. `cargo xtask web-contract-browser` complète ce contrôle
   avec le vrai SDK WASM, WebKit et Chromium (navigateurs du harnais requis).
   Un build Xcode vérifie le consommateur réel.
6. `soiree check`, puis les vérifications navigateur applicables. Les fichiers
   `.soiree/` et les sorties UniFFI restent générés et ignorés par Git.
7. Conserver chaque `Connection` aussi longtemps que sa page en a besoin :
   sa dernière copie détruite ferme maintenant le transport. Remplacer les
   callbacks oubliés et les timers/boucles manuels par `Listener`,
   `Listeners`, `Timer` et `AnimationLoop` de `salon_web::ui`, gardés dans
   leur propriétaire. Leurs résultats sont `must_use`. Les listeners DOM et
   les callbacks `on_orientation`, `motion`, `Datagram::on_message` de
   `remote-web` prennent `Fn` : remplacer une capture directement mutable
   par un `Cell` ou `RefCell` détenu par le propriétaire. Libérer les emprunts
   avant une opération DOM pouvant déclencher un événement synchrone.
8. Protéger les tâches avec `OperationSlot` (le dernier appel remplace le
   précédent) ou `OperationScope` (plusieurs tâches indépendantes). Appeler
   `finish` juste avant d’appliquer le résultat ; annuler à la fermeture ou
   au remplacement de l’objet logique. Un ticket n’interrompt pas à lui seul
   le travail natif du navigateur. Brancher `Composition` sur les événements
   de composition et bloquer les nouvelles validations/raccourcis ; garder
   les relâchements de commandes tenues. Un effet ponctuel ordonné peut
   utiliser `OnceEffects`, avec identités croissantes et remise à zéro lors
   d’un changement de session faisant autorité.
9. Ajouter `ui` à `cert.json` : scénarios ou commandes réelles couvrant
   `editing`, `lifecycle`, `async`, `effects`, et une justification pour
   chaque catégorie non applicable. Voir `CERTIFICATION.md`. Le modèle et
   les trois jeux de l’hôte en donnent des exemples. Installer la CI avec
   `soiree upgrade` ; `--dry-run` montre les fichiers visés. La commande
   cible la version du binaire installé et refuse une étiquette non publiée
   avant de modifier le jeu ; elle ne réécrit pas automatiquement sa logique.
10. Remplacer `postcard::from_bytes` par `salon_core::codec::decode` pour
    un message dont le cadre contient exactement un objet. Les décodeurs de
    salon et de la télécommande l’utilisent déjà. Les variantes JSON non
    étiquetées ne sont acceptables que si leurs formes sont disjointes,
    vérifiées par des cas négatifs.

### Change de sens

- Une identité doit être exactement seize octets, exprimés par 32 chiffres
  hexadécimaux. Un datagramme encodé en base64 doit être canonique et complet.
- Les objets JavaScript mal typés sont refusés avant les effets natifs ; les
  ports et octets ne sont plus tronqués par les conversions NSNumber.
- Les valeurs CSS d’un jeu passent par des constructeurs validés et des
  propriétés de catégorie correspondante. Les tracés SVG et la syntaxe
  déclarative CSS restent du vocabulaire du navigateur, pas une promesse de
  typage de toute sa grammaire.
- Les octets ajoutés après un message binaire valide sont refusés ; ils
  étaient auparavant ignorés par `postcard::from_bytes`.
- La destruction du dernier `Connection` clôt son transport. Les ressources
  d’une vue cessent leurs rappels avec leur propriétaire.
- Plusieurs abonnements aux capteurs de `remote-web` partagent la source :
  seul le dernier abonnement détruit envoie `motion:false`.

## v0.1.0 — 2026-10-27

La première version : salon, la télécommande iPhone, les règles communes, le harnais de certification, le catalogue d'items par mécanique, l'outil `soiree` et le modèle de jeu.

### Passer à cette version

1. Installe l'outil : `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.1.0 soiree`.
2. Un jeu neuf : `soiree new <nom>`. Un jeu existant : ajoute `soiree.toml` (`version = "0.1.0"`), remplace les chemins vers salon par `{ git = "https://github.com/build-your-own-game/soiree", tag = "v0.1.0" }`, puis `soiree check`.
3. Les règles se sont durcies depuis `v0.1.0-pre.1`. Recopie dans le jeu les lints de `outil/regles/cargo.toml` (à partir de `wildcard_enum_match_arm`, en retirant `cast_precision_loss = "allow"`) et `max-fn-params-bools = 0` de `outil/regles/clippy.toml` ; `soiree check` dit ce qui manque. Puis `cargo clippy --all-targets` et `cargo clippy --target wasm32-unknown-unknown` disent quoi corriger. Les idiomes attendus :
   - `wildcard_enum_match_arm` : nomme les variants du bras de repli (`A | B(_) => {}`), ou extrais avec `if let`. Un `#[expect]` pour une enum étrangère large, ou pour un `other => panic!` dans un test.
   - `fn_params_excessive_bools` (`max-fn-params-bools = 0`) : une enum à deux variants, ou deux méthodes. Un `#[expect]` quand le bool est la donnée elle-même (une touche enfoncée, un prédicat).
   - `cast_precision_loss` : `f64::from(u32)` ; `Ms::as_f64()` pour le temps de salon ; sinon une seule fonction de conversion avec son `#[expect]`.
   - `arithmetic_side_effects` : `saturating_*` pour le temps et les statistiques, `checked_*` et une erreur pour un identifiant ou une entrée, `wrapping_*` pour un hachage ou une animation, itérateurs ou `checked_sub(1)` pour les index, un `NonZero*` comme diviseur. Une crate de rendu, et les tests, peuvent s'en exempter en tête de crate ou de module : `#![expect(clippy::arithmetic_side_effects, reason = "…")]`.
   - `let_underscore_untyped` et `unused_result_ok` : `let _: Result<(), JsValue> = element.focus();`, jamais `.ok();`.
4. Depuis `v0.1.0-pre.1`, pour suivre ces règles, quelques API de salon et de la télécommande ont changé :
   - salon-core : `Host::set_open(b)` devient `Host::open()` ou `Host::close()`, et `Host::local_present(now, device, b)` devient `local_plugged(now, device)` ou `local_unplugged(now, device)` ;
   - salon-host : `Clock::at` renvoie un `Option<Instant>` (`None` au-delà de ce qu'`Instant` peut tenir) ;
   - salon-host : `qr_svg(url, dark, light)`, qui donnait du SVG en texte, devient `qr(url) -> Result<Qr, String>` : les modules du QR code, dans le type `salon_core::Qr`. La page le dessine avec `salon_web::qr(&qr)`, un `<svg>` dont les couleurs viennent du CSS du jeu (`fill` sur `path`, `background` sur `svg`) ;
   - remote-core : `landscape_left: bool` devient `Orientation::LandscapeLeft` ou `Orientation::LandscapeRight` dans `Wheel::update`, `Pointer::motion` et `remote::Motion`.
   - remote-core : `Pressed::set(id, true)` devient `Pressed::press(id)`, et `Pressed::set(id, false)` devient `Pressed::release(id)`.

   Nouveau : `Ms::as_f64()`, pour passer un temps de salon à une horloge de page ou à une statistique.
5. Le manifeste a un principe 8, « Aucune chaîne n'est un contrat », et `soiree check` attend son point dans « Écarts au manifeste » de `LEARNINGS.md`. Pour le tenir :
   - **La page.** `clippy.toml` interdit maintenant les méthodes à chaînes de `web_sys` (`get_element_by_id`, `set_attribute`, `class_list`, `set_inner_html`, `style().set_property`…). Pour chaque crate qui touche une page, génère son module de noms, puis déclare-le avec `pub mod page;` à la racine du crate :

     ```sh
     soiree dom crates/web/src/page.rs --html ../../../web/index.html --css ../../../web/style.css
     ```

     Les chemins sont relatifs au fichier généré. Ce que seul le Rust crée se déclare avec `--id <id> <balise>`, `--data <nom>`, `--attr <nom>`, `--class <nom>` ou `--tag <balise>`.
   - **Le code.** `document.get_element_by_id("title")` devient `page::id::TITLE.get()?`, qui renvoie directement le bon type. `el.class_list().add_1("tier-row")` devient `page::class::TIER_ROW.add_to(&el)?`. `el.set_attribute("data-row", &id)` devient `page::data::ROW.set(&el, &id)?`, et la lecture `page::data::ROW.get::<RowId>(&el)`. Une valeur de `data-*` qui aiguille (`data-action`) devient une enum avec `Display` et `FromStr`, et un test relit le HTML pour vérifier ses valeurs.
   - **Le suivi.** Après une modification du HTML ou du CSS, `soiree dom crates/web/src/page.rs`. `soiree check` échoue si un module n'est plus à jour.
   - **La version du jeu.** Copie `.soiree/salon/version.rs` (écrit par `soiree check`) comme `build.rs` du crate qui définit `Up`/`Down`, puis remplace la constante écrite à la main par `pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));`.
   - **Les routes.** `salon_core::route::{OFFER, TV_KEY, SOCKET}` remplacent les chemins `"/salon/…"` écrits en littéral.

6. Le modèle est maintenant jouable : `soiree new` donne une simulation sans écran, un hôte et les pages TV/téléphone. Pour un jeu existant, garde sa mécanique et prends les intégrations de `salon/README.md` ; ne remplace pas son code par le modèle.
7. Enregistre les entrées là où elles entrent dans la simulation avec `salon_host::journal::Recorder`, un en-tête typé, puis `record` et `checkpoint` de l’état complet (`salon_core::journal::fingerprint`). Une opération qui tire du hasard, même avant d’appliquer une action de bot, entre aussi dans le journal. Une fin naturelle appelle `finish` ; une nouvelle partie ouvre un nouveau journal avec son état initial exact. Un éditeur solo utilise `salon_web::journal::Recorder` et le récepteur typé `salon_host::journal::browser_sink`. Les journaux sont locaux, ignorés par Git et purgés après trente jours. Expose une commande de replay qui compare l’état, et conserve une fixture fictive versionnée.
8. `Connection::open` installe le filet anti-panique ; une page solo appelle `salon_web::install_tv_diagnostics`. Retire les hooks locaux qui le remplaceraient. `Game::diagnostic` reçoit les paniques et les marqueurs F8 : écris-les dans le journal de partie. La récupération s’arrête après trois recharges rapprochées, et si son compteur ne peut pas être mémorisé.
9. Pour tester l’hôte entier, utilise `salon_host::driver::Driver` et `salon_host::simulation::Network`, avec horloge et réseau à graine. La vraie salle utilise ce même driver. La campagne normale garde des milliers de graines ; `SOIREE_SIM_SEEDS` étend celle des jeux de l’hôte.
10. `soiree certify` ouvre quatre téléphones WebKit et quatre Chromium. `soiree doctor` vérifie le WebKit de sa version de Playwright ; installe-le avec la commande indiquée s’il manque. Ajoute les sondes causales `performance.probes` décrites dans `CERTIFICATION.md` : une entrée réelle, puis son effet sur la TV. Un canvas publie sa projection après un rendu réussi. Les chiffres sont écrits dans `certification/performance.json` et le rapport, et les dépassements font échouer la certification. La suspension DevTools reste vérifiée sur Chromium, les autres scénarios concernent les deux moteurs.
11. `soiree mutants` lance à la demande les mutations sur `crates/core`, avec `--in-diff` et des exclusions motivées. Rien ne l’appelle depuis `check` ou `certify`. Son rapport indique le périmètre et l’empreinte des sources : un petit premier passage ne prouve pas toute la simulation.

### Change de sens

- Les replays ne sont pas un bénéfice automatique du déterminisme : les entrées et tous les tirages aléatoires doivent réellement être enregistrés dans l’ordre de simulation. Le numéro calculé depuis les sources informe le diagnostic ; le format de replay contrôle sa lecture, et l’empreinte de l’état détecte une dérive de sens.
- Une certification mesure désormais des budgets et couvre WebKit. Un ancien passage Chromium seul ou sans sonde de rendu ne satisfait plus ce plancher.
- Une panique de page provoque une récupération visible et bornée. Elle ne reste plus silencieuse et ne déclenche jamais une boucle de rechargement illimitée.
- La liberté de casser les frontières internes s’accompagne d’un test et d’une notice de migration. Le contrat de l’app iPhone et les contraintes du monde réel restent à coordonner et à éprouver explicitement.
