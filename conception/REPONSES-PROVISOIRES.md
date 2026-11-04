# Réponses provisoires aux questions d'INTERFACE.md

> **Statut.** Ces réponses ne sont pas celles du mainteneur. Elles permettent de construire la phase 1 sans attendre. Chacune dit ce qui changerait si le mainteneur répondait autrement. Elles se remplacent par sa réponse dès qu'il l'a donnée.

| # | Question (§ 13) | Réponse provisoire | Ce qui changerait sinon |
|---|---|---|---|
| 1 | Les gros états | La mémoire entière reste l'état (D23). Les points de reprise de Tier list sont gros ; on le mesure. | Une zone de blocs immuables nommés par empreinte, à ajouter dans `jeu-abi` et le moteur. |
| 2 | Le suivi des pages écrites | Non pour la phase 1 (D38). | Rien dans l'interface : une accélération du moteur. |
| 3 | La fréquence | 60 pas par seconde pour tous (D13). | `Declaration` gagne une fréquence ; `Duree::ms` devient une fonction de la fréquence. |
| 4 | Le gel | Le numéro de pas saute (D14). | La règle de `partie` sur le rattrapage. |
| 5 | La prédiction des autres | Sans objet en phase 1 : la prédiction sur appareil vient en phase 2. | — |
| 6 | La fenêtre de Sidequest | Zéro. | Une déclaration du jeu. |
| 7 | Les relâchements forcés de Sidequest | On suit le cahier de Sidequest. | Une règle de l'étape de Sidequest. |
| 8 | La configuration au lancement | Aucune : le contenu est compilé dans le jeu. | `creer` reçoit une configuration. |
| 9 | Le second onglet de Tier list | Sans objet en phase 1 (pas de navigateur). | — |
| 10 | Les gestes qu'exige le navigateur | Sans objet en phase 1. | — |
| 11 | Les rôles | Écran, téléphone, manette. | Des rôles déclarés par le jeu. |
| 12 | Les budgets | Les agents proposent des valeurs mesurées sur les trois jeux ; le mainteneur les fixe dans `CONTRAT.md`. | — |
| 13 | Un second moteur dans l'épreuve | Pas en phase 1 : la comparaison se fait entre le module et la copie native. | Une crate `moteur-wasmi`, qui implémente `partie::Instance`. |
| 14 | Les touches du commun | `F8`, et une touche de diagnostic déclarée par le jeu. | — |
| 15 | Le code de salle | Le commun le donne au jeu dans `Creation`. | Le commun l'ajoute seul aux lignes du flux. |
| 16 | Les anciennes sauvegardes de Tier list | Non. | Une conversion écrite par le jeu. |
| 17 | Les effets provisoires | Sans objet en phase 1 (pas de son). | — |
| 18 | La première preuve | Les `regles` du cœur de la partie (`partie::regles`) : ordre, fenêtre, événement compté une fois, reprise. | — |
| 19 | La déclaration dans le module | Plus tard. | — |
| 20 | L'emplacement | `soiree/crates/`, un seul workspace, le nom `jeu` reste : les jeux en dépendent par git, pas par crates.io. | Les noms des crates. |
