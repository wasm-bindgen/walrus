;; Can remove an unused table.

(module
  (type (;0;) (func (result i32)))
  (table 1 1 funcref)
  (func $f (type 0) (result i32)
    i32.const 42)
  (export "f" (func $f)))

;; CHECK: (module
;; NEXT: (type (;0;) (func (result i32)))
;; NEXT: (func $f (;0;) (type 0) (result i32)
;; NEXT: i32.const 42
;; NEXT: )
;; NEXT: (export "f" (func $f))
