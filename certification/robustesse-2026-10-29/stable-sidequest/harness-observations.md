# Observations du harnais de games/sidequest

Graine 7221, 2.3 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-sidequest-2026-10-29T17-40-09-078Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | finite question deck progresses in both directions from its end : effects<br>real GPU device/context loss restores pixels; retries and late acquisition are bounded : lifecycle, async<br>same authoritative view cannot duplicate actions or feedback : effects<br>real selector and remote continuity, composition and takeover resource cleanup : editing, lifecycle, async |
| C1 | ✓ | WebRTC réel : action causale saine puis sous pertes/délai/réordonnancement dans chaque sens sans repli TCP ; silence 25 s détecté en 5977 ms (1 repli(s) TCP autorisé(s)), puis nouvelle négociation UDP et même identité ; {"upstream":224,"downstream":136,"dropped":204,"delayed":103,"reordered":34,"forwarded":156,"directions":{"upstream":{"received":224,"dropped":147,"delayed":49,"reordered":16,"forwarded":77},"downstream":{"received":136,"dropped":57,"delayed":54,"reordered":18,"forwarded":79}}} |
| R2 | – | lance --soak 30 pour l'endurance |
| R4 | ✓ | 4 mesures après GC : DOM, listeners, tas JS, mémoire externe/WASM et RSS Chromium/GPU dans les budgets ; ce cycle court ne remplace pas l'endurance de 30 min |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée

Dernières lignes d'erreur de l'hôte :

```
summary: 2 players online, 2 phones, 1 TV; 600 ticks, longest gap 20 ms; inputs 2 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 23 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 19 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 26 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 30 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 17 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 2 phones, 2 TV; 600 ticks, longest gap 18 ms; inputs 0 accepted, 0 refused
summary: 2 players online, 4 phones, 4 TV; 600 ticks, longest gap 18 ms; inputs 31 accepted, 0 refused
```