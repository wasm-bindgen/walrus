(module
  (import "" "" (memory 1))
  (export "b" (memory 0))
  )

(; CHECK-ALL:
  (module
    (import "" "" (memory (;0;) 1))
    (export "b" (memory 0))
;)
