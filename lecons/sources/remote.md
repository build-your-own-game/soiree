# Leçons de `soiree/remote` : l'app iPhone et son contrat

## La source

**Source couverte.** `soiree/remote` à l'étiquette `avant-refonte` : l'app iPhone « Télécommande », son cœur Rust, son protocole, son SDK de page, son banc et son historique git. S'y ajoute `recherche/telecommande.toml`, la fiche de recherche propre à l'app : `remote/LEARNINGS.md` et les commits y renvoient sans cesse (trous T1 à T18).

**Ce que j'ai lu.**
- En entier : `remote/LEARNINGS.md`, `remote/CONTRAT.md`, `remote/AGENTS.md`, `remote/README.md`, `remote/Cargo.toml`, `rust-toolchain.toml`.
- En entier : `crates/protocol/src/{lib,layout,web}.rs`, `crates/core/src/{lib,session,remote,draw,trust,join}.rs`, `crates/core/src/wheel.rs` (code), `crates/core/src/pointer.rs` (code), `crates/core/src/controls.rs` (code ; tests survolés), `crates/web/src/{lib,message}.rs`, `crates/ffi/src/web.rs` (exports et tests).
- En entier : `ios/Remote/{GameConnection,WebRemote,RemoteModel,TouchSurface,GameScreen,PadScreen,LocalNetwork,RemoteApp,JoinView}.swift`, `ios/Info.plist`, la phase « Build Rust core » de `project.pbxproj`.
- Survolés (commentaires, noms de tests, constantes) : `crates/protocol/src/presets.rs`, `crates/core/src/feel.rs`, `crates/core/tests/properties.rs` et son fichier de régressions, `crates/protocol/tests/`, `crates/ffi/src/lib.rs`, `fuzz/fuzz_targets/`, `ios/Remote/{ControlViews,LayoutViews}.swift`, `bench/` (tests, hôte, coquille, `salon-door`, `BenchApp.swift`, son `Info.plist`), `xtask/src/main.rs`, `crates/demo`, `crates/demo-page`, `tests/*.mjs`, `tests/WebContract.swift`, `certification/check.md`.
- L'historique : les 38 commits qui touchent `remote/`, messages complets, et les lignes retirées de `LEARNINGS.md` et `CONTRAT.md` au fil des commits.
- Hors de `remote/` : `recherche/telecommande.toml` en entier ; les lignes de `REPETITION.md` et `CERTIFICATION.md` (S1 à S10) qui parlent de l'app.

