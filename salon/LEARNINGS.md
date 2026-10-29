# Ce que salon a retenu, et d'où

`salon` relie la TV, les téléphones (page web ou app) et les manettes à l'hôte. On l'a reconstruit à partir de trois familles de références, lues dans leur code et leur historique (méthode du manifeste) :

- des moteurs réseau Rust « sans entrées-sorties » : str0m (WebRTC), renet, quinn-proto ;
- des serveurs de salles : Colyseus, Nakama, boardgame.io ;
- des téléphones comme manettes, et la synchronisation d'état : AirConsole, Jackbox (client jackbox.tv et le serveur réimplémenté jonahbox), HappyFunTimes, PlayroomKit, geckos.io, les articles de Glenn Fiedler (*Gaffer on Games*), ioquake3.

Les clones ont servi le temps de la lecture ; les références ci-dessous (fichier, commit, issue) permettent de retrouver chaque source.

## Ce qu'on garde

### La forme de l'API

| Leçon | Source | Catégorie | Dans salon |
|---|---|---|---|
| Entrées et sorties par interrogation (`poll`), pas par rappels : en Rust, des rappels imposent `Arc<Mutex<…>>`. | str0m `src/lib.rs` ; quinn-proto | Usage réel | `Host::poll`, `Client::poll` |
| Un contrat tolérant : les entrées arrivent dans n'importe quel ordre, un `tick` en trop ne fait rien, l'échéance suivante se lit à part. str0m exige une vidange complète entre deux entrées, et l'a écrit après des bugs. | quinn-proto #924 ; str0m 81a14a25 | Usage réel | `tick`, `next_deadline` |
| Le temps est absolu, fourni par l'appelant, dès la construction. Des deltas ont caché l'arrêt d'un serveur (renet #3) ; une sentinelle « déjà passé » a purgé des paires ICE (str0m #832). `Instant` panique en WASM (quinn #2436). | renet #3 ; str0m #832, #843 ; quinn #2436 | Accident du langage | `Ms`, notre propre temps ; `new(now, …)` |
| Le temps ne recule pas : un `now` plus petit est ramené au précédent. | str0m `do_handle_timeout` | Usage réel | `Clock::advance` |
| Aucune file sans borne, ni dans le cœur ni dans les coques. | quinn #1131, #1729 ; renet `max_memory_usage_bytes` | Usage réel | bornes partout, débordement explicite |
| Une connexion de transport est jetable ; l'identité vit au-dessus. | renet #59 | Usage réel | appareil et lien séparés |
| Un banc à horloge simulée qui saute à la prochaine échéance, borné en nombre de pas, à graine fixe. | quinn-proto `tests/util.rs` | Usage réel | `salon-core/tests` |

### Identité, présence, cycle de vie

| Leçon | Source | Catégorie | Dans salon |
|---|---|---|---|
| Un identifiant public stable, jamais réattribué, et un secret à part. Le prénom n'est jamais une clé. | AirConsole `getDeviceId` ; Jackbox `id:role:secret` ; Colyseus `sessionId`/`reconnectionToken` ; boardgame.io `credentials` | Usage réel | `DeviceId`, `Token` |
| Utilisateur, session et présence sont trois choses ; indexer le jeu par session crée un fantôme à chaque reconnexion. | Nakama `tracker.go` | Usage réel | le jeu ne voit que des `DeviceId` |
| Une coupure n'est pas un départ. Colyseus a mis des années à séparer `onDrop` de `onLeave` (#348, #893) ; boardgame.io n'a eu `onPlayerLeave` qu'en 2026 (#1088, #1258). | idem | Usage réel | `Left { why }` |
| Valider le jeton avant tout effet de bord : un mauvais jeton suffisait à bloquer une salle. | Colyseus #962 | Usage réel | `Hello` vérifié avant toute mutation |
| Le plus récent gagne, et l'éjecté reçoit un message terminal auquel il ne répond jamais par une reconnexion automatique. Sans lui, deux onglets s'éjectent en boucle. | Nakama e5f52b53 (`single_socket`) ; Colyseus 4002 ; notre Punchline | Usage réel | `Control::Replaced`, `Status::Replaced` |
| Chaque rappel vérifie que sa connexion est encore la courante : une fermeture tardive tuait la connexion revenue. | Colyseus #950, `client.ref === ref` | Usage réel | lien détaché de son appareil dès qu'il est remplacé |
| La présence se déduit, elle ne se compte pas à la main. | Colyseus #423, a00d3e3 | Usage réel | dérivée de la table des appareils |
| Quand l'autorité meurt, on le dit à tout le monde, sinon les écrans restent figés. | Nakama #414 | Usage réel | `Control::Closing`, `Status::HostGone` |
| Une époque d'hôte, pour qu'un client sache que l'hôte a redémarré et jette ce qu'il savait. Les identifiants restent croissants après un redémarrage. | Colyseus DevMode `nextRefId` ; notre tier-list (bug 9) | Usage réel | `Epoch`, `Welcome`, `ClientOutput::Reset` |
| À l'arrivée : l'état complet d'abord, les événements ensuite. Une reconnexion remplace l'état, elle ne s'y ajoute pas. | Colyseus `_onMessage`, fb3a8af ; boardgame.io #727 ; Jackbox `welcome` | Usage réel | l'état est renvoyé à chaque nouveau lien |
| Rejoindre est idempotent : on répond « accepté » à chaque `Hello`. | Nakama #245 ; Fiedler, *Client Server Connection* | Usage réel | `Welcome` à chaque `Hello` |

