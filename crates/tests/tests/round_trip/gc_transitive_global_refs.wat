;; Keep transitively referenced globals through const expressions.
;; Global A is used by a function, Global A references Global B in its initializer.
;; Both should be kept by GC.

(module
  ;; Global B - referenced by Global A's initializer
  (global $globalB i32 (i32.const 42))

  ;; Global A - uses Global B in its initializer (simple const expr)
  (global $globalA i32 (global.get $globalB))

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
    (global $globalB (;0;) i32 i32.const 42)
    (global $globalA (;1;) i32 global.get $globalB)
    (export "f" (func $f))
;)
