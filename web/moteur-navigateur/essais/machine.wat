;; Le module des essais de `Machine` dans le navigateur (PHASE-3, lot 3a).
;;
;; Il n'est pas réécrit : il éprouve les primitives d'un moteur, pas le protocole. Il a
;; les trois exports qu'une machine demande (`memory`, `soiree_boite`, `soiree_appel`)
;; et quelques globales de chaque type. `soiree_appel(k)` joue le scénario `k` :
;;
;;  0  rend l'i64 lu à l'adresse 1024 (ce que la machine y a écrit)
;;  1  écrit 0x1122334455667788 à l'adresse 2048, rend 0
;;  2  `unreachable`
;;  3  lit à l'adresse 0xFFFFFFF0 : hors de la mémoire
;;  4  une récursion sans fin : la pile native
;;  5  `i32.div_s` par zéro
;;  6  `i32.rem_s` par zéro
;;  7  `i32.div_s` de i32::MIN par -1 : débordement
;;  8  `i32.trunc_f32_s` d'un NaN
;;  9  `i32.trunc_f32_s` de 1e10 : hors des bornes
;; 10  `call_indirect` au-delà de la table
;; 11  `call_indirect` vers un élément nul
;; 12  `call_indirect` vers une fonction d'un autre type
;; 13  `memory.grow 1`, écrit 0x5a au premier octet de la page nouvelle, rend la taille
;; 14  rend la globale i64 `soiree_carburant`
;; 15  pose `soiree_carburant` à 0x8000000000000001, rend 0
;; 16  rend 0x0123456789abcdef
;; 17  rend -2
;; 18  rend les bits de la globale f64 `essai_f64`
;;
;; `soiree_boite(n, sorte)` rend `16 n + sorte`, ou 0 au-delà de 4 096.
(module
  (type $rend_i64 (func (result i64)))
  (type $rend_i32 (func (param i32) (result i32)))
  (memory (export "memory") 2 4)
  (global $pile (export "soiree_globale_0") (mut i32) (i32.const 65536))
  ;; -(2^53 + 1) : un nombre du navigateur ne le tient pas exactement.
  (global $carburant (export "soiree_carburant") (mut i64) (i64.const -9007199254740993))
  (global $f32 (export "essai_f32") (mut f32) (f32.const 1.5))
  (global $f64 (export "essai_f64") (mut f64) (f64.const -0.1))
  (global $fixe (export "essai_fixe") i32 (i32.const 7))
  (table 2 funcref)
  (elem (i32.const 0) $zero)
  (func $zero (type $rend_i64) (i64.const 0))
  (func $profond (param $n i32) (result i32)
    (i32.add (call $profond (i32.add (local.get $n) (i32.const 1))) (i32.const 1)))
  (func (export "soiree_boite") (param $n i32) (param $sorte i32) (result i32)
    (if (i32.gt_u (local.get $n) (i32.const 4096)) (then (return (i32.const 0))))
    (i32.add (i32.mul (local.get $n) (i32.const 16)) (local.get $sorte)))
  (func (export "soiree_appel") (param $k i32) (result i64)
    (local $f f32)
    (if (i32.eq (local.get $k) (i32.const 0))
      (then (return (i64.load (i32.const 1024)))))
    (if (i32.eq (local.get $k) (i32.const 1))
      (then
        (i64.store (i32.const 2048) (i64.const 0x1122334455667788))
        (return (i64.const 0))))
    (if (i32.eq (local.get $k) (i32.const 2)) (then (unreachable)))
    (if (i32.eq (local.get $k) (i32.const 3))
      (then (return (i64.extend_i32_u (i32.load (i32.const -16))))))
    (if (i32.eq (local.get $k) (i32.const 4))
      (then (return (i64.extend_i32_u (call $profond (i32.const 0))))))
    (if (i32.eq (local.get $k) (i32.const 5))
      (then (return (i64.extend_i32_s (i32.div_s (i32.const 1) (i32.sub (local.get $k) (i32.const 5)))))))
    (if (i32.eq (local.get $k) (i32.const 6))
      (then (return (i64.extend_i32_s (i32.rem_s (i32.const 1) (i32.sub (local.get $k) (i32.const 6)))))))
    (if (i32.eq (local.get $k) (i32.const 7))
      (then (return (i64.extend_i32_s
        (i32.div_s (i32.const 0x80000000) (i32.sub (i32.const 6) (local.get $k)))))))
    (if (i32.eq (local.get $k) (i32.const 8))
      (then
        (local.set $f (f32.div (f32.const 0) (f32.convert_i32_s (i32.sub (local.get $k) (i32.const 8)))))
        (return (i64.extend_i32_s (i32.trunc_f32_s (local.get $f))))))
    (if (i32.eq (local.get $k) (i32.const 9))
      (then (return (i64.extend_i32_s (i32.trunc_f32_s (f32.const 1e10))))))
    (if (i32.eq (local.get $k) (i32.const 10))
      (then (return (call_indirect (type $rend_i64) (i32.const 5)))))
    (if (i32.eq (local.get $k) (i32.const 11))
      (then (return (call_indirect (type $rend_i64) (i32.const 1)))))
    (if (i32.eq (local.get $k) (i32.const 12))
      (then (return (i64.extend_i32_s (call_indirect (type $rend_i32) (i32.const 3) (i32.const 0))))))
    (if (i32.eq (local.get $k) (i32.const 13))
      (then
        (drop (memory.grow (i32.const 1)))
        (i32.store8 (i32.mul (i32.sub (memory.size) (i32.const 1)) (i32.const 65536)) (i32.const 0x5a))
        (return (i64.extend_i32_u (memory.size)))))
    (if (i32.eq (local.get $k) (i32.const 14))
      (then (return (global.get $carburant))))
    (if (i32.eq (local.get $k) (i32.const 15))
      (then
        (global.set $carburant (i64.const 0x8000000000000001))
        (return (i64.const 0))))
    (if (i32.eq (local.get $k) (i32.const 16))
      (then (return (i64.const 0x0123456789abcdef))))
    (if (i32.eq (local.get $k) (i32.const 17))
      (then (return (i64.const -2))))
    (if (i32.eq (local.get $k) (i32.const 18))
      (then (return (i64.reinterpret_f64 (global.get $f64)))))
    (i64.const -1)))
