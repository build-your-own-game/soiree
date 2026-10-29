# Attente précise intégrée et charge réelle de 50 téléphones

Le correctif de production est dans `a002ab2`. Sous macOS, la salle utilise
une minuterie kqueue critique et une notification USER persistante, reliée
à sa file FIFO bornée. La file reçoit toujours les mêmes messages ; le jeu,
les transports et les budgets ne changent pas de contrat public. Les autres
systèmes gardent l'attente native du canal. Ni spin, ni thread de minuterie,
ni réglage global de macOS. Les erreurs de création, d'attente et de réveil
restent explicites. Nix 0.30.1 fournit les wrappers sûrs des appels système.

## Résultat CI demandé

[Le passage Sidequest `40915597190`](https://github.com/build-your-own-game/sidequest/actions/runs/40915597190)
est réussi sur **macOS 15.7.9 arm64** et **Ubuntu 24.04 x86_64**. C'est le
test existant `fifty_phones_move_at_once`, exécuté exactement et réellement,
sans modification de son code : **50 téléphones, 3 s, profil dev, vrais
WebSockets de loopback**. Aucun navigateur, GPU ou Wi-Fi n'est attesté par
ce test. Les autres assertions fonctionnelles de ce test restent actives.

| Machine CI | Écart entre vues p95, budget ≤ 25 ms | Aller-retour p95, budget ≤ 20 ms | Écarts observés |
|---|---:|---:|---:|
| macOS 15 | **17,700 ms** | **1 ms** | 179 |
| Linux | **18,074 ms** | **1 ms** | 179 |

Les maxima observés sont respectivement 18,501 et 18,382 ms pour les vues,
2 ms pour les allers-retours. Ce sont des observations sur ces fenêtres,
pas des bornes universelles.

Le jeu est au commit `fd0c42e`, dont les seuls ajouts depuis `8332a61` sont
le workflow d'intégration et son contrôleur. Le SDK est exactement
`a002ab2bd2f362e476e8512588d4c2901447e2ed`. La source du test a la même
empreinte SHA-256 avant et après :
`199bb9133472712c7b5cb731271a0152b834564fe12a10fff0dc44aa9392fbad`.
Les deux checkouts sont propres avant résolution. Le contrôleur confirme
que `salon-host`, `salon-core` et `remote-protocol` proviennent du SDK choisi,
puis exécute le test avec `--locked` et refuse zéro test ou un test ignoré.

Le Cargo patch est une **fixture d'intégration explicite d'un SDK non
étiqueté**. Le jeu garde sa dépendance publiée `v0.3.0` ; ce passage ne prétend
pas que sa CI ordinaire utilise déjà le correctif, et aucune étiquette n'est
créée ou déplacée. Une publication du commun puis une mise à niveau du jeu
restent nécessaires pour distribuer le correctif aux amis.

## Réveils et contrôles négatifs

[La CI native du commun `40915484874`](https://github.com/build-your-own-game/soiree/actions/runs/40915484874)
est verte sur macOS et Linux : Clippy et tests de l'hôte, dont six tests de
file (bornes, ordre, producteurs concurrents, fenêtre vide→attente, dernier
émetteur et notifications anciennes). La mesure manuelle historique reste
ignorée dans cette suite ; le succès ci-dessus ne lui attribue aucun résultat.

Un mutant local supprimant uniquement la notification native fait échouer
les deux tests de la fenêtre vide→attente, qui attendent alors jusqu'à
l'échéance de 5 s. Le code restauré les repasse immédiatement. La trace
`notify-mutation.log` conserve ces échecs attendus. La mutation n'a jamais
été publiée ni utilisée dans la CI des 50 téléphones.

La [mesure indépendante de la vraie salle `40915485415`](https://github.com/build-your-own-game/soiree/actions/runs/40915485415)
donne un p95 de 20,063 ms via le lanceur de production, 184 réveils dans son
échantillon d'environ 3 s, et 0,022 s CPU pour l'enfant. Son job vert signifie
que le diagnostic s'est exécuté, pas qu'un budget de jeu y a été certifié.

## Échecs et portée conservés

Un passage local sur macOS 26.6.2 échoue : p95 des vues **26,329 ms**,
maximum 67,936 ms, aller-retour p95 6 ms. Le diagnostic du journal donne un
p95 des réveils serveur de 21 ms et un maximum de 54 ms sur la même fenêtre.
Il reste archivé sous `local-failed/`, avec `passed: false`. La machine est
partagée avec d'autres travaux, mais leur charge n'a pas été relevée au moment
exact : cette observation n'est pas expliquée quantitativement ni transformée
en réussite. Le checkout local `53e2fa4` a un arbre Git identique à `a002ab2`.

Les tests locaux complets de salon, les contrôles natifs/WASM et
`soiree check --locked salon .` passent dans le checkout isolé. Cela ne valide
pas les changements non commités des autres sessions.

La [CI générale du commun `40915485114`](https://github.com/build-your-own-game/soiree/actions/runs/40915485114)
conserve deux échecs dans son job navigateur : Chromium ne demande pas la
ressource bloquée dans le contrôle de navigation, et l'observateur de rendu
ne récolte pas plus de deux images dans sa fixture. Ce job exécute les tests
Node autonomes, sans notre serveur Rust. Il n'est pas présenté comme réussi.

`summary.json` est une vue dérivée, pas une attestation importable ni une
certification globale. Les sous-dossiers conservent les reçus, commandes,
sorties, versions Rust et configurations de fixture. Les sorties standard
sont encodées en JSON réversible avec leur empreinte ; le lock commun aux
trois passages est conservé une fois sous `resolved-locks/`, nommé par son
SHA-256. Les extraits
de métadonnées SDK sont explicitement partiels et portent l'empreinte du
document source complet, disponible dans les artefacts du passage GitHub.
