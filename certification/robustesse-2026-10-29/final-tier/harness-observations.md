# Observations du harnais de games/tier-list

Graine 7230, 1.9 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-tier-list-2026-10-29T18-02-24-373Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | real WASM async exports, independent imports, editing and resource release : async, editing, lifecycle, effects<br>a row keeps its identity, focus and selection across another edit (webkit) : editing<br>a row keeps its identity, focus and selection across another edit (chromium) : editing<br>view cycles neither accumulate handlers nor duplicate edits (webkit) : lifecycle, effects<br>view cycles neither accumulate handlers nor duplicate edits (chromium) : lifecycle, effects |
| C9 | ✓ | IndexedDB déclaré remplacé par 5 contenus abîmés, page rechargée à chaque fois<br>stockage refusé : la page s'ouvre |
| R2 | – | lance --soak 30 pour l'endurance |
| R4 | ✓ | 4 mesures après GC : DOM, listeners, tas JS, mémoire externe/WASM et RSS Chromium/GPU dans les budgets ; ce cycle court ne remplace pas l'endurance de 30 min |

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