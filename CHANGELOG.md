# Changements

Chaque version a sa section, de la plus récente à la plus ancienne. « Passer à cette version » s'adresse à l'agent d'un jeu (voir `AGENTS.md`).

## v0.4.0 — 2026-11-02

Un téléphone revient mieux : recharger, rejouer dans un onglet, dormir ou traverser un redémarrage de l'hôte ne perd plus ce qu'il a envoyé, et le protocole passe à 2. Un jeu peut choisir le stick natif de l'app iPhone et relier ses tests aux exigences communes, et sa CI ne part plus à chaque commit. `soiree certify` devient plus strict : une page qui écrit `console.error`, ou un `advance` sans `advance.progress`, le font échouer.

### Passer à cette version

Le code d'un jeu de la v0.3.0 compile sans changement : aucune API publique n'a disparu. Ce qui suit concerne son comportement et sa certification.

1. Installe l'outil de cette version, puis, à la racine du jeu, regarde ce qui va changer et applique-le :
   ```sh
   cargo install --locked --git https://github.com/build-your-own-game/soiree --tag v0.4.0 soiree
   soiree upgrade --dry-run
   soiree upgrade
   ```
   `soiree upgrade` change l'étiquette dans `Cargo.toml` et les crates du jeu, la version dans `soiree.toml`, les règles et `.github/workflows/soiree.yml`, puis lance `soiree check`. Rien d'autre à faire pour la CI : `soiree check` refuse tout autre workflow. Elle ne part plus qu'à la main (`gh workflow run soiree.yml`) ou sur une pull request.
2. Si `cert.json` déclare `advance`, ajoute `advance.progress`, une expression JavaScript dont la valeur change quand la partie avance. Sans elle, `certify` échoue, alors qu'il ne faisait que le noter :
   ```json
   "advance": { "key": " " }
   "advance": { "key": " ", "progress": "document.querySelector('#manche')?.textContent" }
   ```
   Remplace `#manche` par ce qui, sur ta TV, change à chaque étape.
3. Cherche `console.error` et `console.assert` dans les pages du jeu. Pendant un scénario, chacun fait désormais échouer `certify`. Corrige ce qu'ils signalent, ou écris en `console.warn` ce qui n'est pas une erreur.
4. Dans `cert.json`, déclare ce que la certification exerce maintenant :
   - `refused: {"selector":"…","text":"…"}`, `text.checks` et, pour un jeu fini, `progression: {"kind":"finite","checks":[…]}`. Les commandes fonctionnent sur un hôte frais via `{url}` ; le modèle fournit les fichiers et les variables d'environnement.
   - Un éditeur ou un compteur continu, sans `start` ni `advance`, motive `progression.kind="continuous"`. Un éditeur desktop solo peut motiver `display.role="desktop"` ; ses cadres T2 restent requis. Une TV canvas déclare une commande `layout` qui observe ses cadres réels.
   - Pour une sauvegarde IndexedDB, `"storage": {"kind": "indexeddb", "database": "…", "store": "…", "key": "…"}` (syntaxe JSON, pas TOML).
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
11. `soiree check`, puis `soiree certify`. Pousse le jeu seulement après l'étiquette du commun : un jeu qui dépend d'un salon non publié ne compile pas chez un ami.

Une page restée ouverte pendant la mise à jour est refusée (`Refusal::Version`) : les joueurs rechargent, il n'y a rien à coder.

### Change de sens

