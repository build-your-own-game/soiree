# Observations du harnais de games/tier-list

Graine 6839, 2.4 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-tier-list-2026-10-28T04-52-58-227Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | real WASM async exports, independent imports, editing and resource release : async, editing, lifecycle, effects<br>a row keeps its identity, focus and selection across another edit (webkit) : editing<br>a row keeps its identity, focus and selection across another edit (chromium) : editing<br>view cycles neither accumulate handlers nor duplicate edits (webkit) : lifecycle, effects<br>view cycles neither accumulate handlers nor duplicate edits (chromium) : lifecycle, effects |
| P1 | ✓ | avant partie : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms ; téléphones 0 WebKit + 0 Chromium<br>en jeu sous entrées : 181 images, 60.0 images/s, p50 16.7 ms, p95 16.7 ms, maximum 16.7 ms, 6 gestes pendant l'échantillon |
| P2 | ✓ | sonde 1 (tv, chromium) : entrée → rendu 35.9 ms, budget 50 ms |
| C10 | – | pas de téléphones : le hook solo est exercé par ses tests |
| C1 | – | pas de téléphones (×2) |
| C2 | – | pas de téléphones |
| C3 | – | pas de téléphones<br>il faut au moins 5 téléphones pour ce scénario |
| C4 | – | pas de téléphones |
| C5 | – | pas de téléphones |
| C7 | – | pas de connexion permanente avec l'hôte |
| C9 | ✓ | 8 clé(s) remplacée(s) par 5 contenus abîmés, page rechargée à chaque fois<br>stockage refusé : la page s'ouvre |
| E1 | – | pas de WebSocket déclaré (`socket`) |
| E6 | ✓ | 120 s de gestes au hasard sur la page |
| T3 | – | page d'ordinateur, pas un écran de TV |
| D2 | – | pas de touche « advance » : la progression se juge au salon |
| R1 | – | pas de téléphones |
| R2 | – | lance --soak 30 pour l'endurance |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée

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