# Sept corrections de robustesse — 29 octobre 2026

Ce dossier rassemble la portée des changements locaux et de leur vérification.
Il ne constitue pas une certification globale verte ni une publication du SDK.
Les jeux gardent leur étiquette précédente et compilent ici contre le commun
local. Publier le commun avant tout jeu qui utilise ces nouvelles API.

## Changements

| Point | Protection construite | Vérification pertinente |
|---|---|---|
| 1. Onglets concurrents | Tier List : snapshot IndexedDB, comparaison et écriture dans la même transaction ; conflit visible, aucune suppression des contenus de l'autre onglet. Ancien writer fragmenté supprimé, import unique de l'ancien format. | Deux onglets séquentiels et simultanés ; abort après put ; rechargement ; corruption et reprise. |
| 2. Stockage indisponible | État non sauvegardé explicite et persistant, y compris refus, quota et conflit ; échec conserve le dernier snapshot. | 26 cas navigateur WebKit/Chromium de Tier List, dont refus et quota/reprise. |
| 3. Son | Module salon_web::audio indépendant du réseau : identité des cues, expiration 250 ms, aucun backlog, quatre voix maximum, graphes possédés, geste réel, reprise, mute/volume. TV Punchline et Sidequest intégrées. | Signal de production rendu hors ligne, énergie/crête/silence, vrais gestes, suspension/reprise contrôlée du contexte, doublons, libération, construction en échec, préférences, clavier ; Chromium/WebKit. Interruptions physiques OS non attestées. |
| 4. GPU | Sidequest : device/context révocable, reconstruction de toutes les ressources, génération contre acquisitions périmées, trois reprises automatiques, watchdog 10 s et reprise manuelle DOM. | Pertes induites par GPUDevice.destroy() et WEBGL_lose_context Chromium/WebKit, comparaison des images, échecs, contrôle négatif ; dix captures de référence inchangées. Reset physique du pilote non reproduit. |
| 5. Vrai UDP | Relais sous DTLS/SCTP ; action causalement reliée à la TV, pertes, délai, réordonnancement, silence court puis 25 s, expiration hôte, repli TCP identifié et renégociation avec même appareil. | Campagnes réelles Punchline/Sidequest ; oracle interdit ancien paquet vivant + ACK frais ; test du véritable ordre des datagrammes. |
| 6. Connexions lentes | Admission avant création des threads, quota dérivé du nombre d'appareils et réserve locale, échéances absolues HTTP/WS, reader/writer terminent ensemble, handshake natif borné même sous junk continu. | 34 tests hôte réussis, un test de cadence volontairement ignoré ; vrais sockets lents, écritures partielles, Hello valide, quota de 72 appareils avec app et secours. |
| 7. Rétention navigateur | DOM/listeners, tas JS, mémoires WASM réelles par références faibles, RSS de Chromium et de ses processus GPU. Frontières après GC, budgets et contrôles négatifs. | Tier : 135 cycles réels import/export/suppression, DOM175 et listeners44 constants, WASM4063232 octets constants ; endurance32,4 min dans les budgets mémoire. |

## Résultats et limites conservés

- [Tier : ressources courtes](../../../games/tier-list/certification/browser-resources.json)
  et [endurance](../../../games/tier-list/certification/endurance-2026-10-29/README.md).
  Mémoire réévaluée après cinq minutes de chauffe : aucun dépassement. Le passage
  global reste en échec : un timeout de mesure et une image à 33,3 ms pour un
  budget de 33 ms. Aucun seuil relevé ni échec effacé.
- [Punchline : preuves courantes et essais interrompus](../../../games/punchline/certification/endurance-2026-10-29/README.md).
  UI finale positive sur les deux moteurs, explicitement en loopback ; galerie
  17 écrans × 5 formats, disposition live aux six formats, projection des huit
  phases. L'ancienne endurance garde ses premières pertes WebKit inexpliquées,
  changements d'adresse LAN et erreurs du contrôleur. Elle n'atteste pas R2.
- [Sidequest : contrat GPU et sources](../../../games/sidequest/crates/screen/GPU.md).
  Le [passage ciblé stable](stable-sidequest/verification.json), graine 7221,
  satisfait son périmètre RTC/UI/ressources sans erreur d'empreinte. Il observe
  notamment l'action causale sous défauts UDP effectifs dans chaque sens, la
  détection du silence et le retour du même appareil. Les anciennes empreintes
  modifiées par du travail concurrent restent non attestées. Une endurance a
  été arrêtée après panne du contrôleur, sans succès de trente minutes ; ses
  [preuves sont conservées](../../../games/sidequest/certification/endurance-2026-10-29/README.md).
- [Serveur : tests finaux](soiree-host-final-tests.log) et
  [Clippy](soiree-host-final-clippy.log). Ils incluent le changement de file native
  du chat parallèle, qui est conservé ; il ne fait pas partie de nos corrections.
- Les contrôles finaux `soiree check --locked` des trois jeux passent sur leur
  périmètre demandé (Tier26/53, Punchline15/58, Sidequest27/70).
  Ils précèdent les dernières corrections JavaScript du harnais HTTP/RTC,
  vérifiées séparément par leurs tests et les passages navigateur ultérieurs.
  Ils ne remplacent pas les contrôles navigateur ou les observations sur appareils.
