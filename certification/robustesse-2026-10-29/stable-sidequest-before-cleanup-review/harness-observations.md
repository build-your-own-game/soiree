# Observations du harnais de games/sidequest

Graine 7220, 2.3 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-sidequest-2026-10-29T17-28-32-282Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | finite question deck progresses in both directions from its end : effects<br>real GPU device/context loss restores pixels; retries and late acquisition are bounded : lifecycle, async<br>same authoritative view cannot duplicate actions or feedback : effects<br>real selector and remote continuity, composition and takeover resource cleanup : editing, lifecycle, async |
| C1 | ✓ | téléphone 1 (webkit) a rejoint en 1.2 s depuis l'ouverture<br>téléphone 2 (chromium) a rejoint en 0.5 s depuis l'ouverture<br>WebRTC réel : action causale, pertes/délai/réordonnancement sans repli TCP ; silence 25 s détecté en 3317 ms (1 repli(s) TCP autorisé(s)), puis nouvelle négociation UDP et même identité ; {"upstream":226,"downstream":113,"dropped":193,"delayed":87,"reordered":29,"forwarded":146} |
| R2 | – | lance --soak 30 pour l'endurance |
| R3 | – | --phones 2 < maximum 71 : la salle pleine n'est pas essayée |
| R4 | ✓ | 4 mesures après GC : DOM, listeners, tas JS, mémoire externe/WASM et RSS Chromium/GPU dans les budgets ; ce cycle court ne remplace pas l'endurance de 30 min |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée

Dernières lignes d'erreur de l'hôte :

```
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 2 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 19 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 20 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 4 phones, 4 TV; 599 ticks, longest gap 67 ms; inputs 17 accepted, 0 refused
```