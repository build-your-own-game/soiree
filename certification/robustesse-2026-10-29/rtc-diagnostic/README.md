# Diagnostic RTC du 29 octobre 2026

La cause du premier échec historique 7222 reste inconnue. Deux défauts de
politique de délai ont ensuite été reproduits et corrigés, avec leurs
contrôles négatifs (voir la suite de l’investigation en fin de document).
Le premier passage normal Punchline a échoué,
la reproduction instrumentée à trois téléphones a réussi. Ces observations
restent distinctes ; la seconde n'annule pas l'intermittence de la première.
Ces diagnostics ponctuels ne constituent ni un test d'endurance, ni une
certification du plan complet, ni une mesure de latence représentative.

Le premier échec normal est conservé dans `../stable-punchline-first/` (graine
7222, LAN `172.20.10.4`). Le relais n'a reçu aucun datagramme dans les deux sens,
aucune erreur de relais/page n'a été remontée, le client est revenu au WebSocket
après son délai et le lien str0m a expiré sans ouvrir son DataChannel. Le SDP et
les promesses RTC natifs n'étaient pas instrumentés dans cette observation :
elle ne permet pas d'attribuer l'échec à WebKit, au produit ou au contrôleur.

## Comparaisons isolées

Tous les sockets HTTP/UDP, hôtes et navigateurs sont créés et fermés par les
scripts conservés ici. Ils utilisent les binaires Playwright locaux et le
binaire Punchline existant, sans modification du produit. Les chemins absolus
dans les scripts identifient cette machine ; les JSON conservent les SDP,
états ICE et compteurs effectivement observés.

| Diagnostic | Observation | Limite |
|---|---|---|
| `rtc-capability-20261029.*`, page HTTP loopback | Chromium : 33 STUN/2 s ; WebKit desktop : 38 ; WebKit mobile : 39 vers un socket UDP possédé sur `172.20.10.4`. Les deux peer-pairs WebKit ouvrent leur DataChannel et reçoivent le message attendu. | Socket STUN récepteur passif : ce premier contrôle prouve l'émission UDP, pas la négociation str0m. |
| `rtc-capability-lan-20261029.*`, même page servie sur l'origine HTTP LAN | Même résultat : 33/38/39 STUN, et les deux peer-pairs WebKit fonctionnent. | Écarte une interdiction WebKit générale liée à cette origine HTTP LAN ; ne reproduit pas tout le produit. |
| `rtc-real-relay-20261029.*` | Vrai `/salon/offer` Punchline/str0m ; WebKit mobile ouvre en direct, à travers le rewrite actuel, et avec headers corrigés. Chromium ouvre dans les deux variantes réécrites. | Le mode Chromium direct de cette page synthétique échoue avec `Failed to fetch` avant ICE ; cause non diagnostiquée. Ce contrôle n'est pas un succès uniforme des six cas. |
| `rtc-real-relay-observed-20261029.*` | Même résultat en ajoutant exactement `observeRtc` de `udp.mjs` et le profil IPHONE du harnais. WebKit envoie/reçoit au relais et ouvre son canal. | L'observateur n'empêche pas systématiquement la négociation ; aucune preuve sur la cause intermittente. Le contrôle Chromium direct reste en échec. |
| `rtc-real-relay-background-20261029.*` | Le contrôle lancé avant la demande d'arrêt termine avec succès, avec deux pages WebKit concurrentes et avec/sans `bringToFront`. | Les deux cas rapportent `visibilityState=visible` et `hasFocus=true` : ils ne démontrent pas un vrai passage en arrière-plan. Aucun changement de foreground n'a été appliqué au produit ou au harnais. |

Le diagnostic réel constate un défaut indépendant de cohérence HTTP dans
l'ancien rewrite : `content-length: 764` est conservé alors que le JSON réécrit
fait 766 octets. Le code primaire Playwright local
`tools/cert/node_modules/playwright-core/lib/coreBundle.js`, méthode
`Route._innerFulfill`, reprend les headers de `response` et ne recalcule la
longueur que si elle est absente. Les variantes ancienne et corrigée ouvrent
toutes deux le DataChannel WebKit ici : ce défaut n'est donc pas une cause
établie de l'échec initial. Sa correction et ses tests sont suivis séparément.

## Reproduction du vrai scénario

`cert-rtc-instrumented-20261029.mjs` est la copie exacte du harnais exécutée,
avec imports vers le code commun réel, observation des promesses RTC/états
ICE/fetch par `browser-rtc-instrumented-20261029.mjs`, et conservation du SDP.
Elle conserve l'ancien `route.fulfill({ response, json })`, n'active pas la
page avec `bringToFront` et ne change pas de moteur.

Commande exécutée le 29 octobre à 17:57 UTC :

```sh
SOIREE_OBSERVATIONS_PATH=/tmp/rtc-punchline-instrumented-observations.json \
node /tmp/cert-rtc-instrumented-20261029.mjs \
  /Users/axel2/Projects/build-your-own-game/games/punchline \
  --only rtc --phones 3 --seed 7223
```

