# Les crates du commun

La conception est dans [`../conception/INTERFACE.md`](../conception/INTERFACE.md) (§ 8 pour ce découpage) ; le contrat dans [`../CONTRAT.md`](../CONTRAT.md).

## La carte

| Crate | Rôle | Forme | État |
|---|---|---|---|
| `jeu-abi` | l'enveloppe `Appel`/`Retour`/`EntreeFil` et le vocabulaire du commun, validé au décodage ; la zone de panique et les bords de la forme canonique | `no_std`, sans `unsafe` | écrite, testée |
| `jeu-derive` | `#[jeu::message]`, `#[jeu::etat]`, `#[derive(Variantes)]` | macro procédurale | écrite, testée |
| `jeu` | ce qu'écrit l'auteur d'un jeu, la colle du module, `jeu::module!` | `no_std` ; `unsafe` seulement sous `#[expect]` (allocateur, exports, le sceau `colle::Hote`) | écrite, testée |
| `jeu-natif` | la copie native d'un jeu, comme `partie::Instance` | un seul `unsafe`, qui construit le jeton de l'hôte ; vide sous `wasm32` | écrite, testée |
| `reecriture` | valider et réécrire un module : carburant, profondeur, NaN, croissance, globales | pure, sans `unsafe` | écrite, testée |
| `moteur-wasmtime` | le module réécrit dans wasmtime, comme `partie::Instance` | sans `unsafe` | écrite, testée |
| `partie` | le cœur de la partie qui fait foi ; le contrat `Instance` ; la fenêtre et la tête ; la validation de la scène | `no_std`, sans entrées-sorties | écrite, testée (§ 8.9, étapes 3, 5 et 6) |
| `epreuve` | la soirée simulée, les contrôles E1 à E19, l'étape 1 qui part des sources (règles d'écriture, module et copie native compilés), le binaire `soiree-epreuve` | sans `unsafe` | E1 à E19 écrits, chacun avec son contrôle négatif, sur le plan court et le plan standard ; les sept étapes du CONTRAT au rapport ; `tests/deux_instances` joue `devine` sur les deux instances |
| `exemples/devine` | le jeu d'exemple (§ 9) ; sa crate de module dans `devine/module` | `no_std` | écrit, testé ; le module se compile, sans import |
| `exemples/ardoise` | un jeu sans téléphone, écran seul ; son état garde des valeurs à bourrage copiées depuis la pile (besoins 1 à 3 de `../conception/BESOINS-DES-JEUX.md`) ; sa crate de module dans `ardoise/module` | `no_std` | écrit, testé ; passe l'épreuve (code 0) |
| `exemples/fautifs` | un petit jeu sain (`Socle<Sain>`), le même avec une fenêtre (`Socle<AvecFenetre>`), et un fautif par contrôle, chacun à une règle près ; leurs crates de module dans `fautifs/modules/<nom>` | `no_std` | écrit ; chaque fautif fait échouer exactement son contrôle, par son module et sa copie native (`epreuve/tests/epreuve`, plan court et plan standard). Depuis ses sources, chacun échoue aussi l'étape 1 : la crate `fautifs` tient les `static` de cinq défauts, que la règle « rien hors de l'état » refuse, et `sain` comme `avec-fenetre` rendent 1 (contrôle final de la phase 2) |

## Le graphe

```
 jeu-derive     jeu-abi         reecriture
       \       /      \             |
         jeu           partie        |
        /   \         /      \       |
 exemples   jeu-natif        moteur-wasmtime
                  \              /
                   \--epreuve--/
```

