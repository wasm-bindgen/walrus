(module
  (import "" "" (table 1 funcref))
  (export "b" (table 0))
  )

(; CHECK-ALL:
  (module
    (import "" "" (table (;0;) 1 funcref))
    (export "b" (table 0))
;)