Résultat observé : C1 réussi sur le premier téléphone WebKit, action causale
saine puis sous pertes/délai/réordonnancement effectifs dans chaque sens ;
254 datagrammes montant et 133 descendant. Silence UDP de 25 s constaté par
l'hôte après 5 789 ms ; un repli TCP autorisé pendant ce long silence ; nouvelle
négociation UDP et même identité. Les rapports `rtc-punchline-instrumented-*`
conservent ces résultats et la mesure causale. Aucun incident d'infrastructure
n'est remonté par cette exécution. L'instrumentation détaillée des erreurs RTC
n'a pas de contenu d'échec puisque cette reproduction a réussi.

Les scripts n'ont laissé aucun hôte, serveur HTTP/UDP ou navigateur leur
appartenant en cours d'exécution. La validation normale finale appartient au
passage ultérieur de l'hôte, avec sources figées et headers corrigés ; elle ne
doit pas remplacer l'observation initiale conservée ici.

## Correction causale et régressions — suite de l'investigation

Les nouveaux diagnostics, scripts et rapports bruts sont dans
`negotiation-raw.tar.gz`, avec empreintes dans `negotiation-raw-sha256.json`.
`negotiation-summary.json` donne les résultats de chaque série prédéfinie :
10 ouvertures chaudes, 20 ouvertures alternant ancien/correct framing HTTP,
puis 5 démarrages froids de processus. Toutes ont ouvert RTC. Elles ne prouvent
pas l'absence d'intermittence ; aucun échec n'a été retiré de ces séries.

L'échec Chromium sur la page synthétique a été expliqué : le navigateur dit
explicitement que son origine n'est pas classée dans l'espace réseau local et
refuse le fetch en CORS/private-address-space. La page venait de page.route,
pas du serveur HTTP. Les vrais téléphones passent par leur vraie page HTTP.

Une cause de classe est ensuite démontrée par A/B contrôlé, conservé dans
`rtc-deadline-20261029.json` : réponse SDP retardée 2 s → RTC s'ouvre ; retard
4 s → l'ancien SDK ferme le PC à environ 3 s avant de lire la réponse et passe
en WebSocket ; même retard 4 s avec uniquement ce callback de coupure neutralisé
**dans le diagnostic** → un seul PC ouvre RTC vers 4,3 s, sans WebSocket ; sans
retard → ouverture normale. Ce témoin ne devient pas un test vert du produit.
Il montre une négociation viable détruite par la politique de délai. Il ne
prouve toujours pas que l'échec historique 7222 provenait d'un retard SDP.

La première régression avec fenêtre SDK 15 s a trouvé un autre défaut réel :
le noyau utilisait encore Timing.connect=5 s et détruisait sans fin le transport
avant son repli. Cinq pairs/28 s, zéro WebSocket, SDP en progression : voir
`rtc-negotiation-production-core5s-failure-20261029.json`. Cette observation est
conservée, pas remplacée par le passage suivant.

La correction partage la fenêtre RTC de 15 s avec l'hôte et déclare au noyau
le budget du transport choisi. Le propriétaire RTC garde sa propre échéance
absolue ; le noyau réserve cette fenêtre plus les 5 s de repli TCP, puis le
repli reçoit ses 5 s à partir de sa vraie création. Les délais C1 de connexion
en 30 s et P1/P2 restent inchangés. Les progrès SDP/ICE ne réarment aucun délai.
Les tests purs couvrent la borne, le retour au défaut pour l'essai suivant et
l'impossibilité de repousser le handshake d'un lien déjà ouvert.

Le passage final du vrai hôte et du vrai WASM Punchline exécute le helper commun
`tools/cert/rtc-negotiation.mjs` sans neutralisation du callback : **7/7 cas
WebKit et 7/7 Chromium passent**, même identité. SDP 4 s s'ouvre vers 4,15 s ;
les rejets SDP/DTLS natifs reviennent en TCP en 71–126 ms ; la panne permanente
après progression et la réponse SDP à 16 s ferment le PC entre 14 999,6 et
15 000,5 ms ; la réponse tardive ne rouvre/évince pas WebSocket. L'échec forcé
d'enregistrement du timer libère le pair avant toute négociation asynchrone.
`rtc-negotiation-production-20261029.json` conserve tous ces cas et rapporte
`cleanup: []`. Les ressources possédées par ce passage sont fermées.

Sources primaires :
[RFC 6347 §4.2.4.1](https://www.rfc-editor.org/rfc/rfc6347.html#section-4.2.4.1)
pour les retransmissions DTLS (1 s puis doublement),
[RFC 8838 §16](https://www.rfc-editor.org/rfc/rfc8838.html#section-16) pour les
candidats peer-reflexive permis avec ICE-lite. Le choix d'une borne absolue
15 s est notre politique d'application, déjà présente côté hôte ; ces RFC ne
prescrivent pas ce nombre.
