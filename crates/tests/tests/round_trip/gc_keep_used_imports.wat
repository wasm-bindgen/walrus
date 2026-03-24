;; Keep used imports.

(module
  (type (;0;) (func (result i32)))
  (import "env" "used" (func $used (type 0)))
  (export "used" (func $used)))

;; CHECK: (module
;; NEXT: (type (;0;) (func (result i32)))
;; NEXT: (import "env" "used" (func $used (;0;) (type 0)))
;; NEXT: (export "used" (func $used))
