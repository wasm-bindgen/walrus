(module
  (func $g (result i32)
    i32.const 42
    return)
  (func $f (result i32)
    return_call $g)
  (export "f" (func $f)))

(; CHECK-ALL:
  (module
    (type (;0;) (func (result i32)))
    (func $g (;0;) (type 0) (result i32)
      i32.const 42
      return
    )
    (func $f (;1;) (type 0) (result i32)
      return_call $g
    )
    (export "f" (func $f))
;)
