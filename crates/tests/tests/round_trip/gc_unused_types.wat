;; Can remove an unused type.

(module
  (type (;0;) (func (result i32)))
  (type (;1;) (func (param i32)))
  (func $f (type 0) (result i32)
    i32.const 42)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      i32.const 42
    )
    (export "f" (func $f))
;)
