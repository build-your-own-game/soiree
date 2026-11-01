# Nettoyage des groupes de processus — 29 octobre 2026

`tests.log` conserve six tests satisfaits de `tools/cert/processes.test.mjs`.
`sources.json` identifie exactement les deux fichiers contrôlés. Cette passe
teste le helper ; son intégration dans le contrôleur et les campagnes complètes
sont exécutées séparément par l'agent principal.

`killOwnedGroup(child, {timeoutMs: 4000})` exige le `ChildProcess` possédé et
utilise uniquement son PID original comme identifiant du groupe détaché. Il
n'utilise aucun port ni PID découvert par `lsof`. Il envoie SIGKILL puis ne
réussit qu'après observation de la terminaison de l'enfant et de l'absence de
processus vivants dans ce groupe. Les zombies sont identifiés séparément dans
le reçu. Les appels répétés partagent la même opération. Refus de signaler,
table de processus illisible et délai dépassé sont des `FixtureFailure`.
ESRCH seul ne démontre pas que le groupe est fermé.

Le test réel observe parent et sous-enfant dans le même PGID, leurs deux
sockets ouvertes avant et fermées après, et un processus voisin encore vivant.
Les négatifs injectent un signal sans effet, une lecture PS bloquée, EPERM,
ESRCH avec groupe encore vivant, un enfant non détaché, un PID réutilisé et des
cibles invalides. Un enfant déjà terminé et un groupe contenant seulement un
zombie n'entraînent aucun nouveau signal.

La table utilise les champs `pid`, `pgid` et `stat` de PS communs à macOS et
Linux ; la passe a été exécutée sur macOS. Windows produit une non-attestation
explicite. Le périmètre est le groupe POSIX possédé : un descendant qui crée
volontairement une autre session ne lui appartient plus. Le helper refuse un
leader vivant réutilisant le PID d'un ChildProcess déjà terminé ; il ne
prétend pas remplacer par une preuve formelle l'identité des processus du
noyau entre deux observations.

Références primaires : [processus détachés Node](https://nodejs.org/api/child_process.html#optionsdetached)
et [signaux Node](https://nodejs.org/api/process.html#processkillpid-signal).
