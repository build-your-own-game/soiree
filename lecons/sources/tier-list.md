# Leçons · Tier list

> Les leçons du jeu `games/tier-list`, un éditeur de tier list solo copié de TierMaker. Elles viennent de son `LEARNINGS.md`, de son `verification.toml`, de son code et de son historique git, à l'étiquette `avant-refonte` (34 commits, du 26 octobre au 2 novembre 2026). Les chemins sont relatifs au dépôt du jeu, à cette étiquette. Le cahier des charges du jeu reconstruit est `games/tier-list/CAHIER.md`.
>
> Mécaniques nommées ici : plateau (classement d'objets dans des zones), glisser-déposer, texte saisi, annuler/rétablir, demandes asynchrones, images importées, export d'image, fichiers, persistance, journal et rejeu, outillage.

## Le plateau et ses règles

### TL-01 · Seule une action change le plateau, et la vue se recalcule depuis lui
- **Ce qui arrive :** TierMaker garde son état dans le DOM et détecte les changements avec un `MutationObserver`. Sa sauvegarde lit la page. Une vue qui diverge de l'état devient l'état.
- **D'où on le sait :** `LEARNINGS.md:83` (capture REA de `scripts/pages/create-v7.js`) ; `AGENTS.md` (« Le DOM n'est jamais la source de vérité ») ; `crates/core/src/lib.rs:121-161`.
- **Mécaniques :** plateau.
- **Question :** La vue peut-elle contenir une information que l'état n'a pas ?
- **Pression :** `random_actions_keep_the_board_consistent` (`crates/core/src/tests/properties.rs:413`).
- **Sous la nouvelle forme :** impossible par la forme. « Ce qu'il montre est une vue par appareil, calculée depuis l'état » (CONTRAT, « La forme d'un jeu »).
- **Étage :** commun.

### TL-02 · Un déplacement refusé laisse la tuile où elle était
- **Ce qui arrive :** retirer la tuile de sa place avant de vérifier la destination la perdrait si le rang visé n'existe plus.
- **D'où on le sait :** `crates/core/src/lib.rs:241-250` (vérifications avant `detach`) ; test `a_refused_move_leaves_the_tile_where_it_was` (`crates/core/src/tests.rs:187`), J13.
- **Mécaniques :** plateau.
- **Question :** Une action refusée peut-elle avoir déjà modifié une partie de l'état ?
- **Pression :** `a_refused_move_leaves_the_tile_where_it_was`, `invalid_actions_are_rejected_without_changes`.
- **Sous la nouvelle forme :** impossible par la forme. « Un refus ne change rien : le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte l'entrée. »
- **Étage :** commun.

### TL-03 · Une tuile ne quitte le plateau que par la poubelle, ou en annulant son ajout
- **Ce qui arrive :** vider ou supprimer un rang, ou réinitialiser, pourrait jeter des tuiles. TierMaker les renvoie à la fin de la réserve. Le joueur doit retrouver tout ce qu'il a ajouté.
- **D'où on le sait :** `LEARNINGS.md` (« Observé », comportement de TierMaker ; « Ce que l'éditeur promet », ligne 61) ; J11 ; `crates/core/src/lib.rs:262-270`, `:292-296`.
- **Mécaniques :** plateau.
- **Question :** Quels gestes peuvent faire disparaître un objet du joueur, et chacun est-il voulu ?
- **Pression :** `only_the_trash_loses_a_tile` (`crates/core/src/tests/properties.rs:489`), `cleared_and_deleted_rows_return_items_to_the_end_of_the_pool`, `reset_restores_rows_and_returns_items_in_insertion_order`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-04 · Un double appui sur une action qui fixe une valeur compte une fois
- **Ce qui arrive :** un double clic involontaire sur une couleur, un déplacement ou une suppression ne doit rien faire de plus qu'un clic. Ajouter, insérer ou déplacer un rang et Reset restent cumulatifs par règle.
- **D'où on le sait :** commit `813775b` ; item ECO-11 du catalogue (`soiree/certifications/economie.md`) ; J14.
- **Mécaniques :** plateau, toute commande déclenchée par un appui.
- **Question :** Pour chaque commande, deux appuis identiques doivent-ils agir une fois ou deux ? Est-ce écrit ?
- **Pression :** `a_double_click_counts_once` (`crates/core/src/tests/properties.rs:532`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-05 · Un compteur d'identifiants refuse l'action au lieu de déborder
- **Ce qui arrive :** une sauvegarde dont le compteur valait `u32::MAX` faisait déborder l'ajout suivant. Avec `overflow-checks`, c'était une panique.
- **D'où on le sait :** commit `28f18ed` (trouvé par `damaged_saves_load_as_nothing_or_a_consistent_board`) ; `crates/core/src/lib.rs:406-413` ; test `a_save_at_the_end_of_the_id_range_refuses_new_ids_without_panicking` (`crates/core/src/tests/properties.rs:703`).
- **Mécaniques :** plateau.
- **Question :** Que fait chaque compteur à sa dernière valeur ?
- **Pression :** le test cité ; le lint `arithmetic_side_effects`.
- **Sous la nouvelle forme :** impossible par la forme. Les règles d'écriture interdisent l'arithmétique qui peut déborder ; une entrée qui fait échouer le jeu est isolée et écartée.
- **Étage :** commun.

### TL-06 · Un état relu ne redonne jamais un identifiant déjà pris
- **Ce qui arrive :** une sauvegarde dont le compteur était en retard sur ses tuiles donnait à la tuile suivante un identifiant déjà sur le plateau. Deux tuiles sous un même identifiant.
- **D'où on le sait :** commit `28f18ed` ; `crates/core/src/lib.rs:484-491` ; test `a_save_with_a_stale_counter_never_reuses_an_id` (`crates/core/src/tests/properties.rs:691`).
- **Mécaniques :** plateau, fichiers.
- **Question :** Après une restauration, les compteurs sont-ils au-delà de tout identifiant présent ?
- **Pression :** le test cité ; `damaged_saves_load_as_nothing_or_a_consistent_board`.
- **Sous la nouvelle forme :** question de jeu. L'état gardé par le commun n'est plus relu, mais le fichier de sauvegarde du jeu l'est.
- **Étage :** jeu.

### TL-07 · Un plateau relu ne place jamais une tuile deux fois
- **Ce qui arrive :** une sauvegarde abîmée pouvait placer une tuile dans deux zones, ou donner le même identifiant à deux rangs.
- **D'où on le sait :** commit `28f18ed` ; `crates/core/src/lib.rs:474-483` ; test `a_save_placing_an_item_twice_is_refused` (`crates/core/src/tests/properties.rs:719`).
- **Mécaniques :** plateau, fichiers.
- **Question :** Quels invariants une donnée relue doit-elle vérifier avant de devenir l'état ?
- **Pression :** le test cité ; la cible de fuzz `fuzz/fuzz_targets/board.rs`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-08 · Une clé écrite deux fois est refusée, pas gardée en silence
- **Ce qui arrive :** relue dans une map, une tuile écrite deux fois sous un même identifiant garde la dernière valeur, sans erreur.
- **D'où on le sait :** commit `f94cdb1` ; `crates/core/src/lib.rs:428-454` ; `LEARNINGS.md:267-273`.
- **Mécaniques :** fichiers.
- **Question :** Le décodeur d'un fichier refuse-t-il les clés en double ?
- **Pression :** `a_backup_refuses_what_an_autosave_would_mend` (`crates/core/src/tests/backup.rs:128`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-09 · Un objet relu n'est pas un tableau
- **Ce qui arrive :** serde accepte par défaut une structure écrite comme un tableau positionnel, et un contenu au discriminant ambigu. Une donnée de forme inattendue passait pour un plateau.
- **D'où on le sait :** J5 ; `LEARNINGS.md:186-189` ; `crates/web/src/storage.rs:356-359`.
- **Mécaniques :** fichiers, persistance.
- **Question :** Le format accepte-t-il d'autres formes que celle qu'il écrit ?
- **Pression :** `saved_json_requires_objects_and_an_unambiguous_content_tag` (`crates/web/src/storage.rs:508`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-10 · Une valeur hors bornes venue d'une sauvegarde s'affiche, elle n'arrête pas la partie
- **Ce qui arrive :** une couleur hors de la palette, ou une tuile placée sans contenu, faisait paniquer le rendu par un index direct.
- **D'où on le sait :** commit `ab6f2fd` ; `crates/core/src/palette.rs:75-81` ; `crates/core/src/lib.rs:334-342` ; test `a_color_outside_the_palette_renders_as_a_new_row` (`crates/core/src/tests.rs:126`).
- **Mécaniques :** plateau.
- **Question :** Une valeur que les types n'excluent pas peut-elle arrêter le rendu ?
- **Pression :** les lints `indexing_slicing`, `unwrap_used`, `expect_used`, `panic`.
- **Sous la nouvelle forme :** impossible par la forme. Les règles d'écriture interdisent l'index hors bornes ; un `Color` ne se construit que valide.
- **Étage :** commun.

## Le fichier de sauvegarde

### TL-11 · Un fichier venu d'ailleurs n'est accepté que si le jeu l'aurait écrit lui-même
- **Ce qui arrive :** l'autosave se répare à la lecture, parce que c'est tout ce que le joueur a. Un fichier, lui, vient d'ailleurs, et le joueur a encore son plateau. Le réparer ferait passer un fichier abîmé pour un autre plateau.
- **D'où on le sait :** commit `f94cdb1` ; `crates/core/src/backup.rs:72-95` (relu, réécrit, comparé) ; `LEARNINGS.md:267-273`.
- **Mécaniques :** fichiers.
- **Question :** Une entrée venue de l'extérieur est-elle refusée, ou réparée en silence ?
- **Pression :** `a_damaged_backup_is_refused_or_is_what_the_type_writes`, `a_backup_refuses_what_an_autosave_would_mend`, `any_bytes_are_refused_without_a_panic`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-12 · Écrire une sauvegarde, c'est la relire
- **Ce qui arrive :** un plateau impossible à restaurer ne doit pas se découvrir le jour où l'onglet n'existe plus.
- **D'où on le sait :** `crates/core/src/backup.rs:59-70` ; `LEARNINGS.md:278-279`.
- **Mécaniques :** fichiers.
- **Question :** Le fichier qu'on donne au joueur a-t-il été relu avant de partir ?
- **Pression :** `a_board_a_backup_cannot_hold_is_not_written` (`crates/core/src/tests/backup.rs:212`).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-13 · Un fichier d'une autre version est refusé par son numéro, avant sa forme
- **Ce qui arrive :** un fichier d'une autre version ne doit rien à celle-ci. Lire sa forme d'abord donnerait « abîmé » au lieu de « autre version ».
- **D'où on le sait :** `crates/core/src/backup.rs:45-52`, `:78-82` ; `crates/web/src/backup.rs:28-30`.
- **Mécaniques :** fichiers.
- **Question :** Le jeu lit-il la version d'une donnée avant sa forme, et le dit-il ?
- **Pression :** `a_backup_of_another_version_is_refused_by_its_number` (`crates/core/src/tests/backup.rs:230`).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-14 · Un fichier se lit en octets, et sa taille se vérifie avant
- **Ce qui arrive :** `Blob.text()` remplace un octet abîmé par U+FFFD, sans erreur. Lire un fichier énorme garde plusieurs copies dans l'onglet.
- **D'où on le sait :** commit `03e73c3` ; `crates/web/src/backup.rs:89-93`, `:110-113` ; `LEARNINGS.md:280-281`.
- **Mécaniques :** fichiers.
- **Question :** Le commun livre-t-il un fichier en octets, avec sa taille connue avant lecture ?
- **Pression :** `the_size_limit_is_exact` (`crates/core/src/tests/backup.rs:280`) ; cas `backup-too-large` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-15 · Une image du plateau ne fait sortir aucune requête
- **Ce qui arrive :** une tuile s'affiche par son URL. Une adresse distante dans un fichier ferait sortir une requête, et tacherait le canvas de l'export.
- **D'où on le sait :** commit `f94cdb1` ; `crates/core/src/backup.rs:25-27`, `:97-104` ; `LEARNINGS.md:274-277` ; commit `0713f8d` (le contrôle réseau correspondant).
- **Mécaniques :** images importées, fichiers, export d'image.
- **Question :** Le langage de dessin du commun peut-il aller chercher une image sur le réseau à partir de l'état d'un jeu ?
- **Pression :** `a_backup_holds_its_images_itself` (`crates/core/src/tests/backup.rs:184`) ; cas `backup-refused` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Annuler et rétablir

### TL-16 · Annuler rend exactement l'état d'avant, compteurs compris
- **Ce qui arrive :** les premières lois d'annulation comparaient le code à lui-même et sautaient les éditions de texte. Rien ne vérifiait l'état exact rendu, ni la marche complète dans l'historique.
- **D'où on le sait :** commit `ad3231d` ; `crates/core/src/session.rs:75-97` ; J12.
- **Mécaniques :** annuler/rétablir.
- **Question :** Annuler puis rétablir rend-il le même état, à l'octet près ?
- **Pression :** `random_sessions_obey_the_undo_laws` (`crates/core/src/tests/properties.rs:446`), contre le modèle indépendant `crates/core/src/tests/model.rs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-17 · Les frappes d'une même édition forment une seule étape
- **Ce qui arrive :** une action par frappe donnerait une étape par lettre. L'étape se termine à la perte de focus, à Entrée, ou dès qu'une autre action arrive.
- **D'où on le sait :** `crates/core/src/session.rs:15-25`, `:60-71`, `:99-103` ; J2.
- **Mécaniques :** annuler/rétablir, texte saisi.
- **Question :** Qu'est-ce qu'une étape quand on tape ?
- **Pression :** `title_edits_coalesce_like_label_edits`, `undo_redo_and_label_edits_coalesce` (`crates/core/src/tests.rs:265`, `:305`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-18 · Une action sans effet ne crée pas d'étape
- **Ce qui arrive :** monter le premier rang ou relâcher une tuile à sa place créerait une étape vide, et viderait « rétablir ».
- **D'où on le sait :** `crates/core/src/session.rs:55-59`.
- **Mécaniques :** annuler/rétablir.
- **Question :** Une action acceptée mais sans effet touche-t-elle à l'historique ?
- **Pression :** `no_op_actions_do_not_create_undo_steps` (`crates/core/src/tests.rs:364`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-19 · L'historique garde exactement ses N dernières étapes
- **Ce qui arrive :** des mutants de la limite de 200 étapes passaient les tests. Une limite se vérifie à N-1, N et N+1, avec des valeurs écrites dans le test.
- **D'où on le sait :** commit `5acbc0d` ; `crates/core/src/session.rs:7`, `:64-68` ; item TOUR-09.
- **Mécaniques :** annuler/rétablir.
- **Question :** La borne de l'historique tient-elle exactement à son seuil ?
- **Pression :** `history_keeps_exactly_the_last_200_steps` (`crates/core/src/tests.rs:325`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-20 · Une copie d'historique partage les gros contenus
- **Ce qui arrive :** 200 copies du plateau, chacune avec ses images, multiplient la mémoire par 200. Les contenus sont immuables : ils se partagent.
- **D'où on le sait :** `crates/core/src/lib.rs:66-77` (contenus en `Rc`) ; `crates/core/src/session.rs:5-7`.
- **Mécaniques :** annuler/rétablir, images importées.
- **Question :** Combien coûte une étape d'historique quand l'état contient des images ?
- **Pression :** `replay_digest_does_not_repeat_large_contents_for_each_undo_step` (`crates/core/src/tests.rs:445`) ; scénario `resources` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique. La mémoire du module est l'état ; le partage doit y survivre.
- **Étage :** mécanique.

### TL-21 · Deux plateaux égaux aux passés différents ne sont pas le même état
- **Ce qui arrive :** une empreinte du seul plateau ne voit pas qu'un rejeu a perdu l'historique : la prochaine annulation les distinguerait.
- **D'où on le sait :** `LEARNINGS.md:113` ; `crates/core/src/session.rs:10-18` ; J3.
- **Mécaniques :** annuler/rétablir, journal et rejeu.
- **Question :** Le rejeu compare-t-il tout l'état, historique compris ?
- **Pression :** `sessions_with_different_histories_differ` (`crates/core/src/tests.rs:350`), `thousands_of_seeded_sessions_replay_with_their_history`.
- **Sous la nouvelle forme :** impossible par la forme. « La mémoire entière de son module compte comme son état : le commun peut la copier, la restaurer et la comparer. »
- **Étage :** commun.

### TL-22 · Une empreinte de l'état ne recalcule pas tout l'historique à chaque pas
- **Ce qui arrive :** hacher la session après chaque commande resérialisait chaque image jusqu'à 200 fois.
- **D'où on le sait :** `crates/core/src/session.rs:106-141` (empreinte mise en cache par copie) ; test `replay_digest_does_not_repeat_large_contents_for_each_undo_step`.
- **Mécaniques :** journal et rejeu, annuler/rétablir.
- **Question :** Combien coûte la copie, la comparaison ou l'empreinte de l'état quand il pèse des dizaines de Mo ?
- **Pression :** le test cité. Pour le commun : budget par pas, mesuré sur un état lourd.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-23 · Un identifiant rendu par l'annulation n'identifie plus le même contenu
- **Ce qui arrive :** annuler restaure aussi les compteurs. « Ajouter A, annuler, ajouter B » donne à B l'identifiant de A. Le cache de sauvegarde, gardé par identifiant, réécrivait A à la place de B. Plus tard, restaurer un fichier ramenait les mêmes identifiants avec d'autres contenus, et la page continuait d'afficher les anciennes tuiles.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:162-166`) ; commit `03e73c3` (`crates/web/src/render.rs:24-29`, `:56-62` ; `LEARNINGS.md:288-293`).
- **Mécaniques :** annuler/rétablir, plateau.
- **Question :** Ce qui garde un élément par son identifiant vérifie-t-il aussi son contenu ?
- **Pression :** cas `storage-identity` et `backup-round-trip` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun. La réconciliation des vues et les caches d'images du commun gardent leurs éléments par identité ; ils doivent être éprouvés avec des identifiants réutilisés.
- **Étage :** commun.

### TL-24 · Annuler vaut mieux que confirmer
- **Ce qui arrive :** Reset et la restauration remplacent tout le plateau. Plutôt qu'une fenêtre « êtes-vous sûr », ce sont des étapes comme les autres, annulables en une touche.
- **D'où on le sait :** `LEARNINGS.md:84`, `:282-285` ; `crates/core/src/lib.rs:156-160`.
- **Mécaniques :** annuler/rétablir.
- **Question :** Un geste destructeur peut-il être annulé, au lieu d'être confirmé ?
- **Pression :** `restoring_is_one_undo_step` (`crates/core/src/tests/backup.rs:295`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-25 · Une fenêtre suit son objet par identité, et se ferme quand il disparaît
- **Ce qui arrive :** dans TierMaker, « ajouter un rang au-dessus » décale les indices : la fenêtre doit recaler le sien. Annuler l'insertion d'un rang dont la fenêtre est ouverte laisserait une fenêtre sur un rang absent.
- **D'où on le sait :** `LEARNINGS.md`, « Observé », « Comportement » ; `crates/web/src/lib.rs:237-242`.
- **Mécaniques :** plateau.
- **Question :** Ce qui désigne un objet dans l'interface tient-il après une annulation qui le supprime ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

## Le texte

### TL-26 · Tout texte saisi a une borne dans les règles, pas seulement dans la page
- **Ce qui arrive :** le titre s'édite en place, sans `maxlength`. Un collage allait droit dans le modèle, dans chaque copie de l'historique et dans chaque sauvegarde.
- **D'où on le sait :** commit `40fd453` ; `crates/core/src/lib.rs:42-44`, `:288-291`.
- **Mécaniques :** texte saisi.
- **Question :** Chaque texte de l'état a-t-il une longueur maximale appliquée par les règles ?
- **Pression :** `a_pasted_title_stops_at_the_limit_of_a_label` (`crates/core/src/tests.rs:436`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-27 · La limite d'un texte se compte pareil dans la page et dans les règles
- **Ce qui arrive :** le navigateur compte `maxlength` en unités UTF-16. Le modèle doit couper dans la même unité. Le test qui le vérifiait relisait `render.rs` comme du texte pour y trouver la limite.
- **D'où on le sait :** commit `28f18ed` ; `crates/core/src/lib.rs:35-58` ; `LEARNINGS.md:119` ; test `the_page_and_the_model_share_their_limits` (`crates/core/src/tests/properties.rs:673`).
- **Mécaniques :** texte saisi.
- **Question :** La limite d'un champ et celle de l'état ont-elles une seule définition ?
- **Pression :** `typed_text_is_cut_to_the_limit_as_the_page_counts`.
- **Sous la nouvelle forme :** impossible par la forme. « Deux morceaux qui doivent s'accorder n'ont qu'une seule définition » ; les champs de texte sont des composants du commun, configurés depuis le jeu.
- **Étage :** commun.

### TL-28 · Couper un texte ne coupe jamais un graphème
- **Ce qui arrive :** au-delà de sa limite, une étiquette gardait la moitié d'une famille emoji, ou perdait l'accent de sa lettre.
- **D'où on le sait :** commit `b9facf2` ; `crates/core/src/lib.rs:46-58` ; item TXT-05 ; régression `crates/core/proptest-regressions/tests/properties.txt`.
- **Mécaniques :** texte saisi.
- **Question :** Chaque coupe de texte s'arrête-t-elle à une frontière de graphème étendu (UAX #29) ?
- **Pression :** `a_cut_never_splits_an_emoji_family_or_an_accent` (`crates/core/src/tests/properties.rs:628`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-29 · Un aperçu tronqué le montre, et garde le texte entier accessible
- **Ce qui arrive :** une tuile coupait les longs mots sans rien montrer. L'aperçu affiche maintenant un début suivi de « … », et garde le texte entier dans l'info-bulle et le nom accessible.
- **D'où on le sait :** `LEARNINGS.md:356-362` (corpus T2) ; `crates/web/src/render.rs:189-197`, `:210-221`.
- **Mécaniques :** texte saisi.
- **Question :** Un texte affiché plus court qu'il n'est le dit-il ?
- **Pression :** `tile_preview_preserves_graphemes_and_keeps_short_text_full` (`crates/web/src/render.rs:446`) ; `tools/text-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-30 · La coupe à la limite est silencieuse
- **Ce qui arrive :** dans un champ en place, le texte en trop reste visible pendant la saisie, puis disparaît à la perte de focus, sans message. C'est une limite connue, non corrigée.
- **D'où on le sait :** J6, hypothèses (`verification.toml`) ; `LEARNINGS.md:61`.
- **Mécaniques :** texte saisi.
- **Question :** Le joueur voit-il qu'un texte a été coupé au moment où il l'est ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-31 · Un éditeur vidé vaut une chaîne vide
- **Ce qui arrive :** un `contenteditable` vidé contient souvent un `<br>` de remplissage ; `innerText` rend alors un saut de ligne que le joueur n'a pas tapé, et il était sauvegardé.
- **D'où on le sait :** `LEARNINGS.md:366-372` ; `crates/web/src/editing.rs:9-18`.
- **Mécaniques :** texte saisi.
- **Question :** Le champ de texte du commun rend-il exactement ce que le joueur a tapé, y compris le vide ?
- **Pression :** `tools/text-check.mjs` (23 entrées sur les tuiles, les étiquettes et le titre).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-32 · Pendant une composition, aucune touche n'est un raccourci ni une validation
- **Ce qui arrive :** avec un clavier IME, Entrée ou une touche de raccourci servent à composer. Les traiter comme validation ou raccourci casse la saisie.
- **D'où on le sait :** `crates/web/src/events.rs:223-225`, `:273-279` ; `LEARNINGS.md:184-185`.
- **Mécaniques :** texte saisi.
- **Question :** Le commun dit-il au jeu qu'une composition est en cours, et retient-il les raccourcis ?
- **Pression :** scénario `editing` de `tools/ui-check.mjs` (composition synthétique). Un vrai IME et le clavier de l'iPhone restent à la répétition.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-33 · Un champ en cours d'édition garde son texte, son focus et sa sélection quand le reste change
- **Ce qui arrive :** ajouter une tuile reconstruisait les rangs et retirait le focus de l'étiquette en cours d'édition. Réécrire le texte d'un champ focalisé déplacerait le curseur sous les doigts.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:152-156`) ; `crates/web/src/render.rs:52`, `:83-87`, `:151-165` ; `crates/web/src/editing.rs:34-93`.
- **Mécaniques :** texte saisi.
- **Question :** Un redessin peut-il toucher un champ que le joueur est en train d'éditer ?
- **Pression :** scénario « a row keeps its identity, focus and selection across another edit » (`cert.json`) ; `tools/ui-check.mjs`, cas `editing`.
- **Sous la nouvelle forme :** pression du commun. Le contrat le demande déjà pour la page : « Un élément en cours d'édition garde son identité, sa sélection et sa composition tant que son objet existe. »
- **Étage :** commun.

### TL-34 · Pendant une saisie, Cmd/Ctrl+Z appartient au champ
- **Ce qui arrive :** un raccourci global d'annulation, actif dans un champ, annulerait le plateau au lieu de la dernière frappe.
- **D'où on le sait :** `crates/web/src/events.rs:261-270`.
- **Mécaniques :** texte saisi, annuler/rétablir.
- **Question :** Quels raccourcis du jeu cèdent la place à un champ de texte focalisé ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-35 · Un texte s'affiche, il ne s'interprète jamais
- **Ce qui arrive :** TierMaker passe l'étiquette en `.html()`, avec un filtre `<script>` ajouté après coup.
- **D'où on le sait :** `LEARNINGS.md:81` ; textes hostiles de `crates/core/src/tests/properties.rs:26-51`.
- **Mécaniques :** texte saisi.
- **Question :** Un texte du joueur peut-il devenir du balisage ?
- **Pression :** `nasty_texts_are_kept_as_typed` (`crates/core/src/tests/properties.rs:641`) ; `tools/text-check.mjs`.
- **Sous la nouvelle forme :** impossible par la forme. Le jeu décrit du texte dans le langage de dessin du commun ; il n'écrit aucun HTML.
- **Étage :** commun.

## Le glisser-déposer

### TL-36 · Souris, doigt et stylet passent par un seul chemin, et la page ne défile pas sous le doigt
- **Ce qui arrive :** TierMaker intercepte `touchmove` avec `preventDefault` pendant le glisser, sinon la page défile au lieu de la tuile.
- **D'où on le sait :** `LEARNINGS.md:79` ; `web/style.css`, `.character` (`touch-action: none`) ; `crates/web/src/drag.rs:1-6`.
- **Mécaniques :** glisser-déposer.
- **Question :** Un élément déplaçable du commun empêche-t-il le défilement natif, sur tous les pointeurs ?
- **Pression :** aucune sur un vrai écran tactile.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-37 · La capture du pointeur se tient sur un élément qui ne bouge pas
- **Ce qui arrive :** déplacer la tuile dans la page libère une capture tenue par la tuile elle-même. Le relâché hors de la fenêtre serait perdu.
- **D'où on le sait :** `crates/web/src/drag.rs:170-176`.
- **Mécaniques :** glisser-déposer.
- **Question :** Qui tient la capture pendant que l'élément déplacé change de place ?
- **Pression :** scénarios de glisser de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-38 · Un refus de capture n'arrête pas le glisser
- **Ce qui arrive :** le navigateur refuse la capture pour un pointeur qui n'est plus actif. Le glisser s'arrêtait alors, alors que les écouteurs de la fenêtre le suivaient très bien.
- **D'où on le sait :** commit `cf397f1`.
- **Mécaniques :** glisser-déposer.
- **Question :** Une fonction du navigateur « au mieux » est-elle traitée comme telle ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-39 · La place d'un lâcher se calcule dans l'ordre de l'état
- **Ce qui arrive :** la première version lisait l'indice dans l'ordre des éléments affichés, que le glisser déplace lui-même en direct. La géométrie dit quelle tuile est sous le pointeur ; seul l'état dit l'ordre, en comptant sans la tuile tenue.
- **D'où on le sait :** commit `f52158b` ; `crates/web/src/drag.rs:225-261` ; `LEARNINGS.md:113` ; J13.
- **Mécaniques :** glisser-déposer.
- **Question :** La destination d'un dépôt s'exprime-t-elle en termes de l'état, indépendamment de l'affichage ?
- **Pression :** `moving_within_and_between_zones` ; la traduction du pointeur n'est éprouvée que par les scénarios de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-40 · On peut glisser vers ce qui est hors de l'écran
- **Ce qui arrive :** TierMaker ajoute un bouton « Pin Images » qui colle la réserve en bas, parce qu'on ne peut pas glisser vers un rang hors de l'écran. Ici, la page défile quand on tient une tuile près du bord, même pointeur immobile, et la cible se recalcule après chaque défilement.
- **D'où on le sait :** `LEARNINGS.md:80` ; `crates/web/src/drag.rs:263-293`.
- **Mécaniques :** glisser-déposer.
- **Question :** Chaque destination est-elle atteignable sans lâcher ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-41 · Une vitesse se compte par unité de temps, pas par image
- **Ce qui arrive :** le défilement automatique avançait de 22 px par image : deux fois plus vite sur un écran à 120 Hz.
- **D'où on le sait :** commit `f52158b` (22 px par image devenus 1320 px/s) ; `crates/web/src/drag.rs:295-305` ; J4, items TR-01 et TR-02.
- **Mécaniques :** glisser-déposer.
- **Question :** Une grandeur dépend-elle de la fréquence de l'écran ?
- **Pression :** `autoscroll_distance_does_not_depend_on_display_frequency` (`crates/web/src/drag.rs:312`).
- **Sous la nouvelle forme :** impossible par la forme. « Le temps avance par pas réguliers, fournis par le commun. »
- **Étage :** commun.

### TL-42 · Un onglet qui se réveille ne fait pas sauter la page
- **Ce qui arrive :** au retour d'un onglet caché, le temps écoulé depuis la dernière image est énorme. Sans plafond, la page entière défilait d'un coup.
- **D'où on le sait :** `crates/web/src/drag.rs:296`, `:304` (plafond de 50 ms) ; `LEARNINGS.md:112`.
- **Mécaniques :** glisser-déposer, temps réel.
- **Question :** Combien de pas le commun livre-t-il après une suspension, et que fait le jeu du temps perdu ?
- **Pression :** `autoscroll_distance_does_not_depend_on_display_frequency` (borne à 10 s).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-43 · Un glisser abandonné laisse la tuile à une place valide
- **Ce qui arrive :** dragula garde la dernière place valide quand on lâche hors de toute zone. Échap, `pointercancel`, ou un glisser commencé hors zone doivent remettre la tuile où l'état le dit.
- **D'où on le sait :** `LEARNINGS.md`, « Glisser-déposer (dragula) » ; `crates/web/src/drag.rs:95`, `:143-151`, `:219-224`.
- **Mécaniques :** glisser-déposer.
- **Question :** Où va l'objet quand le geste finit hors de toute cible, ou s'interrompt ?
- **Pression :** scénarios `drag-lifetime` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-44 · Remplacer tout l'état annule le glisser et l'édition en cours
- **Ce qui arrive :** une restauration qui aboutit pendant qu'on tient une tuile ramène peut-être un autre contenu sous le même identifiant. Le glisser, la composition et la fenêtre ouverte n'ont plus d'objet.
- **D'où on le sait :** commit `03e73c3` ; `crates/web/src/lib.rs:132-140`.
- **Mécaniques :** glisser-déposer, demandes asynchrones.
- **Question :** Que deviennent les gestes en cours quand l'état est remplacé d'un coup ?
- **Pression :** `restoring_ends_the_text_edit` (`crates/core/src/tests/backup.rs:313`) ; cas `backup-race` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-47 · Un seuil sépare un clic d'un glisser, et l'original se distingue de sa copie
- **Ce qui arrive :** sans seuil, chaque clic sur une tuile démarre un glisser. TierMaker montre la copie à 80 % d'opacité, l'original à 20 % dans sa future place. Le seuil choisi est de 4 px.
- **D'où on le sait :** `LEARNINGS.md`, « Glisser-déposer (dragula) » ; `crates/web/src/drag.rs:23-24`, `:86-90` ; `web/style.css`, `.gu-mirror`, `.gu-transit`.
- **Mécaniques :** glisser-déposer.
- **Question :** Le joueur voit-il à la fois ce qu'il tient et où il le posera ?
- **Pression :** aucune ; c'est un jugement de joueur.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Les demandes asynchrones

### TL-45 · Le résultat d'une demande annulée ou remplacée ne touche à rien
- **Ce qui arrive :** fermer un export puis en rouvrir un autre laissait le premier résultat remplir la nouvelle fenêtre.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:146-151`) ; `crates/web/src/media.rs:157-185`.
- **Mécaniques :** demandes asynchrones, export d'image.
- **Question :** Chaque réponse porte-t-elle l'identité de sa demande, et l'état sait-il si cette demande vit encore ?
- **Pression :** cas `export-race` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique. Les réponses reviennent comme des entrées ; le jeu, ou le commun, écarte celles d'une demande morte.
- **Étage :** mécanique.

### TL-46 · Des demandes indépendantes finissent dans n'importe quel ordre, et un remplacement de l'état les annule toutes
- **Ce qui arrive :** deux imports d'images lancés l'un après l'autre peuvent finir dans l'autre ordre : les deux restent valides. Un Reset les annule tous, sinon ils ajouteraient des tuiles au plateau neuf.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:146-151`) ; `crates/web/src/media.rs:27-66` ; `crates/web/src/lib.rs:133-136`.
- **Mécaniques :** demandes asynchrones, images importées.
- **Question :** Une demande est-elle unique (la suivante remplace) ou multiple (toutes comptent) ? Qui les annule ?
- **Pression :** cas `import-race` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

## Les ressources du navigateur

### TL-48 · Une boucle d'animation et un écouteur meurent avec ce qui les a créés
- **Ce qui arrive :** vingt glissers terminés avant l'image suivante gardaient vingt rappels `requestAnimationFrame`. Des écouteurs étaient oubliés avec `forget`.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:157-161`) ; `crates/web/src/drag.rs:38`, `:263-293`.
- **Mécaniques :** toutes.
- **Question :** Chaque rappel enregistré a-t-il un propriétaire qui le libère ?
- **Pression :** cas `drag-lifetime` et scénario `resources` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-49 · Une ressource du navigateur se libère explicitement, même sur l'erreur
- **Ce qui arrive :** un `ImageBitmap` garde sa mémoire décodée tant qu'on ne le ferme pas, même si le canvas échoue après le décodage. Une URL de Blob vit jusqu'à sa révocation.
- **D'où on le sait :** `LEARNINGS.md:157-161`, `:334-336` ; `crates/web/src/media.rs:110-118`, `:398-421` ; `crates/web/src/backup.rs:49-50`.
- **Mécaniques :** images importées, export d'image, fichiers.
- **Question :** Quelles ressources du navigateur ne sont pas rendues par le ramasse-miettes ?
- **Pression :** cas `export-blob-lifecycle`, `backup-blob-lifecycle` et `resources` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-50 · Un écouteur supporte d'être rappelé pendant qu'il s'exécute
- **Ce qui arrive :** un clic sur « Add Images » appelle `fileInput.click()`, qui renvoie aussitôt un clic au même écouteur du document. L'écouteur commun en `FnMut` jetait une erreur avant même d'atteindre le jeu. Trouvé par les gestes aléatoires de la certification.
- **D'où on le sait :** `LEARNINGS.md:174-180`.
- **Mécaniques :** toutes.
- **Question :** Un appel du navigateur peut-il redéclencher un événement de façon synchrone ?
- **Pression :** cas `file-picker-reentry` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-51 · Un événement né pendant un rendu ne s'applique pas au milieu de lui
- **Ce qui arrive :** retirer un élément du DOM pendant un rendu déclenche `focusout` de façon synchrone. L'ancien code ignorait ces événements réentrants par un emprunt qui échoue.
- **D'où on le sait :** `crates/web/src/lib.rs:87-95` ; `crates/web/src/events.rs:318-324`.
- **Mécaniques :** toutes.
- **Question :** Le jeu peut-il recevoir un événement pendant qu'il en traite un autre ?
- **Pression :** aucune spécifique.
- **Sous la nouvelle forme :** impossible par la forme. « Un jeu qui traite un événement à la fois ne connaît pas deux actions simultanées. »
- **Étage :** commun.

### TL-52 · Une fuite se mesure une fois l'historique plein
- **Ce qui arrive :** tant que l'historique borné se remplit, la mémoire monte normalement. Mesurée trop tôt, une croissance légitime ressemble à une fuite, et une fuite se cache derrière.
- **D'où on le sait :** `LEARNINGS.md:250-259` (105 cycles pour remplir 200 étapes, puis 30 cycles mesurés après GC) ; `certification/browser-resources.json`.
- **Mécaniques :** annuler/rétablir.
- **Question :** Les tampons bornés du jeu sont-ils pleins avant que la mesure commence ?
- **Pression :** scénario `resources` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-53 · Un fichier lâché sur la page ne la remplace pas
- **Ce qui arrive :** sans `preventDefault` sur `dragover` et `drop`, le navigateur ouvre le fichier lâché à la place de la page.
- **D'où on le sait :** `crates/web/src/events.rs:61-66`, `:360-365`.
- **Mécaniques :** fichiers, images importées.
- **Question :** Le commun intercepte-t-il les dépôts de fichiers sur toute la page ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-54 · Le même fichier peut être choisi deux fois de suite
- **Ce qui arrive :** un sélecteur de fichiers ne déclenche pas `change` si l'on choisit le même fichier qu'avant. Il faut remettre sa valeur à zéro après chaque choix.
- **D'où on le sait :** `crates/web/src/events.rs:356-357`.
- **Mécaniques :** fichiers.
- **Question :** Deux choix identiques donnent-ils deux entrées ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## L'export d'image

### TL-55 · L'export montre tout le texte : la police rétrécit, la ligne grandit, rien n'est coupé
- **Ce qui arrive :** le PNG perdait tout ce qui dépassait la quatrième ligne d'une tuile, laissait une longue étiquette déborder sur les rangs voisins, et coupait un long titre des deux côtés. Rien ne disait que l'image était plus courte que le plateau.
- **D'où on le sait :** commit `d8570b1`, `bfdc5cc` ; `LEARNINGS.md:346-352` ; `crates/web/src/media.rs:224-233`, `:461-492`, `:554-565` ; J8.
- **Mécaniques :** export d'image.
- **Question :** Un export peut-il perdre une partie de ce qu'il prétend montrer ?
- **Pression :** `an_exported_tile_keeps_every_letter_of_its_text`, `an_exported_row_is_as_tall_as_its_label`, `an_exported_title_is_as_tall_as_its_lines`, `an_exported_line_never_splits_a_grapheme` (`crates/web/src/media.rs:586-658`) ; cas `export-long-text`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-56 · Une image qu'on n'a pas pu dessiner est annoncée
- **Ce qui arrive :** une image qui ne se décodait pas laissait une case vide en silence.
- **D'où on le sait :** commit `d8570b1` ; `crates/web/src/media.rs:76-82`, `:371-384`.
- **Mécaniques :** export d'image.
- **Question :** Chaque élément omis d'un rendu est-il compté et dit ?
- **Pression :** cas `export-broken-image` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### TL-57 · Un export long ne fait pas sauter d'image : lire un canvas coûte, même par une API asynchrone
- **Ce qui arrive :** `toDataURL` bloquait jusqu'à 83 ms avec 192 tuiles. `toBlob` seul laissait encore des images de 50 à 67 ms. Il a fallu des canvas prévus pour la lecture CPU (`willReadFrequently`), un dessin découpé en lots de 3 ms avec une image présentée entre deux, et un encodage asynchrone.
- **D'où on le sait :** commit `3cf335c` ; `LEARNINGS.md:322-345` ; `crates/web/src/media.rs:84-100`, `:235-287`, `:429-450` ; références citées : standard Canvas (`html.spec.whatwg.org`, `toBlob`) et blog Chromium `developer.chrome.com/blog/canvas2d`.
- **Mécaniques :** export d'image.
- **Question :** Un travail lourd demandé au commun respecte-t-il le budget d'image ?
- **Pression :** cas `export-frame-budget` de `tools/ui-check.mjs` (96 tuiles, aucune image au-delà de 33 ms) ; `certification/export-performance.json`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-58 · Le délai d'un contrôleur n'est pas la durée d'une image
- **Ce qui arrive :** le harnais mesurait la cadence avec un minuteur Node. Endormir Node seul faisait déclarer un timeout, alors que le navigateur tenait sa cadence. Les minuteurs Node ne garantissent ni l'instant ni l'ordre de leurs rappels.
- **D'où on le sait :** `LEARNINGS.md:310-321` ; référence citée : `nodejs.org/api/timers.html` ; `certification/p1-2026-10-29/` (retiré par `f20c7e4`, lisible dans `7eb3429`).
- **Mécaniques :** outillage.
- **Question :** La mesure distingue-t-elle le retard de l'outil de celui du jeu ?
- **Pression :** le contrôle négatif du harnais qui bloque vraiment le navigateur 3,5 s et doit échouer.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-59 · L'export se dessine depuis l'état, jamais en photographiant la page
- **Ce qui arrive :** TierMaker élargit la page à 2000 px, grossit les tuiles, photographie le DOM avec html2canvas, puis remet les tailles. L'écran saute, et l'image dépend de la page.
- **D'où on le sait :** `LEARNINGS.md:82`.
- **Mécaniques :** export d'image.
- **Question :** Une image produite par le jeu dépend-elle de ce qui est à l'écran ?
- **Pression :** aucune spécifique.
- **Sous la nouvelle forme :** impossible par la forme. L'information va de l'état vers l'image ; le jeu ne relit rien de l'affichage.
- **Étage :** commun.

## Les images importées

### TL-60 · Une image importée est réduite à la taille où elle s'affiche
- **Ce qui arrive :** TierMaker est passé du cookie à `localStorage` puis à IndexedDB, à cause de la taille des images. Une photo de 12 Mpx pour une tuile de 80 px gonfle la mémoire, l'historique et la sauvegarde. Recadrer en carré et réduire à 160 px (deux fois la tuile, pour les écrans denses) suffit.
- **D'où on le sait :** `LEARNINGS.md:78` ; `crates/web/src/media.rs:21-24`, `:102-138`.
- **Mécaniques :** images importées.
- **Question :** Quelle est la plus grande image que l'état puisse avoir à garder ?
- **Pression :** `backup-large` de `tools/ui-check.mjs` ; `MAX_BYTES` dans `crates/core/src/backup.rs:17-20`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-61 · Un import en partie illisible ajoute le reste, en une étape, et compte les échecs
- **Ce qui arrive :** un fichier qui ne se décode pas ne doit pas faire perdre les autres. Toutes les images d'un geste arrivent en une seule action.
- **D'où on le sait :** `crates/web/src/media.rs:26-66`.
- **Mécaniques :** images importées, annuler/rétablir.
- **Question :** Un lot d'entrées en partie invalide est-il refusé entier, accepté en partie, et que dit-on ?
- **Pression :** cas `import-race` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### TL-62 · Un message qui compte s'accorde avec son nombre
- **Ce qui arrive :** « 1 files could not be read » se remarque. Les deux messages à compteur du jeu sont vérifiés pour 1, 2, 11 et 21.
- **D'où on le sait :** item AFF-05 (`soiree/certifications/affichage.md`) ; `crates/web/src/media.rs:68-82` ; J9.
- **Mécaniques :** affichage.
- **Question :** Chaque message qui contient un nombre est-il accordé ?
- **Pression :** `notices_agree_with_their_number` (`crates/web/src/media.rs:616`). Limité aux deux messages de `media.rs`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

## La persistance (désormais au commun)

### TL-63 · Seule la fin de la transaction prouve l'écriture
- **Ce qui arrive :** la réussite d'une requête `put` n'est pas celle de la transaction IndexedDB. Seul l'événement `complete` autorise à effacer « Saving changes… ». La durabilité `strict` demande une écriture persistante ; elle reste une promesse du navigateur et du matériel, pas une preuve contre une coupure de courant.
- **D'où on le sait :** `LEARNINGS.md:207-214` ; `crates/web/src/storage.rs:251-286` ; références citées : W3C IndexedDB, sections *transaction scheduling*, *transaction lifecycle* et *durability hint*.
- **Mécaniques :** persistance.
- **Question :** Quand le commun dit « sauvegardé », qu'est-ce qui l'établit ?
- **Pression :** `a_failed_write_keeps_the_last_save_and_warns_until_one_lands` (`crates/web/src/storage.rs:396`) ; cas `aborted-save` de `tools/ui-check.mjs` (abandon après `put`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-64 · Une copie ne remplace jamais une sauvegarde qu'elle n'a pas vue
- **Ce qui arrive :** un onglet ancien supprimait les tuiles ajoutées par un autre. La comparaison et le remplacement doivent tenir dans la même transaction. Le perdant garde ses modifications en mémoire, cesse d'écrire, et le dit ; il ne reprend jamais l'écriture tout seul.
- **D'où on le sait :** passe du 29 octobre, commit `3cf335c` (`LEARNINGS.md:202-204`, `:216-226`) ; `crates/web/src/storage.rs:229-233`, `:268-271` ; item ECO-05 ; J10.
- **Mécaniques :** persistance.
- **Question :** Deux auteurs du même état peuvent-ils s'écraser ? Que devient le perdant ?
- **Pression :** `a_conflict_stops_every_later_write`, `a_tab_never_replaces_a_save_it_has_not_seen` ; cas `multi-tab` et `simultaneous-tabs` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun. « La partie qui fait foi vit sur une seule machine » : le commun doit dire ce que devient un second onglet.
- **Étage :** commun.

### TL-65 · Un stockage plein ou refusé garde la dernière sauvegarde, le dit, et réessaie
- **Ce qui arrive :** refuser le stockage permettait d'éditer sans signaler la perte au rechargement. L'avertissement doit rester tant qu'une écriture n'a pas réussi, et la modification suivante doit réessayer. Un refus à l'ouverture ne se réessaie pas avant le rechargement.
- **D'où on le sait :** commit `3cf335c` (`LEARNINGS.md:202-205`, `:223-226`) ; `crates/web/src/storage.rs:59-95`, `:128-154` ; J10, `LEARNINGS.md:66`.
- **Mécaniques :** persistance.
- **Question :** Que voit le joueur quand le stockage échoue, et quand cela s'arrête-t-il ?
- **Pression :** `a_failed_write_keeps_the_last_save_and_warns_until_one_lands`, `without_a_store_the_warning_stays_and_nothing_is_queued` ; cas `quota-failure` et `unavailable-storage`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-66 · Une écriture en cours se dit, et fermer l'onglet avant peut la perdre
- **Ce qui arrive :** pendant l'écriture, la page affiche « Saving changes… Keep this tab open until saving finishes. ». Fermer l'onglet à ce moment peut perdre la dernière modification.
- **D'où on le sait :** `crates/web/src/storage.rs:38` ; `LEARNINGS.md:61`.
- **Mécaniques :** persistance.
- **Question :** Quelle est la fenêtre pendant laquelle une modification acceptée peut encore être perdue ?
- **Pression :** aucune sur une vraie fermeture d'onglet.
- **Sous la nouvelle forme :** pression du commun. « La reprise survit à un onglet qui se recharge » ; l'état et les entrées récentes vivent hors de l'onglet.
- **Étage :** commun.

### TL-67 · Une sauvegarde illisible n'est pas écrasée à la lecture
- **Ce qui arrive :** à l'ouverture, une sauvegarde qu'on ne sait pas lire ouvre un plateau neuf, mais n'est pas effacée : seule une nouvelle édition la remplace, et un avertissement le dit.
- **D'où on le sait :** `LEARNINGS.md:228-229` ; `crates/web/src/storage.rs:137-143`.
- **Mécaniques :** persistance.
- **Question :** Une donnée qu'on ne comprend pas est-elle détruite sans que le joueur l'ait voulu ?
- **Pression :** cas `corrupt-snapshot` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-68 · Un ancien format n'ouvre pas un état vide sans un mot
- **Ce qui arrive :** retirer l'import unique des clés `localStorage` laissait un navigateur qui n'avait que ces clés ouvrir un plateau vide, sans rien dire. Le message ne paraît pas à côté d'une sauvegarde, car l'ancien import gardait les clés : une clé seule ne prouve rien de perdu.
- **D'où on le sait :** commits `252f547`, `810cf24` ; `LEARNINGS.md:231-241` ; `crates/web/src/storage.rs:156-161` ; J16.
- **Mécaniques :** persistance.
- **Question :** Une donnée d'une version antérieure a-t-elle sa conversion, ou au moins un message ?
- **Pression :** `only_a_browser_without_a_save_hears_about_the_old_keys` (`crates/web/src/storage.rs:490`) ; cas `legacy-keys`.
- **Sous la nouvelle forme :** pression du commun. « Une sauvegarde d'une version antérieure passe par une conversion explicite. »
- **Étage :** commun.

### TL-87 · Les écritures d'un même onglet se suivent, et seule la dernière en attente part
- **Ce qui arrive :** une modification faite pendant une écriture attend son tour. Les états intermédiaires sont regroupés : seul le plus récent s'écrit ensuite.
- **D'où on le sait :** `LEARNINGS.md:221-222` ; `crates/web/src/storage.rs:59-76`.
- **Mécaniques :** persistance.
- **Question :** Deux écritures du même auteur peuvent-elles se croiser ?
- **Pression :** `edits_made_during_a_write_are_saved_after_it` (`crates/web/src/storage.rs:418`).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-72 · Supprimer pour libérer de la place doit libérer tout de suite
- **Ce qui arrive :** une tuile supprimée ne retirait sa clé `localStorage` qu'au rechargement, alors que le message de quota conseillait justement de supprimer des images.
- **D'où on le sait :** commit `8b1477e` (`LEARNINGS.md:167-173`).
- **Mécaniques :** persistance.
- **Question :** —
- **Pression :** le test navigateur de l'époque, retiré avec ce format.
- **Sous la nouvelle forme :** obsolète. Il n'y a plus de clé par tuile : le commun garde l'état entier. Le fond de la leçon, un conseil qui doit vraiment marcher, est TL-69.
- **Étage :** commun.

## Les messages

### TL-69 · Un avertissement renvoie au secours qui sauve vraiment
- **Ce qui arrive :** les trois avertissements de stockage disaient d'exporter. Or le PNG ne se réimporte pas et n'a pas la réserve : l'onglet perdant d'un conflit ne gardait qu'une image de son travail. Ils nomment maintenant « Download Backup », un nom qu'un test vérifie dans la page.
- **D'où on le sait :** commit `691a145` ; `LEARNINGS.md:261-266`.
- **Mécaniques :** persistance, fichiers.
- **Question :** Le geste conseillé par un message existe-t-il, sous ce nom, et rend-il vraiment ce qu'on croit ?
- **Pression :** `every_warning_names_the_backup_the_page_offers` (`crates/web/src/storage.rs:478`) ; cas `backup-rescues-a-conflict`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-70 · Un bouton dit ce qu'il fabrique
- **Ce qui arrive :** le bouton qui dessinait le PNG s'appelait « Save or Download ». On le prenait pour une sauvegarde. Il s'appelle « Export Image (PNG) », et sa fenêtre dit ce que l'image ne peut pas faire.
- **D'où on le sait :** commit `691a145` ; `web/index.html` ; `crates/web/src/render.rs:392-396`.
- **Mécaniques :** export d'image.
- **Question :** Le nom d'un bouton promet-il plus que ce qu'il fait ?
- **Pression :** cas `png-says-it-is-an-image` de `tools/ui-check.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-71 · Un avertissement permanent ne cache pas ce que le dernier geste a dit
- **Ce qui arrive :** un avertissement de stockage masquait tout autre message. Un fichier refusé devait pourtant se lire sous lui. Les deux s'affichent bout à bout, l'avertissement d'abord.
- **D'où on le sait :** commit `03e73c3` ; `crates/web/src/render.rs:414-422`.
- **Mécaniques :** affichage.
- **Question :** Un message du commun peut-il cacher un message du jeu ?
- **Pression :** `a_storage_warning_does_not_hide_what_the_gesture_said` (`crates/web/src/render.rs:429`) ; cas `backup-notice-under-a-warning`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-86 · Un diagnostic qui échoue n'arrête pas le jeu
- **Ce qui arrive :** si l'enregistrement du journal échoue au démarrage ou en route, l'édition continue, et un avis le dit : « Replay recording is unavailable; editing still works. ».
- **D'où on le sait :** `crates/web/src/lib.rs:42-51`, `:211-235`, `:249-254`.
- **Mécaniques :** journal et rejeu.
- **Question :** Une panne de l'instrumentation peut-elle empêcher de jouer ?
- **Pression :** aucune spécifique.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

## Le journal et le rejeu

### TL-73 · Une session se rejoue avec son historique, et un ancien journal garde son sens
- **Ce qui arrive :** chaque commande (action, annuler, rétablir, fin d'édition) était journalisée avec une empreinte. Le rejeu comparait le plateau et les piles. Ajouter `Action::Restore` n'a pas changé le sens des entrées existantes : le format n'a pas monté, et les anciens journaux se relisent.
- **D'où on le sait :** `LEARNINGS.md:113`, `:121-125`, `:286-287` ; `crates/core/src/journal.rs` ; journal de régression `crates/core/tests/replays/solo.jsonl`.
- **Mécaniques :** journal et rejeu.
- **Question :** Un journal enregistré avant un changement se rejoue-t-il encore au même résultat ?
- **Pression :** `versioned_solo_journal_replays_the_final_board_and_history`, `thousands_of_seeded_sessions_replay_with_their_history`, `a_restore_replays_from_the_journal` (`crates/core/src/tests/backup.rs:324`).
- **Sous la nouvelle forme :** pression du commun. Le commun enregistre les entrées et rejoue ; un changement de sens sans rupture de type « possède un test ».
- **Étage :** commun.

## La méthode et l'outillage

### TL-74 · Un test qui tire ses attentes du code testé ne voit rien
- **Ce qui arrive :** les tests de placement et d'annulation prenaient leurs valeurs attendues dans `DEFAULT_ROWS`, `NEW_ROW_LABEL` ou `clip`, et comparaient le code à lui-même. Un modèle indépendant, écrit depuis les règles, qui n'appelle rien du code, et des valeurs littérales les ont remplacés.
- **D'où on le sait :** commit `ad3231d` ; `crates/core/src/tests/model.rs:1-11`.
- **Mécaniques :** outillage.
- **Question :** Les attentes d'un test viennent-elles d'ailleurs que du code qu'il juge ?
- **Pression :** l'injection de bugs (`soiree mutants`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-75 · Un mutant survivant montre un chemin sans attente écrite
- **Ce qui arrive :** des mutants de la limite d'historique, de l'égalité des sessions, de `Board::row`, de `Board::zone` et des commandes du journal passaient tous les tests.
- **D'où on le sait :** commit `5acbc0d` ; `certification/mutants/rapport.md`.
- **Mécaniques :** outillage.
- **Question :** Quelles fautes injectées les tests du jeu laissent-ils passer ?
- **Pression :** `soiree mutants`, étape 5 de l'épreuve.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-76 · Une boucle de test est bornée, pour échouer au lieu de pendre
- **Ce qui arrive :** avec un `Session::undo` cassé, la boucle « annuler jusqu'au bout » de la propriété ne finissait jamais : le test pendait jusqu'au timeout au lieu d'échouer.
- **D'où on le sait :** commit `01ad55e`.
- **Mécaniques :** outillage.
- **Question :** Chaque boucle d'un test a-t-elle une borne qui en fait un échec ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### TL-77 · Une garantie ne prétend pas tenir un item qu'elle ne remplit pas
- **Ce qui arrive :** J11 adoptait PROG-03 (« après rejouer, l'état est celui d'une partie neuve »). Mais Reset garde exprès le titre, les tuiles et l'historique : une remise en réserve passait pour une partie neuve.
- **D'où on le sait :** commit `0bcbeec` ; `LEARNINGS.md:297-301`.
- **Mécaniques :** outillage.
- **Question :** L'item adopté dit-il bien ce que la garantie vérifie ?
- **Pression :** la garantie d'examen ; la relecture humaine.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-78 · Être solo ne rend pas à lui seul un item sans objet
- **Ce qui arrive :** sur 97 items du catalogue, treize s'appliquent à cet éditeur solo, chacun par une garantie avec son test. Les 84 autres ont chacun leur raison écrite (votes, réseau, physique, hasard, scores).
- **D'où on le sait :** commit `813775b` ; `AGENTS.md`, « Contrats de vérification ».
- **Mécaniques :** outillage.
- **Question :** Chaque exclusion a-t-elle sa raison propre ?
- **Pression :** la garantie d'examen : « une applicabilité inconnue n'est jamais une exclusion ».
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### TL-79 · Le jeu servi est celui qu'on vient de compiler
- **Ce qui arrive :** avec `CARGO_TARGET_DIR`, l'hôte compilait ailleurs mais donnait `target/wasm32-unknown-unknown` à `wasm-bindgen`. Il pouvait publier une vieille compilation sans aucune erreur.
- **D'où on le sait :** commit `ba94907` ; J7 ; `crates/host/tests/target_dir.rs`.
- **Mécaniques :** outillage.
- **Question :** Ce qui tourne correspond-il aux sources qu'on croit ?
- **Pression :** `the_page_is_read_from_the_target_directory_cargo_was_given`.
- **Sous la nouvelle forme :** pression du commun. « La version d'un jeu se calcule depuis ses sources. Une page restée en cache après un changement est refusée. »
- **Étage :** commun.

### TL-83 · Une valeur a une seule source, même dans la documentation
- **Ce qui arrive :** le README et `AGENTS.md` recopiaient la version du commun et ses durées. Elles devenaient fausses à la mise à jour suivante.
- **D'où on le sait :** commit `c328da1`.
- **Mécaniques :** outillage.
- **Question :** Un document recopie-t-il une valeur qui vit ailleurs ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Obsolètes

### TL-80 · Toutes les sorties natives vont dans le dossier qu'on a donné
- **Ce qui arrive :** `TIER_DATA` n'était pas transmis au journal de diagnostics de salon, qui écrivait dans `data/journals/` du dossier courant. Ces fichiers de 44 octets changeaient l'empreinte des sources pendant la vérification.
- **D'où on le sait :** commit `8b1477e` ; `LEARNINGS.md:127-131` ; `crates/host/tests/data_root.rs`.
- **Mécaniques :** outillage.
- **Question :** —
- **Pression :** `tier_data_contains_common_diagnostics_without_writing_into_the_working_directory`.
- **Sous la nouvelle forme :** obsolète. Le jeu n'a plus d'hôte natif : les sorties appartiennent au commun.
- **Étage :** commun.

### TL-81 · Un test lancé sans Cargo ne s'appelle pas lui-même
- **Ce qui arrive :** `soiree check` lançait le binaire de test sans variable `CARGO`. Le faux `cargo` du test se trouvait lui-même par `PATH` et s'appelait sans fin.
- **D'où on le sait :** commit `019a768`.
- **Mécaniques :** outillage.
- **Question :** —
- **Pression :** —
- **Sous la nouvelle forme :** obsolète. Ce test vérifiait l'hôte natif du jeu, qui disparaît.
- **Étage :** connaissance.

### TL-82 · Comparer des origines, pas des adresses avec leur requête
- **Ce qui arrive :** le contrôle « aucun fichier refusé ne touche le réseau » comparait les requêtes à l'adresse de la page, que le harnais donne avec sa requête. Le propre journal du jeu comptait comme du réseau.
- **D'où on le sait :** commit `0713f8d`.
- **Mécaniques :** outillage.
- **Question :** —
- **Pression :** —
- **Sous la nouvelle forme :** obsolète. Le module enfermé n'atteint pas le réseau ; le commun constate son accès, sans contrôle par URL dans le jeu. La partie durable est TL-15.
- **Étage :** connaissance.

### TL-84 · Le port du serveur doit être libre en TCP et en UDP
- **Ce qui arrive :** salon ouvrait WebRTC sur le même numéro que le serveur HTTP, même pour la tier list qui n'a pas de téléphone.
- **D'où on le sait :** `README.md`, « Lancer ».
- **Mécaniques :** réseau.
- **Question :** —
- **Pression :** —
- **Sous la nouvelle forme :** obsolète. C'est un choix d'implémentation de l'ancien salon ; le relais est à reconcevoir.
- **Étage :** commun.

### TL-85 · Une image cachée pour que la bibliothèque de glisser la voie
- **Ce qui arrive :** TierMaker affiche ses tuiles en `background-image`, plus une `<img>` cachée (`draggable-filler`), pour satisfaire dragula.
- **D'où on le sait :** `LEARNINGS.md:85`.
- **Mécaniques :** glisser-déposer.
- **Question :** —
- **Pression :** —
- **Sous la nouvelle forme :** obsolète. Contournement d'une bibliothèque JavaScript ; le glisser appartient au commun.
- **Étage :** connaissance.

## Récapitulatif

| Classement | Leçons |
|---|---|
| impossible par la forme | TL-01, TL-02, TL-05, TL-10, TL-21, TL-27, TL-35, TL-41, TL-51, TL-59 |
| pression du commun | TL-14, TL-15, TL-22, TL-23, TL-31, TL-32, TL-33, TL-34, TL-36, TL-37, TL-38, TL-42, TL-48, TL-49, TL-50, TL-52, TL-53, TL-54, TL-57, TL-58, TL-63, TL-64, TL-65, TL-66, TL-67, TL-68, TL-71, TL-73, TL-75, TL-77, TL-78, TL-79, TL-86, TL-87 |
| question de mécanique | TL-04, TL-16, TL-17, TL-18, TL-19, TL-20, TL-26, TL-28, TL-29, TL-39, TL-40, TL-43, TL-44, TL-45, TL-46, TL-60, TL-61, TL-62 |
| question de jeu | TL-03, TL-06, TL-07, TL-11, TL-12, TL-13, TL-25, TL-30, TL-55, TL-56 |
| connaissance | TL-08, TL-09, TL-24, TL-47, TL-69, TL-70, TL-74, TL-76, TL-83 |
| obsolète | TL-72, TL-80, TL-81, TL-82, TL-84, TL-85 |

Les identifiants suivent l'ordre où les leçons ont été relevées ; les sections les regroupent par sujet.