- **`Happening::Reset` n'annule plus que ce que l'hôte a dit.** Avant, la file de la page était vidée : une réponse envoyée pendant un redémarrage rendait `Ok`, puis disparaissait sans un mot. Elle part maintenant vers la nouvelle partie, après l'arrivée de l'appareil et avec la phase de son envoi. Un événement en vol quand l'hôte est mort peut atteindre les deux lancements, une fois chacun.
- **« Jouer ici » (`connection.resume()`) rend ce qui attendait dans l'onglet.** Deux onglets montrent un seul jeton, et chacun gardait ses compteurs : après la reprise, l'onglet numérotait sous ce que l'autre avait envoyé, et l'hôte ignorait ses événements en les accusant. L'hôte numérote maintenant chaque client qui prend l'appareil (`Welcome.session`). Ce qui attendait part une fois, après ce que l'autre a envoyé, et ce qui était arrivé ne repart pas. L'hôte s'en souvient pour les huit derniers onglets qui ont envoyé quelque chose : un onglet absent plus longtemps peut répéter ce qu'il avait en vol, sans rien perdre.
- **Une page rechargée est entendue tout de suite.** Elle numérotait ses états depuis 1 alors que l'hôte gardait les numéros de l'ancienne page : chaque état était ignoré et accusé, et `set_state` rendait `Ok`. Le `Welcome` dit où en sont les numéros, et le premier message de la page ne complète plus celui que l'ancienne avait laissé à moitié envoyé, ce qui en faisait un seul de deux. Les paquets changent de forme : `Hello.resume` devient `Option<Resume { epoch, device, session }>` et `Welcome` devient `{ device, epoch, session, up_next, up_seen, state, down_first }`. `PROTOCOL` vaut 2. Une page d'un autre protocole est refusée même si son hello ne se lit plus, au lieu de se reconnecter sans fin.
- **`Host::forget` n'a jamais tué un jeton.** Sa documentation disait que le jeton cessait de marcher : c'était faux, et Punchline comme le modèle comptent sur le contraire. Le téléphone qui le remontre arrive en appareil neuf, sauf si la salle est fermée (`Host::close`). Son numéro n'est pas redonné pendant ce lancement ; il peut l'être après un redémarrage, puisque `Host::known` ne l'a plus.
- **Une identité est écrite avant que son `Welcome` parte.** La salle les enregistrait une fois par seconde : un appareil accueilli juste avant un plantage était inconnu du lancement suivant, et son numéro partait à un autre. Cela couvre aussi un processus tué à l'instant d'après, pas une coupure de courant : le fichier n'est pas synchronisé sur le disque.
- **Un lien qui a reçu son dernier mot n'écoute plus rien.** Un hello sur un lien refusé ou remplacé refaisait toute l'entrée : il reprenait l'appareil au lien courant, ou créait un fantôme qui gardait une place sans jamais de `Left`. Aucun appareil ne quitte plus par un lien qui se ferme tout seul : le jeu n'entend rien.
- **La mort de la salle arrête le processus.** Après une panique du jeu ou une attente système en échec, HTTP répondait encore et les téléphones réessayaient sans fin. `salon_host::run` écrit la cause et sort avec le code 101 (`ROOM_DIED`), ce qui ferme toutes les connexions.
- **Se réveiller abandonne une tentative trop vieille.** `Client::wake` laissait courir son budget de 20 s, au-delà des 10 s de C3. Il abandonne maintenant une tentative plus vieille que `Timing::silent`, et le retour du réseau abandonne toute tentative. Une page lit son temps par `SleepClock`, pour que le sommeil d'un iPhone ne cache pas le silence.
- **`certify` juge les notes écrites hors de leur scénario.** Elles disparaissaient quand le couple scénario et item n'était pas un contrôle. Elles rejoignent maintenant leur item, ou la santé du contrôleur si aucun contrôle de cet item n'a été choisi. C'est pourquoi `console.error` et `advance` sans `advance.progress` font échouer le passage : une vérification qui n'a pas pu finir ne vaut pas une réussite.
- **`soiree check` et `soiree certify` finissent par un résumé en clair.** Ce qu'un humain doit juger (`?` dans le rapport du harnais) ne fait plus échouer la commande : T2, C7 et C9 quand la page dit quelque chose, C4 et C5 quand le jeton et l'appareil ont été retrouvés. Cette décision vit dans le noyau prouvé (`passes_run`). Une vérification qui n'a pas pu finir (`~`) fait toujours échouer, avec le statut « pas pu vérifier ». Le harnais échoue maintenant si un retour donne une autre place, si un téléphone reste sur un écran vide après un redémarrage de l'hôte, ou si le stockage rétabli ne rend pas sa place.
- **Une tentative WebRTC a la même fenêtre absolue de 15 s côté page et côté hôte.** La page la détruisait après 3 s, pendant une négociation encore viable. Les erreurs natives déclenchent le repli WebSocket immédiatement. Un résultat SDP tardif ne remplace jamais ce repli, et la progression ne prolonge pas la fenêtre. Le délai de connexion C1 de 30 s et les budgets de rendu et de latence P1 et P2 ne changent pas.
- **Le budget d'image se juge à 1 ms près.** Une image sautée à 60 Hz dure 33,33 ms et Chrome la rapporte à 33,3 ou 33,4 ms : le budget de 33 ms échouait sur l'image qu'il nomme. Ce 1 ms est la résolution de la mesure, pas une marge, et une vraie image lente échoue toujours.
- **Un jeu n'a plus de CI à chaque commit.** Le workflow du modèle tournait sur macOS à chaque push, sur des dépôts privés qui partagent environ 190 minutes macOS par mois, à 6 à 13 minutes le passage.

