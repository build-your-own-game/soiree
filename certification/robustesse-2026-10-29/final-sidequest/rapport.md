# Certification de games/sidequest

Graine 6839, 4.9 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-sidequest-2026-10-28T02-11-59-440Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | same authoritative view cannot duplicate actions or feedback : effects<br>real selector and remote continuity, composition and takeover resource cleanup : editing, lifecycle, async |
| P1 | ✓ | avant partie : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms ; téléphones 4 WebKit + 4 Chromium<br>en jeu sous entrées : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms, 56 gestes pendant l'échantillon |
| P2 | ✓ | sonde 1 (phone, webkit) : entrée → rendu 49.3 ms, budget 80 ms<br>sonde 2 (phone, chromium) : entrée → rendu 52.3 ms, budget 80 ms |
| C10 | ✓ | webkit : après une vraie panique WASM, même appareil 2, même jeton et même place<br>chromium : après une vraie panique WASM, même appareil 3, même jeton et même place |
| C1 | ✓ | téléphone 1 (webkit) a rejoint en 0.7 s depuis l'ouverture<br>téléphone 2 (chromium) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 3 (webkit) a rejoint en 0.5 s depuis l'ouverture<br>téléphone 4 (chromium) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 5 (webkit) a rejoint en 0.5 s depuis l'ouverture<br>téléphone 6 (chromium) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 7 (webkit) a rejoint en 0.6 s depuis l'ouverture<br>téléphone 8 (chromium) a rejoint en 0.4 s depuis l'ouverture<br>[rtc] un téléphone sans `?salon=ws` a rejoint par WebRTC en 0.4 s (« salon: link 10 opened (WebRTC, 1100 bytes) ») |
| C2 | ✓ | reconnecté à sa place 1.1 s après une coupure de 3 s |
| C3 | ✓ | l'hôte a remarqué le téléphone endormi en 6.0 s<br>au réveil, reconnecté à sa place en 0.1 s<br>téléphone 5 (webkit) : silence détecté par la page en 5.8 s, nouvelle tentative native en 5.8 s (avant tout événement close/error reçu) |
| C4 | ✓ | après un rechargement, de retour à sa place en 0.2 s |
| C5 | ✓ | le dernier onglet gagne, 1 WebSocket(s) ouvert(s) en 10 s |
| C7 | ✓ | après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés |
| C9 | ✓ | stockage refusé : la page s'ouvre et propose de rejoindre (téléphone 1 (webkit) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage revient, et sa place avec)<br>stockage revenu : téléphone 1 (webkit) garde son jeton et l'appareil 2 après rechargement, à sa place<br>3 clé(s) du stockage corrompue(s) : la page s'ouvre et propose de rejoindre (téléphone 2 (chromium) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage est remis, et sa place avec)<br>8/8 téléphones à leur place après les essais de stockage |
| E1 | ✓ | 300 messages illisibles (58 en octets au hasard, 50 en frames texte, 192 paquets réels mutés, sur 2362 capturés) : ignorés, les autres continuent |
| E2 | ✓ | une frame de 64 Kio + 1 octet ferme sa connexion en 0.00 s, code 1009, et elle seule<br>une frame de 4 Mio ferme sa connexion en 0.24 s, code 1009, et elle seule |
| E3 | ✓ | 107500 messages en 3 s d'un appareil accueilli : la TV et les téléphones reçoivent toujours (plus long silence vu : 0.3 s) |
| E4 | ✓ | un appareil du réseau qui rejoue le premier paquet de la TV reçoit Refused(NotAllowed) (13 octets au plus) et sa connexion se ferme en 1.0 s<br>un jeton inventé devient un appareil neuf (Welcome (appareil 16), le vrai téléphone est l'appareil 2) et ne prend la place de personne |
| E6 | ✓ | 120 s de gestes au hasard sur 8 téléphones |
| T3 | – | la TV dessine sur un canvas : à vérifier au salon (S8) |
| D2 | ✓ | 23 écrans TV différents vus |
| R1 | ~ | 45 s à 200 ± 100 ms sans perte de connexion (« jouable » se juge au salon) |
| R2 | – | lance --soak 30 pour l'endurance |
| R3 | – | --phones 8 < maximum 71 : la salle pleine n'est pas essayée |

✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable

Dernières lignes d'erreur de l'hôte :

```
player #11: deleted after 90 s offline
summary: 8 players online, 13 phones, 3 TV; 601 ticks, longest gap 22 ms; inputs 1 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 601 ticks, longest gap 25 ms; inputs 0 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 601 ticks, longest gap 23 ms; inputs 1 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 600 ticks, longest gap 22 ms; inputs 0 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 600 ticks, longest gap 22 ms; inputs 2 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 600 ticks, longest gap 22 ms; inputs 0 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 600 ticks, longest gap 22 ms; inputs 0 accepted, 0 refused
summary: 8 players online, 13 phones, 3 TV; 601 ticks, longest gap 25 ms; inputs 1 accepted, 0 refused
summary: 7 players online, 10 phones, 1 TV; 600 ticks, longest gap 22 ms; inputs 12 accepted, 8 refused
```