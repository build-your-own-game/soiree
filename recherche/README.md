# Recherche

Ce dossier rend mesurable la méthode de `../MANIFESTE.md` (« Extraire le signal du code existant ») : une fiche par sous-système, des citations vérifiables, chaque leçon reliée à un test ou à un trou. `LEARNINGS.md` garde ce qu'on a retenu ; ici, on garde la preuve de la recherche.

| Fichier | Rôle |
|---|---|
| `sous-systemes.toml` | les sous-systèmes, leurs chemins, leur niveau, leurs marches ; les quotas |
| `sources.toml` | les références candidates de chaque sous-système |
| `<sous_systeme>.toml` | la fiche ; `reconnexion.toml` sert de modèle |

## Le processus

1. **Déclencheur.** Ton changement ajoute du nouveau sous un chemin de `sous-systemes.toml` : fichier, type ou fonction publique, dépendance, délai, message de protocole. Sinon, pas de recherche à faire.
2. **Niveau.** Prends le niveau du sous-système. S'il y en a plusieurs, prends le plus haut. Le quota vient de `[[niveau]]`.
3. **Fiche.** Ouvre `<sous_systeme>.toml`, ou crée-la avec `sous_systeme`, `niveau`, une `question` réfutable et `compare_a`, le commit que tu compares.
4. **Chez nous d'abord.** Lis le code et les tests concernés en entier, puis les `LEARNINGS.md`. Écris tes hypothèses de trous avant de lire ailleurs.
5. **Choisis les sources** dans `sources.toml`, en montant l'échelle : `chez-nous`, `reel`, `normes`, `bancs`, `retours`. Ajoute à `sources.toml` ce que tu trouves en route.
6. **Lis l'histoire avant le code.** Dans le scratchpad, jamais dans ce dépôt : `git clone --filter=blob:none`, `git log --grep`, `git log -S`, `gh search issues <mot> --repo <dépôt> --sort comments`. Note dans `consulte` ce que tu as réellement lu, et le commit dans `commit`.
7. **Une leçon par fait.** Remplis un `[[lecon]]` : `lien` figé, `citation` copiée de la source, moins de 15 mots, un seul extrait par document. Trie-la par les quatre catégories du manifeste.
8. **Compare au vrai code.** `etat` vaut `couvert` (nomme dans `garantie` le test ou l'item), `trou`, ou `non-applicable` (écris la `raison`).
9. **Éprouve chaque trou.** Écris une sonde sur une copie de `salon/` hors du dépôt. Le cas doit échouer, puis passer en changeant la seule cause supposée. Sépare `verifie` de `suppose` dans `[[trou]]`. Ne corrige pas dans le même commit.
10. **Arrête-toi** quand le quota est atteint **et** que les 3 dernières sources lues n'ont apporté aucune leçon nouvelle. Liste-les dans `[saturation]`, avec ce qu'elles recoupent, et remplis `[cout]`.
11. **Vérifie** chaque `citation` contre son `lien`, par une machine (`gh api`, `git show`), puis mets à jour `LEARNINGS.md` si tu y travailles.

La dette de recherche ne bloque qu'à l'étiquette du commun : un jeu n'attend jamais une fiche.
