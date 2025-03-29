(module
  (import "x" "y" (global i32))
  (table 1 funcref)
  (func)
  (elem (global.get 0) 0)
  (export "x" (table 0)))

(; CHECK-ALL:
  (module
    (type (;0;) (func))
    (import "x" "y" (global (;0;) i32))
    (func (;0;) (type 0))
    (table (;0;) 1 funcref)
    (export "x" (table 0))
    (elem (;0;) (global.get 0) func 0)
;)
