;; Can remove an unused function.

(module
  (type (;0;) (func (result i32)))
  (func $unused (type 0) (result i32)
    i32.const 1337)
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