### Vivacité

| Leçon | Source | Catégorie | Dans salon |
|---|---|---|---|
| Ni l'état ICE, ni `onclose`, ni `readyState` ne disent qu'un pair est vivant. Les mainteneurs de str0m le disent eux-mêmes ; iOS laisse un WebSocket `OPEN` après la mort du Wi-Fi. | str0m #1093 ; WebKit 247943, 259743 ; nakama-js #118, #148, #151 ; geckos.io #4, #14, #32 | Physique | un battement applicatif, dans les deux sens |
| 5 s sans paquet : un bon délai. On n'a pas besoin d'un « keepalive » à part : le trafic régulier en tient lieu. | Fiedler, *Client Server Connection* ; Colyseus 3 s × 2 | Usage réel | battement 250 ms, absent après 6 s |
| Sous les délais applicatifs, ceux du transport : 15 s en ICE-lite chez str0m, 30 s de « consent freshness » (RFC 7675). | str0m 947598e4 ; RFC 7675 | Physique | nos 6 s passent avant |
| Une reconnexion à intervalle croissant, avec du hasard, et qui ne compte pas un lien mort aussitôt comme un succès. | Colyseus `Reconnection.ts` (`minUptime` 5 s) ; Jackbox `reconnect()` | Physique | 250 ms → 2 s, remis à zéro après 5 s de lien sain |
| Jackbox abandonne après 30 s ; un téléphone de soirée, jamais tant que la page est visible. | Jackbox | Héritage | jamais d'abandon |
| str0m n'a aucun délai tant qu'aucun candidat distant n'est connu : la poignée de main a besoin du sien. | str0m `agent.rs` | Accident | `handshake` 10 s |

### Fiabilité et état

| Leçon | Source | Catégorie | Dans salon |
|---|---|---|---|
| Messages fiables renvoyés dans chaque paquet jusqu'à l'accusé, toujours à partir du plus ancien non accusé : ordre garanti sans tampon de réception. | ioquake3 `sv_snapshot.c` (`reliableAcknowledge`) ; Fiedler, *Reliable Ordered Messages* | Usage réel | flux d'événements |
| La fiabilité du transport s'arrête à la mort du lien : un événement envoyé juste avant une coupure est perdu, quel que soit le transport. Il faut des numéros au niveau de l'appareil, qui survivent aux reconnexions. | renet #59 ; geckos.io #28, #269 (le « fiable » émulé de geckos) | Usage réel | numéros par appareil et par époque |
| Un seul DataChannel non ordonné sans retransmission, et la fiabilité au-dessus. str0m partage 128 KiB entre tous les canaux d'une association, retransmet sans fin sur un canal fiable, et a ignoré `maxRetransmits:0` jusqu'à la 0.22. | str0m `src/sctp/mod.rs`, #1004, #976 ; geckos.io `connectionsManager.ts` | Physique | un seul canal, un seul code pour WebRTC, l'UDP de l'app et WebSocket |
| Un état : le dernier gagne, un seul propriétaire, renvoyé jusqu'à l'accusé. Mélanger états et événements dans un canal a été tenté par Quake 3, puis désactivé. | ioquake3 `SV_ReplacePendingServerCommands` (`#if 0`) ; PlayroomKit P3/P4 ; AirConsole `RateLimiter` | Usage réel | `show`/`set_state` contre `send` |
| Un paquet non fiable reste sous l'unité de transmission : un fragment perdu fait perdre tout le message. | Quake 3 `FRAGMENT_SIZE` ; renet #123 ; str0m MTU 1150 | Physique | paquets de 1 100 octets au plus, morceaux reconstitués |
| Grouper : un paquet par envoi et par lien. 300 petits messages par seconde font un diaporama ; chaque message DataChannel coûte 50 à 70 octets. | geckos.io #49, #68 | Physique | un paquet regroupe accusés, événements et état |
| Pas de delta d'état : il a produit des années de bugs (« refId not found »). À 15 joueurs, l'état entier suffit. | `@colyseus/schema` #128, #196 | Héritage | états entiers |
| Ne pas déconnecter un client qui inonde : jeter, et laisser la pression remonter. Colyseus n'a eu de limite de débit qu'en décembre 2025, et elle ne marchait que sur un transport jusqu'en septembre 2026. | Colyseus 8c14e2e, cf35ef3 | Usage réel | budget par lien, événements non accusés au-delà |
| Une file d'événements qui jette le plus ancien message d'un canal « fiable » est une perte silencieuse. | Colyseus `enqueueMessage` | Héritage | `send` rend `Full`, rien n'est jeté en silence |
| Un refus silencieux est un bug : on dit à l'émetteur pourquoi. | boardgame.io #723, #592 | Usage réel | `Refusal` typée |
| Un compteur de version global jette les réponses simultanées ; c'est au jeu de juger si un message est périmé, par type de message. | boardgame.io #828 | Usage réel | chaque événement porte la phase vue par l'émetteur |

