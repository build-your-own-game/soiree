# Pourquoi la CI voyait des pas de 60 à 80 ms

Le mécanisme principal est établi : le regroupement des minuteries de macOS
(*timer coalescing*) retarde les attentes ordinaires de notre serveur sur les
runners hébergés. Ce n'est pas un coût de calcul du jeu, ni une dégradation
Linux observée. Ce n'est pas non plus une raison de réserver d'emblée la mesure
au Mac de la soirée : une autre minuterie fonctionne nettement mieux sur le
même environnement GitHub.

## Comparaison portable sans jeu

[Campagne `40910644287`](https://github.com/build-your-own-game/soiree/actions/runs/40910644287),
commit `37d23b4`. L'exécutable autonome inclut textuellement le vrai
`room::wait`, avec ses 2 ms de fin d'attente, et compare des primitives
indépendantes. Aucun calcul de jeu, réseau, stockage ou affichage dans la
fenêtre chronométrée. Même Rust 1.99, mêmes échéances 60 Hz arrondies à la
milliseconde et même saut des slots manqués. Deux passages fixés de 3 s par
méthode, ordre inversé au second ; aucun réessai conditionné par le résultat.

| Machine | Vrai `room::wait`, intervalle p95 des deux passages |
|---|---:|
| Mac local, macOS 26.6.2 | 18,8 / 17,6 ms |
| GitHub Ubuntu 24.04 x86_64 | 17,0 / 17,0 ms |
| GitHub macOS 15.7.9 arm64 | 61,4 / 66,2 ms |
| GitHub macOS 26.6.2 arm64 | 70,2 / 65,0 ms |

`sleep`, `park_timeout`, `recv_timeout` et `Condvar` présentent également
des p95 élevés sur les deux macOS hébergés. Cela ne dépend pas uniquement
de la salle ou du canal Rust. Le même problème se reproduit ensuite en C.
La différence existe aussi entre le Mac local et le runner à **version macOS
identique** ; elle ne s'explique donc pas seulement par macOS 15 contre 26.

Les témoins qui restent actifs atteignent généralement 17 ms mais utilisent
presque un cœur entier : ils servent à distinguer l'attente du calcul, pas
à proposer un correctif. Un passage macOS 15 montre aussi une interruption
de 80,9 ms dans le témoin qui ne dort jamais. Les minuteurs ne sont donc pas
une explication de toute interruption possible, ni un moyen de garantir une
borne absolue sur une machine généraliste.

## Intervention causale sur macOS 15

[Job macOS 15 de `40911621742`](https://github.com/build-your-own-game/soiree/actions/runs/40911621742),
commit `b4e09b4`. Même exécutable et même machine. Seule intervention :
`kern.timer.coalescing_enabled`, ordre **activé, désactivé, désactivé, activé**.
Trois mécanismes par phase, ordre inversé sur la seconde moitié.

| Réglage du noyau | `room::wait` p95 | `recv_timeout` p95 | `sleep` p95 |
|---|---:|---:|---:|
| Activé, avant | 65,9 ms | 61,0 ms | 67,7 ms |
| Désactivé, premier passage | 17,6 ms | 20,3 ms | 19,2 ms |
| Désactivé, second passage | 17,5 ms | 20,5 ms | 19,1 ms |
| Réactivé, après | 71,8 ms | 74,9 ms | 71,4 ms |

La remontée après restauration écarte une simple amélioration avec le temps.
Les 180 échéances sont observées dans chaque échantillon sans regroupement,
contre 86 à 122 avec regroupement selon la primitive et le passage.
Le réglage initial `1` a été restauré et relu. L'intervention est confinée à
une VM GitHub jetable ; le script refuse ce mode sur le Mac de l'hôte.
**Désactiver globalement cette fonction n'est pas un correctif produit.**
Le job macOS 15 est terminé avec succès. Le passage macOS 26 encore en file
a été annulé après obtention du contre-test causal sur 15 et des essais
natifs sur les deux versions ; on n'attribue aucun résultat à ce job non exécuté.

## Alternative locale à la minuterie

[Job macOS 26 de `40911083380`](https://github.com/build-your-own-game/soiree/actions/runs/40911083380),
commit `e7fb615`. Programme C indépendant, même thread de QoS DEFAULT
(valeur lue 21), mêmes échéances et même compteur monotone. Deux passages
de 3 s par mécanisme, ordre inversé. Entre les deux variantes kqueue,
seul le drapeau `NOTE_CRITICAL` diffère.

| Minuterie sur le même runner | Intervalle p95, deux passages |
|---|---:|
| `nanosleep` | 63,2 / 70,0 ms |
| `dispatch_semaphore_wait` | 75,2 / 74,1 ms |
| `mach_wait_until` | 72,0 / 66,1 ms |
| kqueue ordinaire | 71,0 / 76,8 ms |
| kqueue `NOTE_CRITICAL` | **19,1 / 19,0 ms** |

La variante critique observe 180 échéances et utilise 0,010 puis 0,004 s
CPU par échantillon d'environ 3 s. Elle n'a pas besoin de monopoliser un cœur
ni de changer les réglages globaux. Le Mac local atteint 17,0 ms avec cette
même variante, contre environ 18–20 ms avec ses attentes ordinaires.

Le job `macos-15` de cette même campagne a reçu l'image **15.8.2**, contre
15.7.9 pour le contre-test du noyau ci-dessus : on conserve cette différence.
Il confirme l'alternative sur la famille macOS 15 : kqueue ordinaire
71,5 / 76,1 ms p95 ; `NOTE_CRITICAL` **17,2 / 17,1 ms** p95, maxima
21,3 / 18,9 ms, avec 0,005 / 0,004 s CPU.

Le code Apple donne le lien entre mécanisme et expérience :
[`filt_timerarm`](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_event.c)
convertit `NOTE_CRITICAL` en urgence de minuterie critique ;
[`timer_compute_leeway` / `timer_call_slop`](https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/kern/timer_call.c)
déterminent la marge de regroupement selon l'urgence et la politique effective.
Rust 1.99 fait attendre son
[`park_timeout` macOS sur un sémaphore dispatch](https://github.com/rust-lang/rust/blob/1.99.0/library/std/src/sys/sync/thread_parking/darwin.rs),
utilisé par le contexte de réception des canaux. Une demande de QoS réussie
ne prouve pas la politique effective ni une précision temporelle.

## Conséquence pour le produit et limites

La couche commune suppose qu'une attente ordinaire suivie d'une fin active
de 2 ms suffit à tenir la cadence. Le retour tardif de l'attente invalide cette
hypothèse. Il faut une attente sensible aux échéances, qui se réveille aussi
immédiatement sur les entrées réseau et l'arrêt. Une intégration kqueue doit
préserver ces propriétés, la file bornée et la gestion des erreurs ; les
échantillons autonomes ne vérifient pas cette intégration.

On garde les budgets du jeu. On ne peut pas attribuer chaque échec possible
à GitHub, exiger une logique de jeu plus restrictive sur cette seule base,
ou déclarer la charge de 50 téléphones corrigée : le serveur de production
n'est pas modifié par cette enquête. La campagne portable concerne une attente
sans jeu ; elle ne certifie pas tout Sidequest sous Linux. La raison précise
pour laquelle la politique effective est plus permissive aux retards sur les
runners que sur le Mac local n'est pas établie par ces lectures de politiques.
Le regroupement lui-même, son effet causal et une alternative sont établis.

Les reçus JSON de ce dossier conservent les entrées et les observations brutes
avec l'empreinte de leur rapport source. Ce sont des diagnostics, jamais des
attestations importables ou une certification globale. Les workflows sont
manuels et n'allongent pas la CI ordinaire.
