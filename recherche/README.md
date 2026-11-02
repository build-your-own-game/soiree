# Recherche

Ce dossier rend mesurable la méthode de `../MANIFESTE.md` (« Extraire le signal du code existant ») : une fiche par sous-système, des citations vérifiables, chaque leçon reliée à un test ou à un trou. `LEARNINGS.md` garde ce qu'on a retenu ; ici, on garde la preuve de la recherche.

| Fichier | Rôle |
|---|---|
| `sous-systemes.toml` | les sous-systèmes, leurs chemins, leur niveau, leurs marches ; les quotas |
| `sources.toml` | les références candidates de chaque sous-système |
| `<sous_systeme>.toml` | la fiche ; `reconnexion.toml` sert de modèle |

## Le processus

1. **Déclencheur.** Lance `soiree recherche plan`. Ton changement ajoute du nouveau sous un chemin de `sous-systemes.toml` : fichier, type ou fonction publique, dépendance, délai, message de protocole. Un bug échappé compte aussi, et le diff ne le voit pas. Sinon, pas de recherche à faire : un correctif ordinaire n'en crée pas.
2. **Niveau.** Prends le niveau du sous-système. S'il y en a plusieurs, prends le plus haut. Le quota vient de `[[niveau]]`. Tu peux relever le niveau ; pour le baisser, écris pourquoi dans `raison_niveau`, comme un `#[expect]`. Une raison qui ne dit rien (`x`) est refusée, et la dette part toujours du plancher de `sous-systemes.toml` : une fiche baissée reste une ligne à solder ou à justifier.
3. **Fiche.** Ouvre `<sous_systeme>.toml`, ou crée-la avec `sous_systeme`, `niveau`, une `question` réfutable et `compare_a`, le commit que tu compares.
4. **Chez nous d'abord.** Lis le code et les tests concernés en entier, puis les `LEARNINGS.md`. Écris tes hypothèses de trous avant de lire ailleurs.
5. **Choisis les sources** dans `sources.toml`, en montant l'échelle : `chez-nous`, `reel`, `normes`, `bancs`, `retours`. Ajoute à `sources.toml` ce que tu trouves en route.
6. **Lis l'histoire avant le code.** Dans le scratchpad, jamais dans ce dépôt : `git clone --filter=blob:none`, `git log --grep`, `git log -S`, `gh search issues <mot> --repo <dépôt> --sort comments`. Note dans `consulte` ce que tu as réellement lu, et le commit dans `commit`.
7. **Une leçon par fait.** Remplis un `[[lecon]]` : `lien` figé, `citation` copiée de la source, moins de 15 mots, un seul extrait par document. Trie-la par les quatre catégories du manifeste.
8. **Compare au vrai code.** `etat` vaut `couvert` (nomme dans `garantie` le test ou l'item), `trou`, ou `non-applicable` (écris la `raison`). Une leçon reprise d'une issue ou d'une page exige `vu_le` sur sa référence.
9. **Éprouve chaque trou.** Écris une sonde sur une copie de `salon/` hors du dépôt. Le cas doit échouer, puis passer en changeant la seule cause supposée. Sépare `verifie` de `suppose` dans `[[trou]]`. Ne corrige pas dans le même commit. Le correctif passe la leçon à `couvert`, nomme sa `garantie` et garde `trou` : c'est ce qui ferme le trou.
10. **Arrête-toi** quand le quota est atteint **et** que les 3 dernières sources lues n'ont apporté aucune leçon nouvelle. Liste-les dans `[saturation]`, avec ce qu'elles recoupent, et remplis `[cout]`. Elles couvrent la nature des sources : au moins une référence `reel`, et au moins une avec `nature = "code-client"` quand le sous-système a un client. Trois serveurs ou trois normes ne disent rien du client.
11. **Vérifie.** `soiree check .` juge la fiche hors ligne ; `soiree recherche verifie` confronte chaque commit et chaque `citation` à l'amont. Publie d'abord les commits que tu cites, puis mets à jour `LEARNINGS.md` si tu y travailles.

La dette de recherche ne bloque qu'à l'étiquette du commun : un jeu n'attend jamais une fiche.

## Les commandes

| Commande | Réseau | Ce qu'elle fait |
|---|---|---|
| `soiree recherche plan` | non | lit le diff depuis la dernière étiquette, ou un autre avec `--depuis REF` ou `--git-diff`, et dit ce qui est nouveau, le sous-système, le niveau, la fiche, ce qui manque, puis ces étapes ; c'est une aide, elle n'empêche rien |
| `soiree check .` | non | à la racine du commun : format et règles de chaque fiche, quotas, saturation, marches, et existence des tests et items cités en `garantie`. Avec `--fast`, ou dans un jeu : une ligne de dette au plus, et rien qui bloque |
| `soiree recherche verifie` | oui | chaque commit existe en amont, chaque citation est à l'endroit indiqué, chaque `empreinte` tient. Sort à 1 si une fiche est fausse, ou s'il n'y a aucune fiche : rien vérifié n'est pas une réussite. Sort à 2 sur une panne, qui ne conclut rien, et une réponse qui n'est pas le commit demandé, comme un portail Wi-Fi, en est une |
| `soiree recherche dette` | non | les sous-systèmes de niveau 3 sans fiche, les fiches sous leur plancher, puis les trous ouverts par gravité ; `AGENTS.md`, « Avant d'étiqueter », dit le seul endroit où elle bloque |

`soiree check .` lit le texte : il dit qu'un test existe, pas qu'il tourne ni qu'il passe.
