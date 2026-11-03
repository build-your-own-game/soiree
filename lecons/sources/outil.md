# Leçons de l'ancien code : la méthode de vérification

## Source couverte

La méthode de vérification de l'ancien commun, à l'étiquette `avant-refonte` (commit `503b386`) :

- `VERIFICATION.md`, `CERTIFICATION.md`, `REPETITION.md` ;
- `outil/` : le CLI `soiree` (`src/`, `tests/`), `outil/regles/`, `outil/verification-kernel/` (README, `src/lib.rs`, `verify.py`) ;
- `tools/cert/` : le harnais Node, Playwright et Toxiproxy ;
- `recherche/` : `README.md`, `sous-systemes.toml` et les quatre fiches `reconnexion`, `fiabilite`, `transport-webrtc`, `telecommande` ;
- l'historique git de ces chemins : 126 commits, du 26 octobre au 2 novembre 2026.

## Ce que j'ai lu

- En entier : `MANIFESTE.md`, `CONTRAT.md`, `VERIFICATION.md`, `CERTIFICATION.md`, `REPETITION.md`, `AGENTS.md` du commun, le README du noyau, `outil/regles/` (les quatre fichiers), `recherche/README.md`, l'en-tête de `sous-systemes.toml`.
- Les messages complets des 126 commits, dont les corps.
- Les commentaires et la documentation de tous les fichiers de `outil/src/` et de `tools/cert/*.mjs`, et les noms de tous les tests de `outil/tests/`. Des passages ciblés de `cert.mjs`, `browser.mjs`, `performance.mjs`, `mutants.rs`, `verify.py`.
- Les fiches de recherche par extraction : les 55 trous (titre, gravité, ce qui est vérifié, ce qui est supposé), les 190 leçons (énoncé, catégorie, état), le coût et la saturation de chaque fiche.
- `CHANGELOG.md`, sections v0.4.0 à v0.2.0 ; `archive/CHANTIERS-2026-10.md` ; `.github/workflows/` (`nuit.yml`, `host-deadlines.yml`) ; `tools/rehearse-upgrade.py` (en-tête) ; le bilan `certification/contracts-v0.3.0/` du commit `0acc4e3`.

## Ce que je n'ai pas lu

- Le corps de la plupart des fonctions de `outil/src/` et de `tools/cert/`, au-delà des commentaires et des passages cités. Les fichiers `*.test.mjs` du harnais, hors leur nom.
- Les diffs des commits : seuls les messages sont lus.
- Les citations et les liens des 190 leçons des fiches, et les références externes elles-mêmes (Colyseus, str0m, WebKit…). Les leçons de réseau de ces fiches ne sont reprises ici que lorsqu'elles touchent la vérification, les vrais appareils ou un trou que nos sondes ont prouvé. Les autres restent à `avant-refonte:soiree/recherche/*.toml`, et relèvent de la source « réseau ».
- `remote/bench`, les tests de `salon/`, les dépôts des jeux, les rapports de `certification/` des jeux, `node_modules/`.
- Rien n'a été exécuté.

Les références `chemin:ligne` désignent les fichiers à l'étiquette `avant-refonte`.

---

## Ce qu'un résultat a le droit de dire

