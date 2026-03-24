;; Do not gc a used memory.

(module
  (memory $m 2)
  (export "m" (memory $m)))

;; CHECK: (module
;; NEXT: (memory $m (;0;) 2)
;; NEXT: (export "m" (memory $m))
