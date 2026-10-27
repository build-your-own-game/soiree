# Certification de build-your-own-game/.chantier-modele

Graine 20261027, 0.4 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-.chantier-modele-2026-10-27T21-56-07-822Z

| Item | État | Détail |
|---|---|---|
| C1 | ✓ | téléphone 1 (webkit) a rejoint en 0.5 s depuis l'ouverture<br>téléphone 2 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>téléphone 3 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 4 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>téléphone 5 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 6 (chromium) a rejoint en 0.3 s depuis l'ouverture<br>téléphone 7 (webkit) a rejoint en 0.4 s depuis l'ouverture<br>téléphone 8 (chromium) a rejoint en 0.3 s depuis l'ouverture |
| C7 | ✓ | après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés |
| C8 | ~ | le 9ᵉ joueur est refusé ; ce qu'il lit : « La partie est pleine. » (clair ?) |
| C9 | ✓ | stockage refusé : la page s'ouvre et propose de rejoindre (téléphone 1 (webkit) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage revient, et sa place avec)<br>stockage revenu : téléphone 1 (webkit) garde son jeton et l'appareil 2 après rechargement, à sa place<br>1 clé(s) du stockage corrompue(s) : la page s'ouvre et propose de rejoindre (téléphone 2 (chromium) avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage est remis, et sa place avec)<br>8/8 téléphones à leur place après les essais de stockage |
| R2 | – | lance --soak 30 pour l'endurance |
| R3 | ✓ | salle pleine à 8 |

✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable

Dernières lignes d'erreur de l'hôte :

```
warning: patch `remote-web v0.1.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
warning: patch `remote-web v0.1.0 (/Users/axel2/Projects/build-your-own-game/soiree/remote/crates/web)` was not used in the crate graph
help: Check that the patched package version and available features are compatible
with the dependency requirements. If the patch has a different version from
what is locked in the Cargo.lock file, run `cargo update` to use the new
version. This may also occur with an optional dependency that is not enabled.
```