**Ce que je n'ai pas pu lire.**
- Les références externes (Delta, PPSSPP, Dolphin, RetroArch, Moonlight, Capacitor, Cordova, WebKit, notes d'Apple, RFC, articles) : je les cite telles que l'ancien code les cite, sans les avoir relues.
- `salon/crates/host/src/apps.rs`, la porte `/remote` de salon : citée par `remote/LEARNINGS.md`, elle relève d'une autre source.
- `ios/Generated/remote_ffi.swift` (généré, 3 920 lignes), `ios/build/` (sorties de compilation), le corpus du fuzzer (3 794 fichiers de données), les `Cargo.lock`.
- Rien n'a été exécuté. Aucune mesure n'a été refaite, et rien de ceci n'a été vu sur un vrai iPhone, sauf mention contraire de l'ancienne source.

Le contrat de la refonte fait foi pour la colonne « Sous la nouvelle forme ». Les mécaniques nommées ici sont : réseau, reconnexion, entrées, manette tactile, stick tactile, boutons tactiles, croix, buzzer, dessin, volant, pointeur, vibration, capteurs, page téléphone, app iPhone, rejoindre, identité.

## Réseau et reconnexion

### REM-01 · Un hôte muet se voit en quelques secondes
- **Ce qui arrive :** seule une erreur de lecture disait qu'un lien était mort. Un hôte figé ou disparu n'en envoie aucune : TCP se tait, le noyau accuse encore réception. L'app gardait un pad qui n'appuyait sur rien, sans un mot. `URLSessionWebSocketTask` ne pingue pas seul. Le correctif envoie un ping toutes les 2 s et lâche un lien dont le ping reste sans réponse jusqu'au suivant.
- **D'où on le sait :** commit 5a90129 ; `remote/crates/core/src/session.rs:33-35` et `:276-294` ; `recherche/telecommande.toml`, T2 (sonde H2 : hôte figé 25 s, aucune erreur ni reconnexion) ; leçons telecommande-03 à 06 (RFC 6455 §5.5.2, doc Apple de `sendPing`, toxic `timeout` de Toxiproxy).
- **Mécaniques :** réseau, reconnexion.
- **Question :** chaque bout du lien détecte-t-il le silence de l'autre, même quand le jeu n'a rien à dire ?
- **Pression :** `session.rs`, `a_game_that_stops_answering_is_lost_within_4_seconds` ; `bench/tests/shell.rs:150` et `:182` (hôte et salon figés). Non mesuré : sur un vrai Wi-Fi, un trou de plus de 2 s coupe un lien sain (`REPETITION.md`, « Hôte muet »).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-02 · Un trou dans l'horloge de l'appareil n'accuse pas l'hôte
- **Ce qui arrive :** si l'app n'a pas tourné (suspension, fil principal bloqué), son ping n'a pas pu recevoir de réponse. Conclure à un hôte mort couperait un lien sain. Règle retenue : un écart de deux périodes veut dire que l'app dormait, et la surveillance repart de zéro.
- **D'où on le sait :** `session.rs:274-294` ; test `an_app_that_slept_does_not_blame_the_host` (`session.rs:574`) ; commit 5a90129.
- **Mécaniques :** réseau, reconnexion.
- **Question :** le chien de garde distingue-t-il « l'autre s'est tu » de « je n'ai pas tourné » ?
- **Pression :** le test cité.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-03 · Un serveur WebSocket qui cesse de lire cesse de répondre aux pings
- **Ce qui arrive :** `tungstenite` et les bibliothèques semblables ne répondent à un ping qu'en lisant le socket. Une porte qui ne lit plus, parce qu'elle n'attend rien, laisse tomber chaque page calme toutes les quelques secondes.
- **D'où on le sait :** `remote/CONTRAT.md:41` ; `bench/tests/shell.rs:166-169` ; `remote/LEARNINGS.md:212` (9 s de page muette tenues contre un vrai salon).
- **Mécaniques :** réseau.
- **Question :** le relais et la partie qui fait foi lisent-ils chaque lien en continu, même sans rien attendre ?
- **Pression :** `bench/tests/shell.rs`, `a_real_salon_answers_the_pings_of_a_quiet_page`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-04 · Chaque tentative de connexion a sa propre échéance
- **Ce qui arrive :** `URLSession.shared` attend 60 s par défaut. Mesuré : 60,6 s sur une adresse muette, au premier scan comme après une coupure. Le plafond de 5 s entre deux essais ne voulait donc rien dire. Une échéance de 5 s par requête ne coupe pas un WebSocket ouvert et calme, tant que les pings passent.
- **D'où on le sait :** commit 97cc51f ; `ios/Remote/GameConnection.swift:68-72` ; T6 (sonde H3 sur 10.255.255.1) ; leçons telecommande-17 et 18 (doc Apple de `timeoutIntervalForRequest`) ; `LEARNINGS.md:212`.
- **Mécaniques :** reconnexion.
- **Question :** combien dure au pire une tentative, et ce plafond s'accorde-t-il avec l'attente entre deux tentatives ?
- **Pression :** `bench/tests/shell.rs:311`, `a_mute_address_costs_a_try_seconds_and_not_a_minute`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-05 · L'appareil revient seul, avec une attente croissante et bornée
- **Ce qui arrive :** après une coupure, l'app se reconnecte sans geste du joueur : 0,5 s, puis le double, jusqu'à 5 s, comme les téléphones de Sidequest. L'attente repart de zéro dès qu'une trame arrive. Le téléphone redit le même `Hello`.
- **D'où on le sait :** `GameConnection.swift:129-131` et `:172-191` ; `LEARNINGS.md:91` ; `CONTRAT.md:40`.
- **Mécaniques :** reconnexion.
- **Question :** après une coupure, l'appareil revient-il seul, sans marteler l'hôte ?
- **Pression :** `session.rs`, `hello_again_after_each_reconnection_with_the_same_player` ; `bench/tests/shell.rs:150`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-06 · Un lien ne compte qu'une fois vraiment ouvert
- **Ce qui arrive :** l'état « en cours » d'un `URLSessionWebSocketTask` ne dit pas que le socket est ouvert. La tâche accepte des envois avant l'ouverture, et une tentative ratée les emporte. `Hello` et ce qui attend ne partent donc qu'à `didOpenWithProtocol`.
- **D'où on le sait :** `LEARNINGS.md:210` ; `GameConnection.swift:42-43`, `:105-111`, `:230-236` ; commit d4ed19d.
- **Mécaniques :** réseau, reconnexion.
- **Question :** ce qu'on confie à un lien avant son ouverture reste-t-il gardé jusqu'à ce qu'il le soit ?
- **Pression :** partielle. `session.rs:446-454` vérifie qu'un essai qui s'ouvre puis meurt ne consomme rien. La course elle-même n'est pas éprouvée : le banc ne rougit pas sans la garde, ses tentatives échouent trop vite.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-07 · Un événement émis pendant une coupure attend le lien suivant
- **Ce qui arrive :** l'app vidait la file avant de regarder le socket. Pendant l'attente entre deux essais, les trames étaient jetées. Dessin en cours, hôte qui coupe : l'hôte recevait le début et 19 points, puis plus rien, ni la suite ni la fin. Le trait restait ouvert pour toujours. Socket.io garde de même ce qui est émis sur un lien mort. Reste ouvert : une trame déjà remise au noyau quand le lien meurt se perd sans erreur, sa fin comprise. Il faudrait un numéro accusé par l'hôte ; un Pong n'y suffit pas, et rejouer Annuler ou Effacer les doublerait.
- **D'où on le sait :** T1 (sonde H1) ; commit d4ed19d ; `session.rs:130-143` et `:191-199` ; leçons telecommande-01 et 02 (commit 8adcfbf de Socket.io) ; `CONTRAT.md:38`.
- **Mécaniques :** dessin, et tout événement : vote, buzzer.
- **Question :** un événement compte-t-il exactement une fois à travers une coupure, y compris celui qui était déjà parti quand le lien est mort ?
- **Pression :** `session.rs:435`, `what_is_drawn_between_two_sockets_waits_behind_hello_and_the_games_show` ; `bench/tests/shell.rs:106`. Aucune pour la trame perdue dans le noyau.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-08 · Un état d'avant la coupure ne vaut plus rien après
- **Ce qui arrive :** entre deux liens, les événements attendent, les états non. Au retour, l'état se relit frais. Rejouer un état périmé ferait bouger le jeu sur un geste déjà fini.
- **D'où on le sait :** `session.rs:10-13` et `:130-143` ; test `a_state_from_before_a_cut_is_not_kept` (`session.rs:480`).
- **Mécaniques :** entrées continues (stick, volant, pointeur).
- **Question :** après une reprise, l'hôte reçoit-il l'état présent plutôt que ceux d'avant ?
- **Pression :** le test cité.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-09 · Ce qui attendait repart derrière la poignée de main, au débit que le receveur accepte
- **Ce qui arrive :** salon jetait les trames d'une app tant qu'il n'avait pas reposé sa télécommande sur ce socket, et celles au-delà de 240 par seconde. Les traits en attente partent donc après le `Show` du jeu, à 180 par seconde. Le jeu doit répondre `Show` à chaque reconnexion, même inchangé : c'est ce qui dit à l'app qu'il l'a reconnue.
- **D'où on le sait :** commit d4ed19d ; `session.rs:36-38`, `:64-73`, `:183-217` ; `LEARNINGS.md:211` ; `CONTRAT.md:38` et `:40`.
- **Mécaniques :** reconnexion, dessin.
- **Question :** la file rejouée après une reprise attend-elle l'accord explicite de l'autre bout, et respecte-t-elle ses limites de débit ?
- **Pression :** `session.rs:496`, `a_long_backlog_goes_out_all_and_at_a_pace_salon_accepts` ; `bench/tests/shell.rs:106`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-10 · Une coupure du réseau ne lève pas les doigts
- **Ce qui arrive :** une coupure relâchait tous les doigts, et un toucher que le cœur ne connaît plus ne se reprend pas au mouvement. Après une micro-coupure, le stick restait mort tant que le joueur ne levait pas le pouce, sans que rien ne le dise. Une coupure n'enlève rien à la main ; seul le départ du premier plan relâche.
- **D'où on le sait :** T9 (sonde H17 : stick à 101, puis 0 malgré le mouvement) ; commit f36fb4e ; `GameConnection.swift:179` ; `RemoteModel.swift:73-78` ; leçon telecommande-24.
- **Mécaniques :** entrées tenues, manette tactile.
- **Question :** l'état local des entrées survit-il à une coupure du lien ?
- **Pression :** `session.rs:747`, `a_thumb_left_on_the_stick_still_steers_after_a_cut` ; `bench/tests/shell.rs:363`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-11 · Quitter le premier plan relâche tout
- **Ce qui arrive :** quand l'app perd ses touchers (sélecteur d'apps, appel), rien ne doit rester appuyé. Un toucher annulé relâche comme un toucher levé, comme chez Delta et PPSSPP. La perte de premier plan appelle un relâchement complet.
- **D'où on le sait :** `LEARNINGS.md:73` ; `GameScreen.swift:54-56` ; `TouchSurface.swift:94-96` ; `controls.rs:604-609` ; `recherche/telecommande.toml`, saturation, note sur DeltaCore.
- **Mécaniques :** entrées.
- **Question :** une entrée peut-elle rester appuyée quand l'appareil ne voit plus le doigt ?
- **Pression :** `remote.rs`, `every_preset_lays_out_and_answers_touches` ; sonde H5 réfutée dans `recherche/telecommande.toml` (le relâchement part en changeant d'app).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-12 · Un affichage redit à la reconnexion ne remet rien à zéro
- **Ce qui arrive :** salon renvoie `Show` à chaque reconnexion, et l'app reconstruisait la télécommande. Le téléphone perdait son dessin alors que la TV le gardait. Annuler n'avait plus rien à annuler, et le trait suivant reprenait le numéro 0.
- **D'où on le sait :** T10 (sonde H18) ; commit a49c0ef ; `session.rs:147-155` ; leçon telecommande-25.
- **Mécaniques :** dessin, reconnexion.
- **Question :** ce qu'affiche un appareil après une reprise est-il le même qu'avant, numéros compris ?
- **Pression :** `session.rs:716` ; `bench/tests/shell.rs:339` ; `crates/core/tests/properties.rs:333-334`.
- **Sous la nouvelle forme :** impossible par la forme. La vue d'un appareil se calcule depuis l'état (« La forme d'un jeu »), et la reprise rend à l'appareil sa place (« Ce que le commun possède », Reprise). Les traits et leurs numéros appartiennent à l'état, plus à la télécommande.
- **Étage :** commun

### REM-13 · Une entrée continue est un état complet : un seul en attente, le dernier gagne, un rappel régulier
- **Ce qui arrive :** Sidequest fondait 13 paquets en un. L'app envoie un état complet à chaque changement, au plus 60 fois par seconde en moyenne, et toutes les 250 ms sinon, pour qu'un jeu qui en a manqué un se rattrape. Moonlight note que fondre les états réduit la file, donc le retard. Le pointeur envoie sa position absolue, pas des écarts : un paquet perdu ne décale rien. Pour les datagrammes d'une page : envoyer des états entiers.
- **D'où on le sait :** `LEARNINGS.md:88` et `:153` ; `session.rs:5-8` et `:25-32` ; `CONTRAT.md:37` et `:92` ; leçon telecommande-38 (`InputStream.c` de moonlight-common-c).
- **Mécaniques :** stick, volant, pointeur, entrées continues.
- **Question :** une entrée continue part-elle comme un état entier, remplacé par le suivant, plutôt qu'en différences ?
- **Pression :** `session.rs:375`, `a_state_goes_out_on_change_at_most_60_times_a_second_and_every_250_ms` ; `properties.rs:299`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-14 · Un état porte un numéro qui ne fait que monter, même à travers les reconnexions
- **Ce qui arrive :** pendant un gel de l'hôte de 25 s, le noyau du téléphone avait accepté 98 états. Ils sont arrivés d'un bloc au dégel. Attendre la fin de chaque envoi n'y change rien. Le receveur doit garder le dernier et ignorer un numéro plus petit ; le numéro monte à travers les reconnexions et les changements de télécommande.
- **D'où on le sait :** leçon telecommande-38, note (sonde H2) ; `crates/protocol/src/lib.rs:83-84` ; `properties.rs:290-297` ; `CONTRAT.md:37`.
- **Mécaniques :** entrées continues.
- **Question :** le receveur reconnaît-il et jette-t-il un état plus vieux que le dernier vu, même arrivé après lui ?
- **Pression :** `properties.rs:299`, `a_phone_comes_back_as_itself_and_keeps_its_rhythm`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-15 · Un frein de débit ne mord que sur un vrai excès
- **Ce qui arrive :** le frein comparait chaque écart à 1/60 s pile, sur une horloge lue à chaque image. Un écart de 16,6 ms échouait, et une image sur deux attendait 33 ms : 35,9 états par seconde mesurés dans l'app. Le correctif laisse partir un état jusqu'à 4 ms plus tôt, sur des créneaux espacés de 1/60 s : la moyenne ne passe pas 60. Le frein d'AirConsole laisse aussi passer tout de suite ce qui suit un silence.
- **D'où on le sait :** T7 (sondes H15) ; commit 6fdbebb ; `session.rs:25-30` et `:270` ; leçons telecommande-19 et 20 (`rate-limiter.js` d'AirConsole).
- **Mécaniques :** réseau, entrées.
- **Question :** le frein laisse-t-il passer un envoi par image d'un écran à 60 Hz dont l'horloge gigote, sans dépasser la moyenne promise ?
- **Pression :** `session.rs:627` et `:635` ; `properties.rs:380-385`. Le rythme d'un vrai `CADisplayLink` reste à mesurer (`REPETITION.md`, « Rythme de l'écran »).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-16 · Un appui plus court qu'un envoi atteint quand même le jeu
- **Ce qui arrive :** l'état ne se lisait qu'à l'envoi. Un appui qui commençait et finissait entre deux envois n'atteignait jamais le jeu, alors que le téléphone avait vibré. Quatre touchers de 4 ms : aucun vu par l'hôte. Désormais, ce qui a été pressé depuis le dernier envoi part dans l'état suivant, puis le relâchement. Moonlight traite de même un appui et son relâchement arrivés d'un bloc, et un bouton y clôt un regroupement.
- **D'où on le sait :** T8 (sonde H16) ; commit b7bae9f ; `remote.rs:41-43` et `:218-244` ; `LEARNINGS.md:94` ; leçons telecommande-21 à 23 (`ControllerSupport.m` de moonlight-ios, commit 421bd74 de moonlight-common-c).
- **Mécaniques :** boutons tactiles, buzzer.
- **Question :** un appui qui commence et finit entre deux échantillons laisse-t-il une trace que le jeu voit ?
- **Pression :** `session.rs:671`, `a_press_shorter_than_a_send_reaches_the_game_and_ends` ; `bench/tests/shell.rs:324`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-17 · Un état ne sait pas compter les appuis
- **Ce qui arrive :** deux appuis dans le même envoi se fondent en un. Dans l'app, cinq touchers à 12 ms d'écart ont donné trois états. Un jeu qui compte les tapes ne peut pas se fier à un état échantillonné.
- **D'où on le sait :** `LEARNINGS.md:94` ; T8, preuve de la variante.
- **Mécaniques :** buzzer, tapotement, rythme.
- **Question :** le jeu a-t-il besoin du nombre d'appuis, ou seulement de leur présence ? S'il compte, déclare-t-il l'appui comme un événement ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-18 · Un appui fait pendant une coupure n'est pas joué après elle comme s'il venait d'être fait
- **Ce qui arrive :** pendant une coupure, l'app oublie les appuis : arrivés plus tard, ils passeraient pour neufs. C'est en tension avec « un événement compte exactement une fois, même en retard ». Le nouveau contrat tranche par l'heure du geste et la fenêtre du passé provisoire.
- **D'où on le sait :** commit b7bae9f ; `session.rs:250-253` ; test `a_press_made_during_a_cut_is_not_played_after_it` (`session.rs:701`).
- **Mécaniques :** buzzer, entrées.
- **Question :** un geste qui arrive après la fenêtre est-il écarté, ou placé à son vrai moment, plutôt que joué à l'instant de son arrivée ?
- **Pression :** le test cité.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-19 · L'ordre d'arrivée ment sur les courses serrées
- **Ce qui arrive :** le Wi-Fi a une longue traîne : p90 vers 20 ms, p99 vers 250 ms. Sur un buzzer, le premier arrivé n'est pas le premier à avoir appuyé. Il faut l'heure du toucher, recalée sur l'horloge de l'hôte (algorithme de Cristian). L'app garde `UITouch.timestamp`, mais ne l'envoie pas.
- **D'où on le sait :** `LEARNINGS.md:128` (Microsoft Research, algorithme de Cristian) ; `TouchSurface.swift:6-7` et `:77`.
- **Mécaniques :** buzzer, courses.
- **Question :** qui a appuyé le premier se décide-t-il par l'heure recalée du geste, et non par l'ordre d'arrivée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-20 · La latence du toucher à la TV n'a jamais été mesurée
- **Ce qui arrive :** le budget de 80 ms porte sur le trajet du doigt jusqu'à la TV. Ni l'app ni son banc ne l'ont mesuré. Le défaut du frein (REM-15) coûtait une image à une frappe sur deux sans que rien ne le montre.
- **D'où on le sait :** `LEARNINGS.md:107` et `:182` (écart 7) ; leçon telecommande-20.
- **Mécaniques :** entrées, toutes.
- **Question :** quelle est la distribution du toucher au pixel de la TV, et quelle part revient au matériel ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-21 · Sur TCP, une perte retient tout ce qui suit
- **Ce qui arrive :** l'app passait par un WebSocket, donc TCP, par choix assumé. Pour limiter le retard d'un paquet bloqué : état complet, numéroté, 60 par seconde au plus, Nagle coupé. Une perte de 200 ms peut lâcher le personnage un instant, pouce immobile : supposé, jamais mesuré.
- **D'où on le sait :** `LEARNINGS.md:181` (écart 6) ; leçon telecommande-44 ; `REPETITION.md:79`.
- **Mécaniques :** entrées continues, réseau.
- **Question :** le jeu reste-t-il correct et jouable sur le pire canal disponible ?
- **Pression :** aucune ; essai humain dans `REPETITION.md`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-22 · Une entrée qui passe par TCP coupe Nagle des deux côtés
- **Ce qui arrive :** sans `TCP_NODELAY`, de petits envois attendent. Le faux jeu le coupe. Côté téléphone, `URLSessionWebSocketTask` le pose seul : vérifié dans le simulateur par les drapeaux du socket, supposé sur un vrai iPhone.
- **D'où on le sait :** `crates/demo/src/main.rs:199` ; leçon telecommande-37 (`PlatformSockets.c` de moonlight-common-c, sonde H19) ; `LEARNINGS.md:181`.
- **Mécaniques :** réseau.
- **Question :** chaque socket TCP qui porte des entrées a-t-il Nagle coupé, à chaque bout ?
- **Pression :** aucune dans le dépôt ; une sonde ponctuelle.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-23 · Un geste commencé finit, même quand la vue change en plein geste
- **Ce qui arrive :** un jeu qui changeait de télécommande en plein trait ne recevait jamais la fin du trait : elle restait dans l'ancienne toile, jetée. Désormais, avant de remplacer la vue, la session lâche tous les doigts et envoie ce qui restait, fin comprise, y compris vers une page. Les tests de propriétés l'ont trouvé ; le fuzzer le retrouve en quelques secondes.
- **D'où on le sait :** commit df675cb et 2f770aa ; `session.rs:169-181` ; `properties.rs:510-516` ; `LEARNINGS.md:162` ; `properties.proptest-regressions`, second cas.
- **Mécaniques :** dessin, gestes en cours.
- **Question :** quand la vue change, chaque geste en cours reçoit-il sa fin ?
- **Pression :** `properties.rs:516`, `a_stroke_cut_by_a_new_remote_still_ends` ; `fuzz/fuzz_targets/phone_session.rs`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-24 · Un changement de taille ou une rotation lâche les doigts posés
- **Ce qui arrive :** une nouvelle disposition, rotation comprise, relâche tout, comme une main levée. Le redimensionnement de la première rotation levait donc les doigts déjà posés ; un test en devenait instable. Pour un joueur, cela tombe avant la manche.
- **D'où on le sait :** `LEARNINGS.md:215` ; `controls.rs:553-558` ; `bench/tests/shell.rs:77-95` ; commit f36fb4e.
- **Mécaniques :** entrées, orientation.
- **Question :** un changement de taille en pleine manche lâche-t-il les entrées tenues, et le joueur peut-il le provoquer sans le vouloir ?
- **Pression :** aucune ; le banc attend un écran stable.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-25 · Au retour de suspension, un socket peut être mort sans le dire
- **Ce qui arrive :** iOS peut reprendre les ressources d'un socket pendant la suspension. Au retour, seule une erreur le dit. Dans le simulateur, 15 s en arrière-plan n'ont pas tué le socket. La reprise après verrouillage d'un vrai iPhone reste à faire.
- **D'où on le sait :** leçon telecommande-42 (TN2277 d'Apple) ; `GameConnection.swift:113-122`.
- **Mécaniques :** reconnexion, suspension.
- **Question :** au retour du premier plan, l'appareil vérifie-t-il son lien au lieu d'attendre une erreur ?
- **Pression :** S1 et S2 de `CERTIFICATION.md`, gestes humains.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-26 · Une place se garde quand le téléphone quitte le premier plan
- **Ce qui arrive :** Moonlight met fin au flux après 60 s d'inactivité. L'app a refusé cette voie : un flux vidéo coûte, une place de soirée se garde. L'app reste sur sa page et se reconnecte.
- **D'où on le sait :** leçon telecommande-41 (`StreamFrameViewController.m` de moonlight-ios).
- **Mécaniques :** reconnexion.
- **Question :** un appareil parti un moment retrouve-t-il sa place sans que personne ne fasse rien ?
- **Pression :** S1 et S2, gestes humains.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-27 · L'hôte applique le contrat de lien d'une app en un seul endroit
- **Ce qui arrive :** au début, la porte `/remote` de salon répondait toujours la page et jetait les entrées : aucun jeu ne pouvait choisir un pad. Le correctif tient les apps par joueur et applique en un seul endroit le numéro, le silence, le débit et une table bornée. Un jeu ne touche jamais un socket.
- **D'où on le sait :** commit eef70bc ; `CONTRAT.md:5-8` ; leçon telecommande-03.
- **Mécaniques :** réseau, app iPhone.
- **Question :** chaque appareil, page ou app, est-il servi par le même code du commun, borné en nombre et en débit ?
- **Pression :** commit eef70bc : un faux client du protocole contre un vrai serveur.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## Dessin

