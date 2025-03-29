(module
  (type (;0;) (func (result i32)))
  (import "env" "f" (func (;0;) (type 0)))
  (func $g (type 0) (result i32)
    (call 0))
  (export "g" (func $g)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (import "env" "f" (func (;0;) (type 0)))
    (func $g (;1;) (type 0) (result i32)
      call 0
    )
    (export "g" (func $g))
;)