### OUT-01 · Un contrôle qui n'a pas tourné n'est jamais une réussite
- **Ce qui arrive :** un test ignoré, un nom absent, zéro test sélectionné, une observation périmée ou une compilation échouée laissaient croire à une obligation tenue.
- **D'où on le sait :** `VERIFICATION.md:161-166` ; `outil/verification-kernel/README.md:10-20` ; commit `0acc4e3`.
- **Mécaniques :** toutes.
- **Question :** ce résultat vient-il d'une exécution réussie, avec la méthode et le périmètre attendus, sur les entrées courantes ?
- **Pression :** le noyau `assess` prouvé par Verus ; `outil/tests/verification_observer.rs` (`green_stdout_and_zero_exit_cannot_replace_missing_or_stale_raw_evidence`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-02 · Les issues d'un contrôle restent distinctes, sans échelle de force
- **Ce qui arrive :** un seul mot « vert » mélangeait réussite, sans objet, inconnu, jugement humain et panne de l'outil. Une mesure de latence et une preuve d'absence de débordement ne se comparent pas.
- **D'où on le sait :** `VERIFICATION.md:153-159` ; `outil/verification-kernel/README.md:22-25` ; `tools/cert/cert.mjs:203`.
- **Mécaniques :** toutes.
- **Question :** le rapport distingue-t-il réussi, échoué, ignoré, non exécuté, panne d'infrastructure, revue nécessaire et sans objet ?
- **Pression :** les tests exhaustifs du noyau (`outil/verification-kernel/src/tests.rs`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-03 · Une applicabilité inconnue n'est jamais une exclusion
- **Ce qui arrive :** une exigence dont on ne sait pas si elle s'applique risquait d'être écartée par défaut, ou parce que l'auteur le demandait.
- **D'où on le sait :** `outil/verification-kernel/README.md:31-35` ; `VERIFICATION.md:108-114` et `142-151` ; `outil/src/verification/resolve.rs:444-445`.
- **Mécaniques :** toutes.
- **Question :** cette exclusion vient-elle d'une règle du commun, ou seulement d'une phrase de l'auteur ?
- **Pression :** le noyau : `Excluded` n'est construit que par la politique ; relier une exigence la rend applicable, rien ne la rend non applicable.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-04 · Une déclaration du jeu ajoute des obligations, elle n'en retire jamais
- **Ce qui arrive :** retirer `socket` de `cert.json` rendait E1 à E4 et C7 sans objet ; `"phone": null` en retirait dix de plus, sans un mot. `layout: false` semblait exclure la zone sûre.
- **D'où on le sait :** commits `e24004f`, `6764852` ; `outil/src/verification/resolve.rs:1` et `533-534`.
- **Mécaniques :** téléphones, réseau, affichage TV.
- **Question :** que se passe-t-il si un jeu retire une clé de sa configuration ?
- **Pression :** `outil/tests/negative_controls.rs` (`a_game_cannot_drop_its_network_items_by_editing_cert_json`).
- **Sous la nouvelle forme :** pression du commun. Les lois communes valent pour tous les jeux ; seule une mécanique déclarée en ajoute.
- **Étage :** commun.

### OUT-05 · Une raison doit dire quelque chose
- **Ce qui arrive :** `x` excusait une décision de catalogue, une catégorie d'interface ou la baisse d'un niveau de recherche.
- **D'où on le sait :** commits `1289ad1`, `2b9e0aa` ; `VERIFICATION.md:135-137`.
- **Mécaniques :** toutes.
- **Question :** chaque « sans objet » porte-t-il une raison qu'un humain peut juger ?
- **Pression :** `outil/tests/negative_controls.rs` (`a_hollow_reason_or_an_empty_command_fails_the_real_check`). Le seuil retenu : deux mots et 12 caractères. Les plus courtes vraies raisons des jeux, comme « Pas de saut. », passent.
- **Sous la nouvelle forme :** pression du commun. C'est la garantie d'examen.
- **Étage :** commun.

### OUT-06 · Une commande qui n'observe rien ne prouve rien
- **Ce qui arrive :** `["true"]` validait U1, T2 et D2 : le harnais ne lisait que le code de sortie. Un filtre de test qui ne sélectionnait plus rien sortait aussi à zéro.
- **D'où on le sait :** commit `1289ad1` ; `outil/src/ui_contract.rs:122-125` ; `CERTIFICATION.md:189-195`.
- **Mécaniques :** toutes.
- **Question :** ce contrôle a-t-il rencontré le jeu, et qu'a-t-il imprimé ?
- **Pression :** refus d'une commande sans `{url}` ni motif `success` ; une commande avec `{url}` seule garde sa limite écrite dans le rapport.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-07 · Zéro cas, zéro fiche, zéro test : rien n'est attesté
- **Ce qui arrive :** `soiree recherche verifie` sortait à 0 avec « 0 contrôle(s) » sur un dossier vide ; une campagne réduite à zéro cas aurait pu passer.
- **D'où on le sait :** commit `35015c9` ; `VERIFICATION.md:251-252`.
- **Mécaniques :** toutes.
- **Question :** combien de cas ce contrôle a-t-il vraiment joués ?
- **Pression :** `outil/tests/recherche.rs` (`verifie_with_nothing_to_verify_never_succeeds`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-08 · Un jugement humain reste visible sans faire échouer, un contrôle inachevé fait échouer
- **Ce qui arrive :** les deux situations se confondaient, et un passage avec des points à juger semblait soit raté, soit fini.
- **D'où on le sait :** commits `23b6cf6`, `c741483` ; `VERIFICATION.md:196-204` ; `outil/verification-kernel/src/lib.rs:152-161`.
- **Mécaniques :** toutes.
- **Question :** qui doit encore regarder quoi, et qu'est-ce qui n'a pas pu finir ?
- **Pression :** la décision de passage est dans le noyau prouvé, avec un quatrième mutant rejeté ; `verification_observer.rs` (`a_human_judgement_passes_the_command_without_counting_as_verified`, `a_check_that_could_not_finish_fails_the_real_command`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-09 · Le résumé ne dit « réussi » que si c'est vrai
- **Ce qui arrive :** un passage avec des jugements imprimait « Aucun problème trouvé », les mots que la doc donnait comme fin du travail. Un passage raté imprimait aussi la phrase de réussite. Le résumé additionnait le sans-objet au vérifié, et comptait le code de sortie du harnais comme un point de plus.
- **D'où on le sait :** commits `6d74558`, `5e35d59`, `92516af`, `c741483` ; `outil/src/verification/render.rs:143-145`, `171`, `480-481`.
- **Mécaniques :** toutes.
- **Question :** la première ligne du rapport dit-elle exactement ce qui est fait, à juger et pas regardé ?
- **Pression :** tests de rendu ; la note la plus grave vient en premier.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-10 · Un périmètre réduit reste visible, et deux passages ne fusionnent pas
- **Ce qui arrive :** `check --fast` ou `certify -- --only ui,panic` pouvaient se lire comme le plan complet. Aucun passage seul n'atteignait d'ailleurs le plan complet : `check`, `certify` et les gestes humains se complètent.
- **D'où on le sait :** `VERIFICATION.md:185-243` ; commit `a6fcd08`.
- **Mécaniques :** toutes.
- **Question :** qu'est-ce que ce passage n'a pas regardé ?
- **Pression :** la sélection entre dans l'identité du rapport ; `--only` vide ou inconnu échoue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-11 · Une panne de l'outil n'est jamais une faute du jeu
- **Ce qui arrive :** sous Windows, le harnais tombait à chaque scénario et le rapport titrait « 23 points à corriger » dans le jeu. Une préparation terminée normalement pouvait passer pour un contrôle exécuté.
- **D'où on le sait :** commit `5e9bf2b` ; `outil/src/certify.rs:12-13`, `36-37` ; `tools/cert/cert.mjs:2142-2143` ; `CERTIFICATION.md:62` (H1).
- **Mécaniques :** toutes.
- **Question :** ce rouge accuse-t-il le jeu ou l'outil ?
- **Pression :** `outil/tests/windows_refusal.rs` ; `verification_observer.rs` (`abort_preserves_finished_observations_and_reports_infrastructure_failure`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-12 · Une note écrite hors de son scénario ne disparaît pas
- **Ce qui arrive :** le verdict ne lisait que les paires scénario-item déclarées. Un jugement de T2 écrit pendant `join`, ou une note des oracles communs, s'évaporait. Une note plus tardive adoucissait la santé du contrôleur.
- **D'où on le sait :** commit `457c45c` ; `outil/src/verification/harness.rs:353-357`.
- **Mécaniques :** toutes.
- **Question :** où va une observation qu'aucun contrôle sélectionné n'écoute ?
- **Pression :** `verification_observer.rs` (`a_judgement_outside_its_scenario_is_shown_by_the_real_command`, `an_unfinished_check_outside_its_scenario_fails_the_real_command`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-13 · Une liaison nomme des contrôles, elle ne prouve pas qu'ils couvrent l'exigence
- **Ce qui arrive :** `check.md` écrivait « satisfait » pour une exigence reliée à un test, alors que son hypothèse ne vivait que dans le JSON, comme si le test couvrait toute l'exigence.
- **D'où on le sait :** commit `6da0b44` ; `outil/src/verification/model.rs:20-21` ; `outil/src/verification/render.rs:133-134`.
- **Mécaniques :** toutes.
- **Question :** sous quelle hypothèse ce test tient-il cette loi ?
- **Pression :** l'hypothèse est imprimée à côté du résultat ; une liaison invalide ne relie rien et fait échouer (`resolve.rs:380-381`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-14 · Une question non revue reste visible, distincte du résultat
- **Ce qui arrive :** les 97 items du catalogue sans décision s'affichaient `unreviewed`, mais un passage réussi pouvait laisser croire que tout avait été étudié.
- **D'où on le sait :** `VERIFICATION.md:116-140`.
- **Mécaniques :** toutes.
- **Question :** chaque question de la base a-t-elle sa réponse ?
- **Pression :** aucune qui bloque dans l'ancien code. Le nouveau contrat le durcit : une question sans réponse bloque l'épreuve.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-15 · Une justification ou une revue n'est jamais importée comme preuve
- **Ce qui arrive :** la tentation existait de transformer un texte d'agent ou un compte rendu humain en résultat automatisé.
- **D'où on le sait :** `VERIFICATION.md:285-289` ; `AGENTS.md` du commun, « Les contrats de vérification ».
- **Mécaniques :** toutes.
- **Question :** cette réussite vient-elle d'une machine ou d'une phrase ?
- **Pression :** les gestes S1 à S10 restent en attente dans le rapport.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-16 · Une observation pour une autre identité ne satisfait rien
- **Ce qui arrive :** un ancien rapport ou le résultat d'un autre scénario pouvait remplir une case.
- **D'où on le sait :** `verification_observer.rs` (`a_result_for_another_scenario_cannot_satisfy_cut`) ; `CERTIFICATION.md:60` (aucun ancien rapport repris).
- **Mécaniques :** toutes.
- **Question :** cette observation appartient-elle à ce contrôle, sur ces entrées ?
- **Pression :** le noyau : fraîcheur, méthode et périmètre sont des faits séparés.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Les faux verts trouvés

### OUT-17 · Tout marquer `#[ignore]` ne fait pas taire un test rouge
- **Ce qui arrive :** une crate dont tous les tests étaient ignorés donnait `cargo:suite = passed`, avec zéro test exécuté.
- **D'où on le sait :** commit `632eff6` ; `VERIFICATION.md:178-183` ; `outil/src/verification/check.rs:280-283`.
- **Mécaniques :** toutes.
- **Question :** une crate qui a des tests en a-t-elle exécuté au moins un ?
- **Pression :** `verification_observer.rs` (`a_suite_where_nothing_ran_cannot_pass_the_real_command`, `a_crate_whose_tests_are_all_ignored_is_not_saved_by_its_neighbour`). Un test ignoré parmi d'autres est compté, nommé et laissé à un humain ; une crate sans test n'est pas un trou.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-18 · Lire le texte ne prouve pas qu'un test existe ni qu'il tourne
- **Ce qui arrive :** chercher `fn` dans les sources prenait `#[cfg(test)] fn helper` pour un test et manquait `#[test] #[ignore]` sur une seule ligne. Un trou de recherche se fermait sur n'importe quel fichier.
- **D'où on le sait :** commits `0acc4e3`, `011c8c5` ; `outil/src/verification/cargo.rs:1-4` ; `outil/src/recherche/controle.rs:151-163`.
- **Mécaniques :** toutes.
- **Question :** ce test a-t-il été découvert par Cargo et libtest, puis exécuté ?
- **Pression :** le runner demande ses exécutables à Cargo, puis sa liste et ses résultats à libtest.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-19 · Un test se désigne par son identité complète
- **Ce qui arrive :** deux tests de même nom, dans deux cibles ou deux configurations, se confondaient.
- **D'où on le sait :** `VERIFICATION.md:53-59`.
- **Mécaniques :** toutes.
- **Question :** quel paquet, quel genre de cible, quelle cible, quel nom complet, quelles features, quel profil ?
- **Pression :** la validation stricte de `verification.toml`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-20 · La sortie de libtest a ses formes : `should panic` en est une
- **Ce qui arrive :** libtest écrit le nom suivi de `- should panic`. L'analyseur ne le reconnaissait pas : toute la cible devenait une erreur d'infrastructure, et un `should_panic` raté n'était pas rapporté.
- **D'où on le sait :** commit `be86064` ; `outil/src/verification/cargo.rs:991-998`, `1448`.
- **Mécaniques :** toutes.
- **Question :** l'analyseur a-t-il été écrit contre la sortie réelle de la version de Rust figée ?
- **Pression :** test sur les lignes exactes de libtest 1.99, lues d'un vrai passage.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-21 · Un délai dépassé est une panne, jamais une réussite
- **Ce qui arrive :** un contrôleur qui ne finit pas risquait de laisser une case vide lue comme verte, ou une fermeture expirée comme réussie.
- **D'où on le sait :** `VERIFICATION.md:172-176` (15 min par compilation et par cible, 30 s pour l'inventaire) ; `tools/cert/cleanup.mjs:4-5`.
- **Mécaniques :** toutes.
- **Question :** chaque processus d'un contrôle a-t-il une échéance, et son dépassement un nom ?
- **Pression :** erreur d'infrastructure à l'échéance. Les commandes hors du runner (fmt, Clippy, doctests) n'avaient pas de délai : c'était un trou connu.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-22 · On arrête un groupe de processus possédé, jamais un PID trouvé sur un port
- **Ce qui arrive :** une cible peut laisser des enfants qui gardent stdout ouvert après sa sortie ; `cargo run` et le jeu qu'il lance doivent mourir ensemble ; un PID mort peut être réutilisé ; un zombie n'est pas du travail vivant.
- **D'où on le sait :** `outil/src/verification/cargo.rs:368-369` ; `tools/cert/cert.mjs:289` ; `tools/cert/processes.mjs:9`, `64` ; `CERTIFICATION.md:391-393`.
- **Mécaniques :** toutes.
- **Question :** l'outil sait-il prouver que tout ce qu'il a lancé s'est arrêté ?
- **Pression :** `tools/cert/processes.test.mjs` ; une fermeture non vérifiée est H1.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-23 · Une promesse n'est pas une valeur vraie
- **Ce qui arrive :** une attente d'interface sondait une expression ; un objet Promise est « vrai » en JavaScript, même s'il résout `false`.
- **D'où on le sait :** `VERIFICATION.md:329-330` ; `tools/cert/ui.mjs:32-33` ; `CHANGELOG.md`, v0.3.0.
- **Mécaniques :** interface.
- **Question :** l'oracle lit-il la valeur résolue, avec une échéance ?
- **Pression :** `tools/cert/ui.test.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-24 · Un écran n'est revenu que s'il reçoit de l'hôte un accueil ou un paquet du jeu
- **Ce qui arrive :** un refus, ou un au revoir, se lisait comme une reconnexion réussie.
- **D'où on le sait :** commit `7845710` ; `CERTIFICATION.md:60` ; `tools/cert/browser.mjs:137-140` ; `tools/cert/cert.mjs:1903-1904`.
- **Mécaniques :** reconnexion.
- **Question :** qu'est-ce qui prouve qu'un appareil a vraiment retrouvé la partie ?
- **Pression :** scénarios `cut`, `reload`, `restart`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-25 · On compare la place retrouvée, on ne la suppose pas
- **Ce qui arrive :** C4 et C5 tenaient un formulaire de retour pour un jugement humain. Un écran vide après redémarrage, ou une place non rendue après récupération du stockage, n'échouaient pas.
- **D'où on le sait :** commit `c741483` ; commit `7a6857e` (un téléphone resté sur l'écran de jeu sans nouvelles est figé).
- **Mécaniques :** reconnexion, identité.
- **Question :** le jeton et l'appareil sont-ils ceux d'avant ?
- **Pression :** contrôles négatifs de la vraie commande, pour les deux issues.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-26 · Une panne injectée doit être constatée avant de juger la réaction
- **Ce qui arrive :** sur un jeu à téléphones, la clé `storage` ne corrompait rien, et C9 passait sans avoir touché la sauvegarde.
- **D'où on le sait :** commit `a26c55c` ; `CERTIFICATION.md:424-425` (`rtc` : les compteurs établissent que les fautes ont eu lieu).
- **Mécaniques :** toutes.
- **Question :** la faute a-t-elle eu lieu ?
- **Pression :** le harnais refuse `storage` dès que `phone` est déclaré ; compteurs de pannes de `rtc`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-27 · Chaque détecteur a son contrôle négatif, qui doit échouer
- **Ce qui arrive :** un détecteur qui ne sait pas rougir donne un vert sans valeur.
- **D'où on le sait :** `CERTIFICATION.md:219` (fermer côté serveur ne satisfait pas l'oracle de `silent`), `414-416` (retenir du DOM et 48 Mio de WASM doit dépasser les budgets) ; `tools/cert/text.mjs:171-176` ; `tools/cert/resources.mjs:63` ; `outil/tests/negative_controls.rs`.
- **Mécaniques :** toutes.
- **Question :** a-t-on vu ce contrôle échouer sur une faute injectée ?
- **Pression :** les contrôles négatifs cités.
- **Sous la nouvelle forme :** pression du commun. Le contrat l'exige : un contrôle doit savoir échouer.
- **Étage :** commun.

### OUT-28 · Une condition préalable ratée compte contre l'item qu'elle sert
- **Ce qui arrive :** `jitter.ready` était lue puis ignorée. Un jeu déjà sorti du salon lançait ses sondes sur un écran sans la commande : R1 restait non observé, et l'échec tombait sur E1.
- **D'où on le sait :** commit `c10ea3d`.
- **Mécaniques :** latence, phases.
- **Question :** quand la préparation échoue, quel item le dit ?
- **Pression :** le harnais fait avancer la partie jusqu'à `ready`, et R1 échoue s'il n'y arrive pas.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-29 · Une progression se prouve par une fin réelle puis une reprise
- **Ce qui arrive :** trois changements d'écran ou un cycle A↔B passaient pour une partie qui avance.
- **D'où on le sait :** `CERTIFICATION.md:363-372` ; `tools/cert/progression-verification.mjs:1-3` ; commit `c7b507f` (le modèle continu ne montrait aucune progression finie).
- **Mécaniques :** phases, manches.
- **Question :** les bots atteignent-ils l'écran de fin, puis une nouvelle partie ?
- **Pression :** `tools/cert/progression-example.test.mjs`, contre un jeu qui finit et un qui tourne entre ses manches.
- **Sous la nouvelle forme :** pression du commun. Les bots du jeu parcourent toutes ses phases.
- **Étage :** commun.

### OUT-30 · L'oracle de rendu lit l'effet précis de l'action
- **Ce qui arrive :** une horloge, un compteur d'images ou n'importe quelle mutation du DOM changent sans que l'action ait eu d'effet, et donnent une latence inventée.
- **D'où on le sait :** `CERTIFICATION.md:117` ; `tools/cert/performance.mjs:181-183`, `244-246`.
- **Mécaniques :** latence.
- **Question :** la valeur lue est-elle la projection affichée de ce geste-là ?
- **Pression :** une action sans effet rend la sonde rouge.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-31 · Un effet finit en revenant ou en se reposant
- **Ce qui arrive :** `rtc` attendait que la valeur revienne à son point de départ, ce que fait un saut et jamais un score. Le jeu de départ échouait toujours C1.
- **D'où on le sait :** commit `0849044` ; `tools/cert/performance.mjs:275`, `289`.
- **Mécaniques :** latence.
- **Question :** l'oracle accepte-t-il un effet qui reste ?
- **Pression :** une valeur qui ne se repose jamais échoue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-32 · Un texte se juge au repos
- **Ce qui arrive :** une vue tout juste dessinée peut être dans la première image de son entrée, à l'opacité 0. WebKit y tombait à chaque fois, et T2 de Punchline s'arrêtait au premier cas.
- **D'où on le sait :** commit `280ac99` ; `tools/cert/text.mjs:127-131`.
- **Mécaniques :** affichage, texte.
- **Question :** l'oracle attend-il la fin des animations finies avant de lire ?
- **Pression :** ce qui finit caché échoue toujours.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-33 · Un `requestAnimationFrame` ne prouve pas que le GPU dessine
- **Ce qui arrive :** sur une TV en canvas, la boucle du navigateur tourne même quand rien n'est présenté.
- **D'où on le sait :** `CERTIFICATION.md:115` ; `archive/CHANTIERS-2026-10.md:235`.
- **Mécaniques :** rendu GPU.
- **Question :** le compteur lu compte-t-il des présentations réussies ?
- **Pression :** `painted`, lu depuis les événements post-rendu (`window.__certPaints`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-34 · `console.error` et `console.assert` d'une page sont des échecs
- **Ce qui arrive :** une erreur écrite en console passait inaperçue. Mais le navigateur écrit lui-même deux erreurs quand le harnais coupe le réseau (`Failed to load resource…`, `WebSocket connection to … failed`).
- **D'où on le sait :** `CERTIFICATION.md:64-69` ; `tools/cert/browser.mjs:227` ; `CHANGELOG.md`, v0.4.0, étape 3.
- **Mécaniques :** toutes.
- **Question :** le jeu écrit-il en erreur ce qui n'en est pas une ?
- **Pression :** l'oracle commun des trois invariants : aucune exception, aucune panique de l'hôte, la TV dessine.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-35 · Une mesure ne conclut que si sa cause a eu lieu pendant elle
- **Ce qui arrive :** un test donnait 250 ms au navigateur ; une machine lente les dépensait avant d'analyser le document, et la ressource était demandée après l'échéance. `hostile` lancé seul comptait la première connexion des téléphones comme une reconnexion. Une fermeture mesurée sur un socket déjà fermé donnait une durée négative.
- **D'où on le sait :** commit `79a6cc0` ; `tools/cert/cert.mjs:1555-1557`, `1669-1671`.
- **Mécaniques :** toutes.
- **Question :** la cause mesurée est-elle postérieure au début de la mesure ?
- **Pression :** un essai dont l'échéance précède sa cause ne conclut rien et double son échéance, dans un onglet neuf.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-36 · Un test n'affirme jamais la vitesse de la machine
- **Ce qui arrive :** un test de cadence exigeait 100 ms entre deux images. En priorité d'arrière-plan, Chrome en met 200 avec Node éveillé.
- **D'où on le sait :** commits `79a6cc0`, `f9049f2`.
- **Mécaniques :** toutes.
- **Question :** ce test affirme-t-il ce que le retard pourrait casser, ou la rapidité du runner ?
- **Pression :** les assertions portent sur le temps du navigateur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## L'identité d'un passage

### OUT-37 · On empreinte les entrées avant et après, et un changement pendant le passage le fait échouer
- **Ce qui arrive :** une source modifiée pendant un passage pouvait se mélanger à ses résultats.
- **D'où on le sait :** `VERIFICATION.md:245-253` ; commit `36a6605`.
- **Mécaniques :** toutes.
- **Question :** les sources, dépendances, configurations, outils et paramètres sont-ils les mêmes à la fin ?
- **Pression :** `verification_observer.rs` (`a_real_source_changed_during_a_run_fails_the_real_command_and_is_named`). L'échec nomme les fichiers.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-38 · Dans le doute, un fichier est une entrée
- **Ce qui arrive :** un fichier suivi nommé `._x` ou `x~` était jeté comme bruit ; « note » n'importe où dans un nom écartait `footnotes.txt` ; un `git check-ignore` qui échoue, dans un sous-module, risquait de cacher un dossier.
- **D'où on le sait :** commits `36a6605`, `e4af95b`, `dfcd33e` ; `outil/src/verification/identity.rs:386-403`, `428-429`.
- **Mécaniques :** toutes.
- **Question :** ce fichier est-il exclu par une règle sûre, ou par une devinette ?
- **Pression :** `verification_observer.rs` (`a_followed_noise_or_a_near_note_changed_during_a_run_fails_it_by_name`, `noise_written_during_a_run_does_not_invalidate_it`). Une fausse alerte coûte moins qu'un angle mort.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-39 · Un `Cargo.lock` périmé se rafraîchit avant l'empreinte, jamais en mode verrouillé
- **Ce qui arrive :** le premier passage échouait : Cargo réécrivait le lock pendant le passage, et l'empreinte le voyait changer.
- **D'où on le sait :** commit `36a6605` ; `outil/src/verification/identity.rs:65-68`.
- **Mécaniques :** toutes.
- **Question :** l'outil fait-il les écritures de Cargo avant de prendre son empreinte ?
- **Pression :** `verification_observer.rs` (`a_stale_lock_is_refreshed_before_the_snapshot_of_an_ordinary_run`, `a_locked_run_still_refuses_a_stale_lock_and_never_rewrites_it`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-40 · L'absence d'un outil fait partie des entrées
- **Ce qui arrive :** un `check` refusait de s'identifier sans Node, dont seul le harnais a besoin.
- **D'où on le sait :** commit `debb481` ; `outil/src/verification/identity.rs:674-678`.
- **Mécaniques :** toutes.
- **Question :** un passage sans tel outil a-t-il une identité différente d'un passage avec ?
- **Pression :** `outil/tests/without_node.rs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-41 · Ce qui est servi et ce qui observe entrent dans l'empreinte
- **Ce qui arrive :** des fichiers servis générés pendant le passage, ou un bundle d'observateur réduit à son point d'entrée, échappaient à l'identité.
- **D'où on le sait :** `outil/src/verification/harness.rs:61-62` ; `outil/src/certify.rs:64` ; `VERIFICATION.md:327-328`.
- **Mécaniques :** toutes.
- **Question :** l'empreinte couvre-t-elle tout ce que le navigateur a reçu et tout le code qui a jugé ?
- **Pression :** préparation avant l'empreinte.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-42 · Une empreinte établit un contexte, pas la qualité de l'oracle
- **Ce qui arrive :** une provenance complète peut donner l'illusion d'un résultat juste ou signé.
- **D'où on le sait :** `VERIFICATION.md:262-267`.
- **Mécaniques :** toutes.
- **Question :** que l'empreinte ne garantit-elle pas ?
- **Pression :** aucune ; c'est une limite écrite.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-43 · Un cache ne restaure jamais un résultat
- **Ce qui arrive :** un rapport ou un binaire restaurés pouvaient passer pour une observation fraîche ; une chaîne de version n'atteste pas une origine.
- **D'où on le sait :** `VERIFICATION.md:291-316` ; `outil/regles/ci.yml:57-73` (version exacte vérifiée après restauration).
- **Mécaniques :** toutes.
- **Question :** ce résultat a-t-il été produit par ce passage ?
- **Pression :** le CLI du commun est toujours réinstallé depuis le checkout ; le SHA-256 de l'archive Verus est vérifié même sur un cache présent.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-44 · Un `[patch]` local compile contre un commun que personne d'autre n'a
- **Ce qui arrive :** chez l'hôte, un jeu passe contre le salon local, et ne compile pas chez un ami.
- **D'où on le sait :** `VERIFICATION.md:255-260` ; `AGENTS.md` du dossier de l'hôte.
- **Mécaniques :** toutes.
- **Question :** ce jeu passe-t-il depuis un clone sans patch, contre l'étiquette publiée ?
- **Pression :** `soiree check --locked --published` ; une source patchée compte par son contenu réel.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-45 · Sous un `[patch]`, Cargo écrit un lock qui casse ailleurs
- **Ce qui arrive :** Cargo enregistre les crates communes sans `source` ; un lock committé ainsi fait échouer `cargo update --package salon-core` chez un ami. Une étape d'étiquette lancée dans `games/` écrivait ce lock.
- **D'où on le sait :** commits `dc1e6f5`, `673b36d` ; `outil/src/upgrade.rs:61-63` ; `outil/tests/negative_controls.rs:475-479`.
- **Mécaniques :** toutes.
- **Question :** le lock committé nomme-t-il la source de chaque crate commune ?
- **Pression :** `public_upgrade_gives_back_the_sources_a_patched_lock_lost`, dans un dossier qu'aucun `.cargo/config.toml` ne couvre, sans quoi le test ne prouve rien.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-46 · `git push --follow-tags` ne pousse que les étiquettes annotées
- **Ce qui arrive :** une étiquette légère restait locale, et l'étiquette nommée par les jeux n'existait pas sur GitHub.
- **D'où on le sait :** commit `3913388`.
- **Mécaniques :** aucune.
- **Question :** l'étiquette est-elle annotée ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-47 · On répète la montée de version avant que l'étiquette existe
- **Ce qui arrive :** avant l'étiquette, `soiree upgrade` refuse et `soiree check` s'arrête à la version : rien ne disait si les jeux passeraient.
- **D'où on le sait :** commit `31d57fb` ; `tools/rehearse-upgrade.py:1-16`.
- **Mécaniques :** toutes.
- **Question :** chaque jeu passe-t-il `check --locked --published` contre la future étiquette ?
- **Pression :** un faux amont temporaire, des clones hors du dossier de l'hôte. Limite : il ne dit rien du lock écrit sous le `[patch]`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-48 · Un jeu ne contourne pas la provenance du commun
- **Ce qui arrive :** vendoriser une crate du commun comme membre du workspace, retirer la version du jeu, ou changer d'organisation pouvait sauter les règles de CI, l'interface et la publication.
- **D'où on le sait :** `outil/src/check.rs:40-42` ; `outil/tests/negative_controls.rs` (`published_cannot_drop_the_game_version_to_bypass_ci_and_ui`, `custom_game_organisation_never_redirects_shared_dependencies_or_ci`) ; `CHANGELOG.md`, v0.2.0.
- **Mécaniques :** toutes.
- **Question :** le commun que le jeu utilise est-il bien celui de l'étiquette ?
- **Pression :** les contrôles négatifs cités.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Le harnais et les navigateurs

### OUT-49 · Le harnais attaque en boîte noire, sans cas propre à un jeu
- **Ce qui arrive :** un cas propre à un jeu dans le harnais aurait affaibli l'épreuve pour tous, et fait douter qu'elle soit la même.
- **D'où on le sait :** `tools/cert/cert.mjs:9-12` ; `AGENTS.md` du commun, « Les règles et le harnais » ; `archive/CHANTIERS-2026-10.md:59`.
- **Mécaniques :** toutes.
- **Question :** l'épreuve est-elle la même pour tous les jeux ?
- **Pression :** le harnais ne connaît un jeu que par `cert.json`, inspiré de web-platform-tests.
- **Sous la nouvelle forme :** pression du commun. L'épreuve ne connaît que la forme d'un jeu.
- **Étage :** commun.

### OUT-50 · Le WebKit de Playwright n'est pas Safari sur iPhone
- **Ce qui arrive :** il attrape les écarts de CSS et d'API, pas la suspension d'iOS, ni un vrai Wi-Fi.
- **D'où on le sait :** `CERTIFICATION.md:27-28`, `49` ; `archive/CHANTIERS-2026-10.md:128-134`.
- **Mécaniques :** téléphones.
- **Question :** ce comportement a-t-il été vu sur un vrai iPhone ?
- **Pression :** l'étage 3, `REPETITION.md`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-51 · Le WebKit exact de la version de Playwright est requis
- **Ce qui arrive :** une autre révision de WebKit déjà installée ne se lance pas avec ce Playwright.
- **D'où on le sait :** `outil/src/doctor.rs:417-418` ; `CERTIFICATION.md:49`.
- **Mécaniques :** toutes.
- **Question :** l'outil vérifie-t-il la révision exacte ?
- **Pression :** `soiree doctor`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-52 · Geler une page demande le débogueur
- **Ce qui arrive :** `Page.setWebLifecycleState` laisse tourner les minuteurs d'une page visible. Seul `Debugger.pause` arrête tout le JavaScript, comme iOS au verrouillage, et seulement dans Chromium, seul à exposer CDP. Dans Chrome 156, le banc ne savait pas produire un vrai dégel.
- **D'où on le sait :** `tools/cert/browser.mjs:437-439` ; commit `5392a03` ; `CERTIFICATION.md:205` ; `recherche/reconnexion.toml`, T8.
- **Mécaniques :** reconnexion.
- **Question :** le gel simulé arrête-t-il vraiment les minuteurs ?
- **Pression :** scénario `freeze` sur un téléphone Chromium.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-53 · Les onglets d'arrière-plan sont bridés
- **Ce qui arrive :** Chrome bride les minuteurs des onglets cachés. Le harnais désactive ce bridage pour ses téléphones ; la TV, elle, doit le subir et rester juste (S5).
- **D'où on le sait :** `tools/cert/browser.mjs:150-151` ; `CERTIFICATION.md:339` (S5).
- **Mécaniques :** temps réel, toutes.
- **Question :** la partie qui fait foi continue-t-elle quand son onglet passe derrière ?
- **Pression :** S5, humain. Le contrat demande désormais une épreuve par navigateur visé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-54 · WebKit n'offre pas d'API publique de glissement tactile
- **Ce qui arrive :** dans WebKit, les taps restent tactiles, mais un glissement passe par un drag à la souris. Un tap préparatoire aurait activé deux fois un bouton.
- **D'où on le sait :** `tools/cert/browser.mjs:417-421` ; `CERTIFICATION.md:205`.
- **Mécaniques :** téléphones, tactile.
- **Question :** ce geste simulé exerce-t-il le toucher, ou seulement le pointeur ?
- **Pression :** aucune pour le vrai toucher d'iOS.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-55 · WebKit diffère l'accusé d'un survol jusqu'au rendu suivant
- **Ce qui arrive :** un drag qui attendait l'accusé du mouvement avant l'appui se bloquait quand le rendu attendait une feuille CSS.
- **D'où on le sait :** `CERTIFICATION.md:207-216` ; `tools/cert/browser.mjs:383-386` ; commit `8a20dbf`.
- **Mécaniques :** tactile, souris.
- **Question :** le pilote envoie-t-il mouvement et appui dans l'ordre, puis attend-il les deux ?
- **Pression :** un test natif qui retient une feuille CSS.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-56 · Sur WebKit, Retour arrière hors d'un champ navigue dans l'historique
- **Ce qui arrive :** le singe tapait Retour arrière sur un téléphone et quittait la page.
- **D'où on le sait :** `CERTIFICATION.md:138` ; `tools/cert/performance.mjs:344`.
- **Mécaniques :** téléphones, texte libre.
- **Question :** une touche simulée a-t-elle un sens hors d'un champ éditable ?
- **Pression :** le singe ne la génère qu'avec un champ texte au focus.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-57 · L'événement WebSocket de l'inspecteur WebKit ne date pas la tentative
- **Ce qui arrive :** il n'arrive qu'à la fin d'un upgrade avalé ; sa date n'est pas celle où la page a essayé.
- **D'où on le sait :** `tools/cert/browser.mjs:84-85`, `217` ; `CERTIFICATION.md:219`.
- **Mécaniques :** reconnexion.
- **Question :** l'observateur voit-il les appels de la page elle-même ?
- **Pression :** le harnais observe les appels natifs `WebSocket` avant les scripts du jeu.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-58 · Une navigation abandonnée continue de courir
- **Ce qui arrive :** sa réponse 500 tardive était attribuée au `goto` suivant.
- **D'où on le sait :** commit `79a6cc0` ; `tools/cert/browser.mjs:263-264`.
- **Mécaniques :** toutes.
- **Question :** chaque requête observée appartient-elle à l'action qui l'a émise ?
- **Pression :** un `goto` ne possède que les requêtes qu'il a vu partir.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-59 · Retirer un script d'initialisation ne défait pas ce que l'ancien document a installé
- **Ce qui arrive :** un stockage interdit par des getters non configurables restait interdit dans le document courant après le retrait du script.
- **D'où on le sait :** `tools/cert/browser.mjs:297-298` ; `tools/cert/cert.mjs:1216-1218`.
- **Mécaniques :** sauvegardes.
- **Question :** le cas suivant part-il d'un document neuf ?
- **Pression :** `tools/cert/storage.test.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-60 · Les compteurs CDP appartiennent à un isolat partagé
- **Ce qui arrive :** des onglets du même site peuvent partager le tas et le DOM que CDP rapporte.
- **D'où on le sait :** `tools/cert/resources.mjs:47`.
- **Mécaniques :** mémoire.
- **Question :** cette mesure est-elle bien celle d'une seule page ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-61 · Réécrire une réponse avec Playwright peut copier des en-têtes périmés
- **Ce qui arrive :** passer la réponse d'origine à `fulfill` copie ses en-têtes d'entité avant le nouveau corps, compressé ou en flux.
- **D'où on le sait :** `tools/cert/udp.mjs:8-9`.
- **Mécaniques :** aucune.
- **Question :** le corps réécrit garde-t-il des en-têtes qui ne lui vont plus ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-62 · Une faute d'un scénario reste dans ce scénario
- **Ce qui arrive :** un rejet non géré d'un gestionnaire de route, sur une connexion réinitialisée, a terminé tout le passage : 22 points jamais atteints.
- **D'où on le sait :** commit `e5ff52e` ; `tools/cert/udp.mjs:129-130`.
- **Mécaniques :** toutes.
- **Question :** une erreur de l'outil peut-elle arrêter les scénarios suivants ?
- **Pression :** l'offre échoue pour la page, et le relais dit pourquoi.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-63 · Deux pages ne partagent pas `performance.now()`
- **Ce qui arrive :** soustraire les `performance.now()` de deux pages donne un écart sans sens ; chaque page a son origine de temps.
- **D'où on le sait :** `tools/cert/performance.mjs:1-4` ; `CERTIFICATION.md:114`.
- **Mécaniques :** latence.
- **Question :** les deux horloges sont-elles ramenées à la même époque ?
- **Pression :** `performance.timeOrigin + performance.now()`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-64 · Chaque cas part d'un état neuf
- **Ce qui arrive :** deux négociations refusées laissaient leurs échecs dans le `sessionStorage` de l'onglet ; le cas suivant partait en WebSocket et ne trouvait pas de pair. La page avait raison. Les commandes de contrôle partageaient aussi les places du parcours principal.
- **D'où on le sait :** commit `af3199a` ; `tools/cert/rtc-negotiation.mjs:3-4` ; `VERIFICATION.md:231-233` ; `tools/cert/text.mjs:200`.
- **Mécaniques :** toutes.
- **Question :** ce cas peut-il lire ce qu'a laissé le précédent ?
- **Pression :** chaque cas oublie l'onglet ; chaque commande reçoit un hôte frais par `{url}`. Un cas à part éprouve la mémoire voulue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-65 · La TV s'ouvre à l'adresse de l'hôte, les téléphones par le réseau
- **Ce qui arrive :** un jeu peut réserver des droits d'organisateur à sa propre origine ; un téléphone ouvert en local ne passe pas par le chemin réel.
- **D'où on le sait :** `tools/cert/cert.mjs:443-444`, `471`.
- **Mécaniques :** téléphones, réseau.
- **Question :** chaque appareil simulé arrive-t-il par le même chemin qu'un vrai ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-66 · Après un redémarrage, un DOM d'avant trompe l'oracle
- **Ce qui arrive :** l'accueil du transport peut précéder la nouvelle vue du jeu ; lire l'ancien DOM classait mal une place vacante.
- **D'où on le sait :** `tools/cert/cert.mjs:1162-1164`.
- **Mécaniques :** reconnexion.
- **Question :** l'oracle lit-il la vue du nouvel hôte ?
- **Pression :** le scénario `restart` repart de documents neufs.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-67 · Une salle garde un moment la place d'un appareil perdu
- **Ce qui arrive :** un téléphone au stockage refusé arrive en appareil neuf, pendant que le jeu garde sa place (90 s dans le salon de Punchline). L'asseoir aussi mettait N + 1 joueurs sur N places, et le dernier trouvait la salle pleine.
- **D'où on le sait :** `tools/cert/cert.mjs:1181-1187`.
- **Mécaniques :** places, identité.
- **Question :** un appareil qui perd son identité peut-il faire déborder la salle ?
- **Pression :** scénario `storage` : chaque téléphone doit retrouver une place à la fin.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-68 · Le harnais dépendait de POSIX
- **Ce qui arrive :** `lsof`, `ps` et les groupes de processus n'existent pas sous Windows ; `ps -M` ne liste les threads que sous macOS.
- **D'où on le sait :** commits `5e9bf2b`, `a8a1e6a`, `a26c55c` ; `tools/cert/toxiproxy.mjs:28-30` ; `tools/cert/cert.mjs:380`.
- **Mécaniques :** toutes.
- **Question :** l'outil sait-il dire qu'il ne peut pas tourner ici, avant de tenter ?
- **Pression :** `outil/tests/windows_refusal.rs`. Rien n'a été exécuté sous Windows.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-69 · Des ports libres en TCP et en UDP, sans retenter un jeu échoué
- **Ce qui arrive :** un port libre en TCP ne dit rien du port UDP que le même hôte doit ouvrir. Il reste la course entre libération et ouverture.
- **D'où on le sait :** `CERTIFICATION.md:391-393` ; `tools/cert/ports.mjs:6-7`.
- **Mécaniques :** réseau.
- **Question :** le port choisi est-il libre pour les deux protocoles ?
- **Pression :** `tools/cert/ports.test.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-70 · Lire la sortie d'un hôte pour savoir qu'il écoute est fragile
- **Ce qui arrive :** certains jeux s'annoncent avant d'écouter ; les lignes de progression de Cargo ne sont pas celles du jeu.
- **D'où on le sait :** `tools/cert/cert.mjs:304`, `335`.
- **Mécaniques :** aucune.
- **Question :** sans objet.
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète. Le commun lance et enferme lui-même le module : il n'a plus à lire une ligne `ready` dans la sortie d'un programme du jeu.
- **Étage :** commun.

### OUT-71 · Décoder les paquets à la main dans l'outil, c'est une seconde définition qui dérive
- **Ce qui arrive :** le harnais décodait l'accusé de salon à la main ; l'accusé a grandi, et un vrai envoi se lisait comme aucun.
- **D'où on le sait :** commit `f281791` ; `tools/cert/cert.mjs:102-104` ; `tools/cert/text.mjs:57-60`.
- **Mécaniques :** réseau.
- **Question :** l'outil lit-il les messages par les mêmes types que le commun ?
- **Pression :** un test d'octets dans salon tenait les deux à une forme.
- **Sous la nouvelle forme :** impossible par la forme. Le contrat, « Les types de bout en bout » : les messages sont des types Rust partagés, et l'épreuve tourne sur le même module.
- **Étage :** commun.

## Pannes injectées et réseau simulé

### OUT-72 · Le chemin du soir doit être celui qu'on éprouve
- **Ce qui arrive :** Toxiproxy ne porte que TCP. Le harnais forçait `?salon=ws`, et WebRTC, le chemin réel, n'était éprouvé que par le faux réseau de salon. Il a fallu un relais UDP possédé qui perturbe les vrais datagrammes DTLS et SCTP.
- **D'où on le sait :** `CERTIFICATION.md:27`, `418-425` ; `archive/CHANTIERS-2026-10.md:219` ; `tools/cert/udp.mjs:1`.
- **Mécaniques :** réseau.
- **Question :** la panne est-elle injectée sous le transport réellement utilisé ?
- **Pression :** scénario `rtc` ; `salon/tests/rtc-browser.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-73 · Un proxy qui avale transmet quand même la fermeture
- **Ce qui arrive :** Toxiproxy, en avalant, laisse passer la fermeture décidée par l'hôte. Un vrai Wi-Fi mort ne la transmet jamais. Une page qui ne réagit qu'à la fermeture passait le test, et échouait le soir.
- **D'où on le sait :** `CERTIFICATION.md:219` ; `tools/cert/toxiproxy.mjs:6-19` ; commit `5392a03`.
- **Mécaniques :** réseau, reconnexion.
- **Question :** la panne simulée se comporte-t-elle comme la panne réelle, y compris dans ce qu'elle ne dit pas ?
- **Pression :** `silent` n'avale que ce que l'hôte envoie ; la page doit appeler elle-même `close()` puis rouvrir en moins de 20 s.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-74 · Toxiproxy 2.13 se connecte à l'amont sans échéance
- **Ce qui arrive :** son `net.Dial` ne s'annule pas ; un SYN bloqué retenait la commande de coupure jusqu'au délai du système.
- **D'où on le sait :** `CERTIFICATION.md:221-230` ; `tools/cert/toxiproxy.mjs:38-43`.
- **Mécaniques :** réseau.
- **Question :** l'outil de pannes a-t-il lui-même des échéances bornées ?
- **Pression :** un connecteur possédé : deux secondes, 128 connexions au plus, annulé à l'arrêt, sans nouvelle tentative ni repli vers la boucle locale.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-75 · Le trou noir se joue : lien ouvert, rien ne passe
- **Ce qui arrive :** un client sans chien de garde reste content d'un lien mort. C'est le cas d'un iPhone qui se verrouille ou d'un routeur qui se fige.
- **D'où on le sait :** `recherche/telecommande.toml`, leçon 06 (Toxiproxy) ; `tools/cert/toxiproxy.mjs:9-12`.
- **Mécaniques :** réseau.
- **Question :** chaque côté découvre-t-il seul un silence ?
- **Pression :** `blackhole` et `deafen` du harnais ; le banc de la télécommande.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-76 · Un banc sans débit, tampon ni gros message ne voit pas les pannes de fragmentation
- **Ce qui arrive :** le banc de salon perdait et réordonnait, mais n'avait ni débit borné, ni tampon qui jette, ni gros message. Trois trous graves du flux fiable le traversaient.
- **D'où on le sait :** `recherche/fiabilite.toml`, T7 et leçons 35, 46, 50.
- **Mécaniques :** réseau.
- **Question :** le réseau simulé a-t-il un débit, un tampon borné et des rafales ?
- **Pression :** 24 000 cas d'un chaos à tuyau étroit. Supposé : quels débit et tampon valent un vrai Wi-Fi.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-77 · Le chaos mêle toutes les pannes, pas seulement celles du réseau
- **Ce qui arrive :** le chaos ne connaissait qu'un réseau : rechargements, onglets jumeaux et redémarrages de l'hôte passaient au travers, et trois trous graves avec eux.
- **D'où on le sait :** `recherche/reconnexion.toml`, T12 et leçon 35 (Jepsen) ; commit `46e0ef7`.
- **Mécaniques :** réseau, reconnexion.
- **Question :** le chaos joue-t-il chaque panne du catalogue, mélangées ?
- **Pression :** le chaos de salon mêle rechargements, jumeaux, « Jouer ici », pages mortes en silence, redémarrages et paquets retenus.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-78 · Une garantie se juge au repos, après guérison
- **Ce qui arrive :** juger en pleine panne confond un retard passager et une perte.
- **D'où on le sait :** `recherche/reconnexion.toml`, leçon 36 (Jepsen).
- **Mécaniques :** réseau.
- **Question :** après les pannes, a-t-on guéri, attendu, puis vérifié ?
- **Pression :** le chaos de salon.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-79 · Les pannes qui comptent sont choisies, pas tirées au hasard
- **Ce qui arrive :** une gigue uniforme ne produit pas le paquet précis retenu puis livré trop tard, ni l'enregistrement forgé qui casse une pile DTLS.
- **D'où on le sait :** `recherche/reconnexion.toml`, leçon 38 (turmoil) ; `recherche/transport-webrtc.toml`, leçon 47 (dimpl).
- **Mécaniques :** réseau.
- **Question :** le banc sait-il retenir un paquet nommé, ou forger un message ?
- **Pression :** les paquets retenus du chaos ; `salon/fuzz/`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-80 · Une panne touche aussi ce qui est déjà en vol
- **Ce qui arrive :** une coupure simulée qui n'atteint que les envois futurs épargne les paquets partis.
- **D'où on le sait :** `recherche/fiabilite.toml`, leçon 45 (turmoil) ; `tools/cert/udp.mjs:71`.
- **Mécaniques :** réseau.
- **Question :** la coupure retient-elle ce qui attendait déjà ?
- **Pression :** le relais UDP du harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-81 · Deux copies de notre pile qui se parlent ne disent rien d'un navigateur
- **Ce qui arrive :** une panique de str0m sur ce que Chromium et WebKit envoient traversait tous les tests du dépôt.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, leçon 01 et T10.
- **Mécaniques :** réseau.
- **Question :** la pile est-elle éprouvée contre les vrais navigateurs ?
- **Pression :** `rtc` sur Chromium et WebKit. Firefox n'a pas été éprouvé : la version de Playwright installée n'a pas ouvert de canal par le relais, cause inconnue.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-82 · Le hasard réduit au plus petit cas, puis gardé comme test fixe
- **Ce qui arrive :** une suite aléatoire d'opérations trouve mieux que des cas écrits ; un cas trouvé une fois doit être rejoué d'abord.
- **D'où on le sait :** `recherche/fiabilite.toml`, leçons 47 et 48 (proptest) ; `CERTIFICATION.md:26`.
- **Mécaniques :** toutes.
- **Question :** chaque échec trouvé par le hasard est-il devenu un test fixe ?
- **Pression :** les propriétés de chaque jeu ; `found_bugs.rs` de salon.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-83 · L'hôte entier se simule sur une horloge simulée
- **Ce qui arrive :** chaque pièce testée seule laissait sans test la boucle de l'hôte : jeu, salon, reconnexions et échéances. C'est là que vit « le téléphone revient pile quand la manche finit ».
- **D'où on le sait :** `archive/CHANTIERS-2026-10.md:120-126` (inspiré de FoundationDB et TigerBeetle).
- **Mécaniques :** toutes.
- **Question :** le commun peut-il jouer des milliers de parties en un processus, rejouables au bit près ?
- **Pression :** campagnes de 2048 graines dans chaque jeu.
- **Sous la nouvelle forme :** pression du commun. Le jeu pur et ses bots le permettent pour tous.
- **Étage :** commun.

### OUT-84 · Une fuite dépend de ce qui arrive, pas de la durée
- **Ce qui arrive :** une endurance en temps réel est lente et lit mal la mémoire du serveur. En temps simulé, la soirée compte l'octet vivant.
- **D'où on le sait :** commit `503b386` ; `CHANGELOG.md`, v0.4.0, « Ce qu'une soirée laisse en mémoire ».
- **Mécaniques :** mémoire.
- **Question :** la mémoire vivante se stabilise-t-elle sur une soirée simulée de six heures ?
- **Pression :** `salon_host::simulation::evening` : 10 min simulées par tour dans `cargo test`, 60 avec `SOIREE_EVENING_MINUTES=60`. Elle a trouvé dans Punchline et Sidequest un téléphone parti gardé en mémoire toute la soirée.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-85 · La mémoire d'un hôte se juge à sa pente, pas à un rapport
- **Ce qui arrive :** R2 comparait la minute 5 et la fin en rapport : Punchline échouait à 3 → 5 Mio, du bruit d'allocateur.
- **D'où on le sait :** commit `6197cc8` ; `tools/cert/cert.mjs:2075-2077` ; `CERTIFICATION.md:325`.
- **Mécaniques :** mémoire.
- **Question :** la pente projetée sur six heures reste-t-elle sous le budget, et les threads cessent-ils de croître ?
- **Pression :** `--soak 30`, temps monotone, cinq minutes de chauffe, séries brutes gardées.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-86 · Les ressources du navigateur se mesurent après collecte, à des frontières répétées
- **Ce qui arrive :** DOM, listeners, tas JS, mémoires WASM vivantes et RSS de Chromium grandissent par cycles de jeu. Le RSS englobe le GPU sans mesurer la VRAM.
- **D'où on le sait :** `CERTIFICATION.md:327`, `397-405` ; `tools/cert/resources.mjs:1-2`.
- **Mécaniques :** mémoire, rendu.
- **Question :** quatre mesures après collecte, sur trois cycles, restent-elles bornées ?
- **Pression :** scénario `resources`. Les seuils sont des détecteurs, pas une preuve d'absence de fuite.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Mesures et budgets

### OUT-87 · Un budget se lit à la résolution de sa mesure
- **Ce qui arrive :** une image sautée à 60 Hz dure 33,33 ms, que Chrome rapporte à 33,3 ou 33,4 ms : le budget de 33 ms échouait sur l'image qu'il nomme. Ajouter un vsync entier aurait accepté une image de 45 ms, vraie lenteur à 120 Hz. La cadence d'un échantillon court a une résolution d'une demi-image par seconde.
- **D'où on le sait :** commit `96173cf` ; `tools/cert/performance.mjs:20-23`, `31-32` ; `CERTIFICATION.md:118`.
- **Mécaniques :** rendu.
- **Question :** la tolérance est-elle la résolution de la mesure, et non une marge ?
- **Pression :** `tools/cert/performance.test.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-88 · `requestAnimationFrame` mesure la présentation, pas le photon
- **Ce qui arrive :** la cadence rAF ne dit ni le temps GPU, ni l'arrivée de la lumière sur la TV.
- **D'où on le sait :** `tools/cert/performance.mjs:1-4` ; `CERTIFICATION.md:118` ; `REPETITION.md:82`.
- **Mécaniques :** latence, rendu.
- **Question :** quelle part de la latence cette mesure ne voit-elle pas ?
- **Pression :** au salon : filmer au ralenti le téléphone et la TV dans le même cadre.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-89 · Une machine chargée fausse les mesures sans un mot
- **Ce qui arrive :** une compilation à côté de la certification retardait clics et images et faisait de faux échecs.
- **D'où on le sait :** commit `671ee7f` ; `tools/cert/load.mjs:1-10`.
- **Mécaniques :** latence, rendu.
- **Question :** la machine était-elle libre pendant la mesure ?
- **Pression :** charge mesurée avant et après, avertissement à partir de la moitié des cœurs. Le verdict ne change pas ; une mesure illisible est inconnue, jamais une machine au repos. `verification_observer.rs` (`a_loaded_machine_is_said_without_changing_the_verdict`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-90 · Sur batterie faible, Chrome bride tout à 30 Hz
- **Ce qui arrive :** sous 20 % de batterie, l'économiseur de Chrome limite tous les rendus à 30 Hz, même une page vide.
- **D'où on le sait :** `CERTIFICATION.md:120-124` (lien vers `battery_saver_mode_manager.cc` de Chromium) ; `REPETITION.md:9`.
- **Mécaniques :** rendu.
- **Question :** la mesure a-t-elle été faite sur secteur ?
- **Pression :** aucune ; consigne.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-91 · Le reçu du navigateur fait foi, et on ne retente pas une mesure
- **Ce qui arrive :** un retard de la boucle de Node transformait une mesure déjà terminée à temps en délai dépassé.
- **D'où on le sait :** `tools/cert/performance.mjs:121-123`, `143-144` ; `CHANGELOG.md`, v0.4.0, « Certification ».
- **Mécaniques :** rendu.
- **Question :** l'échec vient-il du rendu ou de l'observateur ?
- **Pression :** on récupère le même échantillon immuable, jamais un nouvel essai ni un budget plus long.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-92 · L'écart final compte quand le rendu s'arrête
- **Ce qui arrive :** si le rendu du jeu s'arrête pendant que la boucle de l'observateur tourne, l'échantillon semblait sain.
- **D'où on le sait :** `tools/cert/performance.mjs:155-156`.
- **Mécaniques :** rendu.
- **Question :** l'écart entre la dernière image présentée et la fin de l'échantillon est-il compté ?
- **Pression :** il compte comme un blocage, pas comme une image de plus.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-93 · La cadence se mesure aussi en jeu, sous des gestes réels
- **Ce qui arrive :** un salon immobile tient 60 images par seconde ; la partie animée, avec huit téléphones qui jouent, peut ne pas les tenir.
- **D'où on le sait :** `CERTIFICATION.md:118` ; `tools/cert/cert.mjs:1817-1822`, `910-911`.
- **Mécaniques :** rendu.
- **Question :** l'échantillon en jeu exclut-il gels, redimensionnements et captures ?
- **Pression :** `frames_lobby` et `frames_gameplay`, avec le nombre de gestes pendant la fenêtre.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-94 · Une échéance qui expire doit dire ce qu'elle voyait
- **Ce qui arrive :** un passage de Sidequest a échoué C1 avec le seul délai nu de Playwright, et l'échec n'est pas revenu en neuf rejeux.
- **D'où on le sait :** commit `1f30437`.
- **Mécaniques :** toutes.
- **Question :** un échec rare laisse-t-il de quoi le comprendre la prochaine fois ?
- **Pression :** le délai porte la dernière valeur, combien de temps elle a tenu, et le nombre d'images lues.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Les mutations

### OUT-95 · Quand l'agent écrit le code et les tests, les mutations vérifient les tests
- **Ce qui arrive :** des tests peuvent passer sans rien vérifier. Un mutant qui survit est du code dont aucun test ne voit le changement.
- **D'où on le sait :** `archive/CHANTIERS-2026-10.md:105-118`.
- **Mécaniques :** toutes.
- **Question :** quels changements du code aucun test ne remarque-t-il ?
- **Pression :** `soiree mutants`, sur `crates/core` par défaut, ou sur les seules lignes changées (`--in-diff`).
- **Sous la nouvelle forme :** pression du commun. Étape « Injecter des bugs » de l'épreuve.
- **Étage :** commun.

### OUT-96 · Les mutations donnent un rapport sans seuil
- **Ce qui arrive :** un seuil aurait poussé à écrire des tests pour le chiffre. Seuls un échec de l'outil ou de la base non mutée sont des erreurs.
- **D'où on le sait :** `outil/src/mutants.rs:1-5` ; `.github/workflows/nuit.yml:73-74`.
- **Mécaniques :** toutes.
- **Question :** un humain a-t-il lu les survivants ?
- **Pression :** rapport `certification/mutants/rapport.md`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-97 · Les tests écrits par agent laissaient un tiers des mutants vivants
- **Ce qui arrive :** une campagne réelle sur `salon-core` laissait 141 mutants vivants sur 433 : accusés, renvois, renumérotation et échéances n'avaient aucun test qui les voie manquer. La campagne suivante en laissait 11 sur 453.
- **D'où on le sait :** `CHANGELOG.md`, v0.4.0, « Nuit, mutations, tests ».
- **Mécaniques :** réseau, toutes.
- **Question :** combien de mutants survivent dans le code le plus critique ?
- **Pression :** `client_edges.rs`, `host_edges.rs`, `stream_edges.rs`, `types_edges.rs` de salon.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-98 · On mute une copie, et la copie garde la configuration de l'hôte
- **Ce qui arrive :** muter les fichiers de l'auteur est dangereux. Une copie perd la configuration Cargo des dossiers parents, dont le `[patch]` local. `cargo-mutants` copie l'espace de travail seul, et une crate qui va chercher `../remote` ne compile plus.
- **D'où on le sait :** `outil/src/mutants.rs:268-270` ; `.github/workflows/nuit.yml:48-49`.
- **Mécaniques :** toutes.
- **Question :** la copie mutée compile-t-elle comme l'original ?
- **Pression :** la campagne de nuit vérifie qu'il ne reste qu'un paquet dans l'espace isolé.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-99 · Une sélection vide n'est pas un résultat tronqué
- **Ce qui arrive :** pour une sélection vide, `cargo-mutants` n'écrit que `mutants.json`. Inventer des zéros pour un résultat manquant aurait caché une campagne interrompue.
- **D'où on le sait :** `outil/src/mutants.rs:132-134`, et la lecture qui vérifie que les agrégats s'additionnent et que la fin est datée.
- **Mécaniques :** toutes.
- **Question :** la campagne a-t-elle fini, et ses nombres sont-ils cohérents ?
- **Pression :** aucune séparée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-100 · Une exclusion de mutant porte sa raison
- **Ce qui arrive :** exclure sans raison, ou par la configuration native de l'outil, cache du code non testé.
- **D'où on le sait :** `outil/src/mutants.rs`, aide de la commande (`.cargo/soiree-mutants.toml`).
- **Mécaniques :** toutes.
- **Question :** pourquoi ce code est-il exclu ?
- **Pression :** raison non vide obligatoire ; `.cargo/mutants.toml` ignoré.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-101 · Un outil lent ne pourrit pas s'il est exercé à chaque changement du commun
- **Ce qui arrive :** une commande lancée seulement à la demande casse sans qu'on le voie.
- **D'où on le sait :** `archive/CHANTIERS-2026-10.md:113-114` ; `AGENTS.md` du commun.
- **Mécaniques :** toutes.
- **Question :** les tests du commun font-ils tourner une vraie petite campagne ?
- **Pression :** trois mutations ciblées du modèle, toutes détectées, dans les tests de l'outil.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-102 · Retirer chaque correctif mesure le chaos
- **Ce qui arrive :** une affirmation sur ce que voyait le chaos datait d'avant un changement. Remesuré, retirer la garde de T3 demandait plus de 3 000 cas, et l'ancien chaos restait vert à 12 000.
- **D'où on le sait :** commits `08d278e`, `46e0ef7`, `08ed767`, `f2100bd` ; `recherche/reconnexion.toml`, T12.
- **Mécaniques :** réseau, reconnexion.
- **Question :** combien de cas le chaos met-il à rougir quand on retire un correctif ?
- **Pression :** retirer seul le correctif de T1, T6 ou du hello tardif rougit à 64 cas ; celui de T2 en moins de 3 000.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-103 · Une soirée ne voit que ce qu'elle emprunte
- **Ce qui arrive :** sept retraits de code dans salon ont mesuré la soirée simulée. Les quatre qui faisaient grandir la mémoire n'étaient vus qu'avec des pages rechargées, de gros messages et un téléphone qui ne perd jamais son lien. La version courte de `cargo test` manque les plus petites fuites.
- **D'où on le sait :** `CHANGELOG.md`, v0.4.0 ; commit `503b386`.
- **Mécaniques :** mémoire.
- **Question :** l'endurance passe-t-elle par les chemins qui retiennent ?
- **Pression :** la soirée de six heures dans la CI de salon et avant une étiquette.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Les preuves Verus

### OUT-104 · La preuve porte sur le code de production
- **Ce qui arrive :** un modèle réécrit à côté prouve un autre programme que celui qui décide.
- **D'où on le sait :** `outil/verification-kernel/README.md:3-6`, `85-88` ; `outil/verification-kernel/verify.py:22-25`.
- **Mécaniques :** toutes.
- **Question :** le code prouvé est-il celui que le CLI compile ?
- **Pression :** le script refuse `assume`, `admit`, `assume_specification` et `external_body`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-105 · Une preuve ne répond qu'à sa question
- **Ce qui arrive :** le noyau prouve ce que la politique finie conclut de faits donnés. Il ne prouve ni les parseurs, ni l'applicabilité, ni les empreintes, ni les adaptateurs, ni la vérité des observations, ni les règles d'un jeu.
- **D'où on le sait :** `outil/verification-kernel/README.md:31-35`, `92-94` ; `VERIFICATION.md:278-283`.
- **Mécaniques :** toutes.
- **Question :** quelle partie du chemin la preuve ne couvre-t-elle pas ?
- **Pression :** aucune ; limite écrite.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-106 · L'agrégation ne cache jamais une obligation non résolue
- **Ce qui arrive :** un résumé qui additionne des réussites peut masquer un inconnu. Un pli vide, ou fait de seules exclusions, peut fabriquer une réussite.
- **D'où on le sait :** `outil/verification-kernel/README.md:22-29`.
- **Mécaniques :** toutes.
- **Question :** l'agrégat garde-t-il le pire résultat non résolu ?
- **Pression :** Verus prouve que `combine` est commutatif, associatif, idempotent, et qu'un pli vide part de « sans objet ».
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-107 · Un mutant de preuve n'est détecté que par une erreur de vérification
- **Ce qui arrive :** une erreur de syntaxe ou un outil absent font aussi échouer Verus, sans rien prouver sur la spécification.
- **D'où on le sait :** `outil/verification-kernel/verify.py:53-55` ; `outil/verification-kernel/README.md:69-74`.
- **Mécaniques :** toutes.
- **Question :** le mutant est-il rejeté par les postconditions, et seulement par elles ?
- **Pression :** quatre mutants : preuve périmée acceptée, contrôle non exécuté réussi, obligation non résolue perdue, contrôle inachevé qui laisse passer.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-108 · Test exhaustif et preuve sont deux méthodes distinctes
- **Ce qui arrive :** les tests parcourent les 486 combinaisons de faits, les 36 paires et les 216 triplets de verdicts. Ils ne deviennent pas une preuve en le disant.
- **D'où on le sait :** `outil/verification-kernel/README.md:51-54`, `80-81`.
- **Mécaniques :** toutes.
- **Question :** quelle méthode le rapport attribue-t-il à ce résultat ?
- **Pression :** méthodes rapportées séparément.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-109 · La preuve fait partie de l'expérience, même quand un test suffit
- **Ce qui arrive :** Verus a été retiré : il couvrait trente lignes qu'un test parcourait en entier, et aucun bug n'en était venu. Le mainteneur l'a remis le jour même : savoir jusqu'où vont les garanties sur le code fait partie de ce qu'on teste.
- **D'où on le sait :** commits `dca71b3`, `6c4e2eb`.
- **Mécaniques :** toutes.
- **Question :** où le commun permet-il dès maintenant d'exiger une preuve ?
- **Pression :** la CI de Verus.
- **Sous la nouvelle forme :** connaissance. Le contrat en garde l'esprit : le commun permet d'exiger une preuve.
- **Étage :** connaissance.

### OUT-110 · Le vérificateur est figé et ne s'installe jamais en douce
- **Ce qui arrive :** une autre version de Verus peut accepter ou refuser autrement ; un téléchargement implicite change ce qu'on prouve.
- **D'où on le sait :** `outil/verification-kernel/README.md:39-42`, `58-62`, `76-81`.
- **Mécaniques :** toutes.
- **Question :** quelle version exacte, quel binaire, quel hash ?
- **Pression :** vérificateur absent, mauvais ou expiré (60 s) : panne d'infrastructure. Verus n'est pas requis pour compiler un jeu.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Vitesse des contrôles

### OUT-111 · Ordres de grandeur mesurés de l'ancienne boucle
- **Ce qui arrive :** mesures locales, caches chauds, v0.3.0 : boucle statique de 0,9 à 3,3 s ; vérification du code de 1,1 à 33,6 s selon le projet ; première vérification d'un jeu neuf 93,4 s ; certification complète dans le navigateur de 148 à 313 s.
- **D'où on le sait :** `git show 0acc4e3:certification/contracts-v0.3.0/README.md` et `validation.json`.
- **Mécaniques :** toutes.
- **Question :** combien coûte chaque étage de la boucle ?
- **Pression :** aucune ; des mesures conservées.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-112 · On ne promet aucune durée sans mesure conservée
- **Ce qui arrive :** caches, mode rapide et parallélisme semblaient accélérer, sans que rien ne le mesure. Le mode rapide réduit le travail ; il ne sait pas quels contrôles un changement touche.
- **D'où on le sait :** `VERIFICATION.md:214-218`, `314-316` ; `AGENTS.md` du commun.
- **Mécaniques :** toutes.
- **Question :** cette promesse de vitesse a-t-elle une mesure ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-113 · Rien de lent ne bloque la boucle serrée
- **Ce qui arrive :** mutations, longues campagnes de graines, soirée de six heures et fuzzing prennent des minutes ou des heures.
- **D'où on le sait :** `archive/CHANTIERS-2026-10.md:227` ; `.github/workflows/nuit.yml:1-3` ; `AGENTS.md` du commun, « Avant d'étiqueter ».
- **Mécaniques :** toutes.
- **Question :** ce contrôle accompagne-t-il chaque changement, ou tourne-t-il à côté ?
- **Pression :** la nuit, sur des machines Linux gratuites, jamais sur un push ; avant une étiquette.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-114 · Une CI sur macOS coûte cher
- **Ce qui arrive :** le workflow tournait sur macOS à chaque push, sur des dépôts privés qui partagent environ 190 minutes macOS par mois, à 6 à 13 minutes le passage.
- **D'où on le sait :** commit `f166420` ; `outil/regles/ci.yml:3-5`.
- **Mécaniques :** toutes.
- **Question :** quel contrôle a vraiment besoin d'un Mac ?
- **Pression :** déclenchement à la main ou sur une pull request.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-115 · Des tests de navigateur concurrents se gênent
- **Ce qui arrive :** des fixtures natives indépendantes se disputaient le runner.
- **D'où on le sait :** commit `8a20dbf`.
- **Mécaniques :** toutes.
- **Question :** les tests de navigateur tournent-ils à part des compilations ?
- **Pression :** un job séparé, fichiers l'un après l'autre.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-116 · La recherche ne bloque qu'à l'étiquette
- **Ce qui arrive :** une dette de recherche qui bloquerait chaque changement étoufferait la vitesse ; un jeu n'a pas à attendre une fiche du commun.
- **D'où on le sait :** `recherche/README.md:25` ; `outil/src/recherche/dette.rs:1-3` ; `outil/tests/recherche.rs` (`a_game_is_never_stopped_by_research`, `a_broken_sheet_only_costs_a_line_in_the_tight_loop`).
- **Mécaniques :** toutes.
- **Question :** où ce contrôle bloque-t-il, et où informe-t-il seulement ?
- **Pression :** une ligne de dette dans `--fast` et dans un jeu.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Les vrais appareils

### OUT-117 · Ce qu'aucun simulateur ne reproduit reste un jugement humain
- **Ce qui arrive :** suspension d'iOS, vrai Wi-Fi, recul du canapé, plaisir : rien de cela ne sort d'une machine.
- **D'où on le sait :** `CERTIFICATION.md:18-28`, `329-344` ; `REPETITION.md:3`.
- **Mécaniques :** toutes.
- **Question :** qu'est-ce que seul un vrai appareil peut dire de ce jeu ?
- **Pression :** la fiche de répétition, 15 minutes de gestes puis une partie.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-118 · Un essai réel nomme ses appareils, son contexte, sa personne et ce qu'elle a vu
- **Ce qui arrive :** un compte rendu vague ne se rejoue pas et ne se compare pas.
- **D'où on le sait :** `VERIFICATION.md:285-289` ; `REPETITION.md:5` (date, jeu, commits, Wi-Fi).
- **Mécaniques :** toutes.
- **Question :** quel modèle, quel OS, quel transport, quelle durée ?
- **Pression :** la fiche.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-119 · Après une suspension ou une coupure, le téléphone revient seul à sa place
- **Ce qui arrive :** verrouiller 60 s, passer 30 s dans une autre app, couper le Wi-Fi 10 s : un écran figé, un nom ou un score perdu, un rechargement à la main ou plus de 10 s pour revenir sont des échecs.
- **D'où on le sait :** `CERTIFICATION.md:335-337` (S1 à S3) ; `REPETITION.md:26`.
- **Mécaniques :** téléphones, reconnexion.
- **Question :** le joueur a-t-il eu quelque chose à faire ?
- **Pression :** S1 à S3, chronomètre en main.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-120 · Le mode économie d'énergie ne change rien
- **Ce qui arrive :** un téléphone en économie d'énergie peut ralentir ses minuteurs et son réseau.
- **D'où on le sait :** `CERTIFICATION.md:338` (S4).
- **Mécaniques :** téléphones.
- **Question :** le jeu tient-il avec un téléphone en économie d'énergie ?
- **Pression :** S4, humain.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-121 · Tout le monde joue : Safari, l'app et Chrome Android
- **Ce qui arrive :** un jeu essayé dans un seul navigateur casse dans un autre.
- **D'où on le sait :** `CERTIFICATION.md:341` (S7) ; `REPETITION.md:30` (moins de 30 s chacun, sans compte, pas de salle fantôme).
- **Mécaniques :** téléphones.
- **Question :** chaque navigateur visé rejoint-il en moins de 30 s ?
- **Pression :** S7, humain ; C1 dans le harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-122 · La TV se lit depuis le canapé
- **Ce qui arrive :** à 3 m, un texte trop petit, des accents serrés ou un bord rogné par la TV ne se voient qu'en vrai.
- **D'où on le sait :** `CERTIFICATION.md:342` (S8) ; `REPETITION.md:64` (les accents du tableau de Sidequest).
- **Mécaniques :** affichage TV, texte.
- **Question :** tout est-il lisible à 3 m, et rien n'est-il coupé ?
- **Pression :** S8, humain ; T3 dans le harnais pour la zone sûre.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### OUT-123 · Le premier son joue dès le premier geste, depuis la TV
- **Ce qui arrive :** le navigateur bloque le son avant un geste. Le premier geste est sur le Mac, et le son vient de la TV, jamais des téléphones.
- **D'où on le sait :** `CERTIFICATION.md:343` (S9) ; `REPETITION.md:31`.
- **Mécaniques :** son.
- **Question :** quand arrive le premier son, et d'où ?
- **Pression :** S9, humain.
- **Sous la nouvelle forme :** pression du commun. Le commun possède le déblocage du son.
- **Étage :** commun.

### OUT-124 · Seuls des joueurs disent si c'est drôle
- **Ce qui arrive :** aucune machine ne juge le plaisir. On note les rires, les flottements, et un mot de chacun.
- **D'où on le sait :** `CERTIFICATION.md:344` (S10) ; `REPETITION.md:39`.
- **Mécaniques :** toutes.
- **Question :** où la partie s'essouffle-t-elle ?
- **Pression :** S10, humain.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-125 · Le pare-feu de macOS coupe l'adresse réseau d'un binaire neuf
- **Ce qui arrive :** la fenêtre sur les connexions entrantes revient après chaque compilation. Sans « Autoriser », un jeu n'a rien renvoyé sur l'adresse réseau du Mac alors que `localhost` répondait ; TCP est coupé comme UDP, et il n'y a rien à replier. Le modèle neuf a dû être certifié en boucle locale.
- **D'où on le sait :** `REPETITION.md:18`, `100` ; `recherche/transport-webrtc.toml`, leçon 03 et note de coût ; commit `23b6cf6` ; `git show 0acc4e3:certification/contracts-v0.3.0/README.md`.
- **Mécaniques :** réseau.
- **Question :** l'outil reconnaît-il un pare-feu, et le nomme-t-il ?
- **Pression :** `certify` signale le pare-feu quand l'adresse du réseau ne répond pas ; le contrôle LAN produit H1, et aucune fausse erreur du jeu.
- **Sous la nouvelle forme :** pression du commun. Le commun vérifie que les appareils se rejoignent, et dit quoi faire.
- **Étage :** commun.

### OUT-126 · Un passage en boucle locale laisse le réseau local non attesté
- **Ce qui arrive :** passer par `127.0.0.1` isole l'interface de la disponibilité du LAN, mais ne dit rien du chemin réel.
- **D'où on le sait :** `VERIFICATION.md:319-328`.
- **Mécaniques :** réseau.
- **Question :** ce passage a-t-il traversé le vrai réseau ?
- **Pression :** le choix est inscrit dans le périmètre ; aucun repli automatique vers la boucle locale.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-127 · Le simulateur iOS n'applique pas le refus du réseau local
- **Ce qui arrive :** couper « Réseau local » pour l'app ne change rien sur simulateur. Le message qui nomme Réglages n'a jamais été vu sur un iPhone.
- **D'où on le sait :** commit `0e7054d` ; `recherche/telecommande.toml`, T5 ; `REPETITION.md:38` (S6).
- **Mécaniques :** app iPhone.
- **Question :** le refus a-t-il été joué sur un vrai iPhone ?
- **Pression :** S6, humain. Le banc tient la notice et la lecture de `NWConnection`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-128 · Le simulateur n'a ni capteurs ni scanner, et ses touchers sont trop parfaits
- **Ce qui arrive :** le banc voit le côté changer, pas un braquage ; il ne lit pas un vrai QR ; son toucher de 4 ms est un cas limite ; le crochet de test de WebKit tue un processus, ce n'est pas une mort par mémoire.
- **D'où on le sait :** `recherche/telecommande.toml`, T4, T8, T11, T14.
- **Mécaniques :** app iPhone, capteurs, tactile.
- **Question :** ce comportement dépend-il d'un capteur, d'un pouce ou d'une mémoire réelle ?
- **Pression :** les gestes de `REPETITION.md`, « L'app Télécommande, sur un vrai iPhone ».
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-129 · Une coquille native sans banc passe toutes les suites
- **Ce qui arrive :** rien n'exécutait le code Swift de l'app : chaque trou de sa fiche passait chaque suite.
- **D'où on le sait :** commit `32c1964` ; `recherche/telecommande.toml`, T18 et leçon 36 (idb).
- **Mécaniques :** app iPhone.
- **Question :** le code natif d'une app est-il exécuté par un test ?
- **Pression :** `remote/bench` : les vraies sources sur un simulateur, contre un faux jeu qui coupe, gèle ou se tait, et contre un vrai salon. Il échoue plutôt que de sauter là où il ne peut pas tourner, et reste hors du workspace pour que `soiree check remote` n'exige pas de simulateur.
- **Sous la nouvelle forme :** pression du commun. Une app passe les mêmes pressions qu'une page.
- **Étage :** commun.

### OUT-130 · Un banc tactile attend que l'écran se pose
- **Ce qui arrive :** le redimensionnement de la première rotation soulève tout doigt posé avant : un test devenait instable.
- **D'où on le sait :** commit `f36fb4e`.
- **Mécaniques :** app iPhone, tactile.
- **Question :** le banc attend-il la fin des rotations avant de poser un doigt ?
- **Pression :** le banc attend que le pad se stabilise.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-131 · Safari sur iPhone n'a jamais été mesuré sur un vrai Wi-Fi de soirée
- **Ce qui arrive :** sans permission de capture, un navigateur peut ne partir que par la route par défaut. Sur un Wi-Fi sans internet avec le cellulaire actif, ce n'est peut-être pas l'interface de l'hôte : chaque iPhone paierait 3 s par connexion.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T9 et leçon 42 (RFC 8828) ; `REPETITION.md:43`.
- **Mécaniques :** réseau, téléphones.
- **Question :** quel transport l'iPhone prend-il, cellulaire actif puis coupé ?
- **Pression :** le geste de `REPETITION.md`. Supposé, jamais mesuré.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-132 · L'horloge monotone d'un iPhone peut ne pas compter le sommeil
- **Ce qui arrive :** sur Darwin, `performance.now()` lit une horloge qui peut s'arrêter pendant le verrouillage. Le chien de garde ne voyait pas le silence et mettait jusqu'à 6 s.
- **D'où on le sait :** `recherche/reconnexion.toml`, T5 et leçons 12, 33, 34 ; commit `b5f6b0d`.
- **Mécaniques :** temps, reconnexion.
- **Question :** le temps que lit l'appareil compte-t-il le sommeil ?
- **Pression :** `SleepClock` ajoute ce que l'horloge murale a gagné. À mesurer sur un iPhone : retour près de 6 s dans S1.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-133 · On ne règle pas le Wi-Fi à l'aveugle
- **Ce qui arrive :** une soirée dure plus de 30 minutes ; l'app ne déclare pas qu'elle a besoin du Wi-Fi. Rien ne dit qu'un réglage aveugle ne ferait pas pire.
- **D'où on le sait :** `recherche/telecommande.toml`, T17 et leçons 34, 35 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** app iPhone, réseau.
- **Question :** la latence monte-t-elle par à-coups dans une longue partie ?
- **Pression :** à mesurer d'abord (`REPETITION.md`, T17).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-134 · Un réveil coupe un lien encore vivant
- **Ce qui arrive :** après 1,5 s de silence, `wake` détruit un lien valide sans lire ce qui attendait dans le socket. Rien n'est perdu, mais une reconnexion a lieu.
- **D'où on le sait :** `recherche/reconnexion.toml`, T9 et leçon 44 ; `REPETITION.md:41`.
- **Mécaniques :** reconnexion.
- **Question :** le joueur voit-il cette reconnexion, et le gêne-t-elle ?
- **Pression :** sonde sur copie avec un vrai gel de 3 s ; le coût visible reste à juger sur de vrais téléphones.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-135 · Les réglages de stick se font au pouce
- **Ce qui arrive :** 12 % de zone morte et pleine vitesse dès 90 % de la course sont des valeurs de départ.
- **D'où on le sait :** `REPETITION.md:77`.
- **Mécaniques :** stick, manette.
- **Question :** la marche lente et la pleine vitesse tombent-elles juste sous le pouce ?
- **Pression :** aucune ; jugement humain.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### OUT-136 · Un pad en TCP et une page en WebRTC ne vivent pas la même perte
- **Ce qui arrive :** sur TCP, une perte retient tout ce qui suit. Une perte de 200 ms pourrait lâcher le personnage un instant, pouce immobile. Le code le laisse supposer, rien ne l'a mesuré.
- **D'où on le sait :** `REPETITION.md:79` ; `recherche/telecommande.toml`, leçon 44.
- **Mécaniques :** stick, réseau.
- **Question :** le stick natif tarde-t-il sur la page, sur un Wi-Fi qui perd ?
- **Pression :** deux iPhones côte à côte, Network Link Conditioner, profil *Very Bad Network*.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-137 · Un appui très bref peut disparaître
- **Ce qui arrive :** un tap le plus court possible sur la croix, dans Safari puis dans l'app, peut ne jamais atteindre le jeu.
- **D'où on le sait :** `REPETITION.md:81` (TR-03 reste non revu pour cette raison) ; `recherche/telecommande.toml`, T8.
- **Mécaniques :** tactile.
- **Question :** un appui plus court qu'un envoi atteint-il le jeu ?
- **Pression :** le cœur Rust de la télécommande tient l'appui jusqu'à l'état suivant ; sur la page, à juger.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-138 · La page dans l'app et la bascule vers le pad se jugent sur un vrai iPhone
- **Ce qui arrive :** prénom prérempli, portrait gardé, retour du personnage après une fermeture forcée, temps de bascule et de rotation, brouillon perdu, taille des libellés en paysage : tout vient de la lecture du code et d'un faux client.
- **D'où on le sait :** `REPETITION.md:67-76`.
- **Mécaniques :** app iPhone, texte libre.
- **Question :** que manque-t-il au joueur pendant la bascule ?
- **Pression :** aucune ; un KO y est une décision, pas un item cassé.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### OUT-139 · Une partie se calibre sur sa durée réelle
- **Ce qui arrive :** 30 questions de 30 s font 15 minutes sans les résultats ; on ignore où ça s'essouffle, et si les joueurs réclament un compte à rebours.
- **D'où on le sait :** `REPETITION.md:63-65`.
- **Mécaniques :** quiz, manches.
- **Question :** à quelle question l'attention baisse-t-elle ?
- **Pression :** S10.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### OUT-140 · La répétition se prépare
- **Ce qui arrive :** batterie, compilation en parallèle, partage de connexion ou réseau invité faussent ce qu'on observe. Une app signée avec un compte Apple gratuit expire après 7 jours.
- **D'où on le sait :** `REPETITION.md:9-16`.
- **Mécaniques :** toutes.
- **Question :** le matériel est-il celui de la vraie soirée ?
- **Pression :** la liste de la veille : `soiree status`, `soiree doctor`, commits notés, pare-feu autorisé pour chaque binaire.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-141 · Ce qui casse en jouant devient un test, et dit pourquoi aucun item ne l'a vu
- **Ce qui arrive :** un bug vu le soir n'a de valeur que s'il se rejoue. On note heure, geste, appareil, photo, sortie du terminal ; une touche marque l'instant dans le journal.
- **D'où on le sait :** `REPETITION.md:88-90` ; `CERTIFICATION.md:43-45` (postmortems sans blâme).
- **Mécaniques :** toutes.
- **Question :** pourquoi aucune pression ne l'a-t-elle trouvé ?
- **Pression :** reproduire par un test, puis ajouter ou durcir un item. Un test n'est jamais supprimé sans dire ce qui le couvre.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-142 · Le même échec deux fois : on arrête de relancer
- **Ce qui arrive :** relancer un point qui échoue toujours brûle la soirée. Après une correction, il faut repasser le KO, S1, S3, S7, et toute la liste si le commun a changé.
- **D'où on le sait :** `REPETITION.md:100-101`.
- **Mécaniques :** toutes.
- **Question :** ce KO demande-t-il une correction plutôt qu'un nouvel essai ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les règles d'écriture et l'outil

### OUT-143 · Interdire `unwrap` ne suffit pas
- **Ce qui arrive :** le manifeste bannissait `unwrap`, mais laissait `.expect()`, `panic!` et l'index hors bornes arrêter la partie de la même façon.
- **D'où on le sait :** commit `75f0e2e` ; `outil/regles/cargo.toml:20-24`.
- **Mécaniques :** toutes.
- **Question :** quelles constructions arrêtent la partie sans dire `unwrap` ?
- **Pression :** Clippy en erreur, tests exemptés.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-144 · La liste des fonctions flottantes qui diffèrent entre WebAssembly et natif est longue
- **Ce qui arrive :** la première liste oubliait `asin`, `atan`, `tanh`, `log2`, `hypot`, `powi` et d'autres. La précision de `powi` n'est pas spécifiée.
- **D'où on le sait :** commit `75f0e2e` ; `outil/regles/clippy.toml:26-77`.
- **Mécaniques :** physique, temps réel.
- **Question :** la copie native donne-t-elle les mêmes flottants que le module ?
- **Pression :** `disallowed-methods` ; la comparaison native contre module de l'épreuve.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-145 · `Instant::now` panique dans WebAssembly sans système
- **Ce qui arrive :** sur `wasm32-unknown-unknown`, l'appel panique.
- **D'où on le sait :** `outil/regles/clippy.toml:24`.
- **Mécaniques :** temps.
- **Question :** sans objet.
- **Pression :** `disallowed-methods`.
- **Sous la nouvelle forme :** impossible par la forme. « La forme d'un jeu » : le temps arrive comme une entrée, et le module enfermé n'a pas d'horloge.
- **Étage :** commun.

### OUT-146 · Un décodeur refuse les octets restants
- **Ce qui arrive :** `postcard::from_bytes` accepte un message suivi d'octets en trop.
- **D'où on le sait :** `outil/regles/clippy.toml:17`.
- **Mécaniques :** réseau, sauvegardes.
- **Question :** le décodeur rejette-t-il ce qui reste après le message ?
- **Pression :** `salon_core::codec::decode` imposé par Clippy.
- **Sous la nouvelle forme :** pression du commun. Ce qui vient de l'extérieur est validé à l'entrée.
- **Étage :** commun.

### OUT-147 · Durcir est libre, assouplir s'écrit, et les règles se comparent par leur sens
- **Ce qui arrive :** comparer les fichiers de règles ligne à ligne refusait un durcissement, ou laissait passer un `allow` reformulé.
- **D'où on le sait :** `outil/src/lints.rs:1-5` ; `outil/regles/cargo.toml:17-19`.
- **Mécaniques :** toutes.
- **Question :** cette règle est-elle au moins aussi stricte que le socle ?
- **Pression :** `soiree check`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-148 · Les exemptions de test ne tiennent que sous un `cfg(test)` simple
- **Ce qui arrive :** les exemptions de Clippy accordées aux tests ne valent que dans un module sous `cfg(test)` simple ; ailleurs, le code de test tombe sous les règles de production.
- **D'où on le sait :** commit `6843233`.
- **Mécaniques :** toutes.
- **Question :** l'exemption s'applique-t-elle vraiment là où on la croit ?
- **Pression :** Clippy.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-149 · Chaque membre d'un workspace hérite des règles
- **Ce qui arrive :** une crate qui n'hérite pas des lints du workspace y échappe en entier.
- **D'où on le sait :** `outil/src/check.rs:227` ; `outil/src/lints.rs:200`.
- **Mécaniques :** toutes.
- **Question :** toutes les crates suivent-elles le socle ?
- **Pression :** `soiree check`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-150 · La simulation ne dépend ni du navigateur ni du GPU, et le JavaScript n'est que de la colle
- **Ce qui arrive :** une dépendance de navigateur ou de GPU dans `crates/core` empêche la simulation de tourner sans écran ; du JavaScript écrit à la main échappe à Rust.
- **D'où on le sait :** `outil/src/check.rs:29`, `266`, `418`.
- **Mécaniques :** toutes.
- **Question :** sans objet.
- **Pression :** `soiree check`, par liste de crates interdites.
- **Sous la nouvelle forme :** impossible par la forme. L'enfermement : le commun constate, sur le module, ce à quoi il a accès.
- **Étage :** commun.

### OUT-151 · Les règles voyagent dans l'outil de leur version
- **Ce qui arrive :** un script lisait les règles dans les blocs du manifeste : la doc et la configuration se mélangeaient.
- **D'où on le sait :** commits `52eaa7e`, `a6da2a7` ; `outil/src/embedded.rs:1-8`.
- **Mécaniques :** toutes.
- **Question :** un jeu est-il jugé sur les règles de sa version ?
- **Pression :** `soiree check` refuse une autre version.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-152 · Ce qu'une machine ne juge pas doit au moins s'écrire
- **Ce qui arrive :** des principes du manifeste n'avaient aucun contrôle. Chaque jeu disait, principe par principe : respecté, écart avec sa raison, ou à faire.
- **D'où on le sait :** commit `75f0e2e` ; `outil/src/gaps.rs:1-3`.
- **Mécaniques :** toutes.
- **Question :** chaque principe a-t-il sa réponse ?
- **Pression :** `soiree check` échoue sur un point ou une raison manquants, et liste les « à faire ».
- **Sous la nouvelle forme :** pression du commun. C'est la garantie d'examen.
- **Étage :** commun.

### OUT-153 · Les noms de la page sont des constantes Rust
- **Ce qui arrive :** un id, une classe ou une propriété CSS écrits en chaîne côté Rust divergent de la page sans que rien ne casse.
- **D'où on le sait :** `outil/src/dom.rs:1-22` ; `outil/regles/clippy.toml:79-105`.
- **Mécaniques :** page téléphone.
- **Question :** le Rust peut-il nommer ce que la page ne déclare pas ?
- **Pression :** `soiree dom` génère le module ; `soiree check` échoue s'il est périmé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-154 · Une machine neuve manque de ce que rustup n'installe pas
- **Ce qui arrive :** rustup n'installe pas d'éditeur de liens. Sur un Mac, `cc` existe sans les outils d'Apple, et l'appeler ouvre leur installeur. Le harnais demandait Node 22, pour le `WebSocket` global.
- **D'où on le sait :** commit `efa6b15` ; `outil/src/doctor.rs:22`, `98-99`.
- **Mécaniques :** aucune.
- **Question :** que manque-t-il à cette machine, avant la première compilation ?
- **Pression :** `soiree doctor`, qui n'installe rien.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les documents et les agents

### OUT-155 · Une phrase fausse devient du code faux chez chaque agent
- **Ce qui arrive :** le catalogue pointait un fichier généré, le contrat promettait ce que salon ne pouvait pas relayer, `layout: false` n'excluait rien, des clés n'étaient pas documentées, D5 nommait un contrôle inexistant.
- **D'où on le sait :** commit `6764852` ; `outil/src/docs.rs:1-4` ; `AGENTS.md` du commun, « La documentation ».
- **Mécaniques :** toutes.
- **Question :** ce fait a-t-il une seule source ?
- **Pression :** chaque fait a une source unique, les autres documents y renvoient.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-156 · Une machine vérifie les faits d'un document, pas son sens
- **Ce qui arrive :** versions, commandes, options, chemins, scénarios, clés et items cités dérivaient du code.
- **D'où on le sait :** commit `d44d0e7`, `d35786f` ; `outil/src/docs.rs:1-4` ; `AGENTS.md` du commun, « Avant d'étiqueter », étape 9.
- **Mécaniques :** toutes.
- **Question :** un agent neuf, qui ne connaît pas le changement, a-t-il relu la doc contre le code ?
- **Pression :** `soiree check .` lit les documents contre le code ; la relecture par un agent neuf reste humaine.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-157 · Une leçon notée « gardée » nomme ce qui la tient
- **Ce qui arrive :** `LEARNINGS.md` marquait gardée une leçon qui ne l'était pas : l'appui plus court qu'un envoi se perdait. La doc de `Host::forget` disait que le jeton cessait de marcher, et ce n'était pas vrai.
- **D'où on le sait :** commits `b7bae9f`, `0eae554` ; `recherche/telecommande.toml`, leçon 21.
- **Mécaniques :** toutes.
- **Question :** quel test, non ignoré, tient cette affirmation ?
- **Pression :** les fiches de recherche ferment un trou sur un test ou un item seulement.
- **Sous la nouvelle forme :** pression du commun. Une leçon seulement écrite ne compte jamais comme vérifiée.
- **Étage :** commun.

### OUT-158 · Un exemple et une consigne de migration se jouent
- **Ce qui arrive :** le `cert.json` d'exemple ne passait ni `check` ni `certify`. Les étapes de mise à jour pointaient vers des fichiers qu'un jeu n'a pas, et affirmaient un refus que `check` ne faisait pas.
- **D'où on le sait :** commits `5a39e48`, `c7b507f`.
- **Mécaniques :** toutes.
- **Question :** l'exemple donné aux agents passe-t-il lui-même l'épreuve ?
- **Pression :** la CI du modèle (`.github/scripts/model-ci.py`) ; un test joue l'exemple de progression.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-159 · Une consigne de version s'écrit pour un agent qui ne connaît rien
- **Ce qui arrive :** un agent de jeu suit les étapes sans le contexte du commun ; une étape vague se devine de travers.
- **D'où on le sait :** `AGENTS.md` du commun, « `CHANGELOG.md` » ; commit `c7b507f`.
- **Mécaniques :** toutes.
- **Question :** l'étape nomme-t-elle exactement ce qui change, avec le code avant et après ?
- **Pression :** aucune ; relecture.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-160 · Un document peut nommer ce qu'un clone neuf n'a pas encore
- **Ce qui arrive :** `soiree check .` échouait sur un clone : la doc citait des liaisons iOS générées, que Git ignore.
- **D'où on le sait :** commit `7c0ce7a` ; `outil/src/docs.rs:38-40`.
- **Mécaniques :** aucune.
- **Question :** ce chemin existe-t-il dans un clone neuf, ou après génération seulement ?
- **Pression :** une liste explicite de ce qui peut manquer.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## La recherche mesurable

### OUT-161 · Une citation se confronte à l'amont, par un autre programme
- **Ce qui arrive :** la fiche pilote a failli publier un lien mort, qu'aucune règle hors ligne ne voit. Des fiches citaient des commits locaux, introuvables sur GitHub.
- **D'où on le sait :** `outil/src/recherche/verifie.rs:1-4` ; commit `09ca63c` ; `recherche/reconnexion.toml`, `[cout]` (citations revérifiées par un script distinct du générateur).
- **Mécaniques :** toutes.
- **Question :** chaque commit cité est-il publié, chaque citation à la ligne de son lien ?
- **Pression :** `soiree recherche verifie`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-162 · Une panne de réseau ne conclut rien, et un portail Wi-Fi répond 2xx
- **Ce qui arrive :** sans `gh`, le 422 de GitHub pour un commit inconnu passait pour une panne ; toute réponse 2xx tenait un commit, y compris un portail captif.
- **D'où on le sait :** commits `35015c9`, `b342690` ; `outil/src/recherche/verifie.rs:6-8`, `286-287`, `366-367`.
- **Mécaniques :** toutes.
- **Question :** la réponse nomme-t-elle bien ce qu'on a demandé ?
- **Pression :** `outil/tests/recherche.rs` (`verifie_tells_an_outage_from_a_wrong_sheet`, `verifie_never_takes_a_captive_portal_for_a_commit`). Sortie 2 pour une panne.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-163 · On s'arrête quand trois sources de suite n'apprennent rien, et de natures variées
- **Ce qui arrive :** le pilote s'est arrêté trop tôt en comptant les sources seules. Trois normes ne disent rien du code, trois serveurs rien du client.
- **D'où on le sait :** `recherche/README.md:22` ; `outil/src/recherche/controle.rs:550-551`.
- **Mécaniques :** toutes.
- **Question :** les dernières sources lues couvrent-elles le réel et le client ?
- **Pression :** `soiree check .` juge la saturation.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-164 · On écrit ses hypothèses de trous avant de lire ailleurs, et l'histoire avant le code
- **Ce qui arrive :** lire d'abord les autres oriente ce qu'on cherche chez soi ; le code seul cache pourquoi il est ainsi.
- **D'où on le sait :** `recherche/README.md:16-18`.
- **Mécaniques :** toutes.
- **Question :** quelles hypothèses avait-on avant de lire les références ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-165 · Une sonde échoue, puis passe en changeant la seule cause supposée
- **Ce qui arrive :** un trou décrit sans preuve peut être faux, et un correctif mêlé à la preuve cache ce qui a vraiment changé.
- **D'où on le sait :** `recherche/README.md:21` ; commits `e6deddb`, `a60552b` (« Not fixed here »).
- **Mécaniques :** toutes.
- **Question :** qu'est-ce qui est vérifié, et qu'est-ce qui reste supposé ?
- **Pression :** `[[trou]]` sépare `verifie` et `suppose` ; le correctif vient dans un autre commit.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-166 · Un trou ne se ferme que sur un test non ignoré ou un item
- **Ce qui arrive :** un trou fermé sur un fichier, ou sur une ligne de code, passait. T8 n'avait qu'une ligne et aucun test : il a été rouvert.
- **D'où on le sait :** commit `011c8c5`.
- **Mécaniques :** toutes.
- **Question :** quelle garantie exécutable ferme ce trou ?
- **Pression :** `soiree check .` ; il dit qu'un test existe, pas qu'il tourne ni qu'il passe.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-167 · Une fiche creuse ne passe pas
- **Ce qui arrive :** douze références `["x"]` sans aucune leçon passaient. Un niveau inconnu du registre sautait quotas et saturation en silence.
- **D'où on le sait :** commits `011c8c5`, `5d8d40e` ; `outil/src/recherche/controle.rs:323-325`.
- **Mécaniques :** toutes.
- **Question :** ce qui est dit lu peut-il être retrouvé par un autre lecteur ?
- **Pression :** quotas de leçons par niveau ; `outil/tests/recherche.rs` (`a_hollow_sheet_stops_the_full_check`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-168 · Ce qui est dû part du plancher du registre
- **Ce qui arrive :** `niveau = 2` avec `raison_niveau = "x"` faisait qu'un sous-système de niveau 3 ne devait plus rien.
- **D'où on le sait :** commit `2b9e0aa` ; `outil/src/recherche/dette.rs:22-23`.
- **Mécaniques :** toutes.
- **Question :** l'auteur peut-il baisser ce qu'il doit en écrivant un autre niveau ?
- **Pression :** `outil/tests/recherche.rs` (`lowering_a_sheet_never_lowers_what_is_owed`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-169 · Le coût d'une fiche de niveau 3
- **Ce qui arrive :** 33 à 55 minutes d'agent, 23 à 29 références, 13 à 35 sondes exécutées. Les quotas sont calés sur un seul pilote : 28 références, 47 leçons.
- **D'où on le sait :** `[cout]` des quatre fiches ; `recherche/sous-systemes.toml:4-8`.
- **Mécaniques :** toutes.
- **Question :** combien coûte la recherche d'un sous-système ?
- **Pression :** aucune ; mesures.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-170 · Repérer le nouveau dans un diff est une aide, pas un verrou
- **Ce qui arrive :** lire des lignes plutôt que du code peut manquer quelque chose, et voir ce qui n'y est pas. Un bug échappé compte aussi, et le diff ne le voit pas.
- **D'où on le sait :** `outil/src/recherche/plan.rs:1-4` ; `recherche/README.md:13`.
- **Mécaniques :** toutes.
- **Question :** ce changement ajoute-t-il un fichier, une API publique, une dépendance, un délai ou un message ?
- **Pression :** `soiree recherche plan`, qui sort à 0.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Ce que les pressions ont trouvé

### OUT-171 · Un refus qui fait avancer le hasard change la partie
- **Ce qui arrive :** une réponse toute faite refusée faisait quand même avancer le générateur aléatoire de Punchline.
- **D'où on le sait :** commit `d0906bd` (propriétés de l'étage 1).
- **Mécaniques :** hasard.
- **Question :** sans objet.
- **Pression :** les propriétés « un événement refusé ne change rien » de chaque jeu.
- **Sous la nouvelle forme :** impossible par la forme. « Un refus ne change rien » : le jeu lit l'ancien état, et le hasard arrive comme une entrée.
- **Étage :** commun.

### OUT-172 · Les prénoms cassent : doublons, espaces invisibles, nom d'un rôle
- **Ce qui arrive :** un joueur pouvait s'appeler comme le présentateur ; des doublons trop longs et des doubles espaces invisibles passaient.
- **D'où on le sait :** commit `d0906bd`.
- **Mécaniques :** identité, prénoms.
- **Question :** deux prénoms qui se lisent pareil sont-ils refusés ?
- **Pression :** propriétés de Punchline ; corpus de textes vicieux (`tools/cert/nasty.json`).
- **Sous la nouvelle forme :** pression du commun. Le commun fournit des prénoms validés.
- **Étage :** commun.

### OUT-173 · Un joueur fantôme reste « connecté »
- **Ce qui arrive :** un joueur parti restait marqué connecté dans Punchline.
- **D'où on le sait :** commit `d0906bd`.
- **Mécaniques :** présence.
- **Question :** la présence affichée suit-elle les arrivées et départs réels ?
- **Pression :** propriétés de Punchline.
- **Sous la nouvelle forme :** pression du commun. Le jeu voit des appareils qui arrivent, partent et reviennent.
- **Étage :** commun.

### OUT-174 · Une sauvegarde abîmée s'affiche au lieu de paniquer
- **Ce qui arrive :** une couleur hors palette ou une tuile manquante faisaient paniquer la tier list au chargement ; les propriétés y ont trouvé trois bugs.
- **D'où on le sait :** commits `75f0e2e`, `d0906bd`.
- **Mécaniques :** sauvegardes.
- **Question :** que fait le jeu d'une sauvegarde invalide ?
- **Pression :** un test par cas ; C9 dans le harnais.
- **Sous la nouvelle forme :** pression du commun. Le commun conserve l'état, et valide ce qui entre.
- **Étage :** commun.

### OUT-175 · Une entrée NaN et un aller-retour énorme passent sans validation
- **Ce qui arrive :** Sidequest acceptait un bail d'entrée NaN, et affichait l'infini pour un aller-retour énorme. Les deux étaient gardés en tests ignorés.
- **D'où on le sait :** commit `7a6857e`.
- **Mécaniques :** temps réel, flottants.
- **Question :** un flottant venu de l'extérieur est-il borné et fini ?
- **Pression :** tests ignorés, à fermer.
- **Sous la nouvelle forme :** pression du commun. Ce qui vient de l'extérieur est validé à l'entrée.
- **Étage :** commun.

### OUT-176 · Un Wi-Fi mort laisse le socket ouvert
- **Ce qui arrive :** la page de Punchline ne voyait pas un Wi-Fi mort : le navigateur gardait le socket `OPEN`.
- **D'où on le sait :** commit `5392a03` (le harnais en boîte noire a trouvé ce que jouer n'avait pas vu).
- **Mécaniques :** réseau.
- **Question :** chaque côté prouve-t-il la vie du lien par ses propres messages ?
- **Pression :** un battement de l'hôte toutes les 2 s ; la page lâche le socket après 6 s de silence ; scénario `silent`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-177 · Un réaffichage efface ce qu'on tape
- **Ce qui arrive :** un prénom en cours de frappe était effacé par n'importe quel réaffichage, et un tap sur « C'est moi ! » avant l'ouverture du socket était perdu.
- **D'où on le sait :** commit `5392a03`.
- **Mécaniques :** texte libre, page téléphone.
- **Question :** un champ en cours d'édition garde-t-il son texte et sa sélection quand la vue change ?
- **Pression :** U1 `editing` ; le contrat de la page téléphone.
- **Sous la nouvelle forme :** pression du commun. Les champs de texte sont des composants du commun.
- **Étage :** commun.

### OUT-178 · Un téléphone qui inonde l'hôte retarde tous les écrans
- **Ce qui arrive :** un seul téléphone retardait chaque écran de plus de 6 s.
- **D'où on le sait :** commit `5392a03` ; `CERTIFICATION.md:281` (E3).
- **Mécaniques :** réseau.
- **Question :** un appareil peut-il ralentir les autres ?
- **Pression :** un budget de 20 messages par seconde et par connexion, rafales de 40 ; scénario `hostile`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-179 · Beaucoup de TV rognent leur bord
- **Ce qui arrive :** l'horloge, la plaque de manche et les boutons de Punchline étaient dans la bande que coupent beaucoup de TV.
- **D'où on le sait :** commit `5392a03` ; `CERTIFICATION.md:303` (T3, zone sûre des 90 % intérieurs).
- **Mécaniques :** affichage TV.
- **Question :** le texte reste-t-il dans la zone sûre, à chaque format ?
- **Pression :** scénario `layout` en 1280×720, 1920×1080, 3840×2160, 1280×1024, 2560×1080 et 900×1200.
- **Sous la nouvelle forme :** pression du commun. Le commun possède l'affichage.
- **Étage :** commun.

### OUT-180 · Après un redémarrage de l'hôte, chaque écran dit quoi faire
- **Ce qui arrive :** après un redémarrage, les joueurs de Punchline lisaient « La partie a recommencé » au lieu de « Choisis un prénom ». Un téléphone resté sur l'écran de jeu sans nouvelles est figé.
- **D'où on le sait :** commits `5392a03`, `7a6857e`.
- **Mécaniques :** reprise.
- **Question :** après le redémarrage, que voit chaque appareil ?
- **Pression :** scénario `restart` (C7).
- **Sous la nouvelle forme :** pression du commun. Une seule façon de repartir.
- **Étage :** commun.

### OUT-181 · Un champ de moins de 16 px fait zoomer iOS
- **Ce qui arrive :** Safari sur iPhone zoome sur un champ dont le texte est plus petit que 16 px, et la page se décale.
- **D'où on le sait :** `CERTIFICATION.md:304` (T4).
- **Mécaniques :** page téléphone, texte libre.
- **Question :** chaque champ a-t-il au moins 16 px, à l'arrivée et en partie ?
- **Pression :** scénario `layout`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-182 · Les numéros d'une page rechargée repartent sous ceux de l'hôte
- **Ce qui arrive :** une page rechargée numérotait ses états depuis 1 ; l'hôte gardait les anciens numéros et ignorait tout, en accusant réception. Sur Sidequest, le stick serait resté mort.
- **D'où on le sait :** `recherche/reconnexion.toml`, T1 et leçon 03 ; commit `72591fe`.
- **Mécaniques :** reconnexion.
- **Question :** la poignée de main réaligne-t-elle chaque compteur ?
- **Pression :** salon-core sur faux réseau ; chaos.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-183 · Deux onglets d'un même appareil ne gardent pas chacun leur compteur
- **Ce qui arrive :** après « Jouer ici », l'onglet repris numérotait sous ce que l'autre avait envoyé : ses événements étaient ignorés et accusés, sans erreur.
- **D'où on le sait :** `recherche/reconnexion.toml`, T2 et leçon 46 ; commit `62799ad`.
- **Mécaniques :** reconnexion, identité.
- **Question :** l'hôte distingue-t-il les sessions d'un même appareil ?
- **Pression :** salon-core ; chaos (rouge en moins de 3 000 cas sans le correctif).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-184 · Un hello sur un lien en fin de vie reprend l'appareil
- **Ce qui arrive :** un Hello arrivé sur un lien qui se fermait reprenait l'appareil au lien courant.
- **D'où on le sait :** `recherche/reconnexion.toml`, T3 et leçon 04.
- **Mécaniques :** reconnexion.
- **Question :** une fin tardive de l'ancien lien peut-elle toucher le nouveau ?
- **Pression :** la garde de fermeture et le rang des liens ; chaos (3 717 cas sur un tirage sans la garde).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-185 · Une tentative lancée pendant le sommeil reste bloquée au réveil
- **Ce qui arrive :** `wake` gardait une tentative en cours et son budget de 20 s, au-delà des 10 s de C3. Une tentative ouverte hors ligne peut ne jamais finir sur Safari mobile.
- **D'où on le sait :** `recherche/reconnexion.toml`, T4 et leçons 21, 45 ; commit `24a67c1`.
- **Mécaniques :** reconnexion.
- **Question :** au réveil et au retour du réseau, une vieille tentative est-elle abandonnée ?
- **Pression :** salon-core. Supposé : qu'elle reste bloquée sur un vrai iPhone.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-186 · Ce qui attendait disparaissait au redémarrage de l'hôte
- **Ce qui arrive :** un accueil d'un autre lancement vidait la file : une réponse envoyée pendant le redémarrage rendait Ok et disparaissait. Un événement en vol peut atteindre les deux lancements.
- **D'où on le sait :** `recherche/reconnexion.toml`, T6 ; commit `906b7de`.
- **Mécaniques :** reprise, phases.
- **Question :** un événement envoyé pendant un redémarrage compte-t-il exactement une fois ?
- **Pression :** salon-core ; la phase portée par l'événement permet de refuser un périmé.
- **Sous la nouvelle forme :** pression du commun. Un événement compte exactement une fois.
- **Étage :** commun.

### OUT-187 · Un échec passager ne doit pas retenir le repli pour toujours
- **Ce qui arrive :** pendant un redémarrage de l'hôte, deux offres sans réponse comptaient contre WebRTC : la page restait en WebSocket dix minutes, rechargements compris. Un 503 d'une file pleine comptait aussi. `visibilitychange` ne suit plus toujours un dégel.
- **D'où on le sait :** `recherche/reconnexion.toml`, T7, T8 et leçons 22, 23 ; `recherche/transport-webrtc.toml`, T6 et leçon 05 ; commits `7b1033b`, `b5cf782`.
- **Mécaniques :** réseau.
- **Question :** ce qui est retenu comme échec accuse-t-il vraiment le transport ?
- **Pression :** `rtc` (offre refusée qui n'empêche pas WebRTC au rechargement) ; écouteur `resume`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-188 · Une identité s'écrit avant que son accueil parte
- **Ce qui arrive :** la salle sauvait ses identités une fois par seconde : un appareil accueilli juste avant un plantage était inconnu au lancement suivant, et son numéro allait à un autre.
- **D'où on le sait :** `recherche/reconnexion.toml`, T10 et leçon 37 ; commit `0eae554`.
- **Mécaniques :** identité, reprise.
- **Question :** ce qui n'est pas écrit a-t-il pu être promis ?
- **Pression :** salon-core. Limite : le fichier n'est pas synchronisé, une coupure de courant n'est pas couverte.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-189 · Le rattrapage ne se distingue pas du direct
- **Ce qui arrive :** après une reconnexion, un jeu ne sait pas si un événement est en retard ou neuf.
- **D'où on le sait :** `recherche/reconnexion.toml`, T11 et leçon 41 (Discord).
- **Mécaniques :** temps réel.
- **Question :** le jeu a-t-il besoin de savoir qu'un événement arrive en rattrapage ?
- **Pression :** aucune ; trou ouvert, faible.
- **Sous la nouvelle forme :** impossible par la forme. Les entrées arrivent dans l'ordre des gestes, estimé, et un geste en retard est replacé dans le passé récent.
- **Étage :** commun.

### OUT-190 · Oublier un jeton ne ferme pas la porte
- **Ce qui arrive :** `forget` ne fermait pas le retour : le téléphone revenait en appareil neuf dans une salle ouverte.
- **D'où on le sait :** `recherche/reconnexion.toml`, T13 et leçon 06 ; commit `0eae554`.
- **Mécaniques :** identité.
- **Question :** l'autorité peut-elle refuser un retour ?
- **Pression :** salon-core, deux tests.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-191 · Un message plus gros que ce que le tuyau passe bloque tout ce qui le suit
- **Ce qui arrive :** chaque renvoi repartait du premier fragment ; un tuyau qui garde le début d'une rafale et jette la fin ne passait jamais la fin. 60 000 octets à 1 Mb/s avec 32 Kio de tampon : 0 message en 120 s.
- **D'où on le sait :** `recherche/fiabilite.toml`, T1 et leçons 14, 17, 22, 23, 49 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** réseau.
- **Question :** l'accusé dit-il quels fragments sont arrivés ?
- **Pression :** salon-core sur tuyau simulé ; 24 000 cas de chaos étroit. Supposé : que SCTP réel se comporte comme ce tuyau.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-192 · Un trou dans la file fait jeter ce qui suit
- **Ce qui arrive :** tout ce qui arrivait après un paquet perdu était jeté jusqu'au renvoi. 200 événements à 5 ms de gigue prenaient 10 s sans aucune perte.
- **D'où on le sait :** `recherche/fiabilite.toml`, T2 et leçons 01, 34, 42 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** réseau.
- **Question :** le récepteur garde-t-il, dans une borne, ce qui arrive après un trou ?
- **Pression :** salon-core, moyenne de 5 graines.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-193 · Ce qui part doit suivre ce qui revient
- **Ce qui arrive :** salon envoyait toute sa file d'un coup et renvoyait au même rythme sur un lien muet : un état de 60 000 octets coûtait 60 fois sa taille. Envoyer sans délai crée la perte qu'on veut réparer.
- **D'où on le sait :** `recherche/fiabilite.toml`, T3 et leçons 10, 11, 38, 40.
- **Mécaniques :** réseau.
- **Question :** l'envoi ralentit-il quand le lien se tait ou sature ?
- **Pression :** salon-core sur tuyau simulé. Supposé : le coût sur un Wi-Fi partagé par quinze téléphones.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-194 · Un WebSocket ne perd rien, et les renvois s'y empilent devant le frais
- **Ce qui arrive :** le flux renvoyait à la minuterie même sur un transport fiable : des copies passaient devant ce qui était frais. Un WebSocket pouvait aussi rester muet sans erreur.
- **D'où on le sait :** `recherche/fiabilite.toml`, T4 et leçon 02 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** réseau.
- **Question :** le renvoi tient-il compte de ce que le transport garantit déjà ?
- **Pression :** salon-core ; aucune reconnexion ni copie à 300, 64 et 32 kb/s.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-195 · Un gros état remplacé plus vite que son renvoi n'arrive jamais
- **Ce qui arrive :** chaque remplacement perdait ce qui était déjà arrivé du précédent.
- **D'où on le sait :** `recherche/fiabilite.toml`, T5 et leçons 04, 17, 18.
- **Mécaniques :** temps réel, réseau.
- **Question :** un état fréquent et gros finit-il par arriver ?
- **Pression :** salon-core ; un état parti en plusieurs paquets finit avant d'être remplacé.
- **Sous la nouvelle forme :** pression du commun. Un état périmé est remplacé par le plus récent, sans affamer la livraison.
- **Étage :** commun.

### OUT-196 · Un paquet en retard ne ramène pas la phase en arrière
- **Ce qui arrive :** un paquet réordonné faisait reculer la phase vue par la page ; l'événement suivant partait avec une phase périmée et était refusé (E5).
- **D'où on le sait :** `recherche/fiabilite.toml`, T6 et leçons 03, 05, 12, 37.
- **Mécaniques :** phases.
- **Question :** une valeur monotone se compare-t-elle avant d'être écrite ?
- **Pression :** salon-core avec paquets forgés, y compris à travers un redémarrage.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-197 · Une limite que l'envoi ne connaît pas rend Ok puis bloque
- **Ce qui arrive :** un message de plus de 1 024 fragments rendait Ok et bloquait la file pour toujours ; la page ne connaissait pas les limites de l'hôte, et ce qu'elle envoyait de trop disparaissait ou tournait sans fin.
- **D'où on le sait :** `recherche/fiabilite.toml`, T8, T9 et leçons 16, 36, 44.
- **Mécaniques :** réseau.
- **Question :** chaque côté annonce-t-il ce qu'il accepte, et la configuration se vérifie-t-elle à la création ?
- **Pression :** salon-core ; le Welcome porte `max_up` et `max_down`, `send` rend `TooBig`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-198 · Un départ passe après ce qui a été envoyé avant lui
- **Ce qui arrive :** un événement envoyé juste avant `quit` était perdu.
- **D'où on le sait :** `recherche/fiabilite.toml`, T10 et leçon 25 (ENet).
- **Mécaniques :** réseau.
- **Question :** le dernier geste avant un départ arrive-t-il ?
- **Pression :** salon-core sur faux réseau.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-199 · Ce qui attend se borne en octets, pas en messages
- **Ce qui arrive :** une file bornée en nombre de messages laissait retenir une mémoire sans borne utile.
- **D'où on le sait :** `recherche/fiabilite.toml`, T11 et leçons 09, 19.
- **Mécaniques :** mémoire, réseau.
- **Question :** la borne compte-t-elle ce qui coûte vraiment ?
- **Pression :** mémoire comptée par l'allocateur ; `send` rend `Full` à 1 Mio.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-200 · Une configuration incohérente ne fait pas paniquer la salle
- **Ce qui arrive :** un `Timing` à l'envers faisait paniquer la salle ; un battement à 0 empêchait `poll` de jamais répondre.
- **D'où on le sait :** `recherche/fiabilite.toml`, T12 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** toutes.
- **Question :** une configuration absurde est-elle refusée ou ramenée à une valeur sûre ?
- **Pression :** salon-core sur faux réseau.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-201 · Le calcul de la prochaine échéance ne parcourt pas tout
- **Ce qui arrive :** `next_deadline` allouait une fois par événement en attente, 50 µs par appel en debug.
- **D'où on le sait :** `recherche/fiabilite.toml`, T13 et leçon 13 (quinn).
- **Mécaniques :** temps réel.
- **Question :** ce qui tourne à chaque pas a-t-il un coût borné ?
- **Pression :** allocations comptées par l'allocateur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-202 · Une seule page pouvait tuer WebRTC pour toute la soirée
- **Ce qui arrive :** une panique de la pile DTLS, sur un doublon arrivé avant le changement de chiffrement, arrêtait le fil WebRTC de l'hôte ; tous les téléphones suivants recevaient un 503.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T1 et leçons 12, 16.
- **Mécaniques :** réseau.
- **Question :** une erreur d'un lien peut-elle atteindre les autres ?
- **Pression :** isolation par lien, sur Chromium et WebKit. Les navigateurs groupent la fin de poignée de main dans un datagramme : une perte naturelle ne l'isole pas, les 98 pannes unitaires n'ont rien déclenché.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-203 · Là où l'UDP n'arrive pas, attendre 15 s casse la reconnexion
- **Ce qui arrive :** la réponse SDP arrivait tout de suite, puis la page attendait la fenêtre de 15 s à chaque coupure, au-delà des 10 s de C2. Le navigateur met 15 à 30 s à dire `failed`.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T2 et leçons 26, 27, 41, 45 ; commit `ce410e1`.
- **Mécaniques :** réseau, reconnexion.
- **Question :** le repli part-il après un court silence d'ICE, et pas à l'échéance absolue ?
- **Pression :** `rtc`, avec un relais qui ne coupe que DTLS. Seuil de 3 s mesuré sur 36 essais, jamais sur un iPhone.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-204 · L'adresse annoncée doit être celle que le téléphone a jointe
- **Ce qui arrive :** l'hôte annonçait une adresse figée au lancement. Sur une machine à plusieurs interfaces, la route par défaut n'est pas forcément la bonne ; un socket sur `0.0.0.0` ne sait pas par où un paquet est arrivé.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T3 et leçons 21, 28.
- **Mécaniques :** réseau.
- **Question :** l'adresse annoncée suit-elle le chemin réel de chaque lien ?
- **Pression :** prouvée en mémoire, sur les vraies piles ; aucun Mac à deux interfaces essayé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-205 · Au-dessus de 1 062 octets, un paquet part en deux et la perte double
- **Ce qui arrive :** la promesse de tenir dans une trame était fausse : str0m retire 48 + 40 octets d'un MTU de 1 150. Chromium et WebKit envoient eux-mêmes 1 100 octets en un datagramme.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T4 et leçons 02, 09, 29, 30.
- **Mécaniques :** réseau, temps réel.
- **Question :** la taille promise tient-elle compte de chaque en-tête réel ?
- **Pression :** comptage des datagrammes sur les vraies piles. Non mesuré : partage de connexion d'un iPhone, vraie borne.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-206 · Un canal sans retransmission accumule des secondes de retard
- **Ce qui arrive :** le tampon SCTP compte tout ce qui n'est pas accusé, même sur un canal non fiable : au-delà de ce que le chemin tient, ce n'est que du retard. Une garde fixe de 4 Kio perdait toujours la fin d'une rafale.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T5 et leçons 10, 24.
- **Mécaniques :** temps réel, réseau.
- **Question :** ce qui vieillit dans le tampon est-il jeté en amont ?
- **Pression :** sonde sur les vraies piles avec un modèle de lien. Valeurs choisies par sonde, rien mesuré côté page.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-207 · Les offres en cours se bornent
- **Ce qui arrive :** rien ne bornait les négociations en cours ; un flot d'offres ralentissait les liens vivants. Une offre coûte plus qu'un message, et la limite de débit ne la voyait pas.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T7 et leçon 06 ; `CHANGELOG.md`, v0.4.0 (64 négociations au plus).
- **Mécaniques :** réseau.
- **Question :** un appareil hostile peut-il ralentir les liens ouverts par ses offres ?
- **Pression :** boucle locale ; le plus ancien part.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-208 · Une adresse IPv6 acceptée où rien n'écoute
- **Ce qui arrive :** `SALON_PUBLIC_URL` acceptait IPv6 alors que l'hôte n'écoutait qu'en IPv4 ; un test fixait même cette erreur comme valable.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T8 et leçon 04.
- **Mécaniques :** réseau.
- **Question :** une configuration acceptée est-elle servie ?
- **Pression :** refus clair, sur ce Mac.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-209 · Une dépendance non publiée coûte chez chaque ami
- **Ce qui arrive :** dimpl 0.7.6 garde des défauts tant que str0m ne publie rien au-dessus ; une dépendance git sur 46 commits non publiés ajouterait 32 paquets chez chaque ami.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, T11 et leçons 13, 14 ; `CHANGELOG.md`, v0.4.0.
- **Mécaniques :** réseau.
- **Question :** le défaut est-il contenu, et à quel prix le corriger ?
- **Pression :** le défaut est confiné à un lien par T1. Décision de l'hôte.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-210 · Safari cache ses adresses et suspend ses liens
- **Ce qui arrive :** Safari cache ses adresses locales derrière mDNS tant que la page n'a pas la caméra ; il suspend une PeerConnection pour le bfcache au lieu de la fermer, et la page revient avec un lien qui croit vivre. `disconnected` est passager par définition.
- **D'où on le sait :** `recherche/transport-webrtc.toml`, leçons 31, 35, 36, 43.
- **Mécaniques :** réseau, téléphones.
- **Question :** l'hôte attend-il quelque chose des candidats de la page ?
- **Pression :** couvert dans salon.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-211 · Une page en http sur une adresse locale n'a ni micro ni caméra
- **Ce qui arrive :** la norme réserve `mediaDevices` aux contextes sécurisés, et WebKit l'applique dans l'app comme dans Safari. Le contrat promettait le micro ; le bouton du faux jeu levait une exception.
- **D'où on le sait :** `recherche/telecommande.toml`, T12 et leçons 27, 28 ; commit `367201d`.
- **Mécaniques :** micro, caméra.
- **Question :** cette capacité existe-t-elle sur l'adresse d'une vraie soirée ?
- **Pression :** un test tient les deux cas à part.
- **Sous la nouvelle forme :** pression du commun. Le jeu voit des capacités déclarées, jamais une technologie.
- **Étage :** commun.

### OUT-212 · Le pont natif ne suit que la page voulue
- **Ce qui arrive :** le pont de l'app ne regardait ni le cadre ni l'origine : un iframe d'ailleurs envoyait de l'UDP, une page partie ailleurs emportait l'identité. N'importe quel QR obtenait le micro et la caméra sans question.
- **D'où on le sait :** `recherche/telecommande.toml`, T13, T14 et leçons 29, 30 ; commits `237705b`, `9e4c5b9`.
- **Mécaniques :** app iPhone.
- **Question :** qui atteint le pont : quel cadre, quelle origine ?
- **Pression :** une seule fonction Rust, une table de ce qu'elle accepte ; ce qui ne se lit pas avec certitude est refusé.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-213 · Ce qui part pendant une coupure attend, ne disparaît pas
- **Ce qui arrive :** dans l'app, les traits tracés pendant une coupure disparaissaient, et le trait ne finissait jamais. Une trame déjà remise au noyau quand le lien meurt reste perdue sans erreur.
- **D'où on le sait :** `recherche/telecommande.toml`, T1 et leçons 01, 02 ; commit `d4ed19d`.
- **Mécaniques :** dessin, app iPhone.
- **Question :** un événement émis sur un lien mort part-il à la reconnexion ?
- **Pression :** le banc sur simulateur. Reste ouvert : un accusé de trait par l'hôte.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-214 · Chaque côté surveille le silence de l'autre
- **Ce qui arrive :** l'app ne voyait pas un hôte figé : le noyau accusait encore réception. `URLSessionWebSocketTask` ne pingue pas seul.
- **D'où on le sait :** `recherche/telecommande.toml`, T2 et leçons 03, 04, 05 ; commit `5a90129`.
- **Mécaniques :** app iPhone, réseau.
- **Question :** l'app voit-elle un hôte muet en quelques secondes ?
- **Pression :** Ping toutes les 2 s, socket lâché sans Pong. Supposé : qu'un trou de 2 s sur un vrai Wi-Fi ne coupe pas un lien sain.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-215 · Une vue web ne réessaie pas seule, et sa mort laisse un écran vide
- **Ce qui arrive :** un chargement raté laissait un écran vide pour toujours. WebKit recharge une page morte une fois par 30 s, puis laisse la vue vide sans un mot ; le pont gardait les sockets de la page morte.
- **D'où on le sait :** `recherche/telecommande.toml`, T3, T4 et leçons 07 à 14 ; commits `9e8b40a`, `84e5f51`. Une erreur d'annulation vit dans `WebKitErrorDomain`, pas `WKErrorDomain` (commit `237705b`).
- **Mécaniques :** app iPhone.
- **Question :** l'app traite-t-elle elle-même l'échec et la mort de la page ?
- **Pression :** banc sur simulateur ; repli de 1 à 5 s, une notice.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-216 · Une tentative sans échéance propre dure une minute
- **Ce qui arrive :** `URLSession.shared` attend 60 s par défaut sur une adresse muette, mesuré 60,6 s, bien au-delà du plafond de 5 s écrit à côté.
- **D'où on le sait :** `recherche/telecommande.toml`, T6 et leçons 17, 18 ; commit `97cc51f`.
- **Mécaniques :** app iPhone, réseau.
- **Question :** chaque essai a-t-il sa propre échéance ?
- **Pression :** banc sur simulateur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-217 · Un frein comparé à 1/60 s pile saute une image sur deux
- **Ce qui arrive :** l'horloge lue à chaque rafraîchissement gigote ; 16,6 ms échouait, et l'app envoyait 36 états par seconde au lieu de 60. La propriété qui avait figé le défaut exigeait plus que le contrat.
- **D'où on le sait :** `recherche/telecommande.toml`, T7 et leçons 19, 20 ; commit `6fdbebb`.
- **Mécaniques :** stick, temps réel.
- **Question :** la propriété dit-elle ce que le contrat promet, avec la tolérance de l'horloge ?
- **Pression :** aucun écart sous 1/60 s moins la gigue, et jamais 61 états en moins d'une seconde.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-218 · Une coupure ne lâche pas les doigts, et un même affichage ne recrée rien
- **Ce qui arrive :** une coupure relâchait tous les doigts : le stick restait mort jusqu'à ce que le pouce se lève. Salon renvoyait la disposition à chaque reconnexion, et l'app recréait la télécommande : dessin perdu, numéros de trait réutilisés.
- **D'où on le sait :** `recherche/telecommande.toml`, T9, T10 et leçons 24, 25 ; commits `f36fb4e`, `a49c0ef`.
- **Mécaniques :** stick, dessin.
- **Question :** recevoir la même vue après une reprise change-t-il quelque chose ?
- **Pression :** cœur Rust ; banc.
- **Sous la nouvelle forme :** impossible par la forme. La vue se calcule depuis l'état, et la reprise ne recrée rien.
- **Étage :** commun.

### OUT-219 · Un retournement de 180° ne change ni la taille ni les marges
- **Ce qui arrive :** d'un paysage à l'autre, l'orientation n'était pas relue, et le volant braquait à l'envers.
- **D'où on le sait :** `recherche/telecommande.toml`, T11 et leçon 26 ; commit `29d584f`.
- **Mécaniques :** capteurs, volant.
- **Question :** l'orientation se lit-elle sur la géométrie de la scène elle-même ?
- **Pression :** banc ; le vrai volant reste un geste de `REPETITION.md`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-220 · L'écran se rend au système en quittant le jeu
- **Ce qui arrive :** l'écran ne s'éteignait plus jamais après une partie, scanner ouvert.
- **D'où on le sait :** `recherche/telecommande.toml`, T15 et leçons 31, 32 ; commit `04716b5`.
- **Mécaniques :** app iPhone.
- **Question :** chaque réglage pris pour la partie est-il rendu à sa sortie ?
- **Pression :** banc sur simulateur.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### OUT-221 · Une adresse relative avec un chemin remplace la requête de sa base
- **Ce qui arrive :** la page de l'app perdait le code de salle du QR.
- **D'où on le sait :** `recherche/telecommande.toml`, T16 et leçon 33 (RFC 3986) ; commit `fccbf76`.
- **Mécaniques :** app iPhone.
- **Question :** la requête du QR atteint-elle la page ?
- **Pression :** banc sur simulateur.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### OUT-222 · Une panique Rust rattrapée arrête quand même l'app Swift
- **Ce qui arrive :** les liaisons UniFFI rendent la panique à Swift, dont les `try!` générés arrêtent l'app.
- **D'où on le sait :** `recherche/telecommande.toml`, leçon 43.
- **Mécaniques :** app iPhone.
- **Question :** le code partagé avec l'app peut-il paniquer ?
- **Pression :** couvert dans la télécommande.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.