### Réseau et salon

- `salon_web::audio` ajoute des effets TV bornés, identifiés et périssables, débloqués par un geste réel, avec coupure, volume et libération du graphe. Ses régressions rendent le vrai signal et éprouvent les navigateurs. L'activation et la qualité de la sortie physique restent à contrôler sur les appareils.
- `Game::remote`, `Game::on_app` et `Game::feedback` relaient les télécommandes natives : la porte ne répondait que par la page du téléphone et jetait les entrées, donc aucun jeu ne pouvait choisir `Remote::Pad`. Le pilote tient les apps par joueur et applique le contrat au même endroit : `seq`, silence, débit, table bornée.
- `SALON_PUBLIC_URL` dit où les téléphones joignent l'hôte, sans réseau ou derrière un VPN complet. `join_url` y prend sa base, validée à la frontière, et WebRTC annonce son hôte quand c'est une adresse IP.
- La page retient un échec de WebRTC dans son stockage de session : là où l'UDP est bloqué, chaque chargement ne repaie plus les 15 s. WebRTC est retenté après dix minutes, quand le navigateur annonce `online`, ou dès qu'un lien s'ouvre. Une offre restée sans réponse n'est pas un échec, et là où WebRTC a déjà marché un seul échec ne survit pas au rechargement. La page écoute aussi `resume` : `visibilitychange` ne suit plus toujours un dégel.
- Un nom trop long pour un paquet laissait le téléphone sur `Connecting` sans erreur, aucun hello ne partant. Le client le coupe comme l'hôte, et chaque message de contrôle, au plus grand, tient dans le plus petit paquet.
- Le serveur borne les connexions avant de créer leurs threads : deux connexions par appareil configuré, 32 places HTTP et 32 réservées au loopback, au moins 128 au total. Il applique une échéance absolue aux lectures HTTP et WebSocket et borne les écritures. La fermeture d'un lecteur arrête aussi son writer, même si un destinataire conserve sa file. Les événements et états de jeu ne changent pas de format.
- Sous macOS, la salle attend ses échéances avec une minuterie kqueue critique, et les messages réseau ou la fin du dernier émetteur la réveillent immédiatement. La file reste bornée et FIFO. Aucun réglage global, thread de minuterie ou spin permanent : les erreurs natives restent explicites et les budgets des jeux ne changent pas. Les autres systèmes utilisent leur attente de canal native.
- Les secrets de l'hôte ne passent plus par `/dev/urandom`, dont l'ouverture faisait échouer le démarrage sous Windows : ils viennent de `rand_core`, déjà dans le graphe. Rien de cela n'a été exécuté sous Windows.

### Certification

