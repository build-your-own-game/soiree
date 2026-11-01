# Connexion amont Toxiproxy — 29 octobre 2026

Périmètre : infrastructure TCP du harnais. Aucun navigateur, aucun jeu et aucun
réglage réseau/système n'ont été nécessaires à cette reproduction. Les anciens
rapports Punchline et Sidequest restent inchangés et échoués. Ce mécanisme ne
prouve pas la cause de chaque ancien HTTP 503, ni celle de WebRTC intermittent.

## Reproduction et sources

Binaire Homebrew installé : `/opt/homebrew/bin/toxiproxy-server`, version 2.13.0.
Son aide ne fournit pas de délai de connexion amont. Les sources officielles
[v2.13.0 proxy.go](https://github.com/Shopify/toxiproxy/blob/v2.13.0/proxy.go)
et [api.go](https://github.com/Shopify/toxiproxy/blob/v2.13.0/api.go) montrent le
`net.Dial` sans deadline dans l'acceptation, `stop` qui attend cette boucle et
le `http.TimeoutHandler` de 25 s qui produit le HTTP 503 HTML. Le
[changelog courant](https://github.com/Shopify/toxiproxy/blob/main/CHANGELOG.md)
annonce 2.13.0 ; le `proxy.go` courant garde ce Dial.

`baseline.mjs` remplit le backlog d'un serveur TCP loopback qui n'accepte pas,
fait connecter le proxy officiel vers ce serveur, puis le désactive. Le code
ne modifie aucun pare-feu et nettoie ses sockets et ses deux processus possédés.
`baseline.log` mesure POST disable **7 582 ms**, tandis qu'un `/version`
indépendant répond en **0,98 ms**. `baseline-go.log` garde le log du processus.
Une première reproduction avait mesuré 7 601 ms. Ce noyau abandonne ici le SYN
loopback avant les 25 s : le test n'a pas reproduit le HTTP 503 lui-même.

`blocked-go.log`, capturé par SIGQUIT sur un second processus de diagnostic
possédé après 1,1 s, montre simultanément `Proxy.server → net.Dial`,
`Proxy.Update → stop → tomb.Wait` et `freeBlocker → tomb.Wait`. Une API de santé
répondant n'établit donc pas la disponibilité des commandes de faute.

## Correction et régressions

Le moteur des toxics officiel est conservé. Un connecteur possédé par proxy
accepte localement, puis connecte exactement la destination demandée (LAN en
certification) avec délai de 2 s, sans retry ni substitution d'adresse. Les
streams gardent backpressure, octets et FIN. Au plus 128 connexions / 256 sockets
sont possédés ; les connexions, timers et listeners sont fermés à la destruction.
Le délai API de 5 s ne change pas ; HTTP 503 ou JSON invalide reste FixtureFailure.
La disparition de l'adresse LAN reste invalidée par le contrôleur de campagne.

Commande : `node --test soiree/tools/cert/toxiproxy.test.mjs`, depuis le dossier
hôte. `regression.log` : **11 réussis, zéro échec, zéro ignoré**, 4,5 s. La même
fixture de backlog fait expirer l'ancien chemin direct à 250 ms ; le nouveau
`cut` prend **1,81 ms**. Les contrôles vérifient :

- octets binaires/Unicode et fermeture partielle ;
- délai du dial, annulation à l'arrêt, plafond de connexions, nettoyage même si
  DELETE reçoit HTTP 503 et absence de listener après arrêt ;
- destination LAN réelle : local et remote vus par le serveur égalent
  172.20.10.4, sans substitution loopback ;
- `cut`, `reset`, `blackhole`, `deafen`, latence sur les deux directions,
  rétablissement et proxy voisin intact avec le binaire officiel ;
- HTTP/JSON/schéma, délais de corps HTTP et mort du processus : infrastructure.

`sources.sha256` identifie les deux sources de cette régression. Les tests
portables déclarent explicitement leur absence de binaire Toxiproxy ou d'interface
LAN si nécessaire ; aucune de ces absences n'a eu lieu dans ce passage.

## Protocole d'endurance à relancer

Geler sources et binaires, garder le moniteur LAN actif, alimenter le Mac sur
secteur et exécuter les trois campagnes complètes de 30 min successivement,
sans compilations ou autres certifications en parallèle. Conserver toutes les
observations, journaux de l'hôte, mesures et erreurs d'infrastructure. Toute
variation de LAN invalide le passage ; ne pas retenter une commande de faute
échouée pour produire une réussite. Une campagne verte doit attester ses
oracles applicables ; ce test de contrôleur ne remplace pas ces campagnes.
