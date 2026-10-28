# Diagnostic de la CI — 28 octobre 2026

La [validation complète du commun à `8a20dbf`](https://github.com/build-your-own-game/soiree/actions/runs/40676480818)
est réussie : outil et contrôles négatifs, compilation iPhone à froid, SDK
Rust/WASM, ABI Swift, tests navigateur, modèle neuf et noyau Verus. Ce succès
porte sur ses contrôles et ses entrées ; ce n’est pas un certificat global
des jeux ou des appareils physiques.

## Corrections établies

Le projet Xcode ne déclarait pas les fichiers générés par sa phase Rust.
Un clone sans `Generated` ni `target` reproduisait l’erreur ; les sorties
déclarées rétablissent l’ordre de compilation. La vraie CI confirme le succès.

Le contrôle négatif de navigation confondait historique des requêtes et
connexions encore ouvertes après un timeout. Il conserve désormais
l’historique par moteur. Tous les tests du harnais restent exécutés ; leurs
fixtures sont sérialisées dans un job indépendant des compilations du SDK.
Ce job et les intégrations suivantes sont verts dans le passage ci-dessus.

Le scénario de reconnexion Sidequest lisait parfois la TV avant sa nouvelle
projection, bien que le téléphone ait déjà retrouvé son personnage. La
barrière attend maintenant une arrivée TV postérieure au retour téléphone.
Deux régressions ordonnées ont échoué avant correction puis passé, y compris
avec le tampon d’arrivées plein. Ce correctif ne change ni la physique ni
les budgets de temps réel.

## Ce qui reste à distinguer

La [mesure de charge Sidequest](https://github.com/build-your-own-game/sidequest/actions/runs/40674586373)
échoue sur le runner : p95 des vues 86,7 ms et des réveils de l’hôte 85 ms
sur la même fenêtre, malgré un aller-retour p95 de 1 ms. Ce n’est donc pas
une perte de vues démontrée dans l’observateur. Charge de 50 connexions,
durée de 3 s et seuils de 25/20 ms sont conservés. Cette mesure ne démontre
pas, à elle seule, l’unicité des 50 personnages.

Le [contrôle Punchline](https://github.com/build-your-own-game/punchline/actions/runs/40676211102)
observe les dimensions demandées mais aucun événement `resize` dans sa
fenêtre d’une seconde. Une frontière de rendu entre rotations est désormais
explicite ; elle n’a pas résolu cet échec CI. Le délai est un garde-fou du
contrôleur, distinct des budgets produit P1/P2. Les diagnostics supplémentaires
ne doivent jamais transformer une arrivée tardive en réussite rétroactive.

## Expériences de planification indépendantes

Même sans réseau, persistance ni calcul de jeu, la vraie boucle de salle dépasse
les délais sur la machine hébergée. Échantillons prédéfinis de 3 s, même
exécutable dans chaque campagne :

| Campagne CI | Cas | Intervalle p95 |
|---|---|---:|
| [première](https://github.com/build-your-own-game/soiree/actions/runs/40678772945) | lanceur de production | 61,5 ms |
| [QoS et threads](https://github.com/build-your-own-game/soiree/actions/runs/40681434194) | lanceur de production | 63,1 ms |
| même campagne | demandes USER_INITIATED sur les threads | 67,1 ms |
| même campagne | rôle application et demandes sur les threads | 56,5 ms |
| [précision des timers](https://github.com/build-your-own-game/soiree/actions/runs/40683429526) | lanceur de production | 112,6 ms |
| même campagne | activité LatencyCritical réellement conservée | 99,4 ms |
| même campagne | tier de latence zéro | 101,9 ms |

Les appels et tokens observés prouvent leur exécution, pas une précision
temporelle effective. Aucun de ces contre-tests ne justifie un correctif
runtime ou un relâchement de seuil. Les résultats entre campagnes ne sont
pas des répétitions jusqu’à réussite.

Les fichiers JSON de ce dossier sont des **résumés dérivés**, avec sorties
de mesure, politiques demandées, commandes et empreintes des rapports
sources. Ils ne sont pas des attestations importables. Chaque passage conserve
ses artefacts complets dans GitHub ; le statut vert du job de diagnostic
signifie que l’expérience a été exécutée, pas que la cadence est satisfaite.

La machine de référence et le raccord entre test fonctionnel portable et
mesure de performance restent à trancher avec l’hôte. Une mesure hors de
son contexte reste non attestée ; elle ne devient ni satisfaite ni non
applicable par une simple justification.
