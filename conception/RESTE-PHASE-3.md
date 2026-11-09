# Ce qui reste de la phase 3

Commencé au lot 13, pour ce que sa ligne y envoie (PHASE-3, § 7) ; le lot 14 l'écrit en entier (« `conception/RESTE-PHASE-3.md` »). Chaque point dit d'où il vient et où sont ses chiffres.

> **Statut.** Écrit au lot 14, le 2026-11-09, sur la branche `refonte` (rien de commité par ce lot). Ce document fait pour la phase 3 ce que [`RESTE-PHASE-2.md`](RESTE-PHASE-2.md) a fait pour la phase 2 : ce que la phase laisse, point par point, avec d'où il vient. Il ne tranche rien : les décisions sont au mainteneur (§ 6), les gestes humains à la répétition ([`REPETITION-PHASE-3.md`](REPETITION-PHASE-3.md)), les chiffres dans [`MESURES-PHASE-3.md`](MESURES-PHASE-3.md), le détail de chaque lot dans [`LOTS-PHASE-3.md`](LOTS-PHASE-3.md). **Aucune étiquette n'est posée** : la question 19 n'est pas accordée ; le § 5 décrit ce que serait la première, pour que le mainteneur décide.

## 0. En une page

**Ce que les machines ont vérifié.** Une soirée entière se joue, dans Chrome et dans Firefox sans interface, depuis l'écran de démarrage : les trois jeux à la suite, « On rejoue » deux fois chacun (sans objet pour Tier list, qui n'a pas de fin), le changement de jeu sans rien relancer, le plein écran tenu dans Chrome sans interface (l'état du DOM, `document.fullscreenElement` ; *précisé après la vérification du lot 14* : pas une vraie fenêtre, la session de l'écran étant verrouillée), la garde finale reprise à l'identique dans wasmtime (`soiree-banc soiree-complete`, lot 14 ; le plein écran n'est pas jugé dans Firefox sans interface, qui le refuse au banc, mais l'est dans Firefox avec sa fenêtre). La vraie autorité tourne dans le Worker, reprend après un rechargement, un Worker tué, un second onglet (lot 10a) ; les téléphones jouent par la page du jeu, dans de vrais navigateurs et en natif (lot 12) ; le transport est chiffré de bout en bout (Noise NKpsk0, lot 4a) ; chaque milliseconde de l'entrée à la fin du travail du GPU est découpée par segment (lot 13). La page `/#mesure` du Wi-Fi est jouée de bout en bout sur la boucle locale (`soiree-banc mesure-wifi`, lot 14).

**Ce qui n'est pas vert.** Safari n'est exécuté nulle part (question 8). Un pas sauté dans Firefox onglet caché ou fenêtre réduite (question 30). Firefox avec sa fenêtre au-dessus du budget du clavier, selon la définition du pixel (question 32). Les points de reprise de Tier list au pire tiennent le Worker plusieurs dizaines de millisecondes (question 31). Les maxima de M1 ne se retrouvent pas (question 27). Le lot 13b attend la session de l'écran ouverte. La liste entière est au § 2.

**Ce qui est humain.** La soirée réelle, de vrais téléphones, le vrai Wi-Fi (M5), la caméra : **non faits**, avec la fiche qui les guidera (§ 3).

**Ce qui est au mainteneur.** Trente-trois questions de PHASE-3 (§ 9.1) et vingt-deux demandes des jeux (BESOINS-DES-JEUX, D1 à D22), réunies au § 6 avec une recommandation courte chacune ; les budgets proposés (§ 4) ; la première étiquette (§ 5).

## 1. La latence de l'entrée au pixel (lot 13, M2)

Le CONTRAT demande 99 % des gestes en moins de 50 ms depuis un clavier, 80 ms depuis un téléphone, le matériel à part. Mesuré au lot 13 sur machine calme (`MESURES-PHASE-3.md`, « Lot 13 », M2), de l'événement à la fin du travail du GPU (`onSubmittedWorkDone`), l'écran de la machine allumé :

- **Chrome 156 avec sa fenêtre** (120 images par seconde, l'écran de la machine) : le budget du clavier est tenu, 28,5 et 32,5 ms au 99ᵉ centile.
- **Chrome et Firefox sans interface** : tenu (34,5 à 38,2 ms dans Chrome à 60 Hz, 32,9 à 42,4 ms dans Firefox à 120 Hz, selon le passage). Le téléphone natif tient ses 80 ms (30 à 60 ms, glisser compris), sauf dans Firefox avec sa fenêtre (plus bas).
- **Glisser** (*corrigé après la vérification du lot 13*) : les chiffres « avec glisser » écrits d'abord ici (45 à 51 ms au clavier) venaient de modules sans aucun nœud qui glisse (`ardoise`, Punchline), sur une page qui retardait alors le pixel de toute scène pendant un pas : ils ne mesuraient pas un vrai glisser. La page ne glisse plus que si un nœud se déplace. **Mesuré sur Sidequest**, le seul module qui glisse (les gestes de ses six téléphones natifs déplacent les personnages), glisser forcé : le pixel d'un geste attend un pas de plus (16 ms de plus en médiane pour l'attente de l'image à 120 Hz) ; au 99ᵉ centile du téléphone, 49 à 51 ms dans Chrome avec sa fenêtre (34 à 36 sans glisser), 50 à 55 ms dans Chrome sans interface, 58 à 60 ms dans Firefox sans interface : sous les 80 ms. Dans Firefox avec sa fenêtre, 70 et 80,2 ms (72 et 77 sans glisser), le segment du GPU pesant le plus (52 à 65 ms au 99ᵉ centile) : le point suivant. Au clavier, aucun module ne glisse : le coût de glisser n'y est pas mesuré ; d'après le téléphone, ce serait un pas de plus au pixel.
- **Firefox 158 avec sa fenêtre : le budget n'est pas tenu, selon la définition du pixel** : 124 à 137 ms au 99ᵉ centile au clavier. **Le segment fautif est le GPU** : de la soumission du dessin (0,6 à 1,6 ms après le début de l'image) à la résolution de `onSubmittedWorkDone`, 33 à 63 ms en médiane, 84 à 110 ms au 99ᵉ centile. Ce n'est ni caché ni « corrigé » par un seuil. Ce qui n'est pas établi : que l'image soit à l'écran si tard. Firefox peut résoudre cette promesse bien après la présentation ; sans interface (le repli WebGL2), le même segment fait au plus une image. Le téléphone y passe aussi au-dessus de son budget une fois (Sidequest, glisser forcé, avec calage : 80,2 ms au 99ᵉ centile, le GPU nommé, 65 ms).
- *Écrit d'abord au lot 13, puis corrigé à sa reprise* : le premier agent avait nommé fautive l'attente de l'image de Chrome sans interface (69 à 73 ms au clavier) ; l'écran de la machine était éteint, et Chrome sans interface n'y donne que 30 images par seconde. Écran allumé, ce cas disparaît.

