(module
  (type (func))

  (table funcref (elem 1))

  (func
    i32.const 0
    call_indirect
    )

  (func)

  (export "foo" (func 0))
  )

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func (;0;) (type 0)
      i32.const 0
      call_indirect (type 0)
    )
    (func (;1;) (type 0))
    (table (;0;) 1 1 funcref)
    (export "foo" (func 0))
    (elem (;0;) (i32.const 0) func 1)
;)
