# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## Non publié — la refonte, phases 1 et 2

Le commun repart de zéro, d'après [`MANIFESTE.md`](MANIFESTE.md), [`CONTRAT.md`](CONTRAT.md) et [`conception/INTERFACE.md`](conception/INTERFACE.md). L'ancien commun est retiré : salon, la télécommande iPhone (`remote/`), le modèle de jeu (`modele/`) et le harnais de certification (`tools/cert/`). Il reste lisible à l'étiquette `avant-refonte`. À sa place, un workspace de crates dans `crates/` : un jeu s'écrit avec `jeu`, se compile en un module WebAssembly qui n'importe rien, est réécrit (`reecriture`), chargé dans wasmtime (`moteur-wasmtime`), joué par le cœur de la partie qui fait foi (`partie`) sur une horloge simulée, et éprouvé (`epreuve`, contrôles E1 à E19, chacun avec son jeu fautif). Il n'y a encore ni navigateur, ni relais, ni rendu, ni prédiction sur appareil, ni outil `soiree` réécrit : un jeu ne se joue pas encore, il s'éprouve. Aucune étiquette n'est posée ; les étiquettes `v0.x` restent celles de l'ancien commun.

Les trois jeux de la soirée sont reconstruits sur ce commun, dans leur branche `refonte`, et en dépendent par chemin. Ce que chacun a demandé au commun, et la réponse, est dans [`conception/BESOINS-DES-JEUX.md`](conception/BESOINS-DES-JEUX.md) ; ce qui a été mesuré sur eux, dans [`conception/MESURES-PHASE-1.md`](conception/MESURES-PHASE-1.md).

### Passer à cette version

Aucune API de l'ancien commun ne survit : salon, `DeviceEvent`, `soiree.toml`, `cert.json` et `verification.toml` n'existent plus. Un jeu se réécrit sur la crate `jeu`.

1. Lis `conception/INTERFACE.md`, § 2 (l'API d'un jeu) et § 9 (`devine`, un jeu complet), puis `crates/exemples/devine/`.
2. Fais deux crates : les règles (`#![no_std]`, `#![forbid(unsafe_code)]`, `impl jeu::Jeu`) et la crate de module (`jeu::module!(<ton jeu>);` et rien d'autre qu'un attribut `no_std`, `crate-type = ["cdylib"]`, qui ne dépend que de `jeu` et des règles). Reprends les `[workspace.lints]`, `clippy.toml`, `[profile.release]`, `[profile.module]` et `[profile.dev.package."*"]` de ce dépôt, sans rien assouplir : l'épreuve les compare. Une crate d'épreuve écrite à la main n'est plus nécessaire.
3. Pendant la refonte, dépends du commun par chemin, depuis le dossier de l'hôte : `jeu = { path = "../../soiree/crates/jeu" }`, et de même pour `jeu-abi`, `jeu-natif`, `partie`, `epreuve`, `moteur-wasmtime`. Il n'y a pas encore d'étiquette à nommer, ni de `soiree upgrade`.
4. Éprouve le jeu depuis ses sources, depuis `soiree/` :
   ```sh
   cargo run --release -p epreuve --bin soiree-epreuve -- ../games/<jeu>
   ```
   L'épreuve vérifie les règles d'écriture, lance clippy, compile le module et la copie native (écrite hors du dépôt du jeu, dans son dossier de compilation), puis joue. Le jeu passe quand le code de sortie vaut 0, sur le plan standard : chaque étape exigée du CONTRAT faite, chaque contrôle réussi ou sans objet. `soiree-epreuve --wasm <module.wasm>` éprouve un module seul, mais ne rend jamais 0 : l'étape 1 n'y est pas faite. Ne lance pas `soiree check` ni `soiree certify` : l'outil n'est pas réécrit.

### Phase 3, lot 13 : la latence et le diagnostic

Le lot 13 (`conception/PHASE-3.md`, § 2.9, § 8.4, § 8.6 ; `conception/LOTS-PHASE-3.md`, « Lot 13 » ; les chiffres dans `conception/MESURES-PHASE-3.md`, « Lot 13 »). **Un rapport, pas une porte.** L'ABI ne change pas, ni ce que l'épreuve joue ou juge, ni la forme des cases : les modules gardent leur empreinte. Les messages entre la page de l'écran et le Worker changent (`salle::ecran::PROTOCOLE` passe de 3 à 4) ; ceux des téléphones non (`salle::fil::PROTOCOLE`).

