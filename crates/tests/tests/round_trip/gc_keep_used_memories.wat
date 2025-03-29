;; Do not gc a used memory.

(module
  (memory $m 2)
  (export "m" (memory $m)))

(; CHECK-ALL:
  (module
    (memory $m (;0;) 2)
    (export "m" (memory $m))
;)