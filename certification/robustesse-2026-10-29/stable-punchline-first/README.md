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

# Provenance des validations

Le [rapport complet](rapport.md), son [enregistrement JSON](rapport.json) et
les [mesures de performance](performance.json) proviennent du passage à la
graine **6839**, du 28 octobre 2026, de 02:06:47 à 02:11:59 UTC. Il est vert
(`passed: true`, code de sortie 0). Le commun était alors fourni par le
`[patch]` local de l'hôte, comme l'indique encore le rapport.

Ce passage précède la migration des dépendances Cargo vers l'étiquette
publiée `v0.2.0`. Son empreinte
`833fde2198ba40cdfdf4282a5680a2f2b5e5d580106d1c113e2448d94e9e3227`
reste volontairement inchangée : elle décrit les fichiers du passage
effectivement exécuté, avant la modification de `Cargo.toml`, `soiree.toml`
et `Cargo.lock`. Les mesures conservent cette provenance.

La migration a sa validation autonome : une copie neuve hors du dossier de
l'hôte, résolue avec `cargo metadata --all-features --format-version 1`,
puis contrôlée intégralement avec :

```sh
soiree check --locked --published
```

Ce contrôle vérifie notamment le formatage, Clippy natif et WASM, les tests,
les noms DOM et les contrats du jeu. Les cinq crates communes résolues
(`remote-protocol`, `salon-core`, `salon-crypto`, `salon-host`, `salon-web`)
viennent toutes du Git `v0.2.0`, au commit publié
`b226c97660773dceb1c880b4dbb20cdac927d5ec`. Les **163 paquets registry**
gardent les mêmes versions et sommes de contrôle lors du passage du chemin
local au Git. Le lockfile de cette copie autonome est celui conservé dans
le dépôt du jeu.

Pour reproduire ce contrôle de publication, utiliser un clone hors du
dossier hôte et de son `[patch]` local. Les mesures de cadence et de latence
restent celles du passage complet antérieur.

Le passage `check-published.json` / `check-published.md` a été exécuté avec
`soiree check --locked --published` dans une copie hors du patch local de
l’hôte, après publication de `v0.3.0`. Ses lockfiles racine et `fuzz/` utilisent
l’étiquette Git publiée ; ses chemins de provenance restent ceux de cette copie.
