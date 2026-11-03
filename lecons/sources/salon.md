# Leçons de salon, la couche réseau de l'ancien commun

## La source

**Couvert :** `soiree/salon`, tel qu'il est sur la branche `refonte` (identique à l'étiquette `avant-refonte` pour ces fichiers). C'est la couche qui reliait l'hôte natif, la page TV, les téléphones (navigateur ou app iPhone) et les manettes : WebRTC, WebSocket, reconnexion, identité, temps, fiabilité, pare-feu, iOS, et les coques autour (serveur, page, audio, journal).

**Lu en entier :**
- `salon/LEARNINGS.md` (513 lignes) ;
- `salon/README.md` (391 lignes) ;
- `salon/crates/core/src/lib.rs` (298 lignes, la spécification) ;
- `salon/crates/core/tests/found_bugs.rs` (236 lignes) ;
- `salon/AGENTS.md` ;
- les messages complets des 119 commits qui touchent `salon/` (`git -C soiree log -- salon`).

**Lu en partie :**
- dans `crates/core/src/types.rs` : `Timing`, `Limits`, `Why`, `Kind`, `Name`, `SleepClock` ;
- les commentaires de tête de `crates/web/src/lib.rs`, `rtc_memory.rs`, `wake.rs`, `journal.rs` (lignes 255 à 275), et de `crates/host/src/lib.rs`, `rtc.rs`, `address.rs`, `admission.rs`, `store.rs`, `ws.rs` ;
- les noms des tests de `crates/core/tests/`, `crates/host/tests/` et `crates/crypto/tests/` ;
- les seuls titres des trous (T1, T2…) de `recherche/reconnexion.toml`, `recherche/fiabilite.toml` et `recherche/transport-webrtc.toml`, pour citer juste.

**Pas lu :**
- le corps des fiches `recherche/*.toml` : c'est une source à part, avec ses propres références ;
- le diff complet des gros commits sans message (`de110bc`, `d868b76`, `b226c97`, `0acc4e3`, `0834714`, `08af13e`, `a002ab2`, `3d45317`) : leur contenu est repris par `LEARNINGS.md` ;
- le code de `client.rs`, `host.rs`, `stream.rs`, `wire.rs`, et les corps des tests ;
- les dossiers `certification/` (dont `ci-timers-2026-10-29` et `robustesse-2026-10-29`), `remote/` et `CHANGELOG.md` ;
- les références externes elles-mêmes (str0m, Colyseus, Nakama, RFC…) : elles sont citées ici de seconde main, telles que `LEARNINGS.md` les rapporte.

**À savoir pour lire :** les codes C1 à C10 et E1 à E6 sont les items de l'ancien `CERTIFICATION.md`. « L'hôte » est l'ancien programme natif qui tenait la partie. Dans le nouveau contrat, la partie qui fait foi tourne dans le navigateur de l'ordinateur de la soirée, et un relais, local ou en ligne, met les appareils en relation avec un chiffrement de bout en bout. Beaucoup de leçons de l'hôte se transposent donc au relais ou à la page qui fait foi ; chaque leçon le dit quand c'est le cas.

## Le cœur réseau et le temps

