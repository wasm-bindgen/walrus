(module
  (type (func))
  (import "" "a" (func (type 0)))
  (import "" "b" (table 1 funcref))
  (import "" "c" (global i32))
  (import "" "d" (memory 1))
  )

;; CHECK: (module