Ce qui reste à faire, sans seuil changé : regarder à la caméra (la fiche de répétition) quand l'image de Firefox est vraiment à l'écran, et la part de l'écran et de la télé ; Safari (pas exécuté). Au mainteneur : faut-il garder `onSubmittedWorkDone` pour dire le pixel dans Firefox, ou prendre un autre repère (l'image suivante, la caméra) ? ~~Et glisser, qui met le clavier à 45 ou 47 ms dans Chrome à 120 Hz, faut-il le garder tel quel (§ 2.9) ?~~ (*Retirée après la vérification du lot 13* : la question reposait sur des chiffres qui ne mesuraient pas un vrai glisser. Glisser coûte un pas aux gestes qui déplacent un nœud qui glisse, et rien aux autres ; mesuré sur Sidequest, le téléphone reste sous ses 80 ms partout sauf dans Firefox avec sa fenêtre, où le GPU pèse le plus. Rien n'est à trancher sur glisser à partir de ces chiffres.)

**Les premiers gestes après « Jouer »** (*ajouté après la vérification du lot 13*) : les gestes de la première seconde de la partie attendent leur image de 0,7 à 1,9 s (Punchline, `m2-punchline-chrome-ecran-allume`, pas 17 à 47, les six téléphones ; Sidequest à l'essai), tous rattachés à la première image qui dessine la scène du jeu. Le banc ne les compte pas (avant ses conditions) ; le panneau d'une soirée, si, après chaque « Jouer ». Cause probable, non établie : la transition depuis l'écran du commun, pendant laquelle la page ne dessine pas la scène du jeu. À regarder au lot 14 : la cause, et si ces gestes doivent compter au budget (`MESURES-PHASE-3.md`, « Lot 13 », M2).

*Établie au lot 14* (les lignes des passages `m2-punchline-*` du lot 13, relues ; `target/travail/lot-14/` n'ajoute aucune mesure) : **ces gestes sont faits avant « Jouer »**, pendant que l'écran de démarrage du commun couvre le jeu. Dans chaque passage, le dernier rapport d'état du Worker avant la ligne `geste` (« Jouer ») dit déjà ces gestes placés : au pas 108, 6 gestes dans les deux passages de Chrome ; au pas 105, 4 gestes dans Firefox ; et les traces de ces gestes ont des pas de 17 à 79, tous antérieurs. Leur attente de l'image décroît d'un pas à l'autre exactement comme le temps qui les sépare de « Jouer » (Chrome écran allumé : pas 17, 1 850 ms ; pas 32, 1 600 ms ; pas 47, 1 351 ms ; Firefox : pas 18, 1 816 ms ; pas 79, 799 ms) : tous attendent la même image, la première qui dessine la scène du jeu après le geste. Le premier geste placé **après** « Jouer » attend 22 à 32 ms son image (Firefox, pas 127 : 109 ms au total, la première image du jeu ; pas 168 : 48 ms). Ce n'est donc ni le transport, ni le Worker, ni le dessin : c'est la définition du pixel d'un geste fait sous l'écran du commun. Le banc ne les compte pas (ses conditions commencent après) ; **le panneau d'une soirée les compte**, après chaque « Jouer » et après chaque changement de jeu. Ce qui n'est pas fait : les en exclure (le panneau et `salle::trace` ne savent pas qu'un geste a été fait sous l'écran du commun) ; c'est la question 33, sans seuil changé.

## 2. Ce que la phase 3 laisse, sujet par sujet

Repris de chaque section « Ce qui n'est pas fait » de `LOTS-PHASE-3.md`, sans ce qui a été fait depuis. Chaque point renvoie au lot et, s'il en a une, à sa question (§ 6).

### 2.1 Les navigateurs

- **Safari n'est exécuté nulle part** : `safaridriver` refuse la session (« The session timed out while connecting to a Safari instance… ») tant que l'hôte n'a pas coché « Autoriser l'automatisation à distance » ; aucun réglage de la machine n'a été touché. M1, M9, M10, E20, `machine`, `dessin`, `soiree`, `aide`, `pages`, `soiree-complete`, `mesure-wifi` et les messages des pièges de JavaScriptCore y restent à faire (lots 3c à 14 ; question 8).
- **Firefox sans interface refuse le plein écran au clic du banc** (lot 10b) : Échap, le plein écran après « On rejoue » (`soiree-complete`) et le contrôle négatif qui le juge n'y sont pas exécutés sans interface. Avec sa fenêtre (`--avec-interface`, sur l'écran de l'hôte), Firefox tient le plein écran **même session verrouillée** (constaté au lot 14 : `soiree-complete` et son contrôle négatif y sont joués) ; Chrome avec sa fenêtre, non (lot 13b).
- **WebGPU dans Firefox sans interface** n'est pas exposé : la télé y dessine en WebGL2 (lot 9) ; avec sa fenêtre, WebGPU, et le segment du GPU de la question 32.
- **Le plein écran natif de macOS** (le bouton vert) : Échap n'y est pas constaté (lot 10b, correction) ; à la répétition.

### 2.2 La session de l'écran et le lot 13b

- **Le lot 13b n'est pas fini** : la souris de M2 avec la fenêtre de Chrome, et M6 réduite session ouverte, demandent la session de l'écran de l'hôte ouverte, restée verrouillée pendant les lots 13b et 14 (le déverrouillage demande le mot de passe de l'hôte : rien n'a été tenté). Pour les finir, l'hôte présent : `target/travail/lot-13b/campagne.sh m2 m6`, puis `soiree-banc arriere-plan --mode onglet --avec-interface` dans Chrome. La contre-épreuve de la cause (le plein écran qui tient session rouverte) avec eux.
- Deux chemins de `Chrome::etat_de_la_fenetre` n'ont pas tourné en vrai (la relecture qui échoue, le passage par `normal` depuis le plein écran) : seuls leurs tests natifs les tiennent (lot 13b, correction, constat 5).

### 2.3 La partie qui fait foi

- **Un pas sauté** quand le fil du Worker ne tourne pas pendant plus de deux pas : onglet caché ou fenêtre réduite de Firefox, et sous charge dans les deux navigateurs (lots 10a, 12, 13). `arriere-plan` reste rouge dans Firefox ; ni le contrôle ni `rattrapage` ne changent (question 30).
- **Les points de reprise de Tier list au pire** : l'encodage de l'instantané (41 à 112 ms), la fin du point (38 à 86 ms) et une grande case de journal (73 à 134 ms) tiennent chacun un tour du Worker, loin des 2 ms de R2 ; la plus longue tranche de scellement fait 2,08 à 2,34 ms (lot 11 ; question 31). L'encodage d'un bloc toucherait `jeu_abi::encoder`, donc l'épreuve (lot 10a, correction).
- **Les fenêtres du changement de jeu** : un Worker tué entre l'envoi du point neuf et l'écriture d'IndexedDB reprendrait l'ancien module (`Arret::Reprise`) ; un `ModuleRecu` d'avant compté pour le nouveau module ; un changement pendant un changement. Aucun scénario ne les joue ; les fermer monte `VERSION_CASES` ou `salle::fil::PROTOCOLE` (lot 12, fin). `reprendre_de_l_historique` ne traverse pas un changement de jeu : `changer` et `soiree-complete` ne jugent que la garde finale.
- **Le chien de garde n'a jamais tué de Worker dans un navigateur** (ses décisions sont éprouvées en natif) ; un relais relancé sous un écran du navigateur n'est pas joué ; le fichier d'une marque F8 n'est pas rejoué par l'épreuve comme un test (lot 10a).
- **La garde des présences d'un jeu sans téléphone** : gardée par précaution, sans preuve qu'elle serve (lot 12, fin).
- `soiree` (lot 10a) ne dit pas « sans objet » pour un jeu sans phase terminale : il attend 400 s et échoue sur Tier list (lot 11). `soiree-complete` le dit, par `fin: SansObjet` (lot 14).

### 2.4 Les téléphones

- **Ni son, ni motifs de vibration, ni verrou d'écran** sur le téléphone (lot 12) ; la page est hors contexte sécurisé.
- ~~**`Reglages::fond`** n'est pas appliqué par la page du téléphone : un écart du commun à INTERFACE (§ 2.9), écrit au lot 7b (D2).~~ *Fait après la vérification du lot 14* : la page pose le fond que donne chaque vue, #111 sinon ; les écrans du commun gardent le leur ; le scénario `pages` le relit, avec son contrôle négatif (D2). `ecran_allume` et `orientation` restent non appliqués.
- Des coups des bots restent sans chemin par la page (Sidequest : de 10 à 40 gestes par natif et par passage), comptés (lot 12) ; `pages` ne joue ni les locaux d'un joueur ni le transport.
- Pourquoi Chrome sans interface cessait d'accuser les touchers d'une page (lot 12, fin, constat 3) : non séparé.

### 2.5 La télé et le dessin

- **La police d'emoji** n'est pas livrée : un emoji se dessine comme une boîte, compté (lots 8 et 9 ; question 28, D15).
- **E16 ne mesure ni les noms des boutons ni les comptes à rebours ni les `EnPlace`**, bien que leur style soit fixé (lots 8 et 9) : le faire change ce que l'épreuve juge.
- Le panneau de diagnostic est du texte, pas des barres ; `Trace::relais` reste `None` (le transport d'un téléphone n'est pas découpé ; le faire changerait `liaison`) ; la souris et les natifs n'ont pas de segment « événement » (lot 13).
- Les 64 images perdues de Chrome en WebGL2 du lot 9 ne sont pas expliquées ; `/favicon.ico` répond `404`.
- Tier list : le glisser ne déplace pas la tuile en direct (un choix du jeu, `Survol` offert) ; le défilement au bord n'est éprouvé qu'en natif ; le collage, le dépôt de fichiers, une composition IME, Undo et Redo ne sont pas joués par `ecran` (lot 11).

### 2.6 L'épreuve

- **Le plan standard avec E20** n'est pas joué, et sa durée n'est pas mesurée (lots 3b, 3c ; question 24). E20 sur les fautifs et `ardoise` n'est pas relancé depuis le lot 3b (lot 8, correction).
- **`HorsMemoire`, `Piege`, `Profondeur` et `Memoire` ne passent pas par E20** : aucun module de Rust sûr ne les lève par une entrée ; ils restent tenus par les essais de `Machine` (lot 3b, correction).
- Les points 4 à 8 de RESTE-PHASE-2 restent différés (PHASE-3, § 9.3) : `Appareil` et `Message` qui se fabriquent, E8 et un secret transformé, les `build.rs` et la transmutation (question 26), E16 et les limites du jeu, les étapes 3, 5 et 6 du CONTRAT.
- Le monde simulé de l'épreuve ne passe pas par la vraie `salle` (question 12) ; ses constantes attendent M5 (§ 3).

### 2.7 Le jour J et les jeux

- **Le README de chaque jeu ne dit pas `soiree-relais jouer`** (P35) : il suppose l'étiquette du commun (§ 5).
- Les vingt-deux demandes des jeux après le lot 7b (D1 à D22) sont ouvertes (§ 6.2).
- `soiree status` n'a pas été relancé par les agents des lots 13 à 14 : `soiree/AGENTS.md` interdit la commande `soiree` à l'agent (l'ancien outil ne connaît pas les nouvelles crates) ; `git status` des quatre dépôts est relevé à la place.

## 3. Ce qui est humain, et non fait

**Non fait** : la soirée réelle avec de vrais joueurs ; de vrais téléphones (Safari iOS, Chrome Android) ; la mesure du vrai Wi-Fi à 8 à 15 téléphones (M5) ; la caméra au ralenti (le matériel, et le pixel de Firefox avec sa fenêtre) ; le retard de la télé et du son filmé ; le gel d'iOS ; 45 minutes de partie ; la fenêtre vraiment recouverte ou réduite, session ouverte ; la demande du pare-feu ; le plaisir. Tout est guidé par [`REPETITION-PHASE-3.md`](REPETITION-PHASE-3.md), y compris la caméra (§ 3 de la fiche : le carré blanc du panneau, trente appuis par navigateur) et les gestes qui attendent après « Jouer » (§ 4 de la fiche).

**Ce que M5 remplacera** (annexe « Les valeurs choisies » de PHASE-3) : `RAFALE` (100 ms), `PERTE_FIABLE` (1 sur 40), `RETRANSMISSION` (200 ms), `SYNCHRO` (5 s), `DERIVE_MAX_PPM` (200), `DECALAGE_MAX` (une minute), `FERME_EN_ARRIERE_PLAN` (5 s), `FERMETURE` (0,5 s), la gigue de deux pas, `liaison::SILENCE_MAX_MS` (3 s), `PAGE_SANS_LIEN_MS` (15 s), l'aide « aucun téléphone en 60 s », `BONJOUR_REDIT_US` (2 s), le lien sans `Bonjour` (10 s), et les valeurs du mandataire de pannes du banc (lot 5). ~~La page `/#mesure` donne ce qu'il faut (fiche, § 1) ; la répétition le fait.~~ *Corrigé après la vérification du lot 14* : la page `/#mesure` ne donne pas tout. Elle mesure l'aller-retour, les rafales, les écarts entre tics, les échos perdus, les coupures et reconnexions, la dérive de l'horloge et l'arrière-plan (fiche, § 1) : de quoi remplacer `RAFALE`, `PERTE_FIABLE`, `RETRANSMISSION`, `DERIVE_MAX_PPM`, `FERME_EN_ARRIERE_PLAN`, `FERMETURE`, la gigue et `SILENCE_MAX_MS`, et, au chronomètre, `PAGE_SANS_LIEN_MS` et l'aide des 60 s. Elle **ne mesure pas** :
- **`DECALAGE_MAX`** (le décalage de l'horloge d'un téléphone) : la page ne lit que `performance.now()`, jamais l'horloge murale, et le bilan n'a aucun champ de décalage. Le système n'en lit pas non plus (l'heure de l'autorité s'estime par l'écho, SAL-127) : ce décalage est une hypothèse du monde simulé, qui reste choisie ;
- **`SYNCHRO`** (la resynchronisation toutes les 5 s) : c'est un choix du monde simulé ; `derive_ppm` dit seulement si 5 s suffisent (200 millionièmes font 1 ms en 5 s), sans remplacer la valeur ;
- **`BONJOUR_REDIT_US` et le lien sans `Bonjour`** : la page de mesure ne fait ni Noise ni `Bonjour` ; les coupures et les échos perdus en donnent une idée, pas la valeur. Les accueils perdus se liront dans une vraie soirée (le relais et la garde), pas dans `/#mesure` ;
- **les valeurs du mandataire de pannes du banc** : la page ne joue pas de panne ; elles restent celles du lot 5, que les rafales et les coupures mesurées pourront seulement confirmer ou démentir.

La répétition fait la mesure (fiche, § 1, qui le dit aussi). M5 tranche aussi P11 : WebRTC reste hors de la phase 3 tant que la part du Wi-Fi au 99ᵉ centile tient dans les 30 ms qui restent au téléphone (§ 3 de PHASE-3).

## 4. Les budgets proposés au mainteneur

Le mainteneur seul les fixe, dans `CONTRAT.md` (REPONSES-PROVISOIRES, question 12). Rien n'est changé dans le code par ce lot ; chaque chiffre vient de `MESURES-PHASE-3.md`, avec sa date et sa machine (MacBook Apple M5, 16 Go ; Chrome 156, Firefox 158).

**4.1 La durée d'une unité de carburant (E19, question 4).** Aujourd'hui, 800 ps (200 × 2 × 2, trois nombres choisis, `epreuve::BUDGETS_E19`). Sur sept tours valides de chaque outil au calme (lot 13, M1, appels d'au moins 10⁶ unités), en ps par unité, « médiane ; 99ᵉ centile ; maximum » : Chrome, banc 28 à 37 ; 47 à 65 ; 57 à 493, harnais 36 à 39 ; 56 à 78 ; 67 à 136 ; Firefox, banc 62 à 66 ; 98 à 123 ; 217 à 592, harnais 64 à 70 ; 106 à 135 ; 120 à 318. Les maxima changent d'un facteur trois d'une soirée à l'autre et ne se retrouvent pas (question 27) : aucun maximum d'une soirée ne peut servir d'unité. **Proposition** : juger au 99ᵉ centile, et prendre l'unité du navigateur le plus lent que la soirée doit tenir (question 6). Le CONTRAT juge chaque image, pas un centile : une unité au 99ᵉ centile laisse un appel lourd sur cent plus lent que l'unité comptée, ce que la marge doit couvrir (le maximum mesuré, 592 ps dans Firefox, reste sous 800 ps) :
- Chrome seul : 65 ps au plus au 99ᵉ centile → **100 ps** × `MARGE_MACHINE` 2 = **200 ps** (les budgets de E19 multipliés par 4) ;
- Chrome et Firefox : 135 ps au plus au 99ᵉ centile → **200 ps** × 2 = **400 ps** (multipliés par 2) ;
- ou garder 800 ps tant que Safari n'est pas mesuré et que l'ordinateur de la soirée n'est pas connu (question 14) : le plus prudent, qui laisse à Tier list le moins de place.

`MARGE_MACHINE` reste à 2 tant que le banc n'a pas tourné sur l'ordinateur de la soirée. Le coût fixe d'un appel (`FIXE_NS` 10 µs, `PS_PAR_KIO_REMIS_A_ZERO` 10 ns) : garder (mesuré 0,3 à 0,5 µs et 6,2 à 7,0 ns par Kio dans les navigateurs ; MESURES-PHASE-3, « Proposition de budgets de E19 »).

**4.2 La latence (CONTRAT, « Les budgets »).** Garder 50 ms au clavier et 80 ms au téléphone, au 99ᵉ centile, le matériel à part. Ce qui est tenu au calme, de l'événement à la fin du travail du GPU : Chrome avec sa fenêtre, 28,5 à 33,6 ms au clavier ; Chrome et Firefox sans interface, 32,9 à 42,4 ms ; le téléphone natif 30 à 60 ms, glisser compris, sauf Firefox avec sa fenêtre (70 à 80,2 ms ; au clavier 124 à 137 ms, le GPU nommé). **Proposition** d'écrire dans le CONTRAT ce que le banc mesure : le pixel est la fin du travail du GPU de la première image qui dessine en entier la scène du tour qui a joué le pas (§ 8.4 de PHASE-3) ; un geste fait sous l'écran du commun ne compte pas (question 33) ; et le navigateur de la soirée est celui qui le tient (question 6 ; Firefox avec sa fenêtre attend la caméra, question 32).

**4.3 L'image (CONTRAT : aucune image au-delà de 33 ms en jeu).** Garder 33 ms. Mesuré (M8, lots 9 et 11) : aucune image de la télé au-delà, ni sur les pires cas des trois jeux (la plus longue 23,96 ms, Firefox WebGL2), ni pendant l'export du plateau plein de Tier list (le rendu hors écran par bandes, TL-57) ; mais des **tours du Worker** de plus de 33 ms (les points de Tier list au pire, la restauration pleine : jusqu'à 134 ms dans Firefox) font prendre du retard à la partie sans qu'aucune image ne dépasse. **Proposition** : écrire que le budget d'image vaut pour la télé, et dire à part ce que la partie peut prendre de retard sur un travail que l'hôte a demandé (un export, une restauration) ; c'est la question 31.

**4.4 Le tour du Worker (R2, 2 ms).** C'est un seuil de conception, pas du CONTRAT ; il est dépassé par les points de Tier list au pire (question 31). **Proposition** : le garder comme seuil, et décider à la question 31 si Tier list peut prendre quelques pas de retard sur ce plateau.

## 5. La première étiquette du nouveau commun (question 19, non accordée)

Rien n'est posé ni publié. Voici ce que serait la première étiquette, pour que le mainteneur décide.

**La version : `v0.4.0`.** Les étiquettes `v0.1.0` à `v0.3.0` sont celles de l'ancien commun, et une étiquette publiée ne bouge plus ; le nouveau commun casse tout (aucune API de l'ancien ne survit), ce qui monte le chiffre du milieu en `0.x` (`AGENTS.md`, « Versions »). `version` de `[workspace.package]` (aujourd'hui 0.1.0, dans `Cargo.toml` et `web/Cargo.toml`) passe à 0.4.0 dans le même commit. Une autre voie serait `v1.0.0` (une nouvelle base) : à dire si le mainteneur la préfère ; rien ne la demande.

**Ce qu'elle fige.** La branche `refonte` de `soiree/` à son dernier commit vérifié, fusionnée dans `main` ou étiquetée sur `refonte` (au mainteneur). Ce commit tient : les crates du commun (`jeu-abi`, `jeu-derive`, `jeu`, `jeu-natif`, `reecriture`, `moteur-commun`, `moteur-wasmtime`, `partie`, `epreuve`, `banc`, `liaison`, `salle`, `relais`, `mise-en-page`, les exemples et les fautifs) et le workspace `web/` (`moteur-navigateur`, `rendu`, `ecran`, `autorite`, `telephone`, `demarrage.js`) ; les formes : `jeu_abi::ABI` 4, `reecriture::VERSION` 3, `partie::persistance::VERSION_GARDE` 3, `salle::garde::VERSION_CASES` 2, `liaison::PROTOCOLE_RELAIS` 1, `salle::fil::PROTOCOLE` 1, `salle::ecran::PROTOCOLE` 4 ; et les documents de la refonte. `CHANGELOG.md` : la section « Non publié » prend le nom `v0.4.0` et sa date. Ce qui reste de l'ancien système (`CERTIFICATION.md`, `VERIFICATION.md`, `REPETITION.md`, `REJOINDRE.md`, `certifications/`, `recherche/`, `archive/`, `outil/`) partirait avec elle tel quel, sauf décision contraire (question 17).

**Avant de la poser** (`soiree/AGENTS.md` du dossier de l'hôte, « Un changement qui touche salon et un jeu », adapté au nouveau commun) : `cargo test --workspace`, clippy des deux workspaces, `cargo fmt --check`, le plan standard des trois jeux au code 0 depuis leurs sources, `soiree-banc soiree-complete` vert dans Chrome, sur l'arbre exact du commit étiqueté.

**Ce que les jeux changent ensuite**, chacun dans son dépôt, par son agent : leurs dépendances passent du chemin à l'étiquette,
```toml
jeu = { git = "https://github.com/build-your-own-game/soiree", tag = "v0.4.0" }
jeu-abi = { git = "https://github.com/build-your-own-game/soiree", tag = "v0.4.0" }
# de même pour jeu-natif, partie, moteur-wasmtime, mise-en-page
```
et le `.cargo/config.toml` de l'hôte (son modèle dans `soiree/outil/dossier/`) remet un `[patch."https://github.com/build-your-own-game/soiree"]` vers `soiree/crates/*`, pour que l'hôte développe encore les deux ensemble. `soiree upgrade` est l'ancien outil, qui ne connaît pas les nouvelles crates : le passage se fait à la main, et c'est dit dans « Passer à cette version ». Chaque README de jeu gagne la commande unique (`soiree-relais jouer`, P35). Un jeu ne se pousse qu'après l'étiquette poussée.

**Ce qu'un ami doit construire**, sur un Mac (le seul système éprouvé) :
1. Rust 1.99 avec la cible `wasm32-unknown-unknown` (le `rust-toolchain.toml` la demande), et la CLI `wasm-bindgen` 0.2.129 à l'identique de la crate (`cargo install wasm-bindgen-cli --version 0.2.129 --locked`), Chrome (ou Firefox).
2. Le commun à l'étiquette : `git clone --branch v0.4.0 https://github.com/build-your-own-game/soiree`, puis `cargo build --release -p relais -p epreuve` dans `soiree/`.
3. Le paquet de la page : `target/release/soiree-relais preparer`. Il se construit chez l'ami (réponse provisoire à la question 19) ; le joindre construit à l'étiquette éviterait `wasm-bindgen` chez lui, mais lui ferait croire un binaire qu'il n'a pas construit (le CONTRAT : l'épreuve qu'on relance soi-même).
4. Chaque jeu, cloné à côté (`games/<jeu>`), éprouvé chez lui au plan standard et déposé au catalogue : `target/release/soiree-epreuve ../games/<jeu> --catalogue ~/.soiree/catalogue` (environ 25 minutes pour les trois, mesuré sur un M5 : Sidequest seul 562 s ; question 24).
5. Le jour J : `soiree-relais jouer` depuis le dossier d'un jeu ; F9 pour changer de jeu.

**Ce qui reste risqué à l'étiquette** : Safari jamais exécuté ; la question 30 (un pas sauté dans Firefox caché) ; la question 18 (la page en `http` sur un Wi-Fi partagé) ; `build.rs` des jeux compilés chez l'ami sans bac à sable (question 26) ; aucun ami n'a encore construit le commun hors du dossier de l'hôte (`soiree check --locked --published` est l'ancien outil : l'équivalent nouveau, un clone propre qui construit et éprouve, n'est pas écrit).

## 6. Les questions au mainteneur, en un seul endroit

Réunies au lot 14 : les questions du § 9.1 de [`PHASE-3.md`](PHASE-3.md) (1 à 33 ; 32 et 33 ajoutées au lot 14) et les demandes ouvertes des jeux de [`BESOINS-DES-JEUX.md`](BESOINS-DES-JEUX.md) (D1 à D22). **Aucune n'est tranchée ici.** La réponse provisoire de chacune reste celle de sa source ; la recommandation est celle de l'agent du lot 14, courte, pour aider à décider, et ne change rien tant que le mainteneur ne l'a pas reprise. Les questions plus anciennes d'INTERFACE (§ 13) ont leur réponse provisoire dans `REPONSES-PROVISOIRES.md` ; celles qui restent ouvertes sont reprises ci-dessous par la question de PHASE-3 qui les porte (1, 7, 12, 13).

### 6.1 Les questions de PHASE-3, § 9.1

| # | La question, en bref | Recommandation |
|---|---|---|
| 1 | La page comme fonction du module (P19), à la place de la crate `<jeu>-page` | L'accepter : les trois jeux ont leur page ainsi depuis le lot 7, et le vocabulaire a grandi sans casser (D1 à D22 disent ce qui manque encore). |
| 2 | Le jour du service en ligne, d'où vient la page des téléphones | La laisser sans objet jusqu'à la phase 4 ; la poser avec le relais en ligne. |
| 3 | Le relais sert en boucle locale le catalogue, les modules et les flux (écart à « une page de démarrage ») | L'accorder et l'écrire dans le CONTRAT : rien de cela ne sort de la boucle locale (tenu par les tests du relais). |
| 4 | Les budgets de E19 | Choisir d'abord les navigateurs (6), puis l'unité au 99ᵉ centile du plus lent (§ 4.1) ; ne rien régler sur un maximum. |
| 5 | Une exception `unsafe` pour `wasm-bindgen` | La clore : la macro ne l'exige pas (constaté au lot 3a). |
| 6 | Les navigateurs que l'ordinateur de la soirée doit tenir | Chrome avec sa fenêtre seul, tant que la caméra n'a pas tranché le pixel de Firefox (32) et que Safari n'est pas exécuté (8). |
| 7 | Sidequest : la pause (a) et la coupure (b) | Prendre la réponse provisoire (le cahier pour un tenu à fraîcheur) : c'est ce que font la page et l'épreuve depuis le lot 12. |
| 8 | Safari dans le banc (« Autoriser l'automatisation à distance ») | Le cocher une fois, sur la machine de l'hôte, pour exécuter Safari partout avant l'étiquette ; sinon Safari reste hors de la soirée. |
| 9 | Le second workspace `web/` | L'accorder : la règle « mêmes lints » le tient égal au premier. |
| 10 | Rejoindre sans caméra (un code court) ou le QR seul | Le QR seul en phase 3 ; mesurer à la répétition combien de téléphones n'ont pas de caméra qui lit un QR. |
| 11 | Les polices en TTF ou OTF seulement | L'accorder : les trois jeux s'y sont faits (lot 7). |
| 12 | Le monde simulé par la vraie `salle` | En phase 4, avec la prédiction ; ses constantes d'abord remplacées par M5 (§ 3). |
| 13 | Les gros états en blocs immuables | Pas avant que la question 31 soit tranchée ; D21 en serait un premier pas. |
| 14 | Le matériel (télé, ordinateur, routeur, enceintes) | Le renseigner dans le CONTRAT avant la répétition : la caméra mesure la part du matériel sur ce matériel-là. |
| 15 | Signer `soiree-relais` pour le pare-feu | Non en phase 3 : la fiche de la veille dit d'autoriser après chaque compilation. |
| 16 | Un module non éprouvé, autrement que par `--non-eprouve` | Non : l'insigne « NON ÉPROUVÉ » permanent suffit. |
| 17 | La fiche neuve remplace-t-elle `REPETITION.md` | Remplacer après la première répétition faite avec la fiche neuve (`REPETITION-PHASE-3.md`), et archiver l'ancienne avec les autres documents de l'ancien système. |
| 18 | « Tout est chiffré » et la page en `http` sur un Wi-Fi partagé | Accepter la limite pour la phase 3, dite à l'hôte (la fiche le dit) ; la fermer avec l'app (phase 5) plutôt qu'avec un certificat local. |
| 19 | La première étiquette | Après le lot 14 vérifié, `v0.4.0`, le paquet construit chez l'ami (§ 5) ; mais d'abord Safari (8) et la répétition, si l'étiquette doit servir à une vraie soirée chez un ami. |
| 20 | E20 pour le catalogue ou pour publier | Pour publier seulement, comme provisoirement. |
| 21 | Échap en plein écran | Garder Keyboard Lock dans Chrome et l'écran du commun ailleurs (constaté dans Chrome et dans Firefox avec sa fenêtre). |
| 22 | Le budget de `Page` sur un téléphone | Garder la marge de 4 jusqu'à la mesure sur de vrais téléphones (fiche, § 5). |
| 23 | Le personnage retenu de Sidequest et l'ITP de Safari | Accepter la perte, dite. |
| 24 | La durée des vérifications | Ne pas borner l'épreuve standard ; mesurer E20 sur le plan standard avant l'étiquette, puisqu'il est exigé pour publier. |
| 25 | Les effets provisoires | Sans objet en phase 3. |
| 26 | `build.rs` compilés sur l'ordinateur de la soirée | La même confiance que cloner un dépôt, dite dans chaque README ; le bac à sable en phase 4 au plus tôt. |
| 27 | M1 : les maxima du banc | Décider que le maximum d'une soirée n'est pas une mesure de l'outil, et juger M1 aux médianes et au 99ᵉ centile (au calme, le banc y est dans l'étendue du harnais ou au-dessous, lot 13). |
| 28 | La police d'emoji | Autoriser Noto Emoji (OFL) : elle ferme D15 et compte les glyphes des trois jeux. |
| 29 | Le sens de chaque `Courbe` | Garder celui que le rendu dessine (§ 2.9). |
| 30 | Un saut d'un pas quand le Worker ne tourne pas | Rapporter au contrôle le saut qui suit un réveil en retard de plus de deux pas, et demander aux jeux un `rattrapage` d'au moins 2. |
| 31 | Le point de Tier list au pire dans un seul tour | Accepter quelques pas de retard pour Tier list (rien n'y dépend du temps hors du glisser) ; encoder l'instantané d'un bloc quand `jeu_abi` montera pour autre chose. |
| 32 | *Nouvelle (lot 13, numérotée au lot 14).* `onSubmittedWorkDone` dit-il le pixel dans Firefox avec sa fenêtre ? | Attendre la caméra (fiche, § 3) ; si elle voit l'image bien avant, dire le pixel de Firefox par la caméra et garder `onSubmittedWorkDone` comme une borne haute. |
| 33 | *Nouvelle (lot 14).* Un geste fait sous l'écran du commun (avant « Jouer », sous un voile) compte-t-il au budget de latence ? | Non : le compter à part au panneau (« sous l'écran du commun »), puisque le joueur ne voyait pas le jeu. |

### 6.2 Les demandes des jeux après le lot 7b (BESOINS-DES-JEUX, D1 à D22)

Une ABI nouvelle (`jeu_abi::ABI` 5) fait repasser les trois jeux au plan standard depuis leurs sources ; plusieurs demandes peuvent partager une seule montée (BESOINS-DES-JEUX, « Ce que coûte une ABI nouvelle »). **Recommandation d'ensemble** : faire d'abord ce qui ne coûte pas d'ABI (D2, fait après la vérification du lot 14 ; D4, D6, D7, D13 pour le clavier, D15, D16, D20 pour les images, D22), puis réunir dans **une seule ABI 5**, après la répétition, ce que la répétition aura confirmé (D1, D3, D9, D11 d'abord : demandés par deux jeux ou bloquants pour le jeu).

| # | La demande | Jeux | ABI 5 ? | Recommandation |
|---|---|---|---|---|
| D1 | Une coupe « entre les mots, puis entre graphèmes pour un mot trop long » | les trois | oui (une troisième `Coupe`), ou non (`Coupe::Mots` change de sens) | Une troisième `Coupe` dans l'ABI 5 : changer le sens de `Coupe::Mots` sans rupture de type est plus dangereux (CONTRAT, « Les versions »). |
| D2 | La page applique `Reglages::fond` | Punchline | non | ~~La faire : c'est un écart du commun à INTERFACE (§ 2.9), promis « au plus tard avant la fiche de répétition du lot 14 » et **non fait au lot 14** (hors de sa ligne) ; les écrans du commun gardent leur fond.~~ **Fait après la vérification du lot 14** : un écart du commun à INTERFACE (§ 2.9), corrigé à l'échéance que PHASE-3 lui donnait. Reste au mainteneur : garder #111 aux écrans du commun (le voile), ou leur donner le fond du jeu. |
| D3 | L'avis d'un refus donné à la page | Punchline, Sidequest | oui | L'accorder dans l'ABI 5 (deux jeux) : le dernier avis et son pas dans `LocauxFil`. |
| D4 | Ce que valent les champs dans les locaux | Punchline | non | L'écrire et le tenir par un test de `salle` ; trancher avec D11. |
| D5 | Un champ sur plusieurs lignes | Punchline (Tier list, besoin 17) | oui | Différer jusqu'à ce que les cahiers s'accordent sur le retour à la ligne. |
| D6 | Les retours à la ligne du nom d'un bouton | Punchline | non | La faire (`white-space: pre-line`), avec un cas au scénario `pages`. |
| D7 | Le clavier et le focus sur la page | Punchline | non | La faire (`enterkeyhint` déduit, Entrée vers le champ suivant) ; le focus au chargement, à constater sur de vrais téléphones. |
| D8 | Personnages, pâte, compte à rebours pressé, nœud masqué | Punchline | oui | Le nœud masqué dans l'ABI 5 ; la pâte et le « pressé » après la répétition, s'ils manquent aux joueurs. |
| D9 | L'option d'un `choix` montrée choisie | Sidequest | oui | L'accorder dans l'ABI 5, l'option par défaut hors des locaux tant qu'on n'a pas touché. |
| D10 | Un choix multiple borné | Sidequest | oui | Différer : deux `choix` jouent. |
| D11 | La valeur de départ d'un champ | Sidequest | oui | L'accorder dans l'ABI 5, avec la règle de D4 (un champ prérempli jamais touché n'est pas dans les locaux). |
| D12 | Un geste envoyé seul à l'arrivée de la page | Sidequest | oui | Refuser pour l'instant : un appui suffit, et « une fois » à travers un rechargement est difficile à tenir. |
| D13 | La croix au clavier ; un bouton en son centre | Sidequest | non (clavier) ; oui (centre) | Le clavier sans ABI ; le centre différé. |
| D14 | Les animations de la page | Sidequest | oui | Différer après l'ABI 5 : moyen à lourd, rien n'est injouable sans. |
| D15 | La police d'emoji | Sidequest, Punchline | non | La livrer dès la question 28 accordée. |
| D16 | Les glyphes plus larges des polices de repli | Punchline (un constat) | non | Une ligne au diagnostic qui compte les textes mesurés dans une police de repli. |
| D17 | Un interligne dans `StyleTexte` | Tier list | oui | Différer : Tier list ne le demande pas pour lui seul. |
| D18 | L'allure d'un bouton de la télé ; la couleur d'un `Champ` | Tier list | oui (allure) ; non (couleur) | La couleur du champ sans ABI (une encre fixe du commun) ; l'allure, juger à la répétition. |
| D19 | Info-bulle, nom accessible, texte de remplacement ; une fenêtre fixe | Tier list | oui | Différer : l'accessibilité est hors de la phase 3 (§ 5 de PHASE-3). |
| D20 | Le bot nomme un sélecteur ; des images non dessinées | Tier list | oui (sélecteur) ; non (images) | Les images non dessinées dans l'épreuve sans ABI ; le sélecteur dans l'ABI 5. |
| D21 | Un point sans le segment des assets | Tier list | non (la garde change) | Peser avec la question 31 : environ 4 % sur le pire cas, lourd ; pas avant. |
| D22 | La touche de diagnostic et le champ qui a le focus | Tier list | non | Écrire la règle : la touche de diagnostic d'un jeu cède au champ qui a le focus (F8 jamais). |
