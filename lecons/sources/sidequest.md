# Leçons de Sidequest

## La source

Le dépôt `games/sidequest`, à l'étiquette locale `avant-refonte` (14bba35) : 48 commits, du 26 octobre au 2 novembre 2026. Sidequest est le clone en Rust de Sidequest Live, le jeu du hackathon Grok (`~/Projects/grok-hackathon`, 16 commits du 22 octobre 2026). La copie conforme est à l'étiquette `sidequest-v1` ; la V2 l'a allégée, puis portée sur salon, puis dotée d'un score et d'une fin (chantier v0.4.0).

Les numéros de ligne renvoient à l'état d'`avant-refonte`. Les références `fichier.ts:ligne` sont celles de l'original, telles que l'ancien code ou `LEARNINGS.md` les citent.

## Ce que j'ai lu

- `LEARNINGS.md` en entier, `README.md`, `AGENTS.md`, `verification.toml` (garanties J1 à J50 et décisions du catalogue).
- `archive/research/history.md` (l'historique de l'original et sa table de leçons), les parties de `archive/research/critic.md` sur les comportements non décrits, `archive/PLAN-V2.md`.
- Le message des 48 commits ; le diff des commits qui corrigent un défaut (14bba35, 61366ad, 4f5f9b1, c1f966e, 388dfe9, e45664b, b6a06e1, 7778240, 0102dd3, 66821b0, 8332a61), et la liste des fichiers des autres.
- Le code des règles (`crates/core/src/{game,game/player,party,physics,zones,deck,validate,text,protocol}.rs`), de la salle (`crates/host/src/{room,pad,app,schedule}.rs`), de la page téléphone (`crates/controller/src/{session,pad,selection,text,gone}.rs`, `web/remote.rs`, `web/selector.rs`, `web/storage.rs`), de la télé (`crates/screen/src/{ui,floor,scene,timeline,audio}.rs`), et les noms des tests qui portent les garanties.

## Ce que je n'ai pas lu

- Le diff complet des gros commits de fondation (2b151a9, 86e9a9c, 4a5396f, e45e8c0, 3a6391c, 969fcda) : seulement leur message et les parties que `LEARNINGS.md` résume.
- Le code de rendu GPU (`crates/screen/src/web/gpu.rs`, `renderer.rs`), les scripts du harnais (`tools/*.mjs`), les rapports de `certification/`.
- Le code de salon lui-même : ses leçons ont leur propre source. Elles sont citées ici quand Sidequest les a trouvées ou demandées.

---

## Mouvement et sensation

### SQ-01 · Une direction tenue se lâche seule quand son téléphone se tait
- **Ce qui arrive :** un téléphone perd le focus, se met en veille ou passe en arrière-plan pendant qu'un doigt tient une flèche. Sans garde-fou, le personnage continue de marcher seul.
- **D'où on le sait :** `crates/core/src/physics.rs:28` et `:58` (lâchée 450 ms après réception) ; `crates/controller/src/session.rs:25` et `:145` (reposée toutes les 150 ms) ; `crates/controller/src/web/remote.rs:373-379` (tout relâché sur `visibilitychange`, `blur`, `pagehide`) ; original `physics.ts:9`, `main.tsx:63-74` ; `LEARNINGS.md:23`.
- **Mécaniques :** temps réel, entrées tenues.
- **Question :** quand l'appareil qui tient un état se tait, combien de temps cet état vaut-il encore, et la répétition laisse-t-elle assez de marge sous cette limite ?
- **Pression :** `stops_after_a_controller_disappears` (`crates/core/tests/original/physics.rs:74`), `a_held_direction_is_set_again_every_150_ms_and_a_stop_once` (`crates/controller/src/session.rs:302`), J8.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-02 · Un geste livré en retard ne fait jamais bouger personne
- **Ce qui arrive :** en 4G, TCP retient les paquets pendant plusieurs secondes, puis les livre d'un coup. Le personnage rejoue des appuis anciens : 22 px de dérive mesurés. L'original a borné l'âge d'une direction par un bail de l'hôte ; l'ancien jeu, par l'heure de l'hôte portée par chaque direction (1 s au plus). Un « stop » passe toujours.
- **D'où on le sait :** original e2067ec et `movement-corrections.md` ; `crates/core/src/game.rs:147` (`lease_left`) ; `crates/core/src/lib.rs` (`INPUT_LEASE_MS`) ; `LEARNINGS.md:24`, `:191`.
- **Mécaniques :** temps réel, réseau.
- **Question :** jusqu'où le commun replace-t-il un geste en retard, et que devient un geste plus vieux que cette fenêtre ?
- **Pression :** `a_press_delivered_late_does_not_move` (`crates/core/tests/original/movement.rs:44`), étape `late_press` des bots, J8.
- **Sous la nouvelle forme :** pression du commun. Le contrat borne la fenêtre (« Un appareil ne peut pas prétendre avoir agi plus tôt que la fenêtre ne le permet ») et ses pressions doivent reproduire le blocage TCP puis la rafale.
- **Étage :** commun.

### SQ-03 · Le mouvement est un état, jamais un historique
- **Ce qui arrive :** l'original mettait chaque appui en file. Après un blocage, la file partait d'un coup. Ne garder qu'un état en attente a réduit 13 paquets à 1.
- **D'où on le sait :** original `movement-sender.ts` ; `crates/core/src/client/movement.rs:1-10` ; `LEARNINGS.md:25`.
- **Mécaniques :** temps réel, réseau.
- **Question :** ce geste est-il un état (seul le dernier compte) ou un événement (chacun compte une fois) ?
- **Pression :** test de salon `the_latest_state_wins_and_a_big_one_is_cut_to_fit`.
- **Sous la nouvelle forme :** pression du commun. Le commun remplace un état périmé par le plus récent et compte un événement une fois ; le jeu déclare la sorte de chaque geste.
- **Étage :** commun.

### SQ-04 · Le pilotage reste à 30/s, 190 px/s, plafond à 1,15×
- **Ce qui arrive :** avec un coefficient de 18/s, les demi-tours semblaient mous. 30/s retire 60 à 104 ms à vitesse maximale égale. Ces constantes et l'ordre des collisions définissent la sensation du jeu.
- **D'où on le sait :** original e2067ec, `physics.ts:22` ; `crates/core/src/physics.rs:17-25` ; `LEARNINGS.md:26`.
- **Mécaniques :** physique.
- **Question :** le nouveau jeu reproduit-il les trajectoires de référence ?
- **Pression :** `crates/core/tests/physics_reference.rs` contre `crates/core/tests/fixtures/physics.json` (7 scénarios, à 1e-6 px).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-05 · La distance d'arrêt dépend du transport, pas du pilotage
- **Ce qui arrive :** passer de 18/s à 30/s n'a réduit la glissade que de 30,5 à 29,7 px. Le retard du réseau, lui, pèse lourd.
- **D'où on le sait :** `archive/research/history.md`, leçon 5 (comparaison de mouvement de l'original) ; `LEARNINGS.md:92`, `:135-138`.
- **Mécaniques :** physique, temps réel.
- **Question :** sans objet.
- **Pression :** aucune ; mesure avec retard injecté (60 ms par sens : arrêt en 300 ms, 26,5 px).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-06 · La simulation n'est jamais le goulot
- **Ce qui arrive :** 50 joueurs coûtent 0,5 ms au p95 dans l'original, moins de 0,1 ms dans l'ancien jeu. Les collisions en O(n²) suffisent. Ce qui ralentissait, c'était le trajet réseau (serveur distant et 4G).
- **D'où on le sait :** original `latency-investigation.md:19` ; `LEARNINGS.md:27`, `:91`.
- **Mécaniques :** physique.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-07 · On agit à l'appui, pas au relâchement
- **Ce qui arrive :** `click` part au relâchement du doigt, ce qui ajoute le temps de l'appui au retard ressenti. Le saut part sur `pointerdown` ; `click` ne compte que pour une activation clavier (`detail == 0`).
- **D'où on le sait :** original de4d34a ; `crates/controller/src/web/remote.rs:347-357` ; `LEARNINGS.md:28`.
- **Mécaniques :** entrées tactiles.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-08 · Un bouton tenu se relâche par tous les chemins où le pointeur s'en va
- **Ce qui arrive :** un doigt qui glisse hors du bouton, un appel entrant ou une capture refusée laissent une flèche tenue. La croix capture le pointeur au mieux (le navigateur peut refuser), et relâche sur `pointerup`, `pointercancel`, `lostpointercapture` et `blur`. Une répétition de touche n'envoie rien.
- **D'où on le sait :** `crates/controller/src/web/remote.rs:318-345` ; original `RemoteControl.tsx:60-64`, `main.tsx:58` ; `LEARNINGS.md:75`.
- **Mécaniques :** entrées tactiles.
- **Question :** sans objet.
- **Pression :** test navigateur U1 (le personnage cesse de bouger après le relâchement), `LEARNINGS.md:363-369`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-09 · Une page cachée ne répète rien
- **Ce qui arrive :** le navigateur ralentit les minuteurs d'un onglet caché. Une page qui répète ou mesure en arrière-plan envoie des états faux et des mesures fausses.
- **D'où on le sait :** `crates/controller/src/session.rs:141-149` (« background timers lie ») ; original `session.ts:41-45`, `telemetry.ts` ; `archive/research/history.md`, leçon 10.
- **Mécaniques :** temps réel.
- **Question :** que fait un appareil dont la page est cachée, et le commun le sait-il ?
- **Pression :** `hidden_pages_repeat_nothing` (`crates/controller/src/session.rs:340`).
- **Sous la nouvelle forme :** pression du commun. Le commun possède les états tenus, l'horloge et la mesure de latence ; la page n'a plus rien à répéter.
- **Étage :** commun.