### SAL-01 · Le cœur réseau est une fonction de ses entrées, sans entrées-sorties
- **Ce qui arrive :** des rappels (callbacks) imposent en Rust des verrous partagés, et empêchent de faire tourner le même code sur un faux réseau. Salon avait un cœur `no_std`, sans horloge, socket ni fil, piloté par interrogation (`poll`) ; des coques traduisaient le monde en appels.
- **D'où on le sait :** `salon/LEARNINGS.md:17` (str0m `src/lib.rs`, quinn-proto) ; `salon/crates/core/src/lib.rs:9-14` ; `LEARNINGS.md:329`.
- **Mécaniques :** toutes.
- **Question :** le cœur réseau du commun tourne-t-il sans entrées-sorties, pour être joué à vitesse machine sur un réseau simulé ?
- **Pression :** les tests de `crates/core/tests/` sur le faux réseau à graine de `crates/core/tests/net/`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-02 · Un contrat d'appel tolérant : ordre libre, appel en trop sans effet, échéance lue à part
- **Ce qui arrive :** une bibliothèque qui exige un ordre strict d'appels finit par des bugs chez ses appelants. Salon acceptait les entrées dans n'importe quel ordre, un `tick` de trop ne faisait rien, et la prochaine échéance se lisait à part.
- **D'où on le sait :** `LEARNINGS.md:18` (quinn-proto #924 ; str0m 81a14a25) ; `README.md:218` et commit `9691763` (le chaos polle une fois sur deux aux seules échéances, comme les coques).
- **Mécaniques :** toutes.
- **Question :** le commun supporte-t-il un appel en trop, ou un appel seulement aux échéances, sans changer de comportement ?
- **Pression :** `chaos_keeps_every_page_in_its_place` (`crates/core/tests/properties.rs`), qui alterne les deux façons d'appeler.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-03 · str0m exige de vider ses sorties après chaque entrée
- **Ce qui arrive :** str0m pose un invariant de « mutation unique » : chaque entrée doit être suivie de la vidange complète de ses sorties avant la suivante. Sinon son état se corrompt.
- **D'où on le sait :** `salon/crates/host/src/rtc.rs:8-10` (cite `str0m/src/lib.rs`) ; `LEARNINGS.md:18`.
- **Mécaniques :** toutes (transport WebRTC).
- **Question :** si le relais garde str0m, chaque entrée est-elle suivie d'une vidange ?
- **Pression :** aucune dédiée connue.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-04 · Le temps est absolu, fourni par l'appelant dès la construction
- **Ce qui arrive :** des deltas de temps ont caché l'arrêt d'un serveur (renet) ; une sentinelle « déjà passé » a purgé des paires ICE (str0m) ; `Instant` panique en WebAssembly (quinn).
- **D'où on le sait :** `LEARNINGS.md:19` (renet #3 ; str0m #832, #843 ; quinn #2436) ; `lib.rs:30`.
- **Mécaniques :** toutes.
- **Question :** chaque composant du commun reçoit-il un temps absolu dès sa création, jamais un delta ni une sentinelle ?
- **Pression :** la règle d'écriture qui interdit `Instant::now` hors du module de l'horloge.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-05 · Le temps ne recule jamais
- **Ce qui arrive :** une horloge peut rendre une valeur plus petite que la précédente. Salon ramenait un `now` plus petit au précédent, et l'état ne s'altérait pas.
- **D'où on le sait :** `LEARNINGS.md:20` (str0m `do_handle_timeout`) ; `README.md:294`.
- **Mécaniques :** toutes.
- **Question :** que fait le commun d'un temps qui recule ?
- **Pression :** `time_never_goes_back`, `sleep_adds_up_and_time_never_goes_back` (`crates/core/tests/salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-06 · Aucune file sans borne
- **Ce qui arrive :** une file sans borne grandit jusqu'à épuiser la mémoire sous une rafale ou un pair muet.
- **D'où on le sait :** `LEARNINGS.md:21` (quinn #1131, #1729 ; renet `max_memory_usage_bytes`) ; `lib.rs:158-161`.
- **Mécaniques :** toutes.
- **Question :** chaque file du commun a-t-elle une borne, et que se passe-t-il quand on l'atteint ?
- **Pression :** `the_room_has_a_limit`, `what_waits_for_a_device_is_bounded_in_bytes` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-07 · Une file pleine rend une erreur, elle ne jette rien en silence
- **Ce qui arrive :** Colyseus jetait le plus ancien message d'une file « fiable » pleine : une perte silencieuse. Salon rendait `SendError::Full` à l'appelant.
- **D'où on le sait :** `LEARNINGS.md:75` (Colyseus `enqueueMessage`) ; `lib.rs:158-159` ; `README.md:96`.
- **Mécaniques :** événements comptés une fois (réponse, vote, buzzer).
- **Question :** quand une file du commun déborde, qui l'apprend ?
- **Pression :** `the_room_has_a_limit` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-08 · Une connexion de transport est jetable ; l'identité vit au-dessus
- **Ce qui arrive :** quand l'identité est liée au socket, chaque coupure fait un nouveau joueur. Salon séparait l'appareil de son lien.
- **D'où on le sait :** `LEARNINGS.md:22` (renet #59) ; `lib.rs:18-21`.
- **Mécaniques :** toutes.
- **Question :** un appareil garde-t-il son identité quand son lien change de transport ou meurt ?
- **Pression :** `a_cut_phone_comes_back_as_itself_without_losing_an_event`, `a_reloaded_page_gets_its_device_back` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-09 · Un banc à horloge simulée, qui saute à l'échéance suivante, borné en pas, à graine fixe
- **Ce qui arrive :** un test réseau en temps réel est lent et instable. Le banc de quinn-proto saute à la prochaine échéance, borne le nombre de pas et part d'une graine.
- **D'où on le sait :** `LEARNINGS.md:23` (quinn-proto `tests/util.rs`) ; `README.md:218`.
- **Mécaniques :** toutes.
- **Question :** le réseau simulé du commun avance-t-il par échéances, de façon reproductible ?
- **Pression :** `crates/core/tests/net/`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-10 · Le faux réseau exécute la boucle de production, pas une réécriture
- **Ce qui arrive :** une simulation qui réécrit la boucle de production éprouve un autre programme. Salon appelait le même `Driver` depuis la vraie salle et depuis `simulation::Network`, avec la même priorité entre réseau, pas et événements.
- **D'où on le sait :** `LEARNINGS.md:176-180` (FoundationDB, *Simulation and Testing*) ; `README.md:288-302`.
- **Mécaniques :** toutes.
- **Question :** la simulation du commun fait-elle tourner exactement le code de la partie qui fait foi ?
- **Pression :** `crates/host/tests/evening.rs`, qui joue la vraie salle sur ce réseau.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-11 · Une échéance dit quand quelque chose peut partir, pas quand c'est dû
- **Ce qui arrive :** une échéance annoncée pour ce qu'une fenêtre retient encore donne « maintenant » sans fin, et fait tourner à vide le seul fil de la salle. `next_deadline` et ce qui part vraiment doivent lire la même règle.
- **D'où on le sait :** `LEARNINGS.md:65` ; commits `bd88902`, `c8507d0`, `b73bc5d`.
- **Mécaniques :** toutes.
- **Question :** une échéance passée peut-elle laisser `poll` sans rien à rendre ?
- **Pression :** le banc tient pour une erreur une échéance passée alors que `poll` a rendu `None` (`crates/core/tests/net/mod.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-12 · Un réglage de temps se ramène à ce que le code peut tenir
- **Ce qui arrive :** `Timing` était public et non vérifié. Avec `resend_min` au-dessus de `resend_max`, `clamp` paniquait au premier aller-retour mesuré, sur le fil de la salle. Une attente nulle faisait que `poll` ne finissait jamais de rendre ce qui était dû.
- **D'où on le sait :** commits `941eee9`, `81c54b1` ; `crates/core/src/types.rs:273-286` ; `recherche/fiabilite.toml` T12.
- **Mécaniques :** toutes.
- **Question :** un réglage incohérent venu de l'extérieur peut-il arrêter la partie ?
- **Pression :** `a_timing_the_wrong_way_round_does_not_stop_the_room` (`salon.rs`).
- **Sous la nouvelle forme :** impossible par la forme : « ce qui vient de l'extérieur est validé à l'entrée » par des types (contrat, « Les lois communes »).
- **Étage :** commun.

### SAL-13 · Calculer la prochaine échéance n'alloue rien
- **Ce qui arrive :** `due_at` construisait un vecteur par événement en attente : 6 144 octets à chaque appel avec une file pleine, pour chaque appareil, à chaque tour de boucle.
- **D'où on le sait :** commit `01e148f` ; `recherche/fiabilite.toml` T13.
- **Mécaniques :** toutes.
- **Question :** les chemins appelés à chaque tour de boucle allouent-ils ?
- **Pression :** `the_next_deadline_allocates_nothing` (`crates/core/tests/stream_cost.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## L'identité et la présence

### SAL-14 · Un identifiant public stable, jamais réattribué, et un secret à part ; le prénom n'est jamais une clé
- **Ce qui arrive :** un prénom change, se répète et se tape mal ; un identifiant réattribué donne la place d'un joueur à un autre. Toutes les références séparent un identifiant public d'un secret.
- **D'où on le sait :** `LEARNINGS.md:29` (AirConsole `getDeviceId` ; Jackbox `id:role:secret` ; Colyseus `sessionId`/`reconnectionToken` ; boardgame.io `credentials`) ; `lib.rs:67-68`.
- **Mécaniques :** toutes.
- **Question :** le commun distingue-t-il l'identifiant qu'il montre au jeu du secret qui le prouve ?
- **Pression :** `a_phone_token_never_becomes_the_tv`, `a_reloaded_page_gets_its_device_back` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-15 · Utilisateur, session et présence sont trois choses
- **Ce qui arrive :** un jeu indexé par session crée un fantôme à chaque reconnexion.
- **D'où on le sait :** `LEARNINGS.md:30` (Nakama `tracker.go`) ; `README.md:80`.
- **Mécaniques :** toutes.
- **Question :** le jeu peut-il voir une session ou un lien, ou seulement un appareil ?
- **Pression :** aucune nécessaire.
- **Sous la nouvelle forme :** impossible par la forme : le jeu ne voit que « des appareils qui arrivent, partent et reviennent », avec « un identifiant que seul le commun peut créer » (contrat, « Ce que le commun possède »).
- **Étage :** commun.

### SAL-16 · Une coupure n'est pas un départ
- **Ce qui arrive :** Colyseus a mis des années à séparer `onDrop` de `onLeave` ; boardgame.io n'a eu `onPlayerLeave` qu'en 2026. Salon disait la cause (silence, fermeture, départ voulu) et ne libérait jamais une place lui-même.
- **D'où on le sait :** `LEARNINGS.md:31` (Colyseus #348, #893 ; boardgame.io #1088, #1258) ; `lib.rs:52-54` ; `types.rs` (`Why`).
- **Mécaniques :** toutes ; tours, équipes, places.
- **Question :** que veut dire l'absence d'un joueur dans ce jeu : pause, tour sauté, place gardée ?
- **Pression :** `a_player_who_quits_leaves_for_that_reason` (`salon.rs`) pour la cause ; C6 (« chaque phase finit à son échéance ») restait à relier.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SAL-17 · Valider le jeton avant tout effet de bord
- **Ce qui arrive :** chez Colyseus, un mauvais jeton suffisait à bloquer une salle.
- **D'où on le sait :** `LEARNINGS.md:32` (Colyseus #962).
- **Mécaniques :** toutes.
- **Question :** un message d'arrivée invalide peut-il modifier quoi que ce soit ?
- **Pression :** `refusals_are_clear_and_final` (`salon.rs`) ; `fuzz/` (session hostile).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-18 · Le lien le plus récent gagne, et l'éjecté ne se reconnecte pas seul
- **Ce qui arrive :** sans message terminal, deux onglets du même joueur s'éjectent en boucle. Salon disait « remplacé » à l'ancien, qui attendait un geste (« Jouer ici »).
- **D'où on le sait :** `LEARNINGS.md:33` (Nakama e5f52b53 `single_socket` ; Colyseus code 4002 ; Punchline) ; `lib.rs:87-90` ; C5.
- **Mécaniques :** toutes.
- **Question :** que voit un onglet remplacé, et peut-il reprendre la place sur un geste ?
- **Pression :** `a_second_tab_takes_over_and_the_first_stays_put`, `two_tabs_opened_together_leave_one_playing` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-19 · « Le plus récent » se lit dans l'ordre d'ouverture des liens, pas d'arrivée des hello
- **Ce qui arrive :** un téléphone ouvre un lien, son hello reste coincé dans le socket, le réseau revient et il en ouvre un autre. TCP livre alors l'ancien hello avant l'ancienne fermeture : le lien vivant était déclaré remplacé par la tentative abandonnée du téléphone lui-même, et rien ne le ramenait. Même chose avec la page d'avant un rechargement.
- **D'où on le sait :** commit `658b388` ; `lib.rs:87-93`.
- **Mécaniques :** toutes.
- **Question :** un message tardif d'un lien plus ancien peut-il déloger le lien courant ?
- **Pression :** `a_hello_stuck_on_an_abandoned_attempt_does_not_replace_the_link_after_it`, `a_hello_of_the_page_before_a_reload_does_not_replace_the_new_page`, `of_two_tabs_the_newest_link_wins_even_when_its_hello_comes_first`, `a_hello_held_back_leaves_one_tab_playing` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-20 · Chaque rappel vérifie que sa connexion est encore la courante
- **Ce qui arrive :** chez Colyseus, une fermeture tardive tuait la connexion revenue.
- **D'où on le sait :** `LEARNINGS.md:34` (Colyseus #950, `client.ref === ref`).
- **Mécaniques :** toutes.
- **Question :** une fermeture ou une réponse tardive d'un ancien lien peut-elle toucher le nouveau ?
- **Pression :** `a_slow_close_does_not_touch_the_link_that_followed` (`crates/host/tests/transport_faults.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-21 · Un lien qui a reçu son message final ne dit plus rien
- **Ce qui arrive :** un hello sur un lien déjà refusé ou remplacé refaisait toute l'arrivée. Sur un lien remplacé, il reprenait l'appareil au lien courant : deux onglets pouvaient s'éjecter pour de bon. Sur un lien refusé, il créait un appareil fantôme qui tenait une place sans jamais de départ : 48 paquets fermaient la salle à tout le monde.
- **D'où on le sait :** commits `34441dd`, `3eada04` ; `salon/crates/core/tests/found_bugs.rs:71-138` ; `LEARNINGS.md:310` (socket.io 46213a6) ; `recherche/reconnexion.toml` T3.
- **Mécaniques :** toutes.
- **Question :** après un refus ou un remplacement, que fait le commun de ce qui arrive encore sur ce lien ?
- **Pression :** `a_hello_on_a_refused_link_is_not_heard`, `ghosts_do_not_fill_the_room`, `a_late_hello_on_a_replaced_link_does_not_take_the_device_back` (`found_bugs.rs`) ; `a_late_hello_on_a_link_refused_for_a_full_room_takes_no_seat`, `a_hello_after_a_refusal_makes_no_device` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-22 · La présence se déduit, elle ne se compte pas à la main
- **Ce qui arrive :** chez Colyseus, un compteur de clients tenu à la main divergeait de la réalité.
- **D'où on le sait :** `LEARNINGS.md:35` (Colyseus #423, a00d3e3).
- **Mécaniques :** lobby, salle pleine.
- **Question :** le nombre d'appareils présents se calcule-t-il depuis une seule table ?
- **Pression :** `the_room_has_a_limit` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-23 · Quand l'autorité s'arrête, elle le dit à tout le monde
- **Ce qui arrive :** chez Nakama, la mort de l'autorité laissait les écrans figés. Salon envoyait un message de fermeture, et la page affichait « l'hôte est parti ».
- **D'où on le sait :** `LEARNINGS.md:36` (Nakama #414) ; `lib.rs:139-140` (`Host::shutdown`).
- **Mécaniques :** toutes.
- **Question :** quand la partie qui fait foi s'arrête proprement, chaque appareil l'apprend-il ?
- **Pression :** `a_graceful_shutdown_tells_the_phones` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-24 · Un composant mort fait tomber ce qui dépend de lui, au lieu de le laisser répondre dans le vide
- **Ce qui arrive :** le fil de la salle n'avait pas de propriétaire. Après une panique du jeu ou une attente système échouée, le serveur HTTP répondait encore, et les téléphones et la TV réessayaient pour toujours. Un observateur joint désormais le fil, écrit la cause et termine le processus avec un code fixe ; les pages reviennent seules à la relance.
- **D'où on le sait :** commit `1165a5e` ; `README.md:88-92`.
- **Mécaniques :** toutes.
- **Question :** si la partie qui fait foi meurt, le relais et les appareils le savent-ils, ou restent-ils à attendre ?
- **Pression :** `a_dead_room_ends_the_process` (`crates/host/tests/room_death.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-25 · Une époque tirée à chaque lancement, jamais comptée depuis un fichier
- **Ce qui arrive :** une époque d'hôte dit au client que l'hôte a redémarré et qu'il doit oublier ce qu'il savait. Comptée depuis `salon.json`, elle revenait à 1 quand le fichier était perdu, et restait à 1 dans un dossier non inscriptible. La page prenait la nouvelle exécution pour l'ancienne : pas de remise à zéro, pas d'état, et ce qu'elle envoyait était accusé puis perdu, après un `send` réussi.
- **D'où on le sait :** commit `adca835` ; `LEARNINGS.md:37` (Colyseus DevMode `nextRefId` ; tier-list, bug 9) ; `crates/host/src/store.rs:5-8` ; `crates/host/src/lib.rs:30-32`.
- **Mécaniques :** toutes.
- **Question :** un appareil distingue-t-il toujours une nouvelle exécution de l'ancienne, même si le disque a tout perdu ?
- **Pression :** `a_restart_without_memory_makes_new_devices`, `a_restarted_host_gets_its_phones_back` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-26 · Un identifiant ne resert jamais, même après un arrêt brutal
- **Ce qui arrive :** la salle sauvait les identités une fois par seconde. Un appareil accueilli juste avant un plantage était inconnu de l'exécution suivante, et son numéro allait à un autre. Une identité neuve atteint désormais le disque avant son accueil. Un arrêt brutal tombe entre deux écritures : ce qui n'est pas écrit n'a jamais existé. Il reste deux trous : sans `fsync`, une coupure de courant n'est pas couverte ; si l'écriture échoue, l'accueil part quand même et la sauvegarde est retentée chaque seconde.
- **D'où on le sait :** commit `0eae554` ; `LEARNINGS.md:319` (Jepsen `db.clj`) ; `lib.rs:128-133` ; `recherche/reconnexion.toml` T10.
- **Mécaniques :** toutes.
- **Question :** dans quel ordre le commun écrit-il une identité et l'annonce-t-il ?
- **Pression :** `a_restarted_host_gets_its_phones_back` (`salon.rs`) ; pas de test de coupure de courant.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-27 · Un fichier d'état s'écrit dans un fichier temporaire, puis se renomme
- **Ce qui arrive :** un plantage pendant l'écriture laisse un fichier à moitié écrit. Le renommage garde l'ancien fichier entier.
- **D'où on le sait :** `crates/host/src/store.rs:1-3`.
- **Mécaniques :** sauvegardes.
- **Question :** une sauvegarde interrompue laisse-t-elle un fichier lisible ?
- **Pression :** aucune connue.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-28 · À l'arrivée, l'état complet d'abord, les événements ensuite
- **Ce qui arrive :** une reconnexion qui ajoute l'état au lieu de le remplacer montre un écran faux. Colyseus, boardgame.io et Jackbox envoient l'état entier à chaque accueil. Salon renvoyait le dernier état à chaque nouveau lien.
- **D'où on le sait :** `LEARNINGS.md:38` (Colyseus `_onMessage`, fb3a8af ; boardgame.io #727 ; Jackbox `welcome`) ; `lib.rs:25-29`.
- **Mécaniques :** toutes.
- **Question :** un appareil qui revient reçoit-il l'état entier avant tout événement ?
- **Pression :** `a_reloaded_page_gets_its_device_back`, `a_cut_phone_comes_back_as_itself_without_losing_an_event` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-29 · Rejoindre est idempotent
- **Ce qui arrive :** un accusé d'arrivée perdu pousse le client à répéter sa demande. Répondre « accepté » à chaque demande la rend sans danger.
- **D'où on le sait :** `LEARNINGS.md:39` (Nakama #245 ; Fiedler, *Client Server Connection*) ; `lib.rs:77-78`.
- **Mécaniques :** toutes.
- **Question :** une demande d'arrivée répétée change-t-elle quelque chose ?
- **Pression :** `a_repeated_hello_changes_nothing` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-30 · Tout message de contrôle tient dans le plus petit paquet
- **Ce qui arrive :** le prénom était le seul champ du hello sans borne. Au-delà d'un paquet, l'encodage abandonnait et le téléphone restait sur « Connexion… » pour toujours, sans erreur. Un autre bug, trouvé par un mutant, abandonnait à la première liste vide et aurait perdu un message de contrôle à côté d'événements en attente.
- **D'où on le sait :** commits `42b2271`, `dcd78b4` ; `found_bugs.rs:174-197` ; `lib.rs:78-79`.
- **Mécaniques :** toutes.
- **Question :** chaque message de contrôle, à sa taille maximale, part-il sur le transport le plus étroit ?
- **Pression :** `a_name_too_long_for_a_packet_does_not_hang_the_phone` (`found_bugs.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-31 · Un prénom se nettoie d'une seule façon, des deux côtés
- **Ce qui arrive :** un prénom coupé par le téléphone pourrait différer de celui que l'hôte garde. Salon retirait les caractères de contrôle, les espaces autour, coupait à une longueur fixe, et un prénom vide après nettoyage devenait « sans prénom ».
- **D'où on le sait :** `found_bugs.rs:199-235` ; `types.rs` (`Name`).
- **Mécaniques :** lobby, prénoms.
- **Question :** le prénom affiché dépend-il de qui l'a coupé ?
- **Pression :** `a_name_cut_by_the_client_is_the_one_the_host_would_keep`, `a_name_with_nothing_left_joins_without_a_name` (`found_bugs.rs`).
- **Sous la nouvelle forme :** impossible par la forme : un seul type de prénom validé, partagé (contrat, « Identité : des prénoms validés » et « Les types de bout en bout »).
- **Étage :** commun.

### SAL-32 · Un stockage cassé fait un appareil neuf, jamais une page qui ne s'ouvre pas
- **Ce qui arrive :** le jeton se tire une fois du hasard sûr du système et se garde dans `localStorage`. Un stockage plein, cassé ou interdit existe. Salon tirait alors un nouveau jeton : l'appareil arrivait comme neuf. `?seat=2` donnait une autre identité dans le même navigateur, pour les essais.
- **D'où on le sait :** `lib.rs:67-68` ; `salon/crates/web/src/lib.rs:6-8` ; `README.md:173` ; C9.
- **Mécaniques :** toutes.
- **Question :** que fait la page quand son stockage est inaccessible ou corrompu ?
- **Pression :** scénario `storage` de l'ancienne certification (C9).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-33 · Seul l'ordinateur de la soirée peut tenir le rôle de la TV
- **Ce qui arrive :** un téléphone ne doit pas pouvoir devenir l'écran qui compte. Salon servait une clé à la page TV seulement par la boucle locale, avec un `Host` local, sans `Origin` étranger ni en-tête de proxy. Une clé rejouée depuis le réseau était refusée. Les actions de l'organisateur passaient par la TV.
- **D'où on le sait :** `crates/host/src/lib.rs:19-21` ; `lib.rs:68-71` ; `README.md:86` ; E4.
- **Mécaniques :** toutes ; actions d'organisateur.
- **Question :** qui peut prétendre tenir la partie qui fait foi, et comment le commun le constate-t-il ?
- **Pression :** `a_phone_token_never_becomes_the_tv`, `a_welcomed_phone_cannot_turn_into_the_tv`, `the_tv_token_never_becomes_a_phone`, `a_tv_refused_for_an_old_key_comes_back_with_the_new_one` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-34 · Oublier un appareil n'est pas fermer la porte
- **Ce qui arrive :** la doc de `forget` promettait que le jeton ne marchait plus ; c'était faux, et Punchline comme le modèle comptaient sur le contraire : un appareil oublié revient en appareil neuf. Colyseus laisse l'autorité refuser un retour. Chez salon, seule la fermeture de la salle gardait dehors.
- **D'où on le sait :** commit `0eae554` ; `LEARNINGS.md:323` (Colyseus f7831c7) ; `README.md:83` ; `recherche/reconnexion.toml` T13.
- **Mécaniques :** lobby, éjection d'un joueur.
- **Question :** un joueur retiré par le jeu peut-il revenir, et sous quelle identité ?
- **Pression :** `a_forgotten_device_comes_back_as_a_new_one`, `a_closed_room_keeps_a_forgotten_device_out` (`salon.rs`).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SAL-35 · Les identités retenues n'ont pas de plafond, et c'est voulu
- **Ce qui arrive :** chaque jeton neuf ajoute une identité gardée d'une soirée à l'autre. Une limite de débit laisserait une rafale de jetons fermer la salle aux vrais joueurs. Coût mesuré : environ 90 octets par identité, un fichier réécrit en 1,2 ms pour 10 000 et 12 ms pour 100 000, sur SSD, sur le fil de la salle.
- **D'où on le sait :** commit `7b8ea01` ; `README.md:99` ; `lib.rs:159-161`.
- **Mécaniques :** toutes.
- **Question :** ce que le commun retient sans borne est-il écrit, avec son coût ?
- **Pression :** `ghosts_do_not_fill_the_room` (`found_bugs.rs`) pour la rafale.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-36 · Les onglets d'un même appareil ne partagent pas leurs numéros ; seule l'autorité peut les distinguer
- **Ce qui arrive :** deux onglets montrent le même jeton, mais chacun a ses compteurs. Après « Jouer ici », un onglet numérotait sous ce que l'autre avait envoyé : ses événements étaient ignorés et accusés, et ce qui attendait était jeté. Comparer les compteurs ne suffit pas : l'autre onglet a pu envoyer moins. L'hôte numérote chaque session qui prend l'appareil, et lui dit ce qui était arrivé de son ancienne numérotation.
- **D'où on le sait :** commit `62799ad` ; `LEARNINGS.md:308` (socket.io 121fd7c) ; `lib.rs:93-99` ; `recherche/reconnexion.toml` T2.
- **Mécaniques :** événements comptés une fois.
- **Question :** quand deux instances montrent la même identité, qui décide de la numérotation ?
- **Pression :** `play_here_keeps_counting_after_the_other_tab_sent_events`, `what_waits_in_a_tab_survives_the_other_one_sending_less`, `an_event_that_arrived_is_not_repeated_by_the_tab_that_takes_back`, `play_here_is_heard_and_says_what_this_tab_holds` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-37 · La mémoire des onglets passés est bornée, exacte, et ne compte que ceux qui ont livré quelque chose
- **Ce qui arrive :** un accueil perdu crée une session qui n'a rien livré ; quatre suffisaient à oublier où en étaient les numéros d'un onglet. Seules les sessions qui ont livré prennent une place, huit au plus. La borne pouvait passer de 8 à 1, 7 ou 9 avec tout au vert : elle est maintenant tenue des deux côtés. Un onglet absent plus longtemps peut répéter ce qu'il avait en vol, sans rien perdre.
- **D'où on le sait :** commits `17b8d5c`, `89361d7` ; `lib.rs:97-99`.
- **Mécaniques :** événements comptés une fois.
- **Question :** une borne du commun est-elle tenue exactement, ou seulement « à peu près » par les tests ?
- **Pression :** `the_host_remembers_exactly_the_last_eight_tabs_that_sent_something`, `tabs_that_send_nothing_do_not_count_among_the_eight`, `a_tab_away_for_many_takeovers_loses_nothing` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-38 · Quand un seul côté repart de zéro, l'accueil réaligne chaque compteur
- **Ce qui arrive :** une page rechargée numérotait ses états à partir de 1, alors que l'hôte gardait les numéros de l'ancienne page : chaque état était ignoré, accusé, et `set_state` rendait Ok. Le jeu n'entendait plus les commandes d'un téléphone. L'accusé rendait ce silence invisible.
- **D'où on le sait :** commit `72591fe` ; `LEARNINGS.md:307` (Colyseus a22c6c5) ; `found_bugs.rs:140-172` ; `lib.rs:104-108` ; `recherche/reconnexion.toml` T1.
- **Mécaniques :** états tenus (manette, stick) ; événements.
- **Question :** après un rechargement, le premier message de la page est-il entendu ?
- **Pression :** `a_reloaded_page_is_heard_when_it_sets_its_state` (`found_bugs.rs`) ; `a_reloaded_page_is_heard_at_once`, `a_state_set_before_the_welcome_of_a_reloaded_page_counts` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-39 · Une nouvelle session abandonne ce qui est à moitié reçu
- **Ce qui arrive :** repartir de zéro réutilise des numéros. Un message à moitié reçu de l'ancienne page se complétait avec les morceaux de la nouvelle, et faisait un message fait de deux.
- **D'où on le sait :** `LEARNINGS.md:309` ; commit `72591fe` ; `lib.rs:107-108`.
- **Mécaniques :** gros messages, événements.
- **Question :** un message découpé peut-il mêler les morceaux de deux sessions ?
- **Pression :** `a_half_sent_event_of_the_old_page_is_not_mixed_with_the_new_ones`, `a_half_sent_state_of_the_old_page_is_not_mixed_with_the_new_ones` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-40 · Une page d'un autre protocole est refusée, même quand son hello ne se lit plus
- **Ce qui arrive :** une page restée ouverte pendant un changement des paquets envoie un hello que l'hôte ne sait plus lire. Sans refus clair, elle se reconnecte pour toujours. Seul un hello cassé d'un autre protocole est invité à recharger ; un hello cassé du protocole courant est du bruit.
- **D'où on le sait :** commits `72591fe`, `89361d7` ; `lib.rs:84-86`.
- **Mécaniques :** toutes.
- **Question :** une page d'une version antérieure du commun reçoit-elle un refus lisible ?
- **Pression :** `a_page_of_another_protocol_is_told_so_even_when_its_hello_does_not_read`, `a_broken_hello_of_this_protocol_is_garbage_and_not_another_version` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-41 · Une page restée en cache après un changement est refusée, pas lue de travers
- **Ce qui arrive :** une page en cache décode de travers des messages qui ont changé. Salon calculait la version du jeu depuis ses sources ; tout fichier comptait, commentaires compris, ce qui coûtait un rechargement inutile parfois.
- **D'où on le sait :** `README.md:30-44` ; `lib.rs:81-84`.
- **Mécaniques :** toutes.
- **Question :** un appareil peut-il faire tourner une autre version que la partie qui fait foi ?
- **Pression :** `Refusal::Version` et son test dans `salon.rs`.
- **Sous la nouvelle forme :** impossible par la forme : « le module du jeu arrive ensuite par la connexion, depuis la partie qui fait foi : un appareil a toujours exactement sa version » (contrat, « Relier les appareils »).
- **Étage :** commun.

### SAL-42 · Un refus est typé, clair et final, jamais un lobby muet
- **Ce qui arrive :** salle pleine, salle fermée, autre version, fausse TV : sans message, le joueur attend un lobby qui ne vient pas. Un refus silencieux est un bug.
- **D'où on le sait :** `lib.rs:79-81` ; `LEARNINGS.md:76` (boardgame.io #723, #592) ; C8.
- **Mécaniques :** toutes.
- **Question :** chaque refus du commun dit-il pourquoi à l'appareil ?
- **Pression :** `refusals_are_clear_and_final` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## La vivacité et la reconnexion

### SAL-43 · Seul un battement applicatif dit qu'un pair est vivant
- **Ce qui arrive :** ni l'état ICE, ni `onclose`, ni `readyState` ne le disent. iOS laisse un WebSocket `OPEN` après la mort du Wi-Fi. Salon envoyait un paquet au moins toutes les 250 ms dans les deux sens ; la page jetait son socket après le délai d'absence, sans attendre un `onclose` qui peut ne jamais venir.
- **D'où on le sait :** `LEARNINGS.md:45` (str0m #1093 ; WebKit 247943, 259743 ; nakama-js #118, #148, #151 ; geckos.io #4, #14, #32) ; `lib.rs:114-118` ; C3.
- **Mécaniques :** toutes.
- **Question :** comment chaque côté sait-il que l'autre est mort quand rien ne se ferme ?
- **Pression :** `silence_is_noticed_on_both_sides` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-44 · Cinq à six secondes sans paquet : un bon délai d'absence sur un réseau local
- **Ce qui arrive :** Fiedler propose 5 s ; Colyseus 3 s deux fois. Le trafic régulier sert de battement, sans message à part. Salon : battement 250 ms, « faible » après 1,5 s, absent après 6 s.
- **D'où on le sait :** `LEARNINGS.md:46` (Fiedler, *Client Server Connection* ; Colyseus) ; `types.rs:298-312`.
- **Mécaniques :** toutes.
- **Question :** quel délai d'absence le commun choisit-il, et sur quelle mesure ?
- **Pression :** `silence_is_noticed_on_both_sides` (`salon.rs`) ; C3 (absence constatée en moins de 20 s).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-45 · Les délais du transport sont plus longs que ceux du jeu
- **Ce qui arrive :** sous les délais applicatifs, il y a ceux du transport : 15 s en ICE-lite chez str0m, 30 s de « consent freshness » (RFC 7675). Les délais du commun doivent passer avant.
- **D'où on le sait :** `LEARNINGS.md:47` (str0m 947598e4 ; RFC 7675).
- **Mécaniques :** toutes.
- **Question :** les délais du commun sont-ils plus courts que ceux du transport qu'ils surveillent ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-46 · Les délais choisis pour le réseau local ne valent pas pour internet
- **Ce qui arrive :** salon jetait les délais d'internet (Nakama 15 s / 25 s, Quake 3 200 s, HappyFunTimes 15 s puis 7,5 s), car tout restait sur le réseau local. Le nouveau contrat ajoute un service en ligne qui peut faire passer les messages, avec la latence d'internet. Les délais de salon n'y ont jamais été éprouvés.
- **D'où on le sait :** `LEARNINGS.md:89`.
- **Mécaniques :** toutes.
- **Question :** les délais d'absence et de renvoi tiennent-ils quand les messages passent par internet ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-47 · Une reconnexion à intervalle croissant, avec du hasard, qui ne compte pas un lien mort aussitôt comme un succès
- **Ce qui arrive :** quinze téléphones qui réessaient ensemble au même rythme chargent l'hôte au même instant. Un lien qui meurt juste après son ouverture ne doit pas remettre l'attente à zéro. Salon : de 250 ms doublé jusqu'à 2 s, ±25 %, soit 188 ms à 2,5 s ; remise à zéro après 5 s de lien sain. La documentation avait de faux chiffres, corrigés.
- **D'où on le sait :** `LEARNINGS.md:48` (Colyseus `Reconnection.ts`, `minUptime` 5 s ; Jackbox `reconnect()`) ; `lib.rs:100-104` ; commit `cc5c0a7`.
- **Mécaniques :** toutes.
- **Question :** comment les appareils réessaient-ils, et quand leur attente revient-elle au début ?
- **Pression :** `a_phone_without_host_keeps_trying_ever_more_calmly` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-48 · Un téléphone de soirée n'abandonne jamais tant que sa page est visible
- **Ce qui arrive :** Jackbox abandonne après 30 s. À une soirée, un joueur abandonné par sa page est un joueur éjecté.
- **D'où on le sait :** `LEARNINGS.md:49` ; `lib.rs:100-101`.
- **Mécaniques :** toutes.
- **Question :** un appareil cesse-t-il un jour de se reconnecter seul ?
- **Pression :** `a_phone_without_host_keeps_trying_ever_more_calmly` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-49 · Toute attente d'un pair a sa propre échéance
- **Ce qui arrive :** str0m n'a aucun délai tant qu'aucun candidat distant n'est connu. Une connexion qui ne dit jamais qui elle est garde sa place. Salon fermait une connexion sans hello après 10 s ; la porte de l'app bornait aussi les pairs qui envoient des trames complètes sans jamais dire hello.
- **D'où on le sait :** `LEARNINGS.md:50`, `LEARNINGS.md:434-435` ; `crates/host/src/rtc.rs:34-37` ; `types.rs` (`Timing::handshake`) ; `lib.rs:329` (« chaque état d'attente a une échéance »).
- **Mécaniques :** toutes.
- **Question :** existe-t-il dans le commun une attente sans échéance ?
- **Pression :** aucune dédiée connue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-50 · Au réveil, une tentative lancée avant le sommeil est abandonnée
- **Ce qui arrive :** une tentative lancée avant le sommeil reste bloquée au réveil, parfois plus de 10 s ; ouverte hors ligne, elle peut ne jamais finir sur Safari mobile. Salon gardait son budget de 20 s, au-delà des 10 s de C3. Au réveil, il abandonne une tentative de plus de 1,5 s ; au retour du réseau, toutes. Une tentative jeune survit au réveil : `pageshow` arrive à chaque chargement.
- **D'où on le sait :** commit `24a67c1` ; `LEARNINGS.md:312` (Phoenix 2575a6b ; centrifuge-js) ; `lib.rs:112-113` ; `recherche/reconnexion.toml` T4.
- **Mécaniques :** toutes.
- **Question :** que fait un appareil d'une connexion en cours quand la page revient au premier plan ou que le réseau revient ?
- **Pression :** `waking_up_abandons_an_attempt_from_before_the_sleep`, `waking_up_leaves_a_young_attempt_alone`, `the_network_coming_back_abandons_any_attempt`, `the_network_coming_back_retries_at_once`, `the_network_coming_back_leaves_a_live_link_alone` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-51 · Une horloge monotone peut ne pas compter le sommeil
- **Ce qui arrive :** sur Darwin, WebKit lit `mach_absolute_time`, qui ne compte pas le sommeil : le silence d'un iPhone endormi passait inaperçu, et le chien de garde prenait jusqu'à 6 s. Salon ajoute ce que l'horloge murale a gagné d'un coup sur la monotone. Sans effet là où la monotone compte le sommeil : il n'y avait pas à mesurer l'iPhone avant de corriger. Une petite dérive n'est pas du sommeil ; une horloge murale avancée à la main coûte une reconnexion.
- **D'où on le sait :** commit `b5f6b0d` ; `LEARNINGS.md:313` (socket.io engine.io ; WebKit `CurrentTime.cpp` ; w3c/hr-time#115) ; `lib.rs:118-119` ; `recherche/reconnexion.toml` T5.
- **Mécaniques :** toutes ; comptes à rebours.
- **Question :** l'horloge de la page voit-elle le temps passé en veille ?
- **Pression :** `a_clock_stopped_by_sleep_does_not_hide_the_silence`, `a_clock_that_counts_sleep_is_left_as_it_is`, `drift_is_not_sleep` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-52 · `visibilitychange` ne suit plus toujours le dégel d'une page
- **Ce qui arrive :** Phoenix le note pour Chrome 149 : une page dégelée ne reçoit pas toujours `visibilitychange`. Salon écoutait aussi `resume` sur `document`, et `pageshow`. Le banc ne sait pas produire un vrai dégel : `Page.setWebLifecycleState` n'y a aucun effet (Chrome 156).
- **D'où on le sait :** `LEARNINGS.md:316` (Phoenix `socket.js`) ; `salon/crates/web/src/lib.rs:20-22` ; commits `7b1033b`, `a2ac66a` ; `recherche/reconnexion.toml` T8.
- **Mécaniques :** toutes.
- **Question :** quels signaux du navigateur réveillent la page, et lequel a été vu dans un vrai dégel ?
- **Pression :** `salon/tests/ui-browser.mjs` envoie `resume` dans WebKit et Chromium ; `a_page_that_wakes_up_reconnects_at_once` (`salon.rs`). Pas de vrai dégel.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-53 · iOS gèle les pages en arrière-plan et n'annonce pas un changement de Wi-Fi
- **Ce qui arrive :** `online` n'est annoncé au mieux qu'après une perte de connexion. iOS ne dit rien quand il passe sur un autre Wi-Fi, ni quand l'hôte ouvre son pare-feu. Seul un délai borne alors l'attente.
- **D'où on le sait :** `salon/crates/web/src/rtc_memory.rs:14-19` ; `web/src/lib.rs:20-22` ; `README.md:210`.
- **Mécaniques :** toutes.
- **Question :** ce que le commun attend d'un signal du navigateur a-t-il aussi un délai ?
- **Pression :** aucune sur un vrai iPhone.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-54 · Au réveil, couper un lien peut-être vivant a un coût non mesuré
- **Ce qui arrive :** salon coupait au réveil un lien qui pouvait encore vivre, et renégociait. centrifuge-js laisse d'abord passer ce qui attend dans le socket. L'attendre retarderait d'autant chaque lien vraiment mort. Resté ouvert.
- **D'où on le sait :** `LEARNINGS.md:324` (centrifuge-js fe060f3) ; commit `b308223` ; `recherche/reconnexion.toml` T9.
- **Mécaniques :** toutes.
- **Question :** au réveil, le commun attend-il ce qui est déjà reçu avant de juger le lien mort ?
- **Pression :** aucune ; renvoyé à la fiche de répétition.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-55 · Le rattrapage ne se distingue pas du direct
- **Ce qui arrive :** après une reconnexion, les événements manqués arrivent d'un coup, comme s'ils se passaient maintenant. Discord marque la reprise par `RESUMED`. Salon ne l'a pas fait, faute de gêne constatée.
- **D'où on le sait :** `LEARNINGS.md:325` ; `recherche/reconnexion.toml` T11.
- **Mécaniques :** son, vibration, animations d'événements.
- **Question :** un appareil qui rattrape rejoue-t-il des effets périmés ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme : « un effet ponctuel porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas », et l'instant où il doit se produire (contrat, « La forme d'un jeu »).
- **Étage :** commun.

### SAL-56 · Ce qui est envoyé pendant une coupure part à la reconnexion, quelle que soit l'issue
- **Ce qui arrive :** un accueil d'une autre exécution vidait la file : une réponse envoyée pendant un redémarrage avait rendu Ok et disparaissait. La file survit maintenant à la remise à zéro ; elle part vers la nouvelle exécution, après l'arrivée de l'appareil, avec la phase de son envoi.
- **D'où on le sait :** commit `906b7de` ; `LEARNINGS.md:311` (socket.io 8adcfbf ; Figma) ; `README.md:172` ; `recherche/reconnexion.toml` T6.
- **Mécaniques :** événements comptés une fois.
- **Question :** un geste fait hors ligne atteint-il la partie, et comment le jeu le juge-t-il ?
- **Pression :** `what_waits_goes_to_the_host_that_restarted`, `what_waits_goes_to_a_host_without_memory_under_the_new_identity`, `events_sent_before_the_first_welcome_count` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-57 · Un événement en vol quand l'autorité meurt peut atteindre les deux exécutions
- **Ce qui arrive :** l'ancienne exécution l'a reçu sans l'accuser ; l'appareil le renvoie à la nouvelle. Salon le livrait une fois à chacune.
- **D'où on le sait :** `lib.rs:136-138` ; commit `906b7de`.
- **Mécaniques :** événements comptés une fois.
- **Question :** la reprise depuis un état sauvé et les entrées qui l'ont suivi compte-t-elle exactement une fois un événement en vol ?
- **Pression :** `an_event_in_flight_when_the_host_dies_reaches_each_run_once` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-58 · Un au-revoir se lit avant de fermer son lien
- **Ce qui arrive :** les premiers messages d'au-revoir portent ce qui attend de partir. L'hôte fermait le lien sans les lire : un événement envoyé juste avant de quitter était perdu, après un `send` réussi.
- **D'où on le sait :** commit `85db0de` ; `recherche/fiabilite.toml` T10.
- **Mécaniques :** événements comptés une fois.
- **Question :** le dernier message d'un appareil qui part est-il livré avant son départ ?
- **Pression :** `an_event_sent_just_before_quitting_arrives` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-59 · Une fermeture qui trouve son transport en retard se redit
- **Ce qui arrive :** une fermeture qui trouvait son transport encombré était perdue, contrairement à son commentaire. Rien d'autre ne termine un lien WebRTC ou un lien d'app dont le pair le garde ouvert.
- **D'où on le sait :** commit `cc5c0a7`.
- **Mécaniques :** toutes.
- **Question :** un ordre de fermeture peut-il se perdre ?
- **Pression :** aucune dédiée connue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-60 · Sur WebSocket, la trame de fermeture est une donnée, et passe devant le FIN
- **Ce qui arrive :** le toxique `slow_close` de Toxiproxy ne retient que le FIN TCP. La trame `Close` de WebSocket passe devant : une page apprend la fermeture de l'hôte par la trame. C'est la fermeture vers l'hôte que le retard touche vraiment.
- **D'où on le sait :** `LEARNINGS.md:317` ; commit `1becb46`.
- **Mécaniques :** toutes.
- **Question :** les pannes de fermeture sont-elles jouées dans les deux sens ?
- **Pression :** `a_page_does_not_wait_for_a_close_that_is_slow_to_come`, `a_page_reloaded_behind_a_slow_close_just_plays`, `a_reset_loses_nothing_and_doubles_nothing` (`crates/host/tests/transport_faults.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-61 · Le faux réseau garde une fermeture derrière les données de son lien, comme TCP
- **Ce qui arrive :** sur TCP, une fermeture arrive après les données envoyées avant elle, et ce qui suit un paquet coincé reste derrière lui. Un faux réseau qui ne le fait pas ne voit pas le bug de SAL-19.
- **D'où on le sait :** commit `658b388` ; `README.md:218`.
- **Mécaniques :** toutes.
- **Question :** le réseau simulé respecte-t-il l'ordre d'un flux, fermeture comprise ?
- **Pression :** `chaos_keeps_every_page_in_its_place` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-62 · Ne retenir le repli que si le canal principal n'a jamais marché ici
- **Ce qui arrive :** une page où WebRTC marchait voyait deux échecs pendant un redémarrage de l'hôte, et restait dix minutes sur WebSocket, rechargements compris. Un échec après un succès est passager. Deux vrais échecs restent retenus : un binaire neuf bloqué par le pare-feu de macOS est un échec après un succès, et coûterait 15 s à chaque reconnexion.
- **D'où on le sait :** commit `7b1033b` ; `LEARNINGS.md:314` (Phoenix b2eeccf) ; `web/src/rtc_memory.rs:12-13`.
- **Mécaniques :** toutes.
- **Question :** quand le commun renonce-t-il au meilleur canal, et pour combien de temps ?
- **Pression :** tests purs de `rtc_memory.rs`, sans navigateur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-63 · Seule une offre qui a reçu une réponse peut accuser WebRTC
- **Ce qui arrive :** quand la requête d'offre ne recevait pas de réponse, l'échéance retenait quand même un échec de WebRTC. Toute réponse HTTP, même un 503 d'une file de signalisation pleine, comptait comme une offre répondue : la page rechargée restait dix minutes sur WebSocket alors que WebRTC marchait. Un hôte qui redémarre ne dit rien de WebRTC.
- **D'où on le sait :** commits `6b29e39`, `7b1033b`, `b5cf782` ; `LEARNINGS.md:315` ; `recherche/reconnexion.toml` T7 ; `recherche/transport-webrtc.toml` T6.
- **Mécaniques :** toutes.
- **Question :** le commun distingue-t-il « l'autre côté manque » de « ce canal ne passe pas » ?
- **Pression :** tests purs de `rtc_memory.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-64 · Un échec du meilleur canal se retient dans l'onglet, avec une sortie
- **Ce qui arrive :** là où l'UDP est bloqué, chaque chargement attendait toute la fenêtre de 15 s avant WebSocket. L'onglet garde l'échec en stockage de session : le chargement suivant passe tout de suite par WebSocket. WebRTC est retenté après dix minutes, sur `online`, ou dès qu'un lien WebRTC s'ouvre. Une autre adresse de l'hôte est une autre origine, avec sa propre mémoire.
- **D'où on le sait :** commit `0a24d04` ; `README.md:210` ; `web/src/rtc_memory.rs:1-19`.
- **Mécaniques :** toutes.
- **Question :** un rechargement repaie-t-il l'échec du canal ?
- **Pression :** tests purs de `rtc_memory.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Le transport : WebRTC et WebSocket

### SAL-65 · WebRTC marche depuis une page `http://` locale, sans serveur internet
- **Ce qui arrive :** mesuré le 27/10/2026, hôte str0m 0.24.1 en ICE-lite, page sans contexte sécurisé : Safari iOS 26.5 ouvre un DataChannel non ordonné sans retransmission en 90 ms ; 500 pings sur 500 ; aller-retour médian 4 ms, p95 7 ms, p99 67 ms, maximum 182 ms. Le Mac était sur le partage de connexion de l'iPhone, pas sur un Wi-Fi de salon.
- **D'où on le sait :** `LEARNINGS.md:98-101` ; `lib.rs:187-192` ; `recherche/transport-webrtc.toml` T9 (jamais mesuré sur un vrai Wi-Fi de soirée).
- **Mécaniques :** toutes.
- **Question :** ces chiffres tiennent-ils sur le Wi-Fi du salon, avec quinze téléphones ?
- **Pression :** aucune ; une mesure ponctuelle.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-66 · Safari cache son adresse derrière un nom `.local` : le pair l'apprend par le premier paquet
- **Ce qui arrive :** l'hôte apprend l'adresse du téléphone par son premier paquet (candidat *peer-reflexive*), comme geckos.io. La RFC 8838 autorise l'initiateur full ICE à ne pas envoyer ses candidats à un pair ICE-lite : une offre sans candidat n'est pas à elle seule un échec. Entre deux navigateurs, les deux côtés cacheraient leur adresse.
- **D'où on le sait :** `LEARNINGS.md:103`, `LEARNINGS.md:457-461` (RFC 8838 §16) ; `crates/host/src/rtc.rs:4-5`.
- **Mécaniques :** toutes.
- **Question :** comment le pair apprend-il l'adresse d'un navigateur qui la cache ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-67 · Un seul DataChannel non ordonné, sans retransmission, et la fiabilité au-dessus
- **Ce qui arrive :** str0m partage 128 KiB entre tous les canaux d'une association, retransmet sans fin sur un canal fiable, et a ignoré `maxRetransmits:0` jusqu'à sa 0.22. Salon faisait la fiabilité, l'ordre et l'abandon des états périmés une fois, avec le même code pour WebRTC, l'UDP de l'app et WebSocket.
- **D'où on le sait :** `LEARNINGS.md:69` (str0m `src/sctp/mod.rs`, #1004, #976 ; geckos.io `connectionsManager.ts`) ; `LEARNINGS.md:334`.
- **Mécaniques :** toutes.
- **Question :** la fiabilité du commun dépend-elle d'un canal qui se comporte comme promis ?
- **Pression :** les tests du flux dans `crates/core/tests/`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-68 · Un paquet non fiable reste dans une seule trame réseau
- **Ce qui arrive :** sur un canal non fiable, un fragment IP perdu fait perdre tout le message. `Caps::DATAGRAM` promettait une trame, mais str0m retire 88 octets de son MTU par défaut de 1 150 : un paquet de plus de 1 062 octets partait en deux datagrammes, et la perte doublait. Salon garde désormais le plus grand paquet dans un seul datagramme.
- **D'où on le sait :** `LEARNINGS.md:71` (Quake 3 `FRAGMENT_SIZE` ; renet #123 ; str0m MTU 1150) ; commit `f8230d6` ; `recherche/transport-webrtc.toml` T4.
- **Mécaniques :** toutes.
- **Question :** la plus grande unité envoyée sur le canal non fiable tient-elle dans un datagramme, en-têtes du transport compris ?
- **Pression :** `the_fullest_packet_a_client_sends_still_reads` (`stream_cost.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-69 · Grouper : un paquet par envoi et par lien
- **Ce qui arrive :** 300 petits messages par seconde font un diaporama ; chaque message DataChannel coûte 50 à 70 octets. Salon mettait accusés, phase, événements et état dans un seul paquet.
- **D'où on le sait :** `LEARNINGS.md:72` (geckos.io #49, #68) ; `lib.rs:172-175`.
- **Mécaniques :** toutes ; états à haute fréquence.
- **Question :** combien de messages du transport un pas du jeu produit-il par appareil ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-70 · Le tampon d'un canal sans retransmission ne perd rien : le frais attend derrière le périmé
- **Ce qui arrive :** sur un lien plus lent que le jeu, des états frais attendaient des secondes derrière des états périmés : 6,7 s pour un téléphone à 30 Ko/s avec les 128 KiB de str0m. Salon saute l'écriture quand le tampon est resté au-dessus de 4 Kio pendant 100 ms, comme une perte.
- **D'où on le sait :** `lib.rs:231-247` ; commit `0210e4c` ; `recherche/transport-webrtc.toml` T5.
- **Mécaniques :** états tenus à haute fréquence (manette, position).
- **Question :** sur un lien lent, le dernier état attend-il derrière les anciens ?
- **Pression :** `messages_of_many_fragments_get_through_the_backlog_guard` (`crates/host/tests/rtc_faults.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-71 · Une limite en octets sous la taille d'une rafale affame ce qu'elle protège
- **Ce qui arrive :** une garde à 4 Kio fixes réduisait le retard de 6,7 s à 160 ms, mais perdait à chaque renvoi les mêmes derniers fragments d'une rafale : un message de 4 000 octets n'arrivait plus, sur un lien sain. Ce qui compte n'est pas un tampon plein, mais un tampon qui le reste. Et une garde qui se tait tout à fait laisse la reprise au minuteur de SCTP : un paquet toutes les 100 ms, et une coupure de 3 s qui en coûtait 6,8.
- **D'où on le sait :** `LEARNINGS.md:494` ; `lib.rs:236-262`.
- **Mécaniques :** gros messages.
- **Question :** une protection contre l'encombrement laisse-t-elle passer une rafale entière, et un paquet de temps en temps ?
- **Pression :** `messages_of_many_fragments_get_through_the_backlog_guard` (`rtc_faults.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-72 · Un refus d'écriture du canal se lit comme une perte
- **Ce qui arrive :** salon ne lisait pas le retour de `write` de str0m, et n'avait plus à le lire : un refus se voit dans les accusés, comme une perte.
- **D'où on le sait :** `LEARNINGS.md:61` (str0m `channel.rs`).
- **Mécaniques :** toutes.
- **Question :** un refus local d'envoi est-il rattrapé par la fiabilité au-dessus ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-73 · Sur l'ordinateur même, WebSocket suffit et porte plus
- **Ce qui arrive :** la TV, sur l'ordinateur de la soirée, passait toujours par WebSocket : en local il ne perd rien et porte des paquets de 16 Kio, contre 1 100 octets sur un DataChannel. Une TV en WebRTC, avant le candidat de bouclage, était refusée et réessayait sans fin.
- **D'où on le sait :** `LEARNINGS.md:111` ; `README.md:173`.
- **Mécaniques :** toutes.
- **Question :** la page qui fait foi, sur l'ordinateur de la soirée, a-t-elle besoin de WebRTC pour joindre le relais local ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-74 · Le candidat de bouclage échappe au pare-feu
- **Ce qui arrive :** l'hôte annonçait aussi `127.0.0.1` : une page de l'ordinateur même (le harnais, un essai) passait par la boucle locale, que le pare-feu ne filtre pas. Un téléphone ignore ce candidat. Un lien venu de la boucle est local, comme un WebSocket venu de la boucle.
- **D'où on le sait :** `LEARNINGS.md:110` ; `crates/host/src/rtc.rs:2-4`.
- **Mécaniques :** toutes.
- **Question :** une page de l'ordinateur de la soirée dépend-elle du pare-feu pour joindre le relais ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-75 · L'adresse annoncée est celle que le téléphone a vraiment jointe
- **Ce qui arrive :** le candidat était fixé au lancement depuis la route par défaut. Un téléphone qui joignait l'hôte par une autre adresse recevait un candidat inutilisable, et payait le repli à chaque connexion. Le socket qui écoute partout ne sait pas qui a été joint ; la connexion TCP de l'offre le sait. str0m jette un contrôle ICE qui n'est pas pour un de ses candidats : l'adresse annoncée et celle dite à str0m doivent venir du même endroit, par lien.
- **D'où on le sait :** commit `df4afe3` ; `LEARNINGS.md:495` ; `recherche/transport-webrtc.toml` T3.
- **Mécaniques :** toutes.
- **Question :** un appareil reçoit-il une adresse qu'il peut joindre, même sur une machine à plusieurs interfaces ?
- **Pression :** `node salon/tests/rtc-browser.mjs` (selon le commit).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-76 · La route par défaut est une supposition, pas un fait
- **Ce qui arrive :** sans réseau, elle n'existe pas, et l'adresse du QR code devient `http://localhost`, qu'aucun téléphone n'atteint. Derrière un VPN complet, elle existe mais personne ne l'atteint. Chaque jeu aurait dû le résoudre seul. Une variable commune disait l'adresse vraie, validée à l'entrée ; une adresse IP y est aussi annoncée par WebRTC, un nom ne peut pas l'être.
- **D'où on le sait :** commit `3194971` ; `README.md:111-113` ; `crates/host/src/address.rs:1-4`.
- **Mécaniques :** toutes.
- **Question :** comment le commun sait-il et dit-il où les téléphones peuvent le joindre ?
- **Pression :** `the_public_url_replaces_the_network_address`, `without_a_public_url_phones_get_the_network_address`, `a_public_url_that_is_no_address_is_refused_out_loud` (`crates/host/tests/public_url.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-77 · Une adresse qu'on n'écoute pas se refuse à voix haute
- **Ce qui arrive :** salon n'écoutait qu'en IPv4. Une adresse publique IPv6 donnait un QR code qui menait nulle part, sans un mot.
- **D'où on le sait :** commit `d696953` ; `recherche/transport-webrtc.toml` T8.
- **Mécaniques :** toutes.
- **Question :** le commun annonce-t-il une adresse sur laquelle rien n'écoute ?
- **Pression :** `an_ipv6_public_url_is_refused_out_loud` (`public_url.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-78 · Les négociations en cours se bornent
- **Ce qui arrive :** chaque offre vivait 15 s, et chaque datagramme les parcourait toutes, sans jeton : une boucle d'offres faisait passer un lien vivant de 0 à 75 ms d'aller-retour. Au plus 64 en même temps ; la plus ancienne part.
- **D'où on le sait :** commit `65f64f4` ; `recherche/transport-webrtc.toml` T7.
- **Mécaniques :** toutes.
- **Question :** un appareil qui négocie en boucle peut-il ralentir les liens des autres ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-79 · Une dépendance peut paniquer sur une entrée réseau, et un fil partagé rend cette panique mortelle
- **Ce qui arrive :** dimpl 0.7.6 fait `assert_eq!` sur deux enregistrements égaux d'un pair sans jeton. Le fil unique de WebRTC tombait pour toute la soirée, sans un mot, jusqu'au redémarrage. Chaque appel à str0m est gardé par lien ; un lien qui a paniqué n'est plus jamais touché ; le fil repart si une panique lui échappe. Cela exige le déroulement de pile : aucun profil n'écrit `panic = "abort"`.
- **D'où on le sait :** commits `9bcd5c5`, `be5ca06`, `b308223` ; `LEARNINGS.md:492` (dimpl #189) ; `recherche/transport-webrtc.toml` T1, T11.
- **Mécaniques :** toutes.
- **Question :** la panique d'une dépendance du commun sur l'entrée d'un seul appareil coûte-t-elle plus que ce lien ?
- **Pression :** `a_panic_in_a_dependency_costs_one_link` (`rtc_faults.rs`) ; `dimpl_on_a_doubled_early_encrypted_record` (`crates/crypto/tests/handshake_faults.rs`), épinglé pour que monter dimpl force à le regarder.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-80 · `failed` arrive trop tard pour décider d'un repli
- **Ce qui arrive :** là où l'UDP n'arrive pas mais TCP oui, la réponse SDP vient tout de suite, puis plus rien : le navigateur met 15 à 30 s à dire `failed`, et chaque coupure coûtait 15 s, contre 10 s promises par C2. En réseau local, une réponse STUN prend des millisecondes. Salon replie si ICE reste muet 3 s après la réponse. Sur 36 essais à 40 % de pertes, ce délai n'a pas ajouté de replis.
- **D'où on le sait :** commit `ce410e1` ; `LEARNINGS.md:493` ; `web/src/rtc_memory.rs:21-38` ; `recherche/transport-webrtc.toml` T2.
- **Mécaniques :** toutes.
- **Question :** sur quel signal le commun renonce-t-il au meilleur canal, et ce signal arrive-t-il à temps ?
- **Pression :** `node salon/tests/rtc-browser.mjs`, avec un relais qui ne laisse passer que TCP.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-81 · Une borne n'est prouvée que si rien d'autre ne la devance
- **Ce qui arrive :** avec le repli sur silence d'ICE, la borne absolue de 15 s n'est plus atteinte par les pannes d'ICE. Le harnais doit couper DTLS seul, après ICE, pour l'éprouver.
- **D'où on le sait :** `LEARNINGS.md:493` ; commit `ce410e1`.
- **Mécaniques :** toutes.
- **Question :** chaque délai du commun a-t-il une panne qui l'atteint vraiment ?
- **Pression :** le relais du harnais qui ne jette que DTLS (`rtc-browser.mjs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-82 · Une négociation lente n'est pas une négociation impossible
- **Ce qui arrive :** en retardant seulement la réponse SDP de 4 s, l'ancien SDK fermait sa connexion à environ 3 s, avant de lire la réponse, et passait à WebSocket sans avoir envoyé un datagramme. Sans ce rappel de coupure, le DataChannel s'ouvrait vers 4,3 s. DTLS prévoit un premier délai d'une seconde puis son doublement : trois secondes ne suffisent pas à conclure.
- **D'où on le sait :** `LEARNINGS.md:444-461` (RFC 6347 §4.2.4.1) ; témoins dans `certification/robustesse-2026-10-29/rtc-diagnostic/` (non lus).
- **Mécaniques :** toutes.
- **Question :** les délais du commun laissent-ils aux retransmissions natives du protocole le temps d'agir ?
- **Pression :** `tools/cert/rtc-negotiation.mjs` (retards SDP de 2 et 4 s, vrais rejets SDP et DTLS, panne permanente, réponse après le repli).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-83 · Une seule fenêtre absolue de négociation, partagée par les deux côtés
- **Ce qui arrive :** la page et l'hôte avaient des délais différents. Salon fixe 15 s absolues, de la création du pair à l'ouverture du canal, dans le même crate pour les deux. Les étapes réussies ne la renouvellent pas. Une erreur native ou `Failed` replie tout de suite. Un minuteur impossible à armer libère le pair et choisit WebSocket. Des générations empêchent une réponse SDP périmée de remplacer le transport choisi.
- **D'où on le sait :** `LEARNINGS.md:462-468` ; `lib.rs:222-227` ; commit `3d45317`.
- **Mécaniques :** toutes.
- **Question :** le délai de négociation a-t-il une seule définition, et une réponse tardive peut-elle défaire le repli ?
- **Pression :** `rtc-negotiation.mjs` ; fermeture observée entre 14 999,6 et 15 000,5 ms, 7 cas sur 7 par moteur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-84 · Des délais superposés ont un propriétaire cohérent
- **Ce qui arrive :** le délai de connexion générique (5 s) détruisait chaque tentative WebRTC avant ses 15 s, puis en recréait une, sans jamais choisir TCP. Salon laisse la coque déclarer un budget absolu de phase (15 s WebRTC puis 5 s TCP), qui ne touche ni l'essai suivant ni le chien de garde d'un lien déjà ouvert.
- **D'où on le sait :** `LEARNINGS.md:478-486` ; `lib.rs:109-111`.
- **Mécaniques :** toutes.
- **Question :** deux délais du commun peuvent-ils s'annuler l'un l'autre ?
- **Pression :** `transport_phase_budgets_are_absolute_and_the_next_attempt_keeps_default`, `a_transport_budget_cannot_postpone_an_open_connections_handshake` (`salon.rs`) ; la panne UDP réelle.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-85 · Une campagne de pannes qui attend une ouverture saine ne voit pas les pannes de la négociation
- **Ce qui arrive :** la campagne UDP attendait une ouverture saine avant d'injecter ses pertes. Elle n'éprouvait ni les durées des étapes, ni la poignée de main. Salon y a ajouté chaque datagramme perdu ou doublé seul, dans chaque sens, un ClientHello en deux fragments, cinq enregistrements forgés sur un lien vivant.
- **D'où on le sait :** `LEARNINGS.md:469-477` ; commit `be5ca06` ; `recherche/transport-webrtc.toml` T10.
- **Mécaniques :** toutes.
- **Question :** les pannes sont-elles injectées pendant l'établissement du lien, et pas seulement après ?
- **Pression :** `no_single_lost_or_doubled_datagram_stops_the_handshake`, `a_client_hello_in_two_fragments_still_opens`, `forged_records_do_not_close_a_live_link` (`handshake_faults.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-86 · Perturber le vrai trajet chiffré, pas seulement le protocole simulé
- **Ce qui arrive :** les notes de str0m gardent des corrections SCTP sous pertes, de réutilisation de canaux et d'expiration ICE : des classes de bugs que le protocole applicatif simulé ne voit pas. Le relais UDP du harnais transporte les vrais datagrammes chiffrés et ne modifie que les adresses des candidats SDP. Une ouverture sans panne ne suffisait pas à attester ce chemin.
- **D'où on le sait :** `LEARNINGS.md:437-440` (str0m `CHANGELOG.md`) ; `LEARNINGS.md:355-358`.
- **Mécaniques :** toutes.
- **Question :** les pressions du réseau passent-elles par le vrai chiffrement et la vraie renégociation ?
- **Pression :** le relais UDP du harnais ; `rtc-browser.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-87 · BoringSSL refuse en silence un certificat qui ne peut pas signer
- **Ce qui arrive :** avec un certificat de racine (autorité, `keyCertSign` seulement), str0m contre str0m se connectait, mais Chrome renvoyait son ClientHello sans fin : l'hôte signait son échange de clés avec une clé qui n'y avait pas droit. Trouvé en changeant une chose à la fois : le moteur de chiffrement, puis le candidat de bouclage, puis le certificat.
- **D'où on le sait :** `LEARNINGS.md:108` (constaté le 27/10/2026).
- **Mécaniques :** toutes (chiffrement du canal).
- **Question :** le certificat du commun est-il une feuille qui peut signer ?
- **Pression :** `the_certificate_may_sign_and_is_no_authority`, `every_certificate_is_new_and_well_formed` (`crates/crypto/tests/handshake.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-88 · Un test entre deux exemplaires du même code n'éprouve pas ce que l'autre côté exige
- **Ce qui arrive :** str0m contre str0m passait ; Chrome refusait (SAL-87). La leçon dépasse le certificat : seul l'autre vrai côté (navigateur, pile SCTP, iOS) dit ce qu'il exige.
- **D'où on le sait :** `LEARNINGS.md:108`.
- **Mécaniques :** toutes.
- **Question :** chaque frontière du commun est-elle éprouvée contre la vraie implémentation d'en face ?
- **Pression :** `rtc-browser.mjs`, `ui-browser.mjs`, `audio-browser.mjs` dans WebKit et Chromium.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-89 · Un décodeur écrit à la main à côté du vrai dérive en silence
- **Ce qui arrive :** le harnais décodait les paquets à la main. L'accusé avait grandi sous lui : un vrai envoi se lisait comme aucun. Un test d'octets dans salon tient maintenant les deux à une seule forme.
- **D'où on le sait :** commit `f281791`.
- **Mécaniques :** toutes.
- **Question :** un outil qui lit les messages du commun partage-t-il leur définition ?
- **Pression :** le test d'octets ajouté par `f281791`.
- **Sous la nouvelle forme :** impossible par la forme : « les messages entre l'ordinateur de la soirée et les autres appareils sont des types Rust partagés » (contrat, « Les types de bout en bout »).
- **Étage :** commun.

### SAL-90 · Une seconde lecture indépendante du format fait rougir un changement de forme sans nouvelle version
- **Ce qui arrive :** les types du protocole étaient privés. Les tests de bord forgeaient les paquets avec une copie du format, champ à champ. Un changement de forme sans nouveau numéro de protocole les faisait échouer, volontairement.
- **D'où on le sait :** `salon/AGENTS.md` (`crates/core/tests/wire/mod.rs`) ; commit `283a015`.
- **Mécaniques :** toutes.
- **Question :** un changement du format sur le fil sans changement de version est-il détecté ?
- **Pression :** `client_edges.rs`, `host_edges.rs`, `stream_edges.rs`, `types_edges.rs` avec `wire/mod.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-91 · Un seul certificat par lancement, fait au démarrage
- **Ce qui arrive :** str0m panique s'il doit générer un certificat et ne le peut pas ; il en générait un par connexion, ce qui est lent. Fait au démarrage, une panne de hasard devient une erreur au lancement.
- **D'où on le sait :** `LEARNINGS.md:109` (str0m #405).
- **Mécaniques :** toutes.
- **Question :** ce qui peut échouer faute de ressource se fait-il au lancement, plutôt qu'en pleine partie ?
- **Pression :** `every_certificate_is_new_and_well_formed` (`handshake.rs`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-92 · La cryptographie de WebRTC en Rust seul demande sa propre crate
- **Ce qui arrive :** tous les fournisseurs de str0m compilent du C : `aws-lc-rs`, OpenSSL, CryptoKit, et même `str0m-rust-crypto`, qui génère son certificat avec `rcgen` sur `aws-lc-rs`. Salon reprenait ce dernier (MIT ou Apache-2.0) et construisait le certificat avec `p256` et `x509-cert`.
- **D'où on le sait :** `LEARNINGS.md:107` ; `README.md:198-200`.
- **Mécaniques :** toutes.
- **Question :** le chiffrement du commun tire-t-il du C, et est-ce un choix écrit ?
- **Pression :** la construction sans compilateur C.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-93 · Le hasard du système se demande par l'API portable, pas par `/dev/urandom`
- **Ce qui arrive :** ouvrir `/dev/urandom` faisait échouer le démarrage sous Windows. Salon prenait ses octets par `rand_core`, qui passe par `getrandom`.
- **D'où on le sait :** commit `1b9fdda`.
- **Mécaniques :** toutes.
- **Question :** le relais et l'outil démarrent-ils sur chaque système visé ?
- **Pression :** aucune connue hors construction Windows.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-94 · Une dépendance directe nouvelle du commun change le verrou de chaque jeu
- **Ce qui arrive :** ajouter `getrandom` en dépendance directe de l'hôte aurait changé le `Cargo.lock` de chaque jeu construit avec `--locked`. Salon a pris une crate déjà dans le graphe.
- **D'où on le sait :** commit `1b9fdda`.
- **Mécaniques :** toutes.
- **Question :** un changement de dépendances du commun casse-t-il la construction verrouillée des jeux ?
- **Pression :** `soiree check --locked`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Le réseau du salon, le système et le matériel

### SAL-95 · Le pare-feu de macOS bloque en silence l'UDP de tout binaire neuf
- **Ce qui arrive :** le 27/10/2026, pare-feu activé, pas un paquet WebRTC n'est arrivé ; la page TV est passée par WebSocket au bout de 3 s. Chaque `cargo build` produit un binaire que macOS tient pour une nouvelle app. Avant la soirée : lancer chaque programme une fois et accepter la demande, ou signer avec un certificat stable.
- **D'où on le sait :** `LEARNINGS.md:162` ; `README.md:210`.
- **Mécaniques :** toutes.
- **Question :** le jour de la soirée, le relais local reçoit-il l'UDP, et le commun le dit-il sinon ?
- **Pression :** aucune automatique ; un item de la répétition.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-96 · La confidentialité du réseau local d'Apple retarde le premier envoi
- **Ce qui arrive :** l'app iPhone doit demander la permission d'accéder au réseau local, et son premier envoi peut échouer avant la réponse. Un hôte macOS lancé depuis le Finder en a besoin aussi ; lancé depuis le Terminal, non.
- **D'où on le sait :** `LEARNINGS.md:163` (Apple TN3179).
- **Mécaniques :** app iPhone.
- **Question :** l'app et le relais local supportent-ils un premier envoi refusé ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-97 · Pas de gyroscope dans une page `http://` sur iOS
- **Ce qui arrive :** iOS refuse d'office la permission des capteurs de mouvement hors HTTPS. C'est ce qui a tué HappyFunTimes. L'inclinaison passait par l'app.
- **D'où on le sait :** `LEARNINGS.md:104` ; `types.rs` (`Kind::Phone` : « no motion sensors on http:// »).
- **Mécaniques :** capteurs de mouvement, inclinaison, visée.
- **Question :** le jeu reste-t-il jouable sans capteurs de mouvement sur un téléphone en navigateur ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SAL-98 · Sur un Wi-Fi sans internet, le téléphone peut partir sur le réseau cellulaire
- **Ce qui arrive :** iOS et Android peuvent basculer sur le cellulaire quand le Wi-Fi n'a pas d'internet ; ils n'atteignent plus l'ordinateur de la soirée. HappyFunTimes falsifiait le portail captif d'Apple, et cela cassait tous les deux ans. À vérifier au salon.
- **D'où on le sait :** `LEARNINGS.md:105`, `LEARNINGS.md:93` (HappyFunTimes `apple-captive-portal-handler.js`).
- **Mécaniques :** toutes.
- **Question :** avec le seul relais local et sans internet, les téléphones restent-ils sur le Wi-Fi ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-99 · Beaucoup de box isolent les clients du Wi-Fi
- **Ce qui arrive :** les téléphones ne voient pas l'hôte. Un petit routeur dédié règle le problème.
- **D'où on le sait :** `LEARNINGS.md:106` (HappyFunTimes `troubleshooting.md`).
- **Mécaniques :** toutes.
- **Question :** le commun détecte-t-il des appareils isolés, et bascule-t-il sur le service en ligne ou le dit-il ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-100 · Certains routeurs n'acceptent que 13 ou 14 clients
- **Ce qui arrive :** un petit routeur dédié, qui règle l'isolation, peut plafonner le nombre de téléphones d'une soirée.
- **D'où on le sait :** `LEARNINGS.md:106`.
- **Mécaniques :** toutes.
- **Question :** le matériel de la soirée accepte-t-il tous les appareils prévus ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-101 · Les manettes étaient lues par l'hôte natif, faute de savoir ce que Chrome permet
- **Ce qui arrive :** salon lisait les manettes dans le programme natif, pas dans la page TV : Chrome en limiterait le nombre (jamais vérifié), et la page n'avait pas à relayer. Dans le nouveau contrat, la partie qui fait foi est une page.
- **D'où on le sait :** `LEARNINGS.md:164` ; `README.md:84`.
- **Mécaniques :** manettes.
- **Question :** combien de manettes la Gamepad API du navigateur visé lit-elle à la fois ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-102 · Aucune constante réseau n'a été essayée sur un vrai Wi-Fi de soirée
- **Ce qui arrive :** les garanties du lien étroit tenaient sur les tuyaux simulés. Aucune constante n'avait été essayée sur un vrai Wi-Fi, ni contre une vraie pile SCTP. Restait à mesurer au salon, avec quinze téléphones : aller-retour, pertes, temps de reconnexion après un iPhone verrouillé.
- **D'où on le sait :** `lib.rs:151-157` ; `LEARNINGS.md:335` ; `recherche/transport-webrtc.toml` T9.
- **Mécaniques :** toutes.
- **Question :** quels chiffres du commun viennent du réel, et lesquels du banc ?
- **Pression :** aucune ; la fiche de répétition.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## La fiabilité des événements et des états

### SAL-103 · La fiabilité du transport s'arrête à la mort du lien
- **Ce qui arrive :** un événement envoyé juste avant une coupure est perdu, quel que soit le transport, même « fiable ». Il faut des numéros au niveau de l'appareil, qui survivent aux reconnexions.
- **D'où on le sait :** `LEARNINGS.md:68` (renet #59 ; geckos.io #28, #269) ; `lib.rs:22-24`.
- **Mécaniques :** événements comptés une fois.
- **Question :** un événement est-il compté exactement une fois à travers les coupures et les changements de transport ?
- **Pression :** `chaos_never_loses_or_doubles_an_event` (`properties.rs`) ; `events_arrive_once_and_in_order_through_a_bad_network` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-104 · Garder ce qui arrive après un trou, et le combler sans attendre la minuterie
- **Ce qui arrive :** Quake 3 se passe de tampon de réception parce qu'il remet tout le non-accusé dans chaque paquet. Salon ne le faisait pas et jetait ce qui suivait un trou : 200 événements mettaient 10 s, sans aucune perte, à 5 ms de gigue. Le récepteur garde 64 numéros d'avance et le dit ; trois accusés qui passent par-dessus un fragment le font renvoyer.
- **D'où on le sait :** `LEARNINGS.md:56` (ioquake3 `sv_snapshot.c` ; Fiedler, *Reliable Ordered Messages*) ; commit `f566450` ; `lib.rs:176-181` ; `recherche/fiabilite.toml` T2.
- **Mécaniques :** événements comptés une fois.
- **Question :** un paquet en retard bloque-t-il tout ce qui le suit ?
- **Pression :** `a_hole_in_the_queue_does_not_cost_what_follows` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-105 · Un message découpé s'accuse morceau par morceau
- **Ce qui arrive :** un renvoi repartait du fragment 0, et l'accusé ne disait rien d'un message incomplet. Un tuyau qui garde le début d'une rafale portait sans fin les mêmes fragments, et la file attendait derrière.
- **D'où on le sait :** `LEARNINGS.md:57` (renet `reliable.rs` ; Gaffer on Games, *Sending Large Blocks of Data*) ; commit `cc9ec13` ; `recherche/fiabilite.toml` T1.
- **Mécaniques :** gros messages.
- **Question :** un message plus gros que ce que le lien passe entre deux renvois finit-il par arriver ?
- **Pression :** `a_message_bigger_than_the_pipe_arrives` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-106 · Un accusé de morceaux dit à quel découpage il se rapporte
- **Ce qui arrive :** le récepteur garde ses morceaux jusqu'au premier fragment du nouveau découpage. Un accusé sur un nouveau lien pouvait nommer des fragments de l'ancien ; lu comme le nouveau, il marquait arrivé ce qui ne l'était pas, et rien ne le renvoyait. Trouvé par le chaos sur une page passée de WebRTC à WebSocket.
- **D'où on le sait :** commit `7167d5c` ; `LEARNINGS.md:58`.
- **Mécaniques :** gros messages ; changement de transport.
- **Question :** un accusé peut-il être lu dans un autre découpage que le sien ?
- **Pression :** `chaos_keeps_every_page_in_its_place_through_a_narrow_pipe` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-107 · Le débit se règle sur ce qui revient
- **Ce qui arrive :** salon ne ralentissait jamais : une file partait d'un coup dans un tampon plein, et un renvoi gardait son rythme sur un lien muet, 60 fois la taille d'un état en 5,9 s. Sans contrôle, à 100 kb/s et 128 Kio de tampon, cinq messages de 60 000 octets n'arrivaient pas en 120 s. Une fenêtre de dix paquets, doublée à chaque aller-retour, divisée par deux à chaque perte ; chaque minuterie expirée double l'attente suivante. Mesuré sur le banc seulement.
- **D'où on le sait :** commit `30510fb` ; `LEARNINGS.md:59` (RFC 9002 §7, §7.2 ; quinn `connection/mod.rs`) ; `recherche/fiabilite.toml` T1, T3.
- **Mécaniques :** toutes ; gros messages.
- **Question :** le commun ralentit-il sur un lien saturé ou muet ?
- **Pression :** `a_silent_link_is_not_hammered`, `a_message_bigger_than_the_pipe_arrives` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-108 · Un fragment su perdu passe devant la fenêtre, et chaque fragment pèse sa taille
- **Ce qui arrive :** un fragment perdu attendait derrière une fenêtre que seule son arrivée pouvait rouvrir. Compté pour un découpage entier, un petit événement fermait la fenêtre, et plus rien ne l'ouvrait. Trouvé par le chaos, entre 2 500 et 12 000 cas.
- **D'où on le sait :** commits `53bf348`, `0456c1a`, `bfb999e` ; `LEARNINGS.md:60` (RFC 9002 §7).
- **Mécaniques :** toutes.
- **Question :** le contrôle de débit peut-il retenir pour toujours ce qui doit le débloquer ?
- **Pression :** `chaos_keeps_every_page_in_its_place_through_a_narrow_pipe` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-109 · Une fenêtre qui se coupe en deux a un plancher, et se coupe à chaque perte
- **Ce qui arrive :** sans plancher, à 0 la fenêtre n'en sortait plus ; à 2 octets, un seul fragment la remplissait. Une attente infinie, prise pour « une perte par aller-retour », ne la coupait qu'une fois dans la vie d'un flux.
- **D'où on le sait :** commits `4c17e2d`, `7b8b6e5` ; `LEARNINGS.md:64` (RFC 9002 §7.2) ; `recherche/fiabilite.toml` T3.
- **Mécaniques :** toutes.
- **Question :** une fenêtre de débit peut-elle tomber à zéro ou ne jamais se réduire ?
- **Pression :** les tests de bord de `stream_edges.rs` (commit `0519842`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-110 · Un gros état remplacé plus vite que son renvoi n'arrive jamais
- **Ce qui arrive :** chaque nouvel état effaçait celui qui était en route : un état de 30 000 octets montré plus souvent qu'il ne pouvait arriver n'arrivait presque jamais, 8 sur 300 à 10 % de pertes. L'état en route finit, et le dernier montré entre-temps prend sa place. Un nouveau lien repart du dernier.
- **D'où on le sait :** commit `5f4cca8` ; `LEARNINGS.md:66` (renet #123) ; `lib.rs:25-29` ; `recherche/fiabilite.toml` T5.
- **Mécaniques :** gros états (plateau, image, liste).
- **Question :** un état volumineux et fréquent finit-il par s'afficher ?
- **Pression :** `a_big_state_refreshed_often_still_arrives`, `the_latest_state_wins_and_a_big_one_is_cut_to_fit` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-111 · Ce qui est monotone se compare avant d'écrire
- **Ce qui arrive :** le client écrivait la phase de tout paquet lu. Un paquet qui en doublait un autre faisait reculer la phase, et la réponse suivante partait périmée : le jeu la refusait. Seul le paquet le plus récent d'une exécution dit la phase ; les morceaux et accusés d'un vieux paquet restent lus, ils sont sûrs.
- **D'où on le sait :** commit `a542fbc` ; `LEARNINGS.md:67` (quinn 62b4f3f ; RFC 9000 §9.3) ; `recherche/fiabilite.toml` T6.
- **Mécaniques :** manches, questions, votes.
- **Question :** un message réordonné peut-il faire reculer une valeur qui ne doit qu'avancer ?
- **Pression :** `chaos_keeps_every_page_in_its_place` (retirer ce correctif le fait rougir).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-112 · Un état et un événement ne se transportent pas pareil
- **Ce qui arrive :** un état (un stick, ce qu'un écran montre) : le dernier gagne, un propriétaire, renvoyé jusqu'à l'accusé et à chaque reconnexion. Un événement (un vote) : compté une fois, dans l'ordre. Quake 3 a tenté de mêler les deux dans un canal, puis l'a désactivé.
- **D'où on le sait :** `LEARNINGS.md:70` (ioquake3 `SV_ReplacePendingServerCommands`, `#if 0` ; PlayroomKit ; AirConsole `RateLimiter`) ; `lib.rs:22-29`.
- **Mécaniques :** états tenus ; événements comptés une fois.
- **Question :** chaque message du jeu est-il déclaré état ou événement ?
- **Pression :** `chaos_never_loses_or_doubles_an_event` (`properties.rs`) ; `the_latest_state_wins_and_a_big_one_is_cut_to_fit` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-113 · Pas de delta d'état pour une salle d'amis
- **Ce qui arrive :** `@colyseus/schema` a produit des années de bugs avec ses deltas (« refId not found »). À quinze joueurs, l'état entier suffisait.
- **D'où on le sait :** `LEARNINGS.md:73` (`@colyseus/schema` #128, #196).
- **Mécaniques :** toutes.
- **Question :** la projection envoyée à un appareil est-elle entière, et si non, que coûte un delta ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-114 · Un appareil qui inonde ralentit lui-même, et personne d'autre
- **Ce qui arrive :** Colyseus n'a eu de limite de débit qu'en décembre 2025, sur un seul transport jusqu'en septembre 2026. Salon ne déconnectait pas un client qui inonde : au-delà d'un budget par lien, les paquets sont jetés sans être lus ; au-delà d'un budget d'événements, ceux-ci restent non accusés et l'émetteur les renvoie plus tard.
- **D'où on le sait :** `LEARNINGS.md:74` (Colyseus 8c14e2e, cf35ef3) ; `lib.rs:144-147` ; E3.
- **Mécaniques :** toutes.
- **Question :** un appareil qui inonde dégrade-t-il les autres ?
- **Pression :** `a_flooding_phone_does_not_slow_down_the_others` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-115 · Chaque événement porte la phase vue par son émetteur ; le jeu juge ce qui est périmé
- **Ce qui arrive :** chez boardgame.io, un compteur de version global jetait les réponses simultanées. Salon laissait chaque événement porter la phase vue par l'appareil : une réponse tardive pour une manche finie se reconnaît, et les réponses simultanées à la même question comptent toutes.
- **D'où on le sait :** `LEARNINGS.md:77` (boardgame.io #828) ; `lib.rs:162-165` ; `README.md:82` ; E5.
- **Mécaniques :** manches, questions, votes, réponses.
- **Question :** comment le jeu reconnaît-il une action faite pour une phase déjà passée, sans jeter les actions simultanées ?
- **Pression :** `an_event_carries_the_phase_its_sender_saw` (`salon.rs`) pour le transport ; E5 « à relier » pour les jeux.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SAL-116 · Un paquet coûte ce qu'il porte, jamais ce qu'il annonce
- **Ce qui arrive :** un paquet valide de 64 Kio contenait 9 000 à 13 000 fragments vides ; chacun allouait 1 024 places et les sommait : 480 paquets tenaient le fil de la salle trois secondes. Un message ne réserve que pour les fragments venus ; un paquet qui annonce plus de parties qu'un émetteur ne peut en mettre est du bruit.
- **D'où on le sait :** commit `da23aaa` ; `lib.rs:147-149`.
- **Mécaniques :** toutes.
- **Question :** ce qu'un paquet hostile coûte au commun est-il proportionnel à sa taille ?
- **Pression :** `a_packet_costs_what_it_carries_not_what_it_announces`, `a_packet_with_more_parts_than_a_sender_can_fit_is_garbage`, `what_waits_past_a_hole_is_bounded_in_fragments`, `what_joins_a_message_past_a_hole_counts_against_the_bound` (`stream_cost.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-117 · Une entrée hostile ou cassée n'atteint jamais le jeu, et ne coûte qu'à son lien
- **Ce qui arrive :** octets illisibles, paquets coupés, fragments absurdes, mauvais types : comptés et ignorés. Un paquet de plus de 64 Kio ferme ce lien seul. Un lien hostile doit laisser un téléphone honnête connecté, et devoir exactement ce qu'on lui devait.
- **D'où on le sait :** `lib.rs:141-150` ; commit `004ff87` ; E1, E2.
- **Mécaniques :** toutes.
- **Question :** que coûte au reste de la salle une session forgée ?
- **Pression :** `garbage_never_reaches_the_game`, `garbage_from_a_phone_is_ignored` (`properties.rs`) ; cibles `fuzz/fuzz_targets/decode.rs` et `session.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-118 · Un décodage consomme tout le cadre
- **Ce qui arrive :** un message valide suivi d'un second objet ou d'un suffixe passait pour valide. `codec::decode` exige que tous les octets soient consommés, pour les paquets, les messages du jeu et la télécommande.
- **D'où on le sait :** `LEARNINGS.md:238-245` ; `README.md:386-391`.
- **Mécaniques :** toutes.
- **Question :** chaque porte de décodage refuse-t-elle les octets en trop ?
- **Pression :** `crates/core/tests/framing/mod.rs`, à travers les vrais `Host` et `Client` ; remplacer le décodeur par un qui ignore les suffixes fait échouer les deux tests.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-119 · Un type désérialisable peut accepter le mauvais sens
- **Ce qui arrive :** une enum `untagged` et des champs tous optionnels peuvent reconnaître plusieurs formes. Chez Dioxus, un événement de souris se décodait en silence comme une annulation vide ; le chemin principal correct ne protégeait pas l'autre API publique. Un aller-retour du sérialiseur ne teste pas un autre producteur.
- **D'où on le sait :** `LEARNINGS.md:203` (Dioxus #5582) ; `LEARNINGS.md:243-245` ; `README.md:389-391`.
- **Mécaniques :** toutes ; fichiers de contenu, rejeux.
- **Question :** chaque forme décodée a-t-elle un discriminant explicite, et chaque porte publique ses entrées ambiguës testées ?
- **Pression :** tests négatifs de la passe page/app (tableaux à la place d'objets, identités, ports, octets, base64), partiels.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-120 · Une limite au-delà de ce que le découpage porte rendait Ok, puis bloquait la file
- **Ce qui arrive :** un message de plus de 1 024 fragments partait avec Ok, et chaque fragment était du bruit pour le récepteur : l'événement suivant n'arrivait jamais. Les limites sont ramenées à 962 560 octets quand on les donne, et `send` refuse tout de suite.
- **D'où on le sait :** commit `42f94e0` ; `README.md:97` ; `types.rs` (`Limits::bounded`) ; `recherche/fiabilite.toml` T8.
- **Mécaniques :** gros messages.
- **Question :** le plus gros message accepté à l'envoi est-il accepté à la réception ?
- **Pression :** `a_limit_past_what_fragments_carry_is_cut_down`, `the_largest_message_is_the_last_one_a_receiver_takes` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-121 · Les limites de la salle sont celles de l'autorité, dites à l'accueil
- **Ce qui arrive :** la page gardait les limites par défaut, l'hôte pouvait en avoir d'autres. Ce que la page croyait trop gros, elle le sautait et l'accusait ; un état qu'elle refusait tournait sans fin. L'accueil dit les limites, et un état trop gros est passé comme un événement.
- **D'où on le sait :** commit `5de18d6` ; `README.md:97` ; `lib.rs:182-183` ; `recherche/fiabilite.toml` T9.
- **Mécaniques :** toutes.
- **Question :** les deux côtés lisent-ils les mêmes limites ?
- **Pression :** `a_device_takes_the_limits_of_its_host` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-122 · Ce qui attend pour un appareil se borne aussi en octets
- **Ce qui arrive :** 256 messages de 60 000 octets étaient acceptés pour un téléphone qui ne répondait pas : 16 Mio pour un, 384 Mio pour une salle pleine. La file dit « pleine » au-delà de 1 Mio, et laisse toujours entrer un message, quelle que soit sa taille.
- **D'où on le sait :** commit `49fcad4` ; `README.md:96` ; `recherche/fiabilite.toml` T11.
- **Mécaniques :** gros messages.
- **Question :** la mémoire retenue pour un appareil muet est-elle bornée ?
- **Pression :** `what_waits_for_a_device_is_bounded_in_bytes`, `a_full_queue_costs_less_than_twice_its_size` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-123 · Sur un flux, aucune minuterie de renvoi
- **Ce qui arrive :** un WebSocket ne perd rien, mais la minuterie y envoyait des copies : à 300 kb/s, elles se tenaient 49 s devant ce qui était frais. Un paquet jeté par l'hôte avant le WebSocket se prouve par l'ordre : l'autre côté a fait écho d'un paquet parti après lui. Salon distinguait flux et datagramme par la taille du paquet : au-dessus d'une trame, ce n'est pas un datagramme.
- **D'où on le sait :** commit `982b2b2` ; `LEARNINGS.md:62` (RFC 9000 §9.3, par analogie) ; `lib.rs:180-182` ; `recherche/fiabilite.toml` T4.
- **Mécaniques :** toutes.
- **Question :** sur un transport fiable et ordonné, le commun renvoie-t-il ce que le transport garantit déjà ?
- **Pression :** `a_lossless_link_carries_no_copies`, `a_stream_that_drops_a_packet_recovers_it` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-124 · Un flux vivant et muet doit avoir une sortie
- **Ce qui arrive :** sans minuterie, un trou dans ce qui prouve une perte laisse le flux vivant et muet, sans erreur. Douze mille cas du chaos étroit l'ont trouvé. Avec quelque chose en attente et aucun accusé qui bouge pendant trois fois le délai d'absence, chaque côté lâche le lien, et la reconnexion reprend par un chemin déjà éprouvé. Le jeu voit l'appareil partir et revenir.
- **D'où on le sait :** commits `0b9a076`, `53bf348` ; `LEARNINGS.md:63` ; `lib.rs:119-124` ; `types.rs:288-295`.
- **Mécaniques :** toutes.
- **Question :** existe-t-il un état où le lien reste ouvert sans que rien ne progresse ?
- **Pression :** `a_stream_with_a_hole_in_its_proof_comes_back`, `a_slow_stream_is_not_taken_for_a_mute_one` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-125 · Seul un accusé qui bouge prouve un progrès, file par file, pour ce qui est parti
- **Ce qui arrive :** un état remontré sur un flux bloqué remettait la garde à zéro : un écran rafraîchi assez souvent n'était jamais lâché. Des états qui passaient cachaient un événement bloqué à côté. Un état qui attend la fenêtre ne doit rien à l'autre côté. Un accusé qui ne nomme qu'un vieux découpage ou un vieil état ne prouve rien. C'est un mutant survivant qui a montré la dernière erreur, dans la garde elle-même.
- **D'où on le sait :** commits `355064c`, `7a0498b`, `53bf348`, `31b1201`, `08ed767` ; `LEARNINGS.md:63`.
- **Mécaniques :** toutes.
- **Question :** ce qui sert de preuve de progrès peut-il être produit par le côté qui attend ?
- **Pression :** `a_stream_with_a_hole_in_its_proof_comes_back` (`salon.rs`) ; campagne de mutants.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-126 · Un test qui bute sur sa propre borne ne prouve rien
- **Ce qui arrive :** 256 messages de 40 000 octets atteignaient la borne de 1 Mio : le test s'arrêtait à son propre envoi. Avec 256 de 4 000 octets, il passe, et échoue sur le code d'avant.
- **D'où on le sait :** commit `5807e66`.
- **Mécaniques :** toutes.
- **Question :** un test atteint-il vraiment le cas qu'il nomme ?
- **Pression :** le test de `next_deadline` corrigé.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Le temps partagé et la mesure

### SAL-127 · L'aller-retour se mesure par écho, avec le temps de garde
- **Ce qui arrive :** chaque paquet porte son heure d'envoi et l'écho de la dernière heure reçue, avec le temps qu'il l'a gardée. Lissage à 10 %. Quake 3 note 999 pour « inconnu ».
- **D'où on le sait :** `LEARNINGS.md:83` (Fiedler, *Reliability over UDP* ; Quake 3) ; `lib.rs:172-175`.
- **Mécaniques :** toutes.
- **Question :** comment le commun mesure-t-il la latence réelle de chaque appareil ?
- **Pression :** `round_trips_and_the_host_clock_are_measured` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-128 · L'horloge de l'autorité vue d'un appareil se garde au plus petit aller-retour, et se mesure en continu
- **Ce qui arrive :** AirConsole ne la mesure qu'une fois ; HappyFunTimes ne filtre pas. Salon gardait la mesure faite au plus petit aller-retour, et mesurait sans cesse ; la page l'utilisait pour des comptes à rebours en phase avec la TV.
- **D'où on le sait :** `LEARNINGS.md:84` (AirConsole `getServerTime` ; HappyFunTimes `syncedclock.js`) ; `README.md:174`.
- **Mécaniques :** comptes à rebours, rythme, buzzer.
- **Question :** comment un appareil estime-t-il l'écart de son horloge avec celle de la partie qui fait foi ?
- **Pression :** `round_trips_and_the_host_clock_are_measured` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-129 · Horodater un geste pour départager, mais borner l'heure annoncée
- **Ce qui arrive :** un buzzer se départage à l'heure du geste, pas à celle de l'arrivée. Mais un téléphone ne doit pas pouvoir antidater.
- **D'où on le sait :** `LEARNINGS.md:85` (AirConsole, documentation) ; `README.md:61`.
- **Mécaniques :** buzzer, réflexes, course.
- **Question :** de combien un appareil peut-il prétendre avoir agi plus tôt ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-130 · La couche réseau avance par échéances, pas par pas fixes
- **Ce qui arrive :** salon ne simulait rien : il avançait par échéances (`next_deadline`), comme quinn-proto, ce qui s'écartait du pas fixe du manifeste d'alors. Dans le nouveau contrat, le pas régulier est celui du jeu.
- **D'où on le sait :** `LEARNINGS.md:330`.
- **Mécaniques :** toutes.
- **Question :** le transport doit-il suivre le pas du jeu, ou ses propres échéances ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les minuteries du système

### SAL-131 · macOS regroupe les minuteries des programmes sans fenêtre
- **Ce qui arrive :** un jeu à 60 Hz voyait des pas espacés de 21 à 28 ms sous charge. La salle dormait jusqu'à 2 ms de l'échéance puis attendait en cédant la main : sur 3 s, le 99ᵉ centile passait d'environ 20,9 à 18,8 ms. Dans le nouveau contrat, le pas vit dans un onglet ; le même genre de politique s'applique à un navigateur en arrière-plan.
- **D'où on le sait :** `LEARNINGS.md:112`.
- **Mécaniques :** tout jeu à pas réguliers.
- **Question :** le pas de la partie qui fait foi reste-t-il régulier quand son programme n'a pas le premier plan ?
- **Pression :** la sonde `steps_come_on_time`, à lancer à la main, sans seuil.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-132 · Mesurer la politique et le lieu du retard avant d'agir
- **Ce qui arrive :** le dernier spin ne borne pas un réveil système tardif : il ne commence qu'après le retour de l'attente. Un essai contrôlé (28/10/2026) montre un effet causal des politiques : processus normal, 180 réveils et p95 de 21,8 ms ; `taskpolicy -c utility`, 58 et 122,7 ms ; `taskpolicy -b`, 22 et 211,3 ms. Une activité Foundation `latencyCritical` n'annule pas un état d'arrière-plan imposé. Augmenter le spin ou les budgets n'établirait rien.
- **D'où on le sait :** `LEARNINGS.md:113-143` (Apple, *Prioritize Work at the Task Level* ; `latencyCritical`).
- **Mécaniques :** tout jeu à pas réguliers.
- **Question :** d'où vient un retard de pas : la politique du processus, la minuterie, ou le calcul ?
- **Pression :** aucune ; une enquête datée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-133 · Une cadence demande une minuterie adaptée, pas seulement une priorité
- **Ce qui arrive :** la même attente donnait 17 ms p95 sur GitHub Linux, 61 à 70 ms sur GitHub macOS 15 et 26, 18 à 19 ms sur le Mac local. Le regroupement désactivé sur une VM : 17,6 ms ; réactivé : 71,8 ms. Une minuterie kqueue marquée critique (`NOTE_CRITICAL`) donnait 19 ms contre 71 à 77 ms, avec très peu de CPU.
- **D'où on le sait :** `LEARNINGS.md:144-161` (xnu `kern_event.c`, `timer_call.c`) ; `LEARNINGS.md:398-400`.
- **Mécaniques :** tout jeu à pas réguliers.
- **Question :** la minuterie qui rythme la partie dit-elle au système qu'elle est urgente ?
- **Pression :** `the_rooms_deadline_is_a_critical_timer` (voir SAL-136).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-134 · Une attente précise reste réveillable par le réseau et par l'arrêt
- **Ce qui arrive :** une minuterie précise ne doit pas empêcher un paquet ou un arrêt de réveiller la boucle. Salon enregistrait un événement utilisateur dès la création de la file ; la notification restait jusqu'à sa consommation, même arrivée entre la lecture à vide et l'appel au noyau ; le dernier émetteur se détruisait avant de notifier ; une file de capacité nulle était refusée. Sans `unsafe` ajouté.
- **D'où on le sait :** `LEARNINGS.md:398-417` (Nix 0.30.1 `src/sys/event.rs`) ; commit `a002ab2`.
- **Mécaniques :** toutes.
- **Question :** la boucle du relais ou de la partie peut-elle manquer un réveil entre « rien à lire » et « j'attends » ?
- **Pression :** six tests de file (bornes, ordre, producteurs concurrents, fenêtre vide puis attente, déconnexion, vieilles notifications) ; pas une preuve de temps réel.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-135 · La machine d'intégration continue n'est pas l'ordinateur de la soirée
- **Ce qui arrive :** les runners GitHub macOS donnaient des retards trois fois plus grands que le Mac local, pour une politique effective dont l'origine n'a pas été établie. Un réglage global du système (couper le regroupement) reste réservé aux VM jetables, jamais au produit ni au Mac de l'hôte.
- **D'où on le sait :** `LEARNINGS.md:144-157`.
- **Mécaniques :** tout jeu à pas réguliers.
- **Question :** une mesure de cadence dit-elle sur quelle machine, sous quelle politique, elle a été prise ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-136 · Une propriété qui ne se voit qu'en temps réel reçoit un test de structure
- **Ce qui arrive :** on pouvait retirer `NOTE_CRITICAL` avec tous les tests au vert. Le test compare l'événement entier remis au noyau, sans mesurer de délai. `fflags()` panique sur ce bit, alors que l'événement se compare. Le noyau l'a confirmé une fois à la main (`lskq`).
- **D'où on le sait :** commit `7bb4466` ; `LEARNINGS.md:507-513`.
- **Mécaniques :** tout jeu à pas réguliers.
- **Question :** un réglage critique pour la latence peut-il disparaître sans qu'aucun test ne rougisse ?
- **Pression :** `the_rooms_deadline_is_a_critical_timer`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-137 · Une sonde qui rattrape ses créneaux un par un invente des intervalles
- **Ce qui arrive :** l'ancien jeu de test rattrapait les créneaux manqués un par un, et créait des intervalles artificiels de 1 ms après un retard. La sonde saute désormais les créneaux manqués, compte les réveils distincts et le retard sur l'échéance. Une sonde qui imprime sans seuil n'est pas une régression.
- **D'où on le sait :** `LEARNINGS.md:124-127`, `LEARNINGS.md:502-505`.
- **Mécaniques :** mesure des budgets d'image et de pas.
- **Question :** la mesure d'une cadence compte-t-elle des réveils réels ?
- **Pression :** la sonde `steps_come_on_time`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Le serveur, les ressources et les pannes de processus

### SAL-138 · Écrire sur une sortie d'erreur fermée fait paniquer
- **Ce qui arrive :** `eprintln!` panique quand stderr n'existe plus : une ligne de journal sur le fil de la salle arrêtait le jeu.
- **D'où on le sait :** commit `c416b82` ; `README.md:90`.
- **Mécaniques :** toutes.
- **Question :** le relais et l'outil écrivent-ils leurs journaux sans pouvoir paniquer ?
- **Pression :** `a_closed_stderr_does_not_end_the_room` (`room_death.rs`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-139 · Une erreur passagère du noyau n'est pas une panne
- **Ce qui arrive :** l'attente `kevent` ne tolérait que `EINTR` ; un noyau à court de mémoire un instant arrêtait la salle. Le test confondait aussi un lien fermé avec une lecture expirée.
- **D'où on le sait :** commit `c416b82` ; `README.md:90`.
- **Mécaniques :** toutes.
- **Question :** quelles erreurs système le commun retente-t-il, et lesquelles l'arrêtent ?
- **Pression :** `crates/host/tests/room_death.rs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-140 · Une panique ne se rattrape pas sur un état à moitié modifié
- **Ce qui arrive :** une panique dans le jeu arrêtait la salle : « un état à moitié modifié ne vaut pas une partie qui continue ». Le processus sortait avec un code fixe, et les pages revenaient à la relance.
- **D'où on le sait :** `README.md:88-92`.
- **Mécaniques :** toutes.
- **Question :** après une faute du jeu, depuis quel état la partie reprend-elle ?
- **Pression :** `a_dead_room_ends_the_process` (`room_death.rs`).
- **Sous la nouvelle forme :** impossible par la forme : la mémoire du module est son état, que le commun copie et restaure, et « une entrée qui fait échouer le jeu est isolée et écartée » (contrat, « La forme d'un jeu »).
- **Étage :** commun.

### SAL-141 · Un fil unique possède la partie, et le jeu ne verrouille rien
- **Ce qui arrive :** salon donnait l'hôte et le jeu à un seul fil : le jeu était du code séquentiel, sans verrou.
- **D'où on le sait :** `crates/host/src/lib.rs:26-28`.
- **Mécaniques :** toutes.
- **Question :** le jeu peut-il recevoir deux entrées en même temps ?
- **Pression :** aucune nécessaire.
- **Sous la nouvelle forme :** impossible par la forme : « un jeu qui traite un événement à la fois ne connaît pas deux actions simultanées » (manifeste, « Le commun et les jeux »).
- **Étage :** commun.

### SAL-142 · Un délai de lecture s'applique à un appel système, pas à une requête
- **Ce qui arrive :** `set_read_timeout` borne chaque lecture : un pair qui envoie un octet toutes les 10 ms tient la connexion indéfiniment. L'adaptateur recalcule le temps restant avant chaque lecture et refuse les en-têtes incomplets.
- **D'où on le sait :** `LEARNINGS.md:340-344` (Rust std, `TcpStream::set_read_timeout`).
- **Mécaniques :** toutes.
- **Question :** chaque requête reçue par le relais a-t-elle une échéance totale ?
- **Pression :** un test réel qui envoie un octet toutes les 10 ms et doit atteindre la limite totale.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-143 · Admettre une connexion avant de créer son fil, avec une réserve pour l'ordinateur même
- **Ce qui arrive :** les quotas du protocole ne protègent pas les étapes qui précèdent son décodage. Salon admettait avant de créer le fil, un permis couvrant toute la connexion, et gardait des places à la boucle locale que le réseau ne peut pas prendre.
- **D'où on le sait :** `LEARNINGS.md:344-346` ; `crates/host/src/admission.rs:1-2` ; commit `3d45317`.
- **Mécaniques :** toutes.
- **Question :** une rafale de connexions venues du réseau peut-elle empêcher l'ordinateur de la soirée de joindre son propre relais ?
- **Pression :** le test d'admission (144 connexions, marge HTTP, réserve locale).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-144 · Le quota d'admission se dérive des limites de la salle et compte chaque porte
- **Ce qui arrive :** un plafond fixe de 96 clients réseau aurait empêché les 71 iPhones de Sidequest d'utiliser à la fois la porte de l'app et le WebSocket de secours. Les budgets d'écriture portent sur la réponse entière, pas sur chaque écriture partielle.
- **D'où on le sait :** `LEARNINGS.md:429-435` ; `README.md:98`.
- **Mécaniques :** toutes.
- **Question :** les quotas du relais suivent-ils le nombre d'appareils prévu, toutes voies comprises ?
- **Pression :** le test d'admission.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-145 · Un écrivain WebSocket appartient à sa connexion
- **Ce qui arrive :** une file d'écriture qui survit à son lecteur garde un fil orphelin. Salon bornait la sortie ; l'arrêt du lecteur interrompait le socket, et l'écrivain était rejoint avant de libérer le permis.
- **D'où on le sait :** `LEARNINGS.md:347-349`.
- **Mécaniques :** toutes.
- **Question :** la fin d'une connexion libère-t-elle tout ce qu'elle possède ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-146 · Lire et écrire un WebSocket sur deux fils, et écrire des trames entières
- **Ce qui arrive :** avec un seul fil, un paquet attendait qu'une lecture expire avant de partir : sur le lien de la TV, une gigue à chaque image. Les deux écrivent sous un seul verrou, des tampons entiers, pour que les trames ne s'entrelacent pas.
- **D'où on le sait :** `crates/host/src/ws.rs:1-6`.
- **Mécaniques :** toutes.
- **Question :** un envoi du relais attend-il une lecture ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-147 · Un descripteur de fichier par lien
- **Ce qui arrive :** trois descripteurs par WebSocket épuisaient les 256 que macOS donne par défaut à un terminal, avant que l'admission ne refuse : les liens suivants mouraient en silence. L'acceptation tournait aussi à vide quand il n'en restait plus.
- **D'où on le sait :** commit `7b8ea01` ; `README.md:98`.
- **Mécaniques :** toutes.
- **Question :** le relais tient-il le nombre d'appareils prévu dans les limites par défaut du système ?
- **Pression :** `a_link_takes_one_descriptor` (`room_death.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-148 · Un refus ferme sa connexion
- **Ce qui arrive :** l'app recevait un refus, et la connexion restait ouverte, avec son fil et son permis, tant que l'app ne partait pas.
- **D'où on le sait :** commit `47c7449`.
- **Mécaniques :** app iPhone.
- **Question :** que garde le relais d'un pair refusé ?
- **Pression :** `another_version_of_the_app_is_refused_with_a_reason` (`crates/host/tests/app_remote.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-149 · Les en-têtes d'isolation débloquent les fils de WebAssembly
- **Ce qui arrive :** une page ne peut utiliser les fils WebAssembly que servie avec `Cross-Origin-Opener-Policy` et `Cross-Origin-Embedder-Policy`.
- **D'où on le sait :** `crates/host/src/lib.rs:4-7`.
- **Mécaniques :** jeux qui veulent des fils dans la page.
- **Question :** la page de démarrage du relais envoie-t-elle ces en-têtes ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-150 · Ne jamais servir un fichier hors de son dossier
- **Ce qui arrive :** un serveur de pages qui suit les chemins demandés peut livrer n'importe quel fichier de la machine. Salon ne servait que le dossier `dist` du jeu.
- **D'où on le sait :** `crates/host/src/lib.rs:4-7`.
- **Mécaniques :** toutes.
- **Question :** le relais peut-il servir autre chose que sa page de démarrage ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** impossible par la forme : « le relais reste générique. Il ne sert qu'une page de démarrage » (contrat, « Relier les appareils »).
- **Étage :** commun.

## L'app iPhone et sa télécommande

### SAL-151 · Le budget appartient à la connexion, et se lit avant tout le reste
- **Ce qui arrive :** le budget appartenait au joueur, et se lisait après que sa place était prise : un nouveau nom à chaque trame passait tout. 100 000 hello sur un lien donnaient au jeu 199 999 événements pour un budget de 480. Une connexion est désormais le premier joueur qu'elle nomme ; ce qu'elle prétend ensuite n'est pas lu.
- **D'où on le sait :** commit `923f328` ; `README.md:152`.
- **Mécaniques :** app iPhone.
- **Question :** un pair peut-il échapper à son budget en changeant d'identité ?
- **Pression :** aucune nommée ; le faux client du protocole de l'app (commit `eef70bc`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-152 · Seule une commande affichée envoie des entrées, dans leur plage
- **Ce qui arrive :** la doc disait que seul un pad envoie entrées et tracés, et rien ne le tenait : une app sur la page téléphone atteignait le jeu avec n'importe quel état, et une inclinaison de -128 se lisait -1,008. Les valeurs sont ramenées à leur plage symétrique (-128 devient -127) ; ce qui croise un changement de télécommande est jeté ; un numéro plus ancien n'arrive pas.
- **D'où on le sait :** commit `dffe8af` ; `README.md:152`.
- **Mécaniques :** app iPhone ; sticks, inclinaison, dessin.
- **Question :** une entrée d'un appareil peut-elle arriver hors de sa plage, ou pour une commande qui n'est plus affichée ?
- **Pression :** `a_game_chooses_its_remote_and_hears_it` (`app_remote.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-153 · Ce qu'affiche l'app se calcule depuis l'état du jeu
- **Ce qui arrive :** salon demandait la télécommande au jeu après chaque pas, envoyait ce qui changeait, et la renvoyait après une reconnexion : aucun message à oublier. De même, sans état montré à son arrivée, l'écran d'un téléphone restait vide.
- **D'où on le sait :** `README.md:150`, `README.md:81`.
- **Mécaniques :** app iPhone ; page téléphone.
- **Question :** un appareil peut-il rester sans vue ?
- **Pression :** `by_default_the_app_gets_the_phone_page_and_keeps_it` (`app_remote.rs`).
- **Sous la nouvelle forme :** impossible par la forme : « ce qu'il montre est une vue par appareil, calculée depuis l'état » (contrat, « La forme d'un jeu »).
- **Étage :** commun.

### SAL-154 · Un pad muet a quitté ; une page peut se taire ; une seconde connexion prend la place sans départ
- **Ce qui arrive :** un pad qui ne dit rien pendant le délai d'absence a quitté, comme une connexion fermée ; une page peut rester muette aussi longtemps qu'elle veut. Une seconde connexion du même joueur prend la place de la première sans départ. Un inconnu prend la place d'un absent ou reçoit un refus ; l'absent revient en « retour » s'il est parmi les 256 derniers à avoir perdu sa place. Un mutant de la fermeture de la première connexion survivait : rien ne regardait ce qu'elle devenait.
- **D'où on le sait :** `README.md:152` ; commits `085d61e`, `eef70bc`.
- **Mécaniques :** app iPhone.
- **Question :** comment le commun décide-t-il qu'un appareil sans page est parti ?
- **Pression :** le test de seconde connexion de `085d61e`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-155 · Une vibration pour un joueur absent est perdue
- **Ce qui arrive :** une vibration gardée pour un joueur déconnecté arriverait trop tard pour avoir un sens.
- **D'où on le sait :** `README.md:153`.
- **Mécaniques :** vibration.
- **Question :** un effet ponctuel qui a passé son instant est-il encore joué ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## La page : panique, journal, ressources

### SAL-156 · Le crochet de panique s'exécute avant l'arrêt du runtime
- **Ce qui arrive :** même avec `panic=abort`, le crochet Rust tourne avant l'arrêt. Il peut envoyer un beacon et armer un rappel natif de rechargement ; un rappel Rust programmé après dépendrait du WebAssembly qui vient de s'arrêter. La borne de rechargements doit survivre au rechargement : stockage de session, avec arrêt si ce stockage est refusé. Après trois tentatives rapprochées, un message fixe remplace le rechargement.
- **D'où on le sait :** `LEARNINGS.md:181-189` (Rust `set_hook` ; W3C Beacon) ; `README.md:267-274` ; C10.
- **Mécaniques :** toutes.
- **Question :** une panique dans une page la ramène-t-elle à sa place, sans boucle de rechargements ?
- **Pression :** scénario `panic` de l'ancienne certification, qui déclenche une vraie panique WASM dans chaque moteur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-157 · Le journal enregistre l'entrée au moment où le cœur l'applique
- **Ce qui arrive :** enregistrer les paquets au bord du réseau ne garde pas l'ordre retenu par l'autorité. L'écrivain sature explicitement plutôt que de bloquer la simulation ou de prétendre que le rejeu est complet. Un point de contrôle après chaque lot permet de vérifier un préfixe après un arrêt brutal.
- **D'où on le sait :** `LEARNINGS.md:168-175` (enregistreur de Sidequest, `games/sidequest/crates/host/src/recording.rs`) ; `README.md:222-243`.
- **Mécaniques :** toutes.
- **Question :** le journal enregistre-t-il les entrées dans l'ordre exact où le jeu les a reçues ?
- **Pression :** les rejeux de non-régression de Sidequest.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-158 · Un journal se relit strictement, et son empreinte couvre l'état entier
- **Ce qui arrive :** une ligne complète abîmée ou un numéro manquant est refusé ; seules les entrées couvertes par le dernier point de contrôle sont rendues ; une dernière ligne tronquée est abandonnée et signalée. L'empreinte doit couvrir l'état complet, générateur aléatoire compris : une vue réseau ne suffit pas pour rejouer.
- **D'où on le sait :** `README.md:222-239`.
- **Mécaniques :** toutes.
- **Question :** un rejeu peut-il passer pour fidèle alors qu'une partie de l'état n'est pas comparée ?
- **Pression :** aucune nommée ici.
- **Sous la nouvelle forme :** impossible par la forme : « la mémoire entière de son module compte comme son état : le commun peut la copier, la restaurer et la comparer » (contrat, « La forme d'un jeu »).
- **Étage :** commun.

### SAL-159 · Attendre le disque n'a pas sa place sur le fil de la simulation
- **Ce qui arrive :** `flush()` attend le disque. Il était réservé aux commandes, aux tests et aux fils HTTP. Une erreur disque asynchrone se constatait à part.
- **D'où on le sait :** `README.md:241-245`.
- **Mécaniques :** toutes.
- **Question :** un appel du commun sur le chemin d'un pas peut-il attendre le disque ou le réseau ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-160 · Un nettoyage automatique ne supprime que ses propres fichiers
- **Ce qui arrive :** à l'ouverture d'un journal, seuls les fichiers du dossier de journaux de plus de 30 jours qui commencent par l'en-tête typé du commun étaient supprimés ; les fichiers étrangers restaient. Les rejeux versionnés ne contenaient pas de textes de joueurs.
- **D'où on le sait :** `README.md:245-250`.
- **Mécaniques :** toutes ; textes de joueurs.
- **Question :** ce que le commun supprime est-il prouvé à lui ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-161 · Une page qui envoie son journal découpe, borne, et garde `keepalive` pour la fin
- **Ce qui arrive :** une page solo envoyait son journal par HTTP : au plus 4 Mio en attente, en fragments typés de 8 Kio sous la limite de 64 Kio. Seule la requête terminale de `pagehide` dépense le petit quota `keepalive` du navigateur. Les sessions survivent à une suspension, et un rechargement remplace la session abandonnée.
- **D'où on le sait :** `README.md:252-265` ; `crates/web/src/journal.rs:263-266`.
- **Mécaniques :** jeux dont l'état vit dans la page.
- **Question :** ce qu'une page envoie en partant tient-il dans le quota du navigateur ?
- **Pression :** aucune nommée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-162 · Ce qui casse se marque d'une touche, et se range dans l'ordre de la partie
- **Ce qui arrive :** sur la TV, F8 marquait un bug. Les diagnostics typés, dont les paniques des pages, arrivaient au jeu dans l'ordre de la salle, pour qu'il ajoute un marqueur à son journal.
- **D'où on le sait :** `README.md:276-284`.
- **Mécaniques :** toutes.
- **Question :** un bug vu pendant une partie se retrouve-t-il à sa place dans l'enregistrement ?
- **Pression :** le scénario de certification `?salon-test-panic=1`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-163 · Toute ressource du navigateur a un propriétaire et une fin
- **Ce qui arrive :** un écouteur, un minuteur ou une boucle d'animation qui survit à sa vue rappelle du code mort. Salon donnait à chacun une durée de vie possédée, annulée à la destruction, même pendant son propre rappel. La dernière copie d'une connexion ferme le transport ; un rappel qui capture fortement son propriétaire crée un cycle. `forget` n'est pas une fuite en soi, mais ne vaut pas pour une vue recréée.
- **D'où on le sait :** `LEARNINGS.md:199`, `LEARNINGS.md:219-225` (Gloo `EventListener`, `Timeout` ; Leptos, cycle de vie) ; `README.md:345-356`, `README.md:372-376`.
- **Mécaniques :** page téléphone ; page TV.
- **Question :** après cent créations et destructions d'une vue, reste-t-il un abonnement ou un rappel ?
- **Pression :** `salon/tests/ui-browser.mjs` dans WebKit et Chromium.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-164 · Une réponse asynchrone appartient à l'opération qui l'a demandée
- **Ce qui arrive :** une réponse ne devient pas valable parce que l'écran a retrouvé le même état. Dans tier-list, fermer puis rouvrir une exportation retrouvait l'état qu'attendait l'ancienne. Un ticket d'opération protège l'application du résultat ; il n'annule pas la promesse native, dont les ressources doivent aussi être libérées. Leptos montre aussi comment perdre une notification entre lecture et abonnement.
- **D'où on le sait :** `LEARNINGS.md:200`, `LEARNINGS.md:226-231` (Leptos `spawn_local_scoped_with_cancellation`, #4853) ; `README.md:357-376`.
- **Mécaniques :** imports, exports, décodage d'images, wake lock.
- **Question :** une réponse tardive peut-elle modifier une vue après annulation, remplacement ou destruction de son opération ?
- **Pression :** cas réels de tier-list (export périmé, erreur tardive, import après réinitialisation) et de Punchline (wake lock).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SAL-165 · L'identité d'un élément ne dépend pas de sa position, et un éditeur garde la sienne
- **Ce qui arrive :** réordonner ou supprimer pendant une édition déplace texte, sélection et focus sur le mauvais objet. Recopier le texte après recréation ne garde ni le focus, ni le curseur, ni la composition. Une clé Rust ne prouve pas à elle seule le comportement du navigateur.
- **D'où on le sait :** `LEARNINGS.md:201`, `LEARNINGS.md:232-237` (Yew, listes avec clés) ; `README.md:378-382`.
- **Mécaniques :** listes, champs de texte, glisser-déposer.
- **Question :** un champ en cours d'édition reste-t-il le même nœud quand un état arrive ?
- **Pression :** les suites U1 de l'ancienne certification, sur de vrais nœuds.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### SAL-166 · Une saisie n'est pas une suite de touches
- **Ce qui arrive :** composition, collage et `contenteditable` ont leurs propres données et timings. Une validation pendant une composition coupe un mot. `isComposing` et le code 229 signalent une composition ; bloquer une nouvelle pression ne doit pas perdre le relâchement d'une commande tenue. Un événement synthétique ne certifie pas un vrai clavier IME ni celui de l'iPhone.
- **D'où on le sait :** `LEARNINGS.md:202`, `LEARNINGS.md:232-237` (Dioxus #5581) ; `README.md:363-366`.
- **Mécaniques :** saisie de texte.
- **Question :** le jeu valide-t-il un texte pendant une composition, ou perd-il un relâchement ?
- **Pression :** `ui-browser.mjs` (composition synthétique) ; vrai clavier en répétition.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### SAL-167 · Une vue reçue plusieurs fois ne rejoue ni action ni effet
- **Ce qui arrive :** un état dérivé se calcule ; les effets parlent au monde extérieur. Projeter deux fois la même vue ne doit rien rejouer. Salon gardait l'identité du dernier effet joué, remise à zéro avec la session ; ce n'était pas un dédoublonneur général d'événements désordonnés.
- **D'où on le sait :** `LEARNINGS.md:204`, `LEARNINGS.md:246-250` (Leptos, effets et valeurs dérivées) ; `README.md:367-370`.
- **Mécaniques :** son, vibration, animations de résultat.
- **Question :** réafficher une vue rejoue-t-il un son ?
- **Pression :** la répétition d'une vue de résultats Sidequest, sans nouvelle action ni retour.
- **Sous la nouvelle forme :** impossible par la forme : un effet ponctuel « porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas » (contrat, « La forme d'un jeu »).
- **Étage :** commun.

### SAL-168 · Il faut éprouver le vérificateur lui-même
- **Ce qui arrive :** chez Leptos, la CI restait verte avec trois tests échoués, faute de propager les erreurs des scripts. Salon faisait traverser les vraies commandes à des cas négatifs : sous-processus en erreur, DOM périmé, mauvaise catégorie CSS, assertion navigateur fausse, expiration, test sélectionné absent. Une nouvelle tentative réussie garde la trace du premier échec.
- **D'où on le sait :** `LEARNINGS.md:205`, `LEARNINGS.md:251-258` (Leptos #4840).
- **Mécaniques :** toutes.
- **Question :** chaque contrôle de l'épreuve sait-il échouer à travers la vraie commande ?
- **Pression :** les contrôles négatifs de l'outil.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-169 · Un glissement simulé dans WebKit commençait par un tap
- **Ce qui arrive :** le harnais faisait un tap complet puis un drag : un seul geste activait deux fois un bouton. La branche WebKit utilise un seul drag de pointeur natif, vérifié par les événements `isTrusted`. Un contexte fermé sur un geste bloqué est un échec du moyen de vérification, jamais un vert.
- **D'où on le sait :** `LEARNINGS.md:260-268`.
- **Mécaniques :** glisser-déposer, gestes tactiles.
- **Question :** le geste simulé par le harnais est-il le geste qu'un joueur fait ?
- **Pression :** le harnais, par les événements réels.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-170 · WebKit diffère l'accusé d'un mouvement sans bouton jusqu'au rendu suivant
- **Ce qui arrive :** sur une page dont la CSS est retenue, le mouvement est reçu, mais son accusé attend ; un événement qui n'est pas un mouvement le libère. Le harnais qui attendait cet accusé bloquait. Il envoie maintenant mouvement puis appui sans attendre, puis relâche toujours le bouton.
- **D'où on le sait :** `LEARNINGS.md:270-284` (WebKit `WebPage::mouseEvent`, `WebPageProxy::handleMouseEvent`, au commit de Playwright 1.64.0).
- **Mécaniques :** gestes à la souris dans le harnais.
- **Question :** le pilote de navigateur attend-il un accusé que le moteur retient ?
- **Pression :** le test du harnais qui garde les erreurs concurrentes et celles du relâchement.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-171 · Un écouteur peut être rappelé pendant son propre rappel
- **Ce qui arrive :** un clic qui ouvre le sélecteur de fichiers en cliquant son `input` réentre tout de suite dans l'écouteur du document. Une closure `FnMut` l'interdisait avant même que le jeu décide. Les écouteurs sont `Fn` ; l'état mutable vit chez son propriétaire, sans emprunt gardé pendant un appel DOM qui peut déclencher un événement.
- **D'où on le sait :** `LEARNINGS.md:286-299` (`wasm-bindgen` `closure.rs`) ; `README.md:349-354`.
- **Mécaniques :** page téléphone ; page TV.
- **Question :** un écouteur supporte-t-il d'être rappelé avant d'avoir fini ?
- **Pression :** un test WASM dans les deux moteurs qui redispatche l'événement pendant son rappel ; l'ancienne closure le fait rougir.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-172 · Le tas JavaScript ne compte pas la mémoire WebAssembly
- **Ce qui arrive :** `Runtime.getHeapUsage` du CDP ne comptait pas la mémoire WASM du contrôle négatif. Le harnais observe aussi les mémoires exportées par WebAssembly. Le RSS inclut Chromium et le GPU ; la mesure ne connaît ni la VRAM ni WebKit.
- **D'où on le sait :** `LEARNINGS.md:350-354` (CDP, domaine `Memory`).
- **Mécaniques :** toutes.
- **Question :** la mesure de mémoire bornée regarde-t-elle la mémoire du module ?
- **Pression :** le contrôle négatif du harnais.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-173 · Un outil de pannes peut se bloquer lui-même
- **Ce qui arrive :** Toxiproxy 2.13 n'a pas de délai sur sa connexion amont, et son arrêt attend sa boucle d'acceptation ; son API rend un 503 HTML après 25 s, pendant que `/version` reste sain. Un backlog TCP saturé a reproduit une coupure bloquée 7,6 s. Le harnais place devant la vraie destination un connecteur possédé, borné à 2 s et 128 connexions. Toxiproxy ne joue que TCP.
- **D'où on le sait :** `LEARNINGS.md:355-371` (Toxiproxy v2.13.0 `proxy.go`, `api.go`).
- **Mécaniques :** toutes.
- **Question :** l'outil qui injecte les pannes a-t-il ses propres délais, et son échec se distingue-t-il d'un échec du jeu ?
- **Pression :** régressions du connecteur (octets et FIN, échéance, nettoyage sur 503, voisin intact).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Le son de la TV

### SAL-174 · Un contexte audio reste suspendu sans geste, et seul `running` est une réussite
- **Ce qui arrive :** un contexte peut rester suspendu tant qu'un geste n'a pas autorisé le son. Salon le créait dans un geste natif et montrait son état. Un état autre que `running`, dont `interrupted` rencontré dans Safari, n'était jamais une réussite implicite.
- **D'où on le sait :** `LEARNINGS.md:375-380` (Web Audio 1.1 ; Chrome, autoplay et jeux).
- **Mécaniques :** son.
- **Question :** le commun sait-il, et montre-t-il, que le son joue vraiment ?
- **Pression :** `node salon/tests/audio-browser.mjs` dans Chromium et WebKit.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-175 · Aucun contexte audio avant un geste, pas même à l'import
- **Ce qui arrive :** Tone.js créait un contexte dès l'import, avec des effets de bord. Salon n'en créait aucun avant un geste, propriété vérifiée par un constructeur natif instrumenté. Howler confirme le besoin de déblocage, de suspension et de libération explicites.
- **D'où on le sait :** `LEARNINGS.md:421-427` (Tone.js #1102 ; Howler README).
- **Mécaniques :** son.
- **Question :** charger la page crée-t-il une ressource audio ?
- **Pression :** le constructeur instrumenté de `audio-browser.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-176 · Le temps audio place le son ; le temps de l'événement décide s'il vaut encore
- **Ce qui arrive :** `currentTime` ordonnance l'enveloppe de chaque source ; le temps fourni par le jeu décide de sa fraîcheur. Aucun son n'attend dans une file : bloqué, muet ou périmé, il est consommé. Une reconnexion établit une nouvelle référence silencieuse.
- **D'où on le sait :** `LEARNINGS.md:381-384`.
- **Mécaniques :** son.
- **Question :** un son en retard ou rattrapé se joue-t-il quand même ?
- **Pression :** `audio-browser.mjs` (doublons, expiration).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-177 · La somme des sons reste sous la pleine échelle
- **Ce qui arrive :** plusieurs sons simultanés saturent. Salon : quatre oscillateurs au plus, amplitude 0,12 chacun, volume maître au plus 1 ; leur somme reste sous la pleine échelle.
- **D'où on le sait :** `LEARNINGS.md:385-386`.
- **Mécaniques :** son.
- **Question :** le pire cas de sons simultanés sature-t-il ?
- **Pression :** `audio-browser.mjs` (budget, rendu `OfflineAudioContext`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-178 · Les sources audio ont une fin, et un contexte cassé se remplace au geste suivant
- **Ce qui arrive :** les sources terminées étaient arrêtées et déconnectées par un minuteur possédé ; cacher ou quitter la page les arrêtait. Un contexte fermé, ou dont la reprise restait bloquée, était remplacé au geste suivant ; un ancien résultat asynchrone ne possédait pas le nouveau.
- **D'où on le sait :** `LEARNINGS.md:386-390`.
- **Mécaniques :** son.
- **Question :** que devient le son après une suspension, une interruption ou un contexte fermé ?
- **Pression :** `audio-browser.mjs` (suspension, panne de construction, destruction répétée).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-179 · Le banc audio ne remplace pas la vraie TV
- **Ce qui arrive :** le banc ne mesure ni HDMI, ni Bluetooth, ni volume acoustique, ni les interruptions matérielles d'iOS. Un essai sur la vraie TV restait requis.
- **D'où on le sait :** `LEARNINGS.md:391-396` (item S9).
- **Mécaniques :** son.
- **Question :** le son a-t-il été entendu sur le matériel de la soirée ?
- **Pression :** aucune automatique ; la répétition.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les pressions : chaos, mutants, soirée simulée

### SAL-180 · Le chaos mêle les pannes de tout le catalogue, et retient un paquet précis
- **Ce qui arrive :** le chaos ne jouait qu'un réseau : les trous T1, T2 et T3 de la reconnexion passaient au travers. Il mêle désormais rechargements, onglets jumeaux, « Jouer ici », pages mortes en silence, vrais gels, départs, appareils retirés, changements de transport, redémarrages avec ou sans identités, hôte figé ou absent, retour du réseau, rafales dans les deux sens, paquet retenu, hello coincé. Il vérifie aussi un numéro par place, un appareil par jeton, pas de remise à zéro sans cause.
- **D'où on le sait :** commits `46e0ef7`, `9691763` ; `LEARNINGS.md:303`, `LEARNINGS.md:318` (Jepsen `combined.clj` ; turmoil 61891a3) ; `README.md:218` ; `recherche/reconnexion.toml` T12.
- **Mécaniques :** toutes.
- **Question :** les pannes du monde sont-elles jouées ensemble, et pas une par une ?
- **Pression :** `chaos_keeps_every_page_in_its_place` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-181 · Le banc a un débit et un tampon, et reste physique
- **Ce qui arrive :** le faux réseau ne connaissait que pertes, doublons et gigue : le chaos restait vert alors qu'un message plus gros que le tuyau n'arrivait jamais. Puis, trop étroit, il donnait à un flux le tampon du tuyau : des paquets de 16 Kio ne pouvaient pas entrer derrière 8 Kio, ce qu'aucun WebSocket ne fait.
- **D'où on le sait :** commits `0dfc504`, `1e39346` ; `recherche/fiabilite.toml` T7.
- **Mécaniques :** toutes ; gros messages.
- **Question :** le réseau simulé a-t-il un débit, un tampon et des messages à la taille maximale permise ?
- **Pression :** `chaos_keeps_every_page_in_its_place_through_a_narrow_pipe` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-182 · Le chaos vérifie qu'une page seule finit par jouer sans qu'on le lui demande
- **Ce qui arrive :** la phase de guérison reprenait toutes les pages, et l'état final acceptait « remplacé » : une page éjectée par son propre passé passait inaperçue. Une page seule doit maintenant finir par jouer sans geste ; sans le correctif du hello, la propriété échoue en 64 cas.
- **D'où on le sait :** commit `e4b275f`.
- **Mécaniques :** toutes.
- **Question :** la fin d'un scénario de pannes vérifie-t-elle l'état attendu, ou se contente-t-elle d'un état acceptable ?
- **Pression :** `chaos_keeps_every_page_in_its_place` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-183 · On mesure ce que vaut le chaos en retirant chaque correctif
- **Ce qui arrive :** retirer seul le correctif de T1, T2 ou T6 le fait rougir en moins de 3 000 cas ; la garde de T3, doublée par le rang des liens, après 3 717 cas sur un tirage. Une affirmation sur la sensibilité du chaos datait d'avant ce doublement, et était fausse.
- **D'où on le sait :** `LEARNINGS.md:318` ; commit `08d278e`.
- **Mécaniques :** toutes.
- **Question :** combien de cas faut-il au chaos pour voir chaque bug connu ?
- **Pression :** mesure à la main, par retrait de correctif.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-184 · Vérifier que la taille d'une campagne change vraiment
- **Ce qui arrive :** un commit a cru qu'un champ littéral masquait `PROPTEST_CASES`, et a ajouté une aide inutile ; proptest honorait déjà la variable. Le test qui compte les cas joués par la vraie propriété est resté.
- **D'où on le sait :** commits `8dda2ee`, `e717966`.
- **Mécaniques :** toutes.
- **Question :** une campagne annoncée plus longue joue-t-elle vraiment plus de cas ?
- **Pression :** `proptest_cases_really_widens_the_chaos` (`properties.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-185 · Les mutants montrent ce qu'aucun test ne regarde
- **Ce qui arrive :** une campagne a laissé 141 mutants sur 433 vivants dans le cœur : accusés, renvois, renumérotation et échéances n'avaient aucun test qui remarque leur absence. Chaque bug trouvé ainsi est reproduit avant son correctif et reste en régression. Un bug ouvert porte un `#[ignore]` avec sa raison : un test ignoré ne prouve rien.
- **D'où on le sait :** commit `283a015` ; `found_bugs.rs:1-5` ; `salon/AGENTS.md`.
- **Mécaniques :** toutes.
- **Question :** quels mutants du commun survivent, et pourquoi ?
- **Pression :** `found_bugs.rs` et les fichiers `*_edges.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-186 · Une fuite dépend de ce qui arrive, pas de la durée
- **Ce qui arrive :** de vraies fuites mises dans salon ne se voyaient qu'une fois que la soirée rechargeait des pages, envoyait de gros messages et gardait un lien vivant tout du long. Le test joue des tours d'une soirée simulée sur le vrai hôte et compte exactement les octets vivants. Il ne voit que les chemins qu'il emprunte : une fuite injectée à la main dit s'il la voit.
- **D'où on le sait :** commit `503b386` ; `salon/AGENTS.md` ; `README.md:304-309`.
- **Mécaniques :** toutes.
- **Question :** la mémoire du commun se stabilise-t-elle sur une soirée qui fait tout ce qu'une soirée fait ?
- **Pression :** `an_evening_leaves_nothing_in_the_room`, `a_game_that_keeps_what_it_hears_is_caught` (`crates/host/tests/evening.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-187 · Compter le coût en octets alloués, pas en temps
- **Ce qui arrive :** un temps dépend de la machine et de sa charge ; un compteur d'octets alloués, non. Salon mesurait ce qu'un paquet hostile coûte à la salle par l'allocateur.
- **D'où on le sait :** `salon/AGENTS.md` (`stream_cost.rs`).
- **Mécaniques :** toutes.
- **Question :** les tests de coût du commun sont-ils reproductibles d'une machine à l'autre ?
- **Pression :** `crates/core/tests/stream_cost.rs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-188 · Ce que les mutants ne distinguent pas de rien se retire
- **Ce qui arrive :** des `break` après une vidange ne changeaient rien : sans rien à dire, le tour suivant s'arrêtait seul. Les mutants ne pouvaient pas les distinguer de leur absence.
- **D'où on le sait :** commit `445f562`.
- **Mécaniques :** toutes.
- **Question :** un mutant survivant montre-t-il un test manquant, ou du code inutile ?
- **Pression :** la campagne de mutants.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les frontières et les noms

### SAL-189 · Les chemins du commun se nomment une fois, pour les deux côtés
- **Ce qui arrive :** l'offre WebRTC, la clé de la TV, le diagnostic et le WebSocket étaient servis à côté des pages du jeu. Nommés une fois, un jeu ne pouvait pas les masquer par erreur.
- **D'où on le sait :** `README.md:194-196` ; `lib.rs:287-298`.
- **Mécaniques :** toutes.
- **Question :** un nom partagé entre la page et le relais a-t-il une seule définition ?
- **Pression :** aucune nécessaire.
- **Sous la nouvelle forme :** impossible par la forme : « deux morceaux qui doivent s'accorder n'ont qu'une seule définition » (contrat, « Les types de bout en bout »).
- **Étage :** commun.

### SAL-190 · Ce qui reste en chaîne se lit en un seul endroit, et se valide à l'entrée
- **Ce qui arrive :** l'offre et la réponse WebRTC au format du navigateur, la clé du jeton dans `localStorage`, les paramètres d'URL et les noms d'événements restaient des chaînes. Chacune était lue en un seul endroit, validée à son entrée.
- **D'où on le sait :** `LEARNINGS.md:336`.
- **Mécaniques :** toutes.
- **Question :** quelles chaînes traversent encore une frontière du commun, et où sont-elles validées ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-191 · Le QR code traverse le fil en modules, pas en balisage
- **Ce qui arrive :** l'hôte fabriquait l'adresse et le code, et l'envoyait en modules typés ; la page le dessinait en SVG. Aucun balisage ne traversait le fil.
- **D'où on le sait :** `README.md:101-109` ; `types.rs` (`Qr`).
- **Mécaniques :** lobby.
- **Question :** ce que l'autorité envoie à afficher est-il une donnée typée ?
- **Pression :** aucune nécessaire.
- **Sous la nouvelle forme :** impossible par la forme : les messages sont des types partagés, et la vue se calcule depuis l'état (contrat, « Les types de bout en bout »).
- **Étage :** commun.

### SAL-192 · La page ne lit pas le DOM par chaîne
- **Ce qui arrive :** un identifiant ou une classe écrit à la main dans le Rust diverge du HTML. `soiree dom` lisait le HTML et le CSS et écrivait un module de constantes typées ; le check le régénérait et échouait s'il différait. Une classe n'existait que si le HTML ou le CSS la nommait.
- **D'où on le sait :** `README.md:176-192`.
- **Mécaniques :** page téléphone ; page TV.
- **Question :** un nom du DOM utilisé par le code existe-t-il dans la page ?
- **Pression :** `soiree check`, qui régénère et compare.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### SAL-193 · Les valeurs de style écrites par le jeu sont typées
- **Ce qui arrive :** une longueur passée à une couleur, un nombre non fini ou un caractère de contrôle dans une URL d'image cassent la page en silence. Des constructeurs les refusaient ; chaque propriété avait sa catégorie, déclarée dans sa feuille pour les variables.
- **D'où on le sait :** `README.md:311-341`.
- **Mécaniques :** page téléphone ; page TV.
- **Question :** le jeu peut-il écrire une valeur de style d'une mauvaise catégorie ?
- **Pression :** tests de compilation négatifs.
- **Sous la nouvelle forme :** impossible par la forme : « ce qui vient de l'extérieur est validé à l'entrée » et deux morceaux n'ont qu'une définition (contrat, « Les lois communes »).
- **Étage :** commun.

## Ce que salon avait écarté

### SAL-194 · Salon refusait l'exécution optimiste sur l'appareil ; le contrat la reprend
- **Ce qui arrive :** salon jetait l'exécution optimiste côté client de boardgame.io : l'écran qui comptait était la TV, branchée sur l'autorité.
- **D'où on le sait :** `LEARNINGS.md:92`.
- **Mécaniques :** prédiction.
- **Question :** aucune ; la décision est inversée.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète : le nouveau contrat prédit sur l'appareil en exécutant la même étape sur la projection (contrat, « Réagir sans attendre »).
- **Étage :** connaissance.

### SAL-195 · Une seule salle, quinze amis au plus
- **Ce qui arrive :** salon jetait la recherche de partie par HTTP, Redis, le multi-nœud et les jetons JWT, et le public de Jackbox (jusqu'à 10 000) : un hôte, une salle, quinze amis. Le service en ligne du nouveau contrat peut servir plusieurs salles à la fois.
- **D'où on le sait :** `LEARNINGS.md:90-91`.
- **Mécaniques :** toutes.
- **Question :** le relais en ligne doit-il tenir plusieurs salles, et avec quelles limites ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### SAL-196 · Un lien neuf plutôt qu'un redémarrage ICE
- **Ce qui arrive :** salon n'utilisait pas l'ICE restart de str0m : ouvrir un lien neuf était plus simple, et passait par la reconnexion déjà éprouvée.
- **D'où on le sait :** `LEARNINGS.md:94`.
- **Mécaniques :** toutes.
- **Question :** le commun a-t-il une seule façon de refaire un lien ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les manettes

### SAL-197 · Une manette est un appareil comme les autres, qui se débranche et revient
- **Ce qui arrive :** salon faisait entrer les manettes et le clavier de l'hôte par la même porte que les téléphones : un appareil local, avec son état tenu, débranché puis rebranché. Le jeu ne distinguait pas une manette débranchée d'un téléphone coupé.
- **D'où on le sait :** `README.md:84` ; `lib.rs:18-21`, `lib.rs:54` ; `types.rs` (`Why::Closed` : « or the gamepad was unplugged »).
- **Mécaniques :** manettes, clavier.
- **Question :** une manette débranchée en pleine partie est-elle un départ, et rebranchée, un retour au même joueur ?
- **Pression :** `a_gamepad_is_a_device_like_the_others` (`salon.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.
