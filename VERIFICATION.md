# Contrats de vérification

Ce document décrit le mécanisme de la vérification.
Le [manifeste](MANIFESTE.md) porte l’ambition ; le
[socle commun](CERTIFICATION.md) décrit les exigences. Ici, on précise ce que
l’outil sait effectivement établir, sur quelles entrées et avec quelles limites.

## Du contrat au résultat

Le système garde cinq objets séparés :

| Objet | Rôle | Source |
|---|---|---|
| Exigence | Ce qui doit être vrai et pourquoi | Socle commun, engagements du jeu, catalogue |
| Déclaration | Ce que le jeu promet, les contrôles proposés, ses hypothèses | `verification.toml` et configuration `cert.json` |
| Plan | Obligations applicables, contrôles attendus, méthode et périmètre de chacun | Résolution par l’outil |
| Observation | Ce qu’un contrôleur a réellement exécuté et constaté | Cargo/libtest, contrôles statiques, harnais |
| Évaluation | Ce que ces observations permettent de conclure pour ce plan | Évaluateur commun et noyau de décision |

Une déclaration ajoute des engagements. Elle ne peut pas remplacer les minima
communs ni déclarer elle-même qu’un contrôle a réussi. `CERTIFICATION.md` du jeu
contient une vue générée des engagements ; les résultats datés vivent dans
`certification/`. Modifier la vue ne produit aucune observation.

## Écrire le contrat du jeu

Le format actuel est TOML, `schema = 1`. Un engagement actif a un identifiant
`J` suivi de chiffres, une affirmation et au moins un contrôle déclaré. Exemple
pour un jeu dont le paquet Cargo s’appelle `mon-jeu-core` :

```toml
schema = 1

[[guarantees]]
id = "J1"
claim = "Le même vote ne modifie le score qu’une fois."
checks = ["vote-unique"]
origins = []
assumptions = ["Les votes portent un identifiant stable pendant la manche."]

[[checks]]
id = "vote-unique"
kind = "cargo-test"
package = "mon-jeu-core"
target_kind = "lib"
target = "mon_jeu_core"
name = "tests::a_duplicate_vote_does_not_score_twice"
features = []
default_features = true
profile = "dev"
```

Le nom de cible est celui de Cargo ; le nom du test est son chemin complet dans
libtest. Pour une cible binaire, utiliser `target_kind = "bin"` et le nom du
binaire ; pour un fichier `replay.rs` du dossier `tests`, utiliser `target_kind = "test"`,
`target = "replay"` et le nom complet annoncé par cet exécutable.
Paquet, genre de cible, cible, nom, features, activation des features par défaut
et profil forment ensemble l’identité. Les features sont triées et dédupliquées.
Les trois derniers champs valent par défaut `[]`, `true` et `"dev"`.

```sh
soiree contracts                 # affiche le plan, sans exécuter ses contrôles
soiree contracts --write-view    # actualise la vue dans CERTIFICATION.md
soiree check                    # produit de nouvelles observations
```

Les contrôles déclarables par un jeu sont actuellement des `cargo-test`.
Une preuve, une mesure ou un script arbitraire ne deviennent pas un tel test en
changeant leur étiquette. Les extensions de méthodes demandent un adaptateur
commun qui définit son oracle et les données nécessaires à son évaluation.

### Relier une exigence commune

Sept exigences du socle, marquées « à relier » dans
[`CERTIFICATION.md`](CERTIFICATION.md), dépendent des règles de chaque jeu : le
commun ne sait pas les exécuter. Ce sont C6, E5, T1, D1, D3, D4 et D5. Un jeu
les relie à ses propres contrôles dans `verification.toml` :

```toml
[[bindings]]
requirement = "D1"
checks = ["replay-deterministe"]
assumptions = ["Le replay couvre une partie entière, de la graine à la fin."]

[[checks]]
id = "replay-deterministe"
kind = "cargo-test"
package = "mon-jeu-core"
target_kind = "test"
target = "replay"
name = "same_inputs_give_the_same_states"
```

`requirement` et `checks` sont obligatoires, `assumptions` est facultatif ; tout
autre champ est refusé. Un contrôle n’a pas besoin d’une garantie `J` s’il est
cité par une liaison. Plusieurs liaisons et garanties peuvent citer le même
contrôle, qui ne s’exécute qu’une fois. La vue du jeu liste ses liaisons :
`soiree contracts --write-view` l’actualise.

