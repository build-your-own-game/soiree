# Leçons de Punchline

> Source : le dépôt `games/punchline` à l'étiquette `avant-refonte` (commit `a42e60e`), son `LEARNINGS.md` et son historique (`git -C games/punchline log avant-refonte`). Les chemins sont relatifs à `games/punchline/`, les lignes sont celles de cette étiquette. « salon » désigne l'ancienne couche réseau commune (`soiree/salon/`), dont les leçons détaillées sont dans `soiree/salon/LEARNINGS.md`.
>
> Le cahier des charges du jeu à reconstruire est `games/punchline/CAHIER.md`.

## Texte saisi sous échéance

### PUN-01 · Ce qu'un joueur a tapé avant l'échéance compte comme sa réponse
- **Ce qui arrive :** une réponse ne partait que par le bouton « Envoyer ». En finale, un joueur tapait, l'heure tombait, et son rival gagnait 6 000 points par forfait. La table de tri disait « envoi automatique : garder », et le code ne l'avait jamais fait.
- **D'où on le sait :** commit `684785c` ; `LEARNINGS.md`, « Le brouillon compte » ; `crates/core/src/lib.rs:1126` (les brouillons rendus à la fin de l'écriture) ; manette Jackbox `autoSubmit`, `write-live` (`LEARNINGS.md`, « Observé »).
- **Mécaniques :** texte saisi, tour par tour à échéance.
- **Question :** quand une phase d'écriture se ferme, que devient ce qui a été tapé sans être envoyé ?
- **Pression :** `a_draft_counts_when_the_time_runs_out`, `a_draft_counts_however_writing_ends`, `a_sent_answer_outranks_any_draft`, `a_final_half_written_keeps_what_was_typed` (J37).
- **Sous la nouvelle forme :** question de mécanique : le commun fournit les champs de texte, mais la règle « le brouillon compte » est une décision de jeu à écrire pour toute écriture sous minuteur.
- **Étage :** mécanique.

### PUN-02 · Le texte doit être sur la partie qui fait foi avant que le téléphone ne s'endorme
- **Ce qui arrive :** un envoi « au dernier instant » suppose un téléphone éveillé à l'échéance. Un téléphone verrouillé 21 s n'exécute plus rien : son minuteur dort avec lui, et ce qu'il garde en mémoire ne sert à personne. L'heure du téléphone ne doit donc pas décider : le texte part au fil de la frappe, et seule la fin d'écriture sur l'hôte tranche.
- **D'où on le sait :** `LEARNINGS.md`, « Le brouillon compte », « Pourquoi au fil de la frappe » ; commit `684785c` ; test `a_phone_asleep_for_the_end_of_writing_loses_nothing_it_typed` (`crates/host/src/room/properties.rs`).
- **Mécaniques :** texte saisi, téléphone.
- **Question :** ce que l'appareil sait au moment de l'échéance est-il déjà sur la partie qui fait foi ?
- **Pression :** `a_draft_typed_before_the_phone_locks_counts_at_the_deadline`, `a_draft_typed_during_a_cut_arrives_with_the_link`.
- **Sous la nouvelle forme :** pression du commun : « une page suspendue quand l'écran se verrouille » est dans « Ce que le réel impose » ; le commun doit l'imiter dans ses pressions pour tous les jeux qui ont du texte.
- **Étage :** commun.

### PUN-03 · Un brouillon est un état : seul le dernier compte, et son débit est borné
- **Ce qui arrive :** envoyé comme événement, chaque frappe aurait rempli la file d'événements, compté en double après une coupure, et coûté une entrée de journal. Sous forme d'état, seul le dernier voyage et il passe une coupure. Mais la limite d'événements de salon ne s'appliquait pas aux états : l'hôte gardait le dernier brouillon de chaque téléphone et ne le donnait au jeu qu'au réveil suivant, au plus dix fois par seconde.
- **D'où on le sait :** `LEARNINGS.md`, « Un état salon, pas un événement » et « Le débit » ; `crates/host/src/room.rs:290` et `:313`.
- **Mécaniques :** texte saisi, réseau.
- **Question :** une entrée qui change à chaque frappe est-elle un état ou un événement, et qui borne son débit ?
- **Pression :** `a_flood_of_drafts_is_one_draft` (`crates/host/src/room/properties.rs:1184`).
- **Sous la nouvelle forme :** pression du commun : « Un état périmé est remplacé par le plus récent ; un événement compte exactement une fois » appartient au commun, qui doit aussi borner un flot d'états avant qu'il n'entre dans la partie et son enregistrement.
- **Étage :** commun.

### PUN-04 · Un brouillon appartient à une phase d'écriture et à un joueur
- **Ce qui arrive :** les numéros de consigne repartaient de 0 à chaque manche, et les brouillons réapparaissaient d'une manche à l'autre. Plus tard : une reconnexion peut ne recevoir aucune vue intermédiaire, et deux écritures successives au contenu identique se confondaient. La vue d'écriture porte maintenant le numéro de phase, et l'écran ne garde un brouillon que pour cette phase et ce propriétaire.
- **D'où on le sait :** `LEARNINGS.md`, « Bugs trouvés en jouant » et « Durée de vie et continuité d'édition » ; `crates/web/src/lib.rs:65` ; commit `6daa637` (PROG-02, poison posé au changement de manche).
- **Mécaniques :** texte saisi, progression.
- **Question :** l'identité d'un champ en cours d'édition survit-elle à un index réutilisé, ou à des vues intermédiaires perdues ?
- **Pression :** `coalesced_rounds_cannot_reuse_an_identical_prompts_draft`, `resending_one_writing_phase_keeps_only_its_owners_draft`, `a_round_reads_nothing_the_last_one_left`.
- **Sous la nouvelle forme :** question de mécanique : le contrat dit qu'un élément en édition garde son identité « tant que son objet existe » ; c'est au jeu de donner à cet objet une identité unique sur la soirée, pas un index de manche.
- **Étage :** mécanique.

## Absences et qui est attendu

### PUN-05 · Une salle qui perd tout le monde ne clôt pas sa phase
- **Ce qui arrive :** « tout le monde a fini » se calculait sur les joueurs connectés. Un routeur qui décroche : personne n'est connecté, donc la phase est complète, et la manche brûle.
- **D'où on le sait :** commit `6f9590a` ; `LEARNINGS.md`, « Trois bugs que le harnais ne pouvait pas voir ».
- **Mécaniques :** tour par tour à échéance, présence.
- **Question :** quand plus personne n'est là, une phase qui attend des gestes se ferme-t-elle par elle-même ?
- **Pression :** `a_room_that_loses_everyone_waits_for_the_deadline` (J11).
- **Sous la nouvelle forme :** question de mécanique : le commun dit qui part et qui revient ; ce qu'une absence ferme est une règle de la mécanique.
- **Étage :** mécanique.

### PUN-06 · Le premier téléphone revenu ne ferme pas la phase sur les autres
- **Ce qui arrive :** les téléphones d'un Wi-Fi tombé reviennent à quelques secondes d'écart. Le premier correctif comptait les absents dehors dès que quelqu'un était là : le premier revenu refermait la phase sur les suivants. Une revue a trouvé trois contre-exemples : retour d'un joueur qui avait fini, retour d'un duelliste au vote, départs échelonnés.
- **D'où on le sait :** commit `178692d` ; `LEARNINGS.md`, « Trois bugs… », paragraphe sur le premier correctif incomplet.
- **Mécaniques :** présence, tour par tour.
- **Question :** un appareil qui vient de revenir compte-t-il déjà comme présent pour fermer une phase ?
- **Pression :** `the_first_person_back_does_not_end_writing_for_the_others`, `a_contestant_back_first_does_not_close_the_vote`, `people_who_leave_one_by_one_are_not_counted_out_at_once`, `people_back_from_an_outage_count_once_they_have_stayed`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-07 · Une panne qui touche la moitié de la salle attend l'échéance
- **Ce qui arrive :** sept téléphones sur huit coupés, le seul présent a fini : 20 s plus tard l'écriture se fermait, avec encore 70 s à l'échéance, et les sept perdaient leurs réponses. Règle : un absent ne compte dehors que si les présents sont strictement plus nombreux que les absents. Le prix : à deux humains et des robots, un seul absent attend l'échéance.
- **D'où on le sait :** commit `a99614c` ; `LEARNINGS.md`, « Une panne partielle » ; `crates/rules/src/lib.rs:574`.
- **Mécaniques :** présence.
- **Question :** une panne partielle peut-elle priver les absents d'un geste qu'ils avaient encore le temps de faire ?
- **Pression :** `half_the_room_gone_waits_for_the_deadline`, `a_partial_outage_does_not_close_writing_on_those_it_cut` ; prouvé par Verus (J34).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-08 · Chaque personne a sa propre horloge de présence
- **Ce qui arrive :** la grâce de 20 s avait une seule horloge pour la salle : tout départ ou retour la relançait. Un absent définitif et un téléphone qui clignote toutes les 15 s repoussaient l'écriture à ses 90 s. Maintenant, un absent compte dehors quand son absence à lui a duré 20 s, et seuls comptent comme présents ceux qui sont là depuis 20 s ou ne sont jamais partis.
- **D'où on le sait :** commit `4e1ecf7` ; `LEARNINGS.md`, « Une horloge par personne » ; `crates/core/src/lib.rs:1218`.
- **Mécaniques :** présence.
- **Question :** un appareil instable peut-il retenir indéfiniment une phase à cause d'un autre absent ?
- **Pression :** `a_phone_that_blinks_does_not_hold_up_someone_gone`, `each_absent_gets_a_whole_grace_of_their_own`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-09 · Le vote n'attend jamais les deux duellistes
- **Ce qui arrive :** un vote restait ouvert après le dernier bulletin quand l'un des deux duellistes venait de partir, parce que la présence était vérifiée pour tout le monde. Les duellistes n'ont rien à faire au vote.
- **D'où on le sait :** commit `9fdd9b5` ; `crates/core/src/lib.rs:1195`.
- **Mécaniques :** vote.
- **Question :** une phase attend-elle quelqu'un qui n'a aucun geste à y faire ?
- **Pression :** `a_duelist_who_drops_does_not_hold_a_finished_vote` (J14) ; propriété `a_short_cut_changes_neither_phases_nor_scores` qui l'a trouvé.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-10 · La table qui quitte le salon est celle du podium
- **Ce qui arrive :** le retrait d'un absent était permis à la fin, et l'hôte retire tout absent depuis plus de 90 s dès qu'il le peut : un leader au téléphone verrouillé disparaissait du podium. Le retrait ne vaut plus qu'au salon.
- **D'où on le sait :** commits `6f9590a` et `9e980d0` ; `crates/core/src/lib.rs:644`.
- **Mécaniques :** progression, classement.
- **Question :** un joueur absent peut-il disparaître d'un résultat qu'il a contribué à faire ?
- **Pression :** `an_absent_player_keeps_their_place_on_the_podium` (J10), `the_podium_keeps_the_table`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-11 · L'organisateur est le premier humain présent
- **Ce qui arrive :** le VIP était le premier arrivé, présent ou non : un VIP au téléphone endormi retirait « On rejoue ! » aux autres.
- **D'où on le sait :** commits `6f9590a`, `178692d` ; `crates/core/src/lib.rs:586` ; `LEARNINGS.md`, « Ce que le jeu promet », ligne « Qui peut relancer ? ».
- **Mécaniques :** salon, rôles.
- **Question :** un rôle réservé à un appareil peut-il bloquer la partie quand cet appareil dort ?
- **Pression :** `an_absent_vip_leaves_the_restart_to_someone_here`.
- **Sous la nouvelle forme :** question de mécanique : tout jeu qui confie l'organisation à un téléphone doit y répondre ; le jour où le commun tient le rôle de VIP, elle monte d'un étage.
- **Étage :** mécanique.