| Crate | Dépend de, dans le commun |
|---|---|
| `jeu-abi`, `jeu-derive`, `reecriture` | rien |
| `jeu` | `jeu-abi`, `jeu-derive` |
| `partie` | `jeu-abi` |
| `jeu-natif` | `jeu-abi`, `jeu`, `partie` |
| `moteur-wasmtime` | `jeu-abi`, `partie`, `reecriture` |
| `epreuve` | `jeu-abi`, `partie`, `moteur-wasmtime`, `reecriture` ; en test et dans l'exemple `natif`, `jeu-natif`, `devine`, `ardoise`, `fautifs`. Le binaire que l'étape 1 écrit pour un jeu dépend de `epreuve`, `jeu-natif` et de la crate de règles du jeu |
| `devine`, `ardoise`, `fautifs` | `jeu` ; leurs crates de module, `jeu` et leur jeu |

**Le moteur et le cœur ne dépendent jamais de `jeu`** : ils ne connaissent aucun type de jeu (D6).

## Ce qui est figé pour les autres crates

- **`jeu-abi`** : la définition unique des octets échangés. Changer la forme d'un `Appel`, d'un `Retour` ou d'une `EntreeFil` fait rougir `forme_figee` ; un tel changement monte `jeu_abi::ABI` et réécrit les octets attendus de `tests/abi/commun.rs`, à la main. L'ABI 2 a ajouté `Appel::Proprietes` et `Appel::Hypothese` (et leurs retours), à la fin des deux enums, pour E10 et E11 : les octets de l'ABI 1 n'ont pas bougé. L'ABI 3 a ajouté à `ProprietesFil` le nombre de variantes de `Phase` (E3) et les noms d'`Epreuve::INVARIANTS` (E4) : seuls les octets de `Retour::Proprietes` ont changé.
- **`partie::Fabrique::neuve_tete`** : une méthode ajoutée, avec une valeur par défaut (`neuve`), pour la seconde instance qu'une fenêtre fait naître. Une fabrique existante n'a rien à changer ; la sonde de l'épreuve s'en sert pour ne pas juger la tête.
- **`partie::Instance`**, `Fabrique`, `Echec`, `Faute`, `PanneOutil`, `CausePanne` : le contrat entre le cœur et les deux instances. `partie::modele::InstanceModele` en est une implémentation sans WebAssembly, pour les tests du cœur.
- **La zone de panique et les bords de la forme canonique** (`jeu_abi::lire_panique`, `entete_de_panique`, `lire_canonique`) : écrits hors de postcard, définis une fois dans `jeu-abi`, écrits par la colle de `jeu`, lus par le moteur et l'épreuve.

## Les besoins des jeux

Les trois jeux reconstruits ont rapporté ce que le commun ne leur donnait pas. La réponse à chacun, accordé ou refusé avec sa raison, et ce que chaque jeu doit changer, est dans [`../conception/BESOINS-DES-JEUX.md`](../conception/BESOINS-DES-JEUX.md). En bref, pour qui lit le code :

- **`reecriture::VERSION` vaut 3** (`crates/reecriture/src/lib.rs`, qui fait foi et dit ce que change chaque version). La version 2 a ajouté la passe `pile`, qui remet la pile à zéro au début de chaque écriture, sans carburant, et compte le carburant par segment (chaque tête de boucle, branche et jonction) et non plus par fonction entière ; la version 3 fait payer à `memory.init` sa longueur divisée par 64, comme `memory.copy` et `memory.fill`, et compte autant d'étages de profondeur à un appel que le cadre de la fonction appelée en demande.
- **`jeu`** : `Texte::avec_suffixe` garde l'espace du suffixe ; `Cle::nommee`, `Cle::numero`, `IdEffet::nomme` sont `const` ; `Attente::autorite` ; `Construction::demande` ; `Partage::meme` ; un `Vec<u8>` s'écrit d'un bloc dans la forme canonique (mêmes octets) ; un regard d'appareil est illisible pour un jeu sans téléphone.
- **`partie`** : la fraîcheur et l'âge d'un tenu sont des bornes strictes ; une valeur tenue se relâche quand sa dernière redite a dépassé `age_max`.
- **`epreuve`** : les pannes de téléphone sont sans objet pour un jeu sans téléphone ; E11 lit l'hypothèse pendant toute la coupure, dans la soirée sans coupure.
- L'ABI ne changeait pas à cette réponse (`jeu_abi::ABI` valait 2) ; elle vaut 3 depuis la phase 2 (voir « Ce qui est figé pour les autres crates »).

