(module
  (import "x" "y" (global i32))
  (memory 1)
  (func)
  (data (global.get 0) "")
  (export "x" (memory 0)))

(; CHECK-ALL:
  (module
    (import "x" "y" (global (;0;) i32))
    (memory (;0;) 1)
    (export "x" (memory 0))
    (data (;0;) (global.get 0) "")
;)
