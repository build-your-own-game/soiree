# Certification de build-your-own-game/.chantier-modele

Graine 20261027, 4.8 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-.chantier-modele-2026-10-27T21-39-20-161Z

| Item | État | Détail |
|---|---|---|
| P1 | ✓ | avant partie : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms ; téléphones 4 WebKit + 4 Chromium<br>en jeu sous entrées : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms, 49 gestes pendant l'échantillon |
| P2 | ✓ | sonde 1 (phone, webkit) : entrée → rendu 25.4 ms, budget 80 ms<br>sonde 2 (phone, chromium) : entrée → rendu 23.5 ms, budget 80 ms |
| C10 | ✓ | webkit : après une vraie panique WASM, même appareil 2, même jeton et même place<br>chromium : après une vraie panique WASM, même appareil 3, même jeton et même place |
| C1 | ✓ | téléphone 1 (webkit) a rejoint en 0.6 s depuis l'ouverture<br>téléphone 2 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>téléphone 3 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 4 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>téléphone 5 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 6 (chromium) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 7 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 8 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>[rtc] un téléphone sans `?salon=ws` ouvre son lien par WebRTC (« salon: link 11 opened (WebRTC, 1100 bytes) ») ; la salle est pleine (8/8), son entrée n'est pas jugée (il lit « La partie est pleine. ») |
| C2 | ✓ | reconnecté à sa place 0.8 s après une coupure de 3 s |
| C3 | ✓ | l'hôte a remarqué le téléphone endormi en 5.9 s<br>au réveil, reconnecté à sa place en 0.1 s<br>téléphone 5 (webkit) : silence détecté par la page en 5.9 s, nouvelle tentative native en 5.9 s (avant tout événement close/error reçu) |
| C4 | ✓ | après un rechargement, de retour à sa place en 0.2 s |
| C5 | ✓ | le dernier onglet gagne, 1 WebSocket(s) ouvert(s) en 10 s |
| C7 | ✓ | après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés |
| C8 | ~ | le 9ᵉ joueur est refusé ; ce qu'il lit : « La partie est pleine. » (clair ?) |
| C9 | ✓ | stockage refusé : la page s'ouvre et propose de rejoindre (téléphone 1 (webkit) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage revient, et sa place avec)<br>1 clé(s) du stockage corrompue(s) : la page s'ouvre et propose de rejoindre (téléphone 2 (chromium) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage est remis, et sa place avec) |
| E1 | ✓ | 300 messages illisibles (68 en octets au hasard, 47 en frames texte, 185 paquets réels mutés, sur 2390 capturés) : ignorés, les autres continuent |
| E2 | ✓ | une frame de 64 Kio + 1 octet ferme sa connexion en 0.00 s, code 1009, et elle seule<br>une frame de 4 Mio ferme sa connexion en 0.24 s, code 1009, et elle seule |
| E3 | ✓ | 500 messages en 3 s d'un appareil accueilli, puis l'hôte a fermé sa connexion au bout de 0.0 s : la TV et les téléphones reçoivent toujours (plus long silence vu : 0.1 s) |
| E4 | ✓ | un appareil du réseau qui rejoue le premier paquet de la TV reçoit Refused(NotAllowed) (13 octets au plus) et sa connexion se ferme en 1.0 s<br>un jeton inventé est refusé (Refused(Closed)) et ne prend la place de personne |
| E6 | ✓ | 120 s de gestes au hasard sur 8 téléphones |
| T3 | ✓ | TV 1920×1080 : tout le texte tient (×2)<br>TV 1280×720 : tout le texte tient (×2)<br>TV 3840×2160 : tout le texte tient (×2)<br>TV 1280×1024 : tout le texte tient (×2)<br>TV 2560×1080 : tout le texte tient (×2)<br>TV 900×1200 : tout le texte tient (×2) |
| T4 | ✓ | aucun champ visible sous 16 px (×2) |
| D2 | – | pas de touche « advance » : la progression se juge au salon |
| R1 | ~ | 45 s à 200 ± 100 ms sans perte de connexion (« jouable » se juge au salon) |
| R2 | – | lance --soak 30 pour l'endurance |
| R3 | ✓ | salle pleine à 8 |

✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable

Dernières lignes d'erreur de l'hôte :

```
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
salon: WebRTC link 11 dropped (stalled: false)
warning: patch `remote-web v0.1.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
```