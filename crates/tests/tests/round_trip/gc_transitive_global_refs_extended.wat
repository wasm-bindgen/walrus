;; Keep transitively referenced globals through extended const expressions.
;; Global A uses Global B and Global C in an extended const expression (with arithmetic).
;; All used globals should be kept by GC.

(module
  ;; Base globals used in extended const expression
  (global $globalB i32 (i32.const 10))
  (global $globalC i32 (i32.const 32))

  ;; Global A - uses extended const expr: globalB + globalC
  (global $globalA i32
    (i32.add
      (global.get $globalB)
      (global.get $globalC)))

  ;; Unused global that should be removed
  (global $unused i32 (i32.const 999))

  ;; Function that uses Global A
  (func $f (result i32)
    global.get $globalA)

  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      global.get $globalA
    )
    (global $globalB (;0;) i32 i32.const 10)
    (global $globalC (;1;) i32 i32.const 32)
    (global $globalA (;2;) i32 global.get $globalB global.get $globalC i32.add)
    (export "f" (func $f))
;)
