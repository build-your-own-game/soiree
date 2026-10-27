# Certification du modèle

Passage complet du 27 octobre 2026, graine `20261027`, quatre téléphones
WebKit et quatre Chromium. `rapport.json` indique un passage réussi et des
sources et un harnais inchangés pendant le test.

Le modèle a été rendu avec `nom = "modele-test"`,
`org = "build-your-own-game"` et `version = "0.1.0"`. Les règles embarquées
ont été ajoutées comme le fait `soiree new`. Pour ce passage avant publication,
les trois dépendances salon de son `Cargo.toml` utilisent les chemins locaux
`../soiree/salon/crates/{core,host,web}` à la place de l’étiquette non publiée.
Le dossier de test `.chantier-modele` était un dépôt Git initialisé localement ;
il est supprimé après conservation de ces rapports. Son empreinte couvre les
fichiers rendus, pas les sources avec les placeholders de `modele/`.

Les captures détaillées restent dans le dossier temporaire indiqué au rapport.
Ce passage automatisé ne remplace ni le test sur de vrais iPhone ni le jugement
des textes et du jeu dans un salon.
