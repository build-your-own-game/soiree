## Contrats de vérification 0.3.0 (non publiée)

`check.json` décrit le périmètre des vérifications du code. Les nouveaux
`verification*.json` et `harness-observations*.json` décrivent leurs campagnes
et leurs limites ; `harness-performance*.json` garde leurs mesures. Un fichier
suffixé `-full` conserve le passage complet avant une observation ciblée plus
récente. Lire leur sélection et leur identité, sans déduire un résultat global
d’un seul code de sortie.

Les fichiers `rapport.json`, `rapport.md` et `performance.json` sont historiques
et ont été conservés à l’identique par cette migration. Ils ne constituent pas
une validation du code présent. Les notes ci-dessous gardent leur contexte
historique.

# Certification de Sidequest — commun v0.2.0

Le [rapport complet](rapport.md), ses [métadonnées](rapport.json) et les
[mesures](performance.json) conservent le passage vert du **28 octobre 2026**,
graine **6839**, avec quatre téléphones WebKit et quatre Chromium. Le serveur,
les pages et le harnais sont restés inchangés pendant ce passage.

Ce passage précède la migration des dépendances Cargo vers l’étiquette
publiée : il utilisait le commun 0.2.0 du dossier de l’hôte. Son empreinte
`25e6ed1f8015748421b8d602567fefcd9530089bf14392f8ae0ac5d5baaccd51`
et celle du harnais
`f33beed61e5356fbf86744ca84f9245c6d40fe3e76e20cb68763071ff6f7cd49`
restent celles de ce passage ; elles ne sont pas réécrites pour prétendre
certifier les manifestes migrés.

## Vérification autonome après publication

`soiree upgrade` a été exécuté dans le dépôt, puis les deux dépendances
directes de membres du workspace (`remote-web` et `remote-protocol`) ont été
alignées sur **v0.2.0**. Aucun code applicatif n’a changé pour cette migration.

Une copie neuve hors du dossier de l’hôte a ensuite été vérifiée dans
`/private/tmp/soiree-sidequest-v020-final-1793154719870`. Aucun remplacement
local Cargo n’y est installé. Le dossier `target` réutilise seulement les
artifacts de compilation ; il ne remplace aucune dépendance.

Les lockfiles source ont été conservés en entrée de la résolution : les
**230 entrées registry** de la racine et les **47 du fuzz** gardent leurs
versions et checksums. Les commandes suivantes y ont terminé avec le code 0 :

```sh
cargo metadata --all-features --format-version 1
cargo metadata --manifest-path fuzz/Cargo.toml --all-features --format-version 1
cargo metadata --manifest-path fuzz/Cargo.toml --all-features --locked --format-version 1
soiree check --locked --published
```

Le dernier contrôle complet indique **0 erreur, 0 avertissement** : règles,
CI épinglée, modules DOM, contrats UI, formatage, Clippy natif et WASM et
tests. Le graphe racine utilise `remote-protocol`, `remote-web`, `salon-core`,
`salon-crypto`, `salon-host` et `salon-web` ; le fuzz utilise `salon-core`.
Toutes ces crates sont en 0.2.0 et leur source effective est :

```text
git+https://github.com/build-your-own-game/soiree?tag=v0.2.0#b226c97660773dceb1c880b4dbb20cdac927d5ec
```

Les deux lockfiles ainsi validés ont été recopiés dans le dépôt, sans relancer
Cargo dans le dossier de l’hôte après cette copie. Leurs SHA-256 sont :

- `Cargo.lock` : `11456de78a3f4c48276284ff928ae3b4d7c7ef49d5f9ae70e17efc426d5fa351`
- `fuzz/Cargo.lock` : `ce5569cae59500cda1847003115047d724a0b3d9824e75a7e9befdf8447e3bb2`

Les preuves locales sont dans `/tmp/soiree-sidequest-v020-published-check.log`,
`/tmp/soiree-sidequest-v020-root-metadata.json`,
`/tmp/soiree-sidequest-v020-fuzz-locked-metadata.json` et
`/tmp/soiree-sidequest-v020-verification.json`.
L’[ancienne validation v0.1.0](release-validation.md) est conservée comme archive.

Le passage `check-published.json` / `check-published.md` a été exécuté avec
`soiree check --locked --published` dans une copie hors du patch local de
l’hôte, après publication de `v0.3.0`. Ses lockfiles racine et `fuzz/` utilisent
l’étiquette Git publiée ; ses chemins de provenance restent ceux de cette copie.
