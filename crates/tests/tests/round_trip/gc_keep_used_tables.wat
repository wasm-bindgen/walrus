;; Can remove an unused table.

(module
  (type (;0;) (func (result i32)))
  (table 1 1 funcref)
  (export "t" (table 0)))

;; CHECK: (module
;; NEXT: (table (;0;) 1 1 funcref)
;; NEXT: (export "t" (table 0))