### PUN-12 · « On rejoue ! » ne retire pas un téléphone endormi
- **Ce qui arrive :** la relance retirait tout absent à cet instant, alors que le salon accorde 90 s. Un téléphone en veille une seconde perdait son siège au clic.
- **D'où on le sait :** commit `9010b25`.
- **Mécaniques :** salon, présence.
- **Question :** une transition déclenchée par quelqu'un d'autre peut-elle punir un absent momentané ?
- **Pression :** `a_new_game_keeps_a_sleeping_phone_in_the_lobby`, `a_new_game_gives_a_sleeping_phone_the_lobbys_grace` (J17).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-13 · Une place absente se garde 90 s au salon, et le téléphone le dit à son retour
- **Ce qui arrive :** sans délai, un téléphone qui se recharge perd son personnage. Sans fin, une place vide reste toute la soirée. Repris de Sidequest Live : 90 s. Un téléphone qui revient après voit « Ta place a été libérée : choisis ton prénom pour revenir. ».
- **D'où on le sait :** `LEARNINGS.md`, « Réseau : salon », « Ce qu'une absence veut dire » ; `crates/host/src/room.rs:36` et `:39`.
- **Mécaniques :** salon, présence.
- **Question :** combien de temps une place attend-elle son joueur, et que voit-il s'il revient trop tard ?
- **Pression :** `an_absent_player_loses_their_lobby_seat_after_the_grace`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-14 · Un lien qui tombe revient avec ses gestes : ne pas oublier l'appareil trop tôt
- **Ce qui arrive :** la salle oubliait aussitôt un téléphone qui n'avait pas encore joué. Or un lien qui tombe est un départ suivi d'un retour quelques centaines de millisecondes plus tard : oublié entre-temps, le téléphone revenait comme neuf, et le « C'est moi ! » touché pendant la reconnexion était jeté. Le joueur restait sur le formulaire, sans signe que son geste était perdu.
- **D'où on le sait :** `LEARNINGS.md`, « Un C'est moi ! perdu (C2) » ; `crates/host/src/room.rs:243`.
- **Mécaniques :** identité, reprise.
- **Question :** un geste fait pendant une reconnexion survit-il à ce que la partie décide pendant la coupure ?
- **Pression :** `a_join_tapped_while_reconnecting_counts` (`crates/host/src/room/properties.rs:917`), qui passe par le vrai client et le vrai hôte et échouait avant le correctif.
- **Sous la nouvelle forme :** pression du commun : l'identité, les événements gardés jusqu'à livraison et la reprise appartiennent au commun.
- **Étage :** commun.

### PUN-15 · Sur une soirée, la mémoire reste bornée
- **Ce qui arrive :** un joueur parti pour de bon perdait son siège, mais son téléphone restait dans la salle et dans salon toute la soirée, et chaque changement lui recalculait un écran. Six heures simulées gardaient 181 téléphones et tournaient six fois plus lentement.
- **D'où on le sait :** commit `a42e60e` ; `crates/host/src/room.rs:403` à `:416`.
- **Mécaniques :** identité, endurance.
- **Question :** que garde la partie d'un appareil qui n'a plus de place, et quand l'oublie-t-elle ?
- **Pression :** `an_evening_leaves_nothing_in_the_room` (`crates/host/src/room/simulated.rs:586`), `a_phone_that_never_played_is_forgotten_after_the_grace`.
- **Sous la nouvelle forme :** pression du commun : « Mémoire bornée » est une loi commune, mesurée sur une soirée entière.
- **Étage :** commun.

## Vote, points, égalités