## Ce que la forme scelle, et ce qui reste aux pressions

- **L'épreuve part des sources** (`soiree-epreuve <dossier>`, `src/sources.rs`, `src/ecriture.rs`) : elle vérifie que la crate de module se réduit à `jeu::module!`, que chaque crate du jeu compilée dans le module pose `#![forbid(unsafe_code)]` et est membre du workspace du jeu (une crate par chemin hors de lui échoue ; le code tiers est nommé), ses lints, ses profils, son `clippy.toml`, l'absence d'`allow`, d'un `static` ou d'un `thread_local!` (rien hors de l'état), et d'un `impl Forme`/`Etat`/`Message` écrit à la main (la forme se calcule), puis lance clippy et compile le module et la copie native elle-même ; elle juge enfin ce que les `build.rs` du jeu ont dit à Cargo (seul le maximum de mémoire de la crate de module change la compilation). Un `build.rs` tourne sur la machine sans borne : l'épreuve ne l'enferme pas. La convention de la copie native est dans `../AGENTS.md`.
- **La colle demande le jeton de l'hôte.** `jeu::colle::appeler`, `canonique`, `wasm::boite` et `wasm::appel` prennent un `&colle::Hote`, dont le constructeur est `unsafe` : seuls les exports de `jeu::module!`, la copie native et les tests de la colle en construisent un. La crate de règles, sous `#![forbid(unsafe_code)]`, ne peut pas se rappeler sous le regard d'un joueur, ni relire sa forme canonique pour montrer un secret. Ce sceau ne vaut que si la crate de règles et chaque crate du jeu dont elle dépend sont bien sans `unsafe`, la crate de module réduite à la macro et son `build.rs` limité au maximum de mémoire, et le code tiers sans fonction qui forge un type sans champ : l'étape 1 de l'épreuve le vérifie quand elle part des sources ; un module donné seul (`--wasm`) ne le dit pas, et son code de sortie n'est jamais 0. `jeu-natif` est vide sous `wasm32` : un jeu ne peut pas s'y faire tourner lui-même.
- **Les budgets de E19 sont provisoires, et écrits à un seul endroit** : `epreuve::BUDGETS_E19` (`crates/epreuve/src/budgets.rs`), dérivés du temps d'une image du CONTRAT par des mesures de wasmtime sur la machine de l'hôte (`cargo run --release -p epreuve --example etalonner -- <module.wasm>…` les refait) et une marge choisie pour le navigateur et l'ordinateur de la soirée. Le coût d'un appel y est son carburant plus un coût fixe, qui compte la remise à zéro des zones du module (`moteur_wasmtime::ModuleCharge::octets_remis_a_zero`) : le carburant seul ne borne pas la durée d'un appel. Le rapport les dit provisoires ; c'est le mainteneur qui fixera les budgets dans `CONTRAT.md`.
- **Un regard ne se garde pas.** `Regard` n'est ni `Clone` ni `Copy`, et ne se construit que dans `jeu` : une vue, un bot ou une projection ne l'ont que le temps d'un appel.
- **Un appareil et un message se fabriquent encore.** `Appareil::du_fil` est public (le moteur et le cœur en ont besoin), et `Appareil` se désérialise ; un `impl Message` écrit à la main compile, mais l'étape 1 de l'épreuve le refuse (règle « forme calculée », lecture textuelle). Les raisons, et les pressions qui les attrapent, sont dans `../conception/RESTE-PHASE-1.md`, point 10 ; ce qui reste ouvert après la phase 2, dans `../conception/RESTE-PHASE-2.md`.
- **Un refus ne change pas l'avenir.** E6 rejoue chaque entrée acceptée qui suit des refus sur l'instantané d'avant le premier, et compare retour, forme canonique et vues : un `static` écrit dans `Avant` se voit dès qu'il change ce que fait l'entrée suivante (fautif `retient-un-refus`).
- **E8 trouve un secret d'un octet** qu'un geste ou un tenu écrit : la sonde rejoue l'entrée acceptée avec un autre dernier octet ; si la variante change un secret qu'un appareil étranger ne voit pas, la projection de cet appareil et la vue de chaque regard dont la projection ne change pas doivent rester les mêmes (fautif `choix-qui-fuit`). Un secret tiré du hasard n'est vu que par la recherche d'octets : copié tel quel, s'il fait moins de quatre octets (ou de trois avec sa longueur en tête), ou transformé (en minuscules, en anagramme, par sa longueur, par une couleur), à tout moment, il n'est vu par rien (revue de la phase 2, constat 13).
- **La reprise est éprouvée** : quand la partie tombe en panne (trois `Temps` écartés), la soirée appelle `Partie::repartir`, le script le rejoue (E9, E14) et le rapport le dit (fautif `panique-au-temps`). Une nouvelle partie, comme la création, est un point sain.
- **Le monde simulé ressemble au réel** (`epreuve::Monde`, `src/telephone.rs`, `src/fichiers.rs`) : les bots d'un téléphone décident sur la vue que ce téléphone a reçue (l'instantané de la définitive d'où elle vient), chaque téléphone a son horloge et `fait` porte l'erreur de son estimation, un téléphone sur trois n'a que WebSocket où une perte retient la file, les poignées de main ratent, un onglet rechargé renumérote ses tenus et l'accueil le réaligne (`Partie::numero_tenu`), la page redit son tenu toutes les 250 ms, un onglet passe en arrière-plan, et les demandes de fichiers sont servies avec de vrais contenus, abîmés, trop gros et lents. `Rapport::monde` dit ce que le monde a vraiment fait. `Monde::PHASE_1`, le monde trop sage d'avant, ne sert qu'à montrer que les fautifs `juge-sur-le-fait` (E4) et `fichier-qui-panique` (E13) n'y échouaient pas.
- **La période de lecture de l'hypothèse de E11 est publique** : `jeu::PERIODE_HYPOTHESE` (10 pas).
- **Les soirées ont leurs départs et leurs doublons** : au plan standard, un téléphone part pour de bon (`Panne::Depart`), et le réseau livre deux fois des gestes (`Panne::Doublon`) ; une panne manquée rend ses contrôles « pas exécuté ».
- **E16 compare les pires cas aux soirées** : la vue de l'écran la plus lourde des soirées ne doit pas dépasser de plus de 10 % (`MARGE_DU_PIRE_CAS_POUR_CENT`, choisi) celle du plus lourd des pires cas (fautif `pire-cas-trop-leger`). L'épreuve ne connaît pas les limites propres au jeu : un pire cas plus léger que ses limites, mais plus lourd que ce que les bots atteignent, passe encore.
- **E3 connaît toutes les variantes de `Phase`**, et **E4 chaque invariant déclaré** (`Epreuve::INVARIANTS`) : une phase sans pire cas jamais atteinte (fautif `phase-sans-cas`), un invariant qui n'a jamais tourné (fautif `invariant-jamais-evalue`) échouent.

