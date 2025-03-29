(module
  (type (;0;) (func (result i32)))
  (func $a (type 0)
    i32.const 0)
  (func $b (type 0)
    (i32.add (i32.const 1) (i32.const 2)))
  (func $c (type 0)
    (i32.add
      (i32.const 3)
      (i32.add (i32.const 4) (i32.const 5))))
  (func $d (type 0)
    (i32.add
      (i32.add (i32.const 0) (i32.const 1))
      (i32.add (i32.const 2) (i32.const 3))))
  (export "a" (func $a))
  (export "b" (func $b))
  (export "c" (func $c))
  (export "d" (func $d)))

;; Note that these functions get properly sorted from largest to smallest.

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $d (;0;) (type 0) (result i32)
      i32.const 0
      i32.const 1
      i32.add
      i32.const 2
      i32.const 3
      i32.add
      i32.add
    )
    (func $c (;1;) (type 0) (result i32)
      i32.const 3
      i32.const 4
      i32.const 5
      i32.add
      i32.add
    )
    (func $b (;2;) (type 0) (result i32)
      i32.const 1
      i32.const 2
      i32.add
    )
    (func $a (;3;) (type 0) (result i32)
      i32.const 0
    )
    (export "a" (func $a))
    (export "b" (func $b))
    (export "c" (func $c))
    (export "d" (func $d))
;)
