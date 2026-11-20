# Pour l'agent qui modifie la console

> Ce fichier s'adresse à l'agent de code qui travaille dans ce dépôt, quel qu'il soit. Tous les jeux de la soirée dépendent de ce dépôt.

Lis `MANIFESTE.md`, puis `CONTRAT.md`. Ne les modifie pas sans demande explicite de l'hôte.

## Ce que contient ce dépôt

| Dossier | Contenu |
|---|---|
| `regles/` | les règles : `rust-toolchain.toml`, `clippy.toml`, `lints.toml`, et `conseils.toml`, ce que `soiree check` dit quand une règle refuse. L'outil les embarque. |
| `outil/` | l'outil `soiree` : `new`, `check`, `publish`, `rules`, `manifeste`, et pour l'hôte `games`, `play` (l'étagère, `outil/src/shelf.rs` et sa page `outil/etagere/`) et `maj` |
| `REJOINDRE.md` | le texte qu'un ami colle dans son assistant IA pour commencer |
| `CONTRAT.md` | le contrat de la cartouche : ce qu'un jeu fournit, ce que la console fournit. L'outil l'embarque (`soiree contrat`). |
| `cartouche/` | la brique « cartouche » : le contrat (`Machine`, `State`, `Input`, le ticket `Content` d'un contenu lourd, les demandes au monde extérieur `Ask` et leurs réponses `Answer`), le tirage au hasard des commandes (`draw`), le journal d'une partie (`journal`) et le banc qui joue des parties (`bench`). Sans système, comme la machine qui l'emprunte. |
| `telecommande/` | la brique « télécommande » : le programme de tous les jeux (`console`, sur l'ordinateur : il fait tourner la machine, sert les pages, écrit le journal, range les contenus lourds ;  `doors` fait ce que la machine demande au monde extérieur) et le lien d'une page vers le jeu (`web`, dans le navigateur : `connect`, `upload`) |
| `iphone/` | l'app iPhone « Soirée » (Swift) : une manette pour tous les jeux. Elle scanne le QR code, charge la télécommande du jeu et lui donne les capteurs de mouvement et les vibrations que Safari refuse. Son contrat avec les pages est dans `iphone/README.md`. |
| `modele/` | le jeu de départ, un vrai jeu avec son propre workspace ; `soiree new` le copie |
| `dossier/` | les modèles des fichiers du dossier de l'hôte |

## Changer la console, c'est changer tous les jeux

- **Durcir une règle ou changer une brique fait échouer `soiree check` dans les jeux.** C'est voulu : c'est ainsi que la console force les jeux à suivre. Fais-le exprès, pour une raison que l'hôte a constatée, pas par précaution.
- **Chaque refus de `soiree check` dit quoi corriger**, assez clairement pour que l'agent d'un jeu le fasse seul, sans connaître ce dépôt.
- **Une règle se vérifie par la machine.** Si l'outil ne peut pas la vérifier, elle ne va pas dans le manifeste.
- **La mécanique va dans les outils, le sens dans les textes.** Une commande, une adresse, une étape à suivre : l'outil la dit au moment où elle sert. Les `AGENTS.md` et `REJOINDRE.md` ne gardent que le but et ce qui compte.
- **Avant d'ajouter une brique**, demande-toi si un seul jeu en a besoin. Si oui, sa place est dans ce jeu.

## Les règles

Leur source est `regles/`. La console se les applique : `Cargo.toml`, `rust-toolchain.toml` et `clippy.toml`, à la racine et dans `modele/`, en sont des copies. Un test de l'outil vérifie qu'elles sont identiques. Après un changement dans `regles/`, recopie la table dans le `Cargo.toml` de la racine, lance `soiree rules modele`, puis recopie `modele/clippy.toml` et `modele/rust-toolchain.toml` à la racine.

- **Un refus oriente.** Quand tu ajoutes ou durcis une règle, écris dans `regles/conseils.toml` pourquoi la console la pose et par où sortir : `soiree check` l'affiche sous le refus. Une règle qui ne dit que « non » pousse l'agent d'un jeu vers une exception.
- **Un principe plutôt qu'une liste.** Avant d'interdire des noms un par un, cherche la vérification qui les refuse tous d'un coup : la machine d'un jeu ne liste pas ce qui est interdit, elle se compile sans système.

## Le contrat de la cartouche

Un jeu est une cartouche : une machine et des pages. La console est le programme. `CONTRAT.md` dit ce que chacun fournit ; il est fait pour ne plus bouger.

- **Le contrat se change par ajout.** Une méthode nouvelle avec une valeur par défaut, une porte de plus : aucun jeu n'a à changer. Une porte vers le monde extérieur est une chose de plus à demander (`Ask::…`) et une lecture de plus sur la réponse (`Answer::as_…`) : un jeu n'écrit jamais de `match` sur ces types, donc aucun ne casse. Retirer ou changer le sens d'une ligne du contrat oblige à réécrire tous les jeux : ne le fais que sur demande de l'hôte.
- **Ce que la console apprend à faire s'appuie sur les propriétés du contrat**, sans rien demander aux jeux : rejouer une partie, la reprendre, la rattraper. Avant d'ajouter quelque chose au contrat, cherche si la console peut le faire seule.
- **Une contrainte tient par construction, pas par une règle.** Le jeu n'a pas de programme à lui (`src/main.rs` est le même pour tous, `soiree check` le vérifie) : il n'y a donc nulle part où cacher une règle du jeu hors de la machine. La machine se compile sans système : elle ne peut pas lire l'horloge.

## La machine d'un jeu

Chaque jeu déclare sa machine : le crate qui porte ses règles (`[package.metadata.soiree] machine = true`) et dont le type `Jeu` remplit `cartouche::Machine`. Même graine et mêmes entrées, même partie, sur l'ordinateur comme dans un navigateur.

- **Par principe** : `soiree check` compile la machine pour une cible sans système (`wasm32v1-none`). L'horloge, les fichiers, le réseau, le hasard du système, `HashMap` et les fonctions mathématiques du système n'y existent pas : aucune liste à tenir, et une dépendance qui s'en sert est refusée aussi. C'est une compilation sans lancement, rapide.
- **`glam`** y est permis avec `libm` et `scalar-math` seulement : sans elles, il calcule avec des instructions propres au processeur et le dernier chiffre change (essai fait : 20 000 pas, différents par défaut, identiques avec ces deux options).
- **Par le banc** (`cartouche/src/bench.rs`, lancé par `outil/src/machine.rs`) : l'exemple `banc` de la machine, le même fichier dans tous les jeux, joue des parties sans une ligne écrite par le jeu. Des pages arrivent et partent, envoient ce que la machine suggère (`Machine::suggest`) ou des commandes tirées au hasard depuis leur type (`cartouche::draw`), le temps passe par rafales. Ce que la machine demande au monde extérieur (`Machine::asks`), le banc y répond à sa place, pannes comprises ; il ne consulte son hasard pour ça que si la machine demande quelque chose, pour qu'un jeu qui ne demande rien joue les mêmes parties qu'avant. `soiree check` le lance deux fois en natif, puis en WebAssembly (`outil/src/wasm.rs`, un interpréteur embarqué : rien à installer à côté), et compare les lignes d'empreinte. Puis `--long`, en natif : plus de parties, la reprise depuis la photo comparée à la partie jouée d'une traite, le temps de chaque pas.
- **Les parties du banc se jouent ensemble**, une par cœur : elles ne se doivent rien l'une à l'autre. En natif, le banc se relance lui-même, un processus par partie (`--courte <n>`, `--longue <n>`), et rend leurs sorties dans l'ordre des graines : la même sortie que l'une après l'autre. Un processus et pas un fil : une machine a le droit de ne pas savoir tourner à plusieurs dans le même programme. En WebAssembly, c'est `soiree check` qui lance le programme une fois par partie (`outil/src/machine.rs`), si le banc dit savoir le faire (`--sait parties`) ; le banc d'une console d'avant est lancé d'un bloc, comme avant. Jouées ensemble, les parties se gênent : avant de refuser un jeu pour sa lenteur, le banc refait sa mesure une partie à la fois.
- **L'empreinte** est celle des octets de l'état (`cartouche::fingerprint`) : le jeu ne l'écrit plus. Un champ oublié n'existe plus ; un état caché hors de la photo se voit à la reprise.
- **Un jeu peut garder un exemple `empreinte`**, pour des parties écrites d'avance qui vont là où le hasard ne va pas : `soiree check` le lance comme le banc.
- `modele/machine/` montre la forme attendue.

## Ce qui tourne pendant la soirée

`telecommande::console::run` est le programme de tous les jeux. Il donne le temps (`TICKS_PER_SECOND`, au plus un dixième de seconde de rattrapage), n'envoie une vue que si elle a changé (au plus soixante par seconde), porte les effets une fois, et écrit dans le dossier du jeu, sous `.soiree/` :

- **le journal** de chaque lancement (`.soiree/journal/`) : la graine, chaque entrée et chaque réponse du monde extérieur, avec le numéro de son pas. `soiree rejouer <fichier>` refait la partie et vérifie ses repères ;
- **l'état gardé** (`.soiree/etat`) des jeux qui le demandent (`REMEMBERS`). Un état illisible est mis de côté, jamais écrasé ;
- **les contenus lourds** (`.soiree/contenus/`) que les pages lui confient à `/contenu` : une image, un son. Chacun porte le nom de son ticket (`cartouche::Content`) et se redemande à `/contenu/<nom>`. Au-delà de 500 Mo, les contenus montrés il y a le plus longtemps s'effacent.

- **les sauvegardes** (`.soiree/sauvegardes/`) qu'une machine demande de garder (`Ask::save`) : un fichier par nom.

Ce que la machine demande au monde extérieur (`Machine::asks`), `telecommande/src/doors.rs` le fait : la date à la montre de l'hôte, une question à internet, les octets d'un contenu, les sauvegardes. La réponse revient par `Machine::answer` et s'écrit au journal ; d'un contenu, le journal ne garde que le ticket. Un secret (`{NOM}` dans un en-tête) vient de la variable `SOIREE_SECRET_NOM` et ne part que vers l'adresse de `SOIREE_SECRET_NOM_POUR` : il n'entre ni dans la machine ni dans le journal.

**Le menu de la console** (`telecommande/src/menu.rs`, dans le navigateur) est le même pour tous les jeux : il démarre avec le lien de la page (`web::connect`), écoute les touches avant le jeu, et parle à la console par `/soiree/console`.

- Sur l'ordinateur de l'hôte, Échap maintenu l'ouvre ; un appui court est rendu au jeu. Une page venue d'ailleurs que de l'ordinateur ne peut rien demander : c'est l'adresse de la connexion qui le dit, pas la page.
- Un jeu peut avoir sa propre pause (`web::pause`, `web::resume`) : la console arrête le temps de la même façon, sans montrer son menu, et n'écoute plus que l'écran. La pause tombe si la page qui l'a demandée se ferme.
- Menu ouvert, le jeu est figé pour tout le monde : la console ne donne plus le temps et jette ce que les pages envoient. Dans chaque page, elle retient aussi les images (`requestAnimationFrame`), arrête l'horloge (`performance.now`) et coupe le son, puis rend le tout à la reprise : une page qui anime la scène toute seule entre deux vues se fige avec la partie. La machine n'en sait rien, et le journal se rejoue comme s'il n'y avait pas eu de pause.
- « Recommencer » donne une machine neuve et un journal neuf, sans relancer le jeu. Un jeu dont la console garde l'état (`REMEMBERS`) ne le propose pas.
- « Quitter » n'existe que si l'étagère a lancé le jeu (`SOIREE_ETAGERE`) : le jeu écrit ce qu'il garde, s'arrête, et l'écran retourne à l'étagère.

**L'étagère** (`soiree play`) vérifie chaque jeu une fois, puis range à part son programme et ses pages (`games/.etagere/<jeu>/<version>/`). Elle prépare plusieurs jeux à la fois (un pour quatre cœurs, trois au plus : la vérification d'un jeu occupe déjà plusieurs cœurs et chronomètre la machine), et commence par les jeux déjà rangés : un jeu neuf, long à vérifier, ne fait attendre personne. Elle lance ce qui est rangé, avec `SOIREE_PAGES` pour que la brique ne recompile rien. Les téléphones n'ouvrent jamais le port d'un jeu : ils arrivent à l'étagère, qui les amène octet pour octet au jeu en cours (`SOIREE_RELAIS`, la porte des téléphones dans `telecommande/src/tls.rs`). Leur adresse ne change donc pas d'un jeu à l'autre, et le pare-feu de l'hôte ne connaît qu'un programme, l'étagère ; le jeu, lui, n'écoute que l'ordinateur. Ce qui arrive par cette porte vient du Wi-Fi : ça ne peut pas ouvrir le menu. Le dossier `.soiree/` d'un jeu (journal, état gardé) reste dans son dépôt.

Un jeu qui se joue sans télécommande (`REMOTES = false`) n'a ni adresse ni QR code pour les téléphones : la console répond elle-même à `/t`, et refuse un lien de télécommande.

## Avant de dire que c'est fini

Depuis la racine :

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p telecommande --target wasm32-unknown-unknown -- -D warnings
cargo check -p cartouche --lib --target wasm32v1-none
cargo test --workspace
cargo run -p soiree -- check modele
```

Puis `soiree check` dans chaque jeu du dossier de l'hôte (`games/*`). Un jeu qui ne passe plus : soit tu corriges ici, soit c'est le changement voulu et tu remets le jeu à niveau.

## Versions

- Une version de la console est une étiquette git sur ce dépôt (`v0.4.0`, `v0.5.0`…). Son numéro est celui de `[workspace.package]` dans `Cargo.toml`.
- Les jeux empruntent les deux briques à cette étiquette, et `soiree new` écrit ces dépendances.
- `README.md` et `REJOINDRE.md` nomment l'étiquette à installer. Un test de l'outil vérifie que c'est celle de la version courante.
- Committer, étiqueter et pousser demandent l'accord de l'hôte.
