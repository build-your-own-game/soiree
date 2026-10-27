# Complément C9 — 27 octobre 2026

Le rapport complet `../rapport.md` est conservé avec son JSON et ses mesures. Il a été exécuté avec le harnais SHA-256 `56b7e391dd9a2465cce1d159364bcaec4095ec36921e7ecdfc062708ee7bca6c`. Le présent complément utilise le harnais corrigé `1cf0847f757a720ac7dba1c4f9f82fc40bc3fee8c52a158a3793308f3a512718` et les mêmes sources de jeu, empreinte `3dcd08ecfce9b590d2562f8722f596b93176a0320211cc174ffbdc15a46cc12a`.

Commande : `node cert.mjs .chantier-modele --only join,restart,storage --seed 20261027` — **code de sortie 0**, quatre téléphones WebKit et quatre Chromium. Seuls ces trois scénarios sont rejoués ; les budgets et les autres résultats du passage complet ne sont pas remesurés ici. `performance.json` contient donc les arrivées de ce passage, sans échantillon de cadence ni sonde de latence.

Le correctif concerne seulement C9 : des documents frais attendent leur nouveau Welcome et leur écran courant avant de décider si la place existe ; le jeton d’un appareil entré sans stockage est restauré avant le démarrage du document suivant, dans la clé observée initialement. Le jeton, l’appareil après un nouveau Welcome et la place sont contrôlés après rechargement. Une réparation impossible échoue C9/H1 ; le résultat final exige huit téléphones sur huit à leur place.

Le modèle exerce une place déjà occupée : après l’essai sans stockage, l’identité originale et sa place reviennent au rechargement.

Les captures `storage-phone-1.png` et `storage-phone-2.png`, le rapport, la sortie brute et les empreintes de `metadata.json` permettent de contrôler ce complément. La régression réelle des deux moteurs et toute la suite Node passent (17 tests).
