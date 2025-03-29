(module
  (type (func))
  (import "" "" (func (type 0)))
  (export "b" (func 0))
  )

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (import "" "" (func (;0;) (type 0)))
    (export "b" (func 0))
;)
