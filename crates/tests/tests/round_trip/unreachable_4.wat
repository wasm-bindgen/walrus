(module
  (func $f (result i32)
    unreachable
    i32.add)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      unreachable
    )
    (export "f" (func $f))
;)