### REM-28 · Un dessin voyage en traits vectoriels, en entiers, dans une toile fixe
- **Ce qui arrive :** Jackbox est passé des images aux vecteurs. La toile mesure 4096 × 2304, en 16:9 comme la TV, pour se lire pareil sur chaque téléphone. Les coordonnées sont signées : un trait peut déborder sans être coupé (scribble.rs).
- **D'où on le sait :** `LEARNINGS.md:160` ; `crates/protocol/src/lib.rs:155-182` ; `crates/core/src/draw.rs:10-11`.
- **Mécaniques :** dessin.
- **Question :** un dessin se transmet-il en traits, indépendants de l'écran qui les a tracés ?
- **Pression :** `draw.rs`, `the_canvas_is_16_by_9_between_the_tool_columns`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-29 · Chaque trait a un identifiant
- **Ce qui arrive :** sans identifiant, il faut deviner les traits, et Annuler devient fragile (scribble.rs #279). Avec, Annuler et Effacer deviennent triviaux : début, points, fin, annuler, effacer.
- **D'où on le sait :** `LEARNINGS.md:161` ; `crates/protocol/src/lib.rs:159-179`.
- **Mécaniques :** dessin.
- **Question :** un trait se désigne-t-il sans ambiguïté, pour être annulé ou rejoué ?
- **Pression :** `remote.rs`, `drawing_on_the_canvas_with_the_tools_beside_it`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-30 · Un seul doigt dessine
- **Ce qui arrive :** un second doigt posé pendant un trait est ignoré, comme dans scribble.rs.
- **D'où on le sait :** `LEARNINGS.md:163` ; `draw.rs:4` et `:147-149`.
- **Mécaniques :** dessin.
- **Question :** que fait un deuxième doigt sur la toile ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-31 · Un dessin lit tous les échantillons du doigt
- **Ce qui arrive :** UIKit livre les touchers vers 60 Hz, alors que l'écran échantillonne à 120 Hz et plus. Sans les touchers fusionnés, un trait rapide perd la moitié de ses points.
- **D'où on le sait :** `LEARNINGS.md:164` (doc Apple de `coalescedTouches`) ; `TouchSurface.swift:11-13` et `:84`.
- **Mécaniques :** dessin.
- **Question :** la page lit-elle chaque échantillon fusionné du doigt, pas seulement le dernier par image ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-32 · Lisser un trait dans le temps et le finir sur le vrai dernier point
- **Ce qui arrive :** les points à moins de 1,5 pt du précédent sont jetés. Le lissage est une moyenne dans le temps, 12 ms, pas par échantillon : environ le « streamline 0,6 » de perfect-freehand à 120 échantillons par seconde. Au lever, le trait rejoint le vrai dernier point.
- **D'où on le sait :** `LEARNINGS.md:166` (perfect-freehand, signature_pad, tldraw) ; `draw.rs:17-23` et `:177-221`.
- **Mécaniques :** dessin.
- **Question :** le lissage dépend-il du temps plutôt que du nombre d'échantillons, et le trait finit-il là où le doigt s'est levé ?
- **Pression :** `draw.rs`, `a_line_follows_the_finger_and_ends_on_its_last_point`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-33 · Une tape laisse un point
- **Ce qui arrive :** un trait sans mouvement doit se voir. Sans règle, une tape ne dessine rien.
- **D'où on le sait :** `LEARNINGS.md:167` (perfect-freehand) ; `draw.rs:7`.
- **Mécaniques :** dessin.
- **Question :** une tape sur la toile laisse-t-elle une trace ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-34 · Pour deviner un dessin, lisible bat joli
- **Ce qui arrive :** une épaisseur constante se lit mieux qu'un trait qui varie avec la vitesse. Excalidraw a ajouté cette option en 2026.
- **D'où on le sait :** `LEARNINGS.md:168` ; `draw.rs:7-8`.
- **Mécaniques :** dessin à deviner.
- **Question :** le trait reste-t-il lisible à 3 m, sur la TV ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-35 · Un trait se dessine en courbes par les milieux, pas en segments
- **Ce qui arrive :** relier les échantillons par des segments donne un polygone sur un trait rapide (scribble.rs #368).
- **D'où on le sait :** `LEARNINGS.md:169` ; `ios/Remote/LayoutViews.swift:89-90`.
- **Mécaniques :** dessin.
- **Question :** le rendu d'un trait rapide reste-t-il une courbe ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-36 · Peu d'outils, et Effacer se confirme
- **Ce qui arrive :** 8 à 12 couleurs, 3 tailles, Annuler, Effacer ; ni pot de peinture ni formes (scribble.rs, Drawful 2). L'app en garde 7, de 44 pt chacune. Effacer demande deux appuis en 2 s.
- **D'où on le sait :** `LEARNINGS.md:170` ; `draw.rs:24-25` et `:240-249`.
- **Mécaniques :** dessin.
- **Question :** un geste qui détruit le dessin peut-il partir par accident ?
- **Pression :** `remote.rs`, `drawing_on_the_canvas_with_the_tools_beside_it`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-37 · Les touchers prédits ne servent qu'à l'affichage local
- **Ce qui arrive :** iOS fournit des touchers prédits, provisoires, faits pour le dessin. Ils ne partent jamais au jeu. L'app ne s'en sert pas encore.
- **D'où on le sait :** `LEARNINGS.md:75` et `:165` (doc Apple).
- **Mécaniques :** dessin, prédiction.
- **Question :** un échantillon prédit par le système peut-il atteindre la partie qui fait foi ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

## La page dans l'app (WebKit)

### REM-38 · Un chargement raté se retente, et le joueur le sait
- **Ce qui arrive :** la page du jeu ne venait pas, et rien ne réessayait. Le socket allait bien : aucun message. Le joueur voyait un fond vide et la croix. WebKit tente quatre fois en quelques millisecondes, puis abandonne (code -1005). Le correctif réessaie après 1 s, puis le double jusqu'à 5 s, et l'écran dit que la page ne répond pas. C'est le chemin par défaut de tous les jeux.
- **D'où on le sait :** T3 (sonde H8) ; commit 9e8b40a ; `WebRemote.swift:176-218` ; `GameConnection.swift:252-253` ; leçon telecommande-13 (`RNCWebViewImpl.m` de react-native-webview).
- **Mécaniques :** page téléphone, app iPhone.
- **Question :** une page qui ne vient pas peut-elle laisser un écran vide, sans un mot ?
- **Pression :** `bench/tests/shell.rs:211`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-39 · La mort du processus de la page se traite soi-même
- **Ce qui arrive :** WebKit recharge seul une page morte une fois par fenêtre de 30 s. À la mort suivante, la vue reste vide, sans rien dire. Le symptôme vu ailleurs : écran blanc au retour, rien à toucher, cause invisible pendant des mois. Le correctif traite la mort : nouveau chargement plutôt que `reload()`, car une vue morte peut n'avoir plus d'adresse, avec la même attente et le même mot qu'un échec. Cordova note qu'un rechargement ne suffit pas toujours.
- **D'où on le sait :** T4 (sonde H7) ; commit 84e5f51 ; `WebRemote.swift:189-196` ; leçons telecommande-07, 09 à 12 et 14 (Capacitor beebff4, Cordova 5a13d63 et issue 1232, `WebPageProxy.cpp` de WebKit, doc Apple).
- **Mécaniques :** page téléphone, app iPhone.
- **Question :** que voit le joueur quand le moteur de la page meurt deux fois de suite ?
- **Pression :** `bench/tests/shell.rs:270`. Le banc tue le processus par un crochet de test de WebKit, qui ne recharge jamais seul : plus dur que la réalité. La mort par manque de mémoire sur un vrai iPhone reste à essayer (`REPETITION.md`, « Page lourde »).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-40 · Ce que tenait une page morte ou remplacée est rendu
- **Ce qui arrive :** le pont gardait les capteurs et les sockets d'une page disparue. La page rechargée trouvait ses huit sockets pris. Capacitor remet aussi son pont à zéro à la mort de la page.
- **D'où on le sait :** T4 ; commit 84e5f51 ; `WebRemote.swift:192-196` et `:220-229` ; leçon telecommande-08 (Capacitor 1de1f15).
- **Mécaniques :** capteurs, sockets, page téléphone.
- **Question :** à la mort ou au remplacement d'une page, chaque ressource qu'elle tenait est-elle libérée ?
- **Pression :** `bench/tests/shell.rs:270` (sockets libres après trois morts).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-41 · Une navigation annulée n'est pas une panne
- **Ce qui arrive :** annuler une navigation la fait échouer avec l'erreur 102 de `WebKitErrorDomain`, pas de `WKErrorDomain`. Un nouvel essai naïf ramenait alors l'ancienne page par-dessus la suivante. Une annulation d'URLSession, au remplacement d'un chargement, ne relance rien non plus.
- **D'où on le sait :** `LEARNINGS.md:213` (banc, T13) ; commit 237705b ; `WebRemote.swift:65-66` et `:198-205`.
- **Mécaniques :** page téléphone.
- **Question :** le code qui réessaie distingue-t-il un échec d'une annulation voulue ?
- **Pression :** `bench/tests/shell.rs:476`.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-42 · Une page qui en remplace une autre n'en hérite rien
- **Ce qui arrive :** une orientation demandée ou une donnée injectée survivait au changement de page. Chaque page a donc sa vue web neuve, qui repart en paysage. La mise à jour d'une vue ne recharge rien : elle aurait couru avec les nouveaux essais et ramené une page d'où elle était partie.
- **D'où on le sait :** `LEARNINGS.md:97` ; `GameScreen.swift:67-73` ; `WebRemote.swift:26-27` et `:51-53` ; commits cf03bf8 et 9e8b40a.
- **Mécaniques :** page téléphone.
- **Question :** une nouvelle page part-elle d'un état propre, sans rien de la précédente ?
- **Pression :** aucune dédiée ; sonde H9 réfutée.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-43 · Une valeur annoncée par événement se redit quand la page commence à écouter
- **Ce qui arrive :** une rotation pendant le chargement arrivait avant que la page écoute : elle ne l'apprenait jamais. L'app redit l'orientation à la fin du chargement, et pose la valeur courante avant les scripts.
- **D'où on le sait :** `LEARNINGS.md:97` ; `WebRemote.swift:168-174` ; `crates/protocol/src/web.rs:336-342`.
- **Mécaniques :** page téléphone, orientation, capacités.
- **Question :** une page qui arrive reçoit-elle l'état présent de chaque capacité, et pas seulement les changements à venir ?
- **Pression :** `cargo xtask web-contract-browser` (vérifie l'information et les événements dans WebKit et Chromium).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-44 · Un demi-tour ne change ni la taille ni les marges
- **Ce qui arrive :** l'orientation n'était relue qu'au changement de taille ou de marges. D'un paysage à l'autre, rien ne bouge : l'événement ne partait pas, `window.telecommande` mentait, et le volant braquait à l'envers (83 contre -83). Il faut observer la géométrie de la scène elle-même.
- **D'où on le sait :** T11 (sonde H14) ; commit 29d584f ; `WebRemote.swift:302-334` ; `TouchSurface.swift:58-70` ; leçon telecommande-26.
- **Mécaniques :** volant, pointeur, orientation.
- **Question :** l'appareil voit-il un retournement de 180° ?
- **Pression :** `bench/tests/shell.rs:392` et `:420`, par une demande de géométrie, pas par une main, sans capteurs. Sur un vrai iPhone : `REPETITION.md`, « Retournement ».
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-45 · Une ressource de page vit tant que son propriétaire la garde, et un capteur partagé s'arrête au dernier
- **Ce qui arrive :** dans le SDK de page, un abonnement libère sa ressource à sa destruction. Deux vues partagent un seul capteur : seul le dernier abonnement détruit l'arrête. Remplacer un abonné chevauche l'ancien et le nouveau sans couper le capteur. La libération ne dépend ni du nombre de vues ni des rappels déjà reçus.
- **D'où on le sait :** `CONTRAT.md:14-19` ; `LEARNINGS.md:185-191` ; `crates/web/src/lib.rs:76-113` ; `tests/web-lifecycle-browser.mjs:1` et `:59-60`.
- **Mécaniques :** capteurs, page téléphone.
- **Question :** détruire une vue peut-il couper une ressource dont une autre vue a encore besoin ?
- **Pression :** `node remote/tests/web-lifecycle-browser.mjs`, dans WebKit et Chromium, avec le vrai SDK compilé.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-46 · Un événement du navigateur peut revenir pendant qu'on le traite
- **Ce qui arrive :** les événements du DOM peuvent se déclencher de façon synchrone, au milieu d'un rappel. Un emprunt mutable gardé à travers un appel au navigateur panique à la réentrée. Règle de l'ancien SDK : l'état mutable dans une cellule du propriétaire, et aucun emprunt ne traverse un appel qui peut déclencher un événement.
- **D'où on le sait :** `CONTRAT.md:20-23` ; `crates/web/src/lib.rs:102`.
- **Mécaniques :** page téléphone.
- **Question :** un rappel peut-il être rappelé avant d'avoir rendu la main, et le code y survit-il ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-47 · L'app n'injecte qu'une donnée, jamais du JavaScript écrit à la main
- **Ce qui arrive :** la première version injectait des fonctions dans `window.telecommande` (vibrer, capteurs). Elle n'injecte plus qu'une donnée ; la page poste des messages au gestionnaire de WebKit et reçoit des événements. Une page en Rust passe par la colle générée.
- **D'où on le sait :** commit cf03bf8 ; `LEARNINGS.md:96` ; `WebRemote.swift:87-92` ; lignes retirées de `CONTRAT.md` au commit cf03bf8.
- **Mécaniques :** page téléphone, app iPhone.
- **Question :** l'app ou le commun ajoutent-ils à une page du code que personne ne compile ni ne type ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-48 · Le contrat entre la page et l'app a une seule définition
- **Ce qui arrive :** le JSON entre page et app était lu clé par clé en Swift, et `player` restait une chaîne sans contrôle de ses 32 chiffres. Désormais un seul module Rust le définit ; Swift reçoit des enums et des records générés, par des conversions exhaustives. Deux contrôles le vérifient avec les vraies liaisons Swift, dans JavaScriptCore, puis dans WebKit et Chromium avec le vrai SDK compilé.
- **D'où on le sait :** `LEARNINGS.md:183` (écart 8) et `:200` ; `AGENTS.md:16` ; lignes retirées au commit b226c97 ; `tests/WebContract.swift:1-2` ; `tests/web-contract-browser.mjs:1-3` ; `crates/ffi/src/web.rs:1-3`.
- **Mécaniques :** app iPhone, page téléphone.
- **Question :** un renommage d'un côté casse-t-il la compilation de l'autre ?
- **Pression :** `cargo xtask web-contract` et `cargo xtask web-contract-browser` (macOS).
- **Sous la nouvelle forme :** impossible par la forme. « Les types de bout en bout » : le contrat entre une page et l'app iPhone a une définition Rust unique, dont les liaisons Swift sont générées.
- **Étage :** commun

### REM-49 · Une page n'ouvre pas de socket elle-même
- **Ce qui arrive :** l'app offrait de l'UDP aux pages, vers l'hôte du jeu. Salon n'en livrait aucun au jeu : la capacité n'existait que hors de salon, et le contrat l'a dit tard. Les datagrammes se perdent ou se doublent.
- **D'où on le sait :** commit e6d3a0a ; `CONTRAT.md:92` et `:110-112` ; `crates/web/src/lib.rs:116-153`.
- **Mécaniques :** réseau, page téléphone.
- **Question :** un jeu peut-il ouvrir un canal réseau qui contourne le commun ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme. Le module est enfermé et n'atteint le monde que par le commun ; le réseau appartient au commun (« Ce que le commun possède »).
- **Étage :** commun

### REM-50 · Dans l'app, une page peut jouer du son sans geste préalable
- **Ce qui arrive :** la vue web de l'app autorise la lecture en ligne et n'exige aucun geste avant le son. Safari, lui, attend un premier geste. C'est une capacité que l'app apporte. Non vérifié par un test.
- **D'où on le sait :** `WebRemote.swift:33-34`.
- **Mécaniques :** son, capacités.
- **Question :** la capacité « son sans geste préalable » est-elle déclarée seulement là où elle existe ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

## La frontière de confiance du pont

### REM-51 · Seule la page demandée atteint les capacités natives
- **Ce qui arrive :** le pont ne regardait ni le cadre ni l'origine, et aucune navigation n'était filtrée. Un iframe d'une autre origine envoyait de l'UDP. Une page partie ailleurs emportait `window.telecommande`, donc l'identité du joueur. La règle : le cadre principal seulement, sur le même schéma, hôte et port que la page demandée. Une navigation du haut vers une autre origine est annulée, `window.open` n'ouvre rien, une redirection de `http` vers `https` compte comme une autre origine, et un iframe ne reçoit ni pont ni identité. Capacitor décide lui aussi de chaque navigation.
- **D'où on le sait :** T13 (sonde H11) ; commit 237705b ; `crates/core/src/trust.rs:1-16` et `:83-95` ; `WebRemote.swift:94-119` ; `CONTRAT.md:126-135` ; leçon telecommande-29 (`WebViewDelegationHandler.swift` de Capacitor).
- **Mécaniques :** app iPhone, page téléphone.
- **Question :** un contenu chargé par la page, ou vers lequel elle mène, peut-il appeler une capacité native ?
- **Pression :** tests de `trust.rs` ; `bench/tests/shell.rs:441` et `:476`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-52 · Une adresse qu'on pourrait lire de deux façons est refusée
- **Ce qui arrive :** un `user@` dans l'adresse, un antislash, un `%` dans l'hôte, un port illisible : un navigateur et notre analyseur pourraient en tirer deux hôtes différents. Tout cela est refusé, comme `about:`, `data:`, `blob:`, `file:` et tout schéma autre que `http` et `https`. `127.0.0.1` et `localhost` sont deux origines.
- **D'où on le sait :** `trust.rs:11-16`, `:43-56` et tests `:123-146`.
- **Mécaniques :** app iPhone, rejoindre.
- **Question :** l'analyse d'une adresse peut-elle diverger de celle du navigateur sur l'hôte ou le port ?
- **Pression :** tests de `trust.rs`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-53 · Micro et caméra : le joueur répond, pas l'app à sa place
- **Ce qui arrive :** l'app accordait le micro sans question aux pages de l'hôte du QR. Or tout QR en `http` rejoint, le scanner rejoint au premier code lu, et la caméra est déjà accordée pour scanner. Une page derrière n'importe quel QR obtenait le micro sans un mot. Le besoin d'éviter la double question est réel (Cordova #1166), mais se règle pour une page que l'app a choisie, pas pour toute adresse lue dans un QR.
- **D'où on le sait :** T14 (sonde H12) ; commit 9e4c5b9 ; `WebRemote.swift:157-166` ; `LEARNINGS.md:98` ; leçon telecommande-30.
- **Mécaniques :** app iPhone, micro, caméra.
- **Question :** une permission donnée à l'app peut-elle passer à une page sans que le joueur la voie ?
- **Pression :** `bench/tests/shell.rs:509`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-54 · Sur l'adresse d'une soirée, micro et caméra n'existent pas
- **Ce qui arrive :** la norme réserve `navigator.mediaDevices` aux contextes sécurisés. Une page en `http` sur une IP du réseau local n'en est pas un ; la même page sur `127.0.0.1` l'est. WebKit applique la règle dans l'app comme dans Safari. Le contrat promettait le micro ; le bouton du faux jeu levait une exception.
- **D'où on le sait :** T12 (sonde H10) ; commit 367201d ; `CONTRAT.md:93` ; leçons telecommande-27 et 28 (`getusermedia.html` du W3C, `Navigator+MediaDevices.idl` de WebKit).
- **Mécaniques :** micro, caméra, capacités.
- **Question :** le commun déclare-t-il une capacité que le contexte de la page ne peut pas donner ?
- **Pression :** `bench/tests/shell.rs:530`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-55 · Un identifiant partagé par tous les jeux n'est pas un secret
- **Ce qui arrive :** `window.telecommande.player` vaut les mêmes 16 octets dans tous les jeux, et toute page de l'origine peut le lire. Sidequest l'employait comme jeton de salon. Une page malveillante à laquelle un QR mène directement est sa propre origine : elle le reçoit.
- **D'où on le sait :** `CONTRAT.md:135` ; T13, « suppose » ; `trust.rs:4-6`.
- **Mécaniques :** identité.
- **Question :** un jeu se sert-il d'un identifiant lisible par d'autres pour authentifier un joueur ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme. Le jeu reçoit « un identifiant que seul le commun peut créer » (« Ce que le commun possède », Identité), et le relais chiffre de bout en bout avec la clé du QR : un tiers ne peut ni lire ni forger une identité.
- **Étage :** commun

### REM-56 · Un texte injecté dans un script est échappé par le sérialiseur
- **Ce qui arrive :** le prénom du joueur entre dans le script posé avant la page. Un prénom avec guillemets, antislash, retour à la ligne ou `</script>` doit rester une donnée. Le JSON est produit en Rust et relu par le décodeur de la page.
- **D'où on le sait :** `crates/ffi/src/web.rs:262-275` ; `LEARNINGS.md:196`.
- **Mécaniques :** identité, page téléphone.
- **Question :** un texte saisi par un joueur peut-il devenir du code ailleurs ?
- **Pression :** `native_identity_and_script_escaping_share_the_page_decoder`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## Validation et décodage

### REM-57 · Ce que décrit le jeu est borné avant d'atteindre le moteur d'affichage
- **Ce qui arrive :** un contrôle placé à `f32::MAX`, ou un stick de course infime, mettait le bouton du stick à NaN. UIKit refuse un calque à NaN et lève une exception. Le correctif écarte un contrôle centré à plus d'un écran du bord, impose 1 pt de course au moins, et écarte les identifiants au-delà de 31 et les tailles négatives ou infinies.
- **D'où on le sait :** commit df675cb ; `controls.rs:19-20`, `:47-51`, `:360-372` ; `LEARNINGS.md:100` ; `properties.rs:464-471` ; `properties.proptest-regressions`, premier cas.
- **Mécaniques :** affichage, manette tactile.
- **Question :** une description absurde venue du jeu peut-elle produire une valeur non finie dans le rendu natif ?
- **Pression :** `properties.rs:258` et `:471` ; `fuzz/fuzz_targets/phone_session.rs`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-58 · Une longueur absurde échoue tout de suite, sans réserver de mémoire
- **Ce qui arrive :** une trame peut annoncer une liste de 2^64 éléments. Réserver la place arrêterait le processus, ou prendrait des secondes. Chaque liste et chaque texte des deux sens est essayé, suivi de rien ou de 64 Kio.
- **D'où on le sait :** `crates/protocol/tests/properties.rs:104-113`.
- **Mécaniques :** réseau, décodage.
- **Question :** un décodeur peut-il allouer selon une longueur qu'il n'a pas encore vérifiée ?
- **Pression :** `a_huge_length_fails_at_once`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-59 · Un décodeur refuse les octets en trop
- **Ce qui arrive :** une trame suivie d'un suffixe, ou deux messages collés, se lisaient comme le premier. Les décodeurs exigent désormais une trame exacte.
- **D'où on le sait :** `crates/protocol/src/lib.rs:192-194` et `:200-210` ; `CONTRAT.md:14-15` ; `LEARNINGS.md:193-194`.
- **Mécaniques :** réseau, décodage.
- **Question :** un message accepte-t-il des octets après sa fin ?
- **Pression :** `public_decoders_reject_concatenated_or_extended_frames`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-60 · Un désérialiseur dérivé accepte plus que le contrat écrit
- **Ce qui arrive :** un test négatif a montré que la désérialisation automatique acceptait des tableaux positionnels à la place des objets documentés. La frontière exige maintenant des objets, jusqu'à chaque événement haptique.
- **D'où on le sait :** `LEARNINGS.md:202` ; `crates/protocol/src/web.rs:210-219` et `:395-399`.
- **Mécaniques :** décodage, page téléphone.
- **Question :** le décodeur refuse-t-il chaque forme que le contrat ne nomme pas ?
- **Pression :** `what_the_app_sends_reads_back`, `native_commands_keep_legacy_batches_defaults_and_bounds` (`web.rs`).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-61 · Un base64 entrant doit être canonique
- **Ce qui arrive :** l'ancien décodeur acceptait une fin tronquée ou un remplissage incorrect. Le nouveau réencode et compare : seule la forme canonique passe.
- **D'où on le sait :** `LEARNINGS.md:196` ; `web.rs:412-438` et tests `:563-568`.
- **Mécaniques :** décodage.
- **Question :** deux textes différents peuvent-ils donner les mêmes octets ?
- **Pression :** `identities_and_datagrams_are_validated_before_becoming_types`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-62 · Les demandes d'une page sont bornées : elle vient du réseau
- **Ce qui arrive :** une page peut demander n'importe quoi. Bornes retenues : 64 événements haptiques, 5 s chacun, valeurs ramenées dans leur domaine, NaN remplacé par la valeur par défaut ; 8 sockets ; datagrammes de 65 507 octets au plus ; UDP vers l'hôte du jeu seulement ; ports de 1 à 65 535. Un objet mal typé est refusé sans effet partiel.
- **D'où on le sait :** `LEARNINGS.md:99` ; `CONTRAT.md:167-176` ; `web.rs:194-246` ; `WebRemote.swift:143-152`.
- **Mécaniques :** vibration, réseau, page téléphone.
- **Question :** une demande du jeu peut-elle épuiser une ressource de l'appareil ?
- **Pression :** `a_pattern_is_cut_at_64_events`, `native_commands_keep_legacy_batches_defaults_and_bounds`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-63 · Un échantillon de capteur non fini n'atteint pas la page
- **Ce qui arrive :** un échantillon dont une valeur n'est pas finie est écarté avant de devenir un événement. Aucune trace d'un tel échantillon réel : c'est une garde.
- **D'où on le sait :** `web.rs:321-334` ; `CONTRAT.md:176`.
- **Mécaniques :** capteurs.
- **Question :** une valeur non finie venue du système peut-elle entrer dans le jeu ?
- **Pression :** `native_commands_keep_legacy_batches_defaults_and_bounds`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-64 · Une trame illisible est ignorée, sans rien changer
- **Ce qui arrive :** ce qui vient du réseau ne doit pas pouvoir faire planter. Une trame illisible est signalée et ne change rien. Tout ce qui se décode se réencode en octets qui redonnent le même message.
- **D'où on le sait :** `session.rs:49-50` et `:165` ; `LEARNINGS.md:92` ; `properties.rs:211-216` ; `fuzz/fuzz_targets/decode.rs`.
- **Mécaniques :** réseau, décodage.
- **Question :** une trame quelconque peut-elle faire autre chose qu'être lue ou refusée ?
- **Pression :** `any_frame_from_the_game_is_acted_on_or_ignored` ; `any_bytes_are_a_message_or_refused` ; fuzzer `decode`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-65 · Dans une app, une panique Rust tue l'app
- **Ce qui arrive :** UniFFI rattrape la panique et la rend à Swift, dont les appels générés en `try!` arrêtent l'app. Le commentaire du verrou disait qu'une panique laisse des données utilisables : côté Swift, l'app est déjà morte. Le seul rempart est l'absence de panique.
- **D'où on le sait :** leçon telecommande-43 (`rustcalls.rs` d'uniffi-rs) ; `crates/ffi/src/lib.rs:467-469`.
- **Mécaniques :** app iPhone.
- **Question :** un chemin du cœur peut-il paniquer derrière une frontière qui ne sait pas s'en remettre ?
- **Pression :** les lints (`unwrap`, `expect`, `panic`, index) et le fuzzer `phone_session`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-66 · Un type de taille fixe perd sa taille en traversant une frontière générée
- **Ce qui arrive :** l'identité, 16 octets, traverse UniFFI comme une liste. Le constructeur de session complète ou tronque en silence ; la fonction qui prépare la page, elle, refuse une autre longueur. Le contrôle doit se faire à chaque entrée.
- **D'où on le sait :** `crates/ffi/src/lib.rs:305-318` ; `crates/ffi/src/web.rs:171-173` ; `GameConnection.swift:265-275`.
- **Mécaniques :** identité, app iPhone.
- **Question :** une valeur de taille fixe garde-t-elle sa taille vérifiée de l'autre côté d'une liaison générée ?
- **Pression :** `web_bootstrap` refusé à 15 octets, dans `native_identity_and_script_escaping_share_the_page_decoder` ; rien pour la session.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## La connexion vue par le joueur

### REM-67 · Le refus du réseau local se lit sur le chemin d'une NWConnection
- **Ce qui arrive :** quand le joueur refuse le réseau local, `URLSession` échoue sans dire pourquoi. Seul le chemin d'une `NWConnection` donne la raison `localNetworkDenied`. La première opération peut échouer avant la réponse à l'alerte. Le simulateur n'applique pas cette permission.
- **D'où on le sait :** T5 (sonde H4) ; commit 0e7054d ; `LocalNetwork.swift:4-37` ; `GameConnection.swift:193-202` ; leçon telecommande-16 (TN3179 d'Apple).
- **Mécaniques :** rejoindre, réseau.
- **Question :** l'appareil sait-il que c'est la permission qui manque, et pas l'hôte ?
- **Pression :** `bench/tests/shell.rs:239`, avec un faux juge de permission ; le vrai refus seulement sur un iPhone (S6).
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-68 · La vue web échappe à la permission du réseau local
- **Ce qui arrive :** le trafic de `WKWebView` n'est pas soumis à cette permission. Refusée, la page du jeu marcherait encore, alors que le lien de l'app ne le peut jamais : un état incohérent, que rien ne signale.
- **D'où on le sait :** leçon telecommande-16 (TN3179) ; `GameConnection.swift:193-194` ; T5, « cas ».
- **Mécaniques :** rejoindre, app iPhone.
- **Question :** deux chemins réseau du même appareil peuvent-ils obéir à des permissions différentes ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-69 · Un seul message à la fois, qui nomme la vraie cause et ce qu'il faut faire
- **Ce qui arrive :** tout échec se lisait « Connexion perdue », même sans avoir jamais atteint le jeu. Deux messages se superposaient et devenaient illisibles. Désormais un seul message, choisi en un seul endroit : jamais atteint, perdu, page qui ne répond pas, ou réseau local refusé avec le chemin exact dans Réglages.
- **D'où on le sait :** T5 ; commit 0e7054d ; `GameConnection.swift:238-257` ; leçon telecommande-15 ; S6 de `CERTIFICATION.md`.
- **Mécaniques :** rejoindre, reconnexion.
- **Question :** le joueur lit-il une seule phrase, qui dit ce qui se passe et quoi faire ?
- **Pression :** `bench/tests/shell.rs:239` et `:253`. La superposition est réglée par construction ; aucun test ne lit l'écran.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-70 · L'écran reste allumé pendant la partie, et seulement pendant
- **Ce qui arrive :** la veille était coupée à l'arrivée du jeu et jamais rendue. Sur l'accueil, scanner ouvert, le téléphone ne s'éteignait plus. Moonlight a corrigé la même faute ; Apple demande de rendre le réglage dès que le besoin cesse.
- **D'où on le sait :** T15 (sonde H6) ; commit 04716b5 ; `GameScreen.swift:46-53` ; leçons telecommande-31 et 32.
- **Mécaniques :** capacités, app iPhone.
- **Question :** chaque réglage pris au système lui est-il rendu à la sortie ?
- **Pression :** `bench/tests/shell.rs:561`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-71 · Sans la bonne clé, iOS ferme le Wi-Fi après 30 minutes
- **Ce qui arrive :** Apple annonce qu'iOS ferme la connexion Wi-Fi active après 30 minutes sans `UIRequiresPersistentWiFi`. Une soirée dure plus longtemps. L'app ne la déclare pas ; Moonlight non plus. À mesurer avant de corriger.
- **D'où on le sait :** T17 ; leçon telecommande-34 (doc Apple) ; `ios/Info.plist` (clé absente).
- **Mécaniques :** réseau, app iPhone.
- **Question :** la latence monte-t-elle par à-coups après 30 minutes de partie ?
- **Pression :** aucune ; `REPETITION.md`, « Wi-Fi qui s'endort ».
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-72 · Le trafic d'entrée ne dit pas au Wi-Fi qu'il est pressé
- **Ce qui arrive :** Moonlight marque son trafic pour la qualité de service. L'app passe par une session qui n'annonce aucun type de service. L'effet n'a pas été mesuré.
- **D'où on le sait :** T17 ; leçon telecommande-35 (commit c0e3dc6 de moonlight-common-c).
- **Mécaniques :** réseau.
- **Question :** marquer le trafic d'entrée change-t-il sa latence sur le Wi-Fi d'une soirée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-73 · Plusieurs portes pour rejoindre
- **Ce qui arrive :** scanné avec l'appareil photo, le QR ouvre la page web dans Safari. Cette page peut offrir un lien `telecommande://`, qui ouvre l'app sur le jeu, après une question d'iOS. Sans caméra, comme dans le simulateur, l'adresse se tape. Dans l'app, le scanner rejoint au premier code qui se lit comme un jeu, sans confirmation.
- **D'où on le sait :** `CONTRAT.md:26-27` ; `README.md:29-34` ; `JoinView.swift:4-6` et `:73-120` ; `crates/core/src/join.rs:1-8` ; T14, « suppose ».
- **Mécaniques :** rejoindre.
- **Question :** un joueur avec ou sans l'app, avec ou sans caméra, arrive-t-il dans la partie en moins de 30 s ?
- **Pression :** `properties.rs:415` et `:451` ; S7 de `CERTIFICATION.md`, geste humain.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-74 · Si l'adresse de l'hôte change, il faut rescanner
- **Ce qui arrive :** Moonlight découvre ses PC par Bonjour. L'app ne découvre rien : le QR donne l'adresse. Si l'IP de l'ordinateur de la soirée change, le joueur doit quitter et rescanner.
- **D'où on le sait :** leçon telecommande-40.
- **Mécaniques :** rejoindre.
- **Question :** que se passe-t-il pour les appareils quand l'adresse de l'ordinateur de la soirée change en pleine soirée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-75 · Une adresse relative avec un chemin remplace la requête de sa base
- **Ce qui arrive :** selon RFC 3986, résoudre `/room` contre `http://hôte/room?code=AB12` donne `/room`, sans le code. La requête du QR atteignait le socket, mais pas la page : la page perdait son code de salle.
- **D'où on le sait :** T16 (sonde H13) ; commit fccbf76 ; `GameConnection.swift:158-170` ; leçon telecommande-33 (RFC 3986 §5.2.2).
- **Mécaniques :** rejoindre.
- **Question :** ce que porte le QR atteint-il chaque endroit qui en a besoin, après chaque navigation ?
- **Pression :** `bench/tests/shell.rs:573`.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-76 · L'identité survit à la reconnexion et au relancement, sans compte
- **Ce qui arrive :** l'app tire 16 octets au hasard une fois, les garde, et les renvoie dans chaque `Hello`, comme le jeton anonyme de Sidequest. Le prénom est saisi une fois, prérempli, coupé à 24 caractères.
- **D'où on le sait :** `LEARNINGS.md:90` ; `GameConnection.swift:259-275` ; `JoinView.swift:29-34`.
- **Mécaniques :** identité.
- **Question :** un appareil qui revient, même après avoir été fermé de force, retrouve-t-il la même identité ?
- **Pression :** `hello_again_after_each_reconnection_with_the_same_player` ; essai humain dans `REPETITION.md:73`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-77 · Une version différente est refusée, avec une raison que le joueur lit
- **Ce qui arrive :** `Hello` porte la version du protocole. Si elle diffère, le jeu répond un refus motivé, et l'app l'affiche au lieu de décoder de travers.
- **D'où on le sait :** `CONTRAT.md:35` ; `crates/protocol/src/lib.rs:20-21` et `:51-52` ; `GameScreen.swift:94-109`.
- **Mécaniques :** app iPhone, versions.
- **Question :** un appareil d'une autre version peut-il décoder de travers sans le savoir ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** impossible par la forme. « Les types de bout en bout » : la version se calcule depuis les sources et une page restée en cache est refusée ; « Relier les appareils » : le module arrive par la connexion, un appareil a toujours exactement sa version.
- **Étage :** commun

### REM-78 · Quitter demande confirmation, depuis un coin hors des contrôles
- **Ce qui arrive :** un pouce qui frôle la croix de sortie en jeu ne doit pas faire quitter. La croix ouvre une question. Elle est en haut à gauche, dans la marge de l'encoche.
- **D'où on le sait :** `GameScreen.swift:111-128` ; `README.md:51`.
- **Mécaniques :** app iPhone, page téléphone.
- **Question :** un geste involontaire peut-il faire sortir un joueur de la partie ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

## Ce que la plateforme impose

### REM-79 · Safari sur iPhone n'offre ni vibration, ni capteurs sans question, ni orientation, ni UDP
- **Ce qui arrive :** Safari n'a aucune vibration, redemande la permission des capteurs à chaque visite, ne garde ni le plein écran ni l'orientation, et ne parle pas UDP. C'est la raison d'être de l'app ; un jeu doit pourtant rester jouable dans Safari.
- **D'où on le sait :** `LEARNINGS.md:95` (WebKit) ; `CONTRAT.md:79`.
- **Mécaniques :** vibration, capteurs, orientation, capacités.
- **Question :** le jeu reste-t-il jouable quand ces capacités manquent ?
- **Pression :** S7 de `CERTIFICATION.md`, un iPhone en Safari.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-80 · Dans l'app, les capteurs ne demandent pas de permission, pour l'instant
- **Ce qui arrive :** l'app lit le mouvement sans question au joueur. iOS 27.2 ajoute un statut d'autorisation : l'app déclare déjà pourquoi elle s'en sert, au cas où.
- **D'où on le sait :** `ios/Info.plist:32-34` ; `CONTRAT.md:90`.
- **Mécaniques :** capteurs.
- **Question :** l'accès aux capteurs peut-il devenir une question au joueur, et l'app y est-elle prête ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-81 · Sur iOS, seul WebKit exécute du code téléchargé à pleine vitesse
- **Ce qui arrive :** c'est pourquoi la voie libre de l'app passe par une vue web. Une app ne peut pas compiler du code à la volée.
- **D'où on le sait :** `LEARNINGS.md:86`.
- **Mécaniques :** app iPhone.
- **Question :** comment une app fait-elle tourner le module d'un jeu sans compiler à la volée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-82 · Donner des fonctions natives à du contenu web demande l'accord d'Apple
- **Ce qui arrive :** la règle 4.7.2 de l'App Store encadre l'accès de contenu web à des fonctions natives. Écart assumé sur TestFlight, à revoir avant une publication publique.
- **D'où on le sait :** `LEARNINGS.md:101` (App Review Guidelines 4.7.2).
- **Mécaniques :** app iPhone.
- **Question :** l'app peut-elle être publiée telle quelle ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-83 · Le HTTP local demande une exception de sécurité déclarée
- **Ce qui arrive :** les jeux tournent en `http` sur une machine du réseau local. L'app déclare l'exception de réseau local d'App Transport Security, et l'usage du réseau local.
- **D'où on le sait :** `ios/Info.plist:24-31` ; leçon telecommande-39 (Moonlight ff5e8d1).
- **Mécaniques :** app iPhone, réseau.
- **Question :** l'app déclare-t-elle chaque usage réseau qu'iOS exige ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-84 · Les reconnaisseurs de gestes retardent les touchers près des bords
- **Ce qui arrive :** même avec les gestes système différés, les reconnaisseurs retardent les touchers près des bords. L'app lit les touchers bruts d'une vue et diffère les gestes système partout. iOS retient aussi les touchers près du bord haut : la toile y garde ses outils à distance. À mesurer sur un vrai iPhone.
- **D'où on le sait :** `LEARNINGS.md:72` (Delta 39cc979, Apple Forums 654645) ; `TouchSurface.swift:4-5` ; `GameScreen.swift:43-45` ; `presets.rs:330-331`.
- **Mécaniques :** manette tactile, entrées.
- **Question :** un toucher près d'un bord arrive-t-il aussi vite qu'au centre ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-85 · SwiftUI se limite à 60 Hz sur un écran ProMotion sans une clé
- **Ce qui arrive :** sans `CADisableMinimumFrameDurationOnPhone`, l'app dessine à 60 Hz sur un iPhone à 120 Hz.
- **D'où on le sait :** `LEARNINGS.md:74` (source secondaire) ; `ios/Info.plist:5-7`.
- **Mécaniques :** affichage, app iPhone.
- **Question :** l'app dessine-t-elle à la fréquence de l'écran ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-86 · Le mouvement se lit une fois par image
- **Ce qui arrive :** Apple recommande pour les jeux de lire le mouvement fusionné à chaque image plutôt que par rappels. L'app le demande à 100 Hz et ignore un échantillon dont l'horodatage n'a pas changé.
- **D'où on le sait :** `RemoteModel.swift:144-173`.
- **Mécaniques :** capteurs, volant, pointeur.
- **Question :** un même échantillon peut-il être compté deux fois ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-87 · Le moteur haptique se réinitialise après une interruption
- **Ce qui arrive :** une interruption audio ou un passage en arrière-plan réinitialise le moteur haptique. L'app l'oublie et le recrée au motif suivant. Certains appareils n'ont pas de vibration : l'app le vérifie avant.
- **D'où on le sait :** `WebRemote.swift:247-278`.
- **Mécaniques :** vibration.
- **Question :** une vibration demandée après une interruption joue-t-elle encore ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-88 · Le simulateur ne dit pas tout
- **Ce qui arrive :** le simulateur n'a ni capteurs, ni caméra, n'applique pas la permission du réseau local, partage la carte réseau du Mac et rend autrement sous la souris. Le crochet de WebKit qui tue une page ne recharge jamais seul, contrairement à une vraie mort. Il ne dit rien de la mémoire d'un vrai iPhone.
- **D'où on le sait :** `LEARNINGS.md:55`, `:140` et `:214` ; T5 et T17, « suppose » ; `README.md:30`.
- **Mécaniques :** app iPhone.
- **Question :** quelle part d'une garantie éprouvée sur simulateur vaut sur un vrai téléphone ?
- **Pression :** `REPETITION.md`, « L'app Télécommande, sur un vrai iPhone ».
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

## Perception

### REM-89 · Un appui en retard de moins d'environ 60 ms passe inaperçu
- **Ce qui arrive :** au-dessous de ce seuil, le retard d'un appui ne se remarque pas.
- **D'où on le sait :** `LEARNINGS.md:104` (Jota et al., CHI 2013).
- **Mécaniques :** entrées, boutons tactiles.
- **Question :** le budget de latence d'un appui part-il de ce seuil ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-90 · Ce qui suit le doigt se dessine dans la même image
- **Ce qui arrive :** un glisser se perçoit dès quelques millisecondes de retard. Le bouton du stick dessiné sur le téléphone doit donc suivre le doigt dans la même image ; le budget de 80 ms ne vaut que jusqu'à la TV.
- **D'où on le sait :** `LEARNINGS.md:105-107` (Ng et al., UIST 2012).
- **Mécaniques :** stick tactile, dessin, prédiction.
- **Question :** ce qui suit le doigt sur l'appareil attend-il un aller-retour ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## Sensation de la manette tactile

### REM-91 · Le gate d'un stick N64 est un octogone
- **Ce qui arrive :** un vrai stick va jusqu'à environ 85 sur un axe et (70, 70) dans un cran diagonal. Avec un cercle, les diagonales lisent 60, et les jeux qui lisent chaque axe à part (GoldenEye, Ocarina of Time) perdent environ 20 % de vitesse en diagonale.
- **D'où on le sait :** `LEARNINGS.md:18` (ROM de test mimi, manuel Nintendo §3-5, décompilations, Dolphin db75509ec5) ; `controls.rs:44-45` ; `feel.rs:246` et `:278`.
- **Mécaniques :** stick tactile.
- **Question :** le stick virtuel imite-t-il la forme du gate qu'attend le jeu ?
- **Pression :** `stick_reads_up_as_positive_and_stops_in_the_n64_gate`, `diagonal_rim_is_further_than_the_axis_rim`.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-92 · La zone morte du stick ne filtre que le tremblement
- **Ce qui arrive :** les jeux ont leur propre zone morte : 7 dans SM64, OoT et MM, 5 dans GoldenEye, un carré de 12 dans MK64. En ajouter une grosse les empile (PPSSPP #15022). L'app garde 3 %, remis à l'échelle pour éviter un saut.
- **D'où on le sait :** `LEARNINGS.md:19` ; `controls.rs:30-32` et `:660-678` ; `feel.rs:297`.
- **Mécaniques :** stick tactile.
- **Question :** la zone morte de l'appareil s'empile-t-elle avec celle du jeu ?
- **Pression :** `stick_dead_zone_only_filters_jitter`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-93 · Personne ne se plaint de la courbe d'un stick, seulement de son amplitude
- **Ce qui arrive :** sur cinq émulateurs, aucune plainte sur la réponse linéaire. L'app la garde.
- **D'où on le sait :** `LEARNINGS.md:20`.
- **Mécaniques :** stick tactile.
- **Question :** la courbe de réponse vaut-elle qu'on s'en écarte ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-94 · Un stick tactile a besoin d'assez de course
- **Ce qui arrive :** trop peu de course est la plainte numéro un : Delta, avec environ 15 pt (#380, #527). Bonne valeur : 12 à 14 % de la hauteur de l'écran. L'app donne 50 pt pour la pleine déflexion.
- **D'où on le sait :** `LEARNINGS.md:21` (Delta, Mupen64Plus-AE, Dolphin) ; `presets.rs:50`.
- **Mécaniques :** stick tactile.
- **Question :** la course du stick laisse-t-elle un pouce doser sa poussée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-95 · Le stick se place sous le pouce, pas à une place fixe
- **Ce qui arrive :** un stick fixe fait dériver le pouce quand le joueur regarde la TV (Baldauf et al., ACM TOMM 2015). Apple, la Touch Adaptation Kit de Microsoft et Dolphin placent le stick là où le pouce se pose.
- **D'où on le sait :** `LEARNINGS.md:22` (HIG Apple, TAK `relative`, Baldauf 2015, Dolphin).
- **Mécaniques :** stick tactile.
- **Question :** le stick se retrouve-t-il sans regarder le téléphone ?
- **Pression :** `stick_centers_where_the_thumb_lands_near_its_rest` (`feel.rs:197`).
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-96 · Toucher autour du stick le pousse déjà dans cette direction
- **Ce qui arrive :** un pouce posé loin du repos doit pousser tout de suite, pas devenir un nouveau centre (PPSSPP #14410, zone de recentrage de RetroArch). Règle : à moins de 0,6 course du repos, le point posé devient le centre ; plus loin, le centre s'arrête sur ce cercle.
- **D'où on le sait :** `LEARNINGS.md:23` ; `controls.rs:33-35` et `:275-286` ; `feel.rs:220`.
- **Mécaniques :** stick tactile.
- **Question :** un pouce posé loin du centre pousse-t-il déjà le stick ?
- **Pression :** `a_thumb_landing_far_from_the_stick_already_pushes_it`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-97 · Toute une moitié d'écran attrape le stick
- **Ce qui arrive :** un cercle autour du dessin rate le pouce qui ne regarde pas. Le Touch Controller d'Apple donne toute une moitié d'écran au stick. L'app fait de même, hors croix et boutons.
- **D'où on le sait :** `LEARNINGS.md:24` (Touch Controller, WWDC26) ; `controls.rs:431` et `:696` ; `feel.rs:229`.
- **Mécaniques :** stick tactile.
- **Question :** quelle surface attrape le stick ?
- **Pression :** `the_whole_left_half_takes_the_stick`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-98 · Le recentrage reste invisible
- **Ce qui arrive :** au premier essai, le dessin du stick qui sautait sous le pouce donnait l'impression d'un bug. Le socle reste fixe ; seul le bouton bouge.
- **D'où on le sait :** `LEARNINGS.md:25` (retour de l'hôte) ; `controls.rs:636-658`.
- **Mécaniques :** stick tactile.
- **Question :** ce qui bouge à l'écran correspond-il à ce que le joueur croit faire ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-99 · Le doigt qui a pris un contrôle le garde jusqu'à ce qu'il se lève
- **Ce qui arrive :** le pouce qui a pris le stick le garde partout et n'appuie jamais sur un bouton. Un second pouce posé du côté du stick est ignoré.
- **D'où on le sait :** `LEARNINGS.md:26` (PPSSPP 4ec1a8d12, Dolphin, Mupen64Plus-AE, RetroArch) ; `controls.rs:5-6` ; `feel.rs:330` et `:339`.
- **Mécaniques :** stick tactile, manette tactile.
- **Question :** un doigt qui déborde de son contrôle peut-il en actionner un autre ?
- **Pression :** `a_thumb_that_leaves_the_stick_keeps_steering_and_presses_nothing`, `a_second_thumb_on_the_stick_side_is_ignored`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-100 · Le centre ne suit pas un doigt qui dépasse le bord
- **Ce qui arrive :** aucun des cinq émulateurs ne fait suivre le centre. Le stick reste bloqué au gate.
- **D'où on le sait :** `LEARNINGS.md:27`.
- **Mécaniques :** stick tactile.
- **Question :** que fait le stick quand le pouce va au-delà de son bord ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-101 · Une seule vibration au bord du stick, réarmée plus bas
- **Ce qui arrive :** vibrer pendant que le stick bouge déplaît (Dolphin PR #13220). L'app vibre une fois en touchant le gate, à 97 %, et se réarme sous 85 %.
- **D'où on le sait :** `LEARNINGS.md:28` ; `controls.rs:36-38` et `:736-749` ; `feel.rs:312`.
- **Mécaniques :** stick tactile, vibration.
- **Question :** une vibration continue accompagne-t-elle un geste continu ?
- **Pression :** `rim_haptic_fires_once_until_the_stick_comes_back`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-102 · Un léger aimantage vers les huit crans, à essayer
- **Ce qui arrive :** le vrai stick N64 a huit crans. Un aimantage léger reste à essayer sur un vrai iPhone.
- **D'où on le sait :** `LEARNINGS.md:29` (mimi, Mupen64Plus-AE 0300f244).
- **Mécaniques :** stick tactile.
- **Question :** un aimantage aide-t-il à trouver les diagonales sans regarder ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-103 · Glisser d'un bouton à l'autre appuie le nouveau et relâche l'ancien
- **Ce qui arrive :** PPSSPP, Mupen64Plus-AE, Delta et RetroArch le font tous. Un doigt glissé n'a pas à se lever pour changer de bouton.
- **D'où on le sait :** `LEARNINGS.md:35` ; `controls.rs:11-14` et `:458-480`.
- **Mécaniques :** boutons tactiles.
- **Question :** un doigt qui glisse vers un autre bouton l'appuie-t-il ?
- **Pression :** `sliding_from_b_to_a_rolls_through_both` (`feel.rs:138`).
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-104 · Un pouce entre deux boutons voisins voulus ensemble presse les deux
- **Ce qui arrive :** sur la N64, un pouce entre A, B et Z presse les deux voisins, exprès. L'app les place en triangle, à 20 pt l'un de l'autre. Les boutons C, eux, ne se combinent jamais.
- **D'où on le sait :** `LEARNINGS.md:36` (skin N64 de Delta, RetroArch #14565) ; `layout.rs:74-76` ; `feel.rs:119` et `:127`.
- **Mécaniques :** boutons tactiles.
- **Question :** quels boutons peuvent s'appuyer ensemble d'un seul pouce, et lesquels jamais ?
- **Pression :** `one_thumb_between_a_and_b_presses_both`, `one_thumb_between_two_c_buttons_presses_one`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-105 · Rouler le pouce d'un bouton à l'autre ne relâche pas le premier trop tôt
- **Ce qui arrive :** dans la zone commune, les deux restent appuyés. Sinon le premier lâche avant que le second ne prenne.
- **D'où on le sait :** `LEARNINGS.md:37` (Delta #445, PPSSPP #14490) ; `feel.rs:138`.
- **Mécaniques :** boutons tactiles.
- **Question :** un roulement de pouce laisse-t-il un instant sans aucun bouton ?
- **Pression :** `sliding_from_b_to_a_rolls_through_both`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-106 · Un bouton tenu garde une marge plus large
- **Ce qui arrive :** sans hystérésis, un bouton clignote sur son bord (RetroArch #15534). L'app donne 15 pt pour un nouvel appui, 30 pt pour un bouton déjà tenu.
- **D'où on le sait :** `LEARNINGS.md:38` ; `controls.rs:29` et `:462` ; `feel.rs:163`.
- **Mécaniques :** boutons tactiles.
- **Question :** un doigt immobile sur le bord d'un bouton le fait-il clignoter ?
- **Pression :** `a_held_button_keeps_a_wider_margin`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-107 · Sur un téléphone, seuls les deux pouces sont libres
- **Ce qui arrive :** sur la vraie manette, Z se tient avec l'index pendant que le pouce gauche dirige. Mupen64Plus-AE met Z à gauche, ce qui bloque le stick. L'app le met à droite avec A et B.
- **D'où on le sait :** `LEARNINGS.md:39` (TAK, Mupen64Plus-AE, Delta) ; `presets.rs` (module `n64`).
- **Mécaniques :** manette tactile.
- **Question :** un geste qui demande trois doigts à la fois est-il possible avec deux pouces ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-108 · Les tailles se comptent en points, depuis la zone sûre, et 44 pt au moins
- **Ce qui arrive :** un pouce a la même taille sur tous les iPhone. Au moins 44 pt pour un contrôle fréquent (Apple). Les tailles se comptent depuis les bords de la zone sûre, pas en part de l'écran. Contrôlé sur quatre géométries d'iPhone : rien hors de la zone, rien qui se chevauche, le stick au repos dégagé.
- **D'où on le sait :** `LEARNINGS.md:40` (HIG, Touch Controller) ; `crates/core/src/lib.rs:160-276` ; `feel.rs:50`.
- **Mécaniques :** manette tactile, page téléphone.
- **Question :** chaque contrôle tient-il, sans chevauchement, sur chaque téléphone visé ?
- **Pression :** `every_preset_fits_every_phone`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-109 · Un bouton appuyé est caché sous le pouce : un halo plus large le montre
- **Ce qui arrive :** sous le pouce, on ne voit pas son propre appui. Un halo de la couleur du bouton dépasse autour.
- **D'où on le sait :** `LEARNINGS.md:41` (HIG, PPSSPP a9a45481c) ; `ControlViews.swift:40-44`.
- **Mécaniques :** boutons tactiles.
- **Question :** le joueur voit-il qu'il appuie, quand son doigt cache le bouton ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-110 · Rien d'important en bas au centre : le geste d'accueil y vit
- **Ce qui arrive :** le bas de l'écran, au centre, appartient au geste « accueil » d'iOS. Start se place en haut au centre, comme sur la vraie manette.
- **D'où on le sait :** `LEARNINGS.md:42` (n64brew, PPSSPP f42f7ac9d).
- **Mécaniques :** manette tactile, page téléphone.
- **Question :** un contrôle se trouve-t-il là où le système prend les gestes pour lui ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-111 · Start et Select : petits, au centre, sans marge
- **Ce qui arrive :** on ne doit pas les toucher par accident. Ils n'ont aucune zone élargie, et 28 pt au moins au lieu de 44.
- **D'où on le sait :** `LEARNINGS.md:122` (Delta, RetroArch) ; `layout.rs:77-79` ; `feel.rs:179`.
- **Mécaniques :** boutons tactiles.
- **Question :** un bouton rare et lourd de conséquences peut-il partir par accident ?
- **Pression :** `minor_buttons_take_no_touch_beyond_their_drawing`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-112 · A et B sur une diagonale de 40 à 45°, assez proches pour un appui à deux
- **Ce qui arrive :** Delta place A et B à 45° sur la NES et vers 39° sur la Game Boy, B en bas à gauche. L'app prend 40° pour le pad, 45° pour la croix, 16 à 20 pt d'écart.
- **D'où on le sait :** `LEARNINGS.md:121` ; `presets.rs:100-101` et `:151-152`.
- **Mécaniques :** boutons tactiles.
- **Question :** la disposition des boutons suit-elle le pouce qui les atteint ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-113 · Peu de boutons, gros, sous les pouces, pour jouer sans regarder
- **Ce qui arrive :** les joueurs regardent la TV. AirConsole garde peu de boutons, gros, aux coins ou sous les pouces. L'app s'en tient à deux à quatre boutons de jeu par modèle.
- **D'où on le sait :** `LEARNINGS.md:120` (AirConsole).
- **Mécaniques :** manette tactile, page téléphone.
- **Question :** la page se joue-t-elle sans la regarder ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-114 · Un contrôle se place depuis le coin le plus proche du pouce qui l'utilise
- **Ce qui arrive :** chaque contrôle se place à un décalage en points depuis l'un des neuf repères de l'écran utile, comme le Touch Controller d'Apple. Un téléphone plus grand n'élargit que le milieu.
- **D'où on le sait :** `LEARNINGS.md:85` ; `layout.rs:110-113`.
- **Mécaniques :** manette tactile, page téléphone.
- **Question :** sur un téléphone plus grand, les contrôles restent-ils sous les pouces ?
- **Pression :** `every_preset_fits_every_phone`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-115 · Un modèle prêt à l'emploi est une composition comme une autre
- **Ce qui arrive :** les sept modèles de l'app sont des compositions ordinaires, lisibles comme exemples, et adaptables (renommer, recolorer, ajouter, retirer).
- **D'où on le sait :** `LEARNINGS.md:85` ; `presets.rs:1-4` ; `CONTRAT.md:43-68`.
- **Mécaniques :** manette tactile.
- **Question :** un exemple fourni par le commun utilise-t-il seulement ce qu'un jeu peut utiliser ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-116 · L'appui s'affiche tout de suite, sans animation
- **Ce qui arrive :** aucun émulateur n'anime le passage à l'état appuyé, ni n'échange de la latence contre un effet.
- **D'où on le sait :** `LEARNINGS.md:50` (PPSSPP `GamepadEmu.cpp`, Delta 1ef4247b2 et 633dfa869, Dolphin, RetroArch) ; `ControlViews.swift:58-60`.
- **Mécaniques :** boutons tactiles.
- **Question :** un effet visuel retarde-t-il l'appui ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-117 · Un flash plus court qu'environ 100 ms se lit comme un scintillement
- **Ce qui arrive :** filmé dans le simulateur, une tape rapide s'affichait une seule image, 8 ms, voire pas du tout, puis tout disparaissait d'un coup. En dessous d'environ 100 ms, l'œil cumule la luminosité (loi de Bloch). Un bouton reste donc dessiné appuyé au moins 60 ms ; le jeu reçoit le relâchement à l'instant.
- **D'où on le sait :** `LEARNINGS.md:46` et `:51` ; commit 025eb7e ; `RemoteModel.swift:13-21` et `:116-127`.
- **Mécaniques :** boutons tactiles, affichage.
- **Question :** un appui très court se voit-il, sans retarder ce que reçoit le jeu ?
- **Pression :** aucune ; hors des tests (`LEARNINGS.md:176`).
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-118 · Le relâchement s'efface en fondu
- **Ce qui arrive :** les kits d'interface ajoutent un fondu ou une durée minimale : Android au moins 225 ms puis 150 ms de fondu, Unity 100 ms, Chromium sur iOS 70 ms. L'app fait revenir le bouton en 60 ms et effacer le halo en 120 ms.
- **D'où on le sait :** `LEARNINGS.md:52` (Android `RippleForeground`, Unity uGUI, Chromium, Flutter) ; `ControlViews.swift:61-62`.
- **Mécaniques :** boutons tactiles.
- **Question :** la fin d'un appui se lit-elle sans à-coup ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-119 · Un gros changement à chaque appui distrait
- **Ce qui arrive :** PPSSPP a réduit l'agrandissement d'un appui de ×2 à ×1,5. L'app réduit le halo à 2,4 fois le rayon et 40 % d'opacité.
- **D'où on le sait :** `LEARNINGS.md:53` (PPSSPP #12987, a9a45481c).
- **Mécaniques :** boutons tactiles.
- **Question :** le retour visuel d'un appui attire-t-il l'œil loin de la TV ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-120 · Ne pas changer un rayon d'ombre ou de flou à chaque appui
- **Ce qui arrive :** ombres et flous sont des passes de rendu supplémentaires ; changer leur rayon à chaque appui coûte. L'app garde un rayon fixe et ne change que le décalage et l'opacité, et n'a plus de flou sur le halo de L et R.
- **D'où on le sait :** `LEARNINGS.md:54` (doc Apple, ProMotion, `shadowPath`) ; `ControlViews.swift:108-110`.
- **Mécaniques :** affichage.
- **Question :** un appui déclenche-t-il un rendu coûteux ?
- **Pression :** aucune ; à vérifier avec Instruments sur un vrai iPhone.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-121 · Sur une croix, les directions droites l'emportent
- **Ce qui arrive :** un pouce qui vise tout droit ne doit pas lire une diagonale. L'app donne 50° aux axes et 40° aux diagonales, avec une petite zone morte au centre.
- **D'où on le sait :** `LEARNINGS.md:61` (RetroArch, sensibilité diagonale de 80 % ; Delta, Dolphin, PPSSPP) ; `controls.rs:39-43` et `:326` ; `feel.rs:349`.
- **Mécaniques :** croix.
- **Question :** un pouce qui vise un axe obtient-il cet axe ?
- **Pression :** `dpad_favors_straight_directions`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-122 · La direction d'une croix reste tant que le doigt ne se lève pas
- **Ce qui arrive :** même hors de la croix, le doigt garde la croix (« Sticky D-Pad » de PPSSPP).
- **D'où on le sait :** `LEARNINGS.md:62` (PPSSPP ce4705586) ; `controls.rs:15-16`.
- **Mécaniques :** croix.
- **Question :** un pouce qui déborde de la croix la lâche-t-il ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-123 · Un mode quatre directions pour les menus
- **Ce qui arrive :** dans un menu, une diagonale gêne. La Touch Adaptation Kit offre un mode exclusif. À faire quand un jeu le demandera.
- **D'où on le sait :** `LEARNINGS.md:63` (TAK `exclusive`).
- **Mécaniques :** croix, menus.
- **Question :** le jeu a-t-il besoin d'exclure les diagonales ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-124 · Une croix de 130 à 150 pt, centrée vers 100 pt du bord
- **Ce qui arrive :** ce sont les mesures de Delta. L'app prend 150 pt, centre à 110 pt.
- **D'où on le sait :** `LEARNINGS.md:123` ; `presets.rs:150`.
- **Mécaniques :** croix.
- **Question :** la croix tient-elle sous le pouce sans le tendre ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-125 · Le buzzer réagit au contact, pas au relâchement
- **Ce qui arrive :** un buzzer qui attend le relâchement perd la course. Tout l'écran est le bouton.
- **D'où on le sait :** `LEARNINGS.md:125` (Delta, `touchesBegan`) ; `feel.rs:105`.
- **Mécaniques :** buzzer.
- **Question :** l'instant qui compte est-il celui où le doigt touche ?
- **Pression :** `the_buzzer_answers_anywhere`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-126 · La lumière du buzzer annonce la décision de l'hôte, pas l'appui
- **Ce qui arrive :** le joueur voit « appuyé » tout de suite, puis « premier » ou « trop tard » quand le jeu le dit (Buzz!). L'app teinte le fond sur ordre du jeu. Les états Attente, Armé, Premier, Trop tard restent à faire.
- **D'où on le sait :** `LEARNINGS.md:126` (pilote `hid-sony` de Linux, `buzz_set_leds`) ; `crates/protocol/src/lib.rs:49` et `:65-70`.
- **Mécaniques :** buzzer.
- **Question :** le téléphone annonce-t-il un résultat avant que la partie qui fait foi l'ait décidé ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-127 · Buzzer trop tôt bloque un quart de seconde
- **Ce qui arrive :** sans pénalité, les joueurs martèlent. La règle de *Jeopardy!* de 1985 bloque environ 0,25 s.
- **D'où on le sait :** `LEARNINGS.md:127`.
- **Mécaniques :** buzzer.
- **Question :** marteler le buzzer donne-t-il un avantage ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-128 · Vibration : impact léger à l'appui, tic au relâchement
- **Ce qui arrive :** un impact moyen est « trop fort, flou ». Mieux : impact léger à l'appui, tic de sélection au relâchement.
- **D'où on le sait :** `LEARNINGS.md:69` (Delta #57, MAME4iOS 2d449ec) ; `RemoteModel.swift:175-215`.
- **Mécaniques :** vibration, boutons tactiles.
- **Question :** la vibration d'un appui se sent-elle nette ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-129 · Un générateur de vibration se prépare, et se reprépare après chaque usage
- **Ce qui arrive :** la préparation dure quelques secondes, et chaque vibration la consomme. L'app prépare quand un doigt se pose, et après chaque vibration.
- **D'où on le sait :** `LEARNINGS.md:70` (Delta ac0d42b, doc Apple de `prepare()`) ; `RemoteModel.swift:177-195`.
- **Mécaniques :** vibration.
- **Question :** la première vibration d'un geste part-elle en retard ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-130 · On doit pouvoir couper les vibrations
- **Ce qui arrive :** Apple le demande. L'app ne l'offre pas encore.
- **D'où on le sait :** `LEARNINGS.md:71` (HIG *Playing haptics*).
- **Mécaniques :** vibration.
- **Question :** un joueur peut-il couper les vibrations sans perdre le jeu ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## Volant et pointeur

### REM-131 · Le volant lit la gravité fusionnée par le système
- **Ce qui arrive :** ni l'accéléromètre brut, ni des angles d'Euler : la gravité que le système fusionne depuis l'accéléromètre et le gyroscope.
- **D'où on le sait :** `LEARNINGS.md:134` (PPSSPP f84e63f7, doc CoreMotion) ; `wheel.rs:4-7`.
- **Mécaniques :** volant.
- **Question :** l'angle du volant tremble-t-il avec les secousses de la main ?
- **Pression :** `level_goes_straight_and_25_degrees_is_full_lock`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-132 · L'angle du volant est l'élévation du grand côté du téléphone
- **Ce qui arrive :** cet angle ne dépend pas de l'inclinaison avant-arrière : le volant marche tenu droit ou à plat.
- **D'où on le sait :** `LEARNINGS.md:135` (PPSSPP `TiltEventProcessor`, SuperTuxKart) ; `wheel.rs:44`.
- **Mécaniques :** volant.
- **Question :** le volant marche-t-il quelle que soit la façon de tenir le téléphone ?
- **Pression :** tests de `wheel.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-133 · Braquage complet entre 19,5 et 32°, petite zone morte
- **Ce qui arrive :** SuperTuxKart braque à fond à 19,5°, PPSSPP à 32°, zone morte d'environ 2,3°. L'app prend 25° et 2,5°, linéaire. Un réglage de sensibilité reste à faire.
- **D'où on le sait :** `LEARNINGS.md:136` ; `wheel.rs:11-15`.
- **Mécaniques :** volant.
- **Question :** à quel angle braque-t-on à fond, et tenu droit, va-t-on droit ?
- **Pression :** `level_goes_straight_and_25_degrees_is_full_lock`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-134 · Téléphone presque à plat : garder le dernier braquage
- **Ce qui arrive :** à plat, l'angle ne veut plus rien dire. Sous 0,3 g dans le plan de l'écran, l'app garde la dernière valeur (SuperTuxKart).
- **D'où on le sait :** `LEARNINGS.md:137` ; `wheel.rs:16-18` et `:37-39`.
- **Mécaniques :** volant.
- **Question :** que lit le volant quand le téléphone est posé à plat ?
- **Pression :** tests de `wheel.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-135 · Pas de calibration : la gravité est absolue
- **Ce qui arrive :** rien ne dérive, il n'y a rien à recalibrer (SuperTuxKart, PPSSPP).
- **D'où on le sait :** `LEARNINGS.md:138` ; `wheel.rs:6-7`.
- **Mécaniques :** volant.
- **Question :** le joueur doit-il calibrer avant de jouer ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-136 · La finesse du braquage ne fait pas le plaisir
- **Ce qui arrive :** Mario Kart Wii ne lit que 15 niveaux de braquage. L'app envoie une valeur continue ; le jeu arrondit s'il veut.
- **D'où on le sait :** `LEARNINGS.md:139` (Dolphin, bug 8756).
- **Mécaniques :** volant.
- **Question :** le jeu a-t-il besoin de plus de finesse que le joueur n'en dose ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-137 · Le sens des capteurs dépend de l'orientation de l'interface
- **Ce qui arrive :** d'un paysage à l'autre, le signe du volant et le « vers le haut » du pointeur s'inversent. Un signe faux fait braquer à l'envers.
- **D'où on le sait :** `LEARNINGS.md:154` (RetroArch `cocoa_input.m`) ; `wheel.rs:40-44` ; `pointer.rs:86-89`.
- **Mécaniques :** volant, pointeur, capteurs.
- **Question :** les capteurs se lisent-ils dans les axes de l'interface telle qu'elle est tournée maintenant ?
- **Pression :** tests de `wheel.rs` et `pointer.rs` ; à vérifier sur un vrai iPhone.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-138 · Un pointeur relatif, par le gyroscope, jamais par le magnétomètre
- **Ce qui arrive :** une TV perturbe le magnétomètre. Le curseur bouge à partir de la vitesse de rotation (Dolphin).
- **D'où on le sait :** `LEARNINGS.md:146` ; `pointer.rs:5-7`.
- **Mécaniques :** pointeur.
- **Question :** le pointeur dépend-il d'un capteur que la TV dérègle ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-139 · Gauche et droite se mesurent autour de la vraie verticale
- **Ce qui arrive :** la vitesse de rotation projetée sur la gravité donne la même sensation quelle que soit l'inclinaison du téléphone (« player space » de Jibb Smart).
- **D'où on le sait :** `LEARNINGS.md:147` (GyroWiki) ; `pointer.rs:73-84`.
- **Mécaniques :** pointeur.
- **Question :** tourner vers la droite déplace-t-il le curseur vers la droite, le téléphone penché ou non ?
- **Pression :** tests de `pointer.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-140 · Environ 30° pour traverser l'écran
- **Ce qui arrive :** Dolphin prend 25°, JoyShockMapper environ 45°. L'app prend 30°, et les mêmes degrés par point en hauteur.
- **D'où on le sait :** `LEARNINGS.md:148` ; `pointer.rs:16-19`.
- **Mécaniques :** pointeur.
- **Question :** combien faut-il tourner le poignet pour traverser la TV ?
- **Pression :** tests de `pointer.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-141 · Le curseur s'arrête au bord, ce qui le recentre
- **Ce qui arrive :** comme une souris, un curseur bloqué au bord repart dès qu'on revient. Un bouton Centrer le remet au milieu.
- **D'où on le sait :** `LEARNINGS.md:149` (Dolphin `EmulateIMUCursor`) ; `pointer.rs:119-127`.
- **Mécaniques :** pointeur.
- **Question :** un pointeur relatif qui dérive se rattrape-t-il sans calibration ?
- **Pression :** `a_finger_off_the_pointer_buttons_drags_the_cursor_and_center_brings_it_back`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-142 · Lisser par paliers : sous environ 5°/s seulement
- **Ce qui arrive :** un mouvement lent est surtout le tremblement de la main ; un mouvement rapide est voulu. Lissé en dessous, jamais au-dessus (Jibb Smart).
- **D'où on le sait :** `LEARNINGS.md:150` ; `pointer.rs:20-25`.
- **Mécaniques :** pointeur.
- **Question :** le lissage retarde-t-il un geste rapide ?
- **Pression :** tests de `pointer.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-143 · Un trou dans les échantillons n'est pas un mouvement
- **Ce qui arrive :** un écart de plus de 0,1 s entre deux échantillons est une pause, pas un pas de temps : intégrer dessus ferait sauter le curseur (SuperTuxKart plafonne son pas).
- **D'où on le sait :** `LEARNINGS.md:151` ; `pointer.rs:26-27` et `:66-70`.
- **Mécaniques :** pointeur, capteurs.
- **Question :** une pause du flux de capteurs peut-elle produire un grand mouvement ?
- **Pression :** tests de `pointer.rs`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

### REM-144 · Un glisser du doigt en secours du gyroscope
- **Ce qui arrive :** sans gyroscope, ou pour viser fin, un doigt glissé hors des boutons déplace le curseur (Dolphin Android). 600 pt de course traversent la TV.
- **D'où on le sait :** `LEARNINGS.md:152` ; `pointer.rs:11-12` et `:28-29`.
- **Mécaniques :** pointeur.
- **Question :** le pointeur reste-t-il utilisable sans capteur ?
- **Pression :** `a_finger_off_the_pointer_buttons_drags_the_cursor_and_center_brings_it_back`.
- **Sous la nouvelle forme :** question de mécanique
- **Étage :** mécanique

## Le jeu et sa télécommande

### REM-145 · C'est le jeu qui choisit ce que montre le téléphone, selon la phase
- **Ce qui arrive :** chez Jackbox, chaque phase a son écran de téléphone. L'app n'a pas de menu : le jeu dit quoi montrer, et en change à tout moment.
- **D'où on le sait :** `LEARNINGS.md:84` ; commit 8a1d809.
- **Mécaniques :** page téléphone.
- **Question :** le joueur doit-il choisir lui-même un mode de télécommande ?
- **Pression :** `the_game_picks_the_remote_and_can_change_it` (`session.rs:348`).
- **Sous la nouvelle forme :** impossible par la forme. Le jeu rend une vue par appareil, calculée depuis son état (« La forme d'un jeu ») ; sa page lui appartient (« La page téléphone »).
- **Étage :** commun

### REM-146 · Un jeu garde toujours une voie sans contrainte
- **Ce qui arrive :** l'hôte a demandé une voie libre, à côté des modèles : la page du jeu, en plein écran.
- **D'où on le sait :** `LEARNINGS.md:86` ; `CONTRAT.md:69`.
- **Mécaniques :** page téléphone.
- **Question :** un jeu peut-il montrer ce qu'aucun modèle ne prévoit ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme. À l'intérieur de la forme, un jeu est libre, et chaque jeu code la page de téléphone qu'il veut (« La page téléphone »).
- **Étage :** commun

### REM-147 · Une app qui ne décide rien : la coquille transmet, le cœur pur décide
- **Ce qui arrive :** la coquille Swift tient le socket, transmet doigts et capteurs, dessine et joue les vibrations ; le cœur Rust, sans entrée ni sortie, décide tout et se teste en natif. Le chien de garde vit dans le cœur pour que le temps reste une entrée. Pourtant, les 18 trous de la fiche vivaient dans la coquille, que rien n'exécutait.
- **D'où on le sait :** `AGENTS.md:7` et `:21` ; `crates/core/src/lib.rs:1-6` ; commit 5a90129 ; T18.
- **Mécaniques :** app iPhone.
- **Question :** quelle part d'un appareil échappe encore aux tests à vitesse machine ?
- **Pression :** les tests natifs du cœur ; le banc de REM-148 pour le reste.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

## Bancs, outillage et méthode

### REM-148 · Chaque appareil passe les mêmes pressions, coquille comprise
- **Ce qui arrive :** rien n'exécutait la connexion, le modèle du pad ni la page de l'app : chaque trou passait toutes les suites en vert. Le banc compile les vraies sources de la coquille avec son propre point d'entrée, les lance sur un simulateur, et les joue contre un faux jeu qui coupe, gèle ou se tait, puis contre un vrai salon. Il juge par ce que le jeu a reçu. Reste hors banc : l'attribution des doigts par la vue de toucher, que le banc contourne.
- **D'où on le sait :** T18 ; commit 32c1964 ; `bench/src/lib.rs:1-11` ; `bench/app/BenchApp.swift:5-9` ; `LEARNINGS.md:176` ; leçon telecommande-36 (idb).
- **Mécaniques :** app iPhone, page téléphone.
- **Question :** le code qui tient le lien sur un vrai appareil tourne-t-il dans une épreuve, ou seulement le cœur ?
- **Pression :** `cd remote/bench && cargo test`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-149 · Un banc qui ne peut pas tourner échoue, il ne saute pas
- **Ce qui arrive :** sans macOS, Xcode et simulateur, le banc échoue au lieu de passer. `soiree check` dit explicitement quand le contrôle du contrat page-app n'a pas tourné.
- **D'où on le sait :** `bench/src/lib.rs:10-11` ; commit 32c1964 ; `LEARNINGS.md:198`.
- **Mécaniques :** épreuve.
- **Question :** un contrôle qui n'a pas tourné peut-il se lire comme réussi ?
- **Pression :** `certification/check.md`.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-150 · Le pare-feu du Mac bloque un binaire de test sur l'IP du réseau
- **Ce qui arrive :** sur la boucle locale, tout passe ; sur l'IP du réseau, le pare-feu arrête le faux hôte. Le banc joue l'adresse locale par `localhost.`, qui n'est pas non plus un contexte sécurisé. La clé qui l'autorise n'existe que dans le banc.
- **D'où on le sait :** `LEARNINGS.md:216` ; `bench/app/Info.plist:40-46`.
- **Mécaniques :** épreuve.
- **Question :** le banc éprouve-t-il le même contexte qu'une vraie soirée ?
- **Pression :** `bench/tests/shell.rs:525-530`.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-151 · L'outil du simulateur peut se bloquer : appeler le vrai binaire, borner chaque appel
- **Ce qui arrive :** `xcrun simctl` se bloque quand CoreSimulator est plus vieux que Xcode. `xcodebuild` refuse alors la destination simulateur ; le build passe par le SDK. Chaque appel est borné, et un démarrage est lent en CI.
- **D'où on le sait :** `bench/src/shell.rs:12-18` ; T18, « cas ».
- **Mécaniques :** épreuve.
- **Question :** un outil de l'épreuve peut-il la bloquer sans fin ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-152 · Un banc qui dépend des fichiers d'un composant en travaux casse quand il bouge
- **Ce qui arrive :** pendant une refonte de salon, le banc ne compilait plus. Le vrai salon du banc se construit à part, et seuls les tests qui en ont besoin le disent.
- **D'où on le sait :** `LEARNINGS.md:217` ; `bench/src/salon.rs:13-14` ; commit 32c1964.
- **Mécaniques :** épreuve.
- **Question :** un banc échoue-t-il pour une raison étrangère à ce qu'il éprouve ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-153 · Un build doit marcher depuis un clone neuf
- **Ce qui arrive :** le premier build CI de v0.3.0 a échoué : la source Swift générée était absente. La phase de build la produisait, mais ne la déclarait pas à Xcode. Une génération préalable en local masquait le défaut. La phase déclare désormais ses sorties, reste toujours à refaire pour voir les changements Rust transitifs, et la CI vérifie l'absence de la source générée avant de construire.
- **D'où on le sait :** `LEARNINGS.md:219-232` ; commit 6bef5d8 ; `ios/Remote.xcodeproj/project.pbxproj:129-150`.
- **Mécaniques :** app iPhone, outillage.
- **Question :** le build passe-t-il sur une machine qui n'a jamais rien généré ?
- **Pression :** la CI, depuis un clone sans fichiers générés.
- **Sous la nouvelle forme :** pression du commun
- **Étage :** commun

### REM-154 · Cargo lancé par Xcode tourne dans un environnement propre
- **Ce qui arrive :** l'environnement d'Xcode (SDK, cibles de déploiement) ferait construire les outils hôtes de Cargo contre le SDK iOS. La phase lance Cargo dans un environnement vidé.
- **D'où on le sait :** `project.pbxproj:150`.
- **Mécaniques :** outillage.
- **Question :** un outil appelé depuis un autre hérite-t-il d'un environnement qui le trompe ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-155 · Cargo s'installe aussi par Homebrew
- **Ce qui arrive :** l'environnement propre ne cherchait Cargo que dans `~/.cargo/bin`. Les machines macOS de GitHub, comme beaucoup de Mac, installent rustup par Homebrew : le build ne démarrait pas.
- **D'où on le sait :** commit 3f891a6 ; `project.pbxproj:150`.
- **Mécaniques :** outillage.
- **Question :** l'outillage trouve-t-il Rust partout où on l'installe ?
- **Pression :** la CI macOS.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-156 · La bibliothèque et l'outil de wasm-bindgen ont la même version exacte
- **Ce qui arrive :** la crate doit correspondre exactement au CLI installé, sinon la colle générée ne se charge pas.
- **D'où on le sait :** `remote/Cargo.toml` (dépendance `wasm-bindgen`, épinglée) ; `README.md:39`.
- **Mécaniques :** outillage, page téléphone.
- **Question :** deux outils qui doivent s'accorder ont-ils une seule version déclarée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-157 · Une propriété peut figer un défaut
- **Ce qui arrive :** la propriété du frein exigeait « jamais deux états à moins de 1/60 s ». Elle interdisait le correctif de REM-15 et figeait la perte d'une image sur deux. Elle dit maintenant ce que promet le contrat : aucun écart sous 1/60 s moins le tremblement de l'horloge, et jamais 61 états en moins d'une seconde.
- **D'où on le sait :** T7 ; `properties.rs:380-385` ; leçon telecommande-20.
- **Mécaniques :** épreuve.
- **Question :** la propriété dit-elle ce que le contrat promet, ou ce que le code fait ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-158 · Une leçon notée « gardée » sans sa pression peut mentir
- **Ce qui arrive :** `LEARNINGS.md` a longtemps marqué « Gardé » l'appui plus court qu'une image, alors que l'état ne se lisait qu'à l'envoi. De même, il promettait le micro accordé sans seconde question, puis disponible sur une adresse locale : ni l'un ni l'autre n'était vrai.
- **D'où on le sait :** commit b7bae9f ; `LEARNINGS.md:94` ; lignes retirées aux commits b7bae9f, 9e4c5b9 et 367201d.
- **Mécaniques :** épreuve, documents.
- **Question :** chaque leçon donnée pour tenue nomme-t-elle le test qui la tient ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

### REM-159 · Une doc qui promet ce que le code ne fait pas égare l'agent qui la suit
- **Ce qui arrive :** `CONTRAT.md` promettait des télécommandes natives que salon ne pouvait pas relayer, et « le WebSocket les livre tous » alors que les traits se perdaient (REM-07). Un agent qui suit la doc écrit alors un jeu faux.
- **D'où on le sait :** commit 6764852 ; lignes retirées de `CONTRAT.md` aux commits eef70bc et d4ed19d ; T1.
- **Mécaniques :** documents.
- **Question :** chaque promesse du contrat a-t-elle un test qui échouerait si elle cessait d'être vraie ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance

## Accidents de l'ancienne architecture

### REM-160 · Le QR du jeu suffisait : même hôte, même port, chemin `/remote`, même requête
- **Ce qui arrive :** l'app déduisait l'adresse de son socket de celle de la page du QR, pour ne rien demander de plus aux jeux.
- **D'où on le sait :** `LEARNINGS.md:87` ; `crates/core/src/join.rs:1-55`.
- **Mécaniques :** rejoindre.
- **Question :** sans objet.
- **Pression :** `a_game_page_leads_to_its_remote_socket_with_the_same_query`.
- **Sous la nouvelle forme :** obsolète. Le relais ne sert qu'une page de démarrage, la même pour tous les jeux, et le QR porte une clé (« Relier les appareils ») : il n'y a plus d'adresse de jeu à dériver.
- **Étage :** commun

### REM-161 · Le format de la vraie manette N64 ne voyage pas
- **Ce qui arrive :** la première version envoyait les 4 octets de la vraie manette (bits de libultra, X et Y signés), avec son bit de remise à zéro sous L+R+Start. Tous les modèles parlent désormais le même état. Le gate octogonal, lui, reste (REM-91).
- **D'où on le sait :** commit 3eb7a68 ; `LEARNINGS.md:93` ; lignes retirées au commit 8a1d809.
- **Mécaniques :** manette tactile.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète. Héritage d'un matériel émulé ; le jeu reçoit des entrées typées du commun.
- **Étage :** connaissance

### REM-162 · Les commandes combinées des anciennes pages
- **Ce qui arrive :** une ancienne page pouvait combiner plusieurs commandes dans un objet ; l'app gardait leur ordre (vibration, motif, capteurs, orientation, datagramme).
- **D'où on le sait :** `crates/protocol/src/web.rs:183-192` ; `CONTRAT.md:169-171`.
- **Mécaniques :** page téléphone.
- **Question :** sans objet.
- **Pression :** `native_commands_keep_legacy_batches_defaults_and_bounds`.
- **Sous la nouvelle forme :** obsolète. Compatibilité avec des pages JavaScript écrites à la main ; le contrat n'a qu'une définition typée.
- **Étage :** connaissance

### REM-163 · GCVirtualController a une disposition figée
- **Ce qui arrive :** la manette virtuelle d'Apple n'offre qu'un stick et quatre boutons par côté : trop peu pour la N64.
- **D'où on le sait :** `LEARNINGS.md:76` (WWDC21).
- **Mécaniques :** manette tactile.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance
- **Étage :** connaissance