La validation est stricte et fait échouer `soiree contracts` et `soiree check` :
un contrôle inconnu ou répété, une liste vide, une exigence reliée deux fois,
une exigence hors des sept. Une liaison invalide ne relie rien. Une exigence
portée par un scénario commun, comme C1 ou P1, ne se relie pas : une liaison
ajoute des contrôles à une exigence qui n’en avait pas et ne retire jamais un
minimum commun. Une garantie `J` au texte voisin ne relie rien non plus.

Relier C6, T1, D4 ou D5, dont l’application dépend du jeu, déclare que le jeu
est concerné : l’exigence devient applicable et ses contrôles décident. Il
n’existe pas de déclaration inverse : une exigence conditionnelle non reliée
reste inconnue, jamais écartée. Une liaison compte comme tout contrôle, par
l’observation de son test pour les entrées courantes ; ignoré, absent ou en
échec, il ne la satisfait pas. Elle nomme des contrôles sans établir qu’ils
couvrent toute l’exigence, et le rapport garde cette hypothèse.

## Catalogue, applicabilité et décisions

Les 97 items du [catalogue](certifications/README.md) sont une réserve
d’enseignements, à examiner selon les mécaniques du jeu. Ils ne sont pas 97
exigences universelles et ne sont pas considérés comme tous revus.
Chaque item sans décision explicite apparaît comme `unreviewed` dans le plan.
Cet état de revue est distinct du résultat des obligations déjà adoptées : un
passage réussi ne signifie pas que tout le catalogue a été étudié.

Un engagement peut citer des identifiants du catalogue dans `origins`.
Les décisions s’écrivent séparément :

```toml
[[catalogue_decisions]]
id = "PHY-01"
status = "not-applicable"
reason = "Le jeu classe des réponses ; il ne simule ni corps ni collisions."
```

Les statuts admis sont `adopted`, `not-applicable` et `unreviewed`, avec une
raison non vide. `adopted` exige un engagement actif qui cite cet item ;
`not-applicable` ne peut pas contredire un engagement qui l’adopte. Les sources
extérieures, l’analyse et les limites de l’adaptation se détaillent dans
`LEARNINGS.md` ; `origins` attend des identifiants du catalogue, pas des URL.

L’applicabilité des minima communs est résolue par l’outil à partir de leur
politique et de la configuration. Par exemple, l’absence d’écran téléphone
écarte certains contrôles propres à cet écran. L’absence de progression
observable pour la terminaison d’une partie peut laisser l’obligation
**inconnue**. Un test absent, un manque de temps ou une justification rédigée
par l’agent ne transforment jamais un inconnu en non-applicable.
La machine peut vérifier la cohérence d’une déclaration ; elle ne démontre pas
que le jeu a déclaré toutes ses capacités ni tous ses risques.

## Méthodes et résultats

Une méthode décrit le moyen de savoir : analyse statique, test, mesure, preuve
ou justification. Un résultat décrit ce qui s’est passé : réussi, échoué,
ignoré, non exécuté, erreur d’infrastructure ou revue nécessaire. Aucun ordre
unique de « force » ne remplace ces deux axes. Une mesure de latence et une
preuve d’absence de débordement répondent à deux questions différentes.

Une obligation applicable n’est satisfaite que si son observation a réussi,
avec la méthode et le périmètre attendus, pour les entrées courantes.
Un test ignoré, un nom absent, zéro test sélectionné, une observation périmée
ou une identité inconnue laissent l’obligation en attente. Une erreur de
compilation, une panne de contrôleur et un invariant contredit restent
distinguables dans les détails du rapport.

Le runner demande à Cargo ses exécutables de test, puis à libtest sa liste et
ses résultats. Il ne recherche pas de `fn` dans le texte. Une cible est exécutée
une fois par configuration, même si plusieurs engagements citent ses tests.
Le passage complet conserve aussi les échecs de tests non référencés.
Un harness personnalisé demande un adaptateur explicite. Les processus ont
des délais : 15 minutes pour une compilation et une cible, 30 secondes pour
l’inventaire et la découverte ; dépasser le délai donne une erreur
d’infrastructure, aucune réussite implicite.

Le contrôle `cargo:suite` juge ce qui a tourné dans tout le workspace. Si rien
n’a tourné, ou si une crate a des tests et n’en a exécuté aucun, il reste
`not-run` et le passage échoue : tout marquer `#[ignore]` ne fait pas taire un
test rouge. Un test ignoré parmi d’autres est compté, nommé avec sa raison et
laissé à un humain (`review-required`), sans faire échouer le passage. Une crate
sans aucun test, à côté de crates qui en exécutent, ne compte pas comme un trou.

