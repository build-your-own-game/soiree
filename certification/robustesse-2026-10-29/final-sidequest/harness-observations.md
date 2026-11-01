# Observations du harnais de games/sidequest

Graine 7232, 2.3 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-sidequest-2026-10-29T18-06-39-536Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | finite question deck progresses in both directions from its end : effects<br>real GPU device/context loss restores pixels; retries and late acquisition are bounded : lifecycle, async<br>same authoritative view cannot duplicate actions or feedback : effects<br>real selector and remote continuity, composition and takeover resource cleanup : editing, lifecycle, async |
| C1 | ✓ | WebRTC réel : action causale saine puis sous pertes/délai/réordonnancement dans chaque sens sans repli TCP ; silence 25 s détecté en 3310 ms (1 repli(s) TCP autorisé(s)), puis nouvelle négociation UDP et même identité ; {"upstream":240,"downstream":125,"dropped":195,"delayed":113,"reordered":38,"forwarded":170,"directions":{"upstream":{"received":240,"dropped":154,"delayed":55,"reordered":18,"forwarded":86},"downstream":{"received":125,"dropped":41,"delayed":58,"reordered":20,"forwarded":84}}} |
| R2 | – | lance --soak 30 pour l'endurance |
| R4 | ✓ | 4 mesures après GC : DOM, listeners, tas JS, mémoire externe/WASM et RSS Chromium/GPU dans les budgets ; ce cycle court ne remplace pas l'endurance de 30 min |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée

Dernières lignes d'erreur de l'hôte :

```
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 19 ms; inputs 2 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 19 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 24 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 22 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 22 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 4 phones, 4 TV; 600 ticks, longest gap 17 ms; inputs 23 accepted, 0 refused
```