- L'outil a passé ses 96 tests unitaires, huit contrôles négatifs et quatre tests
  d'observation. Le harnais a passé [57 tests](harness-final-tests.log), dont les régressions du contrôleur,
  de ressources, de progression et de transport. Les campagnes ciblées ne
  deviennent pas une garantie générale par addition de ces nombres.

Les budgets mémoire ne mesurent pas la VRAM et les compteurs CDP ne sont pas
attribués à WebKit. Les cycles de vie WebKit sont exercés par ses parcours réels.
La sortie audio physique de la TV/iPhone, le vrai verrouillage iOS et le Wi-Fi
réel restent des observations sur appareils (S1–S10).

## Reprises après la fin du travail parallèle

Le [premier passage stable Punchline](stable-punchline-first/verification.json),
graine 7222, passe les parcours UI/audio, disposition, coupure, gel, recharge,
entrées hostiles et ressources. Son scénario RTC sur WebKit ne reçoit aucun
datagramme au relais et échoue. Le [diagnostic indépendant](rtc-diagnostic/README.md)
et la reproduction instrumentée suivante réussissent, avec le même moteur et
trois téléphones. Aucune cause produit ou contrôleur n'est démontrée pour
l'intermittence initiale ; la réussite suivante ne l'efface pas.

Le [passage Tier List 7223](stable-tier-controller-change/verification.json)
observe positivement UI, stockage et ressources, mais le vérificateur refuse
l'attestation parce que le contrôleur a changé pendant l'exécution. Le rapport
et les observations restent conservés séparément du passage ultérieur figé.

Les trois reprises finales utilisent les sources figées et le harnais corrigé.
Chacune sort avec le code zéro, sans erreur d'empreinte et avec son périmètre
demandé satisfait ; `complete` et `all_automated_obligations_satisfied` restent
faux. [Les résultats machine](final-runs.json) conservent cette distinction.

| Jeu / graine | Périmètre final satisfait | Rapport |
|---|---|---|
| Tier List / 7230 | UI WebKit/Chromium, stockage endommagé/refusé, rétention ; 135 vrais cycles import/export/suppression dans le pilote UI. | [Rapport](final-tier/verification.json), [journal](final-tier/run.log) |
| Punchline / 7231 | RTC causal sur WebKit sain puis perturbé, UI/audio, six dispositions, coupure/gel/recharge, entrées hostiles, rétention. | [Rapport](final-punchline/verification.json), [journal](final-punchline/run.log) |
| Sidequest / 7232 | RTC causal sain puis perturbé, parcours fini des questions, pertes GPU induites et reconstruction, UI WebKit/Chromium, rétention. | [Rapport](final-sidequest/verification.json), [journal](final-sidequest/run.log) |

Le premier échec RTC Punchline demeure inexpliqué et conservé : le passage final
positif n'établit pas sa cause. Aucun seuil ni moteur n'a été changé pour faire
passer cette reprise.

## Fiabilité du laboratoire

Les défaillances Toxiproxy deviennent des erreurs d'infrastructure bornées,
avec validation de la réponse et de la faute demandée. La disparition de
l'adresse LAN testée invalide le passage ; aucun repli loopback implicite.
Un paquet ACK frais ne rafraîchit plus un ancien paquet RTC. Le pilote
Sidequest parcourt son paquet fini dans les deux sens et observe les questions,
pas les animations : 39 transitions causales vérifiées en 60 secondes. Les
contrôles de rotation Punchline attendent l'événement natif et l'état DOM/layout
attendu ; une barrière de deux rAF ne remplace plus cette propriété.

La fermeture défaillante d'un navigateur/relais devient aussi une erreur H1,
avec nettoyage indépendant des autres ressources et conservation de l'erreur
primaire. Les comptes du relais prouvent les défauts dans chaque sens avant
les silences. Le corps SDP réécrit est transmis avec sa véritable longueur
UTF-8, sans anciens en-têtes d'encodage/transfert. Les [huit tests ciblés finaux](udp-final-tests.log)
incluent ce contrôle et passent après le dernier changement ; ils recouvrent
en partie les 57 tests précédents et ne s'y additionnent pas.

## Copie figée du modèle

Le travail parallèle a modifié le commun pendant plusieurs passages. Leurs
échecs d'empreinte ont été conservés. Une copie physique figée, identifiée par
[snapshot.json](snapshot.json), permet de vérifier le modèle sans écrasement ni
arrêt de l'autre chat. Son contrôle natif/WASM est satisfait (11/54 obligations
sur le périmètre demandé) ; les autres restent non attestées.

Le préflight LAN du modèle figé n'a pas répondu : le rapport et le journal sont
conservés séparément. Les tests locaux UI/stockage/ressources utilisent ensuite
un profil loopback explicitement identifié, sans attestation LAN. Ce dernier périmètre est satisfait avec empreinte stable :
[rapport machine](model-loopback-verification.json) et
[journal](soiree-model-frozen-loopback-valid.log). Les premiers
essais de ce profil ont été refusés par le vérificateur à cause de liens
symboliques de cache ; ceux-ci ont été remplacés par des copies physiques, sans
assouplir le vérificateur.

Les sources et leurs références sont dans salon/LEARNINGS.md, le contrat GPU,
et les LEARNINGS des jeux : W3C IndexedDB/WebAudio/WebGPU, Chrome autoplay,
wgpu/Khronos, Rust std, CDP, str0m et les projets audio cités.