## Choisir le passage sans perdre le périmètre

| Passage | Contrôles demandés | Lecture correcte du succès |
|---|---|---|
| `soiree check --fast` | Déclarations, règles et cohérence communes | Ce sous-ensemble a passé ; compilation, tests et harnais restent non attestés |
| `soiree check` | Même socle, formatage, Clippy natif/WASM, suite Cargo et doctests | Le périmètre de vérification du code a passé |
| `soiree certify` | Scénarios ordinaires du harnais navigateur | Ces scénarios ont passé ; ce n’est pas un résultat des tests du code ni des appareils réels |
| `soiree certify -- --only ui,panic` | Scénarios explicitement choisis et santé du contrôleur | Ce périmètre réduit a passé, sans attester les autres scénarios |
| `soiree certify -- --soak 30` | Scénarios ordinaires et endurance R2 | L’endurance demandée a aussi été observée |

Un `--only` vide ou inconnu échoue. Un scénario sélectionné mais sauté,
partiel ou dépourvu d’observation attendue ne réussit pas simplement parce que
Node est sorti avec le code zéro. Les contrôles non sélectionnés restent
visibles. Le code de sortie porte sur le périmètre demandé : un jugement humain
reste listé sans faire échouer, un contrôle qui n’a pas pu finir fait échouer.
Quand il reste un jugement, le résumé commence par `✓ Aucun échec, N point(s) à
juger` : le code de sortie vaut 0, mais ces points ne sont pas attestés. L’agent
lit chacun, corrige ce qui est un défaut et montre le reste à l’auteur avant de
dire que c’est fini.

Les rapports structurés sont `certification/check-fast.json`,
`certification/check.json` et `certification/verification.json`, avec une vue
Markdown à côté. Ils distinguent le périmètre demandé satisfait, les obligations
automatisées satisfaites et le plan complet. Les passages séparés ne sont pas
fusionnés automatiquement en un certificat global : leur sélection entre dans
leur identité. Un rapport du harnais est une observation, pas la source des
engagements.

Le mode rapide réduit le travail demandé ; il ne prétend pas encore identifier
tous les contrôles affectés par un changement. Les durées, l’invalidation fine,
le parallélisme et les caches doivent être mesurés sur les jeux et la CI.
Aucun budget de quelques secondes, aucune accélération ni durée de CI n’est
garanti par cette première implémentation.

## Premier état de la couverture

Le plan garde aussi sept exigences communes sans liaison exécutable (`Unbound`)
tant qu’un jeu ne les relie pas : C6, E5, T1, D1, D3, D4 et D5. Elles restent
alors non attestées.
Les conditions de genre ou de mécanique qui ne sont pas déclarées restent
inconnues, notamment pour les phases, les champs texte et la simulation en temps
réel. La réussite du périmètre statique ne les transforme pas en garanties.

Le harnais distingue désormais `text` (corpus, effets/refus, cadres et négatifs),
`progression` (fin réelle puis reprise) et `capacity` (maximum réel, indépendant
du nombre de téléphones ordinaires). Les commandes de contrôle possèdent un
hôte isolé : les états et places des autres parcours ne les contaminent pas.
R1 observe la causalité fonctionnelle sous latence, son appréciation physique
reste au salon. T4 est observé indépendamment de la stratégie de rendu TV.
Les domaines explicites desktop/interaction continue ont leurs négatifs :
une désactivation seule ou un jeu à phases ne peut pas les usurper.

Les rapports `check` et `certify` ne se fusionnent pas automatiquement ; leurs
sélections restent visibles. Même toutes reliées, ces exigences sont observées
par `check`, les scénarios par `certify`, et les gestes S1–S10 restent humains :
aucun passage isolé n’atteint donc `complete`. Le succès de la CI ciblée du
modèle n'est pas présenté comme celui de tous les scénarios, ni de toutes les
exigences.

## Provenance et hypothèses

Les entrées sont empreintées avant et après le passage : sources du jeu,
dépendances Git ou locales effectivement résolues par Cargo, lockfile et graphe
de dépendances, configurations Cargo parentes, plan, sélection, paramètres,
binaire du contrôleur, harnais utilisé et versions d’outils recueillies.
Les paramètres connus de campagne, notamment les graines et nombres de cas,
comptent aussi. Une campagne explicitement réduite à zéro n’atteste rien.
Les racines de sources inspectées sont conservées dans la provenance.