- **La trace de bout en bout** (§ 8.4) : chaque entrée de l'écran part au Worker avec son numéro, l'instant de son événement (`event.timeStamp`) et celui de son envoi (`DeLaPage::Ecran { numero, envoi_us }`) ; chaque geste d'un téléphone porte déjà sa `TraceTelephone`, que l'autorité ramène du temps de la partie à son heure (`Horloge::heure_us`). L'autorité suit chaque entrée placée jusqu'au tour qui joue son pas (`TRACES_EN_COURS_MAX`, 256), et rend sa trace (`VersPage::Traces`) juste après la scène de ce tour, avec le début et la fin du pas et la fin des vues (le chronomètre du Worker tourne désormais hors du banc). La page y ajoute la scène reçue, le début de l'image qui la dessine en entier (`Trace::image_us`, un champ nouveau), le dessin soumis et le travail du GPU fini. `salle::trace` : les segments, les distributions (jamais une moyenne ; centiles au rang le plus proche), le panneau.
- **Le panneau de diagnostic** (§ 2.9), ouvert et fermé par la touche que déclare le jeu (`VersPage::Jeu { diagnostic }` ; elle ne va plus au jeu) : par source (clavier, souris, chaque téléphone), les 50ᵉ, 95ᵉ et 99ᵉ centiles de chaque segment sur les 200 derniers gestes, la ligne du budget, les pas par image, le moteur, les glyphes absents, la ligne du matériel à part ; un carré blanc d'au moins 100 ms à chaque entrée (REM-117).
- **Glisser** (`rendu::glisser`) : à 100 Hz et plus, ou forcé par le banc, la position des nœuds qui glissent va de la scène précédente à la dernière en un pas, **seulement quand l'un d'eux se déplace** (`rendu::glisser::deplace_des_noeuds` ; sinon la scène se dessine en entier tout de suite) ; le pixel d'une entrée est la première image où chaque nœud est au moins à sa place du pas de l'entrée (la scène dessinée en entier, ou un chemin qui part d'elle). *Corrigé après la vérification* : la page glissait après chaque scène neuve, même sans nœud qui glisse, et retardait d'un pas le pixel de toute scène ; les chiffres « avec glisser » d'`ardoise` et de Punchline ne mesuraient que ce retard, et sont remplacés par ceux de Sidequest, le seul module qui glisse.
- **Le calage mesurable** : la commande du banc `CommandeEcran::Calage(Non)` fait jouer le pas par la seule minuterie du Worker (sans elle, au signal de l'image, P6) ; `CommandeEcran::Echo` répond à la page par une ligne `echo` (M14). Ignorées hors du banc.
- **`soiree-banc mesures <module>`** (`banc::mesures`) : M2 (de vraies touches et de vrais clics, les gestes des téléphones natifs ; avec et sans calage, avec et sans glisser ; par source et par segment ; le segment qui pèse le plus nommé quand le budget n'est pas tenu), M14 (l'écart des horloges entre la page et son Worker, un autre onglet, le banc ; avant les conditions de M2), ce que glisser a fait dans chaque condition (`window.__banc.glisses()` : les scènes où un nœud a glissé, les images à mi-chemin), et son contrôle négatif (le dessin ralenti de 33 ms, `window.__banc.dessin_lent`, doit se voir dans le segment du dessin). `--avec-interface` : la cadence de l'écran de la machine.
- **`soiree-banc arriere-plan --avec-interface --mode recouverte|reduite`** (M6) : la fenêtre du navigateur recouverte par une autre, ou réduite dans le Dock ; ce que la page en dit est noté, pas jugé. Chrome se lance aussi avec sa fenêtre (`Chrome::lancer_avec`) ; `aide --avec-interface` reste Firefox seul. Le banc tape les chiffres et les lettres (`navigateur::caractere_de`).

### Passer à cette version

Rien à faire : aucune API d'un jeu ne change. La touche du diagnostic que le jeu déclare ouvre désormais le panneau du commun et ne lui arrive plus (INTERFACE, § 6.4 : c'était déjà son rôle).

### Phase 3, lot 7b : les jeux corrigent leurs pages et leurs textes

Les trois jeux corrigent, dans leurs dépôts, les débordements que E16 a trouvés au lot 8 et ce que leur page montrait dans un navigateur (`conception/PHASE-3.md`, § 7, lot 7b ; `conception/LOTS-PHASE-3.md`, « Lot 7b »). **Le commun ne change pas** : ni son code, ni l'ABI (`jeu_abi::ABI` vaut 4), ni ce que l'épreuve joue ou juge. Après les trois jeux, depuis leurs sources, au plan standard, les trois rendent 0 (E16 : aucun débordement) ; le scénario `pages` est vert sur leurs trois modules dans Chrome et Firefox ; la référence du plan court revient à celle du lot 7 (Sidequest 0, Punchline 1 par E2 seul, Tier list 0).

Ce que les jeux demandent encore au commun, depuis le lot 7 et à ce lot, est rassemblé **sans réponse** dans `conception/BESOINS-DES-JEUX.md`, « Les demandes ouvertes après le lot 7b » (22 demandes, chacune avec son coût estimé et l'ABI qu'elle demanderait) : au mainteneur de trancher. Deux reviennent dans plusieurs jeux : une coupe « entre les mots, et un mot trop long entre graphèmes » (les trois) et l'avis d'un refus donné à la page (Punchline et Sidequest).

#### Passer à cette version

Rien à changer pour un jeu. Ce que les jeux de la soirée ont appris, et qui vaut pour un autre : un pire cas se fait des caractères les plus larges de ses polices (lus dans `hmtx`), pas de « W » ; un mot plus large que sa boîte ne se coupe qu'en `Coupe::Graphemes` ; et une page du téléphone ne voit pas encore `Reglages::fond` (son corps reste #111 quelle que soit la vue : D2). *Après la vérification du lot 7b :* D2 est rangée comme un écart de la page à INTERFACE (§ 2.9), né au lot 12, à corriger dans le commun sans ABI au plus tard avant la fiche de répétition du lot 14 (`conception/PHASE-3.md`, § 7, ligne du lot 12) ; un jeu peut donc continuer à choisir ses encres pour le fond qu'il donne.

### Phase 3, lot 12 (fin) : changer de jeu, le singe qui partage le salon, le fil des natifs

La fin du lot 12 (`conception/PHASE-3.md`, § 2.8, § 8.1, § 8.2 ; `conception/LOTS-PHASE-3.md`, « Lot 12, fin »). L'ABI ne change pas, ni les messages (`salle::fil::PROTOCOLE`, `salle::ecran::PROTOCOLE`), ni la forme des cases (`VERSION_CASES`), ni ce que l'épreuve joue ou juge : les modules gardent leur empreinte.

- **Changer de jeu sans rien relancer** : `Autorite::changer_de_jeu` (une partie neuve dont le premier pas suit le dernier de l'ancienne, sa chaîne de garde partie d'un point neuf au numéro suivant, mêmes liens, même registre ; `RefusChangement`, `Comptes::changements`) ; le Worker charge le module du catalogue sur `CommandeEcran::ChangerDeJeu`, le livre aux téléphones et garde le nouveau module dans son enregistrement ; l'écran de démarrage fait de chaque ligne de sa liste un bouton (`demarrage::ACTION_CHOISIR`), « Jouer » demande le jeu choisi, **F9** remontre l'écran de démarrage par-dessus la partie ; la page d'un téléphone (`JoueurDePage`, `web/telephone`) se refait sur le nouveau module. Un jeu sans téléphone ne reçoit des téléphones encore reliés ni présence, ni regard, ni bot. `simulation::Soiree::changer_de_jeu`, `verifier_l_historique` accepte la chaîne neuve d'un autre jeu (`Historique::changements`).
- **Le singe partage le salon** (régression de la fusion du lot 11) : il ne joue rien tant que chaque autre téléphone natif n'a pas vu accuser un premier geste (`salle::page::salon_au_complet`, `JoueurDePage::retenir_le_singe`, `Soiree::singe_attend_le_salon`), et n'appuie sur un bouton tiré qu'une fois sur cinq (`page::SINGE_BOUTON_UN_SUR` : à chaque tour, il lançait puis annulait chaque partie de Punchline) ; ses tours se comptent (`ComptesJoueur::tours_du_singe`). `soiree_en_memoire --singe-attend`.
- **Les téléphones natifs du banc tournent sur leur propre fil** (`banc::soiree::Natifs`) : une attente du banc (l'essai d'une session Safari, 30 s ; une évaluation `BiDi`) ne les tait plus. Dans `soiree-71`, l'autorité perdait leurs 69 liens (M13 : de 71 appareils accueillis à 2) parce que le banc ne les servait plus. Un silence des natifs au-delà de 1 s est une faute du scénario ; le fil relie au plus quatre téléphones par tour.
- **`soiree-banc changer <module>…`** : le premier jeu, puis chacun des autres, depuis l'écran de démarrage ; `--fautif changer-sans-livrer` est son contrôle négatif. La page rechargée du scénario `soiree --pages` doit retrouver ce qu'elle avait retenu.
- **Les boutons et les maintenus de la page du téléphone coupent un mot trop long** (`overflow-wrap: anywhere`, comme les textes) : un bouton de vote de Punchline débordait à 768 × 1024.
- **`Locaux::retenu` survit au rechargement** de la page d'un téléphone (`localStorage`, par salle et par jeu ; `PageDuTelephone::retenu`, `reprendre_le_retenu`).
- *Après la vérification de la fin du lot 12* :
  - **`soiree-banc pages <module>… --negatif <module>`** (`banc::pages`) : chaque page de téléphone de pire cas (les pires cas construits dans wasmtime depuis les limites, la vue de chaque appareil) posée dans la page du téléphone d'un vrai navigateur, à 360 × 640, 390 × 844 et 768 × 1024 ; un élément qui déborde est une faute qui nomme sa clé. Son contrôle négatif est un fautif de module, `fautif-page-qui-deborde-module` (`fautifs::jeux::PageQuiDeborde` : à 9 999 points, la page écrit le prénom du record à 512 px). La page du téléphone gagne, en mode banc seulement, `window.__banc.morceau` et `window.__banc.pire_cas`.
  - La page rechargée du scénario `soiree --pages` doit **reprendre son retenu de `localStorage`** avant toute vue (`retenu_repris`, `retenu_cle` dans l'état du banc), avec un contrôle négatif (la clé ôtée, rien de repris) ; `telephone::retenu` (la clé, quand écrire) se teste en natif.
  - `soiree` arrête les pilotes des pages avant les marques F8 ; `changer` compte les gestes d'un natif depuis sa page refaite, exige que chacun rejoue, et fait d'un natif relié de nouveau une faute.

### Passer à cette version

Rien à faire : aucune API d'un jeu ne change. Un jeu sans téléphone peut désormais voir des téléphones reliés à la salle (après un changement de jeu) : le commun ne lui en dit rien.

### Phase 3, lot 12 : les téléphones

Les téléphones ont leur page dans le navigateur, et les téléphones natifs du banc passent par elle (`conception/PHASE-3.md`, § 2.5, § 2.10, § 8.1, § 8.2 et lot 12 ; `conception/LOTS-PHASE-3.md`, « Lot 12 »). L'ABI ne change pas, ni les messages chiffrés (`salle::fil::PROTOCOLE` reste 1), ni rien de ce que l'épreuve joue ou juge : les modules gardent leur empreinte.

- **`web/telephone`** (nouvelle crate du workspace `web/`, dans le paquet de la page) : la page `/s/<SALLE>#<fragment>`. L'appareil (`salle::appareil`), la page du jeu appelée dans son module (`Page`, rien d'autre après `Bonjour` et `Decrire`), rendue en DOM par clé (`textContent`, des champs jamais réécrits sous le doigt, au moins 16 px), la croix et les maintenus à capture du pointeur, le carrousel balayable, les polices par `FontFace` et les images en `Blob` reçues de l'autorité ; le jeton dans `localStorage`, l'onglet, les gestes non accusés et les effets joués dans `sessionStorage` ; la reconnexion à intervalle croissant ; les six écrans du commun, un seul à la fois. Elle ne tire pas `wgpu`, et aucune crate tierce nouvelle.
- **Le module part aux téléphones par la connexion chiffrée** (`salle::livraison`) : en morceaux de 64 Kio, au plus deux non accusés par téléphone (`VersAutorite::ModuleRecu`), aucun sur un lien encombré ; puis les ressources de la page. Un téléphone qui revient sans avoir rechargé le dit connu (`module_connu`) et ne le reçoit pas deux fois. `Autorite::livrer` ; le Worker le livre.
- **La page sans DOM** (`salle::page`) : les locaux du commun, les tenus (le dernier gagne, au plus `debit_max` par seconde, un relâchement passe toujours, un tenu relâché ne se redit pas), un doigt lâché quand la page n'a plus sa clé (REM-23), l'arrière-plan (REM-11), la coupure pour un tenu à fraîcheur (P28), et, pour le banc, le chemin d'un coup de bot sur la page et le singe.
- **`salle::appareil`** : le module et les ressources reçus, `garder_la_file` et `reprendre_la_file` (`sessionStorage`), `regler_la_redite`, `taire_la_redite`, `coupe` (l'écho sans réponse 1,5 s), `coups_par_la_page`, `lien_relais`.
- **Le banc** : `soiree-banc soiree <jeu> --pages chrome,firefox,safari` ouvre une page dans chaque navigateur (Chrome en émulation tactile, `Input.dispatchTouchEvent` ; Firefox par des pointeurs `touch`), la pilote par sa chaîne d'entrées, la recharge en pleine écriture réseau gelé (aucun geste perdu), ouvre un onglet jumeau puis « Reprendre ici », coupe une page qui tient la croix (P28), juge ses débordements à trois tailles avec leur contrôle négatif, lit la redite du tenu dans l'enregistrement du relais (M7) et y cherche les secrets ; les téléphones natifs du scénario passent par la page (le premier est un singe). Les sauts d'une reprise se rattachent désormais au rattrapage que le jeu déclare (Sidequest : 3).

Après la vérification du lot 12 (`conception/LOTS-PHASE-3.md`, « Lot 12, correction après la vérification ») :

- **Les coups des bots trouvent leur chemin par les choix** (`PageDuTelephone::chercher_le_geste`) : le prénom tapé, puis chaque choix de la page tourné vers le geste du bot (le `Rejoindre` de Sidequest et son look tiré au hasard) ; une direction continue que la croix ne donne pas prend la plus proche (`approcher_le_tenu`, comptée à part : `tenus_approches`). Un téléphone natif qui n'envoie aucun geste par la page est une faute du scénario `soiree`. `soiree_en_memoire --par-la-page` fait passer les téléphones de la soirée en mémoire par la page.
- **Un point s'écrit d'un bloc** (`partie::persistance::octets`, sur les instantanés du point et sur `ContenuPoint`) : les mêmes octets (les tests de forme figée le tiennent), sans un appel de sérialiseur par octet ; le tour qui prend un point de Punchline ne dépasse plus 10 ms à le sceller dans Chrome (de 10 à 22 ms au lot 12).
- **Le banc** : la file pleine provoquée sur une page réseau gelé, son compte exact, puis chaque geste accusé (SAL-07) ; `--minutes M` (la partie prolongée) ; `--retardataires` (les pages arrivent quand les téléphones natifs sont tous là, et les allers-retours d'écho des autres ne doivent pas croître pendant qu'elles reçoivent le module) ; M13 (la part du commun dans le Worker à chaque tour, `RepartitionDuTour`, rapportée dans les lignes `etat`) ; les jetons des pages et un témoin positif dans la recherche des secrets ; M7 dit aussi sur les seuls moments où le tenu n'est pas relâché ; Firefox aux trois tailles. Le gel du banc (`__banc.geler`) retient ce que la page envoie au lieu de le perdre, comme une WebSocket ouverte. Le pilote des pages attend 3 s entre deux boutons ; M7 relit l'horloge du relais à chaque passage (`relais_us0`).
- **La page du téléphone** : les enfants d'une pile prennent leur case et leur rang (`z-index`) à la pose : l'image de l'avatar ne recouvre plus le bouton « Sauter » de Sidequest.
- `Appareil::file_max`, `Appareil::prendre_les_allers_retours`, `Autorite::appareils_accueillis`.

#### Passer à cette version

Rien à faire pour un jeu : son API ne change pas. Sa page (`Jeu::page`) est désormais celle que les téléphones affichent : ce qu'elle rend se voit tel quel, sur de vrais navigateurs.
### Phase 3, lot 11 : Tier list jouable sur la télé

La télé sait maintenant tout ce dont Tier list a besoin (`conception/PHASE-3.md`, § 2.9 et lot 11 ; `conception/LOTS-PHASE-3.md`, « Lot 11 »), dans `web/ecran` :

- **Les composants du DOM**, posés sur la toile à la boîte de leur nœud : `Champ` (un `<input>`), `EnPlace` (un texte éditable sur place, jamais du HTML, TL-35), `Selecteur` (un `<input type=file>` invisible sur la boîte que le jeu dessine ; dépôt et collage n'importe où sur la page, TL-53), `Lien` (un `<a download>` vers les octets de la ressource). Un champ garde son contenu, son curseur et sa composition tant que sa clé existe ; la page ne le réécrit que si le jeu a changé la valeur lui-même, jamais pour l'écho, même en retard, de ce qu'elle a envoyé (TL-33) ; la saisie se coupe à la limite du champ entre deux graphèmes (TL-27, TL-28) ; un raccourci cède au champ qui a le focus, et aucune touche n'en est un pendant une composition (TL-32, TL-34).
- **`Deplacables`** : le glisser dans la page (seuil, place visée dans l'ordre de l'état sans l'objet tenu, dernière place valide hors des zones, poubelle, Échap et `pointercancel`, défilement au bord selon le temps écoulé et plafonné au réveil : TL-36 à TL-47). La télé dessine l'objet tenu à 20 %, une copie à 80 % qui suit le pointeur, et une marque à la place visée.
- **`Modale`** (un clic hors de son panneau envoie `fermer`), **la page qui défile** (`FormeEcran::Page` : la toile fixe, un espaceur de la hauteur de la scène).
- **Les demandes** : `Image` (décodée, recadrée en carré, réduite, encodée en WebP si le navigateur sait l'écrire, sinon PNG), `Lire` (la taille vérifiée avant, TL-14), `Rendu` (par bandes, une image présentée entre deux, TL-57 ; le compte des images que le rendu n'a pas, TL-56), `Telecharger` (dans l'activation du geste, sinon un bouton du commun la redemande, P24). **`Ligne` ne passe plus par la page** : l'autorité la donne à sa garde, et le relais l'ajoute à `<garde>/<SALLE>/flux/<nom>` (un nom de 1 à 64 signes `a-z0-9-_`, une ligne de 64 Kio au plus sans saut de ligne, un flux de 16 Mio au plus, 16 flux au plus par salle). Une ligne que le relais n'écrirait pas (nom invalide, trop longue, coupée d'un saut de ligne) est refusée au jeu (`Fait(Err(Refuse))`, `Fait(Err(TropGros))`) ; un flux plein, ou un flux de trop, fait ignorer la ligne sans couper la garde ni arrêter la partie.
- **Les images reçues en partie** (une image importée, l'image exportée) sont envoyées par l'autorité à la page avant la scène ou le rendu qui les cite, une fois chacune, et de nouveau après un rechargement.
- `salle::ecran::PROTOCOLE` passe à 3 (la commande du banc `Bots`).
- `soiree-banc soiree` rattache un saut à sa reprise selon le rattrapage du jeu (`de` de `p` à `p + rattrapage`) : Sidequest (`rattrapage: 3`) y échouait.
- Le banc gagne `soiree-banc ecran tier-list` : import, glisser, édition en place, rechargement, export et sauvegarde captés, restauration, plateau plein, dans Chrome (WebGPU et WebGL2 forcé) et Firefox, avec son contrôle négatif (`--fautif activation-perdue`) ; et M3 (le point de reprise de Tier list pris, scellé et envoyé dans le Worker) et M8 (le travail long) dans `MESURES-PHASE-3.md`, « Lot 11 ».
- **Avec le lot 10b** (la remontée dans le lot 11) : une seule chaîne du clavier, la file des événements de la page puis `page::touche` (F8, P38, l'écran du commun qui couvre le jeu) ; un raccourci se cherche par `cibles::raccourci` (Cmd+Maj+Z reconnu) et prime sur le clavier brut ; tant que l'écran du commun couvre le jeu, aucun clic, glisser ni fichier ne lui va, et l'écran de démarrage cache ses composants ; « Jouer » s'écoute sur le document, en pixels CSS ; le voile passe au-dessus de la page du jeu. `soiree-banc ecran tier-list` clique « Jouer » puis « Cliquer pour reprendre ». Il constate aussi que rien ne va au jeu sous l'écran du commun : sous l'écran de démarrage (un clic sur la page du jeu, une image donnée), au clic sur « Jouer », au clic sur « Cliquer pour reprendre » posé sur Reset (la page dit au banc chaque entrée donnée au jeu, une ligne `au-jeu`) ; ses contrôles négatifs `--fautif geste-au-jeu` et `--fautif ne-couvre-pas` ; `--dpr R` le joue à un autre rapport de pixels (2 joué dans Chrome et Firefox).
- **À la fusion avec le lot 12** (`conception/LOTS-PHASE-3.md`, « Fusion du lot 11 ») : les deux lots avaient chacun corrigé le rattachement des sauts au rattrapage du jeu ; la règle fusionnée est celle des deux pour une reprise (`de` de `p` à `p + rattrapage`), et celle du lot 11 pour un retard injecté (`de` de `p` à `p + rattrapage + 1`, et `arriere-plan` passe le rattrapage du jeu), là où le lot 12 gardait `p + 2` ; les deux règles ne diffèrent qu'à un rattrapage au-delà de 1 (Sidequest). Le singe d'une page (`salle::page`, lot 12) tire sa sorte de geste parmi celles que la page offre, et non parmi les cinq ; une page rechargée dans `salle::simulation` garde les comptes de son téléphone : la soirée de devine par la page, dont la trajectoire change avec `Ligne` donnée à la garde, voyait son singe à zéro geste une graine sur deux. Rien ne change pour un jeu. *Correction de la fusion* : ce singe, au salon de Punchline, appuie à chacun de ses tours sur les boutons du VIP ; quand son téléphone a rejoint le premier, il lance chaque partie 5 à 45 pas après l'ouverture du salon, et un natif qui n'a pas rejoint à temps ne reçoit plus aucun coup (le passage Firefox en échec de `soiree-punchline`) : un risque ouvert du scénario, causé par la fusion, laissé au lot qui reprend le lot 12 ; la règle du retard injecté, jouée par `arriere-plan` sur Sidequest, rattache ses douze sauts, chacun à `p + 3`.

#### Passer à cette version

Rien à faire pour un jeu : son API ne change pas. Un jeu qui demande `Ligne` reçoit `Fait` dès que l'autorité a donné la ligne à la garde, ou `Fait(Err(_))` si la ligne n'est pas de celles qu'un flux accepte (`liaison::flux_valide`, `liaison::ligne_valide`) ; une ligne au-delà de 16 Mio de flux est perdue sans que le jeu le sache ; le relais n'accuse pas une ligne : une autorité tuée avant d'avoir répondu la redemande, et la ligne peut s'écrire deux fois.

### Phase 3, lot 10b : le démarrage et le jour J

Le jour de la soirée a sa commande et son écran (`conception/PHASE-3.md`, § 2.5, § 2.8, § 2.9 et lot 10b ; `conception/LOTS-PHASE-3.md`, « Lot 10b »). L'ABI ne change pas, ni rien de ce que l'épreuve joue ou juge : les modules gardent leur empreinte, et leurs rapports leurs contrôles.

- **`soiree-epreuve <jeu> --catalogue <dossier>`** : sur un code 0 du plan standard seulement (un code 0 du plan court ou d'un plan réglé n'éprouve pas un module, et n'y va pas ; E20 n'est pas exigé), le module et son rapport (`<empreinte>.wasm`, `<empreinte>.json` : empreinte livrée, empreinte réécrite et version de la réécriture, date, plan, nom, places, verdict) vont au catalogue. **`soiree-epreuve --construire <jeu>`** compile le module comme l'étape 1 (la même invocation de cargo) et écrit `<empreinte> <chemin>`.
- **`soiree-relais jouer [<jeu>]`** : le module construit, son rapport cherché par empreinte (`$HOME/.soiree/catalogue` par défaut, `--catalogue`), **refusé sans rapport** (on ne joue qu'un module éprouvé ; le relais refuse au lancement un rapport qui ne dit pas un code 0 du plan standard, et un module construit qui n'a pas les octets que le catalogue garde sous son empreinte), joué avec `--non-eprouve` (l'écran affiche alors « NON ÉPROUVÉ » en permanence), le paquet de la page construit, le relais lancé, l'écran ouvert. **`soiree-relais preparer`** construit le paquet de la page (le banc aussi, par la même fonction ; son empreinte est désormais un FNV-1a de 128 bits).
- **L'écran de démarrage**, une scène du commun : les jeux du catalogue, le QR, l'adresse, la salle, les téléphones reliés, l'aide ; « Jouer » est le geste du plein écran, du son et du verrou d'écran. Un onglet rechargé montre « Cliquer pour reprendre ».
- **L'aide** : « Pas de réseau », « Un téléphone a ouvert la page mais n'a pas pu se relier », « Certains Wi-Fi isolent les appareils » (aucun téléphone une minute après le QR), « Autorise soiree-relais » (le pare-feu de macOS, lu sans droits). Le relais gagne `GET /aide` et `GET /jeux/<empreinte>.json` (boucle locale).
- **Le clavier va au jeu** : les raccourcis de la dernière scène (leur action) et le clavier brut ; **Échap en plein écran** (P38) arrive au jeu dans Chrome (Keyboard Lock), et ailleurs sort du plein écran sans arriver au jeu, l'écran du commun le disant (constaté dans Chrome 156 et dans Firefox 158 avec sa fenêtre, dans le plein écran de Firefox et non celui de macOS ; Safari n'est pas constaté).
- **Le son** : les quatre signaux synthétisés, les sons déclarés décodés au geste ; un effet tombé avant le geste est compté perdu, une identité ne rejoue jamais.
- **Le Worker vérifie l'empreinte réécrite** du module contre son rapport d'épreuve, et le QR des jeux porte enfin l'adresse de la salle.
- `salle::autorite::Comptes::entrees_ecran` ; le banc gagne `soiree-banc aide` (`--avec-interface` : Firefox avec sa fenêtre, seul moyen d'y avoir le plein écran au clic du banc) et ses pilotes, le clic et Échap.

#### Passer à cette version

Rien à faire pour un jeu : son API ne change pas. Pour jouer : `soiree-epreuve <jeu> --catalogue ~/.soiree/catalogue` (le plan standard : pas de `--court`), puis `soiree-relais jouer <jeu>` (P35). Un jeu dont un geste ne s'annule que par Échap le voit annulé au second Échap hors de Chrome (`conception/BESOINS-DES-JEUX.md`, besoin 25).

### Phase 3, lot 10a : l'autorité dans le navigateur, et la reprise

La partie qui fait foi tourne enfin dans le navigateur de l'ordinateur de la soirée (`conception/PHASE-3.md`, § 2.3, § 2.4 et lot 10a ; `conception/LOTS-PHASE-3.md`, « Lot 10a »). Une crate nouvelle dans `web/` : `autorite`, le Worker ; `ecran` devient la page qui le lance et le garde ; `web/demarrage.js`, le seul JavaScript écrit à la main, charge l'un ou l'autre. Le relais sert le paquet de la page et les modules livrés. L'ABI ne change pas, ni rien de ce que l'épreuve joue ou juge : les modules gardent leur empreinte.

- **La reprise, par un seul chemin** : G, la clé de l'autorité, le secret d'entrée, le code de salle, le module joué et le dernier numéro de case écrit vivent dans IndexedDB de `http://localhost:7878`, jamais dans l'adresse ; un écran rechargé, un Worker tué, un second onglet relisent la garde et reprennent la partie. Dans Chrome et Firefox, chaque reprise a la forme canonique que wasmtime reprend des mêmes cases, et la soirée entière, rejouée dans wasmtime depuis sa première case, retrouve chaque empreinte (`soiree-banc soiree`).
- **Une seule autorité** : un second onglet de l'écran prend la salle ; le premier dit « Cette salle est maintenant ouverte dans un autre onglet — Reprendre ici ».
- **Deux garde-fous muraux et l'entrée qui bloque** : la page tue le Worker muet 2 s (1 s pendant `Bonjour` et `Decrire`), si elle a elle-même tourné ; au second arrêt pendant le même geste, le Worker neuf l'écarte, le téléphone le retire de sa file, et la page le dit en panne de l'outil. Au-delà de trois relances en une minute : « Le commun est en panne ».
- **F8** pose une marque, gardée dans le registre de chaque case de garde ; `soiree-banc rejouer-garde` rejoue la garde jusqu'à chaque marque.
- **La forme de la garde change** (`salle::garde::VERSION_CASES` 2 : le registre porte les marques et les gestes écartés) : une garde écrite avant ce lot ne se relit plus. Les messages entre la page et le Worker passent à `salle::ecran::PROTOCOLE` 2.
- **La politique de contenu du relais** gagne `img-src 'self' blob:` : la télé décode une image SVG par un `<img>` sur un `blob:`.
- Le banc gagne `soiree`, `arriere-plan` et `rejouer-garde`, et M12 (`soiree --m12`), la durée d'une reprise au pire moment (`MESURES-PHASE-3.md`, « Lot 10a »).
- **Après la vérification du lot 10a** (`LOTS-PHASE-3.md`, « Lot 10a, correction ») : un point de reprise se scelle par tranches de 128 Kio, une par réveil du Worker (`salle::chiffre::ScellementDeCase`, les mêmes octets qu'un scellement d'un coup), et non plus d'un coup dans le tour qui le prend ; la minuterie de secours du Worker bat toutes les 4 ms (et non 8), et joue le pas, images ou pas, dès qu'attendre son réveil suivant ferait manquer deux pas (`Autorite::secours_presse`). Ce sont les deux causes des sauts d'un pas de Punchline, onglet caché (au tour d'un point, et au moment où l'onglet se cache). Le banc juge chaque saut de la soirée (une reprise ou un retard injecté l'explique, sinon c'est une faute), constate que l'onglet d'`arriere-plan` est vraiment caché, y injecte le retard pendant qu'il l'est, et joue `--fois` passages. `salle` appelle `chacha20` et `poly1305` directement (les versions que `chacha20poly1305` tirait déjà).

#### Passer à cette version

Rien à faire pour un jeu : son API ne change pas. Une garde de relais écrite avant ce lot ne se relit plus (`soiree-relais oublier <SALLE>` la remet à neuf).

### Phase 3, lot 9 : le rendu

La télé dessine enfin la scène (`conception/PHASE-3.md`, § 2.9 et lot 9 ; `conception/LOTS-PHASE-3.md`, « Lot 9 »). Deux crates nouvelles dans `web/` : `rendu` (le dessin sur `wgpu` 30, Metal en natif pour ses tests, WebGPU ou WebGL2 dans le navigateur, les textes par `glyphon` 0.12) et `ecran` (la page de la télé, qui reçoit la scène du Worker et la dessine). Le banc gagne le scénario `dessin`. L'ABI ne change pas, ni rien de ce que l'épreuve joue ou juge : les modules gardent leur empreinte.

- **La télé dessine exactement ce que l'épreuve mesure.** Chaque texte se dessine d'après l'agencement de `mise-en-page`, par le tampon qu'elle en donne (`MiseEnPage::tampon`), dans les mêmes polices : la télé ne coupe ni ne mesure jamais elle-même. Constaté sur les pires cas des trois jeux dans Chrome (WebGPU et WebGL2) et Firefox (WebGL2) : chacun des 2 104 textes des pires cas (et les 1 000 d'une scène synthétique) agencé comme en natif, au bit près (`Agencement::octets_canoniques`). Le `Cargo.lock` de `web/` tient aux versions du commun ce que `mise-en-page` tire, et le banc le vérifie.
- **Ce que le commun dessine lui-même** : le nom d'un `Bouton`, sur un fond gris très sombre, en blanc, au centre, à la plus grande taille qui tient (`mise_en_page::bouton`) ; le texte d'un `CompteARebours`, en secondes arrondies au-dessus, `m:ss` dès une minute (`mise_en_page::compte`) ; le QR code de l'adresse, noir sur blanc avec sa marge de quatre modules ; une image absente ou que le navigateur ne décode pas, en gris. E16 ne mesure toujours ni les noms des boutons ni les comptes à rebours.
- **Les animations au repos** : un nœud est à son visuel d'arrivée, sauf sous `Courbe::Rebond`, qui revient à son départ (un saut, comme celui de Sidequest ; le sens des courbes est posé au mainteneur, `PHASE-3.md`, question 29).
- **Les images** : un SVG se décode par un élément `<img>`, tout autre format par `createImageBitmap`.
- **Aucune image de pire cas au-delà de 33 ms de travail du GPU fini**, mesuré dans Chrome et Firefox (`MESURES-PHASE-3.md`, « Lot 9 »).
- **Après la vérification du lot 9** (`LOTS-PHASE-3.md`, « Lot 9, correction ») : le banc juge plus strictement le scénario `dessin` (un point demandé qui manque au rapport, ou rapporté hors de la toile, ou une durée d'image qui manque, est un échec ; les points relus exactement, à 0 près, sont comptés) ; le contrôle négatif oublie, dans chaque scène, le nœud que le banc choisit pour que l'oubli se voie (`Fautif::OublierLeNoeud`, qui remplace `Fautif::OublierUnNoeud`) : il se voit désormais chez Punchline et Sidequest ; une image que le navigateur ne décode pas dit sa cause (`ecran::texte_d_erreur`). La police d'emoji monochrome n'est toujours pas livrée (`PHASE-3.md`, question 28).

#### Passer à cette version

Rien à faire : l'API d'un jeu ne change pas. Si ton jeu se sert de `Courbe::Rebond`, sache qu'il revient à son départ ; si tes images sont des SVG, elles se dessinent dans la télé.

### Phase 3, lot 8 : la mesure des textes

Le commun mesure enfin les textes de la scène (`conception/PHASE-3.md`, § 2.9, P21, P22 et lot 8 ; `conception/LOTS-PHASE-3.md`, « Lot 8 »). Une crate nouvelle, `mise-en-page`, sur `cosmic-text` 0.19 (sans GPU ni polices du système) ; l'épreuve s'en sert pour E1 et E16, la télé la dessinera au lot 9. L'ABI ne change pas : les modules gardent leur empreinte.

- **La règle de l'agencement** (en tête de `crates/mise-en-page/src/agencement.rs`) : la taille est celle du cadratin, **une ligne fait 1,2 fois la taille** ; `Coupe::Mots` coupe aux occasions de UAX #14 (un mot plus large que la boîte reste entier, et déborde), `Coupe::Graphemes` entre deux graphèmes, jamais dans un graphème (TL-55) ; en `Coupe::Mots`, l'espace devant `?`, `!`, `:`, `;` devient une espace fine insécable (SQ-46) ; `Taille::Budget` prend la plus grande taille, au 1/256 près, où le texte tient ; les lignes s'alignent selon l'`Alignement`, leur bloc se centre dans la hauteur ; un glyphe absent de toutes les polices se dessine comme une boîte, et se compte.
- **La police du commun** (`Cle::POLICE_DU_COMMUN`) est Liberation Sans 2.1.5, sous SIL OFL 1.1 (`crates/mise-en-page/polices/`). Pas de CJK, et pas encore d'emoji : la police d'emoji monochrome prévue n'est pas livrée ; elle attend la décision de l'hôte (fournir une police libre, ou en autoriser le téléchargement : `conception/PHASE-3.md`, question 28). Un test vérifie le sha256 de la police et de sa licence.
- **E1 valide les polices déclarées** (P22) : chacune, lue par `Ressources` sur une instance créée, doit être rendue, de sa taille déclarée, et un fichier sfnt (TTF ou OTF) que la mise en page lit. WOFF, WOFF2 et les collections sont refusés en le disant. Fautif : `police-woff2`.
- **E16 mesure les débordements** : chaque nœud `Texte` de la scène de chaque pire cas doit tenir dans sa boîte, même à la plus petite taille de son style ; un débordement nomme le pire cas, le texte, sa clé, sa boîte et la raison (trop large, trop haut, trop de lignes, ou un style qui ne se mesure pas : taille nulle, `min` au-dessus du `max`). La note de E16 dit les textes mesurés, les débordements et les glyphes absents. Les noms des boutons, les comptes à rebours et la valeur d'un `EnPlace` ne se mesurent pas encore. Fautif : `texte-qui-deborde`.
- `devine` ne change pas ; le socle des fautifs et `ardoise` prennent une plus petite taille d'une demi-unité, pour que leurs textes tiennent (le prénom de douze « W » du socle, le chiffre d'une case d'`ardoise`) : leurs modules changent d'empreinte.

#### Passer à cette version

1. Déclare tes polices en TTF ou OTF, jamais en WOFF ni WOFF2 : E1 refuse sinon (Punchline l'a fait au lot 7, Tier list déclare Liberation Sans).
2. Relance l'épreuve et lis E16 : chaque débordement nomme le pire cas, le texte, sa boîte et la raison. Corrige la boîte (une ligne fait 1,2 fois la taille), la plus petite taille, les `Lignes` ou la `Coupe` (un mot plus large que sa boîte ne se coupe qu'en `Coupe::Graphemes`). Les débordements trouvés au lot 8 sur les trois jeux sont listés dans `conception/LOTS-PHASE-3.md`, « Lot 8 » ; ils se corrigent au lot 7b.
3. Un jeu dont le workspace dépend de `epreuve` (Tier list) voit son `Cargo.lock` changer à sa prochaine compilation (`mise-en-page`, `cosmic-text` et ce qu'elle tire, parmi les dépendances de `epreuve`) : à committer dans le jeu, avant toute compilation `--locked`, en gardant `unicode-script` à 0.5.8 et `yazi` à 0.2.1 tant que leurs versions suivantes ont moins de deux semaines.

### Phase 3, lot 7 : les jeux passent à l'ABI 4

Les trois jeux passent à l'ABI 4 dans leurs dépôts (Punchline et Sidequest écrivent leur page, Tier list ses boîtes de composants, son sélecteur nommé, sa police libre et une autre touche de diagnostic) ; le commun arme ensuite les deux règles que le lot 6 laissait en attente (`conception/PHASE-3.md`, P33, § 2.10, § 8.3 et lot 7 ; `conception/LOTS-PHASE-3.md`, « Lot 7 »). L'ABI ne change pas (`jeu_abi::ABI` vaut 4), ni l'API d'un jeu.

- **F8 est au commun** (P33) : une déclaration dont la touche du diagnostic est F8 est refusée au chargement (`moteur_commun::chargement`, que partagent wasmtime et le navigateur : « déclaration refusée : diagnostic : F8 est la touche du commun… »). E1 échoue, et rien d'autre ne tourne. Fautif `diagnostic-f8`.
- **E21 refuse la page vide** d'un jeu qui a des téléphones : une page valide sans aucun nœud est un échec, sous des locaux vides comme tirés au hasard ; quand chaque page est vide, le rapport nomme la page par défaut de `Jeu::page`. La note de E21 dit combien de pages vides sur combien de valides. Fautif `page-vide`. Le socle des fautifs gagne `Defaut::page_par_defaut`.

#### Passer à cette version

1. Si ton jeu déclare `diagnostic: CodeTouche::F8`, prends une autre touche (Punchline et Sidequest ont `KeyD`, Tier list `F2`) : sinon le module est refusé au chargement.
2. Si ton jeu a des téléphones, écris `Jeu::page` (voir `crates/exemples/devine`) et assure-toi qu'aucune vue ne donne une page sans nœud : E21 échoue sinon. Un jeu sans téléphone n'a rien à faire.

### Phase 3, lot 6 : l'ABI 4

**L'ABI change** : `jeu_abi::ABI` vaut 4, et un module de l'ABI 3 est refusé au chargement ; chaque jeu se recompile contre ce commun (`conception/PHASE-3.md`, § 2.10, § 6, § 8.3 et lot 6 ; `conception/LOTS-PHASE-3.md`, « Lot 6 »). Les empreintes des modules et de leur forme changent avec.

- **La page du téléphone** est une fonction du module (P19) : `Jeu::page(vue: &Recue<Self::VueTelephone>, l: &Locaux<'_>, p: &mut Page<Self>)`, une page vide par défaut. Elle ne lit que la vue reçue et les locaux (l'écran, le texte des champs nettoyé par `Texte<L>`, les choix, les maintenus appuyés, les croix, ce que la page a retenu, les capacités) ; elle s'écrit en flux : `colonne`, `rangee`, `grille`, `pile`, `texte`, `image`, `image_teintee`, `compte_a_rebours`, `bouton` (au contact ou au relâchement), `maintenu`, `croix`, `champ` (`ChampPage`), `choix` (balayable : un carrousel), `tenir`, `retenir` (1 Kio). Sur le fil : `Appel::Page` et `Retour::Page(PageFil)`, une lecture sans état, que la colle traite comme `Valider` ; un jeu sans téléphone y répond `Illisible`. Le commun valide une page comme ce qui vient de l'extérieur (`jeu_abi::valider_page` : 256 nœuds, 16 étages, clés uniques parmi les frères, textes, options, tailles bornés, assets déclarés, gestes sous `limites.geste`).
- **La scène** : un composant peut avoir une boîte (`Scene::composant_dans(cle, boite, c)`, `SorteNoeud::Composant { boite: Option<Rect>, composant }`, vérifiée dans le repère) ; `ComposantVu::Modale` gagne `panneau`, `ComposantVu::Lien` gagne `fichier` ; un `Champ` ne demande plus qu'une ligne (`Lignes::Une`), sinon la vue est invalide (`DefautScene::ChampMultiligne`).
- **Les entrées et les réponses** : `Composant::Fichiers` dit son sélecteur (`selecteur: Option<Cle>`, `None` pour un dépôt ou un collage ; des fichiers qui nomment leur sélecteur ne passent que par lui) ; `Reponse::Rendu` rend une `ImageRendue { octets, non_dessinees }` (TL-56). `Demande::Telecharger` ne change pas.
- **La garde du cœur** : `partie::VERSION_GARDE` vaut 3 (une arrivée de fichiers et un rendu y changent de forme).
- **L'épreuve** : **E21, « Pages »**, dans le plan par défaut et dans l'étape 4 du CONTRAT : chaque vue envoyée aux téléphones des soirées (distinctes, au plus 256) donne, dans wasmtime, une page valide, sous des locaux vides puis tirés au hasard, et chaque geste et tenu de la page passe `Valider` ; sans objet pour un jeu sans téléphone ; la page vide d'un jeu à téléphones n'est pas encore refusée (lot 7). **E19** juge chaque page sur `BudgetsE19::page`, une lecture courante divisée par `MARGE_TELEPHONE` (4, choisi). **E1** refuse un module livré au-delà de `moteur_commun::TAILLE_MODULE_MAX`, 4 Mio (choisi). Trois fautifs nouveaux : `module-trop-gros` (E1), `page-qui-panique` et `page-qui-envoie-un-geste-invalide` (E21). Le socle des fautifs et `devine` ont une page.
- `moteur-commun` : `TAILLE_MODULE_MAX` ; `trace::NOMS` gagne `Page`. `reecriture::Reecrit` gagne `taille_originale` ; `moteur_wasmtime::ModuleCharge::taille_livree`.
- L'exemple `cargo run --release -p epreuve --example cout_d_un_noeud -- <ardoise_module.wasm>` mesure le carburant d'un nœud de scène.

#### Passer à cette version

1. Recompile ton module : un module de l'ABI 3 est refusé (« le module parle l'ABI 3, le moteur de 4 à 4 »).
2. Si ton jeu écrit `ComposantVu::Modale { fermer }`, ajoute `panneau` (la boîte de la fenêtre) ; s'il filtre `Composant::Fichiers { geste, fichiers }`, ajoute `..` (ou lis `selecteur`) ; s'il lit `Reponse::Rendu(Ok(o))`, `o` est une `ImageRendue` : `o.octets`, et `o.non_dessinees` à annoncer ; s'il filtre `SorteNoeud::Composant(c)` (dans ses tests), écris `SorteNoeud::Composant { composant: c, .. }` ; un `ComposantVu::Champ` doit avoir `lignes: Lignes::Une`. Tier list est adapté ainsi dans son dépôt, sans changer son comportement.
3. Un jeu à téléphones peut écrire sa page dès maintenant (`Jeu::page`, voir `crates/exemples/devine`) ; elle sera exigée au lot 7 (E21 refusera la page vide). Sans page, rien ne change.
4. Le rapport de l'épreuve a un contrôle de plus, E21 ; un jeu sans téléphone l'a sans objet.

### Phase 3, lot 5 : le relais et la soirée native

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes, ni pour ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.4, § 2.5, § 8.1 et lot 5 ; `conception/LOTS-PHASE-3.md`, « Lot 5 »).

- **`relais`**, une crate nouvelle, et le binaire **`soiree-relais`** : un cœur sans entrées-sorties (`relais::coeur` : les salles, les rôles, une seule autorité par salle, la plus récente dans l'ordre d'ouverture des liens, le routage des trames qu'il ne sait pas lire, les refus typés qui ferment leur lien, les liens sans bonjour en 10 s ou muets depuis 3 s fermés, un battement par lien toutes les 250 ms, le `Ping` de WebSocket) ; une coque `std` sans runtime (deux fils par lien, un fil pour les salles, un pour la garde, l'admission avant le fil avec une réserve pour la boucle locale) ; le HTTP sur le seul port (`GET /`, `GET /s/<salle>`, `GET /ws`, `GET /salle`, une page minimale en attendant celle du lot 10b), qui juge qui parle (`Host`, `Origin`, en-têtes de mandataire : SAL-33) ; la boucle locale dans les deux familles (un port pris dans l'une empêche le démarrage), chaque adresse des interfaces annoncée, `--adresse`, `--sans-reseau` ; la garde sur disque, écrite durablement (fichier temporaire, `sync_all`, renommage, dossier synchronisé), un fichier par numéro de case (`cases/<n sur 20 chiffres>`, la sorte de la case dans son premier octet : une case qui en remplace une autre de l'autre sorte ne laisse jamais son numéro vide), bornée (64 Mio par case, 256 Mio par salle) ; `soiree-relais oublier <SALLE>` ; `--banc` et `--enregistrer`. Il ne dépend que de `liaison`, et d'aucune crate de chiffrement (un test le vérifie sur `cargo metadata`).
- **`banc`** gagne le scénario **`soiree-banc soiree-native <module.wasm>`** : une soirée par le vrai relais, l'autorité native (`salle::Autorite` sur wasmtime) et des téléphones natifs (`salle::Appareil`) qui jouent les coups des bots, derrière un mandataire TCP de pannes (`banc::mandataire`), avec `kill -9` du relais (`--tuer-le-relais`) ; il juge la chaîne écrite, chaque geste une fois, tout accusé écrit, la garde finale relue sur le disque du relais, et cherche dans tout ce que le relais a vu les secrets de la soirée : les clés, les jetons, et chaque message de geste et de vue en clair, échantillonné sur les téléphones.
- **`salle::simulation`** : les jugements se partagent avec la soirée native (`accuses_hors_de_l_historique`, `reprendre_de_la_garde`, `forme_et_empreinte`, `reponse_par_defaut` publics) ; `Historique` compte aussi les entrées écartées, les tenus, les sauts, et garde les octets des gestes. `salle::appareil::Appareil::gestes_en_file` dit le numéro et les octets des gestes qui attendent leur accusé (le banc y prend les messages de geste qu'il cherche). Rien ne change de ce que la salle fait.
- Dépendances tierces nouvelles, dans `relais` : `httparse` 1.10.1 (déjà au `Cargo.lock`) et `if-addrs` 0.15.0 (qui tire `libc`, déjà au `Cargo.lock`).

#### Passer à cette version

1. Un jeu n'a rien à changer à son code, ni ses rapports d'épreuve.
2. Pour voir son jeu tourner par le vrai relais, sous pannes : compiler son module, puis, depuis `soiree/`, `cargo build --release -p relais -p banc` et `target/release/soiree-banc soiree-native <chemin du .wasm> --telephones 8 --duree 120`. Le code de sortie vaut 0 si tout tient.
3. Un jeu dont le workspace dépend de `epreuve` (Tier list) ne voit pas son `Cargo.lock` changer : `epreuve` ne dépend ni de `relais` ni de `banc`.

### Phase 3, lot 4b : la salle, sans entrées-sorties

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes, ni pour ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.3 à § 2.7, § 8.1 et lot 4b ; `conception/LOTS-PHASE-3.md`, « Lot 4b »).

- **`salle`** gagne ce qui fait tourner une soirée, toujours sans entrées-sorties, le temps et le hasard en paramètre :
  - **`salle::autorite`** : `Autorite`, la partie qui fait foi (`partie::Partie`) derrière ses deux liens au relais (« temps réel » et garde) ; le démarrage (bonjour au relais, garde listée et lue, partie ouverte ou reprise) ; les sessions Noise des appareils, sans aucun effet avant leur bonjour chiffré (SAL-17) ; le registre des jetons ; le lien le plus récent qui gagne, dans l'ordre d'ouverture des liens (SAL-18, SAL-19, SAL-21) ; un bonjour répété qui redonne le même accueil (SAL-29) ; l'accueil qui réaligne geste et tenu (SAL-38) ; chaque geste placé une fois par onglet, et accusé seulement une fois écrite la case de journal qui le contient (SAL-56, SAL-57) ; la demi-latence mesurée par l'autorité (SAL-129) ; les silences, les poignées de main inachevées coupées en 10 s ; les vues aux appareils et la scène à la page ; un tour borné (`avancer_borne`, `ARRIERES_PAR_TOUR`) au signal de chaque image. En **mode banc** (jamais en soirée), elle calcule les coups du bot de chaque appareil et de l'écran (`Partie::bot`), que les appareils natifs jouent par le vrai chemin (`VersAppareil::Banc`).
  - **`salle::appareil`** : `Appareil`, le cœur d'un téléphone sans sa page : bonjour au relais, poignée de main, bonjour chiffré redit sans accueil (2 s), gestes en file jusqu'à leur accusé et renvoyés après chaque accueil, file pleine dite (`Envoi::FilePleine`), tenu numéroté au-dessus de ce que l'accueil dit et redit à sa période, horloge de l'autorité estimée par écho, rechargement qui garde jeton, onglet et file (`sessionStorage`, P26), « Reprendre ici » après un remplacement.
  - **`salle::garde`** : la chaîne des cases (une seule suite de numéros, points et journal mêlés), leur contenu (`ContenuPoint` : ouverture, point, registre ; `ContenuJournal` : un morceau de journal à base absolue et le registre), `VERSION_CASES` = 1 ; `Ecrivain`, qui scelle chaque case à la suite de la précédente et dit quand effacer ; `relire`, qui vérifie la chaîne depuis le dernier point et refuse une garde plus vieille que le dernier numéro que la coque sait écrit, trouée, intervertie, renommée, abîmée, d'une autre salle ou d'une autre clé (`RefusDeGarde`).
  - **`salle::horloge`** : le temps de la partie à partir de l'ouverture (`Horloge`), l'heure de l'autorité estimée au plus petit aller-retour des 16 derniers échos (`Estimation`, SAL-127, SAL-128), l'aller-retour que l'autorité mesure (`AllerRetour`, SAL-129).
  - **`salle::ecran`** : les messages entre la page de l'écran et le Worker, `VersPage` et `DeLaPage`, `PROTOCOLE` = 1, figés avec leur seconde lecture (`tests/ecran.rs`).
  - **`salle::simulation`** : un relais en mémoire (le modèle de `soiree-relais`, lot 5) et un réseau simulé déterministe, à horloge qui saute d'échéance en échéance et graine fixe (SAL-09), qui font tourner la vraie autorité et de vrais appareils, avec leurs pannes : coupures, rechargements, onglets jumeaux, poignées de main rejouées par un invité, autorité tuée puis relancée depuis la garde. Le relais en mémoire n'écrit une case qu'au bout de son temps d'écriture, à l'instant de son `Ecrit` ; ce qu'une autorité tuée avait déjà donné à sa garde s'écrit jusqu'au bout. `Soiree::accuses_non_ecrits` dit tout geste accusé absent de la chaîne écrite. L'exemple `cargo run --release -p salle --example soiree_en_memoire -- <module.wasm>` y joue le module d'un jeu sur wasmtime ; il échoue aussi quand personne n'a joué (ni geste d'un téléphone, ni coup du bot de l'écran, que `Comptes::coups_ecran` compte).
- **`partie`** gagne deux lectures, sans rien changer à ce qu'elle joue : `Partie::bot` (les coups du bot du jeu, lus sur la définitive, pour le mode banc de la salle) et `Partie::presence` (ce que la définitive sait d'un appareil, que la salle relit après une reprise).
- `salle` dépend désormais de `partie` ; en développement, de `moteur-wasmtime` (les soirées en mémoire sur `devine` et sur les jeux). Aucune dépendance tierce nouvelle.

#### Passer à cette version

1. Un jeu n'a rien à changer à son code, ni ses rapports d'épreuve : l'épreuve joue et juge comme avant (le plan court des trois jeux rend la référence du § 7 de PHASE-3).
2. Pour voir son jeu tourner derrière la vraie salle, sous pannes : compiler son module (`cargo build -p <jeu>-module --target wasm32-unknown-unknown --profile module`), puis, depuis `soiree/`, `cargo run --release -p salle --example soiree_en_memoire -- <chemin du .wasm>`. Le code de sortie vaut 0 si chaque geste a compté une fois et que la garde reprend la même partie.

### Phase 3, lot 4a : la liaison et le chiffrement

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes, ni pour ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.6, § 2.7 et lot 4a ; `conception/LOTS-PHASE-3.md`, « Lot 4a »).

- **`liaison`**, une crate nouvelle (`no_std`) : les trames **en clair** entre le relais et les bouts, `VersRelais` et `DuRelais` (le code de salle, les liens, les rôles, la classe d'une trame, les cases de la garde, les adresses, les refus typés, les raisons d'une coupure et les causes d'une fermeture), `PROTOCOLE_RELAIS` = 1, et les constantes du lien : `TRAME_MAX` (256 Kio), `BATTEMENT_MS` (250), `SILENCE_MAX_MS` (3 000) et `SILENCE_MAX` (180 pas), `LIENS_MAX_SANS_AUTORITE` (16), les bornes de la redite d'un état tenu (`REDITE_MIN`, 100 ms ; `REDITE_MAX`, 250 ms) et `periode_de_redite` (P27 : le tiers de la plus courte de la fraîcheur et de l'âge maximal, entre les deux bornes ; 150 ms chez Sidequest, rien chez Punchline). `protocole_d_un_bonjour` lit le protocole d'un bonjour qui ne se décode plus (SAL-40). Le relais (lot 5) ne dépendra que d'elle.
- **`salle`**, une crate nouvelle, sans entrées-sorties : **`salle::chiffre`**, la poignée de main `Noise_NKpsk0_25519_ChaChaPoly_BLAKE2s` (par `snow`, sans charge, 48 octets par message), les trames de transport (ChaCha20-Poly1305 sous les clés du `Split`, nonce explicite, classe en données associées, fenêtre de rejeu de 1 024), les cases de la garde (XChaCha20-Poly1305, en-tête de la chaîne en données associées), les clés (`SecretDEntree` et sa psk, `PaireAutorite`, `CleDeGarde` et la preuve d'autorité) et le fragment du QR (`Fragment`, base64url strict) ; le hasard y entre en paramètre (`Hasard`), et `snow` reçoit un résolveur qui en tire, jamais le sien ; **`salle::fil`**, les messages chiffrés `VersAutorite` et `VersAppareil`, `PROTOCOLE` = 1. Les octets de chaque message, des dérivations, d'une poignée de main et d'une case à graine fixe sont figés, avec une seconde lecture sans serde ni postcard (SAL-90) ; le vecteur public de cacophony pour ce motif se retrouve octet pour octet.
- **`epreuve`** relit `liaison::SILENCE_MAX` et `liaison::REDITE_MAX` dans son téléphone simulé (`SILENCE_MAX`, `RAPPEL_TENU`), aux mêmes valeurs qu'avant (180 et 15 pas), une seule définition pour le monde simulé et la vraie pile. Le téléphone simulé redit toujours à 250 ms pour tous les jeux : passer à `periode_de_redite` changerait ce que l'épreuve joue, et attend M7 (lot 12).
- Dépendances tierces nouvelles : `snow` 0.10.0 (sans fonctionnalités par défaut, ni `getrandom` ni `ring`) et `chacha20poly1305` 0.10.1, dans `salle` seulement. Ni l'une ni l'autre ne tire `getrandom` pour `wasm32-unknown-unknown`.

#### Passer à cette version

1. Un jeu n'a rien à changer à son code, ni ses rapports d'épreuve : l'épreuve joue et juge comme avant.
2. Un jeu dont le workspace dépend de `epreuve` (Tier list) voit son `Cargo.lock` gagner une ligne à sa prochaine compilation (`liaison` parmi les dépendances de `epreuve`) : à committer dans le jeu.

### Phase 3, lot 3c : étalonner, la profondeur, Safari

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3). **Les empreintes réécrites changent toutes** : l'épreuve, la capture du harnais et le navigateur réécrivent désormais sous `ReglagesMoteur::PHASE_3` (`conception/PHASE-3.md`, § 2.3, P4 et lot 3c ; `conception/LOTS-PHASE-3.md`, « Lot 3c »).

- **`reecriture::Reglages::PHASE_3`** et **`moteur_wasmtime::ReglagesMoteur::PHASE_3`** : les réglages de la partie, les mêmes pour wasmtime et les navigateurs. Seule la profondeur change : **64 étages** au lieu de 4 096, réglée entre ce que les jeux atteignent sur toute leur épreuve au plan standard (18 au plus, Tier list) et ce que tient la pile d'un Worker (247 étages dans Chrome 156, 492 dans Firefox 158, sur les cadres les plus épais par étage) : M9. Sous `PHASE_1`, un Worker de Chrome aurait épuisé sa pile native avant la faute `Profondeur`. `PHASE_1` reste défini (les tests des jeux s'en servent encore).
- **La variante de mesure de la passe de profondeur**, réservée au banc et aux mesures : `reecriture::reecrire_en_mesurant_la_profondeur` (`passes::profondeur_mesuree`) garde la plus grande profondeur vue dans une seconde globale, `soiree_profondeur_atteinte` ; `moteur_wasmtime::Moteur::charger_en_mesurant_la_profondeur` la charge, et le protocole du commun en lit le maximum après chaque appel (`moteur_commun::profondeur_atteinte`). Le module brûle le même carburant et rend les mêmes octets ; seule son empreinte diffère. L'exemple `epreuve/examples/profondeur.rs` joue l'épreuve entière sur un module ainsi réécrit et dit la profondeur atteinte.
- **`soiree-banc etalonner`** : M1 (la trace de la capture du harnais rejouée dans un Worker de chaque navigateur, appel par appel, sur `moteur_navigateur::charger` et le protocole du commun : coût fixe, durée d'une unité, facteur navigateur/wasmtime, remise à zéro, instance neuve, instantané, restauration, réécriture, compilation), M10 (la boucle vide) et M9 (la profondeur que tiennent le Worker et la page). La page du harnais qui recopiait le protocole en JavaScript est retirée (`mesures/navigateur/page/banc-coeur.js`, `banc.js`, `banc.html`, `trace.js`, `mesurer.mjs`, `analyser.mjs`, `hors-navigateur.mjs`) ; le format de la trace a une seule définition, `moteur_commun::trace`, que la capture écrit (avec le module livré, `livre.wasm`).
- **Safari, par `safaridriver`** (`banc::safari`, WebDriver classique en HTTP, sans dépendance) derrière le même pilote que Chrome et Firefox, dans `machine`, `rejouer` (E20) et `etalonner`. Tant que l'hôte n'a pas autorisé l'automatisation à distance, `safaridriver` refuse la session, et Safari est « pas exécuté » avec son message. `moteur_navigateur::MoteurJs::JavaScriptCore` reconnaît Safari ; aucun de ses messages de pièges n'est constaté, et les essais de `Machine` échouent pour lui tant que la table est vide.
- **`BUDGET_DE_CHARGEMENT` revu d'après M10 dans les navigateurs, et baissé de 2²⁶ à 2²⁴ unités** : Firefox chargé a mis jusqu'à 6 092 ps par unité de boucle vide (410 ms pour 2²⁶ unités, deux fois et demie seulement sous le garde-fou d'une seconde) ; 2²⁴ en prend au plus 102 ms (calculé), et `Decrire` (au plus 9 322 unités chez les cinq modules) garde un facteur 1 800. Un module dont `Bonjour` ou `Decrire` brûlerait entre 2²⁴ et 2²⁶ unités est désormais refusé au chargement, en faute `Budget` (`MESURES-PHASE-3.md`, « Lot 3c »).
- **Après la vérification** : `soiree-banc etalonner --tours <n>` rejoue M1 sur plusieurs tours (un navigateur neuf par module et par tour, les tours s'enchaînant d'un navigateur à l'autre) et résume l'étendue de la médiane, du 99ᵉ centile et du maximum sur les tours. Son code de sortie vaut **1** dès un défaut, qui l'emporte sur « pas exécuté » : un écart de carburant, d'issue ou de retour avec wasmtime, une panne du rejeu, un module réécrit autrement que dans la capture, ou une récursion de M9 qui, chargée sous `PHASE_3` par `moteur_navigateur::charger`, n'est pas la faute `Profondeur` (M9 l'essaie maintenant dans le Worker et sur la page). Avant, il ne disait que si tout avait été mesuré. Un test du commun fige `reecriture::Reglages::PHASE_3.profondeur_max` à 64, entre les deux moitiés de M9. La mesure 1 rejouée en quatre tours, à côté du harnais retiré, retrouve le carburant, le coût fixe, la remise à zéro, les médianes et les 99ᵉ centiles, pas les maxima (`MESURES-PHASE-3.md`, « La mesure 1 en quatre tours »).
- **À la fin du lot** : `soiree-banc etalonner --fautif fausser-apres:<n>`, le contrôle négatif de M1 : la trace rejouée sur un moteur qui, passé `n` appels, lit le carburant restant d'une unité de trop ; il doit compter un écart au `n + 1`-ième appel, et le code vaut 1 (ses résultats bruts s'écrivent sous `etalonnage/<navigateur>-fautif/`) ; un contrôle qui ne voit pas son écart à cet appel, ou ne peut être jugé, est une panne de l'outil, le code 2, comme sous `rejouer --fautif`, et le résumé le liste sous `controle_rate`. En M9, un essai sans plafond que la pile native n'arrête pas est un défaut (code 1), et non plus un avertissement. Dans le rejeu de M1, un instantané ou une restauration qui nomme une instance ou un instantané absent est une panne du rejeu (code 1), et non plus un événement sauté. `banc` prend `jeu-abi` en dépendance de test. La mesure 1 refaite sur machine calme, en alternance avec le harnais retiré (huit tours chacun) : aucun écart, les médianes retrouvées (à deux tours près sur huit dans Chrome), les queues non ; le critère de `etalonner` devient « zéro écart, et médianes dans l'étendue du harnais », les queues à remesurer au lot 13 (PHASE-3, question 27 ; `MESURES-PHASE-3.md`, « La mesure 1 sur machine calme »).
- Aucune dépendance tierce nouvelle. La capture du harnais dépend de `moteur-commun`.

#### Passer à cette version

1. Un jeu n'a rien à changer à son code. Ses rapports d'épreuve et ses empreintes réécrites changent : relancer l'épreuve. Un jeu dont un appel imbrique plus de 64 étages tomberait désormais en faute `Profondeur` ; aucun des trois n'en approche (18 au plus).
2. Les tests d'un jeu qui chargent son module par `ReglagesMoteur::PHASE_1` continuent de passer ; pour éprouver ce que joue l'autorité, passer à `ReglagesMoteur::PHASE_3`.
3. Pour E20 avec Safari : `--navigateurs chrome,firefox,safari`. Tant que l'automatisation à distance n'est pas autorisée sur la machine, Safari est « pas exécuté », et E20 avec lui : le code de sortie vaut alors 3. Sans Safari, `--navigateurs chrome,firefox`, comme au lot 3b.

### Phase 3, lot 3b : le second moteur de l'épreuve

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes, ni pour ce que le plan par défaut de l'épreuve joue et juge : E20 ne tourne que si on le demande (`conception/PHASE-3.md`, § 8.2, § 8.3 et lot 3b ; `conception/LOTS-PHASE-3.md`, « Lot 3b »).

- **E20, « Deux moteurs »**, un contrôle long : `soiree-epreuve <jeu> --navigateurs chrome,firefox` joue le plan dans wasmtime, reprend chaque soirée jouée depuis sa création avec un relevé de chaque appel (carburant, issue, faute classée, empreinte courte du retour ; chaque instantané, restauration et empreinte ; la forme canonique finale), puis lance `soiree-banc rejouer`, qui reprend les mêmes soirées dans un Worker de chaque navigateur et compare chaque événement au relevé de wasmtime. Un résultat par navigateur au rapport (texte et JSON, champ `e20`) : réussi, échec (le premier événement qui diverge, avec la soirée et l'entrée), panne de l'outil, ou pas exécuté (Safari, jusqu'au lot 3c). E20 n'est pas du plan par défaut (`IdControle::DU_PLAN`) : un code 0 sans lui suffit pour jouer (P17). Il n'est dans aucune étape du CONTRAT.
- **Son contrôle négatif, joué à chaque E20** : le moteur fautif `fausser-apres:<n>` (une machine du navigateur qui, passé `n` appels, lit le carburant restant d'une unité de trop) doit diverger à l'appel `n + 1`, sur le carburant ; sinon E20 est une panne de l'outil. `--moteur-fautif <n>` le fait jouer à la place du vrai moteur (`CommunFautif::FausserApres`) : E20 échoue, et lui seul ; le rapport le dit.
- **`moteur_commun::releve`** : le relevé, sa forme, sa comparaison (en chemin : le rejeu s'arrête à la première différence) et la reprise des soirées, une seule fois pour wasmtime et le navigateur. Avant les soirées, deux fautes `Budget` provoquées sur chaque module (`Decrire` à la moitié de son carburant, puis à une unité près ; `releve::budgets_provoques`) : les journaux d'aujourd'hui ne font refaire que des `Panique`, et sans elles aucune autre sorte de faute ne se comparerait d'un moteur à l'autre. Wasmtime doit les classer `Budget`, sinon E20 est une panne de l'outil.
- **Le banc ne se croit pas sur parole non plus** : son code de sortie doit s'accorder avec son résultat, sinon E20 est une panne de l'outil ; un banc qui s'arrête avant tout navigateur garde la nature de son arrêt (« pas exécuté » pour un navigateur absent ; panne de l'outil pour la règle « mêmes lints » qui refuse `web/`, un résultat illisible ou jamais écrit).
- **`partie::Partie::reprendre_en_refaisant_les_fautes`** : comme `reprendre`, mais chaque entrée écartée du journal est rejouée, sa faute vérifiée, puis la définitive reconstruite, comme la partie l'a fait. Sans elle, une reprise saute les fautes, et E20 ne les comparerait pas. `reprendre` ne change pas.
- **`banc`** : un même pilote pour Chrome (CDP) et Firefox (WebDriver BiDi, sans `geckodriver`), `soiree-banc machine --navigateur firefox`, et le scénario `soiree-banc rejouer <dossier>`. Le jugement ne croit pas la page sur parole : le relevé qu'elle obtient doit avoir l'empreinte BLAKE3 de celui que le banc a servi.
- **`moteur-navigateur`** : les messages de pièges de SpiderMonkey (Firefox 158) rangés à côté de ceux de V8, chacun avec son moteur (`MoteurJs`) ; `index out of bounds`, que Firefox lève d'un accès hors de la mémoire comme d'une table, est ambigu (`AMBIGUS` désigne désormais une faute `HorsMemoire` ou un code de wasmtime). `ModuleCharge::instance_sur`, pour une machine qui enveloppe la vraie (le moteur fautif).
- Aucune dépendance tierce nouvelle. `banc` et `epreuve` dépendent de `moteur-commun`.

#### Passer à cette version

1. Un jeu n'a rien à changer à son code. Un jeu dont le workspace dépend de `epreuve` (Tier list) voit son `Cargo.lock` gagner une ligne à sa prochaine compilation (`moteur-commun` parmi les dépendances de `epreuve`) : à committer dans le jeu. `IdControle::TOUS` a vingt contrôles ; un plan qu'un jeu écrirait à la main avec `TOUS` demanderait E20, qui serait « pas exécuté » sans navigateur : qu'il prenne `IdControle::DU_PLAN`.
2. Pour E20 : `cargo build --release -p banc -p epreuve` dans le commun, puis `target/release/soiree-epreuve ../games/<jeu> --navigateurs chrome,firefox` ; l'épreuve trouve `soiree-banc` à côté d'elle, ou par `SOIREE_BANC`.

### Phase 3, lot 3a : le moteur du navigateur

Rien ne change pour l'API d'un jeu, ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes, ni pour ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.2, § 2.3 et lot 3a ; `conception/LOTS-PHASE-3.md`, « Lot 3a »).

- **Un second workspace, `web/`** (P31), pour ce qui ne sert que dans le navigateur. Ses `[workspace.lints]`, son `clippy.toml`, son `rust-toolchain.toml` et ses profils (sauf `module`, propre aux modules des jeux) sont ceux du commun, à l'identique, et chacun de ses membres écrit `[lints] workspace = true` : la règle « mêmes lints » le vérifie (`crates/banc/src/web.rs`), avec son témoin `web/temoins/lints-differents/`, qu'elle refuse.
- **`web/moteur-navigateur`** : le module réécrit dans le navigateur. `MachineNavigateur`, l'`impl moteur_commun::Machine` sur `WebAssembly.Module`, `Instance`, `Memory` et `Global` (la mémoire par un `Uint8Array` relu à chaque opération, les `i64` par `BigInt`, le type des globales lu dans le module par `wasmparser`) ; `classer`, qui ramène une erreur du navigateur à un piège brut : un `RuntimeError` par son message (les messages de V8 constatés dans Chrome 156 prennent le nom que wasmtime donne au même piège, `MESSAGES`, sauf `float unrepresentable in integer range`, qui en désigne deux, `AMBIGUS`), un `RangeError` ou un `InternalError: too much recursion` en pile native ; `charger` et `FabriqueNavigateur`, comme `moteur-wasmtime`. La machine du navigateur n'a pas de garde-fou mural : elle ne rend jamais `Piege::Interrompu`. Dépendances : `wasm-bindgen` = 0.2.129 (la version de l'outil installé), `js-sys` 0.3.106 (sans `unsafe-eval`), `wasmparser` 0.245.1.
- **Une crate de plus dans le commun, `banc`**, et le binaire `soiree-banc` : un pilote CDP minimal de Chrome sans interface (profil neuf, effacé à la fermeture), un petit serveur HTTP sur la boucle locale (COOP et COEP posés) qui reçoit `POST /banc/rapport`, la règle « mêmes lints » (`soiree-banc regles-web`) et le scénario `soiree-banc machine` : les essais de `Machine` (lire, écrire, remplir, grandir, globales `i64` et des autres types, `soiree_boite` et `soiree_appel`, instance neuve, onze pièges classés, un chargement de bout en bout) joués dans Chrome, puis leur contrôle négatif (une machine qui lit les `i64` par un nombre : les essais `globales_i64` échouent, et eux seuls). Code 0 réussi ; 1 échec (dont la règle « mêmes lints » qui refuse `web/`, et une page qui ne rapporte rien ou rapporte une erreur) ; 2 pas exécuté (Chrome absent, construction impossible). Le jugement compte les essais (33) et vérifie que le rapport est celui du passage joué. Dépendance nouvelle : `tungstenite` 0.30.0 (`handshake` seul).
- `#[wasm_bindgen]` n'exige pas de code `unsafe` (question 5, P32) : les crates de `web/` posent `#![forbid(unsafe_code)]` comme les autres.

#### Passer à cette version

1. Un jeu n'a rien à faire : ni son code, ni son `Cargo.lock` ne changent (`banc` et `web/` ne sont des dépendances d'aucun jeu).
2. Changer une règle d'écriture du commun (`[workspace.lints]`, `clippy.toml`, `rust-toolchain.toml`) se fait aussi, à l'identique, dans `web/` ; sinon `crates/banc/tests/web.rs` rougit.

### Phase 3, lot 2 : le moteur en deux

Rien ne change pour l'API d'un jeu ni pour l'ABI (`jeu_abi::ABI` vaut 3), ni pour les empreintes : le même module réécrit donne, sur la soirée de `devine`, les mêmes empreintes avant et après (`conception/PHASE-3.md`, § 2.3, lot 2 ; `conception/LOTS-PHASE-3.md`, « Lot 2 »).

- **Une crate de plus, `moteur-commun`** (P2) : le protocole d'un appel, une seule fois pour tous les moteurs. Le trait `Machine` (les primitives d'un moteur : lire, écrire et remettre à zéro la mémoire, la faire grandir, lire et poser une globale par son nom, appeler `soiree_boite` et `soiree_appel`, une instance neuve) et le piège brut `Piege` ; `Protocole<M: Machine>`, une `partie::Instance` ; le classement d'un piège, dans l'ordre d'avant (tenu par `classer_garde_son_ordre`) ; l'empreinte ; `verifier_chargement` (`Bonjour`, zones, `Decrire`, déclaration) ; `RefusModule` ; le module d'essai `essais::BONJOUR_QUI_BOUCLE`. `no_std`, sans entrées-sorties.
- **`moteur-wasmtime` est réduit à `impl Machine` sur wasmtime**, l'époque du garde-fou et le fil de chaque instance. Son API ne change pas : `Moteur`, `ReglagesMoteur`, `ModuleCharge`, `InstanceWasm`, `FabriqueWasm`, `InstantaneWasm` (désormais un alias de `moteur_commun::Instantane`), `RefusModule` et `BUDGET_DE_CHARGEMENT` (réexportés de `moteur-commun`). Nouveaux : `ModuleCharge::carte` et `ModuleCharge::carburant_du_chargement`, l'exemple `boucle_vide` (M10). Le garde-fou mural borne toujours un appel en entier, `soiree_boite` et `soiree_appel` ensemble : son échéance se pose une fois, dans `Machine::boite`.
- **L'instantané ne garde plus que la mémoire hors des quatre zones** (P3) : la pile, la boîte, le brouillon et la zone de panique, remises à zéro avant chaque appel et exclues de l'empreinte, ne sont plus copiées, et la restauration les remet à zéro. Chez Tier list, un plateau neuf garde 323 584 octets au lieu de 25,4 Mio ; le pire cas, 16,8 Mio (M3). Sa forme gardée change : **`partie::persistance::VERSION_GARDE` vaut 2** ; une garde de la version 1 est refusée (`RefusGarde::Version`). Les octets figés de l'instantané sont dans `crates/moteur-commun/src/instantane.rs` ; ceux des types de `partie` n'ont pas changé.
- **`BUDGET_DE_CHARGEMENT` passe de 2³² à 2²⁶** (P5 ; RESTE-PHASE-2, point 16) : une boucle dans `Bonjour` ou `Decrire` est la faute `Budget` du jeu sous le garde-fou d'une seconde de `ReglagesMoteur::PHASE_1`, même sur une machine chargée, au lieu d'une panne de l'outil. Mesuré : `Bonjour` et `Decrire` des cinq modules brûlent au plus 9 322 unités.
- `epreuve` compte `moteur-commun` parmi les crates du commun (étape 1, `sources.rs`).

#### Passer à cette version

1. Un jeu n'a rien à faire dans son code. Son `Cargo.lock` gagne `moteur-commun` (que `moteur-wasmtime`, pris par chemin, tire désormais) : Cargo l'écrit à la première compilation ; c'est un changement à committer dans le dépôt du jeu.
2. Un `Bonjour` ou un `Decrire` qui brûlerait plus de 2²⁶ unités est refusé (faute `Budget`). Aucun des cinq modules mesurés n'en approche (au plus 9 322).
3. Du code qui lisait les champs de `InstantaneWasm` (aucun hors du moteur) lit `pages()`, `plages()`, `octets()` et `globales()` de `moteur_commun::Instantane`. Une garde écrite avant ce lot ne se relit plus.
4. Changer la forme de l'instantané monte `VERSION_GARDE` et réécrit à la main les octets figés de `crates/moteur-commun/src/instantane.rs` (et non plus de `moteur-wasmtime/src/interieur.rs`, retiré).

### Phase 3, lot 1 : le cœur prêt pour le monde

Rien ne change pour l'API d'un jeu ni pour l'ABI (`jeu_abi::ABI` vaut 3). Ce qui change dans `partie` et dans ce que l'épreuve joue et juge (`conception/PHASE-3.md`, § 2.4, § 2.11, lot 1 ; `conception/LOTS-PHASE-3.md`, « Lot 1 ») :

- **Un pas trop chargé d'arriérés se finit sur plusieurs tours** (P7). `Partie::avancer_borne(cible, arrieres_par_tour)` joue au plus `arrieres_par_tour` entrées d'un arriéré (un geste ou une entrée de l'écran faits sur une vue de plus de `partie::ARRIERE_PAS` pas avant leur réception) ; au-delà, le pas reste en cours (`Avancee::en_cours`, `Partie::pas_en_cours`) et se finit au tour suivant. Les arrivées d'un pas se figent au tour où il commence, et celles d'une avancée au tour où elle commence : une arrivée reçue pendant une avancée inachevée va après sa fin (son dernier pas, puis son saut), comme si l'avancée avait fini d'un seul tour. Le journal ne dépend pas du découpage, ni les vues de l'écran : avec une fenêtre, la tête juge à la fin de l'avancée les arrivées reçues pendant elle (CAT-14). `Partie::avancer` finit toujours d'un seul tour, comme avant. Nouveaux : `partie::ARRIERE_PAS` (60), `partie::ARRIERES_PAR_TOUR` (8), `Partie::avancer_decoupe` (pour éprouver le découpage), `Partie::finir_le_pas`, `Partie::fin_du_dernier_pas`, et le champ `Avancee::en_cours`.
- **Le journal a une base d'indices absolus** (P10) : `Journal::base`, `Journal::fin`, `Journal::ligne`, `Journal::tronquer_avant`, `Partie::tronquer_journal_avant` (jamais au-delà du dernier point). `Journal::lignes` ne rend que les lignes gardées, la première d'indice `base`. `Partie::reprendre` accepte un journal tronqué ; un point avant sa base est refusé (`RefusReprise::PointHorsJournal`, dont le sens s'élargit).
- **La forme de la garde** : `partie::persistance` encode et relit un point de reprise (sa chaîne `sain` et `periodique` aplatie et dédoublonnée), le journal ou un morceau de journal, l'ouverture et l'état du cœur, derrière un en-tête de cinq octets (`VERSION_GARDE` en `u32` petit-boutiste, puis la sorte). L'instantané s'écrit par le trait `persistance::InstantaneGarde`, implémenté pour `moteur_wasmtime::InstantaneWasm` et le modèle des tests. Les types gardés de `partie` (lignes du journal, `EtatCoeur`, `Ouverture`, `Budgets`, `Faute`) dérivent `Serialize` et `Deserialize`. `PointDeReprise::sain` et `periodique` se lisent.
- **Une seule définition des budgets de la soirée** : `partie::BUDGETS_DE_SOIREE` ; `epreuve::BUDGETS` en est désormais un alias (même valeur, 2²⁸ par sorte d'appel), et `epreuve::ARRIERE_PAS` relit `partie::ARRIERE_PAS`.
- **L'épreuve joue un tour par pas simulé**, comme le Worker un par image (`avancer_borne` à `ARRIERES_PAR_TOUR`), et **E19 juge chaque tour** : l'image d'un tour est son coût, plus, s'il a fini son avancée, celui des entrées placées aux `fenêtre + 1` pas qui suivent (le rejeu de la tête) ; sans découpage, c'est la somme de `(fenêtre + 2)` pas, comme avant. Le coût fixe de chaque entrée compte. La règle de la phase 2 qui ne comptait pas le coût fixe des arriérés est retirée. Après un gel, un jeu peut donc voir un verdict de E19 changer : ses arriérés se jouent huit par tour, et le rattrapage qui suit joue plusieurs pas dans un même tour.
- **E12 juge aussi le découpage d'un pas, pour tout jeu** : le script de la soirée des propriétés, rejoué en tours d'une entrée, avec les arrivées reçues pendant le pas découpé (la seconde moitié d'une rafale d'au moins trois arrivées passe juste après le pas qui la jouait), doit donner le même journal, ligne pour ligne, et les mêmes vues ; une soirée où aucune arrivée ne suit un pas découpé compare encore les pas joués en plusieurs tours, et le rapport le dit. E12 n'est donc plus sans objet pour un jeu sans fenêtre, sauf si aucun pas de cette soirée n'a deux entrées venues d'arrivées (un jeu où l'écran seul joue, comme `ardoise`). Son contrôle négatif est un défaut du commun, non un jeu : `Plan::commun_fautif = Some(CommunFautif::EtaleMal)`.
- **E14 reprend par la garde** : chaque reprise de l'épreuve part d'un point et d'un journal écrits puis relus par `partie::persistance`, le journal tronqué avant le point. La soirée longue dit la longueur de son journal, en lignes et en octets.

#### Passer à cette version

1. Un jeu n'a rien à faire : relancer `soiree-epreuve` depuis le dossier du jeu, et lire E12 et E19, dont ce qui est jugé a changé.
2. Du code qui construit un `Avancee` littéral ajoute `en_cours: None`. Du code qui lit `Journal::lignes()` par un indice venu d'un point ou d'un rapport soustrait `Journal::base()` (ou passe par `Journal::ligne(indice)`) ; tant que rien n'est tronqué, la base vaut 0.
3. Du code qui construit un `epreuve::Plan` littéral sans `..Plan::court(…)` ajoute `commun_fautif: None`.
4. Changer la forme d'un type gardé (un champ, un variant, leur ordre) monte `partie::persistance::VERSION_GARDE` et réécrit à la main les octets figés de `crates/partie/tests/coeur/garde.rs`, de `forme_garde.rs` et de l'instantané de `moteur-wasmtime` (`src/interieur.rs`).

### Après la phase 2 : les fautifs à `static` à part, et l'épreuve nomme un `static`

Rien ne change pour un jeu : ni l'ABI, ni l'API de `jeu`, ni ce que l'épreuve juge. Deux points de `conception/RESTE-PHASE-2.md` sont faits (1 et 2) :

- **Les fautifs repassent l'étape 1 depuis leurs sources.** Les cinq défauts dont la faute est un `static` (E6 `refuse-apres-avoir-ecrit` et `retient-un-refus`, E7 `vue-qui-ecrit`, E8 `secret-dans-un-static`, E14 `etat-hors-enveloppe`) quittent `fautifs` pour une crate à eux, `crates/exemples/fautifs-statiques` ; leurs types se nomment `fautifs_statiques::jeux::<Nom>` au lieu de `fautifs::jeux::<Nom>`, et leurs crates de module, qui restent dans `crates/exemples/fautifs/modules/<nom>`, dépendent de `fautifs-statiques`. Depuis leurs sources, `sain` et `avec-fenetre` rendent 0, un fautif sans `static` ne fait échouer que son contrôle ; un fautif à `static` échoue l'étape 1 et son contrôle (tests `sain_et_un_fautif_sans_static_depuis_leurs_sources` et `un_fautif_a_static_echoue_l_etape_1_en_nommant_le_static`, `crates/epreuve/tests/sources.rs`).
- **La règle « rien hors de l'état » nomme le `static`** qu'elle refuse : ``un `static` `NOM` : une valeur gardée hors de l'état (écrire un `const`)``, au lieu de ``un `static` : …``. Un outil qui lit ce texte dans le rapport doit s'y attendre.
- **`moteur-wasmtime`, `charger::bonjour_qui_boucle_refuse_avec_sa_faute` ne dépend plus de la charge** : son garde-fou mural passe de 2 s à une minute, et seul le carburant de chargement réel (`BUDGET_DE_CHARGEMENT`) arrête la boucle (OUT-11). Le test prend environ 2 s, plus sous la charge.

#### Passer à cette version

1. Un jeu n'a rien à faire.
2. Du code qui nomme l'un des cinq fautifs à `static` (`fautifs::jeux::RefuseApresAvoirEcrit`, `RetientUnRefus`, `VueQuiEcrit`, `SecretDansUnStatic`, `EtatHorsEnveloppe`) dépend de `fautifs-statiques` (`fautifs-statiques.workspace = true`) et écrit `fautifs_statiques::jeux::<Nom>`. Le trait `fautifs::defauts::Defaut` et le socle ne changent pas.
3. Un défaut de plus qui tient un `static` va dans `fautifs-statiques`, jamais dans `fautifs`.

### Le contrôle final de la phase 2

Rien ne change pour un jeu : ni l'ABI (`jeu_abi::ABI` vaut 3), ni l'API de `jeu`, ni l'épreuve. Le contrôle final (2026-11-05, `conception/RESTE-PHASE-1.md`, « Contrôle final de la phase 2 ») a tout relancé depuis zéro ; ce qui reste après la phase 2, et ce que la phase 3 devra mesurer ou trancher, est dans `conception/RESTE-PHASE-2.md`. Deux choses à savoir en éprouvant :

- **Les fautifs du commun ne passent pas l'étape 1 depuis leurs sources** (corrigé depuis : voir « Après la phase 2 » ci-dessus) : la crate `fautifs`, que toutes leurs crates de module partagent, tient cinq `static` (ceux des défauts E6, E7, E8 et E14), que la règle « rien hors de l'état » refuse. `soiree-epreuve crates/exemples/fautifs/modules/<nom>` rend donc 1 même pour `sain` et `avec-fenetre`. Pour voir un fautif échouer exactement son contrôle, l'éprouver par son module et sa copie native (`cargo run --release -p epreuve --example natif -- <nom> <module>`, ou le test `plan_standard`). Un jeu n'est pas touché.
- **Le plan standard est ce qui fait foi** ; le plan court ne garantit pas le résultat d'un jeu (Punchline y échouait E2 et E15 en phase 1, faute de temps pour deux parties).

### La forme, les sources et l'ABI 3 (phase 2)

Ce qui change pour un jeu écrit sur le commun de la phase 1 :

- **ABI 3** (`jeu_abi::ABI`) : `Retour::Proprietes` porte le nombre de variantes de `Phase` (E3 juge chaque phase, qu'un pire cas y mène ou non) et les noms des invariants (E4). Un module compilé contre l'ABI 2 est refusé au chargement : recompiler.
- **`Epreuve::INVARIANTS`**, sans valeur par défaut : `const INVARIANTS: &'static [&'static str] = &["…", …];`, la liste exacte des noms passés à `jeu::exiger!` dans `invariants` et `transition`. Un nom vide ou déclaré deux fois ne compile pas ; un nom déclaré jamais évalué dans une soirée, ou évalué sans être déclaré, fait échouer E4.
- **`Regard` n'est plus `Clone` ni `Copy`**, et ne se construit que dans `jeu` (`jeu::essai::regard` pour les tests natifs, absent sous `wasm32`). `jeu::colle::appeler`, `canonique`, `wasm::boite` et `wasm::appel` demandent un `&colle::Hote`, dont le constructeur est `unsafe` ; `colle::enveloppe_de_test` est retirée. `jeu-natif` est vide sous `wasm32`.
- **Les règles d'écriture** : `usize` n'est ni `Etat` ni `Message` (écrire `u32` ou `u64`) ; `f32::min`, `f32::max`, `f64::min` et `f64::max` sont interdits par `clippy.toml` (écrire `jeu::math::fminf`, `fmaxf`, `fmin`, `fmax`, ou une comparaison) ; la police du commun se nomme `Cle::POLICE_DU_COMMUN`.
- **`reecriture::VERSION` vaut 3** : `memory.init` paie sa longueur divisée par 64 ; une fonction a au plus `LOCAUX_MAX` (49 984) paramètres et locaux et `HAUTEUR_MAX` (1 024) valeurs sur la pile d'opérandes ; `soiree_profondeur` compte en étages de 4 096 octets de pile native. Le moteur remet à zéro la pile, la boîte, le brouillon et la zone de panique avant chaque appel.
- **L'épreuve part des sources** : `soiree-epreuve <dossier du jeu>` (étape 1 : règles d'écriture, clippy natif et `wasm32`, module et copie native compilés par l'épreuve). La crate de module se réduit à `jeu::module!(<chemin du type du jeu>);` ; la crate `<jeu>-epreuve` écrite à la main n'est plus nécessaire. Le rapport nomme les sept étapes du CONTRAT ; un module seul (`--wasm`) rend 3 au mieux.
- **Les contrôles** : la soirée appelle `Partie::repartir` quand la partie tombe en panne (fautif `panique-au-temps`, E2) ; E6 rejoue l'entrée acceptée qui suit des refus sur l'instantané d'avant eux (« même avenir », fautif `retient-un-refus`) ; E8 rejoue un geste ou un tenu avec un autre dernier octet (fautif `choix-qui-fuit`) ; E3 et E4 comme ci-dessus (fautifs `phase-sans-cas`, `invariant-jamais-evalue`). `etat-hors-enveloppe` échoue désormais E14 sur le plan standard.

Ajouts sans rupture dans `partie` : `Partie::repartir`, `Ligne::Reprise` (un `match` exhaustif sur `Ligne` doit le nommer), `EtatCoeur::reveil`, `Partie::numero_tenu`.

Pour s'adapter : déclarer `INVARIANTS` ; remplacer tout `usize` de l'état ou d'un message, tout `f32::min`/`max`, tout `Cle::du_fil(0)` ; réduire la crate de module à `jeu::module!`, retirer la crate d'épreuve écrite à la main ; reprendre `clippy.toml` et les profils du commun ; relancer `soiree-epreuve` depuis le dossier du jeu.

### L'épreuve, après la revue de la phase 2

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu. Les constats sont numérotés dans `conception/RESTE-PHASE-1.md`, « Revue de la phase 2 ».

- **L'étape 1 refuse de nouvelles choses** (règles nouvelles au rapport) :
  - « rien hors de l'état » : aucun `static` (atomique, `Cell`, `OnceLock`, `static mut`, ou même immuable) ni `thread_local!` dans le code des crates du jeu, hors de `tests/`. Écrire un `const` à la place d'un `static` immuable. Une valeur qui doit durer va dans l'état.
  - « forme calculée » : aucun `impl Forme`, `impl Etat` ni `impl Message` écrit à la main, aucun `Etat as …` (ou `Forme as`, `Message as` ; `as _` reste permis), aucun usage de `jeu::cache`, `Ecrivain` ou `Liseur`. Seules `#[jeu::etat]` et `#[jeu::message]` écrivent ces traits.
  - « sans unsafe » : une crate par chemin qui entre dans le module sans être membre du workspace du jeu, ni du commun, échoue ; en faire un membre. Le code tiers (registre, git) reste permis, et le rapport le nomme, avec la présence ou non d'`unsafe` dans ses sources.
  - « build.rs » : ce que le `build.rs` d'une crate du jeu dit à Cargo (le fichier `output` que Cargo garde) ne peut être que `rerun-if-changed`, `rerun-if-env-changed`, `warning`, `error`, `rustc-check-cfg`, et, pour la crate de module seule, `rustc-link-arg-cdylib=--max-memory=<octets>`. Le rapport nomme les `build.rs` lancés sur la machine.
- **E8 recharge la projection dans une instance neuve**, et non plus dans la mémoire de la définitive : un `static` où une entrée recopie un secret ne passe plus dans la vue rechargée. **E8 varie aussi un geste ou un tenu d'un seul octet** (un vote en enum plate) ; le rapport dit quand la variation n'a rien jugé. **E8 compare les valeurs secrètes une à une** quand une longueur change : Punchline n'échoue plus à tort au dernier bulletin.
- **E16 compare chaque pire cas aux soirées** : si une vue de l'écran des soirées coûte plus de carburant que la vue du plus lourd des pires cas, E16 échoue. Un pire cas se construit depuis les limites.
- **Deux pannes de plus au plan standard** : `Panne::Depart` (un téléphone part pour de bon, assez tôt pour que la soirée dure encore 90 s au moins) et `Panne::Doublon` (le réseau livre deux fois le même geste). Un jeu dont une phase attend pour toujours un joueur parti peut maintenant échouer E5 dans une soirée principale.
- **Les arriérés de E19** ne sont plus que ce qui arrive dans les deux secondes après le retour d'un appareil ou la fin d'un gel, fait sur une vue d'avant ce retour. Un geste ordinaire sur un écran resté le même paie son coût fixe : un module aux grandes zones peut maintenant échouer E19 quand plusieurs téléphones répondent dans la même image.
- **La soirée longue (E15)** choisit la cible de chaque allée et venue parmi les téléphones présents quand elle commence ; une allée et venue manquée rend E15 « pas exécuté ».
- **Le rapport** : l'étape 4 est « faite en partie » (`IssueEtape::FaiteEnPartie`, `"faite_en_partie"` en JSON) tant que la comparaison avec et sans prédiction manque ; le verdict d'un code 0 nomme les étapes du CONTRAT que cette version ne fait pas. Les budgets de E19 sont dits « choisis », non « mesurés ». Les secrets non cherchés par E8 sont décrits avec leur borne exacte (moins de 4 octets, ou de 3 avec leur longueur).

Ruptures dans la crate `epreuve` : `Panne` a deux variants de plus (`Depart`, `Doublon`) ; `IssueEtape` en a un de plus (`FaiteEnPartie`) ; `Rapport::verdict_du_code_0` est nouvelle ; `RATTRAPAGE_PAS` est nouvelle. Deux fautifs nouveaux : `secret-dans-un-static` (E8) et `pire-cas-trop-leger` (E16). `devine` garde son curseur secret pour son joueur.

Pour s'adapter : relancer l'épreuve depuis les sources. Remplacer chaque `static` du code par un `const` ou par un champ de l'état ; supprimer tout `impl Etat`/`Forme`/`Message` écrit à la main au profit de `#[jeu::etat]`/`#[jeu::message]` ; rendre membre du workspace toute crate par chemin dont dépend la crate de règles ; ne garder dans un `build.rs` que ce qui est permis ci-dessus. Lire E5 (le départ pour de bon), E16 (un pire cas plus léger que l'écran d'une soirée) et E19 (le coût fixe des gestes ordinaires).

### Les budgets de E19, dérivés du temps d'une image

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu :

- **E19 juge un coût, pas seulement un carburant.** Le coût d'un appel est son carburant plus le coût fixe d'un appel du module : le passage de l'appel et la remise à zéro des zones que le moteur fait avant chaque appel (la pile, la boîte, le brouillon, la zone de panique), que le carburant ne compte pas. Pour un module aux zones de 2 Mio, de l'ordre de 156 000 unités par appel ; pour un brouillon de 24 Mio, de l'ordre de 1,3 million. Un jeu dont le brouillon est grand paie donc chaque appel, même vide.
- **Ce qui est jugé, et comment** : chaque image des soirées (la somme du coût des entrées de `(fenêtre + 2)` pas consécutifs) à son maximum, et leur 99ᵉ centile ; chaque lecture courante du cœur (`Regarder`, `Etat`, `Valider`, `Ressources`) à son maximum ; la vue de l'écran de chaque pire cas à son maximum, contre son propre budget, plus large que celui d'une lecture courante. Le 99ᵉ centile d'une entrée ou d'une lecture n'est plus jugé seul.
- **Les budgets sont provisoires, et le rapport le dit** : `epreuve::BUDGETS_E19` les dérive, à un seul endroit (`crates/epreuve/src/budgets.rs`), du temps d'une image du CONTRAT (33 ms au plus, 60 images par seconde), de mesures de wasmtime sur la machine de l'hôte (`examples/etalonner.rs`), d'une marge choisie (4) pour le navigateur et l'ordinateur de la soirée, et de la part de l'image laissée au jeu (la moitié). Aujourd'hui : entrées d'une image 10 312 500 unités au plus et 5 208 333 au 99ᵉ centile, lecture courante 5 208 333 au plus, vue d'un pire cas 10 312 500 au plus. Le mainteneur les fixera dans `CONTRAT.md`.
- **Une entrée d'un arriéré** (faite sur une vue de plus d'une seconde : un téléphone coupé ou en arrière-plan, un ordinateur de la soirée gelé) ne compte que son carburant dans les images, pas son coût fixe : leur nombre dans un pas est le fait du commun. Le rapport les compte à part.

Ruptures dans la crate `epreuve` : `BUDGET_ENTREE`, `BUDGET_LECTURE` et `BUDGET_IMAGE` sont retirés, remplacés par `BUDGETS_E19` (un `BudgetsE19`), ses constantes d'origine (`PS_PAR_UNITE`, `FIXE_NS`, `PS_PAR_KIO_REMIS_A_ZERO`, `MARGE_NAVIGATEUR`, `MARGE_MACHINE`, `PART_DU_JEU_POUR_CENT`, `PART_DES_ENTREES_POUR_CENT`, `IMAGE_MAX_NS`, `IMAGE_COURANTE_NS`) et `ARRIERE_PAS` ; `Rapport` a deux champs de plus, `budgets: BudgetsE19` et `octets_remis_a_zero: Option<u64>` ; le texte du rapport a une ligne « Budgets de E19 provisoires… », et le JSON un objet `budgets` (`"provisoires": true`). Ajout dans `moteur-wasmtime` : `ModuleCharge::octets_remis_a_zero`. Trois fautifs nouveaux, chacun pour E19 seul : `pire-cas-trop-lourd`, `fin-trop-lourde` et `zones-lourdes`.

Pour s'adapter : relancer l'épreuve et lire E19. Un échec par le coût fixe se corrige en réduisant les zones que le jeu déclare (`limites.brouillon`), un échec d'un pire cas en allégeant la vue de l'écran à ses limites déclarées, un échec d'une lecture courante en allégeant la vue la plus lourde que les soirées atteignent.

### Le monde simulé de l'épreuve ressemble au réel

Ce qui change pour un jeu, sans changer l'ABI (`jeu_abi::ABI` vaut toujours 3) ni l'API d'un jeu :

- **Les bots d'un téléphone décident sur la vue que ce téléphone a reçue**, en retard d'un trajet, et sur l'état de la définitive d'où elle vient (`bot_telephone` reçoit cet état-là). Un téléphone sans vue (à la connexion, après un rechargement) ne fait rien. Un bot qui comptait sur l'état courant pour ne jouer que des gestes acceptables en verra refuser : c'est voulu. Un geste porte, dans `quand.vu`, le pas de la vue que la page montrait quand il a été fait, même renvoyé après une coupure.
- **`quand.fait` porte l'erreur de l'horloge du téléphone**, jusqu'à un demi-aller-retour, dans les deux sens ; la demi-latence que le cœur reçoit est mesurée, gigue comprise. Un jeu qui juge sur `fait` ce qu'un joueur a vu se trompe : il faut lire `quand.vu` (fautif `juge-sur-le-fait`).
- **Le réseau** : un téléphone sur trois n'a que WebSocket (une perte retient la file) ; les tenus se perdent et se réordonnent sur WebRTC ; la page redit son tenu toutes les 250 ms ; les poignées de main ratent ; un onglet rechargé renumérote ses tenus, et l'accueil le réaligne ; une panne nouvelle, `Panne::ArrierePlan { duree }`, relâche le tenu, fige la page, et ferme sa connexion au-delà de 5 s. La soirée longue (E15) a ses coupures, rechargements, arrière-plans et sommeils.
- **Les fichiers** : les fichiers qu'apporte un bot de l'écran arrivent au jeu (`Composant::Fichiers`), et `Image`, `Lire`, `Rendu`, `Telecharger` sont servis avec de vrais contenus. Une image factice revient `Illisible` ; `jeu::fichiers::png_uni` écrit un vrai PNG pour un bot.
- **E13** tire ses gestes abîmés sorte par sorte (par le premier octet, le variant), et non plus parmi tous les gestes vus : un geste rare parmi ce qu'envoient les bots y a sa chance.

Ajouts sans rupture : `jeu::PERIODE_HYPOTHESE` (et `jeu_abi::PERIODE_HYPOTHESE`, dans le prélude), la période où E11 lit l'hypothèse ; `jeu::fichiers::png_uni` ; `partie::Partie::numero_tenu`. Ruptures dans la crate `epreuve` : `Plan` a un champ `monde: Monde` (`Monde::REEL` dans `Plan::standard` et `Plan::court`), `Rapport` un champ `monde: MondeJoue`, `Panne` une variante `ArrierePlan { duree }` (un `match` exhaustif doit la nommer), et le JSON du rapport un objet `monde`.

Pour s'adapter : relancer l'épreuve ; lire les refus nouveaux de ses bots ; remplacer tout jugement de ce qu'un joueur a vu par `quand.vu` ; Tier list, faire apporter de vrais PNG à son bot ; Sidequest, vérifier `jeu::PERIODE_HYPOTHESE` à la compilation.

## v0.4.0 — 2026-11-02

Un téléphone revient mieux : recharger, rejouer dans un onglet, dormir ou traverser un redémarrage de l'hôte ne perd plus ce qu'il a envoyé, et le protocole passe à 2. Un jeu peut choisir le stick natif de l'app iPhone et relier ses tests aux exigences communes, et sa CI ne part plus à chaque commit. `soiree certify` devient plus strict : une page qui écrit `console.error`, ou un `advance` sans `advance.progress`, le font échouer. `soiree check` aussi : une crate dont aucun test ne tourne, un `cert.json` qui retire `socket` ou `phone` sans le dire, une raison vide de sens ou une commande qui n'observe rien ne passent plus.

### Passer à cette version

Le code d'un jeu de la v0.3.0 compile sans changement : aucune API publique n'a disparu. Ce qui suit concerne son comportement et sa certification.

1. Installe l'outil de cette version, puis, à la racine du jeu, regarde ce qui va changer et applique-le :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.0 soiree
   soiree upgrade --dry-run
   soiree upgrade
   ```
   `soiree upgrade` change l'étiquette dans `Cargo.toml` et les crates du jeu, la version dans `soiree.toml`, les règles, `.github/workflows/soiree.yml` et `.soiree/`, met à jour les dépendances communes, puis lance `soiree check`. Ce `soiree check` peut encore échouer sur ce que les étapes suivantes corrigent : continue, et l'étape 13 le relance. `soiree upgrade` regarde seulement la première ligne de `soiree.yml` : il s'arrête avant toute écriture si l'en-tête `# @generated by soiree:` a disparu, et sinon il réécrit le fichier. Une modification faite à la main sous cet en-tête disparaît donc : renomme d'abord ta version. `soiree check` compare tout le fichier et refuse un `soiree.yml` modifié. Rien d'autre à faire pour cette CI : elle ne part plus qu'à la main (`gh workflow run soiree.yml`) ou sur une pull request. `soiree check` ne lit pas les autres fichiers de `.github/workflows/` : si l'un d'eux part à chaque commit, retire ce déclencheur toi-même. `soiree upgrade` ne touche pas à `fuzz/`, qui est un espace de travail à part : si `fuzz/Cargo.lock` existe, lance ensuite `(cd fuzz && cargo update --workspace)`, sinon il reste sur l'ancienne étiquette.
2. Si `cert.json` déclare `advance`, ajoute `advance.progress`, une expression JavaScript dont la valeur change quand la partie avance. Sans elle, `certify` échoue, alors qu'il ne faisait que le noter :
   ```json
   "advance": { "key": " " }
   "advance": { "key": " ", "progress": "document.querySelector('#manche')?.textContent" }
   ```
   Remplace `#manche` par ce qui, sur ta TV, change à chaque étape.
3. Cherche `console.error` et `console.assert` dans les pages du jeu. Pendant un scénario, chacun fait désormais échouer `certify`. Corrige ce qu'ils signalent, ou écris en `console.warn` ce qui n'est pas une erreur.
4. Dans `cert.json`, déclare ce que la certification exerce maintenant. `soiree upgrade` a écrit les exemples du modèle dans `.soiree/modele/tools/`, et `.soiree/CERTIFICATION.md` décrit chaque clé. Une commande reçoit un hôte frais par `{url}` et le navigateur du harnais par `SOIREE_CERT_BROWSER_MODULE`. Éprouve chacune avec `soiree certify -- --only text,progression,layout`.
   - `socket` : obligatoire dès que `phone` est déclaré, en général `"socket": "/salon/ws"`. Un jeu sans téléphone écrit `"phone": null`, `"socket": null` et `display` (voir plus bas) : retirer ces clés n'exclut plus aucun item.
   - `players.max` : obligatoire dès que `phone` est déclaré, par exemple `"players": {"min": 2, "max": 8}`. `soiree check` ne le demande pas, mais `capacity` échoue sans lui, alors qu'il sautait R3.
   - `refused` : le sélecteur et le texte exacts de ce que voit le joueur en trop, par exemple `"refused": {"selector": "#status", "text": "La partie est pleine."}`.
   - `text.checks` : copie l'exemple, puis remplace ses sélecteurs et ses textes par ceux de ton jeu.
     ```sh
     mkdir -p tools
     cp .soiree/modele/tools/text-check.mjs tools/text-check.mjs
     ```
     ```json
     "text": {"checks": [{
       "name": "chaque source de texte du jeu, cadres réels et négatifs",
       "command": ["node", "tools/text-check.mjs", "{url}"],
       "timeout_ms": 120000,
       "success": "no free-text entry"
     }]}
     ```
     `success` est une expression régulière que la sortie doit contenir : remplace-la par ce que ton script imprime quand il a fini.
   - Un jeu qui a une fin : copie l'exemple, puis change dans ce fichier `FINISHED`, vraie seulement à l'écran de fin, et `RUNNING_AGAIN`, vraie quand la partie d'après tourne. Si ton jeu ne repart pas par `advance` puis `start`, change aussi `restart`. Le script exige `advance.progress` (étape 2) et lit `join`, `joined`, `start` et `players` dans `cert.json`.
     ```sh
     cp .soiree/modele/tools/progression-exemple.mjs tools/progression-check.mjs
     ```
     ```json
     "progression": {"kind": "finite", "checks": [{
       "name": "une vraie partie va jusqu'à sa fin, puis une nouvelle reprend",
       "command": ["node", "tools/progression-check.mjs", "{url}"],
       "timeout_ms": 90000,
       "success": "D2 : fin réelle"
     }]}
     ```
   - Un jeu sans fin, sans `start` ni `advance`, comme un compteur ou un éditeur, écrit `"progression": {"kind": "continuous", "reason": "Compteur partagé : ni manche, ni départ, ni fin."}`. Un éditeur desktop solo écrit aussi `"display": {"role": "desktop", "reason": "Éditeur solo au clavier et à la souris, sans téléphone."}` ; ses cadres T2 restent requis.
   - Une TV dessinée sur un canvas déclare `"layout": {"name": "…", "command": ["node", "tools/layout-check.mjs", "{url}"], "timeout_ms": 120000}`, un script à toi qui lit les cadres vraiment dessinés et échoue quand l'un sort de la zone sûre. Une TV en DOM n'a rien à déclarer.
   - Pour une sauvegarde IndexedDB d'un jeu sans téléphone, `"storage": {"kind": "indexeddb", "database": "…", "store": "…", "key": "…"}`. Le harnais refuse cette clé dès que `phone` est déclaré.
   - Une raison tient maintenant en au moins deux mots et 12 caractères : `reason` de `catalogue_decisions`, `ui.not_applicable`, `progression.reason` et `display.reason`.
   - Une commande de `ui.checks`, `text.checks`, `progression.checks` ou `layout` contient `{url}` ou déclare `success`. Sans l'un ni l'autre, elle n'observe rien du jeu : `soiree check` et le harnais la refusent, y compris une commande de `ui.checks` qui passait en v0.3.0. Avec `{url}` seul, le code de sortie est toute la preuve : déclare aussi `success`.
5. Relis comment le jeu reçoit un événement : ce qu'une page a envoyé pendant un redémarrage de l'hôte arrive maintenant à la nouvelle partie, après `DeviceEvent::Arrived` et avec la phase de son envoi. Refuse-le comme tout événement périmé :
   ```rust
   DeviceEvent::Event { device, event, phase, .. } => {
       if phase != host.phase() {
           return; // périmé : envoyé pour une autre manche
       }
       self.jouer(device, event);
   }
   ```
   Des phases qui repartent de zéro à chaque lancement peuvent tomber juste : un jeu qui garde son état d'un lancement à l'autre doit reconnaître lui-même une action déjà comptée, car un événement en vol quand l'hôte est mort peut atteindre les deux lancements.
6. Sur la page, ne renvoie rien à la main après `Happening::Reset` ni après `connection.resume()` : salon envoie déjà ce qui attendait, une fois. `Happening::Reset` reste l'endroit où oublier l'écran et attendre le nouveau.
7. Si le jeu comptait sur `host.forget(device)` pour tenir un téléphone dehors, ajoute `host.close()` : un jeton oublié ne nomme plus personne, et le téléphone revient en appareil neuf dans une salle ouverte.
   ```rust
   host.forget(device);
   host.close(); // personne de nouveau n'entre, ce téléphone non plus
   ```
8. Ajoute à `.gitignore` ces quatre lignes, réécrites à chaque `soiree check`, puis retire-les du dépôt avec `git rm --cached --ignore-unmatch certification/check.json certification/check.md certification/check-fast.json certification/check-fast.md` :
   ```text
   /certification/check.json
   /certification/check.md
   /certification/check-fast.json
   /certification/check-fast.md
   ```
9. Facultatif : relie à tes tests celles des exigences C6, E5, T1, D1, D3, D4 et D5 que le jeu tient, avec `[[bindings]]` dans `verification.toml` (`VERIFICATION.md`, « Relier une exigence commune »), puis `soiree contracts --write-view`. Le stick natif de l'app passe par `Game::remote`, `Game::on_app` et `Game::feedback`, qui ont tous une valeur par défaut (`.soiree/salon/README.md`).
10. Dans les passages ciblés, prévois `resources` (R4) : son succès ne remplace pas les trente minutes d'endurance. `rtc` exige un vrai trajet UDP et éprouve les moteurs disponibles, les négociations retardées ou défaillantes et le repli dans son domaine prévu.
11. Si `soiree check` écrit `rien n'a tourné dans …`, cette crate a des tests et tous sont `#[ignore]` : retire l'attribut d'au moins l'un d'eux. Si le workspace n'a aucun test, écris-en un. Un test ignoré parmi d'autres ne fait rien échouer : le résumé devient `✓ Aucun échec, N point(s) à juger` et le nomme avec sa raison. Lis chaque point, corrige ce qui est un défaut et montre le reste à l'auteur avant de dire que c'est fini.
12. Si ta page s'affiche dans l'app iPhone, vérifie trois choses (`remote/CONTRAT.md`, « Qui atteint l'app ») ; aucun jeu du dossier n'est concerné au moment d'écrire :
    - un iframe n'atteint plus `webkit.messageHandlers.telecommande`, même de ton origine : passe ces messages par le cadre principal ;
    - un lien, `location` ou une redirection vers une autre origine sont annulés, schéma et port compris : envoie dans `Remote::Web` l'adresse finale, par exemple celle qui suit une redirection vers `https` ;
    - `navigator.mediaDevices` n'existe pas sur une adresse locale en `http`, ce qui était déjà vrai, et ailleurs WebKit pose maintenant sa question : teste `navigator.mediaDevices` avant de t'en servir.
    Hors de salon, un serveur `/remote` doit de plus continuer à lire son socket : l'app le lâche quand un Ping reste sans Pong.
13. `soiree check`, puis `soiree certify`. Pousse le jeu seulement après l'étiquette du commun : un jeu qui dépend d'un salon non publié ne compile pas chez un ami.

Une page restée ouverte pendant la mise à jour est refusée (`Refusal::Version`) : les joueurs rechargent, il n'y a rien à coder.

Le flux fiable ne demande aucun changement de code non plus, mais quatre choses se voient dans un jeu qui envoie de gros messages :
- `Host::show` ne remplace plus un état déjà parti en plusieurs paquets : il finit, puis le dernier montré le remplace. Une page peut donc afficher un état de plus avant le dernier. Avec un état qui tient dans un paquet, rien ne change.
- `Host::send` et `Client::send` rendent `Full` dès que 1 Mio d'événements attend, et plus seulement à `max_pending` messages. Traite `Full` comme avant. Un seul événement passe toujours, quelle que soit sa taille permise.
- `Limits::max_up` et `max_down` sont ramenés à 962 560 octets quand salon les reçoit : au-delà, `send` rend `TooBig` au lieu de rendre `Ok` et de bloquer la file.
- Après son `Welcome`, un `Client` prend le `max_up` et le `max_down` de l'hôte, quels que soient ceux de son `ClientConfig` : `Client::send` rend `TooBig` au-delà du `max_up` de l'hôte.

### Change de sens

- Le lien le plus récent gagne : un hello resté coincé sur une tentative abandonnée, ou celui de la page d'avant un rechargement, remplaçait le lien vivant. Il perd désormais, quel que soit l'ordre d'arrivée.
- L'époque est tirée au sort à chaque départ. Comptée depuis `salon.json`, elle retombait à 1 quand le fichier manquait, et la page perdait ses envois sans erreur.
- Une connexion `/remote` est un seul joueur, avec son budget. Une deuxième identité sur un lien n'est pas lue.
- Une absente évincée revient en `AppEvent::Back`, tant qu'elle est parmi les 256 dernières à avoir perdu sa place.
- `AppEvent::Input` et `AppEvent::Draw` n'arrivent que pendant un `Remote::Pad`. Ceux qui croisent un changement de télécommande sont jetés, et -128 devient -127.
- Une app refusée voit sa connexion fermée.
- Un paquet coûte ce qu'il transporte : au plus 1 635 parts par paquet.
- Les identités n'ont pas de plafond : voir `salon/README.md`, « Les limites ». « Nothing grows without bound » était faux.

- **`Happening::Reset` n'annule plus que ce que l'hôte a dit.** Avant, la file de la page était vidée : une réponse envoyée pendant un redémarrage rendait `Ok`, puis disparaissait sans un mot. Elle part maintenant vers la nouvelle partie, après l'arrivée de l'appareil et avec la phase de son envoi. Un événement en vol quand l'hôte est mort peut atteindre les deux lancements, une fois chacun.
- **« Jouer ici » (`connection.resume()`) rend ce qui attendait dans l'onglet.** Deux onglets montrent un seul jeton, et chacun gardait ses compteurs : après la reprise, l'onglet numérotait sous ce que l'autre avait envoyé, et l'hôte ignorait ses événements en les accusant. L'hôte numérote maintenant chaque client qui prend l'appareil (`Welcome.session`). Ce qui attendait part une fois, après ce que l'autre a envoyé, et ce qui était arrivé ne repart pas. L'hôte s'en souvient pour les huit derniers onglets qui ont envoyé quelque chose : un onglet absent plus longtemps peut répéter ce qu'il avait en vol, sans rien perdre.
- **Une page rechargée est entendue tout de suite.** Elle numérotait ses états depuis 1 alors que l'hôte gardait les numéros de l'ancienne page : chaque état était ignoré et accusé, et `set_state` rendait `Ok`. Le `Welcome` dit où en sont les numéros, et le premier message de la page ne complète plus celui que l'ancienne avait laissé à moitié envoyé, ce qui en faisait un seul de deux. Les paquets changent de forme : `Hello.resume` devient `Option<Resume { epoch, device, session }>` et `Welcome` devient `{ device, epoch, session, up_next, up_seen, state, down_first, max_up, max_down }`. `PROTOCOL` vaut 2. Une page d'un autre protocole est refusée même si son hello ne se lit plus, au lieu de se reconnecter sans fin.
- **`Host::forget` n'a jamais tué un jeton.** Sa documentation disait que le jeton cessait de marcher : c'était faux, et Punchline comme le modèle comptent sur le contraire. Le téléphone qui le remontre arrive en appareil neuf, sauf si la salle est fermée (`Host::close`). Son numéro n'est pas redonné pendant ce lancement ; il peut l'être après un redémarrage, puisque `Host::known` ne l'a plus.
- **Une identité est écrite avant que son `Welcome` parte.** La salle les enregistrait une fois par seconde : un appareil accueilli juste avant un plantage était inconnu du lancement suivant, et son numéro partait à un autre. Cela couvre aussi un processus tué à l'instant d'après, pas une coupure de courant : le fichier n'est pas synchronisé sur le disque.
- **Un lien qui a reçu son dernier mot n'écoute plus rien.** Un hello sur un lien refusé ou remplacé refaisait toute l'entrée : il reprenait l'appareil au lien courant, ou créait un fantôme qui gardait une place sans jamais de `Left`. Aucun appareil ne quitte plus par un lien qui se ferme tout seul : le jeu n'entend rien.
- **La mort de la salle arrête le processus.** Après une panique du jeu ou une attente système en échec, HTTP répondait encore et les téléphones réessayaient sans fin. `salon_host::run` écrit la cause et sort avec le code 101 (`ROOM_DIED`), ce qui ferme toutes les connexions.
- **Se réveiller abandonne une tentative trop vieille.** `Client::wake` laissait courir son budget de 20 s, au-delà des 10 s de C3. Il abandonne maintenant une tentative plus vieille que `Timing::silent`, et le retour du réseau abandonne toute tentative. Une page lit son temps par `SleepClock`, pour que le sommeil d'un iPhone ne cache pas le silence.
- **`certify` juge les notes écrites hors de leur scénario.** Elles disparaissaient quand le couple scénario et item n'était pas un contrôle. Elles rejoignent maintenant leur item, ou la santé du contrôleur si aucun contrôle de cet item n'a été choisi. C'est pourquoi `console.error` et `advance` sans `advance.progress` font échouer le passage : une vérification qui n'a pas pu finir ne vaut pas une réussite.
- **`soiree check` et `soiree certify` finissent par un résumé en clair.** Ce qu'un humain doit juger (`?` dans le rapport du harnais) ne fait plus échouer la commande : T2, C7 et C9 quand la page dit quelque chose, C4 et C5 quand le jeton et l'appareil ont été retrouvés. Cette décision vit dans le noyau prouvé (`passes_run`). Une vérification qui n'a pas pu finir (`~`) fait toujours échouer, avec le statut « pas pu vérifier ». Le harnais échoue maintenant si un retour donne une autre place, si un téléphone reste sur un écran vide après un redémarrage de l'hôte, ou si le stockage rétabli ne rend pas sa place.
- **Une tentative WebRTC a la même fenêtre absolue de 15 s côté page et côté hôte.** La page la détruisait après 3 s, pendant une négociation encore viable. Les erreurs natives déclenchent le repli WebSocket immédiatement. Un résultat SDP tardif ne remplace jamais ce repli, et la progression ne prolonge pas la fenêtre. Le délai de connexion C1 de 30 s et les budgets de rendu et de latence P1 et P2 ne changent pas.
- **Le budget d'image se juge à 1 ms près.** Une image sautée à 60 Hz dure 33,33 ms et Chrome la rapporte à 33,3 ou 33,4 ms : le budget de 33 ms échouait sur l'image qu'il nomme. Ce 1 ms est la résolution de la mesure, pas une marge, et une vraie image lente échoue toujours.
- **`cargo:suite` ne réussit plus quand rien n'a tourné.** Une crate dont tous les tests étaient `#[ignore]` donnait un vert à zéro test exécuté. Si rien ne tourne dans le workspace, ou si une crate a des tests et n'en exécute aucun, le contrôle reste `not-run` et le passage échoue. Un test ignoré parmi d'autres est compté et nommé avec sa raison, sans faire échouer. Une crate sans aucun test, à côté de crates qui en exécutent, ne compte pas comme un trou.
- **`✓ Aucun problème trouvé` veut dire ce qu'il dit.** Une revue donnait ce titre et le code de sortie 0, alors que `README.md` et `REJOINDRE.md` en font la fin du travail. Le titre devient `✓ Aucun échec, N point(s) à juger`, le code de sortie reste 0, et le résumé dit quoi faire de ces points.
- **`cert.json` ne retire plus d'item en silence.** Enlever `socket` rendait E1 à E4 et C7 non applicables, et `"phone": null` dix autres items. `phone` exige maintenant `socket`, et un jeu sans téléphone exige `display.role = "desktop"` avec sa raison. Sinon, ces items restent applicables et le passage échoue.
- **Un jeu n'a plus de CI à chaque commit.** Le workflow du modèle tournait sur macOS à chaque push, sur des dépôts privés qui partagent environ 190 minutes macOS par mois, à 6 à 13 minutes le passage.
- **`soiree upgrade` ne s'arrête plus sur un `Cargo.lock` écrit sous un `[patch]`.** Ce fichier nomme salon sans sa source : `cargo update --package salon-core` refusait, une fois les manifestes déjà réécrits. `upgrade` lance maintenant `cargo update --workspace`, qui rend leur source aux crates communes et garde tout le reste verrouillé.

### Réseau et salon

- `salon_web::audio` ajoute des effets TV bornés, identifiés et périssables, débloqués par un geste réel, avec coupure, volume et libération du graphe. Ses régressions rendent le vrai signal et éprouvent les navigateurs. L'activation et la qualité de la sortie physique restent à contrôler sur les appareils.
- `Game::remote`, `Game::on_app` et `Game::feedback` relaient les télécommandes natives : la porte ne répondait que par la page du téléphone et jetait les entrées, donc aucun jeu ne pouvait choisir `Remote::Pad`. Le pilote tient les apps par joueur et applique le contrat au même endroit : `seq`, silence, débit, table bornée.
- `SALON_PUBLIC_URL` dit où les téléphones joignent l'hôte, sans réseau ou derrière un VPN complet. `join_url` y prend sa base, validée à la frontière, et WebRTC annonce son hôte quand c'est une adresse IP.
- La page retient un échec de WebRTC dans son stockage de session : là où l'UDP est bloqué, chaque chargement ne repaie plus les 15 s. WebRTC est retenté après dix minutes, quand le navigateur annonce `online`, ou dès qu'un lien s'ouvre. Une offre restée sans réponse n'est pas un échec, et là où WebRTC a déjà marché un seul échec ne survit pas au rechargement. La page écoute aussi `resume` : `visibilitychange` ne suit plus toujours un dégel.
- Un nom trop long pour un paquet laissait le téléphone sur `Connecting` sans erreur, aucun hello ne partant. Le client le coupe comme l'hôte, et chaque message de contrôle, au plus grand, tient dans le plus petit paquet.
- Le serveur borne les connexions avant de créer leurs threads : deux connexions par appareil configuré, 32 places HTTP et 32 réservées au loopback, au moins 128 au total. Il applique une échéance absolue aux lectures HTTP et WebSocket et borne les écritures. La fermeture d'un lecteur arrête aussi son writer, même si un destinataire conserve sa file. Les événements et états de jeu ne changent pas de format.
- Sous macOS, la salle attend ses échéances avec une minuterie kqueue critique, et les messages réseau ou la fin du dernier émetteur la réveillent immédiatement. La file reste bornée et FIFO. Aucun réglage global, thread de minuterie ou spin permanent : les erreurs natives restent explicites et les budgets des jeux ne changent pas. Les autres systèmes utilisent leur attente de canal native.
- Les secrets de l'hôte ne passent plus par `/dev/urandom`, dont l'ouverture faisait échouer le démarrage sous Windows : ils viennent de `rand_core`, déjà dans le graphe. Rien de cela n'a été exécuté sous Windows.
- **Une panique d'une dépendance de WebRTC ne coûte plus qu'un lien.** dimpl 0.7.6 panique sur deux enregistrements identiques d'une page sans jeton, et un seul fil servait toutes les pages : la salle entière passait en WebSocket jusqu'au redémarrage de l'hôte, et chaque offre recevait 503. Chaque appel à str0m est gardé par lien, et le fil repart si une panique lui échappe. Il faut le déroulement de pile : un jeu ne doit pas écrire `panic = "abort"` dans ses profils, et l'hôte le dit au lancement s'il le fait.
- **Le repli sur WebSocket part 3 s après la réponse de l'hôte si ICE n'a rien entendu.** Là où l'UDP n'arrive pas, un téléphone neuf et chaque reprise attendaient 15 s. C'est un changement visible : un téléphone sur un réseau où ICE met plus de 3 s à répondre passe en WebSocket, ce qu'il ne faisait pas. Les 15 s absolues restent pour une réponse qui tarde et pour DTLS ou SCTP bloqués après ICE. `RTC_PATIENCE_MS` garde sa valeur. Rien à changer dans un jeu.
- WebRTC annonce d'abord l'adresse à laquelle le téléphone a envoyé son offre, puis celle d'avant, puis la boucle locale, et chaque lien écoute sur la sienne : un Mac à deux interfaces, ou dont le DHCP a changé l'adresse, ne fait plus payer le repli.
- Un paquet de `Caps::DATAGRAM` tient en un datagramme : str0m coupait en deux tout paquet de plus de 1 062 octets, sur un canal où un fragment perdu perd le message.
- Le tampon du canal ne fait plus attendre l'état frais derrière l'état périmé : un lien au-dessus de 4 Kio pendant 100 ms saute ses écritures, sauf une toutes les 100 ms, sur l'hôte comme sur la page. Le type est `salon_core::transport::Backlog` ; un jeu n'a pas à s'en servir.
- Une offre refusée (503, 400) n'est plus comptée comme un échec de WebRTC : la page rechargée essaie de nouveau WebRTC.
- Au plus 64 liens négocient à la fois, le plus ancien part. Une boucle d'offres ne ralentit plus les liens ouverts.
- `SALON_PUBLIC_URL` refuse une adresse IPv6 et le dit, au lieu d'un QR code qui ne mène nulle part : salon n'écoute qu'en IPv4. Passe par une adresse IPv4 ou un nom.
- **Un message plus gros que ce que le tuyau passe arrive.** Chaque renvoi repartait du fragment 0 et l'accusé ne disait rien d'un message incomplet : un tuyau qui garde le début d'une rafale ne passait jamais la fin, et tout ce qui suivait dans la file attendait pour toujours, sans erreur. L'accusé nomme maintenant les fragments arrivés, et un renvoi ne porte que les autres. Sur le banc, 60 000 octets à 1 Mb/s et 32 Kio de tampon : 0 message en 120 s avant, 941 kb/s utiles après. Cinq messages à 100 kb/s et 128 Kio : 2 sur 5 en 120 s avant, 5 sur 5 en 31,5 s après.
- **Ce qui part suit ce qui revient.** Une fenêtre part de dix paquets, double à chaque aller-retour et se coupe en deux à chaque perte, et chaque minuterie expirée double l'attente suivante. Sur un lien muet, un état de 60 000 octets coûtait 60 fois sa taille avant `Timing::absent`. Le prix : un gros message sur un lien rapide prend quelques allers-retours de plus. Ces réglages n'ont été éprouvés que sur un tuyau simulé.
- **Ce qui arrive après un trou attend.** Le récepteur garde 64 numéros, et trois accusés qui passent par-dessus un fragment le renvoient sans minuterie. 200 événements à 5 ms de gigue prenaient 10 s sans aucune perte.
- **Aucune minuterie de renvoi sur un WebSocket.** Les copies passaient devant ce qui est frais. Ce que l'hôte jette avant le socket est prouvé perdu par l'ordre du flux.
- **Un WebSocket ne reste plus muet sans erreur.** Si ce qui est parti reste sans nouvelle pendant trois fois `Timing::absent`, soit 18 s, alors que l'autre côté parle encore, le lien est déclaré perdu des deux côtés et la reconnexion reprend. Un jeu le voit comme une coupure, sans rien à changer : `DeviceEvent::Left`, avec `Why::Silent` si l'hôte renonce le premier et `Why::Closed` si c'est la page, puis `Back`. Aucune copie, et aucune reconnexion sur le banc jusqu'à 32 kb/s.
- **Le Welcome dit les limites de la salle** (`max_up`, `max_down`), et un état trop gros est sauté comme un événement au lieu de repartir sans fin.
- Un paquet en retard ne ramène plus la phase en arrière, ce qui faisait refuser une réponse (E5). Un événement envoyé juste avant `quit` arrive. Un `Timing` à l'envers ne fait plus paniquer la salle, et un `heartbeat` ou un `resend_min` à 0 valent 1 ms : `poll` ne répondait plus jamais `None`.
- Les paquets changent de forme : `Ack` porte `ahead`, `event_parts` et `state_parts`, et `Welcome` porte `max_up` et `max_down`. `PROTOCOL` reste à 2 : aucune étiquette n'a publié le protocole 2. Un fragment porte 940 octets sur un lien `Caps::DATAGRAM`, contre 996.

### Ce qu'une soirée laisse en mémoire

`salon_host::simulation` compte désormais ce que garde une salle, sans attendre. `evening` joue six tours sur le vrai hôte et les vrais clients, en temps simulé, et rend les octets vivants après chaque tour ; `flat` dit s'ils se stabilisent. Le compteur est `Counting`, l'allocateur à déclarer dans les tests (`#[global_allocator]`). `Network` gagne `replace`, une nouvelle page, et `disturb`, une coupure ou un silence tirés au dé. Un tour dure 10 min simulées dans `cargo test` et 60 avec `SOIREE_EVENING_MINUTES=60` : une fuite dépend de ce qui arrive, pas de la durée.

- Punchline et Sidequest en ont une, et elle y a trouvé le même défaut : un téléphone parti pour de bon restait en mémoire, dans la salle et dans salon, jusqu'à la fin de la soirée. Corrigé dans les deux. Le jeu ne perd rien : ce téléphone, s'il revient, est un nouveau venu, comme après l'oubli d'un inconnu.
- Sept retraits de code dans salon ont mesuré le test. Les quatre qui faisaient vraiment grandir la mémoire sont vus sur les six heures, grâce à des pages rechargées, de gros messages et un téléphone qui ne perd jamais son lien : une soirée ne voit que ce qu'elle emprunte. Les trois autres ne changent pas la soirée d'un octet : ils gardent des octets plus longtemps sans les accumuler. La version courte de `cargo test` manque les plus petites fuites, d'où les six heures dans la CI de salon et avant une étiquette.
- `--soak 30` reste en option : c'est la seule endurance avec un vrai navigateur et de vrais sockets. La mémoire du serveur s'y lit moins bien : une mesure en processus bouge de quelques Mio toute seule, là où la soirée simulée compte l'octet.

### Certification

- Une salle de moins de 8 joueurs se certifie : les parcours ordinaires ouvrent `players.max` téléphones au plus, et `silent` ne demande plus cinq téléphones.
- `cert.json.storage` est refusé sur un jeu à téléphones, où il ne changeait rien : C9 pouvait passer sans avoir touché la sauvegarde.
- `rtc` perturbe le vrai transport UDP puis renégocie son DataChannel avec la même identité. Il accepte qu'un effet se termine en revenant à sa valeur ou en cessant de bouger : il supposait un saut qui retombe, et échouait toujours sur un compteur comme celui du modèle.
- Le nouveau contrôle R4 observe la rétention DOM, JS et WASM et le RSS de Chromium et du GPU ; l'endurance R2 conserve aussi ces mesures. Les contrôles négatifs éprouvent les détecteurs. Le refus de stockage inclut IndexedDB, et `cert.json.storage` peut identifier la sauvegarde à corrompre.
- La certification complète exerce `capacity` au maximum déclaré, `text` sur un corpus déterministe et `progression` jusqu'à une fin réelle puis une reprise. Les commandes UI, texte et progression ont chacune leur salle. Les champs de 16 px sont contrôlés même avec une TV canvas, dont la géométrie exige un oracle réel.
- La cadence garde le reçu d'une mesure déjà terminée si Node est retardé, sans nouvel échantillon ni acceptation d'un arrêt réel du rendu. L'endurance garde ses séries brutes, un temps monotone, cinq minutes de chauffe effective et aucun thread supplémentaire conservé.
- Le harnais mesure la part occupée du processeur avant de lancer quoi que ce soit et après avoir tout arrêté, garde les deux dans le rapport et prévient dans le résumé à partir de la moitié des cœurs : une compilation à côté faisait de faux échecs sans un mot. Le verdict ne change pas, et une mesure illisible reste inconnue, jamais une machine au repos.
- L'identité d'un passage ne s'invalide plus pour ce qui n'est pas une source : bruit de la machine, notes que Git ne suit pas, dossier de données ignoré. Ils sont nommés dans la provenance, tout fichier suivi par Git reste une entrée, et rien n'est présumé hors de Git. Une vraie source changée pendant un passage le fait toujours échouer, et l'échec nomme les fichiers. Un `Cargo.lock` périmé faisait échouer le premier passage : un passage ordinaire le rafraîchit d'abord, comme Cargo ; un passage verrouillé le refuse toujours et ne réécrit rien.
- `[[bindings]]` relie à ses tests C6, E5, T1, D1, D3, D4 ou D5, qui dépendent des règles de chaque jeu. La validation est stricte : contrôle inconnu ou répété, liste vide, exigence reliée deux fois ou hors des sept sont refusés, et un minimum commun garde ses contrôles. Relier une exigence conditionnelle la rend applicable ; rien ne la rend non applicable. Une liaison ne compte que par l'observation de son test. Le modèle relie E5 et D1, les deux que ses tests portent vraiment.
- Deux tests du harnais supposaient un runner rapide : 250 ms pour atteindre une ressource retenue, 100 ms entre deux images. Une tentative dont le délai a expiré avant sa cause ne conclut plus rien et double ce délai, et le test de cadence vérifie que chaque intervalle vient du reçu du navigateur. Une navigation abandonnée à son délai continue de courir : sa réponse 500 tardive ne se lit plus comme l'échec de la suivante.
- Le contrôleur conserve les pannes TCP de Toxiproxy, mais borne sa connexion amont par un connecteur possédé : deux secondes pour joindre la vraie adresse LAN, sans nouvelle tentative ni repli. Un SYN bloqué ne retient plus la commande de coupure jusqu'au délai du système. Le connecteur borne ses ressources et les libère à l'arrêt ; les erreurs d'administration restent des pannes d'infrastructure.
- `rtc` juge quatre cas de plus : la fermeture 3 s après la réponse quand ICE reste muet, la reprise d'un WebSocket sain sous UDP muet en moins de 5 s, l'échéance absolue de 15 s une fois ICE passé, DTLS seul étant coupé, et une offre refusée qui n'empêche pas WebRTC au rechargement. L'ancien cas de l'échéance absolue après la réponse SDP ne pouvait plus l'atteindre : ICE décide avant. Le fichier `salon/tests/rtc-browser.mjs` joue ces cas sur l'hôte réel de la démo, dans les deux moteurs, sans jeu.
- `rtc` commence chaque cas de négociation avec la mémoire d'un onglet neuf : deux refus laissaient leurs échecs dans `sessionStorage`, et le cas de l'échéance absolue partait sans WebRTC. Un cas à part vérifie qu'un rechargement après un échec ne repaie pas la fenêtre.
- T2 juge un texte au repos : `assertLiteral` attend les animations finies du texte et de ses ancêtres, puisqu'une vue tout juste dessinée peut être à l'opacité 0 de sa première image. Ce qui finit caché échoue toujours.
- `jitter.ready` ne se contente plus d'être lue : si elle reste fausse, le harnais fait avancer la partie par `advance`, et R1 échoue s'il n'y arrive pas. Une panne de `jitter` compte pour R1, plus pour E1.
- `certify` ne dit plus « Ce passage réussit » à côté de `✗ À corriger` quand il reste aussi des points à juger.
- `certify` signale le pare-feu quand l'adresse du réseau ne répond pas. Les tests du harnais navigateur tournent dans un job indépendant des compilations Rust et iPhone, leurs fichiers l'un après l'autre. Le contrôle de navigation conserve l'historique des requêtes : la fermeture d'une connexion au délai ne signifie pas que sa ressource n'a pas été demandée. Ses délais et ses assertions ne changent pas.

### Outil

- `soiree doctor` demande Node 22, et non plus 20 : `hostile` a besoin du `WebSocket` global de Node.
- `soiree contracts --migrate` disparaît : tous les jeux ont `verification.toml`.
- Une raison tient en au moins deux mots et 12 caractères (`catalogue_decisions`, `ui.not_applicable`, `progression.reason`, `display.reason`, `raison_niveau`) : `x` n'excuse plus rien. Une commande de `ui`, `text`, `progression` ou `layout` sans `{url}` ni `success` est refusée : `["true"]` validait U1, T2 et D2. Avec `{url}` seul, le code de sortie reste toute la preuve, et le rapport du passage note cette limite. Le Markdown des rapports écrit les hypothèses à côté du résultat d'une liaison ou d'une garantie du jeu.
- Tout fichier suivi par Git reste une entrée de l'identité, même nommé `._x` ou `x~`. Dans un dépôt Git, seul un `.md` ou un `.txt` non suivi est présumé personnel, s'il est dans un dossier `notes/`, si son nom contient le mot `note` ou `notes`, ou s'il commence par `todo`, `scratch` ou `brouillon` : `footnotes.txt` restait dehors.
- `.soiree/modele/tools/` donne à un jeu les exemples que la mise à jour lui demande de copier, dont `progression-exemple.mjs` pour un jeu fini.
- `soiree check .` lit les documents du commun contre le code : versions, commandes et options, chemins, scénarios, clés de `cert.json` et items. Le catalogue renvoie à `verification.toml`, `CONTRAT.md` dit ce que salon permet vraiment, et seuls les derniers résultats restent dans `certification/` ; les campagnes passées sont dans l'historique git.
- `soiree check` marche sans Node, sauf si `cert.json` déclare `ui.checks[].success`, et il le dit alors clairement. Le harnais l'exige toujours.
- `soiree doctor` vérifie l'éditeur de liens, que rustup n'installe pas, et GitHub CLI, qui reste facultatif.
- Le `.gitignore` du modèle ignore les rapports que `soiree check` réécrit : ils entraient dans le premier commit d'un ami avec des chemins locaux. Les rapports de `soiree certify` restent suivis, avec le code. Le modèle ne provoque plus de fausses erreurs `{{nom}}` dans la sortie de Cargo.
- Sous Windows, `soiree certify` s'arrête avant `npm ci` et dit qu'il n'y est pas encore disponible, sans le faux titre `✗ À corriger : 23 point(s)` : le harnais dépend de `lsof`, de `ps` et des groupes de processus POSIX. L'identité d'un passage trouve `node.exe`, `Path` et `USERPROFILE`, et Toxiproxy n'appelle plus `which`. Rien de cela n'a été exécuté sous Windows.
- Un test `should_panic` se lit : libtest écrit `test nom - should panic ... ok`, un nom que l'analyseur ne reconnaissait pas, ce qui transformait toute la cible en erreur d'infrastructure.

### Recherche mesurable

`recherche/` rend mesurable la méthode du manifeste : une fiche par sous-système, des citations vérifiables, chaque leçon reliée à un test ou à un trou. La fiche pilote, `recherche/reconnexion.toml`, a lu 28 sources et relevé 13 trous dans salon, dont trois graves. `fiabilite`, `transport-webrtc` et `telecommande` en ont relevé 41 de plus, dont sept graves, que cette version ferme presque tous.

- `soiree recherche plan` dit ce que le diff ajoute, le sous-système, le niveau, la fiche, ce qui manque et les étapes à suivre. Il n'empêche rien.
- `soiree recherche verifie` confronte à l'amont chaque commit et chaque citation des fiches. Un lien mort fait échouer la fiche ; une panne de réseau a son propre code et ne conclut rien.
- `soiree recherche dette` liste les sous-systèmes de niveau 3 sans fiche, les fiches sous leur plancher et les trous ouverts. Elle part du plancher de `sous-systemes.toml`, pas du niveau écrit dans la fiche. `soiree check .` juge les fiches hors ligne, et la dette ne bloque qu'à l'étiquette.
- `soiree recherche verifie` échoue s'il n'a aucune fiche à vérifier ou si le dossier n'existe pas. Sans `gh`, le 422 de GitHub pour un commit inconnu est un échec, pas une panne, et une réponse 2xx qui n'est pas ce commit, comme un portail Wi-Fi, ne conclut rien.
- Une fiche doit tenir ses leçons (`lecons` dans `[[niveau]]`), dire dans `consulte` de quoi retrouver sa lecture, et une raison de baisse doit en être une. Un trou ne se ferme que sur un test non ignoré ou un item, jamais sur un fichier ou une ligne de code : `#[cfg(test)]` n'est plus pris pour un test, et `#[test] #[ignore]` sur une ligne est vu.

La dette de niveau 3 restante, ligne par ligne :

- `telecommande` T17, faible : l'app ne déclare ni `UIRequiresPersistentWiFi` ni de type de service réseau. L'effet ne se voit qu'avec de vrais iPhones et une partie de plus de 30 min, et rien ne dit qu'un réglage aveugle ne ferait pas pire : à mesurer d'abord (`REPETITION.md`).
- `transport-webrtc` T9, moyen : Safari iOS n'a jamais été mesuré sur un vrai Wi-Fi de soirée. Il faut un vrai iPhone : le cas est dans `REPETITION.md`.
- `transport-webrtc` T11, moyen : dimpl 0.7.6 garde ses défauts, contenus à un lien par T1. Aucune version publiée de str0m n'embarque dimpl 0.9, et une dépendance `git` sur 46 commits non publiés, avec 32 paquets de plus chez chaque ami, coûterait plus que ce défaut confiné : on attend la publication.
- `reconnexion` T9, faible : `wake` coupe un lien encore valide après 1,5 s de silence. Rien n'est perdu, et le coût visible pour un joueur reste à juger sur de vrais téléphones (`REPETITION.md`).
- `reconnexion` T11, faible : le rattrapage ne se distingue pas du direct. Rien ne montre que cela gêne un jeu, et les jeux peuvent horodater leurs événements.

### Nuit, mutations, tests

- `nuit.yml` lance chaque nuit, sur des machines Linux gratuites, ou à la main, jamais sur un push : les mutations de `salon-core`, dont le rapport n'a pas de seuil, et un fuzzing borné dont le corpus se garde d'une nuit à l'autre. `salon/fuzz/` attaque les paquets et une session hostile : un lien hostile doit laisser connecté un téléphone honnête, et lui devoir exactement ce qui lui était dû.
- Une campagne réelle sur `salon-core` laissait 141 mutants vivants sur 433 : accusés, renvois, renumérotation et échéances n'avaient aucun test pour les voir manquer. La campagne suivante en laisse 11 sur 453. Les nouveaux tests (`client_edges.rs`, `host_edges.rs`, `stream_edges.rs`, `types_edges.rs`) forgent des paquets avec une copie du format, et `found_bugs.rs` garde chaque bug reproduit en les lisant.
- Le chaos joue rechargements, onglets jumeaux, « Jouer ici », pages mortes en silence, redémarrages de l'hôte et paquets retenus : retirer seul un correctif de ces trous le fait rougir, parfois après plusieurs milliers de cas (mesures dans `recherche/reconnexion.toml`). `NOTE_CRITICAL` a son test : le retirer de la minuterie le fait échouer. Un serveur réel dans un processus fils prouve la mort de la salle.

### L'app iPhone

Le projet Xcode déclare les fichiers produits par sa phase de compilation Rust : les liaisons Swift, leur en-tête, leur module et l'archive native. Xcode attend ainsi leur génération avant de compiler Swift, y compris sur un checkout neuf, et la CI le vérifie sans liaisons prégénérées. Aucun `cargo xtask ios` préalable n'est nécessaire.

La coquille Swift a enfin un banc : `remote/bench` compile ses vraies sources, les lance sur un simulateur et les joue contre un faux jeu qui coupe, gèle ou se tait, et contre un vrai salon. `recherche/telecommande.toml` y a mené 17 de ses 18 trous à un test ; T17 reste à mesurer.

- **Une coupure ne perd plus un trait.** Ce qui est tracé pendant une coupure attend le socket suivant, derrière `Hello` et le `Show` du jeu, à 180 trames par seconde au plus pour rester sous le budget de salon. Seule reste perdue une trame déjà remise au noyau quand le lien meurt : la voir demanderait un accusé.
- **Un hôte muet se voit.** L'app envoie un Ping toutes les 2 s et lâche le socket sans Pong, en 4 s au plus. Une tentative vers une adresse muette dure 5 s, plus 60.
- **Une page ne laisse plus un écran vide.** Un chargement raté et un processus de page mort se rechargent, avec le même repli de 1 à 5 s et une notice, et les capteurs et sockets d'une page morte sont rendus.
- **Une seule notice à la fois.** Réseau local refusé, elle nomme le réglage. Un jeu jamais joint n'est plus une « connexion perdue ».
- **Le pouce survit à une coupure.** Une coupure ne lâche plus les doigts, et le même `Show` reçu à nouveau garde la télécommande, son dessin et ses numéros de trait.
- **Les états partent à 60 par seconde.** Sur un écran à 60 Hz, le frein en sautait un sur deux. Un appui plus court qu'un envoi atteint maintenant le jeu.
- **Un retournement de 180° se dit.** La page reçoit `telecommande:orientation`, et le volant ne s'inverse plus.
- **La page, et elle seule.** Le pont et `window.telecommande` restent au cadre principal de l'origine de la page (`remote_core::trust`), et WebKit demande au joueur avant de donner le micro ou la caméra, qu'une page obtenait sans un mot.
- **Le reste.** L'écran se remet en veille en quittant un jeu, et une page relative garde la requête du QR.

## v0.3.0 — 2026-10-28

Les engagements propres au jeu passent de tableaux Markdown à
`verification.toml`. L’outil résout leurs obligations et les contrôles communs,
exécute les contrôleurs, puis évalue des observations rattachées à leurs entrées.
`CERTIFICATION.md` devient une vue des engagements ; les rapports datés restent
séparés. Cette rupture demande une migration explicite des identités de tests :
un nom de fonction trouvé dans le texte ou un ancien rapport ne suffit plus.

Cargo fournit les cibles et libtest les tests réellement découverts et exécutés.
Paquet, genre de cible, cible, nom complet, features, features par défaut et
profil distinguent les identités. Tests ignorés, zéro sélection et compilation
échouée ne satisfont aucun engagement. Le noyau de décision de production est
vérifié avec une version figée de Verus ; les adaptateurs, oracles et décisions
d’applicabilité restent hors de cette preuve.

`check --fast` et `certify -- --only …` rendent leur périmètre explicite.
`certify` échoue si un contrôle demandé est absent, sauté, partiel ou si le
contrôleur tombe en panne ; un simple code de sortie Node réussi ne suffit plus.
Un passage ciblé réussi laisse les autres obligations non attestées. Les items
du catalogue sans décision restent non revus et les gestes sur appareils réels
restent des jugements humains. Les coûts d’itération et de CI restent à mesurer.

La CI conserve les téléchargements Cargo, les compilations et les outils figés,
avec contrôle exact de leurs versions. Le binaire du commun est toujours
reconstruit depuis son checkout. Aucun rapport n’est restauré comme résultat
valide ; le gain de temps de ces caches reste à mesurer.

Les attentes UI attendent désormais la valeur résolue de leur expression : une
promesse qui résout `false` ne franchit pas l’attente. Les expressions régulières
et les délais sont validés avant l’exécution des contrôles UI. Le transport de
test est inscrit dans le périmètre ; un passage `phone_upstream: "loopback"`
laisse les garanties d’accès au réseau local non attestées. Les fichiers servis
sont préparés avant leur empreinte.

`soiree upgrade` met aussi à niveau les dépendances communes déclarées
directement dans les manifestes des membres du workspace. En 0.2.0, seules
celles du manifeste racine étaient alignées ; Sidequest conservait ainsi
`remote-web` et `remote-protocol` à l’ancienne étiquette dans ses membres.
Les commentaires et les dépendances de développement, de construction et
propres à une cible restent préservés. La découverte Cargo fonctionne
avant le premier lockfile et ne le crée ni ne le modifie ; un membre hors
du dossier du jeu ou une étiquette absente arrête la commande avant toute
écriture.

Le lock du projet fuzz de la télécommande et sa cible `phone_session` sont remis en état :
le booléen du corpus est converti vers `Orientation` au bord de l’entrée,
sans changer les octets des corpus.

### Passer à cette version

1. Installer l’outil avec
   `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.3.0 soiree`.
2. À la racine du jeu, lancer `soiree upgrade --dry-run`, puis
   `soiree upgrade`. La commande aligne le manifeste racine, les membres
   du workspace, les règles et la CI sur cette étiquette publiée. Un
   sous-projet indépendant, tel qu’un workspace `fuzz/`, garde son propre
   lockfile : le résoudre aussi après la migration s’il dépend du jeu.
3. Préparer `references.json` : pour chaque référence historique active, donner
   son paquet Cargo, son genre de cible (`lib`, `bin` ou `test`), sa cible,
   son nom complet libtest et sa configuration. Une redirection de helper ou
   de nom doit être expliquée. Le format exact est dans `VERIFICATION.md`.
4. Lancer `soiree contracts --migrate references.json`, relire les engagements,
   hypothèses et décisions importés, puis `soiree contracts --write-view`.
   L’original est conservé dans `CERTIFICATION.legacy.md` ; aucun résultat
   historique n’est importé comme réussi. Un jeu qui possède déjà un contrat
   structuré l’édite directement, sans relancer cette migration.
5. Lancer `soiree check`, les scénarios nécessaires avec `soiree certify`, puis
   le passage `soiree check --locked --published` depuis un clone sans patch
   local. Examiner les contrôles en attente et les limites du rapport avant
   de déclarer le périmètre validé. Les crates communes du SDK gardent leur
   version de paquet 0.2.0 ; l’étiquette Git v0.3.0 fige l’ensemble distribué.

## v0.2.0 — 2026-10-28

Le contrat entre une page libre et l’app iPhone est défini dans
`remote-protocol::web`, réutilisé par le SDK WASM et par les liaisons Swift
UniFFI. Le format JSON public reste identique. Les propriétés CSS écrites par
Rust demandent maintenant une catégorie de valeur typée.

Les pages possèdent leurs ressources navigateur via `salon_web::ui` :
listeners, timers, animations, tickets d’opérations asynchrones, composition
et effets ponctuels. Les formulaires gardent leur identité pendant les
mises à jour qui ne changent pas leur tâche. Les tests exercent les
destructions, les résultats tardifs, les compositions et les décodeurs
ambigus, ainsi que la propagation des échecs des contrôles eux-mêmes.
`soiree new` et `soiree upgrade` installent une CI versionnée ; le mode
`check --published` refuse les dépendances communes non publiées ou locales.
Ce mode exige un jeu versionné (`Cargo.toml` et `soiree.toml`) ; retirer la
version ne permet pas de sauter les règles de CI et les déclarations U1.
Les noms de jeu `salon` et `remote` sont réservés aux crates du commun ;
`--org` choisit le dépôt du jeu sans rediriger ses dépendances communes.

### Passer à cette version

1. Après publication de l’étiquette `v0.2.0`, installer l’outil de cette version
   et aligner les dépendances salon/remote ainsi que `soiree.toml`. Pendant le
   développement chez l’hôte, les jeux gardent leur ancienne étiquette Cargo
   et utilisent le `[patch]` local ; ils ne doivent pas être poussés ainsi.
2. Régénérer chaque module avec `soiree dom <page.rs>`. Pour une propriété
   personnalisée écrite par Rust, déclarer sa catégorie dans sa feuille CSS,
   par exemple `/* soiree-type: --accent color */`. Catégories : `color`,
   `length`, `number`, `time`, `image`. Une propriété sans catégorie n’a pas
   de setter ; les déclarations absentes ou contradictoires échouent.
3. Remplacer les valeurs libres de `prop::X.set` par une référence à
   `salon_web::style::{Color, Length, Number, Time, Image}` :
   `Color::rgb(255, 0, 0)`, `Length::px(x)?`, `Number::new(n)?`,
   `Time::milliseconds(ms)` ou `Image::url(src)?`. Les couleurs hexadécimales
   existantes se valident avec `parse::<Color>()`. Les coordonnées et nombres
   non finis sont refusés. Les noms de propriété viennent du générateur.
4. `remote_web::Info::player` est un `PlayerId` validé : utiliser `.bytes()`
   pour le jeton ou `.to_string()` pour l’affichage. Retirer les décodeurs
   hexadécimaux qui validaient à nouveau cette même identité.
5. Reconstruire l’app avec `cargo xtask ios`. `WebRemote.swift` utilise les
   commandes générées de `remote-ffi`, et les scripts de sortie sont fournis
   par le même contrat Rust. `cargo xtask web-contract` compile et exécute les
   liaisons Swift réelles et vérifie les scripts dans JavaScriptCore ; ce
   contrôle rejoint `soiree check remote` sur macOS. Ailleurs, il est signalé
   comme non exécuté. `cargo xtask web-contract-browser` complète ce contrôle
   avec le vrai SDK WASM, WebKit et Chromium (navigateurs du harnais requis).
   Un build Xcode vérifie le consommateur réel.
6. `soiree check`, puis les vérifications navigateur applicables. Les fichiers
   `.soiree/` et les sorties UniFFI restent générés et ignorés par Git.
7. Conserver chaque `Connection` aussi longtemps que sa page en a besoin :
   sa dernière copie détruite ferme maintenant le transport. Remplacer les
   callbacks oubliés et les timers/boucles manuels par `Listener`,
   `Listeners`, `Timer` et `AnimationLoop` de `salon_web::ui`, gardés dans
   leur propriétaire. Leurs résultats sont `must_use`. Les listeners DOM et
   les callbacks `on_orientation`, `motion`, `Datagram::on_message` de
   `remote-web` prennent `Fn` : remplacer une capture directement mutable
   par un `Cell` ou `RefCell` détenu par le propriétaire. Libérer les emprunts
   avant une opération DOM pouvant déclencher un événement synchrone.
8. Protéger les tâches avec `OperationSlot` (le dernier appel remplace le
   précédent) ou `OperationScope` (plusieurs tâches indépendantes). Appeler
   `finish` juste avant d’appliquer le résultat ; annuler à la fermeture ou
   au remplacement de l’objet logique. Un ticket n’interrompt pas à lui seul
   le travail natif du navigateur. Brancher `Composition` sur les événements
   de composition et bloquer les nouvelles validations/raccourcis ; garder
   les relâchements de commandes tenues. Un effet ponctuel ordonné peut
   utiliser `OnceEffects`, avec identités croissantes et remise à zéro lors
   d’un changement de session faisant autorité.
9. Ajouter `ui` à `cert.json` : scénarios ou commandes réelles couvrant
   `editing`, `lifecycle`, `async`, `effects`, et une justification pour
   chaque catégorie non applicable. Voir `CERTIFICATION.md`. Le modèle et
   les trois jeux de l’hôte en donnent des exemples. Installer la CI avec
   `soiree upgrade` ; `--dry-run` montre les fichiers visés. La commande
   cible la version du binaire installé et refuse une étiquette non publiée
   avant de modifier le jeu ; elle ne réécrit pas automatiquement sa logique.
10. Remplacer `postcard::from_bytes` par `salon_core::codec::decode` pour
    un message dont le cadre contient exactement un objet. Les décodeurs de
    salon et de la télécommande l’utilisent déjà. Les variantes JSON non
    étiquetées ne sont acceptables que si leurs formes sont disjointes,
    vérifiées par des cas négatifs.

### Change de sens

- Une identité doit être exactement seize octets, exprimés par 32 chiffres
  hexadécimaux. Un datagramme encodé en base64 doit être canonique et complet.
- Les objets JavaScript mal typés sont refusés avant les effets natifs ; les
  ports et octets ne sont plus tronqués par les conversions NSNumber.
- Les valeurs CSS d’un jeu passent par des constructeurs validés et des
  propriétés de catégorie correspondante. Les tracés SVG et la syntaxe
  déclarative CSS restent du vocabulaire du navigateur, pas une promesse de
  typage de toute sa grammaire.
- Les octets ajoutés après un message binaire valide sont refusés ; ils
  étaient auparavant ignorés par `postcard::from_bytes`.
- La destruction du dernier `Connection` clôt son transport. Les ressources
  d’une vue cessent leurs rappels avec leur propriétaire.
- Plusieurs abonnements aux capteurs de `remote-web` partagent la source :
  seul le dernier abonnement détruit envoie `motion:false`.

## v0.1.0 — 2026-10-27

La première version : salon, la télécommande iPhone, les règles communes, le harnais de certification, le catalogue d'items par mécanique, l'outil `soiree` et le modèle de jeu.

### Passer à cette version

1. Installe l'outil : `cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.1.0 soiree`.
2. Un jeu neuf : `soiree new <nom>`. Un jeu existant : ajoute `soiree.toml` (`version = "0.1.0"`), remplace les chemins vers salon par `{ git = "https://github.com/build-your-own-game/soiree", tag = "v0.1.0" }`, puis `soiree check`.
3. Les règles se sont durcies depuis `v0.1.0-pre.1`. Recopie dans le jeu les lints de `outil/regles/cargo.toml` (à partir de `wildcard_enum_match_arm`, en retirant `cast_precision_loss = "allow"`) et `max-fn-params-bools = 0` de `outil/regles/clippy.toml` ; `soiree check` dit ce qui manque. Puis `cargo clippy --all-targets` et `cargo clippy --target wasm32-unknown-unknown` disent quoi corriger. Les idiomes attendus :
   - `wildcard_enum_match_arm` : nomme les variants du bras de repli (`A | B(_) => {}`), ou extrais avec `if let`. Un `#[expect]` pour une enum étrangère large, ou pour un `other => panic!` dans un test.
   - `fn_params_excessive_bools` (`max-fn-params-bools = 0`) : une enum à deux variants, ou deux méthodes. Un `#[expect]` quand le bool est la donnée elle-même (une touche enfoncée, un prédicat).
   - `cast_precision_loss` : `f64::from(u32)` ; `Ms::as_f64()` pour le temps de salon ; sinon une seule fonction de conversion avec son `#[expect]`.
   - `arithmetic_side_effects` : `saturating_*` pour le temps et les statistiques, `checked_*` et une erreur pour un identifiant ou une entrée, `wrapping_*` pour un hachage ou une animation, itérateurs ou `checked_sub(1)` pour les index, un `NonZero*` comme diviseur. Une crate de rendu, et les tests, peuvent s'en exempter en tête de crate ou de module : `#![expect(clippy::arithmetic_side_effects, reason = "…")]`.
   - `let_underscore_untyped` et `unused_result_ok` : `let _: Result<(), JsValue> = element.focus();`, jamais `.ok();`.
4. Depuis `v0.1.0-pre.1`, pour suivre ces règles, quelques API de salon et de la télécommande ont changé :
   - salon-core : `Host::set_open(b)` devient `Host::open()` ou `Host::close()`, et `Host::local_present(now, device, b)` devient `local_plugged(now, device)` ou `local_unplugged(now, device)` ;
   - salon-host : `Clock::at` renvoie un `Option<Instant>` (`None` au-delà de ce qu'`Instant` peut tenir) ;
   - salon-host : `qr_svg(url, dark, light)`, qui donnait du SVG en texte, devient `qr(url) -> Result<Qr, String>` : les modules du QR code, dans le type `salon_core::Qr`. La page le dessine avec `salon_web::qr(&qr)`, un `<svg>` dont les couleurs viennent du CSS du jeu (`fill` sur `path`, `background` sur `svg`) ;
   - remote-core : `landscape_left: bool` devient `Orientation::LandscapeLeft` ou `Orientation::LandscapeRight` dans `Wheel::update`, `Pointer::motion` et `remote::Motion`.
   - remote-core : `Pressed::set(id, true)` devient `Pressed::press(id)`, et `Pressed::set(id, false)` devient `Pressed::release(id)`.

   Nouveau : `Ms::as_f64()`, pour passer un temps de salon à une horloge de page ou à une statistique.
5. Le manifeste a un principe 8, « Aucune chaîne n'est un contrat », et `soiree check` attend son point dans « Écarts au manifeste » de `LEARNINGS.md`. Pour le tenir :
   - **La page.** `clippy.toml` interdit maintenant les méthodes à chaînes de `web_sys` (`get_element_by_id`, `set_attribute`, `class_list`, `set_inner_html`, `style().set_property`…). Pour chaque crate qui touche une page, génère son module de noms, puis déclare-le avec `pub mod page;` à la racine du crate :

     ```sh
     soiree dom crates/web/src/page.rs --html ../../../web/index.html --css ../../../web/style.css
     ```

     Les chemins sont relatifs au fichier généré. Ce que seul le Rust crée se déclare avec `--id <id> <balise>`, `--data <nom>`, `--attr <nom>`, `--class <nom>` ou `--tag <balise>`.
   - **Le code.** `document.get_element_by_id("title")` devient `page::id::TITLE.get()?`, qui renvoie directement le bon type. `el.class_list().add_1("tier-row")` devient `page::class::TIER_ROW.add_to(&el)?`. `el.set_attribute("data-row", &id)` devient `page::data::ROW.set(&el, &id)?`, et la lecture `page::data::ROW.get::<RowId>(&el)`. Une valeur de `data-*` qui aiguille (`data-action`) devient une enum avec `Display` et `FromStr`, et un test relit le HTML pour vérifier ses valeurs.
   - **Le suivi.** Après une modification du HTML ou du CSS, `soiree dom crates/web/src/page.rs`. `soiree check` échoue si un module n'est plus à jour.
   - **La version du jeu.** Copie `.soiree/salon/version.rs` (écrit par `soiree check`) comme `build.rs` du crate qui définit `Up`/`Down`, puis remplace la constante écrite à la main par `pub const VERSION: u32 = include!(concat!(env!("OUT_DIR"), "/version.rs"));`.
   - **Les routes.** `salon_core::route::{OFFER, TV_KEY, SOCKET}` remplacent les chemins `"/salon/…"` écrits en littéral.

6. Le modèle est maintenant jouable : `soiree new` donne une simulation sans écran, un hôte et les pages TV/téléphone. Pour un jeu existant, garde sa mécanique et prends les intégrations de `salon/README.md` ; ne remplace pas son code par le modèle.
7. Enregistre les entrées là où elles entrent dans la simulation avec `salon_host::journal::Recorder`, un en-tête typé, puis `record` et `checkpoint` de l’état complet (`salon_core::journal::fingerprint`). Une opération qui tire du hasard, même avant d’appliquer une action de bot, entre aussi dans le journal. Une fin naturelle appelle `finish` ; une nouvelle partie ouvre un nouveau journal avec son état initial exact. Un éditeur solo utilise `salon_web::journal::Recorder` et le récepteur typé `salon_host::journal::browser_sink`. Les journaux sont locaux, ignorés par Git et purgés après trente jours. Expose une commande de replay qui compare l’état, et conserve une fixture fictive versionnée.
8. `Connection::open` installe le filet anti-panique ; une page solo appelle `salon_web::install_tv_diagnostics`. Retire les hooks locaux qui le remplaceraient. `Game::diagnostic` reçoit les paniques et les marqueurs F8 : écris-les dans le journal de partie. La récupération s’arrête après trois recharges rapprochées, et si son compteur ne peut pas être mémorisé.
9. Pour tester l’hôte entier, utilise `salon_host::driver::Driver` et `salon_host::simulation::Network`, avec horloge et réseau à graine. La vraie salle utilise ce même driver. La campagne normale garde des milliers de graines ; `SOIREE_SIM_SEEDS` étend celle des jeux de l’hôte.
10. `soiree certify` ouvre quatre téléphones WebKit et quatre Chromium. `soiree doctor` vérifie le WebKit de sa version de Playwright ; installe-le avec la commande indiquée s’il manque. Ajoute les sondes causales `performance.probes` décrites dans `CERTIFICATION.md` : une entrée réelle, puis son effet sur la TV. Un canvas publie sa projection après un rendu réussi. Les chiffres sont écrits dans `certification/performance.json` et le rapport, et les dépassements font échouer la certification. La suspension DevTools reste vérifiée sur Chromium, les autres scénarios concernent les deux moteurs.
11. `soiree mutants` lance à la demande les mutations sur `crates/core`, avec `--in-diff` et des exclusions motivées. Rien ne l’appelle depuis `check` ou `certify`. Son rapport indique le périmètre et l’empreinte des sources : un petit premier passage ne prouve pas toute la simulation.

### Change de sens

- Les replays ne sont pas un bénéfice automatique du déterminisme : les entrées et tous les tirages aléatoires doivent réellement être enregistrés dans l’ordre de simulation. Le numéro calculé depuis les sources informe le diagnostic ; le format de replay contrôle sa lecture, et l’empreinte de l’état détecte une dérive de sens.
- Une certification mesure désormais des budgets et couvre WebKit. Un ancien passage Chromium seul ou sans sonde de rendu ne satisfait plus ce plancher.
- Une panique de page provoque une récupération visible et bornée. Elle ne reste plus silencieuse et ne déclenche jamais une boucle de rechargement illimitée.
- La liberté de casser les frontières internes s’accompagne d’un test et d’une notice de migration. Le contrat de l’app iPhone et les contraintes du monde réel restent à coordonner et à éprouver explicitement.