- `rtc` perturbe le vrai transport UDP puis renégocie son DataChannel avec la même identité. Il accepte qu'un effet se termine en revenant à sa valeur ou en cessant de bouger : il supposait un saut qui retombe, et échouait toujours sur un compteur comme celui du modèle.
- Le nouveau contrôle R4 observe la rétention DOM, JS et WASM et le RSS de Chromium et du GPU ; l'endurance R2 conserve aussi ces mesures. Les contrôles négatifs éprouvent les détecteurs. Le refus de stockage inclut IndexedDB, et `cert.json.storage` peut identifier la sauvegarde à corrompre.
- La certification complète exerce `capacity` au maximum déclaré, `text` sur un corpus déterministe et `progression` jusqu'à une fin réelle puis une reprise. Les commandes UI, texte et progression ont chacune leur salle. Les champs de 16 px sont contrôlés même avec une TV canvas, dont la géométrie exige un oracle réel.
- La cadence garde le reçu d'une mesure déjà terminée si Node est retardé, sans nouvel échantillon ni acceptation d'un arrêt réel du rendu. L'endurance garde ses séries brutes, un temps monotone, cinq minutes de chauffe effective et aucun thread supplémentaire conservé.
- Le harnais mesure la part occupée du processeur avant de lancer quoi que ce soit et après avoir tout arrêté, garde les deux dans le rapport et prévient dans le résumé à partir de la moitié des cœurs : une compilation à côté faisait de faux échecs sans un mot. Le verdict ne change pas, et une mesure illisible reste inconnue, jamais une machine au repos.
- L'identité d'un passage ne s'invalide plus pour ce qui n'est pas une source : bruit de la machine, notes que Git ne suit pas, dossier de données ignoré. Ils sont nommés dans la provenance, tout fichier suivi par Git reste une entrée, et rien n'est présumé hors de Git. Une vraie source changée pendant un passage le fait toujours échouer, et l'échec nomme les fichiers. Un `Cargo.lock` périmé faisait échouer le premier passage : un passage ordinaire le rafraîchit d'abord, comme Cargo ; un passage verrouillé le refuse toujours et ne réécrit rien.
- `[[bindings]]` relie à ses tests C6, E5, T1, D1, D3, D4 ou D5, qui dépendent des règles de chaque jeu. La validation est stricte : contrôle inconnu ou répété, liste vide, exigence reliée deux fois ou hors des sept sont refusés, et un minimum commun garde ses contrôles. Relier une exigence conditionnelle la rend applicable ; rien ne la rend non applicable. Une liaison ne compte que par l'observation de son test. Le modèle relie E5 et D1, les deux que ses tests portent vraiment.
- Le contrôleur conserve les pannes TCP de Toxiproxy, mais borne sa connexion amont par un connecteur possédé : deux secondes pour joindre la vraie adresse LAN, sans nouvelle tentative ni repli. Un SYN bloqué ne retient plus la commande de coupure jusqu'au délai du système. Le connecteur borne ses ressources et les libère à l'arrêt ; les erreurs d'administration restent des pannes d'infrastructure.
- `certify` signale le pare-feu quand l'adresse du réseau ne répond pas. Les tests du harnais navigateur tournent dans un job indépendant des compilations Rust et iPhone, leurs fichiers l'un après l'autre. Le contrôle de navigation conserve l'historique des requêtes : la fermeture d'une connexion au délai ne signifie pas que sa ressource n'a pas été demandée. Ses délais et ses assertions ne changent pas.

### Outil

- `soiree contracts --migrate` disparaît : tous les jeux ont `verification.toml`.
- `soiree check .` lit les documents du commun contre le code : versions, commandes et options, chemins, scénarios, clés de `cert.json` et items. Le catalogue renvoie à `verification.toml`, `CONTRAT.md` dit ce que salon permet vraiment, et seuls les derniers résultats restent dans `certification/` ; les campagnes passées sont dans l'historique git.
- `soiree check` marche sans Node, sauf si `cert.json` déclare `ui.checks[].success`, et il le dit alors clairement. Le harnais l'exige toujours.
- `soiree doctor` vérifie l'éditeur de liens, que rustup n'installe pas, et GitHub CLI, qui reste facultatif.
- Le `.gitignore` du modèle ignore les rapports que `soiree check` réécrit : ils entraient dans le premier commit d'un ami avec des chemins locaux. Les rapports de `soiree certify` restent suivis, avec le code. Le modèle ne provoque plus de fausses erreurs `{{nom}}` dans la sortie de Cargo.
- Sous Windows, `soiree certify` s'arrête avant `npm ci` et dit qu'il n'y est pas encore disponible, sans le faux titre `✗ À corriger : 23 point(s)` : le harnais dépend de `lsof`, de `ps` et des groupes de processus POSIX. L'identité d'un passage trouve `node.exe`, `Path` et `USERPROFILE`, et Toxiproxy n'appelle plus `which`. Rien de cela n'a été exécuté sous Windows.
- Un test `should_panic` se lit : libtest écrit `test nom - should panic ... ok`, un nom que l'analyseur ne reconnaissait pas, ce qui transformait toute la cible en erreur d'infrastructure.

