# {{nom}}

> Ce fichier s'adresse à l'agent de code qui fait ce jeu, quel qu'il soit. La personne avec qui tu travailles en est l'auteur et le seul maître : ce dépôt est le sien.

## Avant d'écrire du code

Lis, dans `.soiree/` (écrit par `soiree`, à la version de `soiree.toml`) :
- `MANIFESTE.md` : le pari, le niveau d'exigence, les principes. Applique-le.
- `CERTIFICATION.md` : le plancher que tout jeu tient le soir même.
- `salon/README.md` : comment la TV, les téléphones et les manettes rejoignent le jeu. Tu ne touches jamais un socket ni une reconnexion : c'est salon.
- `remote/CONTRAT.md`, seulement si le jeu utilise l'app iPhone.

Si `.soiree/` manque, lance `soiree check` : il le recrée. Si `soiree` manque, lance `soiree doctor` après l'avoir installé (voir le README du dépôt commun).

## Vérifier pendant le développement

**Après chaque changement : `soiree check --fast`.** Ce passage immédiat ne porte que sur les contrôles statiques et signale le reste comme non attesté. Il ne certifie ni les tests natifs ni le comportement de l'interface.

**Avant de valider une étape : `soiree check`.** Il exécute les suites natives, les doctests, Clippy natif/WASM et les contrôles des garanties J. Le passage rapide ne le remplace pas.

La CI fournie est liée à cette version du commun ; ne la retirer ni
l’assouplir. Mettre à niveau avec `soiree upgrade` depuis l’outil de la
version cible. Pour une interface nouvelle, utiliser `salon_web::ui` et
déclarer les contrôles de `cert.json.ui` : ressources possédées, tâches
périmées invalidées, identité et composition des champs préservées, effets
ponctuels identifiés. Une catégorie non applicable cesse de l’être dès que
sa mécanique apparaît.

**La boucle lourde, à la fin d'une étape : `soiree certify`.** 5 à 10 minutes : le vrai serveur, la TV et 8 téléphones dans un navigateur sans écran, coupures, réseau lent, entrées hostiles, endurance. Lance-la en arrière-plan quand l'étape te paraît finie, pas avant. Corrige les échecs produit et les pannes du contrôleur, puis relance le périmètre concerné. Un résultat de revue ou non attesté reste visible et peut rendre le code de sortie non nul : il demande une décision, une observation ou un contrôle manquant, pas une relance pour obtenir artificiellement du vert. Documente ces limites et committe `certification/` avec le code. Un passage ciblé réussi ne valide pas les autres scénarios.

Entre les deux, joue : lance le jeu (`cargo run --release -p {{nom}}-host`), ouvre la TV et des téléphones (`/play?seat=1`, `/play?seat=2`…), regarde des captures. Si ton environnement a un navigateur ou un simulateur iOS, sers-t'en.

## La certification propre à ce jeu

`verification.toml` à la racine déclare ce que **ce jeu-ci** doit tenir, en plus des exigences communes. `CERTIFICATION.md` en présente une vue lisible générée : change le contrat source, pas son bloc généré.

`soiree contracts` valide le contrat et imprime son plan JSON sans exécuter les contrôles ; `soiree contracts --write-view` met à jour la vue lisible. Ces commandes ne produisent pas une réussite de test.

Chaque garantie garde un identifiant stable, une exigence observable et des références à des contrôles. Un contrôle `cargo-test` identifie exactement son paquet, sa catégorie et son nom de cible Cargo, puis le nom complet du test, modules compris. Un helper n'est pas un test : nomme le test qui l'exécute et conserve son rôle dans `assumptions`. Les hypothèses et limites restent explicites ; le texte d'une garantie ne prouve pas qu'elle est tenue.