Une source locale patchée compte par son contenu réel. Cela permet de vérifier
un chantier local sans prétendre que sa dépendance est publiée.
`soiree check --locked --published` conserve un rôle différent : refuser les
dépendances communes locales ou à la mauvaise étiquette avant distribution.
Une prévalidation locale ne remplace jamais ce passage depuis un clone sans
patch de l’hôte, une fois l’étiquette publiée.

Une empreinte établit un contexte, pas la qualité de l’oracle ni une signature
infalsifiable. Le code de résolution, les adaptateurs, l’environnement de la
machine, les outils et leurs hypothèses restent des frontières de confiance.
Leur observation n’inventorie pas tous les comportements possibles d’un GPU,
d’iOS ou du réseau réel. Écrire `assumptions` rend les limites discutables et
révisables ; leur présence ne démontre pas qu’elles sont vraies.

## La place exacte de Verus et de la revue

Le [noyau de décision](outil/verification-kernel/README.md) est le code de
production appelé par l’évaluateur. Verus vérifie ses fonctions de décision
finies et leurs lois d’agrégation : une observation absente ou périmée ne peut pas y
devenir une réussite ; agréger des résultats ne masque pas une obligation
incomplète. Les tests exhaustifs de ce domaine fini et les mutations de preuve
complètent le contrôle et sont rapportés comme des méthodes distinctes.

Cela ne prouve pas les parseurs, la résolution de l’applicabilité, le calcul des
empreintes, les adaptateurs Cargo/navigateur, les spécifications du jeu ni la
véracité des observations fournies au noyau. Verus, son solveur, les
spécifications de sa bibliothèque et la compilation restent dans la base de
confiance. Les commandes reproductibles et les rapports de preuve sont décrits
dans le README du noyau ; Verus n’est pas requis pour compiler chaque jeu.

Les gestes S1–S10 sur appareils réels restent des jugements humains. Un compte
rendu doit nommer les appareils, le contexte, la personne et ce qui a été
observé. Le système actuel les laisse en attente ; il n’importe pas un texte
de revue comme une preuve automatisée. Forcer l’explicitation aide la revue,
mais une explication convaincante peut encore être erronée ou incomplète.

## Caches de CI et confiance

Les workflows mettent en cache les téléchargements Cargo et des répertoires
bornés de compilation native, WASM et iOS. Les clés distinguent système,
architecture, Rust 1.99, version du SDK et lockfiles ; les sorties compilées ont
aussi une clé par révision. Les replis restent dans la même plateforme,
toolchain et version du SDK. Cargo décide quels artefacts restent réutilisables
pour les sources courantes et exécute à nouveau les contrôles demandés.

Les outils externes immuables ont leur cache par versions figées. L’installation
de `soiree`, wasm-bindgen ou cargo-mutants n’est sautée que si le binaire restauré
réussit sa commande de version et renvoie exactement la version attendue.
Dans la CI du commun, `soiree` est toujours réinstallé depuis le checkout courant,
avec son répertoire de compilation conservé : son numéro de version seul ne
permet pas de réutiliser un ancien binaire de développement. La CI d’un jeu
utilise la source du SDK à son étiquette publiée et immuable.

Ces caches de compilation et de binaires supposent le runner et les caches
GitHub dignes de confiance ; une chaîne de version n’est pas une attestation
cryptographique d’origine. Cargo garde son contrôle des dépendances et le
téléchargement de Verus vérifie en plus le SHA-256 figé de l’archive officielle,
y compris sur un cache présent. Aucun rapport de vérification ni observation
brute n’est restauré comme autorité : les résultats sont produits à nouveau.
La présence d’un cache ne transforme jamais une obligation en réussite et
n’établit pas un gain de temps ; les mesures de la campagne du modèle sont
conservées comme artefact de la CI commune.


### Portée locale du transport pendant une vérification UI

`cert.json` peut fixer `"phone_upstream": "loopback"`
pour isoler la continuité UI et la récupération d’une panique de la disponibilité
du LAN. Le rapport inscrit ce choix dans le périmètre des observations. Les
exigences de réseau local restent alors non attestées ; ce passage ne satisfait
pas leur applicabilité. Le mode par défaut est `lan`, sans repli automatique.
Le modèle temporaire de validation locale utilise cette portée UI lorsque le
pare-feu de l’hôte bloque son nouvel exécutable ; aucune règle du pare-feu
n’est changée par le harnais.

Avant les observations, le harnais prépare les fichiers servis puis prend leur
empreinte. Une voie LAN indisponible produit une panne de préparation du
contrôleur avant les scénarios concernés. Les attentes UI sondent une valeur
booléenne résolue avec une échéance ; un objet Promise ne vaut pas un succès.