## Les deux instances, ensemble

`epreuve/tests/deux_instances` mène une soirée de `devine` du même pas sur deux parties qui font foi : l'une sur la copie native, l'autre sur le module compilé pour `wasm32-unknown-unknown`, réécrit et chargé par `moteur-wasmtime`. Fautes injectées et réelles, gel, demande, départ et retour ; à chaque pas, mêmes avancées, mêmes vues, même journal (aux empreintes d'instance près), même forme canonique. Le journal est ensuite rejoué entrée par entrée sur la copie native, deux instances du module réécrit et le module brut, et la partie reprend de trois points sur chaque instance. Une empreinte d'instance ne se compare qu'entre instances de la même sorte ; d'une sorte à l'autre, c'est la forme canonique (`Partie::canonique`) qui se compare.

## Ce que `jeu-abi` porte, et ce qui reste à `jeu`

`jeu-abi` porte ce qui n'a pas de paramètre de jeu et que le moteur ou le cœur doivent lire : temps (`Pas`, `Instant`, `Duree`, `HeureMurale`), appareils et capacités, entrées de l'écran, manette, valeurs validées (`Fini`, `Axe`, `Axe2`, `Borne`, `Cumul`, `Bouton`, `Fixe`, `TexteCourt`, `CodeTouche`, `Places`), clés et effets, la scène sur le fil (`SceneFil`), demandes et réponses, la déclaration possédée (`DeclarationFil`), et toutes les formes `…Fil` de § 3.5.

`jeu` garde ce qui porte un type du jeu, ou un jeton que seul le commun crée : `Entree`, `Geste`, `Changement`, `DeEcran`, `Avant`, `Apres`, `Issue`, `Sorties`, `Hasard`, `Autorite`, `Regard`, `Secret`, `Qui`, `Maintenant`, `Echeance`, `Reveil`, `Scene<A>`, `Montre<V>`, `Definitif<T>`, `CompteARebours`, `Demande<'d>`, `Declaration`, `Applicable<T>`, `Raison`, `Pause`, `Telephones`, `ReglesTenu`, `Asset`, `Texte<L>`, `Limite`, `Appareils<V>`, et les traits. Chacun s'encode avec les mêmes octets que sa forme `…Fil` : `Echeance` comme un `Pas`, `Applicable<T>` comme `ApplicableFil<T>`, `Scene<A>` comme `SceneFil`.

## Les règles du workspace

- Les lints et les profils sont dans `../Cargo.toml` (`[workspace.lints]`), repris de `../outil/regles/`. Chaque membre écrit `[lints] workspace = true`. `unsafe_code` y est refusé ; chaque crate qui n'en a jamais besoin pose `#![forbid(unsafe_code)]`.
- Les dépendances tierces sont déclarées une fois, avec leur raison, dans `[workspace.dependencies]` : un membre écrit `serde.workspace = true` et peut y ajouter des fonctionnalités.
- Une crate de module se compile pour WebAssembly avec le profil `module` (`panic = "abort"`, LTO, débordements vérifiés) :
  `cargo build -p devine-module --target wasm32-unknown-unknown --profile module`. En natif, elle se compile avec `std` et `jeu::module!` n'y écrit rien.
- Un fautif de plus : sa crate de module va dans `exemples/fautifs/modules/<nom>/`, que le workspace prend par motif.
- `../outil/` reste un workspace à part.

## Constaté pour les crates à venir

- **La pile en bas de la mémoire** (§ 3.1). Le 2026-11-03, sur un Mac Apple Silicon (aarch64-apple-darwin), rustc 1.99.1 : un petit module `no_std` compilé pour `wasm32-unknown-unknown`, sans option de lien, a `__stack_pointer` à 1 048 576 et ses données à partir de 1 048 576. La pile est donc déjà en premier, sans `--stack-first` explicite. La passe `valider` de `reecriture` doit le constater sur chaque module (`Refus::PileNonEnBas`), pas le supposer.
- **Pas de maximum de mémoire par défaut.** Le même module déclare 17 pages au moins et aucun maximum. INTERFACE.md (§ 3.6) demande un maximum présent. Tranché : la crate de module le pose par son `build.rs` (`--max-memory`, égal à `limites.memoire`, comme `devine/module/build.rs`), et `reecriture` refuse un module sans maximum (`RefusMemoire::SansMaximum`) sans le poser elle-même.
- **Le décodage de postcard 1.1.3** ne réserve rien pour une suite qui annonce plus d'éléments qu'il ne reste d'octets (`tests/allocations.rs`), et lit un varint non minimal comme sa valeur (`strict::varint_non_minimal_se_lit_comme_sa_valeur`).
