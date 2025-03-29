(module
  (memory 1 1 shared)

  (func (export "atomics")
    (i32.atomic.rmw.cmpxchg
      (i32.const 0)
      (i32.const 1)
      (i32.const 2))

    (i32.atomic.rmw.add
      (i32.const 0)
      (i32.const 1))

    (memory.atomic.notify
      (i32.const 0)
      (i32.const 1))

    (memory.atomic.wait32
      (i32.const 0)
      (i32.const 1)
      (i64.const 2)
      )

    (memory.atomic.wait64
      (i32.const 0)
      (i64.const 1)
      (i64.const 2)
      )

    i32.add
    i32.add
    i32.add
    i32.add
    drop
  )
)

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func (;0;) (type 0)
      i32.const 0
      i32.const 1
      i32.const 2
      i32.atomic.rmw.cmpxchg
      i32.const 0
      i32.const 1
      i32.atomic.rmw.add
      i32.const 0
      i32.const 1
      memory.atomic.notify
      i32.const 0
      i32.const 1
      i64.const 2
      memory.atomic.wait32
      i32.const 0
      i64.const 1
      i64.const 2
      memory.atomic.wait64
      i32.add
      i32.add
      i32.add
      i32.add
      drop
    )
    (memory (;0;) 1 1 shared)
    (export "atomics" (func 0))
;)