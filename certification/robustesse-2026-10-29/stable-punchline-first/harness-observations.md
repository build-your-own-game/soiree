# Observations du harnais de games/punchline

Graine 7222, 1.6 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-punchline-2026-10-29T17-43-04-719Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | ✓ Saisie et ressources de la page téléphone : editing, lifecycle, async<br>– effects : Aucun son ni vibration dans cette page ; les actions de jeu partent des gestes, jamais de la projection des vues. Les effets ponctuels futurs devront déclarer leur scénario. |
| C2 | ✓ | reconnecté à sa place 1.7 s après une coupure de 3 s |
| C3 | ✓ | l'hôte a remarqué le téléphone endormi en 6.0 s<br>au réveil, reconnecté à sa place en 0.1 s |
| C4 | ✓ | après un rechargement, de retour à sa place en 0.3 s |
| E1 | ✗ | ✗ [rtc] le scénario s'est interrompu : aucun paquet salon vivant sur le vrai DataChannel UDP en 15 s<br>✓ 300 messages illisibles (58 en octets au hasard, 50 en frames texte, 192 paquets réels mutés, sur 698 capturés) : ignorés, les autres continuent |
| E2 | ✓ | une frame de 64 Kio + 1 octet ferme sa connexion en 0.00 s, code 1009, et elle seule<br>une frame de 4 Mio ferme sa connexion en 0.24 s, code 1009, et elle seule |
| E3 | ✓ | 104500 messages en 3 s d'un appareil accueilli : la TV et les téléphones reçoivent toujours (plus long silence vu : 0.3 s) |
| E4 | ✓ | un appareil du réseau qui rejoue le premier paquet de la TV reçoit Refused(NotAllowed) (13 octets au plus) et sa connexion se ferme en 1.0 s<br>un jeton inventé devient un appareil neuf (Welcome (appareil 8), le vrai téléphone est l'appareil 2) et ne prend la place de personne |
| T3 | ✓ | TV 1920×1080 : tout le texte tient<br>TV 1280×720 : tout le texte tient<br>TV 3840×2160 : tout le texte tient<br>TV 1280×1024 : tout le texte tient<br>TV 2560×1080 : tout le texte tient<br>TV 900×1200 : tout le texte tient |
| T4 | ✓ | aucun champ visible sous 16 px |
| R2 | – | lance --soak 30 pour l'endurance |
| R4 | ✓ | 4 mesures après GC : DOM, listeners, tas JS, mémoire externe/WASM et RSS Chromium/GPU dans les budgets ; ce cycle court ne remplace pas l'endurance de 30 min |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée

Dernières lignes d'erreur de l'hôte :

```
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
warning: patch `remote-web v0.2.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
salon: WebRTC link 5 dropped (stalled: true)
```