### Temps et mesure

| Leçon | Source | Catégorie | Dans salon |
|---|---|---|---|
| L'aller-retour : chaque paquet porte son heure d'envoi et l'écho de la dernière heure reçue, avec le temps de garde. Lissage à 10 %. | Fiedler, *Reliability over UDP* ; Quake 3 (999 = inconnu) | Physique | `DeviceInfo::rtt` |
| L'horloge de l'hôte vue du client : garder la mesure au plus petit aller-retour, et mesurer en continu. AirConsole ne la mesure qu'une fois, HappyFunTimes ne filtre pas. | AirConsole `getServerTime` ; HFT `syncedclock.js` | Physique | `Client::host_time` |
| Horodater une entrée pour départager un buzzer, mais borner l'heure annoncée pour qu'un téléphone ne puisse pas antidater. | AirConsole (doc) | Usage réel | `DeviceEvent::Event { at }` |

## Ce qu'on jette

- Les délais d'internet : Nakama 15 s / 25 s, Quake 3 200 s, HappyFunTimes 15 s puis 7,5 s. On est sur un réseau local.
- La recherche de partie par HTTP, Redis, le multi-nœud, les jetons JWT : un seul hôte, une seule salle.
- Le public de Jackbox (jusqu'à 10 000) : 15 amis au plus.
- L'exécution optimiste côté client de boardgame.io : l'écran qui compte est la TV, branchée sur l'autorité.
- La falsification du portail captif de HappyFunTimes, qui cassait tous les deux ans.
- L'ICE restart de str0m : un lien neuf est plus simple.

## Ce que le navigateur et le matériel imposent

- **WebRTC en http local fonctionne.** Mesuré le 27/10/2026 avec un hôte str0m 0.24.1 en ICE-lite et une page servie en `http://` (pas de contexte sécurisé) :
  - Safari sur iPhone (iOS 26.5) ouvre un DataChannel non ordonné sans retransmission en 90 ms ;
  - 500 pings sur 500 ; aller-retour médian 4 ms, p95 7 ms, p99 67 ms, maximum 182 ms ;
  - le Mac était sur le partage de connexion de l'iPhone ; à refaire sur le Wi-Fi du salon.

  Safari cache son adresse locale derrière un nom `.local` : l'hôte l'apprend par le premier paquet du téléphone (candidat *peer-reflexive*), comme geckos.io.
- **Pas de gyroscope dans une page `http://`** : iOS refuse d'office la permission hors HTTPS. C'est ce qui a tué HappyFunTimes. L'inclinaison et la visée au gyroscope passent par l'app. Accepté.
- **Un Wi-Fi sans internet** : iOS et Android peuvent basculer sur le réseau cellulaire (HappyFunTimes `apple-captive-portal-handler.js`). À vérifier au salon.
- **L'isolation des clients** sur beaucoup de box empêche les téléphones de voir l'hôte (HappyFunTimes `troubleshooting.md`). Un petit routeur dédié la règle ; certains ne prennent que 13 ou 14 clients.
- **Sans C : la cryptographie de WebRTC en Rust seul** (`crates/crypto`). Les fournisseurs de str0m compilent tous du C : `aws-lc-rs`, OpenSSL, CryptoKit (Swift), et même `str0m-rust-crypto`, qui génère son certificat avec `rcgen` sur `aws-lc-rs`. `salon-crypto` reprend ce dernier (MIT ou Apache-2.0) et construit le certificat avec `p256` et `x509-cert`.
- **BoringSSL refuse en silence un certificat qui ne peut pas signer.** Constaté le 27/10/2026 : avec le profil « racine » de `x509-cert` (CA, `keyCertSign` seulement), str0m contre str0m se connectait, mais Chrome renvoyait son ClientHello sans fin, l'hôte ayant signé son échange de clés avec une clé qui n'y avait pas droit. Trouvé en changeant une chose à la fois (le moteur de chiffrement, puis le candidat de bouclage, puis le certificat). Le certificat est désormais une feuille qui peut signer (`digitalSignature`) ; un test le vérifie. Leçon générale : un test entre deux exemplaires du même code n'éprouve pas ce que l'autre côté exige.
- **Un seul certificat par lancement de l'hôte.** str0m panique s'il doit en générer un et ne le peut pas ; il en générait un par connexion, ce qui est lent (str0m #405). Il est fait au démarrage : une panne de hasard devient une erreur au lancement.
- **Le candidat de bouclage.** L'hôte annonce aussi `127.0.0.1` : une page de l'ordinateur même (le harnais, un essai) passe par la boucle locale, que le pare-feu ne filtre pas. Un téléphone ignore ce candidat. Un lien venu de la boucle est local, comme un WebSocket venu de la boucle.
- **La TV toujours en WebSocket.** Elle est sur l'ordinateur même : un WebSocket local ne perd rien et porte des paquets de 16 Kio, contre 1 100 octets sur un DataChannel. Une TV en WebRTC, avant le candidat de bouclage, était refusée (« seul l'ordinateur du jeu peut être la TV ») et réessayait sans fin.
- **Le rythme de la salle.** macOS regroupe les minuteries des programmes sans fenêtre : un jeu à 60 Hz voyait des pas espacés de 21 à 28 ms sous charge. La salle dort jusqu'à 2 ms de l'échéance puis attend en cédant la main : sur un essai de 3 s, le 99ᵉ centile passe d'environ 20,9 à 18,8 ms, et le pire de 21–24 à 19,5 ms (`steps_come_on_time`, à lancer à la main).
- **Le dernier spin ne borne pas un réveil système tardif.** Il ne commence
  qu'après le retour de `recv_timeout`. Un essai contrôlé le 28/10/2026 sur
  ce même `room::wait`, sans réseau ni calcul de jeu, montre un effet causal
  des politiques macOS : sur un échantillon fixe de 3 s par cas, le processus
  normal donne 180 réveils et des intervalles p95 de 21,8 ms ; un clamp
  `taskpolicy -c utility`, 58 réveils et 122,7 ms ; `taskpolicy -b`,
  22 réveils et 211,3 ms. Retirer explicitement ce dernier état dans l'enfant
  rétablit 180 réveils et 21,9 ms. Les temps CPU des enfants restent entre
  0,028 et 0,047 s. Ce n'est ni une garantie de latence ni la preuve que le
  runner GitHub subit l'une de ces politiques.

  Le probe `steps_come_on_time` saute désormais les slots manqués et mesure
  les réveils distincts ainsi que le retard sur l'échéance ; son ancien jeu
  de test rattrapait les slots un par un et créait des intervalles artificiels
  de 1 ms après un retard. Le diagnostic autonome
  `.github/scripts/macos-scheduling.py` (depuis la racine du commun) compare
  aussi le lancement direct avec les fonctions de lancement extraites du
  runner Cargo courant : groupes de processus, pipes, threads lecteurs et
  polling identiques. Il conserve commandes, empreintes, sorties et politiques
  lisibles dans un artefact, sans certifier un budget de jeu. Son lecteur natif
  ne modifie aucune politique ; il observe le processus et le thread principal
  avant libtest, pas la QoS du thread de salle.

  Une activité Foundation `userInitiated`, même `latencyCritical`, n'a pas
  annulé l'état d'arrière-plan explicitement imposé lors de ce même essai ;
  aucune dépendance Foundation n'a été ajoutée au produit. Apple décrit la
  [QoS comme une indication qui règle aussi la latence des minuteries](https://developer.apple.com/library/archive/documentation/Performance/Conceptual/power_efficiency_guidelines_osx/PrioritizeWorkAtTheTaskLevel.html)
  et réserve [latencyCritical](https://developer.apple.com/documentation/foundation/processinfo/activityoptions/latencycritical)
  aux rares activités qui ont besoin de la précision maximale. Il faut mesurer
  la politique et le lieu du retard sur la machine concernée avant de choisir
  un changement du shell ; augmenter le spin ou les budgets ne l'établirait pas.
- **Une cadence demande une minuterie adaptée, pas seulement une priorité.**
  Le 29/10/2026, le même `room::wait` autonome donne 17 ms p95 sur GitHub
  Linux, 61–70 ms sur GitHub macOS 15/26, et 18–19 ms sur le Mac local.
  Une intervention sur une même VM macOS 15 établit l'effet du regroupement
  des minuteries : activé 65,9 ms ; désactivé 17,6 puis 17,5 ms ; réactivé
  71,8 ms. Le réglage initial est restauré et relu. Ce contre-test reste
  réservé aux VM jetables, jamais au produit ni au Mac de l'hôte.
  Sur macOS 26 hébergé, deux variantes C ne différant que par `NOTE_CRITICAL`
  donnent 71,0/76,8 ms pour la minuterie kqueue normale et 19,1/19,0 ms pour
  la critique, avec moins de 0,011 s CPU par échantillon de 3 s. Le noyau Apple
  [associe ce drapeau à l'urgence critique](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c)
  et [calcule la marge de regroupement selon l'urgence et la politique effective](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/timer_call.c).
  Le mécanisme principal est établi ; l'origine exacte de la politique
  effective plus défavorable du runner ne l'est pas. Il faut intégrer une
  attente précise **et réveillable par le réseau et l'arrêt**, sans changer
  les budgets ; ce prototype de minuterie ne prouve pas encore cette
  intégration ni la charge de 50 téléphones. Mesures, commandes, limites et
  empreintes sont dans [`ci-timers-2026-10-29`](../certification/ci-timers-2026-10-29/README.md).
- **Le pare-feu de macOS bloque l'UDP de tout nouveau binaire, en silence.** Constaté le 27/10/2026 : le pare-feu est activé, `salon-demo` n'y était pas autorisé, et pas un seul paquet WebRTC n'est arrivé (la page TV est passée par WebSocket au bout de 3 s). Chaque `cargo build` produit un binaire neuf, que macOS considère comme une nouvelle app. Avant la soirée : lancer chaque jeu une fois et accepter la demande, ou signer les binaires avec un certificat stable. C'est un item du salon.
- **La confidentialité du réseau local d'Apple** (TN3179) : l'app iPhone doit demander la permission, et son premier envoi peut échouer avant la réponse. Un hôte macOS lancé depuis le Finder en a besoin aussi ; lancé depuis le Terminal, non.
- **Les manettes** sont lues par l'hôte natif, pas par la page TV : Chrome en limiterait le nombre (à vérifier), et la page TV n'a pas à relayer.

## Enregistrer et rejouer le même hôte

- **Usage réel, garder.** Le journal reçoit les entrées au moment où le cœur
  les applique : enregistrer les paquets au bord du réseau ne garderait pas
  l'ordre retenu par l'hôte. Le writer sature explicitement plutôt que de
  bloquer la simulation ou de prétendre que le replay est complet. Un
  checkpoint après chaque lot permet de vérifier un préfixe après un arrêt
  brutal. Source : l'enregistreur de Sidequest
  (`games/sidequest/crates/host/src/recording.rs`, avant sa mutualisation),
  et ses replays de non-régression.
- **Usage réel, garder.** Le faux réseau doit exécuter la boucle de production,
  pas une réécriture de celle-ci. `Driver` est donc appelé par la salle réelle
  et par `simulation::Network`, avec la même priorité entre réseau, ticks et
  événements. L'horloge et le réseau sont des entrées. Source :
  [FoundationDB, Simulation and Testing](https://apple.github.io/foundationdb/testing.html).
- **Runtime et navigateur, garder.** Le panic hook Rust s'exécute avant
  l'arrêt du runtime, même avec `panic=abort`. Il peut transmettre un beacon
  et armer un callback natif `location.reload` ; un callback Rust programmé
  après la panique dépendrait encore du WASM qui vient de s'arrêter. Le
  beacon ne demande aucun traitement de réponse côté page, et peut finir
  pendant son remplacement. La borne de rechargements doit survivre au
  rechargement, donc `sessionStorage`, avec arrêt si ce stockage est refusé.
  Sources : [Rust, set_hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html),
  [W3C Beacon](https://www.w3.org/TR/beacon/).

## Bibliothèques web Rust : pistes à éprouver

Revue du 27 octobre 2026. Sources : documentation, implémentations et
correctifs de Gloo, Leptos, Yew et Dioxus. Les applications ci-dessous sont
nos déductions ; ce tableau ne déclare aucune nouvelle garantie acquise.

| Leçon et source | Catégorie | Situation ici et vérification proposée |
|---|---|---|
| Toute ressource navigateur a un propriétaire et une fin : un listener ou un timer doit être désarmé avec sa portée. [Gloo EventListener, implémentation de Drop](https://docs.rs/gloo-events/0.3.0/src/gloo_events/lib.rs.html), [Gloo Timeout](https://docs.rs/gloo-timers/latest/gloo_timers/callback/struct.Timeout.html), [Leptos, cycle de vie](https://book.leptos.dev/appendix_life_cycle.html). | Usage réel, garder ; RAII est une réponse Rust. | `remote-web::Listener` et les timers de la manette Sidequest ont déjà un `Drop`. Plusieurs callbacks sont conservés par `forget` : ce choix peut être correct pour toute la durée de la page, mais pas pour une vue recréée. Tester des créations/destructions répétées, le nombre d'abonnements et l'absence de callbacks après destruction ; ne pas conclure à une fuite sur la seule présence de `forget`. |
| Une tâche asynchrone appartient à une opération précise ; sa réponse ne devient pas valable simplement parce que l'écran a retrouvé le même état. Leptos [annule les tâches avec leur propriétaire](https://docs.rs/leptos/0.8.22/leptos/task/fn.spawn_local_scoped_with_cancellation.html). Ses [correctifs de concurrence #4853](https://github.com/leptos-rs/leptos/pull/4853) montrent aussi comment perdre une notification entre lecture et abonnement. | Usage réel, garder ; les verrous SSR précis ne se transposent pas au navigateur. | Salon protège déjà les callbacks réseau par une génération. Étendre l'analyse aux imports, exports et décodages d'images. Dans tier-list, `media::export` reconnaît une exportation encore attendue par `Modal::Export(None)` ; fermer puis rouvrir une exportation retrouve ce même état. C'est un cas à reproduire, pas un défaut confirmé par un test. Tester A/B terminés dans l'ordre inverse, fermeture/réouverture et erreur tardive ; utiliser une identité d'opération et, si possible, l'annulation. |
| L'identité d'un élément de liste ne doit pas dépendre de sa position. [Yew, listes avec clés](https://yew.rs/docs/concepts/html/lists). | Usage réel, garder. | Les identités de joueurs et de tuiles existent ; il faut encore vérifier leur projection DOM. Réordonner ou supprimer des éléments pendant une édition et contrôler que texte, sélection et focus restent attachés au bon objet. Une clé Rust ne prouve pas à elle seule ce comportement du navigateur. |
| Une saisie n'est pas une suite de touches : composition, collage et `contenteditable` ont leurs propres données et timings. Dioxus [#5581](https://github.com/DioxusLabs/dioxus/pull/5581) ajoute `beforeinput`, `is_composing` et des tests de composition Unicode ; le correctif aligne aussi la lecture de `contenteditable` entre renderers. | Navigateur et usage réel, garder. | Ajouter des parcours de saisie et de mise à jour reçue pendant l'édition. Tester composition, sélection, collage et absence de validation prématurée dans WebKit et Chromium. Les événements synthétiques vérifient nos handlers ; ils ne remplacent pas un essai avec un vrai clavier IME ou le clavier iPhone. |
| Un type désérialisable peut accepter le mauvais sens : une enum `untagged` et des champs tous optionnels ou par défaut peuvent reconnaître plusieurs formes. Dioxus [#5582](https://github.com/DioxusLabs/dioxus/issues/5582) documente un événement de souris décodé silencieusement comme une annulation vide ; le chemin principal correct ne protégeait pas l'autre API publique. | Représentation et usage réel, garder ; aucun besoin de reprendre leur architecture. | Préférer un discriminant explicite pour nos messages et examiner toutes les portes publiques de décodage, dont les replays. Ajouter des entrées mal formées et ambiguës avec des attentes indépendantes. La passe page/app possède déjà des tests négatifs sur les tableaux à la place d'objets, les identités, ports, octets et base64 ; elle ne constitue pas un audit de tous nos décodeurs. |
| Un état dérivé se calcule ; les effets synchronisent avec le monde extérieur. [Leptos, effets et valeurs dérivées](https://book.leptos.dev/reactivity/14_create_effect.html). | Architecture, garder ; le graphe réactif est un choix d'implémentation. | Cela conforte les vues pures et l'état faisant autorité du manifeste. Éviter de recopier scores, présence ou sélection dans plusieurs états mutables. Tester qu'une même vue reçue peut être projetée plusieurs fois sans nouvelle action de jeu ni effet externe dupliqué ; les effets ponctuels demandent leur identité propre. |
| Il faut éprouver le vérificateur et la propagation de ses erreurs. Leptos [#4840](https://github.com/leptos-rs/leptos/pull/4840) rapporte trois tests échoués alors que la CI restait verte ; le correctif rétablit notamment la propagation des erreurs des scripts. | Outillage et usage réel, garder. | La petite campagne de mutants de l'outil est déjà un contrôle de sensibilité. Compléter par des fixtures volontairement invalides traversant les vraies commandes et wrappers : type CSS incorrect, module DOM périmé, échec navigateur et échec de sous-processus doivent tous donner une sortie non nulle. Une nouvelle tentative réussie doit conserver la trace de l'échec initial. |

Priorité proposée : cycle de vie des ressources, identité des opérations
asynchrones, puis continuité de saisie. Pour chaque piste, commencer par un
cas qui met notre code en défaut, puis mutualiser dans `salon-web` seulement
ce qui sert effectivement à plusieurs jeux. Les mécanismes de SSR,
d'hydratation, de SEO ou de routage général ne sont pas des exigences de nos
jeux ; leurs détails ne sont à reprendre que si un besoin réel apparaît.

## Mise en œuvre des leçons web (28 octobre 2026)

Les pistes précédentes ont donné des protections exécutées, avec des portées
séparées : primitives communes, intégration des jeux et tests du vérificateur.

- **Ressources.** `salon_web::ui::{Listener, Listeners, Timer, AnimationLoop}`
  ont une durée de vie possédée et sont `must_use`. La dernière copie de
  `Connection` ferme le transport. Le modèle conserve explicitement ses
  ressources, avec un callback faible pour éviter un cycle. Le test
  `salon/tests/ui-browser.mjs` observe les vrais abonnements et fermetures
  dans WebKit et Chromium : cent créations/destructions, absence de rappel
  tardif, annulation pendant un callback, copies de connexion.
- **Asynchrone.** `OperationSlot` remplace une opération ; `OperationScope`
  laisse coexister des opérations indépendantes. Détruire ou annuler leur
  propriétaire invalide les tickets. Tier-list possède les cas réels
  d'export périmé, d'erreur tardive et d'import après réinitialisation ;
  Punchline protège les demandes de wake lock. Le navigateur peut encore
  terminer sa Promise : ses résultats natifs périmés sont aussi libérés.
- **Édition.** `Composition` protège les nouvelles validations et raccourcis,
  tout en laissant passer les relâchements d'une commande tenue. Tier-list
  conserve ses lignes éditées ; Punchline conserve les formulaires de la
  même tâche ; Sidequest possède ses listeners avec ses vues. Les suites
  U1 comparent de vrais nœuds, valeurs, sélections et effets observables.
  Une composition synthétique ne certifie pas un clavier IME physique.
- **Décodage.** `codec::decode` exige que tous les octets du cadre soient
  consommés. Salon l'utilise pour les paquets et messages du jeu ; remote
  et le décodeur public de Sidequest le réutilisent. Les tests
  `core/tests/framing/mod.rs` traversent les vrais Host/Client dans les deux
  directions. Remplacer le décodeur par celui qui ignore les suffixes, dans
  une copie isolée, fait effectivement échouer les deux tests. Les enums
  JSON non étiquetées encore utilisées pour nombre/nom de flottant ont des
  formes disjointes ; aucune interdiction aveugle de toute enum `untagged`.
- **Effets.** `OnceEffects` est un watermark borné pour un flux d'identités
  ordonnées, remis à zéro avec sa session. Il ne décide pas quels effets
  sont voulus. La répétition d'une vue de résultats Sidequest est vérifiée
  sans nouvelle action ni feedback, avec un contrôle de test réellement
  exécuté. Les jeux dépourvus d'effet ponctuel justifient cette absence.
- **Vérificateurs et héritage.** Les cas négatifs traversent les commandes
  publiques : sous-processus en erreur, DOM périmé, mauvaise catégorie CSS,
  assertion navigateur fausse, expiration et test sélectionné absent ne
  peuvent donner du vert. `new` et `upgrade` fournissent la CI versionnée ;
  `check --published` refuse les dépendances communes locales ou d'une
  autre étiquette. `cert.json.ui` décrit les contrôles ou les raisons de
  non-applicabilité, et les commandes complémentaires utilisent le module
  navigateur du harnais installé, sans dépôt voisin.

Un contrôle négatif supplémentaire du harnais a révélé qu'un glissement
WebKit commençait par un tap complet puis un drag : un seul geste fabriquait
deux activations d'un bouton. La branche WebKit utilise désormais un seul
drag de pointeur natif, vérifié par les événements réels `isTrusted` ; les
taps tactiles restent exercés séparément. La limite sur les déplacements
tactiles WebKit est explicite, sans événement synthétique présenté comme
un geste natif. Cette correction ne prouve pas à elle seule la cause de
chaque blocage du pilote ; un contexte fermé pour geste bloqué reste un
échec du moyen de vérification, jamais une certification verte.

Un second contrôle causal a isolé une barrière du pilote : WebKit diffère
l’accusé d’un mouvement sans bouton jusqu’à une mise à jour de rendu ;
un événement non-mouvement force sa libération. C’est explicite dans
[`WebPage::mouseEvent`](https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebKit/WebProcess/WebPage/WebPage.cpp#L4001)
et [`WebPageProxy::handleMouseEvent`](https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebKit/UIProcess/WebPageProxy.cpp#L4577),
au commit figé par
[Playwright 1.64.0](https://github.com/microsoft/playwright/blob/v1.64.0/browser_patches/webkit/UPSTREAM_CONFIG.sh).
Sur une page native dont la CSS reste retenue, le mouvement est reçu mais
son accusé attend ; envoyer l’appui le libère immédiatement, avant la CSS.
Le harnais envoie désormais mouvement puis appui sans attendre ce premier
accusé, attend les deux résultats, puis poursuit le drag et relâche toujours
le bouton. Le test conserve aussi les erreurs concurrentes et celles du
relâchement. Ce résultat prouve un défaut de notre ancien enchaînement ; il
ne permet pas d’attribuer rétroactivement tous les blocages de certification
à cette seule cause.

La certification de Tier-list a aussi exercé un vrai clic qui ouvre le
sélecteur de fichiers en cliquant son `input` : ce second clic réentre
synchroniquement dans le listener du document. Le nouvel adaptateur
`Closure<dyn FnMut(Event)>` l’interdisait avant même que la politique
d’événements du jeu puisse intervenir. Les listeners utilisent désormais
`Closure<dyn Fn(Event)>` et des handlers `Fn` ; les captures mutables restent
explicites chez leur propriétaire (`Cell` ou `RefCell`, sans emprunt gardé
pendant un appel DOM réentrant). Cette distinction est celle de
[`wasm-bindgen::Closure`](https://github.com/wasm-bindgen/wasm-bindgen/blob/0.2.129/src/closure.rs).
Un test WASM dans les deux moteurs redispatche le même événement pendant
son callback, avec un garde de récursion fourni par l’appelant : les deux
invocations doivent être livrées immédiatement et la destruction doit
désabonner exactement ce callback. L’ancienne closure `FnMut` rend ce
contrôle rouge ; aucun événement n’est mis en file ni perdu par le helper.

## Écarts au manifeste

1. **Respecté.** Le cœur est `#![no_std]` : ni horloge, ni threads, ni E/S, ni `HashMap`. Les valeurs du réseau passent par des types validés à leur construction (`Name`, les tailles, les numéros). Chaque état d'attente a une échéance (`Timing`). Aucune file n'est sans borne.
2. **Écart.** Pas de pas de simulation fixe. Raison : `salon` ne simule rien ; il avance par échéances (`next_deadline`), comme quinn-proto.
3. **Respecté.** `Host` et `Client` sont des fonctions de leurs entrées : temps, octets reçus, graine. Le hasard (jetons, gigue des reconnexions) vient d'une graine fournie par la coque.
4. **Respecté.** `salon-core` ne dépend que de `serde`, `postcard` et `rand_chacha`, sans `std`.
5. **Respecté.** WebRTC en http local, sans serveur internet (voir plus haut, le test Safari).
6. **Respecté.** Un DataChannel non ordonné et non fiable ; la fiabilité, l'ordre et l'abandon des états périmés sont faits ici, une fois pour tous les jeux.
7. **À faire.** Mesurer sur le Wi-Fi du salon, avec 15 téléphones : aller-retour, pertes, temps de reconnexion après un iPhone verrouillé.
8. **Respecté.** Les messages sont les types `Up` et `Down` du jeu, encodés en postcard par `salon-core` : aucun JSON écrit à la main. Les chemins de salon sont nommés une fois (`salon_core::route`), pour l'hôte et la page. La démo calcule sa version depuis ses sources (`version.rs`) ; l'hôte et la page la tiennent de la même bibliothèque, `salon-demo`. Sa page passe par `soiree dom`, QR code compris : l'hôte l'envoie en modules typés (`Qr`), la page le dessine en SVG. Restent en chaînes, chacune lue en un seul endroit et validée à son entrée : l'offre et la réponse WebRTC, au format `RTCSessionDescription` du navigateur (la page l'écrit avec `json!`, l'hôte la lit dans le `SdpOffer` de str0m) ; la clé du jeton dans `localStorage` (sa valeur passe par `unhex` avant de devenir un `Token`) ; les paramètres `?seat=` et `?salon=ws` ; les noms d'événements du navigateur. `cert.json` écrit `/salon/ws` en toutes lettres : le harnais attaque en boîte noire.
