(module
  (func $f (result i32)
    (return (i32.const 1)))
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $f (;0;) (type 0) (result i32)
      i32.const 1
      return
    )
    (export "f" (func $f))
;)
