(module
  (memory 1)

  (func (export "a")
    (memory.init 0
      (i32.const 1)
      (i32.const 2)
      (i32.const 3))
    (data.drop 2)

    (memory.copy
      (i32.const 1)
      (i32.const 2)
      (i32.const 3))

    (memory.fill
      (i32.const 1)
      (i32.const 2)
      (i32.const 3))
  )

  (data "A")
  (data (i32.const 0) "b")
  (data "C")
)

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (memory (;0;) 1)
    (export "a" (func 0))
    (func (;0;) (type 0)
      i32.const 1
      i32.const 2
      i32.const 3
      memory.init 0
      data.drop 2
      i32.const 1
      i32.const 2
      i32.const 3
      memory.copy
      i32.const 1
      i32.const 2
      i32.const 3
      memory.fill
    )
    (data (;0;) "A")
    (data (;1;) (i32.const 0) "b")
    (data (;2;) "C")
;)
