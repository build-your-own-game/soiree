# Observations du harnais de soiree-model/ci-model

Graine 6839, 0.1 min. Captures : /var/folders/bj/9vhl4y7s36d0brk50ms41z440000gp/T/cert-ci-model-2026-10-28T04-52-07-235Z

| Item | État | Détail |
|---|---|---|
| U1 | ✓ | ✓ Le bouton reste le même après une mise à jour (webkit) : lifecycle<br>✓ Le bouton reste le même après une mise à jour (chromium) : lifecycle<br>– editing : Le modèle ne propose aucun champ de saisie. Ajouter un parcours editing dès qu’un éditeur est ajouté.<br>– effects : Le modèle projette des compteurs ; aucun son ni vibration déclenché par une vue.<br>– async : Aucun import, export ou chargement asynchrone propre au jeu ; le transport commun est exercé séparément. |
| C10 | ✓ | webkit : après une vraie panique WASM, même appareil 2, même jeton et même place<br>chromium : après une vraie panique WASM, même appareil 3, même jeton et même place |
| R2 | – | lance --soak 30 pour l'endurance |

✓ observé · ✗ échec · ~ à juger par un humain, ou vérifié en partie · – non exécuté ou exclusion explicitée
