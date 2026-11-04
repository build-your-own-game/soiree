# Les crates du commun

La conception est dans [`../conception/INTERFACE.md`](../conception/INTERFACE.md) (§ 8 pour ce découpage) ; le contrat dans [`../CONTRAT.md`](../CONTRAT.md).

## La carte

| Crate | Rôle | Forme | État |
|---|---|---|---|
| `jeu-abi` | l'enveloppe `Appel`/`Retour`/`EntreeFil` et le vocabulaire du commun, validé au décodage ; la zone de panique et les bords de la forme canonique | `no_std`, sans `unsafe` | écrite, testée |
| `jeu-derive` | `#[jeu::message]`, `#[jeu::etat]`, `#[derive(Variantes)]` | macro procédurale | écrite, testée |
| `jeu` | ce qu'écrit l'auteur d'un jeu, la colle du module, `jeu::module!` | `no_std` ; `unsafe` seulement sous `#[expect]` (allocateur, exports) | écrite, testée |
| `jeu-natif` | la copie native d'un jeu, comme `partie::Instance` | sans `unsafe` | écrite, testée |
| `reecriture` | valider et réécrire un module : carburant, profondeur, NaN, croissance, globales | pure, sans `unsafe` | écrite, testée |
| `moteur-wasmtime` | le module réécrit dans wasmtime, comme `partie::Instance` | sans `unsafe` | écrite, testée |
| `partie` | le cœur de la partie qui fait foi ; le contrat `Instance` ; la fenêtre et la tête ; la validation de la scène | `no_std`, sans entrées-sorties | écrite, testée (§ 8.9, étapes 3, 5 et 6) |
| `epreuve` | la soirée simulée, les contrôles E1 à E19, le binaire `soiree-epreuve` | sans `unsafe` | E1 à E19 écrits, chacun avec son contrôle négatif ; `tests/deux_instances` joue `devine` sur les deux instances |
| `exemples/devine` | le jeu d'exemple (§ 9) ; sa crate de module dans `devine/module` | `no_std` | écrit, testé ; le module se compile, sans import |
| `exemples/ardoise` | un jeu sans téléphone, écran seul ; son état garde des valeurs à bourrage copiées depuis la pile (besoins 1 à 3 de `../conception/BESOINS-DES-JEUX.md`) ; sa crate de module dans `ardoise/module` | `no_std` | écrit, testé ; passe l'épreuve (code 0) |
| `exemples/fautifs` | un petit jeu sain (`Socle<Sain>`), le même avec une fenêtre (`Socle<AvecFenetre>`), et un fautif par contrôle, chacun à une règle près ; leurs crates de module dans `fautifs/modules/<nom>` | `no_std` | écrit ; chaque fautif fait échouer exactement son contrôle (`epreuve/tests/epreuve`) |

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
| `epreuve` | `jeu-abi`, `partie`, `moteur-wasmtime`, `reecriture` ; en test et dans l'exemple `natif`, `jeu-natif`, `devine`, `ardoise`, `fautifs` |
| `devine`, `ardoise`, `fautifs` | `jeu` ; leurs crates de module, `jeu` et leur jeu |

**Le moteur et le cœur ne dépendent jamais de `jeu`** : ils ne connaissent aucun type de jeu (D6).

## Ce qui est figé pour les autres crates

- **`jeu-abi`** : la définition unique des octets échangés. Changer la forme d'un `Appel`, d'un `Retour` ou d'une `EntreeFil` fait rougir `forme_figee` ; un tel changement monte `jeu_abi::ABI` et réécrit les octets attendus de `tests/abi/commun.rs`, à la main. L'ABI 2 a ajouté `Appel::Proprietes` et `Appel::Hypothese` (et leurs retours), à la fin des deux enums, pour E10 et E11 : les octets de l'ABI 1 n'ont pas bougé.
- **`partie::Fabrique::neuve_tete`** : une méthode ajoutée, avec une valeur par défaut (`neuve`), pour la seconde instance qu'une fenêtre fait naître. Une fabrique existante n'a rien à changer ; la sonde de l'épreuve s'en sert pour ne pas juger la tête.
- **`partie::Instance`**, `Fabrique`, `Echec`, `Faute`, `PanneOutil`, `CausePanne` : le contrat entre le cœur et les deux instances. `partie::modele::InstanceModele` en est une implémentation sans WebAssembly, pour les tests du cœur.
- **La zone de panique et les bords de la forme canonique** (`jeu_abi::lire_panique`, `entete_de_panique`, `lire_canonique`) : écrits hors de postcard, définis une fois dans `jeu-abi`, écrits par la colle de `jeu`, lus par le moteur et l'épreuve.

## Les besoins des jeux

Les trois jeux reconstruits ont rapporté ce que le commun ne leur donnait pas. La réponse à chacun, accordé ou refusé avec sa raison, et ce que chaque jeu doit changer, est dans [`../conception/BESOINS-DES-JEUX.md`](../conception/BESOINS-DES-JEUX.md). En bref, pour qui lit le code :

- **`reecriture::VERSION` vaut 2.** La passe `pile` remet la pile à zéro au début de chaque écriture, sans carburant ; le carburant se compte par segment (chaque tête de boucle, branche et jonction), et non plus par fonction entière.
- **`jeu`** : `Texte::avec_suffixe` garde l'espace du suffixe ; `Cle::nommee`, `Cle::numero`, `IdEffet::nomme` sont `const` ; `Attente::autorite` ; `Construction::demande` ; `Partage::meme` ; un `Vec<u8>` s'écrit d'un bloc dans la forme canonique (mêmes octets) ; un regard d'appareil est illisible pour un jeu sans téléphone.
- **`partie`** : la fraîcheur et l'âge d'un tenu sont des bornes strictes ; une valeur tenue se relâche quand sa dernière redite a dépassé `age_max`.
- **`epreuve`** : les pannes de téléphone sont sans objet pour un jeu sans téléphone ; E11 lit l'hypothèse pendant toute la coupure, dans la soirée sans coupure.
- L'ABI ne change pas (`jeu_abi::ABI` vaut toujours 2).

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