### Recherche mesurable

`recherche/` rend mesurable la méthode du manifeste : une fiche par sous-système, des citations vérifiables, chaque leçon reliée à un test ou à un trou. La fiche pilote, `recherche/reconnexion.toml`, a lu 28 sources et relevé 13 trous dans salon, dont trois graves.

- `soiree recherche plan` dit ce que le diff ajoute, le sous-système, le niveau, la fiche, ce qui manque et les étapes à suivre. Il n'empêche rien.
- `soiree recherche verifie` confronte à l'amont chaque commit et chaque citation des fiches. Un lien mort fait échouer la fiche ; une panne de réseau a son propre code et ne conclut rien.
- `soiree recherche dette` liste les sous-systèmes de niveau 3 sans fiche et les trous ouverts. `soiree check .` juge les fiches hors ligne, et la dette ne bloque qu'à l'étiquette.

La dette de niveau 3 restante, ligne par ligne :

- `fiabilite`, `transport-webrtc`, `telecommande` : pas encore de fiche. La méthode a d'abord été éprouvée sur la reconnexion, le sujet le plus risqué pour une soirée. Ces trois fiches sont à écrire pour une version suivante.
- T12, moyen : le chaos joue maintenant rechargements, jumeaux et redémarrages. Il reste au harnais à employer `slow_close` et `reset_peer`, sur un vrai transport.
- T9, faible : `wake` coupe un lien encore valide après 1,5 s de silence. Rien n'est perdu, et le coût visible pour un joueur reste à juger sur de vrais téléphones (`REPETITION.md`).
- T11, faible : le rattrapage ne se distingue pas du direct. Rien ne montre que cela gêne un jeu, et les jeux peuvent horodater leurs événements.

### Nuit, mutations, tests

- `nuit.yml` lance chaque nuit, sur des machines Linux gratuites, ou à la main, jamais sur un push : les mutations de `salon-core`, dont le rapport n'a pas de seuil, et un fuzzing borné dont le corpus se garde d'une nuit à l'autre. `salon/fuzz/` attaque les paquets et une session hostile : un lien hostile doit laisser connecté un téléphone honnête, et lui devoir exactement ce qui lui était dû.
- Une campagne réelle sur `salon-core` laissait 141 mutants vivants sur 433 : accusés, renvois, renumérotation et échéances n'avaient aucun test pour les voir manquer. La campagne suivante en laisse 11 sur 453. Les nouveaux tests (`client_edges.rs`, `host_edges.rs`, `stream_edges.rs`, `types_edges.rs`) forgent des paquets avec une copie du format, et `found_bugs.rs` garde chaque bug reproduit en les lisant.
- Le chaos joue rechargements, onglets jumeaux, « Jouer ici », pages mortes en silence, redémarrages de l'hôte et paquets retenus : retirer seul le correctif d'un des trous T1, T2, T3 ou T6 le fait rougir en moins de 3 000 cas. `NOTE_CRITICAL` a son test : le retirer de la minuterie le fait échouer. Un serveur réel dans un processus fils prouve la mort de la salle.

### L'app iPhone

Le projet Xcode déclare les fichiers produits par sa phase de compilation Rust : les liaisons Swift, leur en-tête, leur module et l'archive native. Xcode attend ainsi leur génération avant de compiler Swift, y compris sur un checkout neuf, et la CI le vérifie sans liaisons prégénérées. Aucun `cargo xtask ios` préalable n'est nécessaire.

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
