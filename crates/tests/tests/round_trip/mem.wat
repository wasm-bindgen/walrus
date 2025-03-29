(module
  (memory 0)
  (func (export "f")
    i32.const 1
    i32.load
    drop

    i32.const 2
    f32.const 3
    f32.store))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func (;0;) (type 0)
      i32.const 1
      i32.load
      drop
      i32.const 2
      f32.const 0x1.8p+1 (;=3;)
      f32.store
    )
    (memory (;0;) 0)
    (export "f" (func 0))
;)
