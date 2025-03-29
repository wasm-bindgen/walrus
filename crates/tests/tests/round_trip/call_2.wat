;; Calls with parameters.

(module
  (type (;0;) (func (param i32) (result i32)))
  (import "env" "f" (func (type 0)))
  (func $g (type 0) (param i32) (result i32)
    (local.get 0)
    (call 0))
  (export "g" (func $g)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (param i32) (result i32)))
    (import "env" "f" (func (;0;) (type 0)))
    (func $g (;1;) (type 0) (param i32) (result i32)
      local.get 0
      call 0
    )
    (export "g" (func $g))
;)