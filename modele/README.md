# {{nom}}

Un petit jeu jouable : jusqu’à huit téléphones appuient, la TV compte. Remplace cette
mécanique en gardant les frontières de simulation, de rendu et de réseau.

- `cargo run --release -p {{nom}}-host` construit les pages et lance la TV sur
  http://localhost:8090 ; l’adresse des téléphones et son QR sont sur la TV.
- `GAME_PORT=8091` choisit un autre port ; `GAME_DATA` choisit le dossier local
  des identités et des journaux (data par défaut, ignoré par Git).
- `cargo run --release -p {{nom}}-host -- --replay <journal.jsonl>` rejoue et
  compare l’état complet. F8 sur la TV marque un bug. Les journaux réels sont
  purgés après trente jours ; les fixtures versionnées utilisent des noms fictifs.
- `soiree check` passe les règles et les tests. `soiree certify` attaque la TV
  et les téléphones Chromium/WebKit et mesure les budgets.
- `SOIREE_SIM_SEEDS=10000 cargo test whole_host` prolonge la campagne rapide
  de 2048 graines. `soiree mutants --re next_score` vérifie à la demande que les
  tests attrapent les mutations du compteur ; ce travail reste hors des checks.

`crates/core` contient la simulation, ses messages et les replays sans écran.
`crates/host` adapte les appareils salon et écrit les entrées après fixation de
leur ordre. `crates/web` dessine les vues et envoie les actions ; le module
page est généré depuis les fichiers de `web/`. Le réseau reste dans salon.

Lis `AGENTS.md`, puis les documents de `.soiree/`, avant de modifier le jeu.

La CI `.github/workflows/soiree.yml` est liée à la version `{{version}}` du
commun. Elle vérifie les dépendances publiées avec
`soiree check --locked --published`, puis exerce `ui` et la récupération
après panique. `soiree upgrade --dry-run` montre le passage à la version de
l’outil installé ; `soiree upgrade` l’applique après vérification de son
étiquette publiée. Adapter ensuite le code signalé par les contrôles.

Pour ajouter des interfaces, conserver `Listeners`, `Timer` et
`AnimationLoop` avec leur vue ; protéger les tâches par `OperationSlot` ou
`OperationScope` et préserver les éditeurs et leur composition. Décrire les
contrôles dans `cert.json.ui`. Le modèle n’a pas d’éditeur ni d’import :
remplacer leurs raisons de non-applicabilité dès que ces fonctions existent.

Après une modification de l’interface, sur un hôte de test frais :
`node tools/press-check.mjs http://localhost:8090 <dossier-du-harnais>`.
Ce test réutilise le navigateur du harnais et vérifie qu’un appui en cours
reste compté quand un autre joueur fait changer la vue.
