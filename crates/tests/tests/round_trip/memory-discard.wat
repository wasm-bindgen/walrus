(module
  (memory 1)

  (func (export "a")
    (memory.discard
      (i32.const 1)
      (i32.const 2))
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (memory (;0;) 1)
    (export "a" (func 0))
    (func (;0;) (type 0)
      i32.const 1
      i32.const 2
      memory.discard
    )
  )
;)
