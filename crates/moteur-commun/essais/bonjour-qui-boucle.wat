;; Le module de `Bonjour` qui boucle (PHASE-3, P5 et M10 ; RESTE-PHASE-2, points 2 et 16).
;;
;; Le plus petit module que la réécriture accepte : une mémoire de deux pages, la pile
;; en bas, `soiree_boite` qui rend toujours la seconde page, et `soiree_appel` qui
;; boucle sans fin dès son entrée. Le tout premier appel du chargement, `Bonjour`, ne
;; rend donc jamais : seul le carburant de chargement l'arrête, et c'est la faute
;; `Budget` du jeu, jamais le garde-fou mural (une panne de l'outil, HIS-06).
;;
;; Il sert à deux choses : le test `bonjour_qui_boucle_est_une_faute_avec_le_garde_fou_de_chargement`
;; (`moteur-wasmtime`), et la mesure de la durée d'une unité de carburant sur une
;; boucle vide (M10) : dans wasmtime par l'exemple `boucle_vide` de `moteur-wasmtime`,
;; dans chaque navigateur par `soiree-banc etalonner` (lot 3c), qui le lit ici
;; (`moteur_commun::essais::BONJOUR_QUI_BOUCLE`). La boucle est celle du test de
;; `moteur-wasmtime/tests/moteur/charger.rs` : `(loop $l (br $l))`.
(module
  (memory (export "memory") 2 2)
  (global $__stack_pointer (mut i32) (i32.const 65536))
  (func (export "soiree_boite") (param $n i32) (param $sorte i32) (result i32)
    (i32.const 65536))
  (func (export "soiree_appel") (param $n i32) (result i64)
    (loop $l (br $l))
    (unreachable)))
