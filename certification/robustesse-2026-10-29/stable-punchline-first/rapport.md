# Certification de games/punchline

Graine 6839, 5.1 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-punchline-2026-10-28T02-54-06-104Z

| Item | État | Détail |
|---|---|---|
| U1 | ✗ | [ui] le scénario s'est interrompu : ui « Saisie et ressources de la page téléphone » : sortie 1 webkit: composition, form identity/caret, 20 redraws and stale native wake-lock completion passed node:internal/modules/run_main:111 triggerUncaughtException( ^ AssertionError [ERR_ASSERTION]: rotation check outlived Starting 0 !== 1 at writingForm (file:///Users/axel2/Projects/build-your-own-game/games/punchline/tools/ui-check.mjs:130:12) at async file:///Users/axel2/Projects/build-your-own-game/games/punchline/tools/ui-check.mjs:305:5 { generatedMessage: false, code: 'ERR_ASSERTION', actual: 0, expected: 1, operator: 'strictEqual', diff: 'simple' } Node.js v26.12.1  |
| P1 | ✗ | ✗ en jeu sous entrées : 179 images, 59.3 images/s, p50 16.7 ms, p95 16.7 ms, maximum 50.0 ms, 46 gestes pendant l'échantillon (image 50.0 ms > 33 ms ; 59.3 images/s < 60)<br>✓ avant partie : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms ; téléphones 4 WebKit + 4 Chromium |
| P2 | ✓ | sonde 1 (phone, webkit) : entrée → rendu 21.2 ms, budget 80 ms<br>sonde 2 (tv, chromium) : entrée → rendu 34.9 ms, budget 50 ms |
| C10 | ✓ | webkit : après une vraie panique WASM, même appareil 2, même jeton et même place<br>chromium : après une vraie panique WASM, même appareil 3, même jeton et même place |
| C1 | ✓ | téléphone 1 (webkit) a rejoint en 1.0 s depuis l'ouverture<br>téléphone 2 (chromium) a rejoint en 0.5 s depuis l'ouverture<br>téléphone 3 (webkit) a rejoint en 0.7 s depuis l'ouverture<br>téléphone 4 (chromium) a rejoint en 0.6 s depuis l'ouverture<br>téléphone 5 (webkit) a rejoint en 0.6 s depuis l'ouverture<br>téléphone 6 (chromium) a rejoint en 0.6 s depuis l'ouverture<br>téléphone 7 (webkit) a rejoint en 0.8 s depuis l'ouverture<br>téléphone 8 (chromium) a rejoint en 0.7 s depuis l'ouverture<br>[rtc] un téléphone sans `?salon=ws` ouvre son lien par WebRTC (« salon: link 11 opened (WebRTC, 1100 bytes) ») ; la salle est pleine (8/8), son entrée n'est pas jugée (il lit « PUNCH LINE PUNCH LINE TON PRÉNOM ? La partie est complète. C'EST MOI ! ») |
| C2 | ✓ | reconnecté à sa place 0.9 s après une coupure de 3 s |
| C3 | ✓ | l'hôte a remarqué le téléphone endormi en 6.0 s<br>au réveil, reconnecté à sa place en 0.1 s<br>téléphone 5 (webkit) : silence détecté par la page en 5.8 s, nouvelle tentative native en 5.8 s (avant tout événement close/error reçu) |
| C4 | ✓ | après un rechargement, de retour à sa place en 0.2 s |
| C5 | ✓ | le dernier onglet gagne, 1 WebSocket(s) ouvert(s) en 10 s |
| C7 | ~ | ~ 8 joueur(s) ont perdu leur place (partie neuve) ; ils lisent : « PUNCH LINE PUNCH LINE TON PRÉNOM ? La partie a recommencé : choisis ton prénom pour revenir. C'EST M »<br>✓ après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés |
| C8 | ~ | le 9ᵉ joueur est refusé ; ce qu'il lit : « PUNCH LINE PUNCH LINE TON PRÉNOM ? La partie est complète. C'EST MOI ! » (clair ?) |
| C9 | ✓ | stockage refusé : on rejoint quand même, en appareil neuf<br>stockage revenu : téléphone 1 (webkit) garde son jeton et l'appareil 10 après rechargement, à sa place<br>1 clé(s) du stockage corrompue(s) : la page s'ouvre et on rejoint en appareil neuf<br>8/8 téléphones à leur place après les essais de stockage |
| E1 | ✓ | 300 messages illisibles (58 en octets au hasard, 50 en frames texte, 192 paquets réels mutés, sur 2400 capturés) : ignorés, les autres continuent |
| E2 | ✓ | une frame de 64 Kio + 1 octet ferme sa connexion en 0.00 s, code 1009, et elle seule<br>une frame de 4 Mio ferme sa connexion en 0.24 s, code 1009, et elle seule |
| E3 | ✓ | 112500 messages en 3 s d'un appareil accueilli : la TV et les téléphones reçoivent toujours (plus long silence vu : 0.3 s) |
| E4 | ✓ | un appareil du réseau qui rejoue le premier paquet de la TV reçoit Refused(NotAllowed) (13 octets au plus) et sa connexion se ferme en 1.0 s<br>un jeton inventé devient un appareil neuf (Welcome (appareil 14), le vrai téléphone est l'appareil 2) et ne prend la place de personne |
| E6 | ✓ | 120 s de gestes au hasard sur 8 téléphones |
| T3 | ✓ | TV 1920×1080 : tout le texte tient (×2)<br>TV 1280×720 : tout le texte tient (×2)<br>TV 3840×2160 : tout le texte tient (×2)<br>TV 1280×1024 : tout le texte tient (×2)<br>TV 2560×1080 : tout le texte tient (×2)<br>TV 900×1200 : tout le texte tient (×2) |
| T4 | ✓ | aucun champ visible sous 16 px (×2) |
| D2 | ✓ | 23 écrans TV différents vus |
| R1 | ~ | 45 s à 200 ± 100 ms sans perte de connexion (« jouable » se juge au salon) |
| R2 | – | lance --soak 30 pour l'endurance |
| R3 | ✓ | salle pleine à 8 |

✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable

Dernières lignes d'erreur de l'hôte :

```
warning: patch `remote-web v0.2.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
warning: patch `remote-web v0.2.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
```