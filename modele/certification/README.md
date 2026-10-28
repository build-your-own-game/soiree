# Résultats de vérification

Les fichiers produits sont des observations datées liées à leurs entrées :

- `check-fast.json` / `check-fast.md` : contrôles statiques demandés ;
- `check.json` / `check.md` : contrôles du code et exécution des tests ;
- `verification.json` / `verification.md` : périmètre du harnais et évaluation ;
- `harness-observations.json` / `harness-observations.md` : observations brutes ;
- `harness-performance.json` : mesures du passage navigateur.

Les anciens `rapport.json`, `rapport.md` et `performance.json`, lorsqu’ils
existent, restent des pièces historiques. Ils ne sont pas réétiquetés comme
preuves du code présent. Les passages séparés ne sont pas fusionnés en une
certification globale. Un résultat non attesté ou à examiner reste explicite.

Les engagements viennent de `verification.toml` ; `CERTIFICATION.md` en est la
vue. Ne modifie pas les résultats à la main : relance le contrôle concerné.