### SQ-10 · L'horloge d'un appareil s'estime par l'aller-retour le plus court
- **Ce qui arrive :** un aller-retour retardé fausse l'écart d'horloge. On garde le meilleur échantillon d'une fenêtre, on ignore les aberrants, on repart de zéro à chaque connexion.
- **D'où on le sait :** original `connection-timing.ts:10-21` ; `LEARNINGS.md:29`, `:238` (ce que salon ne testait pas lui a été demandé : un aller-retour bloqué ne pilote pas l'horloge, les vieux échantillons sortent).
- **Mécaniques :** temps réel.
- **Question :** l'estimation d'horloge résiste-t-elle à un aller-retour bloqué et à des échantillons qui vieillissent ?
- **Pression :** test de salon `round_trips_and_the_host_clock_are_measured` (à 30 ms près sous 20 ms de gigue).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Identité, présence et reconnexion

### SQ-11 · Le dernier onglet ouvert gagne, et l'ancien ne revient pas seul
- **Ce qui arrive :** un joueur ouvre le jeu dans un second onglet. Les deux se disputent le personnage. Le dernier gagne ; l'ancien affiche « DÉJÀ OUVERT AILLEURS » et un bouton « JOUER ICI », sans se reconnecter de lui-même. Dans l'original, le code de fermeture choisi entrait en collision avec celui de la bibliothèque (SQ-66).
- **D'où on le sait :** original `PartyRoom.ts:170-194` ; `crates/controller/src/gone.rs` ; `LEARNINGS.md:30`, `:205`.
- **Mécaniques :** identité.
- **Question :** que voit un appareil remplacé, et peut-il reprendre la main sans recharger ?
- **Pression :** test de salon `a_second_tab_takes_over_and_the_first_stays_put` (C5), étape `second_tab` des bots, `a_second_tab_or_a_refusal_ends_the_session` (`crates/controller/src/session.rs:392`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-12 · Un personnage attend son téléphone 90 s, puis l'appareil repart de zéro
- **Ce qui arrive :** un téléphone coupé garde son personnage 90 s, à sa place. Au-delà, le personnage est supprimé ; l'appareil qui revient en choisit un nouveau. Un appareil qui rejoint à nouveau retrouve son personnage et son look, même s'il envoie un autre prénom.
- **D'où on le sait :** `crates/core/src/lib.rs` (`OFFLINE_GRACE_MS`) ; `crates/core/src/game.rs:428-462`, `:727-749` ; original `PartyRoom.ts` ; `archive/research/critic.md` § 2.G.5.
- **Mécaniques :** identité, présence.
- **Question :** combien de temps un personnage attend-il son joueur, et que retrouve celui-ci à son retour ?
- **Pression :** `a_device_that_joins_again_keeps_its_character_and_ignores_the_new_name_and_look`, `a_device_back_after_a_cut_gets_its_character_where_it_stood`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-13 · Un téléphone réessaie sans abandonner, et se reconnecte dès son réveil
- **Ce qui arrive :** l'original abandonnait après 10 essais (« Room unavailable. Reload to try again. »). Un serveur relancé pendant la soirée laissait les téléphones morts. Salon réessaie sans fin, à 2 s au plus (±25 %), et reconnecte aussitôt une page réveillée d'un gel.
- **D'où on le sait :** original `session.ts:96-110` ; `LEARNINGS.md:57`, `:254-255` ; c57943b.
- **Mécaniques :** réseau.
- **Question :** un appareil revient-il seul après une coupure, un gel ou un redémarrage, sans que personne touche rien ?
- **Pression :** `a_page_woken_from_a_freeze_reconnects_at_once` (`crates/core/tests/phone_link.rs:194`), `a_phone_keeps_trying_until_the_host_is_back` (`:227`), scénarios C2, C3, C4, C7 du harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-14 · Un chien de garde compte depuis le dernier paquet reçu
- **Ce qui arrive :** un Wi-Fi peut mourir sans fermer la connexion. Un chien de garde qui compterait sur les minuteurs serait trompé par un onglet caché, dont Chrome ralentit les minuteurs. Compter depuis le dernier paquet reçu évite les deux pièges.
- **D'où on le sait :** `LEARNINGS.md:173`, `:252-253` ; c57943b (« Phones now notice a silently dead Wi-Fi within 3 s »).
- **Mécaniques :** réseau.
- **Question :** combien de temps une connexion morte en silence passe-t-elle pour vivante ?
- **Pression :** test de salon `silence_is_noticed_on_both_sides` (C3), scénario `freeze` du harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-15 · On montre comment rejoindre avant de lancer le jeu
- **Ce qui arrive :** scanner doit être le premier geste. Le QR occupe d'abord tout l'écran, les joueurs rejoignent, puis JOUER révèle la salle, et le QR reste dans un coin. JOUER est retenu par salle : une télé rechargée ne repasse pas par l'accueil.
- **D'où on le sait :** original a2dbb54, `StageDisplay.tsx`, `DESIGN.md:7` ; `crates/screen/src/web/mod.rs:33-35`, `:472-480` ; `LEARNINGS.md:31`.
- **Mécaniques :** toutes.
- **Question :** l'écran dit-il comment rejoindre, dès l'ouverture et pendant toute la partie ?
- **Pression :** captures `tools/visual/golden/screen/preview_welcome-*.png`.
- **Sous la nouvelle forme :** pression du commun. Le contrat l'exige de tout jeu (« Le jeu s'ouvre … et affiche comment le rejoindre »).
- **Étage :** commun.

### SQ-16 · Le QR porte une adresse que les téléphones joignent vraiment
- **Ce qui arrive :** une page ouverte sur `localhost` donnerait aux téléphones une adresse qui ne mène nulle part. L'original choisissait la première IPv4 du réseau local. Certaines box isolent les appareils : les téléphones n'atteignent alors pas l'ordinateur.
- **D'où on le sait :** original `index.ts:36`, `README.md:20` ; `crates/host/src/options.rs:157` ; `LEARNINGS.md:32`, `:194`.
- **Mécaniques :** réseau.
- **Question :** l'adresse du QR est-elle joignable depuis un téléphone sur ce Wi-Fi, et que se passe-t-il si le Wi-Fi isole les appareils ?
- **Pression :** `phones_get_a_reachable_address` ; le reste à vérifier en répétition.
- **Sous la nouvelle forme :** pression du commun. Le contrat prévoit le repli par le service en ligne quand le Wi-Fi isole les appareils.
- **Étage :** commun.

