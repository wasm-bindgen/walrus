;; Can remove an unused import.

(module
  (type (;0;) (func (result i32)))
  (import "env" "unused" (func $unused (type 0)))
  (func $f (type 0) (result i32)
    i32.const 42)
  (export "f" (func $f)))

;; CHECK: (module
;; NEXT: (type (;0;) (func (result i32)))
;; NEXT: (func $f (;0;) (type 0) (result i32)
;; NEXT: i32.const 42
;; NEXT: )
;; NEXT: (export "f" (func $f))