### PUN-16 · Un bulletin déposé compte ; un bulletin absent n'est pas inventé
- **Ce qui arrive :** il fallait trancher ce que vaut le vote d'un joueur parti. Décision : déposé, il compte, même si le téléphone s'endort ensuite, et son retour ne donne pas de second bulletin ; jamais déposé, il n'est ni tiré au sort ni reporté. Le catalogue (VOTE-04) titrait l'inverse, mais demandait une décision écrite.
- **D'où on le sait :** commit `6daa637` ; `LEARNINGS.md`, « Ce que le jeu promet », ligne « Que devient un vote absent ? ».
- **Mécaniques :** vote.
- **Question :** que devient le bulletin d'un votant parti puis revenu ?
- **Pression :** `a_ballot_outlives_the_phone_that_cast_it` (J15), `a_lone_voter_who_locks_their_phone_keeps_their_vote`, `a_ballot_stands_whatever_its_phone_does`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-17 · Le compteur de votes ne lit jamais plus de bulletins que de votants
- **Ce qui arrive :** le compteur divisait tous les bulletins par les votants présents. Un votant qui votait puis se coupait faisait lire « 2/1 » à la télé.
- **D'où on le sait :** commit `6d2d05e` ; `crates/core/src/lib.rs:1153`.
- **Mécaniques :** vote, affichage.
- **Question :** un nombre affiché est-il le quotient de deux grandeurs mesurées sur le même ensemble ?
- **Pression :** `the_vote_counter_never_says_two_out_of_one` (J19), `the_vote_counter_adds_up` ; remettre le bug fait échouer la simulation dès la graine 0.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-18 · Un duel sans vote ne paie personne, et un vote que personne ne peut émettre se ferme aussitôt
- **Ce qui arrive :** à trois joueurs, le seul votant parti depuis plus de 20 s : attendre l'échéance n'y changerait rien. Personne n'a tranché, personne ne marque.
- **D'où on le sait :** commit `95dcbcc` ; `LEARNINGS.md`, ligne « Un duel sans vote ? ».
- **Mécaniques :** vote.
- **Question :** que paie un duel sans aucun bulletin, et combien de temps l'attend-on ?
- **Pression :** `a_vote_nobody_can_cast_ends_at_once`, `no_vote_means_no_points`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-19 · Le rang vient du score, et la couronne vient du rang
- **Ce qui arrive :** la télé numérotait le podium par position quand les téléphones classaient par score : deux gagnants à égalité lisaient 1 et 2, avec une seule couronne. Ensuite, à quatre ex æquo ou plus, le quatrième restait au rang 1 sans couronne dans la liste sous le podium : la couronne venait de la place sur le podium.
- **D'où on le sait :** commits `9e980d0`, `be4d198` ; `crates/core/src/views.rs:164`.
- **Mécaniques :** égalités, classement.
- **Question :** une égalité se lit-elle de la même façon sur tous les écrans et à toutes les places ?
- **Pression :** `equal_scores_share_their_rank` (J20), `every_winner_wears_a_crown_wherever_their_row_falls` ; `tools/fit-check.mjs` compte les couronnes d'une vraie page.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-20 · Les symboles et les emojis font partie d'une réponse
- **Ce qui arrive :** le jinx retirait tout ce qui n'est pas alphanumérique : deux réponses différentes en emojis donnaient le même texte vide, et le duel valait zéro.
- **D'où on le sait :** commit `6f9590a` ; `LEARNINGS.md`, « Trois bugs… », « Tous les emojis se valaient ».
- **Mécaniques :** texte, égalité de réponses.
- **Question :** la comparaison de deux textes retire-t-elle quelque chose qui porte du sens ?
- **Pression :** `answers_that_differ_only_by_their_symbols_are_not_a_jinx` (J12).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-21 · Seule la fin d'une phrase s'ignore, et un point d'interrogation garde son sens
- **Ce qui arrive :** le correctif des emojis retirait encore une liste de signes autour des mots, deux-points compris : `:)` et `;)`, `:D` et `D`, `Oui !` et `Oui ?` faisaient jinx. L'oracle recopiait la même liste et ne pouvait pas le voir. Le choix assumé : un jinx manqué coûte un vote, un faux jinx tout le duel.
- **D'où on le sait :** commit `ad8c11a` ; `LEARNINGS.md`, « Faux jinx » ; `crates/core/src/text.rs:121`.
- **Mécaniques :** texte, égalité de réponses.
- **Question :** dans le doute entre deux textes, quelle erreur coûte le moins au joueur ?
- **Pression :** `only_the_end_of_a_sentence_is_ignored` (décision écrite paire par paire), `the_key_follows_the_rule` (confronté à l'oracle `spec_key`, `crates/core/src/tests/properties.rs:692`).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-22 · Deux formes Unicode d'un même mot sont un seul texte, et les accents comptent
- **Ce qui arrive :** un `é` composé et un `é` décomposé donnaient deux réponses, donc pas de jinx, et deux prénoms. La comparaison doit aussi être la même sur tous les appareils, sans dépendre de leur langue : `pêche` n'est pas `péché`.
- **D'où on le sait :** commit `b58ad32` ; `crates/core/src/text.rs:106`.
- **Mécaniques :** texte.
- **Question :** l'égalité de deux textes dépend-elle de l'appareil qui les a saisis ?
- **Pression :** `two_unicode_forms_are_one_text`, `the_key_depends_on_no_language`, `names_are_compared_once_cleaned` (J22).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-23 · Ce qui ne se dessine pas n'est ni un prénom ni une réponse
- **Ce qui arrive :** U+200B et ses voisins passaient pour un prénom ou une réponse, et créaient un second « Léa ». Le premier correctif suivait la liste des ignorables par défaut d'Unicode : U+FFF9 à U+FFFB passaient encore. Toute la catégorie Cf est maintenant relue point par point ; seules ses marques préfixées restent, parce qu'elles se dessinent.
- **D'où on le sait :** commits `b58ad32`, `f1f5e19` ; `crates/core/src/text.rs:18`.
- **Mécaniques :** texte.
- **Question :** un texte sans encre peut-il occuper une place ou passer pour un autre ?
- **Pression :** `invisible_text_is_no_answer_and_no_name`, `the_format_category_is_read_point_by_point`, `an_invisible_character_does_not_make_a_second_name` (J21) ; `tools/text-check.mjs` sur le corpus T2.
- **Sous la nouvelle forme :** pression du commun : le commun promet « des prénoms validés » ; le nettoyage d'un texte saisi vaut pour tous les jeux qui en reçoivent.
- **Étage :** commun.

### PUN-24 · Une coupe tombe entre deux graphèmes et se mesure en capitales
- **Ce qui arrive :** une coupe à 80 caractères pouvait séparer un emoji composé. Et la télé affiche en capitales : `ß` en devient deux, `ﬃ` trois ; un texte « de 80 caractères » débordait le pire cas vérifié.
- **D'où on le sait :** commit `b58ad32` ; `crates/core/src/text.rs:69`.
- **Mécaniques :** texte, affichage.
- **Question :** la limite d'un texte compte-t-elle ce que l'écran dessine vraiment ?
- **Pression :** `a_text_is_cut_between_graphemes`, `capitals_stay_within_the_budget`, `a_cut_keeps_whole_graphemes`.
- **Sous la nouvelle forme :** pression du commun : le commun fournit les champs et les textes affichés ; la coupe et le budget doivent y être justes pour tous.
- **Étage :** commun.

### PUN-25 · Un prénom en double reçoit un suffixe qui tient dans la limite, et l'animateur a un nom réservé
- **Ce qui arrive :** les tests de propriétés ont trouvé un joueur qui s'appelait comme l'animateur, et des doublons dont le suffixe « 2 » dépassait les 12 caractères que la télé est vérifiée à tenir.
- **D'où on le sait :** commit `3f00326` ; `LEARNINGS.md`, « Bugs trouvés par la certification », dernier point ; `crates/core/src/lib.rs:800`.
- **Mécaniques :** salon, texte.
- **Question :** un nom construit par le jeu respecte-t-il les mêmes limites qu'un nom saisi ?
- **Pression :** `names_keep_to_the_limit_and_apart`, `names_fit_and_differ`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-26 · Un caractère retiré ne laisse pas deux espaces
- **Ce qui arrive :** retirer un caractère invisible entre deux espaces les laissait toutes les deux : deux prénoms identiques à l'œil restaient différents.
- **D'où on le sait :** commit `3f00326` ; `crates/core/src/text.rs:85` (nettoyage mot par mot).
- **Mécaniques :** texte.
- **Question :** le nettoyage est-il stable : nettoyer deux fois donne-t-il le même texte ?
- **Pression :** `cleaning_twice_is_cleaning_once`, `an_invisible_character_changes_no_name`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-27 · Un refus ne fait pas avancer le générateur
- **Ce qui arrive :** un « Je sèche » refusé tirait déjà une réponse de secours : le refus ne changeait rien à l'écran, mais décalait tous les tirages suivants.
- **D'où on le sait :** commit `3f00326` ; `crates/core/src/lib.rs:714` ; `LEARNINGS.md`, « Bugs trouvés par la certification ».
- **Mécaniques :** hasard.
- **Question :** une entrée refusée laisse-t-elle une trace dans l'état, même invisible ?
- **Pression :** `a_refused_stock_answer_draws_nothing`, `refused_input_changes_nothing`.
- **Sous la nouvelle forme :** impossible par la forme : « Un refus ne change rien : le jeu lit l'ancien état, et ne produit un nouvel état que s'il accepte l'entrée ».
- **Étage :** commun.

### PUN-28 · Deux « Je sèche » sur un même duel ne se jinxent jamais
- **Ce qui arrive :** deux joueurs qui abandonnaient la même consigne pouvaient tirer la même réponse de secours, et perdre le duel sur un jinx que personne n'avait joué.
- **D'où on le sait :** commit `9c225fb` ; `crates/core/src/lib.rs:1012`.
- **Mécaniques :** hasard, égalité de réponses.
- **Question :** le hasard peut-il produire une situation qu'aucun joueur n'a choisie et qui le punit ?
- **Pression :** `a_stock_answer_never_jinxes_its_rival`, `a_single_stock_answer_still_answers` (J23).
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-29 · Une manche ne montre jamais deux fois la même consigne
- **Ce qui arrive :** le paquet se remélangeait dès qu'il était vide, au milieu d'une manche : une longue soirée à table pleine pouvait montrer deux fois une consigne dans la même manche. Un paquet neuf passe maintenant sous un paquet trop mince pour la manche suivante.
- **D'où on le sait :** commit `4025d5e` ; `crates/core/src/lib.rs:861`.
- **Mécaniques :** hasard, paquet.
- **Question :** un paquet qui se remélange peut-il redonner une carte déjà sortie dans la même manche ?
- **Pression :** `a_prompt_waits_for_the_deck_to_be_spent`, `each_prompt_goes_to_two_players`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-30 · Deux parties d'une soirée ne rejouent pas les mêmes consignes
- **Ce qui arrive :** une relance qui repartirait de la même graine ou de paquets neufs reproduirait la partie précédente. Les paquets et le générateur continuent d'une partie à l'autre.
- **D'où on le sait :** `crates/core/src/lib.rs:737` (relance) ; catalogue HAS-04 ; `crates/host/src/main.rs:127`.
- **Mécaniques :** hasard, progression.
- **Question :** que garde-t-on du hasard quand on relance une partie dans la même soirée ?
- **Pression :** `two_games_of_an_evening_differ`, `two_launches_draw_two_seeds`.
- **Sous la nouvelle forme :** question de jeu : le commun fournit la graine, le jeu décide ce qui continue à la relance.
- **Étage :** jeu.

### PUN-31 · Le partage des points est monotone et borné
- **Ce qui arrive :** les promesses du barème n'étaient écrites nulle part : un vote de plus ne doit jamais coûter, un vote pour le rival ne rapporte rien, à votes égaux on gagne autant, un partage verse au plus la cagnotte et au moins la cagnotte moins 1, une réponse de secours garde exactement la moitié arrondie vers le bas.
- **D'où on le sait :** `LEARNINGS.md`, « Ce que Verus prouve des règles », J33 ; `crates/rules/src/lib.rs:331`.
- **Mécaniques :** score, vote.
- **Question :** quelles inégalités le barème doit-il toujours respecter, quel que soit le nombre de votes ?
- **Pression :** prouvé par Verus (`tools/verify-rules.py --mutations`) ; `one_more_vote_never_costs_a_side_any_points`, `a_vote_for_the_rival_never_pays`, `equal_votes_earn_the_same`, `sharing_the_votes_loses_at_most_a_point`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-32 · Un partage de points ne sature pas
- **Ce qui arrive :** le calcul multipliait en 32 bits avec saturation. À partir de 715 828 votes en finale, le côté qui recevait un vote de plus perdait des points. Six votants n'y arrivent pas, et les tests s'arrêtaient à huit : seule la preuve l'a vu. Le calcul se fait maintenant en 64 bits.
- **D'où on le sait :** commit `ac8421b` ; `LEARNINGS.md`, « Ce que la preuve a appris » ; `crates/rules/src/lib.rs:268`.
- **Mécaniques :** score.
- **Question :** l'arithmétique qui protège du débordement change-t-elle le sens de la promesse ?
- **Pression :** mutant `the-old-saturating-share`, refusé par Verus.
- **Sous la nouvelle forme :** connaissance : la règle d'écriture impose de choisir `checked_`, `saturating_` ou `wrapping_` selon le sens ; saturer peut casser une monotonie, et seul un énoncé sans précondition le révèle.
- **Étage :** connaissance.

### PUN-33 · « Zéro vote, zéro point » ne vaut que pour un duel voté
- **Ce qui arrive :** écrite telle quelle, la promesse est fausse : un forfait paie sans vote. Le type du duel sépare maintenant « sans réponse », « forfait », « jinx » et « voté ».
- **D'où on le sait :** `LEARNINGS.md`, « Ce que la preuve a appris » ; `crates/rules/src/lib.rs:96`.
- **Mécaniques :** score.
- **Question :** une promesse sur le score tient-elle dans tous les cas de fin, ou seulement dans le cas courant ?
- **Pression :** `a_duel_without_a_vote_pays_by_the_table`, `a_duel_pays_what_the_table_says`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

## Temps et robots

### PUN-34 · Après un gel, la phase suivante repart de maintenant
- **Ce qui arrive :** la phase suivante repartait de l'échéance passée : un réveil en retard de 3 s la raccourcissait de 3 s à l'écran, et un gel de 10 minutes menait au podium d'un coup. Le test de J5 figeait ce comportement contraire à J5. Décidé par le mainteneur : le temps perdu est jeté, la partie s'allonge.
- **D'où on le sait :** commit `7e4ae4b` ; `LEARNINGS.md`, « J5, tranché par l'hôte » ; `crates/core/src/lib.rs:1238`, `crates/rules/src/lib.rs:593` ; catalogue TR-02.
- **Mécaniques :** tour par tour à échéance, temps.
- **Question :** quand l'ordinateur de la soirée se réveille en retard, qui paie le temps perdu ?
- **Pression :** `a_long_freeze_does_not_run_through_the_phases_it_slept_over`, `a_late_tick_does_not_shorten_the_next_phase`, `a_phase_gets_its_whole_length_from_when_it_really_starts`.
- **Sous la nouvelle forme :** question de mécanique : le commun fournit des pas réguliers, mais un onglet suspendu ou un gel reste possible ; la politique du temps perdu est celle du genre.
- **Étage :** mécanique.

### PUN-35 · Ce que font les robots ne dépend pas du moment où la partie se réveille
- **Ce qui arrive :** les robots agissaient sur une phase dont l'échéance était passée avant le réveil : le tirage dépendait de l'instant où l'hôte se réveillait, et deux exécutions d'une même soirée divergeaient.
- **D'où on le sait :** commit `9fdd9b5` ; `crates/core/src/lib.rs:1342`.
- **Mécaniques :** robots, temps.
- **Question :** une décision de robot dépend-elle d'autre chose que de l'état et du pas courant ?
- **Pression :** `when_the_room_wakes_up_does_not_change_what_bots_do` (J16).
- **Sous la nouvelle forme :** impossible par la forme : « Le temps avance par pas réguliers, fournis par le commun » ; il n'y a plus d'instant de réveil variable entre deux exécutions.
- **Étage :** commun.

### PUN-36 · Les robots et l'animateur n'ont pas de téléphone à perdre
- **Ce qui arrive :** les robots avaient une connexion fantôme : un départ pouvait leur être appliqué, et ils entraient dans les comptes de présence.
- **D'où on le sait :** commit `9fdd9b5` ; `crates/core/src/lib.rs:632`.
- **Mécaniques :** robots, présence.
- **Question :** un joueur sans appareil peut-il être compté absent ou présent ?
- **Pression :** `a_bot_has_no_phone_to_lose`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-37 · Il n'y a pas de pause : une partie finit toujours
- **Ce qui arrive :** choix délibéré face aux absences : chaque phase garde son échéance. Une coupure plus longue que l'échéance perd la phase en cours. Sans cela, un téléphone dans une poche bloquerait la soirée.
- **D'où on le sait :** `LEARNINGS.md`, « Ce que le jeu promet », ligne « Et les autres phases ? » et « Trois bugs… ».
- **Mécaniques :** tour par tour à échéance.
- **Question :** une phase peut-elle attendre indéfiniment un geste humain ?
- **Pression :** `silence_never_blocks_the_game`, `a_game_with_nothing_in_it_still_ends`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

## Actions, vues et reconnexion

### PUN-38 · Une action faite pour une phase passée est refusée
- **Ce qui arrive :** un vote parti pendant un duel arrivait au duel suivant ; un Espace pressé deux fois passait deux phases ; une réponse arrivait après la fin de l'écriture. Chaque action porte maintenant la phase que l'écran voyait, et le téléphone lit « Trop tard : l'écran a changé. ».
- **D'où on le sait :** commit `0b70753` ; `crates/host/src/room.rs:348` et `:394` ; catalogue E5.
- **Mécaniques :** tour par tour, vote.
- **Question :** à quoi s'applique un geste qui arrive après que l'écran a changé ?
- **Pression :** `a_late_vote_does_not_land_on_the_next_duel` (`crates/host/src/room/properties.rs:592`), `a_key_pressed_twice_skips_once` (`:633`), `stale_actions_change_nothing`.
- **Sous la nouvelle forme :** pression du commun : le commun ordonne les gestes « dans l'ordre où ils ont été faits » ; il doit garantir à tous les jeux qu'un geste s'applique à ce que l'appareil voyait, ou qu'il est refusé et que l'appareil le sait.
- **Étage :** commun.

### PUN-39 · Un compte à rebours renvoyé après une reconnexion ne s'allonge pas
- **Ce qui arrive :** une vue portait le temps restant au moment où elle avait été faite. Renvoyée après une reconnexion, elle redonnait ce temps en entier. La vue porte maintenant l'heure de l'hôte, et l'écran retire le retard qu'il mesure.
- **D'où on le sait :** `LEARNINGS.md`, « Réseau : salon », « Les comptes à rebours » ; `crates/web/src/lib.rs:418`.
- **Mécaniques :** temps, affichage.
- **Question :** un minuteur affiché dépend-il du moment où l'appareil a reçu sa vue ?
- **Pression :** `countdowns_stay_in_bounds` ; aucune pression sur un vrai réseau.
- **Sous la nouvelle forme :** impossible par la forme : « Les appareils partagent des numéros de pas, jamais une heure », et la vue se calcule sur l'appareil depuis la projection.
- **Étage :** commun.

### PUN-40 · Ce qu'on tape survit à un redessin
- **Ce qui arrive :** chaque redessin recréait le champ du prénom vide, et une reconnexion redessine. Le prénom tapé disparaissait pendant la frappe.
- **D'où on le sait :** commit `ee0417d` ; `LEARNINGS.md`, « Le prénom effacé pendant la frappe ».
- **Mécaniques :** texte saisi, page téléphone.
- **Question :** un champ en cours d'édition garde-t-il son contenu, son curseur et sa composition à travers un réaffichage ?
- **Pression :** `tools/ui-check.mjs` (U1) observe l'identité réelle des éléments, le focus et la sélection.
- **Sous la nouvelle forme :** pression du commun : les champs de texte sont des composants du commun, qui garde leur identité « tant que son objet existe ».
- **Étage :** commun.

### PUN-41 · Un téléphone qui s'ouvre attend de savoir qui il est
- **Ce qui arrive :** une page qui s'ouvre ne sait pas encore si elle est un joueur qui revient. Demander tout de suite le prénom fait taper un nom pour rien, ou en crée un second. Elle dit « Un instant… » jusqu'à ce que la partie lui parle.
- **D'où on le sait :** `LEARNINGS.md`, « Le prénom effacé pendant la frappe » (`PhoneState::heard`) ; `crates/web/src/phone.rs`, écran « Un instant… ».
- **Mécaniques :** identité, page téléphone.
- **Question :** un appareil affiche-t-il une demande qui dépend d'une réponse qu'il n'a pas encore ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** pression du commun : l'identité est au commun, qui sait avant le jeu si un appareil revient.
- **Étage :** commun.

### PUN-42 · Après un redémarrage, le téléphone dit pourquoi il redemande un prénom
- **Ce qui arrive :** après un redémarrage de l'hôte, un téléphone lisait « Choisis un prénom. » sans comprendre. Il lit maintenant « La partie a recommencé : choisis ton prénom pour revenir. », et l'ancien prénom quitte la barre.
- **D'où on le sait :** commit `ee0417d` ; `crates/web/src/lib.rs:436` ; catalogue C7.
- **Mécaniques :** reprise.
- **Question :** après une panne de l'ordinateur de la soirée, que perd un joueur, et le sait-il ?
- **Pression :** cert `restart` (ancien harnais).
- **Sous la nouvelle forme :** impossible par la forme : « Une seule façon de repartir : un appareil retrouve sa place, le commun reprend la partie » ; un redémarrage ne fait plus recommencer la partie.
- **Étage :** commun.

### PUN-43 · La télé revient seule après un redémarrage de l'hôte
- **Ce qui arrive :** la clé de la télé changeait à chaque lancement de l'hôte, et la page ne la lisait qu'à son ouverture : après un redémarrage, la télé était refusée au lieu de revenir. Corrigé dans salon, qui garde la clé et relit `/salon/tv` après un refus.
- **D'où on le sait :** `LEARNINGS.md`, « La TV refusée après un redémarrage de l'hôte (C7), avec salon » ; vérifié à la main le 27 octobre 2026.
- **Mécaniques :** reprise, rôles d'appareil.
- **Question :** l'appareil qui tient la partie peut-il se retrouver exclu par sa propre relance ?
- **Pression :** aucune automatique.
- **Sous la nouvelle forme :** obsolète : la partie qui fait foi tourne dans le navigateur de la télé elle-même, et il n'y a plus de clé de télé distribuée par un serveur local à relire.
- **Étage :** commun.

### PUN-44 · Une page d'une autre version propose « Recharger », pas « Réessayer »
- **Ce qui arrive :** « Réessayer » renvoyait la même version et échouait encore. Vérifié avec une version 3 face à une page de version 2 : les deux pages affichent « Le jeu a été mis à jour : recharge la page. ».
- **D'où on le sait :** `LEARNINGS.md`, « Une page d'une autre version » ; `crates/web/src/lib.rs:351`.
- **Mécaniques :** versions.
- **Question :** un appareil resté sur une ancienne version peut-il se réparer seul ?
- **Pression :** vérification manuelle du 27 octobre 2026.
- **Sous la nouvelle forme :** impossible par la forme : « Le module du jeu arrive ensuite par la connexion, depuis la partie qui fait foi : un appareil a toujours exactement sa version ».
- **Étage :** commun.

### PUN-45 · Deux onglets pour un même joueur ne s'éjectent pas sans fin
- **Ce qui arrive :** le nouvel onglet éjectait l'ancien, qui se reconnectait aussitôt et éjectait le nouveau, sans fin. L'ancien onglet propose maintenant « Reprendre ici ».
- **D'où on le sait :** commit `db9be53` ; `LEARNINGS.md`, « Bugs trouvés en jouant » ; `soiree/salon/LEARNINGS.md`.
- **Mécaniques :** identité.
- **Question :** que fait l'ancien appareil quand un autre prend sa place ?
- **Pression :** `a_second_tab_takes_over_and_the_first_stays_put` (salon) ; côté jeu `one_device_per_player`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-46 · Un Wi-Fi mort ne ferme pas la connexion
- **Ce qui arrive :** quand le Wi-Fi meurt, le navigateur garde la socket ouverte et ne livre jamais `onclose`. Le téléphone ne voyait rien. Corrigé par un signe de vie toutes les 2 s et une reconnexion après 6 s de silence, puis passé à salon.
- **D'où on le sait :** commit `ee0417d` ; `LEARNINGS.md`, « Bugs trouvés par la certification » (C3).
- **Mécaniques :** réseau.
- **Question :** comment un appareil sait-il que le lien est mort quand rien ne le lui dit ?
- **Pression :** `silence_is_noticed_on_both_sides` (salon) ; cert « Wi-Fi silencieux » de l'ancien harnais.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-47 · Un téléphone qui inonde l'hôte ne ralentit pas les autres
- **Ce qui arrive :** un téléphone qui envoyait en rafale retardait tous les écrans de plus de 6 s. Corrigé par un budget de 20 messages par seconde, rafales de 40, puis passé à salon.
- **D'où on le sait :** commit `ee0417d` ; `LEARNINGS.md`, « Bugs trouvés par la certification » (E3).
- **Mécaniques :** réseau.
- **Question :** un appareil peut-il dégrader le service des autres ?
- **Pression :** `a_flooding_phone_does_not_slow_down_the_others` (salon).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-48 · Un téléphone ne devient jamais la télé, et ne laisse pas de joueur fantôme
- **Ce qui arrive :** une connexion de téléphone qui se redéclarait télé laissait un joueur « connecté » sans appareil. Le rôle de télé n'était accordé qu'à une connexion venant de l'ordinateur de la soirée.
- **D'où on le sait :** commits `3f00326`, `0b70753` ; `LEARNINGS.md`, « Tests retirés au passage à salon ».
- **Mécaniques :** rôles d'appareil, identité.
- **Question :** un appareil peut-il changer de rôle, et que devient alors le joueur qu'il portait ?
- **Pression :** `a_phone_that_says_hello_as_the_tv_stays_a_phone`, `a_phone_token_never_becomes_the_tv` (salon).
- **Sous la nouvelle forme :** pression du commun : seul le commun crée une identité et un rôle d'appareil.
- **Étage :** commun.

### PUN-49 · Être devant la télé vaut autorisation, et un jeton ne se devine pas
- **Ce qui arrive :** pas de compte ni de mot de passe : scanner le QR code suffit. Les jetons d'appareil venaient de l'aléa du système, et l'URL du QR code ne portait aucun secret.
- **D'où on le sait :** commit `db9be53` ; `LEARNINGS.md`, « Authentification ».
- **Mécaniques :** identité.
- **Question :** que peut faire quelqu'un qui a vu le QR code, ou qui invente un jeton ?
- **Pression :** `a_restart_without_memory_makes_new_devices`, `a_join_never_takes_a_seat`.
- **Sous la nouvelle forme :** pression du commun : le nouveau contrat met une clé dans le QR code pour chiffrer de bout en bout ; la question reste, avec une réponse différente.
- **Étage :** commun.

### PUN-50 · Une vue n'est renvoyée que si elle change, compte à rebours mis à part
- **Ce qui arrive :** l'hôte renvoyait toutes les vues toutes les 5 s pour rafraîchir les minuteurs. Avec l'heure de l'hôte dans la vue, ce rafraîchissement est devenu inutile.
- **D'où on le sait :** `LEARNINGS.md`, « Les comptes à rebours » ; `crates/host/src/room.rs:443`.
- **Mécaniques :** réseau, affichage.
- **Question :** que coûte une vue envoyée sans changement ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète : le commun envoie la projection et l'appareil calcule sa vue ; le jeu n'envoie plus de vue.
- **Étage :** commun.

## Affichage de la télé

### PUN-51 · Le texte reste dans la zone sûre de la télé
- **Ce qui arrive :** l'horloge, la plaque de manche et les boutons d'organisation touchaient les bords de l'écran, que beaucoup de télés rognent. La scène est maintenant dessinée à 90 %, par une mise à l'échelle qui garde la mise en page vérifiée.
- **D'où on le sait :** commit `ee0417d` ; `web/style.css:267` ; catalogue T3.
- **Mécaniques :** affichage.
- **Question :** ce qui doit se lire tient-il dans la zone que toute télé affiche ?
- **Pression :** cert T3 de l'ancien harnais.
- **Sous la nouvelle forme :** pression du commun : le commun possède l'affichage et connaît la télé ; la zone sûre vaut pour tous les jeux.
- **Étage :** commun.

### PUN-52 · La mise en page de la télé ne dépend pas de la fenêtre
- **Ce qui arrive :** dans une fenêtre plus étroite que 16:9, le texte sortait des bulles : les tailles suivaient la hauteur de la fenêtre et les largeurs sa largeur, donc le texte passait à la ligne plus souvent. La mise en page n'avait été vue qu'en 1600 × 900. La télé est maintenant une scène 16:9 dont toutes les longueurs sont des parts.
- **D'où on le sait :** commit `15c2f67` ; `LEARNINGS.md`, « Mise en page déterministe de la TV ».
- **Mécaniques :** affichage de texte.
- **Question :** les retours à la ligne sont-ils les mêmes sur tous les écrans ?
- **Pression :** `tools/fit-check.mjs`, qui échoue si la taille du texte change d'un format à l'autre.
- **Sous la nouvelle forme :** question de mécanique : le langage de dessin est au commun, mais la scène fixe s'impose à tout jeu qui affiche du texte de longueur variable.
- **Étage :** mécanique.

### PUN-53 · Le cadre d'une bulle est la boîte de son texte
- **Ce qui arrive :** un SVG étiré sur une boîte qui grandit avec le texte déformait ses bords. La bulle est maintenant dessinée depuis la boîte du texte, et ses tenons et sa queue sont des pièces de taille fixe qui dépassent. Rien n'est étiré, et le texte ne peut pas sortir du cadre.
- **D'où on le sait :** commits `faadc7a`, `15c2f67` ; `crates/web/src/clay.rs:272`.
- **Mécaniques :** affichage.
- **Question :** un cadre décoratif suit-il son contenu, ou le contenu doit-il tenir dans le cadre ?
- **Pression :** `tools/fit-check.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-54 · Les pires cas se construisent depuis les limites et se vérifient par machine
- **Ce qui arrive :** rien ne garantissait qu'une réponse de 80 caractères, ou trois en finale, tienne à l'écran. La page des pires cas montre chaque écran avec les textes les plus longs en lettres larges, un mot de 80 lettres sans espace, la table pleine, l'égalité à huit, et un contrôle l'ouvre sous cinq formats. Un test oblige à ajouter un pire cas pour tout nouvel écran.
- **D'où on le sait :** commit `15c2f67` ; `crates/core/src/gallery.rs` ; `tools/fit-check.mjs:23`.
- **Mécaniques :** affichage de texte.
- **Question :** chaque écran a-t-il un pire cas construit depuis les constantes du protocole, et une machine le regarde-t-elle ?
- **Pression :** `gallery_has_a_worst_case_for_every_tv_screen` ; `node tools/fit-check.mjs`.
- **Sous la nouvelle forme :** pression du commun : le commun peut rendre chaque pire cas déclaré par un jeu sous plusieurs formats et y chercher débordements et chevauchements.
- **Étage :** commun.

### PUN-55 · Le contenu est validé à son chargement
- **Ce qui arrive :** une consigne trop longue pour la télé, une accolade mal écrite (`{jouer}`) ou un fichier vide ne se voyaient qu'en jouant. L'hôte refuse maintenant de démarrer, et un test lit les vrais fichiers par la même fonction.
- **D'où on le sait :** `crates/core/src/lib.rs:86` ; `LEARNINGS.md`, « Écarts au manifeste », point 8.
- **Mécaniques :** contenu.
- **Question :** un contenu que les écrans ne savent pas montrer peut-il atteindre une partie ?
- **Pression :** `shipped_content_passes_the_hosts_check`, `content_the_screens_cannot_show_is_refused`.
- **Sous la nouvelle forme :** pression du commun : « Les fichiers de contenu sont validés à leur chargement et vérifiés par un test ».
- **Étage :** commun.

### PUN-56 · Une réponse acceptée apparaît telle quelle sur la télé, dans son cadre
- **Ce qui arrive :** l'absence de panique ne suffit pas : un texte hostile peut être accepté puis mal dessiné. Les 23 entrées du corpus commun passent comme prénoms puis réponses par de vraies pages, et une réponse acceptée doit apparaître littéralement sur la télé, dans son cadre. Les contrôles négatifs coupent un vrai nœud dans un cadre de 1 px pour vérifier que l'oracle sait échouer.
- **D'où on le sait :** `LEARNINGS.md`, « Corpus T2 dans les vrais écrans » ; `tools/text-check.mjs`.
- **Mécaniques :** texte, affichage.
- **Question :** ce que la partie a accepté est-il ce que l'écran montre ?
- **Pression :** `tools/text-check.mjs` (Chromium et WebKit).
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-57 · On mesure un texte avec ses vraies polices
- **Ce qui arrive :** mesurer avant le chargement des polices, c'est mesurer la police de secours : le texte rétrécit pour rien, ou déborde une fois la vraie police arrivée.
- **D'où on le sait :** `web/index.html` (chargement des trois polices avant l'initialisation) ; `LEARNINGS.md`, « Pièges rencontrés ».
- **Mécaniques :** affichage de texte.
- **Question :** la mesure d'un texte attend-elle la police qui le dessinera ?
- **Pression :** `tools/fit-check.mjs`.
- **Sous la nouvelle forme :** connaissance : vraie dans un navigateur ; le commun la porte s'il dessine le texte lui-même.
- **Étage :** connaissance.

### PUN-58 · Un débordement vers le haut ou la gauche échappe à `scrollHeight`
- **Ce qui arrive :** une boîte centrée ou en `wrap-reverse` déborde vers le haut ou la gauche, et `scrollHeight` ne le compte pas. Le contrôle compare aussi les boîtes des enfants.
- **D'où on le sait :** `LEARNINGS.md`, « Pièges rencontrés en écrivant la vérification ».
- **Mécaniques :** affichage, vérification.
- **Question :** le contrôle de débordement voit-il les quatre côtés ?
- **Pression :** `tools/fit-check.mjs` (`dom::misfits`, `crates/web/src/dom.rs:373`).
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-59 · Une rotation décorative fausse les rectangles mesurés
- **Ce qui arrive :** les bulles penchées agrandissent leur rectangle à l'écran (`getBoundingClientRect`), et le contrôle voyait des chevauchements qui n'existent pas. On mesure les boîtes de mise en page sans transformation, et l'on centre avec des marges automatiques plutôt qu'avec une translation.
- **D'où on le sait :** `LEARNINGS.md`, « Pièges rencontrés en écrivant la vérification ».
- **Mécaniques :** affichage, vérification.
- **Question :** la mesure porte-t-elle sur la boîte de mise en page ou sur son dessin transformé ?
- **Pression :** `tools/fit-check.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-60 · Avec un interligne de 1, les lettres dépassent de leur ligne
- **Ce qui arrive :** à `line-height: 1`, les lettres dépassent d'environ 6 %. Mesuré au pixel, le résultat changeait selon la taille de la fenêtre. Un texte à budget a au moins 1,15.
- **D'où on le sait :** `LEARNINGS.md`, « Pièges rencontrés en écrivant la vérification » ; `web/style.css:298`.
- **Mécaniques :** affichage de texte.
- **Question :** la boîte d'un texte contient-elle toute son encre ?
- **Pression :** `tools/fit-check.mjs`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-61 · Une marge haute se confond avec la marge basse précédente
- **Ce qui arrive :** deux marges verticales voisines fusionnent en CSS : la place réservée disparaissait. On réserve la place avec un remplissage intérieur.
- **D'où on le sait :** `LEARNINGS.md`, « Pièges rencontrés en écrivant la vérification ».
- **Mécaniques :** affichage.
- **Question :** l'espace réservé autour d'un élément survit-il à ses voisins ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance : propre au CSS ; disparaît si le commun dessine sans le navigateur.
- **Étage :** connaissance.

### PUN-62 · Un filtre SVG coûte cher : le fond ne se redessine que quand il change
- **Ce qui arrive :** les filtres de pâte (bruit, déplacement, éclairage) sont lourds. Redessiner le tunnel à chaque vue, ou le faire tourner, coûterait des images. Il n'est redessiné que lorsque sa teinte change.
- **D'où on le sait :** commit `faadc7a` ; `LEARNINGS.md`, « Visuel » ; `crates/web/src/dom.rs:180`.
- **Mécaniques :** affichage, GPU.
- **Question :** un effet coûteux est-il recalculé alors que rien de ce qu'il montre n'a changé ?
- **Pression :** aucune mesure dédiée.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-63 · Huit pages téléphone sur un ordinateur font des pics GPU
- **Ce qui arrive :** détacher la grille du salon pendant le départ, ou changer sa géométrie, faisait perdre son travail de peinture : à huit téléphones simulés, l'annulation dépassait 50 ms. La grille reste en place, masquée et inerte, avec sa couche composée. Deux lots de 20 annulations ont donné 0/20 puis 1/20 au-dessus de 50 ms : le pic a disparu, sans promesse répétable.
- **D'où on le sait :** `LEARNINGS.md`, « Durée de vie et continuité d'édition » ; `crates/web/src/phone.rs:52`.
- **Mécaniques :** affichage, page téléphone.
- **Question :** une transition fréquente détruit-elle un rendu coûteux qu'elle va devoir refaire ?
- **Pression :** sondes ciblées dans `certification/`.
- **Sous la nouvelle forme :** connaissance : les budgets d'image sont mesurés par le commun, mais ce remède dépend du moteur de rendu du navigateur.
- **Étage :** connaissance.

### PUN-64 · La matière compte plus que les couleurs
- **Ce qui arrive :** la première version copiait les couleurs de Quiplash 3 sur des dégradés lisses : elle faisait toujours « page web ». Dans Quiplash 3, chaque surface est de la pâte photographiée : bords irréguliers, relief, traces de doigts, ombre portée.
- **D'où on le sait :** commit `faadc7a` ; `LEARNINGS.md`, « Visuel ».
- **Mécaniques :** direction artistique.
- **Question :** qu'est-ce qui donne son identité à la référence : ses couleurs ou sa matière ?
- **Pression :** aucune ; jugement humain.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-65 · Nos personnages ne ressemblent pas à ceux de Jackbox
- **Ce qui arrive :** Cactus et Goutte étaient trop proches du Cactus et de la Larme de Quiplash 3 : ils sont devenus Cornichon et Glaçon. Aucune image, aucun texte de Jackbox n'est dans le dépôt, qui est fait pour être partagé.
- **D'où on le sait :** commit `faadc7a` ; `LEARNINGS.md`, « Personnages » et « Contenu ».
- **Mécaniques :** contenu, assets.
- **Question :** un asset inspiré d'un jeu existant s'en distingue-t-il assez ?
- **Pression :** aucune ; jugement humain.
- **Sous la nouvelle forme :** connaissance : « nous créons nos propres assets ».
- **Étage :** connaissance.

### PUN-66 · Un son appartient à une phase, pas à un réaffichage
- **Ce qui arrive :** un signal sonore lié à la vue se rejouerait à chaque changement du compteur de votes ou à chaque reconnexion. L'identité du son est la phase (manche, consigne), sans le compteur ; une connexion nouvelle repart en silence ; une vue vieille de plus de 250 ms ne rejoue pas une annonce périmée.
- **D'où on le sait :** commit `532f9a0` ; `LEARNINGS.md`, « Audio des phases sur la TV » ; `crates/web/src/sound.rs:52`.
- **Mécaniques :** son.
- **Question :** un effet ponctuel peut-il être rejoué par un réaffichage, une reconnexion ou un rejeu ?
- **Pression :** `redraws_and_vote_counts_do_not_repeat_phase_audio`.
- **Sous la nouvelle forme :** impossible par la forme : « Un effet ponctuel, comme un son ou une vibration, porte une identité, pour qu'un réaffichage ou un rejeu ne le rejoue pas. Il porte aussi l'instant où il doit se produire ».
- **Étage :** commun.

## Téléphone et navigateurs

### PUN-67 · Safari zoome sur un champ de moins de 16 px
- **Ce qui arrive :** sur iPhone, toucher un champ dont le texte fait moins de 16 px zoome la page, et le joueur perd sa mise en page. Les champs font 24 px.
- **D'où on le sait :** `LEARNINGS.md`, « Technologies à étudier » (Safari iOS) et « Visuel » (saisie de 24 px).
- **Mécaniques :** page téléphone, texte saisi.
- **Question :** un champ de saisie déclenche-t-il le zoom automatique du navigateur ?
- **Pression :** aucune ; à vérifier sur un vrai iPhone.
- **Sous la nouvelle forme :** pression du commun : les champs de texte sont des composants du commun.
- **Étage :** commun.

### PUN-68 · Le verrou d'écran se demande après un geste et se perd en arrière-plan
- **Ce qui arrive :** certains navigateurs n'accordent le maintien de l'écran allumé qu'après un geste, et le retirent quand la page passe en arrière-plan. Le téléphone le redemande au premier toucher et à chaque retour au premier plan. Une demande dont la réponse arrive après qu'une autre l'a remplacée est refusée, et son verrou libéré.
- **D'où on le sait :** `crates/web/src/events.rs:186` ; `LEARNINGS.md`, « Durée de vie et continuité d'édition » (`OperationSlot`) ; Wake Lock à partir d'iOS 16.4.
- **Mécaniques :** page téléphone.
- **Question :** une capacité du navigateur qui se perd sans prévenir est-elle redemandée, et une réponse périmée est-elle ignorée ?
- **Pression :** `tools/ui-check.mjs` inverse des fins de demandes autour d'un masquage.
- **Sous la nouvelle forme :** pression du commun : « l'écran maintenu allumé » est une capacité déclarée par l'appareil, que le commun gère.
- **Étage :** commun.

### PUN-69 · Le clavier du téléphone redimensionne la page
- **Ce qui arrive :** à l'ouverture du clavier, la hauteur visible change, et une page en hauteur fixe cache le bouton « Envoyer ». La page suit la hauteur dynamique et respecte les encoches (`dvh`, `safe-area`).
- **D'où on le sait :** `LEARNINGS.md`, « Technologies à étudier » ; `web/style.css` (`.phone`, `min-height: 100dvh`, `env(safe-area-inset-*)`).
- **Mécaniques :** page téléphone.
- **Question :** l'action principale reste-t-elle visible clavier ouvert ?
- **Pression :** aucune ; à vérifier sur un vrai iPhone.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-70 · La composition d'un caractère bloque les validations prématurées
- **Ce qui arrive :** avec un clavier à composition (IME, accents morts), Entrée valide la composition, pas le formulaire. Envoyer à ce moment-là part avec un texte incomplet.
- **D'où on le sait :** `LEARNINGS.md`, « Durée de vie et continuité d'édition » ; `crates/web/src/events.rs:303`, `:329`, `:388`.
- **Mécaniques :** texte saisi.
- **Question :** une validation peut-elle partir pendant qu'un caractère se compose ?
- **Pression :** `tools/ui-check.mjs` (composition pendant une rotation).
- **Sous la nouvelle forme :** pression du commun : « Un élément en cours d'édition garde son identité, sa sélection et sa composition ».
- **Étage :** commun.

### PUN-71 · WebKit livre le redimensionnement avant le rendu, et un rendu suspendu fige l'attente
- **Ce qui arrive :** la première CI a expiré au premier redimensionnement d'un téléphone WebKit. Attendre un drapeau avec le sondage par défaut de Playwright pouvait expirer quand les rappels d'animation étaient suspendus. Puis la rotation retour pouvait commencer avant l'image suivante du paysage : WebKit livre `resize` avant les rappels d'animation et le calcul de mise en page.
- **D'où on le sait :** commits `0d40bd0`, `e7e7312` ; `LEARNINGS.md`, « Observer un redimensionnement réel » ; source WebKit citée, `Source/WebCore/page/Page.cpp#L2122` (commit `56453fdf`).
- **Mécaniques :** vérification sur navigateur.
- **Question :** un contrôle attend-il un vrai événement du navigateur, ou suppose-t-il un ordre que le moteur ne garantit pas ?
- **Pression :** le contrôle de rotation du harnais.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-72 · Un contrôle expiré reste un échec, et un diagnostic tardif ne valide rien
- **Ce qui arrive :** après l'expiration d'une observation, il est tentant d'attendre encore et d'accepter une réussite tardive. Le contrôle garde son échec ; un diagnostic passif de 3 s au plus relève ce qui arrive ensuite, sans rien valider.
- **D'où on le sait :** commits `c710a8c`, `6770a99`.
- **Mécaniques :** vérification.
- **Question :** un contrôle peut-il changer de verdict après son échéance ?
- **Pression :** aucune dédiée.
- **Sous la nouvelle forme :** connaissance : principe d'écriture des contrôles de l'épreuve.
- **Étage :** connaissance.

## Vérification

### PUN-73 · Une coupure courte ne change rien : le vérifier sur le vrai jeu
- **Ce qui arrive :** une soirée de deux parties, tirée au hasard, est jouée deux fois sur le vrai jeu, la seconde avec des coupures de moins de 20 s sur n'importe quel siège. Phases, réponses, bulletins et scores doivent être identiques à chaque pas. Cette propriété a trouvé trois bugs d'un coup : le vote retenu par un duelliste, le siège perdu sur « On rejoue ! », les robots qui dépendaient du réveil.
- **D'où on le sait :** commit `c305d8a` ; `LEARNINGS.md`, « Transparence d'une coupure (J18) » ; `crates/core/src/tests/properties.rs:1696`.
- **Mécaniques :** présence, reprise.
- **Question :** la même partie, avec et sans coupures courtes, a-t-elle la même histoire ?
- **Pression :** `a_short_cut_changes_neither_phases_nor_scores`, `rehearsals_reach_the_final_and_the_next_game`.
- **Sous la nouvelle forme :** pression du commun : le commun peut rejouer chaque jeu avec et sans coupures et comparer leur histoire définitive, comme il le fait avec et sans prédiction.
- **Étage :** commun.

### PUN-74 · Un oracle ne partage aucun code avec la production
- **Ce qui arrive :** l'oracle des propriétés appelait la fonction d'égalité du jeu et recopiait son arithmétique : une règle fausse ne pouvait pas le faire échouer. Plus tard, l'oracle du jinx recopiait la même liste de signes que le code. Il est devenu une table de décision écrite depuis `LEARNINGS.md`, plus un test qui donne chaque ligne en chiffres.
- **D'où on le sait :** commits `685fa72`, `ad8c11a` ; `crates/core/src/tests/properties.rs:751` (`expected_outcome`).
- **Mécaniques :** toutes.
- **Question :** un test peut-il échouer si la règle elle-même est fausse ?
- **Pression :** campagne de mutants (`soiree mutants`, `certification/mutants/`).
- **Sous la nouvelle forme :** pression du commun : l'épreuve injecte des bugs dans le jeu ; un oracle qui recopie le code laisse vivre le mutant, et le rapport le montre.
- **Étage :** commun.

### PUN-75 · Un harnais qui vérifie la survie ne voit pas une règle fausse
- **Ce qui arrive :** trois règles fausses (podium sans absents, manche brûlée par une coupure, emojis égaux) ont échappé à tous les scénarios : le harnais vérifiait que la partie survit, pas qu'elle est juste, et ses robots répondaient des textes ordinaires. Elles ont été trouvées en relisant les règles avec un cas de soirée en tête : un téléphone verrouillé, un routeur qui décroche, une réponse en emoji.
- **D'où on le sait :** `LEARNINGS.md`, « Trois bugs que le harnais ne pouvait pas voir » ; commit `6f9590a`.
- **Mécaniques :** toutes.
- **Question :** les bots et les scénarios jouent-ils les cas qui cassent les règles, pas seulement ceux qui cassent le programme ?
- **Pression :** aucune générale.
- **Sous la nouvelle forme :** pression du commun : l'épreuve doit nourrir les bots d'entrées hostiles (corpus de texte, verrouillages, silences) et vérifier les invariants de règle à chaque pas.
- **Étage :** commun.

### PUN-76 · Les téléphones simulés doivent jouer, et les scores se recomptent depuis les vues
- **Ce qui arrive :** les téléphones simulés se connectaient et se coupaient, mais ne jouaient pas. Ils répondent maintenant, abandonnent, se taisent, votent et se verrouillent après avoir voté, sur un réseau qui perd, jusqu'au salon suivant. Une table reconstruite depuis les seules vues de la télé vérifie chaque révélation et chaque score. Remettre le bug du « 2/1 » la fait échouer dès la graine 0.
- **D'où on le sait :** commit `d587766` ; `crates/host/src/room/simulated.rs:443`.
- **Mécaniques :** toutes.
- **Question :** la simulation vérifie-t-elle ce que voient les joueurs, ou seulement l'état interne ?
- **Pression :** `phones_that_play_are_paid_by_the_rules`, `whole_host_replays_thousands_of_seeded_deadline_races`.
- **Sous la nouvelle forme :** pression du commun.
- **Étage :** commun.

### PUN-77 · Un test qui boucle sur un mutant expire au lieu d'échouer
- **Ce qui arrive :** une aide de test attendait la phase d'écriture sans borne : sur une fin de phase cassée, trois mutants tournaient jusqu'au délai au lieu d'échouer. L'aide échoue maintenant tout de suite.
- **D'où on le sait :** commit `ba7b39d`.
- **Mécaniques :** vérification.
- **Question :** un test a-t-il une borne qui le fait échouer vite quand la partie ne progresse pas ?
- **Pression :** campagne de mutants.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-78 · Le rejeu compare chaque pas, et les journaux de référence contiennent de vrais gestes
- **Ce qui arrive :** `LEARNINGS.md` affirmait que chaque point de contrôle était comparé ; le rejeu ne comparait que le dernier : une dérive ne disait pas où elle commençait, et une dérive qui se résorbe ne se voyait pas. Le seul journal versionné ne contenait que des robots : ni arrivée, ni coupure, ni relance.
- **D'où on le sait :** commit `65b94a8` ; `LEARNINGS.md`, « Écarts au manifeste », point 3.
- **Mécaniques :** enregistrement, rejeu.
- **Question :** le rejeu dit-il à quel pas une partie diverge, et ses journaux exercent-ils les entrées d'une vraie soirée ?
- **Pression :** `a_versioned_evening_with_a_cut_and_a_new_game_replays`.
- **Sous la nouvelle forme :** pression du commun : le commun enregistre, rejoue et compare les états ; les journaux d'épreuve doivent contenir de vrais gestes.
- **Étage :** commun.

### PUN-79 · Un index de duel périmé ne doit pas tuer la partie
- **Ce qui arrive :** les règles et les vues indexaient directement les duels, les côtés et le contenu : un numéro de duel périmé pouvait faire paniquer le fil de la salle et arrêter la soirée.
- **D'où on le sait :** commit `d1821d5`.
- **Mécaniques :** toutes.
- **Question :** une donnée périmée peut-elle arrêter la partie ?
- **Pression :** lints `indexing_slicing`, `expect_used`, `panic` en erreur.
- **Sous la nouvelle forme :** pression du commun : règles d'écriture (« ni index hors bornes ») et loi « Ne jamais planter la partie », par pression et reprise.
- **Étage :** commun.

### PUN-80 · Une partie finie se lit dans les types : trois manches, pas un compteur
- **Ce qui arrive :** l'écran des scores passait à la manche suivante par `+1` sur un octet : rien n'empêchait un écran des scores de la finale d'ouvrir une quatrième manche, sauf que la suite n'y menait pas. La fin reposait sur un invariant de test. La manche n'a plus que trois valeurs, et chaque transition baisse une mesure finie.
- **D'où on le sait :** commit `ac8421b` ; `LEARNINGS.md`, « Ce que la preuve a appris » ; `crates/rules/src/lib.rs:628`, `:776`.
- **Mécaniques :** progression.
- **Question :** qu'est-ce qui garantit qu'une partie atteint sa fin : le type, ou le chemin que suit le code ?
- **Pression :** `a_game_only_moves_forward_and_soon_ends`, `a_game_has_exactly_three_rounds` ; prouvé par Verus (J35).
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-81 · La borne d'une boucle se calcule, et la compilation refuse une table trop grande
- **Ce qui arrive :** la boucle d'avancement était bornée à 64 sans que ce nombre ait été calculé. Le pire cas est 57 pas ; une vraie partie en fait 49. Un jeu à douze places aurait passé la borne : une assertion à la compilation refuse maintenant plus de personnages que de duels prévus.
- **D'où on le sait :** `LEARNINGS.md`, « Ce que la preuve a appris » ; `crates/core/src/lib.rs:58` ; `crates/rules/src/lib.rs:41`.
- **Mécaniques :** progression.
- **Question :** une borne de sécurité a-t-elle été calculée, et que se passe-t-il si une constante voisine change ?
- **Pression :** `a_game_of_instant_phases_ends_in_one_wake_up`.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

### PUN-82 · Une preuve porte sur ce qu'une fonction décide, pas sur ce qu'on lui passe
- **Ce qui arrive :** le seul vrai bug du chantier de preuve, celui du gel (PUN-34), était dans l'argument passé à la fonction de départ de phase, donc dans l'appelant. La fonction était prouvée, et restait juste avec la mauvaise heure.
- **D'où on le sait :** `LEARNINGS.md`, « Ce qu'elle n'a pas vu ».
- **Mécaniques :** vérification.
- **Question :** la preuve couvre-t-elle aussi ce que l'appelant lui donne ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance : « Une preuve ne répond qu'à la question qu'on lui pose ».
- **Étage :** connaissance.

### PUN-83 · Un mutant doit échouer sur la promesse visée, et une preuve peut être vide
- **Ce qui arrive :** compter les erreurs de Verus ne suffit pas : un `assert` qui échoue masque le `ensures` qui suit, et quatre mutants sur 27 tombaient sur une autre ligne que prévu. Un mutant cru attrapé par une promesse l'était par une autre. Et des hypothèses contradictoires rendraient un lemme vrai pour rien : trois contrôles y ajoutent `assert(false)`.
- **D'où on le sait :** commit `bbc5cf7` ; `LEARNINGS.md`, « Pièges des mutants » ; `tools/verify-rules.py`.
- **Mécaniques :** vérification.
- **Question :** un mutant tué l'est-il par la promesse qu'il visait, et la preuve pourrait-elle passer à vide ?
- **Pression :** `tools/verify-rules.py --mutations`.
- **Sous la nouvelle forme :** pression du commun : l'épreuve revérifie les preuves et injecte des bugs ; elle doit exiger que l'échec vienne de l'énoncé visé.
- **Étage :** commun.

### PUN-84 · Ce que coûte une preuve
- **Ce qui arrive :** mesuré le 2 novembre 2026 : 143 lignes exécutables, 354 de spécification et de preuve, soit 2,5 par ligne de code ; vérification en 1,1 s, 30 s avec 30 mutants ; 46 s de compilation à froid pour tous à cause des macros. Aucun échec du solveur : le coût était de découper le code et d'écrire des postconditions qui ne recopient pas le code.
- **D'où on le sait :** `LEARNINGS.md`, « Coût réel ».
- **Mécaniques :** vérification.
- **Question :** que coûte d'exiger une preuve, et pour qui ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance.
- **Étage :** connaissance.

## Origines et forme de l'ancien jeu

### PUN-85 · Le téléphone est un afficheur générique piloté par l'état
- **Ce qui arrive :** la manette de Jackbox n'a aucune règle : toute la logique tourne sur la machine de la télé, et le téléphone affiche un écran choisi par l'état du joueur, puis renvoie des actions courtes. N'importe quel téléphone joue sans rien installer.
- **D'où on le sait :** analyse REA du bundle `jackbox.tv` et de la manette Quiplash 3 (`LEARNINGS.md`, « Observé », captures `ev_6ba434c4…` et `ev_5c547eb1…`).
- **Mécaniques :** page téléphone.
- **Question :** la page téléphone détient-elle une règle ou un état de jeu ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** impossible par la forme : « Elle ne détient pas l'état du jeu, ne prédit rien elle-même et ne gère pas la connexion ».
- **Étage :** commun.

### PUN-86 · Le VIP lance la partie, et la salle se ferme au départ
- **Ce qui arrive :** chez Jackbox, le premier joueur lance quand assez de joueurs sont là, peut annuler le départ, choisit la suite après la partie, et la salle se verrouille quand la partie commence.
- **D'où on le sait :** manette Quiplash 3 (`vip_cancel`, `vip_postgame`) ; `LEARNINGS.md`, « Observé » et « Règles retenues ».
- **Mécaniques :** salon.
- **Question :** qui lance, qui annule, qui relance, et peut-on rejoindre une partie en cours ?
- **Pression :** `joining_names_characters_and_lobby`.
- **Sous la nouvelle forme :** question de mécanique.
- **Étage :** mécanique.

### PUN-87 · Modération, censure et public : sans objet entre amis
- **Ce qui arrive :** Jackbox a ajouté au fil des packs la modération, le filtre, le signalement, l'expulsion et un public de milliers de spectateurs, pour le streaming devant des inconnus. On joue entre amis dans un salon : jeté.
- **D'où on le sait :** registre `features` de `jackbox.tv` ; `LEARNINGS.md`, « Leçons à trier ».
- **Mécaniques :** salon.
- **Question :** une fonction de la référence répond-elle à notre usage, ou au sien ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** connaissance : décision à revoir si un jeu se joue un jour avec des inconnus.
- **Étage :** connaissance.

### PUN-88 · Les messages accordent le nombre
- **Ce qui arrive :** la télé et les téléphones affichaient « 0 points » et « 1 joueur(s) ». En français, 0 et 1 prennent le singulier.
- **D'où on le sait :** commit `e813703` (J32) ; `crates/core/src/views.rs:12`.
- **Mécaniques :** affichage de texte.
- **Question :** un message avec un nombre s'accorde-t-il avec ce nombre ?
- **Pression :** `a_counted_message_agrees_with_its_number`.
- **Sous la nouvelle forme :** question de jeu.
- **Étage :** jeu.

### PUN-89 · La graine de la soirée ne dépend pas d'un fichier du système
- **Ce qui arrive :** l'hôte ouvrait `/dev/urandom` pour tirer sa graine : il ne démarrait pas sous Windows.
- **D'où on le sait :** commit `92ecd0d` ; `crates/host/src/main.rs:127`.
- **Mécaniques :** hasard.
- **Question :** d'où vient le hasard de la soirée, sur chaque système visé ?
- **Pression :** `two_launches_draw_two_seeds` (HAS-04) ; compilé pour `x86_64-pc-windows-gnu`, pas exécuté.
- **Sous la nouvelle forme :** impossible par la forme : le hasard entre comme une entrée fournie par le commun, et le module enfermé n'atteint aucun fichier.
- **Étage :** commun.

### PUN-90 · La forme binaire des messages est figée par un test
- **Ce qui arrive :** le corpus de fuzz et les journaux de rejeu sont des octets. Un changement silencieux de l'encodage les rendait illisibles sans que rien n'échoue. Un test fige les octets de quelques messages.
- **D'où on le sait :** `LEARNINGS.md`, « Réseau : salon », « L'encodage » (`messages_have_a_stable_binary_shape`).
- **Mécaniques :** protocole, enregistrement.
- **Question :** une donnée enregistrée reste-t-elle lisible après un changement de code ?
- **Pression :** `messages_have_a_stable_binary_shape`.
- **Sous la nouvelle forme :** pression du commun : « Une donnée d'une version antérieure passe par une conversion explicite ».
- **Étage :** commun.

### PUN-91 · L'encodeur ne lit pas les énumérations à étiquette interne
- **Ce qui arrive :** `postcard` ne sait pas lire les énumérations serde à étiquette interne : il a fallu les retirer du protocole au passage à salon.
- **D'où on le sait :** `LEARNINGS.md`, « Réseau : salon », « L'encodage » ; `crates/core/src/protocol.rs` (en-tête).
- **Mécaniques :** protocole.
- **Question :** le format de transport accepte-t-il toutes les formes de types que le jeu déclare ?
- **Pression :** `views_travel_through_salon`.
- **Sous la nouvelle forme :** obsolète : accident d'un choix d'encodeur de l'ancienne architecture ; le commun possède ses types de bout en bout.
- **Étage :** commun.

### PUN-92 · Le temps entrait par des réveils toutes les 100 ms, sans pas fixe
- **Ce qui arrive :** la salle avançait par échéances, réveillée toutes les 100 ms, avec l'heure en paramètre de chaque entrée. C'était assumé comme écart au manifeste pour un jeu au tour par tour. Cette liberté a permis les bugs de PUN-34 et PUN-35.
- **D'où on le sait :** `LEARNINGS.md`, « Écarts au manifeste », point 2.
- **Mécaniques :** temps.
- **Question :** le temps d'un jeu au tour par tour dépend-il de la fréquence de réveil de l'hôte ?
- **Pression :** aucune.
- **Sous la nouvelle forme :** obsolète : le nouveau contrat fournit le temps par pas réguliers à tous les jeux, et « un jeu au tour par tour n'a rien de plus à faire ».
- **Étage :** commun.
