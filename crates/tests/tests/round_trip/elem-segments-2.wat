(module
  (table 1 funcref)
  (func)
  (elem (i32.const 1) 0)
  (elem (i32.const 2) 0)
  (export "foo" (table 0))
  )

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (func (;0;) (type 0))
    (table (;0;) 1 funcref)
    (export "foo" (table 0))
    (elem (;0;) (i32.const 1) func 0)
    (elem (;1;) (i32.const 2) func 0)
;)