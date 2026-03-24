;; Can remove two unused functions that call each other.

(module
  (type (;0;) (func (result i32)))

  ;; Two functions that are mutually recursive.
  (func $a (type 0) (result i32)
    call $b)
  (func $b (type 0) (result i32)
    call $a)

  ;; An unrelated function that we export.
  (func $f (type 0) (result i32)
    i32.const 42)
  (export "f" (func $f)))

;; CHECK: (module
;; NEXT: (type (;0;) (func (result i32)))
;; NEXT: (func $f (;0;) (type 0) (result i32)
;; NEXT: i32.const 42
;; NEXT: )
;; NEXT: (export "f" (func $f))
