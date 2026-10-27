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
- **Le pare-feu de macOS bloque l'UDP de tout nouveau binaire, en silence.** Constaté le 27/10/2026 : le pare-feu est activé, `salon-demo` n'y était pas autorisé, et pas un seul paquet WebRTC n'est arrivé (la page TV est passée par WebSocket au bout de 3 s). Chaque `cargo build` produit un binaire neuf, que macOS considère comme une nouvelle app. Avant la soirée : lancer chaque jeu une fois et accepter la demande, ou signer les binaires avec un certificat stable. C'est un item du salon.
- **La confidentialité du réseau local d'Apple** (TN3179) : l'app iPhone doit demander la permission, et son premier envoi peut échouer avant la réponse. Un hôte macOS lancé depuis le Finder en a besoin aussi ; lancé depuis le Terminal, non.
- **Les manettes** sont lues par l'hôte natif, pas par la page TV : Chrome en limiterait le nombre (à vérifier), et la page TV n'a pas à relayer.

## Écarts au manifeste

1. **Respecté.** Le cœur est `#![no_std]` : ni horloge, ni threads, ni E/S, ni `HashMap`. Les valeurs du réseau passent par des types validés à leur construction (`Name`, les tailles, les numéros). Chaque état d'attente a une échéance (`Timing`). Aucune file n'est sans borne.
2. **Écart.** Pas de pas de simulation fixe. Raison : `salon` ne simule rien ; il avance par échéances (`next_deadline`), comme quinn-proto.
3. **Respecté.** `Host` et `Client` sont des fonctions de leurs entrées : temps, octets reçus, graine. Le hasard (jetons, gigue des reconnexions) vient d'une graine fournie par la coque.
4. **Respecté.** `salon-core` ne dépend que de `serde`, `postcard` et `rand_chacha`, sans `std`.
5. **Respecté.** WebRTC en http local, sans serveur internet (voir plus haut, le test Safari).
6. **Respecté.** Un DataChannel non ordonné et non fiable ; la fiabilité, l'ordre et l'abandon des états périmés sont faits ici, une fois pour tous les jeux.
7. **À faire.** Mesurer sur le Wi-Fi du salon, avec 15 téléphones : aller-retour, pertes, temps de reconnexion après un iPhone verrouillé.