Ne pars pas d'une page blanche : `.soiree/certifications/` est un catalogue d'items rangés par mécanique (physique, vote, tour par tour, score…), chacun tiré d'un vrai bug de jeu.
1. Repère les mécaniques de ton jeu et lis les fichiers correspondants.
2. Prends **tous** les items qui s'appliquent, adapte-les à ton jeu, et garde leur identifiant dans `origins`. Déclare leur décision `adopted` avec une raison dans `catalogue_decisions`.
3. Un item que tu écartes alors que sa mécanique est dans ton jeu : déclare `not-applicable` avec sa raison dans `catalogue_decisions`. Sans décision explicite, l'item reste non revu ; il ne devient ni satisfait ni non applicable.
4. Refais ce tri quand le jeu gagne une mécanique.

Ajoute ensuite ce que le catalogue n'a pas : chaque règle de ton jeu qui peut casser devient un item. Bons items : ce qu'un joueur pressé, un téléphone en retard ou une égalité peuvent casser.

Ne réutilise jamais un ancien rapport comme résultat courant, et ne déduis pas une réussite de la seule présence d'un test. Les contrôles UI et leurs catégories restent déclarés dans `cert.json.ui` ; ils ne remplacent pas les garanties propres au jeu. Un item retiré conserve son identifiant avec `retired = true`, et `LEARNINGS.md` explique ce qui le couvre désormais.

## Un bug trouvé en jouant

1. Reproduis-le : une suite d'entrées ou un replay, qui devient un test.
2. Écris dans `LEARNINGS.md` pourquoi aucun item ne l'a vu.
3. Ajoute ou durcis une garantie et son contrôle dans le `verification.toml` du jeu. Si la faille n'est pas propre à ton jeu, propose-la au catalogue, sans attendre qu'on te le demande : ouvre une pull request sur le dépôt commun qui ajoute l'item au bon fichier de `certifications/`, au format du catalogue, avec un lien vers le test de ton jeu qui l'a trouvée. L'hôte décide de l'ajouter ou non ; en attendant, la garantie reste dans le `verification.toml` du jeu. Si tu ne peux pas ouvrir de pull request toi-même, écris-en le contenu pour l'auteur.

## Ce que tu ne modifies pas

- Le dépôt commun (salon, la télécommande, les règles, le harnais). S'il manque quelque chose à salon, ou si une règle commune gêne sans raison, ouvre une issue sur `https://github.com/build-your-own-game/soiree` qui explique le besoin et ce que tu as essayé. Si tu ne peux pas ouvrir d'issue toi-même, écris-en le texte pour l'auteur.
- `rust-toolchain.toml`, `clippy.toml` et les lints de `Cargo.toml` : `soiree` les écrit. Tu peux durcir une règle ; pour en assouplir une, il faut `#[expect(lint, reason = "…")]` à l'endroit précis, jamais `#[allow]`.
- `.soiree/` : régénéré à chaque `soiree check`.
- Le module écrit par `soiree dom` (souvent `page.rs`) : il tire du HTML et du CSS de la page les seuls noms que le Rust peut y lire, et les règles interdisent de lire la page par chaîne ailleurs. Change la page, puis relance `soiree dom` suivi du chemin du module.
- Le `build.rs` copié de salon (`version.rs`) : il calcule depuis les sources la version que salon vérifie ; aucune version à monter à la main. `salon/README.md` dit comment brancher l'un et l'autre.

## Travailler avec l'auteur

- **Il ne code peut-être pas.** Demande-le-lui au début s'il ne l'a pas dit. S'il ne code pas, c'est toi le développeur : parle-lui du jeu (règles, sensation, ce qui fait rire), pas de Rust. Montre-lui le jeu qui tourne plutôt que du code.
- **Le jeu reste jouable à tout moment.** Avance par petites couches qui marchent.
- **Committe et pousse souvent,** avec des messages qui disent ce qui change pour le joueur. Ce dépôt est sur GitHub : c'est sa sauvegarde et c'est là que l'hôte le récupère.
- **Pour que le jeu soit proposé à la soirée,** il doit être dans `catalogue.toml` du dépôt commun : ouvre une pull request qui y ajoute une ligne, une seule fois.

## Ce jeu

*À remplir par toi, au fil du projet : en une phrase, ce qu'est le jeu ; les commandes pour le lancer et le tester ; ce que contient chaque crate ; les décisions qu'un autre agent doit connaître avant d'y toucher.*
