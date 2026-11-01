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

# Portée des validations de v0.2.0

La certification complète à la graine **6839** a réussi sur le candidat
avant la migration des dépendances Cargo vers l’étiquette publiée. Le rapport
`rapport.json` conserve son empreinte de sources exacte :

```text
d44f0b34ce7eac01a052375556b535ac86f0d8e9c06b4de4539cc438d559146c
```

Cette empreinte n’est pas recalculée pour faire passer la migration de
métadonnées pour un nouveau passage navigateur. Les fichiers de rapport
restent inchangés :

| Fichier | SHA-256 |
|---|---|
| `rapport.json` | `e23a04e727c5abf5d54ba3a0fd96adc1d223c8e501176f0b5a6040e22873da20` |
| `rapport.md` | `17ad17b4134a9c534270a28685e9eeae6bf993752a0b0e79db0a51f4a67f661f` |
| `performance.json` | `a36986ea037f630df48f4a356903ce468ed51bb39dc1392ae4dafa9cd4fbab79` |

Après publication du commun, le vrai `soiree upgrade` a aligné le jeu sur
`v0.2.0`. Un snapshot neuf a ensuite été vérifié dans `/tmp`, hors du dossier
de l’hôte et de son `[patch]` local. Les rapports versionnés ont été copiés
sans modification dans ce snapshot, comme dans un clone du dépôt. Les
commandes suivantes ont réussi :

```sh
cargo metadata --all-features --format-version 1
soiree check --locked --published
cargo metadata --locked --all-features --format-version 1
```

La vérification complète donne **0 erreur, 0 avertissement** : règles, CI
commune, modules DOM, contrats UI, formatage, Clippy natif/WASM et tests.
Les 163 paquets registry conservent exactement leurs noms, versions et
sources précédents. Les cinq crates communes (`remote-protocol`,
`salon-core`, `salon-crypto`, `salon-host`, `salon-web`) viennent toutes de
la même source publiée, sans dépendance commune de chemin :

```text
git+https://github.com/build-your-own-game/soiree?tag=v0.2.0#b226c97660773dceb1c880b4dbb20cdac927d5ec
```

Le `Cargo.lock` résolu dans ce snapshot est recopié dans le jeu. Ce passage
établit la compilation et les vérifications avec le commun réellement
publié ; les mesures navigateur restent celles du candidat identifié plus
haut. `release-validation.md` garde la preuve historique de v0.1.0.

Le passage `check-published.json` / `check-published.md` a été exécuté avec
`soiree check --locked --published` dans une copie hors du patch local de
l’hôte, après publication de `v0.3.0`. Ses lockfiles racine et `fuzz/` utilisent
l’étiquette Git publiée ; ses chemins de provenance restent ceux de cette copie.