### SQ-17 · Le pare-feu de macOS bloque l'UDP de chaque binaire neuf
- **Ce qui arrive :** macOS demande d'autoriser chaque nouveau binaire. Sans réponse, l'UDP de WebRTC est bloqué sans erreur, et tout passe par WebSocket après 3 s d'attente.
- **D'où on le sait :** `README.md` (« Le pare-feu de macOS demande d'autoriser chaque nouveau binaire ») ; `LEARNINGS.md:350`.
- **Mécaniques :** réseau.
- **Question :** le commun dit-il quel canal il a obtenu, et le jeu reste-t-il correct sur le pire ?
- **Pression :** aucune dans le jeu ; harnais forcé en WebSocket (`?salon=ws`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-18 · Un contrôle qui force un chemin ne voit pas le chemin par défaut
- **Ce qui arrive :** la télé ouverte sans option tentait WebRTC, que salon refusait comme non local. Elle rechargeait sa clé et réessayait toutes les 2 s, sans fin : l'adresse du README ne montrait jamais le jeu. Le harnais ne l'a pas vu, parce qu'il ajoutait toujours `?salon=ws`.
- **D'où on le sait :** `LEARNINGS.md:202`.
- **Mécaniques :** réseau.
- **Question :** l'épreuve exerce-t-elle aussi le chemin que prend un vrai joueur, sans option ?
- **Pression :** vérification manuelle du 27 octobre (un seul WebSocket, aucun `RTCPeerConnection`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-19 · Une rafale de messages ralentit l'expéditeur, elle ne le coupe jamais
- **Ce qui arrive :** Colyseus fermait toute connexion au-delà de 60 messages par seconde (code 4002). En 4G, un téléphone qui sortait d'un blocage vidait son retard d'un coup et se faisait éjecter juste après avoir rejoint.
- **D'où on le sait :** original `Room.mjs:1402-1407`, `latency-investigation.md:46` ; `LEARNINGS.md:34`.
- **Mécaniques :** réseau.
- **Question :** que fait le commun d'un appareil qui envoie trop ?
- **Pression :** aucune dans le jeu.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-20 · Plusieurs joueurs partagent une même adresse IP
- **Ce qui arrive :** à une soirée, tous les téléphones sortent par la même adresse du Wi-Fi. L'original avait dû relever sa limite de connexions anonymes à 180 par minute pour cette raison.
- **D'où on le sait :** original 5daa8a9 `auth.ts` ; `archive/research/history.md`, leçon 29.
- **Mécaniques :** réseau, identité.
- **Question :** une limite du commun suppose-t-elle une adresse par personne ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-21 · Seul l'ordinateur de la soirée organise, même derrière un proxy ou depuis la page d'un autre site
- **Ce qui arrive :** une page web ouverte dans le navigateur du Mac pouvait envoyer une commande à `localhost`. La règle est devenue : bouclage, `Host` local, aucun en-tête de proxy, aucun `Origin` étranger.
- **D'où on le sait :** `LEARNINGS.md:80`, `:263-265` ; `crates/host/tests/e2e.rs:663`.
- **Mécaniques :** organisation.
- **Question :** qui peut envoyer une commande d'organisateur, et par quels chemins ?
- **Pression :** `only_this_computer_organizes_even_through_a_proxy_or_another_site` (`Origin` étranger, `null`, autre port, `X-Forwarded-For`, `Forwarded`, `Host` public ou doublé : tous refusés) ; étape `strangers_cannot_command` des bots.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-22 · Un téléphone ne pilote jamais la partie
- **Ce qui arrive :** l'original avait des comptes, un organisateur par adresse e-mail, des clés d'hôte. Sans comptes, un seul invariant reste : téléphones et spectateurs n'envoient jamais de commande qui compte.
- **D'où on le sait :** `LEARNINGS.md:37` ; `crates/host/src/room.rs:437` (« Phones never command »), `:543-556`.
- **Mécaniques :** organisation.
- **Question :** quels appareils ont le droit de donner quelles commandes ?
- **Pression :** étape `strangers_cannot_command` des bots.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

## Organisation de la partie

### SQ-23 · Une commande de l'organisateur ne vaut que pour la phase qu'elle a vue
- **Ce qui arrive :** → pressée deux fois avant que la télé voie la question suivante sautait une question. Dans l'original, la seconde pression au bout du paquet laissait l'avis « The final question has already been played. » collé jusqu'à une reconnexion. Chaque commande qui fait avancer porte sa phase ; périmée, elle est jetée sans message. Pause et chat fixent une valeur : jamais périmés.
- **D'où on le sait :** `crates/host/src/room.rs:462-475`, `:545-551` ; `archive/research/critic.md` § 2.G.4 ; `LEARNINGS.md:193`.
- **Mécaniques :** progression, vote.
- **Question :** une commande répétée ou rejouée après une coupure agit-elle une fois, ou autant de fois qu'elle arrive ?
- **Pression :** `a_command_from_an_older_phase_is_dropped` (`crates/host/tests/e2e.rs:594`), J15.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-24 · Naviguer termine et enregistre la manche ; aux bords, rien ne bouge
- **Ce qui arrive :** l'organisateur, debout près de l'ordinateur, mène la soirée aux flèches. Suivant ou Précédent compte la manche en cours, puis lance l'autre avec un minuteur neuf. Au bord du paquet, rien ne change ; dans l'original, le curseur levait une erreur avant le comptage.
- **D'où on le sait :** original 2d3d626, `PartyRoom.ts:207-234` ; `crates/core/src/game.rs:574-589` ; `LEARNINGS.md:33`.
- **Mécaniques :** progression.
- **Question :** que devient la manche en cours quand l'organisateur change de question ?
- **Pression :** `navigates_the_deck_like_the_load_test` (`crates/core/tests/original/rules.rs:25`), J12.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-25 · Un refus ne change rien
- **Ce qui arrive :** un Suivant refusé faute de numéro de manche libre avançait quand même le curseur du paquet, alors que le commentaire disait « Nothing changes ». Le test qui pose le compteur à sa limite l'a trouvé.
- **D'où on le sait :** c1f966e ; `crates/core/src/game.rs:574-589` (le numéro est demandé avant le curseur) ; `LEARNINGS.md:581`.
- **Mécaniques :** toutes.
- **Question :** sans objet.
- **Pression :** `a_counter_at_its_limit_refuses_and_changes_nothing` (`crates/core/src/game.rs:980`), J49.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat : « Un refus ne change rien : le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte l'entrée. »
- **Étage :** commun.

### SQ-26 · Une règle vit dans l'étape du jeu, nulle part ailleurs
- **Ce qui arrive :** la règle « un message doit avoir de l'encre » vivait dans la salle, avant l'appel au jeu. Le journal, le fuzzer et les tests du cœur ne la voyaient pas : un message réduit à un caractère de contrôle passait, dépensait le délai et effaçait la bulle. De même, la page téléphone interdit le saut aux Résultats mais le cœur l'accepte, et le pad saute ; dans l'original, la touche Espace de la page n'était pas filtrée.
- **D'où on le sait :** 7778240 ; `LEARNINGS.md:497`, `:608` ; `archive/research/critic.md` § 2.G.3 ; `crates/controller/src/session.rs` (`controls_disabled`).
- **Mécaniques :** toutes.
- **Question :** sans objet.
- **Pression :** `a_chat_with_no_drawable_glyph_is_refused_by_the_game_and_explained` (`crates/host/src/room/simulation.rs:436`), J23.
- **Sous la nouvelle forme :** impossible par la forme. Le jeu est une fonction pure qui reçoit des entrées ; la page « ne détient pas l'état du jeu », et la prédiction exécute la même étape.
- **Étage :** commun.

### SQ-27 · Une coupure à l'échéance ne vide pas la manche
- **Ce qui arrive :** le comptage ne prenait que les joueurs en ligne. Un Wi-Fi qui tombe à l'échéance donnait une manche à 0 participant, et tous les bulletins perdus. Désormais, un joueur coupé pendant la manche compte là où il se tient ; un fantôme d'avant la manche ne compte pas, car il n'a pas vu la question.
- **D'où on le sait :** e45664b ; `crates/core/src/game.rs:383-385`, `:623-638` ; `LEARNINGS.md:498`, `:549`.
- **Mécaniques :** vote, présence.
- **Question :** qui est attendu à la fin d'une manche, et que vaut la réponse d'un joueur coupé ?
- **Pression :** `a_global_cut_at_the_deadline_keeps_every_answer` (`crates/core/tests/original/rules.rs:182`), J24, J39.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-28 · Une coupure courte ne change pas le résultat
- **Ce qui arrive :** une propriété compare une même session jouée avec et sans coupures (retour normal, page rechargée, aucun retour avant l'échéance). Elle a deux hypothèses : le joueur coupé ne tenait pas de direction, et personne ne poussait son personnage. Sur 1 500 cas, 519 ont été écartés pour la seconde ; dans 3, le résultat différait vraiment.
- **D'où on le sait :** 6f6a8a7 ; `LEARNINGS.md:573-575`.
- **Mécaniques :** vote, physique, présence.
- **Question :** une coupure transparente pour le joueur l'est-elle aussi pour le score, et sous quelles hypothèses écrites ?
- **Pression :** `a_cut_does_not_change_the_result_of_a_round` (`crates/core/tests/certification/cuts.rs:296`), `a_cut_and_a_return_before_the_deadline_leave_the_round_unchanged` (`crates/host/src/room/simulation.rs:360`, 48 graines), J42.
- **Sous la nouvelle forme :** pression du commun. Le commun peut jouer chaque partie avec et sans coupures injectées et comparer l'histoire définitive, sous les hypothèses que le jeu déclare.
- **Étage :** commun.

### SQ-29 · Une relance de question ne retire jamais un point
- **Ce qui arrive :** ← puis → juste après une question : la relance, terminée une fraction de seconde plus tard avec les positions de la question d'avant, écrasait le vrai résultat. Le point gagné disparaissait. Règle : une relance menée au bout ajoute les nouveaux gagnants ; interrompue, elle ne change rien.
- **D'où on le sait :** 388dfe9 ; `crates/core/src/party.rs:83-115` ; `LEARNINGS.md:557-569`.
- **Mécaniques :** score.
- **Question :** qu'arrive-t-il aux points d'une question jouée deux fois ?
- **Pression :** `a_question_played_again_never_takes_its_points_back` (`crates/core/src/party.rs:273`), `an_interrupted_replay_changes_nothing`, J27.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-30 · La raison de gagner reprend le geste qui existe déjà
- **Ce qui arrive :** le jeu n'avait aucun score, et ses questions n'ont pas de bonne réponse. Compter des « bonnes réponses » aurait cassé le jeu. On marque en se tenant avec le plus grand nombre, dès deux personnes. Écartés : la minorité qui gagne (elle fige tout le monde jusqu'à la dernière seconde), aucun point aux égalités (3 contre 3 ne marquerait rien).
- **D'où on le sait :** 4a4f58c ; `crates/core/src/party.rs:1-30` ; `LEARNINGS.md:503-507`.
- **Mécaniques :** score, vote.
- **Question :** la règle de score récompense-t-elle le geste que les joueurs font déjà ?
- **Pression :** `the_crowd_scores_and_the_round_that_ends_the_party_counts`, `a_room_that_stands_apart_scores_nobody`, J26.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-31 · Une égalité n'est jamais présentée comme la victoire d'un seul
- **Ce qui arrive :** des scores égaux doivent partager leur rang et la victoire. L'annonce est calculée depuis le même classement que le tableau, pour ne jamais dire autre chose que lui. Le prénom ne départage jamais.
- **D'où on le sait :** `crates/core/src/party.rs:125-180`, `:196-216` ; catalogue EGA-04 à EGA-06.
- **Mécaniques :** score, égalités.
- **Question :** que dit l'annonce quand plusieurs joueurs sont à égalité ?
- **Pression :** `equal_scores_share_their_rank_and_the_win` (`crates/core/src/party.rs:312`), `the_headline_says_what_the_standings_say` (`:328`), J30.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-32 · On ne termine pas une partie sur un tableau vide
- **Ce qui arrive :** TERMINER restait possible avant toute manche, et montrait un tableau final sans rien à dire. Le cœur refuse maintenant (« Aucune manche jouée : lance une question avant de terminer. »).
- **D'où on le sait :** 125335b ; `crates/core/src/game.rs:512-517` ; `LEARNINGS.md:515`.
- **Mécaniques :** progression.
- **Question :** que montre la fin d'une partie qui n'a rien joué ?
- **Pression :** `after_the_end_commands_are_refused_and_the_board_does_not_change` (`crates/core/tests/original/ending.rs:270`), J28.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-33 · Une action irréversible demande une seconde pression, et un double-clic n'en est pas une
- **Ce qui arrive :** TERMINER et REJOUER (qui efface les points) partaient à un clic. Ils demandent une seconde pression avant 4 s ; puis on a vu qu'un double-clic passait pour une décision, et la seconde pression ne compte qu'après 0,4 s.
- **D'où on le sait :** 125335b, 4f5f9b1 ; `crates/screen/src/ui.rs:83-86`, `:249-280`.
- **Mécaniques :** organisation.
- **Question :** quelles commandes ne s'annulent pas, et comment l'interface évite-t-elle qu'elles partent par accident ?
- **Pression :** tests de `ui::Confirmation`, sans navigateur.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-34 · Une touche qui ne s'annule pas ne voisine pas une touche courante
- **Ce qui arrive :** la fin de partie sur `E` touchait le `D` du diagnostic sur un clavier. Elle demande Maj+E.
- **D'où on le sait :** `crates/screen/src/ui.rs:226-245` ; `LEARNINGS.md:513`.
- **Mécaniques :** organisation.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Minuteurs

### SQ-35 · Le temps restant affiché part de la durée exacte
- **Ce qui arrive :** une échéance est une somme de flottants. Une manche de 30 s démarrait à 31 quand la soustraction tombait à 30 000,000001 ms. Le temps restant passe d'abord par la milliseconde entière, puis s'arrondit à la seconde supérieure.
- **D'où on le sait :** b080652 ; `crates/core/src/protocol.rs:268-270` ; `LEARNINGS.md:590`.
- **Mécaniques :** minuteurs.
- **Question :** le nombre affiché au début d'une manche est-il exactement sa durée ?
- **Pression :** `the_time_left_counts_down_stops_with_the_pause_and_goes_with_the_round` (`crates/core/tests/original/rules.rs:773`), invariant des sessions, J48.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-36 · Le temps restant vient de l'état : figé en pause, gardé pendant une coupure
- **Ce qui arrive :** l'échéance d'une vue porte le temps passé en pause, là où la reprise la mettrait : le nombre se fige sans que la télé connaisse la pause. Une télé coupée garde son dernier nombre au lieu de compter seule, car elle ne sait pas si la manche est finie.
- **D'où on le sait :** `crates/core/src/game.rs:756-766` ; `LEARNINGS.md:589-591`.
- **Mécaniques :** minuteurs, pause.
- **Question :** l'écran compte-t-il quoi que ce soit lui-même ?
- **Pression :** `pause_freezes_everyone_and_shifts_the_deadline` (`crates/core/tests/original/rules.rs:285`), J10, J48.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-37 · Un nombre se lit à 3 m s'il fait la distance divisée par 150 à 200
- **Ce qui arrive :** le compte à rebours fait 56 px sur un monde de 760, environ 45 mm sur une télé 1080p de 55 pouces, le double du minimum habituel. Calculé, pas mesuré.
- **D'où on le sait :** `LEARNINGS.md:592`.
- **Mécaniques :** affichage.
- **Question :** sans objet.
- **Pression :** aucune ; à juger sur la vraie télé.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Texte saisi

### SQ-38 · Ce qu'on accepte doit pouvoir s'afficher
- **Ce qui arrive :** « 🦄 », « 日本語 » ou « \0 » étaient acceptés comme prénom ; la police pixel les lâchait, d'où une ligne vide sur la télé. Un chat fait d'emojis inconnus ou d'arabe était accepté sans bulle. Règle : au moins un caractère que la police encre, avec la table des caractères encrés dans le cœur, comparée glyphe par glyphe à la police. Les espaces ont une avance mais pas d'encre : ils ne prouvent rien.
- **D'où on le sait :** 7778240 ; corpus de textes hostiles (`LEARNINGS.md:479-490`) ; `crates/core/src/text.rs:12` ; `LEARNINGS.md:496`.
- **Mécaniques :** texte saisi, affichage.
- **Question :** tout texte accepté s'affiche-t-il, et la règle d'acceptation est-elle tirée de ce que l'affichage sait dessiner ?
- **Pression :** `the_core_knows_what_the_font_inks` (`crates/art/src/text.rs:522`), `a_name_with_nothing_to_draw_is_refused` (`crates/core/tests/original/rules.rs:465`), J22, J23.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-39 · On valide le texte tel qu'il sera montré
- **Ce qui arrive :** les caractères de contrôle sont retirés après la validation des longueurs. Un message fait seulement de contrôles passait la validation, puis devenait vide. Dans l'original, « salut⏎toi » devient « saluttoi ». La règle d'encre porte donc sur le texte après retrait.
- **D'où on le sait :** `crates/core/src/game/player.rs:232-236` ; `crates/core/src/text.rs:43` ; `archive/research/critic.md` § 2.G.1 ; `LEARNINGS.md:497`.
- **Mécaniques :** texte saisi.
- **Question :** la validation porte-t-elle sur le texte reçu ou sur celui qui sera affiché ?
- **Pression :** `chat_applies_cooldown_limits_and_strips_controls` (`crates/core/tests/original/rules.rs:631`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-40 · La page et le jeu mesurent un texte de la même façon
- **Ce qui arrive :** le `maxlength` du navigateur compte en unités UTF-16 (un emoji vaut 2) ; le jeu compte en points de code. Les rapports de l'original disaient UTF-16, son code comptait en points de code : c'est le code qui faisait foi. Une page plus permissive que le jeu envoie des textes refusés sans explication.
- **D'où on le sait :** `crates/controller/src/text.rs:1-3` ; `crates/core/src/text.rs` (`char_count`) ; `LEARNINGS.md:59`.
- **Mécaniques :** texte saisi.
- **Question :** ce que la page accepte, le jeu l'accepte-t-il, et l'inverse ?
- **Pression :** `the_host_enforces_the_limits`, `what_the_page_allows_the_host_accepts` (`crates/core/tests/certification/text.rs:35`, `:54`), J16.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-41 · Couper un texte respecte les graphèmes
- **Ce qui arrive :** le préremplissage du prénom coupe aux points de code. Il peut amputer une famille d'emojis ou une lettre suivie d'un accent combinant. Laissé ouvert (TXT-05) : rien ne le prouvait.
- **D'où on le sait :** `crates/controller/src/text.rs` (`prefilled_name`) ; `verification.toml`, décision TXT-05 ; `LEARNINGS.md:579`.
- **Mécaniques :** texte saisi.
- **Question :** où un texte peut-il être coupé sans produire un caractère cassé ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-42 · Un refus reste vrai pour tous les alphabets
- **Ce qui arrive :** le refus disait « Essaie des lettres ou un emoji connu » à quelqu'un qui venait de taper un prénom arabe, donc des lettres. Il dit maintenant « l'alphabet latin ».
- **D'où on le sait :** 61366ad ; `crates/core/src/validate.rs` (`NO_GLYPHS`).
- **Mécaniques :** texte saisi.
- **Question :** sans objet.
- **Pression :** `tools/text-check.mjs` lit la phrase.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-43 · Un prénom inséré dans une annonce reste une donnée
- **Ce qui arrive :** un prénom comme « {points} » ou « %s %n » ne doit jamais être lu comme un gabarit. L'annonce finale insère les prénoms en un seul passage.
- **D'où on le sait :** `crates/core/src/party.rs:351` ; catalogue TXT-09.
- **Mécaniques :** texte saisi, affichage.
- **Question :** un texte de joueur peut-il être interprété par le code qui l'affiche ?
- **Pression :** `a_name_that_looks_like_a_template_stays_a_name`, J44.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-44 · Une bulle reste bornée et entière à l'écran
- **Ce qui arrive :** dans l'original, un mot de 80 caractères donnait une bulle de 791 px, centrée sur le joueur. Les mots trop longs sont désormais coupés, et la carte d'un joueur près d'un bord reste entière dans le monde, sans déplacer le joueur.
- **D'où on le sait :** `archive/research/critic.md` § 2.G.2 ; `LEARNINGS.md:118`, `:423-425`.
- **Mécaniques :** affichage, texte saisi.
- **Question :** le plus long texte possible tient-il à l'écran, au pire endroit ?
- **Pression :** tests natifs des bords des bulles, `tools/layout-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

## Langue, typographie et contenu

### SQ-45 · Les accents français tiennent dans la case 5 × 7
- **Ce qui arrive :** « PECHE » pour « PÊCHE », « CELINE » pour « CÉLINE » : une faute en grand sur la télé. Le tableau n'a qu'un rang vide entre deux lignes. La lettre est redessinée sur cinq rangs sous son accent, dans la même case, comme sur les afficheurs à matrice (HD44780) : largeur et mise en page ne bougent pas.
- **D'où on le sait :** 39b7ebf ; `crates/art/src/font.rs:1-5` ; `LEARNINGS.md:523`.
- **Mécaniques :** affichage.
- **Question :** sans objet.
- **Pression :** `every_character_of_the_party_deck_is_drawn_as_itself`, J36.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-46 · Le signe isolé de la typographie française reste avec son mot
- **Ce qui arrive :** « Tu fais quoi ? » s'écrit avec une espace avant le point d'interrogation. Une coupure de ligne naïve envoie le « ? » seul en début de ligne suivante.
- **D'où on le sait :** `crates/art/src/room.rs:116-135` (`keep_signs_with_words`) ; `LEARNINGS.md:523`.
- **Mécaniques :** affichage.
- **Question :** sans objet.
- **Pression :** `every_question_of_the_party_deck_fits_the_board` (`crates/art/tests/decks.rs:88`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-47 · Ce qu'un joueur lit est dans sa langue et inclut tout le monde
- **Ce qui arrive :** le paquet et la barre étaient en français, mais le téléphone, les refus et les boutons restaient en anglais. Les libellés évitent le masculin seul (« En ligne », « Toi »). Le paquet avait des réponses au masculin seul et une réponse qui supposait un verre de plus.
- **D'où on le sait :** 0b4b37e, 88d7eaa ; `LEARNINGS.md:529`.
- **Mécaniques :** affichage, contenu.
- **Question :** sans objet.
- **Pression :** `every_word_on_screen_is_french_and_in_the_font` (`crates/screen/src/ui.rs:1467`), `the_pixel_words_are_french_and_fit_a_phone` (`crates/controller/src/gone.rs:81`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-48 · Une règle de groupe demande des questions sans bonne réponse
- **Ce qui arrive :** une question de culture générale a une bonne réponse, et casse la règle du groupe. Le paquet préfère les choix tranchés et les échelles sur soi. Il se lance sans option, pour ne pas tomber le soir même sur le paquet du hackathon.
- **D'où on le sait :** `LEARNINGS.md:521`.
- **Mécaniques :** contenu, vote.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Affichage et rendu

### SQ-49 · Une mise en page se vérifie au pire cas, ligne contre ligne
- **Ce qui arrive :** le classement remplit sa première colonne jusqu'à six lignes, mais le bloc suivant se plaçait comme si les deux colonnes étaient équilibrées : « LE MOMENT DE LA PARTIE » tombait sur les scores. Le test de pire cas ne regardait que les bords du panneau. C'est la capture qui l'a montré.
- **D'où on le sait :** 7b268f6 ; `LEARNINGS.md:517`.
- **Mécaniques :** affichage.
- **Question :** au pire cas tiré des limites du protocole, deux textes se touchent-ils ?
- **Pression :** `the_final_board_holds_at_its_worst_case` (`crates/screen/src/ui.rs:1614`), J34.
- **Sous la nouvelle forme :** pression du commun. Le langage de dessin du commun connaît les rectangles de texte soumis et peut refuser un chevauchement pour tout jeu.
- **Étage :** commun.

### SQ-50 · Un texte dessiné dans un canvas échappe aux contrôles du DOM
- **Ce qui arrive :** le contrôle de mise en page lisait le DOM, qui ne voit pas le texte rastérisé par le GPU. La zone sûre de la télé restait sans vérification. La télé publie maintenant les rectangles réellement soumis et le texte qui a produit chaque texture ; un contrôle négatif déplace le vrai canvas.
- **D'où on le sait :** `LEARNINGS.md:418-446`.
- **Mécaniques :** affichage.
- **Question :** le contrôle de mise en page voit-il ce que le GPU dessine ?
- **Pression :** `tools/layout-check.mjs` (six formats, Chromium/WebGPU et WebKit/WebGL2).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-51 · On observe le canvas après présentation, sans horloge ni compteur
- **Ce qui arrive :** une mesure qui lit un horodatage ou un compteur d'images peut simuler une réponse. La projection de mesure ne contient que les positions arrondies, les hauteurs de saut et les bulles des sprites présentés.
- **D'où on le sait :** `crates/screen/src/rendered.rs:1-3` ; `LEARNINGS.md:288`.
- **Mécaniques :** affichage.
- **Question :** une mesure d'affichage peut-elle passer sans que rien n'ait été dessiné ?
- **Pression :** `the_rendered_value_changes_with_the_drawn_sprite_only` (`crates/screen/src/rendered.rs:163`), J20.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-52 · Le pixel art reste sur une grille entière, identique à sa référence
- **Ce qui arrive :** les avatars de l'original étaient d'abord des polygones vectoriels, dont les pixels n'étaient pas carrés. Ils sont devenus des grilles de cases entières, agrandies au plus proche.
- **D'où on le sait :** original 2bf9d9b, `DESIGN.md:21` ; `crates/art/src/avatar.rs` ; `LEARNINGS.md:38`.
- **Mécaniques :** affichage.
- **Question :** chaque sprite est-il identique, au pixel près, à sa référence ?
- **Pression :** `crates/art/tests/reference.rs` contre `crates/art/tests/fixtures/reference.json` (1 352 avatars, salle, tableaux), J38.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-53 · WebGPU et WebGL2 n'arrondissent pas pareil un agrandissement au plus proche
- **Ce qui arrive :** agrandi en CSS, un canvas WebGPU départage les égalités d'échantillonnage dans l'autre sens que WebGL, et seulement sur les lignes. À 1920 × 1200 (échelle 1,5), 41 648 pixels faux sur 63 lignes ; 0 avec WebGL2. Retourner le canvas aligne les lignes mais décale 38 colonnes. La V2 copie elle-même l'image à l'échelle : les deux moteurs deviennent identiques.
- **D'où on le sait :** `LEARNINGS.md:56`, `:119`.
- **Mécaniques :** affichage.
- **Question :** l'image est-elle la même sur chaque moteur, à chaque taille d'écran plausible (16:10, DPR 2) ?
- **Pression :** `tools/visual/screen.sh` sur 16:9, 16:10 et DPR 2.
- **Sous la nouvelle forme :** pression du commun. Le contrat ne connaît qu'une façon de dessiner ; le commun fait lui-même l'agrandissement.
- **Étage :** commun.

### SQ-54 · Un navigateur peut annoncer WebGPU et ne rien afficher
- **Ce qui arrive :** le panneau intégré de l'app Claude expose WebGPU, ne présente rien, et ne signale aucune erreur.
- **D'où on le sait :** `LEARNINGS.md:120`.
- **Mécaniques :** affichage.
- **Question :** le commun vérifie-t-il qu'une image a bien été présentée, et pas seulement que l'API existe ?
- **Pression :** aucune ; `?webgl2` force le repli.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-55 · Le GPU peut disparaître pendant la partie
- **Ce qui arrive :** le périphérique graphique est révocable, distinct de la surface. Une perte recrée canvas, périphérique, pipelines et textures depuis le modèle conservé ; une génération invalide les acquisitions tardives ; trois reprises au plus. Un message reste lisible quand le GPU ne dessine plus.
- **D'où on le sait :** 2b35460 ; `crates/screen/GPU.md` ; `LEARNINGS.md:383-396`.
- **Mécaniques :** affichage.
- **Question :** que voit la salle quand le GPU est perdu, et le jeu continue-t-il ?
- **Pression :** régressions qui injectent la perte du vrai périphérique dans Chromium et WebKit (`cert.json.ui.checks`).
- **Sous la nouvelle forme :** pression du commun. Le contrat : « Si le GPU disparaît, le commun reconstruit l'image depuis l'état. »
- **Étage :** commun.

### SQ-56 · Glisser entre deux pas demande une marge au-delà d'un pas
- **Ce qui arrive :** la télé dessine dans le passé, entre les deux derniers états. Avec exactement un pas de retard, 15 à 28 % des images restaient figées dans 3 essais sur 9. Avec 4 ms de marge (20,7 ms en tout), 0 à 1 %.
- **D'où on le sait :** `crates/screen/src/timeline.rs:12-22` ; `LEARNINGS.md:122`, `:199-201`.
- **Mécaniques :** temps réel, affichage.
- **Question :** quelle part des images reste figée, à la cadence réelle de la télé ?
- **Pression :** part d'images figées dans le panneau `D` ; aucune pression automatique.
- **Sous la nouvelle forme :** pression du commun. Le commun fait glisser l'affichage entre deux pas ; il doit mesurer les images figées.
- **Étage :** commun.

### SQ-57 · Une mesure d'images figées se fait sur plusieurs passes
- **Ce qui arrive :** télé et hôte battent tous deux à 60 Hz sur la même horloge. Leur phase relative, tirée à l'ouverture de la page, bouge à peine pendant une passe. Une passe isolée donnait 0 % ou 14 % selon le tirage.
- **D'où on le sait :** `LEARNINGS.md:200`.
- **Mécaniques :** temps réel, affichage.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-58 · Pour glisser, l'horloge de la télé est celle des arrivées
- **Ce qui arrive :** la question n'est pas « quelle heure est-il sur l'hôte » mais « quel état est arrivé ». Avec l'heure de l'hôte, tout le trajet de chaque vue mangerait la marge ; et l'estimation en millisecondes entières sautait d'une milliseconde, soit 6 % d'un pas. L'horloge des arrivées garde son meilleur échantillon 5 s.
- **D'où on le sait :** `crates/screen/src/timeline.rs:33-53` ; `LEARNINGS.md:203`.
- **Mécaniques :** temps réel, affichage.
- **Question :** sur quelle horloge l'affichage glisse-t-il, et cette horloge saute-t-elle ?
- **Pression :** `the_clock_follows_the_quickest_recent_arrival` (`crates/screen/src/timeline.rs:184`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-59 · Un fil qui dort en millisecondes entières se réveille en retard
- **Ce qui arrive :** la boucle de salle dormait par `recv_timeout` en millisecondes entières ; sur macOS, elle se réveillait parfois tard (écarts de 21 à 28 ms, une fois 62 ms, sans aucun appareil). Salon a fini par dormir jusqu'à 2 ms avant l'échéance, puis céder la main en boucle : p95 des écarts entre vues de 20,2–22,2 à 17,8–18,6 ms.
- **D'où on le sait :** `LEARNINGS.md:200-201`.
- **Mécaniques :** temps réel.
- **Question :** de combien les pas du commun sont-ils en retard sur leur échéance, au p95 et au pire ?
- **Pression :** mesure des écarts entre pas dans le journal.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-60 · Une machine chargée gèle la boucle 100 à 400 ms
- **Ce qui arrive :** avec 6,3 Go de swap et d'autres certifications en parallèle, des grappes de pas en retard de 100 à 425 ms figeaient 20 à 45 % d'une fenêtre de 4 s. Un petit programme témoin, sans réseau ni travail, montrait les mêmes grappes à la seconde près : la cause était la machine. Aucune marge raisonnable ne couvre de tels arrêts.
- **D'où on le sait :** `LEARNINGS.md:201`.
- **Mécaniques :** temps réel.
- **Question :** sans objet.
- **Pression :** aucune ; à refaire sur la machine du soir, au calme.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-61 · Un budget qui échoue s'instrumente, il ne se relâche pas
- **Ce qui arrive :** en CI, 50 téléphones ont donné un p95 de 86,4 ms entre vues pour un budget de 25 ms. Au lieu de relâcher le seuil, on a relevé, pour chaque vue, l'heure d'arrivée et l'heure de l'hôte. La relance a montré que les réveils de l'hôte avaient le même p95 (85 ms) : retard de l'hôte, pas perte de vues. La cause du réveil tardif reste ouverte.
- **D'où on le sait :** 9ac2b20, fd0c42e ; `LEARNINGS.md:291-329`.
- **Mécaniques :** temps réel.
- **Question :** quand un budget échoue, le rapport sépare-t-il le retard de l'hôte, celui de l'observateur et celui de la machine ?
- **Pression :** `fifty_phones_move_at_once` et son diagnostic d'échec.
- **Sous la nouvelle forme :** pression du commun. Le contrat demande au rapport de séparer la part du commun et du jeu de celle du matériel.
- **Étage :** commun.

### SQ-62 · Après un retard de l'hôte, on rattrape peu et on jette le reste
- **Ce qui arrive :** le minuteur de Node de l'original ne rattrapait jamais ; une boucle qui rattraperait tout partirait en spirale. L'ancien jeu rattrapait 3 pas au plus. Un test concentre le temps virtuel autour d'une échéance après un retard, sans toucher aux durées du jeu.
- **D'où on le sait :** `crates/host/src/schedule.rs:1-13` ; `LEARNINGS.md:286`.
- **Mécaniques :** temps réel.
- **Question :** combien de pas le commun rattrape-t-il après un retard, et les pas dépendent-ils du découpage du temps en appels ?
- **Pression :** `a_stalled_host_catches_up_a_few_steps_and_drops_the_rest` (`crates/host/src/schedule.rs:70`), `the_steps_due_do_not_depend_on_how_time_is_cut` (`:85`), J7, J45.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-63 · Le pas de l'original tournait à 59,1 Hz
- **Ce qui arrive :** le minuteur de Node donnait 59,1 pas par seconde avec un pas de 1/60 s. Le vrai 60 Hz va 1,4 % plus vite, sans différence perceptible.
- **D'où on le sait :** `archive/research/libraries.md` § 1.2 ; `LEARNINGS.md:39`.
- **Mécaniques :** temps réel.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète. Accident du runtime JavaScript ; le contrat fournit des pas réguliers.
- **Étage :** connaissance.

## Messages, état et déterminisme

### SQ-64 · Un téléphone ne reçoit que ce qu'il montre
- **Ce qui arrive :** dans l'original, chaque téléphone recevait tout l'état à 30 Hz sans s'en servir.
- **D'où on le sait :** original `main.tsx:55-82` ; `LEARNINGS.md:42`.
- **Mécaniques :** réseau.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat calcule une projection et une vue par appareil.
- **Étage :** commun.

### SQ-65 · Les messages sont des types partagés, jamais du texte dans l'état
- **Ce qui arrive :** l'original rangeait du JSON dans des chaînes de son état synchronisé (`questionJson`, `zonesJson`, `resultJson`) et lisait des compteurs privés de sa bibliothèque.
- **D'où on le sait :** original `state.ts`, `PartyRoom.ts:182` ; `LEARNINGS.md:36`.
- **Mécaniques :** réseau.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat : « Les messages … sont des types Rust partagés, jamais du texte écrit à la main. »
- **Étage :** commun.

### SQ-66 · Un code de fermeture du jeu entrait en collision avec ceux de sa bibliothèque
- **Ce qui arrive :** le code 4001 « autre onglet » de l'original était aussi celui de l'arrêt du serveur de Colyseus.
- **D'où on le sait :** original `session.ts:90`, `Protocol.d.ts:263` ; `LEARNINGS.md:35`.
- **Mécaniques :** réseau.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète. Accident d'une bibliothèque ; les raisons de fermeture sont des types du commun.
- **Étage :** connaissance.

### SQ-67 · Une valeur non finie ne passe pas la validation
- **Ce qui arrive :** un bail `NaN` était accepté : `lease_ms <= 0.0` est faux pour `NaN`. Les comparaisons se posent maintenant dans le sens où `NaN` échoue (`lease_ms > 0.0`). Seuls un journal écrit à la main et le fuzzer l'atteignaient.
- **D'où on le sait :** 3e99875, c57943b ; `crates/core/src/game/player.rs:199-201` ; `LEARNINGS.md:175-180`.
- **Mécaniques :** entrées.
- **Question :** sans objet.
- **Pression :** `a_lease_that_is_not_a_number_is_refused` (`crates/core/tests/certification/sessions.rs:256`), J3.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat : « Ce qui vient de l'extérieur est validé à l'entrée — Forme : des types » ; un geste ne porte que des valeurs finies bornées.
- **Étage :** commun.

### SQ-68 · Un `NaN` gardé dans l'état ne se rejoue pas au bit près
- **Ce qui arrive :** un « stop » mémorisait encore le bail `NaN`. Le journal écrit tous les `NaN` de la même façon : le rejeu ne retrouvait pas l'état au bit près. Un stop sans bail vivant vaut désormais un bail de 0. Il a fallu aussi `float_roundtrip` pour relire les flottants au bit près.
- **D'où on le sait :** c57943b ; `crates/core/src/game/player.rs:205-208` ; `LEARNINGS.md:79`, `:177`.
- **Mécaniques :** toutes.
- **Question :** l'enregistrement d'une partie la rejoue-t-il au bit près, flottants spéciaux compris ?
- **Pression :** `a_stop_with_a_nan_lease_replays_bit_for_bit` (`crates/core/tests/replays.rs:155`), `checkpoint_fixture_replays_and_a_wrong_fingerprint_is_rejected`, J17.
- **Sous la nouvelle forme :** pression du commun. Le commun enregistre et rejoue, et compare l'empreinte de l'état.
- **Étage :** commun.

### SQ-69 · Une mesure absurde ne s'affiche pas « inf »
- **Ce qui arrive :** un aller-retour de 1e300 ms devenait « inf » dans le panneau de diagnostic.
- **D'où on le sait :** 3e99875 ; `LEARNINGS.md:178`.
- **Mécaniques :** diagnostic.
- **Question :** sans objet.
- **Pression :** invariant `round_trips_are_plausible`.
- **Sous la nouvelle forme :** impossible par la forme. La latence appartient au commun, qui la mesure en millisecondes entières bornées ; le jeu ne la reçoit pas.
- **Étage :** commun.

### SQ-70 · Les longueurs se calculent par racine carrée, pas par `hypot`
- **Ce qui arrive :** `Math.hypot` de V8 diffère de √(x² + y²) sur le dernier bit. La physique n'utilise que la racine carrée, correctement arrondie partout ; les trajectoires se comparent à 1e-6 px près à celles de l'original.
- **D'où on le sait :** `crates/core/src/physics.rs:10-12`.
- **Mécaniques :** physique.
- **Question :** la copie native et le module donnent-ils les mêmes nombres, au bit près ?
- **Pression :** `crates/core/tests/physics_reference.rs`, `crates/core/tests/state_hash.rs`.
- **Sous la nouvelle forme :** pression du commun. Le contrat compare la copie native au module et impose `libm` pour les fonctions qui diffèrent.
- **Étage :** commun.

## Physique et présence au sol

### SQ-71 · L'ordre de résolution des collisions fait partie de la règle
- **Ce qui arrive :** la séparation parcourt les paires dans l'ordre d'arrivée des joueurs. Changer cet ordre change les trajectoires. Les identifiants séquentiels de l'ancien jeu (au lieu des UUID de l'original) départagent aussi deux joueurs exactement superposés par ordre d'arrivée.
- **D'où on le sait :** `crates/core/src/physics.rs:1-8`, `:112-132` ; `LEARNINGS.md:61`.
- **Mécaniques :** physique.
- **Question :** dans quel ordre les corps sont-ils résolus, et cet ordre est-il stable d'une exécution à l'autre ?
- **Pression :** trajectoires de référence (scénario `crowd`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-72 · Deux corps exactement superposés se séparent sans valeur non finie
- **Ce qui arrive :** à distance nulle, la normale est 0/0. Sous 0,001 px, on prend une normale fixe, orientée par l'identifiant.
- **D'où on le sait :** `crates/core/src/physics.rs:35`, `:166-178`.
- **Mécaniques :** physique.
- **Question :** que fait la physique de deux corps au même point ?
- **Pression :** `separates_exact_overlapping_spawns_without_nan` (`crates/core/tests/original/physics.rs:86`), `a_long_evening_never_diverges` (100 000 pas, J43), J4.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-73 · Un nouveau personnage apparaît sur une place libre
- **Ce qui arrive :** l'original reprenait sa grille au début après 72 joueurs, et superposait les arrivants. Le nouveau cherche le premier point à 28 px de tout le monde (fantômes compris), puis une grille fine, puis le point le plus éloigné.
- **D'où on le sait :** `crates/core/src/game/player.rs:31-73` ; `LEARNINGS.md:115`.
- **Mécaniques :** physique, présence.
- **Question :** où apparaît un joueur dans une salle pleine ?
- **Pression :** `never_spawns_on_top_of_anyone_even_past_the_grid` (`crates/core/tests/original/rules.rs:550`), J5.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-74 · Un joueur coupé est un fantôme qui compte sans se voir
- **Ce qui arrive :** un personnage hors ligne ne bouge pas et ne bloque personne : on le traverse. La télé ne le dessine pas. Comme sa réponse compte (SQ-27), il compte dans une zone où personne ne le voit, jusqu'à son retour. Limite assumée.
- **D'où on le sait :** `crates/core/src/physics.rs:5-6` ; `LEARNINGS.md:553`.
- **Mécaniques :** présence, vote, affichage.
- **Question :** que voit la salle d'un joueur absent qui compte encore ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SQ-75 · La salle ne travaille que pour les appareils présents
- **Ce qui arrive :** les ensembles de télés et de téléphones ne rétrécissaient jamais. Une télé fermée coûtait encore une vue encodée à chaque pas, et le résumé comptait les absents.
- **D'où on le sait :** b6a06e1 ; `crates/host/src/room.rs:494-540` ; `LEARNINGS.md:499`.
- **Mécaniques :** présence.
- **Question :** le coût d'un pas dépend-il des appareils présents, ou de tous ceux jamais vus ?
- **Pression :** `the_room_tracks_the_devices_that_are_here` (`crates/host/src/room/simulation.rs:387`), J25.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-76 · Une soirée oublie les appareils partis pour de bon
- **Ce qui arrive :** rien ne disait à salon de lâcher un appareil. Chaque identité vue (un onglet privé, un stockage effacé) restait pour la soirée : 208 connues après six heures simulées, pour neuf présentes.
- **D'où on le sait :** 14bba35 ; `crates/host/src/room.rs:291-305`.
- **Mécaniques :** présence, identité.
- **Question :** que reste-t-il en mémoire après six heures de passages ?
- **Pression :** `an_evening_leaves_nothing_in_the_room` (`crates/host/src/room/simulation.rs:531`), qui compte les octets, J50.
- **Sous la nouvelle forme :** pression du commun. Loi « Mémoire bornée », mesurée.
- **Étage :** commun.

## Sauvegarde, reprise et effets

### SQ-77 · Un serveur relancé ne perd plus la partie
- **Ce qui arrive :** relancé, l'ancien serveur repartait de zéro, points compris. Il gardait seulement le code de salle et les identités, et chaque téléphone rejoignait seul avec le personnage retenu pour cette salle. La partie, elle, était perdue (ECO-04, limite connue).
- **D'où on le sait :** c57943b ; `LEARNINGS.md:173-174` ; `verification.toml`, décision ECO-04.
- **Mécaniques :** reprise.
- **Question :** sans objet.
- **Pression :** `a_restarted_host_keeps_its_room_and_its_devices`.
- **Sous la nouvelle forme :** impossible par la forme. Le commun conserve l'état et reprend la partie ; le jeu n'a rien à écrire.
- **Étage :** commun.

### SQ-78 · Une écriture qui échoue n'arrête pas la partie
- **Ce qui arrive :** le journal et les résultats s'écrivent sur disque. Une erreur ou une saturation arrête l'enregistrement avec une trace, sans arrêter le jeu, et la télé annonce qu'un résultat n'a pas été enregistré.
- **D'où on le sait :** `crates/host/src/room.rs:62`, `:600-608` ; `LEARNINGS.md:284`.
- **Mécaniques :** sauvegarde.
- **Question :** que se passe-t-il quand le disque refuse d'écrire ?
- **Pression :** `a_failed_save_is_reported_to_the_organizer` (`crates/host/tests/e2e.rs:550`), J19.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-79 · Un réaffichage ne rejoue ni son ni vibration
- **Ce qui arrive :** la vue des Résultats se renvoie à chaque reconnexion. Une page qui vibrait à chaque vue des Résultats vibrait à chaque retour ; rejoindre pendant les Résultats les annonçait. Les sons de la télé suivent des transitions de phase, pas des images.
- **D'où on le sait :** `crates/controller/src/session.rs` (`shown` : « Only a change counts ») ; `crates/screen/src/audio.rs:1-60` ; `LEARNINGS.md:398-401`.
- **Mécaniques :** effets.
- **Question :** sans objet.
- **Pression :** `repeating_a_view_emits_neither_actions_nor_duplicate_feedback` (`crates/controller/src/session.rs:424`), J21.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat : « Un effet ponctuel … porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas. »
- **Étage :** commun.

### SQ-80 · Le diagnostic se lit sans lire de pixels
- **Ce qui arrive :** le panneau `D` est dessiné dans le monde. Pour qu'un script lise la part d'images figées, son texte est copié dans un élément invisible de la page.
- **D'où on le sait :** `LEARNINGS.md:204` ; `crates/screen/src/web/access.rs`.
- **Mécaniques :** diagnostic.
- **Question :** une machine peut-elle lire les mesures du jeu sans capturer l'écran ?
- **Pression :** `tools/layout-check.mjs` lit le panneau par son « fps ».
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## L'épreuve et ses outils

### SQ-81 · Un contrôle qui lit un mot affiché casse à la traduction
- **Ce qui arrive :** après la traduction, le contrôle du GPU cherchait encore le mot anglais du message de rendu, et ne le trouvait plus.
- **D'où on le sait :** c88a9eb ; `LEARNINGS.md:541`.
- **Mécaniques :** toutes.
- **Question :** un contrôle dépend-il d'un texte destiné aux joueurs ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun. Les contrôles du commun lisent des valeurs typées, pas des mots affichés.
- **Étage :** commun.

### SQ-82 · Une borne de harnais tirée du contenu casse quand le contenu grandit
- **Ce qui arrive :** le contrôle de progression supposait qu'on revient au début du paquet en 60 s : vrai pour six questions, faux pour trente. Sa borne compte maintenant 20 changements distincts.
- **D'où on le sait :** `LEARNINGS.md:525`.
- **Mécaniques :** progression.
- **Question :** les bornes de l'épreuve viennent-elles de ce que le jeu déclare, ou d'hypothèses sur son contenu ?
- **Pression :** `tools/progression-check.mjs`, non relancé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-83 · Une empreinte régénérée ne prouve plus son origine
- **Ce qui arrive :** `state-hashes.txt` est régénéré à chaque changement voulu de l'état ; 217 de ses 351 lignes avaient changé. Il détecte une divergence entre compilations ou un changement involontaire ; il ne prouve plus la conformité au portage initial.
- **D'où on le sait :** f938ff9 ; `LEARNINGS.md:285`.
- **Mécaniques :** toutes.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-84 · Une mutation qui survit montre une entrée que rien ne teste
- **Ce qui arrive :** retirer la flèche droite et le `d` du clavier de la page ne cassait aucun test, parce que rien ne testait la droite. `soiree mutants` ne mutait que le cœur ; les deux pads ont été mutés à la main.
- **D'où on le sait :** b4190b6 ; `LEARNINGS.md:626-637`.
- **Mécaniques :** entrées.
- **Question :** chaque entrée possible a-t-elle un test qui échoue si on la retire ?
- **Pression :** `every_key_has_its_direction` (`crates/controller/src/pad.rs:97`) ; rapports `certification/mutants/`.
- **Sous la nouvelle forme :** pression du commun. L'épreuve injecte des bugs dans tout le code du jeu, page comprise.
- **Étage :** commun.

### SQ-85 · Des bots qui publient leur état en deux fois se bloquent
- **Ce qui arrive :** un bot publiait « connecté » et sa vue pendant son passage, son appareil à la fin. Un lecteur réveillé par « connecté » ne trouvait pas d'appareil ; dans `rejoin`, le verrou d'une condition vivait jusqu'à la fin de l'instruction et le message reverrouillait : interblocage, 3 fois sur 3 sous 30 ms de délai.
- **D'où on le sait :** 66821b0 ; `LEARNINGS.md:652`.
- **Mécaniques :** bots.
- **Question :** sans objet.
- **Pression :** scénario des bots, 12 sur 12 après correction.
- **Sous la nouvelle forme :** obsolète. Accident de bots à fils et à verrous hors du jeu ; le contrat fait jouer les bots dans le module, de façon déterministe.
- **Étage :** connaissance.

### SQ-86 · Une vue datée de l'instant d'arrivée ne se rejoue pas
- **Ce qui arrive :** l'hôte montrait à une télé qui arrive la salle à cet instant, datée de cet instant. Arrivée après un pas, cette vue portait le numéro du pas sans son heure : le rejeu ne pouvait pas la refaire. Le test supposait que c'était toujours le pas 0.
- **D'où on le sait :** 0102dd3 ; `LEARNINGS.md:653`.
- **Mécaniques :** toutes.
- **Question :** sans objet.
- **Pression :** `the_original_load_test_assertions_hold_and_the_journal_replays_them`, 15 sur 15 avec une télé retardée de 120 ms.
- **Sous la nouvelle forme :** impossible par la forme. Une vue se calcule depuis l'état d'un pas, et le temps n'avance que par pas.
- **Étage :** commun.

### SQ-87 · La télé et le téléphone ne voient pas l'état au même moment
- **Ce qui arrive :** le retour d'un personnage arrivait parfois au téléphone avant la vue suivante de la télé. Un test qui lisait la télé aussitôt voyait encore le joueur absent. Il attend désormais une arrivée télé postérieure qui contient ce joueur.
- **D'où on le sait :** 8332a61 ; `LEARNINGS.md:331-339`.
- **Mécaniques :** toutes.
- **Question :** comment un test attend-il qu'un appareil ait vu un pas donné ?
- **Pression :** deux régressions ordonnées sous verrou, sans sommeil.
- **Sous la nouvelle forme :** pression du commun. Le harnais du commun observe chaque appareil par numéro de pas.
- **Étage :** commun.

## L'app iPhone et le pad natif

### SQ-88 · L'app et sa page sont un seul joueur
- **Ce qui arrive :** le joueur de l'app et l'appareil de sa page n'étaient reliés nulle part. La solution proposée créait un second appareil, donc un second personnage. Le lien ne tenait qu'au hasard : la page prenait déjà le joueur de l'app comme jeton.
- **D'où on le sait :** `LEARNINGS.md:612` ; `crates/host/src/app.rs:1-12`.
- **Mécaniques :** identité, appareils.
- **Question :** une app et la page qu'elle affiche sont-elles un même appareil pour le jeu ?
- **Pression :** `a_native_stick_moves_a_character_next_to_a_browser_phone` (`crates/host/tests/e2e.rs:968`), J46.
- **Sous la nouvelle forme :** pression du commun. L'app remplit le même contrat d'appareil qu'une page.
- **Étage :** commun.

### SQ-89 · Une page fermée pour laisser place au pad n'est pas un départ
- **Ce qui arrive :** l'app ferme sa page pour montrer le pad. Le commun annonce alors un départ. Chaque jeu qui mêle page et pad doit réécrire une présence à deux sources : le personnage reste tant que la page ou le pad est là.
- **D'où on le sait :** `LEARNINGS.md:613` ; `crates/host/src/room.rs:514-526`.
- **Mécaniques :** présence, appareils.
- **Question :** quand un appareil change de surface, le jeu voit-il un départ ?
- **Pression :** J46.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-90 · Un geste du pad doit porter son heure
- **Ce qui arrive :** les états du pad n'avaient pas d'heure, et `/remote` ne mesurait aucun aller-retour. La borne d'une seconde ne s'appliquait pas au pad ; seul l'homme mort de 450 ms le bornait. Le diagnostic n'avait rien à montrer pour un joueur au pad.
- **D'où on le sait :** `LEARNINGS.md:614` ; `crates/host/src/room.rs:325-350`.
- **Mécaniques :** temps réel, appareils.
- **Question :** tous les appareils placent-ils leurs gestes dans le temps de la même façon ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-91 · Un pad sur TCP qui répète lentement lâche le personnage
- **Ce qui arrive :** la page a un canal non fiable et non ordonné ; le pad passait par un WebSocket, où un paquet perdu retient tout ce qui suit. Il répétait toutes les 250 ms contre 150 pour la page, sous un homme mort de 450 ms : une perte de 200 ms sur un pouce immobile lâche le personnage un instant. Supposé d'après le code, pas mesuré.
- **D'où on le sait :** `LEARNINGS.md:615`.
- **Mécaniques :** temps réel, réseau, appareils.
- **Question :** le rythme de répétition de chaque appareil laisse-t-il assez de marge sous la limite d'un état tenu, sur le pire canal ?
- **Pression :** aucune ; à mesurer sur un vrai réseau.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-92 · Revenir de la page au pad demande un geste
- **Ce qui arrive :** la page ne peut pas demander le pad. Il faut un événement de jeu : ici, un message envoyé ou la question suivante.
- **D'où on le sait :** `LEARNINGS.md:616` ; 7b41c23.
- **Mécaniques :** appareils.
- **Question :** comment un joueur passe-t-il d'une surface à l'autre, dans les deux sens ?
- **Pression :** `writing_goes_through_the_phone_page_and_back_to_the_stick` (`crates/host/tests/e2e.rs:1082`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SQ-93 · Ce que le jeu rend à chaque pas reste bon marché quand rien ne change
- **Ce qui arrive :** le commun demandait la disposition du pad après chaque pas, pour chaque app, et comparait tout. Quelques allocations par app, soixante fois par seconde, pour une réponse qui ne change presque jamais.
- **D'où on le sait :** `LEARNINGS.md:617`.
- **Mécaniques :** appareils.
- **Question :** ce que l'interface demande au jeu à chaque pas coûte-t-il quand rien n'a changé ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun. Chaque pas a un budget, mesuré.
- **Étage :** commun.

### SQ-94 · Un appareil natif a son faux client
- **Ce qui arrive :** la simulation de salon ne jouait que des pages. Le test de bout en bout du pad a dû écrire son client `/remote` à la main, sur un vrai WebSocket.
- **D'où on le sait :** `LEARNINGS.md:618`.
- **Mécaniques :** appareils.
- **Question :** chaque sorte d'appareil a-t-elle un faux client dans les pressions du commun ?
- **Pression :** `crates/host/tests/e2e.rs` (client écrit à la main).
- **Sous la nouvelle forme :** pression du commun. Le contrat : tous les appareils « passent les mêmes pressions ».
- **Étage :** commun.

### SQ-95 · Une télécommande garde la même disposition d'une phase à l'autre
- **Ce qui arrive :** l'app lâche tous les doigts à chaque nouvelle disposition. Un stick masqué aux Résultats obligerait à reposer le pouce au moment où la question suivante en a besoin. Changer de télécommande à chaque manche coûterait un rechargement et une rotation, soixante fois pour trente questions.
- **D'où on le sait :** `crates/host/src/pad.rs:22-37` ; `LEARNINGS.md:602-604`.
- **Mécaniques :** appareils, entrées.
- **Question :** sans objet.
- **Pression :** aucune ; à essayer sur un vrai iPhone.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-96 · Un stick a une zone morte et atteint la pleine vitesse avant sa butée
- **Ce qui arrive :** l'app ne filtre que la gigue du doigt (3 %). Un pouce posé ne doit pas marcher : zone morte de 12 %. Sur la croix, chaque appui va à pleine vitesse : un stick à 85 % ne doit pas perdre la course, d'où la pleine vitesse dès 90 %. Valeurs de départ, à régler au pouce.
- **D'où on le sait :** `crates/host/src/pad.rs:13-20`, `:39-50` ; `LEARNINGS.md:605`.
- **Mécaniques :** entrées.
- **Question :** sans objet.
- **Pression :** tests de `crates/host/src/pad.rs` ; le réglage, au pouce.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-97 · Une vibration pour ce qu'on veut sentir sans regarder la télé
- **Ce qui arrive :** marquer un point est le seul événement qu'on veut sentir sans lever les yeux. Le pad vibre pour lui seul ; une manche sans point ne dit rien.
- **D'où on le sait :** e668cb3 ; `LEARNINGS.md:606`.
- **Mécaniques :** effets, score.
- **Question :** sans objet.
- **Pression :** `the_pad_vibrates_for_a_point_and_for_nothing_else` (`crates/host/tests/e2e.rs:1147`), J47.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-98 · Une identité partagée entre jeux permet de se faire passer pour un joueur
- **Ce qui arrive :** dans l'app, la page prend le joueur de l'app comme jeton, pour garder son personnage si la vue web perd son stockage. Le prix : ce jeton est le même dans tous les jeux de la soirée, et chaque hôte le reçoit. Un hôte pourrait se faire passer pour ce joueur auprès d'un autre. Accepté pour une soirée entre amis.
- **D'où on le sait :** `LEARNINGS.md:126` ; `crates/controller/src/web/storage.rs:77-90`.
- **Mécaniques :** identité.
- **Question :** une identité stable peut-elle être rejouée par un autre jeu ?
- **Pression :** `the_apps_player_is_the_token_and_anything_else_is_not` (`crates/controller/src/web/storage.rs:125`).
- **Sous la nouvelle forme :** pression du commun. Le contrat : « un identifiant que seul le commun peut créer ».
- **Étage :** commun.

## La page téléphone

### SQ-99 · Un appui plus court qu'un envoi peut disparaître
- **Ce qui arrive :** la direction est un état. Un appui et son relâchement entre deux envois ne laissent aucune trace. Laissé ouvert (TR-03) : seul un vrai téléphone dira si c'est gênant sur la croix.
- **D'où on le sait :** `verification.toml`, décision TR-03 ; `LEARNINGS.md:579`.
- **Mécaniques :** temps réel, entrées tenues.
- **Question :** un geste très bref sur un état tenu doit-il produire un effet, et lequel ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SQ-100 · Un message de validité s'attache au bon champ
- **Ce qui arrive :** l'original affichait « Choose a nickname. » sur le premier champ du formulaire, qui était celui du code de salle quand il était visible. Le formulaire restait bloqué jusqu'au rechargement.
- **D'où on le sait :** `LEARNINGS.md:74`.
- **Mécaniques :** texte saisi.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SQ-101 · La page tient à 320 px
- **Ce qui arrive :** à 390 px, la page du sélecteur débordait horizontalement, faute de `box-sizing` sur son conteneur. Le sélecteur doit marcher à 320 px, avec des boutons d'accessoire de 72 px.
- **D'où on le sait :** original f245c30, 2bf9d9b ; `archive/research/history.md`, leçon 31.
- **Mécaniques :** page téléphone.
- **Question :** la page tient-elle sans défilement horizontal au plus étroit des téléphones ?
- **Pression :** captures `tools/visual/golden/phone/*-320x700.png`.
- **Sous la nouvelle forme :** pression du commun. Le commun peut rendre chaque page aux largeurs de téléphone et refuser un débordement.
- **Étage :** commun.

### SQ-102 · Le bouton de saut porte l'avatar du joueur
- **Ce qui arrive :** dans une foule, chacun se demande « lequel suis-je ? ». Le bouton de saut montre son personnage, dans sa couleur, et le fait bondir à l'appui.
- **D'où on le sait :** original 0cb2b61, `DESIGN.md:9`, `:14` ; `crates/controller/src/web/remote.rs:174`.
- **Mécaniques :** page téléphone.
- **Question :** sans objet.
- **Pression :** captures `tools/visual/golden/phone/remote-*.png`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.
